use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

pub async fn require_member(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<String, ApiError> {
    sqlx::query_scalar::<_, String>(
        "SELECT role FROM project_memberships WHERE project_id = $1 AND user_id = $2",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::Forbidden)
}

pub async fn require_writer(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<String, ApiError> {
    let role = require_member(pool, project_id, user_id).await?;
    if role == "viewer" {
        return Err(ApiError::Forbidden);
    }
    Ok(role)
}

pub async fn require_admin(pool: &PgPool, project_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
    let role = require_member(pool, project_id, user_id).await?;
    if !matches!(role.as_str(), "owner" | "admin") {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}
