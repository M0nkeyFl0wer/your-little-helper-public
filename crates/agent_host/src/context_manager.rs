//! Context Manager -- the RAG orchestration layer.
//!
//! Owns the document corpus, the knowledge graph (`GraphStore`), the
//! embedding service, and the daily log. Together these form the agent's
//! long-term memory:
//!
//! - **Documents** are typed (Persona, Research, Skill, Template,
//!   Reference, Campaign) and auto-loaded based on the active agent mode.
//! - **Knowledge graph** stores entities extracted from documents and
//!   their relationships for multi-hop retrieval.
//! - **Embeddings** enable semantic search over both documents and graph
//!   nodes.
//! - **Daily log** provides episodic, date-keyed archival memory.
//!
//! The manager also supports distribution levels (Internal / ExternalBeta /
//! Public) to control what context is exposed in different release tiers.

use crate::daily_log::DailyLogManager;
use crate::embedding::EmbeddingService;
use crate::graph_store::GraphStore;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use shared::skill::Mode;
use std::collections::HashMap;
use std::path::PathBuf;
use walkdir::WalkDir; // Added import

/// Types of context documents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextType {
    /// User personas for content targeting
    Persona,
    /// Research data and findings
    Research,
    /// Special skill documentation
    Skill,
    /// File and data templates
    Template,
    /// General reference documents
    Reference,
    /// Campaign/project specific context
    Campaign,
}

impl ContextType {
    pub fn display_name(&self) -> &'static str {
        match self {
            ContextType::Persona => "👤 Personas",
            ContextType::Research => "🔬 Research",
            ContextType::Skill => "🛠️  Skills",
            ContextType::Template => "📄 Templates",
            ContextType::Reference => "📚 Reference",
            ContextType::Campaign => "🎯 Campaign",
        }
    }

    pub fn folder_name(&self) -> &'static str {
        match self {
            ContextType::Persona => "personas",
            ContextType::Research => "research",
            ContextType::Skill => "skills",
            ContextType::Template => "templates",
            ContextType::Reference => "reference",
            ContextType::Campaign => "campaigns",
        }
    }

    /// Which chat modes can use this context type
    pub fn applicable_modes(&self) -> &'static [Mode] {
        match self {
            ContextType::Persona => &[Mode::Content],
            ContextType::Research => &[Mode::Research, Mode::Data],
            ContextType::Skill => &[Mode::Fix, Mode::Data],
            ContextType::Template => &[Mode::Data, Mode::Content],
            ContextType::Reference => &[Mode::Fix, Mode::Research, Mode::Data, Mode::Content],
            ContextType::Campaign => &[Mode::Content, Mode::Research],
        }
    }
}

/// Distribution level for context packages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionLevel {
    /// Internal team only - full project context
    Internal,
    /// External beta testers - curated project context
    ExternalBeta,
    /// Public release - generic documentation only
    Public,
}

/// A context document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDocument {
    /// Unique identifier
    pub id: String,
    /// Document name/title
    pub name: String,
    /// Type of context
    pub context_type: ContextType,
    /// File path
    pub path: PathBuf,
    /// Content (loaded on demand)
    #[serde(skip)]
    pub content: Option<String>,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// Description
    pub description: String,
    /// When added
    pub added_at: chrono::DateTime<chrono::Utc>,
    /// Size in bytes
    pub size_bytes: u64,
}

/// Context search result
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextSearchResult {
    pub document: ContextDocument,
    /// Relevance score (0-100)
    pub relevance_score: u8,
    /// Matching excerpts
    pub excerpts: Vec<String>,
}

/// Context manager
pub struct ContextManager {
    /// Base directory for context storage
    base_dir: PathBuf,
    /// All loaded documents
    documents: HashMap<String, ContextDocument>,
    /// Document contents cache
    content_cache: HashMap<String, String>,
    /// Knowledge Graph for RAG
    pub graph: GraphStore,
    /// Daily Log Manager for Episodic Memory
    daily_log: DailyLogManager,
    /// Configuration for embedding service
    pub embedding_service: Option<EmbeddingService>,

    /// Track documents added since last optimization
    pub docs_added_since_opt: usize,
}

impl ContextManager {
    /// Increment document counter and return current count
    pub fn increment_docs_added(&mut self) -> usize {
        self.docs_added_since_opt += 1;
        self.docs_added_since_opt
    }

    /// Check if optimization is needed based on threshold
    pub fn needs_optimization(&mut self, threshold: usize) -> bool {
        if self.docs_added_since_opt >= threshold {
            self.docs_added_since_opt = 0;
            true
        } else {
            false
        }
    }

    /// Create a new context manager
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        // Ensure base directory exists
        std::fs::create_dir_all(&base_dir)?;

        // Create subdirectories for each type
        for context_type in [
            ContextType::Persona,
            ContextType::Research,
            ContextType::Skill,
            ContextType::Template,
            ContextType::Reference,
            ContextType::Campaign,
        ] {
            let dir = base_dir.join(context_type.folder_name());
            std::fs::create_dir_all(&dir)?;
        }

