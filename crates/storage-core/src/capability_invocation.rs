//! Internal Core persistence port for already-admitted CapabilityInvocation lifecycle
//! observations, plus the combined Trust/Approval/Effect dispatch admission boundary.

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

/// Core Trust decision issued for one exact Invocation request.
///
/// The storage boundary re-checks every binding of this value against the
/// persisted rows before it starts an Effect. A decision for a different
/// request, grant, activation, scope or Effect never authorizes a dispatch.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrustDispatchDecision {
    /// `ALLOW` or `REQUIRE_APPROVAL`. `DENY` never authorizes a dispatch.
    pub decision: String,
    pub reason_code: String,
    pub policy_id: String,
    pub policy_revision: u32,
    pub policy_content_digest: String,
    /// Digest of the exact operation request this decision was issued for.
    pub request_digest: String,
    /// Digest of the exact capability/action/target/execution binding.
    pub action_digest: String,
    /// Digest of the Workspace and scope the decision was issued in.
    pub scope_digest: String,
    pub invocation_id: String,
    pub capability_grant_id: String,
    pub activation_id: String,
    pub effect_id: String,
}

/// Event identities for the three durable facts one admission transaction commits.
#[derive(Clone, Debug)]
pub struct CapabilityDispatchEvents {
    pub invocation: CapabilityInvocationEventContext,
    pub effect: crate::EffectEvidenceEventContext,
    pub approval_use: crate::EffectEvidenceEventContext,
}

/// Internal, Runtime-authenticated request to admit one consequential dispatch.
///
/// No Operator route exposes this command. The Broker is the only intended
/// caller, and it may dispatch a provider only after this transaction commits.
#[derive(Clone, Debug)]
pub struct AdmitCapabilityDispatchCommit {
    pub workspace_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub request_id: String,
    pub invocation_id: String,
    pub expected_invocation_version: u64,
    pub effect_id: String,
    pub expected_effect_version: u64,
    /// Approval whose single use this admission consumes for the Effect.
    pub approval_id: String,
    pub approval_use_id: String,
    pub trust_decision: TrustDispatchDecision,
    pub events: CapabilityDispatchEvents,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedCapabilityDispatch {
    pub invocation_id: String,
    pub invocation_version: u64,
    pub effect_id: String,
    pub effect_version: u64,
    pub effect_state: String,
    pub dispatch_ordinal: u32,
    pub approval_use_id: String,
    pub approval_id: String,
    pub events: Vec<DomainEvent>,
    pub replayed: bool,
}

/// The combined Trust + ApprovalUse + Invocation + Effect admission writer.
///
/// One authoritative transaction rechecks the live owner/lease, grant,
/// activation, exact Trust decision and exact Approval, then transitions the
/// Invocation, links and starts its Effect, appends the single `ApprovalUse`,
/// and writes the state snapshots, events, audit record and idempotency
/// receipt together. A rejected admission writes nothing.
pub trait CapabilityDispatchStore: Send + Sync {
    fn admit_capability_dispatch(
        &self,
        commit: AdmitCapabilityDispatchCommit,
    ) -> Result<CommittedCapabilityDispatch, StoreError>;
}
