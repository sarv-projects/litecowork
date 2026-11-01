//! Durable `ASK` surfacing for a pinned lead-failover policy.
//!
//! `TASK-RUNTIME.md` requires `ASK` to "add a Needs You choice from its ordered
//! fallback list". The durable artifact is a planning-scope `UserRequest` of kind
//! `DECISION`: the ordered fallback bindings are the only answers the owner may
//! give, the immutable `user.request.created.v1` event records the choice request,
//! and the exact idempotency receipt replays the committed request.
//!
//! This module never changes a Task's lead, never starts a session, never reserves
//! a lease and never dispatches a provider. It re-reads the pinned TaskSpec policy,
//! the current lead, the observation freshness and the durable binding eligibility
//! inside the same transaction that writes the request, the event and the receipt,
//! so a caller-supplied payload can never widen the offered choices.
//!
//! The observation source identity is the authenticated Runtime service that
//! produced it. One observation may produce at most one owner choice request: the
//! durable request ID is derived from the observation identity, so a replayed
//! observation cannot open a second Needs You item.

use super::delegation_failover::{self, StoredFailoverObservation};
use super::*;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use storage_core::{DomainEvent, EventDraft, StoreError};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// The pinned policy bounds the fallback list to at most three bindings.
const MAX_FALLBACK_CHOICES: usize = 3;
/// A lead replacement is a decision between named alternatives.
const ASK_REQUEST_KIND: &str = "DECISION";
/// The owner answers from the Needs You surface; there is no external handoff.
const ASK_INTERACTION_MODE: &str = "FORM";
const ASK_EVENT_TYPE: &str = "user.request.created.v1";
const USER_REQUEST_STATE_MEDIA_TYPE: &str = "application/vnd.litecowork.user-request-state+json";
const ASK_REQUEST_ID_PREFIX: &str = "lead-failover-choice-";
const MAX_LABEL_BYTES: usize = 512;

/// One durable owner-choice request for a pinned `ASK` lead-failover policy.
///
/// The commit is a service command: the caller is the authenticated Runtime or
/// adapter service that observed the confirmed trigger. It never carries owner
/// authority, a provider handle, a transcript or a secret.
#[derive(Clone, Debug)]
pub struct LeadFailoverChoiceRequestCommit {
    pub workspace_id: String,
    pub task_id: String,
    /// The authenticated service identity that produced the observation.
    pub request_scope_id: String,
    /// The caller idempotency key. The committed receipt is replayed for it.
    pub request_id: String,
    pub expected_task_version: u64,
    pub expected_task_spec_revision: u64,
    /// The confirmed provider/Runtime trigger observation.
    pub observation: Value,
    /// The event draft. The transaction owns the request identity and payload: it
    /// replaces `entity_id` with the derived request ID and writes the canonical
    /// `user.request.created.v1` payload.
    pub event: EventDraft,
}

/// The committed owner-choice request.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LeadFailoverChoiceRequestReceipt {
    /// The durable `UserRequest` ID.
    pub request_id: String,
    pub workspace_id: String,
    pub task_id: String,
    /// The planning AgentSession that scopes the request to this Task.
    pub agent_session_id: String,
    /// The ordered fallback choices presented to the owner, in pinned policy order.
    pub fallback_agent_binding_ids: Vec<String>,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// The writer-thread operation used by the store method.

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PendingUserRequestAggregateState {
    request_id: String,
    workspace_id: String,
    task_id: String,
    status: String,
    version: u64,
}

/// The Task states in which a pending Task-scoped UserRequest can still be
/// answered. A request created outside them would be unanswerable.
const RESPONDABLE_TASK_STATUSES: [&str; 7] = [
    "READY",
    "RUNNING",
    "WAITING_USER",
    "BLOCKED",
    "NEEDS_USER",
    "INCOMPLETE",
    "PAUSED",
];

