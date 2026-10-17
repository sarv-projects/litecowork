#![cfg(any(target_os = "linux", target_os = "macos"))]

use super::*;

use domain_workspace::{ChangeReplicationPolicy, EventContext, WorkspaceService};
use operator_ipc::{RequestFrame, RequestHeader};
use sha2::Digest;
use std::{
    fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, PermissionsExt},
    },
    path::Path,
    sync::Arc,
    time::Duration,
};
use storage_core::{
    BlobPurpose, DeviceIdentityRecord, LocalRuntimeWorkspaceEnrollmentRequest, ReplicationPolicy,
    RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
    RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
    RuntimeWorkspaceBindingStore, StoreError, WorkspaceCreateRequest,
};
use storage_sqlite::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
use zeroize::Zeroizing;

const WORKSPACE: &str = "root-selection-ipc-workspace";
const OWNER: &str = "root-selection-ipc-owner";
const RUNTIME: &str = "root-selection-ipc-runtime";
const NOW: &str = "2026-10-15T12:00:00Z";

fn router_for(store: SqliteWorkspaceStore, principal_id: &str) -> axum::Router {
    let (_, stream_shutdown) = tokio::sync::watch::channel(false);
    super::build_operator_router(super::ApiState {
        store,
        principal_id: principal_id.to_owned(),
        runtime_id: RUNTIME.to_owned(),
        local_incarnation_id: "root-selection-ipc-incarnation".to_owned(),
        lifecycle_blockers: vec![],
        runtime_identity: Arc::new(
            storage_sqlite::RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                .expect("test Runtime identity"),
        ),
        stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
        runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        stream_shutdown,
    })
}

#[derive(Clone)]
struct RootSelectionTestKeys;

