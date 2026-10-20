//! Pure child-Attempt admission policy over trusted, versioned projections.
//!
//! This decision is advisory. The durable Task admission transaction must reread the
//! same Task, Plan, Step, Attempt, session, lease, profile and budget facts under its
//! serialization boundary and must fail closed if it cannot do so.

const PLATFORM_MAX_CHILD_DEPTH: u8 = 2;
const PLATFORM_MAX_ACTIVE_CHILDREN: u8 = 8;
pub const CHILD_ADMISSION_POLICY_VERSION: &str = "host-child-admission-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildAdmissionRequest {
    pub workspace_id: String,
    pub task_id: String,
    pub step_id: String,
    pub parent_attempt_id: String,
    pub child_attempt_id: String,
    pub expected_task_version: u64,
    pub expected_task_spec_revision: u64,
    pub expected_plan_revision: u64,
    pub profile_id: String,
    pub profile_revision: u64,
    pub agent_binding_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildTaskProjection {
    pub workspace_id: String,
    pub task_id: String,
    pub status: String,
    pub version: u64,
    pub task_spec_revision: u64,
    pub current_plan_revision: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildStepProjection {
    pub task_id: String,
    pub step_id: String,
    pub plan_revision: u64,
    pub is_member_of_current_plan: bool,
    pub status: String,
    pub dependencies_satisfied: bool,
    pub current_attempt_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParentAttemptProjection {
    pub attempt_id: String,
    pub task_id: String,
    pub status: String,
    pub task_spec_revision: u64,
    pub plan_revision: u64,
    pub agent_session_attempt_id: Option<String>,
    pub agent_session_status: Option<String>,
    pub lease_attempt_id: Option<String>,
    pub lease_status: Option<String>,
    /// Ordered root-to-parent IDs. The final element must be `attempt_id`.
    pub ancestry: Vec<String>,
    /// The profile pinned to this parent Attempt, retained at the immutable revision.
    pub profile_pin: Option<PinnedProfileProjection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedProfileProjection {
    pub profile_id: String,
    pub revision: u64,
    pub max_host_delegation_depth: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateProfileProjection {
    pub workspace_id: String,
    pub profile_id: String,
    pub revision: u64,
    pub agent_binding_id: String,
    pub enabled: bool,
    pub archived: bool,
    pub max_concurrency: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildAdmissionSnapshot {
    pub task: ChildTaskProjection,
    pub step: ChildStepProjection,
    pub parent: ParentAttemptProjection,
    pub candidate_profile: CandidateProfileProjection,
    /// `Some` only when this Task pins a Coworker revision.
    pub coworker_pinned: bool,
    pub coworker_enabled_profile_ids: Vec<String>,
    pub task_max_concurrency: Option<u8>,
    pub coworker_max_concurrency: Option<u8>,
    pub active_task_child_attempts: u8,
    pub active_profile_attempts: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildAdmissionDenial {
    InvalidInput,
    WorkspaceMismatch,
    StaleTask,
    StalePlanOrSpec,
    StepNotReady,
    ParentAttemptNotCurrent,
    ParentSessionNotCurrent,
    ParentLeaseNotCurrent,
    InvalidLineage,
    DepthLimitReached,
    AuthorityPrerequisitesUnproven,
    ProfileRevisionMismatch,
    ProfileUnavailable,
    ProfileNotEnabledForCoworker,
    ConcurrencyLimitReached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildAdmissionDecision {
    Admit {
        child_depth: u8,
        effective_concurrency_limit: u8,
    },
    Deny(ChildAdmissionDenial),
}

/// Check parent provenance, accepted plan pins, bounded ancestry and the effective
/// cross-profile concurrency ceilings. This function has no persistence or authority
/// side effects and does not replace Trust, Environment, adapter or budget admission.
pub fn decide_child_attempt_admission(
    request: &ChildAdmissionRequest,
    snapshot: &ChildAdmissionSnapshot,
) -> ChildAdmissionDecision {
    if !valid_id(&request.workspace_id)
        || !valid_id(&request.task_id)
        || !valid_id(&request.step_id)
        || !valid_id(&request.parent_attempt_id)
        || !valid_id(&request.child_attempt_id)
        || !valid_id(&request.profile_id)
        || !valid_id(&request.agent_binding_id)
        || request.expected_task_version == 0
        || request.expected_task_spec_revision == 0
        || request.expected_plan_revision == 0
        || request.profile_revision == 0
    {
        return deny(ChildAdmissionDenial::InvalidInput);
    }
    let task = &snapshot.task;
    let step = &snapshot.step;
    let parent = &snapshot.parent;
    let profile = &snapshot.candidate_profile;
    if task.workspace_id != request.workspace_id
        || task.task_id != request.task_id
        || profile.workspace_id != request.workspace_id
    {
        return deny(ChildAdmissionDenial::WorkspaceMismatch);
    }
    if task.version != request.expected_task_version || task.status != "RUNNING" {
        return deny(ChildAdmissionDenial::StaleTask);
    }
    if task.task_spec_revision != request.expected_task_spec_revision
        || task.current_plan_revision != Some(request.expected_plan_revision)
        || step.task_id != task.task_id
        || step.step_id != request.step_id
        || step.plan_revision != request.expected_plan_revision
        || !step.is_member_of_current_plan
    {
        return deny(ChildAdmissionDenial::StalePlanOrSpec);
    }
    if step.status != "READY" || !step.dependencies_satisfied || step.current_attempt_id.is_some() {
        return deny(ChildAdmissionDenial::StepNotReady);
    }
    if parent.attempt_id != request.parent_attempt_id
        || parent.task_id != task.task_id
        || parent.status != "RUNNING"
        || parent.task_spec_revision != request.expected_task_spec_revision
        || parent.plan_revision != request.expected_plan_revision
    {
        return deny(ChildAdmissionDenial::ParentAttemptNotCurrent);
    }
    if parent.agent_session_attempt_id.as_deref() != Some(parent.attempt_id.as_str())
        || parent.agent_session_status.as_deref() != Some("ACTIVE")
    {
        return deny(ChildAdmissionDenial::ParentSessionNotCurrent);
    }
    if parent.lease_attempt_id.as_deref() != Some(parent.attempt_id.as_str())
        || parent.lease_status.as_deref() != Some("ACTIVE")
    {
        return deny(ChildAdmissionDenial::ParentLeaseNotCurrent);
    }
    if parent.ancestry.is_empty()
        || parent.ancestry.len() > usize::from(PLATFORM_MAX_CHILD_DEPTH) + 1
        || parent.ancestry.last().map(String::as_str) != Some(parent.attempt_id.as_str())
        || parent.ancestry.iter().any(|id| !valid_id(id))
        || has_duplicates(&parent.ancestry)
        || parent.ancestry.contains(&request.child_attempt_id)
        || parent.profile_pin.as_ref().is_some_and(|pin| {
            !valid_id(&pin.profile_id)
                || pin.revision == 0
                || pin.max_host_delegation_depth > PLATFORM_MAX_CHILD_DEPTH
        })
    {
        return deny(ChildAdmissionDenial::InvalidLineage);
    }
    let parent_depth = (parent.ancestry.len() - 1) as u8;
    // A delegated parent must carry the immutable profile revision that admitted it.
    // Without that pin, a child could inherit platform depth while bypassing the
    // profile's max_host_delegation_depth ceiling. Root Attempts may have no profile.
    if parent_depth > 0 && parent.profile_pin.is_none() {
        return deny(ChildAdmissionDenial::InvalidLineage);
    }
    let child_depth = parent_depth.saturating_add(1);
    if child_depth > PLATFORM_MAX_CHILD_DEPTH
        || parent.profile_pin.as_ref().is_some_and(|pin| {
            child_depth > parent_depth.saturating_add(pin.max_host_delegation_depth)
        })
    {
        return deny(ChildAdmissionDenial::DepthLimitReached);
    }
    if profile.profile_id != request.profile_id
        || profile.revision != request.profile_revision
        || !valid_id(&profile.agent_binding_id)
        || profile.agent_binding_id != request.agent_binding_id
    {
        return deny(ChildAdmissionDenial::ProfileRevisionMismatch);
    }
    if !profile.enabled
        || profile.archived
        || !(1..=PLATFORM_MAX_ACTIVE_CHILDREN).contains(&profile.max_concurrency)
    {
        return deny(ChildAdmissionDenial::ProfileUnavailable);
    }
    if snapshot.coworker_pinned
        && !snapshot
            .coworker_enabled_profile_ids
            .iter()
            .any(|id| id == &profile.profile_id)
    {
        return deny(ChildAdmissionDenial::ProfileNotEnabledForCoworker);
    }
    let Some(effective_concurrency_limit) = effective_concurrency_limit(snapshot) else {
        return deny(ChildAdmissionDenial::InvalidInput);
    };
    if snapshot.active_task_child_attempts >= effective_concurrency_limit
        || snapshot.active_profile_attempts >= profile.max_concurrency
    {
        return deny(ChildAdmissionDenial::ConcurrencyLimitReached);
    }
    ChildAdmissionDecision::Admit {
        child_depth,
        effective_concurrency_limit,
    }
}

/// Evaluate the shared child-admission policy using a storage-owned, transactionally
/// consistent projection. The result is still advisory; only the final Attempt writer
/// may commit and it must repeat every check in its own write transaction.
pub fn decide_child_attempt_admission_from_durable_snapshot(
    request: &ChildAdmissionRequest,
    durable: &storage_core::ChildAttemptAdmissionSnapshot,
) -> ChildAdmissionDecision {
    let task = ChildTaskProjection {
        workspace_id: durable.task.workspace_id.clone(),
        task_id: durable.task.task_id.clone(),
        status: durable.task.status.clone(),
        version: durable.task.version,
        task_spec_revision: durable.task_spec.revision,
        current_plan_revision: durable.task.current_plan_revision,
    };
    let step = ChildStepProjection {
        task_id: durable.step.task_id.clone(),
        step_id: durable.step.step_id.clone(),
        plan_revision: durable.step.plan_revision,
        is_member_of_current_plan: durable.plan.revision
            == durable.task.current_plan_revision.unwrap_or_default()
            && durable.plan.steps.iter().any(|planned| {
                durable.step.logical_key.as_deref() == Some(planned.logical_key.as_str())
            }),
        status: durable.step.status.clone(),
        dependencies_satisfied: durable
            .dependency_steps
            .iter()
            .all(|dependency| dependency.status == "COMPLETED"),
        current_attempt_id: durable.step.current_attempt_id.clone(),
    };
    let parent_session = durable.parent_session.as_ref().filter(|session| {
        session.workspace_id == durable.task.workspace_id
            && session.scope_kind == "ATTEMPT_EXECUTION"
            && session.task_id.as_deref() == Some(durable.task.task_id.as_str())
            && session.task_spec_revision == Some(durable.task_spec.revision)
            && session.attempt_id.as_deref() == Some(durable.parent_attempt.attempt_id.as_str())
            && session.agent_binding_id == durable.parent_attempt.agent_binding_id
            && session.runtime_id == durable.parent_attempt.runtime_id
            && session.runtime_incarnation_id == durable.parent_attempt.runtime_incarnation_id
            && durable.parent_attempt.agent_session_id.as_deref()
                == Some(session.agent_session_id.as_str())
    });
    let parent_lease = durable.parent_lease.as_ref().filter(|lease| {
        durable.parent_lease_current
            && durable.parent_attempt.execution_lease_id.as_deref() == Some(lease.lease_id.as_str())
            && lease.task_id == durable.task.task_id
            && lease.step_id == durable.parent_attempt.step_id
            && lease.attempt_id == durable.parent_attempt.attempt_id
            && lease.runtime_id == durable.parent_attempt.runtime_id
            && lease.runtime_incarnation_id == durable.parent_attempt.runtime_incarnation_id
    });
    let parent = ParentAttemptProjection {
        attempt_id: durable.parent_attempt.attempt_id.clone(),
        task_id: durable.parent_attempt.task_id.clone(),
        status: if durable.parent_step.as_ref().is_some_and(|step| {
            step.status == "RUNNING"
                && step.current_attempt_id.as_deref()
                    == Some(durable.parent_attempt.attempt_id.as_str())
                && step.plan_revision == durable.plan.revision
        }) {
            serde_json::to_value(durable.parent_attempt.status)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default()
        } else {
            "NOT_CURRENT".to_owned()
        },
        task_spec_revision: durable
            .parent_session
            .as_ref()
            .and_then(|session| session.task_spec_revision)
            .unwrap_or_default(),
        plan_revision: durable
            .parent_step
            .as_ref()
            .map(|step| step.plan_revision)
            .unwrap_or_default(),
        agent_session_attempt_id: parent_session.and_then(|session| session.attempt_id.clone()),
        agent_session_status: parent_session.map(|session| session.status.clone()),
        lease_attempt_id: parent_lease.map(|lease| lease.attempt_id.clone()),
        lease_status: parent_lease.map(|lease| match lease.state {
            storage_core::ExecutionLeaseState::Active => "ACTIVE".to_owned(),
            storage_core::ExecutionLeaseState::Releasing => "RELEASING".to_owned(),
            storage_core::ExecutionLeaseState::Released => "RELEASED".to_owned(),
            storage_core::ExecutionLeaseState::Expired => "EXPIRED".to_owned(),
            storage_core::ExecutionLeaseState::Revoked => "REVOKED".to_owned(),
        }),
        ancestry: durable.parent_ancestry.clone(),
        profile_pin: durable
            .parent_attempt
            .delegation_profile_id
            .as_ref()
            .and_then(|profile_id| {
                durable
                    .parent_attempt
                    .delegation_profile_revision
                    .zip(durable.parent_profile_max_delegation_depth)
                    .map(
                        |(revision, max_host_delegation_depth)| PinnedProfileProjection {
                            profile_id: profile_id.clone(),
                            revision,
                            max_host_delegation_depth,
                        },
                    )
            }),
    };
    let candidate_profile = CandidateProfileProjection {
        workspace_id: durable.candidate_profile.workspace_id.clone(),
        profile_id: durable.candidate_profile.profile_id.clone(),
        revision: durable.candidate_profile.current_revision,
        agent_binding_id: durable.candidate_profile.agent_binding_id.clone(),
        enabled: durable.candidate_profile.status == "ENABLED"
            && durable.candidate_profile.binding_enabled,
        archived: durable.candidate_profile.status == "ARCHIVED",
        max_concurrency: durable.candidate_profile.max_concurrency,
    };
    decide_child_attempt_admission(
        request,
        &ChildAdmissionSnapshot {
            task,
            step,
            parent,
            candidate_profile,
            coworker_pinned: durable.task.origin_coworker_id.is_some(),
            coworker_enabled_profile_ids: durable.coworker_enabled_profile_ids.clone(),
            task_max_concurrency: durable.task_max_concurrency,
            coworker_max_concurrency: durable.coworker_max_concurrency,
            active_task_child_attempts: durable.active_task_child_attempts,
            active_profile_attempts: durable.active_profile_attempts,
        },
    )
}

/// Evaluate a read-only preview without presenting policy eligibility as executable
/// admission. An otherwise eligible child is reported as blocked until all required
/// authority proofs are confirmed in the snapshot; the final writer still must repeat
/// these proofs atomically before it can create an Attempt.
pub fn preview_child_attempt_admission_from_durable_snapshot(
    request: &ChildAdmissionRequest,
    durable: &storage_core::ChildAttemptAdmissionSnapshot,
) -> ChildAdmissionDecision {
    let decision = decide_child_attempt_admission_from_durable_snapshot(request, durable);
    match decision {
        ChildAdmissionDecision::Admit { .. }
            if !storage_core::unproven_child_admission_gates(&durable.proofs).is_empty() =>
        {
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::AuthorityPrerequisitesUnproven)
        }
        other => other,
    }
}

/// Load the store-owned snapshot and run the shared policy evaluator. This is a
/// read-only admission preview; execution still requires the atomic writer and all
/// authority proofs to be present at commit time.
pub fn preview_child_attempt_admission<S: storage_core::StepAttemptStore>(
    store: &S,
    command: &storage_core::AdmitStepAttempt,
) -> Result<ChildAdmissionDecision, storage_core::StoreError> {
    let (Some(parent_attempt_id), Some(profile_id), Some(profile_revision)) = (
        command.parent_attempt_id.as_deref(),
        command.delegation_profile_id.as_deref(),
        command.delegation_profile_revision,
    ) else {
        return Ok(ChildAdmissionDecision::Deny(
            ChildAdmissionDenial::InvalidInput,
        ));
    };
    let request = ChildAdmissionRequest {
        workspace_id: command.workspace_id.clone(),
        task_id: command.task_id.clone(),
        step_id: command.step_id.clone(),
        parent_attempt_id: parent_attempt_id.to_owned(),
        child_attempt_id: command.attempt_id.clone(),
        expected_task_version: command.expected_task_version,
        expected_task_spec_revision: command.expected_task_spec_revision,
        expected_plan_revision: command.expected_plan_revision,
        profile_id: profile_id.to_owned(),
        profile_revision,
        agent_binding_id: command.agent_binding_id.clone(),
    };
    let snapshot = store.child_attempt_admission_snapshot(command)?;
    Ok(preview_child_attempt_admission_from_durable_snapshot(
        &request, &snapshot,
    ))
}

fn effective_concurrency_limit(snapshot: &ChildAdmissionSnapshot) -> Option<u8> {
    // Task/Coworker limits cap the aggregate active child count across profiles.
    // A profile's max_concurrency is independent and is enforced against that
    // profile's own active count below; folding it into this aggregate cap would
    // incorrectly make one low-concurrency profile throttle other profiles.
    let mut limit = PLATFORM_MAX_ACTIVE_CHILDREN;
    for candidate in [
        snapshot.task_max_concurrency,
        snapshot.coworker_max_concurrency,
    ]
    .into_iter()
    .flatten()
    {
        if !(1..=PLATFORM_MAX_ACTIVE_CHILDREN).contains(&candidate) {
            return None;
        }
        limit = limit.min(candidate);
    }
    // A pinned Coworker may leave this override unset; in that case the stricter
    // platform/Task ceiling still applies. A Coworker-only limit without a
    // pinned Coworker revision is inconsistent caller input.
    if !snapshot.coworker_pinned && snapshot.coworker_max_concurrency.is_some() {
        return None;
    }
    Some(limit)
}

fn has_duplicates(values: &[String]) -> bool {
    values
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != values.len()
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn deny(reason: ChildAdmissionDenial) -> ChildAdmissionDecision {
    ChildAdmissionDecision::Deny(reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use storage_core::{
        AdmitStepAttempt, ChildAttemptAdmissionSnapshot, CommittedStepAttempt,
        ExecutionEventContext, StepAttemptStore, StoreError,
    };

    #[derive(Clone, Default)]
    struct PreviewStore {
        reads: Arc<AtomicUsize>,
    }

    impl StepAttemptStore for PreviewStore {
        fn child_attempt_admission_snapshot(
            &self,
            _command: &AdmitStepAttempt,
        ) -> Result<ChildAttemptAdmissionSnapshot, StoreError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(durable_snapshot())
        }

        fn admit_step_attempt(
            &self,
            _command: AdmitStepAttempt,
        ) -> Result<CommittedStepAttempt, StoreError> {
            panic!("read-only child admission preview must never call the writer")
        }
    }

    fn event(id: &str) -> ExecutionEventContext {
        ExecutionEventContext {
            event_id: id.to_owned(),
            origin_runtime_id: "runtime-a".to_owned(),
            hlc_timestamp: "2026-10-17T10:00:00Z".to_owned(),
            correlation_id: "correlation-a".to_owned(),
            causation_id: None,
            recorded_at: "2026-10-17T10:00:00Z".to_owned(),
        }
    }

    fn delegated_command() -> AdmitStepAttempt {
        AdmitStepAttempt {
            workspace_id: "workspace-a".to_owned(),
            owner_principal_id: "owner-a".to_owned(),
            request_id: "request-a".to_owned(),
            task_id: "task-a".to_owned(),
            step_id: "step-child".to_owned(),
            expected_task_version: 7,
            expected_step_version: 1,
            expected_plan_revision: 2,
            expected_task_spec_revision: 3,
            expected_previous_epoch: 0,
            attempt_id: "attempt-child".to_owned(),
            lease_id: "lease-child".to_owned(),
            parent_attempt_id: Some("attempt-parent".to_owned()),
            delegation_profile_id: Some("profile-review".to_owned()),
            delegation_profile_revision: Some(4),
            capability_grant_ids: Vec::new(),
            failover_class: "SAFE_PORTABLE".to_owned(),
            agent_binding_id: "binding-review".to_owned(),
            runtime_id: "runtime-a".to_owned(),
            runtime_incarnation_id: "incarnation-a".to_owned(),
            environment_id: "environment-a".to_owned(),
            dependency_plan_digest: format!("sha256:{}", "a".repeat(64)),
            fencing_token_digest: format!("sha256:{}", "b".repeat(64)),
            issuer_key_version: 1,
            lease_duration_ms: 60_000,
            budget_admission_decision_ref: "decision-a".to_owned(),
            expected_budget_reservation_ids: Vec::new(),
            events: storage_core::AttemptAdmissionEvents {
                attempt: event("event-attempt"),
                lease: event("event-lease"),
                step: event("event-step"),
                task_status: None,
                budget: Vec::new(),
            },
        }
    }

    fn request() -> ChildAdmissionRequest {
        ChildAdmissionRequest {
            workspace_id: "workspace-a".into(),
            task_id: "task-a".into(),
            step_id: "step-child".into(),
            parent_attempt_id: "attempt-parent".into(),
            child_attempt_id: "attempt-child".into(),
            expected_task_version: 7,
            expected_task_spec_revision: 3,
            expected_plan_revision: 2,
            profile_id: "profile-review".into(),
            profile_revision: 4,
            agent_binding_id: "binding-review".into(),
        }
    }

    fn snapshot() -> ChildAdmissionSnapshot {
        ChildAdmissionSnapshot {
            task: ChildTaskProjection {
                workspace_id: "workspace-a".into(),
                task_id: "task-a".into(),
                status: "RUNNING".into(),
                version: 7,
                task_spec_revision: 3,
                current_plan_revision: Some(2),
            },
            step: ChildStepProjection {
                task_id: "task-a".into(),
                step_id: "step-child".into(),
                plan_revision: 2,
                is_member_of_current_plan: true,
                status: "READY".into(),
                dependencies_satisfied: true,
                current_attempt_id: None,
            },
            parent: ParentAttemptProjection {
                attempt_id: "attempt-parent".into(),
                task_id: "task-a".into(),
                status: "RUNNING".into(),
                task_spec_revision: 3,
                plan_revision: 2,
                agent_session_attempt_id: Some("attempt-parent".into()),
                agent_session_status: Some("ACTIVE".into()),
                lease_attempt_id: Some("attempt-parent".into()),
                lease_status: Some("ACTIVE".into()),
                ancestry: vec!["attempt-root".into(), "attempt-parent".into()],
                profile_pin: Some(PinnedProfileProjection {
                    profile_id: "profile-parent".into(),
                    revision: 1,
                    max_host_delegation_depth: 1,
                }),
            },
            candidate_profile: CandidateProfileProjection {
                workspace_id: "workspace-a".into(),
                profile_id: "profile-review".into(),
                revision: 4,
                agent_binding_id: "binding-review".into(),
                enabled: true,
                archived: false,
                max_concurrency: 2,
            },
            coworker_pinned: true,
            coworker_enabled_profile_ids: vec!["profile-review".into()],
            task_max_concurrency: Some(6),
            coworker_max_concurrency: Some(4),
            active_task_child_attempts: 1,
            active_profile_attempts: 1,
        }
    }

    fn durable_snapshot() -> storage_core::ChildAttemptAdmissionSnapshot {
        use storage_core::{
            AgentSessionRecord, AttemptRecord, AttemptState, ChildAttemptAdmissionProofs,
            DelegationProfileAdmissionProjection, ExecutionLeaseRecord, ExecutionLeaseState,
            PlanRevisionRecord, PlannedStepRecord, StepRecord, TaskRecord, TaskSpecRevisionRecord,
        };
        storage_core::ChildAttemptAdmissionSnapshot {
            task: TaskRecord {
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
            },
            task_spec: TaskSpecRevisionRecord {
                task_id: "task-a".into(),
                workspace_id: "workspace-a".into(),
                revision: 3,
                parent_revisions: vec![2],
                objective: "Complete the accepted task".into(),
                task_category: Some("CODE".into()),
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
            },
            plan: PlanRevisionRecord {
                task_id: "task-a".into(),
                revision: 2,
                task_spec_revision: 3,
                produced_by_agent_session_id: "planning-session".into(),
                produced_by_attempt_id: None,
                steps: vec![PlannedStepRecord {
                    logical_key: "step-child".into(),
                    title: "Review source".into(),
                    objective: "Review source".into(),
                    depends_on_logical_keys: vec![],
                    required_capabilities: vec![],
                    acceptance_criteria: vec![],
                }],
                reason_for_revision: None,
                created_at: "2026-10-17T10:00:00Z".into(),
            },
            step: StepRecord {
                step_id: "step-child".into(),
                task_id: "task-a".into(),
                plan_revision: 2,
                logical_key: Some("step-child".into()),
                title: "Review source".into(),
                objective: "Review source".into(),
                dependencies: vec![],
                required_capabilities: vec![],
                acceptance_criteria: vec![],
                status: "READY".into(),
                current_attempt_id: None,
                created_at: "2026-10-17T10:00:00Z".into(),
                updated_at: "2026-10-17T10:00:00Z".into(),
                version: 1,
            },
            dependency_steps: vec![],
            parent_attempt: AttemptRecord {
                attempt_id: "attempt-parent".into(),
                task_id: "task-a".into(),
                step_id: "step-parent".into(),
                parent_attempt_id: None,
                agent_binding_id: "binding-lead".into(),
                delegation_profile_id: None,
                delegation_profile_revision: None,
                agent_session_id: Some("session-parent".into()),
                runtime_id: "runtime-a".into(),
                runtime_incarnation_id: "inc-a".into(),
                environment_id: "environment-a".into(),
                capability_grant_ids: vec![],
                execution_lease_id: Some("lease-parent".into()),
                failover_class: "SAFE_PORTABLE".into(),
                checkpoint_ref: None,
                status: AttemptState::Running,
                failure: None,
                started_at: Some("2026-10-17T10:00:00Z".into()),
                settled_at: None,
                created_at: "2026-10-17T10:00:00Z".into(),
                version: 1,
            },
            parent_step: Some(StepRecord {
                step_id: "step-parent".into(),
                task_id: "task-a".into(),
                plan_revision: 2,
                logical_key: Some("step-parent".into()),
                title: "Coordinate work".into(),
                objective: "Coordinate work".into(),
                dependencies: vec![],
                required_capabilities: vec![],
                acceptance_criteria: vec![],
                status: "RUNNING".into(),
                current_attempt_id: Some("attempt-parent".into()),
                created_at: "2026-10-17T10:00:00Z".into(),
                updated_at: "2026-10-17T10:00:00Z".into(),
                version: 1,
            }),
            parent_session: Some(AgentSessionRecord {
                agent_session_id: "session-parent".into(),
                workspace_id: "workspace-a".into(),
                scope_kind: "ATTEMPT_EXECUTION".into(),
                conversation_id: None,
                conversation_turn_id: None,
                task_id: Some("task-a".into()),
                task_spec_revision: Some(3),
                attempt_id: Some("attempt-parent".into()),
                agent_binding_id: "binding-lead".into(),
                endpoint_id: "endpoint-lead".into(),
                runtime_id: "runtime-a".into(),
                runtime_incarnation_id: "inc-a".into(),
                configuration_digest: None,
                harness_descriptor_digest: Some(format!("sha256:{}", "a".repeat(64))),
                status: "ACTIVE".into(),
                started_at: "2026-10-17T10:00:00Z".into(),
                last_event_at: None,
                closed_at: None,
                version: 1,
            }),
            parent_lease: Some(ExecutionLeaseRecord {
                lease_id: "lease-parent".into(),
                task_id: "task-a".into(),
                step_id: "step-parent".into(),
                attempt_id: "attempt-parent".into(),
                runtime_id: "runtime-a".into(),
                runtime_incarnation_id: "inc-a".into(),
                epoch: 1,
                issuer_key_version: 1,
                fencing_token_digest: format!("sha256:{}", "b".repeat(64)),
                state: ExecutionLeaseState::Active,
                checkpoint_ref: None,
                acquired_at: "2026-10-17T10:00:00Z".into(),
                renew_by: "2026-10-17T10:01:00Z".into(),
                expires_at: "2026-10-17T10:02:00Z".into(),
                version: 1,
            }),
            parent_lease_current: true,
            parent_ancestry: vec!["attempt-parent".into()],
            parent_profile_max_delegation_depth: None,
            candidate_profile: DelegationProfileAdmissionProjection {
                profile_id: "profile-review".into(),
                workspace_id: "workspace-a".into(),
                agent_binding_id: "binding-review".into(),
                current_revision: 4,
                requested_revision: 4,
                status: "ENABLED".into(),
                binding_enabled: true,
                max_concurrency: 2,
                max_host_delegation_depth: 1,
            },
            coworker_enabled_profile_ids: vec![],
            task_max_concurrency: Some(4),
            coworker_max_concurrency: None,
            active_task_child_attempts: 0,
            active_profile_attempts: 0,
            proofs: ChildAttemptAdmissionProofs::default(),
        }
    }

    #[test]
    fn current_parent_and_ready_step_use_the_task_wide_concurrency_cap() {
        assert_eq!(
            decide_child_attempt_admission(&request(), &snapshot()),
            ChildAdmissionDecision::Admit {
                child_depth: 2,
                effective_concurrency_limit: 4,
            }
        );
    }

    #[test]
    fn a_profile_limit_does_not_throttle_other_profiles_task_wide_capacity() {
        let mut state = snapshot();
        state.candidate_profile.max_concurrency = 1;
        state.active_task_child_attempts = 2;
        state.active_profile_attempts = 0;
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Admit {
                child_depth: 2,
                effective_concurrency_limit: 4,
            }
        );
    }

    #[test]
    fn stale_or_cross_workspace_parent_lineage_is_denied() {
        let mut state = snapshot();
        state.parent.ancestry[0] = "attempt-parent".into();
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::InvalidLineage)
        );

        let mut state = snapshot();
        state.parent.task_id = "task-other".into();
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::ParentAttemptNotCurrent)
        );

        let mut state = snapshot();
        state.parent.ancestry.push("attempt-child".into());
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::InvalidLineage)
        );
    }

    #[test]
    fn profile_revision_must_match_the_exact_enabled_binding_revision() {
        let mut state = snapshot();
        state.candidate_profile.revision -= 1;
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::ProfileRevisionMismatch)
        );

        let mut state = snapshot();
        state.coworker_enabled_profile_ids.clear();
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::ProfileNotEnabledForCoworker)
        );
    }

    #[test]
    fn ready_step_must_be_materialized_from_the_current_accepted_plan() {
        let mut state = snapshot();
        state.step.is_member_of_current_plan = false;
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::StalePlanOrSpec)
        );
    }

    #[test]
    fn ancestor_profile_depth_and_platform_depth_prevent_recursion() {
        let mut state = snapshot();
        state
            .parent
            .profile_pin
            .as_mut()
            .unwrap()
            .max_host_delegation_depth = 0;
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::DepthLimitReached)
        );

        let mut state = snapshot();
        state.parent.ancestry = vec![
            "attempt-root".into(),
            "attempt-middle".into(),
            "attempt-parent".into(),
        ];
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::DepthLimitReached)
        );
    }

    #[test]
    fn delegated_parent_without_its_immutable_profile_pin_cannot_delegate_again() {
        let mut state = snapshot();
        state.parent.profile_pin = None;
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::InvalidLineage)
        );

        let mut root = snapshot();
        root.parent.ancestry = vec!["attempt-root".into()];
        root.parent.attempt_id = "attempt-root".into();
        root.parent.agent_session_attempt_id = Some("attempt-root".into());
        root.parent.lease_attempt_id = Some("attempt-root".into());
        root.parent.profile_pin = None;
        let mut root_request = request();
        root_request.parent_attempt_id = "attempt-root".into();
        assert!(matches!(
            decide_child_attempt_admission(&root_request, &root),
            ChildAdmissionDecision::Admit { child_depth: 1, .. }
        ));
    }

    #[test]
    fn task_and_profile_concurrency_are_both_enforced() {
        let mut state = snapshot();
        state.coworker_max_concurrency = Some(2);
        state.active_task_child_attempts = 2;
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::ConcurrencyLimitReached)
        );

        let mut state = snapshot();
        state.active_profile_attempts = state.candidate_profile.max_concurrency;
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::ConcurrencyLimitReached)
        );
    }

    #[test]
    fn pinned_coworker_without_an_override_inherits_the_stricter_existing_limit() {
        let mut state = snapshot();
        state.coworker_max_concurrency = None;
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Admit {
                child_depth: 2,
                effective_concurrency_limit: 6,
            }
        );

        state.coworker_pinned = false;
        assert!(matches!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Admit { .. }
        ));
        state.coworker_max_concurrency = Some(3);
        assert_eq!(
            decide_child_attempt_admission(&request(), &state),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::InvalidInput)
        );
    }

    #[test]
    fn durable_projection_maps_exact_step_session_and_live_lease_before_policy_decision() {
        assert_eq!(
            decide_child_attempt_admission_from_durable_snapshot(&request(), &durable_snapshot()),
            ChildAdmissionDecision::Admit {
                child_depth: 1,
                effective_concurrency_limit: 4,
            }
        );

        let mut stale_session = durable_snapshot();
        stale_session.parent_session.as_mut().unwrap().task_id = Some("task-other".into());
        assert_eq!(
            decide_child_attempt_admission_from_durable_snapshot(&request(), &stale_session),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::ParentSessionNotCurrent)
        );

        let mut expired_lease = durable_snapshot();
        expired_lease.parent_lease_current = false;
        assert_eq!(
            decide_child_attempt_admission_from_durable_snapshot(&request(), &expired_lease),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::ParentLeaseNotCurrent)
        );
    }

    #[test]
    fn preview_does_not_report_admission_without_authority_proofs() {
        let durable = durable_snapshot();
        assert_eq!(
            preview_child_attempt_admission_from_durable_snapshot(&request(), &durable),
            ChildAdmissionDecision::Deny(ChildAdmissionDenial::AuthorityPrerequisitesUnproven)
        );
    }

    #[test]
    fn coordinator_exposes_durable_child_preview_without_reaching_attempt_writer() {
        let store = PreviewStore::default();
        let reads = store.reads.clone();
        let coordinator = crate::StepAttemptCoordinator::new(store);

        assert_eq!(
            coordinator.preview_delegated_admission(&delegated_command()),
            Ok(ChildAdmissionDecision::Deny(
                ChildAdmissionDenial::AuthorityPrerequisitesUnproven
            ))
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn coordinator_rejects_incomplete_parent_profile_pin_before_storage_read() {
        let store = PreviewStore::default();
        let reads = store.reads.clone();
        let coordinator = crate::StepAttemptCoordinator::new(store);
        let mut command = delegated_command();
        command.delegation_profile_revision = None;

        assert_eq!(
            coordinator.preview_delegated_admission(&command),
            Ok(ChildAdmissionDecision::Deny(
                ChildAdmissionDenial::InvalidInput
            ))
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }
}
