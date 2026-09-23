//! A typed client for the Envbyte API.
//!
//! Commands call these functions instead of building URLs and JSON by hand, so
//! a change to the wire format is a change in one place, and every request
//! goes through the same auth and error handling.

pub mod accounts;
pub mod client;
pub mod env;
pub mod keys;
pub mod members;
pub mod projects;

pub use client::Session;
