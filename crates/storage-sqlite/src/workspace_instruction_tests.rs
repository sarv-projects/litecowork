use super::*;
use domain_workspace::{CreateWorkspaceInstructionRevision, WorkspaceService};

const WORKSPACE_ID: &str = "workspace-instruction-replay";
const RESOURCE_ID: &str = "resource-instruction-replay";
const RESOURCE_REVISION_ID: &str = "revision-instruction-replay";
const CONTENT: &[u8] = b"Keep local data private.\n";

fn seed_instruction_resource(
    store: &SqliteWorkspaceStore,
    directory: &tempfile::TempDir,
) -> BlobRef {
    let blob = store
        .inner
        .blobs
        .put(WORKSPACE_ID, BlobPurpose::Resource, CONTENT, "text/plain")
        .expect("store instruction Resource bytes");
    let mut connection = Connection::open(state_database(directory)).expect("open fixture DB");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable constraints");
    let transaction = connection.transaction().expect("start fixture transaction");
    transaction
        .execute(
            "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,context_document_json,provenance_json,created_at,updated_at,version)
             VALUES (?1,?2,'FILE','{}','instructions.txt',?3,'PERSONAL',NULL,'{}','2026-10-11T00:00:00Z','2026-10-11T00:00:00Z',1)",
            params![RESOURCE_ID, WORKSPACE_ID, RESOURCE_REVISION_ID],
        )
        .expect("seed instruction Resource");
    transaction
        .execute(
            "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
             VALUES (?1,?2,?3,?4,'text/plain','2026-10-11T00:00:00Z','{}')",
            params![
                RESOURCE_REVISION_ID,
                RESOURCE_ID,
                blob.digest,
                i64::try_from(CONTENT.len()).expect("content length"),
            ],
        )
        .expect("seed instruction Resource revision");
    transaction
        .commit()
        .expect("instruction Resource fixture commits");
    blob
}

fn create_instruction_revision(
    store: &SqliteWorkspaceStore,
    expected_version: u64,
    parent_revisions: Vec<u64>,
    request_id: &str,
    event_id: &str,
) -> Result<CommittedWorkspaceInstructionRevision, StoreError> {
    let content_digest = digest(CONTENT);
    create_instruction_revision_with_resource_ref(
        store,
        expected_version,
        parent_revisions,
        request_id,
        event_id,
        json!({
            "workspace_id": WORKSPACE_ID,
            "resource_id": RESOURCE_ID,
            "revision_id": RESOURCE_REVISION_ID,
        }),
        content_digest,
    )
}

fn create_instruction_revision_with_resource_ref(
    store: &SqliteWorkspaceStore,
    expected_version: u64,
    parent_revisions: Vec<u64>,
    request_id: &str,
    event_id: &str,
    content_ref: Value,
    content_digest: String,
) -> Result<CommittedWorkspaceInstructionRevision, StoreError> {
    let request_payload = json!({
        "operation": "workspace.instructions.revision.create.v1",
        "workspace_id": WORKSPACE_ID,
        "expected_version": expected_version,
        "parent_revisions": parent_revisions,
        "content_ref": content_ref,
        "content_digest": content_digest,
    });
    WorkspaceService::new(store.clone()).create_instruction_revision_idempotent(
        CreateWorkspaceInstructionRevision {
            workspace_id: WORKSPACE_ID.to_owned(),
            expected_version,
            parent_revisions,
            content_ref: request_payload["content_ref"].clone(),
            content_digest,
            authored_by_principal_id: "owner-local".to_owned(),
            event: EventContext {
                event_id: event_id.to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: "2026-10-11T00:01:00Z".to_owned(),
                correlation_id: "correlation-instruction".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-11T00:01:00Z".to_owned(),
            },
        },
        request_id.to_owned(),
        request_payload,
    )
}

#[test]
fn workspace_instruction_revision_commit_replays_original_after_workspace_advances() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    let blob = seed_instruction_resource(&store, &directory);

    let first = create_instruction_revision(
        &store,
        1,
        vec![],
        "instruction-save-1",
        "event-instruction-1",
    )
    .expect("first revision commits");
    assert_eq!(first.instruction_revision.revision, 1);
    assert!(first.instruction_revision.parent_revisions.is_empty());
    assert_eq!(first.workspace.version, 2);
    assert_eq!(
        first.event.event_type,
        "workspace.instructions.revision.created.v1"
    );

    let second = create_instruction_revision(
        &store,
        2,
        vec![1],
        "instruction-save-2",
        "event-instruction-2",
    )
    .expect("second revision commits");
    assert_eq!(second.instruction_revision.revision, 2);
    assert_eq!(second.instruction_revision.parent_revisions, vec![1]);
    assert_eq!(second.workspace.version, 3);

    let replay = create_instruction_revision(
        &store,
        1,
        vec![],
        "instruction-save-1",
        "event-instruction-replay",
    )
    .expect("same idempotency key and payload return the original commit");
    assert_eq!(replay, first);
    assert!(matches!(
        create_instruction_revision(
            &store,
            1,
            vec![1],
            "instruction-save-1",
            "event-instruction-changed-replay",
        ),
        Err(StoreError::Conflict { .. })
    ));

    let workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("read Workspace")
        .expect("Workspace remains present");
    assert_eq!(workspace.current_instruction_revision, Some(2));
    assert_eq!(workspace.version, 3);
    let revisions = store
        .list_workspace_instruction_revisions(WORKSPACE_ID, 0, 10)
        .expect("list instruction history");
    assert_eq!(
        revisions
            .iter()
            .map(|revision| revision.revision)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(revisions[0].content_digest, blob.digest);

    let connection = Connection::open(state_database(&directory)).expect("inspect committed state");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type = 'Workspace' AND type = 'workspace.instructions.revision.created.v1'",
            [],
            |row| row.get(0),
        )
        .expect("count instruction events");
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = 'owner-local' AND request_id IN ('instruction-save-1','instruction-save-2')",
            [],
            |row| row.get(0),
        )
        .expect("count instruction receipts");
    assert_eq!(event_count, 2);
    assert_eq!(receipt_count, 2);
}

