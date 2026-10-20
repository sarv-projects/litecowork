//! Authenticated, read-only ZIP metadata preview for one exact managed Resource revision.
//!
//! This route has no event, receipt, Invocation, Effect, filesystem destination, or member
//! payload. It is source-mounted and represented in OpenAPI, but it does not establish
//! provider readiness: its worker bridge is available only with the non-default Linux
//! `source-zip-worker` development feature, which the release sidecar build does not enable;
//! extraction, Core child-Resource provenance, and platform qualification remain open.

use super::*;
#[path = "resource_archive_worker.rs"]
mod resource_archive_worker;

use self::resource_archive_worker::{SourceRevision, ZipEntryReport, ZipLimits};

const MAX_ZIP_PREVIEW_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CONCURRENT_ZIP_PREVIEWS: usize = 2;
static ARCHIVE_PREVIEW_ADMISSION: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| {
        std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_ZIP_PREVIEWS))
    });

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/resources/{resource_id}/archive/preview",
            post(preview_resource_archive),
        )
        .route_layer(middleware::from_fn(archive_preview_no_store))
}

async fn archive_preview_no_store(request: Request<Body>, next: Next) -> Response {
    mark_no_store(next.run(request).await)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivePreviewRequest {
    resource_revision_id: String,
    content_digest: String,
}

#[derive(Serialize)]
struct ArchivePreviewResponse {
    resource_ref: storage_core::PinnedResourceRef,
    source_content_digest: String,
    provider_version: String,
    archive_bytes: usize,
    limits: ZipLimits,
    entries: Vec<ZipEntryReport>,
}

async fn preview_resource_archive(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(resource_id): Path<String>,
    Json(body): Json<ArchivePreviewRequest>,
) -> Result<Response, Response> {
    preview_resource_archive_inner(state, headers, resource_id, body)
        .await
        .map(mark_no_store)
        .map_err(mark_no_store)
}

async fn preview_resource_archive_inner(
    state: ApiState,
    headers: HeaderMap,
    resource_id: String,
    body: ArchivePreviewRequest,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    if body.resource_revision_id.is_empty()
        || body.resource_revision_id.len() > 160
        || body.resource_revision_id.contains('\0')
        || !valid_resource_digest(&body.content_digest)
    {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "The pinned Resource revision is invalid",
        ));
    }

    // A single process-wide bound covers both blob reads and isolated workers. Acquire
    // without waiting so repeated local requests cannot accumulate large pending reads.
    let _admission_permit = ARCHIVE_PREVIEW_ADMISSION
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            operator_error(
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "The bounded archive preview workers are busy; retry shortly",
            )
        })?;

    let content = state
        .store
        .read_resource_content_bounded(
            &workspace_id,
            &resource_id,
            Some(&body.resource_revision_id),
            MAX_ZIP_PREVIEW_BYTES,
        )
        .map_err(map_archive_source_error)?
        .ok_or_else(|| {
            operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "The pinned Resource revision is unavailable",
            )
        })?;
    let summary = content.summary;
    if summary.workspace_id != workspace_id
        || summary.resource_id != resource_id
        || summary.resource_revision_id != body.resource_revision_id
        || summary.content_digest != body.content_digest
        || content.content.len() as u64 != summary.size_bytes
        || content.content.len() as u64 > MAX_ZIP_PREVIEW_BYTES
    {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "The pinned Resource revision no longer matches its verified content",
        ));
    }

    let digest = body
        .content_digest
        .strip_prefix("sha256:")
        .expect("validated Resource content digest has a sha256 prefix")
        .to_owned();
    let source = SourceRevision {
        workspace_id: workspace_id.clone(),
        resource_id: resource_id.clone(),
        revision_id: body.resource_revision_id.clone(),
        content_sha256: digest,
    };
    let archive = content.content;
    let manifest =
        tokio::task::spawn_blocking(move || resource_archive_worker::preview(&source, &archive))
            .await
            .map_err(|_| worker_error(resource_archive_worker::ZipWorkerError::Failed))?
            .map_err(worker_error)?;
    let response = ArchivePreviewResponse {
        resource_ref: storage_core::PinnedResourceRef {
            workspace_id,
            resource_id,
            revision_id: body.resource_revision_id,
        },
        source_content_digest: summary.content_digest,
        provider_version: manifest.provider_version,
        archive_bytes: manifest.archive_bytes,
        limits: manifest.limits,
        entries: manifest.entries,
    };
    Ok(Json(response).into_response())
}

