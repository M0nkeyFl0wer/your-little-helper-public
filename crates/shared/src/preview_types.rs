//! Preview content types for the Interactive Preview Companion feature.
//!
//! This module defines the types used to represent preview content that can be
//! displayed in the preview panel, including files, web pages, images, ASCII art,
//! and mode introductions.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The type of file being previewed (mirrors viewers crate)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    Text,
    Image,
    Csv,
    Json,
    Html,
    Pdf,
    Markdown,
    Unknown,
}

impl FileType {
    /// Detect file type from path extension
    pub fn from_path(path: &std::path::Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Some("txt" | "log" | "rs" | "py" | "js" | "ts" | "sh" | "toml" | "yaml" | "yml") => {
                FileType::Text
            }
            Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg") => FileType::Image,
            Some("csv" | "tsv") => FileType::Csv,
            Some("json") => FileType::Json,
            Some("html" | "htm") => FileType::Html,
            Some("pdf") => FileType::Pdf,
            Some("md" | "markdown") => FileType::Markdown,
            _ => FileType::Unknown,
        }
    }
}

/// A single file search result
#[derive(Clone, Debug)]
pub struct SearchResultItem {
    /// Full path to the file
    pub path: PathBuf,
    /// File name for display
    pub name: String,
    /// Parent directory
    pub parent: String,
    /// File size in bytes
    pub size: u64,
    /// Last modified timestamp (Unix epoch)
    pub modified: Option<i64>,
    /// Fuzzy match score (0.0-1.0)
    pub score: f32,
}

/// Content that can be displayed in the preview panel
#[derive(Clone, Debug)]
pub enum PreviewContent {
    /// Local file preview
    File { path: PathBuf, file_type: FileType },
    /// Web page preview
    Web {
        url: String,
        title: Option<String>,
        screenshot: Option<PathBuf>,
        og_image: Option<String>,
        snippet: Option<String>,
    },
    /// Direct image
    Image { source: ImageSource },
    /// ASCII art state
    Ascii { state: AsciiState },
    /// Mode introduction
    ModeIntro {
        mode: String, // Mode name as string to avoid circular dependency
    },
    /// Search results from fuzzy file finder
    SearchResults {
        query: String,
        results: Vec<SearchResultItem>,
        total_count: usize,
        search_time_ms: u64,
    },
    /// Version history for a file
    VersionHistory {
        file_path: PathBuf,
        file_name: String,
        versions: Vec<crate::version::FileVersion>,
    },
    /// Error state
    Error { message: String, source: String },
    /// Security dashboard (Fix/Secure mode)
    Security(SecurityView),
    /// Skills list for a mode
    SkillsList {
        mode: String,
        skills: Vec<SkillPreviewInfo>,
    },
    /// Helpful tip
    Tip { title: String, message: String },

