//! Durable Environment metadata persistence.
//!
//! This adapter persists Environment records and Runtime-local provider bindings. It
//! never calls an Environment provider; the opaque locator is written only to the
//! private binding table and is excluded from aggregate snapshots, events and receipts.

use super::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use storage_core::{
    CommittedEnvironment, EnvironmentConfig, EnvironmentCreateRequest, EnvironmentHealth,
    EnvironmentIdentity, EnvironmentLifecycleRequest, EnvironmentListRequest, EnvironmentOwner,
    EnvironmentRecord, EnvironmentRequestIdentity, EnvironmentSharingScope,
    EnvironmentSharingScopeChangeRequest, EnvironmentStatus, EnvironmentStore, LifecycleHolds,
    ProviderBindingCommit, RuntimeId, RuntimeIncarnationId, StoreError,
};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

#[derive(Clone)]
pub struct SqliteEnvironmentStore {
    store: SqliteWorkspaceStore,
}

impl SqliteEnvironmentStore {
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
            Command::EnvironmentOperation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }
}

impl EnvironmentStore for SqliteEnvironmentStore {
    fn list_environments(
        &self,
        request: EnvironmentListRequest,
    ) -> Result<Vec<EnvironmentRecord>, StoreError> {
        self.run(move |connection| {
            let workspace_id = request.workspace_id().as_str();
            let mut statement = connection
                .prepare(
                    "SELECT environment_id FROM environments
                     WHERE owner_workspace_id = ?1 AND (?2 IS NULL OR environment_id > ?2)
                     ORDER BY environment_id ASC LIMIT ?3",
                )
                .map_err(map_database_error)?;
            let ids = statement
                .query_map(
                    params![
                        workspace_id,
                        request.after_environment_id().map(|id| id.as_str()),
                        i64::from(request.limit()),
                    ],
                    |row| row.get::<_, String>(0),
                )
                .map_err(map_database_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_database_error)?;
            ids.into_iter()
                .map(|id| {
                    load_environment(connection, workspace_id, &id)?.ok_or(StoreError::NotFound)
                })
                .collect()
        })
    }

    fn get_environment(
        &self,
        workspace_id: &str,
        environment_id: &str,
    ) -> Result<Option<EnvironmentRecord>, StoreError> {
        let (workspace_id, environment_id) = (workspace_id.to_owned(), environment_id.to_owned());
        self.run(move |connection| load_environment(connection, &workspace_id, &environment_id))
    }

