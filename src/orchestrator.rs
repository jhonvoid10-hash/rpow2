//! Top-level coordinator.
//!
//! Responsibilities:
//! 1. Spawn the [`mining pool`](crate::miner::scheduler) once at boot.
//! 2. Replenish state.json so we always have `target_accounts` Active
//!    accounts (registers more if short, refreshes Expired ones).
//! 3. For every Active account, spawn one async loop that:
//!    `/challenge` → mine → `/mint` → update state → maybe auto-pool → sleep.
//! 4. Listen on a [`tokio::sync::broadcast`] for shutdown signals and
//!    propagate them to every spawned task.
//!
//! The orchestrator is `'static` and runs until the shutdown receiver
//! fires (SIGINT, SIGTERM, or 'q' from the dashboard).

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use rand::Rng;
use tokio::sync::{broadcast, oneshot, Semaphore};
use tracing::{error, info, warn};

use crate::account::{AccountStatus, StateStore};
use crate::api::Rpow2Client;
use crate::auth;
use crate::config::Config;
use crate::dashboard::events::{DashBus, DashEvent};
use crate::email::EmailProvider;
use crate::error::{FarmError, FarmResult};
use crate::miner::scheduler::{MiningHandle, MiningJob};
use crate::miner::pow::hex_to_bytes;
use crate::pool;
use crate::throttle::GlobalThrottle;

/// Concurrency limit for parallel registrations. Many email backends
/// (especially mail.tm) have soft per-IP rate limits; doing 50
/// registrations in parallel just trips them. Keep small.
const REGISTER_PARALLELISM: usize = 4;

pub struct Orchestrator {
    cfg: Arc<Config>,
    state: StateStore,
    email_provider: Arc<dyn EmailProvider + Send + Sync>,
    mining: MiningHandle,
    bus: DashBus,
    shutdown: broadcast::Sender<()>,
    throttle: Arc<GlobalThrottle>,
}

impl Orchestrator {
    pub fn new(
        cfg: Arc<Config>,
        state: StateStore,
        email_provider: Arc<dyn EmailProvider + Send + Sync>,
        mining: MiningHandle,
        bus: DashBus,
        shutdown: broadcast::Sender<()>,
    ) -> Self {
        let throttle = GlobalThrottle::new(cfg.throttle.clone());
        Self {
            cfg,
            state,
            email_provider,
            mining,
            bus,
            shutdown,
            throttle,
        }
    }

