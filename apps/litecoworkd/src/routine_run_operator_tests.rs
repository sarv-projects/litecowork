//! Route-level evidence for an authenticated manual Routine run.
//!
//! These tests drive the real `build_operator_router` with the real SQLite
//! store. They prove authentication and owner admission precede receipt
//! replay, the changed-payload conflict, the CREATED TaskView shape, and that
//! an exact retry still succeeds after the Routine is archived while a new run
//! is refused.

use super::*;

use axum::body::to_bytes;
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use rusqlite::{Connection, params};
use sha2::Digest;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;
use storage_core::{
    BlobPurpose, DeviceIdentityRecord, LocalRuntimeWorkspaceEnrollmentRequest,
    RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
    RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
    RuntimeWorkspaceBindingStore, WorkspaceCreateRequest,
};
use storage_sqlite::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
use zeroize::Zeroizing;

const WORKSPACE: &str = "routine-run-route-workspace";
const OWNER: &str = "routine-run-route-owner";
const RUNTIME: &str = "routine-run-route-runtime";
const INCARNATION: &str = "routine-run-route-incarnation";
const NOW: &str = "2026-10-21T12:00:00Z";
const ROUTINE: &str = "routine-1";
const RUN_PATH: &str = "/v1/routines/routine-1/run";
const AUTOMATION: &str = "automation-1";
const TEST_RUN_PATH: &str = "/v1/automations/automation-1/test-runs";

#[derive(Clone)]
struct RoutineRunTestKeys;

impl WorkspaceBlobKeyProvider for RoutineRunTestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([0x36; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob(
                "unknown Routine route test key version".to_owned(),
            ));
        }
        self.current_key(workspace_id, purpose)
    }
}

