//! Authenticated Suggestions projection and event-backed expiry settlement.
//!
//! Expiry is a bounded system transition. Every item is independently atomic with
//! its canonical event, aggregate snapshot reference, and idempotency receipt. Other
//! Suggestion owner mutations and atomic TASK-proposal admission are implemented here;
//! proposal generation remains outside this adapter.

use super::*;
use domain_responsibility::{PrincipalKind, PrincipalRef, Suggestion, SuggestionAction, SuggestionExpiryStore, SuggestionGoalRef, SuggestionKind, SuggestionLatencyClass, SuggestionOwnerAction, SuggestionOwnerActionStore, SuggestionPage, SuggestionPreference, SuggestionPreferenceStore, SuggestionResourceRef, SuggestionServiceError, SuggestionServiceRef, SuggestionStatus, SuggestionVisibility, SUGGESTION_SERVICE_PRINCIPAL_ID};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestionReadError {
    Unauthorized,
    InvalidRequest,
    ExpiryPending,
    Storage,
}
impl std::fmt::Display for SuggestionReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for SuggestionReadError {}

#[derive(Clone, Debug)]
pub struct SuggestionEventContext {
    pub origin_runtime_id: String,
    pub correlation_id: String,
}

#[derive(Clone)]
pub struct SqliteSuggestionStore {
    store: SqliteWorkspaceStore,
    event_context: Option<SuggestionEventContext>,
}

impl SqliteSuggestionStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self { Self { store, event_context: None } }

    pub fn with_event_context(
        store: SqliteWorkspaceStore,
        event_context: SuggestionEventContext,
    ) -> Result<Self, SuggestionServiceError> {
        if event_context.origin_runtime_id.trim().is_empty() || event_context.correlation_id.trim().is_empty() {
            return Err(SuggestionServiceError::InvalidRequest);
        }
        Ok(Self { store, event_context: Some(event_context) })
    }

    pub fn list(
        &self,
        principal_id: &str,
        workspace_id: &str,
        status: Option<SuggestionStatus>,
        visibility: SuggestionVisibility,
        now: &str,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<SuggestionPage, SuggestionReadError> {
        if principal_id.trim().is_empty() || workspace_id.trim().is_empty() || now.trim().is_empty()
            || !(1..=200).contains(&limit)
            || after.as_ref().is_some_and(|(time, id)| time.is_empty() || id.is_empty())
        { return Err(SuggestionReadError::InvalidRequest); }
        let (principal, workspace, now) = (principal_id.to_owned(), workspace_id.to_owned(), now.to_owned());
        let after = after.map(|(time, id)| canonicalize_utc_timestamp(&time).map(|time| (time, id)))
            .transpose().map_err(|_| SuggestionReadError::InvalidRequest)?;
        let status = status.map(status_name);
        let visibility = visibility_name(visibility);
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::SuggestionOperation { operation: Box::new(move |connection| {
                let result = (|| {
                    let tx = connection.transaction().map_err(|_| SuggestionReadError::Storage)?;
                    authorize_owner(&tx, &principal, &workspace)?;
                    let expiry_pending: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM suggestions WHERE workspace_id = ?1 AND status = 'PROPOSED' AND expires_at <= ?2)",
                        params![workspace, now], |row| row.get(0),
                    ).map_err(|_| SuggestionReadError::Storage)?;
                    if expiry_pending { return Err(SuggestionReadError::ExpiryPending); }
                    let mut statement = tx.prepare(
                        "SELECT suggestion_id FROM suggestions WHERE workspace_id = ?1 AND (?2 IS NULL OR status = ?2) AND (status <> 'PROPOSED' OR expires_at > ?3) AND (status <> 'PROPOSED' OR ?4 = 'ALL' OR (?4 = 'VISIBLE' AND (snoozed_until IS NULL OR snoozed_until <= ?3)) OR (?4 = 'SNOOZED' AND snoozed_until > ?3)) AND (?5 IS NULL OR created_at < ?5 OR (created_at = ?5 AND suggestion_id < ?6)) ORDER BY created_at DESC, suggestion_id DESC LIMIT ?7"
                    ).map_err(|_| SuggestionReadError::Storage)?;
                    let ids = statement.query_map(
                        params![workspace, status, now, visibility, after.as_ref().map(|value| value.0.as_str()), after.as_ref().map(|value| value.1.as_str()), (limit + 1) as i64],
                        |row| row.get::<_, String>(0),
                    ).map_err(|_| SuggestionReadError::Storage)?
                        .collect::<rusqlite::Result<Vec<_>>>().map_err(|_| SuggestionReadError::Storage)?;
                    let more = ids.len() > limit;
                    let mut items = Vec::new();
                    for id in ids.into_iter().take(limit) {
                        items.push(load_suggestion(&tx, &workspace, &id)?);
                    }
                    let next = if more { items.last().map(|item| (item.created_at.clone(), item.suggestion_id.clone())) } else { None };
                    Ok(SuggestionPage { items, next })
                })();
                let _ = reply.send(result);
            }) },
            receive,
        ).map_err(|_| SuggestionReadError::Storage)?
    }

    pub fn get(&self, principal_id: &str, workspace_id: &str, suggestion_id: &str) -> Result<Option<Suggestion>, SuggestionReadError> {
        if principal_id.trim().is_empty() || workspace_id.trim().is_empty() || suggestion_id.trim().is_empty() {
            return Err(SuggestionReadError::InvalidRequest);
        }
        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let id = suggestion_id.to_owned();
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::SuggestionOperation { operation: Box::new(move |connection| {
                let result = (|| {
                    let tx = connection.transaction().map_err(|_| SuggestionReadError::Storage)?;
                    authorize_owner(&tx, &principal, &workspace)?;
                    load_suggestion_optional(&tx, &workspace, &id)
                })();
                let _ = reply.send(result);
            }) }, receive,
        ).map_err(|_| SuggestionReadError::Storage)?
    }
}

impl SuggestionPreferenceStore for SqliteSuggestionStore {
    fn list_kind_preferences(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<SuggestionPreference>, SuggestionServiceError> {
        if owner_principal_id.trim().is_empty() || workspace_id.trim().is_empty() {
            return Err(SuggestionServiceError::InvalidRequest);
        }
        let (owner, workspace) = (owner_principal_id.to_owned(), workspace_id.to_owned());
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::SuggestionOperation { operation: Box::new(move |connection| {
                let result = (|| {
                    let tx = connection.transaction().map_err(|_| SuggestionServiceError::Storage)?;
                    authorize_active_suggestion_owner(&tx, &owner, &workspace)?;
                    let mut preferences = vec![
                        default_preference(&workspace, SuggestionKind::TaskOpportunity),
                        default_preference(&workspace, SuggestionKind::RoutineOpportunity),
                        default_preference(&workspace, SuggestionKind::AutomationOpportunity),
                    ];
                    let mut statement = tx.prepare(
                        "SELECT kind, muted, updated_at, version FROM suggestion_preferences WHERE workspace_id = ?1",
                    ).map_err(|_| SuggestionServiceError::Storage)?;
                    let rows = statement.query_map(params![workspace], |row| Ok((
                        row.get::<_, String>(0)?, row.get::<_, bool>(1)?, row.get::<_, String>(2)?, row.get::<_, u64>(3)?,
                    ))).map_err(|_| SuggestionServiceError::Storage)?
                        .collect::<rusqlite::Result<Vec<_>>>().map_err(|_| SuggestionServiceError::Storage)?;
                    for (kind, muted, updated_at, version) in rows {
                        let kind = parse_kind(&kind).map_err(map_read_error)?;
                        if let Some(pref) = preferences.iter_mut().find(|pref| pref.kind == kind) {
                            *pref = SuggestionPreference { workspace_id: workspace.clone(), kind, muted, updated_at: Some(updated_at), version };
                        }
                    }
                    Ok(preferences)
                })();
                let _ = reply.send(result);
            }) }, receive,
        ).map_err(|_| SuggestionServiceError::Storage)?
    }

