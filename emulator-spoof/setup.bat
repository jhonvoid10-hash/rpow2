@echo off
title LDPlayer Auto-Setup
color 0A
cls

echo ============================================================
echo   LDPlayer Auto-Setup - Device Spoof
echo   by jhonvoid10-hash
echo ============================================================
echo.

:: Cek Python
python --version >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Python tidak ditemukan!
    echo Silahkan download di: https://python.org
    pause
    exit
)

:: Cek LDPlayer
if not exist "C:\LDPlayer\LDPlayer9\ldconsole.exe" (
    echo [WARNING] LDPlayer tidak ditemukan di C:\LDPlayer\LDPlayer9\
    echo Pastikan LDPlayer 9 sudah terinstall!
    echo.
)

:: Install requirements
echo [1/4] Install requirements...
pip install psutil -q
echo       Done!

:: Jalankan spoof generator
echo [2/4] Generate device profiles...
python spoof_device.py
echo       Done!

:: Jalankan LDPlayer setup
echo [3/4] Setup LDPlayer instances...
python ldplayer_setup.py
echo       Done!

echo.
echo ============================================================
echo [4/4] Setup selesai!
echo.
echo Langkah selanjutnya:
echo   1. Buka LDPlayer
echo   2. Tiap instance sudah punya device berbeda
echo   3. Login Google account berbeda di tiap instance
echo   4. Install app target (RPOW2, Grass, dll)
echo ============================================================
echo.
pause