#[test]
fn stale_workspace_instruction_revision_write_leaves_no_partial_state() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    seed_instruction_resource(&store, &directory);
    create_instruction_revision(&store, 1, vec![], "instruction-current", "event-current")
        .expect("current instruction revision commits");

    let error = create_instruction_revision(
        &store,
        1,
        vec![1],
        "instruction-stale",
        "event-instruction-stale",
    )
    .expect_err("stale Workspace version must conflict");
    assert!(matches!(error, StoreError::Conflict { .. }), "{error:?}");

    let workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("read Workspace")
        .expect("Workspace remains present");
    assert_eq!(workspace.current_instruction_revision, Some(1));
    assert_eq!(workspace.version, 2);
    assert_eq!(
        store
            .list_workspace_instruction_revisions(WORKSPACE_ID, 0, 10)
            .expect("read instruction history")
            .len(),
        1
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect committed state");
    let counts: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM domain_events WHERE type = 'workspace.instructions.revision.created.v1'),
                (SELECT COUNT(*) FROM request_dedup WHERE principal_id = 'owner-local' AND request_id IN ('instruction-current','instruction-stale')),
                (SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = 'runtime-local'),
                (SELECT COUNT(*) FROM workspace_instruction_revisions WHERE workspace_id = ?1)",
            [WORKSPACE_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read revision event, receipt, sequence, and row counts");
    assert_eq!(counts, (1, 1, 2, 1));
}

#[test]
fn workspace_instruction_event_failure_rolls_back_revision_and_receipt() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    seed_instruction_resource(&store, &directory);
    let connection = Connection::open(state_database(&directory)).expect("open failure fixture");
    connection
        .execute_batch(
            "CREATE TRIGGER reject_instruction_event
             BEFORE INSERT ON domain_events
             WHEN NEW.type = 'workspace.instructions.revision.created.v1'
             BEGIN SELECT RAISE(ABORT, 'injected instruction event failure'); END;",
        )
        .expect("install event failure trigger");

    assert!(
        create_instruction_revision(
            &store,
            1,
            vec![],
            "instruction-event-failure",
            "event-instruction-failure",
        )
        .is_err()
    );

    let workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("read Workspace")
        .expect("Workspace remains present");
    assert_eq!(workspace.current_instruction_revision, None);
    assert_eq!(workspace.version, 1);
    assert!(
        store
            .list_workspace_instruction_revisions(WORKSPACE_ID, 0, 10)
            .expect("read instruction history")
            .is_empty()
    );
    let counts: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM domain_events WHERE type = 'workspace.instructions.revision.created.v1'),
                (SELECT COUNT(*) FROM request_dedup WHERE principal_id = 'owner-local' AND request_id = 'instruction-event-failure'),
                (SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = 'runtime-local'),
                (SELECT COUNT(*) FROM workspace_instruction_revisions WHERE workspace_id = ?1)",
            [WORKSPACE_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read rollback evidence counts");
    assert_eq!(counts, (0, 0, 1, 0));
}

