//! File organization skill with NO DELETE policy.
//!
//! This skill helps organize files by moving and archiving them.
//! It intentionally refuses any deletion requests and offers safe alternatives.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{
    FileAction, FileResult, Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput,
    SkillOutput, SuggestedAction,
};
use std::path::PathBuf;

use crate::skills::common::SafeFileOps;

/// Patterns that indicate a deletion request
const DELETE_PATTERNS: &[&str] = &[
    "delete",
    "remove",
    "erase",
    "trash",
    "get rid of",
    "throw away",
    "eliminate",
    "destroy",
    "wipe",
    "clear out",
    "purge",
    "discard",
    "dispose",
    "rm ",
    "rm -",
    "unlink",
];

/// File organization skill.
///
/// Provides safe file organization operations that NEVER delete files.
/// Deletion requests are detected and refused with archive alternatives.
pub struct FileOrganize {
    safe_ops: SafeFileOps,
}

impl FileOrganize {
    pub fn new(archive_dir: PathBuf) -> Self {
        Self {
            safe_ops: SafeFileOps::new(archive_dir),
        }
    }

    /// Check if a query appears to be a deletion request
    fn is_deletion_request(query: &str) -> bool {
        let query_lower = query.to_lowercase();
        DELETE_PATTERNS
            .iter()
            .any(|pattern| query_lower.contains(pattern))
    }

    /// Generate a refusal message with safe alternatives
    fn deletion_refusal_message(file_path: &str) -> String {
        format!(
            "I can't delete files - that's a safety feature to protect your data.\n\n\
             Instead, I can help you:\n\n\
             - **Archive** the file (moves it to a dated archive folder)\n\
             - **Move** it to a different location\n\
             - **Organize** it into a better folder structure\n\n\
             Would you like me to archive '{}' instead?\n\n\
             Archived files can always be restored later.",
            file_path
        )
    }

    /// Parse organize action from query
    fn parse_organize_action(query: &str) -> OrganizeAction {
        let query_lower = query.to_lowercase();

        if query_lower.contains("archive") {
            OrganizeAction::Archive
        } else if query_lower.contains("move") || query_lower.contains("rename") {
            OrganizeAction::Move
        } else if query_lower.contains("copy") || query_lower.contains("duplicate") {
            OrganizeAction::Copy
        } else if query_lower.contains("organize") || query_lower.contains("sort") {
            OrganizeAction::Organize
        } else {
            OrganizeAction::Unknown
        }
    }
}

/// Types of organization actions
#[derive(Debug, Clone, Copy, PartialEq)]
enum OrganizeAction {
    Archive,
    Move,
    Copy,
    Organize,
    Unknown,
}

impl Default for FileOrganize {
    fn default() -> Self {
        Self::new(PathBuf::from("~/.little-helper/archive"))
    }
}

