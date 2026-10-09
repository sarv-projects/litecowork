//! Durable Effect/Evidence rows, snapshots, events, idempotency and fences.
//!
//! This adapter has no provider transport. A successful PROPOSED commit is a necessary
//! precondition for future dispatch, but is not itself a Trust decision or dispatch.
use super::*;
use domain_effects::{EffectError, EffectRecord, EffectState, EvidenceLevel, EvidenceRecord};
use serde::Serialize;
use storage_core::{
    AppendEvidenceCommit, CommittedEffect, CommittedEvidence, EffectEvidenceEventContext,
    EffectEvidenceStore, EffectFenceBinding, EffectRetryBasis, EffectTransitionMetadata,
    ProposeEffectCommit, TransitionEffectCommit,
};

trait Replayable {
    fn set_replayed(&mut self);
}

impl Replayable for CommittedEffect {
    fn set_replayed(&mut self) {
        self.replayed = true;
    }
}

impl Replayable for CommittedEvidence {
    fn set_replayed(&mut self) {
        self.replayed = true;
    }
}

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

#[derive(Clone)]
pub struct SqliteEffectEvidenceStore {
    store: SqliteWorkspaceStore,
}

impl SqliteEffectEvidenceStore {
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
            Command::EffectEvidenceOperation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }
}

impl EffectEvidenceStore for SqliteEffectEvidenceStore {
    fn propose_effect(
        &self,
        mut command: ProposeEffectCommit,
    ) -> Result<CommittedEffect, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_event_context(
            &command.workspace_id,
            &command.runtime_id,
            &command.runtime_incarnation_id,
            &command.event,
        )?;
        command.effect.created_at = canonicalize_utc_timestamp(&command.effect.created_at)?;
        command.effect.updated_at = canonicalize_utc_timestamp(&command.effect.updated_at)?;
        validate_proposal(&command)?;
        if command.effect.created_at != command.event.recorded_at
            || command.effect.updated_at != command.event.recorded_at
        {
            return Err(StoreError::Invalid(
                "initial Effect timestamps must equal its event transaction time".to_owned(),
            ));
        }
        let workspace_id = command.workspace_id.clone();
        let effect = command.effect.clone();
        let state_ref = put_state(
            &self.store,
            &workspace_id,
            &effect,
            "application/vnd.litecowork.effect+json",
            effect.version,
        )?;
        let principal = runtime_principal(&command.runtime_id, &command.runtime_incarnation_id);
        let fingerprint = digest(&canonical_json(
            &json!({"workspace_id":workspace_id,"effect":effect,"fence":command.fence}),
        )?);
        self.run(move |connection| {
            propose_transaction(connection, command, principal, fingerprint, state_ref)
        })
    }

    fn transition_effect(
        &self,
        mut command: TransitionEffectCommit,
    ) -> Result<CommittedEffect, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_event_context(
            &command.workspace_id,
            &command.runtime_id,
            &command.runtime_incarnation_id,
            &command.event,
        )?;
        if invalid_identifier(&command.request_id, 256)
            || invalid_identifier(&command.effect_id, 256)
            || command.expected_version == 0
        {
            return Err(StoreError::Invalid(
                "Effect transition identity is invalid".to_owned(),
            ));
        }
        let workspace_id = command.workspace_id.clone();
        let principal = runtime_principal(&command.runtime_id, &command.runtime_incarnation_id);
        let fingerprint = digest(&canonical_json(
            &json!({"workspace_id":workspace_id,"effect_id":command.effect_id,"expected_version":command.expected_version,"next_state":command.next_state,"fence":command.fence,"metadata":command.metadata}),
        )?);
        // Resolve an idempotent replay before applying the transition to the current
        // head. Otherwise a retry after a successful transition can fail as an invalid
        // state change before it reaches the transactional replay check.
        let replay_workspace = command.workspace_id.clone();
        let replay_runtime = command.runtime_id.clone();
        let replay_incarnation = command.runtime_incarnation_id.clone();
        let replay_principal = principal.clone();
        let replay_request_id = command.request_id.clone();
        let replay_fingerprint = fingerprint.clone();
        let effect_id = command.effect_id.clone();
        let (prior, current) = self.run(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_runtime(&tx, &replay_workspace, &replay_runtime, &replay_incarnation)?;
            let prior = replay::<CommittedEffect>(
                &tx,
                &replay_principal,
                &replay_request_id,
                &replay_fingerprint,
            )?;
            let current = if prior.is_none() {
                Some(load_effect(&tx, &replay_workspace, &effect_id)?.ok_or(StoreError::NotFound)?)
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
        let updated = apply_transition(
            &current,
            &command.next_state,
            &command.metadata,
            command.event.recorded_at.clone(),
        )?;
        let state_ref = put_state(
            &self.store,
            &workspace_id,
            &updated,
            "application/vnd.litecowork.effect+json",
            updated.version,
        )?;
        self.run(move |connection| {
            transition_transaction(
                connection,
                command,
                principal,
                fingerprint,
                state_ref,
                current,
            )
        })
    }

    fn append_evidence(
        &self,
        mut command: AppendEvidenceCommit,
    ) -> Result<CommittedEvidence, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_event_context(
            &command.workspace_id,
            &command.runtime_id,
            &command.runtime_incarnation_id,
            &command.event,
        )?;
        if invalid_identifier(&command.workspace_id, 256)
            || invalid_identifier(&command.request_id, 256)
        {
            return Err(StoreError::Invalid(
                "Evidence command identity is invalid".to_owned(),
            ));
        }
        command.evidence.created_at = canonicalize_utc_timestamp(&command.evidence.created_at)?;
        command.evidence.validate().map_err(map_effect_error)?;
        if command.evidence.created_at != command.event.recorded_at {
            return Err(StoreError::Invalid(
                "Evidence timestamp must equal its event transaction time".to_owned(),
            ));
        }
        let workspace_id = command.workspace_id.clone();
        let evidence = command.evidence.clone();
        let state_ref = put_state(
            &self.store,
            &workspace_id,
            &evidence,
            "application/vnd.litecowork.evidence+json",
            1,
        )?;
        let principal = runtime_principal(&command.runtime_id, &command.runtime_incarnation_id);
        let fingerprint = digest(&canonical_json(
            &json!({"workspace_id":workspace_id,"evidence":evidence}),
        )?);
        self.run(move |connection| {
            append_evidence_transaction(connection, command, principal, fingerprint, state_ref)
        })
    }

    fn get_effect(
        &self,
        workspace_id: &str,
        effect_id: &str,
    ) -> Result<Option<EffectRecord>, StoreError> {
        let (workspace, id) = (workspace_id.to_owned(), effect_id.to_owned());
        self.run(move |connection| {
            if invalid_identifier(&workspace, 256) || invalid_identifier(&id, 256) {
                return Err(StoreError::Invalid("Effect identity is invalid".to_owned()));
            }
            let tx = connection.transaction().map_err(map_database_error)?;
            let effect = load_effect(&tx, &workspace, &id)?;
            tx.commit().map_err(map_database_error)?;
            Ok(effect)
        })
    }

    fn list_effects(
        &self,
        workspace_id: &str,
        task_id: &str,
        limit: usize,
    ) -> Result<Vec<EffectRecord>, StoreError> {
        if !(1..=200).contains(&limit)
            || invalid_identifier(workspace_id, 256)
            || invalid_identifier(task_id, 256)
        {
            return Err(StoreError::Invalid(
                "Effect page query is invalid".to_owned(),
            ));
        }
        let (workspace, task) = (workspace_id.to_owned(), task_id.to_owned());
        self.run(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_task(&tx, &workspace, &task)?;
            let mut statement = tx.prepare("SELECT e.effect_id FROM effects e JOIN tasks t ON t.task_id=e.task_id WHERE t.workspace_id=?1 AND e.task_id=?2 ORDER BY e.created_at ASC, e.effect_id ASC LIMIT ?3").map_err(map_database_error)?;
            let ids = statement.query_map(params![workspace, task, limit as i64], |row| row.get::<_, String>(0)).map_err(map_database_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(map_database_error)?;
            drop(statement);
            let mut records = Vec::with_capacity(ids.len());
            for id in ids { records.push(load_effect(&tx, &workspace, &id)?.ok_or(StoreError::NotFound)?); }
            tx.commit().map_err(map_database_error)?;
            Ok(records)
        })
    }

    fn list_evidence(
        &self,
        workspace_id: &str,
        task_id: &str,
        limit: usize,
    ) -> Result<Vec<EvidenceRecord>, StoreError> {
        if !(1..=200).contains(&limit)
            || invalid_identifier(workspace_id, 256)
            || invalid_identifier(task_id, 256)
        {
            return Err(StoreError::Invalid(
                "Evidence page query is invalid".to_owned(),
            ));
        }
        let (workspace, task) = (workspace_id.to_owned(), task_id.to_owned());
        self.run(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            authorize_task(&tx, &workspace, &task)?;
            let mut statement = tx.prepare("SELECT e.evidence_id, e.task_id, e.subject_ref, e.level, e.kind, e.producer_json, e.payload_ref_json, e.payload_digest, e.created_at FROM evidence e JOIN tasks t ON t.task_id=e.task_id WHERE t.workspace_id=?1 AND e.task_id=?2 ORDER BY e.created_at ASC, e.evidence_id ASC LIMIT ?3").map_err(map_database_error)?;
            let rows = statement.query_map(params![workspace, task, limit as i64], read_evidence_row).map_err(map_database_error)?;
            let result = rows.map(|row| row.map_err(map_database_error).and_then(parse_evidence_row)).collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            tx.commit().map_err(map_database_error)?;
            Ok(result)
        })
    }
}