        // Initialize embedding service (graceful failure)
        let embedding_service = match EmbeddingService::new() {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("Warning: Failed to initialize embedding service: {}", e);
                None
            }
        };

        let mut manager = Self {
            base_dir: base_dir.to_path_buf(),
            documents: HashMap::new(),
            content_cache: HashMap::new(),
            graph: GraphStore::new(),
            daily_log: DailyLogManager::new(
                &base_dir.parent().unwrap_or(&base_dir).join("memory"),
            )?,
            embedding_service,
            docs_added_since_opt: 0,
        };

        // Load existing documents
        manager.scan_documents()?;

        Ok(manager)
    }

    /// Get the default context directory
    pub fn default_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("little_helper")
            .join("context")
    }

    /// Scan for all documents in the context directory
    fn scan_documents(&mut self) -> Result<()> {
        self.documents.clear();

        for entry in WalkDir::new(&self.base_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();

            // Determine context type from parent folder
            let context_type = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .and_then(|name| match name {
                    "personas" => Some(ContextType::Persona),
                    "research" => Some(ContextType::Research),
                    "skills" => Some(ContextType::Skill),
                    "templates" => Some(ContextType::Template),
                    "reference" => Some(ContextType::Reference),
                    "campaigns" => Some(ContextType::Campaign),
                    _ => None,
                })
                .unwrap_or(ContextType::Reference);

            let metadata = std::fs::metadata(path)?;
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();

            let id = format!(
                "{}/{}",
                context_type.folder_name(),
                path.file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("unknown")
            );

            let doc = ContextDocument {
                id: id.clone(),
                name,
                context_type,
                path: path.to_path_buf(),
                content: None,
                tags: Vec::new(),
                description: String::new(),
                added_at: chrono::Utc::now(),
                size_bytes: metadata.len(),
            };

            // Add to graph with appropriate mode
            let mode = context_type
                .applicable_modes()
                .first()
                .copied()
                .unwrap_or(shared::skill::Mode::Find);
            let graph_mode = match mode {
                shared::skill::Mode::Find => crate::graph_store::Mode::Find,
                shared::skill::Mode::Fix => crate::graph_store::Mode::Fix,
                shared::skill::Mode::Research => crate::graph_store::Mode::Research,
                shared::skill::Mode::Build => crate::graph_store::Mode::Build,
                shared::skill::Mode::Data => crate::graph_store::Mode::Data,
                shared::skill::Mode::Content => crate::graph_store::Mode::Content,
            };

            // Embedding logic
            let mut embedding = None;
            if let Some(service) = &self.embedding_service {
                // If not already embedded, read file and embed
                // Note: We use doc.name as label. If multiple files have same name, we might skip embedding for second one.
                // Improvement: Check by ID or ensure unique labels. For now, rely on idempotency.
                if !self.graph.has_embedding(&doc.name) {
                    // Limit embedding to reasonable file sizes to avoid startup lag
                    if doc.size_bytes < 50_000 {
                        if let Ok(content) = std::fs::read_to_string(&doc.path) {
                            if let Ok(e) = service.embed(&content) {
                                embedding = Some(e);
                            }
                        }
                    }
                }
            }

            self.graph.add_node(
                &doc.name,
                Some("Document".to_string()),
                &doc.id,
                graph_mode,
                embedding,
            );

            self.documents.insert(id, doc);
            self.docs_added_since_opt += 1;
        }

        if self.needs_optimization(50) {
            self.optimize_memory();
        }

        Ok(())
    }

    /// Scan external directories and add to context
    pub fn scan_external_dirs(&mut self, dirs: &[String]) -> Result<()> {
        for dir_str in dirs {
            let dir = PathBuf::from(dir_str);
            if !dir.exists() {
                continue;
            }

            // Limit depth to avoid massive scans
            for entry in WalkDir::new(&dir)
                .follow_links(true)
                .max_depth(5)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let path = entry.path();
                // Skip hidden files/dirs
                if path.to_string_lossy().contains("/.") {
                    continue;
                }

                // Only process text files
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let valid_exts = [
                    "md", "txt", "rs", "py", "js", "ts", "json", "toml", "yaml", "yml", "html",
                    "css", "c", "cpp", "h",
                ];
                if !valid_exts.contains(&ext.as_str()) {
                    continue;
                }

                // Construct ID
                // Use a stable ID based on path hash or similar? For now, path string is fine.
                let id = format!(
                    "external/{}",
                    path.to_string_lossy().replace("/", "_").replace("\\", "_")
                );

                // Check if doc exists
                if !self.documents.contains_key(&id) {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let metadata = std::fs::metadata(path)?;

                    let doc = ContextDocument {
                        id: id.clone(),
                        name: name.clone(),
                        context_type: ContextType::Reference,
                        path: path.to_path_buf(),
                        content: None,
                        tags: vec!["external".to_string()],
                        description: format!("External file from {}", dir_str),
                        added_at: chrono::Utc::now(),
                        size_bytes: metadata.len(),
                    };

                    // Embed
                    let mut embedding = None;
                    if let Some(service) = &self.embedding_service {
                        if !self.graph.has_embedding(&doc.name) && doc.size_bytes < 50_000 {
                            if let Ok(content) = std::fs::read_to_string(&doc.path) {
                                if let Ok(e) = service.embed(&content) {
                                    embedding = Some(e);
                                }
                            }
                        }
                    }

                    self.graph.add_node(
                        &doc.name,
                        Some("External File".to_string()),
                        &doc.id,
                        crate::graph_store::Mode::Research,
                        embedding,
                    );
                    self.documents.insert(id, doc);
                    self.docs_added_since_opt += 1;
                }
            }
        }

        if self.needs_optimization(50) {
            self.optimize_memory();
        }
        Ok(())
    }

    /// Add a new document
    pub fn add_document(
        &mut self,
        name: &str,
        context_type: ContextType,
        content: &str,
        description: &str,
        tags: Vec<String>,
    ) -> Result<ContextDocument> {
        let folder = self.base_dir.join(context_type.folder_name());
        let filename = format!("{}.md", name.replace(" ", "_").to_lowercase());
        let path = folder.join(&filename);

        // Write content
        std::fs::write(&path, content)?;

        let id = format!("{}/{}", context_type.folder_name(), filename);

        let doc = ContextDocument {
            id: id.clone(),
            name: name.to_string(),
            context_type,
            path,
            content: Some(content.to_string()),
            tags,
            description: description.to_string(),
            added_at: chrono::Utc::now(),
            size_bytes: content.len() as u64,
        };

        self.content_cache.insert(id.clone(), content.to_string());

        // Add to graph with appropriate mode
        let mode = context_type
            .applicable_modes()
            .first()
            .copied()
            .unwrap_or(shared::skill::Mode::Find);
        let graph_mode = match mode {
            shared::skill::Mode::Find => crate::graph_store::Mode::Find,
            shared::skill::Mode::Fix => crate::graph_store::Mode::Fix,
            shared::skill::Mode::Research => crate::graph_store::Mode::Research,
            shared::skill::Mode::Build => crate::graph_store::Mode::Build,
            shared::skill::Mode::Data => crate::graph_store::Mode::Data,
            shared::skill::Mode::Content => crate::graph_store::Mode::Content,
        };

        let embedding = self
            .embedding_service
            .as_ref()
            .and_then(|s| s.embed(content).ok());
        self.graph.add_node(
            &doc.name,
            Some("Document".to_string()),
            &doc.id,
            graph_mode,
            embedding,
        );

        self.documents.insert(id.clone(), doc.clone());
        self.docs_added_since_opt += 1;

        if self.needs_optimization(50) {
            self.optimize_memory();
        }

        Ok(doc)
    }

    /// Run memory optimization (consolidate & prune)
    fn optimize_memory(&mut self) {
        println!("[System] Running automated memory optimization...");
        // Consolidate duplicates (high confidence)
        let merged = self.graph.consolidate_nodes(0.95);
        // Prune only very low quality nodes, keep old ones for now unless explicitly hated
        let pruned = self.graph.prune_nodes(-0.8, 60);

        if merged > 0 || pruned > 0 {
            println!(
                "[System] Memory Optimized: Merged {} nodes, Pruned {} nodes.",
                merged, pruned
            );
        }
    }

    /// Add a node to the knowledge graph manually (or via LLM extraction)
    pub fn add_graph_node(
        &mut self,
        label: &str,
        category: Option<String>,
        source_id: &str,
        mode: crate::graph_store::Mode,
    ) {
        let embedding = self
            .embedding_service
            .as_ref()
            .and_then(|s| s.embed(label).ok());
        self.graph
            .add_node(label, category, source_id, mode, embedding);
    }

    /// Add a relationship to the knowledge graph
    pub fn add_graph_edge(
        &mut self,
        source_label: &str,
        target_label: &str,
        relation: &str,
        mode: crate::graph_store::Mode,
    ) {
        // We try to embed labels if they are new nodes, but add_node handles updates.
        // However, generating embeddings for every edge addition might be overkill if nodes exist.
        // add_node checks if embedding exists inside the graph store logic I wrote?
        // No, I wrote: if let Some(node) = node_weight_mut { if node.embedding.is_none() && embedding.is_some() ... }
        // So passing embedding is safe.

        // Optimize: Check if nodes exist to avoid embedding cost?
        // For now, let's just do it. Labels are short.
        let source_embedding = self
            .embedding_service
            .as_ref()
            .and_then(|s| s.embed(source_label).ok());
        let source_idx = self
            .graph
            .add_node(source_label, None, "system", mode, source_embedding);

        let target_embedding = self
            .embedding_service
            .as_ref()
            .and_then(|s| s.embed(target_label).ok());
        let target_idx = self
            .graph
            .add_node(target_label, None, "system", mode, target_embedding);

        self.graph.add_edge(source_idx, target_idx, relation);
    }

    /// Record user feedback for a specific entity/concept/command
    pub fn record_feedback(&mut self, label: &str, positive: bool) {
        let delta = if positive { 0.1 } else { -0.1 };
        self.graph.update_node_feedback(label, delta);
        // Save graph asynchronously? For now, we rely on periodic saves or manual saves.
        // But let's try to save immediately for persistence.
        let graph_path = self.base_dir.join("knowledge_graph.json");
        let _ = self.graph.save_to_file(graph_path);
    }

    /// Remove a document
    pub fn remove_document(&mut self, id: &str) -> Result<()> {
        if let Some(doc) = self.documents.remove(id) {
            std::fs::remove_file(&doc.path)?;
            self.content_cache.remove(id);
        }
        Ok(())
    }

    /// Get document content (with caching)
    pub fn get_content(&mut self, id: &str) -> Result<Option<String>> {
        // Check cache first
        if let Some(content) = self.content_cache.get(id) {
            return Ok(Some(content.clone()));
        }

        // Load from disk
        if let Some(doc) = self.documents.get(id) {
            let content = std::fs::read_to_string(&doc.path)?;
            self.content_cache.insert(id.to_string(), content.clone());
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// Search documents by query
    pub fn search(&mut self, query: &str, mode: Option<Mode>) -> Vec<ContextSearchResult> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        // Collect document IDs first to avoid borrow checker issues
        let doc_ids: Vec<String> = self
            .documents
            .iter()
            .filter(|(_, doc)| {
                if let Some(m) = mode {
                    doc.context_type.applicable_modes().contains(&m)
                } else {
                    true
                }
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in doc_ids {
            // Get the document info we need before calling get_content
            let doc = self.documents.get(&id).unwrap().clone();
            let name_lower = doc.name.to_lowercase();
            let tags = doc.tags.clone();
            let description = doc.description.clone();

            // Load content
            if let Ok(Some(content)) = self.get_content(&id) {
                let content_lower = content.to_lowercase();

                // Simple relevance scoring
                let mut score: u8 = 0;
                let mut excerpts = Vec::new();

                // Name match (high score)
                if name_lower.contains(&query_lower) {
                    score += 40;
                }

                // Tag match
                if tags.iter().any(|t| t.to_lowercase().contains(&query_lower)) {
                    score += 30;
                }

                // Content match
                if content_lower.contains(&query_lower) {
                    score += 20;

                    // Extract matching context (up to 3 excerpts)
                    for line in content.lines() {
                        if line.to_lowercase().contains(&query_lower) && excerpts.len() < 3 {
                            let excerpt = if line.len() > 200 {
                                format!("{}...", &line[..200])
                            } else {
                                line.to_string()
                            };
                            excerpts.push(excerpt);
                        }
                    }
                }

                // Description match
                if description.to_lowercase().contains(&query_lower) {
                    score += 10;
                }

                if score > 0 {
                    results.push(ContextSearchResult {
                        document: doc,
                        relevance_score: score,
                        excerpts,
                    });
                }
            }
        }

        // --- GRAPH RAG AUGMENTATION ---

        // 0. Vector Search (Semantic) with Graph Expansion
        if let Some(service) = &self.embedding_service {
            if let Ok(query_vec) = service.embed(query) {
                // Search for top 10 semantically similar nodes
                let vec_results = self.graph.vector_search(&query_vec, 10, 0.4); // 0.4 threshold

                for (idx, score) in vec_results {
                    if let Some(node) = self.graph.graph.node_weight(idx) {
                        // Find doc by ID (source_id)
                        if let Some(doc) = self.documents.get(&node.source_id) {
                            // Check mode
                            if let Some(m) = mode {
                                if !doc.context_type.applicable_modes().contains(&m) {
                                    continue;
                                }
                            }

                            // Add or update result
                            let semantic_score = (score * 100.0) as u8;
                            let reason = format!("Semantic Match ({:.2}): {}", score, node.label);

                            self.add_or_boost_result(&mut results, doc, semantic_score, &reason);

                            // --- 1-HOP GRAPH EXPANSION ---
                            // If strong match (> 0.6), look at neighbors
                            if score > 0.6 {
                                let related_nodes = self.graph.get_related_nodes(idx);
                                for (rel_idx, edge_label, role) in related_nodes {
                                    if let Some(rel_node) = self.graph.graph.node_weight(rel_idx) {
                                        if let Some(rel_doc) =
                                            self.documents.get(&rel_node.source_id)
                                        {
                                            // Filter by mode again
                                            if let Some(m) = mode {
                                                if !rel_doc
                                                    .context_type
                                                    .applicable_modes()
                                                    .contains(&m)
                                                {
                                                    continue;
                                                }
                                            }

                                            // Add with slightly lower score but clear reasoning
                                            let rel_score = (score * 80.0) as u8; // 80% of original match
                                            let rel_reason = format!(
                                                "Graph Connection: {} {} {} ({})",
                                                node.label, edge_label, rel_node.label, role
                                            );

                                            self.add_or_boost_result(
                                                &mut results,
                                                rel_doc,
                                                rel_score,
                                                &rel_reason,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 1. Find nodes related to the query terms (Keyword/Graph)
        let mut related_docs = Vec::new();
        // Simple heuristic: treat query as a potential entity label
        let related_entities = self.graph.find_related(query, 2); // 2 hops depth

        for (entity_label, relation_desc) in related_entities {
            // Find documents containing this related entity
            for (_, doc) in self.documents.iter() {
                if let Some(m) = mode {
                    if !doc.context_type.applicable_modes().contains(&m) {
                        continue;
                    }
                }

                if doc.tags.contains(&entity_label) || doc.name.contains(&entity_label) {
                    related_docs.push((doc.clone(), relation_desc.clone()));
                }
            }
        }

        // Add related docs to results if not already present
        for (doc, reason) in related_docs {
            if let Some(existing) = results.iter_mut().find(|r| r.document.id == doc.id) {
                existing.relevance_score = existing.relevance_score.max(50);
                existing
                    .excerpts
                    .push(format!("Graph Connection: {}", reason));
            } else {
                results.push(ContextSearchResult {
                    document: doc,
                    relevance_score: 50, // Arbitrary score for graph connection
                    excerpts: vec![format!("Related via graph: {}", reason)],
                });
            }
        }
        // -----------------------------

        // Sort by relevance
        results.sort_by(|a, b| b.relevance_score.cmp(&a.relevance_score));

        results
    }

    /// Helper to add a result or boost it if it already exists
    fn add_or_boost_result(
        &self,
        results: &mut Vec<ContextSearchResult>,
        doc: &ContextDocument,
        score: u8,
        reason: &str,
    ) {
        if let Some(existing) = results.iter_mut().find(|r| r.document.id == doc.id) {
            // Boost existing score
            existing.relevance_score = existing.relevance_score.max(score).min(100);
            // Avoid duplicate reasons
            if !existing.excerpts.iter().any(|e| e.contains(reason)) {
                existing.excerpts.push(reason.to_string());
            }
        } else {
            results.push(ContextSearchResult {
                document: doc.clone(),
                relevance_score: score,
                excerpts: vec![reason.to_string()],
            });
        }
    }

    /// Get all documents of a specific type
    pub fn get_by_type(&self, context_type: ContextType) -> Vec<&ContextDocument> {
        self.documents
            .values()
            .filter(|doc| doc.context_type == context_type)
            .collect()
    }

    /// Get documents applicable to a mode
    pub fn get_for_mode(&self, mode: Mode) -> Vec<&ContextDocument> {
        self.documents
            .values()
            .filter(|doc| doc.context_type.applicable_modes().contains(&mode))
            .collect()
    }

    /// Format context for AI prompt
    pub fn format_context_for_prompt(&mut self, documents: &[&ContextDocument]) -> Result<String> {
        let mut prompt = String::new();

        // 1. Inject Episodic Memory (Recent Logs)
        // This gives the agent "recent history" context
        let recent_logs = self.get_recent_logs(3)?;
        if !recent_logs.is_empty() {
            prompt.push_str("## 🧠 Recent Memory (Episodic Context)\n");
            prompt.push_str("Here is a summary of recent work and decisions:\n\n");
            prompt.push_str(&recent_logs);
            prompt.push_str("\n---\n\n");
        }

        // 2. Inject Semantic Memory (Documents)
        prompt.push_str("## 📚 Reference Context (Semantic Memory)\n\n");

        for doc in documents {
            prompt.push_str(&format!(
                "### {} ({})",
                doc.name,
                doc.context_type.display_name()
            ));
            if !doc.description.is_empty() {
                prompt.push_str(&format!(" - {}", doc.description));
            }
            prompt.push('\n');

            if let Some(content) = self.get_content(&doc.id)? {
                // Truncate if too long (first 2000 chars)
                let preview = if content.len() > 2000 {
                    format!(
                        "{}\n\n[... {} more characters ...]",
                        &content[..2000],
                        content.len() - 2000
                    )
                } else {
                    content
                };
                prompt.push_str(&preview);
            }

            prompt.push_str("\n\n---\n\n");
        }

        prompt.push_str("You can reference this context in your responses. If the user asks about something covered in these documents, use the information provided.\n");

        Ok(prompt)
    }

    /// Retrieve recent daily logs (Episodic Memory)
    pub fn get_recent_logs(&self, days: usize) -> Result<String> {
        let logs = self.daily_log.list_logs()?;
        let mut recent_content = String::new();

        // Take up to `days` logs
        for path in logs.into_iter().take(days) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let filename = path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("unknown");
                recent_content.push_str(&format!("### Log: {}\n{}\n\n", filename, content));
            }
        }

        Ok(recent_content)
    }

    /// Setup context package based on distribution level
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use agent_host::context_manager::{ContextManager, DistributionLevel};
    ///
    /// // For internal team testing
    /// manager.setup_package(DistributionLevel::Internal)?;
    ///
    /// // For external beta testers (outside team but trusted)
    /// manager.setup_package(DistributionLevel::ExternalBeta)?;
    ///
    /// // For public release
    /// manager.setup_package(DistributionLevel::Public)?;
    /// ```
    pub fn setup_package(&mut self, level: DistributionLevel) -> Result<()> {
        match level {
            DistributionLevel::Internal => self.setup_internal_package(),
            DistributionLevel::ExternalBeta => self.setup_external_beta_package(),
            DistributionLevel::Public => self.setup_public_package(),
        }
    }

    /// Setup for INTERNAL TEAM - Full project context
    ///
    /// Includes all project-specific research, personas, and internal documents.
    /// Use this for your immediate team members.
    fn setup_internal_package(&mut self) -> Result<()> {
        // ========================================================================
        // INTERNAL TEAM DOCUMENTS
        // ========================================================================
        // Full project context including sensitive research and proprietary info.
        // NOT for external distribution.
        // ========================================================================
        let persona_content = r#"# Tech-Savvy Early Adopter

## Profile
- **Role**: Software Developer / Technical Lead
- **Age**: 28-40
- **Tech Comfort**: High
- **Primary Use Case**: Development workflows, automation, productivity

## Goals
- Save time on repetitive tasks
- Stay in flow state while coding
- Reduce context switching
- Automate boring work

## Pain Points
- Too many browser tabs open
- Forget to run common commands
- Hard to find files quickly
- Manual research takes too long

## How Little Helper Helps
- **Fix Mode**: Quick terminal commands, system diagnostics
- **Research Mode**: Fast answers without browser tab explosion
- **Data Mode**: Process CSVs, analyze logs
- **Content Mode**: Write docs, emails, PR descriptions

## Communication Style
- Prefers concise, technical answers
- Wants code examples
- Appreciates keyboard shortcuts
- Likes automation scripts

## Sample Prompts They Might Use
- "Find all Rust files modified today"
- "Check what's using most CPU"
- "Summarize this research paper"
- "Generate a PR description for these changes"
"#;

        self.add_document(
            "Tech Savvy Early Adopter",
            ContextType::Persona,
            persona_content,
            "Primary persona for beta testing with technical coworkers",
            vec![
                "persona".to_string(),
                "beta".to_string(),
                "technical".to_string(),
                "developer".to_string(),
            ],
        )?;

        // Example Research: Little Helper Capabilities
        let research_content = r#"# Little Helper - System Capabilities

## Core Modes

### Fix Mode (🔧)
**Purpose**: Diagnose and fix problems, system administration
**Skills**:
- System diagnostics (CPU, memory, disk health)
- Process monitoring and management
- Startup optimization
- Privacy auditing
- File organization and cleanup
- Error explanation
- Terminal command execution

**Use Cases**:
- "Why is my computer slow?"
- "Check my startup programs"
- "Who can access my camera?"
- "Organize my downloads folder"

### Research Mode (🔬)
**Purpose**: Deep research with citations and sources
**Features**:
- Web search integration
- Document analysis
- Source citation
- Fact verification

**Use Cases**:
- "What are best practices for Rust error handling?"
- "Compare different database options"
- "Find latest news on AI developments"

### Data Mode (📊)
**Purpose**: Work with data files, CSVs, analysis
**Features**:
- CSV/Excel processing
- Data visualization suggestions
- Statistical analysis
- File conversion

**Use Cases**:
- "Analyze this sales data"
- "Convert this JSON to CSV"
- "Plot trends from this dataset"

### Content Mode (✍️)
**Purpose**: Content creation with personas
**Features**:
- Writing assistance
- Persona-based tone adjustment
- Template usage
- Document generation

**Use Cases**:
- "Write a project update email"
- "Generate documentation for this code"
- "Create a presentation outline"

## Safety Features
- Command approval workflow (no auto-execution of dangerous commands)
- Restricted directory access
- Privacy-first (local processing)
- No deletion operations (archive only)

## Device Requirements
- **Local AI**: 8GB+ RAM for small models, 16GB+ for better performance
- **Cloud AI**: Works on any device with API key
- **OS**: macOS, Windows, Linux supported
"#;

        self.add_document(
            "Little Helper Capabilities",
            ContextType::Research,
            research_content,
            "Complete reference for what Little Helper can do",
            vec![
                "research".to_string(),
                "capabilities".to_string(),
                "reference".to_string(),
                "features".to_string(),
            ],
        )?;

        // Example Template: Weekly Status Update
        let template_content = r#"# Template: Weekly Status Update

## Format

### What I Worked On This Week
- [Project/Task 1]: [Brief description]
  - [Specific accomplishment]
  - [Specific accomplishment]
- [Project/Task 2]: [Brief description]

### Key Wins
1. [Notable achievement with impact]
2. [Another achievement]

### Blockers/Issues
- [Issue]: [Status/Help needed]
- [Issue]: [Status/Help needed]

### Next Week Plans
1. [Priority 1]
2. [Priority 2]
3. [Priority 3]

### Needs From Team
- [Specific ask or resource needed]

## Usage Instructions

Fill in the brackets with your specific information. Keep it concise - aim for 2-3 minute read time.

Tone: Professional but friendly, focus on outcomes over activities.
"#;

        self.add_document(
            "Weekly Status Update",
            ContextType::Template,
            template_content,
            "Template for writing weekly status updates",
            vec![
                "template".to_string(),
                "status".to_string(),
                "weekly".to_string(),
                "communication".to_string(),
            ],
        )?;

        // Reference: File Organization Best Practices
        let reference_content = r#"# File Organization Best Practices

## The PARA Method

### Projects
Active projects with a clear goal and deadline
- Current work
- Client deliverables
- Personal goals

### Areas
Ongoing responsibilities without clear end date
- Health
- Finances
- Career development
- Relationships

### Resources
Reference material for future use
- Articles to read
- Templates
- Research papers
- Cheat sheets

### Archives
Completed projects, old reference material
- Past work
- Old versions
- Outdated resources

## Naming Conventions

### Date Format: YYYY-MM-DD
- 2026-01-30_project_proposal.md
- 2026-01-30-meeting-notes.md

### Version Control
- filename_v1.md
- filename_v2.md
- filename_FINAL.md

### Status Prefixes
- DRAFT_
- REVIEW_
- FINAL_
- ARCHIVE_

## Folder Structure Example

```
Documents/
├── 00_Inbox/           # Temporary holding
├── 01_Projects/        # Active projects
├── 02_Areas/          # Ongoing responsibilities
├── 03_Resources/      # Reference material
├── 04_Archives/       # Completed/outdated
└── 99_Templates/      # Reusable templates
```

## Quick Tips

1. **Inbox Zero**: Process inbox daily, move items to appropriate folders
2. **One place**: Each file lives in exactly one location
3. **Search-friendly**: Use descriptive names, include dates
4. **Review weekly**: Archive completed items, update statuses
5. **Backup**: Keep copies in cloud storage (Google Drive, etc.)

## Tools That Help

- **Search**: Everything (Windows), Spotlight (macOS), locate (Linux)
- **Sync**: Google Drive, Dropbox, Nextcloud
- **Tags**: Use OS tags for cross-cutting categories
- **Automation**: Hazel (macOS), AutoHotkey (Windows), cron (Linux)
"#;

        self.add_document(
            "File Organization Guide",
            ContextType::Reference,
            reference_content,
            "Best practices for organizing files and folders",
            vec![
                "reference".to_string(),
                "organization".to_string(),
                "files".to_string(),
                "productivity".to_string(),
            ],
        )?;

        // Skill Guide: Effective Prompting
        let skill_content = r#"# Effective Prompting Guide

## The CO-STAR Framework

### Context (C)
Provide background information
- "I'm working on a Rust project..."
- "This is for a technical audience..."
- "I have 8GB of RAM..."

### Objective (O)
Be specific about what you want
- Weak: "Help with code"
- Strong: "Refactor this function to use Result instead of unwrap"

### Style (S)
Specify the tone and format
- "Explain like I'm 5"
- "Technical documentation style"
- "Bullet points, max 5 items"

### Tone (T)
Set the personality
- "Professional but friendly"
- "Direct and concise"
- "Encouraging and supportive"

### Audience (A)
Who is the output for?
- "Non-technical manager"
- "Senior developer"
- "End user"

### Response (R)
Specify output format
- "Code only, no explanation"
- "Include examples"
- "Markdown table format"

## Example Prompts by Mode

### Fix Mode
Good: "My laptop is slow to boot. Check what apps launch on startup and suggest which ones to disable."
Good: "Find all PDF files in ~/Downloads larger than 10MB and show me what they are"

### Research Mode
Good: "Research the pros and cons of SQLite vs PostgreSQL for a desktop app with 10K users. Cite sources."
Good: "What are the latest developments in Rust web frameworks in 2026?"

### Data Mode
Good: "Analyze this CSV of sales data and tell me the top 3 products by revenue"
Good: "Convert this messy JSON to a clean CSV with columns: name, email, signup_date"

### Content Mode
Good: "Write a project status email to my team lead. Tone: professional. Include: completed tasks, blockers, next steps."
Good: "Generate 5 tweet variations announcing our beta launch. Style: casual tech community."

## Tips for Better Results

1. **Be specific**: Include numbers, dates, file paths
2. **Provide examples**: "Like this: [example]"
3. **Iterate**: Start simple, add constraints based on results
4. **Reference context**: "Use the Tech Savvy Persona from my context"
5. **Specify length**: "In 3 bullet points" or "Under 200 words"
6. **Ask for alternatives**: "Give me 3 different approaches"

## What to Avoid

- **Vague requests**: "Do something with this"
- **Multiple tasks**: Stick to one objective per prompt
- **Assuming knowledge**: Provide necessary context
- **No constraints**: Give boundaries (time, format, style)
"#;

        self.add_document(
            "Effective Prompting Guide",
            ContextType::Skill,
            skill_content,
            "How to write effective prompts for better AI responses",
            vec![
                "skill".to_string(),
                "prompting".to_string(),
                "tips".to_string(),
                "guide".to_string(),
            ],
        )?;

        // Public build: no bundled campaign/research documents.

        Ok(())
    }

    /// Setup for EXTERNAL BETA TESTERS
    ///
    /// Curated project context for trusted testers outside the immediate team.
    /// Includes helpful personas and templates but excludes sensitive project details.
    fn setup_external_beta_package(&mut self) -> Result<()> {
        // ========================================================================
        // EXTERNAL BETA DOCUMENTS
        // ========================================================================
        // Curated selection suitable for external beta testers.
        // Includes personas and templates but excludes proprietary research.
        // ========================================================================

        // Generic persona (non-project-specific)
        let persona_content = r#"# Tech-Savvy Early Adopter

## Profile
- Role: Software Developer / Technical Lead
- Tech Comfort: High
- Primary Use: Development workflows, automation

## Goals
- Save time on repetitive tasks
- Stay in flow state
- Reduce context switching

## Communication Style
- Prefers concise, technical answers
- Wants code examples
- Appreciates keyboard shortcuts
"#;

        self.add_document(
            "Tech Savvy Early Adopter",
            ContextType::Persona,
            persona_content,
            "General persona for technical users",
            vec![
                "persona".to_string(),
                "beta".to_string(),
                "technical".to_string(),
            ],
        )?;

        // Generic capabilities (without internal project details)
        let capabilities_content = r#"# Little Helper - Capabilities Guide

## Core Modes

**Fix Mode**: System diagnostics, troubleshooting, file organization
**Research Mode**: Deep research with web search and citations  
**Data Mode**: CSV analysis, data visualization, file conversion
**Content Mode**: Writing assistance, templates, content creation

## Key Features
- Local AI processing (privacy-first)
- Command approval workflow (safety)
- Multiple AI providers (flexibility)
- Built-in file viewers

## Getting Started
Try: "Find my recent downloads" or "Help me organize this folder"
"#;

        self.add_document(
            "Little Helper Capabilities",
            ContextType::Research,
            capabilities_content,
            "Overview of features and capabilities",
            vec!["reference".to_string(), "capabilities".to_string()],
        )?;

        // Generic templates
        let template_content = r#"# Template: Weekly Status Update

## Format

### What I Worked On
- [Task]: [Description]
  - [Accomplishment]

### Key Wins
1. [Achievement]

### Blockers
- [Issue]: [Status]

### Next Week
1. [Priority]

## Tips
Keep it concise - 2-3 minute read time.
Focus on outcomes, not just activities.
"#;

        self.add_document(
            "Weekly Status Update",
            ContextType::Template,
            template_content,
            "Template for weekly status reports",
            vec![
                "template".to_string(),
                "status".to_string(),
                "weekly".to_string(),
            ],
        )?;

        // Generic skills
        let skill_content = r#"# Effective Prompting Guide

## The CO-STAR Framework
- **Context**: Background information
- **Objective**: What you want
- **Style**: Tone and format
- **Tone**: Personality
- **Audience**: Who it's for
- **Response**: Output format

## Tips
1. Be specific
2. Provide examples
3. Iterate
4. Specify length
5. Ask for alternatives

## Example
Good: "Write a project status email. Tone: professional. Include: completed tasks, blockers, next steps."
"#;

        self.add_document(
            "Effective Prompting Guide",
            ContextType::Skill,
            skill_content,
            "How to write effective prompts",
            vec!["skill".to_string(), "prompting".to_string()],
        )?;

        Ok(())
    }

    /// Setup for PUBLIC RELEASES
    ///
    /// This version includes only generic, non-project-specific documents
    /// suitable for general users. Use this for any public distribution.
    fn setup_public_package(&mut self) -> Result<()> {
        // Generic help documentation suitable for all users
        let prompting_content = r#"# Effective Prompting Guide

## The CO-STAR Framework

### Context (C)
Provide background information
- "I'm working on a Rust project..."
- "This is for a technical audience..."

### Objective (O)
Be specific about what you want
- Weak: "Help with code"
- Strong: "Refactor this function to use Result instead of unwrap"

### Style (S)
Specify the tone and format
- "Explain like I'm 5"
- "Technical documentation style"

### Tone (T)
Set the personality
- "Professional but friendly"
- "Direct and concise"

### Audience (A)
Who is the output for?
- "Non-technical manager"
- "Senior developer"

### Response (R)
Specify output format
- "Code only, no explanation"
- "Include examples"

## Tips for Better Results

1. **Be specific**: Include numbers, dates, file paths
2. **Provide examples**: "Like this: [example]"
3. **Iterate**: Start simple, add constraints
4. **Specify length**: "In 3 bullet points"
5. **Ask for alternatives**: "Give me 3 different approaches"

## What to Avoid

- Vague requests: "Do something with this"
- Multiple tasks per prompt
- Assuming knowledge
- No constraints
"#;

        self.add_document(
            "Effective Prompting Guide",
            ContextType::Skill,
            prompting_content,
            "How to write effective prompts for better AI responses",
            vec![
                "skill".to_string(),
                "prompting".to_string(),
                "tips".to_string(),
            ],
        )?;

        // Generic file organization guide
        let organization_content = r#"# File Organization Best Practices

## The PARA Method

**Projects**: Active work with clear goals
**Areas**: Ongoing responsibilities
**Resources**: Reference material
**Archives**: Completed/outdated items

## Naming Conventions

**Dates**: YYYY-MM-DD-project-name.md
**Versions**: filename_v1.md, filename_v2.md
**Status**: DRAFT_, REVIEW_, FINAL_

## Quick Tips

1. Process inbox daily
2. Each file in exactly one location
3. Use descriptive names
4. Archive completed items weekly
5. Keep backups in cloud storage
"#;

        self.add_document(
            "File Organization Guide",
            ContextType::Reference,
            organization_content,
            "Best practices for organizing files and folders",
            vec!["reference".to_string(), "organization".to_string()],
        )?;

        // Generic personas for public release - useful for any user
        let general_user_persona = r#"# General User Persona

## Profile
- **Role**: Everyday computer user
- **Tech Comfort**: Moderate
- **Primary Use**: Productivity, organization, getting things done

## Goals
- Save time on routine tasks
- Stay organized
- Get quick answers without searching
- Reduce digital clutter

## Communication Preferences
- Clear, straightforward explanations
- Step-by-step instructions when needed
- Practical examples
- No unnecessary jargon

## Common Tasks
- Finding and organizing files
- Writing emails and documents
- Researching topics quickly
- Managing schedules and tasks
- Troubleshooting basic tech issues
"#;

        self.add_document(
            "General User",
            ContextType::Persona,
            general_user_persona,
            "Default persona for everyday productivity tasks",
            vec![
                "persona".to_string(),
                "general".to_string(),
                "default".to_string(),
            ],
        )?;

        let creative_persona = r#"# Creative Professional Persona

## Profile
- **Role**: Writer, designer, marketer, or content creator
- **Tech Comfort**: Moderate to high
- **Primary Use**: Content creation, ideation, editing

## Goals
- Generate creative ideas quickly
- Polish and refine content
- Maintain consistent voice and style
- Meet deadlines efficiently

## Communication Preferences
- Creative, inspiring tone
- Constructive feedback
- Alternative suggestions
- Collaborative brainstorming

## Common Tasks
- Writing articles, emails, social posts
- Brainstorming campaign ideas
- Editing and proofreading
- Creating templates
- Researching trends and topics
"#;

        self.add_document(
            "Creative Professional",
            ContextType::Persona,
            creative_persona,
            "Persona for content creation and creative work",
            vec![
                "persona".to_string(),
                "creative".to_string(),
                "writing".to_string(),
            ],
        )?;

        let tech_persona = r#"# Technical User Persona

## Profile
- **Role**: Developer, IT professional, or tech enthusiast
- **Tech Comfort**: High
- **Primary Use**: Automation, system management, development

## Goals
- Automate repetitive tasks
- Troubleshoot technical issues
- Stay in flow state
- Work efficiently with code and systems

## Communication Preferences
- Concise, technical answers
- Code examples and commands
- Direct solutions
- Keyboard shortcuts and automation

## Common Tasks
- Writing and debugging code
- System administration
- File and data processing
- Researching technical topics
- Creating scripts and tools
"#;

        self.add_document(
            "Technical User",
            ContextType::Persona,
            tech_persona,
            "Persona for technical and development work",
            vec![
                "persona".to_string(),
                "technical".to_string(),
                "developer".to_string(),
            ],
        )?;

        Ok(())
    }

    /// Get all documents
    pub fn all_documents(&self) -> Vec<&ContextDocument> {
        self.documents.values().collect()
    }

    /// Get document count by type
    pub fn count_by_type(&self) -> HashMap<ContextType, usize> {
        let mut counts = HashMap::new();
        for doc in self.documents.values() {
            *counts.entry(doc.context_type).or_insert(0) += 1;
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_context_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ContextManager::new(temp_dir.path().to_path_buf());
        assert!(manager.is_ok());
    }

    #[test]
    fn test_add_and_retrieve_document() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = ContextManager::new(temp_dir.path().to_path_buf()).unwrap();

        let doc = manager
            .add_document(
                "Test Document",
                ContextType::Reference,
                "Test content",
                "Test description",
                vec!["test".to_string()],
            )
            .unwrap();

        assert_eq!(doc.name, "Test Document");
        assert_eq!(doc.context_type, ContextType::Reference);

        let content = manager.get_content(&doc.id).unwrap();
        assert_eq!(content, Some("Test content".to_string()));
    }

    #[test]
    fn test_search_documents() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = ContextManager::new(temp_dir.path().to_path_buf()).unwrap();

        manager
            .add_document(
                "Rust Programming",
                ContextType::Research,
                "Rust is a systems programming language...",
                "About Rust",
                vec!["programming".to_string(), "rust".to_string()],
            )
            .unwrap();

        let results = manager.search("rust", None);
        assert!(!results.is_empty());
        assert_eq!(results[0].document.name, "Rust Programming");
    }
}
