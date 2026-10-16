use super::*;

use std::path::Path;

const OWNER: &str = "workspace-key-bounds-owner";
const RUNTIME: &str = "workspace-key-bounds-runtime";
const NOW: &str = "2026-10-15T12:00:00Z";

fn open_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private test data directory");
    SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            IpcReplayTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens")
}

fn create_workspace(store: &SqliteWorkspaceStore, workspace_id: &str) {
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Workspace key-bound route fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: format!("{workspace_id}-created"),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: format!("{workspace_id}-created"),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner Workspace commits");
}

fn request_receipt_count(directory: &tempfile::TempDir, request_id: &str) -> i64 {
    rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open receipt database")
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            rusqlite::params![OWNER, request_id],
            |row| row.get(0),
        )
        .expect("count owner request receipts")
}

fn event_count(directory: &tempfile::TempDir, workspace_id: &str, event_type: &str) -> i64 {
    rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open event database")
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = ?2",
            rusqlite::params![workspace_id, event_type],
            |row| row.get(0),
        )
        .expect("count Workspace events")
}

async fn archive_request(router: Router, workspace_id: &str, request_id: &str) -> Response {
    let mut request = Request::post(format!("/v1/workspaces/{workspace_id}/archive"))
        .header("x-workspace-id", workspace_id)
        .header("if-match", "1")
        .header("idempotency-key", request_id)
        .body(Body::empty())
        .expect("archive request is valid HTTP");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .oneshot(request)
        .await
        .expect("archive route responds")
}

#[tokio::test]
async fn archive_route_accepts_129_and_200_byte_keys_and_rejects_201_without_writes() {
    let directory = tempfile::tempdir().expect("temporary archive route directory");
    let store = open_store(&directory);
    for workspace_id in ["archive-key-129", "archive-key-200", "archive-key-201"] {
        create_workspace(&store, workspace_id);
    }
    let router = workspace_archive_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceArchiveApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
        });

    for (workspace_id, length) in [("archive-key-129", 129), ("archive-key-200", 200)] {
        let key = "k".repeat(length);
        let response = archive_request(router.clone(), workspace_id, &key).await;
        assert_eq!(response.status(), StatusCode::OK, "{length}-byte key");
        assert_eq!(request_receipt_count(&directory, &key), 1);
        assert_eq!(
            event_count(&directory, workspace_id, "workspace.archived.v1"),
            1
        );

        let replay = archive_request(router.clone(), workspace_id, &key).await;
        assert_eq!(replay.status(), StatusCode::OK, "{length}-byte replay");
        assert_eq!(request_receipt_count(&directory, &key), 1);
        assert_eq!(
            event_count(&directory, workspace_id, "workspace.archived.v1"),
            1,
            "exact replay does not duplicate the archive event"
        );
    }

    let too_long_key = "k".repeat(201);
    let rejected = archive_request(router, "archive-key-201", &too_long_key).await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(rejected.into_body(), 4096)
        .await
        .expect("bounded invalid-key response");
    assert_eq!(
        serde_json::from_slice::<Value>(&body).expect("error JSON")["code"],
        "INVALID_ARGUMENT"
    );
    let workspace = store
        .get_workspace("archive-key-201")
        .expect("read rejected Workspace")
        .expect("rejected Workspace remains stored");
    assert_eq!(workspace.status, "ACTIVE");
    assert_eq!(workspace.version, 1);
    assert_eq!(request_receipt_count(&directory, &too_long_key), 0);
    assert_eq!(
        event_count(&directory, "archive-key-201", "workspace.archived.v1"),
        0
    );
}

async fn policy_request(
    router: Router,
    workspace_id: &str,
    expected_version: u64,
    request_id: &str,
    policy: &str,
) -> Response {
    let mut request = Request::patch(format!("/v1/workspaces/{workspace_id}"))
        .header("x-workspace-id", workspace_id)
        .header("if-match", expected_version.to_string())
        .header("idempotency-key", request_id)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"replication_policy": policy}).to_string(),
        ))
        .expect("policy request is valid HTTP");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .oneshot(request)
        .await
        .expect("policy route responds")
}

