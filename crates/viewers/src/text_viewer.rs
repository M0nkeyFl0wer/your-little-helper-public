//! Text/Code viewer with optional syntax highlighting

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Text viewer state
pub struct TextViewer {
    path: Option<PathBuf>,
    content: String,
    line_numbers: bool,
    wrap_lines: bool,
    scroll_offset: f32,
}

impl Default for TextViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextViewer {
    pub fn new() -> Self {
        Self {
            path: None,
            content: String::new(),
            line_numbers: true,
            wrap_lines: true,
            scroll_offset: 0.0,
        }
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        self.content = fs::read_to_string(path)?;
        self.path = Some(path.to_path_buf());
        self.scroll_offset = 0.0;
        Ok(())
    }

    pub fn load_string(&mut self, content: String, virtual_path: Option<&str>) {
        self.content = content;
        self.path = virtual_path.map(PathBuf::from);
        self.scroll_offset = 0.0;
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_loaded(&self) -> bool {
        !self.content.is_empty()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Toolbar
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.line_numbers, "Line numbers");
            ui.checkbox(&mut self.wrap_lines, "Wrap lines");

            if let Some(path) = &self.path {
                ui.separator();
                ui.label(format!("{}", path.display()));
            }
        });

        ui.separator();

        // Content area
        let text_style = egui::TextStyle::Monospace;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.line_numbers {
                    self.render_with_line_numbers(ui);
                } else {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.content.as_str())
                            .font(text_style)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                }
            });
    }

    fn render_with_line_numbers(&self, ui: &mut egui::Ui) {
        let lines: Vec<&str> = self.content.lines().collect();
        let line_count = lines.len();
        let gutter_width = format!("{}", line_count).len();

        egui::Grid::new("text_with_lines")
            .num_columns(2)
            .spacing([8.0, 0.0])
            .show(ui, |ui| {
                for (i, line) in lines.iter().enumerate() {
                    // Line number (right-aligned, dimmed)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("{:>width$}", i + 1, width = gutter_width))
                                .monospace()
                                .weak(),
                        );
                    });

                    // Line content
                    ui.label(egui::RichText::new(*line).monospace());
                    ui.end_row();
                }
            });
    }
}
