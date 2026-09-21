//! `greenbyte status` - what this directory is linked to and whether it works.

use colored::Colorize;

use crate::{
    commands::context::access_token,
    core::{
        api::{accounts, client::server_url, keys, projects, Session},
        crypto::{identity, keyring::Keyring},
        env_files::env_file_path,
        workspace::{commit_log, global_auth, project_config},
    },
    ui,
};

pub async fn show() -> Result<(), String> {
    ui::heading("Status");

    let config = project_config::exists()
        .then(project_config::load)
        .transpose()?;
    let server = config
        .as_ref()
        .map(|config| config.server_url.clone())
        .unwrap_or_else(server_url);

    let session = report_account(&server).await;
    report_identity();

    let Some(config) = config else {
        println!();
        ui::warn("No project linked in this directory.");
        ui::note("Run `greenbyte create <name>` or `greenbyte init`.");
        return Ok(());
    };

    println!();
    ui::field("Project", &config.qualified_name());
    if let Some(id) = &config.project_id {
        ui::field("ID", id);
    }
    ui::field("Server", &config.server_url);

    if let (Some(session), Some(project_id)) = (&session, config.project_id.as_deref()) {
        report_project_key(session, project_id).await;
    }
    report_env_file();
    report_snapshots();
    Ok(())
}

/// Confirms the stored session still works, rather than just that a token file
/// exists.
async fn report_account(server: &str) -> Option<Session> {
    let stored = global_auth::load().unwrap_or_default();
    let Ok(token) = access_token(server, None).await else {
        ui::field(
            "Account",
            &"not signed in (run `greenbyte login`)".red().to_string(),
        );
        return None;
    };
    let Ok(session) = Session::new(server, token) else {
        ui::field("Account", &"unusable server URL".red().to_string());
        return None;
    };
    match accounts::me(&session).await {
        Ok(profile) => {
            ui::field(
                "Account",
                &format!("{} <{}>", profile.username.cyan(), profile.email),
            );
            if profile.public_key.is_none() {
                ui::warn("No identity key published. Run `greenbyte identity publish`.");
            }
            Some(session)
        }
        Err(error) => {
            ui::field(
                "Account",
                &stored.email.unwrap_or_else(|| "unknown".to_string()),
            );
            ui::warn(&format!("Could not reach the server: {error}"));
            None
        }
    }
}

fn report_identity() {
    match identity::load_or_create() {
        Ok((identity, created)) => {
            ui::field("Identity", &identity.fingerprint());
            if created {
                ui::note("Created just now; publish it with `greenbyte identity publish`.");
            }
        }
        Err(error) => ui::warn(&format!("Identity key unavailable: {error}")),
    }
}

/// Reports which project key versions this device can actually open - the
/// question that matters after a rotation.
async fn report_project_key(session: &Session, project_id: &str) {
    let held = match keys::grants(session, project_id).await {
        Ok(grants) => grants,
        Err(error) => {
            ui::field("Project key", &"unavailable".yellow().to_string());
            ui::note(&error);
            return;
        }
    };
    let Ok((identity, _)) = identity::load_or_create() else {
        ui::field("Project key", &"identity unavailable".yellow().to_string());
        return;
    };
    // Report what actually opens, not merely what was handed over.
    let keyring = match Keyring::open(held, &identity) {
        Ok(keyring) => keyring,
        Err(error) => {
            ui::field("Project key", &"unreadable".red().to_string());
            ui::note(&error);
            ui::note("If you replaced your identity key, ask an admin to re-grant access.");
            return;
        }
    };
    let versions: Vec<String> = keyring.versions().iter().map(i32::to_string).collect();
    let newest = keyring.current_version();

    match projects::current_key_version(session, project_id).await {
        Ok(current) if current == newest => {
            ui::field("Project key", &format!("v{current} (current)"));
        }
        Ok(current) => {
            ui::field(
                "Project key",
                &format!("v{newest} - project is on v{current}")
                    .yellow()
                    .to_string(),
            );
            ui::note("Ask an admin to grant you the current key.");
        }
        Err(_) => ui::field("Project key", &format!("v{newest}")),
    }
    if versions.len() > 1 {
        ui::note(&format!(
            "Also holds versions {} for reading history.",
            versions.join(", ")
        ));
    }
}

fn report_env_file() {
    let path = env_file_path();
    if path.exists() {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        ui::field(".env", &format!("present ({size} bytes)"));
    } else {
        ui::field(
            ".env",
            &"not found (run `greenbyte pull`)".yellow().to_string(),
        );
    }
}

fn report_snapshots() {
    let store = commit_log::load().unwrap_or_default();
    match store.commits.last() {
        None => ui::field("Snapshots", "none yet"),
        Some(latest) => ui::field(
            "Snapshots",
            &format!(
                "{} total, latest [{}] \"{}\"",
                store.commits.len(),
                &latest.id[..8],
                latest.message
            ),
        ),
    }
}
