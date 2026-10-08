//! Passive Goal persistence through the bounded SQLite writer.
//!
//! Goals store user intent and immutable references to existing same-Workspace work.
//! This adapter does not create Tasks, schedule Automations, or infer progress.

use super::*;
use domain_responsibility::*;
use serde::{Deserialize, Serialize};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;
type Decision = Box<dyn Fn(&mut dyn GoalTransaction) -> Result<Goal, GoalError> + Send>;
const MAX_GOAL_PROGRESS_EVIDENCE_REFS: usize = 1_000;

#[derive(Clone, Debug)]
pub struct GoalEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoalPage {
    pub items: Vec<(Goal, GoalRevision)>,
    pub next: Option<(String, String)>,
}

#[derive(Clone)]
pub struct SqliteGoalStore {
    store: SqliteWorkspaceStore,
    context: GoalEventContext,
}

impl SqliteGoalStore {
    pub fn new(store: SqliteWorkspaceStore, mut context: GoalEventContext) -> Result<Self, GoalError> {
        if [&context.event_id, &context.origin_runtime_id, &context.hlc_timestamp, &context.correlation_id]
            .iter().any(|value| value.trim().is_empty())
        {
            return Err(GoalError::InvalidDefinition);
        }
        context.recorded_at = canonicalize_utc_timestamp(&context.recorded_at).map_err(map_error)?;
        Ok(Self { store, context })
    }

