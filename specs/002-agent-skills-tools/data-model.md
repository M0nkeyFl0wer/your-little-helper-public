# Data Model: Agent Skills and Tools

**Feature**: 002-agent-skills-tools | **Date**: 2026-01-04

## Entity Relationship Diagram

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│      Skill      │────<│  SkillExecution  │>────│    FileIndex    │
└────────┬────────┘     └──────────────────┘     └─────────────────┘
         │                       │
         │                       │
         ▼                       ▼
┌─────────────────┐     ┌──────────────────┐
│ SkillPermission │     │    AuditLog      │
└─────────────────┘     └──────────────────┘

┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  ResearchPlan   │     │ ContentCalendar  │     │   FileVersion   │
└─────────────────┘     └──────────────────┘     └─────────────────┘

┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   MPAContext    │     │ DashboardProject │     │   BuildProject  │
└─────────────────┘     └──────────────────┘     └─────────────────┘

┌─────────────────┐     ┌──────────────────┐
│  DesignConfig   │     │ ProviderStatus   │
└─────────────────┘     └──────────────────┘
```

## Core Entities

### Skill

A discrete capability an agent can invoke.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | String | PK, unique | Skill identifier (e.g., "fuzzy_file_search") |
| name | String | required | Display name |
| description | String | required | Human-readable description |
| permission_level | Enum | Safe \| Sensitive | Permission classification |
| modes | Vec<Mode> | non-empty | Which modes can use this skill |
| input_schema | JSON | optional | Expected input format |
| output_type | Enum | Text \| Files \| Data \| Mixed | Output classification |

**Validation Rules**:
- `id` must be snake_case, alphanumeric
- `modes` must contain at least one valid mode

### SkillPermission

User's grant level for a skill.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| skill_id | String | FK → Skill.id | Reference to skill |
| permission | Enum | Enabled \| Disabled \| Ask | User's permission setting |
| scope | Enum | Global \| PerMode | Permission scope |
| granted_at | DateTime | optional | When permission was granted |
| session_approved | bool | default false | Per-session approval flag |

**Validation Rules**:
- Sensitive skills default to `Ask` permission
- `session_approved` resets on app restart

### SkillExecution

A record of a skill being used.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | UUID | PK | Unique execution ID |
| skill_id | String | FK → Skill.id | Which skill was invoked |
| mode | Enum | Mode | Which mode context |
| timestamp | DateTime | required | When execution started |
| input | JSON | required | Input parameters |
| output | JSON | optional | Output data (null if failed) |
| status | Enum | Running \| Completed \| Failed \| Timeout | Execution status |
| duration_ms | u64 | required | Execution time in milliseconds |
| error | String | optional | Error message if failed |

**State Transitions**:
```
Running → Completed | Failed | Timeout
```

### FileIndex

Searchable index of files across drives.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | i64 | PK, auto | SQLite rowid |
| path | String | unique | Full file path |
| name | String | indexed (FTS5) | Filename for fuzzy search |
| extension | String | optional | File extension |
| size_bytes | i64 | required | File size |
| modified_at | DateTime | required | Last modified time |
| drive_id | String | required | Drive/mount identifier |
| keywords | String | optional | Extracted keywords for search |
| indexed_at | DateTime | required | When file was indexed |

**Validation Rules**:
- `path` must be absolute
- `size_bytes` >= 0
- Index refreshes on file system changes (inotify/fsevents)

### ResearchPlan

Pre-research clarification state.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | UUID | PK | Plan identifier |
| topic | String | required | Research topic |
| audience | String | required | Target audience |
| output_format | Enum | Report \| Summary \| Data \| Presentation | Desired output |
| quality_criteria | Vec<String> | optional | User-defined quality standards |
| positive_examples | Vec<String> | optional | Examples of good output |
| negative_examples | Vec<String> | optional | Examples to avoid |
| include_mpa_context | bool | default true | Include MPA training data |
| created_at | DateTime | required | When plan was created |
| status | Enum | Clarifying \| Ready \| InProgress \| Complete | Plan status |

### ContentCalendar

Planning artifact for content work.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | UUID | PK | Calendar identifier |
| entries | Vec<CalendarEntry> | required | Calendar entries |
| last_modified | DateTime | required | Last update time |
| sync_status | Enum | Local \| Syncing \| Synced \| Error | Google Sheets sync status |

**CalendarEntry** (nested):
| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| date | Date | required | Scheduled date |
| title | String | required | Content title |
| status | Enum | Planned \| InProgress \| Review \| Published | Content status |
| assignee | String | optional | Who's responsible |
| notes | String | optional | Additional notes |
| persona_id | UUID | optional | Target persona |

### FileVersion

A versioned state of a file (hidden git).

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | UUID | PK | Version identifier |
| file_path | String | required | Original file path |
| commit_sha | String | required | Git commit SHA |
| timestamp | DateTime | required | When version was created |
| description | String | required | Auto-generated description |
| size_bytes | i64 | required | File size at this version |
| is_current | bool | required | Is this the current version |

**Validation Rules**:
- User-facing: no git terminology ("Version 3" not "Commit abc123")
- Description auto-generated from diff summary

### MPAContext

Marine protected area training data.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | String | PK | Context identifier |
| topics | Vec<String> | required | Covered topics |
| references | Vec<Reference> | required | Source references |
| key_facts | Vec<String> | required | Important facts |
| messaging_guidelines | String | optional | Tone/style guidance |
| last_updated | DateTime | required | Data freshness |

### DashboardProject

Dashboard creation workflow state.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | UUID | PK | Project identifier |
| name | String | required | Dashboard name |
| step | u8 | 1-5 | Current wizard step |
| data_source_path | String | required | Source data file |
| data_profile | JSON | optional | Step 1 output |
| config | JSON | optional | Step 2 output |
| aggregated_data_path | String | optional | Step 3 output |
| validation_status | Enum | Pending \| Passed \| Warnings \| Failed | Step 4 result |
| validation_issues | Vec<String> | optional | Validation problems |
| output_path | String | optional | Step 5 output |
| created_at | DateTime | required | Project start |
| updated_at | DateTime | required | Last update |

**State Transitions**:
```
Step 1 (Analyze) → Step 2 (Config) → Step 3 (Aggregate) → Step 4 (Validate) → Step 5 (Launch)
```

### BuildProject

Spec-kit project state.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | UUID | PK | Project identifier |
| name | String | required | Feature name |
| spec_path | String | optional | Path to spec.md |
| plan_path | String | optional | Path to plan.md |
| tasks_path | String | optional | Path to tasks.md |
| current_task | u32 | optional | Current task index |
| total_tasks | u32 | optional | Total tasks count |
| implementation_status | Enum | Specifying \| Planning \| TaskGen \| Implementing \| Complete | Workflow status |
| created_at | DateTime | required | Project start |
| updated_at | DateTime | required | Last update |

### DesignConfig

Design tool configuration.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| canva_mcp_enabled | bool | required | Canva MCP available |
| canva_last_check | DateTime | optional | Last health check |
| gemini_cli_signed_in | bool | required | Gemini CLI authenticated |
| gemini_api_key_set | bool | required | API key configured |
| default_templates | Vec<String> | optional | Preferred design templates |

### ProviderStatus

External provider health state.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| provider_id | String | PK | Provider identifier |
| status | Enum | Available \| Unavailable \| NeedsSetup | Current status |
| last_check | DateTime | required | Last health check |
| error_message | String | optional | Last error if unavailable |
| setup_url | String | optional | Link to setup instructions |

### AuditLog

File operation and skill execution audit trail.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | UUID | PK | Log entry ID |
| timestamp | DateTime | required | When event occurred |
| event_type | Enum | SkillExec \| FileOp \| PermChange \| Error | Event classification |
| skill_id | String | optional | Related skill |
| file_path | String | optional | Related file |
| action | String | required | What happened |
| details | JSON | optional | Additional data |
| user_visible | bool | default true | Show in settings panel |

**Validation Rules**:
- All file operations MUST create audit log entry
- Sensitive skill executions MUST be logged
- Logs accessible only via settings panel (per clarification)

## Enums

### Mode
```rust
enum Mode {
    Find,
    Fix,
    Research,
    Data,
    Content,
    Build,
}
```

### PermissionLevel
```rust
enum PermissionLevel {
    Safe,      // Auto-approved
    Sensitive, // Per-session confirmation
}
```

### Permission
```rust
enum Permission {
    Enabled,
    Disabled,
    Ask,
}
```

### ExecutionStatus
```rust
enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Timeout,
}
```

## Storage Strategy

| Entity | Storage | Rationale |
|--------|---------|-----------|
| FileIndex | SQLite (FTS5) | Fast fuzzy search, persistent |
| FileVersion | Git repository | Native versioning, proven reliability |
| ContentCalendar | JSON file | Simple, future Sheets sync |
| AuditLog | JSON files (rotated) | Append-only, easy inspection |
| SkillPermission | JSON config | Persists across sessions |
| DashboardProject | JSON file per project | Self-contained |
| BuildProject | Follows spec-kit structure | Interoperability |
| Session state | In-memory | Ephemeral by design |

## Relationships

1. **Skill → SkillExecution**: One skill can have many executions (1:N)
2. **SkillExecution → AuditLog**: Each execution creates audit entries (1:N)
3. **FileVersion → FileIndex**: Versions reference indexed files (N:1)
4. **ContentCalendar.entries → Persona**: Entries may target personas (N:1)
5. **DashboardProject → FileIndex**: Projects reference data sources (N:1)