    /// Drive the system to steady state. Returns when shutdown fires.
    pub async fn run(self) -> FarmResult<()> {
        let Self {
            cfg,
            state,
            email_provider,
            mining,
            bus,
            shutdown,
            throttle,
        } = self;

        // Eagerly spawn account loops for any account already runnable in
        // state.json (resume case). Active accounts mine immediately;
        // Expired accounts enter the loop and refresh in-place. New
        // accounts spawn their own loops from inside replenish_accounts
        // as soon as registration succeeds, so we don't have to wait for
        // the full replenish phase to finish.
        let snapshot = state.snapshot().await;
        let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        // Ramp-up: stagger initial loop spawns so we don't trigger a thundering
        // herd of N concurrent /me + /challenge calls into Cloudflare on
        // restart, which causes upstream 503s and self-induced rate limiting.
        // Each account loop will jitter its own first call too, but giving the
        // process a steady drip-feed here is the most reliable fix.
        let runnable_count = snapshot
            .accounts
            .iter()
            .filter(|r| r.status.should_run_loop())
            .count();
        let ramp_step_ms = if runnable_count > 0 && cfg.loop_pacing.spawn_ramp_window_ms > 0 {
            (cfg.loop_pacing.spawn_ramp_window_ms / runnable_count as u64).max(1)
        } else {
            0
        };
        for record in snapshot.accounts.iter() {
            if record.status.should_run_loop() {
                handles.push(spawn_account_mining_loop(
                    cfg.clone(),
                    state.clone(),
                    mining.clone(),
                    bus.clone(),
                    email_provider.clone(),
                    shutdown.clone(),
                    throttle.clone(),
                    record.email.clone(),
                    /* notify_active = */ false,
                ));
                if ramp_step_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(ramp_step_ms)).await;
                }
            } else {
                bus.log_warn(format!(
                    "resume: skipping {} (status={:?})",
                    record.email, record.status
                ));
            }
        }
        info!(
            count = runnable_count,
            ramp_window_ms = cfg.loop_pacing.spawn_ramp_window_ms,
            "spawned account loops with staggered ramp-up (Active+Expired)"
        );

        // Replenish runs PERIODICALLY in background. Each cycle registers
        // new accounts up to `target_accounts` if short. Subsequent cycles
        // keep filling the target if the first burst was incomplete (e.g.
        // due to transient solver/IMAP errors). As each registration
        // completes, replenish spawns a mining loop for that account
        // immediately rather than waiting for the rest of the batch.
        // Refresh of Expired accounts is handled inside each per-account
        // loop (see run_account_loop), so replenish only registers.
        {
            let cfg = cfg.clone();
            let state = state.clone();
            let email_provider = email_provider.clone();
            let bus = bus.clone();
            let mining = mining.clone();
            let shutdown_tx = shutdown.clone();
            let throttle = throttle.clone();
            let mut shutdown_rx = shutdown.subscribe();
            tokio::spawn(async move {
                let interval_secs = cfg.loop_pacing.replenish_interval_secs.max(60);
                let mut tick = tokio::time::interval(Duration::from_secs(interval_secs));
                // First tick fires immediately by default. If a previous
                // cycle ran long, skip missed ticks instead of bursting.
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tokio::select! {
                        _ = tick.tick() => {
                            match replenish_accounts(
                                &cfg, &state, email_provider.clone(), &bus, &mining, &shutdown_tx, throttle.clone(),
                            ).await {
                                Ok(_) => bus.log_info("replenish cycle complete"),
                                Err(e) => bus.log_error(format!("replenish error: {e}")),
                            }
                        }
                        _ = shutdown_rx.recv() => {
                            info!("replenish: shutdown received, aborting");
                            break;
                        }
                    }
                }
            });
        }

        // Periodic state-flush task: every 30 s force-write to disk so a
        // crash doesn't lose more than 30 s of progress.
        {
            let state = state.clone();
            let mut shutdown_rx = shutdown.subscribe();
            let bus = bus.clone();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(30));
                tick.tick().await; // discard immediate first tick
                loop {
                    tokio::select! {
                        _ = tick.tick() => {
                            if let Err(e) = state.save().await {
                                bus.log_error(format!("state save failed: {e}"));
                            }
                        }
                        _ = shutdown_rx.recv() => {
                            info!("state flusher: shutdown received");
                            break;
                        }
                    }
                }
            });
        }

        // Aggregate stats publisher: every 2 s tally and broadcast.
        {
            let state = state.clone();
            let bus = bus.clone();
            let mut shutdown_rx = shutdown.subscribe();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(2));
                loop {
                    tokio::select! {
                        _ = tick.tick() => {
                            let snap = state.snapshot().await;
                            let mut active = 0;
                            let mut pending = 0;
                            let mut expired = 0;
                            let mut dead = 0;
                            let mut total_mints = 0u64;
                            let mut total_balance = 0u64;
                            for r in &snap.accounts {
                                match r.status {
                                    AccountStatus::Active => active += 1,
                                    AccountStatus::Pending => pending += 1,
                                    AccountStatus::Expired => expired += 1,
                                    AccountStatus::Banned | AccountStatus::Dead => dead += 1,
                                }
                                total_mints += r.minted_total;
                                total_balance += r.balance;
                            }
                            bus.send(DashEvent::Aggregate {
                                active_accounts: active,
                                pending_accounts: pending,
                                expired_accounts: expired,
                                dead_accounts: dead,
                                total_mints,
                                total_balance,
                                timestamp: Utc::now(),
                            });
                        }
                        _ = shutdown_rx.recv() => {
                            info!("aggregate publisher: shutdown received");
                            break;
                        }
                    }
                }
            });
        }

        // Wait for shutdown, then drain.
        let mut shutdown_rx = shutdown.subscribe();
        let _ = shutdown_rx.recv().await;
        info!("orchestrator: shutdown received, draining {} loops", handles.len());

        // Final state save.
        if let Err(e) = state.save().await {
            error!("final state save failed: {e}");
        }
        for h in handles {
            let _ = tokio::time::timeout(Duration::from_secs(3), h).await;
        }
        Ok(())
    }
}

