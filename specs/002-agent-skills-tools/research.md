# Research: Agent Skills and Tools

**Feature**: 002-agent-skills-tools | **Date**: 2026-01-04

## 1. SQLite Fuzzy Search Strategy

**Decision**: Use SQLite FTS5 with trigram tokenizer + strsim for re-ranking

**Rationale**:
- FTS5 provides sub-second full-text search on millions of rows
- Trigram tokenizer enables partial matching (handles typos, partial names)
- Rust's `rusqlite` crate has excellent FTS5 support
- Post-query re-ranking with `strsim::jaro_winkler` gives fzf-like results
- Can store file metadata (path, name, size, mtime) alongside FTS index

**Alternatives Considered**:
- **Tantivy** (Rust search engine): Too heavy for this use case, overkill for file names
- **In-memory Vec + strsim**: Doesn't scale to 1M files, RAM usage ~500MB+
- **External Elasticsearch**: Requires separate service, deployment complexity

**Implementation Notes**:
```sql
CREATE VIRTUAL TABLE file_index USING fts5(
    name,           -- filename for fuzzy search
    path,           -- full path
    content=''      -- contentless for metadata-only
);
```
Query pattern: `SELECT * FROM file_index WHERE name MATCH 'budget*' ORDER BY rank`
Then re-rank top 100 results with `jaro_winkler(query, name)`.

## 2. Git Library Choice

**Decision**: Use `git2-rs` crate (libgit2 bindings)

**Rationale**:
- Pure Rust API, no subprocess overhead
- Full git functionality (init, add, commit, log, checkout)
- Battle-tested (used by Cargo itself)
- Can operate on any directory without affecting user's global git config
- Supports bare repositories for minimal footprint

**Alternatives Considered**:
- **Subprocess git**: Requires git installed, harder to parse output, shell injection risk
- **gitoxide (gix)**: Newer, less mature, API still evolving
- **Custom diff/patch**: Reinventing the wheel

**Implementation Notes**:
- Initialize `.little-helper/versions/` repo per watched directory
- Auto-commit on file save with descriptive message
- Show history via `git log --oneline` equivalent
- Restore via `git checkout <sha> -- <file>`
- Never expose git terminology to user ("version" not "commit")

## 3. Playwright Integration

**Decision**: Use Playwright MCP server via existing MCP infrastructure

**Rationale**:
- Little Helper already has MCP client infrastructure (chrome-devtools MCP)
- Playwright MCP server provides standardized tool interface
- Avoids direct subprocess management complexity
- Can reuse authentication/session from browser context
- Community-maintained MCP servers available

**Alternatives Considered**:
- **Direct subprocess**: More control but more code to maintain
- **Puppeteer**: JavaScript-only, would need Node.js subprocess
- **Headless Chrome CDP directly**: Lower level, more complex

**Implementation Notes**:
- Install: `npx @anthropic/mcp-playwright`
- Configure in MCP settings alongside chrome-devtools
- Call via existing MCP tool invocation in agent_host
- Skill wrapper translates natural language → MCP tool calls

## 4. Chrome DevTools MCP

**Decision**: Leverage existing chrome-devtools MCP already in use

**Rationale**:
- Already installed and working (per conversation context)
- Provides network inspection, DOM queries, console access
- Used for Fix mode browser debugging
- Can share browser context with Playwright for Research

**Implementation Notes**:
- Skills call through existing MCP infrastructure
- `browser_debug` skill wraps DevTools MCP tools
- Can inspect network waterfall, console errors, DOM state
- Used for both Fix mode troubleshooting and Research evidence capture

## 5. File Picker + Drag-Drop

**Decision**: Use `rfd` (Rust File Dialogs) for picker, egui native for drag-drop

**Rationale**:
- `rfd` already in Cargo.toml, provides native OS file dialogs
- egui has built-in drag-drop support via `ctx.input().raw.dropped_files`
- Combines platform-native UX with in-app convenience
- Can accept both files and images for context

**Alternatives Considered**:
- **Pure egui file browser**: Less native feel, more code to write
- **Tauri/WebView**: Would require architecture change

**Implementation Notes**:
```rust
// File picker (rfd)
if ui.button("Add File").clicked() {
    if let Some(path) = rfd::FileDialog::new().pick_file() {
        context.add_file(path);
    }
}

// Drag-drop (egui native)
for file in &ctx.input(|i| i.raw.dropped_files.clone()) {
    if let Some(path) = &file.path {
        context.add_file(path.clone());
    }
}
```

## 6. Password Dialog

**Decision**: Use egui `TextEdit` with password masking + secure memory handling

**Rationale**:
- egui's TextEdit supports `password(true)` for masked input
- Can create modal dialog using egui's `Window` with `collapsible(false)`
- Use `zeroize` crate to clear password from memory after use
- Integrates with existing `execute_with_sudo` in executor.rs

**Alternatives Considered**:
- **System keychain**: More secure but OS-specific, complex integration
- **External auth dialog (zenity/kdialog)**: Less integrated UX
- **Terminal prompt**: User explicitly doesn't want this

**Implementation Notes**:
```rust
// In modals/password_dialog.rs
pub struct PasswordDialog {
    password: String,
    command: String,
    on_submit: Option<Box<dyn FnOnce(String)>>,
}

impl PasswordDialog {
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        egui::Window::new("Authentication Required")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Command requires elevated privileges:\n{}", self.command));
                ui.add(egui::TextEdit::singleline(&mut self.password).password(true));
                if ui.button("Authenticate").clicked() {
                    // Submit and clear
                }
            });
    }
}
```

## 7. Skill Execution Architecture

**Decision**: Trait-based skill system with async execution and permission checking

**Rationale**:
- Rust traits provide compile-time guarantees
- Async execution prevents UI blocking
- Permission model enforced before skill invocation
- Detailed logging built into executor

**Implementation Pattern**:
```rust
// In shared/skill.rs
#[async_trait]
pub trait Skill: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn permission_level(&self) -> PermissionLevel;
    fn modes(&self) -> &[Mode];

    async fn execute(&self, input: SkillInput, ctx: &SkillContext) -> Result<SkillOutput>;
}

pub enum PermissionLevel {
    Safe,      // Auto-approved
    Sensitive, // Per-session confirmation required
}

// In agent_host/skills/mod.rs
pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
    session_approvals: HashSet<String>,
}
```

## 8. External Tool Integration Pattern

**Decision**: Provider trait with health check, graceful fallback, and setup guidance

**Rationale**:
- Consistent interface across Canva, Gemini, Playwright, spec-kit
- Health checks detect availability before use
- Fallback with setup guidance per FR-006a
- Easy to add new integrations

**Implementation Pattern**:
```rust
// In providers/mod.rs
#[async_trait]
pub trait ExternalProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn health_check(&self) -> ProviderStatus;
    fn setup_instructions(&self) -> &str;
}

pub enum ProviderStatus {
    Available,
    Unavailable { reason: String },
    NeedsSetup { instructions: String },
}
```

## Summary

All research items resolved. Key decisions:
- **SQLite FTS5 + strsim** for fuzzy file search
- **git2-rs** for hidden version control
- **MCP servers** for Playwright and DevTools integration
- **rfd + egui native** for file input
- **egui modal** for password dialog
- **Trait-based** skill and provider architecture

Ready for Phase 1: data-model.md and contracts/.
