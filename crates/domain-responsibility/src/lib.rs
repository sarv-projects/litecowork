//! Coworker identity and Automation definition domain boundary.
//!
//! This crate supplies immutable revision values, guarded lifecycle decisions, and
//! transaction ports. It has no persistence adapter, trigger provider, Task runner,
//! Operator route, or UI projection. See README.md for integration obligations.
mod delegation_profiles;
mod goals;
mod identity;
mod models;
mod routines;
mod service;
mod suggestions;

pub use delegation_profiles::*;
pub use goals::*;
pub use identity::*;
pub use models::*;
pub use routines::*;
pub use service::*;
pub use suggestions::*;

#[cfg(test)]
mod tests;
