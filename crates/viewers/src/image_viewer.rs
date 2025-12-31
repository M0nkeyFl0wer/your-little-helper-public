//! Image viewer with zoom and pan

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Image viewer state
pub struct ImageViewer {
    path: Option<PathBuf>,
    texture: Option<egui::TextureHandle>,
    image_size: Option<[usize; 2]>,
    zoom: f32,
    pan_offset: egui::Vec2,
    fit_to_window: bool,
}

impl Default for ImageViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageViewer {
    pub fn new() -> Self {
        Self {
            path: None,
            texture: None,
            image_size: None,
            zoom: 1.0,
            pan_offset: egui::Vec2::ZERO,
            fit_to_window: true,
        }
    }

    pub fn load(&mut self, path: &Path, ctx: &egui::Context) -> Result<()> {
        let image_data = std::fs::read(path)?;
        let image = image::load_from_memory(&image_data)?;
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];

        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);

        let texture = ctx.load_texture(
            path.to_string_lossy(),
            color_image,
            egui::TextureOptions::LINEAR,
        );

        self.texture = Some(texture);
        self.image_size = Some(size);
        self.path = Some(path.to_path_buf());
        self.zoom = 1.0;
        self.pan_offset = egui::Vec2::ZERO;
        self.fit_to_window = true;

        Ok(())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_loaded(&self) -> bool {
        self.texture.is_some()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Toolbar
        ui.horizontal(|ui| {
            if ui.button("-").clicked() {
                self.zoom = (self.zoom * 0.8).max(0.1);
                self.fit_to_window = false;
            }
            ui.label(format!("{:.0}%", self.zoom * 100.0));
            if ui.button("+").clicked() {
                self.zoom = (self.zoom * 1.25).min(10.0);
                self.fit_to_window = false;
            }
            ui.separator();
            if ui.button("Fit").clicked() {
                self.fit_to_window = true;
                self.pan_offset = egui::Vec2::ZERO;
            }
            if ui.button("100%").clicked() {
                self.zoom = 1.0;
                self.fit_to_window = false;
                self.pan_offset = egui::Vec2::ZERO;
            }

            if let Some(size) = self.image_size {
                ui.separator();
                ui.label(format!("{}x{}", size[0], size[1]));
            }

            if let Some(path) = &self.path {
                ui.separator();
                ui.label(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                );
            }
        });

        ui.separator();

        // Image display
        if let Some(texture) = &self.texture {
            let available_size = ui.available_size();
            let image_size = texture.size_vec2();

            let display_size = if self.fit_to_window {
                // Calculate fit-to-window size
                let scale_x = available_size.x / image_size.x;
                let scale_y = available_size.y / image_size.y;
                let scale = scale_x.min(scale_y).min(1.0);
                self.zoom = scale;
                image_size * scale
            } else {
                image_size * self.zoom
            };

            // Scrollable area for panning
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let (rect, response) = ui
                        .allocate_exact_size(display_size.max(available_size), egui::Sense::drag());

                    // Handle panning
                    if response.dragged() {
                        self.pan_offset += response.drag_delta();
                        self.fit_to_window = false;
                    }

                    // Handle scroll wheel zoom
                    if response.hovered() {
                        let scroll = ui.input(|i| i.raw_scroll_delta.y);
                        if scroll != 0.0 {
                            let factor = if scroll > 0.0 { 1.1 } else { 0.9 };
                            self.zoom = (self.zoom * factor).clamp(0.1, 10.0);
                            self.fit_to_window = false;
                        }
                    }

                    // Draw image centered
                    let image_rect =
                        egui::Rect::from_center_size(rect.center() + self.pan_offset, display_size);

                    ui.painter().image(
                        texture.id(),
                        image_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("No image loaded");
            });
        }
    }
}
