//! The encrypted form of an environment file.
//!
//! Three formats exist. Only v3 is ever written:
//!
//! - **v3** - AES-256-GCM directly under the project data key. The key is
//!   already 256 bits of entropy, so no password stretching is needed, and the
//!   key version is bound into the authenticated data.
//! - **v2** - AES-256-GCM under a key stretched from a typed master key.
//!   Readable so that snapshots taken before key wrapping still open.
//! - **legacy** - v2's ancestor, additionally bound to the writing machine's
//!   MAC address.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Key, Nonce,
};
use argon2::{password_hash::SaltString, Algorithm, Argon2, Params, PasswordHasher, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::core::crypto::DataKey;

// These tags predate the rename to Envbyte and are deliberately kept. The AAD
// is authenticated by AES-GCM, so changing it would make every stored file
// undecryptable; users never see any of them.
const V3_PREFIX: &str = "greenbyte:v3:";
const V3_AAD: &[u8] = b"greenbyte:v3";
const V2_PREFIX: &str = "greenbyte:v2:";
const V2_AAD: &[u8] = b"greenbyte:v2";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
pub const MAX_ENCRYPTED_PAYLOAD_LEN: usize = 16 * 1024 * 1024;

/// An encrypted environment file, as stored and transmitted.
#[derive(Debug, Clone)]
pub struct EncryptedPayload {
    pub data: String,
}

#[derive(Serialize, Deserialize)]
struct EnvelopeV3 {
    version: u8,
    algorithm: String,
    /// Which project data key opens this. Lets a client pick the right key
    /// when reading history written before a rotation.
    key_version: i32,
    nonce: String,
    ciphertext: String,
}

#[derive(Serialize, Deserialize)]
struct EnvelopeV2 {
    version: u8,
    algorithm: String,
    kdf: String,
    salt: String,
    nonce: String,
    ciphertext: String,
}

/// Encrypts under the project data key at a specific key version.
pub fn encrypt(
    plaintext: &str,
    data_key: &DataKey,
    key_version: i32,
) -> Result<EncryptedPayload, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(data_key.as_slice()));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext.as_bytes(),
                aad: &aad_for(key_version),
            },
        )
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let envelope = EnvelopeV3 {
        version: 3,
        algorithm: "AES-256-GCM".to_string(),
        key_version,
        nonce: BASE64.encode(nonce),
        ciphertext: BASE64.encode(ciphertext),
    };
    let serialized = serde_json::to_vec(&envelope)
        .map_err(|e| format!("Could not serialize encrypted payload: {e}"))?;
    Ok(EncryptedPayload {
        data: format!("{V3_PREFIX}{}", BASE64.encode(serialized)),
    })
}

/// Reads the key version a payload needs, without decrypting it.
///
/// Callers use this to choose which of their held data keys to try.
pub fn key_version_of(payload: &EncryptedPayload) -> Option<i32> {
    let encoded = payload.data.strip_prefix(V3_PREFIX)?;
    let serialized = BASE64.decode(encoded).ok()?;
    let envelope: EnvelopeV3 = serde_json::from_slice(&serialized).ok()?;
    Some(envelope.key_version)
}

pub fn is_current_format(payload: &EncryptedPayload) -> bool {
    payload.data.starts_with(V3_PREFIX)
}

/// Decrypts a v3 payload with the supplied data key.
pub fn decrypt(payload: &EncryptedPayload, data_key: &DataKey) -> Result<String, String> {
    check_size(payload)?;
    let encoded = payload.data.strip_prefix(V3_PREFIX).ok_or(
        "This snapshot predates project key wrapping. Decrypt it with `--master-key`, or push it \
         again from a client that holds the project key to upgrade it.",
    )?;
    let serialized = BASE64
        .decode(encoded)
        .map_err(|e| format!("Encrypted envelope decode failed: {e}"))?;
    let envelope: EnvelopeV3 = serde_json::from_slice(&serialized)
        .map_err(|e| format!("Invalid encrypted envelope: {e}"))?;
    if envelope.version != 3 || envelope.algorithm != "AES-256-GCM" {
        return Err("Unsupported encrypted payload version or algorithm.".to_string());
    }
    let nonce_bytes = decode_nonce(&envelope.nonce)?;
    let ciphertext = BASE64
        .decode(&envelope.ciphertext)
        .map_err(|e| format!("Invalid encrypted content: {e}"))?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(data_key.as_slice()));
    let plaintext = cipher.decrypt(
        Nonce::from_slice(&nonce_bytes),
        Payload {
            msg: &ciphertext,
            aad: &aad_for(envelope.key_version),
        },
    );
    decode_plaintext(plaintext)
}

