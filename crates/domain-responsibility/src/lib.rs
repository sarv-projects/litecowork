//! Coworker identity and Automation definition domain boundary.
//!
//! This crate supplies immutable revision values, guarded lifecycle decisions, and
//! transaction ports. It has no persistence adapter, trigger provider, Task runner,
//! Operator route, or UI projection. See README.md for integration obligations.
mod models;
mod service;
mod identity;
mod goals;
mod suggestions;
mod routines;
mod delegation_profiles;

pub use models::*;
pub use service::*;
pub use identity::*;
pub use goals::*;
pub use suggestions::*;
pub use routines::*;
pub use delegation_profiles::*;

#[cfg(test)]
mod tests;
