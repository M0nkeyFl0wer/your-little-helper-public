# Little Helper

A beautiful, private AI assistant for your Mac. Chat with AI to find files, get tech support, and research topics - all running locally on your machine.

## Install (Apple Silicon Macs)

One command in Terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/M0nkeyFl0wer/claudia-lite/main/install-easy.sh | bash
```

**What it does:**
- Downloads Little Helper app (~5MB)
- Installs Ollama AI engine
- Downloads AI model (~2GB)
- Sets up auto-start

**Requirements:** macOS 11+ on Apple Silicon (M1/M2/M3/M4)

---

## Features

**Find Mode** - Ask your AI to find files on your Mac, Google Drive, or connected drives

**Fix Mode** - Get personalized tech support and troubleshooting help

**Research Mode** - Have conversations about any topic

Everything runs locally. Your data never leaves your Mac.

---

## First Launch

macOS will warn about "unidentified developer" the first time:

1. **Right-click** the app in Applications
2. Click **Open**
3. Click **Open** again in the dialog

After that, it opens normally.

---

## How to Use

1. Open Little Helper from Applications
2. Choose your mode: Find, Fix, or Research
3. Start chatting! Ask questions naturally like:
   - "Find my tax documents from 2023"
   - "My WiFi is acting up, can you help?"
   - "Tell me about sustainable gardening"

---

## Cloud Providers (Optional)

By default, everything runs locally with Ollama. You can optionally add cloud AI:

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

```bash
rm -rf "/Applications/Little Helper.app"
launchctl unload ~/Library/LaunchAgents/com.littlehelper.ollama.plist
rm ~/Library/LaunchAgents/com.littlehelper.ollama.plist
```

---

## Build from Source

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/M0nkeyFl0wer/claudia-lite.git
cd claudia-lite
cargo build --release -p app
```

---

MIT License
