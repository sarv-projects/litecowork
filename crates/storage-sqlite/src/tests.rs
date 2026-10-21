#[path = "conversation_turn_tests.rs"]
mod conversation_turn_tests;
#[path = "invocation_tests.rs"]
mod invocation_tests;
#[path = "needs_you_tests.rs"]
mod needs_you_tests;
#[path = "resource_upload_gc_tests.rs"]
mod resource_upload_gc_tests;
#[path = "rich_presentation_tests.rs"]
mod rich_presentation_tests;
#[path = "runtime_binding_tests.rs"]
mod runtime_binding_tests;
#[path = "runtime_capability_host_tests.rs"]
mod runtime_capability_host_tests;
#[path = "runtime_lifecycle_tests.rs"]
mod runtime_lifecycle_tests;
#[path = "runtime_offer_tests.rs"]
mod runtime_offer_tests;
#[path = "task_lead_handoff_tests.rs"]
mod task_lead_handoff_tests;
#[path = "workspace_archive_tests.rs"]
mod workspace_archive_tests;
#[path = "workspace_instruction_tests.rs"]
mod workspace_instruction_tests;
#[path = "writer_shutdown_tests.rs"]
mod writer_shutdown_tests;
use super::*;
#[path = "artifact_tests.rs"]
mod artifact_tests;
#[path = "automation_host_tests.rs"]
mod automation_host_tests;
#[path = "coworker_tests.rs"]
mod coworker_tests;
#[path = "delegation_profile_tests.rs"]
mod delegation_profile_tests;
#[path = "environment_tests.rs"]
mod environment_tests;
#[path = "evidence_run_binding_migration_tests.rs"]
mod evidence_run_binding_migration_tests;
#[path = "goal_tests.rs"]
mod goal_tests;
#[path = "routine_tests.rs"]
mod routine_tests;
use domain_workspace::{
    ChangeReplicationPolicy, CreateWorkspace, EventContext, ResourceService,
    SetContextDocumentStatus, WorkspaceService,
};
use rusqlite::Connection;
use std::sync::Barrier;
use storage_core::{
    ActivateTaskPlanningSession, AgentSessionRecord, AggregateStateRef, BlobPurpose, BlobRef,
    CommittedWorkspace, DispatchAdmissionBlocker, EffectEvidenceEventContext, EffectEvidenceStore,
    EffectTransitionMetadata, EventDraft, ReplicationPolicy, StoreError, TaskPlanningSessionStart,
    TransitionEffectCommit, Workspace,
};
use zeroize::Zeroizing;

#[test]
fn shared_idempotency_key_bound_is_200_printable_ascii_bytes() {
    use super::valid_shared_idempotency_key;

    assert!(valid_shared_idempotency_key("k"));
    assert!(valid_shared_idempotency_key(&"k".repeat(128)));
    assert!(valid_shared_idempotency_key(&"k".repeat(129)));
    assert!(valid_shared_idempotency_key(&"k".repeat(200)));
    assert!(!valid_shared_idempotency_key(""));
    assert!(!valid_shared_idempotency_key(&"k".repeat(201)));
    assert!(!valid_shared_idempotency_key("has space"));
    assert!(!valid_shared_idempotency_key("é"));
}

#[test]
fn workspace_root_status_receipt_replays_exact_payload_and_rejects_key_reuse() {
    use storage_core::{
        AggregateStateRef, BlobRef, CommittedWorkspaceRootStatus, DomainEvent,
        WorkspaceCreateRequest, WorkspaceRootRecord,
    };

    let connection = Connection::open_in_memory().expect("in-memory SQLite");
    connection
        .execute_batch(
            "CREATE TABLE request_dedup(
               principal_id TEXT NOT NULL,
               request_id TEXT NOT NULL,
               request_digest TEXT NOT NULL,
               response_json TEXT,
               response_digest TEXT,
               created_at TEXT NOT NULL,
               expires_at TEXT,
               PRIMARY KEY(principal_id, request_id)
             );",
        )
        .expect("idempotency receipt fixture");

    let request_payload = json!({
        "operation": "workspace.root.pause.v1",
        "workspace_id": "workspace-root-receipt",
        "workspace_root_id": "root-receipt",
        "expected_version": 1,
    });
    let committed = CommittedWorkspaceRootStatus {
        root: WorkspaceRootRecord {
            workspace_root_id: "root-receipt".to_owned(),
            workspace_id: "workspace-root-receipt".to_owned(),
            resource_id: "resource-root-receipt".to_owned(),
            location_id: "location-root-receipt".to_owned(),
            display_name: "root".to_owned(),
            watch_policy: "METADATA".to_owned(),
            replication_policy: "NONE".to_owned(),
            status: "PAUSED".to_owned(),
            added_by: json!({"kind":"USER","principal_id":"owner"}),
            created_at: "2026-10-15T10:00:00Z".to_owned(),
            updated_at: "2026-10-15T10:01:00Z".to_owned(),
            version: 2,
        },
        location_availability: Some("AVAILABLE".to_owned()),
        event: DomainEvent {
            event_id: "event-root-receipt".to_owned(),
            workspace_id: "workspace-root-receipt".to_owned(),
            entity_type: "WorkspaceRoot".to_owned(),
            entity_id: "root-receipt".to_owned(),
            origin_runtime_id: "runtime-root-receipt".to_owned(),
            origin_sequence: 1,
            entity_revision: 2,
            hlc_timestamp: "2026-10-15T10:01:00Z".to_owned(),
            correlation_id: "correlation-root-receipt".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "workspace.root.status.changed.v1".to_owned(),
            payload: json!({"workspace_root_id":"root-receipt","from":"ACTIVE","to":"PAUSED","reason_code":"USER_PAUSED","aggregate_version":2}),
            aggregate_state_ref: AggregateStateRef {
                blob: BlobRef {
                    digest: format!("sha256:{}", "a".repeat(64)),
                    size_bytes: 1,
                    media_type: "application/vnd.litecowork.workspace-root+json".to_owned(),
                },
                entity_revision: 2,
                record_schema_version: 1,
            },
            recorded_at: "2026-10-15T10:01:00Z".to_owned(),
            payload_digest: format!("sha256:{}", "b".repeat(64)),
        },
    };
    let response_json = String::from_utf8(canonical_json(&committed).expect("canonical response"))
        .expect("response JSON is UTF-8");
    connection
        .execute(
            "INSERT INTO request_dedup(
               principal_id, request_id, request_digest, response_json,
               response_digest, created_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            rusqlite::params![
                "owner",
                "root-status-key",
                digest(&canonical_json(&request_payload).expect("canonical request")),
                response_json,
                digest(response_json.as_bytes()),
                "2026-10-15T10:01:00Z",
            ],
        )
        .expect("committed receipt fixture");

    let request = WorkspaceCreateRequest {
        principal_id: "owner".to_owned(),
        request_id: "root-status-key".to_owned(),
        request_payload: request_payload.clone(),
    };
    assert_eq!(
        get_workspace_root_status_receipt(&connection, &request)
            .expect("exact payload replays")
            .expect("receipt exists"),
        committed,
    );

    let changed_request = WorkspaceCreateRequest {
        request_payload: json!({
            "operation": "workspace.root.pause.v1",
            "workspace_id": "workspace-root-receipt",
            "workspace_root_id": "root-receipt",
            "expected_version": 2,
        }),
        ..request
    };
    let error = get_workspace_root_status_receipt(&connection, &changed_request)
        .expect_err("changed payload cannot replay the receipt");
    assert_eq!(
        error.to_string(),
        "idempotency key was reused with a different request payload"
    );
}

#[test]
fn workspace_root_status_transaction_rejects_changed_digest_at_commit_boundary() {
    use storage_core::{
        AggregateStateRef, BlobRef, EventDraft, WorkspaceCreateRequest, WorkspaceRootRecord,
        WorkspaceRootStatusAction, WorkspaceRootStatusCommit,
    };

    let mut connection = Connection::open_in_memory().expect("in-memory SQLite");
    connection
        .execute_batch(
            "CREATE TABLE request_dedup(
               principal_id TEXT NOT NULL,
               request_id TEXT NOT NULL,
               request_digest TEXT NOT NULL,
               response_json TEXT,
               response_digest TEXT,
               created_at TEXT NOT NULL,
               expires_at TEXT,
               PRIMARY KEY(principal_id, request_id)
             );",
        )
        .expect("idempotency receipt fixture");

    let request_payload = json!({
        "operation": "workspace.root.pause.v1",
        "workspace_id": "workspace-root-transaction-receipt",
        "workspace_root_id": "root-transaction-receipt",
        "expected_version": 1,
    });
    let prior_payload = json!({
        "operation": "workspace.root.pause.v1",
        "workspace_id": "workspace-root-transaction-receipt",
        "workspace_root_id": "root-transaction-receipt",
        "expected_version": 2,
    });
    connection
        .execute(
            "INSERT INTO request_dedup(
               principal_id, request_id, request_digest, response_json,
               response_digest, created_at, expires_at
             ) VALUES (?1, ?2, ?3, NULL, NULL, ?4, NULL)",
            rusqlite::params![
                "owner",
                "root-status-transaction-key",
                digest(&canonical_json(&prior_payload).expect("canonical prior request")),
                "2026-10-15T10:01:00Z",
            ],
        )
        .expect("seed the concurrent request's receipt");

    let commit = WorkspaceRootStatusCommit {
        request: WorkspaceCreateRequest {
            principal_id: "owner".to_owned(),
            request_id: "root-status-transaction-key".to_owned(),
            request_payload,
        },
        expected_version: 1,
        action: WorkspaceRootStatusAction::Pause,
        runtime_id: None,
        runtime_incarnation_id: None,
        root: WorkspaceRootRecord {
            workspace_root_id: "root-transaction-receipt".to_owned(),
            workspace_id: "workspace-root-transaction-receipt".to_owned(),
            resource_id: "resource-transaction-receipt".to_owned(),
            location_id: "location-transaction-receipt".to_owned(),
            display_name: "root".to_owned(),
            watch_policy: "METADATA".to_owned(),
            replication_policy: "NONE".to_owned(),
            status: "PAUSED".to_owned(),
            added_by: json!({"kind":"USER","principal_id":"owner"}),
            created_at: "2026-10-15T10:00:00Z".to_owned(),
            updated_at: "2026-10-15T10:01:00Z".to_owned(),
            version: 2,
        },
        event: EventDraft {
            event_id: "event-root-transaction-receipt".to_owned(),
            workspace_id: "workspace-root-transaction-receipt".to_owned(),
            entity_type: "WorkspaceRoot".to_owned(),
            entity_id: "root-transaction-receipt".to_owned(),
            origin_runtime_id: "runtime-root-transaction-receipt".to_owned(),
            entity_revision: 2,
            hlc_timestamp: "2026-10-15T10:01:00Z".to_owned(),
            correlation_id: "correlation-root-transaction-receipt".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "workspace.root.status.changed.v1".to_owned(),
            payload: json!({
                "workspace_root_id": "root-transaction-receipt",
                "from": "ACTIVE",
                "to": "PAUSED",
                "reason_code": "USER_PAUSED",
                "aggregate_version": 2,
            }),
            recorded_at: "2026-10-15T10:01:00Z".to_owned(),
        },
    };
    let root_state_ref = AggregateStateRef {
        blob: BlobRef {
            digest: format!("sha256:{}", "a".repeat(64)),
            size_bytes: 1,
            media_type: "application/vnd.litecowork.workspace-root+json".to_owned(),
        },
        entity_revision: 2,
        record_schema_version: 1,
    };

    let error = update_workspace_root_status_transaction(&mut connection, commit, root_state_ref)
        .expect_err("transactional receipt recheck rejects a changed request digest");
    assert!(matches!(error, StoreError::IdempotencyConflict));
}

#[test]
fn automation_occurrence_aggregate_version_is_not_claim_fencing() {
    let connection = Connection::open_in_memory().expect("in-memory SQLite");
    connection.execute_batch(
        "CREATE TABLE automation_occurrences(workspace_id TEXT NOT NULL, occurrence_id TEXT PRIMARY KEY);
         CREATE TABLE domain_events(workspace_id TEXT, entity_type TEXT, entity_id TEXT, entity_revision INTEGER);
         INSERT INTO automation_occurrences VALUES('workspace', 'existing');
         INSERT INTO domain_events VALUES('workspace', 'AutomationOccurrence', 'existing', 4);",
    ).expect("legacy occurrence fixture");
    connection
        .execute_batch(include_str!("../../../docs/schemas/sqlite-v11.sql"))
        .expect("v11 aggregate revision migration");
    let migrated: i64 = connection
        .query_row(
            "SELECT version FROM automation_occurrences WHERE occurrence_id = 'existing'",
            [],
            |row| row.get(0),
        )
        .expect("migrated occurrence version");
    assert_eq!(migrated, 4);
    connection
        .execute(
            "UPDATE automation_occurrences SET version = 5 WHERE occurrence_id = 'existing'",
            [],
        )
        .expect("one aggregate transition increments exactly once");
    assert!(
        connection
            .execute(
                "UPDATE automation_occurrences SET version = 5 WHERE occurrence_id = 'existing'",
                []
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE automation_occurrences SET version = 7 WHERE occurrence_id = 'existing'",
                []
            )
            .is_err()
    );
    connection.execute("INSERT INTO automation_occurrences(workspace_id, occurrence_id) VALUES('workspace', 'new')", [])
        .expect("new aggregate begins with default version one");
    assert!(
        connection
            .execute(
                "UPDATE automation_occurrences SET version = 3 WHERE occurrence_id = 'new'",
                []
            )
            .is_err()
    );
}
#[test]
fn effect_start_is_blocked_before_any_storage_mutation() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let database = state_database(&directory);
    let before = {
        let connection = Connection::open(&database).expect("open read-only check connection");
        (
            connection
                .query_row("SELECT COUNT(*) FROM effects", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count Effects"),
            connection
                .query_row("SELECT COUNT(*) FROM domain_events", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count events"),
            connection
                .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count idempotency receipts"),
        )
    };

    let error = SqliteEffectEvidenceStore::new(store.clone())
        .transition_effect(TransitionEffectCommit {
            workspace_id: "workspace".to_owned(),
            runtime_id: "runtime".to_owned(),
            runtime_incarnation_id: "incarnation".to_owned(),
            request_id: "start-effect".to_owned(),
            effect_id: "effect".to_owned(),
            expected_version: 1,
            next_state: domain_effects::EffectState::Started,
            fence: None,
            metadata: EffectTransitionMetadata {
                result_ref: None,
                observed_state: None,
                observation_evidence_ref: None,
                verification_ref: None,
                retry_authorization: None,
                failure_code: None,
                failure_retryable: None,
                failure_digest: None,
                ambiguity_reason_digest: None,
            },
            event: EffectEvidenceEventContext {
                event_id: "event".to_owned(),
                origin_runtime_id: "runtime".to_owned(),
                origin_runtime_incarnation_id: "incarnation".to_owned(),
                hlc_timestamp: "2026-10-09T10:00:00Z".to_owned(),
                correlation_id: "correlation".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-09T10:00:00Z".to_owned(),
            },
        })
        .expect_err("Effect start must fail closed until atomic dispatch admission exists");

    assert_eq!(
        error,
        StoreError::DispatchAdmissionUnavailable {
            blockers: vec![
                DispatchAdmissionBlocker::TrustDecisionUnavailable,
                DispatchAdmissionBlocker::InvocationTransitionWriterUnavailable,
                DispatchAdmissionBlocker::ApprovalUseConsumptionUnavailable,
            ],
        }
    );
    let after = {
        let connection = Connection::open(&database).expect("open read-only check connection");
        (
            connection
                .query_row("SELECT COUNT(*) FROM effects", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count Effects"),
            connection
                .query_row("SELECT COUNT(*) FROM domain_events", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count events"),
            connection
                .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count idempotency receipts"),
        )
    };
    assert_eq!(
        after, before,
        "blocked admission writes no rows or receipts"
    );
    let blob_root = directory.path().join("blobs");
    let blob_count = if blob_root.exists() {
        std::fs::read_dir(blob_root)
            .expect("read BlobStore root")
            .count()
    } else {
        0
    };
    assert_eq!(
        blob_count, 0,
        "blocked admission writes no aggregate-state blob"
    );
    drop(store);
}

#[test]
fn task_create_receipt_resolves_exact_replay_before_admission_state() {
    let connection = Connection::open_in_memory().expect("in-memory SQLite");
    connection
        .execute_batch(
            "CREATE TABLE request_dedup(
           principal_id TEXT NOT NULL, request_id TEXT NOT NULL, request_digest TEXT NOT NULL,
           response_json TEXT, response_digest TEXT, PRIMARY KEY(principal_id, request_id)
         );",
        )
        .expect("idempotency receipt fixture");
    let payload = json!({
        "workspace_id": "workspace",
        "automation_id": "automation",
        "automation_revision": 3,
        "expected_automation_version": 7,
        "trigger_id": "manual",
        "inputs": {"name": "value"},
    });
    let response = json!({"committed": "READY task"});
    let request_digest = digest(&canonical_json(&payload).expect("canonical request"));
    let response_json = String::from_utf8(canonical_json(&response).expect("canonical response"))
        .expect("response JSON is UTF-8");
    connection.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params!["owner", "request-1", request_digest, response_json, digest(response_json.as_bytes())],
    ).expect("commit receipt");

    assert_eq!(
        verify_request_receipt::<Value>(
            &connection,
            "owner",
            "request-1",
            &digest(&canonical_json(&payload).expect("canonical request")),
        )
        .expect("exact receipt resolves"),
        Some(response),
    );
    let changed = json!({"workspace_id": "workspace", "inputs": {"name": "changed"}});
    assert!(matches!(
        verify_request_receipt::<Value>(
            &connection,
            "owner",
            "request-1",
            &digest(&canonical_json(&changed).expect("canonical changed request")),
        ),
        Err(StoreError::Conflict { .. }),
    ));
    assert_eq!(
        verify_request_receipt::<Value>(
            &connection,
            "owner",
            "missing",
            &digest(&canonical_json(&payload).expect("canonical request")),
        )
        .expect("missing receipt is not a replay"),
        None,
    );
}

#[derive(Clone)]
struct TestKeys {
    key: [u8; 32],
}

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new(self.key),
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

fn test_store(directory: &tempfile::TempDir, busy_timeout: Duration) -> SqliteWorkspaceStore {
    let blob_store = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));
    SqliteWorkspaceStore::open(
        state_database(directory),
        blob_store,
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout,
        },
    )
    .expect("SQLite store opens")
}

