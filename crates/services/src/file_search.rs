use anyhow::Result;
use ignore::WalkBuilder;
use shared::search_types::{SearchQuery, SearchResult};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FinderOptions {
    pub allowed_dirs: Vec<PathBuf>,
    pub max_results: usize,
}

fn score_name(name: &str, query: &str) -> Option<f32> {
    let n = name.to_lowercase();
    let q = query.to_lowercase();
    if let Some(idx) = n.find(&q) {
        let proximity = 1.0 - (idx as f32 / (n.len().max(1) as f32));
        let len_bonus = (q.len() as f32 / n.len().max(1) as f32).min(0.5);
        Some(0.5 + proximity * 0.4 + len_bonus * 0.1)
    } else {
        None
    }
}

pub fn search(opts: FinderOptions, query: SearchQuery) -> Result<Vec<SearchResult>> {
    let mut results: Vec<SearchResult> = Vec::new();
    let exts = query
        .extensions
        .as_ref()
        .map(|v| v.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>());

    for dir in opts.allowed_dirs {
        let walker = WalkBuilder::new(dir)
            .hidden(false)
            .ignore(true)
            .git_ignore(true)
            .git_exclude(true)
            .build();

        for dent in walker {
            let dent = match dent {
                Ok(d) => d,
                Err(_) => continue,
            };
            let path = dent.path();
            if !path.is_file() {
                continue;
            }
            let file_name = match path.file_name().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            if let Some(exts) = &exts {
                if let Some(ext) = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_lowercase())
                {
                    if !exts.iter().any(|e| e == &ext) {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            if let Some(score) = score_name(&file_name, &query.text) {
                let meta = fs::metadata(path);
                let (size, modified) = match meta {
                    Ok(m) => {
                        let size = m.len();
                        let ts = m
                            .modified()
                            .ok()
                            .and_then(|t| t.elapsed().ok())
                            .map(|e| chrono::Utc::now().timestamp() - e.as_secs() as i64);
                        (size, ts)
                    }
                    Err(_) => (0, None),
                };
                results.push(SearchResult {
                    path: path.to_string_lossy().into_owned(),
                    file_name,
                    size_bytes: size,
                    modified,
                    score,
                });
            }
        }
    }

    // Sort by score desc, then recent first
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.modified.cmp(&a.modified))
    });
    results.truncate(opts.max_results);
    Ok(results)
}
