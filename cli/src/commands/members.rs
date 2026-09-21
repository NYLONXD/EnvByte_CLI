//! `greenbyte add`, `members`, `remove` and `role`.

use colored::Colorize;

use crate::{
    commands::context::ProjectContext,
    core::{
        api::members,
        crypto::{identity::fingerprint_of, sealing},
    },
    ui,
};

/// `greenbyte add <email>` - invites a collaborator and seals the project key
/// to them in the same step.
///
/// This is the whole point of key wrapping: the colleague never has to be sent
/// a secret over chat, and the server never learns one either.
pub async fn add(email: String) -> Result<(), String> {
    validate_email(&email)?;
    let context = ProjectContext::load().await?;
    let keyring = context.keyring().await?;
    let (data_key, key_version) = keyring.current()?;

    let bar = ui::spinner("Looking up their identity key...");
    let invitee = members::lookup(&context.session, &context.project_id, &email).await;
    bar.finish_and_clear();
    let invitee = invitee?;

    // Show the fingerprint so it can be confirmed out of band if the project
    // warrants that level of care.
    ui::step(&format!(
        "{} identity key {}",
        invitee.username.cyan(),
        fingerprint_of(&invitee.public_key)?.dimmed()
    ));

    let wrapped = sealing::seal(data_key, &invitee.public_key)?;
    let bar = ui::spinner("Sending the invitation...");
    let result = members::invite(&context.session, &context.project_id, &email, &wrapped).await;
    bar.finish_and_clear();
    let result = result?;

    ui::success(&format!("Invited {}.", email.cyan()));
    ui::note(&format!(
        "Project key v{key_version} is sealed to their identity - nothing to share by hand."
    ));
    ui::note("They run `greenbyte init` and paste the token from their email.");
    ui::note(&format!(
        "The token expires in {} hours.",
        result.expires_in_hours
    ));
    Ok(())
}

pub async fn list() -> Result<(), String> {
    let context = ProjectContext::load().await?;
    let roster = members::list(&context.session, &context.project_id).await?;
    ui::heading(&format!("Members of {}", context.config.qualified_name()));
    for member in &roster {
        let key_state = match (&member.public_key, member.key_version) {
            (None, _) => "no identity key".red().to_string(),
            (Some(_), None) => "holds no project key".yellow().to_string(),
            (Some(_), Some(version)) => format!("key v{version}").dimmed().to_string(),
        };
        println!(
            "  {:<16} {:<28} {:<8} {:<20} {}",
            member.username,
            member.email,
            member.role,
            key_state,
            member.user_id.dimmed()
        );
    }
    Ok(())
}

/// `greenbyte remove <user_id>` - revokes access.
///
/// Removal deletes their sealed key on the server, but they may have kept the
/// one they already opened, so this always points at `rotate`.
pub async fn remove(user_id: String) -> Result<(), String> {
    uuid::Uuid::parse_str(&user_id).map_err(|_| "Invalid collaborator user ID.".to_string())?;
    if !ui::confirm(&format!("Remove collaborator {user_id}?"))? {
        println!("Aborted.");
        return Ok(());
    }
    let context = ProjectContext::load().await?;
    members::remove(&context.session, &context.project_id, &user_id).await?;

    ui::success("Collaborator removed.");
    ui::warn("They may still hold the current project key.");
    ui::note("Run `greenbyte rotate` now to retire it and re-encrypt every file.");
    Ok(())
}

pub async fn change_role(user_id: String, role: String) -> Result<(), String> {
    uuid::Uuid::parse_str(&user_id).map_err(|_| "Invalid collaborator user ID.".to_string())?;
    if !matches!(role.as_str(), "admin" | "member" | "viewer") {
        return Err("Role must be admin, member, or viewer.".to_string());
    }
    let context = ProjectContext::load().await?;
    members::change_role(&context.session, &context.project_id, &user_id, &role).await?;
    ui::success(&format!("Role updated to {}.", role.cyan()));
    Ok(())
}

fn validate_email(email: &str) -> Result<(), String> {
    let (local, domain) = email
        .split_once('@')
        .ok_or("Enter a valid collaborator email address.")?;
    if local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || email.chars().any(char::is_whitespace)
    {
        return Err("Enter a valid collaborator email address.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_collaborator_addresses() {
        assert!(validate_email("dev@example.com").is_ok());
        for value in [
            "",
            "no-at-sign",
            "spaces @example.com",
            "a@b",
            "@example.com",
        ] {
            assert!(validate_email(value).is_err(), "accepted {value}");
        }
    }
}