/// A real store with an owner-enrolled local Runtime, one lead AgentBinding,
/// one ACTIVE Routine revision, and the full Operator router.
fn fixture() -> (tempfile::TempDir, SqliteWorkspaceStore, axum::Router) {
    let directory = tempfile::tempdir().expect("temporary Routine route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private Routine route directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            RoutineRunTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Routine run route fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "routine-run-route-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "routine-run-route-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner Workspace commits");

    let public_key = [0x36; 32];
    store
        .register_local_incarnation(
            RuntimeRecord {
                runtime_id: RUNTIME.to_owned(),
                device_identity: DeviceIdentityRecord {
                    device_id: format!(
                        "ed25519-sha256:{}",
                        hex::encode(sha2::Sha256::digest(public_key))
                    ),
                    public_key: format!("ed25519:{}", hex::encode(public_key)),
                    key_version: 1,
                    issued_at: NOW.to_owned(),
                    display_name: None,
                },
                runtime_version: "0.1.0-test".to_owned(),
                platform: std::env::consts::OS.to_owned(),
                architecture: std::env::consts::ARCH.to_owned(),
                roles: vec!["OPERATOR_ENDPOINT".to_owned()],
                trust_zone: "PERSONAL_DEVICE".to_owned(),
                availability: "RECOVERING".to_owned(),
                startup_policy: "MANUAL".to_owned(),
                current_incarnation_id: INCARNATION.to_owned(),
                resource_capacity: json!({"sampled_at": NOW}),
                last_seen: NOW.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: INCARNATION.to_owned(),
                runtime_id: RUNTIME.to_owned(),
                process_started_at: NOW.to_owned(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: INCARNATION.to_owned(),
                os_boot_id: Some("routine-run-route-boot".to_owned()),
                observed_at: NOW.to_owned(),
            },
        )
        .expect("local Runtime registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: INCARNATION.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("local Runtime becomes available");
    store
        .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "routine-run-route-enrollment".to_owned(),
                request_payload: json!({"workspace_id": WORKSPACE}),
            },
            expected_workspace_version: 1,
            runtime_workspace_binding_id: "routine-run-route-binding".to_owned(),
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: INCARNATION.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            now: NOW.to_owned(),
            correlation_id: "routine-run-route-enrollment".to_owned(),
        })
        .expect("owner enrolls local Runtime for the Workspace");

    // AgentBinding and Routine rows share the state database with the store.
    let connection =
        Connection::open(directory.path().join("state.sqlite3")).expect("fixture database opens");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at)
             VALUES ('profile-routine-run', 'fixture', 'Fixture Agent', '2026-10-20T08:00:00Z')",
            [],
        )
        .expect("fixture AgentProfile");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version)
             VALUES ('binding-routine-run', ?1, 'profile-routine-run', '{}', '{}', 1, 1, '2026-10-20T08:00:00Z', 1)",
            params![WORKSPACE],
        )
        .expect("fixture AgentBinding");
    connection
        .execute(
            "UPDATE workspaces SET default_agent_binding_id = 'binding-routine-run' WHERE workspace_id = ?1",
            params![WORKSPACE],
        )
        .expect("the Workspace default binding is set");
    let tx = connection
        .unchecked_transaction()
        .expect("fixture Routine transaction opens");
    tx.execute(
        "INSERT INTO routines(routine_id, workspace_id, name, current_revision, status, created_at, updated_at, version)
         VALUES (?2, ?1, 'Routine', 1, 'ACTIVE', '2026-10-20T08:00:00Z', '2026-10-20T08:00:00Z', 1)",
        params![WORKSPACE, ROUTINE],
    )
    .expect("fixture Routine");
    tx.execute(
        "INSERT INTO routine_revisions(workspace_id, routine_id, revision, objective_template, instructions, input_schema_json, constraints_json, non_goals_json, required_outputs_json, acceptance_criteria_json, approvals_required_json, input_bindings_json, required_capabilities_json, preferred_agent_binding_id, placement_preference_json, budget_ceiling_json, verification_policy_json, authored_by_json, created_at)
         VALUES (?1, ?2, 1, 'Review project', 'Review the project', '{}', '[]', '[]', '[]', '[]', '[]', '[]', '[]', NULL, '\"AUTO\"', NULL, '{}', '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', '2026-10-20T08:00:00Z')",
        params![WORKSPACE, ROUTINE],
    )
    .expect("fixture Routine revision");
    tx.execute(
        "INSERT INTO automations(automation_id, workspace_id, name, current_revision, status, created_at, updated_at, version)
         VALUES (?1, ?2, 'Test Automation', 1, 'PAUSED', '2026-10-20T08:00:00Z', '2026-10-20T08:00:00Z', 1)",
        params![AUTOMATION, WORKSPACE],
    )
    .expect("fixture Automation");
    tx.execute(
        "INSERT INTO automation_revisions(workspace_id, automation_id, revision, routine_id, routine_revision, coworker_id, coworker_revision, triggers_json, execution_policy_json, authored_by_json, created_at)
         VALUES (?1, ?2, 1, ?3, 1, NULL, NULL, '[]', '{\"placement_preference\":\"AUTO\",\"max_concurrent_occurrences\":1,\"overlap_policy\":\"SKIP\",\"retry_policy\":{\"max_attempts\":1,\"initial_backoff_ms\":100,\"max_backoff_ms\":100,\"multiplier\":1.0,\"jitter\":false,\"retryable_error_codes\":[]},\"budget_ceiling\":null,\"notification_policy\":\"ON_FAILURE\",\"wake_policy\":\"NEVER\"}', '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', '2026-10-20T08:00:00Z')",
        params![WORKSPACE, AUTOMATION, ROUTINE],
    )
    .expect("fixture Automation revision");
    tx.commit().expect("fixture Routine commits");
    drop(connection);

    let (_, stream_shutdown) = tokio::sync::watch::channel(false);
    let router = build_operator_router(ApiState {
        store: store.clone(),
        principal_id: OWNER.to_owned(),
        runtime_id: RUNTIME.to_owned(),
        local_incarnation_id: INCARNATION.to_owned(),
        lifecycle_blockers: vec![],
        runtime_identity: Arc::new(
            storage_sqlite::RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                .expect("test Runtime identity"),
        ),
        stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
        runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        stream_shutdown,
    });
    (directory, store, router)
}

fn run_body(inputs: serde_json::Value) -> String {
    json!({"routine_revision": 1, "inputs": inputs}).to_string()
}

