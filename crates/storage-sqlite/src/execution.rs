//! SQLite execution authority. Caller snapshots are advisory only; each operation
//! reloads durable Task/Plan/Step/Runtime/Environment rows on the single writer.
//! Admission and live lease mutation stay closed until the Runtime/Trust proof sources
//! below are represented durably.

use super::*;
use storage_core::{
    AdmitStepAttempt, AttemptAdmissionChecks, AttemptBudgetAdmission, ChildAttemptAdmissionProofs,
    ChildAttemptAdmissionSnapshot, DelegationProfileAdmissionProjection, ExecutionCheck,
    ExecutionLeaseCommand, ExecutionLeaseMutationSnapshot, ExpireExecutionLease,
    StepAttemptAdmissionSnapshot, StepAttemptStore,
};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

struct ExpiryStateRefs {
    task: AggregateStateRef,
    step: AggregateStateRef,
    attempt: AggregateStateRef,
    lease: AggregateStateRef,
}

#[derive(Clone)]
pub struct SqliteStepAttemptStore {
    store: SqliteWorkspaceStore,
}

impl SqliteStepAttemptStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store }
    }

    fn run<T, F>(&self, operation: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::ExecutionOperation {
                operation: Box::new(move |db| {
                    let _ = reply.send(operation(db));
                }),
            },
            receive,
        )
    }
}

impl StepAttemptStore for SqliteStepAttemptStore {
    fn child_attempt_admission_snapshot(
        &self,
        command: &AdmitStepAttempt,
    ) -> Result<ChildAttemptAdmissionSnapshot, StoreError> {
        let command = command.clone();
        self.run(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            let snapshot = child_attempt_admission_snapshot(&tx, &command)?;
            tx.commit().map_err(map_database_error)?;
            Ok(snapshot)
        })
    }

    fn step_attempt_admission_snapshot(
        &self,
        command: &AdmitStepAttempt,
    ) -> Result<StepAttemptAdmissionSnapshot, StoreError> {
        let command = command.clone();
        self.run(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            let result = admission_snapshot(&tx, &command)?;
            tx.commit().map_err(map_database_error)?;
            Ok(result)
        })
    }

    fn admit_step_attempt(
        &self,
        command: AdmitStepAttempt,
    ) -> Result<storage_core::CommittedStepAttempt, StoreError> {
        self.run(move |connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_database_error)?;
            let snapshot = admission_snapshot(&tx, &command)?;
            let child_snapshot = command.parent_attempt_id.as_ref()
                .map(|_| child_attempt_admission_snapshot(&tx, &command))
                .transpose()?;
            validate_admission_rows(&tx, &command, &snapshot)?;
            if let Some(child) = child_snapshot {
                let missing = storage_core::unproven_child_admission_gates(&child.proofs);
                if !missing.is_empty() {
                    return Err(StoreError::Invalid(format!(
                        "child Attempt admission denied; required durable proofs are missing or unconfirmed: {missing:?}"
                    )));
                }
                return Err(StoreError::Invalid(
                    "child Attempt admission denied; atomic grant, budget reservation, Attempt, lease, Step, event, and receipt writer is not implemented".to_owned(),
                ));
            }
            Err(StoreError::Invalid("Attempt admission unavailable: durable Environment/process containment, Trust grant, budget reservation, and Effect-reconciliation proofs have no integrated producers".to_owned()))
        })
    }

    fn execution_lease_mutation_snapshot(
        &self,
        command: &ExecutionLeaseCommand,
    ) -> Result<ExecutionLeaseMutationSnapshot, StoreError> {
        let command = command.clone();
        self.run(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            let result = lease_snapshot(&tx, &command)?;
            tx.commit().map_err(map_database_error)?;
            Ok(result)
        })
    }

    fn renew_execution_lease(
        &self,
        command: storage_core::RenewExecutionLease,
    ) -> Result<storage_core::CommittedExecutionLeaseMutation, StoreError> {
        self.run(move |connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_database_error)?;
            let snapshot = lease_snapshot(&tx, &command.context)?;
            validate_lease_identity(&command.context, &snapshot)?;
            Err(StoreError::Invalid("lease renewal unavailable: authenticated private Runtime control receipt and lease aggregate-state/event commit are not integrated".to_owned()))
        })
    }

    fn release_execution_lease(
        &self,
        command: storage_core::ReleaseExecutionLease,
    ) -> Result<storage_core::CommittedExecutionLeaseMutation, StoreError> {
        self.run(move |connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_database_error)?;
            let snapshot = lease_snapshot(&tx, &command.context)?;
            validate_lease_identity(&command.context, &snapshot)?;
            Err(StoreError::Invalid("lease release unavailable: process quiescence, Invocation settlement, Effect reconciliation and private Runtime control proofs are not integrated".to_owned()))
        })
    }

    fn list_expired_execution_leases(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        recovery_runtime_id: &str,
        recovery_runtime_incarnation_id: &str,
        limit: usize,
    ) -> Result<Vec<storage_core::ExpiredExecutionLeaseCandidate>, StoreError> {
        let owner_principal_id = owner_principal_id.to_owned();
        let workspace_id = workspace_id.to_owned();
        let recovery_runtime_id = recovery_runtime_id.to_owned();
        let recovery_runtime_incarnation_id = recovery_runtime_incarnation_id.to_owned();
        self.run(move |connection| {
            if limit == 0 || limit > 256 {
                return Err(StoreError::Invalid("execution recovery page size is invalid".to_owned()));
            }
            let tx = connection.transaction().map_err(map_database_error)?;
            let now = sqlite_now(&tx)?;
            let authorized: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces w JOIN runtime_workspace_bindings b ON b.workspace_id = w.workspace_id AND b.runtime_id = ?3 AND b.status = 'ACTIVE' AND b.revoked_at IS NULL JOIN runtimes r ON r.runtime_id = b.runtime_id AND r.current_incarnation_id = ?4 JOIN runtime_incarnations i ON i.runtime_id = r.runtime_id AND i.runtime_incarnation_id = r.current_incarnation_id WHERE w.workspace_id = ?1 AND w.owner_principal_id = ?2 AND w.status = 'ACTIVE' AND r.trust_zone = 'PERSONAL_DEVICE' AND ((r.availability = 'ONLINE' AND i.recovery_state = 'READY') OR (r.availability = 'DEGRADED' AND i.recovery_state = 'DEGRADED')) AND EXISTS(SELECT 1 FROM json_each(r.roles_json) rr WHERE rr.value = 'OPERATOR_ENDPOINT') AND EXISTS(SELECT 1 FROM json_each(b.roles_json) br WHERE br.value = 'EXECUTOR'))",
                params![workspace_id, owner_principal_id, recovery_runtime_id, recovery_runtime_incarnation_id],
                |row| row.get(0),
            ).map_err(map_database_error)?;
            if !authorized { return Err(StoreError::Invalid("current Runtime is not authorized to recover this Workspace".to_owned())); }
            let mut statement = tx.prepare(
                "SELECT t.workspace_id, t.task_id, s.step_id, a.attempt_id, l.lease_id,
                        t.version, s.version, a.version, l.version
                 FROM execution_leases l
                 JOIN tasks t ON t.task_id = l.task_id AND t.workspace_id = ?1 AND t.status = 'RUNNING'
                 JOIN steps s ON s.task_id = t.task_id AND s.step_id = l.step_id
                 JOIN attempts a ON a.task_id = t.task_id AND a.attempt_id = l.attempt_id
                 JOIN workspaces w ON w.workspace_id = t.workspace_id AND w.owner_principal_id = ?2 AND w.status = 'ACTIVE'
                 WHERE l.state IN ('ACTIVE', 'RELEASING') AND julianday(l.expires_at) <= julianday(?3)
                   AND s.current_attempt_id = a.attempt_id AND s.status IN ('RUNNING', 'WAITING_USER', 'VERIFYING')
                   AND a.status IN ('CREATED', 'PREPARING', 'RUNNING', 'WAITING_APPROVAL', 'WAITING_RESOURCE', 'CHECKPOINTING')
                 ORDER BY l.expires_at ASC, l.lease_id ASC LIMIT ?4",
            ).map_err(map_database_error)?;
            let rows = statement.query_map(
                params![workspace_id, owner_principal_id, now, to_sql_i64(limit as u64, "execution recovery page size")?],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, i64>(5)?, row.get::<_, i64>(6)?, row.get::<_, i64>(7)?, row.get::<_, i64>(8)?)),
            ).map_err(map_database_error)?;
            let candidates = rows.map(|row| {
                let (workspace_id, task_id, step_id, attempt_id, lease_id, task_version, step_version, attempt_version, lease_version) = row.map_err(map_database_error)?;
                Ok(storage_core::ExpiredExecutionLeaseCandidate {
                    workspace_id, task_id, step_id, attempt_id, lease_id,
                    task_version: from_sql_i64(task_version, "Task version")?,
                    step_version: from_sql_i64(step_version, "Step version")?,
                    attempt_version: from_sql_i64(attempt_version, "Attempt version")?,
                    lease_version: from_sql_i64(lease_version, "ExecutionLease version")?,
                })
            }).collect::<Result<Vec<_>, StoreError>>()?;
            drop(statement);
            tx.commit().map_err(map_database_error)?;
            Ok(candidates)
        })
    }

    fn expire_execution_lease(
        &self,
        mut command: ExpireExecutionLease,
    ) -> Result<storage_core::CommittedExecutionLeaseMutation, StoreError> {
        canonicalize_recovery_events(&mut command)?;
        let command_for_read = command.clone();
        let current =
            self.run(move |connection| load_expiry_material(connection, &command_for_read))?;
        // Authoritative content blobs are written and verified before opening the
        // SQLite transaction. A losing CAS leaves only unreferenced GC candidates.
        let updated = current.updated(command.events.lease.recorded_at.clone())?;
        let task_state = put_state(
            &self.store,
            &command.workspace_id,
            &updated.task_snapshot,
            "application/vnd.litecowork.task+json",
            updated.task.version,
        )?;
        let step_state = put_state(
            &self.store,
            &command.workspace_id,
            &updated.step,
            "application/vnd.litecowork.step+json",
            updated.step.version,
        )?;
        let attempt_state = put_state(
            &self.store,
            &command.workspace_id,
            &updated.attempt,
            "application/vnd.litecowork.attempt+json",
            updated.attempt.version,
        )?;
        let lease_state = put_state(
            &self.store,
            &command.workspace_id,
            &updated.lease,
            "application/vnd.litecowork.execution-lease+json",
            updated.lease.version,
        )?;
        self.run(move |connection| {
            expire_transaction(
                connection,
                command,
                current,
                updated,
                ExpiryStateRefs {
                    task: task_state,
                    step: step_state,
                    attempt: attempt_state,
                    lease: lease_state,
                },
            )
        })
    }
}

