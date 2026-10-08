//! Workspace-scoped DelegationProfile reads and idempotent write transactions.
//!
//! Profile heads, immutable revisions, aggregate state references, domain events, and
//! request receipts are committed through the bounded SQLite writer. Adapter descriptor,
//! Trust, and Environment admission integration remains outside this persistence slice;
//! profile enablement therefore fails closed in the domain service.
use super::*;
use domain_responsibility::{
    CommittedDelegationProfile, DelegationProfile, DelegationProfileAppend,
    DelegationProfileCommandScope, DelegationProfileError, DelegationProfileEvent,
    DelegationProfileMutation, DelegationProfileRevision, DelegationProfileStatus,
    DelegationProfileStore as DomainDelegationProfileStore, DelegationProfileTransaction,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

#[derive(Clone, Debug, PartialEq)]
pub struct DelegationProfilePage {
    /// Current profile heads with their exact current immutable revision.
    pub items: Vec<Value>,
    /// Final returned (updated_at, delegation_profile_id), ordered descending.
    pub next: Option<(String, String)>,
}

#[derive(Clone)]
pub struct SqliteDelegationProfileStore {
    store: SqliteWorkspaceStore,
    context: Option<DelegationProfileEventContext>,
}

#[derive(Clone, Debug)]
pub struct DelegationProfileEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

impl SqliteDelegationProfileStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store, context: None }
    }

    pub fn new_with_context(store: SqliteWorkspaceStore, context: DelegationProfileEventContext) -> Result<Self, DelegationProfileError> {
        if [&context.event_id, &context.origin_runtime_id, &context.hlc_timestamp, &context.correlation_id, &context.recorded_at].iter().any(|value| value.trim().is_empty()) {
            return Err(DelegationProfileError::InvalidDefinition);
        }
        validate_timestamp(&context.recorded_at).map_err(domain_store_error)?;
        Ok(Self { store, context: Some(context) })
    }

    fn run<T, F>(&self, operation: F) -> Result<T, DelegationProfileError>
    where T: Send + 'static, F: FnOnce(&mut Connection) -> Result<T, DelegationProfileError> + Send + 'static {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(Command::DelegationProfileOperation { operation: Box::new(move |connection| {
            let _ = reply.send(Ok(operation(connection)));
        }) }, receive).map_err(domain_store_error)?
    }

    pub fn list(
        &self,
        principal_id: &str,
        workspace_id: &str,
        agent_binding_id: Option<&str>,
        status: Option<&str>,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<DelegationProfilePage, StoreError> {
        if principal_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || agent_binding_id.is_some_and(|id| id.trim().is_empty() || id.len() > 256)
            || status.is_some_and(|value| !matches!(value, "ENABLED" | "DISABLED" | "ARCHIVED"))
            || !(1..=200).contains(&limit)
            || after.as_ref().is_some_and(|(time, id)| time.is_empty() || id.is_empty())
        {
            return Err(StoreError::Invalid("DelegationProfile query is invalid".to_owned()));
        }

        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let binding = agent_binding_id.map(str::to_owned);
        let status = status.map(str::to_owned);
        if let Some((time, _)) = after.as_ref() {
            validate_timestamp(time)?;
        }
        let after = after.map(|(time, id)| (time.to_owned(), id.to_owned()));
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::DelegationProfileOperation {
                operation: Box::new(move |connection| {
                    let result = (|| {
                        let tx = connection.transaction().map_err(map_database_error)?;
                        let owned: bool = tx
                            .query_row(
                                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2)",
                                params![workspace, principal],
                                |row| row.get(0),
                            )
                            .map_err(map_database_error)?;
                        if !owned {
                            return Err(StoreError::NotFound);
                        }

                        let mut statement = tx
                            .prepare(
                                "SELECT delegation_profile_id, updated_at FROM delegation_profiles \
                                 WHERE workspace_id = ?1 \
                                   AND (?2 IS NULL OR agent_binding_id = ?2) \
                                   AND (?3 IS NULL OR status = ?3) \
                                   AND (?4 IS NULL OR updated_at < ?4 OR (updated_at = ?4 AND delegation_profile_id < ?5)) \
                                 ORDER BY updated_at DESC, delegation_profile_id DESC LIMIT ?6",
                            )
                            .map_err(map_database_error)?;
                        let rows = statement
                            .query_map(
                                params![
                                    workspace,
                                    binding,
                                    status,
                                    after.as_ref().map(|cursor| cursor.0.as_str()),
                                    after.as_ref().map(|cursor| cursor.1.as_str()),
                                    (limit + 1) as i64,
                                ],
                                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                            )
                            .map_err(map_database_error)?
                            .collect::<rusqlite::Result<Vec<_>>>()
                            .map_err(map_database_error)?;
                        drop(statement);

                        let more = rows.len() > limit;
                        let mut items = Vec::with_capacity(rows.len().min(limit));
                        for (profile_id, _) in rows.iter().take(limit) {
                            items.push(load_profile(&tx, &workspace, profile_id)?);
                        }
                        let next = if more {
                            rows.get(limit - 1).cloned()
                        } else {
                            None
                        };
                        Ok(DelegationProfilePage { items, next })
                    })();
                    let _ = reply.send(result);
                }),
            },
            receive,
        )
    }

    pub fn get(&self, principal_id: &str, workspace_id: &str, profile_id: &str) -> Result<Option<Value>, StoreError> {
        if principal_id.trim().is_empty() || workspace_id.trim().is_empty() || profile_id.trim().is_empty() {
            return Err(StoreError::Invalid("DelegationProfile lookup is invalid".to_owned()));
        }
        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let id = profile_id.to_owned();
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(Command::DelegationProfileOperation { operation: Box::new(move |connection| {
            let result = (|| {
                let tx = connection.transaction().map_err(map_database_error)?;
                let owned: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2)",
                    params![workspace, principal], |row| row.get(0),
                ).map_err(map_database_error)?;
                if !owned { return Err(StoreError::NotFound); }
                let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM delegation_profiles WHERE workspace_id=?1 AND delegation_profile_id=?2)", params![workspace,id], |row| row.get(0)).map_err(map_database_error)?;
                if !exists { return Ok(None); }
                load_profile(&tx, &workspace, &id).map(Some)
            })();
            let _ = reply.send(result);
        }) }, receive)
    }
}

