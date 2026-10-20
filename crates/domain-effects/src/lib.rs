//! Core-owned Effect and Evidence values.
//!
//! This crate defines domain validation and transitions only. It does not authorize
//! provider calls, dispatch capabilities, issue grants, or reconcile external state.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_VERIFICATION_SUBJECTS: usize = 128;
const MAX_VERIFICATION_INPUTS: usize = 128;
const MAX_VERIFICATION_EVIDENCE_REFS: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EffectState {
    Proposed,
    Started,
    Acknowledged,
    Reconciling,
    Observed,
    Verified,
    Failed,
    Ambiguous,
}

impl EffectState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "PROPOSED",
            Self::Started => "STARTED",
            Self::Acknowledged => "ACKNOWLEDGED",
            Self::Reconciling => "RECONCILING",
            Self::Observed => "OBSERVED",
            Self::Verified => "VERIFIED",
            Self::Failed => "FAILED",
            Self::Ambiguous => "AMBIGUOUS",
        }
    }

    pub const fn allows(self, next: Self) -> bool {
        use EffectState::*;
        matches!(
            (self, next),
            (Proposed, Started | Failed)
                | (Started, Acknowledged | Failed | Ambiguous)
                | (Acknowledged, Observed | Failed | Ambiguous)
                | (Observed, Verified | Ambiguous)
                | (Ambiguous, Reconciling)
                | (Reconciling, Observed | Failed | Ambiguous | Started)
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceLevel {
    Reported,
    Observed,
    Verified,
}

impl EvidenceLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reported => "REPORTED",
            Self::Observed => "OBSERVED",
            Self::Verified => "VERIFIED",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionMethod {
    StructuredApi,
    StructuredBrowser,
    AccessibilityBrowser,
    ScreenComputerUse,
    DeterministicLocal,
    NativeAgentTool,
    Unknown,
}

impl ExecutionMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StructuredApi => "STRUCTURED_API",
            Self::StructuredBrowser => "STRUCTURED_BROWSER",
            Self::AccessibilityBrowser => "ACCESSIBILITY_BROWSER",
            Self::ScreenComputerUse => "SCREEN_COMPUTER_USE",
            Self::DeterministicLocal => "DETERMINISTIC_LOCAL",
            Self::NativeAgentTool => "NATIVE_AGENT_TOOL",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRecord {
    pub effect_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub capability_ref: Option<Value>,
    pub operation: String,
    /// A `ResourceRef` object or an opaque, non-empty provider target string.
    pub target: Value,
    pub idempotency_key: Option<String>,
    pub state: EffectState,
    pub request_digest: String,
    pub capability_invocation_id: String,
    pub execution_method: ExecutionMethod,
    pub dispatch_ordinal: u32,
    pub result_ref: Option<Value>,
    pub observed_state: Option<Value>,
    pub verification_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

impl EffectRecord {
    pub fn validate(&self) -> Result<(), EffectError> {
        for value in [
            self.effect_id.as_str(),
            self.task_id.as_str(),
            self.attempt_id.as_str(),
            self.operation.as_str(),
            self.capability_invocation_id.as_str(),
            self.created_at.as_str(),
            self.updated_at.as_str(),
        ] {
            if value.trim().is_empty() || value.len() > 4096 || value.chars().any(char::is_control)
            {
                return Err(EffectError::InvalidRecord);
            }
        }
        if !valid_digest(&self.request_digest)
            || self.version == 0
            || self.dispatch_ordinal > 1_000_000
        {
            return Err(EffectError::InvalidRecord);
        }
        if self
            .verification_ref
            .as_ref()
            .is_some_and(|reference| invalid_bounded(reference, 256))
            || (self.state == EffectState::Verified && self.verification_ref.is_none())
        {
            return Err(EffectError::InvalidRecord);
        }
        if self.operation.len() > 512
            || self
                .idempotency_key
                .as_ref()
                .is_some_and(|key| invalid_bounded(key, 1024))
        {
            return Err(EffectError::InvalidRecord);
        }
        if !valid_target(&self.target)
            || !self.capability_ref.as_ref().is_none_or(Value::is_object)
            || !self.result_ref.as_ref().is_none_or(Value::is_object)
            || !self.observed_state.as_ref().is_none_or(Value::is_object)
        {
            return Err(EffectError::InvalidRecord);
        }
        if (self.state == EffectState::Proposed && self.dispatch_ordinal != 0)
            || (!matches!(self.state, EffectState::Proposed | EffectState::Failed)
                && self.dispatch_ordinal == 0)
        {
            return Err(EffectError::InvalidRecord);
        }
        Ok(())
    }

    pub fn transition(&self, next: EffectState, at: String) -> Result<Self, EffectError> {
        if !self.state.allows(next) {
            return Err(EffectError::InvalidTransition);
        }
        if self.state == EffectState::Reconciling && next == EffectState::Started {
            return Err(EffectError::RetryNotAuthorized);
        }
        self.transition_validated(next, at)
    }

    /// Return to STARTED only after a persisted reconciliation decision proves the
    /// prior dispatch did not occur or a stable provider idempotency key makes retry safe.
    pub fn authorize_retry(
        &self,
        at: String,
        reconciliation_evidence_ref: &str,
        confirmed_not_occurred: bool,
        same_key_idempotent: bool,
    ) -> Result<Self, EffectError> {
        if self.state != EffectState::Reconciling || reconciliation_evidence_ref.trim().is_empty() {
            return Err(EffectError::RetryNotAuthorized);
        }
        if !confirmed_not_occurred && !(same_key_idempotent && self.idempotency_key.is_some()) {
            return Err(EffectError::RetryNotAuthorized);
        }
        self.transition_validated(EffectState::Started, at)
    }

    fn transition_validated(&self, next: EffectState, at: String) -> Result<Self, EffectError> {
        if invalid_bounded(&at, 128) {
            return Err(EffectError::InvalidRecord);
        }
        let mut updated = self.clone();
        updated.state = next;
        updated.updated_at = at;
        updated.version = self
            .version
            .checked_add(1)
            .ok_or(EffectError::VersionOverflow)?;
        if next == EffectState::Started {
            updated.dispatch_ordinal = self
                .dispatch_ordinal
                .checked_add(1)
                .ok_or(EffectError::VersionOverflow)?;
        }
        updated.validate()?;
        Ok(updated)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub evidence_id: String,
    pub task_id: String,
    pub subject_ref: String,
    pub level: EvidenceLevel,
    pub kind: String,
    pub producer: Value,
    pub payload_ref: Option<Value>,
    pub payload_digest: Option<String>,
    /// The exact VerificationRun that produced verifier-scoped Evidence. Runtime
    /// reports leave this unset; Runtime admission cannot claim a verifier run.
    #[serde(default)]
    pub verification_run_id: Option<String>,
    pub created_at: String,
}

impl EvidenceRecord {
    pub fn validate(&self) -> Result<(), EffectError> {
        for value in [
            self.evidence_id.as_str(),
            self.task_id.as_str(),
            self.subject_ref.as_str(),
            self.kind.as_str(),
            self.created_at.as_str(),
        ] {
            if invalid_bounded(value, 4096) {
                return Err(EffectError::InvalidRecord);
            }
        }
        if !valid_producer(&self.producer)
            || self
                .verification_run_id
                .as_deref()
                .is_some_and(|value| invalid_bounded(value, 256))
            || self
                .payload_ref
                .as_ref()
                .is_some_and(|value| !valid_resource_ref(value))
            || self
                .payload_digest
                .as_ref()
                .is_some_and(|digest| !valid_digest(digest))
        {
            return Err(EffectError::InvalidRecord);
        }
        Ok(())
    }
}

/// Exact immutable Resource revision identity consumed by a verifier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationResourceRef {
    pub workspace_id: String,
    pub resource_id: String,
    pub revision_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationInput {
    pub resource_ref: VerificationResourceRef,
    /// Digest of bytes actually supplied to the verifier, if bytes were read.
    pub consumed_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationRunStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Inconclusive,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationFailureCode {
    Timeout,
    IntegrityFailure,
    VerifierUnavailable,
    UnsupportedCriterion,
    InputUnavailable,
    TaskCancelled,
}

impl VerificationFailureCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "TIMEOUT",
            Self::IntegrityFailure => "INTEGRITY_FAILURE",
            Self::VerifierUnavailable => "VERIFIER_UNAVAILABLE",
            Self::UnsupportedCriterion => "UNSUPPORTED_CRITERION",
            Self::InputUnavailable => "INPUT_UNAVAILABLE",
            Self::TaskCancelled => "TASK_CANCELLED",
        }
    }
}

impl VerificationRunStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Running => "RUNNING",
            Self::Passed => "PASSED",
            Self::Failed => "FAILED",
            Self::Inconclusive => "INCONCLUSIVE",
        }
    }

    pub const fn terminal(self) -> bool {
        matches!(self, Self::Passed | Self::Failed | Self::Inconclusive)
    }
}

