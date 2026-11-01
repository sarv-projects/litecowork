use super::*;

use axum::http::header::CONTENT_TYPE;
use rusqlite::Connection;
use sha2::Digest as _;
use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Arc, time::Duration};
use storage_core::ResourceUploadStore;
use tower::ServiceExt;

const FIXTURE_TIME: &str = "2026-10-15T12:00:00Z";

struct UploadArchiveFixture {
    directory: tempfile::TempDir,
    store: SqliteWorkspaceStore,
    router: Router,
    workspace_id: String,
    owner_id: String,
}

fn upload_archive_fixture(
    workspace_id: &str,
    owner_id: &str,
    runtime_id: &str,
) -> UploadArchiveFixture {
    let directory = tempfile::tempdir().expect("temporary upload archive directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private upload archive directory");
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
            workspace_id: workspace_id.to_owned(),
            name: "Resource upload archive replay fixture".to_owned(),
            owner_principal_id: owner_id.to_owned(),
            event: EventContext {
                event_id: format!("{workspace_id}-created"),
                origin_runtime_id: runtime_id.to_owned(),
                hlc_timestamp: FIXTURE_TIME.to_owned(),
                correlation_id: format!("{workspace_id}-created"),
                causation_id: None,
                recorded_at: FIXTURE_TIME.to_owned(),
            },
        })
        .expect("owner Workspace commits");
    let router = operator_router(&store, owner_id, runtime_id);

    UploadArchiveFixture {
        directory,
        store,
        router,
        workspace_id: workspace_id.to_owned(),
        owner_id: owner_id.to_owned(),
    }
}

fn create_owned_workspace(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    owner_id: &str,
    runtime_id: &str,
) {
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Second owner upload-key scope fixture".to_owned(),
            owner_principal_id: owner_id.to_owned(),
            event: EventContext {
                event_id: format!("{workspace_id}-created"),
                origin_runtime_id: runtime_id.to_owned(),
                hlc_timestamp: FIXTURE_TIME.to_owned(),
                correlation_id: format!("{workspace_id}-created"),
                causation_id: None,
                recorded_at: FIXTURE_TIME.to_owned(),
            },
        })
        .expect("second owner's Workspace commits into the existing SQLite store");
}

fn operator_router(store: &SqliteWorkspaceStore, owner_id: &str, runtime_id: &str) -> Router {
    let (_, stream_shutdown) = tokio::sync::watch::channel(false);
    super::super::build_operator_router(super::super::ApiState {
        store: store.clone(),
        principal_id: owner_id.to_owned(),
        runtime_id: runtime_id.to_owned(),
        local_incarnation_id: format!("upload-archive-incarnation-{owner_id}"),
        lifecycle_blockers: vec![],
        runtime_identity: Arc::new(
            storage_sqlite::RuntimeOsPrincipalIdentity::test_support(runtime_id)
                .expect("test Runtime identity"),
        ),
        stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
        runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        stream_shutdown,
    })
}

async fn send_authenticated(router: &Router, mut request: Request<Body>) -> Response {
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated Operator route responds")
}

fn sha256(content: &[u8]) -> String {
    format!("sha256:{:x}", sha2::Sha256::digest(content))
}

async fn create_upload(
    fixture: &UploadArchiveFixture,
    request_id: &str,
    display_name: &str,
    content: &[u8],
) -> Response {
    create_upload_for(
        &fixture.router,
        &fixture.workspace_id,
        request_id,
        display_name,
        content,
    )
    .await
}

async fn create_upload_for(
    router: &Router,
    workspace_id: &str,
    request_id: &str,
    display_name: &str,
    content: &[u8],
) -> Response {
    let body = serde_json::json!({
        "workspace_id": workspace_id,
        "display_name": display_name,
        "media_type": "text/plain",
        "size_bytes": content.len(),
        "expected_digest": sha256(content),
    });
    let request = Request::post("/v1/resources/uploads")
        .header("x-workspace-id", workspace_id)
        .header("idempotency-key", request_id)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&body).expect("serialize upload request"),
        ))
        .expect("upload request builds");
    send_authenticated(router, request).await
}

async fn send_chunk(
    fixture: &UploadArchiveFixture,
    upload_id: &str,
    chunk_index: u64,
    request_id: &str,
    start: usize,
    total: usize,
    content: &[u8],
) -> Response {
    send_chunk_for(
        &fixture.router,
        &fixture.workspace_id,
        upload_id,
        chunk_index,
        request_id,
        start,
        total,
        content,
    )
    .await
}

