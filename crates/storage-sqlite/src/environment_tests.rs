use super::*;
use super::{create_workspace, state_database, test_store};
use domain_environment::{
    EnvironmentLifecycle, LifecycleDecision, ProviderProvisionResult, ProviderReadinessProof,
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Barrier},
    time::Duration,
};
use storage_core::{
    BudgetCeiling, BudgetEnforcement, BudgetEnforcementPolicy, EnvironmentBackupPolicy,
    EnvironmentClass, EnvironmentConfig, EnvironmentCreateRequest, EnvironmentHealth,
    EnvironmentIdentity, EnvironmentLifecycleRequest, EnvironmentLifetime, EnvironmentListRequest,
    EnvironmentOwner, EnvironmentRecord, EnvironmentRequestIdentity, EnvironmentSharingScope,
    EnvironmentSharingScopeChangeRequest, EnvironmentStatus, EnvironmentStore, EventDraft,
    FilesystemIsolation, IsolationSpec, NetworkMode, NetworkPolicy, ProcessIsolation,
    ProviderBindingCommit, ResourceLimits, ResourceScope, RuntimeId, RuntimeIncarnationId,
    StoreError, TaskId, WorkspaceId,
};

const AT: &str = "2026-10-09T12:00:00.000000000Z";
const PROVIDER_OBSERVED_AT: &str = "2026-10-09T08:00:00.000000000Z";

#[test]
fn v11_to_v13_migrations_are_ordered_atomic_and_retryable() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("state")).unwrap();
    let mut connection = Connection::open(state_database(&directory)).unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap();

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV11Migration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, V10_SCHEMA_VERSION);
    let v11_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 11",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(v11_receipts, 0);

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV12Migration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let rich_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rich_presentations'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let v12_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 12",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((version, rich_tables, v12_receipts), (11, 0, 0));

    let trigger_before_v13: Option<String> = connection.query_row(
        "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='environment_identity_immutable'",
        [], |row| row.get(0),
    ).optional().unwrap();
    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV13Migration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let trigger_sql: Option<String> = connection.query_row(
        "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='environment_identity_immutable'",
        [], |row| row.get(0),
    ).optional().unwrap();
    let v13_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 13",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 12);
    assert_eq!(trigger_sql, trigger_before_v13);
    assert!(
        trigger_sql
            .as_deref()
            .is_some_and(|sql| !sql.contains("sharing_scope"))
    );
    assert_eq!(v13_receipts, 0);

    migrate(&mut connection).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let migration_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version IN (11, 12, 13)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let rich_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rich_presentations'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((version, migration_receipts, rich_table), (13, 3, 1));
}

fn adapter(store: &SqliteWorkspaceStore) -> SqliteEnvironmentStore {
    SqliteEnvironmentStore::new(store.clone())
}

fn seed_environment_dependencies(store: &SqliteWorkspaceStore, directory: &tempfile::TempDir) {
    create_workspace(store, "workspace-env");
    let connection = Connection::open(state_database(directory)).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    tx.execute(
        "INSERT INTO runtimes(runtime_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,resource_capacity_json,last_seen,version)
         VALUES ('runtime-env','{}','test','linux','x86_64','[\"OPERATOR_ENDPOINT\"]','PERSONAL_DEVICE','ONLINE','MANUAL','inc-env','{}',?1,1)",
        [AT],
    ).unwrap();
    tx.execute(
        "INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state,ready_at,version)
         VALUES ('inc-env','runtime-env',?1,'test',0,'READY',?1,1)", [AT],
    ).unwrap();
    tx.execute(
        "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,version)
         VALUES ('rwb-env','runtime-env','workspace-env','LOCAL_ENROLLMENT','ACTIVE','[\"EXECUTOR\"]',?1,?1,1)",
        [AT],
    ).unwrap();
    tx.execute(
        "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at)
         VALUES ('profile-env','test','Test agent',?1)",
        [AT],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version)
         VALUES ('binding-env','workspace-env','profile-env','{}','{}',1,1,?1,1)", [AT],
    ).unwrap();
    tx.execute(
        "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version)
         VALUES ('task-env','workspace-env',1,'READY','binding-env','NORMAL','{}',?1,?1,1)", [AT],
    ).unwrap();
    tx.execute(
        "INSERT INTO task_spec_revisions(task_id,workspace_id,revision,objective,lead_failover_policy_json,placement_preference,authored_by_json,created_at)
         VALUES ('task-env','workspace-env',1,'Environment test','{\"mode\":\"DISABLED\",\"triggers\":[],\"fallback_agent_binding_ids\":[],\"max_lead_changes\":0}','\"AUTO\"','{}',?1)", [AT],
    ).unwrap();
    tx.commit().unwrap();
}

