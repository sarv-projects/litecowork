use super::*;
use rusqlite::{Connection, params};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex, mpsc::SyncSender};
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
    second_remove_entered: Option<SyncSender<()>>,
}

#[derive(Clone)]
struct ResourceContentGetPause {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    armed: Arc<AtomicBool>,
    first_only: Arc<AtomicBool>,
}

type ResourceContentPutPause = ResourceContentGetPause;

#[derive(Clone)]
struct AggregateStatePutPause {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    armed: Arc<AtomicBool>,
    first_only: Arc<AtomicBool>,
}

struct TestUploadBlobStore {
    inner: FileBlobStore<TestKeys>,
    chunk_put_pause: Option<ChunkPutPause>,
    gc_remove_pause: Option<GcRemovePause>,
    aggregate_state_put_pause: Option<AggregateStatePutPause>,
    aggregate_state_puts: Arc<AtomicUsize>,
    fail_next_remove: Arc<AtomicBool>,
    fail_next_resource_remove: Arc<AtomicBool>,
    fail_after_resource_remove: Arc<AtomicBool>,
    resource_remove_calls: Arc<AtomicUsize>,
    resource_content_put_pause: Option<ResourceContentPutPause>,
    resource_content_get_pause: Option<ResourceContentGetPause>,
}

struct ConcurrentIndexPutGate {
    armed: AtomicBool,
    calls: AtomicUsize,
    first_entered: SyncSender<()>,
    second_entered: SyncSender<()>,
    first_release: Barrier,
    second_release: Barrier,
}

struct PausedIndexPutBlobStore {
    inner: Arc<dyn BlobStore>,
    gate: Arc<ConcurrentIndexPutGate>,
}

#[derive(Clone)]
struct ResourceIndexBlobPutPause {
    armed: Arc<AtomicBool>,
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    first_only: Arc<AtomicBool>,
    digests: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone)]
struct ResourceIndexTokenPause {
    armed: Arc<AtomicBool>,
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    first_only: Arc<AtomicBool>,
}

#[derive(Clone)]
struct ResourceIndexBlobGetPause {
    armed: Arc<AtomicBool>,
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    first_only: Arc<AtomicBool>,
}

struct PausedResourceIndexBlobStore {
    inner: Arc<dyn BlobStore>,
    pause: ResourceIndexBlobPutPause,
    get_pause: Option<ResourceIndexBlobGetPause>,
    token_pause: Option<ResourceIndexTokenPause>,
}

struct ResourceIndexReservationObserver {
    inner: Arc<dyn BlobStore>,
    database: PathBuf,
    saw_reserved_index_put: Arc<Mutex<Vec<bool>>>,
}

struct ResourceContentReservationObserver {
    inner: Arc<dyn BlobStore>,
    database: PathBuf,
    saw_reserved_resource_put: Arc<Mutex<Vec<bool>>>,
}

impl BlobStore for ResourceIndexReservationObserver {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
        if purpose == BlobPurpose::ResourceIndex {
            let connection = Connection::open(&self.database).map_err(map_database_error)?;
            let table_exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'resource_index_blob_registry')",
                    [],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            let reserved = if table_exists {
                connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM resource_index_blob_registry WHERE workspace_id = ?1 AND digest = ?2 AND size_bytes = ?3 AND state = 'RESERVED')",
                        params![workspace_id, digest(bytes), bytes.len() as i64],
                        |row| row.get(0),
                    )
                    .map_err(map_database_error)?
            } else {
                false
            };
            self.saw_reserved_index_put
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(reserved);
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

impl BlobStore for ResourceContentReservationObserver {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
        if purpose == BlobPurpose::Resource {
            let connection = Connection::open(&self.database).map_err(map_database_error)?;
            let table_exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'resource_content_blob_registry')",
                    [],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            let reserved = if table_exists {
                connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM resource_content_blob_registry WHERE workspace_id = ?1 AND digest = ?2 AND size_bytes = ?3 AND media_type = ?4 AND state = 'RESERVED')",
                        params![workspace_id, digest(bytes), bytes.len() as i64, media_type],
                        |row| row.get(0),
                    )
                    .map_err(map_database_error)?
            } else {
                false
            };
            self.saw_reserved_resource_put
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(reserved);
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

impl BlobStore for PausedResourceIndexBlobStore {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
        let blob = self.inner.put(workspace_id, purpose, bytes, media_type)?;
        if purpose == BlobPurpose::ResourceIndex {
            self.pause
                .digests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(blob.digest.clone());
            if self.pause.armed.load(Ordering::SeqCst)
                && self.pause.first_only.swap(false, Ordering::SeqCst)
            {
                self.pause.entered.wait();
                self.pause.release.wait();
            }
        }
        Ok(blob)
    }

    fn get(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<Vec<u8>, StoreError> {
        if purpose == BlobPurpose::ResourceIndex
            && let Some(pause) = &self.get_pause
            && pause.armed.load(Ordering::SeqCst)
            && pause.first_only.swap(false, Ordering::SeqCst)
        {
            pause.entered.wait();
            pause.release.wait();
        }
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
        if key_version == Some(1)
            && normalized_terms.iter().any(|term| term == "needle")
            && let Some(pause) = &self.token_pause
            && pause.armed.load(Ordering::SeqCst)
            && pause.first_only.swap(false, Ordering::SeqCst)
        {
            pause.entered.wait();
            pause.release.wait();
        }
        self.inner
            .resource_index_tokens(workspace_id, key_version, normalized_terms)
    }
}

impl BlobStore for PausedIndexPutBlobStore {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
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
        self.inner.remove(workspace_id, purpose, blob)
    }

    fn resource_index_token(
        &self,
        workspace_id: &str,
        key_version: Option<u32>,
        normalized_term: &str,
    ) -> Result<(u32, String), StoreError> {
        let result = self
            .inner
            .resource_index_token(workspace_id, key_version, normalized_term)?;
        if key_version.is_none()
            && normalized_term == "__lc_current_key_availability_check_v1"
            && self.gate.armed.load(Ordering::SeqCst)
        {
            match self.gate.calls.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    self.gate.first_entered.send(()).map_err(|_| {
                        StoreError::Blob("first index writer disappeared".to_owned())
                    })?;
                    self.gate.first_release.wait();
                }
                1 => {
                    self.gate.second_entered.send(()).map_err(|_| {
                        StoreError::Blob("second index writer disappeared".to_owned())
                    })?;
                    self.gate.second_release.wait();
                }
                _ => {}
            }
        }
        Ok(result)
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

impl BlobStore for TestUploadBlobStore {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
        if purpose == BlobPurpose::AggregateState {
            self.aggregate_state_puts.fetch_add(1, Ordering::SeqCst);
            if let Some(pause) = &self.aggregate_state_put_pause
                && pause.armed.load(Ordering::SeqCst)
                && pause.first_only.swap(false, Ordering::SeqCst)
            {
                pause.entered.wait();
                pause.release.wait();
            }
        }
        if purpose == BlobPurpose::ResourceUploadChunk
            && let Some(pause) = &self.chunk_put_pause
            && pause.first_only.swap(false, Ordering::SeqCst)
        {
            pause.entered.wait();
            pause.release.wait();
        }
        if purpose == BlobPurpose::Resource
            && let Some(pause) = &self.resource_content_put_pause
            && pause.armed.load(Ordering::SeqCst)
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
        if purpose == BlobPurpose::Resource
            && let Some(pause) = &self.resource_content_get_pause
            && pause.armed.load(Ordering::SeqCst)
            && pause.first_only.swap(false, Ordering::SeqCst)
        {
            pause.entered.wait();
            pause.release.wait();
        }
        self.inner.get(workspace_id, purpose, blob)
    }

    fn remove(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<(), StoreError> {
        if purpose == BlobPurpose::Resource {
            self.resource_remove_calls.fetch_add(1, Ordering::SeqCst);
        }
        if purpose == BlobPurpose::Resource
            && self.fail_next_resource_remove.swap(false, Ordering::SeqCst)
        {
            return Err(StoreError::Blob(
                "injected Resource content removal failure".to_owned(),
            ));
        }
        if self.fail_next_remove.swap(false, Ordering::SeqCst) {
            return Err(StoreError::Blob("injected blob removal failure".to_owned()));
        }
        if purpose == BlobPurpose::ResourceUploadChunk
            && let Some(pause) = &self.gc_remove_pause
            && !pause.first_only.load(Ordering::SeqCst)
            && let Some(second_remove_entered) = &pause.second_remove_entered
        {
            second_remove_entered.send(()).map_err(|_| {
                StoreError::Blob("second upload GC remove observer disappeared".to_owned())
            })?;
        }
        self.inner.remove(workspace_id, purpose, blob)?;
        if purpose == BlobPurpose::Resource
            && self
                .fail_after_resource_remove
                .swap(false, Ordering::SeqCst)
        {
            return Err(StoreError::Blob(
                "injected ambiguous Resource content removal acknowledgement".to_owned(),
            ));
        }
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

struct ResourceIndexUnavailableKeys {
    index_lookups: Arc<AtomicUsize>,
}

struct FailNthResourceIndexKeyLookup {
    index_lookups: Arc<AtomicUsize>,
    fail_on: usize,
}

impl WorkspaceBlobKeyProvider for FailNthResourceIndexKeyLookup {
    fn current_key(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        self.maybe_fail(purpose)?;
        TestKeys { key: [23_u8; 32] }.current_key(workspace_id, purpose)
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        self.maybe_fail(purpose)?;
        TestKeys { key: [23_u8; 32] }.key_by_version(workspace_id, purpose, version)
    }
}

impl FailNthResourceIndexKeyLookup {
    fn maybe_fail(&self, purpose: BlobPurpose) -> Result<(), StoreError> {
        if purpose == BlobPurpose::ResourceIndex
            && self.index_lookups.fetch_add(1, Ordering::SeqCst) + 1 == self.fail_on
        {
            return Err(StoreError::Blob(
                "injected later Resource index key failure".to_owned(),
            ));
        }
        Ok(())
    }
}

struct RotatingResourceIndexKeys {
    current_index_version: Arc<AtomicU32>,
}

impl RotatingResourceIndexKeys {
    fn key(version: u32) -> WorkspaceBlobKey {
        let mut bytes = [0_u8; 32];
        bytes[..4].copy_from_slice(&version.to_be_bytes());
        bytes[4] = 0xa7;
        WorkspaceBlobKey {
            version,
            bytes: Zeroizing::new(bytes),
        }
    }
}

impl WorkspaceBlobKeyProvider for RotatingResourceIndexKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        let version = if purpose == BlobPurpose::ResourceIndex {
            self.current_index_version.load(Ordering::SeqCst)
        } else {
            1
        };
        Ok(Self::key(version))
    }

    fn key_by_version(
        &self,
        _workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        let latest = if purpose == BlobPurpose::ResourceIndex {
            self.current_index_version.load(Ordering::SeqCst)
        } else {
            1
        };
        if version == 0 || version > latest {
            return Err(StoreError::Blob("unknown test key version".to_owned()));
        }
        Ok(Self::key(version))
    }
}

impl WorkspaceBlobKeyProvider for ResourceIndexUnavailableKeys {
    fn current_key(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if purpose == BlobPurpose::ResourceIndex {
            self.index_lookups.fetch_add(1, Ordering::SeqCst);
            return Err(StoreError::Blob(
                "test Resource index key unavailable".to_owned(),
            ));
        }
        TestKeys { key: [23_u8; 32] }.current_key(workspace_id, purpose)
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if purpose == BlobPurpose::ResourceIndex {
            self.index_lookups.fetch_add(1, Ordering::SeqCst);
            return Err(StoreError::Blob(
                "test Resource index key unavailable".to_owned(),
            ));
        }
        TestKeys { key: [23_u8; 32] }.key_by_version(workspace_id, purpose, version)
    }
}

fn store_without_resource_index_keys(
    directory: &tempfile::TempDir,
    index_lookups: Arc<AtomicUsize>,
) -> SqliteWorkspaceStore {
    open_store(
        directory,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            ResourceIndexUnavailableKeys { index_lookups },
        )),
    )
}

fn create_resource_with_content(
    store: &SqliteWorkspaceStore,
    resource_id: &str,
    display_name: &str,
    media_type: &str,
    content: &[u8],
) -> Result<storage_core::CommittedResource, StoreError> {
    create_resource_with_content_request(
        store,
        &format!("request-resource-create-{resource_id}"),
        resource_id,
        display_name,
        media_type,
        content,
    )
}

fn create_resource_with_content_request(
    store: &SqliteWorkspaceStore,
    request_id: &str,
    resource_id: &str,
    display_name: &str,
    media_type: &str,
    content: &[u8],
) -> Result<storage_core::CommittedResource, StoreError> {
    create_resource_with_optional_content_media_type_and_request(
        store,
        request_id,
        resource_id,
        display_name,
        Some(media_type),
        content,
    )
}

fn create_resource_with_optional_content_media_type(
    store: &SqliteWorkspaceStore,
    resource_id: &str,
    display_name: &str,
    media_type: Option<&str>,
    content: &[u8],
) -> Result<storage_core::CommittedResource, StoreError> {
    create_resource_with_optional_content_media_type_and_request(
        store,
        &format!("request-resource-create-{resource_id}"),
        resource_id,
        display_name,
        media_type,
        content,
    )
}

fn create_resource_with_optional_content_media_type_and_request(
    store: &SqliteWorkspaceStore,
    request_id: &str,
    resource_id: &str,
    display_name: &str,
    media_type: Option<&str>,
    content: &[u8],
) -> Result<storage_core::CommittedResource, StoreError> {
    let revision_id = format!("revision-{resource_id}");
    let content_digest = digest(content);
    let resource = ResourceRecord {
        resource_id: resource_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        kind: "FILE".to_owned(),
        provider_identity: json!({}),
        identity_digest: None,
        display_name: display_name.to_owned(),
        current_revision_id: Some(revision_id.clone()),
        sensitivity: "PERSONAL".to_owned(),
        provenance: json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
        created_at: CREATED.to_owned(),
        updated_at: CREATED.to_owned(),
        version: 1,
    };
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id,
        resource_id: resource_id.to_owned(),
        parent_revision_ids: Vec::new(),
        provider_revision: None,
        content_digest: Some(content_digest.clone()),
        size_bytes: Some(content.len() as u64),
        media_type: media_type.map(str::to_owned),
        observed_at: CREATED.to_owned(),
        created_by: json!({"kind": "USER", "principal_id": OWNER}),
    };
    let event_id = format!("event-resource-create-{resource_id}");
    let event = EventDraft {
        event_id: event_id.clone(),
        workspace_id: WORKSPACE.to_owned(),
        entity_type: "Resource".to_owned(),
        entity_id: resource_id.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: 1,
        hlc_timestamp: CREATED.to_owned(),
        correlation_id: event_id.clone(),
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
        recorded_at: CREATED.to_owned(),
    };
    store.create_resource(
        WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: request_id.to_owned(),
            request_payload: json!({
                "operation": "resource.quick_import.v1",
                "workspace_id": WORKSPACE,
                "display_name": display_name,
                "media_type": media_type.unwrap_or("application/octet-stream"),
                "content_digest": content_digest,
                "size_bytes": content.len(),
            }),
        },
        resource,
        revision,
        event,
        content.to_vec(),
    )
}

fn create_upload_with_content(
    store: &SqliteWorkspaceStore,
    upload_id: &str,
    display_name: &str,
    media_type: &str,
    content: &[u8],
) -> ResourceUploadSessionRecord {
    let content_digest = digest(content);
    let event_id = format!("event-create-{upload_id}");
    store
        .create(
            WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: format!("request-create-{upload_id}"),
                request_payload: json!({
                    "operation": "resource.upload.create.v1",
                    "workspace_id": WORKSPACE,
                    "upload_id": upload_id,
                    "expected_digest": content_digest,
                }),
            },
            ResourceUploadSessionRecord {
                upload_id: upload_id.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                display_name: display_name.to_owned(),
                media_type: media_type.to_owned(),
                expected_size_bytes: content.len() as u64,
                expected_digest: Some(content_digest.clone()),
                context_document: None,
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
            },
            EventDraft {
                event_id: event_id.clone(),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "ResourceUpload".to_owned(),
                entity_id: upload_id.to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                entity_revision: 1,
                hlc_timestamp: CREATED.to_owned(),
                correlation_id: event_id,
                causation_id: None,
                schema_version: 1,
                event_type: "resource.upload.created.v1".to_owned(),
                payload: json!({
                    "upload_id": upload_id,
                    "workspace_id": WORKSPACE,
                    "expected_size_bytes": content.len(),
                    "expected_digest": content_digest,
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

fn commit_upload_with_content(
    store: &SqliteWorkspaceStore,
    upload_id: &str,
    resource_id: &str,
    display_name: &str,
    media_type: &str,
    content: &[u8],
) -> Result<storage_core::CommittedResourceUpload, StoreError> {
    commit_upload_with_content_and_failure_event(
        store,
        upload_id,
        resource_id,
        display_name,
        media_type,
        content,
        None,
    )
}

fn commit_upload_with_content_and_failure_event(
    store: &SqliteWorkspaceStore,
    upload_id: &str,
    resource_id: &str,
    display_name: &str,
    media_type: &str,
    content: &[u8],
    upload_failure_event: Option<EventDraft>,
) -> Result<storage_core::CommittedResourceUpload, StoreError> {
    let revision_id = format!("revision-{resource_id}");
    let content_digest = digest(content);
    let resource = ResourceRecord {
        resource_id: resource_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        kind: "FILE".to_owned(),
        provider_identity: json!({}),
        identity_digest: None,
        display_name: display_name.to_owned(),
        current_revision_id: Some(revision_id.clone()),
        sensitivity: "PERSONAL".to_owned(),
        provenance: json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
        created_at: CREATED.to_owned(),
        updated_at: CREATED.to_owned(),
        version: 1,
    };
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id,
        resource_id: resource_id.to_owned(),
        parent_revision_ids: Vec::new(),
        provider_revision: None,
        content_digest: Some(content_digest),
        size_bytes: Some(content.len() as u64),
        media_type: Some(media_type.to_owned()),
        observed_at: CREATED.to_owned(),
        created_by: json!({"kind": "USER", "principal_id": OWNER}),
    };
    let event_id = format!("event-resource-create-{resource_id}");
    store.commit(
        WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: format!("request-commit-{upload_id}"),
            request_payload: json!({}),
        },
        upload_id,
        resource.clone(),
        revision,
        EventDraft {
            event_id: event_id.clone(),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "Resource".to_owned(),
            entity_id: resource_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 1,
            hlc_timestamp: CREATED.to_owned(),
            correlation_id: event_id.clone(),
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
            recorded_at: CREATED.to_owned(),
        },
        Some(EventDraft {
            event_id: format!("event-upload-commit-{upload_id}"),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: upload_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 3,
            hlc_timestamp: CREATED.to_owned(),
            correlation_id: format!("correlation-upload-commit-{upload_id}"),
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
            recorded_at: CREATED.to_owned(),
        }),
        upload_failure_event,
    )
}

fn chunk_input_with_content(
    upload_id: &str,
    request_id: &str,
    content: &[u8],
) -> ResourceUploadChunkInput {
    let mut chunk = chunk_input(upload_id, request_id);
    chunk.content_range.end_offset_inclusive = content.len() as u64 - 1;
    chunk.content_range.total_size_bytes = content.len() as u64;
    chunk.sha256 = digest(content);
    chunk.content = content.to_vec();
    chunk
}

fn resource_index_projection_counts(
    directory: &tempfile::TempDir,
    resource_id: &str,
) -> (i64, i64) {
    let connection = Connection::open(state_database(directory)).expect("open SQLite");
    let index_count = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource_id],
            |row| row.get(0),
        )
        .expect("count Resource index snapshots");
    let term_count = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_text_index_terms WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, resource_id],
            |row| row.get(0),
        )
        .expect("count Resource index terms");
    (index_count, term_count)
}

#[test]
fn resource_index_blob_has_a_durable_reservation_before_publication() {
    let directory = tempfile::tempdir().expect("temporary ResourceIndex reservation directory");
    let saw_reserved_index_put = Arc::new(Mutex::new(Vec::new()));
    let blobs: Arc<dyn BlobStore> = Arc::new(ResourceIndexReservationObserver {
        inner: file_blob_store(&directory),
        database: state_database(&directory),
        saw_reserved_index_put: Arc::clone(&saw_reserved_index_put),
    });
    let store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);

    create_resource_with_content(
        &store,
        "resource-index-reservation-order",
        "notes.txt",
        "text/plain",
        b"durable reservation before encrypted publication",
    )
    .expect("Resource creation publishes an encrypted text index");

    assert_eq!(
        *saw_reserved_index_put
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        [true],
        "the exact Workspace/digest/size must be durably reserved before BlobStore::put"
    );
}

#[test]
fn resource_content_blob_has_a_durable_reservation_before_publication() {
    let directory = tempfile::tempdir().expect("temporary Resource content reservation directory");
    let saw_reserved_resource_put = Arc::new(Mutex::new(Vec::new()));
    let store = open_store(
        &directory,
        Arc::new(ResourceContentReservationObserver {
            inner: file_blob_store(&directory),
            database: state_database(&directory),
            saw_reserved_resource_put: Arc::clone(&saw_reserved_resource_put),
        }),
    );
    create_workspace_for_uploads(&store);

    create_resource_with_content(
        &store,
        "resource-content-reservation-direct",
        "direct.txt",
        "text/plain",
        b"direct Resource content is reserved before publication",
    )
    .expect("direct Resource creation publishes content");

    const UPLOAD_CONTENT: &[u8] = b"uploaded Resource content is reserved before publication";
    create_upload_with_content(
        &store,
        "upload-content-reservation",
        "uploaded.txt",
        "text/plain",
        UPLOAD_CONTENT,
    );
    store
        .put_chunk(chunk_input_with_content(
            "upload-content-reservation",
            "request-content-reservation-chunk",
            UPLOAD_CONTENT,
        ))
        .expect("upload reaches CONTENT_RECEIVED");
    commit_upload_with_content(
        &store,
        "upload-content-reservation",
        "resource-content-reservation-upload",
        "uploaded.txt",
        "text/plain",
        UPLOAD_CONTENT,
    )
    .expect("uploaded Resource content publishes");

    assert_eq!(
        *saw_reserved_resource_put
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        [true, true],
        "both direct and upload Resource writers must durably reserve the exact blob before BlobStore::put"
    );
}

#[test]
fn final_chunk_receipt_replays_after_content_received_and_commit_without_new_writes() {
    const UPLOAD_ID: &str = "upload-final-chunk-terminal-replay";
    const REQUEST_ID: &str = "request-final-chunk-terminal-replay";
    const RESOURCE_ID: &str = "resource-final-chunk-terminal-replay";
    const CONTENT: &[u8] = b"exact final chunk stays idempotent after commit";
    let directory = tempfile::tempdir().expect("temporary final-chunk replay directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_upload_with_content(&store, UPLOAD_ID, "terminal.txt", "text/plain", CONTENT);

    let accepted = store
        .put_chunk(chunk_input_with_content(UPLOAD_ID, REQUEST_ID, CONTENT))
        .expect("final chunk advances the upload to CONTENT_RECEIVED");
    assert_eq!(accepted.state, ResourceUploadState::ContentReceived);
    let received_replay = store
        .put_chunk(chunk_input_with_content(UPLOAD_ID, REQUEST_ID, CONTENT))
        .expect("exact final-chunk receipt replays at CONTENT_RECEIVED");
    assert_eq!(received_replay.state, ResourceUploadState::ContentReceived);
    assert_eq!(received_replay.progress_version, accepted.progress_version);

    let committed = commit_upload_with_content(
        &store,
        UPLOAD_ID,
        RESOURCE_ID,
        "terminal.txt",
        "text/plain",
        CONTENT,
    )
    .expect("verified upload commits");
    assert_eq!(committed.session.state, ResourceUploadState::Committed);
    let blob_count = blob_object_count(&directory);
    let committed_replay = store
        .put_chunk(chunk_input_with_content(UPLOAD_ID, REQUEST_ID, CONTENT))
        .expect("exact final-chunk receipt replays at COMMITTED");
    assert_eq!(committed_replay.state, ResourceUploadState::Committed);
    assert_eq!(committed_replay.version, committed.session.version);
    assert_eq!(
        committed_replay.progress_version,
        committed.session.progress_version
    );
    assert_eq!(blob_object_count(&directory), blob_count);

    let connection = Connection::open(state_database(&directory)).expect("inspect replay ledger");
    let request_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunk_requests
             WHERE upload_id = ?1 AND request_id = ?2",
            params![UPLOAD_ID, REQUEST_ID],
            |row| row.get(0),
        )
        .expect("count the original request receipt");
    let lifecycle_event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events
             WHERE entity_type = 'ResourceUpload' AND entity_id = ?1
               AND type = 'resource.upload.status.changed.v1'",
            [UPLOAD_ID],
            |row| row.get(0),
        )
        .expect("count upload lifecycle events");
    assert_eq!(request_count, 1);
    assert_eq!(
        lifecycle_event_count, 2,
        "chunk/commit status transitions remain single-write"
    );
}

