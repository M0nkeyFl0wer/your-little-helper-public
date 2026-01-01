@echo off
REM Little Helper - Windows One-Click Installer
REM Downloads and runs the PowerShell installer

echo.
echo ========================================
echo    Little Helper - Windows Installer
echo ========================================
echo.

REM Check for PowerShell
where powershell >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo ERROR: PowerShell is required but not found.
    echo Please install PowerShell and try again.
    pause
    exit /b 1
)

echo Downloading installer...
echo.

REM Download and run the installer
powershell -ExecutionPolicy Bypass -Command ^
    "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; ^
    $installer = \"$env:TEMP\install-littlehelper.ps1\"; ^
    Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/M0nkeyFl0wer/your-little-helper/main/installer/install-windows.ps1' -OutFile $installer; ^
    & $installer"

pause
