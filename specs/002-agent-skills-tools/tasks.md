# Tasks: Agent Skills and Tools

**Input**: Design documents from `/specs/002-agent-skills-tools/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/skill-api.md

**Organization**: Tasks grouped by user story for independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1-US13)
- All paths relative to repository root (`/home/flower/Projects/little-helper/`)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add new dependencies and create skill system foundation

- [ ] T001 Add rusqlite with FTS5 to workspace Cargo.toml dependencies
- [ ] T002 Add git2 crate to workspace Cargo.toml dependencies
- [ ] T003 [P] Add zeroize crate for secure password handling in Cargo.toml
- [ ] T004 [P] Create crates/shared/src/skill.rs with Skill trait and PermissionLevel enum
- [ ] T005 [P] Create crates/shared/src/events.rs with SkillExecution and AuditLog types
- [ ] T006 Update crates/shared/src/lib.rs to export skill and events modules
- [ ] T007 Create crates/agent_host/src/skills/mod.rs with SkillRegistry struct
- [ ] T008 [P] Create crates/services/src/file_index.rs with SQLite FTS5 schema
- [ ] T009 [P] Create crates/services/src/version_control.rs with git2-rs wrapper
- [ ] T010 Update crates/services/src/lib.rs to export file_index and version_control modules
- [ ] T011 [P] Create crates/providers/src/mod.rs with ProviderRegistry and ProviderStatus types
- [ ] T012 Create tests/fixtures/ directory with sample files for testing

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core skill infrastructure that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [ ] T013 Implement SafeFileOps in crates/agent_host/src/skills/common/safe_file_ops.rs (NO delete method)
- [ ] T014 Implement AuditLogger in crates/agent_host/src/skills/common/audit.rs with JSON file rotation
- [ ] T015 [P] Implement SkillContext in crates/shared/src/skill.rs with session_approvals and providers
- [ ] T016 [P] Implement permission check flow in crates/agent_host/src/skills/mod.rs (Safe auto-approve, Sensitive per-session)
- [ ] T017 Implement async skill execution wrapper in crates/agent_host/src/executor.rs
- [ ] T018 [P] Create crates/app/src/modals/mod.rs with modal infrastructure
- [ ] T019 Implement PasswordDialog in crates/app/src/modals/password_dialog.rs (egui masked input + zeroize)
- [ ] T020 [P] Create crates/app/src/widgets/mod.rs with widget infrastructure
- [ ] T021 Implement FilePickerWidget in crates/app/src/widgets/file_picker.rs (rfd integration)
- [ ] T022 Implement DragDropHandler in crates/app/src/widgets/drag_drop.rs (egui dropped_files)
- [ ] T023 Create crates/agent_host/src/skills/common/mod.rs to export safe_file_ops and audit

**Checkpoint**: Foundation ready - skill system can now invoke skills with permissions and logging

---

## Phase 3: User Story 1 - Fuzzy File Finder with Drive Index (Priority: P1) 🎯 MVP

**Goal**: fzf-like file search across all drives with sub-second results on 100K-1M files

**Independent Test**: Type "budget 202" in Find mode and verify fuzzy-matched files appear within 1 second

### Implementation for User Story 1

- [ ] T024 [P] [US1] Implement FileIndex entity with SQLite schema in crates/services/src/file_index.rs
- [ ] T025 [P] [US1] Implement FTS5 trigram tokenizer setup in crates/services/src/file_index.rs
- [ ] T026 [US1] Implement drive scanning with walkdir in crates/services/src/file_index.rs scan_drive()
- [ ] T027 [US1] Implement fuzzy_search() with FTS5 MATCH + strsim re-ranking in crates/services/src/file_index.rs
- [ ] T028 [P] [US1] Create FuzzyFileSearch skill in crates/agent_host/src/skills/find/fuzzy_search.rs
- [ ] T029 [P] [US1] Create DriveIndex skill in crates/agent_host/src/skills/find/drive_index.rs
- [ ] T030 [P] [US1] Create FilePreview skill in crates/agent_host/src/skills/find/file_preview.rs
- [ ] T031 [US1] Create crates/agent_host/src/skills/find/mod.rs to export Find mode skills
- [ ] T032 [US1] Register Find mode skills in crates/agent_host/src/skills/mod.rs SkillRegistry
- [ ] T033 [US1] Integrate fuzzy search results display in crates/app/src/preview_panel.rs

**Checkpoint**: Find mode can search files with fuzzy matching - verify <1 second on indexed drives

---

## Phase 4: User Story 2 - Deep Research with Clarifying Questions (Priority: P1)

**Goal**: Research agent asks clarifying questions before starting, produces citations

**Independent Test**: Ask "research marine protected areas" and verify clarifying questions appear before research starts

### Implementation for User Story 2

- [ ] T034 [P] [US2] Create ResearchPlan entity in crates/shared/src/research.rs
- [ ] T035 [US2] Implement ResearchClarify skill in crates/agent_host/src/skills/research/clarify.rs
- [ ] T036 [P] [US2] Implement WebSearch skill in crates/agent_host/src/skills/research/web_search.rs
- [ ] T037 [P] [US2] Implement WebFetch skill in crates/agent_host/src/skills/research/web_fetch.rs
- [ ] T038 [P] [US2] Implement CitationValidate skill in crates/agent_host/src/skills/research/citation.rs
- [ ] T039 [US2] Create MPAContext loader in crates/agent_host/src/skills/research/mpa_context.rs
- [ ] T040 [US2] Create crates/agent_host/src/skills/research/mod.rs to export Research skills
- [ ] T041 [US2] Register Research mode skills in SkillRegistry
- [ ] T042 [US2] Implement citation display with clickable links in crates/app/src/preview_panel.rs

**Checkpoint**: Research mode asks clarifying questions and produces cited results

---

## Phase 5: User Story 3 - Advanced Research with Browser Automation (Priority: P1)

**Goal**: Research can use Playwright for dynamic sites and Python for analysis

**Independent Test**: Ask agent to extract data from a JavaScript-heavy site and verify dynamic content is captured

### Implementation for User Story 3

- [ ] T043 [P] [US3] Create Playwright provider in crates/providers/src/playwright.rs with MCP integration
- [ ] T044 [P] [US3] Create Python executor in crates/providers/src/python.rs with subprocess
- [ ] T045 [US3] Implement BrowserAutomate skill in crates/agent_host/src/skills/research/browser_automate.rs
- [ ] T046 [US3] Implement PythonAnalysis skill in crates/agent_host/src/skills/research/python_analysis.rs
- [ ] T047 [US3] Add browser_automate and python_analysis to Research mod.rs
- [ ] T048 [US3] Implement provider health check and setup guidance in crates/providers/src/mod.rs

**Checkpoint**: Research mode can scrape dynamic sites and run Python analysis

---

## Phase 6: User Story 4 - Tech Support with System Diagnostics (Priority: P1)

**Goal**: Fix mode offers diagnostics, shows htop-like system info, helps troubleshoot

**Independent Test**: Switch to Fix mode and verify diagnostic offer appears with system status in preview

### Implementation for User Story 4

- [ ] T049 [P] [US4] Implement SystemDiagnostic skill in crates/agent_host/src/skills/fix/diagnostic.rs
- [ ] T050 [P] [US4] Implement SystemInfoDisplay skill in crates/agent_host/src/skills/fix/system_info.rs (CPU, RAM, disk)
- [ ] T051 [P] [US4] Implement NetworkTroubleshoot skill in crates/agent_host/src/skills/fix/network.rs (ping, DNS, traceroute)
- [ ] T052 [US4] Implement BrowserDebug skill in crates/agent_host/src/skills/fix/browser_debug.rs (DevTools MCP)
- [ ] T053 [P] [US4] Implement LogAnalysis skill in crates/agent_host/src/skills/fix/log_analysis.rs
- [ ] T054 [US4] Create crates/agent_host/src/skills/fix/mod.rs to export Fix skills
- [ ] T055 [US4] Register Fix mode skills in SkillRegistry
- [ ] T056 [US4] Create htop-like display widget in crates/app/src/widgets/system_monitor.rs
- [ ] T057 [US4] Integrate system monitor in preview panel for Fix mode

**Checkpoint**: Fix mode shows system status and offers diagnostics within 3 seconds

---

## Phase 7: User Story 5 - Content Creation with Calendar and MPA Context (Priority: P1)

**Goal**: Content mode shows calendar in preview, uses MPA training data as context

**Independent Test**: Switch to Content mode, verify calendar appears, click to open as spreadsheet

### Implementation for User Story 5

- [ ] T058 [P] [US5] Create ContentCalendar entity in crates/shared/src/calendar.rs
- [ ] T059 [P] [US5] Implement ContentCalendar skill in crates/agent_host/src/skills/content/calendar.rs
- [ ] T060 [US5] Implement CalendarSpreadsheet skill in crates/agent_host/src/skills/content/spreadsheet.rs
- [ ] T061 [P] [US5] Create MPAContext skill for Content mode in crates/agent_host/src/skills/content/mpa_context.rs
- [ ] T062 [US5] Create crates/agent_host/src/skills/content/mod.rs to export Content skills
- [ ] T063 [US5] Create calendar preview widget in crates/app/src/widgets/calendar_preview.rs
- [ ] T064 [US5] Create spreadsheet viewer widget in crates/app/src/widgets/spreadsheet.rs
- [ ] T065 [US5] Integrate calendar preview in Content mode preview panel

**Checkpoint**: Content mode shows calendar, opens as spreadsheet, uses MPA context

---

## Phase 8: User Story 6 - Hidden Version Control with Easy Revert (Priority: P1)

**Goal**: All file changes versioned with git2-rs, user can revert via natural language

**Independent Test**: Edit a file, ask "show earlier versions", verify list appears, restore a version

### Implementation for User Story 6

- [ ] T066 [P] [US6] Create FileVersion entity in crates/shared/src/version.rs
- [ ] T067 [US6] Implement VersionControlService.save_version() in crates/services/src/version_control.rs
- [ ] T068 [US6] Implement VersionControlService.list_versions() with user-friendly descriptions
- [ ] T069 [US6] Implement VersionControlService.restore_version() preserving history
- [ ] T070 [P] [US6] Create VersionHistory skill in crates/agent_host/src/skills/common/version_history.rs
- [ ] T071 [US6] Create VersionRestore skill in crates/agent_host/src/skills/common/version_restore.rs
- [ ] T072 [US6] Register version skills for All modes in SkillRegistry
- [ ] T073 [US6] Create version history display widget in crates/app/src/widgets/version_history.rs

**Checkpoint**: Files are auto-versioned, users can view/restore versions without git terminology

---

## Phase 9: User Story 7 - Safe File Organization (No Deletion) (Priority: P1)

**Goal**: Agent NEVER deletes files, offers archive/organize alternatives

**Independent Test**: Ask agent to delete a file, verify refusal with organization alternative offered

### Implementation for User Story 7

- [ ] T074 [US7] Implement FileOrganize skill in crates/agent_host/src/skills/find/file_organize.rs
- [ ] T075 [US7] Add archive_file() and move_file() to SafeFileOps (verify no delete method exists)
- [ ] T076 [US7] Implement deletion request detection and refusal in FileOrganize skill
- [ ] T077 [US7] Add file operation logging to AuditLogger for all moves/archives
- [ ] T078 [US7] Create audit log viewer in crates/app/src/screens/settings.rs (primary user only)

**Checkpoint**: Deletion requests are refused, alternatives offered, all file ops logged

---

## Phase 10: User Story 8 - Data Analysis with Validation (Priority: P1)

**Goal**: Data analysis with clickable source references and validation

**Independent Test**: Provide CSV, ask for analysis, click any statistic to see source rows

### Implementation for User Story 8

- [ ] T079 [P] [US8] Implement FileRead skill in crates/agent_host/src/skills/data/file_read.rs
- [ ] T080 [P] [US8] Implement ParseData skill in crates/agent_host/src/skills/data/parse_data.rs (CSV, JSON, Excel)
- [ ] T081 [US8] Implement AnalyzeWithReferences skill in crates/agent_host/src/skills/data/analyze.rs
- [ ] T082 [P] [US8] Implement DataValidate skill in crates/agent_host/src/skills/data/validate.rs
- [ ] T083 [P] [US8] Implement GenerateChart skill in crates/agent_host/src/skills/data/chart.rs
- [ ] T084 [US8] Create crates/agent_host/src/skills/data/mod.rs to export Data skills
- [ ] T085 [US8] Register Data mode skills in SkillRegistry
- [ ] T086 [US8] Create clickable reference display in crates/app/src/widgets/data_reference.rs

**Checkpoint**: Data analysis shows validated results with clickable source references

---

## Phase 11: User Story 9 - Dashboard Builder Wizard (Priority: P1)

**Goal**: 5-step wizard for dashboard creation with checkpoints

**Independent Test**: Say "build a dashboard", walk through 5 steps to produce working dashboard

### Implementation for User Story 9

- [ ] T087 [P] [US9] Create DashboardProject entity in crates/shared/src/dashboard.rs
- [ ] T088 [US9] Implement DashboardWizard skill in crates/agent_host/src/skills/data/wizard.rs
- [ ] T089 [US9] Implement DashboardAnalyze skill (Step 1) in crates/agent_host/src/skills/data/dashboard_analyze.rs
- [ ] T090 [US9] Implement DashboardConfig skill (Step 2) in crates/agent_host/src/skills/data/dashboard_config.rs
- [ ] T091 [US9] Implement DashboardValidate skill (Step 4) in crates/agent_host/src/skills/data/dashboard_validate.rs
- [ ] T092 [US9] Implement DashboardQA skill (Step 5) in crates/agent_host/src/skills/data/dashboard_qa.rs
- [ ] T093 [US9] Create wizard progress widget in crates/app/src/widgets/wizard_progress.rs
- [ ] T094 [US9] Add dashboard skills to Data mod.rs

**Checkpoint**: Dashboard wizard guides through 5 steps with checkpoints

---

## Phase 12: User Story 10 - Build Mode with Spec-Kit Integration (Priority: P1)

**Goal**: New Build mode integrates with spec-kit-assistant for software development

**Independent Test**: Say "start a new feature" and verify spec-kit workflow initiates

### Implementation for User Story 10

- [ ] T095 [P] [US10] Create BuildProject entity in crates/shared/src/build.rs
- [ ] T096 [US10] Create Speckit provider in crates/providers/src/speckit.rs
- [ ] T097 [P] [US10] Implement SpeckitSpecify skill in crates/agent_host/src/skills/build/specify.rs
- [ ] T098 [P] [US10] Implement SpeckitPlan skill in crates/agent_host/src/skills/build/plan.rs
- [ ] T099 [P] [US10] Implement SpeckitTasks skill in crates/agent_host/src/skills/build/tasks.rs
- [ ] T100 [US10] Implement SpeckitImplement skill in crates/agent_host/src/skills/build/implement.rs
- [ ] T101 [P] [US10] Implement SpeckitClarify skill in crates/agent_host/src/skills/build/clarify.rs
- [ ] T102 [P] [US10] Implement SpeckitAnalyze skill in crates/agent_host/src/skills/build/analyze.rs
- [ ] T103 [US10] Create crates/agent_host/src/skills/build/mod.rs to export Build skills
- [ ] T104 [US10] Register Build mode skills in SkillRegistry
- [ ] T105 [US10] Add Build tab to mode tabs in crates/app/src/main.rs

**Checkpoint**: Build mode invokes spec-kit commands successfully

---

## Phase 13: User Story 11 - Mode Capability Outline on Tab Switch (Priority: P1)

**Goal**: Each mode tab shows capability outline in preview panel

**Independent Test**: Switch to each mode, verify capability outline with skills and examples appears <1 second

### Implementation for User Story 11

- [ ] T106 [US11] Create ModeCapabilityOutline skill in crates/agent_host/src/skills/common/capability_outline.rs
- [ ] T107 [US11] Define capability content for Find mode (skills, examples)
- [ ] T108 [P] [US11] Define capability content for Fix mode
- [ ] T109 [P] [US11] Define capability content for Research mode
- [ ] T110 [P] [US11] Define capability content for Data mode (include dashboard wizard)
- [ ] T111 [P] [US11] Define capability content for Content mode (include coming soon labels)
- [ ] T112 [P] [US11] Define capability content for Build mode (speckit commands)
- [ ] T113 [US11] Implement capability outline display in crates/app/src/preview_panel.rs on_mode_change()
- [ ] T114 [US11] Add clickable example prompts that populate input field

**Checkpoint**: All 6 modes show capability outlines with clickable examples

---

## Phase 14: User Story 12 - Design Creation with Canva and Gemini (Priority: P1)

**Goal**: Content mode integrates Canva MCP and Nano Banana (Gemini CLI) for design

**Independent Test**: Ask to create a design, verify Canva templates appear or Gemini generates options

### Implementation for User Story 12

- [ ] T115 [P] [US12] Create Canva provider in crates/providers/src/canva.rs with MCP integration
- [ ] T116 [P] [US12] Create Gemini provider in crates/providers/src/gemini.rs with CLI wrapper
- [ ] T117 [US12] Implement CanvaMCP skill in crates/agent_host/src/skills/content/canva.rs
- [ ] T118 [US12] Implement NanoBananaDesign skill in crates/agent_host/src/skills/content/gemini_design.rs
- [ ] T119 [P] [US12] Implement DesignTemplates skill in crates/agent_host/src/skills/content/templates.rs
- [ ] T120 [US12] Create DesignConfig entity in crates/shared/src/design.rs
- [ ] T121 [US12] Implement graceful degradation with setup guidance when tools not configured
- [ ] T122 [US12] Add design skills to Content mod.rs

**Checkpoint**: Design tools work when configured, show setup guidance otherwise

---

## Phase 15: User Story 13 - Persona Engine and Content Automation (Priority: P1)

**Goal**: Content mode integrates persona system and automation engine, shows coming soon features

**Independent Test**: Ask to create a persona, verify detailed profile is generated

### Implementation for User Story 13

- [ ] T123 [P] [US13] Create Persona provider in crates/providers/src/persona.rs
- [ ] T124 [P] [US13] Create ContentAutomation provider in crates/providers/src/automation.rs
- [ ] T125 [US13] Implement PersonaEngine skill in crates/agent_host/src/skills/content/persona.rs
- [ ] T126 [US13] Implement ContentAutomation skill in crates/agent_host/src/skills/content/automation.rs
- [ ] T127 [US13] Add "coming soon" placeholder skills for MetaAds and DataViz
- [ ] T128 [US13] Update Content mode capability outline with coming soon labels
- [ ] T129 [US13] Add persona and automation skills to Content mod.rs

**Checkpoint**: Persona engine generates profiles, coming soon features displayed

---

## Phase 16: Polish & Cross-Cutting Concerns

**Purpose**: Final integration, optimization, and cleanup

- [ ] T130 [P] Verify all 47 skills registered in SkillRegistry
- [ ] T131 [P] Performance test file index with 100K files - verify <1 second search
- [ ] T132 [P] Verify no-delete policy: grep codebase for any fs::remove or delete calls in skills
- [ ] T133 [P] Test per-session confirmation flow for all Sensitive skills
- [ ] T134 Run integration test for skill execution logging (timing, inputs, outputs)
- [ ] T135 [P] Test graceful degradation when Playwright/Canva/Gemini not configured
- [ ] T136 Test password dialog with sudo commands
- [ ] T137 [P] Test file picker and drag-drop for adding context
- [ ] T138 Review audit log accessibility (primary user only in settings)
- [ ] T139 Run cargo clippy and fix warnings
- [ ] T140 Update CLAUDE.md with new skill system documentation

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies - start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 - BLOCKS all user stories
- **Phase 3-15 (User Stories)**: All depend on Phase 2 completion
- **Phase 16 (Polish)**: Depends on desired user stories being complete

### User Story Dependencies

Most stories are independent after Foundational phase:

| Story | Depends On | Notes |
|-------|------------|-------|
| US1 (Find) | Phase 2 only | Core MVP - enables file search |
| US2 (Research basic) | Phase 2 only | Independent |
| US3 (Research advanced) | US2 | Extends Research with automation |
| US4 (Fix) | Phase 2 only | Independent |
| US5 (Content) | Phase 2 only | Independent |
| US6 (Version Control) | Phase 2 only | Used by all modes |
| US7 (No Delete) | Phase 2 only | Critical safety - should be early |
| US8 (Data) | Phase 2 only | Independent |
| US9 (Dashboard) | US8 | Extends Data mode |
| US10 (Build) | Phase 2 only | Independent |
| US11 (Capability Outline) | US1-US10 | Needs mode skills defined first |
| US12 (Design) | US5 | Extends Content mode |
| US13 (Persona) | US5 | Extends Content mode |

### Recommended Order for Single Developer

1. Phase 1 → Phase 2 → US7 (No Delete - critical safety first)
2. US1 (Find - MVP value)
3. US6 (Version Control - safety net for all modes)
4. US11 (Capability Outline - discoverability)
5. US4 (Fix), US2 (Research), US5 (Content) - core modes
6. US8, US9 (Data + Dashboard)
7. US10 (Build)
8. US3, US12, US13 (Advanced features)
9. Phase 16 (Polish)

### Parallel Opportunities

**Within Phase 1**: T003, T004, T005, T008, T009, T011 can run in parallel
**Within Phase 2**: T015, T016, T18, T20 can run in parallel
**User Stories**: After Phase 2, any independent stories (US1, US2, US4, US5, US6, US7, US8, US10) can start in parallel

---

## Summary

| Metric | Value |
|--------|-------|
| **Total Tasks** | 140 |
| **Setup Phase** | 12 tasks |
| **Foundational Phase** | 11 tasks |
| **User Story Tasks** | 106 tasks |
| **Polish Phase** | 11 tasks |
| **User Stories** | 13 (all P1) |
| **Parallel Opportunities** | 58 tasks marked [P] |

### Tasks Per User Story

| Story | Tasks | Skills Implemented |
|-------|-------|-------------------|
| US1 - Find | 10 | fuzzy_file_search, drive_index, file_preview |
| US2 - Research Basic | 9 | research_clarify, web_search, web_fetch, citation_validate, mpa_context |
| US3 - Research Advanced | 6 | browser_automate, python_analysis |
| US4 - Fix | 9 | system_diagnostic, system_info_display, network_troubleshoot, browser_debug, log_analysis |
| US5 - Content | 8 | content_calendar, calendar_spreadsheet, mpa_context |
| US6 - Version Control | 8 | version_history, version_restore |
| US7 - No Delete | 5 | file_organize (with SafeFileOps) |
| US8 - Data | 8 | file_read, parse_data, analyze_with_references, data_validate, generate_chart |
| US9 - Dashboard | 8 | dashboard_wizard, dashboard_analyze, dashboard_config, dashboard_validate, dashboard_qa |
| US10 - Build | 11 | speckit_specify, speckit_plan, speckit_tasks, speckit_implement, speckit_clarify, speckit_analyze |
| US11 - Capability Outline | 9 | mode_capability_outline |
| US12 - Design | 8 | canva_mcp, nano_banana_design, design_templates |
| US13 - Persona | 7 | persona_engine, content_automation |

### MVP Scope

**Minimum Viable Product**: Complete Phase 1, Phase 2, US1, US6, US7, US11

This delivers:
- ✅ File search across all drives (core value)
- ✅ Version control safety net
- ✅ No-delete policy enforced
- ✅ Mode capability outlines for discoverability
- ✅ Foundation for all other features

Estimated: ~56 tasks for MVP