#[test]
fn direct_resource_create_defaults_missing_media_type_before_tracking_and_readback() {
    const CONTENT: &[u8] = b"Resource bytes with no authored MIME metadata";
    const RESOURCE_ID: &str = "resource-content-default-media-type";
    let directory = tempfile::tempdir().expect("temporary Resource media type default directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);

    create_resource_with_optional_content_media_type(
        &store,
        RESOURCE_ID,
        "unknown-content.bin",
        None,
        CONTENT,
    )
    .expect("missing authored media type uses the canonical binary default");

    let revisions = store
        .list_resource_revisions_page(WORKSPACE, RESOURCE_ID, None, 10)
        .expect("read the committed revision");
    assert_eq!(revisions.len(), 1);
    assert_eq!(
        revisions[0].revision.media_type.as_deref(),
        Some("application/octet-stream"),
        "persisted revision metadata must match the media type used to identify its blob"
    );
    let content = store
        .read_resource_content(WORKSPACE, RESOURCE_ID)
        .expect("read Resource content")
        .expect("Resource exists");
    assert_eq!(content.summary.media_type, "application/octet-stream");
    assert_eq!(content.content, CONTENT);
}

#[test]
fn direct_resource_create_replay_and_metadata_conflict_skip_blob_preparation() {
    const REQUEST_ID: &str = "request-resource-create-replay-preflight";
    const CONTENT: &[u8] = b"same bytes under different Resource metadata";
    let directory = tempfile::tempdir().expect("temporary Resource replay directory");
    let (blobs, aggregate_state_puts) = observed_blob_store(&directory, None);
    let store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    let initial_state_puts = aggregate_state_puts.load(Ordering::SeqCst);

    let created = create_resource_with_content_request(
        &store,
        REQUEST_ID,
        "resource-create-replay-original",
        "notes.txt",
        "text/plain",
        CONTENT,
    )
    .expect("first Resource create commits");
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        initial_state_puts + 1,
        "first Resource create writes one aggregate snapshot"
    );

    let replay = create_resource_with_content_request(
        &store,
        REQUEST_ID,
        "resource-create-replay-candidate",
        "notes.txt",
        "text/plain",
        CONTENT,
    )
    .expect("exact Resource create replay returns its receipt");
    assert_eq!(replay.resource.resource_id, created.resource.resource_id);
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        initial_state_puts + 1,
        "exact replay must not persist a candidate Resource aggregate snapshot"
    );

    assert!(matches!(
        create_resource_with_content_request(
            &store,
            REQUEST_ID,
            "resource-create-media-conflict-candidate",
            "notes.txt",
            "text/markdown",
            CONTENT,
        ),
        Err(StoreError::Conflict {
            expected: None,
            actual: None
        })
    ));
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        initial_state_puts + 1,
        "changed metadata must conflict before writing candidate state or mutating the registry"
    );
}

#[test]
fn concurrent_direct_resource_create_replay_across_open_stores_serializes_blob_write() {
    const REQUEST_ID: &str = "request-resource-create-concurrent-replay";
    const CONTENT: &[u8] = b"concurrent direct Resource create retry";
    let directory = tempfile::tempdir().expect("temporary concurrent Resource create directory");
    let pause = AggregateStatePutPause {
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        armed: Arc::new(AtomicBool::new(false)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let (blobs, aggregate_state_puts) = observed_blob_store(&directory, Some(pause.clone()));
    let store = open_store(&directory, Arc::clone(&blobs));
    let second_store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    let initial_state_puts = aggregate_state_puts.load(Ordering::SeqCst);
    let first_request_lock = store.create_request_lock(OWNER, REQUEST_ID);
    let second_request_lock = second_store.create_request_lock(OWNER, REQUEST_ID);
    assert!(
        Arc::ptr_eq(&first_request_lock, &second_request_lock),
        "separately opened stores for the same database must share the process-local request lock"
    );
    pause.armed.store(true, Ordering::SeqCst);

    let first_store = store.clone();
    let first = std::thread::spawn(move || {
        create_resource_with_content_request(
            &first_store,
            REQUEST_ID,
            "resource-create-concurrent-first",
            "concurrent.txt",
            "text/plain",
            CONTENT,
        )
    });
    pause.entered.wait();

    let (second_started_sender, second_started_receiver) = std::sync::mpsc::channel();
    let (second_result_sender, second_result_receiver) = std::sync::mpsc::channel();
    let second = std::thread::spawn(move || {
        let _ = second_started_sender.send(());
        let result = create_resource_with_content_request(
            &second_store,
            REQUEST_ID,
            "resource-create-concurrent-second",
            "concurrent.txt",
            "text/plain",
            CONTENT,
        );
        let _ = second_result_sender.send(result);
    });
    second_started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("second exact Resource create retry starts");
    let completed_before_first_blob_release = second_result_receiver
        .recv_timeout(Duration::from_millis(250))
        .ok();
    let second_finished_early = completed_before_first_blob_release.is_some();
    pause.release.wait();

    let first_result = first.join().expect("first Resource create thread joins");
    let second_result = match completed_before_first_blob_release {
        Some(result) => result,
        None => second_result_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("serialized second Resource create completes after the first receipt commits"),
    };
    let first_committed = first_result.expect("first Resource create commits");
    let second_committed = second_result.expect("second exact request replays first Resource");
    second.join().expect("second Resource create thread joins");

    assert!(
        !second_finished_early,
        "same-key Resource create must not pass preflight or write blobs while the first request is unresolved"
    );
    assert_eq!(
        first_committed, second_committed,
        "both handles must return the exact same committed Resource receipt"
    );
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        initial_state_puts + 1,
        "concurrent exact retries across open stores write only the winning aggregate snapshot"
    );
}

fn resource_content_blob_ref(bytes: &[u8], media_type: &str) -> BlobRef {
    BlobRef {
        digest: digest(bytes),
        size_bytes: bytes.len() as u64,
        media_type: media_type.to_owned(),
    }
}

fn resource_content_registry_state(
    directory: &tempfile::TempDir,
    blob: &BlobRef,
) -> Option<String> {
    Connection::open(state_database(directory))
        .expect("open Resource content registry")
        .query_row(
            "SELECT state FROM resource_content_blob_registry WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, blob.digest],
            |row| row.get(0),
        )
        .optional()
        .expect("read Resource content registry state")
}

fn downgrade_resource_content_schema_to_v16(connection: &Connection) {
    downgrade_resource_index_schema_to_v18(connection);
    downgrade_upload_chunk_schema_to_v17(connection);
    connection
        .execute_batch(
            "DROP TRIGGER resource_content_blob_registry_reference_delete_guard;
             DROP TRIGGER resource_content_blob_registry_identity_update_guard;
             DROP TRIGGER resource_content_blob_registry_revision_update_guard;
             DROP TRIGGER resource_content_blob_registry_head_update_guard;
             DROP TRIGGER resource_content_blob_registry_location_update_guard;
             DROP TRIGGER resource_content_blob_registry_location_insert_guard;
             DROP TRIGGER resource_content_blob_registry_revision_insert_guard;
             DROP INDEX resource_content_blob_registry_gc;
             DROP TABLE resource_content_blob_registry;
             DELETE FROM schema_migrations WHERE version = 17;
             PRAGMA user_version = 16;",
        )
        .expect("restore the v16 migration boundary");
}

fn downgrade_resource_index_schema_to_v18(connection: &Connection) {
    connection
        .execute_batch(
            "DROP TRIGGER IF EXISTS workspace_root_observations_delete_private_binding;
             DROP TABLE IF EXISTS workspace_root_observation_batch_receipts;
             DROP TABLE IF EXISTS workspace_root_observation_bindings;
             DROP TABLE IF EXISTS workspace_root_observations;
             DROP TABLE IF EXISTS workspace_root_observation_generations;
             DELETE FROM schema_migrations WHERE version = 21;
             PRAGMA user_version = 20;
             DROP TRIGGER IF EXISTS verification_run_insert_guard;
             DROP TRIGGER IF EXISTS verification_run_transition_guard;
             DROP TRIGGER IF EXISTS verification_run_terminal_immutable;
             DROP TRIGGER IF EXISTS verification_run_delete_guard;
             DROP INDEX IF EXISTS idx_verification_runs_task_status_time;
             ALTER TABLE verification_runs DROP COLUMN failure_code;
             ALTER TABLE verification_runs DROP COLUMN version;
             DELETE FROM schema_migrations WHERE version = 20;
             PRAGMA user_version = 19;
             DROP TRIGGER IF EXISTS resource_index_blob_registry_reference_delete_guard;
             DROP TRIGGER IF EXISTS resource_text_index_blob_registry_update_guard;
             DROP TRIGGER IF EXISTS resource_text_index_blob_registry_insert_guard;
             DROP INDEX IF EXISTS resource_index_blob_registry_gc;
             DROP TABLE IF EXISTS resource_index_blob_registry;
             DELETE FROM schema_migrations WHERE version = 19;
             PRAGMA user_version = 18;",
        )
        .expect("restore the v18 migration boundary without ResourceIndex registry objects");
}

fn base_schema_object(start_marker: &str, end_marker: &str) -> &'static str {
    let start = SQLITE_V1_DDL
        .find(start_marker)
        .expect("base schema contains the requested object");
    let tail = &SQLITE_V1_DDL[start..];
    let end = tail
        .find(end_marker)
        .expect("base schema object has its closing marker")
        + end_marker.len();
    &tail[..end]
}

fn downgrade_upload_chunk_schema_to_v17(connection: &Connection) {
    connection
        .execute_batch(
            "DROP TRIGGER resource_upload_chunk_blob_registry_identity_update_guard;
             DROP TRIGGER resource_upload_chunk_blob_registry_state_update_guard;
             DROP TRIGGER resource_upload_chunk_reservation_resource_guard;
             DROP TRIGGER resource_upload_chunk_reservation_registry_guard;
             DROP TRIGGER resource_upload_chunk_acceptance_resource_guard;
             DROP TRIGGER resource_upload_chunk_registry_insert_guard;
             DROP TRIGGER resource_upload_chunk_ledger_no_update;
             DROP TRIGGER resource_upload_chunk_ledger_no_delete;
             DROP TRIGGER resource_upload_chunk_blob_registry_delete_guard;
             DROP TRIGGER resource_upload_blob_reservation_identity_update_guard;
             DROP TRIGGER resource_upload_blob_reservation_state_update_guard;
             DROP TRIGGER resource_upload_blob_reservation_delete_guard;
             DROP TABLE resource_upload_chunk_blob_registry;
             DROP TRIGGER context_document_purge_receipt_admission_guard;
             DROP TRIGGER context_document_purge_receipt_transition_guard;
             DROP TRIGGER context_document_deleted_requires_purge_ack;
             DROP TRIGGER context_document_purge_plan_shape_guard;
             DROP INDEX idx_context_purge_pending;
             DROP TABLE context_document_purge_receipts;
             DELETE FROM schema_migrations WHERE version = 18;",
        )
        .expect("remove v18-only objects before restoring the v17 boundary");

    for (start, end) in [
        ("CREATE TABLE context_document_purge_receipts (", "\n);"),
        ("CREATE INDEX idx_context_purge_pending", ";"),
        (
            "CREATE TRIGGER context_document_purge_plan_shape_guard",
            "\nEND;",
        ),
        (
            "CREATE TRIGGER context_document_purge_receipt_admission_guard",
            "\nEND;",
        ),
        (
            "CREATE TRIGGER context_document_purge_receipt_transition_guard",
            "\nEND;",
        ),
        (
            "CREATE TRIGGER context_document_deleted_requires_purge_ack",
            "\nEND;",
        ),
    ] {
        connection
            .execute_batch(base_schema_object(start, end))
            .expect("restore the base purge schema objects used before v18");
    }
    connection
        .pragma_update(None, "user_version", 17)
        .expect("restore the v17 schema version");
}

fn resource_gc_failure_blob_store(
    directory: &tempfile::TempDir,
    fail_before_remove: bool,
    fail_after_remove: bool,
) -> Arc<dyn BlobStore> {
    Arc::new(TestUploadBlobStore {
        inner: FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys { key: [23_u8; 32] },
        ),
        chunk_put_pause: None,
        gc_remove_pause: None,
        aggregate_state_put_pause: None,
        aggregate_state_puts: Arc::new(AtomicUsize::new(0)),
        fail_next_remove: Arc::new(AtomicBool::new(false)),
        fail_next_resource_remove: Arc::new(AtomicBool::new(fail_before_remove)),
        fail_after_resource_remove: Arc::new(AtomicBool::new(fail_after_remove)),
        resource_remove_calls: Arc::new(AtomicUsize::new(0)),
        resource_content_put_pause: None,
        resource_content_get_pause: None,
    })
}

fn resource_remove_observer_blob_store(
    directory: &tempfile::TempDir,
) -> (Arc<dyn BlobStore>, Arc<AtomicUsize>) {
    let remove_calls = Arc::new(AtomicUsize::new(0));
    let store = TestUploadBlobStore {
        inner: FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys { key: [23_u8; 32] },
        ),
        chunk_put_pause: None,
        gc_remove_pause: None,
        aggregate_state_put_pause: None,
        aggregate_state_puts: Arc::new(AtomicUsize::new(0)),
        fail_next_remove: Arc::new(AtomicBool::new(false)),
        fail_next_resource_remove: Arc::new(AtomicBool::new(false)),
        fail_after_resource_remove: Arc::new(AtomicBool::new(false)),
        resource_remove_calls: Arc::clone(&remove_calls),
        resource_content_put_pause: None,
        resource_content_get_pause: None,
    };
    (Arc::new(store), remove_calls)
}

#[test]
fn expired_resource_content_reservation_without_a_published_object_is_reclaimed() {
    let directory = tempfile::tempdir().expect("temporary unpublished Resource content directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    let blob = resource_content_blob_ref(b"reserved before Resource content put", "text/plain");
    store
        .reserve_resource_content_blob(WORKSPACE, &blob)
        .expect("persist content intent before an external object publication");
    Connection::open(state_database(&directory))
        .expect("open expired Resource content reservation")
        .execute(
            "UPDATE resource_content_blob_registry
             SET expires_at = '2000-01-01T00:00:00Z'
             WHERE workspace_id = ?1 AND digest = ?2 AND state = 'RESERVED'",
            params![WORKSPACE, blob.digest],
        )
        .expect("age the pre-put reservation past its lease");

    assert_eq!(
        resource_content_registry_state(&directory, &blob).as_deref(),
        Some("RESERVED")
    );
    assert_eq!(
        store
            .collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100)
            .expect("expired reservation is reclaimed even when put never published"),
        1
    );
    assert_eq!(resource_content_registry_state(&directory, &blob), None);
    assert!(matches!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &blob),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn resource_content_gc_fences_a_paused_writer_before_projection_commit() {
    const CONTENT: &[u8] = b"\x89PNG\r\n\x1a\nResource writer paused after its reservation";
    const RESOURCE_ID: &str = "resource-content-writer-gc-fence";
    let directory = tempfile::tempdir().expect("temporary Resource content writer/GC directory");
    let pause = ResourceContentPutPause {
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        armed: Arc::new(AtomicBool::new(true)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let writer_blobs: Arc<dyn BlobStore> = Arc::new(TestUploadBlobStore {
        inner: FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys { key: [23_u8; 32] },
        ),
        chunk_put_pause: None,
        gc_remove_pause: None,
        aggregate_state_put_pause: None,
        aggregate_state_puts: Arc::new(AtomicUsize::new(0)),
        fail_next_remove: Arc::new(AtomicBool::new(false)),
        fail_next_resource_remove: Arc::new(AtomicBool::new(false)),
        fail_after_resource_remove: Arc::new(AtomicBool::new(false)),
        resource_remove_calls: Arc::new(AtomicUsize::new(0)),
        resource_content_put_pause: Some(pause.clone()),
        resource_content_get_pause: None,
    });
    let writer_store = open_store(&directory, writer_blobs);
    create_workspace_for_uploads(&writer_store);
    let blob = resource_content_blob_ref(CONTENT, "image/png");

    let write_store = writer_store.clone();
    let (write_sender, write_receiver) = std::sync::mpsc::sync_channel(1);
    let writer = std::thread::spawn(move || {
        let result = create_resource_with_content(
            &write_store,
            RESOURCE_ID,
            "paused.png",
            "image/png",
            CONTENT,
        );
        let _ = write_sender.send(result);
    });
    pause.entered.wait();
    let reservation_was_committed_before_pause =
        resource_content_registry_state(&directory, &blob).as_deref() == Some("RESERVED");

    let collector_store = writer_store.clone();
    let (collector_sender, collector_receiver) = std::sync::mpsc::sync_channel(1);
    let collector = std::thread::spawn(move || {
        let result =
            collector_store.collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100);
        let _ = collector_sender.send(result);
    });
    let claim_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while resource_content_registry_state(&directory, &blob).as_deref() != Some("DELETING")
        && std::time::Instant::now() < claim_deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    let collector_claimed =
        resource_content_registry_state(&directory, &blob).as_deref() == Some("DELETING");
    let no_partial_blob_is_visible = matches!(
        file_blob_store(&directory).get(WORKSPACE, BlobPurpose::Resource, &blob),
        Err(StoreError::NotFound)
    );

    pause.release.wait();
    let write_result = write_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("Resource writer finishes after releasing the provider barrier");
    writer.join().expect("Resource writer thread exits");
    let collected = collector_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("collector finishes after the writer releases its digest lock")
        .expect("collector removes the now-published unrooted object");
    collector.join().expect("Resource collector thread exits");

    assert!(
        collector_claimed,
        "GC persists DELETING while waiting for the writer lock"
    );
    assert!(
        reservation_was_committed_before_pause,
        "the writer pauses only after its exact durable reservation commits"
    );
    assert!(
        no_partial_blob_is_visible,
        "the paused writer has not published bytes yet"
    );
    assert!(matches!(write_result, Err(StoreError::Busy)));
    assert_eq!(collected, 1);
    assert_eq!(resource_content_registry_state(&directory, &blob), None);
    assert!(matches!(
        writer_store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &blob),
        Err(StoreError::NotFound)
    ));
    let resource_count: i64 = Connection::open(state_database(&directory))
        .expect("inspect fenced Resource projection")
        .query_row(
            "SELECT COUNT(*) FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, RESOURCE_ID],
            |row| row.get(0),
        )
        .expect("count absent Resource projection");
    assert_eq!(resource_count, 0);
}

#[test]
fn resource_content_gc_keeps_shared_blob_until_every_local_historical_root_is_external() {
    const SHARED: &[u8] = b"Resource content shared by a current and historical revision";
    const ALTERNATE: &[u8] = b"new head content moves the shared reference into history";
    let directory = tempfile::tempdir().expect("temporary shared Resource content GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_resource_with_content(
        &store,
        "resource-content-shared-history-a",
        "shared-history-a.txt",
        "text/plain",
        SHARED,
    )
    .expect("first Resource points at the shared content");
    create_resource_with_content(
        &store,
        "resource-content-shared-history-b",
        "shared-history-b.txt",
        "text/plain",
        SHARED,
    )
    .expect("second Resource shares the exact Workspace content object");
    create_resource_with_content(
        &store,
        "resource-content-alternate-head",
        "alternate-head.txt",
        "text/plain",
        ALTERNATE,
    )
    .expect("another committed Resource registers the replacement head object");

    let shared_blob = resource_content_blob_ref(SHARED, "text/plain");
    let alternate_digest = digest(ALTERNATE);
    let resource_id = "resource-content-shared-history-a";
    let previous_revision = format!("revision-{resource_id}");
    let alternate_revision = "revision-resource-content-shared-history-a-new-head";
    let connection = Connection::open(state_database(&directory))
        .expect("open Resource historical-root fixture");
    connection
        .execute(
            "INSERT INTO resource_revisions(
               resource_revision_id, resource_id, provider_revision, content_digest,
               size_bytes, media_type, observed_at, created_by_json
             ) VALUES (?1, ?2, NULL, ?3, ?4, 'text/plain', ?5, '{}')",
            params![
                alternate_revision,
                resource_id,
                alternate_digest,
                ALTERNATE.len() as i64,
                CREATED,
            ],
        )
        .expect("append a new exact-registered head revision");
    connection
        .execute(
            "INSERT INTO resource_revision_parents(resource_id, child_revision_id, parent_revision_id)
             VALUES (?1, ?2, ?3)",
            params![resource_id, alternate_revision, previous_revision],
        )
        .expect("retain the original shared revision in the ancestry");
    connection
        .execute(
            "UPDATE resources SET current_revision_id = ?1, version = version + 1
             WHERE workspace_id = ?2 AND resource_id = ?3",
            params![alternate_revision, WORKSPACE, resource_id],
        )
        .expect("move Resource head to the alternate content");
    connection
        .execute(
            "UPDATE resource_locations
             SET observed_revision_id = ?1, observed_digest = ?2
             WHERE resource_id = ?3 AND provider_ref = 'litecowork.encrypted_blob'",
            params![alternate_revision, alternate_digest, resource_id],
        )
        .expect("advance the local managed location with its Resource head");
    connection
        .execute(
            "UPDATE resource_locations SET provider_ref = 'external.test'
             WHERE resource_id = ?1",
            ["resource-content-shared-history-b"],
        )
        .expect("remove the second Resource's local managed root");
    drop(connection);

    let historical_roots: i64 = Connection::open(state_database(&directory))
        .expect("inspect historical Resource roots")
        .query_row(
            "SELECT COUNT(*) FROM resource_revisions revision
             WHERE revision.resource_id = ?1 AND revision.content_digest = ?2
               AND revision.resource_revision_id <> ?3",
            params![resource_id, shared_blob.digest, alternate_revision],
            |row| row.get(0),
        )
        .expect("count the prior Resource revisions that keep shared content");
    assert_eq!(historical_roots, 1);
    assert_eq!(
        store
            .collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100)
            .expect("historical local revision remains a content GC root"),
        0
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &shared_blob)
            .expect("historical revision keeps the shared encrypted object"),
        SHARED
    );

    Connection::open(state_database(&directory))
        .expect("open final local content root")
        .execute(
            "UPDATE resource_locations SET provider_ref = 'external.test'
             WHERE resource_id = ?1",
            [resource_id],
        )
        .expect("remove the final local managed root while keeping immutable history");
    assert_eq!(
        store
            .collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100)
            .expect("externalized Resources no longer root the local object"),
        1
    );
    assert_eq!(
        resource_content_registry_state(&directory, &shared_blob),
        None
    );
    assert!(matches!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &shared_blob),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn resource_content_gc_preserves_deleting_fence_after_remove_failure_and_retries() {
    const CONTENT: &[u8] = b"retry Resource content deletion after restart";
    let directory = tempfile::tempdir().expect("temporary Resource content delete retry directory");
    let store = open_store(
        &directory,
        resource_gc_failure_blob_store(&directory, true, false),
    );
    create_workspace_for_uploads(&store);
    create_resource_with_content(
        &store,
        "resource-content-delete-retry",
        "delete-retry.txt",
        "text/plain",
        CONTENT,
    )
    .expect("Resource content is tracked");
    let blob = resource_content_blob_ref(CONTENT, "text/plain");
    Connection::open(state_database(&directory))
        .expect("open local content location for simulated externalization")
        .execute(
            "UPDATE resource_locations SET provider_ref = 'external.test'
             WHERE resource_id = ?1",
            ["resource-content-delete-retry"],
        )
        .expect("remove the local managed root");

    assert!(matches!(
        store.collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100),
        Err(StoreError::Blob(_))
    ));
    assert_eq!(
        resource_content_registry_state(&directory, &blob).as_deref(),
        Some("DELETING"),
        "failed exact-object deletion must preserve its durable fence"
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &blob)
            .expect("the injected provider error leaves the object intact"),
        CONTENT
    );

    drop(store);
    let restarted = open_store(&directory, file_blob_store(&directory));
    assert_eq!(
        restarted
            .collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100)
            .expect("restart retries the fenced exact-object deletion"),
        1
    );
    assert_eq!(resource_content_registry_state(&directory, &blob), None);
    assert!(matches!(
        restarted
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &blob),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn resource_content_gc_retries_after_remove_acknowledgement_was_ambiguous() {
    const CONTENT: &[u8] = b"Resource content removal acknowledgement lost after success";
    let directory =
        tempfile::tempdir().expect("temporary Resource content ambiguous-remove directory");
    let store = open_store(
        &directory,
        resource_gc_failure_blob_store(&directory, false, true),
    );
    create_workspace_for_uploads(&store);
    create_resource_with_content(
        &store,
        "resource-content-remove-ack-lost",
        "remove-ack-lost.txt",
        "text/plain",
        CONTENT,
    )
    .expect("Resource content is tracked");
    let blob = resource_content_blob_ref(CONTENT, "text/plain");
    Connection::open(state_database(&directory))
        .expect("open local content location for simulated externalization")
        .execute(
            "UPDATE resource_locations SET provider_ref = 'external.test'
             WHERE resource_id = ?1",
            ["resource-content-remove-ack-lost"],
        )
        .expect("remove the local managed root");

    assert!(matches!(
        store.collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100),
        Err(StoreError::Blob(_))
    ));
    assert_eq!(
        resource_content_registry_state(&directory, &blob).as_deref(),
        Some("DELETING"),
        "ambiguous provider acknowledgment retains the deletion fence"
    );
    assert!(
        matches!(
            store
                .inner
                .blobs
                .get(WORKSPACE, BlobPurpose::Resource, &blob),
            Err(StoreError::NotFound)
        ),
        "the wrapper physically removed the object before losing the acknowledgment"
    );

    drop(store);
    let restarted = open_store(&directory, file_blob_store(&directory));
    assert_eq!(
        restarted
            .collect_orphan_resource_content_blobs("2099-01-01T00:00:01Z", 100)
            .expect("missing-object remove is idempotent and releases the persisted fence"),
        1
    );
    assert_eq!(resource_content_registry_state(&directory, &blob), None);
}

