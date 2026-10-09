//! Internal Core persistence port for already-admitted CapabilityInvocation lifecycle
//! observations. It deliberately has no create or dispatch operation.

use crate::{DomainEvent, StoreError};
use domain_invocations::{CapabilityInvocationRecord, InvocationStatus};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityInvocationEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub origin_runtime_incarnation_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

#[derive(Clone, Debug)]
pub struct CapabilityInvocationTransitionCommit {
    pub workspace_id: String,
    pub invocation_id: String,
    pub expected_version: u64,
    pub next_status: InvocationStatus,
    pub request_id: String,
    pub event: CapabilityInvocationEventContext,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedCapabilityInvocation {
    pub invocation: CapabilityInvocationRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// Internal transition writer. `DISPATCHED` is deliberately absent as a command: a
/// future combined Trust + Effect + ApprovalUse admission transaction must own it.
pub trait CapabilityInvocationStore: Send + Sync {
    fn transition_invocation(
        &self,
        commit: CapabilityInvocationTransitionCommit,
    ) -> Result<CommittedCapabilityInvocation, StoreError>;
}
