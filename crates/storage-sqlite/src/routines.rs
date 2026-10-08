//! Saved Routine persistence through the bounded SQLite writer.
//!
//! This module persists definitions and immutable revisions only. It does not create
//! Tasks, process triggers, or execute a Routine.

use super::*;
use domain_responsibility::*;
use serde::{Deserialize, Serialize};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;
type Decision = Box<dyn Fn(&mut dyn RoutineTransaction) -> Result<Routine, RoutineError> + Send>;

#[derive(Clone, Debug)]
pub struct RoutineEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutinePage {
    pub items: Vec<(Routine, RoutineRevision)>,
    pub next: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutineRevisionPage {
    pub items: Vec<RoutineRevision>,
    /// The last included revision when another page is available.
    pub next: Option<u64>,
}

#[derive(Clone)]
pub struct SqliteRoutineStore {
    store: SqliteWorkspaceStore,
    context: RoutineEventContext,
}

impl SqliteRoutineStore {
    pub fn new(store: SqliteWorkspaceStore, mut context: RoutineEventContext) -> Result<Self, RoutineError> {
        if [&context.event_id, &context.origin_runtime_id, &context.hlc_timestamp, &context.correlation_id]
            .iter().any(|value| value.trim().is_empty())
        {
            return Err(RoutineError::InvalidDefinition);
        }
        context.recorded_at = canonicalize_utc_timestamp(&context.recorded_at).map_err(map_error)?;
        Ok(Self { store, context })
    }

    fn run<T, F>(&self, operation: F) -> Result<T, RoutineError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, RoutineError> + Send + 'static,
    {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::RoutineOperation { operation: Box::new(move |connection| {
                let _ = reply.send(operation(connection));
            }) },
            receive,
        ).map_err(map_error)?
    }

    pub fn get(&self, principal_id: &str, workspace_id: &str, routine_id: &str) -> Result<Option<(Routine, RoutineRevision)>, RoutineError> {
        let (principal, workspace, id) = (principal_id.to_owned(), workspace_id.to_owned(), routine_id.to_owned());
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            load_routine(&tx, &workspace, &id)
        })
    }

    pub fn get_revision(&self, principal_id: &str, workspace_id: &str, routine_id: &str, revision: u64) -> Result<Option<RoutineRevision>, RoutineError> {
        if revision == 0 { return Err(RoutineError::InvalidDefinition); }
        let (principal, workspace, id) = (principal_id.to_owned(), workspace_id.to_owned(), routine_id.to_owned());
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            load_revision(&tx, &workspace, &id, revision)
        })
    }

    pub fn list_revisions(
        &self,
        principal_id: &str,
        workspace_id: &str,
        routine_id: &str,
        after_revision: Option<u64>,
        limit: usize,
    ) -> Result<RoutineRevisionPage, RoutineError> {
        if !(1..=200).contains(&limit) || after_revision == Some(0) {
            return Err(RoutineError::InvalidDefinition);
        }
        let (principal, workspace, id) = (principal_id.to_owned(), workspace_id.to_owned(), routine_id.to_owned());
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            let mut statement = tx.prepare(
                "SELECT revision FROM routine_revisions WHERE workspace_id = ?1 AND routine_id = ?2 AND (?3 IS NULL OR revision > ?3) ORDER BY revision ASC LIMIT ?4"
            ).map_err(sql_error)?;
            let revisions = statement.query_map(
                params![workspace, id, after_revision, (limit + 1) as i64],
                |row| row.get::<_, u64>(0),
            ).map_err(sql_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?;
            let more = revisions.len() > limit;
            let mut items = Vec::new();
            for revision in revisions.into_iter().take(limit) {
                items.push(load_revision(&tx, &workspace, &id, revision)?.ok_or(RoutineError::Storage)?);
            }
            let next = if more { items.last().map(|revision| revision.revision) } else { None };
            Ok(RoutineRevisionPage { items, next })
        })
    }

    pub fn list(
        &self,
        principal_id: &str,
        workspace_id: &str,
        status: Option<RoutineStatus>,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<RoutinePage, RoutineError> {
        if !(1..=200).contains(&limit) || after.as_ref().is_some_and(|(time, id)| time.is_empty() || id.is_empty()) {
            return Err(RoutineError::InvalidDefinition);
        }
        let (principal, workspace) = (principal_id.to_owned(), workspace_id.to_owned());
        let after = after.map(|(time, id)| canonicalize_utc_timestamp(&time).map(|time| (time, id))).transpose().map_err(map_error)?;
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            let mut statement = tx.prepare(
                "SELECT routine_id FROM routines WHERE workspace_id = ?1 AND (?2 IS NULL OR status = ?2) AND (?3 IS NULL OR updated_at < ?3 OR (updated_at = ?3 AND routine_id < ?4)) ORDER BY updated_at DESC, routine_id DESC LIMIT ?5"
            ).map_err(sql_error)?;
            let ids = statement.query_map(
                params![workspace, status.map(status_str), after.as_ref().map(|cursor| cursor.0.as_str()), after.as_ref().map(|cursor| cursor.1.as_str()), (limit + 1) as i64],
                |row| row.get::<_, String>(0),
            ).map_err(sql_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?;
            let more = ids.len() > limit;
            let mut items = Vec::new();
            for id in ids.into_iter().take(limit) {
                items.push(load_routine(&tx, &workspace, &id)?.ok_or(RoutineError::Storage)?);
            }
            let next = if more { items.last().map(|(routine, _)| (routine.updated_at.clone(), routine.routine_id.clone())) } else { None };
            Ok(RoutinePage { items, next })
        })
    }
}