    fn set_kind_preference(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
        kind: SuggestionKind,
        muted: bool,
        expected_version: u64,
        request_id: &str,
        as_of: &str,
    ) -> Result<SuggestionPreference, SuggestionServiceError> {
        if owner_principal_id.trim().is_empty() || workspace_id.trim().is_empty()
            || request_id.trim().is_empty() || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        { return Err(SuggestionServiceError::InvalidRequest); }
        let context = self.event_context.clone().ok_or(SuggestionServiceError::InvalidRequest)?;
        let as_of = canonicalize_utc_timestamp(as_of).map_err(|_| SuggestionServiceError::InvalidRequest)?;
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let request_id = request_id.to_owned();
        let kind_name = kind_name(kind).to_owned();
        let fingerprint = digest(&canonical_json(&json!({
            "workspace_id": workspace,
            "kind": kind_name,
            "muted": muted,
            "expected_version": expected_version,
        })).map_err(|_| SuggestionServiceError::InvalidRequest)?);
        let blobs = Arc::clone(&self.store.inner.blobs);
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::SuggestionOperation { operation: Box::new(move |connection| {
                let result = execute_preference_change(
                    connection, blobs.as_ref(), &owner, &workspace, kind, muted,
                    expected_version, &request_id, &fingerprint, &as_of, &context,
                );
                let _ = reply.send(result);
            }) }, receive,
        ).map_err(|_| SuggestionServiceError::Storage)?
    }
}

impl storage_core::SuggestionTaskAcceptanceStore for SqliteWorkspaceStore {
    fn create_task_from_suggestion(
        &self,
        mut commit: storage_core::SuggestedTaskCreateCommit,
    ) -> Result<storage_core::CommittedTask, StoreError> {
        if commit.suggestion_id.trim().is_empty() || commit.expected_suggestion_version == 0
            || commit.task.request.principal_id.trim().is_empty()
            || commit.task.request.request_id.trim().is_empty()
        { return Err(StoreError::Invalid("Suggestion acceptance identity is invalid".to_owned())); }

        super::canonicalize_task_timestamps(&mut commit.task)?;
        commit.accepted_at = super::canonicalize_utc_timestamp(&commit.accepted_at)?;
        commit.suggestion_event.recorded_at = super::canonicalize_utc_timestamp(&commit.suggestion_event.recorded_at)?;
        super::validate_task_create_commit(&commit.task)?;
        validate_suggestion_acceptance_commit(&commit)?;

        let workspace = commit.task.task.workspace_id.clone();
        let owner = commit.task.request.principal_id.clone();
        let suggestion_id = commit.suggestion_id.clone();
        let current = SqliteSuggestionStore::new(self.clone()).get(&owner, &workspace, &suggestion_id)
            .map_err(|error| match error {
                SuggestionReadError::Unauthorized => StoreError::Invalid("authenticated Principal does not own this Workspace".to_owned()),
                SuggestionReadError::InvalidRequest => StoreError::Invalid("Suggestion acceptance is invalid".to_owned()),
                _ => StoreError::NotFound,
            })?.ok_or(StoreError::NotFound)?;
        // A retry may arrive after the first transaction committed but before its
        // response reached the desktop. Keep the persisted accepted value around so
        // the transaction can validate the Task receipt and return the original Task.
        // Any fresh request against an already accepted Suggestion will fail inside
        // the same transaction and roll back its newly attempted Task insert.
        let accepted = match current.status {
            SuggestionStatus::Proposed => prepare_task_acceptance(&current, &commit)?,
            SuggestionStatus::Accepted => current.clone(),
            _ => return Err(StoreError::Conflict { expected: Some(commit.expected_suggestion_version), actual: Some(current.version) }),
        };

        let task_snapshot = storage_core::TaskAggregateSnapshot {
            task: commit.task.task.clone(),
            current_spec_revision: commit.task.initial_spec_revision.clone(),
            current_plan_revision: None,
            current_steps: Vec::new(),
        };
        let task_state_bytes = super::canonical_json(&task_snapshot)?;
        let task_blob = self.inner.blobs.put(&workspace, BlobPurpose::AggregateState, &task_state_bytes, super::TASK_STATE_MEDIA_TYPE)?;
        if task_blob.size_bytes != task_state_bytes.len() as u64
            || self.inner.blobs.get(&workspace, BlobPurpose::AggregateState, &task_blob)? != task_state_bytes
        { return Err(StoreError::Integrity("Task aggregate state failed blob verification".to_owned())); }
        let task_state_ref = AggregateStateRef { blob: task_blob, entity_revision: commit.task.task.version, record_schema_version: 1 };

        let suggestion_state_bytes = super::canonical_json(&accepted)?;
        let suggestion_blob = self.inner.blobs.put(&workspace, BlobPurpose::AggregateState, &suggestion_state_bytes, "application/vnd.litecow.suggestion+json")?;
        if suggestion_blob.digest != super::digest(&suggestion_state_bytes)
            || suggestion_blob.size_bytes != suggestion_state_bytes.len() as u64
            || self.inner.blobs.get(&workspace, BlobPurpose::AggregateState, &suggestion_blob)? != suggestion_state_bytes
        { return Err(StoreError::Integrity("Suggestion aggregate state failed blob verification".to_owned())); }

        let event = commit.suggestion_event.clone();
        let expected_version = commit.expected_suggestion_version;
        let accepted_at = commit.accepted_at.clone();
        let context = SuggestionEventContext { origin_runtime_id: event.origin_runtime_id.clone(), correlation_id: event.correlation_id.clone() };
        let state_blob = suggestion_blob;
        let current_before = current;
        let task_commit = commit.task;
        let idempotency_context = serde_json::json!({
            "operation": "suggestion.accept_task.v1",
            "suggestion_id": suggestion_id,
            "expected_suggestion_version": expected_version,
        });
        let (reply, receive) = mpsc::channel();
        self.execute_command(
            Command::SuggestionOperation { operation: Box::new(move |connection| {
                let result = (|| {
                    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_database_error)?;
                    super::authorize_active_suggestion_owner(&tx, &owner, &workspace).map_err(map_suggestion_store_error)?;
                    let current = load_suggestion_optional(&tx, &workspace, &suggestion_id)
                        .map_err(|error| map_suggestion_store_error(map_read_error(error)))?.ok_or(StoreError::NotFound)?;
                    let created = super::create_task_in_transaction(&tx, task_commit, task_state_ref, Some(idempotency_context))?;

                    // The idempotency receipt is sufficient only because Task and Suggestion
                    // transition share this transaction. A replay must prove the linked state.
                    if current.status == SuggestionStatus::Accepted {
                        if current.result_task_id.as_deref() != Some(created.view.task.task_id.as_str())
                            || Some(current.version) != expected_version.checked_add(1)
                        { return Err(StoreError::Integrity("Suggestion acceptance receipt does not match its durable Task".to_owned())); }
                        tx.commit().map_err(map_database_error)?;
                        return Ok(created);
                    }
                    if current != current_before || current.status != SuggestionStatus::Proposed
                        || current.version != expected_version || current.expires_at <= accepted_at
                    {
                        return Err(StoreError::Conflict { expected: Some(expected_version), actual: Some(current.version) });
                    }
                    let resolved_by = json!({ "principal_id": owner, "kind": "USER" });
                    let changed = tx.execute(
                        "UPDATE suggestions SET status = 'ACCEPTED', snoozed_until = NULL, resolved_at = ?1, resolved_by_json = ?2, resolution_reason = 'ACCEPTED_BY_OWNER', result_task_id = ?3, version = ?4 WHERE workspace_id = ?5 AND suggestion_id = ?6 AND status = 'PROPOSED' AND version = ?7 AND expires_at > ?1",
                        params![accepted_at, encode(&resolved_by).map_err(map_suggestion_store_error)?, created.view.task.task_id, accepted.version, workspace, suggestion_id, expected_version],
                    ).map_err(map_database_error)?;
                    if changed != 1 { return Err(StoreError::Conflict { expected: Some(expected_version), actual: Some(current.version) }); }
                    let persisted = load_suggestion(&tx, &workspace, &suggestion_id)
                        .map_err(|error| map_suggestion_store_error(map_read_error(error)))?;
                    if persisted != accepted { return Err(StoreError::Integrity("accepted Suggestion does not match its snapshot".to_owned())); }
                    write_suggestion_event(
                        &tx, &workspace, &context, &accepted_at, &event.event_id, &suggestion_id,
                        accepted.version, "suggestion.resolved.v1", event.payload.clone(), state_blob,
                    ).map_err(|_| StoreError::Integrity("Suggestion event could not be committed".to_owned()))?;
                    tx.commit().map_err(map_database_error)?;
                    Ok(created)
                })();
                let _ = reply.send(result);
            }) }, receive,
        )?
    }
}

