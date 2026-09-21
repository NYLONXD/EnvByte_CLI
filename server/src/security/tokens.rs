//! Opaque bearer tokens: refresh sessions, invitations, one-time tokens.
//!
//! Only hashes are stored. A leaked database therefore yields nothing that can
//! be replayed against the API.

use chrono::{Duration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{config::Config, db::models::User, error::ApiError};

pub fn random_token(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    rand::thread_rng().fill_bytes(&mut value);
    hex::encode(value)
}

/// Domain-free SHA-256. The inputs are always full-entropy `random_token`
/// output, so a fast hash is enough to make the stored value unusable.
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub async fn issue_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    config: &Config,
) -> Result<Zeroizing<String>, ApiError> {
    let token = Zeroizing::new(random_token(32));
    sqlx::query(
        "INSERT INTO refresh_sessions (id, user_id, token_hash, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(hash_token(&token))
    .bind(Utc::now() + Duration::days(config.refresh_token_ttl_days))
    .execute(pool)
    .await?;
    Ok(token)
}

/// Exchanges a refresh token for a fresh one, revoking the presented token in
/// the same transaction so a captured token cannot be replayed.
pub async fn rotate_refresh_token(
    transaction: &mut Transaction<'_, Postgres>,
    raw_token: &str,
    config: &Config,
) -> Result<(User, Zeroizing<String>), ApiError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT u.id, u.username, u.email, u.password_hash, u.public_key
         FROM refresh_sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1 AND s.revoked_at IS NULL AND s.expires_at > now()
         FOR UPDATE OF s",
    )
    .bind(hash_token(raw_token))
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ApiError::Unauthorized)?;

    sqlx::query("UPDATE refresh_sessions SET revoked_at = now() WHERE token_hash = $1")
        .bind(hash_token(raw_token))
        .execute(&mut **transaction)
        .await?;

    let replacement = Zeroizing::new(random_token(32));
    sqlx::query(
        "INSERT INTO refresh_sessions (id, user_id, token_hash, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(hash_token(&replacement))
    .bind(Utc::now() + Duration::days(config.refresh_token_ttl_days))
    .execute(&mut **transaction)
    .await?;
    Ok((user, replacement))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_hashes_are_stable() {
        let first = random_token(32);
        let second = random_token(32);
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
        assert_eq!(hash_token(&first), hash_token(&first));
        assert_ne!(hash_token(&first), hash_token(&second));
    }
}