#[test]
fn sqlite_planning_admission_fails_closed_before_persisting_a_starting_session() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let timestamp = "2026-10-09T10:00:00Z".to_owned();
    let session = AgentSessionRecord {
        agent_session_id: "planner-session".to_owned(),
        workspace_id: "workspace".to_owned(),
        scope_kind: "TASK_PLANNING".to_owned(),
        conversation_id: None,
        conversation_turn_id: None,
        task_id: Some("task".to_owned()),
        task_spec_revision: Some(1),
        attempt_id: None,
        agent_binding_id: "binding".to_owned(),
        endpoint_id: "endpoint".to_owned(),
        runtime_id: "runtime".to_owned(),
        runtime_incarnation_id: "incarnation".to_owned(),
        configuration_digest: None,
        harness_descriptor_digest: None,
        status: "STARTING".to_owned(),
        started_at: timestamp.clone(),
        last_event_at: Some(timestamp.clone()),
        closed_at: None,
        version: 1,
    };
    let start = TaskPlanningSessionStart {
        principal_id: "owner".to_owned(),
        request_id: "request".to_owned(),
        request_payload: json!({"task":"task"}),
        expected_task_version: 1,
        session,
        event: EventDraft {
            event_id: "event-start".to_owned(),
            workspace_id: "workspace".to_owned(),
            entity_type: "AgentSession".to_owned(),
            entity_id: "planner-session".to_owned(),
            origin_runtime_id: "runtime".to_owned(),
            entity_revision: 1,
            hlc_timestamp: timestamp.clone(),
            correlation_id: "correlation".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "agent.session.starting.v1".to_owned(),
            payload: json!({}),
            recorded_at: timestamp.clone(),
        },
    };

    assert_eq!(
        store.start_task_planning_session(start).unwrap_err(),
        StoreError::Invalid("TASK_PLANNING_ISOLATION_UNAVAILABLE".to_owned()),
    );

    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let sessions: i64 = connection
        .query_row("SELECT COUNT(*) FROM agent_sessions", [], |row| row.get(0))
        .expect("read session count");
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type = 'AgentSession'",
            [],
            |row| row.get(0),
        )
        .expect("read event count");
    assert_eq!(sessions, 0);
    assert_eq!(events, 0);
}

#[test]
fn sqlite_planning_activation_fails_closed_before_session_lookup_or_task_transition() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let timestamp = "2026-10-09T10:00:00Z".to_owned();

    assert_eq!(
        store
            .activate_task_planning_session(ActivateTaskPlanningSession {
                workspace_id: "workspace".to_owned(),
                agent_session_id: "planner-session".to_owned(),
                expected_session_version: 1,
                expected_task_version: 1,
                occurred_at: timestamp,
                host_instance_id: "host".to_owned(),
                native_session_ref: None,
                session_event: EventDraft {
                    event_id: "event-started".to_owned(),
                    workspace_id: "workspace".to_owned(),
                    entity_type: "AgentSession".to_owned(),
                    entity_id: "planner-session".to_owned(),
                    origin_runtime_id: "runtime".to_owned(),
                    entity_revision: 2,
                    hlc_timestamp: "2026-10-09T10:00:00Z".to_owned(),
                    correlation_id: "correlation".to_owned(),
                    causation_id: Some("event-starting".to_owned()),
                    schema_version: 1,
                    event_type: "agent.session.started.v1".to_owned(),
                    payload: json!({}),
                    recorded_at: "2026-10-09T10:00:00Z".to_owned(),
                },
                task_status_event: None,
            })
            .unwrap_err(),
        StoreError::Invalid("TASK_PLANNING_ISOLATION_UNAVAILABLE".to_owned()),
    );

    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let sessions: i64 = connection
        .query_row("SELECT COUNT(*) FROM agent_sessions", [], |row| row.get(0))
        .expect("read session count");
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type IN ('AgentSession', 'Task')",
            [],
            |row| row.get(0),
        )
        .expect("read event count");
    assert_eq!(sessions, 0);
    assert_eq!(events, 0);
}

fn state_database(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory.path().join("state").join("state.sqlite3")
}

fn service(store: SqliteWorkspaceStore) -> WorkspaceService<SqliteWorkspaceStore> {
    WorkspaceService::new(store)
}

fn context(event_id: &str, minute: u8) -> EventContext {
    let timestamp = format!("2026-10-06T10:{minute:02}:00Z");
    EventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        hlc_timestamp: timestamp.clone(),
        correlation_id: "correlation-1".to_owned(),
        causation_id: None,
        recorded_at: timestamp,
    }
}

fn create_workspace(store: &SqliteWorkspaceStore, workspace_id: &str) -> CommittedWorkspace {
    service(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Desktop workspace".to_owned(),
            owner_principal_id: "owner-local".to_owned(),
            event: context("event-create", 0),
        })
        .expect("Workspace creation commits")
}

#[test]
fn conversation_create_is_durable_idempotent_and_workspace_scoped() {
    use storage_core::{
        ConversationRecord, ConversationStore, CreateConversationCommit, WorkspaceCreateRequest,
    };
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-conversation");
    let at = "2026-10-09T12:00:00Z";
    let record = ConversationRecord {
        conversation_id: "conversation-one".into(),
        workspace_id: "workspace-conversation".into(),
        title: Some("Release notes".into()),
        active_agent_binding_id: None,
        version: 1,
        created_at: at.into(),
    };
    let event_context = context("event-conversation-one", 7);
    let payload = serde_json::json!({"operation":"conversation.create.v1","workspace_id":"workspace-conversation","title":"Release notes"});
    let commit = CreateConversationCommit {
        request: WorkspaceCreateRequest {
            principal_id: "owner-local".into(),
            request_id: "i".repeat(200),
            request_payload: payload,
        },
        conversation: record.clone(),
        event: EventDraft {
            event_id: event_context.event_id,
            workspace_id: record.workspace_id.clone(),
            entity_type: "Conversation".into(),
            entity_id: record.conversation_id.clone(),
            origin_runtime_id: event_context.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: event_context.hlc_timestamp,
            correlation_id: event_context.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.created.v1".into(),
            payload: serde_json::json!({"conversation_id":"conversation-one","created_by":{"kind":"USER","principal_id":"owner-local"}}),
            recorded_at: event_context.recorded_at,
        },
    };
    let adapter = SqliteConversationStore::new(store.clone());
    let created = adapter
        .create_conversation(commit.clone())
        .expect("Conversation commits");
    fn count_blob_files(path: &std::path::Path) -> usize {
        std::fs::read_dir(path)
            .expect("blob directory lists")
            .map(|entry| {
                let entry = entry.expect("blob directory entry reads");
                if entry.file_type().expect("blob entry type reads").is_dir() {
                    count_blob_files(&entry.path())
                } else {
                    1
                }
            })
            .sum()
    }
    let blob_count_before_replay = count_blob_files(&directory.path().join("blobs"));
    let mut retry = commit.clone();
    retry.conversation.conversation_id = "conversation-retry-candidate".into();
    retry.event.event_id = "event-conversation-retry-candidate".into();
    retry.event.entity_id = retry.conversation.conversation_id.clone();
    retry.event.payload = serde_json::json!({
        "conversation_id":retry.conversation.conversation_id,
        "created_by":{"kind":"USER","principal_id":"owner-local"}
    });
    let replay = adapter
        .create_conversation(retry)
        .expect("same request replays exact result");
    assert_eq!(created.conversation, record);
    assert_eq!(replay, created);
    assert_eq!(
        count_blob_files(&directory.path().join("blobs")),
        blob_count_before_replay,
        "a lost-response retry must resolve its receipt before writing a candidate aggregate Blob"
    );
    assert_eq!(
        store
            .read_workspace_events("workspace-conversation")
            .unwrap()
            .iter()
            .filter(|event| event.event_type == "conversation.created.v1")
            .count(),
        1
    );
    assert_eq!(
        adapter
            .get_conversation("owner-local", "workspace-conversation", "conversation-one")
            .unwrap(),
        Some(record)
    );
    assert!(
        adapter
            .get_conversation("someone-else", "workspace-conversation", "conversation-one")
            .is_err()
    );
}

fn seed_workspace_notes_resource(directory: &tempfile::TempDir) {
    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let mut connection = Connection::open(state_database(directory)).expect("fixture connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable constraints");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction.execute(
        "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,context_document_json,provenance_json,created_at,updated_at,version)
         VALUES ('resource-context','workspace-context','FILE','{}','notes.txt','revision-context','PERSONAL',
                 '{\"kind\":\"WORKSPACE_NOTES\",\"owner_ref\":{\"kind\":\"WORKSPACE\",\"workspace_id\":\"workspace-context\"},\"status\":\"ACTIVE\"}',
                 '{\"source_inputs\":[],\"transformations\":[],\"tool_reports\":[]}',
                 '2026-10-06T10:00:00Z','2026-10-06T10:00:00Z',1)",
        [],
    ).expect("ContextDocument Resource fixture");
    transaction.execute(
        "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
         VALUES ('revision-context','resource-context',?1,0,'text/plain','2026-10-06T10:00:00Z','{}')",
        [DIGEST],
    ).expect("Resource revision fixture");
    transaction.execute(
        "INSERT INTO resource_content_blob_registry(workspace_id,digest,size_bytes,media_type,created_at,expires_at,state,claimed_at)
         VALUES ('workspace-context',?1,0,'text/plain','2026-10-06T10:00:00Z',NULL,'TRACKED',NULL)",
        [DIGEST],
    ).expect("Resource content registry fixture");
    transaction.execute(
        "INSERT INTO resource_locations(location_id,resource_id,provider_ref,locator_ref_id,availability,writable,observed_revision_id,observed_digest,observed_at,last_checked_at)
         VALUES ('location-context','resource-context','litecowork.encrypted_blob',?1,'AVAILABLE',0,'revision-context',?1,'2026-10-06T10:00:00Z','2026-10-06T10:00:00Z')",
        [DIGEST],
    ).expect("Resource location fixture");
    transaction.commit().expect("fixture commits");
}

#[test]
fn blob_read_failure_reports_context_document_transition_only_when_proven() {
    let blob_error = StoreError::Blob("Resource blob disappeared during read".to_owned());

    assert_eq!(
        prefer_context_document_status_error_after_read_failure(
            blob_error.clone(),
            Ok(Some("DELETION_PENDING".to_owned())),
        ),
        StoreError::Invalid("CONTEXT_DOCUMENT_DELETION_PENDING".to_owned()),
    );
    assert_eq!(
        prefer_context_document_status_error_after_read_failure(
            blob_error.clone(),
            Ok(Some("REVOKED".to_owned())),
        ),
        StoreError::Invalid("CONTEXT_DOCUMENT_REVOKED".to_owned()),
    );
    assert_eq!(
        prefer_context_document_status_error_after_read_failure(
            blob_error.clone(),
            Ok(Some("ACTIVE".to_owned())),
        ),
        blob_error,
    );
    assert_eq!(
        prefer_context_document_status_error_after_read_failure(
            StoreError::Blob("Resource blob unavailable".to_owned()),
            Err(StoreError::Database("status read failed".to_owned())),
        ),
        StoreError::Blob("Resource blob unavailable".to_owned()),
    );
}

