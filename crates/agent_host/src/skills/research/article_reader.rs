//! Article reader skill for Research mode.
//!
//! Fetches and extracts readable content from web pages,
//! providing summaries and key information.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use html2text::from_read;
use regex::Regex;
use reqwest::Client;
use shared::skill::{
    Citation, Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};
use std::time::Duration;

/// Article reader skill.
pub struct ArticleReader {
    client: Client,
}

impl ArticleReader {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Extract URL from user query
    fn extract_url(query: &str) -> Option<String> {
        // Look for http:// or https:// URLs
        for word in query.split_whitespace() {
            let clean = word.trim_matches(|c: char| c == '"' || c == '\'' || c == '<' || c == '>');
            if clean.starts_with("http://") || clean.starts_with("https://") {
                return Some(clean.to_string());
            }
        }
        None
    }

    /// Fetch and parse article content
    async fn fetch_article(&self, url: &str) -> Result<ArticleContent> {
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(20))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("article fetch failed with {}", response.status()));
        }
        let html = response.text().await?;
        let title = extract_title(&html);
        let text = from_read(html.as_bytes(), 80);
        let content = text.trim().to_string();
        if content.is_empty() {
            return Err(anyhow!("article did not return readable content"));
        }
        let word_count = content.split_whitespace().count() as u32;
        let read_time_minutes = estimate_read_time(word_count);

        Ok(ArticleContent {
            url: url.to_string(),
            title,
            author: None,
            published_date: None,
            content: Some(content),
            word_count,
            read_time_minutes,
            fetched: true,
        })
    }

    /// Format article content for display
    fn format_article(article: &ArticleContent) -> String {
        if !article.fetched {
            return format!(
                "## Article Reader\n\n\
                 **URL**: {}\n\n\
                 Article fetching is not yet enabled. To read this article:\n\n\
                 1. Network access needs to be enabled in Settings\n\
                 2. The article will be fetched and summarized\n\n\
                 **What I can do when enabled**:\n\
                 - Extract the main article text (removing ads, navigation, etc.)\n\
                 - Provide a summary of key points\n\
                 - Estimate reading time\n\
                 - Cite the source for your research\n",
                article.url
            );
        }

        let mut output = String::new();

        // Title
        if let Some(ref title) = article.title {
            output.push_str(&format!("# {}\n\n", title));
        }

        // Metadata
        if article.author.is_some() || article.published_date.is_some() {
            if let Some(ref author) = article.author {
                output.push_str(&format!("**Author**: {}\n", author));
            }
            if let Some(ref date) = article.published_date {
                output.push_str(&format!("**Published**: {}\n", date));
            }
            output.push('\n');
        }

        // Reading stats
        output.push_str(&format!(
            "*{} words · {} min read*\n\n",
            article.word_count, article.read_time_minutes
        ));

        // Content
        if let Some(ref content) = article.content {
            output.push_str("---\n\n");
            output.push_str(content);
            output.push_str("\n\n---\n");
        }

        // Source
        output.push_str(&format!(
            "\n**Source**: [{}]({})\n",
            article.url, article.url
        ));

        output
    }
}

fn extract_title(html: &str) -> Option<String> {
    let re = Regex::new(r"(?is)<title[^>]*>(.*?)</title>").ok()?;
    re.captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().trim().to_string())
}

fn estimate_read_time(word_count: u32) -> u32 {
    let minutes = (word_count + 199) / 200;
    minutes.max(1)
}

impl Default for ArticleReader {
    fn default() -> Self {
        Self::new()
    }
}

/// Extracted article content
struct ArticleContent {
    url: String,
    title: Option<String>,
    author: Option<String>,
    published_date: Option<String>,
    content: Option<String>,
    word_count: u32,
    read_time_minutes: u32,
    fetched: bool,
}

#[async_trait]
impl Skill for ArticleReader {
    fn id(&self) -> &'static str {
        "article_reader"
    }

    fn name(&self) -> &'static str {
        "Article Reader"
    }

    fn description(&self) -> &'static str {
        "Fetch and extract readable content from web pages"
    }

    fn permission_level(&self) -> PermissionLevel {
        // Requires network access
        PermissionLevel::Sensitive
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Research]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        // Try to extract URL from query or params
        let url = input
            .params
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Self::extract_url(&input.query));

        let url = match url {
            Some(u) => u,
            None => {
                return Ok(SkillOutput::text(
                    "Please provide a URL to read.\n\n\
                     Example: \"Read https://example.com/article\"\n\n\
                     I'll extract the main content and provide a summary.",
                ));
            }
        };

        let article = self.fetch_article(&url).await?;
        let formatted = Self::format_article(&article);

        Ok(SkillOutput {
            result_type: ResultType::Text,
            text: Some(formatted),
            files: Vec::new(),
            data: Some(serde_json::json!({
                "url": url,
                "title": article.title,
                "word_count": article.word_count,
                "read_time_minutes": article.read_time_minutes,
                "fetched": article.fetched,
            })),
            citations: vec![Citation {
                text: article.title.clone().unwrap_or_else(|| url.clone()),
                url: url.clone(),
                accessed_at: Utc::now(),
                verified: article.fetched,
            }],
            suggested_actions: Vec::new(),
        })
    }
}
