//! Focused tests for the Core-owned Resource index state projection.

use super::*;
use std::sync::Arc;
use std::time::Duration;

use storage_core::{
    EventDraft, ResourceRecord, ResourceRevisionRecord, ResourceStore, ResourceUploadSessionRecord,
    ResourceUploadState, ResourceUploadStore, StoreError, WorkspaceCreateRequest,
};

use crate::blob::{WorkspaceBlobKey, WorkspaceBlobKeyProvider};
use crate::FileBlobStore;
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};

const WORKSPACE: &str = "workspace-rag-index-state";
const OWNER: &str = "owner-rag-index-state";
const CREATED: &str = "2026-10-20T10:00:00Z";
const EXPIRES: &str = "2026-10-20T10:01:00Z";
const TEXT: &[u8] = b"the quick brown fox jumps over the lazy dog";
const ZIP: &[u8] = b"PK\x03\x04replacement head";

#[derive(Clone)]
struct TestKeys([u8; 32]);

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: zeroize::Zeroizing::new(self.0),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob("unknown test key version".to_owned()));
        }
        self.current_key(workspace_id, purpose)
    }
}

fn state_database(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory.path().join("state").join("state.sqlite3")
}

fn open_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
    SqliteWorkspaceStore::open(
        state_database(directory),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys([23_u8; 32]),
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("SQLite store opens")
}

fn create_workspace(store: &SqliteWorkspaceStore) {
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "RAG index state Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "event-rag-index-state-workspace".to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: CREATED.to_owned(),
                correlation_id: "correlation-rag-index-state-workspace".to_owned(),
                causation_id: None,
                recorded_at: CREATED.to_owned(),
            },
        })
        .expect("Workspace commits");
}

fn create_text_resource(
    store: &SqliteWorkspaceStore,
    resource_id: &str,
    display_name: &str,
) -> Result<storage_core::CommittedResource, StoreError> {
    create_resource_with_media_type(store, resource_id, display_name, "text/plain", TEXT)
}

fn create_resource_with_media_type(
    store: &SqliteWorkspaceStore,
    resource_id: &str,
    display_name: &str,
    media_type: &str,
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
        media_type: Some(media_type.to_owned()),
        observed_at: CREATED.to_owned(),
        created_by: json!({"kind": "USER", "principal_id": OWNER}),
    };
    let event_id = format!("event-resource-create-{resource_id}");
    store.create_resource(
        WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: format!("request-resource-create-{resource_id}"),
            request_payload: json!({
                "operation": "resource.quick_import.v1",
                "workspace_id": WORKSPACE,
                "display_name": display_name,
                "media_type": media_type,
                "content_digest": content_digest,
                "size_bytes": content.len(),
            }),
        },
        resource,
        revision,
        EventDraft {
            event_id: event_id.clone(),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "Resource".to_owned(),
            entity_id: resource_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 1,
            hlc_timestamp: CREATED.to_owned(),
            correlation_id: event_id,
            causation_id: None,
            schema_version: 1,
            event_type: "resource.created.v1".to_owned(),
            payload: json!({
                "resource_id": resource_id,
                "workspace_id": WORKSPACE,
                "kind": "FILE",
                "aggregate_version": 1,
            }),
            recorded_at: CREATED.to_owned(),
        },
        content.to_vec(),
    )
}

