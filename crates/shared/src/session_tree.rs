//! Tree-structured session storage with JSONL persistence.
//!
//! Replaces the flat `Vec<ChatMessage>` session model with a tree that supports:
//! - **Branching**: fork from any message to explore alternative paths
//! - **Navigation**: walk the active branch, switch between siblings
//! - **Append-only JSONL**: each node is one JSON line, crash-safe
//! - **Compaction nodes**: summaries that replace ranges of messages (Phase 5)
//!
//! ## JSONL Format
//!
//! Each session file is a `.jsonl` file with one JSON object per line:
//!
//! ```text
//! {"type":"session_meta","id":"...","mode":"find","title":"...","created_at":"..."}
//! {"type":"node","id":"...","parent_id":null,"role":"assistant","content":"Welcome!","timestamp":"14:30"}
//! {"type":"node","id":"...","parent_id":"...","role":"user","content":"Hello","timestamp":"14:31"}
//! {"type":"active_leaf","node_id":"..."}
//! ```
//!
//! ## Relationship to the UI
//!
//! The `active_branch()` method returns messages along the path from root to
//! `active_leaf`, which the UI renders as a flat conversation. Branching creates
//! a new child of an earlier message and moves `active_leaf` to the new branch.
//!
//! ## Compatibility
//!
//! This module lives in the `shared` crate so both `app` (UI) and `agent_host`
//! (AI loop) can use it. The `BranchMessage` struct is intentionally compatible
//! with the app's `ChatMessage` fields (role, content, details, timestamp).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

/// Classifies what kind of node this is in the session tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum NodeType {
    /// A regular chat message (user, assistant, or system).
    Normal,
    /// A compaction summary that replaced a range of older messages.
    /// The original messages remain in the tree for navigation but are
    /// excluded from the active branch's API payload.
    Compaction {
        /// IDs of the messages this summary replaces.
        summarized_ids: Vec<Uuid>,
        /// Token count of the original messages (for stats/debugging).
        original_token_count: u32,
    },
    /// A system-generated event (e.g., mode switch, skill invocation log).
    SystemEvent,
}

/// A single node in the session tree.
///
/// Each node has an optional `parent_id` — root nodes have `None`.
/// Multiple children of the same parent represent conversation branches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageNode {
    /// Unique identifier for this node.
    pub id: Uuid,
    /// Parent node, or `None` for root messages (e.g., welcome message).
    pub parent_id: Option<Uuid>,
    /// Message role: "user", "assistant", or "system".
    pub role: String,
    /// The message content displayed to the user.
    pub content: String,
    /// Optional low-level details (provider errors, tool output, etc.).
    /// Kept out of the main message bubble but available on expand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// Display timestamp (e.g., "14:30") — matches the app's ChatMessage format.
    pub timestamp: String,
    /// When this node was created (ISO 8601).
    pub created_at: DateTime<Utc>,
    /// What kind of node this is (normal, compaction, system event).
    #[serde(default = "default_node_type")]
    pub node_type: NodeType,
}

/// Default to Normal for backwards compatibility with older JSONL files.
fn default_node_type() -> NodeType {
    NodeType::Normal
}

impl MessageNode {
    /// Create a new normal message node.
    pub fn new(parent_id: Option<Uuid>, role: &str, content: &str, timestamp: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            parent_id,
            role: role.to_string(),
            content: content.to_string(),
            details: None,
            timestamp: timestamp.to_string(),
            created_at: Utc::now(),
            node_type: NodeType::Normal,
        }
    }

    /// Create a new node with details attached.
    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }
}

// ---------------------------------------------------------------------------
// JSONL line types (tagged union for the file format)
// ---------------------------------------------------------------------------

