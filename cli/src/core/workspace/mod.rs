//! The files Greenbyte keeps on disk.
//!
//!   `.greenbyte`        per-directory project link  (`project_config`)
//!   `.greenbyte-logs`   local encrypted snapshots   (`commit_log`)
//!   `~/.greenbyte-auth` account session             (`global_auth`)
//!   `~/.greenbyte-identity` device identity key     (`crypto::identity`)
//!
//! All of them are written through `paths::secure_atomic_write`, so a crash
//! never leaves a half-written credential behind.

pub mod commit_log;
pub mod global_auth;
pub mod paths;
pub mod project_config;
