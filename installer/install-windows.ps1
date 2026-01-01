# Little Helper - Windows Installer
# Run: powershell -ExecutionPolicy Bypass -File install-windows.ps1

Write-Host ""
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host "       Little Helper Setup" -ForegroundColor Magenta
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host ""

# Check Windows
if ($env:OS -ne "Windows_NT") {
    Write-Host "This installer is for Windows only" -ForegroundColor Yellow
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Host "This will install:" -ForegroundColor Cyan
Write-Host "  - Little Helper app"
Write-Host "  - Ollama AI engine (optional, for local AI)"
Write-Host "  - AI model (~2GB download, optional)"
Write-Host ""
Write-Host "The app works immediately - Ollama setup is optional!" -ForegroundColor Yellow
Write-Host ""

$confirm = Read-Host "Ready to install? (Y/n)"
if ($confirm -eq "n" -or $confirm -eq "N") {
    Write-Host "Installation cancelled."
    exit 0
}

# Create install directory
$installDir = "$env:LOCALAPPDATA\LittleHelper"
Write-Host ""
Write-Host "Installing to: $installDir" -ForegroundColor Cyan

if (Test-Path $installDir) {
    Write-Host "Removing old installation..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force $installDir -ErrorAction SilentlyContinue
}
New-Item -ItemType Directory -Force -Path $installDir | Out-Null

Write-Host ""
Write-Host "Step 1/3: Downloading Little Helper..." -ForegroundColor Cyan

# Download pre-built app
$releaseUrl = "https://github.com/M0nkeyFl0wer/your-little-helper/releases/latest/download/LittleHelper-Windows.zip"
$zipPath = "$env:TEMP\LittleHelper.zip"

try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $releaseUrl -OutFile $zipPath -UseBasicParsing
    Write-Host "Downloaded!" -ForegroundColor Green
} catch {
    Write-Host "Download failed: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please check:" -ForegroundColor Yellow
    Write-Host "  - Your internet connection"
    Write-Host "  - That releases exist at: $releaseUrl"
    Write-Host ""
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Host "Extracting..." -ForegroundColor Cyan
try {
    Expand-Archive -Path $zipPath -DestinationPath $installDir -Force
    Remove-Item $zipPath -ErrorAction SilentlyContinue
    
    # Handle nested folder from zip (LittleHelper-Windows subfolder)
    $nestedDir = "$installDir\LittleHelper-Windows"
    if (Test-Path $nestedDir) {
        Get-ChildItem $nestedDir | Move-Item -Destination $installDir -Force
        Remove-Item $nestedDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    
    Write-Host "Extracted!" -ForegroundColor Green
} catch {
    Write-Host "Extraction failed: $_" -ForegroundColor Red
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Host ""
Write-Host "Step 2/3: Creating shortcuts..." -ForegroundColor Cyan

# Create desktop shortcut
try {
    $WshShell = New-Object -comObject WScript.Shell
    $Shortcut = $WshShell.CreateShortcut("$env:USERPROFILE\Desktop\Little Helper.lnk")
    $Shortcut.TargetPath = "$installDir\LittleHelper.exe"
    $Shortcut.WorkingDirectory = $installDir
    $Shortcut.Description = "Your AI Assistant"
    $Shortcut.Save()
    Write-Host "Desktop shortcut created!" -ForegroundColor Green
} catch {
    Write-Host "Could not create shortcut (not critical)" -ForegroundColor Yellow
}

# Create Start Menu shortcut
try {
    $startMenuPath = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs"
    $Shortcut = $WshShell.CreateShortcut("$startMenuPath\Little Helper.lnk")
    $Shortcut.TargetPath = "$installDir\LittleHelper.exe"
    $Shortcut.WorkingDirectory = $installDir
    $Shortcut.Description = "Your AI Assistant"
    $Shortcut.Save()
    Write-Host "Start Menu shortcut created!" -ForegroundColor Green
} catch {
    Write-Host "Could not create Start Menu shortcut (not critical)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Step 3/3: Ollama AI Setup (Optional)" -ForegroundColor Cyan
Write-Host ""
Write-Host "Ollama lets you run AI locally on your computer." -ForegroundColor White
Write-Host "It's free and your data stays private." -ForegroundColor White
Write-Host ""

$installOllama = Read-Host "Install Ollama for local AI? (Y/n)"

if ($installOllama -ne "n" -and $installOllama -ne "N") {
    # Check if Ollama is already installed
    $ollamaPath = Get-Command ollama -ErrorAction SilentlyContinue

    if (-not $ollamaPath) {
        Write-Host ""
        Write-Host "Downloading Ollama..." -ForegroundColor Yellow
        $ollamaUrl = "https://ollama.com/download/OllamaSetup.exe"
        $ollamaInstaller = "$env:TEMP\OllamaSetup.exe"
        
        try {
            Invoke-WebRequest -Uri $ollamaUrl -OutFile $ollamaInstaller -UseBasicParsing
            Write-Host "Download complete!" -ForegroundColor Green
            
            Write-Host ""
            Write-Host "Running Ollama installer..." -ForegroundColor Yellow
            Write-Host "(Follow the installation prompts that appear)" -ForegroundColor White
            Start-Process -FilePath $ollamaInstaller -Wait
            
            # Refresh PATH
            $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
            
            Write-Host "Ollama installed!" -ForegroundColor Green
        } catch {
            Write-Host "Ollama download failed: $_" -ForegroundColor Yellow
            Write-Host "You can install it later from: https://ollama.com/download" -ForegroundColor Yellow
        }
    } else {
        Write-Host "Ollama already installed!" -ForegroundColor Green
    }

    # Check again after potential install
    $ollamaPath = Get-Command ollama -ErrorAction SilentlyContinue
    
    if ($ollamaPath) {
        Write-Host ""
        Write-Host "Downloading AI model (~2GB)..." -ForegroundColor Cyan
        Write-Host "This takes a few minutes. Please wait..." -ForegroundColor Yellow
        Write-Host ""

        # Start Ollama service
        Start-Process -FilePath "ollama" -ArgumentList "serve" -WindowStyle Hidden -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 3

        # Pull the model
        try {
            & ollama pull llama3.2:3b
            Write-Host ""
            Write-Host "AI model ready!" -ForegroundColor Green
        } catch {
            Write-Host "Model download had issues. You can try later with:" -ForegroundColor Yellow
            Write-Host "  ollama pull llama3.2:3b" -ForegroundColor White
        }
    }
} else {
    Write-Host ""
    Write-Host "Skipping Ollama setup." -ForegroundColor Yellow
    Write-Host "You can install it later from: https://ollama.com/download" -ForegroundColor White
}

Write-Host ""
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host "       Installation Complete!" -ForegroundColor Green
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host ""
Write-Host "Little Helper is ready!" -ForegroundColor Green
Write-Host ""
Write-Host "Find it at:" -ForegroundColor Cyan
Write-Host "  - Desktop shortcut"
Write-Host "  - Start Menu"
Write-Host "  - $installDir\LittleHelper.exe"
Write-Host ""

$open = Read-Host "Open Little Helper now? (Y/n)"
if ($open -ne "n" -and $open -ne "N") {
    Start-Process -FilePath "$installDir\LittleHelper.exe"
}

Write-Host ""
Write-Host "You can close this window now." -ForegroundColor Cyan
Read-Host "Press Enter to exit"