fn new_record() -> EnvironmentRecord {
    let identity = EnvironmentIdentity {
        environment_id: storage_core::EnvironmentId::new("environment-1").unwrap(),
        runtime_id: RuntimeId::new("runtime-env").unwrap(),
        owner_workspace_id: WorkspaceId::new("workspace-env").unwrap(),
        owner: EnvironmentOwner {
            task_id: Some(TaskId::new("task-env").unwrap()),
            attempt_id: None,
            coworker_id: None,
            principal_id: None,
        },
        created_by_incarnation_id: Some(RuntimeIncarnationId::new("inc-env").unwrap()),
    };
    let config = EnvironmentConfig {
        name: "Test environment".into(),
        provider_kind: "test-provider".into(),
        class: EnvironmentClass::Container,
        lifetime: EnvironmentLifetime::TaskRetained,
        sharing_scope: EnvironmentSharingScope::TaskShared,
        source_resources: Vec::new(),
        resource_limits: ResourceLimits {
            cpu_millis: 1000,
            memory_bytes: 1024 * 1024,
            storage_bytes: 1024 * 1024,
            max_processes: Some(16),
            max_lifetime_seconds: Some(3600),
        },
        network_policy: NetworkPolicy {
            mode: NetworkMode::None,
            allowed_domains: Vec::new(),
            max_response_bytes: 0,
            max_download_bytes: 0,
            deny_private_networks: true,
        },
        budget_ceiling: BudgetCeiling {
            max_wall_time_ms: Some(60_000),
            max_cost_minor_units: None,
            currency: None,
            max_tokens: None,
            max_child_attempts: Some(0),
            max_concurrency: Some(1),
        },
        budget_enforcement_policy: BudgetEnforcementPolicy::AllowHostMonitored,
        budget_enforcement: BudgetEnforcement::HostMonitored,
        provision_preview_digest: None,
        retention_expires_at: None,
        backup_policy: EnvironmentBackupPolicy::Excluded,
        isolation: IsolationSpec {
            filesystem: FilesystemIsolation::ContainerFs,
            process: ProcessIsolation::Container,
            network: NetworkMode::None,
            write_scope: ResourceScope {
                workspace_id: WorkspaceId::new("workspace-env").unwrap(),
                resource_ids: Vec::new(),
            },
        },
    };
    let initial = domain_environment::new_environment(identity, config, AT, AT).unwrap();
    match initial.request_provision(initial.expected_state()).unwrap() {
        LifecycleDecision::Applied(record) => record.with_updated_at(AT).unwrap(),
        LifecycleDecision::ReconciliationRequired(_) => panic!("provision request must apply"),
    }
}

fn request_identity(request_id: &str, body: Value) -> EnvironmentRequestIdentity {
    let body = canonical_json(&body).unwrap();
    EnvironmentRequestIdentity::new(
        storage_core::PrincipalId::new("owner-local").unwrap(),
        request_id,
        body.clone(),
        digest(&body),
    )
    .unwrap()
}

fn create_request(
    record: EnvironmentRecord,
    request_id: &str,
    event_id: &str,
) -> EnvironmentCreateRequest {
    EnvironmentCreateRequest::new(
        request_identity(request_id, json!({"environment_id": record.identity().environment_id.as_str(), "runtime_id":"runtime-env"})),
        record.clone(),
        EventDraft {
            event_id: event_id.into(), workspace_id: "workspace-env".into(),
            entity_type: "Environment".into(), entity_id: record.identity().environment_id.as_str().into(),
            origin_runtime_id: "runtime-env".into(), entity_revision: record.version(),
            hlc_timestamp: AT.into(), correlation_id: format!("corr-{event_id}"), causation_id: None,
            schema_version: 1, event_type: "environment.created.v1".into(),
            payload: json!({
                "environment_id": record.identity().environment_id.as_str(),
                "runtime_id": "runtime-env", "from": "NEW", "to": "PROVISIONING",
                "provider_kind": record.config().provider_kind,
                "lifetime": "TASK_RETAINED"
            }),
            recorded_at: AT.into(),
        },
    ).unwrap()
}

fn seed_persistent_coworker_environment(directory: &tempfile::TempDir) {
    let mut connection = Connection::open(state_database(directory)).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .unwrap();
    let tx = connection.transaction().unwrap();
    tx.execute(
        "INSERT INTO coworkers(coworker_id,workspace_id,current_revision,status,created_at,updated_at,version)
         VALUES ('coworker-env','workspace-env',1,'ACTIVE',?1,?1,1)",
        [AT],
    ).unwrap();
    tx.execute(
        "INSERT INTO coworker_revisions(coworker_id,revision,workspace_id,name,role_description,delegation_strategy,enabled_delegation_profile_ids_json,interaction_policy_json,context_policy_json,notification_policy_json,authored_by_json,created_at)
         VALUES ('coworker-env',1,'workspace-env','Env owner','Environment fixture','BALANCED','[]',
           '{\"read_only_work\":\"STANDARD_TRUST_POLICY\",\"draft_creation\":\"STANDARD_TRUST_POLICY\",\"external_mutation\":\"STANDARD_TRUST_POLICY\",\"destructive_action\":\"HANDOFF_TO_OWNER\",\"financial_commitment\":\"HANDOFF_TO_OWNER\"}',
           '{}','{}','{}',?1)",
        [AT],
    ).unwrap();
    tx.execute(
        "INSERT INTO environment_provision_previews(preview_digest,workspace_id,authenticated_principal_json,normalized_request_digest,eligibility_basis_json,expires_at,status,created_at)
         VALUES (?1,'workspace-env','{}',?2,'{}','2030-01-01T00:00:00Z','ISSUED',?3)",
        params![format!("sha256:{}", "c".repeat(64)), format!("sha256:{}", "d".repeat(64)), AT],
    ).unwrap();
    tx.execute(
        "INSERT INTO environments(environment_id,runtime_id,provider_kind,class,lifetime,owner_workspace_id,owner_coworker_id,sharing_scope,name,created_by_incarnation_id,status,health,budget_enforcement_policy,budget_enforcement,source_resources_json,resource_limits_json,network_policy_json,budget_ceiling_json,provision_preview_digest,backup_policy,isolation_json,created_at,updated_at,version)
         VALUES ('environment-persistent','runtime-env','test-provider','CONTAINER','WORKSPACE_PERSISTENT','workspace-env','coworker-env','COWORKER_PRIVATE','Persistent fixture','inc-env','SUSPENDED','HEALTHY','ALLOW_HOST_MONITORED','HOST_MONITORED','[]',
           '{\"cpu_millis\":1000,\"memory_bytes\":1048576,\"storage_bytes\":1048576,\"max_processes\":16,\"max_lifetime_seconds\":3600}',
           '{\"mode\":\"NONE\",\"allowed_domains\":[],\"max_response_bytes\":0,\"max_download_bytes\":0,\"deny_private_networks\":true}',
           '{\"max_wall_time_ms\":60000,\"max_cost_minor_units\":null,\"currency\":null,\"max_tokens\":null,\"max_child_attempts\":0,\"max_concurrency\":1}',
           ?1,'EXCLUDED','{\"filesystem\":\"CONTAINER_FS\",\"process\":\"CONTAINER\",\"network\":\"NONE\",\"write_scope\":{\"workspace_id\":\"workspace-env\",\"resource_ids\":[]}}',?2,?2,4)",
        params![format!("sha256:{}", "c".repeat(64)), AT],
    ).unwrap();
    tx.execute(
        "UPDATE environment_provision_previews SET status='CONSUMED',consumed_by_request_id='persistent-create',environment_id='environment-persistent' WHERE preview_digest=?1",
        [format!("sha256:{}", "c".repeat(64))],
    ).unwrap();
    tx.commit().unwrap();
}

