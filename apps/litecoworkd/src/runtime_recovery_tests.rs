use super::{
    RuntimeLocalState, RuntimeState, StartupPolicy, ensure_private_state_directory,
    persist_degraded_catalog_transition, persist_state, recover_expired_execution_leases,
    register_runtime_incarnation,
};
use domain_task::{CreateStandaloneTask, TaskService};
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt};
use std::{path::Path, sync::Arc, time::Duration};
use storage_core::{BlobPurpose, DeviceIdentityRecord, RuntimeIncarnationRecord, StoreError};
use storage_sqlite::{
    FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider,
};
use zeroize::Zeroizing;

const RUNTIME_ID: &str = "runtime-integrated-recovery-test";
const OLD_INCARNATION_ID: &str = "incarnation-integrated-recovery-old";
const NOW: &str = "2026-10-12T10:00:00Z";

struct TestKeys;

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([33_u8; 32]),
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

fn local_state(incarnation_id: &str, clean: bool) -> RuntimeLocalState {
    RuntimeLocalState {
        schema_version: 1,
        runtime_id: RUNTIME_ID.to_owned(),
        local_principal_id: Some("principal-integrated-recovery-test".to_owned()),
        os_principal_binding_established: true,
        local_incarnation_id: incarnation_id.to_owned(),
        startup_policy: StartupPolicy::Manual,
        recovery_state: if clean {
            RuntimeState::Stopped
        } else {
            RuntimeState::Starting
        },
        recovered_from_unclean_shutdown: false,
        last_shutdown_clean: clean,
        blockers: Vec::new(),
        version: 1,
    }
}

fn test_device_identity() -> DeviceIdentityRecord {
    let public_key = [17_u8; 32];
    DeviceIdentityRecord {
        device_id: format!("ed25519-sha256:{}", hex::encode(Sha256::digest(public_key))),
        public_key: format!("ed25519:{}", hex::encode(public_key)),
        key_version: 1,
        issued_at: NOW.to_owned(),
        display_name: None,
    }
}