#[allow(clippy::too_many_arguments)] // Mirrors the chunk route's independently asserted wire fields.
async fn send_chunk_for(
    router: &Router,
    workspace_id: &str,
    upload_id: &str,
    chunk_index: u64,
    request_id: &str,
    start: usize,
    total: usize,
    content: &[u8],
) -> Response {
    assert!(!content.is_empty() && start + content.len() <= total);
    let end = start + content.len() - 1;
    let request = Request::put(format!(
        "/v1/resources/uploads/{upload_id}/chunks/{chunk_index}"
    ))
    .header("x-workspace-id", workspace_id)
    .header("idempotency-key", request_id)
    .header("content-range", format!("bytes {start}-{end}/{total}"))
    .header(
        "x-chunk-sha256",
        format!("{:x}", sha2::Sha256::digest(content)),
    )
    .body(Body::from(content.to_vec()))
    .expect("upload chunk request builds");
    send_authenticated(router, request).await
}

async fn archive_workspace(fixture: &UploadArchiveFixture, request_id: &str) -> Response {
    let request = Request::post(format!("/v1/workspaces/{}/archive", fixture.workspace_id))
        .header("x-workspace-id", &fixture.workspace_id)
        .header("if-match", "1")
        .header("idempotency-key", request_id)
        .body(Body::empty())
        .expect("Workspace archive request builds");
    send_authenticated(&fixture.router, request).await
}

async fn commit_upload(
    fixture: &UploadArchiveFixture,
    upload_id: &str,
    request_id: &str,
) -> Response {
    commit_upload_for(
        &fixture.router,
        &fixture.workspace_id,
        upload_id,
        request_id,
    )
    .await
}

async fn commit_upload_for(
    router: &Router,
    workspace_id: &str,
    upload_id: &str,
    request_id: &str,
) -> Response {
    let request = Request::post(format!("/v1/resources/uploads/{upload_id}/commit"))
        .header("x-workspace-id", workspace_id)
        .header("idempotency-key", request_id)
        .body(Body::empty())
        .expect("Resource commit request builds");
    send_authenticated(router, request).await
}

async fn response_body(response: Response) -> Bytes {
    to_bytes(response.into_body(), 16 * 1024)
        .await
        .expect("bounded Operator response body")
}

#[derive(Debug, Eq, PartialEq)]
struct UploadLedgerCounts {
    chunks: i64,
    chunk_receipts: i64,
    reservations: i64,
    blob_registry_rows: i64,
    lifecycle_events: i64,
}

fn upload_ledger_counts(fixture: &UploadArchiveFixture, upload_id: &str) -> UploadLedgerCounts {
    let connection = Connection::open(fixture.directory.path().join("state.sqlite3"))
        .expect("open upload ledger database");
    let count = |sql: &str| {
        connection
            .query_row(sql, [upload_id], |row| row.get(0))
            .expect("count upload ledger rows")
    };
    UploadLedgerCounts {
        chunks: count("SELECT COUNT(*) FROM resource_upload_chunks WHERE upload_id = ?1"),
        chunk_receipts: count(
            "SELECT COUNT(*) FROM resource_upload_chunk_requests WHERE upload_id = ?1",
        ),
        reservations: count(
            "SELECT COUNT(*) FROM resource_upload_blob_reservations WHERE upload_id = ?1",
        ),
        blob_registry_rows: connection
            .query_row(
                "SELECT COUNT(*) FROM resource_upload_chunk_blob_registry WHERE workspace_id = ?1",
                [&fixture.workspace_id],
                |row| row.get(0),
            )
            .expect("count upload blob registry rows"),
        lifecycle_events: connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE entity_id = ?1 AND type IN ('resource.upload.created.v1', 'resource.upload.status.changed.v1')",
                [upload_id],
                |row| row.get(0),
            )
            .expect("count upload lifecycle events"),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct UploadCreateLedgerCounts {
    sessions: i64,
    upload_events: i64,
    create_receipts: i64,
}

fn upload_create_ledger_counts(
    fixture: &UploadArchiveFixture,
    request_id: &str,
    fresh_request_id: &str,
) -> UploadCreateLedgerCounts {
    let connection = Connection::open(fixture.directory.path().join("state.sqlite3"))
        .expect("open upload-create ledger database");
    UploadCreateLedgerCounts {
        sessions: connection
            .query_row(
                "SELECT COUNT(*) FROM resource_upload_sessions WHERE workspace_id = ?1",
                [&fixture.workspace_id],
                |row| row.get(0),
            )
            .expect("count upload sessions"),
        upload_events: connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'resource.upload.created.v1'",
                [&fixture.workspace_id],
                |row| row.get(0),
            )
            .expect("count upload-create events"),
        create_receipts: connection
            .query_row(
                "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id IN (?2, ?3)",
                rusqlite::params![fixture.owner_id, request_id, fresh_request_id],
                |row| row.get(0),
            )
            .expect("count upload-create request receipts"),
    }
}

