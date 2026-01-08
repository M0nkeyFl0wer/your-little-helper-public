//! Web Preview Service for the Interactive Preview Companion feature.
//!
//! This module provides web page preview capabilities including:
//! - Screenshot capture using wkhtmltoimage (if available)
//! - Open Graph metadata extraction (title, description, og:image)
//! - Text-only fallback (title + snippet + URL)
//! - Caching to avoid repeated captures

use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Web preview result with screenshot or fallback content
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebPreview {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub og_image: Option<String>,
    pub screenshot_path: Option<PathBuf>,
    pub snippet: Option<String>,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

/// Cache entry with expiration
struct CacheEntry {
    preview: WebPreview,
    expires_at: Instant,
}

/// Web preview service with caching
pub struct WebPreviewService {
    cache: Mutex<HashMap<String, CacheEntry>>,
    cache_dir: PathBuf,
    cache_duration: Duration,
    wkhtmltoimage_available: bool,
}

impl WebPreviewService {
    /// Create a new web preview service
    pub fn new() -> Self {
        let cache_dir = Self::get_cache_dir();
        let _ = fs::create_dir_all(&cache_dir);

        Self {
            cache: Mutex::new(HashMap::new()),
            cache_dir,
            cache_duration: Duration::from_secs(15 * 60), // 15 minutes
            wkhtmltoimage_available: Self::check_wkhtmltoimage(),
        }
    }

    /// Get the cache directory for screenshots
    fn get_cache_dir() -> PathBuf {
        directories::ProjectDirs::from("com.local", "Little Helper", "LittleHelper")
            .map(|p| p.cache_dir().join("web_previews"))
            .unwrap_or_else(|| PathBuf::from("./cache/web_previews"))
    }

    /// Check if wkhtmltoimage is available on the system
    fn check_wkhtmltoimage() -> bool {
        Command::new("wkhtmltoimage")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get a preview for a URL, using cache if available
    pub async fn get_preview(&self, url: &str) -> Result<WebPreview> {
        // Check cache first
        if let Some(cached) = self.get_cached(url) {
            return Ok(cached);
        }

        // Fetch fresh preview
        let preview = self.fetch_preview(url).await?;

        // Cache the result
        self.cache_preview(url, preview.clone());

        Ok(preview)
    }

    /// Get cached preview if still valid
    fn get_cached(&self, url: &str) -> Option<WebPreview> {
        let cache = self.cache.lock().ok()?;
        let entry = cache.get(url)?;

        if entry.expires_at > Instant::now() {
            Some(entry.preview.clone())
        } else {
            None
        }
    }

    /// Cache a preview result
    fn cache_preview(&self, url: &str, preview: WebPreview) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(
                url.to_string(),
                CacheEntry {
                    preview,
                    expires_at: Instant::now() + self.cache_duration,
                },
            );
        }
    }

    /// Fetch preview for a URL with fallback chain
    async fn fetch_preview(&self, url: &str) -> Result<WebPreview> {
        // Try to fetch the page content for metadata
        let (title, description, og_image) = self.fetch_metadata(url).await.unwrap_or_default();

        // Try screenshot capture if available
        let screenshot_path = if self.wkhtmltoimage_available {
            self.capture_screenshot(url).await.ok()
        } else {
            None
        };

        // Generate snippet from description or URL
        let snippet = description
            .clone()
            .or_else(|| Some(format!("Source: {}", url)));

        Ok(WebPreview {
            url: url.to_string(),
            title,
            description,
            og_image,
            screenshot_path,
            snippet,
            fetched_at: chrono::Utc::now(),
        })
    }

    /// Fetch Open Graph and basic metadata from a URL
    async fn fetch_metadata(
        &self,
        url: &str,
    ) -> Result<(Option<String>, Option<String>, Option<String>)> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (compatible; LittleHelper/1.0)")
            .build()?;

        let response = client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP error: {}", response.status()));
        }

        let html = response.text().await?;

        // Extract title
        let title = self.extract_title(&html);

        // Extract Open Graph metadata
        let og_title = self.extract_og_meta(&html, "og:title");
        let og_description = self.extract_og_meta(&html, "og:description");
        let og_image = self.extract_og_meta(&html, "og:image");

        // Extract meta description as fallback
        let meta_description = self.extract_meta_description(&html);

        Ok((
            og_title.or(title),
            og_description.or(meta_description),
            og_image,
        ))
    }

    /// Extract <title> from HTML
    fn extract_title(&self, html: &str) -> Option<String> {
        let re = Regex::new(r"(?i)<title[^>]*>([^<]+)</title>").ok()?;
        re.captures(html)
            .and_then(|c| c.get(1))
            .map(|m| html_decode(m.as_str().trim()))
    }

    /// Extract Open Graph meta tag content
    fn extract_og_meta(&self, html: &str, property: &str) -> Option<String> {
        let pattern = format!(
            r#"(?i)<meta[^>]*property=["']{}["'][^>]*content=["']([^"']+)["']"#,
            regex::escape(property)
        );
        let re = Regex::new(&pattern).ok()?;

        if let Some(caps) = re.captures(html) {
            return caps.get(1).map(|m| html_decode(m.as_str()));
        }

        // Try alternate order (content before property)
        let pattern_alt = format!(
            r#"(?i)<meta[^>]*content=["']([^"']+)["'][^>]*property=["']{}["']"#,
            regex::escape(property)
        );
        let re_alt = Regex::new(&pattern_alt).ok()?;
        re_alt
            .captures(html)
            .and_then(|c| c.get(1))
            .map(|m| html_decode(m.as_str()))
    }

    /// Extract meta description
    fn extract_meta_description(&self, html: &str) -> Option<String> {
        let re =
            Regex::new(r#"(?i)<meta[^>]*name=["']description["'][^>]*content=["']([^"']+)["']"#)
                .ok()?;

        if let Some(caps) = re.captures(html) {
            return caps.get(1).map(|m| html_decode(m.as_str()));
        }

        // Try alternate order
        let re_alt =
            Regex::new(r#"(?i)<meta[^>]*content=["']([^"']+)["'][^>]*name=["']description["']"#)
                .ok()?;
        re_alt
            .captures(html)
            .and_then(|c| c.get(1))
            .map(|m| html_decode(m.as_str()))
    }

    /// Capture screenshot using wkhtmltoimage
    async fn capture_screenshot(&self, url: &str) -> Result<PathBuf> {
        // Generate a unique filename based on URL hash
        let url_hash = format!("{:x}", md5_hash(url));
        let screenshot_path = self.cache_dir.join(format!("{}.png", url_hash));

        // Check if screenshot already exists and is recent
        if screenshot_path.exists() {
            if let Ok(metadata) = fs::metadata(&screenshot_path) {
                if let Ok(modified) = metadata.modified() {
                    if modified.elapsed().unwrap_or(Duration::MAX) < self.cache_duration {
                        return Ok(screenshot_path);
                    }
                }
            }
        }

        // Capture screenshot using wkhtmltoimage
        let output = tokio::process::Command::new("wkhtmltoimage")
            .args([
                "--quality",
                "80",
                "--width",
                "1200",
                "--height",
                "800",
                "--disable-javascript",
                "--load-error-handling",
                "ignore",
                "--load-media-error-handling",
                "ignore",
                url,
                screenshot_path.to_str().unwrap_or("screenshot.png"),
            ])
            .output()
            .await?;

        if output.status.success() && screenshot_path.exists() {
            Ok(screenshot_path)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Screenshot capture failed: {}", stderr))
        }
    }

    /// Check if wkhtmltoimage is available
    pub fn has_screenshot_support(&self) -> bool {
        self.wkhtmltoimage_available
    }

    /// Clear expired cache entries
    pub fn cleanup_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            let now = Instant::now();
            cache.retain(|_, entry| entry.expires_at > now);
        }

        // Also cleanup old screenshot files
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        // Remove files older than 1 hour
                        if modified.elapsed().unwrap_or(Duration::MAX) > Duration::from_secs(3600) {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }
    }
}

impl Default for WebPreviewService {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple HTML entity decoding
fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

/// Simple hash function for URL -> filename
fn md5_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let service = WebPreviewService::new();
        let html = "<html><head><title>Test Page</title></head></html>";
        assert_eq!(service.extract_title(html), Some("Test Page".to_string()));
    }

    #[test]
    fn test_extract_og_meta() {
        let service = WebPreviewService::new();
        let html = r#"<html><head><meta property="og:title" content="OG Title"></head></html>"#;
        assert_eq!(
            service.extract_og_meta(html, "og:title"),
            Some("OG Title".to_string())
        );
    }

    #[test]
    fn test_html_decode() {
        assert_eq!(html_decode("Hello &amp; World"), "Hello & World");
        assert_eq!(html_decode("&lt;tag&gt;"), "<tag>");
    }
}
