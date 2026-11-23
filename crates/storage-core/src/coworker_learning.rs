//! Compatibility re-export for the responsibility-domain learning gates.
//!
//! The canonical pure policy lives in `domain-responsibility`; storage-core keeps
//! this module so internal storage consumers retain their existing import path.
pub use domain_responsibility::learning_gates::*;
