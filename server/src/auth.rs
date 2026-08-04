use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{config::Config, error::ApiError, models::User, state::AppState};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    iss: String,
    iat: usize,
    exp: usize,
    jti: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub id: Uuid,
}

impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let value = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[state.config.jwt_issuer.as_str()]);
        let token = decode::<Claims>(
            value,
            &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| ApiError::Unauthorized)?;
        Ok(Self {
            id: token.claims.sub,
        })
    }
}

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

pub fn access_token(user_id: Uuid, config: &Config) -> Result<String, ApiError> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        iss: config.jwt_issuer.clone(),
        iat: now.timestamp() as usize,
        exp: (now.timestamp() + config.access_token_ttl_seconds) as usize,
        jti: Uuid::new_v4(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|error| {
        tracing::error!(%error, "JWT signing failed");
        ApiError::Internal
    })
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

pub async fn rotate_refresh_token(
    transaction: &mut Transaction<'_, Postgres>,
    raw_token: &str,
    config: &Config,
) -> Result<(User, Zeroizing<String>), ApiError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT u.id, u.username, u.email, u.password_hash
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

pub fn random_token(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    rand::thread_rng().fill_bytes(&mut value);
    hex::encode(value)
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
