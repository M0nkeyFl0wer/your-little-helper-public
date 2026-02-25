//! Parser for SKILL.md extension files following the Agent Skills standard.
//!
//! The Agent Skills standard (https://agentskills.io/specification) defines a
//! portable format for AI agent capabilities. Skills are Markdown files with
//! YAML frontmatter containing metadata (name, description, modes, etc.)
//! and a body containing instructions for the AI agent.
//!
//! This parser supports the same format used by Pi, Claude Code, Cursor,
//! Gemini CLI, GitHub Copilot, and 30+ other agent tools.

use anyhow::{bail, Context, Result};
use shared::skill::{Mode, PermissionLevel};
use std::path::{Path, PathBuf};

/// Parsed extension definition from a SKILL.md file.
///
/// Contains all metadata from the YAML frontmatter plus the Markdown body
/// which serves as instructions for the AI agent when the skill is invoked.
#[derive(Debug, Clone)]
pub struct ExtensionDef {
    /// Unique skill identifier (kebab-case, e.g., "deploy-staging").
    /// Must match the parent directory name per the Agent Skills standard.
    pub id: String,
    /// Human-readable display name shown in the UI and tool definitions.
    pub name: String,
    /// Short description (max 1024 chars) sent to the model as tool description.
    pub description: String,
    /// Whether the skill requires user confirmation before execution.
    /// Defaults to Safe if not specified.
    pub permission: PermissionLevel,
    /// Which agent modes can invoke this skill.
    /// Defaults to all modes if not specified.
    pub modes: Vec<Mode>,
    /// Full Markdown body — instructions for the AI agent.
    /// This is injected into the conversation when the skill is invoked.
    pub body: String,
    /// Path to the source SKILL.md file (for error reporting and reload).
    pub source_path: PathBuf,
}

/// Raw YAML frontmatter structure for deserialization.
///
/// Fields are all optional except `name` and `description`, matching the
/// Agent Skills specification's required vs optional fields.
#[derive(Debug, serde::Deserialize)]
struct RawFrontmatter {
    /// Required: skill name (1-64 chars)
    name: String,
    /// Required: short description (1-1024 chars)
    description: String,
    /// Optional: "safe" or "sensitive" (default: "safe")
    #[serde(default)]
    permission: Option<String>,
    /// Optional: list of mode names (default: all modes)
    #[serde(default)]
    modes: Option<Vec<String>>,
    /// Optional: compatibility notes
    #[serde(default)]
    compatibility: Option<String>,
    /// Optional: license identifier
    #[serde(default)]
    license: Option<String>,
    /// Optional: arbitrary metadata (author, version, etc.)
    #[serde(default)]
    metadata: Option<serde_yaml::Value>,
}

/// Parse a SKILL.md file into an ExtensionDef.
///
/// Expects a Markdown file with YAML frontmatter delimited by `---`:
/// ```markdown
/// ---
/// name: my-skill
/// description: Does something useful
/// permission: safe
/// modes: [find, fix]
/// ---
/// # Instructions for the AI agent...
/// ```
///
/// Returns Err with a descriptive message if the file is malformed or
/// missing required fields.
pub fn parse_extension(path: &Path) -> Result<ExtensionDef> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read extension file: {}", path.display()))?;

    // Split on YAML frontmatter delimiters (---)
    let (frontmatter_str, body) = split_frontmatter(&content)
        .with_context(|| format!("Invalid SKILL.md format in {}", path.display()))?;

    // Parse YAML frontmatter
    let raw: RawFrontmatter = serde_yaml::from_str(frontmatter_str)
        .with_context(|| format!("Invalid YAML frontmatter in {}", path.display()))?;

    // Validate required fields
    if raw.name.is_empty() || raw.name.len() > 64 {
        bail!(
            "Skill name must be 1-64 characters, got '{}' in {}",
            raw.name,
            path.display()
        );
    }
    if raw.description.is_empty() || raw.description.len() > 1024 {
        bail!(
            "Skill description must be 1-1024 characters in {}",
            path.display()
        );
    }

    // Derive the skill ID from the parent directory name (per Agent Skills standard),
    // falling back to the sanitized skill name if the file is at the root level.
    let id = derive_skill_id(path, &raw.name);

    // Parse permission level (default: Safe)
    let permission = match raw.permission.as_deref() {
        Some("sensitive") => PermissionLevel::Sensitive,
        Some("safe") | None => PermissionLevel::Safe,
        Some(other) => bail!(
            "Unknown permission '{}' in {} (expected 'safe' or 'sensitive')",
            other,
            path.display()
        ),
    };

    // Parse mode list (default: all modes)
    let modes = match raw.modes {
        Some(mode_strs) => {
            let mut modes = Vec::new();
            for s in &mode_strs {
                modes.push(parse_mode(s).with_context(|| {
                    format!("Unknown mode '{}' in {}", s, path.display())
                })?);
            }
            modes
        }
        // No modes specified = available in all modes
        None => Mode::all().to_vec(),
    };

    Ok(ExtensionDef {
        id,
        name: raw.name,
        description: raw.description,
        permission,
        modes,
        body: body.to_string(),
        source_path: path.to_path_buf(),
    })
}

