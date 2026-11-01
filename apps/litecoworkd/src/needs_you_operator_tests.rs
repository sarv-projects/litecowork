use super::*;
use base64::Engine as _;
use storage_core::{NeedsYouCursor, NeedsYouFilter, NeedsYouStatus};

const OWNER: &str = "needs-you-route-owner";
const WORKSPACE: &str = "needs-you-route-workspace";
const NOW: &str = "2026-10-14T12:00:00.000000000Z";
const TASK: &str = "needs-you-route-task";
const APPROVAL: &str = "needs-you-route-approval";
const USER_REQUEST: &str = "needs-you-route-request";
const USER_REQUEST_SESSION: &str = "needs-you-route-session";
const APPROVAL_ACTION_DIGEST: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

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

/// The same Workspace, plus one pending Approval that the Needs You projection must
/// present as one open item and drop again once the source is resolved.
fn source_router_fixture() -> (tempfile::TempDir, SqliteWorkspaceStore, Router) {
    let (directory, store, _needs_you_only) = router_fixture();
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open the fixture SQLite database");
    // The Task and Approval rows are the minimal source records the inbox projects.
    // Fixture-only foreign keys are disabled so the projection can be exercised without
    // the full Task/Attempt admission graph.
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable fixture-only foreign keys");
    connection
        .execute(
            "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version) VALUES (?1,?2,1,'WAITING_USER','needs-you-test-binding','NORMAL','{\"principal_id\":\"needs-you-route-owner\",\"kind\":\"USER\"}',?3,?3,1)",
            rusqlite::params![TASK, WORKSPACE, NOW],
        )
        .expect("seed the source Task");
    connection
        .execute(
            "INSERT INTO approvals(approval_id,task_id,kind,action_summary,target_ref_json,scope_digest,action_digest,risk,required_assurance,status,requested_at,version) VALUES (?1,?2,'CAPABILITY_ESCALATION','Send this exact message','{\"capability_ref\":{\"capability_id\":\"mail.send\",\"identity_kind\":\"PACKAGE_COMPONENT\",\"source\":\"test:litespm\",\"package_version\":\"1.0.0\",\"digest\":\"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"target_digest\":\"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\",\"execution_method\":\"STRUCTURED_API\",\"effect_id\":null}','sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',?3,'HIGH_IMPACT','LOCAL_STRONG','PENDING',?4,1)",
            rusqlite::params![APPROVAL, TASK, APPROVAL_ACTION_DIGEST, NOW],
        )
        .expect("seed the pending Approval");
    drop(connection);
    let router = super::super::approval_operator::test_router(store.clone(), OWNER.to_owned()).merge(
        super::super::needs_you_operator::test_router(store.clone(), OWNER.to_owned()),
    );
    (directory, store, router)
}

fn authenticated(mut request: Request<Body>) -> Request<Body> {
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    request
}

/// The same Workspace, plus one pending UserRequest that the Needs You projection must
/// present as one open item and drop again once the owner answers it.
fn user_request_router_fixture() -> (tempfile::TempDir, SqliteWorkspaceStore, Router) {
    let (directory, store, _needs_you_only) = router_fixture();
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open the fixture SQLite database");
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable fixture-only foreign keys");
    connection
        .execute(
            "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version) VALUES (?1,?2,1,'WAITING_USER','needs-you-test-binding','NORMAL','{\"principal_id\":\"needs-you-route-owner\",\"kind\":\"USER\"}',?3,?3,1)",
            rusqlite::params![TASK, WORKSPACE, NOW],
        )
        .expect("seed the source Task");
    connection
        .execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at) VALUES (?1,?2,'TASK_PLANNING',?3,1,'binding-fixture','endpoint-fixture','runtime-fixture','incarnation-fixture','ACTIVE',?4)",
            rusqlite::params![USER_REQUEST_SESSION, WORKSPACE, TASK, NOW],
        )
        .expect("seed the source AgentSession");
    connection
        .execute(
            "INSERT INTO user_requests(request_id,workspace_id,task_id,agent_session_id,kind,interaction_mode,prompt,choices_json,status,created_at,version) VALUES (?1,?2,?3,?4,'QUESTION','FORM','Choose an option','[]','PENDING',?5,1)",
            rusqlite::params![USER_REQUEST, WORKSPACE, TASK, USER_REQUEST_SESSION, NOW],
        )
        .expect("seed the pending UserRequest");
    drop(connection);
    let router = super::super::user_request_operator::test_router(
        store.clone(),
        OWNER.to_owned(),
    )
    .merge(super::super::needs_you_operator::test_router(
        store.clone(),
        OWNER.to_owned(),
    ));
    (directory, store, router)
}

async fn needs_you_page(router: &Router, status: &str) -> Value {
    let request = authenticated(
        Request::get(format!("/v1/needs-you?status={status}&limit=50"))
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("request is valid"),
    );
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("no-store")),
        "the private inbox projection must never be cacheable"
    );
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded page body");
    serde_json::from_slice(&body).expect("valid page JSON")
}

async fn resolve_approval(router: &Router, key: &str, version: &str, body: &str) -> Response {
    let request = authenticated(
        Request::post(format!("/v1/approvals/{APPROVAL}/resolve"))
            .header("x-workspace-id", WORKSPACE)
            .header("idempotency-key", key)
            .header("if-match", version)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .expect("request is valid"),
    );
    router
        .clone()
        .oneshot(request)
        .await
        .expect("router returns a response")
}

