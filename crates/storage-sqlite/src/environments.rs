//! Durable Environment metadata persistence.
//!
//! This adapter persists Environment records and Runtime-local provider bindings. It
//! never calls an Environment provider; the opaque locator is written only to the
//! private binding table and is excluded from aggregate snapshots, events and receipts.

use super::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use storage_core::{
    CommittedEnvironment, CommittedEnvironmentControlLease, EnvironmentConfig,
    EnvironmentControlLeaseAcquire, EnvironmentControlLeaseEnd, EnvironmentControlLeaseEndState,
    EnvironmentControlLeaseRecord, EnvironmentControlLeaseState, EnvironmentControlLeaseStore,
    EnvironmentControlLeaseTakeover, EnvironmentControlOwner, EnvironmentCreateRequest,
    EnvironmentHealth, EnvironmentIdentity, EnvironmentLifecycleRequest, EnvironmentListRequest,
    EnvironmentOwner, EnvironmentRecord, EnvironmentRequestIdentity, EnvironmentSharingScope,
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

impl EnvironmentControlLeaseStore for SqliteEnvironmentStore {
    fn acquire_environment_control_lease(
        &self,
        request: EnvironmentControlLeaseAcquire,
    ) -> Result<CommittedEnvironmentControlLease, StoreError> {
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| acquire_control_lease(connection, blobs.as_ref(), request))
    }

    fn take_over_environment_control_lease(
        &self,
        request: EnvironmentControlLeaseTakeover,
    ) -> Result<CommittedEnvironmentControlLease, StoreError> {
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| takeover_control_lease(connection, blobs.as_ref(), request))
    }

    fn end_environment_control_lease(
        &self,
        request: EnvironmentControlLeaseEnd,
    ) -> Result<CommittedEnvironmentControlLease, StoreError> {
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| end_control_lease(connection, blobs.as_ref(), request))
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

#[derive(Deserialize, Serialize)]
struct ControlLeaseReceipt {
    lease: EnvironmentControlLeaseRecord,
}

