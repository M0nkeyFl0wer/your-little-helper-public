# Quality Checklist: Semantic Search + File Graph + Entropy Bot

## Implementation Status

### Semantic Search Layer
- [X] CHK001 `file_embeddings` table created in schema [Spec §3.4]
- [X] CHK002 `EmbeddingClient` HTTP client for Ollama `/api/embeddings` [Spec §3.1]
- [X] CHK003 `embed_single()` and `embed_texts()` batch support [Spec §3.1]
- [X] CHK004 `store_embedding()` and `get_embedding()` BLOB encode/decode [Spec §3.4]
- [X] CHK005 `files_without_embeddings()` query for incremental generation [Spec §3.1, R5]
- [X] CHK006 `embedding_coverage()` reporting method [Spec §3.5]
- [X] CHK007 `classify_intent()` heuristic (extension, path, camelCase → filename) [Spec §3.3]
- [X] CHK008 `cosine_similarity()` on f32 slices [Spec §3.2]
- [X] CHK009 `semantic_search()` with RRF fusion of FTS5 + Jaro-Winkler + cosine [Spec §3.2]
- [X] CHK010 Graceful degradation when no embeddings exist [Spec §3.1, R6]
- [X] CHK011 `build_embedding_text()` composite string builder [Spec §3.1, R2]
- [X] CHK012 `embedding_score` field added to `FileSearchResult` [Spec §3.2]
- [ ] CHK013 LRU cache for query embeddings (64 entries) [Spec §3.5, R18]
- [ ] CHK014 Wire `semantic_search` into `fuzzy_file_search` skill [Plan §Phase 4]

### File Relationship Graph
- [X] CHK020 `file_edges` table with indexes created in schema [Spec §3.2]
- [X] CHK021 `file_content_hashes` table created in schema [Plan]
- [X] CHK022 `EdgeCandidate` struct and `upsert_edges()` helper [Spec §3.2]
- [X] CHK023 `compute_content_hash()` SHA-256 (skip > 10MB) [Spec §3.1, R5]
- [X] CHK024 `find_sibling_edges()` — same-stem, test files, config pairs, README [Spec §3.1, R4]
- [X] CHK025 `find_duplicate_edges()` from content hashes [Spec §3.1, R5]
- [X] CHK026 `find_reference_edges()` — regex scan text files for filename mentions [Spec §3.1, R3]
- [X] CHK027 `find_comod_edges()` — git log co-modification analysis [Spec §3.3]
- [X] CHK028 `compute_graph_edges()` orchestrator [Spec §3.3]
- [X] CHK029 `related_files()` BFS traversal within N hops [Spec §3.4, R14]
- [X] CHK030 `find_duplicates()` query [Spec §3.4, R17]
- [X] CHK031 `prune_edges()` below strength threshold [Spec §3.5, R18]
- [X] CHK032 `store_content_hash()` method [Plan §Phase 1]

### Entropy Bot
- [X] CHK040 `entropy_scores` table created in schema [Spec §3.5, R18]
- [X] CHK041 `suggestions` table with indexes created in schema [Spec §3.5, R19]
- [X] CHK042 `EntropyBotConfig` with idle threshold, min interval, dirs per pass [Spec §3.1]
- [X] CHK043 `EntropyBotStatus` enum (Idle, Scanning, Sleeping, Disabled) [Spec §3.6, R20]
- [X] CHK044 `naming_entropy()` Shannon entropy scorer [Spec §3.2, R6]
- [X] CHK045 `age_spread()` temporal spread scorer [Spec §3.2, R6]
- [X] CHK046 `depth_waste()` single-child chain scorer [Spec §3.2, R6]
- [X] CHK047 `composite_score()` weighted average [Spec §3.2, R7]
- [X] CHK048 `score_pass()` — iterate directories, run all scorers, upsert [Spec §3.2]
- [X] CHK049 `generate_suggestions()` with archive + deduplicate rules [Spec §3.3]
- [X] CHK050 Suggestion deduplication (type + paths hash) [Plan §D3]
- [X] CHK051 `dismiss_suggestion()` status update [Spec §3.3, R12]
- [X] CHK052 `defer_suggestion()` with date [Spec §3.3, R12]
- [X] CHK053 `accept_suggestion()` — staging area with manifest [Spec §3.4, R13-R14]
- [X] CHK054 `summary()` for UI (pending count, reclaimable space, top dirs) [Spec §3.6, R23]
- [X] CHK055 `start()` idle monitoring loop with cooperative cancellation [Spec §3.1, R4]
- [X] CHK056 Entropy scorer unit tests passing [Quality]

### Infrastructure
- [X] CHK060 `sha2`, `flate2`, `tar` added to workspace dependencies [Plan]
- [X] CHK061 Module exports in `services/src/lib.rs` [Plan]
- [X] CHK062 Full workspace compiles clean (zero warnings in our code) [Quality]
- [X] CHK063 All existing tests pass (14 tests) [Quality]
- [X] CHK064 Debug logging removed from providers (openai, gemini, router, state) [Cleanup]

### Deferred / Future
- [ ] CHK070 UI: Suggestions panel in sidebar [Spec §3.6, R21-R22]
- [ ] CHK071 UI: Entropy status indicator in top bar [Spec §3.6, R20]
- [ ] CHK072 Wire entropy bot into AppState lifecycle [Plan §Phase 5]
- [ ] CHK073 Content hash computation during scan_drive() [Plan §Phase 1]
- [ ] CHK074 Similarity analyzer (depends on embeddings being populated) [Plan §Phase 4]
- [ ] CHK075 Co-modification edge path-to-ID resolution [Plan §Phase 3]
