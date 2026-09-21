//! `.greenbyte-logs` - local encrypted snapshots, the offline half of history.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::workspace::paths::secure_atomic_write;

const LOGS_FILE: &str = ".greenbyte-logs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCommit {
    pub id: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    /// Ciphertext, never plaintext - this file is as safe as the project key.
    pub env_snapshot: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub remote_commit_id: Option<String>,
    #[serde(default)]
    pub key_version: Option<i32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CommitLog {
    pub commits: Vec<LocalCommit>,
}

pub fn logs_path() -> PathBuf {
    Path::new(LOGS_FILE).to_path_buf()
}

pub fn load() -> Result<CommitLog, String> {
    let path = logs_path();
    if !path.exists() {
        return Ok(CommitLog::default());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Could not read .greenbyte-logs: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Corrupt .greenbyte-logs: {e}"))
}

pub fn save(store: &CommitLog) -> Result<(), String> {
    let raw =
        serde_json::to_string_pretty(store).map_err(|e| format!("Serialization error: {e}"))?;
    secure_atomic_write(&logs_path(), raw.as_bytes())
}

pub fn append(commit: LocalCommit) -> Result<(), String> {
    let mut store = load()?;
    store.commits.push(commit);
    save(&store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_logs_without_the_new_fields_still_load() {
        let raw = r#"{"commits":[{"id":"id","message":"m",
                      "timestamp":"2024-01-01T00:00:00Z","env_snapshot":"data"}]}"#;
        let store: CommitLog = serde_json::from_str(raw).unwrap();
        assert_eq!(store.commits[0].filename, None);
        assert_eq!(store.commits[0].key_version, None);
    }
}
