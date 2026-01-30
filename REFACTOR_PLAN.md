# Little Helper Code Review & Refactoring Plan

**Date:** January 29, 2026  
**Branch:** `refactor/main-rs-modularization`  
**Goal:** Break down the 1400+ line `main.rs` into maintainable modules

---

## Current State

### The Problem
`crates/app/src/main.rs` is **1,394 lines** and contains:
- AppState struct and all state management
- UI rendering for entire application
- AI response handling and parsing
- Command execution coordination
- Settings management
- Theme handling
- Onboarding flow
- Chat interface logic

**Impact:** 
- Violates Single Responsibility Principle
- Hard to navigate and maintain
- Merge conflicts are frequent
- Testing is nearly impossible

---

## Refactoring Strategy

### Phase 1: Extract UI Components (Priority: HIGH)
**Goal:** Move all UI rendering code into dedicated modules

**New Structure:**
```
crates/app/src/
├── main.rs              # Reduced to ~300 lines (entry point only)
├── lib.rs               # Module exports
├── state/
│   ├── mod.rs           # AppState and state management
│   ├── chat.rs          # Chat state and message handling
│   ├── settings.rs      # Settings dialog state
│   └── preview.rs       # Preview panel state
├── ui/
│   ├── mod.rs           # UI coordination
│   ├── chat.rs          # Chat interface rendering
│   ├── sidebar.rs       # Mode sidebar
│   ├── toolbar.rs       # Top toolbar
│   ├── preview.rs       # Preview panel UI
│   ├── onboarding.rs    # First-run UI
│   └── settings.rs      # Settings dialogs
├── handlers/
│   ├── mod.rs           # Handler coordination
│   ├── ai_response.rs   # AI response parsing and handling
│   ├── commands.rs      # Command execution
│   └── preview.rs       # Preview content handling
└── widgets/
    ├── mod.rs           # Widget exports
    ├── drag_drop.rs     # Drag and drop handling
    ├── audit_viewer.rs  # Audit log viewer
    ├── file_picker.rs   # File picker widget
    └── version_history.rs # Version history widget
```

**Tasks:**
- [ ] 1.1 Create `state/` directory and extract AppState
- [ ] 1.2 Create `ui/` directory and extract UI rendering functions
- [ ] 1.3 Create `handlers/` directory and extract AI/command handlers
- [ ] 1.4 Create `widgets/` directory and move existing widgets
- [ ] 1.5 Update `main.rs` to use new modules
- [ ] 1.6 Update `Cargo.toml` if needed

---

### Phase 2: Fix Performance Issues (Priority: MEDIUM)
**Goal:** Address regex compilation and other hot path issues

**Issues to Fix:**
1. **Regex compiled on every AI response** (main.rs lines 959-960)
   - Use `lazy_static!` or `once_cell::Lazy`
   - Compile once at startup

2. **String-based command classification** (executor.rs)
   - Parse command structure properly
   - Check actual binary path, not just string prefix

**Tasks:**
- [ ] 2.1 Add `lazy_static` or `once_cell` dependency
- [ ] 2.2 Create static regexes for AI response parsing
- [ ] 2.3 Improve command classification in executor.rs

---

### Phase 3: Complete TODOs (Priority: MEDIUM)
**Goal:** Implement stubbed functionality

**TODOs Found:**
1. **Clipboard operations** (preview_panel.rs lines 291-296)
   - `CopyPath` action is stubbed
   - `CopyUrl` action is stubbed

2. **Missing viewers** (viewers/Cargo.toml)
   - PDF rendering (commented out)
   - Excel support (commented out)
   - SQLite browser (commented out)

**Tasks:**
- [ ] 3.1 Implement clipboard copy for file paths
- [ ] 3.2 Implement clipboard copy for URLs
- [ ] 3.3 Add `arboard` or similar clipboard crate

---

### Phase 4: Code Quality Improvements (Priority: LOW)
**Goal:** Standardize and clean up

**Issues:**
1. **Inconsistent Cargo.toml metadata**
   - Some use `version.workspace = true`, others `version = "0.1.0"`
   - Standardize all to use workspace inheritance

2. **Hardcoded paths** (context.rs lines 30-32)
   - `~/Projects/MCP-research-content-automation-engine`
   - Make configurable via settings

3. **Web search HTML scraping** (executor.rs lines 743-808)
   - Uses regex on DuckDuckGo HTML
   - Fragile, will break if DDG changes HTML

**Tasks:**
- [ ] 4.1 Standardize all Cargo.toml files
- [ ] 4.2 Make campaign context path configurable
- [ ] 4.3 Consider using proper search API

---

## Progress Tracking

### Completed
- [x] Committed uncommitted changes (security preview types)
- [x] Created feature branch `refactor/main-rs-modularization`
- [x] Created this refactoring plan

### In Progress
- [ ] Phase 1: Extract UI Components

### Pending
- [ ] Phase 2: Fix Performance Issues
- [ ] Phase 3: Complete TODOs
- [ ] Phase 4: Code Quality Improvements

---

## Commit Strategy

**Rule:** Commit after each significant refactor, not at the end

**Commit Message Format:**
```
refactor(app): extract [component] into [module]

- Moved [what] from main.rs to [where]
- Reduced main.rs from [X] to [Y] lines
- No functional changes
```

**Example:**
```
refactor(app): extract AppState into state/ module

- Moved AppState and related types to state/mod.rs
- Reduced main.rs from 1394 to 1100 lines
- No functional changes
```

---

## Testing Strategy

After each phase:
1. `cargo build` - Ensure it compiles
2. `cargo test` - Run existing tests
3. Manual test - Run the app and verify basic functionality
4. Commit and push

---

## Success Criteria

- [ ] main.rs is under 500 lines
- [ ] Each module has a single responsibility
- [ ] All existing tests pass
- [ ] No functional regressions
- [ ] Code is more readable and maintainable

---

## Notes

- **No AI Attribution:** Per project policy, no AI co-author lines in commits
- **Test Copies:** If creating scripts, always test on copies first
- **User Approval:** User must test on localhost before production deploy