impl DomainDelegationProfileStore for SqliteDelegationProfileStore {
    fn transaction<F>(&mut self, scope: &DelegationProfileCommandScope, fingerprint: &str, operation: F) -> Result<CommittedDelegationProfile, DelegationProfileError>
    where F: Fn(&mut dyn DelegationProfileTransaction) -> Result<CommittedDelegationProfile, DelegationProfileError> + Send + 'static {
        let context = self.context.clone().ok_or(DelegationProfileError::Storage)?;
        let scope = scope.clone();
        let request_fingerprint = digest(&canonical_json(&json!({"workspace_id":scope.workspace_id,"command_digest":fingerprint})).map_err(domain_store_error)?);
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| execute_profile_transaction(connection, &scope, &request_fingerprint, &context, blobs.as_ref(), Box::new(operation)))
    }
}

type ProfileDecision = Box<dyn Fn(&mut dyn DelegationProfileTransaction) -> Result<CommittedDelegationProfile, DelegationProfileError> + Send>;

struct ProfileTransaction<'a> {
    connection: &'a Connection,
    scope: &'a DelegationProfileCommandScope,
    context: &'a DelegationProfileEventContext,
    pending: Option<DelegationProfileMutation>,
}
impl DelegationProfileTransaction for ProfileTransaction<'_> {
    fn now(&self) -> String { self.context.recorded_at.clone() }
    fn profile(&mut self, id: &str) -> Result<Option<CommittedDelegationProfile>, DelegationProfileError> {
        load_profile_typed(self.connection, &self.scope.workspace_id, id)
    }
    fn binding_is_enabled_in_workspace(&mut self, id: &str) -> Result<bool, DelegationProfileError> {
        self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2 AND enabled = 1)",
            params![self.scope.workspace_id, id], |row| row.get(0),
        ).map_err(profile_sql_error)
    }
    fn name_is_available(&mut self, binding_id: &str, name_key: &str, excluding_profile_id: Option<&str>) -> Result<bool, DelegationProfileError> {
        self.connection.query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM delegation_profiles WHERE workspace_id=?1 AND agent_binding_id=?2 AND name_key=?3 AND status <> 'ARCHIVED' AND (?4 IS NULL OR delegation_profile_id <> ?4))",
            params![self.scope.workspace_id, binding_id, name_key, excluding_profile_id], |row| row.get(0),
        ).map_err(profile_sql_error)
    }
    fn commit(&mut self, mutation: DelegationProfileMutation) -> Result<CommittedDelegationProfile, DelegationProfileError> {
        if self.pending.is_some() { return Err(DelegationProfileError::InvalidDefinition); }
        let result = mutation.committed.clone();
        self.pending = Some(mutation);
        Ok(result)
    }
}

