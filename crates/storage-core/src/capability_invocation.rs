//! Internal Core persistence port for already-admitted CapabilityInvocation lifecycle
//! observations. It deliberately has no create or dispatch operation.

use crate::{DomainEvent, StoreError};
pub use domain_invocations::{CapabilityInvocationRecord, InvocationStatus};
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

/// Provenance claimed by the provider adapter that has authenticated the
/// external observation. The SQLite store verifies Runtime authority and the
/// bound provider identity, but does not itself authenticate provider responses.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InvocationProviderObservationSource {
    AuthenticatedCallback,
    AuthenticatedQuery,
}

/// Sanitized provider observation. Only a trusted adapter that has authenticated
/// provider state and matched it to the immutable request may submit this value.
/// No HTTP route exposes this command; it is an internal persistence port.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityInvocationProviderObservation {
    pub invocation_id: String,
    pub request_digest: String,
    pub provider_identity_digest: String,
    pub terminal_status: InvocationStatus,
    pub observed_at: String,
    pub source: InvocationProviderObservationSource,
}

#[derive(Clone, Debug)]
pub struct CapabilityInvocationReconciliationCommit {
    pub workspace_id: String,
    pub invocation_id: String,
    pub expected_version: u64,
    pub request_id: String,
    pub observation: CapabilityInvocationProviderObservation,
    pub event: CapabilityInvocationEventContext,
}

/// Owner-scoped keyset page over durable Invocation history for one Task.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityInvocationPageRequest {
    pub owner_principal_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub after_created_at: Option<String>,
    pub after_invocation_id: Option<String>,
    /// Storage accepts 1..=201 so the Operator can fetch one extra row to detect a
    /// continuation cursor while exposing at most 200 rows to clients.
    pub limit: usize,
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

    /// Persist an authenticated provider observation for an AMBIGUOUS record.
    /// Implementations must bind its invocation, request, and provider identity,
    /// then atomically commit state, event, and idempotency receipt.
    fn reconcile_terminal_invocation(
        &self,
        commit: CapabilityInvocationReconciliationCommit,
    ) -> Result<CommittedCapabilityInvocation, StoreError>;

    /// Read one Invocation only after checking the authenticated Workspace owner.
    fn get_invocation_for_owner(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        invocation_id: &str,
    ) -> Result<Option<CapabilityInvocationRecord>, StoreError>;

    /// Read a stable descending Task history page under owner/Workspace scope.
    fn list_task_invocations_for_owner(
        &self,
        request: CapabilityInvocationPageRequest,
    ) -> Result<Vec<CapabilityInvocationRecord>, StoreError>;
}
