use super::*;
use base64::Engine as _;
use storage_core::{NeedsYouCursor, NeedsYouFilter, NeedsYouStatus};

const OWNER: &str = "needs-you-route-owner";
const WORKSPACE: &str = "needs-you-route-workspace";
const NOW: &str = "2026-10-14T12:00:00.000000000Z";

fn router_fixture() -> (tempfile::TempDir, SqliteWorkspaceStore, Router) {
    let directory = tempfile::tempdir().expect("temporary Needs You route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
    let store = SqliteWorkspaceStore::open(
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
    .expect("real SQLite store opens");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Needs You route fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "needs-you-workspace-created".to_owned(),
                origin_runtime_id: "needs-you-runtime".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "needs-you-workspace-create".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits");
    let router = super::super::needs_you_operator::test_router(store.clone(), OWNER.to_owned());
    (directory, store, router)
}

#[tokio::test]
async fn needs_you_route_defaults_open_and_requires_workspace_context() {
    let (_directory, _store, router) = router_fixture();
    let mut request = Request::get("/v1/needs-you?limit=10")
        .header("x-workspace-id", WORKSPACE)
        .body(Body::empty())
        .expect("request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096)
        .await
        .expect("bounded page body");
    let body: Value = serde_json::from_slice(&body).expect("valid page JSON");
    assert_eq!(body["items"], json!([]));
    assert_eq!(body["next_cursor"], Value::Null);

    let request = Request::get("/v1/needs-you")
        .body(Body::empty())
        .expect("request is valid");
    let mut request = request;
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let response = router
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn needs_you_route_rejects_foreign_workspace_even_when_projection_is_empty() {
    let (_directory, store, _router) = router_fixture();
    let request = Request::get("/v1/needs-you")
        .header("x-workspace-id", WORKSPACE)
        .body(Body::empty())
        .expect("request is valid");
    let mut request = request;
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    // Build a route with a different authenticated owner and use the real Workspace.
    let router = super::super::needs_you_operator::test_router(store, "another-owner".to_owned());
    let response = router
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn needs_you_route_rejects_cursor_reuse_with_a_different_limit() {
    let (_directory, store, _router) = router_fixture();
    let router = super::super::needs_you_operator::test_router(store, OWNER.to_owned());
    let cursor = NeedsYouCursor {
        workspace_id: WORKSPACE.to_owned(),
        filter: NeedsYouFilter {
            status: Some(NeedsYouStatus::Open),
            kind: None,
            task_id: None,
        },
        limit: 10,
        after_created_at: NOW.to_owned(),
        after_item_id: format!("sha256:{}", "a".repeat(64)),
    };
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&cursor).expect("cursor serializes"));
    let mut request = Request::get(format!("/v1/needs-you?limit=11&cursor={encoded}"))
        .header("x-workspace-id", WORKSPACE)
        .body(Body::empty())
        .expect("request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let response = router
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn needs_you_route_rejects_malformed_cursor_positions_as_bad_requests() {
    let (_directory, store, _router) = router_fixture();
    let router = super::super::needs_you_operator::test_router(store, OWNER.to_owned());
    for (after_created_at, after_item_id) in [
        ("not-a-timestamp", format!("sha256:{}", "a".repeat(64))),
        ("2026-10-14T12:00:00Z", format!("sha256:{}", "a".repeat(64))),
        (NOW, "not-a-digest".to_owned()),
    ] {
        let cursor = NeedsYouCursor {
            workspace_id: WORKSPACE.to_owned(),
            filter: NeedsYouFilter {
                status: Some(NeedsYouStatus::Open),
                kind: None,
                task_id: None,
            },
            limit: 10,
            after_created_at: after_created_at.to_owned(),
            after_item_id,
        };
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&cursor).expect("cursor serializes"));
        let mut request = Request::get(format!("/v1/needs-you?limit=10&cursor={encoded}"))
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("request is valid");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("router returns a response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn needs_you_route_serves_through_authenticated_unix_ipc() {
    let (directory, _store, router) = router_fixture();
    let expected_uid = fs::metadata(directory.path())
        .expect("private route directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint derives");
    let listener = endpoint.bind().expect("local endpoint binds");
    let server_budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
        .expect("bounded server response budget");
    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID desktop peer is authenticated");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await
        .expect("Needs You response is framed and sent");
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
        .expect("bounded client response budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "needs-you-ipc-read".to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/needs-you?limit=10".to_owned(),
            headers: vec![LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: WORKSPACE.to_owned(),
            }],
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded Needs You request frame");
    client
        .write_request(&request)
        .await
        .expect("request frame writes");
    let response = client
        .read_response(&client_budget)
        .await
        .expect("correlated Needs You response reads");
    assert_eq!(response.header.status, StatusCode::OK.as_u16());
    let body: Value = serde_json::from_slice(response.body()).expect("page JSON is valid");
    assert_eq!(body, json!({"items": [], "next_cursor": null}));
    server.await.expect("single-request IPC server completes");
}
