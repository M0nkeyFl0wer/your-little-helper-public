#!/bin/bash

# Little Helper - One-Click Mac Installer
# This script installs everything you need to run Little Helper

set -e

echo "🌸 Installing Little Helper..."
echo ""

# Check if we're on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo "❌ This installer is for macOS only"
    exit 1
fi

# Create Little Helper directory
INSTALL_DIR="$HOME/Little Helper"
mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

echo "📦 Installing Ollama (AI Engine)..."
# Install Ollama
if ! command -v ollama &> /dev/null; then
    curl -fsSL https://ollama.com/install.sh | sh
    echo "✅ Ollama installed"
else
    echo "✅ Ollama already installed"
fi

# Start Ollama service
echo "🚀 Starting AI service..."
ollama serve &
OLLAMA_PID=$!
sleep 3

# Install a good, lightweight local model
echo "🤖 Installing AI model (this may take a few minutes)..."
echo "   Downloading a smart but fast model for local processing..."

# Use a smaller, faster model that's great for conversation
if ! ollama list | grep -q "llama3.2:3b"; then
    ollama pull llama3.2:3b
    echo "✅ AI model installed"
else
    echo "✅ AI model already available"
fi

# Install Rust if not present (needed to build Little Helper)
echo "🦀 Checking Rust installation..."
if ! command -v cargo &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
    echo "✅ Rust installed"
else
    echo "✅ Rust already installed"
fi

# Clone and build Little Helper
echo "🌸 Installing Little Helper app..."
if [ ! -d "little-helper" ]; then
    git clone https://github.com/M0nkeyFl0wer/claudia-lite.git little-helper
else
    cd little-helper
    git pull
    cd ..
fi

cd little-helper
cargo build --release -p app

# Create app bundle for Mac
echo "📱 Creating Mac app bundle..."
mkdir -p "Little Helper.app/Contents/MacOS"
mkdir -p "Little Helper.app/Contents/Resources"

# Copy the binary
cp target/release/app "Little Helper.app/Contents/MacOS/Little Helper"

# Create Info.plist
cat > "Little Helper.app/Contents/Info.plist" << EOF
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
    <string>1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

# Move to Applications folder
echo "🚀 Installing to Applications folder..."
rm -rf "/Applications/Little Helper.app"
cp -r "Little Helper.app" "/Applications/"

# Create startup script for Ollama
cat > "$HOME/Library/LaunchAgents/com.littlehelper.ollama.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.littlehelper.ollama</string>
    <key>ProgramArguments</key>
    <array>
        <string>$(which ollama)</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF

# Load the service
launchctl load "$HOME/Library/LaunchAgents/com.littlehelper.ollama.plist"

echo ""
echo "🎉 Installation complete!"
echo ""
echo "✅ Little Helper is now installed in Applications"
echo "✅ AI model (llama3.2:3b) is ready for local processing"
echo "✅ Ollama service will start automatically"
echo ""
echo "🌸 You can now open Little Helper from Applications!"
echo "   By default, it uses local AI for complete privacy."
echo "   You can optionally configure cloud providers (OpenAI, Anthropic, Gemini) in settings."
echo ""
echo "📝 Note: The first time you use it, macOS may ask for permission"
echo "   to access files - just click 'Allow' to enable file search."