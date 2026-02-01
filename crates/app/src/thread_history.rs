//! Unified Thread History System
//!
//! Provides a master list of all conversations across all modes,
//! with auto-titling and fzf-style fuzzy search.
//!
//! Features:
//! - Single view of all threads from all modes (Fix, Research, Data, Content)
//! - Auto-generated titles based on conversation topic
//! - Fuzzy search (fzf-style) across titles and content
//! - Thread metadata (mode, timestamp, message count)
//! - Easy thread resurrection and continuation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Simplified message for storage (avoids serialization issues)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleMessage {
    pub role: String,
    pub content: String,
}

/// A thread represents a conversation across any mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    /// Unique thread ID
    pub id: String,
    /// Auto-generated title based on content
    pub title: String,
    /// Which mode this thread belongs to
    pub mode: crate::types::ChatMode,
    /// When thread started
    pub started_at: SystemTime,
    /// When last message was sent
    pub last_activity: SystemTime,
    /// Number of messages
    pub message_count: usize,
    /// Preview of last message
    pub last_message_preview: String,
    /// Full chat history (for resurrection) - stored as simple strings
    #[serde(skip)] // Don't serialize full messages to avoid type issues
    pub messages: Vec<SimpleMessage>,
    /// Whether thread is pinned/important
    pub is_pinned: bool,
    /// User can add custom tags
    pub tags: Vec<String>,
}

impl Thread {
    /// Create a new thread
    pub fn new(id: String, mode: crate::types::ChatMode, first_message: &str) -> Self {
        let now = SystemTime::now();
        Self {
            id,
            title: Self::generate_title(first_message),
            mode,
            started_at: now,
            last_activity: now,
            message_count: 1,
            last_message_preview: Self::truncate_preview(first_message),
            messages: Vec::new(),
            is_pinned: false,
            tags: Vec::new(),
        }
    }

    /// Auto-generate a title from the first message
    fn generate_title(message: &str) -> String {
        // Remove common prefixes
        let cleaned = message
            .trim_start_matches("Can you ")
            .trim_start_matches("Could you ")
            .trim_start_matches("Please ")
            .trim_start_matches("Help me ")
            .trim_start_matches("I need ")
            .trim_start_matches("How do I ")
            .trim_start_matches("What is ")
            .trim_start_matches("Find ")
            .trim_start_matches("Search for ")
            .trim_start_matches("Show me ")
            .trim_start_matches("Tell me ")
            .trim_start_matches("Explain ")
            .trim_start_matches("Analyze ")
            .trim_start_matches("Write ")
            .trim_start_matches("Create ")
            .trim_start_matches("Make ")
            .trim_start_matches("Fix ")
            .trim_start_matches("Check ")
            .trim_start_matches("Organize ")
            .trim_start_matches("Research ")
            .to_string();

        // Extract key nouns/names (simple approach)
        let words: Vec<&str> = cleaned.split_whitespace().collect();

        // Try to find a meaningful title (up to 6 words)
        let title_words: Vec<&str> = words
            .iter()
            .take(8)
            .filter(|w| !is_stop_word(w))
            .take(6)
            .copied()
            .collect();

        let title = if title_words.is_empty() {
            // Fallback to first 5 words
            words.iter().take(5).copied().collect::<Vec<_>>().join(" ")
        } else {
            title_words.join(" ")
        };

        // Capitalize first letter
        let mut chars = title.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str(),
        }
    }

    /// Update thread with new message
    pub fn add_message(&mut self, message: &str) {
        self.last_activity = SystemTime::now();
        self.message_count += 1;
        self.last_message_preview = Self::truncate_preview(message);

        // Update title if this is a significant message (e.g., from user, not too short)
        if message.len() > 20 && self.message_count <= 3 {
            let new_title = Self::generate_title(message);
            if new_title.len() > self.title.len() {
                self.title = new_title;
            }
        }
    }

    /// Truncate message for preview
    fn truncate_preview(message: &str) -> String {
        let max_len = 80;
        if message.len() <= max_len {
            message.to_string()
        } else {
            format!("{}...", &message[..max_len])
        }
    }

    /// Format for display in thread list
    pub fn format_for_list(&self) -> String {
        let mode_icon = match self.mode {
            crate::types::ChatMode::Find => "🔎",
            crate::types::ChatMode::Fix => "🔧",
            crate::types::ChatMode::Research => "🔬",
            crate::types::ChatMode::Data => "📊",
            crate::types::ChatMode::Content => "✍️",
            crate::types::ChatMode::Build => "🐶",
        };

        let time_str = format_time_ago(self.last_activity);
        let pinned = if self.is_pinned { "📌 " } else { "" };

        format!(
            "{}{} {} · {} · {} msg · {}",
            pinned, mode_icon, self.title, time_str, self.message_count, self.last_message_preview
        )
    }
}

