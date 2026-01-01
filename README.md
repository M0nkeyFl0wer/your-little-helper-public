# Little Helper

A beautiful, private AI assistant for your desktop. Chat with AI to find files, get tech support, research topics, and work with data - all running locally on your machine.

## Download

**[Download Latest Release](https://github.com/M0nkeyFl0wer/claudia-lite/releases/latest)**

| Platform | File | Requirements |
|----------|------|--------------|
| **Mac (Apple Silicon)** | `LittleHelper-macOS-arm64.zip` | macOS 11+, M1/M2/M3/M4 |
| **Windows** | `LittleHelper-Windows.zip` | Windows 10/11, 64-bit |

---

## Quick Install

### Mac

1. Download `LittleHelper-macOS-arm64.zip`
2. Unzip and double-click **"Setup Little Helper.app"**
3. Click Install, enter your password, wait ~5 min
4. Done!

*First time opening: if macOS warns "unidentified developer" - right-click > Open > Open*

**Or use Terminal:**
```bash
curl -fsSL https://raw.githubusercontent.com/M0nkeyFl0wer/claudia-lite/main/install-easy.sh | bash
```

### Windows

1. Download `LittleHelper-Windows.zip`
2. Unzip and double-click **"Install.bat"**
3. Follow prompts, wait ~5 min for AI download
4. Done!

*Windows may show SmartScreen warning - click "More info" > "Run anyway"*

---

## Features

| Mode | What it does |
|------|--------------|
| **Find** | Search for files on your computer |
| **Fix** | Tech support and troubleshooting |
| **Research** | Deep research with web search |
| **Data** | Work with CSV, JSON, databases |
| **Content** | Create and manage content |

**Built-in viewers:** Text, Images, PDF, CSV, JSON, HTML

**Everything runs locally.** Your data stays on your machine.

---

## First Launch

The app will ask for your name and let you pick a background image (optional).

Choose your AI mode and start chatting:
- "Find my tax documents from 2023"
- "My WiFi is acting up, can you help?"
- "Research the history of coffee"

---

## Cloud Providers (Optional)

By default, Little Helper uses Ollama (local AI). You can add cloud providers:

```bash
# OpenAI
export OPENAI_API_KEY="your-key"

# Anthropic Claude
export ANTHROPIC_API_KEY="your-key"

# Google Gemini
export GEMINI_API_KEY="your-key"
```

---

## Uninstall

### Mac
```bash
rm -rf "/Applications/Little Helper.app"
launchctl unload ~/Library/LaunchAgents/com.littlehelper.ollama.plist
rm ~/Library/LaunchAgents/com.littlehelper.ollama.plist
```

### Windows
1. Delete the LittleHelper folder from `%LOCALAPPDATA%\LittleHelper`
2. Delete the desktop shortcut
3. Uninstall Ollama from Add/Remove Programs (optional)

---

## Build from Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/M0nkeyFl0wer/claudia-lite.git
cd claudia-lite
cargo build --release -p app
```

---

MIT License