/// Advances the Resource head with a new unindexable ZIP revision through the
/// revision-upload protocol, so the previous text projection becomes superseded.
fn advance_head_with_zip_revision(
    store: &SqliteWorkspaceStore,
    resource_id: &str,
    upload_id: &str,
    revision_id: &str,
) -> Result<(), StoreError> {
    let expected_digest = digest(ZIP);
    let event_id = format!("event-create-revision-upload-{upload_id}");
    store.create_revision(
        WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: format!("request-create-revision-upload-{upload_id}"),
            request_payload: json!({
                "operation": "resource.revision.upload.create.v1",
                "workspace_id": WORKSPACE,
                "resource_id": resource_id,
                "expected_resource_version": 1,
                "parent_revision_ids": [format!("revision-{resource_id}")],
                "media_type": "application/zip",
                "expected_digest": expected_digest,
                "size_bytes": ZIP.len(),
            }),
        },
        ResourceUploadSessionRecord {
            upload_id: upload_id.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            display_name: String::new(),
            media_type: "application/zip".to_owned(),
            expected_size_bytes: ZIP.len() as u64,
            expected_digest: Some(expected_digest.clone()),
            context_document: None,
            folder_import: None,
            resource_id: Some(resource_id.to_owned()),
            committed_resource_id: None,
            expected_resource_version: Some(1),
            parent_revision_ids: vec![format!("revision-{resource_id}")],
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
            correlation_id: event_id.clone(),
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.created.v1".to_owned(),
            payload: json!({
                "upload_id": upload_id,
                "workspace_id": WORKSPACE,
                "expected_size_bytes": ZIP.len(),
                "expected_digest": expected_digest,
                "chunk_size_bytes": 4_194_304,
                "expires_at": EXPIRES,
                "resource_id": resource_id,
                "expected_resource_version": 1,
                "parent_revision_ids": [format!("revision-{resource_id}")],
                "aggregate_version": 1,
            }),
            recorded_at: CREATED.to_owned(),
        },
    )?;

    let chunk = storage_core::ResourceUploadChunkInput {
        upload_id: upload_id.to_owned(),
        chunk_index: 0,
        request_id: format!("request-chunk-{upload_id}"),
        content_range: storage_core::ResourceUploadContentRange {
            start_offset: 0,
            end_offset_inclusive: ZIP.len() as u64 - 1,
            total_size_bytes: ZIP.len() as u64,
        },
        sha256: expected_digest.clone(),
        content: ZIP.to_vec(),
        received_at: CREATED.to_owned(),
        lifecycle_event: EventDraft {
            event_id: format!("event-chunk-{upload_id}"),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: upload_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 1,
            hlc_timestamp: CREATED.to_owned(),
            correlation_id: format!("correlation-chunk-{upload_id}"),
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.status.changed.v1".to_owned(),
            payload: json!({
                "upload_id": upload_id,
                "from": "OPEN",
                "to": "CONTENT_RECEIVED",
                "aggregate_version": 2,
            }),
            recorded_at: CREATED.to_owned(),
        },
    };
    store.put_chunk(chunk)?;

    let session = store
        .get(WORKSPACE, upload_id)?
        .ok_or(StoreError::NotFound)?;
    let mut resource = store
        .get_resource_record(WORKSPACE, resource_id)?
        .ok_or(StoreError::NotFound)?;
    resource.version += 1;
    resource.current_revision_id = Some(revision_id.to_owned());
    resource.updated_at = CREATED.to_owned();
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id.to_owned(),
        resource_id: resource_id.to_owned(),
        parent_revision_ids: session.parent_revision_ids.clone(),
        provider_revision: None,
        content_digest: Some(expected_digest.clone()),
        size_bytes: Some(ZIP.len() as u64),
        media_type: Some("application/zip".to_owned()),
        observed_at: CREATED.to_owned(),
        created_by: json!({"kind": "USER", "principal_id": OWNER}),
    };
    let resource_event_id = format!("event-revision-{revision_id}");
    let upload_event_version = session.version + 1;
    store.commit(
        WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: format!("request-commit-revision-{upload_id}"),
            request_payload: json!({}),
        },
        upload_id,
        resource,
        revision,
        EventDraft {
            event_id: resource_event_id.clone(),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "Resource".to_owned(),
            entity_id: resource_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 2,
            hlc_timestamp: CREATED.to_owned(),
            correlation_id: resource_event_id,
            causation_id: None,
            schema_version: 1,
            event_type: "resource.revision.created.v1".to_owned(),
            payload: json!({
                "resource_id": resource_id,
                "resource_revision_id": revision_id,
                "parent_revision_ids": session.parent_revision_ids,
                "content_digest": expected_digest,
                "size_bytes": ZIP.len(),
                "media_type": "application/zip",
                "created_by": json!({"kind": "USER", "principal_id": OWNER}),
                "aggregate_version": 2,
            }),
            recorded_at: CREATED.to_owned(),
        },
        Some(EventDraft {
            event_id: format!("event-upload-commit-{upload_id}"),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: upload_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: upload_event_version,
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
                "aggregate_version": upload_event_version,
            }),
            recorded_at: CREATED.to_owned(),
        }),
        None,
    )?;
    Ok(())
}

