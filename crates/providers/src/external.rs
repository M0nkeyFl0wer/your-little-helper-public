//! External provider registry for skill integrations.
//!
//! Manages external tools like Playwright, Canva, Gemini CLI, and spec-kit.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Provider availability status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderStatus {
    /// Provider is available and ready
    Available,
    /// Provider is unavailable
    Unavailable { reason: String },
    /// Provider needs configuration/setup
    NeedsSetup { instructions: String },
}

impl ProviderStatus {
    pub fn is_available(&self) -> bool {
        matches!(self, ProviderStatus::Available)
    }
}

/// Trait for external providers (Playwright, Canva, Gemini, etc.)
///
/// Note: Uses async_trait for object safety
#[async_trait::async_trait]
pub trait ExternalProvider: Send + Sync {
    /// Provider identifier
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Check if provider is available
    async fn health_check(&self) -> ProviderStatus;

    /// Get setup instructions
    fn setup_instructions(&self) -> &'static str;
}

/// Registry for external providers
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn ExternalProvider>>,
    /// Cached status (refreshed on demand)
    status_cache: HashMap<String, ProviderStatus>,
}

impl ProviderRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            status_cache: HashMap::new(),
        }
    }

    /// Register a provider
    pub fn register(&mut self, provider: Arc<dyn ExternalProvider>) {
        let id = provider.id().to_string();
        self.providers.insert(id, provider);
    }

    /// Get a provider by ID
    pub fn get(&self, id: &str) -> Option<&Arc<dyn ExternalProvider>> {
        self.providers.get(id)
    }

    /// Check provider status (with caching)
    pub async fn check(&mut self, id: &str) -> ProviderStatus {
        if let Some(provider) = self.providers.get(id) {
            let status = provider.health_check().await;
            self.status_cache.insert(id.to_string(), status.clone());
            status
        } else {
            ProviderStatus::Unavailable {
                reason: format!("Provider '{}' not found", id),
            }
        }
    }

    /// Get cached status (may be stale)
    pub fn cached_status(&self, id: &str) -> Option<&ProviderStatus> {
        self.status_cache.get(id)
    }

    /// Check all providers and update cache
    pub async fn check_all(&mut self) -> HashMap<String, ProviderStatus> {
        let mut results = HashMap::new();

        for (id, provider) in &self.providers {
            let status = provider.health_check().await;
            self.status_cache.insert(id.clone(), status.clone());
            results.insert(id.clone(), status);
        }

        results
    }

    /// Get all registered provider IDs
    pub fn provider_ids(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Get provider info for display
    pub fn provider_info(&self, id: &str) -> Option<ProviderInfo> {
        self.providers.get(id).map(|p| ProviderInfo {
            id: p.id(),
            name: p.name(),
            setup_instructions: p.setup_instructions(),
            status: self.status_cache.get(id).cloned(),
        })
    }

    /// Get all providers with their info
    pub fn all_info(&self) -> Vec<ProviderInfo> {
        self.providers
            .iter()
            .map(|(id, p)| ProviderInfo {
                id: p.id(),
                name: p.name(),
                setup_instructions: p.setup_instructions(),
                status: self.status_cache.get(id).cloned(),
            })
            .collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Provider information for display
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub setup_instructions: &'static str,
    pub status: Option<ProviderStatus>,
}

/// Placeholder provider for not-yet-implemented integrations
pub struct PlaceholderProvider {
    id: &'static str,
    name: &'static str,
    instructions: &'static str,
}

impl PlaceholderProvider {
    pub fn new(id: &'static str, name: &'static str, instructions: &'static str) -> Self {
        Self {
            id,
            name,
            instructions,
        }
    }
}

#[async_trait::async_trait]
impl ExternalProvider for PlaceholderProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    async fn health_check(&self) -> ProviderStatus {
        ProviderStatus::NeedsSetup {
            instructions: self.instructions.to_string(),
        }
    }

    fn setup_instructions(&self) -> &'static str {
        self.instructions
    }
}

/// Initialize provider registry with all known providers
pub fn init_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();

    // Register placeholder providers for future implementation
    registry.register(Arc::new(PlaceholderProvider::new(
        "playwright",
        "Playwright Browser Automation",
        "Install with: npx @anthropic/mcp-playwright",
    )));

    registry.register(Arc::new(PlaceholderProvider::new(
        "canva",
        "Canva Design Tools",
        "Configure Canva MCP in settings",
    )));

    registry.register(Arc::new(PlaceholderProvider::new(
        "gemini_cli",
        "Gemini CLI (Nano Banana)",
        "Sign in with Gemini CLI and configure workspace API key",
    )));

    registry.register(Arc::new(PlaceholderProvider::new(
        "speckit",
        "Spec Kit Assistant",
        "Install spec-kit-assistant and set its location in Settings",
    )));

    registry.register(Arc::new(PlaceholderProvider::new(
        "persona",
        "Persona Generation System",
        "Set up your personas folder in Settings",
    )));

    registry.register(Arc::new(PlaceholderProvider::new(
        "content_automation",
        "Content Automation Engine",
        "Optional: connect your content workspace folder in Settings",
    )));

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_registry() {
        let mut registry = ProviderRegistry::new();

        registry.register(Arc::new(PlaceholderProvider::new(
            "test",
            "Test Provider",
            "Test instructions",
        )));

        assert!(registry.get("test").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[tokio::test]
    async fn test_health_check() {
        let mut registry = init_registry();

        let status = registry.check("playwright").await;
        assert!(matches!(status, ProviderStatus::NeedsSetup { .. }));
    }
}
