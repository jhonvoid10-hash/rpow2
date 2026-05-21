//! Configuration loader: parse `config.toml` into typed structs.
//!
//! Layout mirrors `config.example.toml` 1:1. The `Config` struct is the
//! single source of truth passed around the orchestrator.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::{FarmError, FarmResult};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_target_accounts")]
    pub target_accounts: usize,
    #[serde(default = "default_state_path")]
    pub state_path: String,
    #[serde(default = "default_mining_workers")]
    pub mining_workers: usize,

    #[serde(default)]
    pub loop_pacing: LoopPacing,
    #[serde(default)]
    pub retry: RetryConfig,
    pub email: EmailConfig,
    #[serde(default)]
    pub auto_pool: AutoPool,
    pub rpow2: Rpow2Config,
    #[serde(default)]
    pub dashboard: DashboardConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub panel: PanelConfig,
    #[serde(default)]
    pub turnstile: TurnstileConfig,
    #[serde(default)]
    pub throttle: crate::throttle::ThrottleConfig,
}

fn default_target_accounts() -> usize {
    50
}
fn default_state_path() -> String {
    "state.json".into()
}
fn default_mining_workers() -> usize {
    0 // resolved later via num_cpus
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoopPacing {
    pub sleep_after_mint_ms: u64,
    pub sleep_jitter_ms: u64,
    pub sleep_on_error_ms: u64,
    pub sleep_on_rate_limit_ms: u64,
    /// Timeout for mining job in ms. If exceeded, cancel and request new challenge.
    /// Default 30000 = 30 seconds.
    #[serde(default = "default_mining_timeout_ms")]
    pub mining_timeout_ms: u64,
    /// Random delay (min..=max secs) inserted BEFORE each new account
    /// registration starts. Spreads out auth/request POSTs so 100+
    /// signups don't burst in seconds (looks more organic; reduces
    /// risk of anti-fraud rate-limiting on the rpow2 side).
    /// Set both to 0 to disable.
    #[serde(default = "default_register_delay_min")]
    pub register_delay_min_secs: u64,
    #[serde(default = "default_register_delay_max")]
    pub register_delay_max_secs: u64,
    /// Total wall-clock window over which to ramp-up the spawn of resume
    /// account loops on startup. Spreading 100+ accounts over e.g. 30s
    /// avoids slamming Cloudflare with concurrent /me + /challenge calls.
    #[serde(default = "default_spawn_ramp_window_ms")]
    pub spawn_ramp_window_ms: u64,
    /// Interval (seconds) between periodic replenish cycles. Each cycle
    /// registers new accounts up to `target_accounts` if short. Set to
    /// 0 / very low to disable. Default 600 = 10 minutes.
    #[serde(default = "default_replenish_interval_secs")]
    pub replenish_interval_secs: u64,
}

fn default_register_delay_min() -> u64 {
    5
}
fn default_mining_timeout_ms() -> u64 {
    30_000
}
fn default_register_delay_max() -> u64 {
    30
}
fn default_spawn_ramp_window_ms() -> u64 {
    30_000
}
fn default_replenish_interval_secs() -> u64 {
    600
}

impl Default for LoopPacing {
    fn default() -> Self {
        Self {
            sleep_after_mint_ms: 500,
            sleep_jitter_ms: 250,
            sleep_on_error_ms: 5000,
            sleep_on_rate_limit_ms: 30000,
            mining_timeout_ms: default_mining_timeout_ms(),
            register_delay_min_secs: default_register_delay_min(),
            register_delay_max_secs: default_register_delay_max(),
            spawn_ramp_window_ms: default_spawn_ramp_window_ms(),
            replenish_interval_secs: default_replenish_interval_secs(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub backoff_jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_backoff_ms: 1000,
            max_backoff_ms: 30000,
            backoff_multiplier: 2.0,
            backoff_jitter: 0.3,
        }
    }
}

// ---- email ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmailProviderKind {
    CatchallImap,
    Tempmail,
    GmailAlias,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmailConfig {
    pub provider: EmailProviderKind,
    #[serde(default)]
    pub catchall_imap: Option<CatchallImapConfig>,
    #[serde(default)]
    pub tempmail: Option<TempmailConfig>,
    #[serde(default)]
    pub gmail_alias: Option<GmailAliasConfig>,
    /// Periodic IMAP health probe — catches revoked App Passwords / expired
    /// tokens early instead of letting every refresh silently hang.
    #[serde(default)]
    pub health_check: ImapHealthCheckConfig,
    /// Hard cap on the number of *concurrent* IMAP login sessions across
    /// the whole bot. Each per-account poll holds one permit for the
    /// duration of its TLS handshake + login + search + fetch (~200-500ms).
    /// 1 = strictly serial polls (safest for Gmail's anomaly detector).
    /// Bump cautiously if you have a self-hosted IMAP server that can
    /// take more parallelism.
    #[serde(default = "default_imap_max_concurrent_logins")]
    pub imap_max_concurrent_logins: usize,
}

fn default_imap_max_concurrent_logins() -> usize {
    1
}

/// Background IMAP login probe. When `enabled`, a task wakes up every
/// `interval_secs`, performs a tiny connect+login+select+logout, and (if
/// configured) sends a Telegram alert on transition Ok→Broken or after
/// `alert_cooldown_secs` while still broken. Recovery (Broken→Ok)
/// also fires a one-shot recovery alert.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImapHealthCheckConfig {
    #[serde(default = "default_imap_health_enabled")]
    pub enabled: bool,
    #[serde(default = "default_imap_health_interval")]
    pub interval_secs: u64,
    #[serde(default = "default_imap_health_cooldown")]
    pub alert_cooldown_secs: u64,
}

fn default_imap_health_enabled() -> bool {
    true
}
fn default_imap_health_interval() -> u64 {
    600 // 10 min
}
fn default_imap_health_cooldown() -> u64 {
    1800 // 30 min between repeat alerts while still broken
}

impl Default for ImapHealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: default_imap_health_enabled(),
            interval_secs: default_imap_health_interval(),
            alert_cooldown_secs: default_imap_health_cooldown(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatchallImapConfig {
    pub domain: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_username: String,
    pub imap_password: String,
    #[serde(default = "default_mailbox")]
    pub mailbox: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_email_wait")]
    pub max_wait_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TempmailConfig {
    #[serde(default = "default_tempmail_base")]
    pub api_base: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_email_wait")]
    pub max_wait_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmailAliasConfig {
    pub base_email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_username: String,
    pub imap_password: String,
    #[serde(default = "default_mailbox")]
    pub mailbox: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_email_wait")]
    pub max_wait_secs: u64,
}

fn default_mailbox() -> String {
    "INBOX".into()
}
fn default_poll_interval() -> u64 {
    5000
}
fn default_email_wait() -> u64 {
    120
}
fn default_tempmail_base() -> String {
    "https://api.mail.tm".into()
}

// ---- auto-pool -----------------------------------------------------------

/// Strategy for distributing token sends across multiple master wallets.
///
/// | Value            | Behaviour                                                   |
/// |------------------|-------------------------------------------------------------|
/// | `"random"`       | Pick one wallet uniformly at random each send.              |
/// | `"round-robin"`  | Strict rotation: A→B→C→A→B→C… (global atomic counter).     |
/// | `"shuffled"`     | Full rotation per cycle, order shuffled each new cycle.     |
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PoolStrategy {
    /// Pick uniformly at random every send (default).
    #[default]
    Random,
    /// Strict round-robin: A→B→C→A→B→C…
    RoundRobin,
    /// Rotate through a shuffled order, re-shuffle each full cycle.
    Shuffled,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutoPool {
    #[serde(default)]
    pub enabled: bool,
    /// Single master (backward compat). Ignored if master_emails is non-empty.
    #[serde(default)]
    pub master_email: String,
    /// Multiple master wallets. Strategy determines how one is chosen per send.
    #[serde(default)]
    pub master_emails: Vec<String>,
    /// How to pick a master wallet when multiple are configured.
    /// Options: "random" (default) | "round-robin" | "shuffled"
    #[serde(default)]
    pub strategy: PoolStrategy,
    #[serde(default = "default_threshold")]
    pub threshold: u64,
    #[serde(default = "default_keep_balance")]
    pub keep_balance: u64,
    #[serde(default = "default_send_jitter")]
    pub send_jitter_secs: u64,
    #[serde(default)]
    pub send_after_n_mints: u32,
}

impl AutoPool {
    /// Returns the full list of master emails.
    /// Merges master_emails + master_email (backward compat, deduped).
    pub fn effective_masters(&self) -> Vec<String> {
        let mut list = self.master_emails.clone();
        if !self.master_email.is_empty() && !list.contains(&self.master_email) {
            list.push(self.master_email.clone());
        }
        list
    }

    /// Pick a master email using `strategy = "random"`.
    pub fn pick_master(&self) -> Option<String> {
        use rand::seq::SliceRandom;
        let list = self.effective_masters();
        list.choose(&mut rand::thread_rng()).cloned()
    }
}

fn default_threshold() -> u64 {
    50
}
fn default_keep_balance() -> u64 {
    0
}
fn default_send_jitter() -> u64 {
    30
}

impl Default for AutoPool {
    fn default() -> Self {
        Self {
            enabled: false,
            master_email: String::new(),
            master_emails: Vec::new(),
            strategy: PoolStrategy::default(),
            threshold: default_threshold(),
            keep_balance: default_keep_balance(),
            send_jitter_secs: default_send_jitter(),
            send_after_n_mints: 0,
        }
    }
}

// ---- rpow2 ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rpow2Config {
    #[serde(default = "default_api_base")]
    pub api_base: String,
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    #[serde(default)]
    pub user_agent_pool: Vec<String>,
    /// Optional HTTP/SOCKS5 proxy URL, e.g. "http://user:pass@host:port".
    /// Used when `proxy_pool` is empty.
    #[serde(default)]
    pub proxy: Option<String>,
    /// Pool of proxy URLs. When non-empty, each account is assigned a proxy
    /// deterministically by hashing its email address. Overrides `proxy`.
    #[serde(default)]
    pub proxy_pool: Vec<String>,
}

impl Rpow2Config {
    /// Return the proxy URL to use for a given account email.
    ///
    /// If `proxy_pool` is non-empty, assigns a proxy deterministically based
    /// on a simple hash of the email so the same account always uses the same
    /// proxy (stable across restarts). Falls back to the single `proxy` field
    /// when the pool is empty.
    pub fn proxy_for_email(&self, email: &str) -> Option<&str> {
        if !self.proxy_pool.is_empty() {
            let hash = email
                .bytes()
                .fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
            Some(self.proxy_pool[hash % self.proxy_pool.len()].as_str())
        } else {
            self.proxy.as_deref()
        }
    }
}

fn default_api_base() -> String {
    "https://api.rpow2.com".into()
}
fn default_user_agent() -> String {
    "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0".into()
}

// ---- dashboard / logging -------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DashboardConfig {
    #[serde(default = "default_dash_enabled")]
    pub enabled: bool,
    #[serde(default = "default_refresh_rate")]
    pub refresh_rate_ms: u64,
}

fn default_dash_enabled() -> bool {
    true
}
fn default_refresh_rate() -> u64 {
    250
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            refresh_rate_ms: 250,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_file")]
    pub file: String,
    #[serde(default)]
    pub json: bool,
}

fn default_log_level() -> String {
    "info".into()
}
fn default_log_file() -> String {
    "logs/farm.log".into()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: default_log_file(),
            json: false,
        }
    }
}

// ---- notifications -------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub telegram: TelegramConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub chat_id: String,
    #[serde(default = "default_true")]
    pub notify_farm_lifecycle: bool,
    #[serde(default = "default_true")]
    pub notify_account_registered: bool,
    #[serde(default = "default_true")]
    pub notify_account_dead: bool,
    #[serde(default = "default_true")]
    pub notify_per_pool: bool,
    #[serde(default)]
    pub notify_per_mint: bool,
    #[serde(default = "default_true")]
    pub hourly_summary: bool,
    #[serde(default = "default_summary_interval")]
    pub summary_interval_secs: u64,
    /// Send a Telegram alert when observed challenge difficulty drops to
    /// `difficulty_alert_threshold` bits or lower (compared to a previous
    /// higher reading). Useful for catching off-peak windows where rpow2
    /// temporarily lowers global difficulty.
    #[serde(default = "default_true")]
    pub notify_difficulty_drop: bool,
    #[serde(default = "default_difficulty_threshold")]
    pub difficulty_alert_threshold: u32,
    /// Minimum seconds between difficulty drop alerts (dedup). Default 600s.
    #[serde(default = "default_difficulty_alert_cooldown")]
    pub difficulty_alert_cooldown_secs: u64,
    /// Minimum gap (ms) between two Telegram sends. Telegram throttles
    /// bots at ~1 msg/sec/chat sustained; bursts above ~20 msg/few-sec
    /// trigger long 429 cooldowns (1500-1700s). Default 1100ms = ~0.9
    /// msg/sec, safe under the limit.
    #[serde(default = "default_min_send_interval_ms")]
    pub min_send_interval_ms: u64,
    /// Skip sending a Telegram message whose body is byte-identical to
    /// one sent within the last `dedup_window_secs` seconds. Catches
    /// e.g. repeated identical "Active" notifications during recovery
    /// bursts. Set to 0 to disable.
    #[serde(default = "default_dedup_window_secs")]
    pub dedup_window_secs: u64,
}

