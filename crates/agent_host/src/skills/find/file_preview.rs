//! File preview skill for Find mode.
//!
//! Provides file content preview and metadata display.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{
    FileAction, FileResult, Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput,
    SkillOutput,
};
use std::fs;
use std::path::PathBuf;

/// Maximum preview size (64KB)
const MAX_PREVIEW_SIZE: usize = 64 * 1024;

/// File preview skill.
///
/// Displays file content preview and metadata.
pub struct FilePreview;

impl FilePreview {
    pub fn new() -> Self {
        Self
    }

    /// Get file metadata as formatted string
    fn format_metadata(&self, path: &PathBuf) -> Result<String> {
        let metadata = fs::metadata(path)?;

        let size = format_size(metadata.len());
        let modified = metadata
            .modified()
            .map(|t| {
                chrono::DateTime::<chrono::Utc>::from(t)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_else(|_| "Unknown".to_string());

        let file_type = if metadata.is_dir() {
            "Directory"
        } else if metadata.is_symlink() {
            "Symlink"
        } else {
            "File"
        };

        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "None".to_string());

        Ok(format!(
            "📄 {}\n\n\
             Type: {}\n\
             Extension: {}\n\
             Size: {}\n\
             Modified: {}\n\
             Path: {}",
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            file_type,
            extension,
            size,
            modified,
            path.display()
        ))
    }

    /// Get file content preview
    fn get_preview(&self, path: &PathBuf) -> Result<Option<String>> {
        let metadata = fs::metadata(path)?;

        // Skip directories
        if metadata.is_dir() {
            return Ok(None);
        }

        // Check if binary
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if is_binary_extension(&extension) {
            return Ok(Some(format!("[Binary file - {} bytes]", metadata.len())));
        }

        // Try to read as text
        if metadata.len() > MAX_PREVIEW_SIZE as u64 {
            // Read first chunk only
            let content = fs::read(path)?;
            let preview = String::from_utf8_lossy(&content[..MAX_PREVIEW_SIZE.min(content.len())]);
            return Ok(Some(format!(
                "{}...\n\n[Truncated - file is {} bytes]",
                preview,
                metadata.len()
            )));
        }

        match fs::read_to_string(path) {
            Ok(content) => Ok(Some(content)),
            Err(_) => Ok(Some("[Unable to read file as text]".to_string())),
        }
    }
}

impl Default for FilePreview {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for FilePreview {
    fn id(&self) -> &'static str {
        "file_preview"
    }

    fn name(&self) -> &'static str {
        "File Preview"
    }

    fn description(&self) -> &'static str {
        "Preview file content and metadata"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Find, Mode::Research, Mode::Data]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        // Get path from params or query
        let path_str = input
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| input.query.trim().to_string());

        if path_str.is_empty() {
            return Ok(SkillOutput::text("Please provide a file path to preview."));
        }

        let path = PathBuf::from(&path_str);

        // Verify path exists
        if !path.exists() {
            return Ok(SkillOutput::text(format!(
                "File not found: {}",
                path.display()
            )));
        }

        // Get metadata
        let metadata_text = self.format_metadata(&path)?;

        // Get preview
        let preview = self.get_preview(&path)?;

        // Build output text
        let text = if let Some(preview_content) = &preview {
            format!(
                "{}\n\n─────────────────────\n\n{}",
                metadata_text, preview_content
            )
        } else {
            metadata_text
        };

        // Build structured data
        let data = serde_json::json!({
            "path": path.to_string_lossy(),
            "name": path.file_name().map(|n| n.to_string_lossy().to_string()),
            "extension": path.extension().map(|e| e.to_string_lossy().to_string()),
            "preview_available": preview.is_some(),
        });

        Ok(SkillOutput {
            result_type: ResultType::Mixed,
            text: Some(text),
            files: vec![FileResult {
                path: path.clone(),
                action: FileAction::Modified, // Just viewing, using Modified as placeholder
                preview,
            }],
            data: Some(data),
            citations: Vec::new(),
            suggested_actions: vec![
                shared::skill::SuggestedAction {
                    label: "Open in default app".to_string(),
                    skill_id: "open_file".to_string(),
                    params: [(
                        "path".to_string(),
                        serde_json::json!(path.to_string_lossy()),
                    )]
                    .into_iter()
                    .collect(),
                },
                shared::skill::SuggestedAction {
                    label: "Search for similar".to_string(),
                    skill_id: "fuzzy_file_search".to_string(),
                    params: [(
                        "query".to_string(),
                        serde_json::json!(path
                            .file_stem()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()),
                    )]
                    .into_iter()
                    .collect(),
                },
            ],
        })
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        let has_path = !input.query.trim().is_empty()
            || input.params.get("path").and_then(|v| v.as_str()).is_some();

        if !has_path {
            anyhow::bail!("Please provide a file path to preview");
        }
        Ok(())
    }
}

/// Format file size in human-readable form
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Check if file extension indicates binary content
fn is_binary_extension(ext: &str) -> bool {
    matches!(
        ext,
        "exe"
            | "dll"
            | "so"
            | "dylib"
            | "bin"
            | "zip"
            | "tar"
            | "gz"
            | "bz2"
            | "xz"
            | "7z"
            | "rar"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "ico"
            | "svg"
            | "mp3"
            | "wav"
            | "ogg"
            | "flac"
            | "m4a"
            | "mp4"
            | "avi"
            | "mkv"
            | "mov"
            | "webm"
            | "pdf"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
            | "sqlite"
            | "db"
            | "mdb"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1_572_864), "1.5 MB");
    }

    #[test]
    fn test_binary_extension() {
        assert!(is_binary_extension("exe"));
        assert!(is_binary_extension("png"));
        assert!(is_binary_extension("pdf"));
        assert!(!is_binary_extension("txt"));
        assert!(!is_binary_extension("rs"));
        assert!(!is_binary_extension("md"));
    }
}
