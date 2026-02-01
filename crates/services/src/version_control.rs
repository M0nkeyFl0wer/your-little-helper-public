//! Hidden version control service using git2-rs.
//!
//! Provides automatic versioning of files without exposing git terminology to users.
//! All versions are stored in a hidden `.little-helper/versions` directory.
//!
//! ## Cross-Platform Notes
//! - Works on Windows, macOS, and Linux
//! - On Windows, the .little-helper directory is not auto-hidden (Unix behavior)
//!   Consider using ATTRIB +H in production for true hidden folders

use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use git2::{Commit, ObjectType, Repository, Signature, StatusOptions};
use std::path::{Path, PathBuf};

// Re-export FileVersion from shared crate
pub use shared::version::FileVersion;

/// Version control service for hidden git-based versioning
pub struct VersionControlService {
    /// Root directory being version controlled
    root: PathBuf,
    /// Path to the hidden git repository
    repo_path: PathBuf,
    /// Git repository
    repo: Repository,
}

impl VersionControlService {
    /// Initialize version control for a directory
    pub fn new(root: &Path) -> Result<Self> {
        let repo_path = root.join(".little-helper").join("versions");

        // Create hidden directory
        std::fs::create_dir_all(&repo_path)?;

        // Initialize or open repository
        let repo = if repo_path.join(".git").exists() {
            Repository::open(&repo_path)?
        } else {
            let repo = Repository::init(&repo_path)?;

            // Initial commit
            let sig = Signature::now("Little Helper", "helper@local")?;
            let tree_id = repo.index()?.write_tree()?;

            // Scope the tree borrow so repo can be moved after
            {
                let tree = repo.find_tree(tree_id)?;
                repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    "Initial version tracking setup",
                    &tree,
                    &[],
                )?;
            }

            repo
        };