fn validate_proposal(command: &ProposeEffectCommit) -> Result<(), StoreError> {
    command.effect.validate().map_err(map_effect_error)?;
    if command.effect.state != EffectState::Proposed
        || command.effect.dispatch_ordinal != 0
        || command.effect.version != 1
        || command.effect.task_id.trim().is_empty()
        || command.effect.attempt_id.trim().is_empty()
    {
        return Err(StoreError::Invalid(
            "Effect proposal must be a new, undispatched Attempt operation".to_owned(),
        ));
    }
    if invalid_identifier(&command.workspace_id, 256)
        || invalid_identifier(&command.request_id, 256)
        || invalid_identifier(&command.fence.lease_id, 256)
        || command.fence.epoch == 0
        || !is_digest(&command.fence.fencing_token_digest)
    {
        return Err(StoreError::Invalid(
            "Effect proposal command or fence binding is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_event_context(
    workspace: &str,
    runtime: &str,
    incarnation: &str,
    event: &EffectEvidenceEventContext,
) -> Result<(), StoreError> {
    if [
        workspace,
        runtime,
        incarnation,
        event.event_id.as_str(),
        event.origin_runtime_id.as_str(),
        event.origin_runtime_incarnation_id.as_str(),
        event.hlc_timestamp.as_str(),
        event.correlation_id.as_str(),
    ]
    .iter()
    .any(|value| {
        value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control)
    }) || event.origin_runtime_id != runtime
        || event.origin_runtime_incarnation_id != incarnation
        || event.causation_id.as_ref().is_some_and(|value| {
            value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control)
        })
    {
        return Err(StoreError::Invalid(
            "Effect/Evidence event context is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn runtime_principal(runtime_id: &str, incarnation_id: &str) -> String {
    format!("runtime:{runtime_id}:{incarnation_id}")
}
fn invalid_identifier(value: &str, max: usize) -> bool {
    value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control)
}

fn authorize_runtime(
    connection: &Connection,
    workspace: &str,
    runtime: &str,
    incarnation: &str,
) -> Result<(), StoreError> {
    let authorized: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces w JOIN runtimes r ON r.runtime_id=?2 JOIN runtime_incarnations i ON i.runtime_id=r.runtime_id AND i.runtime_incarnation_id=?3 AND i.recovery_state IN ('READY','DEGRADED') JOIN runtime_workspace_bindings b ON b.runtime_id=r.runtime_id AND b.workspace_id=w.workspace_id AND b.status='ACTIVE' AND b.revoked_at IS NULL WHERE w.workspace_id=?1 AND w.status='ACTIVE' AND r.current_incarnation_id=i.runtime_incarnation_id AND r.availability IN ('ONLINE','DEGRADED') AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value='EXECUTOR'))",
        params![workspace, runtime, incarnation], |row| row.get(0),
    ).map_err(map_database_error)?;
    if authorized {
        Ok(())
    } else {
        Err(StoreError::Invalid(
            "Runtime incarnation is not authorized to mutate Effects/Evidence in this Workspace"
                .to_owned(),
        ))
    }
}