    /// A proposed cleanup plan for a folder
    CleanupPlan {
        title: String,
        folder: PathBuf,
        snapshot: Vec<CleanupSnapshotItem>,
        moves: Vec<CleanupMove>,
        renames: Vec<CleanupRename>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanupSnapshotItem {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_rfc3339: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanupMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanupRename {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Source for an image to display
#[derive(Clone, Debug)]
pub enum ImageSource {
    /// Load from local file path
    File(PathBuf),
    /// Load from URL
    Url(String),
    /// Already loaded bytes
    Bytes(Vec<u8>),
}

/// ASCII art states for personality
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsciiState {
    Welcome,
    Thinking,
    Success,
    Error,
}

impl std::fmt::Display for AsciiState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsciiState::Welcome => write!(f, "welcome"),
            AsciiState::Thinking => write!(f, "thinking"),
            AsciiState::Success => write!(f, "success"),
            AsciiState::Error => write!(f, "error"),
        }
    }
}

/// A parsed preview tag from an agent response
#[derive(Clone, Debug)]
pub struct ParsedPreviewTag {
    /// Type of content: "file", "web", "image", "ascii"
    pub content_type: String,
    /// File path (for type="file" or type="image" with path)
    pub path: Option<String>,
    /// URL (for type="web" or type="image" with url)
    pub url: Option<String>,
    /// ASCII state (for type="ascii")
    pub state: Option<String>,
    /// Caption/description text between the tags
    pub caption: String,
}

impl ParsedPreviewTag {
    /// Convert parsed tag to PreviewContent
    pub fn to_content(&self) -> Option<PreviewContent> {
        match self.content_type.as_str() {
            "file" => {
                let path = PathBuf::from(self.path.as_ref()?);
                let file_type = FileType::from_path(&path);
                Some(PreviewContent::File { path, file_type })
            }
            "web" => Some(PreviewContent::Web {
                url: self.url.clone()?,
                title: None,
                screenshot: None,
                og_image: None,
                snippet: Some(self.caption.clone()),
            }),
            "image" => {
                if let Some(path) = &self.path {
                    Some(PreviewContent::Image {
                        source: ImageSource::File(PathBuf::from(path)),
                    })
                } else if let Some(url) = &self.url {
                    Some(PreviewContent::Image {
                        source: ImageSource::Url(url.clone()),
                    })
                } else {
                    None
                }
            }
            "ascii" => {
                let state = match self.state.as_deref() {
                    Some("welcome") => AsciiState::Welcome,
                    Some("thinking") => AsciiState::Thinking,
                    Some("success") => AsciiState::Success,
                    Some("error") => AsciiState::Error,
                    _ => AsciiState::Welcome,
                };
                Some(PreviewContent::Ascii { state })
            }
            // Security views are created programmatically, not from tags
            // The agent will use <preview type="security" view="health"> etc.
            "security" => {
                // Security views need to be populated with actual data,
                // so we return a placeholder that gets filled in by the app
                let view = match self.state.as_deref() {
                    Some("health") => SecurityView::Health(HealthDashboard::default()),
                    Some("privacy") => SecurityView::Privacy(PrivacyAudit::default()),
                    Some("processes") => SecurityView::Processes(ProcessAudit::default()),
                    Some("updates") => SecurityView::Updates(UpdateStatus::default()),
                    Some("connections") => SecurityView::Connections(ConnectionMonitor::default()),
                    Some("cleanup") => SecurityView::Cleanup(CleanupRecommendations::default()),
                    _ => SecurityView::Health(HealthDashboard::default()),
                };
                Some(PreviewContent::Security(view))
            }
            _ => None,
        }
    }
}

fn parse_preview_attributes(attrs_str: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut chars = attrs_str.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch == '/' {
            chars.next();
            continue;
        }

        let mut key = String::new();
        while let Some(&c) = chars.peek() {
            if c == '=' || c.is_whitespace() {
                break;
            }
            key.push(c);
            chars.next();
        }

        while let Some(&c) = chars.peek() {
            if c == '=' {
                chars.next();
                break;
            } else if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        while let Some(&c) = chars.peek() {
            if !c.is_whitespace() {
                break;
            }
            chars.next();
        }

        let mut value = String::new();
        if let Some(&c) = chars.peek() {
            if c == '"' || c == '\'' {
                let quote = c;
                chars.next();
                while let Some(next_char) = chars.next() {
                    if next_char == quote {
                        break;
                    }
                    value.push(next_char);
                }
            } else {
                while let Some(&next_char) = chars.peek() {
                    if next_char.is_whitespace() {
                        break;
                    }
                    value.push(next_char);
                    chars.next();
                }
            }
        }

        if !key.is_empty() {
            attrs.push((key, value));
        }
    }

    attrs
}

/// Parse all preview tags from a response. Supports both legacy and new formats.
pub fn parse_preview_tags(response: &str) -> Vec<ParsedPreviewTag> {
    let mut tags = Vec::new();
    let mut cursor = 0;
    const OPEN: &str = "<preview";
    const CLOSE: &str = "</preview>";

    while let Some(rel_start) = response[cursor..].find(OPEN) {
        let start = cursor + rel_start;
        let header_end_rel = match response[start..].find('>') {
            Some(end) => end,
            None => break,
        };
        let header_end = start + header_end_rel;
        let body_start = header_end + 1;
        let close_rel = match response[body_start..].find(CLOSE) {
            Some(idx) => idx,
            None => break,
        };
        let close_start = body_start + close_rel;
        let caption = response[body_start..close_start].trim().to_string();
        let attrs_str = &response[start + OPEN.len()..header_end];

        let mut content_type = String::new();
        let mut path = None;
        let mut url = None;
        let mut state = None;

        for (key, value) in parse_preview_attributes(attrs_str) {
            match key.as_str() {
                "type" => content_type = value,
                "path" => {
                    if !value.is_empty() {
                        path = Some(value);
                    }
                }
                "url" => {
                    if !value.is_empty() {
                        url = Some(value);
                    }
                }
                "state" => {
                    if !value.is_empty() {
                        state = Some(value);
                    }
                }
                _ => {}
            }
        }

        if content_type.is_empty() {
            if !caption.is_empty() {
                content_type = "file".to_string();
                path = Some(caption.clone());
            } else {
                cursor = close_start + CLOSE.len();
                continue;
            }
        }

        tags.push(ParsedPreviewTag {
            content_type,
            path,
            url,
            state,
            caption,
        });

        cursor = close_start + CLOSE.len();
    }

    tags
}

/// Parse the first preview tag from an agent response
pub fn parse_preview_tag(response: &str) -> Option<ParsedPreviewTag> {
    parse_preview_tags(response).into_iter().next()
}

/// Strip preview tags from a response for display
pub fn strip_preview_tags(response: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    const OPEN: &str = "<preview";
    const CLOSE: &str = "</preview>";

    while let Some(rel_start) = response[cursor..].find(OPEN) {
        let start = cursor + rel_start;
        let header_end_rel = match response[start..].find('>') {
            Some(end) => end,
            None => {
                output.push_str(&response[cursor..]);
                return output.trim().to_string();
            }
        };
        let header_end = start + header_end_rel;
        let body_start = header_end + 1;
        let close_rel = match response[body_start..].find(CLOSE) {
            Some(idx) => idx,
            None => {
                output.push_str(&response[cursor..]);
                return output.trim().to_string();
            }
        };
        output.push_str(&response[cursor..start]);
        cursor = body_start + close_rel + CLOSE.len();
    }

    output.push_str(&response[cursor..]);
    output.trim().to_string()
}

/// Reference to preview content stored in a message
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewReference {
    /// Type of content: "file", "web", "image", "ascii"
    pub content_type: String,
    /// Path or URL to the content
    pub source: String,
}

// =============================================================================
// Security Preview Types (Fix/Secure Mode)
// =============================================================================

/// Visual status indicator - shown with colors/icons
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityStatus {
    /// ✅ All good (green)
    Good,
    /// ⚠️ Needs attention (yellow)
    Warning,
    /// 🔴 Critical issue (red)
    Critical,
    /// ❓ Unknown/checking (gray)
    Unknown,
}

impl SecurityStatus {
    /// Get emoji for display
    pub fn emoji(&self) -> &'static str {
        match self {
            SecurityStatus::Good => "✅",
            SecurityStatus::Warning => "⚠️",
            SecurityStatus::Critical => "🔴",
            SecurityStatus::Unknown => "❓",
        }
    }

