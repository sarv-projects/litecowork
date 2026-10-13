//! Read-store checks seed committed SQL records; this is not a publication service.
use super::*;
use storage_core::{
    ArtifactContentRecord, ArtifactReadStore, DependencyFreshness,
    DependencyInvalidationPageRequest, ResourceDependencyPageRequest, ResourceDependencyReadStore,
    TaskPresentationReadStore,
};
use storage_core::{ArtifactLibraryAction, ArtifactLibraryCommand, ArtifactLibraryWriteStore};

fn library_request(
    action: ArtifactLibraryAction,
    expected_version: u64,
    request_id: &str,
) -> ArtifactLibraryCommand {
    ArtifactLibraryCommand {
        principal_id: "owner-local".to_owned(),
        workspace_id: "workspace-artifact".to_owned(),
        artifact_id: "artifact-report".to_owned(),
        action,
        expected_version,
        request_id: request_id.to_owned(),
        event: EventDraft {
            event_id: format!("event-{request_id}"),
            workspace_id: "workspace-artifact".to_owned(),
            entity_type: "Artifact".to_owned(),
            entity_id: "artifact-report".to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: expected_version,
            hlc_timestamp: "2026-10-08T00:01:00Z".to_owned(),
            correlation_id: format!("correlation-{request_id}"),
            causation_id: None,
            schema_version: 1,
            event_type: String::new(),
            payload: json!({}),
            recorded_at: "2026-10-08T00:01:00Z".to_owned(),
        },
    }
}

#[test]
fn library_transitions_keep_history_and_replay_original_receipt_after_archive_and_restart() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    publish_fixture(&store, &directory, 1, b"immutable");
    let heads = store
        .get_artifact_append_heads("workspace-artifact", "artifact-report")
        .unwrap()
        .unwrap();
    let saved = store
        .change_artifact_library(library_request(
            ArtifactLibraryAction::Promote,
            1,
            "promote",
        ))
        .unwrap();
    assert_eq!(saved.artifact.library_status, "SAVED");
    assert_eq!(saved.artifact.version, 2);
    assert_eq!(saved.artifact.current_version, 1);
    assert_eq!(
        saved.event.as_ref().unwrap().event_type,
        "artifact.library.promoted.v1"
    );
    let archived = store
        .change_artifact_library(library_request(
            ArtifactLibraryAction::Archive,
            2,
            "archive",
        ))
        .unwrap();
    assert_eq!(archived.artifact.library_status, "ARCHIVED");
    assert_eq!(archived.artifact.version, 3);
    assert_eq!(
        archived.event.as_ref().unwrap().payload,
        json!({"artifact_id":"artifact-report", "from":"SAVED", "to":"ARCHIVED", "aggregate_version":3})
    );
    assert_eq!(
        store
            .get_artifact_append_heads("workspace-artifact", "artifact-report")
            .unwrap()
            .unwrap()
            .resource,
        heads.resource
    );
    assert_eq!(
        store
            .read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 100)
            .unwrap()
            .unwrap(),
        b"immutable"
    );
    assert!(
        store
            .list_artifacts_page("workspace-artifact", Some("SAVED"), None, None, None, 50)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .list_artifacts_page("workspace-artifact", Some("ARCHIVED"), None, None, None, 50)
            .unwrap()
            .len(),
        1
    );
    assert!(matches!(
        store.change_artifact_library(library_request(ArtifactLibraryAction::Archive, 2, "stale")),
        Err(StoreError::Conflict { .. })
    ));
    let noop = store
        .change_artifact_library(library_request(ArtifactLibraryAction::Archive, 3, "noop"))
        .unwrap();
    assert_eq!(noop.artifact, archived.artifact);
    assert!(noop.event.is_none());
    assert!(
        store
            .change_artifact_library(library_request(ArtifactLibraryAction::Archive, 3, "noop"))
            .unwrap()
            .replayed
    );
    drop(store);
    let store = test_store(&directory, Duration::from_secs(1));
    let replayed = store
        .change_artifact_library(library_request(
            ArtifactLibraryAction::Promote,
            1,
            "promote",
        ))
        .unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.artifact, saved.artifact);
    assert_eq!(replayed.event, saved.event);
    let connection = Connection::open(state_database(&directory)).unwrap();
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type = 'Artifact'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn library_rejects_illegal_transitions_owner_mismatch_and_changed_request_key() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    publish_fixture(&store, &directory, 1, b"immutable");
    assert!(
        matches!(store.change_artifact_library(library_request(ArtifactLibraryAction::Archive, 1, "illegal")), Err(StoreError::Invalid(ref code)) if code == "INVALID_ARTIFACT_TRANSITION")
    );
    let mut denied = library_request(ArtifactLibraryAction::Promote, 1, "denied");
    denied.principal_id = "someone-else".to_owned();
    assert!(matches!(
        store.change_artifact_library(denied),
        Err(StoreError::NotFound)
    ));
    store
        .change_artifact_library(library_request(
            ArtifactLibraryAction::Promote,
            1,
            "shared-key",
        ))
        .unwrap();
    assert!(
        matches!(store.change_artifact_library(library_request(ArtifactLibraryAction::Archive, 2, "shared-key")), Err(StoreError::Invalid(ref code)) if code == "IDEMPOTENCY_CONFLICT")
    );
    assert!(
        matches!(store.change_artifact_library(library_request(ArtifactLibraryAction::Promote, 2, "already-saved")), Err(StoreError::Invalid(ref code)) if code == "INVALID_ARTIFACT_TRANSITION")
    );
    let connection = Connection::open(state_database(&directory)).unwrap();
    connection.execute("UPDATE workspaces SET status = 'ARCHIVED', version = version + 1 WHERE workspace_id = 'workspace-artifact'", []).unwrap();
    assert!(
        matches!(store.change_artifact_library(library_request(ArtifactLibraryAction::Archive, 2, "workspace-archived")), Err(StoreError::Invalid(ref code)) if code == "WORKSPACE_ARCHIVED")
    );
    assert!(
        store
            .change_artifact_library(library_request(
                ArtifactLibraryAction::Promote,
                1,
                "shared-key"
            ))
            .unwrap()
            .replayed
    );
}

