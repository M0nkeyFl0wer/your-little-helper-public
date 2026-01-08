//! Audit logging for skill executions and file operations.
//!
//! Provides JSON-based audit logging with automatic file rotation.
//! Logs are accessible to the primary user through the settings panel.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use shared::events::{AuditEntry, AuditFilter, EventType};
use shared::skill::FileAction;
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Maximum entries to keep in memory for fast access
const MEMORY_CACHE_SIZE: usize = 1000;

/// Maximum log file size before rotation (10 MB)
const MAX_LOG_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Number of rotated log files to keep
const MAX_ROTATED_FILES: usize = 5;

/// Audit logger that writes to JSON files with rotation.
pub struct AuditLogger {
    /// Directory for audit log files
    log_dir: PathBuf,
    /// Current log file path
    current_log: PathBuf,
    /// In-memory cache of recent entries
    cache: RwLock<VecDeque<AuditEntry>>,
}

impl AuditLogger {
    /// Create a new audit logger with the given log directory.
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&log_dir)
            .with_context(|| format!("Failed to create audit log directory {:?}", log_dir))?;

        let current_log = log_dir.join("audit.jsonl");

        let logger = Self {
            log_dir,
            current_log,
            cache: RwLock::new(VecDeque::with_capacity(MEMORY_CACHE_SIZE)),
        };

        // Load recent entries into cache
        logger.load_cache()?;

        Ok(logger)
    }

    /// Log a skill execution event.
    pub fn log_skill_execution(
        &self,
        skill_id: &str,
        action: &str,
        details: Option<serde_json::Value>,
    ) -> Result<()> {
        let entry = AuditEntry::skill_execution(skill_id, action, details);
        self.log_entry(entry)
    }

    /// Log a file operation event.
    pub fn log_file_operation(
        &self,
        path: PathBuf,
        action: FileAction,
        skill_id: Option<String>,
    ) -> Result<()> {
        let entry = AuditEntry::file_operation(path, action, skill_id);
        self.log_entry(entry)
    }

    /// Log a permission change event.
    pub fn log_permission_change(
        &self,
        skill_id: &str,
        old_permission: &str,
        new_permission: &str,
    ) -> Result<()> {
        let entry = AuditEntry::permission_change(skill_id, old_permission, new_permission);
        self.log_entry(entry)
    }

    /// Log an error event.
    pub fn log_error(
        &self,
        message: &str,
        skill_id: Option<String>,
        file_path: Option<PathBuf>,
    ) -> Result<()> {
        let entry = AuditEntry::error(message, skill_id, file_path);
        self.log_entry(entry)
    }

    /// Log a raw audit entry.
    pub fn log_entry(&self, entry: AuditEntry) -> Result<()> {
        // Check if rotation is needed
        self.maybe_rotate()?;

        // Write to file
        let json =
            serde_json::to_string(&entry).with_context(|| "Failed to serialize audit entry")?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.current_log)
            .with_context(|| format!("Failed to open audit log {:?}", self.current_log))?;

        writeln!(file, "{}", json).with_context(|| "Failed to write audit entry")?;

        // Update cache
        let mut cache = self.cache.write();
        if cache.len() >= MEMORY_CACHE_SIZE {
            cache.pop_front();
        }
        cache.push_back(entry);

        Ok(())
    }

    /// Query audit entries with optional filtering.
    pub fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEntry>> {
        let cache = self.cache.read();
        let mut results: Vec<AuditEntry> = cache
            .iter()
            .filter(|entry| self.matches_filter(entry, &filter))
            .cloned()
            .collect();

        // Apply limit
        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        // Results are in chronological order; reverse for most recent first
        results.reverse();

        Ok(results)
    }

    /// Get the most recent N entries.
    pub fn recent(&self, count: usize) -> Vec<AuditEntry> {
        let cache = self.cache.read();
        cache.iter().rev().take(count).cloned().collect()
    }

    /// Get user-visible entries for the settings panel.
    pub fn user_visible_entries(&self, limit: usize) -> Vec<AuditEntry> {
        let cache = self.cache.read();
        cache
            .iter()
            .rev()
            .filter(|e| e.user_visible)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get entries for a specific skill.
    pub fn entries_for_skill(&self, skill_id: &str, limit: usize) -> Vec<AuditEntry> {
        let cache = self.cache.read();
        cache
            .iter()
            .rev()
            .filter(|e| e.skill_id.as_deref() == Some(skill_id))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get file operation entries for a path.
    pub fn entries_for_path(&self, path: &Path, limit: usize) -> Vec<AuditEntry> {
        let cache = self.cache.read();
        cache
            .iter()
            .rev()
            .filter(|e| e.file_path.as_ref().map_or(false, |p| p == path))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get the current log file path.
    pub fn log_file(&self) -> &Path {
        &self.current_log
    }

    /// Get all log files (current + rotated).
    pub fn all_log_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = vec![self.current_log.clone()];

        for i in 1..=MAX_ROTATED_FILES {
            let rotated = self.log_dir.join(format!("audit.{}.jsonl", i));
            if rotated.exists() {
                files.push(rotated);
            }
        }

        Ok(files)
    }

    /// Check if a filter matches an entry.
    fn matches_filter(&self, entry: &AuditEntry, filter: &AuditFilter) -> bool {
        // Event type filter
        if let Some(ref types) = filter.event_types {
            let entry_type = match entry.event_type {
                EventType::SkillExec => EventType::SkillExec,
                EventType::FileOp => EventType::FileOp,
                EventType::PermChange => EventType::PermChange,
                EventType::Error => EventType::Error,
            };
            if !types
                .iter()
                .any(|t| std::mem::discriminant(t) == std::mem::discriminant(&entry_type))
            {
                return false;
            }
        }

        // Skill filter
        if let Some(ref skill_id) = filter.skill_id {
            if entry.skill_id.as_deref() != Some(skill_id.as_str()) {
                return false;
            }
        }

        // Path prefix filter
        if let Some(ref prefix) = filter.path_prefix {
            if let Some(ref path) = entry.file_path {
                if !path.starts_with(prefix) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // User visible filter
        if filter.user_visible_only && !entry.user_visible {
            return false;
        }

        // Time range filters
        if let Some(from) = filter.from {
            if entry.timestamp < from {
                return false;
            }
        }

        if let Some(to) = filter.to {
            if entry.timestamp >= to {
                return false;
            }
        }

        true
    }

    /// Load recent entries from disk into the cache.
    fn load_cache(&self) -> Result<()> {
        if !self.current_log.exists() {
            return Ok(());
        }

        let file = File::open(&self.current_log)
            .with_context(|| format!("Failed to open audit log {:?}", self.current_log))?;

        let reader = BufReader::new(file);
        let mut cache = self.cache.write();

        for line in reader.lines() {
            let line = line.with_context(|| "Failed to read audit log line")?;
            if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
                if cache.len() >= MEMORY_CACHE_SIZE {
                    cache.pop_front();
                }
                cache.push_back(entry);
            }
        }

        Ok(())
    }

    /// Rotate log files if current file exceeds max size.
    fn maybe_rotate(&self) -> Result<()> {
        if !self.current_log.exists() {
            return Ok(());
        }

        let metadata = fs::metadata(&self.current_log)
            .with_context(|| format!("Failed to get metadata for {:?}", self.current_log))?;

        if metadata.len() < MAX_LOG_FILE_SIZE {
            return Ok(());
        }

        // Rotate existing files
        for i in (1..MAX_ROTATED_FILES).rev() {
            let from = self.log_dir.join(format!("audit.{}.jsonl", i));
            let to = self.log_dir.join(format!("audit.{}.jsonl", i + 1));
            if from.exists() {
                fs::rename(&from, &to)
                    .with_context(|| format!("Failed to rotate {:?} to {:?}", from, to))?;
            }
        }

        // Move current to .1
        let first_rotated = self.log_dir.join("audit.1.jsonl");
        fs::rename(&self.current_log, &first_rotated)
            .with_context(|| format!("Failed to rotate current log to {:?}", first_rotated))?;

        // Delete oldest if exceeds max
        let oldest = self
            .log_dir
            .join(format!("audit.{}.jsonl", MAX_ROTATED_FILES + 1));
        if oldest.exists() {
            fs::remove_file(&oldest)
                .with_context(|| format!("Failed to remove oldest log {:?}", oldest))?;
        }

        Ok(())
    }
}

/// Statistics about audit log entries.
#[derive(Debug, Clone, Default)]
pub struct AuditStats {
    pub total_entries: usize,
    pub skill_executions: usize,
    pub file_operations: usize,
    pub permission_changes: usize,
    pub errors: usize,
    pub oldest_entry: Option<DateTime<Utc>>,
    pub newest_entry: Option<DateTime<Utc>>,
}

impl AuditLogger {
    /// Get statistics about the audit log.
    pub fn stats(&self) -> AuditStats {
        let cache = self.cache.read();
        let mut stats = AuditStats {
            total_entries: cache.len(),
            ..Default::default()
        };

        for entry in cache.iter() {
            match entry.event_type {
                EventType::SkillExec => stats.skill_executions += 1,
                EventType::FileOp => stats.file_operations += 1,
                EventType::PermChange => stats.permission_changes += 1,
                EventType::Error => stats.errors += 1,
            }
        }

        stats.oldest_entry = cache.front().map(|e| e.timestamp);
        stats.newest_entry = cache.back().map(|e| e.timestamp);

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, AuditLogger) {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();
        (temp_dir, logger)
    }

    #[test]
    fn test_log_skill_execution() {
        let (_temp_dir, logger) = setup();

        logger
            .log_skill_execution(
                "fuzzy_file_search",
                "Searched for 'budget 2024'",
                Some(serde_json::json!({"results": 5})),
            )
            .unwrap();

        let recent = logger.recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].skill_id, Some("fuzzy_file_search".to_string()));
    }

    #[test]
    fn test_log_file_operation() {
        let (_temp_dir, logger) = setup();

        logger
            .log_file_operation(
                PathBuf::from("/home/user/doc.txt"),
                FileAction::Archived {
                    to: PathBuf::from("/archive/doc.txt"),
                },
                Some("file_organize".to_string()),
            )
            .unwrap();

        let recent = logger.recent(1);
        assert_eq!(recent.len(), 1);
        assert!(recent[0].action.contains("archived"));
    }

    #[test]
    fn test_query_with_filter() {
        let (_temp_dir, logger) = setup();

        // Log some entries
        logger
            .log_skill_execution("skill_a", "Action A", None)
            .unwrap();
        logger
            .log_skill_execution("skill_b", "Action B", None)
            .unwrap();
        logger
            .log_skill_execution("skill_a", "Action C", None)
            .unwrap();

        // Query for skill_a only
        let filter = AuditFilter::new().skill("skill_a");
        let results = logger.query(filter).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|e| e.skill_id == Some("skill_a".to_string())));
    }

    #[test]
    fn test_user_visible_entries() {
        let (_temp_dir, logger) = setup();

        // Log visible entry
        logger
            .log_skill_execution("visible_skill", "Visible action", None)
            .unwrap();

        // Log internal entry
        let internal_entry =
            AuditEntry::skill_execution("internal_skill", "Internal action", None).internal();
        logger.log_entry(internal_entry).unwrap();

        let visible = logger.user_visible_entries(10);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].skill_id, Some("visible_skill".to_string()));
    }

    #[test]
    fn test_cache_limit() {
        let (_temp_dir, logger) = setup();

        // Log more than cache size
        for i in 0..MEMORY_CACHE_SIZE + 100 {
            logger
                .log_skill_execution("test", &format!("Action {}", i), None)
                .unwrap();
        }

        let cache = logger.cache.read();
        assert_eq!(cache.len(), MEMORY_CACHE_SIZE);
    }

    #[test]
    fn test_stats() {
        let (_temp_dir, logger) = setup();

        logger.log_skill_execution("skill", "exec", None).unwrap();
        logger
            .log_file_operation(PathBuf::from("/file"), FileAction::Created, None)
            .unwrap();
        logger.log_error("Error!", None, None).unwrap();

        let stats = logger.stats();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.skill_executions, 1);
        assert_eq!(stats.file_operations, 1);
        assert_eq!(stats.errors, 1);
    }
}
