//! Skill system types and traits for agent tool execution.
//!
//! This module defines the core abstractions for skills that agents can invoke.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Permission level for skills
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionLevel {
    /// Auto-approved, runs without confirmation
    Safe,
    /// Requires per-session confirmation before execution
    Sensitive,
}

/// User's permission setting for a skill
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    Enabled,
    Disabled,
    Ask,
}

impl Default for Permission {
    fn default() -> Self {
        Permission::Ask
    }
}

/// Agent modes that can use skills
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    Find,
    Fix,
    Research,
    Data,
    Content,
    Build,
}

impl Mode {
    pub fn all() -> &'static [Mode] {
        &[
            Mode::Find,
            Mode::Fix,
            Mode::Research,
            Mode::Data,
            Mode::Content,
            Mode::Build,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Mode::Find => "Find",
            Mode::Fix => "Fix",
            Mode::Research => "Research",
            Mode::Data => "Data",
            Mode::Content => "Content",
            Mode::Build => "Build",
        }
    }
}

/// Skill execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Timeout,
}

/// Result type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultType {
    Text,
    Files,
    Data,
    Mixed,
    Error,
}

/// Input to a skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInput {
    /// Natural language request from user
    pub query: String,
    /// Structured parameters (skill-specific)
    pub params: HashMap<String, serde_json::Value>,
    /// Files/images added to context
    pub context_files: Vec<PathBuf>,
    /// Current conversation context (last N messages)
    pub conversation: Vec<String>,
}

impl SkillInput {
    pub fn from_query(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            params: HashMap::new(),
            context_files: Vec::new(),
            conversation: Vec::new(),
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    pub fn with_file(mut self, path: PathBuf) -> Self {
        self.context_files.push(path);
        self
    }
}

/// File action result (NO DELETE - by design)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileAction {
    Created,
    Modified,
    Moved { from: PathBuf },
    Archived { to: PathBuf },
    // Note: There is intentionally NO Delete variant
}

/// A file result from skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    pub path: PathBuf,
    pub action: FileAction,
    pub preview: Option<String>,
}

/// Citation for research results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub text: String,
    pub url: String,
    pub accessed_at: DateTime<Utc>,
    pub verified: bool,
}

/// Suggested follow-up action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    pub label: String,
    pub skill_id: String,
    pub params: HashMap<String, serde_json::Value>,
}

/// Output from a skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    /// Primary result type
    pub result_type: ResultType,
    /// Text response for display
    pub text: Option<String>,
    /// Files generated or modified
    pub files: Vec<FileResult>,
    /// Structured data (for Data mode)
    pub data: Option<serde_json::Value>,
    /// Citations/sources (for Research mode)
    pub citations: Vec<Citation>,
    /// Follow-up actions suggested
    pub suggested_actions: Vec<SuggestedAction>,
}

impl SkillOutput {
    pub fn text(message: impl Into<String>) -> Self {
        Self {
            result_type: ResultType::Text,
            text: Some(message.into()),
            files: Vec::new(),
            data: None,
            citations: Vec::new(),
            suggested_actions: Vec::new(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            result_type: ResultType::Error,
            text: Some(message.into()),
            files: Vec::new(),
            data: None,
            citations: Vec::new(),
            suggested_actions: Vec::new(),
        }
    }

    pub fn with_citation(mut self, citation: Citation) -> Self {
        self.citations.push(citation);
        self
    }

    pub fn with_file(mut self, file: FileResult) -> Self {
        self.files.push(file);
        if self.result_type == ResultType::Text {
            self.result_type = ResultType::Mixed;
        }
        self
    }
}

/// Context provided to skills during execution
pub struct SkillContext {
    /// Current mode
    pub mode: Mode,
    /// Session approval cache (for Sensitive skills)
    pub session_approvals: Arc<RwLock<HashSet<String>>>,
    /// App data directory for storing skill data
    pub data_dir: PathBuf,
    /// Current working directory for file operations
    pub working_dir: PathBuf,
}

impl SkillContext {
    pub fn new(mode: Mode, data_dir: PathBuf) -> Self {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| data_dir.clone());
        Self {
            mode,
            session_approvals: Arc::new(RwLock::new(HashSet::new())),
            data_dir,
            working_dir,
        }
    }

