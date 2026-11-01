//! Immutable persistence for authenticated hosted-child ResultEnvelopes.
//!
//! A provider's report is untrusted input. This adapter binds it to the exact terminal
//! child Attempt and AgentSession, stores the canonical envelope as the aggregate-state
//! blob, and commits the immutable result row and safe summary event together.

use super::*;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};
use std::sync::mpsc;
use storage_core::{
    AggregateStateRef, BlobPurpose, CommittedDelegationResult, DelegationResultPublishCommit,
    DelegationResultReadQuery, DelegationResultStore, DomainEvent, StoreError,
};

const RESULT_ENVELOPE_MEDIA_TYPE: &str = "application/vnd.litecow.delegation-result+json";
const RESULT_EVENT_TYPE: &str = "delegation.result.published.v1";
const RESULT_EVENT_PAYLOAD_KEYS: [&str; 6] = [
    "child_attempt_id",
    "task_id",
    "status",
    "revision_at_start",
    "result_digest",
    "aggregate_version",
];


#[derive(Clone)]
pub struct SqliteDelegationResultStore {
    store: SqliteWorkspaceStore,
}

impl SqliteDelegationResultStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store }
    }

    fn run<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }
}

impl DelegationResultStore for SqliteDelegationResultStore {
    fn publish_delegation_result(
        &self,
        mut commit: DelegationResultPublishCommit,
    ) -> Result<CommittedDelegationResult, StoreError> {
        validate_publish_identity(&commit)?;
        commit.event.recorded_at = canonicalize_utc_timestamp(&commit.event.recorded_at)?;
        commit.event.hlc_timestamp = canonicalize_utc_timestamp(&commit.event.hlc_timestamp)?;
        let bytes = canonical_json(&commit.envelope_json)?;
        if bytes.len() > 1_048_576 {
            return Err(invalid("DELEGATION_RESULT_ENVELOPE_TOO_LARGE"));
        }
        if digest(&bytes) != commit.envelope_digest {
            return Err(invalid("DELEGATION_RESULT_DIGEST_MISMATCH"));
        }
        let workspace_id = commit.workspace_id.clone();
        let event = commit.event.clone();
        let blob_store = Arc::clone(&self.store.inner.blobs);

        // Avoid creating a second blob for a normal exact replay. The commit transaction
        // repeats this check after blob preparation to close the concurrent-publisher race.
        if let Some(existing) = self.run({
            let commit = commit.clone();
            move |connection| {
                let tx = connection.transaction().map_err(map_database_error)?;
                let result = find_replay(&tx, &commit)?;
                tx.commit().map_err(map_database_error)?;
                Ok(result)
            }
        })? {
            verify_result_blob(blob_store.as_ref(), &existing)?;
            return Ok(existing);
        }

        let blob = blob_store.put(
            &workspace_id,
            BlobPurpose::AggregateState,
            &bytes,
            RESULT_ENVELOPE_MEDIA_TYPE,
        )?;
        if blob.digest != digest(&bytes)
            || blob.size_bytes != bytes.len() as u64
            || blob_store.get(&workspace_id, BlobPurpose::AggregateState, &blob)? != bytes
        {
            return Err(StoreError::Io(
                "delegation result aggregate-state blob failed verification".to_owned(),
            ));
        }

        self.run(move |connection| publish_transaction(connection, commit, event, blob))
            .and_then(|result| {
                verify_result_blob(blob_store.as_ref(), &result)?;
                Ok(result)
            })
    }

    fn get_delegation_result(
        &self,
        query: DelegationResultReadQuery,
    ) -> Result<Option<CommittedDelegationResult>, StoreError> {
        validate_read_query(&query)?;
        let blob_store = Arc::clone(&self.store.inner.blobs);
        let result = self.run(move |connection| {
            let tx = connection.transaction().map_err(map_database_error)?;
            let owner: Option<String> = tx
                .query_row(
                    "SELECT owner_principal_id FROM workspaces WHERE workspace_id = ?1",
                    [&query.workspace_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            if owner.as_deref() != Some(query.owner_principal_id.as_str()) {
                return Ok(None);
            }
            let result = load_result(&tx, &query.child_attempt_id)?;
            let result = result
                .filter(|result| {
                    result.workspace_id == query.workspace_id
                        && result.task_id == query.task_id
                        && result.revision_at_start == query.task_spec_revision
                })
                .map(|result| {
                    if !result_is_scoped_to_parent(&tx, &query, &result)? {
                        return Ok(None);
                    }
                    let event = load_event(&tx, &result.event_id)?.ok_or_else(|| {
                        StoreError::Integrity(
                            "delegation result references a missing domain event".to_owned(),
                        )
                    })?;
                    validate_stored_event(&event, &result)?;
                    Ok(Some(CommittedDelegationResult {
                        workspace_id: result.workspace_id,
                        task_id: result.task_id,
                        child_attempt_id: result.child_attempt_id,
                        published_by_agent_session_id: result.published_by_agent_session_id,
                        runtime_id: result.runtime_id,
                        runtime_incarnation_id: result.runtime_incarnation_id,
                        envelope_json: result.envelope_json,
                        envelope_digest: result.envelope_digest,
                        event,
                        replayed: false,
                    }))
                })
                .transpose()?;
            let result = result.flatten();
            tx.commit().map_err(map_database_error)?;
            Ok(result)
        })?;
        if let Some(result) = &result {
            verify_result_blob(blob_store.as_ref(), result)?;
        }
        Ok(result)
    }
}

fn result_is_scoped_to_parent(
    tx: &Transaction<'_>,
    query: &DelegationResultReadQuery,
    result: &ExistingResult,
) -> Result<bool, StoreError> {
    tx.query_row(
        "SELECT EXISTS(
           SELECT 1
           FROM delegation_results r
           JOIN tasks t ON t.task_id = r.task_id AND t.workspace_id = r.workspace_id
           JOIN attempts child ON child.attempt_id = r.child_attempt_id AND child.task_id = r.task_id
           JOIN attempts parent ON parent.attempt_id = child.parent_attempt_id AND parent.task_id = child.task_id
           JOIN agent_sessions parent_session
             ON parent_session.agent_session_id = parent.agent_session_id
            AND parent_session.workspace_id = t.workspace_id
            AND parent_session.task_id = t.task_id
            AND parent_session.attempt_id = parent.attempt_id
            AND parent_session.agent_binding_id = parent.agent_binding_id
            AND parent_session.runtime_id = parent.runtime_id
            AND parent_session.runtime_incarnation_id = parent.runtime_incarnation_id
            AND parent_session.scope_kind = 'ATTEMPT_EXECUTION'
           WHERE r.child_attempt_id = ?1 AND r.task_id = ?2 AND r.workspace_id = ?3
             AND r.revision_at_start = ?4 AND child.parent_attempt_id = ?5
             AND parent.attempt_id = ?5 AND parent_session.task_spec_revision = ?4
         )",
        params![
            result.child_attempt_id,
            query.task_id,
            query.workspace_id,
            to_sql_i64(query.task_spec_revision, "Result read TaskSpec revision")?,
            query.parent_attempt_id,
        ],
        |row| row.get(0),
    )
    .map_err(map_database_error)
}

fn verify_result_blob(
    blob_store: &dyn storage_core::BlobStore,
    result: &CommittedDelegationResult,
) -> Result<(), StoreError> {
    let state_ref = &result.event.aggregate_state_ref;
    let expected = canonical_json(&result.envelope_json)?;
    let bytes = blob_store.get(
        &result.workspace_id,
        BlobPurpose::AggregateState,
        &state_ref.blob,
    )?;
    if state_ref.blob.media_type != RESULT_ENVELOPE_MEDIA_TYPE
        || state_ref.blob.digest != result.envelope_digest
        || state_ref.blob.digest != digest(&expected)
        || state_ref.blob.size_bytes != expected.len() as u64
        || bytes != expected
    {
        return Err(StoreError::Integrity(
            "delegation result aggregate-state blob does not match immutable envelope".to_owned(),
        ));
    }
    Ok(())
}

fn publish_transaction(
    connection: &mut Connection,
    commit: DelegationResultPublishCommit,
    event_draft: storage_core::EventDraft,
    blob: storage_core::BlobRef,
) -> Result<CommittedDelegationResult, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    if let Some(replay) = find_replay(&tx, &commit)? {
        return Ok(replay);
    }
    let attempt = load_publish_attempt(&tx, &commit)?;
    validate_result_attempt(&commit, &event_draft, &attempt)?;

