use super::{create_workspace, state_database, test_store};
use domain_workspace::{ArchiveWorkspace, EventContext, WorkspaceService};
use rusqlite::Connection;
use std::time::Duration;
use storage_core::{
    EventDraft, EventStore, IdempotentWorkspaceStore, StateStore, StoreError,
    WorkspaceArchiveBlockerKind, WorkspaceCreateRequest,
};

const WORKSPACE_ID: &str = "workspace-archive";
const NOW: &str = "2026-10-11T12:00:00Z";

fn archive(
    store: &super::SqliteWorkspaceStore,
    expected_version: u64,
    request_id: &str,
) -> Result<storage_core::CommittedWorkspace, StoreError> {
    WorkspaceService::new(store.clone()).archive_idempotent(
        ArchiveWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            expected_version,
            principal_id: "owner-local".to_owned(),
            event: EventContext {
                event_id: format!("event-{request_id}"),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: format!("correlation-{request_id}"),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        },
        request_id.to_owned(),
        serde_json::json!({
            "operation": "workspace.archive.v1",
            "workspace_id": WORKSPACE_ID,
            "expected_version": expected_version,
        }),
    )
}

#[test]
fn workspace_archive_is_atomic_terminal_and_exactly_replayable() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);

    let first = archive(&store, 1, "workspace-archive-1").expect("quiescent Workspace archives");
    assert_eq!(first.workspace.status, "ARCHIVED");
    assert_eq!(first.workspace.version, 2);
    assert_eq!(first.event.event_type, "workspace.archived.v1");
    assert_eq!(
        first.event.payload["archived_by"]["principal_id"],
        "owner-local"
    );

    let replay = archive(&store, 1, "workspace-archive-1")
        .expect("exact retry returns original archive receipt");
    assert_eq!(replay, first);

    let changed_payload = WorkspaceService::new(store.clone()).archive_idempotent(
        ArchiveWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            expected_version: 1,
            principal_id: "owner-local".to_owned(),
            event: EventContext {
                event_id: "event-changed-archive-payload".to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-changed-archive-payload".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        },
        "workspace-archive-1".to_owned(),
        serde_json::json!({"operation":"workspace.archive.v1","workspace_id":WORKSPACE_ID,"expected_version":1,"changed":true}),
    );
    assert!(matches!(changed_payload, Err(StoreError::Conflict { .. })));
    assert!(matches!(
        archive(&store, 2, "workspace-archive-2"),
        Err(StoreError::Invalid(_))
    ));

    let current = store
        .get_workspace(WORKSPACE_ID)
        .expect("Workspace read succeeds")
        .expect("Workspace remains present");
    assert_eq!(current.status, "ARCHIVED");
    assert_eq!(current.version, 2);
    let events = store
        .read_workspace_events(WORKSPACE_ID)
        .expect("event history reads");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "workspace.archived.v1")
            .count(),
        1
    );
}

#[test]
fn workspace_archive_rejects_nonterminal_task_without_any_archive_write() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable fixture-only foreign key validation");
    connection
        .execute(
            "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version)
             VALUES('task-active-archive',?1,1,'RUNNING','binding-fixture','NORMAL','{}',?2,?2,1)",
            rusqlite::params![WORKSPACE_ID, NOW],
        )
        .expect("seed nonterminal Task blocker");
    drop(connection);

    let result = archive(&store, 1, "workspace-archive-blocked");
    assert!(matches!(
        result,
        Err(StoreError::WorkspaceNotQuiescent { blockers })
            if blockers == vec![storage_core::WorkspaceArchiveBlocker {
                kind: WorkspaceArchiveBlockerKind::NonterminalTasks,
                count: 1,
            }]
    ));

    let current = store
        .get_workspace(WORKSPACE_ID)
        .expect("Workspace read succeeds")
        .expect("Workspace remains present");
    assert_eq!(current.status, "ACTIVE");
    assert_eq!(current.version, 1);
    assert!(
        store
            .read_workspace_events(WORKSPACE_ID)
            .expect("event history reads")
            .iter()
            .all(|event| event.event_type != "workspace.archived.v1")
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect receipts");
    let receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id='workspace-archive-blocked'",
            [],
            |row| row.get(0),
        )
        .expect("count archive receipts");
    assert_eq!(receipts, 0);
}

