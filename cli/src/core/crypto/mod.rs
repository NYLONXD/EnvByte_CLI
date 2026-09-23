//! Everything the CLI does with keys and ciphertext.
//!
//! The shape of the scheme:
//!
//! ```text
//! identity key (X25519, per device)
//!   └─ opens ─> project data key, one version per rotation
//!                 └─ encrypts ─> .env file contents
//! ```
//!
//! Nothing secret is ever sent to the server: it stores one sealed copy of the
//! data key per member, and ciphertext it cannot read.

pub mod envelope;
pub mod identity;
pub mod keyring;
pub mod sealing;

use rand::RngCore;
use zeroize::Zeroizing;

pub use envelope::EncryptedPayload;

pub const DATA_KEY_LEN: usize = 32;

/// A project data key, wiped from memory on drop.
pub type DataKey = Zeroizing<[u8; DATA_KEY_LEN]>;

/// Mints a project data key. Full entropy, so it is used directly as an
/// AES-256 key with no stretching.
pub fn generate_data_key() -> DataKey {
    let mut key = [0_u8; DATA_KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);
    Zeroizing::new(key)
}

/// Decrypts a payload of any vintage.
///
/// Current payloads open with a held project key. Anything older predates key
/// wrapping and needs the master key that was typed in at the time, so the
/// caller is asked for it only at the moment one is actually encountered.
pub fn open_payload(
    keyring: &keyring::Keyring,
    payload: &EncryptedPayload,
    mac_address: Option<&str>,
) -> Result<String, String> {
    if envelope::is_current_format(payload) {
        return keyring.decrypt(payload);
    }
    eprintln!(
        "This snapshot predates per-member key wrapping and needs the old project master key.
         Push it again afterwards to move it onto the current project key."
    );
    let master_key = read_master_key()?;
    envelope::decrypt_with_master_key(payload, &master_key, mac_address)
}

/// Reads a pre-wrapping master key, for opening snapshots written before
/// per-member key wrapping existed.
fn read_master_key() -> Result<Zeroizing<String>, String> {
    let key = match std::env::var("ENVBYTE_MASTER_KEY") {
        Ok(key) => key,
        Err(std::env::VarError::NotPresent) => {
            rpassword::prompt_password("Legacy master key: ").map_err(|e| e.to_string())?
        }
        Err(e) => return Err(format!("Could not read ENVBYTE_MASTER_KEY: {e}")),
    };
    if key.is_empty() {
        return Err("Master key cannot be empty.".to_string());
    }
    Ok(Zeroizing::new(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_keys_are_full_length_and_unique() {
        let first = generate_data_key();
        let second = generate_data_key();
        assert_eq!(first.len(), DATA_KEY_LEN);
        assert_ne!(*first, *second);
    }
}