fn acquire_control_lease(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    request: EnvironmentControlLeaseAcquire,
) -> Result<CommittedEnvironmentControlLease, StoreError> {
    validate_control_command_identity(
        &request.identity,
        &request.workspace_id,
        &request.event,
        &request.acquired_at,
    )?;
    validate_control_request_body(
        &request.identity,
        &json!({
            "workspace_id": request.workspace_id,
            "control_lease_id": request.control_lease_id,
            "environment_id": request.environment_id,
            "task_id": request.task_id,
            "attempt_id": request.attempt_id,
            "agent_session_id": request.agent_session_id,
            "runtime_id": request.runtime_id,
            "runtime_incarnation_id": request.runtime_incarnation_id,
            "acquired_at": request.acquired_at,
            "expires_at": request.expires_at,
        }),
    )?;
    if !valid_control_digest(&request.fencing_token_digest) || request.issuer_key_version == 0 {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_FENCING_CREDENTIAL_INVALID".to_owned(),
        ));
    }
    let principal_id = request.identity.principal_id().as_str().to_owned();
    let request_digest =
        control_request_digest("environment.control_lease.acquire", &request.identity)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    validate_workspace_mutation_scope(&tx, &request.workspace_id, &principal_id)?;
    if let Some(replayed) = read_control_lease_receipt(
        &tx,
        &principal_id,
        request.identity.request_id(),
        &request_digest,
    )? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replayed);
    }
    validate_current_attempt_binding(
        &tx,
        &request.workspace_id,
        &request.environment_id,
        &request.task_id,
        &request.attempt_id,
        &request.agent_session_id,
        &request.runtime_id,
        &request.runtime_incarnation_id,
        &request.acquired_at,
    )?;
    let max_epoch: Option<i64> = tx
        .query_row(
            "SELECT MAX(epoch) FROM environment_control_leases WHERE environment_id = ?1",
            [&request.environment_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let epoch = max_epoch
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("CONTROL_LEASE_EPOCH_EXHAUSTED".to_owned()))?;
    let epoch = u64::try_from(epoch)
        .map_err(|_| StoreError::Integrity("control lease epoch is out of range".to_owned()))?;
    let record = EnvironmentControlLeaseRecord {
        control_lease_id: request.control_lease_id.clone(),
        environment_id: request.environment_id.clone(),
        task_id: request.task_id.clone(),
        attempt_id: request.attempt_id.clone(),
        agent_session_id: request.agent_session_id.clone(),
        runtime_id: request.runtime_id.clone(),
        runtime_incarnation_id: request.runtime_incarnation_id.clone(),
        owner: EnvironmentControlOwner::Agent {
            agent_session_id: request.agent_session_id.clone(),
        },
        epoch,
        issuer_key_version: request.issuer_key_version,
        fencing_token_digest: request.fencing_token_digest.clone(),
        state: EnvironmentControlLeaseState::Active,
        acquired_at: request.acquired_at.clone(),
        expires_at: request.expires_at.clone(),
        version: 1,
    };
    validate_control_lease_record(&record)?;
    validate_control_lease_event(&request.event, &record, None, None)?;
    tx.execute(
        "INSERT INTO environment_control_leases(
            control_lease_id,environment_id,task_id,attempt_id,runtime_id,
            runtime_incarnation_id,owner_kind,owner_ref_json,epoch,issuer_key_version,
            fencing_token_digest,state,acquired_at,expires_at,version
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'ACTIVE',?12,?13,1)",
        params![
            record.control_lease_id,
            record.environment_id,
            record.task_id,
            record.attempt_id,
            record.runtime_id,
            record.runtime_incarnation_id,
            record.owner.kind(),
            json_text(&record.owner.reference_json()?)?,
            to_sql_i64(record.epoch, "control lease epoch")?,
            i64::from(record.issuer_key_version),
            record.fencing_token_digest,
            record.acquired_at,
            record.expires_at,
        ],
    )
    .map_err(map_database_error)?;
    let state_ref = put_control_lease_state(blobs, &request.workspace_id, &record)?;
    insert_domain_event(&tx, &request.event, &state_ref)?;
    let committed = CommittedEnvironmentControlLease {
        lease: record,
        replayed: false,
    };
    save_control_lease_receipt(
        &tx,
        &principal_id,
        request.identity.request_id(),
        &request_digest,
        &committed,
        &request.acquired_at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn takeover_control_lease(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    request: EnvironmentControlLeaseTakeover,
) -> Result<CommittedEnvironmentControlLease, StoreError> {
    validate_control_command_identity(
        &request.identity,
        &request.workspace_id,
        &request.event,
        &request.at,
    )?;
    validate_control_request_body(
        &request.identity,
        &json!({
            "workspace_id": request.workspace_id,
            "control_lease_id": request.control_lease_id,
            "environment_id": request.environment_id,
            "task_id": request.task_id,
            "attempt_id": request.attempt_id,
            "agent_session_id": request.agent_session_id,
            "runtime_id": request.runtime_id,
            "runtime_incarnation_id": request.runtime_incarnation_id,
            "expected_version": request.expected_version,
            "expected_epoch": request.expected_epoch,
        }),
    )?;
    if !valid_control_digest(&request.fencing_token_digest) || request.issuer_key_version == 0 {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_FENCING_CREDENTIAL_INVALID".to_owned(),
        ));
    }
    let principal_id = request.identity.principal_id().as_str().to_owned();
    let request_digest =
        control_request_digest("environment.control_lease.takeover", &request.identity)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    validate_workspace_mutation_scope(&tx, &request.workspace_id, &principal_id)?;
    if let Some(replayed) = read_control_lease_receipt(
        &tx,
        &principal_id,
        request.identity.request_id(),
        &request_digest,
    )? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replayed);
    }
    validate_current_attempt_binding(
        &tx,
        &request.workspace_id,
        &request.environment_id,
        &request.task_id,
        &request.attempt_id,
        &request.agent_session_id,
        &request.runtime_id,
        &request.runtime_incarnation_id,
        &request.at,
    )?;
    let mut current = load_control_lease(&tx, &request.control_lease_id)?;
    validate_control_lease_command_binding(
        &tx,
        &current,
        &request.workspace_id,
        &request.environment_id,
        &request.task_id,
        &request.attempt_id,
        &request.agent_session_id,
        &request.runtime_id,
        &request.runtime_incarnation_id,
    )?;
    if current.version != request.expected_version || current.epoch != request.expected_epoch {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_version),
            actual: Some(current.version),
        });
    }
    if current.state != EnvironmentControlLeaseState::Active
        || !matches!(current.owner, EnvironmentControlOwner::Agent { .. })
        || canonicalize_utc_timestamp(&current.expires_at)?
            <= canonicalize_utc_timestamp(&request.at)?
    {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_TAKEOVER_UNAVAILABLE".to_owned(),
        ));
    }
    ensure_no_unsettled_control_invocations(&tx, &request.attempt_id)?;
    let next_epoch = current
        .epoch
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("CONTROL_LEASE_EPOCH_EXHAUSTED".to_owned()))?;
    let next_version = current
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("CONTROL_LEASE_VERSION_EXHAUSTED".to_owned()))?;
    let old_owner = current.owner.clone();
    let next_owner = EnvironmentControlOwner::Human {
        principal_id: principal_id.clone(),
        principal_kind: storage_core::HumanPrincipalKind::User,
    };
    tx.execute(
        "UPDATE environment_control_leases
         SET owner_kind='HUMAN',owner_ref_json=?1,epoch=?2,issuer_key_version=?3,
             fencing_token_digest=?4,version=?5
         WHERE control_lease_id=?6 AND state='ACTIVE' AND epoch=?7 AND version=?8",
        params![
            json_text(&next_owner.reference_json()?)?,
            to_sql_i64(next_epoch, "control lease epoch")?,
            i64::from(request.issuer_key_version),
            request.fencing_token_digest,
            to_sql_i64(next_version, "control lease version")?,
            request.control_lease_id,
            to_sql_i64(request.expected_epoch, "expected control lease epoch")?,
            to_sql_i64(request.expected_version, "expected control lease version")?,
        ],
    )
    .map_err(map_database_error)?;
    if tx.changes() != 1 {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_version),
            actual: Some(current.version),
        });
    }
    current.owner = next_owner;
    current.epoch = next_epoch;
    current.issuer_key_version = request.issuer_key_version;
    current.fencing_token_digest = request.fencing_token_digest;
    current.version = next_version;
    validate_control_lease_event(&request.event, &current, Some(&old_owner), Some("ACTIVE"))?;
    let state_ref = put_control_lease_state(blobs, &request.workspace_id, &current)?;
    insert_domain_event(&tx, &request.event, &state_ref)?;
    let committed = CommittedEnvironmentControlLease {
        lease: current,
        replayed: false,
    };
    save_control_lease_receipt(
        &tx,
        &principal_id,
        request.identity.request_id(),
        &request_digest,
        &committed,
        &request.at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn end_control_lease(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    request: EnvironmentControlLeaseEnd,
) -> Result<CommittedEnvironmentControlLease, StoreError> {
    validate_control_command_identity(
        &request.identity,
        &request.workspace_id,
        &request.event,
        &request.at,
    )?;
    validate_control_request_body(
        &request.identity,
        &json!({
            "workspace_id": request.workspace_id,
            "control_lease_id": request.control_lease_id,
            "environment_id": request.environment_id,
            "task_id": request.task_id,
            "attempt_id": request.attempt_id,
            "agent_session_id": request.agent_session_id,
            "runtime_id": request.runtime_id,
            "runtime_incarnation_id": request.runtime_incarnation_id,
            "expected_version": request.expected_version,
            "expected_epoch": request.expected_epoch,
            "end_state": request.end_state,
        }),
    )?;
    let principal_id = request.identity.principal_id().as_str().to_owned();
    let request_digest =
        control_request_digest("environment.control_lease.end", &request.identity)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    validate_workspace_mutation_scope(&tx, &request.workspace_id, &principal_id)?;
    if let Some(replayed) = read_control_lease_receipt(
        &tx,
        &principal_id,
        request.identity.request_id(),
        &request_digest,
    )? {
        tx.commit().map_err(map_database_error)?;
        return Ok(replayed);
    }
    let mut current = load_control_lease(&tx, &request.control_lease_id)?;
    validate_control_lease_command_binding(
        &tx,
        &current,
        &request.workspace_id,
        &request.environment_id,
        &request.task_id,
        &request.attempt_id,
        &request.agent_session_id,
        &request.runtime_id,
        &request.runtime_incarnation_id,
    )?;
    if current.version != request.expected_version || current.epoch != request.expected_epoch {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_version),
            actual: Some(current.version),
        });
    }
    if current.state != EnvironmentControlLeaseState::Active {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_ALREADY_ENDED".to_owned(),
        ));
    }
    let old_owner = current.owner.clone();
    let end_state = match request.end_state {
        EnvironmentControlLeaseEndState::Released => EnvironmentControlLeaseState::Released,
        EnvironmentControlLeaseEndState::Revoked => EnvironmentControlLeaseState::Revoked,
    };
    let next_version = current
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("CONTROL_LEASE_VERSION_EXHAUSTED".to_owned()))?;
    tx.execute(
        "UPDATE environment_control_leases SET state=?1,version=?2
         WHERE control_lease_id=?3 AND state='ACTIVE' AND epoch=?4 AND version=?5",
        params![
            control_lease_state_text(end_state),
            to_sql_i64(next_version, "control lease version")?,
            request.control_lease_id,
            to_sql_i64(request.expected_epoch, "expected control lease epoch")?,
            to_sql_i64(request.expected_version, "expected control lease version")?,
        ],
    )
    .map_err(map_database_error)?;
    if tx.changes() != 1 {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_version),
            actual: Some(current.version),
        });
    }
    current.state = end_state;
    current.version = next_version;
    validate_control_lease_event(&request.event, &current, Some(&old_owner), Some("ACTIVE"))?;
    let state_ref = put_control_lease_state(blobs, &request.workspace_id, &current)?;
    insert_domain_event(&tx, &request.event, &state_ref)?;
    let committed = CommittedEnvironmentControlLease {
        lease: current,
        replayed: false,
    };
    save_control_lease_receipt(
        &tx,
        &principal_id,
        request.identity.request_id(),
        &request_digest,
        &committed,
        &request.at,
    )?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn validate_control_command_identity(
    identity: &EnvironmentRequestIdentity,
    workspace_id: &str,
    event: &EventDraft,
    at: &str,
) -> Result<(), StoreError> {
    validate_request_digest(identity)?;
    if workspace_id.trim().is_empty()
        || identity.principal_id().as_str().trim().is_empty()
        || canonicalize_utc_timestamp(at)? != at
        || event.schema_version != 1
        || event.event_type != "environment.control.lease.changed.v1"
        || event.workspace_id != workspace_id
        || event.entity_type != "EnvironmentControlLease"
        || event.entity_id.trim().is_empty()
        || event.origin_runtime_id.trim().is_empty()
        || canonicalize_utc_timestamp(&event.recorded_at)? != event.recorded_at
        || event.recorded_at != at
    {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_COMMAND_IDENTITY_INVALID".to_owned(),
        ));
    }
    Ok(())
}

