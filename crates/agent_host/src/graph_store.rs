//! Graph Store for RAG
//! 
//! Implements a Knowledge Graph using `petgraph` to store entities and their relationships.
//! This allows for "multi-hop" reasoning where the agent can find connections between
//! concepts that aren't continuously mentioned in the same text chunk.

use anyhow::Result;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::Bfs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use strsim::jaro_winkler;
use std::time::{SystemTime, UNIX_EPOCH};

/// Operational Mode for the Agent
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Mode {
    Find,
    Fix,
    Research,
    Build, // Renamed from "Data" in some contexts, but let's stick to the app's modes
    Data,
    Content,
    General,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::General
    }
}

/// A node in the knowledge graph representing an entity or concept
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeData {
    /// Unique label/name of the entity (e.g., "Alice", "Project X")
    pub label: String,
    /// Type of entity (e.g., "Person", "Project", "Technology", "Feedback", "Preference")
    pub category: Option<String>,
    /// Source document ID where this was found
    pub source_id: String,
    /// The mode this knowledge is most relevant to (Specialization)
    #[serde(default)]
    pub mode: Mode,
    /// Usage count (for reinforcement learning)
    #[serde(default)]
    pub usage_count: u32,
    /// User feedback score (-1 to 1)
    #[serde(default)]
    pub feedback_score: f32,
    /// Last accessed timestamp (Unix seconds)
    #[serde(default = "default_timestamp")]
    pub last_accessed: u64,
    /// Vector embedding for semantic search
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

fn default_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// An edge representing a relationship between two nodes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeData {
    /// Description of the relationship (e.g., "works on", "depends on")
    pub relation: String,
    /// Confidence score (0.0 - 1.0)
    pub weight: f32,
}

/// Serializable graph structure
#[derive(Debug, Serialize, Deserialize)]
pub struct GraphStore {
    /// Underlying graph: Nodes are entities, Edges are relationships
    pub graph: DiGraph<NodeData, EdgeData>,
    /// Lookup map for quick node access by label
    node_map: HashMap<String, NodeIndex>,
}

