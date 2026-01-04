# Implementation Tasks: Interactive Preview Companion

**Feature**: 001-preview-window-behaviour
**Generated**: 2026-01-04
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

## Task Conventions

- `[TaskID]` - Unique identifier (e.g., T001)
- `[P?]` - Priority (P1/P2/P3)
- `[Story?]` - Parent user story (S1-S7)
- File paths are relative to repository root

---

## Phase 0: Setup

- [x] [T001] [P1] [Setup] Create feature branch `001-preview-window-behaviour` from main
- [x] [T002] [P1] [Setup] Create `crates/shared/src/preview_types.rs` with PreviewContent and related types per `data-model.md`
- [x] [T003] [P1] [Setup] Create `crates/app/src/preview_panel.rs` with PreviewPanel struct skeleton per `contracts/internal-apis.md`
- [x] [T004] [P1] [Setup] Create `crates/app/src/ascii_art.rs` with AsciiState enum and placeholder art
- [x] [T005] [P1] [Setup] Create `crates/agent_host/src/prompts.rs` with ModePrompt structure
- [x] [T006] [P1] [Setup] Create `crates/app/src/onboarding.rs` with OnboardingFlow skeleton

---

## Phase 1: Foundational Infrastructure

### Preview Types & State (Shared Crate)

- [x] [T010] [P1] [Foundation] Implement `PreviewContent` enum with File, Web, Image, Ascii, ModeIntro, Error variants in `crates/shared/src/preview_types.rs`
- [x] [T011] [P1] [Foundation] Implement `ImageSource` enum (File, Url, Bytes) in `crates/shared/src/preview_types.rs`
- [x] [T012] [P1] [Foundation] Implement `ParsedPreviewTag` struct and `parse_preview_tag()` function per `contracts/preview-protocol.md`
- [x] [T013] [P1] [Foundation] Implement `strip_preview_tags()` function to clean agent responses for display
- [x] [T014] [P1] [Foundation] Add `PreviewReference` struct to `ChatMessage` in `crates/app/src/sessions.rs`
- [x] [T015] [P1] [Foundation] Export preview_types module from `crates/shared/src/lib.rs`

### Viewer Enhancements (Viewers Crate)

- [x] [T020] [P1] [Foundation] Define `Zoomable` trait with set_zoom, zoom, reset_zoom, zoom_in, zoom_out in `crates/viewers/src/lib.rs`
- [x] [T021] [P1] [Foundation] Implement Zoomable for ImageViewer in `crates/viewers/src/image_viewer.rs`
- [x] [T022] [P1] [Foundation] Implement Zoomable for TextViewer in `crates/viewers/src/text_viewer.rs`
- [x] [T023] [P1] [Foundation] Add zoom state (f32, 0.25-4.0 clamped) to all viewers
- [x] [T024] [P1] [Foundation] Implement handle_input() for Ctrl+scroll zoom in viewers

### Preview Panel Core (App Crate)

- [x] [T030] [P1] [Foundation] Implement `PreviewState` struct with visible, content, zoom, scroll_offset, fullscreen fields in `crates/app/src/preview_panel.rs`
- [x] [T031] [P1] [Foundation] Implement `PreviewPanel::new()` and Default trait
- [x] [T032] [P1] [Foundation] Implement `PreviewPanel::ui()` for basic panel rendering with egui
- [x] [T033] [P1] [Foundation] Implement `PreviewPanel::show_content()` to display PreviewContent
- [x] [T034] [P1] [Foundation] Implement `PreviewPanel::hide()` and `toggle()` methods
- [x] [T035] [P1] [Foundation] Implement `PreviewPanel::is_visible()` and `is_fullscreen()` getters
- [x] [T036] [P1] [Foundation] Integrate PreviewPanel into main.rs layout alongside chat panel

---

## Phase 2: User Story Implementation (P1 Stories)

### Story 1: Mode Introduction & Specialized Agents

> When user switches mode tabs, show mode introduction in preview and load specialized agent.