/// Commit the owner-choice request, its event and its replay receipt atomically.
///
/// A rejected commit writes nothing: no request row, no event and no receipt.
pub(super) fn commit_transaction(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    commit: LeadFailoverChoiceRequestCommit,
) -> Result<LeadFailoverChoiceRequestReceipt, StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let request_digest = digest(&canonical_json(&canonical_request_payload(&commit))?);
    if let Some(mut receipt) = load_receipt(
        &transaction,
        &commit.request_scope_id,
        &commit.request_id,
        &request_digest,
    )? {
        receipt.replayed = true;
        return Ok(receipt);
    }
    validate_commit(&commit)?;

    let observation = StoredFailoverObservation::parse(&commit.observation)?;
    observation.validate_observation_freshness(&commit.event.recorded_at)?;
    if observation.source_service_id() != commit.request_scope_id {
        return Err(StoreError::Invalid(
            "lead failover observation source does not match the requesting service".to_owned(),
        ));
    }
    let request_id = derive_request_id(
        &commit.workspace_id,
        observation.source_service_id(),
        observation.source_observation_ref(),
    )?;

    let workspace_status: Option<String> = transaction
        .query_row(
            "SELECT status FROM workspaces WHERE workspace_id = ?1",
            [&commit.workspace_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    match workspace_status.as_deref() {
        Some("ACTIVE") => {}
        _ => return Err(StoreError::NotFound),
    }

    let task = load_task_view(&transaction, &commit.workspace_id, &commit.task_id)?
        .ok_or(StoreError::NotFound)?;
    if task.task.version != commit.expected_task_version {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_task_version),
            actual: Some(task.task.version),
        });
    }
    if task.task.current_spec_revision != commit.expected_task_spec_revision {
        return Err(StoreError::Invalid("STALE_SPEC_REVISION".to_owned()));
    }
    if !RESPONDABLE_TASK_STATUSES.contains(&task.task.status.as_str()) {
        return Err(StoreError::Invalid(
            "Task cannot hold an answerable owner choice request in its current state".to_owned(),
        ));
    }
    let current_lead_agent_binding_id = task.task.lead_agent_binding_id.clone();
    if observation.affected_agent_binding_id() != current_lead_agent_binding_id {
        return Err(StoreError::Invalid(
            "lead failover observation does not match the current lead".to_owned(),
        ));
    }

    let policy = delegation_failover::load_pinned_policy(
        &transaction,
        &commit.workspace_id,
        &commit.task_id,
        commit.expected_task_spec_revision,
    )?;
    if policy.mode != "ASK" {
        return Err(StoreError::Invalid(
            "lead failover policy does not ask the owner for a choice".to_owned(),
        ));
    }
    if !policy
        .triggers
        .iter()
        .any(|trigger| trigger == observation.trigger())
    {
        return Err(StoreError::Invalid(
            "lead failover trigger is not allowed by the pinned policy".to_owned(),
        ));
    }
    if policy.fallback_agent_binding_ids.is_empty()
        || policy.fallback_agent_binding_ids.len() > MAX_FALLBACK_CHOICES
    {
        return Err(StoreError::Invalid(
            "lead failover policy has no bounded fallback list to offer".to_owned(),
        ));
    }

    let choices = durable_choices(
        &transaction,
        &commit.workspace_id,
        &current_lead_agent_binding_id,
        &policy.fallback_agent_binding_ids,
    )?;
    if choices.is_empty() {
        return Err(StoreError::Invalid(
            "LEAD_FAILOVER_NO_ELIGIBLE_FALLBACK".to_owned(),
        ));
    }
    let agent_session_id = load_planning_session(
        &transaction,
        &commit.workspace_id,
        &commit.task_id,
        &current_lead_agent_binding_id,
    )?;

    let existing: Option<String> = transaction
        .query_row(
            "SELECT request_id FROM user_requests WHERE request_id = ?1",
            [&request_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    if existing.is_some() {
        return Err(StoreError::Invalid(
            "LEAD_FAILOVER_OBSERVATION_ALREADY_ASKED".to_owned(),
        ));
    }

    let prompt = ask_prompt(observation.trigger());
    let choices_json = String::from_utf8(canonical_json(&choices)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let response_schema_json = String::from_utf8(canonical_json(&answer_schema())?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    if choices_json.len() > storage_core::USER_REQUEST_CHOICES_MAX_BYTES
        || prompt.len() > storage_core::USER_REQUEST_PROMPT_MAX_BYTES
    {
        return Err(StoreError::Invalid(
            "lead failover choice request exceeds its bounded size".to_owned(),
        ));
    }
    transaction
        .execute(
            "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id, kind,
                interaction_mode, prompt, response_schema_json, choices_json, status, created_at,
                expires_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'PENDING', ?10, NULL, 1)",
            params![
                request_id,
                commit.workspace_id,
                commit.task_id,
                agent_session_id,
                ASK_REQUEST_KIND,
                ASK_INTERACTION_MODE,
                prompt,
                response_schema_json,
                choices_json,
                commit.event.recorded_at,
            ],
        )
        .map_err(map_database_error)?;

    let state_ref =
        put_user_request_state(blobs, &commit.workspace_id, &request_id, &commit.task_id)?;
    let mut event = commit.event;
    let recorded_at = event.recorded_at.clone();
    event.entity_type = "user_request".to_owned();
    event.entity_id = request_id.clone();
    event.entity_revision = 1;
    event.payload = json!({
        "request_id": request_id,
        "workspace_id": commit.workspace_id,
        "task_id": commit.task_id,
        "agent_session_id": agent_session_id,
        "kind": ASK_REQUEST_KIND,
        "interaction_mode": ASK_INTERACTION_MODE,
    });
    let event = insert_domain_event(&transaction, &event, &state_ref)?;

    let committed = LeadFailoverChoiceRequestReceipt {
        request_id: request_id.clone(),
        workspace_id: commit.workspace_id.clone(),
        task_id: commit.task_id.clone(),
        agent_session_id: agent_session_id.clone(),
        fallback_agent_binding_ids: choices
            .iter()
            .filter_map(|choice| {
                choice
                    .get("value")
                    .and_then(|value| value.get("agent_binding_id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect(),
        event,
        replayed: false,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    transaction.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![commit.request_scope_id, commit.request_id, request_digest, response_json,
            digest(response_json.as_bytes()), recorded_at],
    ).map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(committed)
}

/// The ordered, currently eligible fallback choices in pinned policy order. The
/// current lead is never offered as its own replacement.
fn durable_choices(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    current_lead_agent_binding_id: &str,
    fallback_agent_binding_ids: &[String],
) -> Result<Vec<Value>, StoreError> {
    let mut choices = Vec::new();
    for fallback_id in fallback_agent_binding_ids {
        if fallback_id == current_lead_agent_binding_id {
            continue;
        }
        let display_name: Option<String> = transaction
            .query_row(
                "SELECT p.display_name FROM agent_bindings b
                 JOIN agent_profiles p ON p.agent_profile_id = b.agent_profile_id
                 WHERE b.workspace_id = ?1 AND b.agent_binding_id = ?2 AND b.enabled = 1 AND b.lead_eligible = 1",
                params![workspace_id, fallback_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_database_error)?;
        let Some(display_name) = display_name else {
            // Not an eligible fallback at all; the durable transaction skips it.
            continue;
        };
        choices.push(json!({
            "choice_id": fallback_id,
            "label": choice_label(&display_name, fallback_id),
            "description": format!("Pinned fallback {}.", choices.len() + 1),
            "value": {"agent_binding_id": fallback_id},
        }));
    }
    Ok(choices)
}

/// The Task's planning AgentSession scopes the choice request to this Task. The
/// request is bound to the session of the current lead so that the answer returns
/// to the Task that raised it.
fn load_planning_session(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    task_id: &str,
    current_lead_agent_binding_id: &str,
) -> Result<String, StoreError> {
    let session: Option<String> = transaction
        .query_row(
            "SELECT agent_session_id FROM agent_sessions
             WHERE workspace_id = ?1 AND task_id = ?2 AND scope_kind = 'TASK_PLANNING'
               AND attempt_id IS NULL AND agent_binding_id = ?3
             ORDER BY CASE WHEN status IN ('STARTING', 'ACTIVE', 'INTERRUPTING', 'CLOSING')
                            THEN 0 ELSE 1 END,
                      started_at DESC, agent_session_id DESC
             LIMIT 1",
            params![workspace_id, task_id, current_lead_agent_binding_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    session.ok_or_else(|| {
        StoreError::Invalid(
            "Task has no planning AgentSession to scope the owner choice request".to_owned(),
        )
    })
}

/// One observation may produce at most one owner choice request in a Workspace. The
/// durable request ID is derived from the observation identity, so a replayed or
/// duplicated observation cannot open a second Needs You item or a second choice
/// request for another Task of the same Workspace. This matches the automatic
/// failover rule that one observation authorizes one lead decision per Workspace.
fn derive_request_id(
    workspace_id: &str,
    source_service_id: &str,
    source_observation_ref: &str,
) -> Result<String, StoreError> {
    let material = canonical_json(&json!({
        "workspace_id": workspace_id,
        "source_service_id": source_service_id,
        "source_observation_ref": source_observation_ref,
    }))?;
    let digest = digest(&material);
    Ok(format!(
        "{ASK_REQUEST_ID_PREFIX}{}",
        digest.get(7..39).unwrap_or_default()
    ))
}

/// The host-authored summary shown in Needs You. It names the confirmed trigger
/// only; it never carries provider text, a native handle or a secret.
fn ask_prompt(trigger: &str) -> String {
    format!(
        "The pinned lead reported {trigger} and cannot continue. Choose a replacement lead from the ordered fallback list."
    )
}

/// The answer contract. Only a named fallback binding is a valid answer, so a
/// free-form response cannot silently select an unbound replacement.
fn answer_schema() -> Value {
    json!({
        "type": "object",
        "required": ["agent_binding_id"],
        "additionalProperties": false,
        "properties": {
            "agent_binding_id": {"type": "string", "minLength": 1, "maxLength": 512},
        },
    })
}

/// A bounded, host-authored label. The catalog display name is sanitized; the
/// binding identity is always present so the choice stays unambiguous.
fn choice_label(display_name: &str, agent_binding_id: &str) -> String {
    let sanitized: String = display_name
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let sanitized = sanitized.trim();
    let bounded = if sanitized.len() > MAX_LABEL_BYTES {
        sanitized.get(..MAX_LABEL_BYTES).unwrap_or_default().trim()
    } else {
        sanitized
    };
    if bounded.is_empty() {
        agent_binding_id.to_owned()
    } else {
        format!("{bounded} ({agent_binding_id})")
    }
}

fn put_user_request_state(
    blobs: &dyn BlobStore,
    workspace_id: &str,
    request_id: &str,
    task_id: &str,
) -> Result<AggregateStateRef, StoreError> {
    let state = PendingUserRequestAggregateState {
        request_id: request_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        task_id: task_id.to_owned(),
        status: "PENDING".to_owned(),
        version: 1,
    };
    let bytes = canonical_json(&state)?;
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
        entity_revision: 1,
        record_schema_version: 1,
    })
}

/// The canonical idempotency payload of one owner-choice request.
fn canonical_request_payload(commit: &LeadFailoverChoiceRequestCommit) -> Value {
    json!({
        "workspace_id": commit.workspace_id,
        "task_id": commit.task_id,
        "request_scope_id": commit.request_scope_id,
        "expected_task_version": commit.expected_task_version,
        "expected_task_spec_revision": commit.expected_task_spec_revision,
        "observation": commit.observation,
    })
}

fn load_receipt(
    transaction: &Transaction<'_>,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
) -> Result<Option<LeadFailoverChoiceRequestReceipt>, StoreError> {
    let prior: Option<(String, Option<String>, Option<String>)> = transaction
        .query_row(
            "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            params![principal_id, request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((prior_digest, response_json, response_digest)) = prior else {
        return Ok(None);
    };
    if prior_digest != request_digest {
        return Err(StoreError::IdempotencyConflict);
    }
    let response_json = response_json.ok_or_else(|| {
        StoreError::Integrity("lead failover choice request receipt is incomplete".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "lead failover choice request receipt digest does not match".to_owned(),
        ));
    }
    serde_json::from_str(&response_json)
        .map(Some)
        .map_err(|error| StoreError::Integrity(error.to_string()))
}

fn validate_commit(commit: &LeadFailoverChoiceRequestCommit) -> Result<(), StoreError> {
    let ids = [
        commit.workspace_id.as_str(),
        commit.task_id.as_str(),
        commit.request_scope_id.as_str(),
        commit.request_id.as_str(),
        commit.event.event_id.as_str(),
        commit.event.origin_runtime_id.as_str(),
        commit.event.hlc_timestamp.as_str(),
        commit.event.correlation_id.as_str(),
        commit.event.recorded_at.as_str(),
    ];
    if ids.iter().any(|value| {
        value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control)
    }) || commit.expected_task_version == 0
        || commit.expected_task_spec_revision == 0
        || commit.event.schema_version != 1
        || commit.event.event_type != ASK_EVENT_TYPE
        || commit.event.entity_type != "user_request"
        || commit.event.entity_revision != 1
        || commit.event.workspace_id != commit.workspace_id
        || parse_timestamp(&commit.event.recorded_at).is_none()
    {
        return Err(StoreError::Invalid(
            "lead failover choice request commit is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn parse_timestamp(value: &str) -> Option<OffsetDateTime> {
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return None;
    }
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use std::sync::Arc;
    use std::time::Duration;
    use storage_core::{
        BlobPurpose, NeedsYouFilter, NeedsYouPageRequest, NeedsYouReadStore,
        UserRequestDetailReadStore, UserRequestResolutionActor, UserRequestResolutionStore,
        UserRequestResponseInput,
    };
    use zeroize::Zeroizing;

    const OWNER: &str = "owner-ask";
    const WORKSPACE: &str = "workspace-ask";
    const TASK: &str = "task-ask";
    const LEAD: &str = "binding-lead";
    const FALLBACK_1: &str = "binding-fallback-1";
    const FALLBACK_2: &str = "binding-fallback-2";
    const SERVICE: &str = "lead-failover-observer";
    const NOW: &str = "2026-10-20T10:00:00Z";

    struct TestKeys;

    impl super::super::blob::WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<super::super::blob::WorkspaceBlobKey, StoreError> {
            Ok(super::super::blob::WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([7_u8; 32]),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            version: u32,
        ) -> Result<super::super::blob::WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob("unknown test key version".to_owned()));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    struct Fixture {
        _directory: tempfile::TempDir,
        store: SqliteWorkspaceStore,
        database: std::path::PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("temporary store directory");
            let database = directory.path().join("state").join("state.sqlite3");
            let blobs = Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys));
            let store = SqliteWorkspaceStore::open(
                &database,
                blobs,
                SqliteConfig {
                    writer_queue_capacity: 8,
                    busy_timeout: Duration::from_secs(5),
                },
            )
            .expect("SQLite store opens");
            Self {
                _directory: directory,
                store,
                database,
            }
        }

        fn connection(&self) -> Connection {
            Connection::open(&self.database).expect("open fixture database")
        }

        fn seed(&self, policy: &Value, task_status: &str) {
            WorkspaceService::new(self.store.clone())
                .create(CreateWorkspace {
                    workspace_id: WORKSPACE.to_owned(),
                    name: "Ask workspace".to_owned(),
                    owner_principal_id: OWNER.to_owned(),
                    event: context("event-create-workspace"),
                })
                .expect("fixture Workspace commits");
            let connection = self.connection();
            connection
                .execute_batch(
                    "INSERT INTO runtimes(runtime_id, device_identity_json, runtime_version,
                        platform, architecture, roles_json, trust_zone, availability, startup_policy,
                        current_incarnation_id, resource_capacity_json, last_seen, version)
                     VALUES ('runtime-ask', '{}', '0.1', 'linux', 'x86_64', '[]',
                        'LOCAL', 'ONLINE', 'MANUAL', NULL, '{}', '2026-10-20T09:00:00Z', 1);
                     INSERT INTO runtime_incarnations(runtime_incarnation_id, runtime_id, process_started_at,
                        litecowork_version, recovered_from_unclean_shutdown, recovery_state, ready_at, version)
                     VALUES ('incarnation-ask', 'runtime-ask', '2026-10-20T09:00:00Z', '0.1', 0, 'READY',
                        '2026-10-20T09:00:00Z', 1);
                     UPDATE runtimes SET current_incarnation_id = 'incarnation-ask' WHERE runtime_id = 'runtime-ask';
                     INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at)
                     VALUES ('profile-ask', 'fixture', 'Fixture Agent', '2026-10-20T09:00:00Z');
                     INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id,
                        endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version)
                     VALUES ('binding-lead', 'workspace-ask', 'profile-ask', '{}', '{}', 1, 1, '2026-10-20T09:00:00Z', 1);
                     INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id,
                        endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version)
                     VALUES ('binding-fallback-1', 'workspace-ask', 'profile-ask', '{}', '{}', 1, 1, '2026-10-20T09:00:00Z', 1);
                     INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id,
                        endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version)
                     VALUES ('binding-fallback-2', 'workspace-ask', 'profile-ask', '{}', '{}', 1, 1, '2026-10-20T09:00:00Z', 1);",
                )
                .expect("fixture rows insert");
            connection
                .execute_batch(&format!(
                    "BEGIN;
                     INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status,
                        lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                        created_at, updated_at, version)
                     VALUES ('task-ask', 'workspace-ask', 1, 'RUNNING', 'binding-lead', '[]', 'NORMAL',
                        '{{\"kind\":\"USER\",\"principal_id\":\"owner-ask\"}}', '2026-10-20T09:00:00Z',
                        '2026-10-20T09:00:00Z', 1);
                     INSERT INTO task_spec_revisions(task_id, workspace_id, revision, objective,
                        lead_failover_policy_json, placement_preference, preferred_lead_agent_binding_id,
                        authored_by_json, created_at)
                     VALUES ('task-ask', 'workspace-ask', 1, 'Ask objective', '{}', '\"AUTO\"', 'binding-lead',
                        '{{\"kind\":\"USER\",\"principal_id\":\"owner-ask\"}}', '2026-10-20T09:00:00Z');
                     COMMIT;",
                    policy
                ))
                .expect("fixture Task and TaskSpec revision insert");
            connection
                .execute(
                    "INSERT INTO agent_sessions(agent_session_id, workspace_id, scope_kind, task_id,
                        task_spec_revision, agent_binding_id, endpoint_id, runtime_id,
                        runtime_incarnation_id, status, started_at, last_event_at, version)
                     VALUES ('session-ask', 'workspace-ask', 'TASK_PLANNING', 'task-ask', 1, 'binding-lead',
                        'endpoint-ask', 'runtime-ask', 'incarnation-ask', 'ACTIVE', '2026-10-20T09:00:00Z',
                        '2026-10-20T09:00:00Z', 1)",
                    [],
                )
                .expect("fixture planning session inserts");
            if task_status != "RUNNING" {
                connection
                    .execute(
                        "UPDATE tasks SET status = ?1 WHERE task_id = 'task-ask'",
                        [task_status],
                    )
                    .expect("fixture Task status updates");
            }
        }

        /// A second Task of the same Workspace led by the same binding. The same
        /// observation must not open a second owner choice request for it.
        fn seed_second_task(&self, policy: &Value) {
            let connection = self.connection();
            connection
                .execute_batch(&format!(
                    "BEGIN;
                     INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status,
                        lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                        created_at, updated_at, version)
                     VALUES ('task-ask-2', 'workspace-ask', 1, 'RUNNING', 'binding-lead', '[]', 'NORMAL',
                        '{{\"kind\":\"USER\",\"principal_id\":\"owner-ask\"}}', '2026-10-20T09:00:00Z',
                        '2026-10-20T09:00:00Z', 1);
                     INSERT INTO task_spec_revisions(task_id, workspace_id, revision, objective,
                        lead_failover_policy_json, placement_preference, preferred_lead_agent_binding_id,
                        authored_by_json, created_at)
                     VALUES ('task-ask-2', 'workspace-ask', 1, 'Second Ask objective', '{}', '\"AUTO\"',
                        'binding-lead', '{{\"kind\":\"USER\",\"principal_id\":\"owner-ask\"}}',
                        '2026-10-20T09:00:00Z');
                     COMMIT;",
                    policy
                ))
                .expect("second fixture Task and TaskSpec revision insert");
            connection
                .execute(
                    "INSERT INTO agent_sessions(agent_session_id, workspace_id, scope_kind, task_id,
                        task_spec_revision, agent_binding_id, endpoint_id, runtime_id,
                        runtime_incarnation_id, status, started_at, last_event_at, version)
                     VALUES ('session-ask-2', 'workspace-ask', 'TASK_PLANNING', 'task-ask-2', 1,
                        'binding-lead', 'endpoint-ask', 'runtime-ask', 'incarnation-ask', 'ACTIVE',
                        '2026-10-20T09:00:00Z', '2026-10-20T09:00:00Z', 1)",
                    [],
                )
                .expect("second fixture planning session inserts");
        }
    }

    fn context(event_id: &str) -> EventContext {
        EventContext {
            event_id: event_id.to_owned(),
            origin_runtime_id: "runtime-ask".to_owned(),
            hlc_timestamp: NOW.to_owned(),
            correlation_id: "correlation-ask".to_owned(),
            causation_id: None,
            recorded_at: NOW.to_owned(),
        }
    }

    fn ask_policy() -> Value {
        json!({
            "mode": "ASK",
            "triggers": ["QUOTA_EXHAUSTED"],
            "fallback_agent_binding_ids": [FALLBACK_1, FALLBACK_2],
            "max_lead_changes": 0
        })
    }

    fn observation(reference: &str) -> Value {
        json!({
            "trigger": "QUOTA_EXHAUSTED",
            "affected_agent_binding_id": LEAD,
            "source": {"service_id": SERVICE},
            "source_observation_ref": reference,
            "runtime_incarnation_id": "incarnation-ask",
            "observed_at": "2026-10-20T09:59:00Z",
            "expires_at": "2026-10-20T10:01:00Z",
        })
    }

    fn commit(request_id: &str, observation: Value) -> LeadFailoverChoiceRequestCommit {
        commit_for_task(request_id, TASK, observation)
    }

    fn commit_for_task(
        request_id: &str,
        task_id: &str,
        observation: Value,
    ) -> LeadFailoverChoiceRequestCommit {
        LeadFailoverChoiceRequestCommit {
            workspace_id: WORKSPACE.to_owned(),
            task_id: task_id.to_owned(),
            request_scope_id: SERVICE.to_owned(),
            request_id: request_id.to_owned(),
            expected_task_version: 1,
            expected_task_spec_revision: 1,
            observation,
            event: EventDraft {
                event_id: format!("event-{request_id}"),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "user_request".to_owned(),
                entity_id: "replaced-by-the-transaction".to_owned(),
                origin_runtime_id: "runtime-ask".to_owned(),
                entity_revision: 1,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: format!("correlation-{request_id}"),
                causation_id: None,
                schema_version: 1,
                event_type: ASK_EVENT_TYPE.to_owned(),
                payload: json!({}),
                recorded_at: NOW.to_owned(),
            },
        }
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection
            .query_row(sql, [], |row| row.get(0))
            .expect("fixture count")
    }

    #[test]
    fn an_ask_policy_commits_a_durable_choice_request_in_the_needs_you_inbox() {
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "RUNNING");
        let receipt = fixture
            .store
            .commit_lead_failover_choice_request(commit("ask-1", observation("quota-1")))
            .expect("the ASK policy commits a durable choice request");
        assert_eq!(
            receipt.fallback_agent_binding_ids,
            vec![FALLBACK_1, FALLBACK_2]
        );
        assert!(!receipt.replayed);
        assert_eq!(receipt.event.event_type, ASK_EVENT_TYPE);
        assert_eq!(receipt.event.entity_type, "user_request");
        assert_eq!(receipt.event.entity_id, receipt.request_id);
        assert_eq!(receipt.event.payload["kind"], ASK_REQUEST_KIND);
        assert_eq!(
            receipt.event.payload["interaction_mode"],
            ASK_INTERACTION_MODE
        );
        assert_eq!(receipt.event.payload["task_id"], TASK);
        assert_eq!(receipt.event.payload["agent_session_id"], "session-ask");
        assert!(receipt.request_id.starts_with(ASK_REQUEST_ID_PREFIX));

        let connection = fixture.connection();
        let (kind, mode, status, choices): (String, String, String, String) = connection
            .query_row(
                "SELECT kind, interaction_mode, status, choices_json FROM user_requests WHERE request_id = ?1",
                [&receipt.request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("committed UserRequest row");
        assert_eq!(kind, ASK_REQUEST_KIND);
        assert_eq!(mode, ASK_INTERACTION_MODE);
        assert_eq!(status, "PENDING");
        let choices: Value = serde_json::from_str(&choices).expect("choices are JSON");
        let choices = choices.as_array().expect("choices are an array");
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0]["choice_id"], FALLBACK_1);
        assert_eq!(choices[1]["choice_id"], FALLBACK_2);
        assert_eq!(choices[0]["value"]["agent_binding_id"], FALLBACK_1);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM domain_events WHERE type = 'user.request.created.v1'"
            ),
            1
        );
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM request_dedup"), 1);

        let page = fixture
            .store
            .list_needs_you_page(NeedsYouPageRequest {
                workspace_id: WORKSPACE.to_owned(),
                filter: NeedsYouFilter::default(),
                limit: 200,
                cursor: None,
            })
            .expect("the Needs You projection reads the committed request");
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0].user_request_id.as_deref(),
            Some(receipt.request_id.as_str())
        );
        assert_eq!(page.items[0].task_id.as_deref(), Some(TASK));

        let detail = fixture
            .store
            .get_user_request_detail(WORKSPACE, &receipt.request_id)
            .expect("the request detail reads")
            .expect("the request detail exists");
        assert_eq!(detail.choices.len(), 2);
        assert_eq!(detail.choices[0].choice_id, FALLBACK_1);
        assert_eq!(detail.choices[1].choice_id, FALLBACK_2);
    }

    #[test]
    fn the_owner_answer_resolves_to_the_chosen_fallback_binding() {
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "RUNNING");
        let receipt = fixture
            .store
            .commit_lead_failover_choice_request(commit("ask-answer", observation("quota-answer")))
            .expect("the ASK policy commits a durable choice request");
        let response = SqliteUserRequestResolutionStore::new(fixture.store.clone())
            .respond_to_user_request(storage_core::RespondUserRequestCommit {
                owner_principal_id: OWNER.to_owned(),
                actor: UserRequestResolutionActor {
                    principal_id: OWNER.to_owned(),
                    kind: "USER".to_owned(),
                },
                workspace_id: WORKSPACE.to_owned(),
                request_id: receipt.request_id.clone(),
                idempotency_key: "answer-1".to_owned(),
                expected_version: 1,
                response_id: "response-1".to_owned(),
                input: UserRequestResponseInput::ChoiceId(FALLBACK_2.to_owned()),
                event: EventDraft {
                    event_id: "event-answer-1".to_owned(),
                    workspace_id: WORKSPACE.to_owned(),
                    entity_type: "user_request".to_owned(),
                    entity_id: receipt.request_id.clone(),
                    origin_runtime_id: "runtime-ask".to_owned(),
                    entity_revision: 2,
                    hlc_timestamp: "2026-10-20T10:05:00Z".to_owned(),
                    correlation_id: "correlation-answer-1".to_owned(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "user.request.resolved.v1".to_owned(),
                    payload: json!({}),
                    recorded_at: "2026-10-20T10:05:00Z".to_owned(),
                },
            })
            .expect("the owner answers the choice request");
        assert_eq!(response.status, "ANSWERED");
        assert!(response.response_digest.is_some());
        let connection = fixture.connection();
        let stored: String = connection
            .query_row(
                "SELECT response_json FROM user_request_responses WHERE request_id = ?1",
                [&receipt.request_id],
                |row| row.get(0),
            )
            .expect("stored owner response");
        let stored: Value = serde_json::from_str(&stored).expect("response is JSON");
        assert_eq!(stored["agent_binding_id"], FALLBACK_2);
    }

    #[test]
    fn an_exact_replay_returns_the_committed_receipt_without_a_second_request() {
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "RUNNING");
        let first = fixture
            .store
            .commit_lead_failover_choice_request(commit("ask-replay", observation("quota-replay")))
            .expect("first commit");
        let replay = fixture
            .store
            .commit_lead_failover_choice_request(commit("ask-replay", observation("quota-replay")))
            .expect("exact replay");
        assert!(replay.replayed);
        assert_eq!(replay.request_id, first.request_id);
        let connection = fixture.connection();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM user_requests"), 1);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM domain_events WHERE type = 'user.request.created.v1'"
            ),
            1
        );
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM request_dedup"), 1);
    }

    #[test]
    fn one_observation_cannot_open_a_second_choice_request() {
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "RUNNING");
        fixture
            .store
            .commit_lead_failover_choice_request(commit("ask-first", observation("quota-once")))
            .expect("the first request commits");
        let error = fixture
            .store
            .commit_lead_failover_choice_request(commit("ask-second", observation("quota-once")))
            .expect_err("the same observation cannot open a second request");
        assert!(
            matches!(&error, StoreError::Invalid(code) if code == "LEAD_FAILOVER_OBSERVATION_ALREADY_ASKED"),
            "unexpected error: {error:?}"
        );
        let connection = fixture.connection();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM user_requests"), 1);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM domain_events WHERE type = 'user.request.created.v1'"
            ),
            1
        );
    }

    #[test]
    fn stale_expired_future_or_wrong_lead_observations_are_rejected_without_mutation() {
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "RUNNING");
        let cases = [
            (
                "expired",
                json!({
                    "trigger": "QUOTA_EXHAUSTED",
                    "affected_agent_binding_id": LEAD,
                    "source": {"service_id": SERVICE},
                    "source_observation_ref": "quota-expired",
                    "observed_at": "2026-10-20T09:59:00Z",
                    "expires_at": "2026-10-20T10:00:00Z",
                }),
            ),
            (
                "future",
                json!({
                    "trigger": "QUOTA_EXHAUSTED",
                    "affected_agent_binding_id": LEAD,
                    "source": {"service_id": SERVICE},
                    "source_observation_ref": "quota-future",
                    "observed_at": "2026-10-20T10:00:01Z",
                    "expires_at": "2026-10-20T10:01:00Z",
                }),
            ),
            (
                "wrong-lead",
                json!({
                    "trigger": "QUOTA_EXHAUSTED",
                    "affected_agent_binding_id": FALLBACK_1,
                    "source": {"service_id": SERVICE},
                    "source_observation_ref": "quota-wrong-lead",
                    "observed_at": "2026-10-20T09:59:00Z",
                    "expires_at": "2026-10-20T10:01:00Z",
                }),
            ),
            (
                "wrong-trigger",
                json!({
                    "trigger": "AGENT_UNAVAILABLE",
                    "affected_agent_binding_id": LEAD,
                    "source": {"service_id": SERVICE},
                    "source_observation_ref": "quota-wrong-trigger",
                    "observed_at": "2026-10-20T09:59:00Z",
                    "expires_at": "2026-10-20T10:01:00Z",
                }),
            ),
            (
                "foreign-source",
                json!({
                    "trigger": "QUOTA_EXHAUSTED",
                    "affected_agent_binding_id": LEAD,
                    "source": {"service_id": "another-service"},
                    "source_observation_ref": "quota-foreign",
                    "observed_at": "2026-10-20T09:59:00Z",
                    "expires_at": "2026-10-20T10:01:00Z",
                }),
            ),
        ];
        for (index, (name, observation)) in cases.into_iter().enumerate() {
            let error = fixture
                .store
                .commit_lead_failover_choice_request(commit(
                    &format!("ask-invalid-{index}"),
                    observation,
                ))
                .expect_err("an invalid observation is rejected");
            assert!(
                matches!(error, StoreError::Invalid(_)),
                "{name} produced an unexpected error"
            );
        }
        let connection = fixture.connection();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM user_requests"), 0);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM domain_events WHERE type = 'user.request.created.v1'"
            ),
            0
        );
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM request_dedup"), 0);
    }

    #[test]
    fn one_observation_cannot_open_a_choice_request_for_another_task() {
        // One adapter observation authorizes one owner choice in the Workspace, the
        // same rule the automatic failover transaction applies to a lead change.
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "RUNNING");
        fixture.seed_second_task(&ask_policy());
        fixture
            .store
            .commit_lead_failover_choice_request(commit("ask-first", observation("quota-shared")))
            .expect("the first request commits");
        let error = fixture
            .store
            .commit_lead_failover_choice_request(commit_for_task(
                "ask-other-task",
                "task-ask-2",
                observation("quota-shared"),
            ))
            .expect_err("the same observation cannot open a second request");
        assert!(
            matches!(&error, StoreError::Invalid(code) if code == "LEAD_FAILOVER_OBSERVATION_ALREADY_ASKED"),
            "unexpected error: {error:?}"
        );
        let connection = fixture.connection();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM user_requests"), 1);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM domain_events WHERE type = 'user.request.created.v1'"
            ),
            1
        );
    }

    #[test]
    fn a_policy_that_does_not_ask_the_owner_never_creates_a_choice_request() {
        let fixture = Fixture::new();
        fixture.seed(
            &json!({
                "mode": "ALLOW_LISTED",
                "triggers": ["QUOTA_EXHAUSTED"],
                "fallback_agent_binding_ids": [FALLBACK_1],
                "max_lead_changes": 1
            }),
            "RUNNING",
        );
        let error = fixture
            .store
            .commit_lead_failover_choice_request(commit(
                "ask-allow-listed",
                observation("quota-allowed"),
            ))
            .expect_err("an automatic policy never asks the owner");
        assert!(matches!(error, StoreError::Invalid(_)));
        let connection = fixture.connection();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM user_requests"), 0);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM domain_events WHERE type = 'user.request.created.v1'"
            ),
            0
        );
    }

    #[test]
    fn without_a_planning_session_no_choice_request_is_created() {
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "RUNNING");
        fixture
            .connection()
            .execute("DELETE FROM agent_sessions", [])
            .expect("clear the planning session");
        let error = fixture
            .store
            .commit_lead_failover_choice_request(commit(
                "ask-no-session",
                observation("quota-no-session"),
            ))
            .expect_err("a Task without a planning session cannot scope the request");
        assert!(matches!(error, StoreError::Invalid(_)));
        let connection = fixture.connection();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM user_requests"), 0);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM domain_events WHERE type = 'user.request.created.v1'"
            ),
            0
        );
    }

    #[test]
    fn a_stale_task_version_or_spec_revision_is_rejected() {
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "RUNNING");
        let mut stale_task = commit("ask-stale-task", observation("quota-stale-task"));
        stale_task.expected_task_version = 9;
        let error = fixture
            .store
            .commit_lead_failover_choice_request(stale_task)
            .expect_err("a stale Task version is rejected");
        assert!(matches!(error, StoreError::Conflict { .. }));

        let mut stale_spec = commit("ask-stale-spec", observation("quota-stale-spec"));
        stale_spec.expected_task_spec_revision = 9;
        let error = fixture
            .store
            .commit_lead_failover_choice_request(stale_spec)
            .expect_err("a stale TaskSpec revision is rejected");
        assert!(matches!(&error, StoreError::Invalid(code) if code == "STALE_SPEC_REVISION"));
        let connection = fixture.connection();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM user_requests"), 0);
    }

    #[test]
    fn an_ineligible_fallback_is_not_offered_to_the_owner() {
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "RUNNING");
        fixture
            .connection()
            .execute(
                "UPDATE agent_bindings SET enabled = 0 WHERE agent_binding_id = ?1",
                [FALLBACK_1],
            )
            .expect("disable the first fallback");
        let receipt = fixture
            .store
            .commit_lead_failover_choice_request(commit(
                "ask-ineligible",
                observation("quota-ineligible"),
            ))
            .expect("the remaining eligible fallback is still offered");
        assert_eq!(receipt.fallback_agent_binding_ids, vec![FALLBACK_2]);
    }

    #[test]
    fn a_task_that_cannot_be_answered_holds_no_choice_request() {
        let fixture = Fixture::new();
        fixture.seed(&ask_policy(), "VERIFYING");
        let error = fixture
            .store
            .commit_lead_failover_choice_request(commit("ask-verify", observation("quota-verify")))
            .expect_err("a Task that cannot hold an answerable request is rejected");
        assert!(matches!(error, StoreError::Invalid(_)));
        let connection = fixture.connection();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM user_requests"), 0);
    }
}
