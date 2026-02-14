# Little Helper

```
 / \__
(    @\___
 /         O
/   (_____/
/_____/   U
```

**Your Private Agentic Assistant.**  
Little Helper is more than a chatbot—it's a desktop agent with a semantic brain, git-backed memory, and a powerful spec-driven build system. It runs locally, keeping your data safe.

## Download

**[Download Latest Release](https://github.com/M0nkeyFl0wer/your-little-helper-public/releases/latest)**

| Platform | File | Requirements |
|----------|------|--------------|
| **Mac (Apple Silicon)** | `LittleHelper-macOS-arm64.zip` | macOS 11+, M1/M2/M3/M4 |
| **Windows** | `LittleHelper-Windows.zip` | Windows 10/11, 64-bit |

---

## What Makes It Different?

### 🧠 Semantic Brain (Graph RAG)
Unlike standard bots that forget everything, Little Helper builds a knowledge graph of your entities, projects, and preferences. It learns from your feedback and connects the dots between related concepts using a locally structured graph database.

### 🛡️ Dual-Layer Git Memory
1.  **Shadow Git (`.little-helper/versions`):** Every file change you make is backed up here automatically. This is your "Undo" button. It works even if you haven't initialized a real git repo.
2.  **Real Git Integration:** The agent acts as your "Git Co-pilot." It can `git init`, `git add`, and `git commit` to your actual project repository, helping you maintain a clean history without leaving the chat.

### 🏗️ Spec-Driven Development (Native Rust)
The `Build` mode features a native "Spec Kit" workflow to take you from idea to code:
1.  **Scaffold**: `scaffold my-app` (Creates `specs/`, `src/`, and `git init`)
2.  **Spec**: `init spec` (Creates a structured spec template in `specs/`)
3.  **Implement**: `implement spec` (The agent reads the spec and writes the code)

*No Node.js or external dependencies required—just pure Rust power.*

---

## Modes & Skills

| Mode | Personality | Superpower |
|------|-------------|------------|
| **Find** | **Scout** | Deep file search & metadata hunting |
| **Fix** | **Doc** | System diagnostics & auto-healing (with permission) |
| **Research** | **Scholar** | Multi-hop web research with citation verification |
| **Build** | **Spec** | **Spec-Kit & Git Co-pilot** |
| **Data** | **Analyst** | CSV/JSON analysis & visualization |
| **Content** | **Muse** | Creative writing & persona management |

**Built-in Viewers:** Text, Images, PDF, CSV, JSON, HTML

---

## Privacy Protocols

- **Local First:** Logic runs on your machine.
- **Permissioned Access:** "Terminal commands are never auto-run." You approve every valid command.
- **Secret Scanning:** The agent proactively detects and warns before you paste secrets or API keys.
- **Safe Defaults:** Generated scripts use safe permissions (e.g., `chmod 600`).

---

## Quick Start
### Mac
`curl -fsSL https://raw.githubusercontent.com/M0nkeyFl0wer/your-little-helper-public/main/install-easy.sh | bash`

### Windows
Download `LittleHelper-Windows.zip`, unzip, and run `Install.bat`.

---

## Cloud Providers (Optional)
By default, Little Helper uses Ollama (local AI). You can add cloud providers:
```bash
export OPENAI_API_KEY="your-key"
export ANTHROPIC_API_KEY="your-key"
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
1. Delete `%LOCALAPPDATA%\LittleHelper`
2. Delete the desktop shortcut

---

## Build from Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/M0nkeyFl0wer/your-little-helper.git
cd your-little-helper
cargo build --release -p app
```

MIT License