- [ ] [T100] [P1] [S1] Create mode prompt text files: `crates/agent_host/src/prompts/find.txt`, `fix.txt`, `research.txt`, `data.txt`, `content.txt`
- [ ] [T101] [P1] [S1] Implement `get_mode_prompt(mode: ChatMode) -> ModePrompt` in `crates/agent_host/src/prompts.rs`
- [ ] [T102] [P1] [S1] Implement `get_system_prompt(mode, user_name, memory_summary, permissions) -> String` in `crates/agent_host/src/prompts.rs`
- [ ] [T103] [P1] [S1] Define ModePrompt struct with mode, name, personality, expertise, example_questions, tools_description, tone
- [ ] [T104] [P1] [S1] Create mode introduction content for PreviewContent::ModeIntro variant
- [ ] [T105] [P1] [S1] Implement `PreviewPanel::show_mode_intro(mode: ChatMode)` method
- [ ] [T106] [P1] [S1] Hook mode tab click to call show_mode_intro() in `crates/app/src/main.rs`
- [ ] [T107] [P1] [S1] Hook mode tab click to update agent system prompt via agent_host
- [ ] [T108] [P1] [S1] Verify each mode shows distinct introduction within 500ms (SC-001)
- [ ] [T109] [P1] [S1] Verify agents respond with mode-appropriate personality (SC-002)

### Story 2: Persistent Conversation Memory

> Users can scroll back through history; agent remembers previous conversations.

- [ ] [T110] [P1] [S2] Verify auto-save on message send/receive in `crates/app/src/sessions.rs` (FR-013a)
- [ ] [T111] [P1] [S2] Implement `SessionManager::get_context_messages(mode, max_messages)` for AI context window
- [ ] [T112] [P1] [S2] Implement `SessionManager::get_or_load(mode, id)` for lazy loading sessions from disk
- [ ] [T113] [P1] [S2] Implement `SessionManager::reload(mode)` to force reload from disk
- [ ] [T114] [P1] [S2] Implement lazy loading for conversation scrollback (load older messages on scroll)
- [ ] [T115] [P1] [S2] Include memory summary in system prompt via `get_memory_summary()`
- [ ] [T116] [P1] [S2] Add "New Thread" button to create fresh conversation within mode (FR-017)
- [ ] [T117] [P1] [S2] Add conversation thread browser/picker UI (FR-018)
- [ ] [T118] [P1] [S2] Add "Clear History" option for mode (FR-019)
- [ ] [T119] [P1] [S2] Verify conversation restored after app restart (SC-003)
- [ ] [T120] [P1] [S2] Verify 100+ message scrollback without noticeable delay (SC-004)
- [ ] [T121] [P1] [S2] Verify agent references previous context in follow-up conversations (SC-005)

### Story 3: Contextual Preview During Research

> Preview shows web search sources, screenshots, key images during research.

- [ ] [T130] [P1] [S3] Create `crates/services/src/web_preview.rs` module
- [ ] [T131] [P1] [S3] Implement website screenshot capture using wkhtmltoimage (if available)
- [ ] [T132] [P1] [S3] Implement Open Graph metadata extraction fallback (title, description, og:image)
- [ ] [T133] [P1] [S3] Implement text-only fallback (title + snippet + URL)
- [ ] [T134] [P1] [S3] Implement screenshot caching to avoid repeated captures
- [ ] [T135] [P1] [S3] Integrate preview tag parsing into response handler (parse `<preview>` tags)
- [ ] [T136] [P1] [S3] Update PreviewPanel when preview tag detected in agent response
- [ ] [T137] [P1] [S3] Display source URL in preview header (FR-026)
- [ ] [T138] [P1] [S3] Handle web preview failures gracefully (show placeholder with URL)
- [ ] [T139] [P1] [S3] Verify web source previews display for 80% of research queries (SC-006)

### Story 4: File Preview with Quick Actions

> Users can interact with file previews: zoom, scroll, fullscreen, open in app, reveal in folder.