#[test]
fn pinned_resource_content_reads_exact_historical_revision_without_head_fallback() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(5));
    create_workspace(&store, "workspace-history");
    let old_bytes = b"first revision\n";
    let head_bytes = b"second revision\n";
    let old_blob = store
        .inner
        .blobs
        .put(
            "workspace-history",
            BlobPurpose::Resource,
            old_bytes,
            "text/plain",
        )
        .expect("store old bytes");
    let head_blob = store
        .inner
        .blobs
        .put(
            "workspace-history",
            BlobPurpose::Resource,
            head_bytes,
            "text/plain",
        )
        .expect("store head bytes");
    let mut connection = Connection::open(state_database(&directory)).expect("fixture connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable constraints");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction.execute(
        "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,context_document_json,provenance_json,created_at,updated_at,version)
         VALUES ('resource-history','workspace-history','FILE','{}','notes.txt','revision-two','PERSONAL',NULL,'{}','2026-10-08T00:00:00Z','2026-10-08T00:00:00Z',1)",
        [],
    ).expect("Resource fixture");
    transaction.execute(
        "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
         VALUES ('revision-one','resource-history',?1,?2,'text/plain','2026-10-08T00:00:00Z','{}')",
        params![old_blob.digest, i64::try_from(old_bytes.len()).expect("old size")],
    ).expect("old revision fixture");
    transaction.execute(
        "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
         VALUES ('revision-two','resource-history',?1,?2,'text/plain','2026-10-08T00:01:00Z','{}')",
        params![head_blob.digest, i64::try_from(head_bytes.len()).expect("head size")],
    ).expect("head revision fixture");
    transaction.execute(
        "INSERT INTO resource_revision_parents(resource_id,child_revision_id,parent_revision_id) VALUES ('resource-history','revision-two','revision-one')",
        [],
    ).expect("revision ancestry fixture");
    for blob in [&old_blob, &head_blob] {
        transaction
            .execute(
                "INSERT INTO resource_content_blob_registry(
                   workspace_id, digest, size_bytes, media_type, created_at,
                   expires_at, state, claimed_at
                 ) VALUES ('workspace-history', ?1, ?2, 'text/plain',
                           '2026-10-08T00:00:00Z', NULL, 'TRACKED', NULL)",
                rusqlite::params![blob.digest, blob.size_bytes as i64],
            )
            .expect("Resource content registry fixture");
    }
    transaction.execute(
        "INSERT INTO resource_locations(location_id,resource_id,provider_ref,locator_ref_id,availability,writable,observed_revision_id,observed_digest,observed_at,last_checked_at)
         VALUES ('location-history','resource-history','litecowork.encrypted_blob',?1,'AVAILABLE',0,'revision-two',?1,'2026-10-08T00:01:00Z','2026-10-08T00:01:00Z')",
        [&head_blob.digest],
    ).expect("managed local location fixture");
    transaction.commit().expect("fixture commits");

    let old = store
        .read_resource_content_bounded(
            "workspace-history",
            "resource-history",
            Some("revision-one"),
            1024,
        )
        .expect("pinned historical read succeeds")
        .expect("old Resource exists");
    assert_eq!(old.summary.resource_revision_id, "revision-one");
    assert_eq!(old.content, old_bytes);
    let head = store
        .read_resource_content_bounded(
            "workspace-history",
            "resource-history",
            Some("revision-two"),
            1024,
        )
        .expect("pinned head read succeeds")
        .expect("Resource exists");
    assert_eq!(head.summary.resource_revision_id, "revision-two");
    assert_eq!(head.content, head_bytes);
    assert!(
        store
            .read_resource_content_bounded(
                "workspace-history",
                "resource-history",
                Some("revision-foreign"),
                1024
            )
            .expect("unknown pin is a normal miss")
            .is_none()
    );
    assert!(
        store
            .read_resource_content_bounded(
                "workspace-other",
                "resource-history",
                Some("revision-one"),
                1024
            )
            .expect("cross-Workspace selection is a normal miss")
            .is_none()
    );
    assert_eq!(
        store
            .read_resource_content_bounded(
                "workspace-history",
                "resource-history",
                Some("revision-one"),
                4
            )
            .unwrap_err(),
        StoreError::Invalid("RESOURCE_READ_LIMIT_EXCEEDED".to_owned()),
        "the metadata bound rejects before any blob fetch",
    );

    let task = storage_core::TaskView {
        task: storage_core::TaskRecord {
            task_id: "task-history".to_owned(),
            workspace_id: "workspace-history".to_owned(),
            conversation_id: None,
            current_spec_revision: 3,
            current_plan_revision: None,
            status: "READY".to_owned(),
            resume_status: None,
            routine_id: None,
            routine_revision: None,
            automation_id: None,
            automation_occurrence_id: None,
            origin_coworker_id: None,
            origin_coworker_revision: None,
            lead_agent_binding_id: "agent-1".to_owned(),
            blocking_conditions: Vec::new(),
            priority: "NORMAL".to_owned(),
            created_by: json!({"kind": "USER"}),
            created_at: "2026-10-08T00:00:00Z".to_owned(),
            updated_at: "2026-10-08T00:00:00Z".to_owned(),
            completed_at: None,
            version: 9,
        },
        current_spec_revision: storage_core::TaskSpecRevisionRecord {
            task_id: "task-history".to_owned(),
            workspace_id: "workspace-history".to_owned(),
            revision: 3,
            parent_revisions: vec![2],
            objective: "Use the immutable selected source".to_owned(),
            task_category: None,
            constraints: Vec::new(),
            non_goals: Vec::new(),
            input_refs: vec![json!({
                "workspace_id": "workspace-history",
                "resource_id": "resource-history",
                "revision_id": "revision-one"
            })],
            workspace_instruction_revision: None,
            required_outputs: Vec::new(),
            acceptance_criteria: Vec::new(),
            approvals_required: Vec::new(),
            budget: None,
            delegation_budget_policy: None,
            lead_failover_policy: json!({"mode": "ASK"}),
            deadline: None,
            source_message_refs: Vec::new(),
            placement_preference: json!({"kind": "LOCAL"}),
            preferred_lead_agent_binding_id: Some("agent-1".to_owned()),
            authored_by: json!({"kind": "USER"}),
            created_at: "2026-10-08T00:01:00Z".to_owned(),
        },
    };
    let staging_parent = directory.path().join("staging");
    std::fs::create_dir(&staging_parent).expect("staging parent");
    let prepared = local_environment_staging::stage_task_spec_inputs(
        &store,
        &task,
        "workspace-history",
        "task-history",
        9,
        3,
        &staging_parent,
        local_environment_staging::StagingLimits {
            max_files: 4,
            max_file_bytes: 1024,
            max_total_input_bytes: 2048,
            max_path_bytes: 240,
        },
    )
    .expect("Task's exact historical revision stages from ResourceStore");
    assert_eq!(
        prepared.status,
        local_environment_staging::PreparationStatus::PreparedOnly
    );
    assert_eq!(prepared.input_file_count, 1);
    assert_eq!(prepared.input_bytes, old_bytes.len() as u64);
    assert_eq!(
        std::fs::read(prepared.input_root.join("notes.txt")).expect("staged bytes"),
        old_bytes,
        "staging must not substitute the newer Resource head",
    );
    prepared.cleanup().expect("prepared directory cleanup");
}

#[test]
fn task_input_admission_rejects_all_non_active_context_document_states() {
    assert!(ensure_context_document_content_readable(None).is_ok());
    assert!(
        ensure_context_document_content_readable(Some(
            r#"{"kind":"WORKSPACE_NOTES","status":"ACTIVE"}"#
        ))
        .is_ok()
    );
    assert_eq!(
        ensure_context_document_content_readable(Some(
            r#"{"kind":"WORKSPACE_NOTES","status":"DELETION_PENDING"}"#
        ))
        .unwrap_err(),
        StoreError::Invalid("CONTEXT_DOCUMENT_DELETION_PENDING".to_owned()),
    );
    assert_eq!(
        ensure_context_document_content_readable(Some(
            r#"{"kind":"WORKSPACE_NOTES","status":"DELETED"}"#
        ))
        .unwrap_err(),
        StoreError::Invalid("CONTEXT_DOCUMENT_DELETED".to_owned()),
    );
}

#[test]
fn context_document_owner_status_guard_allows_only_active_revoked_transitions() {
    assert!(context_document_owner_transition_allowed(
        "ACTIVE",
        ContextDocumentOwnerStatus::Revoked
    ));
    assert!(context_document_owner_transition_allowed(
        "REVOKED",
        ContextDocumentOwnerStatus::Active
    ));
    assert!(!context_document_owner_transition_allowed(
        "ACTIVE",
        ContextDocumentOwnerStatus::Active
    ));
    assert!(!context_document_owner_transition_allowed(
        "REVOKED",
        ContextDocumentOwnerStatus::Revoked
    ));
    assert!(!context_document_owner_transition_allowed(
        "DELETION_PENDING",
        ContextDocumentOwnerStatus::Active
    ));
    assert!(!context_document_owner_transition_allowed(
        "DELETED",
        ContextDocumentOwnerStatus::Revoked
    ));
}

#[test]
fn context_document_status_revocation_restore_stale_and_replay_are_atomic() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-context");
    seed_workspace_notes_resource(&directory);
    let task_inputs = json!([{
        "workspace_id": "workspace-context",
        "resource_id": "resource-context",
        "revision_id": "revision-context"
    }]);
    let admission_connection =
        Connection::open(state_database(&directory)).expect("Task admission connection");
    assert!(
        validate_task_resource_inputs(
            &admission_connection,
            "workspace-context",
            task_inputs.as_array().unwrap()
        )
        .is_ok()
    );
    assert_eq!(
        validate_task_resource_inputs(
            &admission_connection,
            "workspace-context",
            &[json!({
                "workspace_id": "workspace-context",
                "resource_id": "resource-context",
                "revision_id": "revision-context",
                "path": "/etc/passwd"
            })],
        )
        .unwrap_err(),
        StoreError::Invalid(
            "Task inputs must be unique, pinned Resource revisions in this Workspace".to_owned()
        ),
        "Task pins reject fields outside the closed PinnedResourceRef schema",
    );

    let revoke = SetContextDocumentStatus {
        workspace_id: "workspace-context".to_owned(),
        resource_id: "resource-context".to_owned(),
        principal_id: "owner-local".to_owned(),
        request_id: "request-revoke-context".to_owned(),
        expected_version: 1,
        target_status: ContextDocumentOwnerStatus::Revoked,
        event: context("event-revoke-context", 1),
    };
    let revoked = ResourceService::new(store.clone())
        .set_context_document_status(revoke.clone())
        .expect("owner revocation commits");
    assert_eq!(revoked.resource.resource.version, 2);
    assert_eq!(
        revoked
            .resource
            .context_document
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str),
        Some("REVOKED")
    );
    assert_eq!(
        revoked.event.event_type,
        "resource.context_document.status.changed.v1"
    );
    assert_eq!(
        validate_task_resource_inputs(
            &admission_connection,
            "workspace-context",
            task_inputs.as_array().unwrap()
        )
        .unwrap_err(),
        StoreError::Invalid("CONTEXT_DOCUMENT_REVOKED".to_owned()),
        "revoked ContextDocuments cannot be newly pinned as Task inputs",
    );
    assert_eq!(
        store
            .read_resource_content_bounded(
                "workspace-context",
                "resource-context",
                Some("revision-context"),
                10
            )
            .unwrap_err(),
        StoreError::Invalid("CONTEXT_DOCUMENT_REVOKED".to_owned())
    );

    let replay = ResourceService::new(store.clone())
        .set_context_document_status(revoke)
        .expect("same owner request replays after commit");
    assert_eq!(replay, revoked);
    let status_event_count: i64 = Connection::open(state_database(&directory)).expect("inspect database")
        .query_row("SELECT COUNT(*) FROM domain_events WHERE entity_id='resource-context' AND type='resource.context_document.status.changed.v1'", [], |row| row.get(0))
        .expect("count status events");
    assert_eq!(
        status_event_count, 1,
        "idempotent replay must not append another event"
    );

    let stale_restore = SetContextDocumentStatus {
        workspace_id: "workspace-context".to_owned(),
        resource_id: "resource-context".to_owned(),
        principal_id: "owner-local".to_owned(),
        request_id: "request-stale-restore".to_owned(),
        expected_version: 1,
        target_status: ContextDocumentOwnerStatus::Active,
        event: context("event-stale-restore", 2),
    };
    assert!(matches!(
        ResourceService::new(store.clone()).set_context_document_status(stale_restore),
        Err(StoreError::Conflict { .. })
    ));

    let restore = SetContextDocumentStatus {
        workspace_id: "workspace-context".to_owned(),
        resource_id: "resource-context".to_owned(),
        principal_id: "owner-local".to_owned(),
        request_id: "request-restore-context".to_owned(),
        expected_version: 2,
        target_status: ContextDocumentOwnerStatus::Active,
        event: context("event-restore-context", 3),
    };
    let restored = ResourceService::new(store.clone())
        .set_context_document_status(restore)
        .expect("owner restores retained ContextDocument");
    assert_eq!(restored.resource.resource.version, 3);
    assert_eq!(
        restored
            .resource
            .context_document
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str),
        Some("ACTIVE")
    );
    assert!(
        validate_task_resource_inputs(
            &admission_connection,
            "workspace-context",
            task_inputs.as_array().unwrap()
        )
        .is_ok()
    );
    assert_eq!(
        validate_task_resource_inputs(
            &admission_connection,
            "workspace-context",
            &[task_inputs[0].clone(), task_inputs[0].clone()]
        )
        .unwrap_err(),
        StoreError::Invalid(
            "Task inputs must be unique, pinned Resource revisions in this Workspace".to_owned()
        ),
    );
    assert_eq!(
        validate_task_resource_inputs(
            &admission_connection,
            "another-workspace",
            task_inputs.as_array().unwrap()
        )
        .unwrap_err(),
        StoreError::Invalid(
            "Task inputs must be unique, pinned Resource revisions in this Workspace".to_owned()
        ),
    );
    let events = store
        .read_workspace_events("workspace-context")
        .expect("read workspace events");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.entity_id == "resource-context"
                && event.event_type == "resource.context_document.status.changed.v1")
            .count(),
        2
    );
}

