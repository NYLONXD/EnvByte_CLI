//! `~/.greenbyte-auth` - the account session, shared by every project on this
//! machine.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::workspace::paths::secure_atomic_write;

const AUTH_FILE: &str = ".greenbyte-auth";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalAuth {
    pub email: Option<String>,
    pub auth_token: Option<String>,
    pub user_id: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Fingerprint of the identity key last published for this account, so the
    /// CLI can notice a device whose key the server does not know.
    #[serde(default)]
    pub published_key_fingerprint: Option<String>,
}

pub fn path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(AUTH_FILE)
}

pub fn load() -> Result<GlobalAuth, String> {
    let path = path();
    if !path.exists() {
        return Ok(GlobalAuth::default());
    }
    let raw =
        std::fs::read_to_string(&path).map_err(|e| format!("Could not read auth file: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Corrupt auth file: {e}"))
}

pub fn save(auth: &GlobalAuth) -> Result<(), String> {
    let raw =
        serde_json::to_string_pretty(auth).map_err(|e| format!("Serialization error: {e}"))?;
    secure_atomic_write(&path(), raw.as_bytes())
}

pub fn clear() -> Result<(), String> {
    let path = path();
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Could not remove {}: {e}", path.display()))?;
    }
    Ok(())
}