    let state_ref = AggregateStateRef {
        blob,
        entity_revision: 1,
        record_schema_version: 1,
    };
    let event = insert_domain_event(&tx, &event_draft, &state_ref)?;
    let status = required_string(&commit.envelope_json, "status")?;
    let revision = required_u64(&commit.envelope_json, "revision_at_start")?;
    let envelope_json = String::from_utf8(canonical_json(&commit.envelope_json)?)
        .map_err(|error| invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO delegation_results(child_attempt_id, task_id, workspace_id,
             published_by_agent_session_id, runtime_id, runtime_incarnation_id, status,
             revision_at_start, envelope_json, envelope_digest, event_id, published_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            commit.child_attempt_id,
            attempt.task_id,
            commit.workspace_id,
            commit.agent_session_id,
            commit.runtime_id,
            commit.runtime_incarnation_id,
            status,
            to_sql_i64(revision, "ResultEnvelope task revision")?,
            envelope_json,
            commit.envelope_digest,
            event.event_id,
            event.recorded_at,
        ],
    )
    .map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;

    Ok(CommittedDelegationResult {
        workspace_id: commit.workspace_id,
        task_id: attempt.task_id,
        child_attempt_id: commit.child_attempt_id,
        published_by_agent_session_id: commit.agent_session_id,
        runtime_id: commit.runtime_id,
        runtime_incarnation_id: commit.runtime_incarnation_id,
        envelope_json: commit.envelope_json,
        envelope_digest: commit.envelope_digest,
        event,
        replayed: false,
    })
}

#[derive(Clone)]
struct ExistingResult {
    child_attempt_id: String,
    task_id: String,
    workspace_id: String,
    published_by_agent_session_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
    status: String,
    revision_at_start: u64,
    envelope_json: Value,
    envelope_digest: String,
    event_id: String,
}

struct PublishAttempt {
    task_id: String,
    workspace_id: String,
    status: String,
    parent_attempt_id: Option<String>,
    agent_session_id: Option<String>,
    runtime_id: String,
    runtime_incarnation_id: String,
    session_task_spec_revision: Option<u64>,
    session_status: Option<String>,
    attempt_settled_at: Option<String>,
    session_closed_at: Option<String>,
}

fn load_publish_attempt(
    tx: &Transaction<'_>,
    commit: &DelegationResultPublishCommit,
) -> Result<PublishAttempt, StoreError> {
    let row: Option<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = tx
        .query_row(
            "SELECT a.task_id, t.workspace_id, a.status, a.parent_attempt_id,
                    a.agent_session_id, a.runtime_id, a.runtime_incarnation_id,
                    s.task_spec_revision, s.status, a.settled_at, s.closed_at
             FROM attempts a
             JOIN tasks t ON t.task_id = a.task_id AND t.workspace_id = ?1
             LEFT JOIN agent_sessions s
               ON s.agent_session_id = a.agent_session_id
              AND s.workspace_id = t.workspace_id
              AND s.task_id = a.task_id
              AND s.attempt_id = a.attempt_id
              AND s.runtime_id = a.runtime_id
              AND s.runtime_incarnation_id = a.runtime_incarnation_id
              AND s.agent_binding_id = a.agent_binding_id
              AND s.scope_kind = 'ATTEMPT_EXECUTION'
             WHERE a.attempt_id = ?2 AND a.agent_session_id = ?3
               AND a.runtime_id = ?4 AND a.runtime_incarnation_id = ?5",
            params![
                commit.workspace_id,
                commit.child_attempt_id,
                commit.agent_session_id,
                commit.runtime_id,
                commit.runtime_incarnation_id,
            ],
            |row| {
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
                    row.get(9)?,
                    row.get(10)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        task_id,
        workspace_id,
        status,
        parent_attempt_id,
        agent_session_id,
        runtime_id,
        runtime_incarnation_id,
        task_spec_revision,
        session_status,
        attempt_settled_at,
        session_closed_at,
    )) = row
    else {
        return Err(StoreError::NotFound);
    };
    Ok(PublishAttempt {
        task_id,
        workspace_id,
        status,
        parent_attempt_id,
        agent_session_id,
        runtime_id,
        runtime_incarnation_id,
        session_task_spec_revision: task_spec_revision
            .map(|value| from_sql_i64(value, "AgentSession TaskSpec revision"))
            .transpose()?,
        session_status,
        attempt_settled_at,
        session_closed_at,
    })
}