#[test]
fn resource_location_list_is_workspace_scoped_public_and_uses_committed_freshness() {
    const CURRENT_DIGEST: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const STALE_DIGEST: &str =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-context");
    seed_workspace_notes_resource(&directory);
    let connection = Connection::open(state_database(&directory)).expect("fixture connection");
    connection
        .execute(
            "UPDATE resource_locations SET locator_ref_id = 'opaque-stable-location-ref' WHERE location_id = 'location-context'",
            [],
        )
        .expect("private locator sentinel");
    connection
        .execute(
            "INSERT INTO resource_locations(location_id, resource_id, provider_ref, locator_ref_id, availability, writable, observed_revision_id, observed_digest, observed_at, last_checked_at)
             VALUES ('location-offline', 'resource-context', 'provider.private', 'opaque-private-locator', 'OFFLINE', 1, 'revision-context', ?1, '2026-10-06T10:01:00Z', '2026-10-06T10:02:00Z'),
                    ('location-stale', 'resource-context', 'provider.private', 'opaque-private-locator-2', 'AVAILABLE', 0, 'revision-context', ?2, '2026-10-06T10:03:00Z', NULL),
                    ('location-unknown', 'resource-context', 'provider.private', 'opaque-private-locator-3', 'UNKNOWN', 0, NULL, NULL, '2026-10-06T10:04:00Z', NULL),
                    ('location-placeholder', 'resource-context', 'provider.private', 'opaque-private-locator-5', 'PLACEHOLDER', 0, 'revision-context', ?1, '2026-10-06T10:04:30Z', NULL),
                    ('location-revoked', 'resource-context', 'provider.private', 'opaque-private-locator-8', 'REVOKED', 0, 'revision-context', ?1, '2026-10-06T10:04:40Z', NULL),
                    ('location-unavailable', 'resource-context', 'provider.private', 'opaque-private-locator-9', 'UNAVAILABLE', 0, 'revision-context', ?1, '2026-10-06T10:04:50Z', NULL)",
            [CURRENT_DIGEST, STALE_DIGEST],
        )
        .expect("additional location observations");
    connection
        .execute(
            "INSERT INTO resource_revisions(resource_revision_id, resource_id, content_digest, size_bytes, media_type, observed_at, created_by_json)
             VALUES ('revision-old', 'resource-context', ?1, 0, 'text/plain', '2026-10-06T09:00:00Z', '{}')",
            [CURRENT_DIGEST],
        )
        .expect("historical Resource revision");
    connection
        .execute(
            "INSERT INTO resource_revision_parents(resource_id, child_revision_id, parent_revision_id)
             VALUES ('resource-context', 'revision-context', 'revision-old')",
            [],
        )
        .expect("historical revision ancestry");
    connection
        .execute(
            "INSERT INTO resource_locations(location_id, resource_id, provider_ref, locator_ref_id, availability, writable, observed_revision_id, observed_digest, observed_at, last_checked_at)
             VALUES ('location-old-revision', 'resource-context', 'provider.private', 'opaque-location-ref-6', 'AVAILABLE', 0, 'revision-old', ?1, '2026-10-06T10:04:45Z', NULL)",
            [CURRENT_DIGEST],
        )
        .expect("historical revision location observation");
    connection
        .execute(
            "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, context_document_json, provenance_json, created_at, updated_at, version)
             VALUES ('resource-without-locations', 'workspace-context', 'FILE', '{}', 'empty.txt', NULL, 'PERSONAL', NULL, '{}', '2026-10-06T10:05:00Z', '2026-10-06T10:05:00Z', 1)",
            [],
        )
        .expect("resource without a location");
    connection
        .execute(
            "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, context_document_json, provenance_json, created_at, updated_at, version)
             VALUES ('resource-no-digest', 'workspace-context', 'WEB_RESOURCE', '{}', 'remote', NULL, 'PERSONAL', NULL, '{}', '2026-10-06T10:06:00Z', '2026-10-06T10:06:00Z', 1)",
            [],
        )
        .expect("resource with an uncomparable current revision");
    connection
        .execute(
            "INSERT INTO resource_revisions(resource_revision_id, resource_id, content_digest, size_bytes, media_type, observed_at, created_by_json)
             VALUES ('revision-no-digest', 'resource-no-digest', NULL, NULL, 'text/html', '2026-10-06T10:06:00Z', '{}')",
            [],
        )
        .expect("current Resource revision without content digest");
    connection
        .execute(
            "UPDATE resources SET current_revision_id = 'revision-no-digest' WHERE resource_id = 'resource-no-digest'",
            [],
        )
        .expect("select digestless current revision");
    connection
        .execute(
            "INSERT INTO resource_locations(location_id, resource_id, provider_ref, locator_ref_id, availability, writable, observed_revision_id, observed_digest, observed_at, last_checked_at)
             VALUES ('location-no-current-digest', 'resource-no-digest', 'provider.private', 'opaque-location-ref-4', 'AVAILABLE', 0, 'revision-no-digest', ?1, '2026-10-06T10:06:00Z', '2026-10-06T10:06:00Z')",
            [CURRENT_DIGEST],
        )
        .expect("location of Resource revision without digest");
    connection
        .execute(
            "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, context_document_json, provenance_json, created_at, updated_at, version)
             VALUES ('resource-revisionless', 'workspace-context', 'FOLDER', '{}', 'folder', NULL, 'PERSONAL', NULL, '{}', '2026-10-06T10:07:00Z', '2026-10-06T10:07:00Z', 1)",
            [],
        )
        .expect("revisionless Resource");
    connection
        .execute(
            "INSERT INTO resource_locations(location_id, resource_id, provider_ref, locator_ref_id, availability, writable, observed_revision_id, observed_digest, observed_at, last_checked_at)
             VALUES ('location-revisionless', 'resource-revisionless', 'provider.private', 'opaque-location-ref-7', 'AVAILABLE', 0, NULL, NULL, '2026-10-06T10:07:00Z', NULL)",
            [],
        )
        .expect("revisionless resource location");
    connection
        .execute(
            "INSERT INTO workspace_roots(
               workspace_root_id, workspace_id, resource_id, location_id, display_name,
               watch_policy, replication_policy, status, added_by_json,
               created_at, updated_at, version
             ) VALUES (
               'root-resource-location-view', 'workspace-context', 'resource-context',
               'location-context', 'root', 'METADATA', 'NONE', 'ACTIVE',
               '{\"kind\":\"USER\",\"principal_id\":\"owner\"}',
               '2026-10-06T10:00:00Z', '2026-10-06T10:00:00Z', 1
             )",
            [],
        )
        .expect("root owning the selected FILE location");
    connection
        .execute(
            "INSERT INTO workspace_root_file_locations(
               workspace_id, workspace_root_id, location_id, resource_id, observed_at
             ) VALUES (
               'workspace-context', 'root-resource-location-view',
               'location-context', 'resource-context', '2026-10-06T10:00:00Z'
             )",
            [],
        )
        .expect("index root-linked FILE location");

    let locations = store
        .list_resource_locations("workspace-context", "resource-context")
        .expect("location projection query")
        .expect("resource exists in selected Workspace");
    assert_eq!(
        locations
            .iter()
            .map(|location| (location.location_id.as_str(), location.freshness.as_str()))
            .collect::<Vec<_>>(),
        [
            ("location-context", "CURRENT"),
            ("location-offline", "UNAVAILABLE"),
            ("location-old-revision", "STALE"),
            ("location-placeholder", "UNKNOWN"),
            ("location-revoked", "UNAVAILABLE"),
            ("location-stale", "STALE"),
            ("location-unavailable", "UNAVAILABLE"),
            ("location-unknown", "UNKNOWN"),
        ]
    );
    connection
        .execute(
            "UPDATE workspace_roots SET status='REVOKED'
             WHERE workspace_root_id='root-resource-location-view'",
            [],
        )
        .expect("revoke owning root for read-projection check");
    let after_revocation = store
        .list_resource_locations("workspace-context", "resource-context")
        .expect("read after root revocation")
        .expect("resource remains retained");
    assert!(
        after_revocation
            .iter()
            .all(|location| location.location_id != "location-context"),
        "a root-revoked ResourceLocation must not remain in the read projection"
    );
    assert_eq!(locations[0].runtime_id, None);
    assert_eq!(locations[0].environment_id, None);
    assert_eq!(locations[0].connection_id, None);
    assert_eq!(
        locations[0].last_checked_at.as_deref(),
        Some("2026-10-06T10:00:00Z")
    );
    let public_json =
        serde_json::to_string(&locations).expect("public location projection serializes");
    assert!(!public_json.contains("opaque-stable-location-ref"));
    assert!(!public_json.contains("opaque-location-ref"));
    assert!(!public_json.contains("provider.private"));
    assert!(!public_json.contains("locator_ref_id"));
    assert!(!public_json.contains("provider_ref"));

    assert_eq!(
        store
            .list_resource_locations("workspace-other", "resource-context")
            .expect("cross-Workspace query succeeds as a lookup"),
        None,
        "a Resource from another Workspace is not disclosed"
    );
    assert_eq!(
        store
            .list_resource_locations("workspace-context", "missing-resource")
            .expect("missing Resource query succeeds as a lookup"),
        None
    );
    assert_eq!(
        store
            .list_resource_locations("workspace-context", "resource-without-locations")
            .expect("empty location query succeeds"),
        Some(Vec::new())
    );
    assert_eq!(
        store
            .list_resource_locations("workspace-context", "resource-no-digest")
            .expect("uncomparable observation query succeeds")
            .expect("resource exists")
            .first()
            .expect("location exists")
            .freshness,
        "UNKNOWN",
        "freshness cannot be called STALE without a current digest to compare"
    );
    assert_eq!(
        store
            .list_resource_locations("workspace-context", "resource-revisionless")
            .expect("revisionless observation query succeeds")
            .expect("resource exists")
            .first()
            .expect("location exists")
            .freshness,
        "UNKNOWN"
    );
}

#[test]
fn applies_full_contract_schema_and_reports_sqlite_runtime() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let version = store.sqlite_version().expect("SQLite version");
    let mut components = version.split('.').map(|part| part.parse::<u32>().unwrap());
    let actual = (
        components.next().expect("major version"),
        components.next().expect("minor version"),
        components.next().expect("patch version"),
    );
    assert!(
        actual >= (3, 38, 0),
        "SQLite {version} is below the contract minimum"
    );
    println!("SQLite {version}");

    let connection =
        Connection::open(state_database(&directory)).expect("inspect migrated database");
    let user_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read user_version");
    assert_eq!(user_version, SCHEMA_VERSION);
    let migration: (String, String, String) = connection
        .query_row(
            "SELECT name, source_checksum, schema_fingerprint FROM schema_migrations WHERE version = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("migration receipt");
    assert_eq!(migration.0, MIGRATION_NAME);
    assert!(migration.1.starts_with("sha256:") && migration.1.len() == 71);
    assert!(migration.2.starts_with("sha256:") && migration.2.len() == 71);
    let trust_migration: (String, String) = connection
        .query_row(
            "SELECT name, source_checksum FROM schema_migrations WHERE version = 14",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("Trust v14 migration receipt");
    assert_eq!(trust_migration.0, V14_MIGRATION_NAME);
    assert_eq!(trust_migration.0, "trust_policy_decision_audit_v14");
    assert!(trust_migration.1.starts_with("sha256:") && trust_migration.1.len() == 71);
    let invocation_migration: (String, String) = connection
        .query_row(
            "SELECT name, source_checksum FROM schema_migrations WHERE version = 15",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("Invocation v15 migration receipt");
    assert_eq!(invocation_migration.0, V15_MIGRATION_NAME);
    assert!(invocation_migration.1.starts_with("sha256:") && invocation_migration.1.len() == 71);
    let v16_migration: (String, String) = connection
        .query_row(
            "SELECT name, source_checksum FROM schema_migrations WHERE version = 16",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("Agent Registry migration receipt");
    assert_eq!(v16_migration.0, V16_MIGRATION_NAME);
    assert_eq!(v16_migration.1, digest(SQLITE_V16_DDL.as_bytes()));
    let v19_migration: (String, String) = connection
        .query_row(
            "SELECT name, source_checksum FROM schema_migrations WHERE version = 19",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("ResourceIndex v19 migration receipt");
    assert_eq!(v19_migration.0, V19_MIGRATION_NAME);
    assert_eq!(v19_migration.1, digest(SQLITE_V19_DDL.as_bytes()));
    let v20_migration: (String, String) = connection
        .query_row(
            "SELECT name, source_checksum FROM schema_migrations WHERE version = 20",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("VerificationRun v20 migration receipt");
    assert_eq!(v20_migration.0, V20_MIGRATION_NAME);
    assert_eq!(v20_migration.1, digest(SQLITE_V20_DDL.as_bytes()));
    let v21_migration: (String, String) = connection
        .query_row(
            "SELECT name, source_checksum FROM schema_migrations WHERE version = 21",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("WorkspaceRoot observations v21 migration receipt");
    assert_eq!(v21_migration.0, V21_MIGRATION_NAME);
    assert_eq!(v21_migration.1, digest(SQLITE_V21_DDL.as_bytes()));
    let v22_migration: (String, String) = connection
        .query_row(
            "SELECT name, source_checksum FROM schema_migrations WHERE version = 22",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("VerificationRun Evidence binding v22 migration receipt");
    assert_eq!(v22_migration.0, V22_MIGRATION_NAME);
    assert_eq!(v22_migration.1, digest(SQLITE_V22_DDL.as_bytes()));
    let v23_migration: (String, String) = connection
        .query_row(
            "SELECT name, source_checksum FROM schema_migrations WHERE version = 23",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("WorkspaceRoot membership v23 migration receipt");
    assert_eq!(v23_migration.0, V23_MIGRATION_NAME);
    assert_eq!(v23_migration.1, digest(SQLITE_V23_DDL.as_bytes()));
    let policy_decision_column: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('audit_records') WHERE name='policy_decision_json')",
            [],
            |row| row.get(0),
        )
        .expect("Trust v14 policy-decision column");
    assert!(policy_decision_column);
    for table in [
        "workspaces",
        "domain_events",
        "schema_migrations",
        "workspace_origin_sequences",
        "resource_text_indexes",
        "resource_text_index_terms",
        "agent_registry_cache",
        "agent_installation_observations",
        "agent_control_descriptors",
        "agent_lifecycle_operations",
        "agent_binding_configurations",
        "resource_index_blob_registry",
        "resource_content_blob_registry",
        "resource_upload_chunk_blob_registry",
        "workspace_root_observation_generations",
        "workspace_root_observations",
        "workspace_root_observation_bindings",
        "workspace_root_observation_batch_receipts",
        "workspace_root_file_locations",
        "workspace_root_cleanup_outbox",
    ] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .expect("look up schema table");
        assert!(exists, "missing contract table {table}");
    }
    assert!(store.get_workspace("missing").expect("read").is_none());
}

#[test]
fn upgrades_schema_v13_through_resource_index_registry_v19() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let database = state_database(&directory);
    std::fs::create_dir_all(database.parent().expect("database parent path"))
        .expect("create the private state directory before opening the v13 boundary");
    let mut connection = Connection::open(database).expect("open database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV14Migration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read the v13 migration boundary");
    assert_eq!(version, 13);

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV17Migration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read the Agent Registry v16 boundary");
    assert_eq!(version, 16);
    let agent_registry_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'agent_registry_cache')",
            [],
            |row| row.get(0),
        )
        .expect("inspect Agent Registry v16 table");
    let resource_index_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'resource_index_blob_registry')",
            [],
            |row| row.get(0),
        )
        .expect("inspect ResourceIndex v19 table before its migration");
    assert!(agent_registry_exists);
    assert!(!resource_index_exists);

    migrate(&mut connection).expect("apply migrations v14 through v26 in order");

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read migrated schema version");
    assert_eq!(version, SCHEMA_VERSION);
    let applied: Vec<i64> = connection
        .prepare("SELECT version FROM schema_migrations WHERE version >= 14 ORDER BY version")
        .expect("prepare migration query")
        .query_map([], |row| row.get(0))
        .expect("query applied migrations")
        .collect::<Result<_, _>>()
        .expect("collect migration versions");
    assert_eq!(applied, [14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26]);
}