fn authorize_task(connection: &Connection, workspace: &str, task: &str) -> Result<(), StoreError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE workspace_id=?1 AND task_id=?2)",
            params![workspace, task],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if exists {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

fn validate_live_fence(
    connection: &Connection,
    workspace: &str,
    runtime: &str,
    incarnation: &str,
    task: &str,
    attempt: &str,
    invocation_id: &str,
    fence: &EffectFenceBinding,
) -> Result<(), StoreError> {
    let now = sqlite_now(connection)?;
    let valid: bool = connection.query_row(
        "SELECT EXISTS(
          SELECT 1 FROM tasks t
          JOIN steps s ON s.task_id=t.task_id AND s.current_attempt_id=?5 AND s.status='RUNNING'
          JOIN attempts a ON a.task_id=t.task_id AND a.attempt_id=?5 AND a.step_id=s.step_id
            AND a.status='RUNNING' AND a.execution_lease_id=?6 AND a.runtime_id=?3 AND a.runtime_incarnation_id=?4
          JOIN execution_leases l ON l.lease_id=a.execution_lease_id AND l.task_id=t.task_id AND l.step_id=s.step_id AND l.attempt_id=a.attempt_id
            AND l.runtime_id=a.runtime_id AND l.runtime_incarnation_id=a.runtime_incarnation_id
            AND l.epoch=?7 AND l.fencing_token_digest=?8 AND l.state='ACTIVE' AND julianday(l.expires_at)>julianday(?9)
          JOIN capability_invocations i ON i.invocation_id=?10 AND i.workspace_id=t.workspace_id AND i.scope_kind='ATTEMPT_EXECUTION'
            AND i.task_id=t.task_id AND i.attempt_id=a.attempt_id AND i.status='CREATED' AND i.effect_id IS NULL
            AND i.capability_grant_id IN (SELECT g.capability_grant_id FROM capability_grants g WHERE g.scope_kind='ATTEMPT_EXECUTION'
              AND g.task_id=t.task_id AND g.attempt_id=a.attempt_id AND g.status='ACTIVE' AND (g.expires_at IS NULL OR julianday(g.expires_at)>julianday(?9))
              AND EXISTS(SELECT 1 FROM json_each(g.allowed_operations_json) op WHERE op.value=i.operation))
          WHERE t.workspace_id=?1 AND t.task_id=?2 AND t.status='RUNNING'
        )",
        params![workspace, task, runtime, incarnation, attempt, fence.lease_id, to_sql_i64(fence.epoch, "lease epoch")?, fence.fencing_token_digest, now, invocation_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if valid {
        Ok(())
    } else {
        Err(StoreError::Invalid(
            "Effect dispatch fence, Attempt, Invocation, grant, or Task is not currently eligible"
                .to_owned(),
        ))
    }
}

fn validate_invocation_match(
    connection: &Connection,
    workspace: &str,
    effect: &EffectRecord,
) -> Result<(), StoreError> {
    let raw: Option<(String, String, String, String, String, Option<String>)> = connection.query_row(
        "SELECT i.workspace_id, i.task_id, i.attempt_id, i.request_digest, i.execution_method, i.idempotency_key FROM capability_invocations i WHERE i.invocation_id=?1",
        params![effect.capability_invocation_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    ).optional().map_err(map_database_error)?;
    let Some((actual_workspace, task, attempt, request_digest, method, key)) = raw else {
        return Err(StoreError::NotFound);
    };
    let operation: String = connection
        .query_row(
            "SELECT operation FROM capability_invocations WHERE invocation_id=?1",
            params![effect.capability_invocation_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let capability: String = connection
        .query_row(
            "SELECT capability_ref_json FROM capability_invocations WHERE invocation_id=?1",
            params![effect.capability_invocation_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let expected_capability = effect.capability_ref.as_ref().map(encode).transpose()?;
    if actual_workspace != workspace
        || task != effect.task_id
        || attempt != effect.attempt_id
        || request_digest != effect.request_digest
        || method != effect.execution_method.as_str()
        || operation != effect.operation
        || key != effect.idempotency_key
        || expected_capability
            .as_ref()
            .is_some_and(|expected| capability != *expected)
    {
        return Err(StoreError::Invalid(
            "Effect does not match its exact CapabilityInvocation".to_owned(),
        ));
    }
    Ok(())
}

fn apply_transition(
    current: &EffectRecord,
    next: &EffectState,
    metadata: &EffectTransitionMetadata,
    at: String,
) -> Result<EffectRecord, StoreError> {
    let mut updated = if current.state == EffectState::Reconciling && *next == EffectState::Started
    {
        let retry = metadata.retry_authorization.as_ref().ok_or_else(|| {
            StoreError::Invalid("Effect retry requires reconciler authorization".to_owned())
        })?;
        current.authorize_retry(
            at,
            &retry.evidence_id,
            retry.basis == EffectRetryBasis::ConfirmedNotOccurred,
            retry.basis == EffectRetryBasis::SameKeyIdempotent,
        )
    } else {
        current.transition(*next, at)
    }
    .map_err(map_effect_error)?;
    if metadata.retry_authorization.is_some()
        && !(current.state == EffectState::Reconciling && *next == EffectState::Started)
    {
        return Err(StoreError::Invalid(
            "reconciliation retry proof is only valid for an authorized retry".to_owned(),
        ));
    }
    if metadata
        .result_ref
        .as_ref()
        .is_some_and(|value| !value.is_object())
        || metadata
            .observed_state
            .as_ref()
            .is_some_and(|value| !value.is_object())
        || metadata
            .verification_ref
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        || metadata
            .observation_evidence_ref
            .as_ref()
            .is_some_and(|value| invalid_identifier(value, 256))
        || metadata
            .failure_code
            .as_ref()
            .is_some_and(|value| !valid_error_code(value))
        || metadata
            .failure_digest
            .as_ref()
            .is_some_and(|value| !is_digest(value))
        || metadata
            .ambiguity_reason_digest
            .as_ref()
            .is_some_and(|value| !is_digest(value))
    {
        return Err(StoreError::Invalid(
            "Effect transition metadata is malformed".to_owned(),
        ));
    }
    if (metadata.result_ref.is_some()
        && !matches!(
            next,
            EffectState::Acknowledged | EffectState::Observed | EffectState::Verified
        ))
        || (metadata.observed_state.is_some()
            && !matches!(
                next,
                EffectState::Observed | EffectState::Verified | EffectState::Reconciling
            ))
        || (metadata.verification_ref.is_some() && *next != EffectState::Verified)
        || (metadata.observation_evidence_ref.is_some() && *next != EffectState::Observed)
        || (metadata.failure_code.is_some() && *next != EffectState::Failed)
        || (metadata.failure_retryable.is_some() && *next != EffectState::Failed)
        || (metadata.failure_digest.is_some() && *next != EffectState::Failed)
        || (metadata.ambiguity_reason_digest.is_some() && *next != EffectState::Ambiguous)
    {
        return Err(StoreError::Invalid(
            "Effect transition metadata is not valid for its target state".to_owned(),
        ));
    }
    if *next == EffectState::Observed && metadata.observation_evidence_ref.is_none() {
        return Err(StoreError::Invalid(
            "OBSERVED Effect requires an immutable observation Evidence reference".to_owned(),
        ));
    }
    if *next == EffectState::Failed
        && (metadata.failure_code.is_none() || metadata.failure_retryable.is_none())
    {
        return Err(StoreError::Invalid(
            "FAILED Effect requires a typed failure code and retry classification".to_owned(),
        ));
    }
    if *next == EffectState::Ambiguous && metadata.ambiguity_reason_digest.is_none() {
        return Err(StoreError::Invalid(
            "AMBIGUOUS Effect requires a digest of its reconciliation reason".to_owned(),
        ));
    }
    if metadata.result_ref.is_some() {
        updated.result_ref = metadata.result_ref.clone();
    }
    if metadata.observed_state.is_some() {
        updated.observed_state = metadata.observed_state.clone();
    }
    if *next == EffectState::Observed {
        let mut observation = updated
            .observed_state
            .take()
            .unwrap_or_else(|| serde_json::Map::new().into());
        let object = observation.as_object_mut().ok_or_else(|| {
            StoreError::Invalid("OBSERVED state must be a JSON object".to_owned())
        })?;
        if object.contains_key("_evidence_id") {
            return Err(StoreError::Invalid(
                "observed_state uses the reserved _evidence_id field".to_owned(),
            ));
        }
        let evidence_id = metadata.observation_evidence_ref.clone().ok_or_else(|| {
            StoreError::Invalid("OBSERVED Effect requires observation Evidence".to_owned())
        })?;
        object.insert("_evidence_id".to_owned(), Value::String(evidence_id));
        updated.observed_state = Some(observation);
    }
    if metadata.verification_ref.is_some() {
        updated.verification_ref = metadata.verification_ref.clone();
    }
    if *next == EffectState::Verified && updated.verification_ref.is_none() {
        return Err(StoreError::Invalid(
            "VERIFIED Effect requires a verification Evidence reference".to_owned(),
        ));
    }
    updated.validate().map_err(map_effect_error)?;
    Ok(updated)
}

fn propose_transaction(
    connection: &mut Connection,
    command: ProposeEffectCommit,
    principal: String,
    fingerprint: String,
    state_ref: AggregateStateRef,
) -> Result<CommittedEffect, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_runtime(
        &tx,
        &command.workspace_id,
        &command.runtime_id,
        &command.runtime_incarnation_id,
    )?;
    if let Some(response) =
        replay::<CommittedEffect>(&tx, &principal, &command.request_id, &fingerprint)?
    {
        return Ok(response);
    }
    validate_live_fence(
        &tx,
        &command.workspace_id,
        &command.runtime_id,
        &command.runtime_incarnation_id,
        &command.effect.task_id,
        &command.effect.attempt_id,
        &command.effect.capability_invocation_id,
        &command.fence,
    )?;
    validate_invocation_match(&tx, &command.workspace_id, &command.effect)?;
    validate_effect_resource_refs(&tx, &command.workspace_id, &command.effect)?;
    if load_effect(&tx, &command.workspace_id, &command.effect.effect_id)?.is_some() {
        return Err(StoreError::Conflict {
            expected: None,
            actual: Some(1),
        });
    }
    let target = encode(&command.effect.target)?;
    let capability = command
        .effect
        .capability_ref
        .as_ref()
        .map(encode)
        .transpose()?;
    tx.execute("INSERT INTO effects(effect_id,task_id,attempt_id,capability_ref_json,operation,target_json,idempotency_key,state,request_digest,capability_invocation_id,execution_method,result_ref_json,observed_state_json,verification_ref,dispatch_ordinal,created_at,updated_at,version) VALUES (?1,?2,?3,?4,?5,?6,?7,'PROPOSED',?8,?9,?10,NULL,NULL,NULL,0,?11,?11,1)", params![command.effect.effect_id, command.effect.task_id, command.effect.attempt_id, capability, command.effect.operation, target, command.effect.idempotency_key, command.effect.request_digest, command.effect.capability_invocation_id, command.effect.execution_method.as_str(), command.event.recorded_at]).map_err(map_database_error)?;
    let payload = effect_event_payload(&command.effect, None, EffectState::Proposed, None);
    let event = write_event(
        &tx,
        &command.workspace_id,
        &command.effect.effect_id,
        "Effect",
        1,
        &command.event,
        "effect.proposed.v1",
        payload,
        state_ref,
    )?;
    let response = CommittedEffect {
        effect: command.effect.clone(),
        event,
        replayed: false,
    };
    save_replay(
        &tx,
        &principal,
        &command.request_id,
        &fingerprint,
        &response,
        &command.event.recorded_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(response)
}

fn transition_transaction(
    connection: &mut Connection,
    command: TransitionEffectCommit,
    principal: String,
    fingerprint: String,
    state_ref: AggregateStateRef,
    expected: EffectRecord,
) -> Result<CommittedEffect, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_runtime(
        &tx,
        &command.workspace_id,
        &command.runtime_id,
        &command.runtime_incarnation_id,
    )?;
    if let Some(response) =
        replay::<CommittedEffect>(&tx, &principal, &command.request_id, &fingerprint)?
    {
        return Ok(response);
    }
    let current =
        load_effect(&tx, &command.workspace_id, &command.effect_id)?.ok_or(StoreError::NotFound)?;
    if current.version != command.expected_version || current != expected {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(current.version),
        });
    }
    let updated = apply_transition(
        &current,
        &command.next_state,
        &command.metadata,
        command.event.recorded_at.clone(),
    )?;
    if command.next_state == EffectState::Started {
        let fence = command.fence.as_ref().ok_or_else(|| {
            StoreError::Invalid("starting an Effect requires a current Runtime fence".to_owned())
        })?;
        validate_live_fence(
            &tx,
            &command.workspace_id,
            &command.runtime_id,
            &command.runtime_incarnation_id,
            &current.task_id,
            &current.attempt_id,
            &current.capability_invocation_id,
            fence,
        )?;
    }
    if current.state == EffectState::Reconciling && command.next_state == EffectState::Started {
        let evidence_id = command
            .metadata
            .retry_authorization
            .as_ref()
            .ok_or_else(|| StoreError::Invalid("Effect retry authorization is missing".to_owned()))?
            .evidence_id
            .as_str();
        validate_retry_evidence(&tx, &command.workspace_id, &current, evidence_id)?;
    }
    if command.next_state == EffectState::Observed {
        let evidence_id = command
            .metadata
            .observation_evidence_ref
            .as_deref()
            .ok_or_else(|| {
                StoreError::Invalid("OBSERVED Effect requires observation Evidence".to_owned())
            })?;
        validate_observation_evidence(&tx, &command.workspace_id, &current, evidence_id)?;
    }
    validate_effect_resource_refs(&tx, &command.workspace_id, &updated)?;
    if command.next_state == EffectState::Verified {
        let evidence_id = updated
            .verification_ref
            .as_deref()
            .ok_or_else(|| StoreError::Invalid("verification evidence is missing".to_owned()))?;
        validate_verification_evidence(&tx, &command.workspace_id, &updated, evidence_id)?;
    }
    let result_ref = updated.result_ref.as_ref().map(encode).transpose()?;
    let observed = updated.observed_state.as_ref().map(encode).transpose()?;
    let changed = tx.execute("UPDATE effects SET state=?1,result_ref_json=?2,observed_state_json=?3,verification_ref=?4,dispatch_ordinal=?5,updated_at=?6,version=?7 WHERE effect_id=?8 AND version=?9", params![updated.state.as_str(), result_ref, observed, updated.verification_ref, i64::from(updated.dispatch_ordinal), updated.updated_at, to_sql_i64(updated.version, "Effect version")?, updated.effect_id, to_sql_i64(command.expected_version, "Effect version")?]).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(current.version),
        });
    }
    let event_type = transition_event(&current.state, &updated.state)?;
    let payload = effect_event_payload(
        &updated,
        Some(current.state),
        updated.state,
        Some(&command.metadata),
    );
    let event = write_event(
        &tx,
        &command.workspace_id,
        &updated.effect_id,
        "Effect",
        updated.version,
        &command.event,
        event_type,
        payload,
        state_ref,
    )?;
    let response = CommittedEffect {
        effect: updated,
        event,
        replayed: false,
    };
    save_replay(
        &tx,
        &principal,
        &command.request_id,
        &fingerprint,
        &response,
        &command.event.recorded_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(response)
}

fn append_evidence_transaction(
    connection: &mut Connection,
    command: AppendEvidenceCommit,
    principal: String,
    fingerprint: String,
    state_ref: AggregateStateRef,
) -> Result<CommittedEvidence, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_runtime(
        &tx,
        &command.workspace_id,
        &command.runtime_id,
        &command.runtime_incarnation_id,
    )?;
    if let Some(response) =
        replay::<CommittedEvidence>(&tx, &principal, &command.request_id, &fingerprint)?
    {
        return Ok(response);
    }
    authorize_task(&tx, &command.workspace_id, &command.evidence.task_id)?;
    validate_evidence_producer(
        &command.runtime_id,
        &command.runtime_incarnation_id,
        &command.evidence,
    )?;
    validate_evidence_refs(&tx, &command.workspace_id, &command.evidence)?;
    let producer = encode(&command.evidence.producer)?;
    let payload_ref = command
        .evidence
        .payload_ref
        .as_ref()
        .map(encode)
        .transpose()?;
    tx.execute("INSERT INTO evidence(evidence_id,task_id,subject_ref,level,kind,producer_json,payload_ref_json,payload_digest,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![command.evidence.evidence_id, command.evidence.task_id, command.evidence.subject_ref, command.evidence.level.as_str(), command.evidence.kind, producer, payload_ref, command.evidence.payload_digest, command.event.recorded_at]).map_err(map_database_error)?;
    let payload = json!({"evidence_id":command.evidence.evidence_id,"task_id":command.evidence.task_id,"subject_ref":command.evidence.subject_ref,"level":command.evidence.level.as_str(),"kind":command.evidence.kind,"producer":command.evidence.producer,"payload_digest":command.evidence.payload_digest});
    let event = write_event(
        &tx,
        &command.workspace_id,
        &command.evidence.evidence_id,
        "Evidence",
        1,
        &command.event,
        "evidence.created.v1",
        payload,
        state_ref,
    )?;
    let response = CommittedEvidence {
        evidence: command.evidence.clone(),
        event,
        replayed: false,
    };
    save_replay(
        &tx,
        &principal,
        &command.request_id,
        &fingerprint,
        &response,
        &command.event.recorded_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(response)
}

fn validate_evidence_refs(
    connection: &Connection,
    workspace: &str,
    evidence: &EvidenceRecord,
) -> Result<(), StoreError> {
    if let Some(reference) = &evidence.payload_ref {
        let object = reference.as_object().ok_or_else(|| {
            StoreError::Invalid("Evidence payload ResourceRef must be an object".to_owned())
        })?;
        if object.len() != 3
            || !["workspace_id", "resource_id", "revision_id"]
                .iter()
                .all(|key| object.contains_key(*key))
        {
            return Err(StoreError::Invalid("Evidence payload ResourceRef must contain exactly workspace_id, resource_id, and revision_id".to_owned()));
        }
        if reference.get("workspace_id").and_then(Value::as_str) != Some(workspace) {
            return Err(StoreError::Invalid(
                "Evidence payload Resource must belong to its Task Workspace".to_owned(),
            ));
        }
        let resource_id = reference
            .get("resource_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                StoreError::Invalid("Evidence payload ResourceRef is malformed".to_owned())
            })?;
        let revision_id = reference
            .get("revision_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                StoreError::Invalid("Evidence payload ResourceRef must pin a revision".to_owned())
            })?;
        if invalid_identifier(resource_id, 256) || invalid_identifier(revision_id, 256) {
            return Err(StoreError::Invalid(
                "Evidence payload ResourceRef identity is invalid".to_owned(),
            ));
        }
        let exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM resource_revisions rr JOIN resources r ON r.resource_id=rr.resource_id WHERE r.workspace_id=?1 AND r.resource_id=?2 AND rr.resource_revision_id=?3)", params![workspace, resource_id, revision_id], |row| row.get(0)).map_err(map_database_error)?;
        if !exists {
            return Err(StoreError::NotFound);
        }
    }
    Ok(())
}