#[test]
fn library_event_failure_rolls_back_status_version_and_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    publish_fixture(&store, &directory, 1, b"immutable");
    let before = store
        .get_artifact("workspace-artifact", "artifact-report")
        .unwrap()
        .unwrap();
    let mut command = library_request(ArtifactLibraryAction::Promote, 1, "rollback");
    command.event.event_id = "event-create".to_owned(); // Existing Workspace event ID forces insert failure.
    assert!(store.change_artifact_library(command).is_err());
    assert_eq!(
        store
            .get_artifact("workspace-artifact", "artifact-report")
            .unwrap()
            .unwrap(),
        before
    );
    let connection = Connection::open(state_database(&directory)).unwrap();
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id = 'rollback'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
    store
        .change_artifact_library(library_request(
            ArtifactLibraryAction::Promote,
            1,
            "rollback",
        ))
        .unwrap();
}

#[test]
fn concurrent_library_promotions_have_one_winner() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    publish_fixture(&store, &directory, 1, b"immutable");
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|index| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.change_artifact_library(library_request(
                    ArtifactLibraryAction::Promote,
                    1,
                    &format!("race-{index}"),
                ))
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(StoreError::Conflict { .. })))
            .count(),
        1
    );
}

#[test]
fn linked_library_status_changes_do_not_fetch_or_delete_external_content() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    let mut connection = Connection::open(state_database(&directory)).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .unwrap();
    let tx = connection.transaction().unwrap();
    let provenance = r#"{"source_inputs":[{"resource_ref":{"workspace_id":"workspace-artifact","resource_id":"external-source","revision_id":"provider-r1"}}],"transformations":[],"tool_reports":[]}"#;
    tx.execute("INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('external-source', 'workspace-artifact', 'CONNECTOR_OBJECT', '{}', 'Provider source', 'provider-r1', 'PERSONAL', ?1, '2026-10-08T00:00:00Z', '2026-10-08T00:00:00Z', 1)", [r#"{"source_inputs":[],"transformations":[],"tool_reports":[]}"#]).unwrap();
    tx.execute("INSERT INTO resource_revisions(resource_revision_id, resource_id, provider_revision, observed_at, created_by_json) VALUES ('provider-r1', 'external-source', 'provider-r1', '2026-10-08T00:00:00Z', '{}')", []).unwrap();
    tx.execute("INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('artifact-resource', 'workspace-artifact', 'ARTIFACT', '{}', 'Linked report', 'linked-revision', 'PERSONAL', ?1, '2026-10-08T00:00:00Z', '2026-10-08T00:00:00Z', 1)", [provenance]).unwrap();
    tx.execute("INSERT INTO resource_revisions(resource_revision_id, resource_id, provider_revision, observed_at, created_by_json) VALUES ('linked-revision', 'artifact-resource', 'provider-r1', '2026-10-08T00:00:00Z', '{}')", []).unwrap();
    tx.execute("INSERT INTO artifacts(artifact_id, workspace_id, resource_id, kind, display_name, current_version, library_status, created_at, version) VALUES ('artifact-report', 'workspace-artifact', 'artifact-resource', 'document', 'Linked report', 1, 'TRANSIENT', '2026-10-08T00:00:00Z', 1)", []).unwrap();
    tx.execute("INSERT INTO artifact_versions(artifact_id, resource_id, version, resource_revision_id, input_refs_json, content_kind, resource_ref_json, provider_revision, content_observed_at, provenance_json, created_at) VALUES ('artifact-report', 'artifact-resource', 1, 'linked-revision', '[' || ?1 || ']', 'EXTERNAL_RESOURCE', ?1, 'provider-r1', '2026-10-08T00:00:00Z', ?2, '2026-10-08T00:00:00Z')",
        params![r#"{"workspace_id":"workspace-artifact","resource_id":"external-source","revision_id":"provider-r1"}"#, provenance]).unwrap();
    tx.commit().unwrap();
    let before = store
        .get_artifact_version("workspace-artifact", "artifact-report", 1)
        .unwrap()
        .unwrap();
    store
        .change_artifact_library(library_request(
            ArtifactLibraryAction::Promote,
            1,
            "linked-promote",
        ))
        .unwrap();
    store
        .change_artifact_library(library_request(
            ArtifactLibraryAction::Archive,
            2,
            "linked-archive",
        ))
        .unwrap();
    assert_eq!(
        store
            .get_artifact_version("workspace-artifact", "artifact-report", 1)
            .unwrap()
            .unwrap(),
        before
    );
    assert!(
        matches!(store.read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 100), Err(StoreError::Invalid(ref code)) if code == "ARTIFACT_EXTERNAL_CONTENT_UNAVAILABLE")
    );
}

#[test]
fn library_replay_rejects_corrupt_receipts_and_archive_blocks_text_staging() {
    use storage_core::ArtifactVersionWriteStore;
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    publish_fixture(&store, &directory, 1, b"immutable");
    store
        .change_artifact_library(library_request(ArtifactLibraryAction::Promote, 1, "save"))
        .unwrap();
    store
        .change_artifact_library(library_request(
            ArtifactLibraryAction::Archive,
            2,
            "archive-terminal",
        ))
        .unwrap();
    assert!(
        matches!(store.stage_text_content("owner-local", "workspace-artifact", "artifact-report", b"new bytes"), Err(StoreError::Invalid(ref code)) if code == "ARTIFACT_ARCHIVED")
    );
    let connection = Connection::open(state_database(&directory)).unwrap();
    connection.execute("UPDATE request_dedup SET response_json = '{}' WHERE principal_id = 'owner-local' AND request_id = 'save'", []).unwrap();
    assert!(matches!(
        store.change_artifact_library(library_request(ArtifactLibraryAction::Promote, 1, "save")),
        Err(StoreError::Integrity(_))
    ));
}

fn publish_fixture(
    store: &SqliteWorkspaceStore,
    directory: &tempfile::TempDir,
    version: u64,
    text: &[u8],
) -> BlobRef {
    publish_fixture_with_inputs(store, directory, version, text, &[])
}

fn publish_fixture_with_inputs(
    store: &SqliteWorkspaceStore,
    directory: &tempfile::TempDir,
    version: u64,
    text: &[u8],
    input_refs: &[serde_json::Value],
) -> BlobRef {
    let blob = store
        .inner
        .blobs
        .put(
            "workspace-artifact",
            BlobPurpose::Artifact,
            text,
            "text/plain",
        )
        .expect("commit fixture blob");
    let mut connection = Connection::open(state_database(directory)).expect("fixture connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable constraints");
    let transaction = connection.transaction().expect("fixture transaction");
    let revision = format!("artifact-revision-{version}");
    let provenance = r#"{"source_inputs":[],"transformations":[],"tool_reports":[]}"#;
    if version == 1 {
        transaction.execute("INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('artifact-resource', 'workspace-artifact', 'ARTIFACT', '{}', 'Report', ?1, 'PERSONAL', ?2, '2026-10-08T00:00:00Z', '2026-10-08T00:00:00Z', 1)", params![revision, provenance]).expect("fixture Resource");
        transaction.execute("INSERT INTO artifacts(artifact_id, workspace_id, resource_id, kind, display_name, current_version, library_status, created_at, version) VALUES ('artifact-report', 'workspace-artifact', 'artifact-resource', 'document', 'Report', 1, 'TRANSIENT', '2026-10-08T00:00:00Z', 1)", []).expect("fixture Artifact");
    }
    transaction.execute("INSERT INTO resource_revisions(resource_revision_id, resource_id, content_digest, size_bytes, media_type, observed_at, created_by_json) VALUES (?1, 'artifact-resource', ?2, ?3, 'text/plain', '2026-10-08T00:00:00Z', '{}')", params![revision, blob.digest, text.len() as i64]).expect("fixture revision");
    if version > 1 {
        transaction.execute("UPDATE resources SET current_revision_id = ?1, version = version + 1 WHERE resource_id = 'artifact-resource'", [&revision]).expect("advance Resource head first");
    }
    transaction.execute("INSERT INTO artifact_versions(artifact_id, resource_id, version, resource_revision_id, content_kind, content_digest, storage_ref_json, content_media_type, content_size_bytes, provenance_json, created_at, input_refs_json) VALUES ('artifact-report', 'artifact-resource', ?1, ?2, 'MANAGED_BLOB', ?3, ?4, 'text/plain', ?5, ?6, '2026-10-08T00:00:00Z', ?7)", params![version as i64, revision, blob.digest, serde_json::to_string(&blob).expect("blob JSON"), text.len() as i64, provenance, serde_json::to_string(input_refs).expect("fixture input refs")]).expect("fixture immutable version");
    if version > 1 {
        transaction.execute("UPDATE artifacts SET current_version = ?1, version = version + 1 WHERE artifact_id = 'artifact-report'", [version as i64]).expect("advance Artifact pointer last");
    }
    transaction
        .commit()
        .expect("fixture commits with full constraints");
    blob
}

#[test]
fn dependency_invalidation_page_is_owner_edge_scoped_and_keyset_stable() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    let database = state_database(&directory);
    let mut connection = Connection::open(&database).expect("fixture SQLite connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable fixture constraints");
    let transaction = connection
        .transaction()
        .expect("source fixture transaction");
    transaction.execute(
        "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('source-resource', 'workspace-artifact', 'CONNECTOR_OBJECT', '{}', 'Source', 'source-r3', 'PERSONAL', '{}', '2026-10-08T00:00:00Z', '2026-10-08T00:00:00Z', 3)",
        [],
    ).expect("source Resource");
    for (revision, timestamp) in [
        ("source-r1", "2026-10-08T00:00:00Z"),
        ("source-r2", "2026-10-08T00:00:01Z"),
        ("source-r3", "2026-10-08T00:00:02Z"),
    ] {
        transaction.execute(
            "INSERT INTO resource_revisions(resource_revision_id, resource_id, provider_revision, observed_at, created_by_json) VALUES (?1, 'source-resource', ?1, ?2, '{}')",
            params![revision, timestamp],
        ).expect("source revision");
    }
    transaction.commit().expect("source fixture is consistent");
    drop(connection);

    let input_refs = [json!({
        "workspace_id": "workspace-artifact",
        "resource_id": "source-resource",
        "revision_id": "source-r1"
    })];
    publish_fixture_with_inputs(&store, &directory, 1, b"immutable artifact", &input_refs);
    let connection = Connection::open(&database).expect("fixture SQLite connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable fixture constraints");
    let transaction = connection
        .unchecked_transaction()
        .expect("empty edge fixture transaction");
    transaction.execute(
        "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('empty-artifact-resource', 'workspace-artifact', 'ARTIFACT', '{}', 'Empty history artifact', 'empty-artifact-r1', 'PERSONAL', '{}', '2026-10-08T00:00:00Z', '2026-10-08T00:00:00Z', 1)",
        [],
    ).expect("empty-history Artifact Resource");
    transaction.execute(
        "INSERT INTO resource_revisions(resource_revision_id, resource_id, observed_at, created_by_json) VALUES ('empty-artifact-r1', 'empty-artifact-resource', '2026-10-08T00:00:00Z', '{}')",
        [],
    ).expect("empty-history Artifact Resource revision");
    transaction.execute(
        "INSERT INTO artifacts(artifact_id, workspace_id, resource_id, kind, display_name, current_version, library_status, created_at) VALUES ('empty-artifact', 'workspace-artifact', 'empty-artifact-resource', 'document', 'Empty history artifact', 1, 'TRANSIENT', '2026-10-08T00:00:00Z')",
        [],
    ).expect("empty-history Artifact");
    transaction.execute(
        "INSERT INTO artifact_versions(artifact_id, resource_id, version, resource_revision_id, content_kind, content_digest, storage_ref_json, content_media_type, content_size_bytes, provenance_json, created_at) VALUES ('empty-artifact', 'empty-artifact-resource', 1, 'empty-artifact-r1', 'MANAGED_BLOB', ?1, '{}', 'text/plain', 0, '{}', '2026-10-08T00:00:00Z')",
        [format!("sha256:{}", "b".repeat(64))],
    ).expect("empty-history Artifact version");
    transaction.execute(
        "INSERT INTO dependency_edges(dependency_edge_id, workspace_id, source_resource_id, source_revision_id, dependent_kind, dependent_ref, artifact_id, artifact_version, created_at) VALUES ('edge-empty-history', 'workspace-artifact', 'source-resource', 'source-r1', 'ARTIFACT_VERSION', 'artifact://workspace-artifact/empty-artifact@v1', 'empty-artifact', 1, '2026-10-08T00:00:00Z')",
        [],
    ).expect("edge with no invalidation rows");
    transaction
        .commit()
        .expect("empty edge fixture is consistent");
    connection.execute(
        "INSERT INTO dependency_edges(dependency_edge_id, workspace_id, source_resource_id, source_revision_id, dependent_kind, dependent_ref, artifact_id, artifact_version, verification_run_id, created_at) VALUES ('edge-source-artifact', 'workspace-artifact', 'source-resource', 'source-r1', 'ARTIFACT_VERSION', 'artifact://workspace-artifact/artifact-report@v1', 'artifact-report', 1, NULL, '2026-10-08T00:00:00Z')",
        [],
    ).expect("revision-pinned dependency edge");
    connection.execute(
        "INSERT INTO invalidation_records(invalidation_record_id, dependency_edge_id, observed_revision_id, reason_code, created_at) VALUES ('invalidation-z', 'edge-source-artifact', 'source-r2', 'SOURCE_RESOURCE_REVISION_ADVANCED', '2026-10-08T00:00:01Z'), ('invalidation-a', 'edge-source-artifact', 'source-r3', 'SOURCE_RESOURCE_REVISION_ADVANCED', '2026-10-08T00:00:01Z')",
        [],
    ).expect("same-time invalidation history");

    let page_one = store
        .list_dependency_invalidations_page(DependencyInvalidationPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-artifact".to_owned(),
            dependency_edge_id: "edge-source-artifact".to_owned(),
            after_created_at: None,
            after_invalidation_record_id: None,
            limit: 1,
        })
        .expect("first page query")
        .expect("owned edge exists");
    assert_eq!(page_one.len(), 1);
    assert_eq!(page_one[0].invalidation_record_id, "invalidation-a");
    let page_two = store
        .list_dependency_invalidations_page(DependencyInvalidationPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-artifact".to_owned(),
            dependency_edge_id: "edge-source-artifact".to_owned(),
            after_created_at: Some(page_one[0].created_at.clone()),
            after_invalidation_record_id: Some(page_one[0].invalidation_record_id.clone()),
            limit: 1,
        })
        .expect("second page query")
        .expect("owned edge exists");
    assert_eq!(page_two.len(), 1);
    assert_eq!(page_two[0].invalidation_record_id, "invalidation-z");
    assert!(
        store
            .list_dependency_invalidations_page(DependencyInvalidationPageRequest {
                owner_principal_id: "owner-local".to_owned(),
                workspace_id: "workspace-artifact".to_owned(),
                dependency_edge_id: "edge-empty-history".to_owned(),
                after_created_at: None,
                after_invalidation_record_id: None,
                limit: 10,
            })
            .expect("empty history query")
            .expect("existing edge with empty history")
            .is_empty()
    );
    assert!(
        store
            .list_dependency_invalidations_page(DependencyInvalidationPageRequest {
                owner_principal_id: "other-owner".to_owned(),
                workspace_id: "workspace-artifact".to_owned(),
                dependency_edge_id: "edge-source-artifact".to_owned(),
                after_created_at: None,
                after_invalidation_record_id: None,
                limit: 10,
            })
            .expect("foreign owner query is nondisclosing")
            .is_none()
    );
}

