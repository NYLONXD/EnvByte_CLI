use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use argon2::{password_hash::SaltString, Algorithm, Argon2, Params, PasswordHasher, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

const ENVELOPE_PREFIX: &str = "greenbyte:v2:";
const ENVELOPE_AAD: &[u8] = b"greenbyte:v2";
const SALT_LEN: usize = 16;
const MAX_ENCRYPTED_PAYLOAD_LEN: usize = 16 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct EncryptionEnvelopeV2 {
    version: u8,
    algorithm: String,
    kdf: String,
    salt: String,
    nonce: String,
    ciphertext: String,
}

/// Encrypted payload — stores nonce + ciphertext together, base64 encoded.
#[derive(Debug, Clone)]
pub struct EncryptedPayload {
    pub data: String, // base64(nonce + ciphertext)
}

/// Derives a 32-byte AES key from a master key string + MAC address.
/// Uses Argon2id for key stretching — slow by design (protects against brute force).
fn derive_legacy_key(master_key: &str, mac_address: &str) -> Result<[u8; 32], String> {
    // Salt = first 16 bytes of mac_address padded/truncated
    let truncated: String = mac_address.chars().take(16).collect();
    let salt_str = format!("{truncated:0<16}");
    let salt =
        SaltString::encode_b64(salt_str.as_bytes()).map_err(|e| format!("Salt error: {}", e))?;

    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(master_key.as_bytes(), &salt)
        .map_err(|e| format!("Key derivation failed: {}", e))?;

    // Extract 32 bytes from the hash output
    let hash_bytes = password_hash.hash.unwrap();
    let bytes = hash_bytes.as_bytes();
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes[..32]);
    Ok(key)
}

