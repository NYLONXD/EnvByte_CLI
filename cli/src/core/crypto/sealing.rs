//! Sealing a project data key to a member's identity key.
//!
//! This is what replaced passing a master key around by hand. An admin seals
//! the project key to the recipient's published X25519 public key; only the
//! holder of the matching secret can open it, and the server - which stores
//! the result - cannot.
//!
//! Construction: ephemeral X25519, HKDF-SHA256 to a wrapping key, then
//! AES-256-GCM. Both public keys are bound into the HKDF info, so a sealed key
//! cannot be replayed against a different recipient.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, Payload},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

use crate::core::crypto::{identity::Identity, DataKey, DATA_KEY_LEN};

const WRAP_PREFIX: &str = "greenbyte:wrap:v1:";
const WRAP_INFO: &[u8] = b"greenbyte:key-wrap:v1";
const NONCE_LEN: usize = 12;

#[derive(Serialize, Deserialize)]
struct WrapEnvelope {
    version: u8,
    algorithm: String,
    /// The ephemeral public key this data key was sealed with.
    epk: String,
    nonce: String,
    ciphertext: String,
}

/// Seals `data_key` so that only the holder of `recipient`'s secret can open it.
pub fn seal(data_key: &DataKey, recipient_public_key_base64: &str) -> Result<String, String> {
    let recipient = decode_public_key(recipient_public_key_base64)?;
    let ephemeral = Identity::generate();
    let ephemeral_public = ephemeral.public_key();

    let mut wrapping_key = derive_wrapping_key(
        ephemeral.secret(),
        &recipient,
        ephemeral_public.as_bytes(),
        recipient.as_bytes(),
    )?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&wrapping_key));
    let mut nonce_bytes = [0_u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher.encrypt(
        Nonce::from_slice(&nonce_bytes),
        Payload {
            msg: data_key.as_slice(),
            aad: WRAP_INFO,
        },
    );
    wrapping_key.zeroize();
    let ciphertext = ciphertext.map_err(|e| format!("Could not seal the project key: {e}"))?;

    let envelope = WrapEnvelope {
        version: 1,
        algorithm: "X25519-HKDF-SHA256-AES256GCM".to_string(),
        epk: BASE64.encode(ephemeral_public.as_bytes()),
        nonce: BASE64.encode(nonce_bytes),
        ciphertext: BASE64.encode(ciphertext),
    };
    let serialized = serde_json::to_vec(&envelope)
        .map_err(|e| format!("Could not serialize the sealed key: {e}"))?;
    Ok(format!("{WRAP_PREFIX}{}", BASE64.encode(serialized)))
}

/// Opens a sealed data key with this device's identity secret.
pub fn open(wrapped: &str, identity: &Identity) -> Result<DataKey, String> {
    let encoded = wrapped.strip_prefix(WRAP_PREFIX).ok_or(
        "This project key is in a format this CLI does not understand. Upgrade the CLI, or ask \
         an admin to re-grant your access.",
    )?;
    let serialized = BASE64
        .decode(encoded)
        .map_err(|e| format!("Sealed key is not valid base64: {e}"))?;
    let envelope: WrapEnvelope =
        serde_json::from_slice(&serialized).map_err(|e| format!("Malformed sealed key: {e}"))?;
    if envelope.version != 1 || envelope.algorithm != "X25519-HKDF-SHA256-AES256GCM" {
        return Err("Unsupported sealed key version or algorithm.".to_string());
    }

    let ephemeral_public = decode_public_key(&envelope.epk)?;
    let nonce_bytes = BASE64
        .decode(&envelope.nonce)
        .map_err(|e| format!("Invalid sealed key nonce: {e}"))?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err("Invalid sealed key nonce length.".to_string());
    }
    let ciphertext = BASE64
        .decode(&envelope.ciphertext)
        .map_err(|e| format!("Invalid sealed key contents: {e}"))?;

    let our_public = identity.public_key();
    let mut wrapping_key = derive_wrapping_key(
        identity.secret(),
        &ephemeral_public,
        ephemeral_public.as_bytes(),
        our_public.as_bytes(),
    )?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&wrapping_key));
    let plaintext = cipher.decrypt(
        Nonce::from_slice(&nonce_bytes),
        Payload {
            msg: &ciphertext,
            aad: WRAP_INFO,
        },
    );
    wrapping_key.zeroize();
    let plaintext = plaintext.map_err(|_| {
        "This project key was not sealed to your identity. If you replaced your identity key, \
         ask an admin to grant you access again."
            .to_string()
    })?;
    if plaintext.len() != DATA_KEY_LEN {
        return Err("Sealed key did not contain a valid project data key.".to_string());
    }
    let mut key = [0_u8; DATA_KEY_LEN];
    key.copy_from_slice(&plaintext);
    Ok(Zeroizing::new(key))
}