fn store(directory: &Path) -> SqliteWorkspaceStore {
    SqliteWorkspaceStore::open(
        directory.join("litecowork.sqlite3"),
        Arc::new(FileBlobStore::new(directory.join("blobs"), TestKeys)),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("production SQLite store opens")
}

fn register_from_bootstrap(
    store: &SqliteWorkspaceStore,
    state: &RuntimeLocalState,
) -> RuntimeIncarnationRecord {
    register_runtime_incarnation(store, state, test_device_identity(), NOW)
        .expect("Runtime bootstrap registers its incarnation")
}

#[test]
fn unclean_restart_registration_fences_the_previous_incarnation_lease() {
    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    #[cfg(unix)]
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict temporary Runtime data directory");
    ensure_private_state_directory(directory.path()).expect("restrict Runtime data directory");
    let first = local_state(OLD_INCARNATION_ID, false);
    persist_state(directory.path(), &first).expect("persist old unclean bootstrap state");

    let store = store(directory.path());
    register_from_bootstrap(&store, &first);

    let database = directory.path().join("litecowork.sqlite3");
    let fixture = Connection::open(&database).expect("open isolated lease fixture");
    fixture
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable unrelated fixture references");
    fixture
        .execute(
            "INSERT INTO execution_leases(lease_id, task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id, epoch, issuer_key_version, fencing_token_digest, state, acquired_at, renew_by, expires_at, version)
             VALUES ('stale-lease', 'missing-task', 'missing-step', 'missing-attempt', ?1, ?2, 1, 1, ?3, 'ACTIVE', ?4, ?4, ?4, 1)",
            params![RUNTIME_ID, OLD_INCARNATION_ID, format!("sha256:{}", "b".repeat(64)), NOW],
        )
        .expect("old current incarnation can own the seeded active lease");
    drop(fixture);

    let next = RuntimeLocalState::begin(directory.path()).expect("begin after unclean stop");
    assert_eq!(next.runtime_id, RUNTIME_ID);
    assert_ne!(next.local_incarnation_id, OLD_INCARNATION_ID);
    assert!(next.recovered_from_unclean_shutdown);
    assert!(!next.last_shutdown_clean);
    persist_state(directory.path(), &next).expect("persist new bootstrap incarnation");

    let registered = register_from_bootstrap(&store, &next);
    assert!(registered.recovered_from_unclean_shutdown);
    assert_eq!(registered.runtime_incarnation_id, next.local_incarnation_id);

    let connection = Connection::open(database).expect("reopen Runtime catalog");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("restore production foreign-key enforcement");
    let (current_incarnation, recovered_unclean): (String, i64) = connection
        .query_row(
            "SELECT r.current_incarnation_id, i.recovered_from_unclean_shutdown FROM runtimes r JOIN runtime_incarnations i ON i.runtime_id = r.runtime_id AND i.runtime_incarnation_id = r.current_incarnation_id WHERE r.runtime_id = ?1",
            [RUNTIME_ID],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read new durable incarnation");
    assert_eq!(current_incarnation, next.local_incarnation_id);
    assert_eq!(recovered_unclean, 1);

    let error = connection
        .execute(
            "INSERT INTO execution_leases(lease_id, task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id, epoch, issuer_key_version, fencing_token_digest, state, acquired_at, renew_by, expires_at, version)
             VALUES ('stale-lease', 'missing-task', 'missing-step', 'missing-attempt', ?1, ?2, 1, 1, ?3, 'ACTIVE', ?4, ?4, ?4, 1)",
            params![RUNTIME_ID, OLD_INCARNATION_ID, format!("sha256:{}", "b".repeat(64)), NOW],
        )
        .expect_err("old incarnation cannot acquire an active lease after restart");
    assert!(
        error
            .to_string()
            .contains("EXECUTION_LEASE_STALE_RUNTIME_INCARNATION")
    );

    let error = connection
        .execute(
            "UPDATE execution_leases SET expires_at=?1 WHERE lease_id='stale-lease'",
            [NOW],
        )
        .expect_err("old incarnation cannot renew its prior active lease");
    assert!(
        error
            .to_string()
            .contains("EXECUTION_LEASE_STALE_RUNTIME_INCARNATION")
    );

    let lease_after_rejected_renewal: (String, String, i64) = connection
        .query_row(
            "SELECT state, expires_at, version FROM execution_leases WHERE lease_id='stale-lease'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read old lease after rejected renewal");
    assert_eq!(
        lease_after_rejected_renewal,
        ("ACTIVE".to_owned(), NOW.to_owned(), 1)
    );
}

#[test]
fn startup_recovery_failure_persists_blocker_without_partial_settlement_then_retries() {
    const OWNER: &str = "startup-recovery-owner";
    const WORKSPACE: &str = "startup-recovery-workspace";
    const TASK: &str = "startup-recovery-task";
    const BINDING: &str = "startup-recovery-agent-binding";
    const OLD_INCARNATION: &str = "startup-recovery-old-incarnation";

    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    #[cfg(unix)]
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict temporary Runtime data directory");
    ensure_private_state_directory(directory.path()).expect("restrict Runtime data directory");
    let store = store(directory.path());

    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Startup recovery fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "startup-recovery-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "startup-recovery-workspace-created-correlation".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits through its service");

    let old = local_state(OLD_INCARNATION, true);
    persist_state(directory.path(), &old).expect("persist clean old bootstrap state");
    register_from_bootstrap(&store, &old);

    let database = directory.path().join("litecowork.sqlite3");
    let fixture = Connection::open(&database).expect("open existing-execution fixture");
    fixture
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enforce production foreign keys in fixture");
    fixture
        .execute(
            "UPDATE runtime_incarnations SET recovery_state='READY', ready_at=?1, version=version+1 WHERE runtime_incarnation_id=?2",
            params![NOW, OLD_INCARNATION],
        )
        .expect("record the old incarnation as an established ready Runtime");
    fixture
        .execute(
            "UPDATE runtimes SET availability='ONLINE', version=version+1 WHERE runtime_id=?1",
            [RUNTIME_ID],
        )
        .expect("record the old Runtime as online before its Attempt");
    fixture
        .execute(
            "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,version)
             VALUES ('startup-recovery-rwb',?1,?2,'LOCAL_ENROLLMENT','ACTIVE','[\"EXECUTOR\",\"OPERATOR_ENDPOINT\"]',?3,?3,1)",
            params![RUNTIME_ID, WORKSPACE, NOW],
        )
        .expect("active executor binding exists for the old Runtime");
    fixture
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at)
             VALUES ('startup-recovery-profile','test','Recovery fixture agent',?1)",
            [NOW],
        )
        .expect("AgentProfile persists");
    fixture
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version)
             VALUES (?1,?2,'startup-recovery-profile','{}','{}',1,1,?3,1)",
            params![BINDING, WORKSPACE, NOW],
        )
        .expect("AgentBinding persists");
    drop(fixture);

    TaskService::new(store.clone())
        .create_standalone(CreateStandaloneTask {
            task_id: TASK.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            workspace_instruction_revision: None,
            lead_agent_binding_id: BINDING.to_owned(),
            origin_coworker_id: None,
            origin_coworker_revision: None,
            expected_coworker_version: None,
            coworker_default_lead_failover_policy: None,
            principal_id: OWNER.to_owned(),
            request_id: "startup-recovery-task-create".to_owned(),
            request_payload: serde_json::json!({
                "workspace_id": WORKSPACE,
                "objective": "Recover a previously running step after Runtime restart",
                "preferred_lead_agent_binding_id": BINDING,
                "source_message_refs": [], "constraints": [], "non_goals": [],
                "input_refs": [], "required_outputs": [], "acceptance_criteria": [],
                "approvals_required": [], "placement_preference": "AUTO"
            }),
            event: EventContext {
                event_id: "startup-recovery-task-created".to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "startup-recovery-task-created-correlation".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Task and intent revision persist through TaskService");

    // Planning and Attempt admission are intentionally gated in production. Seed
    // one previously admitted execution as durable history, retaining all SQLite
    // foreign keys, checks, and triggers.
    let fixture = Connection::open(&database).expect("open existing-execution fixture");
    fixture
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enforce production foreign keys in fixture");
    let transaction = fixture
        .unchecked_transaction()
        .expect("begin fixture transaction");
    transaction
        .execute(
            "UPDATE tasks SET status='RUNNING', current_plan_revision=1, version=version+1, updated_at=?1 WHERE task_id=?2",
            params![NOW, TASK],
        )
        .expect("persist running Task with accepted-plan pointer");
    let task_version: i64 = transaction
        .query_row(
            "SELECT version FROM tasks WHERE task_id=?1",
            [TASK],
            |row| row.get(0),
        )
        .expect("read seeded Task revision");
    transaction
        .execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at,version)
             VALUES ('startup-recovery-planning-session',?1,'TASK_PLANNING',?2,1,?3,'startup-recovery-endpoint',?4,?5,'CLOSED',?6,1)",
            params![WORKSPACE, TASK, BINDING, RUNTIME_ID, OLD_INCARNATION, NOW],
        )
        .expect("historical planning session satisfies immutable plan provenance");
    let plan_step = serde_json::json!({
        "logical_key": "recover-step", "title": "Recover step",
        "objective": "Settle the expired attempt", "depends_on_logical_keys": [],
        "required_capabilities": [], "acceptance_criteria": []
    });
    transaction
        .execute(
            "INSERT INTO plan_revisions(task_id,revision,task_spec_revision,produced_by_agent_session_id,produced_by_attempt_id,steps_json,reason_for_revision,created_at)
             VALUES (?1,1,1,'startup-recovery-planning-session',NULL,?2,'Previously accepted execution plan',?3)",
            params![TASK, serde_json::json!([plan_step]).to_string(), NOW],
        )
        .expect("immutable plan provenance and session context match");
    transaction
        .execute(
            "INSERT INTO steps(step_id,task_id,plan_revision,logical_key,title,objective,dependencies_json,required_capabilities_json,acceptance_criteria_json,status,current_attempt_id,created_at,updated_at,version)
             VALUES ('startup-recovery-step',?1,1,'recover-step','Recover step','Settle the expired attempt','[]','[]','[]','RUNNING','startup-recovery-attempt',?2,?2,1)",
            params![TASK, NOW],
        )
        .expect("materialize the exact accepted PlanRevision step");
    transaction
        .execute(
            "INSERT INTO environments(environment_id,runtime_id,provider_kind,class,lifetime,owner_workspace_id,owner_task_id,sharing_scope,name,created_by_incarnation_id,status,health,budget_enforcement_policy,budget_enforcement,source_resources_json,resource_limits_json,network_policy_json,budget_ceiling_json,backup_policy,created_at,updated_at,version)
             VALUES ('startup-recovery-environment',?1,'test-provider','CONTAINER','TASK_RETAINED',?2,?3,'TASK_SHARED','Recovery fixture',?4,'READY','HEALTHY','ALLOW_HOST_MONITORED','HOST_MONITORED','[]','{}','{}','{}','EXCLUDED',?5,?5,1)",
            params![RUNTIME_ID, WORKSPACE, TASK, OLD_INCARNATION, NOW],
        )
        .expect("Task-shared Environment belongs to the old Runtime incarnation");
    transaction
        .execute(
            "INSERT INTO attempts(attempt_id,task_id,step_id,agent_binding_id,runtime_id,runtime_incarnation_id,environment_id,failover_class,status,started_at,created_at,version)
             VALUES ('startup-recovery-attempt',?1,'startup-recovery-step',?2,?3,?4,'startup-recovery-environment','LOCAL_BOUND','RUNNING',?5,?5,1)",
            params![TASK, BINDING, RUNTIME_ID, OLD_INCARNATION, NOW],
        )
        .expect("running Attempt satisfies Task, Step, Runtime and Environment guards");
    transaction
        .execute(
            "INSERT INTO execution_leases(lease_id,task_id,step_id,attempt_id,runtime_id,runtime_incarnation_id,epoch,issuer_key_version,fencing_token_digest,state,acquired_at,renew_by,expires_at,version)
             VALUES ('startup-recovery-lease',?1,'startup-recovery-step','startup-recovery-attempt',?2,?3,1,1,?4,'ACTIVE','2000-01-01T00:00:00Z','2000-01-01T00:00:00Z','2000-01-01T00:00:00Z',1)",
            params![TASK, RUNTIME_ID, OLD_INCARNATION, format!("sha256:{}", "c".repeat(64))],
        )
        .expect("expired lease was valid under the old current incarnation");
    transaction
        .commit()
        .expect("complete valid historical execution graph");
    let foreign_key_failures: i64 = fixture
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("check seeded graph foreign keys");
    assert_eq!(
        foreign_key_failures, 0,
        "fixture has no broken foreign keys"
    );
    drop(fixture);

    let next = RuntimeLocalState::begin(directory.path()).expect("begin next Runtime incarnation");
    assert_ne!(next.local_incarnation_id, OLD_INCARNATION);
    persist_state(directory.path(), &next).expect("persist new bootstrap state");
    register_from_bootstrap(&store, &next);
    let mut incarnation_version = 1;
    let mut degraded = next.clone();
    degraded.transition(
        RuntimeState::Degraded,
        vec!["TASK_ATTEMPT_ADMISSION_NOT_INTEGRATED".to_owned()],
    );
    persist_degraded_catalog_transition(
        directory.path(),
        &store,
        &mut degraded,
        &mut incarnation_version,
    )
    .expect("new incarnation becomes durably DEGRADED before recovery");

    let connection = Connection::open(&database).expect("open recovery failure injector");
    connection
        .execute_batch(
            "CREATE TRIGGER inject_expired_lease_recovery_failure
             BEFORE INSERT ON domain_events
             WHEN NEW.type = 'lease.expired.v1'
             BEGIN SELECT RAISE(ABORT, 'injected recovery event failure'); END;",
        )
        .expect("install transaction failure injector");
    drop(connection);

    let blockers = super::recover_and_persist_startup_blockers(
        directory.path(),
        &store,
        OWNER,
        &mut degraded,
        vec![super::RuntimeLifecycleBlocker {
            code: "TASK_ATTEMPT_ADMISSION_NOT_INTEGRATED".to_owned(),
            created_at: NOW.to_owned(),
        }],
    )
    .expect("recovery failure is durably projected before Operator admission");
    assert_eq!(
        blockers
            .iter()
            .map(|blocker| blocker.code.as_str())
            .collect::<Vec<_>>(),
        [
            "TASK_ATTEMPT_ADMISSION_NOT_INTEGRATED",
            "EXPIRED_EXECUTION_LEASE_RECOVERY_FAILED"
        ]
    );
    let failed_recovery_state: (String, String, String, String, i64, i64, i64) =
        Connection::open(&database)
            .expect("reopen after injected transaction failure")
            .query_row(
                "SELECT l.state,a.status,s.status,t.status,t.version,s.version,a.version
                 FROM execution_leases l JOIN attempts a ON a.attempt_id=l.attempt_id
                 JOIN steps s ON s.step_id=l.step_id JOIN tasks t ON t.task_id=l.task_id
                 WHERE l.lease_id='startup-recovery-lease'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("read graph after rejected recovery transaction");
    assert_eq!(
        failed_recovery_state,
        (
            "ACTIVE".into(),
            "RUNNING".into(),
            "RUNNING".into(),
            "RUNNING".into(),
            task_version,
            1,
            1
        )
    );
    let failed_recovery_events: i64 = Connection::open(&database)
        .expect("reopen event log after rejected transaction")
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id=?1 AND entity_id IN ('startup-recovery-lease','startup-recovery-attempt','startup-recovery-step',?2) AND type IN ('lease.expired.v1','attempt.status.changed.v1','step.status.changed.v1','task.status.changed.v1')",
            params![WORKSPACE, TASK],
            |row| row.get(0),
        )
        .expect("count events after recovery rollback");
    assert_eq!(failed_recovery_events, 0);
    let persisted = super::read_state(&directory.path().join("runtime-state.json"))
        .expect("read failed-recovery lifecycle projection")
        .expect("failed-recovery projection persists");
    assert!(
        persisted
            .blockers
            .iter()
            .any(|blocker| { blocker == "EXPIRED_EXECUTION_LEASE_RECOVERY_FAILED" })
    );

    let connection = Connection::open(&database).expect("open failure injector for removal");
    connection
        .execute_batch("DROP TRIGGER inject_expired_lease_recovery_failure")
        .expect("remove transaction failure injector");
    drop(connection);

    assert_eq!(
        recover_expired_execution_leases(&store, OWNER, RUNTIME_ID, &next.local_incarnation_id)
            .expect("startup recovery settles expired lease"),
        1
    );

    let connection = Connection::open(&database).expect("read recovered execution graph");
    let state: (String, String, String, String, i64, i64, i64) = connection
        .query_row(
            "SELECT l.state,a.status,s.status,t.status,t.version,s.version,a.version
             FROM execution_leases l JOIN attempts a ON a.attempt_id=l.attempt_id
             JOIN steps s ON s.step_id=l.step_id JOIN tasks t ON t.task_id=l.task_id
             WHERE l.lease_id='startup-recovery-lease'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .expect("read settled lease, Attempt, Step and Task");
    assert_eq!(
        state,
        (
            "EXPIRED".into(),
            "ABANDONED".into(),
            "BLOCKED".into(),
            "BLOCKED".into(),
            task_version + 1,
            2,
            2
        )
    );
    let blocker: String = connection
        .query_row(
            "SELECT blocking_conditions_json FROM tasks WHERE task_id=?1",
            [TASK],
            |row| row.get(0),
        )
        .expect("read Task recovery blocker");
    assert!(blocker.contains("STALE_FENCE"));
    let recovered_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id=?1 AND entity_id IN ('startup-recovery-lease','startup-recovery-attempt','startup-recovery-step',?2) AND type IN ('lease.expired.v1','attempt.status.changed.v1','step.status.changed.v1','task.status.changed.v1')",
            params![WORKSPACE, TASK],
            |row| row.get(0),
        )
        .expect("count durable recovery events");
    assert_eq!(recovered_events, 4);
    drop(connection);

    assert_eq!(
        recover_expired_execution_leases(&store, OWNER, RUNTIME_ID, &next.local_incarnation_id)
            .expect("repeated startup recovery is idempotent"),
        0
    );
    let connection = Connection::open(&database).expect("read replay-safe recovery result");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id=?1",
            [WORKSPACE],
            |row| row.get(0),
        )
        .expect("count Workspace events after retry");
    assert_eq!(
        event_count,
        recovered_events + 2,
        "retry adds no recovery events"
    );
}
