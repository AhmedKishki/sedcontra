//! Agent definitions: a prompt in a Markdown file, looked up in layers, with the orchestrator's
//! output shape appended, and the model the configuration assigns to the agent. The prompt
//! names no provider and no model.

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};

use crate::config::{Config, ModelSettings, config_dir};

const DISCIPLINE_FILE: &str = "discipline.md";
const DISCIPLINE_PLACEHOLDER: &str = "{{discipline}}";

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
        "adversary.md" => Some(include_str!("../agents/adversary.md")),
        "advocate.md" => Some(include_str!("../agents/advocate.md")),
        DISCIPLINE_FILE => Some(include_str!("../agents/discipline.md")),
        _ => None,
    }
}

/// The first of: the project's file, the user's file, the bundled copy.
fn find(file: &str) -> anyhow::Result<(String, String)> {
    let candidates = [Some(PathBuf::from(".sedcontra/agents")), config_dir().map(|d| d.join("agents"))];
    for dir in candidates.into_iter().flatten() {
        let path = dir.join(file);
        if path.exists() {
            let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            return Ok((text, path.display().to_string()));
        }
    }
    bundled(file)
        .map(|text| (text.to_string(), "bundled".to_string()))
        .ok_or_else(|| anyhow!("no agent file {file} in .sedcontra/agents, the user config or the bundled set"))
}

/// `file` names a prompt for this run in place of the layered lookup. The output shape is the
/// orchestrator's protocol, so it is appended here and a custom prompt cannot leave it out.
pub fn load_agent(name: &str, config: &Config, file: Option<&Path>, shape: &str) -> anyhow::Result<AgentDef> {
    let (discipline, _) = find(DISCIPLINE_FILE)?;
    let (text, source) = match file {
        Some(path) => (
            std::fs::read_to_string(path).with_context(|| format!("reading the {name} prompt {}", path.display()))?,
            path.display().to_string(),
        ),
        None => find(&format!("{name}.md"))?,
    };
    if text.starts_with("+++") {
        bail!("{source}: an agent file holds only the prompt; set the model in [models.{name}] of the configuration");
    }
    let prompt = text.trim().replace(DISCIPLINE_PLACEHOLDER, discipline.trim());
    Ok(AgentDef {
        name: name.to_string(),
        prompt: format!("{prompt}\n\n## Output\n\n{}", shape.trim()),
        source,
        model: config.model_for(name)?,
    })
}
