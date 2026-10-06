use super::*;
use domain_workspace::{ChangeReplicationPolicy, CreateWorkspace, EventContext, WorkspaceService};
use rusqlite::Connection;
use std::sync::Barrier;
use storage_core::{
    AggregateStateRef, BlobPurpose, CommittedWorkspace, EventDraft, ReplicationPolicy, StoreError,
    Workspace,
};
use zeroize::Zeroizing;

#[derive(Clone)]
struct TestKeys {
    key: [u8; 32],
}

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new(self.key),
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

fn test_store(directory: &tempfile::TempDir, busy_timeout: Duration) -> SqliteWorkspaceStore {
    let blob_store = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));
    SqliteWorkspaceStore::open(
        state_database(directory),
        blob_store,
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout,
        },
    )
    .expect("SQLite store opens")
}

fn state_database(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory.path().join("state").join("state.sqlite3")
}

fn service(store: SqliteWorkspaceStore) -> WorkspaceService<SqliteWorkspaceStore> {
    WorkspaceService::new(store)
}

fn context(event_id: &str, minute: u8) -> EventContext {
    let timestamp = format!("2026-10-06T10:{minute:02}:00Z");
    EventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        hlc_timestamp: timestamp.clone(),
        correlation_id: "correlation-1".to_owned(),
        causation_id: None,
        recorded_at: timestamp,
    }
}

fn create_workspace(store: &SqliteWorkspaceStore, workspace_id: &str) -> CommittedWorkspace {
    service(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Desktop workspace".to_owned(),
            owner_principal_id: "owner-local".to_owned(),
            event: context("event-create", 0),
        })
        .expect("Workspace creation commits")
}

#[test]
fn applies_full_contract_schema_and_reports_sqlite_runtime() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let version = store.sqlite_version().expect("SQLite version");
    let mut components = version.split('.').map(|part| part.parse::<u32>().unwrap());
    let actual = (
        components.next().expect("major version"),
        components.next().expect("minor version"),
        components.next().expect("patch version"),
    );
    assert!(
        actual >= (3, 38, 0),
        "SQLite {version} is below the contract minimum"
    );
    println!("SQLite {version}");

    let connection =
        Connection::open(state_database(&directory)).expect("inspect migrated database");
    let user_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read user_version");
    assert_eq!(user_version, SCHEMA_VERSION);
    let migration: (String, String, String) = connection
        .query_row(
            "SELECT name, source_checksum, schema_fingerprint FROM schema_migrations WHERE version = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("migration receipt");
    assert_eq!(migration.0, MIGRATION_NAME);
    assert!(migration.1.starts_with("sha256:") && migration.1.len() == 71);
    assert!(migration.2.starts_with("sha256:") && migration.2.len() == 71);
    for table in [
        "workspaces",
        "domain_events",
        "schema_migrations",
        "workspace_origin_sequences",
    ] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .expect("look up schema table");
        assert!(exists, "missing contract table {table}");
    }
    assert!(store.get_workspace("missing").expect("read").is_none());
}

#[cfg(unix)]
#[test]
fn sqlite_state_directory_and_database_file_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let state_dir = directory.path().join("state");
    let database = state_dir.join("state.sqlite3");

    assert_eq!(
        fs::metadata(state_dir)
            .expect("state directory metadata")
            .permissions()
            .mode()
            & 0o077,
        0
    );
    assert_eq!(
        fs::metadata(database)
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    drop(store);
}

#[cfg(unix)]
#[test]
fn sqlite_store_rejects_symlink_database_path() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("temporary directory");
    let state_dir = directory.path().join("state");
    ensure_private_directory(&state_dir).expect("private state directory");
    let target = state_dir.join("outside.sqlite3");
    fs::write(&target, b"do not open").expect("create target");
    let linked_database = state_dir.join("linked.sqlite3");
    symlink(&target, &linked_database).expect("make database symlink");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));

    let result = SqliteWorkspaceStore::open(linked_database, blobs, SqliteConfig::default());
    assert!(matches!(result, Err(StoreError::Io(_))));
    assert_eq!(
        fs::read(target).expect("target is unchanged"),
        b"do not open"
    );
}

