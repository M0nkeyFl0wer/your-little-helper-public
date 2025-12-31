# Little Helper v2.0 - Implementation TODO

**Start Date:** 2025-12-31
**Target:** Windows first, then macOS (Apple Silicon)

---

## Phase 1: Foundation & Build System
*Target: 1 week*

### Windows Cross-Compilation
- [ ] Set up cross-compilation toolchain (mingw-w64)
- [ ] Add Windows target to Cargo.toml
- [ ] Test basic build: `cargo build --release --target x86_64-pc-windows-gnu`
- [ ] Resolve any Windows-specific dependency issues
- [ ] Create GitHub Actions workflow for Windows builds
- [ ] Test .exe on actual Windows machine

### Project Structure Updates
- [ ] Create `crates/viewers/` for file viewers
- [ ] Add `screens/` module to app crate
- [ ] Add `panels/` module to app crate  
- [ ] Add `widgets/` module to app crate
- [ ] Add `modals/` module to app crate
- [ ] Update workspace Cargo.toml with new crates

---

## Phase 2: Terminal Abstraction Layer
*Target: 1 week*

### Invisible Command Execution
- [ ] Refactor `agent_host` executor to return structured output
- [ ] Create `CommandResult` struct with: stdout, stderr, exit_code, duration
- [ ] Implement progress parsing (detect percentage, spinners, etc.)
- [ ] Create "activity indicator" UI component (not raw terminal)
- [ ] Parse common output patterns into user-friendly messages

### Expand Safe Commands
- [ ] Add file modification commands (cp, mv, mkdir) with confirmation flag
- [ ] Add rm with mandatory confirmation
- [ ] Add Python script execution (python, python3)
- [ ] Add Node.js execution (node, npm, npx)
- [ ] Add pip/pip3 with confirmation
- [ ] Add git commands (status, diff, add, commit, push)
- [ ] Add database CLIs (psql, sqlite3, mysql) - read queries only initially

### Command Confirmation System
- [ ] Create `DangerLevel` enum: Safe, NeedsConfirmation, Dangerous
- [ ] Classify all commands by danger level
- [ ] Create confirmation modal widget
- [ ] Show plain-English explanation of what command will do
- [ ] Log all executed commands to audit file

---

## Phase 3: Password/Sudo Handling
*Target: 3 days*

### Sudo Detection
- [ ] Detect `sudo` prefix in commands
- [ ] Detect "Permission denied" in output
- [ ] Detect "password required" patterns

### Password Modal
- [ ] Create secure password input widget (masked)
- [ ] Style modal for "admin access required"
- [ ] Clear password from memory after use
- [ ] Never log or store password

### Sudo Execution
- [ ] Implement stdin password piping to sudo
- [ ] Handle sudo timeout/failure gracefully
- [ ] Show success/failure message to user
- [ ] Test on Linux first, then Windows (runas), then macOS

---

## Phase 4: File Viewers
*Target: 2 weeks*

### PDF Viewer
- [ ] Research Rust PDF libraries (pdf-rs, pdfium-render, mupdf)
- [ ] Add PDF dependency to viewers crate
- [ ] Implement basic PDF rendering to texture
- [ ] Add page navigation (prev/next, jump to page)
- [ ] Add zoom controls
- [ ] Add text extraction for AI context
- [ ] Test with various PDF types (text, scanned, forms)

### HTML Viewer
- [ ] Add `wry` or similar webview dependency
- [ ] Create HTML preview panel
- [ ] Handle local file:// URLs
- [ ] Handle relative assets (images, CSS)
- [ ] Add source view toggle
- [ ] Security: sandbox webview, no external navigation

### Image Viewer
- [ ] Enhance current image display
- [ ] Add zoom (scroll wheel)
- [ ] Add pan (drag)
- [ ] Add fit-to-window / actual-size toggle
- [ ] Support: PNG, JPG, GIF, BMP, WebP, SVG
- [ ] Add image info display (dimensions, size, format)

### Table Viewer (CSV/Excel)
- [ ] Add `csv` and `calamine` dependencies
- [ ] Create table widget with headers
- [ ] Implement column sorting (click header)
- [ ] Implement column filtering
- [ ] Add search across all cells
- [ ] Handle large files (virtualized scrolling)
- [ ] Add export modified data
- [ ] Support: CSV, TSV, XLS, XLSX

### JSON Viewer
- [ ] Create tree view for nested JSON
- [ ] Add collapse/expand nodes
- [ ] Add raw view toggle
- [ ] Syntax highlighting
- [ ] Path display (click to copy path)

---

## Phase 5: Data Features
*Target: 1 week*

### SQLite Browser
- [ ] Add `rusqlite` dependency
- [ ] Create database connection manager
- [ ] List tables in sidebar
- [ ] Show table schema
- [ ] Browse table data (paginated)
- [ ] Query input with syntax highlighting
- [ ] Display query results in table view
- [ ] Export results to CSV
- [ ] AI can run SELECT queries
- [ ] Write queries require confirmation

### Data Operations
- [ ] AI can summarize CSV data
- [ ] AI can find patterns/anomalies
- [ ] AI can suggest data cleaning
- [ ] AI can transform/pivot data
- [ ] AI can generate charts (future)

---

## Phase 6: MCP Content Engine Integration
*Target: 1 week*