fn mark_no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn valid_resource_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn map_archive_source_error(error: StoreError) -> Response {
    match error {
        StoreError::Invalid(message) if message == "RESOURCE_READ_LIMIT_EXCEEDED" => {
            operator_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "INVALID_ARGUMENT",
                "The archive exceeds the bounded preview size",
            )
        }
        StoreError::Invalid(message) if message == "RESOURCE_CONTENT_EXTERNAL" => operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "RESOURCE_CONTENT_EXTERNAL",
            "The pinned Resource revision is not stored by the local managed provider",
        ),
        StoreError::Invalid(message) if message.starts_with("CONTEXT_DOCUMENT_") => operator_error(
            StatusCode::CONFLICT,
            "CONTEXT_DOCUMENT_NOT_ACTIVE",
            "This ContextDocument is not active; its content cannot be previewed",
        ),
        StoreError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "The pinned Resource revision is unavailable",
        ),
        StoreError::Integrity(_) => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "The Resource revision failed its integrity check",
        ),
        _ => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "RESOURCE_LOCATION_UNAVAILABLE",
            "The pinned Resource content is temporarily unavailable",
        ),
    }
}

fn worker_error(error: resource_archive_worker::ZipWorkerError) -> Response {
    match error {
        resource_archive_worker::ZipWorkerError::ArchiveRejected(reason) => {
            operator_error_with_details(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_ARGUMENT",
                "The archive could not be safely previewed",
                Some(json!({ "reason_code": reason })),
            )
        }
        resource_archive_worker::ZipWorkerError::Timeout => operator_error(
            StatusCode::REQUEST_TIMEOUT,
            "TIMEOUT",
            "The bounded archive preview exceeded its processing deadline",
        ),
        resource_archive_worker::ZipWorkerError::OutputLimit => operator_error(
            StatusCode::BAD_GATEWAY,
            "DEPENDENCY_UNAVAILABLE",
            "The isolated archive worker exceeded its response limit",
        ),
        resource_archive_worker::ZipWorkerError::Unavailable => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "The isolated archive preview provider is unavailable on this Runtime",
        ),
        resource_archive_worker::ZipWorkerError::Failed => operator_error(
            StatusCode::BAD_GATEWAY,
            "DEPENDENCY_UNAVAILABLE",
            "The isolated archive preview provider failed",
        ),
    }
}