/// A single line in the JSONL session file.
///
/// Each line is self-describing via the `type` field, making the format
/// forward-compatible — unknown line types are silently skipped.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JsonlLine {
    /// First line: session metadata.
    #[serde(rename = "session_meta")]
    SessionMeta {
        id: String,
        mode: String,
        title: String,
        created_at: DateTime<Utc>,
    },
    /// A message node in the tree.
    #[serde(rename = "node")]
    Node(MessageNode),
    /// Tracks which leaf is currently active (last one wins on load).
    #[serde(rename = "active_leaf")]
    ActiveLeaf { node_id: Uuid },
    /// Title update (emitted when auto-title is generated from first user message).
    #[serde(rename = "title_update")]
    TitleUpdate { title: String },
}

// ---------------------------------------------------------------------------
// Session metadata
// ---------------------------------------------------------------------------

/// Metadata for a session, stored as the first line of the JSONL file.
#[derive(Debug, Clone)]
pub struct SessionMeta {
    /// Unique session ID (UUID).
    pub id: String,
    /// Mode this session belongs to (e.g., "find", "fix", "research").
    pub mode: String,
    /// Human-readable title (auto-generated or user-set).
    pub title: String,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// SessionTree — the core data structure
// ---------------------------------------------------------------------------

/// A tree-structured conversation session.
///
/// Messages form a tree via parent_id references. The `active_leaf` tracks
/// which branch the user is currently viewing. Walking from active_leaf
/// back to the root gives the current conversation as a flat list.
#[derive(Debug, Clone)]
pub struct SessionTree {
    /// Session metadata (id, mode, title, etc.).
    pub meta: SessionMeta,
    /// All nodes indexed by their UUID.
    nodes: HashMap<Uuid, MessageNode>,
    /// Children of each node, in insertion order.
    children: HashMap<Uuid, Vec<Uuid>>,
    /// Root nodes (no parent). Usually just one (the welcome message).
    roots: Vec<Uuid>,
    /// The currently active leaf — determines which branch the UI shows.
    pub active_leaf: Option<Uuid>,
    /// Path to the JSONL file on disk (set when loaded or created).
    pub file_path: Option<PathBuf>,
}

impl SessionTree {
    /// Create a new empty session tree.
    pub fn new(id: &str, mode: &str, title: &str) -> Self {
        Self {
            meta: SessionMeta {
                id: id.to_string(),
                mode: mode.to_string(),
                title: title.to_string(),
                created_at: Utc::now(),
            },
            nodes: HashMap::new(),
            children: HashMap::new(),
            roots: Vec::new(),
            active_leaf: None,
            file_path: None,
        }
    }

    // -----------------------------------------------------------------------
    // Tree operations
    // -----------------------------------------------------------------------

    /// Append a node to the tree and update `active_leaf`.
    ///
    /// If the node has a parent, it's added as a child of that parent.
    /// If the node has no parent, it becomes a root node.
    /// The `active_leaf` is always updated to the new node's ID.
    pub fn append_node(&mut self, node: MessageNode) -> Uuid {
        let id = node.id;

        // Track parent-child relationship
        if let Some(parent_id) = node.parent_id {
            self.children.entry(parent_id).or_default().push(id);
        } else {
            self.roots.push(id);
        }

        self.nodes.insert(id, node);
        self.active_leaf = Some(id);
        id
    }

    /// Append a node and persist it to the JSONL file.
    ///
    /// This is the primary way to add messages during a conversation.
    /// The node is written to disk immediately (append-only, crash-safe).
    pub fn append_and_persist(&mut self, node: MessageNode) -> std::io::Result<Uuid> {
        let id = self.append_node(node.clone());

        // Write to JSONL file if we have a path
        if let Some(ref path) = self.file_path {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;

            let line = JsonlLine::Node(node);
            serde_json::to_writer(&mut file, &line)?;
            writeln!(file)?;

            // Also persist the active_leaf update
            let leaf_line = JsonlLine::ActiveLeaf { node_id: id };
            serde_json::to_writer(&mut file, &leaf_line)?;
            writeln!(file)?;
        }

        Ok(id)
    }