### API Client
- [ ] Create `mcp_client.rs` in services crate
- [ ] Implement authentication (API token)
- [ ] Endpoint: POST /reports (upload)
- [ ] Endpoint: GET /batches (list)
- [ ] Endpoint: GET /batches/{id} (details)
- [ ] Endpoint: PATCH /content/{id} (approve/reject)
- [ ] Endpoint: GET /schedule (calendar)
- [ ] Error handling and retries

### UI Components
- [ ] Create ContentQueue screen/panel
- [ ] List pending content items
- [ ] Show content preview with source reference
- [ ] Approve/reject buttons with feedback input
- [ ] Calendar view for scheduled content
- [ ] Platform icons (Facebook, Twitter, etc.)

### Workflow Integration
- [ ] "Create content from report" command
- [ ] Drag-drop report file to generate content
- [ ] AI can suggest edits to content
- [ ] Link to original source (click to open)

---

## Phase 7: Onboarding & Configuration
*Target: 3 days*

### First-Run Wizard
- [ ] Detect first run (no settings file)
- [ ] Screen 1: Welcome message
- [ ] Screen 2: Provider selection
  - [ ] "Use Team Account" (pre-configured)
  - [ ] "Enter API Key" (manual)
  - [ ] "Run Locally" (Ollama)
- [ ] Screen 3: Success + quick tips
- [ ] Skip option for power users

### Settings Improvements
- [ ] Encrypted API key storage
- [ ] Use OS keychain where available
- [ ] Settings UI with tabs (Providers, Integrations, UI, Security)
- [ ] Import/export settings
- [ ] Reset to defaults option

### Pre-configured Mode (Phase 1 Windows)
- [ ] Embed encrypted keys in binary
- [ ] Or bundle encrypted config file
- [ ] Machine-specific decryption key
- [ ] Document key rotation process

---

## Phase 8: macOS Build
*Target: 3 days*

### Cross-Compilation Setup
- [ ] Research aarch64-apple-darwin cross-compile options
- [ ] Option A: Build on actual Mac hardware
- [ ] Option B: Use osxcross toolchain
- [ ] Option C: GitHub Actions macOS runner

### App Bundle
- [ ] Create .app bundle structure
- [ ] Add Info.plist
- [ ] Add icon (icns format)
- [ ] Test on Apple Silicon Mac

### Code Signing (Optional)
- [ ] Ad-hoc signing for testing
- [ ] Document Gatekeeper bypass for users
- [ ] Future: Apple Developer account for proper signing

### macOS-Specific
- [ ] Test Gemini provider
- [ ] Test Ollama integration
- [ ] Test file permissions (sandbox)
- [ ] Test sudo/password handling

---

## Phase 9: Testing & Polish
*Target: 1 week*

### Testing
- [ ] Unit tests for new modules
- [ ] Integration tests for command execution
- [ ] Test all file viewers with various files
- [ ] Test MCP integration end-to-end
- [ ] Test on fresh Windows install
- [ ] Test on fresh macOS install
- [ ] Security audit of command execution

### Polish
- [ ] Consistent error messages
- [ ] Loading states for all async operations
- [ ] Keyboard shortcuts
- [ ] Tooltips for buttons
- [ ] Help menu / documentation link
- [ ] About dialog with version

### Bug Fixes
- [ ] (Track issues as they arise)

---

## Phase 10: Persona System Integration (Future/Optional)
*Defer until v2.1*

- [ ] API client for persona-generation-system
- [ ] Persona cards UI
- [ ] Link personas to content targeting
- [ ] AI can reference personas in content creation

---

## Stretch Goals (Post-Launch)

- [ ] Streaming AI responses
- [ ] Voice input (whisper)
- [ ] Voice output (TTS)
- [ ] Google Drive integration
- [ ] Slack bot integration
- [ ] Auto-updates
- [ ] Crash reporting (opt-in)
- [ ] Usage analytics (opt-in)
- [ ] Plugin system

---

## Notes & Decisions

### Resolved
- Windows build: Use mingw-w64 cross-compilation from Linux
- PDF library: TBD (evaluate pdf-rs vs pdfium)
- HTML rendering: Use wry (Tauri's webview)

### Open Questions
1. Pre-loaded API keys — acceptable for Phase 1?
2. Apple code signing — ad-hoc vs proper certificate?
3. MCP Engine — local or team server deployment?
4. Which Ollama models to recommend?

### Dependencies to Add
```toml
# viewers crate
pdf = "0.8"           # or pdfium-render
wry = "0.24"          # webview
image = "0.24"        # already have

# services crate  
csv = "1.3"
calamine = "0.22"     # Excel
rusqlite = "0.29"

# security
keyring = "2"         # OS keychain
aes-gcm = "0.10"      # encryption fallback
```

---

## Progress Tracking

| Phase | Status | Started | Completed |
|-------|--------|---------|-----------|
| 1. Foundation | Not Started | - | - |
| 2. Terminal Abstraction | Not Started | - | - |
| 3. Password Handling | Not Started | - | - |
| 4. File Viewers | Not Started | - | - |
| 5. Data Features | Not Started | - | - |
| 6. MCP Integration | Not Started | - | - |
| 7. Onboarding | Not Started | - | - |
| 8. macOS Build | Not Started | - | - |
| 9. Testing | Not Started | - | - |
| 10. Persona System | Deferred | - | - |
