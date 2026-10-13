use super::*;
use operator_ipc::{ResponseFrame, ResponseHeader, unix::UnixEndpoint};
use std::{
    os::unix::fs::{DirBuilderExt, MetadataExt},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

struct PrivateDirectory(PathBuf);

impl PrivateDirectory {
    fn new() -> Self {
        let unique = format!(
            "lc-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the Unix epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .expect("private test directory is created with owner-only permissions");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Debug)]
struct ObservedRequest {
    method: String,
    path_and_query: String,
    headers: Vec<LogicalHeader>,
    body: Vec<u8>,
}

type ResponseHandler = Box<dyn Fn(&ObservedRequest) -> (u16, Vec<u8>) + Send + Sync + 'static>;

struct FixtureServer(Option<JoinHandle<()>>);

impl FixtureServer {
    fn join(mut self) -> std::thread::Result<()> {
        self.0
            .take()
            .expect("fixture join handle is present")
            .join()
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        if let Some(server) = self.0.take() {
            let _ = server.join();
        }
    }
}

fn spawn_operator_fixture(
    data_dir: &std::path::Path,
    request_count: usize,
    response_for: ResponseHandler,
) -> (Arc<Mutex<Vec<ObservedRequest>>>, FixtureServer) {
    let endpoint = UnixEndpoint::derive(data_dir).expect("fixture endpoint derives");
    let expected_uid = fs::metadata(data_dir)
        .expect("fixture directory metadata is readable")
        .uid();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let server_observed = Arc::clone(&observed);
    let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(0);
    let server = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("fixture runtime starts");
        let listener = {
            let _runtime_context = runtime.enter();
            endpoint.bind().expect("fixture endpoint binds")
        };
        ready_sender
            .send(())
            .expect("fixture startup waiter is present");
        runtime.block_on(async move {
            let budget =
                Arc::new(InFlightBodyBudget::new(1024 * 1024).expect("fixture budget is valid"));
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            for _ in 0..request_count {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                let mut connection = tokio::time::timeout(
                    remaining,
                    listener.accept(|peer| peer.uid == expected_uid),
                )
                .await
                .expect("Tauri connects before fixture deadline")
                .expect("fixture authenticates the same-UID Tauri peer");
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                let request = connection.read_request(&budget);
                let request = tokio::time::timeout(remaining, request)
                    .await
                    .expect("Tauri request completes before fixture deadline")
                    .expect("Tauri request frame is valid");
                let request_id = request.header.request_id.clone();
                let body = request.body().to_vec();
                let observed_request = ObservedRequest {
                    method: request.header.method,
                    path_and_query: request.header.path_and_query,
                    headers: request.header.headers,
                    body,
                };
                let (status, response_body) = response_for(&observed_request);
                server_observed
                    .lock()
                    .expect("request observation lock is healthy")
                    .push(observed_request);
                let frame = ResponseFrame::new(
                    ResponseHeader {
                        protocol_version: PROTOCOL_VERSION,
                        request_id,
                        status,
                        headers: vec![LogicalHeader {
                            name: "Content-Type".to_owned(),
                            value: "application/json".to_owned(),
                        }],
                        body_length: response_body.len() as u64,
                    },
                    response_body,
                    &budget,
                )
                .expect("fixture response frame is valid");
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                tokio::time::timeout(remaining, connection.write_response(&frame))
                    .await
                    .expect("fixture response completes before deadline")
                    .expect("fixture response is correlated and sent");
            }
        });
    });
    ready_receiver
        .recv()
        .expect("fixture endpoint is ready before the client connects");
    (observed, FixtureServer(Some(server)))
}

fn json_bytes(value: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("fixture JSON serializes")
}

fn header<'a>(request: &'a ObservedRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}

#[test]
fn workspace_instruction_bridge_uses_authenticated_ipc_and_maps_pages_and_create() {
    let directory = PrivateDirectory::new();
    let revision = |number: u64, parents: Vec<u64>| {
        serde_json::json!({
            "workspace_id": "workspace-1",
            "revision": number,
            "parent_revisions": parents,
            "content_ref": {
                "workspace_id": "workspace-1",
                "resource_id": format!("resource-{number}"),
                "revision_id": format!("resource-revision-{number}"),
            },
            "content_digest": format!("sha256:{}", "a".repeat(64)),
            "authored_by": {"principal_id": "owner-1", "kind": "USER"},
            "created_at": "2026-10-13T12:00:00Z",
        })
    };
    let response_for: ResponseHandler = Box::new(move |request| {
        match (request.method.as_str(), request.path_and_query.as_str()) {
            ("GET", "/v1/operator/readiness") => (
                200,
                json_bytes(serde_json::json!({
                    "operator_state": "SERVING",
                    "runtime_id": "runtime-1",
                    "local_incarnation_id": "incarnation-1",
                    "api_contract_version": 1,
                })),
            ),
            ("GET", "/v1/workspaces/workspace-1/instructions/revisions?limit=200") => (
                200,
                json_bytes(serde_json::json!({
                    "items": [revision(1, vec![])],
                    "next_cursor": "after / page?",
                })),
            ),
            (
                "GET",
                "/v1/workspaces/workspace-1/instructions/revisions?limit=200&cursor=after+%2F+page%3F",
            ) => (
                200,
                json_bytes(serde_json::json!({
                    "items": [revision(2, vec![1])],
                    "next_cursor": null,
                })),
            ),
            ("POST", "/v1/workspaces/workspace-1/instructions/revisions") => {
                (201, json_bytes(revision(3, vec![2])))
            }
            _ => (
                500,
                json_bytes(serde_json::json!({"error": {"code": "UNEXPECTED_REQUEST"}})),
            ),
        }
    });
    let (observed, server) = spawn_operator_fixture(directory.path(), 4, response_for);

    operator_ipc::require_os_local_ipc().expect("host supports authenticated local IPC");
    let client = LocalOperatorClient::from_data_dir(directory.path())
        .expect("production client derives the trusted private endpoint");
    let (client, readiness) =
        validate_operator_client_until(client, None, None, Instant::now() + Duration::from_secs(3))
            .expect("readiness is validated over the production IPC client");
    assert_eq!(readiness.runtime_id, "runtime-1");
    assert_eq!(readiness.local_incarnation_id, "incarnation-1");

    let revisions =
        list_workspace_instructions_with_client(&client, "/v1/workspaces", "workspace-1")
            .expect("all instruction pages are read");
    assert_eq!(
        revisions
            .iter()
            .map(|item| item.revision)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let first_view = serde_json::to_value(&revisions[0]).expect("view serializes");
    assert_eq!(first_view["workspaceId"], "workspace-1");
    assert_eq!(first_view["revision"], 1);
    assert_eq!(first_view["contentRef"]["resource_id"], "resource-1");
    assert_eq!(first_view["authoredBy"]["kind"], "USER");

    let created = create_workspace_instruction_revision_with_client(
        &client,
        "/v1/workspaces",
        WorkspaceInstructionCreateInput {
            workspace_id: "workspace-1".to_owned(),
            expected_version: 3,
            parent_revisions: Vec::new(),
            resource_id: "resource-3".to_owned(),
            resource_revision_id: "resource-revision-3".to_owned(),
            content_digest: format!("sha256:{}", "a".repeat(64)),
            request_id: "instruction-save-1".to_owned(),
        },
    )
    .expect("create response maps to the desktop view");
    assert_eq!(created.revision, 3);
    assert_eq!(created.parent_revisions, vec![2]);
    assert_eq!(created.workspace_id, "workspace-1");

    server.join().expect("all fixture exchanges complete");
    let requests = observed.lock().expect("request log is healthy");
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path_and_query, "/v1/operator/readiness");
    assert_eq!(
        requests[1].path_and_query,
        "/v1/workspaces/workspace-1/instructions/revisions?limit=200"
    );
    assert_eq!(header(&requests[1], "X-Workspace-ID"), Some("workspace-1"));
    assert_eq!(
        requests[2].path_and_query,
        "/v1/workspaces/workspace-1/instructions/revisions?limit=200&cursor=after+%2F+page%3F"
    );
    assert_eq!(requests[3].method, "POST");
    assert_eq!(header(&requests[3], "X-Workspace-ID"), Some("workspace-1"));
    assert_eq!(header(&requests[3], "If-Match"), Some("3"));
    assert_eq!(
        header(&requests[3], "Idempotency-Key"),
        Some("instruction-save-1")
    );
    assert_eq!(
        header(&requests[3], "Content-Type"),
        Some("application/json")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[3].body)
            .expect("create body is valid JSON"),
        serde_json::json!({
            "parent_revisions": [],
            "content_ref": {
                "workspace_id": "workspace-1",
                "resource_id": "resource-3",
                "revision_id": "resource-revision-3",
            },
            "content_digest": format!("sha256:{}", "a".repeat(64)),
        })
    );
}

