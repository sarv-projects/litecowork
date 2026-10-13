use super::*;
use rusqlite::{Connection, params};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use storage_core::{
    BlobStore, ResourceRecord, ResourceRevisionRecord, ResourceStore,
    ResourceTextIndexRebuildOutcome, ResourceTextIndexRebuildRequest, ResourceUploadChunkInput,
    ResourceUploadContentRange, ResourceUploadSessionRecord, ResourceUploadState,
    ResourceUploadStore, WorkspaceCreateRequest,
};

const WORKSPACE: &str = "workspace-upload-gc";
const OWNER: &str = "owner-upload-gc";
const CREATED: &str = "2026-10-12T10:00:00Z";
const EXPIRES: &str = "2026-10-12T10:01:00Z";
const AFTER_EXPIRY: &str = "2026-10-12T10:02:00Z";
const CHUNK: &[u8] = b"durable upload chunk";

#[derive(Clone)]
struct ChunkPutPause {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    first_only: Arc<AtomicBool>,
}

#[derive(Clone)]
struct GcRemovePause {
    removed: Arc<Barrier>,
    release: Arc<Barrier>,
    first_only: Arc<AtomicBool>,
}

struct TestUploadBlobStore {
    inner: FileBlobStore<TestKeys>,
    chunk_put_pause: Option<ChunkPutPause>,
    gc_remove_pause: Option<GcRemovePause>,
    fail_next_remove: Arc<AtomicBool>,
}

impl BlobStore for TestUploadBlobStore {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
        if purpose == BlobPurpose::ResourceUploadChunk
            && let Some(pause) = &self.chunk_put_pause
            && pause.first_only.swap(false, Ordering::SeqCst)
        {
            pause.entered.wait();
            pause.release.wait();
        }
        self.inner.put(workspace_id, purpose, bytes, media_type)
    }

    fn get(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<Vec<u8>, StoreError> {
        self.inner.get(workspace_id, purpose, blob)
    }

    fn remove(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<(), StoreError> {
        if self.fail_next_remove.swap(false, Ordering::SeqCst) {
            return Err(StoreError::Blob("injected blob removal failure".to_owned()));
        }
        self.inner.remove(workspace_id, purpose, blob)?;
        if purpose == BlobPurpose::ResourceUploadChunk
            && let Some(pause) = &self.gc_remove_pause
            && pause.first_only.swap(false, Ordering::SeqCst)
        {
            pause.removed.wait();
            pause.release.wait();
        }
        Ok(())
    }

    fn resource_index_token(
        &self,
        workspace_id: &str,
        key_version: Option<u32>,
        normalized_term: &str,
    ) -> Result<(u32, String), StoreError> {
        self.inner
            .resource_index_token(workspace_id, key_version, normalized_term)
    }

    fn resource_index_tokens(
        &self,
        workspace_id: &str,
        key_version: Option<u32>,
        normalized_terms: &[String],
    ) -> Result<(u32, Vec<String>), StoreError> {
        self.inner
            .resource_index_tokens(workspace_id, key_version, normalized_terms)
    }
}

fn open_store(directory: &tempfile::TempDir, blobs: Arc<dyn BlobStore>) -> SqliteWorkspaceStore {
    SqliteWorkspaceStore::open(
        state_database(directory),
        blobs,
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("SQLite store opens")
}

fn file_blob_store(directory: &tempfile::TempDir) -> Arc<dyn BlobStore> {
    Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [23_u8; 32] },
    ))
}

struct MissingIndexKeys;

impl crate::blob::WorkspaceBlobKeyProvider for MissingIndexKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<crate::blob::WorkspaceBlobKey, StoreError> {
        Err(StoreError::Blob(
            "test Resource index key unavailable".to_owned(),
        ))
    }

    fn key_by_version(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
        _version: u32,
    ) -> Result<crate::blob::WorkspaceBlobKey, StoreError> {
        Err(StoreError::Blob(
            "test Resource index key unavailable".to_owned(),
        ))
    }
}

