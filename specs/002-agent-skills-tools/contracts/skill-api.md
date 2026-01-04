# Skill API Contract

**Feature**: 002-agent-skills-tools | **Date**: 2026-01-04

## Overview

Internal API contract for skill invocation within Little Helper. Skills are invoked by agents through the SkillRegistry and execute asynchronously with permission checks.

## Core Trait

```rust
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
    fn modes(&self) -> &[Mode];

    /// Execute the skill with given input
    async fn execute(&self, input: SkillInput, ctx: &SkillContext) -> Result<SkillOutput>;

    /// Optional: Validate input before execution
    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        Ok(()) // Default: accept all input
    }
}
```

## Input/Output Types

### SkillInput

```rust
pub struct SkillInput {
    /// Natural language request from user
    pub query: String,

    /// Structured parameters (skill-specific)
    pub params: HashMap<String, serde_json::Value>,

    /// Files/images added to context
    pub context_files: Vec<PathBuf>,

    /// Current conversation context (last N messages)
    pub conversation: Vec<Message>,
}
```

### SkillOutput

```rust
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

pub enum ResultType {
    Text,
    Files,
    Data,
    Mixed,
    Error,
}

pub struct FileResult {
    pub path: PathBuf,
    pub action: FileAction,
    pub preview: Option<String>,
}

pub enum FileAction {
    Created,
    Modified,
    Moved { from: PathBuf },
    Archived { to: PathBuf },
    // Note: NO Delete variant - deletion is forbidden
}

pub struct Citation {
    pub text: String,
    pub url: String,
    pub accessed_at: DateTime<Utc>,
    pub verified: bool,
}
```

### SkillContext

```rust
pub struct SkillContext {
    /// Current mode
    pub mode: Mode,

    /// Session approval cache (for Sensitive skills)
    pub session_approvals: Arc<RwLock<HashSet<String>>>,

    /// Audit logger
    pub audit: Arc<AuditLogger>,

    /// Provider registry (for external tools)
    pub providers: Arc<ProviderRegistry>,

    /// File index service
    pub file_index: Arc<FileIndexService>,

    /// Version control service
    pub version_control: Arc<VersionControlService>,

    /// MPA context (for Research/Content modes)
    pub mpa_context: Arc<MPAContext>,
}
```

## Skill Registry

```rust
pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
    permissions: SkillPermissions,
}

impl SkillRegistry {
    /// Register a skill
    pub fn register(&mut self, skill: Arc<dyn Skill>);

    /// Get skills available for a mode
    pub fn for_mode(&self, mode: Mode) -> Vec<&dyn Skill>;

    /// Invoke a skill with permission check
    pub async fn invoke(
        &self,
        skill_id: &str,
        input: SkillInput,
        ctx: &SkillContext,
    ) -> Result<SkillExecution>;

    /// Check if skill is approved for this session
    pub fn is_session_approved(&self, skill_id: &str) -> bool;

    /// Grant session approval for sensitive skill
    pub fn approve_session(&mut self, skill_id: &str);
}
```

## Skill Execution Flow

```
1. Agent requests skill invocation
     │
     ▼
2. Registry looks up skill by ID
     │
     ▼
3. Permission check
     │
     ├─ Safe skill → proceed
     │
     └─ Sensitive skill
          │
          ├─ Session approved → proceed
          │
          └─ Not approved → request confirmation
               │
               ├─ User approves → add to session, proceed
               │
               └─ User denies → return Denied result
     │
     ▼
4. Input validation (skill.validate_input)
     │
     ▼
5. Create SkillExecution record (status: Running)
     │
     ▼
6. Execute skill (skill.execute)
     │
     ├─ Success → update execution (status: Completed)
     │
     ├─ Error → update execution (status: Failed)
     │
     └─ Timeout → update execution (status: Timeout)
     │
     ▼
7. Log to audit trail
     │
     ▼
8. Return SkillOutput to agent
```

## Mode-Specific Skill Sets

### Find Mode

| Skill ID | Permission | Description |
|----------|------------|-------------|
| fuzzy_file_search | Safe | fzf-like search across indexed drives |
| drive_index | Safe | Maintain and update file index |
| file_preview | Safe | Preview file contents |
| file_organize | Sensitive | Move/archive files (never delete) |

### Fix Mode

| Skill ID | Permission | Description |
|----------|------------|-------------|
| system_diagnostic | Safe | Run comprehensive system check |
| system_info_display | Safe | Show htop-like system status |
| network_troubleshoot | Safe | Diagnose connectivity issues |
| browser_debug | Sensitive | Inspect browser via DevTools MCP |
| log_analysis | Sensitive | Read and analyze system logs |

### Research Mode

