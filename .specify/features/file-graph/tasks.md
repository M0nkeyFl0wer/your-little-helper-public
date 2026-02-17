# Tasks: File Relationship Graph

## Phase 1: Setup — Schema + Content Hashing

- [ ] T001 Add `file_edges` table creation (with indexes) to `FileIndexService::new()` in `crates/services/src/file_index.rs`
- [ ] T002 Add `file_content_hashes` table creation to `FileIndexService::new()`
- [ ] T003 Add `sha2` dependency to `crates/services/Cargo.toml`
- [ ] T004 Implement `compute_content_hash(path: &Path) -> Result<String>` (SHA-256, skip files > 10MB)
- [ ] T005 Integrate content hash computation into `scan_drive()` — compute hash during file indexing, store in `file_content_hashes`

## Phase 2: Foundational — Edge Analyzers

- [ ] T006 Create `crates/services/src/graph_analyzers.rs` with `EdgeAnalyzer` trait: `fn analyze(&self, db: &Connection) -> Result<Vec<EdgeCandidate>>`
- [ ] T007 Implement `SiblingAnalyzer` — detect same-stem-different-ext, config pairs, README associations within directories
- [ ] T008 [P] Implement `ReferenceAnalyzer` — regex scan text files (< 100KB) for patterns matching other indexed filenames
- [ ] T009 [P] Implement `DuplicateAnalyzer` — query `file_content_hashes` for matching SHA-256 values, create `duplicate` edges with strength 1.0
- [ ] T010 Implement `upsert_edges(edges: &[EdgeCandidate])` helper — INSERT OR REPLACE into `file_edges` with conflict handling on unique index
- [ ] T011 Add `pub mod graph_analyzers;` to `crates/services/src/lib.rs`

## Phase 3: User Story — Co-Modification Analysis

- [ ] T012 Implement `CoModAnalyzer` — shell out to `git log --name-only --format=%H --since=6months`, parse output into co-occurrence pairs
- [ ] T013 Normalize co-occurrence counts to 0-1 strength (divide by max pair frequency), filter pairs below 0.3 threshold

## Phase 4: User Story — Similarity Edges + Queries

- [ ] T014 Implement `SimilarityAnalyzer` — query `file_embeddings` table, compute cosine similarity for files within same directory, create edges above 0.85 threshold
- [ ] T015 Implement `FileIndexService::compute_graph_edges() -> Result<GraphStats>` — orchestrate all analyzers, report counts per edge type
- [ ] T016 Implement `FileIndexService::related_files(file_id, depth, limit) -> Result<Vec<FileSearchResult>>` — BFS traversal on `file_edges`, sorted by aggregate strength
- [ ] T017 Implement `FileIndexService::find_duplicates() -> Result<Vec<(FileSearchResult, FileSearchResult, f64)>>` — join `file_content_hashes` on SHA-256

## Phase 5: Polish

- [ ] T018 Implement `FileIndexService::prune_edges(min_strength: f64) -> Result<usize>` — DELETE edges below threshold
- [ ] T019 Wire `related_files()` into search results — add `related: Vec<String>` field to `FileSearchResult` for top 3 related file paths
- [ ] T020 Add graph stats to embedding coverage reporting (edge count by type)

## Summary

- **Total tasks:** 20
- **Parallel opportunities:** T008, T009 (independent analyzers); T001, T002, T003 (independent setup)
- **Dependencies:** T001-T002 before T005. T003-T004 before T005. T006 before T007-T009. T014 depends on semantic-search embeddings. T010 before T015.