#[test]
fn resource_content_read_holds_blob_against_gc_after_location_externalization() {
    const CONTENT: &[u8] = b"Resource read admitted before local location externalization";
    const RESOURCE_ID: &str = "resource-content-read-gc-fence";
    let directory = tempfile::tempdir().expect("temporary Resource content read/GC race directory");
    let pause = ResourceContentGetPause {
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        armed: Arc::new(AtomicBool::new(false)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let read_blobs: Arc<dyn BlobStore> = Arc::new(TestUploadBlobStore {
        inner: FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys { key: [23_u8; 32] },
        ),
        chunk_put_pause: None,
        gc_remove_pause: None,
        aggregate_state_put_pause: None,
        aggregate_state_puts: Arc::new(AtomicUsize::new(0)),
        fail_next_remove: Arc::new(AtomicBool::new(false)),
        fail_next_resource_remove: Arc::new(AtomicBool::new(false)),
        fail_after_resource_remove: Arc::new(AtomicBool::new(false)),
        resource_remove_calls: Arc::new(AtomicUsize::new(0)),
        resource_content_put_pause: None,
        resource_content_get_pause: Some(pause.clone()),
    });
    let reader_store = open_store(&directory, read_blobs);
    let collector_store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&reader_store);
    create_resource_with_content(
        &reader_store,
        RESOURCE_ID,
        "read-gc-fence.txt",
        "text/plain",
        CONTENT,
    )
    .expect("locally managed content is published and tracked");
    let blob = resource_content_blob_ref(CONTENT, "text/plain");
    pause.armed.store(true, Ordering::SeqCst);

    let read_store = reader_store.clone();
    let (read_sender, read_receiver) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let result = read_store.read_resource_content(WORKSPACE, RESOURCE_ID);
        let _ = read_sender.send(result);
    });
    pause.entered.wait();

    Connection::open(state_database(&directory))
        .expect("open Resource location while content read is paused")
        .execute(
            "UPDATE resource_locations SET provider_ref = 'external.test'
             WHERE resource_id = ?1",
            [RESOURCE_ID],
        )
        .expect("remove the local managed root after the read acquired its digest lock");
    let gc_store = collector_store.clone();
    let (gc_sender, gc_receiver) = std::sync::mpsc::sync_channel(1);
    let collector = std::thread::spawn(move || {
        let result = gc_store.collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100);
        let _ = gc_sender.send(result);
    });

    let claim_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while resource_content_registry_state(&directory, &blob).as_deref() != Some("DELETING")
        && std::time::Instant::now() < claim_deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    let collector_claimed =
        resource_content_registry_state(&directory, &blob).as_deref() == Some("DELETING");
    let blob_survives_while_read_is_paused = matches!(
        file_blob_store(&directory).get(WORKSPACE, BlobPurpose::Resource, &blob),
        Ok(content) if content == CONTENT
    );

    pause.release.wait();
    let read_result = read_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("paused Resource read completes after the barrier is released");
    reader.join().expect("Resource read thread exits");
    let collected = gc_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("collector completes after the read releases its digest lock")
        .expect("Resource GC removes the now-unrooted object");
    collector.join().expect("Resource collector thread exits");

    assert!(
        collector_claimed,
        "GC must persist DELETING before waiting for the reader"
    );
    assert!(
        blob_survives_while_read_is_paused,
        "the reader's shared digest lock keeps the exact object available"
    );
    let read = read_result
        .expect("reader admitted before externalization returns successfully")
        .expect("Resource existed when the read was admitted");
    assert_eq!(read.content, CONTENT);
    assert_eq!(collected, 1);
    assert_eq!(resource_content_registry_state(&directory, &blob), None);
    assert!(matches!(
        file_blob_store(&directory).get(WORKSPACE, BlobPurpose::Resource, &blob),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn failed_resource_projection_is_reclaimed_after_restart() {
    const CONTENT: &[u8] = b"\x89PNG\r\n\x1a\nresource content whose SQL commit is rejected";
    let directory = tempfile::tempdir().expect("temporary Resource content GC directory");
    let blobs = file_blob_store(&directory);
    let store = open_store(&directory, Arc::clone(&blobs));
    create_workspace_for_uploads(&store);
    let content_blob = BlobRef {
        digest: digest(CONTENT),
        size_bytes: CONTENT.len() as u64,
        media_type: "image/png".to_owned(),
    };
    Connection::open(state_database(&directory))
        .expect("open Resource transaction failpoint")
        .execute_batch(
            "CREATE TRIGGER reject_resource_create_event
             BEFORE INSERT ON domain_events
             WHEN NEW.entity_type = 'Resource'
             BEGIN
               SELECT RAISE(ABORT, 'TEST_FAIL_RESOURCE_COMMIT');
             END;",
        )
        .expect("inject final Resource SQL failure after blob publication");

    assert!(
        create_resource_with_content(
            &store,
            "resource-content-gc-failed-projection",
            "failed.png",
            "image/png",
            CONTENT,
        )
        .is_err()
    );
    assert_eq!(
        blobs
            .get(WORKSPACE, BlobPurpose::Resource, &content_blob)
            .expect("the encrypted object was published before SQL rejected its projection"),
        CONTENT
    );
    let reservation_state: String = Connection::open(state_database(&directory))
        .expect("inspect Resource content reservation")
        .query_row(
            "SELECT state FROM resource_content_blob_registry WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, content_blob.digest],
            |row| row.get(0),
        )
        .expect("failed SQL transaction leaves its durable reservation");
    assert_eq!(reservation_state, "RESERVED");

    Connection::open(state_database(&directory))
        .expect("remove the injected SQL failure")
        .execute_batch("DROP TRIGGER reject_resource_create_event;")
        .expect("restore production schema before reopen");
    drop(store);
    let restarted = open_store(&directory, blobs);
    assert_eq!(
        restarted
            .collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100)
            .expect("restart sweep claims and removes the failed create object"),
        1
    );
    assert!(matches!(
        restarted
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &content_blob),
        Err(StoreError::NotFound)
    ));
    let remaining: i64 = Connection::open(state_database(&directory))
        .expect("inspect cleared content registry")
        .query_row(
            "SELECT COUNT(*) FROM resource_content_blob_registry WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, content_blob.digest],
            |row| row.get(0),
        )
        .expect("count matching content registry row");
    assert_eq!(remaining, 0);
}

#[test]
fn failed_resource_upload_commit_projection_is_reclaimed_after_restart() {
    const CONTENT: &[u8] = b"\x89PNG\r\n\x1a\nresource upload content whose SQL commit is rejected";
    const UPLOAD_ID: &str = "resource-upload-gc-failed-projection";
    const RESOURCE_ID: &str = "resource-upload-gc-failed-projection-result";
    let directory = tempfile::tempdir().expect("temporary Resource upload GC directory");
    let blobs = file_blob_store(&directory);
    let store = open_store(&directory, Arc::clone(&blobs));
    create_workspace_for_uploads(&store);
    create_upload_with_content(&store, UPLOAD_ID, "failed-upload.png", "image/png", CONTENT);
    store
        .put_chunk(chunk_input_with_content(
            UPLOAD_ID,
            "resource-upload-gc-failed-projection-chunk",
            CONTENT,
        ))
        .expect("upload content reaches CONTENT_RECEIVED");

    let content_blob = resource_content_blob_ref(CONTENT, "image/png");
    Connection::open(state_database(&directory))
        .expect("open Resource upload transaction failpoint")
        .execute_batch(
            "CREATE TRIGGER reject_resource_upload_commit_event
             BEFORE INSERT ON domain_events
             WHEN NEW.entity_type = 'Resource' AND NEW.entity_id = 'resource-upload-gc-failed-projection-result'
             BEGIN
               SELECT RAISE(ABORT, 'TEST_FAIL_RESOURCE_UPLOAD_COMMIT');
             END;",
        )
        .expect("inject final Resource SQL failure after upload blob publication");

    assert!(
        commit_upload_with_content(
            &store,
            UPLOAD_ID,
            RESOURCE_ID,
            "failed-upload.png",
            "image/png",
            CONTENT,
        )
        .is_err()
    );
    assert_eq!(
        blobs
            .get(WORKSPACE, BlobPurpose::Resource, &content_blob)
            .expect(
                "upload commit published the Resource object before SQL rejected its projection"
            ),
        CONTENT
    );
    assert_eq!(
        resource_content_registry_state(&directory, &content_blob).as_deref(),
        Some("RESERVED"),
        "the failed projection rolls back RESERVED-to-TRACKED in the commit transaction"
    );
    let session = store
        .get(WORKSPACE, UPLOAD_ID)
        .expect("read upload session after failed commit")
        .expect("upload session remains durable");
    assert_eq!(session.state, ResourceUploadState::ContentReceived);
    assert_eq!(session.committed_resource_id, None);
    let connection = Connection::open(state_database(&directory)).expect("inspect failed commit");
    for (table, id_column, id) in [
        ("resources", "resource_id", RESOURCE_ID),
        ("resource_revisions", "resource_id", RESOURCE_ID),
        ("resource_locations", "resource_id", RESOURCE_ID),
    ] {
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE {id_column} = ?1");
        let count: i64 = connection
            .query_row(&sql, [id], |row| row.get(0))
            .expect("inspect absent Resource projection");
        assert_eq!(count, 0, "failed upload commit left rows in {table}");
    }
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            params![OWNER, format!("resource-upload-commit:{UPLOAD_ID}")],
            |row| row.get(0),
        )
        .expect("inspect absent upload commit receipt");
    assert_eq!(receipt_count, 0);
    drop(connection);

    Connection::open(state_database(&directory))
        .expect("remove the injected SQL failure")
        .execute_batch("DROP TRIGGER reject_resource_upload_commit_event;")
        .expect("restore production schema before reopen");
    drop(store);
    let restarted = open_store(&directory, blobs);
    assert_eq!(
        restarted
            .collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100)
            .expect("restart sweep claims and removes the failed upload commit object"),
        1
    );
    assert!(matches!(
        restarted
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &content_blob),
        Err(StoreError::NotFound)
    ));
    assert_eq!(
        resource_content_registry_state(&directory, &content_blob),
        None
    );
    assert_eq!(
        restarted
            .get(WORKSPACE, UPLOAD_ID)
            .expect("read upload after restart")
            .expect("failed upload remains available for retry")
            .state,
        ResourceUploadState::ContentReceived
    );
}

#[test]
fn resource_content_v17_migration_backfills_all_local_revisions_only() {
    const LOCAL_HEAD: &[u8] = b"\x89PNG local Resource initial revision";
    const HISTORICAL: &[u8] = b"\x89PNG content referenced by a historical revision";
    const EXTERNAL_ONLY: &[u8] = b"\x89PNG external-only Resource bytes";
    const NULL_MEDIA_CONTENT: &[u8] = b"legacy local Resource bytes with no MIME metadata";
    let directory = tempfile::tempdir().expect("temporary Resource content migration directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_resource_with_content(
        &store,
        "resource-content-migration-local",
        "local.png",
        "image/png",
        LOCAL_HEAD,
    )
    .expect("create locally managed Resource");
    create_resource_with_content(
        &store,
        "resource-content-migration-shared",
        "shared.png",
        "image/png",
        HISTORICAL,
    )
    .expect("create content used by a historical revision");
    let historical_digest = digest(HISTORICAL);
    let external_digest = digest(EXTERNAL_ONLY);
    let external_blob = resource_content_blob_ref(EXTERNAL_ONLY, "image/png");
    let external_fixture = Connection::open(state_database(&directory))
        .expect("open v16-style external Resource fixture");
    external_fixture
        .execute_batch(&format!(
            "BEGIN;
             INSERT INTO resources(
               resource_id, workspace_id, kind, provider_identity_json, identity_digest,
               display_name, current_revision_id, sensitivity, context_document_json,
               provenance_json, created_at, updated_at, version
             ) VALUES (
               'resource-content-migration-external', '{WORKSPACE}', 'FILE', '{{}}', NULL,
               'external.png', 'revision-resource-content-migration-external', 'PERSONAL', NULL,
               '{{\"source_inputs\":[],\"transformations\":[],\"tool_reports\":[]}}',
               '{CREATED}', '{CREATED}', 1
             );
             INSERT INTO resource_revisions(
               resource_revision_id, resource_id, provider_revision, content_digest,
               size_bytes, media_type, observed_at, created_by_json
             ) VALUES (
               'revision-resource-content-migration-external',
               'resource-content-migration-external', NULL, '{external_digest}',
               {}, 'image/png', '{CREATED}', '{{}}'
             );
             INSERT INTO resource_locations(
               location_id, resource_id, runtime_id, environment_id, connection_id,
               provider_ref, locator_ref_id, availability, writable,
               observed_revision_id, observed_digest, observed_at, last_checked_at
             ) VALUES (
               'location-resource-content-migration-external',
               'resource-content-migration-external', NULL, NULL, NULL,
               'external.test', 'external-only-locator', 'AVAILABLE', 0,
               'revision-resource-content-migration-external', '{external_digest}',
               '{CREATED}', NULL
             );
             COMMIT;",
            EXTERNAL_ONLY.len()
        ))
        .expect("model a v16 external-only Resource without publishing a local object");
    assert!(matches!(
        file_blob_store(&directory).get(WORKSPACE, BlobPurpose::Resource, &external_blob),
        Err(StoreError::NotFound)
    ));
    drop(external_fixture);
    Connection::open(state_database(&directory))
        .expect("open Resource revision fixture")
        .execute_batch(&format!(
            "INSERT INTO resource_revisions(
               resource_revision_id, resource_id, provider_revision, content_digest,
               size_bytes, media_type, observed_at, created_by_json
             ) VALUES (
               'revision-resource-content-migration-history',
               'resource-content-migration-local', NULL, '{historical_digest}',
               {}, 'image/png', '{CREATED}', '{{}}'
             );",
            HISTORICAL.len()
        ))
        .expect("add historical local revision");
    drop(store);
    file_blob_store(&directory)
        .put(
            WORKSPACE,
            BlobPurpose::Resource,
            NULL_MEDIA_CONTENT,
            "application/octet-stream",
        )
        .expect("model the local blob written by the v16 default-media path");

    let mut connection = Connection::open(state_database(&directory)).expect("open v16 boundary");
    downgrade_resource_content_schema_to_v16(&connection);
    let null_media_digest = digest(NULL_MEDIA_CONTENT);
    let null_media_revision = "revision-resource-content-migration-null-media";
    connection
        .execute(
            "INSERT INTO resource_revisions(
               resource_revision_id, resource_id, provider_revision, content_digest,
               size_bytes, media_type, observed_at, created_by_json
             ) VALUES (?1, 'resource-content-migration-local', NULL, ?2, ?3, NULL, ?4, '{}')",
            params![
                null_media_revision,
                null_media_digest,
                NULL_MEDIA_CONTENT.len() as i64,
                CREATED,
            ],
        )
        .expect("add a v16 local revision whose MIME metadata was absent");
    connection
        .execute(
            "INSERT INTO resource_revision_parents(resource_id, child_revision_id, parent_revision_id)
             VALUES ('resource-content-migration-local', ?1, 'revision-resource-content-migration-local')",
            [null_media_revision],
        )
        .expect("retain a valid Resource revision ancestry");
    connection
        .execute(
            "UPDATE resources SET current_revision_id = ?1, version = 2
             WHERE workspace_id = ?2 AND resource_id = 'resource-content-migration-local'",
            params![null_media_revision, WORKSPACE],
        )
        .expect("advance the local Resource to the v16 missing-media revision");
    connection
        .execute(
            "UPDATE resource_locations
             SET observed_revision_id = ?1, observed_digest = ?2
             WHERE resource_id = 'resource-content-migration-local'",
            params![null_media_revision, null_media_digest],
        )
        .expect("keep the managed location observation aligned with the current head");
    migrate(&mut connection).expect("v17 backfills exact local Resource revision roots");

    let local_rows: Vec<(String, i64, String, String)> = connection
        .prepare(
            "SELECT digest, size_bytes, media_type, state
             FROM resource_content_blob_registry
             WHERE workspace_id = ?1 ORDER BY digest",
        )
        .expect("prepare local content backfill query")
        .query_map([WORKSPACE], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query local content backfill")
        .collect::<Result<_, _>>()
        .expect("collect backfilled registry rows");
    assert_eq!(local_rows.len(), 3);
    assert!(local_rows.iter().any(|(digest_value, size, media, state)| {
        digest_value == &digest(LOCAL_HEAD)
            && *size == LOCAL_HEAD.len() as i64
            && media == "image/png"
            && state == "TRACKED"
    }));
    assert!(local_rows.iter().any(|(digest_value, size, media, state)| {
        digest_value == &historical_digest
            && *size == HISTORICAL.len() as i64
            && media == "image/png"
            && state == "TRACKED"
    }));
    assert!(local_rows.iter().any(|(digest_value, size, media, state)| {
        digest_value == &null_media_digest
            && *size == NULL_MEDIA_CONTENT.len() as i64
            && media == "application/octet-stream"
            && state == "TRACKED"
    }));
    assert!(
        !local_rows
            .iter()
            .any(|(digest_value, _, _, _)| digest_value == &external_digest)
    );
    assert!(
        matches!(
            file_blob_store(&directory).get(WORKSPACE, BlobPurpose::Resource, &external_blob),
            Err(StoreError::NotFound)
        ),
        "the external-only migration fixture never published local Resource bytes"
    );
    drop(connection);

    let (blobs, resource_remove_calls) = resource_remove_observer_blob_store(&directory);
    let reopened = open_store(&directory, blobs);
    assert_eq!(
        reopened
            .collect_orphan_resource_content_blobs("2099-01-01T00:00:00Z", 100)
            .expect("the migrated local revisions remain rooted"),
        0
    );
    assert_eq!(resource_remove_calls.load(Ordering::SeqCst), 0);
    let null_media_blob = resource_content_blob_ref(NULL_MEDIA_CONTENT, "application/octet-stream");
    assert_eq!(
        reopened
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::Resource, &null_media_blob)
            .expect("v17 backfill uses the default MIME type that identifies the existing object"),
        NULL_MEDIA_CONTENT
    );
    let legacy_content = reopened
        .read_resource_content(WORKSPACE, "resource-content-migration-local")
        .expect("read the migrated NULL-media Resource revision")
        .expect("the migrated local Resource remains readable");
    assert_eq!(
        legacy_content.summary.media_type,
        "application/octet-stream"
    );
    assert_eq!(legacy_content.content, NULL_MEDIA_CONTENT);
    let catalog_row = reopened
        .list_resources_page(WORKSPACE, None, None, 100)
        .expect("read the Resource catalog projection")
        .into_iter()
        .find(|resource| resource.resource_id == "resource-content-migration-local")
        .expect("the migrated Resource remains in the catalog");
    assert_eq!(catalog_row.media_type, "application/octet-stream");
}

fn assert_v17_migration_was_not_partially_applied(connection: &Connection) {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("inspect retained schema version");
    assert_eq!(version, 16);
    let registry_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'resource_content_blob_registry')",
            [],
            |row| row.get(0),
        )
        .expect("inspect absent v17 registry table");
    assert!(!registry_exists);
    let migration_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 17",
            [],
            |row| row.get(0),
        )
        .expect("inspect absent v17 migration receipt");
    assert_eq!(migration_count, 0);
}

#[test]
fn resource_content_v17_migration_rejects_missing_managed_history_metadata_atomically() {
    const CONTENT: &[u8] = b"managed Resource with malformed v16 history";
    const RESOURCE_ID: &str = "resource-content-migration-missing-history";
    let directory = tempfile::tempdir().expect("temporary malformed Resource migration directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_resource_with_content(&store, RESOURCE_ID, "history.txt", "text/plain", CONTENT)
        .expect("create valid local Resource head");
    drop(store);

    let mut connection = Connection::open(state_database(&directory))
        .expect("open Resource content v16 migration boundary");
    downgrade_resource_content_schema_to_v16(&connection);
    connection
        .execute(
            "INSERT INTO resource_revisions(
               resource_revision_id, resource_id, content_digest, size_bytes,
               media_type, observed_at, created_by_json
             ) VALUES ('revision-missing-managed-history', ?1, NULL, 1, 'text/plain', ?2, '{}')",
            params![RESOURCE_ID, CREATED],
        )
        .expect("model a legacy managed historical revision with missing exact blob metadata");

    assert!(matches!(
        migrate(&mut connection),
        Err(StoreError::CorruptSchema(message))
            if message == "managed Resource revision is missing exact blob metadata"
    ));
    assert_v17_migration_was_not_partially_applied(&connection);
}

