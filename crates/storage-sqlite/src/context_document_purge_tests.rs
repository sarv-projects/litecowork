use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use storage_core::{
    AcknowledgeContextDocumentPurgeTargetCommand, BlobPurpose, BlobRef, BlobStore,
    CommittedContextDocumentPurge, ContextDocumentPurgeReplicaKind,
    ContextDocumentPurgeStartCommand, ContextDocumentPurgeStatus, ContextDocumentPurgeStore,
    ContextDocumentPurgeTarget, ContextDocumentStatusEventContext, EventDraft, ResourceRecord,
    ResourceRevisionRecord, ResourceStore, ResourceUploadChunkInput, ResourceUploadContentRange,
    ResourceUploadSessionRecord, ResourceUploadState, ResourceUploadStore, StoreError,
    WorkspaceCreateRequest,
};
use zeroize::Zeroizing;

const WORKSPACE: &str = "workspace-context-purge";
const OWNER: &str = "owner-local";
const CREATED_AT: &str = "2026-10-15T10:00:00Z";
const CONTEXT_ADDED_AT: &str = "2026-10-15T10:00:02Z";
const GC_AT: &str = "2099-01-01T00:00:00Z";

struct PurgeTestKeys;

impl crate::WorkspaceBlobKeyProvider for PurgeTestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<crate::WorkspaceBlobKey, StoreError> {
        Ok(crate::WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([19_u8; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<crate::WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob(
                "unknown purge fixture key version".to_owned(),
            ));
        }
        self.current_key(workspace_id, purpose)
    }
}

fn state_database(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory.path().join("state").join("state.sqlite3")
}

fn test_store(directory: &tempfile::TempDir) -> crate::SqliteWorkspaceStore {
    let blobs = Arc::new(crate::FileBlobStore::new(
        directory.path().join("blobs"),
        PurgeTestKeys,
    ));
    test_store_with_blobs(directory, blobs)
}

fn test_store_with_blobs(
    directory: &tempfile::TempDir,
    blobs: Arc<dyn BlobStore>,
) -> crate::SqliteWorkspaceStore {
    crate::SqliteWorkspaceStore::open(
        state_database(directory),
        blobs,
        crate::SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("SQLite purge fixture opens")
}

struct ChunkPutPause {
    armed: std::sync::atomic::AtomicBool,
    entered_sender: std::sync::mpsc::SyncSender<()>,
    release_receiver: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}

struct ChunkPutPauseControl {
    state: Arc<ChunkPutPause>,
    entered_receiver: std::sync::mpsc::Receiver<()>,
    release_sender: Option<std::sync::mpsc::SyncSender<()>>,
}

impl ChunkPutPauseControl {
    fn release(&mut self) {
        if let Some(sender) = self.release_sender.take() {
            let _ = sender.send(());
        }
    }
}

impl Drop for ChunkPutPauseControl {
    fn drop(&mut self) {
        self.release();
    }
}

struct PausingChunkBlobStore {
    inner: Arc<crate::FileBlobStore<PurgeTestKeys>>,
    pause: Arc<ChunkPutPause>,
}

impl BlobStore for PausingChunkBlobStore {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
        // Pause only after FileBlobStore has published and verified the encrypted
        // object. The caller remains inside put_chunk's Workspace lock until release.
        let blob = self.inner.put(workspace_id, purpose, bytes, media_type)?;
        if purpose == BlobPurpose::ResourceUploadChunk
            && self
                .pause
                .armed
                .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.pause
                .entered_sender
                .send(())
                .map_err(|_| StoreError::Blob("chunk pause observer disappeared".to_owned()))?;
            self.pause
                .release_receiver
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .recv_timeout(Duration::from_secs(15))
                .map_err(|_| StoreError::Blob("chunk pause release timed out".to_owned()))?;
        }
        Ok(blob)
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
        self.inner.remove(workspace_id, purpose, blob)
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

fn fixture_with_chunk_put_pause() -> (PurgeFixture, ChunkPutPauseControl) {
    let directory = tempfile::tempdir().expect("temporary purge database and BlobStore");
    let inner = Arc::new(crate::FileBlobStore::new(
        directory.path().join("blobs"),
        PurgeTestKeys,
    ));
    let (entered_sender, entered_receiver) = std::sync::mpsc::sync_channel(1);
    let (release_sender, release_receiver) = std::sync::mpsc::sync_channel(1);
    let pause = Arc::new(ChunkPutPause {
        armed: std::sync::atomic::AtomicBool::new(false),
        entered_sender,
        release_receiver: std::sync::Mutex::new(release_receiver),
    });
    let blobs: Arc<dyn BlobStore> = Arc::new(PausingChunkBlobStore {
        inner,
        pause: Arc::clone(&pause),
    });
    let store = test_store_with_blobs(&directory, blobs);
    create_workspace(&store, WORKSPACE);
    (
        PurgeFixture { directory, store },
        ChunkPutPauseControl {
            state: pause,
            entered_receiver,
            release_sender: Some(release_sender),
        },
    )
}

fn create_workspace(store: &crate::SqliteWorkspaceStore, workspace_id: &str) {
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Context purge test Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: format!("event-create-{workspace_id}"),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: CREATED_AT.to_owned(),
                correlation_id: format!("correlation-create-{workspace_id}"),
                causation_id: None,
                recorded_at: CREATED_AT.to_owned(),
            },
        })
        .expect("purge fixture Workspace commits");
}

struct PurgeFixture {
    directory: tempfile::TempDir,
    store: crate::SqliteWorkspaceStore,
}

struct ResourceFixture {
    resource_id: String,
    revision_id: String,
    content: Vec<u8>,
    content_digest: String,
    index_digest: String,
}

fn fixture() -> PurgeFixture {
    let directory = tempfile::tempdir().expect("temporary purge database and BlobStore");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE);
    PurgeFixture { directory, store }
}

fn event_context(event_id: &str, recorded_at: &str) -> ContextDocumentStatusEventContext {
    ContextDocumentStatusEventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        hlc_timestamp: recorded_at.to_owned(),
        correlation_id: format!("correlation-{event_id}"),
        causation_id: None,
        recorded_at: recorded_at.to_owned(),
    }
}

fn context_document_metadata() -> Value {
    json!({
        "kind": "WORKSPACE_NOTES",
        "owner_ref": {"kind": "WORKSPACE", "workspace_id": WORKSPACE},
        "status": "ACTIVE",
    })
}

fn create_context_resource(
    fixture: &PurgeFixture,
    resource_id: &str,
    revision_id: &str,
    content: &[u8],
) -> ResourceFixture {
    let content_digest = crate::digest(content);
    let resource = ResourceRecord {
        resource_id: resource_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        kind: "FILE".to_owned(),
        provider_identity: json!({}),
        identity_digest: None,
        display_name: format!("{resource_id}.txt"),
        current_revision_id: Some(revision_id.to_owned()),
        sensitivity: "PERSONAL".to_owned(),
        provenance: json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
        created_at: CREATED_AT.to_owned(),
        updated_at: CREATED_AT.to_owned(),
        version: 1,
    };
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id.to_owned(),
        resource_id: resource_id.to_owned(),
        parent_revision_ids: Vec::new(),
        provider_revision: None,
        content_digest: Some(content_digest.clone()),
        size_bytes: Some(content.len() as u64),
        media_type: Some("text/plain".to_owned()),
        observed_at: CREATED_AT.to_owned(),
        created_by: json!({"kind": "USER", "principal_id": OWNER}),
    };
    let event_id = format!("event-create-{resource_id}");
    fixture
        .store
        .create_resource(
            WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: format!("request-create-{resource_id}"),
                request_payload: json!({
                    "operation": "resource.create.v1",
                    "workspace_id": WORKSPACE,
                    "resource_id": resource_id,
                    "upload_id": format!("fixture-upload-{resource_id}"),
                }),
            },
            resource.clone(),
            revision,
            EventDraft {
                event_id: event_id.clone(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "Resource".to_owned(),
                entity_id: resource_id.to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                entity_revision: 1,
                hlc_timestamp: CREATED_AT.to_owned(),
                correlation_id: format!("correlation-{event_id}"),
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
                recorded_at: CREATED_AT.to_owned(),
            },
            content.to_vec(),
        )
        .expect("managed Resource content commits");

    let database = state_database(&fixture.directory);
    Connection::open(&database)
        .expect("open purge fixture database")
        .execute(
            "UPDATE resources SET context_document_json = ?1, updated_at = ?2, version = 2
             WHERE workspace_id = ?3 AND resource_id = ?4 AND version = 1",
            params![
                String::from_utf8(crate::canonical_json(&context_document_metadata()).unwrap())
                    .unwrap(),
                CONTEXT_ADDED_AT,
                WORKSPACE,
                resource_id,
            ],
        )
        .expect("attach valid active ContextDocument metadata to the fixture Resource");

    let index_digest: String = Connection::open(&database)
        .expect("open index fixture database")
        .query_row(
            "SELECT extracted_blob_digest FROM resource_text_indexes
             WHERE workspace_id = ?1 AND resource_id = ?2 AND resource_revision_id = ?3",
            params![WORKSPACE, resource_id, revision_id],
            |row| row.get(0),
        )
        .expect("read the committed extracted snapshot identity");

    ResourceFixture {
        resource_id: resource_id.to_owned(),
        revision_id: revision_id.to_owned(),
        content: content.to_vec(),
        content_digest,
        index_digest,
    }
}

fn begin_command(
    resource_id: &str,
    request_id: &str,
    expected_version: u64,
    event_id: &str,
    recorded_at: &str,
) -> ContextDocumentPurgeStartCommand {
    ContextDocumentPurgeStartCommand {
        workspace_id: WORKSPACE.to_owned(),
        resource_id: resource_id.to_owned(),
        principal_id: OWNER.to_owned(),
        request_id: request_id.to_owned(),
        expected_version,
        request_payload: json!({
            "workspace_id": WORKSPACE,
            "resource_id": resource_id,
            "status": "DELETION_PENDING",
            "expected_version": expected_version,
        }),
        event: event_context(event_id, recorded_at),
    }
}

fn begin_purge(
    fixture: &PurgeFixture,
    resource_id: &str,
    request_id: &str,
    expected_version: u64,
    event_id: &str,
    recorded_at: &str,
) -> Result<CommittedContextDocumentPurge, storage_core::StoreError> {
    fixture.store.begin_context_document_purge(begin_command(
        resource_id,
        request_id,
        expected_version,
        event_id,
        recorded_at,
    ))
}

fn scalar_count(connection: &Connection, sql: &str, resource_id: &str) -> i64 {
    connection
        .query_row(sql, params![WORKSPACE, resource_id], |row| row.get(0))
        .expect("count rows for one Resource")
}

fn registry_exists(directory: &tempfile::TempDir, registry_table: &str, digest: &str) -> bool {
    let connection = Connection::open(state_database(directory)).expect("open registry database");
    let sql = format!(
        "SELECT EXISTS(SELECT 1 FROM {registry_table} WHERE workspace_id = ?1 AND digest = ?2)"
    );
    connection
        .query_row(&sql, params![WORKSPACE, digest], |row| row.get(0))
        .expect("read exact registry identity")
}

fn target_sort_key(target: &ContextDocumentPurgeTarget) -> (String, &'static str) {
    let kind = match target.replica_kind {
        ContextDocumentPurgeReplicaKind::Blob => "BLOB",
        ContextDocumentPurgeReplicaKind::DerivedIndex => "DERIVED_INDEX",
        ContextDocumentPurgeReplicaKind::UploadChunk => "UPLOAD_CHUNK",
    };
    (target.replica_ref.clone(), kind)
}

const PURGE_UPLOAD_ID: &str = "upload-context-purge-chunk";
const PURGE_UPLOAD_CONTENT: &[u8] = b"staged revision bytes for context purge";

