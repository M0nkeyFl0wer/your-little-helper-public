# Feature: File Relationship Graph

## 1. Overview

Build a graph layer on top of the file index that captures relationships between files — co-modification patterns, content similarity edges, directory siblings, references/imports, and semantic clusters. Enables "related files" discovery and graph-aware search re-ranking.

## 2. Problem Statement

Files don't exist in isolation. A user searching for one file often wants nearby related files — the config that goes with the script, the test that covers the module, the document that references the data file. The current flat index has no concept of relationships. Users must manually navigate directory trees or remember associations.

## 3. Requirements

### 3.1 Edge Types
- R1: `similar_to` — cosine similarity above threshold (0.85) between file embeddings
- R2: `co_modified` — files frequently modified in the same git commit or within same 5-minute window
- R3: `references` — file A contains a string that matches file B's name or path (import/include/link detection)
- R4: `sibling` — files in the same directory with related names (e.g., `foo.rs` and `foo_test.rs`, `config.yaml` and `config.example.yaml`)
- R5: `duplicate` — files with identical content hash (exact duplicates) or very high similarity (>0.98)

### 3.2 Graph Storage
- R6: New `file_edges` table:
  ```sql
  CREATE TABLE file_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id INTEGER NOT NULL REFERENCES files(id),
    target_id INTEGER NOT NULL REFERENCES files(id),
    edge_type TEXT NOT NULL,          -- similar_to, co_modified, references, sibling, duplicate
    strength REAL NOT NULL DEFAULT 1.0, -- 0.0 to 1.0
    metadata TEXT,                     -- JSON: commit hashes, line numbers, etc.
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
  );
  CREATE INDEX idx_edges_source ON file_edges(source_id);
  CREATE INDEX idx_edges_target ON file_edges(target_id);
  CREATE INDEX idx_edges_type ON file_edges(edge_type);
  CREATE UNIQUE INDEX idx_edges_pair ON file_edges(source_id, target_id, edge_type);
  ```

### 3.3 Graph Computation
- R7: Edge computation runs as a background task (not blocking UI or search)
- R8: Incremental: only recompute edges for files modified since last graph update
- R9: `similar_to` edges derived from embedding cosine similarity (requires semantic-search feature)
- R10: `co_modified` edges derived from git log analysis (`git log --name-only`)
- R11: `references` edges derived from content scanning (regex for filenames/paths in text files)
- R12: `sibling` edges derived from directory listing + name similarity heuristics
- R13: `duplicate` edges derived from SHA-256 content hashing

### 3.4 Graph Queries
- R14: `related_files(file_id, depth=1, limit=20)` — return files connected within N hops
- R15: `file_clusters()` — group files into clusters based on graph connectivity
- R16: Search re-ranking: boost files that are related to recently accessed/searched files
- R17: Duplicate detection: list all exact and near-duplicate pairs

### 3.5 Performance
- R18: Edge table size bounded — prune edges below strength 0.3 during compaction
- R19: Graph computation < 5 minutes for 100K file index
- R20: Related files query < 100ms

## 4. Non-Requirements
- No visualization of the graph (future feature)
- No cross-drive edges (edges only within same drive_id)
- No real-time edge updates on file save (batch only)
- No PageRank or centrality scoring (future feature)

## 5. API Surface

```rust
impl FileIndexService {
    /// Compute graph edges for files modified since last run.
    pub async fn compute_graph_edges(&self) -> Result<GraphStats>;

    /// Get files related to a given file within N hops.
    pub fn related_files(&self, file_id: i64, depth: usize, limit: usize) -> Result<Vec<FileSearchResult>>;

    /// Find duplicate file pairs.
    pub fn find_duplicates(&self) -> Result<Vec<(FileSearchResult, FileSearchResult, f64)>>;

    /// Prune weak edges below threshold.
    pub fn prune_edges(&self, min_strength: f64) -> Result<usize>;
}
```

## 6. Dependencies
- Depends on semantic-search feature for `similar_to` edge computation
- `git2` crate (or shell `git log`) for co-modification analysis
- `sha2` crate for content hashing (duplicate detection)
- Existing `rusqlite` for storage

## 7. Migration Path
- New table via `CREATE TABLE IF NOT EXISTS`
- Graph edges populated during entropy bot idle cycles
- Search works without graph (edges are optional boost signal)
