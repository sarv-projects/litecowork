//! Runtime-local, redacted observations of native agent authentication.
//!
//! These records are operational evidence for one exact AgentProfile and Runtime
//! incarnation. They are not Workspace authority, credentials, provider transcripts,
//! or proof that a native session is ready.

use crate::StoreError;
use serde::{Deserialize, Serialize};

pub const AGENT_AUTH_OBSERVATION_MAX_ID_BYTES: usize = 256;
pub const AGENT_AUTH_OBSERVATION_MAX_METHOD_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentAuthObservationKey {
    pub agent_profile_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub module_id: String,
    pub module_version: String,
    pub auth_method_id: String,
}

/// Scope for reading the current observations of the exact descriptor's declared
/// methods. An observation for any other profile, Runtime, incarnation or module
/// version cannot be projected into this descriptor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentAuthObservationScope {
    pub agent_profile_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub module_id: String,
    pub module_version: String,
}

impl AgentAuthObservationKey {
    pub fn scope(&self) -> AgentAuthObservationScope {
        AgentAuthObservationScope {
            agent_profile_id: self.agent_profile_id.clone(),
            runtime_id: self.runtime_id.clone(),
            runtime_incarnation_id: self.runtime_incarnation_id.clone(),
            module_id: self.module_id.clone(),
            module_version: self.module_version.clone(),
        }
    }
}

/// `UNKNOWN` is represented by no current row. Keeping it out of persisted states
/// prevents absence, expiry or a failed read from looking like a provider assertion.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentAuthObservationState {
    NeedsAuth,
    Authenticating,
    Authenticated,
    Expired,
    Degraded,
    Error,
}

impl AgentAuthObservationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeedsAuth => "NEEDS_AUTH",
            Self::Authenticating => "AUTHENTICATING",
            Self::Authenticated => "AUTHENTICATED",
            Self::Expired => "EXPIRED",
            Self::Degraded => "DEGRADED",
            Self::Error => "ERROR",
        }
    }

    pub fn from_str(value: &str) -> Result<Self, StoreError> {
        match value {
            "NEEDS_AUTH" => Ok(Self::NeedsAuth),
            "AUTHENTICATING" => Ok(Self::Authenticating),
            "AUTHENTICATED" => Ok(Self::Authenticated),
            "EXPIRED" => Ok(Self::Expired),
            "DEGRADED" => Ok(Self::Degraded),
            "ERROR" => Ok(Self::Error),
            _ => Err(StoreError::CorruptSchema(
                "agent auth observation has an unknown state".to_owned(),
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentAuthObservationRecord {
    pub key: AgentAuthObservationKey,
    pub state: AgentAuthObservationState,
    pub generation: u64,
    pub observed_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentAuthObservationReservation {
    Reserved { generation: u64, expires_at: String },
    InFlight(AgentAuthObservationRecord),
}

pub trait AgentAuthObservationStore: Send + Sync {
    /// Atomically reserves a new AUTHENTICATING generation, or returns the exact
    /// already active reservation. No native process or other external call belongs
    /// inside this transaction.
    fn reserve_agent_auth_observation(
        &self,
        key: AgentAuthObservationKey,
        observed_at: &str,
        expires_at: &str,
    ) -> Result<AgentAuthObservationReservation, StoreError>;

    /// Settles or cancels only the matching current AUTHENTICATING generation. A
    /// stale callback returns `false` and cannot replace a newer attempt.
    fn settle_agent_auth_observation(
        &self,
        key: AgentAuthObservationKey,
        generation: u64,
        state: AgentAuthObservationState,
        observed_at: &str,
        expires_at: &str,
    ) -> Result<bool, StoreError>;

    /// Returns a single current method observation, if its exact scope is unexpired.
    fn current_agent_auth_observation(
        &self,
        key: AgentAuthObservationKey,
        now: &str,
    ) -> Result<Option<AgentAuthObservationRecord>, StoreError>;

    /// Lists only current observations under one exact descriptor scope. Implementors
    /// bound this to the descriptor's declared method limit.
    fn list_agent_auth_observations(
        &self,
        scope: AgentAuthObservationScope,
        now: &str,
    ) -> Result<Vec<AgentAuthObservationRecord>, StoreError>;
}

pub fn validate_agent_auth_observation_key(
    key: &AgentAuthObservationKey,
) -> Result<(), StoreError> {
    let identifier_ok = |value: &str, max_bytes: usize| {
        !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
    };
    if !identifier_ok(&key.agent_profile_id, AGENT_AUTH_OBSERVATION_MAX_ID_BYTES)
        || !identifier_ok(&key.runtime_id, AGENT_AUTH_OBSERVATION_MAX_ID_BYTES)
        || !identifier_ok(
            &key.runtime_incarnation_id,
            AGENT_AUTH_OBSERVATION_MAX_ID_BYTES,
        )
        || !identifier_ok(&key.module_id, 128)
        || !identifier_ok(&key.module_version, 128)
        || !identifier_ok(&key.auth_method_id, AGENT_AUTH_OBSERVATION_MAX_METHOD_BYTES)
    {
        return Err(StoreError::Invalid(
            "agent auth observation identity is invalid".to_owned(),
        ));
    }
    Ok(())
}

pub fn validate_agent_auth_observation_scope(
    scope: &AgentAuthObservationScope,
) -> Result<(), StoreError> {
    validate_agent_auth_observation_key(&AgentAuthObservationKey {
        agent_profile_id: scope.agent_profile_id.clone(),
        runtime_id: scope.runtime_id.clone(),
        runtime_incarnation_id: scope.runtime_incarnation_id.clone(),
        module_id: scope.module_id.clone(),
        module_version: scope.module_version.clone(),
        auth_method_id: "scope-probe".to_owned(),
    })
}
