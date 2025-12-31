# Little Helper v2.0 - Full Specification

**Project:** claudia-lite (Your Little Helper)
**Date:** 2025-12-31
**Status:** Planning Phase
**Target Platforms:** Windows (primary), macOS (Apple Silicon)

---

## Executive Summary

Little Helper v2.0 transforms the current AI chat app into a **frictionless desktop IDE for non-technical users**. The tool enables team members to work with files, data, and AI without ever seeing a terminal — while the AI agent has full terminal access behind the scenes.

### Key Principles
1. **Zero friction onboarding** — Double-click exe/app, start working
2. **Terminal power, no terminal UI** — Agent runs scripts invisibly
3. **Live collaboration** — Files pop open, preview in real-time
4. **Password prompts, not sudo errors** — Graceful privilege escalation
5. **Data-first workflow** — CSV, databases, content creation built-in

---

## Target Users & Deployment Strategy

### Phase 1: Windows Version (Boss + Senior Team)
- **Users:** 3-5 senior team members
- **Distribution:** Single .exe file (or .msi installer)
- **Auth:** Pre-configured API keys baked in OR easy first-run setup
- **Purpose:** Get feedback, iterate before wider rollout

### Phase 2: macOS Version (Friend on Apple Silicon)
- **Users:** 1 external collaborator
- **Distribution:** .app bundle (unsigned or ad-hoc signed)
- **Auth:** Gemini API + local Ollama (no pre-loaded keys)
- **Purpose:** Cross-platform validation, privacy-conscious deployment

### Phase 3: Team Rollout
- **Users:** Full team
- **Distribution:** Based on Phase 1 feedback
- **Auth:** Flexible per-user configuration

---

## Core Features

### 1. Frictionless Onboarding

#### Windows (.exe)
```
User downloads little-helper.exe
  → Double-click
  → First-run wizard (3 screens max):
     1. "Welcome! Let's get you set up"
     2. Choose provider: [Use Team Account] [Use Your Own Key] [Run Locally]
     3. "You're ready!" → Main app
```

#### macOS (.app)
```
User downloads LittleHelper.app
  → Drag to Applications (or run from Downloads)
  → Gatekeeper warning: "Open Anyway" in System Preferences
  → Same 3-screen wizard
```

#### Pre-configured Mode (Phase 1)
- API keys embedded in binary or bundled config
- User just clicks "Use Team Account" and starts working
- Keys stored encrypted, not plain text

### 2. Invisible Terminal Access

The AI agent has **full shell access** but users never see terminal output directly.

#### How It Works
```
User: "Run the weekly report script"
  → Agent executes: ./scripts/weekly_report.py
  → Progress shown as: "Running weekly report... ████████░░ 80%"
  → Output parsed and shown as: "Report complete! Generated 47 pages."
  → Errors shown as: "I hit a snag — the database connection timed out. Want me to retry?"
```

#### Safe Command Expansion
Current whitelist is read-only. Expand to include:
- File modifications (cp, mv, mkdir, rm with confirmation)
- Python/Node script execution
- Package managers (pip, npm) with confirmation
- Database CLI tools (psql, sqlite3)
- Git operations

#### Dangerous Commands
- Require explicit confirmation modal
- Show what will happen in plain English
- "This will delete 47 files. Are you sure?"

### 3. Password/Sudo Handling

When a command needs elevated privileges:

```
Agent tries: sudo apt update
  → Detects "sudo" or permission error
  → Shows modal: "This action requires your password"
  → Secure password input (dots, not visible)
  → Agent retries with password via stdin
  → Password NEVER stored or logged
```

#### Implementation
```rust
// Detect sudo requirement
if command.contains("sudo") || output.contains("Permission denied") {
    // Show password modal
    let password = show_password_dialog("This action requires admin access");
    // Re-run with password piped to stdin
    run_with_sudo(command, password);
}
```

### 4. Live File Preview & Collaboration