#[cfg(unix)]
#[test]
fn sqlite_store_rejects_broad_state_directory_without_creating_database() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    let state_dir = directory.path().join("state");
    fs::create_dir(&state_dir).expect("create state directory");
    fs::set_permissions(&state_dir, fs::Permissions::from_mode(0o755))
        .expect("make state directory broad");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));

    let result = SqliteWorkspaceStore::open(
        state_dir.join("state.sqlite3"),
        blobs,
        SqliteConfig::default(),
    );
    assert!(matches!(result, Err(StoreError::Io(_))));
    assert!(!state_dir.join("state.sqlite3").exists());
}

#[test]
fn canonical_json_uses_jcs_object_and_number_encoding() {
    let value: serde_json::Value = serde_json::from_str(
        r#"{"numbers":[333333333.33333329,1E30,4.50,2e-3,0.000000000000000000000000001],"z":1,"a":2}"#,
    )
    .expect("valid JSON fixture");

    assert_eq!(
        canonical_json(&value).expect("JCS canonical bytes"),
        br#"{"a":2,"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"z":1}"#,
    );
    assert!(matches!(
        canonical_json(&serde_json::json!({"sequence": 9_007_199_254_740_992_u64})),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn workspace_create_update_restart_and_projection_replay_match() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-persist");
    service(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: "workspace-persist".to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: Vec::new(),
            event: context("event-policy", 1),
        })
        .expect("policy update commits");
    drop(store);

    let reopened = test_store(&directory, Duration::from_secs(1));
    let stored = reopened
        .get_workspace("workspace-persist")
        .expect("read Workspace")
        .expect("Workspace exists");
    let replayed = reopened
        .rebuild_workspace_projection("workspace-persist")
        .expect("rebuild projection")
        .expect("projection exists");
    assert_eq!(replayed, stored);
    assert_eq!(replayed.replication_policy, ReplicationPolicy::MetadataOnly);
    assert_eq!(replayed.version, 2);

    let events = reopened
        .read_workspace_events("workspace-persist")
        .expect("read event stream");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].origin_sequence, 1);
    assert_eq!(events[1].origin_sequence, 2);
    assert_eq!(events[0].entity_revision, 1);
    assert_eq!(events[1].entity_revision, 2);
    println!(
        "storage replay: sqlite={}, workspace={}, version={}, policy={}, events={}, replay_matches=true",
        reopened.sqlite_version().expect("SQLite version"),
        replayed.workspace_id,
        replayed.version,
        replayed.replication_policy.as_str(),
        events.len(),
    );
}

#[test]
fn concurrent_expected_version_writes_have_one_winner() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-race");
    let barrier = Arc::new(Barrier::new(3));

    let make_candidate = |policy: ReplicationPolicy, event_id: &'static str| {
        let mut workspace = store
            .get_workspace("workspace-race")
            .expect("read Workspace")
            .expect("Workspace exists");
        workspace.replication_policy = policy.clone();
        workspace.version = 2;
        workspace.updated_at = "2026-10-06T10:01:00Z".to_owned();
        let event = EventDraft {
            event_id: event_id.to_owned(),
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 2,
            hlc_timestamp: "2026-10-06T10:01:00Z".to_owned(),
            correlation_id: "race".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "workspace.replication_policy.changed.v1".to_owned(),
            payload: json!({
                "workspace_id": workspace.workspace_id,
                "from": "LOCAL_ONLY",
                "to": policy.as_str(),
                "replication_scope_root_ids": [],
                "aggregate_version": 2,
            }),
            recorded_at: "2026-10-06T10:01:00Z".to_owned(),
        };
        (workspace, event)
    };
    let left = make_candidate(ReplicationPolicy::MetadataOnly, "event-left");
    let right = make_candidate(ReplicationPolicy::FullWorkspace, "event-right");

    let spawn =
        |store: SqliteWorkspaceStore, barrier: Arc<Barrier>, candidate: (Workspace, EventDraft)| {
            std::thread::spawn(move || {
                barrier.wait();
                store.commit_workspace(Some(1), candidate.0, candidate.1)
            })
        };
    let left_thread = spawn(store.clone(), barrier.clone(), left);
    let right_thread = spawn(store.clone(), barrier.clone(), right);
    barrier.wait();

    let results = [
        left_thread.join().expect("left writer finished"),
        right_thread.join().expect("right writer finished"),
    ];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(StoreError::Conflict { .. })))
            .count(),
        1
    );
    assert_eq!(
        store
            .read_workspace_events("workspace-race")
            .expect("events")
            .len(),
        2
    );
    assert_eq!(
        store
            .get_workspace("workspace-race")
            .expect("Workspace")
            .expect("exists")
            .version,
        2
    );
}