fn control_request_digest(
    operation: &str,
    identity: &EnvironmentRequestIdentity,
) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&json!({
        "operation": operation,
        "request_digest": identity.canonical_request_digest(),
    }))?))
}

fn validate_control_request_body(
    identity: &EnvironmentRequestIdentity,
    expected: &Value,
) -> Result<(), StoreError> {
    let actual: Value = serde_json::from_slice(identity.canonical_request_body())
        .map_err(|_| StoreError::Invalid("CONTROL_LEASE_REQUEST_BODY_INVALID".to_owned()))?;
    if canonical_json(&actual)? != canonical_json(expected)? {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_REQUEST_BODY_MISMATCH".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_no_unsettled_control_invocations(
    connection: &Connection,
    attempt_id: &str,
) -> Result<(), StoreError> {
    let unsettled: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM capability_invocations
             WHERE attempt_id=?1 AND status NOT IN ('SUCCEEDED','FAILED','CANCELLED')",
            [attempt_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if unsettled != 0 {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_ACTIONS_NOT_DRAINED".to_owned(),
        ));
    }
    Ok(())
}

fn read_control_lease_receipt(
    connection: &Connection,
    principal_id: &str,
    request_id: &str,
    expected_digest: &str,
) -> Result<Option<CommittedEnvironmentControlLease>, StoreError> {
    let receipt: Option<(String, Option<String>, Option<String>)> = connection
        .query_row(
            "SELECT request_digest,response_json,response_digest FROM request_dedup
             WHERE principal_id=?1 AND request_id=?2",
            params![principal_id, request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((request_digest, response, response_digest)) = receipt else {
        return Ok(None);
    };
    if request_digest != expected_digest {
        return Err(StoreError::IdempotencyConflict);
    }
    let response = response.ok_or_else(|| {
        StoreError::Integrity("Environment control receipt has no response".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "Environment control receipt digest mismatch".to_owned(),
        ));
    }
    let decoded: ControlLeaseReceipt = serde_json::from_str(&response).map_err(|error| {
        StoreError::Integrity(format!("Environment control receipt is malformed: {error}"))
    })?;
    validate_control_lease_record(&decoded.lease).map_err(|error| {
        StoreError::Integrity(format!("Environment control receipt is invalid: {error}"))
    })?;
    Ok(Some(CommittedEnvironmentControlLease {
        lease: decoded.lease,
        replayed: true,
    }))
}

fn save_control_lease_receipt(
    tx: &Transaction<'_>,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
    committed: &CommittedEnvironmentControlLease,
    at: &str,
) -> Result<(), StoreError> {
    let response = String::from_utf8(canonical_json(&ControlLeaseReceipt {
        lease: committed.lease.clone(),
    })?)
    .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at)
         VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![principal_id, request_id, request_digest, response, digest(response.as_bytes()), at],
    )
    .map_err(map_database_error)?;
    Ok(())
}