fn create_pinned_revision_upload(fixture: &PurgeFixture, resource: &ResourceFixture) {
    create_pinned_revision_upload_with_content(fixture, resource, PURGE_UPLOAD_CONTENT);
}

fn create_pinned_revision_upload_with_content(
    fixture: &PurgeFixture,
    resource: &ResourceFixture,
    upload_content: &[u8],
) {
    create_pinned_revision_upload_session_with_content(fixture, resource, upload_content);
    put_pinned_revision_upload_chunk(fixture, upload_content);
}

fn create_pinned_revision_upload_session_with_content(
    fixture: &PurgeFixture,
    resource: &ResourceFixture,
    upload_content: &[u8],
) {
    create_named_pinned_revision_upload_session(
        fixture,
        resource,
        upload_content,
        PURGE_UPLOAD_ID,
        "request-context-purge-upload-create",
        "event-context-purge-upload-created",
    );
}

fn create_named_pinned_revision_upload_session(
    fixture: &PurgeFixture,
    resource: &ResourceFixture,
    upload_content: &[u8],
    upload_id: &str,
    request_id: &str,
    event_id: &str,
) {
    let upload_digest = crate::digest(upload_content);
    fixture
        .store
        .create_revision(
            WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: request_id.to_owned(),
                request_payload: json!({
                    "operation": "resource.revision.upload.create.v1",
                    "workspace_id": WORKSPACE,
                    "resource_id": resource.resource_id,
                    "expected_resource_version": 2,
                    "parent_revision_ids": [resource.revision_id],
                    "media_type": "text/plain",
                    "size_bytes": upload_content.len(),
                    "expected_digest": upload_digest,
                }),
            },
            ResourceUploadSessionRecord {
                upload_id: upload_id.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                display_name: String::new(),
                media_type: "text/plain".to_owned(),
                expected_size_bytes: upload_content.len() as u64,
                expected_digest: Some(upload_digest.clone()),
                context_document: None,
                folder_import: None,
                resource_id: Some(resource.resource_id.clone()),
                committed_resource_id: None,
                expected_resource_version: Some(2),
                parent_revision_ids: vec![resource.revision_id.clone()],
                chunk_size_bytes: 4_194_304,
                received_ranges: Vec::new(),
                next_missing_offset: 0,
                state: ResourceUploadState::Open,
                expires_at: "2099-01-01T00:00:00Z".to_owned(),
                created_at: "2026-10-15T10:00:03Z".to_owned(),
                version: 1,
                progress_version: 1,
            },
            EventDraft {
                event_id: event_id.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "ResourceUpload".to_owned(),
                entity_id: upload_id.to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                entity_revision: 1,
                hlc_timestamp: "2026-10-15T10:00:03Z".to_owned(),
                correlation_id: format!("correlation-{event_id}"),
                causation_id: None,
                schema_version: 1,
                event_type: "resource.upload.created.v1".to_owned(),
                payload: json!({
                    "upload_id": upload_id,
                    "workspace_id": WORKSPACE,
                    "expected_size_bytes": upload_content.len(),
                    "expected_digest": upload_digest,
                    "chunk_size_bytes": 4_194_304,
                    "expires_at": "2099-01-01T00:00:00Z",
                    "resource_id": resource.resource_id,
                    "expected_resource_version": 2,
                    "parent_revision_ids": [resource.revision_id],
                    "aggregate_version": 1,
                }),
                recorded_at: "2026-10-15T10:00:03Z".to_owned(),
            },
        )
        .expect("create revision upload pinned to the active ContextDocument head");
}

fn put_pinned_revision_upload_chunk(fixture: &PurgeFixture, upload_content: &[u8]) {
    put_named_pinned_revision_upload_chunk(
        fixture,
        upload_content,
        PURGE_UPLOAD_ID,
        "request-context-purge-upload-chunk",
    );
}

fn put_named_pinned_revision_upload_chunk(
    fixture: &PurgeFixture,
    upload_content: &[u8],
    upload_id: &str,
    request_id: &str,
) {
    let mut chunk = pinned_revision_upload_chunk(upload_content);
    chunk.upload_id = upload_id.to_owned();
    chunk.request_id = request_id.to_owned();
    chunk.lifecycle_event.event_id = format!("event-{request_id}");
    chunk.lifecycle_event.entity_id = upload_id.to_owned();
    chunk.lifecycle_event.correlation_id = format!("correlation-{request_id}");
    fixture
        .store
        .put_chunk(chunk)
        .expect("accept the pinned revision upload chunk through SQLite and FileBlobStore");
}

fn pinned_revision_upload_chunk(upload_content: &[u8]) -> ResourceUploadChunkInput {
    let upload_digest = crate::digest(upload_content);
    ResourceUploadChunkInput {
        upload_id: PURGE_UPLOAD_ID.to_owned(),
        chunk_index: 0,
        request_id: "request-context-purge-upload-chunk".to_owned(),
        content_range: ResourceUploadContentRange {
            start_offset: 0,
            end_offset_inclusive: upload_content.len() as u64 - 1,
            total_size_bytes: upload_content.len() as u64,
        },
        sha256: upload_digest,
        content: upload_content.to_vec(),
        received_at: "2026-10-15T10:00:03Z".to_owned(),
        lifecycle_event: EventDraft {
            event_id: "event-context-purge-upload-chunk".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: PURGE_UPLOAD_ID.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 1,
            hlc_timestamp: "2026-10-15T10:00:03Z".to_owned(),
            correlation_id: "correlation-context-purge-upload-chunk".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.status.changed.v1".to_owned(),
            payload: json!({}),
            recorded_at: "2026-10-15T10:00:03Z".to_owned(),
        },
    }
}

fn assert_purge_reconciles_reservation_only_upload_after_restart(object_was_written: bool) {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-upload-reservation-only",
        "revision-purge-upload-reservation-only",
        b"context document with an interrupted upload reservation",
    );
    create_pinned_revision_upload_session_with_content(&fixture, &resource, PURGE_UPLOAD_CONTENT);
    let chunk_digest = crate::digest(PURGE_UPLOAD_CONTENT);
    let mut connection = Connection::open(state_database(&fixture.directory))
        .expect("open SQLite to persist the pre-acceptance reservation boundary");
    let outcome = crate::reserve_resource_upload_blob_transaction(
        &mut connection,
        crate::ResourceUploadBlobReservation {
            workspace_id: WORKSPACE,
            upload_id: PURGE_UPLOAD_ID,
            request_id: "request-context-purge-reservation-only",
            chunk_index: 0,
            chunk_digest: &chunk_digest,
            size_bytes: PURGE_UPLOAD_CONTENT.len() as u64,
            created_at: "2026-10-15T10:00:03Z",
            expires_at: "2099-01-01T00:00:00Z",
        },
    )
    .expect("commit the durable reservation before the accepted chunk row");
    assert_eq!(
        outcome,
        crate::ResourceUploadBlobReservationOutcome::WriteObject
    );
    drop(connection);

    let chunk_blob = if object_was_written {
        Some(
            fixture
                .store
                .inner
                .blobs
                .put(
                    WORKSPACE,
                    BlobPurpose::ResourceUploadChunk,
                    PURGE_UPLOAD_CONTENT,
                    "application/octet-stream",
                )
                .expect("simulate a process stop after the physical put"),
        )
    } else {
        None
    };
    assert_eq!(
        chunk_blob.as_ref().map(|blob| blob.digest.as_str()),
        object_was_written.then_some(chunk_digest.as_str())
    );

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect the durable reservation without an accepted chunk row");
    let accepted_chunks: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunks WHERE upload_id = ?1",
            [PURGE_UPLOAD_ID],
            |row| row.get(0),
        )
        .expect("count accepted chunk ledger rows");
    let reservation_state: String = connection
        .query_row(
            "SELECT state FROM resource_upload_blob_reservations
             WHERE upload_id = ?1 AND request_id = ?2",
            params![PURGE_UPLOAD_ID, "request-context-purge-reservation-only"],
            |row| row.get(0),
        )
        .expect("read the interrupted durable reservation");
    drop(connection);
    assert_eq!(accepted_chunks, 0);
    assert_eq!(reservation_state, "RESERVED");

    let committed = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-upload-reservation-only",
        2,
        "event-purge-upload-reservation-only",
        "2026-10-15T10:00:04Z",
    )
    .expect("purge seals an interrupted durable reservation as a physical chunk target");
    assert_eq!(committed.status.required_replica_count, 3);

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect the sealed reservation-only target and expired upload");
    let targets_json: String = connection
        .query_row(
            "SELECT targets_json FROM context_document_purge_plans
             WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource.resource_id],
            |row| row.get(0),
        )
        .expect("read the immutable purge target manifest");
    let upload_state: String = connection
        .query_row(
            "SELECT state FROM resource_upload_sessions WHERE upload_id = ?1",
            [PURGE_UPLOAD_ID],
            |row| row.get(0),
        )
        .expect("read the upload state after purge admission");
    let expiry_payload: String = connection
        .query_row(
            "SELECT payload_json FROM domain_events
             WHERE entity_type = 'ResourceUpload' AND entity_id = ?1
               AND type = 'resource.upload.status.changed.v1'
               AND json_extract(payload_json, '$.to') = 'EXPIRED'
             ORDER BY origin_sequence DESC LIMIT 1",
            [PURGE_UPLOAD_ID],
            |row| row.get(0),
        )
        .expect("read the durable upload-expiry event");
    let reservation_state: String = connection
        .query_row(
            "SELECT state FROM resource_upload_blob_reservations
             WHERE upload_id = ?1 AND request_id = ?2",
            params![PURGE_UPLOAD_ID, "request-context-purge-reservation-only"],
            |row| row.get(0),
        )
        .expect("read the purge-fenced durable reservation");
    let registry_state: String = connection
        .query_row(
            "SELECT state FROM resource_upload_chunk_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, chunk_digest],
            |row| row.get(0),
        )
        .expect("read the shared chunk registry before collector claim");
    drop(connection);

    let targets: Value =
        serde_json::from_str(&targets_json).expect("target manifest is valid JSON");
    let exact_reservation_target = targets.as_array().is_some_and(|targets| {
        targets.iter().any(|target| {
            target["replica_ref"] == chunk_digest
                && target["replica_kind"] == "UPLOAD_CHUNK"
                && target["target_revision_ids"] == json!([])
                && target["target_upload_chunk_refs"]
                    == json!([{"upload_id": PURGE_UPLOAD_ID, "chunk_index": 0}])
        })
    });
    let expiry_payload: Value =
        serde_json::from_str(&expiry_payload).expect("event payload is JSON");
    assert!(
        exact_reservation_target,
        "manifest must retain exact reserved upload identity"
    );
    assert_eq!(upload_state, "EXPIRED");
    assert_eq!(expiry_payload["reason_code"], "CONTEXT_DOCUMENT_DELETION");
    assert_eq!(reservation_state, "DELETING");
    assert_eq!(
        registry_state, "RESERVED",
        "collector claims the object behind a durable fence"
    );

    let PurgeFixture { directory, store } = fixture;
    drop(store);
    let fixture = PurgeFixture {
        store: test_store(&directory),
        directory,
    };
    let collected_chunks = fixture
        .store
        .collect_orphan_chunks(GC_AT, 100)
        .expect("restart recovery removes the exact reservation-only upload object");
    assert_eq!(collected_chunks, 1);
    fixture
        .store
        .collect_orphan_resource_content_blobs(GC_AT, 100)
        .expect("collect the unreferenced managed Resource revision blob");
    fixture
        .store
        .collect_orphan_resource_index_blobs(GC_AT, 100)
        .expect("collect the unreferenced ResourceIndex snapshot");
    assert!(matches!(
        fixture.store.inner.blobs.get(
            WORKSPACE,
            BlobPurpose::ResourceUploadChunk,
            &storage_core::BlobRef {
                digest: chunk_digest.clone(),
                size_bytes: PURGE_UPLOAD_CONTENT.len() as u64,
                media_type: "application/octet-stream".to_owned(),
            },
        ),
        Err(StoreError::NotFound)
    ));
    assert!(!registry_exists(
        &fixture.directory,
        "resource_upload_chunk_blob_registry",
        &chunk_digest,
    ));

    let pending = fixture
        .store
        .list_pending_context_document_purge_targets(100)
        .expect("load all pending targets after restart reconciliation");
    assert_eq!(pending.len(), 3);
    for (sequence, target) in pending.iter().enumerate() {
        assert!(acknowledge_purge_target(&fixture, target, sequence as u32));
    }
    let status = fixture
        .store
        .get_context_document_deletion_status(WORKSPACE, &resource.resource_id, OWNER)
        .expect("read final owner-scoped purge status")
        .expect("purge status remains available after restart");
    assert_eq!(status.status, ContextDocumentPurgeStatus::Deleted);
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect durable receipts after completion");
    let acknowledged: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM context_document_purge_receipts
             WHERE workspace_id = ?1 AND resource_id = ?2 AND status = 'ACKNOWLEDGED'",
            params![WORKSPACE, resource.resource_id],
            |row| row.get(0),
        )
        .expect("count committed exact target receipts");
    assert_eq!(acknowledged, 3);
}

