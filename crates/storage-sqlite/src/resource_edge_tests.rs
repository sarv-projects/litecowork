use super::*;
use storage_core::{
    DerivedResourceCreateCommit, ResourceEdgeRecord, ResourceRecord, ResourceRevisionRecord,
    WorkspaceCreateRequest,
};
use zeroize::Zeroizing;

struct EdgeTestKeys;

impl WorkspaceBlobKeyProvider for EdgeTestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([73; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob("unknown edge-test key version".to_owned()));
        }
        self.current_key(workspace_id, purpose)
    }
}

fn store() -> (tempfile::TempDir, SqliteWorkspaceStore) {
    let directory = tempfile::tempdir().expect("temporary edge storage");
    let state_directory = directory.path().join("state");
    ensure_private_directory(&state_directory).expect("create private edge state directory");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        EdgeTestKeys,
    ));
    let store = SqliteWorkspaceStore::open(
        state_directory.join("litecowork.sqlite3"),
        blobs,
        SqliteConfig::default(),
    )
    .expect("open edge-test storage");
    (directory, store)
}

fn create_workspace(store: &SqliteWorkspaceStore, workspace_id: &str, suffix: &str) {
    domain_workspace::WorkspaceService::new(store.clone())
        .create(domain_workspace::CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Edge test workspace".to_owned(),
            owner_principal_id: "edge-owner".to_owned(),
            event: domain_workspace::EventContext {
                event_id: format!("workspace-event-{suffix}"),
                origin_runtime_id: "runtime-edge-test".to_owned(),
                hlc_timestamp: "2026-10-18T12:00:00Z".to_owned(),
                correlation_id: format!("workspace-correlation-{suffix}"),
                causation_id: None,
                recorded_at: "2026-10-18T12:00:00Z".to_owned(),
            },
        })
        .expect("create edge-test Workspace");
}

fn create_resource(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    id: &str,
) -> CommittedResource {
    let content = format!("content for {id}").into_bytes();
    let observed_at = "2026-10-18T12:01:00Z".to_owned();
    let digest = digest(&content);
    store
        .create_resource(
            WorkspaceCreateRequest {
                principal_id: "edge-owner".to_owned(),
                request_id: format!("request-create-{id}"),
                request_payload: json!({"resource_id":id,"content_digest":&digest}),
            },
            ResourceRecord {
                resource_id: id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                kind: "FILE".to_owned(),
                provider_identity: json!({}),
                identity_digest: None,
                display_name: format!("{id}.txt"),
                current_revision_id: Some(format!("revision-{id}")),
                sensitivity: "PERSONAL".to_owned(),
                provenance: json!({"source_inputs":[],"transformations":[],"tool_reports":[]}),
                created_at: observed_at.clone(),
                updated_at: observed_at.clone(),
                version: 1,
            },
            ResourceRevisionRecord {
                resource_revision_id: format!("revision-{id}"),
                resource_id: id.to_owned(),
                parent_revision_ids: vec![],
                provider_revision: None,
                content_digest: Some(digest),
                size_bytes: Some(content.len() as u64),
                media_type: Some("text/plain".to_owned()),
                observed_at: observed_at.clone(),
                created_by: json!({"kind":"OWNER","principal_id":"edge-owner"}),
            },
            EventDraft {
                event_id: format!("event-create-{id}"),
                workspace_id: workspace_id.to_owned(),
                entity_type: "Resource".to_owned(),
                entity_id: id.to_owned(),
                origin_runtime_id: "runtime-edge-test".to_owned(),
                entity_revision: 1,
                hlc_timestamp: observed_at.clone(),
                correlation_id: format!("correlation-create-{id}"),
                causation_id: None,
                schema_version: 2,
                event_type: "resource.created.v2".to_owned(),
                payload: json!({}),
                recorded_at: observed_at,
            },
            content,
        )
        .expect("create test Resource")
}