/// A verifier result permanently pinned to the exact criterion revision, input
/// revisions and Evidence it consumed. Retries create a different run record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRunRecord {
    pub verification_run_id: String,
    pub task_id: String,
    pub criterion_id: String,
    pub task_spec_revision: u64,
    pub criterion_digest: String,
    pub verifier_kind: String,
    pub verifier_version: String,
    pub subject_refs: Vec<VerificationResourceRef>,
    pub inputs: Vec<VerificationInput>,
    pub status: VerificationRunStatus,
    pub evidence_refs: Vec<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub version: u64,
    pub failure_code: Option<VerificationFailureCode>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationRunError {
    InvalidRecord,
    InvalidTransition,
    TerminalImmutable,
    VersionOverflow,
}

impl VerificationRunRecord {
    pub fn validate(&self) -> Result<(), VerificationRunError> {
        let valid = |value: &str, max: usize| !invalid_bounded(value, max);
        if !valid(&self.verification_run_id, 256)
            || !valid(&self.task_id, 256)
            || !valid(&self.criterion_id, 512)
            || self.task_spec_revision == 0
            || self.version == 0
            || !valid_digest(&self.criterion_digest)
            || !valid(&self.verifier_kind, 256)
            || !valid(&self.verifier_version, 256)
            || self.subject_refs.len() > MAX_VERIFICATION_SUBJECTS
            || self.inputs.len() > MAX_VERIFICATION_INPUTS
            || self.evidence_refs.len() > MAX_VERIFICATION_EVIDENCE_REFS
            || self.evidence_refs.iter().any(|value| !valid(value, 256))
            || has_duplicates(&self.evidence_refs)
            || self
                .subject_refs
                .iter()
                .any(|reference| !reference.validate())
            || self.inputs.iter().any(|input| {
                !input.resource_ref.validate()
                    || input
                        .consumed_digest
                        .as_deref()
                        .is_some_and(|digest| !valid_digest(digest))
            })
            || has_duplicate_inputs(&self.inputs)
            || self
                .started_at
                .as_deref()
                .is_some_and(|value| !valid(value, 128))
            || self
                .completed_at
                .as_deref()
                .is_some_and(|value| !valid(value, 128))
        {
            return Err(VerificationRunError::InvalidRecord);
        }
        match self.status {
            VerificationRunStatus::Pending
                if self.started_at.is_none()
                    && self.completed_at.is_none()
                    && self.evidence_refs.is_empty()
                    && self.failure_code.is_none() => {}
            VerificationRunStatus::Running
                if self.started_at.is_some()
                    && self.completed_at.is_none()
                    && self.failure_code.is_none()
                    && self.evidence_refs.is_empty() => {}
            status if status.terminal() => {
                if self.started_at.is_none() || self.completed_at.is_none() {
                    return Err(VerificationRunError::InvalidRecord);
                }
                match status {
                    VerificationRunStatus::Passed
                        if self.evidence_refs.is_empty() || self.failure_code.is_some() =>
                    {
                        return Err(VerificationRunError::InvalidRecord);
                    }
                    VerificationRunStatus::Failed | VerificationRunStatus::Inconclusive
                        if self.failure_code.is_none() =>
                    {
                        return Err(VerificationRunError::InvalidRecord);
                    }
                    _ => {}
                }
            }
            _ => return Err(VerificationRunError::InvalidRecord),
        }
        Ok(())
    }