#[derive(Clone)]
struct ExpiryMaterial {
    task: storage_core::TaskRecord,
    task_spec: storage_core::TaskSpecRevisionRecord,
    plan: storage_core::PlanRevisionRecord,
    steps: Vec<storage_core::StepRecord>,
    step: storage_core::StepRecord,
    attempt: storage_core::AttemptRecord,
    lease: storage_core::ExecutionLeaseRecord,
}

#[derive(Clone)]
struct ExpiryCommitMaterial {
    task_snapshot: storage_core::TaskAggregateSnapshot,
    task: storage_core::TaskRecord,
    step: storage_core::StepRecord,
    attempt: storage_core::AttemptRecord,
    lease: storage_core::ExecutionLeaseRecord,
}

impl ExpiryMaterial {
    fn updated(&self, at: String) -> Result<ExpiryCommitMaterial, StoreError> {
        let mut task = self.task.clone();
        let mut step = self.step.clone();
        let mut attempt = self.attempt.clone();
        let mut lease = self.lease.clone();
        task.status = "BLOCKED".to_owned();
        task.updated_at = at.clone();
        task.version = task
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("Task version exhausted".to_owned()))?;
        let blocker_id = format!("lease-expired:{}", lease.lease_id);
        let blocker = json!({
            "blocker_id": blocker_id,
            "code": "STALE_FENCE",
            "safe_message": "A worker lease expired. Its external effects must be reconciled before this Step can continue.",
            "resolution_hint": "Review the task's unresolved effects and resume after their state is confirmed.",
            "created_at": at,
        });
        if !task
            .blocking_conditions
            .iter()
            .any(|existing| existing.get("blocker_id") == blocker.get("blocker_id"))
        {
            task.blocking_conditions.push(blocker);
        }
        step.status = "BLOCKED".to_owned();
        step.updated_at = at.clone();
        step.version = step
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("Step version exhausted".to_owned()))?;
        attempt.status = storage_core::AttemptState::Abandoned;
        attempt.settled_at = Some(at.clone());
        attempt.failure = Some(json!({"reason_code":"LEASE_EXPIRED","recovery_required":true}));
        attempt.version = attempt
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("Attempt version exhausted".to_owned()))?;
        lease.state = storage_core::ExecutionLeaseState::Expired;
        lease.version = lease
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("ExecutionLease version exhausted".to_owned()))?;
        let mut steps = self.steps.clone();
        let Some(slot) = steps
            .iter_mut()
            .find(|candidate| candidate.step_id == step.step_id)
        else {
            return Err(StoreError::Integrity(
                "expired Step is absent from its current PlanRevision".to_owned(),
            ));
        };
        *slot = step.clone();
        let task_snapshot = storage_core::TaskAggregateSnapshot {
            task: task.clone(),
            current_spec_revision: self.task_spec.clone(),
            current_plan_revision: Some(self.plan.clone()),
            current_steps: steps,
        };
        Ok(ExpiryCommitMaterial {
            task_snapshot,
            task,
            step,
            attempt,
            lease,
        })
    }
}

fn load_expiry_material(
    connection: &Connection,
    command: &ExpireExecutionLease,
) -> Result<ExpiryMaterial, StoreError> {
    let task_view = load_task_view(connection, &command.workspace_id, &command.task_id)?
        .ok_or(StoreError::NotFound)?;
    let plan_revision = task_view.task.current_plan_revision.ok_or_else(|| {
        StoreError::Invalid("expired Attempt Task has no current PlanRevision".to_owned())
    })?;
    let plan = list_plan_revisions(connection, &command.workspace_id, &command.task_id)?
        .into_iter()
        .find(|candidate| candidate.revision == plan_revision)
        .ok_or_else(|| StoreError::Integrity("current PlanRevision is missing".to_owned()))?;
    let steps = list_steps(
        connection,
        &command.workspace_id,
        &command.task_id,
        Some(plan_revision),
    )?;
    let step = steps
        .iter()
        .find(|candidate| candidate.step_id == command.step_id)
        .cloned()
        .ok_or(StoreError::NotFound)?;
    let attempt = load_attempt(connection, &command.task_id, &command.attempt_id)?
        .ok_or(StoreError::NotFound)?;
    let lease = load_lease(connection, &command.lease_id)?.ok_or(StoreError::NotFound)?;
    Ok(ExpiryMaterial {
        task: task_view.task,
        task_spec: task_view.current_spec_revision,
        plan,
        steps,
        step,
        attempt,
        lease,
    })
}