#[test]
fn workspace_instruction_bridge_preserves_stale_version_error_mapping() {
    let directory = PrivateDirectory::new();
    let response_for: ResponseHandler = Box::new(|request| {
        if request.path_and_query == "/v1/operator/readiness" {
            (
                200,
                json_bytes(serde_json::json!({
                    "operator_state": "SERVING",
                    "runtime_id": "runtime-1",
                    "local_incarnation_id": "incarnation-1",
                    "api_contract_version": 1,
                })),
            )
        } else {
            (
                409,
                json_bytes(serde_json::json!({
                    "error": {"code": "STALE_WORKSPACE_VERSION"}
                })),
            )
        }
    });
    let (observed, server) = spawn_operator_fixture(directory.path(), 2, response_for);
    operator_ipc::require_os_local_ipc().expect("host supports authenticated local IPC");
    let client = LocalOperatorClient::from_data_dir(directory.path())
        .expect("production client derives the trusted private endpoint");
    let (client, _) =
        validate_operator_client_until(client, None, None, Instant::now() + Duration::from_secs(3))
            .expect("fixture Runtime is ready");

    let error = create_workspace_instruction_revision_with_client(
        &client,
        "/v1/workspaces",
        WorkspaceInstructionCreateInput {
            workspace_id: "workspace-1".to_owned(),
            expected_version: 2,
            parent_revisions: vec![1],
            resource_id: "resource-2".to_owned(),
            resource_revision_id: "resource-revision-2".to_owned(),
            content_digest: format!("sha256:{}", "b".repeat(64)),
            request_id: "instruction-save-2".to_owned(),
        },
    )
    .expect_err("stale Workspace versions remain explicit");
    assert_eq!(error, "STALE_WORKSPACE_VERSION");
    server.join().expect("fixture exchanges complete");
    let requests = observed.lock().expect("request log is healthy");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].method, "POST");
    assert_eq!(
        requests[1].path_and_query,
        "/v1/workspaces/workspace-1/instructions/revisions"
    );
    assert_eq!(header(&requests[1], "If-Match"), Some("2"));
}