    pub fn start(&self, at: String) -> Result<Self, VerificationRunError> {
        self.validate()?;
        if self.status != VerificationRunStatus::Pending || !valid_timestamp(&at) {
            return Err(VerificationRunError::InvalidTransition);
        }
        let mut next = self.clone();
        next.status = VerificationRunStatus::Running;
        next.started_at = Some(at);
        next.version = self
            .version
            .checked_add(1)
            .ok_or(VerificationRunError::VersionOverflow)?;
        next.validate()?;
        Ok(next)
    }

    pub fn finish(
        &self,
        status: VerificationRunStatus,
        evidence_refs: Vec<String>,
        failure_code: Option<VerificationFailureCode>,
        at: String,
    ) -> Result<Self, VerificationRunError> {
        self.validate()?;
        if self.status.terminal() {
            return Err(VerificationRunError::TerminalImmutable);
        }
        if self.status != VerificationRunStatus::Running
            || !status.terminal()
            || !valid_timestamp(&at)
        {
            return Err(VerificationRunError::InvalidTransition);
        }
        let mut next = self.clone();
        next.status = status;
        next.evidence_refs = evidence_refs;
        next.completed_at = Some(at);
        next.failure_code = failure_code;
        next.version = self
            .version
            .checked_add(1)
            .ok_or(VerificationRunError::VersionOverflow)?;
        next.validate()?;
        Ok(next)
    }
}

impl VerificationResourceRef {
    fn validate(&self) -> bool {
        !invalid_bounded(&self.workspace_id, 256)
            && !invalid_bounded(&self.resource_id, 256)
            && !invalid_bounded(&self.revision_id, 256)
    }
}

fn valid_timestamp(value: &str) -> bool {
    !invalid_bounded(value, 128)
}

fn has_duplicates(values: &[String]) -> bool {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).any(|pair| pair[0] == pair[1])
}

