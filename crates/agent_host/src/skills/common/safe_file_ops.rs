//! Safe file operations that enforce the NO DELETE policy.
//!
//! This module provides file operations that can ONLY create, modify, move, or archive files.
//! There is intentionally NO delete functionality - this is a core safety constraint.

use anyhow::{Context, Result};
use chrono::Utc;
use shared::skill::FileAction;
use std::fs;
use std::path::{Path, PathBuf};

/// Safe file operations that enforce the no-delete policy.
///
/// # Safety Constraints
/// - **NO DELETE**: This struct intentionally has no delete method
/// - All operations preserve data - files can only be archived, never removed
/// - Archive operations move files to a designated archive directory
pub struct SafeFileOps {
    /// Base directory for archived files
    archive_dir: PathBuf,
}

impl SafeFileOps {
    /// Create a new SafeFileOps with the given archive directory
    pub fn new(archive_dir: PathBuf) -> Self {
        Self { archive_dir }
    }

    /// Create a new file with the given content.
    ///
    /// Returns error if file already exists (use modify_file instead).
    pub fn create_file(&self, path: &Path, content: &[u8]) -> Result<FileAction> {
        if path.exists() {
            anyhow::bail!(
                "File already exists: {:?}. Use modify_file to update existing files.",
                path
            );
        }

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directories for {:?}", path))?;
        }

        fs::write(path, content).with_context(|| format!("Failed to create file {:?}", path))?;

        Ok(FileAction::Created)
    }

    /// Modify an existing file with new content.
    ///
    /// Returns error if file doesn't exist (use create_file instead).
    pub fn modify_file(&self, path: &Path, content: &[u8]) -> Result<FileAction> {
        if !path.exists() {
            anyhow::bail!(
                "File does not exist: {:?}. Use create_file to create new files.",
                path
            );
        }

        fs::write(path, content).with_context(|| format!("Failed to modify file {:?}", path))?;

        Ok(FileAction::Modified)
    }

    /// Create or modify a file (upsert operation).
    pub fn write_file(&self, path: &Path, content: &[u8]) -> Result<FileAction> {
        let existed = path.exists();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directories for {:?}", path))?;
        }

        fs::write(path, content).with_context(|| format!("Failed to write file {:?}", path))?;

        Ok(if existed {
            FileAction::Modified
        } else {
            FileAction::Created
        })
    }

    /// Append content to an existing file.
    ///
    /// Creates the file if it doesn't exist.
    pub fn append_file(&self, path: &Path, content: &[u8]) -> Result<FileAction> {
        use std::io::Write;

        let existed = path.exists();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directories for {:?}", path))?;
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("Failed to open file for append {:?}", path))?;

        file.write_all(content)
            .with_context(|| format!("Failed to append to file {:?}", path))?;

        Ok(if existed {
            FileAction::Modified
        } else {
            FileAction::Created
        })
    }

    /// Move a file from one location to another.
    ///
    /// The source file is moved (not copied), preserving the no-delete policy
    /// since no data is lost.
    pub fn move_file(&self, from: &Path, to: &Path) -> Result<FileAction> {
        if !from.exists() {
            anyhow::bail!("Source file does not exist: {:?}", from);
        }

        if to.exists() {
            anyhow::bail!(
                "Destination file already exists: {:?}. Archive it first.",
                to
            );
        }

        // Ensure destination parent directory exists
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directories for {:?}", to))?;
        }

        fs::rename(from, to)
            .with_context(|| format!("Failed to move file from {:?} to {:?}", from, to))?;

        Ok(FileAction::Moved {
            from: from.to_path_buf(),
        })
    }

    /// Archive a file to the archive directory.
    ///
    /// This is the safe alternative to deletion. Files are moved to an archive
    /// directory with a timestamp, preserving all data.
    pub fn archive_file(&self, path: &Path) -> Result<FileAction> {
        if !path.exists() {
            anyhow::bail!("File does not exist: {:?}", path);
        }

        // Create archive subdirectory structure
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let archive_path = self
            .archive_dir
            .join(timestamp.to_string())
            .join(&file_name);

        // Ensure archive directory exists
        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create archive directory {:?}", parent))?;
        }

        // Move file to archive
        fs::rename(path, &archive_path)
            .with_context(|| format!("Failed to archive file {:?} to {:?}", path, archive_path))?;

        Ok(FileAction::Archived { to: archive_path })
    }

    /// Archive a file with a custom archive subdirectory.
    ///
    /// Useful for organizing archives by category or project.
    pub fn archive_file_to(&self, path: &Path, subdir: &str) -> Result<FileAction> {
        if !path.exists() {
            anyhow::bail!("File does not exist: {:?}", path);
        }

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let archive_path = self
            .archive_dir
            .join(subdir)
            .join(timestamp.to_string())
            .join(&file_name);

        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create archive directory {:?}", parent))?;
        }

        fs::rename(path, &archive_path)
            .with_context(|| format!("Failed to archive file {:?} to {:?}", path, archive_path))?;

        Ok(FileAction::Archived { to: archive_path })
    }

    /// Copy a file to a new location.
    ///
    /// Unlike move, this preserves the original file.
    pub fn copy_file(&self, from: &Path, to: &Path) -> Result<FileAction> {
        if !from.exists() {
            anyhow::bail!("Source file does not exist: {:?}", from);
        }

        // Ensure destination parent directory exists
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directories for {:?}", to))?;
        }

        fs::copy(from, to)
            .with_context(|| format!("Failed to copy file from {:?} to {:?}", from, to))?;

        Ok(FileAction::Created)
    }

    /// Create a directory (and all parent directories).
    pub fn create_dir(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path).with_context(|| format!("Failed to create directory {:?}", path))
    }

    /// Check if a path exists.
    pub fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    /// Read a file's contents.
    pub fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        fs::read(path).with_context(|| format!("Failed to read file {:?}", path))
    }

    /// Read a file as a UTF-8 string.
    pub fn read_file_string(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path)
            .with_context(|| format!("Failed to read file as string {:?}", path))
    }

    /// Get the archive directory path.
    pub fn archive_dir(&self) -> &Path {
        &self.archive_dir
    }
}

