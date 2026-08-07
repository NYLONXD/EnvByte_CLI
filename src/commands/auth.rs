use crate::utils::{
    config::{http_client, response_error, server_url, validate_server_url},
    local_store::{clear_global_auth, load_config, load_global_auth, save_global_auth, GlobalAuth},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Serialize)]
struct RegisterRequest<'a> {
    username: &'a str,
    email: &'a str,
    password: &'a str,
}

#[derive(Serialize)]
struct LoginRequest<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct AuthResponse {
    token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    user_id: String,
    #[allow(dead_code)]
    username: String,
    #[allow(dead_code)]
    email: String,
}

/// `greenbyte register` — create a new account
pub async fn register() -> Result<(), String> {
    println!("{}", "Creating your Greenbyte account".bold());
    println!();

    let email = prompt("Email: ")?;
    let user_id = prompt("Username (no spaces): ")?;
    let password = Zeroizing::new(prompt_password("Password: ")?);
    let password_confirm = Zeroizing::new(prompt_password("Confirm password: ")?);

    validate_email(&email)?;
    if user_id.is_empty() || user_id.chars().any(char::is_whitespace) {
        return Err("Username must be non-empty and contain no spaces.".to_string());
    }
    if password.len() < 12 {
        return Err("Password must contain at least 12 characters.".to_string());
    }

    if password != password_confirm {
        return Err("Passwords do not match.".to_string());
    }

    let spinner = start_spinner("Registering...");

    let server = server_url();
    validate_server_url(&server)?;
    let client = http_client()?;
    let res = client
        .post(format!("{server}/auth/signup"))
        .json(&RegisterRequest {
            username: &user_id,
            email: &email,
            password: &password,
        })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        return Err(response_error(res, "Registration").await);
    }

    let signup: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Response parse error: {}", e))?;
    let data: AuthResponse = if signup["verification_required"].as_bool() == Some(true) {
        println!(
            "{} A verification token was sent to {}.",
            "●".green(),
            email.cyan()
        );
        let verification = Zeroizing::new(prompt("Verification token: ")?);
        let spinner = start_spinner("Verifying email...");
        let response = client
            .post(format!("{server}/auth/verify-email"))
            .json(&serde_json::json!({ "email": email, "token": &*verification }))
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;
        spinner.finish_and_clear();
        if !response.status().is_success() {
            return Err(response_error(response, "Email verification").await);
        }
        response
            .json()
            .await
            .map_err(|e| format!("Verification response parse error: {e}"))?
    } else {
        serde_json::from_value(signup)
            .map_err(|e| format!("Registration response parse error: {e}"))?
    };

    save_global_auth(&GlobalAuth {
        email: Some(email.clone()),
        auth_token: Some(data.token),
        user_id: Some(data.user_id),
        refresh_token: data.refresh_token,
    })?;

    println!(
        "{} Account created for {}",
        "✓".green().bold(),
        email.cyan()
    );
    println!("  You're now logged in.");
    Ok(())
}

/// `greenbyte login` — login to existing account
pub async fn login() -> Result<(), String> {
    println!("{}", "Login to Greenbyte".bold());
    println!();

    let email = prompt("Email: ")?;
    let password = Zeroizing::new(prompt_password("Password: ")?);
    validate_email(&email)?;

    let spinner = start_spinner("Authenticating...");

    let server = server_url();
    validate_server_url(&server)?;
    let client = http_client()?;
    let res = client
        .post(format!("{server}/auth/login"))
        .json(&LoginRequest {
            email: &email,
            password: &password,
        })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    let status = res.status();

    if !status.is_success() {
        return Err(response_error(res, "Login").await);
    }

    let data: AuthResponse = res
        .json()
        .await
        .map_err(|e| format!("Response parse error: {}", e))?;

    save_global_auth(&GlobalAuth {
        email: Some(email.clone()),
        auth_token: Some(data.token),
        user_id: Some(data.user_id),
        refresh_token: data.refresh_token,
    })?;

    println!("{} Logged in as {}", "✓".green().bold(), email.cyan());

    Ok(())
}

