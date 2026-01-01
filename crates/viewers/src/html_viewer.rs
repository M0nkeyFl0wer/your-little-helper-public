//! HTML Viewer - displays HTML with option to open in browser

use anyhow::Result;
use egui::{self, ScrollArea};
use std::path::{Path, PathBuf};

pub struct HtmlViewer {
    path: Option<PathBuf>,
    content: String,
    show_source: bool,
}

impl Default for HtmlViewer {
    fn default() -> Self {
        Self {
            path: None,
            content: String::new(),
            show_source: true,
        }
    }
}

impl HtmlViewer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        self.content = std::fs::read_to_string(path)?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Toolbar
        ui.horizontal(|ui| {
            ui.label("HTML Preview");
            ui.separator();

            if ui.button("Open in Browser").clicked() {
                if let Some(path) = &self.path {
                    let _ = open::that(path);
                }
            }

            ui.checkbox(&mut self.show_source, "Show Source");
        });

        ui.separator();

        if self.show_source {
            // Show HTML source with basic syntax highlighting
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // Simple syntax highlighting for HTML
                    let mut job = egui::text::LayoutJob::default();

                    let tag_color = egui::Color32::from_rgb(86, 156, 214); // Blue for tags
                    let attr_color = egui::Color32::from_rgb(156, 220, 254); // Light blue for attrs
                    let string_color = egui::Color32::from_rgb(206, 145, 120); // Orange for strings
                    let text_color = ui.visuals().text_color();

                    let mut in_tag = false;
                    let mut in_string = false;
                    let mut current_chunk = String::new();

                    for ch in self.content.chars() {
                        match ch {
                            '<' => {
                                if !current_chunk.is_empty() {
                                    job.append(
                                        &current_chunk,
                                        0.0,
                                        egui::TextFormat {
                                            color: if in_string {
                                                string_color
                                            } else {
                                                text_color
                                            },
                                            ..Default::default()
                                        },
                                    );
                                    current_chunk.clear();
                                }
                                in_tag = true;
                                current_chunk.push(ch);
                            }
                            '>' => {
                                current_chunk.push(ch);
                                job.append(
                                    &current_chunk,
                                    0.0,
                                    egui::TextFormat {
                                        color: tag_color,
                                        ..Default::default()
                                    },
                                );
                                current_chunk.clear();
                                in_tag = false;
                            }
                            '"' if in_tag => {
                                if in_string {
                                    current_chunk.push(ch);
                                    job.append(
                                        &current_chunk,
                                        0.0,
                                        egui::TextFormat {
                                            color: string_color,
                                            ..Default::default()
                                        },
                                    );
                                    current_chunk.clear();
                                    in_string = false;
                                } else {
                                    if !current_chunk.is_empty() {
                                        job.append(
                                            &current_chunk,
                                            0.0,
                                            egui::TextFormat {
                                                color: attr_color,
                                                ..Default::default()
                                            },
                                        );
                                        current_chunk.clear();
                                    }
                                    in_string = true;
                                    current_chunk.push(ch);
                                }
                            }
                            _ => {
                                current_chunk.push(ch);
                            }
                        }
                    }

                    // Remaining text
                    if !current_chunk.is_empty() {
                        job.append(
                            &current_chunk,
                            0.0,
                            egui::TextFormat {
                                color: text_color,
                                ..Default::default()
                            },
                        );
                    }

                    ui.label(job);
                });
        } else {
            // Show a simple text extraction (strip tags)
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let text = strip_html_tags(&self.content);
                    ui.label(&text);
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

/// Simple HTML tag stripper for text preview
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        if !in_tag && chars[i] == '<' {
            // Check for script/style start
            let remaining: String = lower_chars[i..].iter().collect();
            if remaining.starts_with("<script") {
                in_script = true;
            } else if remaining.starts_with("<style") {
                in_style = true;
            } else if remaining.starts_with("</script") {
                in_script = false;
            } else if remaining.starts_with("</style") {
                in_style = false;
            }
            in_tag = true;
        } else if in_tag && chars[i] == '>' {
            in_tag = false;
            // Add space after block elements
            let remaining: String = lower_chars[i.saturating_sub(10)..=i].iter().collect();
            if remaining.contains("/p>")
                || remaining.contains("/div")
                || remaining.contains("/h")
                || remaining.contains("/li")
                || remaining.contains("<br")
            {
                result.push('\n');
            }
        } else if !in_tag && !in_script && !in_style {
            // Decode common entities
            if chars[i] == '&' {
                let remaining: String = chars[i..].iter().take(10).collect();
                if remaining.starts_with("&nbsp;") {
                    result.push(' ');
                    i += 5;
                } else if remaining.starts_with("&amp;") {
                    result.push('&');
                    i += 4;
                } else if remaining.starts_with("&lt;") {
                    result.push('<');
                    i += 3;
                } else if remaining.starts_with("&gt;") {
                    result.push('>');
                    i += 3;
                } else if remaining.starts_with("&quot;") {
                    result.push('"');
                    i += 5;
                } else {
                    result.push(chars[i]);
                }
            } else {
                result.push(chars[i]);
            }
        }
        i += 1;
    }

    // Clean up whitespace
    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