fn sharing_scope_change_request(
    request_id: &str,
    event_id: &str,
    target_scope: EnvironmentSharingScope,
    target_coworker_id: Option<&str>,
    expected_version: u64,
) -> EnvironmentSharingScopeChangeRequest {
    let target_scope_text = match target_scope {
        EnvironmentSharingScope::CoworkerPrivate => "COWORKER_PRIVATE",
        EnvironmentSharingScope::WorkspaceShared => "WORKSPACE_SHARED",
        EnvironmentSharingScope::AttemptPrivate => "ATTEMPT_PRIVATE",
        EnvironmentSharingScope::TaskShared => "TASK_SHARED",
        EnvironmentSharingScope::UserShared => "USER_SHARED",
    };
    let environment_id = "environment-persistent";
    let body = json!({
        "environment_id": environment_id,
        "expected_version": expected_version,
        "target_sharing_scope": target_scope_text,
        "target_coworker_id": target_coworker_id,
    });
    EnvironmentSharingScopeChangeRequest::new(
        request_identity(request_id, body),
        storage_core::EnvironmentId::new(environment_id).unwrap(),
        WorkspaceId::new("workspace-env").unwrap(),
        expected_version,
        target_scope,
        target_coworker_id.map(|id| storage_core::CoworkerId::new(id).unwrap()),
        EventDraft {
            event_id: event_id.into(),
            workspace_id: "workspace-env".into(),
            entity_type: "Environment".into(),
            entity_id: environment_id.into(),
            origin_runtime_id: "runtime-env".into(),
            entity_revision: expected_version + 1,
            hlc_timestamp: AT.into(),
            correlation_id: format!("corr-{event_id}"),
            causation_id: None,
            schema_version: 1,
            event_type: "environment.sharing_scope.changed.v1".into(),
            payload: json!({
                "environment_id": environment_id,
                "from": "COWORKER_PRIVATE",
                "to": target_scope_text,
                "changed_by": {"kind":"USER","principal_id":"owner-local"},
                "aggregate_version": expected_version + 1,
            }),
            recorded_at: AT.into(),
        },
    )
    .unwrap()
}

fn seed_active_environment_control_lease(directory: &tempfile::TempDir) {
    let connection = Connection::open(state_database(directory)).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .unwrap();
    connection.execute(
        "INSERT INTO environment_control_leases(control_lease_id,environment_id,task_id,attempt_id,runtime_id,runtime_incarnation_id,owner_kind,owner_ref_json,epoch,issuer_key_version,fencing_token_digest,state,acquired_at,expires_at,version)
         VALUES ('lease-env-active','environment-persistent','task-env','fixture-attempt','runtime-env','inc-env','HUMAN','{}',1,1,?1,'ACTIVE',?2,'2030-01-01T00:00:00Z',1)",
        params![format!("sha256:{}", "e".repeat(64)), AT],
    ).unwrap();
}

fn seed_environment_checkpoint(directory: &tempfile::TempDir) {
    let connection = Connection::open(state_database(directory)).unwrap();
    connection
        .execute(
            "INSERT INTO environment_checkpoints(checkpoint_id,environment_id,digest,portable_snapshot_ref_json,created_at)
             VALUES ('checkpoint-env','environment-persistent',?1,NULL,?2)",
            params![format!("sha256:{}", "f".repeat(64)), AT],
        )
        .unwrap();
}

