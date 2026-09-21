//! Resolving what a command needs before it can do anything.
//!
//! Most commands need the same three things: a project link, a live session,
//! and this device's identity. Gathering them in one place keeps that
//! boilerplate out of every command and makes the failure messages uniform.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use crate::{
    core::{
        api::{accounts, client::server_url, keys, Session},
        crypto::{
            identity::{self, Identity},
            keyring::Keyring,
        },
        workspace::{global_auth, project_config::ProjectConfig},
    },
    ui,
};

/// A command operating inside a linked project.
pub struct ProjectContext {
    pub config: ProjectConfig,
    pub project_id: String,
    pub session: Session,
    pub identity: Identity,
}

impl ProjectContext {
    /// Loads the project link, refreshes the session and loads the identity.
    pub async fn load() -> Result<Self, String> {
        let config = crate::core::workspace::project_config::load()?;
        let project_id = config.require_project_id()?.to_string();
        let token = access_token(&config.server_url, config.auth_token.clone()).await?;
        let session = Session::new(&config.server_url, token)?;
        let (identity, created) = identity::load_or_create()?;
        if created {
            announce_new_identity(&identity);
            accounts::publish_identity_key(&session, &identity.public_key_base64()).await?;
        }
        Ok(Self {
            config,
            project_id,
            session,
            identity,
        })
    }

    /// Fetches and opens every project key this member holds.
    pub async fn keyring(&self) -> Result<Keyring, String> {
        let grants = keys::grants(&self.session, &self.project_id).await?;
        Keyring::open(grants, &self.identity)
    }
}

/// Resolves a session for commands that are not inside a project.
pub async fn account_session() -> Result<Session, String> {
    let server = server_url();
    let token = access_token(&server, None).await?;
    Session::new(&server, token)
}

/// Makes sure this device has an identity and that the server knows its public
/// half.
///
/// Called after every sign-in, so a member becomes invitable as soon as they
/// log in rather than at some later, surprising moment.
pub async fn ensure_identity_published(session: &Session) -> Result<Identity, String> {
    let (identity, created) = identity::load_or_create()?;
    if created {
        announce_new_identity(&identity);
    }
    let fingerprint = identity.fingerprint();
    let mut auth = global_auth::load().unwrap_or_default();
    let already_published = auth.published_key_fingerprint.as_deref() == Some(fingerprint.as_str());
    if !already_published {
        accounts::publish_identity_key(session, &identity.public_key_base64()).await?;
        auth.published_key_fingerprint = Some(fingerprint);
        global_auth::save(&auth)?;
    }
    Ok(identity)
}

fn announce_new_identity(identity: &Identity) {
    ui::step(&format!(
        "Created a device identity key ({})",
        identity.fingerprint()
    ));
    ui::note(&format!(
        "Stored in {}. Treat it like an SSH private key.",
        identity::identity_path().display()
    ));
    ui::note("Lose it and an admin can grant you access again; leak it and they must rotate.");
}

/// Returns a usable access token, refreshing it when it is close to expiry.
pub async fn access_token(server: &str, fallback: Option<String>) -> Result<String, String> {
    let mut auth = global_auth::load()?;
    if let Some(token) = auth.auth_token.as_ref() {
        if !expires_soon(token) {
            return Ok(token.clone());
        }
    }
    if let Some(refresh_token) = auth.refresh_token.clone() {
        let refreshed = accounts::refresh(server, &refresh_token).await?;
        auth.auth_token = Some(refreshed.token.clone());
        auth.refresh_token = refreshed.refresh_token;
        auth.email = Some(refreshed.email);
        auth.user_id = Some(refreshed.user_id);
        global_auth::save(&auth)?;
        return Ok(refreshed.token);
    }
    auth.auth_token
        .or(fallback)
        .ok_or_else(|| "Not logged in. Run `greenbyte login` first.".to_string())
}

/// Treats a token as expired two minutes early, so a long operation does not
/// fail halfway through.
fn expires_soon(token: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn token_expiring_at(timestamp: i64) -> String {
        let claims = serde_json::json!({ "exp": timestamp });
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        format!("header.{payload}.signature")
    }

    #[test]
    fn treats_expired_and_nearly_expired_tokens_as_stale() {
        let now = chrono::Utc::now().timestamp();
        assert!(expires_soon(&token_expiring_at(now - 1)));
        assert!(expires_soon(&token_expiring_at(now + 60)));
        assert!(!expires_soon(&token_expiring_at(now + 3600)));
    }

    #[test]
    fn treats_unreadable_tokens_as_stale() {
        for token in ["", "not-a-jwt", "a.b", "a.!!!.c"] {
            assert!(expires_soon(token), "accepted {token}");
        }
    }
}
