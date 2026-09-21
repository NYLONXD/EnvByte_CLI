//! One module per command group.
//!
//! Commands are thin: they gather input, call into `core`, and print. Anything
//! worth testing lives in `core`, which does not touch the terminal.

pub mod account;
pub mod audit;
pub mod context;
pub mod history;
pub mod identity;
pub mod members;
pub mod project;
pub mod rollback;
pub mod rotate;
pub mod snapshot;
pub mod status;
pub mod sync;
