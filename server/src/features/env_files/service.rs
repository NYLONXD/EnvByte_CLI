//! Rules the environment-file handlers share: payload bounds, filename safety,
//! key-version and base-commit enforcement, and rollback.

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{db::audit, error::ApiError, state::AppState};

pub const MAX_ENCRYPTED_CONTENT: usize = 16 * 1024 * 1024;

/// Rejects content encrypted under anything but the project's current data key.
///
/// After a rotation this is what stops a client that still holds the retired
/// key from writing a file the rest of the team can no longer read. The
/// project row stays share-locked until the caller commits, and a rotation
/// locks it exclusively, so no rotation can land between this check and the
/// write it guards.
pub async fn verify_key_version(
    transaction: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    presented: i32,
) -> Result<(), ApiError> {
    let expected =
        sqlx::query_scalar::<_, i32>("SELECT key_version FROM projects WHERE id = $1 FOR SHARE")
            .bind(project_id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;
    if presented != expected {
        return Err(ApiError::Conflict(format!(
            "this content was encrypted with project key version {presented}, but the project \
             is on version {expected}; pull again to pick up the current key before pushing"
        )));
    }
    Ok(())
}

/// Refuses a write edited from anything but the file's current commit, so a
/// device cannot overwrite a change it never saw. `None` on either side
/// means the file has no content yet.
pub fn require_current_base(base: Option<Uuid>, current: Option<Uuid>) -> Result<(), ApiError> {
    if base == current {
        return Ok(());
    }
    let message = if base.is_none() {
        "this file already exists on the server; pull it and merge your changes before pushing"
    } else {
        "this file has changed on the server since you last synced it; pull and merge before pushing"
    };
    Err(ApiError::Stale {
        message: message.to_string(),
        current_commit_id: current,
    })
}

/// Accepts only a bare `.env*` filename in the project root - no directories,
/// no traversal, no absolute paths.
pub fn validate_filename(value: &str) -> Result<(), ApiError> {
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

pub async fn rollback_to_commit(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
    commit_id: Uuid,
) -> Result<(), ApiError> {
    crate::security::permissions::require_writer(&state.pool, project_id, user_id).await?;
    let row = sqlx::query_as::<_, (Uuid, Uuid, String, i32)>(
        "SELECT v.id, f.id, f.filename, v.key_version
         FROM env_versions v JOIN env_files f ON f.id = v.env_file_id
         WHERE v.commit_id = $1 AND f.project_id = $2",
    )
    .bind(commit_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("commit not found in this project".to_string()))?;
    let (version_id, file_id, filename, key_version) = row;
    let mut transaction = state.pool.begin().await?;
    sqlx::query("UPDATE env_files SET current_version_id = $1, updated_at = now() WHERE id = $2")
        .bind(version_id)
        .bind(file_id)
        .execute(&mut *transaction)
        .await?;
    audit::record(
        &mut transaction,
        Some(user_id),
        Some(project_id),
        "env.rolled_back",
        "env_version",
        Some(version_id),
        serde_json::json!({
            "filename": filename,
            "commit_id": commit_id,
            "key_version": key_version,
        }),
    )
    .await?;
    transaction.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_current_commit_as_a_base() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        assert!(require_current_base(None, None).is_ok());
        assert!(require_current_base(Some(first), Some(first)).is_ok());
        assert!(matches!(
            require_current_base(Some(first), Some(second)),
            Err(ApiError::Stale { current_commit_id: Some(id), .. }) if id == second
        ));
        assert!(require_current_base(None, Some(first)).is_err());
        assert!(require_current_base(Some(first), None).is_err());
    }

    #[test]
    fn rejects_filename_traversal() {
        assert!(validate_filename(".env.production").is_ok());
        assert!(validate_filename("../.env").is_err());
        assert!(validate_filename(".env/secret").is_err());
        assert!(validate_filename("/etc/.env").is_err());
        assert!(validate_filename("secrets.txt").is_err());
    }
}
