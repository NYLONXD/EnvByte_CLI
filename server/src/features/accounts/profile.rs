//! The signed-in account's own profile and identity key.

use axum::{extract::State, Json};
use uuid::Uuid;

use crate::{
    db::models::User,
    error::ApiError,
    features::accounts::{
        dto::{MeResponse, PublishKeyRequest},
        validation::validate_public_key,
    },
    security::AuthUser,
    state::AppState,
};

pub async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<MeResponse>, ApiError> {
    let record = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, public_key FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    Ok(Json(MeResponse {
        id: record.id,
        username: record.username,
        email: record.email,
        public_key: record.public_key,
    }))
}

/// Publishes this account's X25519 identity public key.
///
/// The key is public by design - collaborators need it to seal a project key
/// for this account. Replacing it does not revoke existing grants, because
/// those were sealed to the previous key and only the holder of the matching
/// secret can open them; an admin re-grants after a client changes identity.
pub async fn publish_key(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<PublishKeyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    validate_public_key(&request.public_key)?;
    let previous = sqlx::query_scalar::<_, Option<String>>(
        "SELECT public_key FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    if previous.as_deref() == Some(request.public_key.as_str()) {
        return Ok(Json(serde_json::json!({ "ok": true, "changed": false })));
    }
    sqlx::query(
        "UPDATE users SET public_key = $1, public_key_updated_at = now(), updated_at = now()
         WHERE id = $2",
    )
    .bind(&request.public_key)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    crate::db::audit::record_now(
        &state.pool,
        Some(user.id),
        None,
        "identity.key_published",
        "user",
        Some(user.id),
        serde_json::json!({ "replaced_existing": previous.is_some() }),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "changed": true,
        "replaced_existing": previous.is_some(),
    })))
}

/// Resolves the identity key a project key must be sealed to.
///
/// Shared by the invite and rotation paths, which both need to fail loudly
/// rather than silently skip a member who has no key.
pub async fn require_public_key(pool: &sqlx::PgPool, user_id: Uuid) -> Result<String, ApiError> {
    sqlx::query_scalar::<_, Option<String>>("SELECT public_key FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .flatten()
        .ok_or_else(|| {
            ApiError::Conflict(
                "that account has not published an identity key yet; ask them to sign in once \
                 with an up-to-date CLI first"
                    .to_string(),
            )
        })
}
