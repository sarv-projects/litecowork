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
use storage_core::{
    BlobPurpose, ConversationMessageRecord, ConversationRecord, ConversationStore,
    ConversationTurnStore, CreateConversationCommit, CreateConversationTurnCommit,
    DeviceIdentityRecord, EventDraft, RuntimeIncarnationRecord, StoreError, WorkspaceCreateRequest,
    conversation::{
        CommitConversationAgentResponse, ConversationTurn, ConversationTurnStatus,
        PresentationPreference, request_payload_value,
    },
};
use storage_sqlite::{
    FileBlobStore, SqliteConfig, SqliteConversationStore, SqliteWorkspaceStore, WorkspaceBlobKey,
    WorkspaceBlobKeyProvider,
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

#[test]
fn conversation_session_recovery_releases_orphan_starting_session_idempotently() {
    const OWNER: &str = "conversation-orphan-recovery-owner";
    const WORKSPACE: &str = "conversation-orphan-recovery-workspace";
    const CONVERSATION: &str = "conversation-orphan-recovery-conversation";
    const TURN: &str = "conversation-orphan-recovery-turn";
    const SESSION: &str = "conversation-orphan-recovery-session";
    const HOST: &str = "conversation-orphan-recovery-host";
    const BINDING: &str = "conversation-orphan-recovery-binding";
    const PROFILE: &str = "conversation-orphan-recovery-profile";

    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    #[cfg(unix)]
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict temporary Runtime data directory");
    ensure_private_state_directory(directory.path()).expect("restrict Runtime data directory");
    let store = store(directory.path());
    let old = local_state(OLD_INCARNATION_ID, true);
    persist_state(directory.path(), &old).expect("persist the previous Runtime incarnation");
    register_from_bootstrap(&store, &old);

    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Orphan Conversation recovery fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "conversation-orphan-recovery-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "conversation-orphan-recovery-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits through WorkspaceService");

    let database = directory.path().join("litecowork.sqlite3");
    let connection = Connection::open(&database).expect("open Conversation recovery fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enforce production foreign keys in the fixture");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at)
             VALUES (?1,'test','Recovery fixture agent',?2)",
            params![PROFILE, NOW],
        )
        .expect("AgentProfile persists");
    connection
        .execute(
            "INSERT INTO agent_endpoints(endpoint_id,agent_profile_id,protocol,topology,protocol_version,capabilities_json)
             VALUES ('conversation-orphan-recovery-endpoint',?1,'CLI','PROCESS_ADAPTER','test','{}')",
            [PROFILE],
        )
        .expect("AgentEndpoint persists");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version)
             VALUES (?1,?2,?3,'{}','{}',1,1,?4,1)",
            params![BINDING, WORKSPACE, PROFILE, NOW],
        )
        .expect("lead-eligible Workspace binding persists");
    connection
        .execute(
            "UPDATE workspaces SET default_agent_binding_id=?1 WHERE workspace_id=?2",
            params![BINDING, WORKSPACE],
        )
        .expect("configure the binding selected by the turn");
    connection
        .execute(
            "INSERT INTO agent_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,agent_profile_id,endpoint_id,hosting_mode,state,process_identity_ref,ownership,started_at,last_used_at,idle_since)
             VALUES (?1,?2,?3,?4,'conversation-orphan-recovery-endpoint','LOCAL_PER_SESSION','BUSY',NULL,'LITECOWORK',?5,?5,NULL)",
            params![HOST, RUNTIME_ID, OLD_INCARNATION_ID, PROFILE, NOW],
        )
        .expect("prior-incarnation AgentHost persists");
    drop(connection);

    let conversation_event_id = "conversation-orphan-recovery-conversation-created";
    SqliteConversationStore::new(store.clone())
        .create_conversation(CreateConversationCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "conversation-orphan-recovery-conversation-request".to_owned(),
                request_payload: request_payload_value(WORKSPACE, Some("Recovery fixture")),
            },
            conversation: ConversationRecord {
                conversation_id: CONVERSATION.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                title: Some("Recovery fixture".to_owned()),
                active_agent_binding_id: None,
                version: 1,
                created_at: NOW.to_owned(),
            },
            event: EventDraft {
                event_id: conversation_event_id.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "Conversation".to_owned(),
                entity_id: CONVERSATION.to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                entity_revision: 1,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: conversation_event_id.to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.created.v1".to_owned(),
                payload: serde_json::json!({
                    "conversation_id": CONVERSATION,
                    "created_by": {"kind":"USER","principal_id":OWNER}
                }),
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Conversation and creation event commit");

    let message_id = format!("{TURN}-user-message");
    let content = serde_json::json!([{"kind":"TEXT","text":"Keep this request after recovery."}]);
    let resource_refs = serde_json::json!([]);
    let author = serde_json::json!({"kind":"USER","principal_id":OWNER});
    let turn_request = serde_json::json!({
        "operation":"conversation.turn.create.v1",
        "workspace_id":WORKSPACE,
        "conversation_id":CONVERSATION,
        "turn_id":TURN,
        "message_id":message_id,
        "author":author,
        "content":content,
        "resource_refs":resource_refs,
        "presentation_preference":"AUTO"
    });
    let content_digest = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(&content).expect("serialize USER content")
        ))
    );
    SqliteConversationStore::new(store.clone())
        .create_conversation_turn(CreateConversationTurnCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "conversation-orphan-recovery-turn-request".to_owned(),
                request_payload: turn_request,
            },
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            message: ConversationMessageRecord {
                message_id: message_id.clone(),
                conversation_id: CONVERSATION.to_owned(),
                author: author.clone(),
                role: "USER".to_owned(),
                agent_session_id: None,
                agent_binding_id: None,
                turn_id: Some(TURN.to_owned()),
                content: content.clone(),
                resource_refs: resource_refs.clone(),
                source_channel_ref: None,
                created_at: NOW.to_owned(),
            },
            turn: ConversationTurn {
                turn_id: TURN.to_owned(),
                conversation_id: CONVERSATION.to_owned(),
                user_message_id: message_id.clone(),
                agent_session_id: None,
                status: ConversationTurnStatus::Open,
                retry_ordinal: 0,
                presentation_preference: PresentationPreference::Auto,
                created_at: NOW.to_owned(),
                settled_at: None,
                version: 1,
            },
            message_event: EventDraft {
                event_id: "conversation-orphan-recovery-user-message".to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "Conversation".to_owned(),
                entity_id: CONVERSATION.to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                entity_revision: 2,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "conversation-orphan-recovery-turn".to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.message.added.v1".to_owned(),
                payload: serde_json::json!({
                    "message_id":message_id,
                    "conversation_id":CONVERSATION,
                    "author":author,
                    "content_digest":content_digest,
                    "resource_refs":resource_refs
                }),
                recorded_at: NOW.to_owned(),
            },
            turn_event: EventDraft {
                event_id: "conversation-orphan-recovery-turn-created".to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "ConversationTurn".to_owned(),
                entity_id: TURN.to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                entity_revision: 1,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "conversation-orphan-recovery-turn".to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.turn.created.v2".to_owned(),
                payload: serde_json::json!({
                    "turn_id":TURN,
                    "conversation_id":CONVERSATION,
                    "user_message_id":message_id,
                    "agent_binding_id":BINDING,
                    "aggregate_version":1,
                    "presentation_preference":"AUTO"
                }),
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("USER message, OPEN turn and dispatch intent commit together");

    // Simulate a durable STARTING reservation that survived the process, before the
    // turn acquired its session pointer. Recovery must settle the intent, then release
    // this orphan session through the STARTING -> LOST storage transition.
    let connection = Connection::open(&database).expect("open orphan session fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enforce production foreign keys for the orphan session");
    connection
        .execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,conversation_id,conversation_turn_id,task_id,task_spec_revision,attempt_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,configuration_digest,harness_descriptor_digest,status,started_at,last_event_at,closed_at,version)
             VALUES (?1,?2,'CONVERSATION',?3,?4,NULL,NULL,NULL,?5,'conversation-orphan-recovery-endpoint',?6,?7,NULL,NULL,'STARTING',?8,?8,NULL,1)",
            params![SESSION, WORKSPACE, CONVERSATION, TURN, BINDING, RUNTIME_ID, OLD_INCARNATION_ID, NOW],
        )
        .expect("persist the orphan STARTING reservation with valid scope references");
    connection
        .execute(
            "INSERT INTO agent_session_host_bindings(agent_session_id,host_instance_id,native_session_ref,bound_at)
             VALUES (?1,?2,NULL,?3)",
            params![SESSION, HOST, NOW],
        )
        .expect("attach the orphan session to its prior-incarnation host");
    assert!(
        connection
            .execute(
                "DELETE FROM agent_session_host_bindings WHERE agent_session_id=?1",
                [SESSION],
            )
            .is_err(),
        "SQLite must reject releasing a host binding while its AgentSession is STARTING"
    );
    let binding_count_before_recovery: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM agent_session_host_bindings WHERE agent_session_id=?1",
            [SESSION],
            |row| row.get(0),
        )
        .expect("read the binding after the rejected early release");
    assert_eq!(binding_count_before_recovery, 1);
    drop(connection);

    let next = RuntimeLocalState::begin(directory.path()).expect("begin a new Runtime incarnation");
    assert_ne!(next.local_incarnation_id, OLD_INCARNATION_ID);
    persist_state(directory.path(), &next).expect("persist the restarted Runtime incarnation");
    register_from_bootstrap(&store, &next);

    let report = crate::agents::conversation_recovery::recover_conversation_sessions(
        &store, RUNTIME_ID, OWNER,
    );
    assert_eq!(
        (
            report.failures,
            report.intents_settled,
            report.starting_sessions_released
        ),
        (0, 1, 1),
        "the unsettled intent and the now-orphaned STARTING session are both reconciled"
    );
    let connection = Connection::open(&database).expect("read recovered Conversation state");
    let turn_state: (String, Option<String>) = connection
        .query_row(
            "SELECT status, settled_at FROM conversation_turns WHERE turn_id=?1",
            [TURN],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read recovered turn");
    assert_eq!(turn_state.0, "FAILED");
    assert!(turn_state.1.is_some());
    let intent_state: String = connection
        .query_row(
            "SELECT state FROM conversation_dispatch_intents WHERE turn_id=?1",
            [TURN],
            |row| row.get(0),
        )
        .expect("read recovered dispatch intent");
    assert_eq!(intent_state, "SETTLED");
    let (session_state, session_closed_at): (String, Option<String>) = connection
        .query_row(
            "SELECT status, closed_at FROM agent_sessions WHERE agent_session_id=?1",
            [SESSION],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read recovered AgentSession");
    assert_eq!(session_state, "LOST");
    assert!(session_closed_at.is_some());
    let binding_count_after_recovery: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM agent_session_host_bindings WHERE agent_session_id=?1",
            [SESSION],
            |row| row.get(0),
        )
        .expect("read the host binding after recovery");
    assert_eq!(
        binding_count_after_recovery, 0,
        "the settled LOST session releases its local host binding"
    );
    let message_counts: (i64, i64) = connection
        .query_row(
            "SELECT SUM(role='USER'), SUM(role='AGENT') FROM conversation_messages WHERE turn_id=?1",
            [TURN],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("count messages for the recovered turn");
    assert_eq!(
        message_counts,
        (1, 0),
        "recovery keeps USER input and invents no answer"
    );
    let reason: String = connection
        .query_row(
            "SELECT json_extract(payload_json,'$.reason_code') FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1 AND type='conversation.turn.settled.v1'",
            [TURN],
            |row| row.get(0),
        )
        .expect("read the typed recovery reason");
    assert_eq!(reason, "RUNTIME_RESTARTED");
    drop(connection);

    let repeated = crate::agents::conversation_recovery::recover_conversation_sessions(
        &store, RUNTIME_ID, OWNER,
    );
    assert_eq!(repeated, Default::default(), "a second sweep is idempotent");
}

#[test]
fn conversation_session_recovery_fails_active_session_after_prior_incarnation() {
    const OWNER: &str = "conversation-active-recovery-owner";
    const WORKSPACE: &str = "conversation-active-recovery-workspace";
    const CONVERSATION: &str = "conversation-active-recovery-conversation";
    const TURN: &str = "conversation-active-recovery-turn";
    const MESSAGE: &str = "conversation-active-recovery-user-message";
    const SESSION: &str = "conversation-active-recovery-session";
    const HOST: &str = "conversation-active-recovery-host";
    const BINDING: &str = "conversation-active-recovery-binding";
    const PROFILE: &str = "conversation-active-recovery-profile";
    const ENDPOINT: &str = "conversation-active-recovery-endpoint";

    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    #[cfg(unix)]
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict temporary Runtime data directory");
    ensure_private_state_directory(directory.path()).expect("restrict Runtime data directory");
    let store = store(directory.path());
    let old = local_state(OLD_INCARNATION_ID, true);
    persist_state(directory.path(), &old).expect("persist the previous Runtime incarnation");
    register_from_bootstrap(&store, &old);

    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Active Conversation recovery fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "conversation-active-recovery-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "conversation-active-recovery-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits through WorkspaceService");

    let database = directory.path().join("litecowork.sqlite3");
    let connection = Connection::open(&database).expect("open active recovery fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enforce production foreign keys in the fixture");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at)
             VALUES (?1,'test','Active recovery fixture agent',?2)",
            params![PROFILE, NOW],
        )
        .expect("AgentProfile persists");
    connection
        .execute(
            "INSERT INTO agent_endpoints(endpoint_id,agent_profile_id,protocol,topology,protocol_version,capabilities_json)
             VALUES (?1,?2,'CLI','PROCESS_ADAPTER','test','{}')",
            params![ENDPOINT, PROFILE],
        )
        .expect("AgentEndpoint persists");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version)
             VALUES (?1,?2,?3,'{}','{}',1,1,?4,1)",
            params![BINDING, WORKSPACE, PROFILE, NOW],
        )
        .expect("lead-eligible Workspace binding persists");
    connection
        .execute(
            "UPDATE workspaces SET default_agent_binding_id=?1 WHERE workspace_id=?2",
            params![BINDING, WORKSPACE],
        )
        .expect("configure the binding selected by the turn");
    connection
        .execute(
            "INSERT INTO agent_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,agent_profile_id,endpoint_id,hosting_mode,state,process_identity_ref,ownership,started_at,last_used_at,idle_since)
             VALUES (?1,?2,?3,?4,?5,'LOCAL_PER_SESSION','BUSY',NULL,'LITECOWORK',?6,?6,NULL)",
            params![HOST, RUNTIME_ID, OLD_INCARNATION_ID, PROFILE, ENDPOINT, NOW],
        )
        .expect("prior-incarnation AgentHost persists");
    drop(connection);

    let conversation_event_id = "conversation-active-recovery-conversation-created";
    SqliteConversationStore::new(store.clone())
        .create_conversation(CreateConversationCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "conversation-active-recovery-conversation-request".to_owned(),
                request_payload: request_payload_value(WORKSPACE, Some("Active recovery")),
            },
            conversation: ConversationRecord {
                conversation_id: CONVERSATION.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                title: Some("Active recovery".to_owned()),
                active_agent_binding_id: Some(BINDING.to_owned()),
                version: 1,
                created_at: NOW.to_owned(),
            },
            event: EventDraft {
                event_id: conversation_event_id.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "Conversation".to_owned(),
                entity_id: CONVERSATION.to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                entity_revision: 1,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: conversation_event_id.to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.created.v1".to_owned(),
                payload: serde_json::json!({
                    "conversation_id": CONVERSATION,
                    "created_by": {"kind":"USER","principal_id":OWNER}
                }),
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Conversation and creation event commit");

    let content = serde_json::json!([{"kind":"TEXT","text":"Recover this active request."}]);
    let resource_refs = serde_json::json!([]);
    let author = serde_json::json!({"kind":"USER","principal_id":OWNER});
    let turn_request = serde_json::json!({
        "operation":"conversation.turn.create.v1",
        "workspace_id":WORKSPACE,
        "conversation_id":CONVERSATION,
        "turn_id":TURN,
        "message_id":MESSAGE,
        "author":author,
        "content":content,
        "resource_refs":resource_refs,
        "presentation_preference":"AUTO"
    });
    let content_digest = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(&content).expect("serialize USER content")
        ))
    );
    SqliteConversationStore::new(store.clone())
        .create_conversation_turn(CreateConversationTurnCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "conversation-active-recovery-turn-request".to_owned(),
                request_payload: turn_request,
            },
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            message: ConversationMessageRecord {
                message_id: MESSAGE.to_owned(),
                conversation_id: CONVERSATION.to_owned(),
                author: author.clone(),
                role: "USER".to_owned(),
                agent_session_id: None,
                agent_binding_id: None,
                turn_id: Some(TURN.to_owned()),
                content: content.clone(),
                resource_refs: resource_refs.clone(),
                source_channel_ref: None,
                created_at: NOW.to_owned(),
            },
            turn: ConversationTurn {
                turn_id: TURN.to_owned(),
                conversation_id: CONVERSATION.to_owned(),
                user_message_id: MESSAGE.to_owned(),
                agent_session_id: None,
                status: ConversationTurnStatus::Open,
                retry_ordinal: 0,
                presentation_preference: PresentationPreference::Auto,
                created_at: NOW.to_owned(),
                settled_at: None,
                version: 1,
            },
            message_event: EventDraft {
                event_id: "conversation-active-recovery-user-message".to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "Conversation".to_owned(),
                entity_id: CONVERSATION.to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                entity_revision: 2,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: TURN.to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.message.added.v1".to_owned(),
                payload: serde_json::json!({
                    "message_id": MESSAGE,
                    "conversation_id": CONVERSATION,
                    "author": author,
                    "content_digest": content_digest,
                    "resource_refs": resource_refs
                }),
                recorded_at: NOW.to_owned(),
            },
            turn_event: EventDraft {
                event_id: "conversation-active-recovery-turn-created".to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "ConversationTurn".to_owned(),
                entity_id: TURN.to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                entity_revision: 1,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: TURN.to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.turn.created.v2".to_owned(),
                payload: serde_json::json!({
                    "turn_id": TURN,
                    "conversation_id": CONVERSATION,
                    "user_message_id": MESSAGE,
                    "agent_binding_id": BINDING,
                    "aggregate_version": 1,
                    "presentation_preference": "AUTO"
                }),
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("USER message, OPEN turn and dispatch intent commit together");

    let connection = Connection::open(&database).expect("open activated Conversation state");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enforce production foreign keys for the activated session");
    connection
        .execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,conversation_id,conversation_turn_id,task_id,task_spec_revision,attempt_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,configuration_digest,harness_descriptor_digest,status,started_at,last_event_at,closed_at,version)
             VALUES (?1,?2,'CONVERSATION',?3,?4,NULL,NULL,NULL,?5,?6,?7,?8,NULL,NULL,'ACTIVE',?9,?9,NULL,2)",
            params![SESSION, WORKSPACE, CONVERSATION, TURN, BINDING, ENDPOINT, RUNTIME_ID, OLD_INCARNATION_ID, NOW],
        )
        .expect("persist the active AgentSession from the previous incarnation");
    connection
        .execute(
            "UPDATE conversation_turns SET agent_session_id=?1,status='RUNNING',version=2 WHERE turn_id=?2 AND status='OPEN'",
            params![SESSION, TURN],
        )
        .expect("persist the activated RUNNING turn/session pointer");
    connection
        .execute(
            "UPDATE conversation_dispatch_intents SET state='CLAIMED',claimed_by_runtime_incarnation=?1,claimed_at=?2,version=version+1 WHERE turn_id=?3 AND retry_ordinal=0 AND state='UNCLAIMED'",
            params![OLD_INCARNATION_ID, NOW, TURN],
        )
        .expect("claim the dispatch intent under the previous incarnation");
    connection
        .execute(
            "INSERT INTO agent_session_host_bindings(agent_session_id,host_instance_id,native_session_ref,bound_at)
             VALUES (?1,?2,NULL,?3)",
            params![SESSION, HOST, NOW],
        )
        .expect("attach the active session to its prior-incarnation host");
    drop(connection);

    let next = RuntimeLocalState::begin(directory.path()).expect("begin a new Runtime incarnation");
    assert_ne!(next.local_incarnation_id, OLD_INCARNATION_ID);
    persist_state(directory.path(), &next).expect("persist the restarted Runtime incarnation");
    register_from_bootstrap(&store, &next);

    let report = crate::agents::conversation_recovery::recover_conversation_sessions(
        &store, RUNTIME_ID, OWNER,
    );
    assert_eq!((report.failures, report.intents_settled), (0, 1));
    assert_eq!(report.starting_sessions_released, 0);

    let connection = Connection::open(&database).expect("read recovered active state");
    let (turn_status, turn_session): (String, Option<String>) = connection
        .query_row(
            "SELECT status,agent_session_id FROM conversation_turns WHERE turn_id=?1",
            [TURN],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read the recovered active turn");
    assert_eq!(turn_status, "FAILED");
    assert_eq!(turn_session.as_deref(), Some(SESSION));
    let session_status: String = connection
        .query_row(
            "SELECT status FROM agent_sessions WHERE agent_session_id=?1",
            [SESSION],
            |row| row.get(0),
        )
        .expect("read the recovered AgentSession");
    assert_eq!(session_status, "LOST");
    let intent_state: String = connection
        .query_row(
            "SELECT state FROM conversation_dispatch_intents WHERE turn_id=?1 AND retry_ordinal=0",
            [TURN],
            |row| row.get(0),
        )
        .expect("read the recovered dispatch intent");
    assert_eq!(intent_state, "SETTLED");
    let binding_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM agent_session_host_bindings WHERE agent_session_id=?1",
            [SESSION],
            |row| row.get(0),
        )
        .expect("read the recovered host binding");
    assert_eq!(binding_count, 0);
    let message_counts: (i64, i64) = connection
        .query_row(
            "SELECT SUM(role='USER'),SUM(role='AGENT') FROM conversation_messages WHERE turn_id=?1",
            [TURN],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("count messages for the recovered turn");
    assert_eq!(message_counts, (1, 0));
    let reason: String = connection
        .query_row(
            "SELECT json_extract(payload_json,'$.reason_code') FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1 AND type='conversation.turn.settled.v1'",
            [TURN],
            |row| row.get(0),
        )
        .expect("read the typed recovery reason");
    assert_eq!(reason, "RUNTIME_RESTARTED");
}

