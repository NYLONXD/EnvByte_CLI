use axum::{extract::State, Json};
use serde::Serialize;
use uuid::Uuid;

use crate::{error::ApiError, models::User, security::AuthUser, state::AppState};

#[derive(Serialize)]
pub struct MeResponse {
    id: Uuid,
    username: String,
    email: String,
}

pub async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<MeResponse>, ApiError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    Ok(Json(MeResponse {
        id: user.id,
        username: user.username,
        email: user.email,
    }))
}
