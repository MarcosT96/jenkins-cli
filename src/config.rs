//! User config file handling.
//!
//! Stores the Jenkins base URL + user + API token in
//! `$HOME/.jenkins-cli-config.json`. Environment variables (`JENKINS_URL`,
//! `JENKINS_USER`, `JENKINS_TOKEN`) take precedence over the file, so CI and
//! containerized MCP usage can skip writing a config file entirely.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

const CONFIG_FILE_NAME: &str = ".jenkins-cli-config.json";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Auth {
    /// Jenkins base URL, e.g. "http://10.35.0.51:18080" (no trailing slash).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(rename = "apiToken", skip_serializing_if = "Option::is_none")]
    pub api_token: Option<String>,
    /// Accept self-signed/invalid TLS certs for this instance.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub insecure: bool,
}

impl Auth {
    pub fn is_complete(&self) -> bool {
        self.url.is_some() && self.user.is_some() && self.api_token.is_some()
    }
}

/// Resolve the config file path (`$HOME/.jenkins-cli-config.json`).
pub fn path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::Usage("Could not resolve HOME directory.".into()))?;
    Ok(home.join(CONFIG_FILE_NAME))
}

/// Load the config, returning a default (empty) config if the file is absent.
pub fn load() -> Result<Config> {
    let path = path()?;
    match std::fs::read_to_string(&path) {
        Ok(contents) if contents.trim().is_empty() => Ok(Config::default()),
        Ok(contents) => Ok(serde_json::from_str(&contents)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e.into()),
    }
}

/// Write the config back with pretty formatting. The file holds the API
/// token in plaintext, so on Unix it is restricted to `0o600`.
pub fn save(config: &Config) -> Result<()> {
    let path = path()?;
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Normalize a base URL by trimming any trailing slash.
pub fn normalize_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// Resolve the effective auth: env vars (`JENKINS_URL`/`JENKINS_USER`/
/// `JENKINS_TOKEN`) override the config file. `JENKINS_INSECURE=1` opts into
/// accepting invalid TLS certs when no config file value is set.
pub fn require_auth() -> Result<Auth> {
    let env_url = std::env::var("JENKINS_URL").ok();
    let env_user = std::env::var("JENKINS_USER").ok();
    let env_token = std::env::var("JENKINS_TOKEN").ok();

    let file_auth = load()?.auth.unwrap_or_default();

    let auth = Auth {
        url: env_url.map(|u| normalize_url(&u)).or(file_auth.url),
        user: env_user.or(file_auth.user),
        api_token: env_token.or(file_auth.api_token),
        insecure: std::env::var("JENKINS_INSECURE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(file_auth.insecure),
    };

    if auth.is_complete() {
        Ok(auth)
    } else {
        Err(AppError::NoAuth)
    }
}
