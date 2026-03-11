use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::PathBuf;
use std::sync::Arc;

pub struct EmbeddingService {
    model: Arc<TextEmbedding>,
}

impl EmbeddingService {
    pub fn new() -> Result<Self> {
        // Ensure model downloads/caches do NOT land in the repo working tree.
        // fastembed supports an explicit cache directory.
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

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let documents = vec![text];
        let embeddings = self.model.embed(documents, None)?;

        embeddings
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Failed to generate embedding"))
    }

    pub fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let embeddings = self.model.embed(texts, None)?;
        Ok(embeddings)
    }
}
