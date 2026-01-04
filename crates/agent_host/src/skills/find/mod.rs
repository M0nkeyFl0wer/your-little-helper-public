//! Find mode skills for file search and navigation.
//!
//! Provides:
//! - Fuzzy file search across indexed drives
//! - Drive/directory indexing
//! - File preview with metadata
//! - Safe file organization (NO DELETE policy)

pub mod drive_index;
pub mod file_organize;
pub mod file_preview;
pub mod fuzzy_search;

pub use drive_index::{default_index_paths, DriveIndex};
pub use file_organize::FileOrganize;
pub use file_preview::FilePreview;
pub use fuzzy_search::FuzzyFileSearch;

use crate::skills::SkillRegistry;
use services::file_index::FileIndexService;
use std::path::PathBuf;
use std::sync::Arc;

/// Register all Find mode skills with the registry
pub fn register_skills(registry: &mut SkillRegistry, file_index: Arc<FileIndexService>) {
    registry.register(Arc::new(FuzzyFileSearch::new(file_index.clone())));
    registry.register(Arc::new(DriveIndex::new(file_index)));
    registry.register(Arc::new(FilePreview::new()));

    // File organization with archive directory
    let archive_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("little-helper")
        .join("archive");
    registry.register(Arc::new(FileOrganize::new(archive_dir)));
}
