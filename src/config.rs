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
    /// Jenkins base URL, e.g. "http://jenkins.example.com:8080" (no trailing slash).
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

/// Normalize a base URL: trim whitespace, prepend `http://` when no scheme is
/// given (e.g. `jenkins.example.com:8080`), and drop trailing slashes.
pub fn normalize_url(url: &str) -> String {
    let url = url.trim();
    let url = if url.contains("://") {
        url.to_string()
    } else {
        format!("http://{url}")
    };
    url.trim_end_matches('/').to_string()
}

/// Normalize and validate a base URL. `field` names the configuration source
/// (e.g. `auth.url`) and is surfaced in the error. The offending value is
/// never echoed, since a URL may embed credentials.
pub fn parse_base_url(raw: &str, field: &str) -> Result<String> {
    let invalid = |reason: String| AppError::InvalidUrl {
        field: field.to_string(),
        reason,
    };
    if raw.trim().is_empty() {
        return Err(invalid("it is empty".into()));
    }
    let normalized = normalize_url(raw);
    let parsed = reqwest::Url::parse(&normalized).map_err(|e| invalid(e.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(invalid(format!(
            "unsupported scheme \"{}\" (use http or https)",
            parsed.scheme()
        )));
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err(invalid("it has no host".into()));
    }
    Ok(normalized)
}

/// Resolve the effective auth: env vars (`JENKINS_URL`/`JENKINS_USER`/
/// `JENKINS_TOKEN`) override the config file. `JENKINS_INSECURE=1` opts into
/// accepting invalid TLS certs when no config file value is set.
pub fn require_auth() -> Result<Auth> {
    let env_url = std::env::var("JENKINS_URL").ok();
    let env_user = std::env::var("JENKINS_USER").ok();
    let env_token = std::env::var("JENKINS_TOKEN").ok();
    let env_insecure = std::env::var("JENKINS_INSECURE").ok();

    let file_auth = load()?.auth.unwrap_or_default();
    let file_label = format!("auth.url in {}", path()?.display());

    resolve_auth(
        (env_url, env_user, env_token, env_insecure),
        file_auth,
        &file_label,
    )
}

type EnvAuth = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn resolve_auth(env: EnvAuth, file_auth: Auth, file_label: &str) -> Result<Auth> {
    let (env_url, env_user, env_token, env_insecure) = env;

    let url = match (env_url, file_auth.url) {
        (Some(u), _) => Some(parse_base_url(&u, "JENKINS_URL")?),
        (None, Some(u)) => Some(parse_base_url(&u, file_label)?),
        (None, None) => None,
    };

    let auth = Auth {
        url,
        user: env_user.or(file_auth.user),
        api_token: env_token.or(file_auth.api_token),
        insecure: env_insecure
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(file_auth.insecure),
    };

    if auth.is_complete() {
        Ok(auth)
    } else {
        Err(AppError::NoAuth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(url: &str) -> Auth {
        Auth {
            url: Some(url.into()),
            user: Some("u".into()),
            api_token: Some("t".into()),
            insecure: false,
        }
    }

    #[test]
    fn normalizes_scheme_whitespace_and_trailing_slash() {
        assert_eq!(normalize_url("jenkins.example.com:8080"), "http://jenkins.example.com:8080");
        assert_eq!(normalize_url(" https://j.io// "), "https://j.io");
        assert_eq!(normalize_url("http://j.io"), "http://j.io");
    }

    #[test]
    fn rejects_unusable_urls_naming_the_field() {
        for bad in ["", "   ", "ftp://host", "http://", "http://bad host"] {
            let err = parse_base_url(bad, "auth.url").unwrap_err().to_string();
            assert!(err.contains("auth.url"), "{bad:?} -> {err}");
            assert!(!err.contains("builder error"), "{bad:?} -> {err}");
        }
    }

    #[test]
    fn error_never_echoes_embedded_credentials() {
        let err = parse_base_url("ftp://user:secret@host", "auth.url")
            .unwrap_err()
            .to_string();
        assert!(!err.contains("secret"), "{err}");
    }

    #[test]
    fn file_url_without_scheme_is_fixed_up() {
        let auth = resolve_auth((None, None, None, None), file("jenkins.example.com:8080/"), "f").unwrap();
        assert_eq!(auth.url.as_deref(), Some("http://jenkins.example.com:8080"));
    }

    #[test]
    fn env_overrides_file_and_blames_the_env_var() {
        let ok =
            resolve_auth((Some("j.io".into()), None, None, None), file("bad://"), "f").unwrap();
        assert_eq!(ok.url.as_deref(), Some("http://j.io"));

        let err = resolve_auth(
            (Some("ftp://j".into()), None, None, None),
            file("j.io"),
            "f",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("JENKINS_URL"), "{err}");
    }

    #[test]
    fn invalid_file_url_blames_the_file_field() {
        let err = resolve_auth(
            (None, None, None, None),
            file("ftp://j"),
            "auth.url in ~/x.json",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("auth.url in ~/x.json"), "{err}");
    }
}