#[test]
fn generic_workspace_commit_cannot_archive_with_another_event_type() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    let mut workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("Workspace read succeeds")
        .expect("Workspace exists");
    workspace.status = "ARCHIVED".to_owned();
    workspace.version = 2;
    workspace.updated_at = NOW.to_owned();
    let event = EventDraft {
        event_id: "event-wrong-archive".to_owned(),
        workspace_id: WORKSPACE_ID.to_owned(),
        entity_type: "Workspace".to_owned(),
        entity_id: WORKSPACE_ID.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: 2,
        hlc_timestamp: NOW.to_owned(),
        correlation_id: "correlation-wrong-archive".to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "workspace.replication_policy.changed.v1".to_owned(),
        payload: serde_json::json!({
            "workspace_id": WORKSPACE_ID,
            "archived_by": {"kind": "USER", "principal_id": "owner-local"},
            "aggregate_version": 2,
        }),
        recorded_at: NOW.to_owned(),
    };
    let result = store.commit_workspace_idempotent(
        WorkspaceCreateRequest {
            principal_id: "owner-local".to_owned(),
            request_id: "workspace-archive-wrong-event".to_owned(),
            request_payload: serde_json::json!({
                "operation": "workspace.archive.v1",
                "workspace_id": WORKSPACE_ID,
                "expected_version": 1,
            }),
        },
        1,
        workspace,
        event,
    );
    assert!(matches!(result, Err(StoreError::Invalid(_))));
    assert_eq!(
        store
            .get_workspace(WORKSPACE_ID)
            .expect("Workspace reads")
            .expect("Workspace stays present")
            .status,
        "ACTIVE"
    );
}

#[test]
fn storage_archive_rejects_request_payload_that_does_not_match_the_transition() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    let mut workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("Workspace read succeeds")
        .expect("Workspace exists");
    workspace.status = "ARCHIVED".to_owned();
    workspace.version = 2;
    workspace.updated_at = NOW.to_owned();
    let event = EventDraft {
        event_id: "event-mismatched-archive-request".to_owned(),
        workspace_id: WORKSPACE_ID.to_owned(),
        entity_type: "Workspace".to_owned(),
        entity_id: WORKSPACE_ID.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: 2,
        hlc_timestamp: NOW.to_owned(),
        correlation_id: "correlation-mismatched-archive-request".to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "workspace.archived.v1".to_owned(),
        payload: serde_json::json!({
            "workspace_id": WORKSPACE_ID,
            "archived_by": {"kind": "USER", "principal_id": "owner-local"},
            "aggregate_version": 2,
        }),
        recorded_at: NOW.to_owned(),
    };
    let result = store.commit_workspace_idempotent(
        WorkspaceCreateRequest {
            principal_id: "owner-local".to_owned(),
            request_id: "workspace-archive-mismatched-request".to_owned(),
            request_payload: serde_json::json!({
                "operation": "workspace.replication_policy.update.v1",
                "workspace_id": WORKSPACE_ID,
                "expected_version": 1,
            }),
        },
        1,
        workspace,
        event,
    );
    assert!(matches!(result, Err(StoreError::Invalid(_))));
    assert_eq!(
        store
            .get_workspace(WORKSPACE_ID)
            .expect("Workspace reads")
            .expect("Workspace remains present")
            .status,
        "ACTIVE"
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect receipts");
    let receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id='workspace-archive-mismatched-request'",
            [],
            |row| row.get(0),
        )
        .expect("count archive receipts");
    assert_eq!(receipts, 0);
}

#[test]
fn disabled_automation_with_persisted_trigger_cursor_blocks_workspace_archive() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TRIGGER automation_cursor_requires_active_trigger_host_insert;",
        )
        .expect("disable fixture-only cursor host validation");
    connection
        .execute(
            "INSERT INTO automations(automation_id,workspace_id,name,current_revision,status,created_at,updated_at,version)
             VALUES('automation-disabled-archive',?1,'Disabled fixture',1,'DISABLED',?2,?2,2)",
            rusqlite::params![WORKSPACE_ID, NOW],
        )
        .expect("seed disabled Automation");
    connection
        .execute(
            "INSERT INTO automation_cursors(workspace_id,automation_id,active_automation_revision,trigger_id,trigger_host_runtime_id,host_epoch,cursor_digest,last_seen_digest,last_observation_ref_json,next_scheduled_at,last_checked_at,observation_gap_since,version)
             VALUES(?1,'automation-disabled-archive',1,'manual','runtime-fixture',1,?2,NULL,NULL,NULL,?3,NULL,1)",
            rusqlite::params![WORKSPACE_ID, format!("sha256:{}", "b".repeat(64)), NOW],
        )
        .expect("seed persisted TriggerHost ownership record");
    drop(connection);

    assert!(matches!(
        archive(&store, 1, "workspace-archive-disabled-trigger"),
        Err(StoreError::WorkspaceNotQuiescent { blockers })
            if blockers == vec![storage_core::WorkspaceArchiveBlocker {
                kind: WorkspaceArchiveBlockerKind::ActiveAutomationTriggers,
                count: 1,
            }]
    ));
}

