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
        let response = r#"<preview type="file" path="C:\Users\Ben West\Notes.txt">
Context
</preview>"#;
        let tag = parse_preview_tag(response).unwrap();
        assert_eq!(tag.path, Some(r"C:\Users\Ben West\Notes.txt".to_string()));
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