#[test]
fn injected_failures_roll_back_aggregate_event_and_sequence_together() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let database_path = directory.path().join("fault").join("fault.sqlite3");
    ensure_private_directory(database_path.parent().expect("database parent"))
        .expect("private database parent");
    let mut connection = Connection::open(&database_path).expect("open test database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable FK");
    connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("enable WAL");
    migrate(&mut connection).expect("initial migration");

    for failpoint in [
        Failpoint::AfterAggregate,
        Failpoint::AfterSequence,
        Failpoint::AfterEvent,
        Failpoint::BeforeCommit,
    ] {
        let workspace = Workspace {
            workspace_id: "workspace-rollback".to_owned(),
            name: "Rollback".to_owned(),
            owner_principal_id: "owner".to_owned(),
            replication_policy: ReplicationPolicy::LocalOnly,
            replication_scope_root_ids: Vec::new(),
            current_instruction_revision: None,
            default_agent_binding_id: None,
            primary_coworker_id: None,
            hub_runtime_id: None,
            status: "ACTIVE".to_owned(),
            created_at: "2026-10-06T10:00:00Z".to_owned(),
            updated_at: "2026-10-06T10:00:00Z".to_owned(),
            version: 1,
        };
        let event = EventDraft {
            event_id: "event-rollback".to_owned(),
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: "runtime-1".to_owned(),
            entity_revision: 1,
            hlc_timestamp: workspace.created_at.clone(),
            correlation_id: "correlation".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "workspace.created.v1".to_owned(),
            payload: json!({
                "workspace_id": workspace.workspace_id,
                "owner_principal_id": workspace.owner_principal_id,
                "replication_policy": "LOCAL_ONLY",
                "replication_scope_root_ids": [],
            }),
            recorded_at: workspace.created_at.clone(),
        };
        let state_ref = AggregateStateRef {
            blob: storage_core::BlobRef {
                digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
                size_bytes: 1,
                media_type: "application/json".to_owned(),
            },
            entity_revision: 1,
            record_schema_version: 1,
        };

        let result = commit_workspace_transaction(
            &mut connection,
            None,
            workspace,
            event,
            state_ref,
            Some(failpoint),
        );
        assert!(result.is_err());
        for table in ["workspaces", "domain_events", "workspace_origin_sequences"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count rows");
            assert_eq!(count, 0, "{table} changed at {failpoint:?}");
        }
    }
}

#[test]
fn migration_crash_rolls_back_ddl_and_can_retry() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut connection =
        Connection::open(directory.path().join("migration.sqlite3")).expect("open test database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable FK");
    connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("enable WAL");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringMigration)).is_err());
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 0);
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .expect("count schema objects");
    assert_eq!(count, 0);

    migrate(&mut connection).expect("retry full migration");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn schema_drift_and_unknown_newer_versions_fail_closed() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = state_database(&directory);
    let store = test_store(&directory, Duration::from_secs(1));
    drop(store);
    let connection = Connection::open(&path).expect("open database");
    connection
        .execute_batch("DROP TRIGGER workspace_default_agent_binding_insert_guard")
        .expect("remove trigger to simulate corruption");
    drop(connection);
    let blob_store = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));
    let result = SqliteWorkspaceStore::open(&path, blob_store, SqliteConfig::default());
    assert!(matches!(result, Err(StoreError::CorruptSchema(_))));

    let newer_path = directory.path().join("state").join("newer.sqlite3");
    let connection = Connection::open(&newer_path).expect("open future database");
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .expect("set newer schema marker");
    drop(connection);
    let blob_store = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [19_u8; 32] },
    ));
    let result = SqliteWorkspaceStore::open(&newer_path, blob_store, SqliteConfig::default());
    assert!(matches!(result, Err(StoreError::UnsupportedSchema(2))));
}