    fn create_environment(
        &self,
        request: EnvironmentCreateRequest,
    ) -> Result<CommittedEnvironment, StoreError> {
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| create_environment(connection, blobs.as_ref(), request))
    }

    fn transition_environment(
        &self,
        request: EnvironmentLifecycleRequest,
    ) -> Result<CommittedEnvironment, StoreError> {
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| transition_environment(connection, blobs.as_ref(), request))
    }

    fn change_environment_sharing_scope(
        &self,
        request: EnvironmentSharingScopeChangeRequest,
    ) -> Result<CommittedEnvironment, StoreError> {
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| {
            change_environment_sharing_scope(connection, blobs.as_ref(), request)
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EnvironmentReceipt {
    identity: EnvironmentIdentity,
    config: EnvironmentConfig,
    status: EnvironmentStatus,
    health: EnvironmentHealth,
    created_at: String,
    updated_at: String,
    version: u64,
}

impl From<&EnvironmentRecord> for EnvironmentReceipt {
    fn from(record: &EnvironmentRecord) -> Self {
        Self {
            identity: record.identity().clone(),
            config: record.config().clone(),
            status: record.status(),
            health: record.health(),
            created_at: record.created_at().to_owned(),
            updated_at: record.updated_at().to_owned(),
            version: record.version(),
        }
    }
}

impl EnvironmentReceipt {
    fn restore(self) -> Result<EnvironmentRecord, StoreError> {
        domain_environment::restore_environment(
            self.identity,
            self.config,
            self.status,
            self.health,
            self.created_at,
            self.updated_at,
            self.version,
        )
        .map_err(|error| {
            StoreError::Integrity(format!("stored Environment receipt is invalid: {error}"))
        })
    }
}

fn create_environment(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    request: EnvironmentCreateRequest,
) -> Result<CommittedEnvironment, StoreError> {
    let identity = request.identity();
    let record = request.record();
    validate_request_digest(identity)?;
    validate_event(
        request.event(),
        record,
        "environment.created.v1",
        Some((EnvironmentStatus::New, EnvironmentStatus::Provisioning)),
    )?;
    let request_digest = operation_digest(
        "environment.create",
        identity,
        record.identity().owner_workspace_id.as_str(),
        record.identity().environment_id.as_str(),
    )?;
    let principal_id = identity.principal_id().as_str().to_owned();
    let request_id = identity.request_id().to_owned();
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;

    // Authorization and replay resolution share the write transaction, so a
    // Workspace cannot become archived or change owners between the check and
    // returning a prior receipt.
    validate_create_owner_scope(&tx, identity, record, request.event())?;
    if let Some(replayed) = read_receipt(&tx, &principal_id, &request_id, &request_digest)? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replayed);
    }

    // New admission also verifies the Runtime's current incarnation before any
    // immutable aggregate blob is written. A failed SQL commit may leave an
    // unreferenced blob for normal garbage collection, but replay/auth failures do not.
    validate_create_scope(&tx, identity, record, request.event())?;
    let state_ref = put_environment_state(blobs, record)?;
    let version = to_sql_i64(record.version(), "Environment version")?;
    insert_environment(&tx, record)?;
    let event = insert_domain_event(&tx, request.event(), &state_ref)?;
    let committed = CommittedEnvironment {
        record: record.clone(),
        replayed: false,
    };
    save_receipt(
        &tx,
        &principal_id,
        &request_id,
        &request_digest,
        &committed,
        request.event().recorded_at.as_str(),
    )?;
    let _ = (event, version);
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn transition_environment(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    request: EnvironmentLifecycleRequest,
) -> Result<CommittedEnvironment, StoreError> {
    let identity = request.identity();
    let proposed = request.proposed();
    validate_request_digest(identity)?;
    validate_event_identity(request.event(), proposed, "environment.state.changed.v1")?;
    let request_digest = operation_digest(
        "environment.transition",
        identity,
        request.workspace_id().as_str(),
        request.environment_id().as_str(),
    )?;
    let principal_id = identity.principal_id().as_str().to_owned();
    let request_id = identity.request_id().to_owned();
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    validate_workspace_mutation_scope(&tx, request.workspace_id().as_str(), &principal_id)?;
    if let Some(replayed) = read_receipt(&tx, &principal_id, &request_id, &request_digest)? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replayed);
    }
    let workspace_id = request.workspace_id().as_str();
    let environment_id = request.environment_id().as_str();
    let current =
        load_environment(&tx, workspace_id, environment_id)?.ok_or(StoreError::NotFound)?;
    validate_workspace_mutation_scope(&tx, workspace_id, &principal_id)?;
    let expected = request.expected();
    if current.version() != expected.version || current.status() != expected.status {
        return Err(StoreError::Conflict {
            expected: Some(expected.version),
            actual: Some(current.version()),
        });
    }
    validate_event(
        request.event(),
        proposed,
        "environment.state.changed.v1",
        Some((current.status(), proposed.status())),
    )?;
    if current.identity() != proposed.identity() || current.config() != proposed.config() {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_IMMUTABLE_IDENTITY_CHANGED".to_owned(),
        ));
    }
    if proposed.updated_at() != request.event().recorded_at {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_EVENT_TIME_MISMATCH".to_owned(),
        ));
    }
    domain_environment::validate_transition_proposal(&current, expected, proposed).map_err(
        |error| StoreError::Invalid(format!("Environment transition rejected: {error}")),
    )?;
    if request.environment_id().as_str() != proposed.identity().environment_id.as_str()
        || request.workspace_id().as_str() != proposed.identity().owner_workspace_id.as_str()
    {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_REQUEST_SCOPE_MISMATCH".to_owned(),
        ));
    }
    validate_transition_holds(&tx, environment_id, proposed.status())?;
    match proposed.status() {
        EnvironmentStatus::Ready => {
            let binding = request.provider_binding().ok_or_else(|| {
                StoreError::Invalid("ENVIRONMENT_PROVIDER_BINDING_REQUIRED".to_owned())
            })?;
            validate_and_insert_provider_binding(&tx, proposed, binding)?;
        }
        EnvironmentStatus::Busy | EnvironmentStatus::Checkpointing => {
            if !has_current_provider_binding(&tx, proposed, None)? {
                return Err(StoreError::Invalid(
                    "ENVIRONMENT_CURRENT_PROVIDER_BINDING_REQUIRED".to_owned(),
                ));
            }
        }
        _ => {
            if request.provider_binding().is_some() {
                return Err(StoreError::Invalid(
                    "ENVIRONMENT_UNEXPECTED_PROVIDER_BINDING".to_owned(),
                ));
            }
        }
    }
    let state_ref = put_environment_state(blobs, proposed)?;
    let changed = tx
        .execute(
            "UPDATE environments SET status = ?1, health = ?2, updated_at = ?3, version = ?4
             WHERE environment_id = ?5 AND owner_workspace_id = ?6 AND version = ?7 AND status = ?8",
            params![
                enum_text(proposed.status())?,
                enum_text(proposed.health())?,
                proposed.updated_at(),
                to_sql_i64(proposed.version(), "Environment version")?,
                environment_id,
                workspace_id,
                to_sql_i64(expected.version, "expected Environment version")?,
                enum_text(expected.status)?,
            ],
        )
        .map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(expected.version),
            actual: None,
        });
    }
    let event = insert_domain_event(&tx, request.event(), &state_ref)?;
    let committed = CommittedEnvironment {
        record: proposed.clone(),
        replayed: false,
    };
    save_receipt(
        &tx,
        &principal_id,
        &request_id,
        &request_digest,
        &committed,
        request.event().recorded_at.as_str(),
    )?;
    let _ = event;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn change_environment_sharing_scope(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    request: EnvironmentSharingScopeChangeRequest,
) -> Result<CommittedEnvironment, StoreError> {
    let identity = request.identity();
    validate_request_digest(identity)?;
    validate_sharing_change_request_body(&request)?;
    let request_digest = operation_digest(
        "environment.sharing_scope.change",
        identity,
        request.workspace_id().as_str(),
        request.environment_id().as_str(),
    )?;
    let principal_id = identity.principal_id().as_str().to_owned();
    let request_id = identity.request_id().to_owned();
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    validate_workspace_mutation_scope(&tx, request.workspace_id().as_str(), &principal_id)?;
    if let Some(replayed) = read_receipt(&tx, &principal_id, &request_id, &request_digest)? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replayed);
    }

    let workspace_id = request.workspace_id().as_str();
    let environment_id = request.environment_id().as_str();
    let current =
        load_environment(&tx, workspace_id, environment_id)?.ok_or(StoreError::NotFound)?;
    validate_workspace_mutation_scope(&tx, workspace_id, &principal_id)?;
    if current.version() != request.expected_version() {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_version()),
            actual: Some(current.version()),
        });
    }

    let target_coworker_id = request.target_coworker_id().map(|id| id.as_str());
    if request.target_scope() == EnvironmentSharingScope::CoworkerPrivate {
        let selectable: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM coworkers WHERE workspace_id = ?1 AND coworker_id = ?2 AND status IN ('ACTIVE','PAUSED'))",
            params![workspace_id, target_coworker_id],
            |row| row.get(0),
        ).map_err(map_database_error)?;
        if !selectable {
            return Err(StoreError::NotFound);
        }
    }
    let holds = read_sharing_scope_holds(&tx, environment_id)?;
    let recorded_at = request.event().recorded_at.clone();
    let proposed = domain_environment::change_sharing_scope(
        &current,
        storage_core::ExpectedState {
            version: current.version(),
            status: current.status(),
        },
        request.target_scope(),
        request.target_coworker_id().cloned(),
        holds,
        recorded_at,
    )
    .map_err(|error| {
        StoreError::Invalid(format!(
            "Environment sharing-scope change rejected: {error}"
        ))
    })?;

    validate_sharing_scope_event(&request, &current, &proposed)?;
    let state_ref = put_environment_state(blobs, &proposed)?;
    let changed = tx.execute(
        "UPDATE environments SET sharing_scope = ?1, owner_coworker_id = ?2, updated_at = ?3, version = ?4
         WHERE environment_id = ?5 AND owner_workspace_id = ?6 AND version = ?7 AND status = 'SUSPENDED'",
        params![
            enum_text(proposed.config().sharing_scope)?,
            proposed.identity().owner.coworker_id.as_ref().map(|id| id.as_str()),
            proposed.updated_at(),
            to_sql_i64(proposed.version(), "Environment version")?,
            environment_id,
            workspace_id,
            to_sql_i64(request.expected_version(), "expected Environment version")?,
        ],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_version()),
            actual: None,
        });
    }
    insert_domain_event(&tx, request.event(), &state_ref)?;
    let committed = CommittedEnvironment {
        record: proposed,
        replayed: false,
    };
    save_receipt(
        &tx,
        &principal_id,
        &request_id,
        &request_digest,
        &committed,
        request.event().recorded_at.as_str(),
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn validate_sharing_change_request_body(
    request: &EnvironmentSharingScopeChangeRequest,
) -> Result<(), StoreError> {
    let expected = json!({
        "environment_id": request.environment_id().as_str(),
        "expected_version": request.expected_version(),
        "target_sharing_scope": enum_text(request.target_scope())?,
        "target_coworker_id": request.target_coworker_id().map(|id| id.as_str()),
    });
    if canonical_json(&expected)? != request.identity().canonical_request_body() {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_SHARING_REQUEST_BODY_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn validate_sharing_scope_event(
    request: &EnvironmentSharingScopeChangeRequest,
    current: &EnvironmentRecord,
    proposed: &EnvironmentRecord,
) -> Result<(), StoreError> {
    let event = request.event();
    let expected_version = request.expected_version().checked_add(1).ok_or_else(|| {
        StoreError::Invalid("Environment sharing-scope version exhausted".to_owned())
    })?;
    let expected_payload = json!({
        "environment_id": request.environment_id().as_str(),
        "from": enum_text(current.config().sharing_scope)?,
        "to": enum_text(proposed.config().sharing_scope)?,
        "changed_by": { "kind": "USER", "principal_id": request.identity().principal_id().as_str() },
        "aggregate_version": expected_version,
    });
    if event.schema_version != 1
        || event.event_type != "environment.sharing_scope.changed.v1"
        || event.workspace_id != request.workspace_id().as_str()
        || event.entity_type != "Environment"
        || event.entity_id != request.environment_id().as_str()
        || event.origin_runtime_id != current.identity().runtime_id.as_str()
        || event.entity_revision != expected_version
        || event.payload != expected_payload
        || canonicalize_utc_timestamp(&event.recorded_at)? != event.recorded_at
        || proposed.updated_at() != event.recorded_at
    {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_SHARING_EVENT_INVALID".to_owned(),
        ));
    }
    Ok(())
}

fn read_sharing_scope_holds(
    connection: &Connection,
    environment_id: &str,
) -> Result<LifecycleHolds, StoreError> {
    let active_attempts: i64 = connection.query_row(
        "SELECT COUNT(*) FROM attempts WHERE environment_id = ?1 AND status NOT IN ('COMPLETED','FAILED','ABANDONED','CANCELLED')",
        [environment_id], |row| row.get(0),
    ).map_err(map_database_error)?;
    let unsettled_invocations: i64 = connection.query_row(
        "SELECT COUNT(*) FROM capability_invocations i JOIN attempts a ON a.attempt_id = i.attempt_id WHERE a.environment_id = ?1 AND i.status NOT IN ('SUCCEEDED','FAILED','CANCELLED')",
        [environment_id], |row| row.get(0),
    ).map_err(map_database_error)?;
    let unsettled_control_leases: i64 = connection.query_row(
        "SELECT COUNT(*) FROM environment_control_leases WHERE environment_id = ?1 AND state IN ('ACTIVE','RELEASING')",
        [environment_id], |row| row.get(0),
    ).map_err(map_database_error)?;
    let unresolved_effects: i64 = connection.query_row(
        "SELECT COUNT(*) FROM effects e JOIN attempts a ON a.task_id = e.task_id AND a.attempt_id = e.attempt_id WHERE a.environment_id = ?1 AND e.state IN ('PROPOSED','STARTED','ACKNOWLEDGED','RECONCILING','AMBIGUOUS')",
        [environment_id], |row| row.get(0),
    ).map_err(map_database_error)?;
    // Checkpoints have no independent release/hold state yet. Treat every retained
    // checkpoint row as a hold so this command cannot guess that provider state is
    // disposable. A typed checkpoint-hold lifecycle can narrow this conservative rule.
    let checkpoint_holds: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM environment_checkpoints WHERE environment_id = ?1",
            [environment_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    Ok(LifecycleHolds {
        active_attempts: u32::try_from(active_attempts).map_err(|_| {
            StoreError::Integrity("active Environment Attempt count is out of range".to_owned())
        })?,
        unsettled_invocations: u32::try_from(unsettled_invocations).map_err(|_| {
            StoreError::Integrity(
                "unsettled Environment Invocation count is out of range".to_owned(),
            )
        })?,
        unsettled_control_leases: u32::try_from(unsettled_control_leases).map_err(|_| {
            StoreError::Integrity(
                "active Environment control-lease count is out of range".to_owned(),
            )
        })?,
        checkpoint_holds: u32::try_from(checkpoint_holds).map_err(|_| {
            StoreError::Integrity("Environment checkpoint count is out of range".to_owned())
        })?,
        unresolved_effects: u32::try_from(unresolved_effects).map_err(|_| {
            StoreError::Integrity("unresolved Environment Effect count is out of range".to_owned())
        })?,
    })
}

fn validate_request_digest(identity: &EnvironmentRequestIdentity) -> Result<(), StoreError> {
    let value: Value = serde_json::from_slice(identity.canonical_request_body())
        .map_err(|_| StoreError::Invalid("ENVIRONMENT_REQUEST_BODY_INVALID".to_owned()))?;
    if canonical_json(&value)? != identity.canonical_request_body()
        || digest(identity.canonical_request_body()) != identity.canonical_request_digest()
    {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_REQUEST_DIGEST_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn operation_digest(
    operation: &str,
    identity: &EnvironmentRequestIdentity,
    workspace_id: &str,
    environment_id: &str,
) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&json!({
        "operation": operation,
        "workspace_id": workspace_id,
        "environment_id": environment_id,
        "request_digest": identity.canonical_request_digest(),
    }))?))
}

