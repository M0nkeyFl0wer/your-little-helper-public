//! Security primitives for command execution sandboxing and session authentication.
//!
//! Two independent mechanisms live here:
//!
//! - **`PathSandbox`** -- restricts all file-touching commands to a set of
//!   user-approved directories.  Every path token in a command is resolved
//!   and checked *before* the shell is spawned, preventing directory
//!   traversal even when the LLM crafts creative paths.
//!
//! - **`SecurityContext`** -- time-boxed TOTP session gate.  Destructive
//!   commands (rm, chmod, kill, ...) require an active authentication
//!   window before the executor will proceed. The window expires after a
//!   configurable timeout (default 15 minutes) to limit blast radius.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Tracks whether a TOTP verification has occurred recently enough to
/// permit destructive operations. Uses an atomic timestamp so the check
/// is lock-free and safe to share across async tasks.
#[derive(Debug)]
pub struct SecurityContext {
    /// Unix epoch seconds of last successful verification; 0 means never.
    last_auth_time: AtomicI64,
    /// How long a successful verification remains valid.
    auth_timeout: Duration,
}

impl SecurityContext {
    pub fn new(timeout_mins: u64) -> Self {
        Self {
            last_auth_time: AtomicI64::new(0),
            auth_timeout: Duration::from_secs(timeout_mins * 60),
        }
    }

    /// Mark the session as authenticated NOW.
    pub fn authenticate(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        self.last_auth_time.store(now, Ordering::Relaxed);
    }

    /// Check if the session is currently authenticated (within timeout).
    pub fn is_authenticated(&self) -> bool {
        let last = self.last_auth_time.load(Ordering::Relaxed);
        if last == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let elapsed = now - last;
        // Check if elapsed is positive and within timeout
        elapsed >= 0 && elapsed < self.auth_timeout.as_secs() as i64
    }
}

/// Filesystem access guard. All allowed directories are canonicalised at
/// construction time so that symlink tricks and `..` traversals cannot
/// escape the sandbox at check time. An empty `allowed_dirs` list means
/// *everything* is blocked -- fail-closed by design.
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

        Self {
            allowed_dirs: canonical_dirs,
        }
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
        self.allowed_dirs
            .iter()
            .any(|allowed| canonical_path.starts_with(allowed))
    }

    /// Heuristic scan of a command string for path tokens that escape the
    /// sandbox. Not a full shell parser -- it splits on whitespace, skips
    /// flag tokens, and treats anything with a path separator (or `cd`
    /// arguments) as a candidate path to validate. This catches the vast
    /// majority of real-world commands without needing a proper shell AST.
    pub fn validate_command(&self, cmd: &str, cwd: &Path) -> Result<(), String> {
        let tokens: Vec<&str> = cmd.split_whitespace().collect();

        let mut prev: Option<&str> = None;
        for token in tokens {
            // Skip flags
            if token.starts_with('-') {
                prev = Some(token);
                continue;
            }

            let is_cd_arg = prev == Some("cd");

            // Heuristic: If it has a separator, it MIGHT be a path
            // Special-case: `cd <arg>` should always be treated as a path.
            if is_cd_arg
                || token.contains('/')
                || token.contains('\\')
                || token == "~"
                || token.starts_with("~/")
            {
                // Try to resolve it (lightweight; not a shell parser).
                let path = if token == "~" {
                    dirs::home_dir().unwrap_or_else(|| cwd.to_path_buf())
                } else if let Some(rest) = token.strip_prefix("~/") {
                    dirs::home_dir()
                        .unwrap_or_else(|| cwd.to_path_buf())
                        .join(rest)
                } else {
                    PathBuf::from(token)
                };
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

            prev = Some(token);
        }

        Ok(())
    }
}
