# Implementation Plan: Entropy Bot

## Architecture

The entropy bot is a self-contained async task that monitors idle time and runs analysis passes when the system is inactive. It consumes data from the file index and graph, produces entropy scores and suggestions.

```
┌───────────────┐     ┌───────────────┐     ┌───────────────┐
│ Idle Detector │────▶│ Scheduler     │────▶│ Score Pass    │
│ (OS idle API) │     │ (rate limit)  │     │               │
└───────────────┘     └───────────────┘     └───────┬───────┘
                                                     │
                              ┌───────────────────────┼───────────────┐
                              ▼                       ▼               ▼
                       ┌──────────┐           ┌──────────┐    ┌──────────┐
                       │ Naming   │           │ Age/Depth│    │ Duplicate│
                       │ Analyzer │           │ Analyzer │    │ + Orphan │
                       └──────────┘           └──────────┘    └──────────┘
                              │                       │               │
                              └───────────────────────┼───────────────┘
                                                      ▼
                                              ┌──────────────┐
                                              │ Suggestion   │
                                              │ Generator    │
                                              └──────┬───────┘
                                                     ▼
                                              ┌──────────────┐
                                              │ UI Panel     │
                                              │ (cards)      │
                                              └──────────────┘
```

## Data Model

### New Table: `entropy_scores`

```sql
CREATE TABLE IF NOT EXISTS entropy_scores (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    naming_entropy REAL,
    age_spread REAL,
    depth_waste REAL,
    duplicate_ratio REAL,
    orphan_score REAL,
    composite_score REAL NOT NULL,
    computed_at TEXT NOT NULL
);
```

### New Table: `suggestions`

```sql
CREATE TABLE IF NOT EXISTS suggestions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    suggestion_type TEXT NOT NULL,
    affected_paths TEXT NOT NULL,
    reason TEXT NOT NULL,
    confidence REAL NOT NULL,
    space_savings_bytes INTEGER,
    status TEXT NOT NULL DEFAULT 'pending',
    deferred_until TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_suggestions_status ON suggestions(status);
CREATE INDEX IF NOT EXISTS idx_suggestions_type ON suggestions(suggestion_type);
```

## Key Decisions

### D1: Idle Detection
**Choice:** Two-tier approach:
1. Primary: Check if no user input to the app for N minutes (track last UI interaction timestamp in AppState)
2. Fallback: X11 `XScreenSaverQueryInfo` via `x11rb` crate (optional, Linux only)

**Rationale:** App-level idle is simpler, cross-platform, and sufficient. OS-level idle is a nice-to-have for when the app is in the background.

### D2: Scoring Granularity
**Choice:** Score per-directory rather than per-file for entropy dimensions, then propagate to files.
**Rationale:** Naming entropy and age spread are directory-level properties. Computing per-directory reduces work by ~100x. Duplicate ratio and orphan score are per-file but computed from existing graph edges.

### D3: Suggestion Deduplication
**Choice:** Hash `(suggestion_type, sorted affected_paths)` and skip if already exists with status `pending` or `deferred`.
**Rationale:** Prevents duplicate suggestions across runs.

### D4: Staging Area
**Choice:** `~/.ylh-staging/{timestamp}/` with `manifest.json` per operation.
**Rationale:** Timestamped dirs prevent collisions. Manifest enables undo. 30-day TTL enforced by cleanup pass.

## File Changes

| File | Changes |
|------|---------|
| `crates/services/src/entropy_bot.rs` | **NEW** — `EntropyBot` struct, idle loop, score pass, suggestion generator |
| `crates/services/src/entropy_scorers.rs` | **NEW** — NamingAnalyzer, AgeAnalyzer, DepthAnalyzer, DuplicateScorer, OrphanScorer |
| `crates/services/src/file_index.rs` | New table creation, queries for entropy data |
| `crates/services/src/lib.rs` | Export new modules |
| `crates/services/Cargo.toml` | Add `sha2`, `flate2`, `tar` crates |
| `crates/app/src/types.rs` | Add `entropy_bot_status` field, suggestion count |
| `crates/app/src/main.rs` | Status indicator in top bar, suggestions panel |

## Phases

### Phase 1: Schema + Scoring Framework
- Add `entropy_scores` and `suggestions` tables
- Implement `EntropyBot` struct with config
- Implement `score_pass()` with naming_entropy and age_spread scorers

### Phase 2: Remaining Scorers
- depth_waste: count single-child directory chains
- duplicate_ratio: query `file_content_hashes` table (from file-graph feature)
- orphan_score: count files with zero edges in `file_edges` table
- Composite score calculation with configurable weights

### Phase 3: Suggestion Engine
- Rules engine: map score thresholds to suggestion types
- Deduplication logic
- Confidence scoring based on how many entropy signals fire

### Phase 4: Idle Loop + UI
- Idle detection (app-level: track last interaction timestamp)
- Scheduler: respect rate limits (1 pass/hour, configurable)
- Cooperative cancellation via `CancellationToken`
- Top bar status indicator
- Suggestion cards in sidebar

### Phase 5: Safe Operations
- Staging area creation and manifest
- Accept handler: copy → verify checksum → remove original
- Undo handler: restore from staging
- Cleanup pass: purge staging dirs older than 30 days
