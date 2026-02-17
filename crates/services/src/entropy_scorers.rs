//! Entropy scoring functions for measuring file organization quality.

use crate::file_index::FileIndexEntry;
use std::collections::HashMap;
use std::path::Path;

/// Score how inconsistent file naming is within a directory (0.0 = consistent, 1.0 = chaotic).
pub fn naming_entropy(files: &[FileIndexEntry]) -> f64 {
    if files.len() < 2 {
        return 0.0;
    }

    let mut patterns = HashMap::new();
    for f in files {
        let pattern = classify_naming_pattern(&f.name);
        *patterns.entry(pattern).or_insert(0u32) += 1;
    }

    if patterns.len() <= 1 {
        return 0.0; // All same pattern
    }

    // Shannon entropy normalized to 0-1
    let total = files.len() as f64;
    let mut entropy = 0.0;
    for &count in patterns.values() {
        let p = count as f64 / total;
        if p > 0.0 {
            entropy -= p * p.log2();
        }
    }
    let max_entropy = (patterns.len() as f64).log2();
    if max_entropy > 0.0 {
        (entropy / max_entropy).min(1.0)
    } else {
        0.0
    }
}

/// Classify a filename's naming convention.
fn classify_naming_pattern(name: &str) -> &'static str {
    let stem = Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    if stem.contains('-') && stem.chars().all(|c| c.is_lowercase() || c == '-' || c.is_ascii_digit()) {
        "kebab-case"
    } else if stem.contains('_') && stem.chars().all(|c| c.is_lowercase() || c == '_' || c.is_ascii_digit()) {
        "snake_case"
    } else if stem.contains('_') && stem.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit()) {
        "SCREAMING_SNAKE"
    } else if stem.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
        && stem.contains(|c: char| c.is_lowercase())
        && !stem.contains('_') && !stem.contains('-')
    {
        "PascalCase"
    } else if stem.chars().next().map(|c| c.is_lowercase()).unwrap_or(false)
        && stem.contains(|c: char| c.is_uppercase())
        && !stem.contains('_') && !stem.contains('-')
    {
        "camelCase"
    } else if stem.contains(' ') {
        "spaces"
    } else {
        "other"
    }
}

/// Score how spread out file ages are in a directory (0.0 = all same age, 1.0 = wide spread).
pub fn age_spread(files: &[FileIndexEntry]) -> f64 {
    if files.len() < 2 {
        return 0.0;
    }

    let timestamps: Vec<i64> = files.iter().map(|f| f.modified_at.timestamp()).collect();
    let min_ts = *timestamps.iter().min().unwrap();
    let max_ts = *timestamps.iter().max().unwrap();

    let spread_seconds = (max_ts - min_ts) as f64;
    let six_months_seconds = 180.0 * 24.0 * 3600.0;

    (spread_seconds / six_months_seconds).min(1.0)
}

/// Score depth waste — single-child directory chains (0.0 = efficient, 1.0 = wasteful).
pub fn depth_waste(dir_path: &Path) -> f64 {
    let mut current = dir_path.to_path_buf();
    let mut chain_length = 0u32;
    let mut total_depth = 0u32;

    // Walk down checking for single-child directories
    loop {
        total_depth += 1;
        let entries: Vec<_> = match std::fs::read_dir(&current) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(_) => break,
        };
        if entries.len() == 1 && entries[0].path().is_dir() {
            chain_length += 1;
            current = entries[0].path();
        } else {
            break;
        }
        if total_depth > 20 {
            break; // Safety limit
        }
    }

    if total_depth == 0 {
        return 0.0;
    }
    (chain_length as f64 / total_depth as f64).min(1.0)
}

/// Compute composite entropy score from individual dimensions.
pub fn composite_score(
    naming: f64,
    age: f64,
    depth: f64,
    duplicate: f64,
    orphan: f64,
) -> f64 {
    let weights = [0.25, 0.20, 0.15, 0.25, 0.15];
    let scores = [naming, age, depth, duplicate, orphan];
    weights.iter().zip(scores.iter()).map(|(w, s)| w * s).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_naming_pattern_classification() {
        assert_eq!(classify_naming_pattern("hello-world.rs"), "kebab-case");
        assert_eq!(classify_naming_pattern("hello_world.rs"), "snake_case");
        assert_eq!(classify_naming_pattern("HelloWorld.rs"), "PascalCase");
        assert_eq!(classify_naming_pattern("helloWorld.rs"), "camelCase");
        assert_eq!(classify_naming_pattern("HELLO_WORLD.rs"), "SCREAMING_SNAKE");
        assert_eq!(classify_naming_pattern("hello world.txt"), "spaces");
    }

    #[test]
    fn test_composite_all_zero() {
        assert_eq!(composite_score(0.0, 0.0, 0.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn test_composite_all_one() {
        let score = composite_score(1.0, 1.0, 1.0, 1.0, 1.0);
        assert!((score - 1.0).abs() < 0.001);
    }
}