fn put_state<T: serde::Serialize>(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    value: &T,
    media_type: &str,
    revision: u64,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(value)?;
    let blob = store.inner.blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &bytes,
        media_type,
    )?;
    if blob.size_bytes != bytes.len() as u64
        || store
            .inner
            .blobs
            .get(workspace_id, BlobPurpose::AggregateState, &blob)?
            != bytes
    {
        return Err(StoreError::Integrity(
            "execution aggregate-state blob failed verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: revision,
        record_schema_version: 1,
    })
}

fn canonicalize_recovery_events(command: &mut ExpireExecutionLease) -> Result<(), StoreError> {
    let events = [
        &mut command.events.lease,
        &mut command.events.attempt,
        &mut command.events.step,
        &mut command.events.task,
    ];
    let mut ids = std::collections::HashSet::new();
    for event in events {
        if event.origin_runtime_id != command.recovery_runtime_id
            || event.event_id.trim().is_empty()
            || event.event_id.len() > 256
            || event.event_id.chars().any(char::is_control)
            || event.correlation_id.trim().is_empty()
            || event.correlation_id.len() > 256
            || event.correlation_id.chars().any(char::is_control)
            || !ids.insert(event.event_id.clone())
        {
            return Err(StoreError::Invalid(
                "lease expiry event context is invalid".to_owned(),
            ));
        }
        validate_timestamp(&event.hlc_timestamp)?;
        if let Some(causation_id) = &event.causation_id
            && (causation_id.trim().is_empty()
                || causation_id.len() > 256
                || causation_id.chars().any(char::is_control))
        {
            return Err(StoreError::Invalid(
                "lease expiry event causation ID is invalid".to_owned(),
            ));
        }
        event.recorded_at = canonicalize_utc_timestamp(&event.recorded_at)?;
    }
    let base = &command.events.lease;
    if [
        &command.events.attempt,
        &command.events.step,
        &command.events.task,
    ]
    .iter()
    .any(|event| {
        event.correlation_id != base.correlation_id
            || event.hlc_timestamp != base.hlc_timestamp
            || event.recorded_at != base.recorded_at
    }) {
        return Err(StoreError::Invalid(
            "lease expiry events must share one canonical transaction time and correlation"
                .to_owned(),
        ));
    }
    Ok(())
}