struct PreparedPreferenceChange {
    preference: SuggestionPreference,
    prior_preference_version: u64,
    prior_muted: bool,
    prior_suggestions: Vec<Suggestion>,
    resolved_suggestions: Vec<Suggestion>,
}

fn execute_preference_change(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    owner: &str,
    workspace: &str,
    kind: SuggestionKind,
    muted: bool,
    expected_version: u64,
    request_id: &str,
    fingerprint: &str,
    as_of: &str,
    context: &SuggestionEventContext,
) -> Result<SuggestionPreference, SuggestionServiceError> {
    let prepared = {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| SuggestionServiceError::Storage)?;
        authorize_active_suggestion_owner(&tx, owner, workspace)?;
        if let Some(prior) = preference_receipt(&tx, owner, request_id, fingerprint, workspace, kind)? {
            return Ok(prior);
        }
        let current = load_preference(&tx, workspace, kind)?;
        let current_version = current.as_ref().map(|value| value.version).unwrap_or(0);
        if current_version != expected_version { return Err(SuggestionServiceError::VersionConflict); }
        let next_version = expected_version.checked_add(1).ok_or(SuggestionServiceError::Storage)?;
        let preference = SuggestionPreference {
            workspace_id: workspace.to_owned(), kind, muted, updated_at: Some(as_of.to_owned()), version: next_version,
        };
        let prior_muted = current.as_ref().is_some_and(|value| value.muted);
        let prior_suggestions = load_proposed_kind(&tx, workspace, kind, as_of)?;
        let resolved_suggestions = if muted {
            prior_suggestions.iter().cloned().map(|item| prepare_muted_suggestion(item, owner, as_of)).collect::<Result<Vec<_>, _>>()?
        } else { Vec::new() };
        PreparedPreferenceChange {
            preference,
            prior_preference_version: current_version,
            prior_muted,
            prior_suggestions,
            resolved_suggestions,
        }
    };

    let preference_bytes = canonical_json(&prepared.preference).map_err(|_| SuggestionServiceError::Storage)?;
    let preference_blob = blobs.put(workspace, BlobPurpose::AggregateState, &preference_bytes, "application/vnd.litecow.suggestion-preference+json")
        .map_err(|_| SuggestionServiceError::Storage)?;
    if preference_blob.digest != digest(&preference_bytes) || preference_blob.size_bytes != preference_bytes.len() as u64
        || blobs.get(workspace, BlobPurpose::AggregateState, &preference_blob).map_err(|_| SuggestionServiceError::Storage)? != preference_bytes
    { return Err(SuggestionServiceError::Storage); }

    let mut suggestion_blobs = Vec::with_capacity(prepared.resolved_suggestions.len());
    for suggestion in &prepared.resolved_suggestions {
        let bytes = canonical_json(suggestion).map_err(|_| SuggestionServiceError::Storage)?;
        let blob = blobs.put(workspace, BlobPurpose::AggregateState, &bytes, "application/vnd.litecow.suggestion+json")
            .map_err(|_| SuggestionServiceError::Storage)?;
        if blob.digest != digest(&bytes) || blob.size_bytes != bytes.len() as u64
            || blobs.get(workspace, BlobPurpose::AggregateState, &blob).map_err(|_| SuggestionServiceError::Storage)? != bytes
        { return Err(SuggestionServiceError::Storage); }
        suggestion_blobs.push(blob);
    }

    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| SuggestionServiceError::Storage)?;
    authorize_active_suggestion_owner(&tx, owner, workspace)?;
    if let Some(prior) = preference_receipt(&tx, owner, request_id, fingerprint, workspace, kind)? {
        return Ok(prior);
    }
    let current = load_preference(&tx, workspace, kind)?;
    if current.as_ref().map(|value| value.version).unwrap_or(0) != prepared.prior_preference_version {
        return Err(SuggestionServiceError::VersionConflict);
    }
    let current_suggestions = load_proposed_kind(&tx, workspace, kind, as_of)?;
    if current_suggestions != prepared.prior_suggestions {
        return Err(SuggestionServiceError::VersionConflict);
    }

    match current {
        Some(_) => {
            let changed = tx.execute(
                "UPDATE suggestion_preferences SET muted = ?1, updated_by_json = ?2, updated_at = ?3, version = ?4 WHERE workspace_id = ?5 AND kind = ?6 AND version = ?7",
                params![muted, encode(&PrincipalRef { principal_id: owner.to_owned(), kind: PrincipalKind::User })?, as_of, prepared.preference.version, workspace, kind_name(kind), expected_version],
            ).map_err(|_| SuggestionServiceError::Storage)?;
            if changed != 1 { return Err(SuggestionServiceError::VersionConflict); }
        }
        None => {
            if expected_version != 0 { return Err(SuggestionServiceError::VersionConflict); }
            tx.execute(
                "INSERT INTO suggestion_preferences(workspace_id, kind, muted, updated_by_json, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![workspace, kind_name(kind), muted, encode(&PrincipalRef { principal_id: owner.to_owned(), kind: PrincipalKind::User })?, as_of, prepared.preference.version],
            ).map_err(|_| SuggestionServiceError::VersionConflict)?;
        }
    }

    for ((suggestion, expected), blob) in prepared.resolved_suggestions.iter().zip(&prepared.prior_suggestions).zip(&suggestion_blobs) {
        let resolved_by = suggestion.resolved_by.as_ref().ok_or(SuggestionServiceError::Storage)?;
        let changed = tx.execute(
            "UPDATE suggestions SET status = 'DISMISSED', snoozed_until = NULL, resolved_at = ?1, resolved_by_json = ?2, resolution_reason = 'MUTED_KIND', version = ?3 WHERE workspace_id = ?4 AND suggestion_id = ?5 AND status = 'PROPOSED' AND version = ?6 AND expires_at > ?1 AND kind = ?7",
            params![as_of, encode(resolved_by)?, suggestion.version, workspace, suggestion.suggestion_id, expected.version, kind_name(kind)],
        ).map_err(|_| SuggestionServiceError::Storage)?;
        if changed != 1 { return Err(SuggestionServiceError::VersionConflict); }
        let persisted = load_suggestion(&tx, workspace, &suggestion.suggestion_id).map_err(map_read_error)?;
        if persisted != *suggestion { return Err(SuggestionServiceError::Storage); }
        let event_id = deterministic_preference_event_id(owner, request_id, &format!("suggestion:{}", suggestion.suggestion_id));
        let payload = json!({
            "suggestion_id": suggestion.suggestion_id,
            "from": "PROPOSED",
            "to": "DISMISSED",
            "resolved_by": resolved_by,
            "resolution_reason": "MUTED_KIND",
            "aggregate_version": suggestion.version,
        });
        write_suggestion_event(&tx, workspace, context, as_of, &event_id, &suggestion.suggestion_id, suggestion.version,
            "suggestion.resolved.v1", payload, blob.clone())?;
    }

    let preference_event_id = deterministic_preference_event_id(owner, request_id, &format!("preference:{}", kind_name(kind)));
    let preference_entity_id = preference_entity_id(kind);
    let preference_payload = json!({
        "workspace_id": workspace,
        "kind": kind_name(kind),
        "from_muted": prepared.prior_muted,
        "to_muted": muted,
        "changed_by": PrincipalRef { principal_id: owner.to_owned(), kind: PrincipalKind::User },
        "aggregate_version": prepared.preference.version,
    });
    write_domain_event(&tx, workspace, context, as_of, &preference_event_id, "SuggestionPreference", &preference_entity_id,
        prepared.preference.version, "suggestion.preference.changed.v1", preference_payload, preference_blob)?;

    let response = encode(&prepared.preference)?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![owner, request_id, fingerprint, response, digest(response.as_bytes()), as_of],
    ).map_err(|_| SuggestionServiceError::IdempotencyConflict)?;
    tx.commit().map_err(|_| SuggestionServiceError::Storage)?;
    Ok(prepared.preference)
}