#[test]
fn workspace_instruction_revision_rejects_a_noncurrent_resource_revision() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    let old_bytes = b"older instruction content\n";
    let current_bytes = b"current instruction content\n";
    let old_blob = store
        .inner
        .blobs
        .put(WORKSPACE_ID, BlobPurpose::Resource, old_bytes, "text/plain")
        .expect("store old Resource revision");
    let current_blob = store
        .inner
        .blobs
        .put(
            WORKSPACE_ID,
            BlobPurpose::Resource,
            current_bytes,
            "text/plain",
        )
        .expect("store current Resource revision");
    let mut connection = Connection::open(state_database(&directory)).expect("open fixture DB");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable constraints");
    let transaction = connection
        .transaction()
        .expect("start Resource revision fixture transaction");
    transaction
        .execute(
            "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,context_document_json,provenance_json,created_at,updated_at,version)
             VALUES (?1,?2,'FILE','{}','instructions.txt','instruction-resource-current','PERSONAL',NULL,'{}','2026-10-11T00:00:00Z','2026-10-11T00:01:00Z',2)",
            params![RESOURCE_ID, WORKSPACE_ID],
        )
        .expect("seed Resource with an advanced head");
    for (revision_id, blob, content) in [
        ("instruction-resource-old", &old_blob, old_bytes.as_slice()),
        (
            "instruction-resource-current",
            &current_blob,
            current_bytes.as_slice(),
        ),
    ] {
        transaction
            .execute(
                "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
                 VALUES (?1,?2,?3,?4,'text/plain','2026-10-11T00:00:00Z','{}')",
                params![
                    revision_id,
                    RESOURCE_ID,
                    blob.digest,
                    i64::try_from(content.len()).expect("Resource revision length"),
                ],
            )
            .expect("seed immutable Resource revision");
    }
    transaction
        .commit()
        .expect("Resource revision fixture commits");

    let request_payload = json!({
        "operation": "workspace.instructions.revision.create.v1",
        "workspace_id": WORKSPACE_ID,
        "expected_version": 1,
        "parent_revisions": [],
        "content_ref": {
            "workspace_id": WORKSPACE_ID,
            "resource_id": RESOURCE_ID,
            "revision_id": "instruction-resource-old",
        },
        "content_digest": old_blob.digest,
    });
    let result = WorkspaceService::new(store.clone()).create_instruction_revision_idempotent(
        CreateWorkspaceInstructionRevision {
            workspace_id: WORKSPACE_ID.to_owned(),
            expected_version: 1,
            parent_revisions: vec![],
            content_ref: json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": RESOURCE_ID,
                "revision_id": "instruction-resource-old",
            }),
            content_digest: old_blob.digest,
            authored_by_principal_id: "owner-local".to_owned(),
            event: EventContext {
                event_id: "event-noncurrent-instruction".to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: "2026-10-11T00:02:00Z".to_owned(),
                correlation_id: "correlation-noncurrent-instruction".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-11T00:02:00Z".to_owned(),
            },
        },
        "instruction-noncurrent-resource".to_owned(),
        request_payload,
    );
    assert!(
        matches!(result, Err(StoreError::Invalid(_))),
        "historical Resource head was unexpectedly accepted: {result:?}"
    );
    let workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("read Workspace")
        .expect("Workspace remains present");
    assert_eq!(workspace.current_instruction_revision, None);
    assert_eq!(workspace.version, 1);
    assert!(
        store
            .list_workspace_instruction_revisions(WORKSPACE_ID, 0, 10)
            .expect("read instruction history")
            .is_empty()
    );
}

#[test]
fn workspace_instruction_refs_reject_foreign_missing_and_unpinned_resources_without_writes() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE_ID);
    seed_instruction_resource(&store, &directory);

    let cases = [
        (
            "instruction-reference-negative-foreign",
            json!({
                "workspace_id": "another-workspace",
                "resource_id": RESOURCE_ID,
                "revision_id": RESOURCE_REVISION_ID,
            }),
            true,
        ),
        (
            "instruction-reference-negative-missing-resource",
            json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": "resource-that-does-not-exist",
                "revision_id": RESOURCE_REVISION_ID,
            }),
            false,
        ),
        (
            "instruction-reference-negative-missing-revision",
            json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": RESOURCE_ID,
                "revision_id": "revision-that-does-not-exist",
            }),
            false,
        ),
        (
            "instruction-reference-negative-unpinned-null",
            json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": RESOURCE_ID,
                "revision_id": null,
            }),
            false,
        ),
        (
            "instruction-reference-negative-unpinned-missing",
            json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": RESOURCE_ID,
            }),
            false,
        ),
    ];
    for (request_id, content_ref, invalid_scope) in cases {
        let result = create_instruction_revision_with_resource_ref(
            &store,
            1,
            vec![],
            request_id,
            "event-instruction-reference-negative",
            content_ref,
            digest(CONTENT),
        );
        if invalid_scope {
            assert!(
                matches!(result, Err(StoreError::Invalid(_))),
                "foreign ResourceRef was not rejected: {result:?}"
            );
        } else {
            assert!(
                matches!(result, Err(StoreError::NotFound)),
                "missing or unpinned ResourceRef was not rejected: {result:?}"
            );
        }
    }

    let workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("read Workspace after rejected ResourceRefs")
        .expect("Workspace remains present");
    assert_eq!(workspace.current_instruction_revision, None);
    assert_eq!(workspace.version, 1);
    assert!(
        store
            .list_workspace_instruction_revisions(WORKSPACE_ID, 0, 10)
            .expect("read rejected instruction history")
            .is_empty()
    );
    let connection = Connection::open(state_database(&directory)).expect("open fixture DB");
    let (instruction_events, receipts): (i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM domain_events WHERE type = 'workspace.instructions.revision.created.v1'),
                (SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'instruction-reference-negative-%')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("inspect rejected instruction state");
    assert_eq!((instruction_events, receipts), (0, 0));
}