#[test]
fn suspended_persistent_environment_scope_change_commits_record_event_and_replay_together() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    seed_persistent_coworker_environment(&directory);
    let adapter = adapter(&store);

    let committed = adapter
        .change_environment_sharing_scope(sharing_scope_change_request(
            "scope-change-1",
            "scope-event-1",
            EnvironmentSharingScope::WorkspaceShared,
            None,
            4,
        ))
        .unwrap();
    assert!(!committed.replayed);
    assert_eq!(
        committed.record.config().sharing_scope,
        EnvironmentSharingScope::WorkspaceShared
    );
    assert_eq!(committed.record.identity().owner.coworker_id, None);
    assert_eq!(committed.record.status(), EnvironmentStatus::Suspended);
    assert_eq!(committed.record.version(), 5);

    let replay = adapter
        .change_environment_sharing_scope(sharing_scope_change_request(
            "scope-change-1",
            "scope-event-regenerated",
            EnvironmentSharingScope::WorkspaceShared,
            None,
            4,
        ))
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.record, committed.record);
    let connection = Connection::open(state_database(&directory)).unwrap();
    let (events, receipts): (i64, i64) = connection.query_row(
        "SELECT (SELECT COUNT(*) FROM domain_events WHERE type='environment.sharing_scope.changed.v1'), (SELECT COUNT(*) FROM request_dedup WHERE request_id='scope-change-1')",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert_eq!((events, receipts), (1, 1));
}

#[test]
fn sharing_scope_change_rejects_active_control_lease_without_partial_commit() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    seed_persistent_coworker_environment(&directory);
    seed_active_environment_control_lease(&directory);
    let adapter = adapter(&store);

    let error = adapter
        .change_environment_sharing_scope(sharing_scope_change_request(
            "scope-change-held",
            "scope-event-held",
            EnvironmentSharingScope::WorkspaceShared,
            None,
            4,
        ))
        .unwrap_err();
    assert!(
        matches!(error, StoreError::Invalid(message) if message.contains("SharingScopeChangeHeld"))
    );
    let record = adapter
        .get_environment("workspace-env", "environment-persistent")
        .unwrap()
        .unwrap();
    assert_eq!(
        record.config().sharing_scope,
        EnvironmentSharingScope::CoworkerPrivate
    );
    assert_eq!(
        record
            .identity()
            .owner
            .coworker_id
            .as_ref()
            .unwrap()
            .as_str(),
        "coworker-env"
    );
    assert_eq!(record.version(), 4);
    let connection = Connection::open(state_database(&directory)).unwrap();
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE type='environment.sharing_scope.changed.v1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id='scope-change-held'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((events, receipts), (0, 0));
}

#[test]
fn sharing_scope_change_checks_expected_environment_version() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    seed_persistent_coworker_environment(&directory);
    let adapter = adapter(&store);

    assert!(matches!(
        adapter.change_environment_sharing_scope(sharing_scope_change_request(
            "scope-change-stale",
            "scope-event-stale",
            EnvironmentSharingScope::WorkspaceShared,
            None,
            3,
        )),
        Err(StoreError::Conflict {
            expected: Some(3),
            actual: Some(4)
        })
    ));
}

#[test]
fn sharing_scope_change_fails_closed_when_checkpoint_hold_state_is_unknown() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    seed_persistent_coworker_environment(&directory);
    seed_environment_checkpoint(&directory);
    let adapter = adapter(&store);

    let error = adapter
        .change_environment_sharing_scope(sharing_scope_change_request(
            "scope-change-checkpoint",
            "scope-event-checkpoint",
            EnvironmentSharingScope::WorkspaceShared,
            None,
            4,
        ))
        .unwrap_err();
    assert!(
        matches!(error, StoreError::Invalid(message) if message.contains("checkpoint_holds: 1"))
    );
    let record = adapter
        .get_environment("workspace-env", "environment-persistent")
        .unwrap()
        .unwrap();
    assert_eq!(
        record.config().sharing_scope,
        EnvironmentSharingScope::CoworkerPrivate
    );
    assert_eq!(record.version(), 4);
}

fn ready_transition(
    current: &EnvironmentRecord,
    request_id: &str,
    event_id: &str,
    incarnation: &str,
    locator: &str,
) -> EnvironmentLifecycleRequest {
    let expected_incarnation = RuntimeIncarnationId::new("inc-env").unwrap();
    let ready = match current
        .apply_provision_result(
            current.expected_state(),
            &expected_incarnation,
            ProviderProvisionResult::Ready(ProviderReadinessProof {
                environment_id: current.identity().environment_id.clone(),
                runtime_id: current.identity().runtime_id.clone(),
                runtime_incarnation_id: expected_incarnation.clone(),
                provider_kind: current.config().provider_kind.clone(),
                source_resources: current.config().source_resources.clone(),
                isolation: current.config().isolation.clone(),
                health: EnvironmentHealth::Healthy,
                budget_enforcement: BudgetEnforcement::HostMonitored,
            }),
        )
        .unwrap()
    {
        LifecycleDecision::Applied(record) => record.with_updated_at(AT).unwrap(),
        LifecycleDecision::ReconciliationRequired(_) => panic!("matching proof must apply"),
    };
    let binding = ProviderBindingCommit::new(
        current.identity().environment_id.clone(),
        current.identity().runtime_id.clone(),
        RuntimeIncarnationId::new(incarnation).unwrap(),
        current.config().provider_kind.clone(),
        locator,
        PROVIDER_OBSERVED_AT,
        None,
    )
    .unwrap();
    let body =
        json!({"environment_id": current.identity().environment_id.as_str(), "action":"ready"});
    EnvironmentLifecycleRequest::new(
        request_identity(request_id, body),
        current.identity().environment_id.clone(),
        current.identity().owner_workspace_id.clone(),
        current.expected_state(),
        ready.clone(),
        Some(binding),
        EventDraft {
            event_id: event_id.into(), workspace_id: "workspace-env".into(), entity_type: "Environment".into(),
            entity_id: current.identity().environment_id.as_str().into(), origin_runtime_id: "runtime-env".into(),
            entity_revision: ready.version(), hlc_timestamp: AT.into(), correlation_id: format!("corr-{event_id}"),
            causation_id: None, schema_version: 1, event_type: "environment.state.changed.v1".into(),
            payload: json!({"environment_id": current.identity().environment_id.as_str(), "runtime_id":"runtime-env",
                "provider_kind": current.config().provider_kind, "from":"PROVISIONING", "to":"READY"}),
            recorded_at: AT.into(),
        },
    ).unwrap()
}

