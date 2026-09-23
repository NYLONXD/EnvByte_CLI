//! Registration, sign-in, email verification and password recovery.

use axum::{extract::State, Json};
use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    db::models::User,
    error::ApiError,
    features::accounts::{
        dto::{
            AuthResponse, ForgotPasswordRequest, LoginRequest, RefreshRequest,
            ResendVerificationRequest, ResetPasswordRequest, SignupRequest, SignupResponse,
            VerifyEmailRequest,
        },
        validation::{normalize_email, validate_password, validate_username},
    },
    security::{
        access_token, hash_password, hash_token, issue_refresh_token, random_token,
        rotate_refresh_token, verify_password,
    },
    state::AppState,
};

pub async fn signup(
    State(state): State<AppState>,
    Json(request): Json<SignupRequest>,
) -> Result<Json<SignupResponse>, ApiError> {
    validate_username(&request.username)?;
    let email = normalize_email(&request.email)?;
    validate_password(&request.password)?;
    let password = Zeroizing::new(request.password);
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|error| {
            tracing::error!(%error, "password hashing worker failed");
            ApiError::Internal
        })??;
    let user = User {
        id: Uuid::new_v4(),
        username: request.username,
        email,
        password_hash,
        public_key: None,
    };
    let verification = Zeroizing::new(random_token(32));
    let mut transaction = state.pool.begin().await?;
    // An account whose verification lapsed never proved it owns its address,
    // so it must not hold that address or username forever.
    sqlx::query(
        "DELETE FROM users u
         WHERE u.email_verified_at IS NULL
           AND (lower(u.email) = lower($1) OR lower(u.username) = lower($2))
           AND NOT EXISTS (
               SELECT 1 FROM email_verifications v
               WHERE v.user_id = u.id AND v.consumed_at IS NULL AND v.expires_at > now()
           )",
    )
    .bind(&user.email)
    .bind(&user.username)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, $4)")
        .bind(user.id)
        .bind(&user.username)
        .bind(&user.email)
        .bind(&user.password_hash)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO email_verifications (id, user_id, token_hash, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(hash_token(&verification))
    .bind(chrono::Utc::now() + chrono::Duration::hours(state.config.email_verification_ttl_hours))
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    if let Err(error) = state
        .email
        .send_verification(&user.email, &verification)
        .await
    {
        tracing::error!(%error, user_id = %user.id, "email verification delivery failed");
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user.id)
            .execute(&state.pool)
            .await;
        return Err(ApiError::Internal);
    }
    Ok(Json(SignupResponse {
        verification_required: true,
        email: user.email,
    }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let email = normalize_email(&request.email)?;
    let password = Zeroizing::new(request.password);
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, public_key FROM users
         WHERE lower(email) = lower($1) AND email_verified_at IS NOT NULL",
    )
    .bind(email)
    .fetch_optional(&state.pool)
    .await?;
    // An unknown address still costs a full Argon2 verification, so response
    // time does not reveal which addresses are registered.
    let encoded = user
        .as_ref()
        .map(|user| user.password_hash.clone())
        .unwrap_or_else(dummy_password_hash);
    let verified = tokio::task::spawn_blocking(move || verify_password(&password, &encoded))
        .await
        .map_err(|error| {
            tracing::error!(%error, "password verification worker failed");
            ApiError::Internal
        })?;
    let Some(user) = user else {
        return Err(ApiError::Unauthorized);
    };
    if !verified {
        return Err(ApiError::Unauthorized);
    }
    build_auth_response(&state.pool, &state, user).await
}

