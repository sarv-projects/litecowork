use super::{
    RuntimeLocalState, RuntimeState, StartupPolicy, fail_registered_startup, read_state,
    resume_workspace_root_live, revalidate_workspace_roots,
};
use crate::local_filesystem::{RuntimeIdentityKey, open_selected_directory};
use domain_workspace::{
    AddWorkspaceRoot, ChangeWorkspaceRootStatus, CreateWorkspace, EventContext,
    WorkspaceRootService, WorkspaceService,
};
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Arc, time::Duration};
use storage_core::{
    BlobPurpose, DeviceIdentityRecord, RuntimeIncarnationLocalObservationRecord,
    RuntimeIncarnationRecord, RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
    RuntimeWorkspaceBindingStore, StoreError, WorkspaceCreateRequest, WorkspaceRootStatusAction,
    WorkspaceRootStore,
};
use storage_sqlite::{
    FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider,
};
use zeroize::Zeroizing;

const WORKSPACE: &str = "workspace-root-revalidation-test";
const OWNER: &str = "owner-root-revalidation-test";
const RUNTIME: &str = "runtime-root-revalidation-test";
const FIRST_INCARNATION: &str = "incarnation-root-revalidation-old";
const NEXT_INCARNATION: &str = "incarnation-root-revalidation-new";
const RECOVERY_INCARNATION: &str = "incarnation-root-revalidation-recovery";
const NOW: &str = "2026-10-12T10:00:00Z";

struct TestKeys;

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([41_u8; 32]),
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

struct TestRuntimeIdentity;

impl RuntimeIdentityKey for TestRuntimeIdentity {
    fn keyed_pseudonym(&self, namespace: &str, material: &[u8]) -> String {
        let mut digest = Sha256::new();
        digest.update(b"litecowork-test-runtime-pseudonym-v1\0");
        digest.update(namespace.as_bytes());
        digest.update([0]);
        digest.update(material);
        hex::encode(digest.finalize())
    }
}

fn private_tempdir() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("create test directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict test directory permissions");
    directory
}

fn event(event_id: &str, runtime_id: &str) -> EventContext {
    EventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: runtime_id.to_owned(),
        hlc_timestamp: NOW.to_owned(),
        correlation_id: format!("correlation-{event_id}"),
        causation_id: None,
        recorded_at: NOW.to_owned(),
    }
}

fn store_fixture() -> (tempfile::TempDir, SqliteWorkspaceStore) {
    let directory = private_tempdir();
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Revalidation fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: event("workspace-created", RUNTIME),
        })
        .expect("Workspace creates");

    let public_key = [19_u8; 32];
    let device_id = format!("ed25519-sha256:{}", hex::encode(Sha256::digest(public_key)));
    store
        .register_local_incarnation(
            RuntimeRecord {
                runtime_id: RUNTIME.to_owned(),
                device_identity: DeviceIdentityRecord {
                    device_id,
                    public_key: format!("ed25519:{}", hex::encode(public_key)),
                    key_version: 1,
                    issued_at: NOW.to_owned(),
                    display_name: None,
                },
                runtime_version: "0.1.0-test".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                roles: vec!["OPERATOR_ENDPOINT".to_owned()],
                trust_zone: "PERSONAL_DEVICE".to_owned(),
                availability: "RECOVERING".to_owned(),
                startup_policy: "MANUAL".to_owned(),
                current_incarnation_id: FIRST_INCARNATION.to_owned(),
                resource_capacity: serde_json::json!({"sampled_at": NOW}),
                last_seen: NOW.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: FIRST_INCARNATION.to_owned(),
                runtime_id: RUNTIME.to_owned(),
                process_started_at: NOW.to_owned(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: FIRST_INCARNATION.to_owned(),
                os_boot_id: Some("test-boot-old".to_owned()),
                observed_at: NOW.to_owned(),
            },
        )
        .expect("old Runtime incarnation registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: FIRST_INCARNATION.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("Runtime enters its serving state");
    store
        .enroll_local_runtime(storage_core::LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "runtime-enrollment".to_owned(),
                request_payload: serde_json::json!({"workspace_id": WORKSPACE}),
            },
            expected_workspace_version: 1,
            runtime_workspace_binding_id: "runtime-workspace-binding".to_owned(),
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: FIRST_INCARNATION.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            now: NOW.to_owned(),
            correlation_id: "runtime-enrollment-correlation".to_owned(),
        })
        .expect("Runtime enrolls in Workspace");
    (directory, store)
}