#[test]
fn conversation_session_recovery_preserves_final_response_and_receipt() {
    const OWNER: &str = "conversation-final-recovery-owner";
    const WORKSPACE: &str = "conversation-final-recovery-workspace";
    const CONVERSATION: &str = "conversation-final-recovery-conversation";
    const TURN: &str = "conversation-final-recovery-turn";
    const USER_MESSAGE: &str = "conversation-final-recovery-user-message";
    const AGENT_MESSAGE: &str = "conversation-final-recovery-agent-message";
    const SESSION: &str = "conversation-final-recovery-session";
    const HOST: &str = "conversation-final-recovery-host";
    const BINDING: &str = "conversation-final-recovery-binding";
    const PROFILE: &str = "conversation-final-recovery-profile";
    const ENDPOINT: &str = "conversation-final-recovery-endpoint";
    const RESPONSE_REQUEST: &str = "conversation-final-recovery-response-request";
    const RESPONSE_ID: &str = "conversation-final-recovery-provider-response";
    const ANSWER: &str = "The committed response survives Runtime recovery.";
    const COMMITTED_AT: &str = "2026-10-12T10:00:01Z";

    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    #[cfg(unix)]
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict temporary Runtime data directory");
    ensure_private_state_directory(directory.path()).expect("restrict Runtime data directory");

    let old = local_state(OLD_INCARNATION_ID, false);
    persist_state(directory.path(), &old).expect("persist the unclean prior Runtime incarnation");
    let store = store(directory.path());
    register_from_bootstrap(&store, &old);

    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Final response recovery fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "conversation-final-recovery-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "conversation-final-recovery-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits through WorkspaceService");

    let database = directory.path().join("litecowork.sqlite3");
    let connection = Connection::open(&database).expect("open final response recovery fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enforce production foreign keys in fixture");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at)
             VALUES (?1,'test','Final response recovery agent',?2)",
            params![PROFILE, NOW],
        )
        .expect("AgentProfile persists");
    connection
        .execute(
            "INSERT INTO agent_endpoints(endpoint_id,agent_profile_id,protocol,topology,protocol_version,capabilities_json)
             VALUES (?1,?2,'CLI','PROCESS_ADAPTER','test','{}')",
            params![ENDPOINT, PROFILE],
        )
        .expect("AgentEndpoint persists");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version)
             VALUES (?1,?2,?3,'{}','{}',1,1,?4,1)",
            params![BINDING, WORKSPACE, PROFILE, NOW],
        )
        .expect("lead-eligible Workspace binding persists");
    connection
        .execute(
            "UPDATE workspaces SET default_agent_binding_id=?1 WHERE workspace_id=?2",
            params![BINDING, WORKSPACE],
        )
        .expect("configure the binding selected by turn admission");
    connection
        .execute(
            "INSERT INTO agent_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,agent_profile_id,endpoint_id,hosting_mode,state,process_identity_ref,ownership,started_at,last_used_at,idle_since)
             VALUES (?1,?2,?3,?4,?5,'LOCAL_PER_SESSION','BUSY',NULL,'LITECOWORK',?6,?6,NULL)",
            params![HOST, RUNTIME_ID, OLD_INCARNATION_ID, PROFILE, ENDPOINT, NOW],
        )
        .expect("prior-incarnation AgentHost persists");
    drop(connection);

    let conversation_store = SqliteConversationStore::new(store.clone());
    conversation_store
        .create_conversation(CreateConversationCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "conversation-final-recovery-create-request".to_owned(),
                request_payload: request_payload_value(WORKSPACE, Some("Final response recovery")),
            },
            conversation: ConversationRecord {
                conversation_id: CONVERSATION.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                title: Some("Final response recovery".to_owned()),
                active_agent_binding_id: Some(BINDING.to_owned()),
                version: 1,
                created_at: NOW.to_owned(),
            },
            event: EventDraft {
                event_id: "conversation-final-recovery-conversation-created".to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "Conversation".to_owned(),
                entity_id: CONVERSATION.to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                entity_revision: 1,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: CONVERSATION.to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.created.v1".to_owned(),
                payload: serde_json::json!({
                    "conversation_id": CONVERSATION,
                    "created_by": {"kind":"USER","principal_id":OWNER}
                }),
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Conversation and creation event commit");

    let user_content =
        serde_json::json!([{"kind":"TEXT","text":"Please preserve this completed answer."}]);
    let user_author = serde_json::json!({"kind":"USER","principal_id":OWNER});
    let resource_refs = serde_json::json!([]);
    let user_content_digest = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(&user_content).expect("serialize USER content")
        ))
    );
    conversation_store
        .create_conversation_turn(CreateConversationTurnCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "conversation-final-recovery-turn-request".to_owned(),
                request_payload: serde_json::json!({
                    "operation":"conversation.turn.create.v1",
                    "workspace_id":WORKSPACE,
                    "conversation_id":CONVERSATION,
                    "turn_id":TURN,
                    "message_id":USER_MESSAGE,
                    "author":user_author,
                    "content":user_content,
                    "resource_refs":resource_refs,
                    "presentation_preference":"AUTO"
                }),
            },
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            message: ConversationMessageRecord {
                message_id: USER_MESSAGE.to_owned(),
                conversation_id: CONVERSATION.to_owned(),
                author: user_author.clone(),
                role: "USER".to_owned(),
                agent_session_id: None,
                agent_binding_id: None,
                turn_id: Some(TURN.to_owned()),
                content: user_content.clone(),
                resource_refs: resource_refs.clone(),
                source_channel_ref: None,
                created_at: NOW.to_owned(),
            },
            turn: ConversationTurn {
                turn_id: TURN.to_owned(),
                conversation_id: CONVERSATION.to_owned(),
                user_message_id: USER_MESSAGE.to_owned(),
                agent_session_id: None,
                status: ConversationTurnStatus::Open,
                retry_ordinal: 0,
                presentation_preference: PresentationPreference::Auto,
                created_at: NOW.to_owned(),
                settled_at: None,
                version: 1,
            },
            message_event: EventDraft {
                event_id: "conversation-final-recovery-user-message".to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "Conversation".to_owned(),
                entity_id: CONVERSATION.to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                entity_revision: 2,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: TURN.to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.message.added.v1".to_owned(),
                payload: serde_json::json!({
                    "message_id": USER_MESSAGE,
                    "conversation_id": CONVERSATION,
                    "author": user_author,
                    "content_digest": user_content_digest,
                    "resource_refs": resource_refs
                }),
                recorded_at: NOW.to_owned(),
            },
            turn_event: EventDraft {
                event_id: "conversation-final-recovery-turn-created".to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "ConversationTurn".to_owned(),
                entity_id: TURN.to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                entity_revision: 1,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: TURN.to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.turn.created.v2".to_owned(),
                payload: serde_json::json!({
                    "turn_id": TURN,
                    "conversation_id": CONVERSATION,
                    "user_message_id": USER_MESSAGE,
                    "agent_binding_id": BINDING,
                    "aggregate_version": 1,
                    "presentation_preference": "AUTO"
                }),
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("USER message, OPEN turn and dispatch intent commit together");

    let connection = Connection::open(&database).expect("open activated Conversation state");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enforce production foreign keys for the activated session");
    connection
        .execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,conversation_id,conversation_turn_id,task_id,task_spec_revision,attempt_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,configuration_digest,harness_descriptor_digest,status,started_at,last_event_at,closed_at,version)
             VALUES (?1,?2,'CONVERSATION',?3,?4,NULL,NULL,NULL,?5,?6,?7,?8,NULL,NULL,'ACTIVE',?9,?9,NULL,2)",
            params![SESSION, WORKSPACE, CONVERSATION, TURN, BINDING, ENDPOINT, RUNTIME_ID, OLD_INCARNATION_ID, NOW],
        )
        .expect("persist the active AgentSession from the prior Runtime incarnation");
    connection
        .execute(
            "UPDATE conversation_turns SET agent_session_id=?1,status='RUNNING',version=2 WHERE turn_id=?2 AND status='OPEN'",
            params![SESSION, TURN],
        )
        .expect("persist the activated RUNNING turn/session pointer");
    connection
        .execute(
            "UPDATE conversation_dispatch_intents SET state='CLAIMED',claimed_by_runtime_incarnation=?1,claimed_at=?2,version=version+1 WHERE turn_id=?3 AND retry_ordinal=0 AND state='UNCLAIMED'",
            params![OLD_INCARNATION_ID, NOW, TURN],
        )
        .expect("claim the dispatch intent under the prior Runtime incarnation");
    connection
        .execute(
            "INSERT INTO agent_session_host_bindings(agent_session_id,host_instance_id,native_session_ref,bound_at)
             VALUES (?1,?2,NULL,?3)",
            params![SESSION, HOST, NOW],
        )
        .expect("attach the active session to its prior-incarnation host");
    drop(connection);

    let answer = serde_json::json!([{"kind":"TEXT","text":ANSWER}]);
    let final_commit = || CommitConversationAgentResponse {
        request: WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: RESPONSE_REQUEST.to_owned(),
            request_payload: serde_json::json!({
                "turn_id": TURN,
                "retry_ordinal": 0,
                "provider_response_id": RESPONSE_ID
            }),
        },
        principal_id: OWNER.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        conversation_id: CONVERSATION.to_owned(),
        turn_id: TURN.to_owned(),
        agent_session_id: SESSION.to_owned(),
        expected_turn_version: 2,
        expected_session_version: 2,
        retry_ordinal: 0,
        provider_response_id: RESPONSE_ID.to_owned(),
        turn_status: ConversationTurnStatus::Completed,
        message: ConversationMessageRecord {
            message_id: AGENT_MESSAGE.to_owned(),
            conversation_id: CONVERSATION.to_owned(),
            author: serde_json::json!({"kind":"AGENT","agent_binding_id":BINDING}),
            role: "AGENT".to_owned(),
            agent_session_id: Some(SESSION.to_owned()),
            agent_binding_id: Some(BINDING.to_owned()),
            turn_id: Some(TURN.to_owned()),
            content: answer.clone(),
            resource_refs: serde_json::json!([]),
            source_channel_ref: None,
            created_at: COMMITTED_AT.to_owned(),
        },
        usage: None,
        message_event: EventDraft {
            event_id: "conversation-final-recovery-agent-message".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "Conversation".to_owned(),
            entity_id: CONVERSATION.to_owned(),
            origin_runtime_id: RUNTIME_ID.to_owned(),
            entity_revision: 3,
            hlc_timestamp: COMMITTED_AT.to_owned(),
            correlation_id: TURN.to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.message.added.v1".to_owned(),
            payload: serde_json::json!({
                "conversation_id": CONVERSATION,
                "message_id": AGENT_MESSAGE,
                "role": "AGENT"
            }),
            recorded_at: COMMITTED_AT.to_owned(),
        },
        turn_event: EventDraft {
            event_id: "conversation-final-recovery-turn-settled".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ConversationTurn".to_owned(),
            entity_id: TURN.to_owned(),
            origin_runtime_id: RUNTIME_ID.to_owned(),
            entity_revision: 3,
            hlc_timestamp: COMMITTED_AT.to_owned(),
            correlation_id: TURN.to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.turn.settled.v1".to_owned(),
            payload: serde_json::json!({
                "turn_id": TURN,
                "from": "RUNNING",
                "to": "COMPLETED",
                "aggregate_version": 3
            }),
            recorded_at: COMMITTED_AT.to_owned(),
        },
        session_event: EventDraft {
            event_id: "conversation-final-recovery-session-closed".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "AgentSession".to_owned(),
            entity_id: SESSION.to_owned(),
            origin_runtime_id: RUNTIME_ID.to_owned(),
            entity_revision: 3,
            hlc_timestamp: COMMITTED_AT.to_owned(),
            correlation_id: TURN.to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "agent.session.closed.v1".to_owned(),
            payload: serde_json::json!({
                "agent_session_id": SESSION,
                "from": "ACTIVE",
                "to": "CLOSED",
                "aggregate_version": 3
            }),
            recorded_at: COMMITTED_AT.to_owned(),
        },
    };
    let committed = conversation_store
        .commit_conversation_agent_response(final_commit())
        .expect("final response and request receipt commit atomically");
    assert_eq!(committed.message.message_id, AGENT_MESSAGE);
    assert_eq!(committed.message.content, answer);
    assert_eq!(committed.turn.status, ConversationTurnStatus::Completed);
    assert_eq!(committed.session.status, "CLOSED");

    let durable_state = |connection: &Connection| {
        connection
            .query_row(
                "SELECT
                    (SELECT status FROM conversation_turns WHERE turn_id=?1),
                    (SELECT settled_at FROM conversation_turns WHERE turn_id=?1),
                    (SELECT content_json FROM conversation_messages WHERE message_id=?2),
                    (SELECT COUNT(*) FROM conversation_messages WHERE turn_id=?1 AND role='AGENT'),
                    (SELECT provider_response_id FROM conversation_response_commits WHERE turn_id=?1 AND retry_ordinal=0),
                    (SELECT state FROM conversation_dispatch_intents WHERE turn_id=?1 AND retry_ordinal=0),
                    (SELECT status FROM agent_sessions WHERE agent_session_id=?3),
                    (SELECT response_json FROM request_dedup WHERE principal_id=?4 AND request_id=?5)",
                params![TURN, AGENT_MESSAGE, SESSION, OWNER, RESPONSE_REQUEST],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .expect("read committed Conversation recovery state")
    };
    let connection = Connection::open(&database).expect("read committed response before restart");
    let before_recovery = durable_state(&connection);
    assert_eq!(before_recovery.0, "COMPLETED");
    assert_eq!(
        before_recovery.2,
        serde_json::to_string(&answer).expect("serialize answer")
    );
    assert_eq!(before_recovery.3, 1);
    assert_eq!(before_recovery.4, RESPONSE_ID);
    assert_eq!(before_recovery.5, "SETTLED");
    assert_eq!(before_recovery.6, "CLOSED");
    drop(connection);

    let next = RuntimeLocalState::begin(directory.path()).expect("begin after unclean exit");
    assert_ne!(next.local_incarnation_id, OLD_INCARNATION_ID);
    assert!(next.recovered_from_unclean_shutdown);
    persist_state(directory.path(), &next).expect("persist restarted Runtime incarnation");
    register_from_bootstrap(&store, &next);

    let report = crate::agents::conversation_recovery::recover_conversation_sessions(
        &store, RUNTIME_ID, OWNER,
    );
    assert_eq!(
        report,
        Default::default(),
        "settled response is outside recovery scope"
    );

    let connection = Connection::open(&database).expect("read response after restart sweep");
    assert_eq!(
        durable_state(&connection),
        before_recovery,
        "recovery must preserve the committed response, receipt, and terminal state"
    );
    drop(connection);

    let replayed = conversation_store
        .commit_conversation_agent_response(final_commit())
        .expect("the exact response receipt remains replayable after recovery");
    assert_eq!(replayed.message.message_id, committed.message.message_id);
    assert_eq!(replayed.message.content, committed.message.content);
    assert_eq!(replayed.turn.status, ConversationTurnStatus::Completed);
    assert_eq!(replayed.session.status, "CLOSED");
}