pub async fn verify_email(
    State(state): State<AppState>,
    Json(request): Json<VerifyEmailRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let email = normalize_email(&request.email)?;
    let token = Zeroizing::new(request.token);
    let mut transaction = state.pool.begin().await?;
    let user = sqlx::query_as::<_, User>(
        "SELECT u.id, u.username, u.email, u.password_hash, u.public_key
         FROM email_verifications v JOIN users u ON u.id = v.user_id
         WHERE lower(u.email) = lower($1) AND v.token_hash = $2
           AND v.consumed_at IS NULL AND v.expires_at > now()
         FOR UPDATE OF v",
    )
    .bind(email)
    .bind(hash_token(&token))
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    sqlx::query(
        "UPDATE email_verifications SET consumed_at = now()
         WHERE user_id = $1 AND consumed_at IS NULL",
    )
    .bind(user.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("UPDATE users SET email_verified_at = now(), updated_at = now() WHERE id = $1")
        .bind(user.id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    build_auth_response(&state.pool, &state, user).await
}

/// Minimum gap between verification emails to one account.
const RESEND_COOLDOWN_SECONDS: f64 = 60.0;

pub async fn resend_verification(
    State(state): State<AppState>,
    Json(request): Json<ResendVerificationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let email = normalize_email(&request.email)?;
    let password = Zeroizing::new(request.password);
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, public_key FROM users
         WHERE lower(email) = lower($1) AND email_verified_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?;
    let encoded = user
        .as_ref()
        .map(|user| user.password_hash.clone())
        .unwrap_or_else(dummy_password_hash);
    let verified = tokio::task::spawn_blocking(move || verify_password(&password, &encoded))
        .await
        .map_err(|error| {
            tracing::error!(%error, "password verification worker failed");
            ApiError::Internal
        })?;
    if let (Some(user), true) = (user, verified) {
        let recently_sent = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                 SELECT 1 FROM email_verifications
                 WHERE user_id = $1 AND created_at > now() - make_interval(secs => $2)
             )",
        )
        .bind(user.id)
        .bind(RESEND_COOLDOWN_SECONDS)
        .fetch_one(&state.pool)
        .await?;
        if !recently_sent {
            let verification = Zeroizing::new(random_token(32));
            let mut transaction = state.pool.begin().await?;
            sqlx::query(
                "DELETE FROM email_verifications WHERE user_id = $1 AND consumed_at IS NULL",
            )
            .bind(user.id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "INSERT INTO email_verifications (id, user_id, token_hash, expires_at)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(Uuid::new_v4())
            .bind(user.id)
            .bind(hash_token(&verification))
            .bind(
                chrono::Utc::now()
                    + chrono::Duration::hours(state.config.email_verification_ttl_hours),
            )
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            if let Err(error) = state
                .email
                .send_verification(&user.email, &verification)
                .await
            {
                tracing::error!(%error, user_id = %user.id, "verification resend failed");
            }
        }
    }
    // Always the same answer: it must not reveal which addresses are
    // registered, verified, or paired with the given password.
    Ok(Json(serde_json::json!({
        "message": "If that address and password match an unverified account, a new token has been sent."
    })))
}

pub async fn forgot_password(
    State(state): State<AppState>,
    Json(request): Json<ForgotPasswordRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let email = normalize_email(&request.email)?;
    let user_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM users WHERE lower(email) = lower($1) AND email_verified_at IS NOT NULL",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?;
    if let Some(user_id) = user_id {
        let token = Zeroizing::new(random_token(32));
        sqlx::query(
            "INSERT INTO password_resets (id, user_id, token_hash, expires_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(hash_token(&token))
        .bind(
            chrono::Utc::now() + chrono::Duration::minutes(state.config.password_reset_ttl_minutes),
        )
        .execute(&state.pool)
        .await?;
        if let Err(error) = state.email.send_password_reset(&email, &token).await {
            tracing::error!(%error, user_id = %user_id, "password reset delivery failed");
        }
    }
    // Always the same answer, so this endpoint cannot be used to enumerate
    // registered addresses.
    Ok(Json(serde_json::json!({
        "message": "If that verified account exists, a reset token has been sent."
    })))
}

pub async fn reset_password(
    State(state): State<AppState>,
    Json(request): Json<ResetPasswordRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let email = normalize_email(&request.email)?;
    validate_password(&request.password)?;
    let token = Zeroizing::new(request.token);
    let password = Zeroizing::new(request.password);
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|error| {
            tracing::error!(%error, "password reset hashing worker failed");
            ApiError::Internal
        })??;
    let mut transaction = state.pool.begin().await?;
    let user_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT u.id FROM password_resets r JOIN users u ON u.id = r.user_id
         WHERE lower(u.email) = lower($1) AND r.token_hash = $2
           AND r.consumed_at IS NULL AND r.expires_at > now()
         FOR UPDATE OF r",
    )
    .bind(email)
    .bind(hash_token(&token))
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    sqlx::query("UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2")
        .bind(password_hash)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "UPDATE password_resets SET consumed_at = now() WHERE user_id = $1 AND consumed_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE refresh_sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let token = Zeroizing::new(request.refresh_token);
    let mut transaction = state.pool.begin().await?;
    let (user, replacement) = rotate_refresh_token(&mut transaction, &token, &state.config).await?;
    transaction.commit().await?;
    Ok(Json(AuthResponse {
        token: access_token(user.id, &state.config)?,
        refresh_token: replacement.to_string(),
        user_id: user.id,
        username: user.username,
        email: user.email,
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = Zeroizing::new(request.refresh_token);
    sqlx::query("UPDATE refresh_sessions SET revoked_at = now() WHERE token_hash = $1 AND revoked_at IS NULL")
        .bind(hash_token(&token))
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn build_auth_response(
    pool: &PgPool,
    state: &AppState,
    user: User,
) -> Result<Json<AuthResponse>, ApiError> {
    let refresh = issue_refresh_token(pool, user.id, &state.config).await?;
    Ok(Json(AuthResponse {
        token: access_token(user.id, &state.config)?,
        refresh_token: refresh.to_string(),
        user_id: user.id,
        username: user.username,
        email: user.email,
    }))
}

fn dummy_password_hash() -> String {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DUMMY
        .get_or_init(|| hash_password("not-a-real-password-value").expect("static password hashes"))
        .clone()
}