fn validate_result_attempt(
    commit: &DelegationResultPublishCommit,
    event: &storage_core::EventDraft,
    attempt: &PublishAttempt,
) -> Result<(), StoreError> {
    let result_status = required_string(&commit.envelope_json, "status")?;
    let revision = required_u64(&commit.envelope_json, "revision_at_start")?;
    let terminal_status_matches = result_status_matches_attempt(&result_status, &attempt.status);
    let expected_payload = json!({
        "child_attempt_id": commit.child_attempt_id,
        "task_id": attempt.task_id,
        "status": result_status,
        "revision_at_start": revision,
        "result_digest": commit.envelope_digest,
        "aggregate_version": 1,
    });
    if attempt.workspace_id != commit.workspace_id
        || attempt.parent_attempt_id.is_none()
        || attempt.agent_session_id.as_deref() != Some(commit.agent_session_id.as_str())
        || attempt.runtime_id != commit.runtime_id
        || attempt.runtime_incarnation_id != commit.runtime_incarnation_id
        || attempt.session_task_spec_revision != Some(revision)
        || !matches!(attempt.session_status.as_deref(), Some("CLOSED" | "LOST"))
        || attempt
            .attempt_settled_at
            .as_deref()
            .is_none_or(str::is_empty)
        || attempt
            .session_closed_at
            .as_deref()
            .is_none_or(str::is_empty)
        || !is_publishable_terminal_attempt_status(&attempt.status)
        || !terminal_status_matches
        || event.workspace_id != commit.workspace_id
        || event.entity_type != "DELEGATION_RESULT"
        || event.entity_id != commit.child_attempt_id
        || event.origin_runtime_id != commit.runtime_id
        || event.entity_revision != 1
        || event.schema_version != 1
        || event.event_type != RESULT_EVENT_TYPE
        || event.origin_runtime_id != commit.runtime_id
        || event.payload != expected_payload
        || !payload_has_exact_keys(&event.payload, &RESULT_EVENT_PAYLOAD_KEYS)
    {
        return Err(invalid("DELEGATION_RESULT_ATTEMPT_OR_EVENT_MISMATCH"));
    }
    Ok(())
}

fn is_publishable_terminal_attempt_status(status: &str) -> bool {
    matches!(status, "COMPLETED" | "FAILED" | "CANCELLED" | "ABANDONED")
}

fn result_status_matches_attempt(result_status: &str, attempt_status: &str) -> bool {
    matches!(
        (result_status, attempt_status),
        ("COMPLETED", "COMPLETED")
            | ("FAILED", "FAILED")
            | ("BLOCKED", "FAILED")
            | ("CANCELLED", "CANCELLED")
            | ("CANCELLED", "ABANDONED")
    )
}

