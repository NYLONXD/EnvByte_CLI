pub mod config;
pub mod email;
pub mod error;
pub mod models;
pub mod permissions;
pub mod rate_limit;
pub mod routes;
pub mod security;
pub mod state;

use std::{sync::Arc, time::Duration};

use axum::{
    http::{
        header::{AUTHORIZATION, COOKIE},
        HeaderName, StatusCode,
    },
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::{
    catch_panic::CatchPanicLayer,
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::{rate_limit::RateLimiter, state::AppState};

pub fn app(state: AppState) -> Router {
    let auth_routes = Router::new()
        .route("/auth/signup", post(routes::auth::signup))
        .route("/auth/login", post(routes::auth::login))
        .route("/auth/verify-email", post(routes::auth::verify_email))
        .route("/auth/forgot-password", post(routes::auth::forgot_password))
        .route("/auth/reset-password", post(routes::auth::reset_password))
        .route("/auth/refresh", post(routes::auth::refresh))
        .route("/auth/logout", post(routes::auth::logout))
        .route_layer(middleware::from_fn_with_state(
            state.auth_limiter.clone(),
            rate_limit::enforce,
        ));

    Router::new()
        .merge(auth_routes)
        .route("/health/live", get(routes::health::live))
        .route("/health/ready", get(routes::health::ready))
        .route("/users/me", get(routes::users::me))
        .route(
            "/users/me/env",
            post(routes::env_files::push).get(routes::env_files::pull),
        )
        .route(
            "/projects",
            post(routes::projects::create).get(routes::projects::list),
        )
        .route(
            "/projects/{project_name}/join",
            post(routes::projects::join),
        )
        .route(
            "/projects/{project_id}/collaborators",
            post(routes::projects::invite).get(routes::projects::members),
        )
        .route(
            "/projects/{project_id}/collaborators/{user_id}",
            axum::routing::delete(routes::projects::remove_member)
                .patch(routes::projects::change_role),
        )
        .route(
            "/projects/{project_id}/rollback",
            post(routes::projects::rollback),
        )
        .route(
            "/projects/{project_id}/commits",
            get(routes::env_files::history),
        )
        .route("/projects/{project_id}/audit", get(routes::projects::audit))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(
            state.general_limiter,
            rate_limit::enforce,
        ))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(RequestBodyLimitLayer::new(20 * 1024 * 1024))
        .layer(SetSensitiveRequestHeadersLayer::new([
            AUTHORIZATION,
            COOKIE,
        ]))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::new(HeaderName::from_static(
            "x-request-id",
        )))
        .layer(SetRequestIdLayer::new(
            HeaderName::from_static("x-request-id"),
            MakeRequestUuid,
        ))
        .layer(CatchPanicLayer::new())
}

pub fn build_state(pool: sqlx::PgPool, config: config::Config) -> Result<AppState, String> {
    let email = email::build_sender(&config.email)?;
    let general_limiter = Arc::new(RateLimiter::per_minute(config.rate_limit_per_minute));
    let auth_limiter = Arc::new(RateLimiter::per_minute(config.auth_rate_limit_per_minute));
    Ok(AppState {
        pool,
        config: Arc::new(config),
        email,
        general_limiter,
        auth_limiter,
    })
}