fn expire_transaction(
    connection: &mut Connection,
    command: ExpireExecutionLease,
    expected: ExpiryMaterial,
    updated: ExpiryCommitMaterial,
    state_refs: ExpiryStateRefs,
) -> Result<storage_core::CommittedExecutionLeaseMutation, StoreError> {
    let ExpiryStateRefs {
        task: task_state,
        step: step_state,
        attempt: attempt_state,
        lease: lease_state,
    } = state_refs;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let runtime_authorized: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces w JOIN runtime_workspace_bindings b ON b.workspace_id = w.workspace_id AND b.runtime_id = ?3 AND b.status = 'ACTIVE' AND b.revoked_at IS NULL JOIN runtimes r ON r.runtime_id = b.runtime_id AND r.current_incarnation_id = ?4 AND r.trust_zone = 'PERSONAL_DEVICE' JOIN runtime_incarnations i ON i.runtime_id = r.runtime_id AND i.runtime_incarnation_id = r.current_incarnation_id WHERE w.workspace_id = ?1 AND w.owner_principal_id = ?2 AND w.status = 'ACTIVE' AND r.runtime_id = ?3 AND ((r.availability = 'ONLINE' AND i.recovery_state = 'READY') OR (r.availability = 'DEGRADED' AND i.recovery_state = 'DEGRADED')) AND EXISTS(SELECT 1 FROM json_each(r.roles_json) rr WHERE rr.value = 'OPERATOR_ENDPOINT') AND EXISTS(SELECT 1 FROM json_each(b.roles_json) br WHERE br.value = 'EXECUTOR'))",
        params![command.workspace_id, command.owner_principal_id, command.recovery_runtime_id, command.recovery_runtime_incarnation_id], |row| row.get(0),
    ).map_err(map_database_error)?;
    if !runtime_authorized {
        return Err(StoreError::Invalid(
            "recovery Runtime is not currently authorized for this Workspace".to_owned(),
        ));
    }
    let request_digest = digest(&canonical_json(&json!({
        "operation":"expire_execution_lease", "workspace_id":command.workspace_id, "owner_principal_id":command.owner_principal_id, "task_id":command.task_id,
        "step_id":command.step_id, "attempt_id":command.attempt_id, "lease_id":command.lease_id,
        "recovery_runtime_id":command.recovery_runtime_id,
        "recovery_runtime_incarnation_id":command.recovery_runtime_incarnation_id,
        "expected_task_version":command.expected_task_version, "expected_step_version":command.expected_step_version,
        "expected_attempt_version":command.expected_attempt_version, "expected_lease_version":command.expected_lease_version,
        "events":command.events,
    }))?);
    let receipt: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![command.recovery_runtime_id, command.request_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((stored_digest, response, response_digest)) = receipt {
        if stored_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response = response.ok_or_else(|| {
            StoreError::Integrity("execution recovery receipt has no response".to_owned())
        })?;
        if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "execution recovery receipt digest does not match its response".to_owned(),
            ));
        }
        let mut committed: storage_core::CommittedExecutionLeaseMutation =
            serde_json::from_str(&response)
                .map_err(|error| StoreError::Integrity(error.to_string()))?;
        committed.replayed = true;
        tx.commit().map_err(map_database_error)?;
        return Ok(committed);
    }
    let current = load_expiry_material(&tx, &command)?;
    if current.task.version != command.expected_task_version
        || current.step.version != command.expected_step_version
        || current.attempt.version != command.expected_attempt_version
        || current.lease.version != command.expected_lease_version
        || current.task.version != expected.task.version
        || current.step.version != expected.step.version
        || current.attempt.version != expected.attempt.version
        || current.lease.version != expected.lease.version
        || current.lease.lease_id != command.lease_id
        || current.lease.task_id != command.task_id
        || current.lease.step_id != command.step_id
        || current.lease.attempt_id != command.attempt_id
        || current.step.current_attempt_id.as_deref() != Some(command.attempt_id.as_str())
    {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_lease_version),
            actual: Some(current.lease.version),
        });
    }
    if !matches!(
        current.lease.state,
        storage_core::ExecutionLeaseState::Active | storage_core::ExecutionLeaseState::Releasing
    ) || !matches!(
        current.attempt.status,
        storage_core::AttemptState::Created
            | storage_core::AttemptState::Preparing
            | storage_core::AttemptState::Running
            | storage_core::AttemptState::WaitingApproval
            | storage_core::AttemptState::WaitingResource
            | storage_core::AttemptState::Checkpointing
    ) || current.task.status != "RUNNING"
        || !matches!(
            current.step.status.as_str(),
            "RUNNING" | "WAITING_USER" | "VERIFYING"
        )
        || current.task.current_plan_revision != Some(current.plan.revision)
        || !accepted_plan_matches_steps(&current.plan, &current.steps)
    {
        return Err(StoreError::Invalid(
            "lease or Task is no longer recoverable".to_owned(),
        ));
    }
    let now = sqlite_now(&tx)?;
    if timestamp_unix_ms(&now)? < timestamp_unix_ms(&current.lease.expires_at)? {
        return Err(StoreError::Invalid(
            "lease has not expired according to the SQLite authority clock".to_owned(),
        ));
    }
    let at = command.events.lease.recorded_at.as_str();
    if updated.task.status != "BLOCKED"
        || updated.step.status != "BLOCKED"
        || updated.lease.state != storage_core::ExecutionLeaseState::Expired
        || task_state.entity_revision != updated.task.version
        || step_state.entity_revision != updated.step.version
        || attempt_state.entity_revision != updated.attempt.version
        || lease_state.entity_revision != updated.lease.version
        || [
            &command.events.lease,
            &command.events.attempt,
            &command.events.step,
            &command.events.task,
        ]
        .iter()
        .any(|e| e.recorded_at != at)
    {
        return Err(StoreError::Invalid(
            "lease expiry snapshots/events are inconsistent".to_owned(),
        ));
    }
    let next_task = to_sql_i64(updated.task.version, "Task version")?;
    let next_step = to_sql_i64(updated.step.version, "Step version")?;
    let next_attempt = to_sql_i64(updated.attempt.version, "Attempt version")?;
    let next_lease = to_sql_i64(updated.lease.version, "lease version")?;
    if tx.execute("UPDATE execution_leases SET state='EXPIRED', version=?1 WHERE lease_id=?2 AND version=?3 AND state IN ('ACTIVE','RELEASING')",
        params![next_lease, command.lease_id, to_sql_i64(command.expected_lease_version, "lease version")?]).map_err(map_database_error)? != 1
        || tx.execute("UPDATE attempts SET status='ABANDONED', failure_json=?1, settled_at=?2, version=?3 WHERE task_id=?4 AND attempt_id=?5 AND version=?6 AND status IN ('CREATED','PREPARING','RUNNING','WAITING_APPROVAL','WAITING_RESOURCE','CHECKPOINTING')",
            params![String::from_utf8(canonical_json(&updated.attempt.failure)?).map_err(|e|StoreError::Invalid(e.to_string()))?, at, next_attempt, command.task_id, command.attempt_id, to_sql_i64(command.expected_attempt_version,"Attempt version")?]).map_err(map_database_error)? != 1
        || tx.execute("UPDATE steps SET status='BLOCKED', updated_at=?1, version=?2 WHERE task_id=?3 AND step_id=?4 AND version=?5 AND current_attempt_id=?6 AND status IN ('RUNNING','WAITING_USER','VERIFYING')",
            params![at, next_step, command.task_id, command.step_id, to_sql_i64(command.expected_step_version,"Step version")?, command.attempt_id]).map_err(map_database_error)? != 1
        || tx.execute("UPDATE tasks SET status='BLOCKED', blocking_conditions_json=?1, updated_at=?2, version=?3 WHERE workspace_id=?4 AND task_id=?5 AND version=?6 AND status='RUNNING'",
            params![String::from_utf8(canonical_json(&updated.task.blocking_conditions)?).map_err(|e|StoreError::Invalid(e.to_string()))?, at, next_task, command.workspace_id, command.task_id, to_sql_i64(command.expected_task_version,"Task version")?]).map_err(map_database_error)? != 1
    { return Err(StoreError::Conflict { expected: Some(command.expected_task_version), actual: None }); }

    let drafts = vec![
        event_draft(
            &command.events.lease,
            &command.workspace_id,
            "ExecutionLease",
            &command.lease_id,
            next_lease as u64,
            "lease.expired.v1",
            json!({"lease_id":command.lease_id,"task_id":command.task_id,"step_id":command.step_id,"attempt_id":command.attempt_id,"runtime_id":current.lease.runtime_id,"runtime_incarnation_id":current.lease.runtime_incarnation_id,"epoch":current.lease.epoch.to_string(),"expires_at":current.lease.expires_at}),
        ),
        event_draft(
            &command.events.attempt,
            &command.workspace_id,
            "Attempt",
            &command.attempt_id,
            next_attempt as u64,
            "attempt.status.changed.v1",
            json!({"attempt_id":command.attempt_id,"from":format!("{:?}",current.attempt.status).to_ascii_uppercase(),"to":"ABANDONED","reason_code":"LEASE_EXPIRED","aggregate_version":updated.attempt.version}),
        ),
        event_draft(
            &command.events.step,
            &command.workspace_id,
            "Step",
            &command.step_id,
            next_step as u64,
            "step.status.changed.v1",
            json!({"step_id":command.step_id,"task_id":command.task_id,"from":current.step.status,"to":"BLOCKED","reason_code":"LEASE_EXPIRED_EFFECT_RECONCILIATION_REQUIRED","aggregate_version":updated.step.version}),
        ),
        event_draft(
            &command.events.task,
            &command.workspace_id,
            "Task",
            &command.task_id,
            next_task as u64,
            "task.status.changed.v1",
            json!({"task_id":command.task_id,"from":current.task.status,"to":"BLOCKED","reason_code":"LEASE_EXPIRED_EFFECT_RECONCILIATION_REQUIRED","actor":{"service_id":"LeaseRecoveryService"},"aggregate_version":updated.task.version,"blocking_conditions":updated.task.blocking_conditions}),
        ),
    ];
    let refs = [&lease_state, &attempt_state, &step_state, &task_state];
    let events = drafts
        .into_iter()
        .zip(refs)
        .map(|(draft, state)| insert_domain_event(&tx, &draft, state))
        .collect::<Result<Vec<_>, _>>()?;
    let mut committed = storage_core::CommittedExecutionLeaseMutation {
        lease: updated.lease.clone(),
        events,
        replayed: false,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|e| StoreError::Invalid(e.to_string()))?;
    tx.execute("INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![command.recovery_runtime_id, command.request_id, request_digest, response_json, digest(response_json.as_bytes()), at]).map_err(map_database_error)?;
    let _ = now;
    tx.commit().map_err(map_database_error)?;
    committed.replayed = false;
    Ok(committed)
}

