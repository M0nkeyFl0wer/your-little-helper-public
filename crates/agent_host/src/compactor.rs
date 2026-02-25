//! Auto-compaction: summarize old messages when context fills up.
//!
//! Instead of silently dropping old messages (current behavior via
//! `build_api_messages_with_budget`), compaction asks the AI to produce
//! a concise summary. The summary replaces the original range in the
//! active branch, but the originals remain navigable in the session tree.
//!
//! ## How It Works
//!
//! 1. **Trigger**: `should_compact()` checks if the estimated token count
//!    exceeds `trigger_threshold_tokens`.
//! 2. **Compact**: `compact()` sends the oldest N messages to the model
//!    with a summarization prompt. The result is a `CompactionResult`
//!    containing the summary text and metadata.
//! 3. **Integration**: The caller creates a `NodeType::Compaction` node
//!    in the session tree (see `shared::session_tree`), replacing the
//!    summarized range on the active branch while preserving originals.
//!
//! ## Strategies
//!
//! - **Drop** (current behavior): discard oldest messages. Zero-cost but
//!   loses context permanently in the API call.
//! - **Summarize**: ask the model to produce a concise summary. Costs
//!   one extra API call but preserves key information.
//! - **Manual**: signal the UI to ask the user what to keep.
//!
//! ## Token Estimation
//!
//! Uses the same rough heuristic as `build_api_messages_with_budget`:
//! ~4 characters per token. This is intentionally conservative — better
//! to compact a bit early than to blow the context window.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Strategy for handling context overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionStrategy {
    /// Discard oldest messages (current default behavior).
    Drop,
    /// Summarize oldest messages via an extra model call.
    Summarize,
    /// Ask the user what to keep (not yet implemented in UI).
    Manual,
}

impl Default for CompactionStrategy {
    fn default() -> Self {
        CompactionStrategy::Summarize
    }
}

/// Configuration for the compaction system.
///
/// These values are tuned for the app's default 8000-token comfort budget
/// with 2000 reserved for the reply (leaving 6000 for context).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Compact when estimated context tokens exceed this threshold.
    /// Default: 6000 (matches the app's COMFORT_TOTAL_TOKENS - RESERVED_FOR_REPLY).
    pub trigger_threshold_tokens: u32,

    /// Target token count after compaction. The summarizer aims to
    /// reduce the compacted range to roughly this size.
    /// Default: 3000 (leaves plenty of room for new messages).
    pub target_after_compact_tokens: u32,

    /// Which strategy to use when compaction is triggered.
    pub strategy: CompactionStrategy,

    /// Minimum number of messages before compaction is even considered.
    /// Prevents compacting very short conversations where the summary
    /// would be as long as the originals.
    pub min_messages_to_compact: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            trigger_threshold_tokens: 6000,
            target_after_compact_tokens: 3000,
            strategy: CompactionStrategy::Summarize,
            min_messages_to_compact: 6,
        }
    }
}

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

/// The result of a compaction operation.
///
/// Contains the summary text and metadata needed to create a
/// `NodeType::Compaction` node in the session tree.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// The generated summary text.
    pub summary: String,
    /// How many messages were summarized.
    pub summarized_count: usize,
    /// Estimated token count of the original messages.
    pub original_token_count: u32,
    /// Estimated token count of the summary.
    pub summary_token_count: u32,
}

// ---------------------------------------------------------------------------
// Compactor
// ---------------------------------------------------------------------------

/// Handles context compaction decisions and summarization.
pub struct Compactor {
    pub config: CompactionConfig,
}

