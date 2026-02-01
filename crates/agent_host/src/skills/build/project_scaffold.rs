//! Project Scaffold Skill - Quick project templates

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput};
use std::fs;
use std::path::PathBuf;

/// Quickly scaffold common project types
pub struct ProjectScaffoldSkill;

#[async_trait]
impl Skill for ProjectScaffoldSkill {
    fn id(&self) -> &'static str {
        "project_scaffold"
    }

    fn name(&self) -> &'static str {
        "Project Scaffold"
    }

    fn description(&self) -> &'static str {
        "Quickly create project structure for common project types (React, Rust, Python, etc.)"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Sensitive // Creates files and directories
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Build]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        // Parse template and name from query or params
        let template = input
            .params
            .get("template")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| {
                // Try to extract from query like "create react app myapp"
                let words: Vec<&str> = input.query.split_whitespace().collect();
                for (_i, word) in words.iter().enumerate() {
                    if ["react", "rust", "python", "node", "web"].contains(word) {
                        return word.to_string();
                    }
                }
                "node".to_string()
            });

        let name = input
            .params
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Try to extract project name from query
                input
                    .query
                    .split_whitespace()
                    .last()
                    .filter(|w| {
                        ![
                            "create", "scaffold", "new", "react", "rust", "python", "node", "web",
                            "project", "app",
                        ]
                        .contains(w)
                    })
                    .unwrap_or("my-project")
                    .to_string()
            });

        let parent_dir = input
            .params
            .get("directory")
            .and_then(|v| v.as_str())
            .map(|s| PathBuf::from(s))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let project_dir = parent_dir.join(&name);

        // Create project directory
        if project_dir.exists() {
            return Ok(SkillOutput::text(format!(
                "Directory already exists: {}",
                project_dir.display()
            )));
        }

        fs::create_dir_all(&project_dir)
            .map_err(|e| anyhow::anyhow!("Failed to create directory: {}", e))?;

        match template.as_str() {
            "react" | "web" => {
                // Create React/Web project structure
                let dirs = ["src", "public", "src/components", "src/hooks"];
                for dir in dirs {
                    fs::create_dir_all(project_dir.join(dir)).ok();
                }

                // package.json
                let package_json = format!(
                    r#"{{
  "name": "{}",
  "version": "0.1.0",
  "private": true,
  "scripts": {{
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  }},
  "dependencies": {{
    "react": "^18.2.0",
    "react-dom": "^18.2.0"
  }},
  "devDependencies": {{
    "@vitejs/plugin-react": "^4.0.0",
    "vite": "^5.0.0"
  }}
}}"#,
                    name
                );
                fs::write(project_dir.join("package.json"), &package_json).ok();

                // Basic index.html
                let index_html = format!(
                    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
</head>
<body>
    <div id="root"></div>
    <script type="module" src="/src/main.jsx"></script>
</body>
</html>"#,
                    name
                );
                fs::write(project_dir.join("index.html"), &index_html).ok();

                // src/main.jsx
                let main_jsx = r#"import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)"#;
                fs::write(project_dir.join("src/main.jsx"), main_jsx).ok();

                // src/App.jsx
                let app_jsx = format!(
                    r#"export default function App() {{
  return (
    <div>
      <h1>{}</h1>
      <p>Edit src/App.jsx to get started</p>
    </div>
  )
}}"#,
                    name
                );
                fs::write(project_dir.join("src/App.jsx"), &app_jsx).ok();
            }

            "rust" => {
                // Create Rust project with cargo
                std::process::Command::new("cargo")
                    .args(["init", "--name", &name])
                    .current_dir(&project_dir)
                    .output()
                    .ok();
            }

            "python" => {
                // Create Python project structure
                let src_dir = project_dir.join(name.replace("-", "_"));
                fs::create_dir_all(&src_dir).ok();
                fs::create_dir_all(project_dir.join("tests")).ok();

                // __init__.py
                fs::write(src_dir.join("__init__.py"), "").ok();

                // main.py
                let main_py = format!(
                    r#"def main():
    print("Hello from {}!")

if __name__ == "__main__":
    main()
"#,
                    name
                );
                fs::write(src_dir.join("main.py"), &main_py).ok();

                // pyproject.toml
                let pyproject = format!(
                    r#"[project]
name = "{}"
version = "0.1.0"
description = ""
readme = "README.md"
requires-python = ">=3.9"
dependencies = []

[project.scripts]
{} = "{}:main"
"#,
                    name,
                    name,
                    name.replace("-", "_")
                );
                fs::write(project_dir.join("pyproject.toml"), &pyproject).ok();
            }

            "node" => {
                // Create Node.js project
                fs::create_dir_all(project_dir.join("src")).ok();

                let package_json = format!(
                    r#"{{
  "name": "{}",
  "version": "0.1.0",
  "type": "module",
  "main": "src/index.js",
  "scripts": {{
    "start": "node src/index.js",
    "dev": "node --watch src/index.js"
  }}
}}"#,
                    name
                );
                fs::write(project_dir.join("package.json"), &package_json).ok();

                let index_js = format!(
                    r#"console.log('Hello from {}!')
"#,
                    name
                );
                fs::write(project_dir.join("src/index.js"), &index_js).ok();
            }

            _ => {
                return Ok(SkillOutput::text(format!(
                    "Unknown template: {}\n\n\
                    Available templates:\n\
                    - react: React + Vite project\n\
                    - rust: Cargo project\n\
                    - python: Python package\n\
                    - node: Node.js project\n\
                    - web: Same as react",
                    template
                )));
            }
        }

        // Add common files
        let readme = format!("# {}\n\nCreated with Little Helper\n", name);
        fs::write(project_dir.join("README.md"), &readme).ok();

        let gitignore = match template.as_str() {
            "rust" => "target/\nCargo.lock\n",
            "python" => "__pycache__/\n*.pyc\n.venv/\ndist/\n*.egg-info/\n",
            _ => "node_modules/\ndist/\n.env\n",
        };
        fs::write(project_dir.join(".gitignore"), gitignore).ok();

        Ok(SkillOutput::text(format!(
            "Created {} project: {}\n\n\
            Location: {}\n\n\
            Next steps:\n\
            1. cd {}\n\
            2. {}\n\
            3. Start building!",
            template,
            name,
            project_dir.display(),
            name,
            match template.as_str() {
                "rust" => "cargo run",
                "python" => "python -m venv .venv && source .venv/bin/activate",
                _ => "npm install && npm run dev",
            }
        )))
    }
}