fn validate_effect_resource_refs(
    connection: &Connection,
    workspace: &str,
    effect: &EffectRecord,
) -> Result<(), StoreError> {
    if effect.target.is_object() {
        validate_resource_ref_owner(connection, workspace, &effect.target)?;
    }
    if let Some(result_ref) = &effect.result_ref {
        validate_resource_ref_owner(connection, workspace, result_ref)?;
    }
    Ok(())
}

fn validate_resource_ref_owner(
    connection: &Connection,
    workspace: &str,
    reference: &Value,
) -> Result<(), StoreError> {
    let object = reference
        .as_object()
        .ok_or_else(|| StoreError::Invalid("ResourceRef must be an object".to_owned()))?;
    if object
        .keys()
        .any(|key| !["workspace_id", "resource_id", "revision_id"].contains(&key.as_str()))
        || !object.contains_key("workspace_id")
        || !object.contains_key("resource_id")
    {
        return Err(StoreError::Invalid(
            "ResourceRef fields are invalid".to_owned(),
        ));
    }
    if reference.get("workspace_id").and_then(Value::as_str) != Some(workspace) {
        return Err(StoreError::Invalid(
            "ResourceRef must belong to the Effect Workspace".to_owned(),
        ));
    }
    let resource_id = reference
        .get("resource_id")
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::Invalid("ResourceRef resource_id is malformed".to_owned()))?;
    let revision_id = reference.get("revision_id").and_then(Value::as_str);
    if invalid_identifier(resource_id, 256) {
        return Err(StoreError::Invalid(
            "ResourceRef identity/revision is invalid".to_owned(),
        ));
    }
    let exists: bool = if let Some(revision) = revision_id {
        if invalid_identifier(revision, 256) {
            return Err(StoreError::Invalid(
                "ResourceRef revision_id is malformed".to_owned(),
            ));
        }
        connection.query_row("SELECT EXISTS(SELECT 1 FROM resource_revisions rr JOIN resources r ON r.resource_id=rr.resource_id WHERE r.workspace_id=?1 AND r.resource_id=?2 AND rr.resource_revision_id=?3)", params![workspace, resource_id, revision], |row| row.get(0)).map_err(map_database_error)?
    } else {
        connection.query_row("SELECT EXISTS(SELECT 1 FROM resources r WHERE r.workspace_id=?1 AND r.resource_id=?2)", params![workspace, resource_id], |row| row.get(0)).map_err(map_database_error)?
    };
    if exists {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

/// This Runtime-authenticated append path can record what a Runtime reports, but it is
/// not an independent observer/verifier authority. Higher assurance must use a separately
/// authenticated provider contract; in particular Runtime can never name itself as a
/// verifier or submit arbitrary VERIFIED evidence here.
fn validate_evidence_producer(
    runtime: &str,
    incarnation: &str,
    evidence: &EvidenceRecord,
) -> Result<(), StoreError> {
    if evidence.level != EvidenceLevel::Reported {
        return Err(StoreError::Invalid("Runtime Evidence admission only supports REPORTED; OBSERVED/VERIFIED require an independently authenticated producer".to_owned()));
    }
    let expected = json!({"principal_id":runtime_principal(runtime, incarnation),"kind":"RUNTIME"});
    if evidence.producer != expected {
        return Err(StoreError::Invalid(
            "Evidence producer must match the authenticated Runtime principal".to_owned(),
        ));
    }
    Ok(())
}

fn validate_retry_evidence(
    connection: &Connection,
    workspace: &str,
    effect: &EffectRecord,
    evidence_id: &str,
) -> Result<(), StoreError> {
    let valid: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM evidence e JOIN tasks t ON t.task_id=e.task_id WHERE t.workspace_id=?1 AND e.task_id=?2 AND e.evidence_id=?3 AND e.level IN ('OBSERVED','VERIFIED') AND e.subject_ref=?4)", params![workspace, effect.task_id, evidence_id, format!("effect:{}", effect.effect_id)], |row| row.get(0)).map_err(map_database_error)?;
    if valid {
        Ok(())
    } else {
        Err(StoreError::Invalid(
            "retry evidence must be observed/verified evidence about this exact Effect".to_owned(),
        ))
    }
}

