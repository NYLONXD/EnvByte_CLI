use axum::{extract::State, http::StatusCode, Json};

use crate::state::AppState;

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