/// HKDF-SHA256 over the X25519 shared secret, with both public keys bound into
/// `info` so the result is specific to this sender/recipient pair.
fn derive_wrapping_key(
    secret: &StaticSecret,
    peer: &PublicKey,
    ephemeral_public: &[u8; 32],
    recipient_public: &[u8; 32],
) -> Result<[u8; 32], String> {
    let mut shared = secret.diffie_hellman(peer).to_bytes();
    let mut info = Vec::with_capacity(WRAP_INFO.len() + 64);
    info.extend_from_slice(WRAP_INFO);
    info.extend_from_slice(ephemeral_public);
    info.extend_from_slice(recipient_public);
    let hkdf = Hkdf::<Sha256>::new(None, &shared);
    let mut key = [0_u8; 32];
    let result = hkdf
        .expand(&info, &mut key)
        .map_err(|e| format!("Key wrapping derivation failed: {e}"));
    shared.zeroize();
    result?;
    Ok(key)
}

pub fn decode_public_key(encoded: &str) -> Result<PublicKey, String> {
    let raw = BASE64
        .decode(encoded)
        .map_err(|e| format!("Invalid public key encoding: {e}"))?;
    if raw.len() != 32 {
        return Err("A public key must be exactly 32 bytes.".to_string());
    }
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&raw);
    Ok(PublicKey::from(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::crypto::generate_data_key;

    #[test]
    fn the_recipient_can_open_what_was_sealed_to_them() {
        let recipient = Identity::generate();
        let data_key = generate_data_key();
        let wrapped = seal(&data_key, &recipient.public_key_base64()).unwrap();
        assert!(wrapped.starts_with(WRAP_PREFIX));
        assert_eq!(*open(&wrapped, &recipient).unwrap(), *data_key);
    }

    #[test]
    fn nobody_else_can_open_it() {
        let recipient = Identity::generate();
        let stranger = Identity::generate();
        let wrapped = seal(&generate_data_key(), &recipient.public_key_base64()).unwrap();
        assert!(open(&wrapped, &stranger).is_err());
    }

    #[test]
    fn sealing_twice_produces_different_ciphertext() {
        let recipient = Identity::generate();
        let data_key = generate_data_key();
        let first = seal(&data_key, &recipient.public_key_base64()).unwrap();
        let second = seal(&data_key, &recipient.public_key_base64()).unwrap();
        assert_ne!(first, second);
        assert_eq!(*open(&first, &recipient).unwrap(), *data_key);
        assert_eq!(*open(&second, &recipient).unwrap(), *data_key);
    }

    #[test]
    fn tampering_is_detected() {
        let recipient = Identity::generate();
        let wrapped = seal(&generate_data_key(), &recipient.public_key_base64()).unwrap();
        let mut tampered = wrapped.clone();
        tampered.push('A');
        assert!(open(&tampered, &recipient).is_err());
    }

    #[test]
    fn malformed_input_never_panics() {
        let recipient = Identity::generate();
        for value in ["", "greenbyte:wrap:v1:", "greenbyte:wrap:v1:zzz", "nope"] {
            assert!(open(value, &recipient).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn rejects_public_keys_of_the_wrong_size() {
        assert!(decode_public_key(&BASE64.encode([1_u8; 31])).is_err());
        assert!(decode_public_key(&BASE64.encode([1_u8; 32])).is_ok());
    }
}
