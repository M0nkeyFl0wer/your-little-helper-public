//! ASCII art library for Little Helper
//!
//! This module provides professional ASCII art for various states and modes.

use shared::preview_types::AsciiState;

/// Get ASCII art for a given state
pub fn get_ascii_art(state: AsciiState) -> &'static str {
    match state {
        AsciiState::Welcome => WELCOME_ART,
        AsciiState::Thinking => THINKING_ART,
        AsciiState::Success => SUCCESS_ART,
        AsciiState::Error => ERROR_ART,
    }
}

/// Get ASCII art for a mode introduction
pub fn get_mode_art(mode: &str) -> &'static str {
    match mode.to_lowercase().as_str() {
        "find" => FIND_MODE_ART,
        "fix" => FIX_HELPER_ART,
        "research" => RESEARCH_HELPER_ART,
        "data" => DATA_HELPER_ART,
        "content" => CONTENT_HELPER_ART,
        _ => WELCOME_ART,
    }
}

// Professional box drawing characters for clean borders
const WELCOME_ART: &str = r#"
    ╔═══════════════════════════════════╗
    ║                                   ║
    ║     Welcome to Little Helper      ║
    ║                                   ║
    ╚═══════════════════════════════════╝
"#;

// Clean thinking indicator with animated feel
const THINKING_ART: &str = r#"
    ╔═══════════════════════════════════╗
    ║         ◉  Thinking...  ◉         ║
    ╚═══════════════════════════════════╝
"#;

// Success indicator
const SUCCESS_ART: &str = r#"
    ╔═══════════════════════════════════╗
    ║      ✓  Done!  ✓                  ║
    ╚═══════════════════════════════════╝
"#;

// Error indicator
const ERROR_ART: &str = r#"
    ╔═══════════════════════════════════╗
    ║      ✗  Error  ✗                  ║
    ╚═══════════════════════════════════╝
"#;

// Find mode art - simple and clean
const FIND_MODE_ART: &str = r#"
    ╔═══════════════════════════════════╗
    ║                                   ║
    ║      Find Helper                  ║
    ║      🔍 File Detective            ║
    ║                                   ║
    ╚═══════════════════════════════════╝
"#;

// Fix Helper art
const FIX_HELPER_ART: &str = r#"
    ╔═══════════════════════════════════╗
    ║                                   ║
    ║      Fix Helper                   ║
    ║      🔧 Tech Support              ║
    ║                                   ║
    ╚═══════════════════════════════════╝
"#;

// Research Helper art
const RESEARCH_HELPER_ART: &str = r#"
    ╔═══════════════════════════════════╗
    ║                                   ║
    ║      Research Helper              ║
    ║      📚 Knowledge Seeker          ║
    ║                                   ║
    ╚═══════════════════════════════════╝
"#;

// Data Helper art
const DATA_HELPER_ART: &str = r#"
    ╔═══════════════════════════════════╗
    ║                                   ║
    ║      Data Helper                  ║
    ║      📊 Data Analyst              ║
    ║                                   ║
    ╚═══════════════════════════════════╝
"#;

// Content Helper art
const CONTENT_HELPER_ART: &str = r#"
    ╔═══════════════════════════════════╗
    ║                                   ║
    ║      Content Helper               ║
    ║      ✏️ Creative Writer           ║
    ║                                   ║
    ╚═══════════════════════════════════╝
"#;

/// Render ASCII art with theme-aware colors
pub fn render_ascii_art(ui: &mut egui::Ui, art: &str, is_dark_mode: bool) {
    let text_color = if is_dark_mode {
        egui::Color32::from_rgb(200, 200, 200)
    } else {
        egui::Color32::from_rgb(60, 60, 60)
    };

    ui.vertical_centered(|ui| {
        ui.add(egui::Label::new(
            egui::RichText::new(art).monospace().color(text_color),
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_ascii_art() {
        assert!(!get_ascii_art(AsciiState::Welcome).is_empty());
        assert!(!get_ascii_art(AsciiState::Thinking).is_empty());
        assert!(!get_ascii_art(AsciiState::Success).is_empty());
        assert!(!get_ascii_art(AsciiState::Error).is_empty());
    }

    #[test]
    fn test_get_mode_art() {
        assert!(!get_mode_art("find").is_empty());
        assert!(!get_mode_art("fix").is_empty());
        assert!(!get_mode_art("research").is_empty());
        assert!(!get_mode_art("data").is_empty());
        assert!(!get_mode_art("content").is_empty());
    }
}