impl WorkspaceBlobKeyProvider for RootSelectionTestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([0x35; 32]),
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
                "unknown root-selection test key version".to_owned(),
            ));
        }
        self.current_key(workspace_id, purpose)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn send_over_authenticated_unix_ipc(
    router: axum::Router,
    endpoint_directory: &Path,
    request_id: &str,
    method: &str,
    path: &str,
    headers: Vec<LogicalHeader>,
    body: Vec<u8>,
) -> (u16, Vec<u8>) {
    let expected_uid = fs::metadata(endpoint_directory)
        .expect("private IPC directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(endpoint_directory).expect("safe IPC endpoint derives");
    let listener = endpoint.bind().expect("local IPC endpoint binds");
    let server_budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
        .expect("bounded server body budget");
    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID desktop peer authenticates");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await
        .expect("local IPC exchange completes");
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer authenticates");
    let client_budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
        .expect("bounded client body budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            method: method.to_owned(),
            path_and_query: path.to_owned(),
            headers,
            body_length: body.len() as u64,
        },
        body,
        &client_budget,
    )
    .expect("bounded request frame is valid");
    client
        .write_request(&request)
        .await
        .expect("request frame writes");
    let response = client
        .read_response(&client_budget)
        .await
        .expect("correlated response reads");
    server.await.expect("local IPC server completes");
    (response.header.status, response.body().to_vec())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn private_subdirectory(parent: &Path, name: &str) -> std::path::PathBuf {
    let path = parent.join(name);
    fs::create_dir(&path).expect("private IPC subdirectory creates");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
        .expect("private IPC subdirectory permissions are set");
    path
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn root_selection_body(selected_path: &Path) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "expected_workspace_version": 1,
        "selected_path_base64": base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(selected_path.as_os_str().as_bytes()),
        "watch_policy": "METADATA",
        "replication_policy": "NONE",
    }))
    .expect("native selection request serializes")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn idempotency_header(key: &str) -> Vec<LogicalHeader> {
    vec![LogicalHeader {
        name: "idempotency-key".to_owned(),
        value: key.to_owned(),
    }]
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn native_workspace_root_selection_is_private_owner_scoped_and_idempotent() {
    let directory = tempfile::tempdir().expect("temporary root-selection data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private test data directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            RootSelectionTestKeys,
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
            name: "Root selection IPC fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "root-selection-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "root-selection-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner Workspace commits");
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
                current_incarnation_id: "root-selection-ipc-incarnation".to_owned(),
                resource_capacity: json!({"sampled_at": NOW}),
                last_seen: NOW.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: "root-selection-ipc-incarnation".to_owned(),
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
                runtime_incarnation_id: "root-selection-ipc-incarnation".to_owned(),
                os_boot_id: Some("root-selection-ipc-boot".to_owned()),
                observed_at: NOW.to_owned(),
            },
        )
        .expect("local Runtime registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: "root-selection-ipc-incarnation".to_owned(),
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
                request_id: "root-selection-ipc-enrollment".to_owned(),
                request_payload: json!({"workspace_id": WORKSPACE}),
            },
            expected_workspace_version: 1,
            runtime_workspace_binding_id: "root-selection-ipc-binding".to_owned(),
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: "root-selection-ipc-incarnation".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            now: NOW.to_owned(),
            correlation_id: "root-selection-ipc-enrollment".to_owned(),
        })
        .expect("owner enrolls local Runtime for the Workspace");

    let selected_directory = directory.path().join("selected-project");
    fs::create_dir(&selected_directory).expect("selected folder fixture creates");
    fs::write(
        selected_directory.join("private-name.txt"),
        b"content is never opened",
    )
    .expect("metadata-only scan fixture creates");
    let selection_body = root_selection_body(&selected_directory);
    let owner_router = router_for(store.clone(), OWNER);
    let owner_endpoint = private_subdirectory(directory.path(), "owner-ipc");
    let (status, receipt) = send_over_authenticated_unix_ipc(
        owner_router.clone(),
        &owner_endpoint,
        "root-selection-create",
        "POST",
        operator_ipc::PRIVATE_LOCAL_ROOT_SELECTION_PATH,
        idempotency_header("root-selection-create-key"),
        selection_body.clone(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK.as_u16(),
        "root selection failed: {}",
        String::from_utf8_lossy(&receipt)
    );
    let receipt_json: Value = serde_json::from_slice(&receipt).expect("safe receipt is JSON");
    assert_eq!(receipt_json["workspace_id"], WORKSPACE);
    assert_eq!(receipt_json["display_name"], "selected-project");
    assert_eq!(receipt_json["status"], "ACTIVE");
    assert_eq!(receipt_json["location_availability"], "AVAILABLE");

    let root_id = receipt_json["workspace_root_id"]
        .as_str()
        .expect("root selection returns a root ID")
        .to_owned();
    let scan_path = format!("/v1/workspace-roots/{root_id}/scan");
    let scan_request = |request_id: &str| {
        Request::post(scan_path.clone())
            .header("x-workspace-id", WORKSPACE)
            .header(header::IF_MATCH, "1")
            .header("idempotency-key", request_id)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .expect("metadata scan request is valid")
    };
    let unauthorized_scan = scan_request("root-selection-scan-key");
    let unauthorized_scan_response = router_for(store.clone(), OWNER)
        .oneshot(unauthorized_scan)
        .await
        .expect("unauthenticated scan returns a response");
    assert_eq!(
        unauthorized_scan_response.status(),
        StatusCode::UNAUTHORIZED,
        "the mounted scan action remains authenticated local Operator traffic"
    );

    let mut scan = scan_request("root-selection-scan-key");
    scan.extensions_mut().insert(AuthenticatedLocalPeer);
    let scan_response = router_for(store.clone(), OWNER)
        .oneshot(scan)
        .await
        .expect("authenticated metadata scan returns a response");
    assert_eq!(scan_response.status(), StatusCode::OK);
    assert_eq!(scan_response.headers()[header::CACHE_CONTROL], "no-store");
    let scan_body = to_bytes(scan_response.into_body(), 4096)
        .await
        .expect("bounded metadata scan response");
    let scan_json: Value = serde_json::from_slice(&scan_body).expect("scan summary is JSON");
    assert_eq!(scan_json["workspace_id"], WORKSPACE);
    assert_eq!(scan_json["workspace_root_id"], root_id);
    assert_eq!(scan_json["root_version"], 1);
    assert_eq!(scan_json["status"], "COMPLETE");
    assert_eq!(scan_json["entries_examined"], 1);
    assert_eq!(scan_json["files_observed"], 1);
    assert!(scan_json["partial_reason"].is_null());
    let public_scan = scan_json.to_string();
    assert!(!public_scan.contains("private-name.txt"));
    assert!(!public_scan.contains("content is never opened"));
    assert!(!public_scan.contains(&selected_directory.to_string_lossy().to_string()));

    // A second complete scan gives the production observation route two rows to
    // paginate, while keeping the following budget-exhaustion scan partial.
    fs::write(selected_directory.join("second-private.txt"), b"never read")
        .expect("second metadata-only observation fixture creates");
    let mut second_scan = scan_request("root-selection-second-complete-scan-key");
    second_scan.extensions_mut().insert(AuthenticatedLocalPeer);
    let second_scan_response = router_for(store.clone(), OWNER)
        .oneshot(second_scan)
        .await
        .expect("second complete metadata scan returns a response");
    assert_eq!(second_scan_response.status(), StatusCode::OK);
    let second_scan_body = to_bytes(second_scan_response.into_body(), 4096)
        .await
        .expect("bounded second scan summary");
    let second_scan_json: Value =
        serde_json::from_slice(&second_scan_body).expect("second scan summary is JSON");
    assert_eq!(second_scan_json["status"], "COMPLETE");
    assert_eq!(second_scan_json["files_observed"], 2);

    let observations_path = format!("/v1/workspace-roots/{root_id}/observations?limit=1");
    let observation_request = |path: &str, if_match: Option<&str>| {
        let mut request = Request::get(path)
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("WorkspaceRoot observations request is valid");
        if let Some(version) = if_match {
            request.headers_mut().insert(
                header::IF_MATCH,
                version.parse().expect("If-Match header is valid"),
            );
        }
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        request
    };
    let first_page_response = router_for(store.clone(), OWNER)
        .oneshot(observation_request(&observations_path, Some("1")))
        .await
        .expect("owner observation page returns a response");
    assert_eq!(first_page_response.status(), StatusCode::OK);
    assert_eq!(
        first_page_response.headers()[header::CACHE_CONTROL],
        "no-store"
    );
    let first_page_body = to_bytes(first_page_response.into_body(), 8192)
        .await
        .expect("bounded observation page");
    let first_page: Value =
        serde_json::from_slice(&first_page_body).expect("observation page is JSON");
    assert_eq!(first_page["workspace_id"], WORKSPACE);
    assert_eq!(first_page["workspace_root_id"], root_id);
    assert_eq!(first_page["items"].as_array().unwrap().len(), 1);
    assert_eq!(first_page["items"][0]["display_name"], "private-name.txt");
    assert_eq!(first_page["items"][0]["resource_id"], Value::Null);
    let cursor = first_page["next_cursor"]
        .as_str()
        .expect("first page has a continuation cursor")
        .to_owned();
    let first_page_text = first_page.to_string();
    let selected_directory_text = selected_directory.to_string_lossy().to_string();
    for private_value in [
        "file_identity_digest",
        "private_relative_locator",
        "content is never opened",
        selected_directory_text.as_str(),
    ] {
        assert!(
            !first_page_text.contains(private_value),
            "observation response leaked {private_value}"
        );
    }

    let next_page_path =
        format!("/v1/workspace-roots/{root_id}/observations?limit=1&cursor={cursor}");
    let next_page_response = router_for(store.clone(), OWNER)
        .oneshot(observation_request(&next_page_path, Some("1")))
        .await
        .expect("signed cursor roundtrip returns a response");
    assert_eq!(next_page_response.status(), StatusCode::OK);
    let next_page_body = to_bytes(next_page_response.into_body(), 8192)
        .await
        .expect("bounded continuation page");
    let next_page: Value =
        serde_json::from_slice(&next_page_body).expect("continuation page is JSON");
    assert_eq!(next_page["items"].as_array().unwrap().len(), 1);
    assert_eq!(next_page["items"][0]["display_name"], "second-private.txt");
    assert!(next_page["next_cursor"].is_null());

    let scoped_cursor_path =
        format!("/v1/workspace-roots/{root_id}/observations?limit=1&q=other&cursor={cursor}");
    let mismatched_cursor_response = router_for(store.clone(), OWNER)
        .oneshot(observation_request(&scoped_cursor_path, Some("1")))
        .await
        .expect("query-scoped cursor request returns a response");
    assert_eq!(mismatched_cursor_response.status(), StatusCode::BAD_REQUEST);

    let mut tampered = cursor.clone();
    let final_index = tampered.len() - 1;
    let replacement = if &tampered[final_index..] == "A" {
        "B"
    } else {
        "A"
    };
    tampered.replace_range(final_index.., replacement);
    let tampered_path =
        format!("/v1/workspace-roots/{root_id}/observations?limit=1&cursor={tampered}");
    let tampered_response = router_for(store.clone(), OWNER)
        .oneshot(observation_request(&tampered_path, Some("1")))
        .await
        .expect("tampered cursor request returns a response");
    assert_eq!(tampered_response.status(), StatusCode::BAD_REQUEST);

    let missing_if_match = router_for(store.clone(), OWNER)
        .oneshot(observation_request(&observations_path, None))
        .await
        .expect("missing If-Match request returns a response");
    assert_eq!(missing_if_match.status(), StatusCode::BAD_REQUEST);
    let stale_if_match = router_for(store.clone(), OWNER)
        .oneshot(observation_request(&observations_path, Some("2")))
        .await
        .expect("stale If-Match request returns a response");
    assert_eq!(stale_if_match.status(), StatusCode::CONFLICT);
    let foreign_observation_response = router_for(store.clone(), "root-selection-foreign-owner")
        .oneshot(observation_request(&observations_path, Some("1")))
        .await
        .expect("foreign owner observation request returns a response");
    assert_eq!(foreign_observation_response.status(), StatusCode::NOT_FOUND);

    for index in 0..1_000 {
        fs::write(
            selected_directory.join(format!("scan-private-name-{index:04}.txt")),
            b"",
        )
        .expect("bounded partial-scan fixture file creates");
    }
    let mut partial_scan = scan_request("root-selection-partial-scan-key");
    partial_scan.extensions_mut().insert(AuthenticatedLocalPeer);
    let partial_response = router_for(store.clone(), OWNER)
        .oneshot(partial_scan)
        .await
        .expect("bounded partial metadata scan returns a response");
    assert_eq!(partial_response.status(), StatusCode::OK);
    let partial_body = to_bytes(partial_response.into_body(), 4096)
        .await
        .expect("bounded partial scan summary");
    let partial_json: Value = serde_json::from_slice(&partial_body).expect("partial scan is JSON");
    assert_eq!(partial_json["status"], "PARTIAL");
    assert_eq!(partial_json["entries_examined"], 1_000);
    assert_eq!(partial_json["files_observed"], 1_000);
    assert_eq!(partial_json["partial_reason"], "SCAN_BUDGET_REACHED");
    assert!(!partial_json.to_string().contains("scan-private-name-"));

    fs::rename(
        &selected_directory,
        directory.path().join("selected-project-moved"),
    )
    .expect("move selected folder before replay");
    let scan_replay_endpoint = private_subdirectory(directory.path(), "scan-replay-ipc");
    let (replay_status, replay_body) = send_over_authenticated_unix_ipc(
        router_for(store.clone(), OWNER),
        &scan_replay_endpoint,
        "root-selection-scan-replay",
        "POST",
        &scan_path,
        vec![
            LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: WORKSPACE.to_owned(),
            },
            LogicalHeader {
                name: "if-match".to_owned(),
                value: "1".to_owned(),
            },
            LogicalHeader {
                name: "idempotency-key".to_owned(),
                value: "root-selection-scan-key".to_owned(),
            },
            LogicalHeader {
                name: "content-type".to_owned(),
                value: "application/json".to_owned(),
            },
        ],
        b"{}".to_vec(),
    )
    .await;
    assert_eq!(replay_status, StatusCode::OK.as_u16());
    assert_eq!(replay_body, scan_body.as_ref());
    fs::rename(
        directory.path().join("selected-project-moved"),
        &selected_directory,
    )
    .expect("restore selected folder for the independent root-selection replay case");

    let mut changed_scan = Request::post("/v1/workspace-roots/other-root/scan")
        .header("x-workspace-id", WORKSPACE)
        .header(header::IF_MATCH, "1")
        .header("idempotency-key", "root-selection-scan-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .expect("changed metadata scan request is valid");
    changed_scan.extensions_mut().insert(AuthenticatedLocalPeer);
    let changed_response = router_for(store.clone(), OWNER)
        .oneshot(changed_scan)
        .await
        .expect("changed key reuse returns a response");
    assert_eq!(changed_response.status(), StatusCode::CONFLICT);

    let mut list_request = Request::get("/v1/workspace-roots?limit=100")
        .header("x-workspace-id", WORKSPACE)
        .body(Body::empty())
        .expect("WorkspaceRoot list request is valid");
    list_request.extensions_mut().insert(AuthenticatedLocalPeer);
    let list_response = router_for(store.clone(), OWNER)
        .oneshot(list_request)
        .await
        .expect("owner list route returns a response");
    assert_eq!(list_response.status(), StatusCode::OK);
    assert_eq!(list_response.headers()[header::CACHE_CONTROL], "no-store");
    let list_body = to_bytes(list_response.into_body(), 4096)
        .await
        .expect("bounded WorkspaceRoot list body");
    let list_json: Value = serde_json::from_slice(&list_body).expect("list response is JSON");
    assert_eq!(list_json["items"].as_array().unwrap().len(), 1);
    assert_eq!(list_json["items"][0]["display_name"], "selected-project");
    assert_eq!(list_json["items"][0]["status"], "ACTIVE");
    assert_eq!(list_json["items"][0]["location_availability"], "AVAILABLE");
    assert!(list_json["nextCursor"].is_null());
    let public_list_json = list_json.to_string();
    assert!(!public_list_json.contains(&selected_directory.to_string_lossy().to_string()));
    for private_field in [
        "private_binding",
        "private_locator",
        "locator_ref_id",
        "file_identity",
        "raw_filesystem_instance_id",
        "raw_file_id",
    ] {
        assert!(
            !public_list_json.contains(private_field),
            "WorkspaceRoot list leaked private field {private_field}"
        );
    }

    let mut foreign_list_request = Request::get("/v1/workspace-roots")
        .header("x-workspace-id", WORKSPACE)
        .body(Body::empty())
        .expect("foreign WorkspaceRoot list request is valid");
    foreign_list_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let foreign_list_response = router_for(store.clone(), "root-selection-foreign-owner")
        .oneshot(foreign_list_request)
        .await
        .expect("foreign owner list route returns a response");
    assert_eq!(foreign_list_response.status(), StatusCode::NOT_FOUND);
    let foreign_list_body = to_bytes(foreign_list_response.into_body(), 4096)
        .await
        .expect("bounded foreign WorkspaceRoot error body");
    let foreign_list_error: Value =
        serde_json::from_slice(&foreign_list_body).expect("foreign error is JSON");
    assert_eq!(foreign_list_error["code"], "NOT_FOUND");

    WorkspaceService::new(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: WORKSPACE.to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: vec![],
            event: EventContext {
                event_id: "root-selection-workspace-version-advanced".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: "2026-10-15T12:00:01Z".to_owned(),
                correlation_id: "root-selection-workspace-version-advanced".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-15T12:00:01Z".to_owned(),
            },
        })
        .expect("unrelated Workspace update advances its version");
    assert_eq!(
        store
            .get_workspace(WORKSPACE)
            .expect("updated Workspace reads")
            .expect("Workspace remains present")
            .version,
        2
    );

    let replay_endpoint = private_subdirectory(directory.path(), "replay-ipc");
    let (replay_status, replay_receipt) = send_over_authenticated_unix_ipc(
        owner_router,
        &replay_endpoint,
        "root-selection-replay",
        "POST",
        operator_ipc::PRIVATE_LOCAL_ROOT_SELECTION_PATH,
        idempotency_header("root-selection-create-key"),
        selection_body.clone(),
    )
    .await;
    assert_eq!(
        replay_status,
        StatusCode::OK.as_u16(),
        "exact root-selection replay failed: {}",
        String::from_utf8_lossy(&replay_receipt)
    );
    assert_eq!(
        replay_receipt, receipt,
        "exact retry returns its original receipt"
    );

    let stale_endpoint = private_subdirectory(directory.path(), "fresh-stale-ipc");
    let (stale_status, stale_body) = send_over_authenticated_unix_ipc(
        router_for(store.clone(), OWNER),
        &stale_endpoint,
        "root-selection-fresh-stale",
        "POST",
        operator_ipc::PRIVATE_LOCAL_ROOT_SELECTION_PATH,
        idempotency_header("root-selection-fresh-stale-key"),
        selection_body.clone(),
    )
    .await;
    assert_eq!(stale_status, StatusCode::CONFLICT.as_u16());
    let stale_error: Value = serde_json::from_slice(&stale_body).expect("stale error is JSON");
    assert_eq!(stale_error["code"], "STALE_WORKSPACE_VERSION");

    let foreign_endpoint = private_subdirectory(directory.path(), "foreign-ipc");
    let (foreign_status, foreign_body) = send_over_authenticated_unix_ipc(
        router_for(store.clone(), "root-selection-foreign-owner"),
        &foreign_endpoint,
        "root-selection-foreign",
        "POST",
        operator_ipc::PRIVATE_LOCAL_ROOT_SELECTION_PATH,
        idempotency_header("root-selection-foreign-key"),
        selection_body,
    )
    .await;
    assert_eq!(foreign_status, StatusCode::NOT_FOUND.as_u16());
    let foreign_error: Value = serde_json::from_slice(&foreign_body).expect("safe error is JSON");
    assert_eq!(foreign_error["code"], "NOT_FOUND");

    let public_post_endpoint = private_subdirectory(directory.path(), "public-post-ipc");
    let (public_post_status, _) = send_over_authenticated_unix_ipc(
        router_for(store.clone(), OWNER),
        &public_post_endpoint,
        "root-selection-public-post",
        "POST",
        "/v1/workspace-roots",
        idempotency_header("root-selection-public-key"),
        b"{}".to_vec(),
    )
    .await;
    assert_eq!(
        public_post_status,
        StatusCode::METHOD_NOT_ALLOWED.as_u16(),
        "the public logical root collection does not serve path-based creation"
    );

    let connection = rusqlite::Connection::open(&database).expect("fixture database reopens");
    let root_count = connection
        .query_row("SELECT COUNT(*) FROM workspace_roots", [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("committed root count reads");
    assert_eq!(root_count, 1, "replay and foreign requests add no root");
}