/// Register new accounts to bring total count up to `cfg.target_accounts`.
/// Refresh of Expired accounts is intentionally NOT done here — it is
/// handled inside each per-account loop (see `run_account_loop`), so
/// running this concurrently with active loops does not race on the
/// same magic-link mailbox.
/// Each successful registration eagerly spawns a mining loop so the
/// account starts producing as soon as it is active (no wait for the
/// rest of the batch).
async fn replenish_accounts(
    cfg: &Arc<Config>,
    state: &StateStore,
    email_provider: Arc<dyn EmailProvider + Send + Sync>,
    bus: &DashBus,
    mining: &MiningHandle,
    shutdown: &broadcast::Sender<()>,
    throttle: Arc<GlobalThrottle>,
) -> FarmResult<()> {
    let snap = state.snapshot().await;
    let active = snap
        .accounts
        .iter()
        .filter(|a| a.status == AccountStatus::Active)
        .count();
    let need_to_register = cfg.target_accounts.saturating_sub(snap.accounts.len());
    let expired_count = snap
        .accounts
        .iter()
        .filter(|a| a.status == AccountStatus::Expired)
        .count();

    if need_to_register == 0 && expired_count == 0 {
        // Steady state — nothing to do.
        return Ok(());
    }

    bus.log_info(format!(
        "replenish: have={} active={} target={} need_to_register={} expired={} (refresh handled by per-account loops)",
        snap.accounts.len(),
        active,
        cfg.target_accounts,
        need_to_register,
        expired_count,
    ));

    let sem = Arc::new(Semaphore::new(REGISTER_PARALLELISM));

    let mut tasks = Vec::new();

    for _ in 0..need_to_register {
        let cfg = cfg.clone();
        let state = state.clone();
        let provider = email_provider.clone();
        let bus = bus.clone();
        let permit = sem.clone();
        let mining = mining.clone();
        let shutdown = shutdown.clone();
        let throttle = throttle.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = permit.acquire().await.expect("semaphore");
            stagger_register(&cfg).await;
            match auth::register_new_account(&cfg, &*provider).await {
                Ok(record) => {
                    let email = record.email.clone();
                    state.add_pending(record.clone()).await;
                    state
                        .update(&email, |r| {
                            *r = record;
                        })
                        .await;
                    bus.log_info(format!("registered new account: {email}"));
                    if let Err(e) = state.save().await {
                        bus.log_error(format!("state save after register: {e}"));
                    }
                    // Eagerly spawn the mining loop so this account starts
                    // producing immediately rather than waiting for the rest
                    // of the replenish batch to finish.
                    spawn_account_mining_loop(
                        cfg.clone(),
                        state.clone(),
                        mining.clone(),
                        bus.clone(),
                        provider.clone(),
                        shutdown.clone(),
                        throttle.clone(),
                        email,
                        /* notify_active = */ true,
                    );
                }
                Err(e) => {
                    bus.log_error(format!("register failed: {e}"));
                }
            }
        }));
    }

    for t in tasks {
        let _ = t.await;
    }
    Ok(())
}

