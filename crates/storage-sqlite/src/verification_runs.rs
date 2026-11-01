//! Durable, revision-pinned VerificationRun transitions.
//!
//! This module does not execute verification. It accepts a result only from the
//! authenticated verifier identity pinned on the run, and commits VERIFIED Evidence
//! with its exact run binding in the same transaction as a passing result.

use super::*;
use crate::workspace_access::{require_read_access, require_write_access};
use domain_effects::{
    EvidenceLevel, EvidenceRecord, VerificationFailureCode, VerificationInput,
    VerificationResourceRef, VerificationRunRecord, VerificationRunStatus,
};
use serde::Serialize;
use storage_core::{
    CommittedVerificationRun, SettleVerificationRunCommit, StartVerificationRunCommit,
    VerificationRunActor, VerificationRunEventContext, VerificationRunStore,
};


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
            Command::Operation {
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
            || (command.next_status == VerificationRunStatus::Passed
                && (command.verified_evidence.is_empty()
                    || command.evidence_refs
                        != command
                            .verified_evidence
                            .iter()
                            .map(|evidence| evidence.evidence_id.clone())
                            .collect::<Vec<_>>()
                    || command.failure_code.is_some()))
            || (command.next_status != VerificationRunStatus::Passed
                && (!command.verified_evidence.is_empty() || !command.evidence_refs.is_empty()))
            || (matches!(
                command.next_status,
                VerificationRunStatus::Failed | VerificationRunStatus::Inconclusive
            ) && command.failure_code.is_none())
        {
            return Err(StoreError::Invalid(
                "VerificationRun settlement is invalid or lacks a typed outcome".into(),
            ));
        }
        for evidence in &mut command.verified_evidence {
            if evidence
                .verification_run_id
                .as_deref()
                .is_some_and(|run_id| run_id != command.verification_run_id)
            {
                return Err(StoreError::Invalid(
                    "Verifier Evidence is already bound to a different run".into(),
                ));
            }
            evidence.verification_run_id = Some(command.verification_run_id.clone());
            evidence.created_at = command.event.recorded_at.clone();
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
            "verified_evidence":command.verified_evidence,
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
        for evidence in &mut command.verified_evidence {
            validate_verifier_evidence(&current, &command.actor.verifier_service_id, evidence)?;
            evidence.created_at = command.event.recorded_at.clone();
            evidence.verification_run_id = Some(current.verification_run_id.clone());
            evidence
                .validate()
                .map_err(|_| StoreError::Invalid("Verifier Evidence record is invalid".into()))?;
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
        let prepared_evidence = command
            .verified_evidence
            .iter()
            .map(|evidence| {
                let state_ref = put_state(
                    self,
                    &command.workspace_id,
                    evidence,
                    "application/vnd.litecow.evidence+json",
                    1,
                )?;
                let context =
                    verification_evidence_event(&command.workspace_id, evidence, &command.event);
                Ok((evidence.clone(), state_ref, context))
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        self.run_verification(move |connection| {
            settle_transaction(
                connection,
                command,
                current,
                settled,
                state_ref,
                prepared_evidence,
            )
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
            authorize_owner_for_read(&tx, &workspace, &owner)?;
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
        after_started_at: Option<&str>,
        after_verification_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VerificationRunRecord>, StoreError> {
        if !(1..=200).contains(&limit)
            || invalid_identifier(owner_principal_id, 256)
            || invalid_identifier(workspace_id, 256)
            || invalid_identifier(task_id, 256)
            || after_started_at.is_some() != after_verification_run_id.is_some()
            || after_started_at.is_some_and(|value| invalid_identifier(value, 64))
            || after_verification_run_id.is_some_and(|value| invalid_identifier(value, 256))
        {
            return Err(StoreError::Invalid(
                "VerificationRun page query is invalid".into(),
            ));
        }
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let task = task_id.to_owned();
        let after = after_started_at
            .zip(after_verification_run_id)
            .map(|(started_at, run_id)| (started_at.to_owned(), run_id.to_owned()));
        self.run_verification(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_task_for_read(&tx, &owner, &workspace, &task)?;
            let mut runs = Vec::new();
            match after {
                Some((started_at, run_id)) => {
                    let mut statement = tx.prepare(
                        "SELECT verification_run_id FROM verification_runs WHERE task_id=?1 AND (started_at<?2 OR (started_at=?2 AND verification_run_id<?3)) ORDER BY started_at DESC, verification_run_id DESC LIMIT ?4",
                    ).map_err(map_database_error)?;
                    let ids = statement
                        .query_map(params![task, started_at, run_id, limit as i64], |row| {
                            row.get::<_, String>(0)
                        })
                        .map_err(map_database_error)?
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .map_err(map_database_error)?;
                    drop(statement);
                    for id in ids {
                        if let Some(run) = load_run(&tx, &workspace, &id)? {
                            runs.push(run);
                        }
                    }
                }
                None => {
                    let mut statement = tx.prepare(
                        "SELECT verification_run_id FROM verification_runs WHERE task_id=?1 ORDER BY started_at DESC, verification_run_id DESC LIMIT ?2",
                    ).map_err(map_database_error)?;
                    let ids = statement
                        .query_map(params![task, limit as i64], |row| row.get::<_, String>(0))
                        .map_err(map_database_error)?
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .map_err(map_database_error)?;
                    drop(statement);
                    for id in ids {
                        if let Some(run) = load_run(&tx, &workspace, &id)? {
                            runs.push(run);
                        }
                    }
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
            Command::Operation {
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
    prepared_evidence: Vec<(EvidenceRecord, AggregateStateRef, EventDraft)>,
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
        "verified_evidence":command.verified_evidence,
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
    if command.next_status == VerificationRunStatus::Passed {
        let mut expected = command.evidence_refs.clone();
        expected.sort();
        let mut attached = tx
            .prepare(
                "SELECT evidence_id FROM evidence WHERE task_id=?1 AND verification_run_id=?2 ORDER BY evidence_id",
            )
            .map_err(map_database_error)?
            .query_map(
                params![current.task_id, current.verification_run_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(map_database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?;
        for (evidence, state_ref, event) in &prepared_evidence {
            if attached.contains(&evidence.evidence_id)
                || !command.evidence_refs.contains(&evidence.evidence_id)
            {
                return Err(StoreError::Invalid(
                    "VerificationRun Evidence identity is duplicated or unlisted".into(),
                ));
            }
            insert_verification_evidence(&tx, evidence, state_ref, event)?;
            attached.push(evidence.evidence_id.clone());
            attached.sort();
        }
        attached.sort();
        if attached != expected {
            return Err(StoreError::Invalid(
                "VerificationRun must list exactly its bound VERIFIED Evidence".into(),
            ));
        }
    }
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

fn validate_verifier_evidence(
    run: &VerificationRunRecord,
    verifier_service_id: &str,
    evidence: &EvidenceRecord,
) -> Result<(), StoreError> {
    evidence
        .validate()
        .map_err(|_| StoreError::Invalid("Verifier Evidence record is invalid".into()))?;
    let producer_matches = evidence.producer.as_object().is_some_and(|producer| {
        producer.len() == 1
            && producer.get("service_id").and_then(Value::as_str) == Some(verifier_service_id)
    });
    let subject_matches = if run.subject_refs.is_empty() {
        evidence.subject_ref == format!("task:{}", run.task_id)
    } else {
        run.subject_refs
            .iter()
            .any(|subject| evidence.subject_ref == verification_resource_uri(subject))
    };
    let payload_ref_matches = evidence.payload_ref.as_ref().is_none_or(|payload_ref| {
        run.subject_refs
            .iter()
            .chain(run.inputs.iter().map(|input| &input.resource_ref))
            .any(|reference| {
                payload_ref.get("workspace_id").and_then(Value::as_str)
                    == Some(reference.workspace_id.as_str())
                    && payload_ref.get("resource_id").and_then(Value::as_str)
                        == Some(reference.resource_id.as_str())
                    && payload_ref.get("revision_id").and_then(Value::as_str)
                        == Some(reference.revision_id.as_str())
            })
    });
    if evidence.level != EvidenceLevel::Verified
        || evidence.task_id != run.task_id
        || !producer_matches
        || !subject_matches
        || !payload_ref_matches
        || evidence
            .verification_run_id
            .as_deref()
            .is_some_and(|run_id| run_id != run.verification_run_id)
    {
        return Err(StoreError::Invalid(
            "Verifier Evidence must match the authenticated verifier, exact Task, subject, and run"
                .into(),
        ));
    }
    Ok(())
}

fn verification_resource_uri(reference: &VerificationResourceRef) -> String {
    format!(
        "resource://{}/{}@{}",
        reference.workspace_id, reference.resource_id, reference.revision_id
    )
}

fn verification_evidence_event(
    workspace_id: &str,
    evidence: &EvidenceRecord,
    context: &VerificationRunEventContext,
) -> EventDraft {
    let digest_suffix =
        digest(format!("{}\0{}", context.event_id, evidence.evidence_id).as_bytes());
    EventDraft {
        event_id: format!("event-verification-evidence-{}", &digest_suffix[7..]),
        workspace_id: workspace_id.to_owned(),
        entity_type: "Evidence".into(),
        entity_id: evidence.evidence_id.clone(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        entity_revision: 1,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: Some(context.event_id.clone()),
        schema_version: 1,
        event_type: "evidence.created.v1".into(),
        payload: json!({
            "evidence_id": evidence.evidence_id,
            "task_id": evidence.task_id,
            "subject_ref": evidence.subject_ref,
            "level": evidence.level.as_str(),
            "kind": evidence.kind,
            "producer": evidence.producer,
            "payload_digest": evidence.payload_digest,
        }),
        recorded_at: context.recorded_at.clone(),
    }
}

fn insert_verification_evidence(
    tx: &Transaction<'_>,
    evidence: &EvidenceRecord,
    state_ref: &AggregateStateRef,
    event: &EventDraft,
) -> Result<(), StoreError> {
    let producer = encode(&evidence.producer)?;
    let payload_ref = evidence.payload_ref.as_ref().map(encode).transpose()?;
    tx.execute(
        "INSERT INTO evidence(evidence_id,task_id,subject_ref,level,kind,producer_json,payload_ref_json,payload_digest,created_at,verification_run_id) VALUES (?1,?2,?3,'VERIFIED',?4,?5,?6,?7,?8,?9)",
        params![
            evidence.evidence_id,
            evidence.task_id,
            evidence.subject_ref,
            evidence.kind,
            producer,
            payload_ref,
            evidence.payload_digest,
            event.recorded_at,
            evidence.verification_run_id,
        ],
    )
    .map_err(map_database_error)?;
    insert_domain_event(tx, event, state_ref)?;
    Ok(())
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
        if cancelled {
            // A cancelled run may close against the immutable revision it
            // originally observed, even after the current head has moved.
            validate_resource_ref(tx, workspace, subject)?;
        } else {
            validate_subject_head(tx, workspace, subject)?;
        }
    }
    Ok(())
}

fn validate_subject_head(
    tx: &Transaction<'_>,
    workspace: &str,
    subject: &VerificationResourceRef,
) -> Result<(), StoreError> {
    if subject.workspace_id != workspace {
        return Err(StoreError::Invalid(
            "cross-Workspace verifier subjects are unsupported".into(),
        ));
    }
    let is_current_head: bool = tx
        .query_row(
            "SELECT EXISTS(
                SELECT 1
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
                  )
            )",
            params![workspace, subject.resource_id, subject.revision_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if is_current_head {
        Ok(())
    } else {
        Err(StoreError::Invalid(
            "VerificationRun subject is stale, conflicted, or unavailable".into(),
        ))
    }
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
    let observed: Option<Option<String>> = tx
        .query_row(
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
            params![
                workspace,
                input.resource_ref.resource_id,
                input.resource_ref.revision_id
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
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
    let dependencies = run
        .inputs
        .iter()
        .map(|input| &input.resource_ref)
        .chain(run.subject_refs.iter());
    for reference in dependencies {
        let edge_id = format!(
            "verification:{}:{}:{}",
            run.verification_run_id, reference.resource_id, reference.revision_id
        );
        tx.execute(
            "INSERT INTO dependency_edges(dependency_edge_id,workspace_id,source_resource_id,source_revision_id,dependent_kind,dependent_ref,artifact_id,artifact_version,verification_run_id,created_at) VALUES (?1,?2,?3,?4,'VERIFICATION_RUN','verification://' || ?5,NULL,NULL,?5,?6) ON CONFLICT(dependency_edge_id) DO NOTHING",
            params![edge_id, workspace, reference.resource_id, reference.revision_id, run.verification_run_id, at],
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

/// Owner check for a mutation: an ARCHIVED Workspace reports `WORKSPACE_ARCHIVED`.
fn authorize_owner(tx: &Transaction<'_>, workspace: &str, owner: &str) -> Result<(), StoreError> {
    require_write_access(tx, workspace, owner)
}

/// Owner check for a read: an ARCHIVED Workspace stays readable.
fn authorize_owner_for_read(
    tx: &Transaction<'_>,
    workspace: &str,
    owner: &str,
) -> Result<(), StoreError> {
    require_read_access(tx, workspace, owner)
}

fn authorize_task(
    tx: &Transaction<'_>,
    owner: &str,
    workspace: &str,
    task: &str,
) -> Result<(), StoreError> {
    authorize_task_with_workspace_policy(tx, owner, workspace, task, true)
}

fn authorize_task_for_read(
    tx: &Transaction<'_>,
    owner: &str,
    workspace: &str,
    task: &str,
) -> Result<(), StoreError> {
    authorize_task_with_workspace_policy(tx, owner, workspace, task, false)
}

/// Apply the Workspace archive policy first, then scope the Task to that Workspace.
///
/// Keeping the two facts separate preserves the contractual error code for an
/// archived Workspace while still refusing a Task that belongs elsewhere.
fn authorize_task_with_workspace_policy(
    tx: &Transaction<'_>,
    owner: &str,
    workspace: &str,
    task: &str,
    require_active: bool,
) -> Result<(), StoreError> {
    if require_active {
        require_write_access(tx, workspace, owner)?;
    } else {
        require_read_access(tx, workspace, owner)?;
    }
    let authorized: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id=?1 AND workspace_id=?2)",
            params![task, workspace],
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
    fn verifier_evidence_must_bind_to_the_exact_task_run_identity_and_subject() {
        let subject = VerificationResourceRef {
            workspace_id: "workspace-1".into(),
            resource_id: "resource-1".into(),
            revision_id: "revision-4".into(),
        };
        let mut run = running_run();
        run.subject_refs = vec![subject];
        let evidence = domain_effects::EvidenceRecord {
            evidence_id: "evidence-1".into(),
            task_id: run.task_id.clone(),
            subject_ref: "resource://workspace-1/resource-1@revision-4".into(),
            level: domain_effects::EvidenceLevel::Verified,
            kind: "criterion-check".into(),
            producer: json!({"service_id":"test-runner"}),
            payload_ref: Some(json!({
                "workspace_id":"workspace-1",
                "resource_id":"resource-1",
                "revision_id":"revision-4"
            })),
            payload_digest: Some(format!("sha256:{}", "b".repeat(64))),
            verification_run_id: None,
            created_at: "2026-10-16T11:00:01Z".into(),
        };

        assert!(validate_verifier_evidence(&run, "test-runner", &evidence).is_ok());

        let mut wrong_task = evidence.clone();
        wrong_task.task_id = "another-task".into();
        assert!(validate_verifier_evidence(&run, "test-runner", &wrong_task).is_err());

        let mut wrong_verifier = evidence.clone();
        wrong_verifier.producer = json!({"service_id":"another-verifier"});
        assert!(validate_verifier_evidence(&run, "test-runner", &wrong_verifier).is_err());

        let mut wrong_subject = evidence.clone();
        wrong_subject.subject_ref = "resource://workspace-1/resource-1@other-revision".into();
        assert!(validate_verifier_evidence(&run, "test-runner", &wrong_subject).is_err());

        let mut wrong_run = evidence;
        wrong_run.verification_run_id = Some("other-run".into());
        assert!(validate_verifier_evidence(&run, "test-runner", &wrong_run).is_err());

        let mut wrong_payload = wrong_task.clone();
        wrong_payload.task_id = run.task_id.clone();
        wrong_payload.payload_ref = Some(json!({
            "workspace_id":"workspace-1",
            "resource_id":"resource-1",
            "revision_id":"stale-revision"
        }));
        assert!(validate_verifier_evidence(&run, "test-runner", &wrong_payload).is_err());
    }

    #[test]
    fn passing_run_atomically_commits_verifier_evidence_and_both_events() {
        let mut connection = verification_settlement_fixture();
        let criterion = json!({"criterion_id":"criterion-1","description":"check"});
        let mut run = running_run();
        run.criterion_digest = digest(&canonical_json(&criterion).unwrap());
        connection
            .execute(
                "UPDATE verification_runs SET criterion_digest=?1 WHERE verification_run_id='verify-1'",
                [run.criterion_digest.as_str()],
            )
            .expect("pin exact criterion digest");
        let evidence = verifier_evidence(&run);
        let settled = run
            .finish(
                VerificationRunStatus::Passed,
                vec![evidence.evidence_id.clone()],
                None,
                "2026-10-16T11:00:01Z".into(),
            )
            .expect("pass run with evidence");
        let context = verification_event_context("completion-event");
        let command = SettleVerificationRunCommit {
            workspace_id: "workspace-1".into(),
            actor: VerificationRunActor {
                owner_principal_id: "owner-1".into(),
                verifier_service_id: "test-runner".into(),
            },
            request_id: "settle-request-1".into(),
            verification_run_id: run.verification_run_id.clone(),
            expected_version: run.version,
            next_status: VerificationRunStatus::Passed,
            evidence_refs: vec![evidence.evidence_id.clone()],
            verified_evidence: vec![evidence],
            failure_code: None,
            event: context,
        };
        let run_state_ref = test_state_ref("verification-run", settled.version);
        let evidence_state_ref = test_state_ref("evidence", 1);
        let evidence = command.verified_evidence[0].clone();
        let evidence_event = verification_evidence_event(
            "workspace-1",
            &evidence,
            &verification_event_context("completion-event"),
        );

        let prepared_evidence = vec![(evidence, evidence_state_ref, evidence_event)];
        let result = settle_transaction(
            &mut connection,
            command.clone(),
            run.clone(),
            settled.clone(),
            run_state_ref.clone(),
            prepared_evidence.clone(),
        )
        .expect("settle verified run");

        assert_eq!(result.run.status, VerificationRunStatus::Passed);
        let stored: (String, String, String) = connection
            .query_row(
                "SELECT level,verification_run_id,producer_json FROM evidence WHERE evidence_id='evidence-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read committed Evidence");
        assert_eq!(stored.0, "VERIFIED");
        assert_eq!(stored.1, "verify-1");
        assert_eq!(
            serde_json::from_str::<Value>(&stored.2).unwrap()["service_id"],
            "test-runner"
        );
        let event_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("count transaction events");
        assert_eq!(event_count, 2);
        let replay = settle_transaction(
            &mut connection,
            command,
            run,
            settled,
            run_state_ref,
            prepared_evidence,
        )
        .expect("replay lost settlement response");
        assert!(replay.replayed);
        let evidence_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM evidence", [], |row| row.get(0))
            .expect("count bound Evidence after replay");
        assert_eq!(evidence_count, 1);
        let replay_event_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("count events after replay");
        assert_eq!(replay_event_count, 2);
    }

    #[test]
    fn cancelled_run_closes_durably_without_verified_evidence() {
        let mut connection = verification_settlement_fixture();
        let criterion = json!({"criterion_id":"criterion-1","description":"check"});
        let mut run = running_run();
        run.criterion_digest = digest(&canonical_json(&criterion).unwrap());
        connection
            .execute(
                "UPDATE verification_runs SET criterion_digest=?1 WHERE verification_run_id='verify-1'",
                [run.criterion_digest.as_str()],
            )
            .expect("pin exact criterion digest");
        connection
            .execute(
                "UPDATE tasks SET status='CANCEL_REQUESTED' WHERE task_id='task-1'",
                [],
            )
            .expect("request Task cancellation");

        let completed_at = "2026-10-16T11:00:01Z";
        let settled = run
            .finish(
                VerificationRunStatus::Inconclusive,
                vec![],
                Some(VerificationFailureCode::TaskCancelled),
                completed_at.into(),
            )
            .expect("close cancelled verifier run with typed outcome");
        let command = SettleVerificationRunCommit {
            workspace_id: "workspace-1".into(),
            actor: VerificationRunActor {
                owner_principal_id: "owner-1".into(),
                verifier_service_id: "test-runner".into(),
            },
            request_id: "settle-cancelled-request".into(),
            verification_run_id: run.verification_run_id.clone(),
            expected_version: run.version,
            next_status: VerificationRunStatus::Inconclusive,
            evidence_refs: vec![],
            verified_evidence: vec![],
            failure_code: Some(VerificationFailureCode::TaskCancelled),
            event: verification_event_context("cancelled-completion-event"),
        };
        let run_state_ref = test_state_ref("verification-run", settled.version);

        let result = settle_transaction(
            &mut connection,
            command.clone(),
            run.clone(),
            settled.clone(),
            run_state_ref.clone(),
            vec![],
        )
        .expect("persist cancelled run closure");

        assert_eq!(result.run.status, VerificationRunStatus::Inconclusive);
        assert_eq!(
            result.run.failure_code,
            Some(VerificationFailureCode::TaskCancelled)
        );
        let persisted: (String, Option<String>, Option<String>, i64) = connection
            .query_row(
                "SELECT status,failure_code,completed_at,version FROM verification_runs WHERE verification_run_id='verify-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read settled cancelled run");
        assert_eq!(
            persisted,
            (
                "INCONCLUSIVE".into(),
                Some("TASK_CANCELLED".into()),
                Some(completed_at.into()),
                3,
            )
        );
        let evidence_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM evidence", [], |row| row.get(0))
            .expect("count verifier Evidence");
        assert_eq!(
            evidence_count, 0,
            "cancellation cannot create VERIFIED Evidence"
        );
        let completion_events: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE event_id='cancelled-completion-event' AND type='verification.completed.v2'",
                [],
                |row| row.get(0),
            )
            .expect("count completion event");
        assert_eq!(completion_events, 1);
        let receipts: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
                params![
                    runtime_principal("runtime-1", "incarnation-1"),
                    "settle-cancelled-request"
                ],
                |row| row.get(0),
            )
            .expect("count cancellation receipt");
        assert_eq!(receipts, 1);

        let replay = settle_transaction(
            &mut connection,
            command,
            run,
            settled,
            run_state_ref,
            vec![],
        )
        .expect("replay lost cancellation response");
        assert!(replay.replayed);
        let event_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("count events after replay");
        assert_eq!(event_count, 1, "exact replay cannot append another event");
    }

    #[test]
    fn completion_evaluator_rejects_unbound_or_stale_verifier_results() {
        use domain_effects::{
            CompletionBlocker, RequiredVerificationCriterion, assess_completion_with_evidence,
        };

        fn assert_criterion_blocked(assessment: domain_effects::CompletionAssessment) {
            assert!(!assessment.eligible);
            assert_eq!(
                assessment.blockers,
                vec![CompletionBlocker::CriterionUnverified(
                    "criterion-1".to_owned()
                )]
            );
        }

        let criterion = RequiredVerificationCriterion {
            criterion_id: "criterion-1".to_owned(),
            task_spec_revision: 7,
            criterion_digest: format!("sha256:{}", "a".repeat(64)),
        };
        let run = VerificationRunRecord {
            verification_run_id: "verification-run-1".to_owned(),
            task_id: "task-1".to_owned(),
            criterion_id: criterion.criterion_id.clone(),
            task_spec_revision: criterion.task_spec_revision,
            criterion_digest: criterion.criterion_digest.clone(),
            verifier_kind: "test-runner".to_owned(),
            verifier_version: "1.2.3".to_owned(),
            subject_refs: vec![VerificationResourceRef {
                workspace_id: "workspace-1".to_owned(),
                resource_id: "resource-1".to_owned(),
                revision_id: "revision-1".to_owned(),
            }],
            inputs: vec![],
            status: VerificationRunStatus::Passed,
            evidence_refs: vec!["evidence-1".to_owned()],
            started_at: Some("2026-10-20T10:00:00Z".to_owned()),
            completed_at: Some("2026-10-20T10:00:01Z".to_owned()),
            version: 3,
            failure_code: None,
        };
        let evidence = EvidenceRecord {
            evidence_id: "evidence-1".to_owned(),
            task_id: "task-1".to_owned(),
            subject_ref: "resource://workspace-1/resource-1@revision-1".to_owned(),
            level: EvidenceLevel::Verified,
            kind: "criterion-check".to_owned(),
            producer: json!({"service_id": "test-runner"}),
            payload_ref: None,
            payload_digest: None,
            verification_run_id: Some("verification-run-1".to_owned()),
            created_at: "2026-10-20T10:00:01Z".to_owned(),
        };

        let accepted = assess_completion_with_evidence(
            "task-1",
            &[criterion.clone()],
            &[run.clone()],
            &[evidence.clone()],
            0,
            0,
            0,
        );
        assert!(
            accepted.eligible,
            "exact PASSED run plus bound VERIFIED Evidence satisfies the criterion"
        );
        assert!(accepted.blockers.is_empty());

        // An agent report or a still-running verifier cannot satisfy a criterion.
        let mut not_passed = run.clone();
        not_passed.status = VerificationRunStatus::Running;
        not_passed.evidence_refs.clear();
        not_passed.completed_at = None;
        not_passed.version = 2;
        assert_criterion_blocked(assess_completion_with_evidence(
            "task-1",
            &[criterion.clone()],
            &[not_passed],
            &[evidence.clone()],
            0,
            0,
            0,
        ));

        // The run must be pinned to the current criterion digest, not a stale revision.
        let mut stale_criterion = criterion.clone();
        stale_criterion.criterion_digest = format!("sha256:{}", "b".repeat(64));
        assert_criterion_blocked(assess_completion_with_evidence(
            "task-1",
            &[stale_criterion.clone()],
            &[run.clone()],
            &[evidence.clone()],
            0,
            0,
            0,
        ));

        // Only VERIFIED Evidence counts; REPORTED or OBSERVED cannot close completion.
        for level in [EvidenceLevel::Reported, EvidenceLevel::Observed] {
            let mut unverified = evidence.clone();
            unverified.level = level;
            assert_criterion_blocked(assess_completion_with_evidence(
                "task-1",
                &[criterion.clone()],
                &[run.clone()],
                &[unverified],
                0,
                0,
                0,
            ));
        }

        // Evidence bound to another run cannot be reused for this run.
        let mut other_run = evidence.clone();
        other_run.verification_run_id = Some("verification-run-other".to_owned());
        assert_criterion_blocked(assess_completion_with_evidence(
            "task-1",
            &[criterion.clone()],
            &[run.clone()],
            &[other_run],
            0,
            0,
            0,
        ));

        // Evidence must name a subject the run actually pinned.
        let mut unpinned_subject = evidence.clone();
        unpinned_subject.subject_ref =
            "resource://workspace-1/resource-1@revision-other".to_owned();
        assert_criterion_blocked(assess_completion_with_evidence(
            "task-1",
            &[criterion.clone()],
            &[run.clone()],
            &[unpinned_subject],
            0,
            0,
            0,
        ));

        // Binding must be exact: extra Evidence attached to the run but not listed
        // in its evidence_refs rejects the run instead of being silently ignored.
        let mut extra_bound = evidence.clone();
        extra_bound.evidence_id = "evidence-2".to_owned();
        assert_criterion_blocked(assess_completion_with_evidence(
            "task-1",
            &[criterion.clone()],
            &[run.clone()],
            &[evidence.clone(), extra_bound],
            0,
            0,
            0,
        ));
    }

    fn verification_settlement_fixture() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory verification DB");
        connection
            .execute_batch(
                "CREATE TABLE workspaces(workspace_id TEXT, owner_principal_id TEXT, status TEXT);
                 CREATE TABLE runtimes(runtime_id TEXT, current_incarnation_id TEXT, availability TEXT);
                 CREATE TABLE runtime_incarnations(runtime_id TEXT, runtime_incarnation_id TEXT, recovery_state TEXT);
                 CREATE TABLE runtime_workspace_bindings(runtime_id TEXT, workspace_id TEXT, status TEXT, revoked_at TEXT, roles_json TEXT);
                 CREATE TABLE tasks(task_id TEXT, workspace_id TEXT, status TEXT, current_spec_revision INTEGER);
                 CREATE TABLE task_spec_revisions(task_id TEXT, workspace_id TEXT, revision INTEGER, acceptance_criteria_json TEXT);
                 CREATE TABLE verification_runs(verification_run_id TEXT PRIMARY KEY,task_id TEXT,criterion_id TEXT,task_spec_revision INTEGER,criterion_digest TEXT,verifier_kind TEXT,verifier_version TEXT,subject_refs_json TEXT,inputs_json TEXT,status TEXT,evidence_refs_json TEXT,started_at TEXT,completed_at TEXT,version INTEGER,failure_code TEXT);
                 CREATE TABLE evidence(evidence_id TEXT PRIMARY KEY,task_id TEXT,subject_ref TEXT,level TEXT,kind TEXT,producer_json TEXT,payload_ref_json TEXT,payload_digest TEXT,created_at TEXT);
                 CREATE TABLE effects(effect_id TEXT PRIMARY KEY,task_id TEXT,state TEXT,verification_ref TEXT);
                 CREATE TABLE workspace_origin_sequences(workspace_id TEXT,origin_runtime_id TEXT,last_sequence INTEGER,PRIMARY KEY(workspace_id,origin_runtime_id));
                 CREATE TABLE domain_events(event_id TEXT PRIMARY KEY,workspace_id TEXT,entity_type TEXT,entity_id TEXT,origin_runtime_id TEXT,origin_sequence INTEGER,entity_revision INTEGER,hlc_timestamp TEXT,correlation_id TEXT,causation_id TEXT,schema_version INTEGER,type TEXT,payload_json TEXT,aggregate_state_ref_json TEXT,recorded_at TEXT,payload_digest TEXT);
                 CREATE TABLE request_dedup(principal_id TEXT,request_id TEXT,request_digest TEXT,response_json TEXT,response_digest TEXT,created_at TEXT,expires_at TEXT,PRIMARY KEY(principal_id,request_id));
                 INSERT INTO workspaces VALUES ('workspace-1','owner-1','ACTIVE');
                 INSERT INTO runtimes VALUES ('runtime-1','incarnation-1','ONLINE');
                 INSERT INTO runtime_incarnations VALUES ('runtime-1','incarnation-1','READY');
                 INSERT INTO runtime_workspace_bindings VALUES ('runtime-1','workspace-1','ACTIVE',NULL,'[\"EXECUTOR\"]');
                 INSERT INTO tasks VALUES ('task-1','workspace-1','VERIFYING',7);
                 INSERT INTO task_spec_revisions VALUES ('task-1','workspace-1',7,'[{\"criterion_id\":\"criterion-1\",\"description\":\"check\"}]');
                 INSERT INTO verification_runs VALUES ('verify-1','task-1','criterion-1',7,'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','test-runner','1.2.3','[]','[]','RUNNING','[]','2026-10-16T11:00:00Z',NULL,2,NULL);",
            )
            .expect("create settlement fixture");
        connection
            .execute_batch(SQLITE_V22_DDL)
            .expect("apply same-run Evidence guards");
        connection
    }

    fn verifier_evidence(run: &VerificationRunRecord) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: "evidence-1".into(),
            task_id: run.task_id.clone(),
            subject_ref: format!("task:{}", run.task_id),
            level: EvidenceLevel::Verified,
            kind: "criterion-check".into(),
            producer: json!({"service_id":"test-runner"}),
            payload_ref: None,
            payload_digest: Some(format!("sha256:{}", "b".repeat(64))),
            verification_run_id: Some(run.verification_run_id.clone()),
            created_at: "2026-10-16T11:00:01Z".into(),
        }
    }

    fn verification_event_context(event_id: &str) -> VerificationRunEventContext {
        VerificationRunEventContext {
            event_id: event_id.into(),
            origin_runtime_id: "runtime-1".into(),
            origin_runtime_incarnation_id: "incarnation-1".into(),
            hlc_timestamp: "2026-10-16T11:00:01Z".into(),
            correlation_id: "correlation-1".into(),
            causation_id: None,
            recorded_at: "2026-10-16T11:00:01Z".into(),
        }
    }

    fn test_state_ref(media_type: &str, version: u64) -> AggregateStateRef {
        AggregateStateRef {
            blob: storage_core::BlobRef {
                digest: format!("sha256:{}", "c".repeat(64)),
                size_bytes: 1,
                media_type: media_type.into(),
            },
            entity_revision: version,
            record_schema_version: 1,
        }
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

    #[test]
    fn passed_verification_rejects_stale_subject_but_cancelled_run_can_close() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch(
                "CREATE TABLE tasks(task_id TEXT, workspace_id TEXT, status TEXT, current_spec_revision INTEGER);
                 CREATE TABLE task_spec_revisions(task_id TEXT, workspace_id TEXT, revision INTEGER, acceptance_criteria_json TEXT);
                 CREATE TABLE resources(workspace_id TEXT, resource_id TEXT, current_revision_id TEXT);
                 CREATE TABLE resource_revisions(resource_revision_id TEXT, resource_id TEXT, content_digest TEXT);
                 CREATE TABLE resource_revision_parents(resource_id TEXT, child_revision_id TEXT, parent_revision_id TEXT);
                 INSERT INTO tasks VALUES ('task-1','workspace-1','VERIFYING',7);
                 INSERT INTO task_spec_revisions VALUES ('task-1','workspace-1',7,'[{\"criterion_id\":\"criterion-1\",\"description\":\"check\"}]');
                 INSERT INTO resources VALUES ('workspace-1','resource-1','revision-old');
                 INSERT INTO resource_revisions VALUES ('revision-old','resource-1',NULL);",
            )
            .expect("create subject freshness fixture");
        let criterion = json!({"criterion_id":"criterion-1","description":"check"});
        let mut run = running_run();
        run.criterion_digest = digest(&canonical_json(&criterion).expect("canonical criterion"));
        run.subject_refs = vec![VerificationResourceRef {
            workspace_id: "workspace-1".into(),
            resource_id: "resource-1".into(),
            revision_id: "revision-old".into(),
        }];

        let tx = connection
            .transaction()
            .expect("subject validation transaction");
        assert!(check_task_criterion_and_inputs(&tx, "workspace-1", &run, false).is_ok());
        tx.execute(
            "INSERT INTO resource_revisions VALUES ('revision-current','resource-1',NULL)",
            [],
        )
        .expect("insert new source head");
        tx.execute(
            "INSERT INTO resource_revision_parents VALUES ('resource-1','revision-current','revision-old')",
            [],
        )
        .expect("link source revision history");
        tx.execute(
            "UPDATE resources SET current_revision_id='revision-current' WHERE resource_id='resource-1'",
            [],
        )
        .expect("move source head");

        assert!(
            check_task_criterion_and_inputs(&tx, "workspace-1", &run, false).is_err(),
            "a PASSED run must not settle against a stale subject revision"
        );
        run.subject_refs[0].revision_id = "revision-current".into();
        tx.execute(
            "INSERT INTO resource_revisions VALUES ('revision-branch','resource-1',NULL)",
            [],
        )
        .expect("insert conflicting source head");
        assert!(
            check_task_criterion_and_inputs(&tx, "workspace-1", &run, false).is_err(),
            "a PASSED run must not settle against a conflicted subject head"
        );
        tx.execute(
            "UPDATE tasks SET status='CANCELLED' WHERE task_id='task-1'",
            [],
        )
        .expect("cancel Task for typed closure");
        assert!(check_task_criterion_and_inputs(&tx, "workspace-1", &run, true).is_ok());
    }

    #[test]
    fn verification_run_dependency_edges_include_subjects_and_inputs() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch(
                "CREATE TABLE dependency_edges(
                    dependency_edge_id TEXT PRIMARY KEY,
                    workspace_id TEXT NOT NULL,
                    source_resource_id TEXT NOT NULL,
                    source_revision_id TEXT NOT NULL,
                    dependent_kind TEXT NOT NULL,
                    dependent_ref TEXT NOT NULL,
                    artifact_id TEXT,
                    artifact_version INTEGER,
                    verification_run_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    UNIQUE(source_resource_id,source_revision_id,dependent_kind,dependent_ref)
                 );",
            )
            .expect("create dependency edge fixture");
        let input_ref = VerificationResourceRef {
            workspace_id: "workspace-1".into(),
            resource_id: "input-resource".into(),
            revision_id: "input-revision".into(),
        };
        let subject_ref = VerificationResourceRef {
            workspace_id: "workspace-1".into(),
            resource_id: "subject-resource".into(),
            revision_id: "subject-revision".into(),
        };
        let mut run = running_run();
        run.inputs = vec![
            VerificationInput {
                resource_ref: input_ref,
                consumed_digest: None,
            },
            VerificationInput {
                resource_ref: subject_ref.clone(),
                consumed_digest: None,
            },
        ];
        run.subject_refs = vec![subject_ref];

        let tx = connection
            .transaction()
            .expect("dependency edge transaction");
        insert_dependency_edges(&tx, "workspace-1", &run, "2026-10-17T00:00:00Z")
            .expect("persist all verification dependencies");
        let edges = tx
            .prepare(
                "SELECT source_resource_id,source_revision_id FROM dependency_edges ORDER BY source_resource_id",
            )
            .expect("prepare dependency edge query")
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .expect("query dependency edges")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect dependency edges");

        assert_eq!(
            edges,
            vec![
                ("input-resource".into(), "input-revision".into()),
                ("subject-resource".into(), "subject-revision".into()),
            ]
        );
    }
}
