use std::sync::Arc;

use sqlx::PgPool;

use crate::{config::Config, email::EmailSender, rate_limit::RateLimiter};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub email: Arc<dyn EmailSender>,
    pub general_limiter: Arc<RateLimiter>,
    pub auth_limiter: Arc<RateLimiter>,
}
