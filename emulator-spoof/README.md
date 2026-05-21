# Emulator Spoof - Bikin Emulator Terdeteksi Seperti HP Asli

## Cara Pakai

### 1. Generate profil
```cmd
cd emulator-spoof
python spoof_device.py
```

Masukkan jumlah instance yang diinginkan (misal: 20)

### 2. Output
Akan dibuatkan folder `profiles/` berisi:
```
profiles/
├── instance_000/
│   ├── profile.json          ← data lengkap
│   ├── build.prop            ← untuk push ke emulator
│   ├── mumu_config.json      ← setting MuMu Player
│   ├── ldplayer.config       ← setting LDPlayer
│   └── magisk_props.txt      ← untuk MagiskHide
├── instance_001/
│   └── ...
└── all_profiles.json         ← semua profil dalam 1 file
```

---

## Setup per Emulator

### MuMu Player:
1. Buka MuMu Multi Player
2. Buat instance baru
3. Klik Settings (gear icon)
4. Tab "Other" → masukkan:
   - Phone Model: dari `mumu_config.json`
   - IMEI: dari `mumu_config.json`
5. Tab "Display" → set resolution dari config

### LDPlayer:
1. Tutup LDPlayer
2. Buka folder: `C:\LDPlayer\LDPlayer9\vms\config\`
3. Edit file `leidian0.config`, `leidian1.config` dll
4. Tambahkan isi dari `ldplayer.config`
5. Buka LDPlayer lagi

### Magisk (untuk root emulator):
1. Install Magisk di emulator
2. Install module "MagiskHide Props Config"
3. Push `magisk_props.txt` ke `/data/adb/modules/`
4. Reboot emulator

---

## Apa yang di-spoof:

| Property | Keterangan |
|---|---|
| Brand/Model | Samsung, Xiaomi, OPPO, dll |
| IMEI | Random unik (valid Luhn) |
| Android ID | Random 16 hex |
| Serial | Random |
| MAC Address | Random |
| Fingerprint | Build fingerprint HP asli |
| Display | Resolusi HP asli |
| Network | Operator/SIM simulasi |
| GPS | Koordinat random Indonesia |
| Battery | Level & temp simulasi |
| Sensors | Accelerometer, gyro, dll |

---

## Anti-Detection Tips:

1. **Jangan pakai resolusi emulator default** (1280x720)
2. **Disable ADB over network** di emulator
3. **Hapus app emulator** yang ke-install default (misal MuMu assistant)
4. **Pakai Google Play Store** seperti HP biasa
5. **Install app normal** (WA, IG, dll) biar terlihat natural
6. **Jangan jalankan semua instance sekaligus** — mulai 5-10 dulu
