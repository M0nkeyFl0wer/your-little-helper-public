use anyhow::{Result, Context};
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use std::sync::Arc;

pub struct EmbeddingService {
    model: Arc<TextEmbedding>,
}

impl EmbeddingService {
    pub fn new() -> Result<Self> {
        let mut options = InitOptions::new(EmbeddingModel::AllMiniLML6V2);
        options.show_download_progress = true;
        
        let model = TextEmbedding::try_new(options)?;

        Ok(Self {
            model: Arc::new(model),
        })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let documents = vec![text];
        let embeddings = self.model.embed(documents, None)?;
        
        embeddings.first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Failed to generate embedding"))
    }
    
    pub fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let embeddings = self.model.embed(texts, None)?;
        Ok(embeddings)
    }
}