fn file_count_recursively(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    fs::read_dir(path)
        .expect("read private blob directory")
        .map(|entry| {
            let entry = entry.expect("read blob directory entry");
            let file_type = entry.file_type().expect("read blob entry type");
            if file_type.is_dir() {
                file_count_recursively(&entry.path())
            } else if file_type.is_file() {
                1
            } else {
                0
            }
        })
        .sum()
}

#[tokio::test]
async fn chunk_receipt_replays_after_archive_and_new_chunk_is_rejected_without_blob() {
    const WORKSPACE: &str = "upload-chunk-archive-workspace";
    const OWNER: &str = "upload-chunk-archive-owner";
    const RUNTIME: &str = "upload-chunk-archive-runtime";
    const CREATE_KEY: &str = "upload-chunk-archive-create";
    const CHUNK_KEY: &str = "upload-chunk-archive-part-one";
    const CHUNK_SIZE: usize = 4_194_304;
    const FINAL_CHUNK: &[u8] = b"tail";
    let mut whole_content = vec![b'a'; CHUNK_SIZE];
    whole_content.extend_from_slice(FINAL_CHUNK);
    let first_chunk = &whole_content[..CHUNK_SIZE];
    let changed_chunk = vec![b'z'; CHUNK_SIZE];
    let total = whole_content.len();

    let fixture = upload_archive_fixture(WORKSPACE, OWNER, RUNTIME);
    let created = create_upload(&fixture, CREATE_KEY, "partial.txt", &whole_content).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = response_body(created).await;
    let session: serde_json::Value =
        serde_json::from_slice(&created_body).expect("upload session response is JSON");
    let upload_id = session["upload_id"]
        .as_str()
        .expect("upload response contains its upload_id");

    let accepted_chunk = send_chunk(&fixture, upload_id, 0, CHUNK_KEY, 0, total, first_chunk).await;
    assert_eq!(accepted_chunk.status(), StatusCode::NO_CONTENT);
    let accepted_counts = upload_ledger_counts(&fixture, upload_id);
    assert_eq!(accepted_counts.chunks, 1);
    assert_eq!(accepted_counts.chunk_receipts, 1);
    assert_eq!(accepted_counts.reservations, 0);
    assert_eq!(accepted_counts.lifecycle_events, 1);
    let archived = archive_workspace(&fixture, "upload-chunk-archive-workspace-archive").await;
    assert_eq!(archived.status(), StatusCode::OK);
    let archived_blob_count = file_count_recursively(&fixture.directory.path().join("blobs"));

    let exact_retry = send_chunk(&fixture, upload_id, 0, CHUNK_KEY, 0, total, first_chunk).await;
    assert_eq!(exact_retry.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        upload_ledger_counts(&fixture, upload_id),
        accepted_counts,
        "exact chunk replay returns its receipt without adding a chunk, receipt, or event"
    );

    let changed_retry =
        send_chunk(&fixture, upload_id, 0, CHUNK_KEY, 0, total, &changed_chunk).await;
    assert_eq!(changed_retry.status(), StatusCode::CONFLICT);
    let changed_error: serde_json::Value =
        serde_json::from_slice(&response_body(changed_retry).await)
            .expect("changed chunk conflict is JSON");
    assert_eq!(
        changed_error["code"], "UPLOAD_OFFSET_CONFLICT",
        "the archived route must compare the stored request payload before mutable-state admission"
    );

    let counts_before_new_request = upload_ledger_counts(&fixture, upload_id);
    let blobs_before_new_request = file_count_recursively(&fixture.directory.path().join("blobs"));
    assert_eq!(counts_before_new_request, accepted_counts);
    assert_eq!(blobs_before_new_request, archived_blob_count);

    let fresh_chunk = send_chunk(
        &fixture,
        upload_id,
        1,
        "upload-chunk-archive-part-two",
        CHUNK_SIZE,
        total,
        FINAL_CHUNK,
    )
    .await;
    assert_eq!(fresh_chunk.status(), StatusCode::CONFLICT);
    let fresh_error: serde_json::Value = serde_json::from_slice(&response_body(fresh_chunk).await)
        .expect("archived Workspace error is JSON");
    assert_eq!(
        fresh_error["code"], "WORKSPACE_ARCHIVED",
        "a new chunk request must fail at the archived Workspace boundary"
    );
    assert_eq!(
        upload_ledger_counts(&fixture, upload_id),
        counts_before_new_request,
        "a fresh chunk request against an archived Workspace creates no durable rows"
    );
    assert_eq!(
        file_count_recursively(&fixture.directory.path().join("blobs")),
        blobs_before_new_request,
        "a fresh rejected chunk creates no encrypted blob"
    );

    // Retain the Store through the final ledger assertions so the test covers the same
    // live SQLite-backed Operator instance throughout the archive and replay sequence.
    assert_eq!(
        fixture
            .store
            .get(&fixture.workspace_id, upload_id)
            .expect("upload session remains readable")
            .expect("upload session remains durable")
            .received_ranges
            .len(),
        1
    );
}

