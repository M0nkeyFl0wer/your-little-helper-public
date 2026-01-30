//! Context Token Manager
//!
//! Manages token budgets and context window constraints for AI conversations.
//! Prevents context bloat and helps users understand token costs.
//!
//! Strategy:
//! - Monitor cumulative token usage per conversation
//! - Warn when approaching limits
//! - Suggest context pruning strategies
//! - Track which documents are actually being referenced
//! - Auto-summarize old context to save tokens

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Token budget settings per provider
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Maximum context window (varies by model)
    pub max_context_tokens: usize,
    /// Target usage (leave headroom for responses)
    pub target_usage_percent: f64,
    /// Warning threshold
    pub warning_threshold_percent: f64,
    /// Auto-prune when exceeding
    pub auto_prune_threshold_percent: f64,
}

impl TokenBudget {
    /// Conservative budget for GPT-4 (8k context)
    pub fn gpt4_8k() -> Self {
        Self {
            max_context_tokens: 8192,
            target_usage_percent: 0.7,         // Use 70% for context
            warning_threshold_percent: 0.8,    // Warn at 80%
            auto_prune_threshold_percent: 0.9, // Prune at 90%
        }
    }

    /// Budget for GPT-3.5 (16k context)
    pub fn gpt35_16k() -> Self {
        Self {
            max_context_tokens: 16384,
            target_usage_percent: 0.75,
            warning_threshold_percent: 0.85,
            auto_prune_threshold_percent: 0.95,
        }
    }

    /// Budget for Claude (200k context - very generous)
    pub fn claude() -> Self {
        Self {
            max_context_tokens: 200000,
            target_usage_percent: 0.5, // Can use more but keep it reasonable
            warning_threshold_percent: 0.7,
            auto_prune_threshold_percent: 0.8,
        }
    }

    /// Calculate actual token limits
    pub fn target_tokens(&self) -> usize {
        (self.max_context_tokens as f64 * self.target_usage_percent) as usize
    }

    pub fn warning_tokens(&self) -> usize {
        (self.max_context_tokens as f64 * self.warning_threshold_percent) as usize
    }

    pub fn prune_tokens(&self) -> usize {
        (self.max_context_tokens as f64 * self.auto_prune_threshold_percent) as usize
    }
}

