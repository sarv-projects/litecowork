//! Durable, revision-pinned VerificationRun transitions.
//!
//! This module does not execute verification and cannot mark Evidence VERIFIED by
//! accepting verifier-supplied JSON. A future authenticated Core service must supply
//! verifier identity; SQLite binds that declared identity to each run and rechecks
//! TaskSpec/input heads on writes. PASSED remains rejected until same-run Evidence
//! provenance is enforceable at this boundary.

use super::*;
use domain_effects::{
    VerificationFailureCode, VerificationInput, VerificationResourceRef, VerificationRunRecord,
    VerificationRunStatus,
};
use serde::Serialize;
use storage_core::{
    CommittedVerificationRun, SettleVerificationRunCommit, StartVerificationRunCommit,
    VerificationRunActor, VerificationRunEventContext, VerificationRunStore,
};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

impl VerificationRunStore for SqliteWorkspaceStore {
    fn start_verification_run(
        &self,
        mut command: StartVerificationRunCommit,
    ) -> Result<CommittedVerificationRun, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_event_context(
            &command.workspace_id,
            &command.event.origin_runtime_id,
            &command.event.origin_runtime_incarnation_id,
            &command.event,
        )?;
        if command.run.status != VerificationRunStatus::Pending
            || command.run.version != 1
            || command.run.started_at.is_some()
            || command.run.completed_at.is_some()
            || !command.run.evidence_refs.is_empty()
            || command.run.failure_code.is_some()
            || command.run.task_spec_revision == 0
            || command.run.verifier_kind != command.actor.verifier_service_id
        {
            return Err(StoreError::Invalid(
                "VerificationRun start must be a new pending run from its authenticated verifier"
                    .into(),
            ));
        }
        validate_actor(&command.workspace_id, &command.actor, &command.request_id)?;
        let running = command
            .run
            .start(command.event.recorded_at.clone())
            .map_err(|_| {
                StoreError::Invalid("VerificationRun start transition is invalid".into())
            })?;
        let state_ref = put_state(
            self,
            &command.workspace_id,
            &running,
            "application/vnd.litecowork.verification-run+json",
            running.version,
        )?;
        let (reply, receive) = mpsc::channel();
        self.execute_command(
            Command::VerificationRunOperation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(start_transaction(connection, command, running, state_ref));
                }),
            },
            receive,
        )
    }

    fn settle_verification_run(
        &self,
        mut command: SettleVerificationRunCommit,
    ) -> Result<CommittedVerificationRun, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_event_context(
            &command.workspace_id,
            &command.event.origin_runtime_id,
            &command.event.origin_runtime_incarnation_id,
            &command.event,
        )?;
        validate_actor(&command.workspace_id, &command.actor, &command.request_id)?;
        if command.expected_version == 0
            || !command.next_status.terminal()
            || command.next_status == VerificationRunStatus::Passed
            || (matches!(
                command.next_status,
                VerificationRunStatus::Failed | VerificationRunStatus::Inconclusive
            ) && command.failure_code.is_none())
        {
            return Err(StoreError::Invalid(
                "VerificationRun settlement is invalid or lacks a typed outcome".into(),
            ));
        }
        let workspace = command.workspace_id.clone();
        let run_id = command.verification_run_id.clone();
        let expected_version = command.expected_version;
        let principal = runtime_principal(
            &command.event.origin_runtime_id,
            &command.event.origin_runtime_incarnation_id,
        );
        let request_fingerprint = digest(&canonical_json(&json!({
            "workspace_id":workspace,
            "actor":command.actor,
            "run_id":run_id,
            "expected_version":expected_version,
            "status":command.next_status,
            "evidence_refs":command.evidence_refs,
            "failure_code":command.failure_code,
        }))?);
        let probe_workspace = workspace.clone();
        let probe_actor = command.actor.clone();
        let probe_request_id = command.request_id.clone();
        let probe_event = command.event.clone();
        let probe_principal = principal.clone();
        let probe_fingerprint = request_fingerprint.clone();
        let (prior, current) = self.run_verification(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_runtime(
                &tx,
                &probe_workspace,
                &probe_event.origin_runtime_id,
                &probe_event.origin_runtime_incarnation_id,
            )?;
            authorize_owner(&tx, &probe_workspace, &probe_actor.owner_principal_id)?;
            let prior = replay::<CommittedVerificationRun>(
                &tx,
                &probe_principal,
                &probe_request_id,
                &probe_fingerprint,
            )?;
            let current = if prior.is_none() {
                Some(load_run(&tx, &probe_workspace, &run_id)?.ok_or(StoreError::NotFound)?)
            } else {
                None
            };
            tx.commit().map_err(map_database_error)?;
            Ok((prior, current))
        })?;
        if let Some(prior) = prior {
            return Ok(prior);
        }
        let current = current.ok_or(StoreError::NotFound)?;
        if current.version != command.expected_version {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_version),
                actual: Some(current.version),
            });
        }
        if current.verifier_kind != command.actor.verifier_service_id {
            return Err(StoreError::Invalid(
                "only the verifier that started this run may settle it".into(),
            ));
        }
        let settled = current
            .finish(
                command.next_status,
                command.evidence_refs.clone(),
                command.failure_code,
                command.event.recorded_at.clone(),
            )
            .map_err(|_| StoreError::Invalid("VerificationRun result is invalid".into()))?;
        let state_ref = put_state(
            self,
            &command.workspace_id,
            &settled,
            "application/vnd.litecowork.verification-run+json",
            settled.version,
        )?;
        self.run_verification(move |connection| {
            settle_transaction(connection, command, current, settled, state_ref)
        })
    }

    fn get_verification_run(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        verification_run_id: &str,
    ) -> Result<Option<VerificationRunRecord>, StoreError> {
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let run_id = verification_run_id.to_owned();
        self.run_verification(move |connection| {
            if invalid_identifier(&owner, 256)
                || invalid_identifier(&workspace, 256)
                || invalid_identifier(&run_id, 256)
            {
                return Err(StoreError::Invalid(
                    "VerificationRun query is invalid".into(),
                ));
            }
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &owner)?;
            let run = load_run(&tx, &workspace, &run_id)?;
            tx.commit().map_err(map_database_error)?;
            Ok(run)
        })
    }

    fn list_verification_runs(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        task_id: &str,
        limit: usize,
    ) -> Result<Vec<VerificationRunRecord>, StoreError> {
        if !(1..=200).contains(&limit)
            || invalid_identifier(owner_principal_id, 256)
            || invalid_identifier(workspace_id, 256)
            || invalid_identifier(task_id, 256)
        {
            return Err(StoreError::Invalid(
                "VerificationRun page query is invalid".into(),
            ));
        }
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let task = task_id.to_owned();
        self.run_verification(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_task(&tx, &owner, &workspace, &task)?;
            let mut statement = tx.prepare(
                "SELECT verification_run_id FROM verification_runs WHERE task_id=?1 ORDER BY started_at DESC, verification_run_id DESC LIMIT ?2",
            ).map_err(map_database_error)?;
            let ids = statement.query_map(params![task, limit as i64], |row| row.get::<_, String>(0))
                .map_err(map_database_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_database_error)?;
            drop(statement);
            let mut runs = Vec::with_capacity(ids.len());
            for id in ids {
                if let Some(run) = load_run(&tx, &workspace, &id)? {
                    runs.push(run);
                }
            }
            tx.commit().map_err(map_database_error)?;
            Ok(runs)
        })
    }
}