#[test]
fn resource_dependents_page_derives_freshness_from_revision_heads_and_paginates_stably() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");

    let inputs = [
        json!({"workspace_id":"workspace-artifact", "resource_id":"source-current", "revision_id":"current-r3"}),
        json!({"workspace_id":"workspace-artifact", "resource_id":"source-current", "revision_id":"current-r1"}),
        json!({"workspace_id":"workspace-artifact", "resource_id":"source-branch", "revision_id":"branch-r1"}),
        json!({"workspace_id":"workspace-artifact", "resource_id":"source-unknown", "revision_id":"unknown-r1"}),
    ];
    publish_fixture_with_inputs(&store, &directory, 1, b"dependent artifact", &inputs);
    publish_fixture_with_inputs(&store, &directory, 2, b"dependent artifact v2", &inputs);

    let mut connection = Connection::open(state_database(&directory)).expect("fixture connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable fixture constraints");
    let transaction = connection.transaction().expect("fixture transaction");
    for (resource_id, current_revision_id) in [
        ("source-current", Some("current-r3")),
        ("source-branch", None),
        // One head with a missing canonical pointer is inconsistent, so freshness is UNKNOWN.
        ("source-unknown", None),
        ("source-empty", None),
    ] {
        transaction
            .execute(
                "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES (?1, 'workspace-artifact', 'FILE', '{}', ?1, ?2, 'PERSONAL', '{}', '2026-10-08T00:00:00Z', '2026-10-08T00:00:00Z', 1)",
                rusqlite::params![resource_id, current_revision_id],
            )
            .expect("source Resource");
    }
    for (revision_id, resource_id) in [
        ("current-r1", "source-current"),
        ("current-r2", "source-current"),
        ("current-r3", "source-current"),
        ("branch-r1", "source-branch"),
        ("branch-r2", "source-branch"),
        ("branch-r3", "source-branch"),
        ("unknown-r1", "source-unknown"),
    ] {
        transaction
            .execute(
                "INSERT INTO resource_revisions(resource_revision_id, resource_id, observed_at, created_by_json) VALUES (?1, ?2, '2026-10-08T00:00:00Z', '{}')",
                rusqlite::params![revision_id, resource_id],
            )
            .expect("immutable source revision");
    }
    for (resource_id, child, parent) in [
        ("source-current", "current-r2", "current-r1"),
        ("source-current", "current-r3", "current-r2"),
        ("source-branch", "branch-r2", "branch-r1"),
        ("source-branch", "branch-r3", "branch-r1"),
    ] {
        transaction
            .execute(
                "INSERT INTO resource_revision_parents(resource_id, child_revision_id, parent_revision_id) VALUES (?1, ?2, ?3)",
                rusqlite::params![resource_id, child, parent],
            )
            .expect("revision ancestry");
    }
    transaction.execute(
        "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at) VALUES ('dependents-verifier-agent', 'fixture', 'Fixture Verifier', '2026-10-08T00:00:00Z')",
        [],
    ).expect("verification AgentProfile");
    transaction.execute(
        "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version) VALUES ('dependents-verifier-binding', 'workspace-artifact', 'dependents-verifier-agent', '{}', '{}', 0, 0, '2026-10-08T00:00:00Z', 1)",
        [],
    ).expect("verification AgentBinding");
    transaction.execute(
        "INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status, lead_agent_binding_id, priority, created_by_json, created_at, updated_at, version) VALUES ('dependents-verification-task', 'workspace-artifact', 1, 'READY', 'dependents-verifier-binding', 'NORMAL', '{}', '2026-10-08T00:00:00Z', '2026-10-08T00:00:00Z', 1)",
        [],
    ).expect("verification Task");
    transaction.execute(
        "INSERT INTO task_spec_revisions(task_id, workspace_id, revision, objective, lead_failover_policy_json, placement_preference, authored_by_json, created_at) VALUES ('dependents-verification-task', 'workspace-artifact', 1, 'Verify dependencies', '{\"mode\":\"DISABLED\",\"triggers\":[],\"fallback_agent_binding_ids\":[],\"max_lead_changes\":0}', '{}', '{}', '2026-10-08T00:00:00Z')",
        [],
    ).expect("verification TaskSpecRevision");
    transaction.execute(
        "INSERT INTO verification_runs(verification_run_id, task_id, criterion_id, task_spec_revision, criterion_digest, verifier_kind, verifier_version, subject_refs_json, inputs_json, status) VALUES ('verification-run-current', 'dependents-verification-task', 'criterion-1', 1, ?1, 'fixture', '1', '[]', ?2, 'PASSED')",
        rusqlite::params![format!("sha256:{}", "b".repeat(64)), serde_json::to_string(&[json!({"resource_ref":{"workspace_id":"workspace-artifact","resource_id":"source-current","revision_id":"current-r3"}})]).expect("verification input JSON")],
    ).expect("immutable VerificationRun");
    transaction.execute(
        "INSERT INTO dependency_edges(dependency_edge_id, workspace_id, source_resource_id, source_revision_id, dependent_kind, dependent_ref, verification_run_id, created_at) VALUES ('edge-verification-current', 'workspace-artifact', 'source-current', 'current-r3', 'VERIFICATION_RUN', 'verification://verification-run-current', 'verification-run-current', '2026-10-08T00:00:00Z')",
        [],
    ).expect("VerificationRun dependency edge");
    for (edge_id, source, revision, dependent_version, created_at) in [
        (
            "edge-current",
            "source-current",
            "current-r3",
            1,
            "2026-10-08T00:00:00Z",
        ),
        (
            "edge-stale-with-invalidation",
            "source-current",
            "current-r1",
            1,
            "2026-10-08T00:00:00Z",
        ),
        (
            "edge-stale-without-invalidation",
            "source-current",
            "current-r1",
            2,
            "2026-10-08T00:00:00Z",
        ),
        // This projection points at an existing ArtifactVersion but its immutable input set does
        // not contain this exact source revision. It must not receive a confident freshness state.
        (
            "edge-z-projection-drift",
            "source-current",
            "current-r2",
            1,
            "2026-10-08T00:00:00Z",
        ),
        (
            "edge-conflicted",
            "source-branch",
            "branch-r1",
            1,
            "2026-10-08T00:00:03Z",
        ),
        (
            "edge-unknown",
            "source-unknown",
            "unknown-r1",
            1,
            "2026-10-08T00:00:04Z",
        ),
    ] {
        let dependent_ref =
            format!("artifact://workspace-artifact/artifact-report@v{dependent_version}");
        transaction
            .execute(
                "INSERT INTO dependency_edges(dependency_edge_id, workspace_id, source_resource_id, source_revision_id, dependent_kind, dependent_ref, artifact_id, artifact_version, created_at) VALUES (?1, 'workspace-artifact', ?2, ?3, 'ARTIFACT_VERSION', ?4, 'artifact-report', ?5, ?6)",
                rusqlite::params![edge_id, source, revision, dependent_ref, dependent_version, created_at],
            )
            .expect("immutable dependency edge");
    }
    transaction
        .execute(
            "INSERT INTO invalidation_records(invalidation_record_id, dependency_edge_id, observed_revision_id, reason_code, created_at) VALUES ('invalid-a', 'edge-stale-with-invalidation', 'current-r2', 'SOURCE_RESOURCE_REVISION_ADVANCED', '2026-10-08T00:00:05Z'), ('invalid-z', 'edge-stale-with-invalidation', 'current-r3', 'SOURCE_RESOURCE_REVISION_ADVANCED', '2026-10-08T00:00:05Z')",
            [],
        )
        .expect("immutable invalidation history");
    transaction
        .commit()
        .expect("source graph and dependency fixture commit");

    let first_page = store
        .list_resource_dependents_page(ResourceDependencyPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-artifact".to_owned(),
            resource_id: "source-current".to_owned(),
            after_created_at: None,
            after_dependency_edge_id: None,
            limit: 2,
        })
        .expect("first dependent page")
        .expect("owned source Resource exists");
    assert_eq!(first_page.len(), 2);
    assert_eq!(
        first_page[0].dependency_edge.dependency_edge_id,
        "edge-current"
    );
    assert_eq!(
        first_page[0].dependent_freshness,
        DependencyFreshness::Current
    );
    assert!(first_page[0].latest_invalidation.is_none());
    assert_eq!(
        first_page[1].dependency_edge.dependency_edge_id,
        "edge-stale-with-invalidation"
    );
    assert_eq!(
        first_page[1].dependent_freshness,
        DependencyFreshness::Stale
    );
    assert_eq!(
        first_page[1]
            .latest_invalidation
            .as_ref()
            .unwrap()
            .invalidation_record_id,
        "invalid-z"
    );

    let second_page = store
        .list_resource_dependents_page(ResourceDependencyPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-artifact".to_owned(),
            resource_id: "source-current".to_owned(),
            after_created_at: Some(first_page[1].dependency_edge.created_at.clone()),
            after_dependency_edge_id: Some(
                first_page[1].dependency_edge.dependency_edge_id.clone(),
            ),
            limit: 2,
        })
        .expect("second dependent page")
        .expect("owned source Resource exists");
    assert_eq!(second_page.len(), 2);
    assert_eq!(
        second_page[0].dependency_edge.dependency_edge_id,
        "edge-stale-without-invalidation"
    );
    assert_eq!(
        second_page[0].dependent_freshness,
        DependencyFreshness::Stale
    );
    assert!(second_page[0].latest_invalidation.is_none());
    assert_eq!(
        second_page[1].dependency_edge.dependency_edge_id,
        "edge-verification-current"
    );
    assert_eq!(
        second_page[1].dependent_freshness,
        DependencyFreshness::Current
    );

    let third_page = store
        .list_resource_dependents_page(ResourceDependencyPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-artifact".to_owned(),
            resource_id: "source-current".to_owned(),
            after_created_at: Some(second_page[1].dependency_edge.created_at.clone()),
            after_dependency_edge_id: Some(
                second_page[1].dependency_edge.dependency_edge_id.clone(),
            ),
            limit: 2,
        })
        .expect("third dependent page")
        .expect("owned source Resource exists");
    assert_eq!(third_page.len(), 1);
    assert_eq!(
        third_page[0].dependency_edge.dependency_edge_id,
        "edge-z-projection-drift"
    );
    assert_eq!(
        third_page[0].dependent_freshness,
        DependencyFreshness::Unknown
    );

    let conflicted = store
        .list_resource_dependents_page(ResourceDependencyPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-artifact".to_owned(),
            resource_id: "source-branch".to_owned(),
            after_created_at: None,
            after_dependency_edge_id: None,
            limit: 10,
        })
        .expect("conflicted source page")
        .expect("owned source Resource exists");
    assert_eq!(
        conflicted[0].dependent_freshness,
        DependencyFreshness::Conflicted
    );

    let unknown = store
        .list_resource_dependents_page(ResourceDependencyPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-artifact".to_owned(),
            resource_id: "source-unknown".to_owned(),
            after_created_at: None,
            after_dependency_edge_id: None,
            limit: 10,
        })
        .expect("unknown source page")
        .expect("owned source Resource exists");
    assert_eq!(unknown[0].dependent_freshness, DependencyFreshness::Unknown);

    let empty = store
        .list_resource_dependents_page(ResourceDependencyPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-artifact".to_owned(),
            resource_id: "source-empty".to_owned(),
            after_created_at: None,
            after_dependency_edge_id: None,
            limit: 10,
        })
        .expect("empty dependent page")
        .expect("owned source Resource exists with no dependencies");
    assert!(empty.is_empty());

    assert!(
        store
            .list_resource_dependents_page(ResourceDependencyPageRequest {
                owner_principal_id: "other-owner".to_owned(),
                workspace_id: "workspace-artifact".to_owned(),
                resource_id: "source-current".to_owned(),
                after_created_at: None,
                after_dependency_edge_id: None,
                limit: 10,
            })
            .expect("foreign owner result is nondisclosing")
            .is_none()
    );
}

