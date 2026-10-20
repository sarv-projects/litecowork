//! Durable, owner-authenticated UserRequest response and dismissal transactions.
//!
//! This module records immutable ordinary input and source lifecycle only. It never
//! dispatches an external provider input or resumes a Conversation, Task, or Invocation.

use super::*;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::mpsc;
use storage_core::{
    DismissUserRequestCommit, RespondUserRequestCommit, StoreError, USER_REQUEST_ID_MAX_BYTES,
    USER_REQUEST_IDEMPOTENCY_KEY_MAX_BYTES, USER_REQUEST_RESPONSE_MAX_BYTES,
    UserRequestResolutionActor, UserRequestResolutionReceipt, UserRequestResolutionStore,
    UserRequestResponseInput,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const MAX_SCHEMA_BYTES: usize = 64 * 1024;
const MAX_SCHEMA_DEPTH: usize = 32;
const MAX_SCHEMA_ARRAY_ITEMS: usize = 1024;
const MAX_SCHEMA_OBJECT_PROPERTIES: usize = 512;
const MAX_RESPONSE_VALUE_DEPTH: usize = 32;
const USER_REQUEST_STATE_MEDIA_TYPE: &str = "application/vnd.litecowork.user-request-state+json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UserRequestAggregateState {
    request_id: String,
    workspace_id: String,
    status: String,
    version: u64,
    resolved_at: String,
    resolved_by: UserRequestResolutionActor,
    response_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<Value>,
}

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

#[derive(Clone)]
pub struct SqliteUserRequestResolutionStore {
    store: SqliteWorkspaceStore,
}

impl SqliteUserRequestResolutionStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store }
    }

    fn run<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::UserRequestResolutionOperation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }
}

impl UserRequestResolutionStore for SqliteUserRequestResolutionStore {
    fn respond_to_user_request(
        &self,
        mut command: RespondUserRequestCommit,
    ) -> Result<UserRequestResolutionReceipt, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_response_command(&command)?;
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| respond_transaction(connection, blobs.as_ref(), command))
    }

    fn dismiss_user_request(
        &self,
        mut command: DismissUserRequestCommit,
    ) -> Result<UserRequestResolutionReceipt, StoreError> {
        command.event.recorded_at = canonicalize_utc_timestamp(&command.event.recorded_at)?;
        validate_dismiss_command(&command)?;
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| dismiss_transaction(connection, blobs.as_ref(), command))
    }
}

fn validate_response_command(command: &RespondUserRequestCommit) -> Result<(), StoreError> {
    validate_common(
        &command.owner_principal_id,
        &command.actor,
        &command.workspace_id,
        &command.request_id,
        &command.idempotency_key,
        command.expected_version,
        &command.event,
    )?;
    validate_id(&command.response_id, "UserRequest response ID")?;
    match &command.input {
        UserRequestResponseInput::Value(response) => {
            let response_bytes = canonical_json(response)?;
            if response_bytes.len() > USER_REQUEST_RESPONSE_MAX_BYTES {
                return Err(invalid("USER_REQUEST_RESPONSE_TOO_LARGE"));
            }
            validate_json_value_depth(response, 0)?;
        }
        UserRequestResponseInput::ChoiceId(choice_id) => {
            validate_id(choice_id, "UserRequest choice ID")?
        }
    }
    Ok(())
}

fn validate_dismiss_command(command: &DismissUserRequestCommit) -> Result<(), StoreError> {
    validate_common(
        &command.owner_principal_id,
        &command.actor,
        &command.workspace_id,
        &command.request_id,
        &command.idempotency_key,
        command.expected_version,
        &command.event,
    )
}

fn validate_common(
    owner: &str,
    actor: &UserRequestResolutionActor,
    workspace: &str,
    request: &str,
    idempotency_key: &str,
    expected_version: u64,
    event: &EventDraft,
) -> Result<(), StoreError> {
    validate_id(owner, "owner principal ID")?;
    validate_id(workspace, "Workspace ID")?;
    validate_id(request, "UserRequest ID")?;
    if actor.principal_id != owner || actor.kind != "USER" {
        return Err(invalid("UserRequest response actor is invalid"));
    }
    if idempotency_key.is_empty()
        || idempotency_key.len() > USER_REQUEST_IDEMPOTENCY_KEY_MAX_BYTES
        || !idempotency_key
            .bytes()
            .all(|byte| (0x20..=0x7e).contains(&byte))
    {
        return Err(invalid("UserRequest idempotency key is invalid"));
    }
    if expected_version == 0 || expected_version >= i64::MAX as u64 {
        return Err(invalid("UserRequest version is invalid"));
    }
    if event.workspace_id != workspace
        || event.entity_type != "user_request"
        || event.entity_id != request
        || event.entity_revision != expected_version.saturating_add(1)
        || event.event_type != "user.request.resolved.v1"
        || event.schema_version != 1
    {
        return Err(invalid("UserRequest event identity is inconsistent"));
    }
    for (name, value) in [
        ("event ID", event.event_id.as_str()),
        ("Runtime ID", event.origin_runtime_id.as_str()),
        ("correlation ID", event.correlation_id.as_str()),
    ] {
        validate_id(value, name)?;
    }
    if event.hlc_timestamp.trim().is_empty() || event.hlc_timestamp.len() > 128 {
        return Err(invalid("UserRequest event clock is invalid"));
    }
    Ok(())
}