impl RoutineStore for SqliteRoutineStore {
    fn transaction<F>(&mut self, scope: &RoutineOwnerScope, fingerprint: &str, operation: F) -> Result<Routine, RoutineError>
    where
        F: Fn(&mut dyn RoutineTransaction) -> Result<Routine, RoutineError> + Send + 'static,
    {
        let scope = scope.clone();
        let fingerprint = digest(&canonical_json(&json!({"workspace_id": scope.workspace_id, "command_digest": fingerprint})).map_err(map_error)?);
        let context = self.context.clone();
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| execute_transaction(connection, &scope, &fingerprint, &context, blobs.as_ref(), Box::new(operation)))
    }
}

fn authorize(connection: &Connection, principal: &str, workspace: &str) -> Result<(), RoutineError> {
    if principal.trim().is_empty() || workspace.trim().is_empty() { return Err(RoutineError::Unauthorized); }
    let owned: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2)",
        params![workspace, principal], |row| row.get(0),
    ).map_err(sql_error)?;
    if owned { Ok(()) } else { Err(RoutineError::Unauthorized) }
}

fn authorize_active(connection: &Connection, principal: &str, workspace: &str) -> Result<(), RoutineError> {
    authorize(connection, principal, workspace)?;
    let status: Option<String> = connection.query_row(
        "SELECT status FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2",
        params![workspace, principal], |row| row.get(0),
    ).optional().map_err(sql_error)?;
    match status.as_deref() {
        Some("ACTIVE") => Ok(()),
        Some("ARCHIVED") => Err(RoutineError::WorkspaceArchived),
        _ => Err(RoutineError::Unauthorized),
    }
}

fn map_error(error: StoreError) -> RoutineError {
    match error {
        StoreError::NotFound => RoutineError::NotFound,
        StoreError::Conflict { .. } => RoutineError::VersionConflict,
        StoreError::Invalid(_) => RoutineError::InvalidDefinition,
        _ => RoutineError::Storage,
    }
}
fn sql_error(error: rusqlite::Error) -> RoutineError { map_error(map_database_error(error)) }
fn encode<T: Serialize>(value: &T) -> Result<String, RoutineError> {
    String::from_utf8(canonical_json(value).map_err(map_error)?).map_err(|_| RoutineError::InvalidDefinition)
}
fn decode<T: serde::de::DeserializeOwned>(value: String) -> Result<T, RoutineError> {
    serde_json::from_str(&value).map_err(|_| RoutineError::Storage)
}
fn status_str(status: RoutineStatus) -> &'static str {
    match status { RoutineStatus::Active => "ACTIVE", RoutineStatus::Archived => "ARCHIVED" }
}
fn parse_status(status: &str) -> Result<RoutineStatus, RoutineError> {
    match status { "ACTIVE" => Ok(RoutineStatus::Active), "ARCHIVED" => Ok(RoutineStatus::Archived), _ => Err(RoutineError::Storage) }
}