    fn run<T, F>(&self, operation: F) -> Result<T, GoalError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, GoalError> + Send + 'static,
    {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::GoalOperation { operation: Box::new(move |connection| {
                let _ = reply.send(operation(connection));
            }) },
            receive,
        ).map_err(map_error)?
    }

    pub fn get(&self, principal_id: &str, workspace_id: &str, goal_id: &str) -> Result<Option<(Goal, GoalRevision)>, GoalError> {
        let (principal, workspace, id) = (principal_id.to_owned(), workspace_id.to_owned(), goal_id.to_owned());
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            load_goal(&tx, &workspace, &id)
        })
    }

    pub fn get_revision(&self, principal_id: &str, workspace_id: &str, goal_id: &str, revision: u64) -> Result<Option<GoalRevision>, GoalError> {
        if revision == 0 { return Err(GoalError::InvalidDefinition); }
        let (principal, workspace, id) = (principal_id.to_owned(), workspace_id.to_owned(), goal_id.to_owned());
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            load_goal_revision(&tx, &workspace, &id, revision)
        })
    }

    /// Build a bounded read-only projection from current linked Task heads and committed
    /// Evidence IDs. Verification and dependency-freshness readers are not integrated,
    /// so their counts remain null and the projection reports those limitations.
    pub fn progress(
        &self,
        principal_id: &str,
        workspace_id: &str,
        goal_id: &str,
        revision: u64,
    ) -> Result<Option<GoalProgressProjection>, GoalError> {
        if revision == 0 { return Err(GoalError::InvalidDefinition); }
        let (principal, workspace, id, computed_at) = (
            principal_id.to_owned(), workspace_id.to_owned(), goal_id.to_owned(),
            self.context.recorded_at.clone(),
        );
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            let Some(goal_revision) = load_goal_revision(&tx, &workspace, &id, revision)? else {
                return Ok(None);
            };
            let task_ids = &goal_revision.definition.related_task_ids;
            let mut limitations = Vec::new();
            if !task_ids.is_empty() {
                limitations.push(GoalProgressLimitation::VerificationRunReadModelUnavailable);
                limitations.push(GoalProgressLimitation::TaskDependencyFreshnessUnavailable);
            }
            if !goal_revision.definition.related_artifact_refs.is_empty() {
                limitations.push(GoalProgressLimitation::ArtifactDependencyFreshnessUnavailable);
            }
            let mut contributions = Vec::with_capacity(task_ids.len());
            let mut evidence_query = tx.prepare(
                "SELECT evidence_id FROM evidence WHERE task_id = ?1 ORDER BY created_at DESC, evidence_id DESC LIMIT ?2",
            ).map_err(sql_error)?;
            let mut evidence_remaining = MAX_GOAL_PROGRESS_EVIDENCE_REFS;
            for task_id in task_ids {
                let status: String = tx.query_row(
                    "SELECT status FROM tasks WHERE workspace_id = ?1 AND task_id = ?2",
                    params![workspace, task_id], |row| row.get(0),
                ).optional().map_err(sql_error)?.ok_or(GoalError::Storage)?;
                let evidence_limit = evidence_remaining.min(200);
                let evidence_refs = evidence_query.query_map(params![task_id, evidence_limit + 1], |row| row.get::<_, String>(0))
                    .map_err(sql_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?;
                let mut evidence_refs = evidence_refs;
                if evidence_refs.len() > evidence_limit {
                    evidence_refs.truncate(evidence_limit);
                    if !limitations.contains(&GoalProgressLimitation::EvidenceListTruncated) {
                        limitations.push(GoalProgressLimitation::EvidenceListTruncated);
                    }
                }
                evidence_remaining = evidence_remaining.saturating_sub(evidence_refs.len());
                let outcome_state = match status.as_str() {
                    "INCOMPLETE" | "FAILED" | "CANCELLED" => GoalTaskOutcomeState::Incomplete,
                    "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING"
                    | "NEEDS_USER" | "PAUSE_REQUESTED" | "PAUSED" | "COMPLETED"
                    | "CANCEL_REQUESTED" => GoalTaskOutcomeState::Unverified,
                    _ => return Err(GoalError::Storage),
                };
                contributions.push(GoalTaskContribution {
                    task_id: task_id.clone(), task_status: status, outcome_state, evidence_refs,
                });
            }
            let artifact_rows = {
                let mut statement = tx.prepare(
                    "SELECT l.artifact_id, l.artifact_version, av.verification_refs_json FROM goal_artifact_links l JOIN artifacts a ON a.workspace_id = l.workspace_id AND a.artifact_id = l.artifact_id JOIN artifact_versions av ON av.artifact_id = l.artifact_id AND av.version = l.artifact_version WHERE l.workspace_id = ?1 AND l.goal_id = ?2 AND l.revision = ?3 ORDER BY l.artifact_id, l.artifact_version",
                ).map_err(sql_error)?;
                statement.query_map(params![workspace, id, revision], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?, row.get::<_, String>(2)?))
                }).map_err(sql_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?
            };
            let mut artifact_evidence_refs = Vec::with_capacity(artifact_rows.len());
            for (artifact_id, version, refs_json) in artifact_rows {
                let declared_refs: Vec<String> = decode(refs_json)?;
                let declared_refs: std::collections::BTreeSet<_> = declared_refs.into_iter().collect();
                let evidence_limit = evidence_remaining.min(200);
                if declared_refs.len() > evidence_limit {
                    if !limitations.contains(&GoalProgressLimitation::EvidenceListTruncated) {
                        limitations.push(GoalProgressLimitation::EvidenceListTruncated);
                    }
                }
                let mut committed_refs = Vec::new();
                let inspected_refs: Vec<_> = declared_refs.into_iter().take(evidence_limit).collect();
                evidence_remaining = evidence_remaining.saturating_sub(inspected_refs.len());
                for evidence_id in inspected_refs {
                    let exists: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM evidence e JOIN tasks t ON t.task_id = e.task_id WHERE t.workspace_id = ?1 AND e.evidence_id = ?2)",
                        params![workspace, evidence_id], |row| row.get(0),
                    ).map_err(sql_error)?;
                    if exists { committed_refs.push(evidence_id); }
                    else if !limitations.contains(&GoalProgressLimitation::ArtifactEvidenceReferenceUnresolved) {
                        limitations.push(GoalProgressLimitation::ArtifactEvidenceReferenceUnresolved);
                    }
                }
                artifact_evidence_refs.push(GoalArtifactEvidenceRefs { artifact_id, version, evidence_refs: committed_refs });
            }
            let summary = if task_ids.is_empty() {
                format!("No Tasks are linked. {} pinned Artifact version(s) and their resolvable Evidence references are shown; Goal completion is not inferred from Artifact links or Goal text.", artifact_evidence_refs.len())
            } else {
                format!(
                    "Current status and committed Evidence references are shown for {} linked Task(s) and {} pinned Artifact version(s). Verified completion and source freshness are unavailable until the VerificationRun and dependency-freshness readers are integrated.",
                    task_ids.len(), artifact_evidence_refs.len(),
                )
            };
            Ok(Some(GoalProgressProjection {
                computed_at,
                availability: if limitations.is_empty() { GoalProgressAvailability::Complete } else { GoalProgressAvailability::Partial },
                limitations,
                verified_task_count: task_ids.is_empty().then_some(0),
                linked_task_count: task_ids.len() as u64,
                stale_source_count: (task_ids.is_empty() && goal_revision.definition.related_artifact_refs.is_empty()).then_some(0),
                conflicted_source_count: (task_ids.is_empty() && goal_revision.definition.related_artifact_refs.is_empty()).then_some(0),
                contributions,
                artifact_evidence_refs,
                summary,
            }))
        })
    }

    pub fn list(
        &self,
        principal_id: &str,
        workspace_id: &str,
        coworker_id: Option<&str>,
        status: Option<GoalStatus>,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<GoalPage, GoalError> {
        if !(1..=200).contains(&limit)
            || after.as_ref().is_some_and(|(time, id)| time.is_empty() || id.is_empty())
        {
            return Err(GoalError::InvalidDefinition);
        }
        let (principal, workspace) = (principal_id.to_owned(), workspace_id.to_owned());
        let coworker = coworker_id.map(str::to_owned);
        let after = after.map(|(time, id)| canonicalize_utc_timestamp(&time).map(|time| (time, id))).transpose().map_err(map_error)?;
        self.run(move |connection| {
            let tx = connection.transaction().map_err(sql_error)?;
            authorize(&tx, &principal, &workspace)?;
            let mut statement = tx.prepare(
                "SELECT goal_id FROM goals WHERE workspace_id = ?1 AND (?2 IS NULL OR coworker_id = ?2) AND (?3 IS NULL OR status = ?3) AND (?4 IS NULL OR updated_at < ?4 OR (updated_at = ?4 AND goal_id < ?5)) ORDER BY updated_at DESC, goal_id DESC LIMIT ?6"
            ).map_err(sql_error)?;
            let ids = statement.query_map(
                params![workspace, coworker, status.map(goal_status), after.as_ref().map(|cursor| cursor.0.as_str()), after.as_ref().map(|cursor| cursor.1.as_str()), (limit + 1) as i64],
                |row| row.get::<_, String>(0),
            ).map_err(sql_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?;
            let more = ids.len() > limit;
            let mut items = Vec::new();
            for id in ids.into_iter().take(limit) {
                items.push(load_goal(&tx, &workspace, &id)?.ok_or(GoalError::Storage)?);
            }
            let next = if more { items.last().map(|(goal, _)| (goal.updated_at.clone(), goal.goal_id.clone())) } else { None };
            Ok(GoalPage { items, next })
        })
    }
}

impl GoalStore for SqliteGoalStore {
    fn transaction<F>(&mut self, scope: &GoalOwnerScope, fingerprint: &str, operation: F) -> Result<Goal, GoalError>
    where
        F: Fn(&mut dyn GoalTransaction) -> Result<Goal, GoalError> + Send + 'static,
    {
        let scope = scope.clone();
        let fingerprint = digest(&canonical_json(&json!({"workspace_id": scope.workspace_id, "command_digest": fingerprint})).map_err(map_error)?);
        let context = self.context.clone();
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| execute_transaction(connection, &scope, &fingerprint, &context, blobs.as_ref(), Box::new(operation)))
    }
}

