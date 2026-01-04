# Research: Interactive Preview Companion

**Feature**: 001-preview-window-behaviour
**Date**: 2026-01-04

## Research Questions

### 1. Web Page Preview/Screenshot Capture

**Question**: How to capture website previews for display in the preview panel when the AI performs web searches?

**Decision**: Use headless browser screenshot via external tool (wkhtmltoimage) or fallback to text+image extraction

**Rationale**:
- Full browser rendering gives accurate previews but adds dependency
- wkhtmltoimage is widely available and produces good results
- Fallback to extracting Open Graph images + title/description for lightweight preview
- Cache screenshots locally to avoid repeated captures

**Alternatives Considered**:
1. **Embed WebView** - Heavy dependency, security concerns, rejected
2. **Use existing curl approach** - Already in executor.rs, good for text extraction
3. **Screenshot service API** - Adds network dependency, privacy concerns
4. **Headless Chrome/Puppeteer** - Too heavy for a desktop app

**Implementation Notes**:
```rust
// Preview content types
pub enum WebPreview {
    Screenshot { path: PathBuf, url: String },
    OpenGraph { title: String, description: String, image_url: Option<String> },
    TextOnly { title: String, snippet: String, url: String },
}
```

---

### 2. Zoom/Scroll/Pan Implementation in egui

**Question**: Best pattern for implementing zoom, scroll, and pan controls in egui viewers?

**Decision**: Use egui's built-in scroll area with custom zoom transform

**Rationale**:
- egui's ScrollArea handles scrolling natively
- Apply zoom as a transform on the content (scale factor)
- Track zoom level as state in viewer
- Use mouse wheel + Ctrl for zoom, plain scroll for pan

**Alternatives Considered**:
1. **Custom viewport math** - More control but reinvents the wheel
2. **Image crate zoom** - Only works for images, not text/other content

**Implementation Pattern**:
```rust
pub struct ZoomableViewer {
    zoom: f32,           // 0.25 to 4.0
    offset: egui::Vec2,  // pan offset
    content: ViewerContent,
}

impl ZoomableViewer {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let response = egui::ScrollArea::both()
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size() * self.zoom);
                // Render content with scale transform
            });

        // Handle zoom with Ctrl+scroll
        if response.hovered() && ui.input(|i| i.modifiers.ctrl) {
            self.zoom += ui.input(|i| i.scroll_delta.y) * 0.01;
            self.zoom = self.zoom.clamp(0.25, 4.0);
        }
    }
}
```

---

### 3. Fullscreen Mode in eframe

**Question**: How to implement fullscreen preview that overlays the main UI?

**Decision**: Use egui::Window with fixed fullscreen positioning + close button

**Rationale**:
- egui::Window can be positioned to fill the screen
- No need for OS-level fullscreen (complex, platform-specific)
- Escape key and close button for exit
- Simpler than true fullscreen, equally effective for preview

**Implementation Pattern**:
```rust
fn show_fullscreen_preview(&mut self, ctx: &egui::Context) {
    if self.fullscreen {
        egui::Window::new("Preview")
            .fixed_rect(ctx.screen_rect())
            .title_bar(false)
            .show(ctx, |ui| {
                // Close button in corner
                if ui.button("✕").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.fullscreen = false;
                }
                // Render content
                self.render_content(ui);
            });
    }
}
```

---

### 4. Mode-Specific Agent Prompts

**Question**: How to structure system prompts so each mode has distinct personality and capabilities?

**Decision**: Prompt templates with mode-specific sections + shared base

**Rationale**:
- Base prompt defines tool usage and safety rules (shared)
- Mode section defines personality, expertise, and example interactions
- Prompts stored as const strings or loaded from files
- Easy to iterate on personalities without code changes

**Implementation Pattern**:
```rust
pub fn get_system_prompt(mode: ChatMode, user_name: &str, memory_summary: &str) -> String {
    let base = include_str!("prompts/base.txt");
    let mode_prompt = match mode {
        ChatMode::Find => include_str!("prompts/find.txt"),
        ChatMode::Fix => include_str!("prompts/fix.txt"),
        ChatMode::Research => include_str!("prompts/research.txt"),
        ChatMode::Data => include_str!("prompts/data.txt"),
        ChatMode::Content => include_str!("prompts/content.txt"),
    };

    format!("{base}\n\n## Your Role\n{mode_prompt}\n\n## User Context\nUser's name: {user_name}\n\n## Memory\n{memory_summary}")
}
```

---

### 5. Conversation Memory for Agent Context

**Question**: How to provide conversation history to agents so they can reference past discussions?