fn find_replay(
    tx: &Transaction<'_>,
    commit: &DelegationResultPublishCommit,
) -> Result<Option<CommittedDelegationResult>, StoreError> {
    let Some(existing) = load_result(tx, &commit.child_attempt_id)? else {
        return Ok(None);
    };
    if existing.workspace_id != commit.workspace_id {
        return Err(StoreError::NotFound);
    }
    if existing.published_by_agent_session_id != commit.agent_session_id
        || existing.runtime_id != commit.runtime_id
        || existing.runtime_incarnation_id != commit.runtime_incarnation_id
    {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let existing_canonical = canonical_json(&existing.envelope_json)?;
    let incoming_canonical = canonical_json(&commit.envelope_json)?;
    if existing.envelope_digest != commit.envelope_digest
        || existing_canonical != incoming_canonical
    {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let event = load_event(tx, &existing.event_id)?.ok_or_else(|| {
        StoreError::Integrity("delegation result references a missing domain event".to_owned())
    })?;
    validate_stored_event(&event, &existing)?;
    Ok(Some(CommittedDelegationResult {
        workspace_id: existing.workspace_id,
        task_id: existing.task_id,
        child_attempt_id: existing.child_attempt_id,
        published_by_agent_session_id: existing.published_by_agent_session_id,
        runtime_id: existing.runtime_id,
        runtime_incarnation_id: existing.runtime_incarnation_id,
        envelope_json: existing.envelope_json,
        envelope_digest: existing.envelope_digest,
        event,
        replayed: true,
    }))
}

fn load_result(
    connection: &Connection,
    child_attempt_id: &str,
) -> Result<Option<ExistingResult>, StoreError> {
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        String,
        String,
        String,
    )> = connection
        .query_row(
            "SELECT child_attempt_id, task_id, workspace_id, published_by_agent_session_id,
                    runtime_id, runtime_incarnation_id, status, revision_at_start,
                    envelope_json, envelope_digest, event_id
             FROM delegation_results WHERE child_attempt_id = ?1",
            [child_attempt_id],
            |row| {
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
                    row.get(9)?,
                    row.get(10)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    row.map(
        |(
            child_attempt_id,
            task_id,
            workspace_id,
            published_by_agent_session_id,
            runtime_id,
            runtime_incarnation_id,
            status,
            revision_at_start,
            envelope,
            envelope_digest,
            event_id,
        )| {
            let envelope_json: Value = serde_json::from_str(&envelope)
                .map_err(|error| StoreError::Integrity(error.to_string()))?;
            let bytes = canonical_json(&envelope_json)?;
            let canonical_text = String::from_utf8(bytes.clone())
                .map_err(|error| StoreError::Integrity(error.to_string()))?;
            let revision_at_start =
                from_sql_i64(revision_at_start, "stored ResultEnvelope revision")?;
            if envelope != canonical_text
                || digest(&bytes) != envelope_digest
                || required_string(&envelope_json, "child_attempt_id")? != child_attempt_id
                || required_string(&envelope_json, "status")? != status
                || required_u64(&envelope_json, "revision_at_start")? != revision_at_start
            {
                return Err(StoreError::Integrity(
                    "stored delegation result columns do not match its canonical envelope"
                        .to_owned(),
                ));
            }
            Ok(ExistingResult {
                child_attempt_id,
                task_id,
                workspace_id,
                published_by_agent_session_id,
                runtime_id,
                runtime_incarnation_id,
                status,
                revision_at_start,
                envelope_json,
                envelope_digest,
                event_id,
            })
        },
    )
    .transpose()
}

fn load_event(connection: &Connection, event_id: &str) -> Result<Option<DomainEvent>, StoreError> {
    let row: Option<(
        String,
        String,
        String,
        String,
        i64,
        i64,
        String,
        String,
        Option<String>,
        i64,
        String,
        String,
        String,
        String,
        String,
        String,
    )> = connection
        .query_row(
            "SELECT event_id, workspace_id, entity_type, entity_id, origin_sequence,
                    entity_revision, origin_runtime_id, hlc_timestamp, causation_id,
                    schema_version, correlation_id, type, payload_json,
                    aggregate_state_ref_json, recorded_at, payload_digest
             FROM domain_events WHERE event_id = ?1",
            [event_id],
            |row| {
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
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    row.map(
        |(
            event_id,
            workspace_id,
            entity_type,
            entity_id,
            origin_sequence,
            entity_revision,
            origin_runtime_id,
            hlc_timestamp,
            causation_id,
            schema_version,
            correlation_id,
            event_type,
            payload,
            state_ref,
            recorded_at,
            stored_payload_digest,
        )| {
            let payload: Value = serde_json::from_str(&payload)
                .map_err(|error| StoreError::Integrity(error.to_string()))?;
            let aggregate_state_ref = serde_json::from_str(&state_ref)
                .map_err(|error| StoreError::Integrity(error.to_string()))?;
            let payload_bytes = canonical_json(&payload)?;
            let actual_payload_digest = digest(&payload_bytes);
            if actual_payload_digest != stored_payload_digest {
                return Err(StoreError::Integrity(
                    "stored domain event payload digest does not match payload".to_owned(),
                ));
            }
            Ok(DomainEvent {
                event_id,
                workspace_id,
                entity_type,
                entity_id,
                origin_runtime_id,
                origin_sequence: from_sql_i64(origin_sequence, "event origin sequence")?,
                entity_revision: from_sql_i64(entity_revision, "event entity revision")?,
                hlc_timestamp,
                correlation_id,
                causation_id,
                schema_version: u32::try_from(schema_version).map_err(|_| {
                    StoreError::Integrity("event schema version is invalid".to_owned())
                })?,
                event_type,
                payload,
                aggregate_state_ref,
                recorded_at,
                payload_digest: stored_payload_digest,
            })
        },
    )
    .transpose()
}

fn validate_stored_event(event: &DomainEvent, result: &ExistingResult) -> Result<(), StoreError> {
    let expected_payload = json!({
        "child_attempt_id": result.child_attempt_id,
        "task_id": result.task_id,
        "status": result.status,
        "revision_at_start": result.revision_at_start,
        "result_digest": result.envelope_digest,
        "aggregate_version": 1,
    });
    if event.workspace_id != result.workspace_id
        || event.event_id != result.event_id
        || event.entity_type != "DELEGATION_RESULT"
        || event.entity_id != result.child_attempt_id
        || event.origin_runtime_id != result.runtime_id
        || event.origin_sequence == 0
        || event.entity_revision != 1
        || event.schema_version != 1
        || event.event_type != RESULT_EVENT_TYPE
        || event.payload != expected_payload
        || !payload_has_exact_keys(&event.payload, &RESULT_EVENT_PAYLOAD_KEYS)
        || event.aggregate_state_ref.blob.media_type != RESULT_ENVELOPE_MEDIA_TYPE
        || event.aggregate_state_ref.blob.digest != result.envelope_digest
        || event.aggregate_state_ref.entity_revision != 1
        || event.aggregate_state_ref.record_schema_version != 1
    {
        return Err(StoreError::Integrity(
            "stored delegation result event does not match its immutable result".to_owned(),
        ));
    }
    Ok(())
}

fn validate_publish_identity(commit: &DelegationResultPublishCommit) -> Result<(), StoreError> {
    for (name, value) in [
        ("Workspace ID", commit.workspace_id.as_str()),
        ("Runtime ID", commit.runtime_id.as_str()),
        (
            "Runtime incarnation ID",
            commit.runtime_incarnation_id.as_str(),
        ),
        ("AgentSession ID", commit.agent_session_id.as_str()),
        ("child Attempt ID", commit.child_attempt_id.as_str()),
    ] {
        validate_id(value, name)?;
    }
    validate_id(&commit.event.event_id, "result event ID")?;
    validate_id(&commit.event.correlation_id, "result correlation ID")?;
    if let Some(causation_id) = &commit.event.causation_id {
        validate_id(causation_id, "result causation ID")?;
    }
    if !valid_sha256(&commit.envelope_digest) {
        return Err(invalid("DELEGATION_RESULT_DIGEST_INVALID"));
    }
    let keys = [
        "child_attempt_id",
        "status",
        "summary",
        "output_refs",
        "artifact_refs",
        "evidence_refs",
        "changed_resource_refs",
        "unresolved",
        "blockers",
        "verification_hints",
        "revision_at_start",
    ];
    if !commit.envelope_json.is_object()
        || keys.iter().any(|key| {
            !commit
                .envelope_json
                .as_object()
                .is_some_and(|object| object.contains_key(*key))
        })
        || commit
            .envelope_json
            .as_object()
            .is_some_and(|object| object.keys().any(|key| !is_result_envelope_key(key)))
        || required_string(&commit.envelope_json, "child_attempt_id")? != commit.child_attempt_id
        || !matches!(
            required_string(&commit.envelope_json, "status")?.as_str(),
            "COMPLETED" | "FAILED" | "BLOCKED" | "CANCELLED"
        )
        || required_u64(&commit.envelope_json, "revision_at_start")? == 0
    {
        return Err(invalid("DELEGATION_RESULT_ENVELOPE_IDENTITY_INVALID"));
    }
    Ok(())
}

fn validate_read_query(query: &DelegationResultReadQuery) -> Result<(), StoreError> {
    validate_id(&query.owner_principal_id, "Owner principal ID")?;
    validate_id(&query.workspace_id, "Workspace ID")?;
    validate_id(&query.task_id, "Task ID")?;
    validate_id(&query.parent_attempt_id, "parent Attempt ID")?;
    validate_id(&query.child_attempt_id, "child Attempt ID")?;
    if query.parent_attempt_id == query.child_attempt_id || query.task_spec_revision == 0 {
        return Err(invalid("Result read TaskSpec revision must be positive"));
    }
    Ok(())
}

fn validate_id(value: &str, field: &str) -> Result<(), StoreError> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(invalid(format!("{field} is invalid")));
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_result_envelope_key(value: &str) -> bool {
    matches!(
        value,
        "child_attempt_id"
            | "status"
            | "summary"
            | "output_refs"
            | "artifact_refs"
            | "evidence_refs"
            | "changed_resource_refs"
            | "unresolved"
            | "blockers"
            | "usage"
            | "confidence"
            | "verification_hints"
            | "revision_at_start"
    )
}

fn required_string(value: &Value, key: &str) -> Result<String, StoreError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("ResultEnvelope {key} is missing or invalid")))
}

fn required_u64(value: &Value, key: &str) -> Result<u64, StoreError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("ResultEnvelope {key} is missing or invalid")))
}

