use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};

// ─── File names ─────────────────────────────────────────────────────────────
const CLI_FILE: &str = ".greenbyte";          // project config + token store
const LOGS_FILE: &str = ".greenbyte-logs";    // local commit history

// ─── Data Structures ─────────────────────────────────────────────────────────

/// Stored in .greenbyte — project identity + auth tokens for this machine.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LocalConfig {
    pub project_name: Option<String>,
    pub project_id: Option<String>,
    pub server_url: String,
    pub auth_token: Option<String>,      // JWT from login
    pub soft_token: Option<String>,      // session token from server
    pub refresher_token: Option<String>, // OTT + MAC (device-bound)
    pub master_key_hint: Option<String>, // NOT the key itself, just a hint/id
}

/// A single local commit snapshot.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalCommit {
    pub id: String,              // UUID — the "address" for rollback
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub env_snapshot: String,    // encrypted .env content at this point
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
        return Err(
            "No .greenbyte file found. Run `greenbyte init <project>` first.".to_string(),
        );
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read .greenbyte: {}", e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Corrupt .greenbyte file: {}", e))
}

pub fn save_config(config: &LocalConfig) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Serialization error: {}", e))?;
    fs::write(config_path(), raw)
        .map_err(|e| format!("Could not write .greenbyte: {}", e))
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
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read auth file: {}", e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Corrupt auth file: {}", e))
}

pub fn save_global_auth(auth: &GlobalAuth) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(auth)
        .map_err(|e| format!("Serialization error: {}", e))?;
    fs::write(global_auth_path(), raw)
        .map_err(|e| format!("Could not write auth file: {}", e))
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
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read .greenbyte-logs: {}", e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Corrupt .greenbyte-logs: {}", e))
}

pub fn save_logs(store: &LogStore) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Serialization error: {}", e))?;
    fs::write(logs_path(), raw)
        .map_err(|e| format!("Could not write .greenbyte-logs: {}", e))
}

pub fn append_commit(commit: LocalCommit) -> Result<(), String> {
    let mut store = load_logs()?;
    store.commits.push(commit);
    save_logs(&store)
}