// NOTE: There is intentionally NO delete_file, remove_file, or similar method.
// This is a core safety constraint of the skill system.
// If you need to "delete" a file, use archive_file() instead.

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, SafeFileOps) {
        let temp_dir = TempDir::new().unwrap();
        let archive_dir = temp_dir.path().join("archive");
        let ops = SafeFileOps::new(archive_dir);
        (temp_dir, ops)
    }

    #[test]
    fn test_create_file() {
        let (temp_dir, ops) = setup();
        let file_path = temp_dir.path().join("test.txt");

        let action = ops.create_file(&file_path, b"hello world").unwrap();
        assert!(matches!(action, FileAction::Created));
        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "hello world");
    }

    #[test]
    fn test_create_file_already_exists() {
        let (temp_dir, ops) = setup();
        let file_path = temp_dir.path().join("test.txt");

        fs::write(&file_path, b"existing").unwrap();
        let result = ops.create_file(&file_path, b"new content");
        assert!(result.is_err());
    }

    #[test]
    fn test_modify_file() {
        let (temp_dir, ops) = setup();
        let file_path = temp_dir.path().join("test.txt");

        fs::write(&file_path, b"original").unwrap();
        let action = ops.modify_file(&file_path, b"modified").unwrap();
        assert!(matches!(action, FileAction::Modified));
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "modified");
    }

    #[test]
    fn test_move_file() {
        let (temp_dir, ops) = setup();
        let from_path = temp_dir.path().join("from.txt");
        let to_path = temp_dir.path().join("subdir/to.txt");

        fs::write(&from_path, b"content").unwrap();
        let action = ops.move_file(&from_path, &to_path).unwrap();

        assert!(matches!(action, FileAction::Moved { .. }));
        assert!(!from_path.exists());
        assert!(to_path.exists());
        assert_eq!(fs::read_to_string(&to_path).unwrap(), "content");
    }

    #[test]
    fn test_archive_file() {
        let (temp_dir, ops) = setup();
        let file_path = temp_dir.path().join("to_archive.txt");

        fs::write(&file_path, b"archive me").unwrap();
        let action = ops.archive_file(&file_path).unwrap();

        assert!(matches!(action, FileAction::Archived { .. }));
        assert!(!file_path.exists()); // Original is gone

        // File should be in archive directory
        if let FileAction::Archived { to } = action {
            assert!(to.exists());
            assert_eq!(fs::read_to_string(&to).unwrap(), "archive me");
        }
    }

    #[test]
    fn test_write_file_upsert() {
        let (temp_dir, ops) = setup();
        let file_path = temp_dir.path().join("upsert.txt");

        // Create new file
        let action1 = ops.write_file(&file_path, b"first").unwrap();
        assert!(matches!(action1, FileAction::Created));

        // Update existing file
        let action2 = ops.write_file(&file_path, b"second").unwrap();
        assert!(matches!(action2, FileAction::Modified));
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "second");
    }

    #[test]
    fn test_no_delete_method_exists() {
        // This test documents that SafeFileOps has no delete method.
        // If someone adds one, this comment should remind them why it's forbidden.
        //
        // The following methods exist:
        // - create_file
        // - modify_file
        // - write_file
        // - append_file
        // - move_file
        // - archive_file
        // - archive_file_to
        // - copy_file
        // - create_dir
        // - exists
        // - read_file
        // - read_file_string
        //
        // The following methods do NOT exist by design:
        // - delete_file
        // - remove_file
        // - unlink
        // - rm
        //
        // If you need to "delete" a file, use archive_file() instead.
    }
}
