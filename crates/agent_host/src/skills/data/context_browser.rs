//! Context Browser Skill
//!
//! Allows users to search, browse, and use context documents (personas, research, templates)
//! through natural language queries.
//!
//! This skill integrates the ContextManager with the AI system so users can:
//! - Search their knowledge base: "Find my notes on Rust programming"
//! - Use personas: "Switch to the Tech Savvy Early Adopter persona"
//! - Apply templates: "Use the weekly status template"
//! - Reference research: "What do I know about file organization?"

use crate::context_manager::{ContextManager, ContextType, DistributionLevel};
use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput, SuggestedAction};
use std::collections::HashMap;

/// Context Browser Skill
pub struct ContextBrowser {
    /// The context manager instance
    manager: ContextManager,
}

impl ContextBrowser {
    /// Create a new context browser
    pub fn new() -> Result<Self> {
        let manager = ContextManager::new(ContextManager::default_dir())?;
        Ok(Self { manager })
    }
    
    /// Handle search queries
    fn handle_search(&mut self, query: &str, mode: Option<Mode>) -> Result<SkillOutput> {
        let results = self.manager.search(query, mode);
        
        if results.is_empty() {
            return Ok(SkillOutput {
                result_type: shared::skill::ResultType::Text,
                text: Some(format!("No documents found matching '{}'. Try different keywords or add new documents.", query)),
                files: Vec::new(),
                data: None,
                citations: Vec::new(),
                suggested_actions: vec![
                    SuggestedAction {
                        label: "📁 Browse all context documents".to_string(),
                        skill_id: "context_browser".to_string(),
                        params: {
                            let mut p = HashMap::new();
                            p.insert("action".to_string(), serde_json::json!("list_all"));
                            p
                        },
                    }
                ],
            });
        }
        
        let mut output = String::new();
        output.push_str(&format!("## 🔍 Search Results for '{}'", query));
        output.push_str(&format!("\n\nFound {} relevant documents:\n\n", results.len()));
        
        let mut suggested_actions = Vec::new();
        
        for (i, result) in results.iter().take(5).enumerate() {
            let doc = &result.document;
            
            output.push_str(&format!("### {}. {} {}\n", 
                i + 1,
                doc.context_type.display_name(),
                doc.name));
            
            if !doc.description.is_empty() {
                output.push_str(&format!("_{}_\n", doc.description));
            }
            
            output.push_str(&format!("Relevance: {}/100\n", result.relevance_score));
            
            // Show excerpts
            if !result.excerpts.is_empty() {
                output.push_str("\n**Matching content:**\n");
                for excerpt in &result.excerpts {
                    output.push_str(&format!("> {}\n", excerpt));
                }
            }
            
            output.push('\n');
            
            // Add action to view full document
            let mut params = HashMap::new();
            params.insert("action".to_string(), serde_json::json!("view"));
            params.insert("doc_id".to_string(), serde_json::json!(doc.id.clone()));
            
            suggested_actions.push(SuggestedAction {
                label: format!("📄 View full: {}", doc.name),
                skill_id: "context_browser".to_string(),
                params,
            });
        }
        
        // Add browse action
        suggested_actions.push(SuggestedAction {
            label: "📁 Browse by category".to_string(),
            skill_id: "context_browser".to_string(),
            params: {
                let mut p = HashMap::new();
                p.insert("action".to_string(), serde_json::json!("browse_categories"));
                p
            },
        });
        
        Ok(SkillOutput {
            result_type: shared::skill::ResultType::Text,
            text: Some(output),
            files: Vec::new(),
            data: Some(serde_json::to_value(&results)?),
            citations: Vec::new(),
            suggested_actions,
        })
    }
    
    /// List all documents
    fn handle_list_all(&mut self) -> Result<SkillOutput> {
        let docs = self.manager.all_documents();
        let by_type = self.manager.count_by_type();
        
        let mut output = String::new();
        output.push_str("## 📚 Your Context Library\n\n");
        output.push_str(&format!("**{} documents** organized by type:\n\n", docs.len()));
        
        // Show counts by type
        for (context_type, count) in by_type {
            output.push_str(&format!("{} **{}**\n", 
                context_type.display_name(),
                count));
        }
        
        output.push_str("\n### All Documents\n\n");
        
        let mut suggested_actions = Vec::new();
        
        for (i, doc) in docs.iter().enumerate() {
            output.push_str(&format!("{}. {} - {}\n",
                i + 1,
                doc.context_type.display_name(),
                doc.name));
            
            if !doc.description.is_empty() {
                output.push_str(&format!("   _{}_\n", doc.description));
            }
            
            // Add view action for each
            let mut params = HashMap::new();
            params.insert("action".to_string(), serde_json::json!("view"));
            params.insert("doc_id".to_string(), serde_json::json!(doc.id.clone()));
            
            suggested_actions.push(SuggestedAction {
                label: format!("View {}", doc.name),
                skill_id: "context_browser".to_string(),
                params,
            });
        }
        
        output.push_str("\n💡 **Tip:** You can search with natural language like:\n");
        output.push_str("• \"Find my notes on Rust\"\n");
        output.push_str("• \"What personas do I have?\"\n");
        output.push_str("• \"Show me the weekly status template\"\n");
        
        Ok(SkillOutput {
            result_type: shared::skill::ResultType::Text,
            text: Some(output),
            files: Vec::new(),
            data: None,
            citations: Vec::new(),
            suggested_actions,
        })
    }
    
