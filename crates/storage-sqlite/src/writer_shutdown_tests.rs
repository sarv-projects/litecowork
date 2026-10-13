use super::*;

#[test]
fn saturated_writer_drains_abandoned_replies_before_shutdown_and_replay() {
    const WORKSPACE_ID: &str = "workspace-writer-shutdown";

    let directory = tempfile::tempdir().expect("temporary store directory");
    let database = state_database(&directory);
    let blob_root = directory.path().join("blobs");
    let blobs: Arc<dyn storage_core::BlobStore> = Arc::new(FileBlobStore::new(
        &blob_root,
        TestKeys { key: [19_u8; 32] },
    ));
    let store = SqliteWorkspaceStore::open(
        &database,
        Arc::clone(&blobs),
        SqliteConfig {
            writer_queue_capacity: 3,
            busy_timeout: Duration::from_secs(1),
        },
    )
    .expect("SQLite store opens");
    let created = service(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Writer shutdown".to_owned(),
            owner_principal_id: "owner-local".to_owned(),
            event: context("event-writer-shutdown-create", 0),
        })
        .expect("Workspace creation commits");

    let mut updated = created.workspace;
    updated.replication_policy = ReplicationPolicy::MetadataOnly;
    updated.version = 2;
    updated.updated_at = "2026-10-12T12:00:00Z".to_owned();
    let state_bytes = canonical_json(&updated).expect("canonical Workspace state");
    let state_ref = AggregateStateRef {
        blob: blobs
            .put(
                WORKSPACE_ID,
                BlobPurpose::AggregateState,
                &state_bytes,
                STATE_MEDIA_TYPE,
            )
            .expect("write immutable state blob"),
        entity_revision: 2,
        record_schema_version: 1,
    };
    let event = EventDraft {
        event_id: "event-writer-shutdown-update".to_owned(),
        workspace_id: WORKSPACE_ID.to_owned(),
        entity_type: "Workspace".to_owned(),
        entity_id: WORKSPACE_ID.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        entity_revision: 2,
        hlc_timestamp: "2026-10-12T12:00:00Z".to_owned(),
        correlation_id: "correlation-writer-shutdown".to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "workspace.replication_policy.changed.v1".to_owned(),
        payload: serde_json::json!({
            "workspace_id": WORKSPACE_ID,
            "from": "LOCAL_ONLY",
            "to": "METADATA_ONLY",
            "replication_scope_root_ids": [],
            "aggregate_version": 2,
        }),
        recorded_at: "2026-10-12T12:00:00Z".to_owned(),
    };

    let (writer_started, writer_started_receiver) = mpsc::channel();
    let (release_writer, release_writer_receiver) = mpsc::channel();
    assert!(
        store
            .inner
            .sender
            .send(Command::ConversationOperation {
                operation: Box::new(move |_| {
                    writer_started.send(()).expect("signal writer entered gate");
                    release_writer_receiver
                        .recv()
                        .expect("test releases writer gate");
                }),
            })
            .is_ok()
    );
    writer_started_receiver
        .recv()
        .expect("writer reaches deterministic gate");

    let (abandoned_read_reply, abandoned_read_receiver) = mpsc::channel();
    assert!(
        store
            .inner
            .sender
            .send(Command::GetWorkspace {
                workspace_id: WORKSPACE_ID.to_owned(),
                reply: abandoned_read_reply,
            })
            .is_ok()
    );
    drop(abandoned_read_receiver);

    let (abandoned_write_reply, abandoned_write_receiver) = mpsc::channel();
    assert!(
        store
            .inner
            .sender
            .send(Command::CommitWorkspace {
                commit: Box::new(WorkspaceCommit {
                    expected_version: Some(1),
                    workspace: updated.clone(),
                    draft: event,
                    state_ref,
                    request: None,
                }),
                reply: abandoned_write_reply,
            })
            .is_ok()
    );
    drop(abandoned_write_receiver);

    assert!(store.inner.sender.send(Command::Shutdown).is_ok());
    let (full_queue_reply, full_queue_receiver) = mpsc::channel();
    drop(full_queue_receiver);
    assert!(matches!(
        store.inner.sender.try_send(Command::GetWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            reply: full_queue_reply,
        }),
        Err(mpsc::TrySendError::Full(_))
    ));

    release_writer.send(()).expect("release writer gate");
    if let Some(join) = store.inner.join.lock().expect("writer join lock").take() {
        join.join().expect("writer drains commands and shuts down");
    }
    drop(store);

    let reopened = SqliteWorkspaceStore::open(
        &database,
        Arc::new(FileBlobStore::new(
            &blob_root,
            TestKeys { key: [19_u8; 32] },
        )),
        SqliteConfig {
            writer_queue_capacity: 3,
            busy_timeout: Duration::from_secs(1),
        },
    )
    .expect("store reopens after writer shutdown");
    let current = reopened
        .get_workspace(WORKSPACE_ID)
        .expect("read reopened Workspace")
        .expect("admitted write commits without its response waiter");
    assert_eq!(current.version, 2);
    assert_eq!(current.replication_policy, ReplicationPolicy::MetadataOnly);
    assert_eq!(
        reopened
            .read_workspace_events(WORKSPACE_ID)
            .expect("read reopened events")
            .len(),
        2
    );
    assert_eq!(
        reopened
            .rebuild_workspace_projection(WORKSPACE_ID)
            .expect("replay reopened events"),
        Some(current)
    );
}
