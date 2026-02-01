//! Version history skill for viewing file version history.
//!
//! This skill allows users to view the version history of files
//! without exposing git terminology.

use anyhow::Result;
use async_trait::async_trait;
use services::version_control::VersionControlService;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};
use shared::version::FileVersion;
use std::path::PathBuf;

/// Version history skill for viewing file versions.
///
/// Shows all saved versions of a file with user-friendly descriptions.
/// Available in all modes as version control is a cross-cutting concern.
pub struct VersionHistory;

impl VersionHistory {
    pub fn new() -> Self {
        Self
    }

    /// Create version control service for a path
    fn create_service(path: &PathBuf) -> Result<VersionControlService> {
        let root = path.parent().unwrap_or(path);
        VersionControlService::new(root)
    }

    /// Format version list for display
    fn format_versions(&self, file_name: &str, versions: &[FileVersion]) -> String {
        if versions.is_empty() {
            return format!("No saved versions found for '{}'.\n\nTip: Versions are automatically saved when files are modified through Little Helper.", file_name);
        }

        let mut output = format!("📋 Version History for '{}'\n", file_name);
        output.push_str(&format!("{} versions found\n\n", versions.len()));

        for version in versions.iter().rev() {
            let current_marker = if version.is_current {
                " ← current"
            } else {
                ""
            };
            output.push_str(&format!(
                "  Version {}{}\n    {} • {}\n    {}\n\n",
                version.version_number,
                current_marker,
                version.relative_time(),
                version.formatted_size(),
                version.description,
            ));
        }

        output.push_str("\nTo restore a version, say \"restore to version N\" or \"go back to the version from [time]\"");
        output
    }
}

impl Default for VersionHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for VersionHistory {
    fn id(&self) -> &'static str {
        "version_history"
    }

    fn name(&self) -> &'static str {
        "Version History"
    }

    fn description(&self) -> &'static str {
        "View saved versions of a file with easy restore options"
    }

    fn permission_level(&self) -> PermissionLevel {
        // Safe because it only reads version history
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        // Available in all modes as a cross-cutting concern
        &[
            Mode::Find,
            Mode::Fix,
            Mode::Research,
            Mode::Data,
            Mode::Content,
            Mode::Build,
        ]
    }

    async fn execute(&self, input: SkillInput, ctx: &SkillContext) -> Result<SkillOutput> {
        // Get path from params or query
        let path_str = input
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| input.query.trim().to_string());

        if path_str.is_empty() {
            return Ok(SkillOutput::text(
                "Please provide a file path to view version history.\n\n\
                 Example: \"show versions of report.docx\"\n\
                 Or: \"what are the earlier versions of /path/to/file.txt\"",
            ));
        }

        let path = if path_str.starts_with('/') || path_str.starts_with('\\') {
            PathBuf::from(&path_str)
        } else {
            ctx.working_dir.join(&path_str)
        };

        // Verify file exists
        if !path.exists() {
            return Ok(SkillOutput::text(format!(
                "File not found: {}\n\nPlease provide a valid file path.",
                path.display()
            )));
        }

        // Create version service and get history
        let service = Self::create_service(&path)?;
        let versions = service.list_versions(&path)?;

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let text = self.format_versions(&file_name, &versions);

        // Build structured data
        let data = serde_json::json!({
            "file_path": path.to_string_lossy(),
            "file_name": file_name,
            "version_count": versions.len(),
            "versions": versions.iter().map(|v| {
                serde_json::json!({
                    "version_number": v.version_number,
                    "timestamp": v.timestamp.to_rfc3339(),
                    "relative_time": v.relative_time(),
                    "description": v.description,
                    "size": v.formatted_size(),
                    "is_current": v.is_current,
                })
            }).collect::<Vec<_>>(),
        });

        Ok(SkillOutput {
            result_type: ResultType::Data,
            text: Some(text),
            files: Vec::new(),
            data: Some(data),
            citations: Vec::new(),
            suggested_actions: if !versions.is_empty() {
                vec![shared::skill::SuggestedAction {
                    label: "Restore previous version".to_string(),
                    skill_id: "version_restore".to_string(),
                    params: [(
                        "path".to_string(),
                        serde_json::json!(path.to_string_lossy()),
                    )]
                    .into_iter()
                    .collect(),
                }]
            } else {
                Vec::new()
            },
        })
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        let has_path = !input.query.trim().is_empty()
            || input.params.get("path").and_then(|v| v.as_str()).is_some();

        if !has_path {
            anyhow::bail!("Please provide a file path to view version history");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_empty_versions() {
        let skill = VersionHistory::new();
        let output = skill.format_versions("test.txt", &[]);
        assert!(output.contains("No saved versions"));
    }
}
