//! Source evaluation skill for Research mode.
//!
//! Helps evaluate the credibility and reliability of information sources.
//! Provides guidance on assessing source quality for research.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};

/// Source evaluation skill.
pub struct SourceEvaluator;

impl SourceEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Analyze a URL or domain for credibility indicators
    fn analyze_source(url: &str) -> SourceAnalysis {
        let url_lower = url.to_lowercase();

        // Extract domain
        let domain = extract_domain(&url_lower);

        let mut indicators = Vec::new();
        let mut score = 50; // Start at neutral

        // Check for known reliable sources
        if is_academic_domain(&domain) {
            indicators.push(SourceIndicator {
                category: "Domain Type".to_string(),
                finding: "Academic/Educational (.edu) domain".to_string(),
                impact: Impact::Positive,
            });
            score += 20;
        } else if is_government_domain(&domain) {
            indicators.push(SourceIndicator {
                category: "Domain Type".to_string(),
                finding: "Government (.gov) domain".to_string(),
                impact: Impact::Positive,
            });
            score += 15;
        } else if is_organization_domain(&domain) {
            indicators.push(SourceIndicator {
                category: "Domain Type".to_string(),
                finding: "Organization (.org) domain".to_string(),
                impact: Impact::Neutral,
            });
            // .org is neutral - can be reliable or not
        }

        // Check for known news/media sources
        if is_major_news_source(&domain) {
            indicators.push(SourceIndicator {
                category: "Source Type".to_string(),
                finding: "Established news organization".to_string(),
                impact: Impact::Positive,
            });
            score += 10;
        }

        // Check for Wikipedia (reliable for general info, cite their sources)
        if domain.contains("wikipedia") {
            indicators.push(SourceIndicator {
                category: "Source Type".to_string(),
                finding: "Wikipedia article - check cited sources for academic use".to_string(),
                impact: Impact::Neutral,
            });
        }

        // Check for social media (generally less reliable for facts)
        if is_social_media(&domain) {
            indicators.push(SourceIndicator {
                category: "Source Type".to_string(),
                finding: "Social media platform - verify claims with primary sources".to_string(),
                impact: Impact::Negative,
            });
            score -= 15;
        }

        // Check for secure connection
        if url.starts_with("https://") {
            indicators.push(SourceIndicator {
                category: "Security".to_string(),
                finding: "Uses HTTPS (encrypted connection)".to_string(),
                impact: Impact::Positive,
            });
            score += 5;
        } else if url.starts_with("http://") {
            indicators.push(SourceIndicator {
                category: "Security".to_string(),
                finding: "Uses HTTP (not encrypted)".to_string(),
                impact: Impact::Negative,
            });
            score -= 5;
        }

        SourceAnalysis {
            url: url.to_string(),
            domain,
            credibility_score: score.clamp(0, 100) as u8,
            indicators,
        }
    }

    /// Format the analysis for display
    fn format_analysis(analysis: &SourceAnalysis) -> String {
        let mut output = String::new();

        output.push_str("## Source Evaluation\n\n");
        output.push_str(&format!("**URL**: {}\n", analysis.url));
        output.push_str(&format!("**Domain**: {}\n\n", analysis.domain));

        // Credibility score with visual indicator
        let score_icon = if analysis.credibility_score >= 75 {
            "🟢"
        } else if analysis.credibility_score >= 50 {
            "🟡"
        } else {
            "🔴"
        };

        output.push_str(&format!(
            "### Credibility Score: {} {}/100\n\n",
            score_icon, analysis.credibility_score
        ));

        // Indicators
        if !analysis.indicators.is_empty() {
            output.push_str("### Findings\n\n");
            for indicator in &analysis.indicators {
                let icon = match indicator.impact {
                    Impact::Positive => "✅",
                    Impact::Neutral => "ℹ️",
                    Impact::Negative => "⚠️",
                };
                output.push_str(&format!(
                    "{} **{}**: {}\n",
                    icon, indicator.category, indicator.finding
                ));
            }
            output.push('\n');
        }

        // General guidance
        output.push_str("### Research Tips\n\n");
        output.push_str("When evaluating sources, consider:\n\n");
        output.push_str("1. **Authority**: Who wrote it? What are their credentials?\n");
        output.push_str("2. **Accuracy**: Are claims supported with evidence?\n");
        output.push_str("3. **Currency**: When was it published? Is it still relevant?\n");
        output.push_str("4. **Purpose**: Is it informing, persuading, or selling?\n");
        output.push_str("5. **Cross-reference**: Do other reliable sources agree?\n");

        output
    }
}