fn validate_observation_evidence(
    connection: &Connection,
    workspace: &str,
    effect: &EffectRecord,
    evidence_id: &str,
) -> Result<(), StoreError> {
    let valid: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM evidence e JOIN tasks t ON t.task_id=e.task_id WHERE t.workspace_id=?1 AND e.task_id=?2 AND e.evidence_id=?3 AND e.level IN ('OBSERVED','VERIFIED') AND e.subject_ref=?4)", params![workspace, effect.task_id, evidence_id, format!("effect:{}", effect.effect_id)], |row| row.get(0)).map_err(map_database_error)?;
    if valid {
        Ok(())
    } else {
        Err(StoreError::Invalid(
            "OBSERVED Effect requires same-Task OBSERVED/VERIFIED Evidence about this exact Effect"
                .to_owned(),
        ))
    }
}

fn validate_verification_evidence(
    connection: &Connection,
    workspace: &str,
    effect: &EffectRecord,
    evidence_id: &str,
) -> Result<(), StoreError> {
    let valid: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM evidence e JOIN tasks t ON t.task_id=e.task_id WHERE t.workspace_id=?1 AND e.task_id=?2 AND e.evidence_id=?3 AND e.level='VERIFIED' AND e.subject_ref=?4 AND json_extract(e.producer_json,'$.service_id') IS NOT NULL AND EXISTS(SELECT 1 FROM verification_runs v WHERE v.task_id=e.task_id AND v.status='PASSED' AND v.verifier_kind=json_extract(e.producer_json,'$.service_id') AND EXISTS(SELECT 1 FROM json_each(v.evidence_refs_json) ref WHERE ref.value=e.evidence_id)))", params![workspace, effect.task_id, evidence_id, format!("effect:{}", effect.effect_id)], |row| row.get(0)).map_err(map_database_error)?;
    if valid {
        Ok(())
    } else {
        Err(StoreError::Invalid("Effect verification requires exact subject-bound VERIFIED Evidence referenced by a passing VerificationRun".to_owned()))
    }
}

