#!/bin/bash

# Little Helper - GUI Installer Script
# Called by Setup Little Helper.app

clear
echo ""
echo "  ╔═══════════════════════════════════════╗"
echo "  ║     🌸 Little Helper Setup 🌸         ║"
echo "  ╚═══════════════════════════════════════╝"
echo ""

# Colors
PINK='\033[38;5;213m'
BLUE='\033[38;5;117m'
GREEN='\033[38;5;120m'
YELLOW='\033[38;5;228m'
NC='\033[0m'

# Check macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${YELLOW}This installer is for macOS only${NC}"
    exit 1
fi

echo -e "${BLUE}Step 1/4: Downloading Little Helper app...${NC}"
echo ""

TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

# Download pre-built app
RELEASE_URL="https://github.com/M0nkeyFl0wer/your-little-helper/releases/latest/download/LittleHelper-macOS-arm64.zip"

if curl -fsSL -o "LittleHelper.zip" "$RELEASE_URL" 2>/dev/null; then
    unzip -q "LittleHelper.zip"
    echo -e "${GREEN}✓ Downloaded!${NC}"
else
    echo -e "${YELLOW}Could not download pre-built app.${NC}"
    echo "Please check your internet connection and try again."
    read -p "Press Enter to exit..."
    exit 1
fi

echo ""
echo -e "${BLUE}Step 2/4: Installing app to Applications...${NC}"
echo ""

if [[ -d "/Applications/Little Helper.app" ]]; then
    rm -rf "/Applications/Little Helper.app"
fi
mv "Little Helper.app" "/Applications/"
echo -e "${GREEN}✓ App installed!${NC}"

echo ""
echo -e "${BLUE}Step 3/4: Installing Ollama AI engine...${NC}"
echo ""

if ! command -v ollama &> /dev/null; then
    echo "This requires your Mac password (one time only):"
    echo ""
    curl -fsSL https://ollama.com/install.sh | sh
    echo ""
    echo -e "${GREEN}✓ Ollama installed!${NC}"
else
    echo -e "${GREEN}✓ Ollama already installed!${NC}"
fi

echo ""
echo -e "${BLUE}Step 4/4: Downloading AI model...${NC}"
echo -e "${YELLOW}This downloads ~2GB and takes a few minutes.${NC}"
echo ""

# Start Ollama in background
ollama serve &>/dev/null &
sleep 3

# Download the model
if ! ollama list 2>/dev/null | grep -q "llama3.2:3b"; then
    ollama pull llama3.2:3b
fi

echo ""
echo -e "${GREEN}✓ AI model ready!${NC}"

# Setup auto-start for Ollama
PLIST_PATH="$HOME/Library/LaunchAgents/com.littlehelper.ollama.plist"
mkdir -p "$HOME/Library/LaunchAgents"

OLLAMA_PATH=$(which ollama)
cat > "$PLIST_PATH" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.littlehelper.ollama</string>
    <key>ProgramArguments</key>
    <array>
        <string>$OLLAMA_PATH</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF

launchctl load "$PLIST_PATH" 2>/dev/null || true

# Cleanup
cd "$HOME"
rm -rf "$TEMP_DIR"

echo ""
echo "  ╔═══════════════════════════════════════╗"
echo "  ║     ✨ Installation Complete! ✨      ║"
echo "  ╚═══════════════════════════════════════╝"
echo ""
echo -e "${GREEN}Little Helper is ready to use!${NC}"
echo ""
echo -e "${YELLOW}First time opening the app:${NC}"
echo "  macOS may warn about 'unidentified developer'"
echo "  Just right-click > Open > Open"
echo ""

# Open the app
echo "Opening Little Helper..."
sleep 2
open "/Applications/Little Helper.app"

echo ""
echo "You can close this window now."
echo ""
