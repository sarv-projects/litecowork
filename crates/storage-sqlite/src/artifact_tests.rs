//! Read-store checks seed committed SQL records; this is not a publication service.
use super::*;
use storage_core::{ArtifactContentRecord, ArtifactReadStore, TaskPresentationReadStore};

fn publish_fixture(store: &SqliteWorkspaceStore, directory: &tempfile::TempDir, version: u64, text: &[u8]) -> BlobRef {
    let blob = store.inner.blobs.put("workspace-artifact", BlobPurpose::Artifact, text, "text/plain").expect("commit fixture blob");
    let mut connection = Connection::open(state_database(directory)).expect("fixture connection");
    connection.execute_batch("PRAGMA foreign_keys = ON;").expect("enable constraints");
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
    transaction.commit().expect("fixture commits with full constraints");
    blob
}

#[test]
fn exact_artifact_revision_reads_survive_new_head_and_restart() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    publish_fixture(&store, &directory, 1, b"first immutable output");
    publish_fixture(&store, &directory, 2, b"second immutable output");
    assert_eq!(store.get_artifact("workspace-artifact", "artifact-report").unwrap().unwrap().current_version, 2);
    let old = store.get_artifact_version("workspace-artifact", "artifact-report", 1).unwrap().unwrap();
    assert_eq!(old.resource_revision_id, "artifact-revision-1");
    assert!(matches!(old.content, ArtifactContentRecord::ManagedBlob { .. }));
    assert_eq!(store.read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 100).unwrap().unwrap(), b"first immutable output");
    drop(store);
    let reopened = test_store(&directory, Duration::from_secs(1));
    assert_eq!(reopened.read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 100).unwrap().unwrap(), b"first immutable output");
}

#[test]
fn artifact_reads_enforce_workspace_bounds_and_content_limits() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    publish_fixture(&store, &directory, 1, b"bounded output");
    assert!(store.get_artifact("other-workspace", "artifact-report").unwrap().is_none());
    assert!(store.get_artifact_version("other-workspace", "artifact-report", 1).unwrap().is_none());
    assert!(store.read_artifact_content_bounded("other-workspace", "artifact-report", 1, 100).unwrap().is_none());
    assert!(matches!(store.read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 1), Err(StoreError::Invalid(ref code)) if code == "ARTIFACT_READ_LIMIT_EXCEEDED"));
    assert!(store.list_artifacts_page("workspace-artifact", Some("SAVED"), None, None, None, 50).unwrap().is_empty());
    assert_eq!(store.list_artifacts_page("workspace-artifact", Some("TRANSIENT"), None, None, None, 50).unwrap().len(), 1);
    assert!(store.get_artifact_version("workspace-artifact", "artifact-report", 3).unwrap().is_none());
}

#[test]
fn missing_or_mismatched_artifact_blob_never_returns_other_bytes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");
    let blob = publish_fixture(&store, &directory, 1, b"content to remove");
    store.inner.blobs.remove("workspace-artifact", BlobPurpose::Artifact, &blob).expect("remove test blob");
    assert!(matches!(store.read_artifact_content_bounded("workspace-artifact", "artifact-report", 1, 100), Err(StoreError::NotFound)));
    assert!(store.get_artifact_version("workspace-artifact", "artifact-report", 1).unwrap().is_some());
}

#[test]
fn task_presentation_read_returns_task_and_resolvable_artifact_head_together() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-artifact");

    let mut connection = Connection::open(state_database(&directory)).expect("fixture connection");
    connection.execute_batch("PRAGMA foreign_keys = ON;").expect("enable constraints");
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
        "INSERT INTO task_spec_revisions(task_id, workspace_id, revision, objective, lead_failover_policy_json,
                                         placement_preference, authored_by_json, created_at)
         VALUES ('presentation-task', 'workspace-artifact', 1, 'Inspect a report', '{}', '{}', '{}',
                 '2026-10-08T00:00:00Z')",
        [],
    ).expect("fixture TaskSpecRevision");
    transaction.commit().expect("fixture Task commits");

    publish_fixture(&store, &directory, 1, b"current output");
    let mut connection = Connection::open(state_database(&directory)).expect("fixture connection");
    connection.execute(
        "UPDATE artifacts SET task_id = 'presentation-task', version = version + 1 WHERE artifact_id = 'artifact-report'",
        [],
    ).expect("attach Artifact to Task");

    let snapshot = store.get_task_presentation("workspace-artifact", "presentation-task").unwrap().unwrap();
    assert_eq!(snapshot.task.current_spec_revision.objective, "Inspect a report");
    assert!(snapshot.steps.is_empty());
    assert!(!snapshot.steps_overflow);
    assert!(!snapshot.artifacts_overflow);
    assert_eq!(snapshot.artifacts.len(), 1);
    assert_eq!(snapshot.artifacts[0].artifact.artifact_id, "artifact-report");
    assert_eq!(snapshot.artifacts[0].version.version, 1);
    assert_eq!(snapshot.artifacts[0].version.resource_revision_id, "artifact-revision-1");
}
