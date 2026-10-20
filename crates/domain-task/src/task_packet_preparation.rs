//! Prepare a canonical, bounded TaskPacket from caller-resolved durable projections.
//!
//! This helper carries only references, never referenced bytes or credentials. Its
//! allowsets are evidence from the caller's authoritative source reads; this module does
//! not grant access, resolve content, or replace Trust checks.

use crate::delegation_envelopes::DelegationEnvelopeError;
use crate::{
    ArtifactVersionRef, LooseResourceRef, PinnedResourceRef, TaskPacket, validate_task_packet,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use storage_core::{PlanRevisionRecord, StepRecord, TaskRecord, TaskSpecRevisionRecord};

const MAX_PACKET_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct TaskPacketSourcePins {
    pub workspace_id: String,
    pub task_id: String,
    pub step_id: String,
    pub parent_attempt_id: String,
    pub task_spec_revision: u64,
    pub plan_revision: u64,
    pub workspace_instruction_revision: Option<u64>,
    pub delegation_profile_id: String,
    pub delegation_profile_revision: u64,
    /// Exact input Resource revisions resolved and authorized by the caller.
    pub authorized_inputs: BTreeSet<PinnedResourceRef>,
    /// Exact Artifact versions independently resolved and authorized by the caller.
    pub authorized_artifacts: BTreeSet<ArtifactVersionRef>,
    /// Committed Task decision events in this Task's Conversation.
    pub committed_decision_event_ids: BTreeSet<String>,
    /// Grants already issued for this child scope; parent grants must not appear here.
    pub child_scoped_grant_ids: BTreeSet<String>,
    /// Resource references the caller permits in criteria/outputs.
    pub authorized_resource_refs: BTreeSet<LooseResourceRef>,
    /// Write destinations separately approved by the caller.
    pub authorized_output_destinations: BTreeSet<LooseResourceRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTaskPacket {
    pub packet: TaskPacket,
    pub canonical_json: Vec<u8>,
    pub canonical_digest: String,
}

/// Exact accepted aggregate rows that bound a newly prepared child TaskPacket.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptedTaskPacketContext<'a> {
    pub task: &'a TaskRecord,
    pub task_spec: &'a TaskSpecRevisionRecord,
    pub plan: &'a PlanRevisionRecord,
    pub step: &'a StepRecord,
    pub delegation_profile_id: &'a str,
    pub delegation_profile_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskPacketPreparationError {
    InvalidContextBudget,
    IdentityMismatch,
    UnauthorizedInputReference,
    UnauthorizedArtifactReference,
    UnauthorizedDecisionReference,
    UnauthorizedGrantReference,
    UnauthorizedResourceReference,
    UnauthorizedOutputDestination,
    AcceptedStepMismatch,
    ObjectiveDoesNotMatchAcceptedStep,
    ContextBudgetExceeded { limit: usize, actual: usize },
    InvalidEnvelope(DelegationEnvelopeError),
}

/// Connect preparation to exact durable Task/Plan/Step/Profile pins. Exact objective
/// equality is used as a conservative V1 narrowing rule because free-form prose cannot
/// be proven semantically narrower without another model call. References and outputs
/// are still checked against the resolver's authorized allowsets by `prepare_task_packet`.
pub fn prepare_task_packet_for_accepted_step(
    packet: TaskPacket,
    source: &TaskPacketSourcePins,
    accepted: &AcceptedTaskPacketContext<'_>,
    context_budget_bytes: usize,
) -> Result<PreparedTaskPacket, TaskPacketPreparationError> {
    let task = accepted.task;
    let spec = accepted.task_spec;
    let plan = accepted.plan;
    let step = accepted.step;
    let is_member = step.logical_key.as_deref().is_some_and(|key| {
        plan.steps.iter().any(|planned| {
            planned.logical_key == key
                && planned.objective == step.objective
                && planned.title == step.title
        })
    });
    if task.workspace_id != source.workspace_id
        || task.task_id != source.task_id
        || task.current_spec_revision != spec.revision
        || task.current_plan_revision != Some(plan.revision)
        || spec.workspace_id != task.workspace_id
        || spec.task_id != task.task_id
        || plan.task_id != task.task_id
        || plan.task_spec_revision != spec.revision
        || step.task_id != task.task_id
        || step.plan_revision != plan.revision
        || step.step_id != source.step_id
        || plan.revision != source.plan_revision
        || spec.revision != source.task_spec_revision
        || accepted.delegation_profile_id != source.delegation_profile_id
        || accepted.delegation_profile_revision != source.delegation_profile_revision
        || !is_member
    {
        return Err(TaskPacketPreparationError::AcceptedStepMismatch);
    }
    if packet.objective != step.objective {
        return Err(TaskPacketPreparationError::ObjectiveDoesNotMatchAcceptedStep);
    }
    prepare_task_packet(packet, source, context_budget_bytes)
}

/// Validates exact durable identity pins and caller-supplied authorization allowsets,
/// then emits canonical bytes only if they fit the Task/profile context budget. The
/// function never truncates, rewrites, or substitutes packet fields.
pub fn prepare_task_packet(
    packet: TaskPacket,
    source: &TaskPacketSourcePins,
    context_budget_bytes: usize,
) -> Result<PreparedTaskPacket, TaskPacketPreparationError> {
    if context_budget_bytes == 0 || context_budget_bytes > MAX_PACKET_BYTES {
        return Err(TaskPacketPreparationError::InvalidContextBudget);
    }
    if packet.workspace_instruction_revision != source.workspace_instruction_revision
        || packet
            .workspace_instruction_revision
            .is_some_and(|revision| revision == 0)
        || packet.task_id != source.task_id
        || packet.step_id != source.step_id
        || packet.parent_attempt_id != source.parent_attempt_id
        || packet.task_spec_revision != source.task_spec_revision
        || packet.plan_revision != source.plan_revision
        || packet.delegation_profile_id != source.delegation_profile_id
        || packet.delegation_profile_revision != source.delegation_profile_revision
        || source.task_spec_revision == 0
        || source.plan_revision == 0
        || source.delegation_profile_revision == 0
    {
        return Err(TaskPacketPreparationError::IdentityMismatch);
    }
    validate_task_packet(&packet, &source.workspace_id)
        .map_err(TaskPacketPreparationError::InvalidEnvelope)?;

    if packet
        .input_refs
        .iter()
        .any(|reference| !source.authorized_inputs.contains(reference))
    {
        return Err(TaskPacketPreparationError::UnauthorizedInputReference);
    }
    if packet
        .artifact_refs
        .iter()
        .any(|reference| !source.authorized_artifacts.contains(reference))
    {
        return Err(TaskPacketPreparationError::UnauthorizedArtifactReference);
    }
    if packet.relevant_decisions.iter().any(|decision| {
        !source
            .committed_decision_event_ids
            .contains(&decision.source_event_id)
    }) {
        return Err(TaskPacketPreparationError::UnauthorizedDecisionReference);
    }
    if packet
        .capability_grant_refs
        .iter()
        .any(|grant| !source.child_scoped_grant_ids.contains(grant))
    {
        return Err(TaskPacketPreparationError::UnauthorizedGrantReference);
    }
    if packet.acceptance_criteria.iter().any(|criterion| {
        criterion
            .subject_refs
            .iter()
            .any(|reference| !source.authorized_resource_refs.contains(reference))
    }) {
        return Err(TaskPacketPreparationError::UnauthorizedResourceReference);
    }
    if packet.required_output.iter().any(|output| {
        output
            .destination
            .as_ref()
            .is_some_and(|destination| !source.authorized_output_destinations.contains(destination))
    }) {
        return Err(TaskPacketPreparationError::UnauthorizedOutputDestination);
    }

    let canonical_json = serde_json_canonicalizer::to_vec(&packet).map_err(|_| {
        TaskPacketPreparationError::InvalidEnvelope(DelegationEnvelopeError::Invalid)
    })?;
    if canonical_json.len() > context_budget_bytes {
        return Err(TaskPacketPreparationError::ContextBudgetExceeded {
            limit: context_budget_bytes,
            actual: canonical_json.len(),
        });
    }
    let canonical_digest = format!("sha256:{:x}", Sha256::digest(&canonical_json));
    Ok(PreparedTaskPacket {
        packet,
        canonical_json,
        canonical_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AcceptanceCriterionRef, BoundedDecision, DelegationOutputRequirement,
        RequiredDelegationEvidence,
    };
    use serde_json::{Value, json};
    use storage_core::PlannedStepRecord;

    fn packet() -> TaskPacket {
        TaskPacket {
            task_id: "task-a".into(),
            step_id: "step-a".into(),
            parent_attempt_id: "attempt-parent".into(),
            task_spec_revision: 3,
            plan_revision: 2,
            workspace_instruction_revision: Some(5),
            delegation_profile_id: "profile-a".into(),
            delegation_profile_revision: 4,
            objective: "Create a brief summary".into(),
            constraints: vec!["Use the accepted inputs".into()],
            input_refs: vec![PinnedResourceRef {
                workspace_id: "workspace-a".into(),
                resource_id: "resource-a".into(),
                revision_id: "revision-a".into(),
            }],
            artifact_refs: vec![ArtifactVersionRef {
                workspace_id: "workspace-a".into(),
                artifact_id: "artifact-a".into(),
                version: 2,
            }],
            relevant_decisions: vec![BoundedDecision {
                source_event_id: "event-decision".into(),
                summary: "Keep the output concise".into(),
            }],
            required_output: vec![DelegationOutputRequirement {
                output_id: "summary".into(),
                description: "A concise summary".into(),
                kind: None,
                media_type: None,
                destination: Some(LooseResourceRef {
                    workspace_id: "workspace-a".into(),
                    resource_id: "resource-out".into(),
                    revision_id: None,
                }),
                mandatory: true,
            }],
            acceptance_criteria: vec![AcceptanceCriterionRef {
                criterion_id: "concise".into(),
                description: "Summary is under 100 words".into(),
                subject_refs: vec![LooseResourceRef {
                    workspace_id: "workspace-a".into(),
                    resource_id: "resource-a".into(),
                    revision_id: Some("revision-a".into()),
                }],
                required_evidence: RequiredDelegationEvidence::Observed,
                verifier_hint: None,
                mandatory: true,
            }],
            capability_grant_refs: vec!["grant-child".into()],
            deadline: None,
            budget: None,
        }
    }

    fn pins() -> TaskPacketSourcePins {
        TaskPacketSourcePins {
            workspace_id: "workspace-a".into(),
            task_id: "task-a".into(),
            step_id: "step-a".into(),
            parent_attempt_id: "attempt-parent".into(),
            task_spec_revision: 3,
            plan_revision: 2,
            workspace_instruction_revision: Some(5),
            delegation_profile_id: "profile-a".into(),
            delegation_profile_revision: 4,
            authorized_inputs: [PinnedResourceRef {
                workspace_id: "workspace-a".into(),
                resource_id: "resource-a".into(),
                revision_id: "revision-a".into(),
            }]
            .into_iter()
            .collect(),
            authorized_artifacts: [ArtifactVersionRef {
                workspace_id: "workspace-a".into(),
                artifact_id: "artifact-a".into(),
                version: 2,
            }]
            .into_iter()
            .collect(),
            committed_decision_event_ids: ["event-decision".into()].into_iter().collect(),
            child_scoped_grant_ids: ["grant-child".into()].into_iter().collect(),
            authorized_resource_refs: [LooseResourceRef {
                workspace_id: "workspace-a".into(),
                resource_id: "resource-a".into(),
                revision_id: Some("revision-a".into()),
            }]
            .into_iter()
            .collect(),
            authorized_output_destinations: [LooseResourceRef {
                workspace_id: "workspace-a".into(),
                resource_id: "resource-out".into(),
                revision_id: None,
            }]
            .into_iter()
            .collect(),
        }
    }

    fn accepted_rows() -> (
        TaskRecord,
        TaskSpecRevisionRecord,
        PlanRevisionRecord,
        StepRecord,
    ) {
        let task = TaskRecord {
            task_id: "task-a".into(),
            workspace_id: "workspace-a".into(),
            conversation_id: None,
            current_spec_revision: 3,
            current_plan_revision: Some(2),
            status: "RUNNING".into(),
            resume_status: None,
            routine_id: None,
            routine_revision: None,
            automation_id: None,
            automation_occurrence_id: None,
            origin_coworker_id: None,
            origin_coworker_revision: None,
            lead_agent_binding_id: "binding-lead".into(),
            blocking_conditions: vec![],
            priority: "NORMAL".into(),
            created_by: json!({"kind":"USER","principal_id":"owner"}),
            created_at: "2026-10-17T10:00:00Z".into(),
            updated_at: "2026-10-17T10:00:00Z".into(),
            completed_at: None,
            version: 7,
        };
        let spec = TaskSpecRevisionRecord {
            task_id: "task-a".into(),
            workspace_id: "workspace-a".into(),
            revision: 3,
            parent_revisions: vec![2],
            objective: "Summarize the selected source".into(),
            task_category: None,
            constraints: vec![],
            non_goals: vec![],
            input_refs: vec![],
            workspace_instruction_revision: None,
            required_outputs: vec![],
            acceptance_criteria: vec![],
            approvals_required: vec![],
            budget: None,
            delegation_budget_policy: None,
            lead_failover_policy: Value::Null,
            deadline: None,
            source_message_refs: vec![],
            placement_preference: json!({}),
            preferred_lead_agent_binding_id: None,
            authored_by: json!({"kind":"USER","principal_id":"owner"}),
            created_at: "2026-10-17T10:00:00Z".into(),
        };
        let plan = PlanRevisionRecord {
            task_id: "task-a".into(),
            revision: 2,
            task_spec_revision: 3,
            produced_by_agent_session_id: "planning-session".into(),
            produced_by_attempt_id: None,
            steps: vec![PlannedStepRecord {
                logical_key: "summary".into(),
                title: "Summarize source".into(),
                objective: "Create a brief summary".into(),
                depends_on_logical_keys: vec![],
                required_capabilities: vec![],
                acceptance_criteria: vec![],
            }],
            reason_for_revision: None,
            created_at: "2026-10-17T10:00:00Z".into(),
        };
        let step = StepRecord {
            step_id: "step-a".into(),
            task_id: "task-a".into(),
            plan_revision: 2,
            logical_key: Some("summary".into()),
            title: "Summarize source".into(),
            objective: "Create a brief summary".into(),
            dependencies: vec![],
            required_capabilities: vec![],
            acceptance_criteria: vec![],
            status: "READY".into(),
            current_attempt_id: None,
            created_at: "2026-10-17T10:00:00Z".into(),
            updated_at: "2026-10-17T10:00:00Z".into(),
            version: 1,
        };
        (task, spec, plan, step)
    }

    #[test]
    fn accepted_step_producer_pins_exact_task_plan_step_and_objective() {
        let (task, spec, plan, step) = accepted_rows();
        let accepted = AcceptedTaskPacketContext {
            task: &task,
            task_spec: &spec,
            plan: &plan,
            step: &step,
            delegation_profile_id: "profile-a",
            delegation_profile_revision: 4,
        };
        assert!(
            prepare_task_packet_for_accepted_step(packet(), &pins(), &accepted, 64 * 1024).is_ok()
        );

        let mut widened = packet();
        widened.objective = "Also modify the source and publish the result".into();
        assert_eq!(
            prepare_task_packet_for_accepted_step(widened, &pins(), &accepted, 64 * 1024),
            Err(TaskPacketPreparationError::ObjectiveDoesNotMatchAcceptedStep)
        );

        let mut stale_step = step.clone();
        stale_step.plan_revision = 1;
        let stale = AcceptedTaskPacketContext {
            task: &task,
            task_spec: &spec,
            plan: &plan,
            step: &stale_step,
            delegation_profile_id: "profile-a",
            delegation_profile_revision: 4,
        };
        assert_eq!(
            prepare_task_packet_for_accepted_step(packet(), &pins(), &stale, 64 * 1024),
            Err(TaskPacketPreparationError::AcceptedStepMismatch)
        );
    }

    #[test]
    fn prepares_exact_pinned_packet_as_canonical_bytes_with_digest() {
        let prepared = prepare_task_packet(packet(), &pins(), MAX_PACKET_BYTES)
            .expect("authorized bounded packet");
        assert_eq!(
            prepared.canonical_json.len(),
            serde_json_canonicalizer::to_vec(&prepared.packet)
                .unwrap()
                .len()
        );
        assert!(prepared.canonical_digest.starts_with("sha256:"));
    }

    #[test]
    fn rejects_every_changed_durable_identity_pin() {
        let mut value = packet();
        value.parent_attempt_id = "other-parent".into();
        assert_eq!(
            prepare_task_packet(value, &pins(), MAX_PACKET_BYTES),
            Err(TaskPacketPreparationError::IdentityMismatch)
        );

        let mut value = packet();
        value.delegation_profile_revision += 1;
        assert_eq!(
            prepare_task_packet(value, &pins(), MAX_PACKET_BYTES),
            Err(TaskPacketPreparationError::IdentityMismatch)
        );
    }

    #[test]
    fn rejects_unapproved_input_artifact_decision_grant_and_output_refs() {
        let mut value = packet();
        value.input_refs[0].revision_id = "unapproved-revision".into();
        assert_eq!(
            prepare_task_packet(value, &pins(), MAX_PACKET_BYTES),
            Err(TaskPacketPreparationError::UnauthorizedInputReference)
        );

        let mut value = packet();
        value.artifact_refs[0].version += 1;
        assert_eq!(
            prepare_task_packet(value, &pins(), MAX_PACKET_BYTES),
            Err(TaskPacketPreparationError::UnauthorizedArtifactReference)
        );

        let mut value = packet();
        value.relevant_decisions[0].source_event_id = "uncommitted".into();
        assert_eq!(
            prepare_task_packet(value, &pins(), MAX_PACKET_BYTES),
            Err(TaskPacketPreparationError::UnauthorizedDecisionReference)
        );

        let mut value = packet();
        value.capability_grant_refs[0] = "parent-grant".into();
        assert_eq!(
            prepare_task_packet(value, &pins(), MAX_PACKET_BYTES),
            Err(TaskPacketPreparationError::UnauthorizedGrantReference)
        );

        let mut value = packet();
        value.acceptance_criteria[0].subject_refs[0].resource_id = "other".into();
        assert_eq!(
            prepare_task_packet(value, &pins(), MAX_PACKET_BYTES),
            Err(TaskPacketPreparationError::UnauthorizedResourceReference)
        );

        let mut value = packet();
        value.required_output[0]
            .destination
            .as_mut()
            .unwrap()
            .resource_id = "other".into();
        assert_eq!(
            prepare_task_packet(value, &pins(), MAX_PACKET_BYTES),
            Err(TaskPacketPreparationError::UnauthorizedOutputDestination)
        );
    }

    #[test]
    fn rejects_packet_over_context_budget_without_truncating() {
        let packet = packet();
        let too_small = serde_json_canonicalizer::to_vec(&packet).unwrap().len() - 1;
        assert!(matches!(
            prepare_task_packet(packet, &pins(), too_small),
            Err(TaskPacketPreparationError::ContextBudgetExceeded { .. })
        ));
    }
}
