//! The provider API key: where it is stored, how it is loaded, and how it is kept out of every text a model reads.
//!
//! The key travels only in the HTTP `Authorization` header. It never enters a prompt, and every provider
//! error is redacted before it can reach a report, an MCP result, stderr or the run record.

use std::fmt;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};

use crate::config::{ProviderConfig, config_dir};

/// The longest provider error text passed on, after redaction.
const MAX_ERROR_CHARS: usize = 500;

/// An API key that cannot be printed by accident: `Debug` and `Display` show `[redacted]`.
pub struct ApiKey(String);

impl ApiKey {
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Removes the key from a text and shortens it, so a provider that echoes the key leaks nothing.
    pub fn redact(&self, text: &str) -> String {
        let redacted = text.replace(&self.0, "[redacted]");
        match redacted.char_indices().nth(MAX_ERROR_CHARS) {
            Some((cut, _)) => format!("{}…", &redacted[..cut]),
            None => redacted,
        }
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey([redacted])")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// The key file of a provider: `api_key_file` if set, else `<config dir>/keys/<provider>`.
pub fn key_path(provider: &str, cfg: &ProviderConfig) -> anyhow::Result<PathBuf> {
    match &cfg.api_key_file {
        Some(path) => Ok(path.clone()),
        None => config_dir()
            .map(|dir| dir.join("keys").join(provider))
            .ok_or_else(|| anyhow!("no user config directory to keep the key in; set api_key_file for provider {provider}")),
    }
}

/// The key file first; the environment variable only when no key file exists.
pub fn load_key(provider: &str, cfg: &ProviderConfig) -> anyhow::Result<ApiKey> {
    let path = key_path(provider, cfg)?;
    if path.exists() {
        check_private(&path)?;
        let key = std::fs::read_to_string(&path).with_context(|| format!("reading the key file {}", path.display()))?;
        let key = key.trim();
        if key.is_empty() {
            bail!("the key file {} is empty; run `sedcontra key set {provider}`", path.display());
        }
        return Ok(ApiKey(key.to_string()));
    }
    if let Some(var) = &cfg.api_key_env
        && let Ok(key) = std::env::var(var)
        && !key.trim().is_empty()
    {
        return Ok(ApiKey(key.trim().to_string()));
    }
    bail!(
        "no API key for provider {provider}: run `sedcontra key set {provider}` in your own terminal{}",
        cfg.api_key_env.as_ref().map(|v| format!(", or set {v} for CI")).unwrap_or_default()
    )
}

/// Refuses a key file that anyone but its owner can read or write.
#[cfg(unix)]
fn check_private(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)?.permissions().mode();
    if mode & 0o077 != 0 {
        bail!(
            "the key file {} can be read or written by other users (mode {:o}); run `chmod 600 {}`",
            path.display(),
            mode & 0o777,
            path.display()
        );
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_private(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// Writes the key to the provider's key file, readable and writable by its owner only.
pub fn store_key(path: &Path, key: &str) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).with_context(|| format!("opening {}", path.display()))?;
    #[cfg(unix)]
    {
        // A file that existed before keeps its old mode on open, so set it explicitly.
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    writeln!(file, "{key}")?;
    Ok(())
}

/// Reads a key typed at the terminal without echo, or piped on stdin.
pub fn read_key_from_user(provider: &str) -> anyhow::Result<String> {
    let key = if std::io::stdin().is_terminal() {
        rpassword::prompt_password(format!("API key for {provider} (input hidden): "))?
    } else {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        text
    };
    let key = key.trim().to_string();
    if key.is_empty() {
        bail!("no key given");
    }
    Ok(key)
}