#[async_trait]
impl Skill for FileOrganize {
    fn id(&self) -> &'static str {
        "file_organize"
    }

    fn name(&self) -> &'static str {
        "Organize Files"
    }

    fn description(&self) -> &'static str {
        "Safely organize, move, and archive files (no deletion - files are always preserved)"
    }

    fn permission_level(&self) -> PermissionLevel {
        // Sensitive because it modifies the file system
        PermissionLevel::Sensitive
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Find, Mode::Fix]
    }

    async fn execute(&self, input: SkillInput, ctx: &SkillContext) -> Result<SkillOutput> {
        let query = &input.query;

        // CRITICAL: Check for deletion requests first
        if Self::is_deletion_request(query) {
            // Extract file path from query or params
            let file_path = input
                .params
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| extract_file_path_from_query(query).unwrap_or_default());

            let message = Self::deletion_refusal_message(&file_path);

            return Ok(SkillOutput {
                result_type: ResultType::Text,
                text: Some(message),
                files: Vec::new(),
                data: Some(serde_json::json!({
                    "action": "deletion_refused",
                    "reason": "no_delete_policy",
                    "alternative": "archive",
                    "file_path": file_path,
                })),
                citations: Vec::new(),
                suggested_actions: if !file_path.is_empty() {
                    vec![
                        SuggestedAction {
                            label: "Archive instead".to_string(),
                            skill_id: "file_organize".to_string(),
                            params: [
                                ("path".to_string(), serde_json::json!(file_path)),
                                ("action".to_string(), serde_json::json!("archive")),
                            ]
                            .into_iter()
                            .collect(),
                        },
                        SuggestedAction {
                            label: "Move to different folder".to_string(),
                            skill_id: "file_organize".to_string(),
                            params: [
                                ("path".to_string(), serde_json::json!(file_path)),
                                ("action".to_string(), serde_json::json!("move")),
                            ]
                            .into_iter()
                            .collect(),
                        },
                    ]
                } else {
                    Vec::new()
                },
            });
        }

        // Get action and path from params or parse from query
        let action_str = input
            .params
            .get("action")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let action = if let Some(ref a) = action_str {
            match a.as_str() {
                "archive" => OrganizeAction::Archive,
                "move" => OrganizeAction::Move,
                "copy" => OrganizeAction::Copy,
                "organize" => OrganizeAction::Organize,
                _ => Self::parse_organize_action(query),
            }
        } else {
            Self::parse_organize_action(query)
        };

        let source_path = input
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| extract_file_path_from_query(query));

        let dest_path = input
            .params
            .get("destination")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Handle based on action
        match action {
            OrganizeAction::Archive => {
                let source = match source_path {
                    Some(p) => resolve_path(&p, &ctx.working_dir),
                    None => {
                        return Ok(SkillOutput::text(
                            "Please specify which file to archive.\n\n\
                             Example: \"archive old_report.pdf\"",
                        ));
                    }
                };

                if !source.exists() {
                    return Ok(SkillOutput::text(format!(
                        "File not found: {}\n\nPlease check the path and try again.",
                        source.display()
                    )));
                }

                let file_name = source
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                match self.safe_ops.archive_file(&source) {
                    Ok(FileAction::Archived { to }) => Ok(SkillOutput {
                        result_type: ResultType::Files,
                        text: Some(format!(
                            "Archived '{}'\n\n\
                                 The file has been moved to:\n{}\n\n\
                                 You can restore it anytime from the archive.",
                            file_name,
                            to.display()
                        )),
                        files: vec![FileResult {
                            path: to.clone(),
                            action: FileAction::Archived { to: to.clone() },
                            preview: None,
                        }],
                        data: Some(serde_json::json!({
                            "action": "archived",
                            "original_path": source.to_string_lossy(),
                            "archive_path": to.to_string_lossy(),
                        })),
                        citations: Vec::new(),
                        suggested_actions: vec![SuggestedAction {
                            label: "View archive folder".to_string(),
                            skill_id: "file_preview".to_string(),
                            params: [(
                                "path".to_string(),
                                serde_json::json!(to
                                    .parent()
                                    .map(|p| p.to_string_lossy())
                                    .unwrap_or_default()),
                            )]
                            .into_iter()
                            .collect(),
                        }],
                    }),
                    Ok(_) => Ok(SkillOutput::text("File operation completed.")),
                    Err(e) => Ok(SkillOutput::error(format!("Failed to archive file: {}", e))),
                }
            }

            OrganizeAction::Move => {
                let source = match source_path {
                    Some(p) => resolve_path(&p, &ctx.working_dir),
                    None => {
                        return Ok(SkillOutput::text(
                            "Please specify the file to move and destination.\n\n\
                             Example: \"move report.pdf to ~/Documents/Reports/\"",
                        ));
                    }
                };

                let dest = match dest_path {
                    Some(p) => resolve_path(&p, &ctx.working_dir),
                    None => {
                        return Ok(SkillOutput::text(format!(
                            "Where would you like to move '{}'?\n\n\
                             Example: \"move {} to ~/Documents/\"",
                            source
                                .file_name()
                                .map(|n| n.to_string_lossy())
                                .unwrap_or_default(),
                            source.display()
                        )));
                    }
                };

                if !source.exists() {
                    return Ok(SkillOutput::text(format!(
                        "File not found: {}",
                        source.display()
                    )));
                }

                let file_name = source
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                // If dest is a directory, append filename
                let final_dest = if dest.is_dir() {
                    dest.join(&file_name)
                } else {
                    dest
                };

                match self.safe_ops.move_file(&source, &final_dest) {
                    Ok(action) => Ok(SkillOutput {
                        result_type: ResultType::Files,
                        text: Some(format!(
                            "Moved '{}'\n\n\
                                 From: {}\n\
                                 To: {}",
                            file_name,
                            source.display(),
                            final_dest.display()
                        )),
                        files: vec![FileResult {
                            path: final_dest.clone(),
                            action,
                            preview: None,
                        }],
                        data: Some(serde_json::json!({
                            "action": "moved",
                            "from": source.to_string_lossy(),
                            "to": final_dest.to_string_lossy(),
                        })),
                        citations: Vec::new(),
                        suggested_actions: Vec::new(),
                    }),
                    Err(e) => Ok(SkillOutput::error(format!("Failed to move file: {}", e))),
                }
            }

            OrganizeAction::Copy => {
                let source = match source_path {
                    Some(p) => resolve_path(&p, &ctx.working_dir),
                    None => {
                        return Ok(SkillOutput::text(
                            "Please specify the file to copy and destination.\n\n\
                             Example: \"copy config.yaml to ~/backup/\"",
                        ));
                    }
                };

                let dest = match dest_path {
                    Some(p) => resolve_path(&p, &ctx.working_dir),
                    None => {
                        return Ok(SkillOutput::text(format!(
                            "Where would you like to copy '{}'?",
                            source
                                .file_name()
                                .map(|n| n.to_string_lossy())
                                .unwrap_or_default()
                        )));
                    }
                };

                if !source.exists() {
                    return Ok(SkillOutput::text(format!(
                        "File not found: {}",
                        source.display()
                    )));
                }

                let file_name = source
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let final_dest = if dest.is_dir() {
                    dest.join(&file_name)
                } else {
                    dest
                };

                match self.safe_ops.copy_file(&source, &final_dest) {
                    Ok(action) => Ok(SkillOutput {
                        result_type: ResultType::Files,
                        text: Some(format!(
                            "Copied '{}'\n\n\
                                 Original: {}\n\
                                 Copy: {}",
                            file_name,
                            source.display(),
                            final_dest.display()
                        )),
                        files: vec![FileResult {
                            path: final_dest.clone(),
                            action,
                            preview: None,
                        }],
                        data: Some(serde_json::json!({
                            "action": "copied",
                            "source": source.to_string_lossy(),
                            "copy": final_dest.to_string_lossy(),
                        })),
                        citations: Vec::new(),
                        suggested_actions: Vec::new(),
                    }),
                    Err(e) => Ok(SkillOutput::error(format!("Failed to copy file: {}", e))),
                }
            }

            OrganizeAction::Organize | OrganizeAction::Unknown => Ok(SkillOutput::text(
                "How would you like to organize your files?\n\n\
                     I can help you:\n\
                     - **Archive** files (safely store them with timestamps)\n\
                     - **Move** files to different folders\n\
                     - **Copy** files to backup locations\n\n\
                     Note: I never delete files - your data is always safe!\n\n\
                     Example commands:\n\
                     - \"archive old_project.zip\"\n\
                     - \"move report.pdf to ~/Documents/2024/\"\n\
                     - \"copy config.yaml to ~/backup/\"",
            )),
        }
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        // Allow any input - we handle guidance in execute
        if input.query.trim().is_empty() && input.params.is_empty() {
            anyhow::bail!("Please tell me how you'd like to organize your files");
        }
        Ok(())
    }
}