#[test]
fn purge_reconciles_reservation_only_upload_after_restart_before_blob_put() {
    assert_purge_reconciles_reservation_only_upload_after_restart(false);
}

#[test]
fn purge_reconciles_reservation_only_upload_after_restart_after_blob_put() {
    assert_purge_reconciles_reservation_only_upload_after_restart(true);
}

#[test]
fn purge_plan_allows_same_digest_as_resource_blob_and_upload_chunk() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-shared-blob-chunk-digest",
        "revision-purge-shared-blob-chunk-digest",
        b"identical resource and uploaded chunk bytes",
    );
    create_pinned_revision_upload_with_content(&fixture, &resource, &resource.content);

    let committed = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-shared-blob-chunk-digest",
        2,
        "event-purge-shared-blob-chunk-digest",
        "2026-10-15T10:00:04Z",
    )
    .expect("BLOB and UPLOAD_CHUNK are distinct targets even when their digest is equal");

    assert_eq!(committed.status.required_replica_count, 3);
    assert!(purge_manifest_has_exact_upload_chunk_target(
        &fixture,
        &resource.resource_id,
        &resource.content_digest,
    ));
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect the sealed manifest with colliding digests");
    let blob_target_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM context_document_purge_receipts
             WHERE workspace_id = ?1 AND resource_id = ?2
               AND replica_ref = ?3 AND replica_kind = 'BLOB'",
            params![WORKSPACE, resource.resource_id, resource.content_digest],
            |row| row.get(0),
        )
        .expect("same-digest content object has its own BLOB receipt");
    assert_eq!(blob_target_count, 1);
}

fn purge_upload_chunk_blob() -> storage_core::BlobRef {
    storage_core::BlobRef {
        digest: crate::digest(PURGE_UPLOAD_CONTENT),
        size_bytes: PURGE_UPLOAD_CONTENT.len() as u64,
        media_type: "application/octet-stream".to_owned(),
    }
}

fn purge_manifest_has_exact_upload_chunk_target(
    fixture: &PurgeFixture,
    resource_id: &str,
    chunk_digest: &str,
) -> bool {
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("open the sealed upload purge manifest");
    let targets_json: String = connection
        .query_row(
            "SELECT targets_json FROM context_document_purge_plans
             WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource_id],
            |row| row.get(0),
        )
        .expect("read the exact sealed upload purge manifest");
    let targets: Value = serde_json::from_str(&targets_json).expect("manifest is valid JSON");
    targets.as_array().is_some_and(|targets| {
        targets.iter().any(|target| {
            target["replica_ref"] == chunk_digest
                && target["replica_kind"] == "UPLOAD_CHUNK"
                && target["target_revision_ids"] == json!([])
                && target["target_upload_chunk_refs"]
                    == json!([{"upload_id": PURGE_UPLOAD_ID, "chunk_index": 0}])
        })
    })
}

#[test]
fn purge_waits_for_inflight_linked_chunk_writer_and_seals_accepted_reference() {
    // This proves the process-local coordination used by one litecoworkd instance;
    // it does not claim coordination between separate daemon processes.
    let (fixture, mut pause) = fixture_with_chunk_put_pause();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-chunk-writer-race",
        "revision-purge-chunk-writer-race",
        b"context document with a racing revision upload",
    );
    create_pinned_revision_upload_session_with_content(&fixture, &resource, PURGE_UPLOAD_CONTENT);

    pause
        .state
        .armed
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let writer_store = fixture.store.clone();
    let (writer_sender, writer_receiver) = std::sync::mpsc::sync_channel(1);
    let writer = std::thread::spawn(move || {
        let result = writer_store.put_chunk(pinned_revision_upload_chunk(PURGE_UPLOAD_CONTENT));
        let _ = writer_sender.send(result);
    });
    let writer_paused = pause
        .entered_receiver
        .recv_timeout(Duration::from_secs(5))
        .is_ok();

    let (contention_sender, contention_receiver) = std::sync::mpsc::sync_channel(1);
    fixture
        .store
        .observe_next_resource_index_write_contention(WORKSPACE, contention_sender);
    let purge_store = fixture.store.clone();
    let (purge_sender, purge_receiver) = std::sync::mpsc::sync_channel(1);
    let purge = std::thread::spawn(move || {
        let result = purge_store.begin_context_document_purge(begin_command(
            "resource-purge-chunk-writer-race",
            "request-purge-chunk-writer-race",
            2,
            "event-purge-chunk-writer-race",
            "2026-10-15T10:00:04Z",
        ));
        let _ = purge_sender.send(result);
    });

    let contention_observed = contention_receiver
        .recv_timeout(Duration::from_secs(5))
        .is_ok();
    let (purge_finished_while_writer_paused, early_purge_result) =
        match purge_receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(result) => (true, Some(result)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (false, None),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => (true, None),
        };

    // Release before inspecting any outcome so every exit path lets the writer
    // proceed and both worker threads can be joined.
    pause.release();
    let writer_result = writer_receiver.recv_timeout(Duration::from_secs(8));
    let writer_joined = writer.join().is_ok();
    let purge_result =
        early_purge_result.or_else(|| purge_receiver.recv_timeout(Duration::from_secs(8)).ok());
    let purge_joined = purge.join().is_ok();

    if !contention_observed {
        let observer_key = (
            fixture.store.inner.database_path.clone(),
            WORKSPACE.to_owned(),
        );
        crate::RESOURCE_INDEX_WRITE_CONTENTION_OBSERVERS
            .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&observer_key);
    }

    let purge_admitted = purge_result
        .as_ref()
        .is_some_and(|result| matches!(result, Ok(committed) if committed.status.status == ContextDocumentPurgeStatus::DeletionPending));
    let writer_committed = matches!(
        writer_result,
        Ok(Ok(ref session)) if session.state == ResourceUploadState::ContentReceived
    );
    let chunk_digest = crate::digest(PURGE_UPLOAD_CONTENT);
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("open the completed chunk-writer purge race database");
    let final_upload_state = connection
        .query_row(
            "SELECT state FROM resource_upload_sessions WHERE upload_id = ?1",
            [PURGE_UPLOAD_ID],
            |row| row.get::<_, String>(0),
        )
        .ok();
    let accepted_chunk_digest = connection
        .query_row(
            "SELECT sha256 FROM resource_upload_chunks
             WHERE upload_id = ?1 AND chunk_index = 0",
            [PURGE_UPLOAD_ID],
            |row| row.get::<_, String>(0),
        )
        .ok();
    drop(connection);
    let exact_chunk_ref_is_sealed = purge_manifest_has_exact_upload_chunk_target(
        &fixture,
        &resource.resource_id,
        &chunk_digest,
    );

    assert!(
        writer_paused,
        "the real chunk BlobStore put reached its pause"
    );
    assert!(
        contention_observed,
        "purge must observe contention on the shared Workspace ResourceIndex lock"
    );
    assert!(
        !purge_finished_while_writer_paused,
        "purge must not seal its plan while the linked chunk writer is inside BlobStore put"
    );
    assert!(
        writer_joined && writer_committed,
        "the released writer commits the accepted chunk"
    );
    assert!(
        purge_joined && purge_admitted,
        "purge completes after the writer releases its lock"
    );
    assert_eq!(
        accepted_chunk_digest.as_deref(),
        Some(chunk_digest.as_str())
    );
    assert!(
        exact_chunk_ref_is_sealed,
        "the sealed manifest contains the exact upload_id/chunk_index reference"
    );
    assert_eq!(
        final_upload_state.as_deref(),
        Some("EXPIRED"),
        "purge expires the linked session after including its accepted chunk"
    );
}

fn acknowledge_purge_target(
    fixture: &PurgeFixture,
    target: &storage_core::PendingContextDocumentPurgeTarget,
    sequence: u32,
) -> bool {
    let acknowledged_second = 20 + sequence * 2;
    let completion_second = acknowledged_second + 1;
    let acknowledged_at = format!("2026-10-15T10:00:{acknowledged_second:02}Z");
    let completion_at = format!("2026-10-15T10:00:{completion_second:02}Z");
    fixture
        .store
        .acknowledge_context_document_purge_target_if_object_physically_removed(
            AcknowledgeContextDocumentPurgeTargetCommand {
                workspace_id: target.workspace_id.clone(),
                resource_id: target.resource_id.clone(),
                manifest_digest: target.manifest_digest.clone(),
                target: target.target.clone(),
                acknowledged_at: acknowledged_at.clone(),
                event: event_context(
                    &format!("event-context-purge-ack-{sequence}"),
                    &acknowledged_at,
                ),
                completion_event: event_context(
                    &format!("event-context-purge-complete-{sequence}"),
                    &completion_at,
                ),
            },
        )
        .expect("acknowledge only after the exact target object is physically removed")
}

