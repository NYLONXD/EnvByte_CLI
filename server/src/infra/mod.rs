//! Adapters to the world outside the process: mail delivery and request
//! shaping. Nothing here knows about Greenbyte's domain rules.

pub mod email;
pub mod rate_limit;