#[test]
fn uploaded_text_index_rebuild_search_and_restart_preserve_exact_pins() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);

    let upload_id = "upload-index-roundtrip";
    let session = create_upload_with_context_document(
        &store,
        upload_id,
        Some(json!({
            "kind": "WORKSPACE_NOTES",
            "owner_ref": {"kind": "WORKSPACE", "workspace_id": WORKSPACE},
            "status": "ACTIVE",
        })),
    );
    let received = store
        .put_chunk(chunk_input(upload_id, "request-index-chunk"))
        .expect("production chunk path accepts uploaded text");
    assert_eq!(received.state, ResourceUploadState::ContentReceived);

    let resource_id = "resource-index-roundtrip";
    let revision_id = "revision-index-roundtrip";
    let revision_at = "2026-10-12T10:00:03Z";
    let content_digest = digest(CHUNK);
    let resource = ResourceRecord {
        resource_id: resource_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        kind: "FILE".to_owned(),
        provider_identity: json!({}),
        identity_digest: None,
        display_name: session.display_name.clone(),
        current_revision_id: Some(revision_id.to_owned()),
        sensitivity: "PERSONAL".to_owned(),
        provenance: json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
        created_at: revision_at.to_owned(),
        updated_at: revision_at.to_owned(),
        version: 1,
    };
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id.to_owned(),
        resource_id: resource_id.to_owned(),
        parent_revision_ids: Vec::new(),
        provider_revision: None,
        content_digest: Some(content_digest.clone()),
        size_bytes: Some(CHUNK.len() as u64),
        media_type: Some("text/plain".to_owned()),
        observed_at: revision_at.to_owned(),
        created_by: json!({"kind": "USER", "principal_id": OWNER}),
    };
    let resource_event = EventDraft {
        event_id: "event-resource-index-roundtrip".to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        entity_type: "Resource".to_owned(),
        entity_id: resource_id.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: 1,
        hlc_timestamp: revision_at.to_owned(),
        correlation_id: "correlation-index-roundtrip".to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "resource.created.v1".to_owned(),
        payload: json!({
            "resource_id": resource_id,
            "workspace_id": WORKSPACE,
            "kind": "FILE",
            "provenance": resource.provenance,
            "aggregate_version": 1,
        }),
        recorded_at: revision_at.to_owned(),
    };
    let commit_event = EventDraft {
        event_id: "event-upload-commit-index-roundtrip".to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        entity_type: "ResourceUpload".to_owned(),
        entity_id: upload_id.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: 3,
        hlc_timestamp: revision_at.to_owned(),
        correlation_id: "correlation-index-roundtrip".to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "resource.upload.status.changed.v1".to_owned(),
        payload: json!({
            "upload_id": upload_id,
            "from": "CONTENT_RECEIVED",
            "to": "COMMITTED",
            "resource_id": resource_id,
            "aggregate_version": 3,
        }),
        recorded_at: revision_at.to_owned(),
    };
    let committed = store
        .commit(
            WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "unused-upload-commit-request".to_owned(),
                request_payload: json!({}),
            },
            upload_id,
            resource,
            revision,
            resource_event,
            Some(commit_event),
            None,
        )
        .expect("production upload commit materializes a managed Resource");
    assert_eq!(committed.session.state, ResourceUploadState::Committed);
    assert_eq!(committed.resource.resource_id, resource_id);

    let initial = store
        .search_indexed_resource_text(WORKSPACE, "upload chunk", None, None, None, None, 20)
        .expect("initial indexed search succeeds");
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].resource_revision_id, revision_id);
    assert_eq!(initial[0].source_content_digest, content_digest);

    let connection =
        Connection::open(state_database(&directory)).expect("open derived index fixture");
    connection
        .execute(
            "DELETE FROM resource_text_index_terms WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource_id],
        )
        .expect("simulate loss of rebuildable term projection");
    connection
        .execute(
            "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource_id],
        )
        .expect("simulate loss of rebuildable index metadata");
    drop(connection);

    assert!(
        store
            .search_indexed_resource_text(WORKSPACE, "upload chunk", None, None, None, None, 20)
            .expect("search without derived index stays bounded")
            .is_empty()
    );
    let rebuilt = store
        .reindex_resource_text(ResourceTextIndexRebuildRequest {
            principal_id: OWNER.to_owned(),
            request_id: "request-rebuild-index-roundtrip".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            resource_id: resource_id.to_owned(),
            resource_revision_id: revision_id.to_owned(),
            content_digest: content_digest.clone(),
            indexed_at: "2026-10-12T10:01:00Z".to_owned(),
            correlation_id: "correlation-index-roundtrip".to_owned(),
        })
        .expect("production exact-pin rebuild succeeds");
    assert_eq!(rebuilt.outcome, ResourceTextIndexRebuildOutcome::Indexed);
    let replay = store
        .reindex_resource_text(ResourceTextIndexRebuildRequest {
            principal_id: OWNER.to_owned(),
            request_id: "request-rebuild-index-roundtrip".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            resource_id: resource_id.to_owned(),
            resource_revision_id: revision_id.to_owned(),
            content_digest: content_digest.clone(),
            indexed_at: "2026-10-12T10:02:00Z".to_owned(),
            correlation_id: "correlation-index-roundtrip".to_owned(),
        })
        .expect("exact rebuild retry returns the committed outcome");
    assert_eq!(replay, rebuilt);

    let database = Connection::open(state_database(&directory))
        .expect("inspect encrypted text-index projection");
    let plaintext_terms: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM resource_text_index_terms WHERE workspace_id = ?1 AND resource_id = ?2 AND term_token IN ('upload', 'chunk')",
            params![WORKSPACE, resource_id],
            |row| row.get(0),
        )
        .expect("inspect keyed term tokens");
    assert_eq!(plaintext_terms, 0, "SQLite must not store source terms");
    drop(database);

    let before_restart = store
        .search_indexed_resource_text(WORKSPACE, "upload chunk", None, None, None, None, 20)
        .expect("rebuilt index is searchable");
    assert_eq!(before_restart.len(), 1);
    assert_eq!(before_restart[0].snippet, "durable upload chunk");
    assert_eq!(before_restart[0].matched_term_count, 2);
    assert_eq!(
        before_restart[0].matched_spans,
        vec![
            storage_core::ResourceTextMatchSpan {
                term: "chunk".to_owned(),
                start_utf8_byte: 15,
                end_utf8_byte_exclusive: 20,
            },
            storage_core::ResourceTextMatchSpan {
                term: "upload".to_owned(),
                start_utf8_byte: 8,
                end_utf8_byte_exclusive: 14,
            },
        ]
    );

    drop(store);
    let reopened = open_store(&directory, file_blob_store(&directory));
    let after_restart = reopened
        .search_indexed_resource_text(WORKSPACE, "upload chunk", None, None, None, None, 20)
        .expect("encrypted index survives SQLite and BlobStore reopen");
    assert_eq!(after_restart, before_restart);
    assert!(
        reopened
            .search_indexed_resource_text(
                "workspace-index-other",
                "upload chunk",
                None,
                None,
                None,
                None,
                20,
            )
            .expect("search remains scoped to the requested Workspace")
            .is_empty()
    );

    drop(reopened);
    let unavailable_key_store = open_store(
        &directory,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            MissingIndexKeys,
        )),
    );
    assert!(matches!(
        unavailable_key_store.search_indexed_resource_text(
            WORKSPACE,
            "upload chunk",
            None,
            None,
            None,
            None,
            20,
        ),
        Err(StoreError::Blob(_))
    ));
    drop(unavailable_key_store);

    let reopened = open_store(&directory, file_blob_store(&directory));

    ResourceService::new(reopened.clone())
        .set_context_document_status(SetContextDocumentStatus {
            workspace_id: WORKSPACE.to_owned(),
            resource_id: resource_id.to_owned(),
            principal_id: OWNER.to_owned(),
            request_id: "request-revoke-index-roundtrip".to_owned(),
            expected_version: 1,
            target_status: ContextDocumentOwnerStatus::Revoked,
            event: EventContext {
                event_id: "event-revoke-index-roundtrip".to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: "2026-10-12T10:04:00Z".to_owned(),
                correlation_id: "correlation-index-roundtrip".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-12T10:04:00Z".to_owned(),
            },
        })
        .expect("owner revocation commits against the exact Resource");
    assert!(
        reopened
            .search_indexed_resource_text(WORKSPACE, "upload chunk", None, None, None, None, 20)
            .expect("revocation excludes indexed content")
            .is_empty()
    );
    assert_eq!(
        reopened
            .reindex_resource_text(ResourceTextIndexRebuildRequest {
                principal_id: OWNER.to_owned(),
                request_id: "request-rebuild-after-revoke".to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                resource_id: resource_id.to_owned(),
                resource_revision_id: revision_id.to_owned(),
                content_digest,
                indexed_at: "2026-10-12T10:03:00Z".to_owned(),
                correlation_id: "correlation-index-roundtrip".to_owned(),
            })
            .expect_err("revoked Resource cannot be reindexed"),
        StoreError::Invalid("CONTEXT_DOCUMENT_REVOKED".to_owned())
    );
}