#[test]
fn exact_artifact_revision_reads_survive_new_head_and_restart() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    publish_fixture(&store, &directory, 1, b"first immutable output");
    publish_fixture(&store, &directory, 2, b"second immutable output");
    assert_eq!(
        store
            .get_artifact("workspace-artifact", "artifact-report")
            .unwrap()
            .unwrap()
            .current_version,
        2
    );
    let old = store
        .get_artifact_version("workspace-artifact", "artifact-report", 1)
        .unwrap()
        .unwrap();
    assert_eq!(old.resource_revision_id, "artifact-revision-1");
    assert!(matches!(
        old.content,
        ArtifactContentRecord::ManagedBlob { .. }
    ));
    assert_eq!(
        store
            .read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 100)
            .unwrap()
            .unwrap(),
        b"first immutable output"
    );
    drop(store);
    let reopened = test_store(&directory, Duration::from_secs(1));
    assert_eq!(
        reopened
            .read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 100)
            .unwrap()
            .unwrap(),
        b"first immutable output"
    );
}

#[test]
fn artifact_reads_enforce_workspace_bounds_and_content_limits() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    publish_fixture(&store, &directory, 1, b"bounded output");
    assert!(
        store
            .get_artifact("other-workspace", "artifact-report")
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .get_artifact_version("other-workspace", "artifact-report", 1)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .read_artifact_content_bounded("other-workspace", "artifact-report", 1, 100)
            .unwrap()
            .is_none()
    );
    assert!(
        matches!(store.read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 1), Err(StoreError::Invalid(ref code)) if code == "ARTIFACT_READ_LIMIT_EXCEEDED")
    );
    assert!(
        store
            .list_artifacts_page("workspace-artifact", Some("SAVED"), None, None, None, 50)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .list_artifacts_page(
                "workspace-artifact",
                Some("TRANSIENT"),
                None,
                None,
                None,
                50
            )
            .unwrap()
            .len(),
        1
    );
    assert!(
        store
            .get_artifact_version("workspace-artifact", "artifact-report", 3)
            .unwrap()
            .is_none()
    );
}