#[test]
fn conversation_session_recovery_reports_workspace_enumeration_failure() {
    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    let store = store(directory.path());
    let database = directory.path().join("litecowork.sqlite3");
    let connection = Connection::open(&database).expect("open recovery fixture database");
    connection
        .execute(
            "ALTER TABLE workspaces RENAME COLUMN workspace_id TO unavailable_workspace_id",
            [],
        )
        .expect("make Workspace catalog enumeration fail without damaging recovery tables");
    drop(connection);

    assert!(
        store.list_workspaces().is_err(),
        "fixture must exercise the failed Workspace enumeration"
    );

    let report = crate::agents::conversation_recovery::recover_conversation_sessions(
        &store,
        RUNTIME_ID,
        "principal-integrated-recovery-test",
    );

    assert_eq!(report.failures, 1);
}

#[test]
fn startup_workspace_enumeration_failure_persists_recovery_blockers() {
    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    ensure_private_state_directory(directory.path()).expect("restrict Runtime data directory");
    let store = store(directory.path());
    let database = directory.path().join("litecowork.sqlite3");
    let connection = Connection::open(&database).expect("open recovery fixture database");
    connection
        .execute(
            "ALTER TABLE workspaces RENAME COLUMN workspace_id TO unavailable_workspace_id",
            [],
        )
        .expect("make Workspace catalog enumeration fail without damaging recovery tables");
    drop(connection);

    assert!(
        store.list_workspaces().is_err(),
        "fixture must exercise the failed Workspace enumeration"
    );

    let mut state = local_state("startup-workspace-enumeration-failure", false);
    persist_state(directory.path(), &state).expect("persist starting Runtime state");
    let blockers = super::recover_and_persist_startup_blockers(
        directory.path(),
        &store,
        "principal-integrated-recovery-test",
        &mut state,
        Vec::new(),
    )
    .expect("recovery failures are durably projected before Operator admission");

    let blocker_codes = blockers
        .iter()
        .map(|blocker| blocker.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        blocker_codes,
        [
            "EXPIRED_EXECUTION_LEASE_RECOVERY_FAILED",
            "CONVERSATION_SESSION_RECOVERY_FAILED"
        ]
    );
    assert_eq!(state.recovery_state, RuntimeState::Degraded);
    assert_eq!(state.blockers, blocker_codes);

    let persisted = super::read_state(&directory.path().join("runtime-state.json"))
        .expect("read private Runtime state")
        .expect("startup recovery state is persisted");
    assert_eq!(persisted.recovery_state, RuntimeState::Degraded);
    assert_eq!(persisted.blockers, blocker_codes);
}