fn event_draft(
    context: &storage_core::ExecutionEventContext,
    workspace: &str,
    entity_type: &str,
    entity_id: &str,
    revision: u64,
    event_type: &str,
    payload: Value,
) -> EventDraft {
    EventDraft {
        event_id: context.event_id.clone(),
        workspace_id: workspace.to_owned(),
        entity_type: entity_type.to_owned(),
        entity_id: entity_id.to_owned(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        entity_revision: revision,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        schema_version: 1,
        event_type: event_type.to_owned(),
        payload,
        recorded_at: context.recorded_at.clone(),
    }
}

/// Project durable child-admission facts under one SQLite read transaction. This does
/// not authorize a child or establish the external proofs below; the final writer must
/// repeat these reads when it eventually commits an admitted Attempt.
fn child_attempt_admission_snapshot(
    connection: &Connection,
    command: &AdmitStepAttempt,
) -> Result<ChildAttemptAdmissionSnapshot, StoreError> {
    let (Some(parent_id), Some(profile_id), Some(profile_revision)) = (
        command.parent_attempt_id.as_deref(),
        command.delegation_profile_id.as_deref(),
        command.delegation_profile_revision,
    ) else {
        return Err(StoreError::Invalid(
            "child-admission projection requires a parent Attempt and pinned profile revision"
                .to_owned(),
        ));
    };
    let base = admission_snapshot(connection, command)?;
    let task_view = load_task_view(connection, &command.workspace_id, &command.task_id)?
        .ok_or(StoreError::NotFound)?;
    let parent_attempt =
        load_attempt(connection, &command.task_id, parent_id)?.ok_or(StoreError::NotFound)?;
    let current_plan_steps = list_steps(
        connection,
        &command.workspace_id,
        &command.task_id,
        task_view.task.current_plan_revision,
    )?;
    let parent_step = current_plan_steps
        .iter()
        .find(|step| step.step_id == parent_attempt.step_id)
        .cloned();
    let parent_session = parent_attempt
        .agent_session_id
        .as_deref()
        .map(|session_id| load_agent_session(connection, &command.workspace_id, session_id))
        .transpose()?
        .flatten();
    let parent_lease = parent_attempt
        .execution_lease_id
        .as_deref()
        .map(|lease_id| load_lease(connection, lease_id))
        .transpose()?
        .flatten();
    let sqlite_now = sqlite_now(connection)?;
    let parent_lease_current = parent_lease.as_ref().is_some_and(|lease| {
        lease.state == storage_core::ExecutionLeaseState::Active
            && timestamp_unix_ms(&lease.expires_at)
                .ok()
                .zip(timestamp_unix_ms(&sqlite_now).ok())
                .is_some_and(|(expires, now)| expires > now)
    });

    // The lineage is bounded by the platform's maximum depth. Reject cycles and
    // missing ancestors as integrity errors rather than returning a partial path.
    let mut parent_ancestry = Vec::new();
    let mut current = Some(parent_attempt.clone());
    let mut seen = std::collections::HashSet::new();
    while let Some(attempt) = current {
        if !seen.insert(attempt.attempt_id.clone()) || parent_ancestry.len() > 2 {
            return Err(StoreError::Integrity(
                "child Attempt ancestry is cyclic or exceeds the platform bound".to_owned(),
            ));
        }
        parent_ancestry.push(attempt.attempt_id.clone());
        current = match attempt.parent_attempt_id.as_deref() {
            Some(ancestor_id) => Some(
                load_attempt(connection, &command.task_id, ancestor_id)?.ok_or_else(|| {
                    StoreError::Integrity("parent Attempt ancestry has a missing row".to_owned())
                })?,
            ),
            None => None,
        };
    }
    parent_ancestry.reverse();
    if parent_ancestry.len() > 3 {
        return Err(StoreError::Invalid(
            "host delegation depth exceeds the platform maximum".to_owned(),
        ));
    }

    let profile_row: Option<(String, String, i64, String, i64, i64, bool)> = connection
        .query_row(
            "SELECT p.agent_binding_id, p.status, p.current_revision, r.workspace_id,
                    r.max_concurrency, r.max_host_delegation_depth, b.enabled
             FROM delegation_profiles p
             JOIN agent_bindings b ON b.workspace_id = p.workspace_id AND b.agent_binding_id = p.agent_binding_id
             JOIN delegation_profile_revisions r
               ON r.delegation_profile_id = p.delegation_profile_id
              AND r.revision = ?3 AND r.workspace_id = p.workspace_id
             WHERE p.workspace_id = ?1 AND p.delegation_profile_id = ?2",
            params![
                command.workspace_id,
                profile_id,
                to_sql_i64(profile_revision, "DelegationProfile revision")?
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get::<_, i64>(6)? == 1,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        profile_binding,
        profile_status,
        current_revision,
        profile_workspace,
        profile_concurrency,
        profile_depth,
        binding_enabled,
    )) = profile_row
    else {
        return Err(StoreError::NotFound);
    };

    let (coworker_enabled_profile_ids, coworker_policy_json): (Vec<String>, Option<String>) =
        if let (Some(coworker_id), Some(coworker_revision)) = (
            task_view.task.origin_coworker_id.as_deref(),
            task_view.task.origin_coworker_revision,
        ) {
            let row: Option<(String, Option<String>)> = connection
                .query_row(
                    "SELECT enabled_delegation_profile_ids_json, delegation_budget_policy_json
                     FROM coworker_revisions WHERE workspace_id = ?1 AND coworker_id = ?2 AND revision = ?3",
                    params![
                        command.workspace_id,
                        coworker_id,
                        to_sql_i64(coworker_revision, "Coworker revision")?
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(map_database_error)?;
            let (profiles_json, budget_policy) = row.ok_or_else(|| {
                StoreError::Integrity("Task's pinned Coworker revision is missing".to_owned())
            })?;
            (
                serde_json::from_str(&profiles_json)
                    .map_err(|error| StoreError::Integrity(error.to_string()))?,
                budget_policy,
            )
        } else {
            (Vec::new(), None)
        };
    let task_max_concurrency = policy_max_concurrency(
        task_view
            .current_spec_revision
            .delegation_budget_policy
            .as_ref(),
    )?;
    let coworker_max_concurrency = policy_json_max_concurrency(coworker_policy_json.as_deref())?;
    let active_task_children =
        count_active_children(connection, &command.workspace_id, &command.task_id, None)?;
    let active_profile_attempts = count_active_children(
        connection,
        &command.workspace_id,
        &command.task_id,
        Some(profile_id),
    )?;
    let parent_profile_max_delegation_depth = match (
        parent_attempt.delegation_profile_id.as_deref(),
        parent_attempt.delegation_profile_revision,
    ) {
        (Some(parent_profile_id), Some(parent_profile_revision)) => {
            let value: Option<i64> = connection
                .query_row(
                    "SELECT max_host_delegation_depth FROM delegation_profile_revisions
                     WHERE workspace_id = ?1 AND delegation_profile_id = ?2 AND revision = ?3",
                    params![
                        command.workspace_id,
                        parent_profile_id,
                        to_sql_i64(parent_profile_revision, "parent profile revision")?
                    ],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            Some(
                u8::try_from(value.ok_or_else(|| {
                    StoreError::Integrity("parent profile revision is missing".to_owned())
                })?)
                .map_err(|_| StoreError::Integrity("parent profile depth is invalid".to_owned()))?,
            )
        }
        (None, None) => None,
        _ => {
            return Err(StoreError::Integrity(
                "parent Attempt has a partial profile pin".to_owned(),
            ));
        }
    };

    Ok(ChildAttemptAdmissionSnapshot {
        task: task_view.task,
        task_spec: task_view.current_spec_revision,
        plan: base.plan,
        step: base.step,
        dependency_steps: base.dependency_steps,
        parent_attempt,
        parent_step,
        parent_session,
        parent_lease,
        parent_lease_current,
        parent_ancestry,
        parent_profile_max_delegation_depth,
        candidate_profile: DelegationProfileAdmissionProjection {
            profile_id: profile_id.to_owned(),
            workspace_id: profile_workspace,
            agent_binding_id: profile_binding,
            current_revision: from_sql_i64(current_revision, "current profile revision")?,
            requested_revision: profile_revision,
            status: profile_status,
            binding_enabled,
            max_concurrency: u8::try_from(profile_concurrency)
                .map_err(|_| StoreError::Integrity("profile concurrency is invalid".to_owned()))?,
            max_host_delegation_depth: u8::try_from(profile_depth)
                .map_err(|_| StoreError::Integrity("profile depth is invalid".to_owned()))?,
        },
        coworker_enabled_profile_ids,
        task_max_concurrency,
        coworker_max_concurrency,
        active_task_child_attempts: active_task_children,
        active_profile_attempts,
        // No durable producer currently proves any of these authority checks.
        proofs: ChildAttemptAdmissionProofs::default(),
    })
}

fn policy_max_concurrency(policy: Option<&Value>) -> Result<Option<u8>, StoreError> {
    let Some(policy) = policy else {
        return Ok(None);
    };
    parse_optional_concurrency(policy.get("max_concurrent_children"))
}

fn policy_json_max_concurrency(policy_json: Option<&str>) -> Result<Option<u8>, StoreError> {
    let Some(policy_json) = policy_json else {
        return Ok(None);
    };
    let policy: Value = serde_json::from_str(policy_json).map_err(|error| {
        StoreError::Integrity(format!("Coworker budget policy JSON is invalid: {error}"))
    })?;
    parse_optional_concurrency(policy.get("max_concurrent_children"))
}

fn parse_optional_concurrency(value: Option<&Value>) -> Result<Option<u8>, StoreError> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() {
        return Ok(None);
    }
    let Some(value) = value.as_u64() else {
        return Err(StoreError::Integrity(
            "max_concurrent_children is not an unsigned integer".to_owned(),
        ));
    };
    if !(1..=8).contains(&value) {
        return Err(StoreError::Integrity(
            "max_concurrent_children is outside the supported range".to_owned(),
        ));
    }
    Ok(Some(value as u8))
}

fn count_active_children(
    connection: &Connection,
    workspace_id: &str,
    task_id: &str,
    profile_id: Option<&str>,
) -> Result<u8, StoreError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM attempts a JOIN tasks t ON t.task_id = a.task_id
             WHERE t.workspace_id = ?1 AND a.task_id = ?2 AND a.parent_attempt_id IS NOT NULL
               AND a.status NOT IN ('COMPLETED','FAILED','ABANDONED','CANCELLED')
               AND (?3 IS NULL OR a.delegation_profile_id = ?3)",
            params![workspace_id, task_id, profile_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    u8::try_from(count).map_err(|_| {
        StoreError::Invalid(
            "active child Attempt count exceeds the bounded platform limit".to_owned(),
        )
    })
}

fn admission_snapshot(
    connection: &Connection,
    command: &AdmitStepAttempt,
) -> Result<StepAttemptAdmissionSnapshot, StoreError> {
    let tx = connection;
    let (owner, workspace_status): (String, String) = tx
        .query_row(
            "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
            [&command.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;
    if owner != command.owner_principal_id {
        return Err(StoreError::Invalid(
            "Workspace owner authorization failed".to_owned(),
        ));
    }
    let task = load_task_view(tx, &command.workspace_id, &command.task_id)?
        .ok_or(StoreError::NotFound)?
        .task;
    let plan_revision = task
        .current_plan_revision
        .ok_or_else(|| StoreError::Invalid("Task has no accepted PlanRevision".to_owned()))?;
    let plan = list_plan_revisions(tx, &command.workspace_id, &command.task_id)?
        .into_iter()
        .find(|plan| plan.revision == plan_revision)
        .ok_or_else(|| StoreError::Integrity("current PlanRevision is missing".to_owned()))?;
    let steps = list_steps(
        tx,
        &command.workspace_id,
        &command.task_id,
        Some(plan_revision),
    )?;
    if !accepted_plan_matches_steps(&plan, &steps) {
        return Err(StoreError::Integrity(
            "materialized Steps do not match the exact accepted PlanRevision".to_owned(),
        ));
    }
    let step = steps
        .iter()
        .find(|step| step.step_id == command.step_id)
        .cloned()
        .ok_or(StoreError::NotFound)?;
    let dependency_steps = step
        .dependencies
        .iter()
        .map(|dependency| {
            steps
                .iter()
                .find(|candidate| candidate.step_id == *dependency)
                .cloned()
                .ok_or_else(|| {
                    StoreError::Integrity(
                        "Step dependency is not in the accepted PlanRevision".to_owned(),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let current_epoch: u64 = tx
        .query_row(
            "SELECT COALESCE(MAX(epoch), 0) FROM execution_leases WHERE step_id = ?1",
            [&command.step_id],
            |row| from_row_u64(row, 0),
        )
        .map_err(map_database_error)?;
    let conflicting_lease = tx.query_row(
        "SELECT lease_id, task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id, epoch, issuer_key_version, fencing_token_digest, state, checkpoint_ref_json, acquired_at, renew_by, expires_at, version FROM execution_leases WHERE step_id = ?1 AND state IN ('ACTIVE','RELEASING') ORDER BY epoch DESC LIMIT 1",
        [&command.step_id], lease_from_row,
    ).optional().map_err(map_database_error)?;
    let environment: Option<(String, String, String)> = tx.query_row(
        "SELECT e.runtime_id, r.current_incarnation_id, e.status FROM environments e JOIN runtimes r ON r.runtime_id = e.runtime_id WHERE e.environment_id = ?1 AND e.owner_workspace_id = ?2",
        params![command.environment_id, command.workspace_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    let (environment_runtime_id, environment_incarnation, environment_status) =
        environment.ok_or(StoreError::NotFound)?;
    let runtime_ready: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM runtimes r JOIN runtime_incarnations i ON i.runtime_id = r.runtime_id AND i.runtime_incarnation_id = r.current_incarnation_id JOIN runtime_workspace_bindings b ON b.runtime_id = r.runtime_id AND b.workspace_id = ?1 AND b.status = 'ACTIVE' WHERE r.runtime_id = ?2 AND r.current_incarnation_id = ?3 AND r.availability = 'ONLINE' AND i.recovery_state = 'READY' AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value = 'EXECUTOR'))",
        params![command.workspace_id, command.runtime_id, command.runtime_incarnation_id], |row| row.get(0),
    ).map_err(map_database_error)?;
    let binding_enabled: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2 AND enabled = 1 AND (?3 IS NOT NULL OR lead_eligible = 1))",
        params![command.workspace_id, command.agent_binding_id, command.parent_attempt_id], |row| row.get(0),
    ).map_err(map_database_error)?;
    let dependency_digest = digest(&canonical_json(&plan.steps)?);
    let checks = AttemptAdmissionChecks {
        owner_authorized: ExecutionCheck::Confirmed,
        workspace_active: if workspace_status == "ACTIVE" {
            ExecutionCheck::Confirmed
        } else {
            ExecutionCheck::Denied
        },
        runtime_ready: if runtime_ready {
            ExecutionCheck::Confirmed
        } else {
            ExecutionCheck::Denied
        },
        runtime_executor_binding: if runtime_ready {
            ExecutionCheck::Confirmed
        } else {
            ExecutionCheck::Denied
        },
        // Enabled/READY catalog state alone does not prove adapter compatibility,
        // an owned Environment control lease, or the required mounted inputs.
        agent_compatible: if binding_enabled {
            ExecutionCheck::Unsupported
        } else {
            ExecutionCheck::Denied
        },
        environment_available: if environment_status == "READY"
            && environment_runtime_id == command.runtime_id
        {
            ExecutionCheck::Unsupported
        } else {
            ExecutionCheck::Denied
        },
        inputs_available: ExecutionCheck::Unsupported,
        policy_grants_budget: ExecutionCheck::Unsupported,
        dependency_plan_current: if task.current_plan_revision == Some(plan.revision)
            && task.current_spec_revision == plan.task_spec_revision
        {
            ExecutionCheck::Confirmed
        } else {
            ExecutionCheck::Denied
        },
        effects_reconciled: ExecutionCheck::Unsupported,
        mutation_fencing: ExecutionCheck::Unsupported,
        process_containment: ExecutionCheck::Unsupported,
        private_credential_delivery: ExecutionCheck::Unsupported,
        prior_attempt_settled: if step.current_attempt_id.is_none() {
            ExecutionCheck::Confirmed
        } else {
            ExecutionCheck::Unknown
        },
        explicit_recovery_authorized: ExecutionCheck::Unsupported,
    };
    Ok(StepAttemptAdmissionSnapshot {
        task,
        plan,
        step,
        dependency_steps,
        latest_step_epoch: current_epoch,
        conflicting_lease,
        checks,
        environment_runtime_id,
        environment_runtime_incarnation_id: environment_incarnation,
        environment_id: command.environment_id.clone(),
        selected_agent_binding_id: command.agent_binding_id.clone(),
        dependency_plan_digest: dependency_digest,
        budget_admission: AttemptBudgetAdmission::Unsupported,
    })
}

fn validate_admission_rows(
    connection: &Connection,
    command: &AdmitStepAttempt,
    snapshot: &StepAttemptAdmissionSnapshot,
) -> Result<(), StoreError> {
    validate_parent_profile_tuple(
        command.parent_attempt_id.as_deref(),
        command.delegation_profile_id.as_deref(),
        command.delegation_profile_revision,
    )?;
    if snapshot.task.version != command.expected_task_version
        || snapshot.step.version != command.expected_step_version
        || snapshot.plan.revision != command.expected_plan_revision
        || snapshot.plan.task_spec_revision != command.expected_task_spec_revision
        || snapshot.latest_step_epoch != command.expected_previous_epoch
        || snapshot.dependency_plan_digest != command.dependency_plan_digest
        || snapshot.environment_id != command.environment_id
        || snapshot.environment_runtime_id != command.runtime_id
        || snapshot.environment_runtime_incarnation_id != command.runtime_incarnation_id
        || snapshot.selected_agent_binding_id != command.agent_binding_id
    {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_task_version),
            actual: Some(snapshot.task.version),
        });
    }
    if snapshot.conflicting_lease.is_some() {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    if !matches!(snapshot.task.status.as_str(), "READY" | "RUNNING")
        || snapshot.step.status != "READY"
    {
        return Err(StoreError::Invalid(
            "Task or Step is not currently admissible".to_owned(),
        ));
    }
    if !command.capability_grant_ids.is_empty() {
        return Err(StoreError::Invalid(
            "Attempt scoped Grants are not part of the atomic admission transaction yet".to_owned(),
        ));
    }
    if let Some(parent_id) = &command.parent_attempt_id {
        let (Some(profile_id), Some(profile_revision)) = (
            &command.delegation_profile_id,
            command.delegation_profile_revision,
        ) else {
            return Err(StoreError::Invalid(
                "delegated child Attempt requires a complete parent/profile revision tuple"
                    .to_owned(),
            ));
        };
        let parent_eligible: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM attempts p JOIN execution_leases l ON l.lease_id = p.execution_lease_id JOIN delegation_profiles dp ON dp.delegation_profile_id = ?4 AND dp.workspace_id = ?1 AND dp.agent_binding_id = ?5 AND dp.status = 'ENABLED' AND dp.current_revision = ?6 JOIN agent_bindings b ON b.workspace_id = dp.workspace_id AND b.agent_binding_id = dp.agent_binding_id AND b.enabled = 1 JOIN delegation_profile_revisions dpr ON dpr.delegation_profile_id = dp.delegation_profile_id AND dpr.revision = ?6 AND dpr.workspace_id = ?1 JOIN tasks t ON t.task_id = p.task_id WHERE t.workspace_id = ?1 AND p.task_id = ?2 AND p.attempt_id = ?3 AND p.status = 'RUNNING' AND l.state = 'ACTIVE' AND l.expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now') AND (t.origin_coworker_id IS NULL OR EXISTS(SELECT 1 FROM coworker_revisions cr, json_each(cr.enabled_delegation_profile_ids_json) allowed WHERE cr.coworker_id = t.origin_coworker_id AND cr.revision = t.origin_coworker_revision AND allowed.value = ?4)))",
            params![command.workspace_id, command.task_id, parent_id, profile_id, command.agent_binding_id,
                to_sql_i64(profile_revision, "DelegationProfile revision")?], |row| row.get(0),
        ).map_err(map_database_error)?;
        if !parent_eligible {
            return Err(StoreError::Invalid(
                "delegated parent or pinned worker profile is no longer eligible".to_owned(),
            ));
        }
        return Ok(());
    }
    if snapshot.task.lead_agent_binding_id != command.agent_binding_id {
        return Err(StoreError::Invalid("selected lead differs from the Task's durably selected lead; perform an authorized lead handoff first".to_owned()));
    }
    if !matches!(
        command.failover_class.as_str(),
        "SAFE_PORTABLE" | "REPLAYABLE" | "HANDOFF_REQUIRED" | "LOCAL_BOUND"
    ) {
        return Err(StoreError::Invalid(
            "Attempt failover class is invalid".to_owned(),
        ));
    }
    validate_admission_event_contexts(&command.events, &command.runtime_id)?;
    // There is no durable process-containment or private-credential-delivery evidence
    // in the current schema, so no database facts can authorize starting a provider.
    let _ = connection;
    Ok(())
}

fn validate_parent_profile_tuple(
    parent_attempt_id: Option<&str>,
    profile_id: Option<&str>,
    profile_revision: Option<u64>,
) -> Result<(), StoreError> {
    match (parent_attempt_id, profile_id, profile_revision) {
        (None, None, None) | (Some(_), Some(_), Some(_)) => Ok(()),
        _ => Err(StoreError::Invalid(
            "parent Attempt and DelegationProfile ID/revision must be supplied together".to_owned(),
        )),
    }
}

fn accepted_plan_matches_steps(
    plan: &storage_core::PlanRevisionRecord,
    steps: &[storage_core::StepRecord],
) -> bool {
    if plan.steps.len() != steps.len() {
        return false;
    }
    let by_key = steps
        .iter()
        .filter_map(|step| step.logical_key.as_deref().map(|key| (key, step)))
        .collect::<std::collections::HashMap<_, _>>();
    if by_key.len() != plan.steps.len() {
        return false;
    }
    plan.steps.iter().all(|planned| {
        let Some(step) = by_key.get(planned.logical_key.as_str()) else {
            return false;
        };
        let expected_dependencies = planned
            .depends_on_logical_keys
            .iter()
            .map(|key| by_key.get(key.as_str()).map(|step| step.step_id.as_str()))
            .collect::<Option<Vec<_>>>();
        expected_dependencies.as_ref().is_some_and(|dependencies| {
            step.task_id == plan.task_id
                && step.plan_revision == plan.revision
                && step.title == planned.title
                && step.objective == planned.objective
                && step.required_capabilities == planned.required_capabilities
                && step.acceptance_criteria == planned.acceptance_criteria
                && step
                    .dependencies
                    .iter()
                    .map(String::as_str)
                    .eq(dependencies.iter().copied())
        })
    })
}

fn validate_admission_event_contexts(
    events: &storage_core::AttemptAdmissionEvents,
    runtime_id: &str,
) -> Result<(), StoreError> {
    let all = [&events.attempt, &events.lease, &events.step]
        .into_iter()
        .chain(events.task_status.iter())
        .chain(events.budget.iter())
        .collect::<Vec<_>>();
    if all.len() < 3 || all.len() > 36 {
        return Err(StoreError::Invalid(
            "Attempt event set is incomplete or oversized".to_owned(),
        ));
    }
    let first = all[0];
    let mut ids = std::collections::HashSet::new();
    for event in all {
        if event.origin_runtime_id != runtime_id
            || !ids.insert(event.event_id.as_str())
            || event.correlation_id != first.correlation_id
            || event.hlc_timestamp != first.hlc_timestamp
            || canonicalize_utc_timestamp(&event.recorded_at)? != event.recorded_at
        {
            return Err(StoreError::Invalid(
                "Attempt event contexts are not canonical and mutually bound".to_owned(),
            ));
        }
        validate_timestamp(&event.hlc_timestamp)?;
        if event.causation_id.as_ref().is_some_and(|value| {
            value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control)
        }) {
            return Err(StoreError::Invalid(
                "Attempt event causation ID is invalid".to_owned(),
            ));
        }
    }
    Ok(())
}

fn lease_snapshot(
    connection: &Connection,
    command: &ExecutionLeaseCommand,
) -> Result<ExecutionLeaseMutationSnapshot, StoreError> {
    let task = load_task_view(connection, &command.workspace_id, &command.task_id)?
        .ok_or(StoreError::NotFound)?
        .task;
    let step = list_steps(connection, &command.workspace_id, &command.task_id, None)?
        .into_iter()
        .find(|step| step.step_id == command.step_id)
        .ok_or(StoreError::NotFound)?;
    let attempt = load_attempt(connection, &command.task_id, &command.attempt_id)?
        .ok_or(StoreError::NotFound)?;
    let lease = load_lease(connection, &command.lease_id)?.ok_or(StoreError::NotFound)?;
    let latest_step_epoch: u64 = connection
        .query_row(
            "SELECT COALESCE(MAX(epoch),0) FROM execution_leases WHERE step_id = ?1",
            [&command.step_id],
            |row| from_row_u64(row, 0),
        )
        .map_err(map_database_error)?;
    let now = sqlite_now(connection)?;
    let now_unix_ms = timestamp_unix_ms(&now)?;
    let expires_at_unix_ms = timestamp_unix_ms(&lease.expires_at)?;
    let renew_by_unix_ms = timestamp_unix_ms(&lease.renew_by)?;
    Ok(ExecutionLeaseMutationSnapshot {
        task,
        step,
        attempt,
        lease,
        latest_step_epoch,
        now_unix_ms,
        expires_at_unix_ms,
        renew_by_unix_ms,
        authenticated_runtime: ExecutionCheck::Unsupported,
        credential_verified: ExecutionCheck::Unsupported,
        runtime_current: ExecutionCheck::Unsupported,
        mutation_fencing: ExecutionCheck::Unsupported,
        writer_quiescence: ExecutionCheck::Unsupported,
        invocations_settled: ExecutionCheck::Unsupported,
        effects_reconciled: ExecutionCheck::Unsupported,
    })
}
fn validate_lease_identity(
    command: &ExecutionLeaseCommand,
    snapshot: &ExecutionLeaseMutationSnapshot,
) -> Result<(), StoreError> {
    if snapshot.task.workspace_id != command.workspace_id
        || snapshot.task.task_id != command.task_id
        || snapshot.step.step_id != command.step_id
        || snapshot.step.current_attempt_id.as_deref() != Some(command.attempt_id.as_str())
        || snapshot.attempt.attempt_id != command.attempt_id
        || snapshot.lease.lease_id != command.lease_id
        || snapshot.lease.attempt_id != command.attempt_id
        || snapshot.lease.runtime_id != command.runtime_id
        || snapshot.lease.runtime_incarnation_id != command.runtime_incarnation_id
        || snapshot.lease.epoch != command.expected_epoch
    {
        return Err(StoreError::Invalid(
            "lease identity or fencing epoch does not match".to_owned(),
        ));
    }
    if snapshot.task.version != command.expected_task_version
        || snapshot.attempt.version != command.expected_attempt_version
        || snapshot.lease.version != command.expected_lease_version
    {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_lease_version),
            actual: Some(snapshot.lease.version),
        });
    }
    Err(StoreError::Invalid(
        "private Runtime control authorization receipt is absent from SQLite".to_owned(),
    ))
}

