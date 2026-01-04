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
        match path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()).as_deref() {
            Some("txt" | "log" | "rs" | "py" | "js" | "ts" | "sh" | "toml" | "yaml" | "yml") => FileType::Text,
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

/// Content that can be displayed in the preview panel
#[derive(Clone, Debug)]
pub enum PreviewContent {
    /// Local file preview
    File {
        path: PathBuf,
        file_type: FileType,
    },
    /// Web page preview
    Web {
        url: String,
        title: Option<String>,
        screenshot: Option<PathBuf>,
        og_image: Option<String>,
        snippet: Option<String>,
    },
    /// Direct image
    Image {
        source: ImageSource,
    },
    /// ASCII art state
    Ascii {
        state: AsciiState,
    },
    /// Mode introduction
    ModeIntro {
        mode: String, // Mode name as string to avoid circular dependency
    },
    /// Error state
    Error {
        message: String,
        source: String,
    },
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

/// Parse preview tags from an agent response
///
/// Format: `<preview type="TYPE" [path="..."] [url="..."] [state="..."]>caption</preview>`
pub fn parse_preview_tag(response: &str) -> Option<ParsedPreviewTag> {
    // Simple regex-like parsing without regex crate
    let start = response.find("<preview ")?;
    let end = response.find("</preview>")?;

    if end <= start {
        return None;
    }

    let tag_end = response[start..].find('>')? + start;
    let attrs_str = &response[start + 9..tag_end]; // Skip "<preview "
    let caption = response[tag_end + 1..end].trim().to_string();

    // Parse attributes
    let mut content_type = String::new();
    let mut path = None;
    let mut url = None;
    let mut state = None;

    // Simple attribute parsing
    for part in attrs_str.split_whitespace() {
        if let Some((key, value)) = part.split_once('=') {
            let value = value.trim_matches('"').trim_matches('\'').to_string();
            match key {
                "type" => content_type = value,
                "path" => path = Some(value),
                "url" => url = Some(value),
                "state" => state = Some(value),
                _ => {}
            }
        }
    }

    if content_type.is_empty() {
        return None;
    }

    Some(ParsedPreviewTag {
        content_type,
        path,
        url,
        state,
        caption,
    })
}

/// Strip preview tags from a response for display
pub fn strip_preview_tags(response: &str) -> String {
    let mut result = response.to_string();

    // Keep stripping tags until none remain
    while let (Some(start), Some(end)) = (result.find("<preview "), result.find("</preview>")) {
        if end > start {
            result = format!("{}{}", &result[..start], &result[end + 10..]);
        } else {
            break;
        }
    }

    result.trim().to_string()
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
    fn test_file_type_detection() {
        assert_eq!(FileType::from_path(std::path::Path::new("test.txt")), FileType::Text);
        assert_eq!(FileType::from_path(std::path::Path::new("image.png")), FileType::Image);
        assert_eq!(FileType::from_path(std::path::Path::new("data.csv")), FileType::Csv);
        assert_eq!(FileType::from_path(std::path::Path::new("config.json")), FileType::Json);
    }
}
