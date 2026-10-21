//! Bounded, portable handoff projection for a Task lead change.
//!
//! `DELEGATION.md` requires a Task lead change to create "a bounded handoff projection
//! from TaskSpec, Plan, Steps, decisions, Artifacts, Evidence, open Effects, blockers,
//! remaining budget, and reason. It contains no hidden reasoning, full native
//! transcript, or provider handle. It does not reparent active child Attempts."
//!
//! This module builds that projection from durable state and caller-resolved pins. It
//! carries references and bounded summaries only: a native transcript, hidden
//! reasoning, a native session handle, a provider handle, or a secret has no field here
//! and therefore cannot be carried. The function never truncates; a projection that
//! exceeds its context budget is rejected instead of being shortened.

use crate::{ArtifactVersionRef, BoundedDecision, DelegationBlocker, DelegationBudgetSpec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Maximum canonical size of one handoff projection.
const MAX_PACKET_BYTES: usize = 1024 * 1024;

const MAX_OBJECTIVE_BYTES: usize = 4000;
const MAX_CONSTRAINTS: usize = 64;
const MAX_CONSTRAINT_BYTES: usize = 1000;
const MAX_STEPS: usize = 256;
const MAX_STEP_FIELD_BYTES: usize = 1000;
const MAX_DECISIONS: usize = 64;
const MAX_DECISION_SUMMARY_BYTES: usize = 1000;
const MAX_ARTIFACTS: usize = 256;
const MAX_EVIDENCE: usize = 256;
const MAX_EFFECTS: usize = 256;
const MAX_BLOCKERS: usize = 64;
const MAX_REASON_BYTES: usize = 1000;

/// One accepted Step, reduced to the bounded facts a replacement lead may read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeadHandoffStep {
    pub step_id: String,
    pub logical_key: Option<String>,
    pub title: String,
    pub objective: String,
    pub status: String,
}

/// The portable projection handed to a replacement lead.
///
/// Every field is a reference or a bounded summary of durable state. No field can
/// carry a transcript, hidden reasoning, a native session handle, a provider handle, or
/// a secret.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeadHandoffPacket {
    pub workspace_id: String,
    pub task_id: String,
    pub task_spec_revision: u64,
    /// The accepted PlanRevision the replacement lead continues, when the Task already
    /// has one. A lead change before planning carries no plan.
    pub plan_revision: Option<u64>,
    pub objective: String,
    pub constraints: Vec<String>,
    pub steps: Vec<LeadHandoffStep>,
    pub relevant_decisions: Vec<BoundedDecision>,
    pub artifact_refs: Vec<ArtifactVersionRef>,
    pub evidence_refs: Vec<String>,
    pub open_effect_refs: Vec<String>,
    pub blockers: Vec<DelegationBlocker>,
    pub remaining_budget: Option<DelegationBudgetSpec>,
    pub reason: String,
}

/// Exact durable identity the projection was derived from, plus the caller's
/// authorized reference allowsets.
#[derive(Clone, Debug, PartialEq)]
pub struct LeadHandoffSourcePins {
    pub workspace_id: String,
    pub task_id: String,
    pub task_spec_revision: u64,
    pub plan_revision: Option<u64>,
    /// Artifact versions the caller independently resolved and authorized.
    pub authorized_artifacts: BTreeSet<ArtifactVersionRef>,
    /// Committed decision event IDs the caller independently resolved.
    pub committed_decision_event_ids: BTreeSet<String>,
    /// Evidence IDs the caller independently resolved.
    pub authorized_evidence_ids: BTreeSet<String>,
    /// Open Effect IDs the caller independently resolved.
    pub open_effect_ids: BTreeSet<String>,
}

