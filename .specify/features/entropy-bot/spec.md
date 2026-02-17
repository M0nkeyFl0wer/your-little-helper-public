# Feature: Entropy Bot — Idle-Time File Organization Daemon

## 1. Overview

A background daemon that runs during computer idle time to analyze file organization quality, detect entropy (disorder), and surface actionable suggestions — duplicate files, orphaned documents, naming inconsistencies, stale archives, deep nesting waste. Think of it as a janitor that observes but doesn't touch anything without explicit permission.

## 2. Problem Statement

File systems accumulate entropy over time. Downloads pile up, projects get abandoned, duplicates multiply, naming conventions drift. Users know their files are messy but don't have time or tools to assess the damage, let alone fix it. By the time they search for something, the disorder has already cost them — they can't find what they need, or they find 5 copies and don't know which is current.

## 3. Requirements

### 3.1 Idle Detection
- R1: Monitor system idle time via OS APIs (X11 idle time on Linux, or fallback to "no user input to the app for N minutes")
- R2: Configurable idle threshold (default: 5 minutes of inactivity)
- R3: Configurable schedule: run at most once per hour, with a daily deep scan option
- R4: Stop immediately when user activity resumes (cooperative cancellation)
- R5: CPU/IO throttling: run at low priority (nice +19), limit to 1 thread

### 3.2 Entropy Scoring
- R6: Score each indexed directory on 5 dimensions (0.0 to 1.0 each):
  - `naming_entropy` — how inconsistent are file naming conventions within the directory
  - `age_spread` — how far apart are the oldest and newest files (stale mixed with fresh)
  - `depth_waste` — directories with only 1 child, or deeply nested single-file paths
  - `duplicate_ratio` — fraction of files that have duplicates elsewhere
  - `orphan_score` — files not connected in the file graph (no references, no siblings, no co-modifications)
- R7: Composite entropy score: weighted average of 5 dimensions
- R8: Per-file entropy attributes stored in new table

### 3.3 Suggestion Engine
- R9: Generate actionable suggestions based on entropy scores:
  - `archive` — directory hasn't been touched in 6+ months, suggest archiving
  - `deduplicate` — exact or near-duplicate pairs found, suggest keeping one
  - `flatten` — deeply nested single-file directories, suggest flattening
  - `rename` — files with naming pattern violations (mixed case, spaces, special chars)
  - `organize` — high-entropy directory, suggest grouping by type/date/project
- R10: Each suggestion includes: affected paths, reason, confidence (0-1), estimated space savings
- R11: Suggestions persisted in SQLite, deduplicated across runs
- R12: User can dismiss, defer (snooze 7 days), or accept suggestions

### 3.4 Safe Operations
- R13: Accept = move files to a staging area first, never delete directly
- R14: Staging area: `~/.ylh-staging/` with timestamped subdirectories
- R15: Undo window: staged files kept for 30 days before permanent deletion prompt
- R16: All operations logged to `~/.ylh-staging/operations.log` with timestamps and checksums
- R17: Archive operation creates tar.gz in staging, removes originals only after checksum verification

### 3.5 Storage Schema
- R18: New `entropy_scores` table:
  ```sql
  CREATE TABLE entropy_scores (
    file_id INTEGER PRIMARY KEY REFERENCES files(id),
    naming_entropy REAL,
    age_spread REAL,
    depth_waste REAL,
    duplicate_ratio REAL,
    orphan_score REAL,
    composite_score REAL NOT NULL,
    computed_at TEXT NOT NULL
  );
  ```
- R19: New `suggestions` table:
  ```sql
  CREATE TABLE suggestions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    suggestion_type TEXT NOT NULL,     -- archive, deduplicate, flatten, rename, organize
    affected_paths TEXT NOT NULL,       -- JSON array of paths
    reason TEXT NOT NULL,
    confidence REAL NOT NULL,
    space_savings_bytes INTEGER,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, dismissed, deferred, accepted, completed
    deferred_until TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
  );
  ```

### 3.6 UI Integration
- R20: Status indicator in top bar: entropy bot state (idle/scanning/sleeping)
- R21: Suggestions panel accessible from sidebar or dedicated view
- R22: Each suggestion card shows: type icon, affected paths, reason, confidence bar, action buttons (Accept/Dismiss/Defer)
- R23: Dashboard summary: total suggestions, space reclaimable, top 5 highest-entropy directories

### 3.7 Performance
- R24: Directory scoring < 50ms per directory
- R25: Full scan of 100K files completes within 10 minutes at low priority
- R26: Suggestion generation is incremental — only re-score modified directories

## 4. Non-Requirements
- No automatic file operations without user confirmation (read-only by default)
- No network operations (purely local analysis)
- No integration with cloud storage providers
- No machine learning for suggestion quality (heuristics only, for now)

## 5. API Surface

```rust
pub struct EntropyBot {
    db: Arc<rusqlite::Connection>,
    config: EntropyBotConfig,
    cancel: CancellationToken,
}

impl EntropyBot {
    /// Start the idle-time monitoring loop.
    pub async fn start(&self) -> Result<()>;

    /// Run a single scoring pass (called during idle time).
    pub async fn score_pass(&self) -> Result<ScoringStats>;

    /// Generate suggestions from current entropy scores.
    pub fn generate_suggestions(&self) -> Result<Vec<Suggestion>>;

    /// Accept a suggestion (move to staging).
    pub async fn accept_suggestion(&self, id: i64) -> Result<OperationResult>;

    /// Dismiss a suggestion.
    pub fn dismiss_suggestion(&self, id: i64) -> Result<()>;

    /// Defer a suggestion for N days.
    pub fn defer_suggestion(&self, id: i64, days: u32) -> Result<()>;

    /// Get current entropy summary for UI.
    pub fn summary(&self) -> Result<EntropySummary>;
}
```

## 6. Dependencies
- `tokio` for async idle loop + cancellation
- `sha2` for content hashing (shared with file-graph duplicate detection)
- `flate2` + `tar` for archive creation
- `nix` or `x11rb` for idle time detection on Linux
- Existing `rusqlite` for storage

## 7. Migration Path
- New tables via `CREATE TABLE IF NOT EXISTS`
- Bot disabled by default, enabled in settings
- Zero impact when disabled — no background work, no CPU usage
- Suggestions accumulate silently; user opts into reviewing them
