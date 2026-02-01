//! Data mode skills for data analysis and visualization.
//!
//! Provides:
//! - CSV file analysis
//! - Data profiling and statistics
//! - Chart type recommendations
//! - Context browsing (personas, research, templates)
//! - Data cleaning helpers (future)

pub mod chart_recommender;
pub mod context_browser;
pub mod csv_analyzer;

pub use chart_recommender::ChartRecommender;
pub use context_browser::ContextBrowser;
pub use csv_analyzer::CsvAnalyzer;

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all Data mode skills with the registry
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(CsvAnalyzer::new()));
    registry.register(Arc::new(ChartRecommender::new()));
    registry.register(Arc::new(ContextBrowser::default()));
}