fn reopen_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
    SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("reopen real SQLite store")
}

fn register_incarnation(store: &SqliteWorkspaceStore, incarnation_id: &str, started_at: &str) {
    let public_key = [19_u8; 32];
    store
        .register_local_incarnation(
            RuntimeRecord {
                runtime_id: RUNTIME.to_owned(),
                device_identity: DeviceIdentityRecord {
                    device_id: format!(
                        "ed25519-sha256:{}",
                        hex::encode(Sha256::digest(public_key))
                    ),
                    public_key: format!("ed25519:{}", hex::encode(public_key)),
                    key_version: 1,
                    issued_at: NOW.to_owned(),
                    display_name: None,
                },
                runtime_version: "0.1.0-test".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                roles: vec!["OPERATOR_ENDPOINT".to_owned()],
                trust_zone: "PERSONAL_DEVICE".to_owned(),
                availability: "RECOVERING".to_owned(),
                startup_policy: "MANUAL".to_owned(),
                current_incarnation_id: incarnation_id.to_owned(),
                resource_capacity: serde_json::json!({"sampled_at": NOW}),
                last_seen: started_at.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                runtime_id: RUNTIME.to_owned(),
                process_started_at: started_at.to_owned(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: true,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                os_boot_id: Some(format!("test-boot-{incarnation_id}")),
                observed_at: started_at.to_owned(),
            },
        )
        .expect("new Runtime incarnation registers");
}

fn mark_incarnation_degraded(store: &SqliteWorkspaceStore, incarnation_id: &str) {
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("Runtime incarnation reaches its resumable serving state");
}

fn add_root(store: &SqliteWorkspaceStore, suffix: &str, path: &Path) -> String {
    let opened = open_selected_directory(path).expect("selected directory opens");
    let identity = opened.identity();
    let (identity_digest, file_identity) = identity.keyed_projection(&TestRuntimeIdentity);
    let location_id = format!("location-{suffix}");
    let root_id = format!("root-{suffix}");
    WorkspaceRootService::new(store.clone())
        .add_root(AddWorkspaceRoot {
            workspace_id: WORKSPACE.to_owned(),
            expected_workspace_version: 1,
            principal_id: OWNER.to_owned(),
            request_id: format!("root-add-{suffix}"),
            resource_id: format!("resource-{suffix}"),
            identity_digest,
            file_identity,
            location_id: location_id.clone(),
            locator_ref_id: format!("locator-{suffix}"),
            workspace_root_id: root_id.clone(),
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: FIRST_INCARNATION.to_owned(),
            private_locator: opened.private_locator().to_owned(),
            file_identity_binding: identity.binding_record(
                location_id,
                RUNTIME.to_owned(),
                FIRST_INCARNATION.to_owned(),
                NOW.to_owned(),
            ),
            display_name: format!("folder-{suffix}"),
            watch_policy: "METADATA".to_owned(),
            replication_policy: "NONE".to_owned(),
            event: event(&format!("root-event-{suffix}"), RUNTIME),
        })
        .expect("WorkspaceRoot creates");
    root_id
}

fn pause_root(store: &SqliteWorkspaceStore, root_id: &str, event_id: &str) {
    WorkspaceRootService::new(store.clone())
        .change_root_status(ChangeWorkspaceRootStatus {
            workspace_id: WORKSPACE.to_owned(),
            workspace_root_id: root_id.to_owned(),
            expected_version: 1,
            action: WorkspaceRootStatusAction::Pause,
            principal_id: OWNER.to_owned(),
            request_id: format!("pause-{root_id}"),
            runtime_id: None,
            runtime_incarnation_id: None,
            event: event(event_id, RUNTIME),
        })
        .expect("WorkspaceRoot pauses");
}

fn resume_root(
    store: &SqliteWorkspaceStore,
    runtime_incarnation_id: &str,
    root_id: &str,
    request_id: &str,
    expected_version: u64,
    event_id: &str,
) -> Result<storage_core::CommittedWorkspaceRootStatus, super::WorkspaceRootResumeError> {
    resume_workspace_root_live(
        store,
        RUNTIME,
        runtime_incarnation_id,
        &TestRuntimeIdentity,
        WORKSPACE,
        root_id,
        OWNER,
        request_id,
        expected_version,
        event(event_id, RUNTIME),
    )
}

#[test]
fn workspace_root_revalidation_recovers_only_matching_identity_and_keeps_paused_intent() {
    let (_directory, store) = store_fixture();
    let folders = private_tempdir();
    let matching_path = folders.path().join("matching");
    let replacement_path = folders.path().join("replacement");
    let paused_path = folders.path().join("paused");
    fs::create_dir(&matching_path).expect("matching root directory");
    fs::create_dir(&replacement_path).expect("replaceable root directory");
    fs::create_dir(&paused_path).expect("paused root directory");
    let matching_id = add_root(&store, "matching", &matching_path);
    let replacement_id = add_root(&store, "replacement", &replacement_path);
    let paused_id = add_root(&store, "paused", &paused_path);
    WorkspaceRootService::new(store.clone())
        .change_root_status(ChangeWorkspaceRootStatus {
            workspace_id: WORKSPACE.to_owned(),
            workspace_root_id: paused_id.clone(),
            expected_version: 1,
            action: WorkspaceRootStatusAction::Pause,
            principal_id: OWNER.to_owned(),
            request_id: "pause-before-restart".to_owned(),
            runtime_id: None,
            runtime_incarnation_id: None,
            event: event("root-paused", RUNTIME),
        })
        .expect("root pauses");

    let moved_path = folders.path().join("replacement-moved");
    fs::rename(&replacement_path, &moved_path).expect("move original directory");
    fs::create_dir(&replacement_path).expect("replace root at saved path");
    register_incarnation(&store, NEXT_INCARNATION, NOW);

    revalidate_workspace_roots(&store, RUNTIME, NEXT_INCARNATION, &TestRuntimeIdentity)
        .expect("root-local failures do not block startup revalidation");

    let roots = store
        .list_workspace_roots(WORKSPACE, None, None, None, 20)
        .expect("list revalidated roots");
    let by_id = |id: &str| {
        roots
            .iter()
            .find(|entry| entry.root.workspace_root_id == id)
            .expect("root remains listed")
    };
    assert_eq!(by_id(&matching_id).root.status, "ACTIVE");
    assert_eq!(by_id(&matching_id).root.version, 1);
    assert_eq!(by_id(&matching_id).location_availability, "AVAILABLE");
    assert_eq!(by_id(&replacement_id).root.status, "UNAVAILABLE");
    assert_eq!(by_id(&replacement_id).root.version, 2);
    assert_eq!(by_id(&replacement_id).location_availability, "UNAVAILABLE");
    assert_eq!(by_id(&paused_id).root.status, "PAUSED");
    assert_eq!(by_id(&paused_id).root.version, 2);
    assert_eq!(by_id(&paused_id).location_availability, "AVAILABLE");

    let connection = rusqlite::Connection::open(_directory.path().join("state.sqlite3"))
        .expect("inspect real SQLite revalidation state");
    let current_bindings: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_location_bindings WHERE runtime_incarnation_id = ?1",
            [NEXT_INCARNATION],
            |row| row.get(0),
        )
        .expect("count new locator bindings");
    let current_identities: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM file_identity_bindings WHERE runtime_incarnation_id = ?1",
            [NEXT_INCARNATION],
            |row| row.get(0),
        )
        .expect("count new identity bindings");
    assert_eq!(current_bindings, 2);
    assert_eq!(current_identities, 2);

    let replacement_lost = folders.path().join("replacement-lost");
    fs::rename(&replacement_path, &replacement_lost).expect("move rejected replacement aside");
    fs::rename(&moved_path, &replacement_path)
        .expect("restore the originally authorized directory");
    register_incarnation(&store, RECOVERY_INCARNATION, "2026-10-12T11:00:00Z");
    revalidate_workspace_roots(&store, RUNTIME, RECOVERY_INCARNATION, &TestRuntimeIdentity)
        .expect("later exact identity match recovers an unavailable root");
    let recovered = store
        .list_workspace_roots(WORKSPACE, None, None, None, 20)
        .expect("list roots after identity recovery");
    let recovered_replacement = recovered
        .iter()
        .find(|entry| entry.root.workspace_root_id == replacement_id)
        .expect("replacement root remains listed");
    assert_eq!(recovered_replacement.root.status, "ACTIVE");
    assert_eq!(recovered_replacement.root.version, 3);
    assert_eq!(recovered_replacement.location_availability, "AVAILABLE");
    let recovered_paused = recovered
        .iter()
        .find(|entry| entry.root.workspace_root_id == paused_id)
        .expect("paused root remains listed");
    assert_eq!(recovered_paused.root.status, "PAUSED");
    assert_eq!(recovered_paused.root.version, 2);
    assert_eq!(recovered_paused.location_availability, "AVAILABLE");
    let recovery_bindings: i64 = rusqlite::Connection::open(_directory.path().join("state.sqlite3"))
        .expect("reopen SQLite for recovery binding checks")
        .query_row(
            "SELECT (SELECT COUNT(*) FROM resource_location_bindings WHERE runtime_incarnation_id = ?1)
                  + (SELECT COUNT(*) FROM file_identity_bindings WHERE runtime_incarnation_id = ?1)",
            [RECOVERY_INCARNATION],
            |row| row.get(0),
        )
        .expect("count recovery-incarnation bindings");
    assert_eq!(recovery_bindings, 6);
    let recovery_events: String = rusqlite::Connection::open(_directory.path().join("state.sqlite3"))
        .expect("reopen SQLite for recovery event checks")
        .query_row(
            "SELECT group_concat(payload_json, '') FROM domain_events WHERE entity_id = ?1 AND type = 'workspace.root.status.changed.v1'",
            [&replacement_id],
            |row| row.get(0),
        )
        .expect("read replacement root status events");
    assert!(recovery_events.contains("IDENTITY_CHANGED"));
    assert!(recovery_events.contains("ROOT_IDENTITY_REVALIDATED"));

    let private_locator = open_selected_directory(&matching_path)
        .expect("matching path opens")
        .private_locator()
        .to_owned();
    let stored_public_data: String = connection
        .query_row(
            "SELECT COALESCE(group_concat(payload_json, ''), '') FROM domain_events WHERE entity_id IN (?1, ?2, ?3)",
            rusqlite::params![matching_id, replacement_id, paused_id],
            |row| row.get(0),
        )
        .expect("inspect root events");
    assert!(!stored_public_data.contains(&private_locator));
    assert!(!stored_public_data.contains(&folders.path().to_string_lossy().to_string()));
}

