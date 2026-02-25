//! Data mode skills for data analysis and visualization.
//!
//! Provides:
//! - CSV file analysis and statistics
//! - Context browsing (personas, research, templates)

pub mod context_browser;
pub mod csv_analyzer;

pub use context_browser::ContextBrowser;
pub use csv_analyzer::CsvAnalyzer;

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all Data mode skills with the registry
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(CsvAnalyzer::new()));
    registry.register(Arc::new(ContextBrowser::default()));
}
