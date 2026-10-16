use super::*;

use axum::http::header::CONTENT_TYPE;
use base64::Engine as _;
use std::{fs, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tower::ServiceExt;

#[tokio::test]
async fn authenticated_quick_import_persists_route_payload_without_upload_session() {
    const WORKSPACE: &str = "quick-import-route-workspace";
    const OWNER: &str = "quick-import-route-owner";
    const RUNTIME: &str = "quick-import-route-runtime";
    const NOW: &str = "2026-10-15T12:00:00Z";
    let directory = tempfile::tempdir().expect("temporary quick-import directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private test directory");
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
            name: "Quick import route fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "quick-import-route-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "quick-import-route-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits");
    let (_, stream_shutdown) = tokio::sync::watch::channel(false);
    let router = super::super::build_operator_router(super::super::ApiState {
        store: store.clone(),
        principal_id: OWNER.to_owned(),
        runtime_id: RUNTIME.to_owned(),
        local_incarnation_id: "quick-import-route-incarnation".to_owned(),
        lifecycle_blockers: vec![],
        runtime_identity: Arc::new(
            storage_sqlite::RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                .expect("test Runtime identity"),
        ),
        stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
        runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        stream_shutdown,
    });

    async fn submit(
        router: &Router,
        display_name: &str,
        media_type: &str,
        content: &[u8],
    ) -> Response {
        let body = serde_json::json!({
            "display_name": display_name,
            "media_type": media_type,
            "content_base64": base64::engine::general_purpose::STANDARD.encode(content),
        });
        let mut request = Request::post("/v1/resources/quick-import")
            .header("x-workspace-id", WORKSPACE)
            .header("idempotency-key", "quick-import-route-create")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&body).expect("serialize request"),
            ))
            .expect("quick-import request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("Operator responds")
    }

    let content = b"quick import without resumable upload";
    let response = submit(&router, "notes.txt", "text/plain", content).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let response_body = to_bytes(response.into_body(), 4096)
        .await
        .expect("bounded Resource receipt");
    let receipt: serde_json::Value =
        serde_json::from_slice(&response_body).expect("Resource receipt is JSON");
    assert_eq!(receipt["workspaceId"], WORKSPACE);
    assert_eq!(receipt["displayName"], "notes.txt");
    assert_eq!(receipt["sizeBytes"], content.len() as u64);
    assert_eq!(
        receipt["contentDigest"],
        format!("sha256:{}", hex::encode(sha2::Sha256::digest(content)))
    );
    let resource_id = receipt["resourceId"].as_str().expect("Resource ID");
    let stored = store
        .read_resource_content(WORKSPACE, resource_id)
        .expect("Resource content read succeeds")
        .expect("Resource exists");
    assert_eq!(stored.content, content);

    let replay = submit(&router, "notes.txt", "text/plain", content).await;
    assert_eq!(replay.status(), StatusCode::CREATED);
    assert_eq!(
        to_bytes(replay.into_body(), 4096)
            .await
            .expect("bounded replay receipt"),
        response_body,
        "an exact retry returns the committed Resource receipt"
    );

    let changed_content =
        submit(&router, "notes.txt", "text/plain", b"changed retry payload").await;
    assert_eq!(changed_content.status(), StatusCode::CONFLICT);

    let changed_name = submit(&router, "renamed.txt", "text/plain", content).await;
    assert_eq!(changed_name.status(), StatusCode::CONFLICT);

    let changed_media_type = submit(&router, "notes.txt", "text/markdown", content).await;
    assert_eq!(changed_media_type.status(), StatusCode::CONFLICT);
    assert_eq!(
        store
            .list_resources_page(WORKSPACE, None, None, 10)
            .expect("Resource catalog reads")
            .len(),
        1,
        "the quick import creates one durable Resource"
    );
}