#[tokio::test]
async fn upload_create_exact_receipt_replays_after_archive_without_duplicate_records() {
    const WORKSPACE: &str = "upload-create-archive-workspace";
    const OWNER: &str = "upload-create-archive-owner";
    const RUNTIME: &str = "upload-create-archive-runtime";
    const CREATE_KEY: &str = "upload-create-archive-create";
    const FRESH_KEY: &str = "upload-create-archive-fresh";
    const CONTENT: &[u8] = b"upload creation receipt remains durable";

    let fixture = upload_archive_fixture(WORKSPACE, OWNER, RUNTIME);
    let created = create_upload(&fixture, CREATE_KEY, "stable.txt", CONTENT).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let original_body = response_body(created).await;
    let original: serde_json::Value =
        serde_json::from_slice(&original_body).expect("created session is JSON");
    let upload_id = original["upload_id"]
        .as_str()
        .expect("created session contains its upload ID");
    let archived = archive_workspace(&fixture, "upload-create-archive-workspace-archive").await;
    assert_eq!(archived.status(), StatusCode::OK);

    let stable_counts = upload_create_ledger_counts(&fixture, CREATE_KEY, FRESH_KEY);
    let stable_blob_count = file_count_recursively(&fixture.directory.path().join("blobs"));
    let exact_retry = create_upload(&fixture, CREATE_KEY, "stable.txt", CONTENT).await;
    assert_eq!(exact_retry.status(), StatusCode::CREATED);
    assert_eq!(response_body(exact_retry).await, original_body);

    let changed_retry = create_upload(&fixture, CREATE_KEY, "changed.txt", CONTENT).await;
    assert_eq!(changed_retry.status(), StatusCode::CONFLICT);

    let fresh_request = create_upload(&fixture, FRESH_KEY, "new.txt", CONTENT).await;
    assert_eq!(fresh_request.status(), StatusCode::CONFLICT);
    let fresh_error: serde_json::Value =
        serde_json::from_slice(&response_body(fresh_request).await)
            .expect("fresh archived request error is JSON");
    assert_eq!(fresh_error["code"], "WORKSPACE_ARCHIVED");
    assert_eq!(
        upload_create_ledger_counts(&fixture, CREATE_KEY, FRESH_KEY),
        stable_counts,
        "exact retry and denied new requests add no session, event, or receipt"
    );
    assert_eq!(
        file_count_recursively(&fixture.directory.path().join("blobs")),
        stable_blob_count,
        "receipt replay and archive denial create no additional state blob"
    );
    assert_eq!(
        fixture
            .store
            .get(&fixture.workspace_id, upload_id)
            .expect("original upload remains readable")
            .expect("original upload remains durable")
            .state,
        storage_core::ResourceUploadState::Open
    );
}

#[derive(Debug, Eq, PartialEq)]
struct CommitLedgerCounts {
    resources: i64,
    upload_lifecycle_events: i64,
    resource_events: i64,
    commit_receipts: i64,
}

fn commit_ledger_counts(
    fixture: &UploadArchiveFixture,
    upload_id: &str,
    resource_id: &str,
) -> CommitLedgerCounts {
    let connection = Connection::open(fixture.directory.path().join("state.sqlite3"))
        .expect("open committed upload database");
    let resources = connection
        .query_row(
            "SELECT COUNT(*) FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
            rusqlite::params![fixture.workspace_id, resource_id],
            |row| row.get(0),
        )
        .expect("count committed Resources");
    let upload_lifecycle_events = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id = ?1 AND type IN ('resource.upload.created.v1', 'resource.upload.status.changed.v1')",
            [upload_id],
            |row| row.get(0),
        )
        .expect("count upload lifecycle events");
    let resource_events = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id = ?1 AND type IN ('resource.created.v1', 'resource.created.v2')",
            [resource_id],
            |row| row.get(0),
        )
        .expect("count Resource creation events");
    let commit_request_id = format!("resource-upload-commit:{upload_id}");
    let commit_receipts = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            rusqlite::params![fixture.owner_id, commit_request_id],
            |row| row.get(0),
        )
        .expect("count Resource commit receipts");
    CommitLedgerCounts {
        resources,
        upload_lifecycle_events,
        resource_events,
        commit_receipts,
    }
}

