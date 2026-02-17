//! Edge analyzers for computing file relationships in the graph.

use anyhow::Result;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

/// A candidate edge to insert into the file_edges table.
#[derive(Debug, Clone)]
pub struct EdgeCandidate {
    pub source_id: i64,
    pub target_id: i64,
    pub edge_type: String,
    pub strength: f64,
    pub metadata: Option<String>,
}

/// Compute SHA-256 hash of a file's contents. Skips files > 10MB.
pub fn compute_content_hash(path: &Path) -> Result<Option<String>> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > 10 * 1024 * 1024 {
        return Ok(None);
    }
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = format!("{:x}", hasher.finalize());
    Ok(Some(hash))
}

/// Insert or update edges in the file_edges table.
pub fn upsert_edges(conn: &Connection, edges: &[EdgeCandidate]) -> Result<usize> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut count = 0;
    for edge in edges {
        let affected = conn.execute(
            "INSERT INTO file_edges (source_id, target_id, edge_type, strength, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(source_id, target_id, edge_type) DO UPDATE SET
                strength = excluded.strength,
                metadata = excluded.metadata,
                updated_at = excluded.updated_at",
            params![
                edge.source_id,
                edge.target_id,
                edge.edge_type,
                edge.strength,
                edge.metadata,
                now
            ],
        )?;
        count += affected;
    }
    Ok(count)
}

