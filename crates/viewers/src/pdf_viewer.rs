//! PDF Viewer - displays PDF info and opens in system viewer
//!
//! Full PDF rendering in egui is complex. This viewer:
//! - Shows file metadata (size, pages if detectable)
//! - Extracts text if possible
//! - Provides button to open in default PDF reader

use anyhow::Result;
use egui::{self, ScrollArea};
use std::fs;
use std::path::{Path, PathBuf};

pub struct PdfViewer {
    path: Option<PathBuf>,
    file_size: u64,
    extracted_text: String,
    error_message: Option<String>,
}

impl Default for PdfViewer {
    fn default() -> Self {
        Self {
            path: None,
            file_size: 0,
            extracted_text: String::new(),
            error_message: None,
        }
    }
}

impl PdfViewer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        self.path = Some(path.to_path_buf());

        // Get file size
        if let Ok(metadata) = fs::metadata(path) {
            self.file_size = metadata.len();
        }

        // Try to extract some text using simple heuristics
        // (Real PDF text extraction needs a proper library)
        self.extracted_text = self.try_extract_text(path);

        Ok(())
    }

    fn try_extract_text(&mut self, path: &Path) -> String {
        // Read raw bytes and look for text streams
        // This is a very basic approach - real PDF needs a proper parser
        match fs::read(path) {
            Ok(bytes) => {
                // Look for PDF version
                let header = String::from_utf8_lossy(&bytes[..std::cmp::min(100, bytes.len())]);
                let mut info = String::new();

                if header.contains("%PDF-") {
                    let version = header.lines().next().unwrap_or("%PDF-?.?").trim();
                    info.push_str(&format!("Format: {}\n", version));
                }

                // Count approximate pages (look for /Page objects)
                let content = String::from_utf8_lossy(&bytes);
                let page_count = content
                    .matches("/Type /Page")
                    .count()
                    .saturating_sub(content.matches("/Type /Pages").count());
                if page_count > 0 {
                    info.push_str(&format!("Estimated pages: ~{}\n", page_count));
                }

                // Try to find title in metadata
                if let Some(title_start) = content.find("/Title") {
                    let title_area =
                        &content[title_start..std::cmp::min(title_start + 200, content.len())];
                    if let Some(paren_start) = title_area.find('(') {
                        if let Some(paren_end) = title_area[paren_start..].find(')') {
                            let title = &title_area[paren_start + 1..paren_start + paren_end];
                            if !title.is_empty() && title.len() < 100 {
                                info.push_str(&format!("Title: {}\n", title));
                            }
                        }
                    }
                }

                info
            }
            Err(e) => {
                self.error_message = Some(format!("Could not read file: {}", e));
                String::new()
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Header
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("PDF Document").strong());
            ui.separator();

            if ui.button("Open in PDF Reader").clicked() {
                if let Some(path) = &self.path {
                    let _ = open::that(path);
                }
            }

            if ui.button("Show in Folder").clicked() {
                if let Some(path) = &self.path {
                    if let Some(parent) = path.parent() {
                        let _ = open::that(parent);
                    }
                }
            }
        });

        ui.separator();

        // File info
        ui.horizontal(|ui| {
            ui.label("Size:");
            ui.label(format_file_size(self.file_size));
        });

        if let Some(error) = &self.error_message {
            ui.colored_label(egui::Color32::RED, error);
        }

        ui.separator();

        // Extracted info
        if !self.extracted_text.is_empty() {
            ui.label(egui::RichText::new("Document Info:").strong());
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(&self.extracted_text);
                });
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label("PDF preview requires opening in your PDF reader.");
                ui.add_space(20.0);
                if ui.button("Open PDF").clicked() {
                    if let Some(path) = &self.path {
                        let _ = open::that(path);
                    }
                }
            });
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_loaded(&self) -> bool {
        self.path.is_some()
    }
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