#[test]
fn workspace_root_revalidation_sql_failure_rolls_back_and_degrades_startup() {
    let (directory, store) = store_fixture();
    let folders = private_tempdir();
    let selected = folders.path().join("selected");
    fs::create_dir(&selected).expect("selected root directory");
    let root_id = add_root(&store, "rollback", &selected);
    register_incarnation(&store, NEXT_INCARNATION, NOW);

    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open fixture SQLite database");
    connection
        .execute_batch(
            "CREATE TRIGGER fail_workspace_root_revalidation_receipt
             BEFORE INSERT ON request_dedup
             WHEN NEW.request_id LIKE 'rootrv_%'
             BEGIN SELECT RAISE(ABORT, 'injected revalidation receipt failure'); END;",
        )
        .expect("install transactional failure injection");
    let events_before: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id IN (?1, ?2)",
            rusqlite::params![root_id, "resource-rollback"],
            |row| row.get(0),
        )
        .expect("count root and Resource events before revalidation");
    let receipts_before: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'rootrv_%'",
            [],
            |row| row.get(0),
        )
        .expect("count revalidation receipts before failure");
    drop(connection);

    let error = revalidate_workspace_roots(&store, RUNTIME, NEXT_INCARNATION, &TestRuntimeIdentity)
        .expect_err("injected durable receipt failure blocks startup recovery");
    assert_eq!(
        error,
        "WorkspaceRoot revalidation could not be committed atomically"
    );

    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("reopen fixture SQLite database");
    let root: (String, i64) = connection
        .query_row(
            "SELECT status, version FROM workspace_roots WHERE workspace_root_id = ?1",
            [&root_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read root after failed transaction");
    let location: String = connection
        .query_row(
            "SELECT availability FROM resource_locations WHERE location_id = 'location-rollback'",
            [],
            |row| row.get(0),
        )
        .expect("read location after failed transaction");
    let current_bindings: i64 = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM resource_location_bindings WHERE runtime_incarnation_id = ?1)
                  + (SELECT COUNT(*) FROM file_identity_bindings WHERE runtime_incarnation_id = ?1)",
            [NEXT_INCARNATION],
            |row| row.get(0),
        )
        .expect("count current-incarnation bindings after rollback");
    let events_after: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id IN (?1, ?2)",
            rusqlite::params![root_id, "resource-rollback"],
            |row| row.get(0),
        )
        .expect("count root and Resource events after rollback");
    let receipts_after: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'rootrv_%'",
            [],
            |row| row.get(0),
        )
        .expect("count revalidation receipts after rollback");
    assert_eq!(root, ("ACTIVE".to_owned(), 1));
    assert_eq!(location, "AVAILABLE");
    assert_eq!(current_bindings, 0);
    assert_eq!(events_after, events_before);
    assert_eq!(receipts_after, receipts_before);

    let mut state = RuntimeLocalState {
        schema_version: 1,
        runtime_id: RUNTIME.to_owned(),
        local_principal_id: Some("principal-root-revalidation-test".to_owned()),
        os_principal_binding_established: true,
        local_incarnation_id: NEXT_INCARNATION.to_owned(),
        startup_policy: StartupPolicy::Manual,
        recovery_state: RuntimeState::Recovering,
        recovered_from_unclean_shutdown: true,
        last_shutdown_clean: false,
        blockers: Vec::new(),
        version: 1,
    };
    let mut incarnation_version = 1;
    let failed_start = fail_registered_startup(
        directory.path(),
        &store,
        &mut state,
        &mut incarnation_version,
        "WORKSPACE_ROOT_REVALIDATION_COMMIT_FAILED",
        &error,
    );
    assert_eq!(failed_start, Err(error));
    assert_eq!(incarnation_version, 2);
    let persisted = read_state(&directory.path().join("runtime-state.json"))
        .expect("read private local Runtime state")
        .expect("private Runtime state persists");
    assert_eq!(persisted.recovery_state, RuntimeState::Degraded);
    assert_eq!(
        persisted.blockers,
        ["WORKSPACE_ROOT_REVALIDATION_COMMIT_FAILED"]
    );
}

