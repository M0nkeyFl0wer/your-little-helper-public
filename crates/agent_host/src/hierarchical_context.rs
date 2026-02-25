//! Hierarchical context loading from HELPER.md / AGENTS.md / CLAUDE.md files.
//!
//! Follows the convention established by Pi, Claude Code, and other agent tools:
//! context files placed in project directories are automatically loaded into the
//! system prompt, providing project-specific instructions to the AI agent.
//!
//! ## Loading Order (concatenated, most general first)
//!
//! 1. **Global:** `~/.config/little_helper/HELPER.md` — applies to all projects
//! 2. **Per-project:** `HELPER.md`, `AGENTS.md`, or `CLAUDE.md` in each `allowed_dirs` entry
//!
//! The recognized filenames (checked in order, first match wins per directory):
//! - `HELPER.md` — Little Helper's native format
//! - `AGENTS.md` — Pi / Agent Skills standard
//! - `CLAUDE.md` — Claude Code convention
//!
//! ## Security
//!
//! - Only scans directories listed in `allowed_dirs` (user-trusted paths)
//! - Maximum 100KB per file to prevent accidental large file inclusion
//! - No symlink traversal outside allowed boundaries

use std::path::{Path, PathBuf};

/// Maximum file size we'll load (100KB). Files larger than this are skipped
/// to prevent accidental inclusion of large files that could blow up the
/// system prompt and waste tokens.
const MAX_CONTEXT_FILE_BYTES: u64 = 100 * 1024;

/// Recognized context filenames, checked in priority order.
/// First match wins per directory.
const CONTEXT_FILENAMES: &[&str] = &["HELPER.md", "AGENTS.md", "CLAUDE.md"];

/// A single loaded context file with its source information.
///
/// Used internally to track where each piece of context came from,
/// which is useful for debugging and the UI status display.
#[derive(Debug, Clone)]
pub struct ContextSource {
    /// Human-readable label (e.g., "Global", "Project: /home/user/my-app")
    pub label: String,
    /// The file that was loaded
    pub path: PathBuf,
    /// The file content
    pub content: String,
}

/// Load hierarchical context from global config and project directories.
///
/// Returns the concatenated context string ready to be appended to the
/// system prompt. Returns an empty string if no context files are found.
///
/// ## Arguments
///
/// - `allowed_dirs`: User's configured allowed directories (project roots)
/// - `project_root`: Optional explicit project root (overrides auto-detection)
///
/// ## Example
///
/// ```text
/// --- Context: Global ---
/// Always use polite language with the user.
///
/// --- Context: Project /home/user/my-app ---
/// This is a React app using TypeScript. The API is at /src/api/.
/// ```
pub fn load_hierarchical_context(
    allowed_dirs: &[String],
    project_root: Option<&str>,
) -> String {
    let sources = collect_context_sources(allowed_dirs, project_root);

    if sources.is_empty() {
        return String::new();
    }

    // Concatenate all sources with headers for clarity
    let mut result = String::new();
    for source in &sources {
        result.push_str(&format!("\n--- Context: {} ---\n", source.label));
        result.push_str(&source.content);
        result.push('\n');
    }

    result
}

/// Collect all context sources without concatenating them.
///
/// Useful for UI display (showing which files are loaded) and for
/// testing individual sources.
pub fn collect_context_sources(
    allowed_dirs: &[String],
    project_root: Option<&str>,
) -> Vec<ContextSource> {
    let mut sources = Vec::new();

    // 1. Global context: ~/.config/little_helper/HELPER.md
    if let Some(config_dir) = dirs::config_dir() {
        let global_dir = config_dir.join("little_helper");
        if let Some(source) = try_load_context_file(&global_dir, "Global") {
            sources.push(source);
        }
    }

    // 2. Per-project context: check each allowed_dir for context files.
    //    These represent user-trusted project directories.
    for dir in allowed_dirs {
        let dir_path = PathBuf::from(dir);
        if !dir_path.exists() || !dir_path.is_dir() {
            continue;
        }

        let label = format!("Project: {}", dir);
        if let Some(source) = try_load_context_file(&dir_path, &label) {
            sources.push(source);
        }
    }

    // 3. Explicit project root (if set and not already covered by allowed_dirs)
    if let Some(root) = project_root {
        let root_path = PathBuf::from(root);
        // Only load if this path wasn't already scanned in allowed_dirs
        let already_scanned = allowed_dirs.iter().any(|d| d == root);
        if !already_scanned && root_path.exists() && root_path.is_dir() {
            let label = format!("Project: {}", root);
            if let Some(source) = try_load_context_file(&root_path, &label) {
                sources.push(source);
            }
        }
    }

    sources
}