/// `greenbyte logout` — remove the local login token.
pub async fn logout() -> Result<(), String> {
    let auth = load_global_auth()?;
    if let Some(refresh_token) = auth.refresh_token {
        let server = load_config()
            .ok()
            .map(|config| config.server_url)
            .unwrap_or_else(server_url);
        if let Ok(client) = http_client() {
            if let Err(error) = client
                .post(format!("{}/auth/logout", server.trim_end_matches('/')))
                .json(&serde_json::json!({ "refresh_token": refresh_token }))
                .send()
                .await
            {
                eprintln!(
                    "{} Could not revoke the remote session: {error}",
                    "⚠".yellow()
                );
            }
        }
    }
    clear_global_auth()?;
    println!(
        "{} Logged out. Project files and encrypted snapshots were kept.",
        "✓".green().bold()
    );
    Ok(())
}

pub async fn reset_password() -> Result<(), String> {
    let email = prompt("Email: ")?;
    validate_email(&email)?;
    let server = server_url();
    validate_server_url(&server)?;
    let client = http_client()?;
    let response = client
        .post(format!("{server}/auth/forgot-password"))
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await
        .map_err(|e| format!("Could not request password reset: {e}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Password reset request").await);
    }
    println!("If the account exists, a reset token has been sent.");
    let token = Zeroizing::new(prompt("Reset token: ")?);
    let password = Zeroizing::new(prompt_password("New password: ")?);
    let confirmation = Zeroizing::new(prompt_password("Confirm new password: ")?);
    if password.len() < 12 {
        return Err("Password must contain at least 12 characters.".to_string());
    }
    if password != confirmation {
        return Err("Passwords do not match.".to_string());
    }
    let response = client
        .post(format!("{server}/auth/reset-password"))
        .json(&serde_json::json!({
            "email": email,
            "token": &*token,
            "password": &*password,
        }))
        .send()
        .await
        .map_err(|e| format!("Could not reset password: {e}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Password reset").await);
    }
    clear_global_auth()?;
    println!(
        "{} Password changed. Run `greenbyte login`.",
        "✓".green().bold()
    );
    Ok(())
}

pub async fn access_token(server: &str, fallback: Option<String>) -> Result<String, String> {
    let mut auth = load_global_auth()?;
    if let Some(token) = auth.auth_token.as_ref() {
        if !token_expires_soon(token) {
            return Ok(token.clone());
        }
    }
    if let Some(refresh_token) = auth.refresh_token.clone() {
        let client = http_client()?;
        let response = client
            .post(format!("{}/auth/refresh", server.trim_end_matches('/')))
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(|e| format!("Could not refresh login session: {e}"))?;
        if !response.status().is_success() {
            return Err(response_error(response, "Session refresh").await);
        }
        let refreshed: AuthResponse = response
            .json()
            .await
            .map_err(|e| format!("Invalid session refresh response: {e}"))?;
        auth.auth_token = Some(refreshed.token.clone());
        auth.refresh_token = refreshed.refresh_token;
        auth.email = Some(refreshed.email);
        auth.user_id = Some(refreshed.user_id);
        save_global_auth(&auth)?;
        return Ok(refreshed.token);
    }
    auth.auth_token
        .or(fallback)
        .ok_or_else(|| "Not logged in. Run `greenbyte login` first.".to_string())
}

fn token_expires_soon(token: &str) -> bool {
    let Some(payload) = token.split('.').nth(1) else {
        return true;
    };
    let Ok(decoded) = URL_SAFE_NO_PAD.decode(payload) else {
        return true;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&decoded) else {
        return true;
    };
    value["exp"]
        .as_i64()
        .is_none_or(|expires| expires <= chrono::Utc::now().timestamp() + 120)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn prompt(label: &str) -> Result<String, String> {
    use std::io::{self, Write};
    print!("{}", label);
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| e.to_string())?;
    Ok(input.trim().to_string())
}

fn prompt_password(label: &str) -> Result<String, String> {
    rpassword::prompt_password(label).map_err(|e| e.to_string())
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

fn start_spinner(msg: &str) -> indicatif::ProgressBar {
    use indicatif::{ProgressBar, ProgressStyle};
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}
