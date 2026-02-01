//! Drag and drop handler for adding files to context.
//!
//! Uses egui's dropped_files functionality to handle file drops.

use egui::{Context, DroppedFile, Id, Pos2, Rect, Response, Sense, Ui, Vec2};
use std::path::PathBuf;

/// Handler for drag and drop file operations.
pub struct DragDropHandler {
    /// Files that have been dropped
    dropped_files: Vec<PathBuf>,
    /// Whether files are currently being dragged over
    hovering: bool,
    /// ID for the drop zone
    id: Id,
}

impl DragDropHandler {
    /// Create a new drag and drop handler.
    pub fn new(id: impl std::hash::Hash) -> Self {
        Self {
            dropped_files: Vec::new(),
            hovering: false,
            id: Id::new(id),
        }
    }

    /// Process any dropped files from the frame.
    ///
    /// Call this each frame to capture dropped files.
    pub fn update(&mut self, ctx: &Context) {
        // Check for files being dragged over the window
        ctx.input(|i| {
            self.hovering = !i.raw.hovered_files.is_empty();

            // Process dropped files
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    self.dropped_files.push(path.clone());
                }
            }
        });
    }

    /// Take and clear dropped files.
    pub fn take_dropped_files(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.dropped_files)
    }

    /// Get dropped files without clearing.
    pub fn dropped_files(&self) -> &[PathBuf] {
        &self.dropped_files
    }

    /// Check if files are currently being dragged over the window.
    pub fn is_hovering(&self) -> bool {
        self.hovering
    }

    /// Check if there are any dropped files waiting to be processed.
    pub fn has_dropped_files(&self) -> bool {
        !self.dropped_files.is_empty()
    }

    /// Clear dropped files without returning them.
    pub fn clear(&mut self) {
        self.dropped_files.clear();
    }

    /// Show a drop zone UI element.
    ///
    /// Returns a response that can be used to detect interactions.
    pub fn show_drop_zone(&mut self, ui: &mut Ui, size: Vec2, label: &str) -> Response {
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

        // Visual style changes when hovering with files
        let visuals = if self.hovering {
            ui.visuals().widgets.hovered
        } else {
            ui.visuals().widgets.inactive
        };

        // Draw the drop zone
        ui.painter().rect(
            rect,
            4.0,
            if self.hovering {
                visuals.bg_fill.gamma_multiply(1.2)
            } else {
                visuals.bg_fill
            },
            visuals.bg_stroke,
        );

        // Draw dashed border when hovering
        if self.hovering {
            let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
            ui.painter().rect_stroke(rect, 4.0, stroke);
        }

        // Draw label
        let text_pos = rect.center();
        ui.painter().text(
            text_pos,
            egui::Align2::CENTER_CENTER,
            if self.hovering {
                "📥 Drop files here"
            } else {
                label
            },
            egui::FontId::proportional(14.0),
            if self.hovering {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().text_color()
            },
        );

        response
    }

    /// Show a compact drop zone (inline with other UI).
    pub fn show_compact_drop_zone(&mut self, ui: &mut Ui) -> Response {
        let height = 40.0;
        let available_width = ui.available_width();
        self.show_drop_zone(ui, Vec2::new(available_width, height), "📎 Drop files or click to add")
    }

    /// Show an overlay when files are being dragged over the entire window.
    pub fn show_drag_overlay(&self, ctx: &Context) {
        if !self.hovering {
            return;
        }

        egui::Area::new(self.id.with("overlay"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();

                // Semi-transparent overlay
                ui.painter().rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::from_black_alpha(100),
                );

                // Centered drop indicator
                let indicator_size = Vec2::new(300.0, 150.0);
                let indicator_rect = Rect::from_center_size(screen_rect.center(), indicator_size);

                ui.painter().rect(
                    indicator_rect,
                    8.0,
                    ui.visuals().extreme_bg_color,
                    egui::Stroke::new(3.0, ui.visuals().selection.bg_fill),
                );

                ui.painter().text(
                    indicator_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "📥 Drop files to add to context",
                    egui::FontId::proportional(18.0),
                    ui.visuals().strong_text_color(),
                );
            });
    }
}

/// Extension trait for processing dropped files.
pub trait DroppedFileExt {
    /// Get the file path if available.
    fn path(&self) -> Option<&PathBuf>;

    /// Get the file name.
    fn name(&self) -> &str;

    /// Check if this is an image file (by extension).
    fn is_image(&self) -> bool;

    /// Check if this is a text file (by extension).
    fn is_text(&self) -> bool;
}

impl DroppedFileExt for DroppedFile {
    fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn is_image(&self) -> bool {
        let name = self.name.to_lowercase();
        name.ends_with(".png")
            || name.ends_with(".jpg")
            || name.ends_with(".jpeg")
            || name.ends_with(".gif")
            || name.ends_with(".webp")
            || name.ends_with(".svg")
    }

    fn is_text(&self) -> bool {
        let name = self.name.to_lowercase();
        name.ends_with(".txt")
            || name.ends_with(".md")
            || name.ends_with(".json")
            || name.ends_with(".yaml")
            || name.ends_with(".yml")
            || name.ends_with(".toml")
            || name.ends_with(".rs")
            || name.ends_with(".py")
            || name.ends_with(".js")
            || name.ends_with(".ts")
            || name.ends_with(".html")
            || name.ends_with(".css")
    }
}

/// Helper to filter dropped files by type.
pub fn filter_by_extension<'a>(
    files: &'a [PathBuf],
    extensions: &[&str],
) -> impl Iterator<Item = &'a PathBuf> {
    files.iter().filter(move |path| {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
            .unwrap_or(false)
    })
}

/// Get human-readable file type.
pub fn file_type_label(path: &PathBuf) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => "Image",
        "pdf" => "PDF",
        "txt" | "md" => "Text",
        "json" | "yaml" | "yml" | "toml" => "Config",
        "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" => "Code",
        "csv" | "xls" | "xlsx" => "Spreadsheet",
        "doc" | "docx" => "Document",
        _ => "File",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drag_drop_handler_creation() {
        let handler = DragDropHandler::new("test");
        assert!(!handler.is_hovering());
        assert!(!handler.has_dropped_files());
    }

    #[test]
    fn test_filter_by_extension() {
        let files = vec![
            PathBuf::from("test.txt"),
            PathBuf::from("image.png"),
            PathBuf::from("code.rs"),
            PathBuf::from("data.json"),
        ];

        let text_files: Vec<_> = filter_by_extension(&files, &["txt", "md"]).collect();
        assert_eq!(text_files.len(), 1);
        assert_eq!(text_files[0].file_name().unwrap(), "test.txt");

        let code_files: Vec<_> = filter_by_extension(&files, &["rs", "py", "json"]).collect();
        assert_eq!(code_files.len(), 2);
    }

    #[test]
    fn test_file_type_label() {
        assert_eq!(file_type_label(&PathBuf::from("test.rs")), "Code");
        assert_eq!(file_type_label(&PathBuf::from("image.png")), "Image");
        assert_eq!(file_type_label(&PathBuf::from("data.csv")), "Spreadsheet");
    }
}
