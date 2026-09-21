//! `greenbyte create` and `greenbyte init`.

use colored::Colorize;

use crate::{
    commands::context::{account_session, ensure_identity_published},
    core::{
        api::{client::server_url, projects},
        crypto::{generate_data_key, sealing},
        device,
        workspace::project_config::{self, ProjectConfig},
    },
    ui,
};

/// `greenbyte create <name>` - starts a new project in this directory.
///
/// The project data key is minted here and sealed to the creator's own
/// identity. Nothing is printed for the user to write down, because nothing
/// needs to be: colleagues get their own sealed copy when they are invited.
pub async fn create(project_name: String) -> Result<(), String> {
    validate_project_name(&project_name)?;
    if project_config::exists() {
        return Err(
            "A .greenbyte file already exists here. Use `greenbyte init` to link an existing \
             project."
                .to_string(),
        );
    }

    let session = account_session().await?;
    let identity = ensure_identity_published(&session).await?;

    let data_key = generate_data_key();
    let wrapped = sealing::seal(&data_key, &identity.public_key_base64())?;

    let bar = ui::spinner(&format!("Creating project '{project_name}'..."));
    let created = projects::create(&session, &project_name, &wrapped).await;
    bar.finish_and_clear();
    let created = created?;

    project_config::save(&ProjectConfig {
        project_name: Some(project_name.clone()),
        owner_username: None,
        project_id: Some(created.project_id),
        server_url: session.server.clone(),
        auth_token: None,
        refresh_token: None,
        soft_token: Some(created.soft_token),
        refresher_token: None,
        key_version: Some(created.key_version),
    })?;

    ui::success(&format!("Project '{}' created.", project_name.cyan()));
    ui::note(&format!(
        "The project key is sealed to your identity ({}). There is nothing to copy down.",
        identity.fingerprint()
    ));
    ui::note("Add a collaborator with `greenbyte add <email>` - they get their own sealed copy.");
    add_to_gitignore()?;
    Ok(())
}

/// `greenbyte init` - joins a project you were invited to.
///
/// The invitation identifies the project, so no name is needed and none can be
/// guessed. The project key arrives already sealed to this account.
pub async fn init(expected_name: Option<String>) -> Result<(), String> {
    if let Some(name) = &expected_name {
        validate_project_name(name)?;
    }
    if project_config::exists() {
        return Err("Already initialized. A .greenbyte file exists here.".to_string());
    }

    // Joining happens as the signed-in account: an invitation proves you were
    // invited, not who you are, so a leaked token cannot become a session.
    let session = account_session().await?;
    let identity = ensure_identity_published(&session).await?;

    let ott = ui::prompt("Enter your one-time token (from the invitation email): ")?;
    if ott.trim().is_empty() {
        return Err("A one-time token is required to join a project.".to_string());
    }
    let mac = device::get_device_mac();
    let refresher_token = device::make_refresher_token(&ott, mac.as_deref());

    let bar = ui::spinner("Redeeming invitation...");
    let joined = projects::join(&session, &ott, &refresher_token, mac.as_deref()).await;
    bar.finish_and_clear();
    let joined = joined?;

    if let Some(expected) = &expected_name {
        if !expected.eq_ignore_ascii_case(&joined.project_name) {
            ui::warn(&format!(
                "This invitation is for '{}/{}', not '{expected}'.",
                joined.owner_username, joined.project_name
            ));
        }
    }

    project_config::save(&ProjectConfig {
        project_name: Some(joined.project_name.clone()),
        owner_username: Some(joined.owner_username.clone()),
        project_id: Some(joined.project_id),
        server_url: session.server.clone(),
        auth_token: None,
        refresh_token: None,
        soft_token: Some(joined.soft_token),
        refresher_token: Some(refresher_token),
        key_version: Some(joined.key_version),
    })?;

    ui::success(&format!(
        "Joined {}.",
        format!("{}/{}", joined.owner_username, joined.project_name).cyan()
    ));
    ui::note(&format!(
        "The project key was sealed to your identity ({}).",
        identity.fingerprint()
    ));
    ui::note("Run `greenbyte pull` to get the current .env.");
    add_to_gitignore()?;
    Ok(())
}

/// Keeps Greenbyte's own files, and every `.env`, out of version control.
fn add_to_gitignore() -> Result<(), String> {
    let gitignore = std::path::Path::new(".gitignore");
    let mut existing = std::fs::read_to_string(gitignore).unwrap_or_default();
    let required = [
        ".greenbyte",
        ".greenbyte-logs",
        ".env",
        ".env.*",
        "!.env.example",
    ];
    let mut changed = false;
    for entry in required {
        if !existing.lines().any(|line| line.trim() == entry) {
            if !existing.is_empty() && !existing.ends_with('\n') {
                existing.push('\n');
            }
            existing.push_str(entry);
            existing.push('\n');
            changed = true;
        }
    }
    if changed {
        std::fs::write(gitignore, existing)
            .map_err(|e| format!("Could not update .gitignore: {e}"))?;
        ui::step("Added Greenbyte's files to .gitignore");
    }
    Ok(())
}

pub fn validate_project_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(
            "Project names must be 1-64 characters using letters, numbers, '-' or '_'.".to_string(),
        );
    }
    Ok(())
}

/// `greenbyte projects` - every project this account can reach.
///
/// Names are shown as `owner/project`, because two accounts may each own one
/// called `backend`.
pub async fn list() -> Result<(), String> {
    let session = match crate::core::workspace::project_config::load() {
        Ok(config) => {
            let token =
                crate::commands::context::access_token(&config.server_url, config.auth_token)
                    .await?;
            crate::core::api::Session::new(&config.server_url, token)?
        }
        Err(_) => {
            let token = crate::commands::context::access_token(&server_url(), None).await?;
            crate::core::api::Session::new(&server_url(), token)?
        }
    };
    let projects = projects::list(&session).await?;
    if projects.is_empty() {
        ui::note("No projects yet. Run `greenbyte create <name>`.");
        return Ok(());
    }
    ui::heading("Projects");
    for project in projects {
        println!(
            "  {:<32} {:<8} key v{}",
            format!("{}/{}", project.owner_username, project.name).cyan(),
            project.role,
            project.key_version
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_reasonable_project_names() {
        for name in ["api", "api-production", "api_2", "A1"] {
            assert!(validate_project_name(name).is_ok(), "rejected {name}");
        }
    }

    #[test]
    fn rejects_unusable_project_names() {
        for name in ["", "has space", "slash/name", "dot.name", &"a".repeat(65)] {
            assert!(validate_project_name(name).is_err(), "accepted {name}");
        }
    }
}