fn default_true() -> bool {
    true
}
fn default_summary_interval() -> u64 {
    3600
}
fn default_difficulty_threshold() -> u32 {
    30
}
fn default_difficulty_alert_cooldown() -> u64 {
    600
}
fn default_min_send_interval_ms() -> u64 {
    1100
}
fn default_dedup_window_secs() -> u64 {
    60
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bot_token: String::new(),
            chat_id: String::new(),
            notify_farm_lifecycle: true,
            notify_account_registered: true,
            notify_account_dead: true,
            notify_per_pool: true,
            notify_per_mint: false,
            hourly_summary: true,
            summary_interval_secs: 3600,
            notify_difficulty_drop: true,
            difficulty_alert_threshold: default_difficulty_threshold(),
            difficulty_alert_cooldown_secs: default_difficulty_alert_cooldown(),
            min_send_interval_ms: default_min_send_interval_ms(),
            dedup_window_secs: default_dedup_window_secs(),
        }
    }
}

// ---- panel ---------------------------------------------------------------

/// HTTP monitoring panel config. The panel is a small embedded web server
/// that exposes JSON endpoints + a single-page HTML dashboard. Bind to
/// localhost and put nginx with TLS / basic-auth in front for public
/// exposure.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PanelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_panel_bind")]
    pub bind: String,
    #[serde(default = "default_panel_port")]
    pub port: u16,
    /// Optional bearer token. If non-empty, requests must send it as
    /// `Authorization: Bearer <token>` OR `?token=<token>`. Leave empty
    /// when binding to localhost behind an authenticated reverse proxy.
    #[serde(default)]
    pub auth_token: String,
}