    /// Get human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            SecurityStatus::Good => "Good",
            SecurityStatus::Warning => "Needs attention",
            SecurityStatus::Critical => "Action needed",
            SecurityStatus::Unknown => "Checking...",
        }
    }
}

/// Overall security health dashboard
/// Shows: protection, updates, privacy, storage, performance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthDashboard {
    /// Is protection enabled? (firewall, antivirus)
    pub protection_status: SecurityStatus,
    pub protection_label: String,

    /// Are updates current?
    pub update_status: SecurityStatus,
    pub update_label: String,

    /// Privacy issues found
    pub privacy_status: SecurityStatus,
    pub privacy_issues_count: usize,

    /// Storage health (percent free)
    pub storage_status: SecurityStatus,
    pub storage_percent_free: u8,

    /// System performance
    pub performance_status: SecurityStatus,
    pub performance_label: String,

    /// Overall score (0-100)
    pub overall_score: u8,
    /// Human-friendly summary ("Looking good!" / "A few things to check")
    pub summary: String,

    /// When this scan was performed
    pub scan_time: Option<i64>,
}

impl Default for HealthDashboard {
    fn default() -> Self {
        Self {
            protection_status: SecurityStatus::Unknown,
            protection_label: "Checking...".into(),
            update_status: SecurityStatus::Unknown,
            update_label: "Checking...".into(),
            privacy_status: SecurityStatus::Unknown,
            privacy_issues_count: 0,
            storage_status: SecurityStatus::Unknown,
            storage_percent_free: 0,
            performance_status: SecurityStatus::Unknown,
            performance_label: "Checking...".into(),
            overall_score: 0,
            summary: "Running security check...".into(),
            scan_time: None,
        }
    }
}

/// Access status for an app's permission
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessStatus {
    /// User explicitly granted - shows "You approved"
    Approved,
    /// Granted but suspicious/unused - shows "Review this"
    NeedsReview,
    /// Definitely should remove - shows "Remove access"
    Revoke,
}

impl AccessStatus {
    pub fn emoji(&self) -> &'static str {
        match self {
            AccessStatus::Approved => "✅",
            AccessStatus::NeedsReview => "⚠️",
            AccessStatus::Revoke => "🔴",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AccessStatus::Approved => "You approved",
            AccessStatus::NeedsReview => "Review this",
            AccessStatus::Revoke => "Remove access",
        }
    }
}

