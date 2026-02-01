//! Common skill infrastructure shared across all modes.
//!
//! This module provides foundational components:
//! - SafeFileOps: File operations with NO DELETE policy
//! - AuditLogger: JSON-based audit logging with rotation
//! - VersionHistory: View file version history
//! - VersionRestore: Restore files to previous versions

pub mod audit;
pub mod safe_file_ops;
pub mod version_history;
pub mod version_restore;

pub use audit::{AuditLogger, AuditStats};
pub use safe_file_ops::SafeFileOps;
pub use version_history::VersionHistory;
pub use version_restore::VersionRestore;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

/// Initialize common skill infrastructure.
///
/// Creates the SafeFileOps and AuditLogger instances with the given data directory.
pub fn init_common_infrastructure(data_dir: &PathBuf) -> Result<CommonInfrastructure> {
    let archive_dir = data_dir.join("archive");
    let log_dir = data_dir.join("audit");

    let safe_file_ops = SafeFileOps::new(archive_dir);
    let audit_logger = AuditLogger::new(log_dir)?;

    Ok(CommonInfrastructure {
        safe_file_ops: Arc::new(safe_file_ops),
        audit_logger: Arc::new(audit_logger),
    })
}

/// Common infrastructure shared across all skills.
pub struct CommonInfrastructure {
    pub safe_file_ops: Arc<SafeFileOps>,
    pub audit_logger: Arc<AuditLogger>,
}

use crate::skills::SkillRegistry;

/// Register common skills available in all modes.
pub fn register_common_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(VersionHistory::new()));
    registry.register(Arc::new(VersionRestore::new()));
}