fn load_routine(connection: &Connection, workspace: &str, id: &str) -> Result<Option<(Routine, RoutineRevision)>, RoutineError> {
    let raw: Option<(String, u64, String, String, String, u64)> = connection.query_row(
        "SELECT name, current_revision, status, created_at, updated_at, version FROM routines WHERE workspace_id = ?1 AND routine_id = ?2",
        params![workspace, id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    ).optional().map_err(sql_error)?;
    let Some((name, current_revision, status, created_at, updated_at, version)) = raw else { return Ok(None); };
    let routine = Routine {
        routine_id: id.to_owned(), workspace_id: workspace.to_owned(), name, current_revision,
        status: parse_status(&status)?, created_at, updated_at, version,
    };
    let revision = load_revision(connection, workspace, id, current_revision)?.ok_or(RoutineError::Storage)?;
    Ok(Some((routine, revision)))
}

fn load_revision(connection: &Connection, workspace: &str, id: &str, revision: u64) -> Result<Option<RoutineRevision>, RoutineError> {
    let raw: Option<(String, String, String, String, String, String, String, String, String, Option<String>, String, Option<String>, String, String, String)> = connection.query_row(
        "SELECT objective_template, instructions, input_schema_json, constraints_json, non_goals_json, required_outputs_json, acceptance_criteria_json, approvals_required_json, input_bindings_json, preferred_agent_binding_id, placement_preference_json, budget_ceiling_json, required_capabilities_json, verification_policy_json, authored_by_json FROM routine_revisions WHERE workspace_id = ?1 AND routine_id = ?2 AND revision = ?3",
        params![workspace, id, revision], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?, row.get(12)?, row.get(13)?, row.get(14)?)),
    ).optional().map_err(sql_error)?;
    let Some((objective_template, instructions, input_schema, constraints, non_goals, outputs, criteria, approvals, bindings, preferred_agent_binding_id, placement, budget, capabilities, verification, authored_by)) = raw else { return Ok(None); };
    let created_at: String = connection.query_row(
        "SELECT created_at FROM routine_revisions WHERE workspace_id = ?1 AND routine_id = ?2 AND revision = ?3",
        params![workspace, id, revision], |row| row.get(0),
    ).map_err(sql_error)?;
    Ok(Some(RoutineRevision {
        routine_id: id.to_owned(), revision,
        definition: RoutineRevisionInput {
            objective_template, instructions, input_schema: decode(input_schema)?, constraints: decode(constraints)?,
            non_goals: decode(non_goals)?, required_outputs: decode(outputs)?, acceptance_criteria: decode(criteria)?,
            approvals_required: decode(approvals)?, input_bindings: decode(bindings)?, required_capabilities: decode(capabilities)?,
            preferred_agent_binding_id, placement_preference: decode(placement)?, budget_ceiling: budget.map(decode).transpose()?,
            verification_policy: decode(verification)?,
        },
        authored_by: decode(authored_by)?, created_at,
    }))
}