#[test]
fn v21_workspace_root_observation_migration_rolls_back_atomically_and_replays() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let database = state_database(&directory);
    std::fs::create_dir_all(database.parent().expect("database parent path"))
        .expect("create the private state directory");
    let mut connection = Connection::open(database).expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV21Migration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read version after injected failure");
    assert_eq!(version, 20);
    let uncommitted_objects: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name IN (
               'workspace_root_observation_generations',
               'workspace_root_observations',
               'workspace_root_observation_bindings',
               'workspace_root_observation_batch_receipts',
               'workspace_root_observations_delete_private_binding'
             )",
            [],
            |row| row.get(0),
        )
        .expect("inspect failed migration objects");
    assert_eq!(uncommitted_objects, 0);
    let uncommitted_receipt: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 21",
            [],
            |row| row.get(0),
        )
        .expect("inspect failed migration receipt");
    assert_eq!(uncommitted_receipt, 0);

    migrate(&mut connection).expect("retry WorkspaceRoot observations migration");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read replayed schema version");
    assert_eq!(version, SCHEMA_VERSION);
    verify_migration_record(&connection, 21, V21_MIGRATION_NAME, SQLITE_V21_DDL, false)
        .expect("verify replayed v21 migration receipt");
    verify_migration_record(&connection, 22, V22_MIGRATION_NAME, SQLITE_V22_DDL, false)
        .expect("verify v22 migration receipt");
    verify_migration_record(&connection, 23, V23_MIGRATION_NAME, SQLITE_V23_DDL, false)
        .expect("verify v23 migration receipt");
    verify_migration_record(&connection, 24, V24_MIGRATION_NAME, SQLITE_V24_DDL, false)
        .expect("verify historical v24 migration receipt");
    verify_migration_record(&connection, 26, V26_MIGRATION_NAME, SQLITE_V26_DDL, true)
        .expect("verify latest v26 migration receipt");
}

#[test]
fn v19_resource_index_migration_rolls_back_atomically_and_replays() {
    const WORKSPACE_ID: &str = "workspace-v19-rollback";
    const SHARED_INDEX_CONTENT: &[u8] = b"legacy ResourceIndex snapshot shared by two Resources";
    const INDEXED_AT: &str = "2026-10-16T10:00:00Z";

    type ResourceIndexMigrationRow = (
        String,
        String,
        String,
        String,
        String,
        i64,
        String,
        i64,
        String,
    );

    fn resource_index_rows(connection: &Connection) -> Vec<ResourceIndexMigrationRow> {
        let mut statement = connection
            .prepare(
                "SELECT workspace_id, resource_id, resource_revision_id,
                        source_content_digest, extracted_blob_digest, extracted_size_bytes,
                        parser_id, token_key_version, indexed_at
                 FROM resource_text_indexes
                 ORDER BY resource_id, resource_revision_id",
            )
            .expect("prepare ResourceTextIndex snapshot query");
        statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })
            .expect("query ResourceTextIndex snapshot")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect ResourceTextIndex rows")
    }

    let directory = tempfile::tempdir().expect("temporary directory");
    let database = state_database(&directory);
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    drop(store);

    let mut connection = Connection::open(database).expect("open migration test database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");

    let content_digest = digest(SHARED_INDEX_CONTENT);
    let index_digest = digest(SHARED_INDEX_CONTENT);
    connection
        .execute(
            "INSERT INTO resource_index_blob_registry(
                 workspace_id, digest, size_bytes, created_at, expires_at, state, claimed_at
             ) VALUES (?1, ?2, ?3, ?4, NULL, 'TRACKED', NULL)",
            rusqlite::params![
                WORKSPACE_ID,
                index_digest,
                SHARED_INDEX_CONTENT.len() as i64,
                INDEXED_AT,
            ],
        )
        .expect("register the existing v19 index snapshot before seeding legacy rows");

    for (resource_id, revision_id, display_name) in [
        (
            "resource-v19-rollback-a",
            "revision-v19-rollback-a",
            "rollback-a.txt",
        ),
        (
            "resource-v19-rollback-b",
            "revision-v19-rollback-b",
            "rollback-b.txt",
        ),
    ] {
        connection
            .execute(
                "INSERT INTO resources(
                     resource_id, workspace_id, kind, provider_identity_json, identity_digest,
                     display_name, current_revision_id, sensitivity, provenance_json,
                     created_at, updated_at, version
                 ) VALUES (?1, ?2, 'FILE', '{}', NULL, ?3, NULL, 'PERSONAL', '{}', ?4, ?4, 1)",
                rusqlite::params![resource_id, WORKSPACE_ID, display_name, INDEXED_AT],
            )
            .expect("insert a legacy Resource");
        connection
            .execute(
                "INSERT INTO resource_revisions(
                     resource_revision_id, resource_id, content_digest, size_bytes,
                     media_type, observed_at, created_by_json
                 ) VALUES (?1, ?2, ?3, ?4, 'text/plain', ?5, '{\"kind\":\"USER\",\"principal_id\":\"owner-local\"}')",
                rusqlite::params![
                    revision_id,
                    resource_id,
                    content_digest,
                    SHARED_INDEX_CONTENT.len() as i64,
                    INDEXED_AT,
                ],
            )
            .expect("insert the matching immutable Resource revision");
        connection
            .execute(
                "UPDATE resources SET current_revision_id = ?1 WHERE resource_id = ?2",
                rusqlite::params![revision_id, resource_id],
            )
            .expect("select the Resource's current revision");
        connection
            .execute(
                "INSERT INTO resource_text_indexes(
                     workspace_id, resource_id, resource_revision_id, source_content_digest,
                     extracted_blob_digest, extracted_size_bytes, parser_id, token_key_version,
                     indexed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'litecowork.plain-text.v1', 1, ?7)",
                rusqlite::params![
                    WORKSPACE_ID,
                    resource_id,
                    revision_id,
                    content_digest,
                    index_digest,
                    SHARED_INDEX_CONTENT.len() as i64,
                    INDEXED_AT,
                ],
            )
            .expect("insert a valid current-revision ResourceTextIndex row");
    }
    let source_rows_before = resource_index_rows(&connection);
    assert_eq!(source_rows_before.len(), 2);

    connection
        .execute_batch(
            "DROP TRIGGER IF EXISTS capability_invocation_dispatch_requires_consumed_approval;
             DROP TRIGGER IF EXISTS capability_invocation_status_transition_guard;
             DELETE FROM schema_migrations WHERE version = 26;
             PRAGMA user_version = 25;",
        )
        .expect("restore the v26 migration boundary");
    connection
        .execute_batch(SQLITE_V15_DDL)
        .expect("restore the v15 invocation guards");
    connection
        .execute_batch(
            "DROP TRIGGER IF EXISTS resource_edges_immutable_update;
             DROP TRIGGER IF EXISTS resource_edges_immutable_delete;
             DELETE FROM schema_migrations WHERE version = 25;
             DROP TRIGGER IF EXISTS delegation_results_immutable_update;
             DROP TRIGGER IF EXISTS delegation_results_immutable_delete;
             DROP TABLE IF EXISTS delegation_results;
             DELETE FROM schema_migrations WHERE version = 24;
             DROP INDEX IF EXISTS idx_workspace_root_file_locations_cleanup;
             DROP TABLE IF EXISTS workspace_root_file_locations;
             DROP TABLE IF EXISTS workspace_root_cleanup_outbox;
             DELETE FROM schema_migrations WHERE version = 23;
             DROP TRIGGER effect_verified_requires_bound_evidence_v22;
             DROP TRIGGER verification_run_pass_requires_bound_evidence;
             DROP TRIGGER evidence_verification_run_binding_insert_guard;
             DROP INDEX evidence_verification_run;
             ALTER TABLE evidence DROP COLUMN verification_run_id;
             DELETE FROM schema_migrations WHERE version = 22;
             PRAGMA user_version = 21;
             DROP TRIGGER workspace_root_observations_delete_private_binding;
             DROP TABLE workspace_root_observation_batch_receipts;
             DROP TABLE workspace_root_observation_bindings;
             DROP TABLE workspace_root_observations;
             DROP TABLE workspace_root_observation_generations;
             DELETE FROM schema_migrations WHERE version = 21;
             PRAGMA user_version = 20;
             DROP TRIGGER verification_run_insert_guard;
             DROP TRIGGER verification_run_transition_guard;
             DROP TRIGGER verification_run_terminal_immutable;
             DROP TRIGGER verification_run_delete_guard;
             DROP INDEX idx_verification_runs_task_status_time;
             ALTER TABLE verification_runs DROP COLUMN failure_code;
             ALTER TABLE verification_runs DROP COLUMN version;
             DELETE FROM schema_migrations WHERE version = 20;
             DROP TRIGGER resource_index_blob_registry_reference_delete_guard;
             DROP TRIGGER resource_text_index_blob_registry_update_guard;
             DROP TRIGGER resource_text_index_blob_registry_insert_guard;
             DROP INDEX resource_index_blob_registry_gc;
             DROP TABLE resource_index_blob_registry;
             DELETE FROM schema_migrations WHERE version = 19;
             PRAGMA user_version = 18;",
        )
        .expect("restore a valid v18 migration boundary");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV19Migration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read version after failed v19 migration");
    assert_eq!(version, 18);
    assert_eq!(
        resource_index_rows(&connection),
        source_rows_before,
        "failed v19 backfill must preserve every source projection row"
    );
    let v19_objects: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name IN (
               'resource_index_blob_registry',
               'resource_index_blob_registry_gc',
               'resource_text_index_blob_registry_insert_guard',
               'resource_text_index_blob_registry_update_guard',
               'resource_index_blob_registry_reference_delete_guard'
             )",
            [],
            |row| row.get(0),
        )
        .expect("inspect rolled back ResourceIndex objects");
    assert_eq!(v19_objects, 0);
    let v19_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 19",
            [],
            |row| row.get(0),
        )
        .expect("inspect rolled back v19 receipt");
    assert_eq!(v19_receipts, 0);

    migrate(&mut connection).expect("retry v19 migration after rollback");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read replayed v19 version");
    assert_eq!(version, V26_SCHEMA_VERSION);
    verify_migration_record(&connection, 19, V19_MIGRATION_NAME, SQLITE_V19_DDL, false)
        .expect("verify replayed v19 receipt");
    verify_migration_record(&connection, 20, V20_MIGRATION_NAME, SQLITE_V20_DDL, false)
        .expect("verify v20 receipt after v21 replay");
    verify_migration_record(&connection, 21, V21_MIGRATION_NAME, SQLITE_V21_DDL, false)
        .expect("verify replayed v21 receipt");
    verify_migration_record(&connection, 22, V22_MIGRATION_NAME, SQLITE_V22_DDL, false)
        .expect("verify replayed v22 receipt");
    verify_migration_record(&connection, 23, V23_MIGRATION_NAME, SQLITE_V23_DDL, false)
        .expect("verify replayed v23 receipt");
    verify_migration_record(&connection, 24, V24_MIGRATION_NAME, SQLITE_V24_DDL, false)
        .expect("verify v24 receipt after v25 replay");
    verify_migration_record(&connection, 26, V26_MIGRATION_NAME, SQLITE_V26_DDL, true)
        .expect("verify latest migration receipt");

    let (registry_count, size_bytes, state, reference_count): (i64, i64, String, i64) = connection
        .query_row(
            "SELECT COUNT(*), MAX(registry.size_bytes), MAX(registry.state),
                    (SELECT COUNT(*) FROM resource_text_indexes idx
                     WHERE idx.workspace_id = ?1
                       AND idx.extracted_blob_digest = ?2)
             FROM resource_index_blob_registry registry
             WHERE registry.workspace_id = ?1 AND registry.digest = ?2",
            rusqlite::params![WORKSPACE_ID, index_digest],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read the backfilled shared ResourceIndex root");
    assert_eq!(registry_count, 1);
    assert_eq!(size_bytes, SHARED_INDEX_CONTENT.len() as i64);
    assert_eq!(state, "TRACKED");
    assert_eq!(reference_count, 2);
    assert_eq!(resource_index_rows(&connection), source_rows_before);
}

#[cfg(unix)]
#[test]
fn sqlite_state_directory_and_database_file_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let state_dir = directory.path().join("state");
    let database = state_dir.join("state.sqlite3");

    assert_eq!(
        fs::metadata(state_dir)
            .expect("state directory metadata")
            .permissions()
            .mode()
            & 0o077,
        0
    );
    assert_eq!(
        fs::metadata(database)
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    drop(store);
}

#[cfg(unix)]
#[test]
fn sqlite_store_rejects_symlink_database_path() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("temporary directory");
    let state_dir = directory.path().join("state");
    ensure_private_directory(&state_dir).expect("private state directory");
    let target = state_dir.join("outside.sqlite3");
    fs::write(&target, b"do not open").expect("create target");
    let linked_database = state_dir.join("linked.sqlite3");
    symlink(&target, &linked_database).expect("make database symlink");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));

    let result = SqliteWorkspaceStore::open(linked_database, blobs, SqliteConfig::default());
    assert!(matches!(result, Err(StoreError::Io(_))));
    assert_eq!(
        fs::read(target).expect("target is unchanged"),
        b"do not open"
    );
}

#[cfg(unix)]
#[test]
fn sqlite_store_rejects_broad_state_directory_without_creating_database() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    let state_dir = directory.path().join("state");
    fs::create_dir(&state_dir).expect("create state directory");
    fs::set_permissions(&state_dir, fs::Permissions::from_mode(0o755))
        .expect("make state directory broad");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));

    let result = SqliteWorkspaceStore::open(
        state_dir.join("state.sqlite3"),
        blobs,
        SqliteConfig::default(),
    );
    assert!(matches!(result, Err(StoreError::Io(_))));
    assert!(!state_dir.join("state.sqlite3").exists());
}

#[test]
fn canonical_json_uses_jcs_object_and_number_encoding() {
    let value: serde_json::Value = serde_json::from_str(
        r#"{"numbers":[333333333.33333329,1E30,4.50,2e-3,0.000000000000000000000000001],"z":1,"a":2}"#,
    )
    .expect("valid JSON fixture");

    assert_eq!(
        canonical_json(&value).expect("JCS canonical bytes"),
        br#"{"a":2,"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"z":1}"#,
    );
    assert!(matches!(
        canonical_json(&serde_json::json!({"sequence": 9_007_199_254_740_992_u64})),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn workspace_create_update_restart_and_projection_replay_match() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-persist");
    service(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: "workspace-persist".to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: Vec::new(),
            event: context("event-policy", 1),
        })
        .expect("policy update commits");
    drop(store);

    let reopened = test_store(&directory, Duration::from_secs(1));
    let stored = reopened
        .get_workspace("workspace-persist")
        .expect("read Workspace")
        .expect("Workspace exists");
    let replayed = reopened
        .rebuild_workspace_projection("workspace-persist")
        .expect("rebuild projection")
        .expect("projection exists");
    assert_eq!(replayed, stored);
    assert_eq!(replayed.replication_policy, ReplicationPolicy::MetadataOnly);
    assert_eq!(replayed.version, 2);

    let events = reopened
        .read_workspace_events("workspace-persist")
        .expect("read event stream");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].origin_sequence, 1);
    assert_eq!(events[1].origin_sequence, 2);
    assert_eq!(events[0].entity_revision, 1);
    assert_eq!(events[1].entity_revision, 2);
    println!(
        "storage replay: sqlite={}, workspace={}, version={}, policy={}, events={}, replay_matches=true",
        reopened.sqlite_version().expect("SQLite version"),
        replayed.workspace_id,
        replayed.version,
        replayed.replication_policy.as_str(),
        events.len(),
    );
}

