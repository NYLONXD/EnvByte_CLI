//! One-time carry-over of files written under the project's former name,
//! Greenbyte.
//!
//! The identity key is the file that matters: losing it means asking an admin
//! to grant access again. So rather than making anyone rename files by hand,
//! the CLI carries them over to their Envbyte names the first time it runs.

use std::path::{Path, PathBuf};

use crate::core::{
    crypto::identity,
    workspace::{commit_log, global_auth, paths::secure_atomic_write, project_config},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Transfer {
    /// The identity is copied, not moved. An old `greenbyte` binary that
    /// found no key would silently create and publish a new one, splitting
    /// the account across two identities.
    Copy,
    /// Everything else is moved: without it, an old binary merely reports
    /// that it is signed out or not linked to a project.
    Move,
}

/// A file carried over from its pre-rename location.
pub struct Carried {
    pub from: PathBuf,
    pub to: PathBuf,
    pub transfer: Transfer,
}

/// Carries over every pre-rename file that has not been carried over yet.
/// Nothing at a new path is ever replaced, so after the first run this is a
/// handful of `exists` checks.
pub fn migrate() -> (Vec<Carried>, Vec<String>) {
    let mut carried = Vec::new();
    let mut failures = Vec::new();
    for (from, to, transfer) in candidates() {
        match carry_over(&from, &to, transfer) {
            Ok(true) => carried.push(Carried { from, to, transfer }),
            Ok(false) => {}
            Err(error) => failures.push(error),
        }
    }
    (carried, failures)
}

/// Whether a project link was carried over, in which case the project's
/// `.gitignore` needs the new names.
pub fn carried_project_link(carried: &[Carried]) -> bool {
    carried
        .iter()
        .any(|entry| entry.to == project_config::config_path())
}

fn candidates() -> Vec<(PathBuf, PathBuf, Transfer)> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut candidates = Vec::new();
    // A custom identity location was chosen deliberately; leave it alone.
    if !identity::has_custom_path() {
        candidates.push((
            home.join(".greenbyte-identity"),
            identity::identity_path(),
            Transfer::Copy,
        ));
    }
    candidates.extend([
        (
            home.join(".greenbyte-auth"),
            global_auth::path(),
            Transfer::Move,
        ),
        (
            PathBuf::from(".greenbyte"),
            project_config::config_path(),
            Transfer::Move,
        ),
        (
            PathBuf::from(".greenbyte-logs"),
            commit_log::logs_path(),
            Transfer::Move,
        ),
    ]);
    candidates
}

fn carry_over(from: &Path, to: &Path, transfer: Transfer) -> Result<bool, String> {
    if !from.is_file() || to.exists() {
        return Ok(false);
    }
    let failed = |e: String| {
        format!(
            "Could not carry {} over to {}: {e}",
            from.display(),
            to.display()
        )
    };
    match transfer {
        // Written atomically, so a crash cannot leave a truncated key at the
        // new path that every later run would then skip over.
        Transfer::Copy => {
            let contents = std::fs::read(from).map_err(|e| failed(e.to_string()))?;
            secure_atomic_write(to, &contents).map_err(failed)?;
        }
        Transfer::Move => std::fs::rename(from, to).map_err(|e| failed(e.to_string()))?,
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let directory = std::env::temp_dir().join(format!("envbyte-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        directory
    }

    #[test]
    fn moves_an_old_file_to_its_new_name() {
        let directory = scratch();
        let (from, to) = (directory.join("old"), directory.join("new"));
        std::fs::write(&from, "link").unwrap();
        assert!(carry_over(&from, &to, Transfer::Move).unwrap());
        assert!(!from.exists());
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "link");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn copies_the_identity_so_an_old_binary_keeps_it() {
        let directory = scratch();
        let (from, to) = (directory.join("old"), directory.join("new"));
        std::fs::write(&from, "identity").unwrap();
        assert!(carry_over(&from, &to, Transfer::Copy).unwrap());
        assert_eq!(std::fs::read_to_string(&from).unwrap(), "identity");
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "identity");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn never_replaces_a_file_at_the_new_name() {
        let directory = scratch();
        let (from, to) = (directory.join("old"), directory.join("new"));
        std::fs::write(&from, "old identity").unwrap();
        std::fs::write(&to, "new identity").unwrap();
        for transfer in [Transfer::Copy, Transfer::Move] {
            assert!(!carry_over(&from, &to, transfer).unwrap());
        }
        assert_eq!(std::fs::read_to_string(&from).unwrap(), "old identity");
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "new identity");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn does_nothing_without_an_old_file() {
        let directory = scratch();
        let to = directory.join("new");
        assert!(!carry_over(&directory.join("old"), &to, Transfer::Move).unwrap());
        assert!(!to.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
