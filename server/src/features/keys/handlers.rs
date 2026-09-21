//! Key handlers: reading your own grants, and starting a rotation.

use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::{
    db::models::KeyGrant,
    error::ApiError,
    features::keys::{
        dto::{RotateRequest, RotateResponse},
        rotation,
    },
    security::{
        permissions::{require_admin, require_member},
        AuthUser,
    },
    state::AppState,
};

/// Returns every data-key grant held by the caller for this project.
///
/// All versions are returned, not just the current one, so that history and
/// rolled-back files written under a retired key stay readable to members who
/// were present at the time.
pub async fn grants(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<KeyGrant>>, ApiError> {
    require_member(&state.pool, project_id, user.id).await?;
    let grants = sqlx::query_as::<_, KeyGrant>(
        "SELECT key_version, wrapped_key FROM project_key_grants
         WHERE project_id = $1 AND user_id = $2 ORDER BY key_version",
    )
    .bind(project_id)
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    if grants.is_empty() {
        return Err(ApiError::NotFound(
            "you hold no key for this project; ask an admin to re-invite you so the project \
             key is sealed to your identity"
                .to_string(),
        ));
    }
    Ok(Json(grants))
}

pub async fn rotate(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(request): Json<RotateRequest>,
) -> Result<Json<RotateResponse>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    rotation::apply(&state, user.id, project_id, request)
        .await
        .map(Json)
}
