//! `.greenbyte` - links a directory to a project.
//!
//! Holds no key material. The project key is sealed on the server and opened
//! with the device identity, so losing this file costs you nothing but the
//! link itself.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::workspace::paths::secure_atomic_write;

const CONFIG_FILE: &str = ".greenbyte";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    pub project_name: Option<String>,
    /// Project names are unique per owner, so the owner is part of the
    /// project's human-readable identity.
    #[serde(default)]
    pub owner_username: Option<String>,
    pub project_id: Option<String>,
    pub server_url: String,
    pub auth_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub soft_token: Option<String>,
    /// Device enrolment token from the join flow.
    pub refresher_token: Option<String>,
    /// Last key version this checkout saw, for a friendlier status display.
    #[serde(default)]
    pub key_version: Option<i32>,
}

impl ProjectConfig {
    /// `owner/project`, the unambiguous way to name a project now that two
    /// accounts may each have one called `backend`.
    pub fn qualified_name(&self) -> String {
        match (&self.owner_username, &self.project_name) {
            (Some(owner), Some(name)) => format!("{owner}/{name}"),
            (None, Some(name)) => name.clone(),
            _ => "unknown project".to_string(),
        }
    }

    pub fn require_project_id(&self) -> Result<&str, String> {
        self.project_id
            .as_deref()
            .ok_or_else(|| "The local .greenbyte file has no project ID.".to_string())
    }
}

pub fn config_path() -> PathBuf {
    Path::new(CONFIG_FILE).to_path_buf()
}

pub fn exists() -> bool {
    config_path().exists()
}

pub fn load() -> Result<ProjectConfig, String> {
    let path = config_path();
    if !path.exists() {
        return Err(
            "No .greenbyte file found. Run `greenbyte create <name>` or `greenbyte init` first."
                .to_string(),
        );
    }
    let raw =
        std::fs::read_to_string(&path).map_err(|e| format!("Could not read .greenbyte: {e}"))?;
    let config: ProjectConfig =
        serde_json::from_str(&raw).map_err(|e| format!("Corrupt .greenbyte file: {e}"))?;
    crate::core::api::client::validate_server_url(&config.server_url)?;
    Ok(config)
}

pub fn save(config: &ProjectConfig) -> Result<(), String> {
    let raw =
        serde_json::to_string_pretty(config).map_err(|e| format!("Serialization error: {e}"))?;
    secure_atomic_write(&config_path(), raw.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualifies_names_by_owner() {
        let mut config = ProjectConfig {
            project_name: Some("backend".to_string()),
            owner_username: Some("alice".to_string()),
            ..Default::default()
        };
        assert_eq!(config.qualified_name(), "alice/backend");
        config.owner_username = None;
        assert_eq!(config.qualified_name(), "backend");
    }

    #[test]
    fn older_config_files_without_the_new_fields_still_load() {
        let raw = r#"{"project_name":"api","project_id":"id","server_url":"https://example.com",
                      "auth_token":null,"soft_token":null,"refresher_token":null}"#;
        let config: ProjectConfig = serde_json::from_str(raw).unwrap();
        assert_eq!(config.owner_username, None);
        assert_eq!(config.key_version, None);
        assert_eq!(config.project_name.as_deref(), Some("api"));
    }
}