fn authorize(connection: &Connection, principal: &str, workspace: &str) -> Result<(), GoalError> {
    if principal.trim().is_empty() || workspace.trim().is_empty() { return Err(GoalError::Unauthorized); }
    let owned: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2)",
        params![workspace, principal], |row| row.get(0),
    ).map_err(sql_error)?;
    if owned { Ok(()) } else { Err(GoalError::Unauthorized) }
}

fn authorize_active(connection: &Connection, principal: &str, workspace: &str) -> Result<(), GoalError> {
    authorize(connection, principal, workspace)?;
    let status: Option<String> = connection.query_row(
        "SELECT status FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2",
        params![workspace, principal], |row| row.get(0),
    ).optional().map_err(sql_error)?;
    match status.as_deref() {
        Some("ACTIVE") => Ok(()),
        Some("ARCHIVED") => Err(GoalError::WorkspaceArchived),
        _ => Err(GoalError::Unauthorized),
    }
}

fn map_error(error: StoreError) -> GoalError {
    match error {
        StoreError::NotFound => GoalError::NotFound,
        StoreError::Conflict { .. } => GoalError::VersionConflict,
        StoreError::Invalid(_) => GoalError::InvalidDefinition,
        _ => GoalError::Storage,
    }
}
fn sql_error(error: rusqlite::Error) -> GoalError { map_error(map_database_error(error)) }
fn encode<T: Serialize>(value: &T) -> Result<String, GoalError> {
    String::from_utf8(canonical_json(value).map_err(map_error)?).map_err(|_| GoalError::InvalidDefinition)
}
fn decode<T: serde::de::DeserializeOwned>(value: String) -> Result<T, GoalError> {
    serde_json::from_str(&value).map_err(|_| GoalError::Storage)
}
fn goal_status(status: GoalStatus) -> &'static str {
    match status { GoalStatus::Active => "ACTIVE", GoalStatus::Paused => "PAUSED", GoalStatus::Completed => "COMPLETED", GoalStatus::Archived => "ARCHIVED" }
}
fn parse_goal_status(status: &str) -> Result<GoalStatus, GoalError> {
    match status { "ACTIVE" => Ok(GoalStatus::Active), "PAUSED" => Ok(GoalStatus::Paused), "COMPLETED" => Ok(GoalStatus::Completed), "ARCHIVED" => Ok(GoalStatus::Archived), _ => Err(GoalError::Storage) }
}

