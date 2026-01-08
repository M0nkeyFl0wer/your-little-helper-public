//! Version tracking types for the hidden version control system.
//!
//! These types are used to represent file versions in a user-friendly way,
//! hiding git terminology from end users.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User-friendly version information (no git terminology exposed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVersion {
    /// Unique version identifier
    pub id: Uuid,
    /// Version number (1, 2, 3, ...) - oldest is 1
    pub version_number: u32,
    /// When this version was created
    pub timestamp: DateTime<Utc>,
    /// Human-readable description of changes
    pub description: String,
    /// File size at this version in bytes
    pub size_bytes: u64,
    /// Whether this is the current (latest) version
    pub is_current: bool,
    /// Internal commit reference (hidden from user display)
    #[serde(skip_serializing)]
    pub commit_ref: String,
}

impl FileVersion {
    /// Create a new FileVersion
    pub fn new(
        version_number: u32,
        timestamp: DateTime<Utc>,
        description: impl Into<String>,
        size_bytes: u64,
        commit_ref: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            version_number,
            timestamp,
            description: description.into(),
            size_bytes,
            is_current: false,
            commit_ref: commit_ref.into(),
        }
    }

    /// Mark this version as current
    pub fn mark_current(mut self) -> Self {
        self.is_current = true;
        self
    }

    /// Format timestamp for display
    pub fn formatted_time(&self) -> String {
        self.timestamp.format("%Y-%m-%d %H:%M").to_string()
    }

    /// Format relative time (e.g., "2 hours ago")
    pub fn relative_time(&self) -> String {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.timestamp);

        if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            let mins = duration.num_minutes();
            if mins == 1 {
                "1 minute ago".to_string()
            } else {
                format!("{} minutes ago", mins)
            }
        } else if duration.num_hours() < 24 {
            let hours = duration.num_hours();
            if hours == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{} hours ago", hours)
            }
        } else if duration.num_days() < 30 {
            let days = duration.num_days();
            if days == 1 {
                "yesterday".to_string()
            } else {
                format!("{} days ago", days)
            }
        } else {
            self.formatted_time()
        }
    }

    /// Format size for display
    pub fn formatted_size(&self) -> String {
        format_size(self.size_bytes)
    }
}

/// A summary of version history for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionHistorySummary {
    /// Path to the file
    pub file_path: String,
    /// File name for display
    pub file_name: String,
    /// Total number of versions
    pub total_versions: usize,
    /// Timestamp of first version
    pub first_version_time: Option<DateTime<Utc>>,
    /// Timestamp of current version
    pub current_version_time: Option<DateTime<Utc>>,
}

/// Format file size in human-readable form
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_version_creation() {
        let version = FileVersion::new(1, Utc::now(), "Initial version", 1024, "abc123");
        assert_eq!(version.version_number, 1);
        assert!(!version.is_current);
    }

    #[test]
    fn test_mark_current() {
        let version = FileVersion::new(1, Utc::now(), "Test", 100, "ref").mark_current();
        assert!(version.is_current);
    }

    #[test]
    fn test_formatted_size() {
        let version = FileVersion::new(1, Utc::now(), "Test", 1536, "ref");
        assert_eq!(version.formatted_size(), "1.5 KB");
    }
}