/// One account's mining lifecycle. Long-running until shutdown.
async fn run_account_loop(
    cfg: Arc<Config>,
    state: StateStore,
    mining: MiningHandle,
    bus: DashBus,
    email_provider: Arc<dyn EmailProvider + Send + Sync>,
    throttle: Arc<GlobalThrottle>,
    email: String,
    notify_active: bool,
) {
    let record = match state.get(&email).await {
        Some(r) => r,
        None => {
            bus.log_error(format!("account loop start: {email} not in state"));
            return;
        }
    };

    let cookie = record.session_cookie.clone();
    let client = match Rpow2Client::new(
        &cfg.rpow2.api_base,
        &record.user_agent,
        cookie,
        cfg.retry.clone(),
        cfg.rpow2.proxy_for_email(&email),
    ) {
        Ok(c) => c,
        Err(e) => {
            bus.log_error(format!("client build for {email}: {e}"));
            return;
        }
    };

    info!(%email, "account loop started");
    // Only fire the "✅ Active" Telegram notif when this is a fresh
    // activation (newly registered or refreshed). For resume-from-state
    // (bot restart), suppress to avoid spamming N copies on every restart.
    if notify_active {
        bus.send(DashEvent::AccountStateChanged {
            email: email.clone(),
            status: AccountStatus::Active,
            balance: record.balance,
            minted_total: record.minted_total,
            last_error: None,
            timestamp: Utc::now(),
        });
    }

    loop {
        // Reload current record (status may have been mutated externally).
        let mut record = match state.get(&email).await {
            Some(r) => r,
            None => return,
        };
        if record.status == AccountStatus::Dead || record.status == AccountStatus::Banned {
            // Sleep & continue — don't burn CPU on graveyard accounts.
            tokio::time::sleep(Duration::from_secs(60)).await;
            continue;
        }

        if record.status == AccountStatus::Expired {
            // Try to refresh in-place.
            match auth::refresh_existing_account(&cfg, email_provider.clone(), &mut record).await {
                Ok(_) => {
                    let new_cookie = record.session_cookie.clone();
                    state.update(&email, |r| *r = record.clone()).await;
                    let _ = state.save().await;
                    client.set_session_cookie(new_cookie);
                    bus.log_info(format!("session refreshed for {email}"));
                }
                Err(e) => {
                    bus.log_warn(format!("refresh {email} failed: {e}; sleeping 5min"));
                    tokio::time::sleep(Duration::from_secs(300)).await;
                    continue;
                }
            }
        }

        // Step 1: get challenge.
        // Throttle gates: (a) waits if a global 5xx-wave pause is active,
        // (b) acquires a token from the /challenge rate bucket so we
        // stay under nginx's 2 rps per-IP limit. Across all 200 account
        // loops this serialises requests at ~challenge_rps.
        throttle.acquire_challenge_slot().await;
        let chall = match client.challenge().await {
            Ok(c) => {
                throttle.report_success().await;
                c
            }
            Err(e) => {
                if e.is_server_overloaded() {
                    throttle.report_overload().await;
                }
                handle_account_error(&state, &bus, &email, &e).await;
                if e.is_account_dead() {
                    continue; // will hit Expired branch on next iter
                }
                let sleep_ms = if matches!(e, FarmError::RateLimited { .. })
                    || e.is_server_overloaded()
                {
                    cfg.loop_pacing.sleep_on_rate_limit_ms
                } else {
                    cfg.loop_pacing.sleep_on_error_ms
                };
                sleep_ms_jittered(sleep_ms, cfg.loop_pacing.sleep_jitter_ms).await;
                continue;
            }
        };

        // Surface current server-side difficulty for tracking / alerting.
        bus.send(DashEvent::DifficultyObserved {
            difficulty_bits: chall.difficulty_bits,
            email: email.clone(),
            timestamp: Utc::now(),
        });

        // Step 2: convert prefix hex → bytes.
        let prefix_bytes = match hex_to_bytes(&chall.nonce_prefix) {
            Some(b) => b,
            None => {
                bus.log_error(format!(
                    "{email}: invalid nonce_prefix hex from server: {}",
                    chall.nonce_prefix
                ));
                sleep_ms_jittered(cfg.loop_pacing.sleep_on_error_ms, 0).await;
                continue;
            }
        };

        // Step 3: submit to mining pool.
        let cancel = Arc::new(AtomicBool::new(false));
        let (reply_tx, reply_rx) = oneshot::channel();
        let job = MiningJob {
            challenge_id: chall.challenge_id.clone(),
            prefix: prefix_bytes,
            difficulty: chall.difficulty_bits,
            reply: reply_tx,
            cancel: cancel.clone(),
        };
        if let Err(e) = mining.submit(job).await {
            bus.log_error(format!("{email}: mining queue closed: {e}"));
            return;
        }

        // Step 4: await result with timeout.
        let outcome = match tokio::time::timeout(
            Duration::from_millis(cfg.loop_pacing.mining_timeout_ms),
            reply_rx,
        ).await {
            Ok(Ok(o)) => o,
            Ok(Err(_)) => {
                // Worker dropped without sending — treat as transient.
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                sleep_ms_jittered(cfg.loop_pacing.sleep_on_error_ms, 0).await;
                continue;
            }
            Err(_) => {
                // Mining timeout — cancel job and request new challenge.
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                bus.log_warn(format!(
                    "{email}: mining timeout after {}ms, requesting new challenge",
                    cfg.loop_pacing.mining_timeout_ms
                ));
                continue;
            }
        };

        // Step 5: submit mint.
        // Throttle gates: same idea as /challenge but using the /mint
        // bucket — this is the tighter limit (Fastify in-app limiter
        // ~0.67 rps with 4 workers, vs nginx 2 rps).
        let nonce_str = outcome.nonce.to_string();
        throttle.acquire_mint_slot().await;
        let mint_resp = match client.mint(&chall.challenge_id, &nonce_str).await {
            Ok(r) => {
                throttle.report_success().await;
                r
            }
            Err(e) => {
                if e.is_server_overloaded() {
                    throttle.report_overload().await;
                }
                handle_account_error(&state, &bus, &email, &e).await;
                let sleep_ms = if matches!(e, FarmError::RateLimited { .. })
                    || e.is_server_overloaded()
                {
                    cfg.loop_pacing.sleep_on_rate_limit_ms
                } else {
                    cfg.loop_pacing.sleep_on_error_ms
                };
                sleep_ms_jittered(sleep_ms, cfg.loop_pacing.sleep_jitter_ms).await;
                continue;
            }
        };

        // Step 6: refresh balance via /me.
        // All amounts are now base units (1 RPOW = 1_000_000_000). Mint
        // reward is whatever the server quoted in the /mint response and
        // can be fractional (e.g. 0.001 RPOW = 1_000_000 base units).
        let me = client.me().await.ok();
        let mint_value = mint_resp.token.value();
        let new_balance = me
            .as_ref()
            .map(|m| m.balance())
            .unwrap_or(record.balance.saturating_add(mint_value));
        let new_minted = me
            .as_ref()
            .map(|m| m.minted())
            .unwrap_or(record.minted_total.saturating_add(mint_value));
        state
            .update(&email, |r| {
                r.balance = new_balance;
                r.minted_total = new_minted;
                r.last_seen = Some(Utc::now());
                r.last_error = None;
            })
            .await;

        bus.send(DashEvent::MintCompleted {
            email: email.clone(),
            nonce: outcome.nonce,
            difficulty_bits: chall.difficulty_bits,
            elapsed_ms: outcome.elapsed_ms,
            balance_after: new_balance,
            minted_total: new_minted,
            timestamp: Utc::now(),
        });
        info!(
            %email,
            nonce = outcome.nonce,
            difficulty = chall.difficulty_bits,
            elapsed_ms = outcome.elapsed_ms as u64,
            mint_value_bu = mint_value,
            balance_bu = new_balance,
            minted_bu = new_minted,
            balance = %crate::api::types::format_rpow(new_balance),
            "mint ok"
        );
        let _ = mint_resp; // we keep token id only in logs

        // Step 7: maybe auto-pool.
        if cfg.auto_pool.enabled {
            if let Some(rec) = state.get(&email).await {
                if let Err(e) = pool::maybe_pool(&cfg.auto_pool, &client, &state, &rec, &bus).await {
                    bus.log_warn(format!("{email}: auto-pool failed: {e}"));
                }
            }
        }

        // Step 8: pacing.
        sleep_ms_jittered(
            cfg.loop_pacing.sleep_after_mint_ms,
            cfg.loop_pacing.sleep_jitter_ms,
        )
        .await;
    }
}

