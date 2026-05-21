"""
Emulator Spoof - Bikin MuMu/LDPlayer terdeteksi sebagai HP asli
===============================================================
Cara pakai:
1. Install: pip install frida-tools
2. Jalankan emulator
3. Jalankan: python spoof_device.py
"""

import random
import string
import json
import os

# =============== DATABASE HP ASLI ===============

REAL_DEVICES = [
    {
        "brand": "Samsung",
        "model": "SM-G991B",
        "device": "o1s",
        "product": "o1sxeea",
        "manufacturer": "samsung",
        "board": "exynos2100",
        "hardware": "exynos2100",
        "fingerprint": "samsung/o1sxeea/o1s:13/TP1A.220624.014/G991BXXS7DWAA:user/release-keys"
    },
    {
        "brand": "Samsung",
        "model": "SM-A536B",
        "device": "a53x",
        "product": "a53xnaxx",
        "manufacturer": "samsung",
        "board": "s5e8825",
        "hardware": "s5e8825",
        "fingerprint": "samsung/a53xnaxx/a53x:14/UP1A.231005.007/A536BXXS8CXA1:user/release-keys"
    },
    {
        "brand": "Xiaomi",
        "model": "2201116SG",
        "device": "viva",
        "product": "viva_global",
        "manufacturer": "Xiaomi",
        "board": "viva",
        "hardware": "mt6781",
        "fingerprint": "Xiaomi/viva_global/viva:13/TP1A.220624.014/V14.0.4.0.TGCMIXM:user/release-keys"
    },
    {
        "brand": "Xiaomi",
        "model": "23049RAD8C",
        "device": "earth",
        "product": "earth_global",
        "manufacturer": "Xiaomi",
        "board": "earth",
        "hardware": "mt6789",
        "fingerprint": "Xiaomi/earth_global/earth:14/UP1A.230905.011/V816.0.2.0.UMFMIXM:user/release-keys"
    },
    {
        "brand": "OPPO",
        "model": "CPH2239",
        "device": "OP52B1L1",
        "product": "CPH2239",
        "manufacturer": "OPPO",
        "board": "mt6833",
        "hardware": "mt6833",
        "fingerprint": "OPPO/CPH2239/OP52B1L1:13/TP1A.220905.001/S.1234567-890:user/release-keys"
    },
    {
        "brand": "vivo",
        "model": "V2111",
        "device": "2111",
        "product": "PD2111",
        "manufacturer": "vivo",
        "board": "mt6833",
        "hardware": "mt6833",
        "fingerprint": "vivo/PD2111/2111:13/TP1A.220624.014/compiler1234567:user/release-keys"
    },
    {
        "brand": "realme",
        "model": "RMX3511",
        "device": "RE58B2L1",
        "product": "RMX3511",
        "manufacturer": "realme",
        "board": "mt6833",
        "hardware": "mt6833",
        "fingerprint": "realme/RMX3511/RE58B2L1:13/TP1A.220905.001/R.12345678:user/release-keys"
    },
    {
        "brand": "Google",
        "model": "Pixel 7",
        "device": "panther",
        "product": "panther",
        "manufacturer": "Google",
        "board": "cloudripper",
        "hardware": "tensor",
        "fingerprint": "google/panther/panther:14/UP1A.231005.007/10754064:user/release-keys"
    },
]


def random_imei():
    """Generate random IMEI yang valid"""
    digits = [random.randint(0, 9) for _ in range(14)]
    # Luhn checksum
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
    mac[0] = mac[0] & 0xfe  # unicast
    return ':'.join(f'{b:02x}' for b in mac)


def random_android_id():
    """Generate random Android ID (16 hex chars)"""
    return ''.join(random.choices('0123456789abcdef', k=16))


def random_serial():
    """Generate random serial number"""
    return ''.join(random.choices(string.ascii_uppercase + string.digits, k=11))


def random_wifi_mac():
    """Generate random WiFi MAC"""
    return random_mac()


def generate_device_profile(index=0):
    """Generate profil HP unik untuk setiap instance emulator"""
    device = random.choice(REAL_DEVICES)

    profile = {
        "index": index,
        "device": {
            "brand": device["brand"],
            "model": device["model"],
            "device": device["device"],
            "product": device["product"],
            "manufacturer": device["manufacturer"],
            "board": device["board"],
            "hardware": device["hardware"],
            "fingerprint": device["fingerprint"],
        },
        "identifiers": {
            "imei": random_imei(),
            "imei2": random_imei(),
            "android_id": random_android_id(),
            "serial": random_serial(),
            "mac_address": random_wifi_mac(),
            "wifi_mac": random_wifi_mac(),
        },
        "display": {
            "width": random.choice([1080, 1440, 2340]),
            "height": random.choice([2400, 2560, 3120]),
            "density": random.choice([420, 480, 560]),
        },
        "network": {
            "operator": random.choice(["51010", "51011", "51089", "310260", "310410"]),
            "operator_name": random.choice(["Telkomsel", "XL", "3", "T-Mobile", "AT&T"]),
            "wifi_ssid": f"Home_{random.randint(100,999)}",
        },
        "location": {
            "latitude": round(random.uniform(-7.0, -6.0), 6),
            "longitude": round(random.uniform(106.0, 107.0), 6),
        },
        "battery": {
            "level": random.randint(30, 95),
            "temperature": random.randint(25, 38),
        },
        "sensors": {
            "accelerometer": True,
            "gyroscope": True,
            "proximity": True,
            "light": True,
        }
    }
    return profile