fn effect_event_payload(
    effect: &EffectRecord,
    from: Option<EffectState>,
    to: EffectState,
    metadata: Option<&EffectTransitionMetadata>,
) -> Value {
    let target_digest = canonical_json(&effect.target)
        .map(|bytes| digest(&bytes))
        .unwrap_or_else(|_| "sha256:invalid".to_owned());
    let mut payload = json!({"effect_id":effect.effect_id,"task_id":effect.task_id,"attempt_id":effect.attempt_id,"to":to.as_str(),"operation":effect.operation,"target_digest":target_digest,"dispatch_ordinal":effect.dispatch_ordinal});
    if let Some(from) = from {
        payload["from"] = Value::String(from.as_str().to_owned());
    }
    if to == EffectState::Proposed {
        payload["capability_invocation_id"] =
            Value::String(effect.capability_invocation_id.clone());
        payload["execution_method"] = Value::String(effect.execution_method.as_str().to_owned());
    }
    if let Some(metadata) = metadata {
        if let Some(evidence_id) = &metadata.observation_evidence_ref {
            payload["observation_evidence_id"] = Value::String(evidence_id.clone());
        }
        if let Some(code) = &metadata.failure_code {
            payload["failure_code"] = Value::String(code.clone());
        }
        if let Some(retryable) = metadata.failure_retryable {
            payload["failure_retryable"] = Value::Bool(retryable);
        }
        if let Some(digest) = &metadata.failure_digest {
            payload["failure_digest"] = Value::String(digest.clone());
        }
        if let Some(digest) = &metadata.ambiguity_reason_digest {
            payload["ambiguity_reason_digest"] = Value::String(digest.clone());
        }
        if let Some(retry) = &metadata.retry_authorization {
            payload["retry_evidence_id"] = Value::String(retry.evidence_id.clone());
            payload["retry_basis"] = Value::String(
                match retry.basis {
                    EffectRetryBasis::ConfirmedNotOccurred => "CONFIRMED_NOT_OCCURRED",
                    EffectRetryBasis::SameKeyIdempotent => "SAME_KEY_IDEMPOTENT",
                }
                .to_owned(),
            );
        }
    }
    payload
}

fn transition_event(from: &EffectState, to: &EffectState) -> Result<&'static str, StoreError> {
    Ok(match (from, to) {
        (EffectState::Proposed, EffectState::Started) => "effect.started.v1",
        (EffectState::Reconciling, EffectState::Started) => "effect.retry.authorized.v1",
        (EffectState::Proposed, EffectState::Failed)
        | (EffectState::Started, EffectState::Failed)
        | (EffectState::Acknowledged, EffectState::Failed)
        | (EffectState::Reconciling, EffectState::Failed) => "effect.failed.v1",
        (EffectState::Started, EffectState::Acknowledged) => "effect.acknowledged.v1",
        (EffectState::Acknowledged, EffectState::Observed)
        | (EffectState::Reconciling, EffectState::Observed) => "effect.observed.v1",
        (EffectState::Observed, EffectState::Verified) => "effect.verified.v1",
        (EffectState::Started, EffectState::Ambiguous)
        | (EffectState::Acknowledged, EffectState::Ambiguous)
        | (EffectState::Observed, EffectState::Ambiguous)
        | (EffectState::Reconciling, EffectState::Ambiguous) => "effect.ambiguous.v1",
        (EffectState::Ambiguous, EffectState::Reconciling) => "effect.reconciliation.started.v1",
        _ => {
            return Err(StoreError::Invalid(
                "Effect transition has no canonical event type".to_owned(),
            ));
        }
    })
}

fn write_event(
    tx: &Transaction<'_>,
    workspace: &str,
    entity: &str,
    entity_type: &str,
    revision: u64,
    context: &EffectEvidenceEventContext,
    event_type: &str,
    payload: Value,
    state_ref: AggregateStateRef,
) -> Result<storage_core::DomainEvent, StoreError> {
    tx.execute("INSERT INTO workspace_origin_sequences(workspace_id,origin_runtime_id,last_sequence) VALUES (?1,?2,1) ON CONFLICT(workspace_id,origin_runtime_id) DO UPDATE SET last_sequence=last_sequence+1", params![workspace, context.origin_runtime_id]).map_err(map_database_error)?;
    let sequence: i64 = tx.query_row("SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id=?1 AND origin_runtime_id=?2", params![workspace, context.origin_runtime_id], |row| row.get(0)).map_err(map_database_error)?;
    let payload_json = encode(&payload)?;
    let state_json = encode(&state_ref)?;
    let payload_digest = digest(payload_json.as_bytes());
    tx.execute("INSERT INTO domain_events(event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,causation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,?11,?12,?13,?14,?15)", params![context.event_id, workspace, entity_type, entity, context.origin_runtime_id, sequence, to_sql_i64(revision, "domain event revision")?, context.hlc_timestamp, context.correlation_id, context.causation_id, event_type, payload_json, state_json, context.recorded_at, payload_digest]).map_err(map_database_error)?;
    Ok(storage_core::DomainEvent {
        event_id: context.event_id.clone(),
        workspace_id: workspace.to_owned(),
        entity_type: entity_type.to_owned(),
        entity_id: entity.to_owned(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        origin_sequence: from_sql_i64(sequence, "origin sequence")?,
        entity_revision: revision,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        schema_version: 1,
        event_type: event_type.to_owned(),
        payload,
        aggregate_state_ref: state_ref,
        recorded_at: context.recorded_at.clone(),
        payload_digest,
    })
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
            "Effect/Evidence aggregate-state blob failed verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: revision,
        record_schema_version: 1,
    })
}