fn load_goal(connection: &Connection, workspace: &str, id: &str) -> Result<Option<(Goal, GoalRevision)>, GoalError> {
    let raw: Option<(Option<String>, String, u64, String, String, String, u64)> = connection.query_row(
        "SELECT coworker_id, status, current_revision, created_at, updated_at, workspace_id, version FROM goals WHERE workspace_id = ?1 AND goal_id = ?2",
        params![workspace, id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
    ).optional().map_err(sql_error)?;
    let Some((coworker_id, status, current_revision, created_at, updated_at, workspace_id, version)) = raw else { return Ok(None); };
    let head = Goal {
        goal_id: id.to_owned(), workspace_id, coworker_id, current_revision,
        status: parse_goal_status(status.as_deref().ok_or(GoalError::Storage)?)?,
        created_at, updated_at, version,
    };
    let revision = load_goal_revision(connection, workspace, id, current_revision)?.ok_or(GoalError::Storage)?;
    Ok(Some((head, revision)))
}

fn load_goal_revision(connection: &Connection, workspace: &str, id: &str, revision: u64) -> Result<Option<GoalRevision>, GoalError> {
    let raw: Option<(String, String, String, Option<String>, String, String)> = connection.query_row(
        "SELECT objective, success_criteria_json, constraints_json, horizon, authored_by_json, created_at FROM goal_revisions WHERE workspace_id = ?1 AND goal_id = ?2 AND revision = ?3",
        params![workspace, id, revision], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    ).optional().map_err(sql_error)?;
    let Some((objective, criteria, constraints, horizon, authored_by, created_at)) = raw else { return Ok(None); };
    let task_ids = {
        let mut statement = connection.prepare("SELECT task_id FROM goal_task_links WHERE workspace_id = ?1 AND goal_id = ?2 AND revision = ?3 ORDER BY task_id").map_err(sql_error)?;
        statement.query_map(params![workspace, id, revision], |row| row.get::<_, String>(0)).map_err(sql_error)?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?
    };
    let artifact_refs = {
        let mut statement = connection.prepare("SELECT artifact_id, artifact_version FROM goal_artifact_links WHERE workspace_id = ?1 AND goal_id = ?2 AND revision = ?3 ORDER BY artifact_id, artifact_version").map_err(sql_error)?;
        statement.query_map(params![workspace, id, revision], |row| Ok(ArtifactVersionRef {
            workspace_id: workspace.to_owned(), artifact_id: row.get(0)?, version: row.get(1)?,
        })).map_err(sql_error)?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?
    };
    let routine_refs = {
        let mut statement = connection.prepare("SELECT routine_id, routine_revision FROM goal_routine_links WHERE workspace_id = ?1 AND goal_id = ?2 AND revision = ?3 ORDER BY routine_id, routine_revision").map_err(sql_error)?;
        statement.query_map(params![workspace, id, revision], |row| Ok(RoutineRevisionRef { routine_id: row.get(0)?, revision: row.get(1)? })).map_err(sql_error)?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(sql_error)?
    };
    Ok(Some(GoalRevision {
        goal_id: id.to_owned(), revision,
        definition: GoalRevisionInput {
            objective, success_criteria: decode(criteria)?, constraints: decode(constraints)?, horizon,
            related_task_ids: task_ids, related_routine_refs: routine_refs,
            related_artifact_refs: artifact_refs,
        },
        authored_by: decode(authored_by)?, created_at,
    }))
}

struct GoalBoundary<'a> {
    connection: &'a Connection,
    scope: &'a GoalOwnerScope,
    context: &'a GoalEventContext,
    pending: Option<GoalMutation>,
}
impl GoalTransaction for GoalBoundary<'_> {
    fn now(&self) -> String { self.context.recorded_at.clone() }
    fn goal(&mut self, id: &str) -> Result<Option<(Goal, GoalRevision)>, GoalError> {
        load_goal(self.connection, &self.scope.workspace_id, id)
    }
    fn validate_references(&mut self, workspace: &str, coworker: Option<&str>, revision: &GoalRevisionInput) -> Result<(), GoalError> {
        if workspace != self.scope.workspace_id { return Err(GoalError::Unauthorized); }
        if let Some(coworker_id) = coworker {
            let exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM coworkers WHERE workspace_id = ?1 AND coworker_id = ?2)", params![workspace, coworker_id], |row| row.get(0)).map_err(sql_error)?;
            if !exists { return Err(GoalError::ReferenceUnavailable); }
        }
        for task_id in &revision.related_task_ids {
            let exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE workspace_id = ?1 AND task_id = ?2)", params![workspace, task_id], |row| row.get(0)).map_err(sql_error)?;
            if !exists { return Err(GoalError::ReferenceUnavailable); }
        }
        for reference in &revision.related_routine_refs {
            let exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM routine_revisions WHERE workspace_id = ?1 AND routine_id = ?2 AND revision = ?3)", params![workspace, reference.routine_id, reference.revision], |row| row.get(0)).map_err(sql_error)?;
            if !exists { return Err(GoalError::ReferenceUnavailable); }
        }
        for reference in &revision.related_artifact_refs {
            if reference.workspace_id != workspace { return Err(GoalError::ReferenceUnavailable); }
            let exists: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM artifacts a JOIN artifact_versions av ON av.artifact_id = a.artifact_id WHERE a.workspace_id = ?1 AND a.artifact_id = ?2 AND av.version = ?3)",
                params![workspace, reference.artifact_id, reference.version], |row| row.get(0),
            ).map_err(sql_error)?;
            if !exists { return Err(GoalError::ReferenceUnavailable); }
        }
        Ok(())
    }
    fn commit(&mut self, mutation: GoalMutation) -> Result<Goal, GoalError> {
        if self.pending.is_some() { return Err(GoalError::InvalidDefinition); }
        let goal = mutation.goal.clone();
        self.pending = Some(mutation);
        Ok(goal)
    }
}

