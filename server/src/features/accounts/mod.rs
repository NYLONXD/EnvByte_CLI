//! Accounts: registration, email verification, sessions, password recovery,
//! and the identity public key a member publishes so colleagues can seal
//! project keys to them.

pub mod dto;
pub mod profile;
pub mod session;
pub mod validation;

use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};

use crate::{infra::rate_limit, state::AppState};

/// Endpoints that accept credentials, held to the stricter auth rate limit.
pub fn credential_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/auth/signup", post(session::signup))
        .route("/auth/login", post(session::login))
        .route("/auth/verify-email", post(session::verify_email))
        .route("/auth/forgot-password", post(session::forgot_password))
        .route("/auth/reset-password", post(session::reset_password))
        .route("/auth/refresh", post(session::refresh))
        .route("/auth/logout", post(session::logout))
        .route_layer(middleware::from_fn_with_state(
            state.auth_limiter.clone(),
            rate_limit::enforce,
        ))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users/me", get(profile::me))
        .route("/users/me/key", put(profile::publish_key))
}