#[test]
fn resource_content_v17_migration_rejects_conflicting_managed_digest_metadata_atomically() {
    const CONTENT: &[u8] = b"managed Resource with conflicting v16 digest size";
    const RESOURCE_ID: &str = "resource-content-migration-conflicting-history";
    let directory =
        tempfile::tempdir().expect("temporary conflicting Resource migration directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_resource_with_content(&store, RESOURCE_ID, "history.txt", "text/plain", CONTENT)
        .expect("create valid local Resource head");
    drop(store);

    let mut connection = Connection::open(state_database(&directory))
        .expect("open Resource content v16 migration boundary");
    downgrade_resource_content_schema_to_v16(&connection);
    connection
        .execute(
            "INSERT INTO resource_revisions(
               resource_revision_id, resource_id, content_digest, size_bytes,
               media_type, observed_at, created_by_json
             ) VALUES (?1, ?2, ?3, ?4, 'text/plain', ?5, '{}')",
            params![
                "revision-conflicting-managed-history",
                RESOURCE_ID,
                digest(CONTENT),
                CONTENT.len() as i64 + 1,
                CREATED,
            ],
        )
        .expect("model a legacy history row with conflicting metadata for one digest");
    let boundary_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("inspect v16 pre-migration boundary");
    let conflicting_rows: bool = connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM resources resource
               JOIN resource_revisions revision ON revision.resource_id = resource.resource_id
               WHERE EXISTS (
                 SELECT 1 FROM resource_locations location
                 WHERE location.resource_id = resource.resource_id
                   AND location.provider_ref = 'litecowork.encrypted_blob'
                   AND location.runtime_id IS NULL
                   AND location.environment_id IS NULL
                   AND location.connection_id IS NULL
               )
               GROUP BY resource.workspace_id, revision.content_digest
               HAVING MIN(revision.size_bytes) <> MAX(revision.size_bytes)
                   OR MIN(COALESCE(revision.media_type, 'application/octet-stream'))
                      <> MAX(COALESCE(revision.media_type, 'application/octet-stream'))
             )",
            [],
            |row| row.get(0),
        )
        .expect("inspect malformed migration fixture");
    assert_eq!(boundary_version, 16);
    assert!(
        conflicting_rows,
        "fixture must contain conflicting exact metadata"
    );

    let migration_error = migrate(&mut connection)
        .expect_err("v17 must reject inconsistent metadata for one local Resource digest");
    assert!(
        matches!(
            &migration_error,
            StoreError::CorruptSchema(message)
                if message == "Resource digest is recorded with inconsistent content metadata"
        ),
        "unexpected migration error: {migration_error:?}"
    );
    assert_v17_migration_was_not_partially_applied(&connection);
}

#[test]
fn resource_content_v17_revision_guard_rejects_missing_and_deleting_reservations() {
    const CONTENT: &[u8] = b"resource content backing a revision guard test";
    const RESOURCE_ID: &str = "resource-content-revision-guard";
    let directory = tempfile::tempdir().expect("temporary Resource content guard directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_resource_with_content(&store, RESOURCE_ID, "guard.txt", "text/plain", CONTENT)
        .expect("create a local managed Resource");
    let connection = Connection::open(state_database(&directory)).expect("open Resource guard DB");
    let missing_digest = digest(b"no registered Resource object");
    let missing = connection
        .execute(
            "INSERT INTO resource_revisions(
           resource_revision_id, resource_id, content_digest, size_bytes,
           media_type, observed_at, created_by_json
         ) VALUES ('revision-without-reservation', ?1, ?2, 1, 'text/plain', ?3, '{}')",
            params![RESOURCE_ID, missing_digest, CREATED],
        )
        .expect_err("the migration-installed guard must reject an unregistered digest");
    assert!(
        missing
            .to_string()
            .contains("RESOURCE_CONTENT_BLOB_NOT_RESERVED")
    );

    let tracked_blob = resource_content_blob_ref(CONTENT, "text/plain");
    connection
        .execute(
            "UPDATE resource_content_blob_registry
             SET state = 'DELETING', expires_at = NULL, claimed_at = ?1
             WHERE workspace_id = ?2 AND digest = ?3 AND state = 'TRACKED'",
            params![CREATED, WORKSPACE, tracked_blob.digest],
        )
        .expect("model a durable deletion fence for the existing object");
    let deleting_state: String = connection
        .query_row(
            "SELECT registry.state
             FROM resource_content_blob_registry registry
             WHERE registry.workspace_id = ?2 AND registry.digest = ?3",
            params![RESOURCE_ID, WORKSPACE, tracked_blob.digest],
            |row| row.get(0),
        )
        .expect("inspect the deletion fence before the revision insert");
    assert_eq!(deleting_state, "DELETING");
    let deleting = connection
        .execute(
            "INSERT INTO resource_revisions(
               resource_revision_id, resource_id, content_digest, size_bytes,
               media_type, observed_at, created_by_json
             ) VALUES ('revision-with-deleting-reservation', ?1, ?2, ?3, 'text/plain', ?4, '{}')",
            params![
                RESOURCE_ID,
                tracked_blob.digest,
                tracked_blob.size_bytes as i64,
                CREATED
            ],
        )
        .expect_err("the migration-installed guard must reject a DELETING object");
    assert!(
        deleting
            .to_string()
            .contains("RESOURCE_CONTENT_BLOB_NOT_RESERVED")
    );

    let revision_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_revisions WHERE resource_id = ?1",
            [RESOURCE_ID],
            |row| row.get(0),
        )
        .expect("inspect immutable Resource revision history");
    assert_eq!(revision_count, 1);
}

#[test]
fn resource_content_v17_location_update_guard_rejects_deleting_reservation() {
    const CONTENT: &[u8] = b"resource content for a managed-location reattachment race";
    const RESOURCE_ID: &str = "resource-content-location-reattach-guard";
    let directory = tempfile::tempdir().expect("temporary Resource location guard directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_resource_with_content(&store, RESOURCE_ID, "reattach.txt", "text/plain", CONTENT)
        .expect("create a local managed Resource");

    let tracked_blob = resource_content_blob_ref(CONTENT, "text/plain");
    let connection = Connection::open(state_database(&directory)).expect("open Resource guard DB");
    let externalized = connection
        .execute(
            "UPDATE resource_locations
             SET provider_ref = 'external.test', locator_ref_id = 'external-location-ref'
             WHERE resource_id = ?1 AND provider_ref = 'litecowork.encrypted_blob'
               AND runtime_id IS NULL AND environment_id IS NULL AND connection_id IS NULL",
            [RESOURCE_ID],
        )
        .expect("externalize the location before collection claims its old object");
    assert_eq!(externalized, 1);
    connection
        .execute(
            "UPDATE resource_content_blob_registry
             SET state = 'DELETING', expires_at = NULL, claimed_at = ?1
             WHERE workspace_id = ?2 AND digest = ?3 AND state = 'TRACKED'",
            params![CREATED, WORKSPACE, tracked_blob.digest],
        )
        .expect("model the collector's durable delete fence after externalization");

    let reattachment = connection
        .execute(
            "UPDATE resource_locations
             SET provider_ref = 'litecowork.encrypted_blob', locator_ref_id = ?1
             WHERE resource_id = ?2 AND provider_ref = 'external.test'",
            params![tracked_blob.digest, RESOURCE_ID],
        )
        .expect_err("the migration-installed guard must reject reattaching a deleting object");
    assert!(
        reattachment
            .to_string()
            .contains("RESOURCE_CONTENT_BLOB_NOT_RESERVED")
    );
    let provider_ref: String = connection
        .query_row(
            "SELECT provider_ref FROM resource_locations WHERE resource_id = ?1",
            [RESOURCE_ID],
            |row| row.get(0),
        )
        .expect("read the unchanged external location after rejection");
    assert_eq!(provider_ref, "external.test");
}

fn resource_index_blob_ref(bytes: &[u8]) -> BlobRef {
    BlobRef {
        digest: digest(bytes),
        size_bytes: bytes.len() as u64,
        media_type: resource_index::INDEX_MEDIA_TYPE.to_owned(),
    }
}

fn resource_index_registry_state(directory: &tempfile::TempDir, blob: &BlobRef) -> Option<String> {
    Connection::open(state_database(directory))
        .expect("open ResourceIndex registry")
        .query_row(
            "SELECT state FROM resource_index_blob_registry WHERE workspace_id = ?1 AND digest = ?2 AND size_bytes = ?3",
            params![WORKSPACE, blob.digest, blob.size_bytes as i64],
            |row| row.get(0),
        )
        .optional()
        .expect("read ResourceIndex registry state")
}

#[test]
fn failed_resource_index_projection_is_reclaimed_after_restart() {
    const CONTENT: &[u8] = b"orphaned encrypted ResourceIndex snapshot";
    let directory = tempfile::tempdir().expect("temporary ResourceIndex orphan directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    Connection::open(state_database(&directory))
        .expect("open ResourceIndex trigger fixture")
        .execute_batch(
            "CREATE TRIGGER reject_resource_index_projection
             BEFORE INSERT ON resource_text_indexes
             BEGIN SELECT RAISE(ABORT, 'injected ResourceIndex SQL failure'); END;",
        )
        .expect("install deterministic post-BlobStore SQL failure");

    assert!(
        create_resource_with_content(
            &store,
            "resource-index-orphan-after-put",
            "orphan.txt",
            "text/plain",
            CONTENT,
        )
        .is_err()
    );

    let index_blob = resource_index_blob_ref(CONTENT);
    assert_eq!(
        resource_index_registry_state(&directory, &index_blob).as_deref(),
        Some("RESERVED"),
        "a SQL rollback must leave the durable write intent unconsumed"
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &index_blob)
            .expect("the injected failure happens after the encrypted blob write"),
        CONTENT
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(
                WORKSPACE,
                BlobPurpose::Resource,
                &BlobRef {
                    digest: digest(CONTENT),
                    size_bytes: CONTENT.len() as u64,
                    media_type: "text/plain".to_owned(),
                },
            )
            .expect("the index collector must not delete the same digest in RESOURCE"),
        CONTENT
    );

    Connection::open(state_database(&directory))
        .expect("open ResourceIndex trigger fixture")
        .execute_batch("DROP TRIGGER reject_resource_index_projection;")
        .expect("remove the injected failure before reopening the schema-verified store");
    drop(store);
    let restarted = open_store(&directory, file_blob_store(&directory));
    assert_eq!(
        restarted
            .collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100)
            .expect("restart collector removes the failed projection's orphan"),
        1
    );
    assert_eq!(resource_index_registry_state(&directory, &index_blob), None);
    assert!(matches!(
        restarted
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &index_blob),
        Err(StoreError::NotFound)
    ));
    assert_eq!(
        restarted
            .inner
            .blobs
            .get(
                WORKSPACE,
                BlobPurpose::Resource,
                &BlobRef {
                    digest: digest(CONTENT),
                    size_bytes: CONTENT.len() as u64,
                    media_type: "text/plain".to_owned(),
                },
            )
            .expect("purpose-scoped Resource content remains available"),
        CONTENT
    );
}

#[test]
fn expired_resource_index_reservation_without_a_published_object_is_reclaimed() {
    let directory = tempfile::tempdir().expect("temporary unpublished ResourceIndex directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    let blob = resource_index_blob_ref(b"reserved before put, then process stopped");
    store
        .reserve_resource_index_snapshot(WORKSPACE, &blob)
        .expect("persist intent before a provider put");

    assert_eq!(
        resource_index_registry_state(&directory, &blob).as_deref(),
        Some("RESERVED")
    );
    assert_eq!(
        store
            .collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100)
            .expect("expired reservation is reclaimed even when put never published"),
        1
    );
    assert_eq!(resource_index_registry_state(&directory, &blob), None);
    assert!(matches!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &blob),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn resource_index_gc_keeps_shared_snapshot_until_every_revision_reference_is_removed() {
    const CONTENT: &[u8] = b"shared exact ResourceIndex snapshot";
    let directory = tempfile::tempdir().expect("temporary shared ResourceIndex directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_resource_with_content(
        &store,
        "resource-index-shared-a",
        "shared-a.txt",
        "text/plain",
        CONTENT,
    )
    .expect("first Resource references the encrypted snapshot");
    create_resource_with_content(
        &store,
        "resource-index-shared-b",
        "shared-b.txt",
        "text/plain",
        CONTENT,
    )
    .expect("second Resource reuses the exact content-addressed snapshot");
    let index_blob = resource_index_blob_ref(CONTENT);

    assert_eq!(
        store
            .collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100)
            .expect("referenced shared snapshots are not collected"),
        0
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect references");
    let references: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_text_indexes WHERE workspace_id = ?1 AND extracted_blob_digest = ?2",
            params![WORKSPACE, index_blob.digest],
            |row| row.get(0),
        )
        .expect("count every Resource revision reference");
    assert_eq!(references, 2);
    connection
        .execute(
            "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, "resource-index-shared-a"],
        )
        .expect("remove one derived revision projection");
    drop(connection);

    assert_eq!(
        store
            .collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100)
            .expect("the second Resource still roots the shared blob"),
        0
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &index_blob)
            .expect("the remaining revision keeps the shared snapshot"),
        CONTENT
    );

    Connection::open(state_database(&directory))
        .expect("open final ResourceIndex projection")
        .execute(
            "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, "resource-index-shared-b"],
        )
        .expect("remove the final derived revision projection");
    assert_eq!(
        store
            .collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100)
            .expect("unreferenced tracked snapshots are collected"),
        1
    );
    assert_eq!(resource_index_registry_state(&directory, &index_blob), None);
}

#[test]
fn resource_index_gc_retains_snapshot_referenced_only_by_a_historical_revision() {
    const ZIP: &[u8] = b"PK\x03\x04new unindexed revision";
    let directory = tempfile::tempdir().expect("temporary historical ResourceIndex root directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_revision_base_resource(
        &store,
        "resource-index-historical-root",
        "revision-upload-revision-create",
    );
    let index_blob = resource_index_blob_ref(CHUNK);
    create_revision_upload_with_content(
        &store,
        "upload-index-historical-root",
        "request-index-historical-root",
        "resource-index-historical-root",
        "application/zip",
        ZIP,
    )
    .expect("append an unindexed ZIP head");
    store
        .put_chunk(chunk_input_with_content(
            "upload-index-historical-root",
            "request-index-historical-root-chunk",
            ZIP,
        ))
        .expect("accept ZIP bytes");
    commit_unindexable_revision_upload(
        &store,
        "upload-index-historical-root",
        "resource-index-historical-root",
        "revision-index-historical-zip",
        ZIP,
    )
    .expect("advance the Resource head without adding a text projection");

    let connection = Connection::open(state_database(&directory)).expect("inspect historical row");
    let (current_revision, historical_references): (String, i64) = connection
        .query_row(
            "SELECT resources.current_revision_id,
                    (SELECT COUNT(*) FROM resource_text_indexes idx
                     WHERE idx.workspace_id = resources.workspace_id
                       AND idx.resource_id = resources.resource_id
                       AND idx.extracted_blob_digest = ?1)
             FROM resources WHERE workspace_id = ?2 AND resource_id = ?3",
            params![
                index_blob.digest,
                WORKSPACE,
                "resource-index-historical-root"
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read current head and historical index reference");
    drop(connection);
    assert_eq!(current_revision, "revision-index-historical-zip");
    assert_eq!(historical_references, 1);
    assert_eq!(
        store
            .collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100)
            .expect("historical index rows remain GC roots"),
        0
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &index_blob)
            .expect("historical revision keeps its encrypted snapshot"),
        CHUNK
    );
}

