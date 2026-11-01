//! Persistence port for Core-owned Effects and append-only Evidence.
use crate::{DomainEvent, StoreError};
use domain_effects::{EffectRecord, EvidenceRecord};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Missing production prerequisites for the atomic Invocation/Effect dispatch
/// admission boundary. These are not policy decisions and do not grant authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchAdmissionBlocker {
    /// No current, exact-action Trust decision is issued to the storage writer.
    TrustDecisionUnavailable,
    /// There is no Invocation writer that atomically admits CREATED -> DISPATCHED.
    InvocationTransitionWriterUnavailable,
    /// Required ApprovalUse consumption is not part of the Effect start transaction.
    ApprovalUseConsumptionUnavailable,
}

impl DispatchAdmissionBlocker {
    /// Current prerequisites for any consequential Effect dispatch. Approval policy
    /// may later prove that an Approval is unnecessary, but that policy decision is
    /// not available here, so this storage boundary fails closed for every start.
    pub const fn current() -> [Self; 3] {
        [
            Self::TrustDecisionUnavailable,
            Self::InvocationTransitionWriterUnavailable,
            Self::ApprovalUseConsumptionUnavailable,
        ]
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectEvidenceEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub origin_runtime_incarnation_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

/// Authenticated private Runtime control supplies the live fence metadata. The
/// fencing credential itself is never represented in this type or durable storage.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectFenceBinding {
    pub lease_id: String,
    pub epoch: u64,
    pub fencing_token_digest: String,
}

#[derive(Clone, Debug)]
pub struct ProposeEffectCommit {
    pub workspace_id: String,
    /// Authenticated Runtime identity from the private control boundary.
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub request_id: String,
    pub fence: EffectFenceBinding,
    pub effect: EffectRecord,
    pub event: EffectEvidenceEventContext,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectTransitionMetadata {
    pub result_ref: Option<Value>,
    pub observed_state: Option<Value>,
    /// Exact immutable Evidence record that independently observed the Effect.
    pub observation_evidence_ref: Option<String>,
    pub verification_ref: Option<String>,
    pub retry_authorization: Option<EffectRetryAuthorization>,
    /// Safe, typed failure provenance. Raw provider/agent messages stay out of events.
    pub failure_code: Option<String>,
    pub failure_retryable: Option<bool>,
    pub failure_digest: Option<String>,
    /// Digest of the bounded ambiguity/reconciliation observation, not its raw text.
    pub ambiguity_reason_digest: Option<String>,
}

/// Internal reconciler decision; never deserialized from Operator/agent input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRetryAuthorization {
    pub evidence_id: String,
    pub basis: EffectRetryBasis,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EffectRetryBasis {
    ConfirmedNotOccurred,
    SameKeyIdempotent,
}

#[derive(Clone, Debug)]
pub struct TransitionEffectCommit {
    pub workspace_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub request_id: String,
    pub effect_id: String,
    pub expected_version: u64,
    pub next_state: domain_effects::EffectState,
    pub fence: Option<EffectFenceBinding>,
    pub metadata: EffectTransitionMetadata,
    pub event: EffectEvidenceEventContext,
}

#[derive(Clone, Debug)]
pub struct AppendEvidenceCommit {
    pub workspace_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub request_id: String,
    pub evidence: EvidenceRecord,
    pub event: EffectEvidenceEventContext,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedEffect {
    pub effect: EffectRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedEvidence {
    pub evidence: EvidenceRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// Internal Core persistence port. It creates no provider requests. Callers may invoke
/// external mutation only after `propose_effect` commits successfully, and only after
/// separate Trust admission succeeds.
pub trait EffectEvidenceStore: Send + Sync {
    fn propose_effect(&self, command: ProposeEffectCommit) -> Result<CommittedEffect, StoreError>;
    fn transition_effect(
        &self,
        command: TransitionEffectCommit,
    ) -> Result<CommittedEffect, StoreError>;
    fn append_evidence(
        &self,
        command: AppendEvidenceCommit,
    ) -> Result<CommittedEvidence, StoreError>;
    fn get_effect(
        &self,
        workspace_id: &str,
        effect_id: &str,
    ) -> Result<Option<EffectRecord>, StoreError>;
    fn list_effects(
        &self,
        workspace_id: &str,
        task_id: &str,
        limit: usize,
    ) -> Result<Vec<EffectRecord>, StoreError>;
    fn list_evidence(
        &self,
        workspace_id: &str,
        task_id: &str,
        limit: usize,
    ) -> Result<Vec<EvidenceRecord>, StoreError>;
}
