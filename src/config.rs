//! The configuration: models, providers, round limit, analyzer thresholds and the runs directory.
//!
//! The built-in defaults live in `config/default.toml`, not in code; a user file overrides them
//! table by table. No agent and no line of code names a provider: the agents' models come from
//! `[models.default]` and `[models.<agent>]`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};

const DEFAULTS: &str = include_str!("../config/default.toml");
const FILE_NAME: &str = "cross-examine.toml";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub rounds: u32,
    #[serde(default)]
    pub runs_dir: Option<PathBuf>,
    pub models: BTreeMap<String, ModelOverride>,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub analyzers: AnalyzerConfig,
}

/// One table of `[models]`: `default` sets every field, an agent's table overrides some of them.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ModelOverride {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub request: Option<toml::Table>,
}

/// The model an agent runs on, resolved from `[models.default]` and the agent's own table.
#[derive(Serialize, Clone, Debug)]
pub struct ModelSettings {
    pub provider: String,
    pub model: String,
    pub max_tokens: u32,
    /// Merged over the request body, e.g. `thinking`.
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

impl Config {
    /// The built-in defaults, overridden by the first file found: `path`, `./cross-examine.toml`,
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

    /// The agent's model: `[models.default]` with the fields of `[models.<agent>]` laid over it.
    pub fn model_for(&self, agent: &str) -> anyhow::Result<ModelSettings> {
        let default = self.models.get("default").cloned().unwrap_or_default();
        let own = self.models.get(agent).cloned().unwrap_or_default();
        let missing = |field: &str| anyhow!("no {field} for agent {agent}: set it in [models.default] or [models.{agent}]");
        let request = match own.request.or(default.request) {
            Some(table) => match serde_json::to_value(table)? {
                serde_json::Value::Object(map) => map,
                _ => serde_json::Map::new(),
            },
            None => serde_json::Map::new(),
        };
        Ok(ModelSettings {
            provider: own.provider.or(default.provider).ok_or_else(|| missing("provider"))?,
            model: own.model.or(default.model).ok_or_else(|| missing("model"))?,
            max_tokens: own.max_tokens.or(default.max_tokens).ok_or_else(|| missing("max_tokens"))?,
            request,
        })
    }

    pub fn round_limit(&self) -> u32 {
        let clamped = self.rounds.clamp(1, 3);
        if clamped != self.rounds {
            eprintln!("warning: rounds = {} is outside 1–3; using {clamped}", self.rounds);
        }
        clamped
    }

    pub fn runs_dir(&self) -> PathBuf {
        self.runs_dir.clone().unwrap_or_else(|| {
            dirs::data_dir()
                .map(|d| d.join("cross-examine").join("runs"))
                .unwrap_or_else(|| PathBuf::from("cross-examine-runs"))
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
    dirs::config_dir().map(|d| d.join("cross-examine"))
}