fn failed_transition(
    current: &EnvironmentRecord,
    request_id: &str,
    event_id: &str,
) -> EnvironmentLifecycleRequest {
    let failed = match current
        .apply_provision_result(
            current.expected_state(),
            &RuntimeIncarnationId::new("inc-env").unwrap(),
            ProviderProvisionResult::Failed,
        )
        .unwrap()
    {
        LifecycleDecision::Applied(record) => record.with_updated_at(AT).unwrap(),
        LifecycleDecision::ReconciliationRequired(_) => panic!("failed provisioning should settle"),
    };
    EnvironmentLifecycleRequest::new(
        request_identity(request_id, json!({"environment_id": current.identity().environment_id.as_str(), "action":"failed"})),
        current.identity().environment_id.clone(),
        current.identity().owner_workspace_id.clone(),
        current.expected_state(),
        failed.clone(),
        None,
        EventDraft {
            event_id: event_id.into(), workspace_id: "workspace-env".into(), entity_type: "Environment".into(),
            entity_id: current.identity().environment_id.as_str().into(), origin_runtime_id: "runtime-env".into(),
            entity_revision: failed.version(), hlc_timestamp: AT.into(), correlation_id: format!("corr-{event_id}"),
            causation_id: None, schema_version: 1, event_type: "environment.state.changed.v1".into(),
            payload: json!({"environment_id": current.identity().environment_id.as_str(), "runtime_id":"runtime-env",
                "provider_kind": current.config().provider_kind, "from":"PROVISIONING", "to":"FAILED"}),
            recorded_at: AT.into(),
        },
    ).unwrap()
}

fn busy_transition(current: &EnvironmentRecord) -> EnvironmentLifecycleRequest {
    let busy = current
        .with_domain_state(
            EnvironmentStatus::Busy,
            current.health(),
            current.version() + 1,
        )
        .unwrap()
        .with_updated_at(AT)
        .unwrap();
    EnvironmentLifecycleRequest::new(
        request_identity("env-busy", json!({"environment_id": current.identity().environment_id.as_str(), "action":"busy"})),
        current.identity().environment_id.clone(),
        current.identity().owner_workspace_id.clone(),
        current.expected_state(),
        busy.clone(),
        None,
        EventDraft {
            event_id: "event-env-busy".into(), workspace_id: "workspace-env".into(),
            entity_type: "Environment".into(), entity_id: current.identity().environment_id.as_str().into(),
            origin_runtime_id: "runtime-env".into(), entity_revision: busy.version(),
            hlc_timestamp: AT.into(), correlation_id: "corr-env-busy".into(), causation_id: None,
            schema_version: 1, event_type: "environment.state.changed.v1".into(),
            payload: json!({"environment_id": current.identity().environment_id.as_str(), "runtime_id":"runtime-env",
                "provider_kind": current.config().provider_kind, "from":"READY", "to":"BUSY"}),
            recorded_at: AT.into(),
        },
    ).unwrap()
}

fn ready_again_transition(current: &EnvironmentRecord) -> EnvironmentLifecycleRequest {
    let ready = current
        .with_domain_state(
            EnvironmentStatus::Ready,
            current.health(),
            current.version() + 1,
        )
        .unwrap()
        .with_updated_at(AT)
        .unwrap();
    let binding = ProviderBindingCommit::new(
        current.identity().environment_id.clone(),
        current.identity().runtime_id.clone(),
        RuntimeIncarnationId::new("inc-env").unwrap(),
        current.config().provider_kind.clone(),
        "opaque/refreshed-provider-locator",
        PROVIDER_OBSERVED_AT,
        None,
    )
    .unwrap();
    EnvironmentLifecycleRequest::new(
        request_identity("env-ready-again", json!({"environment_id": current.identity().environment_id.as_str(), "action":"ready"})),
        current.identity().environment_id.clone(),
        current.identity().owner_workspace_id.clone(),
        current.expected_state(),
        ready.clone(),
        Some(binding),
        EventDraft {
            event_id: "event-env-ready-again".into(), workspace_id: "workspace-env".into(),
            entity_type: "Environment".into(), entity_id: current.identity().environment_id.as_str().into(),
            origin_runtime_id: "runtime-env".into(), entity_revision: ready.version(),
            hlc_timestamp: AT.into(), correlation_id: "corr-env-ready-again".into(), causation_id: None,
            schema_version: 1, event_type: "environment.state.changed.v1".into(),
            payload: json!({"environment_id": current.identity().environment_id.as_str(), "runtime_id":"runtime-env",
                "provider_kind": current.config().provider_kind, "from":"BUSY", "to":"READY"}),
            recorded_at: AT.into(),
        },
    ).unwrap()
}

