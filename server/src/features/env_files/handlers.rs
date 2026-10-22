//! Environment-file handlers.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    db::{audit, models::EnvFileResponse},
    error::ApiError,
    features::env_files::{
        dto::{CommitResponse, PullQuery, PushRequest, PushResponse},
        service::{
            require_current_base, validate_filename, verify_key_version, MAX_ENCRYPTED_CONTENT,
        },
    },
    security::{
        permissions::{require_member, require_writer},
        AuthUser,
    },
    state::AppState,
};

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

    // A retry of a push that already landed succeeds here, before the base
    // check, which it would now fail against its own commit.
    if let Some(commit_id) = request.commit_id {
        if let Some(existing) = replayed_commit(&state, commit_id).await? {
            return existing_matches(&request, existing).map(|()| {
                Json(PushResponse {
                    commit_id,
                    key_version: request.key_version,
                })
            });
        }
    }

    let mut transaction = state.pool.begin().await?;
    verify_key_version(&mut transaction, request.project_id, request.key_version).await?;
    let (file_id, current_commit) =
        lock_file(&mut transaction, request.project_id, &request.filename).await?;
    if let Some(base) = request.base_commit_id {
        require_current_base(base, current_commit)?;
    }
    let version_id = Uuid::new_v4();
    let commit_id = request.commit_id.unwrap_or_else(Uuid::new_v4);
    sqlx::query(
        "INSERT INTO env_versions
         (id, commit_id, env_file_id, encrypted_content, message, author_id, key_version)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(version_id)
    .bind(commit_id)
    .bind(file_id)
    .bind(&request.content)
    .bind(request.message.trim())
    .bind(user.id)
    .bind(request.key_version)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("UPDATE env_files SET current_version_id = $1, updated_at = now() WHERE id = $2")
        .bind(version_id)
        .bind(file_id)
        .execute(&mut *transaction)
        .await?;
    audit::record(
        &mut transaction,
        Some(user.id),
        Some(request.project_id),
        "env.pushed",
        "env_version",
        Some(version_id),
        serde_json::json!({
            "filename": request.filename,
            "commit_id": commit_id,
            "key_version": request.key_version,
        }),
    )
    .await?;
    transaction.commit().await?;
    Ok(Json(PushResponse {
        commit_id,
        key_version: request.key_version,
    }))
}

pub async fn pull(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<PullQuery>,
) -> Result<Json<Vec<EnvFileResponse>>, ApiError> {
    require_member(&state.pool, query.project_id, user.id).await?;
    let files = sqlx::query_as::<_, EnvFileResponse>(
        "SELECT f.filename, v.encrypted_content, v.key_version, v.commit_id FROM env_files f
         JOIN env_versions v ON v.id = f.current_version_id
         WHERE f.project_id = $1 ORDER BY f.filename",
    )
    .bind(query.project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(files))
}

/// One stored version of a file, current or not. A client merging its edit
/// with someone else's needs the version both started from.
///
/// Writers only: they can already roll the project back to any version and
/// pull it, so this reveals nothing new. Viewers see the current files alone.
pub async fn commit(
    State(state): State<AppState>,
    user: AuthUser,
    Path((project_id, commit_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<EnvFileResponse>, ApiError> {
    require_writer(&state.pool, project_id, user.id).await?;
    let file = sqlx::query_as::<_, EnvFileResponse>(
        "SELECT f.filename, v.encrypted_content, v.key_version, v.commit_id
         FROM env_versions v JOIN env_files f ON f.id = v.env_file_id
         WHERE v.commit_id = $1 AND f.project_id = $2",
    )
    .bind(commit_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("commit not found in this project".to_string()))?;
    Ok(Json(file))
}

pub async fn history(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<CommitResponse>>, ApiError> {
    require_member(&state.pool, project_id, user.id).await?;
    let commits = sqlx::query_as::<_, CommitResponse>(
        "SELECT v.commit_id, f.filename, v.message, u.username AS author, v.key_version,
                v.created_at
         FROM env_versions v JOIN env_files f ON f.id = v.env_file_id
         JOIN users u ON u.id = v.author_id
         WHERE f.project_id = $1 ORDER BY v.created_at DESC LIMIT 500",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(commits))
}

/// Creates the file's row on its first push, then locks it, so pushes to one
/// file queue up rather than race. Returns the file and its current commit,
/// which cannot move while the lock is held.
///
/// Two statements, not a join: a locking read that waits on another push
/// re-reads only the locked row, and would pair it with a stale version.
async fn lock_file(
    transaction: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    filename: &str,
) -> Result<(Uuid, Option<Uuid>), ApiError> {
    sqlx::query(
        "INSERT INTO env_files (id, project_id, filename) VALUES ($1, $2, $3)
         ON CONFLICT (project_id, filename) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(filename)
    .execute(&mut **transaction)
    .await?;
    let (file_id, current_version) = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
        "SELECT id, current_version_id FROM env_files
         WHERE project_id = $1 AND filename = $2 FOR UPDATE",
    )
    .bind(project_id)
    .bind(filename)
    .fetch_one(&mut **transaction)
    .await?;
    let current_commit = match current_version {
        Some(version_id) => {
            sqlx::query_scalar::<_, Uuid>("SELECT commit_id FROM env_versions WHERE id = $1")
                .bind(version_id)
                .fetch_optional(&mut **transaction)
                .await?
        }
        None => None,
    };
    Ok((file_id, current_commit))
}

/// A client-supplied commit id makes a retried push idempotent. Looks up what
/// was already stored under that id, if anything.
async fn replayed_commit(
    state: &AppState,
    commit_id: Uuid,
) -> Result<Option<(Uuid, String, String, String)>, ApiError> {
    let row = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT f.project_id, f.filename, v.encrypted_content, v.message
         FROM env_versions v JOIN env_files f ON f.id = v.env_file_id
         WHERE v.commit_id = $1",
    )
    .bind(commit_id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(row)
}

/// Accepts a replay only when it is byte-for-byte the same request; a reused
/// id with different content is a client bug worth surfacing.
fn existing_matches(
    request: &PushRequest,
    existing: (Uuid, String, String, String),
) -> Result<(), ApiError> {
    let (project_id, filename, content, message) = existing;
    let same = project_id == request.project_id
        && filename == request.filename
        && content == request.content
        && message == request.message.trim();
    if same {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "commit ID was already used for a different request".to_string(),
        ))
    }
}