struct RoutineBoundary<'a> {
    connection: &'a Connection,
    scope: &'a RoutineOwnerScope,
    context: &'a RoutineEventContext,
    pending: Option<RoutineMutation>,
}
impl RoutineTransaction for RoutineBoundary<'_> {
    fn now(&self) -> String { self.context.recorded_at.clone() }
    fn routine(&mut self, id: &str) -> Result<Option<(Routine, RoutineRevision)>, RoutineError> {
        load_routine(self.connection, &self.scope.workspace_id, id)
    }
    fn validate_references(&mut self, workspace: &str, revision: &RoutineRevisionInput) -> Result<(), RoutineError> {
        if workspace != self.scope.workspace_id { return Err(RoutineError::Unauthorized); }
        if let Some(binding_id) = &revision.preferred_agent_binding_id {
            let exists: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2)",
                params![workspace, binding_id], |row| row.get(0),
            ).map_err(sql_error)?;
            if !exists { return Err(RoutineError::InvalidDefinition); }
        }
        Ok(())
    }
    fn has_enabled_automation_references(&mut self, routine_id: &str) -> Result<bool, RoutineError> {
        self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM automation_revisions r JOIN automations a ON a.workspace_id = r.workspace_id AND a.automation_id = r.automation_id WHERE r.workspace_id = ?1 AND r.routine_id = ?2 AND a.status = 'ENABLED')",
            params![self.scope.workspace_id, routine_id], |row| row.get(0),
        ).map_err(sql_error)
    }
    fn commit(&mut self, mutation: RoutineMutation) -> Result<Routine, RoutineError> {
        if self.pending.is_some() { return Err(RoutineError::InvalidDefinition); }
        let routine = mutation.routine.clone();
        self.pending = Some(mutation);
        Ok(routine)
    }
}

fn prepare(connection: &Connection, scope: &RoutineOwnerScope, context: &RoutineEventContext, decision: &Decision) -> Result<RoutineMutation, RoutineError> {
    let mut boundary = RoutineBoundary { connection, scope, context, pending: None };
    let result = decision(&mut boundary)?;
    let mutation = boundary.pending.ok_or(RoutineError::InvalidDefinition)?;
    if result != mutation.routine { return Err(RoutineError::InvalidDefinition); }
    Ok(mutation)
}

fn replay(connection: &Connection, scope: &RoutineOwnerScope, fingerprint: &str) -> Result<Option<Routine>, RoutineError> {
    let prior: Option<(String, Option<String>, Option<String>)> = connection.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![scope.principal_id, scope.request_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(sql_error)?;
    let Some((actual, response, response_digest)) = prior else { return Ok(None); };
    if actual != fingerprint { return Err(RoutineError::IdempotencyConflict); }
    let response = response.ok_or(RoutineError::Storage)?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) { return Err(RoutineError::Storage); }
    let routine: Routine = serde_json::from_str(&response).map_err(|_| RoutineError::Storage)?;
    if routine.workspace_id != scope.workspace_id { return Err(RoutineError::Storage); }
    Ok(Some(routine))
}

fn state_value(connection: &Connection, scope: &RoutineOwnerScope, mutation: &RoutineMutation) -> Result<Value, RoutineError> {
    let revision = match &mutation.append_revision {
        Some(revision) => revision.clone(),
        None => load_revision(connection, &scope.workspace_id, &mutation.routine.routine_id, mutation.routine.current_revision)?.ok_or(RoutineError::Storage)?,
    };
    if mutation.routine.workspace_id != scope.workspace_id || mutation.routine.version == 0
        || revision.routine_id != mutation.routine.routine_id || revision.revision != mutation.routine.current_revision
    { return Err(RoutineError::InvalidDefinition); }
    Ok(json!({"routine": mutation.routine, "revision": revision}))
}