fn default_panel_bind() -> String {
    "127.0.0.1".to_string()
}
fn default_panel_port() -> u16 {
    7878
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: default_panel_bind(),
            port: default_panel_port(),
            auth_token: String::new(),
        }
    }
}

// ---- turnstile -----------------------------------------------------------

/// External Cloudflare Turnstile solver sidecar config. The bot calls
/// `POST {endpoint_url}/solve` with `{sitekey, url, action, timeout_ms}`
/// and expects `{token, solved_in_ms}` back. See
/// `turnstile-solver/solver.py` for a reference implementation that
/// uses Camoufox (modified Firefox with a real-browser fingerprint).
///
/// Recommended deployment: run the solver on a residential / home IP
/// and reverse-tunnel to the VPS:
///   ssh -R 9191:127.0.0.1:9191 farm@<vps-ip>
/// Then set `endpoint_url = "http://127.0.0.1:9191"` in this config.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TurnstileConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_turnstile_endpoint")]
    pub endpoint_url: String,
    #[serde(default = "default_turnstile_sitekey")]
    pub sitekey: String,
    #[serde(default = "default_turnstile_page_url")]
    pub page_url: String,
    /// Total wall-clock budget for one solve (used as the body
    /// `timeout_ms` and roughly as `client_timeout_ms` upper bound).
    #[serde(default = "default_turnstile_solve_timeout_ms")]
    pub solve_timeout_ms: u64,
    /// HTTP client timeout for the solve POST. Should be ≥
    /// `solve_timeout_ms` plus a few seconds of headroom for the
    /// browser warm-up + token serialisation.
    #[serde(default = "default_turnstile_client_timeout_ms")]
    pub client_timeout_ms: u64,
}

