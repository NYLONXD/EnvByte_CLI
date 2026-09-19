use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{
    error::ApiError,
    models::EnvFileResponse,
    permissions::{require_member, require_writer},
    security::AuthUser,
    state::AppState,
};

const MAX_ENCRYPTED_CONTENT: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
pub struct PushRequest {
    project_id: Uuid,
    #[serde(default)]
    commit_id: Option<Uuid>,
    filename: String,
    content: String,
    message: String,
    /// Verifier for the project master key the content was encrypted under.
    /// The server cannot decrypt, so this is the only way to stop a member
    /// with the wrong key from overwriting the file with an unreadable
    /// payload.
    master_key_hash: String,
}

#[derive(Serialize)]
pub struct PushResponse {
    commit_id: Uuid,
}

#[derive(Deserialize)]
pub struct PullQuery {
    project_id: Uuid,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CommitResponse {
    commit_id: Uuid,
    filename: String,
    message: String,
    author: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn push(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<PushRequest>,
) -> Result<Json<PushResponse>, ApiError> {
    require_writer(&state.pool, request.project_id, user.id).await?;
    validate_filename(&request.filename)?;
    if request.content.is_empty() || request.content.len() > MAX_ENCRYPTED_CONTENT {
        return Err(ApiError::BadRequest(
            "invalid encrypted payload size".to_string(),
        ));
    }
    if request.message.trim().is_empty() || request.message.len() > 500 {
        return Err(ApiError::BadRequest(
            "commit message must be 1-500 characters".to_string(),
        ));
    }
    verify_master_key(&state, request.project_id, &request.master_key_hash).await?;
    if let Some(commit_id) = request.commit_id {
        let existing = sqlx::query(
            "SELECT f.project_id, f.filename, v.encrypted_content, v.message
             FROM env_versions v JOIN env_files f ON f.id = v.env_file_id
             WHERE v.commit_id = $1",
        )
        .bind(commit_id)
        .fetch_optional(&state.pool)
        .await?;
        if let Some(existing) = existing {
            let same_request = existing.try_get::<Uuid, _>("project_id").ok()
                == Some(request.project_id)
                && existing.try_get::<String, _>("filename").ok().as_deref()
                    == Some(request.filename.as_str())
                && existing
                    .try_get::<String, _>("encrypted_content")
                    .ok()
                    .as_deref()
                    == Some(request.content.as_str())
                && existing.try_get::<String, _>("message").ok().as_deref()
                    == Some(request.message.trim());
            if same_request {
                return Ok(Json(PushResponse { commit_id }));
            }
            return Err(ApiError::Conflict(
                "commit ID was already used for a different request".to_string(),
            ));
        }
    }
    let mut transaction = state.pool.begin().await?;
    let existing =
        sqlx::query("SELECT id FROM env_files WHERE project_id = $1 AND filename = $2 FOR UPDATE")
            .bind(request.project_id)
            .bind(&request.filename)
            .fetch_optional(&mut *transaction)
            .await?;
    let file_id = match existing {
        Some(row) => row
            .try_get::<Uuid, _>("id")
            .map_err(|_| ApiError::Internal)?,
        None => {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO env_files (id, project_id, filename) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(request.project_id)
                .bind(&request.filename)
                .execute(&mut *transaction)
                .await?;
            id
        }
    };
    let version_id = Uuid::new_v4();
    let commit_id = request.commit_id.unwrap_or_else(Uuid::new_v4);
    sqlx::query(
        "INSERT INTO env_versions
         (id, commit_id, env_file_id, encrypted_content, message, author_id)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(version_id)
    .bind(commit_id)
    .bind(file_id)
    .bind(request.content)
    .bind(request.message.trim())
    .bind(user.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("UPDATE env_files SET current_version_id = $1, updated_at = now() WHERE id = $2")
        .bind(version_id)
        .bind(file_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO audit_events (id, actor_id, project_id, action, target_type, target_id, metadata)
         VALUES ($1, $2, $3, 'env.pushed', 'env_version', $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(request.project_id)
    .bind(version_id)
    .bind(serde_json::json!({ "filename": request.filename, "commit_id": commit_id }))
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Json(PushResponse { commit_id }))
}

pub async fn pull(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<PullQuery>,
) -> Result<Json<Vec<EnvFileResponse>>, ApiError> {
    require_member(&state.pool, query.project_id, user.id).await?;
    let files = sqlx::query_as::<_, EnvFileResponse>(
        "SELECT f.filename, v.encrypted_content FROM env_files f
         JOIN env_versions v ON v.id = f.current_version_id
         WHERE f.project_id = $1 ORDER BY f.filename",
    )
    .bind(query.project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(files))
}

pub async fn history(
    State(state): State<AppState>,
    user: AuthUser,
    axum::extract::Path(project_id): axum::extract::Path<Uuid>,
) -> Result<Json<Vec<CommitResponse>>, ApiError> {
    require_member(&state.pool, project_id, user.id).await?;
    let commits = sqlx::query_as::<_, CommitResponse>(
        "SELECT v.commit_id, f.filename, v.message, u.username AS author, v.created_at
         FROM env_versions v JOIN env_files f ON f.id = v.env_file_id
         JOIN users u ON u.id = v.author_id
         WHERE f.project_id = $1 ORDER BY v.created_at DESC LIMIT 500",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(commits))
}

pub async fn rollback_to_commit(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
    commit_id: Uuid,
) -> Result<(), ApiError> {
    require_writer(&state.pool, project_id, user_id).await?;
    let row = sqlx::query(
        "SELECT v.id AS version_id, f.id AS file_id, f.filename
         FROM env_versions v JOIN env_files f ON f.id = v.env_file_id
         WHERE v.commit_id = $1 AND f.project_id = $2",
    )
    .bind(commit_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("commit not found in this project".to_string()))?;
    let version_id: Uuid = row.try_get("version_id").map_err(|_| ApiError::Internal)?;
    let file_id: Uuid = row.try_get("file_id").map_err(|_| ApiError::Internal)?;
    let filename: String = row.try_get("filename").map_err(|_| ApiError::Internal)?;
    let mut transaction = state.pool.begin().await?;
    sqlx::query("UPDATE env_files SET current_version_id = $1, updated_at = now() WHERE id = $2")
        .bind(version_id)
        .bind(file_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO audit_events (id, actor_id, project_id, action, target_type, target_id, metadata)
         VALUES ($1, $2, $3, 'env.rolled_back', 'env_version', $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(project_id)
    .bind(version_id)
    .bind(serde_json::json!({ "filename": filename, "commit_id": commit_id }))
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

/// Rejects content encrypted under anything but the project's master key.
///
/// Compared in constant time so the stored verifier cannot be recovered by
/// timing a series of guesses.
async fn verify_master_key(
    state: &AppState,
    project_id: Uuid,
    presented: &str,
) -> Result<(), ApiError> {
    let expected =
        sqlx::query_scalar::<_, String>("SELECT master_key_hash FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;
    let presented = presented.to_lowercase();
    if presented.len() != expected.len()
        || presented.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() != 1
    {
        return Err(ApiError::Conflict(
            concat!(
                "this content was encrypted with a different master key than the project's; ",
                "check GREENBYTE_MASTER_KEY or the key you entered, because pushing it would ",
                "leave the file unreadable for everyone else"
            )
            .to_string(),
        ));
    }
    Ok(())
}

fn validate_filename(value: &str) -> Result<(), ApiError> {
    let path = std::path::Path::new(value);
    if !value.starts_with(".env")
        || value.len() > 255
        || path.is_absolute()
        || path.components().count() != 1
    {
        return Err(ApiError::BadRequest(
            "invalid environment filename".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_filename_traversal() {
        assert!(validate_filename(".env.production").is_ok());
        assert!(validate_filename("../.env").is_err());
        assert!(validate_filename(".env/secret").is_err());
    }
}