#[test]
fn purge_acknowledgement_event_omits_shared_revision_membership_arrays() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-many-shared-revisions",
        "revision-purge-many-shared-revisions-00",
        b"one managed object shared by many immutable revisions",
    );

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("open many-revision purge fixture");
    let mut parent_revision_id = resource.revision_id.clone();
    for index in 1..=20 {
        let revision_id = format!("revision-purge-many-shared-revisions-{index:02}");
        connection
            .execute(
                "INSERT INTO resource_revisions(resource_revision_id, resource_id, content_digest,
                     size_bytes, media_type, observed_at, created_by_json)
                 VALUES (?1, ?2, ?3, ?4, 'text/plain', ?5, '{}')",
                params![
                    revision_id,
                    resource.resource_id,
                    resource.content_digest,
                    resource.content.len() as i64,
                    "2026-10-15T10:00:03Z",
                ],
            )
            .expect("append immutable revision sharing the exact content object");
        connection
            .execute(
                "INSERT INTO resource_revision_parents(resource_id, child_revision_id, parent_revision_id)
                 VALUES (?1, ?2, ?3)",
                params![resource.resource_id, revision_id, parent_revision_id],
            )
            .expect("append revision parent edge");
        parent_revision_id = revision_id;
    }
    connection
        .execute(
            "UPDATE resources SET current_revision_id = ?1
             WHERE workspace_id = ?2 AND resource_id = ?3",
            params![parent_revision_id, WORKSPACE, resource.resource_id],
        )
        .expect("advance the immutable revision head");
    drop(connection);

    let committed = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-many-shared-revisions",
        2,
        "event-purge-many-shared-revisions",
        "2026-10-15T10:00:04Z",
    )
    .expect("owner seals all shared revision references in the purge manifest");
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect the immutable many-reference manifest");
    let targets_json: String = connection
        .query_row(
            "SELECT targets_json FROM context_document_purge_plans
             WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource.resource_id],
            |row| row.get(0),
        )
        .expect("load exact manifest bytes");
    let targets: Value = serde_json::from_str(&targets_json).expect("manifest is valid JSON");
    let blob_target = targets
        .as_array()
        .expect("manifest target list")
        .iter()
        .find(|target| target["replica_kind"] == "BLOB")
        .expect("shared Resource content target");
    let revision_ids = blob_target["target_revision_ids"]
        .as_array()
        .expect("manifest retains exact revision IDs");
    assert_eq!(revision_ids.len(), 21);
    assert_eq!(committed.status.required_replica_count, 2);
    drop(connection);

    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("remove the now-unreferenced Resource content object"),
        1
    );
    let pending = fixture
        .store
        .list_pending_context_document_purge_targets(100)
        .expect("read the full local target receipt")
        .into_iter()
        .find(|target| target.target.replica_kind == ContextDocumentPurgeReplicaKind::Blob)
        .expect("pending BLOB receipt includes the exact target");
    assert_eq!(pending.target.target_revision_ids.len(), 21);
    assert!(acknowledge_purge_target(&fixture, &pending, 0));

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect the compact acknowledgement event");
    let payload_json: String = connection
        .query_row(
            "SELECT payload_json FROM domain_events
             WHERE workspace_id = ?1 AND entity_id = ?2
               AND type = 'resource.context_document.purge.acknowledged.v2'",
            params![WORKSPACE, resource.resource_id],
            |row| row.get(0),
        )
        .expect("write the membership-array-free v2 acknowledgement event");
    let payload: Value = serde_json::from_str(&payload_json).expect("event payload is valid JSON");
    assert_eq!(
        payload["purge_manifest_digest"],
        json!(committed.status.manifest_digest)
    );
    assert_eq!(payload["replica_ref"], pending.target.replica_ref);
    assert_eq!(payload["replica_kind"], "BLOB");
    assert_eq!(payload["runtime_id"], Value::Null);
    assert_eq!(payload["runtime_incarnation_id"], Value::Null);
    assert_eq!(
        payload["target_digest"],
        crate::digest(&crate::canonical_json(blob_target).expect("canonical manifest target"))
    );
    assert!(payload.get("target_revision_ids").is_none());
    assert!(payload.get("target_upload_chunk_refs").is_none());
    assert!(
        payload["receipt_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    assert!(
        payload_json.len() < 1_000,
        "fixture payload remains compact"
    );
    let expected_receipt_digest = crate::digest(
        &crate::canonical_json(&json!({
            "resource_id": resource.resource_id,
            "purge_manifest_digest": committed.status.manifest_digest,
            "replica_ref": pending.target.replica_ref,
            "replica_kind": "BLOB",
            "runtime_id": null,
            "runtime_incarnation_id": null,
            "target_digest": payload["target_digest"].clone(),
            "acknowledged_at": payload["acknowledged_at"].clone(),
        }))
        .expect("canonical compact acknowledgement receipt"),
    );
    assert_eq!(payload["receipt_digest"], expected_receipt_digest);

    let (receipt_digest, receipt_revision_ids_json): (String, String) = connection
        .query_row(
            "SELECT receipt_digest, target_revision_ids_json
             FROM context_document_purge_receipts
             WHERE workspace_id = ?1 AND resource_id = ?2
               AND replica_ref = ?3 AND replica_kind = 'BLOB'",
            params![WORKSPACE, resource.resource_id, pending.target.replica_ref],
            |row| Ok((row.get::<_, Option<String>>(0)?.unwrap(), row.get(1)?)),
        )
        .expect("read complete durable receipt after compact event");
    let durable_revision_ids: Vec<String> =
        serde_json::from_str(&receipt_revision_ids_json).expect("receipt target IDs are JSON");
    assert_eq!(durable_revision_ids.len(), 21);
    assert_eq!(payload["receipt_digest"], receipt_digest);

    let replay = fixture
        .store
        .acknowledge_context_document_purge_target_if_object_physically_removed(
            AcknowledgeContextDocumentPurgeTargetCommand {
                workspace_id: pending.workspace_id.clone(),
                resource_id: pending.resource_id.clone(),
                manifest_digest: pending.manifest_digest.clone(),
                target: pending.target.clone(),
                acknowledged_at: "2026-10-15T10:00:20Z".to_owned(),
                event: event_context("event-context-purge-ack-0", "2026-10-15T10:00:20Z"),
                completion_event: event_context(
                    "event-context-purge-ack-complete-0",
                    "2026-10-15T10:00:21Z",
                ),
            },
        )
        .expect("exact committed acknowledgement replay is idempotent");
    assert!(replay);
    let acknowledgement_event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_id = ?2
               AND type = 'resource.context_document.purge.acknowledged.v2'",
            params![WORKSPACE, resource.resource_id],
            |row| row.get(0),
        )
        .expect("count one original acknowledgement after replay");
    assert_eq!(acknowledgement_event_count, 1);
    let replayed_payload_json: String = connection
        .query_row(
            "SELECT payload_json FROM domain_events WHERE workspace_id = ?1 AND entity_id = ?2
               AND type = 'resource.context_document.purge.acknowledged.v2'",
            params![WORKSPACE, resource.resource_id],
            |row| row.get(0),
        )
        .expect("read original event after exact replay");
    assert_eq!(replayed_payload_json, payload_json);
}

#[test]
fn purge_acknowledgement_event_omits_upload_chunk_membership_arrays() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-many-upload-chunks",
        "revision-purge-many-upload-chunks",
        b"ContextDocument with many linked upload chunks",
    );
    for index in 0..20 {
        let upload_id = format!("upload-purge-many-chunks-{index:02}");
        create_named_pinned_revision_upload_session(
            &fixture,
            &resource,
            PURGE_UPLOAD_CONTENT,
            &upload_id,
            &format!("request-purge-many-upload-create-{index:02}"),
            &format!("event-purge-many-upload-create-{index:02}"),
        );
        put_named_pinned_revision_upload_chunk(
            &fixture,
            PURGE_UPLOAD_CONTENT,
            &upload_id,
            &format!("request-purge-many-upload-chunk-{index:02}"),
        );
    }

    let committed = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-many-upload-chunks",
        2,
        "event-purge-many-upload-chunks",
        "2026-10-15T10:00:04Z",
    )
    .expect("owner seals all accepted linked chunk references in one target");
    assert_eq!(
        fixture
            .store
            .collect_orphan_chunks(GC_AT, 100)
            .expect("remove the single shared temporary chunk object"),
        1
    );
    let pending = fixture
        .store
        .list_pending_context_document_purge_targets(100)
        .expect("load the complete local upload target")
        .into_iter()
        .find(|target| target.target.replica_kind == ContextDocumentPurgeReplicaKind::UploadChunk)
        .expect("all accepted chunks share one digest target");
    assert_eq!(pending.target.target_upload_chunk_refs.len(), 20);
    assert!(acknowledge_purge_target(&fixture, &pending, 0));

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect compact upload acknowledgement");
    let (payload_json, target_upload_refs_json): (String, String) = connection
        .query_row(
            "SELECT event.payload_json, receipt.target_upload_chunk_refs_json
             FROM domain_events event
             JOIN context_document_purge_receipts receipt
               ON receipt.workspace_id = event.workspace_id
              AND receipt.resource_id = event.entity_id
             WHERE event.workspace_id = ?1 AND event.entity_id = ?2
               AND event.type = 'resource.context_document.purge.acknowledged.v2'
               AND json_extract(event.payload_json, '$.replica_kind') = 'UPLOAD_CHUNK'",
            params![WORKSPACE, resource.resource_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read event and full receipt for exact upload target");
    let payload: Value = serde_json::from_str(&payload_json).expect("ACK payload is JSON");
    let receipt_refs: Vec<storage_core::ContextDocumentPurgeUploadChunkRef> =
        serde_json::from_str(&target_upload_refs_json).expect("full local refs remain JSON");
    let targets_json: String = connection
        .query_row(
            "SELECT targets_json FROM context_document_purge_plans
             WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource.resource_id],
            |row| row.get(0),
        )
        .expect("read immutable upload target manifest");
    let targets: Value = serde_json::from_str(&targets_json).expect("manifest is valid JSON");
    let manifest_target = targets
        .as_array()
        .expect("manifest targets")
        .iter()
        .find(|target| target["replica_kind"] == "UPLOAD_CHUNK")
        .expect("exact upload target");
    assert_eq!(receipt_refs.len(), 20);
    assert_eq!(pending.target.target_upload_chunk_refs.len(), 20);
    assert_eq!(
        payload["purge_manifest_digest"],
        json!(committed.status.manifest_digest)
    );
    assert_eq!(
        payload["target_digest"],
        crate::digest(&crate::canonical_json(manifest_target).expect("canonical target"))
    );
    assert!(payload.get("target_revision_ids").is_none());
    assert!(payload.get("target_upload_chunk_refs").is_none());
    assert!(
        payload_json.len() < 1_000,
        "ACK size does not depend on chunk refs"
    );
}