fn validate_workspace_mutation_scope(
    connection: &Connection,
    workspace_id: &str,
    principal_id: &str,
) -> Result<(), StoreError> {
    let status: Option<(String, String)> = connection
        .query_row(
            "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
            [workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((owner, status)) = status else {
        return Err(StoreError::NotFound);
    };
    if owner != principal_id {
        return Err(StoreError::NotFound);
    }
    if status != "ACTIVE" {
        return Err(StoreError::Invalid("WORKSPACE_ARCHIVED".to_owned()));
    }
    Ok(())
}

fn validate_create_owner_scope(
    connection: &Connection,
    identity: &EnvironmentRequestIdentity,
    record: &EnvironmentRecord,
    event: &EventDraft,
) -> Result<(), StoreError> {
    let env = record.identity();
    let workspace_id = env.owner_workspace_id.as_str();
    if identity.principal_id().as_str().trim().is_empty()
        || event.workspace_id != workspace_id
        || event.entity_type != "Environment"
        || event.entity_id != env.environment_id.as_str()
        || event.origin_runtime_id != env.runtime_id.as_str()
        || event.entity_revision != record.version()
        || record.created_at() != event.recorded_at
        || record.updated_at() != event.recorded_at
    {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_CREATE_IDENTITY_INVALID".to_owned(),
        ));
    }
    let (owner, workspace_status): (String, String) = connection
        .query_row(
            "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
            [workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;
    if owner != identity.principal_id().as_str() {
        return Err(StoreError::NotFound);
    }
    if workspace_status != "ACTIVE" {
        return Err(StoreError::Invalid("WORKSPACE_ARCHIVED".to_owned()));
    }
    validate_record(record)
}

fn validate_create_scope(
    connection: &Connection,
    identity: &EnvironmentRequestIdentity,
    record: &EnvironmentRecord,
    event: &EventDraft,
) -> Result<(), StoreError> {
    validate_create_owner_scope(connection, identity, record, event)?;
    let env = record.identity();
    let workspace_id = env.owner_workspace_id.as_str();
    let runtime: Option<(String, String, String)> = connection
        .query_row(
            "SELECT r.current_incarnation_id, r.availability, i.recovery_state
             FROM runtimes r
             JOIN runtime_incarnations i ON i.runtime_id = r.runtime_id
                 AND i.runtime_incarnation_id = r.current_incarnation_id
             WHERE r.runtime_id = ?1
               AND EXISTS (
                 SELECT 1 FROM runtime_workspace_bindings b
                 WHERE b.runtime_id = r.runtime_id AND b.workspace_id = ?2
                   AND b.status = 'ACTIVE' AND b.revoked_at IS NULL
                   AND EXISTS (SELECT 1 FROM json_each(b.roles_json) role WHERE role.value = 'EXECUTOR')
               )",
            params![env.runtime_id.as_str(), workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((incarnation, availability, recovery)) = runtime else {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_RUNTIME_NOT_CURRENT_EXECUTOR".to_owned(),
        ));
    };
    if env.created_by_incarnation_id.as_ref().map(|id| id.as_str()) != Some(incarnation.as_str())
        || !matches!(
            (availability.as_str(), recovery.as_str()),
            ("ONLINE", "READY") | ("DEGRADED", "DEGRADED")
        )
    {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_RUNTIME_NOT_READY".to_owned(),
        ));
    }
    Ok(())
}