#### File Types Supported
| Type | Preview Method | Collaboration |
|------|---------------|---------------|
| **Text/Code** | Syntax-highlighted editor | AI can read/write |
| **Markdown** | Rendered preview + source | AI can edit |
| **HTML** | Rendered webview | AI can edit source |
| **PDF** | Embedded viewer | AI can read, extract text |
| **Images** | Full display, zoom/pan | AI can describe, basic edits |
| **CSV/Excel** | Table view with sorting | AI can query, transform |
| **JSON** | Tree view + raw | AI can edit |
| **SQLite** | Table browser | AI can query |

#### "Pop Open" Behavior
```
User: "Show me the Q4 report"
  → Agent finds: /reports/Q4_2024.pdf
  → PDF automatically opens in preview panel
  → Agent: "Here's the Q4 report. The summary is on page 3."

User: "Let's work on the newsletter draft"
  → Agent finds: /drafts/newsletter.md
  → Markdown opens in editor panel (left) + preview (right)
  → Agent: "I've opened the newsletter. Want me to improve the intro?"
```

#### Agent Awareness
The agent's system prompt includes:
```
You have the ability to open files for the user to view and collaborate on.
When you find or create a file, USE THE OPEN COMMAND to display it.
Available viewers: text, markdown, html, pdf, image, csv, json, sqlite
Example: <open file="/path/to/file.csv" viewer="csv"/>
```

### 5. Data & Database Features

#### CSV/Excel Handling
- Load CSV files into table view
- Sort, filter, search columns
- AI can: summarize, find patterns, clean data, transform
- Export modified data

#### SQLite Browser
- Open .db files directly
- Browse tables, view schema
- Run queries (AI-assisted or manual)
- Export query results to CSV

#### Database Connections (Future)
- PostgreSQL, MySQL connections via config
- AI can run read queries safely
- Write queries require confirmation

### 6. Content Engine Integration

Integration with **MCP-research-content-automation-engine** for content creation workflows.

#### Features
- Load research reports (markdown) into Little Helper
- Generate social media content via MCP engine API
- Review/approve content in-app
- Schedule content from within the app

#### Integration Method
```rust
// MCP Engine runs as local service or remote
struct MCPClient {
    base_url: String,  // http://localhost:8000 or remote
    api_token: String,
}

impl MCPClient {
    async fn process_report(&self, report_path: &str) -> Result<ContentBatch>;
    async fn get_batch(&self, batch_id: &str) -> Result<ContentBatch>;
    async fn approve_item(&self, item_id: &str) -> Result<()>;
    async fn get_schedule(&self) -> Result<Schedule>;
}
```

#### UI Components
- **Content Queue:** List of pending content items
- **Approval Interface:** Accept/reject with feedback
- **Calendar View:** Scheduled posts by date/platform
- **Source Linking:** Click to see original report section

### 7. Persona Generation Integration (Optional)

Integration with **persona-generation-system** for research and content targeting.

#### Features
- Generate audience personas from data
- Understand psychological profiles for content
- Match content to persona preferences

#### Integration
- REST API calls to persona service
- Display persona cards in UI
- AI can reference personas when creating content

---

## Technical Architecture

### Crate Structure (Updated)
```
claudia-lite/
├── crates/
│   ├── app/              # Main GUI (egui/eframe)
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── screens/  # Setup, Chat, ContentQueue
│   │   │   ├── panels/   # Preview, Editor, Terminal (hidden)
│   │   │   ├── widgets/  # FileTree, TableView, PDFViewer
│   │   │   └── modals/   # Password, Confirmation, Settings
│   ├── agent_host/       # AI agent + command execution
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── executor.rs      # Shell command runner
│   │   │   ├── sudo_handler.rs  # Privilege escalation
│   │   │   └── file_opener.rs   # Open files in UI
│   ├── providers/        # AI providers (Claude, Gemini, Ollama, OpenAI)
│   ├── services/         # File search, data ops
│   │   ├── src/
│   │   │   ├── file_search.rs
│   │   │   ├── csv_handler.rs   # NEW
│   │   │   ├── sqlite_browser.rs # NEW
│   │   │   └── mcp_client.rs    # NEW - Content engine
│   ├── viewers/          # NEW - File viewers
│   │   ├── src/
│   │   │   ├── pdf_viewer.rs
│   │   │   ├── html_viewer.rs
│   │   │   ├── image_viewer.rs
│   │   │   └── table_viewer.rs
│   └── shared/           # Common types
```