#[tokio::test]
async fn policy_route_accepts_129_and_200_byte_keys_and_rejects_201_without_writes() {
    let directory = tempfile::tempdir().expect("temporary policy route directory");
    let store = open_store(&directory);
    const WORKSPACE: &str = "policy-key-bounds-workspace";
    create_workspace(&store, WORKSPACE);
    let router = workspace_policy_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspacePolicyApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
        });

    for (length, version, expected_events, policy) in [
        (129, 1_u64, 1_i64, "METADATA_ONLY"),
        (200, 2_u64, 2_i64, "FULL_WORKSPACE"),
    ] {
        let key = "p".repeat(length);
        let response = policy_request(router.clone(), WORKSPACE, version, &key, policy).await;
        assert_eq!(response.status(), StatusCode::OK, "{length}-byte key");
        assert_eq!(request_receipt_count(&directory, &key), 1);
        assert_eq!(
            event_count(
                &directory,
                WORKSPACE,
                "workspace.replication_policy.changed.v1"
            ),
            expected_events
        );

        let replay = policy_request(router.clone(), WORKSPACE, version, &key, policy).await;
        assert_eq!(replay.status(), StatusCode::OK, "{length}-byte replay");
        assert_eq!(request_receipt_count(&directory, &key), 1);
        assert_eq!(
            event_count(
                &directory,
                WORKSPACE,
                "workspace.replication_policy.changed.v1"
            ),
            expected_events,
            "exact replay does not duplicate the policy event"
        );
    }

    let too_long_key = "p".repeat(201);
    let rejected = policy_request(router, WORKSPACE, 3, &too_long_key, "METADATA_ONLY").await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(rejected.into_body(), 4096)
        .await
        .expect("bounded invalid-key response");
    assert_eq!(
        serde_json::from_slice::<Value>(&body).expect("error JSON")["code"],
        "INVALID_ARGUMENT"
    );
    let workspace = store
        .get_workspace(WORKSPACE)
        .expect("read policy Workspace")
        .expect("Workspace remains stored");
    assert_eq!(workspace.replication_policy.as_str(), "FULL_WORKSPACE");
    assert_eq!(workspace.version, 3);
    assert_eq!(request_receipt_count(&directory, &too_long_key), 0);
    assert_eq!(
        event_count(
            &directory,
            WORKSPACE,
            "workspace.replication_policy.changed.v1"
        ),
        2
    );
}

async fn default_agent_request(
    router: Router,
    workspace_id: &str,
    expected_version: u64,
    request_id: &str,
    binding_id: Value,
) -> Response {
    let mut request = Request::patch(format!(
        "/v1/workspaces/{workspace_id}/default-agent-binding"
    ))
    .header("x-workspace-id", workspace_id)
    .header("if-match", expected_version.to_string())
    .header("idempotency-key", request_id)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
        json!({"agent_binding_id": binding_id}).to_string(),
    ))
    .expect("default AgentBinding request is valid HTTP");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .oneshot(request)
        .await
        .expect("default AgentBinding route responds")
}

