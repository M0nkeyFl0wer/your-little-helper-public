# Implementation Plan: Agent Skills and Tools

**Branch**: `002-agent-skills-tools` | **Date**: 2026-01-04 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/002-agent-skills-tools/spec.md`

## Summary

Transform Little Helper's mode agents (Find, Fix, Research, Data, Content, Build) from personality-only assistants into capable tool-users that can execute real actions. Key features include fzf-like file search with drive indexing, browser automation via Playwright, hidden git-based version control, dashboard builder wizard, spec-kit integration for Build mode, and design tools via Canva MCP and Gemini CLI. Critical constraint: agents MUST NEVER delete files.

## Technical Context

**Language/Version**: Rust 1.75+ (existing codebase)
**Primary Dependencies**:
- GUI: eframe 0.27, egui 0.27
- Async: tokio 1.x
- HTTP: reqwest 0.12
- File ops: walkdir 2, ignore 0.4
- Fuzzy matching: strsim 0.11
- Browser automation: Playwright (via subprocess/MCP)
- Python scripting: Python 3.11+ (subprocess)
- DevTools: chrome-devtools MCP

**Storage**:
- File index: SQLite (new, for fuzzy search)
- Version control: Local git repositories
- Content calendar: JSON/CSV (future: Google Sheets sync)
- Audit logs: JSON files in app data directory

**Testing**: cargo test (unit), integration tests for skills
**Target Platform**: Linux (primary), macOS (secondary)
**Project Type**: Desktop application (Rust workspace with crates)
**Performance Goals**:
- File search: <1 second for 100K-1M files indexed
- Mode switch: <1 second capability outline display
- System diagnostic: <3 seconds
**Constraints**:
- No file deletion (hard block)
- Per-session sensitive skill confirmation
- Detailed skill execution logging
**Scale/Scope**: Medium (100K-1M files in index)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Since the project constitution is not yet customized (still template), applying sensible defaults:

| Gate | Status | Notes |
|------|--------|-------|
| No file deletion | ✅ PASS | Core safety constraint; implemented at skill level |
| Skill permissions | ✅ PASS | Per-session confirmation for sensitive operations |
| Audit logging | ✅ PASS | All file ops and skill executions logged |
| Graceful degradation | ✅ PASS | External integrations fail safely with setup guidance |
| Existing architecture | ✅ PASS | Extends existing crates (services, agent_host) |

## Project Structure

### Documentation (this feature)

```text
specs/002-agent-skills-tools/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── skill-api.md     # Internal skill invocation contract
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
crates/
├── app/                 # GUI application (egui)
│   └── src/
│       ├── main.rs
│       ├── preview_panel.rs    # Mode capability outlines (modify)
│       ├── modals/             # Password dialog (new)
│       └── widgets/            # File picker, drag-drop (new)
├── agent_host/          # Agent execution engine
│   └── src/
│       ├── lib.rs
│       ├── executor.rs         # Skill execution (extend)
│       └── skills/             # NEW: Skill implementations
│           ├── mod.rs
│           ├── find/           # Fuzzy search, drive index
│           ├── fix/            # System diagnostics, browser debug
│           ├── research/       # Web search, Playwright, Python
│           ├── data/           # Parse, validate, dashboard wizard
│           ├── content/        # Calendar, MPA context, design
│           ├── build/          # Spec-kit integration
│           └── common/         # Version control, file ops
├── services/            # Background services
│   └── src/
│       ├── file_search.rs      # Extend with SQLite index
│       ├── file_index.rs       # NEW: Drive indexing service
│       └── version_control.rs  # NEW: Hidden git wrapper
├── shared/              # Common types
│   └── src/
│       ├── skill.rs            # NEW: Skill trait, permission model
│       └── events.rs           # NEW: Skill execution events
└── providers/           # External integrations
    └── src/
        ├── playwright.rs       # NEW: Browser automation
        ├── canva.rs            # NEW: Canva MCP client
        ├── gemini.rs           # NEW: Gemini CLI wrapper
        └── speckit.rs          # NEW: Spec-kit integration

tests/
├── integration/
│   ├── skill_execution_test.rs
│   ├── file_index_test.rs
│   └── version_control_test.rs
└── unit/
    └── (per-crate tests)
```

**Structure Decision**: Extends existing Rust workspace crate structure. New `skills/` module in agent_host contains all skill implementations. Services crate handles background operations (indexing, version control). Providers crate handles external tool integrations.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| SQLite for file index | Sub-second fuzzy search on 1M files | In-memory index exceeds reasonable RAM usage |
| Subprocess for Playwright | Browser automation required for Research | Native Rust browser libs insufficient for complex scraping |
| Subprocess for Python | Data analysis beyond Rust ecosystem | Re-implementing pandas/scipy in Rust not feasible |

## Phase 0: Research Tasks

The following items need research before implementation:

1. **SQLite fuzzy search strategy** - How to implement fzf-like matching efficiently
2. **Git library choice** - git2-rs vs subprocess git for hidden version control
3. **Playwright integration** - MCP server vs direct subprocess invocation
4. **Chrome DevTools MCP** - Integration pattern for browser debugging
5. **File picker + drag-drop** - egui native vs rfd patterns for context input
6. **Password dialog** - Secure input widget in egui for sudo

## Next Steps

1. Generate research.md with findings for all items above
2. Generate data-model.md with entity definitions
3. Generate contracts/ with skill API specifications
4. Generate quickstart.md with development setup
5. Run `/speckit.tasks` to generate implementation tasks
