# Implementation Plan: Interactive Preview Companion

**Branch**: `001-preview-window-behaviour` | **Date**: 2026-01-04 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-preview-window-behaviour/spec.md`

## Summary

Transform the preview panel from a passive file viewer into an interactive companion that:
- Displays contextual content (web search previews, file previews, mode introductions)
- Supports zoom/scroll/fullscreen interaction
- Shows personality through ASCII art states
- Maintains persistent conversation memory with agent context continuity
- Provides onboarding flow for terminal permissions and dependency setup

## Technical Context

**Language/Version**: Rust 2021 edition (1.75+)
**Primary Dependencies**: egui 0.27, eframe 0.27, tokio, serde, reqwest
**Storage**: Filesystem JSON (directories::ProjectDirs - user config dir)
**Testing**: cargo test (unit), manual UI testing
**Target Platform**: Cross-platform desktop (macOS, Windows, Linux)
**Project Type**: Single Rust workspace with multiple crates
**Performance Goals**: Mode switch < 500ms, scroll 100+ messages without delay
**Constraints**: Local-first (no cloud), offline-capable for cached content
**Scale/Scope**: Single user, 5 modes, unlimited conversation history

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**Note**: Constitution is not yet configured for this project. Proceeding with standard best practices:

| Principle | Status | Notes |
|-----------|--------|-------|
| Library-First | PASS | Using existing crate structure (viewers, services, etc.) |
| Test Coverage | DEFERRED | Manual UI testing for now; unit tests for data layer |
| Simplicity | PASS | Building on existing architecture, not over-engineering |

## Project Structure

### Documentation (this feature)

```text
specs/001-preview-window-behaviour/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (internal APIs)
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
crates/
├── app/
│   └── src/
│       ├── main.rs              # Main app, mode switching, layout
│       ├── sessions.rs          # Session/conversation management (exists)
│       ├── context.rs           # App context (exists)
│       ├── secrets.rs           # Credentials (exists)
│       ├── preview_panel.rs     # NEW: Preview panel component
│       ├── onboarding.rs        # NEW: Onboarding flow
│       └── ascii_art.rs         # NEW: ASCII art states
├── agent_host/
│   └── src/
│       ├── lib.rs               # Agent hosting (exists)
│       ├── executor.rs          # Command execution (exists)
│       └── prompts.rs           # NEW: Mode-specific system prompts
├── providers/                   # AI providers (exists, unchanged)
├── services/
│   └── src/
│       ├── web_preview.rs       # NEW: Website screenshot/preview service
│       └── ...                  # Existing services
├── shared/
│   └── src/
│       ├── lib.rs               # Shared types (exists)
│       └── preview_types.rs     # NEW: Preview content types
└── viewers/
    └── src/
        ├── lib.rs               # Viewer trait (exists)
        ├── image_viewer.rs      # Add zoom/scroll (modify)
        ├── text_viewer.rs       # Add zoom/scroll (modify)
        └── ...                  # Other viewers (modify for zoom/scroll)
```

**Structure Decision**: Extend existing crate structure. New functionality added as new modules within existing crates to maintain cohesion.

## Complexity Tracking

No constitution violations to justify.

## Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                         App (eframe)                        │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────────────┐  ┌─────────────┐ │
│  │  Mode Tabs  │  │     Chat Panel      │  │   Preview   │ │
│  │  (5 modes)  │  │  - Message list     │  │   Panel     │ │
│  │             │  │  - Input            │  │  - Content  │ │
│  │  Find       │  │  - Session picker   │  │  - Zoom     │ │
│  │  Fix        │  │                     │  │  - Controls │ │
│  │  Research   │──│  SessionManager     │──│             │ │
│  │  Data       │  │  (sessions.rs)      │  │  Viewers    │ │
│  │  Content    │  │                     │  │  (crate)    │ │
│  └─────────────┘  └─────────────────────┘  └─────────────┘ │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    Agent Host                           ││
│  │  - Mode prompts (personality, tools)                    ││
│  │  - Command executor (existing)                          ││
│  │  - Web search (existing)                                ││
│  └─────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    Providers                            ││
│  │  Ollama | OpenAI | Anthropic | Gemini                   ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Key Integration Points

1. **Preview Panel ↔ Agent Host**: Agent outputs `<preview>` tags that update panel content
2. **Preview Panel ↔ Viewers**: Preview panel uses viewer crate for file rendering
3. **Mode Tabs ↔ SessionManager**: Mode switch triggers session restore/create
4. **Mode Tabs ↔ Agent Host**: Mode switch loads appropriate system prompt
5. **Chat Panel ↔ SessionManager**: Messages auto-saved on send/receive

## Phase Summary

| Phase | Output | Key Deliverables |
|-------|--------|------------------|
| 0 | research.md | Technology decisions, patterns research |
| 1 | data-model.md, contracts/ | Data structures, internal APIs |
| 2 | tasks.md | Implementation tasks (/speckit.tasks) |
