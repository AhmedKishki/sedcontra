//! Agent definitions: Markdown files with TOML front matter, looked up in layers.

use std::path::PathBuf;

use anyhow::{Context, anyhow};
use serde::Deserialize;

use crate::config::config_dir;

const STANDARD_FILE: &str = "critique-standard.md";
const STANDARD_PLACEHOLDER: &str = "{{critique-standard}}";

#[derive(Clone, Debug)]
pub struct AgentDef {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub max_tokens: u32,
    pub request: serde_json::Map<String, serde_json::Value>,
    pub prompt: String,
    /// The file the definition came from, or "bundled".
    pub source: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrontMatter {
    provider: String,
    model: String,
    max_tokens: u32,
    #[serde(default)]
    request: Option<toml::Table>,
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

pub fn load_agent(name: &str) -> anyhow::Result<AgentDef> {
    let (standard, _) = find(STANDARD_FILE)?;
    let (text, source) = find(&format!("{name}.md"))?;
    parse_agent(name, &text, source, &standard)
}

pub fn parse_agent(name: &str, text: &str, source: String, standard: &str) -> anyhow::Result<AgentDef> {
    let rest = text
        .strip_prefix("+++")
        .ok_or_else(|| anyhow!("{source}: the file must open with a +++ front matter block"))?;
    let close = rest
        .find("\n+++")
        .ok_or_else(|| anyhow!("{source}: the +++ front matter block is not closed"))?;
    let front: FrontMatter =
        toml::from_str(&rest[..close]).with_context(|| format!("{source}: invalid front matter"))?;
    let body = rest[close + "\n+++".len()..].trim_start_matches(['\r', '\n']);

    let request = match front.request {
        Some(table) => match serde_json::to_value(table)? {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        },
        None => serde_json::Map::new(),
    };

    Ok(AgentDef {
        name: name.to_string(),
        provider: front.provider,
        model: front.model,
        max_tokens: front.max_tokens,
        request,
        prompt: body.replace(STANDARD_PLACEHOLDER, standard.trim()),
        source,
    })
}
