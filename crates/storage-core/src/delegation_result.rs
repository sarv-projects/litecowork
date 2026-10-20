//! Durable child-Attempt ResultEnvelope persistence boundary.
//!
//! Envelopes are untrusted reports. The storage adapter binds them to the exact
//! terminal child Attempt and authenticated AgentSession and commits the immutable
//! record with its domain event before a parent may consume the result.

use crate::{DomainEvent, EventDraft, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct DelegationResultPublishCommit {
    pub workspace_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub agent_session_id: String,
    pub child_attempt_id: String,
    pub envelope_json: Value,
    pub envelope_digest: String,
    pub event: EventDraft,
}

#[derive(Clone, Debug)]
pub struct DelegationResultReadQuery {
    pub owner_principal_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub parent_attempt_id: String,
    pub child_attempt_id: String,
    pub task_spec_revision: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedDelegationResult {
    pub workspace_id: String,
    pub task_id: String,
    pub child_attempt_id: String,
    pub published_by_agent_session_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub envelope_json: Value,
    pub envelope_digest: String,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// Runtime-authenticated result publication and Workspace-owner result lookup.
/// Implementations must fail closed if exact child-session provenance is absent.
pub trait DelegationResultStore: Send + Sync {
    fn publish_delegation_result(
        &self,
        commit: DelegationResultPublishCommit,
    ) -> Result<CommittedDelegationResult, StoreError>;

    fn get_delegation_result(
        &self,
        query: DelegationResultReadQuery,
    ) -> Result<Option<CommittedDelegationResult>, StoreError>;
}
