//! The configuration: models, providers, round limit, analyzer thresholds and the runs directory.
//!
//! The built-in defaults live in `config/default.toml`, not in code; a user file overrides them
//! table by table. No agent and no line of code names a provider: each agent's model comes from
//! its own `[models.<agent>]` table.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};

const DEFAULTS: &str = include_str!("../config/default.toml");
const FILE_NAME: &str = "sedcontra.toml";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub rounds: u32,
    #[serde(default)]
    pub runs_dir: Option<PathBuf>,
    pub models: BTreeMap<String, ModelSettings>,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub analyzers: AnalyzerConfig,
}

/// The model an agent runs on: its `[models.<agent>]` table.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModelSettings {
    pub provider: String,
    pub model: String,
    pub max_tokens: u32,
    /// Merged over the request body, e.g. `thinking`.
    #[serde(default)]
    pub request: serde_json::Map<String, serde_json::Value>,
}

/// A provider's key lives in its own key file (see `secret`), so a new provider never inherits
/// another provider's key. `api_key_env` is a fallback for CI and is unset unless a file names it.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProviderConfig {
    pub base_url: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub api_key_file: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnalyzerConfig {
    pub stub_max_len: usize,
    pub uncovered_min_words: usize,
}

/// The highest round limit allowed, so a mistyped setting cannot run a debate without end.
const MAX_ROUNDS: u32 = 10;

impl Config {
    /// The built-in defaults, overridden by the first file found: `path`, `./sedcontra.toml`,
    /// then `config.toml` in the user config directory.
    pub fn load(path: Option<&Path>) -> anyhow::Result<Config> {
        let mut table: toml::Table = DEFAULTS.parse().context("parsing the built-in defaults")?;
        let candidates: Vec<PathBuf> = match path {
            Some(p) => vec![p.to_path_buf()],
            None => [Some(PathBuf::from(FILE_NAME)), config_dir().map(|d| d.join("config.toml"))]
                .into_iter()
                .flatten()
                .filter(|p| p.exists())
                .collect(),
        };
        if let Some(file) = candidates.first() {
            let text = std::fs::read_to_string(file).with_context(|| format!("reading config {}", file.display()))?;
            let user: toml::Table = text.parse().with_context(|| format!("parsing config {}", file.display()))?;
            merge(&mut table, user);
        }
        let source = candidates.first().map(|p| p.display().to_string()).unwrap_or_else(|| "the built-in defaults".into());
        toml::Value::Table(table).try_into().with_context(|| format!("reading the configuration from {source}"))
    }

    /// The agent's model, from its own `[models.<agent>]` table.
    pub fn model_for(&self, agent: &str) -> anyhow::Result<ModelSettings> {
        self.models
            .get(agent)
            .cloned()
            .ok_or_else(|| anyhow!("the configuration has no [models.{agent}] table for agent {agent}"))
    }

    pub fn round_limit(&self) -> u32 {
        let clamped = self.rounds.clamp(1, MAX_ROUNDS);
        if clamped != self.rounds {
            eprintln!("warning: rounds = {} is outside 1–{MAX_ROUNDS}; using {clamped}", self.rounds);
        }
        clamped
    }

    pub fn runs_dir(&self) -> PathBuf {
        self.runs_dir.clone().unwrap_or_else(|| {
            dirs::data_dir()
                .map(|d| d.join("sedcontra").join("runs"))
                .unwrap_or_else(|| PathBuf::from("sedcontra-runs"))
        })
    }
}

/// Lays `over` onto `base`: tables merge key by key, any other value replaces.
fn merge(base: &mut toml::Table, over: toml::Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(inner)), toml::Value::Table(value)) => merge(inner, value),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("sedcontra"))
}
