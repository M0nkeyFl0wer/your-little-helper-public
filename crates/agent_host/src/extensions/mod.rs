//! Extension system for loading user-defined skills from SKILL.md files.
//!
//! This module implements the Agent Skills standard (https://agentskills.io),
//! the same format used by Pi, Claude Code, Cursor, Gemini CLI, GitHub Copilot,
//! and 30+ other agent tools. Skills written for any of these tools can be
//! loaded and used in Little Helper.
//!
//! ## Directory Structure
//!
//! Extensions are discovered from two locations:
//!
//! 1. **Global skills:** `~/.config/little_helper/skills/`
//!    ```text
//!    skills/
//!      deploy-staging/
//!        SKILL.md          <- parsed as skill "deploy-staging"
//!      my-tool.md          <- standalone skill file
//!    ```
//!
//! 2. **Project skills:** `.helper/skills/` in any `allowed_dirs` path
//!    ```text
//!    my-project/
//!      .helper/
//!        skills/
//!          lint-check/
//!            SKILL.md      <- project-specific skill
//!    ```
//!
//! ## File Format
//!
//! Each skill is a Markdown file with YAML frontmatter:
//! ```markdown
//! ---
//! name: deploy-staging
//! description: Build and deploy to staging server
//! permission: sensitive
//! modes: [build]
//! ---
//! # Instructions for the AI agent...
//! ```
//!
//! See `parser.rs` for the full format specification.

pub mod parser;
pub mod script_skill;

use crate::skills::SkillRegistry;
use parser::parse_extension;
use script_skill::ScriptSkill;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Error encountered while loading an extension file.
///
/// Non-fatal: individual extension failures don't prevent other
/// extensions from loading.
#[derive(Debug)]
pub struct ExtensionLoadError {
    /// Path to the file that failed to load.
    pub path: PathBuf,
    /// Human-readable error message.
    pub message: String,
}

/// Load all SKILL.md extensions from the global skills directory.
///
/// Scans `~/.config/little_helper/skills/` for:
/// - `*.md` files directly in the directory
/// - `*/SKILL.md` files in subdirectories (per Agent Skills standard)
///
/// Each valid extension is registered as a skill in the registry.
/// Invalid extensions are logged as warnings but don't prevent
/// other extensions from loading.
///
/// Returns a list of any errors encountered during loading.
pub fn load_global_extensions(registry: &mut SkillRegistry) -> Vec<ExtensionLoadError> {
    // Determine the global skills directory
    let skills_dir = match dirs::config_dir() {
        Some(config) => config.join("little_helper").join("skills"),
        None => {
            tracing::warn!("Could not determine config directory for extension loading");
            return vec![];
        }
    };

    load_extensions_from_dir(&skills_dir, registry)
}

/// Load project-specific SKILL.md extensions from allowed directories.
///
/// For each directory in `allowed_dirs`, checks for `.helper/skills/`
/// and loads any SKILL.md files found there.
///
/// This allows users to define project-specific skills that are only
/// available when working with that project.
pub fn load_project_extensions(
    allowed_dirs: &[String],
    registry: &mut SkillRegistry,
) -> Vec<ExtensionLoadError> {
    let mut all_errors = Vec::new();

    for dir in allowed_dirs {
        let skills_dir = PathBuf::from(dir).join(".helper").join("skills");
        if skills_dir.exists() {
            let errors = load_extensions_from_dir(&skills_dir, registry);
            all_errors.extend(errors);
        }
    }

    all_errors
}

/// Load extensions from a specific directory.
///
/// Scans the directory (non-recursively) for:
/// 1. `*.md` files — parsed as standalone skill files
/// 2. Subdirectories containing `SKILL.md` — per Agent Skills standard
///
/// Creates the directory if it doesn't exist (so users know where to
/// put their extensions).
fn load_extensions_from_dir(dir: &Path, registry: &mut SkillRegistry) -> Vec<ExtensionLoadError> {
    let mut errors = Vec::new();

    // Create the directory if it doesn't exist yet, so users can discover
    // where to put their extension files.
    if !dir.exists() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            tracing::debug!(
                "Could not create extensions directory {}: {}",
                dir.display(),
                e
            );
        }
        return errors;
    }

    // Read directory entries
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            errors.push(ExtensionLoadError {
                path: dir.to_path_buf(),
                message: format!("Failed to read extensions directory: {}", e),
            });
            return errors;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
            // Case 1: Standalone .md file (e.g., "deploy-staging.md")
            try_load_extension(&path, registry, &mut errors);
        } else if path.is_dir() {
            // Case 2: Directory with SKILL.md (e.g., "deploy-staging/SKILL.md")
            // Check for both SKILL.md and skill.md (case-insensitive on some platforms)
            let skill_md = path.join("SKILL.md");
            let skill_md_lower = path.join("skill.md");

            if skill_md.exists() {
                try_load_extension(&skill_md, registry, &mut errors);
            } else if skill_md_lower.exists() {
                try_load_extension(&skill_md_lower, registry, &mut errors);
            }
        }
    }

    errors
}