#[test]
fn purge_start_seals_sorted_manifest_unlinks_local_projections_and_replays_exactly() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-start",
        "revision-z",
        b"alpha context bytes remain pinned for history",
    );

    // Add a second immutable revision sharing the same managed content. The target's
    // revision set must retain both IDs in UTF-8 order after the local reference is
    // removed. The local index remains revision-scoped to revision-z.
    let second_revision = "revision-a";
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("open multi-revision purge fixture");
    connection
        .execute(
            "INSERT INTO resource_revisions(resource_revision_id, resource_id, content_digest,
                 size_bytes, media_type, observed_at, created_by_json)
             VALUES (?1, ?2, ?3, ?4, 'text/plain', ?5, '{}')",
            params![
                second_revision,
                resource.resource_id,
                resource.content_digest,
                resource.content.len() as i64,
                "2026-10-15T10:00:03Z",
            ],
        )
        .expect("append immutable revision sharing the exact content object");
    connection
        .execute(
            "INSERT INTO resource_revision_parents(resource_id, child_revision_id, parent_revision_id)
             VALUES (?1, ?2, ?3)",
            params![resource.resource_id, second_revision, resource.revision_id],
        )
        .expect("append parent edge");
    connection
        .execute(
            "UPDATE resources SET current_revision_id = ?1 WHERE workspace_id = ?2 AND resource_id = ?3",
            params![second_revision, WORKSPACE, resource.resource_id],
        )
        .expect("advance the immutable revision head");
    drop(connection);

    let committed = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-start",
        2,
        "event-purge-start",
        "2026-10-15T10:00:04Z",
    )
    .expect("owner starts ContextDocument purge");
    assert_eq!(
        committed.status.status,
        ContextDocumentPurgeStatus::DeletionPending
    );
    assert_eq!(committed.status.required_replica_count, 2);
    assert_eq!(committed.status.acknowledged_replica_count, 0);
    assert_eq!(
        committed.event.event_type,
        "resource.context_document.status.changed.v1"
    );
    assert_eq!(committed.event.payload["from"], "ACTIVE");
    assert_eq!(committed.event.payload["to"], "DELETION_PENDING");

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect committed purge SQL state");
    let (status, manifest_digest, target_count, targets_json): (String, String, i64, String) =
        connection
            .query_row(
                "SELECT json_extract(resource.context_document_json, '$.status'),
                        plan.manifest_digest, plan.target_count, plan.targets_json
                 FROM resources resource JOIN context_document_purge_plans plan
                   ON plan.workspace_id = resource.workspace_id AND plan.resource_id = resource.resource_id
                 WHERE resource.workspace_id = ?1 AND resource.resource_id = ?2",
                params![WORKSPACE, resource.resource_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read immutable plan and tombstone");
    assert_eq!(status, "DELETION_PENDING");
    assert_eq!(
        manifest_digest,
        committed.status.manifest_digest.as_deref().unwrap()
    );
    assert_eq!(target_count, 2);
    let targets: Vec<ContextDocumentPurgeTarget> =
        serde_json::from_str(&targets_json).expect("decode sealed manifest targets");
    let mut sorted_targets = targets.clone();
    sorted_targets.sort_by_key(target_sort_key);
    assert_eq!(
        targets, sorted_targets,
        "manifest target order is stable UTF-8/ASCII order"
    );
    let blob_target = targets
        .iter()
        .find(|target| target.replica_kind == ContextDocumentPurgeReplicaKind::Blob)
        .expect("managed Resource object is an explicit purge target");
    assert_eq!(
        blob_target.target_revision_ids,
        ["revision-a", "revision-z"]
    );
    for target in &targets {
        assert_eq!(target.target_revision_ids, {
            let mut ids = target.target_revision_ids.clone();
            ids.sort();
            ids
        });
    }
    let expected_digest = crate::digest(
        &crate::canonical_json(&serde_json::to_value(&targets).expect("serialize targets"))
            .expect("JCS canonical target array"),
    );
    assert_eq!(
        manifest_digest, expected_digest,
        "manifest digest commits to JCS target array"
    );

    let pending_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM context_document_purge_receipts
             WHERE workspace_id = ?1 AND resource_id = ?2 AND status = 'PENDING'
               AND acknowledged_at IS NULL AND receipt_digest IS NULL",
            params![WORKSPACE, resource.resource_id],
            |row| row.get(0),
        )
        .expect("count exact pending receipts");
    assert_eq!(pending_receipts, 2);
    assert_eq!(
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM resource_locations WHERE resource_id = ?2",
            &resource.resource_id
        ),
        0
    );
    assert_eq!(
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            &resource.resource_id
        ),
        0
    );
    assert_eq!(
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM resource_revisions WHERE resource_id = ?2",
            &resource.resource_id
        ),
        2
    );
    let retained_digests: Vec<String> = connection
        .prepare("SELECT content_digest FROM resource_revisions WHERE resource_id = ?1 ORDER BY resource_revision_id")
        .expect("prepare retained Resource history query")
        .query_map([&resource.resource_id], |row| row.get(0))
        .expect("query retained Resource history")
        .collect::<Result<_, _>>()
        .expect("collect retained content digests");
    assert_eq!(
        retained_digests,
        [
            resource.content_digest.clone(),
            resource.content_digest.clone()
        ]
    );
    drop(connection);

    let status = fixture
        .store
        .get_context_document_deletion_status(WORKSPACE, &resource.resource_id, OWNER)
        .expect("owner reads deletion status")
        .expect("ContextDocument status exists");
    assert_eq!(status.status, ContextDocumentPurgeStatus::DeletionPending);
    assert_eq!(
        status.manifest_digest.as_deref(),
        Some(manifest_digest.as_str())
    );

    let replay = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-start",
        2,
        "event-purge-start",
        "2026-10-15T10:00:04Z",
    )
    .expect("exact request replays after the Resource version advanced");
    assert_eq!(replay, committed);
    assert!(
        matches!(
            begin_purge(
                &fixture,
                &resource.resource_id,
                "request-purge-start",
                3,
                "event-purge-start-changed",
                "2026-10-15T10:00:05Z",
            ),
            Err(storage_core::StoreError::Conflict { .. })
        ),
        "same key with changed request payload conflicts"
    );
    assert!(
        matches!(
            begin_purge(
                &fixture,
                &resource.resource_id,
                "request-purge-stale",
                2,
                "event-purge-stale",
                "2026-10-15T10:00:06Z",
            ),
            Err(storage_core::StoreError::Conflict { .. })
        ),
        "a distinct stale-version request is rejected"
    );
    let event_count: i64 = Connection::open(state_database(&fixture.directory))
        .expect("inspect replay event count")
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id = ?1 AND type = 'resource.context_document.status.changed.v1'",
            [&resource.resource_id],
            |row| row.get(0),
        )
        .expect("count ContextDocument status events");
    assert_eq!(event_count, 1, "replay and stale requests append no event");
}

#[test]
fn purge_plan_sql_rejects_non_digest_refs_and_empty_revision_sets() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-shape-guard",
        "revision-purge-shape-guard",
        b"purge shape guard fixture",
    );
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("open purge shape guard database");
    let invalid_targets = [
        json!([{
            "replica_ref": "/private/blob-store/resource.dat",
            "replica_kind": "BLOB",
            "runtime_id": null,
            "runtime_incarnation_id": null,
            "target_revision_ids": [resource.revision_id],
        }]),
        json!([{
            "replica_ref": resource.content_digest,
            "replica_kind": "BLOB",
            "runtime_id": null,
            "runtime_incarnation_id": null,
            "target_revision_ids": [],
        }]),
    ];

    for targets in invalid_targets {
        let error = connection
            .execute(
                "INSERT INTO context_document_purge_plans(
                   workspace_id, resource_id, manifest_digest, target_count, targets_json, sealed_at
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5)",
                params![
                    WORKSPACE,
                    resource.resource_id,
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    crate::canonical_json(&targets).expect("canonical test target JSON"),
                    CONTEXT_ADDED_AT,
                ],
            )
            .expect_err("SQLite purge-plan shape trigger rejects malformed target");
        assert!(error.to_string().contains("INVALID_ARGUMENT"));
    }
}

#[test]
fn purge_start_denies_non_owner_foreign_workspace_and_external_location_without_writes() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-denials",
        "revision-purge-denials",
        b"external location remains outside local purge control",
    );
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("open external-location fixture");
    connection
        .execute(
            "UPDATE resource_locations SET provider_ref = 'unqualified.provider', locator_ref_id = 'opaque-location-ref'
             WHERE resource_id = ?1 AND provider_ref = 'litecowork.encrypted_blob'",
            [&resource.resource_id],
        )
        .expect("replace local locator with an external provider location");
    let before = (
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
            &resource.resource_id,
        ),
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM context_document_purge_receipts WHERE workspace_id = ?1 AND resource_id = ?2",
            &resource.resource_id,
        ),
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_id = ?2 AND type LIKE 'resource.context_document.%'",
            &resource.resource_id,
        ),
        connection
            .query_row(
                "SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'request-purge-%'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count purge request receipts before denied requests"),
    );
    drop(connection);

    let mut non_owner = begin_command(
        &resource.resource_id,
        "request-purge-non-owner",
        2,
        "event-purge-non-owner",
        "2026-10-15T10:00:03Z",
    );
    non_owner.principal_id = "someone-else".to_owned();
    assert!(matches!(
        fixture.store.begin_context_document_purge(non_owner),
        Err(storage_core::StoreError::NotFound)
    ));

    let mut foreign_workspace = begin_command(
        &resource.resource_id,
        "request-purge-foreign-workspace",
        2,
        "event-purge-foreign-workspace",
        "2026-10-15T10:00:03Z",
    );
    foreign_workspace.workspace_id = "workspace-other".to_owned();
    foreign_workspace.request_payload["workspace_id"] = json!("workspace-other");
    assert!(matches!(
        fixture
            .store
            .begin_context_document_purge(foreign_workspace),
        Err(storage_core::StoreError::NotFound)
    ));

    assert!(
        fixture
            .store
            .begin_context_document_purge(begin_command(
                &resource.resource_id,
                "request-purge-external-location",
                2,
                "event-purge-external-location",
                "2026-10-15T10:00:03Z",
            ))
            .is_err(),
        "unqualified external locations fail closed"
    );

    let connection =
        Connection::open(state_database(&fixture.directory)).expect("inspect denied purge state");
    let after = (
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
            &resource.resource_id,
        ),
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM context_document_purge_receipts WHERE workspace_id = ?1 AND resource_id = ?2",
            &resource.resource_id,
        ),
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_id = ?2 AND type LIKE 'resource.context_document.%'",
            &resource.resource_id,
        ),
        connection
            .query_row(
                "SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'request-purge-%'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count purge request receipts after denied requests"),
    );
    assert_eq!(
        after, before,
        "denied/unsupported requests write no purge state"
    );
    let status: String = connection
        .query_row(
            "SELECT json_extract(context_document_json, '$.status') FROM resources WHERE resource_id = ?1",
            [&resource.resource_id],
            |row| row.get(0),
        )
        .expect("read unchanged ContextDocument status");
    assert_eq!(status, "ACTIVE");
    assert_eq!(
        scalar_count(
            &connection,
            "SELECT COUNT(*) FROM resource_locations WHERE resource_id = ?2 AND provider_ref = 'unqualified.provider'",
            &resource.resource_id
        ),
        1
    );
}