fn validate_current_attempt_binding(
    connection: &Connection,
    workspace_id: &str,
    environment_id: &str,
    task_id: &str,
    attempt_id: &str,
    agent_session_id: &str,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    now: &str,
) -> Result<(), StoreError> {
    if [
        workspace_id,
        environment_id,
        task_id,
        attempt_id,
        agent_session_id,
        runtime_id,
        runtime_incarnation_id,
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_BINDING_INVALID".to_owned(),
        ));
    }
    let valid: Option<(String, String, String, String, String, String, String, bool)> = connection
        .query_row(
            "SELECT a.status,s.status,e.status,r.current_incarnation_id,r.availability,
                    i.recovery_state,t.status,
                    EXISTS (
                      SELECT 1 FROM environment_provider_bindings pb
                      WHERE pb.environment_id=e.environment_id
                        AND pb.runtime_id=r.runtime_id
                        AND pb.runtime_incarnation_id=r.current_incarnation_id
                        AND (pb.expires_at IS NULL OR julianday(pb.expires_at)>julianday(?8))
                    )
             FROM attempts a
             JOIN tasks t ON t.task_id=a.task_id
             JOIN environments e ON e.environment_id=a.environment_id
             JOIN runtimes r ON r.runtime_id=a.runtime_id
             JOIN runtime_incarnations i ON i.runtime_id=r.runtime_id
                AND i.runtime_incarnation_id=r.current_incarnation_id
             JOIN agent_sessions s ON s.agent_session_id=a.agent_session_id
             WHERE t.workspace_id=?1 AND e.environment_id=?2 AND a.task_id=?3
               AND a.attempt_id=?4 AND a.agent_session_id=?5 AND a.runtime_id=?6
               AND a.runtime_incarnation_id=?7 AND a.environment_id=?2
               AND s.workspace_id=?1 AND s.scope_kind='ATTEMPT_EXECUTION'
               AND s.task_id=a.task_id AND s.attempt_id=a.attempt_id
               AND s.agent_binding_id=a.agent_binding_id
               AND s.runtime_id=a.runtime_id AND s.runtime_incarnation_id=a.runtime_incarnation_id
               AND e.owner_workspace_id=?1 AND (e.owner_task_id IS NULL OR e.owner_task_id=a.task_id)
               AND (e.sharing_scope <> 'ATTEMPT_PRIVATE' OR e.owner_attempt_id=a.attempt_id)
               AND EXISTS (
                 SELECT 1 FROM runtime_workspace_bindings b
                 WHERE b.runtime_id=r.runtime_id AND b.workspace_id=?1 AND b.status='ACTIVE'
                   AND b.revoked_at IS NULL
                   AND EXISTS (SELECT 1 FROM json_each(b.roles_json) role WHERE role.value='EXECUTOR')
               )",
            params![workspace_id, environment_id, task_id, attempt_id, agent_session_id, runtime_id, runtime_incarnation_id, now],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        attempt_status,
        session_status,
        environment_status,
        current_incarnation,
        availability,
        recovery,
        task_status,
        provider_binding_current,
    )) = valid
    else {
        return Err(StoreError::NotFound);
    };
    let runtime_usable = current_incarnation == runtime_incarnation_id
        && matches!(
            (availability.as_str(), recovery.as_str()),
            ("ONLINE", "READY") | ("DEGRADED", "DEGRADED")
        );
    // Browser/desktop input is admitted only for a live execution, never while an
    // Attempt is merely preparing, waiting, checkpointing, or cancelling.
    let attempt_active = attempt_status == "RUNNING";
    let task_active = task_status == "RUNNING";
    let session_active = session_status == "ACTIVE";
    let environment_usable = matches!(environment_status.as_str(), "READY" | "BUSY");
    if !runtime_usable
        || !provider_binding_current
        || !attempt_active
        || !task_active
        || !session_active
        || !environment_usable
    {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_ATTEMPT_NOT_CURRENT".to_owned(),
        ));
    }
    if canonicalize_utc_timestamp(now)? != now {
        return Err(StoreError::Invalid("CONTROL_LEASE_TIME_INVALID".to_owned()));
    }
    Ok(())
}