fn prepare(connection: &Connection, scope: &GoalOwnerScope, context: &GoalEventContext, decision: &Decision) -> Result<GoalMutation, GoalError> {
    let mut boundary = GoalBoundary { connection, scope, context, pending: None };
    let result = decision(&mut boundary)?;
    let mutation = boundary.pending.ok_or(GoalError::InvalidDefinition)?;
    if result != mutation.goal { return Err(GoalError::InvalidDefinition); }
    Ok(mutation)
}

fn replay(connection: &Connection, scope: &GoalOwnerScope, fingerprint: &str) -> Result<Option<Goal>, GoalError> {
    let prior: Option<(String, Option<String>, Option<String>)> = connection.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![scope.principal_id, scope.request_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(sql_error)?;
    let Some((actual, response, response_digest)) = prior else { return Ok(None); };
    if actual != fingerprint { return Err(GoalError::IdempotencyConflict); }
    let response = response.ok_or(GoalError::Storage)?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) { return Err(GoalError::Storage); }
    let goal: Goal = serde_json::from_str(&response).map_err(|_| GoalError::Storage)?;
    if goal.workspace_id != scope.workspace_id { return Err(GoalError::Storage); }
    Ok(Some(goal))
}

fn state_value(connection: &Connection, scope: &GoalOwnerScope, mutation: &GoalMutation) -> Result<Value, GoalError> {
    let revision = match &mutation.append_revision {
        Some(revision) => revision.clone(),
        None => load_goal_revision(connection, &scope.workspace_id, &mutation.goal.goal_id, mutation.goal.current_revision)?.ok_or(GoalError::Storage)?,
    };
    if mutation.goal.workspace_id != scope.workspace_id || mutation.goal.version == 0
        || revision.goal_id != mutation.goal.goal_id || revision.revision != mutation.goal.current_revision
    { return Err(GoalError::InvalidDefinition); }
    Ok(json!({"goal": mutation.goal, "revision": revision}))
}