#[test]
fn shared_blob_and_index_receipts_wait_for_last_reference_and_exact_gc_removal() {
    let fixture = fixture();
    let content = b"two ContextDocuments share exact content and index snapshots";
    let first = create_context_resource(
        &fixture,
        "resource-purge-shared-a",
        "revision-shared-a",
        content,
    );
    let second = create_context_resource(
        &fixture,
        "resource-purge-shared-b",
        "revision-shared-b",
        content,
    );
    assert_eq!(first.content_digest, second.content_digest);
    assert_eq!(first.index_digest, second.index_digest);

    let first_start = begin_purge(
        &fixture,
        &first.resource_id,
        "request-purge-shared-a",
        2,
        "event-purge-shared-a",
        "2026-10-15T10:00:03Z",
    )
    .expect("first ContextDocument starts purge");
    let first_pending = fixture
        .store
        .list_pending_context_document_purge_targets(100)
        .expect("list first pending replica targets");
    assert_eq!(first_pending.len(), 2);
    assert!(
        first_pending
            .iter()
            .all(|target| target.resource_id == first.resource_id)
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("shared Resource object remains rooted by second Resource"),
        0
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(GC_AT, 100)
            .expect("shared ResourceIndex snapshot remains rooted by second Resource"),
        0
    );
    assert!(registry_exists(
        &fixture.directory,
        "resource_content_blob_registry",
        &first.content_digest
    ));
    assert!(registry_exists(
        &fixture.directory,
        "resource_index_blob_registry",
        &first.index_digest
    ));
    assert_eq!(first_start.status.acknowledged_replica_count, 0);

    let _second_start = begin_purge(
        &fixture,
        &second.resource_id,
        "request-purge-shared-b",
        2,
        "event-purge-shared-b",
        "2026-10-15T10:00:04Z",
    )
    .expect("second ContextDocument removes final shared references");
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("final shared Resource object is physically removed"),
        1
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(GC_AT, 100)
            .expect("final shared index object is physically removed"),
        1
    );
    assert!(!registry_exists(
        &fixture.directory,
        "resource_content_blob_registry",
        &first.content_digest
    ));
    assert!(!registry_exists(
        &fixture.directory,
        "resource_index_blob_registry",
        &first.index_digest
    ));

    let all_pending = fixture
        .store
        .list_pending_context_document_purge_targets(100)
        .expect("both immutable purge plans remain available for reconciliation");
    assert_eq!(all_pending.len(), 4);
    for (index, pending) in all_pending.iter().enumerate() {
        let event = event_context(
            &format!("event-purge-ack-{}-{index}", pending.resource_id),
            &format!("2026-10-15T10:01:{:02}Z", index + 1),
        );
        assert!(
            fixture
                .store
                .acknowledge_context_document_purge_target_if_object_physically_removed(
                    AcknowledgeContextDocumentPurgeTargetCommand {
                        workspace_id: pending.workspace_id.clone(),
                        resource_id: pending.resource_id.clone(),
                        manifest_digest: pending.manifest_digest.clone(),
                        target: pending.target.clone(),
                        acknowledged_at: format!("2026-10-15T10:00:{:02}Z", index + 5),
                        event,
                        completion_event: event_context(
                            &format!("event-purge-complete-{}-{index}", pending.resource_id),
                            &format!("2026-10-15T10:02:{:02}Z", index + 1),
                        ),
                    },
                )
                .expect("exact physically removed target acknowledgement commits")
        );
    }
    for resource_id in [&first.resource_id, &second.resource_id] {
        let status = fixture
            .store
            .get_context_document_deletion_status(WORKSPACE, resource_id, OWNER)
            .expect("owner reads final status")
            .expect("purged Resource remains addressable for audit");
        assert_eq!(status.status, ContextDocumentPurgeStatus::Deleted);
        assert_eq!(status.required_replica_count, 2);
        assert_eq!(status.acknowledged_replica_count, 2);
    }
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect retained audit history after physical GC");
    for resource in [&first, &second] {
        let (revision_count, digest): (i64, String) = connection
            .query_row(
                "SELECT COUNT(*), MIN(content_digest) FROM resource_revisions WHERE resource_id = ?1",
                [&resource.resource_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("historical Resource identifiers and digest remain");
        assert_eq!(revision_count, 1);
        assert_eq!(digest, resource.content_digest);
    }
}

#[test]
fn purge_reconciles_failed_same_digest_reservations_without_collecting_shared_roots() {
    let fixture = fixture();
    let content = b"same digest remains rooted after a failed publication retry";
    let first = create_context_resource(
        &fixture,
        "resource-purge-reserved-shared-a",
        "revision-purge-reserved-shared-a",
        content,
    );
    let second = create_context_resource(
        &fixture,
        "resource-purge-reserved-shared-b",
        "revision-purge-reserved-shared-b",
        content,
    );
    assert_eq!(first.content_digest, second.content_digest);
    assert_eq!(first.index_digest, second.index_digest);

    // Model a process stop after the durable same-digest reservations commit but
    // before the subsequent BlobStore publication/projection transaction. The first
    // accepted Resource and its peer still root both objects.
    let index_size: i64 = Connection::open(state_database(&fixture.directory))
        .expect("open reservation fixture database")
        .query_row(
            "SELECT size_bytes FROM resource_index_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, first.index_digest],
            |row| row.get(0),
        )
        .expect("read exact registered index snapshot size");
    let content_blob = BlobRef {
        digest: first.content_digest.clone(),
        size_bytes: content.len() as u64,
        media_type: "text/plain".to_owned(),
    };
    let index_blob = BlobRef {
        digest: first.index_digest.clone(),
        size_bytes: u64::try_from(index_size).expect("registered index size is nonnegative"),
        media_type: crate::resource_index::INDEX_MEDIA_TYPE.to_owned(),
    };

    let index_lock = fixture.store.resource_index_write_lock(WORKSPACE);
    let _index_guard = fixture
        .store
        .lock_resource_index_write(WORKSPACE, &index_lock);
    fixture
        .store
        .reserve_resource_index_snapshot(WORKSPACE, &index_blob)
        .expect("same-digest index reservation commits before the simulated crash");
    let content_lock = fixture
        .store
        .resource_content_blob_operation_lock(WORKSPACE, &content_blob.digest);
    let _content_guard = content_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    fixture
        .store
        .reserve_resource_content_blob(WORKSPACE, &content_blob)
        .expect("same-digest content reservation commits before the simulated crash");
    drop(_content_guard);
    drop(_index_guard);

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect durable reservations after the simulated crash");
    let (content_state, content_reservation_created): (String, String) = connection
        .query_row(
            "SELECT state, created_at FROM resource_content_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, first.content_digest],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read content reservation state");
    let (index_state, index_reservation_created): (String, String) = connection
        .query_row(
            "SELECT state, created_at FROM resource_index_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, first.index_digest],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read index reservation state");
    assert_eq!(content_state, "RESERVED");
    assert_eq!(index_state, "RESERVED");
    drop(connection);

    let first_start = begin_purge(
        &fixture,
        &first.resource_id,
        "request-purge-reserved-shared-a",
        2,
        "event-purge-reserved-shared-a",
        "2026-10-15T10:00:03Z",
    )
    .expect("purge seals exact rooted content and index reservations");
    assert_eq!(first_start.status.required_replica_count, 2);
    let first_targets = fixture
        .store
        .list_pending_context_document_purge_targets(100)
        .expect("list first purge's pending targets");
    assert_eq!(first_targets.len(), 2);
    assert!(first_targets.iter().any(|target| {
        target.resource_id == first.resource_id
            && target.target.replica_ref == first.content_digest
            && target.target.replica_kind == ContextDocumentPurgeReplicaKind::Blob
            && target.target.target_revision_ids == [first.revision_id.clone()]
    }));
    assert!(first_targets.iter().any(|target| {
        target.resource_id == first.resource_id
            && target.target.replica_ref == first.index_digest
            && target.target.replica_kind == ContextDocumentPurgeReplicaKind::DerivedIndex
            && target.target.target_revision_ids == [first.revision_id.clone()]
    }));
    let replay = begin_purge(
        &fixture,
        &first.resource_id,
        "request-purge-reserved-shared-a",
        2,
        "event-purge-reserved-shared-a",
        "2026-10-15T10:00:03Z",
    )
    .expect("exact purge retry replays after the reservation recovery path");
    assert_eq!(replay, first_start);

    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(&content_reservation_created, 100)
            .expect("unexpired same-digest reservation remains pending"),
        0
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(&index_reservation_created, 100)
            .expect("unexpired same-digest index reservation remains pending"),
        0
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("shared peer Resource still roots reserved content"),
        0
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(GC_AT, 100)
            .expect("shared peer Resource still roots reserved index"),
        0
    );
    let content_state: String = Connection::open(state_database(&fixture.directory))
        .expect("open shared-root reservation state")
        .query_row(
            "SELECT state FROM resource_content_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, first.content_digest],
            |row| row.get(0),
        )
        .expect("read still-rooted content reservation state");
    let index_state: String = Connection::open(state_database(&fixture.directory))
        .expect("open shared-root index reservation state")
        .query_row(
            "SELECT state FROM resource_index_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, first.index_digest],
            |row| row.get(0),
        )
        .expect("read still-rooted index reservation state");
    assert_eq!(content_state, "RESERVED");
    assert_eq!(index_state, "RESERVED");

    begin_purge(
        &fixture,
        &second.resource_id,
        "request-purge-reserved-shared-b",
        2,
        "event-purge-reserved-shared-b",
        "2026-10-15T10:00:04Z",
    )
    .expect("the peer purge also accepts exact reserved roots");
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(&content_reservation_created, 100)
            .expect("unexpired reservation does not collect after roots are removed"),
        0
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(&index_reservation_created, 100)
            .expect("unexpired index reservation does not collect after roots are removed"),
        0
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("expired, now-unrooted content is collected"),
        1
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(GC_AT, 100)
            .expect("expired, now-unrooted index is collected"),
        1
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("content collector retry is idempotent"),
        0
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(GC_AT, 100)
            .expect("index collector retry is idempotent"),
        0
    );
    assert!(matches!(
        fixture
            .store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &content_blob),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        fixture
            .store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &index_blob),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn empty_purge_plan_completes_only_for_the_exact_verified_manifest() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-empty",
        "revision-purge-empty",
        b"content removed before an empty-plan purge begins",
    );
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("open empty-plan inventory fixture");
    connection
        .execute(
            "DELETE FROM resource_locations WHERE resource_id = ?1",
            [&resource.resource_id],
        )
        .expect("fixture loses its managed content binding before purge begins");
    connection
        .execute(
            "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource.resource_id],
        )
        .expect("fixture loses its derived projection before purge begins");
    drop(connection);
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("remove the final content object before verified-empty inventory"),
        1
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(GC_AT, 100)
            .expect("remove the final index object before verified-empty inventory"),
        1
    );

    let committed = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-empty",
        2,
        "event-purge-empty",
        "2026-10-15T10:00:03Z",
    )
    .expect("empty local replica inventory seals an explicit empty manifest");
    assert_eq!(committed.status.required_replica_count, 0);
    let pending = fixture
        .store
        .list_pending_empty_context_document_purge_plans(100)
        .expect("list explicit empty plans awaiting inventory verification");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].resource_id, resource.resource_id);
    assert_eq!(
        pending[0].manifest_digest,
        committed.status.manifest_digest.unwrap()
    );
    assert!(
        !fixture
            .store
            .complete_empty_context_document_purge_plan_if_verified_empty(
                WORKSPACE,
                &resource.resource_id,
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                event_context("event-purge-empty-wrong-plan", "2026-10-15T10:00:04Z"),
            )
            .expect("wrong plan digest is a no-op")
    );
    assert_eq!(
        fixture
            .store
            .get_context_document_deletion_status(WORKSPACE, &resource.resource_id, OWNER)
            .expect("read still-pending empty-plan status")
            .unwrap()
            .status,
        ContextDocumentPurgeStatus::DeletionPending
    );
    // Model a late same-digest local publication racing empty-plan reconciliation.
    // The immutable revision digest still identifies these objects even though their
    // original ResourceLocation/projection was removed before the plan was sealed.
    fixture
        .store
        .inner
        .blobs
        .put(
            WORKSPACE,
            BlobPurpose::Resource,
            &resource.content,
            "text/plain",
        )
        .expect("publish late managed content for the retained revision digest");
    fixture
        .store
        .inner
        .blobs
        .put(
            WORKSPACE,
            BlobPurpose::ResourceIndex,
            &resource.content,
            crate::resource_index::INDEX_MEDIA_TYPE,
        )
        .expect("publish late derived snapshot for the retained index digest");
    let connection =
        Connection::open(state_database(&fixture.directory)).expect("open late-inventory fixture");
    connection
        .execute(
            "INSERT INTO resource_content_blob_registry(workspace_id, digest, size_bytes,
                 media_type, created_at, expires_at, state, claimed_at)
             VALUES (?1, ?2, ?3, 'text/plain', ?4, NULL, 'TRACKED', NULL)",
            params![
                WORKSPACE,
                resource.content_digest,
                resource.content.len() as i64,
                CONTEXT_ADDED_AT,
            ],
        )
        .expect("record late content in local recovery inventory");
    connection
        .execute(
            "INSERT INTO resource_index_blob_registry(workspace_id, digest, size_bytes,
                 created_at, expires_at, state, claimed_at)
             VALUES (?1, ?2, ?3, ?4, NULL, 'TRACKED', NULL)",
            params![
                WORKSPACE,
                resource.index_digest,
                resource.content.len() as i64,
                CONTEXT_ADDED_AT,
            ],
        )
        .expect("record late derived snapshot in local recovery inventory");
    drop(connection);
    assert!(
        !fixture
            .store
            .complete_empty_context_document_purge_plan_if_verified_empty(
                WORKSPACE,
                &resource.resource_id,
                &pending[0].manifest_digest,
                event_context(
                    "event-purge-empty-inventory-present",
                    "2026-10-15T10:00:05Z"
                ),
            )
            .expect("late exact inventory keeps an empty plan pending"),
        "an empty plan cannot complete while a registered matching object exists"
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("remove late unreferenced content object"),
        1
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(GC_AT, 100)
            .expect("remove late unreferenced derived object"),
        1
    );
    let unregistered_content = fixture
        .store
        .inner
        .blobs
        .put(
            WORKSPACE,
            BlobPurpose::Resource,
            &resource.content,
            "text/plain",
        )
        .expect("model bytes reappearing without a recovery-registry row");
    assert!(
        !fixture
            .store
            .complete_empty_context_document_purge_plan_if_verified_empty(
                WORKSPACE,
                &resource.resource_id,
                &pending[0].manifest_digest,
                event_context(
                    "event-purge-empty-unregistered-content",
                    "2026-10-15T10:00:06Z",
                ),
            )
            .expect("unregistered retained revision bytes keep the plan pending"),
        "empty-plan completion probes retained revision digests without registry rows"
    );
    fixture
        .store
        .inner
        .blobs
        .remove(WORKSPACE, BlobPurpose::Resource, &unregistered_content)
        .expect("remove the modeled unregistered late publication");
    assert!(
        fixture
            .store
            .complete_empty_context_document_purge_plan_if_verified_empty(
                WORKSPACE,
                &resource.resource_id,
                &pending[0].manifest_digest,
                event_context("event-purge-empty-complete", "2026-10-15T10:00:07Z"),
            )
            .expect("exact verified-empty manifest completes")
    );
    assert_eq!(
        fixture
            .store
            .get_context_document_deletion_status(WORKSPACE, &resource.resource_id, OWNER)
            .expect("read completed empty-plan status")
            .unwrap()
            .status,
        ContextDocumentPurgeStatus::Deleted
    );
}