fn suspend_transition(current: &EnvironmentRecord) -> EnvironmentLifecycleRequest {
    let checkpointing = match current
        .request_suspend(current.expected_state(), Default::default())
        .unwrap()
    {
        LifecycleDecision::Applied(record) => record.with_updated_at(AT).unwrap(),
        LifecycleDecision::ReconciliationRequired(_) => panic!("suspend request must apply"),
    };
    EnvironmentLifecycleRequest::new(
        request_identity("env-suspend", json!({"environment_id": current.identity().environment_id.as_str(), "action":"suspend"})),
        current.identity().environment_id.clone(),
        current.identity().owner_workspace_id.clone(),
        current.expected_state(),
        checkpointing.clone(),
        None,
        EventDraft {
            event_id: "event-env-suspend".into(), workspace_id: "workspace-env".into(),
            entity_type: "Environment".into(), entity_id: current.identity().environment_id.as_str().into(),
            origin_runtime_id: "runtime-env".into(), entity_revision: checkpointing.version(),
            hlc_timestamp: AT.into(), correlation_id: "corr-env-suspend".into(), causation_id: None,
            schema_version: 1, event_type: "environment.state.changed.v1".into(),
            payload: json!({"environment_id": current.identity().environment_id.as_str(), "runtime_id":"runtime-env",
                "provider_kind": current.config().provider_kind, "from":"READY", "to":"CHECKPOINTING"}),
            recorded_at: AT.into(),
        },
    ).unwrap()
}

fn destroy_transition(current: &EnvironmentRecord) -> EnvironmentLifecycleRequest {
    let destroying = match current
        .request_destroy(current.expected_state(), Default::default(), true, true)
        .unwrap()
    {
        LifecycleDecision::Applied(record) => record.with_updated_at(AT).unwrap(),
        LifecycleDecision::ReconciliationRequired(_) => panic!("destroy request must apply"),
    };
    EnvironmentLifecycleRequest::new(
        request_identity("env-destroy", json!({"environment_id": current.identity().environment_id.as_str(), "action":"destroy"})),
        current.identity().environment_id.clone(),
        current.identity().owner_workspace_id.clone(),
        current.expected_state(),
        destroying.clone(),
        None,
        EventDraft {
            event_id: "event-env-destroy".into(), workspace_id: "workspace-env".into(),
            entity_type: "Environment".into(), entity_id: current.identity().environment_id.as_str().into(),
            origin_runtime_id: "runtime-env".into(), entity_revision: destroying.version(),
            hlc_timestamp: AT.into(), correlation_id: "corr-env-destroy".into(), causation_id: None,
            schema_version: 1, event_type: "environment.state.changed.v1".into(),
            payload: json!({"environment_id": current.identity().environment_id.as_str(), "runtime_id":"runtime-env",
                "provider_kind": current.config().provider_kind, "from":"READY", "to":"DESTROYING"}),
            recorded_at: AT.into(),
        },
    ).unwrap()
}

#[test]
fn create_get_list_and_exact_replay_commit_one_event_and_aggregate_blob() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let record = new_record();
    let request = create_request(record.clone(), "env-create", "event-env-create");
    let created = adapter.create_environment(request).unwrap();
    assert_eq!(created.record, record);
    assert!(!created.replayed);
    let replay = adapter
        .create_environment(create_request(
            record.clone(),
            "env-create",
            "event-env-create-regenerated",
        ))
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.record, record);
    assert_eq!(
        adapter
            .get_environment("workspace-env", "environment-1")
            .unwrap(),
        Some(record.clone())
    );
    assert_eq!(
        adapter
            .list_environments(
                EnvironmentListRequest::new(WorkspaceId::new("workspace-env").unwrap(), None, 10)
                    .unwrap()
            )
            .unwrap(),
        vec![record]
    );
    let connection = Connection::open(state_database(&directory)).unwrap();
    let (events, receipts): (i64, i64) = connection.query_row(
        "SELECT (SELECT COUNT(*) FROM domain_events WHERE entity_type='Environment'), (SELECT COUNT(*) FROM request_dedup WHERE request_id='env-create')",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert_eq!((events, receipts), (1, 1));
    let schema_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let rich_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rich_presentations'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((schema_version, rich_table), (13, 1));
    for (column, value) in [
        ("sharing_scope", "WORKSPACE_SHARED"),
        ("owner_coworker_id", "coworker-forbidden"),
        ("owner_principal_id", "principal-forbidden"),
    ] {
        let sql = format!("UPDATE environments SET {column} = ?1 WHERE environment_id = ?2");
        let error = connection
            .execute(&sql, params![value, "environment-1"])
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("ENVIRONMENT_SHARING_SCOPE_CHANGE_UNSAFE"),
            "{column} update should be rejected by the dedicated v4 sharing-scope guard; got {error}"
        );
    }
    let error = connection
        .execute(
            "UPDATE environments SET name = 'rewritten' WHERE environment_id = 'environment-1'",
            [],
        )
        .unwrap_err();
    assert!(error.to_string().contains("ENVIRONMENT_IDENTITY_IMMUTABLE"));
}