fn validate_record(record: &EnvironmentRecord) -> Result<(), StoreError> {
    domain_environment::restore_environment(
        record.identity().clone(),
        record.config().clone(),
        record.status(),
        record.health(),
        record.created_at().to_owned(),
        record.updated_at().to_owned(),
        record.version(),
    )
    .map(|_| ())
    .map_err(|error| StoreError::Invalid(format!("Environment record rejected: {error}")))
}

fn validate_event(
    event: &EventDraft,
    record: &EnvironmentRecord,
    expected_type: &str,
    transition: Option<(EnvironmentStatus, EnvironmentStatus)>,
) -> Result<(), StoreError> {
    validate_event_identity(event, record, expected_type)?;
    let reason_code = event.payload.get("reason_code").and_then(Value::as_str);
    if reason_code.is_some_and(|value| {
        value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    }) {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_EVENT_REASON_INVALID".to_owned(),
        ));
    }
    let from = transition.map(|(from, _)| enum_text(from)).transpose()?;
    let to = transition
        .map(|(_, to)| enum_text(to))
        .transpose()?
        .unwrap_or(enum_text(record.status())?);
    let mut expected = json!({
        "environment_id": record.identity().environment_id.as_str(),
        "runtime_id": record.identity().runtime_id.as_str(),
        "provider_kind": record.config().provider_kind,
        "to": to,
    });
    if let Some(from) = from {
        expected["from"] = json!(from);
    }
    if expected_type == "environment.created.v1" {
        expected["lifetime"] = json!(enum_text(record.config().lifetime)?);
        if let Some(digest) = &record.config().provision_preview_digest {
            expected["provision_preview_digest"] = json!(digest);
        }
    }
    if let Some(reason) = reason_code {
        expected["reason_code"] = json!(reason);
    }
    if event.payload != expected {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_EVENT_PAYLOAD_INVALID".to_owned(),
        ));
    }
    Ok(())
}

