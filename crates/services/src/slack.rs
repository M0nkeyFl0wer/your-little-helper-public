//! Slack integration for Little Helper
//!
//! Supports:
//! - Incoming webhooks for notifications
//! - Posting draft-ready messages to channels

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Slack message payload for incoming webhooks
#[derive(Debug, Serialize)]
pub struct SlackMessage {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_emoji: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<SlackBlock>>,
}

/// Slack block for rich formatting
#[derive(Debug, Serialize)]
pub struct SlackBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<SlackText>,
}

#[derive(Debug, Serialize)]
pub struct SlackText {
    #[serde(rename = "type")]
    pub text_type: String,
    pub text: String,
}

/// Slack webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlackConfig {
    /// Incoming webhook URL (from Slack app settings)
    pub webhook_url: Option<String>,
    /// Default channel to post to (optional, webhook usually has default)
    pub default_channel: Option<String>,
    /// Bot username to display
    pub username: Option<String>,
}

impl SlackConfig {
    /// Load config from environment or config file
    pub fn from_env() -> Self {
        Self {
            webhook_url: std::env::var("SLACK_WEBHOOK_URL").ok(),
            default_channel: std::env::var("SLACK_CHANNEL").ok(),
            username: Some("Little Helper".to_string()),
        }
    }
    
    /// Check if Slack is configured
    pub fn is_configured(&self) -> bool {
        self.webhook_url.is_some()
    }
}

/// Send a simple text message to Slack
pub async fn send_message(config: &SlackConfig, message: &str) -> Result<()> {
    let webhook_url = config.webhook_url.as_ref()
        .ok_or_else(|| anyhow!("Slack webhook URL not configured"))?;
    
    let payload = SlackMessage {
        text: message.to_string(),
        channel: config.default_channel.clone(),
        username: config.username.clone(),
        icon_emoji: Some(":robot_face:".to_string()),
        blocks: None,
    };
    
    let client = reqwest::Client::new();
    let response = client.post(webhook_url)
        .json(&payload)
        .send()
        .await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(anyhow!("Slack API error: {} - {}", status, body))
    }
}

/// Send a "drafts ready" notification with rich formatting
pub async fn notify_drafts_ready(
    config: &SlackConfig,
    draft_count: usize,
    draft_folder: &str,
    campaign: Option<&str>,
) -> Result<()> {
    let webhook_url = config.webhook_url.as_ref()
        .ok_or_else(|| anyhow!("Slack webhook URL not configured"))?;
    
    let campaign_text = campaign
        .map(|c| format!(" for *{}*", c))
        .unwrap_or_default();
    
    let blocks = vec![
        SlackBlock {
            block_type: "section".to_string(),
            text: Some(SlackText {
                text_type: "mrkdwn".to_string(),
                text: format!(
                    ":memo: *{} new draft{}{}*\n\nReady for review in Google Drive.",
                    draft_count,
                    if draft_count == 1 { "" } else { "s" },
                    campaign_text
                ),
            }),
        },
        SlackBlock {
            block_type: "section".to_string(),
            text: Some(SlackText {
                text_type: "mrkdwn".to_string(),
                text: format!(":file_folder: `{}`", draft_folder),
            }),
        },
    ];
    
    let payload = SlackMessage {
        text: format!("{} new draft(s) ready for review", draft_count),
        channel: config.default_channel.clone(),
        username: config.username.clone(),
        icon_emoji: Some(":memo:".to_string()),
        blocks: Some(blocks),
    };
    
    let client = reqwest::Client::new();
    let response = client.post(webhook_url)
        .json(&payload)
        .send()
        .await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(anyhow!("Slack API error: {} - {}", status, body))
    }
}

/// Send a content generation complete notification
pub async fn notify_content_generated(
    config: &SlackConfig,
    persona: &str,
    platform: &str,
    preview: &str,
) -> Result<()> {
    let webhook_url = config.webhook_url.as_ref()
        .ok_or_else(|| anyhow!("Slack webhook URL not configured"))?;
    
    // Truncate preview if too long
    let preview_text = if preview.len() > 300 {
        format!("{}...", &preview[..300])
    } else {
        preview.to_string()
    };
    
    let blocks = vec![
        SlackBlock {
            block_type: "section".to_string(),
            text: Some(SlackText {
                text_type: "mrkdwn".to_string(),
                text: format!(
                    ":sparkles: *New {} content generated*\n\n*Persona:* {}\n*Platform:* {}",
                    platform, persona, platform
                ),
            }),
        },
        SlackBlock {
            block_type: "section".to_string(),
            text: Some(SlackText {
                text_type: "mrkdwn".to_string(),
                text: format!("```{}```", preview_text),
            }),
        },
    ];
    
    let payload = SlackMessage {
        text: format!("New {} content for {} persona", platform, persona),
        channel: config.default_channel.clone(),
        username: config.username.clone(),
        icon_emoji: Some(":sparkles:".to_string()),
        blocks: Some(blocks),
    };
    
    let client = reqwest::Client::new();
    let response = client.post(webhook_url)
        .json(&payload)
        .send()
        .await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(anyhow!("Slack API error: {} - {}", status, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_slack_config_from_env() {
        // Just test that it doesn't panic
        let config = SlackConfig::from_env();
        assert!(config.username.is_some());
    }
    
    #[test]
    fn test_is_configured() {
        let config = SlackConfig::default();
        assert!(!config.is_configured());
        
        let config = SlackConfig {
            webhook_url: Some("https://hooks.slack.com/...".to_string()),
            ..Default::default()
        };
        assert!(config.is_configured());
    }
}