fn prepare_profile(connection: &Connection, scope: &DelegationProfileCommandScope, context: &DelegationProfileEventContext, decision: &ProfileDecision) -> Result<DelegationProfileMutation, DelegationProfileError> {
    let mut boundary = ProfileTransaction { connection, scope, context, pending: None };
    let result = decision(&mut boundary)?;
    let mutation = boundary.pending.ok_or(DelegationProfileError::InvalidDefinition)?;
    if result != mutation.committed { return Err(DelegationProfileError::InvalidDefinition); }
    Ok(mutation)
}

fn execute_profile_transaction(connection: &mut Connection, scope: &DelegationProfileCommandScope, fingerprint: &str, context: &DelegationProfileEventContext, blobs: &dyn BlobStore, decision: ProfileDecision) -> Result<CommittedDelegationProfile, DelegationProfileError> {
    let (prepared, state_bytes) = {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(profile_sql_error)?;
        authorize_profile_owner(&tx, &scope.principal_id, &scope.workspace_id)?;
        if let Some(replay) = profile_replay(&tx, scope, fingerprint)? { return Ok(replay); }
        let mutation = prepare_profile(&tx, scope, context, &decision)?;
        let state = profile_state_record(&mutation)?;
        (mutation, canonical_json(&state).map_err(domain_store_error)?)
    };
    let blob = blobs.put(&scope.workspace_id, BlobPurpose::AggregateState, &state_bytes, "application/vnd.litecowork.delegation-profile+json").map_err(domain_store_error)?;
    if blob.digest != digest(&state_bytes) || blob.size_bytes != state_bytes.len() as u64 || blobs.get(&scope.workspace_id, BlobPurpose::AggregateState, &blob).map_err(domain_store_error)? != state_bytes {
        return Err(DelegationProfileError::Storage);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(profile_sql_error)?;
    authorize_profile_owner(&tx, &scope.principal_id, &scope.workspace_id)?;
    if let Some(replay) = profile_replay(&tx, scope, fingerprint)? { return Ok(replay); }
    let final_mutation = prepare_profile(&tx, scope, context, &decision)?;
    let final_state = canonical_json(&profile_state_record(&final_mutation)?).map_err(domain_store_error)?;
    if final_mutation != prepared || final_state != state_bytes || digest(&final_state) != blob.digest { return Err(DelegationProfileError::VersionConflict); }
    persist_profile(&tx, scope, context, &final_mutation, blob)?;
    let response = encode(&final_mutation.committed).map_err(profile_domain_error)?;
    tx.execute("INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)", params![scope.principal_id, scope.request_id, fingerprint, response, digest(response.as_bytes()), context.recorded_at]).map_err(profile_sql_error)?;
    tx.commit().map_err(profile_sql_error)?;
    Ok(final_mutation.committed)
}

fn authorize_profile_owner(connection: &Connection, principal: &str, workspace: &str) -> Result<(), DelegationProfileError> {
    if principal.trim().is_empty() || workspace.trim().is_empty() { return Err(DelegationProfileError::Unauthorized); }
    let status: Option<String> = connection.query_row("SELECT status FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2", params![workspace, principal], |row| row.get(0)).optional().map_err(profile_sql_error)?;
    match status.as_deref() { Some("ACTIVE") => Ok(()), Some("ARCHIVED") => Err(DelegationProfileError::WorkspaceArchived), _ => Err(DelegationProfileError::Unauthorized) }
}

fn profile_replay(connection: &Connection, scope: &DelegationProfileCommandScope, fingerprint: &str) -> Result<Option<CommittedDelegationProfile>, DelegationProfileError> {
    let receipt: Option<(String, Option<String>, Option<String>)> = connection.query_row("SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2", params![scope.principal_id, scope.request_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional().map_err(profile_sql_error)?;
    let Some((prior, body, body_digest)) = receipt else { return Ok(None) };
    if prior != fingerprint { return Err(DelegationProfileError::IdempotencyConflict); }
    let body = body.ok_or(DelegationProfileError::Storage)?;
    if body_digest.as_deref() != Some(digest(body.as_bytes()).as_str()) { return Err(DelegationProfileError::Storage); }
    let result: CommittedDelegationProfile = serde_json::from_str(&body).map_err(|_| DelegationProfileError::Storage)?;
    if result.profile.workspace_id != scope.workspace_id || result.revision.delegation_profile_id != result.profile.delegation_profile_id || result.revision.revision != result.profile.current_revision { return Err(DelegationProfileError::Storage); }
    Ok(Some(result))
}

fn profile_state_record(mutation: &DelegationProfileMutation) -> Result<Value, DelegationProfileError> {
    let profile = &mutation.committed.profile;
    let revision = &mutation.committed.revision;
    if profile.workspace_id != revision.workspace_id || profile.delegation_profile_id != revision.delegation_profile_id || profile.current_revision != revision.revision || profile.version == 0 { return Err(DelegationProfileError::InvalidDefinition); }
    Ok(json!({"delegation_profile":profile,"revision":revision}))
}

fn persist_profile(tx: &Transaction<'_>, scope: &DelegationProfileCommandScope, context: &DelegationProfileEventContext, mutation: &DelegationProfileMutation, blob: BlobRef) -> Result<(), DelegationProfileError> {
    let head = &mutation.committed.profile;
    let revision = &mutation.committed.revision;
    if head.workspace_id != scope.workspace_id || head.updated_at != context.recorded_at || revision.authored_by.principal_id == "" { return Err(DelegationProfileError::InvalidDefinition); }
    let existing = load_profile_typed(tx, &scope.workspace_id, &head.delegation_profile_id)?;
    let event_expected = match (&existing, &mutation.append) {
        (None, Some(DelegationProfileAppend::Revision(revision))) if head.version == 1 && head.current_revision == 1 && head.status == DelegationProfileStatus::Disabled && revision.revision == 1 => {
            ("delegation_profile.created.v1", json!({"delegation_profile_id":head.delegation_profile_id,"workspace_id":head.workspace_id,"agent_binding_id":head.agent_binding_id,"current_revision":1,"status":"DISABLED","aggregate_version":1}))
        }
        (Some(current), Some(DelegationProfileAppend::Revision(revision))) if head.current_revision == current.profile.current_revision + 1 && head.status == current.profile.status => {
            ("delegation_profile.revised.v1", json!({"delegation_profile_id":head.delegation_profile_id,"revision":revision.revision,"revision_digest":revision_digest(revision)?,"authored_by":revision.authored_by,"aggregate_version":head.version}))
        }
        (Some(current), None) if head.current_revision == current.profile.current_revision && head.status != current.profile.status => {
            ("delegation_profile.status.changed.v1", json!({"delegation_profile_id":head.delegation_profile_id,"from":current.profile.status,"to":head.status,"aggregate_version":head.version}))
        }
        _ => return Err(DelegationProfileError::InvalidDefinition),
    };
    if mutation.event.kind != event_expected.0 || mutation.event.payload != event_expected.1 { return Err(DelegationProfileError::InvalidDefinition); }
    if let Some(DelegationProfileAppend::Revision(new_revision)) = &mutation.append {
        if new_revision.delegation_profile_id != head.delegation_profile_id || new_revision.workspace_id != scope.workspace_id || new_revision.revision != head.current_revision || new_revision.name != head.name || new_revision.name_key != head.name_key || new_revision.created_at != context.recorded_at { return Err(DelegationProfileError::InvalidDefinition); }
    }
    match (mutation.expected_version, existing.as_ref()) {
        (None, None) if head.version == 1 => {
            let enabled: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2 AND enabled = 1)", params![scope.workspace_id, head.agent_binding_id], |row| row.get(0)).map_err(profile_sql_error)?;
            if !enabled { return Err(DelegationProfileError::InvalidDefinition); }
            tx.execute("INSERT INTO delegation_profiles(delegation_profile_id, workspace_id, agent_binding_id, name, name_key, current_revision, status, created_at, updated_at, version) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![head.delegation_profile_id, head.workspace_id, head.agent_binding_id, head.name, head.name_key, head.current_revision, profile_status(head.status), head.created_at, head.updated_at, head.version]).map_err(profile_sql_error)?;
        }
        (Some(expected), Some(current)) if current.profile.version == expected && head.version == expected + 1 => {
            if current.profile.status == DelegationProfileStatus::Archived || current.profile.agent_binding_id != head.agent_binding_id { return Err(DelegationProfileError::Archived); }
            // SQLite's head-name trigger requires the immutable target revision to
            // exist before the current head advances. Both writes remain in this tx.
            if let Some(DelegationProfileAppend::Revision(revision)) = &mutation.append { insert_profile_revision(tx, revision)?; }
            let changed = tx.execute("UPDATE delegation_profiles SET name=?1, name_key=?2, current_revision=?3, status=?4, updated_at=?5, version=?6 WHERE workspace_id=?7 AND delegation_profile_id=?8 AND version=?9", params![head.name, head.name_key, head.current_revision, profile_status(head.status), head.updated_at, head.version, scope.workspace_id, head.delegation_profile_id, expected]).map_err(profile_sql_error)?;
            if changed != 1 { return Err(DelegationProfileError::VersionConflict); }
        }
        _ => return Err(DelegationProfileError::VersionConflict),
    }
    if let Some(DelegationProfileAppend::Revision(revision)) = &mutation.append {
        if mutation.expected_version.is_none() { insert_profile_revision(tx, revision)?; }
    }
    let persisted = load_profile_typed(tx, &scope.workspace_id, &head.delegation_profile_id)?.ok_or(DelegationProfileError::Storage)?;
    if persisted.profile != *head || persisted.revision != *revision { return Err(DelegationProfileError::Storage); }
    tx.execute("INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1,?2,1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence=last_sequence+1", params![scope.workspace_id, context.origin_runtime_id]).map_err(profile_sql_error)?;
    let sequence: i64 = tx.query_row("SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id=?1 AND origin_runtime_id=?2", params![scope.workspace_id, context.origin_runtime_id], |row| row.get(0)).map_err(profile_sql_error)?;
    let payload = encode(&mutation.event.payload).map_err(profile_domain_error)?;
    let state_ref = AggregateStateRef { blob, entity_revision: head.version, record_schema_version: 1 };
    tx.execute("INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1,?2,'DelegationProfile',?3,?4,?5,?6,?7,?8,?9,1,?10,?11,?12,?13,?14)", params![context.event_id, scope.workspace_id, head.delegation_profile_id, context.origin_runtime_id, sequence, head.version, context.hlc_timestamp, context.correlation_id, context.causation_id, mutation.event.kind, payload, encode(&state_ref).map_err(profile_domain_error)?, context.recorded_at, digest(payload.as_bytes())]).map_err(profile_sql_error)?;
    Ok(())
}

fn insert_profile_revision(tx: &Transaction<'_>, revision: &DelegationProfileRevision) -> Result<(), DelegationProfileError> {
    tx.execute("INSERT INTO delegation_profile_revisions(delegation_profile_id,revision,workspace_id,name,name_key,routing_description,instructions,session_options_json,session_options_descriptor_digest,required_features_json,preferred_features_json,enforced_policy_json,optimization_preference,quality_floor_json,max_concurrency,max_host_delegation_depth,budget_ceiling_json,latency_class,environment_policy_json,native_delegation_policy,warm_policy_json,authored_by_json,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)", params![revision.delegation_profile_id, revision.revision, revision.workspace_id, revision.name, revision.name_key, revision.routing_description, revision.instructions, encode(&revision.session_options).map_err(profile_domain_error)?, revision.session_options_descriptor_digest, encode(&revision.required_features).map_err(profile_domain_error)?, encode(&revision.preferred_features).map_err(profile_domain_error)?, encode(&revision.enforced_policy).map_err(profile_domain_error)?, optimization_preference(revision.optimization_preference), revision.quality_floor.as_ref().map(|value| encode(value)).transpose().map_err(profile_domain_error)?, revision.max_concurrency, revision.max_host_delegation_depth, revision.budget_ceiling.as_ref().map(|value| encode(value)).transpose().map_err(profile_domain_error)?, latency_class(revision.latency_class), encode(&revision.environment_policy).map_err(profile_domain_error)?, native_delegation_policy(revision.native_delegation_policy), encode(&revision.warm_policy).map_err(profile_domain_error)?, encode(&revision.authored_by).map_err(profile_domain_error)?, revision.created_at]).map_err(profile_sql_error)?;
    Ok(())
}

fn revision_digest(revision: &DelegationProfileRevision) -> Result<String, DelegationProfileError> {
    canonical_json(revision).map(|bytes| digest(&bytes)).map_err(domain_store_error)
}

fn load_profile_typed(connection: &Connection, workspace: &str, id: &str) -> Result<Option<CommittedDelegationProfile>, DelegationProfileError> {
    let row: Option<(String,String,String,String,String,u64,String,String,String,u64)> = connection.query_row("SELECT delegation_profile_id,workspace_id,agent_binding_id,name,name_key,current_revision,status,created_at,updated_at,version FROM delegation_profiles WHERE workspace_id=?1 AND delegation_profile_id=?2", params![workspace,id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?))).optional().map_err(profile_sql_error)?;
    let Some(row) = row else { return Ok(None) };
    let profile = DelegationProfile { delegation_profile_id: row.0, workspace_id: row.1, agent_binding_id: row.2, name: row.3, name_key: row.4, current_revision: row.5, status: parse_profile_status(&row.6)?, created_at: row.7, updated_at: row.8, version: row.9 };
    let revision = load_profile_revision(connection, workspace, id, profile.current_revision)?.ok_or(DelegationProfileError::Storage)?;
    Ok(Some(CommittedDelegationProfile { profile, revision }))
}

fn load_profile_revision(connection: &Connection, workspace: &str, id: &str, revision: u64) -> Result<Option<DelegationProfileRevision>, DelegationProfileError> {
    let row: Option<(u64,String,String,String,String,Option<String>,String,Option<String>,String,String,String,String,Option<String>,u32,u32,Option<String>,String,String,String,String,String,String)> = connection.query_row("SELECT revision,workspace_id,name,name_key,routing_description,instructions,session_options_json,session_options_descriptor_digest,required_features_json,preferred_features_json,enforced_policy_json,optimization_preference,quality_floor_json,max_concurrency,max_host_delegation_depth,budget_ceiling_json,latency_class,environment_policy_json,native_delegation_policy,warm_policy_json,authored_by_json,created_at FROM delegation_profile_revisions WHERE workspace_id=?1 AND delegation_profile_id=?2 AND revision=?3", params![workspace,id,revision], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,r.get(11)?,r.get(12)?,r.get(13)?,r.get(14)?,r.get(15)?,r.get(16)?,r.get(17)?,r.get(18)?,r.get(19)?,r.get(20)?,r.get(21)?))).optional().map_err(profile_sql_error)?;
    let Some(r) = row else { return Ok(None) };
    Ok(Some(DelegationProfileRevision {
        delegation_profile_id:id.to_owned(), revision:r.0, workspace_id:r.1, name:r.2, name_key:r.3, routing_description:r.4, instructions:r.5,
        session_options:decode_json(&r.6).map_err(profile_store_error)?, session_options_descriptor_digest:r.7,
        required_features:serde_json::from_str(&r.8).map_err(|_| DelegationProfileError::Storage)?, preferred_features:serde_json::from_str(&r.9).map_err(|_| DelegationProfileError::Storage)?,
        enforced_policy:serde_json::from_str(&r.10).map_err(|_| DelegationProfileError::Storage)?, optimization_preference:parse_optimization(&r.11)?,
        quality_floor:decode_optional_json(r.12.as_deref()).map_err(profile_store_error)?, max_concurrency:r.13, max_host_delegation_depth:r.14,
        budget_ceiling:decode_optional_json(r.15.as_deref()).map_err(profile_store_error)?, latency_class:parse_latency(&r.16)?,
        environment_policy:serde_json::from_str(&r.17).map_err(|_| DelegationProfileError::Storage)?, native_delegation_policy:parse_native_policy(&r.18)?,
        warm_policy:decode_json(&r.19).map_err(profile_store_error)?, authored_by:serde_json::from_str(&r.20).map_err(|_| DelegationProfileError::Storage)?, created_at:r.21,
    }))
}