/// A validated projection with its canonical bytes and digest.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedLeadHandoffPacket {
    pub packet: LeadHandoffPacket,
    pub canonical_json: Vec<u8>,
    pub canonical_digest: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LeadHandoffPacketError {
    InvalidContextBudget,
    IdentityMismatch,
    UnauthorizedArtifactReference,
    UnauthorizedDecisionReference,
    UnauthorizedEvidenceReference,
    UnauthorizedEffectReference,
    InvalidContent(&'static str),
    ContextBudgetExceeded { limit: usize, actual: usize },
}

/// Validate the exact durable identity pins and the caller's authorized reference
/// allowsets, then emit canonical bytes only when the projection fits its context
/// budget. The function never truncates, rewrites, or substitutes packet fields.
pub fn prepare_lead_handoff_packet(
    packet: LeadHandoffPacket,
    source: &LeadHandoffSourcePins,
    context_budget_bytes: usize,
) -> Result<PreparedLeadHandoffPacket, LeadHandoffPacketError> {
    if context_budget_bytes == 0 || context_budget_bytes > MAX_PACKET_BYTES {
        return Err(LeadHandoffPacketError::InvalidContextBudget);
    }
    if packet.workspace_id != source.workspace_id
        || packet.task_id != source.task_id
        || packet.task_spec_revision != source.task_spec_revision
        || packet.plan_revision != source.plan_revision
        || source.task_spec_revision == 0
    {
        return Err(LeadHandoffPacketError::IdentityMismatch);
    }
    if packet
        .artifact_refs
        .iter()
        .any(|reference| !source.authorized_artifacts.contains(reference))
    {
        return Err(LeadHandoffPacketError::UnauthorizedArtifactReference);
    }
    if packet.relevant_decisions.iter().any(|decision| {
        !source
            .committed_decision_event_ids
            .contains(&decision.source_event_id)
    }) {
        return Err(LeadHandoffPacketError::UnauthorizedDecisionReference);
    }
    if packet
        .evidence_refs
        .iter()
        .any(|reference| !source.authorized_evidence_ids.contains(reference))
    {
        return Err(LeadHandoffPacketError::UnauthorizedEvidenceReference);
    }
    if packet
        .open_effect_refs
        .iter()
        .any(|reference| !source.open_effect_ids.contains(reference))
    {
        return Err(LeadHandoffPacketError::UnauthorizedEffectReference);
    }
    validate_content(&packet)?;

    let canonical_json = serde_json_canonicalizer::to_vec(&packet).map_err(|_| {
        LeadHandoffPacketError::InvalidContent("handoff projection is not canonicalizable")
    })?;
    if canonical_json.len() > context_budget_bytes {
        return Err(LeadHandoffPacketError::ContextBudgetExceeded {
            limit: context_budget_bytes,
            actual: canonical_json.len(),
        });
    }
    let canonical_digest = format!("sha256:{:x}", Sha256::digest(&canonical_json));
    Ok(PreparedLeadHandoffPacket {
        packet,
        canonical_json,
        canonical_digest,
    })
}

fn validate_content(packet: &LeadHandoffPacket) -> Result<(), LeadHandoffPacketError> {
    if !valid_id(&packet.workspace_id)
        || !valid_id(&packet.task_id)
        || packet.objective.trim().is_empty()
        || packet.objective.len() > MAX_OBJECTIVE_BYTES
        || packet.constraints.len() > MAX_CONSTRAINTS
        || packet
            .constraints
            .iter()
            .any(|value| value.len() > MAX_CONSTRAINT_BYTES)
        || packet.steps.len() > MAX_STEPS
        || packet.relevant_decisions.len() > MAX_DECISIONS
        || packet.artifact_refs.len() > MAX_ARTIFACTS
        || packet.evidence_refs.len() > MAX_EVIDENCE
        || packet.open_effect_refs.len() > MAX_EFFECTS
        || packet.blockers.len() > MAX_BLOCKERS
        || packet.reason.trim().is_empty()
        || packet.reason.len() > MAX_REASON_BYTES
    {
        return Err(LeadHandoffPacketError::InvalidContent(
            "handoff projection exceeds its bounded content limits",
        ));
    }
    if packet.steps.iter().any(|step| {
        !valid_id(&step.step_id)
            || step
                .logical_key
                .as_deref()
                .is_some_and(|key| !valid_text(key))
            || step.title.len() > MAX_STEP_FIELD_BYTES
            || step.objective.len() > MAX_STEP_FIELD_BYTES
            || step.status.trim().is_empty()
    }) {
        return Err(LeadHandoffPacketError::InvalidContent(
            "handoff projection contains an invalid Step",
        ));
    }
    if packet.relevant_decisions.iter().any(|decision| {
        !valid_id(&decision.source_event_id) || decision.summary.len() > MAX_DECISION_SUMMARY_BYTES
    }) {
        return Err(LeadHandoffPacketError::InvalidContent(
            "handoff projection contains an invalid decision",
        ));
    }
    if packet
        .evidence_refs
        .iter()
        .chain(packet.open_effect_refs.iter())
        .any(|reference| !valid_id(reference))
    {
        return Err(LeadHandoffPacketError::InvalidContent(
            "handoff projection contains an invalid reference",
        ));
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn packet() -> LeadHandoffPacket {
        LeadHandoffPacket {
            workspace_id: "workspace-a".into(),
            task_id: "task-a".into(),
            task_spec_revision: 3,
            plan_revision: Some(2),
            objective: "Continue the accepted plan".into(),
            constraints: vec!["Keep the accepted inputs".into()],
            steps: vec![LeadHandoffStep {
                step_id: "step-a".into(),
                logical_key: Some("scan".into()),
                title: "Scan the repository".into(),
                objective: "Find the affected files".into(),
                status: "RUNNING".into(),
            }],
            relevant_decisions: vec![BoundedDecision {
                source_event_id: "event-decision".into(),
                summary: "Keep the output concise".into(),
            }],
            artifact_refs: vec![ArtifactVersionRef {
                workspace_id: "workspace-a".into(),
                artifact_id: "artifact-a".into(),
                version: 2,
            }],
            evidence_refs: vec!["evidence-a".into()],
            open_effect_refs: vec!["effect-a".into()],
            blockers: vec![DelegationBlocker {
                blocker_id: "blocker-a".into(),
                code: "AGENT_QUOTA_EXHAUSTED".into(),
                safe_message: "The lead agent quota is exhausted.".into(),
                subject_ref: None,
                resolution_hint: "Choose a different lead".into(),
                user_request_id: None,
                approval_id: None,
                created_at: "2026-10-17T12:00:00Z".into(),
            }],
            remaining_budget: None,
            reason: "The pinned lead reported a quota exhaustion.".into(),
        }
    }

    fn pins() -> LeadHandoffSourcePins {
        LeadHandoffSourcePins {
            workspace_id: "workspace-a".into(),
            task_id: "task-a".into(),
            task_spec_revision: 3,
            plan_revision: Some(2),
            authorized_artifacts: BTreeSet::from([ArtifactVersionRef {
                workspace_id: "workspace-a".into(),
                artifact_id: "artifact-a".into(),
                version: 2,
            }]),
            committed_decision_event_ids: BTreeSet::from(["event-decision".to_owned()]),
            authorized_evidence_ids: BTreeSet::from(["evidence-a".to_owned()]),
            open_effect_ids: BTreeSet::from(["effect-a".to_owned()]),
        }
    }

    #[test]
    fn projection_carries_only_references_and_bounded_summaries() {
        let prepared = prepare_lead_handoff_packet(packet(), &pins(), 64 * 1024)
            .expect("a pinned projection prepares");
        assert_eq!(
            prepared.canonical_digest,
            format!("sha256:{:x}", Sha256::digest(&prepared.canonical_json))
        );
        let value: serde_json::Value =
            serde_json::from_slice(&prepared.canonical_json).expect("canonical JSON");
        // No transcript, reasoning, session handle, provider handle, or secret field
        // exists in the projection at all.
        let keys: Vec<&str> = value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        for forbidden in [
            "transcript",
            "reasoning",
            "session_handle",
            "provider_handle",
            "secret",
            "native_session_id",
        ] {
            assert!(!keys.contains(&forbidden), "unexpected field {forbidden}");
        }
        assert_eq!(value["task_spec_revision"], json!(3));
        assert_eq!(value["plan_revision"], json!(2));
        assert_eq!(value["steps"][0]["step_id"], json!("step-a"));
    }

    #[test]
    fn a_lead_change_before_planning_carries_no_plan_or_steps() {
        let mut unplanned = packet();
        unplanned.plan_revision = None;
        unplanned.steps = vec![];
        let mut unplanned_pins = pins();
        unplanned_pins.plan_revision = None;
        let prepared = prepare_lead_handoff_packet(unplanned, &unplanned_pins, 64 * 1024)
            .expect("a lead change before planning prepares without a plan");
        let value: serde_json::Value =
            serde_json::from_slice(&prepared.canonical_json).expect("canonical JSON");
        assert_eq!(value["plan_revision"], json!(null));
        assert_eq!(value["steps"], json!([]));
    }

    #[test]
    fn a_changed_durable_identity_is_rejected() {
        let mut changed = pins();
        changed.task_spec_revision = 4;
        assert_eq!(
            prepare_lead_handoff_packet(packet(), &changed, 64 * 1024),
            Err(LeadHandoffPacketError::IdentityMismatch)
        );
        let mut changed = pins();
        changed.plan_revision = Some(9);
        assert_eq!(
            prepare_lead_handoff_packet(packet(), &changed, 64 * 1024),
            Err(LeadHandoffPacketError::IdentityMismatch)
        );
    }

    #[test]
    fn every_reference_must_be_in_the_caller_authorized_set() {
        let mut widened = packet();
        widened.artifact_refs[0].artifact_id = "artifact-other".into();
        assert_eq!(
            prepare_lead_handoff_packet(widened, &pins(), 64 * 1024),
            Err(LeadHandoffPacketError::UnauthorizedArtifactReference)
        );

        let mut widened = packet();
        widened.relevant_decisions[0].source_event_id = "event-other".into();
        assert_eq!(
            prepare_lead_handoff_packet(widened, &pins(), 64 * 1024),
            Err(LeadHandoffPacketError::UnauthorizedDecisionReference)
        );

        let mut widened = packet();
        widened.evidence_refs[0] = "evidence-other".into();
        assert_eq!(
            prepare_lead_handoff_packet(widened, &pins(), 64 * 1024),
            Err(LeadHandoffPacketError::UnauthorizedEvidenceReference)
        );

        let mut widened = packet();
        widened.open_effect_refs[0] = "effect-other".into();
        assert_eq!(
            prepare_lead_handoff_packet(widened, &pins(), 64 * 1024),
            Err(LeadHandoffPacketError::UnauthorizedEffectReference)
        );
    }

    #[test]
    fn an_oversized_projection_is_rejected_and_never_truncated() {
        let mut large = packet();
        large.objective = "x".repeat(MAX_OBJECTIVE_BYTES);
        let error = prepare_lead_handoff_packet(large, &pins(), 1024)
            .expect_err("a projection over its budget is rejected");
        assert!(matches!(
            error,
            LeadHandoffPacketError::ContextBudgetExceeded { .. }
        ));
    }

    #[test]
    fn unbounded_or_empty_content_is_rejected() {
        let mut empty_reason = packet();
        empty_reason.reason = "   ".into();
        assert_eq!(
            prepare_lead_handoff_packet(empty_reason, &pins(), 64 * 1024),
            Err(LeadHandoffPacketError::InvalidContent(
                "handoff projection exceeds its bounded content limits"
            ))
        );

        let mut too_many = packet();
        too_many.steps = (0..=MAX_STEPS)
            .map(|index| LeadHandoffStep {
                step_id: format!("step-{index}"),
                logical_key: None,
                title: "Step".into(),
                objective: "Objective".into(),
                status: "READY".into(),
            })
            .collect();
        assert_eq!(
            prepare_lead_handoff_packet(too_many, &pins(), MAX_PACKET_BYTES),
            Err(LeadHandoffPacketError::InvalidContent(
                "handoff projection exceeds its bounded content limits"
            ))
        );
    }

    #[test]
    fn an_invalid_context_budget_is_rejected() {
        assert_eq!(
            prepare_lead_handoff_packet(packet(), &pins(), 0),
            Err(LeadHandoffPacketError::InvalidContextBudget)
        );
        assert_eq!(
            prepare_lead_handoff_packet(packet(), &pins(), MAX_PACKET_BYTES + 1),
            Err(LeadHandoffPacketError::InvalidContextBudget)
        );
    }
}