#[test]
fn empty_purge_admission_rejects_unregistered_revision_bytes() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-unregistered-admission",
        "revision-purge-unregistered-admission",
        b"untracked content must not be hidden by a missing registry row",
    );
    let connection = Connection::open(state_database(&fixture.directory))
        .expect("open unregistered-content admission fixture");
    connection
        .execute(
            "DELETE FROM resource_locations WHERE resource_id = ?1",
            [&resource.resource_id],
        )
        .expect("remove the local managed binding before simulating the orphan");
    connection
        .execute(
            "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource.resource_id],
        )
        .expect("remove the derived projection before empty-plan admission");
    drop(connection);
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_content_blobs(GC_AT, 100)
            .expect("collect the managed object and its registry row"),
        1
    );
    assert_eq!(
        fixture
            .store
            .collect_orphan_resource_index_blobs(GC_AT, 100)
            .expect("collect the derived object and its registry row"),
        1
    );
    let unregistered_content = fixture
        .store
        .inner
        .blobs
        .put(
            WORKSPACE,
            BlobPurpose::Resource,
            &resource.content,
            "text/plain",
        )
        .expect("publish the retained revision digest without recreating its registry row");

    assert!(matches!(
        begin_purge(
            &fixture,
            &resource.resource_id,
            "request-purge-unregistered-admission",
            2,
            "event-purge-unregistered-admission",
            "2026-10-15T10:00:03Z",
        ),
        Err(StoreError::Integrity(_))
    ));
    assert_eq!(
        fixture
            .store
            .get_context_document_deletion_status(WORKSPACE, &resource.resource_id, OWNER)
            .expect("the Resource remains readable after fail-closed admission")
            .unwrap()
            .status,
        ContextDocumentPurgeStatus::Active
    );
    assert_eq!(
        fixture
            .store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &unregistered_content)
            .expect("unregistered bytes remain untouched after admission rejection"),
        resource.content
    );
}

#[test]
fn purge_transaction_rolls_back_plan_receipt_and_event_insertion_failures() {
    for (trigger_name, trigger_sql) in [
        (
            "reject_purge_plan",
            "CREATE TRIGGER reject_purge_plan BEFORE INSERT ON context_document_purge_plans
             BEGIN SELECT RAISE(ABORT, 'injected purge plan failure'); END;",
        ),
        (
            "reject_purge_receipt",
            "CREATE TRIGGER reject_purge_receipt BEFORE INSERT ON context_document_purge_receipts
             BEGIN SELECT RAISE(ABORT, 'injected purge receipt failure'); END;",
        ),
        (
            "reject_purge_event",
            "CREATE TRIGGER reject_purge_event BEFORE INSERT ON domain_events
             WHEN NEW.type = 'resource.context_document.status.changed.v1'
               AND json_extract(NEW.payload_json, '$.to') = 'DELETION_PENDING'
             BEGIN SELECT RAISE(ABORT, 'injected purge event failure'); END;",
        ),
    ] {
        let fixture = fixture();
        let resource = create_context_resource(
            &fixture,
            &format!("resource-purge-rollback-{trigger_name}"),
            &format!("revision-purge-rollback-{trigger_name}"),
            b"rollback fixture retains local content and index",
        );
        Connection::open(state_database(&fixture.directory))
            .expect("open rollback trigger database")
            .execute_batch(trigger_sql)
            .expect("inject one deterministic transaction boundary failure");

        assert!(
            fixture
                .store
                .begin_context_document_purge(begin_command(
                    &resource.resource_id,
                    &format!("request-purge-rollback-{trigger_name}"),
                    2,
                    &format!("event-purge-rollback-{trigger_name}"),
                    "2026-10-15T10:00:03Z",
                ))
                .is_err(),
            "{trigger_name} aborts the complete purge transaction"
        );

        let connection =
            Connection::open(state_database(&fixture.directory)).expect("inspect rollback state");
        assert_eq!(
            scalar_count(
                &connection,
                "SELECT COUNT(*) FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
                &resource.resource_id
            ),
            0
        );
        assert_eq!(
            scalar_count(
                &connection,
                "SELECT COUNT(*) FROM context_document_purge_receipts WHERE workspace_id = ?1 AND resource_id = ?2",
                &resource.resource_id
            ),
            0
        );
        assert_eq!(
            scalar_count(
                &connection,
                "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_id = ?2 AND type = 'resource.context_document.status.changed.v1'",
                &resource.resource_id
            ),
            0
        );
        let (status, version): (String, i64) = connection
            .query_row(
                "SELECT json_extract(context_document_json, '$.status'), version FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
                params![WORKSPACE, resource.resource_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read restored owner state")
            ;
        assert_eq!((status.as_str(), version), ("ACTIVE", 2));
        assert_eq!(
            scalar_count(
                &connection,
                "SELECT COUNT(*) FROM resource_locations WHERE resource_id = ?2",
                &resource.resource_id
            ),
            1
        );
        assert_eq!(
            scalar_count(
                &connection,
                "SELECT COUNT(*) FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
                &resource.resource_id
            ),
            1
        );
        assert_eq!(
            scalar_count(
                &connection,
                "SELECT COUNT(*) FROM request_dedup WHERE principal_id = 'owner-local' AND request_id = ?2",
                &format!("request-purge-rollback-{trigger_name}")
            ),
            0
        );
        assert!(registry_exists(
            &fixture.directory,
            "resource_content_blob_registry",
            &resource.content_digest
        ));
        assert!(registry_exists(
            &fixture.directory,
            "resource_index_blob_registry",
            &resource.index_digest
        ));
    }
}

#[test]
fn purge_acknowledgement_rejects_shared_or_uncollected_registry_objects() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-ack-gated",
        "revision-purge-ack-gated",
        b"object cannot be acknowledged before physical collection",
    );
    let _committed = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-ack-gated",
        2,
        "event-purge-ack-gated",
        "2026-10-15T10:00:03Z",
    )
    .expect("begin purge and retain pending replica receipts");
    let pending = fixture
        .store
        .list_pending_context_document_purge_targets(100)
        .expect("list pending exact targets");
    assert_eq!(pending.len(), 2);

    for (index, target) in pending.iter().enumerate() {
        let mut forged_target = target.target.clone();
        forged_target.replica_ref.push_str("-forged");
        let forged_ack = fixture
            .store
            .acknowledge_context_document_purge_target_if_object_physically_removed(
                AcknowledgeContextDocumentPurgeTargetCommand {
                    workspace_id: target.workspace_id.clone(),
                    resource_id: target.resource_id.clone(),
                    manifest_digest: target.manifest_digest.clone(),
                    target: forged_target,
                    acknowledged_at: format!("2026-10-15T10:00:{:02}Z", index + 3),
                    event: event_context(
                        &format!("event-purge-ack-forged-{index}"),
                        &format!("2026-10-15T10:00:{:02}Z", index + 3),
                    ),
                    completion_event: event_context(
                        &format!("event-purge-complete-forged-{index}"),
                        &format!("2026-10-15T10:00:{:02}Z", index + 3),
                    ),
                },
            );
        assert!(
            !matches!(forged_ack, Ok(true)),
            "a target outside the immutable manifest is never acknowledged"
        );
        assert!(
            !fixture
                .store
                .acknowledge_context_document_purge_target_if_object_physically_removed(
                    AcknowledgeContextDocumentPurgeTargetCommand {
                        workspace_id: target.workspace_id.clone(),
                        resource_id: target.resource_id.clone(),
                        manifest_digest: target.manifest_digest.clone(),
                        target: target.target.clone(),
                        acknowledged_at: format!("2026-10-15T10:00:{:02}Z", index + 4),
                        event: event_context(
                            &format!("event-purge-ack-too-early-{index}"),
                            &format!("2026-10-15T10:00:{:02}Z", index + 4),
                        ),
                        completion_event: event_context(
                            &format!("event-purge-complete-too-early-{index}"),
                            &format!("2026-10-15T10:00:{:02}Z", index + 4),
                        ),
                    },
                )
                .expect("an intact registry object leaves its receipt pending")
        );
    }
    assert_eq!(
        fixture
            .store
            .get_context_document_deletion_status(WORKSPACE, &resource.resource_id, OWNER)
            .expect("read pending ack status")
            .unwrap()
            .acknowledged_replica_count,
        0
    );
    assert!(registry_exists(
        &fixture.directory,
        "resource_content_blob_registry",
        &resource.content_digest
    ));
    assert!(registry_exists(
        &fixture.directory,
        "resource_index_blob_registry",
        &resource.index_digest
    ));
}

