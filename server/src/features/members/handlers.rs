//! Membership handlers.

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{Duration, Utc};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    db::audit,
    error::ApiError,
    features::{
        keys::wrapping::validate_wrapped_key,
        members::dto::{
            ChangeRoleRequest, InviteRequest, LookupRequest, LookupResponse, MemberResponse,
        },
    },
    security::{
        hash_token,
        permissions::{require_admin, require_member},
        random_token, AuthUser,
    },
    state::AppState,
};

/// Resolves an invitee's identity public key so the inviting admin can seal
/// the project key to it.
///
/// Restricted to admins of the project being invited to, which is the same
/// audience that could already learn whether an address is registered by
/// attempting the invite itself.
pub async fn lookup(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(request): Json<LookupRequest>,
) -> Result<Json<LookupResponse>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    let email = request.email.trim().to_lowercase();
    let row = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT id, username, public_key FROM users
         WHERE lower(email) = lower($1) AND email_verified_at IS NOT NULL",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ApiError::NotFound("the invited user must register before being added".to_string())
    })?;
    let public_key = row.2.ok_or_else(|| {
        ApiError::Conflict(
            "that account has not published an identity key yet; ask them to sign in once \
             with an up-to-date CLI first"
                .to_string(),
        )
    })?;
    Ok(Json(LookupResponse {
        user_id: row.0,
        username: row.1,
        public_key,
    }))
}

pub async fn invite(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(request): Json<InviteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    validate_wrapped_key(&request.wrapped_key)?;
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
    let project =
        sqlx::query_as::<_, (String, i32)>("SELECT name, key_version FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;
    let token = Zeroizing::new(random_token(32));
    let invitation_id = Uuid::new_v4();

    // The invitation and the key grant land together: an invitee who can join
    // but cannot decrypt is a worse outcome than a failed invite.
    let mut transaction = state.pool.begin().await?;
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
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO project_key_grants (id, project_id, user_id, key_version, wrapped_key, granted_by)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (project_id, user_id, key_version)
         DO UPDATE SET wrapped_key = EXCLUDED.wrapped_key, granted_by = EXCLUDED.granted_by",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(invitee_id)
    .bind(project.1)
    .bind(&request.wrapped_key)
    .bind(user.id)
    .execute(&mut *transaction)
    .await?;
    audit::record(
        &mut transaction,
        Some(user.id),
        Some(project_id),
        "collaborator.invited",
        "user",
        Some(invitee_id),
        serde_json::json!({ "email": email, "key_version": project.1 }),
    )
    .await?;
    transaction.commit().await?;

    if let Err(error) = state
        .email
        .send_invitation(&email, &project.0, &token, &state.config.public_cli_url)
        .await
    {
        tracing::error!(%error, invitation_id = %invitation_id, "invitation delivery failed");
        let _ = sqlx::query("DELETE FROM project_invitations WHERE id = $1")
            .bind(invitation_id)
            .execute(&state.pool)
            .await;
        return Err(ApiError::Internal);
    }
    Ok(Json(
        serde_json::json!({ "ok": true, "expires_in_hours": state.config.invite_ttl_hours }),
    ))
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<MemberResponse>>, ApiError> {
    require_member(&state.pool, project_id, user.id).await?;
    let members = sqlx::query_as::<_, MemberResponse>(
        "SELECT u.id AS user_id, u.username, u.email, m.role, u.public_key,
                (SELECT max(g.key_version) FROM project_key_grants g
                  WHERE g.project_id = m.project_id AND g.user_id = u.id) AS key_version,
                m.created_at AS joined_at
         FROM project_memberships m JOIN users u ON u.id = m.user_id
         WHERE m.project_id = $1
         ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, lower(u.username)",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(members))
}

/// Reports which key versions one member holds. Used by an admin to check
/// that nobody was left behind after a rotation or a re-invite.
pub async fn grant_status(
    State(state): State<AppState>,
    user: AuthUser,
    Path((project_id, target_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    let versions = sqlx::query_scalar::<_, i32>(
        "SELECT key_version FROM project_key_grants
         WHERE project_id = $1 AND user_id = $2 ORDER BY key_version",
    )
    .bind(project_id)
    .bind(target_id)
    .fetch_all(&state.pool)
    .await?;
    let current = sqlx::query_scalar::<_, i32>("SELECT key_version FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;
    Ok(Json(serde_json::json!({
        "key_versions": versions,
        "current_key_version": current,
        "holds_current_key": versions.contains(&current),
    })))
}

pub async fn remove(
    State(state): State<AppState>,
    user: AuthUser,
    Path((project_id, target_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    let actor_role = require_member(&state.pool, project_id, user.id).await?;
    let target_role = require_member(&state.pool, project_id, target_id).await?;
    if target_role == "owner" || (target_role == "admin" && actor_role != "owner") {
        return Err(ApiError::Forbidden);
    }
    let mut transaction = state.pool.begin().await?;
    sqlx::query("DELETE FROM project_memberships WHERE project_id = $1 AND user_id = $2")
        .bind(project_id)
        .bind(target_id)
        .execute(&mut *transaction)
        .await?;
    // Drops the server's copy of their wrapped keys. They may still hold the
    // data key they unwrapped while a member, which is exactly what a
    // rotation exists to retire.
    sqlx::query("DELETE FROM project_key_grants WHERE project_id = $1 AND user_id = $2")
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
    audit::record(
        &mut transaction,
        Some(user.id),
        Some(project_id),
        "collaborator.removed",
        "user",
        Some(target_id),
        serde_json::json!({ "rotation_required": true }),
    )
    .await?;
    transaction.commit().await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "rotation_required": true,
        "detail": "rotate the project key to retire the copy this member already holds"
    })))
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
    let actor_role = require_member(&state.pool, project_id, user.id).await?;
    let target_role = require_member(&state.pool, project_id, target_id).await?;
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
    audit::record_now(
        &state.pool,
        Some(user.id),
        Some(project_id),
        "collaborator.role_changed",
        "user",
        Some(target_id),
        serde_json::json!({ "old_role": target_role, "new_role": request.role }),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