#[test]
fn create_replay_is_rejected_after_workspace_archival() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let request = create_request(
        new_record(),
        "env-create-archived-replay",
        "event-env-create-archived-replay",
    );
    let created = adapter.create_environment(request.clone()).unwrap();

    let connection = Connection::open(state_database(&directory)).unwrap();
    connection
        .execute(
            "UPDATE workspaces SET status = 'ARCHIVED' WHERE workspace_id = 'workspace-env'",
            [],
        )
        .unwrap();

    assert!(matches!(
        adapter.create_environment(request),
        Err(StoreError::Invalid(message)) if message == "WORKSPACE_ARCHIVED"
    ));
    assert_eq!(
        adapter
            .get_environment("workspace-env", "environment-1")
            .unwrap(),
        Some(created.record)
    );
    let (events, receipts): (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM domain_events WHERE entity_type='Environment'),
                    (SELECT COUNT(*) FROM request_dedup WHERE request_id='env-create-archived-replay')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((events, receipts), (1, 1));
}

#[test]
fn exact_create_replay_survives_runtime_restart_but_new_create_needs_current_incarnation() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let request = create_request(
        new_record(),
        "env-create-restart",
        "event-env-create-restart",
    );
    let committed = adapter.create_environment(request.clone()).unwrap();

    let connection = Connection::open(state_database(&directory)).unwrap();
    connection.execute(
        "INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state,ready_at,version)
         VALUES ('inc-env-2','runtime-env',?1,'test',0,'READY',?1,1)", ["2026-10-09T12:05:00.000000000Z"],
    ).unwrap();
    connection.execute(
        "UPDATE runtimes SET current_incarnation_id='inc-env-2',version=version+1 WHERE runtime_id='runtime-env'",
        [],
    ).unwrap();

    let replay = adapter.create_environment(request).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.record, committed.record);
    let new_request = create_request(
        new_record(),
        "env-create-old-incarnation",
        "event-env-create-old-incarnation",
    );
    assert!(matches!(
        adapter.create_environment(new_request),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn transition_cas_and_provider_binding_are_atomic_and_locator_stays_runtime_local() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let created = adapter
        .create_environment(create_request(
            new_record(),
            "env-create",
            "event-env-create",
        ))
        .unwrap();

    let stale = ready_transition(
        &created.record,
        "env-stale",
        "event-env-stale",
        "old-incarnation",
        "opaque/stale",
    );
    assert!(matches!(
        adapter.transition_environment(stale),
        Err(StoreError::Invalid(_))
    ));
    assert_eq!(
        adapter
            .get_environment("workspace-env", "environment-1")
            .unwrap()
            .unwrap()
            .status(),
        EnvironmentStatus::Provisioning
    );

    let request = ready_transition(
        &created.record,
        "env-ready",
        "event-env-ready",
        "inc-env",
        "opaque/provider-locator-SECRET",
    );
    let ready = adapter.transition_environment(request).unwrap();
    assert_eq!(ready.record.status(), EnvironmentStatus::Ready);
    assert!(!ready.replayed);
    let replay = adapter
        .transition_environment(ready_transition(
            &created.record,
            "env-ready",
            "event-env-ready-regenerated",
            "inc-env",
            "opaque/regenerated-provider-locator",
        ))
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.record, ready.record);

    let connection = Connection::open(state_database(&directory)).unwrap();
    let binding_locator: String = connection.query_row(
        "SELECT opaque_locator_ref FROM environment_provider_bindings WHERE environment_id='environment-1' AND runtime_incarnation_id='inc-env'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(binding_locator, "opaque/provider-locator-SECRET");
    let receipt: String = connection
        .query_row(
            "SELECT response_json FROM request_dedup WHERE request_id='env-ready'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let event_payload: String = connection
        .query_row(
            "SELECT payload_json FROM domain_events WHERE event_id='event-env-ready'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!receipt.contains("opaque/provider-locator-SECRET"));
    assert!(!event_payload.contains("opaque/provider-locator-SECRET"));
}

#[test]
fn ready_binding_refresh_updates_only_matching_current_incarnation_row() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let created = adapter
        .create_environment(create_request(
            new_record(),
            "env-create",
            "event-env-create",
        ))
        .unwrap();
    let ready = adapter
        .transition_environment(ready_transition(
            &created.record,
            "env-ready",
            "event-env-ready",
            "inc-env",
            "opaque/first-provider-locator",
        ))
        .unwrap();
    let busy = adapter
        .transition_environment(busy_transition(&ready.record))
        .unwrap();
    let ready_again = adapter
        .transition_environment(ready_again_transition(&busy.record))
        .unwrap();
    assert_eq!(ready_again.record.status(), EnvironmentStatus::Ready);

    let connection = Connection::open(state_database(&directory)).unwrap();
    let (rows, runtime_id, locator): (i64, String, String) = connection
        .query_row(
            "SELECT COUNT(*), MIN(runtime_id), MIN(opaque_locator_ref) FROM environment_provider_bindings WHERE environment_id='environment-1' AND runtime_incarnation_id='inc-env'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (rows, runtime_id.as_str(), locator.as_str()),
        (1, "runtime-env", "opaque/refreshed-provider-locator")
    );
}

#[test]
fn running_attempt_without_invocations_blocks_checkpointing() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let created = adapter
        .create_environment(create_request(
            new_record(),
            "env-create",
            "event-env-create",
        ))
        .unwrap();
    let ready = adapter
        .transition_environment(ready_transition(
            &created.record,
            "env-ready",
            "event-env-ready",
            "inc-env",
            "opaque/provider-locator",
        ))
        .unwrap();

    // Keep the test focused on the store's authoritative hold query. The historical
    // v4 attempt trigger references the removed runtimes.workspace_id column; do not
    // make that unrelated schema issue a prerequisite for exercising this gate.
    let connection = Connection::open(state_database(&directory)).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = OFF; DROP TRIGGER attempt_environment_owner_guard;")
        .unwrap();
    connection.execute(
        "INSERT INTO attempts(attempt_id,task_id,step_id,agent_binding_id,runtime_id,runtime_incarnation_id,environment_id,failover_class,status,created_at,version)
         VALUES ('attempt-running','task-env','step-not-needed','binding-env','runtime-env','inc-env','environment-1','LOCAL_BOUND','RUNNING',?1,1)", [AT],
    ).unwrap();

    let error = adapter
        .transition_environment(suspend_transition(&ready.record))
        .unwrap_err();
    assert!(
        matches!(error, StoreError::Invalid(message) if message == "ENVIRONMENT_LIFECYCLE_HELD")
    );
    assert_eq!(
        adapter
            .get_environment("workspace-env", "environment-1")
            .unwrap()
            .unwrap()
            .status(),
        EnvironmentStatus::Ready
    );
}

#[test]
fn destroy_fails_closed_without_durable_retention_and_output_proofs() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let created = adapter
        .create_environment(create_request(
            new_record(),
            "env-create",
            "event-env-create",
        ))
        .unwrap();
    let ready = adapter
        .transition_environment(ready_transition(
            &created.record,
            "env-ready",
            "event-env-ready",
            "inc-env",
            "opaque/provider-locator",
        ))
        .unwrap();

    let error = adapter
        .transition_environment(destroy_transition(&ready.record))
        .unwrap_err();
    assert!(
        matches!(error, StoreError::Invalid(message) if message == "ENVIRONMENT_DESTROY_ADMISSION_UNAVAILABLE")
    );
    assert_eq!(
        adapter
            .get_environment("workspace-env", "environment-1")
            .unwrap()
            .unwrap()
            .status(),
        EnvironmentStatus::Ready
    );
}

