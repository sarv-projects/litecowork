use super::{AuthenticatedLocalPeer, authenticate, dispatch_ipc_exchange};
use axum::{
    Json, Router,
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::get,
};
use operator_ipc::{
    DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, InFlightBodyBudget, LogicalHeader, PROTOCOL_VERSION,
    RequestFrame, RequestHeader, unix::UnixEndpoint,
};
use serde_json::{Value, json};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    sync::Arc,
};
use tower::ServiceExt;

fn readiness_router() -> Router {
    Router::new()
        .route(
            "/v1/operator/readiness",
            get(|| async { Json(json!({"operator_state": "SERVING"})) }),
        )
        .route_layer(middleware::from_fn(authenticate))
}

#[tokio::test]
async fn local_ipc_peer_marker_is_required_by_the_operator_authentication_middleware() {
    let unauthenticated = readiness_router()
        .oneshot(
            Request::get("/v1/operator/readiness")
                .body(Body::empty())
                .expect("request is valid"),
        )
        .await
        .expect("router returns a response");
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    // The marker is an internal extension, not a header. This branch models the
    // only trusted source: dispatch_ipc_exchange inserts it after the socket layer
    // has authenticated the kernel-reported peer credentials.
    let mut request = Request::get("/v1/operator/readiness")
        .body(Body::empty())
        .expect("request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let authenticated = readiness_router()
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(authenticated.status(), StatusCode::OK);
}

#[tokio::test]
async fn authenticated_unix_ipc_exchange_reaches_the_shared_operator_middleware() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = std::fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");

    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID peer is accepted");
        dispatch_ipc_exchange(readiness_router(), connection, server_budget)
            .await
            .expect("Operator response is framed and sent");
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-auth-boundary-1".to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/operator/readiness".to_owned(),
            headers: Vec::<LogicalHeader>::new(),
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded readiness request");
    client
        .write_request(&request)
        .await
        .expect("request writes");
    let response = client
        .read_response(&client_budget)
        .await
        .expect("correlated response reads");
    assert_eq!(response.header.status, StatusCode::OK.as_u16());
    let body: Value = serde_json::from_slice(response.body()).expect("JSON readiness body");
    assert_eq!(body, json!({"operator_state": "SERVING"}));
    server.await.expect("server task completes");
}

#[test]
fn ipc_request_frame_rejects_authorization_headers_as_an_authentication_path() {
    // Authorization is intentionally absent from the IPC logical-header allowlist.
    // `RequestFrame::new` runs the same validation used by the wire decoder.
    let budget =
        Arc::new(InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("test budget"));
    let result = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-auth-header-1".to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/operator/readiness".to_owned(),
            headers: vec![LogicalHeader {
                name: "authorization".to_owned(),
                value: "Bearer attacker-controlled".to_owned(),
            }],
            body_length: 0,
        },
        Vec::new(),
        &budget,
    );
    assert!(
        result.is_err(),
        "bearer headers must be rejected by IPC framing"
    );
}