    /// Create context with a specific working directory
    pub fn with_working_dir(mode: Mode, data_dir: PathBuf, working_dir: PathBuf) -> Self {
        Self {
            mode,
            session_approvals: Arc::new(RwLock::new(HashSet::new())),
            data_dir,
            working_dir,
        }
    }

    /// Check if a skill is approved for this session
    pub fn is_session_approved(&self, skill_id: &str) -> bool {
        self.session_approvals.read().contains(skill_id)
    }

    /// Grant session approval for a skill
    pub fn approve_session(&self, skill_id: &str) {
        self.session_approvals.write().insert(skill_id.to_string());
    }
}

/// Record of a skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecution {
    pub id: Uuid,
    pub skill_id: String,
    pub mode: Mode,
    pub timestamp: DateTime<Utc>,
    pub input: SkillInput,
    pub output: Option<SkillOutput>,
    pub status: ExecutionStatus,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl SkillExecution {
    pub fn new(skill_id: impl Into<String>, mode: Mode, input: SkillInput) -> Self {
        Self {
            id: Uuid::new_v4(),
            skill_id: skill_id.into(),
            mode,
            timestamp: Utc::now(),
            input,
            output: None,
            status: ExecutionStatus::Running,
            duration_ms: 0,
            error: None,
        }
    }

    pub fn complete(mut self, output: SkillOutput, duration_ms: u64) -> Self {
        self.status = ExecutionStatus::Completed;
        self.output = Some(output);
        self.duration_ms = duration_ms;
        self
    }

    pub fn fail(mut self, error: impl Into<String>, duration_ms: u64) -> Self {
        self.status = ExecutionStatus::Failed;
        self.error = Some(error.into());
        self.duration_ms = duration_ms;
        self
    }

    pub fn timeout(mut self, duration_ms: u64) -> Self {
        self.status = ExecutionStatus::Timeout;
        self.error = Some("Execution timed out".to_string());
        self.duration_ms = duration_ms;
        self
    }
}

/// Core skill trait that all skills must implement
#[async_trait]
pub trait Skill: Send + Sync {
    /// Unique skill identifier (snake_case)
    fn id(&self) -> &'static str;

    /// Human-readable display name
    fn name(&self) -> &'static str;

    /// Description shown in capability outline
    fn description(&self) -> &'static str;

    /// Permission level (Safe or Sensitive)
    fn permission_level(&self) -> PermissionLevel;

    /// Which modes can invoke this skill
    fn modes(&self) -> &'static [Mode];

    /// Execute the skill with given input
    async fn execute(&self, input: SkillInput, ctx: &SkillContext) -> anyhow::Result<SkillOutput>;

    /// Optional: Validate input before execution
    fn validate_input(&self, _input: &SkillInput) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Skill error types
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Skill not found: {skill_id}")]
    NotFound { skill_id: String },

    #[error("Permission denied for skill: {skill_id}")]
    PermissionDenied { skill_id: String },

    #[error("Skill {skill_id} not available in {mode:?} mode")]
    ModeNotSupported { skill_id: String, mode: Mode },

    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    #[error("Execution failed: {0}")]
    ExecutionFailed(#[from] anyhow::Error),

    #[error("Execution timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("Provider {provider} unavailable: {setup_instructions}")]
    ProviderUnavailable {
        provider: String,
        setup_instructions: String,
    },

    #[error("Operation blocked: {reason}")]
    OperationBlocked { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_input_builder() {
        let input =
            SkillInput::from_query("test query").with_param("key", serde_json::json!("value"));

        assert_eq!(input.query, "test query");
        assert!(input.params.contains_key("key"));
    }

    #[test]
    fn test_skill_output_builder() {
        let output = SkillOutput::text("Hello").with_citation(Citation {
            text: "Source".into(),
            url: "https://example.com".into(),
            accessed_at: Utc::now(),
            verified: true,
        });

        assert_eq!(output.citations.len(), 1);
    }

    #[test]
    fn test_no_delete_file_action() {
        // Verify FileAction has no Delete variant by exhaustive match
        let actions = vec![
            FileAction::Created,
            FileAction::Modified,
            FileAction::Moved {
                from: PathBuf::from("/old"),
            },
            FileAction::Archived {
                to: PathBuf::from("/archive"),
            },
        ];

        for action in actions {
            match action {
                FileAction::Created => {}
                FileAction::Modified => {}
                FileAction::Moved { .. } => {}
                FileAction::Archived { .. } => {} // No Delete variant exists - this is by design
            }
        }
    }
}