- [ ] [T140] [P1] [S4] Implement `PreviewAction` enum (OpenInApp, RevealInFolder, OpenInBrowser, CopyPath, CopyUrl, Close, ZoomIn, ZoomOut, ZoomReset, Fullscreen)
- [ ] [T141] [P1] [S4] Implement `PreviewPanel::available_actions()` based on current content type
- [ ] [T142] [P1] [S4] Implement `PreviewPanel::execute_action(action)` method
- [ ] [T143] [P1] [S4] Implement "Open in App" action using `open::that(path)` (FR-027)
- [ ] [T144] [P1] [S4] Implement "Reveal in Folder" action for Finder/Explorer (FR-028)
- [ ] [T145] [P1] [S4] Implement "Open in Browser" action for web URLs (FR-029)
- [ ] [T146] [P1] [S4] Implement zoom controls UI (buttons for +/-/reset) (FR-030)
- [ ] [T147] [P1] [S4] Implement Ctrl+scroll wheel zoom handling (FR-030)
- [ ] [T148] [P1] [S4] Implement scroll/pan for zoomed content using egui::ScrollArea (FR-031)
- [ ] [T149] [P1] [S4] Implement `PreviewPanel::toggle_fullscreen()` method
- [ ] [T150] [P1] [S4] Implement `PreviewPanel::fullscreen_ui()` for overlay rendering (FR-032)
- [ ] [T151] [P1] [S4] Add close button and Escape key handler for fullscreen exit (FR-033)
- [ ] [T152] [P1] [S4] Preserve zoom/scroll state while viewing same content (FR-034)
- [ ] [T153] [P1] [S4] Verify open file in native app within 2 clicks (SC-007)
- [ ] [T154] [P1] [S4] Verify reveal in Finder/Explorer within 2 clicks (SC-008)
- [ ] [T155] [P1] [S4] Verify zoom range 25% to 400% (SC-009)
- [ ] [T156] [P1] [S4] Verify fullscreen enter/exit in 1 click each (SC-010)
- [ ] [T157] [P1] [S4] Verify controls are discoverable (90% success without instruction - SC-011)

---

## Phase 3: User Story Implementation (P2 Stories)

### Story 5: Friendly Personality with ASCII Art

> ASCII art adds personality during thinking, success, error, and welcome states.

- [ ] [T200] [P2] [S5] Create welcome ASCII art in `crates/app/src/art/welcome.txt`
- [ ] [T201] [P2] [S5] Create thinking ASCII art in `crates/app/src/art/thinking.txt`
- [ ] [T202] [P2] [S5] Create success ASCII art in `crates/app/src/art/success.txt`
- [ ] [T203] [P2] [S5] Create error ASCII art in `crates/app/src/art/error.txt`
- [ ] [T204] [P2] [S5] Create mode-specific ASCII art (optional mascots) for each mode intro
- [ ] [T205] [P2] [S5] Implement `get_ascii_art(state: AsciiState) -> &'static str` in `crates/app/src/ascii_art.rs`
- [ ] [T206] [P2] [S5] Implement `PreviewPanel::show_ascii(state: AsciiState)` method
- [ ] [T207] [P2] [S5] Hook thinking state to show thinking art during AI processing
- [ ] [T208] [P2] [S5] Hook success/error states to task completion events
- [ ] [T209] [P2] [S5] Implement theme-aware ASCII rendering (adapt colors for light/dark)
- [ ] [T210] [P2] [S5] Show welcome art on empty preview panel (FR-035)
- [ ] [T211] [P2] [S5] Verify ASCII art appears in at least 3 distinct states (SC-012)

### Story 6: Preview Persists During Conversation

> Preview remains stable while chatting; only changes on explicit new content.

- [ ] [T220] [P2] [S6] Verify preview does not flicker on user message send (FR-021)
- [ ] [T221] [P2] [S6] Verify preview does not flicker on AI response receive (FR-022)
- [ ] [T222] [P2] [S6] Implement explicit content replacement when AI specifies new preview (FR-023)
- [ ] [T223] [P2] [S6] Add visual indicator when preview content updates (subtle transition)

---

## Phase 4: User Story Implementation (P3 Stories)

### Story 7: Explicit Preview Control

> User can close/reopen preview panel; panel re-opens automatically for new content.

- [ ] [T300] [P3] [S7] Add close button (✕) to preview panel header (FR-040)
- [ ] [T301] [P3] [S7] Implement preview panel collapse with chat panel expansion
- [ ] [T302] [P3] [S7] Implement auto-reopen when new preview content arrives (FR-041)
- [ ] [T303] [P3] [S7] Add "Show Preview" toggle in UI when panel is hidden (FR-042)
- [ ] [T304] [P3] [S7] Display content source (filename/URL/mode) in preview header (FR-042)

---

## Phase 5: Onboarding & Permissions

