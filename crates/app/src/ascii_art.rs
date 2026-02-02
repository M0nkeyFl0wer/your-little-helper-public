//! ASCII art library for Little Helper (dogs edition)

use shared::preview_types::AsciiState;

/// Get ASCII art for a given state
pub fn get_ascii_art(state: AsciiState) -> &'static str {
    match state {
        AsciiState::Welcome => WELCOME_DOG_1,
        AsciiState::Thinking => THINKING_DOG_1,
        AsciiState::Success => SUCCESS_DOG,
        AsciiState::Error => ERROR_DOG,
    }
}

/// Animated ASCII art frames for fun states.
///
/// This keeps the UI lively without distracting motion.
pub fn get_ascii_art_animated(state: AsciiState, time: f64) -> &'static str {
    let frame = ((time * 2.0) as i32) % 2; // 2 fps
    match state {
        AsciiState::Welcome => {
            if frame == 0 {
                WELCOME_DOG_1
            } else {
                WELCOME_DOG_2
            }
        }
        AsciiState::Thinking => {
            if frame == 0 {
                THINKING_DOG_1
            } else {
                THINKING_DOG_2
            }
        }
        AsciiState::Success => SUCCESS_DOG,
        AsciiState::Error => ERROR_DOG,
    }
}

/// Get ASCII art for a mode introduction
pub fn get_mode_art(mode: &str) -> &'static str {
    match mode.to_lowercase().as_str() {
        "find" => FIND_DOG,
        "fix" => FIX_DOG,
        "research" => RESEARCH_DOG,
        "data" => DATA_DOG,
        "content" => CONTENT_DOG,
        "build" => SPEC_DOG,
        _ => WELCOME_DOG_1,
    }
}

// Welcome dog
const WELCOME_DOG_1: &str = r#"
 / \__
(    @\___
 /         O
/   (_____/
/_____/   U
"#;

const WELCOME_DOG_2: &str = r#"
 / \__
(    o\___
 /         O
/   (_____/
/_____/   U
"#;

// Thinking dog
const THINKING_DOG_1: &str = r#"
 / \__
(    @\___
 /  . .   O
/   (___/
/_____/ U
"#;

const THINKING_DOG_2: &str = r#"
 / \__
(    @\___
 /  . o   O
/   (___/
/_____/ U
"#;

// Success dog
const SUCCESS_DOG: &str = r#"
 / \__
(    @\___
 /  /\   O
/  /  \ /
/_____/ U
"#;

// Error dog
const ERROR_DOG: &str = r#"
 / \__
(    x\___
 /   --  O
/   (___/
/_____/ U
"#;

// Find mode dog
const FIND_DOG: &str = r#"
 / \__
(    @\___
 /   /\  O
/   /  \ /
/_____/ U
"#;

// Fix Helper dog
const FIX_DOG: &str = r#"
 / \__
(    @\___
 /  |  | O
/   |__|/
/_____/ U
"#;

// Research Helper dog
const RESEARCH_DOG: &str = r#"
 / \__
(    @\___
 /   __  O
/   (__) /
/_____/ U
"#;

// Data Helper dog
const DATA_DOG: &str = r#"
 / \__
(    @\___
 /  [__] O
/   [__]/
/_____/ U
"#;

// Content Helper dog
const CONTENT_DOG: &str = r#"
 / \__
(    @\___
 /  \_/  O
/   / \ /
/_____/ U
"#;

// Build mode - Spec the dog (compact, fits preview panel)
const SPEC_DOG: &str = r#"
       ██████          ██████
      ██▓▓▓▓▓▓██████████▓▓▓▓▓▓██
      ██▓▓▓▓██          ██▓▓▓▓██
      ██▓▓████    ▓▓▓▓▓▓████▓▓██
      ██  ██  ██▓▓██▓▓██  ██
           ██    ▓▓▓▓▓▓██
    ██              ██
    ██    ██████    ██
    ██    ██████    ██
    ██              ██
           ██    ██    ██
             ████░░████
               ██░░██
               ██░░██
                 ████

      🐕 Spec — Your Build Assistant
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
        assert!(!get_mode_art("build").is_empty());
    }
}
