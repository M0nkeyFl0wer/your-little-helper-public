use shared::skill::SkillInput;
use std::path::PathBuf;

/// Resolve target folder from params (folder or directory)
pub fn resolve_target_folder(input: &SkillInput) -> PathBuf {
    let folder = input
        .params
        .get("folder")
        .and_then(|v| v.as_str())
        .or_else(|| input.params.get("directory").and_then(|v| v.as_str()))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    folder.unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// Resolve spec-kit assistant path from skill input params or defaults
pub fn resolve_spec_kit_path(input: &SkillInput) -> PathBuf {
    if let Some(path) = input
        .params
        .get("spec_kit_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
    {
        return PathBuf::from(path);
    }

    dirs::home_dir()
        .map(|h| h.join("Projects/spec-kit-assistant/spec-assistant.js"))
        .unwrap_or_default()
}