#[tokio::test]
async fn authenticated_quick_import_replays_after_workspace_archive() {
    const WORKSPACE: &str = "quick-import-archive-workspace";
    const OWNER: &str = "quick-import-archive-owner";
    const RUNTIME: &str = "quick-import-archive-runtime";
    const REQUEST_ID: &str = "quick-import-archive-create";
    const NOW: &str = "2026-10-15T12:00:00Z";

    let directory = tempfile::tempdir().expect("temporary quick-import directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private test directory");
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
            name: "Quick import archive fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "quick-import-archive-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "quick-import-archive-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits");
    let (_, stream_shutdown) = tokio::sync::watch::channel(false);
    let router = super::super::build_operator_router(super::super::ApiState {
        store: store.clone(),
        principal_id: OWNER.to_owned(),
        runtime_id: RUNTIME.to_owned(),
        local_incarnation_id: "quick-import-archive-incarnation".to_owned(),
        lifecycle_blockers: vec![],
        runtime_identity: Arc::new(
            storage_sqlite::RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                .expect("test Runtime identity"),
        ),
        stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
        runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        stream_shutdown,
    });

    async fn submit(
        router: &Router,
        request_id: &str,
        display_name: &str,
        media_type: &str,
        content: &[u8],
    ) -> Response {
        let body = serde_json::json!({
            "display_name": display_name,
            "media_type": media_type,
            "content_base64": base64::engine::general_purpose::STANDARD.encode(content),
        });
        let mut request = Request::post("/v1/resources/quick-import")
            .header("x-workspace-id", WORKSPACE)
            .header("idempotency-key", request_id)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&body).expect("serialize request"),
            ))
            .expect("quick-import request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("Operator responds")
    }

    async fn archive(router: &Router) -> Response {
        let mut request = Request::post(format!("/v1/workspaces/{WORKSPACE}/archive"))
            .header("x-workspace-id", WORKSPACE)
            .header("if-match", "1")
            .header("idempotency-key", "quick-import-archive-workspace")
            .body(Body::empty())
            .expect("Workspace archive request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("Workspace archive route responds")
    }

    let content = b"quick import receipt remains replayable";
    let created = submit(&router, REQUEST_ID, "notes.txt", "text/plain", content).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let original_body = to_bytes(created.into_body(), 4096)
        .await
        .expect("bounded Resource response");
    let original: serde_json::Value =
        serde_json::from_slice(&original_body).expect("Resource receipt is JSON");

    let archived = archive(&router).await;
    assert_eq!(archived.status(), StatusCode::OK);
    let archived_body = to_bytes(archived.into_body(), 4096)
        .await
        .expect("bounded Workspace response");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&archived_body)
            .expect("Workspace response is JSON")["status"],
        "ARCHIVED"
    );

    let replay = submit(&router, REQUEST_ID, "notes.txt", "text/plain", content).await;
    assert_eq!(replay.status(), StatusCode::CREATED);
    assert_eq!(
        to_bytes(replay.into_body(), 4096)
            .await
            .expect("bounded replay response"),
        original_body,
        "an exact retry must return the original committed response after archival"
    );

    let changed = submit(
        &router,
        REQUEST_ID,
        "notes.txt",
        "text/plain",
        b"different payload under the same key",
    )
    .await;
    assert_eq!(changed.status(), StatusCode::CONFLICT);
    let changed_body = to_bytes(changed.into_body(), 4096)
        .await
        .expect("bounded conflict response");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&changed_body)
            .expect("conflict response is JSON")["code"],
        "CONFLICT"
    );

    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open persisted quick-import ledger");
    let resource_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resources WHERE workspace_id = ?1",
            [WORKSPACE],
            |row| row.get(0),
        )
        .expect("count Resources");
    let create_event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'resource.created.v1'",
            [WORKSPACE],
            |row| row.get(0),
        )
        .expect("count Resource creation events");
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            rusqlite::params![OWNER, REQUEST_ID],
            |row| row.get(0),
        )
        .expect("count idempotency receipts");
    assert_eq!(resource_count, 1, "one Resource remains committed");
    assert_eq!(
        create_event_count, 1,
        "one Resource event remains committed"
    );
    assert_eq!(
        receipt_count, 1,
        "one idempotency receipt remains committed"
    );
    assert!(original["resourceId"].as_str().is_some());
}