#[test]
fn resource_index_gc_preserves_deleting_fence_after_remove_failure_and_retries() {
    const CONTENT: &[u8] = b"retry ResourceIndex deletion after restart";
    let directory = tempfile::tempdir().expect("temporary ResourceIndex delete retry directory");
    let store = open_store(&directory, paused_blob_store(&directory, None, None, true));
    create_workspace_for_uploads(&store);
    create_resource_with_content(
        &store,
        "resource-index-delete-retry",
        "delete-retry.txt",
        "text/plain",
        CONTENT,
    )
    .expect("Resource snapshot is tracked");
    let index_blob = resource_index_blob_ref(CONTENT);
    Connection::open(state_database(&directory))
        .expect("open projection for simulated purge")
        .execute(
            "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, "resource-index-delete-retry"],
        )
        .expect("remove the derived projection");

    assert!(matches!(
        store.collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100),
        Err(StoreError::Blob(_))
    ));
    assert_eq!(
        resource_index_registry_state(&directory, &index_blob).as_deref(),
        Some("DELETING"),
        "failed exact-object deletion must preserve its durable fence"
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &index_blob)
            .expect("the injected provider error leaves the object intact"),
        CONTENT
    );

    drop(store);
    let restarted = open_store(&directory, file_blob_store(&directory));
    assert_eq!(
        restarted
            .collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100)
            .expect("restart retries the fenced exact-object deletion"),
        1
    );
    assert_eq!(resource_index_registry_state(&directory, &index_blob), None);
    assert!(matches!(
        restarted
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &index_blob),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn resource_index_gc_retries_after_object_removal_was_acknowledgement_ambiguous() {
    const CONTENT: &[u8] = b"ResourceIndex removal acknowledgement lost after success";
    let directory =
        tempfile::tempdir().expect("temporary ResourceIndex ambiguous-remove directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_resource_with_content(
        &store,
        "resource-index-remove-ack-lost",
        "remove-ack-lost.txt",
        "text/plain",
        CONTENT,
    )
    .expect("ResourceIndex snapshot is tracked");
    let blob = resource_index_blob_ref(CONTENT);

    Connection::open(state_database(&directory))
        .expect("open projection for simulated purge")
        .execute(
            "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, "resource-index-remove-ack-lost"],
        )
        .expect("remove derived projection before collecting its snapshot");
    Connection::open(state_database(&directory))
        .expect("open ResourceIndex deletion fence")
        .execute(
            "UPDATE resource_index_blob_registry
             SET state = 'DELETING', expires_at = NULL, claimed_at = ?1
             WHERE workspace_id = ?2 AND digest = ?3 AND state = 'TRACKED'",
            params!["2099-01-01T00:00:00Z", WORKSPACE, blob.digest],
        )
        .expect("persist the collector claim before simulating process stop");
    store
        .inner
        .blobs
        .remove(WORKSPACE, BlobPurpose::ResourceIndex, &blob)
        .expect("simulate successful physical deletion before the process stops");
    assert_eq!(
        resource_index_registry_state(&directory, &blob).as_deref(),
        Some("DELETING")
    );

    drop(store);
    let restarted = open_store(&directory, file_blob_store(&directory));
    assert_eq!(
        restarted
            .collect_orphan_resource_index_blobs("2099-01-01T00:00:01Z", 100)
            .expect("missing-object remove is idempotent and releases the persisted fence"),
        1
    );
    assert_eq!(resource_index_registry_state(&directory, &blob), None);
}

#[test]
fn resource_index_gc_fences_an_expired_paused_writer_before_projection_commit() {
    const CONTENT: &[u8] = b"in-flight ResourceIndex snapshot versus expired reservation GC";
    let directory = tempfile::tempdir().expect("temporary ResourceIndex writer/GC race directory");
    let pause = ResourceIndexBlobPutPause {
        armed: Arc::new(AtomicBool::new(false)),
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        first_only: Arc::new(AtomicBool::new(true)),
        digests: Arc::new(Mutex::new(Vec::new())),
    };
    let blobs: Arc<dyn BlobStore> = Arc::new(PausedResourceIndexBlobStore {
        inner: file_blob_store(&directory),
        pause: pause.clone(),
        get_pause: None,
        token_pause: None,
    });
    let store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    pause.armed.store(true, Ordering::SeqCst);

    let writer_store = store.clone();
    let (writer_sender, writer_receiver) = std::sync::mpsc::sync_channel(1);
    let writer = std::thread::spawn(move || {
        let result = create_resource_with_content(
            &writer_store,
            "resource-index-expired-writer-race",
            "expired-writer-race.txt",
            "text/plain",
            CONTENT,
        );
        writer_sender
            .send(result)
            .expect("writer result receiver remains open");
    });
    pause.entered.wait();

    let blob = resource_index_blob_ref(CONTENT);
    Connection::open(state_database(&directory))
        .expect("open paused ResourceIndex reservation")
        .execute(
            "UPDATE resource_index_blob_registry SET expires_at = ?1
             WHERE workspace_id = ?2 AND digest = ?3 AND state = 'RESERVED'",
            params!["2000-01-01T00:00:00Z", WORKSPACE, blob.digest],
        )
        .expect("make the in-flight reservation eligible for the deterministic race");

    let collector_store = store.clone();
    let (collector_sender, collector_receiver) = std::sync::mpsc::sync_channel(1);
    let collector = std::thread::spawn(move || {
        let result =
            collector_store.collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100);
        collector_sender
            .send(result)
            .expect("collector result receiver remains open");
    });
    let claim_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while resource_index_registry_state(&directory, &blob).as_deref() != Some("DELETING")
        && std::time::Instant::now() < claim_deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        resource_index_registry_state(&directory, &blob).as_deref(),
        Some("DELETING"),
        "collector durably fences the expired reservation before waiting on the writer mutex"
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &blob)
            .expect("collector waits for the in-flight writer mutex before physical deletion"),
        CONTENT
    );

    pause.release.wait();
    assert!(
        writer_receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("paused writer completes after release")
            .is_err(),
        "a DELETING fence prevents the paused writer from committing a projection"
    );
    writer.join().expect("writer thread joins");
    assert_eq!(
        collector_receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("collector completes after writer releases its lock")
            .expect("collector removes the fenced orphan"),
        1
    );
    collector.join().expect("collector thread joins");
    assert_eq!(resource_index_registry_state(&directory, &blob), None);
    assert!(matches!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceIndex, &blob),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn resource_index_search_holds_snapshot_until_materialization_finishes() {
    const CONTENT: &[u8] = b"search snapshot remains readable during derived-row purge";
    let directory = tempfile::tempdir().expect("temporary ResourceIndex search/GC race directory");
    let get_pause = ResourceIndexBlobGetPause {
        armed: Arc::new(AtomicBool::new(false)),
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let blobs: Arc<dyn BlobStore> = Arc::new(PausedResourceIndexBlobStore {
        inner: file_blob_store(&directory),
        pause: ResourceIndexBlobPutPause {
            armed: Arc::new(AtomicBool::new(false)),
            entered: Arc::new(Barrier::new(2)),
            release: Arc::new(Barrier::new(2)),
            first_only: Arc::new(AtomicBool::new(true)),
            digests: Arc::new(Mutex::new(Vec::new())),
        },
        get_pause: Some(get_pause.clone()),
        token_pause: None,
    });
    let store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    create_resource_with_content(
        &store,
        "resource-index-search-gc-race",
        "search-gc-race.txt",
        "text/plain",
        CONTENT,
    )
    .expect("ResourceIndex snapshot is searchable");
    let blob = resource_index_blob_ref(CONTENT);

    get_pause.armed.store(true, Ordering::SeqCst);
    let search_store = store.clone();
    let (search_sender, search_receiver) = std::sync::mpsc::sync_channel(1);
    let search = std::thread::spawn(move || {
        let result = search_store.search_indexed_resource_text(
            WORKSPACE,
            "snapshot remains readable",
            None,
            None,
            None,
            None,
            10,
        );
        search_sender
            .send(result)
            .expect("search result receiver remains open");
    });
    get_pause.entered.wait();

    Connection::open(state_database(&directory))
        .expect("open derived projection for simulated purge")
        .execute(
            "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, "resource-index-search-gc-race"],
        )
        .expect("purge the candidate reference after search selected its snapshot");
    let collector_store = store.clone();
    let (collector_sender, collector_receiver) = std::sync::mpsc::sync_channel(1);
    let collector = std::thread::spawn(move || {
        let result =
            collector_store.collect_orphan_resource_index_blobs("2099-01-01T00:00:00Z", 100);
        collector_sender
            .send(result)
            .expect("collector result receiver remains open");
    });
    let claim_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while resource_index_registry_state(&directory, &blob).as_deref() != Some("DELETING")
        && std::time::Instant::now() < claim_deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    let deletion_claimed =
        resource_index_registry_state(&directory, &blob).as_deref() == Some("DELETING");
    let object_survived_during_read = store
        .inner
        .blobs
        .get(WORKSPACE, BlobPurpose::ResourceIndex, &blob)
        .is_ok();

    get_pause.release.wait();
    let search_results = search_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("search completes after releasing the snapshot read")
        .expect("collector cannot remove a snapshot while search materializes it");
    search.join().expect("search thread joins");
    let removed = collector_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("collector completes after snapshot materialization releases its lock")
        .expect("collector removes the now-unreferenced snapshot");
    collector.join().expect("collector thread joins");

    assert!(
        deletion_claimed,
        "collector persists the deleting fence before waiting"
    );
    assert!(object_survived_during_read);
    assert!(
        search_results.items.is_empty(),
        "the final reference check suppresses a candidate purged while it was materializing"
    );
    assert_eq!(removed, 1);
    assert_eq!(resource_index_registry_state(&directory, &blob), None);
}

#[test]
fn indexed_search_paginates_past_stale_candidate_during_materialization() {
    const FIRST_RESOURCE: &str = "resource-pagination-c";
    const SECOND_RESOURCE: &str = "resource-pagination-b";
    const THIRD_RESOURCE: &str = "resource-pagination-a";
    const FIRST_UPLOAD: &str = "upload-pagination-first-revision";
    const FIRST_NEXT_REVISION: &str = "revision-pagination-first-next";
    const REPLACEMENT_CONTENT: &[u8] = b"PK\x03\x04opaque content no longer matching search";

    let directory = tempfile::tempdir().expect("temporary indexed search pagination directory");
    let get_pause = ResourceIndexBlobGetPause {
        armed: Arc::new(AtomicBool::new(false)),
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let blobs: Arc<dyn BlobStore> = Arc::new(PausedResourceIndexBlobStore {
        inner: file_blob_store(&directory),
        pause: ResourceIndexBlobPutPause {
            armed: Arc::new(AtomicBool::new(false)),
            entered: Arc::new(Barrier::new(2)),
            release: Arc::new(Barrier::new(2)),
            first_only: Arc::new(AtomicBool::new(true)),
            digests: Arc::new(Mutex::new(Vec::new())),
        },
        get_pause: Some(get_pause.clone()),
        token_pause: None,
    });
    let store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);

    // The first candidate needs an exact parent revision so a real revision upload
    // can move its head while search is paused in snapshot materialization.
    create_revision_base_resource(&store, FIRST_RESOURCE, "revision-upload-revision-create");
    create_resource_with_content(
        &store,
        SECOND_RESOURCE,
        "pagination-second.txt",
        "text/plain",
        b"second upload chunk remains searchable",
    )
    .expect("second indexed Resource is searchable");
    create_resource_with_content(
        &store,
        THIRD_RESOURCE,
        "pagination-third.txt",
        "text/plain",
        b"third upload chunk remains searchable",
    )
    .expect("third indexed Resource is searchable");

    let initial = store
        .search_indexed_resource_text(WORKSPACE, "upload chunk", None, None, None, None, 3)
        .expect("three current matching Resources are searchable");
    assert_eq!(
        initial
            .items
            .iter()
            .map(|item| item.result.summary.resource_id.as_str())
            .collect::<Vec<_>>(),
        vec![FIRST_RESOURCE, SECOND_RESOURCE, THIRD_RESOURCE],
        "fixture order must make the subsequently stale Resource the first candidate"
    );

    create_revision_upload_with_content(
        &store,
        FIRST_UPLOAD,
        "request-pagination-first-revision",
        FIRST_RESOURCE,
        "application/zip",
        REPLACEMENT_CONTENT,
    )
    .expect("prepare a real revision upload for the first candidate");
    store
        .put_chunk(chunk_input_with_content(
            FIRST_UPLOAD,
            "request-pagination-first-revision-chunk",
            REPLACEMENT_CONTENT,
        ))
        .expect("receive replacement bytes before search begins");

    get_pause.armed.store(true, Ordering::SeqCst);
    let search_store = store.clone();
    let (search_sender, search_receiver) = std::sync::mpsc::sync_channel(1);
    let search = std::thread::spawn(move || {
        let result = search_store.search_indexed_resource_text(
            WORKSPACE,
            "upload chunk",
            None,
            None,
            None,
            None,
            2,
        );
        search_sender
            .send(result)
            .expect("search result receiver remains open");
    });
    get_pause.entered.wait();

    let advance_result = commit_unindexable_revision_upload(
        &store,
        FIRST_UPLOAD,
        FIRST_RESOURCE,
        FIRST_NEXT_REVISION,
        REPLACEMENT_CONTENT,
    );
    get_pause.release.wait();
    let search_results = search_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("search completes after releasing the paused snapshot read")
        .expect("search materializes candidates after the concurrent revision commit");
    search.join().expect("search thread joins");
    let advanced = advance_result.expect("first candidate advances to a non-matching revision");

    assert_eq!(advanced.resource.version, 2);
    assert_eq!(
        advanced.resource.current_revision_id.as_deref(),
        Some(FIRST_NEXT_REVISION)
    );
    assert_eq!(
        search_results
            .items
            .iter()
            .map(|item| item.result.summary.resource_id.as_str())
            .collect::<Vec<_>>(),
        vec![SECOND_RESOURCE, THIRD_RESOURCE],
        "a stale first page candidate must be omitted without stranding the next match"
    );
    assert_eq!(
        search_results.items.len(),
        2,
        "both current matches must be returned"
    );
    assert_ne!(
        search_results.items[0].result.summary.resource_id,
        search_results.items[1].result.summary.resource_id,
        "pagination must not duplicate a Resource"
    );
    for result in &search_results.items {
        let current = store
            .get_resource_record(WORKSPACE, &result.result.summary.resource_id)
            .expect("current Resource head lookup succeeds")
            .expect("matching Resource remains present");
        assert_eq!(
            result.resource_revision_id,
            current
                .current_revision_id
                .expect("Resource has a current revision"),
            "every returned match must pin the current revision"
        );
    }
}

#[test]
fn indexed_search_returns_progressing_cursor_when_candidate_budget_underfills_page() {
    const RESOURCE_COUNT: usize = 201;

    let directory = tempfile::tempdir().expect("temporary bounded indexed-search directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);

    for index in 0..RESOURCE_COUNT {
        create_resource_with_content(
            &store,
            &format!("resource-search-candidate-budget-{index:03}"),
            &format!("candidate-budget-{index:03}.txt"),
            "text/plain",
            b"needle remains current and searchable",
        )
        .expect("bounded indexed Resource commits");
    }

    let first_page = store
        .search_indexed_resource_text(WORKSPACE, "needle", None, None, None, None, 201)
        .expect("indexed search stops at its verified candidate-count budget");
    assert_eq!(first_page.items.len(), 200);
    let frontier = first_page
        .continuation
        .expect("an underfilled bounded scan returns its advancing frontier");
    assert_eq!(
        frontier.created_at,
        first_page.items.last().unwrap().result.summary.created_at
    );
    assert_eq!(
        frontier.resource_id,
        first_page.items.last().unwrap().result.summary.resource_id
    );

    let second_page = store
        .search_indexed_resource_text(
            WORKSPACE,
            "needle",
            None,
            None,
            Some(&frontier.created_at),
            Some(&frontier.resource_id),
            201,
        )
        .expect("search resumes strictly after the committed frontier");
    assert_eq!(second_page.items.len(), 1);
    assert!(second_page.continuation.is_none());

    let resource_ids = first_page
        .items
        .iter()
        .chain(second_page.items.iter())
        .map(|item| item.result.summary.resource_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(resource_ids.len(), RESOURCE_COUNT);
    assert_eq!(
        resource_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        RESOURCE_COUNT,
        "bounded continuation must neither skip nor duplicate a matching Resource"
    );
}

#[test]
fn indexed_search_returns_cursor_when_full_page_has_remaining_candidates() {
    const RESOURCE_COUNT: usize = 3;

    let directory = tempfile::tempdir().expect("temporary full indexed-search page directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);

    for index in 0..RESOURCE_COUNT {
        create_resource_with_content(
            &store,
            &format!("resource-search-full-page-{index:03}"),
            &format!("full-page-{index:03}.txt"),
            "text/plain",
            b"needle remains current and searchable",
        )
        .expect("indexed Resource commits");
    }

    let first_page = store
        .search_indexed_resource_text(WORKSPACE, "needle", None, None, None, None, 2)
        .expect("indexed search returns a full first page");
    assert_eq!(first_page.items.len(), 2);
    let frontier = first_page
        .continuation
        .expect("full page still reports a cursor when another candidate remains");
    let last = first_page
        .items
        .last()
        .expect("first page has a final item");
    assert_eq!(frontier.created_at, last.result.summary.created_at);
    assert_eq!(frontier.resource_id, last.result.summary.resource_id);

    let second_page = store
        .search_indexed_resource_text(
            WORKSPACE,
            "needle",
            None,
            None,
            Some(&frontier.created_at),
            Some(&frontier.resource_id),
            2,
        )
        .expect("search resumes strictly after the full-page frontier");
    assert_eq!(second_page.items.len(), 1);
    assert!(second_page.continuation.is_none());

    let resource_ids = first_page
        .items
        .iter()
        .chain(second_page.items.iter())
        .map(|item| item.result.summary.resource_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(resource_ids.len(), RESOURCE_COUNT);
    assert_eq!(
        resource_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        RESOURCE_COUNT,
        "full-page continuation must neither skip nor duplicate a matching Resource"
    );
}

#[test]
fn indexed_search_returns_progressing_cursor_when_snapshot_byte_budget_underfills_page() {
    const RESOURCE_COUNT: usize = 65;
    const SNAPSHOT_BYTES: usize = 1_048_576;

    let directory = tempfile::tempdir().expect("temporary bounded indexed-search directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    let mut content = vec![b' '; SNAPSHOT_BYTES];
    for offset in (0..SNAPSHOT_BYTES - b"needle".len()).step_by(128) {
        content[offset..offset + b"needle".len()].copy_from_slice(b"needle");
    }
    content[SNAPSHOT_BYTES - 1] = b'z';

    for index in 0..RESOURCE_COUNT {
        create_resource_with_content(
            &store,
            &format!("resource-search-byte-budget-{index:03}"),
            &format!("byte-budget-{index:03}.txt"),
            "text/plain",
            &content,
        )
        .expect("max-size indexed Resource commits");
    }

    let first_page = store
        .search_indexed_resource_text(WORKSPACE, "needle", None, None, None, None, 100)
        .expect("indexed search stops at its verified snapshot-byte budget");
    assert_eq!(first_page.items.len(), 64);
    let frontier = first_page
        .continuation
        .expect("byte-bounded underfilled page returns its advancing frontier");
    assert_eq!(
        frontier.created_at,
        first_page.items.last().unwrap().result.summary.created_at
    );
    assert_eq!(
        frontier.resource_id,
        first_page.items.last().unwrap().result.summary.resource_id
    );

    let second_page = store
        .search_indexed_resource_text(
            WORKSPACE,
            "needle",
            None,
            None,
            Some(&frontier.created_at),
            Some(&frontier.resource_id),
            100,
        )
        .expect("search resumes strictly after the verified-byte frontier");
    assert_eq!(second_page.items.len(), 1);
    assert!(second_page.continuation.is_none());

    let resource_ids = first_page
        .items
        .iter()
        .chain(second_page.items.iter())
        .map(|item| item.result.summary.resource_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(resource_ids.len(), RESOURCE_COUNT);
    assert_eq!(
        resource_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        RESOURCE_COUNT,
        "byte-bounded continuation must neither skip nor duplicate a matching Resource"
    );
}

#[test]
fn resource_index_v19_migration_backfills_shared_existing_snapshot_references() {
    const CONTENT: &[u8] = b"legacy ResourceIndex snapshot shared across revisions";
    let directory = tempfile::tempdir().expect("temporary ResourceIndex backfill directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_resource_with_content(
        &store,
        "resource-index-backfill-a",
        "backfill-a.txt",
        "text/plain",
        CONTENT,
    )
    .expect("first Resource points to the snapshot");
    create_resource_with_content(
        &store,
        "resource-index-backfill-b",
        "backfill-b.txt",
        "text/plain",
        CONTENT,
    )
    .expect("second Resource shares the snapshot");
    drop(store);

    let mut connection = Connection::open(state_database(&directory)).expect("open v19 database");
    downgrade_resource_index_schema_to_v18(&connection);
    migrate(&mut connection).expect("v19 migration backfills existing ResourceIndex roots");

    let blob = resource_index_blob_ref(CONTENT);
    let (size, state, references): (i64, String, i64) = connection
        .query_row(
            "SELECT registry.size_bytes, registry.state,
                    (SELECT COUNT(*) FROM resource_text_indexes idx
                     WHERE idx.workspace_id = registry.workspace_id
                       AND idx.extracted_blob_digest = registry.digest)
             FROM resource_index_blob_registry registry
             WHERE registry.workspace_id = ?1 AND registry.digest = ?2",
            params![WORKSPACE, blob.digest],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read backfilled shared snapshot root");
    assert_eq!(size, CONTENT.len() as i64);
    assert_eq!(state, "TRACKED");
    assert_eq!(references, 2);
}

fn blob_object_count(directory: &tempfile::TempDir) -> usize {
    fn count_files(path: &std::path::Path) -> usize {
        std::fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| {
                        let file_type = entry.file_type().expect("inspect blob fixture entry");
                        if file_type.is_dir() {
                            count_files(&entry.path())
                        } else if file_type.is_file() {
                            1
                        } else {
                            0
                        }
                    })
                    .sum()
            })
            .unwrap_or(0)
    }

    count_files(&directory.path().join("blobs"))
}

#[test]
fn unindexable_resource_create_and_rebuild_skip_unavailable_index_key() {
    const ZIP: &[u8] = b"PK\x03\x04unparsed archive";
    let directory = tempfile::tempdir().expect("temporary Resource index key directory");
    let available_store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&available_store);
    create_revision_base_resource(
        &available_store,
        "resource-index-key-seed",
        "revision-index-key-seed",
    );
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-index-key-seed"),
        (1, 3),
        "seed an existing index so unavailable retained key history is exercised"
    );
    drop(available_store);

    let index_lookups = Arc::new(AtomicUsize::new(0));
    let store = store_without_resource_index_keys(&directory, Arc::clone(&index_lookups));
    let committed = create_resource_with_content(
        &store,
        "resource-unindexable-create",
        "archive.zip",
        "application/zip",
        ZIP,
    )
    .expect("unsupported ZIP remains an ordinary Resource without index keys");
    assert_eq!(
        committed.resource.resource_id,
        "resource-unindexable-create"
    );
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-unindexable-create"),
        (0, 0)
    );
    assert_eq!(
        index_lookups.load(Ordering::SeqCst),
        0,
        "unindexable creation must skip the unavailable retained-key check"
    );

    let rebuild = ResourceTextIndexRebuildRequest {
        principal_id: OWNER.to_owned(),
        request_id: "request-rebuild-unindexable-zip".to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        resource_id: "resource-unindexable-create".to_owned(),
        resource_revision_id: "revision-resource-unindexable-create".to_owned(),
        content_digest: digest(ZIP),
        indexed_at: CREATED.to_owned(),
        correlation_id: "correlation-rebuild-unindexable-zip".to_owned(),
    };
    let first = store
        .reindex_resource_text(rebuild.clone())
        .expect("explicit rebuild records a NOT_INDEXABLE result");
    assert_eq!(first.outcome, ResourceTextIndexRebuildOutcome::NotIndexable);
    assert_eq!(
        first.reason,
        Some(ResourceTextIndexSkipReason::UnsupportedType)
    );
    let replay = store
        .reindex_resource_text(rebuild)
        .expect("exact rebuild retry replays its committed result");
    assert_eq!(replay, first);
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-unindexable-create"),
        (0, 0),
        "explicit unsupported rebuild must not create index rows"
    );
    let receipts: i64 = Connection::open(state_database(&directory))
        .expect("open receipt database")
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            params![OWNER, "request-rebuild-unindexable-zip"],
            |row| row.get(0),
        )
        .expect("count rebuild receipt");
    assert_eq!(receipts, 1);
    assert_eq!(
        index_lookups.load(Ordering::SeqCst),
        0,
        "unsupported explicit rebuild must not request an index key"
    );
}

#[test]
fn unindexable_resource_upload_commit_skips_unavailable_index_key() {
    const ZIP: &[u8] = b"PK\x03\x04opaque uploaded archive";
    let directory = tempfile::tempdir().expect("temporary Resource upload key directory");
    let available_store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&available_store);
    create_revision_base_resource(
        &available_store,
        "resource-upload-index-key-seed",
        "revision-upload-index-key-seed",
    );
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-upload-index-key-seed"),
        (1, 3),
        "seed an existing ResourceIndex version before removing its key"
    );
    drop(available_store);

    let index_lookups = Arc::new(AtomicUsize::new(0));
    let store = store_without_resource_index_keys(&directory, Arc::clone(&index_lookups));
    create_upload_with_content(
        &store,
        "upload-unindexable-zip",
        "archive.zip",
        "application/zip",
        ZIP,
    );
    store
        .put_chunk(chunk_input_with_content(
            "upload-unindexable-zip",
            "request-chunk-unindexable-zip",
            ZIP,
        ))
        .expect("encrypted ZIP chunk is accepted");

    let committed = commit_upload_with_content(
        &store,
        "upload-unindexable-zip",
        "resource-upload-unindexable-zip",
        "archive.zip",
        "application/zip",
        ZIP,
    )
    .expect("unsupported ZIP upload commits without index keys");
    assert_eq!(
        committed.resource.resource_id,
        "resource-upload-unindexable-zip"
    );
    assert_eq!(committed.session.state, ResourceUploadState::Committed);
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-upload-unindexable-zip"),
        (0, 0)
    );
    assert_eq!(
        index_lookups.load(Ordering::SeqCst),
        0,
        "unindexable upload commit must skip the unavailable retained-key check"
    );
}

#[test]
fn eligible_text_fails_closed_when_resource_index_key_is_unavailable() {
    let directory = tempfile::tempdir().expect("temporary eligible Resource key directory");
    let available_store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&available_store);
    create_revision_base_resource(
        &available_store,
        "resource-eligible-index-key-seed",
        "revision-eligible-index-key-seed",
    );
    drop(available_store);

    let index_lookups = Arc::new(AtomicUsize::new(0));
    let store = store_without_resource_index_keys(&directory, Arc::clone(&index_lookups));
    let direct_create_blob_count = blob_object_count(&directory);
    let result = create_resource_with_content(
        &store,
        "resource-eligible-index-key-denied",
        "eligible.txt",
        "text/plain",
        b"eligible source must not commit without index key access",
    );
    assert!(
        matches!(result, Err(StoreError::Blob(_))),
        "eligible content must fail closed when its retained index key is unavailable"
    );
    assert!(index_lookups.load(Ordering::SeqCst) > 0);

    let connection = Connection::open(state_database(&directory)).expect("inspect SQLite");
    let resource_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, "resource-eligible-index-key-denied"],
            |row| row.get(0),
        )
        .expect("count candidate Resource");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_type = 'Resource' AND entity_id = ?2 AND type = 'resource.created.v1'",
            params![WORKSPACE, "resource-eligible-index-key-denied"],
            |row| row.get(0),
        )
        .expect("count candidate Resource event");
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            params![
                OWNER,
                "request-resource-create-resource-eligible-index-key-denied"
            ],
            |row| row.get(0),
        )
        .expect("count candidate Resource receipt");
    assert_eq!((resource_count, event_count, receipt_count), (0, 0, 0));
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-eligible-index-key-denied"),
        (0, 0)
    );
    assert_eq!(
        blob_object_count(&directory),
        direct_create_blob_count,
        "missing index key must be detected before candidate Resource blobs are written"
    );
    drop(connection);

    const ELIGIBLE_UPLOAD: &[u8] = b"eligible uploaded text requires its Workspace index key";
    create_upload_with_content(
        &store,
        "upload-eligible-index-key-denied",
        "eligible-upload.txt",
        "text/plain",
        ELIGIBLE_UPLOAD,
    );
    store
        .put_chunk(chunk_input_with_content(
            "upload-eligible-index-key-denied",
            "request-chunk-eligible-index-key-denied",
            ELIGIBLE_UPLOAD,
        ))
        .expect("eligible uploaded text reaches CONTENT_RECEIVED");
    let upload_commit_blob_count = blob_object_count(&directory);
    assert!(matches!(
        commit_upload_with_content(
            &store,
            "upload-eligible-index-key-denied",
            "resource-eligible-upload-index-key-denied",
            "eligible-upload.txt",
            "text/plain",
            ELIGIBLE_UPLOAD,
        ),
        Err(StoreError::Blob(_))
    ));
    let pending_upload = store
        .get(WORKSPACE, "upload-eligible-index-key-denied")
        .expect("pending upload state reads")
        .expect("upload session remains present");
    assert_eq!(pending_upload.state, ResourceUploadState::ContentReceived);
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-eligible-upload-index-key-denied"),
        (0, 0)
    );
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-eligible-index-key-seed"),
        (1, 3),
        "denied create/commit must leave the pre-existing indexed Resource intact"
    );
    assert_eq!(
        blob_object_count(&directory),
        upload_commit_blob_count,
        "missing index key must be detected before candidate commit blobs are written"
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect upload state");
    let upload_resource_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, "resource-eligible-upload-index-key-denied"],
            |row| row.get(0),
        )
        .expect("count uncommitted upload Resource");
    let upload_event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_type = 'Resource' AND entity_id = ?2",
            params![WORKSPACE, "resource-eligible-upload-index-key-denied"],
            |row| row.get(0),
        )
        .expect("count uncommitted upload Resource events");
    let upload_receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            params![
                OWNER,
                "resource-upload-commit:upload-eligible-index-key-denied"
            ],
            |row| row.get(0),
        )
        .expect("count uncommitted upload commit receipt");
    assert_eq!(
        (
            upload_resource_count,
            upload_event_count,
            upload_receipt_count
        ),
        (0, 0, 0)
    );
    drop(connection);
    drop(store);

    let restored_store = open_store(&directory, file_blob_store(&directory));
    let retried = commit_upload_with_content(
        &restored_store,
        "upload-eligible-index-key-denied",
        "resource-eligible-upload-index-key-denied",
        "eligible-upload.txt",
        "text/plain",
        ELIGIBLE_UPLOAD,
    )
    .expect("same pending upload commits after the required key becomes available");
    assert_eq!(retried.session.state, ResourceUploadState::Committed);
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-eligible-upload-index-key-denied"),
        (1, 8)
    );
}