/// Decrypts a pre-wrapping payload from a typed master key.
///
/// `mac_address` is consulted only for the oldest format, which bound the key
/// to the machine that wrote it.
pub fn decrypt_with_master_key(
    payload: &EncryptedPayload,
    master_key: &str,
    mac_address: Option<&str>,
) -> Result<String, String> {
    check_size(payload)?;
    if let Some(encoded) = payload.data.strip_prefix(V2_PREFIX) {
        return decrypt_v2(encoded, master_key);
    }
    decrypt_legacy(payload, master_key, mac_address)
}

fn aad_for(key_version: i32) -> Vec<u8> {
    let mut aad = Vec::with_capacity(V3_AAD.len() + 5);
    aad.extend_from_slice(V3_AAD);
    aad.push(b':');
    aad.extend_from_slice(key_version.to_string().as_bytes());
    aad
}

fn check_size(payload: &EncryptedPayload) -> Result<(), String> {
    if payload.data.len() > MAX_ENCRYPTED_PAYLOAD_LEN {
        return Err("Encrypted payload exceeds the 16 MiB safety limit.".to_string());
    }
    Ok(())
}

fn decode_nonce(encoded: &str) -> Result<Vec<u8>, String> {
    let bytes = BASE64
        .decode(encoded)
        .map_err(|e| format!("Invalid encryption nonce: {e}"))?;
    if bytes.len() != NONCE_LEN {
        return Err("Invalid encryption nonce length.".to_string());
    }
    Ok(bytes)
}

fn decrypt_v2(encoded: &str, master_key: &str) -> Result<String, String> {
    let serialized = BASE64
        .decode(encoded)
        .map_err(|e| format!("Encrypted envelope decode failed: {e}"))?;
    let envelope: EnvelopeV2 = serde_json::from_slice(&serialized)
        .map_err(|e| format!("Invalid encrypted envelope: {e}"))?;
    if envelope.version != 2 || envelope.algorithm != "AES-256-GCM" || envelope.kdf != "Argon2id" {
        return Err("Unsupported encrypted payload version or algorithm.".to_string());
    }
    let salt = BASE64
        .decode(&envelope.salt)
        .map_err(|e| format!("Invalid encryption salt: {e}"))?;
    if salt.len() != SALT_LEN {
        return Err("Invalid encryption salt length.".to_string());
    }
    let nonce_bytes = decode_nonce(&envelope.nonce)?;
    let ciphertext = BASE64
        .decode(&envelope.ciphertext)
        .map_err(|e| format!("Invalid encrypted content: {e}"))?;
    let mut key_bytes = derive_v2_key(master_key, &salt)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let plaintext = cipher.decrypt(
        Nonce::from_slice(&nonce_bytes),
        Payload {
            msg: &ciphertext,
            aad: V2_AAD,
        },
    );
    key_bytes.zeroize();
    decode_plaintext(plaintext)
}

fn decrypt_legacy(
    payload: &EncryptedPayload,
    master_key: &str,
    mac_address: Option<&str>,
) -> Result<String, String> {
    let mac_address = mac_address.ok_or(
        "This snapshot uses the legacy device-bound format, which can only be decrypted on the \
         machine that created it while that machine exposes a network interface. Re-push it from \
         that machine to upgrade it.",
    )?;
    let mut key_bytes = derive_legacy_key(master_key, mac_address)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let combined = BASE64
        .decode(&payload.data)
        .map_err(|e| format!("Base64 decode failed: {e}"))?;
    if combined.len() < NONCE_LEN {
        return Err("Encrypted data is too short.".to_string());
    }
    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
    let plaintext = cipher.decrypt(Nonce::from_slice(nonce_bytes), ciphertext);
    key_bytes.zeroize();
    decode_plaintext(plaintext)
}

