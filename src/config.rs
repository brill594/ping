use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub account_id: String,
    pub api_token: String,
    pub from: String,
    pub to: String,
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default = "default_interval")]
    pub min_progress_interval_secs: u64,
    #[serde(default = "default_subject_prefix")]
    pub subject_prefix: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| format!("cannot read config metadata {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "config {} must be a regular file, not a symlink",
                path.display()
            ));
        }
        check_private_permissions(path, &metadata)?;

        let contents = fs::read_to_string(path)
            .map_err(|error| format!("cannot read config {}: {error}", path.display()))?;
        let config: Self = toml::from_str(&contents)
            .map_err(|error| format!("invalid config {}: {error}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("account_id", &self.account_id),
            ("api_token", &self.api_token),
            ("from", &self.from),
            ("to", &self.to),
        ] {
            if value.trim().is_empty() {
                return Err(format!("config field {name} must not be empty"));
            }
        }
        if !looks_like_email(&self.from) || !looks_like_email(&self.to) {
            return Err("config fields from and to must be email addresses".into());
        }
        if self.subject_prefix.contains(['\r', '\n']) {
            return Err("subject_prefix must not contain a newline".into());
        }
        self.listen.parse::<std::net::SocketAddr>().map_err(|_| {
            "config field listen must be an IP socket address such as 127.0.0.1:9109".to_string()
        })?;
        Ok(())
    }
}

pub fn default_config_path() -> PathBuf {
    home_dir().join(".config/ping-agent-mail/config.toml")
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_listen() -> String {
    "127.0.0.1:9109".into()
}

fn default_interval() -> u64 {
    900
}

fn default_subject_prefix() -> String {
    "[Agent]".into()
}

fn looks_like_email(value: &str) -> bool {
    let (local, domain) = value.rsplit_once('@').unwrap_or_default();
    !local.is_empty() && domain.contains('.') && !value.chars().any(char::is_whitespace)
}

#[cfg(unix)]
fn check_private_permissions(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(format!(
            "config {} is accessible by group or others; run chmod 600 {}",
            path.display(),
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_private_permissions(_path: &Path, _metadata: &fs::Metadata) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_check_rejects_obvious_non_addresses() {
        assert!(looks_like_email("agent@example.com"));
        assert!(!looks_like_email("not-an-address"));
        assert!(!looks_like_email("agent@localhost"));
        assert!(!looks_like_email("agent @example.com"));
    }
}