fn profile_status(value: DelegationProfileStatus) -> &'static str { match value { DelegationProfileStatus::Enabled=>"ENABLED", DelegationProfileStatus::Disabled=>"DISABLED", DelegationProfileStatus::Archived=>"ARCHIVED" } }
fn parse_profile_status(value: &str) -> Result<DelegationProfileStatus, DelegationProfileError> { match value { "ENABLED"=>Ok(DelegationProfileStatus::Enabled), "DISABLED"=>Ok(DelegationProfileStatus::Disabled), "ARCHIVED"=>Ok(DelegationProfileStatus::Archived), _=>Err(DelegationProfileError::Storage) } }
fn optimization_preference(value: domain_responsibility::OptimizationPreference) -> &'static str { match value { domain_responsibility::OptimizationPreference::QualityFirst=>"QUALITY_FIRST", domain_responsibility::OptimizationPreference::Balanced=>"BALANCED", domain_responsibility::OptimizationPreference::CostFirst=>"COST_FIRST", domain_responsibility::OptimizationPreference::LatencyFirst=>"LATENCY_FIRST" } }
fn parse_optimization(value: &str) -> Result<domain_responsibility::OptimizationPreference, DelegationProfileError> { match value { "QUALITY_FIRST"=>Ok(domain_responsibility::OptimizationPreference::QualityFirst),"BALANCED"=>Ok(domain_responsibility::OptimizationPreference::Balanced),"COST_FIRST"=>Ok(domain_responsibility::OptimizationPreference::CostFirst),"LATENCY_FIRST"=>Ok(domain_responsibility::OptimizationPreference::LatencyFirst),_=>Err(DelegationProfileError::Storage) } }
fn latency_class(value: domain_responsibility::DelegationLatencyClass) -> &'static str { match value { domain_responsibility::DelegationLatencyClass::Standard=>"STANDARD", domain_responsibility::DelegationLatencyClass::Interactive=>"INTERACTIVE", domain_responsibility::DelegationLatencyClass::DeadlineSensitive=>"DEADLINE_SENSITIVE" } }
fn parse_latency(value: &str) -> Result<domain_responsibility::DelegationLatencyClass, DelegationProfileError> { match value { "STANDARD"=>Ok(domain_responsibility::DelegationLatencyClass::Standard),"INTERACTIVE"=>Ok(domain_responsibility::DelegationLatencyClass::Interactive),"DEADLINE_SENSITIVE"=>Ok(domain_responsibility::DelegationLatencyClass::DeadlineSensitive),_=>Err(DelegationProfileError::Storage) } }
fn native_delegation_policy(value: domain_responsibility::NativeDelegationPolicy) -> &'static str { match value { domain_responsibility::NativeDelegationPolicy::Inherit=>"INHERIT", domain_responsibility::NativeDelegationPolicy::Allow=>"ALLOW", domain_responsibility::NativeDelegationPolicy::DenyIfSupported=>"DENY_IF_SUPPORTED" } }
fn parse_native_policy(value: &str) -> Result<domain_responsibility::NativeDelegationPolicy, DelegationProfileError> { match value { "INHERIT"=>Ok(domain_responsibility::NativeDelegationPolicy::Inherit),"ALLOW"=>Ok(domain_responsibility::NativeDelegationPolicy::Allow),"DENY_IF_SUPPORTED"=>Ok(domain_responsibility::NativeDelegationPolicy::DenyIfSupported),_=>Err(DelegationProfileError::Storage) } }

