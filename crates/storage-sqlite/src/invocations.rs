//! Durable, fail-closed CapabilityInvocation lifecycle transitions.
//!
//! This module can record observations and local pre-dispatch cancellation. It cannot
//! create Invocations or admit provider dispatch; the latter requires the future
//! combined Trust/Effect/ApprovalUse transaction.

use super::*;
use domain_invocations::{CapabilityInvocationRecord, InvocationStatus};
use storage_core::{
    CapabilityInvocationEventContext, CapabilityInvocationStore,
    CapabilityInvocationTransitionCommit, CommittedCapabilityInvocation,
};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

#[derive(Clone)]
pub struct SqliteCapabilityInvocationStore {
    store: SqliteWorkspaceStore,
}

impl SqliteCapabilityInvocationStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store }
    }

    fn run<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::InvocationOperation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }
}

impl CapabilityInvocationStore for SqliteCapabilityInvocationStore {
    fn transition_invocation(
        &self,
        mut commit: CapabilityInvocationTransitionCommit,
    ) -> Result<CommittedCapabilityInvocation, StoreError> {
        if commit.next_status == InvocationStatus::Dispatched {
            return Err(StoreError::Invalid(
                "INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE".to_owned(),
            ));
        }
        commit.event.recorded_at = canonicalize_utc_timestamp(&commit.event.recorded_at)?;
        validate_command(&commit)?;
        let runtime_principal = runtime_principal(
            &commit.event.origin_runtime_id,
            &commit.event.origin_runtime_incarnation_id,
        );
        let fingerprint = digest(&canonical_json(&json!({
            "workspace_id": commit.workspace_id,
            "invocation_id": commit.invocation_id,
            "expected_version": commit.expected_version,
            "next_status": commit.next_status,
            "event": commit.event,
        }))?);

        // Resolve replay and load the current aggregate under one writer transaction.
        let probe_workspace = commit.workspace_id.clone();
        let probe_invocation = commit.invocation_id.clone();
        let probe_principal = runtime_principal.clone();
        let probe_request_id = commit.request_id.clone();
        let probe_fingerprint = fingerprint.clone();
        let probe_runtime = commit.event.origin_runtime_id.clone();
        let probe_incarnation = commit.event.origin_runtime_incarnation_id.clone();
        let (replay, current) = self.run(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_runtime(
                &transaction,
                &probe_workspace,
                &probe_runtime,
                &probe_incarnation,
            )?;
            let replay = load_replay(
                &transaction,
                &probe_principal,
                &probe_request_id,
                &probe_fingerprint,
            )?;
            let current = if replay.is_none() {
                Some(load_invocation(
                    &transaction,
                    &probe_workspace,
                    &probe_invocation,
                )?)
            } else {
                None
            };
            transaction.commit().map_err(map_database_error)?;
            Ok((replay, current))
        })?;
        if let Some(replay) = replay {
            return Ok(replay);
        }
        let current = current.ok_or(StoreError::NotFound)?;
        if current.version != commit.expected_version {
            return Err(StoreError::Conflict {
                expected: Some(commit.expected_version),
                actual: Some(current.version),
            });
        }
        let updated = current
            .transition(commit.next_status, commit.event.recorded_at.clone())
            .map_err(map_transition_error)?;
        let draft = status_event(&commit.event, &current, &updated)?;
        let state_ref = put_invocation_state(&self.store, &updated)?;
        let workspace_id = commit.workspace_id.clone();
        let invocation_id = commit.invocation_id.clone();
        let principal = runtime_principal.clone();
        let request_id = commit.request_id.clone();
        let event_runtime_id = commit.event.origin_runtime_id.clone();
        let event_incarnation_id = commit.event.origin_runtime_incarnation_id.clone();
        self.run(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_runtime(
                &transaction,
                &workspace_id,
                &event_runtime_id,
                &event_incarnation_id,
            )?;
            if let Some(replay) = load_replay(
                &transaction,
                &principal,
                &request_id,
                &fingerprint,
            )? {
                transaction.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let actual = load_invocation(&transaction, &workspace_id, &invocation_id)?;
            if actual.version != current.version || actual.status != current.status {
                return Err(StoreError::Conflict {
                    expected: Some(current.version),
                    actual: Some(actual.version),
                });
            }
            let changed = transaction.execute(
                "UPDATE capability_invocations SET status=?1, updated_at=?2, completed_at=?3, version=?4
                 WHERE workspace_id=?5 AND invocation_id=?6 AND status=?7 AND version=?8",
                params![updated.status.as_str(), updated.updated_at, updated.completed_at,
                    to_sql_i64(updated.version, "Invocation version")?, workspace_id,
                    invocation_id, current.status.as_str(), to_sql_i64(current.version, "Invocation version")?],
            ).map_err(map_database_error)?;
            if changed != 1 {
                return Err(StoreError::Conflict {
                    expected: Some(current.version),
                    actual: None,
                });
            }
            let event = insert_domain_event(&transaction, draft, &state_ref)?;
            let committed = CommittedCapabilityInvocation {
                invocation: updated,
                event,
                replayed: false,
            };
            save_replay(
                &transaction,
                &principal,
                &request_id,
                &fingerprint,
                &committed,
            )?;
            transaction.commit().map_err(map_database_error)?;
            Ok(committed)
        })
    }
}