#[tokio::test]
async fn committed_upload_exact_retry_replays_after_workspace_archive() {
    const WORKSPACE: &str = "upload-commit-archive-workspace";
    const OWNER: &str = "upload-commit-archive-owner";
    const RUNTIME: &str = "upload-commit-archive-runtime";
    const CREATE_KEY: &str = "upload-commit-archive-create";
    const CHUNK_KEY: &str = "upload-commit-archive-chunk";
    const COMMIT_KEY: &str = "upload-commit-archive-commit";
    const RECOVERY_KEY: &str = "upload-commit-archive-recovery";
    const CONTENT: &[u8] = b"committed upload remains replayable";

    let fixture = upload_archive_fixture(WORKSPACE, OWNER, RUNTIME);
    let created = create_upload(&fixture, CREATE_KEY, "committed.txt", CONTENT).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = response_body(created).await;
    let session: serde_json::Value =
        serde_json::from_slice(&created_body).expect("upload session response is JSON");
    let upload_id = session["upload_id"]
        .as_str()
        .expect("upload response contains its upload_id");

    let chunk = send_chunk(&fixture, upload_id, 0, CHUNK_KEY, 0, CONTENT.len(), CONTENT).await;
    assert_eq!(chunk.status(), StatusCode::NO_CONTENT);

    let commit = Request::post(format!("/v1/resources/uploads/{upload_id}/commit"))
        .header("x-workspace-id", WORKSPACE)
        .header("idempotency-key", COMMIT_KEY)
        .body(Body::empty())
        .expect("Resource commit request builds");
    let committed = send_authenticated(&fixture.router, commit).await;
    assert_eq!(committed.status(), StatusCode::CREATED);
    let original_status = committed.status();
    let original_correlation = committed
        .headers()
        .get("x-correlation-id")
        .expect("commit response has the original event correlation")
        .clone();
    let original_body = response_body(committed).await;
    let receipt: serde_json::Value =
        serde_json::from_slice(&original_body).expect("committed Resource response is JSON");
    let resource_id = receipt["resource"]["resource_id"]
        .as_str()
        .expect("commit response includes the Resource identity");
    let committed_counts = commit_ledger_counts(&fixture, upload_id, resource_id);
    assert_eq!(committed_counts.resources, 1);
    assert_eq!(committed_counts.upload_lifecycle_events, 3);
    assert_eq!(committed_counts.resource_events, 1);
    assert_eq!(committed_counts.commit_receipts, 1);

    let archived = archive_workspace(&fixture, "upload-commit-archive-workspace-archive").await;
    assert_eq!(archived.status(), StatusCode::OK);

    let retry = Request::post(format!("/v1/resources/uploads/{upload_id}/commit"))
        .header("x-workspace-id", WORKSPACE)
        .header("idempotency-key", COMMIT_KEY)
        .body(Body::empty())
        .expect("exact Resource commit retry builds");
    let replay = send_authenticated(&fixture.router, retry).await;
    assert_eq!(replay.status(), original_status);
    assert_eq!(
        replay.headers().get("x-correlation-id"),
        Some(&original_correlation),
        "commit replay retains the committed Resource event correlation"
    );
    assert_eq!(
        response_body(replay).await,
        original_body,
        "commit replay returns the exact previously committed Resource receipt"
    );

    let recovery = Request::post(format!("/v1/resources/uploads/{upload_id}/commit"))
        .header("x-workspace-id", WORKSPACE)
        .header("idempotency-key", RECOVERY_KEY)
        .body(Body::empty())
        .expect("deterministic receipt recovery request builds");
    let recovered = send_authenticated(&fixture.router, recovery).await;
    assert_eq!(recovered.status(), original_status);
    assert_eq!(
        recovered.headers().get("x-correlation-id"),
        Some(&original_correlation),
        "recovery alias preserves the original commit correlation"
    );
    assert_eq!(
        response_body(recovered).await,
        original_body,
        "a new caller key recovers the exact Resource from the upload-scoped receipt"
    );
    let recovered_key_receipts = Connection::open(fixture.directory.path().join("state.sqlite3"))
        .expect("open recovered commit receipt database")
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            rusqlite::params![fixture.owner_id, RECOVERY_KEY],
            |row| row.get::<_, i64>(0),
        )
        .expect("count recovered caller-key receipt");
    assert_eq!(recovered_key_receipts, 1);
    assert_eq!(
        commit_ledger_counts(&fixture, upload_id, resource_id),
        committed_counts,
        "post-archive commit replay creates no second Resource, event, or upload-scoped receipt"
    );

    assert_eq!(
        fixture
            .store
            .get(&fixture.workspace_id, upload_id)
            .expect("committed upload remains readable")
            .expect("committed upload remains durable")
            .state,
        storage_core::ResourceUploadState::Committed
    );
}

#[derive(Debug, Eq, PartialEq)]
struct WorkspaceUploadCommitCounts {
    resources: i64,
    resource_events: i64,
    upload_lifecycle_events: i64,
    first_key_receipts: i64,
    second_key_receipts: i64,
}

