@echo off
title LDPlayer - Stop All Instances
color 0C
cls

echo ============================================================
echo   LDPlayer - Stop All Instances
echo ============================================================
echo.

set LDCONSOLE=C:\LDPlayer\LDPlayer9\ldconsole.exe

if not exist "%LDCONSOLE%" (
    echo [ERROR] LDPlayer tidak ditemukan!
    pause
    exit
)

echo Menghentikan semua instance...
"%LDCONSOLE%" quitall
echo.
echo Semua instance sudah dihentikan!
echo.
pause
