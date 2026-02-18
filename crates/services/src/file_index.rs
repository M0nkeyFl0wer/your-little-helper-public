//! File indexing service for fuzzy search across drives.
//!
//! Uses SQLite FTS5 for fast full-text search with trigram tokenization,
//! combined with strsim for fzf-like fuzzy matching.

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use strsim::jaro_winkler;
use walkdir::WalkDir;

/// Result from a fuzzy file search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResult {
    /// Full file path
    pub path: PathBuf,
    /// Filename only
    pub name: String,
    /// File extension (if any)
    pub extension: Option<String>,
    /// File size in bytes
    pub size_bytes: i64,
    /// Last modified time
    pub modified_at: DateTime<Utc>,
    /// Search relevance score (0.0 - 1.0)
    pub score: f64,
    /// Embedding similarity score (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_score: Option<f64>,
}

/// Statistics from embedding generation.
#[derive(Debug, Clone, Default)]
pub struct EmbeddingStats {
    pub files_embedded: usize,
    pub files_skipped: usize,
    pub errors: usize,
}

/// Statistics from graph edge computation.
#[derive(Debug, Clone, Default)]
pub struct GraphStats {
    pub sibling_edges: usize,
    pub reference_edges: usize,
    pub duplicate_edges: usize,
    pub total_edges: usize,
}

/// Search intent classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchIntent {
    Filename,
    Semantic,
    Hybrid,
}

/// File index entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIndexEntry {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub extension: Option<String>,
    pub size_bytes: i64,
    pub modified_at: DateTime<Utc>,
    pub drive_id: String,
    pub indexed_at: DateTime<Utc>,
}

/// Service for indexing and searching files across drives
/// Max entries in the query embedding cache.
const QUERY_EMBED_CACHE_SIZE: usize = 64;

pub struct FileIndexService {
    conn: Arc<Mutex<Connection>>,
    db_path: PathBuf,
    /// LRU-ish cache: query string → embedding vector.
    /// Evicts oldest entries when full.
    query_embed_cache: Mutex<Vec<(String, Vec<f32>)>>,
}

