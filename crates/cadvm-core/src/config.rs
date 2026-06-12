//! Repository configuration (`.cadvm/config.json`) and author resolution.
//!
//! Config is a flat `key = value` store serialized as a JSON object. The only
//! keys cadvm interprets today are [`USER_NAME`] and [`USER_EMAIL`], used to
//! stamp commits, but arbitrary keys may be stored.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::model::Author;
use crate::repo::Repository;

/// Config key for the author name.
pub const USER_NAME: &str = "user.name";
/// Config key for the author email.
pub const USER_EMAIL: &str = "user.email";
/// Environment variable that overrides the author name.
pub const ENV_AUTHOR_NAME: &str = "CADVM_AUTHOR_NAME";
/// Environment variable that overrides the author email.
pub const ENV_AUTHOR_EMAIL: &str = "CADVM_AUTHOR_EMAIL";

/// Fallback author name when nothing is configured.
const UNKNOWN_NAME: &str = "unknown";

/// A flat string key/value configuration store.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Config {
    entries: BTreeMap<String, String>,
}

impl Config {
    /// Load config from the repository, or an empty config if none exists yet.
    pub fn load(repo: &Repository) -> Result<Config> {
        let path = repo.config_path();
        match std::fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(CoreError::io(&path, e)),
        }
    }

    /// Persist config to the repository.
    pub fn save(&self, repo: &Repository) -> Result<()> {
        let path = repo.config_path();
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        std::fs::write(&path, bytes).map_err(|e| CoreError::io(&path, e))
    }

    /// Read a key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// Set a key.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.insert(key.into(), value.into());
    }

    /// All entries, sorted by key.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Resolve the commit author: environment overrides config, which overrides a
/// neutral fallback. Never fails — a snapshot must always be possible.
pub fn resolve_author(repo: &Repository) -> Result<Author> {
    let config = Config::load(repo)?;
    let name = std::env::var(ENV_AUTHOR_NAME)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| config.get(USER_NAME).map(str::to_string))
        .unwrap_or_else(|| UNKNOWN_NAME.to_string());
    let email = std::env::var(ENV_AUTHOR_EMAIL)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| config.get(USER_EMAIL).map(str::to_string))
        .unwrap_or_default();
    Ok(Author { name, email })
}