- [ ] [T400] [P1] [Onboarding] Implement `OnboardingFlow::new()` and step management in `crates/app/src/onboarding.rs`
- [ ] [T401] [P1] [Onboarding] Implement `OnboardingFlow::is_needed(settings)` check for first-run detection
- [ ] [T402] [P1] [Onboarding] Implement Welcome step UI with app introduction
- [ ] [T403] [P1] [Onboarding] Implement Terminal Permission step with clear explanation and consent button (FR-046)
- [ ] [T404] [P1] [Onboarding] Implement Dependency Check step - detect wkhtmltoimage, curl, WSL (Windows) (FR-047)
- [ ] [T405] [P1] [Onboarding] Implement `check_dependency(name)` and `check_all_dependencies()` functions
- [ ] [T406] [P1] [Onboarding] Implement `install_dependency(name)` for auto-installation where possible
- [ ] [T407] [P1] [Onboarding] Implement Verification step - run test command to confirm terminal works (FR-048)
- [ ] [T408] [P1] [Onboarding] Implement Complete step with success confirmation
- [ ] [T409] [P1] [Onboarding] Store onboarding_complete and terminal_permission_granted in AppSettings
- [ ] [T410] [P1] [Onboarding] Update agent system prompts to include terminal capability info (FR-049)
- [ ] [T411] [P1] [Onboarding] Add permissions display in settings screen (FR-050)
- [ ] [T412] [P1] [Onboarding] Handle dependency install failures gracefully with manual instructions
- [ ] [T413] [P1] [Onboarding] Handle permission denial - enable limited mode without terminal
- [ ] [T414] [P1] [Onboarding] Verify 95% first-time users complete onboarding successfully (SC-013)
- [ ] [T415] [P1] [Onboarding] Verify agents execute terminal commands when permission granted (SC-014)

---

## Phase 6: Accessibility

- [ ] [T500] [P2] [A11y] Add keyboard navigation (Tab) for all interactive elements (FR-043)
- [ ] [T501] [P2] [A11y] Add aria-labels/accessible names for buttons and controls (FR-044)
- [ ] [T502] [P2] [A11y] Implement visible focus states for keyboard navigation (FR-045)
- [ ] [T503] [P2] [A11y] Test keyboard-only navigation flow through preview panel
- [ ] [T504] [P2] [A11y] Test screen reader compatibility (VoiceOver/NVDA)

---

## Phase 7: Polish & Edge Cases

- [ ] [T600] [P3] [Polish] Handle edge case web preview failures (timeout, SSL errors, blocked sites) beyond basic fallback in T138
- [ ] [T601] [P3] [Polish] Enforce zoom min/max limits (0.25 to 4.0)
- [ ] [T602] [P3] [Polish] Handle large files in fullscreen with performance optimization
- [ ] [T603] [P3] [Polish] Handle deleted file detection (show error state)
- [ ] [T604] [P3] [Polish] Handle "open in app" failures gracefully
- [ ] [T605] [P2] [Polish] Preserve unsent input text per mode on mode switch
- [ ] [T606] [P3] [Polish] Implement on-demand loading for very long conversation history (edge cases beyond T114)
- [ ] [T607] [P3] [Polish] Warn user when storage is full, offer to clear old conversations
- [ ] [T608] [P3] [Polish] Add subtle animations/transitions for preview content changes

---

## Phase 8: Version Control & Documentation

### Git Workflow (Commit Checkpoints)

- [ ] [T700] [P1] [Git] Commit after Setup phase: `feat(preview): scaffold preview panel and type structures`
- [ ] [T701] [P1] [Git] Commit after Foundation phase: `feat(preview): implement core preview panel with zoom/scroll`
- [ ] [T702] [P1] [Git] Commit after Story 1: `feat(agents): add mode-specific prompts and introductions`
- [ ] [T703] [P1] [Git] Commit after Story 2: `feat(memory): implement persistent conversation history`
- [ ] [T704] [P1] [Git] Commit after Story 3: `feat(preview): add web search preview with screenshots`
- [ ] [T705] [P1] [Git] Commit after Story 4: `feat(preview): add file actions and fullscreen mode`
- [ ] [T706] [P2] [Git] Commit after Stories 5-6: `feat(preview): add ASCII art states and persistence`
- [ ] [T707] [P3] [Git] Commit after Story 7: `feat(preview): add explicit panel controls`
- [ ] [T708] [P1] [Git] Commit after Onboarding: `feat(onboarding): add terminal permission flow`
- [ ] [T709] [P2] [Git] Commit after Accessibility: `feat(a11y): add keyboard navigation and screen reader support`
- [ ] [T710] [P3] [Git] Commit after Polish: `fix(preview): handle edge cases and add transitions`

