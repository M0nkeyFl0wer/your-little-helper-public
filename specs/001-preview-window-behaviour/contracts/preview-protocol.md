# Preview Protocol Contract

**Feature**: 001-preview-window-behaviour
**Date**: 2026-01-04

## Overview

This document defines the protocol for AI agents to request preview content updates. The protocol uses XML-like tags embedded in agent responses that are parsed by the app.

## Tag Format

### Basic Structure

```xml
<preview type="TYPE" [attributes...]>
Optional caption or description
</preview>
```

### Supported Types

#### 1. File Preview

Display a local file in the preview panel.

```xml
<preview type="file" path="/absolute/path/to/file.csv">
This file contains the sales data you requested.
</preview>
```

**Attributes**:
| Attribute | Required | Description |
|-----------|----------|-------------|
| `type` | Yes | Must be "file" |
| `path` | Yes | Absolute path to local file |

**Behavior**:
- App detects file type and uses appropriate viewer
- If file not found, shows error state
- Caption displayed below preview

---

#### 2. Web Preview

Display a website preview (screenshot or metadata).

```xml
<preview type="web" url="https://example.com/article">
Key information from this source about climate change.
</preview>
```

**Attributes**:
| Attribute | Required | Description |
|-----------|----------|-------------|
| `type` | Yes | Must be "web" |
| `url` | Yes | Full URL including protocol |

**Behavior**:
- App attempts to capture screenshot (if wkhtmltoimage available)
- Falls back to Open Graph metadata extraction
- Falls back to title + URL display
- Click opens URL in default browser

---

#### 3. Image Preview

Display an image directly.

```xml
<preview type="image" url="https://example.com/chart.png">
This chart shows the trend over time.
</preview>
```

Or for local images:

```xml
<preview type="image" path="/path/to/screenshot.png">
Screenshot of the error message.
</preview>
```

**Attributes**:
| Attribute | Required | Description |
|-----------|----------|-------------|
| `type` | Yes | Must be "image" |
| `url` | One of | URL to remote image |
| `path` | One of | Path to local image |

**Behavior**:
- Remote images are downloaded and cached
- Local images loaded directly
- Zoom/scroll/fullscreen controls available

---

#### 4. ASCII Art State

Display an ASCII art state (typically used for status).

```xml
<preview type="ascii" state="success">
Task completed successfully!
</preview>
```

**Attributes**:
| Attribute | Required | Description |
|-----------|----------|-------------|
| `type` | Yes | Must be "ascii" |
| `state` | Yes | One of: welcome, thinking, success, error |

**Behavior**:
- Displays pre-defined ASCII art for state
- Adapts to light/dark theme
- Caption displayed below art

---

## Parsing Rules

### Tag Detection

1. Tags are detected using regex: `<preview\s+([^>]+)>([\s\S]*?)</preview>`
2. Multiple tags in one response are allowed (last one takes precedence for display)
3. Tags can appear anywhere in the response (inline with text)

### Attribute Parsing

1. Attributes are key="value" pairs
2. Values must be quoted (single or double quotes)
3. Unknown attributes are ignored

### Escaping

1. Content between tags is NOT escaped (can contain markdown)
2. Paths/URLs with special characters should work as-is
3. Angle brackets in paths should be URL-encoded if needed

## Examples

### Research Response with Web Preview

```
I found several relevant sources about renewable energy.

<preview type="web" url="https://www.iea.org/reports/renewables-2024">
The IEA's 2024 report shows renewable capacity grew 50% year-over-year.
</preview>

Key findings:
1. Solar is now the cheapest energy source in most regions
2. Wind capacity additions reached record levels
3. Investment in clean energy surpassed fossil fuels
```

### File Search Response

```
I found the document you were looking for:

<preview type="file" path="/Users/flower/Documents/Reports/Q4-2025.pdf">
Q4 2025 Financial Report
</preview>

This is the quarterly financial report from Q4 2025. Would you like me to summarize the key points?
```

### Error with Helpful Art

```
I wasn't able to find that file. Let me show you what went wrong:

<preview type="ascii" state="error">
File not found at the specified path
</preview>

Could you double-check the file path? You might also try:
- Searching for the file by name
- Checking if the file was moved
```

## Implementation Notes

### For Agent Prompts

Include in mode-specific prompts:
```
When you want to show a file, image, or web page in the preview panel, use the <preview> tag:
- <preview type="file" path="...">caption</preview> for local files
- <preview type="web" url="...">caption</preview> for websites
- <preview type="image" url="..." or path="...">caption</preview> for images

The preview will appear alongside your response, helping the user see what you're referring to.
```

### For App Parser

```rust
pub struct PreviewTag {
    pub content_type: String,
    pub path: Option<String>,
    pub url: Option<String>,
    pub state: Option<String>,
    pub caption: String,
}

pub fn parse_preview_tags(response: &str) -> Option<PreviewTag> {
    let re = Regex::new(r#"<preview\s+([^>]+)>([\s\S]*?)</preview>"#).ok()?;
    let caps = re.captures(response)?;

    let attrs = parse_attributes(&caps[1]);
    let caption = caps[2].trim().to_string();

    Some(PreviewTag {
        content_type: attrs.get("type")?.clone(),
        path: attrs.get("path").cloned(),
        url: attrs.get("url").cloned(),
        state: attrs.get("state").cloned(),
        caption,
    })
}
```