#[test]
fn purge_expires_revision_upload_tracks_and_collects_its_chunk_before_deletion() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-upload-chunk",
        "revision-purge-upload-chunk",
        b"context document with a staged revision upload",
    );
    create_pinned_revision_upload(&fixture, &resource);
    let chunk_blob = purge_upload_chunk_blob();

    let committed = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-upload-chunk",
        2,
        "event-purge-upload-chunk",
        "2026-10-15T10:00:04Z",
    )
    .expect("purge admission seals a manifest around the Resource and staged chunk");
    assert_eq!(
        committed.status.status,
        ContextDocumentPurgeStatus::DeletionPending
    );

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect the Resource upload and sealed purge plan");
    let session_state: String = connection
        .query_row(
            "SELECT state FROM resource_upload_sessions WHERE upload_id = ?1",
            [PURGE_UPLOAD_ID],
            |row| row.get(0),
        )
        .expect("read the upload state after purge admission");
    let expiry_payload: Option<String> = connection
        .query_row(
            "SELECT payload_json FROM domain_events
             WHERE entity_type = 'ResourceUpload' AND entity_id = ?1
               AND type = 'resource.upload.status.changed.v1'
               AND json_extract(payload_json, '$.to') = 'EXPIRED'
             ORDER BY origin_sequence DESC LIMIT 1",
            [PURGE_UPLOAD_ID],
            |row| row.get(0),
        )
        .ok();
    let chunk_range: Option<(i64, i64, String, String)> = connection
        .query_row(
            "SELECT start_offset, end_offset_exclusive, sha256, temporary_blob_ref
             FROM resource_upload_chunks WHERE upload_id = ?1 AND chunk_index = 0",
            [PURGE_UPLOAD_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .ok();
    drop(connection);

    let expiry_payload =
        expiry_payload.and_then(|payload| serde_json::from_str::<Value>(&payload).ok());
    let upload_target_in_manifest = purge_manifest_has_exact_upload_chunk_target(
        &fixture,
        &resource.resource_id,
        &chunk_blob.digest,
    );
    let expiry_event_is_scoped_to_deletion = expiry_payload.as_ref().is_some_and(|payload| {
        payload["from"] == "CONTENT_RECEIVED"
            && payload["to"] == "EXPIRED"
            && payload["reason_code"] == "CONTEXT_DOCUMENT_DELETION"
    });
    let chunk_range_is_retained = chunk_range
        .as_ref()
        .is_some_and(|(start, end, digest, blob)| {
            *start == 0
                && *end == PURGE_UPLOAD_CONTENT.len() as i64
                && digest == &chunk_blob.digest
                && blob == &chunk_blob.digest
        });

    let chunks_removed = fixture
        .store
        .collect_orphan_chunks(GC_AT, 100)
        .expect("bounded upload-chunk GC sweep runs after purge admission");
    let chunk_object_remains = fixture
        .store
        .inner
        .blobs
        .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &chunk_blob)
        .is_ok();

    fixture
        .store
        .collect_orphan_resource_content_blobs(GC_AT, 100)
        .expect("collect the unreferenced managed Resource revision blob");
    fixture
        .store
        .collect_orphan_resource_index_blobs(GC_AT, 100)
        .expect("collect the unreferenced ResourceIndex snapshot");
    let pending = fixture
        .store
        .list_pending_context_document_purge_targets(100)
        .expect("load exact pending targets from the sealed plan");
    let upload_target = pending
        .iter()
        .find(|target| target.target.replica_kind.as_str() == "UPLOAD_CHUNK");
    let non_upload_targets: Vec<_> = pending
        .iter()
        .filter(|target| target.target.replica_kind.as_str() != "UPLOAD_CHUNK")
        .collect();
    let mut non_upload_targets_acknowledged = true;
    for (index, target) in non_upload_targets.iter().enumerate() {
        non_upload_targets_acknowledged &= acknowledge_purge_target(&fixture, target, index as u32);
    }
    let status_before_upload_target_ack = fixture
        .store
        .get_context_document_deletion_status(WORKSPACE, &resource.resource_id, OWNER)
        .expect("read deletion progress before acknowledging the upload object")
        .expect("ContextDocument deletion status remains available");
    let upload_target_acknowledged = upload_target.is_some_and(|target| {
        acknowledge_purge_target(&fixture, target, non_upload_targets.len() as u32)
    });
    let status_after_upload_target_ack = fixture
        .store
        .get_context_document_deletion_status(WORKSPACE, &resource.resource_id, OWNER)
        .expect("read deletion progress after exact upload target acknowledgement")
        .expect("ContextDocument deletion status remains available");

    let mut gaps = Vec::new();
    if session_state != "EXPIRED" {
        gaps.push(format!("purge left upload session in {session_state}"));
    }
    if !expiry_event_is_scoped_to_deletion {
        gaps.push("purge did not record EXPIRED with CONTEXT_DOCUMENT_DELETION".to_owned());
    }
    if !upload_target_in_manifest {
        gaps.push("sealed targets_json omits the exact digest as UPLOAD_CHUNK".to_owned());
    }
    if !chunk_range_is_retained {
        gaps.push("purge did not retain the accepted chunk range and digest metadata".to_owned());
    }
    if chunks_removed != 1 || chunk_object_remains {
        gaps.push(format!(
            "bounded chunk GC removed {chunks_removed} objects; object remains={chunk_object_remains}"
        ));
    }
    if !non_upload_targets_acknowledged {
        gaps.push(
            "one or more physically collected Resource targets were not acknowledged".to_owned(),
        );
    }
    if status_before_upload_target_ack.status != ContextDocumentPurgeStatus::DeletionPending {
        gaps.push(format!(
            "purge reached {:?} before the upload chunk target was acknowledged",
            status_before_upload_target_ack.status
        ));
    }
    if upload_target.is_none() || !upload_target_acknowledged {
        gaps.push("the exact UPLOAD_CHUNK target could not be acknowledged".to_owned());
    }
    if status_after_upload_target_ack.status != ContextDocumentPurgeStatus::Deleted {
        gaps.push(format!(
            "purge status after exact upload target acknowledgement is {:?}",
            status_after_upload_target_ack.status
        ));
    }
    assert!(
        gaps.is_empty(),
        "ContextDocument upload purge gaps: {gaps:#?}; expiry_event={expiry_payload:#?}"
    );
}

#[test]
fn purge_collects_linked_failed_upload_chunk_and_preserves_terminal_history() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-terminal-upload-chunk",
        "revision-purge-terminal-upload-chunk",
        b"context document linked to a terminal upload",
    );
    create_pinned_revision_upload(&fixture, &resource);
    let chunk_blob = purge_upload_chunk_blob();

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("open the accepted revision-upload session");
    let changed = connection
        .execute(
            "UPDATE resource_upload_sessions
             SET state = 'FAILED', version = version + 1
             WHERE upload_id = ?1 AND state = 'CONTENT_RECEIVED'",
            [PURGE_UPLOAD_ID],
        )
        .expect("transition the accepted upload to its valid terminal failure state");
    assert_eq!(changed, 1);
    drop(connection);

    let committed = begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-terminal-upload-chunk",
        2,
        "event-purge-terminal-upload-chunk",
        "2026-10-15T10:00:04Z",
    )
    .expect("purge seals objects still linked to the terminal upload");
    assert_eq!(
        committed.status.status,
        ContextDocumentPurgeStatus::DeletionPending
    );
    assert!(purge_manifest_has_exact_upload_chunk_target(
        &fixture,
        &resource.resource_id,
        &chunk_blob.digest,
    ));

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("inspect terminal upload after purge admission");
    let (session_state, retained_chunk_count, expiry_event_count): (String, i64, i64) = connection
        .query_row(
            "SELECT session.state,
                    (SELECT COUNT(*) FROM resource_upload_chunks chunk
                     WHERE chunk.upload_id = session.upload_id),
                    (SELECT COUNT(*) FROM domain_events event
                     WHERE event.entity_type = 'ResourceUpload'
                       AND event.entity_id = session.upload_id
                       AND event.type = 'resource.upload.status.changed.v1'
                       AND json_extract(event.payload_json, '$.to') = 'EXPIRED')
             FROM resource_upload_sessions session WHERE session.upload_id = ?1",
            [PURGE_UPLOAD_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read terminal state, immutable chunk history, and expiry events");
    drop(connection);
    assert_eq!(session_state, "FAILED");
    assert_eq!(
        retained_chunk_count, 1,
        "purge retains immutable chunk history"
    );
    assert_eq!(
        expiry_event_count, 0,
        "purge does not rewrite terminal upload state"
    );

    assert_eq!(
        fixture
            .store
            .collect_orphan_chunks(GC_AT, 100)
            .expect("collect the unreferenced terminal transfer object"),
        1
    );
    fixture
        .store
        .collect_orphan_resource_content_blobs(GC_AT, 100)
        .expect("collect the now-unreferenced Resource content object");
    fixture
        .store
        .collect_orphan_resource_index_blobs(GC_AT, 100)
        .expect("collect the now-unreferenced ResourceIndex object");
    assert!(matches!(
        fixture
            .store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &chunk_blob,),
        Err(StoreError::NotFound)
    ));
    assert!(!registry_exists(
        &fixture.directory,
        "resource_upload_chunk_blob_registry",
        &chunk_blob.digest,
    ));

    let pending = fixture
        .store
        .list_pending_context_document_purge_targets(100)
        .expect("load sealed targets for the terminal upload purge");
    let upload_target = pending
        .iter()
        .find(|target| target.target.replica_kind == ContextDocumentPurgeReplicaKind::UploadChunk)
        .expect("terminal chunk has an exact purge receipt");
    assert_eq!(
        upload_target.target.target_upload_chunk_refs,
        vec![storage_core::ContextDocumentPurgeUploadChunkRef {
            upload_id: PURGE_UPLOAD_ID.to_owned(),
            chunk_index: 0,
        }]
    );
    for (sequence, target) in pending.iter().enumerate() {
        assert!(acknowledge_purge_target(&fixture, target, sequence as u32));
    }
    let status = fixture
        .store
        .get_context_document_deletion_status(WORKSPACE, &resource.resource_id, OWNER)
        .expect("read completed purge status")
        .expect("purge status remains available");
    assert_eq!(status.status, ContextDocumentPurgeStatus::Deleted);

    let connection = Connection::open(state_database(&fixture.directory))
        .expect("verify terminal upload history remains after completed purge");
    let (session_state, retained_chunk_count): (String, i64) = connection
        .query_row(
            "SELECT session.state,
                    (SELECT COUNT(*) FROM resource_upload_chunks chunk
                     WHERE chunk.upload_id = session.upload_id)
             FROM resource_upload_sessions session WHERE session.upload_id = ?1",
            [PURGE_UPLOAD_ID],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("terminal upload session and chunk ledger survive deletion completion");
    assert_eq!(session_state, "FAILED");
    assert_eq!(retained_chunk_count, 1);
}

#[test]
fn purged_revision_upload_chunk_retry_does_not_recreate_the_chunk_object() {
    let fixture = fixture();
    let resource = create_context_resource(
        &fixture,
        "resource-purge-upload-retry",
        "revision-purge-upload-retry",
        b"context document whose staged upload is purged",
    );
    create_pinned_revision_upload(&fixture, &resource);
    let chunk_blob = purge_upload_chunk_blob();
    begin_purge(
        &fixture,
        &resource.resource_id,
        "request-purge-upload-retry",
        2,
        "event-purge-upload-retry",
        "2026-10-15T10:00:04Z",
    )
    .expect("purge admission tombstones the ContextDocument");
    let upload_target_in_manifest = purge_manifest_has_exact_upload_chunk_target(
        &fixture,
        &resource.resource_id,
        &chunk_blob.digest,
    );

    fixture
        .store
        .inner
        .blobs
        .remove(WORKSPACE, BlobPurpose::ResourceUploadChunk, &chunk_blob)
        .expect("simulate the physical chunk removal performed by purge GC");
    let retry = fixture.store.put_chunk(ResourceUploadChunkInput {
        upload_id: PURGE_UPLOAD_ID.to_owned(),
        chunk_index: 0,
        request_id: "request-context-purge-upload-chunk".to_owned(),
        content_range: ResourceUploadContentRange {
            start_offset: 0,
            end_offset_inclusive: PURGE_UPLOAD_CONTENT.len() as u64 - 1,
            total_size_bytes: PURGE_UPLOAD_CONTENT.len() as u64,
        },
        sha256: chunk_blob.digest.clone(),
        content: PURGE_UPLOAD_CONTENT.to_vec(),
        received_at: "2026-10-15T10:00:05Z".to_owned(),
        lifecycle_event: EventDraft {
            event_id: "event-context-purge-upload-chunk-retry".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: PURGE_UPLOAD_ID.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 1,
            hlc_timestamp: "2026-10-15T10:00:05Z".to_owned(),
            correlation_id: "correlation-context-purge-upload-chunk-retry".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.status.changed.v1".to_owned(),
            payload: json!({}),
            recorded_at: "2026-10-15T10:00:05Z".to_owned(),
        },
    });
    let object_was_recreated = fixture
        .store
        .inner
        .blobs
        .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &chunk_blob)
        .is_ok();

    assert!(
        upload_target_in_manifest && retry.is_err() && !object_was_recreated,
        "a purge must target the exact upload_id/chunk_index with no Resource revision IDs, then reject retries without recreating the chunk object; target exact={}, retry rejected={}, object recreated={object_was_recreated}",
        upload_target_in_manifest,
        retry.is_err(),
    );
}
