//! Account password hashing.
//!
//! Argon2id with the crate defaults: memory-hard, so a stolen `users` table
//! does not become a fast offline guessing target.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};

use crate::error::ApiError;

pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| {
            tracing::error!(%error, "password hashing failed");
            ApiError::Internal
        })
}

pub fn verify_password(password: &str, encoded: &str) -> bool {
    PasswordHash::new(encoded)
        .ok()
        .and_then(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .ok()
        })
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_only_the_original_password() {
        let encoded = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &encoded));
        assert!(!verify_password("wrong", &encoded));
        assert!(!verify_password(
            "correct horse battery staple",
            "not-a-hash"
        ));
    }
}