async fn handle_account_error(
    state: &StateStore,
    bus: &DashBus,
    email: &str,
    e: &FarmError,
) {
    let new_status = if e.is_account_dead() {
        Some(AccountStatus::Expired)
    } else if matches!(e, FarmError::Api { status: 451, .. }) {
        Some(AccountStatus::Banned)
    } else {
        None
    };

    let msg = e.to_string();
    // Surface to journald at appropriate severity so a human watching
    // logs can spot upstream issues. The dashboard log buffer is also
    // updated below via bus.send for terminal status changes.
    if e.is_server_overloaded() {
        warn!(%email, error = %msg, "upstream overloaded; backing off");
    } else if matches!(e, FarmError::RateLimited { .. }) {
        warn!(%email, error = %msg, "rate-limited; backing off");
    } else if new_status.is_some() {
        warn!(%email, error = %msg, "account terminal error");
    } else {
        // Other transient error — trace at debug to avoid spam.
        tracing::debug!(%email, error = %msg, "transient account error");
    }
    state
        .update(email, |r| {
            if let Some(s) = new_status {
                r.status = s;
            }
            r.last_error = Some(msg.clone());
            r.last_seen = Some(Utc::now());
        })
        .await;
    // For recoverable errors (new_status = None) skip publishing
    // AccountStateChanged — it would re-broadcast `Active` to the
    // notifier which then sends a misleading "✅ Active balance: 0"
    // Telegram message even though the account still has its real
    // balance. Only surface terminal status changes.
    if let Some(status) = new_status {
        let (balance, minted_total) = state
            .get(email)
            .await
            .map(|r| (r.balance, r.minted_total))
            .unwrap_or((0, 0));
        bus.send(DashEvent::AccountStateChanged {
            email: email.to_string(),
            status,
            balance,
            minted_total,
            last_error: Some(msg),
            timestamp: Utc::now(),
        });
    }
}