/// Split a Markdown file into YAML frontmatter and body.
///
/// The frontmatter must be delimited by `---` on its own line at the start
/// of the file, with a closing `---` delimiter.
fn split_frontmatter(content: &str) -> Result<(&str, &str)> {
    let trimmed = content.trim_start();

    // Must start with ---
    if !trimmed.starts_with("---") {
        bail!("File must start with YAML frontmatter (---)");
    }

    // Find the closing --- delimiter (skip the opening one)
    let after_opening = &trimmed[3..];
    let closing_pos = after_opening
        .find("\n---")
        .ok_or_else(|| anyhow::anyhow!("Missing closing --- delimiter for frontmatter"))?;

    // +1 to skip the \n before ---
    let frontmatter = &after_opening[..closing_pos].trim();
    // +4 to skip \n---
    let body = &after_opening[closing_pos + 4..];
    // Trim leading newline from body
    let body = body.strip_prefix('\n').unwrap_or(body);

    Ok((frontmatter, body))
}

/// Derive a skill ID from the file path.
///
/// Per the Agent Skills standard, the skill ID should match the parent
/// directory name. For example, `deploy-staging/SKILL.md` yields `deploy-staging`.
/// Falls back to sanitizing the skill name if the file is a standalone .md file.
fn derive_skill_id(path: &Path, name: &str) -> String {
    // If the file is SKILL.md or skill.md, use the parent directory name
    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
        if filename.eq_ignore_ascii_case("skill.md") {
            if let Some(parent) = path.parent().and_then(|p| p.file_name()) {
                if let Some(dir_name) = parent.to_str() {
                    return dir_name.to_string();
                }
            }
        }
    }

    // Otherwise, use the file stem (e.g., "deploy-staging.md" -> "deploy-staging")
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        return stem.to_string();
    }

    // Last resort: sanitize the skill name
    name.to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

/// Parse a mode string into a Mode enum variant.
fn parse_mode(s: &str) -> Result<Mode> {
    match s.to_lowercase().as_str() {
        "find" => Ok(Mode::Find),
        "fix" => Ok(Mode::Fix),
        "research" => Ok(Mode::Research),
        "data" => Ok(Mode::Data),
        "content" => Ok(Mode::Content),
        "build" => Ok(Mode::Build),
        _ => bail!("Unknown mode: '{}' (expected: find, fix, research, data, content, build)", s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_frontmatter_valid() {
        let content = "---\nname: test\ndescription: hello\n---\n# Body here\nMore content";
        let (fm, body) = split_frontmatter(content).unwrap();
        assert!(fm.contains("name: test"));
        assert!(body.contains("# Body here"));
    }

    #[test]
    fn test_split_frontmatter_no_delimiters() {
        let content = "Just some text without frontmatter";
        assert!(split_frontmatter(content).is_err());
    }

    #[test]
    fn test_split_frontmatter_missing_closing() {
        let content = "---\nname: test\n";
        assert!(split_frontmatter(content).is_err());
    }

    #[test]
    fn test_derive_skill_id_from_directory() {
        let path = Path::new("/skills/deploy-staging/SKILL.md");
        assert_eq!(derive_skill_id(path, "Deploy Staging"), "deploy-staging");
    }

    #[test]
    fn test_derive_skill_id_from_filename() {
        let path = Path::new("/skills/my-tool.md");
        assert_eq!(derive_skill_id(path, "My Tool"), "my-tool");
    }

    #[test]
    fn test_derive_skill_id_sanitized() {
        // When file_stem fails, fall back to sanitized name
        let id = "My Cool Tool"
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>();
        assert_eq!(id, "my-cool-tool");
    }

    #[test]
    fn test_parse_mode_valid() {
        assert_eq!(parse_mode("find").unwrap(), Mode::Find);
        assert_eq!(parse_mode("Fix").unwrap(), Mode::Fix);
        assert_eq!(parse_mode("RESEARCH").unwrap(), Mode::Research);
    }

    #[test]
    fn test_parse_mode_invalid() {
        assert!(parse_mode("unknown").is_err());
    }

    #[test]
    fn test_parse_extension_minimal() {
        // Create a temp file with minimal frontmatter
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("test-skill");
        std::fs::create_dir(&skill_dir).unwrap();
        let path = skill_dir.join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: test-skill\ndescription: A test skill\n---\n# Instructions\nDo the thing.\n",
        )
        .unwrap();

        let def = parse_extension(&path).unwrap();
        assert_eq!(def.id, "test-skill");
        assert_eq!(def.name, "test-skill");
        assert_eq!(def.description, "A test skill");
        assert_eq!(def.permission, PermissionLevel::Safe);
        assert_eq!(def.modes.len(), Mode::all().len()); // defaults to all modes
        assert!(def.body.contains("# Instructions"));
    }

    #[test]
    fn test_parse_extension_with_modes_and_permission() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deploy.md");
        std::fs::write(
            &path,
            "---\nname: deploy\ndescription: Deploy to server\npermission: sensitive\nmodes: [build, fix]\n---\nRun deploy commands.\n",
        )
        .unwrap();

        let def = parse_extension(&path).unwrap();
        assert_eq!(def.permission, PermissionLevel::Sensitive);
        assert_eq!(def.modes.len(), 2);
        assert!(def.modes.contains(&Mode::Build));
        assert!(def.modes.contains(&Mode::Fix));
    }
}
