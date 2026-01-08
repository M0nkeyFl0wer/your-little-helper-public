//! Web search skill for Research mode.
//!
//! Provides web search capabilities to find information online.
//! This is a framework skill that can be connected to various search providers.

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use shared::skill::{
    Citation, Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};

/// Web search skill for research.
pub struct WebSearch {
    /// Whether web search is currently available
    enabled: bool,
}

impl WebSearch {
    pub fn new() -> Self {
        Self {
            // Web search requires API setup
            enabled: false,
        }
    }

    /// Enable web search (when API is configured)
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Perform a web search
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        // This would connect to a search API (DuckDuckGo, Brave, etc.)
        // For now, return guidance on setting up search

        // Placeholder: In production, this would call:
        // - DuckDuckGo Instant Answer API
        // - Brave Search API
        // - or other search providers

        Ok(vec![])
    }

    /// Format search results for display
    fn format_results(query: &str, results: &[SearchResult]) -> String {
        if results.is_empty() {
            return format!(
                "## Web Search\n\n\
                 **Query**: \"{}\"\n\n\
                 Web search is not yet configured. To enable:\n\n\
                 1. Go to Settings > Research\n\
                 2. Add a search API key (Brave, DuckDuckGo, etc.)\n\n\
                 In the meantime, you can:\n\
                 - Open your browser and search manually\n\
                 - Ask me to analyze a specific URL\n",
                query
            );
        }

        let mut output = String::new();
        output.push_str(&format!("## Search Results for \"{}\"\n\n", query));

        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!(
                "### {}. {}\n\
                 **Source**: [{}]({})\n\n\
                 {}\n\n",
                i + 1,
                result.title,
                result.domain,
                result.url,
                result.snippet
            ));
        }

        output.push_str(&format!("\n*Found {} results*\n", results.len()));
        output
    }
}

impl Default for WebSearch {
    fn default() -> Self {
        Self::new()
    }
}

/// A single search result
#[derive(Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub domain: String,
    pub snippet: String,
}

#[async_trait]
impl Skill for WebSearch {
    fn id(&self) -> &'static str {
        "web_search"
    }

    fn name(&self) -> &'static str {
        "Web Search"
    }

    fn description(&self) -> &'static str {
        "Search the web for information on any topic"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Research]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        if input.query.trim().is_empty() {
            return Ok(SkillOutput::text(
                "What would you like to search for?\n\n\
                 Example: \"What are the benefits of renewable energy?\"",
            ));
        }

        let results = self.search(&input.query).await?;
        let formatted = Self::format_results(&input.query, &results);

        // Convert results to citations
        let citations: Vec<Citation> = results
            .iter()
            .map(|r| Citation {
                text: format!("{}: {}", r.title, r.snippet),
                url: r.url.clone(),
                accessed_at: Utc::now(),
                verified: false,
            })
            .collect();

        Ok(SkillOutput {
            result_type: ResultType::Text,
            text: Some(formatted),
            files: Vec::new(),
            data: Some(serde_json::json!({
                "query": input.query,
                "result_count": results.len(),
                "enabled": self.enabled,
            })),
            citations,
            suggested_actions: Vec::new(),
        })
    }
}