fn validate_event_identity(
    event: &EventDraft,
    record: &EnvironmentRecord,
    expected_type: &str,
) -> Result<(), StoreError> {
    if event.schema_version != 1
        || event.event_type != expected_type
        || event.workspace_id != record.identity().owner_workspace_id.as_str()
        || event.entity_type != "Environment"
        || event.entity_id != record.identity().environment_id.as_str()
        || event.origin_runtime_id != record.identity().runtime_id.as_str()
        || event.entity_revision != record.version()
        || canonicalize_utc_timestamp(&event.recorded_at)? != event.recorded_at
    {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_EVENT_IDENTITY_INVALID".to_owned(),
        ));
    }
    Ok(())
}

fn validate_transition_holds(
    connection: &Connection,
    environment_id: &str,
    proposed: EnvironmentStatus,
) -> Result<(), StoreError> {
    if proposed == EnvironmentStatus::Checkpointing {
        let active_attempts: i64 = connection.query_row(
            "SELECT COUNT(*) FROM attempts WHERE environment_id = ?1 AND status IN ('CREATED','PREPARING','RUNNING','WAITING_APPROVAL','WAITING_RESOURCE','CHECKPOINTING','CANCEL_REQUESTED')",
            [environment_id], |row| row.get(0),
        ).map_err(map_database_error)?;
        let unsettled_invocations: i64 = connection.query_row(
            "SELECT COUNT(*) FROM capability_invocations ci JOIN attempts a ON a.attempt_id = ci.attempt_id WHERE a.environment_id = ?1 AND ci.status IN ('CREATED','DISPATCHED','WAITING','INPUT_REQUIRED','CANCEL_REQUESTED','AMBIGUOUS')",
            [environment_id], |row| row.get(0),
        ).map_err(map_database_error)?;
        let active_controls: i64 = connection.query_row(
            "SELECT COUNT(*) FROM environment_control_leases WHERE environment_id = ?1 AND state IN ('ACTIVE','RELEASING')",
            [environment_id], |row| row.get(0),
        ).map_err(map_database_error)?;
        let unresolved_effects: i64 = connection.query_row(
            "SELECT COUNT(*) FROM effects e JOIN attempts a ON a.attempt_id = e.attempt_id WHERE a.environment_id = ?1 AND e.state IN ('PROPOSED','STARTED','ACKNOWLEDGED','RECONCILING','AMBIGUOUS')",
            [environment_id], |row| row.get(0),
        ).map_err(map_database_error)?;
        if active_attempts > 0
            || unsettled_invocations > 0
            || active_controls > 0
            || unresolved_effects > 0
        {
            return Err(StoreError::Invalid("ENVIRONMENT_LIFECYCLE_HELD".to_owned()));
        }
    } else if proposed == EnvironmentStatus::Destroying {
        // This low-level store cannot prove retention expiry or that required
        // outputs were committed. Fail closed until that admission contract is
        // supplied by durable authoritative state.
        return Err(StoreError::Invalid(
            "ENVIRONMENT_DESTROY_ADMISSION_UNAVAILABLE".to_owned(),
        ));
    }
    Ok(())
}

