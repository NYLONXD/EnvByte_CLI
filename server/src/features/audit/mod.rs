//! Reading a project's audit trail. Writing it lives in `db::audit`, because
//! every feature records events.

pub mod handlers;

use axum::{routing::get, Router};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/projects/{project_id}/audit", get(handlers::list))
}