/// Extract a file path from a natural language query
fn extract_file_path_from_query(query: &str) -> Option<String> {
    // First, look for quoted paths (handles "my file.doc" with spaces)
    if let (Some(start), Some(end)) = (query.find('"'), query.rfind('"')) {
        if end > start {
            return Some(query[start + 1..end].to_string());
        }
    }

    // Then look for words with file extensions
    for word in query.split_whitespace() {
        let clean = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '.' && c != '/' && c != '\\' && c != '_' && c != '-'
        });
        if clean.contains('.') && !clean.starts_with('.') && clean.len() > 2 {
            return Some(clean.to_string());
        }
    }

    None
}

/// Resolve a path relative to working directory if not absolute
fn resolve_path(path_str: &str, working_dir: &PathBuf) -> PathBuf {
    let path = PathBuf::from(path_str);
    if path.is_absolute() {
        path
    } else if path_str.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(&path_str[2..])
        } else {
            path
        }
    } else {
        working_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deletion_detection() {
        assert!(FileOrganize::is_deletion_request("delete this file"));
        assert!(FileOrganize::is_deletion_request("please remove old.txt"));
        assert!(FileOrganize::is_deletion_request("trash these files"));
        assert!(FileOrganize::is_deletion_request("get rid of temp files"));
        assert!(FileOrganize::is_deletion_request("rm -rf folder"));

        assert!(!FileOrganize::is_deletion_request("archive this file"));
        assert!(!FileOrganize::is_deletion_request("move to folder"));
        assert!(!FileOrganize::is_deletion_request("organize my downloads"));
    }

    #[test]
    fn test_action_parsing() {
        assert_eq!(
            FileOrganize::parse_organize_action("archive old files"),
            OrganizeAction::Archive
        );
        assert_eq!(
            FileOrganize::parse_organize_action("move report.pdf"),
            OrganizeAction::Move
        );
        assert_eq!(
            FileOrganize::parse_organize_action("copy config"),
            OrganizeAction::Copy
        );
        assert_eq!(
            FileOrganize::parse_organize_action("organize downloads"),
            OrganizeAction::Organize
        );
    }

    #[test]
    fn test_file_path_extraction() {
        assert_eq!(
            extract_file_path_from_query("delete report.pdf"),
            Some("report.pdf".to_string())
        );
        assert_eq!(
            extract_file_path_from_query("move \"my file.doc\" to folder"),
            Some("my file.doc".to_string())
        );
    }
}