impl GraphStore {
    /// Create a new, empty graph
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }

    /// Add a node (entity) to the graph. Returns the NodeIndex.
    /// Idempotent: if node exists, returns existing index.
    pub fn add_node(&mut self, label: &str, category: Option<String>, source_id: &str, mode: Mode, embedding: Option<Vec<f32>>) -> NodeIndex {
        if let Some(&idx) = self.node_map.get(label) {
            // Update embedding if missing and provided
            if let Some(node) = self.graph.node_weight_mut(idx) {
                if node.embedding.is_none() && embedding.is_some() {
                    node.embedding = embedding;
                }
            }
            return idx;
        }

        let node = NodeData {
            label: label.to_string(),
            category,
            source_id: source_id.to_string(),
            mode,
            usage_count: 0,
            feedback_score: 0.0,
            last_accessed: default_timestamp(),
            embedding,
        };

        let idx = self.graph.add_node(node);
        self.node_map.insert(label.to_string(), idx);
        idx
    }

    /// Check if a node exists and has an embedding
    pub fn has_embedding(&self, label: &str) -> bool {
        if let Some(&idx) = self.node_map.get(label) {
            if let Some(node) = self.graph.node_weight(idx) {
                return node.embedding.is_some();
            }
        }
        false
    }

    /// Add a directed edge (relationship) between two nodes
    pub fn add_edge(&mut self, source: NodeIndex, target: NodeIndex, relation: &str) {
        // Check if edge already exists to avoid duplicates
        if let Some(_edge) = self.graph.find_edge(source, target) {
            // Update existing edge if needed, for now we just return
            return;
        }

        let edge_data = EdgeData {
            relation: relation.to_string(),
            weight: 1.0,
        };
        self.graph.add_edge(source, target, edge_data);
    }

    /// Find related nodes up to `depth` hops away
    pub fn find_related(&self, start_label: &str, max_depth: usize) -> Vec<(String, String)> {
        let mut related = Vec::new();
        
        if let Some(&start_idx) = self.node_map.get(start_label) {
            let mut bfs = Bfs::new(&self.graph, start_idx);
            let mut depth_map = HashMap::new();
            depth_map.insert(start_idx, 0);

            while let Some(nx) = bfs.next(&self.graph) {
                let current_depth = *depth_map.get(&nx).unwrap_or(&0);
                
                if current_depth >= max_depth {
                   continue; 
                }

                // Look at neighbors
                for neighbor in self.graph.neighbors(nx) {
                    if !depth_map.contains_key(&neighbor) {
                        depth_map.insert(neighbor, current_depth + 1);
                        
                        // Record relationship
                        if let Some(edge_idx) = self.graph.find_edge(nx, neighbor) {
                            if let Some(edge_weight) = self.graph.edge_weight(edge_idx) {
                                if let Some(target_node) = self.graph.node_weight(neighbor) {
                                    if let Some(source_node) = self.graph.node_weight(nx) {
                                         related.push((
                                            target_node.label.clone(),
                                            format!("{} {} {}", source_node.label, edge_weight.relation, target_node.label)
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        related
    }

    /// Get usage-based neighbors (1-hop) for a node index
    /// Returns (NodeIndex, Relation String, Role [Outgoing/Incoming])
    pub fn get_related_nodes(&self, idx: NodeIndex) -> Vec<(NodeIndex, String, String)> {
        let mut related = Vec::new();
        
        // Outgoing
        for edge in self.graph.edges(idx) {
            use petgraph::visit::EdgeRef;
            related.push((edge.target(), edge.weight().relation.clone(), "related to".to_string()));
        }
        
        // Incoming
        for neighbor in self.graph.neighbors_directed(idx, petgraph::Direction::Incoming) {
             if let Some(edge_idx) = self.graph.find_edge(neighbor, idx) {
                 if let Some(weight) = self.graph.edge_weight(edge_idx) {
                     related.push((neighbor, weight.relation.clone(), "referenced by".to_string()));
                 }
             }
        }
        
        related
    }

    /// Update feedback for a node
    pub fn update_node_feedback(&mut self, label: &str, score_delta: f32) {
        if let Some(&idx) = self.node_map.get(label) {
            if let Some(node) = self.graph.node_weight_mut(idx) {
                node.feedback_score += score_delta;
                node.usage_count += 1;
                // Clamp score between -1.0 and 1.0
                node.feedback_score = node.feedback_score.clamp(-1.0, 1.0);
            }
        }
    }

    /// Consolidate nodes with similar labels
    /// Returns the number of nodes merged
    pub fn consolidate_nodes(&mut self, threshold: f64) -> usize {
        let mut merged_count = 0;
        let mut nodes_to_remove = Vec::new();
        let mut edges_to_add = Vec::new();

        // 1. Identify merge candidates
        // We collect indices to avoid borrowing issues
        let node_indices: Vec<NodeIndex> = self.graph.node_indices().collect();
        
        // Naive O(N^2) - acceptable for current scale
        // In the future, we can use an accumulation index or LSH
        let mut processed = std::collections::HashSet::new();

        for &i in &node_indices {
            if processed.contains(&i) { continue; }
            if nodes_to_remove.contains(&i) { continue; }

            let label_i = self.graph[i].label.clone();
            
            for &j in &node_indices {
                if i == j { continue; }
                if processed.contains(&j) { continue; }
                if nodes_to_remove.contains(&j) { continue; }

                let label_j = &self.graph[j].label;
                
                // Check similarity
                // Normalize slightly by lowercasing for the check
                let sim = jaro_winkler(&label_i.to_lowercase(), &label_j.to_lowercase());
                
                if sim >= threshold {
                    // Merge j into i (keep i as canonical)
                    // Logic: Keep the one with higher usage, or if equal, keep i
                    let (keep, discard) = if self.graph[j].usage_count > self.graph[i].usage_count {
                        (j, i)
                    } else {
                        (i, j)
                    };

                    // Mark for removal
                    if !nodes_to_remove.contains(&discard) {
                         nodes_to_remove.push(discard);
                         
                         // Collect usage stats to merge later (partially done here)
                         self.graph[keep].usage_count += self.graph[discard].usage_count;
                         // Average feedback? Or just sum? Let's take the max for now or weighted avg
                         // Simple approach: keep the 'keep' score but nudge it if 'discard' was good
                         if self.graph[discard].feedback_score > 0.0 {
                             self.graph[keep].feedback_score += 0.1;
                         }
                         
                         // Collect edges to remap
                         // Outgoing from discard -> target
                         for edge in self.graph.edges(discard) {
                             use petgraph::visit::EdgeRef;
                             edges_to_add.push((keep, edge.target(), edge.weight().clone()));
                         }
                         
                         // Incoming from source -> discard
                         // petgraph `edges` is outgoing by default. Walker needed for incoming or `edges_directed`
                         // For DiGraph, we can use `neighbors_directed`
                         let mut incoming = Vec::new();
                         for neighbor in self.graph.neighbors_directed(discard, petgraph::Direction::Incoming) {
                             let edge_idx = self.graph.find_edge(neighbor, discard).unwrap();
                             let weight = self.graph.edge_weight(edge_idx).unwrap().clone();
                             incoming.push((neighbor, keep, weight));
                         }
                         edges_to_add.extend(incoming);
                    }
                }
            }
            processed.insert(i);
        }

        // 2. Apply changes
        // Add new edges
        for (source, target, weight) in edges_to_add {
            // Avoid self-loops if consolidation caused them (rare but possible)
            if source != target {
                // Check if edge exists
                if self.graph.find_edge(source, target).is_none() {
                     self.graph.add_edge(source, target, weight);
                }
            }
        }

        // Remove nodes
        // We must remove them in reverse index order or be careful, 
        // essentially `remove_node` invalidates last index if it swaps.
        // But `petgraph`'s `remove_node` swaps with the last node to be O(1).
        // This changes indices!
        // So we cannot just use the collected NodeIndices safely if we do one by one.
        // Instead, we should filter `retain_nodes`.
        // However, `retain_nodes` predicate doesn't let us merge stats easily.
        
        // Alternative: Re-build map.
        // Actually, `remove_node` returns the defined node data.
        
        // Valid strategy with swap-remove:
        // Identify nodes by ID/Label? No, just be careful.
        // Easiest robust way: 
        // 1. Mark nodes as "tombstoned" (e.g. modify label to "DELETED_")
        // 2. Then `retain_nodes` to remove all "DELETED_"
        
        for idx in &nodes_to_remove {
            // We can't trust the index anymore if we removed previous ones?
            // Actually `petgraph` changes indices.
            // So we should have mapped labels to remove.
            if let Some(node) = self.graph.node_weight_mut(*idx) {
               node.label = format!("__DELETED__{}", node.label); 
            }
        }
        
        self.graph.retain_nodes(|g, ix| {
             !g[ix].label.starts_with("__DELETED__")
        });
        
        // Rebuild map because indices changed
        self.node_map.clear();
        for ix in self.graph.node_indices() {
            let label = self.graph[ix].label.clone();
            self.node_map.insert(label, ix);
        }

        merged_count = nodes_to_remove.len();
        merged_count
    }

    /// Prune nodes that are low quality or unused and old
    /// Returns number of nodes removed.
    pub fn prune_nodes(&mut self, min_feedback: f32, max_unused_days: u64) -> usize {
        let now = default_timestamp();
        let seconds_threshold = max_unused_days * 24 * 3600;
        
        let before_count = self.graph.node_count();

        self.graph.retain_nodes(|g, ix| {
            let node = &g[ix];
            
            // 1. Explicitly bad feedback
            if node.feedback_score < min_feedback {
                return false;
            }
            
            // 2. Unused and Old
            let age_seconds = now.saturating_sub(node.last_accessed);
            if node.usage_count == 0 && age_seconds > seconds_threshold {
                return false;
            }
            
            true
        });
        
        // Rebuild map
        if self.graph.node_count() != before_count {
            self.node_map.clear();
            for ix in self.graph.node_indices() {
                let label = self.graph[ix].label.clone();
                self.node_map.insert(label, ix);
            }
        }
        
        before_count - self.graph.node_count()
    }

    /// Serialize graph to JSON
    pub fn save_to_file(&self, path: PathBuf) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Load graph from JSON
    pub fn load_from_file(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let json = fs::read_to_string(path)?;
        let store: Self = serde_json::from_str(&json)?;
        Ok(store)
    }
    /// Perform a vector search for similar nodes
    pub fn vector_search(&self, query_vec: &[f32], limit: usize, min_score: f32) -> Vec<(NodeIndex, f32)> {
        let mut results = Vec::new();

        for idx in self.graph.node_indices() {
             if let Some(embedding) = &self.graph[idx].embedding {
                let score = cosine_similarity(query_vec, embedding);
                if score >= min_score {
                    results.push((idx, score));
                }
             }
        }

        // Sort by score descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if results.len() > limit {
            results.truncate(limit);
        }
        results
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consolidate_nodes() {
        let mut store = GraphStore::new();
        
        // Add "Apple" and "apple " (fuzzy match)
        let n1 = store.add_node("Apple", None, "test", Mode::General, None);
        let n2 = store.add_node("apple ", None, "test", Mode::General, None);
        
        // Add distinct node
        let n3 = store.add_node("Banana", None, "test", Mode::General, None);

        // Usage stats to influence merge
        store.graph[n1].usage_count = 10;
        store.graph[n2].usage_count = 5;

        // Consolidate
        let merged = store.consolidate_nodes(0.85); // High threshold
        
        assert_eq!(merged, 1);
        assert_eq!(store.graph.node_count(), 2); // Apple, Banana
        
        // Check identifying the survivor
        let idx = *store.node_map.get("Apple").unwrap();
        assert_eq!(store.graph[idx].usage_count, 15); // Summed
    }

    #[test]
    fn test_prune_nodes() {
        let mut store = GraphStore::new();
        
        let n1 = store.add_node("GoodNode", None, "test", Mode::General, None);
        store.graph[n1].feedback_score = 0.5;
        store.graph[n1].last_accessed = default_timestamp(); // Just now
        
        let n2 = store.add_node("BadNode", None, "test", Mode::General, None);
        store.graph[n2].feedback_score = -0.9; // Hated
        
        let n3 = store.add_node("OldNode", None, "test", Mode::General, None);
        store.graph[n3].usage_count = 0;
        store.graph[n3].last_accessed = default_timestamp() - (40 * 24 * 3600); // 40 days old
        
        // Prune
        let removed = store.prune_nodes(-0.5, 30);
        
        assert_eq!(removed, 2);
        assert!(store.node_map.contains_key("GoodNode"));
        assert!(!store.node_map.contains_key("BadNode"));
        assert!(!store.node_map.contains_key("OldNode"));
    }
    
    #[test]
    fn test_vector_search() {
        let mut store = GraphStore::new();
        
        // A simple test with dummy vectors
        // [1.0, 0.0] vs [0.0, 1.0] -> 0.0 similarity
        // [1.0, 0.0] vs [0.9, 0.1] -> high similarity
        
        // Node 1: "X-Axis"
        let v1 = vec![1.0, 0.0];
        let n1 = store.add_node("X-Axis", None, "math", Mode::General, Some(v1));
        
        // Node 2: "Y-Axis"
        let v2 = vec![0.0, 1.0];
        let n2 = store.add_node("Y-Axis", None, "math", Mode::General, Some(v2));
        
        // Node 3: "Near X"
        let v3 = vec![0.9, 0.1];
        let n3 = store.add_node("Near X", None, "math", Mode::General, Some(v3));

        // Search for something close to X-Axis
        let query = vec![1.0, 0.0];
        let results = store.vector_search(&query, 5, 0.5);
        
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, n1); // Exact match first
        assert_eq!(results[1].0, n3); // Near match second
        // Y-Axis should be excluded (sim 0.0 < 0.5)
    }

    #[test]
    fn test_get_related_nodes() {
        let mut store = GraphStore::new();
        
        // Create nodes
        let n_center = store.add_node("Center", None, "test", Mode::General, None);
        let n_out = store.add_node("Target", None, "test", Mode::General, None);
        let n_in = store.add_node("Source", None, "test", Mode::General, None);
        
        // Add edges
        store.add_edge(n_center, n_out, "defines"); // Center -> Target
        store.add_edge(n_in, n_center, "depends_on"); // Source -> Center
        
        // Get related
        let related = store.get_related_nodes(n_center);
        
        assert_eq!(related.len(), 2);
        
        // Check Outgoing
        assert!(related.iter().any(|(idx, rel, role)| 
            *idx == n_out && rel == "defines" && role == "related to"
        ));
        
        // Check Incoming
        assert!(related.iter().any(|(idx, rel, role)| 
            *idx == n_in && rel == "depends_on" && role == "referenced by"
        ));
    }
}