#[test]
fn missing_or_mismatched_artifact_blob_never_returns_other_bytes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    let blob = publish_fixture(&store, &directory, 1, b"content to remove");
    store
        .inner
        .blobs
        .remove("workspace-artifact", BlobPurpose::Artifact, &blob)
        .expect("remove test blob");
    assert!(matches!(
        store.read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 100),
        Err(StoreError::NotFound)
    ));
    assert!(
        store
            .get_artifact_version("workspace-artifact", "artifact-report", 1)
            .unwrap()
            .is_some()
    );
}

#[test]
fn task_presentation_read_returns_task_and_resolvable_artifact_head_together() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");

    let mut connection = Connection::open(state_database(&directory)).expect("fixture connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable constraints");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction.execute(
        "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at)
         VALUES ('presentation-agent', 'fixture', 'Fixture Agent', '2026-10-08T00:00:00Z')",
        [],
    ).expect("fixture AgentProfile");
    transaction.execute(
        "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json,
                                   configuration_json, enabled, lead_eligible, created_at, version)
         VALUES ('presentation-binding', 'workspace-artifact', 'presentation-agent', '{}', '{}', 0, 0,
                 '2026-10-08T00:00:00Z', 1)",
        [],
    ).expect("fixture AgentBinding");
    transaction.execute(
        "INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status, lead_agent_binding_id,
                           priority, created_by_json, created_at, updated_at, version)
         VALUES ('presentation-task', 'workspace-artifact', 1, 'READY', 'presentation-binding',
                 'NORMAL', '{}', '2026-10-08T00:00:00Z', '2026-10-08T00:00:00Z', 1)",
        [],
    ).expect("fixture Task");
    transaction.execute(
        r#"INSERT INTO task_spec_revisions(task_id, workspace_id, revision, objective, lead_failover_policy_json,
                                         placement_preference, authored_by_json, created_at)
         VALUES ('presentation-task', 'workspace-artifact', 1, 'Inspect a report', '{"mode":"DISABLED","triggers":[],"fallback_agent_binding_ids":[],"max_lead_changes":0}', '{}', '{}',
                 '2026-10-08T00:00:00Z')"#,
        [],
    ).expect("fixture TaskSpecRevision");
    transaction.commit().expect("fixture Task commits");

    publish_fixture(&store, &directory, 1, b"current output");
    let connection = Connection::open(state_database(&directory)).expect("fixture connection");
    connection.execute(
        "UPDATE artifacts SET task_id = 'presentation-task', version = version + 1 WHERE artifact_id = 'artifact-report'",
        [],
    ).expect("attach Artifact to Task");

    let snapshot = store
        .get_task_presentation("workspace-artifact", "presentation-task", "runtime-local")
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.origin_sequence, 1);
    assert_eq!(
        snapshot.task.current_spec_revision.objective,
        "Inspect a report"
    );
    assert!(snapshot.steps.is_empty());
    assert!(!snapshot.steps_overflow);
    assert!(!snapshot.artifacts_overflow);
    assert_eq!(snapshot.artifacts.len(), 1);
    assert_eq!(
        snapshot.artifacts[0].artifact.artifact_id,
        "artifact-report"
    );
    assert_eq!(snapshot.artifacts[0].version.version, 1);
    assert_eq!(
        snapshot.artifacts[0].version.resource_revision_id,
        "artifact-revision-1"
    );
}