#[test]
fn an_indexed_text_resource_reports_a_ready_index_state() {
    let directory = tempfile::tempdir().expect("temporary index state directory");
    let store = open_store(&directory);
    create_workspace(&store);
    create_text_resource(&store, "resource-index-ready", "ready.txt")
        .expect("indexed text Resource commits");

    let state = store
        .resource_index_state(WORKSPACE, "resource-index-ready")
        .expect("index state reads")
        .expect("the Resource exists in the selected Workspace");
    assert_eq!(
        state.current_revision_id.as_deref(),
        Some("revision-resource-index-ready")
    );
    assert_eq!(
        state.indexed_revision_id.as_deref(),
        Some("revision-resource-index-ready")
    );
    assert_eq!(state.parser_id.as_deref(), Some("litecowork.plain-text.v1"));
    assert_eq!(state.superseded_index_count, 0);
    assert_eq!(
        state.indexed_source_digest,
        state.current_content_digest,
        "a ready projection is bound to the current content digest"
    );
}

#[test]
fn a_moved_head_reports_a_stale_index_and_keeps_the_superseded_projection_visible() {
    let directory = tempfile::tempdir().expect("temporary index state directory");
    let store = open_store(&directory);
    create_workspace(&store);
    create_text_resource(&store, "resource-index-stale", "stale.txt")
        .expect("indexed text Resource commits");
    advance_head_with_zip_revision(
        &store,
        "resource-index-stale",
        "upload-index-stale",
        "revision-index-stale-zip",
    )
    .expect("the Resource head advances to an unindexable revision");

    let state = store
        .resource_index_state(WORKSPACE, "resource-index-stale")
        .expect("index state reads")
        .expect("the Resource exists in the selected Workspace");
    assert_eq!(
        state.current_revision_id.as_deref(),
        Some("revision-index-stale-zip")
    );
    assert_eq!(
        state.indexed_revision_id, None,
        "the moved head has no projection of its own"
    );
    assert_eq!(
        state.superseded_index_count, 1,
        "the superseded projection stays registered until it is purged"
    );
}

#[test]
fn an_unsupported_resource_reports_a_typed_not_indexable_reason() {
    let directory = tempfile::tempdir().expect("temporary index state directory");
    let store = open_store(&directory);
    create_workspace(&store);
    create_resource_with_media_type(
        &store,
        "resource-index-binary",
        "archive.zip",
        "application/zip",
        ZIP,
    )
    .expect("Resource commits");

    let state = store
        .resource_index_state(WORKSPACE, "resource-index-binary")
        .expect("index state reads")
        .expect("the Resource exists in the selected Workspace");
    // A ZIP is never indexed by the qualified local provider, and it is not an
    // allowlisted plain text media type.
    assert_eq!(
        rag_retrieval::not_indexable_reason(
            &state.display_name,
            &state.media_type,
            state.current_size_bytes
        ),
        Some(storage_core::ResourceTextIndexSkipReason::UnsupportedType)
    );
    assert_eq!(state.indexed_revision_id, None);
    assert_eq!(state.superseded_index_count, 0);
}

#[test]
fn an_unknown_or_out_of_scope_resource_has_no_index_state() {
    let directory = tempfile::tempdir().expect("temporary index state directory");
    let store = open_store(&directory);
    create_workspace(&store);
    create_text_resource(&store, "resource-index-scoped", "scoped.txt")
        .expect("indexed text Resource commits");

    assert!(
        store
            .resource_index_state(WORKSPACE, "resource-index-missing")
            .expect("index state reads")
            .is_none(),
        "an unknown Resource has no index state"
    );
    assert!(
        store
            .resource_index_state("workspace-other", "resource-index-scoped")
            .expect("index state reads")
            .is_none(),
        "a Resource in another Workspace is not visible"
    );
}
