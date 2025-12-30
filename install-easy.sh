#!/bin/bash

# Little Helper - Easy Mac Installer
# Downloads pre-built app, no compilation needed!

set -e

# Colors
PINK='\033[38;5;213m'
BLUE='\033[38;5;117m'
GREEN='\033[38;5;120m'
YELLOW='\033[38;5;228m'
NC='\033[0m'

echo ""
echo -e "${PINK}  .--.      Little Helper${NC}"
echo -e "${PINK} |o_o |     Your AI Assistant${NC}"
echo -e "${PINK} |:_/ |     ${NC}"
echo -e "${PINK}//   \\ \\    Installing...${NC}"
echo -e "${PINK}(|     | )${NC}"
echo -e "${PINK}/'\\_   _/\`\\${NC}"
echo -e "${PINK}\\___)=(___/${NC}"
echo ""

# Check macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${YELLOW}This installer is for macOS only${NC}"
    exit 1
fi

# Detect architecture
ARCH=$(uname -m)
if [[ "$ARCH" == "arm64" ]]; then
    echo -e "${BLUE}Detected: Apple Silicon Mac${NC}"
else
    echo -e "${BLUE}Detected: Intel Mac${NC}"
fi

echo ""
echo -e "${BLUE}This will install:${NC}"
echo "  - Little Helper app (pre-built, ~5MB)"
echo "  - Ollama AI engine (if not installed)"
echo "  - AI model (~2GB download)"
echo ""
read -p "Ready to install? (Y/n): " -n 1 -r
echo
if [[ $REPLY =~ ^[Nn]$ ]]; then
    echo "Installation cancelled."
    exit 0
fi

# Create temp directory
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

# Download pre-built app
echo ""
echo -e "${BLUE}Downloading Little Helper...${NC}"

# Get latest release URL
RELEASE_URL="https://github.com/M0nkeyFl0wer/claudia-lite/releases/latest/download/LittleHelper-macOS-universal.zip"

if curl -fsSL -o "LittleHelper.zip" "$RELEASE_URL" 2>/dev/null; then
    echo -e "${GREEN}Downloaded!${NC}"
else
    echo -e "${YELLOW}No release found. Building from source (this takes a few minutes)...${NC}"

    # Fallback: build from source
    if ! command -v cargo &> /dev/null; then
        echo -e "${BLUE}Installing Rust (needed to build)...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    git clone --depth 1 https://github.com/M0nkeyFl0wer/claudia-lite.git
    cd claudia-lite
    cargo build --release -p app

    mkdir -p "Little Helper.app/Contents/MacOS"
    cp target/release/app "Little Helper.app/Contents/MacOS/Little Helper"
    chmod +x "Little Helper.app/Contents/MacOS/Little Helper"

    cat > "Little Helper.app/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Little Helper</string>
    <key>CFBundleIdentifier</key>
    <string>com.littlehelper.app</string>
    <key>CFBundleName</key>
    <string>Little Helper</string>
    <key>CFBundleVersion</key>
    <string>1.0.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

    cd ..
    mv claudia-lite/"Little Helper.app" ./
fi

# Unzip if we downloaded
if [[ -f "LittleHelper.zip" ]]; then
    unzip -q "LittleHelper.zip"
fi

# Install to Applications
echo -e "${BLUE}Installing to Applications...${NC}"
if [[ -d "/Applications/Little Helper.app" ]]; then
    rm -rf "/Applications/Little Helper.app"
fi
mv "Little Helper.app" "/Applications/"
echo -e "${GREEN}App installed!${NC}"

# Install Ollama
echo ""
echo -e "${BLUE}Setting up AI engine...${NC}"
if ! command -v ollama &> /dev/null; then
    echo "Installing Ollama..."
    curl -fsSL https://ollama.com/install.sh | sh
    echo -e "${GREEN}Ollama installed!${NC}"
else
    echo -e "${GREEN}Ollama already installed!${NC}"
fi

# Start Ollama
echo -e "${BLUE}Starting AI service...${NC}"
ollama serve &>/dev/null &
sleep 3

# Download AI model
echo ""
echo -e "${BLUE}Downloading AI model (this takes a few minutes)...${NC}"
echo -e "${YELLOW}The AI brain is about 2GB - please be patient!${NC}"

if ! ollama list 2>/dev/null | grep -q "llama3.2:3b"; then
    ollama pull llama3.2:3b
    echo -e "${GREEN}AI model ready!${NC}"
else
    echo -e "${GREEN}AI model already installed!${NC}"
fi

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

# Done!
echo ""
echo -e "${PINK}======================================${NC}"
echo -e "${GREEN}  Installation Complete!${NC}"
echo -e "${PINK}======================================${NC}"
echo ""
echo -e "${BLUE}To launch:${NC}"
echo "  1. Open Applications folder"
echo "  2. Double-click 'Little Helper'"
echo ""
echo -e "${YELLOW}First time opening:${NC}"
echo "  macOS may warn 'unidentified developer'"
echo "  Right-click the app > Open > Open"
echo ""
echo -e "${PINK}Enjoy your new AI assistant!${NC}"
echo ""

# Offer to open now
read -p "Open Little Helper now? (Y/n): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Nn]$ ]]; then
    open "/Applications/Little Helper.app"
fi