fn workspace_upload_commit_counts(
    fixture: &UploadArchiveFixture,
    first_upload_id: &str,
    second_upload_id: &str,
    first_request_id: &str,
    second_request_id: &str,
) -> WorkspaceUploadCommitCounts {
    let connection = Connection::open(fixture.directory.path().join("state.sqlite3"))
        .expect("open upload caller-key ledger database");
    let resources = connection
        .query_row(
            "SELECT COUNT(*) FROM resources WHERE workspace_id = ?1",
            [&fixture.workspace_id],
            |row| row.get(0),
        )
        .expect("count Workspace Resources");
    let resource_events = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type IN ('resource.created.v1', 'resource.created.v2')",
            [&fixture.workspace_id],
            |row| row.get(0),
        )
        .expect("count Workspace Resource creation events");
    let upload_lifecycle_events = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id IN (?1, ?2) AND type IN ('resource.upload.created.v1', 'resource.upload.status.changed.v1')",
            rusqlite::params![first_upload_id, second_upload_id],
            |row| row.get(0),
        )
        .expect("count both upload lifecycle event streams");
    let receipt_count = |request_id: &str| {
        connection
            .query_row(
                "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
                rusqlite::params![fixture.owner_id, request_id],
                |row| row.get(0),
            )
            .expect("count caller-key commit receipt")
    };
    WorkspaceUploadCommitCounts {
        resources,
        resource_events,
        upload_lifecycle_events,
        first_key_receipts: receipt_count(first_request_id),
        second_key_receipts: receipt_count(second_request_id),
    }
}

#[tokio::test]
async fn upload_commit_idempotency_key_is_principal_scoped_and_bound_to_one_upload() {
    const WORKSPACE: &str = "upload-commit-key-workspace";
    const OWNER: &str = "upload-commit-key-owner";
    const RUNTIME: &str = "upload-commit-key-runtime";
    const CREATE_A_KEY: &str = "upload-commit-key-create-a";
    const CREATE_B_KEY: &str = "upload-commit-key-create-b";
    const CHUNK_A_KEY: &str = "upload-commit-key-chunk-a";
    const CHUNK_B_KEY: &str = "upload-commit-key-chunk-b";
    const COMMIT_KEY: &str = "upload-commit-key-shared";
    const COMMIT_RETRY_KEY: &str = "upload-commit-key-b-fresh";
    const CONTENT_A: &[u8] = b"upload A caller key binding";
    const CONTENT_B: &[u8] = b"upload B caller key binding";

    let fixture = upload_archive_fixture(WORKSPACE, OWNER, RUNTIME);
    let created_a = create_upload(&fixture, CREATE_A_KEY, "upload-a.txt", CONTENT_A).await;
    assert_eq!(created_a.status(), StatusCode::CREATED);
    let session_a: serde_json::Value =
        serde_json::from_slice(&response_body(created_a).await).expect("upload A session JSON");
    let upload_a_id = session_a["upload_id"]
        .as_str()
        .expect("upload A session has an ID");
    let chunk_a = send_chunk(
        &fixture,
        upload_a_id,
        0,
        CHUNK_A_KEY,
        0,
        CONTENT_A.len(),
        CONTENT_A,
    )
    .await;
    assert_eq!(chunk_a.status(), StatusCode::NO_CONTENT);

    let created_b = create_upload(&fixture, CREATE_B_KEY, "upload-b.txt", CONTENT_B).await;
    assert_eq!(created_b.status(), StatusCode::CREATED);
    let session_b: serde_json::Value =
        serde_json::from_slice(&response_body(created_b).await).expect("upload B session JSON");
    let upload_b_id = session_b["upload_id"]
        .as_str()
        .expect("upload B session has an ID");
    let chunk_b = send_chunk(
        &fixture,
        upload_b_id,
        0,
        CHUNK_B_KEY,
        0,
        CONTENT_B.len(),
        CONTENT_B,
    )
    .await;
    assert_eq!(chunk_b.status(), StatusCode::NO_CONTENT);

    let commit_a = commit_upload(&fixture, upload_a_id, COMMIT_KEY).await;
    assert_eq!(commit_a.status(), StatusCode::CREATED);
    let original_status = commit_a.status();
    let original_correlation = commit_a
        .headers()
        .get("x-correlation-id")
        .expect("upload A commit has a correlation ID")
        .clone();
    let original_body = response_body(commit_a).await;

    let retry_a = commit_upload(&fixture, upload_a_id, COMMIT_KEY).await;
    assert_eq!(retry_a.status(), original_status);
    assert_eq!(
        retry_a.headers().get("x-correlation-id"),
        Some(&original_correlation),
        "an exact caller-key retry keeps the original commit correlation"
    );
    assert_eq!(
        response_body(retry_a).await,
        original_body,
        "an exact caller-key retry returns the byte-identical commit receipt"
    );

    let before_b_reuse = workspace_upload_commit_counts(
        &fixture,
        upload_a_id,
        upload_b_id,
        COMMIT_KEY,
        COMMIT_RETRY_KEY,
    );
    assert_eq!(before_b_reuse.resources, 1);
    assert_eq!(before_b_reuse.resource_events, 1);
    assert_eq!(before_b_reuse.upload_lifecycle_events, 5);
    assert_eq!(before_b_reuse.second_key_receipts, 0);

    let reused_key = commit_upload(&fixture, upload_b_id, COMMIT_KEY).await;
    assert_eq!(
        reused_key.status(),
        StatusCode::CONFLICT,
        "the same Principal cannot spend upload A's caller RequestId on upload B"
    );
    let conflict: serde_json::Value = serde_json::from_slice(&response_body(reused_key).await)
        .expect("caller-key conflict response is JSON");
    assert_eq!(conflict["code"], "CONFLICT");
    let b_after_conflict = fixture
        .store
        .get(&fixture.workspace_id, upload_b_id)
        .expect("read upload B after caller-key conflict")
        .expect("upload B remains durable");
    assert_eq!(
        b_after_conflict.state,
        storage_core::ResourceUploadState::ContentReceived,
        "a rejected key reuse leaves upload B available for an explicit retry"
    );
    assert!(b_after_conflict.committed_resource_id.is_none());
    assert_eq!(
        workspace_upload_commit_counts(
            &fixture,
            upload_a_id,
            upload_b_id,
            COMMIT_KEY,
            COMMIT_RETRY_KEY,
        ),
        before_b_reuse,
        "a rejected cross-upload key reuse adds no Resource, event, or receipt"
    );

    let commit_b = commit_upload(&fixture, upload_b_id, COMMIT_RETRY_KEY).await;
    assert_eq!(commit_b.status(), StatusCode::CREATED);
    let committed_b: serde_json::Value = serde_json::from_slice(&response_body(commit_b).await)
        .expect("upload B commit response is JSON");
    let resource_b_id = committed_b["resource"]["resource_id"]
        .as_str()
        .expect("upload B commit includes a Resource ID");
    assert!(!resource_b_id.is_empty());
    let after_b_commit = workspace_upload_commit_counts(
        &fixture,
        upload_a_id,
        upload_b_id,
        COMMIT_KEY,
        COMMIT_RETRY_KEY,
    );
    assert_eq!(after_b_commit.resources, 2);
    assert_eq!(after_b_commit.resource_events, 2);
    assert_eq!(after_b_commit.upload_lifecycle_events, 6);
    assert_eq!(after_b_commit.first_key_receipts, 1);
    assert_eq!(after_b_commit.second_key_receipts, 1);
    assert_eq!(
        fixture
            .store
            .get(&fixture.workspace_id, upload_b_id)
            .expect("read successfully committed upload B")
            .expect("upload B remains durable")
            .state,
        storage_core::ResourceUploadState::Committed
    );
}