fn derive_key_v2(master_key: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let params = Params::new(64 * 1024, 3, 1, Some(32))
        .map_err(|e| format!("Invalid key derivation parameters: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0_u8; 32];
    argon2
        .hash_password_into(master_key.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Key derivation failed: {e}"))?;
    Ok(key)
}

/// Encrypts plaintext (the .env content) using AES-256-GCM.
///
/// The key is derived from the project master key and a fresh random salt, so
/// the resulting envelope is portable to every collaborator and device.
pub fn encrypt_env(plaintext: &str, master_key: &str) -> Result<EncryptedPayload, String> {
    let mut salt = [0_u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let mut key_bytes = derive_key_v2(master_key, &salt)?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    // Generate random 12-byte nonce
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher.encrypt(
        &nonce,
        aes_gcm::aead::Payload {
            msg: plaintext.as_bytes(),
            aad: ENVELOPE_AAD,
        },
    );
    key_bytes.zeroize();
    let ciphertext = ciphertext.map_err(|e| format!("Encryption failed: {e}"))?;

    let envelope = EncryptionEnvelopeV2 {
        version: 2,
        algorithm: "AES-256-GCM".to_string(),
        kdf: "Argon2id".to_string(),
        salt: BASE64.encode(salt),
        nonce: BASE64.encode(nonce),
        ciphertext: BASE64.encode(ciphertext),
    };
    let serialized = serde_json::to_vec(&envelope)
        .map_err(|e| format!("Could not serialize encrypted payload: {e}"))?;

    Ok(EncryptedPayload {
        data: format!("{ENVELOPE_PREFIX}{}", BASE64.encode(serialized)),
    })
}

/// Decrypts a payload produced by `encrypt_env`.
///
/// `mac_address` is only consulted for legacy (pre-v2) envelopes, which bound
/// the key to the device that wrote them. Current envelopes ignore it.
pub fn decrypt_env(
    payload: &EncryptedPayload,
    master_key: &str,
    mac_address: Option<&str>,
) -> Result<String, String> {
    if payload.data.len() > MAX_ENCRYPTED_PAYLOAD_LEN {
        return Err("Encrypted payload exceeds the 16 MiB safety limit".to_string());
    }
    if let Some(encoded) = payload.data.strip_prefix(ENVELOPE_PREFIX) {
        return decrypt_v2(encoded, master_key);
    }
    decrypt_legacy(payload, master_key, mac_address)
}

fn decrypt_v2(encoded: &str, master_key: &str) -> Result<String, String> {
    let serialized = BASE64
        .decode(encoded)
        .map_err(|e| format!("Encrypted envelope decode failed: {e}"))?;
    let envelope: EncryptionEnvelopeV2 = serde_json::from_slice(&serialized)
        .map_err(|e| format!("Invalid encrypted envelope: {e}"))?;
    if envelope.version != 2 || envelope.algorithm != "AES-256-GCM" || envelope.kdf != "Argon2id" {
        return Err("Unsupported encrypted payload version or algorithm".to_string());
    }
    let salt = BASE64
        .decode(&envelope.salt)
        .map_err(|e| format!("Invalid encryption salt: {e}"))?;
    if salt.len() != SALT_LEN {
        return Err("Invalid encryption salt length".to_string());
    }
    let nonce_bytes = BASE64
        .decode(&envelope.nonce)
        .map_err(|e| format!("Invalid encryption nonce: {e}"))?;
    if nonce_bytes.len() != 12 {
        return Err("Invalid encryption nonce length".to_string());
    }
    let ciphertext = BASE64
        .decode(&envelope.ciphertext)
        .map_err(|e| format!("Invalid encrypted content: {e}"))?;
    let mut key_bytes = derive_key_v2(master_key, &salt)?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher.decrypt(
        nonce,
        aes_gcm::aead::Payload {
            msg: &ciphertext,
            aad: ENVELOPE_AAD,
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
         that machine to upgrade it to the portable format.",
    )?;
    let mut key_bytes = derive_legacy_key(master_key, mac_address)?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let combined = BASE64
        .decode(&payload.data)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;

    if combined.len() < 12 {
        return Err("Encrypted data is too short".to_string());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher.decrypt(nonce, ciphertext);
    key_bytes.zeroize();
    decode_plaintext(plaintext)
}

fn decode_plaintext(plaintext: Result<Vec<u8>, aes_gcm::Error>) -> Result<String, String> {
    let plaintext =
        plaintext.map_err(|_| "Decryption failed — wrong key or corrupted data".to_string())?;
    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {e}"))
}

/// Computes the server-side verifier for a project master key.
///
/// This is a plain domain-separated SHA-256 rather than a slow hash because the
/// key is always 256 bits of `generate_token(32)` output, which is not
/// guessable. If user-chosen master keys are ever accepted, this must become a
/// salted memory-hard hash before the verifier is stored anywhere.
pub fn master_key_verifier(master_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"greenbyte-project-key-v2:");
    hasher.update(master_key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generates a cryptographically secure random token (hex string).
pub fn generate_token(byte_len: usize) -> String {
    let mut bytes = vec![0u8; byte_len];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Loads the project master key from the environment for automation, or securely prompts.
pub fn read_master_key() -> Result<Zeroizing<String>, String> {
    let key = match std::env::var("GREENBYTE_MASTER_KEY") {
        Ok(key) => key,
        Err(std::env::VarError::NotPresent) => {
            rpassword::prompt_password("Master Key: ").map_err(|e| e.to_string())?
        }
        Err(e) => return Err(format!("Could not read GREENBYTE_MASTER_KEY: {e}")),
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
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = "DB_HOST=localhost
DB_PORT=5432
API_KEY=supersecret";
        let master_key = "my-master-key-123";

        let encrypted = encrypt_env(plaintext, master_key).unwrap();
        let decrypted = decrypt_env(&encrypted, master_key, None).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_wrong_key_fails() {
        let encrypted = encrypt_env("SECRET=value", "correct-key").unwrap();
        assert!(decrypt_env(&encrypted, "wrong-key", None).is_err());
    }

    #[test]
    fn test_v2_ciphertext_is_portable_between_devices() {
        let encrypted = encrypt_env("SECRET=value", "shared-project-key").unwrap();
        let decrypted = decrypt_env(&encrypted, "shared-project-key", Some("device-b")).unwrap();
        assert_eq!(decrypted, "SECRET=value");
        assert!(encrypted.data.starts_with(ENVELOPE_PREFIX));
    }

    #[test]
    fn test_v2_ciphertext_decrypts_without_any_mac_address() {
        // Containers and CI runners often expose no network interface at all.
        let encrypted = encrypt_env("SECRET=value", "shared-project-key").unwrap();
        assert_eq!(
            decrypt_env(&encrypted, "shared-project-key", None).unwrap(),
            "SECRET=value"
        );
    }

    #[test]
    fn test_tampered_envelope_fails() {
        let mut encrypted = encrypt_env("SECRET=value", "key").unwrap();
        encrypted.data.push('A');
        assert!(decrypt_env(&encrypted, "key", None).is_err());
    }

    #[test]
    fn test_random_salt_and_nonce_produce_unique_ciphertext() {
        let first = encrypt_env("SECRET=value", "key").unwrap();
        let second = encrypt_env("SECRET=value", "key").unwrap();
        assert_ne!(first.data, second.data);
    }

    #[test]
    fn test_malformed_payloads_do_not_panic() {
        for data in ["", "greenbyte:v2:not-base64", "AAAA"] {
            let result = decrypt_env(
                &EncryptedPayload {
                    data: data.to_string(),
                },
                "key",
                Some("device"),
            );
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_legacy_ciphertext_remains_decryptable() {
        let master_key = "legacy-key";
        let mac = "aabbccddeeff";
        let payload = legacy_payload(master_key, mac, b"OLD=value");
        assert_eq!(
            decrypt_env(&payload, master_key, Some(mac)).unwrap(),
            "OLD=value"
        );
        assert!(decrypt_env(&payload, master_key, Some("different-device")).is_err());
    }

    #[test]
    fn test_legacy_ciphertext_without_a_mac_explains_itself() {
        let payload = legacy_payload("legacy-key", "aabbccddeeff", b"OLD=value");
        let error = decrypt_env(&payload, "legacy-key", None).unwrap_err();
        assert!(error.contains("legacy device-bound format"), "{error}");
    }

    #[test]
    fn test_legacy_key_derivation_handles_short_mac_values() {
        // A one-character identifier used to slice past the end of the string.
        assert!(derive_legacy_key("key", "a").is_ok());
        assert!(derive_legacy_key("key", "").is_ok());
    }

    #[test]
    fn test_master_key_verifier_is_stable_and_distinct() {
        let key = generate_token(32);
        assert_eq!(master_key_verifier(&key), master_key_verifier(&key));
        assert_eq!(master_key_verifier(&key).len(), 64);
        assert_ne!(master_key_verifier(&key), master_key_verifier("other"));
    }

    fn legacy_payload(master_key: &str, mac: &str, plaintext: &[u8]) -> EncryptedPayload {
        let key_bytes = derive_legacy_key(master_key, mac).unwrap();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher.encrypt(&nonce, plaintext).unwrap();
        let mut combined = nonce.to_vec();
        combined.extend(ciphertext);
        EncryptedPayload {
            data: BASE64.encode(combined),
        }
    }
}