fn prepare_muted_suggestion(mut suggestion: Suggestion, owner: &str, as_of: &str) -> Result<Suggestion, SuggestionServiceError> {
    suggestion.status = SuggestionStatus::Dismissed;
    suggestion.snoozed_until = None;
    suggestion.resolved_at = Some(as_of.to_owned());
    suggestion.resolved_by = Some(PrincipalRef { principal_id: owner.to_owned(), kind: PrincipalKind::User });
    suggestion.resolution_reason = Some("MUTED_KIND".to_owned());
    suggestion.result_task_id = None;
    suggestion.version = suggestion.version.checked_add(1).ok_or(SuggestionServiceError::Storage)?;
    Ok(suggestion)
}

fn load_proposed_kind(tx: &Transaction<'_>, workspace: &str, kind: SuggestionKind, as_of: &str) -> Result<Vec<Suggestion>, SuggestionServiceError> {
    let mut statement = tx.prepare("SELECT suggestion_id FROM suggestions WHERE workspace_id = ?1 AND kind = ?2 AND status = 'PROPOSED' AND expires_at > ?3 ORDER BY created_at, suggestion_id")
        .map_err(|_| SuggestionServiceError::Storage)?;
    let ids = statement.query_map(params![workspace, kind_name(kind), as_of], |row| row.get::<_, String>(0))
        .map_err(|_| SuggestionServiceError::Storage)?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|_| SuggestionServiceError::Storage)?;
    ids.into_iter().map(|id| load_suggestion(tx, workspace, &id).map_err(map_read_error)).collect()
}

fn load_preference(connection: &Connection, workspace: &str, kind: SuggestionKind) -> Result<Option<SuggestionPreference>, SuggestionServiceError> {
    let row: Option<(bool, String, u64)> = connection.query_row(
        "SELECT muted, updated_at, version FROM suggestion_preferences WHERE workspace_id = ?1 AND kind = ?2",
        params![workspace, kind_name(kind)], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(|_| SuggestionServiceError::Storage)?;
    Ok(row.map(|(muted, updated_at, version)| SuggestionPreference { workspace_id: workspace.to_owned(), kind, muted, updated_at: Some(updated_at), version }))
}

fn preference_receipt(
    tx: &Transaction<'_>, owner: &str, request_id: &str, fingerprint: &str,
    workspace: &str, kind: SuggestionKind,
) -> Result<Option<SuggestionPreference>, SuggestionServiceError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![owner, request_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(|_| SuggestionServiceError::Storage)?;
    let Some((actual, response, response_digest)) = prior else { return Ok(None); };
    if actual != fingerprint { return Err(SuggestionServiceError::IdempotencyConflict); }
    let response = response.ok_or(SuggestionServiceError::Storage)?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(SuggestionServiceError::Storage);
    }
    let preference: SuggestionPreference = serde_json::from_str(&response).map_err(|_| SuggestionServiceError::Storage)?;
    if preference.workspace_id != workspace || preference.kind != kind { return Err(SuggestionServiceError::Storage); }
    Ok(Some(preference))
}

fn default_preference(workspace: &str, kind: SuggestionKind) -> SuggestionPreference {
    SuggestionPreference { workspace_id: workspace.to_owned(), kind, muted: false, updated_at: None, version: 0 }
}

fn preference_entity_id(kind: SuggestionKind) -> String {
    kind_name(kind).to_owned()
}

fn deterministic_preference_event_id(owner: &str, request_id: &str, key: &str) -> String {
    format!("ev_{}", &digest(format!("LiteCowork/SuggestionPreference/v1\0{owner}\0{request_id}\0{key}").as_bytes())[7..])
}

fn prepare_task_acceptance(
    suggestion: &Suggestion,
    commit: &storage_core::SuggestedTaskCreateCommit,
) -> Result<Suggestion, StoreError> {
    validate_suggestion_acceptance_commit(commit)?;
    if suggestion.workspace_id != commit.task.task.workspace_id
        || suggestion.suggestion_id != commit.suggestion_id
        || suggestion.proposed_action != SuggestionAction::Task
        || suggestion.status != SuggestionStatus::Proposed
        || suggestion.version != commit.expected_suggestion_version
        || suggestion.expires_at <= commit.accepted_at
    { return Err(StoreError::Conflict { expected: Some(commit.expected_suggestion_version), actual: Some(suggestion.version) }); }

    let spec = &commit.task.initial_spec_revision;
    let proposal = suggestion.proposed_task_spec.as_ref().and_then(serde_json::Value::as_object)
        .ok_or_else(|| StoreError::Invalid("TASK Suggestion has no valid TaskSpec proposal".to_owned()))?;
    let proposed_string = |field: &str| proposal.get(field).and_then(serde_json::Value::as_str);
    let proposed_array = |field: &str| proposal.get(field).and_then(serde_json::Value::as_array);
    let expected_array = |field: &str, actual: serde_json::Value| {
        proposed_array(field).is_some_and(|value| value == &actual)
    };
    let proposed_category = match proposal.get("task_category") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(value)) => Some(value.as_str()),
        _ => return Err(StoreError::Invalid("Task proposal category is invalid".to_owned())),
    };
    let proposed_budget = match proposal.get("budget") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(value),
    };
    let proposed_delegation_budget = match proposal.get("delegation_budget_policy") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(value),
    };
    let proposed_deadline = match proposal.get("deadline") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(value)) => Some(super::canonicalize_utc_timestamp(value)?),
        _ => return Err(StoreError::Invalid("Task proposal deadline is invalid".to_owned())),
    };
    if proposed_string("objective") != Some(spec.objective.as_str())
        || proposed_category != spec.task_category.as_deref()
        || !expected_array("constraints", json!(&spec.constraints))
        || !expected_array("input_refs", json!(&spec.input_refs))
        || !expected_array("required_outputs", json!(&spec.required_outputs))
        || !expected_array("acceptance_criteria", json!(&spec.acceptance_criteria))
        || proposed_budget != spec.budget.as_ref()
        || proposed_delegation_budget != spec.delegation_budget_policy.as_ref()
        || proposed_deadline != spec.deadline
    { return Err(StoreError::Invalid("Task does not preserve the exact accepted proposal".to_owned())); }

    // Source references are pinned explanatory provenance and must remain exact Task inputs.
    for source in &suggestion.source_refs {
        if source.workspace_id != suggestion.workspace_id
            || !spec.input_refs.iter().any(|value| {
                value.get("workspace_id").and_then(serde_json::Value::as_str) == Some(source.workspace_id.as_str())
                    && value.get("resource_id").and_then(serde_json::Value::as_str) == Some(source.resource_id.as_str())
                    && value.get("revision_id").and_then(serde_json::Value::as_str) == Some(source.revision_id.as_str())
            })
        { return Err(StoreError::Invalid("Task inputs do not preserve every pinned Suggestion source".to_owned())); }
    }

    let next_version = commit.expected_suggestion_version.checked_add(1)
        .ok_or_else(|| StoreError::Integrity("Suggestion version exhausted".to_owned()))?;
    let event = &commit.suggestion_event;
    let expected_payload = json!({
        "suggestion_id": suggestion.suggestion_id,
        "from": "PROPOSED",
        "to": "ACCEPTED",
        "resolved_by": { "principal_id": commit.task.request.principal_id, "kind": "USER" },
        "resolution_reason": "ACCEPTED_BY_OWNER",
        "result_task_id": commit.task.task.task_id,
        "aggregate_version": next_version,
    });
    if event.workspace_id != suggestion.workspace_id || event.entity_type != "Suggestion"
        || event.entity_id != suggestion.suggestion_id || event.entity_revision != next_version
        || event.schema_version != 1 || event.event_type != "suggestion.resolved.v1"
        || event.payload != expected_payload || event.recorded_at != commit.accepted_at
        || event.origin_runtime_id != commit.task.event.origin_runtime_id
        || event.correlation_id != commit.task.event.correlation_id
        || event.event_id == commit.task.event.event_id
    { return Err(StoreError::Invalid("Suggestion acceptance event is inconsistent".to_owned())); }

    let mut accepted = suggestion.clone();
    accepted.status = SuggestionStatus::Accepted;
    accepted.snoozed_until = None;
    accepted.resolved_at = Some(commit.accepted_at.clone());
    accepted.resolved_by = Some(PrincipalRef { principal_id: commit.task.request.principal_id.clone(), kind: PrincipalKind::User });
    accepted.resolution_reason = Some("ACCEPTED_BY_OWNER".to_owned());
    accepted.result_task_id = Some(commit.task.task.task_id.clone());
    accepted.version = next_version;
    Ok(accepted)
}

