//! Text polishing skill for Content mode.
//!
//! Helps improve text quality by suggesting improvements
//! for clarity, conciseness, and style.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};

/// Text polishing skill.
pub struct TextPolisher;

impl TextPolisher {
    pub fn new() -> Self {
        Self
    }

    /// Analyze text for potential improvements
    fn analyze_text(text: &str) -> TextAnalysis {
        let words: Vec<&str> = text.split_whitespace().collect();
        let sentences: Vec<&str> = text
            .split(|c| c == '.' || c == '!' || c == '?')
            .filter(|s| !s.trim().is_empty())
            .collect();

        let word_count = words.len();
        let sentence_count = sentences.len().max(1);
        let avg_words_per_sentence = word_count as f32 / sentence_count as f32;

        let mut suggestions = Vec::new();

        // Check sentence length
        if avg_words_per_sentence > 25.0 {
            suggestions.push(TextSuggestion {
                category: "Readability".to_string(),
                issue: "Long sentences".to_string(),
                suggestion: "Consider breaking long sentences into shorter ones for clarity."
                    .to_string(),
            });
        }

        // Check for passive voice indicators
        let passive_words = ["was", "were", "been", "being", "is", "are", "am"];
        let passive_count = words
            .iter()
            .filter(|w| passive_words.contains(&w.to_lowercase().as_str()))
            .count();
        if passive_count > word_count / 10 && word_count > 20 {
            suggestions.push(TextSuggestion {
                category: "Style".to_string(),
                issue: "Possible passive voice".to_string(),
                suggestion: "Consider using active voice for more direct, engaging writing."
                    .to_string(),
            });
        }

        // Check for filler words
        let fillers = [
            "very",
            "really",
            "just",
            "actually",
            "basically",
            "literally",
        ];
        let filler_count = words
            .iter()
            .filter(|w| fillers.contains(&w.to_lowercase().as_str()))
            .count();
        if filler_count > 0 {
            suggestions.push(TextSuggestion {
                category: "Conciseness".to_string(),
                issue: format!("Found {} filler word(s)", filler_count),
                suggestion:
                    "Remove filler words like 'very', 'really', 'just' for stronger writing."
                        .to_string(),
            });
        }

        // Check for repetition
        let mut word_freq: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for word in &words {
            let lower = word.to_lowercase();
            if lower.len() > 4 {
                // Only check longer words
                *word_freq.entry(lower).or_insert(0) += 1;
            }
        }
        let repeated: Vec<_> = word_freq.iter().filter(|(_, count)| **count > 3).collect();
        if !repeated.is_empty() && word_count > 50 {
            suggestions.push(TextSuggestion {
                category: "Variety".to_string(),
                issue: "Repeated words".to_string(),
                suggestion: "Consider using synonyms for frequently repeated words.".to_string(),
            });
        }

        // Check for weak starts
        let weak_starts = ["there is", "there are", "it is", "it was"];
        let text_lower = text.to_lowercase();
        for start in weak_starts {
            if text_lower.contains(start) {
                suggestions.push(TextSuggestion {
                    category: "Strength".to_string(),
                    issue: format!("Weak phrase: '{}'", start),
                    suggestion: "Restructure to lead with the subject and a strong verb."
                        .to_string(),
                });
                break;
            }
        }

        TextAnalysis {
            word_count,
            sentence_count,
            avg_words_per_sentence,
            suggestions,
        }
    }

    /// Format the analysis for display
    fn format_analysis(text: &str, analysis: &TextAnalysis) -> String {
        let mut output = String::new();

        output.push_str("## Text Analysis\n\n");

        // Statistics
        output.push_str("### Statistics\n");
        output.push_str(&format!("- **Words**: {}\n", analysis.word_count));
        output.push_str(&format!("- **Sentences**: {}\n", analysis.sentence_count));
        output.push_str(&format!(
            "- **Avg. words/sentence**: {:.1}\n\n",
            analysis.avg_words_per_sentence
        ));

        // Readability assessment
        let readability = if analysis.avg_words_per_sentence < 15.0 {
            "Easy to read"
        } else if analysis.avg_words_per_sentence < 20.0 {
            "Moderately readable"
        } else if analysis.avg_words_per_sentence < 25.0 {
            "Somewhat complex"
        } else {
            "Complex - consider simplifying"
        };
        output.push_str(&format!("**Readability**: {}\n\n", readability));

        // Suggestions
        if analysis.suggestions.is_empty() {
            output.push_str("### Assessment\n\n");
            output.push_str("✅ Your text looks good! No major issues detected.\n");
        } else {
            output.push_str("### Suggestions\n\n");
            for suggestion in &analysis.suggestions {
                output.push_str(&format!(
                    "**{}** - {}\n→ {}\n\n",
                    suggestion.category, suggestion.issue, suggestion.suggestion
                ));
            }
        }

        // Preview of original text
        output.push_str("### Original Text\n\n");
        let preview = if text.len() > 200 {
            format!("{}...", &text[..200])
        } else {
            text.to_string()
        };
        output.push_str(&format!("> {}\n", preview.replace('\n', "\n> ")));

        output
    }
}

impl Default for TextPolisher {
    fn default() -> Self {
        Self::new()
    }
}

struct TextAnalysis {
    word_count: usize,
    sentence_count: usize,
    avg_words_per_sentence: f32,
    suggestions: Vec<TextSuggestion>,
}

struct TextSuggestion {
    category: String,
    issue: String,
    suggestion: String,
}

#[async_trait]
impl Skill for TextPolisher {
    fn id(&self) -> &'static str {
        "text_polisher"
    }

    fn name(&self) -> &'static str {
        "Text Polisher"
    }

    fn description(&self) -> &'static str {
        "Analyze and improve text for clarity and style"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Content]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        if input.query.trim().is_empty() {
            return Ok(SkillOutput::text(
                "Paste some text and I'll analyze it for improvements.\n\n\
                 I can help with:\n\
                 - Readability and sentence structure\n\
                 - Finding filler words to remove\n\
                 - Identifying passive voice\n\
                 - Spotting repetition\n\n\
                 Just paste your text and I'll give you suggestions!",
            ));
        }

        let analysis = Self::analyze_text(&input.query);
        let formatted = Self::format_analysis(&input.query, &analysis);

        Ok(SkillOutput {
            result_type: ResultType::Text,
            text: Some(formatted),
            files: Vec::new(),
            data: Some(serde_json::json!({
                "word_count": analysis.word_count,
                "sentence_count": analysis.sentence_count,
                "avg_words_per_sentence": analysis.avg_words_per_sentence,
                "suggestion_count": analysis.suggestions.len(),
            })),
            citations: Vec::new(),
            suggested_actions: Vec::new(),
        })
    }
}