fn execute_transaction(connection: &mut Connection, scope: &GoalOwnerScope, fingerprint: &str, context: &GoalEventContext, blobs: &dyn BlobStore, decision: Decision) -> Result<Goal, GoalError> {
    let (first, state_bytes) = {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql_error)?;
        authorize_active(&tx, &scope.principal_id, &scope.workspace_id)?;
        if let Some(prior) = replay(&tx, scope, fingerprint)? { return Ok(prior); }
        let prepared = prepare(&tx, scope, context, &decision)?;
        let state_bytes = canonical_json(&state_value(&tx, scope, &prepared)?).map_err(map_error)?;
        (prepared, state_bytes)
    };
    let blob = blobs.put(&scope.workspace_id, BlobPurpose::AggregateState, &state_bytes, "application/vnd.litecowork.goal+json").map_err(map_error)?;
    if blob.digest != digest(&state_bytes) || blob.size_bytes != state_bytes.len() as u64
        || blobs.get(&scope.workspace_id, BlobPurpose::AggregateState, &blob).map_err(map_error)? != state_bytes
    { return Err(GoalError::Storage); }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql_error)?;
    authorize_active(&tx, &scope.principal_id, &scope.workspace_id)?;
    if let Some(prior) = replay(&tx, scope, fingerprint)? { return Ok(prior); }
    let final_mutation = prepare(&tx, scope, context, &decision)?;
    let final_state = canonical_json(&state_value(&tx, scope, &final_mutation)?).map_err(map_error)?;
    if final_mutation != first || final_state != state_bytes || digest(&final_state) != blob.digest { return Err(GoalError::VersionConflict); }
    let result = persist(&tx, scope, context, &final_mutation, blob)?;
    let response = encode(&result)?;
    tx.execute("INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)", params![scope.principal_id, scope.request_id, fingerprint, response, digest(response.as_bytes()), context.recorded_at]).map_err(sql_error)?;
    tx.commit().map_err(sql_error)?;
    Ok(result)
}

