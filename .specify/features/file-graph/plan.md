# Implementation Plan: File Relationship Graph

## Architecture

The file graph adds a `file_edges` table to the existing SQLite database and computes edges through multiple analyzers that run as background tasks.

```
┌────────────────────────────────────────────┐
│              Edge Analyzers                │
├──────────┬──────────┬──────────┬──────────┤
│ Similar  │ CoMod    │ Reference│ Sibling  │
│ Analyzer │ Analyzer │ Analyzer │ Analyzer │
│ (cosine) │ (git log)│ (regex)  │ (names)  │
└────┬─────┴────┬─────┴────┬─────┴────┬─────┘
     │          │          │          │
     └──────────┴──────────┴──────────┘
                    │
              ┌─────▼─────┐
              │ file_edges│
              │  (SQLite) │
              └─────┬─────┘
                    │
     ┌──────────────┼──────────────┐
     ▼              ▼              ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│ Related  │ │ Duplicate│ │ Search   │
│ Files    │ │ Finder   │ │ Boost    │
└──────────┘ └──────────┘ └──────────┘
```

## Data Model

### New Table: `file_edges`

```sql
CREATE TABLE IF NOT EXISTS file_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    target_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    edge_type TEXT NOT NULL,
    strength REAL NOT NULL DEFAULT 1.0,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_edges_source ON file_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_edges_target ON file_edges(target_id);
CREATE INDEX IF NOT EXISTS idx_edges_type ON file_edges(edge_type);
CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_pair ON file_edges(source_id, target_id, edge_type);
```

### New Table: `file_content_hashes`

```sql
CREATE TABLE IF NOT EXISTS file_content_hashes (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    sha256 TEXT NOT NULL,
    computed_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_content_hash ON file_content_hashes(sha256);
```

## Key Decisions

### D1: Git Integration
**Choice:** Shell out to `git log --name-only --format=%H --since=6months` rather than `git2` crate
**Rationale:** Avoids adding `libgit2` as a compile dependency (~30s build time increase). The app already shells out for git in `version_control.rs`. Parse output to build co-modification frequency map.

### D2: Reference Detection
**Choice:** Regex scan of text files for patterns matching other indexed filenames
**Rationale:** Simple, fast, catches imports/includes/links. Pattern: `\b{filename_stem}\b` for each indexed file within same drive. Limit to text files < 100KB.

### D3: Sibling Heuristics
Detect name patterns:
- Same stem, different extension: `foo.rs` ↔ `foo_test.rs`
- Config pairs: `config.yaml` ↔ `config.example.yaml`
- Index files: `mod.rs` ↔ other `.rs` files in same dir
- README: `README.md` ↔ all files in same dir (weak edge, strength 0.3)

### D4: Edge Pruning
Prune edges below strength 0.3 during compaction. Compaction runs after edge computation completes. Keeps table size manageable.

## File Changes

| File | Changes |
|------|---------|
| `crates/services/src/file_index.rs` | New table creation, `compute_graph_edges()`, `related_files()`, `find_duplicates()`, `prune_edges()` |
| `crates/services/src/graph_analyzers.rs` | **NEW** — SimilarityAnalyzer, CoModAnalyzer, ReferenceAnalyzer, SiblingAnalyzer |
| `crates/services/Cargo.toml` | Add `sha2` crate for content hashing |

## Phases

### Phase 1: Schema + Content Hashing
- Add `file_edges` and `file_content_hashes` tables to `FileIndexService::new()`
- Implement content hash computation during scan (SHA-256 for files < 10MB)
- `find_duplicates()` query based on matching hashes

### Phase 2: Sibling + Reference Analyzers
- Sibling analyzer: directory listing + name pattern matching
- Reference analyzer: regex scan for filename stems in text files
- Both write edges to `file_edges` with appropriate strength values

### Phase 3: Co-Modification Analyzer
- Parse `git log` output to build file co-occurrence matrix
- Normalize to frequency score (0-1 range)
- Write `co_modified` edges for pairs above threshold (0.3)

### Phase 4: Similarity Analyzer + Graph Queries
- Depends on semantic-search embeddings being available
- Compute cosine similarity for all pairs within same directory first, then cross-directory
- `related_files()` with BFS traversal up to depth N
- Search boost integration with existing search path
