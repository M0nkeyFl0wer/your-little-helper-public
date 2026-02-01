//! File indexing service for fuzzy search across drives.
//!
//! Uses SQLite FTS5 for fast full-text search with trigram tokenization,
//! combined with strsim for fzf-like fuzzy matching.

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
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
pub struct FileIndexService {
    conn: Arc<Mutex<Connection>>,
    db_path: PathBuf,
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
}

/// Statistics from a scan operation
#[derive(Debug, Clone, Default)]
pub struct ScanStats {
    pub total_files: usize,
    pub indexed: usize,
    pub errors: usize,
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
