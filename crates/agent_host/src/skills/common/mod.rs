//! Common skill infrastructure shared across all modes.
//!
//! This module provides foundational components:
//! - SafeFileOps: File operations with NO DELETE policy
//! - AuditLogger: JSON-based audit logging with rotation
//! - VersionHistory: View file version history
//! - VersionRestore: Restore files to previous versions

mod git_helper;
pub use git_helper::GitHelper;

pub mod audit;
pub mod safe_file_ops;
pub mod version_history;
pub mod version_restore;
pub mod write_file;

pub use audit::{AuditLogger, AuditStats};
pub use safe_file_ops::SafeFileOps;
pub use version_history::VersionHistory;
pub use version_restore::VersionRestore;
pub use write_file::WriteFileSkill;

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

    let security_context = Arc::new(SecurityContext::new(15)); // 15 min timeout

    Ok(CommonInfrastructure {
        safe_file_ops: Arc::new(safe_file_ops),
        audit_logger: Arc::new(audit_logger),
        data_dir: data_dir.clone(),
        security_context,
    })
}

/// Common infrastructure shared across all skills.
use crate::security::SecurityContext;

pub struct CommonInfrastructure {
    pub safe_file_ops: Arc<SafeFileOps>,
    pub audit_logger: Arc<AuditLogger>,
    pub data_dir: PathBuf,
    pub security_context: Arc<SecurityContext>,
}

use crate::skills::SkillRegistry;

/// Register common skills available in all modes.
pub fn register_common_skills(
    registry: &mut SkillRegistry,
    infra: &std::sync::Arc<CommonInfrastructure>,
) {
    registry.register(std::sync::Arc::new(VersionHistory::new()));
    registry.register(std::sync::Arc::new(VersionRestore::new()));
    registry.register(std::sync::Arc::new(WriteFileSkill::new(infra.clone())));
    registry.register(std::sync::Arc::new(GitHelper));
}
