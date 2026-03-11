//! LLM provider routing and client implementations.
//!
//! This crate isolates all LLM API integration behind a single [`router::ProviderRouter`]
//! that tries providers in user-configured preference order, falling back automatically
//! on failure. Each provider module ([`openai`], [`anthropic`], [`gemini`], [`ollama`])
//! implements the same contract: accept `Vec<ChatMessage>`, return `Result<String>`.
//!
//! Additional modules:
//! - [`oauth_helper`] -- Browser-based OAuth 2.0 + PKCE flow for cloud providers.
//! - [`external`] -- Registry for optional tool providers (Playwright, Canva, etc.).

pub mod anthropic;
pub mod external;
pub mod gemini;
pub mod oauth_helper;
pub mod ollama;
pub mod openai;
pub mod router;
