"""
Auto Cookie Collector untuk RPOW2-Farm
=======================================
Cara pakai:
1. pip install selenium webdriver-manager
2. python auto_collector.py
3. Browser akan terbuka, login manual di rpow2.com
4. Setelah login, cookie otomatis tersimpan
5. Browser tutup, tanya mau akun berikutnya atau tidak

Requirements:
    pip install selenium webdriver-manager
"""

import json
import os
import time
from datetime import datetime, timezone

from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from webdriver_manager.chrome import ChromeDriverManager

STATE_FILE = "state.json"
RPOW2_URL = "https://rpow2.com"


def load_state():
    """Load state.json yang sudah ada"""
    if os.path.exists(STATE_FILE):
        with open(STATE_FILE, "r") as f:
            return json.load(f)
    return {"version": 1, "accounts": []}


def save_state(state):
    """Simpan state.json"""
    with open(STATE_FILE, "w") as f:
        json.dump(state, f, indent=2)
    print(f"\n✅ Tersimpan ke {STATE_FILE}")


def get_email_from_cookie(cookies):
    """Ambil email dari cookie rpow_session"""
    for cookie in cookies:
        if cookie["name"] == "rpow_session":
            try:
                import base64
                token = cookie["value"].split(".")[0]
                # Tambah padding jika perlu
                padding = 4 - len(token) % 4
                if padding != 4:
                    token += "=" * padding
                decoded = base64.urlsafe_b64decode(token)
                data = json.loads(decoded)
                return data.get("email", "")
            except Exception:
                pass
    return ""


def collect_account(index):
    """Buka browser, tunggu login, ambil cookie"""
    print(f"\n{'='*50}")
    print(f"  Akun #{index} — Silahkan login di browser!")
    print(f"{'='*50}")

    # Setup Chrome
    options = Options()
    options.add_argument("--start-maximized")
    options.add_argument("--disable-blink-features=AutomationControlled")
    options.add_experimental_option("excludeSwitches", ["enable-automation"])
    options.add_experimental_option("useAutomationExtension", False)

    driver = webdriver.Chrome(
        service=Service(ChromeDriverManager().install()),
        options=options
    )

    # Buka RPOW2
    driver.get(RPOW2_URL)
    print(f"\n🌐 Browser terbuka di {RPOW2_URL}")
    print("📧 Masukkan email kamu dan klik magic link")
    print("⏳ Menunggu kamu login...")

    # Tunggu sampai dapat cookie rpow_session
    session_cookie = None
    user_agent = driver.execute_script("return navigator.userAgent")
    timeout = 300  # 5 menit
    start = time.time()

    while time.time() - start < timeout:
        cookies = driver.get_cookies()
        for cookie in cookies:
            if cookie["name"] == "rpow_session":
                session_cookie = cookie["value"]
                break
        if session_cookie:
            break
        time.sleep(2)

    if not session_cookie:
        print("❌ Timeout! Cookie tidak ditemukan dalam 5 menit.")
        driver.quit()
        return None

    # Ambil email dari cookie
    all_cookies = driver.get_cookies()
    email = get_email_from_cookie(all_cookies)

    if not email:
        email = input("📧 Masukkan email yang kamu gunakan: ").strip()

    print(f"\n✅ Login berhasil!")
    print(f"   Email  : {email}")
    print(f"   Cookie : {session_cookie[:30]}...")

    # Tutup browser
    driver.quit()
    print("🔒 Browser ditutup!")

    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    return {
        "email": email,
        "user_agent": user_agent,
        "session_cookie": f"rpow_session={session_cookie}",
        "session_acquired_at": now,
        "balance": 0,
        "minted_total": 0,
        "sent_total": 0,
        "received_total": 0,
        "status": "active",
        "last_error": None,
        "last_seen": now,
        "created_at": now
    }


def main():
    print("=" * 50)
    print("  RPOW2 Auto Cookie Collector")
    print("=" * 50)

    # Load state yang sudah ada
    state = load_state()
    existing_emails = [a["email"] for a in state["accounts"]]

    if existing_emails:
        print(f"\n📋 Akun yang sudah ada: {len(existing_emails)}")
        for email in existing_emails:
            print(f"   - {email}")

    index = len(state["accounts"]) + 1

    while True:
        account = collect_account(index)

        if account:
            # Cek duplikat
            if account["email"] in existing_emails:
                print(f"⚠️  Email {account['email']} sudah ada! Overwrite? (y/n): ", end="")
                if input().strip().lower() == "y":
                    # Update yang sudah ada
                    for i, a in enumerate(state["accounts"]):
                        if a["email"] == account["email"]:
                            state["accounts"][i] = account
                            break
                    print(f"✅ Akun {account['email']} diupdate!")
            else:
                state["accounts"].append(account)
                existing_emails.append(account["email"])
                print(f"✅ Akun {account['email']} ditambahkan!")

            save_state(state)
            print(f"\n📊 Total akun: {len(state['accounts'])}")

        # Tanya lanjut atau tidak
        print(f"\n{'='*50}")
        print("Mau tambah akun lagi? (y/n): ", end="")
        jawab = input().strip().lower()

        if jawab != "y":
            break

        index += 1
        print("\n⏳ Bersiap buka browser baru dalam 2 detik...")
        time.sleep(2)

    print(f"\n{'='*50}")
    print(f"✅ Selesai! Total {len(state['accounts'])} akun tersimpan di {STATE_FILE}")
    print(f"{'='*50}")

    # Tampilkan semua akun
    print("\n📋 Daftar akun:")
    for i, acc in enumerate(state["accounts"], 1):
        print(f"  {i}. {acc['email']}")


if __name__ == "__main__":
    main()
