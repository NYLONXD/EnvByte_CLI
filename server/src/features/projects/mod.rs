//! Projects: creation, listing, joining by invitation, and rollback.
//!
//! Membership lives in `features::members` and key material in
//! `features::keys`, so this module stays about the project record itself.

pub mod dto;
pub mod handlers;
pub mod naming;

use axum::{routing::post, Router};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects", post(handlers::create).get(handlers::list))
        // Identified by the invitation alone, so project names never have to
        // be guessable or globally unique.
        .route("/projects/join", post(handlers::join))
        .route("/projects/{project_id}/rollback", post(handlers::rollback))
}