#[test]
fn workspace_instruction_bridge_keeps_workspace_id_inside_one_path_segment() {
    let workspace_id = "workspace?section=notes&extra=1#latest";
    assert_eq!(
        workspace_instruction_revisions_path("/v1/workspaces", workspace_id)
            .expect("URI delimiters are safe inside an encoded path segment"),
        "/v1/workspaces/workspace%3Fsection%3Dnotes%26extra%3D1%23latest/instructions/revisions"
    );
}

#[test]
fn workspace_instruction_bridge_sends_encoded_workspace_id_to_operator() {
    let directory = PrivateDirectory::new();
    let response_for: ResponseHandler = Box::new(|request| {
        if request.path_and_query == "/v1/operator/readiness" {
            (
                200,
                json_bytes(serde_json::json!({
                    "operator_state": "SERVING",
                    "runtime_id": "runtime-1",
                    "local_incarnation_id": "incarnation-1",
                    "api_contract_version": 1,
                })),
            )
        } else {
            (
                200,
                json_bytes(serde_json::json!({"items": [], "next_cursor": null})),
            )
        }
    });
    let (observed, server) = spawn_operator_fixture(directory.path(), 2, response_for);
    operator_ipc::require_os_local_ipc().expect("host supports authenticated local IPC");
    let client = LocalOperatorClient::from_data_dir(directory.path())
        .expect("production client derives the trusted private endpoint");
    let (client, _) =
        validate_operator_client_until(client, None, None, Instant::now() + Duration::from_secs(3))
            .expect("fixture Runtime is ready");

    let workspace_id = "workspace?section=notes&extra=1#latest";
    list_workspace_instructions_with_client(&client, "/v1/workspaces", workspace_id)
        .expect("reserved URI characters remain part of the Workspace identifier");

    server.join().expect("fixture exchanges complete");
    let requests = observed.lock().expect("request log is healthy");
    assert_eq!(
        requests[1].path_and_query,
        "/v1/workspaces/workspace%3Fsection%3Dnotes%26extra%3D1%23latest/instructions/revisions?limit=200"
    );
    assert_eq!(header(&requests[1], "X-Workspace-ID"), Some(workspace_id));
}

#[test]
fn workspace_instruction_bridge_rejects_ids_forbidden_by_ipc_path_guard() {
    for workspace_id in ["team/workspace", "team\\workspace", ".", ".."] {
        assert!(workspace_instruction_revisions_path("/v1/workspaces", workspace_id).is_err());
    }
}