fn respond_transaction(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    command: RespondUserRequestCommit,
) -> Result<UserRequestResolutionReceipt, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_owner(&tx, &command.workspace_id, &command.owner_principal_id)?;
    let fingerprint = response_fingerprint(&command)?;
    if let Some(replay) = find_replay(
        &tx,
        &command.actor.principal_id,
        &command.idempotency_key,
        &fingerprint,
    )? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replay);
    }

    let current = load_request_for_mutation(&tx, &command.workspace_id, &command.request_id)?
        .ok_or(StoreError::NotFound)?;
    check_pending_and_version(
        &current,
        command.expected_version,
        &command.event.recorded_at,
    )?;
    let response = resolve_response_input(&current, &command.input)?;
    let resolved_bytes = canonical_json(&response)?;
    if resolved_bytes.len() > USER_REQUEST_RESPONSE_MAX_BYTES {
        return Err(invalid("USER_REQUEST_RESPONSE_TOO_LARGE"));
    }
    validate_json_value_depth(&response, 0)?;
    validate_response(&current, &response)?;
    validate_response_parent(&tx, &command.workspace_id, &current)?;

    let response_bytes = canonical_json(&response)?;
    let response_digest = digest(&response_bytes);
    let actor_json = canonical_json(&command.actor)?;
    let actor_json = String::from_utf8(actor_json)
        .map_err(|_| StoreError::Integrity("UserRequest actor encoding failed".to_owned()))?;
    let response_json = String::from_utf8(response_bytes.clone())
        .map_err(|_| StoreError::Invalid("UserRequest response encoding failed".to_owned()))?;
    tx.execute(
        "INSERT INTO user_request_responses(response_id, request_id, response_json,
            response_digest, responded_by_json, response_channel_binding_id,
            response_provider_event_id, responded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6)",
        params![
            command.response_id,
            command.request_id,
            response_json,
            response_digest,
            actor_json,
            command.event.recorded_at,
        ],
    )
    .map_err(map_database_error)?;

    let version = command
        .expected_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Integrity("UserRequest version exhausted".to_owned()))?;
    let updated = tx
        .execute(
            "UPDATE user_requests
         SET status='ANSWERED', resolved_at=?1, resolved_by_json=?2,
             response_digest=?3, version=?4
         WHERE workspace_id=?5 AND request_id=?6 AND status='PENDING' AND version=?7",
            params![
                command.event.recorded_at,
                actor_json,
                response_digest,
                to_sql_i64(version, "UserRequest version")?,
                command.workspace_id,
                command.request_id,
                to_sql_i64(command.expected_version, "expected UserRequest version")?,
            ],
        )
        .map_err(map_database_error)?;
    if updated != 1 {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: None,
        });
    }

    let state = UserRequestAggregateState {
        request_id: command.request_id.clone(),
        workspace_id: command.workspace_id.clone(),
        status: "ANSWERED".to_owned(),
        version,
        resolved_at: command.event.recorded_at.clone(),
        resolved_by: command.actor.clone(),
        response_digest: Some(response_digest.clone()),
        response: Some(response),
    };
    let state_ref = put_user_request_state(blobs, &command.workspace_id, &state, version)?;
    let event = make_resolution_event(
        &tx,
        command.event,
        "ANSWERED",
        &command.actor,
        Some(&response_digest),
        version,
        &state_ref,
    )?;
    let receipt = UserRequestResolutionReceipt {
        request_id: command.request_id.clone(),
        workspace_id: command.workspace_id.clone(),
        status: "ANSWERED".to_owned(),
        version,
        resolved_at: event.recorded_at.clone(),
        resolved_by: command.actor.clone(),
        response_digest: Some(response_digest),
        event,
        replayed: false,
    };
    save_replay(
        &tx,
        &command.actor.principal_id,
        &command.idempotency_key,
        &fingerprint,
        &receipt,
        &receipt.resolved_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(receipt)
}

fn dismiss_transaction(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    command: DismissUserRequestCommit,
) -> Result<UserRequestResolutionReceipt, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_owner(&tx, &command.workspace_id, &command.owner_principal_id)?;
    let fingerprint = dismiss_fingerprint(&command)?;
    if let Some(replay) = find_replay(
        &tx,
        &command.actor.principal_id,
        &command.idempotency_key,
        &fingerprint,
    )? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replay);
    }

    let current = load_request_for_mutation(&tx, &command.workspace_id, &command.request_id)?
        .ok_or(StoreError::NotFound)?;
    check_pending_and_version(
        &current,
        command.expected_version,
        &command.event.recorded_at,
    )?;
    validate_response_parent(&tx, &command.workspace_id, &current)?;
    let version = command
        .expected_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Integrity("UserRequest version exhausted".to_owned()))?;
    let actor_json = canonical_json(&command.actor)?;
    let actor_json = String::from_utf8(actor_json)
        .map_err(|_| StoreError::Integrity("UserRequest actor encoding failed".to_owned()))?;
    let updated = tx
        .execute(
            "UPDATE user_requests
         SET status='DISMISSED', resolved_at=?1, resolved_by_json=?2, version=?3
         WHERE workspace_id=?4 AND request_id=?5 AND status='PENDING' AND version=?6",
            params![
                command.event.recorded_at,
                actor_json,
                to_sql_i64(version, "UserRequest version")?,
                command.workspace_id,
                command.request_id,
                to_sql_i64(command.expected_version, "expected UserRequest version")?,
            ],
        )
        .map_err(map_database_error)?;
    if updated != 1 {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: None,
        });
    }
    let state = UserRequestAggregateState {
        request_id: command.request_id.clone(),
        workspace_id: command.workspace_id.clone(),
        status: "DISMISSED".to_owned(),
        version,
        resolved_at: command.event.recorded_at.clone(),
        resolved_by: command.actor.clone(),
        response_digest: None,
        response: None,
    };
    let state_ref = put_user_request_state(blobs, &command.workspace_id, &state, version)?;
    let event = make_resolution_event(
        &tx,
        command.event,
        "DISMISSED",
        &command.actor,
        None,
        version,
        &state_ref,
    )?;
    let receipt = UserRequestResolutionReceipt {
        request_id: command.request_id.clone(),
        workspace_id: command.workspace_id.clone(),
        status: "DISMISSED".to_owned(),
        version,
        resolved_at: event.recorded_at.clone(),
        resolved_by: command.actor.clone(),
        response_digest: None,
        event,
        replayed: false,
    };
    save_replay(
        &tx,
        &command.actor.principal_id,
        &command.idempotency_key,
        &fingerprint,
        &receipt,
        &receipt.resolved_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(receipt)
}

