//! Project key rotation.
//!
//! The client mints a fresh data key, seals it once per remaining member and
//! re-encrypts every file. The server's job is to make that all-or-nothing and
//! to refuse a partial rotation, because a half-applied rotation is worse than
//! none: a missing grant locks out a colleague, and a file left on the old key
//! stays readable to whoever the rotation was meant to shut out.

use std::collections::{HashMap, HashSet};

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    db::audit,
    error::ApiError,
    features::{
        env_files::service::{validate_filename, MAX_ENCRYPTED_CONTENT},
        keys::{
            dto::{RotateRequest, RotateResponse},
            wrapping::validate_wrapped_key,
        },
    },
    state::AppState,
};

pub async fn apply(
    state: &AppState,
    actor_id: Uuid,
    project_id: Uuid,
    request: RotateRequest,
) -> Result<RotateResponse, ApiError> {
    validate_payload(&request)?;

    let mut transaction = state.pool.begin().await?;
    let current_version =
        sqlx::query_scalar::<_, i32>("SELECT key_version FROM projects WHERE id = $1 FOR UPDATE")
            .bind(project_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;
    let next_version = current_version + 1;

    require_every_member_granted(&mut transaction, project_id, &request).await?;
    let files_by_name =
        require_every_file_reencrypted(&mut transaction, project_id, &request).await?;

    for grant in &request.grants {
        sqlx::query(
            "INSERT INTO project_key_grants (id, project_id, user_id, key_version, wrapped_key, granted_by)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(project_id)
        .bind(grant.user_id)
        .bind(next_version)
        .bind(&grant.wrapped_key)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await?;
    }

    // Grants belonging to people who have left go entirely; earlier versions
    // held by current members stay, so project history remains readable.
    let revoked = sqlx::query(
        "DELETE FROM project_key_grants
         WHERE project_id = $1
           AND user_id NOT IN (SELECT user_id FROM project_memberships WHERE project_id = $1)",
    )
    .bind(project_id)
    .execute(&mut *transaction)
    .await?
    .rows_affected();

    for file in &request.files {
        let file_id = files_by_name
            .get(&file.filename)
            .copied()
            .ok_or_else(|| ApiError::BadRequest("unknown file in rotation".to_string()))?;
        let version_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO env_versions
             (id, commit_id, env_file_id, encrypted_content, message, author_id, key_version)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(version_id)
        .bind(Uuid::new_v4())
        .bind(file_id)
        .bind(&file.content)
        .bind(format!("key rotation to version {next_version}"))
        .bind(actor_id)
        .bind(next_version)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE env_files SET current_version_id = $1, updated_at = now() WHERE id = $2",
        )
        .bind(version_id)
        .bind(file_id)
        .execute(&mut *transaction)
        .await?;
    }

    sqlx::query("UPDATE projects SET key_version = $1, updated_at = now() WHERE id = $2")
        .bind(next_version)
        .bind(project_id)
        .execute(&mut *transaction)
        .await?;
    audit::record(
        &mut transaction,
        Some(actor_id),
        Some(project_id),
        "project.key_rotated",
        "project",
        Some(project_id),
        serde_json::json!({
            "from_version": current_version,
            "to_version": next_version,
            "members_granted": request.grants.len(),
            "files_reencrypted": request.files.len(),
            "stale_grants_revoked": revoked,
        }),
    )
    .await?;
    transaction.commit().await?;

    Ok(RotateResponse {
        key_version: next_version,
        members_granted: request.grants.len(),
        files_reencrypted: request.files.len(),
        grants_revoked: revoked,
    })
}

fn validate_payload(request: &RotateRequest) -> Result<(), ApiError> {
    for grant in &request.grants {
        validate_wrapped_key(&grant.wrapped_key)?;
    }
    for file in &request.files {
        validate_filename(&file.filename)?;
        if file.content.is_empty() || file.content.len() > MAX_ENCRYPTED_CONTENT {
            return Err(ApiError::BadRequest(
                "invalid encrypted payload size".to_string(),
            ));
        }
    }
    Ok(())
}

/// Every current member, and only current members, must receive the new key.
async fn require_every_member_granted(
    transaction: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    request: &RotateRequest,
) -> Result<(), ApiError> {
    let member_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM project_memberships WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    let granted = request
        .grants
        .iter()
        .map(|grant| grant.user_id)
        .collect::<HashSet<_>>();
    if granted.len() != request.grants.len() {
        return Err(ApiError::BadRequest(
            "duplicate user in rotation grants".to_string(),
        ));
    }
    if granted != member_ids {
        return Err(ApiError::BadRequest(format!(
            "rotation must grant the new key to all {} current members and no one else, \
             but {} were supplied; refresh the member list and retry",
            member_ids.len(),
            granted.len()
        )));
    }
    Ok(())
}

/// Every stored file must move to the new key, or the retired key would still
/// open whatever was left behind. Returns the file ids keyed by filename.
async fn require_every_file_reencrypted(
    transaction: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    request: &RotateRequest,
) -> Result<HashMap<String, Uuid>, ApiError> {
    let stored = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, filename FROM env_files WHERE project_id = $1 FOR UPDATE",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await?;
    let expected = stored
        .iter()
        .map(|(_, name)| name.clone())
        .collect::<HashSet<String>>();
    let supplied = request
        .files
        .iter()
        .map(|file| file.filename.clone())
        .collect::<HashSet<_>>();
    if supplied.len() != request.files.len() {
        return Err(ApiError::BadRequest(
            "duplicate filename in rotation payload".to_string(),
        ));
    }
    if supplied != expected {
        return Err(ApiError::BadRequest(format!(
            "rotation must re-encrypt all {} stored files and no others, but {} were supplied",
            expected.len(),
            supplied.len()
        )));
    }
    Ok(stored.into_iter().map(|(id, name)| (name, id)).collect())
}
