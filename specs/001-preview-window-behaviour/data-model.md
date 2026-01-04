# Data Model: Interactive Preview Companion

**Feature**: 001-preview-window-behaviour
**Date**: 2026-01-04

## Entity Relationship Overview

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│    ChatMode     │────<│   ChatSession   │────<│   ChatMessage   │
│  (enum, 5 vals) │     │   (persisted)   │     │   (persisted)   │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │
        │
        v
┌─────────────────┐     ┌─────────────────┐
│  ModePrompt     │     │   PreviewState  │
│  (runtime)      │     │   (runtime)     │
└─────────────────┘     └─────────────────┘
                              │
                              v
                        ┌─────────────────┐
                        │ PreviewContent  │
                        │  (runtime)      │
                        └─────────────────┘
```

## Entities

### ChatMode (Existing - Enhanced)

**Location**: `crates/app/src/sessions.rs`
**Status**: Exists, no changes needed

```rust
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub enum ChatMode {
    Find,
    Fix,
    Research,
    Data,
    Content,
}
```

**Attributes**:
| Field | Type | Description |
|-------|------|-------------|
| (enum variant) | - | One of 5 mode types |

**Methods** (existing):
- `as_str()` - lowercase string
- `display_name()` - capitalized name
- `icon()` - emoji icon
- `color()` - RGB tuple
- `welcome(name)` - welcome message

---

### ChatSession (Existing - Enhanced)

**Location**: `crates/app/src/sessions.rs`
**Status**: Exists, minor enhancements

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,                    // UUID
    pub mode: ChatMode,
    pub title: String,                 // Auto-generated from first user message
    pub messages: Vec<ChatMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Persistence**:
- Location: `~/.config/LittleHelper/sessions/{mode}/{id}.json`
- Format: JSON (serde)
- Auto-save: On every message add (FR-013a)

**Validation Rules**:
- `id` must be valid UUID
- `messages` must not be empty (welcome message always present)

---

### ChatMessage (Existing - Enhanced)

**Location**: `crates/app/src/sessions.rs`
**Status**: Exists, add preview reference

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,           // "system" | "user" | "assistant"
    pub content: String,        // Message text (may contain preview tags)
    pub timestamp: String,      // HH:MM format
    // NEW: Optional preview content extracted from message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<PreviewReference>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PreviewReference {
    pub content_type: String,   // "file", "web", "image", "ascii"
    pub source: String,         // Path or URL
}
```

---

### PreviewState (New - Runtime Only)

**Location**: `crates/app/src/preview_panel.rs`
**Status**: New

```rust
#[derive(Clone)]
pub struct PreviewState {
    pub visible: bool,
    pub content: Option<PreviewContent>,
    pub zoom: f32,              // 0.25 to 4.0
    pub scroll_offset: egui::Vec2,
    pub fullscreen: bool,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            visible: true,
            content: None,
            zoom: 1.0,
            scroll_offset: egui::Vec2::ZERO,
            fullscreen: false,
        }
    }
}
```

**State Transitions**:
```
              show_content()
    Empty ──────────────────> Loaded
      ^                          │
      │                          │
      │ close()          update_content()
      │                          │
      └──────────────────────────┘
                hide()
    Loaded ──────────────────> Hidden
      ^                          │
      │                          │
      └──────────────────────────┘
              show_content() / toggle()
```

---

### PreviewContent (New - Runtime Only)

**Location**: `crates/shared/src/preview_types.rs`
**Status**: New

```rust
#[derive(Clone)]
pub enum PreviewContent {
    /// Local file preview
    File {
        path: PathBuf,
        file_type: FileType,
        cached_content: Option<CachedFileContent>,
    },
    /// Web page preview
    Web {
        url: String,
        title: Option<String>,
        screenshot_path: Option<PathBuf>,
        og_image: Option<String>,
        snippet: Option<String>,
    },
    /// Direct image
    Image {
        source: ImageSource,
        cached_texture: Option<egui::TextureHandle>,
    },
    /// ASCII art state
    Ascii {
        state: AsciiState,
        art: String,
    },
    /// Mode introduction
    ModeIntro {
        mode: ChatMode,
        title: String,
        description: String,
        examples: Vec<String>,
        art: String,
    },
    /// Error state
    Error {
        message: String,
        original_source: String,
    },
}

#[derive(Clone)]
pub enum ImageSource {
    File(PathBuf),
    Url(String),
    Bytes(Vec<u8>),
}

#[derive(Clone)]
pub enum CachedFileContent {
    Text(String),
    Csv(Vec<Vec<String>>),
    Json(serde_json::Value),
    Image(egui::TextureHandle),
}
```

---

### AsciiState (New)