#[test]
fn first_eligible_resource_fails_before_blob_writes_when_index_key_is_unavailable() {
    let directory = tempfile::tempdir().expect("temporary first index key directory");
    let available_store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&available_store);
    drop(available_store);

    let index_lookups = Arc::new(AtomicUsize::new(0));
    let store = store_without_resource_index_keys(&directory, Arc::clone(&index_lookups));
    let blob_count_before = blob_object_count(&directory);
    assert!(matches!(
        create_resource_with_content(
            &store,
            "resource-first-index-key-denied",
            "first-index.txt",
            "text/plain",
            b"no index key has ever been provisioned for this Workspace",
        ),
        Err(StoreError::Blob(_))
    ));
    assert!(index_lookups.load(Ordering::SeqCst) > 0);
    assert_eq!(
        blob_object_count(&directory),
        blob_count_before,
        "current-key failure before initial index history must not leave candidate blobs"
    );
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-first-index-key-denied"),
        (0, 0)
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect SQLite");
    let candidate_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, "resource-first-index-key-denied"],
            |row| row.get(0),
        )
        .expect("count candidate Resource");
    assert_eq!(candidate_count, 0);

    const UPLOAD_BYTES: &[u8] = b"first indexed upload requires an unavailable current key";
    create_upload_with_content(
        &store,
        "upload-first-index-key-denied",
        "first-upload.txt",
        "text/plain",
        UPLOAD_BYTES,
    );
    store
        .put_chunk(chunk_input_with_content(
            "upload-first-index-key-denied",
            "request-chunk-first-index-key-denied",
            UPLOAD_BYTES,
        ))
        .expect("first-index upload reaches CONTENT_RECEIVED");
    let upload_blob_count_before = blob_object_count(&directory);
    assert!(matches!(
        commit_upload_with_content(
            &store,
            "upload-first-index-key-denied",
            "resource-first-index-upload-key-denied",
            "first-upload.txt",
            "text/plain",
            UPLOAD_BYTES,
        ),
        Err(StoreError::Blob(_))
    ));
    assert_eq!(
        blob_object_count(&directory),
        upload_blob_count_before,
        "current-key failure must precede candidate upload-commit BlobStore writes"
    );
    assert_eq!(
        store
            .get(WORKSPACE, "upload-first-index-key-denied")
            .expect("read first-index upload")
            .expect("upload remains retryable")
            .state,
        ResourceUploadState::ContentReceived
    );
}

#[test]
fn later_index_snapshot_key_failure_precedes_resource_and_upload_candidate_blobs() {
    let direct_directory =
        tempfile::tempdir().expect("temporary later-key direct-create directory");
    let direct_lookups = Arc::new(AtomicUsize::new(0));
    let direct_store = open_store(
        &direct_directory,
        Arc::new(FileBlobStore::new(
            direct_directory.path().join("blobs"),
            FailNthResourceIndexKeyLookup {
                index_lookups: Arc::clone(&direct_lookups),
                fail_on: 3,
            },
        )),
    );
    create_workspace_for_uploads(&direct_store);
    let direct_blob_count = blob_object_count(&direct_directory);
    assert!(matches!(
        create_resource_with_content(
            &direct_store,
            "resource-later-index-key-failure",
            "later-key.txt",
            "text/plain",
            b"the encrypted index snapshot key lookup can fail after token derivation",
        ),
        Err(StoreError::Blob(_))
    ));
    assert_eq!(direct_lookups.load(Ordering::SeqCst), 3);
    assert_eq!(
        blob_object_count(&direct_directory),
        direct_blob_count,
        "ResourceIndex encryption-key failure must precede Resource aggregate/content writes"
    );
    assert_eq!(
        resource_index_projection_counts(&direct_directory, "resource-later-index-key-failure"),
        (0, 0)
    );

    let upload_directory = tempfile::tempdir().expect("temporary later-key upload directory");
    let upload_lookups = Arc::new(AtomicUsize::new(0));
    let upload_store = open_store(
        &upload_directory,
        Arc::new(FileBlobStore::new(
            upload_directory.path().join("blobs"),
            FailNthResourceIndexKeyLookup {
                index_lookups: Arc::clone(&upload_lookups),
                fail_on: 3,
            },
        )),
    );
    create_workspace_for_uploads(&upload_store);
    const CONTENT: &[u8] = b"the upload path must preflight the encrypted snapshot key";
    create_upload_with_content(
        &upload_store,
        "upload-later-index-key-failure",
        "later-key-upload.txt",
        "text/plain",
        CONTENT,
    );
    upload_store
        .put_chunk(chunk_input_with_content(
            "upload-later-index-key-failure",
            "request-later-index-key-chunk",
            CONTENT,
        ))
        .expect("upload content is received before commit");
    let upload_blob_count = blob_object_count(&upload_directory);
    assert!(matches!(
        commit_upload_with_content(
            &upload_store,
            "upload-later-index-key-failure",
            "resource-upload-later-index-key-failure",
            "later-key-upload.txt",
            "text/plain",
            CONTENT,
        ),
        Err(StoreError::Blob(_))
    ));
    assert_eq!(upload_lookups.load(Ordering::SeqCst), 3);
    assert_eq!(
        blob_object_count(&upload_directory),
        upload_blob_count,
        "ResourceIndex encryption-key failure must precede upload commit candidate blobs"
    );
    assert_eq!(
        upload_store
            .get("workspace-upload-gc", "upload-later-index-key-failure")
            .expect("read retryable upload")
            .expect("upload remains present")
            .state,
        ResourceUploadState::ContentReceived
    );
}

#[test]
fn historical_index_key_versions_do_not_consume_searchable_bound() {
    let directory = tempfile::tempdir().expect("temporary historical index-version directory");
    let current_index_version = Arc::new(AtomicU32::new(1));
    let store = open_store(
        &directory,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            RotatingResourceIndexKeys {
                current_index_version: Arc::clone(&current_index_version),
            },
        )),
    );
    create_workspace_for_uploads(&store);

    const RESOURCE_ID: &str = "resource-historical-index-key-version";
    const BASE_REVISION_ID: &str = "revision-upload-revision-create";
    const UPLOAD_ID: &str = "upload-historical-index-key-version";
    create_revision_base_resource(&store, RESOURCE_ID, BASE_REVISION_ID);

    const ZIP_CONTENT: &[u8] = b"PK\x03\x04historical index version is no longer searchable";
    create_revision_upload_with_content(
        &store,
        UPLOAD_ID,
        "request-historical-index-key-version",
        RESOURCE_ID,
        "application/zip",
        ZIP_CONTENT,
    )
    .expect("revision upload pins the current Resource head");
    store
        .put_chunk(chunk_input_with_content(
            UPLOAD_ID,
            "request-historical-index-key-version-chunk",
            ZIP_CONTENT,
        ))
        .expect("unindexable revision bytes are accepted");
    commit_unindexable_revision_upload(
        &store,
        UPLOAD_ID,
        RESOURCE_ID,
        "revision-historical-index-key-version",
        ZIP_CONTENT,
    )
    .expect("unindexable revision advances the current head");

    for version in 2..=resource_index::MAX_SEARCHABLE_KEY_VERSIONS as u32 {
        current_index_version.store(version, Ordering::SeqCst);
        create_resource_with_content(
            &store,
            &format!("resource-current-index-key-version-{version}"),
            &format!("current-version-{version}.txt"),
            "text/plain",
            b"current searchable version bound",
        )
        .expect("current Resource is indexed under its active key version");
    }

    current_index_version.store(
        resource_index::MAX_SEARCHABLE_KEY_VERSIONS as u32 + 1,
        Ordering::SeqCst,
    );
    create_resource_with_content(
        &store,
        "resource-current-index-key-version-final",
        "current-final-version.txt",
        "text/plain",
        b"current searchable version bound",
    )
    .expect("historical version must not block the eighth current searchable version");

    assert_eq!(
        store
            .search_indexed_resource_text(
                WORKSPACE,
                "current searchable",
                None,
                None,
                None,
                None,
                20,
            )
            .expect("all current searchable versions remain available")
            .items
            .len(),
        resource_index::MAX_SEARCHABLE_KEY_VERSIONS
    );
}

#[test]
fn indexed_search_serializes_key_snapshot_with_reindex() {
    const RESOURCE_ID: &str = "resource-search-reindex-key-race";
    const CONTENT: &[u8] = b"the needle is present in this searchable resource";

    let directory = tempfile::tempdir().expect("temporary search/reindex race directory");
    let current_index_version = Arc::new(AtomicU32::new(1));
    let token_pause = ResourceIndexTokenPause {
        armed: Arc::new(AtomicBool::new(false)),
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let blob_put_pause = ResourceIndexBlobPutPause {
        armed: Arc::new(AtomicBool::new(false)),
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        first_only: Arc::new(AtomicBool::new(true)),
        digests: Arc::new(Mutex::new(Vec::new())),
    };
    let blobs: Arc<dyn BlobStore> = Arc::new(PausedResourceIndexBlobStore {
        inner: Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            RotatingResourceIndexKeys {
                current_index_version: Arc::clone(&current_index_version),
            },
        )),
        pause: blob_put_pause,
        get_pause: None,
        token_pause: Some(token_pause.clone()),
    });
    let store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    create_resource_with_content(&store, RESOURCE_ID, "needle.txt", "text/plain", CONTENT)
        .expect("current Resource is indexed under the first key version");
    current_index_version.store(2, Ordering::SeqCst);

    token_pause.armed.store(true, Ordering::SeqCst);
    let search_store = store.clone();
    let (search_sender, search_receiver) = std::sync::mpsc::sync_channel(1);
    let search = std::thread::spawn(move || {
        let result = search_store
            .search_indexed_resource_text(WORKSPACE, "needle", None, None, None, None, 10);
        search_sender
            .send(result)
            .expect("search result receiver remains open");
    });
    token_pause.entered.wait();

    let (contention_sender, contention_receiver) = std::sync::mpsc::sync_channel(1);
    store.observe_next_resource_index_write_contention(WORKSPACE, contention_sender);
    let rebuild_store = store.clone();
    let (rebuild_sender, rebuild_receiver) = std::sync::mpsc::sync_channel(1);
    let rebuild = std::thread::spawn(move || {
        let result = rebuild_store.reindex_resource_text(ResourceTextIndexRebuildRequest {
            principal_id: OWNER.to_owned(),
            request_id: "request-search-reindex-key-race".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            resource_id: RESOURCE_ID.to_owned(),
            resource_revision_id: format!("revision-{RESOURCE_ID}"),
            content_digest: digest(CONTENT),
            indexed_at: "2026-10-12T10:00:05Z".to_owned(),
            correlation_id: "correlation-search-reindex-key-race".to_owned(),
        });
        rebuild_sender
            .send(result)
            .expect("rebuild result receiver remains open");
    });
    let rebuild_observed_search_lock = contention_receiver
        .recv_timeout(Duration::from_secs(5))
        .is_ok();

    // Release both workers before making assertions so a regression cannot strand
    // a blocked test thread.
    token_pause.release.wait();
    let search_result = search_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("search completes after its token derivation is released")
        .expect("search succeeds against its captured current-head key version");
    let rebuild_result = rebuild_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("reindex completes after search releases the Workspace lock")
        .expect("reindex commits under the new active key version");
    search.join().expect("search thread joins");
    rebuild.join().expect("reindex thread joins");

    assert!(
        rebuild_observed_search_lock,
        "reindex must wait while search derives tokens and reads matching candidates"
    );
    assert_eq!(
        search_result.items.len(),
        1,
        "the in-flight search returns its match"
    );
    assert_eq!(
        rebuild_result.outcome,
        ResourceTextIndexRebuildOutcome::Indexed
    );
}

#[test]
fn rebuilding_a_current_index_can_replace_one_key_version_at_the_bound() {
    let directory = tempfile::tempdir().expect("temporary current index-version rebuild directory");
    let current_index_version = Arc::new(AtomicU32::new(1));
    let store = open_store(
        &directory,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            RotatingResourceIndexKeys {
                current_index_version: Arc::clone(&current_index_version),
            },
        )),
    );
    create_workspace_for_uploads(&store);

    const CONTENT: &[u8] = b"versioned common searchable text";
    for version in 1..=resource_index::MAX_SEARCHABLE_KEY_VERSIONS as u32 {
        current_index_version.store(version, Ordering::SeqCst);
        create_resource_with_content(
            &store,
            &format!("resource-rebuild-key-version-{version}"),
            &format!("rebuild-version-{version}.txt"),
            "text/plain",
            CONTENT,
        )
        .expect("Resource is indexed under its current key version");
    }

    current_index_version.store(
        resource_index::MAX_SEARCHABLE_KEY_VERSIONS as u32 + 1,
        Ordering::SeqCst,
    );
    let resource_id = "resource-rebuild-key-version-1";
    let rebuilt = store
        .reindex_resource_text(ResourceTextIndexRebuildRequest {
            principal_id: OWNER.to_owned(),
            request_id: "request-rebuild-key-version-at-bound".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            resource_id: resource_id.to_owned(),
            resource_revision_id: format!("revision-{resource_id}"),
            content_digest: digest(CONTENT),
            indexed_at: "2026-10-12T10:00:03Z".to_owned(),
            correlation_id: "correlation-rebuild-key-version-at-bound".to_owned(),
        })
        .expect("replacement removes the old current key version before adding the active one");

    assert_eq!(rebuilt.outcome, ResourceTextIndexRebuildOutcome::Indexed);
    assert_eq!(
        store
            .search_indexed_resource_text(WORKSPACE, "versioned common", None, None, None, None, 20)
            .expect("eight current key versions remain searchable")
            .items
            .len(),
        resource_index::MAX_SEARCHABLE_KEY_VERSIONS
    );
}

#[test]
fn writing_ninth_index_key_version_is_rejected_before_blob_writes() {
    let directory = tempfile::tempdir().expect("temporary index key-version bound directory");
    let current_index_version = Arc::new(AtomicU32::new(1));
    let store = open_store(
        &directory,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            RotatingResourceIndexKeys {
                current_index_version: Arc::clone(&current_index_version),
            },
        )),
    );
    create_workspace_for_uploads(&store);

    for version in 1..=resource_index::MAX_SEARCHABLE_KEY_VERSIONS as u32 {
        current_index_version.store(version, Ordering::SeqCst);
        create_resource_with_content(
            &store,
            &format!("resource-index-key-version-{version}"),
            &format!("version-{version}.txt"),
            "text/plain",
            b"versioned common searchable text",
        )
        .expect("resource under each retained key version commits");
    }
    assert_eq!(
        store
            .search_indexed_resource_text(WORKSPACE, "versioned common", None, None, None, None, 20)
            .expect("all retained index versions remain searchable")
            .items
            .len(),
        resource_index::MAX_SEARCHABLE_KEY_VERSIONS
    );

    current_index_version.store(
        resource_index::MAX_SEARCHABLE_KEY_VERSIONS as u32 + 1,
        Ordering::SeqCst,
    );
    let direct_blob_count = blob_object_count(&directory);
    assert!(matches!(
        create_resource_with_content(
            &store,
            "resource-index-key-version-overflow",
            "overflow.txt",
            "text/plain",
            b"ninth version must be rejected before writes",
        ),
        Err(StoreError::Blob(_))
    ));
    assert_eq!(blob_object_count(&directory), direct_blob_count);

    const UPLOAD: &[u8] = b"ninth version upload remains retryable";
    create_upload_with_content(
        &store,
        "upload-index-key-version-overflow",
        "overflow-upload.txt",
        "text/plain",
        UPLOAD,
    );
    store
        .put_chunk(chunk_input_with_content(
            "upload-index-key-version-overflow",
            "request-index-key-version-overflow-chunk",
            UPLOAD,
        ))
        .expect("ninth-version upload reaches CONTENT_RECEIVED");
    let upload_blob_count = blob_object_count(&directory);
    assert!(matches!(
        commit_upload_with_content(
            &store,
            "upload-index-key-version-overflow",
            "resource-index-key-version-overflow-upload",
            "overflow-upload.txt",
            "text/plain",
            UPLOAD,
        ),
        Err(StoreError::Blob(_))
    ));
    assert_eq!(blob_object_count(&directory), upload_blob_count);
    assert_eq!(
        store
            .get(WORKSPACE, "upload-index-key-version-overflow")
            .expect("read overflow upload")
            .expect("overflow upload remains present")
            .state,
        ResourceUploadState::ContentReceived
    );
    assert_eq!(
        store
            .search_indexed_resource_text(WORKSPACE, "versioned common", None, None, None, None, 20)
            .expect("overflow rejection leaves prior index searchable")
            .items
            .len(),
        resource_index::MAX_SEARCHABLE_KEY_VERSIONS
    );
}

#[test]
fn concurrent_index_writes_cannot_exceed_searchable_key_version_bound() {
    let directory = tempfile::tempdir().expect("temporary concurrent index key-version directory");
    let current_index_version = Arc::new(AtomicU32::new(1));
    let key_store = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        RotatingResourceIndexKeys {
            current_index_version: Arc::clone(&current_index_version),
        },
    ));
    let (first_entered_sender, first_entered_receiver) = std::sync::mpsc::sync_channel(1);
    let (second_entered_sender, second_entered_receiver) = std::sync::mpsc::sync_channel(1);
    let gate = Arc::new(ConcurrentIndexPutGate {
        armed: AtomicBool::new(false),
        calls: AtomicUsize::new(0),
        first_entered: first_entered_sender,
        second_entered: second_entered_sender,
        first_release: Barrier::new(2),
        second_release: Barrier::new(2),
    });
    let blobs: Arc<dyn BlobStore> = Arc::new(PausedIndexPutBlobStore {
        inner: key_store,
        gate: Arc::clone(&gate),
    });
    let store = open_store(&directory, Arc::clone(&blobs));
    let second_store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);

    for version in 1..resource_index::MAX_SEARCHABLE_KEY_VERSIONS as u32 {
        current_index_version.store(version, Ordering::SeqCst);
        create_resource_with_content(
            &store,
            &format!("resource-concurrent-index-key-version-{version}"),
            &format!("version-{version}.txt"),
            "text/plain",
            b"concurrent version limit stays searchable",
        )
        .expect("resource under each existing key version commits");
    }

    gate.calls.store(0, Ordering::SeqCst);
    gate.armed.store(true, Ordering::SeqCst);
    current_index_version.store(8, Ordering::SeqCst);
    let first_store = store.clone();
    let first = std::thread::spawn(move || {
        create_resource_with_content(
            &first_store,
            "resource-concurrent-index-key-version-eight",
            "version-eight.txt",
            "text/plain",
            b"concurrent version limit stays searchable",
        )
    });
    first_entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("version-eight write passes preflight and reaches snapshot persistence");

    current_index_version.store(9, Ordering::SeqCst);
    let (second_started_sender, second_started_receiver) = std::sync::mpsc::sync_channel(1);
    let second = std::thread::spawn(move || {
        second_started_sender
            .send(())
            .expect("notify second index writer is about to start");
        create_resource_with_content(
            &second_store,
            "resource-concurrent-index-key-version-nine",
            "version-nine.txt",
            "text/plain",
            b"concurrent version limit stays searchable",
        )
    });
    second_started_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("second index writer starts while version-eight preflight is paused");
    // If the lock is absent, the second writer reaches and pauses in its stale
    // preflight here. If the lock is present, it stays outside the key provider
    // until the first transaction has committed.
    let second_preflight_before_first_commit = second_entered_receiver
        .recv_timeout(Duration::from_millis(100))
        .is_ok();
    assert!(
        !second_preflight_before_first_commit,
        "the second writer must wait outside key preflight until the first index commits"
    );

    gate.first_release.wait();
    first
        .join()
        .expect("version-eight resource writer joins")
        .expect("first serialized write stays within the searchable bound");
    let blobs_after_first_commit = blob_object_count(&directory);
    second_entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("second writer reaches key preflight after the first commit");
    gate.second_release.wait();
    let second_result = second.join().expect("version-nine resource writer joins");
    assert!(
        matches!(&second_result, Err(StoreError::Blob(_))),
        "version-nine writer should fail the retained-key preflight: {second_result:?}"
    );

    assert_eq!(
        store
            .search_indexed_resource_text(
                WORKSPACE,
                "concurrent searchable",
                None,
                None,
                None,
                None,
                20
            )
            .expect("transactional key-version bound keeps indexed search available")
            .items
            .len(),
        resource_index::MAX_SEARCHABLE_KEY_VERSIONS
    );
    assert_eq!(
        resource_index_projection_counts(&directory, "resource-concurrent-index-key-version-nine"),
        (0, 0)
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect SQLite");
    let resource_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE, "resource-concurrent-index-key-version-nine"],
            |row| row.get(0),
        )
        .expect("count rejected Resource");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_type = 'Resource' AND entity_id = ?2 AND type = 'resource.created.v1'",
            params![WORKSPACE, "resource-concurrent-index-key-version-nine"],
            |row| row.get(0),
        )
        .expect("count rejected Resource event");
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            params![
                OWNER,
                "request-resource-create-resource-concurrent-index-key-version-nine"
            ],
            |row| row.get(0),
        )
        .expect("count rejected Resource receipt");
    assert_eq!(
        (resource_count, event_count, receipt_count),
        (0, 0, 0),
        "transactional key-version rejection leaves no Resource, event, or receipt"
    );
    assert_eq!(
        blob_object_count(&directory),
        blobs_after_first_commit,
        "a concurrent over-limit Resource write is rejected before creating unreferenced blobs"
    );
}

#[test]
fn uploaded_text_index_rebuild_search_and_restart_preserve_exact_pins() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);

    let upload_id = "upload-index-roundtrip";
    let chunk_request_id = "c".repeat(200);
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
        .put_chunk(chunk_input(upload_id, &chunk_request_id))
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
    assert_eq!(initial.items.len(), 1);
    assert_eq!(initial.items[0].resource_revision_id, revision_id);
    assert_eq!(initial.items[0].source_content_digest, content_digest);

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
            .items
            .is_empty()
    );
    let rebuild_request_id = "r".repeat(200);
    let rebuilt = store
        .reindex_resource_text(ResourceTextIndexRebuildRequest {
            principal_id: OWNER.to_owned(),
            request_id: rebuild_request_id.clone(),
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
            request_id: rebuild_request_id,
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
    assert_eq!(before_restart.items.len(), 1);
    assert_eq!(before_restart.items[0].snippet, "durable upload chunk");
    assert_eq!(before_restart.items[0].matched_term_count, 2);
    assert_eq!(
        before_restart.items[0].matched_spans,
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
            .items
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
            .items
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
        aggregate_state_put_pause: None,
        aggregate_state_puts: Arc::new(AtomicUsize::new(0)),
        fail_next_remove: Arc::new(AtomicBool::new(fail_next_remove)),
        fail_next_resource_remove: Arc::new(AtomicBool::new(false)),
        fail_after_resource_remove: Arc::new(AtomicBool::new(false)),
        resource_remove_calls: Arc::new(AtomicUsize::new(0)),
        resource_content_put_pause: None,
        resource_content_get_pause: None,
    })
}

fn observed_blob_store(
    directory: &tempfile::TempDir,
    aggregate_state_put_pause: Option<AggregateStatePutPause>,
) -> (Arc<dyn BlobStore>, Arc<AtomicUsize>) {
    let aggregate_state_puts = Arc::new(AtomicUsize::new(0));
    let store = TestUploadBlobStore {
        inner: FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys { key: [23_u8; 32] },
        ),
        chunk_put_pause: None,
        gc_remove_pause: None,
        aggregate_state_put_pause,
        aggregate_state_puts: Arc::clone(&aggregate_state_puts),
        fail_next_remove: Arc::new(AtomicBool::new(false)),
        fail_next_resource_remove: Arc::new(AtomicBool::new(false)),
        fail_after_resource_remove: Arc::new(AtomicBool::new(false)),
        resource_remove_calls: Arc::new(AtomicUsize::new(0)),
        resource_content_put_pause: None,
        resource_content_get_pause: None,
    };
    (Arc::new(store), aggregate_state_puts)
}

fn create_upload(store: &SqliteWorkspaceStore, upload_id: &str) -> ResourceUploadSessionRecord {
    create_upload_with_context_document(store, upload_id, None)
}

