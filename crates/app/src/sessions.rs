//! Session management and agent memory
//!
//! Each mode has its own agent personality with specific skills.
//! Conversations are saved and searchable.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// A chat message within a session
#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub timestamp: String,
    /// Optional preview reference for restoring preview when scrolling back
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_ref: Option<shared::preview_types::PreviewReference>,
}

/// Chat modes - each is a distinct agent with different skills
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub enum ChatMode {
    Find,
    Fix,
    Research,
    Data,
    Content,
}

impl ChatMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatMode::Find => "find",
            ChatMode::Fix => "fix",
            ChatMode::Research => "research",
            ChatMode::Data => "data",
            ChatMode::Content => "content",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ChatMode::Find => "Find",
            ChatMode::Fix => "Fix",
            ChatMode::Research => "Research",
            ChatMode::Data => "Data",
            ChatMode::Content => "Content",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ChatMode::Find => "🔍",
            ChatMode::Fix => "🔧",
            ChatMode::Research => "📚",
            ChatMode::Data => "📊",
            ChatMode::Content => "✏️",
        }
    }

    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            ChatMode::Find => (70, 130, 180),
            ChatMode::Fix => (180, 100, 70),
            ChatMode::Research => (130, 100, 180),
            ChatMode::Data => (70, 150, 130),
            ChatMode::Content => (180, 130, 70),
        }
    }

    pub fn welcome(&self, name: &str) -> String {
        match self {
            ChatMode::Find => format!("Hi {}! I'm your Find agent. I can search your computer for files, folders, and content. What are you looking for?", name),
            ChatMode::Fix => format!("Hi {}! I'm your Fix agent. Tell me what's broken or not working right, and I'll run diagnostics to figure it out.", name),
            ChatMode::Research => format!("Hi {}! I'm your Research agent. I'll dig deep into topics, search multiple sources, and give you cited answers. What should I investigate?", name),
            ChatMode::Data => format!("Hi {}! I'm your Data agent. I can analyze CSV files, JSON, databases - whatever you've got. What data are we working with?", name),
            ChatMode::Content => format!("Hi {}! I'm your Content agent. I know your campaign materials and personas. What content should we create?", name),
        }
    }
}