**Location**: `crates/app/src/ascii_art.rs`
**Status**: New

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AsciiState {
    Welcome,
    Thinking,
    Success,
    Error,
    ModeIntro(ChatMode),
}
```

---

### OnboardingState (New)

**Location**: `crates/app/src/onboarding.rs`
**Status**: New

```rust
#[derive(Clone, Serialize, Deserialize)]
pub enum OnboardingStep {
    Welcome,
    TerminalPermission,
    DependencyCheck,
    DependencyInstall { name: String, status: InstallStatus },
    Verification,
    Complete,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum InstallStatus {
    Pending,
    Installing,
    Installed,
    Failed(String),
    Skipped,
}

#[derive(Clone)]
pub struct OnboardingState {
    pub step: OnboardingStep,
    pub terminal_approved: bool,
    pub dependencies: Vec<DependencyStatus>,
    pub verification_result: Option<Result<(), String>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub name: String,           // e.g., "wsl", "curl"
    pub required: bool,         // Must have vs nice-to-have
    pub detected: bool,
    pub install_command: Option<String>,
}
```

**Persistence**: Part of `AppSettings.user_profile.onboarding_complete`

---

### ModePrompt (New - Runtime)

**Location**: `crates/agent_host/src/prompts.rs`
**Status**: New

```rust
pub struct ModePrompt {
    pub mode: ChatMode,
    pub personality: &'static str,
    pub expertise: Vec<&'static str>,
    pub example_questions: Vec<&'static str>,
    pub tools_available: Vec<&'static str>,
    pub tone: &'static str,
}

// Loaded at compile time from files or const strings
pub fn get_mode_prompt(mode: ChatMode) -> ModePrompt {
    match mode {
        ChatMode::Find => FIND_PROMPT,
        ChatMode::Fix => FIX_PROMPT,
        ChatMode::Research => RESEARCH_PROMPT,
        ChatMode::Data => DATA_PROMPT,
        ChatMode::Content => CONTENT_PROMPT,
    }
}
```

---

## Data Flow

### 1. Mode Switch Flow

```
User clicks mode tab
        │
        v
┌───────────────────┐
│ SessionManager    │
│ .list(mode)       │──> Load sessions from disk
│ .current(mode)    │──> Get most recent or create new
└───────────────────┘
        │
        v
┌───────────────────┐
│ PreviewState      │
│ .show_intro(mode) │──> Display mode introduction
└───────────────────┘
        │
        v
┌───────────────────┐
│ AgentHost         │
│ .set_prompt(mode) │──> Load mode-specific system prompt
└───────────────────┘
```

### 2. Message Send Flow

```
User types message, presses Enter
        │
        v
┌───────────────────┐
│ ChatSession       │
│ .add_message()    │──> Add to messages vec
└───────────────────┘
        │
        v
┌───────────────────┐
│ SessionManager    │
│ .save()           │──> Immediately write to disk (crash recovery)
└───────────────────┘
        │
        v
┌───────────────────┐
│ Provider          │
│ .send_message()   │──> Stream response from AI
└───────────────────┘
        │
        v
┌───────────────────┐
│ Response Parser   │
│ .parse_preview()  │──> Extract <preview> tags
└───────────────────┘
        │
        ├──> Update PreviewState if tags found
        │
        v
┌───────────────────┐
│ ChatSession       │
│ .add_message()    │──> Add assistant response
└───────────────────┘
        │
        v
┌───────────────────┐
│ SessionManager    │
│ .save()           │──> Immediately write to disk
└───────────────────┘
```

### 3. Preview Interaction Flow

```
User interacts with preview
        │
        ├──> Zoom: Update PreviewState.zoom (0.25-4.0)
        │
        ├──> Scroll: Update PreviewState.scroll_offset
        │
        ├──> Fullscreen: Toggle PreviewState.fullscreen
        │
        ├──> Open in App: Call open::that(path)
        │
        ├──> Reveal in Finder: Call open::that(parent_dir) + select
        │
        └──> Close: Set PreviewState.visible = false
```

## Storage Schema

### Session Files

**Location**: `~/.config/LittleHelper/sessions/{mode}/{session_id}.json`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "mode": "Research",
  "title": "Climate change impact on...",
  "messages": [
    {
      "role": "assistant",
      "content": "Hi! I'm your Research agent...",
      "timestamp": "14:30"
    },
    {
      "role": "user",
      "content": "Research climate change effects",
      "timestamp": "14:31"
    },
    {
      "role": "assistant",
      "content": "I found several sources...\n<preview type=\"web\" url=\"https://...\">Key findings</preview>",
      "timestamp": "14:32",
      "preview": {
        "content_type": "web",
        "source": "https://..."
      }
    }
  ],
  "created_at": "2026-01-04T14:30:00Z",
  "updated_at": "2026-01-04T14:32:00Z"
}
```

### Settings File

**Location**: `~/.config/LittleHelper/settings.json`

Already exists in `shared/src/lib.rs`. Add to `UserProfile`:

```json
{
  "user_profile": {
    "name": "Flower",
    "mascot_image_path": null,
    "dark_mode": true,
    "onboarding_complete": true,
    "terminal_permission_granted": true
  }
}
```