fn execute_transaction(connection: &mut Connection, scope: &RoutineOwnerScope, fingerprint: &str, context: &RoutineEventContext, blobs: &dyn BlobStore, decision: Decision) -> Result<Routine, RoutineError> {
    let (first, state_bytes) = {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql_error)?;
        authorize_active(&tx, &scope.principal_id, &scope.workspace_id)?;
        if let Some(prior) = replay(&tx, scope, fingerprint)? { return Ok(prior); }
        let prepared = prepare(&tx, scope, context, &decision)?;
        let state_bytes = canonical_json(&state_value(&tx, scope, &prepared)?).map_err(map_error)?;
        (prepared, state_bytes)
    };
    let blob = blobs.put(&scope.workspace_id, BlobPurpose::AggregateState, &state_bytes, "application/vnd.litecowork.routine+json").map_err(map_error)?;
    if blob.digest != digest(&state_bytes) || blob.size_bytes != state_bytes.len() as u64
        || blobs.get(&scope.workspace_id, BlobPurpose::AggregateState, &blob).map_err(map_error)? != state_bytes
    { return Err(RoutineError::Storage); }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql_error)?;
    authorize_active(&tx, &scope.principal_id, &scope.workspace_id)?;
    if let Some(prior) = replay(&tx, scope, fingerprint)? { return Ok(prior); }
    let mutation = prepare(&tx, scope, context, &decision)?;
    let final_state = canonical_json(&state_value(&tx, scope, &mutation)?).map_err(map_error)?;
    if mutation != first || final_state != state_bytes || digest(&final_state) != blob.digest { return Err(RoutineError::VersionConflict); }
    let result = persist(&tx, scope, context, &mutation, blob)?;
    let response = encode(&result)?;
    tx.execute("INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)", params![scope.principal_id, scope.request_id, fingerprint, response, digest(response.as_bytes()), context.recorded_at]).map_err(sql_error)?;
    tx.commit().map_err(sql_error)?;
    Ok(result)
}