**Decision**: Include recent message summary in system prompt + full history in message context

**Rationale**:
- System prompt includes summary of past sessions (already implemented in sessions.rs)
- Last N messages sent as conversation context to AI
- For long conversations, summarize older messages
- Token budget management is provider-specific

**Existing Implementation** (sessions.rs:267-285):
```rust
pub fn get_memory_summary(&self, mode: ChatMode) -> String {
    // Already returns summary of recent sessions
}
```

**Enhancement Needed**:
- Add method to get last N messages for context window
- Add summarization for very long sessions (future enhancement)

---

### 6. Onboarding Flow for Permissions

**Question**: Best UX pattern for requesting terminal permissions and checking dependencies?

**Decision**: Multi-step wizard with progress indication

**Rationale**:
- First-run detection via settings flag (onboarding_complete exists)
- Steps: Welcome → Permissions → Dependencies → Verify → Complete
- Each step shows clear explanation and single action
- Skip option for advanced users

**Implementation Pattern**:
```rust
pub enum OnboardingStep {
    Welcome,
    TerminalPermission,
    DependencyCheck,
    DependencyInstall(String), // Which dependency
    Verification,
    Complete,
}

pub struct OnboardingState {
    step: OnboardingStep,
    terminal_approved: bool,
    dependencies_ok: bool,
    verification_result: Option<Result<(), String>>,
}
```

**Dependency Detection**:
- macOS/Linux: Check for common tools (bash, curl, etc.) - usually present
- Windows: Check for WSL if needed, PowerShell (usually present)
- Run simple test command to verify execution works

---

### 7. ASCII Art State Management

**Question**: How to display appropriate ASCII art for different app states?

**Decision**: State-based art selection with theme-aware rendering

**Rationale**:
- Define states: Welcome, Thinking, Success, Error, ModeIntro
- Store art as multi-line strings
- Render in monospace font with theme-appropriate colors
- Simple fade/transition between states

**Implementation Pattern**:
```rust
pub enum AsciiState {
    Welcome,
    Thinking,
    Success,
    Error,
    ModeIntro(ChatMode),
}

pub fn get_ascii_art(state: AsciiState) -> &'static str {
    match state {
        AsciiState::Welcome => include_str!("art/welcome.txt"),
        AsciiState::Thinking => include_str!("art/thinking.txt"),
        AsciiState::Success => include_str!("art/success.txt"),
        AsciiState::Error => include_str!("art/error.txt"),
        AsciiState::ModeIntro(mode) => match mode {
            ChatMode::Find => include_str!("art/find.txt"),
            // etc.
        },
    }
}
```

---

### 8. Preview Panel Communication Protocol

**Question**: How does the AI agent signal preview content updates?

**Decision**: XML-like tags in agent response that app parses

**Rationale**:
- Pattern already established in similar apps (Claude artifacts, etc.)
- Easy to parse from streamed responses
- Doesn't interfere with normal markdown content
- Can include metadata (file type, URL, etc.)

**Protocol**:
```
<preview type="file" path="/path/to/file.csv">
Optional caption or description
</preview>

<preview type="web" url="https://example.com">
Key finding from this source
</preview>

<preview type="image" url="https://example.com/image.png">
Image description
</preview>
```

**Parsing**:
```rust
pub struct PreviewTag {
    pub content_type: String, // "file", "web", "image"
    pub path: Option<String>,
    pub url: Option<String>,
    pub caption: Option<String>,
}

pub fn parse_preview_tags(response: &str) -> Vec<PreviewTag> {
    // Regex or simple state machine to extract tags
}
```

---

## Technology Decisions Summary

| Area | Decision | Confidence |
|------|----------|------------|
| Web Preview | wkhtmltoimage + OG fallback | Medium |
| Zoom/Scroll | egui ScrollArea + transform | High |
| Fullscreen | egui::Window overlay | High |
| Agent Prompts | Template files per mode | High |
| Memory | Summary in system prompt | High |
| Onboarding | Multi-step wizard | High |
| ASCII Art | State-based with theme colors | High |
| Preview Protocol | XML-like tags in response | High |

## Open Questions for Implementation

1. **wkhtmltoimage availability**: Need to handle case where it's not installed - fallback gracefully
2. **Token budgets**: Different providers have different limits - need provider-aware context trimming
3. **ASCII art design**: Need to create actual art assets - could be deferred to polish phase
4. **WSL detection on Windows**: Need reliable way to check if WSL is available and functional

## Dependencies to Add

```toml
# Cargo.toml additions (if needed)
# wkhtmltoimage called via Command, no crate needed
# No new dependencies required - using existing egui/tokio/reqwest
```