#[derive(Debug, Eq, PartialEq)]
struct OwnedWorkspaceCommitCounts {
    resources: i64,
    resource_events: i64,
    upload_lifecycle_events: i64,
    caller_key_receipts: i64,
}

fn owned_workspace_commit_counts(
    fixture: &UploadArchiveFixture,
    workspace_id: &str,
    owner_id: &str,
    upload_id: &str,
    request_id: &str,
) -> OwnedWorkspaceCommitCounts {
    let connection = Connection::open(fixture.directory.path().join("state.sqlite3"))
        .expect("open shared SQLite commit-key database");
    let resources = connection
        .query_row(
            "SELECT COUNT(*) FROM resources WHERE workspace_id = ?1",
            [workspace_id],
            |row| row.get(0),
        )
        .expect("count owned Workspace Resources");
    let resource_events = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type IN ('resource.created.v1', 'resource.created.v2')",
            [workspace_id],
            |row| row.get(0),
        )
        .expect("count owned Workspace Resource events");
    let upload_lifecycle_events = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id = ?1 AND type IN ('resource.upload.created.v1', 'resource.upload.status.changed.v1')",
            [upload_id],
            |row| row.get(0),
        )
        .expect("count owned upload lifecycle events");
    let caller_key_receipts = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            rusqlite::params![owner_id, request_id],
            |row| row.get(0),
        )
        .expect("count principal-scoped caller commit receipts");
    OwnedWorkspaceCommitCounts {
        resources,
        resource_events,
        upload_lifecycle_events,
        caller_key_receipts,
    }
}

