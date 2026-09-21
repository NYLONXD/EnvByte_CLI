//! The project audit trail.
//!
//! Every feature that mutates membership or data records an event, so the
//! writer lives here rather than being copied into each one. Events are
//! append-only and must never carry secret material - only identifiers and
//! the shape of what changed.

use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::ApiError;

/// Records an event inside a caller-owned transaction, so the audit entry
/// commits or rolls back together with the change it describes.
pub async fn record(
    transaction: &mut Transaction<'_, Postgres>,
    actor_id: Option<Uuid>,
    project_id: Option<Uuid>,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    metadata: Value,
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

/// Records an event on its own, for changes that are already a single
/// statement and need no surrounding transaction.
pub async fn record_now(
    pool: &PgPool,
    actor_id: Option<Uuid>,
    project_id: Option<Uuid>,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    metadata: Value,
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
    .execute(pool)
    .await?;
    Ok(())
}
