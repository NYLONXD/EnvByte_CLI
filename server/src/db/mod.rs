//! Database row types and the helpers every feature shares.
//!
//! Features own their own queries; what lives here is only what more than one
//! of them needs to agree on.

pub mod audit;
pub mod models;