        Ok(Self {
            root: root.to_path_buf(),
            repo_path,
            repo,
        })
    }

    /// Save a new version of a file
    pub fn save_version(&self, file_path: &Path) -> Result<FileVersion> {
        // Get relative path from root
        let rel_path = file_path.strip_prefix(&self.root).unwrap_or(file_path);

        // Copy file to version control directory
        let dest_path = self.repo_path.join(rel_path);
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(file_path, &dest_path)?;

        // Get file metadata
        let metadata = std::fs::metadata(file_path)?;
        let size_bytes = metadata.len();

        // Stage the file
        let mut index = self.repo.index()?;
        index.add_path(rel_path)?;
        index.write()?;

        // Generate description based on changes
        let description = self.generate_description(rel_path)?;

        // Create commit
        let sig = Signature::now("Little Helper", "helper@local")?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let parent = self.repo.head()?.peel_to_commit()?;
        let commit_id =
            self.repo
                .commit(Some("HEAD"), &sig, &sig, &description, &tree, &[&parent])?;

        // Count versions of this file
        let version_count = self.count_versions(rel_path)?;

        Ok(FileVersion::new(
            version_count,
            Utc::now(),
            description,
            size_bytes,
            commit_id.to_string(),
        )
        .mark_current())
    }

    /// List all versions of a file
    pub fn list_versions(&self, file_path: &Path) -> Result<Vec<FileVersion>> {
        let rel_path = file_path.strip_prefix(&self.root).unwrap_or(file_path);

        let rel_path_str = rel_path.to_string_lossy();
        let mut versions = Vec::new();

        // Walk through commit history
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut version_number = 0u32;
        let current_oid = self.repo.head()?.target();

        for oid in revwalk {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;

            // Check if this commit touched our file
            if self.commit_touches_file(&commit, &rel_path_str)? {
                version_number += 1;

                let timestamp = Utc.timestamp_opt(commit.time().seconds(), 0).unwrap();
                let description = self.user_friendly_description(commit.message().unwrap_or(""));
                let size_bytes = self
                    .file_size_at_commit(&commit, &rel_path_str)
                    .unwrap_or(0);

                let mut version = FileVersion::new(
                    version_number,
                    timestamp,
                    description,
                    size_bytes,
                    oid.to_string(),
                );
                if Some(oid) == current_oid {
                    version = version.mark_current();
                }
                versions.push(version);
            }
        }

        // Reverse so oldest is version 1
        versions.reverse();
        for (i, v) in versions.iter_mut().enumerate() {
            v.version_number = (i + 1) as u32;
        }

        Ok(versions)
    }

    /// Restore a file to a previous version
    pub fn restore_version(&self, file_path: &Path, version: &FileVersion) -> Result<()> {
        let rel_path = file_path.strip_prefix(&self.root).unwrap_or(file_path);

        // First save current state as a new version
        if file_path.exists() {
            self.save_version(file_path)?;
        }

        // Get the file content from the specified version
        let commit_oid = git2::Oid::from_str(&version.commit_ref)?;
        let commit = self.repo.find_commit(commit_oid)?;
        let tree = commit.tree()?;

        let entry = tree
            .get_path(rel_path)
            .context("File not found in version")?;

        let blob = self.repo.find_blob(entry.id())?;

        // Write content to original file
        std::fs::write(file_path, blob.content())?;

        Ok(())
    }

    /// Generate a user-friendly description based on file changes
    fn generate_description(&self, rel_path: &Path) -> Result<String> {
        let file_name = rel_path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        // Check if this is a new file or modification
        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);

        let statuses = self.repo.statuses(Some(&mut status_opts))?;
        let status = statuses
            .iter()
            .find(|s| s.path() == Some(rel_path.to_str().unwrap_or("")));

        let description = match status {
            Some(s) if s.status().is_index_new() => {
                format!("Created {}", file_name)
            }
            Some(_) => {
                format!("Updated {}", file_name)
            }
            None => {
                format!("Saved version of {}", file_name)
            }
        };

        Ok(description)
    }

    /// Convert git commit message to user-friendly description
    fn user_friendly_description(&self, message: &str) -> String {
        // Remove technical prefixes and clean up
        let clean = message.lines().next().unwrap_or(message).trim().to_string();

        if clean.is_empty() {
            "Saved version".to_string()
        } else {
            clean
        }
    }

    /// Check if a commit touched a specific file
    fn commit_touches_file(&self, commit: &Commit, file_path: &str) -> Result<bool> {
        let tree = commit.tree()?;

        // Check if file exists in this commit
        if tree.get_path(Path::new(file_path)).is_ok() {
            // Check if it's different from parent
            if commit.parent_count() > 0 {
                let parent = commit.parent(0)?;
                let parent_tree = parent.tree()?;

                let diff = self
                    .repo
                    .diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;

                for delta in diff.deltas() {
                    if let Some(path) = delta.new_file().path() {
                        if path.to_string_lossy() == file_path {
                            return Ok(true);
                        }
                    }
                }

                Ok(false)
            } else {
                // Initial commit
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    /// Get file size at a specific commit
    fn file_size_at_commit(&self, commit: &Commit, file_path: &str) -> Result<u64> {
        let tree = commit.tree()?;
        let entry = tree.get_path(Path::new(file_path))?;

        if entry.kind() == Some(ObjectType::Blob) {
            let blob = self.repo.find_blob(entry.id())?;
            Ok(blob.size() as u64)
        } else {
            Ok(0)
        }
    }

    /// Count versions of a file
    fn count_versions(&self, rel_path: &Path) -> Result<u32> {
        let versions = self.list_versions(&self.root.join(rel_path))?;
        Ok(versions.len() as u32)
    }

    /// Get the root directory being version controlled
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_version_control_init() {
        let temp_dir = TempDir::new().unwrap();
        let vc = VersionControlService::new(temp_dir.path());
        assert!(vc.is_ok());
    }

    #[test]
    fn test_save_and_list_versions() {
        let temp_dir = TempDir::new().unwrap();
        let vc = VersionControlService::new(temp_dir.path()).unwrap();

        // Create a test file
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Version 1").unwrap();

        // Save first version
        let v1 = vc.save_version(&file_path).unwrap();
        assert_eq!(v1.version_number, 1);

        // Modify and save second version
        std::fs::write(&file_path, "Version 2").unwrap();
        let v2 = vc.save_version(&file_path).unwrap();
        assert_eq!(v2.version_number, 2);

        // List versions
        let versions = vc.list_versions(&file_path).unwrap();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_restore_version() {
        let temp_dir = TempDir::new().unwrap();
        let vc = VersionControlService::new(temp_dir.path()).unwrap();

        let file_path = temp_dir.path().join("test.txt");

        // Create and save first version
        std::fs::write(&file_path, "Version 1").unwrap();
        vc.save_version(&file_path).unwrap();

        // Modify and save second version
        std::fs::write(&file_path, "Version 2").unwrap();
        vc.save_version(&file_path).unwrap();

        // Get versions
        let versions = vc.list_versions(&file_path).unwrap();
        let v1 = &versions[0];

        // Restore to first version
        vc.restore_version(&file_path, v1).unwrap();

        // Verify content
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Version 1");
    }
}
