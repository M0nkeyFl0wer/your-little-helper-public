# Tasks: Entropy Bot

## Phase 1: Setup — Schema + Framework

- [ ] T001 Add `entropy_scores` table creation to `FileIndexService::new()` in `crates/services/src/file_index.rs`
- [ ] T002 Add `suggestions` table creation (with indexes) to `FileIndexService::new()`
- [ ] T003 Add `sha2`, `flate2`, `tar` dependencies to `crates/services/Cargo.toml`
- [ ] T004 Create `crates/services/src/entropy_bot.rs` with `EntropyBot` struct, `EntropyBotConfig`, `CancellationToken` field
- [ ] T005 Create `crates/services/src/entropy_scorers.rs` with `EntropyScorer` trait: `fn score(&self, dir_path: &Path, files: &[FileIndexEntry]) -> Result<f64>`
- [ ] T006 Add `pub mod entropy_bot; pub mod entropy_scorers;` to `crates/services/src/lib.rs`

## Phase 2: Foundational — Scoring Engine

- [ ] T007 Implement `NamingEntropyScorer` — analyze naming conventions within a directory (mixed casing, inconsistent separators, pattern count / total files)
- [ ] T008 [P] Implement `AgeSpreadScorer` — compute (newest_modified - oldest_modified) / threshold, normalize to 0-1
- [ ] T009 [P] Implement `DepthWasteScorer` — count single-child directory chains, score = chain_length / total_depth
- [ ] T010 Implement `DuplicateRatioScorer` — query `file_content_hashes` for files with matching SHA-256, ratio = duplicate_count / total_count
- [ ] T011 Implement `OrphanScorer` — query `file_edges` for files with zero edges, ratio = orphan_count / total_count
- [ ] T012 Implement `EntropyBot::score_pass() -> Result<ScoringStats>` — iterate directories, run all scorers, compute weighted composite, upsert into `entropy_scores`

## Phase 3: User Story — Suggestion Engine

- [ ] T013 Implement suggestion rules engine — map entropy thresholds to suggestion types:
  - age_spread > 0.8 + all files old → `archive`
  - duplicate_ratio > 0.2 → `deduplicate`
  - depth_waste > 0.6 → `flatten`
  - naming_entropy > 0.7 → `rename`
  - composite_score > 0.6 → `organize`
- [ ] T014 Implement `EntropyBot::generate_suggestions() -> Result<Vec<Suggestion>>` with deduplication (hash type + sorted paths)
- [ ] T015 Implement `Suggestion` struct with fields: id, suggestion_type, affected_paths, reason, confidence, space_savings_bytes, status
- [ ] T016 Implement `EntropyBot::dismiss_suggestion(id)`, `defer_suggestion(id, days)` — update status in `suggestions` table

## Phase 4: User Story — Safe Operations

- [ ] T017 Implement staging area creation at `~/.ylh-staging/{timestamp}/` with `manifest.json`
- [ ] T018 Implement `EntropyBot::accept_suggestion(id) -> Result<OperationResult>` — copy files to staging, verify checksums, update suggestion status
- [ ] T019 Implement archive operation: create `tar.gz` in staging for `archive` suggestions, verify checksum, then remove originals
- [ ] T020 Implement undo operation: restore files from staging directory back to original paths using manifest
- [ ] T021 Implement staging cleanup: purge staging dirs older than 30 days, log what was permanently removed

## Phase 5: User Story — Idle Loop + Scheduling

- [ ] T022 Implement idle detection: track `last_interaction_time` in AppState, check if elapsed > threshold
- [ ] T023 Implement `EntropyBot::start() -> Result<()>` — async loop: check idle → run score_pass → generate_suggestions → sleep
- [ ] T024 Add rate limiting: max 1 pass per hour, configurable daily deep scan time
- [ ] T025 Add cooperative cancellation: check `CancellationToken` between directory scoring iterations

## Phase 6: Polish — UI Integration

- [ ] T026 Add `entropy_bot_status: EntropyBotStatus` enum (Idle, Scanning, Sleeping) to `AppState` in `crates/app/src/types.rs`
- [ ] T027 Add entropy status indicator in top bar (small icon + text) in `crates/app/src/main.rs`
- [ ] T028 Add `pending_suggestions_count: usize` to `AppState`, query on each score pass completion
- [ ] T029 Add suggestions panel in sidebar: list cards with type icon, paths, reason, confidence bar, Accept/Dismiss/Defer buttons
- [ ] T030 Implement `EntropyBot::summary() -> Result<EntropySummary>` — total suggestions, space reclaimable, top 5 highest-entropy dirs

## Summary

- **Total tasks:** 30
- **Parallel opportunities:** T008, T009 (independent scorers); T007, T010, T011 (partially independent); T017-T021 (independent of scoring)
- **Dependencies:** T001-T002 before T012. T003 before T017-T019. T005 before T007-T011. T012 before T013-T014. T014-T016 before T018. T022 before T023. Depends on file-graph for T010, T011 (need edges + hashes tables).
