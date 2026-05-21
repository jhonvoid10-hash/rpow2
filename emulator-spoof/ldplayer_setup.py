"""
LDPlayer Auto-Setup - Auto konfigurasi semua instance supaya terlihat seperti HP asli
======================================================================================
Cara pakai:
1. Install LDPlayer 9 dulu
2. Jalankan: python ldplayer_setup.py
3. Script akan auto-setup semua instance

Requirements:
    pip install psutil
"""

import json
import os
import random
import string
import subprocess
import time
import glob

# =============== CONFIG ===============
LDPLAYER_PATH = r"C:\LDPlayer\LDPlayer9"
LDCONSOLE = os.path.join(LDPLAYER_PATH, "ldconsole.exe")
NUM_INSTANCES = 20  # Ganti sesuai kebutuhan

# =============== DATABASE HP ASLI ===============
REAL_DEVICES = [
    {
        "brand": "samsung",
        "model": "SM-G991B",
        "manufacturer": "samsung",
        "device": "o1s",
        "fingerprint": "samsung/o1sxeea/o1s:13/TP1A.220624.014/G991BXXS7DWAA:user/release-keys",
        "resolution": "1080,2340",
        "dpi": "420",
    },
    {
        "brand": "samsung",
        "model": "SM-A546B",
        "manufacturer": "samsung",
        "device": "a54x",
        "fingerprint": "samsung/a54xeea/a54x:14/UP1A.231005.007/A546BXXS5CXA1:user/release-keys",
        "resolution": "1080,2340",
        "dpi": "420",
    },
    {
        "brand": "Xiaomi",
        "model": "23049RAD8C",
        "manufacturer": "Xiaomi",
        "device": "earth",
        "fingerprint": "Xiaomi/earth_global/earth:14/UP1A.230905.011/V816.0.2.0.UMFMIXM:user/release-keys",
        "resolution": "1080,2400",
        "dpi": "440",
    },
    {
        "brand": "Xiaomi",
        "model": "22071212AG",
        "manufacturer": "Xiaomi",
        "device": "apollon",
        "fingerprint": "Xiaomi/apollon_global/apollon:13/TP1A.220624.014/V14.0.6.0.TGKMIXM:user/release-keys",
        "resolution": "1080,2400",
        "dpi": "440",
    },
    {
        "brand": "OPPO",
        "model": "CPH2387",
        "manufacturer": "OPPO",
        "device": "OP5154L1",
        "fingerprint": "OPPO/CPH2387/OP5154L1:13/TP1A.220905.001/S.12345678:user/release-keys",
        "resolution": "1080,2400",
        "dpi": "420",
    },
    {
        "brand": "vivo",
        "model": "V2207",
        "manufacturer": "vivo",
        "device": "2207",
        "fingerprint": "vivo/V2207/2207:13/TP1A.220624.014/compiler12345:user/release-keys",
        "resolution": "1080,2408",
        "dpi": "440",
    },
    {
        "brand": "realme",
        "model": "RMX3686",
        "manufacturer": "realme",
        "device": "RE5C0FL1",
        "fingerprint": "realme/RMX3686/RE5C0FL1:13/TP1A.220905.001/R.1234567:user/release-keys",
        "resolution": "1080,2400",
        "dpi": "420",
    },
    {
        "brand": "Google",
        "model": "Pixel 7a",
        "manufacturer": "Google",
        "device": "lynx",
        "fingerprint": "google/lynx/lynx:14/UP1A.231005.007/10754064:user/release-keys",
        "resolution": "1080,2268",
        "dpi": "429",
    },
]


# =============== HELPER FUNCTIONS ===============

def random_imei():
    """Generate IMEI valid dengan Luhn checksum"""
    digits = [random.randint(0, 9) for _ in range(14)]
    total = 0
    for i, d in enumerate(digits):
        if i % 2 == 1:
            d *= 2
            if d > 9:
                d -= 9
        total += d
    check = (10 - (total % 10)) % 10
    digits.append(check)
    return ''.join(map(str, digits))


def random_mac():
    """Generate random MAC address"""
    mac = [random.randint(0x00, 0xff) for _ in range(6)]
    mac[0] = mac[0] & 0xfe
    return ':'.join(f'{b:02x}' for b in mac)


def random_android_id():
    """Generate random Android ID"""
    return ''.join(random.choices('0123456789abcdef', k=16))


def random_serial():
    """Generate random serial number"""
    return ''.join(random.choices(string.ascii_uppercase + string.digits, k=11))


def run_ldconsole(args):
    """Jalankan ldconsole command"""
    if not os.path.exists(LDCONSOLE):
        print(f"❌ ldconsole tidak ditemukan di: {LDCONSOLE}")
        print("   Pastikan LDPlayer 9 sudah terinstall!")
        return False
    try:
        cmd = [LDCONSOLE] + args
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        return result.returncode == 0
    except Exception as e:
        print(f"   Error: {e}")
        return False


def get_instance_name(index):
    return f"leidian{index}"


# =============== MAIN SETUP ===============

