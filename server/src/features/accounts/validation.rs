//! Input rules for account fields.
//!
//! Gathered in one place so the same rule cannot drift between signup, reset
//! and profile updates.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::error::ApiError;

/// Length of a base64-encoded 32-byte X25519 public key.
pub const ENCODED_PUBLIC_KEY_LEN: usize = 44;

pub fn normalize_email(value: &str) -> Result<String, ApiError> {
    let value = value.trim().to_lowercase();
    let (local, domain) = value
        .split_once('@')
        .ok_or_else(|| ApiError::BadRequest("invalid email address".to_string()))?;
    if local.is_empty() || domain.is_empty() || !domain.contains('.') || value.len() > 320 {
        return Err(ApiError::BadRequest("invalid email address".to_string()));
    }
    Ok(value)
}

pub fn validate_username(value: &str) -> Result<(), ApiError> {
    if value.len() < 2
        || value.len() > 64
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character))
    {
        return Err(ApiError::BadRequest(
            "username must be 2-64 letters, numbers, '-' or '_'".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_password(value: &str) -> Result<(), ApiError> {
    if value.len() < 12 || value.len() > 1024 {
        return Err(ApiError::BadRequest(
            "password must contain between 12 and 1024 characters".to_string(),
        ));
    }
    Ok(())
}

/// Accepts only a base64 32-byte value, so a malformed or oversized key can
/// never reach a collaborator's key-wrapping step.
pub fn validate_public_key(value: &str) -> Result<(), ApiError> {
    let decoded = BASE64
        .decode(value)
        .map_err(|_| ApiError::BadRequest("public key must be base64".to_string()))?;
    if value.len() != ENCODED_PUBLIC_KEY_LEN || decoded.len() != 32 {
        return Err(ApiError::BadRequest(
            "public key must be a 32-byte X25519 key".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_account_inputs() {
        assert!(
            normalize_email(" User@Example.com ").is_ok_and(|value| value == "user@example.com")
        );
        assert!(normalize_email("bad").is_err());
        assert!(validate_username("valid-user_2").is_ok());
        assert!(validate_username("bad user").is_err());
        assert!(validate_password("long-enough-password").is_ok());
        assert!(validate_password("short").is_err());
    }

    #[test]
    fn accepts_only_32_byte_base64_public_keys() {
        assert!(validate_public_key(&BASE64.encode([7_u8; 32])).is_ok());
        assert!(validate_public_key(&BASE64.encode([7_u8; 31])).is_err());
        assert!(validate_public_key(&BASE64.encode([7_u8; 64])).is_err());
        assert!(validate_public_key("not base64!!").is_err());
    }
}