fn paused_blob_store(
    directory: &tempfile::TempDir,
    pause: Option<ChunkPutPause>,
    gc_remove_pause: Option<GcRemovePause>,
    fail_next_remove: bool,
) -> Arc<dyn BlobStore> {
    Arc::new(TestUploadBlobStore {
        inner: FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys { key: [23_u8; 32] },
        ),
        chunk_put_pause: pause,
        gc_remove_pause,
        fail_next_remove: Arc::new(AtomicBool::new(fail_next_remove)),
    })
}

fn create_upload(store: &SqliteWorkspaceStore, upload_id: &str) -> ResourceUploadSessionRecord {
    create_upload_with_context_document(store, upload_id, None)
}

fn create_upload_with_context_document(
    store: &SqliteWorkspaceStore,
    upload_id: &str,
    context_document: Option<serde_json::Value>,
) -> ResourceUploadSessionRecord {
    let digest = digest(CHUNK);
    let session = ResourceUploadSessionRecord {
        upload_id: upload_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        display_name: format!("{upload_id}.txt"),
        media_type: "text/plain".to_owned(),
        expected_size_bytes: CHUNK.len() as u64,
        expected_digest: Some(digest.clone()),
        context_document,
        folder_import: None,
        resource_id: None,
        committed_resource_id: None,
        expected_resource_version: None,
        parent_revision_ids: Vec::new(),
        chunk_size_bytes: 4_194_304,
        received_ranges: Vec::new(),
        next_missing_offset: 0,
        state: ResourceUploadState::Open,
        expires_at: EXPIRES.to_owned(),
        created_at: CREATED.to_owned(),
        version: 1,
        progress_version: 1,
    };
    let context = context(&format!("event-create-{upload_id}"), 1);
    store
        .create(
            WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: format!("request-create-{upload_id}"),
                request_payload: json!({
                    "operation": "resource.upload.create.v1",
                    "workspace_id": WORKSPACE,
                    "upload_id": upload_id,
                    "expected_digest": digest,
                }),
            },
            session.clone(),
            EventDraft {
                event_id: context.event_id,
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "ResourceUpload".to_owned(),
                entity_id: upload_id.to_owned(),
                origin_runtime_id: context.origin_runtime_id,
                entity_revision: 1,
                hlc_timestamp: context.hlc_timestamp,
                correlation_id: context.correlation_id,
                causation_id: None,
                schema_version: 1,
                event_type: "resource.upload.created.v1".to_owned(),
                payload: json!({
                    "upload_id": upload_id,
                    "workspace_id": WORKSPACE,
                    "expected_size_bytes": CHUNK.len(),
                    "expected_digest": digest,
                    "chunk_size_bytes": 4_194_304,
                    "expires_at": EXPIRES,
                    "resource_id": null,
                    "expected_resource_version": null,
                    "parent_revision_ids": [],
                    "aggregate_version": 1,
                }),
                recorded_at: CREATED.to_owned(),
            },
        )
        .expect("upload session commits")
}