impl Default for SourceEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract domain from URL
fn extract_domain(url: &str) -> String {
    let without_protocol = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    without_protocol
        .split('/')
        .next()
        .unwrap_or(without_protocol)
        .to_string()
}

fn is_academic_domain(domain: &str) -> bool {
    domain.ends_with(".edu") || domain.ends_with(".ac.uk") || domain.contains("university")
}

fn is_government_domain(domain: &str) -> bool {
    domain.ends_with(".gov") || domain.ends_with(".gov.uk") || domain.ends_with(".gc.ca")
}

fn is_organization_domain(domain: &str) -> bool {
    domain.ends_with(".org")
}

fn is_major_news_source(domain: &str) -> bool {
    let news_domains = [
        "nytimes.com",
        "washingtonpost.com",
        "bbc.com",
        "bbc.co.uk",
        "reuters.com",
        "apnews.com",
        "npr.org",
        "theguardian.com",
        "wsj.com",
        "economist.com",
        "nature.com",
        "sciencemag.org",
    ];
    news_domains.iter().any(|d| domain.contains(d))
}

fn is_social_media(domain: &str) -> bool {
    let social_domains = [
        "twitter.com",
        "x.com",
        "facebook.com",
        "instagram.com",
        "tiktok.com",
        "reddit.com",
        "youtube.com",
        "linkedin.com",
    ];
    social_domains.iter().any(|d| domain.contains(d))
}

struct SourceAnalysis {
    url: String,
    domain: String,
    credibility_score: u8,
    indicators: Vec<SourceIndicator>,
}

struct SourceIndicator {
    category: String,
    finding: String,
    impact: Impact,
}

#[derive(Clone, Copy)]
enum Impact {
    Positive,
    Neutral,
    Negative,
}

#[async_trait]
impl Skill for SourceEvaluator {
    fn id(&self) -> &'static str {
        "source_evaluator"
    }

    fn name(&self) -> &'static str {
        "Source Evaluator"
    }

    fn description(&self) -> &'static str {
        "Evaluate the credibility and reliability of information sources"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
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
            .or_else(|| {
                // Look for URL in query
                for word in input.query.split_whitespace() {
                    if word.starts_with("http://") || word.starts_with("https://") {
                        return Some(
                            word.trim_matches(|c: char| {
                                !c.is_alphanumeric() && c != ':' && c != '/' && c != '.' && c != '-'
                            })
                            .to_string(),
                        );
                    }
                }
                None
            });

        let url = match url {
            Some(u) => u,
            None => {
                return Ok(SkillOutput::text(
                    "Please provide a URL to evaluate.\n\n\
                     Example: \"Evaluate https://example.com/article\"\n\n\
                     I'll analyze the source for credibility indicators.",
                ));
            }
        };

        let analysis = Self::analyze_source(&url);
        let formatted = Self::format_analysis(&analysis);

        Ok(SkillOutput {
            result_type: ResultType::Text,
            text: Some(formatted),
            files: Vec::new(),
            data: Some(serde_json::json!({
                "url": url,
                "domain": analysis.domain,
                "credibility_score": analysis.credibility_score,
                "indicator_count": analysis.indicators.len(),
            })),
            citations: Vec::new(),
            suggested_actions: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("https://www.example.com/page"),
            "www.example.com"
        );
        assert_eq!(extract_domain("http://test.org"), "test.org");
    }

    #[test]
    fn test_academic_source() {
        let analysis = SourceEvaluator::analyze_source("https://www.harvard.edu/research");
        assert!(analysis.credibility_score >= 70);
    }

    #[test]
    fn test_social_media() {
        let analysis = SourceEvaluator::analyze_source("https://twitter.com/user/status/123");
        assert!(analysis.credibility_score < 50);
    }
}