fn insert_environment(tx: &Transaction<'_>, record: &EnvironmentRecord) -> Result<(), StoreError> {
    let identity = record.identity();
    let config = record.config();
    tx.execute(
        "INSERT INTO environments(
            environment_id,runtime_id,provider_kind,class,lifetime,owner_workspace_id,
            owner_task_id,owner_attempt_id,owner_coworker_id,owner_principal_id,sharing_scope,
            name,created_by_incarnation_id,status,health,budget_enforcement_policy,
            budget_enforcement,source_resources_json,resource_limits_json,network_policy_json,
            budget_ceiling_json,provision_preview_digest,retention_expires_at,backup_policy,
            isolation_json,created_at,updated_at,version
         ) VALUES (
            ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,
            ?21,?22,?23,?24,?25,?26,?27,?28
         )",
        params![
            identity.environment_id.as_str(),
            identity.runtime_id.as_str(),
            config.provider_kind,
            enum_text(config.class)?,
            enum_text(config.lifetime)?,
            identity.owner_workspace_id.as_str(),
            identity.owner.task_id.as_ref().map(|id| id.as_str()),
            identity.owner.attempt_id.as_ref().map(|id| id.as_str()),
            identity.owner.coworker_id.as_ref().map(|id| id.as_str()),
            identity.owner.principal_id.as_ref().map(|id| id.as_str()),
            enum_text(config.sharing_scope)?,
            config.name,
            identity
                .created_by_incarnation_id
                .as_ref()
                .map(|id| id.as_str()),
            enum_text(record.status())?,
            enum_text(record.health())?,
            enum_text(config.budget_enforcement_policy)?,
            enum_text(config.budget_enforcement)?,
            json_text(&config.source_resources)?,
            json_text(&config.resource_limits)?,
            json_text(&config.network_policy)?,
            json_text(&config.budget_ceiling)?,
            config.provision_preview_digest,
            config.retention_expires_at,
            enum_text(config.backup_policy)?,
            json_text(&config.isolation)?,
            record.created_at(),
            record.updated_at(),
            to_sql_i64(record.version(), "Environment version")?,
        ],
    )
    .map_err(map_database_error)?;
    Ok(())
}

fn load_environment(
    connection: &Connection,
    workspace_id: &str,
    environment_id: &str,
) -> Result<Option<EnvironmentRecord>, StoreError> {
    type Row = (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
        String,
        i64,
    );
    let row: Option<Row> = connection.query_row(
        "SELECT environment_id,runtime_id,provider_kind,class,lifetime,owner_task_id,owner_attempt_id,
                owner_coworker_id,owner_principal_id,sharing_scope,name,created_by_incarnation_id,
                status,health,budget_enforcement_policy,budget_enforcement,source_resources_json,
                resource_limits_json,network_policy_json,budget_ceiling_json,provision_preview_digest,
                retention_expires_at,backup_policy,isolation_json,created_at,updated_at,version
         FROM environments WHERE owner_workspace_id = ?1 AND environment_id = ?2",
        params![workspace_id, environment_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?,row.get(12)?,row.get(13)?,row.get(14)?,row.get(15)?,row.get(16)?,row.get(17)?,row.get(18)?,row.get(19)?,row.get(20)?,row.get(21)?,row.get(22)?,row.get(23)?,row.get(24)?,row.get(25)?,row.get(26)?)),
    ).optional().map_err(map_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let identity = EnvironmentIdentity {
        environment_id: storage_core::EnvironmentId::new(row.0).map_err(integrity_error)?,
        runtime_id: RuntimeId::new(row.1).map_err(integrity_error)?,
        owner_workspace_id: storage_core::WorkspaceId::new(workspace_id)
            .map_err(integrity_error)?,
        owner: EnvironmentOwner {
            task_id: row
                .5
                .map(storage_core::TaskId::new)
                .transpose()
                .map_err(integrity_error)?,
            attempt_id: row
                .6
                .map(storage_core::AttemptId::new)
                .transpose()
                .map_err(integrity_error)?,
            coworker_id: row
                .7
                .map(storage_core::CoworkerId::new)
                .transpose()
                .map_err(integrity_error)?,
            principal_id: row
                .8
                .map(storage_core::PrincipalId::new)
                .transpose()
                .map_err(integrity_error)?,
        },
        created_by_incarnation_id: row
            .11
            .map(RuntimeIncarnationId::new)
            .transpose()
            .map_err(integrity_error)?,
    };
    let config = EnvironmentConfig {
        name: row.10,
        provider_kind: row.2,
        class: parse_enum(row.3)?,
        lifetime: parse_enum(row.4)?,
        sharing_scope: parse_enum(row.9)?,
        source_resources: parse_json(row.16)?,
        resource_limits: parse_json(row.17)?,
        network_policy: parse_json(row.18)?,
        budget_ceiling: parse_json(row.19)?,
        budget_enforcement_policy: parse_enum(row.14)?,
        budget_enforcement: parse_enum(row.15)?,
        provision_preview_digest: row.20,
        retention_expires_at: row.21,
        backup_policy: parse_enum(row.22)?,
        isolation: parse_json(row.23)?,
    };
    let record = domain_environment::restore_environment(
        identity,
        config,
        parse_enum(row.12)?,
        parse_enum(row.13)?,
        row.24,
        row.25,
        from_sql_i64(row.26, "Environment version")?,
    )
    .map_err(|error| StoreError::Integrity(format!("persisted Environment is invalid: {error}")))?;
    Ok(Some(record))
}