fn create_workspace_for_uploads(store: &SqliteWorkspaceStore) {
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Upload GC Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: context("event-upload-gc-workspace", 0),
        })
        .expect("Workspace commits");
}

fn reserve_chunk(
    directory: &tempfile::TempDir,
    upload_id: &str,
    request_id: &str,
    expires_at: &str,
) {
    let mut connection = Connection::open(state_database(directory)).expect("open SQLite");
    reserve_resource_upload_blob_transaction(
        &mut connection,
        WORKSPACE,
        upload_id,
        request_id,
        0,
        &digest(CHUNK),
        CHUNK.len() as u64,
        CREATED,
        expires_at,
    )
    .expect("upload blob reservation commits");
}

fn write_chunk_blob(store: &SqliteWorkspaceStore) -> BlobRef {
    store
        .inner
        .blobs
        .put(
            WORKSPACE,
            BlobPurpose::ResourceUploadChunk,
            CHUNK,
            "application/octet-stream",
        )
        .expect("encrypted upload chunk blob writes")
}

fn chunk_input(upload_id: &str, request_id: &str) -> ResourceUploadChunkInput {
    let context = context(&format!("event-chunk-{upload_id}"), 2);
    ResourceUploadChunkInput {
        upload_id: upload_id.to_owned(),
        chunk_index: 0,
        request_id: request_id.to_owned(),
        content_range: ResourceUploadContentRange {
            start_offset: 0,
            end_offset_inclusive: CHUNK.len() as u64 - 1,
            total_size_bytes: CHUNK.len() as u64,
        },
        sha256: digest(CHUNK),
        content: CHUNK.to_vec(),
        received_at: CREATED.to_owned(),
        lifecycle_event: EventDraft {
            event_id: context.event_id,
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: upload_id.to_owned(),
            origin_runtime_id: context.origin_runtime_id,
            entity_revision: 2,
            hlc_timestamp: context.hlc_timestamp,
            correlation_id: context.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.status.changed.v1".to_owned(),
            payload: json!({}),
            recorded_at: CREATED.to_owned(),
        },
    }
}

