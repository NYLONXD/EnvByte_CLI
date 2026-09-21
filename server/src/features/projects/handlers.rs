//! Project lifecycle handlers.

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    db::{audit, models::Invitation},
    error::ApiError,
    features::{
        keys::wrapping::validate_wrapped_key,
        projects::{
            dto::{
                CreateProjectRequest, CreateProjectResponse, JoinRequest, JoinResponse,
                ProjectSummary, RollbackRequest,
            },
            naming::{duplicate_name_error, validate_project_name},
        },
    },
    security::{hash_token, random_token, AuthUser},
    state::AppState,
};

pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<CreateProjectResponse>, ApiError> {
    validate_project_name(&request.name)?;
    validate_wrapped_key(&request.wrapped_key)?;
    let project_id = Uuid::new_v4();
    let soft_token = Zeroizing::new(random_token(32));
    let mut transaction = state.pool.begin().await?;
    sqlx::query("INSERT INTO projects (id, name, owner_id, key_version) VALUES ($1, $2, $3, 1)")
        .bind(project_id)
        .bind(&request.name)
        .bind(user.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| duplicate_name_error(error, &request.name))?;
    sqlx::query(
        "INSERT INTO project_memberships (project_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(project_id)
    .bind(user.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO project_key_grants (id, project_id, user_id, key_version, wrapped_key, granted_by)
         VALUES ($1, $2, $3, 1, $4, $3)",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(user.id)
    .bind(&request.wrapped_key)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO device_sessions (id, project_id, user_id, soft_token_hash) VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(user.id)
    .bind(hash_token(&soft_token))
    .execute(&mut *transaction)
    .await?;
    audit::record(
        &mut transaction,
        Some(user.id),
        Some(project_id),
        "project.created",
        "project",
        Some(project_id),
        serde_json::json!({ "name": request.name }),
    )
    .await?;
    transaction.commit().await?;
    Ok(Json(CreateProjectResponse {
        project_id,
        key_version: 1,
        soft_token: soft_token.to_string(),
    }))
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<ProjectSummary>>, ApiError> {
    let projects = sqlx::query_as::<_, ProjectSummary>(
        "SELECT p.id, p.name, o.username AS owner_username, m.role, p.key_version
         FROM projects p
         JOIN project_memberships m ON m.project_id = p.id
         JOIN users o ON o.id = p.owner_id
         WHERE m.user_id = $1 ORDER BY lower(o.username), lower(p.name)",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(projects))
}

/// Redeems an invitation.
///
/// The project is identified by the invitation alone. It used to be named in
/// the path, which turned project names into a guessable global namespace;
/// nothing about joining ever needed the caller to know the name in advance.
pub async fn join(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<JoinRequest>,
) -> Result<Json<JoinResponse>, ApiError> {
    if request.ott.len() < 32
        || request.refresher_token.len() != 64
        || request
            .mac_address
            .as_ref()
            .is_some_and(|value| value.len() > 64)
    {
        return Err(ApiError::BadRequest(
            "invalid enrollment credentials".to_string(),
        ));
    }
    let token = Zeroizing::new(request.ott);
    let mut transaction = state.pool.begin().await?;
    let invitation = sqlx::query_as::<_, Invitation>(
        "SELECT i.id, i.project_id, i.user_id, i.email, i.expires_at, i.consumed_at
         FROM project_invitations i
         WHERE i.token_hash = $1
         FOR UPDATE OF i",
    )
    .bind(hash_token(&token))
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    if invitation.consumed_at.is_some() || invitation.expires_at <= Utc::now() {
        return Err(ApiError::Unauthorized);
    }
    // The invitation names one account. Holding the token is not proof of
    // being that account, so the caller must already be signed in as them.
    if invitation.user_id != user.id {
        return Err(ApiError::Forbidden);
    }
    let project = sqlx::query_as::<_, (String, String, i32)>(
        "SELECT p.name, o.username, p.key_version FROM projects p
         JOIN users o ON o.id = p.owner_id WHERE p.id = $1",
    )
    .bind(invitation.project_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;
    let soft_token = Zeroizing::new(random_token(32));
    sqlx::query(
        "INSERT INTO project_memberships (project_id, user_id, role) VALUES ($1, $2, 'member')
         ON CONFLICT (project_id, user_id) DO NOTHING",
    )
    .bind(invitation.project_id)
    .bind(invitation.user_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("UPDATE project_invitations SET consumed_at = now() WHERE id = $1")
        .bind(invitation.id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO device_sessions
         (id, project_id, user_id, mac_address, refresher_hash, soft_token_hash)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::new_v4())
    .bind(invitation.project_id)
    .bind(invitation.user_id)
    .bind(request.mac_address)
    .bind(hex::encode(Sha256::digest(
        request.refresher_token.as_bytes(),
    )))
    .bind(hash_token(&soft_token))
    .execute(&mut *transaction)
    .await?;
    audit::record(
        &mut transaction,
        Some(invitation.user_id),
        Some(invitation.project_id),
        "collaborator.joined",
        "user",
        Some(invitation.user_id),
        serde_json::json!({ "email": invitation.email }),
    )
    .await?;
    transaction.commit().await?;
    Ok(Json(JoinResponse {
        project_id: invitation.project_id,
        project_name: project.0,
        owner_username: project.1,
        key_version: project.2,
        soft_token: soft_token.to_string(),
    }))
}

pub async fn rollback(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(request): Json<RollbackRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::features::env_files::service::rollback_to_commit(
        &state,
        user.id,
        project_id,
        request.commit_id,
    )
    .await?;
    Ok(Json(
        serde_json::json!({ "ok": true, "commit_id": request.commit_id }),
    ))
}