fn invalid(message: impl Into<String>) -> StoreError {
    StoreError::Invalid(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use std::{collections::HashMap, sync::Mutex};
    use storage_core::{BlobRef, BlobStore, StoreError};

    fn envelope() -> Value {
        json!({
            "child_attempt_id": "child-1",
            "status": "COMPLETED",
            "summary": "finished",
            "output_refs": [],
            "artifact_refs": [],
            "evidence_refs": [],
            "changed_resource_refs": [],
            "unresolved": [],
            "blockers": [],
            "verification_hints": [],
            "revision_at_start": 1,
        })
    }

    fn commit() -> DelegationResultPublishCommit {
        let envelope_json = envelope();
        let envelope_digest = digest(&canonical_json(&envelope_json).expect("canonical JSON"));
        let payload = json!({
            "child_attempt_id": "child-1",
            "task_id": "task-1",
            "status": "COMPLETED",
            "revision_at_start": 1,
            "result_digest": envelope_digest,
            "aggregate_version": 1,
        });
        DelegationResultPublishCommit {
            workspace_id: "workspace-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            agent_session_id: "session-1".to_owned(),
            child_attempt_id: "child-1".to_owned(),
            envelope_json,
            envelope_digest,
            event: storage_core::EventDraft {
                event_id: "event-1".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                entity_type: "DELEGATION_RESULT".to_owned(),
                entity_id: "child-1".to_owned(),
                origin_runtime_id: "runtime-1".to_owned(),
                entity_revision: 1,
                hlc_timestamp: "2026-10-17T12:00:00Z".to_owned(),
                correlation_id: "correlation-1".to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: RESULT_EVENT_TYPE.to_owned(),
                payload,
                recorded_at: "2026-10-17T12:00:00Z".to_owned(),
            },
        }
    }

    fn result_database() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch(
                "CREATE TABLE domain_events (
                    event_id TEXT PRIMARY KEY, workspace_id TEXT, entity_type TEXT,
                    entity_id TEXT, origin_runtime_id TEXT, origin_sequence INTEGER,
                    entity_revision INTEGER, hlc_timestamp TEXT, causation_id TEXT,
                    schema_version INTEGER, correlation_id TEXT, type TEXT, payload_json TEXT,
                    aggregate_state_ref_json TEXT, recorded_at TEXT, payload_digest TEXT
                 );
                 CREATE TABLE delegation_results (
                    child_attempt_id TEXT PRIMARY KEY, task_id TEXT, workspace_id TEXT,
                    published_by_agent_session_id TEXT, runtime_id TEXT,
                    runtime_incarnation_id TEXT, status TEXT, revision_at_start INTEGER,
                    envelope_json TEXT, envelope_digest TEXT, event_id TEXT, published_at TEXT
                 );",
            )
            .expect("test schema");
        let commit = commit();
        let payload_json = String::from_utf8(
            canonical_json(&commit.event.payload).expect("canonical event payload"),
        )
        .expect("UTF-8 payload");
        let payload_digest = digest(payload_json.as_bytes());
        let aggregate_state_ref = AggregateStateRef {
            blob: storage_core::BlobRef {
                digest: commit.envelope_digest.clone(),
                size_bytes: canonical_json(&commit.envelope_json).unwrap().len() as u64,
                media_type: RESULT_ENVELOPE_MEDIA_TYPE.to_owned(),
            },
            entity_revision: 1,
            record_schema_version: 1,
        };
        let state_json =
            String::from_utf8(canonical_json(&aggregate_state_ref).expect("canonical state ref"))
                .expect("UTF-8 state ref");
        let envelope_json =
            String::from_utf8(canonical_json(&commit.envelope_json).expect("canonical envelope"))
                .expect("UTF-8 envelope");
        connection
            .execute(
                "INSERT INTO domain_events VALUES (?1, ?2, ?3, ?4, ?5, 1, 1, ?6, NULL, 1, ?7, ?8, ?9, ?10, ?6, ?11)",
                params![
                    commit.event.event_id,
                    commit.event.workspace_id,
                    commit.event.entity_type,
                    commit.event.entity_id,
                    commit.event.origin_runtime_id,
                    commit.event.recorded_at,
                    commit.event.correlation_id,
                    commit.event.event_type,
                    payload_json,
                    state_json,
                    payload_digest,
                ],
            )
            .expect("insert event");
        connection
            .execute(
                "INSERT INTO delegation_results VALUES ('child-1', 'task-1', 'workspace-1', 'session-1', 'runtime-1', 'incarnation-1', 'COMPLETED', 1, ?1, ?2, 'event-1', '2026-10-17T12:00:00Z')",
                params![envelope_json, commit.envelope_digest],
            )
            .expect("insert immutable result");
        connection
    }

    struct FixedBlobStore(Vec<u8>);

    impl storage_core::BlobStore for FixedBlobStore {
        fn put(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            _bytes: &[u8],
            _media_type: &str,
        ) -> Result<BlobRef, StoreError> {
            Err(StoreError::Blob("test store is read-only".to_owned()))
        }

        fn get(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            _blob: &BlobRef,
        ) -> Result<Vec<u8>, StoreError> {
            Ok(self.0.clone())
        }
    }

    #[derive(Default)]
    struct MemoryBlobStore(Mutex<HashMap<(String, String), Vec<u8>>>);

    impl BlobStore for MemoryBlobStore {
        fn put(
            &self,
            workspace_id: &str,
            _purpose: BlobPurpose,
            bytes: &[u8],
            media_type: &str,
        ) -> Result<BlobRef, StoreError> {
            let blob = BlobRef {
                digest: digest(bytes),
                size_bytes: bytes.len() as u64,
                media_type: media_type.to_owned(),
            };
            self.0.lock().unwrap().insert(
                (workspace_id.to_owned(), blob.digest.clone()),
                bytes.to_vec(),
            );
            Ok(blob)
        }

        fn get(
            &self,
            workspace_id: &str,
            _purpose: BlobPurpose,
            blob: &BlobRef,
        ) -> Result<Vec<u8>, StoreError> {
            self.0
                .lock()
                .unwrap()
                .get(&(workspace_id.to_owned(), blob.digest.clone()))
                .cloned()
                .ok_or_else(|| StoreError::Blob("test blob not found".to_owned()))
        }
    }

    fn open_production_store(
        directory: &tempfile::TempDir,
        blobs: Arc<MemoryBlobStore>,
    ) -> SqliteWorkspaceStore {
        SqliteWorkspaceStore::open(
            directory.path().join("state/state.sqlite3"),
            blobs,
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: std::time::Duration::from_secs(2),
            },
        )
        .expect("fully migrated SQLite store opens")
    }

    fn seed_terminal_child(path: &std::path::Path) {
        let connection = Connection::open(path).expect("open migrated database for fixture");
        connection
            .execute_batch(
                r#"PRAGMA foreign_keys = ON;
                 PRAGMA defer_foreign_keys = ON;
                 BEGIN IMMEDIATE;
                 INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at)
                   VALUES ('profile-1', 'test', 'Test Agent', '2026-10-17T12:00:00Z');
                 INSERT INTO runtimes(runtime_id, device_identity_json, runtime_version,
                   platform, architecture, roles_json, trust_zone, availability, startup_policy,
                   current_incarnation_id, resource_capacity_json, last_seen, version)
                   VALUES ('runtime-1', '{}', 'test', 'linux', 'x86_64',
                   '["OPERATOR_ENDPOINT","EXECUTOR"]', 'PERSONAL_DEVICE', 'ONLINE', 'MANUAL',
                   'incarnation-1', '{}', '2026-10-17T12:00:00Z', 1);
                 INSERT INTO runtime_incarnations(runtime_incarnation_id, runtime_id, process_started_at,
                   litecowork_version, recovered_from_unclean_shutdown, recovery_state, ready_at, version)
                   VALUES ('incarnation-1', 'runtime-1', '2026-10-17T11:00:00Z', 'test', 0,
                   'READY', '2026-10-17T11:00:00Z', 1);
                 INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id, runtime_id,
                   workspace_id, enrollment_mode, status, roles_json, created_at, activated_at, version)
                   VALUES ('runtime-binding-1', 'runtime-1', 'workspace-1', 'LOCAL_ENROLLMENT',
                   'ACTIVE', '["OPERATOR_ENDPOINT","EXECUTOR"]', '2026-10-17T11:00:00Z',
                   '2026-10-17T11:00:00Z', 1);
                 INSERT INTO agent_endpoints(endpoint_id, agent_profile_id, protocol, topology,
                   protocol_version, capabilities_json)
                   VALUES ('endpoint-1', 'profile-1', 'CLI', 'PROCESS_ADAPTER', '1', '[]');
                 INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, runtime_id,
                   endpoint_selection_policy_json, auth_ref, configuration_json, enabled,
                   lead_eligible, created_at, version)
                   VALUES ('binding-1', 'workspace-1', 'profile-1', 'runtime-1', '{}', NULL, '{}',
                   1, 1, '2026-10-17T11:00:00Z', 1);
                 INSERT INTO environments(environment_id, runtime_id, provider_kind, class, lifetime,
                   owner_workspace_id, owner_task_id, owner_attempt_id, owner_coworker_id,
                   owner_principal_id, sharing_scope, name, created_by_incarnation_id, status, health,
                   budget_enforcement_policy, budget_enforcement, source_resources_json,
                   resource_limits_json, network_policy_json, budget_ceiling_json, provision_preview_digest,
                   retention_expires_at, backup_policy, isolation_json, created_at, updated_at, version)
                   VALUES ('environment-1', 'runtime-1', 'LOCAL', 'ISOLATED', 'TASK_RETAINED',
                   'workspace-1', 'task-1', NULL, NULL, NULL, 'TASK_SHARED', 'test environment',
                   'incarnation-1', 'READY', 'HEALTHY', 'ALLOW_HOST_MONITORED', 'HOST_MONITORED',
                   '[]', '{}', '{}', '{}', NULL, NULL, 'EXCLUDED', '{}',
                   '2026-10-17T11:00:00Z', '2026-10-17T11:00:00Z', 1);
                 INSERT INTO tasks(task_id, workspace_id, current_spec_revision, current_plan_revision,
                   status, lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                   created_at, updated_at, version)
                   VALUES ('task-1', 'workspace-1', 1, 1, 'READY', 'binding-1', '[]', 'NORMAL',
                   '{"kind":"USER","principal_id":"owner-1"}', '2026-10-17T11:00:00Z',
                   '2026-10-17T11:00:00Z', 1);
                 INSERT INTO task_spec_revisions(task_id, workspace_id, revision, parent_revisions_json,
                   objective, constraints_json, non_goals_json, input_refs_json, required_outputs_json,
                   acceptance_criteria_json, approvals_required_json, lead_failover_policy_json,
                   source_message_refs_json, placement_preference, authored_by_json, created_at)
                   VALUES ('task-1', 'workspace-1', 1, '[]', 'Test objective', '[]', '[]', '[]', '[]',
                   '[]', '[]', '{"mode":"DISABLED","triggers":[],"fallback_agent_binding_ids":[],"max_lead_changes":0}',
                   '[]', 'AUTO', '{"kind":"USER","principal_id":"owner-1"}', '2026-10-17T11:00:00Z');
                 INSERT INTO agent_sessions(agent_session_id, workspace_id, scope_kind, task_id,
                   task_spec_revision, attempt_id, agent_binding_id, endpoint_id, runtime_id,
                   runtime_incarnation_id, status, started_at, last_event_at, closed_at, version)
                   VALUES ('planning-session-1', 'workspace-1', 'TASK_PLANNING', 'task-1', 1, NULL,
                   'binding-1', 'endpoint-1', 'runtime-1', 'incarnation-1', 'CLOSED',
                   '2026-10-17T11:00:00Z', '2026-10-17T11:00:00Z', '2026-10-17T11:01:00Z', 2);
                 INSERT INTO plan_revisions(task_id, revision, task_spec_revision, produced_by_agent_session_id,
                   produced_by_attempt_id, steps_json, reason_for_revision, created_at)
                   VALUES ('task-1', 1, 1, 'planning-session-1', NULL, '[]', NULL, '2026-10-17T11:00:00Z');
                 INSERT INTO steps(step_id, task_id, plan_revision, logical_key, title, objective,
                   dependencies_json, required_capabilities_json, acceptance_criteria_json, status,
                   current_attempt_id, created_at, updated_at, version)
                   VALUES ('step-1', 'task-1', 1, 'step-1', 'Test step', 'Test objective', '[]', '[]',
                   '[]', 'COMPLETED', 'child-1', '2026-10-17T11:00:00Z', '2026-10-17T12:00:00Z', 2);
                 INSERT INTO delegation_profiles(delegation_profile_id, workspace_id, agent_binding_id,
                   name, name_key, current_revision, status, created_at, updated_at, version)
                   VALUES ('delegation-profile-1', 'workspace-1', 'binding-1', 'Test profile',
                   'test profile', 1, 'ENABLED', '2026-10-17T11:00:00Z', '2026-10-17T11:00:00Z', 1);
                 INSERT INTO delegation_profile_revisions(delegation_profile_id, revision, workspace_id,
                   name, name_key, routing_description, session_options_json, required_features_json,
                   preferred_features_json, enforced_policy_json, optimization_preference,
                   max_concurrency, max_host_delegation_depth, latency_class, environment_policy_json,
                   native_delegation_policy, warm_policy_json, authored_by_json, created_at)
                   VALUES ('delegation-profile-1', 1, 'workspace-1', 'Test profile', 'test profile',
                   'test routing', '{}', '[]', '[]', '{}', 'BALANCED', 1, 0, 'STANDARD', '{}',
                   'DENY_IF_SUPPORTED', '{"host":"COLD","native_session":"CLOSE_ON_SETTLE","capability_hosts":"COLD","browser_environment":"COLD","local_model":"PROVIDER_DEFAULT","ttl_ms":null,"max_memory_bytes":null,"max_idle_cost":null,"triggers":[]}', '{"kind":"USER","principal_id":"owner-1"}',
                   '2026-10-17T11:00:00Z');
                 INSERT INTO attempts(attempt_id, task_id, step_id, parent_attempt_id, agent_binding_id,
                   delegation_profile_id, delegation_profile_revision, agent_session_id, runtime_id,
                   runtime_incarnation_id, environment_id, capability_grant_ids_json, execution_lease_id,
                   failover_class, checkpoint_ref_json, status, failure_json, started_at, settled_at,
                   created_at, version)
                   VALUES ('parent-1', 'task-1', 'step-1', NULL, 'binding-1', NULL, NULL, 'parent-session-1',
                   'runtime-1', 'incarnation-1', 'environment-1', '[]', NULL, 'SAFE_PORTABLE', NULL,
                   'COMPLETED', NULL, '2026-10-17T11:00:00Z', '2026-10-17T11:30:00Z',
                   '2026-10-17T11:00:00Z', 2);
                 INSERT INTO attempts(attempt_id, task_id, step_id, parent_attempt_id, agent_binding_id,
                   delegation_profile_id, delegation_profile_revision, agent_session_id, runtime_id,
                   runtime_incarnation_id, environment_id, capability_grant_ids_json, execution_lease_id,
                   failover_class, checkpoint_ref_json, status, failure_json, started_at, settled_at,
                   created_at, version)
                   VALUES ('child-1', 'task-1', 'step-1', 'parent-1', 'binding-1',
                   'delegation-profile-1', 1, 'session-1', 'runtime-1', 'incarnation-1',
                   'environment-1', '[]', NULL, 'SAFE_PORTABLE', NULL, 'COMPLETED', NULL,
                   '2026-10-17T11:30:00Z', '2026-10-17T12:00:00Z', '2026-10-17T11:30:00Z', 2);
                 INSERT INTO agent_sessions(agent_session_id, workspace_id, scope_kind, task_id,
                   task_spec_revision, attempt_id, agent_binding_id, endpoint_id, runtime_id,
                   runtime_incarnation_id, status, started_at, last_event_at, closed_at, version)
                   VALUES ('session-1', 'workspace-1', 'ATTEMPT_EXECUTION', 'task-1', 1, 'child-1',
                   'binding-1', 'endpoint-1', 'runtime-1', 'incarnation-1', 'CLOSED',
                   '2026-10-17T11:30:00Z', '2026-10-17T12:00:00Z', '2026-10-17T12:00:00Z', 2);
                 INSERT INTO agent_sessions(agent_session_id, workspace_id, scope_kind, task_id,
                   task_spec_revision, attempt_id, agent_binding_id, endpoint_id, runtime_id,
                   runtime_incarnation_id, status, started_at, last_event_at, closed_at, version)
                   VALUES ('parent-session-1', 'workspace-1', 'ATTEMPT_EXECUTION', 'task-1', 1, 'parent-1',
                   'binding-1', 'endpoint-1', 'runtime-1', 'incarnation-1', 'CLOSED',
                   '2026-10-17T11:00:00Z', '2026-10-17T11:30:00Z', '2026-10-17T11:30:00Z', 2);
                 COMMIT;"#,
            )
            .expect("seed linked terminal child and AgentSession");
    }

    #[test]
    fn exact_replay_returns_original_event_and_publisher_identity() {
        let mut connection = result_database();
        let transaction = connection.transaction().expect("transaction");
        let replay = find_replay(&transaction, &commit())
            .expect("lookup replay")
            .expect("existing result");
        assert!(replay.replayed);
        assert_eq!(replay.event.event_id, "event-1");
        assert_eq!(replay.published_by_agent_session_id, "session-1");
        assert_eq!(replay.runtime_id, "runtime-1");
        assert_eq!(replay.runtime_incarnation_id, "incarnation-1");
        transaction.commit().expect("commit");
    }

    #[test]
    fn replay_rejects_a_different_publisher_even_when_digest_matches() {
        for change in ["session", "runtime", "incarnation"] {
            let mut connection = result_database();
            let transaction = connection.transaction().expect("transaction");
            let mut retry = commit();
            match change {
                "session" => retry.agent_session_id = "session-2".to_owned(),
                "runtime" => retry.runtime_id = "runtime-2".to_owned(),
                _ => retry.runtime_incarnation_id = "incarnation-2".to_owned(),
            }
            assert!(find_replay(&transaction, &retry).is_err(), "{change}");
        }
    }

    #[test]
    fn replay_rejects_a_changed_result_digest() {
        let mut connection = result_database();
        let transaction = connection.transaction().expect("transaction");
        let mut retry = commit();
        retry.envelope_digest = format!("sha256:{}", "0".repeat(64));
        assert!(find_replay(&transaction, &retry).is_err());
    }

    #[test]
    fn event_integrity_rejects_unredacted_or_mismatched_payload() {
        let mut connection = result_database();
        let transaction = connection.transaction().expect("transaction");
        let result = load_result(&transaction, "child-1")
            .expect("load result")
            .expect("result row");
        let mut event = load_event(&transaction, &result.event_id)
            .expect("load event")
            .expect("event row");
        event.payload["summary"] = Value::String("raw result leaked".to_owned());
        assert!(validate_stored_event(&event, &result).is_err());
    }

    #[test]
    fn event_integrity_rejects_wrong_event_identity_or_missing_origin_sequence() {
        let mut connection = result_database();
        let transaction = connection.transaction().expect("transaction");
        let result = load_result(&transaction, "child-1")
            .expect("load result")
            .expect("result row");
        let mut event = load_event(&transaction, &result.event_id)
            .expect("load event")
            .expect("event row");
        event.event_id = "other-event".to_owned();
        assert!(validate_stored_event(&event, &result).is_err());
        event.event_id = result.event_id.clone();
        event.origin_sequence = 0;
        assert!(validate_stored_event(&event, &result).is_err());
        transaction.commit().expect("commit");
    }

    #[test]
    fn replay_and_owner_reads_require_the_exact_canonical_envelope_blob() {
        let mut connection = result_database();
        let transaction = connection.transaction().expect("transaction");
        let replay = find_replay(&transaction, &commit())
            .expect("lookup replay")
            .expect("existing result");
        let bytes = canonical_json(&replay.envelope_json).expect("canonical envelope");
        verify_result_blob(&FixedBlobStore(bytes.clone()), &replay).expect("valid blob");
        assert!(verify_result_blob(&FixedBlobStore(b"different bytes".to_vec()), &replay).is_err());
    }

    #[test]
    fn result_status_requires_a_terminal_attempt_with_the_defined_mapping() {
        assert!(is_publishable_terminal_attempt_status("ABANDONED"));
        assert!(!is_publishable_terminal_attempt_status("RUNNING"));
        assert!(result_status_matches_attempt("COMPLETED", "COMPLETED"));
        assert!(result_status_matches_attempt("FAILED", "FAILED"));
        assert!(result_status_matches_attempt("BLOCKED", "FAILED"));
        assert!(result_status_matches_attempt("CANCELLED", "CANCELLED"));
        assert!(result_status_matches_attempt("CANCELLED", "ABANDONED"));
        assert!(!result_status_matches_attempt("BLOCKED", "COMPLETED"));
        assert!(!result_status_matches_attempt("COMPLETED", "FAILED"));
        assert!(!result_status_matches_attempt("FAILED", "ABANDONED"));
        assert!(!result_status_matches_attempt("CANCELLED", "FAILED"));
    }

    #[test]
    fn stored_envelope_must_use_the_canonical_json_encoding() {
        let connection = result_database();
        connection
            .execute(
                "UPDATE delegation_results SET envelope_json = ?1 WHERE child_attempt_id = 'child-1'",
                [serde_json::to_string_pretty(&envelope()).expect("pretty JSON")],
            )
            .expect("mutate test row to noncanonical bytes");
        assert!(matches!(
            load_result(&connection, "child-1"),
            Err(StoreError::Integrity(_))
        ));
    }

    #[test]
    fn production_store_commits_replays_scopes_reads_and_rolls_back_event_on_result_failure() {
        let directory = tempfile::tempdir().expect("temporary production store directory");
        let blobs = Arc::new(MemoryBlobStore::default());
        let store = open_production_store(&directory, blobs.clone());
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: "workspace-1".to_owned(),
                name: "Delegation test".to_owned(),
                owner_principal_id: "owner-1".to_owned(),
                event: EventContext {
                    event_id: "workspace-event-1".to_owned(),
                    origin_runtime_id: "runtime-local".to_owned(),
                    hlc_timestamp: "2026-10-17T10:00:00Z".to_owned(),
                    correlation_id: "workspace-create-1".to_owned(),
                    causation_id: None,
                    recorded_at: "2026-10-17T10:00:00Z".to_owned(),
                },
            })
            .expect("create real workspace");
        drop(store);

        let database_path = directory.path().join("state/state.sqlite3");
        seed_terminal_child(&database_path);
        let store = open_production_store(&directory, blobs.clone());
        let connection = Connection::open(&database_path).expect("inject test failure");
        connection
            .execute_batch(
                "CREATE TRIGGER test_abort_delegation_result
                 BEFORE INSERT ON delegation_results
                 BEGIN SELECT RAISE(ABORT, 'INJECTED_RESULT_ROW_FAILURE'); END;",
            )
            .expect("install rollback failure trigger");
        drop(connection);

        let adapter = SqliteDelegationResultStore::new(store.clone());
        assert!(adapter.publish_delegation_result(commit()).is_err());
        let connection = Connection::open(&database_path).expect("inspect rolled-back store");
        let orphan_event_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE event_id = 'event-1'",
                [],
                |row| row.get(0),
            )
            .expect("count result events");
        let result_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM delegation_results", [], |row| {
                row.get(0)
            })
            .expect("count result rows");
        assert_eq!(
            orphan_event_count, 0,
            "failed row insert rolled back its event"
        );
        assert_eq!(result_count, 0);
        connection
            .execute_batch("DROP TRIGGER test_abort_delegation_result")
            .expect("remove injected failure");
        drop(connection);

        let first = adapter
            .publish_delegation_result(commit())
            .expect("commit real terminal child result");
        assert!(!first.replayed);
        assert_eq!(first.event.event_type, RESULT_EVENT_TYPE);
        assert!(first.event.payload.get("summary").is_none());
        assert!(first.event.payload.get("blockers").is_none());

        let replay = adapter
            .publish_delegation_result(commit())
            .expect("exact same publisher replay");
        assert!(replay.replayed);
        assert_eq!(replay.event.event_id, first.event.event_id);

        let mut changed = commit();
        changed.envelope_json["summary"] = Value::String("changed report".to_owned());
        let changed_bytes =
            canonical_json(&changed.envelope_json).expect("canonical changed result");
        changed.envelope_digest = digest(&changed_bytes);
        changed.event.payload["result_digest"] = Value::String(changed.envelope_digest.clone());
        assert!(matches!(
            adapter.publish_delegation_result(changed),
            Err(StoreError::Conflict { .. })
        ));

        let mut different_publisher = commit();
        different_publisher.agent_session_id = "session-other".to_owned();
        assert!(matches!(
            adapter.publish_delegation_result(different_publisher),
            Err(StoreError::Conflict { .. })
        ));

        let owner_read = adapter
            .get_delegation_result(DelegationResultReadQuery {
                owner_principal_id: "owner-1".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                task_id: "task-1".to_owned(),
                parent_attempt_id: "parent-1".to_owned(),
                child_attempt_id: "child-1".to_owned(),
                task_spec_revision: 1,
            })
            .expect("owner read")
            .expect("owner result exists");
        assert_eq!(owner_read.envelope_digest, first.envelope_digest);
        assert_eq!(owner_read.envelope_json, first.envelope_json);
        for (task_id, parent_attempt_id, task_spec_revision) in [
            ("other-task", "parent-1", 1),
            ("task-1", "other-parent", 1),
            ("task-1", "parent-1", 2),
        ] {
            let scoped = adapter
                .get_delegation_result(DelegationResultReadQuery {
                    owner_principal_id: "owner-1".to_owned(),
                    workspace_id: "workspace-1".to_owned(),
                    task_id: task_id.to_owned(),
                    parent_attempt_id: parent_attempt_id.to_owned(),
                    child_attempt_id: "child-1".to_owned(),
                    task_spec_revision,
                })
                .expect("mismatched parent channel is non-disclosing");
            assert!(
                scoped.is_none(),
                "{task_id}/{parent_attempt_id}/{task_spec_revision}"
            );
        }
        let foreign_read = adapter
            .get_delegation_result(DelegationResultReadQuery {
                owner_principal_id: "other-owner".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                task_id: "task-1".to_owned(),
                parent_attempt_id: "parent-1".to_owned(),
                child_attempt_id: "child-1".to_owned(),
                task_spec_revision: 1,
            })
            .expect("foreign read is denied without disclosure");
        assert!(foreign_read.is_none());
        drop(adapter);
        drop(store);

        let connection = Connection::open(database_path).expect("inspect committed store");
        let event_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE event_id = 'event-1'",
                [],
                |row| row.get(0),
            )
            .expect("count committed event");
        assert_eq!(event_count, 1, "exact replay preserves one original event");
    }
}