#[cfg(all(test, unix))]
mod tests {
    #[cfg(not(all(target_os = "linux", feature = "source-zip-worker")))]
    use super::resource_archive_worker;
    use super::{ArchivePreviewRequest, valid_resource_digest, worker_error};
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header::CONTENT_TYPE},
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use sha2::Digest as _;
    use std::{fs, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
    use storage_core::{BlobPurpose, StoreError};
    use storage_sqlite::{WorkspaceBlobKey, WorkspaceBlobKeyProvider};
    use tower::ServiceExt;

    const WORKSPACE: &str = "archive-preview-workspace";
    const OWNER: &str = "archive-preview-owner";
    const RUNTIME: &str = "archive-preview-runtime";
    const NOW: &str = "2026-10-17T12:00:00Z";
    static ARCHIVE_PREVIEW_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[derive(Clone)]
    struct ArchiveTestKeys;

    impl WorkspaceBlobKeyProvider for ArchiveTestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: zeroize::Zeroizing::new([71_u8; 32]),
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
                    "unknown archive test key version".to_owned(),
                ));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    struct Fixture {
        _directory: tempfile::TempDir,
        store: storage_sqlite::SqliteWorkspaceStore,
        router: axum::Router,
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().expect("temporary archive preview directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private test directory");
        let store = storage_sqlite::SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            Arc::new(storage_sqlite::FileBlobStore::new(
                directory.path().join("blobs"),
                ArchiveTestKeys,
            )),
            storage_sqlite::SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("SQLite store opens");
        domain_workspace::WorkspaceService::new(store.clone())
            .create(domain_workspace::CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Archive preview fixture".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: domain_workspace::EventContext {
                    event_id: "archive-preview-workspace-created".to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "archive-preview-workspace-created".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("owner Workspace commits");
        let (_, stream_shutdown) = tokio::sync::watch::channel(false);
        let router = super::super::build_operator_router(super::super::ApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: "archive-preview-incarnation".to_owned(),
            lifecycle_blockers: vec![],
            runtime_identity: Arc::new(
                storage_sqlite::RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                    .expect("test Runtime identity"),
            ),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_shutdown,
        });
        Fixture {
            _directory: directory,
            store,
            router,
        }
    }

    async fn send(
        router: &axum::Router,
        resource_id: &str,
        workspace: &str,
        body: Vec<u8>,
    ) -> axum::response::Response {
        let mut request = Request::post(format!("/v1/resources/{resource_id}/archive/preview"))
            .header("x-workspace-id", workspace)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .expect("archive preview request builds");
        request
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("Operator responds")
    }

    async fn import(fixture: &Fixture, bytes: &[u8]) -> serde_json::Value {
        let mut request = Request::post("/v1/resources/quick-import")
            .header("x-workspace-id", WORKSPACE)
            .header(
                "idempotency-key",
                format!("archive-preview-import-{:x}", sha2::Sha256::digest(bytes)),
            )
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "display_name": "fixture.zip",
                    "media_type": "application/zip",
                    "content_base64": STANDARD.encode(bytes),
                }))
                .expect("serialize Resource import"),
            ))
            .expect("Resource import request builds");
        request
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
        let response = fixture
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("Resource import responds");
        assert_eq!(response.status(), axum::http::StatusCode::CREATED);
        serde_json::from_slice(
            &to_bytes(response.into_body(), 4096)
                .await
                .expect("bounded import response"),
        )
        .expect("Resource import response is JSON")
    }

    fn preview_body(revision_id: &str, digest: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "resource_revision_id": revision_id,
            "content_digest": digest,
        }))
        .expect("serialize preview request")
    }

    async fn assert_no_store(response: &axum::response::Response) {
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .and_then(|v| v.to_str().ok()),
            Some("no-store")
        );
    }

    fn workspace_event_and_owner_receipt_counts(fixture: &Fixture) -> (i64, i64) {
        let connection =
            rusqlite::Connection::open(fixture._directory.path().join("state.sqlite3"))
                .expect("open archive preview fixture for read-only ledger inspection");
        let events = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1",
                [WORKSPACE],
                |row| row.get(0),
            )
            .expect("count Workspace events");
        let receipts = connection
            .query_row(
                "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1",
                [OWNER],
                |row| row.get(0),
            )
            .expect("count owner receipts");
        (events, receipts)
    }

    fn router_for(fixture: &Fixture, principal_id: &str) -> axum::Router {
        let (_, stream_shutdown) = tokio::sync::watch::channel(false);
        super::super::build_operator_router(super::super::ApiState {
            store: fixture.store.clone(),
            principal_id: principal_id.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: format!("archive-preview-incarnation-{principal_id}"),
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

    #[test]
    fn archive_preview_request_is_closed_and_contains_no_locator_or_payload_field() {
        assert!(
            serde_json::from_value::<ArchivePreviewRequest>(serde_json::json!({
                "resource_revision_id": "revision-1",
                "content_digest": format!("sha256:{}", "a".repeat(64)),
            }))
            .is_ok()
        );
        assert!(
            serde_json::from_value::<ArchivePreviewRequest>(serde_json::json!({
                "resource_revision_id": "revision-1",
                "content_digest": format!("sha256:{}", "a".repeat(64)),
                "path": "/etc/passwd",
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ArchivePreviewRequest>(serde_json::json!({
                "resource_revision_id": "revision-1",
                "content_digest": format!("sha256:{}", "a".repeat(64)),
                "archive_bytes": "not accepted",
            }))
            .is_err()
        );
    }

    #[test]
    fn archive_preview_requires_exact_lowercase_sha256_resource_digest() {
        assert!(valid_resource_digest(&format!("sha256:{}", "a".repeat(64))));
        assert!(!valid_resource_digest(&format!(
            "sha256:{}",
            "A".repeat(64)
        )));
        assert!(!valid_resource_digest("sha256:abc"));
        assert!(!valid_resource_digest(&format!(
            "sha512:{}",
            "a".repeat(64)
        )));
    }

    #[tokio::test]
    async fn archive_preview_endpoint_enforces_owner_scope_exact_pin_and_no_store_errors() {
        let _test_guard = ARCHIVE_PREVIEW_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = fixture();
        let archive = STANDARD.decode("UEsDBBQAAAAAAGNqUV2GphA2BQAAAAUAAAAJAAAAaGVsbG8udHh0aGVsbG9QSwECFAMUAAAAAABjalFdhqYQNgUAAAAFAAAACQAAAAAAAAAAAAAAgAEAAAAAaGVsbG8udHh0UEsFBgAAAAABAAEANwAAACwAAAAAAA==").expect("valid tiny ZIP fixture");
        let imported = import(&fixture, &archive).await;
        let resource_id = imported["resourceId"].as_str().unwrap();
        let revision_id = imported["resourceRevisionId"].as_str().unwrap();
        let digest = imported["contentDigest"].as_str().unwrap();

        // The same owner has a second Workspace. A Resource from the first Workspace
        // must remain undiscoverable when the selected Workspace changes.
        domain_workspace::WorkspaceService::new(fixture.store.clone())
            .create(domain_workspace::CreateWorkspace {
                workspace_id: "archive-preview-other-workspace".to_owned(),
                name: "Other owner Workspace".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: domain_workspace::EventContext {
                    event_id: "archive-preview-other-workspace-created".to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "archive-preview-other-workspace-created".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("second owner Workspace commits");
        let cross_workspace = send(
            &fixture.router,
            resource_id,
            "archive-preview-other-workspace",
            preview_body(revision_id, digest),
        )
        .await;
        assert_eq!(cross_workspace.status(), axum::http::StatusCode::NOT_FOUND);
        assert_no_store(&cross_workspace).await;

        let non_owner = router_for(&fixture, "not-the-owner");
        let denied = send(
            &non_owner,
            resource_id,
            WORKSPACE,
            preview_body(revision_id, digest),
        )
        .await;
        assert_eq!(denied.status(), axum::http::StatusCode::FORBIDDEN);
        assert_no_store(&denied).await;

        let missing_revision = send(
            &fixture.router,
            resource_id,
            WORKSPACE,
            preview_body("stale-revision", digest),
        )
        .await;
        assert_eq!(missing_revision.status(), axum::http::StatusCode::NOT_FOUND);
        assert_no_store(&missing_revision).await;

        let wrong_digest = format!("sha256:{}", "0".repeat(64));
        assert_ne!(wrong_digest, digest);
        let mismatch = send(
            &fixture.router,
            resource_id,
            WORKSPACE,
            preview_body(revision_id, &wrong_digest),
        )
        .await;
        assert_eq!(mismatch.status(), axum::http::StatusCode::CONFLICT);
        assert_no_store(&mismatch).await;

        let response = send(
            &fixture.router,
            &format!("{resource_id}-not-present"),
            WORKSPACE,
            preview_body(revision_id, digest),
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        assert_no_store(&response).await;

        // Worker availability is checked only after an authorized exact Resource
        // revision has been loaded. Exercise that boundary with the valid import;
        // an unknown Resource remains a 404 regardless of worker build features.
        let worker_response = send(
            &fixture.router,
            resource_id,
            WORKSPACE,
            preview_body(revision_id, digest),
        )
        .await;
        assert_no_store(&worker_response).await;
        if cfg!(all(target_os = "linux", feature = "source-zip-worker")) {
            assert_eq!(worker_response.status(), axum::http::StatusCode::OK);
            let preview: serde_json::Value = serde_json::from_slice(
                &to_bytes(worker_response.into_body(), 32 * 1024)
                    .await
                    .expect("bounded preview response"),
            )
            .expect("preview response is JSON");
            assert_eq!(preview["resource_ref"]["workspace_id"], WORKSPACE);
            assert_eq!(preview["resource_ref"]["resource_id"], resource_id);
            assert_eq!(preview["resource_ref"]["revision_id"], revision_id);
            assert_eq!(preview["source_content_digest"], digest);
            assert_eq!(preview["entries"][0]["virtual_path"], "hello.txt");
            assert!(preview.get("payloads").is_none());
        } else {
            assert_eq!(
                worker_response.status(),
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            );
        }

        let invalid_bytes = b"not a ZIP archive";
        let invalid = import(&fixture, invalid_bytes).await;
        let rejected = send(
            &fixture.router,
            invalid["resourceId"].as_str().unwrap(),
            WORKSPACE,
            preview_body(
                invalid["resourceRevisionId"].as_str().unwrap(),
                invalid["contentDigest"].as_str().unwrap(),
            ),
        )
        .await;
        assert_eq!(
            rejected.status(),
            if cfg!(all(target_os = "linux", feature = "source-zip-worker")) {
                axum::http::StatusCode::UNPROCESSABLE_ENTITY
            } else {
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            }
        );
        assert_no_store(&rejected).await;

        let malformed = send(&fixture.router, resource_id, WORKSPACE, b"{".to_vec()).await;
        assert_eq!(malformed.status(), axum::http::StatusCode::BAD_REQUEST);
        assert_no_store(&malformed).await;
    }

    #[tokio::test]
    async fn archive_preview_when_both_worker_slots_are_busy_returns_rate_limited_without_writes() {
        let _test_guard = ARCHIVE_PREVIEW_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = fixture();
        let imported = import(&fixture, b"a valid source object").await;
        let resource_id = imported["resourceId"].as_str().unwrap();
        let revision_id = imported["resourceRevisionId"].as_str().unwrap();
        let digest = imported["contentDigest"].as_str().unwrap();
        let ledger_before = workspace_event_and_owner_receipt_counts(&fixture);

        let _first_slot = super::ARCHIVE_PREVIEW_ADMISSION
            .clone()
            .try_acquire_owned()
            .expect("first preview worker slot is available");
        let _second_slot = super::ARCHIVE_PREVIEW_ADMISSION
            .clone()
            .try_acquire_owned()
            .expect("second preview worker slot is available");
        let non_owner = router_for(&fixture, "archive-preview-non-owner");
        let denied = send(
            &non_owner,
            resource_id,
            WORKSPACE,
            preview_body(revision_id, digest),
        )
        .await;
        assert_eq!(denied.status(), axum::http::StatusCode::FORBIDDEN);
        assert_no_store(&denied).await;

        let missing_resource_id = format!("{resource_id}-not-present");
        let response = send(
            &fixture.router,
            &missing_resource_id,
            WORKSPACE,
            preview_body(revision_id, digest),
        )
        .await;

        assert_eq!(response.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
        assert_no_store(&response).await;
        let body: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), 8192)
                .await
                .expect("bounded rate-limit response"),
        )
        .expect("rate-limit response is JSON");
        assert_eq!(body["code"], "RATE_LIMITED");
        assert_eq!(
            workspace_event_and_owner_receipt_counts(&fixture),
            ledger_before,
            "saturated preview creates no domain event or idempotency receipt"
        );
        assert_eq!(
            fixture
                .store
                .list_resource_revisions_page(WORKSPACE, resource_id, None, 10)
                .expect("Resource remains readable")
                .len(),
            1,
            "saturated preview does not create a child Resource revision"
        );
    }

    #[tokio::test]
    async fn archive_preview_rejects_managed_content_above_sixteen_mib_with_no_store() {
        let _test_guard = ARCHIVE_PREVIEW_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = fixture();
        let bytes = vec![0_u8; 16 * 1024 * 1024 + 1];
        let digest = format!("sha256:{:x}", sha2::Sha256::digest(&bytes));
        let mut create = Request::post("/v1/resources/uploads")
            .header("x-workspace-id", WORKSPACE)
            .header("idempotency-key", "archive-preview-oversize-create")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "workspace_id": WORKSPACE,
                    "display_name": "oversize.zip",
                    "media_type": "application/zip",
                    "size_bytes": bytes.len(),
                    "expected_digest": digest,
                    "context_document": null,
                    "folder_import": null,
                }))
                .expect("serialize upload session"),
            ))
            .expect("upload session request builds");
        create
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
        let created = fixture
            .router
            .clone()
            .oneshot(create)
            .await
            .expect("upload session responds");
        assert_eq!(created.status(), axum::http::StatusCode::CREATED);
        let created: serde_json::Value = serde_json::from_slice(
            &to_bytes(created.into_body(), 8192)
                .await
                .expect("bounded upload session response"),
        )
        .expect("upload session is JSON");
        let upload_id = created["upload_id"].as_str().unwrap();

        let chunk_size = 4 * 1024 * 1024;
        for (index, chunk) in bytes.chunks(chunk_size).enumerate() {
            let start = index * chunk_size;
            let end = start + chunk.len() - 1;
            let request = Request::put(format!("/v1/resources/uploads/{upload_id}/chunks/{index}"))
                .header("x-workspace-id", WORKSPACE)
                .header(
                    "idempotency-key",
                    format!("archive-preview-oversize-{index}"),
                )
                .header(
                    "content-range",
                    format!("bytes {start}-{end}/{}", bytes.len()),
                )
                .header(
                    "x-chunk-sha256",
                    format!("{:x}", sha2::Sha256::digest(chunk)),
                )
                .body(Body::from(chunk.to_vec()))
                .expect("upload chunk request builds");
            let mut request = request;
            request
                .extensions_mut()
                .insert(super::super::AuthenticatedLocalPeer);
            let response = fixture
                .router
                .clone()
                .oneshot(request)
                .await
                .expect("upload chunk responds");
            assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
        }

        let mut commit = Request::post(format!("/v1/resources/uploads/{upload_id}/commit"))
            .header("x-workspace-id", WORKSPACE)
            .header("idempotency-key", "archive-preview-oversize-commit")
            .body(Body::empty())
            .expect("upload commit request builds");
        commit
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
        let committed = fixture
            .router
            .clone()
            .oneshot(commit)
            .await
            .expect("upload commit responds");
        assert_eq!(committed.status(), axum::http::StatusCode::CREATED);
        let resource: serde_json::Value = serde_json::from_slice(
            &to_bytes(committed.into_body(), 8192)
                .await
                .expect("bounded Resource response"),
        )
        .expect("Resource response is JSON");

        let response = send(
            &fixture.router,
            resource["resource"]["resource_id"].as_str().unwrap(),
            WORKSPACE,
            preview_body(
                resource["revision"]["resource_revision_id"]
                    .as_str()
                    .unwrap(),
                &digest,
            ),
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::PAYLOAD_TOO_LARGE);
        assert_no_store(&response).await;
    }

    #[test]
    fn unavailable_worker_maps_to_service_unavailable() {
        let response = worker_error(super::resource_archive_worker::ZipWorkerError::Unavailable);
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[cfg(not(all(target_os = "linux", feature = "source-zip-worker")))]
    #[test]
    fn source_worker_is_unavailable_without_explicit_development_feature() {
        let source = resource_archive_worker::SourceRevision {
            workspace_id: WORKSPACE.to_owned(),
            resource_id: "resource-1".to_owned(),
            revision_id: "revision-1".to_owned(),
            content_sha256: "a".repeat(64),
        };
        assert_eq!(
            resource_archive_worker::preview(&source, b"not read or executed"),
            Err(resource_archive_worker::ZipWorkerError::Unavailable),
        );
    }
}

#[cfg(all(test, not(unix)))]
mod non_unix_tests {
    #[test]
    fn unavailable_worker_maps_to_service_unavailable() {
        let response =
            super::worker_error(super::resource_archive_worker::ZipWorkerError::Unavailable);
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[cfg(not(all(target_os = "linux", feature = "source-zip-worker")))]
    #[test]
    fn source_worker_is_unavailable_without_explicit_development_feature() {
        let source = super::resource_archive_worker::SourceRevision {
            workspace_id: "workspace-1".to_owned(),
            resource_id: "resource-1".to_owned(),
            revision_id: "revision-1".to_owned(),
            content_sha256: "a".repeat(64),
        };
        assert_eq!(
            super::resource_archive_worker::preview(&source, b"not read or executed"),
            Err(super::resource_archive_worker::ZipWorkerError::Unavailable),
        );
    }
}