fn validate_and_insert_provider_binding(
    tx: &Transaction<'_>,
    record: &EnvironmentRecord,
    binding: &ProviderBindingCommit,
) -> Result<(), StoreError> {
    let now = sqlite_now(tx)?;
    let identity = record.identity();
    if binding.environment_id().as_str() != identity.environment_id.as_str()
        || binding.runtime_id().as_str() != identity.runtime_id.as_str()
        || binding.provider_kind() != record.config().provider_kind
        || canonicalize_utc_timestamp(binding.observed_at())? != binding.observed_at()
        || timestamp_unix_ms(binding.observed_at())? > timestamp_unix_ms(&now)?
        || binding.expires_at().is_some_and(|expiry| {
            canonicalize_utc_timestamp(expiry).ok().as_deref() != Some(expiry)
                || timestamp_unix_ms(expiry).ok() <= timestamp_unix_ms(&now).ok()
        })
    {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_PROVIDER_BINDING_INVALID".to_owned(),
        ));
    }
    let current_incarnation: Option<String> = tx
        .query_row(
            "SELECT current_incarnation_id FROM runtimes WHERE runtime_id = ?1",
            [identity.runtime_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?
        .flatten();
    if current_incarnation.as_deref() != Some(binding.runtime_incarnation_id().as_str()) {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_PROVIDER_BINDING_STALE_INCARNATION".to_owned(),
        ));
    }
    let existing: Option<(String, String)> = tx
        .query_row(
            "SELECT runtime_id, provider_kind FROM environment_provider_bindings
             WHERE environment_id = ?1 AND runtime_incarnation_id = ?2",
            params![
                binding.environment_id().as_str(),
                binding.runtime_incarnation_id().as_str()
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    if let Some((runtime_id, provider_kind)) = existing {
        if runtime_id != binding.runtime_id().as_str() || provider_kind != binding.provider_kind() {
            return Err(StoreError::Invalid(
                "ENVIRONMENT_PROVIDER_BINDING_IDENTITY_MISMATCH".to_owned(),
            ));
        }
        let changed = tx
            .execute(
                "UPDATE environment_provider_bindings
                 SET opaque_locator_ref = ?1, observed_at = ?2, expires_at = ?3
                 WHERE environment_id = ?4 AND runtime_id = ?5
                   AND runtime_incarnation_id = ?6 AND provider_kind = ?7",
                params![
                    binding.opaque_locator_ref(),
                    binding.observed_at(),
                    binding.expires_at(),
                    binding.environment_id().as_str(),
                    binding.runtime_id().as_str(),
                    binding.runtime_incarnation_id().as_str(),
                    binding.provider_kind(),
                ],
            )
            .map_err(map_database_error)?;
        if changed != 1 {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
    } else {
        tx.execute(
            "INSERT INTO environment_provider_bindings(environment_id,runtime_id,runtime_incarnation_id,provider_kind,opaque_locator_ref,observed_at,expires_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                binding.environment_id().as_str(), binding.runtime_id().as_str(),
                binding.runtime_incarnation_id().as_str(), binding.provider_kind(),
                binding.opaque_locator_ref(), binding.observed_at(), binding.expires_at(),
            ],
        ).map_err(map_database_error)?;
    }
    if !has_current_provider_binding(tx, record, Some(binding.runtime_incarnation_id().as_str()))? {
        return Err(StoreError::Invalid(
            "ENVIRONMENT_PROVIDER_BINDING_NOT_CURRENT".to_owned(),
        ));
    }
    Ok(())
}

fn has_current_provider_binding(
    connection: &Connection,
    record: &EnvironmentRecord,
    expected_incarnation: Option<&str>,
) -> Result<bool, StoreError> {
    let now = sqlite_now(connection)?;
    let (runtime, environment, provider) = (
        record.identity().runtime_id.as_str(),
        record.identity().environment_id.as_str(),
        record.config().provider_kind.as_str(),
    );
    let found: Option<String> = connection.query_row(
        "SELECT b.runtime_incarnation_id FROM environment_provider_bindings b
         JOIN runtimes r ON r.runtime_id = b.runtime_id AND r.current_incarnation_id = b.runtime_incarnation_id
         WHERE b.environment_id = ?1 AND b.runtime_id = ?2 AND b.provider_kind = ?3
           AND (b.expires_at IS NULL OR julianday(b.expires_at) > julianday(?4))
           AND julianday(b.observed_at) <= julianday(?4)
           AND (?5 IS NULL OR b.runtime_incarnation_id = ?5)",
        params![environment, runtime, provider, now, expected_incarnation],
        |row| row.get(0),
    ).optional().map_err(map_database_error)?;
    Ok(found.is_some())
}

fn put_environment_state(
    blobs: &dyn BlobStore,
    record: &EnvironmentRecord,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(&EnvironmentReceipt::from(record))?;
    let blob = blobs.put(
        record.identity().owner_workspace_id.as_str(),
        BlobPurpose::AggregateState,
        &bytes,
        "application/vnd.litecowork.environment+json",
    )?;
    if blob.size_bytes != bytes.len() as u64
        || blobs.get(
            record.identity().owner_workspace_id.as_str(),
            BlobPurpose::AggregateState,
            &blob,
        )? != bytes
    {
        return Err(StoreError::Integrity(
            "Environment aggregate state failed blob verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: record.version(),
        record_schema_version: 1,
    })
}

fn read_receipt(
    connection: &Connection,
    principal_id: &str,
    request_id: &str,
    expected_digest: &str,
) -> Result<Option<CommittedEnvironment>, StoreError> {
    let receipt: Option<(String, Option<String>, Option<String>)> = connection.query_row(
        "SELECT request_digest,response_json,response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![principal_id, request_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    let Some((stored_digest, response, response_digest)) = receipt else {
        return Ok(None);
    };
    if stored_digest != expected_digest {
        return Err(StoreError::Invalid("IDEMPOTENCY_CONFLICT".to_owned()));
    }
    let response = response
        .ok_or_else(|| StoreError::Integrity("Environment receipt has no response".to_owned()))?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "Environment receipt digest mismatch".to_owned(),
        ));
    }
    let value: ReceiptResponse = serde_json::from_str(&response).map_err(|error| {
        StoreError::Integrity(format!("Environment receipt is malformed: {error}"))
    })?;
    let record = value.record.restore()?;
    if record.identity().environment_id.as_str() != value.environment_id {
        return Err(StoreError::Integrity(
            "Environment receipt identity mismatch".to_owned(),
        ));
    }
    Ok(Some(CommittedEnvironment {
        record,
        replayed: true,
    }))
}