/// Builds an authenticated local-IPC request. The `AuthenticatedLocalPeer`
/// extension is what the production transport inserts after peer authentication.
fn run_request(
    workspace_id: &str,
    idempotency_key: Option<&str>,
    body: &str,
    authenticated: bool,
) -> Request<Body> {
    let mut builder = Request::post(RUN_PATH)
        .header("x-workspace-id", workspace_id)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(key) = idempotency_key {
        builder = builder.header("idempotency-key", key);
    }
    let mut request = builder
        .body(Body::from(body.to_owned()))
        .expect("Routine run request is valid HTTP");
    if authenticated {
        request.extensions_mut().insert(AuthenticatedLocalPeer);
    }
    request
}

async fn send(router: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("Operator router responds");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 256 * 1024)
        .await
        .expect("Routine run response body reads");
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

fn count(connection: &Connection, sql: &str) -> i64 {
    connection
        .query_row(sql, [], |row| row.get(0))
        .expect("count query runs")
}

#[tokio::test]
async fn routine_run_route_authenticates_replays_and_refuses_changed_or_archived_runs() {
    let (directory, _store, router) = fixture();
    let original = run_body(json!({}));
    let key = "routine-run-route-key";

    // Authentication precedes every read, including an already committed receipt.
    let (status, _) = send(&router, run_request(WORKSPACE, Some(key), &original, false)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // The selected Workspace must be owned by the authenticated principal.
    let (status, _) = send(
        &router,
        run_request("another-workspace", Some(key), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The Workspace header and Idempotency-Key are required before any write.
    let (status, _) = send(&router, run_request(WORKSPACE, None, &original, true)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        &router,
        run_request(WORKSPACE, Some(key), &run_body(json!([])), true),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, first) = send(&router, run_request(WORKSPACE, Some(key), &original, true)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(first["task"]["status"], "READY");
    assert_eq!(first["task"]["routine_id"], ROUTINE);
    assert_eq!(first["task"]["routine_revision"], 1);
    assert_eq!(first["task"]["version"], 1);
    assert_eq!(first["task"]["automation_id"], Value::Null);
    assert_eq!(first["task"]["automation_occurrence_id"], Value::Null);
    assert_eq!(first["current_spec_revision"]["revision"], 1);
    assert_eq!(
        first["current_spec_revision"]["objective"],
        "Review project"
    );
    let task_id = first["task"]["task_id"]
        .as_str()
        .expect("the CREATED Task carries its id")
        .to_owned();

    // The same key and payload replay the committed Task even without auth;
    // authentication still refuses it.
    let (status, _) = send(&router, run_request(WORKSPACE, Some(key), &original, false)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, replay) = send(&router, run_request(WORKSPACE, Some(key), &original, true)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(replay, first);

    // The same key with changed inputs is a conflict and cannot replace the receipt.
    let (status, _) = send(
        &router,
        run_request(WORKSPACE, Some(key), &run_body(json!({"topic": "x"})), true),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // A new key creates a second independent READY Task.
    let (status, second) = send(
        &router,
        run_request(WORKSPACE, Some("routine-run-route-key-2"), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(second["task"]["status"], "READY");
    assert_ne!(second["task"]["task_id"], first["task"]["task_id"]);

    // Archiving the Routine makes a new run stale, but the exact committed
    // retry still replays because the receipt is resolved before admission.
    let connection =
        Connection::open(directory.path().join("state.sqlite3")).expect("state database opens");
    connection
        .execute(
            "UPDATE routines SET status = 'ARCHIVED', version = version + 1 WHERE workspace_id = ?1 AND routine_id = ?2",
            params![WORKSPACE, ROUTINE],
        )
        .expect("the Routine transitions to ARCHIVED");
    let (status, replay_after_archive) =
        send(&router, run_request(WORKSPACE, Some(key), &original, true)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(replay_after_archive, first);
    let (status, _) = send(
        &router,
        run_request(WORKSPACE, Some("routine-run-route-key-3"), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Two ordinary Tasks, two specs, two events; no planning or execution rows.
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM tasks WHERE workspace_id = 'routine-run-route-workspace'"
        ),
        2
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM task_spec_revisions WHERE workspace_id = 'routine-run-route-workspace'"
        ),
        2
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = 'routine-run-route-workspace' AND type = 'task.created.v1'"
        ),
        2
    );
    for sql in [
        "SELECT COUNT(*) FROM plan_revisions",
        "SELECT COUNT(*) FROM steps",
        "SELECT COUNT(*) FROM attempts",
        "SELECT COUNT(*) FROM agent_sessions WHERE task_id IS NOT NULL",
    ] {
        assert_eq!(count(&connection, sql), 0, "unexpected rows for {sql}");
    }
    let stored_task: (String, Option<String>, Option<i64>, i64) = connection
        .query_row(
            "SELECT status, routine_id, routine_revision, version FROM tasks WHERE task_id = ?1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("the CREATED Task is persisted");
    assert_eq!(stored_task.0, "READY");
    assert_eq!(stored_task.1.as_deref(), Some(ROUTINE));
    assert_eq!(stored_task.2, Some(1));
    assert_eq!(stored_task.3, 1);
}


fn test_run_request(
    workspace_id: &str,
    idempotency_key: Option<&str>,
    if_match: Option<&str>,
    body: &str,
    authenticated: bool,
) -> Request<Body> {
    let mut builder = Request::post(TEST_RUN_PATH)
        .header("x-workspace-id", workspace_id)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(key) = idempotency_key {
        builder = builder.header("idempotency-key", key);
    }
    if let Some(version) = if_match {
        builder = builder.header("if-match", version);
    }
    let mut request = builder
        .body(Body::from(body.to_owned()))
        .expect("Automation test-run request is valid HTTP");
    if authenticated {
        request.extensions_mut().insert(AuthenticatedLocalPeer);
    }
    request
}

fn test_run_body() -> String {
    test_run_body_for(1)
}

fn test_run_body_for(revision: u64) -> String {
    json!({"automation_revision": revision, "inputs": {}}).to_string()
}

#[tokio::test]
async fn automation_test_run_creates_one_ready_task_without_occurrence_or_cursor() {
    let (directory, _store, router) = fixture();
    let key = "automation-test-run-route-key";
    let original = test_run_body();

    let (status, _) = send(
        &router,
        test_run_request(WORKSPACE, Some(key), Some("1"), &original, false),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(
        &router,
        test_run_request("another-workspace", Some(key), Some("1"), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &router,
        test_run_request(WORKSPACE, None, Some("1"), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        &router,
        test_run_request(WORKSPACE, Some(key), None, &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        &router,
        test_run_request(
            WORKSPACE,
            Some(key),
            Some("1"),
            &json!({"automation_revision": 1, "inputs": []}).to_string(),
            true,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, first) = send(
        &router,
        test_run_request(WORKSPACE, Some(key), Some("1"), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    assert_eq!(first["automation_id"], AUTOMATION);
    assert_eq!(first["automation_revision"], 1);
    assert_eq!(first["task"]["task"]["status"], "READY");
    assert_eq!(first["task"]["task"]["routine_id"], ROUTINE);
    assert_eq!(first["task"]["task"]["routine_revision"], 1);
    assert_eq!(first["task"]["task"]["automation_id"], Value::Null);
    assert_eq!(first["task"]["task"]["automation_occurrence_id"], Value::Null);
    assert!(first["task"]["task"]["automation_test_run_id"].is_string());
    assert_eq!(first["task"]["current_spec_revision"]["revision"], 1);
    let task_id = first["task"]["task"]["task_id"]
        .as_str()
        .expect("the CREATED test Task carries its id")
        .to_owned();

    let (status, replay) = send(
        &router,
        test_run_request(WORKSPACE, Some(key), Some("1"), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay, first);

    let (status, _) = send(
        &router,
        test_run_request(
            WORKSPACE,
            Some(key),
            Some("1"),
            &json!({"automation_revision": 1, "inputs": {"changed": true}}).to_string(),
            true,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Advancing the immutable revision must not break an exact receipt replay:
    // the exact revision is resolved first, and mutable head checks follow it.
    let connection =
        Connection::open(directory.path().join("state.sqlite3")).expect("state database opens");
    connection
        .execute(
            "INSERT INTO automation_revisions(workspace_id, automation_id, revision, routine_id,
                routine_revision, coworker_id, coworker_revision, triggers_json,
                execution_policy_json, authored_by_json, created_at)
             VALUES (?1, ?2, 2, ?3, 1, NULL, NULL, '[]',
                '{\"placement_preference\":\"AUTO\",\"max_concurrent_occurrences\":1,\"overlap_policy\":\"SKIP\",\"retry_policy\":{\"max_attempts\":1,\"initial_backoff_ms\":100,\"max_backoff_ms\":100,\"multiplier\":1.0,\"jitter\":false,\"retryable_error_codes\":[]},\"budget_ceiling\":null,\"notification_policy\":\"ON_FAILURE\",\"wake_policy\":\"NEVER\"}',
                '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', '2026-10-21T12:01:00Z')",
            params![WORKSPACE, AUTOMATION, ROUTINE],
        )
        .expect("fixture Automation revision 2");
    connection
        .execute(
            "UPDATE automations SET current_revision = 2, version = 2 WHERE automation_id = ?1",
            params![AUTOMATION],
        )
        .expect("the Automation advances to revision 2");
    let (status, replay_after_advance) = send(
        &router,
        test_run_request(WORKSPACE, Some(key), Some("1"), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay_after_advance, first);
    let (status, _) = send(
        &router,
        test_run_request(
            WORKSPACE,
            Some("automation-test-run-stale-revision-key"),
            Some("2"),
            &test_run_body_for(1),
            true,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // A disabled Automation refuses a new test run, but the committed receipt
    // still replays exactly.
    connection
        .execute(
            "UPDATE automations SET status = 'DISABLED', version = 3 WHERE automation_id = ?1",
            params![AUTOMATION],
        )
        .expect("the Automation transitions to DISABLED");
    let (status, _) = send(
        &router,
        test_run_request(
            WORKSPACE,
            Some("automation-test-run-disabled-key"),
            Some("3"),
            &test_run_body_for(2),
            true,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, replay_after_disable) = send(
        &router,
        test_run_request(WORKSPACE, Some(key), Some("1"), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay_after_disable, first);

    // Archiving the Workspace refuses a new test run, while the exact committed
    // receipt remains readable before mutable Workspace-status admission.
    connection
        .execute(
            "UPDATE workspaces SET status = 'ARCHIVED', version = version + 1 WHERE workspace_id = ?1",
            params![WORKSPACE],
        )
        .expect("the Workspace transitions to ARCHIVED");
    let (status, _) = send(
        &router,
        test_run_request(
            WORKSPACE,
            Some("automation-test-run-archived-key"),
            Some("3"),
            &test_run_body_for(2),
            true,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, replay_after_archive) = send(
        &router,
        test_run_request(WORKSPACE, Some(key), Some("1"), &original, true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay_after_archive, first);

    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM tasks WHERE workspace_id = 'routine-run-route-workspace'"
        ),
        1
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM task_spec_revisions WHERE workspace_id = 'routine-run-route-workspace'"
        ),
        1
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM automation_test_runs WHERE workspace_id = 'routine-run-route-workspace'"
        ),
        1
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM automation_occurrences WHERE workspace_id = 'routine-run-route-workspace'"
        ),
        0
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM automation_cursors WHERE workspace_id = 'routine-run-route-workspace'"
        ),
        0
    );
    for sql in [
        "SELECT COUNT(*) FROM plan_revisions",
        "SELECT COUNT(*) FROM steps",
        "SELECT COUNT(*) FROM attempts",
        "SELECT COUNT(*) FROM agent_sessions WHERE task_id IS NOT NULL",
    ] {
        assert_eq!(count(&connection, sql), 0, "unexpected rows for {sql}");
    }
    let stored: (String, Option<String>, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT status, automation_id, automation_occurrence_id, automation_test_run_id
             FROM tasks WHERE task_id = ?1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("the test Task is persisted");
    assert_eq!(stored.0, "READY");
    assert_eq!(stored.1, None);
    assert_eq!(stored.2, None);
    assert!(stored.3.is_some());
}