fn expire_upload_for_gc(
    store: &SqliteWorkspaceStore,
    upload_id: &str,
    event_id: &str,
) -> ResourceUploadSessionRecord {
    let current = store
        .get(WORKSPACE, upload_id)
        .expect("read upload before expiry")
        .expect("upload exists before expiry");
    let mut expired = current.clone();
    expired.state = ResourceUploadState::Expired;
    expired.version += 1;
    let event = context(event_id, 2);
    let draft = EventDraft {
        event_id: event.event_id,
        workspace_id: WORKSPACE.to_owned(),
        entity_type: "ResourceUpload".to_owned(),
        entity_id: current.upload_id.clone(),
        origin_runtime_id: event.origin_runtime_id,
        entity_revision: expired.version,
        hlc_timestamp: AFTER_EXPIRY.to_owned(),
        correlation_id: event.correlation_id,
        causation_id: None,
        schema_version: 1,
        event_type: "resource.upload.status.changed.v1".to_owned(),
        payload: json!({
            "upload_id": current.upload_id,
            "from": match current.state {
                ResourceUploadState::Open => "OPEN",
                ResourceUploadState::ContentReceived => "CONTENT_RECEIVED",
                _ => panic!("only active uploads can expire"),
            },
            "to": "EXPIRED",
            "reason_code": "UPLOAD_EXPIRED",
            "aggregate_version": expired.version,
        }),
        recorded_at: AFTER_EXPIRY.to_owned(),
    };
    store
        .expire(current.progress_version, expired.clone(), draft)
        .expect("expiry transition commits before orphan collection");
    expired
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

struct UploadCreateRequest<'a> {
    upload_id: &'a str,
    request_id: &'a str,
    display_name: &'a str,
    media_type: &'a str,
    request_payload: serde_json::Value,
    recorded_at: &'a str,
    expires_at: &'a str,
}

fn create_upload_with_request(
    store: &SqliteWorkspaceStore,
    request: UploadCreateRequest<'_>,
) -> Result<ResourceUploadSessionRecord, StoreError> {
    let session = ResourceUploadSessionRecord {
        upload_id: request.upload_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        display_name: request.display_name.to_owned(),
        media_type: request.media_type.to_owned(),
        expected_size_bytes: CHUNK.len() as u64,
        expected_digest: Some(digest(CHUNK)),
        context_document: None,
        folder_import: None,
        resource_id: None,
        committed_resource_id: None,
        expected_resource_version: None,
        parent_revision_ids: Vec::new(),
        chunk_size_bytes: 4_194_304,
        received_ranges: Vec::new(),
        next_missing_offset: 0,
        state: ResourceUploadState::Open,
        expires_at: request.expires_at.to_owned(),
        created_at: request.recorded_at.to_owned(),
        version: 1,
        progress_version: 1,
    };
    let event_context = context(&format!("event-create-{}", request.upload_id), 1);
    store.create(
        WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: request.request_id.to_owned(),
            request_payload: request.request_payload,
        },
        session.clone(),
        EventDraft {
            event_id: event_context.event_id,
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: request.upload_id.to_owned(),
            origin_runtime_id: event_context.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: event_context.hlc_timestamp,
            correlation_id: event_context.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.created.v1".to_owned(),
            payload: json!({
                "upload_id": request.upload_id,
                "workspace_id": WORKSPACE,
                "expected_size_bytes": CHUNK.len(),
                "expected_digest": digest(CHUNK),
                "chunk_size_bytes": 4_194_304,
                "expires_at": request.expires_at,
                "aggregate_version": 1,
            }),
            recorded_at: request.recorded_at.to_owned(),
        },
    )
}

fn upload_create_payload(display_name: &str, media_type: &str) -> serde_json::Value {
    json!({
        "operation": "resource.upload.create.v1",
        "workspace_id": WORKSPACE,
        "display_name": display_name,
        "media_type": media_type,
        "size_bytes": CHUNK.len(),
        "expected_digest": digest(CHUNK),
        "context_document": null,
    })
}

fn create_revision_base_resource(
    store: &SqliteWorkspaceStore,
    resource_id: &str,
    revision_id: &str,
) {
    let created_at = CREATED;
    let content_digest = digest(CHUNK);
    let resource = ResourceRecord {
        resource_id: resource_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        kind: "FILE".to_owned(),
        provider_identity: json!({}),
        identity_digest: None,
        display_name: "revision-base.txt".to_owned(),
        current_revision_id: Some(revision_id.to_owned()),
        sensitivity: "PERSONAL".to_owned(),
        provenance: json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
        created_at: created_at.to_owned(),
        updated_at: created_at.to_owned(),
        version: 1,
    };
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id.to_owned(),
        resource_id: resource_id.to_owned(),
        parent_revision_ids: Vec::new(),
        provider_revision: None,
        content_digest: Some(content_digest),
        size_bytes: Some(CHUNK.len() as u64),
        media_type: Some("text/plain".to_owned()),
        observed_at: created_at.to_owned(),
        created_by: json!({"kind": "USER", "principal_id": OWNER}),
    };
    store
        .create_resource(
            WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: format!("request-create-revision-base-{resource_id}"),
                request_payload: json!({"upload_id": format!("fixture-upload-{resource_id}")}),
            },
            resource.clone(),
            revision,
            EventDraft {
                event_id: format!("event-create-revision-base-{resource_id}"),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "Resource".to_owned(),
                entity_id: resource_id.to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                entity_revision: resource.version,
                hlc_timestamp: created_at.to_owned(),
                correlation_id: format!("correlation-create-revision-base-{resource_id}"),
                causation_id: None,
                schema_version: 1,
                event_type: "resource.created.v1".to_owned(),
                payload: json!({
                    "resource_id": resource_id,
                    "workspace_id": WORKSPACE,
                    "kind": "FILE",
                    "provenance": resource.provenance,
                    "aggregate_version": resource.version,
                }),
                recorded_at: created_at.to_owned(),
            },
            CHUNK.to_vec(),
        )
        .expect("base Resource exists for revision-upload create");
}

fn revision_upload_create_payload(
    resource_id: &str,
    media_type: &str,
    content: &[u8],
) -> serde_json::Value {
    json!({
        "operation": "resource.revision.upload.create.v1",
        "workspace_id": WORKSPACE,
        "resource_id": resource_id,
        "expected_resource_version": 1,
        "parent_revision_ids": ["revision-upload-revision-create"],
        "media_type": media_type,
        "size_bytes": content.len(),
        "expected_digest": digest(content),
    })
}

fn create_revision_upload_with_request(
    store: &SqliteWorkspaceStore,
    upload_id: &str,
    request_id: &str,
    media_type: &str,
) -> Result<ResourceUploadSessionRecord, StoreError> {
    create_revision_upload_with_resource(
        store,
        upload_id,
        request_id,
        "resource-upload-revision-create",
        media_type,
    )
}

fn create_revision_upload_with_resource(
    store: &SqliteWorkspaceStore,
    upload_id: &str,
    request_id: &str,
    resource_id: &str,
    media_type: &str,
) -> Result<ResourceUploadSessionRecord, StoreError> {
    create_revision_upload_with_content(
        store,
        upload_id,
        request_id,
        resource_id,
        media_type,
        CHUNK,
    )
}

fn create_revision_upload_with_content(
    store: &SqliteWorkspaceStore,
    upload_id: &str,
    request_id: &str,
    resource_id: &str,
    media_type: &str,
    content: &[u8],
) -> Result<ResourceUploadSessionRecord, StoreError> {
    let expected_digest = digest(content);
    let event = context(&format!("event-create-revision-upload-{upload_id}"), 1);
    store.create_revision(
        WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: request_id.to_owned(),
            request_payload: revision_upload_create_payload(resource_id, media_type, content),
        },
        ResourceUploadSessionRecord {
            upload_id: upload_id.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            display_name: String::new(),
            media_type: media_type.to_owned(),
            expected_size_bytes: content.len() as u64,
            expected_digest: Some(expected_digest.clone()),
            context_document: None,
            folder_import: None,
            resource_id: Some(resource_id.to_owned()),
            committed_resource_id: None,
            expected_resource_version: Some(1),
            parent_revision_ids: vec!["revision-upload-revision-create".to_owned()],
            chunk_size_bytes: 4_194_304,
            received_ranges: Vec::new(),
            next_missing_offset: 0,
            state: ResourceUploadState::Open,
            expires_at: EXPIRES.to_owned(),
            created_at: event.recorded_at.clone(),
            version: 1,
            progress_version: 1,
        },
        EventDraft {
            event_id: event.event_id,
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: upload_id.to_owned(),
            origin_runtime_id: event.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: event.hlc_timestamp,
            correlation_id: event.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.created.v1".to_owned(),
            payload: json!({
                "upload_id": upload_id,
                "workspace_id": WORKSPACE,
                "expected_size_bytes": content.len(),
                "expected_digest": expected_digest,
                "chunk_size_bytes": 4_194_304,
                "expires_at": EXPIRES,
                "resource_id": resource_id,
                "expected_resource_version": 1,
                "parent_revision_ids": ["revision-upload-revision-create"],
                "aggregate_version": 1,
            }),
            recorded_at: event.recorded_at,
        },
    )
}

fn commit_unindexable_revision_upload(
    store: &SqliteWorkspaceStore,
    upload_id: &str,
    resource_id: &str,
    revision_id: &str,
    content: &[u8],
) -> Result<storage_core::CommittedResourceUpload, StoreError> {
    let session = store
        .get(WORKSPACE, upload_id)?
        .ok_or(StoreError::NotFound)?;
    let mut resource = store
        .get_resource_record(WORKSPACE, resource_id)?
        .ok_or(StoreError::NotFound)?;
    let parent_revision_ids = session.parent_revision_ids.clone();
    let content_digest = digest(content);
    let observed_at = "2026-10-12T10:00:03Z";
    resource.version += 1;
    resource.current_revision_id = Some(revision_id.to_owned());
    resource.updated_at = observed_at.to_owned();
    let created_by = json!({"kind": "USER", "principal_id": OWNER});
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id.to_owned(),
        resource_id: resource_id.to_owned(),
        parent_revision_ids: parent_revision_ids.clone(),
        provider_revision: None,
        content_digest: Some(content_digest.clone()),
        size_bytes: Some(content.len() as u64),
        media_type: Some("application/zip".to_owned()),
        observed_at: observed_at.to_owned(),
        created_by: created_by.clone(),
    };
    let resource_event_id = format!("event-revision-{revision_id}");
    let resource_event = EventDraft {
        event_id: resource_event_id.clone(),
        workspace_id: WORKSPACE.to_owned(),
        entity_type: "Resource".to_owned(),
        entity_id: resource_id.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: resource.version,
        hlc_timestamp: observed_at.to_owned(),
        correlation_id: resource_event_id.clone(),
        causation_id: None,
        schema_version: 1,
        event_type: "resource.revision.created.v1".to_owned(),
        payload: json!({
            "resource_id": resource_id,
            "resource_revision_id": revision_id,
            "parent_revision_ids": parent_revision_ids,
            "content_digest": content_digest,
            "size_bytes": content.len(),
            "media_type": "application/zip",
            "created_by": created_by,
            "aggregate_version": resource.version,
        }),
        recorded_at: observed_at.to_owned(),
    };
    let upload_event_version = session.version + 1;
    store.commit(
        WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: format!("unused-revision-commit-{upload_id}"),
            request_payload: json!({}),
        },
        upload_id,
        resource,
        revision,
        resource_event,
        Some(EventDraft {
            event_id: format!("event-upload-commit-{upload_id}"),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: upload_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: upload_event_version,
            hlc_timestamp: observed_at.to_owned(),
            correlation_id: format!("correlation-upload-commit-{upload_id}"),
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.status.changed.v1".to_owned(),
            payload: json!({
                "upload_id": upload_id,
                "from": "CONTENT_RECEIVED",
                "to": "COMMITTED",
                "resource_id": resource_id,
                "aggregate_version": upload_event_version,
            }),
            recorded_at: observed_at.to_owned(),
        }),
        None,
    )
}

#[test]
fn unindexable_revision_upload_waits_for_rebuild_blob_preparation() {
    const RESOURCE_ID: &str = "resource-unindexable-rebuild-race";
    const BASE_REVISION_ID: &str = "revision-upload-revision-create";
    const UPLOAD_ID: &str = "upload-unindexable-rebuild-race";
    const NEXT_REVISION_ID: &str = "revision-unindexable-rebuild-race";

    let directory = tempfile::tempdir().expect("temporary ResourceIndex/revision race directory");
    let pause = ResourceIndexBlobPutPause {
        armed: Arc::new(AtomicBool::new(false)),
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        first_only: Arc::new(AtomicBool::new(true)),
        digests: Arc::new(Mutex::new(Vec::new())),
    };
    let blobs: Arc<dyn BlobStore> = Arc::new(PausedResourceIndexBlobStore {
        inner: file_blob_store(&directory),
        pause: pause.clone(),
        get_pause: None,
        token_pause: None,
    });
    let store = open_store(&directory, Arc::clone(&blobs));
    let upload_store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    create_revision_base_resource(&store, RESOURCE_ID, BASE_REVISION_ID);
    let initial_index_count = resource_index_projection_counts(&directory, RESOURCE_ID).0;
    assert_eq!(initial_index_count, 1, "the base revision is indexed");

    const ZIP_CONTENT: &[u8] = b"PK\x03\x04opaque ZIP revision bytes";
    create_revision_upload_with_content(
        &store,
        UPLOAD_ID,
        "request-unindexable-rebuild-race",
        RESOURCE_ID,
        "application/zip",
        ZIP_CONTENT,
    )
    .expect("unindexable revision upload is prepared");
    store
        .put_chunk(chunk_input_with_content(
            UPLOAD_ID,
            "request-chunk-unindexable-rebuild-race",
            ZIP_CONTENT,
        ))
        .expect("unindexable revision bytes are accepted before the race");

    pause.armed.store(true, Ordering::SeqCst);
    let rebuild_store = store.clone();
    let (rebuild_sender, rebuild_receiver) = std::sync::mpsc::sync_channel(1);
    let rebuild = std::thread::spawn(move || {
        let result = rebuild_store.reindex_resource_text(ResourceTextIndexRebuildRequest {
            principal_id: OWNER.to_owned(),
            request_id: "request-unindexable-rebuild-race-rebuild".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            resource_id: RESOURCE_ID.to_owned(),
            resource_revision_id: BASE_REVISION_ID.to_owned(),
            content_digest: digest(CHUNK),
            indexed_at: "2026-10-12T10:00:04Z".to_owned(),
            correlation_id: "correlation-unindexable-rebuild-race".to_owned(),
        });
        rebuild_sender
            .send(result)
            .expect("rebuild result receiver remains open");
    });
    pause.entered.wait();

    let (contention_sender, contention_receiver) = std::sync::mpsc::sync_channel(1);
    store.observe_next_resource_index_write_contention(WORKSPACE, contention_sender);
    let (upload_sender, upload_receiver) = std::sync::mpsc::sync_channel(1);
    let upload = std::thread::spawn(move || {
        let result = commit_unindexable_revision_upload(
            &upload_store,
            UPLOAD_ID,
            RESOURCE_ID,
            NEXT_REVISION_ID,
            ZIP_CONTENT,
        );
        upload_sender
            .send(result)
            .expect("revision upload result receiver remains open");
    });
    let observed_lock_contention = contention_receiver
        .recv_timeout(Duration::from_secs(5))
        .is_ok();
    let upload_result_before_rebuild_release = upload_receiver.try_recv().ok();
    let upload_completed_before_rebuild_release = upload_result_before_rebuild_release.is_some();

    pause.release.wait();
    let rebuild_result = rebuild_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("rebuild completes after releasing its prepared blob")
        .expect("rebuild commits while its pinned revision remains current");
    let upload_result = match upload_result_before_rebuild_release {
        Some(result) => result.expect("unindexable revision upload commits"),
        None => upload_receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("revision upload commits after rebuild releases the Workspace lock")
            .expect("unindexable revision upload commits"),
    };
    rebuild.join().expect("rebuild thread joins");
    upload.join().expect("revision upload thread joins");

    assert!(
        observed_lock_contention,
        "the upload must observe the held Workspace ResourceIndex lock"
    );
    assert!(
        !upload_completed_before_rebuild_release,
        "an unindexable revision upload must not complete while rebuild preparation is paused"
    );
    assert_eq!(
        rebuild_result.outcome,
        ResourceTextIndexRebuildOutcome::Indexed
    );
    assert_eq!(upload_result.resource.version, 2);
    assert_eq!(
        upload_result.resource.current_revision_id.as_deref(),
        Some(NEXT_REVISION_ID)
    );
    assert_eq!(
        resource_index_projection_counts(&directory, RESOURCE_ID).0,
        initial_index_count,
        "the historical rebuild remains indexed and the new unindexable head has no index"
    );

    let candidate_digest = pause
        .digests
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .last()
        .cloned()
        .expect("the rebuild prepared an encrypted ResourceIndex blob");
    let connection = Connection::open(state_database(&directory)).expect("inspect race result");
    let stored_candidate_digest: String = connection
        .query_row(
            "SELECT extracted_blob_digest FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2 AND resource_revision_id = ?3",
            params![WORKSPACE, RESOURCE_ID, BASE_REVISION_ID],
            |row| row.get(0),
        )
        .expect("rebuild candidate is referenced by its committed projection");
    assert_eq!(stored_candidate_digest, candidate_digest);
    let new_head_index_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2 AND resource_revision_id = ?3",
            params![WORKSPACE, RESOURCE_ID, NEXT_REVISION_ID],
            |row| row.get(0),
        )
        .expect("count index rows for the unindexable head");
    assert_eq!(new_head_index_count, 0);
}

#[test]
fn revision_upload_create_replay_and_changed_payload_do_not_write_orphan_aggregate_state() {
    let directory = tempfile::tempdir().expect("temporary revision upload create directory");
    let (blobs, aggregate_state_puts) = observed_blob_store(&directory, None);
    let store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    create_revision_base_resource(
        &store,
        "resource-upload-revision-create",
        "revision-upload-revision-create",
    );
    let resource_state_puts = aggregate_state_puts.load(Ordering::SeqCst);

    let created = create_revision_upload_with_request(
        &store,
        "upload-revision-create-original",
        "request-revision-create-replay",
        "text/plain",
    )
    .expect("revision upload session commits");
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        resource_state_puts + 1,
        "first revision upload writes one aggregate snapshot"
    );

    let replay = create_revision_upload_with_request(
        &store,
        "upload-revision-create-replay-candidate",
        "request-revision-create-replay",
        "text/plain",
    )
    .expect("same-key exact revision-upload replay returns its receipt");
    assert_eq!(replay.upload_id, created.upload_id);
    assert_eq!(replay.created_at, created.created_at);
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        resource_state_puts + 1,
        "exact revision-upload replay must not persist an unreferenced candidate state"
    );

    assert!(matches!(
        create_revision_upload_with_resource(
            &store,
            "upload-revision-create-missing-resource-conflict",
            "request-revision-create-replay",
            "resource-does-not-exist",
            "text/plain",
        ),
        Err(StoreError::Conflict {
            expected: None,
            actual: None
        })
    ));

    assert!(matches!(
        create_revision_upload_with_request(
            &store,
            "upload-revision-create-conflict-candidate",
            "request-revision-create-replay",
            "application/octet-stream",
        ),
        Err(StoreError::Conflict {
            expected: None,
            actual: None
        })
    ));
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        resource_state_puts + 1,
        "changed revision-upload payload must conflict before writing candidate state"
    );

    let connection = Connection::open(state_database(&directory)).expect("inspect SQLite");
    let session_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_sessions WHERE resource_id = ?1",
            ["resource-upload-revision-create"],
            |row| row.get(0),
        )
        .expect("count revision upload sessions");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type = 'ResourceUpload' AND type = 'resource.upload.created.v1' AND entity_id IN ('upload-revision-create-original', 'upload-revision-create-replay-candidate', 'upload-revision-create-conflict-candidate')",
            [],
            |row| row.get(0),
        )
        .expect("count revision upload creation events");
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = 'request-revision-create-replay'",
            [OWNER],
            |row| row.get(0),
        )
        .expect("count revision upload idempotency receipts");
    assert_eq!((session_count, event_count, receipt_count), (1, 1, 1));
}

#[test]
fn concurrent_revision_upload_create_replay_across_open_stores_serializes_blob_write() {
    let directory = tempfile::tempdir().expect("temporary concurrent revision upload directory");
    let pause = AggregateStatePutPause {
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        armed: Arc::new(AtomicBool::new(false)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let (blobs, aggregate_state_puts) = observed_blob_store(&directory, Some(pause.clone()));
    let store = open_store(&directory, Arc::clone(&blobs));
    let second_store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    create_revision_base_resource(
        &store,
        "resource-upload-revision-create",
        "revision-upload-revision-create",
    );
    let first_request_lock = store.create_request_lock(OWNER, "request-revision-create-concurrent");
    let second_request_lock =
        second_store.create_request_lock(OWNER, "request-revision-create-concurrent");
    assert!(
        Arc::ptr_eq(&first_request_lock, &second_request_lock),
        "separately opened stores must share the process-local request lock"
    );
    let resource_state_puts = aggregate_state_puts.load(Ordering::SeqCst);
    pause.armed.store(true, Ordering::SeqCst);

    let first_store = store.clone();
    let first = std::thread::spawn(move || {
        create_revision_upload_with_request(
            &first_store,
            "upload-revision-create-concurrent-first",
            "request-revision-create-concurrent",
            "text/plain",
        )
    });
    pause.entered.wait();

    let (second_started_sender, second_started_receiver) = std::sync::mpsc::channel();
    let (second_result_sender, second_result_receiver) = std::sync::mpsc::channel();
    let second = std::thread::spawn(move || {
        let _ = second_started_sender.send(());
        let result = create_revision_upload_with_request(
            &second_store,
            "upload-revision-create-concurrent-second",
            "request-revision-create-concurrent",
            "text/plain",
        );
        let _ = second_result_sender.send(result);
    });
    second_started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("second exact revision-upload retry starts");
    let completed_before_first_blob_release = second_result_receiver
        .recv_timeout(Duration::from_millis(250))
        .ok();
    let second_finished_early = completed_before_first_blob_release.is_some();
    pause.release.wait();

    let first_result = first.join().expect("first revision upload thread joins");
    let second_result = match completed_before_first_blob_release {
        Some(result) => result,
        None => second_result_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("serialized second revision upload completes after first receipt commits"),
    };
    let first_session = first_result.expect("first revision upload commits");
    let second_session = second_result.expect("second call replays first revision upload");
    second.join().expect("second revision upload thread joins");

    assert!(
        !second_finished_early,
        "same-key revision-upload create must not pass preflight/write while unresolved"
    );
    assert_eq!(first_session.upload_id, second_session.upload_id);
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        resource_state_puts + 1,
        "concurrent exact revision-upload retries across clones write only the winning state blob"
    );
}

#[test]
fn upload_create_replay_and_changed_payload_do_not_write_orphan_aggregate_state() {
    let directory = tempfile::tempdir().expect("temporary upload create directory");
    let (blobs, aggregate_state_puts) = observed_blob_store(&directory, None);
    let store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    let workspace_state_puts = aggregate_state_puts.load(Ordering::SeqCst);

    let payload = upload_create_payload("resume.txt", "text/plain");
    let created = create_upload_with_request(
        &store,
        UploadCreateRequest {
            upload_id: "upload-create-original",
            request_id: "request-create-replay",
            display_name: "resume.txt",
            media_type: "text/plain",
            request_payload: payload.clone(),
            recorded_at: CREATED,
            expires_at: EXPIRES,
        },
    )
    .expect("first upload session commits");
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        workspace_state_puts + 1,
        "first upload writes its aggregate state once"
    );

    let replay = create_upload_with_request(
        &store,
        UploadCreateRequest {
            upload_id: "upload-create-replay-candidate",
            request_id: "request-create-replay",
            display_name: "resume.txt",
            media_type: "text/plain",
            request_payload: payload,
            recorded_at: "2026-10-12T10:00:01Z",
            expires_at: "2026-10-12T10:01:01Z",
        },
    )
    .expect("same-key exact upload replay returns its receipt");
    assert_eq!(replay.upload_id, created.upload_id);
    assert_eq!(replay.created_at, created.created_at);
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        workspace_state_puts + 1,
        "exact replay must not persist an unreferenced candidate state"
    );

    assert!(matches!(
        create_upload_with_request(
            &store,
            UploadCreateRequest {
                upload_id: "upload-create-conflict-candidate",
                request_id: "request-create-replay",
                display_name: "changed.txt",
                media_type: "text/plain",
                request_payload: upload_create_payload("changed.txt", "text/plain"),
                recorded_at: "2026-10-12T10:00:02Z",
                expires_at: "2026-10-12T10:01:02Z",
            },
        ),
        Err(StoreError::Conflict {
            expected: None,
            actual: None
        })
    ));
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        workspace_state_puts + 1,
        "changed-payload conflict must be detected before writing candidate state"
    );

    let connection = Connection::open(state_database(&directory)).expect("inspect SQLite");
    let session_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_sessions WHERE workspace_id = ?1 AND display_name = 'resume.txt'",
            [WORKSPACE],
            |row| row.get(0),
        )
        .expect("count sessions");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_type = 'ResourceUpload' AND type = 'resource.upload.created.v1'",
            [WORKSPACE],
            |row| row.get(0),
        )
        .expect("count upload creation events");
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = 'request-create-replay'",
            [OWNER],
            |row| row.get(0),
        )
        .expect("count idempotency receipts");
    assert_eq!((session_count, event_count, receipt_count), (1, 1, 1));
}