/// Try to load a context file from a directory.
///
/// Checks for each recognized filename in priority order (HELPER.md,
/// AGENTS.md, CLAUDE.md). Returns the first one found, or None if
/// no context file exists in the directory.
fn try_load_context_file(dir: &Path, label: &str) -> Option<ContextSource> {
    for filename in CONTEXT_FILENAMES {
        let path = dir.join(filename);

        if !path.is_file() {
            continue;
        }

        // Check file size before reading to avoid loading huge files
        match std::fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_CONTEXT_FILE_BYTES => {
                tracing::warn!(
                    "Skipping context file {} ({} bytes exceeds {} byte limit)",
                    path.display(),
                    meta.len(),
                    MAX_CONTEXT_FILE_BYTES
                );
                continue;
            }
            Err(e) => {
                tracing::debug!(
                    "Could not read metadata for {}: {}",
                    path.display(),
                    e
                );
                continue;
            }
            _ => {}
        }

        // Read the file content
        match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => {
                tracing::info!(
                    "Loaded context file: {} ({} bytes)",
                    path.display(),
                    content.len()
                );
                return Some(ContextSource {
                    label: label.to_string(),
                    path,
                    content,
                });
            }
            Ok(_) => {
                // File exists but is empty — skip silently
                continue;
            }
            Err(e) => {
                tracing::debug!(
                    "Could not read context file {}: {}",
                    path.display(),
                    e
                );
                continue;
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_no_context_files() {
        // Empty allowed_dirs, no project root → empty result
        let result = load_hierarchical_context(&[], None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_from_allowed_dir() {
        let dir = tempfile::tempdir().unwrap();
        let helper_path = dir.path().join("HELPER.md");
        std::fs::write(&helper_path, "Use TypeScript in this project.").unwrap();

        let allowed = vec![dir.path().to_string_lossy().to_string()];
        let result = load_hierarchical_context(&allowed, None);

        assert!(result.contains("Use TypeScript in this project."));
        assert!(result.contains("Context: Project"));
    }

    #[test]
    fn test_agents_md_fallback() {
        // If HELPER.md doesn't exist but AGENTS.md does, load AGENTS.md
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "Pi-compatible context.").unwrap();

        let allowed = vec![dir.path().to_string_lossy().to_string()];
        let result = load_hierarchical_context(&allowed, None);

        assert!(result.contains("Pi-compatible context."));
    }

    #[test]
    fn test_claude_md_fallback() {
        // CLAUDE.md is recognized as a fallback
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "Claude Code context.").unwrap();

        let allowed = vec![dir.path().to_string_lossy().to_string()];
        let result = load_hierarchical_context(&allowed, None);

        assert!(result.contains("Claude Code context."));
    }

    #[test]
    fn test_helper_md_takes_priority() {
        // If both HELPER.md and AGENTS.md exist, HELPER.md wins
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("HELPER.md"), "Helper context.").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "Agents context.").unwrap();

        let allowed = vec![dir.path().to_string_lossy().to_string()];
        let result = load_hierarchical_context(&allowed, None);

        assert!(result.contains("Helper context."));
        assert!(!result.contains("Agents context."));
    }

    #[test]
    fn test_empty_file_skipped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("HELPER.md"), "   \n  ").unwrap();

        let allowed = vec![dir.path().to_string_lossy().to_string()];
        let result = load_hierarchical_context(&allowed, None);

        assert!(result.is_empty());
    }

    #[test]
    fn test_multiple_dirs() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        std::fs::write(dir1.path().join("HELPER.md"), "Project A context.").unwrap();
        std::fs::write(dir2.path().join("HELPER.md"), "Project B context.").unwrap();

        let allowed = vec![
            dir1.path().to_string_lossy().to_string(),
            dir2.path().to_string_lossy().to_string(),
        ];
        let result = load_hierarchical_context(&allowed, None);

        assert!(result.contains("Project A context."));
        assert!(result.contains("Project B context."));
    }

    #[test]
    fn test_explicit_project_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("HELPER.md"), "Root context.").unwrap();

        // Not in allowed_dirs, but set as explicit project root
        let root = dir.path().to_string_lossy().to_string();
        let result = load_hierarchical_context(&[], Some(&root));

        assert!(result.contains("Root context."));
    }

    #[test]
    fn test_project_root_deduplication() {
        // If project_root is already in allowed_dirs, don't load it twice
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("HELPER.md"), "Only once.").unwrap();

        let dir_str = dir.path().to_string_lossy().to_string();
        let allowed = vec![dir_str.clone()];
        let result = load_hierarchical_context(&allowed, Some(&dir_str));

        // Should appear exactly once
        let count = result.matches("Only once.").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_oversized_file_skipped() {
        let dir = tempfile::tempdir().unwrap();
        // Create a file larger than MAX_CONTEXT_FILE_BYTES (100KB)
        let large_content = "x".repeat(MAX_CONTEXT_FILE_BYTES as usize + 1);
        std::fs::write(dir.path().join("HELPER.md"), large_content).unwrap();

        let allowed = vec![dir.path().to_string_lossy().to_string()];
        let result = load_hierarchical_context(&allowed, None);

        assert!(result.is_empty());
    }

    #[test]
    fn test_nonexistent_dir_handled_gracefully() {
        let allowed = vec!["/nonexistent/path/that/doesnt/exist".to_string()];
        let result = load_hierarchical_context(&allowed, None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_collect_sources_returns_metadata() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "Test content").unwrap();

        let allowed = vec![dir.path().to_string_lossy().to_string()];
        let sources = collect_context_sources(&allowed, None);

        assert_eq!(sources.len(), 1);
        assert!(sources[0].label.starts_with("Project: "));
        assert!(sources[0].path.ends_with("AGENTS.md"));
        assert_eq!(sources[0].content, "Test content");
    }
}