### Documentation

- [ ] [T720] [P1] [Docs] Update README.md with new preview panel features
- [ ] [T721] [P1] [Docs] Update CHANGELOG.md with feature summary
- [ ] [T722] [P2] [Docs] Add user guide section for preview panel controls
- [ ] [T723] [P2] [Docs] Document mode personalities and capabilities for users

### Pull Request & Merge

- [ ] [T730] [P1] [PR] Create PR with summary referencing spec.md user stories
- [ ] [T731] [P1] [PR] Ensure all P1 tasks complete before PR review
- [ ] [T732] [P1] [PR] Run `cargo test` and `cargo clippy` - all pass
- [ ] [T733] [P1] [PR] Manual testing checklist from quickstart.md complete
- [ ] [T734] [P2] [PR] Squash or rebase commits for clean history (optional)
- [ ] [T735] [P1] [PR] Merge to main after approval

---

## Task Summary

| Phase | Task Count | Priority |
|-------|------------|----------|
| Setup | 6 | P1 |
| Foundation | 15 | P1 |
| Story 1 (Mode Agents) | 10 | P1 |
| Story 2 (Memory) | 12 | P1 |
| Story 3 (Web Preview) | 10 | P1 |
| Story 4 (File Actions) | 18 | P1 |
| Story 5 (ASCII Art) | 12 | P2 |
| Story 6 (Persistence) | 4 | P2 |
| Story 7 (Control) | 5 | P3 |
| Onboarding | 16 | P1 |
| Accessibility | 5 | P2 |
| Polish | 9 | P2/P3 |
| Git & Docs | 21 | P1/P2/P3 |
| **Total** | **143** | |

### By Priority

| Priority | Tasks | Percentage |
|----------|-------|------------|
| P1 | 101 | 71% |
| P2 | 28 | 19% |
| P3 | 14 | 10% |

---

## Dependency Order

```
Setup (T001-T006)
    │
    ├── [T700] Commit: scaffold
    v
Foundation (T010-T036)
    │
    ├── [T701] Commit: core preview
    │
    ├──> Story 1 (Mode Agents) ──> [T702] Commit ──> Story 5 (ASCII Art) ──> [T706] Commit
    │
    ├──> Story 2 (Memory) ──────> [T703] Commit ──> Story 6 (Persistence)
    │
    ├──> Story 3 (Web Preview) ─> [T704] Commit
    │
    └──> Story 4 (File Actions) ─> [T705] Commit ──> Story 7 (Control) ──> [T707] Commit
              │
              v
        Onboarding (T400-T415)
              │
              ├── [T708] Commit: onboarding
              v
        Accessibility (T500-T504)
              │
              ├── [T709] Commit: a11y
              v
        Polish (T600-T608)
              │
              ├── [T710] Commit: polish
              v
        Documentation (T720-T723)
              │
              v
        PR & Merge (T730-T735)
```

### Commit Convention

Following [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(scope): description     # New feature
fix(scope): description      # Bug fix
docs(scope): description     # Documentation
refactor(scope): description # Code restructure
test(scope): description     # Tests
```

**Scopes**: `preview`, `agents`, `memory`, `onboarding`, `a11y`

---

## Implementation Notes

### Getting Started

1. Create branch: `git checkout -b 001-preview-window-behaviour`
2. Start with Setup tasks (T001-T006) to create file structure
3. Foundation tasks (T010-T036) build the core types and panel
4. Then implement P1 stories in parallel (Stories 1-4 are independent)

### Testing Strategy

- Unit tests for preview tag parsing (T012, T013)
- Unit tests for session persistence (T110-T121)
- Manual UI testing for zoom/scroll/fullscreen behavior
- Integration testing for onboarding flow

### Key Files Reference

| File | Purpose |
|------|---------|
| `crates/shared/src/preview_types.rs` | PreviewContent, ImageSource, ParsedPreviewTag |
| `crates/app/src/preview_panel.rs` | PreviewPanel, PreviewState, PreviewAction |
| `crates/app/src/ascii_art.rs` | AsciiState, get_ascii_art() |
| `crates/app/src/onboarding.rs` | OnboardingFlow, OnboardingStep, DependencyStatus |
| `crates/agent_host/src/prompts.rs` | ModePrompt, get_system_prompt() |
| `crates/services/src/web_preview.rs` | Screenshot capture, OG extraction |
