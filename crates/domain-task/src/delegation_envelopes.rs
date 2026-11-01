//! Strict, bounded TaskPacket and ResultEnvelope contracts for hosted delegation.
//!
//! These values carry references, never authority. Their consumers must separately
//! resolve committed source events, authorize referenced content, and verify outputs.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const MAX_ENVELOPE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedResourceRef {
    pub workspace_id: String,
    pub resource_id: String,
    pub revision_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactVersionRef {
    pub workspace_id: String,
    pub artifact_id: String,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundedDecision {
    pub source_event_id: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationOutputRequirement {
    pub output_id: String,
    pub description: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub destination: Option<LooseResourceRef>,
    pub mandatory: bool,
}

/// An unpinned Resource reference is used only for an output destination contract.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LooseResourceRef {
    pub workspace_id: String,
    pub resource_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub revision_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceCriterionRef {
    pub criterion_id: String,
    pub description: String,
    #[serde(default)]
    pub subject_refs: Vec<LooseResourceRef>,
    pub required_evidence: RequiredDelegationEvidence,
    #[serde(default)]
    pub verifier_hint: Option<String>,
    pub mandatory: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RequiredDelegationEvidence {
    Reported,
    Observed,
    Verified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DelegationResultStatus {
    Completed,
    Failed,
    Blocked,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationBudgetSpec {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub max_wall_time_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub max_cost_minor_units: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub currency: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub max_tokens: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub max_child_attempts: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub max_concurrency: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DelegationBlockerSubject {
    Resource(LooseResourceRef),
    Service(DelegationServiceRef),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationServiceRef {
    pub service_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationBlocker {
    pub blocker_id: String,
    pub code: String,
    pub safe_message: String,
    #[serde(default)]
    pub subject_ref: Option<DelegationBlockerSubject>,
    pub resolution_hint: String,
    #[serde(default)]
    pub user_request_id: Option<String>,
    #[serde(default)]
    pub approval_id: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskPacket {
    pub task_id: String,
    pub step_id: String,
    pub parent_attempt_id: String,
    pub task_spec_revision: u64,
    pub plan_revision: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub workspace_instruction_revision: Option<u64>,
    pub delegation_profile_id: String,
    pub delegation_profile_revision: u64,
    pub objective: String,
    pub constraints: Vec<String>,
    pub input_refs: Vec<PinnedResourceRef>,
    pub artifact_refs: Vec<ArtifactVersionRef>,
    pub relevant_decisions: Vec<BoundedDecision>,
    pub required_output: Vec<DelegationOutputRequirement>,
    pub acceptance_criteria: Vec<AcceptanceCriterionRef>,
    pub capability_grant_refs: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub deadline: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub budget: Option<DelegationBudgetSpec>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultEnvelope {
    pub child_attempt_id: String,
    pub status: DelegationResultStatus,
    pub summary: String,
    pub output_refs: Vec<PinnedResourceRef>,
    pub artifact_refs: Vec<ArtifactVersionRef>,
    pub evidence_refs: Vec<String>,
    pub changed_resource_refs: Vec<PinnedResourceRef>,
    pub unresolved: Vec<String>,
    pub blockers: Vec<DelegationBlocker>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub usage: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub confidence: Option<f64>,
    pub verification_hints: Vec<String>,
    pub revision_at_start: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegationEnvelopeError {
    Invalid,
    TooLarge,
    WrongWorkspace,
    WrongChildAttempt,
    StaleTaskRevision,
}

impl std::fmt::Display for DelegationEnvelopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DelegationEnvelopeError {}

pub fn validate_task_packet(
    packet: &TaskPacket,
    workspace_id: &str,
) -> Result<(), DelegationEnvelopeError> {
    if !valid_id(workspace_id)
        || ![
            packet.task_id.as_str(),
            packet.step_id.as_str(),
            packet.parent_attempt_id.as_str(),
            packet.delegation_profile_id.as_str(),
        ]
        .into_iter()
        .all(valid_id)
        || packet.task_spec_revision == 0
        || packet.plan_revision == 0
        || packet.delegation_profile_revision == 0
        || packet.workspace_instruction_revision == Some(0)
        || packet.objective.trim().is_empty()
        || packet.objective.len() > 4000
        || packet.constraints.len() > 64
        || packet.input_refs.len() > 256
        || packet.artifact_refs.len() > 256
        || packet.relevant_decisions.len() > 64
        || packet.required_output.len() > 64
        || packet.acceptance_criteria.len() > 64
        || packet.capability_grant_refs.len() > 256
        || packet.constraints.iter().any(|value| value.len() > 1000)
        || !unique(&packet.capability_grant_refs)
        || packet
            .deadline
            .as_ref()
            .is_some_and(|value| !valid_timestamp(value))
        || packet.budget.as_ref().is_some_and(|budget| {
            budget.max_concurrency == Some(0)
                || (budget.max_cost_minor_units.is_some() && budget.currency.is_none())
                || budget.currency.as_ref().is_some_and(|currency| {
                    currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase())
                })
        })
    {
        return Err(DelegationEnvelopeError::Invalid);
    }
    if packet
        .input_refs
        .iter()
        .any(|reference| reference.workspace_id != workspace_id)
        || packet
            .artifact_refs
            .iter()
            .any(|reference| reference.workspace_id != workspace_id)
        || packet.required_output.iter().any(|output| {
            output
                .destination
                .as_ref()
                .is_some_and(|reference| reference.workspace_id != workspace_id)
        })
        || packet.acceptance_criteria.iter().any(|criterion| {
            criterion
                .subject_refs
                .iter()
                .any(|reference| reference.workspace_id != workspace_id)
        })
    {
        return Err(DelegationEnvelopeError::WrongWorkspace);
    }
    if packet
        .input_refs
        .iter()
        .any(|reference| !valid_id(&reference.resource_id) || !valid_id(&reference.revision_id))
        || packet
            .artifact_refs
            .iter()
            .any(|reference| !valid_id(&reference.artifact_id) || reference.version == 0)
        || packet.required_output.iter().any(|output| {
            !valid_id(&output.output_id)
                || output.description.trim().is_empty()
                || output.description.len() > 4000
                || output.kind.as_ref().is_some_and(|value| value.len() > 256)
                || output
                    .media_type
                    .as_ref()
                    .is_some_and(|value| value.len() > 256)
                || output.destination.as_ref().is_some_and(|reference| {
                    !valid_id(&reference.resource_id)
                        || reference
                            .revision_id
                            .as_ref()
                            .is_some_and(|revision_id| !valid_id(revision_id))
                })
        })
        || packet.acceptance_criteria.iter().any(|criterion| {
            !valid_id(&criterion.criterion_id)
                || criterion.description.trim().is_empty()
                || criterion.description.len() > 4000
                || criterion
                    .verifier_hint
                    .as_ref()
                    .is_some_and(|value| value.len() > 1000)
                || criterion.subject_refs.iter().any(|reference| {
                    !valid_id(&reference.resource_id)
                        || reference
                            .revision_id
                            .as_ref()
                            .is_some_and(|id| !valid_id(id))
                })
        })
        || packet.relevant_decisions.iter().any(|decision| {
            !valid_id(&decision.source_event_id)
                || decision.summary.trim().is_empty()
                || decision.summary.len() > 1000
        })
        || packet
            .capability_grant_refs
            .iter()
            .any(|reference| !valid_id(reference))
    {
        return Err(DelegationEnvelopeError::Invalid);
    }
    check_size(packet)
}

pub fn validate_result_envelope(
    result: &ResultEnvelope,
    expected_child_attempt_id: &str,
    expected_workspace_id: &str,
    expected_task_revision: u64,
) -> Result<(), DelegationEnvelopeError> {
    if !valid_id(expected_child_attempt_id)
        || !valid_id(expected_workspace_id)
        || !valid_id(&result.child_attempt_id)
        || result.summary.len() > 4000
        || result.output_refs.len() > 64
        || result.artifact_refs.len() > 64
        || result.evidence_refs.len() > 256
        || result.changed_resource_refs.len() > 256
        || result.unresolved.len() > 32
        || result.blockers.len() > 64
        || result
            .usage
            .as_ref()
            .is_some_and(|values| values.len() > 256)
        || result.verification_hints.len() > 64
        || result.revision_at_start == 0
        || result
            .confidence
            .is_some_and(|confidence| !confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
        || result
            .blockers
            .iter()
            .any(|blocker| !valid_timestamp(&blocker.created_at))
        || result.unresolved.iter().any(|value| value.len() > 1000)
        || result
            .verification_hints
            .iter()
            .any(|value| value.len() > 1000)
        || !unique(&result.evidence_refs)
        || result.usage.as_ref().is_some_and(|values| !unique(values))
    {
        return Err(DelegationEnvelopeError::Invalid);
    }
    if result.child_attempt_id != expected_child_attempt_id {
        return Err(DelegationEnvelopeError::WrongChildAttempt);
    }
    if result.revision_at_start != expected_task_revision {
        return Err(DelegationEnvelopeError::StaleTaskRevision);
    }
    if result
        .output_refs
        .iter()
        .chain(&result.changed_resource_refs)
        .any(|reference| reference.workspace_id != expected_workspace_id)
        || result
            .artifact_refs
            .iter()
            .any(|reference| reference.workspace_id != expected_workspace_id)
        || result.blockers.iter().any(|blocker| {
            blocker
                .subject_ref
                .as_ref()
                .is_some_and(|subject| match subject {
                    DelegationBlockerSubject::Resource(reference) => {
                        reference.workspace_id != expected_workspace_id
                    }
                    DelegationBlockerSubject::Service(_) => false,
                })
        })
    {
        return Err(DelegationEnvelopeError::WrongWorkspace);
    }
    if result
        .output_refs
        .iter()
        .chain(&result.changed_resource_refs)
        .any(|reference| !valid_id(&reference.resource_id) || !valid_id(&reference.revision_id))
        || result
            .artifact_refs
            .iter()
            .any(|reference| !valid_id(&reference.artifact_id) || reference.version == 0)
        || result
            .evidence_refs
            .iter()
            .any(|reference| !valid_id(reference))
        || result
            .usage
            .as_ref()
            .is_some_and(|values| values.iter().any(|reference| !valid_id(reference)))
        || result.blockers.iter().any(|blocker| {
            !valid_id(&blocker.blocker_id)
                || !valid_id(&blocker.code)
                || blocker.safe_message.trim().is_empty()
                || blocker.safe_message.len() > 1000
                || blocker.resolution_hint.trim().is_empty()
                || blocker.resolution_hint.len() > 1000
                || blocker
                    .user_request_id
                    .as_ref()
                    .is_some_and(|id| !valid_id(id))
                || blocker.approval_id.as_ref().is_some_and(|id| !valid_id(id))
                || blocker
                    .subject_ref
                    .as_ref()
                    .is_some_and(|subject| match subject {
                        DelegationBlockerSubject::Resource(reference) => {
                            !valid_id(&reference.resource_id)
                                || reference
                                    .revision_id
                                    .as_ref()
                                    .is_some_and(|id| !valid_id(id))
                        }
                        DelegationBlockerSubject::Service(reference) => {
                            !valid_id(&reference.service_id)
                        }
                    })
        })
    {
        return Err(DelegationEnvelopeError::Invalid);
    }
    check_size(result)
}

fn check_size<T: Serialize>(value: &T) -> Result<(), DelegationEnvelopeError> {
    let bytes =
        serde_json_canonicalizer::to_vec(value).map_err(|_| DelegationEnvelopeError::Invalid)?;
    if bytes.len() > MAX_ENVELOPE_BYTES {
        Err(DelegationEnvelopeError::TooLarge)
    } else {
        Ok(())
    }
}

fn deserialize_non_null_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn valid_timestamp(value: &str) -> bool {
    OffsetDateTime::parse(value, &Rfc3339).is_ok()
}

fn unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn packet() -> TaskPacket {
        TaskPacket {
            task_id: "task-1".into(),
            step_id: "step-1".into(),
            parent_attempt_id: "attempt-parent".into(),
            task_spec_revision: 1,
            plan_revision: 1,
            workspace_instruction_revision: None,
            delegation_profile_id: "profile-1".into(),
            delegation_profile_revision: 1,
            objective: "Inspect the requested files".into(),
            constraints: vec![],
            input_refs: vec![],
            artifact_refs: vec![],
            relevant_decisions: vec![],
            required_output: vec![],
            acceptance_criteria: vec![],
            capability_grant_refs: vec![],
            deadline: None,
            budget: None,
        }
    }

    fn result() -> ResultEnvelope {
        ResultEnvelope {
            child_attempt_id: "attempt-child".into(),
            status: DelegationResultStatus::Completed,
            summary: "Finished".into(),
            output_refs: vec![],
            artifact_refs: vec![],
            evidence_refs: vec![],
            changed_resource_refs: vec![],
            unresolved: vec![],
            blockers: vec![],
            usage: None,
            confidence: Some(0.8),
            verification_hints: vec![],
            revision_at_start: 3,
        }
    }

    #[test]
    fn validates_bounded_packet_and_result_contracts() {
        assert_eq!(validate_task_packet(&packet(), "workspace-1"), Ok(()));
        assert_eq!(
            validate_result_envelope(&result(), "attempt-child", "workspace-1", 3),
            Ok(())
        );
    }

    #[test]
    fn rejects_cross_workspace_references_and_duplicate_grants() {
        let mut value = packet();
        value.input_refs.push(PinnedResourceRef {
            workspace_id: "foreign-workspace".into(),
            resource_id: "resource-1".into(),
            revision_id: "revision-1".into(),
        });
        assert_eq!(
            validate_task_packet(&value, "workspace-1"),
            Err(DelegationEnvelopeError::WrongWorkspace)
        );
        let mut value = packet();
        value.capability_grant_refs = vec!["grant-1".into(), "grant-1".into()];
        assert_eq!(
            validate_task_packet(&value, "workspace-1"),
            Err(DelegationEnvelopeError::Invalid)
        );
    }

    #[test]
    fn result_is_bound_to_child_workspace_and_task_revision() {
        assert_eq!(
            validate_result_envelope(&result(), "other-child", "workspace-1", 3),
            Err(DelegationEnvelopeError::WrongChildAttempt)
        );
        assert_eq!(
            validate_result_envelope(&result(), "attempt-child", "workspace-1", 4),
            Err(DelegationEnvelopeError::StaleTaskRevision)
        );
        let mut value = result();
        value.output_refs.push(PinnedResourceRef {
            workspace_id: "foreign-workspace".into(),
            resource_id: "resource-1".into(),
            revision_id: "revision-1".into(),
        });
        assert_eq!(
            validate_result_envelope(&value, "attempt-child", "workspace-1", 3),
            Err(DelegationEnvelopeError::WrongWorkspace)
        );
    }

    #[test]
    fn serde_rejects_unknown_fields_and_schema_oversized_fields() {
        let mut value = serde_json::to_value(packet()).unwrap();
        value["execution_authority"] = Value::Bool(true);
        assert!(serde_json::from_value::<TaskPacket>(value).is_err());
        let mut value = serde_json::to_value(packet()).unwrap();
        value["deadline"] = Value::Null;
        assert!(serde_json::from_value::<TaskPacket>(value).is_err());
        let mut value = serde_json::to_value(packet()).unwrap();
        value["input_refs"] = serde_json::json!([{
            "workspace_id":"workspace-1", "resource_id":"resource-1", "revision_id":null
        }]);
        assert!(serde_json::from_value::<TaskPacket>(value).is_err());
        let mut value = serde_json::to_value(result()).unwrap();
        value["status"] = Value::String("SUCCEEDED".into());
        assert!(serde_json::from_value::<ResultEnvelope>(value).is_err());
        let mut value = packet();
        value.objective = "x".repeat(MAX_ENVELOPE_BYTES + 1);
        assert_eq!(
            validate_task_packet(&value, "workspace-1"),
            Err(DelegationEnvelopeError::Invalid)
        );
    }

    #[test]
    fn optional_packet_fields_are_omitted_and_deadlines_require_rfc3339() {
        let serialized = serde_json::to_value(packet()).unwrap();
        assert!(serialized.get("deadline").is_none());
        assert!(serialized.get("budget").is_none());
        let mut value = packet();
        value.deadline = Some("tomorrow".into());
        assert_eq!(
            validate_task_packet(&value, "workspace-1"),
            Err(DelegationEnvelopeError::Invalid)
        );
    }

    #[test]
    fn malformed_reference_ids_are_invalid_not_scope_conflicts() {
        let mut value = packet();
        value.input_refs.push(PinnedResourceRef {
            workspace_id: "workspace-1".into(),
            resource_id: "\n".into(),
            revision_id: "revision-1".into(),
        });
        assert_eq!(
            validate_task_packet(&value, "workspace-1"),
            Err(DelegationEnvelopeError::Invalid)
        );
    }

    #[test]
    fn rejects_malformed_optional_output_destination_revision() {
        let mut value = packet();
        value.required_output.push(DelegationOutputRequirement {
            output_id: "report".into(),
            description: "Create a report".into(),
            kind: None,
            media_type: None,
            destination: Some(LooseResourceRef {
                workspace_id: "workspace-1".into(),
                resource_id: "resource-1".into(),
                revision_id: Some("\n".into()),
            }),
            mandatory: true,
        });

        assert_eq!(
            validate_task_packet(&value, "workspace-1"),
            Err(DelegationEnvelopeError::Invalid)
        );
    }
}
