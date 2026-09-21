//! `greenbyte identity` - inspect, export or replace this device's key.
//!
//! The identity key is what project keys are sealed to. It never leaves the
//! device unless the user deliberately exports it.

use colored::Colorize;

use crate::{
    commands::context::{account_session, ensure_identity_published},
    core::crypto::identity::{self, export_secret, Identity},
    ui,
};

/// Shows the key this device uses, and whether the server knows it.
pub async fn show() -> Result<(), String> {
    let (identity, created) = identity::load_or_create()?;
    ui::heading("Device identity");
    ui::field("Fingerprint", &identity.fingerprint());
    ui::field("Public key", &identity.public_key_base64());
    ui::field(
        "Stored in",
        &identity::identity_path().display().to_string(),
    );
    if created {
        ui::note("Created just now.");
    }

    match account_session().await {
        Ok(session) => match crate::core::api::accounts::me(&session).await {
            Ok(profile) => match profile.public_key.as_deref() {
                Some(published) if published == identity.public_key_base64() => {
                    ui::step("Published - colleagues can seal project keys to this device.")
                }
                Some(_) => {
                    ui::warn("The server has a different key for your account.");
                    ui::note("Run `greenbyte identity publish` to replace it.");
                }
                None => {
                    ui::warn("No key published for your account yet.");
                    ui::note("Run `greenbyte identity publish`.");
                }
            },
            Err(error) => ui::warn(&format!("Could not check the published key: {error}")),
        },
        Err(_) => ui::note("Not signed in, so the published key could not be checked."),
    }
    Ok(())
}

/// Publishes the current device key, replacing whatever the server holds.
pub async fn publish() -> Result<(), String> {
    let session = account_session().await?;
    let identity = ensure_identity_published(&session).await?;
    ui::success(&format!("Published identity {}.", identity.fingerprint()));
    Ok(())
}

/// Prints the secret half, for moving to another machine or a CI secret store.
pub async fn export() -> Result<(), String> {
    let (identity, _) = identity::load_or_create()?;
    ui::warn("This prints a secret. Anyone holding it can read every project you can.");
    if !ui::confirm("Print the identity secret key?")? {
        println!("Aborted.");
        return Ok(());
    }
    let secret = export_secret(&identity);
    println!();
    println!("  {}", secret.as_str().cyan());
    println!();
    ui::note("Set it as GREENBYTE_IDENTITY_KEY on the other machine or in CI.");
    Ok(())
}

/// Replaces this device's identity key.
///
/// Existing project keys were sealed to the old key, so access has to be
/// re-granted by an admin afterwards. Saying so up front avoids a confusing
/// "you hold no key" the next time they pull.
pub async fn replace() -> Result<(), String> {
    ui::warn("Replacing your identity makes every project key sealed to it unreadable.");
    ui::note("An admin must re-invite you, or rotate, before you can pull again.");
    if !ui::confirm("Replace this device's identity key?")? {
        println!("Aborted.");
        return Ok(());
    }
    let replacement = Identity::generate();
    identity::save(&replacement)?;
    let session = account_session().await?;
    crate::core::api::accounts::publish_identity_key(&session, &replacement.public_key_base64())
        .await?;

    let mut auth = crate::core::workspace::global_auth::load().unwrap_or_default();
    auth.published_key_fingerprint = Some(replacement.fingerprint());
    crate::core::workspace::global_auth::save(&auth)?;

    ui::success(&format!(
        "Identity replaced. New fingerprint {}.",
        replacement.fingerprint()
    ));
    ui::note("Ask an admin to re-grant your project access.");
    Ok(())
}