fn validate_command(commit: &CapabilityInvocationTransitionCommit) -> Result<(), StoreError> {
    let event = &commit.event;
    if commit.workspace_id.trim().is_empty()
        || commit.invocation_id.trim().is_empty()
        || commit.request_id.trim().is_empty()
        || commit.request_id.len() > 256
        || commit.expected_version == 0
        || event.event_id.trim().is_empty()
        || event.origin_runtime_id.trim().is_empty()
        || event.origin_runtime_incarnation_id.trim().is_empty()
        || event.origin_runtime_id.len() > 256
        || event.origin_runtime_incarnation_id.len() > 256
        || event.hlc_timestamp.trim().is_empty()
        || event.correlation_id.trim().is_empty()
        || event
            .causation_id
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        || [
            event.event_id.as_str(),
            event.origin_runtime_id.as_str(),
            event.origin_runtime_incarnation_id.as_str(),
            event.hlc_timestamp.as_str(),
            event.correlation_id.as_str(),
        ]
        .iter()
        .any(|value| value.chars().any(char::is_control))
    {
        return Err(StoreError::Invalid(
            "CapabilityInvocation transition command is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn map_transition_error(error: domain_invocations::InvocationTransitionError) -> StoreError {
    use domain_invocations::InvocationTransitionError as E;
    match error {
        E::InvalidRecord => {
            StoreError::Integrity("CapabilityInvocation record is invalid".to_owned())
        }
        E::InvalidTransition => {
            StoreError::Invalid("CAPABILITY_INVOCATION_TRANSITION_INVALID".to_owned())
        }
        E::DispatchAdmissionUnavailable => {
            StoreError::Invalid("INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE".to_owned())
        }
        E::VersionOverflow => {
            StoreError::Invalid("CAPABILITY_INVOCATION_VERSION_OVERFLOW".to_owned())
        }
    }
}

fn load_invocation(
    connection: &Connection,
    workspace: &str,
    invocation_id: &str,
) -> Result<CapabilityInvocationRecord, StoreError> {
    let raw = connection
        .query_row(
            "SELECT invocation_id,workspace_id,scope_kind,conversation_id,task_id,attempt_id,
         agent_session_id,capability_grant_id,activation_id,capability_ref_json,operation,
         request_digest,execution_method,action_batch_id,action_batch_ordinal,
         action_batch_operation_count,action_batch_digest,idempotency_key,status,
         provider_task_status,provider_task_created_at,provider_task_expires_at,
         provider_task_ttl_ms,provider_poll_after_ms,provider_updated_at,
         partial_result_refs_json,result_refs_json,effect_id,failure_json,created_at,updated_at,
         completed_at,version
         FROM capability_invocations WHERE workspace_id=?1 AND invocation_id=?2",
            params![workspace, invocation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, Option<i64>>(14)?,
                    row.get::<_, Option<i64>>(15)?,
                    row.get::<_, Option<String>>(16)?,
                    row.get::<_, Option<String>>(17)?,
                    row.get::<_, String>(18)?,
                    row.get::<_, Option<String>>(19)?,
                    row.get::<_, Option<String>>(20)?,
                    row.get::<_, Option<String>>(21)?,
                    row.get::<_, Option<i64>>(22)?,
                    row.get::<_, Option<i64>>(23)?,
                    row.get::<_, Option<String>>(24)?,
                    row.get::<_, String>(25)?,
                    row.get::<_, String>(26)?,
                    row.get::<_, Option<String>>(27)?,
                    row.get::<_, Option<String>>(28)?,
                    row.get::<_, String>(29)?,
                    row.get::<_, String>(30)?,
                    row.get::<_, Option<String>>(31)?,
                    row.get::<_, i64>(32)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;
    let parse_json = |value: String, field: &str| {
        serde_json::from_str(&value).map_err(|_| {
            StoreError::Integrity(format!("CapabilityInvocation {field} JSON is corrupt"))
        })
    };
    let record = CapabilityInvocationRecord {
        invocation_id: raw.0,
        workspace_id: raw.1,
        scope_kind: raw.2,
        conversation_id: raw.3,
        task_id: raw.4,
        attempt_id: raw.5,
        agent_session_id: raw.6,
        capability_grant_id: raw.7,
        activation_id: raw.8,
        capability_ref: parse_json(raw.9, "CapabilityRef")?,
        operation: raw.10,
        request_digest: raw.11,
        execution_method: raw.12,
        action_batch_id: raw.13,
        action_batch_ordinal: raw
            .14
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    StoreError::Integrity("Invocation batch ordinal is out of range".to_owned())
                })
            })
            .transpose()?,
        action_batch_operation_count: raw
            .15
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    StoreError::Integrity("Invocation batch count is out of range".to_owned())
                })
            })
            .transpose()?,
        action_batch_digest: raw.16,
        idempotency_key: raw.17,
        status: InvocationStatus::parse(&raw.18).map_err(map_transition_error)?,
        provider_task_status: raw.19,
        provider_task_created_at: raw.20,
        provider_task_expires_at: raw.21,
        provider_task_ttl_ms: raw
            .22
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| StoreError::Integrity("Invocation TTL is out of range".to_owned()))
            })
            .transpose()?,
        provider_poll_after_ms: raw
            .23
            .map(|value| {
                u64::try_from(value).map_err(|_| {
                    StoreError::Integrity("Invocation poll delay is out of range".to_owned())
                })
            })
            .transpose()?,
        provider_updated_at: raw.24,
        partial_result_refs: parse_json(raw.25, "partial-result refs")?,
        result_refs: parse_json(raw.26, "result refs")?,
        effect_id: raw.27,
        failure: raw
            .28
            .map(|value| parse_json(value, "failure"))
            .transpose()?,
        created_at: raw.29,
        updated_at: raw.30,
        completed_at: raw.31,
        version: u64::try_from(raw.32)
            .map_err(|_| StoreError::Integrity("Invocation version is out of range".to_owned()))?,
    };
    record.validate().map_err(map_transition_error)?;
    Ok(record)
}

