//! Security module for command execution sandboxing.
//!
//! Provides `PathSandbox` to enforce file system access restrictions.

use std::path::{Path, PathBuf};
use shared::settings::AppSettings;

/// A sandbox that restricts file access to specific allowed directories.
#[derive(Debug, Clone)]
pub struct PathSandbox {
    allowed_dirs: Vec<PathBuf>,
}

impl PathSandbox {
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        // Canonicalize all allowed dirs for robust checking
        let canonical_dirs = allowed_dirs
            .into_iter()
            .filter_map(|p| p.canonicalize().ok())
            .collect();
        
        Self { allowed_dirs: canonical_dirs }
    }

    /// Check if a path is allowed (must be within one of the allowed dirs)
    pub fn is_allowed(&self, path: &Path) -> bool {
        if self.allowed_dirs.is_empty() {
             // If no dirs are explicitly allowed, BLOCK EVERYTHING
             return false;
        }

        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // If path doesn't exist, try to resolve it relative to CWD or check parent
                // For now, assume if we can't canonicalize it (e.g. creating new file), 
                // we check the parent.
                if let Some(parent) = path.parent() {
                    if let Ok(p) = parent.canonicalize() {
                        return self.is_allowed_canonical(&p);
                    }
                }
                // Fallback: strictly deny if we can't resolve
                return false;
            }
        };

        self.is_allowed_canonical(&canonical)
    }

    fn is_allowed_canonical(&self, canonical_path: &Path) -> bool {
        self.allowed_dirs.iter().any(|allowed| canonical_path.starts_with(allowed))
    }

    /// Scan a command string for potential path violations.
    /// 
    /// This is a heuristic scan. It splits by whitespace and treats any token
    /// containing a path separator as a candidate path.
    ///
    /// Returns:
    /// - Ok(()) if all detected paths are safe
    /// - Err(String) with a message describing the violation
    pub fn validate_command(&self, cmd: &str, cwd: &Path) -> Result<(), String> {
        // Simple tokenizer - smart enough for basic shell commands
        // Ignores flags (--foo, -f)
        let tokens: Vec<&str> = cmd.split_whitespace().collect();
        
        for token in tokens {
            // Skip flags
            if token.starts_with('-') {
                continue;
            }

            // Heuristic: If it has a separator, it MIGHT be a path
            if token.contains('/') || token.contains('\\') {
                // Try to resolve it
                let path = PathBuf::from(token);
                let abs_path = if path.is_absolute() {
                    path
                } else {
                    cwd.join(path)
                };

                if !self.is_allowed(&abs_path) {
                    return Err(format!(
                        "Access denied: Path '{}' is outside allowed directories.", 
                        token
                    ));
                }
            }
        }
        
        Ok(())
    }
}
