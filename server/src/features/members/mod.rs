//! Project membership: invitations, roles and removal.
//!
//! Every path here also touches key material, because adding a member means
//! sealing the project key to them and removing one means dropping it.

pub mod dto;
pub mod handlers;

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{project_id}/collaborators",
            post(handlers::invite).get(handlers::list),
        )
        .route(
            "/projects/{project_id}/collaborators/lookup",
            post(handlers::lookup),
        )
        .route(
            "/projects/{project_id}/collaborators/{user_id}",
            delete(handlers::remove).patch(handlers::change_role),
        )
        .route(
            "/projects/{project_id}/collaborators/{user_id}/grants",
            get(handlers::grant_status),
        )
}