fn profile_sql_error(error: rusqlite::Error) -> DelegationProfileError { profile_store_error(map_database_error(error)) }
fn profile_store_error(error: StoreError) -> DelegationProfileError { match error { StoreError::NotFound=>DelegationProfileError::NotFound, StoreError::Conflict{..}=>DelegationProfileError::VersionConflict, StoreError::Invalid(_)=>DelegationProfileError::InvalidDefinition, _=>DelegationProfileError::Storage } }
fn profile_domain_error(error: DelegationProfileError) -> StoreError { match error { DelegationProfileError::NotFound=>StoreError::NotFound, DelegationProfileError::VersionConflict=>StoreError::Conflict { expected: None, actual: None }, DelegationProfileError::InvalidDefinition | DelegationProfileError::OptionsInvalid=>StoreError::Invalid("DelegationProfile is invalid".into()), _=>StoreError::Database("DelegationProfile transaction failed".into()) } }
fn domain_store_error(error: StoreError) -> DelegationProfileError { profile_store_error(error) }

fn load_profile(connection: &Connection, workspace: &str, id: &str) -> Result<Value, StoreError> {
    let profile = connection
        .query_row(
            "SELECT delegation_profile_id, workspace_id, agent_binding_id, name, current_revision, status, created_at, updated_at, version \
             FROM delegation_profiles WHERE workspace_id = ?1 AND delegation_profile_id = ?2",
            params![workspace, id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, u64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, u64>(8)?,
                ))
            },
        )
        .map_err(map_database_error)?;
    let revision = connection
        .query_row(
            "SELECT revision, workspace_id, name, routing_description, instructions, session_options_json, \
                    session_options_descriptor_digest, required_features_json, preferred_features_json, \
                    enforced_policy_json, optimization_preference, quality_floor_json, max_concurrency, \
                    max_host_delegation_depth, budget_ceiling_json, latency_class, environment_policy_json, \
                    native_delegation_policy, warm_policy_json, authored_by_json, created_at \
             FROM delegation_profile_revisions \
             WHERE workspace_id = ?1 AND delegation_profile_id = ?2 AND revision = ?3",
            params![workspace, id, profile.4],
            |row| {
                Ok((
                    row.get::<_, u64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?, row.get::<_, String>(10)?, row.get::<_, Option<String>>(11)?,
                    row.get::<_, u32>(12)?, row.get::<_, u32>(13)?, row.get::<_, Option<String>>(14)?,
                    row.get::<_, String>(15)?, row.get::<_, String>(16)?, row.get::<_, String>(17)?,
                    row.get::<_, String>(18)?, row.get::<_, String>(19)?, row.get::<_, String>(20)?,
                ))
            },
        )
        .map_err(map_database_error)?;

    let view = json!({
        "delegation_profile_id": profile.0,
        "workspace_id": profile.1,
        "agent_binding_id": profile.2,
        "name": profile.3,
        "current_revision": profile.4,
        "revision": {
            "delegation_profile_id": id,
            "revision": revision.0,
            "workspace_id": revision.1,
            "name": revision.2,
            "routing_description": revision.3,
            "instructions": revision.4,
            "session_options": decode_json(&revision.5)?,
            "session_options_descriptor_digest": revision.6,
            "required_features": decode_json(&revision.7)?,
            "preferred_features": decode_json(&revision.8)?,
            "enforced_policy": decode_json(&revision.9)?,
            "optimization_preference": revision.10,
            "quality_floor": decode_optional_json(revision.11.as_deref())?,
            "max_concurrency": revision.12,
            "max_host_delegation_depth": revision.13,
            "budget_ceiling": decode_optional_json(revision.14.as_deref())?,
            "latency_class": revision.15,
            "environment_policy": decode_json(&revision.16)?,
            "native_delegation_policy": revision.17,
            "warm_policy": decode_json(&revision.18)?,
            "authored_by": decode_json(&revision.19)?,
            "created_at": revision.20,
        },
        "status": profile.5,
        "created_at": profile.6,
        "updated_at": profile.7,
        "version": profile.8,
    });
    Ok(view)
}

fn decode_json(value: &str) -> Result<Value, StoreError> {
    serde_json::from_str(value).map_err(|error| StoreError::Integrity(error.to_string()))
}

fn decode_optional_json(value: Option<&str>) -> Result<Value, StoreError> {
    value.map(decode_json).transpose().map(|value| value.unwrap_or(Value::Null))
}
