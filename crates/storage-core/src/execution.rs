//! Atomic Step Attempt / ExecutionLease transaction contracts. No implementation
//! may claim support from a process start or caller-supplied eligibility flags.

use crate::{DomainEvent, PlanRevisionRecord, StepRecord, StoreError, TaskRecord};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttemptState {
    Created,
    Preparing,
    Running,
    WaitingApproval,
    WaitingResource,
    Checkpointing,
    Completed,
    Failed,
    Abandoned,
    CancelRequested,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionLeaseState {
    Active,
    Releasing,
    Released,
    Expired,
    Revoked,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptRecord {
    pub attempt_id: String,
    pub task_id: String,
    pub step_id: String,
    pub parent_attempt_id: Option<String>,
    pub agent_binding_id: String,
    pub delegation_profile_id: Option<String>,
    pub delegation_profile_revision: Option<u64>,
    pub agent_session_id: Option<String>,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub environment_id: String,
    pub capability_grant_ids: Vec<String>,
    pub execution_lease_id: Option<String>,
    pub failover_class: String,
    pub checkpoint_ref: Option<Value>,
    pub status: AttemptState,
    pub failure: Option<Value>,
    pub started_at: Option<String>,
    pub settled_at: Option<String>,
    pub created_at: String,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLeaseRecord {
    pub lease_id: String,
    pub task_id: String,
    pub step_id: String,
    pub attempt_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub epoch: u64,
    pub issuer_key_version: u32,
    pub fencing_token_digest: String,
    pub state: ExecutionLeaseState,
    pub checkpoint_ref: Option<Value>,
    pub acquired_at: String,
    pub renew_by: String,
    pub expires_at: String,
    pub version: u64,
}

/// Event metadata from the trusted command boundary; no provider output/credential.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

#[derive(Clone, Debug)]
pub struct AttemptAdmissionEvents {
    pub attempt: ExecutionEventContext,
    pub lease: ExecutionEventContext,
    pub step: ExecutionEventContext,
    pub task_status: Option<ExecutionEventContext>,
    pub budget: Vec<ExecutionEventContext>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionBudgetScope {
    Task,
    Environment,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionBudgetReservationState {
    Reserved,
    Committed,
    Released,
    Expired,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptBudgetReservationRecord {
    pub reservation_id: String,
    pub workspace_id: String,
    pub budget_scope: ExecutionBudgetScope,
    pub task_id: Option<String>,
    pub environment_id: Option<String>,
    pub attempt_id: String,
    pub metric: String,
    pub quantity: String,
    pub unit: String,
    pub currency: Option<String>,
    pub state: ExecutionBudgetReservationState,
    pub created_at: String,
    pub expires_at: Option<String>,
}

/// BudgetService's authoritative decision, never an owner/agent JSON flag. Each
/// reservation has exactly one budget owner. Empty Required is not admissible.
#[derive(Clone, Debug)]
pub enum AttemptBudgetAdmission {
    NotRequired {
        decision_ref: String,
    },
    Required {
        decision_ref: String,
        reservations: Vec<AttemptBudgetReservationRecord>,
    },
    Unsupported,
}

/// Internal service observations. Never deserialize these from an owner/agent
/// request. A supported store recomputes each check at the commit authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionCheck {
    Confirmed,
    Denied,
    Unknown,
    Unsupported,
}

#[derive(Clone, Debug)]
pub struct AttemptAdmissionChecks {
    pub owner_authorized: ExecutionCheck,
    pub workspace_active: ExecutionCheck,
    pub runtime_ready: ExecutionCheck,
    pub runtime_executor_binding: ExecutionCheck,
    pub agent_compatible: ExecutionCheck,
    pub environment_available: ExecutionCheck,
    pub inputs_available: ExecutionCheck,
    pub policy_grants_budget: ExecutionCheck,
    pub dependency_plan_current: ExecutionCheck,
    pub effects_reconciled: ExecutionCheck,
    pub mutation_fencing: ExecutionCheck,
    pub process_containment: ExecutionCheck,
    pub private_credential_delivery: ExecutionCheck,
    pub prior_attempt_settled: ExecutionCheck,
    pub explicit_recovery_authorized: ExecutionCheck,
}

#[derive(Clone, Debug)]
pub struct StepAttemptAdmissionSnapshot {
    pub task: TaskRecord,
    pub plan: PlanRevisionRecord,
    pub step: StepRecord,
    pub dependency_steps: Vec<StepRecord>,
    pub latest_step_epoch: u64,
    /// ACTIVE or RELEASING leases must be settled before another acquisition.
    pub conflicting_lease: Option<ExecutionLeaseRecord>,
    pub checks: AttemptAdmissionChecks,
    pub environment_runtime_id: String,
    pub environment_runtime_incarnation_id: String,
    pub environment_id: String,
    pub selected_agent_binding_id: String,
    pub dependency_plan_digest: String,
    pub budget_admission: AttemptBudgetAdmission,
}

/// Trusted command context is separate from agent/provider output. Only the digest
/// crosses storage; the raw fence is issued over private authenticated control.
#[derive(Clone, Debug)]
pub struct AdmitStepAttempt {
    pub workspace_id: String,
    pub owner_principal_id: String,
    pub request_id: String,
    pub task_id: String,
    pub step_id: String,
    pub expected_task_version: u64,
    pub expected_step_version: u64,
    pub expected_plan_revision: u64,
    pub expected_task_spec_revision: u64,
    pub expected_previous_epoch: u64,
    pub attempt_id: String,
    pub lease_id: String,
    /// All delegation identity fields are present together for a host-delegated
    /// child. A lead Attempt has none of them.
    pub parent_attempt_id: Option<String>,
    pub delegation_profile_id: Option<String>,
    pub delegation_profile_revision: Option<u64>,
    /// Exact pre-existing grants, scoped by the admission transaction to this
    /// Attempt. An empty list is the only currently supported shape.
    pub capability_grant_ids: Vec<String>,
    pub failover_class: String,
    pub agent_binding_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub environment_id: String,
    pub dependency_plan_digest: String,
    pub fencing_token_digest: String,
    pub issuer_key_version: u32,
    pub lease_duration_ms: u64,
    pub budget_admission_decision_ref: String,
    pub expected_budget_reservation_ids: Vec<String>,
    pub events: AttemptAdmissionEvents,
}

#[derive(Clone, Debug)]
pub struct CommittedStepAttempt {
    pub task: TaskRecord,
    pub step: StepRecord,
    pub attempt: AttemptRecord,
    pub lease: ExecutionLeaseRecord,
    pub budget_reservations: Vec<AttemptBudgetReservationRecord>,
    pub events: Vec<DomainEvent>,
    pub replayed: bool,
}

/// LeaseCoordinator authenticates this context on private Runtime control before
/// invoking storage. The digest is metadata and must never substitute for proof.
#[derive(Clone, Debug)]
pub struct ExecutionLeaseCommand {
    pub workspace_id: String,
    pub request_id: String,
    pub task_id: String,
    pub step_id: String,
    pub attempt_id: String,
    pub lease_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub expected_task_version: u64,
    pub expected_attempt_version: u64,
    pub expected_lease_version: u64,
    pub expected_epoch: u64,
    /// Non-secret private-control receipt identity, resolved and revalidated by
    /// authority against authenticated Runtime, exact lease/epoch and credential.
    /// Naming a receipt is never authentication by itself.
    pub private_control_authorization_ref: String,
}

#[derive(Clone, Debug)]
pub struct ExecutionLeaseMutationSnapshot {
    pub task: TaskRecord,
    pub step: StepRecord,
    pub attempt: AttemptRecord,
    pub lease: ExecutionLeaseRecord,
    pub latest_step_epoch: u64,
    /// Parsed from persisted expiry by the authority; caller clocks never count.
    pub now_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub renew_by_unix_ms: u64,
    pub authenticated_runtime: ExecutionCheck,
    pub credential_verified: ExecutionCheck,
    pub runtime_current: ExecutionCheck,
    pub mutation_fencing: ExecutionCheck,
    pub writer_quiescence: ExecutionCheck,
    pub invocations_settled: ExecutionCheck,
    pub effects_reconciled: ExecutionCheck,
}

#[derive(Clone, Debug)]
pub struct RenewExecutionLease {
    pub context: ExecutionLeaseCommand,
    pub lease_duration_ms: u64,
    pub event: ExecutionEventContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeaseReleasePhase {
    Begin,
    Complete,
}

#[derive(Clone, Debug)]
pub struct ReleaseExecutionLease {
    pub context: ExecutionLeaseCommand,
    pub phase: LeaseReleasePhase,
    pub event: ExecutionEventContext,
}

/// Recovery runs from a currently trusted Runtime after the previous Runtime
/// incarnation has disappeared. It never requires credentials from that dead
/// process. The SQLite authority independently proves expiry or incarnation fencing.
#[derive(Clone, Debug)]
pub struct ExpireExecutionLease {
    pub workspace_id: String,
    /// The Runtime's locally authenticated owner. Storage rechecks this against the
    /// Workspace row during the expiry transaction; a preflight lookup is not proof.
    pub owner_principal_id: String,
    pub request_id: String,
    pub task_id: String,
    pub step_id: String,
    pub attempt_id: String,
    pub lease_id: String,
    pub expected_task_version: u64,
    pub expected_step_version: u64,
    pub expected_attempt_version: u64,
    pub expected_lease_version: u64,
    pub recovery_runtime_id: String,
    pub recovery_runtime_incarnation_id: String,
    pub events: ExpiredAttemptEvents,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpiredAttemptEvents {
    pub lease: ExecutionEventContext,
    pub attempt: ExecutionEventContext,
    pub step: ExecutionEventContext,
    pub task: ExecutionEventContext,
}

/// Read-only recovery candidate discovered by storage using its authoritative clock.
/// This is not itself permission to mutate state; expiry rechecks every identity and
/// version in its immediate transaction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpiredExecutionLeaseCandidate {
    pub workspace_id: String,
    pub task_id: String,
    pub step_id: String,
    pub attempt_id: String,
    pub lease_id: String,
    pub task_version: u64,
    pub step_version: u64,
    pub attempt_version: u64,
    pub lease_version: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedExecutionLeaseMutation {
    pub lease: ExecutionLeaseRecord,
    pub events: Vec<DomainEvent>,
    pub replayed: bool,
}

fn unsupported() -> StoreError {
    StoreError::Invalid(
        "atomic Attempt and ExecutionLease storage guarantees are unsupported".to_owned(),
    )
}

/// LeaseCoordinator / TaskService / AttemptRunner share this atomic authority.
/// Implementations MUST revalidate all snapshot checks and CAS versions inside
/// each transaction, validate the canonical event envelope/timestamps, use the
/// authoritative clock, and authenticate the Runtime
/// and raw credential on private control before renewal/release. Idempotency
/// receipts bind authenticated caller, Workspace, operation and normalized input;
/// replay returns the original result without allocating another epoch/events.
///
/// Admission atomically appends CREATED Attempt + ACTIVE lease with next Step
/// epoch, binds both directions, updates Step.current_attempt_id and RUNNING
/// status, moves a READY Task to RUNNING, and commits the BudgetService decision
/// and every required reservation plus snapshots/events/receipt. The transaction
/// rechecks/consumes current budget capacity; merely reading a quote is insufficient.
/// Admission uses attempt.created.v1, lease.acquired.v1, step.status.changed.v1,
/// optional task.status.changed.v1 and budget.reservation.changed.v1 envelopes.
/// Supplied event IDs must be distinct and share trusted origin/correlation/time.
/// Budget reservation IDs/decision and all event metadata bind the idempotency digest;
/// a changed retry conflicts rather than creating another reservation or event.
/// Failed commits write none of these. No native adapter starts before commit.
/// Renewal preserves immutable lease identity, epoch, credential digest and
/// acquisition timestamp. Release is ACTIVE -> RELEASING -> RELEASED; completion
/// requires authoritative quiescence, Invocation settlement and Effect reconciliation.
/// All mediated mutations recheck the current fence at their own commit authority.
/// Defaults fail closed: existing stores cannot imply these guarantees by omission.
pub trait StepAttemptStore: Send + Sync {
    fn step_attempt_admission_snapshot(
        &self,
        _command: &AdmitStepAttempt,
    ) -> Result<StepAttemptAdmissionSnapshot, StoreError> {
        Err(unsupported())
    }
    fn admit_step_attempt(
        &self,
        _command: AdmitStepAttempt,
    ) -> Result<CommittedStepAttempt, StoreError> {
        Err(unsupported())
    }
    fn execution_lease_mutation_snapshot(
        &self,
        _command: &ExecutionLeaseCommand,
    ) -> Result<ExecutionLeaseMutationSnapshot, StoreError> {
        Err(unsupported())
    }
    fn renew_execution_lease(
        &self,
        _command: RenewExecutionLease,
    ) -> Result<CommittedExecutionLeaseMutation, StoreError> {
        Err(unsupported())
    }
    fn release_execution_lease(
        &self,
        _command: ReleaseExecutionLease,
    ) -> Result<CommittedExecutionLeaseMutation, StoreError> {
        Err(unsupported())
    }
    fn list_expired_execution_leases(
        &self,
        _owner_principal_id: &str,
        _workspace_id: &str,
        _recovery_runtime_id: &str,
        _recovery_runtime_incarnation_id: &str,
        _limit: usize,
    ) -> Result<Vec<ExpiredExecutionLeaseCandidate>, StoreError> {
        Err(unsupported())
    }
    fn expire_execution_lease(
        &self,
        _command: ExpireExecutionLease,
    ) -> Result<CommittedExecutionLeaseMutation, StoreError> {
        Err(unsupported())
    }
}