async fn sleep_ms_jittered(base_ms: u64, jitter_ms: u64) {
    let extra = if jitter_ms == 0 {
        0
    } else {
        rand::thread_rng().gen_range(0..=jitter_ms)
    };
    tokio::time::sleep(Duration::from_millis(base_ms + extra)).await;
}

/// Spawn a long-running mining loop for one account. Listens for shutdown
/// via a broadcast subscription so the task exits cleanly when the farm
/// stops. Used both at boot (resume from state.json) and from inside
/// `replenish_accounts` (when a brand-new or refreshed account becomes
/// active mid-replenish).
fn spawn_account_mining_loop(
    cfg: Arc<Config>,
    state: StateStore,
    mining: MiningHandle,
    bus: DashBus,
    email_provider: Arc<dyn EmailProvider + Send + Sync>,
    shutdown: broadcast::Sender<()>,
    throttle: Arc<GlobalThrottle>,
    email: String,
    notify_active: bool,
) -> tokio::task::JoinHandle<()> {
    let mut shutdown_rx = shutdown.subscribe();
    let email_for_log = email.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = run_account_loop(cfg, state, mining, bus, email_provider, throttle, email, notify_active) => {}
            _ = shutdown_rx.recv() => {
                info!(email = %email_for_log, "account loop received shutdown signal");
            }
        }
    })
}

/// Spread out new-account registrations to look more organic to anti-fraud
/// systems. Called after a registration permit is acquired but before the
/// actual auth/request POST is sent.
async fn stagger_register(cfg: &Config) {
    let min = cfg.loop_pacing.register_delay_min_secs;
    let max = cfg.loop_pacing.register_delay_max_secs.max(min);
    if max == 0 {
        return;
    }
    let secs = if min == max {
        min
    } else {
        rand::thread_rng().gen_range(min..=max)
    };
    tokio::time::sleep(Duration::from_secs(secs)).await;
}
