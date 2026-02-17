# Tasks: Semantic Search Layer

## Phase 1: Setup — Schema + Embedding Client

- [ ] T001 Add `file_embeddings` table creation to `FileIndexService::new()` in `crates/services/src/file_index.rs`
- [ ] T002 Create `crates/services/src/embedding_client.rs` with `EmbeddingClient` struct (HTTP client for Ollama `/api/embeddings`)
- [ ] T003 Implement `EmbeddingClient::embed_texts(texts: &[String]) -> Result<Vec<Vec<f32>>>` with batch support (32 per request)
- [ ] T004 Implement `EmbeddingClient::embed_single(text: &str) -> Result<Vec<f32>>` convenience wrapper
- [ ] T005 Add `pub mod embedding_client;` to `crates/services/src/lib.rs`

## Phase 2: Foundational — Embedding Generation

- [ ] T006 Implement `build_embedding_text(file: &FileIndexEntry) -> String` — composite text from name, extension, path components, size category, modification date
- [ ] T007 Implement `extract_text_content(path: &Path, max_tokens: usize) -> Option<String>` — read first 2048 tokens from text files (.md, .txt, .rs, .py, .json, .toml, .yaml, .js, .ts, .css, .html)
- [ ] T008 [P] Implement `FileIndexService::generate_embeddings(batch_size: usize) -> Result<EmbeddingStats>` — query files without embeddings, batch embed, store as BLOB
- [ ] T009 [P] Implement `FileIndexService::embedding_coverage() -> Result<(usize, usize)>` — count files with/without embeddings

## Phase 3: User Story — Hybrid Search

- [ ] T010 Implement `cosine_similarity(a: &[f32], b: &[f32]) -> f64` utility function
- [ ] T011 Implement `classify_intent(query: &str) -> SearchIntent` — heuristic: extension/path/camelCase/snake_case → filename; else semantic
- [ ] T012 Implement `reciprocal_rank_fusion(ranked_lists: &[Vec<(i64, f64)>], k: f64) -> Vec<(i64, f64)>` — merge multiple ranked result lists
- [ ] T013 Implement `FileIndexService::semantic_search(query: &str, limit: usize) -> Result<Vec<FileSearchResult>>` — orchestrate FTS5 + Jaro-Winkler + cosine, fuse with RRF
- [ ] T014 Add LRU cache for query embeddings (64 entries) in `FileIndexService` using `HashMap` with `VecDeque` eviction

## Phase 4: Polish — Integration + Fallback

- [ ] T015 Wire `semantic_search` into the existing `fuzzy_file_search` skill in `crates/agent_host/src/skills/` as the primary search path
- [ ] T016 Add graceful degradation: if Ollama unavailable or no embeddings, fall back to existing FTS5+Jaro-Winkler
- [ ] T017 Add `embedding_score: Option<f64>` to `FileSearchResult` in `crates/services/src/file_index.rs` for transparency

## Summary

- **Total tasks:** 17
- **Parallel opportunities:** T008, T009 (independent of each other); T010, T011, T012 (independent utilities)
- **Dependencies:** T001 must complete before T008. T002-T004 must complete before T008. T010-T012 must complete before T013.