#[tokio::test]
async fn default_agent_route_accepts_129_and_200_byte_keys_and_rejects_201_without_writes() {
    const WORKSPACE: &str = "default-agent-key-bounds-workspace";
    const BINDING: &str = "default-agent-key-bounds-binding";
    let directory = tempfile::tempdir().expect("temporary default AgentBinding route directory");
    let store = open_store(&directory);
    create_workspace(&store, WORKSPACE);
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open AgentBinding fixture database");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at) VALUES ('default-agent-key-bounds-profile', 'fixture', 'Fixture Agent', ?1)",
            [NOW],
        )
        .expect("seed AgentProfile");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version) VALUES (?1, ?2, 'default-agent-key-bounds-profile', '{}', '{}', 1, 1, ?3, 1)",
            rusqlite::params![BINDING, WORKSPACE, NOW],
        )
        .expect("seed eligible AgentBinding");
    drop(connection);

    let router = Router::new()
        .route(
            "/v1/workspaces/{workspace_id}/default-agent-binding",
            axum::routing::patch(super::super::set_workspace_default_agent_binding),
        )
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspacePolicyApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
        });

    for (length, version, expected_events, binding) in [
        (129, 1_u64, 1_i64, json!(BINDING)),
        (200, 2_u64, 2_i64, Value::Null),
    ] {
        let key = "d".repeat(length);
        let response =
            default_agent_request(router.clone(), WORKSPACE, version, &key, binding.clone()).await;
        assert_eq!(response.status(), StatusCode::OK, "{length}-byte key");
        assert_eq!(request_receipt_count(&directory, &key), 1);
        assert_eq!(
            event_count(
                &directory,
                WORKSPACE,
                "workspace.default_agent_binding.changed.v1"
            ),
            expected_events
        );

        let replay = default_agent_request(router.clone(), WORKSPACE, version, &key, binding).await;
        assert_eq!(replay.status(), StatusCode::OK, "{length}-byte replay");
        assert_eq!(request_receipt_count(&directory, &key), 1);
        assert_eq!(
            event_count(
                &directory,
                WORKSPACE,
                "workspace.default_agent_binding.changed.v1"
            ),
            expected_events,
            "exact replay does not duplicate the AgentBinding event"
        );
    }

    let too_long_key = "d".repeat(201);
    let rejected = default_agent_request(router, WORKSPACE, 3, &too_long_key, json!(BINDING)).await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(rejected.into_body(), 4096)
        .await
        .expect("bounded invalid-key response");
    assert_eq!(
        serde_json::from_slice::<Value>(&body).expect("error JSON")["code"],
        "INVALID_ARGUMENT"
    );
    let workspace = store
        .get_workspace(WORKSPACE)
        .expect("read default AgentBinding Workspace")
        .expect("Workspace remains stored");
    assert_eq!(workspace.default_agent_binding_id, None);
    assert_eq!(workspace.version, 3);
    assert_eq!(request_receipt_count(&directory, &too_long_key), 0);
    assert_eq!(
        event_count(
            &directory,
            WORKSPACE,
            "workspace.default_agent_binding.changed.v1"
        ),
        2
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn register_native_root_runtime(store: &SqliteWorkspaceStore) {
    let public_key = [0x35; 32];
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
                current_incarnation_id: "workspace-key-bounds-incarnation".to_owned(),
                resource_capacity: json!({"sampled_at": NOW}),
                last_seen: NOW.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: "workspace-key-bounds-incarnation".to_owned(),
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
                runtime_incarnation_id: "workspace-key-bounds-incarnation".to_owned(),
                os_boot_id: Some("workspace-key-bounds-boot".to_owned()),
                observed_at: NOW.to_owned(),
            },
        )
        .expect("local Runtime registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: "workspace-key-bounds-incarnation".to_owned(),
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
                request_id: "workspace-key-bounds-enrollment".to_owned(),
                request_payload: json!({"workspace_id": "native-root-key-bounds-workspace"}),
            },
            expected_workspace_version: 1,
            runtime_workspace_binding_id: "workspace-key-bounds-runtime-binding".to_owned(),
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: "workspace-key-bounds-incarnation".to_owned(),
            workspace_id: "native-root-key-bounds-workspace".to_owned(),
            now: NOW.to_owned(),
            correlation_id: "workspace-key-bounds-enrollment".to_owned(),
        })
        .expect("owner enrolls local Runtime");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn native_root_state(store: SqliteWorkspaceStore) -> super::super::ApiState {
    let (_, stream_shutdown) = tokio::sync::watch::channel(false);
    super::super::ApiState {
        store,
        principal_id: OWNER.to_owned(),
        runtime_id: RUNTIME.to_owned(),
        local_incarnation_id: "workspace-key-bounds-incarnation".to_owned(),
        lifecycle_blockers: Vec::new(),
        runtime_identity: Arc::new(
            storage_sqlite::RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                .expect("test Runtime identity"),
        ),
        stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
        runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        stream_shutdown,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn native_root_request(router: Router, key: &str, path: &Path) -> Response {
    use std::os::unix::ffi::OsStrExt;

    let body = json!({
        "workspace_id": "native-root-key-bounds-workspace",
        "expected_workspace_version": 1,
        "selected_path_base64": base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(path.as_os_str().as_bytes()),
        "watch_policy": "METADATA",
        "replication_policy": "NONE",
    });
    let mut request = Request::post(operator_ipc::PRIVATE_LOCAL_ROOT_SELECTION_PATH)
        .header("idempotency-key", key)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("native root-selection request is valid HTTP");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .oneshot(request)
        .await
        .expect("native root-selection route responds")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn native_root_selection_route_accepts_129_and_200_byte_keys_and_rejects_201_without_writes()
{
    const WORKSPACE: &str = "native-root-key-bounds-workspace";
    let directory = tempfile::tempdir().expect("temporary native root-selection directory");
    let store = open_store(&directory);
    create_workspace(&store, WORKSPACE);
    register_native_root_runtime(&store);
    let router = super::super::build_operator_router(native_root_state(store.clone()));

    for (length, folder_name) in [(129, "selected-129"), (200, "selected-200")] {
        let selected_path = directory.path().join(folder_name);
        fs::create_dir(&selected_path).expect("selected folder fixture creates");
        let key = "r".repeat(length);
        let response = native_root_request(router.clone(), &key, &selected_path).await;
        assert_eq!(response.status(), StatusCode::OK, "{length}-byte key");
        assert_eq!(request_receipt_count(&directory, &key), 1);
        let body = to_bytes(response.into_body(), 4096)
            .await
            .expect("bounded root-selection response");
        assert_eq!(
            serde_json::from_slice::<Value>(&body).expect("root-selection JSON")["status"],
            "ACTIVE"
        );
    }

    let roots_before_rejection: i64 =
        rusqlite::Connection::open(directory.path().join("state.sqlite3"))
            .expect("open root database")
            .query_row(
                "SELECT COUNT(*) FROM workspace_roots WHERE workspace_id = ?1",
                [WORKSPACE],
                |row| row.get(0),
            )
            .expect("count committed roots");
    let events_before_rejection =
        rusqlite::Connection::open(directory.path().join("state.sqlite3"))
            .expect("open event database")
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1",
                [WORKSPACE],
                |row| row.get::<_, i64>(0),
            )
            .expect("count Workspace events");

    let rejected_folder = directory.path().join("selected-201");
    fs::create_dir(&rejected_folder).expect("rejected request folder fixture creates");
    let too_long_key = "r".repeat(201);
    let rejected = native_root_request(router, &too_long_key, &rejected_folder).await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(rejected.into_body(), 4096)
        .await
        .expect("bounded invalid-key response");
    assert_eq!(
        serde_json::from_slice::<Value>(&body).expect("error JSON")["code"],
        "INVALID_ARGUMENT"
    );
    assert_eq!(request_receipt_count(&directory, &too_long_key), 0);
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("reopen root database");
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM workspace_roots WHERE workspace_id = ?1",
                [WORKSPACE],
                |row| row.get::<_, i64>(0),
            )
            .expect("count roots after rejection"),
        roots_before_rejection
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1",
                [WORKSPACE],
                |row| row.get::<_, i64>(0),
            )
            .expect("count events after rejection"),
        events_before_rejection
    );
}