fn status_event(
    context: &CapabilityInvocationEventContext,
    old: &CapabilityInvocationRecord,
    new: &CapabilityInvocationRecord,
) -> Result<EventDraft, StoreError> {
    let failure_code = new
        .failure
        .as_ref()
        .and_then(|value| value.get("code"))
        .and_then(Value::as_str);
    let mut payload = json!({
        "invocation_id": new.invocation_id,
        "from": old.status.as_str(),
        "to": new.status.as_str(),
        "result_refs": new.result_refs,
        "aggregate_version": new.version,
    });
    if let Some(effect_id) = &new.effect_id {
        payload["effect_id"] = json!(effect_id);
    }
    if let Some(failure_code) = failure_code {
        payload["failure_code"] = json!(failure_code);
    }
    Ok(EventDraft {
        event_id: context.event_id.clone(),
        workspace_id: new.workspace_id.clone(),
        entity_type: "CapabilityInvocation".to_owned(),
        entity_id: new.invocation_id.clone(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        entity_revision: new.version,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        schema_version: 1,
        event_type: "capability.invocation.status.changed.v1".to_owned(),
        payload,
        recorded_at: context.recorded_at.clone(),
    })
}

fn put_invocation_state(
    store: &SqliteWorkspaceStore,
    invocation: &CapabilityInvocationRecord,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(invocation)?;
    let blob = store.inner.blobs.put(
        &invocation.workspace_id,
        BlobPurpose::AggregateState,
        &bytes,
        "application/vnd.litecowork.capability-invocation+json",
    )?;
    if blob.digest != digest(&bytes)
        || blob.size_bytes != bytes.len() as u64
        || store
            .inner
            .blobs
            .get(&invocation.workspace_id, BlobPurpose::AggregateState, &blob)?
            != bytes
    {
        return Err(StoreError::Integrity(
            "CapabilityInvocation aggregate-state blob failed verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: invocation.version,
        record_schema_version: 1,
    })
}

fn runtime_principal(runtime: &str, incarnation: &str) -> String {
    format!("runtime:{runtime}:{incarnation}")
}

fn authorize_runtime(
    connection: &Connection,
    workspace: &str,
    runtime: &str,
    incarnation: &str,
) -> Result<(), StoreError> {
    let authorized: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces w JOIN runtimes r ON r.runtime_id=?2
          JOIN runtime_incarnations i ON i.runtime_id=r.runtime_id AND i.runtime_incarnation_id=?3
            AND i.recovery_state IN ('READY','DEGRADED')
          JOIN runtime_workspace_bindings b ON b.runtime_id=r.runtime_id AND b.workspace_id=w.workspace_id
            AND b.status='ACTIVE' AND b.revoked_at IS NULL
          WHERE w.workspace_id=?1 AND w.status='ACTIVE' AND r.current_incarnation_id=i.runtime_incarnation_id
            AND r.availability IN ('ONLINE','DEGRADED')
            AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value='EXECUTOR'))",
        params![workspace, runtime, incarnation], |row| row.get(0),
    ).map_err(map_database_error)?;
    if authorized {
        Ok(())
    } else {
        Err(StoreError::Invalid("Runtime incarnation is not authorized to mutate CapabilityInvocations in this Workspace".to_owned()))
    }
}