fn has_duplicate_inputs(inputs: &[VerificationInput]) -> bool {
    let mut refs = inputs
        .iter()
        .map(|input| {
            (
                &input.resource_ref.workspace_id,
                &input.resource_ref.resource_id,
                &input.resource_ref.revision_id,
            )
        })
        .collect::<Vec<_>>();
    refs.sort_unstable();
    refs.windows(2).any(|pair| pair[0] == pair[1])
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredVerificationCriterion {
    pub criterion_id: String,
    pub task_spec_revision: u64,
    pub criterion_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompletionBlocker {
    InvalidCriterion,
    EvaluationInputOverBudget,
    VerifierEvidenceBindingUnavailable,
    CriterionUnverified(String),
    UnresolvedEffects,
    PendingApprovals,
    IncompleteChildren,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionAssessment {
    pub eligible: bool,
    pub blockers: Vec<CompletionBlocker>,
}

/// Pure conservative gate. It recommends eligibility only; TaskService still owns
/// the fenced aggregate transition and must recheck all source heads transactionally.
pub fn assess_completion(
    task_id: &str,
    criteria: &[RequiredVerificationCriterion],
    runs: &[VerificationRunRecord],
    unresolved_effects: usize,
    pending_approvals: usize,
    incomplete_children: usize,
) -> CompletionAssessment {
    // No trusted store path can currently bind a verifier's identity and exact
    // same-run Evidence. Keep Task completion closed even for a structurally valid
    // caller-constructed PASSED record until that producer boundary exists.
    let mut blockers = vec![CompletionBlocker::VerifierEvidenceBindingUnavailable];
    for criterion in criteria {
        if invalid_bounded(task_id, 256)
            || invalid_bounded(&criterion.criterion_id, 512)
            || criterion.task_spec_revision == 0
            || !valid_digest(&criterion.criterion_digest)
        {
            blockers.push(CompletionBlocker::InvalidCriterion);
            continue;
        }
        let satisfied = runs.iter().any(|run| {
            run.validate().is_ok()
                && run.task_id == task_id
                && run.status == VerificationRunStatus::Passed
                && run.criterion_id == criterion.criterion_id
                && run.task_spec_revision == criterion.task_spec_revision
                && run.criterion_digest == criterion.criterion_digest
                && !run.evidence_refs.is_empty()
        });
        if !satisfied {
            blockers.push(CompletionBlocker::CriterionUnverified(
                criterion.criterion_id.clone(),
            ));
        }
    }
    if unresolved_effects > 0 {
        blockers.push(CompletionBlocker::UnresolvedEffects);
    }
    if pending_approvals > 0 {
        blockers.push(CompletionBlocker::PendingApprovals);
    }
    if incomplete_children > 0 {
        blockers.push(CompletionBlocker::IncompleteChildren);
    }
    CompletionAssessment {
        eligible: blockers.is_empty(),
        blockers,
    }
}

/// Evaluates completion from a trusted, committed Evidence projection. The caller must
/// obtain `evidence` from the authoritative EvidenceStore (never from agent/verifier
/// output) and the Task transition still has to recheck source heads and open Effects
/// transactionally. This overload can establish exact same-run Evidence binding; the
/// compatibility overload above deliberately stays fail-closed when Evidence rows are
/// unavailable.
pub fn assess_completion_with_evidence(
    task_id: &str,
    criteria: &[RequiredVerificationCriterion],
    runs: &[VerificationRunRecord],
    evidence: &[EvidenceRecord],
    unresolved_effects: usize,
    pending_approvals: usize,
    incomplete_children: usize,
) -> CompletionAssessment {
    let mut blockers = Vec::new();
    if invalid_bounded(task_id, 256) {
        blockers.push(CompletionBlocker::InvalidCriterion);
    }
    if criteria.len() > 256 || runs.len() > 4096 || evidence.len() > 16_384 {
        blockers.push(CompletionBlocker::EvaluationInputOverBudget);
    }

    for criterion in criteria.iter().take(256) {
        if invalid_bounded(&criterion.criterion_id, 512)
            || criterion.task_spec_revision == 0
            || !valid_digest(&criterion.criterion_digest)
        {
            blockers.push(CompletionBlocker::InvalidCriterion);
            continue;
        }

        let satisfied = runs.iter().take(4096).any(|run| {
            if run.validate().is_err()
                || run.task_id != task_id
                || run.status != VerificationRunStatus::Passed
                || run.criterion_id != criterion.criterion_id
                || run.task_spec_revision != criterion.task_spec_revision
                || run.criterion_digest != criterion.criterion_digest
            {
                return false;
            }

            let bound: Vec<_> = evidence
                .iter()
                .take(16_384)
                .filter(|item| {
                    item.verification_run_id.as_deref() == Some(&run.verification_run_id)
                })
                .collect();
            if bound.len() != run.evidence_refs.len() || bound.is_empty() {
                return false;
            }

            run.evidence_refs.iter().all(|evidence_id| {
                let Some(item) = evidence
                    .iter()
                    .take(16_384)
                    .find(|item| item.evidence_id == *evidence_id)
                else {
                    return false;
                };
                item.validate().is_ok()
                    && item.task_id == task_id
                    && item.level == EvidenceLevel::Verified
                    && item.verification_run_id.as_deref() == Some(&run.verification_run_id)
                    && verifier_evidence_subject_is_pinned(run, &item.subject_ref)
            })
        });
        if !satisfied {
            blockers.push(CompletionBlocker::CriterionUnverified(
                criterion.criterion_id.clone(),
            ));
        }
    }

    if unresolved_effects > 0 {
        blockers.push(CompletionBlocker::UnresolvedEffects);
    }
    if pending_approvals > 0 {
        blockers.push(CompletionBlocker::PendingApprovals);
    }
    if incomplete_children > 0 {
        blockers.push(CompletionBlocker::IncompleteChildren);
    }
    CompletionAssessment {
        eligible: blockers.is_empty(),
        blockers,
    }
}

fn verifier_evidence_subject_is_pinned(run: &VerificationRunRecord, subject: &str) -> bool {
    if run.subject_refs.is_empty() {
        return subject == format!("task:{}", run.task_id);
    }
    run.subject_refs.iter().any(|reference| {
        subject
            == format!(
                "resource://{}/{}@{}",
                reference.workspace_id, reference.resource_id, reference.revision_id
            )
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectError {
    InvalidRecord,
    InvalidTransition,
    RetryNotAuthorized,
    MissingVerificationEvidence,
    VersionOverflow,
}

impl std::fmt::Display for EffectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for EffectError {}

fn invalid_bounded(value: &str, max: usize) -> bool {
    value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control)
}
fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_target(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|target| !invalid_bounded(target, 4096))
        || value.as_object().is_some_and(|object| {
            object
                .keys()
                .all(|key| ["workspace_id", "resource_id", "revision_id"].contains(&key.as_str()))
                && valid_resource_ref(value)
        })
}
fn valid_resource_ref(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        object
            .keys()
            .all(|key| ["workspace_id", "resource_id", "revision_id"].contains(&key.as_str()))
            && object
                .get("workspace_id")
                .and_then(Value::as_str)
                .is_some_and(|s| !invalid_bounded(s, 256))
            && object
                .get("resource_id")
                .and_then(Value::as_str)
                .is_some_and(|s| !invalid_bounded(s, 256))
            && object
                .get("revision_id")
                .is_none_or(|r| r.is_null() || r.as_str().is_some_and(|s| !invalid_bounded(s, 256)))
    })
}
fn valid_producer(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    match (
        object.get("principal_id"),
        object.get("kind"),
        object.get("service_id"),
    ) {
        (Some(Value::String(id)), Some(Value::String(kind)), None) => {
            object.len() == 2
                && !invalid_bounded(id, 256)
                && ["USER", "SERVICE", "RUNTIME", "AGENT", "CHANNEL_IDENTITY"]
                    .contains(&kind.as_str())
        }
        (None, None, Some(Value::String(id))) => object.len() == 1 && !invalid_bounded(id, 256),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn proposed() -> EffectRecord {
        EffectRecord {
            effect_id: "effect-1".into(),
            task_id: "task-1".into(),
            attempt_id: "attempt-1".into(),
            capability_ref: Some(json!({"package_id":"mail","capability_id":"send"})),
            operation: "message.send".into(),
            target: json!("mailbox:user@example.test"),
            idempotency_key: Some("stable-1".into()),
            state: EffectState::Proposed,
            request_digest: format!("sha256:{}", "a".repeat(64)),
            capability_invocation_id: "inv-1".into(),
            execution_method: ExecutionMethod::StructuredApi,
            dispatch_ordinal: 0,
            result_ref: None,
            observed_state: None,
            verification_ref: None,
            created_at: "2026-10-08T00:00:00Z".into(),
            updated_at: "2026-10-08T00:00:00Z".into(),
            version: 1,
        }
    }

    #[test]
    fn first_dispatch_follows_persisted_proposal_and_advances_ordinal() {
        let started = proposed()
            .transition(EffectState::Started, "2026-10-08T00:01:00Z".into())
            .unwrap();
        assert_eq!(started.dispatch_ordinal, 1);
        assert_eq!(started.version, 2);
    }

    #[test]
    fn proposed_effect_cannot_skip_started_to_observed() {
        assert_eq!(
            proposed().transition(EffectState::Observed, "2026-10-08T00:01:00Z".into()),
            Err(EffectError::InvalidTransition)
        );
    }

    #[test]
    fn proposal_can_fail_before_dispatch_without_claiming_a_dispatch() {
        let failed = proposed()
            .transition(EffectState::Failed, "2026-10-08T00:01:00Z".into())
            .unwrap();
        assert_eq!(failed.dispatch_ordinal, 0);
        assert!(failed.validate().is_ok());
    }

    #[test]
    fn retry_requires_reconciliation_proof_and_stable_key_when_using_idempotency_basis() {
        let started = proposed()
            .transition(EffectState::Started, "2026-10-08T00:01:00Z".into())
            .unwrap();
        let ambiguous = started
            .transition(EffectState::Ambiguous, "2026-10-08T00:02:00Z".into())
            .unwrap();
        let reconciling = ambiguous
            .transition(EffectState::Reconciling, "2026-10-08T00:03:00Z".into())
            .unwrap();
        assert_eq!(
            reconciling.transition(EffectState::Started, "2026-10-08T00:04:00Z".into()),
            Err(EffectError::RetryNotAuthorized),
        );
        assert_eq!(
            reconciling.authorize_retry("2026-10-08T00:04:00Z".into(), "evidence-1", false, false),
            Err(EffectError::RetryNotAuthorized),
        );
        let retried = reconciling
            .authorize_retry("2026-10-08T00:04:00Z".into(), "evidence-1", true, false)
            .unwrap();
        assert_eq!(retried.dispatch_ordinal, 2);
    }

    #[test]
    fn evidence_requires_digest_shape_and_producer_object() {
        let evidence = EvidenceRecord {
            evidence_id: "ev-1".into(),
            task_id: "task-1".into(),
            subject_ref: "effect:effect-1".into(),
            level: EvidenceLevel::Reported,
            kind: "provider_result".into(),
            producer: json!({"service_id":"adapter"}),
            payload_ref: None,
            payload_digest: None,
            verification_run_id: None,
            created_at: "2026-10-08T00:00:00Z".into(),
        };
        assert!(evidence.validate().is_ok());
    }

    #[test]
    fn old_evidence_payloads_deserialize_without_a_run_binding() {
        let value = json!({
            "evidence_id":"ev-legacy",
            "task_id":"task-1",
            "subject_ref":"task:task-1",
            "level":"REPORTED",
            "kind":"provider_result",
            "producer":{"service_id":"adapter"},
            "payload_ref":null,
            "payload_digest":null,
            "created_at":"2026-10-08T00:00:00Z"
        });
        let evidence: EvidenceRecord = serde_json::from_value(value).unwrap();
        assert_eq!(evidence.verification_run_id, None);
    }

    #[test]
    fn verification_run_pins_exact_criterion_inputs_and_evidence() {
        let run = VerificationRunRecord {
            verification_run_id: "verify-1".into(),
            task_id: "task-1".into(),
            criterion_id: "criterion-1".into(),
            task_spec_revision: 4,
            criterion_digest: format!("sha256:{}", "a".repeat(64)),
            verifier_kind: "deterministic-test".into(),
            verifier_version: "2.1".into(),
            subject_refs: vec![VerificationResourceRef {
                workspace_id: "ws-1".into(),
                resource_id: "resource-1".into(),
                revision_id: "rev-3".into(),
            }],
            inputs: vec![VerificationInput {
                resource_ref: VerificationResourceRef {
                    workspace_id: "ws-1".into(),
                    resource_id: "resource-1".into(),
                    revision_id: "rev-3".into(),
                },
                consumed_digest: Some(format!("sha256:{}", "b".repeat(64))),
            }],
            status: VerificationRunStatus::Running,
            version: 2,
            evidence_refs: vec![],
            started_at: Some("2026-10-16T10:00:00Z".into()),
            completed_at: None,
            failure_code: None,
        };
        assert!(run.validate().is_ok());
        let completed = run
            .finish(
                VerificationRunStatus::Passed,
                vec!["evidence-1".into()],
                None,
                "2026-10-16T10:00:01Z".into(),
            )
            .unwrap();
        assert_eq!(completed.status, VerificationRunStatus::Passed);
        assert_eq!(completed.evidence_refs, vec!["evidence-1"]);
        assert_eq!(completed.task_spec_revision, 4);
    }

    #[test]
    fn verification_run_rejects_duplicate_evidence_and_invalid_consumed_digest() {
        let mut run = VerificationRunRecord {
            verification_run_id: "verify-1".into(),
            task_id: "task-1".into(),
            criterion_id: "criterion-1".into(),
            task_spec_revision: 1,
            criterion_digest: format!("sha256:{}", "a".repeat(64)),
            verifier_kind: "test".into(),
            verifier_version: "1".into(),
            subject_refs: vec![],
            inputs: vec![],
            status: VerificationRunStatus::Passed,
            version: 3,
            evidence_refs: vec!["evidence-1".into(), "evidence-1".into()],
            started_at: Some("2026-10-16T10:00:00Z".into()),
            completed_at: Some("2026-10-16T10:00:01Z".into()),
            failure_code: None,
        };
        assert_eq!(run.validate(), Err(VerificationRunError::InvalidRecord));
        run.evidence_refs = vec!["evidence-1".into()];
        run.inputs.push(VerificationInput {
            resource_ref: VerificationResourceRef {
                workspace_id: "ws-1".into(),
                resource_id: "resource-1".into(),
                revision_id: "rev-3".into(),
            },
            consumed_digest: Some("sha256:bad".into()),
        });
        assert_eq!(run.validate(), Err(VerificationRunError::InvalidRecord));

        run.inputs.clear();
        run.evidence_refs = (0..=MAX_VERIFICATION_EVIDENCE_REFS)
            .map(|index| format!("evidence-{index}"))
            .collect();
        assert_eq!(run.validate(), Err(VerificationRunError::InvalidRecord));
    }

    #[test]
    fn passed_verification_run_requires_immutable_evidence_reference() {
        let run = VerificationRunRecord {
            verification_run_id: "verify-1".into(),
            task_id: "task-1".into(),
            criterion_id: "criterion-1".into(),
            task_spec_revision: 1,
            criterion_digest: format!("sha256:{}", "a".repeat(64)),
            verifier_kind: "test".into(),
            verifier_version: "1".into(),
            subject_refs: vec![],
            inputs: vec![],
            status: VerificationRunStatus::Passed,
            version: 2,
            evidence_refs: vec![],
            started_at: Some("2026-10-16T10:00:00Z".into()),
            completed_at: Some("2026-10-16T10:00:01Z".into()),
            failure_code: None,
        };
        assert_eq!(run.validate(), Err(VerificationRunError::InvalidRecord));
    }

    #[test]
    fn verification_run_terminal_result_cannot_be_rewritten() {
        let run = VerificationRunRecord {
            verification_run_id: "verify-1".into(),
            task_id: "task-1".into(),
            criterion_id: "criterion-1".into(),
            task_spec_revision: 1,
            criterion_digest: format!("sha256:{}", "a".repeat(64)),
            verifier_kind: "test".into(),
            verifier_version: "1".into(),
            subject_refs: vec![],
            inputs: vec![],
            status: VerificationRunStatus::Inconclusive,
            version: 2,
            evidence_refs: vec![],
            started_at: Some("2026-10-16T10:00:00Z".into()),
            completed_at: Some("2026-10-16T10:00:01Z".into()),
            failure_code: Some(VerificationFailureCode::Timeout),
        };
        assert_eq!(
            run.finish(
                VerificationRunStatus::Passed,
                vec!["evidence-1".into()],
                None,
                "2026-10-16T10:00:02Z".into(),
            ),
            Err(VerificationRunError::TerminalImmutable)
        );
    }

    #[test]
    fn completion_requires_exact_passed_run_and_no_open_authority_or_effect_work() {
        let criterion = RequiredVerificationCriterion {
            criterion_id: "criterion-1".into(),
            task_spec_revision: 4,
            criterion_digest: format!("sha256:{}", "a".repeat(64)),
        };
        let pending = VerificationRunRecord {
            verification_run_id: "verify-1".into(),
            task_id: "task-1".into(),
            criterion_id: "criterion-1".into(),
            task_spec_revision: 3,
            criterion_digest: criterion.criterion_digest.clone(),
            verifier_kind: "test".into(),
            verifier_version: "1".into(),
            subject_refs: vec![],
            inputs: vec![],
            status: VerificationRunStatus::Passed,
            version: 2,
            evidence_refs: vec!["evidence-1".into()],
            started_at: Some("2026-10-16T10:00:00Z".into()),
            completed_at: Some("2026-10-16T10:00:01Z".into()),
            failure_code: None,
        };
        let blocked =
            assess_completion("task-1", &[criterion.clone()], &[pending.clone()], 0, 0, 0);
        assert!(!blocked.eligible);
        assert_eq!(
            blocked.blockers,
            vec![
                CompletionBlocker::VerifierEvidenceBindingUnavailable,
                CompletionBlocker::CriterionUnverified("criterion-1".into())
            ]
        );

        let mut exact = pending;
        exact.task_spec_revision = 4;
        let not_yet_trusted =
            assess_completion("task-1", &[criterion.clone()], &[exact.clone()], 0, 0, 0);
        assert!(!not_yet_trusted.eligible);
        assert!(
            not_yet_trusted
                .blockers
                .contains(&CompletionBlocker::VerifierEvidenceBindingUnavailable)
        );
        let cross_task = assess_completion("task-other", &[criterion], &[exact.clone()], 0, 0, 0);
        assert!(!cross_task.eligible);
        let blocked_by_live_state = assess_completion("task-1", &[], &[exact], 1, 1, 1);
        assert_eq!(
            blocked_by_live_state.blockers,
            vec![
                CompletionBlocker::VerifierEvidenceBindingUnavailable,
                CompletionBlocker::UnresolvedEffects,
                CompletionBlocker::PendingApprovals,
                CompletionBlocker::IncompleteChildren,
            ]
        );
    }

    #[test]
    fn committed_same_run_verified_evidence_can_satisfy_exact_criterion() {
        let criterion = RequiredVerificationCriterion {
            criterion_id: "criterion-1".into(),
            task_spec_revision: 4,
            criterion_digest: format!("sha256:{}", "a".repeat(64)),
        };
        let run = VerificationRunRecord {
            verification_run_id: "verify-1".into(),
            task_id: "task-1".into(),
            criterion_id: criterion.criterion_id.clone(),
            task_spec_revision: criterion.task_spec_revision,
            criterion_digest: criterion.criterion_digest.clone(),
            verifier_kind: "deterministic-test".into(),
            verifier_version: "1".into(),
            subject_refs: vec![],
            inputs: vec![],
            status: VerificationRunStatus::Passed,
            version: 2,
            evidence_refs: vec!["evidence-1".into()],
            started_at: Some("2026-10-16T10:00:00Z".into()),
            completed_at: Some("2026-10-16T10:00:01Z".into()),
            failure_code: None,
        };
        let evidence = EvidenceRecord {
            evidence_id: "evidence-1".into(),
            task_id: "task-1".into(),
            subject_ref: "task:task-1".into(),
            level: EvidenceLevel::Verified,
            kind: "criterion-check".into(),
            producer: json!({"service_id":"verifier-1"}),
            payload_ref: None,
            payload_digest: None,
            verification_run_id: Some("verify-1".into()),
            created_at: "2026-10-16T10:00:01Z".into(),
        };

        let assessment =
            assess_completion_with_evidence("task-1", &[criterion], &[run], &[evidence], 0, 0, 0);
        assert!(assessment.eligible);
        assert!(assessment.blockers.is_empty());
    }

    #[test]
    fn completion_rejects_reported_cross_run_and_unpinned_subject_evidence() {
        let criterion = RequiredVerificationCriterion {
            criterion_id: "criterion-1".into(),
            task_spec_revision: 4,
            criterion_digest: format!("sha256:{}", "b".repeat(64)),
        };
        let run = VerificationRunRecord {
            verification_run_id: "verify-1".into(),
            task_id: "task-1".into(),
            criterion_id: criterion.criterion_id.clone(),
            task_spec_revision: criterion.task_spec_revision,
            criterion_digest: criterion.criterion_digest.clone(),
            verifier_kind: "deterministic-test".into(),
            verifier_version: "1".into(),
            subject_refs: vec![VerificationResourceRef {
                workspace_id: "workspace-1".into(),
                resource_id: "resource-1".into(),
                revision_id: "revision-1".into(),
            }],
            inputs: vec![],
            status: VerificationRunStatus::Passed,
            version: 2,
            evidence_refs: vec!["evidence-1".into()],
            started_at: Some("2026-10-16T10:00:00Z".into()),
            completed_at: Some("2026-10-16T10:00:01Z".into()),
            failure_code: None,
        };
        let mut evidence = EvidenceRecord {
            evidence_id: "evidence-1".into(),
            task_id: "task-1".into(),
            subject_ref: "resource://workspace-1/resource-1@revision-1".into(),
            level: EvidenceLevel::Reported,
            kind: "criterion-check".into(),
            producer: json!({"service_id":"verifier-1"}),
            payload_ref: None,
            payload_digest: None,
            verification_run_id: Some("verify-other".into()),
            created_at: "2026-10-16T10:00:01Z".into(),
        };

        let blocked = assess_completion_with_evidence(
            "task-1",
            &[criterion.clone()],
            &[run.clone()],
            &[evidence.clone()],
            0,
            0,
            0,
        );
        assert!(!blocked.eligible);
        assert!(
            blocked
                .blockers
                .contains(&CompletionBlocker::CriterionUnverified(
                    "criterion-1".into()
                ))
        );

        evidence.level = EvidenceLevel::Verified;
        evidence.verification_run_id = Some("verify-1".into());
        evidence.subject_ref = "resource://workspace-1/resource-1@revision-other".into();
        let blocked =
            assess_completion_with_evidence("task-1", &[criterion], &[run], &[evidence], 0, 0, 0);
        assert!(!blocked.eligible);
    }

    #[test]
    fn completion_evaluation_rejects_oversized_projection_sets() {
        let criterion = RequiredVerificationCriterion {
            criterion_id: "criterion-1".into(),
            task_spec_revision: 1,
            criterion_digest: format!("sha256:{}", "a".repeat(64)),
        };
        let criteria = vec![criterion; 257];
        let assessment = assess_completion_with_evidence("task-1", &criteria, &[], &[], 0, 0, 0);
        assert!(!assessment.eligible);
        assert!(
            assessment
                .blockers
                .contains(&CompletionBlocker::EvaluationInputOverBudget)
        );
    }
}