#[tokio::test]
async fn upload_commit_idempotency_key_can_be_used_by_another_principal_in_shared_store() {
    const WORKSPACE_A: &str = "upload-commit-scope-workspace-a";
    const WORKSPACE_B: &str = "upload-commit-scope-workspace-b";
    const OWNER_A: &str = "upload-commit-scope-owner-a";
    const OWNER_B: &str = "upload-commit-scope-owner-b";
    const RUNTIME_A: &str = "upload-commit-scope-runtime-a";
    const RUNTIME_B: &str = "upload-commit-scope-runtime-b";
    const CREATE_A_KEY: &str = "upload-commit-scope-create-a";
    const CREATE_B_KEY: &str = "upload-commit-scope-create-b";
    const CHUNK_A_KEY: &str = "upload-commit-scope-chunk-a";
    const CHUNK_B_KEY: &str = "upload-commit-scope-chunk-b";
    const SHARED_COMMIT_KEY: &str = "upload-commit-scope-shared-commit";
    const CONTENT_A: &[u8] = b"owned Workspace A content";
    const CONTENT_B: &[u8] = b"owned Workspace B content";

    let fixture = upload_archive_fixture(WORKSPACE_A, OWNER_A, RUNTIME_A);
    create_owned_workspace(&fixture.store, WORKSPACE_B, OWNER_B, RUNTIME_B);
    let router_b = operator_router(&fixture.store, OWNER_B, RUNTIME_B);

    let created_a = create_upload(&fixture, CREATE_A_KEY, "owner-a.txt", CONTENT_A).await;
    assert_eq!(created_a.status(), StatusCode::CREATED);
    let session_a: serde_json::Value =
        serde_json::from_slice(&response_body(created_a).await).expect("upload A session JSON");
    let upload_a_id = session_a["upload_id"]
        .as_str()
        .expect("upload A session has an ID");
    let chunk_a = send_chunk(
        &fixture,
        upload_a_id,
        0,
        CHUNK_A_KEY,
        0,
        CONTENT_A.len(),
        CONTENT_A,
    )
    .await;
    assert_eq!(chunk_a.status(), StatusCode::NO_CONTENT);

    let created_b = create_upload_for(
        &router_b,
        WORKSPACE_B,
        CREATE_B_KEY,
        "owner-b.txt",
        CONTENT_B,
    )
    .await;
    assert_eq!(created_b.status(), StatusCode::CREATED);
    let session_b: serde_json::Value =
        serde_json::from_slice(&response_body(created_b).await).expect("upload B session JSON");
    let upload_b_id = session_b["upload_id"]
        .as_str()
        .expect("upload B session has an ID");
    let chunk_b = send_chunk_for(
        &router_b,
        WORKSPACE_B,
        upload_b_id,
        0,
        CHUNK_B_KEY,
        0,
        CONTENT_B.len(),
        CONTENT_B,
    )
    .await;
    assert_eq!(chunk_b.status(), StatusCode::NO_CONTENT);

    let committed_a = commit_upload(&fixture, upload_a_id, SHARED_COMMIT_KEY).await;
    assert_eq!(committed_a.status(), StatusCode::CREATED);
    let response_a: serde_json::Value = serde_json::from_slice(&response_body(committed_a).await)
        .expect("owner A commit response is JSON");
    let resource_a_id = response_a["resource"]["resource_id"]
        .as_str()
        .expect("owner A commit includes its Resource ID");
    assert_eq!(response_a["resource"]["workspace_id"], WORKSPACE_A);

    let committed_b =
        commit_upload_for(&router_b, WORKSPACE_B, upload_b_id, SHARED_COMMIT_KEY).await;
    assert_eq!(
        committed_b.status(),
        StatusCode::CREATED,
        "the same key is independently available to a different principal in this SQLite store"
    );
    let response_b: serde_json::Value = serde_json::from_slice(&response_body(committed_b).await)
        .expect("owner B commit response is JSON");
    let resource_b_id = response_b["resource"]["resource_id"]
        .as_str()
        .expect("owner B commit includes its Resource ID");
    assert_eq!(response_b["resource"]["workspace_id"], WORKSPACE_B);
    assert_ne!(resource_a_id, resource_b_id);

    assert_eq!(
        owned_workspace_commit_counts(
            &fixture,
            WORKSPACE_A,
            OWNER_A,
            upload_a_id,
            SHARED_COMMIT_KEY,
        ),
        OwnedWorkspaceCommitCounts {
            resources: 1,
            resource_events: 1,
            upload_lifecycle_events: 3,
            caller_key_receipts: 1,
        },
        "owner A has one independent commit receipt and Resource in the shared database"
    );
    assert_eq!(
        owned_workspace_commit_counts(
            &fixture,
            WORKSPACE_B,
            OWNER_B,
            upload_b_id,
            SHARED_COMMIT_KEY,
        ),
        OwnedWorkspaceCommitCounts {
            resources: 1,
            resource_events: 1,
            upload_lifecycle_events: 3,
            caller_key_receipts: 1,
        },
        "owner B has a separate receipt namespace and exactly one independent commit"
    );
}
