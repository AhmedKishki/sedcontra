//! Agent definitions: a prompt in a Markdown file, looked up in layers, and the model the
//! configuration assigns to the agent. The prompt names no provider and no model.

use std::path::PathBuf;

use anyhow::{Context, anyhow, bail};

use crate::config::{Config, ModelSettings, config_dir};

const STANDARD_FILE: &str = "critique-standard.md";
const STANDARD_PLACEHOLDER: &str = "{{critique-standard}}";

#[derive(Clone, Debug)]
pub struct AgentDef {
    pub name: String,
    pub prompt: String,
    /// The file the prompt came from, or "bundled".
    pub source: String,
    pub model: ModelSettings,
}

fn bundled(file: &str) -> Option<&'static str> {
    match file {
        "advocate.md" => Some(include_str!("../agents/advocate.md")),
        "adversary.md" => Some(include_str!("../agents/adversary.md")),
        "judge.md" => Some(include_str!("../agents/judge.md")),
        STANDARD_FILE => Some(include_str!("../agents/critique-standard.md")),
        _ => None,
    }
}

/// The first of: the project's file, the user's file, the bundled copy.
fn find(file: &str) -> anyhow::Result<(String, String)> {
    let candidates = [Some(PathBuf::from(".cross-examine/agents")), config_dir().map(|d| d.join("agents"))];
    for dir in candidates.into_iter().flatten() {
        let path = dir.join(file);
        if path.exists() {
            let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            return Ok((text, path.display().to_string()));
        }
    }
    bundled(file)
        .map(|text| (text.to_string(), "bundled".to_string()))
        .ok_or_else(|| anyhow!("no agent file {file} in .cross-examine/agents, the user config or the bundled set"))
}

pub fn load_agent(name: &str, config: &Config) -> anyhow::Result<AgentDef> {
    let (standard, _) = find(STANDARD_FILE)?;
    let (text, source) = find(&format!("{name}.md"))?;
    if text.starts_with("+++") {
        bail!("{source}: an agent file holds only the prompt; set the model in [models.{name}] of the configuration");
    }
    Ok(AgentDef {
        name: name.to_string(),
        prompt: text.trim().replace(STANDARD_PLACEHOLDER, standard.trim()),
        source,
        model: config.model_for(name)?,
    })
}
