//! Audit trail reads.

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    error::ApiError, security::permissions::require_admin, security::AuthUser, state::AppState,
};

#[derive(Serialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    pub actor: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<AuditEvent>>, ApiError> {
    require_admin(&state.pool, project_id, user.id).await?;
    let events = sqlx::query_as::<_, AuditEvent>(
        "SELECT a.id, u.username AS actor, a.action, a.target_type, a.target_id, a.metadata,
                a.created_at
         FROM audit_events a LEFT JOIN users u ON u.id = a.actor_id
         WHERE a.project_id = $1 ORDER BY a.created_at DESC LIMIT 500",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(events))
}
