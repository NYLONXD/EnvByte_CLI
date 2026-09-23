//! Envbyte API.
//!
//! Layering, outermost first:
//!   `app`      - router assembly and shared middleware
//!   `features` - one module per capability, each owning its routes and queries
//!   `security` - sessions, passwords, opaque tokens, membership checks
//!   `db`       - row types and the audit recorder
//!   `infra`    - mail delivery and rate limiting
//!
//! Features may use `security`, `db` and `infra`. Nothing below `features`
//! reaches back up into one.

pub mod app;
pub mod config;
pub mod db;
pub mod error;
pub mod features;
pub mod infra;
pub mod security;
pub mod state;

pub use app::{app, build_state};
