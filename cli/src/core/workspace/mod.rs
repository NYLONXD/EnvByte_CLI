//! The files Envbyte keeps on disk.
//!
//!   `.envbyte`        per-directory project link  (`project_config`)
//!   `.envbyte-logs`   local encrypted snapshots   (`commit_log`)
//!   `~/.envbyte-auth` account session             (`global_auth`)
//!   `~/.envbyte-identity` device identity key     (`crypto::identity`)
//!
//! All of them are written through `paths::secure_atomic_write`, so a crash
//! never leaves a half-written credential behind.

pub mod commit_log;
pub mod global_auth;
pub mod legacy;
pub mod paths;
pub mod project_config;
