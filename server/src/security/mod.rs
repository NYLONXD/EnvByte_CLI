//! Authentication, authorization and credential handling.
//!
//! Split by concern so that a change to session issuance cannot quietly
//! disturb password hashing or the membership checks.

pub mod password;
pub mod permissions;
pub mod session;
pub mod tokens;

pub use password::{hash_password, verify_password};
pub use session::{access_token, AuthUser};
pub use tokens::{hash_token, issue_refresh_token, random_token, rotate_refresh_token};
