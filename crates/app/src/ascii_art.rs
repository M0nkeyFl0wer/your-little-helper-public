//! ASCII art states for the Interactive Preview Companion feature.
//!
//! This module provides ASCII art for various states (welcome, thinking,
//! success, error) to add personality to the Little Helper app.

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
        "fix" => FIX_MODE_ART,
        "research" => RESEARCH_MODE_ART,
        "data" => DATA_MODE_ART,
        "content" => CONTENT_MODE_ART,
        _ => WELCOME_ART,
    }
}

// Welcome ASCII art - friendly helper character
const WELCOME_ART: &str = r#"
    ╭─────────────────────────╮
    │   Welcome to Little     │
    │        Helper!          │
    ╰─────────────────────────╯
           \
            \   ◠‿◠
             \ /    \
              │  ♥  │
              └─┬┬──┘
                ││
"#;

// Thinking ASCII art - contemplative look
const THINKING_ART: &str = r#"
    ╭─────────────────────────╮
    │      Thinking...        │
    ╰─────────────────────────╯
           \
            \   ◔_◔
             \ /    \
              │  ~  │
              └─┬┬──┘
                ││
              ⋯⋯⋯⋯
"#;

// Success ASCII art - happy celebration
const SUCCESS_ART: &str = r#"
    ╭─────────────────────────╮
    │      Done! ✓            │
    ╰─────────────────────────╯
           \
        ★   \   ◠‿◠   ★
             \ /    \
         ★    │  ✓  │   ★
              └─┬┬──┘
            ★  ││  ★
"#;

// Error ASCII art - sympathetic look
const ERROR_ART: &str = r#"
    ╭─────────────────────────╮
    │   Oops, something       │
    │   went wrong...         │
    ╰─────────────────────────╯
           \
            \   ◠_◠
             \ /    \
              │  ?  │
              └─┬┬──┘
                ││
"#;

// Find mode art - magnifying glass
const FIND_MODE_ART: &str = r#"
    ╭─────────────────────────╮
    │      Find Mode          │
    │   🔍 File Detective     │
    ╰─────────────────────────╯

         ┌─────────┐
         │  ◉ ◉    │
         │    ω    │
         │  ╭───╮  │
         └──│ 🔍│──┘
            ╰───╯
"#;

// Fix mode art - wrench/tool
const FIX_MODE_ART: &str = r#"
    ╭─────────────────────────╮
    │      Fix Mode           │
    │   🔧 Tech Support       │
    ╰─────────────────────────╯

         ┌─────────┐
         │  ◉ ◉    │
         │    ω    │
         │  ╭───╮  │
         └──│ 🔧│──┘
            ╰───╯
"#;

// Research mode art - books/study
const RESEARCH_MODE_ART: &str = r#"
    ╭─────────────────────────╮
    │    Research Mode        │
    │   📚 Knowledge Seeker   │
    ╰─────────────────────────╯

         ┌─────────┐
         │  ◉ ◉    │
         │    ω    │
         │  ╭───╮  │
         └──│ 📚│──┘
            ╰───╯
"#;

// Data mode art - charts/analysis
const DATA_MODE_ART: &str = r#"
    ╭─────────────────────────╮
    │      Data Mode          │
    │   📊 Data Analyst       │
    ╰─────────────────────────╯

         ┌─────────┐
         │  ◉ ◉    │
         │    ω    │
         │  ╭───╮  │
         └──│ 📊│──┘
            ╰───╯
"#;

// Content mode art - writing/creativity
const CONTENT_MODE_ART: &str = r#"
    ╭─────────────────────────╮
    │    Content Mode         │
    │   ✏️ Creative Writer    │
    ╰─────────────────────────╯

         ┌─────────┐
         │  ◉ ◉    │
         │    ω    │
         │  ╭───╮  │
         └──│ ✏️│──┘
            ╰───╯
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
