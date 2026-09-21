//! `greenbyte register`, `login`, `logout` and `reset-password`.

use colored::Colorize;
use zeroize::Zeroizing;

use crate::{
    commands::context::{access_token, ensure_identity_published},
    core::{
        api::{
            accounts::{self, LoginRequest, SignupRequest},
            client::server_url,
            Session,
        },
        workspace::{global_auth, project_config},
    },
    ui,
};

pub async fn register() -> Result<(), String> {
    println!("{}", "Create your Greenbyte account".bold());
    println!();

    let email = ui::prompt("Email: ")?;
    let username = ui::prompt("Username (no spaces): ")?;
    let password = Zeroizing::new(ui::prompt_password("Password: ")?);
    let confirmation = Zeroizing::new(ui::prompt_password("Confirm password: ")?);

    validate_email(&email)?;
    validate_username(&username)?;
    validate_password(&password)?;
    if password != confirmation {
        return Err("Passwords do not match.".to_string());
    }

    let server = server_url();
    let bar = ui::spinner("Registering...");
    let signup = accounts::signup(
        &server,
        SignupRequest {
            username: &username,
            email: &email,
            password: &password,
        },
    )
    .await;
    bar.finish_and_clear();
    let signup = signup?;

    if !signup.verification_required {
        return Err(
            "The server did not ask for email verification, which this CLI requires.".to_string(),
        );
    }
    ui::step(&format!(
        "A verification token was sent to {}",
        email.cyan()
    ));
    let token = Zeroizing::new(ui::prompt("Verification token: ")?);

    let bar = ui::spinner("Verifying...");
    let auth = accounts::verify_email(&server, &email, &token).await;
    bar.finish_and_clear();
    let auth = auth?;

    store_session(&email, &auth)?;
    let session = Session::new(&server, auth.token)?;
    let identity = ensure_identity_published(&session).await?;

    ui::success(&format!("Account created for {}.", email.cyan()));
    ui::field("Identity", &identity.fingerprint());
    ui::note("You are signed in. Colleagues can now seal project keys to you.");
    Ok(())
}

pub async fn login() -> Result<(), String> {
    println!("{}", "Sign in to Greenbyte".bold());
    println!();

    let email = ui::prompt("Email: ")?;
    let password = Zeroizing::new(ui::prompt_password("Password: ")?);
    validate_email(&email)?;

    let server = server_url();
    let bar = ui::spinner("Authenticating...");
    let auth = accounts::login(
        &server,
        LoginRequest {
            email: &email,
            password: &password,
        },
    )
    .await;
    bar.finish_and_clear();
    let auth = auth?;

    store_session(&email, &auth)?;
    let session = Session::new(&server, auth.token)?;
    // Publishing on every sign-in means a new machine becomes invitable at
    // once, instead of failing later with a confusing "no identity key".
    let identity = ensure_identity_published(&session).await?;

    ui::success(&format!("Signed in as {}.", email.cyan()));
    ui::field("Identity", &identity.fingerprint());
    Ok(())
}

pub async fn logout() -> Result<(), String> {
    let auth = global_auth::load()?;
    if let Some(refresh_token) = auth.refresh_token {
        let server = project_config::load()
            .map(|config| config.server_url)
            .unwrap_or_else(|_| server_url());
        if let Err(error) = accounts::logout(&server, &refresh_token).await {
            ui::warn(&format!("Could not revoke the remote session: {error}"));
        }
    }
    global_auth::clear()?;
    ui::success("Signed out.");
    ui::note("Your identity key and encrypted snapshots were kept.");
    Ok(())
}

pub async fn reset_password() -> Result<(), String> {
    let email = ui::prompt("Email: ")?;
    validate_email(&email)?;
    let server = server_url();

    accounts::forgot_password(&server, &email).await?;
    println!("If that account exists, a reset token has been sent.");

    let token = Zeroizing::new(ui::prompt("Reset token: ")?);
    let password = Zeroizing::new(ui::prompt_password("New password: ")?);
    let confirmation = Zeroizing::new(ui::prompt_password("Confirm new password: ")?);
    validate_password(&password)?;
    if password != confirmation {
        return Err("Passwords do not match.".to_string());
    }

    accounts::reset_password(&server, &email, &token, &password).await?;
    global_auth::clear()?;

    ui::success("Password changed.");
    ui::note("Run `greenbyte login`. Your identity key and project access are unaffected.");
    Ok(())
}

/// Shows who the local session belongs to, without contacting the server.
pub async fn whoami() -> Result<(), String> {
    let server = project_config::load()
        .map(|config| config.server_url)
        .unwrap_or_else(|_| server_url());
    let token = access_token(&server, None).await?;
    let session = Session::new(&server, token)?;
    let profile = accounts::me(&session).await?;
    ui::field("Username", &profile.username);
    ui::field("Email", &profile.email);
    match profile.public_key.as_deref() {
        Some(key) => ui::field(
            "Identity",
            &crate::core::crypto::identity::fingerprint_of(key)?,
        ),
        None => ui::warn("No identity key published. Run `greenbyte login` to publish one."),
    }
    Ok(())
}

fn store_session(email: &str, auth: &accounts::AuthResponse) -> Result<(), String> {
    let mut stored = global_auth::load().unwrap_or_default();
    stored.email = Some(email.to_string());
    stored.auth_token = Some(auth.token.clone());
    stored.user_id = Some(auth.user_id.clone());
    stored.refresh_token = auth.refresh_token.clone();
    global_auth::save(&stored)
}

fn validate_email(email: &str) -> Result<(), String> {
    let (local, domain) = email
        .split_once('@')
        .ok_or("Enter a valid email address.")?;
    if local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || email.chars().any(char::is_whitespace)
    {
        return Err("Enter a valid email address.".to_string());
    }
    Ok(())
}

fn validate_username(username: &str) -> Result<(), String> {
    if username.len() < 2
        || username.len() > 64
        || !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(
            "Username must be 2-64 characters using letters, numbers, '-' or '_'.".to_string(),
        );
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), String> {
    if password.len() < 12 {
        return Err("Password must contain at least 12 characters.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_account_input() {
        assert!(validate_email("dev@example.com").is_ok());
        assert!(validate_email("nope").is_err());
        assert!(validate_username("dev_2").is_ok());
        assert!(validate_username("has space").is_err());
        assert!(validate_username("a").is_err());
        assert!(validate_password("twelve-chars").is_ok());
        assert!(validate_password("short").is_err());
    }
}
