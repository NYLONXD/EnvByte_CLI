//! Router assembly.
//!
//! Every feature contributes its own `routes()`, so the URL surface of the API
//! is readable in one place without any feature's internals leaking here.

use std::{sync::Arc, time::Duration};

use axum::{
    http::{
        header::{AUTHORIZATION, COOKIE},
        HeaderName, StatusCode,
    },
    middleware, Router,
};
use tower_http::{
    catch_panic::CatchPanicLayer,
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::{
    config,
    features::{accounts, audit, env_files, health, keys, members, projects},
    infra::{email, rate_limit::RateLimiter},
    state::AppState,
};

pub fn app(state: AppState) -> Router {
    Router::new()
        .merge(accounts::credential_routes(state.clone()))
        .merge(health::routes())
        .merge(accounts::routes())
        .merge(projects::routes())
        .merge(members::routes())
        .merge(keys::routes())
        .merge(env_files::routes())
        .merge(audit::routes())
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(
            state.general_limiter,
            crate::infra::rate_limit::enforce,
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
