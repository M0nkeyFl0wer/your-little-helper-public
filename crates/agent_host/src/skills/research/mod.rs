//! Research mode skills for web research and information synthesis.
//!
//! Provides:
//! - Web search capabilities
//! - Article reading and extraction
//! - Source credibility evaluation
//! - Information synthesis (future)

pub mod article_reader;
pub mod source_evaluator;
pub mod web_search;

pub use article_reader::ArticleReader;
pub use source_evaluator::SourceEvaluator;
pub use web_search::WebSearch;

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all Research mode skills with the registry
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(WebSearch::new()));
    registry.register(Arc::new(ArticleReader::new()));
    registry.register(Arc::new(SourceEvaluator::new()));
}
