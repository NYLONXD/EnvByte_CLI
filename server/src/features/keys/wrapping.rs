//! Bounds on the opaque sealed keys the server stores.
//!
//! The server cannot open a wrapped key, so all it can do is refuse one that
//! is obviously malformed before storing it and handing it back out.

use crate::error::ApiError;

/// An ephemeral public key, a nonce and the sealed key itself, base64-encoded
/// inside a small header. 512 bytes leaves generous room for all of that.
pub const MAX_WRAPPED_KEY_LEN: usize = 512;

pub fn validate_wrapped_key(value: &str) -> Result<(), ApiError> {
    if value.is_empty() || value.len() > MAX_WRAPPED_KEY_LEN || !value.is_ascii() {
        return Err(ApiError::BadRequest("invalid wrapped key".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_wrapped_keys() {
        assert!(validate_wrapped_key("greenbyte:wrap:v1:abc").is_ok());
        assert!(validate_wrapped_key("").is_err());
        assert!(validate_wrapped_key(&"a".repeat(MAX_WRAPPED_KEY_LEN + 1)).is_err());
        assert!(validate_wrapped_key("k\u{e9}y").is_err());
    }
}