/// An app's access to a sensitive resource (camera, mic, etc.)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppAccess {
    /// Display name of the app
    pub app_name: String,
    /// Path to app icon (optional)
    pub app_icon: Option<PathBuf>,
    /// Access status
    pub status: AccessStatus,
    /// When last used (optional)
    pub last_used: Option<i64>,
    /// Bundle ID or identifier for actions
    pub app_id: Option<String>,
}

/// Privacy audit showing which apps can access sensitive resources
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PrivacyAudit {
    /// Apps with camera access
    pub camera_access: Vec<AppAccess>,
    /// Apps with microphone access
    pub microphone_access: Vec<AppAccess>,
    /// Apps with location access
    pub location_access: Vec<AppAccess>,
    /// Apps with file/folder access
    pub files_access: Vec<AppAccess>,
    /// Apps with contacts access
    pub contacts_access: Vec<AppAccess>,
    /// When this audit was performed
    pub scan_time: Option<i64>,
}

/// A process flagged as suspicious
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuspiciousProcess {
    /// Process name
    pub name: String,
    /// Process ID
    pub pid: u32,
    /// Why it's flagged (human-readable)
    pub reason: String,
    /// CPU usage percent
    pub cpu_percent: f32,
    /// Memory usage in MB
    pub memory_mb: u64,
    /// Our recommendation ("Probably fine" / "Stop this")
    pub recommendation: String,
    /// Is this a known safe process? (for "What is this?" explanations)
    pub known_safe: bool,
}

/// Process/activity audit showing what's running
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessAudit {
    /// Count of normal processes
    pub normal_count: usize,
    /// Suspicious processes that need review
    pub suspicious: Vec<SuspiciousProcess>,
    /// Was malware detected?
    pub malware_detected: bool,
    /// Malware names if detected
    pub malware_names: Vec<String>,
    /// When this scan was performed
    pub scan_time: Option<i64>,
}

impl Default for ProcessAudit {
    fn default() -> Self {
        Self {
            normal_count: 0,
            suspicious: Vec::new(),
            malware_detected: false,
            malware_names: Vec::new(),
            scan_time: None,
        }
    }
}

/// An available software update
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvailableUpdate {
    /// Display name (e.g., "macOS Sonoma 14.3.1")
    pub name: String,
    /// Icon/emoji for the app/system
    pub icon: String,
    /// Is this a security update?
    pub is_security: bool,
    /// Brief description
    pub description: String,
    /// Action to take (e.g., command to run)
    pub action: Option<String>,
}

/// Update status view
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateStatus {
    /// Critical/security updates available
    pub security_updates: Vec<AvailableUpdate>,
    /// Regular updates available
    pub regular_updates: Vec<AvailableUpdate>,
    /// Count of up-to-date apps
    pub up_to_date_count: usize,
    /// When last checked
    pub last_checked: Option<i64>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            security_updates: Vec::new(),
            regular_updates: Vec::new(),
            up_to_date_count: 0,
            last_checked: None,
        }
    }
}

/// A network connection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkConnection {
    /// Program making the connection
    pub program: String,
    /// Remote host/IP
    pub remote_host: String,
    /// Status (Normal/Unknown/Suspicious)
    pub status: SecurityStatus,
    /// Human-readable status label
    pub status_label: String,
}

/// A listening service
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListeningService {
    /// Program name
    pub program: String,
    /// Port number
    pub port: u16,
    /// Is it local-only or exposed?
    pub local_only: bool,
    /// Status
    pub status: SecurityStatus,
}

/// Network connection monitor
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionMonitor {
    /// Is firewall enabled?
    pub firewall_enabled: bool,
    /// Active outbound connections
    pub connections: Vec<NetworkConnection>,
    /// Services listening for connections
    pub listening: Vec<ListeningService>,
    /// Unknown/suspicious connection count
    pub unknown_count: usize,
    /// When this scan was performed
    pub scan_time: Option<i64>,
}

impl Default for ConnectionMonitor {
    fn default() -> Self {
        Self {
            firewall_enabled: false,
            connections: Vec::new(),
            listening: Vec::new(),
            unknown_count: 0,
            scan_time: None,
        }
    }
}

/// An item that can be cleaned up
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanupItem {
    /// Display name
    pub name: String,
    /// Size in bytes
    pub size_bytes: u64,
    /// Is it selected for cleanup?
    pub selected: bool,
    /// Path or identifier for cleanup action
    pub path: Option<PathBuf>,
    /// Cleanup category
    pub category: CleanupCategory,
}

/// Category of cleanup item
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanupCategory {
    BrowserCache,
    Downloads,
    Trash,
    AppCache,
    Logs,
    TempFiles,
}

