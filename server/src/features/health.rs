//! Liveness and readiness probes.
//!
//! `live` answers as long as the process is up. `ready` additionally proves the
//! database is reachable, which is what a load balancer should gate traffic on.

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
}

pub async fn live() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(serde_json::json!({ "status": "ready" })))
}
