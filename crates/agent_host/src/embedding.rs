//! Local vector embedding service for the knowledge graph.
//!
//! Wraps `fastembed`'s all-MiniLM-L6-v2 model (384-dimensional vectors) to
//! generate embeddings for graph nodes and search queries. The model is
//! downloaded once and cached under the system cache directory to avoid
//! polluting the repo working tree.
//!
//! The `Arc<TextEmbedding>` handle is `Send + Sync`, so the service can be
//! shared across async tasks without additional locking.

use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::PathBuf;
use std::sync::Arc;

/// Thin wrapper around `fastembed::TextEmbedding` that manages model
/// caching and exposes single-text and batch embedding methods.
pub struct EmbeddingService {
    model: Arc<TextEmbedding>,
}

impl EmbeddingService {
    /// Load (or download on first run) the embedding model.
    ///
    /// The model weights are cached under `$XDG_CACHE_HOME/little-helper/fastembed`
    /// to keep them out of the project directory.
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("little-helper")
            .join("fastembed");

        let options = InitOptions::new(EmbeddingModel::AllMiniLML6V2)
            .with_cache_dir(cache_dir)
            .with_show_download_progress(false);

        let model = TextEmbedding::try_new(options)?;

        Ok(Self {
            model: Arc::new(model),
        })
    }

    /// Generate a single embedding vector for the given text.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let documents = vec![text];
        let embeddings = self.model.embed(documents, None)?;

        embeddings
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Failed to generate embedding"))
    }

    /// Batch-embed multiple texts in a single model pass for efficiency.
    pub fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let embeddings = self.model.embed(texts, None)?;
        Ok(embeddings)
    }
}