fn validate_suggestion_acceptance_commit(
    commit: &storage_core::SuggestedTaskCreateCommit,
) -> Result<(), StoreError> {
    let event = &commit.suggestion_event;
    let next_version = commit.expected_suggestion_version.checked_add(1);
    if commit.expected_suggestion_version == 0
        || next_version.is_none()
        || commit.task.task.workspace_id.trim().is_empty()
        || commit.task.task.task_id.trim().is_empty()
        || commit.suggestion_id.trim().is_empty()
        || commit.task.request.principal_id.trim().is_empty()
        || commit.task.event.recorded_at != commit.accepted_at
        || event.recorded_at != commit.accepted_at
        || event.workspace_id != commit.task.task.workspace_id
        || event.entity_type != "Suggestion"
        || event.entity_id != commit.suggestion_id
        || event.entity_revision != next_version.unwrap_or_default()
        || event.event_type != "suggestion.resolved.v1"
        || event.payload.get("suggestion_id").and_then(serde_json::Value::as_str) != Some(commit.suggestion_id.as_str())
        || event.payload.get("to").and_then(serde_json::Value::as_str) != Some("ACCEPTED")
        || event.payload.get("result_task_id").and_then(serde_json::Value::as_str) != Some(commit.task.task.task_id.as_str())
    { return Err(StoreError::Invalid("Suggestion acceptance commit is inconsistent".to_owned())); }
    Ok(())
}

fn map_suggestion_store_error(error: SuggestionServiceError) -> StoreError {
    match error {
        SuggestionServiceError::Unauthorized => StoreError::Invalid("authenticated Principal does not own this Workspace".to_owned()),
        SuggestionServiceError::NotFound => StoreError::NotFound,
        SuggestionServiceError::VersionConflict | SuggestionServiceError::Expired => StoreError::Conflict { expected: None, actual: None },
        SuggestionServiceError::InvalidRequest | SuggestionServiceError::InvalidTransition => StoreError::Invalid("Suggestion cannot be accepted in its current state".to_owned()),
        SuggestionServiceError::ClockUnavailable | SuggestionServiceError::IdempotencyConflict | SuggestionServiceError::ExpiryPending | SuggestionServiceError::Storage => StoreError::Integrity("Suggestion acceptance storage failed".to_owned()),
    }
}

impl SuggestionExpiryStore for SqliteSuggestionStore {
    fn settle_expired(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
        service_principal_id: &str,
        as_of: &str,
        limit: usize,
    ) -> Result<(usize, bool), SuggestionServiceError> {
        if owner_principal_id.trim().is_empty() || workspace_id.trim().is_empty()
            || service_principal_id != SUGGESTION_SERVICE_PRINCIPAL_ID || as_of.trim().is_empty()
            || !(1..=200).contains(&limit)
        { return Err(SuggestionServiceError::InvalidRequest); }
        let context = self.event_context.clone().ok_or(SuggestionServiceError::InvalidRequest)?;
        let as_of = canonicalize_utc_timestamp(as_of).map_err(|_| SuggestionServiceError::InvalidRequest)?;
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let service = service_principal_id.to_owned();
        let blobs = Arc::clone(&self.store.inner.blobs);
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::SuggestionOperation { operation: Box::new(move |connection| {
                let result = settle_expired_bounded(
                    connection, blobs.as_ref(), &owner, &workspace, &service,
                    &as_of, &context, limit,
                );
                let _ = reply.send(result);
            }) },
            receive,
        ).map_err(|_| SuggestionServiceError::Storage)?
    }
}

impl SuggestionOwnerActionStore for SqliteSuggestionStore {
    fn apply_owner_action(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
        suggestion_id: &str,
        expected_version: u64,
        request_id: &str,
        as_of: &str,
        action: SuggestionOwnerAction,
    ) -> Result<Suggestion, SuggestionServiceError> {
        if owner_principal_id.trim().is_empty() || workspace_id.trim().is_empty()
            || suggestion_id.trim().is_empty() || expected_version == 0
            || request_id.trim().is_empty() || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        { return Err(SuggestionServiceError::InvalidRequest); }
        let context = self.event_context.clone().ok_or(SuggestionServiceError::InvalidRequest)?;
        let as_of = canonicalize_utc_timestamp(as_of).map_err(|_| SuggestionServiceError::InvalidRequest)?;
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let suggestion_id = suggestion_id.to_owned();
        let request_id = request_id.to_owned();
        let blobs = Arc::clone(&self.store.inner.blobs);
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::SuggestionOperation { operation: Box::new(move |connection| {
                let result = execute_owner_action(
                    connection, blobs.as_ref(), &owner, &workspace, &suggestion_id,
                    expected_version, &request_id, &as_of, &context, action,
                );
                let _ = reply.send(result);
            }) },
            receive,
        ).map_err(|_| SuggestionServiceError::Storage)?
    }
}

fn settle_expired_bounded(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    owner: &str,
    workspace: &str,
    service: &str,
    as_of: &str,
    context: &SuggestionEventContext,
    limit: usize,
) -> Result<(usize, bool), SuggestionServiceError> {
    let due = {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| SuggestionServiceError::Storage)?;
        authorize_expiry_owner(&tx, owner, workspace)?;
        let mut statement = tx.prepare(
            "SELECT suggestion_id FROM suggestions WHERE workspace_id = ?1 AND status = 'PROPOSED' AND expires_at <= ?2 ORDER BY expires_at, suggestion_id LIMIT ?3"
        ).map_err(|_| SuggestionServiceError::Storage)?;
        let due = statement.query_map(params![workspace, as_of, (limit + 1) as i64], |row| row.get::<_, String>(0))
            .map_err(|_| SuggestionServiceError::Storage)?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(|_| SuggestionServiceError::Storage)?;
        due.into_iter().take(limit).collect::<Vec<_>>()
    };

    let mut settled = 0usize;
    for suggestion_id in due {
        if expire_one(connection, blobs, owner, workspace, service, as_of, context, &suggestion_id)? {
            settled += 1;
        }
    }

    let tx = connection.transaction().map_err(|_| SuggestionServiceError::Storage)?;
    authorize_expiry_owner(&tx, owner, workspace)?;
    let more_due: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM suggestions WHERE workspace_id = ?1 AND status = 'PROPOSED' AND expires_at <= ?2)",
        params![workspace, as_of], |row| row.get(0),
    ).map_err(|_| SuggestionServiceError::Storage)?;
    Ok((settled, more_due))
}