fn blob_ref() -> BlobRef {
    BlobRef {
        digest: digest(CHUNK),
        size_bytes: CHUNK.len() as u64,
        media_type: "application/octet-stream".to_owned(),
    }
}

fn reservation_and_fence_counts(directory: &tempfile::TempDir) -> (i64, i64, Option<String>) {
    let connection = Connection::open(state_database(directory)).expect("inspect SQLite");
    let reservation_count = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_blob_reservations WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, digest(CHUNK)],
            |row| row.get(0),
        )
        .expect("count reservations");
    let fence_count = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_blob_gc_fences WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, digest(CHUNK)],
            |row| row.get(0),
        )
        .expect("count GC fences");
    let state = connection
        .query_row(
            "SELECT state FROM resource_upload_blob_reservations WHERE workspace_id = ?1 AND digest = ?2 LIMIT 1",
            params![WORKSPACE, digest(CHUNK)],
            |row| row.get(0),
        )
        .optional()
        .expect("read reservation state");
    (reservation_count, fence_count, state)
}

fn assert_blob_missing(store: &SqliteWorkspaceStore) {
    assert!(matches!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &blob_ref()),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn upload_blob_gc_serializes_expired_collection_against_inflight_put() {
    let directory = tempfile::tempdir().expect("temporary upload GC directory");
    let pause = ChunkPutPause {
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let gc_remove_pause = GcRemovePause {
        removed: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let store = open_store(
        &directory,
        paused_blob_store(
            &directory,
            Some(pause.clone()),
            Some(gc_remove_pause.clone()),
            false,
        ),
    );
    create_workspace_for_uploads(&store);
    create_upload(&store, "upload-inflight");

    let writer_store = store.clone();
    let writer = std::thread::spawn(move || {
        writer_store.put_chunk(chunk_input("upload-inflight", "request-inflight"))
    });
    pause.entered.wait();

    let gc_store = store.clone();
    let collector = std::thread::spawn(move || gc_store.collect_orphan_chunks(AFTER_EXPIRY, 100));
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while reservation_and_fence_counts(&directory).1 == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "GC did not persist its fence"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    pause.release.wait();
    assert!(matches!(
        writer.join().expect("chunk writer joins"),
        Err(StoreError::Busy)
    ));
    gc_remove_pause.removed.wait();
    gc_remove_pause.release.wait();
    assert_eq!(
        collector
            .join()
            .expect("collector joins")
            .expect("GC succeeds"),
        1
    );
    assert_blob_missing(&store);
    assert_eq!(reservation_and_fence_counts(&directory), (0, 0, None));
}

#[test]
fn upload_blob_gc_retries_deleting_fence_after_restart_and_remove_failure() {
    let directory = tempfile::tempdir().expect("temporary upload GC directory");
    let store = open_store(&directory, paused_blob_store(&directory, None, None, true));
    create_workspace_for_uploads(&store);
    create_upload(&store, "upload-retry-delete");
    reserve_chunk(&directory, "upload-retry-delete", "request-orphan", EXPIRES);
    let blob = write_chunk_blob(&store);

    assert!(matches!(
        store.collect_orphan_chunks(AFTER_EXPIRY, 100),
        Err(StoreError::Blob(_))
    ));
    assert_eq!(
        reservation_and_fence_counts(&directory),
        (1, 1, Some("DELETING".to_owned()))
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &blob)
            .expect("failed removal leaves object intact"),
        CHUNK
    );

    drop(store);
    let restarted = open_store(&directory, file_blob_store(&directory));
    assert_eq!(
        restarted
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("restart retries deletion"),
        1
    );
    assert_blob_missing(&restarted);
    assert_eq!(reservation_and_fence_counts(&directory), (0, 0, None));
    assert_eq!(
        restarted
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("empty retry is safe"),
        0
    );
}

#[test]
fn upload_blob_gc_releases_persisted_fence_after_restart_when_object_is_missing() {
    let directory = tempfile::tempdir().expect("temporary upload GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_upload(&store, "upload-delete-before-crash");
    create_upload(&store, "upload-blocked-by-fence");
    reserve_chunk(
        &directory,
        "upload-delete-before-crash",
        "request-delete-before-crash",
        EXPIRES,
    );
    let blob = write_chunk_blob(&store);

    let claimed = {
        let mut connection = Connection::open(state_database(&directory)).expect("open SQLite");
        claim_orphan_resource_upload_blobs_transaction(&mut connection, AFTER_EXPIRY, 100)
            .expect("GC persists its durable deletion fence")
    };
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].0, WORKSPACE);
    assert_eq!(claimed[0].1, blob);
    assert_eq!(
        reservation_and_fence_counts(&directory),
        (1, 1, Some("DELETING".to_owned()))
    );
    assert!(matches!(
        store.put_chunk(chunk_input(
            "upload-blocked-by-fence",
            "request-blocked-by-fence"
        )),
        Err(StoreError::Busy)
    ));
    assert_eq!(
        reservation_and_fence_counts(&directory),
        (1, 1, Some("DELETING".to_owned()))
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &blob)
            .expect("new reservation is denied without deleting the original object"),
        CHUNK
    );

    store
        .inner
        .blobs
        .remove(WORKSPACE, BlobPurpose::ResourceUploadChunk, &blob)
        .expect("physical deletion completes before simulated crash");
    drop(store);

    let restarted = open_store(&directory, file_blob_store(&directory));
    assert_eq!(
        restarted
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("restart treats missing object as an idempotent deletion"),
        1
    );
    assert_blob_missing(&restarted);
    assert_eq!(reservation_and_fence_counts(&directory), (0, 0, None));
}