#[test]
fn workspace_policy_receipt_replays_after_store_reopen() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-policy-receipt-reopen");

    let principal_id = "owner-local".to_owned();
    let request_id = "request-policy-receipt-reopen".to_owned();
    let request_payload = serde_json::json!({
        "operation": "workspace.replication_policy.change.v1",
        "workspace_id": "workspace-policy-receipt-reopen",
        "expected_version": 1,
        "policy": "METADATA_ONLY",
    });
    let command = ChangeReplicationPolicy {
        workspace_id: "workspace-policy-receipt-reopen".to_owned(),
        expected_version: 1,
        policy: ReplicationPolicy::MetadataOnly,
        replication_scope_root_ids: Vec::new(),
        event: context("event-policy-receipt-reopen", 1),
    };
    let committed = service(store.clone())
        .change_replication_policy_idempotent(
            command.clone(),
            principal_id.clone(),
            request_id.clone(),
            request_payload.clone(),
        )
        .expect("commit the idempotent policy request");
    drop(store);

    let reopened = test_store(&directory, Duration::from_secs(1));
    let replayed = service(reopened.clone())
        .change_replication_policy_idempotent(
            command,
            principal_id.clone(),
            request_id.clone(),
            request_payload,
        )
        .expect("replay the original receipt after reopening the store");
    assert_eq!(replayed, committed);

    let changed_request_payload = serde_json::json!({
        "operation": "workspace.replication_policy.change.v1",
        "workspace_id": "workspace-policy-receipt-reopen",
        "expected_version": 1,
        "policy": "FULL_WORKSPACE",
    });
    let changed_command = ChangeReplicationPolicy {
        workspace_id: "workspace-policy-receipt-reopen".to_owned(),
        expected_version: 1,
        policy: ReplicationPolicy::FullWorkspace,
        replication_scope_root_ids: Vec::new(),
        event: context("event-policy-receipt-reopen-conflict", 2),
    };
    assert!(matches!(
        service(reopened.clone()).change_replication_policy_idempotent(
            changed_command,
            principal_id.clone(),
            request_id.clone(),
            changed_request_payload,
        ),
        Err(StoreError::Conflict { .. })
    ));

    let workspace = reopened
        .get_workspace("workspace-policy-receipt-reopen")
        .expect("read Workspace after replay and conflict")
        .expect("Workspace remains present");
    assert_eq!(workspace, committed.workspace);
    let events = reopened
        .read_workspace_events("workspace-policy-receipt-reopen")
        .expect("read Workspace events after replay and conflict");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].origin_sequence, 1);
    assert_eq!(events[1].origin_sequence, 2);
    assert_eq!(
        reopened
            .rebuild_workspace_projection("workspace-policy-receipt-reopen")
            .expect("rebuild projection after replay and conflict"),
        Some(committed.workspace.clone())
    );

    let connection = Connection::open(state_database(&directory)).expect("inspect receipt");
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            [&principal_id, &request_id],
            |row| row.get(0),
        )
        .expect("count durable request receipts");
    assert_eq!(receipt_count, 1);
}

#[test]
fn workspace_event_replay_pages_use_stable_origin_sequences_and_reject_gaps() {
    use storage_core::EventStore;

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-replay-page");
    service(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: "workspace-replay-page".to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: Vec::new(),
            event: EventContext {
                event_id: "event-replay-page-2".to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: "2026-10-06T09:00:00Z".to_owned(),
                correlation_id: "correlation-replay-page".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-06T10:01:00Z".to_owned(),
            },
        })
        .expect("second origin event commits");

    let first = store
        .read_workspace_event_page("workspace-replay-page", "runtime-local", 0, 1)
        .expect("first replay page reads");
    assert_eq!(first.head_sequence, 2);
    assert_eq!(first.retained_from_sequence, Some(1));
    assert_eq!(first.events.len(), 1);
    assert_eq!(first.events[0].origin_sequence, 1);
    assert_eq!(first.events[0].event_id, "event-create");
    assert!(first.has_more);
    assert!(!first.has_gap);
    assert!(!first.cursor_is_ahead);

    drop(store);
    let reopened = test_store(&directory, Duration::from_secs(1));
    let second = reopened
        .read_workspace_event_page("workspace-replay-page", "runtime-local", 1, 1)
        .expect("resume after restart reads the next event");
    assert_eq!(second.head_sequence, 2);
    assert_eq!(second.events.len(), 1);
    assert_eq!(second.events[0].origin_sequence, 2);
    assert_eq!(second.events[0].event_id, "event-replay-page-2");
    assert!(!second.has_more);
    assert!(!second.has_gap);

    let ahead = reopened
        .read_workspace_event_page("workspace-replay-page", "runtime-local", 3, 10)
        .expect("future cursor is reported as a resync condition");
    assert_eq!(ahead.head_sequence, 2);
    assert!(ahead.cursor_is_ahead);
    assert!(ahead.events.is_empty());

    let isolated = reopened
        .read_workspace_event_page("workspace-replay-page", "another-runtime", 0, 10)
        .expect("origin scope is isolated");
    assert_eq!(isolated.head_sequence, 0);
    assert!(isolated.events.is_empty());
    assert!(
        reopened
            .read_workspace_event_page("workspace-replay-page", "runtime-local", 0, 0)
            .is_err()
    );
    assert!(
        reopened
            .read_workspace_event_page(
                "workspace-replay-page",
                "runtime-local",
                0,
                storage_core::MAX_WORKSPACE_EVENT_REPLAY_PAGE_SIZE + 1,
            )
            .is_err()
    );

    let database = Connection::open(state_database(&directory)).expect("open gap fixture");
    database
        .execute(
            "DELETE FROM domain_events WHERE workspace_id = ?1 AND origin_runtime_id = ?2 AND origin_sequence = 2",
            params!["workspace-replay-page", "runtime-local"],
        )
        .expect("simulate an archived replay entry");
    let gap = reopened
        .read_workspace_event_page("workspace-replay-page", "runtime-local", 1, 10)
        .expect("missing retained sequence is reported");
    assert_eq!(gap.head_sequence, 2);
    assert_eq!(gap.retained_from_sequence, Some(1));
    assert!(gap.events.is_empty());
    assert!(gap.has_gap);

    database
        .execute(
            "DELETE FROM domain_events WHERE workspace_id = ?1 AND origin_runtime_id = ?2 AND origin_sequence = 1",
            params!["workspace-replay-page", "runtime-local"],
        )
        .expect("simulate an expired replay prefix");
    let expired = reopened
        .read_workspace_event_page("workspace-replay-page", "runtime-local", 0, 10)
        .expect("cursor older than retained floor is reported");
    assert_eq!(expired.head_sequence, 2);
    assert_eq!(expired.retained_from_sequence, None);
    assert!(expired.has_gap);
}

#[test]
fn workspace_event_replay_detects_gap_beyond_truncated_page() {
    use storage_core::EventStore;

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-replay-truncated-gap");
    for (version, sequence) in [(1, 2), (2, 3)] {
        service(store.clone())
            .change_replication_policy(ChangeReplicationPolicy {
                workspace_id: "workspace-replay-truncated-gap".to_owned(),
                expected_version: version,
                policy: ReplicationPolicy::MetadataOnly,
                replication_scope_root_ids: Vec::new(),
                event: EventContext {
                    event_id: format!("event-truncated-gap-{sequence}"),
                    origin_runtime_id: "runtime-local".to_owned(),
                    hlc_timestamp: format!("2026-10-06T09:00:0{sequence}Z"),
                    correlation_id: format!("correlation-truncated-gap-{sequence}"),
                    causation_id: None,
                    recorded_at: format!("2026-10-06T10:01:0{sequence}Z"),
                },
            })
            .expect("origin event commits");
    }

    let database = Connection::open(state_database(&directory)).expect("open gap fixture");
    database
        .execute(
            "DELETE FROM domain_events WHERE workspace_id = ?1 AND origin_runtime_id = ?2 AND origin_sequence = 3",
            params!["workspace-replay-truncated-gap", "runtime-local"],
        )
        .expect("simulate a missing event after the returned page");

    let page = store
        .read_workspace_event_page("workspace-replay-truncated-gap", "runtime-local", 0, 1)
        .expect("truncated replay page reads");
    assert_eq!(page.head_sequence, 3);
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].origin_sequence, 1);
    assert!(page.has_more);
    assert!(page.has_gap);
}

#[test]
fn workspace_event_replay_rejects_allocator_head_behind_persisted_events() {
    use storage_core::EventStore;

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-replay-corrupt");
    let database = Connection::open(state_database(&directory)).expect("open corruption fixture");
    database
        .execute(
            "UPDATE workspace_origin_sequences SET last_sequence = 0 WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
            params!["workspace-replay-corrupt", "runtime-local"],
        )
        .expect("simulate an inconsistent durable head");

    assert!(matches!(
        store.read_workspace_event_page("workspace-replay-corrupt", "runtime-local", 0, 10),
        Err(StoreError::Integrity(_))
    ));
}

#[test]
fn concurrent_expected_version_writes_have_one_winner() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-race");
    let barrier = Arc::new(Barrier::new(3));

    let make_candidate = |policy: ReplicationPolicy, event_id: &'static str| {
        let mut workspace = store
            .get_workspace("workspace-race")
            .expect("read Workspace")
            .expect("Workspace exists");
        workspace.replication_policy = policy.clone();
        workspace.version = 2;
        workspace.updated_at = "2026-10-06T10:01:00Z".to_owned();
        let event = EventDraft {
            event_id: event_id.to_owned(),
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 2,
            hlc_timestamp: "2026-10-06T10:01:00Z".to_owned(),
            correlation_id: "race".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "workspace.replication_policy.changed.v1".to_owned(),
            payload: json!({
                "workspace_id": workspace.workspace_id,
                "from": "LOCAL_ONLY",
                "to": policy.as_str(),
                "replication_scope_root_ids": [],
                "aggregate_version": 2,
            }),
            recorded_at: "2026-10-06T10:01:00Z".to_owned(),
        };
        (workspace, event)
    };
    let left = make_candidate(ReplicationPolicy::MetadataOnly, "event-left");
    let right = make_candidate(ReplicationPolicy::FullWorkspace, "event-right");

    let spawn =
        |store: SqliteWorkspaceStore, barrier: Arc<Barrier>, candidate: (Workspace, EventDraft)| {
            std::thread::spawn(move || {
                barrier.wait();
                store.commit_workspace(Some(1), candidate.0, candidate.1)
            })
        };
    let left_thread = spawn(store.clone(), barrier.clone(), left);
    let right_thread = spawn(store.clone(), barrier.clone(), right);
    barrier.wait();

    let results = [
        left_thread.join().expect("left writer finished"),
        right_thread.join().expect("right writer finished"),
    ];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(StoreError::Conflict { .. })))
            .count(),
        1
    );
    assert_eq!(
        store
            .read_workspace_events("workspace-race")
            .expect("events")
            .len(),
        2
    );
    assert_eq!(
        store
            .get_workspace("workspace-race")
            .expect("Workspace")
            .expect("exists")
            .version,
        2
    );
}

#[test]
fn injected_failures_roll_back_aggregate_event_and_sequence_together() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let database_path = directory.path().join("fault").join("fault.sqlite3");
    ensure_private_directory(database_path.parent().expect("database parent"))
        .expect("private database parent");
    let mut connection = Connection::open(&database_path).expect("open test database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable FK");
    connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("enable WAL");
    migrate(&mut connection).expect("initial migration");

    for failpoint in [
        Failpoint::AfterAggregate,
        Failpoint::AfterSequence,
        Failpoint::AfterEvent,
        Failpoint::BeforeCommit,
    ] {
        let workspace = Workspace {
            workspace_id: "workspace-rollback".to_owned(),
            name: "Rollback".to_owned(),
            owner_principal_id: "owner".to_owned(),
            replication_policy: ReplicationPolicy::LocalOnly,
            replication_scope_root_ids: Vec::new(),
            current_instruction_revision: None,
            default_agent_binding_id: None,
            primary_coworker_id: None,
            hub_runtime_id: None,
            status: "ACTIVE".to_owned(),
            created_at: "2026-10-06T10:00:00Z".to_owned(),
            updated_at: "2026-10-06T10:00:00Z".to_owned(),
            version: 1,
        };
        let event = EventDraft {
            event_id: "event-rollback".to_owned(),
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: "runtime-1".to_owned(),
            entity_revision: 1,
            hlc_timestamp: workspace.created_at.clone(),
            correlation_id: "correlation".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "workspace.created.v1".to_owned(),
            payload: json!({
                "workspace_id": workspace.workspace_id,
                "owner_principal_id": workspace.owner_principal_id,
                "replication_policy": "LOCAL_ONLY",
                "replication_scope_root_ids": [],
            }),
            recorded_at: workspace.created_at.clone(),
        };
        let state_ref = AggregateStateRef {
            blob: storage_core::BlobRef {
                digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
                size_bytes: 1,
                media_type: "application/json".to_owned(),
            },
            entity_revision: 1,
            record_schema_version: 1,
        };

        let result = commit_workspace_transaction(
            &mut connection,
            None,
            workspace,
            event,
            state_ref,
            Some(failpoint),
        );
        assert!(result.is_err());
        for table in ["workspaces", "domain_events", "workspace_origin_sequences"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count rows");
            assert_eq!(count, 0, "{table} changed at {failpoint:?}");
        }
    }
}

#[test]
fn before_commit_failure_rolls_back_request_receipt_with_workspace_event_and_sequence() {
    fn assert_table_counts(connection: &Connection, expected: i64) {
        for table in [
            "workspaces",
            "domain_events",
            "workspace_origin_sequences",
            "request_dedup",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count transaction rows");
            assert_eq!(count, expected, "unexpected rows in {table}");
        }
    }

    let directory = tempfile::tempdir().expect("temporary directory");
    let database_path = directory.path().join("fault").join("fault.sqlite3");
    ensure_private_directory(database_path.parent().expect("database parent"))
        .expect("private database parent");
    let mut connection = Connection::open(&database_path).expect("open test database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable FK");
    connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("enable WAL");
    migrate(&mut connection).expect("initial migration");

    let command = || {
        let workspace = Workspace {
            workspace_id: "workspace-receipt-rollback".to_owned(),
            name: "Receipt rollback".to_owned(),
            owner_principal_id: "owner".to_owned(),
            replication_policy: ReplicationPolicy::LocalOnly,
            replication_scope_root_ids: Vec::new(),
            current_instruction_revision: None,
            default_agent_binding_id: None,
            primary_coworker_id: None,
            hub_runtime_id: None,
            status: "ACTIVE".to_owned(),
            created_at: "2026-10-13T10:00:00Z".to_owned(),
            updated_at: "2026-10-13T10:00:00Z".to_owned(),
            version: 1,
        };
        let event = EventDraft {
            event_id: "event-receipt-rollback".to_owned(),
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: "runtime-1".to_owned(),
            entity_revision: 1,
            hlc_timestamp: workspace.created_at.clone(),
            correlation_id: "correlation".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "workspace.created.v1".to_owned(),
            payload: json!({
                "workspace_id": workspace.workspace_id,
                "owner_principal_id": workspace.owner_principal_id,
                "replication_policy": "LOCAL_ONLY",
                "replication_scope_root_ids": [],
            }),
            recorded_at: workspace.created_at.clone(),
        };
        let state_ref = AggregateStateRef {
            blob: BlobRef {
                digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
                size_bytes: 1,
                media_type: "application/json".to_owned(),
            },
            entity_revision: 1,
            record_schema_version: 1,
        };
        (workspace, event, state_ref)
    };
    let request = || RequestDeduplication {
        principal_id: "owner".to_owned(),
        request_id: "create-workspace-receipt-rollback".to_owned(),
        request_digest: digest(b"canonical create workspace request"),
        archive_request_payload: None,
        created_at: "2026-10-13T10:00:00Z".to_owned(),
    };
    let (workspace, event, state_ref) = command();
    let failed = commit_workspace_transaction_with_dedup(
        &mut connection,
        None,
        workspace,
        event,
        state_ref,
        Some(request()),
        Some(Failpoint::BeforeCommit),
    );
    assert!(matches!(
        failed,
        Err(StoreError::Database(message)) if message == "injected BeforeCommit"
    ));
    assert_table_counts(&connection, 0);

    let (workspace, event, state_ref) = command();
    let committed = commit_workspace_transaction_with_dedup(
        &mut connection,
        None,
        workspace,
        event,
        state_ref,
        Some(request()),
        None,
    )
    .expect("retry the exact command after rollback");
    assert_eq!(committed.workspace.version, 1);
    assert_eq!(committed.event.origin_sequence, 1);
    assert_table_counts(&connection, 1);

    let (workspace, event, state_ref) = command();
    let replayed = commit_workspace_transaction_with_dedup(
        &mut connection,
        None,
        workspace,
        event,
        state_ref,
        Some(request()),
        None,
    )
    .expect("exact retry replays the committed response");
    assert_eq!(replayed, committed);
    assert_table_counts(&connection, 1);

    let (request_digest, response_json, response_digest): (String, String, String) = connection
        .query_row(
            "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            ["owner", "create-workspace-receipt-rollback"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read committed request receipt");
    assert_eq!(
        request_digest,
        digest(b"canonical create workspace request")
    );
    assert_eq!(response_digest, digest(response_json.as_bytes()));
    let saved_response: CommittedWorkspace =
        serde_json::from_str(&response_json).expect("decode saved response");
    assert_eq!(saved_response, committed);
}

