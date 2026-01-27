//! Drive indexing skill for Find mode.
//!
//! Scans drives/directories and adds files to the search index.

use anyhow::Result;
use async_trait::async_trait;
use services::file_index::{FileIndexService, ScanStats};
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Drive indexing skill.
///
/// Scans directories and adds files to the fuzzy search index.
pub struct DriveIndex {
    file_index: Arc<FileIndexService>,
}

impl DriveIndex {
    pub fn new(file_index: Arc<FileIndexService>) -> Self {
        Self { file_index }
    }

    fn format_stats(&self, path: &str, stats: &ScanStats) -> String {
        format!(
            "Indexed {} of {} files from '{}'\n\
             Errors: {}\n\n\
             Total files in index: {}",
            stats.indexed,
            stats.total_files,
            path,
            stats.errors,
            self.file_index.file_count().unwrap_or(0)
        )
    }
}

#[async_trait]
impl Skill for DriveIndex {
    fn id(&self) -> &'static str {
        "drive_index"
    }

    fn name(&self) -> &'static str {
        "Index Drive"
    }

    fn description(&self) -> &'static str {
        "Scan a directory or drive and add files to the search index"
    }

    fn permission_level(&self) -> PermissionLevel {
        // Sensitive because it reads file system structure
        PermissionLevel::Sensitive
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Find]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        // Get path from params or query
        let path_str = input
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| input.query.trim().to_string());

        if path_str.is_empty() {
            return Ok(SkillOutput::text(
                "Please provide a directory path to index.\n\n\
                 Example: Index my Documents folder\n\
                 Or: /home/user/Documents",
            ));
        }

        let path = PathBuf::from(&path_str);

        // Verify path exists and is a directory
        if !path.exists() {
            return Ok(SkillOutput::text(format!(
                "Directory not found: {}\n\nPlease provide a valid directory path.",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Ok(SkillOutput::text(format!(
                "'{}' is a file, not a directory.\n\nPlease provide a directory path to index.",
                path.display()
            )));
        }

        // Generate drive ID from path
        let drive_id = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        // Perform scan
        let stats = self.file_index.scan_drive(&path, &drive_id)?;

        let text = self.format_stats(&path_str, &stats);
        let data = serde_json::json!({
            "path": path_str,
            "drive_id": drive_id,
            "stats": {
                "total_files": stats.total_files,
                "indexed": stats.indexed,
                "errors": stats.errors
            },
            "total_in_index": self.file_index.file_count().unwrap_or(0)
        });

        Ok(SkillOutput {
            result_type: ResultType::Data,
            text: Some(text),
            files: Vec::new(),
            data: Some(data),
            citations: Vec::new(),
            suggested_actions: vec![shared::skill::SuggestedAction {
                label: "Search files".to_string(),
                skill_id: "fuzzy_file_search".to_string(),
                params: std::collections::HashMap::new(),
            }],
        })
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        // Path can come from query or params
        let has_path = !input.query.trim().is_empty()
            || input.params.get("path").and_then(|v| v.as_str()).is_some();

        if !has_path {
            anyhow::bail!("Please provide a directory path to index");
        }
        Ok(())
    }
}

/// Get default directories to index based on platform
pub fn default_index_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        // Common user directories
        paths.push(home.join("Documents"));
        paths.push(home.join("Downloads"));
        paths.push(home.join("Desktop"));
        paths.push(home.join("Projects"));
    }

    // Filter to only existing directories
    paths.into_iter().filter(|p| p.is_dir()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_paths() {
        // Just verify it doesn't panic
        let paths = default_index_paths();
        // May or may not have paths depending on system
        println!("Default index paths: {:?}", paths);
    }
}