impl SqliteWorkspaceStore {
    fn run_verification<T, F>(&self, operation: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    {
        let (reply, receive) = mpsc::channel();
        self.execute_command(
            Command::VerificationRunOperation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }
}

fn start_transaction(
    connection: &mut Connection,
    command: StartVerificationRunCommit,
    running: VerificationRunRecord,
    state_ref: AggregateStateRef,
) -> Result<CommittedVerificationRun, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_runtime(
        &tx,
        &command.workspace_id,
        &command.event.origin_runtime_id,
        &command.event.origin_runtime_incarnation_id,
    )?;
    authorize_owner(
        &tx,
        &command.workspace_id,
        &command.actor.owner_principal_id,
    )?;
    let principal = runtime_principal(
        &command.event.origin_runtime_id,
        &command.event.origin_runtime_incarnation_id,
    );
    let fingerprint = digest(&canonical_json(&json!({
        "workspace_id":command.workspace_id,
        "actor":command.actor,
        "run":command.run,
    }))?);
    if let Some(prior) =
        replay::<CommittedVerificationRun>(&tx, &principal, &command.request_id, &fingerprint)?
    {
        return Ok(prior);
    }
    check_task_criterion_and_inputs(&tx, &command.workspace_id, &command.run, false)?;
    tx.execute(
        "INSERT INTO verification_runs(verification_run_id,task_id,criterion_id,task_spec_revision,criterion_digest,verifier_kind,verifier_version,subject_refs_json,inputs_json,status,evidence_refs_json,started_at,completed_at,version,failure_code) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'PENDING','[]',NULL,NULL,1,NULL)",
        params![
            running.verification_run_id,
            running.task_id,
            running.criterion_id,
            to_sql_i64(running.task_spec_revision, "TaskSpec revision")?,
            running.criterion_digest,
            running.verifier_kind,
            running.verifier_version,
            encode(&running.subject_refs)?,
            encode(&running.inputs)?,
        ],
    ).map_err(map_database_error)?;
    tx.execute(
        "UPDATE verification_runs SET status='RUNNING',started_at=?1,version=2 WHERE verification_run_id=?2 AND status='PENDING' AND version=1",
        params![command.event.recorded_at, running.verification_run_id],
    )
    .map_err(map_database_error)?;
    insert_dependency_edges(
        &tx,
        &command.workspace_id,
        &running,
        &command.event.recorded_at,
    )?;
    let payload = run_payload(&running, None);
    let event = write_verification_event(
        &tx,
        &command.workspace_id,
        &running,
        &command.event,
        "verification.started.v1",
        payload,
        state_ref,
    )?;
    let result = CommittedVerificationRun {
        run: running,
        event,
        replayed: false,
    };
    save_replay(
        &tx,
        &principal,
        &command.request_id,
        &fingerprint,
        &result,
        &command.event.recorded_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(result)
}

fn settle_transaction(
    connection: &mut Connection,
    command: SettleVerificationRunCommit,
    current: VerificationRunRecord,
    settled: VerificationRunRecord,
    state_ref: AggregateStateRef,
) -> Result<CommittedVerificationRun, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_runtime(
        &tx,
        &command.workspace_id,
        &command.event.origin_runtime_id,
        &command.event.origin_runtime_incarnation_id,
    )?;
    authorize_owner(
        &tx,
        &command.workspace_id,
        &command.actor.owner_principal_id,
    )?;
    let principal = runtime_principal(
        &command.event.origin_runtime_id,
        &command.event.origin_runtime_incarnation_id,
    );
    let fingerprint = digest(&canonical_json(&json!({
        "workspace_id":command.workspace_id,
        "actor":command.actor,
        "run_id":command.verification_run_id,
        "expected_version":command.expected_version,
        "status":command.next_status,
        "evidence_refs":command.evidence_refs,
        "failure_code":command.failure_code,
    }))?);
    if let Some(prior) =
        replay::<CommittedVerificationRun>(&tx, &principal, &command.request_id, &fingerprint)?
    {
        return Ok(prior);
    }
    let persisted = load_run(&tx, &command.workspace_id, &command.verification_run_id)?
        .ok_or(StoreError::NotFound)?;
    if persisted != current || persisted.version != command.expected_version {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(persisted.version),
        });
    }
    if persisted.verifier_kind != command.actor.verifier_service_id {
        return Err(StoreError::Invalid(
            "VerificationRun verifier identity changed".into(),
        ));
    }
    // A run is bound to the TaskSpec and exact Resource heads observed when it
    // started. Recheck those heads under the same write transaction that settles
    // the run so a source edit or Task cancellation cannot race a result commit.
    check_task_criterion_and_inputs(
        &tx,
        &command.workspace_id,
        &persisted,
        command.failure_code == Some(VerificationFailureCode::TaskCancelled),
    )?;
    tx.execute(
        "UPDATE verification_runs SET status=?1,evidence_refs_json=?2,completed_at=?3,version=?4,failure_code=?5 WHERE verification_run_id=?6 AND status='RUNNING' AND version=?7",
        params![
            command.next_status.as_str(),
            encode(&settled.evidence_refs)?,
            command.event.recorded_at,
            to_sql_i64(settled.version, "VerificationRun version")?,
            settled.failure_code.map(VerificationFailureCode::as_str),
            settled.verification_run_id,
            to_sql_i64(command.expected_version, "expected VerificationRun version")?,
        ],
    ).map_err(map_database_error)?;
    let payload = run_payload(&settled, settled.failure_code);
    let event = write_verification_event(
        &tx,
        &command.workspace_id,
        &settled,
        &command.event,
        "verification.completed.v2",
        payload,
        state_ref,
    )?;
    let result = CommittedVerificationRun {
        run: settled,
        event,
        replayed: false,
    };
    save_replay(
        &tx,
        &principal,
        &command.request_id,
        &fingerprint,
        &result,
        &command.event.recorded_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(result)
}