fn default_turnstile_endpoint() -> String {
    "http://127.0.0.1:9191".to_string()
}
fn default_turnstile_sitekey() -> String {
    "0x4AAAAAADLyZ9ztTUV1Pm1F".to_string()
}
fn default_turnstile_page_url() -> String {
    "https://rpow2.com".to_string()
}
fn default_turnstile_solve_timeout_ms() -> u64 {
    60_000
}
fn default_turnstile_client_timeout_ms() -> u64 {
    90_000
}

impl Default for TurnstileConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint_url: default_turnstile_endpoint(),
            sitekey: default_turnstile_sitekey(),
            page_url: default_turnstile_page_url(),
            solve_timeout_ms: default_turnstile_solve_timeout_ms(),
            client_timeout_ms: default_turnstile_client_timeout_ms(),
        }
    }
}

// -- loader -----------------------------------------------------------------

impl Config {
    pub fn load(path: &Path) -> FarmResult<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| FarmError::Config(format!("cannot read {}: {e}", path.display())))?;
        let mut cfg: Config = toml::from_str(&raw)?;
        cfg.normalise()?;
        Ok(cfg)
    }

    pub fn normalise(&mut self) -> FarmResult<()> {
        if self.mining_workers == 0 {
            let cores = num_cpus::get();
            self.mining_workers = cores.saturating_sub(1).max(1);
        }
        if self.target_accounts == 0 {
            return Err(FarmError::Config("target_accounts must be > 0".into()));
        }
        // Sanity-check the chosen email provider has its config block present
        match self.email.provider {
            EmailProviderKind::CatchallImap if self.email.catchall_imap.is_none() => {
                return Err(FarmError::Config(
                    "[email.catchall_imap] block missing for provider=catchall_imap".into(),
                ));
            }
            EmailProviderKind::Tempmail if self.email.tempmail.is_none() => {
                // tempmail is the only provider that can run with all defaults
                self.email.tempmail = Some(TempmailConfig {
                    api_base: default_tempmail_base(),
                    poll_interval_ms: default_poll_interval(),
                    max_wait_secs: default_email_wait(),
                });
            }
            EmailProviderKind::GmailAlias if self.email.gmail_alias.is_none() => {
                return Err(FarmError::Config(
                    "[email.gmail_alias] block missing for provider=gmail_alias".into(),
                ));
            }
            _ => {}
        }
        if self.auto_pool.enabled && self.auto_pool.effective_masters().is_empty() {
            return Err(FarmError::Config(
                "auto_pool.enabled is true but no master_email(s) configured".into(),
            ));
        }
        Ok(())
    }
}
