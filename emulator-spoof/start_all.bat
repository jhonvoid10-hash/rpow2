@echo off
title LDPlayer - Start All Instances
color 0B
cls

echo ============================================================
echo   LDPlayer - Start All Instances
echo ============================================================
echo.

set LDCONSOLE=C:\LDPlayer\LDPlayer9\ldconsole.exe
set LDPLAYER=C:\LDPlayer\LDPlayer9\LDPlayer.exe

:: Cek LDPlayer
if not exist "%LDCONSOLE%" (
    echo [ERROR] LDPlayer tidak ditemukan!
    pause
    exit
)

:: Tanya berapa instance
set /p NUM="Berapa instance yang mau dijalankan? [default: 5]: "
if "%NUM%"=="" set NUM=5

:: Tanya delay antar instance
set /p DELAY="Delay antar instance (detik)? [default: 10]: "
if "%DELAY%"=="" set DELAY=10

echo.
echo Menjalankan %NUM% instance dengan delay %DELAY% detik...
echo.

:: Jalankan semua instance
for /l %%i in (0,1,%NUM%) do (
    if %%i lss %NUM% (
        echo [%%i] Menjalankan leidian%%i...
        "%LDCONSOLE%" launch --index %%i
        echo       Menunggu %DELAY% detik...
        timeout /t %DELAY% /nobreak >nul
    )
)

echo.
echo ============================================================
echo Semua %NUM% instance sudah berjalan!
echo ============================================================
echo.
pause