fn derive_v2_key(master_key: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let params = Params::new(64 * 1024, 3, 1, Some(32))
        .map_err(|e| format!("Invalid key derivation parameters: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0_u8; 32];
    argon2
        .hash_password_into(master_key.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Key derivation failed: {e}"))?;
    Ok(key)
}

fn derive_legacy_key(master_key: &str, mac_address: &str) -> Result<[u8; 32], String> {
    let truncated: String = mac_address.chars().take(16).collect();
    let salt_str = format!("{truncated:0<16}");
    let salt =
        SaltString::encode_b64(salt_str.as_bytes()).map_err(|e| format!("Salt error: {e}"))?;
    let password_hash = Argon2::default()
        .hash_password(master_key.as_bytes(), &salt)
        .map_err(|e| format!("Key derivation failed: {e}"))?;
    let hash_bytes = password_hash
        .hash
        .ok_or("Legacy key derivation produced no output.")?;
    let bytes = hash_bytes.as_bytes();
    if bytes.len() < 32 {
        return Err("Legacy key derivation produced too few bytes.".to_string());
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes[..32]);
    Ok(key)
}

fn decode_plaintext(plaintext: Result<Vec<u8>, aes_gcm::Error>) -> Result<String, String> {
    let plaintext =
        plaintext.map_err(|_| "Decryption failed - wrong key or corrupted data.".to_string())?;
    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::crypto::generate_data_key;
    use rand::RngCore;

    const SAMPLE: &str = "DB_HOST=localhost\nAPI_KEY=supersecret";

    #[test]
    fn round_trips_under_the_data_key() {
        let key = generate_data_key();
        let encrypted = encrypt(SAMPLE, &key, 1).unwrap();
        assert!(encrypted.data.starts_with(V3_PREFIX));
        assert_eq!(decrypt(&encrypted, &key).unwrap(), SAMPLE);
    }

    #[test]
    fn a_different_data_key_cannot_read_it() {
        let encrypted = encrypt(SAMPLE, &generate_data_key(), 1).unwrap();
        assert!(decrypt(&encrypted, &generate_data_key()).is_err());
    }

    #[test]
    fn the_key_version_is_readable_without_the_key() {
        let encrypted = encrypt(SAMPLE, &generate_data_key(), 7).unwrap();
        assert_eq!(key_version_of(&encrypted), Some(7));
    }

    #[test]
    fn the_key_version_is_authenticated() {
        // Re-labelling a payload as another version must not verify, so a
        // rotation cannot be undone by editing the header.
        let key = generate_data_key();
        let encrypted = encrypt(SAMPLE, &key, 1).unwrap();
        let encoded = encrypted.data.strip_prefix(V3_PREFIX).unwrap();
        let mut envelope: EnvelopeV3 =
            serde_json::from_slice(&BASE64.decode(encoded).unwrap()).unwrap();
        envelope.key_version = 2;
        let relabelled = EncryptedPayload {
            data: format!(
                "{V3_PREFIX}{}",
                BASE64.encode(serde_json::to_vec(&envelope).unwrap())
            ),
        };
        assert!(decrypt(&relabelled, &key).is_err());
    }

    #[test]
    fn each_encryption_is_unique() {
        let key = generate_data_key();
        assert_ne!(
            encrypt(SAMPLE, &key, 1).unwrap().data,
            encrypt(SAMPLE, &key, 1).unwrap().data
        );
    }

    #[test]
    fn tampering_is_detected() {
        let key = generate_data_key();
        let mut encrypted = encrypt(SAMPLE, &key, 1).unwrap();
        encrypted.data.push('A');
        assert!(decrypt(&encrypted, &key).is_err());
    }

    #[test]
    fn malformed_payloads_never_panic() {
        let key = generate_data_key();
        for data in ["", "greenbyte:v3:not-base64", "AAAA", "greenbyte:v3:"] {
            let payload = EncryptedPayload {
                data: data.to_string(),
            };
            assert!(decrypt(&payload, &key).is_err(), "accepted {data}");
            assert!(key_version_of(&payload).is_none() || data.is_empty());
        }
    }

    #[test]
    fn a_v3_payload_reports_itself_as_current() {
        let encrypted = encrypt(SAMPLE, &generate_data_key(), 1).unwrap();
        assert!(is_current_format(&encrypted));
        assert!(!is_current_format(&EncryptedPayload {
            data: "greenbyte:v2:whatever".to_string()
        }));
    }

    #[test]
    fn pre_wrapping_snapshots_still_open_with_their_master_key() {
        // Written in the v2 format this CLI no longer produces.
        let master_key = "an-old-project-master-key";
        let mut salt = [0_u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        let key = derive_v2_key(master_key, &salt).unwrap();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: SAMPLE.as_bytes(),
                    aad: V2_AAD,
                },
            )
            .unwrap();
        let envelope = EnvelopeV2 {
            version: 2,
            algorithm: "AES-256-GCM".to_string(),
            kdf: "Argon2id".to_string(),
            salt: BASE64.encode(salt),
            nonce: BASE64.encode(nonce),
            ciphertext: BASE64.encode(ciphertext),
        };
        let payload = EncryptedPayload {
            data: format!(
                "{V2_PREFIX}{}",
                BASE64.encode(serde_json::to_vec(&envelope).unwrap())
            ),
        };
        assert_eq!(
            decrypt_with_master_key(&payload, master_key, None).unwrap(),
            SAMPLE
        );
        assert!(decrypt_with_master_key(&payload, "wrong", None).is_err());
    }

    #[test]
    fn legacy_key_derivation_handles_short_identifiers() {
        assert!(derive_legacy_key("key", "a").is_ok());
        assert!(derive_legacy_key("key", "").is_ok());
    }

    #[test]
    fn oversized_payloads_are_refused_before_any_work() {
        let payload = EncryptedPayload {
            data: "a".repeat(MAX_ENCRYPTED_PAYLOAD_LEN + 1),
        };
        assert!(decrypt(&payload, &generate_data_key()).is_err());
    }
}