/// Find duplicate files by matching content hashes.
pub fn find_duplicate_edges(conn: &Connection) -> Result<Vec<EdgeCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT a.file_id, b.file_id
         FROM file_content_hashes a
         JOIN file_content_hashes b ON a.sha256 = b.sha256 AND a.file_id < b.file_id",
    )?;
    let edges: Vec<EdgeCandidate> = stmt
        .query_map([], |row| {
            Ok(EdgeCandidate {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                edge_type: "duplicate".to_string(),
                strength: 1.0,
                metadata: None,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(edges)
}

/// Find sibling relationships within directories.
pub fn find_sibling_edges(conn: &Connection) -> Result<Vec<EdgeCandidate>> {
    // Get all files grouped by their parent directory
    let mut stmt = conn.prepare(
        "SELECT id, path, name FROM files ORDER BY path",
    )?;

    struct FileRow {
        id: i64,
        path: String,
        name: String,
    }

    let files: Vec<FileRow> = stmt
        .query_map([], |row| {
            Ok(FileRow {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Group by parent directory
    let mut dir_groups: HashMap<String, Vec<&FileRow>> = HashMap::new();
    for f in &files {
        if let Some(parent) = Path::new(&f.path).parent() {
            let parent_str = parent.to_string_lossy().to_string();
            dir_groups.entry(parent_str).or_default().push(f);
        }
    }

    let mut edges = Vec::new();
    for group in dir_groups.values() {
        if group.len() < 2 || group.len() > 200 {
            continue; // Skip very large directories
        }
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                let a = group[i];
                let b = group[j];
                if let Some(strength) = sibling_strength(&a.name, &b.name) {
                    edges.push(EdgeCandidate {
                        source_id: a.id,
                        target_id: b.id,
                        edge_type: "sibling".to_string(),
                        strength,
                        metadata: None,
                    });
                }
            }
        }
    }
    Ok(edges)
}

/// Compute sibling strength based on naming patterns.
fn sibling_strength(name_a: &str, name_b: &str) -> Option<f64> {
    let stem_a = Path::new(name_a)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let stem_b = Path::new(name_b)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    if stem_a.is_empty() || stem_b.is_empty() {
        return None;
    }

    // Same stem, different extension (e.g., foo.rs and foo.toml)
    if stem_a == stem_b {
        return Some(0.9);
    }

    // Test file pattern (foo.rs and foo_test.rs)
    if stem_b.starts_with(&stem_a) && stem_b[stem_a.len()..].starts_with("_test") {
        return Some(0.85);
    }
    if stem_a.starts_with(&stem_b) && stem_a[stem_b.len()..].starts_with("_test") {
        return Some(0.85);
    }

    // Config pair (config.yaml and config.example.yaml)
    if stem_a.contains(&stem_b) || stem_b.contains(&stem_a) {
        let longer = stem_a.len().max(stem_b.len());
        let shorter = stem_a.len().min(stem_b.len());
        if shorter as f64 / longer as f64 > 0.5 {
            return Some(0.5);
        }
    }

    // README with anything in same dir
    if name_a.to_lowercase().starts_with("readme") || name_b.to_lowercase().starts_with("readme") {
        return Some(0.3);
    }

    None
}

/// Find reference edges by scanning text files for mentions of other filenames.
pub fn find_reference_edges(conn: &Connection) -> Result<Vec<EdgeCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT id, path FROM files WHERE extension IN ('rs', 'py', 'js', 'ts', 'md', 'txt', 'toml', 'yaml', 'yml', 'json', 'html', 'css')",
    )?;

    struct FileRow {
        id: i64,
        path: String,
    }

    let text_files: Vec<FileRow> = stmt
        .query_map([], |row| {
            Ok(FileRow {
                id: row.get(0)?,
                path: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Build a lookup of filename stems -> file IDs
    let mut name_to_ids: HashMap<String, Vec<i64>> = HashMap::new();
    {
        let mut all_stmt = conn.prepare("SELECT id, name FROM files")?;
        let all_files: Vec<(i64, String)> = all_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        for (id, name) in &all_files {
            let stem = Path::new(name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if stem.len() >= 3 {
                name_to_ids.entry(stem).or_default().push(*id);
            }
        }
    }

    let mut edges = Vec::new();
    for file in &text_files {
        let path = Path::new(&file.path);
        // Skip files > 100KB
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > 100 * 1024 {
                continue;
            }
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (stem, target_ids) in &name_to_ids {
            if content.contains(stem.as_str()) {
                for &target_id in target_ids {
                    if target_id != file.id {
                        edges.push(EdgeCandidate {
                            source_id: file.id,
                            target_id,
                            edge_type: "references".to_string(),
                            strength: 0.6,
                            metadata: None,
                        });
                    }
                }
            }
        }
    }
    Ok(edges)
}

/// Analyze git log for co-modification patterns.
pub fn find_comod_edges(repo_root: &Path) -> Result<Vec<EdgeCandidate>> {
    let output = std::process::Command::new("git")
        .args(["log", "--name-only", "--format=%H", "--since=6 months ago"])
        .current_dir(repo_root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok(Vec::new()), // No git or not a repo
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut comod_counts: HashMap<(String, String), u32> = HashMap::new();
    let mut current_files: Vec<String> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            // End of commit block — record co-modifications
            for i in 0..current_files.len() {
                for j in (i + 1)..current_files.len() {
                    let a = &current_files[i];
                    let b = &current_files[j];
                    let key = if a < b {
                        (a.clone(), b.clone())
                    } else {
                        (b.clone(), a.clone())
                    };
                    *comod_counts.entry(key).or_insert(0) += 1;
                }
            }
            current_files.clear();
        } else if line.len() == 40 && line.chars().all(|c| c.is_ascii_hexdigit()) {
            // Commit hash — start new block
            current_files.clear();
        } else {
            current_files.push(line.to_string());
        }
    }
    // Handle last block
    for i in 0..current_files.len() {
        for j in (i + 1)..current_files.len() {
            let a = &current_files[i];
            let b = &current_files[j];
            let key = if a < b {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            };
            *comod_counts.entry(key).or_insert(0) += 1;
        }
    }

    if comod_counts.is_empty() {
        return Ok(Vec::new());
    }

    let max_count = *comod_counts.values().max().unwrap_or(&1) as f64;
    let mut edges = Vec::new();

    // We need to map file paths to IDs — caller will handle this
    // For now, return path-based edges that the caller resolves
    for ((path_a, path_b), count) in &comod_counts {
        let strength = *count as f64 / max_count;
        if strength >= 0.3 {
            // Use negative IDs as placeholder — caller maps paths to real IDs
            edges.push(EdgeCandidate {
                source_id: -1, // placeholder
                target_id: -1, // placeholder
                edge_type: "co_modified".to_string(),
                strength,
                metadata: Some(serde_json::json!({
                    "source_path": path_a,
                    "target_path": path_b,
                    "count": count,
                }).to_string()),
            });
        }
    }

    Ok(edges)
}
