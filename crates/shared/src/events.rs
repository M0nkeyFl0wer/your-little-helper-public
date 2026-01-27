//! Event types for skill execution and audit logging.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::skill::{FileAction, Mode};

/// Event types for the audit log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    /// Skill was executed
    SkillExec,
    /// File operation occurred
    FileOp,
    /// Permission was changed
    PermChange,
    /// Error occurred
    Error,
}

/// Audit log entry for tracking all operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry ID
    pub id: Uuid,
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// Type of event
    pub event_type: EventType,
    /// Related skill (if applicable)
    pub skill_id: Option<String>,
    /// Related file (if applicable)
    pub file_path: Option<PathBuf>,
    /// What happened (human-readable)
    pub action: String,
    /// Additional structured data
    pub details: Option<serde_json::Value>,
    /// Whether to show in user-facing settings panel
    pub user_visible: bool,
}

impl AuditEntry {
    /// Create a skill execution audit entry
    pub fn skill_execution(
        skill_id: impl Into<String>,
        action: impl Into<String>,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: EventType::SkillExec,
            skill_id: Some(skill_id.into()),
            file_path: None,
            action: action.into(),
            details,
            user_visible: true,
        }
    }

    /// Create a file operation audit entry
    pub fn file_operation(path: PathBuf, action: FileAction, skill_id: Option<String>) -> Self {
        let action_str = match &action {
            FileAction::Created => "File created".to_string(),
            FileAction::Modified => "File modified".to_string(),
            FileAction::Moved { from } => format!("File moved from {:?}", from),
            FileAction::Archived { to } => format!("File archived to {:?}", to),
        };

        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: EventType::FileOp,
            skill_id,
            file_path: Some(path),
            action: action_str,
            details: Some(serde_json::to_value(&action).unwrap_or_default()),
            user_visible: true,
        }
    }

    /// Create a permission change audit entry
    pub fn permission_change(
        skill_id: impl Into<String>,
        old_permission: impl Into<String>,
        new_permission: impl Into<String>,
    ) -> Self {
        let action = format!(
            "Permission changed from {} to {}",
            old_permission.into(),
            new_permission.into()
        );

        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: EventType::PermChange,
            skill_id: Some(skill_id.into()),
            file_path: None,
            action,
            details: None,
            user_visible: true,
        }
    }

    /// Create an error audit entry
    pub fn error(
        message: impl Into<String>,
        skill_id: Option<String>,
        file_path: Option<PathBuf>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: EventType::Error,
            skill_id,
            file_path,
            action: message.into(),
            details: None,
            user_visible: true,
        }
    }

    /// Mark entry as internal (not shown in settings)
    pub fn internal(mut self) -> Self {
        self.user_visible = false;
        self
    }
}

/// Filter for querying audit logs
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    /// Filter by event type
    pub event_types: Option<Vec<EventType>>,
    /// Filter by skill
    pub skill_id: Option<String>,
    /// Filter by file path prefix
    pub path_prefix: Option<PathBuf>,
    /// Only user-visible entries
    pub user_visible_only: bool,
    /// Start time (inclusive)
    pub from: Option<DateTime<Utc>>,
    /// End time (exclusive)
    pub to: Option<DateTime<Utc>>,
    /// Maximum entries to return
    pub limit: Option<usize>,
}

impl AuditFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn user_visible(mut self) -> Self {
        self.user_visible_only = true;
        self
    }

    pub fn skill(mut self, skill_id: impl Into<String>) -> Self {
        self.skill_id = Some(skill_id.into());
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn event_type(mut self, event_type: EventType) -> Self {
        self.event_types
            .get_or_insert_with(Vec::new)
            .push(event_type);
        self
    }
}

/// Skill execution event for real-time status updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillEvent {
    /// Skill execution started
    Started {
        execution_id: Uuid,
        skill_id: String,
        mode: Mode,
    },
    /// Progress update during execution
    Progress {
        execution_id: Uuid,
        message: String,
        percent: Option<u8>,
    },
    /// Skill execution completed successfully
    Completed {
        execution_id: Uuid,
        duration_ms: u64,
    },
    /// Skill execution failed
    Failed {
        execution_id: Uuid,
        error: String,
        duration_ms: u64,
    },
    /// Skill execution timed out
    Timeout {
        execution_id: Uuid,
        duration_ms: u64,
    },
}

impl SkillEvent {
    pub fn execution_id(&self) -> Uuid {
        match self {
            SkillEvent::Started { execution_id, .. } => *execution_id,
            SkillEvent::Progress { execution_id, .. } => *execution_id,
            SkillEvent::Completed { execution_id, .. } => *execution_id,
            SkillEvent::Failed { execution_id, .. } => *execution_id,
            SkillEvent::Timeout { execution_id, .. } => *execution_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_skill_execution() {
        let entry = AuditEntry::skill_execution(
            "fuzzy_file_search",
            "Searched for 'budget 2024'",
            Some(serde_json::json!({"results": 5})),
        );

        assert_eq!(entry.skill_id, Some("fuzzy_file_search".to_string()));
        assert!(entry.user_visible);
    }

    #[test]
    fn test_audit_entry_file_operation() {
        let entry = AuditEntry::file_operation(
            PathBuf::from("/home/user/doc.txt"),
            FileAction::Archived {
                to: PathBuf::from("/archive/doc.txt"),
            },
            Some("file_organize".to_string()),
        );

        assert!(entry.action.contains("archived"));
    }

    #[test]
    fn test_audit_filter_builder() {
        let filter = AuditFilter::new()
            .user_visible()
            .skill("fuzzy_file_search")
            .limit(100);

        assert!(filter.user_visible_only);
        assert_eq!(filter.skill_id, Some("fuzzy_file_search".to_string()));
        assert_eq!(filter.limit, Some(100));
    }
}
