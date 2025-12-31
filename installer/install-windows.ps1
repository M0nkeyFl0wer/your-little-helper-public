# Little Helper - Windows Installer
# Run this script to install everything

Write-Host ""
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host "       Little Helper Setup" -ForegroundColor Magenta
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host ""

# Check Windows
if ($env:OS -ne "Windows_NT") {
    Write-Host "This installer is for Windows only" -ForegroundColor Yellow
    exit 1
}

Write-Host "This will install:" -ForegroundColor Cyan
Write-Host "  - Little Helper app"
Write-Host "  - Ollama AI engine"
Write-Host "  - AI model (~2GB download)"
Write-Host ""

$confirm = Read-Host "Ready to install? (Y/n)"
if ($confirm -eq "n" -or $confirm -eq "N") {
    Write-Host "Installation cancelled."
    exit 0
}

# Create install directory
$installDir = "$env:LOCALAPPDATA\LittleHelper"
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
Set-Location $installDir

Write-Host ""
Write-Host "Step 1/4: Downloading Little Helper..." -ForegroundColor Cyan

# Download pre-built app
$releaseUrl = "https://github.com/M0nkeyFl0wer/your-little-helper/releases/latest/download/LittleHelper-Windows.zip"

try {
    Invoke-WebRequest -Uri $releaseUrl -OutFile "LittleHelper.zip" -UseBasicParsing
    Expand-Archive -Path "LittleHelper.zip" -DestinationPath "." -Force
    Remove-Item "LittleHelper.zip"
    Write-Host "Downloaded!" -ForegroundColor Green
} catch {
    Write-Host "Could not download. Check your internet connection." -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "Step 2/4: Creating desktop shortcut..." -ForegroundColor Cyan

# Create desktop shortcut
$WshShell = New-Object -comObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut("$env:USERPROFILE\Desktop\Little Helper.lnk")
$Shortcut.TargetPath = "$installDir\LittleHelper.exe"
$Shortcut.WorkingDirectory = $installDir
$Shortcut.Save()
Write-Host "Shortcut created!" -ForegroundColor Green

Write-Host ""
Write-Host "Step 3/4: Installing Ollama AI engine..." -ForegroundColor Cyan

# Check if Ollama is installed
$ollamaPath = Get-Command ollama -ErrorAction SilentlyContinue

if (-not $ollamaPath) {
    Write-Host "Downloading Ollama installer..." -ForegroundColor Yellow
    $ollamaUrl = "https://ollama.com/download/OllamaSetup.exe"
    Invoke-WebRequest -Uri $ollamaUrl -OutFile "$env:TEMP\OllamaSetup.exe" -UseBasicParsing

    Write-Host "Running Ollama installer (follow the prompts)..." -ForegroundColor Yellow
    Start-Process -FilePath "$env:TEMP\OllamaSetup.exe" -Wait

    # Refresh PATH
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

    Write-Host "Ollama installed!" -ForegroundColor Green
} else {
    Write-Host "Ollama already installed!" -ForegroundColor Green
}

Write-Host ""
Write-Host "Step 4/4: Downloading AI model..." -ForegroundColor Cyan
Write-Host "This downloads ~2GB and takes a few minutes." -ForegroundColor Yellow
Write-Host ""

# Start Ollama service
Start-Process -FilePath "ollama" -ArgumentList "serve" -WindowStyle Hidden -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3

# Pull the model
& ollama pull llama3.2:3b

Write-Host ""
Write-Host "AI model ready!" -ForegroundColor Green

Write-Host ""
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host "       Installation Complete!" -ForegroundColor Green
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host ""
Write-Host "Little Helper is ready to use!" -ForegroundColor Green
Write-Host ""
Write-Host "You can find it:" -ForegroundColor Cyan
Write-Host "  - On your Desktop (shortcut)"
Write-Host "  - At: $installDir\LittleHelper.exe"
Write-Host ""

$open = Read-Host "Open Little Helper now? (Y/n)"
if ($open -ne "n" -and $open -ne "N") {
    Start-Process -FilePath "$installDir\LittleHelper.exe"
}

Write-Host ""
Write-Host "You can close this window now."
Read-Host "Press Enter to exit"