#[test]
fn environment_update_rolls_back_when_domain_event_insert_fails() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let created = adapter
        .create_environment(create_request(
            new_record(),
            "env-create",
            "event-env-create",
        ))
        .unwrap();
    let connection = Connection::open(state_database(&directory)).unwrap();
    connection.execute(
        "INSERT INTO workspace_origin_sequences(workspace_id,origin_runtime_id,last_sequence) VALUES ('workspace-env','runtime-other',1)", [],
    ).unwrap();
    // Force event insertion to fail after the transition has inserted its private
    // provider binding and updated the Environment row.
    let request = ready_transition(
        &created.record,
        "env-rollback",
        "event-env-create",
        "inc-env",
        "opaque/must-rollback",
    );
    assert!(adapter.transition_environment(request).is_err());
    assert_eq!(
        adapter
            .get_environment("workspace-env", "environment-1")
            .unwrap()
            .unwrap()
            .status(),
        EnvironmentStatus::Provisioning
    );
    let (bindings, receipts): (i64, i64) = connection.query_row(
        "SELECT (SELECT COUNT(*) FROM environment_provider_bindings WHERE environment_id='environment-1'), (SELECT COUNT(*) FROM request_dedup WHERE request_id='env-rollback')",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert_eq!((bindings, receipts), (0, 0));
}

#[test]
fn transition_replay_rechecks_current_workspace_owner_and_active_status() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let created = adapter
        .create_environment(create_request(
            new_record(),
            "env-create",
            "event-env-create",
        ))
        .unwrap();
    let request = failed_transition(
        &created.record,
        "env-failed-owner",
        "event-env-failed-owner",
    );
    adapter.transition_environment(request.clone()).unwrap();
    assert!(
        adapter
            .transition_environment(failed_transition(
                &created.record,
                "env-failed-owner",
                "event-env-failed-owner-regenerated",
            ))
            .unwrap()
            .replayed
    );

    let connection = Connection::open(state_database(&directory)).unwrap();
    connection
        .execute("UPDATE workspaces SET owner_principal_id='new-owner' WHERE workspace_id='workspace-env'", [])
        .unwrap();
    assert!(matches!(
        adapter.transition_environment(request.clone()),
        Err(StoreError::NotFound)
    ));

    connection
        .execute("UPDATE workspaces SET owner_principal_id='owner-local',status='ARCHIVED' WHERE workspace_id='workspace-env'", [])
        .unwrap();
    assert!(
        matches!(adapter.transition_environment(request), Err(StoreError::Invalid(message)) if message == "WORKSPACE_ARCHIVED")
    );
}

#[test]
fn competing_lifecycle_cas_has_one_winner_and_preserves_identity() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    seed_environment_dependencies(&store, &directory);
    let adapter = adapter(&store);
    let created = adapter
        .create_environment(create_request(
            new_record(),
            "env-create",
            "event-env-create",
        ))
        .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|index| {
            let adapter = adapter.clone();
            let record = created.record.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let request = failed_transition(
                    &record,
                    &format!("env-race-{index}"),
                    &format!("event-env-race-{index}"),
                );
                barrier.wait();
                adapter.transition_environment(request)
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(StoreError::Conflict { .. })))
            .count(),
        1
    );
    assert_eq!(
        adapter
            .get_environment("workspace-env", "environment-1")
            .unwrap()
            .unwrap()
            .status(),
        EnvironmentStatus::Failed
    );
}
