//! ScriptSkill — wraps a parsed SKILL.md extension as a Skill trait impl.
//!
//! This module bridges external SKILL.md files into the existing SkillRegistry
//! system. When a skill is invoked, the Markdown body is returned as context
//! for the AI agent, which then uses its core tools (bash_execute, file_search,
//! etc.) to carry out the instructions.
//!
//! The `Skill` trait requires `&'static str` returns for id/name/description.
//! We use `Box::leak()` to satisfy this — each extension leaks ~200 bytes of
//! string data that lives for the process lifetime. For a desktop app with
//! dozens of extensions, this is negligible (~2-4KB total).

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput};

use super::parser::ExtensionDef;

/// A skill implementation backed by a SKILL.md file.
///
/// When executed, returns the Markdown body as instructions for the AI agent.
/// The agent then uses its core tools (bash, file search, etc.) to follow
/// those instructions — the skill itself doesn't execute commands directly.
///
/// This matches Pi's "progressive disclosure" pattern: the skill description
/// is loaded into the tool definition at startup (~100 tokens), and the full
/// body is only sent when the skill is actually invoked.
pub struct ScriptSkill {
    /// The parsed extension definition (owned).
    def: ExtensionDef,
    /// Leaked static reference to the skill ID string.
    /// Required by the Skill trait's `fn id() -> &'static str` signature.
    id_static: &'static str,
    /// Leaked static reference to the skill name.
    name_static: &'static str,
    /// Leaked static reference to the skill description.
    description_static: &'static str,
    /// Leaked static reference to the modes array.
    modes_static: &'static [Mode],
}

impl ScriptSkill {
    /// Create a ScriptSkill from a parsed ExtensionDef.
    ///
    /// This leaks the id, name, description, and modes to satisfy the Skill
    /// trait's `&'static str` / `&'static [Mode]` requirements. The leaked
    /// memory lives for the process lifetime — acceptable for a bounded
    /// number of extensions in a desktop app.
    pub fn from_def(def: ExtensionDef) -> Self {
        // Leak strings to get &'static str references.
        // Each string is small (id: ~20 bytes, name: ~30 bytes, desc: ~200 bytes).
        let id_static: &'static str = Box::leak(def.id.clone().into_boxed_str());
        let name_static: &'static str = Box::leak(def.name.clone().into_boxed_str());
        let description_static: &'static str =
            Box::leak(def.description.clone().into_boxed_str());
        let modes_static: &'static [Mode] = Box::leak(def.modes.clone().into_boxed_slice());

        Self {
            def,
            id_static,
            name_static,
            description_static,
            modes_static,
        }
    }
}

#[async_trait]
impl Skill for ScriptSkill {
    fn id(&self) -> &'static str {
        self.id_static
    }

    fn name(&self) -> &'static str {
        self.name_static
    }

    fn description(&self) -> &'static str {
        self.description_static
    }

    fn permission_level(&self) -> PermissionLevel {
        self.def.permission
    }

    fn modes(&self) -> &'static [Mode] {
        self.modes_static
    }

    /// Execute the skill by returning its Markdown body as instructions.
    ///
    /// The AI agent receives these instructions and uses its core tools
    /// (bash_execute, file_search, etc.) to carry them out. This follows
    /// Pi's design philosophy: skills provide context, not execution.
    ///
    /// The query from the user is included so the agent can tailor its
    /// execution of the skill instructions to the specific request.
    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        // Build the response: skill instructions + user's specific query.
        // The agent will interpret these instructions and act on them
        // using its available tools (bash, file search, web search, etc.).
        let response = format!(
            "## Skill: {}\n\n{}\n\n---\n**User request:** {}",
            self.def.name, self.def.body, input.query
        );

        Ok(SkillOutput::text(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::skill::Mode;
    use std::path::PathBuf;

    /// Create a test ExtensionDef for testing
    fn test_def() -> ExtensionDef {
        ExtensionDef {
            id: "test-skill".to_string(),
            name: "Test Skill".to_string(),
            description: "A test skill for unit tests".to_string(),
            permission: PermissionLevel::Safe,
            modes: vec![Mode::Find, Mode::Fix],
            body: "# Instructions\n\nDo the thing with `ls -la`.".to_string(),
            source_path: PathBuf::from("/tmp/test-skill/SKILL.md"),
        }
    }

    #[test]
    fn test_script_skill_metadata() {
        let skill = ScriptSkill::from_def(test_def());
        assert_eq!(skill.id(), "test-skill");
        assert_eq!(skill.name(), "Test Skill");
        assert_eq!(skill.description(), "A test skill for unit tests");
        assert_eq!(skill.permission_level(), PermissionLevel::Safe);
        assert_eq!(skill.modes().len(), 2);
    }

    #[tokio::test]
    async fn test_script_skill_execute() {
        let skill = ScriptSkill::from_def(test_def());
        let ctx = SkillContext::new(Mode::Find, PathBuf::from("/tmp"));
        let input = SkillInput::from_query("list my files");

        let result = skill.execute(input, &ctx).await.unwrap();
        let text = result.text.unwrap();

        // Should contain the skill body instructions
        assert!(text.contains("# Instructions"));
        assert!(text.contains("ls -la"));
        // Should contain the user's query
        assert!(text.contains("list my files"));
    }
}