impl FileIndexService {
    /// Create a new file index service with database at specified path
    pub fn new(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("file_index.db");

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        // Initialize schema
        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path,
            query_embed_cache: Mutex::new(Vec::new()),
        })
    }

    /// Initialize database schema with FTS5 virtual table
    fn init_schema(conn: &Connection) -> Result<()> {
        // Main files table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                extension TEXT,
                size_bytes INTEGER NOT NULL,
                modified_at INTEGER NOT NULL,
                drive_id TEXT NOT NULL,
                indexed_at INTEGER NOT NULL
            )",
            [],
        )?;

        // FTS5 virtual table for full-text search
        // Using default tokenizer which handles trigrams via prefix search
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
                name,
                path,
                content='files',
                content_rowid='id'
            )",
            [],
        )?;

        // Triggers to keep FTS in sync
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
                INSERT INTO files_fts(rowid, name, path) VALUES (new.id, new.name, new.path);
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
                INSERT INTO files_fts(files_fts, rowid, name, path) VALUES('delete', old.id, old.name, old.path);
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE ON files BEGIN
                INSERT INTO files_fts(files_fts, rowid, name, path) VALUES('delete', old.id, old.name, old.path);
                INSERT INTO files_fts(rowid, name, path) VALUES (new.id, new.name, new.path);
            END",
            [],
        )?;

        // Index for drive queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_drive ON files(drive_id)",
            [],
        )?;

        // Embeddings table for semantic search
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_embeddings (
                file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
                embedding BLOB NOT NULL,
                model_name TEXT NOT NULL,
                embedded_at TEXT NOT NULL,
                content_hash TEXT
            )",
            [],
        )?;

        // File content hashes for duplicate detection
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_content_hashes (
                file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
                sha256 TEXT NOT NULL,
                computed_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_content_hash ON file_content_hashes(sha256)",
            [],
        )?;

        // File relationship graph edges
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                target_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                edge_type TEXT NOT NULL,
                strength REAL NOT NULL DEFAULT 1.0,
                metadata TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_source ON file_edges(source_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_target ON file_edges(target_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_type ON file_edges(edge_type)",
            [],
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_pair ON file_edges(source_id, target_id, edge_type)",
            [],
        )?;

        // Entropy scores for file organization analysis
        conn.execute(
            "CREATE TABLE IF NOT EXISTS entropy_scores (
                file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
                naming_entropy REAL,
                age_spread REAL,
                depth_waste REAL,
                duplicate_ratio REAL,
                orphan_score REAL,
                composite_score REAL NOT NULL,
                computed_at TEXT NOT NULL
            )",
            [],
        )?;

        // Suggestions from entropy bot
        conn.execute(
            "CREATE TABLE IF NOT EXISTS suggestions (
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
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_suggestions_status ON suggestions(status)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_suggestions_type ON suggestions(suggestion_type)",
            [],
        )?;

        Ok(())
    }

    /// Scan a directory and add files to the index
    pub fn scan_drive(&self, root: &Path, drive_id: &str) -> Result<ScanStats> {
        let conn = self.conn.lock().unwrap();
        let indexed_at = Utc::now().timestamp();
        let mut stats = ScanStats::default();

        // Use ignore crate patterns
        let walker = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_hidden(e));

        for entry in walker.filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                stats.total_files += 1;

                let path = entry.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let extension = path.extension().map(|e| e.to_string_lossy().to_string());

                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => {
                        stats.errors += 1;
                        continue;
                    }
                };

                let size_bytes = metadata.len() as i64;
                let modified_at = metadata
                    .modified()
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);

                let path_str = path.to_string_lossy().to_string();

                // Upsert file
                let result = conn.execute(
                    "INSERT INTO files (path, name, extension, size_bytes, modified_at, drive_id, indexed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(path) DO UPDATE SET
                        name = excluded.name,
                        extension = excluded.extension,
                        size_bytes = excluded.size_bytes,
                        modified_at = excluded.modified_at,
                        indexed_at = excluded.indexed_at",
                    params![path_str, name, extension, size_bytes, modified_at, drive_id, indexed_at],
                );

                match result {
                    Ok(_) => stats.indexed += 1,
                    Err(_) => stats.errors += 1,
                }
            }
        }

        Ok(stats)
    }

    /// Fuzzy search for files matching the query
    pub fn fuzzy_search(&self, query: &str, limit: usize) -> Result<Vec<FileSearchResult>> {
        let conn = self.conn.lock().unwrap();

        // Prepare query for FTS5 - add prefix matching
        let fts_query = query
            .split_whitespace()
            .map(|word| format!("{}*", word))
            .collect::<Vec<_>>()
            .join(" ");

        // Search using FTS5
        let mut stmt = conn.prepare(
            "SELECT f.id, f.path, f.name, f.extension, f.size_bytes, f.modified_at
             FROM files_fts fts
             JOIN files f ON fts.rowid = f.id
             WHERE files_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let initial_results: Vec<(i64, String, String, Option<String>, i64, i64)> = stmt
            .query_map(params![fts_query, limit * 2], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Re-rank with Jaro-Winkler for fzf-like matching
        let query_lower = query.to_lowercase();
        let mut results: Vec<FileSearchResult> = initial_results
            .into_iter()
            .map(|(_, path, name, extension, size_bytes, modified_at)| {
                let name_lower = name.to_lowercase();
                let score = jaro_winkler(&query_lower, &name_lower);

                FileSearchResult {
                    path: PathBuf::from(&path),
                    name,
                    extension,
                    size_bytes,
                    modified_at: Utc.timestamp_opt(modified_at, 0).unwrap(),
                    score,
                    embedding_score: None,
                }
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    /// Get the count of indexed files
    pub fn file_count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Get indexed files for a specific drive
    pub fn files_for_drive(&self, drive_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM files WHERE drive_id = ?1",
            params![drive_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Clear all entries for a drive
    pub fn clear_drive(&self, drive_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM files WHERE drive_id = ?1", params![drive_id])?;
        Ok(deleted)
    }

    /// Clear the entire index
    pub fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM files", [])?;
        Ok(())
    }

    /// Get database path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Get the shared database connection for external modules (graph analyzers, entropy bot).
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    // ── Query Embedding Cache ──────────────────────────────────────────

    /// Look up a cached query embedding. Returns `None` on cache miss.
    pub fn get_cached_query_embedding(&self, query: &str) -> Option<Vec<f32>> {
        let mut cache = self.query_embed_cache.lock().unwrap();
        // Move matched entry to the end (most recently used)
        if let Some(pos) = cache.iter().position(|(q, _)| q == query) {
            let entry = cache.remove(pos);
            let embedding = entry.1.clone();
            cache.push(entry);
            Some(embedding)
        } else {
            None
        }
    }

    /// Store a query embedding in the cache. Evicts oldest if full.
    pub fn cache_query_embedding(&self, query: &str, embedding: Vec<f32>) {
        let mut cache = self.query_embed_cache.lock().unwrap();
        // Remove existing entry if present (will be re-added at end)
        cache.retain(|(q, _)| q != query);
        // Evict oldest if at capacity
        if cache.len() >= QUERY_EMBED_CACHE_SIZE {
            cache.remove(0);
        }
        cache.push((query.to_string(), embedding));
    }

    // ── Embedding Methods ──────────────────────────────────────────────

    /// Store an embedding for a file.
    pub fn store_embedding(
        &self,
        file_id: i64,
        embedding: &[f32],
        model_name: &str,
        content_hash: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO file_embeddings (file_id, embedding, model_name, embedded_at, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(file_id) DO UPDATE SET
                embedding = excluded.embedding,
                model_name = excluded.model_name,
                embedded_at = excluded.embedded_at,
                content_hash = excluded.content_hash",
            params![file_id, blob, model_name, now, content_hash],
        )?;
        Ok(())
    }

    /// Get the embedding for a file, decoded from BLOB to Vec<f32>.
    pub fn get_embedding(&self, file_id: i64) -> Result<Option<Vec<f32>>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT embedding FROM file_embeddings WHERE file_id = ?1",
        )?;
        let result = stmt.query_row(params![file_id], |row| {
            let blob: Vec<u8> = row.get(0)?;
            Ok(blob)
        });
        match result {
            Ok(blob) => Ok(Some(decode_embedding(&blob))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Check embedding coverage: (files with embeddings, total files).
    pub fn embedding_coverage(&self) -> Result<(usize, usize)> {
        let conn = self.conn.lock().unwrap();
        let with: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_embeddings",
            [],
            |row| row.get(0),
        )?;
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM files",
            [],
            |row| row.get(0),
        )?;
        Ok((with as usize, total as usize))
    }

    /// Get file IDs that don't have embeddings yet.
    pub fn files_without_embeddings(&self, limit: usize) -> Result<Vec<FileIndexEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT f.id, f.path, f.name, f.extension, f.size_bytes, f.modified_at, f.drive_id, f.indexed_at
             FROM files f
             LEFT JOIN file_embeddings fe ON f.id = fe.file_id
             WHERE fe.file_id IS NULL
             LIMIT ?1",
        )?;
        let entries = stmt
            .query_map(params![limit as i64], |row| {
                let modified_ts: i64 = row.get(5)?;
                let indexed_ts: i64 = row.get(7)?;
                Ok(FileIndexEntry {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    extension: row.get(3)?,
                    size_bytes: row.get(4)?,
                    modified_at: Utc.timestamp_opt(modified_ts, 0).unwrap(),
                    drive_id: row.get(6)?,
                    indexed_at: Utc.timestamp_opt(indexed_ts, 0).unwrap(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    // ── Semantic Search ────────────────────────────────────────────────

    /// Classify search intent based on query characteristics.
    pub fn classify_intent(query: &str) -> SearchIntent {
        let has_extension = query.contains('.');
        let has_path_sep = query.contains('/') || query.contains('\\');
        let has_camel = query.chars().any(|c| c.is_uppercase())
            && query.chars().any(|c| c.is_lowercase())
            && !query.contains(' ');
        let has_snake = query.contains('_') && !query.contains(' ');

        if has_extension || has_path_sep || has_camel || has_snake {
            SearchIntent::Filename
        } else if query.split_whitespace().count() >= 3 {
            SearchIntent::Semantic
        } else {
            SearchIntent::Hybrid
        }
    }

    /// Hybrid search combining FTS5, Jaro-Winkler, and embedding cosine similarity.
    /// Falls back to FTS5+Jaro-Winkler if no embeddings are available.
    pub fn semantic_search(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
        limit: usize,
    ) -> Result<Vec<FileSearchResult>> {
        let intent = Self::classify_intent(query);

        // Signal A+B: FTS5 + Jaro-Winkler (existing path)
        let fts_results = self.fuzzy_search(query, limit * 3)?;

        // Signal C: Cosine similarity (if query embedding available)
        let cosine_results = if let Some(qe) = query_embedding {
            self.cosine_search(qe, limit * 3)?
        } else {
            Vec::new()
        };

        if cosine_results.is_empty() {
            // No embeddings available — return FTS+Jaro-Winkler results
            let mut results = fts_results;
            results.truncate(limit);
            return Ok(results);
        }

        // Reciprocal Rank Fusion
        let k = 60.0;
        let mut rrf_scores: HashMap<String, (f64, FileSearchResult)> = HashMap::new();

        // Weight based on intent
        let (fts_weight, cosine_weight) = match intent {
            SearchIntent::Filename => (2.0, 0.5),
            SearchIntent::Semantic => (0.5, 2.0),
            SearchIntent::Hybrid => (1.0, 1.0),
        };

        for (rank, result) in fts_results.iter().enumerate() {
            let path_key = result.path.to_string_lossy().to_string();
            let rrf = fts_weight / (k + rank as f64 + 1.0);
            let entry = rrf_scores
                .entry(path_key)
                .or_insert((0.0, result.clone()));
            entry.0 += rrf;
        }

        for (rank, result) in cosine_results.iter().enumerate() {
            let path_key = result.path.to_string_lossy().to_string();
            let rrf = cosine_weight / (k + rank as f64 + 1.0);
            let entry = rrf_scores
                .entry(path_key)
                .or_insert((0.0, result.clone()));
            entry.0 += rrf;
            entry.1.embedding_score = result.embedding_score;
        }

        let mut fused: Vec<FileSearchResult> = rrf_scores
            .into_values()
            .map(|(score, mut result)| {
                result.score = score;
                result
            })
            .collect();

        fused.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        fused.truncate(limit);
        Ok(fused)
    }

    /// Search by cosine similarity against stored embeddings.
    fn cosine_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<FileSearchResult>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT fe.file_id, fe.embedding, f.path, f.name, f.extension, f.size_bytes, f.modified_at
             FROM file_embeddings fe
             JOIN files f ON fe.file_id = f.id",
        )?;

        let mut scored: Vec<(f64, FileSearchResult)> = stmt
            .query_map([], |row| {
                let blob: Vec<u8> = row.get(1)?;
                let modified_ts: i64 = row.get(6)?;
                Ok((
                    blob,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    modified_ts,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(|(blob, path, name, extension, size_bytes, modified_ts)| {
                let file_emb = decode_embedding(&blob);
                let sim = cosine_similarity(query_embedding, &file_emb);
                (
                    sim,
                    FileSearchResult {
                        path: PathBuf::from(&path),
                        name,
                        extension,
                        size_bytes,
                        modified_at: Utc.timestamp_opt(modified_ts, 0).unwrap(),
                        score: sim,
                        embedding_score: Some(sim),
                    },
                )
            })
            .collect();

        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        Ok(scored.into_iter().map(|(_, r)| r).collect())
    }

    // ── Graph Methods ──────────────────────────────────────────────────

    /// Compute graph edges using all analyzers.
    pub fn compute_graph_edges(&self) -> Result<GraphStats> {
        let conn = self.conn.lock().unwrap();
        let mut stats = GraphStats::default();

        // Sibling edges
        let sibling_edges = crate::graph_analyzers::find_sibling_edges(&conn)?;
        stats.sibling_edges = sibling_edges.len();
        crate::graph_analyzers::upsert_edges(&conn, &sibling_edges)?;

        // Duplicate edges
        let dup_edges = crate::graph_analyzers::find_duplicate_edges(&conn)?;
        stats.duplicate_edges = dup_edges.len();
        crate::graph_analyzers::upsert_edges(&conn, &dup_edges)?;

        // Reference edges (may be slow for large indices)
        let ref_edges = crate::graph_analyzers::find_reference_edges(&conn)?;
        stats.reference_edges = ref_edges.len();
        crate::graph_analyzers::upsert_edges(&conn, &ref_edges)?;

        stats.total_edges = stats.sibling_edges + stats.duplicate_edges + stats.reference_edges;
        Ok(stats)
    }

    /// Get files related to a given file within N hops via graph edges.
    pub fn related_files(
        &self,
        file_id: i64,
        depth: usize,
        limit: usize,
    ) -> Result<Vec<FileSearchResult>> {
        let conn = self.conn.lock().unwrap();
        let mut visited = std::collections::HashSet::new();
        let mut frontier = vec![(file_id, 0.0_f64)]; // (id, cumulative_strength)
        visited.insert(file_id);

        let mut related = Vec::new();

        for _hop in 0..depth {
            let mut next_frontier = Vec::new();
            for &(current_id, parent_strength) in &frontier {
                let mut stmt = conn.prepare(
                    "SELECT target_id, strength FROM file_edges WHERE source_id = ?1
                     UNION
                     SELECT source_id, strength FROM file_edges WHERE target_id = ?1",
                )?;
                let neighbors: Vec<(i64, f64)> = stmt
                    .query_map(params![current_id], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })?
                    .filter_map(|r| r.ok())
                    .filter(|(id, _)| !visited.contains(id))
                    .collect();

                for (neighbor_id, strength) in neighbors {
                    visited.insert(neighbor_id);
                    let combined = parent_strength + strength;
                    next_frontier.push((neighbor_id, combined));
                }
            }
            frontier = next_frontier;
        }

        // Collect all discovered neighbors
        let mut all_neighbors: Vec<(i64, f64)> = frontier;
        all_neighbors.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all_neighbors.truncate(limit);

        for (neighbor_id, strength) in &all_neighbors {
            let mut stmt = conn.prepare(
                "SELECT path, name, extension, size_bytes, modified_at FROM files WHERE id = ?1",
            )?;
            if let Ok(result) = stmt.query_row(params![neighbor_id], |row| {
                let modified_ts: i64 = row.get(4)?;
                Ok(FileSearchResult {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    name: row.get(1)?,
                    extension: row.get(2)?,
                    size_bytes: row.get(3)?,
                    modified_at: Utc.timestamp_opt(modified_ts, 0).unwrap(),
                    score: *strength,
                    embedding_score: None,
                })
            }) {
                related.push(result);
            }
        }

        Ok(related)
    }

    /// Find duplicate file pairs based on content hashes.
    pub fn find_duplicates(&self) -> Result<Vec<(FileSearchResult, FileSearchResult, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT f1.path, f1.name, f1.extension, f1.size_bytes, f1.modified_at,
                    f2.path, f2.name, f2.extension, f2.size_bytes, f2.modified_at
             FROM file_content_hashes h1
             JOIN file_content_hashes h2 ON h1.sha256 = h2.sha256 AND h1.file_id < h2.file_id
             JOIN files f1 ON h1.file_id = f1.id
             JOIN files f2 ON h2.file_id = f2.id
             LIMIT 100",
        )?;
        let pairs = stmt
            .query_map([], |row| {
                let m1: i64 = row.get(4)?;
                let m2: i64 = row.get(9)?;
                Ok((
                    FileSearchResult {
                        path: PathBuf::from(row.get::<_, String>(0)?),
                        name: row.get(1)?,
                        extension: row.get(2)?,
                        size_bytes: row.get(3)?,
                        modified_at: Utc.timestamp_opt(m1, 0).unwrap(),
                        score: 1.0,
                        embedding_score: None,
                    },
                    FileSearchResult {
                        path: PathBuf::from(row.get::<_, String>(5)?),
                        name: row.get(6)?,
                        extension: row.get(7)?,
                        size_bytes: row.get(8)?,
                        modified_at: Utc.timestamp_opt(m2, 0).unwrap(),
                        score: 1.0,
                        embedding_score: None,
                    },
                    1.0_f64,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(pairs)
    }

    /// Prune weak graph edges below a minimum strength threshold.
    pub fn prune_edges(&self, min_strength: f64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM file_edges WHERE strength < ?1",
            params![min_strength],
        )?;
        Ok(deleted)
    }

    /// Store a content hash for a file.
    pub fn store_content_hash(&self, file_id: i64, hash: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO file_content_hashes (file_id, sha256, computed_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(file_id) DO UPDATE SET
                sha256 = excluded.sha256,
                computed_at = excluded.computed_at",
            params![file_id, hash, now],
        )?;
        Ok(())
    }
}

/// Statistics from a scan operation
#[derive(Debug, Clone, Default)]
pub struct ScanStats {
    pub total_files: usize,
    pub indexed: usize,
    pub errors: usize,
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;
    for i in 0..a.len() {
        let fa = a[i] as f64;
        let fb = b[i] as f64;
        dot += fa * fb;
        norm_a += fa * fa;
        norm_b += fb * fb;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

/// Decode a BLOB of little-endian f32 values.
fn decode_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Build the text string to embed for a file entry.
pub fn build_embedding_text(entry: &FileIndexEntry) -> String {
    let ext = entry.extension.as_deref().unwrap_or("");
    let path_components: Vec<&str> = entry.path.split('/').collect();
    let last_3: Vec<&str> = path_components
        .iter()
        .rev()
        .take(3)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let size_cat = if entry.size_bytes < 1024 {
        "tiny"
    } else if entry.size_bytes < 100 * 1024 {
        "small"
    } else if entry.size_bytes < 10 * 1024 * 1024 {
        "medium"
    } else {
        "large"
    };
    let month_year = entry.modified_at.format("%B %Y").to_string();
    format!(
        "{} | {} | {} | {} | {}",
        entry.name,
        ext,
        last_3.join("/"),
        size_cat,
        month_year
    )
}

/// Check if a directory entry is hidden (starts with .)
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_index() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileIndexService::new(temp_dir.path()).unwrap();
        assert_eq!(service.file_count().unwrap(), 0);
    }

    #[test]
    fn test_scan_and_search() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = TempDir::new().unwrap();

        // Create test files
        std::fs::write(temp_dir.path().join("budget_2024.xlsx"), "test").unwrap();
        std::fs::write(temp_dir.path().join("report.pdf"), "test").unwrap();
        std::fs::write(temp_dir.path().join("quarterly_budget.csv"), "test").unwrap();

        let service = FileIndexService::new(data_dir.path()).unwrap();
        let stats = service.scan_drive(temp_dir.path(), "test_drive").unwrap();

        assert_eq!(stats.indexed, 3);

        // Test fuzzy search
        let results = service.fuzzy_search("budget", 10).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].name.to_lowercase().contains("budget"));
    }
}