fn derived_commit(
    source: &CommittedResource,
    child_id: &str,
    edge_id: &str,
    request_id: &str,
) -> DerivedResourceCreateCommit {
    let content = format!("derived content for {child_id}").into_bytes();
    let observed_at = "2026-10-18T12:02:00Z".to_owned();
    let content_digest = digest(&content);
    let revision_id = format!("revision-{child_id}");
    let edge = ResourceEdgeRecord {
        edge_id: edge_id.to_owned(),
        workspace_id: source.resource.workspace_id.clone(),
        from_resource_id: source.resource.resource_id.clone(),
        from_revision_id: Some(source.revision.resource_revision_id.clone()),
        relation: "DERIVED_FROM".to_owned(),
        to_resource_id: child_id.to_owned(),
        to_revision_id: Some(revision_id.clone()),
        observed_at: observed_at.clone(),
        provenance: json!({"provider":"archive-extractor","source":"zip-entry"}),
    };
    DerivedResourceCreateCommit {
        request: WorkspaceCreateRequest {
            principal_id: "edge-owner".to_owned(),
            request_id: request_id.to_owned(),
            request_payload: json!({"workspace_id":source.resource.workspace_id,"child":child_id}),
        },
        resource: ResourceRecord {
            resource_id: child_id.to_owned(),
            workspace_id: source.resource.workspace_id.clone(),
            kind: "FILE".to_owned(),
            provider_identity: json!({}),
            identity_digest: None,
            display_name: format!("{child_id}.txt"),
            current_revision_id: Some(revision_id.clone()),
            sensitivity: "PERSONAL".to_owned(),
            provenance: json!({"source_inputs":[source.resource.resource_id]}),
            created_at: observed_at.clone(),
            updated_at: observed_at.clone(),
            version: 1,
        },
        revision: ResourceRevisionRecord {
            resource_revision_id: revision_id,
            resource_id: child_id.to_owned(),
            parent_revision_ids: vec![],
            provider_revision: None,
            content_digest: Some(content_digest),
            size_bytes: Some(content.len() as u64),
            media_type: Some("text/plain".to_owned()),
            observed_at: observed_at.clone(),
            created_by: json!({"kind":"OWNER","principal_id":"edge-owner"}),
        },
        resource_event: EventDraft {
            event_id: format!("event-create-{child_id}"),
            workspace_id: source.resource.workspace_id.clone(),
            entity_type: "Resource".to_owned(),
            entity_id: child_id.to_owned(),
            origin_runtime_id: "runtime-edge-test".to_owned(),
            entity_revision: 1,
            hlc_timestamp: observed_at.clone(),
            correlation_id: format!("correlation-create-{child_id}"),
            causation_id: None,
            schema_version: 2,
            event_type: "resource.created.v2".to_owned(),
            payload: json!({}),
            recorded_at: observed_at.clone(),
        },
        edge_event: EventDraft {
            event_id: format!("event-{edge_id}"),
            workspace_id: source.resource.workspace_id.clone(),
            entity_type: "ResourceEdge".to_owned(),
            entity_id: edge_id.to_owned(),
            origin_runtime_id: "runtime-edge-test".to_owned(),
            entity_revision: 1,
            hlc_timestamp: observed_at.clone(),
            correlation_id: format!("correlation-{edge_id}"),
            causation_id: None,
            schema_version: 1,
            event_type: "resource.edge.created.v1".to_owned(),
            payload: json!({
                "edge_id": &edge.edge_id,
                "from_resource_id": &edge.from_resource_id,
                "to_resource_id": &edge.to_resource_id,
                "relation": &edge.relation,
                "observed_at": &edge.observed_at,
            }),
            recorded_at: observed_at,
        },
        edge,
        content,
    }
}

#[test]
fn derived_resource_and_edge_commit_once_and_replay_as_one_receipt() {
    let (_directory, store) = store();
    create_workspace(&store, "workspace-edge", "edge");
    let source = create_resource(&store, "workspace-edge", "source");
    let commit = derived_commit(&source, "child", "edge-child", "request-derived");

    let first = store
        .create_derived_resource(commit.clone())
        .expect("commit child and edge atomically");
    let replay = store
        .create_derived_resource(commit)
        .expect("replay the child and edge receipt");
    assert_eq!(first, replay);
    let changed = derived_commit(&source, "child", "edge-changed", "request-derived");
    assert!(store.create_derived_resource(changed).is_err());
    assert_eq!(first.edge_event.event_type, "resource.edge.created.v1");
    assert_eq!(
        first.edge_event.origin_sequence,
        first.resource.event.origin_sequence + 1
    );

    let connection = Connection::open(&store.inner.database_path).expect("inspect edge store");
    let (resources, edges, events, receipts): (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM resources WHERE resource_id='child'),
               (SELECT COUNT(*) FROM resource_edges WHERE edge_id='edge-child'),
               (SELECT COUNT(*) FROM domain_events WHERE entity_id IN ('child','edge-child')),
               (SELECT COUNT(*) FROM request_dedup WHERE request_id='request-derived')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read committed derived projection");
    assert_eq!((resources, edges, events, receipts), (1, 1, 2, 1));
    assert!(
        connection
            .execute(
                "UPDATE resource_edges SET relation='REFERENCES' WHERE edge_id='edge-child'",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM resource_edges WHERE edge_id='edge-child'", [])
            .is_err()
    );
    let retained_edge: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_edges WHERE edge_id='edge-child'",
            [],
            |row| row.get(0),
        )
        .expect("read immutable provenance edge");
    assert_eq!(retained_edge, 1);
}

#[test]
fn derived_resource_edge_rejects_cross_workspace_and_missing_revision_atomically() {
    let (_directory, store) = store();
    create_workspace(&store, "workspace-edge-a", "a");
    create_workspace(&store, "workspace-edge-b", "b");
    let source_a = create_resource(&store, "workspace-edge-a", "source-a");
    let source_b = create_resource(&store, "workspace-edge-b", "source-b");

    let mut cross_workspace =
        derived_commit(&source_a, "child-cross", "edge-cross", "request-cross");
    cross_workspace.edge.from_resource_id = source_b.resource.resource_id.clone();
    cross_workspace.edge.from_revision_id = Some(source_b.revision.resource_revision_id.clone());
    cross_workspace.edge_event.payload["from_resource_id"] =
        json!(cross_workspace.edge.from_resource_id.clone());
    assert!(store.create_derived_resource(cross_workspace).is_err());

    let mut missing_revision = derived_commit(
        &source_a,
        "child-missing",
        "edge-missing",
        "request-missing",
    );
    missing_revision.edge.from_revision_id = Some("revision-does-not-exist".to_owned());
    assert!(store.create_derived_resource(missing_revision).is_err());

    let connection =
        Connection::open(&store.inner.database_path).expect("inspect rolled-back rows");
    let (children, edges, receipts): (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM resources WHERE resource_id IN ('child-cross','child-missing')),
               (SELECT COUNT(*) FROM resource_edges WHERE edge_id IN ('edge-cross','edge-missing')),
               (SELECT COUNT(*) FROM request_dedup WHERE request_id IN ('request-cross','request-missing'))",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read rejected derived projections");
    assert_eq!((children, edges, receipts), (0, 0, 0));
}