fn check_task_criterion_and_inputs(
    tx: &Transaction<'_>,
    workspace: &str,
    run: &VerificationRunRecord,
    allow_cancelled: bool,
) -> Result<(), StoreError> {
    let (status, current_spec, criteria): (String, i64, String) = tx.query_row(
        "SELECT task.status,task.current_spec_revision,spec.acceptance_criteria_json FROM tasks task JOIN task_spec_revisions spec ON spec.task_id=task.task_id AND spec.workspace_id=task.workspace_id AND spec.revision=?3 WHERE task.workspace_id=?1 AND task.task_id=?2",
        params![workspace, run.task_id, to_sql_i64(run.task_spec_revision, "TaskSpec revision")?],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(map_database_error)?;
    let cancelled = allow_cancelled && matches!(status.as_str(), "CANCEL_REQUESTED" | "CANCELLED");
    if (!cancelled && status != "VERIFYING")
        || (!cancelled && current_spec != to_sql_i64(run.task_spec_revision, "TaskSpec revision")?)
    {
        return Err(StoreError::Invalid(
            "Task is no longer verifying the pinned TaskSpec revision".into(),
        ));
    }
    let criteria: Vec<Value> = serde_json::from_str(&criteria)
        .map_err(|_| StoreError::Integrity("TaskSpec acceptance criteria are invalid".into()))?;
    let matching_criteria = criteria
        .iter()
        .filter(|criterion| {
            criterion.get("criterion_id").and_then(Value::as_str) == Some(run.criterion_id.as_str())
        })
        .collect::<Vec<_>>();
    let [criterion] = matching_criteria.as_slice() else {
        return Err(StoreError::Invalid(
            "VerificationRun criterion must occur exactly once in the pinned TaskSpec".into(),
        ));
    };
    if digest(&canonical_json(criterion)?) != run.criterion_digest {
        return Err(StoreError::Invalid(
            "VerificationRun criterion digest is stale".into(),
        ));
    }
    for input in &run.inputs {
        if cancelled {
            validate_input_revision(tx, workspace, input)?;
        } else {
            validate_input_head(tx, workspace, input)?;
        }
    }
    for subject in &run.subject_refs {
        validate_resource_ref(tx, workspace, subject)?;
    }
    Ok(())
}

fn validate_input_head(
    tx: &Transaction<'_>,
    workspace: &str,
    input: &VerificationInput,
) -> Result<(), StoreError> {
    if input.resource_ref.workspace_id != workspace {
        return Err(StoreError::Invalid(
            "cross-Workspace verifier inputs are unsupported".into(),
        ));
    }
    let observed: Option<Option<String>> = tx.query_row(
        "SELECT revision.content_digest
         FROM resources resource
         JOIN resource_revisions revision
           ON revision.resource_id=resource.resource_id
          AND revision.resource_revision_id=?3
         WHERE resource.workspace_id=?1
           AND resource.resource_id=?2
           AND resource.current_revision_id=revision.resource_revision_id
           AND NOT EXISTS (
             SELECT 1
             FROM resource_revision_parents child
             WHERE child.resource_id=revision.resource_id
               AND child.parent_revision_id=revision.resource_revision_id
           )
           AND NOT EXISTS (
             SELECT 1
             FROM resource_revisions other_head
             WHERE other_head.resource_id=resource.resource_id
               AND other_head.resource_revision_id<>revision.resource_revision_id
               AND NOT EXISTS (
                 SELECT 1
                 FROM resource_revision_parents child
                 WHERE child.resource_id=other_head.resource_id
                   AND child.parent_revision_id=other_head.resource_revision_id
               )
           )",
        params![workspace, input.resource_ref.resource_id, input.resource_ref.revision_id],
        |row| row.get(0),
    ).optional().map_err(map_database_error)?;
    let Some(provider_digest) = observed else {
        return Err(StoreError::Invalid(
            "VerificationRun input is stale or unavailable".into(),
        ));
    };
    if input
        .consumed_digest
        .as_ref()
        .is_some_and(|consumed| provider_digest.as_ref() != Some(consumed))
    {
        return Err(StoreError::Integrity(
            "verifier-consumed digest does not match Resource revision".into(),
        ));
    }
    Ok(())
}

fn validate_input_revision(
    tx: &Transaction<'_>,
    workspace: &str,
    input: &VerificationInput,
) -> Result<(), StoreError> {
    if input.resource_ref.workspace_id != workspace {
        return Err(StoreError::Invalid(
            "cross-Workspace verifier inputs are unsupported".into(),
        ));
    }
    let provider_digest: Option<Option<String>> = tx
        .query_row(
            "SELECT revision.content_digest FROM resources resource JOIN resource_revisions revision ON revision.resource_id=resource.resource_id AND revision.resource_revision_id=?3 WHERE resource.workspace_id=?1 AND resource.resource_id=?2",
            params![workspace, input.resource_ref.resource_id, input.resource_ref.revision_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some(provider_digest) = provider_digest else {
        return Err(StoreError::Invalid(
            "VerificationRun input revision is unavailable".into(),
        ));
    };
    if input
        .consumed_digest
        .as_ref()
        .is_some_and(|consumed| provider_digest.as_ref() != Some(consumed))
    {
        return Err(StoreError::Integrity(
            "verifier-consumed digest does not match Resource revision".into(),
        ));
    }
    Ok(())
}

fn validate_resource_ref(
    tx: &Transaction<'_>,
    workspace: &str,
    reference: &VerificationResourceRef,
) -> Result<(), StoreError> {
    if reference.workspace_id != workspace {
        return Err(StoreError::Invalid(
            "cross-Workspace verifier subjects are unsupported".into(),
        ));
    }
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM resources resource JOIN resource_revisions revision ON revision.resource_id=resource.resource_id AND revision.resource_revision_id=?3 WHERE resource.workspace_id=?1 AND resource.resource_id=?2)",
        params![workspace, reference.resource_id, reference.revision_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if exists {
        Ok(())
    } else {
        Err(StoreError::Invalid(
            "VerificationRun subject revision is unavailable".into(),
        ))
    }
}

fn insert_dependency_edges(
    tx: &Transaction<'_>,
    workspace: &str,
    run: &VerificationRunRecord,
    at: &str,
) -> Result<(), StoreError> {
    for input in &run.inputs {
        let edge_id = format!(
            "verification:{}:{}:{}",
            run.verification_run_id, input.resource_ref.resource_id, input.resource_ref.revision_id
        );
        tx.execute(
            "INSERT INTO dependency_edges(dependency_edge_id,workspace_id,source_resource_id,source_revision_id,dependent_kind,dependent_ref,artifact_id,artifact_version,verification_run_id,created_at) VALUES (?1,?2,?3,?4,'VERIFICATION_RUN','verification://' || ?5,NULL,NULL,?5,?6)",
            params![edge_id, workspace, input.resource_ref.resource_id, input.resource_ref.revision_id, run.verification_run_id, at],
        ).map_err(map_database_error)?;
    }
    Ok(())
}

fn run_payload(
    run: &VerificationRunRecord,
    failure_code: Option<VerificationFailureCode>,
) -> Value {
    let mut payload = json!({
        "verification_run_id":run.verification_run_id,
        "task_id":run.task_id,
        "criterion_id":run.criterion_id,
        "task_spec_revision":run.task_spec_revision,
        "criterion_digest":run.criterion_digest,
        "verifier_kind":run.verifier_kind,
        "verifier_version":run.verifier_version,
        "subject_refs":run.subject_refs,
        "inputs":run.inputs,
        "status":run.status,
        "evidence_refs":run.evidence_refs,
        "aggregate_version":run.version,
    });
    if let Some(failure_code) = failure_code {
        payload["failure_code"] = json!(failure_code);
    }
    payload
}

fn write_verification_event(
    tx: &Transaction<'_>,
    workspace: &str,
    run: &VerificationRunRecord,
    context: &VerificationRunEventContext,
    event_type: &str,
    payload: Value,
    state_ref: AggregateStateRef,
) -> Result<storage_core::DomainEvent, StoreError> {
    tx.execute(
        "INSERT INTO workspace_origin_sequences(workspace_id,origin_runtime_id,last_sequence) VALUES (?1,?2,1) ON CONFLICT(workspace_id,origin_runtime_id) DO UPDATE SET last_sequence=last_sequence+1",
        params![workspace, context.origin_runtime_id],
    ).map_err(map_database_error)?;
    let sequence: i64 = tx.query_row(
        "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id=?1 AND origin_runtime_id=?2",
        params![workspace, context.origin_runtime_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    let payload_json = String::from_utf8(canonical_json(&payload)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let state_json = String::from_utf8(canonical_json(&state_ref)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let payload_digest = digest(payload_json.as_bytes());
    // Envelope schema_version tracks the DomainEvent record format; the `.vN`
    // suffix selects the payload schema, as with the other `*.v2` events.
    let schema_version = 1;
    tx.execute(
        "INSERT INTO domain_events(event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,causation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest) VALUES (?1,?2,'VerificationRun',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![context.event_id,workspace,run.verification_run_id,context.origin_runtime_id,sequence,to_sql_i64(run.version,"VerificationRun version")?,context.hlc_timestamp,context.correlation_id,context.causation_id,schema_version,event_type,payload_json,state_json,context.recorded_at,payload_digest],
    ).map_err(map_database_error)?;
    Ok(storage_core::DomainEvent {
        event_id: context.event_id.clone(),
        workspace_id: workspace.to_owned(),
        entity_type: "VerificationRun".into(),
        entity_id: run.verification_run_id.clone(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        origin_sequence: from_sql_i64(sequence, "origin sequence")?,
        entity_revision: run.version,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        schema_version,
        event_type: event_type.to_owned(),
        payload,
        aggregate_state_ref: state_ref,
        recorded_at: context.recorded_at.clone(),
        payload_digest,
    })
}

fn validate_actor(
    workspace: &str,
    actor: &VerificationRunActor,
    request_id: &str,
) -> Result<(), StoreError> {
    if invalid_identifier(workspace, 256)
        || invalid_identifier(&actor.owner_principal_id, 256)
        || invalid_identifier(&actor.verifier_service_id, 256)
        || invalid_identifier(request_id, 256)
    {
        return Err(StoreError::Invalid(
            "VerificationRun actor or request identity is invalid".into(),
        ));
    }
    Ok(())
}

fn authorize_owner(tx: &Transaction<'_>, workspace: &str, owner: &str) -> Result<(), StoreError> {
    let authorized: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
        params![workspace, owner],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if authorized {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

fn authorize_task(
    tx: &Transaction<'_>,
    owner: &str,
    workspace: &str,
    task: &str,
) -> Result<(), StoreError> {
    let authorized: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks task JOIN workspaces workspace ON workspace.workspace_id=task.workspace_id WHERE task.task_id=?3 AND task.workspace_id=?2 AND workspace.owner_principal_id=?1 AND workspace.status='ACTIVE')",
            params![owner, workspace, task],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if authorized {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

fn authorize_runtime(
    tx: &Transaction<'_>,
    workspace: &str,
    runtime: &str,
    incarnation: &str,
) -> Result<(), StoreError> {
    let authorized: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces w JOIN runtimes r ON r.runtime_id=?2 JOIN runtime_incarnations i ON i.runtime_id=r.runtime_id AND i.runtime_incarnation_id=?3 AND i.recovery_state IN ('READY','DEGRADED') JOIN runtime_workspace_bindings b ON b.runtime_id=r.runtime_id AND b.workspace_id=w.workspace_id AND b.status='ACTIVE' AND b.revoked_at IS NULL WHERE w.workspace_id=?1 AND w.status='ACTIVE' AND r.current_incarnation_id=i.runtime_incarnation_id AND r.availability IN ('ONLINE','DEGRADED') AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value='EXECUTOR'))",
        params![workspace,runtime,incarnation], |row| row.get(0),
    ).map_err(map_database_error)?;
    if authorized {
        Ok(())
    } else {
        Err(StoreError::Invalid(
            "Runtime cannot write VerificationRuns in this Workspace".into(),
        ))
    }
}

fn validate_event_context(
    workspace: &str,
    runtime: &str,
    incarnation: &str,
    event: &VerificationRunEventContext,
) -> Result<(), StoreError> {
    if [
        workspace,
        runtime,
        incarnation,
        event.event_id.as_str(),
        event.hlc_timestamp.as_str(),
        event.correlation_id.as_str(),
    ]
    .iter()
    .any(|value| invalid_identifier(value, 256))
        || event.origin_runtime_id != runtime
        || event.origin_runtime_incarnation_id != incarnation
        || event
            .causation_id
            .as_ref()
            .is_some_and(|value| invalid_identifier(value, 256))
    {
        return Err(StoreError::Invalid(
            "VerificationRun event context is invalid".into(),
        ));
    }
    Ok(())
}

fn load_run(
    tx: &Transaction<'_>,
    workspace: &str,
    run_id: &str,
) -> Result<Option<VerificationRunRecord>, StoreError> {
    let row: Option<(String,String,String,i64,String,String,String,String,String,String,String,Option<String>,Option<String>,i64,Option<String>)> = tx.query_row(
        "SELECT run.verification_run_id,run.task_id,run.criterion_id,run.task_spec_revision,run.criterion_digest,run.verifier_kind,run.verifier_version,run.subject_refs_json,run.inputs_json,run.status,run.evidence_refs_json,run.started_at,run.completed_at,run.version,run.failure_code FROM verification_runs run JOIN tasks task ON task.task_id=run.task_id WHERE task.workspace_id=?1 AND run.verification_run_id=?2",
        params![workspace,run_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?,row.get(12)?,row.get(13)?,row.get(14)?)),
    ).optional().map_err(map_database_error)?;
    row.map(
        |(
            verification_run_id,
            task_id,
            criterion_id,
            revision,
            criterion_digest,
            verifier_kind,
            verifier_version,
            subjects,
            inputs,
            status,
            evidence_refs,
            started_at,
            completed_at,
            version,
            failure_code,
        )| {
            let status = match status.as_str() {
                "PENDING" => VerificationRunStatus::Pending,
                "RUNNING" => VerificationRunStatus::Running,
                "PASSED" => VerificationRunStatus::Passed,
                "FAILED" => VerificationRunStatus::Failed,
                "INCONCLUSIVE" => VerificationRunStatus::Inconclusive,
                _ => {
                    return Err(StoreError::Integrity(
                        "VerificationRun status is invalid".into(),
                    ));
                }
            };
            let failure_code = failure_code
                .map(|code| match code.as_str() {
                    "TIMEOUT" => Ok(VerificationFailureCode::Timeout),
                    "INTEGRITY_FAILURE" => Ok(VerificationFailureCode::IntegrityFailure),
                    "VERIFIER_UNAVAILABLE" => Ok(VerificationFailureCode::VerifierUnavailable),
                    "UNSUPPORTED_CRITERION" => Ok(VerificationFailureCode::UnsupportedCriterion),
                    "INPUT_UNAVAILABLE" => Ok(VerificationFailureCode::InputUnavailable),
                    "TASK_CANCELLED" => Ok(VerificationFailureCode::TaskCancelled),
                    _ => Err(StoreError::Integrity(
                        "VerificationRun failure code is invalid".into(),
                    )),
                })
                .transpose()?;
            let run = VerificationRunRecord {
                verification_run_id,
                task_id,
                criterion_id,
                task_spec_revision: from_sql_i64(revision, "TaskSpec revision")?,
                criterion_digest,
                verifier_kind,
                verifier_version,
                subject_refs: serde_json::from_str(&subjects).map_err(|_| {
                    StoreError::Integrity("VerificationRun subject refs are invalid".into())
                })?,
                inputs: serde_json::from_str(&inputs).map_err(|_| {
                    StoreError::Integrity("VerificationRun inputs are invalid".into())
                })?,
                status,
                evidence_refs: serde_json::from_str(&evidence_refs).map_err(|_| {
                    StoreError::Integrity("VerificationRun evidence refs are invalid".into())
                })?,
                started_at,
                completed_at,
                version: from_sql_i64(version, "VerificationRun version")?,
                failure_code,
            };
            run.validate()
                .map_err(|_| StoreError::Integrity("stored VerificationRun is invalid".into()))?;
            Ok(run)
        },
    )
    .transpose()
}

fn replay<T: serde::de::DeserializeOwned + SetReplayed>(
    tx: &Transaction<'_>,
    principal: &str,
    request_id: &str,
    fingerprint: &str,
) -> Result<Option<T>, StoreError> {
    let prior: Option<(String,Option<String>)> = tx.query_row(
        "SELECT request_digest,response_json FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
        params![principal,request_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(map_database_error)?;
    match prior {
        None => Ok(None),
        Some((found, Some(response))) if found == fingerprint => {
            let mut result: T = serde_json::from_str(&response)
                .map_err(|_| StoreError::Integrity("VerificationRun receipt is invalid".into()))?;
            result.set_replayed();
            Ok(Some(result))
        }
        Some((found, None)) if found == fingerprint => Ok(None),
        Some(_) => Err(StoreError::Invalid(
            "request ID was reused with different VerificationRun content".into(),
        )),
    }
}

trait SetReplayed {
    fn set_replayed(&mut self);
}
impl SetReplayed for CommittedVerificationRun {
    fn set_replayed(&mut self) {
        self.replayed = true;
    }
}

fn save_replay<T: Serialize>(
    tx: &Transaction<'_>,
    principal: &str,
    request_id: &str,
    fingerprint: &str,
    response: &T,
    at: &str,
) -> Result<(), StoreError> {
    let response = encode(response)?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![principal,request_id,fingerprint,response,digest(response.as_bytes()),at],
    ).map_err(map_database_error)?;
    Ok(())
}

fn put_state<T: Serialize>(
    store: &SqliteWorkspaceStore,
    workspace: &str,
    value: &T,
    media_type: &str,
    revision: u64,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(value)?;
    let blob = store
        .inner
        .blobs
        .put(workspace, BlobPurpose::AggregateState, &bytes, media_type)?;
    if blob.digest != digest(&bytes)
        || blob.size_bytes != bytes.len() as u64
        || store
            .inner
            .blobs
            .get(workspace, BlobPurpose::AggregateState, &blob)?
            != bytes
    {
        return Err(StoreError::Integrity(
            "VerificationRun aggregate-state blob failed verification".into(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: revision,
        record_schema_version: 1,
    })
}

fn encode<T: Serialize>(value: &T) -> Result<String, StoreError> {
    String::from_utf8(canonical_json(value)?)
        .map_err(|_| StoreError::Integrity("canonical JSON was not UTF-8".to_owned()))
}

fn invalid_identifier(value: &str, max: usize) -> bool {
    value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control)
}

fn runtime_principal(runtime: &str, incarnation: &str) -> String {
    format!("runtime:{runtime}:{incarnation}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running_run() -> VerificationRunRecord {
        VerificationRunRecord {
            verification_run_id: "verify-1".into(),
            task_id: "task-1".into(),
            criterion_id: "criterion-1".into(),
            task_spec_revision: 7,
            criterion_digest: format!("sha256:{}", "a".repeat(64)),
            verifier_kind: "test-runner".into(),
            verifier_version: "1.2.3".into(),
            subject_refs: vec![],
            inputs: vec![],
            status: VerificationRunStatus::Running,
            evidence_refs: vec![],
            started_at: Some("2026-10-16T11:00:00Z".into()),
            completed_at: None,
            version: 2,
            failure_code: None,
        }
    }

    #[test]
    fn started_v1_payload_omits_the_v2_failure_code_field() {
        let payload = run_payload(&running_run(), None);

        assert_eq!(payload["status"], "RUNNING");
        assert_eq!(payload["aggregate_version"], 2);
        assert!(payload.get("failure_code").is_none());
    }

    #[test]
    fn failed_v2_payload_contains_only_the_typed_failure_code() {
        let mut run = running_run();
        run.status = VerificationRunStatus::Inconclusive;
        run.completed_at = Some("2026-10-16T11:01:00Z".into());
        run.version = 3;
        run.failure_code = Some(VerificationFailureCode::Timeout);
        let payload = run_payload(&run, run.failure_code);

        assert_eq!(payload["status"], "INCONCLUSIVE");
        assert_eq!(payload["failure_code"], "TIMEOUT");
        let encoded = serde_json::to_string(&payload).expect("payload serializes");
        assert!(!encoded.contains("raw verifier output"));
    }

    #[test]
    fn sqlite_v20_trigger_rejects_pass_even_with_a_caller_supplied_evidence_ref() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch(SQLITE_V1_DDL)
            .expect("baseline schema");
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .expect("disable unrelated fixture foreign keys");
        connection
            .execute_batch(SQLITE_V20_DDL)
            .expect("VerificationRun transition guards");
        connection
            .execute(
                "INSERT INTO verification_runs(verification_run_id,task_id,criterion_id,task_spec_revision,criterion_digest,verifier_kind,verifier_version,subject_refs_json,inputs_json,status,evidence_refs_json) VALUES ('run','task','criterion',1,?1,'verifier','1','[]','[]','PENDING','[]')",
                [format!("sha256:{}", "a".repeat(64))],
            )
            .expect("insert pending run");
        connection
            .execute(
                "UPDATE verification_runs SET status='RUNNING',started_at='2026-10-16T12:00:00Z',version=2 WHERE verification_run_id='run'",
                [],
            )
            .expect("start run");
        let passed = connection.execute(
            "UPDATE verification_runs SET status='PASSED',evidence_refs_json='[\"caller-evidence\"]',completed_at='2026-10-16T12:00:01Z',version=3 WHERE verification_run_id='run'",
            [],
        );
        assert!(passed.is_err(), "SQLite must reject caller-supplied PASS");

        connection
            .execute(
                "UPDATE verification_runs SET status='INCONCLUSIVE',completed_at='2026-10-16T12:00:01Z',version=3,failure_code='VERIFIER_UNAVAILABLE' WHERE verification_run_id='run'",
                [],
            )
            .expect("typed inconclusive result is legal");
        let terminal_rewrite = connection.execute(
            "UPDATE verification_runs SET status='FAILED',failure_code='TIMEOUT',version=4 WHERE verification_run_id='run'",
            [],
        );
        assert!(
            terminal_rewrite.is_err(),
            "terminal result must be immutable"
        );
    }

    #[test]
    fn conflicted_resource_heads_cannot_be_pinned_as_current_verification_input() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch(
                "CREATE TABLE resources(workspace_id TEXT, resource_id TEXT, current_revision_id TEXT);
                 CREATE TABLE resource_revisions(resource_revision_id TEXT, resource_id TEXT, content_digest TEXT);
                 CREATE TABLE resource_revision_parents(resource_id TEXT, child_revision_id TEXT, parent_revision_id TEXT);
                 INSERT INTO resources VALUES ('workspace-1', 'resource-1', 'head-a');
                 INSERT INTO resource_revisions VALUES ('head-a', 'resource-1', NULL);
                 INSERT INTO resource_revisions VALUES ('head-b', 'resource-1', NULL);",
            )
            .expect("resource revision fixture");
        let tx = connection.transaction().expect("read transaction");
        let input = VerificationInput {
            resource_ref: VerificationResourceRef {
                workspace_id: "workspace-1".into(),
                resource_id: "resource-1".into(),
                revision_id: "head-a".into(),
            },
            consumed_digest: None,
        };

        assert!(validate_input_head(&tx, "workspace-1", &input).is_err());
    }
}