impl CleanupCategory {
    pub fn label(&self) -> &'static str {
        match self {
            CleanupCategory::BrowserCache => "Browser cache",
            CleanupCategory::Downloads => "Old downloads",
            CleanupCategory::Trash => "Trash",
            CleanupCategory::AppCache => "App caches",
            CleanupCategory::Logs => "Log files",
            CleanupCategory::TempFiles => "Temporary files",
        }
    }
}

/// Cleanup recommendations view
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CleanupRecommendations {
    /// Items safe to remove
    pub items: Vec<CleanupItem>,
    /// Total bytes that can be freed
    pub total_bytes: u64,
    /// When this scan was performed
    pub scan_time: Option<i64>,
}

impl CleanupRecommendations {
    /// Format total as human-readable (e.g., "5.6 GB")
    pub fn total_human(&self) -> String {
        let bytes = self.total_bytes as f64;
        if bytes >= 1_000_000_000.0 {
            format!("{:.1} GB", bytes / 1_000_000_000.0)
        } else if bytes >= 1_000_000.0 {
            format!("{:.1} MB", bytes / 1_000_000.0)
        } else if bytes >= 1_000.0 {
            format!("{:.1} KB", bytes / 1_000.0)
        } else {
            format!("{} bytes", self.total_bytes)
        }
    }
}

/// Skill information for preview display
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillPreviewInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub permission_level: String, // "Safe" or "Sensitive"
    pub requires_approval: bool,
}

/// All security preview views
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SecurityView {
    /// Overall health dashboard
    Health(HealthDashboard),
    /// Privacy audit (who can access what)
    Privacy(PrivacyAudit),
    /// Process audit (what's running)
    Processes(ProcessAudit),
    /// Update status
    Updates(UpdateStatus),
    /// Network connections
    Connections(ConnectionMonitor),
    /// Cleanup recommendations
    Cleanup(CleanupRecommendations),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_preview() {
        let response = r#"Here's the file you requested.
<preview type="file" path="/home/user/doc.pdf">
Important document
</preview>
Check it out!"#;

        let tag = parse_preview_tag(response).unwrap();
        assert_eq!(tag.content_type, "file");
        assert_eq!(tag.path, Some("/home/user/doc.pdf".to_string()));
        assert_eq!(tag.caption, "Important document");
    }

    #[test]
    fn test_parse_web_preview() {
        let response = r#"Found this article:
<preview type="web" url="https://example.com/article">
Key findings from research
</preview>"#;

        let tag = parse_preview_tag(response).unwrap();
        assert_eq!(tag.content_type, "web");
        assert_eq!(tag.url, Some("https://example.com/article".to_string()));
    }

    #[test]
    fn test_parse_preview_with_spaces() {
        let response = r#"<preview type="file" path="C:\Users\Example User\Notes.txt">
Context
</preview>"#;
        let tag = parse_preview_tag(response).unwrap();
        assert_eq!(
            tag.path,
            Some(r"C:\Users\Example User\Notes.txt".to_string())
        );
    }

    #[test]
    fn test_parse_multiple_preview_tags() {
        let response = r#"
<preview type="file" path="/tmp/first.txt">First</preview>
<preview type="web" url="https://example.com">Second</preview>
"#;
        let tags = parse_preview_tags(response);
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].path, Some("/tmp/first.txt".to_string()));
        assert_eq!(tags[1].url, Some("https://example.com".to_string()));
    }

    #[test]
    fn test_strip_preview_tags() {
        let response = r#"Here's the file.
<preview type="file" path="/path/to/file">Caption</preview>
Check it out!"#;

        let stripped = strip_preview_tags(response);
        assert!(!stripped.contains("<preview"));
        assert!(!stripped.contains("</preview>"));
        assert!(stripped.contains("Here's the file."));
        assert!(stripped.contains("Check it out!"));
    }

    #[test]
    fn test_strip_preview_tags_with_attributes() {
        let response = r#"<preview type="file" path="C:\Docs\file.txt">Caption</preview>Done"#;
        let stripped = strip_preview_tags(response);
        assert_eq!(stripped, "Done");
    }

    #[test]
    fn test_file_type_detection() {
        assert_eq!(
            FileType::from_path(std::path::Path::new("test.txt")),
            FileType::Text
        );
        assert_eq!(
            FileType::from_path(std::path::Path::new("image.png")),
            FileType::Image
        );
        assert_eq!(
            FileType::from_path(std::path::Path::new("data.csv")),
            FileType::Csv
        );
        assert_eq!(
            FileType::from_path(std::path::Path::new("config.json")),
            FileType::Json
        );
    }
}