/// Tracks token usage for context documents
#[derive(Debug, Clone)]
pub struct ContextUsageTracker {
    /// Budget settings
    budget: TokenBudget,
    /// Current context documents loaded
    loaded_documents: HashMap<String, DocumentUsage>,
    /// Total tokens used
    total_tokens: usize,
    /// Last access times for LRU pruning
    last_accessed: HashMap<String, Instant>,
    /// Access counts for importance scoring
    access_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct DocumentUsage {
    pub document_id: String,
    pub token_count: usize,
    pub times_referenced: usize,
    pub last_referenced: Instant,
}

/// Suggestion for token optimization
#[derive(Debug, Clone)]
pub struct TokenOptimizationSuggestion {
    pub suggestion_type: OptimizationType,
    pub description: String,
    pub potential_savings: usize,
    pub priority: Priority,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationType {
    RemoveUnused,     // Documents never accessed
    SummarizeOld,     // Summarize old conversation context
    SwitchModel,      // Use model with larger context
    SelectiveLoading, // Load only relevant sections
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl ContextUsageTracker {
    pub fn new(budget: TokenBudget) -> Self {
        Self {
            budget,
            loaded_documents: HashMap::new(),
            total_tokens: 0,
            last_accessed: HashMap::new(),
            access_counts: HashMap::new(),
        }
    }

    /// Add a document to context
    pub fn load_document(&mut self, doc_id: String, token_count: usize) {
        self.loaded_documents.insert(
            doc_id.clone(),
            DocumentUsage {
                document_id: doc_id.clone(),
                token_count,
                times_referenced: 0,
                last_referenced: Instant::now(),
            },
        );
        self.total_tokens += token_count;
        self.last_accessed.insert(doc_id, Instant::now());
    }

    /// Mark document as accessed
    pub fn access_document(&mut self, doc_id: &str) {
        self.last_accessed
            .insert(doc_id.to_string(), Instant::now());
        *self.access_counts.entry(doc_id.to_string()).or_insert(0) += 1;

        if let Some(usage) = self.loaded_documents.get_mut(doc_id) {
            usage.times_referenced += 1;
            usage.last_referenced = Instant::now();
        }
    }

    /// Remove a document from context
    pub fn unload_document(&mut self, doc_id: &str) -> Option<DocumentUsage> {
        if let Some(usage) = self.loaded_documents.remove(doc_id) {
            self.total_tokens -= usage.token_count;
            self.last_accessed.remove(doc_id);
            Some(usage)
        } else {
            None
        }
    }

    /// Check current status and return warnings if needed
    pub fn check_status(&self) -> TokenStatus {
        let usage_percent = self.total_tokens as f64 / self.budget.max_context_tokens as f64;

        if usage_percent >= self.budget.auto_prune_threshold_percent {
            TokenStatus::Critical(self.generate_optimization_suggestions())
        } else if usage_percent >= self.budget.warning_threshold_percent {
            TokenStatus::Warning(self.generate_optimization_suggestions())
        } else if usage_percent >= self.budget.target_usage_percent {
            TokenStatus::ApproachingLimit
        } else {
            TokenStatus::Healthy
        }
    }

    /// Generate optimization suggestions based on usage patterns
    fn generate_optimization_suggestions(&self) -> Vec<TokenOptimizationSuggestion> {
        let mut suggestions = Vec::new();

        // Find documents never accessed in this conversation
        let unused: Vec<_> = self
            .loaded_documents
            .values()
            .filter(|doc| doc.times_referenced == 0)
            .collect();

        if !unused.is_empty() {
            let savings: usize = unused.iter().map(|d| d.token_count).sum();
            suggestions.push(TokenOptimizationSuggestion {
                suggestion_type: OptimizationType::RemoveUnused,
                description: format!(
                    "Remove {} unused documents (never referenced in this conversation)",
                    unused.len()
                ),
                potential_savings: savings,
                priority: Priority::High,
            });
        }

        // Find old documents (not accessed recently)
        let old_threshold = Duration::from_secs(600); // 10 minutes
        let old_docs: Vec<_> = self
            .loaded_documents
            .values()
            .filter(|doc| doc.last_referenced.elapsed() > old_threshold && doc.times_referenced > 0)
            .collect();

        if !old_docs.is_empty() {
            let savings: usize = old_docs.iter().map(|d| d.token_count).sum::<usize>() / 2; // Summarize saves ~50%
            suggestions.push(TokenOptimizationSuggestion {
                suggestion_type: OptimizationType::SummarizeOld,
                description: format!(
                    "Summarize {} older documents (last referenced >10 min ago)",
                    old_docs.len()
                ),
                potential_savings: savings,
                priority: Priority::Medium,
            });
        }

        // Suggest model upgrade if at limit
        if self.total_tokens > self.budget.warning_tokens() {
            suggestions.push(TokenOptimizationSuggestion {
                suggestion_type: OptimizationType::SwitchModel,
                description:
                    "Switch to Claude (200k context) or GPT-4-32k for larger context window"
                        .to_string(),
                potential_savings: 0, // Not a savings, but enables more context
                priority: Priority::Medium,
            });
        }

        suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));
        suggestions
    }

    /// Auto-prune least important documents to free up tokens
    pub fn auto_prune(&mut self) -> Vec<String> {
        let mut pruned = Vec::new();
        let target = self.budget.target_tokens();

        while self.total_tokens > target && !self.loaded_documents.is_empty() {
            // Find least valuable document (LRU + low access count)
            if let Some((doc_id, _)) = self.loaded_documents.iter().min_by(|(_, a), (_, b)| {
                let a_score = a.times_referenced
                    + if a.last_referenced.elapsed() > Duration::from_secs(300) {
                        0
                    } else {
                        1
                    };
                let b_score = b.times_referenced
                    + if b.last_referenced.elapsed() > Duration::from_secs(300) {
                        0
                    } else {
                        1
                    };
                a_score.cmp(&b_score)
            }) {
                let doc_id = doc_id.clone();
                pruned.push(doc_id.clone());
                self.unload_document(&doc_id);
            } else {
                break;
            }
        }

        pruned
    }

    /// Format a user-friendly token usage report
    pub fn format_usage_report(&self) -> String {
        let mut report = String::new();
        let usage_percent =
            (self.total_tokens as f64 / self.budget.max_context_tokens as f64 * 100.0) as usize;

        report.push_str(&format!("## 📊 Token Usage\n\n"));
        report.push_str(&format!(
            "**{}/{} tokens** ({}%)\n\n",
            self.total_tokens, self.budget.max_context_tokens, usage_percent
        ));

        // Visual bar
        let filled = (usage_percent / 5).min(20);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(20 - filled));
        report.push_str(&format!("[{bar}]\n\n"));

        // Status
        match self.check_status() {
            TokenStatus::Healthy => report.push_str("✅ **Status**: Healthy\n"),
            TokenStatus::ApproachingLimit => report.push_str("⚠️  **Status**: Approaching limit\n"),
            TokenStatus::Warning(_) => {
                report.push_str("🟡 **Status**: High usage - consider optimizations\n")
            }
            TokenStatus::Critical(_) => {
                report.push_str("🔴 **Status**: Critical - pruning recommended\n")
            }
        }

        report.push('\n');

