//! The configuration file: providers, round limit, analyzer thresholds and the runs directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "cross-examine.toml";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub rounds: u32,
    pub runs_dir: Option<PathBuf>,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub analyzers: AnalyzerConfig,
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
#[serde(default)]
pub struct AnalyzerConfig {
    pub stub_max_len: usize,
    pub uncovered_min_words: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            rounds: 2,
            runs_dir: None,
            providers: default_providers(),
            analyzers: AnalyzerConfig::default(),
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        ProviderConfig {
            base_url: "https://api.deepseek.com".into(),
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
            api_key_file: None,
        }
    }
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        AnalyzerConfig { stub_max_len: 2, uncovered_min_words: 4 }
    }
}

fn default_providers() -> BTreeMap<String, ProviderConfig> {
    BTreeMap::from([("deepseek".to_string(), ProviderConfig::default())])
}

impl Config {
    /// Reads the first file found: `path`, `./cross-examine.toml`, then the user's config directory.
    pub fn load(path: Option<&Path>) -> anyhow::Result<Config> {
        let candidates: Vec<PathBuf> = match path {
            Some(p) => vec![p.to_path_buf()],
            None => [Some(PathBuf::from(FILE_NAME)), config_dir().map(|d| d.join("config.toml"))]
                .into_iter()
                .flatten()
                .collect(),
        };
        for candidate in candidates {
            if path.is_none() && !candidate.exists() {
                continue;
            }
            let text = std::fs::read_to_string(&candidate)
                .with_context(|| format!("reading config {}", candidate.display()))?;
            let mut config: Config = toml::from_str(&text)
                .with_context(|| format!("parsing config {}", candidate.display()))?;
            // A file that adds providers keeps the defaults it does not override.
            let mut providers = default_providers();
            providers.append(&mut config.providers);
            config.providers = providers;
            return Ok(config);
        }
        Ok(Config::default())
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

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("cross-examine"))
}