fn load_replay(
    transaction: &Transaction<'_>,
    principal: &str,
    request_id: &str,
    fingerprint: &str,
) -> Result<Option<CommittedCapabilityInvocation>, StoreError> {
    let prior: Option<(String, Option<String>)> = transaction.query_row(
        "SELECT request_digest,response_json FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
        params![principal, request_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(map_database_error)?;
    match prior {
        None => Ok(None),
        Some((digest, response)) if digest == fingerprint => response
            .map(|value| {
                let mut committed: CommittedCapabilityInvocation = serde_json::from_str(&value)
                    .map_err(|_| {
                        StoreError::Integrity(
                            "CapabilityInvocation transition receipt is corrupt".to_owned(),
                        )
                    })?;
                committed.replayed = true;
                Ok(committed)
            })
            .transpose(),
        Some(_) => Err(StoreError::Invalid(
            "request ID was reused with different CapabilityInvocation transition content"
                .to_owned(),
        )),
    }
}

fn save_replay(
    transaction: &Transaction<'_>,
    principal: &str,
    request_id: &str,
    fingerprint: &str,
    committed: &CommittedCapabilityInvocation,
) -> Result<(), StoreError> {
    let response = canonical_json(committed)?;
    let response =
        String::from_utf8(response).map_err(|error| StoreError::Invalid(error.to_string()))?;
    transaction.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,NULL)",
        params![principal, request_id, fingerprint, response, digest(response.as_bytes()), committed.event.recorded_at],
    ).map_err(map_database_error)?;
    Ok(())
}