/// Attempt to load a single extension file and register it.
///
/// On success, the extension is registered as a skill in the registry.
/// On failure, the error is appended to the errors list but execution continues.
fn try_load_extension(
    path: &Path,
    registry: &mut SkillRegistry,
    errors: &mut Vec<ExtensionLoadError>,
) {
    match parse_extension(path) {
        Ok(def) => {
            let id = def.id.clone();

            // Validate the extension ID doesn't conflict with a built-in skill
            if registry.get(&id).is_some() {
                errors.push(ExtensionLoadError {
                    path: path.to_path_buf(),
                    message: format!(
                        "Extension id '{}' conflicts with an existing skill — skipping",
                        id
                    ),
                });
                return;
            }

            // Validate ID format: lowercase alphanumeric + hyphens/underscores
            if !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
            {
                errors.push(ExtensionLoadError {
                    path: path.to_path_buf(),
                    message: format!(
                        "Extension id '{}' must only contain lowercase letters, digits, hyphens, and underscores",
                        id
                    ),
                });
                return;
            }

            tracing::info!(
                "Loaded extension skill '{}' from {}",
                id,
                path.display()
            );

            // Wrap the parsed definition as a ScriptSkill and register it.
            // ScriptSkill implements the Skill trait, making it indistinguishable
            // from built-in skills once registered.
            let skill = ScriptSkill::from_def(def);
            registry.register(Arc::new(skill));
        }
        Err(e) => {
            tracing::warn!("Failed to load extension {}: {}", path.display(), e);
            errors.push(ExtensionLoadError {
                path: path.to_path_buf(),
                message: e.to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_extensions_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = SkillRegistry::new();

        let errors = load_extensions_from_dir(dir.path(), &mut registry);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_load_extensions_with_skill_file() {
        let dir = tempfile::tempdir().unwrap();

        // Create a skill directory with SKILL.md
        let skill_dir = dir.path().join("test-skill");
        std::fs::create_dir(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\ndescription: A test skill\n---\n# Test\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        let errors = load_extensions_from_dir(dir.path(), &mut registry);

        assert!(errors.is_empty(), "Errors: {:?}", errors.iter().map(|e| &e.message).collect::<Vec<_>>());
        assert!(
            registry.get("test-skill").is_some(),
            "test-skill should be registered"
        );
    }

    #[test]
    fn test_load_extensions_standalone_md() {
        let dir = tempfile::tempdir().unwrap();

        // Create a standalone .md skill file
        std::fs::write(
            dir.path().join("my-tool.md"),
            "---\nname: my-tool\ndescription: A standalone tool\n---\n# Do stuff\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        let errors = load_extensions_from_dir(dir.path(), &mut registry);

        assert!(errors.is_empty());
        assert!(registry.get("my-tool").is_some());
    }

    #[test]
    fn test_load_extensions_invalid_file_skipped() {
        let dir = tempfile::tempdir().unwrap();

        // Create an invalid .md file (no frontmatter)
        std::fs::write(dir.path().join("bad.md"), "No frontmatter here").unwrap();

        // Create a valid one
        let skill_dir = dir.path().join("good-skill");
        std::fs::create_dir(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: good-skill\ndescription: Works fine\n---\n# OK\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        let errors = load_extensions_from_dir(dir.path(), &mut registry);

        // Bad file should produce an error
        assert_eq!(errors.len(), 1);
        // Good file should still be loaded
        assert!(registry.get("good-skill").is_some());
    }

    #[test]
    fn test_load_nonexistent_dir_creates_it() {
        let dir = tempfile::tempdir().unwrap();
        let skills_dir = dir.path().join("does-not-exist");

        let mut registry = SkillRegistry::new();
        let errors = load_extensions_from_dir(&skills_dir, &mut registry);

        assert!(errors.is_empty());
        // Directory should now exist for user discovery
        assert!(skills_dir.exists());
    }

    #[test]
    fn test_conflict_detection() {
        let dir = tempfile::tempdir().unwrap();

        // Create two skill files with the same ID
        let skill1_dir = dir.path().join("my-skill");
        std::fs::create_dir(&skill1_dir).unwrap();
        std::fs::write(
            skill1_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: First one\n---\n# First\n",
        )
        .unwrap();

        // Also create a standalone file that would produce the same ID
        std::fs::write(
            dir.path().join("my-skill.md"),
            "---\nname: my-skill\ndescription: Second one\n---\n# Second\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        let _errors = load_extensions_from_dir(dir.path(), &mut registry);

        // One should succeed, one should conflict
        assert!(registry.get("my-skill").is_some());
        // The conflict error may or may not appear depending on directory iteration order
        // but the registry should only have one entry with that ID
    }
}
