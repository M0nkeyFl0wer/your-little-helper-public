//! Lightweight markdown renderer for egui chat bubbles.
//!
//! Handles the subset of markdown that AI models actually produce:
//! - `# Heading` through `#### Heading`
//! - `**bold**`
//! - `*italic*` (single asterisk not at line start)
//! - `- bullet` and `* bullet` list items
//! - `[text](url)` links
//! - `` `inline code` ``
//! - Paragraphs separated by blank lines

use eframe::egui;

/// Render markdown text into an egui UI region.
///
/// `base_color` is the default text color; `link_color` is for hyperlinks.
pub fn render_markdown(ui: &mut egui::Ui, text: &str, base_color: egui::Color32) {
    let link_color = egui::Color32::from_rgb(100, 170, 240);
    let code_bg = if base_color.r() > 128 {
        // dark mode — lighter code bg
        egui::Color32::from_rgb(60, 60, 70)
    } else {
        egui::Color32::from_rgb(230, 232, 236)
    };

    for line in text.lines() {
        let trimmed = line.trim();

        // Blank line → spacing
        if trimmed.is_empty() {
            ui.add_space(6.0);
            continue;
        }

        // Headings
        if let Some(rest) = trimmed.strip_prefix("#### ") {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(rest)
                    .strong()
                    .size(14.0)
                    .color(base_color),
            );
            ui.add_space(2.0);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            ui.add_space(5.0);
            ui.label(
                egui::RichText::new(rest)
                    .strong()
                    .size(15.0)
                    .color(base_color),
            );
            ui.add_space(2.0);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(rest)
                    .strong()
                    .size(16.0)
                    .color(base_color),
            );
            ui.add_space(3.0);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(rest)
                    .strong()
                    .size(18.0)
                    .color(base_color),
            );
            ui.add_space(4.0);
            continue;
        }

        // Bullet list items: "- text" or "* text" (at line start)
        let (is_bullet, bullet_text) = if let Some(rest) = trimmed.strip_prefix("- ") {
            (true, rest)
        } else if let Some(rest) = trimmed.strip_prefix("* ") {
            (true, rest)
        } else {
            (false, trimmed)
        };

        if is_bullet {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new("  •  ")
                        .size(14.0)
                        .color(base_color),
                );
                render_inline_markdown(ui, bullet_text, base_color, link_color, code_bg);
            });
            continue;
        }

        // Regular paragraph line
        ui.horizontal_wrapped(|ui| {
            render_inline_markdown(ui, trimmed, base_color, link_color, code_bg);
        });
    }
}

/// Render a single line with inline formatting: **bold**, *italic*, `code`, [links](url).
fn render_inline_markdown(
    ui: &mut egui::Ui,
    text: &str,
    base_color: egui::Color32,
    link_color: egui::Color32,
    code_bg: egui::Color32,
) {
    let mut remaining = text;
    let base_size = 14.0;

    while !remaining.is_empty() {
        // Find the next special marker
        let next_marker = find_next_marker(remaining);

        match next_marker {
            None => {
                // No more markers — emit the rest as plain text
                if !remaining.is_empty() {
                    ui.label(
                        egui::RichText::new(remaining)
                            .size(base_size)
                            .color(base_color),
                    );
                }
                break;
            }
            Some((pos, MarkerKind::Bold)) => {
                // Emit text before the marker
                if pos > 0 {
                    ui.label(
                        egui::RichText::new(&remaining[..pos])
                            .size(base_size)
                            .color(base_color),
                    );
                }
                remaining = &remaining[pos + 2..]; // skip **
                if let Some(end) = remaining.find("**") {
                    ui.label(
                        egui::RichText::new(&remaining[..end])
                            .size(base_size)
                            .strong()
                            .color(base_color),
                    );
                    remaining = &remaining[end + 2..];
                } else {
                    // No closing ** — emit as-is
                    ui.label(
                        egui::RichText::new(format!("**{}", remaining))
                            .size(base_size)
                            .color(base_color),
                    );
                    break;
                }
            }
            Some((pos, MarkerKind::Code)) => {
                if pos > 0 {
                    ui.label(
                        egui::RichText::new(&remaining[..pos])
                            .size(base_size)
                            .color(base_color),
                    );
                }
                remaining = &remaining[pos + 1..]; // skip `
                if let Some(end) = remaining.find('`') {
                    egui::Frame::none()
                        .fill(code_bg)
                        .rounding(egui::Rounding::same(3.0))
                        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&remaining[..end])
                                    .size(base_size)
                                    .monospace()
                                    .color(base_color),
                            );
                        });
                    remaining = &remaining[end + 1..];
                } else {
                    ui.label(
                        egui::RichText::new(format!("`{}", remaining))
                            .size(base_size)
                            .color(base_color),
                    );
                    break;
                }
            }
            Some((pos, MarkerKind::Link)) => {
                if pos > 0 {
                    ui.label(
                        egui::RichText::new(&remaining[..pos])
                            .size(base_size)
                            .color(base_color),
                    );
                }
                remaining = &remaining[pos + 1..]; // skip [
                if let Some(close_bracket) = remaining.find("](") {
                    let link_text = &remaining[..close_bracket];
                    remaining = &remaining[close_bracket + 2..]; // skip ](
                    if let Some(close_paren) = remaining.find(')') {
                        let url = &remaining[..close_paren];
                        if ui
                            .add(egui::Hyperlink::from_label_and_url(
                                egui::RichText::new(link_text)
                                    .size(base_size)
                                    .color(link_color)
                                    .underline(),
                                url,
                            ))
                            .on_hover_text(url)
                            .changed()
                        {}
                        remaining = &remaining[close_paren + 1..];
                    } else {
                        // Malformed — emit as-is
                        ui.label(
                            egui::RichText::new(format!("[{}](", link_text))
                                .size(base_size)
                                .color(base_color),
                        );
                    }
                } else {
                    // No ]( — emit as-is
                    ui.label(
                        egui::RichText::new(format!("[{}", remaining))
                            .size(base_size)
                            .color(base_color),
                    );
                    break;
                }
            }
        }
    }
}

#[derive(Debug)]
enum MarkerKind {
    Bold,  // **
    Code,  // `
    Link,  // [
}

/// Find the next inline marker in the text.
fn find_next_marker(text: &str) -> Option<(usize, MarkerKind)> {
    let mut best: Option<(usize, MarkerKind)> = None;

    if let Some(pos) = text.find("**") {
        best = Some((pos, MarkerKind::Bold));
    }
    if let Some(pos) = text.find('`') {
        if best.is_none() || pos < best.as_ref().unwrap().0 {
            best = Some((pos, MarkerKind::Code));
        }
    }
    if let Some(pos) = text.find('[') {
        // Only treat as link if followed by ]( somewhere
        if text[pos..].contains("](") {
            if best.is_none() || pos < best.as_ref().unwrap().0 {
                best = Some((pos, MarkerKind::Link));
            }
        }
    }

    best
}
