//! `envbyte rotate` - retire the project data key and issue a new one.
//!
//! This is what makes offboarding real. Removing a member deletes their sealed
//! copy on the server, but they may have kept the key they already opened;
//! only a rotation makes that copy worthless.
//!
//! The whole operation is prepared locally and applied in one server call, so
//! either every member gets the new key and every file moves to it, or nothing
//! changes at all.

use colored::Colorize;

use crate::{
    commands::context::ProjectContext,
    core::{
        api::{
            env,
            keys::{self, GrantInput, RotatedFile},
            members, projects,
        },
        crypto::{envelope, generate_data_key, open_payload, sealing, EncryptedPayload},
        device,
    },
    ui,
};

pub async fn rotate(assume_yes: bool) -> Result<(), String> {
    let context = ProjectContext::load().await?;
    let keyring = context.keyring().await?;

    let bar = ui::spinner("Gathering members and files...");
    let roster = members::list(&context.session, &context.project_id).await;
    let files = env::pull(&context.session, &context.project_id).await;
    let current_version =
        projects::current_key_version(&context.session, &context.project_id).await;
    bar.finish_and_clear();
    let (roster, files, current_version) = (roster?, files?, current_version?);

    // A member with no published identity key cannot be sealed to. Rotating
    // anyway would lock them out, so stop and name them.
    let unreachable: Vec<&str> = roster
        .iter()
        .filter(|member| member.public_key.is_none())
        .map(|member| member.username.as_str())
        .collect();
    if !unreachable.is_empty() {
        return Err(format!(
            "These members have not published an identity key yet, so a new key cannot be sealed \
             to them: {}. Ask them to run `envbyte login` with an up-to-date CLI, then rotate.",
            unreachable.join(", ")
        ));
    }

    let next_version = current_version + 1;
    ui::heading("Key rotation");
    ui::field("From", &format!("version {current_version}"));
    ui::field("To", &format!("version {next_version}"));
    ui::field(
        "Members",
        &format!("{} will receive the new key", roster.len()),
    );
    ui::field("Files", &format!("{} will be re-encrypted", files.len()));
    for member in &roster {
        ui::note(&format!(
            "  {} <{}> ({})",
            member.username, member.email, member.role
        ));
    }
    println!();
    ui::note("Anyone removed before this point loses access to everything pushed afterwards.");
    ui::note("Project history stays readable to current members.");
    println!();

    if !assume_yes && !ui::confirm("Rotate the project key now?")? {
        println!("Aborted. Nothing changed.");
        return Ok(());
    }

    // Decrypt every file with the keys we hold, then re-encrypt under the new
    // one. Doing this before the server call means a decryption failure costs
    // nothing.
    let new_key = generate_data_key();
    let mut reencrypted = Vec::with_capacity(files.len());
    for file in &files {
        let payload = EncryptedPayload {
            data: file.content.clone(),
        };
        let plaintext = open_payload(&keyring, &payload, device::get_device_mac().as_deref())
            .map_err(|error| {
                format!(
                    "Could not read {} before re-encrypting it: {error}",
                    file.filename
                )
            })?;
        let encrypted = envelope::encrypt(&plaintext, &new_key, next_version)?;
        reencrypted.push(RotatedFile {
            filename: file.filename.clone(),
            content: encrypted.data,
        });
    }

    let mut grants = Vec::with_capacity(roster.len());
    for member in &roster {
        let public_key = member
            .public_key
            .as_deref()
            .ok_or_else(|| format!("{} has no identity key.", member.username))?;
        grants.push(GrantInput {
            user_id: member.user_id.clone(),
            wrapped_key: sealing::seal(&new_key, public_key)?,
        });
    }

    let bar = ui::spinner("Applying rotation...");
    let result = keys::rotate(&context.session, &context.project_id, &grants, &reencrypted).await;
    bar.finish_and_clear();
    let result = result?;

    let mut config = context.config;
    config.key_version = Some(result.key_version);
    crate::core::workspace::project_config::save(&config)?;

    ui::success(&format!(
        "Project key rotated to version {}.",
        result.key_version.to_string().cyan()
    ));
    ui::field("Members granted", &result.members_granted.to_string());
    ui::field("Files re-encrypted", &result.files_reencrypted.to_string());
    if result.grants_revoked > 0 {
        ui::field("Stale keys revoked", &result.grants_revoked.to_string());
    }
    ui::note("Tell the team to run `envbyte pull` to pick up the new key.");
    Ok(())
}