#[test]
fn upload_blob_gc_retains_digest_with_an_accepted_chunk_reference() {
    let directory = tempfile::tempdir().expect("temporary upload GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_upload(&store, "upload-shared-accepted");
    create_upload(&store, "upload-shared-orphan");

    store
        .put_chunk(chunk_input("upload-shared-accepted", "request-accepted"))
        .expect("first upload accepts the shared content-addressed chunk");
    reserve_chunk(
        &directory,
        "upload-shared-orphan",
        "request-orphan-shared",
        EXPIRES,
    );
    assert_eq!(
        store
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("GC scans candidates"),
        0
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &blob_ref())
            .expect("accepted shared object remains available"),
        CHUNK
    );
    assert_eq!(
        reservation_and_fence_counts(&directory),
        (1, 0, Some("RESERVED".to_owned()))
    );
}

#[test]
fn upload_blob_gc_reclaims_crash_before_blob_write_after_restart() {
    let directory = tempfile::tempdir().expect("temporary upload GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_upload(&store, "upload-crash-before-put");
    reserve_chunk(
        &directory,
        "upload-crash-before-put",
        "request-before-put",
        EXPIRES,
    );
    drop(store);

    let restarted = open_store(&directory, file_blob_store(&directory));
    assert_eq!(
        restarted
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("collector retries missing object"),
        1
    );
    assert_blob_missing(&restarted);
    assert_eq!(reservation_and_fence_counts(&directory), (0, 0, None));
}

#[test]
fn upload_blob_gc_reclaims_blob_written_before_receipt_after_restart() {
    let directory = tempfile::tempdir().expect("temporary upload GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_upload(&store, "upload-crash-after-put");
    reserve_chunk(
        &directory,
        "upload-crash-after-put",
        "request-after-put",
        EXPIRES,
    );
    write_chunk_blob(&store);
    drop(store);

    let restarted = open_store(&directory, file_blob_store(&directory));
    assert_eq!(
        restarted
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("collector removes unreferenced object"),
        1
    );
    assert_blob_missing(&restarted);
    assert_eq!(reservation_and_fence_counts(&directory), (0, 0, None));
}
