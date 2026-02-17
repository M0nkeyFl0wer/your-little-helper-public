# Feature: Semantic Search Layer

## 1. Overview

Add embedding-based semantic search to the existing file index, enabling users to find files by meaning rather than just filename matching. Combines FTS5 full-text search, Jaro-Winkler fuzzy matching, cosine similarity on embeddings, and fzf-style interactive filtering through reciprocal rank fusion.

## 2. Problem Statement

The current file search uses FTS5 prefix matching + Jaro-Winkler re-ranking on filenames only. Users with large, unstructured file collections cannot find documents by concept or content — only by remembering exact (or close) filenames. This fails for:
- Files with cryptic names (IMG_20240301.jpg, doc_final_v3.pdf)
- Semantic queries ("that budget spreadsheet from Q3", "the photo from the beach trip")
- Cross-format discovery (finding related files across .md, .pdf, .txt)

## 3. Requirements

### 3.1 Embedding Generation
- R1: Generate embeddings for indexed files using a local model (nomic-embed-text via Ollama, or a bundled ONNX model)
- R2: Embed file metadata: name, path components, extension, size category, modification date context
- R3: Optionally embed file content for text-based files (.md, .txt, .rs, .py, .json, .toml, .yaml) — first 2048 tokens
- R4: Store embeddings in SQLite as BLOB (f32 array, 768 dimensions for nomic-embed-text)
- R5: Incremental embedding — only compute for new/modified files since last index run
- R6: Graceful degradation — if embedding model unavailable, fall back to FTS5+Jaro-Winkler only

### 3.2 Hybrid Search
- R7: Query embedding computed at search time using same model
- R8: Three-signal fusion:
  - Signal A: FTS5 prefix match score (existing)
  - Signal B: Jaro-Winkler filename similarity (existing)
  - Signal C: Cosine similarity between query embedding and file embedding (new)
- R9: Reciprocal Rank Fusion (RRF) to merge ranked lists: `score = Σ 1/(k + rank_i)` where k=60
- R10: Results returned as `Vec<FileSearchResult>` with composite score

### 3.3 Intent Classification
- R11: Classify query intent before search:
  - `filename` — user typed something that looks like a filename → weight signals A+B heavily
  - `semantic` — natural language query → weight signal C heavily
  - `hybrid` — ambiguous → equal weights
- R12: Intent detection via simple heuristics (contains extension, path separator, camelCase/snake_case → filename; otherwise semantic)

### 3.4 Storage Schema
- R13: New `file_embeddings` table:
  ```sql
  CREATE TABLE file_embeddings (
    file_id INTEGER PRIMARY KEY REFERENCES files(id),
    embedding BLOB NOT NULL,        -- f32 x 768 = 3072 bytes
    model_name TEXT NOT NULL,        -- "nomic-embed-text"
    embedded_at TEXT NOT NULL,       -- ISO 8601
    content_hash TEXT                -- SHA256 of embedded text (for change detection)
  );
  ```
- R14: Index on `file_id` for fast joins with `files` table

### 3.5 Performance
- R15: Search latency < 200ms for collections up to 500K files
- R16: Embedding computation runs in background (non-blocking UI)
- R17: Batch embedding requests (32 files per batch) to amortize model overhead
- R18: Cache query embeddings for repeated/similar queries (LRU, 64 entries)

## 4. Non-Requirements
- No cloud embedding APIs (local-only for privacy)
- No full-content indexing for binary files (images, videos, executables)
- No real-time embedding on file write (batch only)
- No training or fine-tuning of embedding models

## 5. API Surface

```rust
// New methods on FileIndexService
impl FileIndexService {
    /// Compute and store embeddings for files missing them.
    pub async fn generate_embeddings(&self, batch_size: usize) -> Result<EmbeddingStats>;

    /// Hybrid search: FTS5 + Jaro-Winkler + cosine similarity with RRF.
    pub async fn semantic_search(&self, query: &str, limit: usize) -> Result<Vec<FileSearchResult>>;

    /// Check embedding coverage (how many files have embeddings vs total).
    pub fn embedding_coverage(&self) -> Result<(usize, usize)>;
}
```

## 6. Dependencies
- `ollama` crate or HTTP client for embedding API (`POST /api/embeddings`)
- No new external crates for cosine similarity (manual dot product on f32 slices)
- SQLite BLOB storage (existing `rusqlite`)

## 7. Migration Path
- New table added via `CREATE TABLE IF NOT EXISTS` on startup
- Zero-downtime: search works without embeddings (graceful degradation per R6)
- Embeddings populated incrementally during idle time or on explicit "reindex" command