/// The story's primary inbox guarantee: resolving the owning source removes exactly one
/// open item, and the same item becomes visible only in the resolved history.
#[tokio::test]
async fn resolving_an_approval_source_removes_exactly_one_open_inbox_item() {
    let (_directory, _store, router) = source_router_fixture();

    let open_before = needs_you_page(&router, "OPEN").await;
    let items = open_before["items"].as_array().expect("open item array");
    assert_eq!(items.len(), 1, "the pending Approval is one open inbox item");
    assert_eq!(items[0]["kind"], "APPROVAL");
    assert_eq!(items[0]["approval_id"], APPROVAL);
    assert_eq!(items[0]["status"], "OPEN");
    let open_item_id = items[0]["item_id"].as_str().expect("stable item identity");

    let resolved = resolve_approval(
        &router,
        "needs-you-approve-once",
        "1",
        r#"{"decision":"APPROVE","confirmation_digest":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#,
    )
    .await;
    assert_eq!(resolved.status(), StatusCode::OK);
    assert_eq!(
        resolved.headers().get(header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("no-store"))
    );

    let open_after = needs_you_page(&router, "OPEN").await;
    assert_eq!(
        open_after["items"],
        json!([]),
        "resolving the source removes the item from the open inbox"
    );

    let history = needs_you_page(&router, "RESOLVED").await;
    let history_items = history["items"].as_array().expect("resolved item array");
    assert_eq!(history_items.len(), 1);
    assert_eq!(history_items[0]["item_id"], open_item_id);
    assert_eq!(history_items[0]["status"], "RESOLVED");
    assert_eq!(history_items[0]["approval_id"], APPROVAL);
}

/// The already-resolved negative case: a second decision with a new request identity is
/// rejected, and the inbox is unchanged by the rejected attempt.
#[tokio::test]
async fn an_already_resolved_source_is_rejected_and_leaves_the_inbox_unchanged() {
    let (_directory, _store, router) = source_router_fixture();
    let body =
        r#"{"decision":"APPROVE","confirmation_digest":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#;
    let first = resolve_approval(&router, "needs-you-approve-once", "1", body).await;
    assert_eq!(first.status(), StatusCode::OK);
    let history_before = needs_you_page(&router, "RESOLVED").await;

    // The stale version is rejected as a conflict, and the now-current version is
    // rejected as an unsatisfiable decision: an Approval that is no longer PENDING cannot
    // be decided again under any request identity.
    for (key, version, expected_status, expected_code) in [
        (
            "needs-you-approve-twice",
            "1",
            StatusCode::CONFLICT,
            "CONFLICT",
        ),
        (
            "needs-you-approve-thrice",
            "2",
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ARGUMENT",
        ),
    ] {
        let second = resolve_approval(&router, key, version, body).await;
        assert_eq!(
            second.status(),
            expected_status,
            "a resolved source cannot be decided again with version {version}"
        );
        assert_eq!(
            second.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-store"))
        );
        let error = to_bytes(second.into_body(), 16 * 1024)
            .await
            .expect("bounded rejection body");
        let error: Value = serde_json::from_slice(&error).expect("rejection JSON");
        assert_eq!(error["code"], expected_code);
        let history_after = needs_you_page(&router, "RESOLVED").await;
        assert_eq!(
            history_after["items"],
            history_before["items"],
            "a rejected decision adds no inbox row"
        );
        assert_eq!(needs_you_page(&router, "OPEN").await["items"], json!([]));
    }
}

/// The same guarantee for the other inbox source: answering a pending UserRequest removes
/// exactly one open item and keeps the resolved history truthful.
#[tokio::test]
async fn answering_a_user_request_source_removes_exactly_one_open_inbox_item() {
    let (_directory, _store, router) = user_request_router_fixture();

    let open_before = needs_you_page(&router, "OPEN").await;
    let items = open_before["items"].as_array().expect("open item array");
    assert_eq!(items.len(), 1, "the pending UserRequest is one open inbox item");
    assert_eq!(items[0]["kind"], "USER_REQUEST");
    assert_eq!(items[0]["user_request_id"], USER_REQUEST);
    let open_item_id = items[0]["item_id"].as_str().expect("stable item identity");

    let request = authenticated(
        Request::post(format!("/v1/user-requests/{USER_REQUEST}/respond"))
            .header("x-workspace-id", WORKSPACE)
            .header("idempotency-key", "needs-you-respond-once")
            .header("if-match", "1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"response":"yes"}"#.to_owned()))
            .expect("request is valid"),
    );
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("no-store"))
    );

    assert_eq!(
        needs_you_page(&router, "OPEN").await["items"],
        json!([]),
        "answering the source removes the item from the open inbox"
    );
    let history = needs_you_page(&router, "RESOLVED").await;
    let history_items = history["items"].as_array().expect("resolved item array");
    assert_eq!(history_items.len(), 1);
    assert_eq!(history_items[0]["item_id"], open_item_id);
    assert_eq!(history_items[0]["status"], "RESOLVED");
    assert_eq!(history_items[0]["user_request_id"], USER_REQUEST);
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
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("no-store")),
        "the private inbox projection must never be cacheable"
    );
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
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("no-store")),
        "a rejected inbox read must not be cacheable either"
    );
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
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("no-store"))
    );
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