#[test]
fn workspace_root_resume_reopens_identity_commits_and_replays_after_store_restart() {
    let (directory, store) = store_fixture();
    let folders = private_tempdir();
    let selected = folders.path().join("resume");
    fs::create_dir(&selected).expect("selected root directory");
    let root_id = add_root(&store, "resume", &selected);
    pause_root(&store, &root_id, "root-resume-paused");
    register_incarnation(&store, NEXT_INCARNATION, "2026-10-12T10:30:00Z");
    revalidate_workspace_roots(&store, RUNTIME, NEXT_INCARNATION, &TestRuntimeIdentity)
        .expect("startup revalidates paused root identity");
    mark_incarnation_degraded(&store, NEXT_INCARNATION);

    let committed = resume_root(
        &store,
        NEXT_INCARNATION,
        &root_id,
        "resume-after-restart",
        2,
        "root-resumed",
    )
    .expect("matching saved directory resumes");
    assert_eq!(committed.root.status, "ACTIVE");
    assert_eq!(committed.root.version, 3);
    assert_eq!(
        committed.location_availability.as_deref(),
        Some("AVAILABLE")
    );
    assert_eq!(committed.event.event_id, "root-resumed");

    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect committed resume");
    let resume_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id = ?1 AND type = 'workspace.root.status.changed.v1' AND json_extract(payload_json, '$.reason_code') = 'USER_RESUMED'",
            [&root_id],
            |row| row.get(0),
        )
        .expect("count committed resume events");
    let current_bindings: i64 = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM resource_location_bindings WHERE location_id = 'location-resume' AND runtime_incarnation_id = ?1)
                  + (SELECT COUNT(*) FROM file_identity_bindings WHERE location_id = 'location-resume' AND runtime_incarnation_id = ?1)",
            [NEXT_INCARNATION],
            |row| row.get(0),
        )
        .expect("count current-incarnation private bindings");
    let binding_observed_at: (String, String) = connection
        .query_row(
            "SELECT lb.observed_at, fib.observed_at
             FROM resource_location_bindings lb
             JOIN file_identity_bindings fib USING(location_id, runtime_id, runtime_incarnation_id)
             WHERE lb.location_id = 'location-resume' AND lb.runtime_incarnation_id = ?1",
            [NEXT_INCARNATION],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read refreshed current-incarnation bindings");
    assert_eq!(resume_events, 1);
    assert_eq!(current_bindings, 2);
    assert_eq!(binding_observed_at.0, committed.root.updated_at);
    assert_eq!(binding_observed_at.1, committed.root.updated_at);
    drop(connection);
    drop(store);

    let restarted_store = reopen_store(&directory);
    let replay = resume_root(
        &restarted_store,
        NEXT_INCARNATION,
        &root_id,
        "resume-after-restart",
        2,
        "must-not-be-written",
    )
    .expect("exact receipt replays after store restart");
    assert_eq!(replay, committed);
    assert_eq!(replay.root.status, "ACTIVE");
    assert_eq!(replay.root.version, 3);
    assert_eq!(replay.event.event_id, "root-resumed");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect replayed resume");
    let resume_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id = ?1 AND type = 'workspace.root.status.changed.v1' AND json_extract(payload_json, '$.reason_code') = 'USER_RESUMED'",
            [&root_id],
            |row| row.get(0),
        )
        .expect("count resume events after replay");
    assert_eq!(resume_events, 1);
}

