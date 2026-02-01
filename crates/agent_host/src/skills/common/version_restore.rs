//! Version restore skill for reverting files to previous versions.
//!
//! This skill allows users to restore files to earlier versions
//! without exposing git terminology. The current version is always
//! preserved before restoring.

use anyhow::Result;
use async_trait::async_trait;
use services::version_control::VersionControlService;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};
use std::path::PathBuf;

/// Version restore skill for reverting files.
///
/// Restores a file to a previous version while preserving the current
/// state in version history. Available in all modes.
pub struct VersionRestore;

impl VersionRestore {
    pub fn new() -> Self {
        Self
    }

    /// Create version control service for a path
    fn create_service(path: &PathBuf) -> Result<VersionControlService> {
        let root = path.parent().unwrap_or(path);
        VersionControlService::new(root)
    }
}

impl Default for VersionRestore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for VersionRestore {
    fn id(&self) -> &'static str {
        "version_restore"
    }

    fn name(&self) -> &'static str {
        "Restore Version"
    }

    fn description(&self) -> &'static str {
        "Restore a file to a previous version (current version is preserved)"
    }

    fn permission_level(&self) -> PermissionLevel {
        // Sensitive because it modifies files
        PermissionLevel::Sensitive
    }

    fn modes(&self) -> &'static [Mode] {
        // Available in all modes
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
        // Get path and version number from params or query
        let path_str = input
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let version_num = input
            .params
            .get("version")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);

        // If params not provided, try to parse from query
        let (path_str, version_num) = if path_str.is_none() {
            parse_restore_query(&input.query)
        } else {
            (path_str, version_num)
        };

        // Validate we have both required params
        let path_str = match path_str {
            Some(p) => p,
            None => {
                return Ok(SkillOutput::text(
                    "Please provide a file path to restore.\n\n\
                     Example: \"restore report.docx to version 2\"\n\
                     Or: \"go back to the previous version of config.json\"",
                ));
            }
        };

        let version_num = match version_num {
            Some(v) => v,
            None => {
                return Ok(SkillOutput::text(format!(
                    "Which version would you like to restore '{}'?\n\n\
                     Say \"show versions of {}\" to see available versions,\n\
                     then \"restore to version N\"",
                    path_str, path_str
                )));
            }
        };

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

        // Create version service
        let service = Self::create_service(&path)?;

        // Get versions
        let versions = service.list_versions(&path)?;

        if versions.is_empty() {
            return Ok(SkillOutput::text(format!(
                "No saved versions found for '{}'.\n\n\
                 Versions are automatically saved when files are modified through Little Helper.",
                path_str
            )));
        }

        // Find the requested version
        let target_version = versions.iter().find(|v| v.version_number == version_num);

        let target = match target_version {
            Some(v) => v,
            None => {
                let available: Vec<u32> = versions.iter().map(|v| v.version_number).collect();
                return Ok(SkillOutput::text(format!(
                    "Version {} not found.\n\n\
                     Available versions: {:?}\n\n\
                     Say \"show versions of {}\" to see details.",
                    version_num, available, path_str
                )));
            }
        };

        // Check if already at this version
        if target.is_current {
            return Ok(SkillOutput::text(format!(
                "File is already at version {}.\n\n\
                 Choose a different version to restore.",
                version_num
            )));
        }

        // Perform restore (this saves current state first)
        service.restore_version(&path, target)?;

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let text = format!(
            "✅ Restored '{}' to version {}\n\n\
             • From: {}\n\
             • Description: {}\n\n\
             Your previous version was saved automatically.\n\
             Say \"show versions of {}\" to see all versions.",
            file_name,
            version_num,
            target.relative_time(),
            target.description,
            file_name
        );

        let data = serde_json::json!({
            "file_path": path.to_string_lossy(),
            "file_name": file_name,
            "restored_version": version_num,
            "restored_from": target.timestamp.to_rfc3339(),
            "description": target.description,
        });

        Ok(SkillOutput {
            result_type: ResultType::Data,
            text: Some(text),
            files: vec![shared::skill::FileResult {
                path: path.clone(),
                action: shared::skill::FileAction::Modified,
                preview: None,
            }],
            data: Some(data),
            citations: Vec::new(),
            suggested_actions: vec![
                shared::skill::SuggestedAction {
                    label: "View all versions".to_string(),
                    skill_id: "version_history".to_string(),
                    params: [(
                        "path".to_string(),
                        serde_json::json!(path.to_string_lossy()),
                    )]
                    .into_iter()
                    .collect(),
                },
                shared::skill::SuggestedAction {
                    label: "Open file".to_string(),
                    skill_id: "file_preview".to_string(),
                    params: [(
                        "path".to_string(),
                        serde_json::json!(path.to_string_lossy()),
                    )]
                    .into_iter()
                    .collect(),
                },
            ],
        })
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        // Either params or query must have path info
        let has_path = input.params.get("path").and_then(|v| v.as_str()).is_some()
            || !input.query.trim().is_empty();

        if !has_path {
            anyhow::bail!("Please provide a file path to restore");
        }
        Ok(())
    }
}

/// Parse restore query for path and version number
/// Examples:
/// - "restore report.docx to version 2"
/// - "go back to version 1 of config.json"
/// - "revert test.txt to version 3"
fn parse_restore_query(query: &str) -> (Option<String>, Option<u32>) {
    let query = query.to_lowercase();

    // Try to extract version number
    let version_num = if let Some(pos) = query.find("version") {
        query[pos + 7..]
            .split_whitespace()
            .next()
            .and_then(|s| s.trim_matches(|c: char| !c.is_numeric()).parse().ok())
    } else {
        None
    };

    // Try to extract file path (look for common patterns)
    let path = extract_file_path(&query);

    (path, version_num)
}

/// Extract file path from a natural language query
fn extract_file_path(query: &str) -> Option<String> {
    // First, look for quoted paths (handles "my file.doc" with spaces)
    if let (Some(start), Some(end)) = (query.find('"'), query.rfind('"')) {
        if end > start {
            return Some(query[start + 1..end].to_string());
        }
    }

    // Then look for words with file extensions
    for word in query.split_whitespace() {
        let clean =
            word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '/' && c != '\\');
        if clean.contains('.') && !clean.starts_with('.') {
            // Likely a file path
            return Some(clean.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_restore_query() {
        let (path, version) = parse_restore_query("restore report.docx to version 2");
        assert_eq!(path, Some("report.docx".to_string()));
        assert_eq!(version, Some(2));
    }

    #[test]
    fn test_extract_file_path() {
        assert_eq!(
            extract_file_path("restore test.txt"),
            Some("test.txt".to_string())
        );
        assert_eq!(
            extract_file_path("go back to \"my file.doc\""),
            Some("my file.doc".to_string())
        );
    }
}