fn expire_one(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    owner: &str,
    workspace: &str,
    service: &str,
    as_of: &str,
    context: &SuggestionEventContext,
    suggestion_id: &str,
) -> Result<bool, SuggestionServiceError> {
    let prepared = {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| SuggestionServiceError::Storage)?;
        authorize_expiry_owner(&tx, owner, workspace)?;
        let Some(current) = load_suggestion_optional(&tx, workspace, suggestion_id).map_err(map_read_error)? else { return Ok(false); };
        if current.status != SuggestionStatus::Proposed || current.expires_at > as_of { return Ok(false); }
        expire_value(current, service, as_of)?
    };
    let state_bytes = canonical_json(&prepared).map_err(|_| SuggestionServiceError::Storage)?;
    let blob = blobs.put(workspace, BlobPurpose::AggregateState, &state_bytes, "application/vnd.litecow.suggestion+json")
        .map_err(|_| SuggestionServiceError::Storage)?;
    if blob.digest != digest(&state_bytes) || blob.size_bytes != state_bytes.len() as u64
        || blobs.get(workspace, BlobPurpose::AggregateState, &blob).map_err(|_| SuggestionServiceError::Storage)? != state_bytes
    { return Err(SuggestionServiceError::Storage); }

    let expected_version = prepared.version.checked_sub(1).ok_or(SuggestionServiceError::Storage)?;
    let request_id = expiry_request_id(workspace, suggestion_id, expected_version);
    let fingerprint = digest(&canonical_json(&json!({
        "workspace_id": workspace,
        "suggestion_id": suggestion_id,
        "expected_version": expected_version,
        "resolution_reason": "SYSTEM_EXPIRY"
    })).map_err(|_| SuggestionServiceError::Storage)?);
    let event_id = format!("ev_{}", request_id.strip_prefix("sug_exp_").ok_or(SuggestionServiceError::Storage)?);
    let resolved_by = PrincipalRef { principal_id: service.to_owned(), kind: PrincipalKind::Service };
    let payload = json!({
        "suggestion_id": suggestion_id,
        "from": "PROPOSED",
        "to": "EXPIRED",
        "resolved_by": resolved_by.clone(),
        "resolution_reason": "SYSTEM_EXPIRY",
        "aggregate_version": prepared.version
    });

    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| SuggestionServiceError::Storage)?;
    authorize_expiry_owner(&tx, owner, workspace)?;
    if let Some(prior) = expiry_receipt(&tx, service, &request_id, &fingerprint)? {
        // A concurrent sweep already committed this exact version. The durable
        // receipt proves the transition; no second event is appended.
        if prior.suggestion_id != suggestion_id || prior.workspace_id != workspace
            || prior.status != SuggestionStatus::Expired || prior.version != prepared.version
            || prior.resolution_reason.as_deref() != Some("SYSTEM_EXPIRY")
            || prior.resolved_by.as_ref() != Some(&resolved_by)
        { return Err(SuggestionServiceError::Storage); }
        return Ok(false);
    }
    let Some(current) = load_suggestion_optional(&tx, workspace, suggestion_id).map_err(map_read_error)? else { return Ok(false); };
    if current.status != SuggestionStatus::Proposed || current.version != expected_version || current.expires_at > as_of {
        return Ok(false);
    }
    let changed = tx.execute(
        "UPDATE suggestions SET status = 'EXPIRED', snoozed_until = NULL, resolved_at = ?1, resolved_by_json = ?2, resolution_reason = 'SYSTEM_EXPIRY', version = ?3 WHERE workspace_id = ?4 AND suggestion_id = ?5 AND status = 'PROPOSED' AND version = ?6 AND expires_at <= ?1",
        params![as_of, encode(&resolved_by)?, prepared.version, workspace, suggestion_id, expected_version],
    ).map_err(|_| SuggestionServiceError::Storage)?;
    if changed != 1 { return Ok(false); }
    let persisted = load_suggestion(&tx, workspace, suggestion_id).map_err(map_read_error)?;
    if persisted != prepared { return Err(SuggestionServiceError::Storage); }
    write_expiry_event(&tx, workspace, context, as_of, &event_id, suggestion_id, prepared.version, payload, blob)?;
    let response = encode(&prepared)?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![service, request_id, fingerprint, response, digest(response.as_bytes()), as_of],
    ).map_err(|_| SuggestionServiceError::Storage)?;
    tx.commit().map_err(|_| SuggestionServiceError::Storage)?;
    Ok(true)
}

fn execute_owner_action(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    owner: &str,
    workspace: &str,
    suggestion_id: &str,
    expected_version: u64,
    request_id: &str,
    as_of: &str,
    context: &SuggestionEventContext,
    action: SuggestionOwnerAction,
) -> Result<Suggestion, SuggestionServiceError> {
    let action_json = canonical_json(&action).map_err(|_| SuggestionServiceError::InvalidRequest)?;
    let fingerprint = digest(&canonical_json(&json!({
        "workspace_id": workspace,
        "suggestion_id": suggestion_id,
        "expected_version": expected_version,
        "action": serde_json::from_slice::<Value>(&action_json).map_err(|_| SuggestionServiceError::InvalidRequest)?
    })).map_err(|_| SuggestionServiceError::InvalidRequest)?);
    let event_id = format!("ev_{}", &digest(format!("LiteCowork/SuggestionOwnerAction/v1\0{owner}\0{request_id}").as_bytes())[7..]);

    let prepared = {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| SuggestionServiceError::Storage)?;
        authorize_active_suggestion_owner(&tx, owner, workspace)?;
        if let Some(prior) = owner_action_receipt(&tx, owner, request_id, &fingerprint)? {
            if prior.workspace_id != workspace || prior.suggestion_id != suggestion_id
                || prior.version != expected_version.checked_add(1).ok_or(SuggestionServiceError::Storage)?
            { return Err(SuggestionServiceError::Storage); }
            return Ok(prior);
        }
        let current = load_suggestion_optional(&tx, workspace, suggestion_id).map_err(map_read_error)?
            .ok_or(SuggestionServiceError::NotFound)?;
        if current.status == SuggestionStatus::Expired { return Err(SuggestionServiceError::Expired); }
        if current.status != SuggestionStatus::Proposed { return Err(SuggestionServiceError::InvalidTransition); }
        if current.version != expected_version { return Err(SuggestionServiceError::VersionConflict); }
        if current.expires_at <= as_of { return Err(SuggestionServiceError::Expired); }
        prepare_owner_action(current, owner, as_of, action.clone())?
    };

    let state_bytes = canonical_json(&prepared).map_err(|_| SuggestionServiceError::Storage)?;
    let blob = blobs.put(workspace, BlobPurpose::AggregateState, &state_bytes, "application/vnd.litecowork.suggestion+json")
        .map_err(|_| SuggestionServiceError::Storage)?;
    if blob.digest != digest(&state_bytes) || blob.size_bytes != state_bytes.len() as u64
        || blobs.get(workspace, BlobPurpose::AggregateState, &blob).map_err(|_| SuggestionServiceError::Storage)? != state_bytes
    { return Err(SuggestionServiceError::Storage); }

    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| SuggestionServiceError::Storage)?;
    authorize_active_suggestion_owner(&tx, owner, workspace)?;
    if let Some(prior) = owner_action_receipt(&tx, owner, request_id, &fingerprint)? {
        if prior.workspace_id != workspace || prior.suggestion_id != suggestion_id
            || prior.version != expected_version.checked_add(1).ok_or(SuggestionServiceError::Storage)?
        { return Err(SuggestionServiceError::Storage); }
        return Ok(prior);
    }
    let current = load_suggestion_optional(&tx, workspace, suggestion_id).map_err(map_read_error)?
        .ok_or(SuggestionServiceError::NotFound)?;
    if current.status == SuggestionStatus::Expired || current.expires_at <= as_of {
        return Err(SuggestionServiceError::Expired);
    }
    if current.status != SuggestionStatus::Proposed { return Err(SuggestionServiceError::InvalidTransition); }
    if current.version != expected_version { return Err(SuggestionServiceError::VersionConflict); }
    let verified = prepare_owner_action(current.clone(), owner, as_of, action.clone())?;
    if verified != prepared { return Err(SuggestionServiceError::VersionConflict); }

    match &action {
        SuggestionOwnerAction::Snooze { .. } => {
            let changed = tx.execute(
                "UPDATE suggestions SET snoozed_until = ?1, version = ?2 WHERE workspace_id = ?3 AND suggestion_id = ?4 AND status = 'PROPOSED' AND version = ?5 AND expires_at > ?6",
                params![prepared.snoozed_until, prepared.version, workspace, suggestion_id, expected_version, as_of],
            ).map_err(|_| SuggestionServiceError::Storage)?;
            if changed != 1 { return Err(SuggestionServiceError::VersionConflict); }
        }
        SuggestionOwnerAction::Dismiss => {
            let resolved_by = prepared.resolved_by.as_ref().ok_or(SuggestionServiceError::Storage)?;
            let changed = tx.execute(
                "UPDATE suggestions SET status = 'DISMISSED', resolved_at = ?1, resolved_by_json = ?2, resolution_reason = 'DISMISSED_BY_OWNER', version = ?3 WHERE workspace_id = ?4 AND suggestion_id = ?5 AND status = 'PROPOSED' AND version = ?6 AND expires_at > ?1",
                params![as_of, encode(resolved_by)?, prepared.version, workspace, suggestion_id, expected_version],
            ).map_err(|_| SuggestionServiceError::Storage)?;
            if changed != 1 { return Err(SuggestionServiceError::VersionConflict); }
        }
    }
    let persisted = load_suggestion(&tx, workspace, suggestion_id).map_err(map_read_error)?;
    if persisted != prepared { return Err(SuggestionServiceError::Storage); }
    let (event_type, payload) = match &action {
        SuggestionOwnerAction::Snooze { .. } => (
            "suggestion.visibility.changed.v1",
            json!({
                "suggestion_id": suggestion_id,
                "from_snoozed_until": current.snoozed_until.clone(),
                "to_snoozed_until": prepared.snoozed_until.clone(),
                "changed_by": PrincipalRef { principal_id: owner.to_owned(), kind: PrincipalKind::User },
                "aggregate_version": prepared.version
            }),
        ),
        SuggestionOwnerAction::Dismiss => (
            "suggestion.resolved.v1",
            json!({
                "suggestion_id": suggestion_id,
                "from": "PROPOSED",
                "to": "DISMISSED",
                "resolved_by": PrincipalRef { principal_id: owner.to_owned(), kind: PrincipalKind::User },
                "resolution_reason": "DISMISSED_BY_OWNER",
                "aggregate_version": prepared.version
            }),
        ),
    };
    write_suggestion_event(&tx, workspace, context, as_of, &event_id, suggestion_id, prepared.version, event_type, payload, blob)?;
    let response = encode(&prepared)?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![owner, request_id, fingerprint, response, digest(response.as_bytes()), as_of],
    ).map_err(|_| SuggestionServiceError::IdempotencyConflict)?;
    tx.commit().map_err(|_| SuggestionServiceError::Storage)?;
    Ok(prepared)
}