#[test]
fn workspace_root_resume_rejects_replaced_directory_and_preserves_paused_intent() {
    let (_directory, store) = store_fixture();
    let folders = private_tempdir();
    let selected = folders.path().join("replace-on-resume");
    let moved = folders.path().join("original-moved");
    fs::create_dir(&selected).expect("selected root directory");
    let root_id = add_root(&store, "replace-on-resume", &selected);
    pause_root(&store, &root_id, "root-replacement-paused");
    register_incarnation(&store, NEXT_INCARNATION, "2026-10-12T10:30:00Z");
    revalidate_workspace_roots(&store, RUNTIME, NEXT_INCARNATION, &TestRuntimeIdentity)
        .expect("startup revalidates paused root before replacement");
    mark_incarnation_degraded(&store, NEXT_INCARNATION);
    let bindings_before_replacement: i64 = rusqlite::Connection::open(
        _directory.path().join("state.sqlite3"),
    )
    .expect("inspect startup-revalidated bindings")
    .query_row(
        "SELECT (SELECT COUNT(*) FROM resource_location_bindings WHERE location_id = 'location-replace-on-resume' AND runtime_incarnation_id = ?1)
              + (SELECT COUNT(*) FROM file_identity_bindings WHERE location_id = 'location-replace-on-resume' AND runtime_incarnation_id = ?1)",
        [NEXT_INCARNATION],
        |row| row.get(0),
    )
    .expect("count startup-revalidated bindings");
    assert_eq!(bindings_before_replacement, 2);
    fs::rename(&selected, &moved).expect("move authorized directory");
    fs::create_dir(&selected).expect("replace directory at same path");

    assert!(matches!(
        resume_root(
            &store,
            NEXT_INCARNATION,
            &root_id,
            "resume-replaced-directory",
            2,
            "must-not-resume"
        ),
        Err(super::WorkspaceRootResumeError::Unavailable)
    ));
    let root = store
        .get_workspace_root(WORKSPACE, &root_id)
        .expect("read root after failed identity check")
        .expect("root remains present");
    assert_eq!(root.status, "PAUSED");
    assert_eq!(root.version, 2);
    let listed = store
        .list_workspace_roots(WORKSPACE, None, None, None, 10)
        .expect("read availability after failed identity check");
    let root_projection = listed
        .iter()
        .find(|entry| entry.root.workspace_root_id == root_id)
        .expect("root remains listed");
    assert_eq!(root_projection.location_availability, "UNAVAILABLE");
    let connection = rusqlite::Connection::open(_directory.path().join("state.sqlite3"))
        .expect("inspect rejected resume events");
    let resumed_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id = ?1 AND type = 'workspace.root.status.changed.v1' AND json_extract(payload_json, '$.reason_code') = 'USER_RESUMED'",
            [&root_id],
            |row| row.get(0),
        )
        .expect("count rejected resume transitions");
    let next_incarnation_bindings: i64 = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM resource_location_bindings WHERE location_id = 'location-replace-on-resume' AND runtime_incarnation_id = ?1)
                  + (SELECT COUNT(*) FROM file_identity_bindings WHERE location_id = 'location-replace-on-resume' AND runtime_incarnation_id = ?1)",
            [NEXT_INCARNATION],
            |row| row.get(0),
        )
        .expect("count unexpected incarnation bindings");
    assert_eq!(resumed_events, 0);
    assert_eq!(next_incarnation_bindings, 0);

    fs::remove_dir(&selected).expect("remove untrusted replacement");
    fs::rename(&moved, &selected).expect("restore original authorized directory");
    assert!(matches!(
        resume_root(
            &store,
            NEXT_INCARNATION,
            &root_id,
            "resume-unavailable-location",
            2,
            "must-wait-for-revalidation",
        ),
        Err(super::WorkspaceRootResumeError::Unavailable)
    ));
    register_incarnation(&store, RECOVERY_INCARNATION, "2026-10-12T11:00:00Z");
    revalidate_workspace_roots(&store, RUNTIME, RECOVERY_INCARNATION, &TestRuntimeIdentity)
        .expect("Runtime revalidation restores the exact original identity");
    mark_incarnation_degraded(&store, RECOVERY_INCARNATION);
    let resumed_after_recovery = resume_root(
        &store,
        RECOVERY_INCARNATION,
        &root_id,
        "resume-after-revalidation",
        2,
        "root-resumed-after-recovery",
    )
    .expect("owner can resume after Runtime revalidation");
    assert_eq!(resumed_after_recovery.root.status, "ACTIVE");
    assert_eq!(resumed_after_recovery.root.version, 3);
}

