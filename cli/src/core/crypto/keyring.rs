//! The set of project data keys this member can open.
//!
//! A member holds one sealed key per version they were present for. Current
//! files are read and written with the newest; older versions stay so that
//! history and rolled-back files remain readable.

use std::collections::BTreeMap;

use crate::core::crypto::{
    envelope::{self, EncryptedPayload},
    identity::Identity,
    sealing, DataKey,
};

pub struct Keyring {
    keys: BTreeMap<i32, DataKey>,
}

/// One sealed key as the server stores it.
pub struct Grant {
    pub key_version: i32,
    pub wrapped_key: String,
}

impl Keyring {
    /// Opens every grant the caller holds. A grant that cannot be opened is
    /// skipped rather than fatal: a member may hold keys from before they
    /// replaced their identity, and the newer ones are what matter.
    pub fn open(grants: Vec<Grant>, identity: &Identity) -> Result<Self, String> {
        let mut keys = BTreeMap::new();
        let mut failures = Vec::new();
        for grant in grants {
            match sealing::open(&grant.wrapped_key, identity) {
                Ok(key) => {
                    keys.insert(grant.key_version, key);
                }
                Err(error) => failures.push((grant.key_version, error)),
            }
        }
        if keys.is_empty() {
            let detail = failures
                .first()
                .map(|(version, error)| format!(" (version {version}: {error})"))
                .unwrap_or_default();
            return Err(format!(
                "None of your project keys could be opened with this device's identity{detail}"
            ));
        }
        Ok(Self { keys })
    }

    /// The newest key this member holds - what new content is written under.
    pub fn current(&self) -> Result<(&DataKey, i32), String> {
        self.keys
            .iter()
            .next_back()
            .map(|(version, key)| (key, *version))
            .ok_or_else(|| "You hold no project keys.".to_string())
    }

    pub fn current_version(&self) -> i32 {
        self.keys.keys().next_back().copied().unwrap_or(0)
    }

    pub fn versions(&self) -> Vec<i32> {
        self.keys.keys().copied().collect()
    }

    pub fn for_version(&self, version: i32) -> Option<&DataKey> {
        self.keys.get(&version)
    }

    /// Decrypts a payload with whichever held key its header names.
    pub fn decrypt(&self, payload: &EncryptedPayload) -> Result<String, String> {
        let version = envelope::key_version_of(payload).ok_or(
            "This snapshot predates project key wrapping. Re-push it from a client that holds \
             the project key, or read it with the old master key.",
        )?;
        let key = self.for_version(version).ok_or_else(|| {
            format!(
                "This content needs project key version {version}, which you do not hold. \
                 Ask an admin to grant you access, or pull again after the next rotation."
            )
        })?;
        envelope::decrypt(payload, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::crypto::generate_data_key;

    fn grant_for(identity: &Identity, version: i32, key: &DataKey) -> Grant {
        Grant {
            key_version: version,
            wrapped_key: sealing::seal(key, &identity.public_key_base64()).unwrap(),
        }
    }

    #[test]
    fn opens_every_held_version_and_prefers_the_newest() {
        let identity = Identity::generate();
        let first = generate_data_key();
        let second = generate_data_key();
        let keyring = Keyring::open(
            vec![
                grant_for(&identity, 1, &first),
                grant_for(&identity, 2, &second),
            ],
            &identity,
        )
        .unwrap();
        assert_eq!(keyring.versions(), vec![1, 2]);
        let (current, version) = keyring.current().unwrap();
        assert_eq!(version, 2);
        assert_eq!(**current, *second);
    }

    #[test]
    fn reads_history_written_under_a_retired_key() {
        let identity = Identity::generate();
        let old = generate_data_key();
        let new = generate_data_key();
        let archived = envelope::encrypt("OLD=value", &old, 1).unwrap();
        let keyring = Keyring::open(
            vec![grant_for(&identity, 1, &old), grant_for(&identity, 2, &new)],
            &identity,
        )
        .unwrap();
        assert_eq!(keyring.decrypt(&archived).unwrap(), "OLD=value");
    }

    #[test]
    fn explains_a_version_the_member_does_not_hold() {
        let identity = Identity::generate();
        let held = generate_data_key();
        let missing = envelope::encrypt("NEW=value", &generate_data_key(), 5).unwrap();
        let keyring = Keyring::open(vec![grant_for(&identity, 1, &held)], &identity).unwrap();
        let error = keyring.decrypt(&missing).unwrap_err();
        assert!(error.contains("version 5"), "{error}");
    }

    #[test]
    fn grants_sealed_to_someone_else_are_skipped_not_fatal() {
        let identity = Identity::generate();
        let stranger = Identity::generate();
        let mine = generate_data_key();
        let keyring = Keyring::open(
            vec![
                grant_for(&stranger, 1, &generate_data_key()),
                grant_for(&identity, 2, &mine),
            ],
            &identity,
        )
        .unwrap();
        assert_eq!(keyring.versions(), vec![2]);
    }

    #[test]
    fn reports_when_nothing_can_be_opened() {
        let identity = Identity::generate();
        let stranger = Identity::generate();
        let result = Keyring::open(
            vec![grant_for(&stranger, 1, &generate_data_key())],
            &identity,
        );
        assert!(result.is_err());
    }
}
