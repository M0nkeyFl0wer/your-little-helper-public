# Quickstart: Agent Skills and Tools

**Feature**: 002-agent-skills-tools | **Date**: 2026-01-04

## Prerequisites

### System Requirements

- **Rust**: 1.75+ (`rustup update stable`)
- **SQLite**: 3.35+ with FTS5 support (usually bundled)
- **Git**: 2.x (for version control service)
- **Python**: 3.11+ (for Research mode scripts)
- **Node.js**: 18+ (for Playwright MCP)

### Optional Dependencies

- **Playwright MCP**: `npx @anthropic/mcp-playwright` (Research mode)
- **Chrome DevTools MCP**: Already configured (Fix mode)
- **Canva API**: Requires MCP setup (Content mode)
- **Gemini CLI**: Requires workspace API key (Content mode)

## Quick Setup

```bash
# 1. Clone and enter the project
cd /home/flower/Projects/little-helper

# 2. Ensure on feature branch
git checkout 002-agent-skills-tools

# 3. Add new dependencies
# In Cargo.toml [workspace.dependencies]:
# rusqlite = { version = "0.31", features = ["bundled", "fts5"] }
# git2 = "0.18"
# zeroize = "1.7"

# 4. Build to verify setup
cargo build

# 5. Run tests
cargo test

# 6. Install Playwright MCP (optional)
npx @anthropic/mcp-playwright

# 7. Run the application
cargo run --release
```

## Development Workflow

### Adding a New Skill

1. **Create skill module** in `crates/agent_host/src/skills/<mode>/`

```rust
// crates/agent_host/src/skills/find/fuzzy_search.rs
use crate::skill::{Skill, SkillInput, SkillOutput, SkillContext};
use shared::{Mode, PermissionLevel};

pub struct FuzzyFileSearch;

#[async_trait]
impl Skill for FuzzyFileSearch {
    fn id(&self) -> &'static str { "fuzzy_file_search" }
    fn name(&self) -> &'static str { "Fuzzy File Search" }
    fn description(&self) -> &'static str { "fzf-like search across indexed drives" }
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Safe }
    fn modes(&self) -> &[Mode] { &[Mode::Find] }

    async fn execute(&self, input: SkillInput, ctx: &SkillContext) -> Result<SkillOutput> {
        // Implementation here
        todo!()
    }
}
```

2. **Register skill** in `crates/agent_host/src/skills/mod.rs`

```rust
pub fn register_all(registry: &mut SkillRegistry) {
    registry.register(Arc::new(find::FuzzyFileSearch));
    // ... other skills
}
```

3. **Add tests** in `crates/agent_host/src/skills/<mode>/tests.rs`

```rust
#[tokio::test]
async fn test_fuzzy_search_partial_match() {
    let skill = FuzzyFileSearch;
    let input = SkillInput::from_query("quarterly rep");
    let ctx = test_context();

    let output = skill.execute(input, &ctx).await.unwrap();
    assert!(!output.files.is_empty());
}
```

### Working with File Index

```rust
use services::file_index::FileIndexService;

// Initialize index
let index = FileIndexService::new("/path/to/app/data").await?;

// Add files from a drive
index.scan_drive("/home/user").await?;

// Fuzzy search
let results = index.fuzzy_search("budget 202", 20).await?;
for result in results {
    println!("{}: {}", result.score, result.path);
}
```

### Working with Version Control

```rust
use services::version_control::VersionControlService;

// Initialize for a directory
let vc = VersionControlService::new("/path/to/watched/dir").await?;

// Save a version (auto-generates description)
vc.save_version("/path/to/file.md").await?;

// List versions (user-friendly format)
let versions = vc.list_versions("/path/to/file.md").await?;
for v in versions {
    println!("Version {} - {} - {}", v.number, v.date, v.description);
}

// Restore a version
vc.restore_version("/path/to/file.md", version_id).await?;
```

### Working with Providers

```rust
use providers::{ProviderRegistry, ProviderStatus};

// Check provider availability
let registry = ProviderRegistry::new();
match registry.check("playwright").await {
    ProviderStatus::Available => println!("Playwright ready"),
    ProviderStatus::NeedsSetup { instructions } => {
        println!("Setup required: {}", instructions);
    }
    ProviderStatus::Unavailable { reason } => {
        println!("Unavailable: {}", reason);
    }
}
```

## Testing

### Unit Tests

```bash
# Run all unit tests
cargo test

# Run tests for a specific crate
cargo test -p agent_host

# Run tests for skills only
cargo test -p agent_host skills::
```

### Integration Tests

```bash
# Run integration tests (requires test fixtures)
cargo test --test integration

# Run specific integration test
cargo test --test skill_execution_test
```

### Test Fixtures

Test data is located in `tests/fixtures/`:
- `sample_files/` - Files for indexing tests
- `csv_data/` - Sample CSVs for Data mode
- `mpa_context/` - MPA training data samples

## Project Structure Reference

```
crates/
├── agent_host/src/
│   ├── skills/
│   │   ├── mod.rs           # Skill registry setup
│   │   ├── find/            # Find mode skills
│   │   │   ├── mod.rs
│   │   │   ├── fuzzy_search.rs
│   │   │   └── drive_index.rs
│   │   ├── fix/             # Fix mode skills
│   │   ├── research/        # Research mode skills
│   │   ├── data/            # Data mode skills
│   │   ├── content/         # Content mode skills
│   │   ├── build/           # Build mode skills
│   │   └── common/          # Shared skills (version control)
│   └── executor.rs          # Extended with skill invocation
├── services/src/
│   ├── file_index.rs        # SQLite FTS5 index
│   └── version_control.rs   # Git wrapper
├── shared/src/
│   ├── skill.rs             # Skill trait and types
│   └── events.rs            # Execution events
└── providers/src/
    ├── mod.rs               # Provider registry
    ├── playwright.rs        # Playwright MCP
    ├── canva.rs             # Canva MCP
    ├── gemini.rs            # Gemini CLI
    └── speckit.rs           # Spec-kit integration
```

## Common Issues

### SQLite FTS5 not available

```bash
# Ensure bundled SQLite is used
# In Cargo.toml:
rusqlite = { version = "0.31", features = ["bundled", "fts5"] }
```

### Git2 linking errors

```bash
# Install system dependencies (Linux)
sudo apt install libssl-dev pkg-config cmake

# Or use bundled libgit2
# In Cargo.toml:
git2 = { version = "0.18", features = ["vendored-libgit2"] }
```

### Playwright MCP not found

```bash
# Install globally
npm install -g @anthropic/mcp-playwright

# Or use npx (auto-installs)
npx @anthropic/mcp-playwright
```

## Related Documentation

- [Specification](./spec.md)
- [Implementation Plan](./plan.md)
- [Research Decisions](./research.md)
- [Data Model](./data-model.md)
- [Skill API Contract](./contracts/skill-api.md)