fn load_attempt(
    connection: &Connection,
    task_id: &str,
    attempt_id: &str,
) -> Result<Option<storage_core::AttemptRecord>, StoreError> {
    connection.query_row("SELECT attempt_id, task_id, step_id, parent_attempt_id, agent_binding_id, delegation_profile_id, delegation_profile_revision, agent_session_id, runtime_id, runtime_incarnation_id, environment_id, capability_grant_ids_json, execution_lease_id, failover_class, checkpoint_ref_json, status, failure_json, started_at, settled_at, created_at, version FROM attempts WHERE task_id = ?1 AND attempt_id = ?2",
        params![task_id,attempt_id], attempt_from_row).optional().map_err(map_database_error)
}
fn load_lease(
    connection: &Connection,
    lease_id: &str,
) -> Result<Option<storage_core::ExecutionLeaseRecord>, StoreError> {
    connection.query_row("SELECT lease_id, task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id, epoch, issuer_key_version, fencing_token_digest, state, checkpoint_ref_json, acquired_at, renew_by, expires_at, version FROM execution_leases WHERE lease_id = ?1", [lease_id], lease_from_row).optional().map_err(map_database_error)
}
fn attempt_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<storage_core::AttemptRecord> {
    let grants: String = row.get(11)?;
    let checkpoint: Option<String> = row.get(14)?;
    let failure: Option<String> = row.get(16)?;
    let status: String = row.get(15)?;
    Ok(storage_core::AttemptRecord {
        attempt_id: row.get(0)?,
        task_id: row.get(1)?,
        step_id: row.get(2)?,
        parent_attempt_id: row.get(3)?,
        agent_binding_id: row.get(4)?,
        delegation_profile_id: row.get(5)?,
        delegation_profile_revision: row_u64_opt(row, 6)?,
        agent_session_id: row.get(7)?,
        runtime_id: row.get(8)?,
        runtime_incarnation_id: row.get(9)?,
        environment_id: row.get(10)?,
        capability_grant_ids: serde_json::from_str(&grants).map_err(|e| decode_error(11, e))?,
        execution_lease_id: row.get(12)?,
        failover_class: row.get(13)?,
        checkpoint_ref: checkpoint
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| decode_error(14, e))?,
        status: serde_json::from_str(&format!("\"{status}\"")).map_err(|e| decode_error(15, e))?,
        failure: failure
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| decode_error(16, e))?,
        started_at: row.get(17)?,
        settled_at: row.get(18)?,
        created_at: row.get(19)?,
        version: from_row_u64(row, 20)?,
    })
}
fn lease_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<storage_core::ExecutionLeaseRecord> {
    let checkpoint: Option<String> = row.get(10)?;
    let state: String = row.get(9)?;
    Ok(storage_core::ExecutionLeaseRecord {
        lease_id: row.get(0)?,
        task_id: row.get(1)?,
        step_id: row.get(2)?,
        attempt_id: row.get(3)?,
        runtime_id: row.get(4)?,
        runtime_incarnation_id: row.get(5)?,
        epoch: from_row_u64(row, 6)?,
        issuer_key_version: u32::try_from(from_row_u64(row, 7)?).map_err(|e| decode_error(7, e))?,
        fencing_token_digest: row.get(8)?,
        state: serde_json::from_str(&format!("\"{state}\"")).map_err(|e| decode_error(9, e))?,
        checkpoint_ref: checkpoint
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| decode_error(10, e))?,
        acquired_at: row.get(11)?,
        renew_by: row.get(12)?,
        expires_at: row.get(13)?,
        version: from_row_u64(row, 14)?,
    })
}
fn decode_error(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}
fn sqlite_now(connection: &Connection) -> Result<String, StoreError> {
    let value: String = connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |row| {
            row.get(0)
        })
        .map_err(map_database_error)?;
    canonicalize_utc_timestamp(&value)
}
fn timestamp_unix_ms(value: &str) -> Result<u64, StoreError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| StoreError::Integrity("invalid stored execution timestamp".to_owned()))?;
    u64::try_from(parsed.unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| StoreError::Integrity("execution timestamp predates epoch".to_owned()))
}

#[cfg(test)]
mod delegation_admission_tests {
    use super::*;

    #[test]
    fn parent_and_profile_pin_must_be_absent_or_complete() {
        assert!(validate_parent_profile_tuple(None, None, None).is_ok());
        assert!(validate_parent_profile_tuple(Some("parent"), Some("profile"), Some(1)).is_ok());
        assert!(validate_parent_profile_tuple(Some("parent"), None, None).is_err());
        assert!(validate_parent_profile_tuple(None, Some("profile"), Some(1)).is_err());
        assert!(validate_parent_profile_tuple(Some("parent"), Some("profile"), None).is_err());
    }
}
