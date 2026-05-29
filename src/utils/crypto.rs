use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::RngCore;

/// Encrypted payload — stores nonce + ciphertext together, base64 encoded.
#[derive(Debug, Clone)]
pub struct EncryptedPayload {
    pub data: String, // base64(nonce + ciphertext)
}

/// Derives a 32-byte AES key from a master key string + MAC address.
/// Uses Argon2id for key stretching — slow by design (protects against brute force).
pub fn derive_key(master_key: &str, mac_address: &str) -> Result<[u8; 32], String> {
    // Salt = first 16 bytes of mac_address padded/truncated
    let salt_str = format!("{:0<16}", &mac_address[..mac_address.len().min(16)]);
    let salt = SaltString::encode_b64(salt_str.as_bytes())
        .map_err(|e| format!("Salt error: {}", e))?;

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

/// Encrypts plaintext (the .env content) using AES-256-GCM.
/// Key is derived from master_key + mac_address.
/// Returns base64-encoded nonce + ciphertext.
pub fn encrypt_env(plaintext: &str, master_key: &str, mac_address: &str) -> Result<EncryptedPayload, String> {
    let key_bytes = derive_key(master_key, mac_address)?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    // Generate random 12-byte nonce
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Prepend nonce to ciphertext, then base64 encode
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(EncryptedPayload {
        data: BASE64.encode(&combined),
    })
}

/// Decrypts a payload encrypted with encrypt_env().
/// Requires the same master_key + mac_address used during encryption.
pub fn decrypt_env(payload: &EncryptedPayload, master_key: &str, mac_address: &str) -> Result<String, String> {
    let key_bytes = derive_key(master_key, mac_address)?;
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

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed — wrong key or corrupted data".to_string())?;

    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {}", e))
}

/// Generates a cryptographically secure random token (hex string).
pub fn generate_token(byte_len: usize) -> String {
    let mut bytes = vec![0u8; byte_len];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = "DB_HOST=localhost\nDB_PORT=5432\nAPI_KEY=supersecret";
        let master_key = "my-master-key-123";
        let mac = "aabbccddeeff";

        let encrypted = encrypt_env(plaintext, master_key, mac).unwrap();
        let decrypted = decrypt_env(&encrypted, master_key, mac).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_wrong_key_fails() {
        let plaintext = "SECRET=value";
        let encrypted = encrypt_env(plaintext, "correct-key", "aabbccddeeff").unwrap();
        let result = decrypt_env(&encrypted, "wrong-key", "aabbccddeeff");
        assert!(result.is_err());
    }
}