//! Read-store checks seed committed SQL records; this is not a publication service.
use super::*;
use storage_core::{ArtifactContentRecord, ArtifactReadStore, TaskPresentationReadStore};
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
    transaction.execute("INSERT INTO artifact_versions(artifact_id, resource_id, version, resource_revision_id, content_kind, content_digest, storage_ref_json, content_media_type, content_size_bytes, provenance_json, created_at) VALUES ('artifact-report', 'artifact-resource', ?1, ?2, 'MANAGED_BLOB', ?3, ?4, 'text/plain', ?5, ?6, '2026-10-08T00:00:00Z')", params![version as i64, revision, blob.digest, serde_json::to_string(&blob).expect("blob JSON"), text.len() as i64, provenance]).expect("fixture immutable version");
    if version > 1 {
        transaction.execute("UPDATE artifacts SET current_version = ?1, version = version + 1 WHERE artifact_id = 'artifact-report'", [version as i64]).expect("advance Artifact pointer last");
    }
    transaction
        .commit()
        .expect("fixture commits with full constraints");
    blob
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
    let mut connection = Connection::open(state_database(&directory)).expect("fixture connection");
    connection.execute(
        "UPDATE artifacts SET task_id = 'presentation-task', version = version + 1 WHERE artifact_id = 'artifact-report'",
        [],
    ).expect("attach Artifact to Task");

    let snapshot = store
        .get_task_presentation("workspace-artifact", "presentation-task")
        .unwrap()
        .unwrap();
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