### Key Dependencies (New)
```toml
# PDF rendering
pdf = "0.8"              # Or pdfium-render for better quality
# HTML rendering
wry = "0.24"             # WebView for HTML/web content
# CSV/Excel
csv = "1.3"
calamine = "0.22"        # Excel reading
# SQLite
rusqlite = "0.29"
# Image handling (already have, enhance)
image = "0.24"
```

### Build Targets

#### Windows
```bash
# Native Windows build
cargo build --release --target x86_64-pc-windows-msvc

# Cross-compile from Linux
cargo build --release --target x86_64-pc-windows-gnu
```

#### macOS (Apple Silicon)
```bash
# Must build ON a Mac or use cross-compilation
cargo build --release --target aarch64-apple-darwin
```

### Packaging

#### Windows Options
1. **Single .exe** — Simplest, just the binary
2. **.msi installer** — Professional, adds Start Menu entry
3. **Portable .zip** — Binary + config in folder

#### macOS Options
1. **.app bundle** — Standard Mac app structure
2. **.dmg** — Disk image for drag-to-install
3. **Unsigned** — Requires Gatekeeper bypass (fine for small team)

---

## UI/UX Design

### Main Layout
```
┌─────────────────────────────────────────────────────────────┐
│  [Little Helper]           [Find] [Fix] [Create]    [⚙️]   │
├─────────────────────────────────────────────────────────────┤
│                    │                    │                   │
│    CHAT PANEL      │   EDITOR PANEL     │  PREVIEW PANEL    │
│                    │                    │                   │
│  [AI messages]     │  [File content]    │  [Rendered view]  │
│  [User messages]   │  [Syntax highlight]│  [PDF/HTML/Image] │
│  [Progress bars]   │  [Line numbers]    │  [Table view]     │
│                    │                    │                   │
├─────────────────────────────────────────────────────────────┤
│  Type a message...                              [Send]      │
└─────────────────────────────────────────────────────────────┘
```

### Panel Behavior
- All panels collapsible
- Double-click file in chat → opens in Editor + Preview
- Drag panel dividers to resize
- Remember layout between sessions

### New Chat Modes
| Mode | Purpose | Agent Behavior |
|------|---------|----------------|
| **Find** | Locate files, search | Read-only commands |
| **Fix** | Debug, repair issues | Can modify with confirmation |
| **Create** | Content generation | Opens editor, generates drafts |
| **Data** | Work with CSV/DB | Table view, queries |

### Modals
1. **Password Modal** — Secure input for sudo
2. **Confirmation Modal** — "This will delete X files"
3. **Settings Modal** — Provider config, preferences
4. **First-Run Wizard** — Onboarding flow

---

## Security Considerations

### API Key Storage
- Keys encrypted at rest (not plain JSON)
- Use OS keychain where available (Windows Credential Manager, macOS Keychain)
- Fallback: encrypted file with machine-specific key

### Command Execution
- Whitelist approach (allow known-safe commands)
- Dangerous commands require confirmation
- Full audit log of executed commands
- No arbitrary code execution from AI

### Password Handling
- Never stored
- Never logged
- Cleared from memory after use
- Used only for immediate sudo operation

### Network Security
- All API calls over HTTPS
- Validate SSL certificates
- No telemetry without explicit consent

---

## Integration APIs

### MCP Content Engine
```
Base URL: http://localhost:8000 (local) or configured remote

Endpoints Used:
  POST /reports          - Upload research report
  GET  /batches          - List content batches
  GET  /batches/{id}     - Get batch details
  PATCH /content/{id}    - Update approval status
  GET  /schedule         - Get content schedule
```

