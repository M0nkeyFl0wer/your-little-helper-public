//! Token Usage Tracking Module
//!
//! Tracks API token usage and costs across conversations,
//! provides budget alerts, and suggests cost optimizations.
//!
//! Features:
//! - Real-time token counting per message
//! - Cost estimation in USD
//! - Daily/weekly/monthly budget tracking
//! - Smart model switching suggestions
//! - Usage history and trends
//! - Team budget management (for pre-loaded keys)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Token usage for a single message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Timestamp of the message
    pub timestamp: DateTime<Utc>,
    /// Input tokens (prompt)
    pub input_tokens: u32,
    /// Output tokens (completion)
    pub output_tokens: u32,
    /// Model used
    pub model: String,
    /// Provider (OpenAI, Anthropic, etc.)
    pub provider: String,
    /// Cost in USD
    pub cost_usd: f64,
    /// Conversation ID
    pub conversation_id: String,
}

impl TokenUsage {
    /// Total tokens for this message
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

/// Pricing per 1K tokens for different models
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub model: String,
    pub provider: String,
    /// Input cost per 1K tokens
    pub input_cost_per_1k: f64,
    /// Output cost per 1K tokens
    pub output_cost_per_1k: f64,
}

impl ModelPricing {
    /// Calculate cost for token usage
    pub fn calculate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        let input_cost = (input_tokens as f64 / 1000.0) * self.input_cost_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * self.output_cost_per_1k;
        input_cost + output_cost
    }
}

/// Known model pricings (as of early 2024)
pub fn get_model_pricing(model: &str) -> ModelPricing {
    let model_lower = model.to_lowercase();

    // OpenAI models
    if model_lower.contains("gpt-4") && model_lower.contains("32k") {
        ModelPricing {
            model: model.to_string(),
            provider: "OpenAI".to_string(),
            input_cost_per_1k: 0.06,
            output_cost_per_1k: 0.12,
        }
    } else if model_lower.contains("gpt-4") {
        ModelPricing {
            model: model.to_string(),
            provider: "OpenAI".to_string(),
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
        }
    } else if model_lower.contains("gpt-3.5") && model_lower.contains("16k") {
        ModelPricing {
            model: model.to_string(),
            provider: "OpenAI".to_string(),
            input_cost_per_1k: 0.003,
            output_cost_per_1k: 0.004,
        }
    } else if model_lower.contains("gpt-3.5") || model_lower.contains("gpt-3") {
        ModelPricing {
            model: model.to_string(),
            provider: "OpenAI".to_string(),
            input_cost_per_1k: 0.0015,
            output_cost_per_1k: 0.002,
        }
    }
    // Anthropic models
    else if model_lower.contains("claude-3") && model_lower.contains("opus") {
        ModelPricing {
            model: model.to_string(),
            provider: "Anthropic".to_string(),
            input_cost_per_1k: 0.015,
            output_cost_per_1k: 0.075,
        }
    } else if model_lower.contains("claude-3") && model_lower.contains("sonnet") {
        ModelPricing {
            model: model.to_string(),
            provider: "Anthropic".to_string(),
            input_cost_per_1k: 0.003,
            output_cost_per_1k: 0.015,
        }
    } else if model_lower.contains("claude-3") && model_lower.contains("haiku") {
        ModelPricing {
            model: model.to_string(),
            provider: "Anthropic".to_string(),
            input_cost_per_1k: 0.00025,
            output_cost_per_1k: 0.00125,
        }
    } else if model_lower.contains("claude") {
        ModelPricing {
            model: model.to_string(),
            provider: "Anthropic".to_string(),
            input_cost_per_1k: 0.008,
            output_cost_per_1k: 0.024,
        }
    }
    // Default fallback
    else {
        ModelPricing {
            model: model.to_string(),
            provider: "Unknown".to_string(),
            input_cost_per_1k: 0.002,
            output_cost_per_1k: 0.002,
        }
    }
}