    /// Fork the conversation from a specific node.
    ///
    /// Creates a new child of `fork_point` with the given content,
    /// effectively starting a new branch. The `active_leaf` moves to
    /// the new branch.
    ///
    /// Returns the ID of the new node, or `None` if `fork_point` doesn't exist.
    pub fn fork_at(
        &mut self,
        fork_point: Uuid,
        role: &str,
        content: &str,
        timestamp: &str,
    ) -> Option<Uuid> {
        // Verify the fork point exists
        if !self.nodes.contains_key(&fork_point) {
            return None;
        }

        let node = MessageNode::new(Some(fork_point), role, content, timestamp);
        let id = self.append_node(node);
        Some(id)
    }

    /// Get the active branch as a flat list of nodes (root to leaf).
    ///
    /// Walks from `active_leaf` back to the root, then reverses to get
    /// chronological order. This is what the UI displays as the conversation.
    pub fn active_branch(&self) -> Vec<&MessageNode> {
        let leaf = match self.active_leaf {
            Some(id) => id,
            None => return Vec::new(),
        };

        let mut path = Vec::new();
        let mut current = Some(leaf);

        // Walk from leaf to root
        while let Some(id) = current {
            if let Some(node) = self.nodes.get(&id) {
                path.push(node);
                current = node.parent_id;
            } else {
                break;
            }
        }

        // Reverse to get root-to-leaf order
        path.reverse();
        path
    }

    /// Get sibling nodes at a given position in the tree.
    ///
    /// Returns all children of the same parent as `node_id`, including
    /// `node_id` itself. Useful for rendering branch navigation arrows
    /// (e.g., "< 2/3 >") in the UI.
    pub fn siblings_of(&self, node_id: Uuid) -> Vec<Uuid> {
        let node = match self.nodes.get(&node_id) {
            Some(n) => n,
            None => return vec![],
        };

        match node.parent_id {
            Some(parent_id) => {
                // Return all children of the parent
                self.children
                    .get(&parent_id)
                    .cloned()
                    .unwrap_or_default()
            }
            None => {
                // Root node — siblings are other roots
                self.roots.clone()
            }
        }
    }

    /// Check if a node has multiple children (is a branch point).
    pub fn is_branch_point(&self, node_id: Uuid) -> bool {
        self.children
            .get(&node_id)
            .map(|c| c.len() > 1)
            .unwrap_or(false)
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: Uuid) -> Option<&MessageNode> {
        self.nodes.get(&id)
    }

    /// Total number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the tree is empty (no nodes).
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Update the session title (e.g., auto-generated from first user message).
    pub fn set_title(&mut self, title: &str) {
        self.meta.title = title.to_string();
    }

    /// Persist a title update to the JSONL file.
    pub fn persist_title_update(&self) -> std::io::Result<()> {
        if let Some(ref path) = self.file_path {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;

            let line = JsonlLine::TitleUpdate {
                title: self.meta.title.clone(),
            };
            serde_json::to_writer(&mut file, &line)?;
            writeln!(file)?;
        }
        Ok(())
    }

    /// Switch the active branch to a sibling node.
    ///
    /// Finds the deepest leaf reachable from `sibling_id` by always
    /// following the first child. This "follows the branch down" so the
    /// user sees the full alternative conversation, not just the fork point.
    pub fn switch_to_branch(&mut self, sibling_id: Uuid) {
        // Walk down to the deepest leaf on this branch
        let mut current = sibling_id;
        loop {
            match self.children.get(&current) {
                Some(kids) if !kids.is_empty() => {
                    current = kids[0]; // Follow first child
                }
                _ => break,
            }
        }
        self.active_leaf = Some(current);
    }

    // -----------------------------------------------------------------------
    // JSONL persistence
    // -----------------------------------------------------------------------