fn validate_control_lease_command_binding(
    connection: &Connection,
    lease: &EnvironmentControlLeaseRecord,
    workspace_id: &str,
    environment_id: &str,
    task_id: &str,
    attempt_id: &str,
    agent_session_id: &str,
    runtime_id: &str,
    runtime_incarnation_id: &str,
) -> Result<(), StoreError> {
    if lease.environment_id != environment_id
        || lease.task_id != task_id
        || lease.attempt_id != attempt_id
        || lease.agent_session_id != agent_session_id
        || lease.runtime_id != runtime_id
        || lease.runtime_incarnation_id != runtime_incarnation_id
    {
        return Err(StoreError::NotFound);
    }
    let workspace: Option<String> = connection
        .query_row(
            "SELECT owner_workspace_id FROM environments WHERE environment_id=?1",
            [&lease.environment_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    if workspace.as_deref() != Some(workspace_id) {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

fn load_control_lease(
    connection: &Connection,
    control_lease_id: &str,
) -> Result<EnvironmentControlLeaseRecord, StoreError> {
    let row: Option<(String,String,String,String,String,String,String,String,i64,i64,String,String,String,i64,String)> = connection.query_row(
        "SELECT c.control_lease_id,c.environment_id,c.task_id,c.attempt_id,a.agent_session_id,
                c.runtime_id,c.runtime_incarnation_id,c.owner_kind,c.epoch,c.issuer_key_version,
                c.fencing_token_digest,c.state,c.acquired_at,c.version,c.expires_at
         FROM environment_control_leases c JOIN attempts a ON a.task_id=c.task_id AND a.attempt_id=c.attempt_id
         WHERE c.control_lease_id=?1",
        [control_lease_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?,row.get(12)?,row.get(13)?,row.get(14)?)),
    ).optional().map_err(map_database_error)?;
    let Some((
        control_lease_id,
        environment_id,
        task_id,
        attempt_id,
        agent_session_id,
        runtime_id,
        runtime_incarnation_id,
        owner_kind,
        epoch,
        issuer_key_version,
        fencing_token_digest,
        state,
        acquired_at,
        version,
        expires_at,
    )) = row
    else {
        return Err(StoreError::NotFound);
    };
    let owner_ref: String = connection
        .query_row(
            "SELECT owner_ref_json FROM environment_control_leases WHERE control_lease_id=?1",
            [control_lease_id.as_str()],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let owner_value: Value = serde_json::from_str(&owner_ref).map_err(|error| {
        StoreError::Integrity(format!(
            "control lease owner reference is malformed: {error}"
        ))
    })?;
    let owner = match owner_kind.as_str() {
        "AGENT" => EnvironmentControlOwner::Agent {
            agent_session_id: owner_value
                .as_str()
                .ok_or_else(|| {
                    StoreError::Integrity(
                        "AGENT control lease owner is not a session string".to_owned(),
                    )
                })?
                .to_owned(),
        },
        "HUMAN" => {
            let principal_id = owner_value
                .get("principal_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StoreError::Integrity("HUMAN control lease owner lacks principal_id".to_owned())
                })?
                .to_owned();
            let principal_kind =
                owner_value
                    .get("kind")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        StoreError::Integrity("HUMAN control lease owner lacks kind".to_owned())
                    })?;
            let principal_kind = serde_json::from_value::<storage_core::HumanPrincipalKind>(
                Value::String(principal_kind.to_owned()),
            )
            .map_err(|error| {
                StoreError::Integrity(format!(
                    "HUMAN control lease owner kind is invalid: {error}"
                ))
            })?;
            EnvironmentControlOwner::Human {
                principal_id,
                principal_kind,
            }
        }
        _ => {
            return Err(StoreError::Integrity(
                "control lease owner kind is invalid".to_owned(),
            ));
        }
    };
    let record = EnvironmentControlLeaseRecord {
        control_lease_id,
        environment_id,
        task_id,
        attempt_id,
        agent_session_id,
        runtime_id,
        runtime_incarnation_id,
        owner,
        epoch: from_sql_i64(epoch, "control lease epoch")?,
        issuer_key_version: u32::try_from(issuer_key_version).map_err(|_| {
            StoreError::Integrity("control lease key version is out of range".to_owned())
        })?,
        fencing_token_digest,
        state: serde_json::from_value(Value::String(state)).map_err(|error| {
            StoreError::Integrity(format!("control lease state is invalid: {error}"))
        })?,
        acquired_at,
        expires_at,
        version: from_sql_i64(version, "control lease version")?,
    };
    validate_control_lease_record(&record).map_err(integrity_error)?;
    Ok(record)
}

fn validate_control_lease_record(lease: &EnvironmentControlLeaseRecord) -> Result<(), StoreError> {
    if [
        lease.control_lease_id.as_str(),
        lease.environment_id.as_str(),
        lease.task_id.as_str(),
        lease.attempt_id.as_str(),
        lease.agent_session_id.as_str(),
        lease.runtime_id.as_str(),
        lease.runtime_incarnation_id.as_str(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
        || lease.epoch == 0
        || lease.version == 0
        || lease.issuer_key_version == 0
        || !valid_control_digest(&lease.fencing_token_digest)
        || canonicalize_utc_timestamp(&lease.acquired_at)? != lease.acquired_at
        || canonicalize_utc_timestamp(&lease.expires_at)? != lease.expires_at
        || canonicalize_utc_timestamp(&lease.expires_at)?
            <= canonicalize_utc_timestamp(&lease.acquired_at)?
    {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_RECORD_INVALID".to_owned(),
        ));
    }
    let owner_ref = lease.owner.reference_json()?;
    match &lease.owner {
        EnvironmentControlOwner::Agent { agent_session_id }
            if agent_session_id != &lease.agent_session_id =>
        {
            return Err(StoreError::Invalid(
                "CONTROL_LEASE_AGENT_OWNER_MISMATCH".to_owned(),
            ));
        }
        EnvironmentControlOwner::Human { .. } => {
            let _ = owner_ref;
        }
        _ => {
            let _ = owner_ref;
        }
    }
    Ok(())
}

fn validate_control_lease_event(
    event: &EventDraft,
    lease: &EnvironmentControlLeaseRecord,
    previous_owner: Option<&EnvironmentControlOwner>,
    previous_state: Option<&str>,
) -> Result<(), StoreError> {
    let mut payload = json!({
        "control_lease_id": lease.control_lease_id,
        "environment_id": lease.environment_id,
        "task_id": lease.task_id,
        "attempt_id": lease.attempt_id,
        "owner_kind": lease.owner.kind(),
        "owner_ref": lease.owner.reference_json()?,
        "epoch": lease.epoch.to_string(),
        "to": control_lease_state_text(lease.state),
        "aggregate_version": lease.version,
        "runtime_id": lease.runtime_id,
        "runtime_incarnation_id": lease.runtime_incarnation_id,
    });
    if let Some(owner) = previous_owner {
        payload["from_owner_kind"] = json!(owner.kind());
        payload["to_owner_kind"] = json!(lease.owner.kind());
    }
    if let Some(state) = previous_state {
        payload["from"] = json!(state);
    }
    if event.schema_version != 1
        || event.event_type != "environment.control.lease.changed.v1"
        || event.entity_type != "EnvironmentControlLease"
        || event.entity_id != lease.control_lease_id
        || event.workspace_id.trim().is_empty()
        || event.origin_runtime_id != lease.runtime_id
        || event.entity_revision != lease.version
        || event.payload != payload
        || canonicalize_utc_timestamp(&event.recorded_at)? != event.recorded_at
    {
        return Err(StoreError::Invalid(
            "CONTROL_LEASE_EVENT_INVALID".to_owned(),
        ));
    }
    Ok(())
}

fn put_control_lease_state(
    blobs: &dyn BlobStore,
    workspace_id: &str,
    lease: &EnvironmentControlLeaseRecord,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(lease)?;
    let blob = blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &bytes,
        "application/vnd.litecowork.environment-control-lease+json",
    )?;
    if blob.size_bytes != bytes.len() as u64
        || blobs.get(workspace_id, BlobPurpose::AggregateState, &blob)? != bytes
    {
        return Err(StoreError::Integrity(
            "control lease aggregate state failed blob verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: lease.version,
        record_schema_version: 1,
    })
}

fn valid_control_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn control_lease_state_text(state: EnvironmentControlLeaseState) -> &'static str {
    match state {
        EnvironmentControlLeaseState::Active => "ACTIVE",
        EnvironmentControlLeaseState::Releasing => "RELEASING",
        EnvironmentControlLeaseState::Released => "RELEASED",
        EnvironmentControlLeaseState::Expired => "EXPIRED",
        EnvironmentControlLeaseState::Revoked => "REVOKED",
    }
}

#[cfg(test)]
mod environment_control_lease_binding_tests {
    use super::*;

    const NOW: &str = "2026-10-17T12:00:00.000000000Z";

