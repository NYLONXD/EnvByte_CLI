//! Project data keys.
//!
//! The server never sees a data key in the clear. It stores one sealed copy
//! per member, hands each member back only their own, and enforces that a
//! rotation is all-or-nothing.

pub mod dto;
pub mod handlers;
pub mod rotation;
pub mod wrapping;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{project_id}/keys", get(handlers::grants))
        .route(
            "/projects/{project_id}/key-rotations",
            post(handlers::rotate),
        )
}
