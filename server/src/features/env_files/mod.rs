//! Encrypted environment files: push, pull, history and rollback.
//!
//! The server treats every payload as opaque bytes. Its only guarantees are
//! that the caller may write, that the payload is bounded, and that it was
//! encrypted under the project's current key.

pub mod dto;
pub mod handlers;
pub mod service;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users/me/env", post(handlers::push).get(handlers::pull))
        .route("/projects/{project_id}/commits", get(handlers::history))
}