fn replay<T: serde::de::DeserializeOwned + Replayable>(
    tx: &Transaction<'_>,
    principal: &str,
    request_id: &str,
    fingerprint: &str,
) -> Result<Option<T>, StoreError> {
    let prior: Option<(String, Option<String>)> = tx.query_row("SELECT request_digest,response_json FROM request_dedup WHERE principal_id=?1 AND request_id=?2", params![principal, request_id], |row| Ok((row.get(0)?, row.get(1)?))).optional().map_err(map_database_error)?;
    match prior {
        None => Ok(None),
        Some((digest_value, response)) if digest_value == fingerprint => response
            .map(|value| {
                let mut result: T = serde_json::from_str(&value).map_err(|error| {
                    StoreError::Integrity(format!(
                        "Effect/Evidence idempotency receipt is corrupt: {error}"
                    ))
                })?;
                result.set_replayed();
                Ok(result)
            })
            .transpose(),
        Some(_) => Err(StoreError::Invalid(
            "request ID was reused with different Effect/Evidence content".to_owned(),
        )),
    }
}

fn save_replay<T: Serialize>(
    tx: &Transaction<'_>,
    principal: &str,
    request: &str,
    fingerprint: &str,
    response: &T,
    at: &str,
) -> Result<(), StoreError> {
    let response = encode(response)?;
    tx.execute("INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,NULL)", params![principal, request, fingerprint, response, digest(response.as_bytes()), at]).map_err(map_database_error)?;
    Ok(())
}

fn load_effect(
    connection: &Connection,
    workspace: &str,
    id: &str,
) -> Result<Option<EffectRecord>, StoreError> {
    let row: Option<RawEffect> = connection.query_row(
        "SELECT e.effect_id,e.task_id,e.attempt_id,e.capability_ref_json,e.operation,e.target_json,e.idempotency_key,e.state,e.request_digest,e.capability_invocation_id,e.execution_method,e.result_ref_json,e.observed_state_json,e.verification_ref,e.dispatch_ordinal,e.created_at,e.updated_at,e.version FROM effects e JOIN tasks t ON t.task_id=e.task_id WHERE t.workspace_id=?1 AND e.effect_id=?2",
        params![workspace,id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?,row.get(12)?,row.get(13)?,row.get(14)?,row.get(15)?,row.get(16)?,row.get(17)?)),
    ).optional().map_err(map_database_error)?;
    row.map(parse_effect_row).transpose()
}

type RawEffect = (
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
    String,
    String,
    i64,
);
fn parse_effect_row(row: RawEffect) -> Result<EffectRecord, StoreError> {
    let (
        effect_id,
        task_id,
        attempt_id,
        capability_ref,
        operation,
        target,
        idempotency_key,
        state,
        request_digest,
        capability_invocation_id,
        execution_method,
        result_ref,
        observed_state,
        verification_ref,
        dispatch_ordinal,
        created_at,
        updated_at,
        version,
    ) = row;
    let effect = EffectRecord {
        effect_id,
        task_id,
        attempt_id,
        capability_ref: decode_optional(capability_ref)?,
        operation,
        target: decode(&target)?,
        idempotency_key,
        state: parse_effect_state(&state)?,
        request_digest,
        capability_invocation_id,
        execution_method: parse_execution_method(&execution_method)?,
        dispatch_ordinal: u32::try_from(dispatch_ordinal)
            .map_err(|_| StoreError::Integrity("negative Effect dispatch ordinal".to_owned()))?,
        result_ref: decode_optional(result_ref)?,
        observed_state: decode_optional(observed_state)?,
        verification_ref,
        created_at,
        updated_at,
        version: from_sql_i64(version, "Effect version")?,
    };
    effect.validate().map_err(map_effect_error)?;
    Ok(effect)
}
fn parse_effect_state(value: &str) -> Result<EffectState, StoreError> {
    match value {
        "PROPOSED" => Ok(EffectState::Proposed),
        "STARTED" => Ok(EffectState::Started),
        "ACKNOWLEDGED" => Ok(EffectState::Acknowledged),
        "RECONCILING" => Ok(EffectState::Reconciling),
        "OBSERVED" => Ok(EffectState::Observed),
        "VERIFIED" => Ok(EffectState::Verified),
        "FAILED" => Ok(EffectState::Failed),
        "AMBIGUOUS" => Ok(EffectState::Ambiguous),
        _ => Err(StoreError::Integrity("unknown Effect state".to_owned())),
    }
}
fn parse_execution_method(value: &str) -> Result<domain_effects::ExecutionMethod, StoreError> {
    match value {
        "STRUCTURED_API" => Ok(domain_effects::ExecutionMethod::StructuredApi),
        "STRUCTURED_BROWSER" => Ok(domain_effects::ExecutionMethod::StructuredBrowser),
        "ACCESSIBILITY_BROWSER" => Ok(domain_effects::ExecutionMethod::AccessibilityBrowser),
        "SCREEN_COMPUTER_USE" => Ok(domain_effects::ExecutionMethod::ScreenComputerUse),
        "DETERMINISTIC_LOCAL" => Ok(domain_effects::ExecutionMethod::DeterministicLocal),
        "NATIVE_AGENT_TOOL" => Ok(domain_effects::ExecutionMethod::NativeAgentTool),
        "UNKNOWN" => Ok(domain_effects::ExecutionMethod::Unknown),
        _ => Err(StoreError::Integrity(
            "unknown Effect execution method".to_owned(),
        )),
    }
}
fn parse_evidence_level(value: &str) -> Result<EvidenceLevel, StoreError> {
    match value {
        "REPORTED" => Ok(EvidenceLevel::Reported),
        "OBSERVED" => Ok(EvidenceLevel::Observed),
        "VERIFIED" => Ok(EvidenceLevel::Verified),
        _ => Err(StoreError::Integrity("unknown Evidence level".to_owned())),
    }
}
fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, StoreError> {
    serde_json::from_str(value).map_err(|error| {
        StoreError::Integrity(format!("stored Effect/Evidence JSON is invalid: {error}"))
    })
}
fn decode_optional<T: serde::de::DeserializeOwned>(
    value: Option<String>,
) -> Result<Option<T>, StoreError> {
    value.map(|value| decode(&value)).transpose()
}
fn encode<T: Serialize>(value: &T) -> Result<String, StoreError> {
    String::from_utf8(canonical_json(value)?)
        .map_err(|_| StoreError::Integrity("canonical JSON was not UTF-8".to_owned()))
}
fn map_effect_error(error: EffectError) -> StoreError {
    StoreError::Invalid(format!("invalid Effect/Evidence domain value: {error}"))
}
fn is_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_uppercase()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn sqlite_now(connection: &Connection) -> Result<String, StoreError> {
    let value: String = connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |row| {
            row.get(0)
        })
        .map_err(map_database_error)?;
    canonicalize_utc_timestamp(&value)
}