def setup_instance(index):
    """Setup 1 instance LDPlayer dengan profil HP acak"""
    device = random.choice(REAL_DEVICES)
    name = get_instance_name(index)

    imei = random_imei()
    imei2 = random_imei()
    android_id = random_android_id()
    mac = random_mac()
    serial = random_serial()

    print(f"\n[{index+1:02d}] Setting up {name}...")
    print(f"      Device : {device['brand']} {device['model']}")
    print(f"      IMEI   : {imei}")
    print(f"      Android ID: {android_id}")

    # Buat instance kalau belum ada
    run_ldconsole(["add", "--name", name])
    time.sleep(1)

    # Set model HP
    run_ldconsole([
        "modifyprop", "--index", str(index),
        "--key", "phoneModel", "--value", device["model"]
    ])

    # Set manufacturer
    run_ldconsole([
        "modifyprop", "--index", str(index),
        "--key", "phoneManufacturer", "--value", device["manufacturer"]
    ])

    # Set IMEI
    run_ldconsole([
        "modifyprop", "--index", str(index),
        "--key", "phoneIMEI", "--value", imei
    ])

    # Set IMEI2
    run_ldconsole([
        "modifyprop", "--index", str(index),
        "--key", "phoneIMEI2", "--value", imei2
    ])

    # Set Android ID
    run_ldconsole([
        "modifyprop", "--index", str(index),
        "--key", "phoneAndroidId", "--value", android_id
    ])

    # Set MAC
    run_ldconsole([
        "modifyprop", "--index", str(index),
        "--key", "macAddress", "--value", mac
    ])

    # Set serial
    run_ldconsole([
        "modifyprop", "--index", str(index),
        "--key", "phoneSerial", "--value", serial
    ])

    # Set resolusi
    w, h = device["resolution"].split(",")
    run_ldconsole([
        "modify", "--index", str(index),
        "--resolution", device["resolution"],
        "--dpi", device["dpi"]
    ])

    # Simpan profil
    profile = {
        "index": index,
        "name": name,
        "device": device,
        "imei": imei,
        "imei2": imei2,
        "android_id": android_id,
        "mac": mac,
        "serial": serial,
    }

    os.makedirs("profiles", exist_ok=True)
    with open(f"profiles/instance_{index:03d}.json", "w") as f:
        json.dump(profile, f, indent=2)

    print(f"      ✅ Done!")
    return profile


def update_config_files():
    """Update file config LDPlayer langsung"""
    config_dir = os.path.join(LDPLAYER_PATH, "vms", "config")
    if not os.path.exists(config_dir):
        print(f"⚠️  Config dir tidak ditemukan: {config_dir}")
        return

    config_files = glob.glob(os.path.join(config_dir, "leidian*.config"))
    print(f"\n📁 Ditemukan {len(config_files)} config file")

    for config_file in sorted(config_files):
        try:
            with open(config_file, 'r', encoding='utf-8') as f:
                config = json.load(f)

            device = random.choice(REAL_DEVICES)
            imei = random_imei()
            android_id = random_android_id()
            mac = random_mac()
            serial = random_serial()
            w, h = device["resolution"].split(",")

            # Update property settings
            if "propertySettings" not in config:
                config["propertySettings"] = {}

            config["propertySettings"]["phoneModel"] = device["model"]
            config["propertySettings"]["phoneManufacturer"] = device["manufacturer"]
            config["propertySettings"]["phoneBrand"] = device["brand"]
            config["propertySettings"]["phoneIMEI"] = imei
            config["propertySettings"]["phoneIMEI2"] = random_imei()
            config["propertySettings"]["phoneAndroidId"] = android_id
            config["propertySettings"]["macAddress"] = mac
            config["propertySettings"]["phoneSerial"] = serial

            # Update display
            if "statusSettings" not in config:
                config["statusSettings"] = {}
            if "resolution" not in config["statusSettings"]:
                config["statusSettings"]["resolution"] = {}

            config["statusSettings"]["resolution"]["width"] = int(w)
            config["statusSettings"]["resolution"]["height"] = int(h)

            if "advancedSettings" not in config:
                config["advancedSettings"] = {}
            config["advancedSettings"]["resolution"] = {"dpi": int(device["dpi"])}

            with open(config_file, 'w', encoding='utf-8') as f:
                json.dump(config, f, indent=2)

            fname = os.path.basename(config_file)
            print(f"  ✅ {fname} → {device['brand']} {device['model']} | IMEI: {imei}")

        except Exception as e:
            print(f"  ❌ {os.path.basename(config_file)}: {e}")


def main():
    print("=" * 60)
    print("  LDPlayer Auto-Setup - Device Spoof")
    print("=" * 60)
    print()

    print("Pilih metode setup:")
    print("  1. Update config file langsung (LDPlayer harus DITUTUP)")
    print("  2. Pakai ldconsole (LDPlayer bisa jalan)")
    print()

    choice = input("Pilihan [1/2]: ").strip() or "1"

    if choice == "1":
        print("\n⚠️  Pastikan LDPlayer sudah DITUTUP dulu!")
        input("Tekan Enter kalau sudah...")
        update_config_files()
    else:
        n = int(input(f"Berapa instance? [default: {NUM_INSTANCES}]: ") or NUM_INSTANCES)
        profiles = []
        for i in range(n):
            p = setup_instance(i)
            profiles.append(p)

        with open("profiles/all_profiles.json", "w") as f:
            json.dump(profiles, f, indent=2)

    print()
    print("=" * 60)
    print("✅ Setup selesai!")
    print()
    print("Langkah selanjutnya:")
    print("  1. Buka LDPlayer")
    print("  2. Setiap instance sudah punya device berbeda")
    print("  3. Login Google account berbeda di tiap instance")
    print("  4. Install app target di semua instance")
    print("=" * 60)


if __name__ == "__main__":
    main()
