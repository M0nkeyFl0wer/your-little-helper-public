# Implementation Plan: Semantic Search Layer

## Architecture

The semantic search layer extends `FileIndexService` in `crates/services/src/file_index.rs` with:
1. A new `file_embeddings` SQLite table
2. An embedding client that talks to Ollama's `/api/embeddings` endpoint
3. Cosine similarity scoring + reciprocal rank fusion in the search path

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ Search Query  │────▶│ Intent       │────▶│ Hybrid       │
│              │     │ Classifier   │     │ Ranker (RRF) │
└──────────────┘     └──────────────┘     └──────┬───────┘
                                                  │
                          ┌───────────────────────┼───────────────────┐
                          ▼                       ▼                   ▼
                   ┌──────────┐           ┌──────────┐        ┌──────────┐
                   │ FTS5     │           │ Jaro-    │        │ Cosine   │
                   │ Prefix   │           │ Winkler  │        │ Similarity│
                   └──────────┘           └──────────┘        └──────────┘
                          │                       │                   │
                          └───────────────────────┼───────────────────┘
                                                  ▼
                                          ┌──────────┐
                                          │ Merged   │
                                          │ Results  │
                                          └──────────┘
```

## Data Model

### New Table: `file_embeddings`

```sql
CREATE TABLE IF NOT EXISTS file_embeddings (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,
    model_name TEXT NOT NULL,
    embedded_at TEXT NOT NULL,
    content_hash TEXT
);
```

Embedding stored as raw `[f32; 768]` — 3072 bytes per file. For 500K files, that's ~1.5GB. Acceptable for local SQLite.

## Key Decisions

### D1: Embedding Model
**Choice:** Ollama `nomic-embed-text` (768 dimensions)
**Rationale:** Already available if user has Ollama installed (common for local LLM users). Fast, small, good quality. Falls back gracefully if unavailable.

### D2: Embedding Input Text
For each file, embed a composite string:
```
{filename} | {extension} | {path_components_last_3} | {size_category} | {modified_month_year}
```
For text files (< 50KB), append first 2048 tokens of content.

### D3: Cosine Similarity Computation
**Choice:** In-process f32 dot product (no external vector DB)
**Rationale:** For 500K files, brute-force cosine takes ~50ms (768-dim × 500K = 384M multiply-adds, single-threaded). Acceptable. Avoids adding a vector DB dependency.

### D4: Query Embedding Cache
LRU cache of 64 entries keyed by query string. Query embedding is the slow part (~100ms round-trip to Ollama).

## File Changes

| File | Changes |
|------|---------|
| `crates/services/src/file_index.rs` | New table creation, `generate_embeddings()`, `semantic_search()`, `embedding_coverage()`, cosine + RRF logic |
| `crates/services/src/embedding_client.rs` | **NEW** — HTTP client for Ollama `/api/embeddings` |
| `crates/services/Cargo.toml` | No new crates needed (reuse `reqwest`, `serde_json`, `tokio`) |
| `crates/shared/src/lib.rs` | Add `embedding_score: Option<f64>` to `FileSearchResult` if needed |

## Phases

### Phase 1: Schema + Embedding Client
- Add `file_embeddings` table creation to `FileIndexService::new()`
- Create `embedding_client.rs` with `embed_texts(texts: &[String]) -> Result<Vec<Vec<f32>>>`
- Batch support (up to 32 texts per request)

### Phase 2: Embedding Generation
- `generate_embeddings()` method: query files without embeddings, batch embed, store
- Build composite text string per file
- Optional content extraction for text files

### Phase 3: Search Integration
- Intent classifier (heuristic)
- Cosine similarity function
- Reciprocal rank fusion
- `semantic_search()` replaces or wraps `fuzzy_search()`

### Phase 4: Background + Caching
- Query embedding LRU cache
- Expose `embedding_coverage()` for UI status
- Wire into idle-time scheduler (entropy bot integration point)