#[derive(Clone)]
struct CurrentRequest {
    status: String,
    kind: String,
    interaction_mode: String,
    response_schema_json: Option<String>,
    choices_json: String,
    expires_at: Option<String>,
    version: u64,
    conversation_id: Option<String>,
    conversation_turn_id: Option<String>,
    task_id: Option<String>,
    attempt_id: Option<String>,
    agent_session_id: String,
}

fn authorize_owner(
    tx: &Transaction<'_>,
    workspace_id: &str,
    owner_principal_id: &str,
) -> Result<(), StoreError> {
    let active: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces
             WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
            params![workspace_id, owner_principal_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if active {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

fn load_request_for_mutation(
    tx: &Transaction<'_>,
    workspace_id: &str,
    request_id: &str,
) -> Result<Option<CurrentRequest>, StoreError> {
    let raw: Option<(
        String,
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
    )> = tx.query_row(
        "SELECT status, kind, interaction_mode, response_schema_json, choices_json, expires_at, version,
                conversation_id, conversation_turn_id, task_id, attempt_id, agent_session_id
         FROM user_requests WHERE workspace_id=?1 AND request_id=?2",
        params![workspace_id, request_id],
        |row| Ok((
            row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?,
            row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
        )),
    ).optional().map_err(map_database_error)?;
    raw.map(
        |(
            status,
            kind,
            interaction_mode,
            response_schema_json,
            choices_json,
            expires_at,
            version,
            conversation_id,
            conversation_turn_id,
            task_id,
            attempt_id,
            agent_session_id,
        )| {
            if version <= 0
                || !matches!(
                    status.as_str(),
                    "PENDING" | "ANSWERED" | "DISMISSED" | "EXPIRED" | "CANCELLED"
                )
                || !matches!(
                    kind.as_str(),
                    "QUESTION" | "DECISION" | "RESOURCE_SELECTION" | "EXTERNAL_AUTHORIZATION"
                )
                || !matches!(interaction_mode.as_str(), "FORM" | "EXTERNAL_URL")
                || expires_at
                    .as_deref()
                    .is_some_and(|value| !valid_timestamp(value))
            {
                return Err(StoreError::Integrity(
                    "stored UserRequest state is invalid".to_owned(),
                ));
            }
            Ok(CurrentRequest {
                status,
                kind,
                interaction_mode,
                response_schema_json,
                choices_json,
                expires_at,
                version: u64::try_from(version).map_err(|_| {
                    StoreError::Integrity("UserRequest version is invalid".to_owned())
                })?,
                conversation_id,
                conversation_turn_id,
                task_id,
                attempt_id,
                agent_session_id,
            })
        },
    )
    .transpose()
}

fn check_pending_and_version(
    current: &CurrentRequest,
    expected_version: u64,
    at: &str,
) -> Result<(), StoreError> {
    if current.version != expected_version {
        return Err(StoreError::Conflict {
            expected: Some(expected_version),
            actual: Some(current.version),
        });
    }
    if let Some(expires) = current.expires_at.as_deref() {
        let expires = OffsetDateTime::parse(expires, &Rfc3339).map_err(|_| {
            StoreError::Integrity("UserRequest expiry timestamp is invalid".to_owned())
        })?;
        let now = OffsetDateTime::parse(at, &Rfc3339).map_err(|_| {
            StoreError::Integrity("UserRequest action timestamp is invalid".to_owned())
        })?;
        if expires <= now {
            return Err(invalid("USER_REQUEST_EXPIRED"));
        }
    }
    if current.status != "PENDING" {
        return Err(StoreError::Conflict {
            expected: Some(expected_version),
            actual: Some(current.version),
        });
    }
    Ok(())
}

fn validate_response_parent(
    tx: &Transaction<'_>,
    workspace_id: &str,
    request: &CurrentRequest,
) -> Result<(), StoreError> {
    let session: Option<(
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = tx
        .query_row(
            "SELECT workspace_id, scope_kind, conversation_id, conversation_turn_id, task_id
             FROM agent_sessions WHERE agent_session_id=?1",
            [&request.agent_session_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((session_workspace, scope_kind, session_conversation, session_turn, session_task)) =
        session
    else {
        return Err(StoreError::NotFound);
    };
    if session_workspace != workspace_id {
        return Err(StoreError::NotFound);
    }

    if let (Some(conversation_id), Some(turn_id)) =
        (&request.conversation_id, &request.conversation_turn_id)
    {
        let turn: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT status, agent_session_id FROM conversation_turns
                 WHERE conversation_id=?1 AND turn_id=?2",
                params![conversation_id, turn_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_database_error)?;
        let Some((status, turn_session)) = turn else {
            return Err(StoreError::NotFound);
        };
        if scope_kind.as_deref() != Some("CONVERSATION")
            || session_conversation.as_deref() != Some(conversation_id)
            || session_turn.as_deref() != Some(turn_id)
            || turn_session.as_deref() != Some(request.agent_session_id.as_str())
        {
            return Err(StoreError::Integrity(
                "UserRequest Conversation source identity changed".to_owned(),
            ));
        }
        if status != "WAITING_USER" {
            return Err(StoreError::Conflict {
                expected: Some(request.version),
                actual: Some(request.version),
            });
        }
        return Ok(());
    }

    let task_id = request.task_id.as_deref().ok_or_else(|| {
        StoreError::Integrity("UserRequest source has no parent identity".to_owned())
    })?;
    let task_status: Option<String> = tx
        .query_row(
            "SELECT status FROM tasks WHERE task_id=?1 AND workspace_id=?2",
            params![task_id, workspace_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some(task_status) = task_status else {
        return Err(StoreError::NotFound);
    };
    let eligible = matches!(
        task_status.as_str(),
        "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "NEEDS_USER" | "INCOMPLETE" | "PAUSED"
    );
    if !eligible {
        return Err(StoreError::Conflict {
            expected: Some(request.version),
            actual: Some(request.version),
        });
    }

    match request.attempt_id.as_deref() {
        None if scope_kind.as_deref() == Some("TASK_PLANNING")
            && session_task.as_deref() == Some(task_id) =>
        {
            Ok(())
        }
        Some(attempt_id)
            if scope_kind.as_deref() == Some("ATTEMPT_EXECUTION")
                && session_task.as_deref() == Some(task_id) =>
        {
            let attempt_matches: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM attempts
                 WHERE task_id=?1 AND attempt_id=?2 AND agent_session_id=?3)",
                    params![task_id, attempt_id, request.agent_session_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if attempt_matches {
                Ok(())
            } else {
                Err(StoreError::NotFound)
            }
        }
        _ => Err(StoreError::Integrity(
            "UserRequest Task source identity changed".to_owned(),
        )),
    }
}

fn validate_response(current: &CurrentRequest, response: &Value) -> Result<(), StoreError> {
    match current.interaction_mode.as_str() {
        "EXTERNAL_URL" => {
            if current.kind != "EXTERNAL_AUTHORIZATION" || current.response_schema_json.is_some() {
                return Err(StoreError::Integrity(
                    "external UserRequest has an invalid contract".to_owned(),
                ));
            }
            let object = response
                .as_object()
                .ok_or_else(|| invalid("INVALID_ARGUMENT"))?;
            if object.len() != 1
                || !matches!(
                    object.get("action").and_then(Value::as_str),
                    Some("accept" | "decline" | "cancel")
                )
            {
                return Err(invalid("INVALID_ARGUMENT"));
            }
            Ok(())
        }
        "FORM" => {
            if current.kind == "EXTERNAL_AUTHORIZATION" {
                return Err(StoreError::Integrity(
                    "external authorization is not a form".to_owned(),
                ));
            }
            if let Some(schema_json) = current.response_schema_json.as_deref() {
                if schema_json.len() > MAX_SCHEMA_BYTES {
                    return Err(invalid("PROVIDER_INPUT_UNSUPPORTED"));
                }
                let schema: Value = serde_json::from_str(schema_json).map_err(|_| {
                    StoreError::Integrity(
                        "stored UserRequest response schema is malformed".to_owned(),
                    )
                })?;
                if schema_depth(&schema, 0).is_err() || schema_has_sensitive_fields(&schema) {
                    return Err(invalid("SENSITIVE_INPUT_UNSUPPORTED"));
                }
                validate_schema(&schema, response, 0).map_err(|reason| match reason {
                    SchemaFailure::Invalid => invalid("INVALID_ARGUMENT"),
                    SchemaFailure::Unsupported => invalid("PROVIDER_INPUT_UNSUPPORTED"),
                })?;
            }
            if value_has_sensitive_content(response, 0) {
                return Err(invalid("SENSITIVE_INPUT_UNSUPPORTED"));
            }
            Ok(())
        }
        _ => Err(StoreError::Integrity(
            "stored UserRequest interaction mode is invalid".to_owned(),
        )),
    }
}

fn resolve_response_input(
    current: &CurrentRequest,
    input: &UserRequestResponseInput,
) -> Result<Value, StoreError> {
    match input {
        UserRequestResponseInput::Value(value) => Ok(value.clone()),
        UserRequestResponseInput::ChoiceId(choice_id) => {
            if current.interaction_mode != "FORM"
                || !matches!(current.kind.as_str(), "DECISION" | "RESOURCE_SELECTION")
            {
                return Err(invalid("INVALID_ARGUMENT"));
            }
            resolve_choice_value(&current.choices_json, choice_id)
        }
    }
}

fn resolve_choice_value(choices_json: &str, choice_id: &str) -> Result<Value, StoreError> {
    if choices_json.len() > storage_core::USER_REQUEST_CHOICES_MAX_BYTES {
        return Err(invalid("PROVIDER_INPUT_UNSUPPORTED"));
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct StoredChoice {
        choice_id: String,
        label: String,
        #[serde(default)]
        description: Option<String>,
        value: Value,
    }

    let choices: Vec<StoredChoice> = serde_json::from_str(choices_json).map_err(|_| {
        StoreError::Integrity("stored UserRequest choices are malformed".to_owned())
    })?;
    if choices.len() > 500 {
        return Err(StoreError::Integrity(
            "stored UserRequest choices exceed their bound".to_owned(),
        ));
    }
    let mut found = None;
    let mut seen = std::collections::HashSet::new();
    for choice in choices {
        validate_id(&choice.choice_id, "stored UserRequest choice ID").map_err(|_| {
            StoreError::Integrity("stored UserRequest choice ID is invalid".to_owned())
        })?;
        if !seen.insert(choice.choice_id.clone()) {
            return Err(StoreError::Integrity(
                "stored UserRequest choice IDs are duplicated".to_owned(),
            ));
        }
        if choice.label.trim().is_empty()
            || choice.label.len() > 16_384
            || choice
                .description
                .as_ref()
                .is_some_and(|text| text.len() > 16_384)
        {
            return Err(StoreError::Integrity(
                "stored UserRequest choice metadata is invalid".to_owned(),
            ));
        }
        if choice.choice_id == choice_id {
            found = Some(choice.value);
        }
    }
    found.ok_or_else(|| invalid("INVALID_ARGUMENT"))
}

fn make_resolution_event(
    tx: &Transaction<'_>,
    mut draft: EventDraft,
    status: &str,
    actor: &UserRequestResolutionActor,
    response_digest: Option<&str>,
    version: u64,
    state_ref: &AggregateStateRef,
) -> Result<DomainEvent, StoreError> {
    if !matches!(status, "ANSWERED" | "DISMISSED")
        || draft.entity_revision != version
        || state_ref.entity_revision != version
    {
        return Err(StoreError::Invalid(
            "UserRequest resolution event version is inconsistent".to_owned(),
        ));
    }
    let mut payload = json!({
        "request_id": draft.entity_id.clone(),
        "from": "PENDING",
        "to": status,
        "resolved_by": actor,
        "aggregate_version": version,
    });
    if let Some(digest) = response_digest {
        payload["response_digest"] = Value::String(digest.to_owned());
    }
    draft.payload = payload;
    insert_domain_event(tx, &draft, state_ref)
}

fn put_user_request_state(
    blobs: &dyn BlobStore,
    workspace_id: &str,
    state: &UserRequestAggregateState,
    version: u64,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(state)?;
    let blob = blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &bytes,
        USER_REQUEST_STATE_MEDIA_TYPE,
    )?;
    if blob.digest != digest(&bytes)
        || blob.size_bytes != bytes.len() as u64
        || blobs.get(workspace_id, BlobPurpose::AggregateState, &blob)? != bytes
    {
        return Err(StoreError::Integrity(
            "UserRequest state blob failed verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: version,
        record_schema_version: 1,
    })
}

fn response_fingerprint(command: &RespondUserRequestCommit) -> Result<String, StoreError> {
    let input = match &command.input {
        UserRequestResponseInput::Value(value) => json!({"response": value}),
        UserRequestResponseInput::ChoiceId(choice_id) => json!({"choice_id": choice_id}),
    };
    let body = json!({
        "operation": "user_request.respond.v1",
        "owner_principal_id": command.owner_principal_id,
        "workspace_id": command.workspace_id,
        "request_id": command.request_id,
        "expected_version": command.expected_version,
        "input": input,
    });
    Ok(digest(&canonical_json(&body)?))
}

fn dismiss_fingerprint(command: &DismissUserRequestCommit) -> Result<String, StoreError> {
    let body = json!({
        "operation": "user_request.dismiss.v1",
        "owner_principal_id": command.owner_principal_id,
        "workspace_id": command.workspace_id,
        "request_id": command.request_id,
        "expected_version": command.expected_version,
    });
    Ok(digest(&canonical_json(&body)?))
}

fn find_replay(
    tx: &Transaction<'_>,
    principal_id: &str,
    idempotency_key: &str,
    request_digest: &str,
) -> Result<Option<UserRequestResolutionReceipt>, StoreError> {
    let previous: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT request_digest, response_json, response_digest
         FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
            params![principal_id, idempotency_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((previous_digest, response_json, response_digest)) = previous else {
        return Ok(None);
    };
    if previous_digest != request_digest {
        return Err(StoreError::IdempotencyConflict);
    }
    let response_json = response_json.ok_or_else(|| {
        StoreError::Integrity("UserRequest receipt has no committed result".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "UserRequest receipt digest does not match".to_owned(),
        ));
    }
    let mut receipt: UserRequestResolutionReceipt = serde_json::from_str(&response_json)
        .map_err(|_| StoreError::Integrity("UserRequest receipt is malformed".to_owned()))?;
    receipt.replayed = true;
    Ok(Some(receipt))
}

fn save_replay(
    tx: &Transaction<'_>,
    principal_id: &str,
    idempotency_key: &str,
    request_digest: &str,
    receipt: &UserRequestResolutionReceipt,
    at: &str,
) -> Result<(), StoreError> {
    let response = canonical_json(receipt)?;
    let response = String::from_utf8(response)
        .map_err(|_| StoreError::Integrity("UserRequest receipt encoding failed".to_owned()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at)
         VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![principal_id, idempotency_key, request_digest, response, digest(response.as_bytes()), at],
    ).map_err(map_database_error)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SchemaFailure {
    Invalid,
    Unsupported,
}

fn validate_schema(schema: &Value, value: &Value, depth: usize) -> Result<(), SchemaFailure> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(SchemaFailure::Unsupported);
    }
    let object = schema.as_object().ok_or(SchemaFailure::Unsupported)?;
    const SUPPORTED: &[&str] = &[
        "$schema",
        "title",
        "description",
        "default",
        "type",
        "const",
        "enum",
        "allOf",
        "anyOf",
        "oneOf",
        "not",
        "properties",
        "required",
        "additionalProperties",
        "minProperties",
        "maxProperties",
        "items",
        "minItems",
        "maxItems",
        "uniqueItems",
        "minLength",
        "maxLength",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "format",
    ];
    if object.keys().any(|key| !SUPPORTED.contains(&key.as_str())) {
        return Err(SchemaFailure::Unsupported);
    }
    if let Some(expected) = object.get("const")
        && expected != value
    {
        return Err(SchemaFailure::Invalid);
    }
    if let Some(values) = object.get("enum") {
        let values = values.as_array().ok_or(SchemaFailure::Unsupported)?;
        if !values.iter().any(|candidate| candidate == value) {
            return Err(SchemaFailure::Invalid);
        }
    }
    if let Some(types) = object.get("type") {
        let accepted = match types {
            Value::String(kind) => matches_schema_type(kind, value),
            Value::Array(kinds) if !kinds.is_empty() => {
                kinds.iter().all(Value::is_string)
                    && kinds
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|kind| matches_schema_type(kind, value))
            }
            _ => return Err(SchemaFailure::Unsupported),
        };
        if !accepted {
            return Err(SchemaFailure::Invalid);
        }
    }

    for key in ["allOf", "anyOf", "oneOf"] {
        let Some(branches) = object.get(key) else {
            continue;
        };
        let branches = branches.as_array().ok_or(SchemaFailure::Unsupported)?;
        let matches = branches
            .iter()
            .map(|branch| validate_schema(branch, value, depth + 1))
            .collect::<Vec<_>>();
        match key {
            "allOf" if matches.iter().any(Result::is_err) => {
                return Err(matches
                    .into_iter()
                    .find_map(Result::err)
                    .unwrap_or(SchemaFailure::Invalid));
            }
            "anyOf" if !matches.iter().any(Result::is_ok) => {
                if matches
                    .iter()
                    .any(|item| *item == Err(SchemaFailure::Unsupported))
                {
                    return Err(SchemaFailure::Unsupported);
                }
                return Err(SchemaFailure::Invalid);
            }
            "oneOf" if matches.iter().filter(|item| item.is_ok()).count() != 1 => {
                if matches
                    .iter()
                    .any(|item| *item == Err(SchemaFailure::Unsupported))
                {
                    return Err(SchemaFailure::Unsupported);
                }
                return Err(SchemaFailure::Invalid);
            }
            _ => {}
        }
    }
    if let Some(negated) = object.get("not") {
        match validate_schema(negated, value, depth + 1) {
            Ok(()) => return Err(SchemaFailure::Invalid),
            Err(SchemaFailure::Unsupported) => return Err(SchemaFailure::Unsupported),
            Err(SchemaFailure::Invalid) => {}
        }
    }

    if let Some(properties) = value.as_object() {
        if properties.len() > MAX_SCHEMA_OBJECT_PROPERTIES {
            return Err(SchemaFailure::Invalid);
        }
        validate_count_bound(object, "minProperties", properties.len())?;
        validate_max_count_bound(object, "maxProperties", properties.len())?;
        if let Some(required) = object.get("required") {
            let required = required.as_array().ok_or(SchemaFailure::Unsupported)?;
            if required.iter().any(|key| !key.is_string()) {
                return Err(SchemaFailure::Unsupported);
            }
            let mut seen = std::collections::HashSet::new();
            for key in required.iter().filter_map(Value::as_str) {
                if !seen.insert(key) {
                    return Err(SchemaFailure::Unsupported);
                }
                if !properties.contains_key(key) {
                    return Err(SchemaFailure::Invalid);
                }
            }
        }
        let property_schemas = object
            .get("properties")
            .map(|value| value.as_object().ok_or(SchemaFailure::Unsupported))
            .transpose()?;
        if let Some(schemas) = property_schemas {
            for (name, value) in properties {
                if let Some(property_schema) = schemas.get(name) {
                    validate_schema(property_schema, value, depth + 1)?;
                } else if let Some(additional) = object.get("additionalProperties") {
                    match additional {
                        Value::Bool(true) => {}
                        Value::Bool(false) => return Err(SchemaFailure::Invalid),
                        Value::Object(_) => validate_schema(additional, value, depth + 1)?,
                        _ => return Err(SchemaFailure::Unsupported),
                    }
                }
            }
        } else if matches!(object.get("additionalProperties"), Some(Value::Bool(false)))
            && !properties.is_empty()
        {
            return Err(SchemaFailure::Invalid);
        }
    } else if let Some(required) = object.get("required") {
        if !required.as_array().is_some_and(Vec::is_empty) {
            return Err(SchemaFailure::Invalid);
        }
    }

    if let Some(items) = value.as_array() {
        if items.len() > MAX_SCHEMA_ARRAY_ITEMS {
            return Err(SchemaFailure::Invalid);
        }
        validate_count_bound(object, "minItems", items.len())?;
        validate_max_count_bound(object, "maxItems", items.len())?;
        if object.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
            for index in 0..items.len() {
                if items[..index].contains(&items[index]) {
                    return Err(SchemaFailure::Invalid);
                }
            }
        } else if object.contains_key("uniqueItems") && object["uniqueItems"].as_bool().is_none() {
            return Err(SchemaFailure::Unsupported);
        }
        if let Some(item_schema) = object.get("items") {
            for item in items {
                validate_schema(item_schema, item, depth + 1)?;
            }
        }
    }

    if let Some(text) = value.as_str() {
        let chars = text.chars().count();
        validate_count_bound(object, "minLength", chars)?;
        validate_max_count_bound(object, "maxLength", chars)?;
        if let Some(format) = object.get("format").and_then(Value::as_str)
            && !matches_format(format, text)
        {
            return Err(SchemaFailure::Invalid);
        }
        if object
            .get("format")
            .is_some_and(|format| !format.is_string())
        {
            return Err(SchemaFailure::Unsupported);
        }
    }

    if let Some(number) = value.as_f64() {
        for (keyword, comparison) in [
            ("minimum", 0),
            ("maximum", 1),
            ("exclusiveMinimum", 2),
            ("exclusiveMaximum", 3),
        ] {
            if let Some(bound) = object.get(keyword) {
                let bound = bound.as_f64().ok_or(SchemaFailure::Unsupported)?;
                let invalid = match comparison {
                    0 => number < bound,
                    1 => number > bound,
                    2 => number <= bound,
                    _ => number >= bound,
                };
                if invalid {
                    return Err(SchemaFailure::Invalid);
                }
            }
        }
        if let Some(multiple) = object.get("multipleOf") {
            let multiple = multiple.as_f64().ok_or(SchemaFailure::Unsupported)?;
            if multiple <= 0.0 || (number / multiple - (number / multiple).round()).abs() > 1e-10 {
                return Err(SchemaFailure::Invalid);
            }
        }
    }
    Ok(())
}

fn matches_schema_type(kind: &str, value: &Value) -> bool {
    match kind {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        _ => false,
    }
}

fn validate_count_bound(
    object: &serde_json::Map<String, Value>,
    key: &str,
    actual: usize,
) -> Result<(), SchemaFailure> {
    if let Some(bound) = object.get(key) {
        let bound = bound
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(SchemaFailure::Unsupported)?;
        if actual < bound {
            return Err(SchemaFailure::Invalid);
        }
    }
    Ok(())
}

fn validate_max_count_bound(
    object: &serde_json::Map<String, Value>,
    key: &str,
    actual: usize,
) -> Result<(), SchemaFailure> {
    if let Some(bound) = object.get(key) {
        let bound = bound
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(SchemaFailure::Unsupported)?;
        if actual > bound {
            return Err(SchemaFailure::Invalid);
        }
    }
    Ok(())
}

fn matches_format(format: &str, value: &str) -> bool {
    match format {
        "date-time" => OffsetDateTime::parse(value, &Rfc3339).is_ok(),
        "email" => value.split_once('@').is_some_and(|(local, host)| {
            !local.is_empty()
                && !host.is_empty()
                && host.contains('.')
                && !value.chars().any(char::is_whitespace)
        }),
        "uri" | "uri-reference" => {
            value.contains(':')
                && value.split_once(':').is_some_and(|(scheme, rest)| {
                    !scheme.is_empty()
                        && scheme.bytes().enumerate().all(|(index, byte)| {
                            byte.is_ascii_alphabetic()
                                || (index > 0 && (byte.is_ascii_digit() || b"+-.".contains(&byte)))
                        })
                        && !rest.chars().any(char::is_control)
                })
        }
        // Under the current JSON Schema profile format is annotation unless the
        // application explicitly declares an assertion; preserve unknown annotations.
        _ => true,
    }
}

fn schema_depth(value: &Value, depth: usize) -> Result<(), ()> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(());
    }
    match value {
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| schema_depth(value, depth + 1)),
        Value::Object(values) => values
            .values()
            .try_for_each(|value| schema_depth(value, depth + 1)),
        _ => Ok(()),
    }
}

fn schema_has_sensitive_fields(schema: &Value) -> bool {
    match schema {
        Value::Object(object) => object.iter().any(|(key, value)| {
            is_sensitive_name(key)
                || ((key == "description" || key == "title")
                    && value.as_str().is_some_and(is_sensitive_phrase))
                || (key == "format" && value.as_str().is_some_and(is_sensitive_name))
                || schema_has_sensitive_fields(value)
        }),
        Value::Array(values) => values.iter().any(schema_has_sensitive_fields),
        _ => false,
    }
}

fn value_has_sensitive_content(value: &Value, depth: usize) -> bool {
    if depth > MAX_RESPONSE_VALUE_DEPTH {
        return true;
    }
    match value {
        Value::String(text) => is_sensitive_phrase(text),
        Value::Array(values) => values
            .iter()
            .any(|value| value_has_sensitive_content(value, depth + 1)),
        Value::Object(object) => object.iter().any(|(key, value)| {
            is_sensitive_name(key) || value_has_sensitive_content(value, depth + 1)
        }),
        _ => false,
    }
}

fn is_sensitive_name(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase().replace(['-', '_', ' '], "");
    [
        "password",
        "passwd",
        "secret",
        "credential",
        "apikey",
        "apitoken",
        "accesstoken",
        "refreshtoken",
        "privatekey",
        "recoverycode",
        "bearertoken",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn is_sensitive_phrase(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "password",
        "api key",
        "access token",
        "refresh token",
        "secret key",
        "private key",
        "recovery code",
        "bearer ",
        "begin private key",
        "begin rsa private key",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
        || looks_like_common_secret(value)
}

fn looks_like_common_secret(value: &str) -> bool {
    let value = value.trim();
    let github_token = ["ghp_", "gho_", "ghu_", "ghs_", "github_pat_"]
        .iter()
        .any(|prefix| value.starts_with(prefix) && value.len() >= prefix.len() + 20);
    let aws_key = value.starts_with("AKIA")
        && value.len() == 20
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric());
    github_token
        || aws_key
        || (value.matches('.').count() == 2
            && value.len() > 80
            && value.split('.').all(|part| part.len() >= 8))
}

fn validate_json_value_depth(value: &Value, depth: usize) -> Result<(), StoreError> {
    if depth > MAX_RESPONSE_VALUE_DEPTH {
        return Err(invalid("USER_REQUEST_RESPONSE_TOO_LARGE"));
    }
    match value {
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| validate_json_value_depth(value, depth + 1)),
        Value::Object(values) => values
            .values()
            .try_for_each(|value| validate_json_value_depth(value, depth + 1)),
        _ => Ok(()),
    }
}

fn valid_timestamp(value: &str) -> bool {
    value.len() <= 64 && value.is_ascii() && OffsetDateTime::parse(value, &Rfc3339).is_ok()
}

fn validate_id(value: &str, name: &str) -> Result<(), StoreError> {
    if value.trim().is_empty()
        || value.len() > USER_REQUEST_ID_MAX_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(invalid(&format!("{name} is invalid")));
    }
    Ok(())
}

fn invalid(code: &str) -> StoreError {
    StoreError::Invalid(code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(mode: &str, kind: &str, schema: Option<&str>) -> CurrentRequest {
        CurrentRequest {
            status: "PENDING".to_owned(),
            kind: kind.to_owned(),
            interaction_mode: mode.to_owned(),
            response_schema_json: schema.map(str::to_owned),
            choices_json: "[]".to_owned(),
            expires_at: None,
            version: 1,
            conversation_id: None,
            conversation_turn_id: None,
            task_id: Some("task-1".to_owned()),
            attempt_id: None,
            agent_session_id: "session-1".to_owned(),
        }
    }

    #[test]
    fn external_authorization_accepts_only_the_bounded_action_enum() {
        let request = request("EXTERNAL_URL", "EXTERNAL_AUTHORIZATION", None);
        for action in ["accept", "decline", "cancel"] {
            assert!(validate_response(&request, &json!({"action": action})).is_ok());
        }
        for response in [
            json!({"action":"accept", "url":"https://attacker.invalid"}),
            json!({"action":"open", "url":"https://attacker.invalid"}),
            json!("accept"),
        ] {
            assert!(matches!(
                validate_response(&request, &response),
                Err(StoreError::Invalid(_))
            ));
        }
    }

    #[test]
    fn form_response_must_match_the_pinned_bounded_schema() {
        let request = request(
            "FORM",
            "DECISION",
            Some(
                r#"{"type":"object","required":["choice"],"properties":{"choice":{"type":"string","enum":["yes","no"]}},"additionalProperties":false}"#,
            ),
        );
        assert!(validate_response(&request, &json!({"choice":"yes"})).is_ok());
        assert!(matches!(
            validate_response(&request, &json!({"choice":"maybe"})),
            Err(StoreError::Invalid(_))
        ));
        assert!(matches!(
            validate_response(&request, &json!({"choice":"yes", "extra":true})),
            Err(StoreError::Invalid(_))
        ));
        assert!(matches!(
            validate_response(&request, &json!({})),
            Err(StoreError::Invalid(_))
        ));
    }

    #[test]
    fn unsupported_or_secret_bearing_form_contracts_fail_closed() {
        let unsupported = request(
            "FORM",
            "QUESTION",
            Some(r#"{"type":"string","pattern":"^[a-z]+$"}"#),
        );
        assert!(matches!(
            validate_response(&unsupported, &json!("answer")),
            Err(StoreError::Invalid(code)) if code == "PROVIDER_INPUT_UNSUPPORTED"
        ));

        let sensitive_schema = request(
            "FORM",
            "QUESTION",
            Some(r#"{"type":"object","properties":{"api_token":{"type":"string"}}}"#),
        );
        assert!(matches!(
            validate_response(&sensitive_schema, &json!({"api_token":"ordinary text"})),
            Err(StoreError::Invalid(code)) if code == "SENSITIVE_INPUT_UNSUPPORTED"
        ));

        let response = request("FORM", "QUESTION", None);
        assert!(matches!(
            validate_response(&response, &json!({"note":"my password is abc"})),
            Err(StoreError::Invalid(code)) if code == "SENSITIVE_INPUT_UNSUPPORTED"
        ));
    }

    #[test]
    fn response_depth_and_size_are_bounded_before_persistence() {
        let deeply_nested =
            (0..=MAX_RESPONSE_VALUE_DEPTH).fold(json!(null), |value, _| json!([value]));
        assert!(matches!(
            validate_json_value_depth(&deeply_nested, 0),
            Err(StoreError::Invalid(code)) if code == "USER_REQUEST_RESPONSE_TOO_LARGE"
        ));
        assert!(matches!(
            validate_json_value_depth(&json!({"ok":"yes"}), 0),
            Ok(())
        ));
    }

    #[test]
    fn choice_id_resolves_only_to_a_unique_stored_value_without_returning_other_choices() {
        let stored = r#"[
            {"choice_id":"yes-1","label":"Yes","description":null,"value":{"answer":"yes"}},
            {"choice_id":"no-1","label":"No","description":"No thanks","value":{"answer":"no"}}
        ]"#;
        assert_eq!(
            resolve_choice_value(stored, "yes-1").expect("known choice resolves"),
            json!({"answer":"yes"})
        );
        assert!(matches!(
            resolve_choice_value(stored, "unknown"),
            Err(StoreError::Invalid(_))
        ));
        assert!(matches!(
            resolve_choice_value(
                r#"[{"choice_id":"same","value":"a"},{"choice_id":"same","value":"b"}]"#,
                "same"
            ),
            Err(StoreError::Integrity(_))
        ));
    }
}