    /// View a specific document
    fn handle_view(&mut self, doc_id: &str) -> Result<SkillOutput> {
        let content = self.manager.get_content(doc_id)?;
        
        if let Some(content) = content {
            // Find the document to get metadata
            let doc = self.manager.all_documents()
                .into_iter()
                .find(|d| d.id == doc_id);
            
            let mut output = String::new();
            
            if let Some(doc) = doc {
                output.push_str(&format!("## {} {}\n\n", 
                    doc.context_type.display_name(),
                    doc.name));
                
                if !doc.description.is_empty() {
                    output.push_str(&format!("_{}_\n\n", doc.description));
                }
            }
            
            output.push_str(&content);
            
            Ok(SkillOutput {
                result_type: shared::skill::ResultType::Text,
                text: Some(output),
                files: Vec::new(),
                data: None,
                citations: Vec::new(),
                suggested_actions: vec![
                    SuggestedAction {
                        label: "🔍 Search other documents".to_string(),
                        skill_id: "context_browser".to_string(),
                        params: HashMap::new(),
                    }
                ],
            })
        } else {
            Ok(SkillOutput {
                result_type: shared::skill::ResultType::Text,
                text: Some("Document not found. It may have been removed.".to_string()),
                files: Vec::new(),
                data: None,
                citations: Vec::new(),
                suggested_actions: vec![
                    SuggestedAction {
                        label: "📁 List all documents".to_string(),
                        skill_id: "context_browser".to_string(),
                        params: {
                            let mut p = HashMap::new();
                            p.insert("action".to_string(), serde_json::json!("list_all"));
                            p
                        },
                    }
                ],
            })
        }
    }
    
    /// Browse by category
    fn handle_browse_categories(&self) -> Result<SkillOutput> {
        let mut output = String::new();
        output.push_str("## 📂 Browse by Category\n\n");
        output.push_str("Select a category to explore:\n\n");
        
        let categories = [
            (ContextType::Persona, "User personas for content creation and targeting"),
            (ContextType::Research, "Research notes, data, and findings"),
            (ContextType::Skill, "Skill guides and how-to documents"),
            (ContextType::Template, "Reusable templates for documents"),
            (ContextType::Reference, "Reference materials and guides"),
            (ContextType::Campaign, "Campaign-specific context and documents"),
        ];
        
        let mut suggested_actions = Vec::new();
        
        for (category, description) in categories {
            let docs = self.manager.get_by_type(category);
            output.push_str(&format!("### {}\n", category.display_name()));
            output.push_str(&format!("_{}_\n", description));
            output.push_str(&format!("**{} documents**\n", docs.len()));
            
            if !docs.is_empty() {
                output.push_str("Examples:\n");
                for doc in docs.iter().take(3) {
                    output.push_str(&format!("• {}\n", doc.name));
                }
                if docs.len() > 3 {
                    output.push_str(&format!("• ... and {} more\n", docs.len() - 3));
                }
            }
            
            output.push('\n');
            
            // Add browse action
            let mut params = HashMap::new();
            params.insert("action".to_string(), serde_json::json!("filter_by_type"));
            params.insert("context_type".to_string(), serde_json::json!(format!("{:?}", category)));
            
            suggested_actions.push(SuggestedAction {
                label: format!("View all {} documents", category.display_name()),
                skill_id: "context_browser".to_string(),
                params,
            });
        }
        
        Ok(SkillOutput {
            result_type: shared::skill::ResultType::Text,
            text: Some(output),
            files: Vec::new(),
            data: None,
            citations: Vec::new(),
            suggested_actions,
        })
    }
    
    /// Setup beta package if empty
    fn setup_if_empty(&mut self) -> Result<()> {
        let docs = self.manager.all_documents();
        if docs.is_empty() {
            println!("Setting up internal context package...");
            self.manager.setup_package(DistributionLevel::Internal)?;
            println!("✓ Context package installed with {} sample documents", self.manager.all_documents().len());
        }
        Ok(())
    }
}

impl Default for ContextBrowser {
    fn default() -> Self {
        Self::new().expect("Failed to create ContextBrowser")
    }
}

#[async_trait]
impl Skill for ContextBrowser {
    fn id(&self) -> &'static str {
        "context_browser"
    }
    
    fn name(&self) -> &'static str {
        "Context Browser"
    }
    
    fn description(&self) -> &'static str {
        "Search and browse your personal knowledge base (personas, research, templates)"
    }
    
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Research, Mode::Content, Mode::Data]
    }
    
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }
    
    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> anyhow::Result<SkillOutput> {
        // Note: We need mutable access but execute takes &self
        // In a real implementation, you'd use interior mutability (Mutex/RefCell)
        // For now, we'll create a new browser instance each time (not ideal for performance)
        let mut browser = ContextBrowser::new()?;
        
        // Setup beta package if this is first use
        browser.setup_if_empty()?;
        
        // Get action from params
        let action = input.params.get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("search");
        
        match action {
            "search" => {
                let query = input.params.get("query")
                    .and_then(|q| q.as_str())
                    .unwrap_or(&input.query);
                let mode = input.params.get("mode")
                    .and_then(|m| serde_json::from_value(m.clone()).ok());
                browser.handle_search(query, mode)
            }
            "list_all" => browser.handle_list_all(),
            "view" => {
                let doc_id = input.params.get("doc_id")
                    .and_then(|id| id.as_str())
                    .unwrap_or("");
                browser.handle_view(doc_id)
            }
            "browse_categories" => browser.handle_browse_categories(),
            _ => {
                // Default to search with the input query
                browser.handle_search(&input.query, None)
            }
        }
    }
}