| Skill ID | Permission | Description |
|----------|------------|-------------|
| research_clarify | Safe | Ask clarifying questions before research |
| web_search | Safe | Search the internet |
| web_fetch | Safe | Fetch and extract web content |
| browser_automate | Sensitive | Use Playwright for dynamic sites |
| python_analysis | Sensitive | Execute Python scripts for analysis |
| mpa_context | Safe | Access marine protected area training data |
| citation_validate | Safe | Validate and format citations |

### Data Mode

| Skill ID | Permission | Description |
|----------|------------|-------------|
| file_read | Safe | Read file contents |
| parse_data | Safe | Parse CSV, JSON, Excel formats |
| analyze_with_references | Safe | Compute stats with source linking |
| data_validate | Safe | Check data quality and consistency |
| generate_chart | Safe | Create visualizations |
| dashboard_wizard | Safe | Step-by-step dashboard creation |
| dashboard_analyze | Safe | Profile survey data |
| dashboard_config | Safe | Configure dashboard layout |
| dashboard_validate | Safe | Run validation gates |
| dashboard_qa | Safe | Interactive QA testing |

### Content Mode

| Skill ID | Permission | Description |
|----------|------------|-------------|
| mpa_context | Safe | Access MPA training data |
| content_calendar | Safe | View and edit content calendar |
| calendar_spreadsheet | Safe | Open calendar as spreadsheet |
| file_read | Safe | Read existing documents |
| file_write_versioned | Sensitive | Save with automatic versioning |
| grammar_check | Safe | Check spelling and grammar |
| canva_mcp | Safe | Access Canva design tools |
| nano_banana_design | Sensitive | Generate designs via Gemini |
| design_templates | Safe | Access design templates |
| persona_engine | Safe | Generate audience personas |
| content_automation | Safe | Automated content workflows |

### Build Mode

| Skill ID | Permission | Description |
|----------|------------|-------------|
| speckit_specify | Safe | Create feature spec |
| speckit_plan | Safe | Generate implementation plan |
| speckit_tasks | Safe | Generate task list |
| speckit_implement | Sensitive | Execute tasks with tracking |
| speckit_clarify | Safe | Ask clarifying questions |
| speckit_analyze | Safe | Cross-artifact analysis |
| speckit_constitution | Safe | Create/update constitution |

### All Modes

| Skill ID | Permission | Description |
|----------|------------|-------------|
| version_history | Safe | View file version history |
| version_restore | Sensitive | Restore previous file version |
| mode_capability_outline | Safe | Display mode capabilities |

## Error Handling

```rust
pub enum SkillError {
    /// Skill not found
    NotFound { skill_id: String },

    /// Permission denied (user declined)
    PermissionDenied { skill_id: String },

    /// Skill not available in this mode
    ModeNotSupported { skill_id: String, mode: Mode },

    /// Input validation failed
    InvalidInput { message: String },

    /// Execution failed
    ExecutionFailed { cause: anyhow::Error },

    /// Execution timed out
    Timeout { duration_ms: u64 },

    /// External provider unavailable
    ProviderUnavailable {
        provider: String,
        setup_instructions: String,
    },

    /// File operation blocked (e.g., deletion attempted)
    OperationBlocked { reason: String },
}
```

## File Safety Contract

**INVARIANT**: No skill may delete files. Ever.

```rust
impl FileAction {
    /// Validate that action is safe
    pub fn is_safe(&self) -> bool {
        // All variants are safe by construction
        // There is no Delete variant
        true
    }
}

/// File operations service that enforces no-delete policy
pub struct SafeFileOps {
    audit: Arc<AuditLogger>,
}

impl SafeFileOps {
    /// Move file to new location
    pub async fn move_file(&self, from: &Path, to: &Path) -> Result<()>;

    /// Archive file to designated folder
    pub async fn archive(&self, path: &Path, archive_dir: &Path) -> Result<PathBuf>;

    /// Copy file to new location
    pub async fn copy(&self, from: &Path, to: &Path) -> Result<()>;

    /// Create new file
    pub async fn create(&self, path: &Path, content: &[u8]) -> Result<()>;

    /// Modify existing file (with version control)
    pub async fn modify(&self, path: &Path, content: &[u8]) -> Result<()>;

    // Note: NO delete method exists
}
```

## Audit Log Contract

```rust
pub struct AuditLogger {
    log_dir: PathBuf,
}

impl AuditLogger {
    /// Log skill execution
    pub async fn log_skill_execution(&self, execution: &SkillExecution);

    /// Log file operation
    pub async fn log_file_op(&self, action: &FileAction, path: &Path);

    /// Log permission change
    pub async fn log_permission_change(&self, skill_id: &str, old: Permission, new: Permission);

    /// Log error
    pub async fn log_error(&self, error: &SkillError);

    /// Get logs (for settings panel)
    pub async fn get_logs(&self, filter: AuditFilter) -> Vec<AuditEntry>;
}
```