/// Usage statistics for a time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    /// Total tokens used
    pub total_tokens: u64,
    /// Total input tokens
    pub input_tokens: u64,
    /// Total output tokens
    pub output_tokens: u64,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Number of messages
    pub message_count: u32,
    /// Breakdown by model
    pub by_model: HashMap<String, u64>, // model -> token count
    /// Breakdown by conversation
    pub by_conversation: HashMap<String, (u64, f64)>, // conversation_id -> (tokens, cost)
}

impl UsageStats {
    pub fn new() -> Self {
        Self {
            total_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_cost_usd: 0.0,
            message_count: 0,
            by_model: HashMap::new(),
            by_conversation: HashMap::new(),
        }
    }

    /// Add token usage to stats
    pub fn add_usage(&mut self, usage: &TokenUsage) {
        let tokens = usage.total_tokens() as u64;
        self.total_tokens += tokens;
        self.input_tokens += usage.input_tokens as u64;
        self.output_tokens += usage.output_tokens as u64;
        self.total_cost_usd += usage.cost_usd;
        self.message_count += 1;

        // Track by model
        *self.by_model.entry(usage.model.clone()).or_insert(0) += tokens;

        // Track by conversation
        let entry = self
            .by_conversation
            .entry(usage.conversation_id.clone())
            .or_insert((0, 0.0));
        entry.0 += tokens;
        entry.1 += usage.cost_usd;
    }

    /// Average cost per message
    pub fn avg_cost_per_message(&self) -> f64 {
        if self.message_count == 0 {
            0.0
        } else {
            self.total_cost_usd / self.message_count as f64
        }
    }

    /// Average tokens per message
    pub fn avg_tokens_per_message(&self) -> f64 {
        if self.message_count == 0 {
            0.0
        } else {
            self.total_tokens as f64 / self.message_count as f64
        }
    }
}

impl Default for UsageStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Budget settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSettings {
    /// Daily budget in USD
    pub daily_budget_usd: Option<f64>,
    /// Weekly budget in USD
    pub weekly_budget_usd: Option<f64>,
    /// Monthly budget in USD
    pub monthly_budget_usd: Option<f64>,
    /// Warn when reaching X% of budget
    pub warning_threshold_percent: f64,
    /// Auto-switch to cheaper model when expensive model exceeds threshold
    pub auto_switch_threshold_usd: Option<f64>,
}

impl Default for BudgetSettings {
    fn default() -> Self {
        Self {
            daily_budget_usd: None,
            weekly_budget_usd: Some(10.0),  // $10/week default
            monthly_budget_usd: Some(50.0), // $50/month default
            warning_threshold_percent: 80.0,
            auto_switch_threshold_usd: Some(5.0), // Switch after $5 in one conversation
        }
    }
}

/// Alert type for budget warnings
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetAlert {
    /// Approaching daily budget
    DailyWarning { used: f64, budget: f64 },
    /// Approaching weekly budget
    WeeklyWarning { used: f64, budget: f64 },
    /// Approaching monthly budget
    MonthlyWarning { used: f64, budget: f64 },
    /// Exceeded auto-switch threshold for expensive model
    AutoSwitchSuggestion {
        current_model: String,
        suggested_model: String,
        current_cost: f64,
    },
}

/// Cost optimization suggestion
#[derive(Debug, Clone)]
pub struct CostOptimization {
    /// Current expensive model
    pub current_model: String,
    /// Suggested cheaper alternative
    pub suggested_model: String,
    /// Potential savings per 1K tokens
    pub savings_per_1k: f64,
    /// Quality comparison
    pub quality_note: String,
    /// When to use each model
    pub recommendation: String,
}

/// Token usage tracker
pub struct TokenTracker {
    /// All recorded usage
    usage_history: Arc<Mutex<Vec<TokenUsage>>>,
    /// Budget settings
    budget_settings: Arc<Mutex<BudgetSettings>>,
    /// Current conversation ID
    current_conversation: Arc<Mutex<Option<String>>>,
    /// Last budget check time
    last_budget_check: Arc<Mutex<Instant>>,
}