/// A saved conversation session
#[derive(Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub mode: ChatMode,
    pub title: String,
    pub messages: Vec<ChatMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ChatSession {
    pub fn new(mode: ChatMode, user_name: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            mode,
            title: "New conversation".to_string(),
            messages: vec![ChatMessage {
                role: "assistant".to_string(),
                content: mode.welcome(user_name),
                timestamp: Utc::now().format("%H:%M").to_string(),
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn add_message(&mut self, msg: ChatMessage) {
        if self.title == "New conversation" && msg.role == "user" {
            self.title = msg
                .content
                .chars()
                .take(40)
                .collect::<String>()
                .trim()
                .to_string();
            if msg.content.len() > 40 {
                self.title.push_str("...");
            }
        }
        self.messages.push(msg);
        self.updated_at = Utc::now();
    }

    fn filename(&self) -> String {
        format!("{}.json", self.id)
    }
}

/// Manages sessions with persistence and search
pub struct SessionManager {
    base_path: PathBuf,
    sessions: std::collections::HashMap<ChatMode, Vec<ChatSession>>,
}

impl SessionManager {
    pub fn new() -> Self {
        let base_path = Self::get_base_path();
        let mut mgr = Self {
            base_path,
            sessions: std::collections::HashMap::new(),
        };
        mgr.load_all();
        mgr
    }

    fn get_base_path() -> PathBuf {
        directories::ProjectDirs::from("com.local", "Little Helper", "LittleHelper")
            .map(|p| p.config_dir().join("sessions"))
            .unwrap_or_else(|| PathBuf::from("./sessions"))
    }

    fn mode_dir(&self, mode: ChatMode) -> PathBuf {
        let dir = self.base_path.join(mode.as_str());
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn load_all(&mut self) {
        for mode in [
            ChatMode::Find,
            ChatMode::Fix,
            ChatMode::Research,
            ChatMode::Data,
            ChatMode::Content,
        ] {
            let mut sessions = Vec::new();
            let dir = self.mode_dir(mode);

            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if let Ok(session) = serde_json::from_str::<ChatSession>(&content) {
                            sessions.push(session);
                        }
                    }
                }
            }

            sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            self.sessions.insert(mode, sessions);
        }
    }

    pub fn list(&self, mode: ChatMode) -> &[ChatSession] {
        self.sessions
            .get(&mode)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn get(&self, mode: ChatMode, id: &str) -> Option<&ChatSession> {
        self.sessions.get(&mode)?.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, mode: ChatMode, id: &str) -> Option<&mut ChatSession> {
        self.sessions
            .get_mut(&mode)?
            .iter_mut()
            .find(|s| s.id == id)
    }

    pub fn current(&self, mode: ChatMode) -> Option<&ChatSession> {
        self.sessions.get(&mode)?.first()
    }

    pub fn create(&mut self, mode: ChatMode, user_name: &str) -> String {
        let session = ChatSession::new(mode, user_name);
        let id = session.id.clone();
        self.save(&session);
        self.sessions.entry(mode).or_default().insert(0, session);
        id
    }

    pub fn add_message(&mut self, mode: ChatMode, id: &str, msg: ChatMessage) {
        if let Some(session) = self.get_mut(mode, id) {
            session.add_message(msg);
            let session_clone = session.clone();
            self.save(&session_clone);
        }
    }

    fn save(&self, session: &ChatSession) {
        let path = self.mode_dir(session.mode).join(session.filename());
        if let Ok(json) = serde_json::to_string_pretty(session) {
            let _ = fs::write(path, json);
        }
    }

    pub fn delete(&mut self, mode: ChatMode, id: &str) {
        if let Some(sessions) = self.sessions.get_mut(&mode) {
            if let Some(pos) = sessions.iter().position(|s| s.id == id) {
                let session = sessions.remove(pos);
                let path = self.mode_dir(mode).join(session.filename());
                let _ = fs::remove_file(path);
            }
        }
    }

    /// Search past conversations (simple keyword match)
    pub fn search(&self, mode: ChatMode, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        if let Some(sessions) = self.sessions.get(&mode) {
            for session in sessions {
                for (i, msg) in session.messages.iter().enumerate() {
                    if msg.content.to_lowercase().contains(&query_lower) {
                        results.push(SearchResult {
                            session_id: session.id.clone(),
                            session_title: session.title.clone(),
                            message_index: i,
                            snippet: extract_snippet(&msg.content, &query_lower),
                            date: session.updated_at,
                        });
                    }
                }
            }
        }

        results.truncate(10); // Limit results
        results
    }

    /// Get summary of past sessions for agent context
    pub fn get_memory_summary(&self, mode: ChatMode) -> String {
        let sessions = self.list(mode);
        if sessions.is_empty() {
            return "No previous conversations in this mode.".to_string();
        }

        let count = sessions.len();
        let recent: Vec<_> = sessions
            .iter()
            .take(5)
            .map(|s| format!("- \"{}\" ({})", s.title, s.updated_at.format("%b %d")))
            .collect();

        format!(
            "You have {} past conversation(s) in this mode. Recent topics:\n{}",
            count,
            recent.join("\n")
        )
    }

    /// Get recent messages for AI context window (T111)
    /// Returns the most recent messages from the current session, limited to max_messages
    pub fn get_context_messages(&self, mode: ChatMode, session_id: &str, max_messages: usize) -> Vec<ChatMessage> {
        if let Some(session) = self.get(mode, session_id) {
            let msg_count = session.messages.len();
            let start = msg_count.saturating_sub(max_messages);
            session.messages[start..].to_vec()
        } else {
            Vec::new()
        }
    }

    /// Get or load a session by ID, loading from disk if not in memory (T112)
    pub fn get_or_load(&mut self, mode: ChatMode, id: &str) -> Option<&ChatSession> {
        // First check if already in memory
        if self.get(mode, id).is_some() {
            return self.get(mode, id);
        }

        // Try to load from disk
        let path = self.mode_dir(mode).join(format!("{}.json", id));
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(session) = serde_json::from_str::<ChatSession>(&content) {
                self.sessions.entry(mode).or_default().push(session);
                // Re-sort by updated_at
                if let Some(sessions) = self.sessions.get_mut(&mode) {
                    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                }
                return self.get(mode, id);
            }
        }

        None
    }

    /// Force reload sessions for a mode from disk (T113)
    pub fn reload(&mut self, mode: ChatMode) {
        let mut sessions = Vec::new();
        let dir = self.mode_dir(mode);

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(session) = serde_json::from_str::<ChatSession>(&content) {
                        sessions.push(session);
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.sessions.insert(mode, sessions);
    }

    /// Get messages for lazy loading scrollback (T114)
    /// Returns a slice of messages starting from offset with limit count
    pub fn get_messages_paginated(
        &self,
        mode: ChatMode,
        session_id: &str,
        offset: usize,
        limit: usize,
    ) -> Vec<ChatMessage> {
        if let Some(session) = self.get(mode, session_id) {
            let end = offset.min(session.messages.len());
            let start = end.saturating_sub(limit);
            session.messages[start..end].to_vec()
        } else {
            Vec::new()
        }
    }

    /// Clear all sessions for a mode (T118)
    pub fn clear_mode(&mut self, mode: ChatMode) {
        let dir = self.mode_dir(mode);

        // Delete all files in the mode directory
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }

        // Clear from memory
        self.sessions.insert(mode, Vec::new());
    }

    /// Get the current active session ID for a mode, creating one if none exists
    pub fn ensure_current(&mut self, mode: ChatMode, user_name: &str) -> String {
        if let Some(session) = self.current(mode) {
            session.id.clone()
        } else {
            self.create(mode, user_name)
        }
    }

    /// Get brief context from current session for system prompt
    /// Includes last topic discussed and any pending tasks mentioned
    pub fn get_brief_context(&self, mode: ChatMode, session_id: &str) -> String {
        if let Some(session) = self.get(mode, session_id) {
            if session.messages.len() <= 1 {
                return String::new();
            }

            // Get last few exchanges
            let recent: Vec<_> = session.messages.iter().rev().take(4).collect();

            let mut context_parts = Vec::new();

            // Extract last user message topic
            if let Some(last_user) = recent.iter().find(|m| m.role == "user") {
                let topic: String = last_user.content.chars().take(100).collect();
                context_parts.push(format!("Last topic: {}", topic.trim()));
            }

            // Check if session title gives context
            if session.title != "New conversation" {
                context_parts.push(format!("Session topic: {}", session.title));
            }

            if context_parts.is_empty() {
                String::new()
            } else {
                context_parts.join("\n")
            }
        } else {
            String::new()
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct SearchResult {
    pub session_id: String,
    pub session_title: String,
    pub message_index: usize,
    pub snippet: String,
    pub date: DateTime<Utc>,
}

fn extract_snippet(content: &str, query: &str) -> String {
    let lower = content.to_lowercase();
    if let Some(pos) = lower.find(query) {
        let start = pos.saturating_sub(30);
        let end = (pos + query.len() + 30).min(content.len());
        let mut snippet = content[start..end].to_string();
        if start > 0 {
            snippet = format!("...{}", snippet);
        }
        if end < content.len() {
            snippet = format!("{}...", snippet);
        }
        snippet
    } else {
        content.chars().take(60).collect()
    }
}