def generate_build_prop(profile):
    """Generate build.prop override untuk emulator"""
    d = profile["device"]
    ids = profile["identifiers"]
    display = profile["display"]

    props = f"""# Device Spoof - Generated
ro.product.brand={d['brand']}
ro.product.model={d['model']}
ro.product.device={d['device']}
ro.product.name={d['product']}
ro.product.manufacturer={d['manufacturer']}
ro.product.board={d['board']}
ro.hardware={d['hardware']}
ro.build.fingerprint={d['fingerprint']}
ro.serialno={ids['serial']}
ro.boot.serialno={ids['serial']}
persist.sys.timezone=Asia/Jakarta
ro.sf.lcd_density={display['density']}
gsm.sim.operator.numeric={profile['network']['operator']}
gsm.operator.alpha={profile['network']['operator_name']}
"""
    return props


def generate_magisk_props(profile):
    """Generate MagiskHide props untuk spoof lebih dalam"""
    d = profile["device"]
    props = f"""FINGERPRINT={d['fingerprint']}
BRAND={d['brand']}
DEVICE={d['device']}
DISPLAY=TP1A.220624.014
MANUFACTURER={d['manufacturer']}
MODEL={d['model']}
PRODUCT={d['product']}
"""
    return props


def generate_mumu_config(profile):
    """Generate config untuk MuMu Player"""
    d = profile["device"]
    ids = profile["identifiers"]
    display = profile["display"]

    config = {
        "phone_model": d["model"],
        "phone_brand": d["brand"],
        "phone_manufacturer": d["manufacturer"],
        "imei": ids["imei"],
        "android_id": ids["android_id"],
        "mac_address": ids["mac_address"],
        "resolution_width": display["width"],
        "resolution_height": display["height"],
        "dpi": display["density"],
    }
    return config


def generate_ldplayer_config(profile):
    """Generate config untuk LDPlayer"""
    d = profile["device"]
    ids = profile["identifiers"]
    display = profile["display"]

    config = f"""propertySettings.phoneModel={d['model']}
propertySettings.phoneManufacturer={d['manufacturer']}
propertySettings.phoneBrand={d['brand']}
propertySettings.phoneIMEI={ids['imei']}
propertySettings.phoneIMSI={random_android_id()}
propertySettings.phoneSimSerial={random_serial()}
propertySettings.phoneAndroidId={ids['android_id']}
propertySettings.macAddress={ids['mac_address']}
statusSettings.resolution.width={display['width']}
statusSettings.resolution.height={display['height']}
advancedSettings.resolution.dpi={display['density']}
"""
    return config


def main():
    """Generate profil untuk N instance emulator"""
    num_instances = int(input("Berapa instance emulator? [default: 20]: ") or "20")

    output_dir = "profiles"
    os.makedirs(output_dir, exist_ok=True)

    all_profiles = []

    for i in range(num_instances):
        profile = generate_device_profile(i)
        all_profiles.append(profile)

        # Simpan per instance
        instance_dir = os.path.join(output_dir, f"instance_{i:03d}")
        os.makedirs(instance_dir, exist_ok=True)

        # build.prop
        with open(os.path.join(instance_dir, "build.prop"), "w") as f:
            f.write(generate_build_prop(profile))

        # MuMu config
        with open(os.path.join(instance_dir, "mumu_config.json"), "w") as f:
            json.dump(generate_mumu_config(profile), f, indent=2)

        # LDPlayer config
        with open(os.path.join(instance_dir, "ldplayer.config"), "w") as f:
            f.write(generate_ldplayer_config(profile))

        # MagiskHide props
        with open(os.path.join(instance_dir, "magisk_props.txt"), "w") as f:
            f.write(generate_magisk_props(profile))

        # Full profile JSON
        with open(os.path.join(instance_dir, "profile.json"), "w") as f:
            json.dump(profile, f, indent=2)

        print(f"  [✓] Instance {i:03d}: {profile['device']['brand']} {profile['device']['model']} | IMEI: {profile['identifiers']['imei']}")

    # Simpan semua profil dalam 1 file
    with open(os.path.join(output_dir, "all_profiles.json"), "w") as f:
        json.dump(all_profiles, f, indent=2)

    print(f"\n✅ {num_instances} profil sudah digenerate di folder: {output_dir}/")
    print(f"\nCara pakai:")
    print(f"  MuMu:     Import mumu_config.json di settings tiap instance")
    print(f"  LDPlayer: Replace config di LDPlayer/vms/config/instance_X.config")
    print(f"  Magisk:   Upload magisk_props.txt ke /data/adb/modules/")


if __name__ == "__main__":
    main()
