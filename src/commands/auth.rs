use colored::Colorize;
use serde::{Deserialize, Serialize};
use crate::utils::{config::server_url, local_store::{save_global_auth, GlobalAuth}};

#[derive(Serialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

#[derive(Serialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct AuthResponse {
    token: String,
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
    let password = prompt_password("Password: ")?;
    let password_confirm = prompt_password("Confirm password: ")?;

    if password != password_confirm {
        return Err("Passwords do not match.".to_string());
    }

    let spinner = start_spinner("Registering...");

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/auth/signup", server_url()))
        .json(&RegisterRequest { username: user_id.clone(), email: email.clone(), password })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();

        return Err(format!(
            "Registration failed ({}): {}",
            status,
            body
        ));
    }

    let data: AuthResponse = res.json().await
        .map_err(|e| format!("Response parse error: {}", e))?;

    save_global_auth(&GlobalAuth {
        email: Some(email.clone()),
        auth_token: Some(data.token),
        user_id: Some(data.user_id),
    })?;

    println!("{} Account created for {}", "✓".green().bold(), email.cyan());
    println!("  You're now logged in.");
    Ok(())
}

/// `greenbyte login` — login to existing account
pub async fn login() -> Result<(), String> {
    println!("{}", "Login to Greenbyte".bold());
    println!();

    let email = prompt("Email: ")?;
    let password = prompt_password("Password: ")?;

    let spinner = start_spinner("Authenticating...");

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/auth/login", server_url()))
        .json(&LoginRequest {
            email: email.clone(),
            password,
        })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    let status = res.status();

    if !status.is_success() {
        let body = res.text().await.unwrap_or_else(|_| "Unable to read response body".to_string());

        return Err(format!(
            "Login failed ({}): {}",
            status,
            body
        ));
    }

    let data: AuthResponse = res
        .json()
        .await
        .map_err(|e| format!("Response parse error: {}", e))?;

    save_global_auth(&GlobalAuth {
        email: Some(email.clone()),
        auth_token: Some(data.token),
        user_id: Some(data.user_id),
    })?;

    println!(
        "{} Logged in as {}",
        "✓".green().bold(),
        email.cyan()
    );

    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn prompt(label: &str) -> Result<String, String> {
    use std::io::{self, Write};
    print!("{}", label);
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| e.to_string())?;
    Ok(input.trim().to_string())
}

fn prompt_password(label: &str) -> Result<String, String> {
    rpassword::prompt_password(label).map_err(|e| e.to_string())
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