#[test]
fn active_channel_host_blocks_workspace_archive() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TRIGGER channel_host_assignment_requires_active_binding_insert;",
        )
        .expect("disable fixture-only channel admission validation");
    connection
        .execute(
            "INSERT INTO channel_host_assignments(channel_binding_id,workspace_id,runtime_id,host_epoch,status,assigned_at) VALUES('channel-host-archive',?1,'runtime-fixture',1,'ACTIVE',?2)",
            rusqlite::params![WORKSPACE_ID, NOW],
        )
        .expect("seed active host blocker");
    drop(connection);

    assert!(matches!(
        archive(&store, 1, "workspace-archive-channel-host"),
        Err(StoreError::WorkspaceNotQuiescent { blockers })
            if blockers == vec![storage_core::WorkspaceArchiveBlocker {
                kind: WorkspaceArchiveBlockerKind::ActiveChannelHosts,
                count: 1,
            }]
    ));
}

#[test]
fn observed_effect_blocks_workspace_archive_until_verification_or_failure() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TRIGGER effect_invocation_scope_guard;
             DROP TRIGGER effect_transition_guard;",
        )
        .expect("disable fixture-only Effect provenance validation");
    connection
        .execute(
            "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version)
             VALUES('task-terminal-effect-archive',?1,1,'COMPLETED','binding-fixture','NORMAL','{}',?2,?2,1)",
            rusqlite::params![WORKSPACE_ID, NOW],
        )
        .expect("seed terminal Task");
    connection
        .execute(
            "INSERT INTO effects(effect_id,task_id,attempt_id,operation,target_json,state,request_digest,capability_invocation_id,execution_method,created_at,updated_at,version)
             VALUES('effect-observed-archive','task-terminal-effect-archive','attempt-fixture','write','{}','PROPOSED',?1,'invocation-fixture','UNKNOWN',?2,?2,1)",
            rusqlite::params![format!("sha256:{}", "a".repeat(64)), NOW],
        )
        .expect("seed proposed Effect");
    connection
        .execute(
            "UPDATE effects SET state='OBSERVED', version=2 WHERE effect_id='effect-observed-archive'",
            [],
        )
        .expect("seed observed but unverified Effect state");
    drop(connection);

    assert!(matches!(
        archive(&store, 1, "workspace-archive-observed-effect"),
        Err(StoreError::WorkspaceNotQuiescent { blockers })
            if blockers == vec![storage_core::WorkspaceArchiveBlocker {
                kind: WorkspaceArchiveBlockerKind::UnresolvedEffects,
                count: 1,
            }]
    ));
}

#[test]
fn active_capability_activation_blocks_workspace_archive() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable fixture-only foreign key validation");
    connection
        .execute(
            "INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,conversation_id,task_id,attempt_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at,version)
             VALUES('activation-active-archive',?1,'TASK_PLANNING',NULL,'task-fixture',NULL,'{}','runtime-fixture','incarnation-fixture','STRUCTURED','ACTIVE','HEALTHY',?2,?2,1)",
            rusqlite::params![WORKSPACE_ID, NOW],
        )
        .expect("seed active CapabilityActivation");
    drop(connection);

    assert!(matches!(
        archive(&store, 1, "workspace-archive-active-activation"),
        Err(StoreError::WorkspaceNotQuiescent { blockers })
            if blockers == vec![storage_core::WorkspaceArchiveBlocker {
                kind: WorkspaceArchiveBlockerKind::ActiveCapabilityActivations,
                count: 1,
            }]
    ));
}
