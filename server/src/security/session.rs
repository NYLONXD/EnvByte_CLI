//! Access tokens and the extractor that turns one into an authenticated user.

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{config::Config, error::ApiError, state::AppState};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    iss: String,
    iat: usize,
    exp: usize,
    jti: Uuid,
}

/// An authenticated caller. Handlers take this as an argument, so a route that
/// forgets to authenticate does not compile into existence unnoticed.
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
