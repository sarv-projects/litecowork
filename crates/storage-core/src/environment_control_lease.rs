//! Durable, epoch-fenced ownership of input control for one exact Environment Attempt.
//!
//! Fencing credentials are issued by the private Runtime control plane. The durable
//! contract carries only their digest and issuer key version. This port deliberately
//! has no renewal or HUMAN-to-AGENT method: those require an authenticated private
//! control channel and a persisted fresh-observation producer.

use crate::{EnvironmentRequestIdentity, EventDraft, StoreError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentControlLeaseState {
    Active,
    Releasing,
    Released,
    Expired,
    Revoked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HumanPrincipalKind {
    User,
    Service,
    Runtime,
    Agent,
    ChannelIdentity,
}

impl HumanPrincipalKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "USER",
            Self::Service => "SERVICE",
            Self::Runtime => "RUNTIME",
            Self::Agent => "AGENT",
            Self::ChannelIdentity => "CHANNEL_IDENTITY",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "SCREAMING_SNAKE_CASE",
    tag = "owner_kind"
)]
pub enum EnvironmentControlOwner {
    Agent {
        agent_session_id: String,
    },
    Human {
        principal_id: String,
        principal_kind: HumanPrincipalKind,
    },
}

impl EnvironmentControlOwner {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Agent { .. } => "AGENT",
            Self::Human { .. } => "HUMAN",
        }
    }

    pub fn reference_json(&self) -> Result<serde_json::Value, StoreError> {
        match self {
            Self::Agent { agent_session_id } => {
                if agent_session_id.trim().is_empty() {
                    return Err(StoreError::Invalid(
                        "CONTROL_LEASE_AGENT_SESSION_REQUIRED".to_owned(),
                    ));
                }
                serde_json::to_value(agent_session_id)
                    .map_err(|error| StoreError::Invalid(error.to_string()))
            }
            Self::Human {
                principal_id,
                principal_kind,
            } => {
                if principal_id.trim().is_empty() {
                    return Err(StoreError::Invalid(
                        "CONTROL_LEASE_HUMAN_PRINCIPAL_REQUIRED".to_owned(),
                    ));
                }
                serde_json::to_value(serde_json::json!({
                    "kind": principal_kind.as_str(),
                    "principal_id": principal_id,
                }))
                .map_err(|error| StoreError::Invalid(error.to_string()))
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentControlLeaseRecord {
    pub control_lease_id: String,
    pub environment_id: String,
    pub task_id: String,
    pub attempt_id: String,
    /// Read from the exact Attempt binding; retained here even after HUMAN takeover.
    pub agent_session_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub owner: EnvironmentControlOwner,
    pub epoch: u64,
    pub issuer_key_version: u32,
    pub fencing_token_digest: String,
    pub state: EnvironmentControlLeaseState,
    pub acquired_at: String,
    pub expires_at: String,
    pub version: u64,
}

#[derive(Clone, Debug)]
pub struct EnvironmentControlLeaseAcquire {
    pub workspace_id: String,
    pub control_lease_id: String,
    pub environment_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub agent_session_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub issuer_key_version: u32,
    pub fencing_token_digest: String,
    pub acquired_at: String,
    pub expires_at: String,
    pub identity: EnvironmentRequestIdentity,
    pub event: EventDraft,
}

#[derive(Clone, Debug)]
pub struct EnvironmentControlLeaseTakeover {
    pub workspace_id: String,
    pub control_lease_id: String,
    pub environment_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub agent_session_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub expected_version: u64,
    pub expected_epoch: u64,
    pub issuer_key_version: u32,
    pub fencing_token_digest: String,
    pub at: String,
    pub identity: EnvironmentRequestIdentity,
    pub event: EventDraft,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentControlLeaseEndState {
    Released,
    Revoked,
}

#[derive(Clone, Debug)]
pub struct EnvironmentControlLeaseEnd {
    pub workspace_id: String,
    pub control_lease_id: String,
    pub environment_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub agent_session_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub expected_version: u64,
    pub expected_epoch: u64,
    pub end_state: EnvironmentControlLeaseEndState,
    pub at: String,
    pub identity: EnvironmentRequestIdentity,
    pub event: EventDraft,
}

#[derive(Clone, Debug)]
pub struct CommittedEnvironmentControlLease {
    pub lease: EnvironmentControlLeaseRecord,
    pub replayed: bool,
}

/// Owner-authenticated durable state transitions. Providers do not call this trait
/// directly; the authenticated Operator/Runtime service owns those boundaries.
pub trait EnvironmentControlLeaseStore: Send + Sync {
    fn acquire_environment_control_lease(
        &self,
        request: EnvironmentControlLeaseAcquire,
    ) -> Result<CommittedEnvironmentControlLease, StoreError>;

    fn take_over_environment_control_lease(
        &self,
        request: EnvironmentControlLeaseTakeover,
    ) -> Result<CommittedEnvironmentControlLease, StoreError>;

    fn end_environment_control_lease(
        &self,
        request: EnvironmentControlLeaseEnd,
    ) -> Result<CommittedEnvironmentControlLease, StoreError>;
}