#[test]
fn concurrent_upload_create_replay_across_store_clones_serializes_blob_write() {
    let directory = tempfile::tempdir().expect("temporary concurrent upload create directory");
    let pause = AggregateStatePutPause {
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        armed: Arc::new(AtomicBool::new(false)),
        first_only: Arc::new(AtomicBool::new(true)),
    };
    let (blobs, aggregate_state_puts) = observed_blob_store(&directory, Some(pause.clone()));
    let store = open_store(&directory, blobs);
    create_workspace_for_uploads(&store);
    let workspace_state_puts = aggregate_state_puts.load(Ordering::SeqCst);
    pause.armed.store(true, Ordering::SeqCst);

    let payload = upload_create_payload("concurrent.txt", "text/plain");
    let first_store = store.clone();
    let first_payload = payload.clone();
    let first = std::thread::spawn(move || {
        create_upload_with_request(
            &first_store,
            UploadCreateRequest {
                upload_id: "upload-create-concurrent-first",
                request_id: "request-create-concurrent",
                display_name: "concurrent.txt",
                media_type: "text/plain",
                request_payload: first_payload,
                recorded_at: CREATED,
                expires_at: EXPIRES,
            },
        )
    });
    pause.entered.wait();

    let second_store = store.clone();
    let second_payload = payload;
    let (second_started_sender, second_started_receiver) = std::sync::mpsc::channel();
    let (second_result_sender, second_result_receiver) = std::sync::mpsc::channel();
    let second = std::thread::spawn(move || {
        let _ = second_started_sender.send(());
        let result = create_upload_with_request(
            &second_store,
            UploadCreateRequest {
                upload_id: "upload-create-concurrent-second",
                request_id: "request-create-concurrent",
                display_name: "concurrent.txt",
                media_type: "text/plain",
                request_payload: second_payload,
                recorded_at: "2026-10-12T10:00:01Z",
                expires_at: "2026-10-12T10:01:01Z",
            },
        );
        let _ = second_result_sender.send(result);
    });
    second_started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("second exact create retry starts");
    let completed_before_first_blob_release = second_result_receiver
        .recv_timeout(Duration::from_millis(250))
        .ok();
    let second_finished_early = completed_before_first_blob_release.is_some();
    pause.release.wait();

    let first_result = first.join().expect("first create thread joins");
    let second_result = match completed_before_first_blob_release {
        Some(result) => result,
        None => second_result_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("serialized second create completes after first receipt commits"),
    };
    let first_session = first_result.expect("first session commits");
    let second_session = second_result.expect("second call replays first session");
    second.join().expect("second create thread joins");

    assert!(
        !second_finished_early,
        "same-key create must not pass preflight/write while the first request is unresolved"
    );
    assert_eq!(first_session.upload_id, second_session.upload_id);
    assert_eq!(
        aggregate_state_puts.load(Ordering::SeqCst),
        workspace_state_puts + 1,
        "concurrent exact retries across store clones write only the winning state blob"
    );
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
        ResourceUploadBlobReservation {
            workspace_id: WORKSPACE,
            upload_id,
            request_id,
            chunk_index: 0,
            chunk_digest: &digest(CHUNK),
            size_bytes: CHUNK.len() as u64,
            created_at: CREATED,
            expires_at,
        },
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
fn upload_chunk_insert_guard_rejects_sha256_that_differs_from_blob_ref() {
    let directory = tempfile::tempdir().expect("temporary upload chunk guard directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    let upload_id = "upload-mismatched-chunk-digest";
    create_upload(&store, upload_id);
    reserve_chunk(
        &directory,
        upload_id,
        "request-mismatched-chunk-digest",
        EXPIRES,
    );

    let blob = write_chunk_blob(&store);
    assert_eq!(blob.digest, digest(CHUNK));
    let connection = Connection::open(state_database(&directory)).expect("open SQLite");
    let tracked = connection
        .execute(
            "UPDATE resource_upload_chunk_blob_registry
             SET state = 'TRACKED', expires_at = NULL
             WHERE workspace_id = ?1 AND digest = ?2 AND state = 'RESERVED'",
            params![WORKSPACE, blob.digest],
        )
        .expect("mark the existing matching object as tracked");
    assert_eq!(tracked, 1);

    let mismatched_sha256 = digest(b"different chunk bytes");
    assert_ne!(mismatched_sha256, blob.digest);
    let rejected = connection
        .execute(
            "INSERT INTO resource_upload_chunks(
                 upload_id, chunk_index, start_offset, end_offset_exclusive,
                 sha256, temporary_blob_ref, received_at
             ) VALUES (?1, 0, 0, ?2, ?3, ?4, ?5)",
            params![
                upload_id,
                CHUNK.len() as i64,
                mismatched_sha256,
                blob.digest,
                CREATED,
            ],
        )
        .expect_err("v18 guard must reject a chunk digest that disagrees with its blob ref");
    assert!(
        rejected
            .to_string()
            .contains("RESOURCE_UPLOAD_CHUNK_BLOB_NOT_TRACKED"),
        "expected the upload-chunk registry guard rejection, got: {rejected}"
    );
    let chunk_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunks WHERE upload_id = ?1",
            [upload_id],
            |row| row.get(0),
        )
        .expect("inspect the immutable accepted-chunk ledger");
    assert_eq!(chunk_count, 0, "rejected insert must leave no chunk row");
}

#[test]
fn upload_blob_recovery_rows_cannot_be_detached_or_deleted_without_a_fence() {
    let directory = tempfile::tempdir().expect("temporary upload GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_upload(&store, "upload-immutable-reservation");
    reserve_chunk(
        &directory,
        "upload-immutable-reservation",
        "request-immutable-reservation",
        EXPIRES,
    );

    let connection = Connection::open(state_database(&directory)).expect("open SQLite");
    let changed_digest = format!("sha256:{}", "f".repeat(64));
    assert!(
        connection
            .execute(
                "UPDATE resource_upload_blob_reservations SET digest = ?1
             WHERE upload_id = ?2 AND request_id = ?3",
                params![
                    changed_digest,
                    "upload-immutable-reservation",
                    "request-immutable-reservation"
                ],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "DELETE FROM resource_upload_blob_reservations
             WHERE upload_id = ?1 AND request_id = ?2",
                params![
                    "upload-immutable-reservation",
                    "request-immutable-reservation"
                ],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "DELETE FROM resource_upload_chunk_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
                params![WORKSPACE, digest(CHUNK)],
            )
            .is_err()
    );
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
        second_remove_entered: None,
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

    let expiry_store = open_store(&directory, file_blob_store(&directory));
    let (expiry_started, expiry_started_receiver) = std::sync::mpsc::sync_channel(1);
    let (expiry_done, expiry_done_receiver) = std::sync::mpsc::sync_channel(1);
    let expirer = std::thread::spawn(move || {
        let current = expiry_store
            .get(WORKSPACE, "upload-inflight")
            .expect("read upload before expiration")
            .expect("upload exists");
        let mut expired = current.clone();
        expired.state = ResourceUploadState::Expired;
        expired.version += 1;
        let event = context("event-expire-upload-inflight", 2);
        let draft = EventDraft {
            event_id: event.event_id,
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: current.upload_id.clone(),
            origin_runtime_id: event.origin_runtime_id,
            entity_revision: expired.version,
            hlc_timestamp: AFTER_EXPIRY.to_owned(),
            correlation_id: event.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.status.changed.v1".to_owned(),
            payload: json!({
                "upload_id": current.upload_id,
                "from": "OPEN",
                "to": "EXPIRED",
                "reason_code": "UPLOAD_EXPIRED",
                "aggregate_version": expired.version,
            }),
            recorded_at: AFTER_EXPIRY.to_owned(),
        };
        expiry_started.send(()).expect("notify expiration attempt");
        let result = expiry_store.expire(current.progress_version, expired, draft);
        expiry_done.send(result).expect("return expiration outcome");
    });
    expiry_started_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("expiration starts while chunk publication is paused");
    assert!(
        matches!(
            expiry_done_receiver.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ),
        "expiration must wait for the in-flight chunk operation"
    );
    pause.release.wait();
    let accepted = writer
        .join()
        .expect("chunk writer joins")
        .expect("the writer commits before expiry takes the exclusive upload lease");
    assert_eq!(accepted.state, ResourceUploadState::ContentReceived);
    assert!(matches!(
        expiry_done_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("expiration returns after writer releases its lease"),
        Err(StoreError::Conflict { .. })
    ));
    expirer.join().expect("expirer thread joins");

    let expiry_retry_store = open_store(&directory, file_blob_store(&directory));
    let current = expiry_retry_store
        .get(WORKSPACE, "upload-inflight")
        .expect("read completed upload")
        .expect("upload exists");
    let mut expired = current.clone();
    expired.state = ResourceUploadState::Expired;
    expired.version += 1;
    let draft = EventDraft {
        event_id: "event-expire-upload-inflight-retry".to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        entity_type: "ResourceUpload".to_owned(),
        entity_id: current.upload_id.clone(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: expired.version,
        hlc_timestamp: AFTER_EXPIRY.to_owned(),
        correlation_id: "correlation-expire-upload-inflight".to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "resource.upload.status.changed.v1".to_owned(),
        payload: json!({
            "upload_id": current.upload_id,
            "from": "CONTENT_RECEIVED",
            "to": "EXPIRED",
            "reason_code": "UPLOAD_EXPIRED",
            "aggregate_version": expired.version,
        }),
        recorded_at: AFTER_EXPIRY.to_owned(),
    };
    let expired = expiry_retry_store
        .expire(current.progress_version, expired, draft)
        .expect("retry expiration against the newly committed progress version");
    assert_eq!(expired.state, ResourceUploadState::Expired);

    let collector_store = open_store(
        &directory,
        paused_blob_store(&directory, None, Some(gc_remove_pause.clone()), false),
    );
    let collector =
        std::thread::spawn(move || collector_store.collect_orphan_chunks(AFTER_EXPIRY, 100));
    gc_remove_pause.removed.wait();
    assert_blob_missing(&store);
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
    expire_upload_for_gc(
        &store,
        "upload-retry-delete",
        "event-expire-upload-retry-delete",
    );

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
    expire_upload_for_gc(
        &store,
        "upload-delete-before-crash",
        "event-expire-upload-delete-before-crash",
    );

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
fn upload_blob_gc_collects_committed_chunks_without_deleting_chunk_history() {
    const UPLOAD_ID: &str = "upload-gc-committed-terminal";
    let directory = tempfile::tempdir().expect("temporary committed upload GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_upload_with_content(&store, UPLOAD_ID, "committed.txt", "text/plain", CHUNK);

    store
        .put_chunk(chunk_input_with_content(
            UPLOAD_ID,
            "request-committed-terminal-chunk",
            CHUNK,
        ))
        .expect("upload accepts its complete chunk");
    let committed = commit_upload_with_content(
        &store,
        UPLOAD_ID,
        "resource-gc-committed-terminal",
        "committed.txt",
        "text/plain",
        CHUNK,
    )
    .expect("complete upload commits a Resource");
    assert_eq!(committed.session.state, ResourceUploadState::Committed);

    let connection = Connection::open(state_database(&directory)).expect("inspect chunk history");
    let history_before: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunks WHERE upload_id = ?1",
            [UPLOAD_ID],
            |row| row.get(0),
        )
        .expect("count committed chunk history");
    assert_eq!(history_before, 1);
    drop(connection);

    assert_eq!(
        store
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("terminal committed upload chunks are collectible"),
        1
    );
    assert_blob_missing(&store);

    let connection =
        Connection::open(state_database(&directory)).expect("inspect retained history");
    let history_after: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunks WHERE upload_id = ?1",
            [UPLOAD_ID],
            |row| row.get(0),
        )
        .expect("count immutable committed chunk history");
    assert_eq!(history_after, 1, "GC preserves immutable chunk history");
    let registry_after: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunk_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, digest(CHUNK)],
            |row| row.get(0),
        )
        .expect("count collected blob registry row");
    assert_eq!(registry_after, 0);
}

#[test]
fn upload_blob_gc_collects_failed_chunks_without_deleting_chunk_history() {
    const UPLOAD_ID: &str = "upload-gc-failed-terminal";
    const RESOURCE_ID: &str = "resource-gc-failed-terminal";
    const UPLOADED_CONTENT: &[u8] = b"durable upload chank";
    let directory = tempfile::tempdir().expect("temporary failed upload GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_upload_with_content(&store, UPLOAD_ID, "failed.txt", "text/plain", CHUNK);
    assert_eq!(UPLOADED_CONTENT.len(), CHUNK.len());

    store
        .put_chunk(chunk_input_with_content(
            UPLOAD_ID,
            "request-failed-terminal-chunk",
            UPLOADED_CONTENT,
        ))
        .expect("upload accepts the chunk before its whole-content digest is checked");
    let content_received = store
        .get(WORKSPACE, UPLOAD_ID)
        .expect("read upload before integrity validation")
        .expect("upload exists");
    assert_eq!(content_received.state, ResourceUploadState::ContentReceived);
    let failed_version = content_received.version + 1;
    let failure_event = EventDraft {
        event_id: format!("event-upload-fail-{UPLOAD_ID}"),
        workspace_id: WORKSPACE.to_owned(),
        entity_type: "ResourceUpload".to_owned(),
        entity_id: UPLOAD_ID.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: failed_version,
        hlc_timestamp: CREATED.to_owned(),
        correlation_id: format!("correlation-upload-fail-{UPLOAD_ID}"),
        causation_id: None,
        schema_version: 1,
        event_type: "resource.upload.status.changed.v1".to_owned(),
        payload: json!({
            "upload_id": UPLOAD_ID,
            "from": "CONTENT_RECEIVED",
            "to": "FAILED",
            "reason_code": "UPLOAD_CONTENT_INTEGRITY_FAILED",
            "aggregate_version": failed_version,
        }),
        recorded_at: CREATED.to_owned(),
    };

    assert!(matches!(
        commit_upload_with_content_and_failure_event(
            &store,
            UPLOAD_ID,
            RESOURCE_ID,
            "failed.txt",
            "text/plain",
            CHUNK,
            Some(failure_event),
        ),
        Err(StoreError::Integrity(_))
    ));
    let failed = store
        .get(WORKSPACE, UPLOAD_ID)
        .expect("read terminal upload after integrity failure")
        .expect("failed upload remains durable");
    assert_eq!(failed.state, ResourceUploadState::Failed);

    let failed_blob = BlobRef {
        digest: digest(UPLOADED_CONTENT),
        size_bytes: UPLOADED_CONTENT.len() as u64,
        media_type: "application/octet-stream".to_owned(),
    };
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &failed_blob)
            .expect("accepted failed-session chunk exists before GC"),
        UPLOADED_CONTENT
    );

    assert_eq!(
        store
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("terminal failed upload chunks are collectible"),
        1
    );
    assert!(matches!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &failed_blob),
        Err(StoreError::NotFound)
    ));

    let connection = Connection::open(state_database(&directory)).expect("inspect failed history");
    let history: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunks WHERE upload_id = ?1",
            [UPLOAD_ID],
            |row| row.get(0),
        )
        .expect("count immutable failed chunk history");
    assert_eq!(history, 1, "GC preserves immutable chunk history");
    let registry: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunk_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE, digest(UPLOADED_CONTENT)],
            |row| row.get(0),
        )
        .expect("count failed chunk registry row");
    assert_eq!(registry, 0);
}

#[test]
fn upload_blob_gc_waits_for_each_live_shared_chunk_or_reservation_reference() {
    const ACCEPTED_FIRST: &str = "upload-shared-terminal-first";
    const ACCEPTED_SECOND: &str = "upload-shared-terminal-second";
    const RESERVED_THIRD: &str = "upload-shared-terminal-reservation";
    let directory = tempfile::tempdir().expect("temporary shared upload GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    for upload_id in [ACCEPTED_FIRST, ACCEPTED_SECOND, RESERVED_THIRD] {
        create_upload_with_content(&store, upload_id, "shared.txt", "text/plain", CHUNK);
    }

    for (upload_id, request_id) in [
        (ACCEPTED_FIRST, "request-shared-terminal-first"),
        (ACCEPTED_SECOND, "request-shared-terminal-second"),
    ] {
        store
            .put_chunk(chunk_input_with_content(upload_id, request_id, CHUNK))
            .expect("each active upload accepts the identical shared chunk");
    }
    reserve_chunk(
        &directory,
        RESERVED_THIRD,
        "request-shared-terminal-reservation",
        EXPIRES,
    );

    expire_upload_for_gc(&store, ACCEPTED_FIRST, "event-expire-shared-terminal-first");
    assert_eq!(
        store
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("the second accepted chunk and third reservation keep the digest rooted"),
        0
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &blob_ref())
            .expect("shared object remains while another accepted chunk is active"),
        CHUNK
    );

    expire_upload_for_gc(
        &store,
        ACCEPTED_SECOND,
        "event-expire-shared-terminal-second",
    );
    assert_eq!(
        store
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("the third active reservation keeps the shared digest rooted"),
        0
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &blob_ref())
            .expect("shared object remains while a reservation is active"),
        CHUNK
    );

    expire_upload_for_gc(
        &store,
        RESERVED_THIRD,
        "event-expire-shared-terminal-reservation",
    );
    assert_eq!(
        store
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("shared digest is collectible after the final active reference ends"),
        1
    );
    assert_blob_missing(&store);
    assert_eq!(reservation_and_fence_counts(&directory), (0, 0, None));

    let connection = Connection::open(state_database(&directory)).expect("inspect shared history");
    for upload_id in [ACCEPTED_FIRST, ACCEPTED_SECOND] {
        let chunk_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM resource_upload_chunks WHERE upload_id = ?1",
                [upload_id],
                |row| row.get(0),
            )
            .expect("count immutable shared accepted-chunk history");
        assert_eq!(
            chunk_count, 1,
            "terminal chunk history remains for {upload_id}"
        );
    }
}

#[test]
fn upload_blob_gc_does_not_treat_elapsed_reservation_as_expired_session() {
    let directory = tempfile::tempdir().expect("temporary upload GC directory");
    let store = open_store(&directory, file_blob_store(&directory));
    create_workspace_for_uploads(&store);
    create_upload(&store, "upload-active-expired-reservation");
    reserve_chunk(
        &directory,
        "upload-active-expired-reservation",
        "request-active-expired-reservation",
        EXPIRES,
    );
    let blob = write_chunk_blob(&store);

    assert_eq!(
        store
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("collector leaves reservations owned by an active upload alone"),
        0
    );
    assert_eq!(
        reservation_and_fence_counts(&directory),
        (1, 0, Some("RESERVED".to_owned()))
    );
    assert_eq!(
        store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &blob)
            .expect("elapsed reservation remains available while its session is active"),
        CHUNK
    );

    expire_upload_for_gc(
        &store,
        "upload-active-expired-reservation",
        "event-expire-active-expired-reservation",
    );
    assert_eq!(
        store
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("collector removes the reservation after committed expiry"),
        1
    );
    assert_blob_missing(&store);
    assert_eq!(reservation_and_fence_counts(&directory), (0, 0, None));
}

#[test]
fn concurrent_upload_blob_sweeps_share_one_durable_removal_result() {
    const SECOND_SWEEP_AT: &str = "2026-10-12T10:02:01Z";
    let directory = tempfile::tempdir().expect("temporary upload GC directory");
    let (second_remove_entered, second_remove_entered_receiver) = std::sync::mpsc::sync_channel(1);
    let gc_remove_pause = GcRemovePause {
        removed: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        first_only: Arc::new(AtomicBool::new(true)),
        second_remove_entered: Some(second_remove_entered),
    };
    let first_store = open_store(
        &directory,
        paused_blob_store(&directory, None, Some(gc_remove_pause.clone()), false),
    );
    create_workspace_for_uploads(&first_store);
    create_upload(&first_store, "upload-concurrent-sweep");
    reserve_chunk(
        &directory,
        "upload-concurrent-sweep",
        "request-concurrent-sweep",
        EXPIRES,
    );
    let blob = write_chunk_blob(&first_store);
    expire_upload_for_gc(
        &first_store,
        "upload-concurrent-sweep",
        "event-expire-upload-concurrent-sweep",
    );

    let first_collector_store = first_store.clone();
    let first_collector =
        std::thread::spawn(move || first_collector_store.collect_orphan_chunks(AFTER_EXPIRY, 100));
    gc_remove_pause.removed.wait();

    let second_collector_store = open_store(
        &directory,
        paused_blob_store(&directory, None, Some(gc_remove_pause.clone()), false),
    );
    let second_collector = std::thread::spawn(move || {
        second_collector_store.collect_orphan_chunks(SECOND_SWEEP_AT, 100)
    });
    let claim_deadline = Instant::now() + Duration::from_secs(5);
    let mut second_claim_committed_while_first_remove_paused = false;
    while Instant::now() < claim_deadline {
        let observed_claim =
            Connection::open(state_database(&directory))
                .ok()
                .and_then(|connection| {
                    connection
                        .query_row(
                            "SELECT state, claimed_at FROM resource_upload_chunk_blob_registry
                         WHERE workspace_id = ?1 AND digest = ?2",
                            params![WORKSPACE, blob.digest],
                            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                        )
                        .optional()
                        .ok()
                        .flatten()
                });
        if observed_claim
            .as_ref()
            .is_some_and(|(state, claimed_at)| state == "DELETING" && claimed_at == SECOND_SWEEP_AT)
        {
            second_claim_committed_while_first_remove_paused = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    gc_remove_pause.release.wait();

    let first_removed = first_collector
        .join()
        .expect("first collector joins")
        .expect("first collector completes removal");
    let second_removed = second_collector
        .join()
        .expect("second collector joins")
        .expect("second collector observes idempotent completion");
    assert!(
        second_claim_committed_while_first_remove_paused,
        "the second sweep must commit its distinct claimed_at while the first provider removal is paused"
    );
    second_remove_entered_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect(
            "second provider removal enters after the first collector releases its digest lock",
        );
    assert_eq!(first_removed + second_removed, 1);
    assert!(matches!(
        first_store
            .inner
            .blobs
            .get(WORKSPACE, BlobPurpose::ResourceUploadChunk, &blob),
        Err(StoreError::NotFound)
    ));
    assert_eq!(reservation_and_fence_counts(&directory), (0, 0, None));
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
    expire_upload_for_gc(
        &restarted,
        "upload-crash-before-put",
        "event-expire-upload-crash-before-put",
    );
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
    expire_upload_for_gc(
        &restarted,
        "upload-crash-after-put",
        "event-expire-upload-crash-after-put",
    );
    assert_eq!(
        restarted
            .collect_orphan_chunks(AFTER_EXPIRY, 100)
            .expect("collector removes unreferenced object"),
        1
    );
    assert_blob_missing(&restarted);
    assert_eq!(reservation_and_fence_counts(&directory), (0, 0, None));
}
