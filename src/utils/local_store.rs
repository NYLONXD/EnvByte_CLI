use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

// ─── File names ─────────────────────────────────────────────────────────────
const CLI_FILE: &str = ".greenbyte"; // project config + token store
const LOGS_FILE: &str = ".greenbyte-logs"; // local commit history

// ─── Data Structures ─────────────────────────────────────────────────────────

/// Stored in .greenbyte — project identity + auth tokens for this machine.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LocalConfig {
    pub project_name: Option<String>,
    pub project_id: Option<String>,
    pub server_url: String,
    pub auth_token: Option<String>, // JWT from login
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub soft_token: Option<String>,      // session token from server
    pub refresher_token: Option<String>, // OTT + MAC (device-bound)
    pub master_key_hint: Option<String>, // NOT the key itself, just a hint/id
}

/// A single local commit snapshot.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalCommit {
    pub id: String, // UUID — the "address" for rollback
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub env_snapshot: String, // encrypted .env content at this point
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub remote_commit_id: Option<String>,
}

/// The .greenbyte-logs file.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LogStore {
    pub commits: Vec<LocalCommit>,
}

// ─── Config (`.greenbyte`) ────────────────────────────────────────────────────

pub fn config_path() -> PathBuf {
    Path::new(CLI_FILE).to_path_buf()
}

pub fn load_config() -> Result<LocalConfig, String> {
    let path = config_path();
    if !path.exists() {
        return Err("No .greenbyte file found. Run `greenbyte init <project>` first.".to_string());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("Could not read .greenbyte: {}", e))?;
    let config: LocalConfig =
        serde_json::from_str(&raw).map_err(|e| format!("Corrupt .greenbyte file: {}", e))?;
    crate::utils::config::validate_server_url(&config.server_url)?;
    Ok(config)
}

pub fn save_config(config: &LocalConfig) -> Result<(), String> {
    let raw =
        serde_json::to_string_pretty(config).map_err(|e| format!("Serialization error: {}", e))?;
    secure_atomic_write(&config_path(), raw.as_bytes())
        .map_err(|e| format!("Could not write .greenbyte: {e}"))
}

pub fn config_exists() -> bool {
    config_path().exists()
}

// ─── Global auth config (`~/.greenbyte-auth`) ─────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GlobalAuth {
    pub email: Option<String>,
    pub auth_token: Option<String>,
    pub user_id: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

pub fn global_auth_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".greenbyte-auth")
}

pub fn load_global_auth() -> Result<GlobalAuth, String> {
    let path = global_auth_path();
    if !path.exists() {
        return Ok(GlobalAuth::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("Could not read auth file: {}", e))?;
    serde_json::from_str(&raw).map_err(|e| format!("Corrupt auth file: {}", e))
}

pub fn save_global_auth(auth: &GlobalAuth) -> Result<(), String> {
    let raw =
        serde_json::to_string_pretty(auth).map_err(|e| format!("Serialization error: {}", e))?;
    secure_atomic_write(&global_auth_path(), raw.as_bytes())
        .map_err(|e| format!("Could not write auth file: {e}"))
}

pub fn clear_global_auth() -> Result<(), String> {
    let path = global_auth_path();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Could not remove {}: {e}", path.display()))?;
    }
    Ok(())
}

// ─── Logs (`.greenbyte-logs`) ─────────────────────────────────────────────────

pub fn logs_path() -> PathBuf {
    Path::new(LOGS_FILE).to_path_buf()
}

pub fn load_logs() -> Result<LogStore, String> {
    let path = logs_path();
    if !path.exists() {
        return Ok(LogStore::default());
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("Could not read .greenbyte-logs: {}", e))?;
    serde_json::from_str(&raw).map_err(|e| format!("Corrupt .greenbyte-logs: {}", e))
}

pub fn save_logs(store: &LogStore) -> Result<(), String> {
    let raw =
        serde_json::to_string_pretty(store).map_err(|e| format!("Serialization error: {}", e))?;
    secure_atomic_write(&logs_path(), raw.as_bytes())
        .map_err(|e| format!("Could not write .greenbyte-logs: {e}"))
}

pub fn append_commit(commit: LocalCommit) -> Result<(), String> {
    let mut store = load_logs()?;
    store.commits.push(commit);
    save_logs(&store)
}

/// Atomically writes a sensitive file and restricts it to the current user on Unix.
pub fn secure_atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.exists() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("greenbyte");
    let temporary = parent.join(format!(".{filename}.{}.tmp", uuid::Uuid::new_v4()));
    let result = write_temporary(&temporary, contents)
        .and_then(|_| fs::rename(&temporary, path).map_err(|e| e.to_string()));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_temporary(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not create {}: {e}", path.display()))?;
    file.write_all(contents)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.sync_all()
        .map_err(|e| format!("Could not sync {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_commit_json_remains_compatible() {
        let raw = r#"{"commits":[{"id":"id","message":"m","timestamp":"2024-01-01T00:00:00Z","env_snapshot":"data"}]}"#;
        let store: LogStore = serde_json::from_str(raw).unwrap();
        assert_eq!(store.commits[0].filename, None);
        assert_eq!(store.commits[0].remote_commit_id, None);
    }

    #[test]
    fn atomic_write_replaces_content() {
        let directory =
            std::env::temp_dir().join(format!("greenbyte-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("secret");
        secure_atomic_write(&path, b"first").unwrap();
        secure_atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
