//! File viewers for Little Helper
//!
//! This crate provides viewers for various file types:
//! - Text/Code (syntax highlighted)
//! - Markdown (rendered + source)
//! - HTML (source view + open in browser)
//! - PDF (info + open in reader)
//! - Images (zoom/pan)
//! - CSV/Excel (table view)
//! - JSON (tree view)
//! - SQLite (table browser)

pub mod csv_viewer;
pub mod html_viewer;
pub mod image_viewer;
pub mod json_viewer;
pub mod pdf_viewer;
pub mod text_viewer;

// TODO: Add later
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
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            // Text/Code
            Some(
                "txt" | "rs" | "py" | "js" | "ts" | "sh" | "bash" | "zsh" | "toml" | "yaml" | "yml"
                | "ini" | "cfg" | "conf" | "c" | "cpp" | "h" | "hpp" | "java" | "go" | "rb" | "php",
            ) => FileType::Text,

            // Markdown
            Some("md" | "markdown") => FileType::Markdown,

            // HTML
            Some("html" | "htm" | "xhtml") => FileType::Html,

            // PDF
            Some("pdf") => FileType::Pdf,

            // Images
            Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico") => {
                FileType::Image
            }

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
        matches!(
            self,
            FileType::Text
                | FileType::Markdown
                | FileType::Html
                | FileType::Pdf
                | FileType::Json
                | FileType::Csv
                | FileType::Image
                | FileType::Unknown // Try as text
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

/// Trait for viewers that support zoom functionality
pub trait Zoomable {
    /// Zoom range constants
    const MIN_ZOOM: f32 = 0.25;
    const MAX_ZOOM: f32 = 4.0;
    const ZOOM_STEP: f32 = 0.25;

    /// Set zoom level (will be clamped to valid range)
    fn set_zoom(&mut self, zoom: f32);

    /// Get current zoom level
    fn zoom(&self) -> f32;

    /// Reset zoom to 100%
    fn reset_zoom(&mut self) {
        self.set_zoom(1.0);
    }

    /// Zoom in by one step
    fn zoom_in(&mut self) {
        let new_zoom = (self.zoom() + Self::ZOOM_STEP).min(Self::MAX_ZOOM);
        self.set_zoom(new_zoom);
    }

    /// Zoom out by one step
    fn zoom_out(&mut self) {
        let new_zoom = (self.zoom() - Self::ZOOM_STEP).max(Self::MIN_ZOOM);
        self.set_zoom(new_zoom);
    }

    /// Clamp zoom value to valid range
    fn clamp_zoom(zoom: f32) -> f32 {
        zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM)
    }

    /// Handle zoom input from scroll wheel
    fn handle_zoom_input(&mut self, ui: &egui::Ui) {
        // Check for Ctrl+scroll
        if ui.input(|i| i.modifiers.ctrl) {
            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta.abs() > 0.0 {
                let zoom_delta = scroll_delta * 0.01; // Scale scroll to zoom
                let new_zoom = Self::clamp_zoom(self.zoom() + zoom_delta);
                self.set_zoom(new_zoom);
            }
        }
    }
}
