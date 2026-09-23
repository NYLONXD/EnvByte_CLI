//! The parts of the CLI that do not touch the terminal.
//!
//!   `crypto`    - identity keys, key sealing, the ciphertext envelope
//!   `api`       - a typed client for the Envbyte API
//!   `workspace` - the files Envbyte keeps on disk
//!
//! Commands compose these; nothing here prints or prompts, so it is all
//! directly testable.

pub mod api;
pub mod crypto;
pub mod device;
pub mod env_files;
pub mod workspace;