fn read_evidence_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
    ))
}
fn parse_evidence_row(
    row: (
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    ),
) -> Result<EvidenceRecord, StoreError> {
    let (
        evidence_id,
        task_id,
        subject_ref,
        level,
        kind,
        producer,
        payload_ref,
        payload_digest,
        created_at,
    ) = row;
    let evidence = EvidenceRecord {
        evidence_id,
        task_id,
        subject_ref,
        level: parse_evidence_level(&level)?,
        kind,
        producer: decode(&producer)?,
        payload_ref: decode_optional(payload_ref)?,
        payload_digest,
        created_at,
    };
    evidence.validate().map_err(map_effect_error)?;
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn effect() -> EffectRecord {
        EffectRecord {
            effect_id: "effect-1".into(),
            task_id: "task-1".into(),
            attempt_id: "attempt-1".into(),
            capability_ref: None,
            operation: "file.write".into(),
            target: json!("workspace-file"),
            idempotency_key: Some("key-1".into()),
            state: EffectState::Proposed,
            request_digest: format!("sha256:{}", "a".repeat(64)),
            capability_invocation_id: "inv-1".into(),
            execution_method: domain_effects::ExecutionMethod::DeterministicLocal,
            dispatch_ordinal: 0,
            result_ref: None,
            observed_state: None,
            verification_ref: None,
            created_at: "2026-10-08T00:00:00Z".into(),
            updated_at: "2026-10-08T00:00:00Z".into(),
            version: 1,
        }
    }

    fn metadata() -> EffectTransitionMetadata {
        EffectTransitionMetadata {
            result_ref: None,
            observed_state: None,
            observation_evidence_ref: None,
            verification_ref: None,
            retry_authorization: None,
            failure_code: None,
            failure_retryable: None,
            failure_digest: None,
            ambiguity_reason_digest: None,
        }
    }

    #[test]
    fn proposed_event_payload_matches_zero_dispatch_ordinal_schema() {
        let payload = effect_event_payload(&effect(), None, EffectState::Proposed, None);
        assert_eq!(payload["dispatch_ordinal"], 0);
        assert_eq!(payload["capability_invocation_id"], "inv-1");
    }

    #[test]
    fn observed_transition_requires_evidence_reference() {
        let started = effect()
            .transition(EffectState::Started, "2026-10-08T00:00:01Z".into())
            .unwrap();
        let acknowledged = started
            .transition(EffectState::Acknowledged, "2026-10-08T00:00:02Z".into())
            .unwrap();
        let error = apply_transition(
            &acknowledged,
            &EffectState::Observed,
            &metadata(),
            "2026-10-08T00:00:03Z".into(),
        )
        .unwrap_err();
        assert!(matches!(error, StoreError::Invalid(_)));
    }

    #[test]
    fn failure_and_ambiguity_events_require_safe_provenance() {
        let mut failed_metadata = metadata();
        failed_metadata.failure_code = Some("AGENT_UNAVAILABLE".into());
        failed_metadata.failure_retryable = Some(true);
        let failed = apply_transition(
            &effect(),
            &EffectState::Failed,
            &failed_metadata,
            "2026-10-08T00:00:01Z".into(),
        )
        .unwrap();
        let payload = effect_event_payload(
            &failed,
            Some(EffectState::Proposed),
            EffectState::Failed,
            Some(&failed_metadata),
        );
        assert_eq!(payload["failure_code"], "AGENT_UNAVAILABLE");
        assert_eq!(payload["failure_retryable"], true);

        let started = effect()
            .transition(EffectState::Started, "2026-10-08T00:00:01Z".into())
            .unwrap();
        let mut ambiguous_metadata = metadata();
        ambiguous_metadata.ambiguity_reason_digest = Some(format!("sha256:{}", "d".repeat(64)));
        let ambiguous = apply_transition(
            &started,
            &EffectState::Ambiguous,
            &ambiguous_metadata,
            "2026-10-08T00:00:02Z".into(),
        )
        .unwrap();
        let payload = effect_event_payload(
            &ambiguous,
            Some(EffectState::Started),
            EffectState::Ambiguous,
            Some(&ambiguous_metadata),
        );
        assert_eq!(
            payload["ambiguity_reason_digest"],
            format!("sha256:{}", "d".repeat(64))
        );
    }

    #[test]
    fn runtime_cannot_submit_observed_or_verified_evidence_or_claim_another_producer() {
        let mut evidence = EvidenceRecord {
            evidence_id: "evidence-1".into(),
            task_id: "task-1".into(),
            subject_ref: "effect:effect-1".into(),
            level: EvidenceLevel::Reported,
            kind: "provider_result".into(),
            producer: json!({"principal_id":"runtime:runtime-1:inc-1","kind":"RUNTIME"}),
            payload_ref: None,
            payload_digest: None,
            created_at: "2026-10-08T00:00:00Z".into(),
        };
        assert!(validate_evidence_producer("runtime-1", "inc-1", &evidence).is_ok());
        evidence.level = EvidenceLevel::Verified;
        assert!(validate_evidence_producer("runtime-1", "inc-1", &evidence).is_err());
        evidence.level = EvidenceLevel::Reported;
        evidence.producer = json!({"service_id":"forged-verifier"});
        assert!(validate_evidence_producer("runtime-1", "inc-1", &evidence).is_err());
    }

    #[test]
    fn replay_response_is_marked_as_replayed() {
        let mut response = CommittedEvidence {
            evidence: EvidenceRecord {
                evidence_id: "evidence-1".into(),
                task_id: "task-1".into(),
                subject_ref: "task:task-1".into(),
                level: EvidenceLevel::Reported,
                kind: "report".into(),
                producer: json!({"service_id":"test"}),
                payload_ref: None,
                payload_digest: None,
                created_at: "2026-10-08T00:00:00Z".into(),
            },
            event: storage_core::DomainEvent {
                event_id: "event-1".into(),
                workspace_id: "workspace-1".into(),
                entity_type: "Evidence".into(),
                entity_id: "evidence-1".into(),
                origin_runtime_id: "runtime-1".into(),
                origin_sequence: 1,
                entity_revision: 1,
                hlc_timestamp: "2026-10-08T00:00:00Z".into(),
                correlation_id: "corr-1".into(),
                causation_id: None,
                schema_version: 1,
                event_type: "evidence.created.v1".into(),
                payload: json!({}),
                aggregate_state_ref: storage_core::AggregateStateRef {
                    blob: storage_core::BlobRef {
                        digest: format!("sha256:{}", "b".repeat(64)),
                        size_bytes: 1,
                        media_type: "application/json".into(),
                    },
                    entity_revision: 1,
                    record_schema_version: 1,
                },
                recorded_at: "2026-10-08T00:00:00Z".into(),
                payload_digest: format!("sha256:{}", "c".repeat(64)),
            },
            replayed: false,
        };
        response.set_replayed();
        assert!(response.replayed);
    }
}