impl TokenTracker {
    /// Create a new token tracker
    pub fn new() -> Self {
        Self {
            usage_history: Arc::new(Mutex::new(Vec::new())),
            budget_settings: Arc::new(Mutex::new(BudgetSettings::default())),
            current_conversation: Arc::new(Mutex::new(None)),
            last_budget_check: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Start tracking a new conversation
    pub fn start_conversation(&self, conversation_id: String) {
        let mut current = self.current_conversation.lock().unwrap();
        *current = Some(conversation_id);
    }

    /// End current conversation
    pub fn end_conversation(&self) {
        let mut current = self.current_conversation.lock().unwrap();
        *current = None;
    }

    /// Record token usage from a message
    pub fn record_usage(&self, input_tokens: u32, output_tokens: u32, model: &str) {
        let pricing = get_model_pricing(model);
        let cost = pricing.calculate_cost(input_tokens, output_tokens);

        let conversation_id = self
            .current_conversation
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let usage = TokenUsage {
            timestamp: Utc::now(),
            input_tokens,
            output_tokens,
            model: model.to_string(),
            provider: pricing.provider,
            cost_usd: cost,
            conversation_id,
        };

        self.usage_history.lock().unwrap().push(usage);

        // Check if we should trigger budget alerts
        self.check_budget_alerts();
    }

    /// Get usage stats for today
    pub fn get_daily_stats(&self) -> UsageStats {
        let today = Utc::now().date_naive();
        let history = self.usage_history.lock().unwrap();

        let mut stats = UsageStats::new();
        for usage in history.iter() {
            if usage.timestamp.date_naive() == today {
                stats.add_usage(usage);
            }
        }
        stats
    }

    /// Get usage stats for current week
    pub fn get_weekly_stats(&self) -> UsageStats {
        let now = Utc::now();
        let week_ago = now - Duration::from_secs(7 * 24 * 60 * 60);
        let history = self.usage_history.lock().unwrap();

        let mut stats = UsageStats::new();
        for usage in history.iter() {
            if usage.timestamp >= week_ago {
                stats.add_usage(usage);
            }
        }
        stats
    }

    /// Get usage stats for current month
    pub fn get_monthly_stats(&self) -> UsageStats {
        let now = Utc::now();
        let month_ago = now - Duration::from_secs(30 * 24 * 60 * 60);
        let history = self.usage_history.lock().unwrap();

        let mut stats = UsageStats::new();
        for usage in history.iter() {
            if usage.timestamp >= month_ago {
                stats.add_usage(usage);
            }
        }
        stats
    }

    /// Get current conversation cost
    pub fn get_conversation_cost(&self) -> f64 {
        let current_id = match self.current_conversation.lock().unwrap().clone() {
            Some(id) => id,
            None => return 0.0,
        };

        let history = self.usage_history.lock().unwrap();
        history
            .iter()
            .filter(|u| u.conversation_id == current_id)
            .map(|u| u.cost_usd)
            .sum()
    }

    /// Check budget and return any alerts
    fn check_budget_alerts(&self) -> Vec<BudgetAlert> {
        let mut alerts = Vec::new();
        let settings = self.budget_settings.lock().unwrap();

        // Only check every 5 minutes to avoid spam
        let mut last_check = self.last_budget_check.lock().unwrap();
        if last_check.elapsed() < Duration::from_secs(300) {
            return alerts;
        }
        *last_check = Instant::now();
        drop(last_check);

        // Check daily budget
        if let Some(daily_budget) = settings.daily_budget_usd {
            let daily_stats = self.get_daily_stats();
            let percent_used = (daily_stats.total_cost_usd / daily_budget) * 100.0;

            if percent_used >= settings.warning_threshold_percent {
                alerts.push(BudgetAlert::DailyWarning {
                    used: daily_stats.total_cost_usd,
                    budget: daily_budget,
                });
            }
        }

        // Check weekly budget
        if let Some(weekly_budget) = settings.weekly_budget_usd {
            let weekly_stats = self.get_weekly_stats();
            let percent_used = (weekly_stats.total_cost_usd / weekly_budget) * 100.0;

            if percent_used >= settings.warning_threshold_percent {
                alerts.push(BudgetAlert::WeeklyWarning {
                    used: weekly_stats.total_cost_usd,
                    budget: weekly_budget,
                });
            }
        }

        // Check monthly budget
        if let Some(monthly_budget) = settings.monthly_budget_usd {
            let monthly_stats = self.get_monthly_stats();
            let percent_used = (monthly_stats.total_cost_usd / monthly_budget) * 100.0;

            if percent_used >= settings.warning_threshold_percent {
                alerts.push(BudgetAlert::MonthlyWarning {
                    used: monthly_stats.total_cost_usd,
                    budget: monthly_budget,
                });
            }
        }

        // Check auto-switch threshold
        if let Some(threshold) = settings.auto_switch_threshold_usd {
            let conversation_cost = self.get_conversation_cost();
            if conversation_cost >= threshold {
                // Get most expensive model in current conversation
                let history = self.usage_history.lock().unwrap();
                let current_id = self.current_conversation.lock().unwrap().clone();

                if let Some(id) = current_id {
                    let expensive_model = history
                        .iter()
                        .filter(|u| u.conversation_id == id)
                        .max_by(|a, b| {
                            let a_price = get_model_pricing(&a.model);
                            let b_price = get_model_pricing(&b.model);
                            a_price
                                .input_cost_per_1k
                                .partial_cmp(&b_price.input_cost_per_1k)
                                .unwrap()
                        })
                        .map(|u| u.model.clone());

                    if let Some(model) = expensive_model {
                        let suggestion = self.get_cheaper_alternative(&model);
                        alerts.push(BudgetAlert::AutoSwitchSuggestion {
                            current_model: model,
                            suggested_model: suggestion.suggested_model,
                            current_cost: conversation_cost,
                        });
                    }
                }
            }
        }

        alerts
    }

    /// Get a cheaper alternative model
    fn get_cheaper_alternative(&self, current_model: &str) -> CostOptimization {
        let _current = get_model_pricing(current_model);

        // Define cheaper alternatives
        let alternatives: Vec<(&str, &str, f64)> = vec![
            ("gpt-4", "gpt-3.5-turbo", 0.9),            // 90% cheaper
            ("claude-3-opus", "claude-3-sonnet", 0.8),  // 80% cheaper
            ("claude-3-sonnet", "claude-3-haiku", 0.9), // 90% cheaper
            ("gpt-4-32k", "gpt-3.5-turbo-16k", 0.95),   // 95% cheaper
        ];

        for (expensive, cheaper, savings) in alternatives {
            if current_model.to_lowercase().contains(expensive) {
                let _cheaper_pricing = get_model_pricing(cheaper);
                return CostOptimization {
                    current_model: current_model.to_string(),
                    suggested_model: cheaper.to_string(),
                    savings_per_1k: savings,
                    quality_note: "Slightly less capable but much faster and cheaper".to_string(),
                    recommendation: format!(
                        "Switch to {} to save {:.0}% on costs while maintaining good quality",
                        cheaper,
                        savings * 100.0
                    ),
                };
            }
        }

        // Default suggestion
        CostOptimization {
            current_model: current_model.to_string(),
            suggested_model: "gpt-3.5-turbo".to_string(),
            savings_per_1k: 0.9,
            quality_note: "Good for most tasks at a fraction of the cost".to_string(),
            recommendation: "Consider using gpt-3.5-turbo for routine tasks".to_string(),
        }
    }

    /// Update budget settings
    pub fn update_budget_settings(&self, settings: BudgetSettings) {
        *self.budget_settings.lock().unwrap() = settings;
    }

    /// Get current budget settings
    pub fn get_budget_settings(&self) -> BudgetSettings {
        self.budget_settings.lock().unwrap().clone()
    }

    /// Format usage report
    pub fn format_usage_report(&self) -> String {
        let daily = self.get_daily_stats();
        let weekly = self.get_weekly_stats();
        let monthly = self.get_monthly_stats();
        let settings = self.get_budget_settings();

        let mut output = String::new();
        output.push_str("## 💰 Token Usage Report\n\n");

        // Today
        output.push_str("### Today\n");
        output.push_str(&format!("• Tokens: {}\n", daily.total_tokens));
        output.push_str(&format!("• Cost: ${:.4}\n", daily.total_cost_usd));
        output.push_str(&format!("• Messages: {}\n", daily.message_count));
        if let Some(budget) = settings.daily_budget_usd {
            let percent = (daily.total_cost_usd / budget) * 100.0;
            output.push_str(&format!("• Budget: ${:.2} ({:.1}%)\n", budget, percent));
        }
        output.push('\n');

        // This week
        output.push_str("### This Week\n");
        output.push_str(&format!("• Tokens: {}\n", weekly.total_tokens));
        output.push_str(&format!("• Cost: ${:.4}\n", weekly.total_cost_usd));
        output.push_str(&format!("• Messages: {}\n", weekly.message_count));
        if let Some(budget) = settings.weekly_budget_usd {
            let percent = (weekly.total_cost_usd / budget) * 100.0;
            output.push_str(&format!("• Budget: ${:.2} ({:.1}%)\n", budget, percent));
        }
        output.push('\n');

        // This month
        output.push_str("### This Month\n");
        output.push_str(&format!("• Tokens: {}\n", monthly.total_tokens));
        output.push_str(&format!("• Cost: ${:.4}\n", monthly.total_cost_usd));
        output.push_str(&format!(
            "• Avg per message: ${:.4}\n",
            monthly.avg_cost_per_message()
        ));
        if let Some(budget) = settings.monthly_budget_usd {
            let percent = (monthly.total_cost_usd / budget) * 100.0;
            output.push_str(&format!("• Budget: ${:.2} ({:.1}%)\n", budget, percent));
        }

        // Model breakdown
        if !monthly.by_model.is_empty() {
            output.push_str("\n### By Model\n");
            for (model, tokens) in &monthly.by_model {
                output.push_str(&format!("• {}: {} tokens\n", model, tokens));
            }
        }

        // Tips
        if monthly.total_cost_usd > 5.0 {
            output.push_str("\n💡 **Tip:** Your usage is increasing. Consider:\n");
            output.push_str("• Using gpt-3.5-turbo for routine tasks (90% cheaper)\n");
            output.push_str("• Switching to local AI for simple queries\n");
        }

        output
    }

    /// Save usage history to disk
    pub fn save_history(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let history = self.usage_history.lock().unwrap();
        let json = serde_json::to_string_pretty(&*history)?;
        std::fs::write(path, json)
    }

    /// Load usage history from disk
    pub fn load_history(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let json = std::fs::read_to_string(path)?;
        let history: Vec<TokenUsage> = serde_json::from_str(&json)?;
        *self.usage_history.lock().unwrap() = history;
        Ok(())
    }
}

impl Default for TokenTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage_calculation() {
        let usage = TokenUsage {
            timestamp: Utc::now(),
            input_tokens: 100,
            output_tokens: 50,
            model: "gpt-3.5-turbo".to_string(),
            provider: "OpenAI".to_string(),
            cost_usd: 0.001,
            conversation_id: "test".to_string(),
        };

        assert_eq!(usage.total_tokens(), 150);
    }

    #[test]
    fn test_model_pricing() {
        let pricing = get_model_pricing("gpt-4");
        assert_eq!(pricing.provider, "OpenAI");
        assert!(pricing.input_cost_per_1k > 0.0);

        let cost = pricing.calculate_cost(1000, 500);
        assert!(cost > 0.0);
    }

    #[test]
    fn test_usage_stats() {
        let mut stats = UsageStats::new();

        let usage = TokenUsage {
            timestamp: Utc::now(),
            input_tokens: 100,
            output_tokens: 50,
            model: "gpt-3.5-turbo".to_string(),
            provider: "OpenAI".to_string(),
            cost_usd: 0.001,
            conversation_id: "test".to_string(),
        };

        stats.add_usage(&usage);
        assert_eq!(stats.total_tokens, 150);
        assert_eq!(stats.message_count, 1);
        assert_eq!(stats.total_cost_usd, 0.001);
    }
}
