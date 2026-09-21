//! The account's X25519 identity key.
//!
//! This is the root of a member's access. Colleagues seal project data keys to
//! its public half, so nothing secret ever has to be sent over a side channel.
//! The secret half never leaves the device and is as sensitive as an SSH
//! private key.

use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

use crate::core::workspace::paths::secure_atomic_write;

const IDENTITY_FILE: &str = ".greenbyte-identity";
/// Lets CI supply the key without a file on disk.
const IDENTITY_ENV: &str = "GREENBYTE_IDENTITY_KEY";

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    version: u8,
    algorithm: String,
    secret_key: String,
    public_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// A loaded identity. The secret is dropped from memory when this goes away.
pub struct Identity {
    secret: StaticSecret,
}

impl Identity {
    pub fn generate() -> Self {
        let mut bytes = [0_u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let secret = StaticSecret::from(bytes);
        bytes.zeroize();
        Self { secret }
    }

    pub fn secret(&self) -> &StaticSecret {
        &self.secret
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::from(&self.secret)
    }

    pub fn public_key_base64(&self) -> String {
        BASE64.encode(self.public_key().as_bytes())
    }

    /// A short, stable label for the key, so two people can confirm over a
    /// call that they are looking at the same identity.
    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.public_key_base64()).unwrap_or_else(|_| "unknown".to_string())
    }
}

/// Derives the displayed fingerprint from an encoded public key.
pub fn fingerprint_of(public_key_base64: &str) -> Result<String, String> {
    let raw = BASE64
        .decode(public_key_base64)
        .map_err(|e| format!("Invalid public key: {e}"))?;
    let digest = Sha256::digest(&raw);
    let hex = hex::encode(&digest[..8]);
    Ok(hex
        .as_bytes()
        .chunks(4)
        .map(|chunk| String::from_utf8_lossy(chunk).to_string())
        .collect::<Vec<_>>()
        .join("-"))
}

pub fn identity_path() -> PathBuf {
    if let Ok(custom) = std::env::var("GREENBYTE_IDENTITY_FILE") {
        return PathBuf::from(custom);
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(IDENTITY_FILE)
}

/// Loads the identity, creating one on first use.
///
/// Returns whether a key was just created, so the caller can tell the user to
/// back it up rather than silently producing one they do not know exists.
pub fn load_or_create() -> Result<(Identity, bool), String> {
    if let Some(identity) = from_environment()? {
        return Ok((identity, false));
    }
    let path = identity_path();
    if path.exists() {
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        let stored: StoredIdentity = serde_json::from_str(&raw)
            .map_err(|e| format!("Corrupt identity file {}: {e}", path.display()))?;
        if stored.version != 1 || stored.algorithm != "X25519" {
            return Err(format!(
                "Unsupported identity format in {}. This CLI understands version 1 X25519 keys.",
                path.display()
            ));
        }
        return Ok((decode_identity(&stored.secret_key)?, false));
    }
    let identity = Identity::generate();
    save(&identity)?;
    Ok((identity, true))
}

/// Reads an identity supplied through the environment, for CI runners that
/// hold the key in a secret store rather than on disk.
fn from_environment() -> Result<Option<Identity>, String> {
    match std::env::var(IDENTITY_ENV) {
        Ok(value) if value.trim().is_empty() => Err(format!("{IDENTITY_ENV} is set but empty.")),
        Ok(value) => Ok(Some(decode_identity(value.trim())?)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(format!("Could not read {IDENTITY_ENV}: {e}")),
    }
}

fn decode_identity(encoded: &str) -> Result<Identity, String> {
    let mut raw = Zeroizing::new(
        BASE64
            .decode(encoded)
            .map_err(|e| format!("Identity key is not valid base64: {e}"))?,
    );
    if raw.len() != 32 {
        return Err("Identity key must decode to exactly 32 bytes.".to_string());
    }
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&raw);
    raw.zeroize();
    let secret = StaticSecret::from(bytes);
    bytes.zeroize();
    Ok(Identity { secret })
}

pub fn save(identity: &Identity) -> Result<(), String> {
    let stored = StoredIdentity {
        version: 1,
        algorithm: "X25519".to_string(),
        secret_key: BASE64.encode(identity.secret.to_bytes()),
        public_key: identity.public_key_base64(),
        created_at: chrono::Utc::now(),
    };
    let raw = Zeroizing::new(
        serde_json::to_string_pretty(&stored)
            .map_err(|e| format!("Could not serialize identity: {e}"))?,
    );
    secure_atomic_write(&identity_path(), raw.as_bytes())
}

/// Exports the secret half, for copying an identity to another machine or into
/// a CI secret store. Callers must treat the result as a credential.
pub fn export_secret(identity: &Identity) -> Zeroizing<String> {
    Zeroizing::new(BASE64.encode(identity.secret.to_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_its_encoded_form() {
        let identity = Identity::generate();
        let encoded = export_secret(&identity);
        let restored = decode_identity(&encoded).unwrap();
        assert_eq!(
            identity.public_key().as_bytes(),
            restored.public_key().as_bytes()
        );
    }

    #[test]
    fn rejects_keys_of_the_wrong_length() {
        assert!(decode_identity(&BASE64.encode([1_u8; 16])).is_err());
        assert!(decode_identity("not base64!!").is_err());
    }

    #[test]
    fn fingerprints_are_stable_readable_and_distinct() {
        let first = Identity::generate();
        let second = Identity::generate();
        assert_eq!(first.fingerprint(), first.fingerprint());
        assert_ne!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.fingerprint().len(), 19); // 4 groups of 4 plus 3 dashes
    }

    #[test]
    fn generated_identities_differ() {
        let first = Identity::generate();
        let second = Identity::generate();
        assert_ne!(
            first.public_key().as_bytes(),
            second.public_key().as_bytes()
        );
    }
}