        // Document breakdown
        if !self.loaded_documents.is_empty() {
            report.push_str("### Loaded Documents\n\n");

            let mut docs: Vec<_> = self.loaded_documents.values().collect();
            docs.sort_by(|a, b| b.token_count.cmp(&a.token_count));

            for doc in docs.iter().take(10) {
                let status = if doc.times_referenced > 0 {
                    "✓"
                } else {
                    "○"
                };
                report.push_str(&format!(
                    "{} **{}** - {} tokens (referenced {}x)\n",
                    status, doc.document_id, doc.token_count, doc.times_referenced
                ));
            }

            if docs.len() > 10 {
                report.push_str(&format!("\n... and {} more\n", docs.len() - 10));
            }
        }

        report
    }

    /// Estimate tokens for text (rough approximation: 4 chars ≈ 1 token)
    pub fn estimate_tokens(text: &str) -> usize {
        text.len() / 4
    }
}

#[derive(Debug, Clone)]
pub enum TokenStatus {
    Healthy,
    ApproachingLimit,
    Warning(Vec<TokenOptimizationSuggestion>),
    Critical(Vec<TokenOptimizationSuggestion>),
}

/// User-facing context management UI helpers
pub struct ContextUIHelper;

impl ContextUIHelper {
    /// Generate onboarding message for new users about context
    pub fn onboarding_message() -> String {
        r#"## 🧠 About Context Documents

**What are they?**
Context documents are files you can add that help me understand your specific situation, research, templates, or personas. Think of them as my long-term memory.

**Why use them?**
- Reference your research without re-explaining
- Save prompts you use frequently
- Create personas for different types of content
- Store templates for quick reuse

**Token Budget**
I have a limited "attention span" (token window). Large documents use more tokens. I'll help you manage this by:
- Showing you how much budget you're using
- Suggesting which documents to keep or remove
- Summarizing older context automatically

**Getting Started**
I've pre-loaded some example documents for you:
- Research on Great Bear Sea MPA Network
- Tech-savvy user persona
- Weekly status template
- File organization guide

**Try asking:**
- "What context documents do I have?"
- "Search my research for [topic]"
- "Keep this file in my context"

💡 **Tip**: After I reference a document, I'll ask if you want to keep it loaded for future questions or remove it to save tokens.
"#.to_string()
    }

    /// Message after using a file
    pub fn post_usage_prompt(document_name: &str, token_cost: usize) -> String {
        format!(
            r#"📄 **Used: {}** ({} tokens)

Would you like to:
1. **Keep loaded** - I'll remember this for follow-up questions
2. **Remove** - Free up tokens for other documents
3. **Add to permanent context** - Always include this for [mode] mode

💡 Documents I reference frequently will be prioritized in your token budget."#,
            document_name, token_cost
        )
    }

    /// Warning about token limits
    pub fn token_warning(usage_percent: usize) -> String {
        format!(
            r#"⚠️  **Context Window {}% Full**

I'm getting close to my token limit. To ensure I can continue helping effectively:

- Consider removing documents you don't need right now
- I can summarize older conversation context
- Switching to Claude provides a much larger context window (200k vs 8k tokens)

**Would you like me to:**
1. Auto-prune unused documents
2. Show you which documents use the most tokens
3. Continue (I may start forgetting earlier parts of our conversation)"#,
            usage_percent
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimation() {
        let text = "Hello world, this is a test.";
        let tokens = ContextUsageTracker::estimate_tokens(text);
        assert!(tokens > 0);
        assert!(tokens < text.len()); // Should be less than character count
    }

    #[test]
    fn test_budget_calculations() {
        let budget = TokenBudget::gpt4_8k();
        assert_eq!(budget.target_tokens(), 5734); // 8192 * 0.7
        assert_eq!(budget.warning_tokens(), 6553); // 8192 * 0.8
    }

    #[test]
    fn test_load_and_access() {
        let mut tracker = ContextUsageTracker::new(TokenBudget::gpt4_8k());

        tracker.load_document("doc1".to_string(), 1000);
        assert_eq!(tracker.total_tokens, 1000);

        tracker.access_document("doc1");
        let usage = tracker.loaded_documents.get("doc1").unwrap();
        assert_eq!(usage.times_referenced, 1);
    }

    #[test]
    fn test_auto_prune() {
        let mut tracker = ContextUsageTracker::new(TokenBudget::gpt4_8k());

        // Load some docs
        tracker.load_document("doc1".to_string(), 1000);
        tracker.load_document("doc2".to_string(), 2000);
        tracker.load_document("doc3".to_string(), 3000);

        // Access only doc1
        tracker.access_document("doc1");
        tracker.access_document("doc1");

        // Simulate time passing for doc2
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Prune should remove doc3 (never accessed) and doc2 (old, only accessed once)
        let pruned = tracker.auto_prune();
        assert!(!pruned.is_empty());
    }
}
