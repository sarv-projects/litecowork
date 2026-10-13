use super::{
    AuthenticatedLocalPeer, OperatorIpcConnection, RuntimeWorkspaceBindingsApiState,
    dispatch_ipc_exchange, runtime_workspace_bindings_router,
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use operator_ipc::{
    DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, InFlightBodyBudget, LogicalHeader, PROTOCOL_VERSION,
    RequestFrame, RequestHeader, unix::UnixEndpoint,
};
use serde_json::{Value, json};
use sha2::Digest;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{fs, sync::Arc, time::Duration};
use storage_core::{
    BlobPurpose, DeviceIdentityRecord, LocalRuntimeWorkspaceEnrollmentRequest,
    RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
    RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
    RuntimeWorkspaceBindingStore, StoreError, WorkspaceCreateRequest,
};
use storage_sqlite::{
    FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider,
};
use tower::ServiceExt;
use zeroize::Zeroizing;

#[derive(Clone)]
struct RuntimeBindingTestKeys;

impl WorkspaceBlobKeyProvider for RuntimeBindingTestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([71_u8; 32]),
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

#[tokio::test]
async fn workspace_runtime_binding_route_pages_owner_scoped_retained_records() {
    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private data directory permissions");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            RuntimeBindingTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    let runtime_id = "runtime-bindings-route";
    let incarnation_id = "incarnation-bindings-route";
    let workspace_id = "workspace-bindings-route";
    let owner_id = "owner-bindings-route";
    let now = "2026-10-11T10:00:00Z";
    let public_key = [19_u8; 32];
    store
        .register_local_incarnation(
            RuntimeRecord {
                runtime_id: runtime_id.to_owned(),
                device_identity: DeviceIdentityRecord {
                    device_id: format!(
                        "ed25519-sha256:{}",
                        hex::encode(sha2::Sha256::digest(public_key))
                    ),
                    public_key: format!("ed25519:{}", hex::encode(public_key)),
                    key_version: 1,
                    issued_at: now.to_owned(),
                    display_name: None,
                },
                runtime_version: "0.1.0-test".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                roles: vec!["OPERATOR_ENDPOINT".to_owned()],
                trust_zone: "PERSONAL_DEVICE".to_owned(),
                availability: "RECOVERING".to_owned(),
                startup_policy: "MANUAL".to_owned(),
                current_incarnation_id: incarnation_id.to_owned(),
                resource_capacity: json!({"sampled_at": now}),
                last_seen: now.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                runtime_id: runtime_id.to_owned(),
                process_started_at: now.to_owned(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                os_boot_id: Some("boot-bindings-route".to_owned()),
                observed_at: now.to_owned(),
            },
        )
        .expect("Runtime registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: now.to_owned(),
            stopped_at: None,
        })
        .expect("Runtime becomes available");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Runtime binding route fixture".to_owned(),
            owner_principal_id: owner_id.to_owned(),
            event: EventContext {
                event_id: "bindings-route-workspace-event".to_owned(),
                origin_runtime_id: runtime_id.to_owned(),
                hlc_timestamp: now.to_owned(),
                correlation_id: "bindings-route-create".to_owned(),
                causation_id: None,
                recorded_at: now.to_owned(),
            },
        })
        .expect("Workspace commits");
    let second_workspace_id = "workspace-bindings-route-second";
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: second_workspace_id.to_owned(),
            name: "Second Runtime binding route fixture".to_owned(),
            owner_principal_id: owner_id.to_owned(),
            event: EventContext {
                event_id: "bindings-route-second-workspace-event".to_owned(),
                origin_runtime_id: runtime_id.to_owned(),
                hlc_timestamp: now.to_owned(),
                correlation_id: "bindings-route-second-create".to_owned(),
                causation_id: None,
                recorded_at: now.to_owned(),
            },
        })
        .expect("second Workspace commits");
    store
        .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: owner_id.to_owned(),
                request_id: "bindings-route-enroll".to_owned(),
                request_payload: json!({"workspace_id": workspace_id}),
            },
            expected_workspace_version: 1,
            runtime_workspace_binding_id: "binding-m-route".to_owned(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            now: now.to_owned(),
            correlation_id: "bindings-route-enroll-correlation".to_owned(),
        })
        .expect("Workspace grants a local Runtime binding");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open binding test fixture connection");
    connection
        .execute(
            "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,revoked_at,version) VALUES('binding-z-route',?1,?2,'MESH_PAIRING','REVOKED','[\"EXECUTOR\"]',?3,?3,?3,1)",
            rusqlite::params![runtime_id, workspace_id, now],
        )
        .expect("retained revoked binding is inserted");

    let foreign_owner_store = store.clone();
    let router = runtime_workspace_bindings_router(RuntimeWorkspaceBindingsApiState {
        store,
        principal_id: owner_id.to_owned(),
        runtime_id: runtime_id.to_owned(),
        local_incarnation_id: incarnation_id.to_owned(),
    });
    let request = |path: String, workspace_header: Option<&str>| {
        let mut request = Request::get(path);
        if let Some(value) = workspace_header {
            request = request.header("X-Workspace-ID", value);
        }
        let mut request = request.body(Body::empty()).expect("GET request is valid");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        request
    };

    let first = router
        .clone()
        .oneshot(request(
            format!("/v1/workspaces/{workspace_id}/runtime-bindings?limit=1"),
            Some(workspace_id),
        ))
        .await
        .expect("first page response is returned");
    assert_eq!(first.status(), StatusCode::OK);
    let first: Value = serde_json::from_slice(
        &to_bytes(first.into_body(), 16 * 1024)
            .await
            .expect("first page body is bounded"),
    )
    .expect("first page JSON decodes");
    assert_eq!(
        first["items"][0]["runtime_workspace_binding_id"],
        "binding-m-route"
    );
    assert_eq!(first["items"][0]["status"], "ACTIVE");
    assert!(first["next_cursor"].is_string());
    assert!(
        !serde_json::to_string(&first)
            .expect("page serializes")
            .contains("ipc://")
    );

    let cursor = first["next_cursor"].as_str().expect("next cursor exists");
    let second = router
        .clone()
        .oneshot(request(
            format!("/v1/workspaces/{workspace_id}/runtime-bindings?limit=1&cursor={cursor}"),
            Some(workspace_id),
        ))
        .await
        .expect("second page response is returned");
    assert_eq!(second.status(), StatusCode::OK);
    let second: Value = serde_json::from_slice(
        &to_bytes(second.into_body(), 16 * 1024)
            .await
            .expect("second page body is bounded"),
    )
    .expect("second page JSON decodes");
    assert_eq!(
        second["items"][0]["runtime_workspace_binding_id"],
        "binding-z-route"
    );
    assert_eq!(second["items"][0]["status"], "REVOKED");
    assert_eq!(second["next_cursor"], Value::Null);

    let changed_limit = router
        .clone()
        .oneshot(request(
            format!("/v1/workspaces/{workspace_id}/runtime-bindings?limit=2&cursor={cursor}"),
            Some(workspace_id),
        ))
        .await
        .expect("changed-limit cursor response is returned");
    assert_eq!(changed_limit.status(), StatusCode::BAD_REQUEST);

    let changed_workspace = router
        .clone()
        .oneshot(request(
            format!(
                "/v1/workspaces/{second_workspace_id}/runtime-bindings?limit=1&cursor={cursor}"
            ),
            Some(second_workspace_id),
        ))
        .await
        .expect("changed-Workspace cursor response is returned");
    assert_eq!(changed_workspace.status(), StatusCode::BAD_REQUEST);

    let malformed_cursor = router
        .clone()
        .oneshot(request(
            format!("/v1/workspaces/{workspace_id}/runtime-bindings?cursor=not-a-cursor"),
            Some(workspace_id),
        ))
        .await
        .expect("malformed cursor response is returned");
    assert_eq!(malformed_cursor.status(), StatusCode::BAD_REQUEST);

    let mismatched_scope = router
        .clone()
        .oneshot(request(
            format!("/v1/workspaces/{workspace_id}/runtime-bindings"),
            Some("another-workspace"),
        ))
        .await
        .expect("mismatched Workspace response is returned");
    assert_eq!(mismatched_scope.status(), StatusCode::FORBIDDEN);

    let foreign_owner = runtime_workspace_bindings_router(RuntimeWorkspaceBindingsApiState {
        store: foreign_owner_store,
        principal_id: "another-principal".to_owned(),
        runtime_id: runtime_id.to_owned(),
        local_incarnation_id: incarnation_id.to_owned(),
    })
    .oneshot(request(
        format!("/v1/workspaces/{workspace_id}/runtime-bindings"),
        Some(workspace_id),
    ))
    .await
    .expect("foreign owner response is returned");
    assert_eq!(foreign_owner.status(), StatusCode::FORBIDDEN);

    let missing_scope = router
        .clone()
        .oneshot(request(
            format!("/v1/workspaces/{workspace_id}/runtime-bindings"),
            None,
        ))
        .await
        .expect("missing Workspace response is returned");
    assert_eq!(missing_scope.status(), StatusCode::BAD_REQUEST);

    let expected_uid = fs::metadata(directory.path())
        .expect("data directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("private IPC endpoint derives");
    let listener = endpoint.bind().expect("authenticated IPC endpoint binds");
    let server_budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
        .expect("server body budget is valid");
    let ipc_server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID IPC client is accepted");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await
        .expect("Runtime binding route response is framed");
    });
    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer identity is authenticated");
    let client_budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
        .expect("client body budget is valid");
    let ipc_request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "runtime-binding-page".to_owned(),
            method: "GET".to_owned(),
            path_and_query: format!("/v1/workspaces/{workspace_id}/runtime-bindings?limit=2"),
            headers: vec![LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: workspace_id.to_owned(),
            }],
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded Runtime binding inventory request is valid");
    client
        .write_request(&ipc_request)
        .await
        .expect("Runtime binding request is written");
    let ipc_response = client
        .read_response(&client_budget)
        .await
        .expect("Runtime binding response is read");
    assert_eq!(ipc_response.header.status, StatusCode::OK.as_u16());
    let ipc_page: Value =
        serde_json::from_slice(ipc_response.body()).expect("Runtime binding IPC response decodes");
    assert_eq!(ipc_page["items"].as_array().map(Vec::len), Some(2));
    ipc_server.await.expect("IPC route task completes");
}