fn persist(tx: &Transaction<'_>, scope: &GoalOwnerScope, context: &GoalEventContext, mutation: &GoalMutation, blob: BlobRef) -> Result<Goal, GoalError> {
    let head = &mutation.goal;
    if head.workspace_id != scope.workspace_id || head.updated_at != context.recorded_at { return Err(GoalError::InvalidDefinition); }
    let actual = load_goal(tx, &scope.workspace_id, &head.goal_id)?;
    let expected_event = match (&actual, &mutation.append_revision) {
        (None, Some(revision)) if head.created_at == context.recorded_at && head.current_revision == 1 && head.version == 1 && head.status == GoalStatus::Active && revision.revision == 1 =>
            ("goal.created.v1", json!({"goal_id": head.goal_id, "workspace_id": head.workspace_id, "current_revision": 1, "status": "ACTIVE", "aggregate_version": 1})),
        (Some((current, _)), Some(revision)) if head.status == current.status && head.current_revision == current.current_revision.checked_add(1).ok_or(GoalError::RevisionOverflow)? => {
            let digest_value = format!("sha256:{}", hex::encode(sha2::Sha256::digest(serde_json_canonicalizer::to_vec(revision).map_err(|_| GoalError::InvalidDefinition)?)));
            ("goal.revised.v1", json!({"goal_id": head.goal_id, "revision": revision.revision, "revision_digest": digest_value, "authored_by": revision.authored_by, "aggregate_version": head.version}))
        }
        (Some((current, _)), None) if head.current_revision == current.current_revision && head.status != current.status =>
            ("goal.status.changed.v1", json!({"goal_id": head.goal_id, "from": current.status, "to": head.status, "aggregate_version": head.version})),
        _ => return Err(GoalError::InvalidDefinition),
    };
    if mutation.event.kind != expected_event.0 || mutation.event.payload != expected_event.1 { return Err(GoalError::InvalidDefinition); }
    match (mutation.expected_version, &actual) {
        (None, None) if head.version == 1 && head.current_revision == 1 && head.status == GoalStatus::Active => {
            tx.execute("INSERT INTO goals(goal_id, workspace_id, coworker_id, current_revision, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![head.goal_id, head.workspace_id, head.coworker_id, head.current_revision, goal_status(head.status), head.created_at, head.updated_at, head.version]).map_err(sql_error)?;
        }
        (Some(expected), Some((current, _))) if current.version == expected && head.version == expected.checked_add(1).ok_or(GoalError::VersionOverflow)? => {
            if current.status == GoalStatus::Archived || current.created_at != head.created_at { return Err(GoalError::Archived); }
            let changed = tx.execute("UPDATE goals SET current_revision = ?1, status = ?2, updated_at = ?3, version = ?4 WHERE workspace_id = ?5 AND goal_id = ?6 AND version = ?7", params![head.current_revision, goal_status(head.status), head.updated_at, head.version, scope.workspace_id, head.goal_id, expected]).map_err(sql_error)?;
            if changed != 1 { return Err(GoalError::VersionConflict); }
        }
        _ => return Err(GoalError::VersionConflict),
    }
    if let Some(revision) = &mutation.append_revision {
        if revision.goal_id != head.goal_id || revision.revision != head.current_revision
            || revision.authored_by != (PrincipalRef { principal_id: scope.principal_id.clone(), kind: PrincipalKind::User })
            || revision.created_at != context.recorded_at
        { return Err(GoalError::InvalidDefinition); }
        validate_goal_revision(&revision.definition)?;
        let d = &revision.definition;
        tx.execute("INSERT INTO goal_revisions(goal_id, revision, workspace_id, objective, success_criteria_json, constraints_json, horizon, authored_by_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", params![revision.goal_id, revision.revision, scope.workspace_id, d.objective, encode(&d.success_criteria)?, encode(&d.constraints)?, d.horizon, encode(&revision.authored_by)?, revision.created_at]).map_err(sql_error)?;
        for task_id in &d.related_task_ids {
            tx.execute("INSERT INTO goal_task_links(goal_id, revision, workspace_id, task_id) VALUES (?1, ?2, ?3, ?4)", params![revision.goal_id, revision.revision, scope.workspace_id, task_id]).map_err(sql_error)?;
        }
        for routine in &d.related_routine_refs {
            tx.execute("INSERT INTO goal_routine_links(goal_id, revision, workspace_id, routine_id, routine_revision) VALUES (?1, ?2, ?3, ?4, ?5)", params![revision.goal_id, revision.revision, scope.workspace_id, routine.routine_id, routine.revision]).map_err(sql_error)?;
        }
        for artifact in &d.related_artifact_refs {
            if artifact.workspace_id != scope.workspace_id { return Err(GoalError::ReferenceUnavailable); }
            tx.execute("INSERT INTO goal_artifact_links(goal_id, revision, workspace_id, artifact_id, artifact_version) VALUES (?1, ?2, ?3, ?4, ?5)", params![revision.goal_id, revision.revision, scope.workspace_id, artifact.artifact_id, artifact.version]).map_err(sql_error)?;
        }
    }
    let persisted = load_goal(tx, &scope.workspace_id, &head.goal_id)?.ok_or(GoalError::Storage)?;
    if persisted.0 != *head || mutation.append_revision.as_ref().is_some_and(|revision| *revision != persisted.1) { return Err(GoalError::Storage); }
    write_event(tx, scope, context, head.version, &mutation.event, blob)?;
    Ok(head.clone())
}

fn write_event(tx: &Transaction<'_>, scope: &GoalOwnerScope, context: &GoalEventContext, version: u64, event: &GoalEvent, blob: BlobRef) -> Result<(), GoalError> {
    if !["goal.created.v1", "goal.revised.v1", "goal.status.changed.v1"].contains(&event.kind.as_str()) { return Err(GoalError::InvalidDefinition); }
    tx.execute("INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1", params![scope.workspace_id, context.origin_runtime_id]).map_err(sql_error)?;
    let sequence: i64 = tx.query_row("SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2", params![scope.workspace_id, context.origin_runtime_id], |row| row.get(0)).map_err(sql_error)?;
    let payload = encode(&event.payload)?;
    let state_ref = AggregateStateRef { blob, entity_revision: version, record_schema_version: 1 };
    tx.execute("INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, 'Goal', ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?11, ?12, ?13, ?14)", params![context.event_id, scope.workspace_id, event.payload.get("goal_id").and_then(Value::as_str).ok_or(GoalError::InvalidDefinition)?, context.origin_runtime_id, sequence, version, context.hlc_timestamp, context.correlation_id, context.causation_id, event.kind, payload, encode(&state_ref)?, context.recorded_at, digest(payload.as_bytes())]).map_err(sql_error)?;
    Ok(())
}