    fn binding_fixture(
        attempt_status: &str,
        task_status: &str,
        provider_expires_at: &str,
    ) -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE tasks(task_id TEXT, workspace_id TEXT, status TEXT);
             CREATE TABLE environments(environment_id TEXT, owner_workspace_id TEXT,
               owner_task_id TEXT, sharing_scope TEXT, owner_attempt_id TEXT, status TEXT);
             CREATE TABLE runtimes(runtime_id TEXT, current_incarnation_id TEXT, availability TEXT);
             CREATE TABLE runtime_incarnations(runtime_id TEXT, runtime_incarnation_id TEXT,
               recovery_state TEXT);
             CREATE TABLE attempts(attempt_id TEXT, task_id TEXT, agent_session_id TEXT,
               agent_binding_id TEXT, runtime_id TEXT, runtime_incarnation_id TEXT,
               environment_id TEXT, status TEXT);
             CREATE TABLE agent_sessions(agent_session_id TEXT, workspace_id TEXT, scope_kind TEXT,
               task_id TEXT, attempt_id TEXT, agent_binding_id TEXT, runtime_id TEXT,
               runtime_incarnation_id TEXT, status TEXT);
             CREATE TABLE runtime_workspace_bindings(runtime_id TEXT, workspace_id TEXT,
               status TEXT, revoked_at TEXT, roles_json TEXT);
             CREATE TABLE environment_provider_bindings(environment_id TEXT, runtime_id TEXT,
               runtime_incarnation_id TEXT, expires_at TEXT);
             INSERT INTO tasks VALUES('task-1','workspace-1','RUNNING');
             INSERT INTO environments VALUES('environment-1','workspace-1','task-1',
               'TASK_SHARED',NULL,'READY');
             INSERT INTO runtimes VALUES('runtime-1','incarnation-1','ONLINE');
             INSERT INTO runtime_incarnations VALUES('runtime-1','incarnation-1','READY');
             INSERT INTO attempts VALUES('attempt-1','task-1','session-1','binding-1',
               'runtime-1','incarnation-1','environment-1','RUNNING');
             INSERT INTO agent_sessions VALUES('session-1','workspace-1','ATTEMPT_EXECUTION',
               'task-1','attempt-1','binding-1','runtime-1','incarnation-1','ACTIVE');
             INSERT INTO runtime_workspace_bindings VALUES('runtime-1','workspace-1',
               'ACTIVE',NULL,'[\"EXECUTOR\"]');
             INSERT INTO environment_provider_bindings VALUES('environment-1','runtime-1',
               'incarnation-1','2030-01-01T00:00:00.000000000Z');",
            )
            .unwrap();
        connection
            .execute("UPDATE attempts SET status=?1", params![attempt_status])
            .unwrap();
        connection
            .execute("UPDATE tasks SET status=?1", params![task_status])
            .unwrap();
        connection
            .execute(
                "UPDATE environment_provider_bindings SET expires_at=?1",
                params![provider_expires_at],
            )
            .unwrap();
        connection
    }

    fn validate(connection: &Connection) -> Result<(), StoreError> {
        validate_current_attempt_binding(
            connection,
            "workspace-1",
            "environment-1",
            "task-1",
            "attempt-1",
            "session-1",
            "runtime-1",
            "incarnation-1",
            NOW,
        )
    }

    #[test]
    fn accepts_only_running_attempt_and_task_with_current_live_provider_binding() {
        let current = binding_fixture("RUNNING", "RUNNING", "2026-10-18T12:00:00.000000000Z");
        assert!(validate(&current).is_ok());

        let preparing = binding_fixture("PREPARING", "RUNNING", "2026-10-18T12:00:00.000000000Z");
        assert!(
            matches!(validate(&preparing), Err(StoreError::Invalid(message)) if message == "CONTROL_LEASE_ATTEMPT_NOT_CURRENT")
        );

        let waiting = binding_fixture("RUNNING", "WAITING_USER", "2026-10-18T12:00:00.000000000Z");
        assert!(
            matches!(validate(&waiting), Err(StoreError::Invalid(message)) if message == "CONTROL_LEASE_ATTEMPT_NOT_CURRENT")
        );

        let expired_provider =
            binding_fixture("RUNNING", "RUNNING", "2026-10-16T12:00:00.000000000Z");
        assert!(
            matches!(validate(&expired_provider), Err(StoreError::Invalid(message)) if message == "CONTROL_LEASE_ATTEMPT_NOT_CURRENT")
        );
    }

    #[derive(Clone)]
    struct ControlLeaseTestKeys;

    impl WorkspaceBlobKeyProvider for ControlLeaseTestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: zeroize::Zeroizing::new([37_u8; 32]),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob("unknown test key version".to_owned()));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    fn transaction_fixture() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE workspaces(workspace_id TEXT, owner_principal_id TEXT, status TEXT);
                 CREATE TABLE tasks(task_id TEXT, workspace_id TEXT, status TEXT);
                 CREATE TABLE environments(environment_id TEXT, owner_workspace_id TEXT,
                   owner_task_id TEXT, sharing_scope TEXT, owner_attempt_id TEXT, status TEXT);
                 CREATE TABLE runtimes(runtime_id TEXT, current_incarnation_id TEXT, availability TEXT);
                 CREATE TABLE runtime_incarnations(runtime_id TEXT, runtime_incarnation_id TEXT,
                   recovery_state TEXT);
                 CREATE TABLE attempts(attempt_id TEXT, task_id TEXT, agent_session_id TEXT,
                   agent_binding_id TEXT, runtime_id TEXT, runtime_incarnation_id TEXT,
                   environment_id TEXT, status TEXT);
                 CREATE TABLE agent_sessions(agent_session_id TEXT, workspace_id TEXT, scope_kind TEXT,
                   task_id TEXT, attempt_id TEXT, agent_binding_id TEXT, runtime_id TEXT,
                   runtime_incarnation_id TEXT, status TEXT);
                 CREATE TABLE runtime_workspace_bindings(runtime_id TEXT, workspace_id TEXT,
                   status TEXT, revoked_at TEXT, roles_json TEXT);
                 CREATE TABLE environment_provider_bindings(environment_id TEXT, runtime_id TEXT,
                   runtime_incarnation_id TEXT, expires_at TEXT);
                 CREATE TABLE capability_invocations(attempt_id TEXT, status TEXT);
                 CREATE TABLE environment_control_leases(
                   control_lease_id TEXT PRIMARY KEY, environment_id TEXT, task_id TEXT,
                   attempt_id TEXT, runtime_id TEXT, runtime_incarnation_id TEXT,
                   owner_kind TEXT, owner_ref_json TEXT, epoch INTEGER, issuer_key_version INTEGER,
                   fencing_token_digest TEXT, state TEXT, acquired_at TEXT, expires_at TEXT,
                   version INTEGER);
                 CREATE TABLE request_dedup(principal_id TEXT, request_id TEXT, request_digest TEXT,
                   response_json TEXT, response_digest TEXT, created_at TEXT, expires_at TEXT,
                   PRIMARY KEY(principal_id,request_id));
                 CREATE TABLE workspace_origin_sequences(workspace_id TEXT, origin_runtime_id TEXT,
                   last_sequence INTEGER, PRIMARY KEY(workspace_id,origin_runtime_id));
                 CREATE TABLE domain_events(event_id TEXT PRIMARY KEY,workspace_id TEXT,entity_type TEXT,
                   entity_id TEXT,origin_runtime_id TEXT,origin_sequence INTEGER,entity_revision INTEGER,
                   hlc_timestamp TEXT,correlation_id TEXT,causation_id TEXT,schema_version INTEGER,
                   type TEXT,payload_json TEXT,aggregate_state_ref_json TEXT,recorded_at TEXT,
                   payload_digest TEXT);
                 INSERT INTO workspaces VALUES('workspace-1','owner-1','ACTIVE');
                 INSERT INTO tasks VALUES('task-1','workspace-1','RUNNING');
                 INSERT INTO environments VALUES('environment-1','workspace-1','task-1',
                   'TASK_SHARED',NULL,'READY');
                 INSERT INTO runtimes VALUES('runtime-1','incarnation-1','ONLINE');
                 INSERT INTO runtime_incarnations VALUES('runtime-1','incarnation-1','READY');
                 INSERT INTO attempts VALUES('attempt-1','task-1','session-1','binding-1',
                   'runtime-1','incarnation-1','environment-1','RUNNING');
                 INSERT INTO agent_sessions VALUES('session-1','workspace-1','ATTEMPT_EXECUTION',
                   'task-1','attempt-1','binding-1','runtime-1','incarnation-1','ACTIVE');
                 INSERT INTO runtime_workspace_bindings VALUES('runtime-1','workspace-1',
                   'ACTIVE',NULL,'[\"EXECUTOR\"]');
                 INSERT INTO environment_provider_bindings VALUES('environment-1','runtime-1',
                   'incarnation-1','2030-01-01T00:00:00.000000000Z');",
            )
            .unwrap();
        connection
    }

    fn identity(request_id: &str, body: Value) -> EnvironmentRequestIdentity {
        let body = canonical_json(&body).unwrap();
        EnvironmentRequestIdentity::new(
            storage_core::PrincipalId::new("owner-1").unwrap(),
            request_id,
            body.clone(),
            digest(&body),
        )
        .unwrap()
    }

    fn lease_event(event_id: &str, revision: u64, at: &str, payload: Value) -> EventDraft {
        EventDraft {
            event_id: event_id.to_owned(),
            workspace_id: "workspace-1".to_owned(),
            entity_type: "EnvironmentControlLease".to_owned(),
            entity_id: "control-lease-1".to_owned(),
            origin_runtime_id: "runtime-1".to_owned(),
            entity_revision: revision,
            hlc_timestamp: at.to_owned(),
            correlation_id: format!("corr-{event_id}"),
            causation_id: None,
            schema_version: 1,
            event_type: "environment.control.lease.changed.v1".to_owned(),
            payload,
            recorded_at: at.to_owned(),
        }
    }

    #[test]
    fn acquire_takeover_release_commit_events_and_exact_receipts_atomically() {
        let mut connection = transaction_fixture();
        let directory = tempfile::tempdir().unwrap();
        let blobs = FileBlobStore::new(directory.path().join("blobs"), ControlLeaseTestKeys);
        let raw_fence_credential = "never-persist-this-fence";
        let fence_digest = digest(raw_fence_credential.as_bytes());
        let acquired_at = NOW;
        let expires_at = "2026-10-17T13:00:00.000000000Z";
        let acquire_body = json!({
            "workspace_id":"workspace-1",
            "control_lease_id":"control-lease-1",
            "environment_id":"environment-1",
            "task_id":"task-1",
            "attempt_id":"attempt-1",
            "agent_session_id":"session-1",
            "runtime_id":"runtime-1",
            "runtime_incarnation_id":"incarnation-1",
            "acquired_at":acquired_at,
            "expires_at":expires_at,
        });
        let acquire_event = lease_event(
            "event-lease-acquire",
            1,
            acquired_at,
            json!({
                "control_lease_id":"control-lease-1",
                "environment_id":"environment-1",
                "task_id":"task-1",
                "attempt_id":"attempt-1",
                "owner_kind":"AGENT",
                "owner_ref":"session-1",
                "epoch":"1",
                "to":"ACTIVE",
                "aggregate_version":1,
                "runtime_id":"runtime-1",
                "runtime_incarnation_id":"incarnation-1",
            }),
        );
        let acquire = EnvironmentControlLeaseAcquire {
            workspace_id: "workspace-1".to_owned(),
            control_lease_id: "control-lease-1".to_owned(),
            environment_id: "environment-1".to_owned(),
            task_id: "task-1".to_owned(),
            attempt_id: "attempt-1".to_owned(),
            agent_session_id: "session-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            issuer_key_version: 1,
            fencing_token_digest: fence_digest.clone(),
            acquired_at: acquired_at.to_owned(),
            expires_at: expires_at.to_owned(),
            identity: identity("acquire-1", acquire_body.clone()),
            event: acquire_event,
        };

        let acquired = acquire_control_lease(&mut connection, &blobs, acquire.clone()).unwrap();
        assert!(!acquired.replayed);
        assert_eq!(acquired.lease.epoch, 1);
        assert!(acquired.lease.agent_session_id == "session-1");
        let replayed = acquire_control_lease(&mut connection, &blobs, acquire).unwrap();
        assert!(replayed.replayed);
        assert_eq!(replayed.lease, acquired.lease);

        let takeover_at = "2026-10-17T12:10:00.000000000Z";
        let takeover_body = json!({
            "workspace_id":"workspace-1",
            "control_lease_id":"control-lease-1",
            "environment_id":"environment-1",
            "task_id":"task-1",
            "attempt_id":"attempt-1",
            "agent_session_id":"session-1",
            "runtime_id":"runtime-1",
            "runtime_incarnation_id":"incarnation-1",
            "expected_version":1,
            "expected_epoch":1,
        });
        let takeover = EnvironmentControlLeaseTakeover {
            workspace_id: "workspace-1".to_owned(),
            control_lease_id: "control-lease-1".to_owned(),
            environment_id: "environment-1".to_owned(),
            task_id: "task-1".to_owned(),
            attempt_id: "attempt-1".to_owned(),
            agent_session_id: "session-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            expected_version: 1,
            expected_epoch: 1,
            issuer_key_version: 1,
            fencing_token_digest: digest(b"new-fence-credential"),
            at: takeover_at.to_owned(),
            identity: identity("takeover-1", takeover_body),
            event: lease_event(
                "event-lease-takeover",
                2,
                takeover_at,
                json!({
                    "control_lease_id":"control-lease-1",
                    "environment_id":"environment-1",
                    "task_id":"task-1",
                    "attempt_id":"attempt-1",
                    "owner_kind":"HUMAN",
                    "owner_ref":{"principal_id":"owner-1","kind":"USER"},
                    "epoch":"2",
                    "to":"ACTIVE",
                    "aggregate_version":2,
                    "runtime_id":"runtime-1",
                    "runtime_incarnation_id":"incarnation-1",
                    "from_owner_kind":"AGENT",
                    "to_owner_kind":"HUMAN",
                    "from":"ACTIVE",
                }),
            ),
        };
        let human = takeover_control_lease(&mut connection, &blobs, takeover.clone()).unwrap();
        assert!(matches!(
            human.lease.owner,
            EnvironmentControlOwner::Human { .. }
        ));
        assert_eq!(human.lease.epoch, 2);
        assert_eq!(human.lease.version, 2);
        assert_eq!(human.lease.agent_session_id, "session-1");

        let mut stale_takeover = takeover.clone();
        stale_takeover.identity = identity(
            "takeover-stale-version",
            json!({
                "workspace_id":"workspace-1",
                "control_lease_id":"control-lease-1",
                "environment_id":"environment-1",
                "task_id":"task-1",
                "attempt_id":"attempt-1",
                "agent_session_id":"session-1",
                "runtime_id":"runtime-1",
                "runtime_incarnation_id":"incarnation-1",
                "expected_version":1,
                "expected_epoch":1,
            }),
        );
        stale_takeover.event = lease_event(
            "event-lease-stale-takeover",
            2,
            takeover_at,
            json!({
                "control_lease_id":"control-lease-1",
                "environment_id":"environment-1",
                "task_id":"task-1",
                "attempt_id":"attempt-1",
                "owner_kind":"HUMAN",
                "owner_ref":{"principal_id":"owner-1","kind":"USER"},
                "epoch":"2",
                "to":"ACTIVE",
                "aggregate_version":2,
                "runtime_id":"runtime-1",
                "runtime_incarnation_id":"incarnation-1",
                "from_owner_kind":"AGENT",
                "to_owner_kind":"HUMAN",
                "from":"ACTIVE",
            }),
        );
        assert!(matches!(
            takeover_control_lease(&mut connection, &blobs, stale_takeover),
            Err(StoreError::Conflict {
                expected: Some(1),
                actual: Some(2)
            })
        ));
        assert_eq!(
            load_control_lease(&connection, "control-lease-1").unwrap(),
            human.lease
        );

        let replayed_takeover = takeover_control_lease(&mut connection, &blobs, takeover).unwrap();
        assert!(replayed_takeover.replayed);
        assert_eq!(replayed_takeover.lease, human.lease);

        let released_at = "2026-10-17T12:20:00.000000000Z";
        let release_body = json!({
            "workspace_id":"workspace-1",
            "control_lease_id":"control-lease-1",
            "environment_id":"environment-1",
            "task_id":"task-1",
            "attempt_id":"attempt-1",
            "agent_session_id":"session-1",
            "runtime_id":"runtime-1",
            "runtime_incarnation_id":"incarnation-1",
            "expected_version":2,
            "expected_epoch":2,
            "end_state":"RELEASED",
        });
        let released = end_control_lease(
            &mut connection,
            &blobs,
            EnvironmentControlLeaseEnd {
                workspace_id: "workspace-1".to_owned(),
                control_lease_id: "control-lease-1".to_owned(),
                environment_id: "environment-1".to_owned(),
                task_id: "task-1".to_owned(),
                attempt_id: "attempt-1".to_owned(),
                agent_session_id: "session-1".to_owned(),
                runtime_id: "runtime-1".to_owned(),
                runtime_incarnation_id: "incarnation-1".to_owned(),
                expected_version: 2,
                expected_epoch: 2,
                end_state: EnvironmentControlLeaseEndState::Released,
                at: released_at.to_owned(),
                identity: identity("release-1", release_body),
                event: lease_event(
                    "event-lease-release",
                    3,
                    released_at,
                    json!({
                        "control_lease_id":"control-lease-1",
                        "environment_id":"environment-1",
                        "task_id":"task-1",
                        "attempt_id":"attempt-1",
                        "owner_kind":"HUMAN",
                        "owner_ref":{"principal_id":"owner-1","kind":"USER"},
                        "epoch":"2",
                        "to":"RELEASED",
                        "aggregate_version":3,
                        "runtime_id":"runtime-1",
                        "runtime_incarnation_id":"incarnation-1",
                        "from_owner_kind":"HUMAN",
                        "to_owner_kind":"HUMAN",
                        "from":"ACTIVE",
                    }),
                ),
            },
        )
        .unwrap();
        assert_eq!(released.lease.state, EnvironmentControlLeaseState::Released);

        let event_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .unwrap();
        let receipt_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| row.get(0))
            .unwrap();
        let event_json: String = connection
            .query_row(
                "SELECT group_concat(payload_json, ' ') FROM domain_events",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let receipts_json: String = connection
            .query_row(
                "SELECT group_concat(response_json, ' ') FROM request_dedup",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((event_count, receipt_count), (3, 3));
        assert!(!event_json.contains(raw_fence_credential));
        assert!(!receipts_json.contains(raw_fence_credential));
        assert!(receipts_json.contains("session-1"));
    }
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
