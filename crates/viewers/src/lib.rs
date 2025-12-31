//! File viewers for Little Helper
//!
//! This crate provides viewers for various file types:
//! - Text/Code (syntax highlighted)
//! - Markdown (rendered + source)
//! - HTML (webview)
//! - PDF (embedded viewer)
//! - Images (zoom/pan)
//! - CSV/Excel (table view)
//! - JSON (tree view)
//! - SQLite (table browser)

pub mod text_viewer;
pub mod image_viewer;
pub mod csv_viewer;
pub mod json_viewer;

// These will be implemented later:
// pub mod pdf_viewer;
// pub mod html_viewer;
// pub mod sqlite_viewer;

use anyhow::Result;
use std::path::Path;

/// Supported file types for viewing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Text,
    Markdown,
    Html,
    Pdf,
    Image,
    Csv,
    Excel,
    Json,
    Sqlite,
    Unknown,
}

impl FileType {
    /// Detect file type from path extension
    pub fn from_path(path: &Path) -> Self {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            // Text/Code
            Some("txt" | "rs" | "py" | "js" | "ts" | "sh" | "bash" | "zsh" |
                 "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" |
                 "c" | "cpp" | "h" | "hpp" | "java" | "go" | "rb" | "php") => FileType::Text,
            
            // Markdown
            Some("md" | "markdown") => FileType::Markdown,
            
            // HTML
            Some("html" | "htm" | "xhtml") => FileType::Html,
            
            // PDF
            Some("pdf") => FileType::Pdf,
            
            // Images
            Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico") => FileType::Image,
            
            // CSV/TSV
            Some("csv" | "tsv") => FileType::Csv,
            
            // Excel
            Some("xls" | "xlsx" | "xlsm") => FileType::Excel,
            
            // JSON
            Some("json" | "jsonl") => FileType::Json,
            
            // SQLite
            Some("db" | "sqlite" | "sqlite3") => FileType::Sqlite,
            
            // Unknown - try to read as text
            _ => FileType::Unknown,
        }
    }

    /// Get human-readable name
    pub fn display_name(&self) -> &'static str {
        match self {
            FileType::Text => "Text",
            FileType::Markdown => "Markdown",
            FileType::Html => "HTML",
            FileType::Pdf => "PDF",
            FileType::Image => "Image",
            FileType::Csv => "CSV",
            FileType::Excel => "Excel",
            FileType::Json => "JSON",
            FileType::Sqlite => "SQLite",
            FileType::Unknown => "Unknown",
        }
    }

    /// Check if we can currently view this type
    pub fn is_supported(&self) -> bool {
        matches!(self, 
            FileType::Text | 
            FileType::Markdown | 
            FileType::Json |
            FileType::Csv |
            FileType::Image |
            FileType::Unknown  // Try as text
        )
    }
}

/// Common trait for all viewers
pub trait Viewer {
    /// Load file content
    fn load(&mut self, path: &Path) -> Result<()>;
    
    /// Render the viewer UI
    fn ui(&mut self, ui: &mut egui::Ui);
    
    /// Get the file path being viewed
    fn path(&self) -> Option<&Path>;
    
    /// Check if content is loaded
    fn is_loaded(&self) -> bool;
}
