#[path = "conversation_turn_tests.rs"]
mod conversation_turn_tests;
#[path = "rich_presentation_tests.rs"]
mod rich_presentation_tests;
use super::*;
#[path = "artifact_tests.rs"]
mod artifact_tests;
#[path = "coworker_tests.rs"]
mod coworker_tests;
#[path = "delegation_profile_tests.rs"]
mod delegation_profile_tests;
#[path = "environment_tests.rs"]
mod environment_tests;
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
    CommittedWorkspace, EventDraft, ReplicationPolicy, StoreError, TaskPlanningSessionStart,
    Workspace,
};
use zeroize::Zeroizing;

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
            request_id: "conversation-create-1".into(),
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
    let replay = adapter
        .create_conversation(commit)
        .expect("same request replays exact result");
    assert_eq!(created.conversation, record);
    assert_eq!(replay.event.event_id, created.event.event_id);
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
    for table in [
        "workspaces",
        "domain_events",
        "schema_migrations",
        "workspace_origin_sequences",
        "resource_text_indexes",
        "resource_text_index_terms",
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
