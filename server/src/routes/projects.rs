use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    error::ApiError,
    models::Invitation,
    permissions::require_admin,
    security::{hash_token, random_token, AuthUser},
    state::AppState,
};

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    name: String,
    master_key_hash: String,
}

#[derive(Serialize)]
pub struct CreateProjectResponse {
    project_id: Uuid,
    soft_token: String,
}

#[derive(Deserialize)]
pub struct InviteRequest {
    email: String,
}

#[derive(Deserialize)]
pub struct JoinRequest {
    ott: String,
    refresher_token: String,
    #[serde(default)]
    mac_address: Option<String>,
}

/// Deliberately carries no account credentials. An invitation proves that the
/// holder was invited, not that they are the invitee, so it must never be
/// exchangeable for a session.
#[derive(Serialize)]
pub struct JoinResponse {
    project_id: Uuid,
    soft_token: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ProjectSummary {
    id: Uuid,
    name: String,
    role: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MemberResponse {
    user_id: Uuid,
    username: String,
    email: String,
    role: String,
    joined_at: chrono::DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct ChangeRoleRequest {
    role: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AuditResponse {
    id: Uuid,
    actor: Option<String>,
    action: String,
    target_type: String,
    target_id: Option<Uuid>,
    metadata: serde_json::Value,
    created_at: chrono::DateTime<Utc>,
}

pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<CreateProjectResponse>, ApiError> {
    validate_project_name(&request.name)?;
    if request.master_key_hash.len() != 64
        || !request
            .master_key_hash
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(ApiError::BadRequest(
            "invalid master key verifier".to_string(),
        ));
    }
    let project_id = Uuid::new_v4();
    let soft_token = Zeroizing::new(random_token(32));
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO projects (id, name, owner_id, master_key_hash) VALUES ($1, $2, $3, $4)",
    )
    .bind(project_id)
    .bind(&request.name)
    .bind(user.id)
    .bind(request.master_key_hash.to_lowercase())
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO project_memberships (project_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(project_id)
    .bind(user.id)
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
    insert_audit(
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
        soft_token: soft_token.to_string(),
    }))
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<ProjectSummary>>, ApiError> {
    let projects = sqlx::query_as::<_, ProjectSummary>(
        "SELECT p.id, p.name, m.role FROM projects p
         JOIN project_memberships m ON m.project_id = p.id
         WHERE m.user_id = $1 ORDER BY lower(p.name)",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(projects))
}

pub async fn invite(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(request): Json<InviteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    let email = request.email.trim().to_lowercase();
    let invitee_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM users
         WHERE lower(email) = lower($1) AND email_verified_at IS NOT NULL",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ApiError::NotFound("the invited user must register before being added".to_string())
    })?;
    if invitee_id == user.id {
        return Err(ApiError::BadRequest(
            "you cannot invite yourself".to_string(),
        ));
    }
    let already_member = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM project_memberships WHERE project_id = $1 AND user_id = $2)",
    )
    .bind(project_id)
    .bind(invitee_id)
    .fetch_one(&state.pool)
    .await?;
    if already_member {
        return Err(ApiError::Conflict(
            "user is already a project member".to_string(),
        ));
    }
    let project_name = sqlx::query_scalar::<_, String>("SELECT name FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;
    let token = Zeroizing::new(random_token(32));
    let invitation_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO project_invitations
         (id, project_id, user_id, email, token_hash, invited_by, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(invitation_id)
    .bind(project_id)
    .bind(invitee_id)
    .bind(&email)
    .bind(hash_token(&token))
    .bind(user.id)
    .bind(Utc::now() + Duration::hours(state.config.invite_ttl_hours))
    .execute(&state.pool)
    .await?;

    if let Err(error) = state
        .email
        .send_invitation(&email, &project_name, &token, &state.config.public_cli_url)
        .await
    {
        tracing::error!(%error, invitation_id = %invitation_id, "invitation delivery failed");
        let _ = sqlx::query("DELETE FROM project_invitations WHERE id = $1")
            .bind(invitation_id)
            .execute(&state.pool)
            .await;
        return Err(ApiError::Internal);
    }
    sqlx::query(
        "INSERT INTO audit_events (id, actor_id, project_id, action, target_type, target_id, metadata)
         VALUES ($1, $2, $3, 'collaborator.invited', 'user', $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(project_id)
    .bind(invitee_id)
    .bind(serde_json::json!({ "email": email }))
    .execute(&state.pool)
    .await?;
    Ok(Json(
        serde_json::json!({ "ok": true, "expires_in_hours": state.config.invite_ttl_hours }),
    ))
}

pub async fn members(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<MemberResponse>>, ApiError> {
    crate::permissions::require_member(&state.pool, project_id, user.id).await?;
    let members = sqlx::query_as::<_, MemberResponse>(
        "SELECT u.id AS user_id, u.username, u.email, m.role, m.created_at AS joined_at
         FROM project_memberships m JOIN users u ON u.id = m.user_id
         WHERE m.project_id = $1
         ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, lower(u.username)",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(members))
}

pub async fn remove_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path((project_id, target_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    let actor_role = crate::permissions::require_member(&state.pool, project_id, user.id).await?;
    let target_role =
        crate::permissions::require_member(&state.pool, project_id, target_id).await?;
    if target_role == "owner" || (target_role == "admin" && actor_role != "owner") {
        return Err(ApiError::Forbidden);
    }
    let mut transaction = state.pool.begin().await?;
    sqlx::query("DELETE FROM project_memberships WHERE project_id = $1 AND user_id = $2")
        .bind(project_id)
        .bind(target_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "UPDATE device_sessions SET revoked_at = now() WHERE project_id = $1 AND user_id = $2",
    )
    .bind(project_id)
    .bind(target_id)
    .execute(&mut *transaction)
    .await?;
    insert_audit(
        &mut transaction,
        Some(user.id),
        Some(project_id),
        "collaborator.removed",
        "user",
        Some(target_id),
        serde_json::json!({}),
    )
    .await?;
    transaction.commit().await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn change_role(
    State(state): State<AppState>,
    user: AuthUser,
    Path((project_id, target_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<ChangeRoleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    if !matches!(request.role.as_str(), "admin" | "member" | "viewer") {
        return Err(ApiError::BadRequest(
            "role must be admin, member, or viewer".to_string(),
        ));
    }
    let actor_role = crate::permissions::require_member(&state.pool, project_id, user.id).await?;
    let target_role =
        crate::permissions::require_member(&state.pool, project_id, target_id).await?;
    if target_role == "owner"
        || actor_role != "owner" && (target_role == "admin" || request.role == "admin")
    {
        return Err(ApiError::Forbidden);
    }
    sqlx::query("UPDATE project_memberships SET role = $1 WHERE project_id = $2 AND user_id = $3")
        .bind(&request.role)
        .bind(project_id)
        .bind(target_id)
        .execute(&state.pool)
        .await?;
    sqlx::query(
        "INSERT INTO audit_events (id, actor_id, project_id, action, target_type, target_id, metadata)
         VALUES ($1, $2, $3, 'collaborator.role_changed', 'user', $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(project_id)
    .bind(target_id)
    .bind(serde_json::json!({ "old_role": target_role, "new_role": request.role }))
    .execute(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn audit(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<AuditResponse>>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    let events = sqlx::query_as::<_, AuditResponse>(
        "SELECT a.id, u.username AS actor, a.action, a.target_type, a.target_id, a.metadata, a.created_at
         FROM audit_events a LEFT JOIN users u ON u.id = a.actor_id
         WHERE a.project_id = $1 ORDER BY a.created_at DESC LIMIT 500",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(events))
}

pub async fn join(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_name): Path<String>,
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
         FROM project_invitations i JOIN projects p ON p.id = i.project_id
         WHERE i.token_hash = $1 AND lower(p.name) = lower($2)
         FOR UPDATE OF i",
    )
    .bind(hash_token(&token))
    .bind(&project_name)
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
    insert_audit(
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
        soft_token: soft_token.to_string(),
    }))
}

pub async fn rollback(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(request): Json<RollbackRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::routes::env_files::rollback_to_commit(&state, user.id, project_id, request.commit_id)
        .await?;
    Ok(Json(
        serde_json::json!({ "ok": true, "commit_id": request.commit_id }),
    ))
}

#[derive(Deserialize)]
pub struct RollbackRequest {
    commit_id: Uuid,
}

fn validate_project_name(value: &str) -> Result<(), ApiError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(ApiError::BadRequest("invalid project name".to_string()));
    }
    Ok(())
}

async fn insert_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor_id: Option<Uuid>,
    project_id: Option<Uuid>,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    metadata: serde_json::Value,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO audit_events (id, actor_id, project_id, action, target_type, target_id, metadata)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::new_v4())
    .bind(actor_id)
    .bind(project_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(metadata)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_project_names() {
        assert!(validate_project_name("api-production_2").is_ok());
        assert!(validate_project_name("bad project").is_err());
        assert!(validate_project_name("").is_err());
    }
}