#[test]
fn migration_crash_rolls_back_ddl_and_can_retry() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut connection =
        Connection::open(directory.path().join("migration.sqlite3")).expect("open test database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable FK");
    connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("enable WAL");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringMigration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 0);
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .expect("count schema objects");
    assert_eq!(count, 0);

    migrate(&mut connection).expect("retry full migration");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn runtime_workspace_binding_migration_preserves_authorization_across_v4_retry() {
    #[derive(Debug, PartialEq, Eq)]
    struct MigratedBinding {
        binding_id: String,
        runtime_id: String,
        workspace_id: String,
        enrollment_mode: String,
        status: String,
        roles_json: String,
        created_at: String,
        activated_at: Option<String>,
        revoked_at: Option<String>,
        version: i64,
    }

    fn migrated_bindings(connection: &Connection) -> Vec<MigratedBinding> {
        connection
            .prepare(
                "SELECT runtime_workspace_binding_id, runtime_id, workspace_id, enrollment_mode,
                        status, roles_json, created_at, activated_at, revoked_at, version
                 FROM runtime_workspace_bindings ORDER BY runtime_id",
            )
            .expect("prepare migrated binding query")
            .query_map([], |row| {
                Ok(MigratedBinding {
                    binding_id: row.get(0)?,
                    runtime_id: row.get(1)?,
                    workspace_id: row.get(2)?,
                    enrollment_mode: row.get(3)?,
                    status: row.get(4)?,
                    roles_json: row.get(5)?,
                    created_at: row.get(6)?,
                    activated_at: row.get(7)?,
                    revoked_at: row.get(8)?,
                    version: row.get(9)?,
                })
            })
            .expect("query migrated bindings")
            .collect::<Result<_, _>>()
            .expect("collect migrated bindings")
    }

    let mut connection = Connection::open_in_memory().expect("open legacy database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    connection
        .execute_batch(SQLITE_V1_DDL)
        .expect("create v1 schema");
    record_migration(&connection, 1, V1_MIGRATION_NAME, SQLITE_V1_DDL).expect("record v1 baseline");
    connection
        .execute_batch(SQLITE_V2_DDL)
        .expect("create v2 schema");
    record_migration(&connection, 2, V2_MIGRATION_NAME, SQLITE_V2_DDL).expect("record v2 baseline");
    connection
        .execute("INSERT INTO workspaces(workspace_id, name, owner_principal_id, replication_policy, status, created_at, updated_at, version) VALUES ('workspace-v2', 'Legacy workspace', 'owner', 'LOCAL_ONLY', 'ACTIVE', '2026-10-10T10:00:00Z', '2026-10-10T10:00:00Z', 1)", [])
        .expect("insert legacy Workspace");
    for (runtime_id, availability, roles_json) in [
        (
            "runtime-active",
            "ONLINE",
            r#"["EXECUTOR","WORKSPACE_HUB"]"#,
        ),
        ("runtime-revoked", "REVOKED", r#"["EXECUTOR"]"#),
    ] {
        connection
            .execute(
                "INSERT INTO runtimes(runtime_id, workspace_id, device_identity_json, runtime_version,
                    platform, architecture, roles_json, trust_zone, availability, startup_policy,
                    current_incarnation_id, resource_capacity_json, last_seen, version)
                 VALUES (?1, 'workspace-v2', '{}', '0.1.0', 'linux', 'x86_64', ?2,
                    'PERSONAL_DEVICE', ?3, 'MANUAL', NULL, '{}', '2026-10-10T10:01:00Z', 1)",
                params![runtime_id, roles_json, availability],
            )
            .expect("insert legacy Runtime");
    }
    connection
        .pragma_update(None, "user_version", 2)
        .expect("mark legacy schema version");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV4Migration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read version after interrupted v4 migration");
    assert_eq!(version, V3_SCHEMA_VERSION);
    let expected = vec![
        MigratedBinding {
            binding_id: "rwb_migrated_runtime-active".to_owned(),
            runtime_id: "runtime-active".to_owned(),
            workspace_id: "workspace-v2".to_owned(),
            enrollment_mode: "MESH_PAIRING".to_owned(),
            status: "ACTIVE".to_owned(),
            roles_json: r#"["EXECUTOR","WORKSPACE_HUB"]"#.to_owned(),
            created_at: "2026-10-10T10:01:00Z".to_owned(),
            activated_at: Some("2026-10-10T10:01:00Z".to_owned()),
            revoked_at: None,
            version: 1,
        },
        MigratedBinding {
            binding_id: "rwb_migrated_runtime-revoked".to_owned(),
            runtime_id: "runtime-revoked".to_owned(),
            workspace_id: "workspace-v2".to_owned(),
            enrollment_mode: "MESH_PAIRING".to_owned(),
            status: "REVOKED".to_owned(),
            roles_json: r#"["EXECUTOR"]"#.to_owned(),
            created_at: "2026-10-10T10:01:00Z".to_owned(),
            activated_at: None,
            revoked_at: Some("2026-10-10T10:01:00Z".to_owned()),
            version: 1,
        },
    ];
    assert_eq!(migrated_bindings(&connection), expected);

    migrate(&mut connection).expect("retry remaining migrations");
    let final_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read final schema version");
    assert_eq!(final_version, SCHEMA_VERSION);
    assert_eq!(migrated_bindings(&connection), expected);
}

#[test]
fn resource_index_migration_failure_is_atomic_and_retryable() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut connection = Connection::open(directory.path().join("migration-v7.sqlite3"))
        .expect("open test database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable FK");
    connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("enable WAL");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV7Migration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version after interrupted v7");
    assert_eq!(version, V6_SCHEMA_VERSION);
    let indexes_exist: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'resource_text_indexes')",
        [],
        |row| row.get(0),
    ).expect("check interrupted v7 rollback");
    assert!(!indexes_exist);

    migrate(&mut connection).expect("retry Resource index migration");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read final schema version");
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn schema_drift_and_unknown_newer_versions_fail_closed() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = state_database(&directory);
    let store = test_store(&directory, Duration::from_secs(1));
    drop(store);
    let connection = Connection::open(&path).expect("open database");
    connection
        .execute_batch("DROP TRIGGER workspace_default_agent_binding_insert_guard")
        .expect("remove trigger to simulate corruption");
    drop(connection);
    let blob_store = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));
    let result = SqliteWorkspaceStore::open(&path, blob_store, SqliteConfig::default());
    assert!(matches!(result, Err(StoreError::CorruptSchema(_))));

    let newer_path = directory.path().join("state").join("newer.sqlite3");
    let connection = Connection::open(&newer_path).expect("open future database");
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .expect("set newer schema marker");
    drop(connection);
    let blob_store = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));
    let result = SqliteWorkspaceStore::open(&newer_path, blob_store, SqliteConfig::default());
    assert!(
        matches!(result, Err(StoreError::UnsupportedSchema(version)) if version == SCHEMA_VERSION + 1)
    );

    let checksum_directory =
        tempfile::tempdir_in(directory.path()).expect("temporary checksum-drift directory");
    let checksum_path = state_database(&checksum_directory);
    let checksum_store = test_store(&checksum_directory, Duration::from_secs(1));
    drop(checksum_store);
    let connection = Connection::open(&checksum_path).expect("open migrated checksum database");
    connection
        .execute(
            "UPDATE schema_migrations SET source_checksum = ?1 WHERE version = 1",
            [format!("sha256:{}", "0".repeat(64))],
        )
        .expect("tamper with recorded migration source checksum");
    drop(connection);
    let blob_store = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));
    let result = SqliteWorkspaceStore::open(&checksum_path, blob_store, SqliteConfig::default());
    assert!(matches!(result, Err(StoreError::CorruptSchema(_))));
    let connection = Connection::open(&checksum_path).expect("reopen checksum database");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version after refused open");
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn external_writer_lock_returns_busy_without_partial_state() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_millis(30));
    create_workspace(&store, "workspace-busy");
    let lock = Connection::open(state_database(&directory)).expect("open lock connection");
    lock.execute_batch("BEGIN IMMEDIATE")
        .expect("hold writer lock");

    let result = service(store.clone()).change_replication_policy(ChangeReplicationPolicy {
        workspace_id: "workspace-busy".to_owned(),
        expected_version: 1,
        policy: ReplicationPolicy::MetadataOnly,
        replication_scope_root_ids: Vec::new(),
        event: context("event-busy", 1),
    });
    assert!(matches!(result, Err(StoreError::Busy)));
    lock.execute_batch("ROLLBACK").expect("release writer lock");

    let current = store
        .get_workspace("workspace-busy")
        .expect("read state")
        .expect("Workspace exists");
    assert_eq!(current.version, 1);
    assert_eq!(
        store
            .read_workspace_events("workspace-busy")
            .expect("read events")
            .len(),
        1
    );
}

#[test]
fn idempotent_workspace_policy_busy_leaves_no_receipt_and_retries_once() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_millis(30));
    let created = create_workspace(&store, "workspace-busy-idempotent");
    let lock = Connection::open(state_database(&directory)).expect("open lock connection");
    lock.execute_batch("BEGIN IMMEDIATE")
        .expect("hold external writer lock");

    let principal_id = "owner-local".to_owned();
    let request_id = "request-policy-busy-retry".to_owned();
    let policy = ReplicationPolicy::MetadataOnly;
    let request_payload = serde_json::json!({
        "operation": "workspace.replication_policy.change.v1",
        "workspace_id": "workspace-busy-idempotent",
        "expected_version": 1,
        "policy": "METADATA_ONLY",
    });
    let command = ChangeReplicationPolicy {
        workspace_id: "workspace-busy-idempotent".to_owned(),
        expected_version: 1,
        policy,
        replication_scope_root_ids: Vec::new(),
        event: context("event-policy-busy-retry", 1),
    };

    let failed = service(store.clone()).change_replication_policy_idempotent(
        command.clone(),
        principal_id.clone(),
        request_id.clone(),
        request_payload.clone(),
    );
    assert!(matches!(failed, Err(StoreError::Busy)));
    lock.execute_batch("ROLLBACK")
        .expect("release external writer lock");

    assert_eq!(
        store
            .get_workspace("workspace-busy-idempotent")
            .expect("read Workspace after Busy"),
        Some(created.workspace.clone())
    );
    let events_after_busy = store
        .read_workspace_events("workspace-busy-idempotent")
        .expect("read events after Busy");
    assert_eq!(events_after_busy.len(), 1);
    assert_eq!(events_after_busy[0].origin_sequence, 1);
    assert_eq!(
        store
            .rebuild_workspace_projection("workspace-busy-idempotent")
            .expect("replay after Busy"),
        Some(created.workspace.clone())
    );
    let connection =
        Connection::open(state_database(&directory)).expect("inspect receipt after Busy");
    let receipts_after_busy: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            [&principal_id, &request_id],
            |row| row.get(0),
        )
        .expect("count receipts after Busy");
    assert_eq!(receipts_after_busy, 0);
    drop(connection);

    let retried = service(store.clone())
        .change_replication_policy_idempotent(
            command.clone(),
            principal_id.clone(),
            request_id.clone(),
            request_payload.clone(),
        )
        .expect("retry identical policy request after lock release");
    assert_eq!(retried.workspace.version, 2);
    assert_eq!(retried.event.origin_sequence, 2);

    let replayed = service(store.clone())
        .change_replication_policy_idempotent(
            command,
            principal_id.clone(),
            request_id.clone(),
            request_payload,
        )
        .expect("replay the committed request");
    assert_eq!(replayed, retried);

    let events_after_replay = store
        .read_workspace_events("workspace-busy-idempotent")
        .expect("read events after replay");
    assert_eq!(events_after_replay.len(), 2);
    assert_eq!(events_after_replay[1].origin_sequence, 2);
    assert_eq!(
        store
            .rebuild_workspace_projection("workspace-busy-idempotent")
            .expect("replay committed policy update"),
        Some(retried.workspace.clone())
    );

    let connection =
        Connection::open(state_database(&directory)).expect("inspect committed request receipt");
    let (response_json, response_digest): (String, String) = connection
        .query_row(
            "SELECT response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            [&principal_id, &request_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read committed policy receipt");
    assert_eq!(response_digest, digest(response_json.as_bytes()));
    let stored_response: CommittedWorkspace =
        serde_json::from_str(&response_json).expect("decode committed policy response");
    assert_eq!(stored_response, retried);
}

#[test]
fn writer_metrics_record_send_wait_and_bounded_in_flight_pressure() {
    const WRITERS: usize = 24;

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_millis(120));
    create_workspace(&store, "workspace-pressure");
    let lock = Connection::open(state_database(&directory)).expect("open lock connection");
    lock.execute_batch("BEGIN IMMEDIATE")
        .expect("hold writer lock");

    let barrier = Arc::new(Barrier::new(WRITERS + 1));
    let writers: Vec<_> = (0..WRITERS)
        .map(|index| {
            let store = store.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                service(store).change_replication_policy(ChangeReplicationPolicy {
                    workspace_id: "workspace-pressure".to_owned(),
                    expected_version: 1,
                    policy: ReplicationPolicy::MetadataOnly,
                    replication_scope_root_ids: Vec::new(),
                    event: context(&format!("event-pressure-{index}"), 1),
                })
            })
        })
        .collect();
    barrier.wait();

    for writer in writers {
        assert!(matches!(
            writer.join().expect("writer thread joins"),
            Err(StoreError::Busy)
        ));
    }

    lock.execute_batch("ROLLBACK").expect("release writer lock");
    let metrics = store.writer_metrics_snapshot();
    assert_eq!(metrics.outstanding_commands, 0);
    assert!(metrics.outstanding_commands_peak > 8);
    assert!(metrics.send_wait_nanos_total > 10_000_000);
    assert!(metrics.send_wait_nanos_max > 10_000_000);
    assert_eq!(
        store
            .get_workspace("workspace-pressure")
            .expect("read state")
            .expect("Workspace exists")
            .version,
        1
    );
}

