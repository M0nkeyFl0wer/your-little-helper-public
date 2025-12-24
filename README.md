# 🌸 Little Helper - Your Personal AI Assistant

A beautiful AI assistant that gives you control over your privacy. Chat with your personal AI to find files, get tech support, and research topics - choose between fully local processing or powerful cloud models.

## ✨ Features
- **🔍 Find Mode**: Ask your AI to find any file on your Mac, Google Drive, or connected drives
- **🔧 Fix Mode**: Get personalized tech support and troubleshooting help
- **🔌 Research Mode**: Have conversations about any topic with your AI
- **🎨 Beautiful Design**: Modern, rounded interface with soft colors
- **🔒 Privacy Options**: Choose fully local processing (Ollama) or cloud models (OpenAI, Anthropic, Gemini)
- **🤖 Smart Conversations**: Real back-and-forth chat with context memory

## 🚀 One-Click Installation (macOS)

Just run this single command in Terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/M0nkeyFl0wer/claudia-lite/main/install-mac.sh | bash
```

This automatically installs:
- ✅ Little Helper app in Applications
- ✅ Local AI engine (Ollama) for privacy-focused operation
- ✅ Smart AI model (3GB, optimized for conversations)
- ✅ Optional: Configure cloud providers (OpenAI, Anthropic, Gemini) in settings

## 🎯 How to Use

1. **Open Little Helper** from Applications
2. **Choose your mode**: Find, Fix, or Research  
3. **Start chatting!** Ask questions naturally like:
   - "Find my tax documents from 2023"
   - "My WiFi is acting up, can you help?"
   - "Tell me about sustainable gardening"

Your AI remembers the conversation context and asks follow-up questions to better help you!

## 🔐 AI Provider Configuration

Little Helper supports multiple AI providers, giving you complete control over privacy and capabilities:

### Local-Only (Default - Maximum Privacy)
By default, Little Helper uses **Ollama** for completely local AI processing. Your data never leaves your computer.

No configuration needed! Just install using the installer above.

### Cloud Providers (Optional - More Powerful)
You can optionally configure cloud AI providers for enhanced capabilities:

#### OpenAI
```bash
export OPENAI_API_KEY="your-api-key-here"
```
Get your API key from: https://platform.openai.com/api-keys

#### Anthropic (Claude)
```bash
export ANTHROPIC_API_KEY="your-api-key-here"
```
Get your API key from: https://console.anthropic.com/settings/keys

#### Google Gemini
```bash
export GEMINI_API_KEY="your-api-key-here"
```
Get your API key from: https://aistudio.google.com/app/apikey

### Provider Priority
Edit your settings file (`~/.config/Little Helper/LittleHelper/settings.json`) to configure provider fallback:

```json
{
  "model": {
    "provider_preference": ["local", "openai", "anthropic", "gemini"]
  }
}
```

The app will try each provider in order until one succeeds. For maximum privacy, use `["local"]` only.

## 🛠️ Developer Setup

If you want to build from source:
```bash
# Prerequisites: Rust toolchain + Ollama running locally
cargo run -p app
```
