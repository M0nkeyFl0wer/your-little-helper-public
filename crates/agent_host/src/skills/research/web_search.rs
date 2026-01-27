//! Web search skill for Research mode.
//!
//! Provides web search capabilities to find information online.
//! This is a framework skill that can be connected to various search providers.

use crate::executor;
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
        Self { enabled: true }
    }

    /// Enable web search (when API is configured)
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Perform a web search
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let result = executor::web_search(query).await?;
        let parsed = parse_results(&result.stdout);
        Ok(parsed)
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

fn parse_results(text: &str) -> Vec<SearchResult> {
    text.split("\n\n")
        .filter_map(|block| {
            let lines: Vec<String> = block
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if lines.len() < 3 {
                return None;
            }

            let title_line = lines.first()?.to_string();
            let title = title_line
                .splitn(2, ". ")
                .nth(1)
                .unwrap_or(title_line.trim())
                .trim()
                .to_string();

            let snippet = lines.get(1).cloned().unwrap_or_default();
            let url_line = lines
                .iter()
                .find(|line| line.starts_with("URL:"))
                .cloned()
                .unwrap_or_default();
            if title.is_empty() || url_line.is_empty() {
                return None;
            }
            let url = url_line.trim_start_matches("URL:").trim().to_string();
            Some(SearchResult {
                title,
                url: url.clone(),
                domain: extract_domain(&url),
                snippet,
            })
        })
        .collect()
}

fn extract_domain(url: &str) -> String {
    let domain = url
        .split('/')
        .nth(2)
        .unwrap_or(url)
        .trim_start_matches("www.")
        .to_string();
    if domain.is_empty() {
        url.to_string()
    } else {
        domain
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
        if !self.enabled {
            return Ok(SkillOutput::text(
                "Web search is disabled. Enable internet research in Settings to run searches.",
            ));
        }

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
