//! Fuzzy file search skill for Find mode.
//!
//! Provides fzf-like fuzzy file search across indexed drives with sub-second results.

use anyhow::Result;
use async_trait::async_trait;
use services::file_index::{FileIndexService, FileSearchResult};
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};
use std::sync::Arc;

/// Fuzzy file search skill.
///
/// Searches indexed files using FTS5 + Jaro-Winkler similarity for fzf-like matching.
pub struct FuzzyFileSearch {
    file_index: Arc<FileIndexService>,
}

impl FuzzyFileSearch {
    pub fn new(file_index: Arc<FileIndexService>) -> Self {
        Self { file_index }
    }

    /// Format search results for display
    fn format_results(&self, results: &[FileSearchResult], query: &str) -> String {
        if results.is_empty() {
            return format!("No files found matching '{}'", query);
        }

        let mut output = format!("Found {} files matching '{}':\n\n", results.len(), query);

        for (i, result) in results.iter().enumerate() {
            let size = format_size(result.size_bytes);
            let modified = result.modified_at.format("%Y-%m-%d %H:%M");
            let score_pct = (result.score * 100.0) as u8;

            output.push_str(&format!(
                "{}. {} ({}%)\n   {} | {}\n   {}\n\n",
                i + 1,
                result.name,
                score_pct,
                size,
                modified,
                result.path.display()
            ));
        }

        output
    }
}

#[async_trait]
impl Skill for FuzzyFileSearch {
    fn id(&self) -> &'static str {
        "fuzzy_file_search"
    }

    fn name(&self) -> &'static str {
        "Fuzzy File Search"
    }

    fn description(&self) -> &'static str {
        "Search files across all indexed drives with fuzzy matching (like fzf)"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Find]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        let query = input.query.trim();

        if query.is_empty() {
            return Ok(SkillOutput::text("Please provide a search query."));
        }

        // Get limit from params or default to 20
        let limit = input
            .params
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;

        // Perform hybrid search (FTS5 + Jaro-Winkler + embeddings if available)
        // Falls back to FTS5 + Jaro-Winkler when no embeddings exist
        let results = self.file_index.semantic_search(query, None, limit)?;

        // Format output
        let text = self.format_results(&results, query);

        // Include structured data for UI
        let data = serde_json::to_value(&results)?;

        Ok(SkillOutput {
            result_type: ResultType::Mixed,
            text: Some(text),
            files: Vec::new(),
            data: Some(data),
            citations: Vec::new(),
            suggested_actions: self.build_suggestions(&results),
        })
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        if input.query.trim().is_empty() {
            anyhow::bail!("Search query cannot be empty");
        }
        Ok(())
    }
}

impl FuzzyFileSearch {
    fn build_suggestions(
        &self,
        results: &[FileSearchResult],
    ) -> Vec<shared::skill::SuggestedAction> {
        let mut suggestions = Vec::new();

        // Suggest opening top result if available
        if let Some(top) = results.first() {
            suggestions.push(shared::skill::SuggestedAction {
                label: format!("Open {}", top.name),
                skill_id: "file_preview".to_string(),
                params: [(
                    "path".to_string(),
                    serde_json::json!(top.path.to_string_lossy()),
                )]
                .into_iter()
                .collect(),
            });
        }

        // Suggest refining search
        suggestions.push(shared::skill::SuggestedAction {
            label: "Refine search".to_string(),
            skill_id: "fuzzy_file_search".to_string(),
            params: std::collections::HashMap::new(),
        });

        suggestions
    }
}

/// Format file size in human-readable form
fn format_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1_572_864), "1.5 MB");
        assert_eq!(format_size(1_610_612_736), "1.5 GB");
    }
}
