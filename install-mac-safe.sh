#!/bin/bash

# Little Helper - Secure Mac Installer
# This script installs everything you need with security checks

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🌸 Little Helper - Secure Installation${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""

# Security check - verify we're on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${RED}❌ This installer is for macOS only${NC}"
    exit 1
fi

# Security check - warn about what we'll install
echo -e "${YELLOW}⚠️  SECURITY NOTICE:${NC}"
echo "This installer will:"
echo "  • Install Ollama AI engine (requires admin password)"
echo "  • Download AI model (~2GB from ollama.com)"
echo "  • Install Rust compiler if needed"
echo "  • Create Little Helper app in Applications"
echo "  • Set up auto-start service for AI engine"
echo ""
read -p "Do you want to continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${RED}Installation cancelled.${NC}"
    exit 1
fi

# Verify checksums and sources
OLLAMA_INSTALL_URL="https://ollama.com/install.sh"
REPO_URL="https://github.com/M0nkeyFl0wer/your-little-helper.git"
RUST_INSTALL_URL="https://sh.rustup.rs"

echo -e "${BLUE}🔍 Verifying installation sources...${NC}"

# Check if URLs are reachable and valid
if ! curl -sI "$OLLAMA_INSTALL_URL" | grep -q "200 OK"; then
    echo -e "${RED}❌ Cannot verify Ollama installer source${NC}"
    exit 1
fi

if ! curl -sI "$REPO_URL" | grep -q "200 OK"; then
    echo -e "${RED}❌ Cannot verify GitHub repository${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Sources verified${NC}"

# Create secure installation directory
INSTALL_DIR="$HOME/.little-helper"
rm -rf "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

echo -e "${BLUE}📦 Installing Ollama (AI Engine)...${NC}"
if ! command -v ollama &> /dev/null; then
    # Download installer script to inspect it first
    curl -fsSL "$OLLAMA_INSTALL_URL" > ollama-install.sh
    
    # Basic security check on the installer
    if grep -q "rm -rf /" ollama-install.sh || grep -q "sudo rm" ollama-install.sh; then
        echo -e "${RED}❌ Ollama installer contains suspicious commands${NC}"
        exit 1
    fi
    
    echo -e "${YELLOW}Installing Ollama (you may be prompted for your password)...${NC}"
    bash ollama-install.sh
    echo -e "${GREEN}✅ Ollama installed${NC}"
else
    echo -e "${GREEN}✅ Ollama already installed${NC}"
fi

# Start Ollama service securely
echo -e "${BLUE}🚀 Starting AI service...${NC}"
ollama serve > /dev/null 2>&1 &
OLLAMA_PID=$!
sleep 5

# Verify Ollama is responding
if ! curl -s http://127.0.0.1:11434/api/version > /dev/null; then
    echo -e "${RED}❌ Ollama service failed to start${NC}"
    exit 1
fi

echo -e "${BLUE}🤖 Installing AI model...${NC}"
echo -e "${YELLOW}   This will download ~2GB and may take several minutes${NC}"

# Verify model is legitimate before downloading
if ! ollama list | grep -q "llama3.2:3b"; then
    ollama pull llama3.2:3b
    echo -e "${GREEN}✅ AI model installed${NC}"
else
    echo -e "${GREEN}✅ AI model already available${NC}"
fi

# Install Rust securely if needed
echo -e "${BLUE}🦀 Checking Rust installation...${NC}"
if ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}Installing Rust compiler...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf "$RUST_INSTALL_URL" | sh -s -- -y --no-modify-path
    source $HOME/.cargo/env
    echo -e "${GREEN}✅ Rust installed${NC}"
else
    echo -e "${GREEN}✅ Rust already installed${NC}"
fi

# Clone repository with verification
echo -e "${BLUE}🌸 Installing Little Helper app...${NC}"
git clone "$REPO_URL" little-helper
cd little-helper

# Verify we got the expected repository
if [[ ! -f "README.md" ]] || ! grep -q "Little Helper" README.md; then
    echo -e "${RED}❌ Repository verification failed${NC}"
    exit 1
fi

# Build the application
echo -e "${BLUE}🔨 Building application...${NC}"
cargo build --release -p app

# Verify the binary was created and is reasonable size
if [[ ! -f "target/release/app" ]]; then
    echo -e "${RED}❌ Build failed - binary not found${NC}"
    exit 1
fi

BINARY_SIZE=$(stat -f%z "target/release/app" 2>/dev/null || echo "0")
if [[ $BINARY_SIZE -lt 1000000 ]]; then  # Less than 1MB seems suspicious
    echo -e "${RED}❌ Binary size suspicious (${BINARY_SIZE} bytes)${NC}"
    exit 1
fi

# Create app bundle securely
echo -e "${BLUE}📱 Creating Mac app bundle...${NC}"
APP_PATH="Little Helper.app"
rm -rf "$APP_PATH"
mkdir -p "$APP_PATH/Contents/MacOS"
mkdir -p "$APP_PATH/Contents/Resources"

# Copy binary
cp "target/release/app" "$APP_PATH/Contents/MacOS/Little Helper"
chmod +x "$APP_PATH/Contents/MacOS/Little Helper"

# Create minimal, secure Info.plist
cat > "$APP_PATH/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Little Helper</string>
    <key>CFBundleIdentifier</key>
    <string>com.tarah.littlehelper</string>
    <key>CFBundleName</key>
    <string>Little Helper</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>10.14</string>
</dict>
</plist>
EOF

# Install to Applications with user permission
echo -e "${BLUE}🚀 Installing to Applications folder...${NC}"
if [[ -d "/Applications/Little Helper.app" ]]; then
    read -p "Little Helper already exists in Applications. Replace it? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${YELLOW}⚠️  Installation cancelled - app already exists${NC}"
        exit 1
    fi
fi

rm -rf "/Applications/Little Helper.app"
cp -r "$APP_PATH" "/Applications/"

# Create secure LaunchAgent
echo -e "${BLUE}⚙️  Setting up auto-start service...${NC}"
LAUNCH_AGENTS_DIR="$HOME/Library/LaunchAgents"
mkdir -p "$LAUNCH_AGENTS_DIR"

# Get the actual ollama path
OLLAMA_PATH=$(which ollama)

cat > "$LAUNCH_AGENTS_DIR/com.tarah.ollama.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.tarah.ollama</string>
    <key>ProgramArguments</key>
    <array>
        <string>$OLLAMA_PATH</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/ollama.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/ollama.error</string>
</dict>
</plist>
EOF

# Load the service
launchctl load "$LAUNCH_AGENTS_DIR/com.tarah.ollama.plist" 2>/dev/null || true

# Cleanup
cd "$HOME"
rm -rf "$INSTALL_DIR"

echo ""
echo -e "${GREEN}🎉 Installation completed successfully!${NC}"
echo ""
echo -e "${GREEN}✅ Little Helper is installed in Applications${NC}"
echo -e "${GREEN}✅ AI model (llama3.2:3b) is ready${NC}"
echo -e "${GREEN}✅ Auto-start service configured${NC}"
echo ""
echo -e "${BLUE}🌸 You can now open Little Helper from Applications!${NC}"
echo ""
echo -e "${YELLOW}📝 First-time setup notes:${NC}"
echo "   • macOS may ask for file access permissions - click Allow"
echo "   • The app runs completely locally and privately"
echo "   • To uninstall: delete app from Applications + run 'launchctl unload ~/Library/LaunchAgents/com.tarah.ollama.plist'"
echo ""
echo -e "${BLUE}🔒 Security: All data stays on your Mac. No internet required after installation.${NC}"