fn prepare_owner_action(
    mut suggestion: Suggestion,
    owner: &str,
    as_of: &str,
    action: SuggestionOwnerAction,
) -> Result<Suggestion, SuggestionServiceError> {
    suggestion.version = suggestion.version.checked_add(1).ok_or(SuggestionServiceError::Storage)?;
    match action {
        SuggestionOwnerAction::Snooze { snoozed_until } => {
            let snoozed_until = snoozed_until
                .map(|value| canonicalize_utc_timestamp(&value).map_err(|_| SuggestionServiceError::InvalidRequest))
                .transpose()?;
            if snoozed_until.as_deref().is_some_and(|until| until <= as_of || until > suggestion.expires_at) {
                return Err(SuggestionServiceError::InvalidRequest);
            }
            suggestion.snoozed_until = snoozed_until;
        }
        SuggestionOwnerAction::Dismiss => {
            suggestion.status = SuggestionStatus::Dismissed;
            suggestion.resolved_at = Some(as_of.to_owned());
            suggestion.resolved_by = Some(PrincipalRef { principal_id: owner.to_owned(), kind: PrincipalKind::User });
            suggestion.resolution_reason = Some("DISMISSED_BY_OWNER".to_owned());
        }
    }
    Ok(suggestion)
}

fn owner_action_receipt(
    tx: &Transaction<'_>,
    owner: &str,
    request_id: &str,
    fingerprint: &str,
) -> Result<Option<Suggestion>, SuggestionServiceError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![owner, request_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(|_| SuggestionServiceError::Storage)?;
    let Some((actual, response, response_digest)) = prior else { return Ok(None); };
    if actual != fingerprint { return Err(SuggestionServiceError::IdempotencyConflict); }
    let response = response.ok_or(SuggestionServiceError::Storage)?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(SuggestionServiceError::Storage);
    }
    serde_json::from_str(&response).map(Some).map_err(|_| SuggestionServiceError::Storage)
}

fn authorize_active_suggestion_owner(tx: &Transaction<'_>, owner: &str, workspace: &str) -> Result<(), SuggestionServiceError> {
    let status: Option<String> = tx.query_row(
        "SELECT status FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2",
        params![workspace, owner], |row| row.get(0),
    ).optional().map_err(|_| SuggestionServiceError::Storage)?;
    match status.as_deref() {
        Some("ACTIVE") => Ok(()),
        Some("ARCHIVED") => Err(SuggestionServiceError::InvalidTransition),
        _ => Err(SuggestionServiceError::Unauthorized),
    }
}

fn expire_value(mut suggestion: Suggestion, service: &str, as_of: &str) -> Result<Suggestion, SuggestionServiceError> {
    suggestion.status = SuggestionStatus::Expired;
    suggestion.snoozed_until = None;
    suggestion.resolved_at = Some(as_of.to_owned());
    suggestion.resolved_by = Some(PrincipalRef { principal_id: service.to_owned(), kind: PrincipalKind::Service });
    suggestion.resolution_reason = Some("SYSTEM_EXPIRY".to_owned());
    suggestion.result_task_id = None;
    suggestion.version = suggestion.version.checked_add(1).ok_or(SuggestionServiceError::Storage)?;
    Ok(suggestion)
}

fn expiry_request_id(workspace: &str, suggestion: &str, version: u64) -> String {
    let key = digest(format!("LiteCowork/SuggestionExpiry/v1\0{workspace}\0{suggestion}\0{version}").as_bytes());
    format!("sug_exp_{}", &key[7..])
}

fn expiry_receipt(
    tx: &Transaction<'_>,
    service: &str,
    request_id: &str,
    fingerprint: &str,
) -> Result<Option<Suggestion>, SuggestionServiceError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![service, request_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(|_| SuggestionServiceError::Storage)?;
    let Some((actual, response, response_digest)) = prior else { return Ok(None); };
    if actual != fingerprint { return Err(SuggestionServiceError::Storage); }
    let response = response.ok_or(SuggestionServiceError::Storage)?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(SuggestionServiceError::Storage);
    }
    serde_json::from_str(&response).map(Some).map_err(|_| SuggestionServiceError::Storage)
}

fn write_expiry_event(
    tx: &Transaction<'_>,
    workspace: &str,
    context: &SuggestionEventContext,
    as_of: &str,
    event_id: &str,
    suggestion_id: &str,
    version: u64,
    payload: Value,
    blob: BlobRef,
) -> Result<(), SuggestionServiceError> {
    write_suggestion_event(
        tx, workspace, context, as_of, event_id, suggestion_id, version,
        "suggestion.resolved.v1", payload, blob,
    )
}

fn write_suggestion_event(
    tx: &Transaction<'_>,
    workspace: &str,
    context: &SuggestionEventContext,
    as_of: &str,
    event_id: &str,
    suggestion_id: &str,
    version: u64,
    event_type: &str,
    payload: Value,
    blob: BlobRef,
) -> Result<(), SuggestionServiceError> {
    write_domain_event(tx, workspace, context, as_of, event_id, "Suggestion", suggestion_id, version, event_type, payload, blob)
}