fn persist(tx: &Transaction<'_>, scope: &RoutineOwnerScope, context: &RoutineEventContext, mutation: &RoutineMutation, blob: BlobRef) -> Result<Routine, RoutineError> {
    let head = &mutation.routine;
    if head.workspace_id != scope.workspace_id || head.updated_at != context.recorded_at { return Err(RoutineError::InvalidDefinition); }
    let actual = load_routine(tx, &scope.workspace_id, &head.routine_id)?;
    let expected_event = match (&actual, &mutation.append_revision) {
        (None, Some(revision)) if head.created_at == context.recorded_at && head.current_revision == 1 && head.version == 1 && head.status == RoutineStatus::Active && revision.revision == 1 =>
            ("routine.created.v1", json!({"routine_id": head.routine_id, "workspace_id": head.workspace_id, "current_revision": 1, "status": "ACTIVE", "aggregate_version": 1})),
        (Some((current, _)), Some(revision)) if current.status == RoutineStatus::Active && head.status == current.status && head.current_revision == current.current_revision.checked_add(1).ok_or(RoutineError::RevisionOverflow)? && head.version == current.version.checked_add(1).ok_or(RoutineError::VersionOverflow)? => {
            let digest_value = revision_digest(revision)?;
            ("routine.revision.created.v1", json!({"routine_id": head.routine_id, "revision": revision.revision, "definition_digest": digest_value, "authored_by": revision.authored_by, "aggregate_version": head.version}))
        }
        (Some((current, _)), None) if current.status == RoutineStatus::Active && head.status == RoutineStatus::Archived && head.current_revision == current.current_revision && head.version == current.version.checked_add(1).ok_or(RoutineError::VersionOverflow)? =>
            ("routine.status.changed.v1", json!({"routine_id": head.routine_id, "from": "ACTIVE", "to": "ARCHIVED", "aggregate_version": head.version})),
        _ => return Err(RoutineError::InvalidDefinition),
    };
    if mutation.event.kind != expected_event.0 || mutation.event.payload != expected_event.1 { return Err(RoutineError::InvalidDefinition); }
    match (mutation.expected_version, &actual) {
        (None, None) if head.version == 1 && head.current_revision == 1 && head.status == RoutineStatus::Active => {
            tx.execute("INSERT INTO routines(routine_id, workspace_id, name, current_revision, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![head.routine_id, head.workspace_id, head.name, head.current_revision, status_str(head.status), head.created_at, head.updated_at, head.version]).map_err(sql_error)?;
        }
        (Some(expected), Some((current, _))) if current.version == expected && head.version == expected.checked_add(1).ok_or(RoutineError::VersionOverflow)? => {
            if current.status != RoutineStatus::Active || current.created_at != head.created_at || current.name != head.name { return Err(RoutineError::Archived); }
            let changed = tx.execute("UPDATE routines SET current_revision = ?1, status = ?2, updated_at = ?3, version = ?4 WHERE workspace_id = ?5 AND routine_id = ?6 AND version = ?7", params![head.current_revision, status_str(head.status), head.updated_at, head.version, scope.workspace_id, head.routine_id, expected]).map_err(sql_error)?;
            if changed != 1 { return Err(RoutineError::VersionConflict); }
        }
        _ => return Err(RoutineError::VersionConflict),
    }
    if let Some(revision) = &mutation.append_revision {
        if revision.routine_id != head.routine_id || revision.revision != head.current_revision
            || revision.authored_by != (PrincipalRef { principal_id: scope.principal_id.clone(), kind: PrincipalKind::User })
            || revision.created_at != context.recorded_at
        { return Err(RoutineError::InvalidDefinition); }
        validate_routine_revision(&revision.definition)?;
        let d = &revision.definition;
        tx.execute("INSERT INTO routine_revisions(workspace_id, routine_id, revision, objective_template, instructions, input_schema_json, constraints_json, non_goals_json, required_outputs_json, acceptance_criteria_json, approvals_required_json, input_bindings_json, required_capabilities_json, preferred_agent_binding_id, placement_preference_json, budget_ceiling_json, verification_policy_json, authored_by_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)", params![scope.workspace_id, revision.routine_id, revision.revision, d.objective_template, d.instructions, encode(&d.input_schema)?, encode(&d.constraints)?, encode(&d.non_goals)?, encode(&d.required_outputs)?, encode(&d.acceptance_criteria)?, encode(&d.approvals_required)?, encode(&d.input_bindings)?, encode(&d.required_capabilities)?, d.preferred_agent_binding_id, encode(&d.placement_preference)?, d.budget_ceiling.as_ref().map(encode).transpose()?, encode(&d.verification_policy)?, encode(&revision.authored_by)?, revision.created_at]).map_err(sql_error)?;
    }
    let persisted = load_routine(tx, &scope.workspace_id, &head.routine_id)?.ok_or(RoutineError::Storage)?;
    if persisted.0 != *head || mutation.append_revision.as_ref().is_some_and(|revision| *revision != persisted.1) { return Err(RoutineError::Storage); }
    write_event(tx, scope, context, head.version, &mutation.event, blob)?;
    Ok(head.clone())
}

fn revision_digest(revision: &RoutineRevision) -> Result<String, RoutineError> {
    let bytes = serde_json_canonicalizer::to_vec(revision).map_err(|_| RoutineError::InvalidDefinition)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn write_event(tx: &Transaction<'_>, scope: &RoutineOwnerScope, context: &RoutineEventContext, version: u64, event: &RoutineEvent, blob: BlobRef) -> Result<(), RoutineError> {
    if !["routine.created.v1", "routine.revision.created.v1", "routine.status.changed.v1"].contains(&event.kind.as_str()) { return Err(RoutineError::InvalidDefinition); }
    tx.execute("INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1", params![scope.workspace_id, context.origin_runtime_id]).map_err(sql_error)?;
    let sequence: i64 = tx.query_row("SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2", params![scope.workspace_id, context.origin_runtime_id], |row| row.get(0)).map_err(sql_error)?;
    let payload = encode(&event.payload)?;
    let state_ref = AggregateStateRef { blob, entity_revision: version, record_schema_version: 1 };
    tx.execute("INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, 'Routine', ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?11, ?12, ?13, ?14)", params![context.event_id, scope.workspace_id, event.payload.get("routine_id").and_then(Value::as_str).ok_or(RoutineError::InvalidDefinition)?, context.origin_runtime_id, sequence, version, context.hlc_timestamp, context.correlation_id, context.causation_id, event.kind, payload, encode(&state_ref)?, context.recorded_at, digest(payload.as_bytes())]).map_err(sql_error)?;
    Ok(())
}