impl Compactor {
    /// Create a new compactor with the given configuration.
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// Create a compactor with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(CompactionConfig::default())
    }

    /// Estimate the token count for a string.
    ///
    /// Uses the same ~4 chars/token heuristic as the rest of the app.
    /// Intentionally conservative (overestimates) to avoid context overflow.
    pub fn estimate_tokens(text: &str) -> u32 {
        (text.len() as u32 + 3) / 4
    }

    /// Check whether compaction should be triggered.
    ///
    /// Returns `true` if the total estimated tokens exceed the threshold
    /// and there are enough messages to make summarization worthwhile.
    pub fn should_compact(&self, messages: &[(String, String)]) -> bool {
        if messages.len() < self.config.min_messages_to_compact {
            return false;
        }

        let total_tokens: u32 = messages
            .iter()
            .map(|(_, content)| Self::estimate_tokens(content))
            .sum();

        total_tokens > self.config.trigger_threshold_tokens
    }

    /// Determine how many messages from the front to compact.
    ///
    /// Finds the smallest prefix of messages whose removal would bring
    /// the remaining messages below `target_after_compact_tokens`.
    /// Always leaves at least the last 2 messages (current exchange).
    pub fn messages_to_compact(&self, messages: &[(String, String)]) -> usize {
        if messages.len() < self.config.min_messages_to_compact {
            return 0;
        }

        let total_tokens: u32 = messages
            .iter()
            .map(|(_, content)| Self::estimate_tokens(content))
            .sum();

        if total_tokens <= self.config.trigger_threshold_tokens {
            return 0;
        }

        // Walk from the front, accumulating tokens to remove
        let target_removal = total_tokens.saturating_sub(self.config.target_after_compact_tokens);
        let mut accumulated = 0u32;
        let mut compact_count = 0;
        let min_keep = 2; // Always keep at least the last 2 messages

        for (_, content) in messages.iter().take(messages.len().saturating_sub(min_keep)) {
            accumulated += Self::estimate_tokens(content);
            compact_count += 1;
            if accumulated >= target_removal {
                break;
            }
        }

        compact_count
    }

    /// Build the prompt for the summarization model call.
    ///
    /// The prompt instructs the model to produce a concise summary that
    /// preserves key facts, decisions, and context needed for the
    /// conversation to continue coherently.
    pub fn build_compaction_prompt(messages: &[(String, String)]) -> String {
        let mut conversation = String::new();
        for (role, content) in messages {
            conversation.push_str(&format!("[{}]: {}\n\n", role, content));
        }

        format!(
            r#"Summarize the following conversation excerpt into a concise paragraph.
Preserve:
- Key facts, names, paths, and numbers mentioned
- Decisions made and their rationale
- Any pending tasks or open questions
- Technical context (commands run, errors seen, files discussed)

Keep the summary under 200 words. Write in third person ("The user asked...", "The assistant found...").

---
{}
---

Summary:"#,
            conversation
        )
    }

    /// Perform compaction using the provided generate function.
    ///
    /// The `generate_fn` is an async function that takes a prompt string
    /// and returns the model's response. This abstraction lets us avoid
    /// depending on a specific provider implementation.
    ///
    /// Returns `None` if the strategy is `Drop` or `Manual`, or if
    /// summarization fails.
    pub async fn compact<F, Fut>(
        &self,
        messages: &[(String, String)],
        generate_fn: F,
    ) -> Option<CompactionResult>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<String, anyhow::Error>>,
    {
        match self.config.strategy {
            CompactionStrategy::Drop => {
                // Drop strategy: no summary, just report what would be removed
                let count = self.messages_to_compact(messages);
                if count == 0 {
                    return None;
                }
                let original_tokens: u32 = messages[..count]
                    .iter()
                    .map(|(_, c)| Self::estimate_tokens(c))
                    .sum();
                Some(CompactionResult {
                    summary: String::new(),
                    summarized_count: count,
                    original_token_count: original_tokens,
                    summary_token_count: 0,
                })
            }
            CompactionStrategy::Summarize => {
                let count = self.messages_to_compact(messages);
                if count == 0 {
                    return None;
                }

                let to_compact = &messages[..count];
                let original_tokens: u32 = to_compact
                    .iter()
                    .map(|(_, c)| Self::estimate_tokens(c))
                    .sum();

                let prompt = Self::build_compaction_prompt(to_compact);

                match generate_fn(prompt).await {
                    Ok(summary) => {
                        let summary_tokens = Self::estimate_tokens(&summary);
                        Some(CompactionResult {
                            summary,
                            summarized_count: count,
                            original_token_count: original_tokens,
                            summary_token_count: summary_tokens,
                        })
                    }
                    Err(e) => {
                        tracing::warn!("Compaction summarization failed: {}", e);
                        None
                    }
                }
            }
            CompactionStrategy::Manual => {
                // Manual strategy: not implemented yet, would signal the UI
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create messages with known sizes
    fn make_messages(sizes: &[usize]) -> Vec<(String, String)> {
        sizes
            .iter()
            .enumerate()
            .map(|(i, size)| {
                let role = if i % 2 == 0 { "user" } else { "assistant" };
                let content = "x".repeat(*size);
                (role.to_string(), content)
            })
            .collect()
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(Compactor::estimate_tokens(""), 0);
        assert_eq!(Compactor::estimate_tokens("hello"), 2); // 5 chars => (5+3)/4 = 2
        assert_eq!(Compactor::estimate_tokens("a".repeat(100).as_str()), 25); // 100 chars => (100+3)/4 = 25
    }

    #[test]
    fn test_should_compact_below_threshold() {
        let compactor = Compactor::with_defaults();
        // Small conversation — should not compact
        let messages = make_messages(&[100, 100, 100]);
        assert!(!compactor.should_compact(&messages));
    }

    #[test]
    fn test_should_compact_too_few_messages() {
        let config = CompactionConfig {
            trigger_threshold_tokens: 10,
            min_messages_to_compact: 6,
            ..Default::default()
        };
        let compactor = Compactor::new(config);
        // Only 3 messages — below min_messages_to_compact
        let messages = make_messages(&[100, 100, 100]);
        assert!(!compactor.should_compact(&messages));
    }

    #[test]
    fn test_should_compact_above_threshold() {
        let config = CompactionConfig {
            trigger_threshold_tokens: 100,
            min_messages_to_compact: 3,
            ..Default::default()
        };
        let compactor = Compactor::new(config);
        // 8 messages * 200 chars each = ~400 tokens, well above 100
        let messages = make_messages(&[200, 200, 200, 200, 200, 200, 200, 200]);
        assert!(compactor.should_compact(&messages));
    }

    #[test]
    fn test_messages_to_compact_leaves_minimum() {
        let config = CompactionConfig {
            trigger_threshold_tokens: 10, // Very low threshold
            target_after_compact_tokens: 5,
            min_messages_to_compact: 3,
            ..Default::default()
        };
        let compactor = Compactor::new(config);
        let messages = make_messages(&[100, 100, 100, 100]);
        let count = compactor.messages_to_compact(&messages);
        // Should leave at least 2 messages
        assert!(count <= messages.len() - 2);
    }

    #[test]
    fn test_messages_to_compact_zero_when_not_needed() {
        let compactor = Compactor::with_defaults();
        let messages = make_messages(&[10, 10, 10]);
        assert_eq!(compactor.messages_to_compact(&messages), 0);
    }

    #[test]
    fn test_build_compaction_prompt() {
        let messages = vec![
            ("user".to_string(), "Find my PDF files".to_string()),
            (
                "assistant".to_string(),
                "I found 3 PDF files in ~/Documents".to_string(),
            ),
        ];
        let prompt = Compactor::build_compaction_prompt(&messages);
        assert!(prompt.contains("Find my PDF files"));
        assert!(prompt.contains("3 PDF files"));
        assert!(prompt.contains("Summary:"));
    }

    #[tokio::test]
    async fn test_compact_with_drop_strategy() {
        let config = CompactionConfig {
            trigger_threshold_tokens: 10,
            target_after_compact_tokens: 5,
            strategy: CompactionStrategy::Drop,
            min_messages_to_compact: 3,
        };
        let compactor = Compactor::new(config);
        let messages = make_messages(&[100, 100, 100, 100, 100]);

        let result = compactor
            .compact(&messages, |_| async { Ok("mock summary".to_string()) })
            .await;

        let result = result.unwrap();
        assert!(result.summary.is_empty()); // Drop doesn't produce a summary
        assert!(result.summarized_count > 0);
        assert!(result.original_token_count > 0);
        assert_eq!(result.summary_token_count, 0);
    }

    #[tokio::test]
    async fn test_compact_with_summarize_strategy() {
        let config = CompactionConfig {
            trigger_threshold_tokens: 10,
            target_after_compact_tokens: 5,
            strategy: CompactionStrategy::Summarize,
            min_messages_to_compact: 3,
        };
        let compactor = Compactor::new(config);
        let messages = make_messages(&[100, 100, 100, 100, 100]);

        let result = compactor
            .compact(&messages, |_prompt| async {
                Ok("The user searched for files and found several results.".to_string())
            })
            .await;

        let result = result.unwrap();
        assert!(result.summary.contains("user searched"));
        assert!(result.summarized_count > 0);
        assert!(result.summary_token_count > 0);
    }

    #[tokio::test]
    async fn test_compact_handles_model_failure() {
        let config = CompactionConfig {
            trigger_threshold_tokens: 10,
            target_after_compact_tokens: 5,
            strategy: CompactionStrategy::Summarize,
            min_messages_to_compact: 3,
        };
        let compactor = Compactor::new(config);
        let messages = make_messages(&[100, 100, 100, 100, 100]);

        let result = compactor
            .compact(&messages, |_prompt| async {
                Err(anyhow::anyhow!("Model unavailable"))
            })
            .await;

        // Should return None on failure, not panic
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_compact_manual_returns_none() {
        let config = CompactionConfig {
            trigger_threshold_tokens: 10,
            target_after_compact_tokens: 5,
            strategy: CompactionStrategy::Manual,
            min_messages_to_compact: 3,
        };
        let compactor = Compactor::new(config);
        let messages = make_messages(&[100, 100, 100, 100, 100]);

        let result = compactor
            .compact(&messages, |_| async { Ok("ignored".to_string()) })
            .await;

        assert!(result.is_none());
    }
}