#[test]
fn workspace_policy_event_payload_is_closed_and_versioned() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-schema");
    service(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: "workspace-schema".to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: Vec::new(),
            event: context("event-schema", 1),
        })
        .expect("update policy");
    let events = store
        .read_workspace_events("workspace-schema")
        .expect("read events");
    assert_eq!(events[0].event_type, "workspace.created.v1");
    assert_eq!(
        events[1].event_type,
        "workspace.replication_policy.changed.v1"
    );
    assert_eq!(events[1].payload.as_object().expect("object").len(), 5);
    assert_eq!(events[1].payload["aggregate_version"], 2);
    assert_eq!(events[1].payload["from"], "LOCAL_ONLY");
    assert_eq!(events[1].payload["to"], "METADATA_ONLY");
}

#[test]
fn reports_blob_commit_failure_before_any_aggregate_write() {
    struct NoSpaceBlobStore;
    impl BlobStore for NoSpaceBlobStore {
        fn put(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            _bytes: &[u8],
            _media_type: &str,
        ) -> Result<storage_core::BlobRef, StoreError> {
            Err(StoreError::Io("simulated no space".to_owned()))
        }
        fn get(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            _blob: &storage_core::BlobRef,
        ) -> Result<Vec<u8>, StoreError> {
            unreachable!("put fails before get")
        }
    }

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("no-space").join("state.sqlite3"),
        Arc::new(NoSpaceBlobStore),
        SqliteConfig::default(),
    )
    .expect("database opens");
    let result = service(store.clone()).create(CreateWorkspace {
        workspace_id: "workspace-diskfull".to_owned(),
        name: "No space".to_owned(),
        owner_principal_id: "owner".to_owned(),
        event: context("event-no-space", 0),
    });
    assert!(matches!(result, Err(StoreError::Io(_))));
    assert!(
        store
            .get_workspace("workspace-diskfull")
            .expect("read database")
            .is_none()
    );
}

#[test]
fn sqlite_full_error_maps_safely_and_rolls_back_workspace_commit() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let created = create_workspace(&store, "workspace-sqlite-full");
    drop(store);

    let database = state_database(&directory);
    let mut connection = Connection::open(&database).expect("reopen migrated database");
    let page_count: u32 = connection
        .pragma_query_value(None, "page_count", |row| row.get(0))
        .expect("read page count");
    connection
        .pragma_update(None, "max_page_count", page_count)
        .expect("cap database at current page count");

    let mut changed = created.workspace.clone();
    changed.name = "x".repeat(1024 * 1024);
    changed.version = 2;
    changed.updated_at = "2026-10-07T12:00:00Z".to_owned();
    let event = EventDraft {
        event_id: "event-full-update".to_owned(),
        workspace_id: changed.workspace_id.clone(),
        entity_type: "Workspace".to_owned(),
        entity_id: changed.workspace_id.clone(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: 2,
        hlc_timestamp: "2026-10-07T12:00:00Z".to_owned(),
        correlation_id: "correlation-full".to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "workspace.replication_policy.changed.v1".to_owned(),
        payload: serde_json::json!({"padding": "x".repeat(1024 * 1024)}),
        recorded_at: "2026-10-07T12:00:00Z".to_owned(),
    };
    let state_ref = AggregateStateRef {
        blob: BlobRef {
            digest: format!("sha256:{}", "0".repeat(64)),
            size_bytes: 1,
            media_type: STATE_MEDIA_TYPE.to_owned(),
        },
        entity_revision: 2,
        record_schema_version: 1,
    };

    let result = commit_workspace_transaction(
        &mut connection,
        Some(1),
        changed.clone(),
        event.clone(),
        state_ref.clone(),
        None,
    );
    assert!(matches!(
        result,
        Err(StoreError::Io(ref message)) if message == "database disk is full"
    ));

    let version: i64 = connection
        .query_row(
            "SELECT version FROM workspaces WHERE workspace_id = ?1",
            ["workspace-sqlite-full"],
            |row| row.get(0),
        )
        .expect("read committed Workspace after failed transaction");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1",
            ["workspace-sqlite-full"],
            |row| row.get(0),
        )
        .expect("read events after failed transaction");
    let sequence: i64 = connection
        .query_row(
            "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = 'runtime-local'",
            ["workspace-sqlite-full"],
            |row| row.get(0),
        )
        .expect("read origin sequence after failed transaction");
    assert_eq!(version, 1);
    assert_eq!(event_count, 1);
    assert_eq!(sequence, 1);

    connection
        .pragma_update(None, "max_page_count", page_count.saturating_add(1024))
        .expect("restore database growth allowance");
    let retried =
        commit_workspace_transaction(&mut connection, Some(1), changed, event, state_ref, None)
            .expect("retry after capacity is restored");
    assert_eq!(retried.event.origin_sequence, 2);
    assert_eq!(retried.workspace.version, 2);

    let final_sequence: i64 = connection
        .query_row(
            "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = 'runtime-local'",
            ["workspace-sqlite-full"],
            |row| row.get(0),
        )
        .expect("read retried origin sequence");
    assert_eq!(final_sequence, 2);
}

#[test]
fn sqlite_full_through_writer_actor_preserves_state_and_allows_retry() {
    use storage_core::StateStore;

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let created = create_workspace(&store, "workspace-actor-sqlite-full");

    let (page_count_sender, page_count_receiver) = mpsc::channel();
    let page_count = store
        .execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let _ = page_count_sender.send(
                        connection
                            .pragma_query_value(None, "page_count", |row| row.get::<_, u32>(0))
                            .map_err(map_database_error),
                    );
                }),
            },
            page_count_receiver,
        )
        .expect("read page count on production writer");

    let (cap_sender, cap_receiver) = mpsc::channel();
    let applied_cap = store
        .execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let result = connection
                        .pragma_update(None, "max_page_count", page_count)
                        .map_err(map_database_error)
                        .and_then(|()| {
                            connection
                                .pragma_query_value(None, "max_page_count", |row| {
                                    row.get::<_, u32>(0)
                                })
                                .map_err(map_database_error)
                        });
                    let _ = cap_sender.send(result);
                }),
            },
            cap_receiver,
        )
        .expect("cap production writer database");
    assert_eq!(applied_cap, page_count);

    let mut changed = created.workspace.clone();
    changed.name = "x".repeat(1024 * 1024);
    changed.version = 2;
    changed.updated_at = "2026-10-12T12:00:00Z".to_owned();
    let event = EventDraft {
        event_id: "event-actor-full-update".to_owned(),
        workspace_id: changed.workspace_id.clone(),
        entity_type: "Workspace".to_owned(),
        entity_id: changed.workspace_id.clone(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: 2,
        hlc_timestamp: "2026-10-12T12:00:00Z".to_owned(),
        correlation_id: "correlation-actor-full".to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "workspace.replication_policy.changed.v1".to_owned(),
        payload: serde_json::json!({"padding": "x".repeat(1024 * 1024)}),
        recorded_at: "2026-10-12T12:00:00Z".to_owned(),
    };

    let failed = store.commit_workspace(Some(1), changed.clone(), event.clone());
    assert!(matches!(
        failed,
        Err(StoreError::Io(ref message)) if message == "database disk is full"
    ));
    assert_eq!(
        store
            .get_workspace("workspace-actor-sqlite-full")
            .expect("read after failed commit"),
        Some(created.workspace.clone())
    );
    let events_after_failure = store
        .read_workspace_events("workspace-actor-sqlite-full")
        .expect("read events after failed commit");
    assert_eq!(events_after_failure.len(), 1);
    assert_eq!(events_after_failure[0].origin_sequence, 1);
    assert_eq!(
        store
            .rebuild_workspace_projection("workspace-actor-sqlite-full")
            .expect("replay after failed commit"),
        Some(created.workspace)
    );

    let (restore_sender, restore_receiver) = mpsc::channel();
    store
        .execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let current_pages = connection
                        .pragma_query_value(None, "page_count", |row| row.get::<_, u32>(0))
                        .map_err(map_database_error);
                    let result = current_pages.and_then(|pages| {
                        connection
                            .pragma_update(None, "max_page_count", pages.saturating_add(1024))
                            .map_err(map_database_error)
                    });
                    let _ = restore_sender.send(result);
                }),
            },
            restore_receiver,
        )
        .expect("restore production writer capacity");

    let retried = store
        .commit_workspace(Some(1), changed, event)
        .expect("retry exact revision through production writer");
    assert_eq!(retried.workspace.version, 2);
    assert_eq!(retried.event.origin_sequence, 2);
    let final_events = store
        .read_workspace_events("workspace-actor-sqlite-full")
        .expect("read retried event stream");
    assert_eq!(final_events.len(), 2);
    assert_eq!(final_events[1].origin_sequence, 2);
    assert_eq!(
        store
            .rebuild_workspace_projection("workspace-actor-sqlite-full")
            .expect("replay retried commit"),
        Some(retried.workspace)
    );
}

#[test]
fn sqlite_full_workspace_policy_command_rolls_back_idempotency_receipt_and_retries() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let created = create_workspace(&store, "workspace-policy-sqlite-full");
    let database = state_database(&directory);
    let principal_id = "owner-local".to_owned();

    let (page_count_sender, page_count_receiver) = mpsc::channel();
    let page_count = store
        .execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let _ = page_count_sender.send(
                        connection
                            .pragma_query_value(None, "page_count", |row| row.get::<_, u32>(0))
                            .map_err(map_database_error),
                    );
                }),
            },
            page_count_receiver,
        )
        .expect("read page count on production writer");

    let (cap_sender, cap_receiver) = mpsc::channel();
    let applied_cap = store
        .execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let result = connection
                        .pragma_update(None, "max_page_count", page_count)
                        .map_err(map_database_error)
                        .and_then(|()| {
                            connection
                                .pragma_query_value(None, "max_page_count", |row| {
                                    row.get::<_, u32>(0)
                                })
                                .map_err(map_database_error)
                        });
                    let _ = cap_sender.send(result);
                }),
            },
            cap_receiver,
        )
        .expect("cap production writer database");
    assert_eq!(applied_cap, page_count);

    let mut committed = created.clone();
    let policies = [
        ReplicationPolicy::MetadataOnly,
        ReplicationPolicy::ActiveTaskInputs,
        ReplicationPolicy::FullWorkspace,
    ];
    let mut failed_command = None;
    for index in 0..10_000_u64 {
        let policy = policies[index as usize % policies.len()].clone();
        let request_id = format!("request-policy-sqlite-full-{index}");
        let request_payload = serde_json::json!({
            "operation": "workspace.replication_policy.change.v1",
            "workspace_id": committed.workspace.workspace_id,
            "expected_version": committed.workspace.version,
            "policy": policy.as_str(),
        });
        let command = ChangeReplicationPolicy {
            workspace_id: committed.workspace.workspace_id.clone(),
            expected_version: committed.workspace.version,
            policy,
            replication_scope_root_ids: Vec::new(),
            event: context(&format!("event-policy-sqlite-full-{index}"), 1),
        };
        match service(store.clone()).change_replication_policy_idempotent(
            command.clone(),
            principal_id.clone(),
            request_id.clone(),
            request_payload.clone(),
        ) {
            Ok(next) => committed = next,
            Err(StoreError::Io(message)) if message == "database disk is full" => {
                failed_command = Some((command, request_id, request_payload));
                break;
            }
            Err(error) => {
                panic!("unexpected policy command failure at iteration {index}: {error:?}")
            }
        }
    }
    let (command, request_id, request_payload) =
        failed_command.expect("SQLite page cap eventually rejects a service command");

    assert_eq!(
        store
            .get_workspace(&created.workspace.workspace_id)
            .expect("read Workspace after failed command"),
        Some(committed.workspace.clone())
    );
    let events_after_failure = store
        .read_workspace_events(&created.workspace.workspace_id)
        .expect("read events after failed command");
    assert_eq!(
        events_after_failure.len() as u64,
        committed.workspace.version
    );
    assert_eq!(
        events_after_failure
            .last()
            .map(|event| event.origin_sequence),
        Some(committed.event.origin_sequence)
    );
    assert_eq!(
        store
            .rebuild_workspace_projection(&created.workspace.workspace_id)
            .expect("replay after failed command"),
        Some(committed.workspace.clone())
    );
    let connection = Connection::open(&database).expect("inspect failed receipt");
    let failed_receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            [&principal_id, &request_id],
            |row| row.get(0),
        )
        .expect("count failed command receipts");
    assert_eq!(failed_receipt_count, 0);
    drop(connection);

    let (restore_sender, restore_receiver) = mpsc::channel();
    store
        .execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let current_pages = connection
                        .pragma_query_value(None, "page_count", |row| row.get::<_, u32>(0))
                        .map_err(map_database_error);
                    let result = current_pages.and_then(|pages| {
                        connection
                            .pragma_update(None, "max_page_count", pages.saturating_add(1024))
                            .map_err(map_database_error)
                    });
                    let _ = restore_sender.send(result);
                }),
            },
            restore_receiver,
        )
        .expect("restore production writer capacity");

    let retried = service(store.clone())
        .change_replication_policy_idempotent(
            command.clone(),
            principal_id.clone(),
            request_id.clone(),
            request_payload.clone(),
        )
        .expect("retry exact policy command through WorkspaceService");
    assert_eq!(retried.workspace.version, committed.workspace.version + 1);
    assert_eq!(retried.workspace.replication_policy, command.policy);
    assert_eq!(
        retried.event.origin_sequence,
        committed.event.origin_sequence + 1
    );

    let replayed_receipt = service(store.clone())
        .change_replication_policy_idempotent(
            command,
            principal_id.clone(),
            request_id.clone(),
            request_payload,
        )
        .expect("same request returns its original committed response");
    assert_eq!(replayed_receipt, retried);

    let connection = Connection::open(&database).expect("inspect committed receipt");
    let (response_json, response_digest): (String, String) = connection
        .query_row(
            "SELECT response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            [&principal_id, &request_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read committed command receipt");
    assert_eq!(response_digest, digest(response_json.as_bytes()));
    let stored_response: CommittedWorkspace =
        serde_json::from_str(&response_json).expect("decode idempotent response");
    assert_eq!(stored_response, retried);

    let final_events = store
        .read_workspace_events(&created.workspace.workspace_id)
        .expect("read events after receipt replay");
    assert_eq!(final_events.len() as u64, retried.workspace.version);
    assert_eq!(
        final_events.last().map(|event| event.origin_sequence),
        Some(retried.event.origin_sequence)
    );
    assert_eq!(
        store
            .rebuild_workspace_projection(&created.workspace.workspace_id)
            .expect("replay after successful retry"),
        Some(retried.workspace)
    );
}