#[derive(Deserialize, Serialize)]
struct ReceiptResponse {
    environment_id: String,
    record: EnvironmentReceipt,
}

fn save_receipt(
    tx: &Transaction<'_>,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
    committed: &CommittedEnvironment,
    recorded_at: &str,
) -> Result<(), StoreError> {
    let response = ReceiptResponse {
        environment_id: committed
            .record
            .identity()
            .environment_id
            .as_str()
            .to_owned(),
        record: EnvironmentReceipt::from(&committed.record),
    };
    let response_json = String::from_utf8(canonical_json(&response)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at)
         VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![principal_id, request_id, request_digest, response_json, digest(response_json.as_bytes()), recorded_at],
    ).map_err(map_database_error)?;
    Ok(())
}

fn enum_text<T: Serialize>(value: T) -> Result<String, StoreError> {
    serde_json::to_value(value)
        .map_err(|error| StoreError::Invalid(error.to_string()))?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| StoreError::Invalid("Environment enum did not serialize as text".to_owned()))
}

fn parse_enum<T: DeserializeOwned>(value: String) -> Result<T, StoreError> {
    serde_json::from_value(Value::String(value)).map_err(|error| {
        StoreError::Integrity(format!("stored Environment enum is invalid: {error}"))
    })
}

fn json_text<T: Serialize>(value: &T) -> Result<String, StoreError> {
    String::from_utf8(canonical_json(value)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))
}

fn parse_json<T: DeserializeOwned>(value: String) -> Result<T, StoreError> {
    serde_json::from_str(&value).map_err(|error| {
        StoreError::Integrity(format!("stored Environment JSON is invalid: {error}"))
    })
}

fn integrity_error(error: StoreError) -> StoreError {
    StoreError::Integrity(format!(
        "persisted Environment identity is invalid: {error}"
    ))
}

fn sqlite_now(connection: &Connection) -> Result<String, StoreError> {
    connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |row| {
            row.get(0)
        })
        .map_err(map_database_error)
}

fn timestamp_unix_ms(value: &str) -> Result<u64, StoreError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| StoreError::Invalid("Environment timestamp is invalid".to_owned()))?;
    u64::try_from(parsed.unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| StoreError::Invalid("Environment timestamp precedes Unix epoch".to_owned()))
}