#[test]
fn external_writer_lock_returns_busy_without_partial_state() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_millis(30));
    create_workspace(&store, "workspace-busy");
    let lock = Connection::open(state_database(&directory)).expect("open lock connection");
    lock.execute_batch("BEGIN IMMEDIATE")
        .expect("hold writer lock");

    let result = service(store.clone()).change_replication_policy(ChangeReplicationPolicy {
        workspace_id: "workspace-busy".to_owned(),
        expected_version: 1,
        policy: ReplicationPolicy::MetadataOnly,
        replication_scope_root_ids: Vec::new(),
        event: context("event-busy", 1),
    });
    assert!(matches!(result, Err(StoreError::Busy)));
    lock.execute_batch("ROLLBACK").expect("release writer lock");

    let current = store
        .get_workspace("workspace-busy")
        .expect("read state")
        .expect("Workspace exists");
    assert_eq!(current.version, 1);
    assert_eq!(
        store
            .read_workspace_events("workspace-busy")
            .expect("read events")
            .len(),
        1
    );
}

#[test]
fn writer_metrics_record_send_wait_and_bounded_in_flight_pressure() {
    const WRITERS: usize = 24;

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_millis(120));
    create_workspace(&store, "workspace-pressure");
    let lock = Connection::open(state_database(&directory)).expect("open lock connection");
    lock.execute_batch("BEGIN IMMEDIATE")
        .expect("hold writer lock");

    let barrier = Arc::new(Barrier::new(WRITERS + 1));
    let writers: Vec<_> = (0..WRITERS)
        .map(|index| {
            let store = store.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                service(store).change_replication_policy(ChangeReplicationPolicy {
                    workspace_id: "workspace-pressure".to_owned(),
                    expected_version: 1,
                    policy: ReplicationPolicy::MetadataOnly,
                    replication_scope_root_ids: Vec::new(),
                    event: context(&format!("event-pressure-{index}"), 1),
                })
            })
        })
        .collect();
    barrier.wait();

    for writer in writers {
        assert!(matches!(
            writer.join().expect("writer thread joins"),
            Err(StoreError::Busy)
        ));
    }

    lock.execute_batch("ROLLBACK").expect("release writer lock");
    let metrics = store.writer_metrics_snapshot();
    assert_eq!(metrics.outstanding_commands, 0);
    assert!(metrics.outstanding_commands_peak > 8);
    assert!(metrics.send_wait_nanos_total > 10_000_000);
    assert!(metrics.send_wait_nanos_max > 10_000_000);
    assert_eq!(
        store
            .get_workspace("workspace-pressure")
            .expect("read state")
            .expect("Workspace exists")
            .version,
        1
    );
}

#[test]
fn workspace_policy_event_payload_is_closed_and_versioned() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-schema");
    service(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: "workspace-schema".to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: Vec::new(),
            event: context("event-schema", 1),
        })
        .expect("update policy");
    let events = store
        .read_workspace_events("workspace-schema")
        .expect("read events");
    assert_eq!(events[0].event_type, "workspace.created.v1");
    assert_eq!(
        events[1].event_type,
        "workspace.replication_policy.changed.v1"
    );
    assert_eq!(events[1].payload.as_object().expect("object").len(), 5);
    assert_eq!(events[1].payload["aggregate_version"], 2);
    assert_eq!(events[1].payload["from"], "LOCAL_ONLY");
    assert_eq!(events[1].payload["to"], "METADATA_ONLY");
}

#[test]
fn reports_blob_commit_failure_before_any_aggregate_write() {
    struct NoSpaceBlobStore;
    impl BlobStore for NoSpaceBlobStore {
        fn put(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            _bytes: &[u8],
            _media_type: &str,
        ) -> Result<storage_core::BlobRef, StoreError> {
            Err(StoreError::Io("simulated no space".to_owned()))
        }
        fn get(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            _blob: &storage_core::BlobRef,
        ) -> Result<Vec<u8>, StoreError> {
            unreachable!("put fails before get")
        }
    }

    let directory = tempfile::tempdir().expect("temporary directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("no-space").join("state.sqlite3"),
        Arc::new(NoSpaceBlobStore),
        SqliteConfig::default(),
    )
    .expect("database opens");
    let result = service(store.clone()).create(CreateWorkspace {
        workspace_id: "workspace-diskfull".to_owned(),
        name: "No space".to_owned(),
        owner_principal_id: "owner".to_owned(),
        event: context("event-no-space", 0),
    });
    assert!(matches!(result, Err(StoreError::Io(_))));
    assert!(
        store
            .get_workspace("workspace-diskfull")
            .expect("read database")
            .is_none()
    );
}