    /// Create a new session and write the initial JSONL file.
    ///
    /// Writes the session_meta line. Subsequent nodes are written via
    /// `append_and_persist()`.
    pub fn create_file(&mut self, path: &Path) -> std::io::Result<()> {
        // Ensure parent directories exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::File::create(path)?;

        // Write session metadata as first line
        let meta_line = JsonlLine::SessionMeta {
            id: self.meta.id.clone(),
            mode: self.meta.mode.clone(),
            title: self.meta.title.clone(),
            created_at: self.meta.created_at,
        };
        serde_json::to_writer(&mut file, &meta_line)?;
        writeln!(file)?;

        self.file_path = Some(path.to_path_buf());
        Ok(())
    }

    /// Load a session tree from a JSONL file.
    ///
    /// Reads line-by-line, building the tree incrementally. Unknown line
    /// types are silently skipped for forward compatibility.
    pub fn from_jsonl(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);

        let mut tree: Option<SessionTree> = None;

        for (line_num, line_result) in reader.lines().enumerate() {
            let line = match line_result {
                Ok(l) if l.trim().is_empty() => continue,
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!("JSONL read error at line {}: {}", line_num + 1, e);
                    continue;
                }
            };

            let parsed: JsonlLine = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        "JSONL parse error at line {}: {} (line: {})",
                        line_num + 1,
                        e,
                        &line[..line.len().min(100)]
                    );
                    continue;
                }
            };

            match parsed {
                JsonlLine::SessionMeta {
                    id,
                    mode,
                    title,
                    created_at,
                } => {
                    let mut t = SessionTree::new(&id, &mode, &title);
                    t.meta.created_at = created_at;
                    t.file_path = Some(path.to_path_buf());
                    tree = Some(t);
                }
                JsonlLine::Node(node) => {
                    if let Some(ref mut t) = tree {
                        t.append_node(node);
                    }
                }
                JsonlLine::ActiveLeaf { node_id } => {
                    if let Some(ref mut t) = tree {
                        t.active_leaf = Some(node_id);
                    }
                }
                JsonlLine::TitleUpdate { title } => {
                    if let Some(ref mut t) = tree {
                        t.meta.title = title;
                    }
                }
            }
        }

        tree.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "JSONL file missing session_meta line",
            )
        })
    }

    /// Convert a flat list of messages (from the old Vec<ChatMessage> format)
    /// into a SessionTree with a linear chain of parent references.
    ///
    /// Used for migrating v1 sessions to the tree format.
    pub fn from_linear_messages(
        session_id: &str,
        mode: &str,
        title: &str,
        messages: Vec<(String, String, Option<String>, String)>, // (role, content, details, timestamp)
    ) -> Self {
        let mut tree = SessionTree::new(session_id, mode, title);
        let mut last_id: Option<Uuid> = None;

        for (role, content, details, timestamp) in messages {
            let mut node = MessageNode::new(last_id, &role, &content, &timestamp);
            if let Some(d) = details {
                node.details = Some(d);
            }
            last_id = Some(tree.append_node(node));
        }

        tree
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a simple tree with a welcome message and two exchanges.
    fn sample_tree() -> SessionTree {
        let mut tree = SessionTree::new("test-session", "find", "Test Session");

        let welcome = MessageNode::new(None, "assistant", "Welcome!", "14:00");
        let welcome_id = tree.append_node(welcome);

        let user1 = MessageNode::new(Some(welcome_id), "user", "Hello", "14:01");
        let user1_id = tree.append_node(user1);

        let assistant1 =
            MessageNode::new(Some(user1_id), "assistant", "How can I help?", "14:01");
        tree.append_node(assistant1);

        tree
    }

    #[test]
    fn test_empty_tree() {
        let tree = SessionTree::new("s1", "find", "Empty");
        assert!(tree.is_empty());
        assert_eq!(tree.node_count(), 0);
        assert!(tree.active_branch().is_empty());
    }

    #[test]
    fn test_append_and_active_branch() {
        let tree = sample_tree();

        assert_eq!(tree.node_count(), 3);
        assert!(!tree.is_empty());

        let branch = tree.active_branch();
        assert_eq!(branch.len(), 3);
        assert_eq!(branch[0].role, "assistant");
        assert_eq!(branch[0].content, "Welcome!");
        assert_eq!(branch[1].role, "user");
        assert_eq!(branch[1].content, "Hello");
        assert_eq!(branch[2].role, "assistant");
        assert_eq!(branch[2].content, "How can I help?");
    }

    #[test]
    fn test_fork_creates_branch() {
        let mut tree = sample_tree();

        // Fork from the welcome message (the root)
        let welcome_id = tree.roots[0];
        let fork_id = tree
            .fork_at(welcome_id, "user", "Different question", "14:05")
            .unwrap();

        // Active branch should now be: welcome -> "Different question"
        let branch = tree.active_branch();
        assert_eq!(branch.len(), 2);
        assert_eq!(branch[1].content, "Different question");

        // Welcome message should now be a branch point
        assert!(tree.is_branch_point(welcome_id));

        // Siblings of the fork should include the original user message and the fork
        let siblings = tree.siblings_of(fork_id);
        assert_eq!(siblings.len(), 2);
    }

    #[test]
    fn test_fork_nonexistent_returns_none() {
        let mut tree = sample_tree();
        let fake_id = Uuid::new_v4();
        assert!(tree.fork_at(fake_id, "user", "test", "14:00").is_none());
    }

    #[test]
    fn test_switch_to_branch() {
        let mut tree = sample_tree();

        // Get the first user message ID (child of welcome)
        let welcome_id = tree.roots[0];
        let original_user_id = tree.children.get(&welcome_id).unwrap()[0];

        // Fork from welcome to create a second branch
        let fork_id = tree
            .fork_at(welcome_id, "user", "Branch B", "14:05")
            .unwrap();

        // Add a reply on the new branch
        tree.fork_at(fork_id, "assistant", "Branch B reply", "14:06");

        // Active branch should be on Branch B (3 nodes: welcome -> fork -> reply)
        assert_eq!(tree.active_branch().len(), 3);
        assert_eq!(tree.active_branch()[1].content, "Branch B");

        // Switch back to original branch
        tree.switch_to_branch(original_user_id);

        // Should follow original_user_id down to its deepest leaf
        let branch = tree.active_branch();
        assert_eq!(branch[1].content, "Hello");
    }

    #[test]
    fn test_siblings_of_root() {
        let mut tree = SessionTree::new("s1", "find", "Test");

        let root1 = MessageNode::new(None, "assistant", "Welcome 1", "14:00");
        let root1_id = tree.append_node(root1);

        let root2 = MessageNode::new(None, "system", "System event", "14:00");
        tree.append_node(root2);

        // Both roots are siblings of each other
        let siblings = tree.siblings_of(root1_id);
        assert_eq!(siblings.len(), 2);
    }

    #[test]
    fn test_from_linear_messages() {
        let messages = vec![
            (
                "assistant".to_string(),
                "Welcome!".to_string(),
                None,
                "14:00".to_string(),
            ),
            (
                "user".to_string(),
                "Hello".to_string(),
                None,
                "14:01".to_string(),
            ),
            (
                "assistant".to_string(),
                "Hi there!".to_string(),
                Some("debug info".to_string()),
                "14:01".to_string(),
            ),
        ];

        let tree = SessionTree::from_linear_messages("s1", "find", "Test", messages);

        assert_eq!(tree.node_count(), 3);

        let branch = tree.active_branch();
        assert_eq!(branch.len(), 3);
        assert_eq!(branch[0].content, "Welcome!");
        assert!(branch[0].parent_id.is_none()); // Root
        assert!(branch[1].parent_id.is_some()); // Child of root
        assert_eq!(branch[2].details.as_deref(), Some("debug info"));
    }

    #[test]
    fn test_jsonl_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-session.jsonl");

        // Create and populate a tree
        let mut tree = SessionTree::new("s1", "find", "Test Session");
        tree.create_file(&path).unwrap();

        let welcome = MessageNode::new(None, "assistant", "Welcome!", "14:00");
        tree.append_and_persist(welcome).unwrap();

        let welcome_id = tree.roots[0];
        let user_msg = MessageNode::new(Some(welcome_id), "user", "Hello!", "14:01");
        tree.append_and_persist(user_msg).unwrap();

        // Update title
        tree.set_title("Hello conversation");
        tree.persist_title_update().unwrap();

        // Load from disk
        let loaded = SessionTree::from_jsonl(&path).unwrap();

        assert_eq!(loaded.meta.id, "s1");
        assert_eq!(loaded.meta.mode, "find");
        assert_eq!(loaded.meta.title, "Hello conversation");
        assert_eq!(loaded.node_count(), 2);

        let branch = loaded.active_branch();
        assert_eq!(branch.len(), 2);
        assert_eq!(branch[0].content, "Welcome!");
        assert_eq!(branch[1].content, "Hello!");
    }

    #[test]
    fn test_jsonl_missing_meta_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.jsonl");

        // Write a node without session_meta first
        std::fs::write(
            &path,
            r#"{"type":"node","id":"550e8400-e29b-41d4-a716-446655440000","parent_id":null,"role":"user","content":"hi","timestamp":"14:00","created_at":"2024-01-01T00:00:00Z","node_type":{"kind":"Normal"}}"#,
        )
        .unwrap();

        let result = SessionTree::from_jsonl(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_jsonl_skips_invalid_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("partial.jsonl");

        // Write valid meta, then garbage, then valid node
        let content = format!(
            "{}\nthis is not json\n{}\n",
            serde_json::to_string(&JsonlLine::SessionMeta {
                id: "s1".into(),
                mode: "find".into(),
                title: "Test".into(),
                created_at: Utc::now(),
            })
            .unwrap(),
            serde_json::to_string(&JsonlLine::Node(MessageNode::new(
                None,
                "assistant",
                "Welcome!",
                "14:00"
            )))
            .unwrap(),
        );
        std::fs::write(&path, content).unwrap();

        let tree = SessionTree::from_jsonl(&path).unwrap();
        assert_eq!(tree.node_count(), 1);
        assert_eq!(tree.active_branch()[0].content, "Welcome!");
    }

    #[test]
    fn test_node_type_default() {
        // Verify deserialization without node_type field defaults to Normal
        let json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","parent_id":null,"role":"user","content":"hi","timestamp":"14:00","created_at":"2024-01-01T00:00:00Z"}"#;
        let node: MessageNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.node_type, NodeType::Normal);
    }

    #[test]
    fn test_compaction_node_type() {
        let node = MessageNode {
            id: Uuid::new_v4(),
            parent_id: None,
            role: "system".to_string(),
            content: "Summary of previous messages".to_string(),
            details: None,
            timestamp: "14:00".to_string(),
            created_at: Utc::now(),
            node_type: NodeType::Compaction {
                summarized_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
                original_token_count: 3500,
            },
        };

        // Roundtrip through JSON
        let json = serde_json::to_string(&node).unwrap();
        let decoded: MessageNode = serde_json::from_str(&json).unwrap();

        match decoded.node_type {
            NodeType::Compaction {
                summarized_ids,
                original_token_count,
            } => {
                assert_eq!(summarized_ids.len(), 2);
                assert_eq!(original_token_count, 3500);
            }
            other => panic!("Expected Compaction, got {:?}", other),
        }
    }

    #[test]
    fn test_set_title_and_persist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("title-test.jsonl");

        let mut tree = SessionTree::new("s1", "find", "Original Title");
        tree.create_file(&path).unwrap();

        tree.set_title("Updated Title");
        tree.persist_title_update().unwrap();

        let loaded = SessionTree::from_jsonl(&path).unwrap();
        assert_eq!(loaded.meta.title, "Updated Title");
    }
}