#[test]
fn workspace_root_resume_stale_version_and_receipt_failure_leave_no_partial_transition() {
    let (directory, store) = store_fixture();
    let folders = private_tempdir();
    let selected = folders.path().join("resume-rollback");
    fs::create_dir(&selected).expect("selected root directory");
    let root_id = add_root(&store, "resume-rollback", &selected);
    pause_root(&store, &root_id, "root-rollback-paused");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open real SQLite database");
    let events_before: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id IN (?1, 'resource-resume-rollback')",
            [&root_id],
            |row| row.get(0),
        )
        .expect("count events before resume attempts");
    let projection_observations_before: (String, String, String) = connection
        .query_row(
            "SELECT l.observed_at, lb.observed_at, fib.observed_at
             FROM resource_locations l
             JOIN resource_location_bindings lb USING(location_id)
             JOIN file_identity_bindings fib USING(location_id, runtime_id, runtime_incarnation_id)
             WHERE l.location_id = 'location-resume-rollback' AND lb.runtime_incarnation_id = ?1",
            [FIRST_INCARNATION],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read location and private binding observations before resume");
    drop(connection);

    assert!(matches!(
        resume_root(
            &store,
            FIRST_INCARNATION,
            &root_id,
            "resume-stale-version",
            1,
            "stale-resume-event"
        ),
        Err(super::WorkspaceRootResumeError::Stale)
    ));
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open SQLite for receipt failure injection");
    let stale_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id = 'resume-stale-version'",
            [],
            |row| row.get(0),
        )
        .expect("count stale-version receipts");
    assert_eq!(stale_receipts, 0);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_workspace_root_resume_receipt
             BEFORE INSERT ON request_dedup
             WHEN NEW.request_id = 'resume-receipt-failure'
             BEGIN SELECT RAISE(ABORT, 'injected resume receipt failure'); END;",
        )
        .expect("install resume receipt failure injection");
    drop(connection);

    assert!(matches!(
        resume_root(
            &store,
            FIRST_INCARNATION,
            &root_id,
            "resume-receipt-failure",
            2,
            "resume-transaction-must-rollback",
        ),
        Err(super::WorkspaceRootResumeError::Internal)
    ));
    let root = store
        .get_workspace_root(WORKSPACE, &root_id)
        .expect("read root after rollback")
        .expect("root remains present");
    assert_eq!(root.status, "PAUSED");
    assert_eq!(root.version, 2);
    let listed = store
        .list_workspace_roots(WORKSPACE, None, None, None, 10)
        .expect("read location after rollback");
    let root_projection = listed
        .iter()
        .find(|entry| entry.root.workspace_root_id == root_id)
        .expect("root remains listed");
    assert_eq!(root_projection.location_availability, "AVAILABLE");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect rolled back resume transaction");
    let events_after: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id IN (?1, 'resource-resume-rollback')",
            [&root_id],
            |row| row.get(0),
        )
        .expect("count events after rollback");
    let receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id = 'resume-receipt-failure'",
            [],
            |row| row.get(0),
        )
        .expect("count failed resume receipt");
    let projection_observations_after: (String, String, String) = connection
        .query_row(
            "SELECT l.observed_at, lb.observed_at, fib.observed_at
             FROM resource_locations l
             JOIN resource_location_bindings lb USING(location_id)
             JOIN file_identity_bindings fib USING(location_id, runtime_id, runtime_incarnation_id)
             WHERE l.location_id = 'location-resume-rollback' AND lb.runtime_incarnation_id = ?1",
            [FIRST_INCARNATION],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read location and private binding observations after rollback");
    assert_eq!(events_after, events_before);
    assert_eq!(receipts, 0);
    assert_eq!(
        projection_observations_after,
        projection_observations_before
    );
}
