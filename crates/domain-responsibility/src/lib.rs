//! Coworker identity and Automation definition domain boundary.
//!
//! This crate supplies immutable revision values, guarded lifecycle decisions, and
//! transaction ports. It has no persistence adapter, trigger provider, Task runner,
//! Operator route, or UI projection. See README.md for integration obligations.
mod automation_health;
mod delegation_profiles;
mod goals;
mod identity;
mod models;
mod notifications;
mod occurrences;
mod routines;
mod schedule;
mod service;
mod suggestions;
mod trigger_host;

pub use automation_health::*;
pub use delegation_profiles::*;
pub use goals::*;
pub use identity::*;
pub use models::*;
pub use notifications::*;
pub use occurrences::*;
pub use routines::*;
pub use schedule::*;
pub use service::*;
pub use suggestions::*;
pub use trigger_host::*;

#[cfg(test)]
mod automation_health_tests;
#[cfg(test)]
mod notification_tests;
#[cfg(test)]
mod occurrence_tests;
#[cfg(test)]
mod schedule_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod trigger_host_tests;