/// Check if word is a stop word (to exclude from titles)
fn is_stop_word(word: &str) -> bool {
    let stop_words = [
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
        "from", "up", "about", "into", "through", "during", "before", "after", "above", "below",
        "between", "among", "is", "are", "was", "were", "be", "been", "being", "have", "has",
        "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "can",
        "this", "that", "these", "those", "i", "you", "he", "she", "it", "we", "they", "me", "him",
        "her", "us", "them", "my", "your", "his", "its", "our", "their", "just", "only", "also",
        "very", "really",
    ];
    stop_words.contains(&word.to_lowercase().as_str())
}

/// Format time as "2m ago", "3h ago", "2d ago"
fn format_time_ago(time: SystemTime) -> String {
    let now = SystemTime::now();
    let duration = now.duration_since(time).unwrap_or_default();
    let seconds = duration.as_secs();

    if seconds < 60 {
        "just now".to_string()
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h ago", seconds / 3600)
    } else if seconds < 604800 {
        format!("{}d ago", seconds / 86400)
    } else {
        let days = seconds / 86400;
        format!("{}w ago", days / 7)
    }
}

/// Thread history manager
#[derive(Debug, Serialize, Deserialize)]
pub struct ThreadHistory {
    /// All threads indexed by ID
    threads: HashMap<String, Thread>,
    /// Ordered list of thread IDs (most recent first)
    ordered_ids: Vec<String>,
    /// Pinned thread IDs
    pinned_ids: Vec<String>,
}

