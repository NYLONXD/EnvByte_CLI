//! One module per feature, each owning its own routes, wire types and queries.
//!
//! Adding a capability means adding a folder here and one line in `app.rs`,
//! rather than growing a shared handler file.

pub mod accounts;
pub mod audit;
pub mod env_files;
pub mod health;
pub mod keys;
pub mod members;
pub mod projects;