### Persona Generation (Optional)
```
Base URL: http://localhost:3000 (local) or configured remote

Endpoints Used:
  GET  /api/v1/personas          - List personas
  GET  /api/v1/personas/{id}     - Get persona details
  POST /api/v1/research/analyze  - Analyze target data
```

---

## Configuration

### Settings File Structure
```json
{
  "version": "2.0",
  "provider": {
    "default": "anthropic",
    "anthropic": {
      "model": "claude-sonnet-4-5-20250929",
      "auth_type": "oauth"  // or "api_key"
    },
    "gemini": {
      "model": "gemini-pro",
      "api_key_encrypted": "..."
    },
    "ollama": {
      "model": "llama3.2:3b",
      "host": "localhost:11434"
    }
  },
  "integrations": {
    "mcp_engine": {
      "enabled": true,
      "url": "http://localhost:8000",
      "api_token_encrypted": "..."
    },
    "persona_system": {
      "enabled": false,
      "url": "http://localhost:3000"
    }
  },
  "ui": {
    "theme": "light",
    "layout": {
      "chat_width": 0.3,
      "editor_width": 0.35,
      "preview_width": 0.35
    }
  },
  "security": {
    "command_confirmations": true,
    "audit_log": true
  }
}
```

### Environment Variables (Alternative)
```bash
LITTLE_HELPER_PROVIDER=anthropic
ANTHROPIC_API_KEY=sk-...
MCP_ENGINE_URL=http://localhost:8000
```

---

## Success Metrics

### Phase 1 (Boss + Senior Team)
- [ ] Successfully onboard 3+ users without IT support
- [ ] Complete 10+ real work tasks without terminal exposure
- [ ] Gather feedback on pain points
- [ ] Zero security incidents

### Phase 2 (Mac Version)
- [ ] Runs on Apple Silicon without issues
- [ ] Gemini + Ollama work correctly
- [ ] File operations work cross-platform

### Phase 3 (Full Team)
- [ ] 90%+ of team can use independently
- [ ] Integrated into daily workflow
- [ ] Content engine producing real output

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Windows build fails | Test early, use GitHub Actions CI |
| macOS code signing | Use ad-hoc signing for small team |
| API rate limits | Implement retry logic, show clear errors |
| Data loss from scripts | Always backup before modifications |
| Password security | Never store, clear from memory |
| AI hallucinations | Confirmation for destructive actions |

---

## Timeline Estimate

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| **Foundation** | 1 week | Build system, Windows cross-compile |
| **Terminal Abstraction** | 1 week | Invisible execution, progress display |
| **Password Handling** | 3 days | Sudo modal, secure input |
| **File Viewers** | 2 weeks | PDF, HTML, CSV, Image viewers |
| **Data Features** | 1 week | SQLite browser, CSV handling |
| **MCP Integration** | 1 week | Content engine API client |
| **Onboarding Flow** | 3 days | First-run wizard |
| **Testing & Polish** | 1 week | Bug fixes, UX refinement |
| **Mac Build** | 3 days | Cross-compile, test on hardware |

**Total: ~7-8 weeks**

---

## Open Questions

1. **Code signing** — Do we have Apple Developer account for proper signing?
2. **Pre-loaded keys** — Is it acceptable to embed API keys in Phase 1 binary?
3. **MCP Engine hosting** — Local or deploy to team server?
4. **Persona system** — Priority or defer to future version?
5. **Ollama models** — Which models to recommend for local use?

---

## Appendix: Current State Summary

### What Exists (claudia-lite v1)
- Multi-provider AI chat (Claude, Gemini, OpenAI, Ollama)
- Claude Code OAuth integration
- Agent with read-only command execution
- Basic file preview (text only)
- File search backend (not in UI)
- Settings persistence

### What's Missing for v2
- Windows build pipeline
- Expanded command execution
- Password/sudo handling
- PDF, HTML, image viewers
- CSV/Excel handling
- SQLite browser
- MCP content engine integration
- Persona system integration
- First-run onboarding wizard
- macOS build for Apple Silicon
