//! Advisory decisions and the atomic durable execution command boundary. Eligibility
//! is not a committed Attempt, lease, AgentSession, or proof of native execution.

use storage_core::{
    AdmitStepAttempt, AttemptBudgetAdmission, AttemptState, CommittedExecutionLeaseMutation,
    CommittedStepAttempt, ExecutionBudgetReservationState, ExecutionBudgetScope, ExecutionCheck,
    ExecutionEventContext, ExecutionLeaseCommand, ExecutionLeaseMutationSnapshot,
    ExecutionLeaseState, ExpireExecutionLease, LeaseReleasePhase, ReleaseExecutionLease,
    RenewExecutionLease, StepAttemptAdmissionSnapshot, StepAttemptStore, StoreError,
};

const MAX_LEASE_DURATION_MS: u64 = 300_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionDenial {
    InvalidCommand,
    UnsupportedGuarantee,
    UnconfirmedGuarantee,
    AuthorizationDenied,
    StaleVersion,
    StalePlanOrSpec,
    TaskNotRunnable,
    StepNotReady,
    DependencyNotComplete,
    LeaseConflict,
    StaleFence,
    RuntimeIdentityMismatch,
    PriorAttemptUnsettled,
    InvalidLeaseState,
    LeaseExpired,
    UnsafeRelease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionDecision<T> {
    Eligible(T),
    Denied(ExecutionDenial),
}

/// Return the proposed next epoch, never a durable allocation or authority token.
pub fn decide_attempt_admission(
    command: &AdmitStepAttempt,
    snapshot: &StepAttemptAdmissionSnapshot,
) -> ExecutionDecision<u64> {
    if !valid_admission(command) {
        return denied(ExecutionDenial::InvalidCommand);
    }
    let task = &snapshot.task;
    let step = &snapshot.step;
    let plan = &snapshot.plan;
    if task.workspace_id != command.workspace_id
        || task.task_id != command.task_id
        || step.step_id != command.step_id
        || step.task_id != task.task_id
        || plan.task_id != task.task_id
    {
        return denied(ExecutionDenial::AuthorizationDenied);
    }
    if task.version != command.expected_task_version
        || step.version != command.expected_step_version
    {
        return denied(ExecutionDenial::StaleVersion);
    }
    if !matches!(task.status.as_str(), "READY" | "RUNNING") {
        return denied(ExecutionDenial::TaskNotRunnable);
    }
    if task.current_plan_revision != Some(command.expected_plan_revision)
        || plan.revision != command.expected_plan_revision
        || step.plan_revision != plan.revision
        || plan.task_spec_revision != command.expected_task_spec_revision
        || task.current_spec_revision != command.expected_task_spec_revision
        || !plan
            .steps
            .iter()
            .any(|planned| step.logical_key.as_deref() == Some(planned.logical_key.as_str()))
    {
        return denied(ExecutionDenial::StalePlanOrSpec);
    }
    if step.status != "READY" {
        return denied(ExecutionDenial::StepNotReady);
    }
    if snapshot.dependency_steps.len() != step.dependencies.len() {
        return denied(ExecutionDenial::DependencyNotComplete);
    }
    let mut dependencies = std::collections::HashSet::new();
    for dependency in &snapshot.dependency_steps {
        if dependency.task_id != task.task_id
            || dependency.plan_revision != plan.revision
            || dependency.status != "COMPLETED"
            || !step.dependencies.contains(&dependency.step_id)
            || !dependencies.insert(dependency.step_id.as_str())
        {
            return denied(ExecutionDenial::DependencyNotComplete);
        }
    }
    if snapshot.conflicting_lease.as_ref().is_some_and(|lease| {
        matches!(
            lease.state,
            ExecutionLeaseState::Active | ExecutionLeaseState::Releasing
        )
    }) {
        // Even wall-clock expiry does not replace expiry/reconciliation at authority.
        return denied(ExecutionDenial::LeaseConflict);
    }
    if snapshot.latest_step_epoch != command.expected_previous_epoch {
        return denied(ExecutionDenial::StaleFence);
    }
    let Some(next_epoch) = snapshot.latest_step_epoch.checked_add(1) else {
        return denied(ExecutionDenial::StaleFence);
    };
    if snapshot.environment_id != command.environment_id
        || snapshot.environment_runtime_id != command.runtime_id
        || snapshot.environment_runtime_incarnation_id != command.runtime_incarnation_id
    {
        return denied(ExecutionDenial::RuntimeIdentityMismatch);
    }
    if snapshot.selected_agent_binding_id != command.agent_binding_id
        || snapshot.dependency_plan_digest != command.dependency_plan_digest
    {
        return denied(ExecutionDenial::UnconfirmedGuarantee);
    }
    if let Err(reason) = validate_budget_admission(command, snapshot) {
        return denied(reason);
    }
    if (task.status == "READY") != command.events.task_status.is_some() {
        return denied(ExecutionDenial::InvalidCommand);
    }
    let checks = &snapshot.checks;
    if (step.current_attempt_id.is_some() || snapshot.latest_step_epoch > 0)
        && (checks.prior_attempt_settled != ExecutionCheck::Confirmed
            || checks.explicit_recovery_authorized != ExecutionCheck::Confirmed)
    {
        return denied(ExecutionDenial::PriorAttemptUnsettled);
    }
    for check in [
        checks.owner_authorized,
        checks.workspace_active,
        checks.runtime_ready,
        checks.runtime_executor_binding,
        checks.agent_compatible,
        checks.environment_available,
        checks.inputs_available,
        checks.policy_grants_budget,
        checks.dependency_plan_current,
        checks.effects_reconciled,
        checks.mutation_fencing,
        checks.process_containment,
        checks.private_credential_delivery,
    ] {
        if let Err(reason) = require_confirmed(check) {
            return denied(reason);
        }
    }
    ExecutionDecision::Eligible(next_epoch)
}

/// Renewal retains the exact Attempt/incarnation/epoch/digest. An expired lease
/// cannot be revived even if its persisted state has not yet been reconciled.
pub fn decide_lease_renewal(
    command: &RenewExecutionLease,
    snapshot: &ExecutionLeaseMutationSnapshot,
) -> ExecutionDecision<u64> {
    if !valid_duration(command.lease_duration_ms)
        || !valid_event(&command.event, &command.context.runtime_id)
    {
        return denied(ExecutionDenial::InvalidCommand);
    }
    if let Err(reason) = validate_lease_context(&command.context, snapshot) {
        return denied(reason);
    }
    if !matches!(snapshot.task.status.as_str(), "READY" | "RUNNING") {
        return denied(ExecutionDenial::TaskNotRunnable);
    }
    if snapshot.lease.state != ExecutionLeaseState::Active {
        return denied(ExecutionDenial::InvalidLeaseState);
    }
    if snapshot.now_unix_ms >= snapshot.expires_at_unix_ms {
        return denied(ExecutionDenial::LeaseExpired);
    }
    if snapshot.now_unix_ms >= snapshot.renew_by_unix_ms {
        return denied(ExecutionDenial::LeaseExpired);
    }
    if matches!(
        snapshot.attempt.status,
        AttemptState::Completed
            | AttemptState::Failed
            | AttemptState::Abandoned
            | AttemptState::Cancelled
            | AttemptState::CancelRequested
    ) {
        return denied(ExecutionDenial::InvalidLeaseState);
    }
    match snapshot.now_unix_ms.checked_add(command.lease_duration_ms) {
        Some(expires_at) if expires_at > snapshot.expires_at_unix_ms => {
            ExecutionDecision::Eligible(expires_at)
        }
        _ => denied(ExecutionDenial::InvalidCommand),
    }
}

pub fn decide_lease_release(
    command: &ReleaseExecutionLease,
    snapshot: &ExecutionLeaseMutationSnapshot,
) -> ExecutionDecision<ExecutionLeaseState> {
    if !valid_event(&command.event, &command.context.runtime_id) {
        return denied(ExecutionDenial::InvalidCommand);
    }
    if let Err(reason) = validate_lease_context(&command.context, snapshot) {
        return denied(reason);
    }
    match (command.phase, snapshot.lease.state) {
        (LeaseReleasePhase::Begin, ExecutionLeaseState::Active)
            if snapshot.now_unix_ms < snapshot.expires_at_unix_ms =>
        {
            ExecutionDecision::Eligible(ExecutionLeaseState::Releasing)
        }
        (LeaseReleasePhase::Begin, ExecutionLeaseState::Active) => {
            denied(ExecutionDenial::LeaseExpired)
        }
        (LeaseReleasePhase::Complete, ExecutionLeaseState::Releasing) => {
            if [
                snapshot.writer_quiescence,
                snapshot.invocations_settled,
                snapshot.effects_reconciled,
            ]
            .iter()
            .any(|check| *check != ExecutionCheck::Confirmed)
            {
                return denied(ExecutionDenial::UnsafeRelease);
            }
            ExecutionDecision::Eligible(ExecutionLeaseState::Released)
        }
        _ => denied(ExecutionDenial::InvalidLeaseState),
    }
}

/// Recovery eligibility is intentionally narrow and does not need the old Runtime
/// credential. The SQLite transaction must independently recompute the same facts.
pub fn decide_lease_expiry(
    command: &ExpireExecutionLease,
    lease: &storage_core::ExecutionLeaseRecord,
    now_unix_ms: u64,
    current_incarnation_id: Option<&str>,
    runtime_revoked: bool,
) -> ExecutionDecision<ExecutionLeaseState> {
    if [
        &command.workspace_id,
        &command.owner_principal_id,
        &command.request_id,
        &command.task_id,
        &command.step_id,
        &command.attempt_id,
        &command.lease_id,
        &command.recovery_runtime_id,
        &command.recovery_runtime_incarnation_id,
    ]
    .iter()
    .any(|id| !valid_id(id))
        || !valid_event(&command.events.lease, &command.recovery_runtime_id)
        || !valid_event(&command.events.attempt, &command.recovery_runtime_id)
        || !valid_event(&command.events.step, &command.recovery_runtime_id)
        || !valid_event(&command.events.task, &command.recovery_runtime_id)
        || command.expected_task_version == 0
        || command.expected_step_version == 0
        || command.expected_attempt_version == 0
        || command.expected_lease_version == 0
        || !valid_recovery_events(&command.events)
    {
        return denied(ExecutionDenial::InvalidCommand);
    }
    if lease.lease_id != command.lease_id
        || lease.task_id != command.task_id
        || lease.step_id != command.step_id
        || lease.attempt_id != command.attempt_id
    {
        return denied(ExecutionDenial::StaleFence);
    }
    if !matches!(
        lease.state,
        ExecutionLeaseState::Active | ExecutionLeaseState::Releasing
    ) {
        return denied(ExecutionDenial::InvalidLeaseState);
    }
    let incarnation_fenced = runtime_revoked
        || current_incarnation_id.is_some_and(|current| current != lease.runtime_incarnation_id);
    if now_unix_ms < parse_unix_ms(&lease.expires_at).unwrap_or(u64::MAX) && !incarnation_fenced {
        return denied(ExecutionDenial::LeaseExpired);
    }
    ExecutionDecision::Eligible(ExecutionLeaseState::Expired)
}

fn valid_recovery_events(events: &storage_core::ExpiredAttemptEvents) -> bool {
    let values = [&events.lease, &events.attempt, &events.step, &events.task];
    let first = values[0];
    let mut ids = std::collections::HashSet::new();
    values.into_iter().all(|event| {
        ids.insert(event.event_id.as_str())
            && event.correlation_id == first.correlation_id
            && event.hlc_timestamp == first.hlc_timestamp
            && event.recorded_at == first.recorded_at
    })
}

fn validate_lease_context(
    command: &ExecutionLeaseCommand,
    snapshot: &ExecutionLeaseMutationSnapshot,
) -> Result<(), ExecutionDenial> {
    if [
        &command.workspace_id,
        &command.request_id,
        &command.task_id,
        &command.step_id,
        &command.attempt_id,
        &command.lease_id,
        &command.runtime_id,
        &command.runtime_incarnation_id,
        &command.private_control_authorization_ref,
    ]
    .iter()
    .any(|id| !valid_id(id))
        || command.expected_task_version == 0
        || command.expected_attempt_version == 0
        || command.expected_lease_version == 0
        || command.expected_epoch == 0
    {
        return Err(ExecutionDenial::InvalidCommand);
    }
    for check in [
        snapshot.authenticated_runtime,
        snapshot.credential_verified,
        snapshot.runtime_current,
        snapshot.mutation_fencing,
    ] {
        require_confirmed(check)?;
    }
    let attempt = &snapshot.attempt;
    let lease = &snapshot.lease;
    if snapshot.task.workspace_id != command.workspace_id
        || snapshot.task.task_id != command.task_id
        || snapshot.step.task_id != command.task_id
        || snapshot.step.step_id != command.step_id
        || snapshot.step.current_attempt_id.as_deref() != Some(command.attempt_id.as_str())
        || attempt.task_id != command.task_id
        || attempt.step_id != command.step_id
        || attempt.attempt_id != command.attempt_id
        || attempt.execution_lease_id.as_deref() != Some(command.lease_id.as_str())
        || lease.task_id != command.task_id
        || lease.step_id != command.step_id
        || lease.attempt_id != command.attempt_id
        || lease.lease_id != command.lease_id
    {
        return Err(ExecutionDenial::StaleFence);
    }
    if attempt.runtime_id != command.runtime_id
        || lease.runtime_id != command.runtime_id
        || attempt.runtime_incarnation_id != command.runtime_incarnation_id
        || lease.runtime_incarnation_id != command.runtime_incarnation_id
    {
        return Err(ExecutionDenial::RuntimeIdentityMismatch);
    }
    if snapshot.task.version != command.expected_task_version
        || attempt.version != command.expected_attempt_version
        || lease.version != command.expected_lease_version
    {
        return Err(ExecutionDenial::StaleVersion);
    }
    if lease.epoch != command.expected_epoch || snapshot.latest_step_epoch != command.expected_epoch
    {
        return Err(ExecutionDenial::StaleFence);
    }
    Ok(())
}

fn require_confirmed(check: ExecutionCheck) -> Result<(), ExecutionDenial> {
    match check {
        ExecutionCheck::Confirmed => Ok(()),
        ExecutionCheck::Denied => Err(ExecutionDenial::AuthorizationDenied),
        ExecutionCheck::Unknown => Err(ExecutionDenial::UnconfirmedGuarantee),
        ExecutionCheck::Unsupported => Err(ExecutionDenial::UnsupportedGuarantee),
    }
}
fn denied<T>(reason: ExecutionDenial) -> ExecutionDecision<T> {
    ExecutionDecision::Denied(reason)
}
fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
fn valid_digest(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
fn valid_duration(duration: u64) -> bool {
    duration > 0 && duration <= MAX_LEASE_DURATION_MS
}
fn valid_admission(command: &AdmitStepAttempt) -> bool {
    [
        &command.workspace_id,
        &command.owner_principal_id,
        &command.request_id,
        &command.task_id,
        &command.step_id,
        &command.attempt_id,
        &command.lease_id,
        &command.agent_binding_id,
        &command.runtime_id,
        &command.runtime_incarnation_id,
        &command.environment_id,
    ]
    .iter()
    .all(|id| valid_id(id))
        && command.expected_task_version > 0
        && command.expected_step_version > 0
        && command.expected_plan_revision > 0
        && command.expected_task_spec_revision > 0
        && command.issuer_key_version > 0
        && valid_digest(&command.dependency_plan_digest)
        && valid_digest(&command.fencing_token_digest)
        && valid_duration(command.lease_duration_ms)
        && valid_id(&command.budget_admission_decision_ref)
        && valid_delegation_identity(command)
        && matches!(
            command.failover_class.as_str(),
            "SAFE_PORTABLE" | "REPLAYABLE" | "HANDOFF_REQUIRED" | "LOCAL_BOUND"
        )
        && command.capability_grant_ids.len() <= 64
        && command.capability_grant_ids.iter().all(|id| valid_id(id))
        && command
            .capability_grant_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            == command.capability_grant_ids.len()
        && command.expected_budget_reservation_ids.len() <= 32
        && command
            .expected_budget_reservation_ids
            .iter()
            .all(|id| valid_id(id))
        && valid_admission_events(command)
}

fn valid_delegation_identity(command: &AdmitStepAttempt) -> bool {
    match (
        &command.parent_attempt_id,
        &command.delegation_profile_id,
        command.delegation_profile_revision,
    ) {
        (None, None, None) => true,
        (Some(parent), Some(profile), Some(revision)) => {
            valid_id(parent) && valid_id(profile) && revision > 0
        }
        _ => false,
    }
}

fn parse_unix_ms(value: &str) -> Option<u64> {
    let parsed =
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()?;
    u64::try_from(parsed.unix_timestamp_nanos().checked_div(1_000_000)?).ok()
}

fn valid_event(event: &ExecutionEventContext, runtime: &str) -> bool {
    valid_id(&event.event_id)
        && event.origin_runtime_id == runtime
        && valid_id(&event.hlc_timestamp)
        && valid_id(&event.correlation_id)
        && valid_id(&event.recorded_at)
        && event.causation_id.as_ref().is_none_or(|id| valid_id(id))
        && time::OffsetDateTime::parse(
            &event.hlc_timestamp,
            &time::format_description::well_known::Rfc3339,
        )
        .is_ok()
        && time::OffsetDateTime::parse(
            &event.recorded_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_ok()
}
fn valid_admission_events(command: &AdmitStepAttempt) -> bool {
    if command.events.budget.len() > 32 {
        return false;
    }
    let first = &command.events.attempt;
    let mut ids = std::collections::HashSet::new();
    [
        &command.events.attempt,
        &command.events.lease,
        &command.events.step,
    ]
    .into_iter()
    .chain(command.events.task_status.iter())
    .chain(command.events.budget.iter())
    .all(|event| {
        valid_event(event, &command.runtime_id)
            && ids.insert(event.event_id.as_str())
            && event.correlation_id == first.correlation_id
            && event.recorded_at == first.recorded_at
            && event.hlc_timestamp == first.hlc_timestamp
    })
}
fn validate_budget_admission(
    command: &AdmitStepAttempt,
    snapshot: &StepAttemptAdmissionSnapshot,
) -> Result<(), ExecutionDenial> {
    let (decision_ref, reservations) = match &snapshot.budget_admission {
        AttemptBudgetAdmission::Unsupported => return Err(ExecutionDenial::UnsupportedGuarantee),
        AttemptBudgetAdmission::NotRequired { decision_ref } => (decision_ref, &[][..]),
        AttemptBudgetAdmission::Required {
            decision_ref,
            reservations,
        } if !reservations.is_empty() && reservations.len() <= 32 => {
            (decision_ref, reservations.as_slice())
        }
        _ => return Err(ExecutionDenial::UnconfirmedGuarantee),
    };
    if decision_ref != &command.budget_admission_decision_ref
        || reservations.len() != command.expected_budget_reservation_ids.len()
        || reservations.len() != command.events.budget.len()
    {
        return Err(ExecutionDenial::UnconfirmedGuarantee);
    }
    let mut ids = std::collections::HashSet::new();
    for reservation in reservations {
        let scoped = match reservation.budget_scope {
            ExecutionBudgetScope::Task => {
                reservation.task_id.as_deref() == Some(command.task_id.as_str())
                    && reservation.environment_id.is_none()
            }
            ExecutionBudgetScope::Environment => {
                reservation.environment_id.as_deref() == Some(command.environment_id.as_str())
                    && reservation.task_id.is_none()
            }
        };
        if !scoped
            || reservation.workspace_id != command.workspace_id
            || reservation.attempt_id != command.attempt_id
            || reservation.state != ExecutionBudgetReservationState::Reserved
            || !valid_id(&reservation.reservation_id)
            || !ids.insert(reservation.reservation_id.as_str())
            || !command
                .expected_budget_reservation_ids
                .contains(&reservation.reservation_id)
            || !valid_id(&reservation.metric)
            || !valid_id(&reservation.unit)
            || !valid_quantity(&reservation.quantity)
            || reservation.currency.as_ref().is_some_and(|currency| {
                currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase())
            })
            || (reservation.metric.eq_ignore_ascii_case("cost") && reservation.currency.is_none())
        {
            return Err(ExecutionDenial::UnconfirmedGuarantee);
        }
    }
    Ok(())
}
fn valid_quantity(quantity: &str) -> bool {
    if quantity.is_empty() || quantity.len() > 64 {
        return false;
    }
    let parts = quantity.split('.').collect::<Vec<_>>();
    parts.len() <= 2
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        && quantity.bytes().any(|byte| (b'1'..=b'9').contains(&byte))
}

pub struct StepAttemptCoordinator<S> {
    store: S,
}
impl<S: StepAttemptStore> StepAttemptCoordinator<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
    pub fn preview_admission(
        &self,
        command: &AdmitStepAttempt,
    ) -> Result<ExecutionDecision<u64>, StoreError> {
        let snapshot = self.store.step_attempt_admission_snapshot(command)?;
        Ok(decide_attempt_admission(command, &snapshot))
    }
    /// The transaction checks the authenticated idempotency receipt before mutable
    /// admission checks. A lost-response replay must recover the committed pair.
    pub fn admit(&self, command: AdmitStepAttempt) -> Result<CommittedStepAttempt, StoreError> {
        if !valid_admission(&command) {
            return Err(StoreError::Invalid(
                "Attempt admission command is invalid".to_owned(),
            ));
        }
        self.store.admit_step_attempt(command)
    }
    pub fn renew(
        &self,
        command: RenewExecutionLease,
    ) -> Result<CommittedExecutionLeaseMutation, StoreError> {
        if !valid_duration(command.lease_duration_ms)
            || !valid_event(&command.event, &command.context.runtime_id)
        {
            return Err(StoreError::Invalid(
                "lease renewal command is invalid".to_owned(),
            ));
        }
        self.store.renew_execution_lease(command)
    }
    pub fn release(
        &self,
        command: ReleaseExecutionLease,
    ) -> Result<CommittedExecutionLeaseMutation, StoreError> {
        if !valid_event(&command.event, &command.context.runtime_id) {
            return Err(StoreError::Invalid(
                "lease release command is invalid".to_owned(),
            ));
        }
        self.store.release_execution_lease(command)
    }

    /// Expiry recovery is performed by the current authority, not the vanished
    /// Runtime. The SQLite store supplies its own clock and rechecks fencing.
    pub fn expire(
        &self,
        command: ExpireExecutionLease,
    ) -> Result<CommittedExecutionLeaseMutation, StoreError> {
        if [
            &command.workspace_id,
            &command.owner_principal_id,
            &command.request_id,
            &command.task_id,
            &command.step_id,
            &command.attempt_id,
            &command.lease_id,
            &command.recovery_runtime_id,
            &command.recovery_runtime_incarnation_id,
        ]
        .iter()
        .any(|id| !valid_id(id))
            || !valid_event(&command.events.lease, &command.recovery_runtime_id)
            || !valid_event(&command.events.attempt, &command.recovery_runtime_id)
            || !valid_event(&command.events.step, &command.recovery_runtime_id)
            || !valid_event(&command.events.task, &command.recovery_runtime_id)
            || command.expected_task_version == 0
            || command.expected_step_version == 0
            || command.expected_attempt_version == 0
            || command.expected_lease_version == 0
            || !valid_recovery_events(&command.events)
        {
            return Err(StoreError::Invalid(
                "lease expiry command is invalid".to_owned(),
            ));
        }
        self.store.expire_execution_lease(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str) -> ExecutionEventContext {
        ExecutionEventContext {
            event_id: id.to_owned(),
            origin_runtime_id: "runtime".to_owned(),
            hlc_timestamp: "2026-10-08T12:00:00Z".to_owned(),
            correlation_id: "correlation".to_owned(),
            causation_id: None,
            recorded_at: "2026-10-08T12:00:00Z".to_owned(),
        }
    }
    fn admission_fixture() -> (AdmitStepAttempt, StepAttemptAdmissionSnapshot) {
        use serde_json::json;
        use storage_core::{
            AttemptAdmissionChecks, PlanRevisionRecord, PlannedStepRecord, StepRecord, TaskRecord,
        };
        let digest = format!("sha256:{}", "a".repeat(64));
        let command = AdmitStepAttempt {
            workspace_id: "workspace".to_owned(),
            owner_principal_id: "owner".to_owned(),
            request_id: "request".to_owned(),
            task_id: "task".to_owned(),
            step_id: "step".to_owned(),
            expected_task_version: 3,
            expected_step_version: 1,
            expected_plan_revision: 1,
            expected_task_spec_revision: 1,
            expected_previous_epoch: 0,
            attempt_id: "attempt".to_owned(),
            lease_id: "lease".to_owned(),
            parent_attempt_id: None,
            delegation_profile_id: None,
            delegation_profile_revision: None,
            capability_grant_ids: vec![],
            failover_class: "SAFE_PORTABLE".to_owned(),
            agent_binding_id: "agent".to_owned(),
            runtime_id: "runtime".to_owned(),
            runtime_incarnation_id: "incarnation".to_owned(),
            environment_id: "environment".to_owned(),
            dependency_plan_digest: digest.clone(),
            fencing_token_digest: digest.clone(),
            issuer_key_version: 1,
            lease_duration_ms: 1000,
            budget_admission_decision_ref: "budget-decision".to_owned(),
            expected_budget_reservation_ids: vec![],
            events: storage_core::AttemptAdmissionEvents {
                attempt: event("attempt-event"),
                lease: event("lease-event"),
                step: event("step-event"),
                task_status: None,
                budget: vec![],
            },
        };
        let task = TaskRecord {
            task_id: "task".to_owned(),
            workspace_id: "workspace".to_owned(),
            conversation_id: None,
            current_spec_revision: 1,
            current_plan_revision: Some(1),
            status: "RUNNING".to_owned(),
            resume_status: None,
            routine_id: None,
            routine_revision: None,
            automation_id: None,
            automation_occurrence_id: None,
            origin_coworker_id: None,
            origin_coworker_revision: None,
            lead_agent_binding_id: "agent".to_owned(),
            blocking_conditions: vec![],
            priority: "NORMAL".to_owned(),
            created_by: json!({"kind":"USER", "principal_id":"owner"}),
            created_at: "timestamp".to_owned(),
            updated_at: "timestamp".to_owned(),
            completed_at: None,
            version: 3,
        };
        let planned = PlannedStepRecord {
            logical_key: "work".to_owned(),
            title: "Work".to_owned(),
            objective: "Work".to_owned(),
            depends_on_logical_keys: vec![],
            required_capabilities: vec![],
            acceptance_criteria: vec![],
        };
        let plan = PlanRevisionRecord {
            task_id: "task".to_owned(),
            revision: 1,
            task_spec_revision: 1,
            produced_by_agent_session_id: "planner".to_owned(),
            produced_by_attempt_id: None,
            steps: vec![planned],
            reason_for_revision: None,
            created_at: "timestamp".to_owned(),
        };
        let step = StepRecord {
            step_id: "step".to_owned(),
            task_id: "task".to_owned(),
            plan_revision: 1,
            logical_key: Some("work".to_owned()),
            title: "Work".to_owned(),
            objective: "Work".to_owned(),
            dependencies: vec![],
            required_capabilities: vec![],
            acceptance_criteria: vec![],
            status: "READY".to_owned(),
            current_attempt_id: None,
            created_at: "timestamp".to_owned(),
            updated_at: "timestamp".to_owned(),
            version: 1,
        };
        let confirmed = ExecutionCheck::Confirmed;
        let checks = AttemptAdmissionChecks {
            owner_authorized: confirmed,
            workspace_active: confirmed,
            runtime_ready: confirmed,
            runtime_executor_binding: confirmed,
            agent_compatible: confirmed,
            environment_available: confirmed,
            inputs_available: confirmed,
            policy_grants_budget: confirmed,
            dependency_plan_current: confirmed,
            effects_reconciled: confirmed,
            mutation_fencing: confirmed,
            process_containment: confirmed,
            private_credential_delivery: confirmed,
            prior_attempt_settled: ExecutionCheck::Unknown,
            explicit_recovery_authorized: ExecutionCheck::Unknown,
        };
        (
            command,
            StepAttemptAdmissionSnapshot {
                task,
                plan,
                step,
                dependency_steps: vec![],
                latest_step_epoch: 0,
                conflicting_lease: None,
                checks,
                environment_runtime_id: "runtime".to_owned(),
                environment_runtime_incarnation_id: "incarnation".to_owned(),
                environment_id: "environment".to_owned(),
                selected_agent_binding_id: "agent".to_owned(),
                dependency_plan_digest: digest,
                budget_admission: AttemptBudgetAdmission::NotRequired {
                    decision_ref: "budget-decision".to_owned(),
                },
            },
        )
    }

    #[test]
    fn admission_requires_accepted_plan_current_identity_and_confirmed_guarantees() {
        let (command, snapshot) = admission_fixture();
        assert_eq!(
            decide_attempt_admission(&command, &snapshot),
            ExecutionDecision::Eligible(1)
        );
        let mut changed = snapshot.clone();
        changed.task.current_plan_revision = None;
        assert_eq!(
            decide_attempt_admission(&command, &changed),
            denied(ExecutionDenial::StalePlanOrSpec)
        );
        changed = snapshot.clone();
        changed.task.status = "CANCEL_REQUESTED".to_owned();
        assert_eq!(
            decide_attempt_admission(&command, &changed),
            denied(ExecutionDenial::TaskNotRunnable)
        );
        changed = snapshot.clone();
        changed.environment_runtime_incarnation_id = "replacement".to_owned();
        assert_eq!(
            decide_attempt_admission(&command, &changed),
            denied(ExecutionDenial::RuntimeIdentityMismatch)
        );
        changed = snapshot.clone();
        changed.checks.mutation_fencing = ExecutionCheck::Unsupported;
        assert_eq!(
            decide_attempt_admission(&command, &changed),
            denied(ExecutionDenial::UnsupportedGuarantee)
        );
        changed = snapshot.clone();
        changed.step.current_attempt_id = Some("previous".to_owned());
        assert_eq!(
            decide_attempt_admission(&command, &changed),
            denied(ExecutionDenial::PriorAttemptUnsettled)
        );
        changed = snapshot.clone();
        changed.step.dependencies = vec!["missing".to_owned()];
        assert_eq!(
            decide_attempt_admission(&command, &changed),
            denied(ExecutionDenial::DependencyNotComplete)
        );
    }

    #[test]
    fn epochs_never_wrap_or_silently_accept_stale_preconditions() {
        let (mut command, mut snapshot) = admission_fixture();
        snapshot.latest_step_epoch = 4;
        assert_eq!(
            decide_attempt_admission(&command, &snapshot),
            denied(ExecutionDenial::StaleFence)
        );
        command.expected_previous_epoch = u64::MAX;
        snapshot.latest_step_epoch = u64::MAX;
        assert_eq!(
            decide_attempt_admission(&command, &snapshot),
            denied(ExecutionDenial::StaleFence)
        );
    }

    #[test]
    fn budget_and_event_context_cannot_be_omitted_or_rebound() {
        let (mut command, mut snapshot) = admission_fixture();
        snapshot.budget_admission = AttemptBudgetAdmission::Unsupported;
        assert_eq!(
            decide_attempt_admission(&command, &snapshot),
            denied(ExecutionDenial::UnsupportedGuarantee)
        );
        snapshot.budget_admission = AttemptBudgetAdmission::NotRequired {
            decision_ref: "different-budget".to_owned(),
        };
        assert_eq!(
            decide_attempt_admission(&command, &snapshot),
            denied(ExecutionDenial::UnconfirmedGuarantee)
        );
        snapshot.budget_admission = AttemptBudgetAdmission::NotRequired {
            decision_ref: "budget-decision".to_owned(),
        };
        command.events.lease.event_id = command.events.attempt.event_id.clone();
        assert_eq!(
            decide_attempt_admission(&command, &snapshot),
            denied(ExecutionDenial::InvalidCommand)
        );
        assert!(!valid_quantity("0"));
        assert!(!valid_quantity("NaN"));
        assert!(!valid_quantity("1e9"));
        assert!(valid_quantity("12.50"));
    }

    #[test]
    fn unknown_and_unsupported_guarantees_never_authorize_execution() {
        assert_eq!(
            require_confirmed(ExecutionCheck::Unknown),
            Err(ExecutionDenial::UnconfirmedGuarantee)
        );
        assert_eq!(
            require_confirmed(ExecutionCheck::Unsupported),
            Err(ExecutionDenial::UnsupportedGuarantee)
        );
        assert_eq!(
            require_confirmed(ExecutionCheck::Denied),
            Err(ExecutionDenial::AuthorizationDenied)
        );
    }
    #[test]
    fn credential_metadata_and_lease_durations_are_bounded() {
        assert!(!valid_digest("raw-credential"));
        assert!(!valid_digest(&format!("sha256:{}", "G".repeat(64))));
        assert!(valid_digest(&format!("sha256:{}", "a".repeat(64))));
        assert!(!valid_duration(0));
        assert!(!valid_duration(MAX_LEASE_DURATION_MS + 1));
    }

    #[test]
    fn delegated_attempt_identity_is_all_or_none_and_grants_are_unique() {
        let (mut command, _) = admission_fixture();
        assert!(valid_admission(&command));
        command.parent_attempt_id = Some("parent".to_owned());
        assert!(!valid_admission(&command));
        command.delegation_profile_id = Some("profile".to_owned());
        command.delegation_profile_revision = Some(1);
        assert!(valid_admission(&command));
        command.capability_grant_ids = vec!["grant".to_owned(), "grant".to_owned()];
        assert!(!valid_admission(&command));
    }

    #[test]
    fn recovery_expiry_requires_canonical_event_times() {
        let mut events = storage_core::ExpiredAttemptEvents {
            lease: event("lease"),
            attempt: event("attempt"),
            step: event("step"),
            task: event("task"),
        };
        assert!(valid_recovery_events(&events));
        events.task.recorded_at = "not-a-time".to_owned();
        assert!(!valid_event(&events.task, "runtime"));
    }
    #[test]
    fn stores_are_unsupported_unless_the_atomic_contract_is_implemented() {
        struct UnsupportedStore;
        impl StepAttemptStore for UnsupportedStore {}
        let context = ExecutionLeaseCommand {
            workspace_id: "workspace".to_owned(),
            request_id: "request".to_owned(),
            task_id: "task".to_owned(),
            step_id: "step".to_owned(),
            attempt_id: "attempt".to_owned(),
            lease_id: "lease".to_owned(),
            runtime_id: "runtime".to_owned(),
            runtime_incarnation_id: "incarnation".to_owned(),
            expected_task_version: 1,
            expected_attempt_version: 1,
            expected_lease_version: 1,
            expected_epoch: 1,
            private_control_authorization_ref: "private-auth-receipt".to_owned(),
        };
        assert!(
            StepAttemptCoordinator::new(UnsupportedStore)
                .renew(RenewExecutionLease {
                    context,
                    lease_duration_ms: 1000,
                    event: event("renew-event")
                })
                .is_err()
        );
    }
}