fn write_domain_event(
    tx: &Transaction<'_>,
    workspace: &str,
    context: &SuggestionEventContext,
    as_of: &str,
    event_id: &str,
    entity_type: &str,
    entity_id: &str,
    version: u64,
    event_type: &str,
    payload: Value,
    blob: BlobRef,
) -> Result<(), SuggestionServiceError> {
    tx.execute("INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1", params![workspace, context.origin_runtime_id]).map_err(|_| SuggestionServiceError::Storage)?;
    let sequence: i64 = tx.query_row("SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2", params![workspace, context.origin_runtime_id], |row| row.get(0)).map_err(|_| SuggestionServiceError::Storage)?;
    let payload_json = encode(&payload)?;
    let state_ref = AggregateStateRef { blob, entity_revision: version, record_schema_version: 1 };
    tx.execute(
        "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, 1, ?10, ?11, ?12, ?8, ?13)",
        params![event_id, workspace, entity_type, entity_id, context.origin_runtime_id, sequence, version, as_of, context.correlation_id, event_type, payload_json, encode(&state_ref)?, digest(payload_json.as_bytes())],
    ).map_err(|_| SuggestionServiceError::Storage)?;
    Ok(())
}

fn authorize_expiry_owner(tx: &Transaction<'_>, owner: &str, workspace: &str) -> Result<(), SuggestionServiceError> {
    let owned: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2)",
        params![workspace, owner], |row| row.get(0),
    ).map_err(|_| SuggestionServiceError::Storage)?;
    if owned { Ok(()) } else { Err(SuggestionServiceError::Unauthorized) }
}

fn map_read_error(error: SuggestionReadError) -> SuggestionServiceError {
    match error {
        SuggestionReadError::Unauthorized => SuggestionServiceError::Unauthorized,
        SuggestionReadError::InvalidRequest => SuggestionServiceError::InvalidRequest,
        SuggestionReadError::ExpiryPending => SuggestionServiceError::Storage,
        SuggestionReadError::Storage => SuggestionServiceError::Storage,
    }
}

fn encode<T: serde::Serialize>(value: &T) -> Result<String, SuggestionServiceError> {
    let bytes = canonical_json(value).map_err(|_| SuggestionServiceError::Storage)?;
    String::from_utf8(bytes).map_err(|_| SuggestionServiceError::Storage)
}

fn authorize_owner(connection: &Connection, principal: &str, workspace: &str) -> Result<(), SuggestionReadError> {
    let owned: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2)",
        params![workspace, principal], |row| row.get(0),
    ).map_err(|_| SuggestionReadError::Storage)?;
    if owned { Ok(()) } else { Err(SuggestionReadError::Unauthorized) }
}

fn status_name(value: SuggestionStatus) -> &'static str {
    match value { SuggestionStatus::Proposed => "PROPOSED", SuggestionStatus::Accepted => "ACCEPTED", SuggestionStatus::Dismissed => "DISMISSED", SuggestionStatus::Expired => "EXPIRED" }
}
fn visibility_name(value: SuggestionVisibility) -> &'static str {
    match value { SuggestionVisibility::Visible => "VISIBLE", SuggestionVisibility::Snoozed => "SNOOZED", SuggestionVisibility::All => "ALL" }
}
fn decode<T: serde::de::DeserializeOwned>(value: String) -> Result<T, SuggestionReadError> {
    serde_json::from_str(&value).map_err(|_| SuggestionReadError::Storage)
}
fn parse_kind(value: &str) -> Result<SuggestionKind, SuggestionReadError> {
    match value { "TASK_OPPORTUNITY" => Ok(SuggestionKind::TaskOpportunity), "ROUTINE_OPPORTUNITY" => Ok(SuggestionKind::RoutineOpportunity), "AUTOMATION_OPPORTUNITY" => Ok(SuggestionKind::AutomationOpportunity), _ => Err(SuggestionReadError::Storage) }
}
fn kind_name(value: SuggestionKind) -> &'static str {
    match value { SuggestionKind::TaskOpportunity => "TASK_OPPORTUNITY", SuggestionKind::RoutineOpportunity => "ROUTINE_OPPORTUNITY", SuggestionKind::AutomationOpportunity => "AUTOMATION_OPPORTUNITY" }
}
fn parse_action(value: &str) -> Result<SuggestionAction, SuggestionReadError> {
    match value { "TASK" => Ok(SuggestionAction::Task), "OPEN_ROUTINE_EDITOR" => Ok(SuggestionAction::OpenRoutineEditor), "OPEN_AUTOMATION_EDITOR" => Ok(SuggestionAction::OpenAutomationEditor), _ => Err(SuggestionReadError::Storage) }
}
fn parse_status(value: &str) -> Result<SuggestionStatus, SuggestionReadError> {
    match value { "PROPOSED" => Ok(SuggestionStatus::Proposed), "ACCEPTED" => Ok(SuggestionStatus::Accepted), "DISMISSED" => Ok(SuggestionStatus::Dismissed), "EXPIRED" => Ok(SuggestionStatus::Expired), _ => Err(SuggestionReadError::Storage) }
}
fn parse_latency(value: Option<String>) -> Result<Option<SuggestionLatencyClass>, SuggestionReadError> {
    value.map(|value| match value.as_str() { "STANDARD" => Ok(SuggestionLatencyClass::Standard), "INTERACTIVE" => Ok(SuggestionLatencyClass::Interactive), "DEADLINE_SENSITIVE" => Ok(SuggestionLatencyClass::DeadlineSensitive), _ => Err(SuggestionReadError::Storage) }).transpose()
}
fn load_suggestion(connection: &Connection, workspace: &str, id: &str) -> Result<Suggestion, SuggestionReadError> {
    load_suggestion_optional(connection, workspace, id)?.ok_or(SuggestionReadError::Storage)
}

fn load_suggestion_optional(connection: &Connection, workspace: &str, id: &str) -> Result<Option<Suggestion>, SuggestionReadError> {
    let row: Option<(Option<String>, String, String, String, String, String, String, Option<String>, Option<String>, Option<String>, String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, u64)> = connection.query_row(
        "SELECT coworker_id, dedupe_key, kind, reason, source_refs_json, goal_refs_json, proposed_action, proposed_by_json, proposed_task_spec_json, estimated_cost_json, status, created_at, expires_at, snoozed_until, resolved_at, resolved_by_json, resolution_reason, result_task_id, latency_class_hint, version FROM suggestions WHERE workspace_id = ?1 AND suggestion_id = ?2",
        params![workspace, id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?, row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?, row.get(16)?, row.get(17)?, row.get(18)?, row.get(19)?)),
    ).optional().map_err(|_| SuggestionReadError::Storage)?;
    let Some(row) = row else { return Ok(None); };
    let (coworker_id, dedupe_key, kind, reason, source_refs, goal_refs, proposed_action, proposed_by, proposed_task_spec, estimated_cost, status, created_at, expires_at, snoozed_until, resolved_at, resolved_by, resolution_reason, result_task_id, latency, version) = row;
    Ok(Some(Suggestion {
        suggestion_id: id.to_owned(), workspace_id: workspace.to_owned(), coworker_id, dedupe_key,
        kind: parse_kind(&kind)?, reason,
        source_refs: decode::<Vec<SuggestionResourceRef>>(source_refs)?,
        goal_refs: decode::<Vec<SuggestionGoalRef>>(goal_refs)?,
        proposed_action: parse_action(&proposed_action)?,
        proposed_by: decode::<SuggestionServiceRef>(proposed_by)?,
        proposed_task_spec: proposed_task_spec.map(decode).transpose()?,
        estimated_cost: estimated_cost.map(decode).transpose()?,
        latency_class_hint: parse_latency(latency)?, status: parse_status(&status)?, created_at,
        expires_at, snoozed_until, resolved_at,
        resolved_by: resolved_by.map(decode::<PrincipalRef>).transpose()?,
        resolution_reason, result_task_id, version,
    }))
}