impl ThreadHistory {
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
            ordered_ids: Vec::new(),
            pinned_ids: Vec::new(),
        }
    }

    /// Create or update a thread
    pub fn upsert_thread(&mut self, thread: Thread) {
        let id = thread.id.clone();

        // Remove from ordered list if exists (to move to front)
        self.ordered_ids.retain(|tid| tid != &id);

        // Add to front
        self.ordered_ids.insert(0, id.clone());

        // Store thread
        self.threads.insert(id, thread);
    }

    /// Get thread by ID
    pub fn get_thread(&self, id: &str) -> Option<&Thread> {
        self.threads.get(id)
    }

    /// Get mutable thread
    pub fn get_thread_mut(&mut self, id: &str) -> Option<&mut Thread> {
        self.threads.get_mut(id)
    }

    /// Pin/unpin a thread
    pub fn toggle_pin(&mut self, id: &str) {
        if let Some(thread) = self.threads.get_mut(id) {
            thread.is_pinned = !thread.is_pinned;

            if thread.is_pinned {
                if !self.pinned_ids.contains(&id.to_string()) {
                    self.pinned_ids.push(id.to_string());
                }
            } else {
                self.pinned_ids.retain(|pid| pid != id);
            }
        }
    }

    /// Search threads with fzf-style fuzzy matching
    pub fn search(&self, query: &str) -> Vec<&Thread> {
        let query_lower = query.to_lowercase();
        let query_chars: Vec<char> = query_lower.chars().collect();

        let mut results: Vec<(&Thread, f64)> = self
            .threads
            .values()
            .filter_map(|thread| {
                let score = fuzzy_match_score(&query_chars, &thread.title.to_lowercase());
                if score > 0.0 {
                    Some((thread, score))
                } else {
                    // Also search in last message preview
                    let preview_score = fuzzy_match_score(
                        &query_chars,
                        &thread.last_message_preview.to_lowercase(),
                    );
                    if preview_score > 0.0 {
                        Some((thread, preview_score * 0.7)) // Lower weight for preview
                    } else {
                        None
                    }
                }
            })
            .collect();

        // Sort by score (highest first)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        results.into_iter().map(|(thread, _)| thread).collect()
    }

    /// Get all threads (pinned first, then by recency)
    pub fn get_all_threads(&self) -> Vec<&Thread> {
        let mut result = Vec::new();

        // First add pinned threads
        for id in &self.pinned_ids {
            if let Some(thread) = self.threads.get(id) {
                result.push(thread);
            }
        }

        // Then add non-pinned threads in order
        for id in &self.ordered_ids {
            if !self.pinned_ids.contains(id) {
                if let Some(thread) = self.threads.get(id) {
                    result.push(thread);
                }
            }
        }

        result
    }

    /// Get threads for a specific mode
    pub fn get_threads_by_mode(&self, mode: crate::types::ChatMode) -> Vec<&Thread> {
        self.get_all_threads()
            .into_iter()
            .filter(|t| t.mode == mode)
            .collect()
    }

    /// Get recent threads (last N)
    pub fn get_recent(&self, count: usize) -> Vec<&Thread> {
        self.get_all_threads().into_iter().take(count).collect()
    }

    /// Delete a thread
    pub fn delete_thread(&mut self, id: &str) {
        self.threads.remove(id);
        self.ordered_ids.retain(|tid| tid != id);
        self.pinned_ids.retain(|pid| pid != id);
    }

    /// Total thread count
    pub fn count(&self) -> usize {
        self.threads.len()
    }

    /// Export to JSON for persistence
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Import from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl Default for ThreadHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Fuzzy match score (fzf-style)
/// Higher score = better match
fn fuzzy_match_score(query_chars: &[char], target: &str) -> f64 {
    if query_chars.is_empty() {
        return 1.0;
    }

    let target_lower = target.to_lowercase();
    let target_chars: Vec<char> = target_lower.chars().collect();

    // Try to find the query as a consecutive substring
    if let Some(start_idx) = find_consecutive_match(query_chars, &target_chars) {
        let mut score = 0.0;

        // Base score for matching
        score += query_chars.len() as f64;

        // Word boundary bonus (if match starts at beginning or after whitespace)
        if start_idx == 0 || is_word_boundary(target_chars[start_idx - 1]) {
            score += 0.8;
        }

        // Consecutive character bonus
        score += (query_chars.len().saturating_sub(1)) as f64 * 0.5;

        // Bonus for starting at the very beginning
        let start_bonus = if start_idx == 0 { 0.5 } else { 0.0 };

        // Normalize by query length
        let base_score = score / query_chars.len() as f64;

        // Penalize long targets slightly
        let length_penalty = (target_chars.len() as f64 - query_chars.len() as f64) * 0.005;

        (base_score + start_bonus - length_penalty).max(0.0)
    } else {
        0.0 // No consecutive match found
    }
}

/// Find if query appears consecutively in target, return starting index if found
fn find_consecutive_match(query: &[char], target: &[char]) -> Option<usize> {
    if query.len() > target.len() {
        return None;
    }

    for start in 0..=target.len() - query.len() {
        let mut matched = true;
        for i in 0..query.len() {
            if query[i] != target[start + i] {
                matched = false;
                break;
            }
        }
        if matched {
            return Some(start);
        }
    }
    None
}

fn is_word_boundary(c: char) -> bool {
    c.is_whitespace() || c == '-' || c == '_' || c == '/' || c == '.'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_title() {
        let msg = "Can you help me find all PDF files in my downloads folder?";
        let title = Thread::generate_title(msg);
        assert!(!title.is_empty());
        assert!(!title.to_lowercase().contains("can you"));
    }

    #[test]
    fn test_fuzzy_match() {
        let query: Vec<char> = "pdf".chars().collect();

        // Good match
        let score1 = fuzzy_match_score(&query, "Find PDF files");
        assert!(score1 > 0.0);

        // Better match (starts with query)
        let score2 = fuzzy_match_score(&query, "PDF organization tips");
        assert!(score2 > score1);

        // No match
        let score3 = fuzzy_match_score(&query, "Something completely different");
        assert_eq!(score3, 0.0);
    }

    #[test]
    fn test_thread_history() {
        let mut history = ThreadHistory::new();

        let thread = Thread::new(
            "test-1".to_string(),
            crate::types::ChatMode::Fix,
            "Help me organize my files",
        );

        history.upsert_thread(thread);
        assert_eq!(history.count(), 1);

        let results = history.search("organize files");
        assert_eq!(results.len(), 1);
    }
}
