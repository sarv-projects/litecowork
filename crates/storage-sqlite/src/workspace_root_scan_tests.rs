use super::*;
use domain_workspace::{
    AddWorkspaceRoot, CreateWorkspace, EventContext, WorkspaceRootService, WorkspaceService,
};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use storage_core::{
    DeviceIdentityRecord, LocalFileIdentityBindingRecord, RuntimeIncarnationLocalObservationRecord,
    RuntimeIncarnationRecord, RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
    RuntimeWorkspaceBindingStore, WorkspaceCreateRequest, WorkspaceRootScanCommit,
    WorkspaceRootScanResult, WorkspaceRootScanStatus, WorkspaceRootStore,
};
use zeroize::Zeroizing;

const OWNER: &str = "scan-owner";
const WORKSPACE: &str = "workspace-scan";
const ROOT: &str = "root-scan";
const RUNTIME: &str = "runtime-scan";
const INCARNATION: &str = "incarnation-scan";
const NOW: &str = "2026-10-16T10:00:00Z";

struct ScanTestKeys;

impl WorkspaceBlobKeyProvider for ScanTestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([31_u8; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob("unknown scan test key version".to_owned()));
        }
        self.current_key(workspace_id, purpose)
    }
}

struct ScanFixture {
    directory: tempfile::TempDir,
    store: SqliteWorkspaceStore,
}

fn fixture() -> ScanFixture {
    let directory = tempfile::tempdir().expect("temporary scan store");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state").join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            ScanTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(1),
        },
    )
    .expect("SQLite store opens");

    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Scan test workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "workspace-created-scan-test".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-scan-test".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace is created");

    let public_key = [23_u8; 32];
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
                runtime_version: "0.1.0-scan-test".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                roles: vec!["OPERATOR_ENDPOINT".to_owned()],
                trust_zone: "PERSONAL_DEVICE".to_owned(),
                availability: "RECOVERING".to_owned(),
                startup_policy: "MANUAL".to_owned(),
                current_incarnation_id: INCARNATION.to_owned(),
                resource_capacity: json!({"sampled_at": NOW}),
                last_seen: NOW.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: INCARNATION.to_owned(),
                runtime_id: RUNTIME.to_owned(),
                process_started_at: NOW.to_owned(),
                litecowork_version: "0.1.0-scan-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: INCARNATION.to_owned(),
                os_boot_id: Some("scan-test-boot".to_owned()),
                observed_at: NOW.to_owned(),
            },
        )
        .expect("local Runtime registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: INCARNATION.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("Runtime becomes available for local work");
    store
        .enroll_local_runtime(storage_core::LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "runtime-enrollment-scan-test".to_owned(),
                request_payload: json!({"workspace_id": WORKSPACE}),
            },
            expected_workspace_version: 1,
            runtime_workspace_binding_id: "runtime-binding-scan-test".to_owned(),
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: INCARNATION.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            now: NOW.to_owned(),
            correlation_id: "correlation-scan-enrollment".to_owned(),
        })
        .expect("Runtime enrolls in Workspace");

    let marker = "scan-test-root-identity";
    let identity_digest = format!("sha256:{}", hex::encode(Sha256::digest(marker.as_bytes())));
    let file_identity_digest = hex::encode(Sha256::digest(marker.as_bytes()));
    let path = URL_SAFE_NO_PAD.encode(b"/tmp/litecowork-scan-test");
    WorkspaceRootService::new(store.clone())
        .add_root(AddWorkspaceRoot {
            workspace_id: WORKSPACE.to_owned(),
            expected_workspace_version: 1,
            principal_id: OWNER.to_owned(),
            request_id: "root-create-scan-test".to_owned(),
            resource_id: "resource-scan-test".to_owned(),
            identity_digest: identity_digest.clone(),
            file_identity: json!({
                "filesystem_instance_id": format!("sha256:{file_identity_digest}"),
                "volume_id": null,
                "file_id": format!("sha256:{file_identity_digest}"),
                "generation": null,
                "platform_kind": "LINUX_DEVICE_INODE",
            }),
            location_id: "location-scan-test".to_owned(),
            locator_ref_id: "locator-ref-scan-test".to_owned(),
            workspace_root_id: ROOT.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: INCARNATION.to_owned(),
            private_locator: format!("unix-path-b64:{path}"),
            file_identity_binding: LocalFileIdentityBindingRecord {
                location_id: "location-scan-test".to_owned(),
                runtime_id: RUNTIME.to_owned(),
                runtime_incarnation_id: INCARNATION.to_owned(),
                raw_filesystem_instance_id: "scan-test-device".to_owned(),
                raw_volume_id: None,
                raw_file_id: "scan-test-inode".to_owned(),
                raw_generation: None,
                platform_kind: "LINUX_DEVICE_INODE".to_owned(),
                observed_at: NOW.to_owned(),
            },
            display_name: "Scan test root".to_owned(),
            watch_policy: "METADATA".to_owned(),
            replication_policy: "NONE".to_owned(),
            event: EventContext {
                event_id: "root-created-scan-test".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-root-scan-test".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("WorkspaceRoot is created");

    ScanFixture { directory, store }
}

fn scan_request(request_id: &str) -> WorkspaceCreateRequest {
    WorkspaceCreateRequest {
        principal_id: OWNER.to_owned(),
        request_id: request_id.to_owned(),
        request_payload: json!({
            "operation": "workspace.root.scan.v1",
            "workspace_id": WORKSPACE,
            "workspace_root_id": ROOT,
            "root_version": 1,
        }),
    }
}

fn scan_commit(request_id: &str, runtime_id: &str) -> WorkspaceRootScanCommit {
    WorkspaceRootScanCommit {
        request: scan_request(request_id),
        workspace_id: WORKSPACE.to_owned(),
        workspace_root_id: ROOT.to_owned(),
        expected_root_version: 1,
        runtime_id: runtime_id.to_owned(),
        runtime_incarnation_id: INCARNATION.to_owned(),
        result: WorkspaceRootScanResult {
            workspace_id: WORKSPACE.to_owned(),
            workspace_root_id: ROOT.to_owned(),
            request_id: request_id.to_owned(),
            root_version: 1,
            status: WorkspaceRootScanStatus::Complete,
            entries_examined: 3,
            files_observed: 2,
            partial_reason: None,
            completed_at: NOW.to_owned(),
        },
    }
}

fn database_path(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory.path().join("state").join("state.sqlite3")
}

#[test]
fn workspace_root_scan_candidate_requires_owner_active_root_current_runtime_and_exact_bindings() {
    let fixture = fixture();
    let candidate = fixture
        .store
        .get_workspace_root_scan_candidate(OWNER, WORKSPACE, ROOT, RUNTIME, INCARNATION)
        .expect("authorized candidate lookup")
        .expect("current active root is eligible");
    assert_eq!(candidate.root.version, 1);
    assert_eq!(candidate.root.status, "ACTIVE");
    assert_eq!(candidate.location.availability, "AVAILABLE");
    match candidate.bindings {
        WorkspaceRootRevalidationBindings::CurrentIncarnation {
            locator,
            file_identity,
        } => {
            assert_eq!(locator.runtime_incarnation_id, INCARNATION);
            assert_eq!(file_identity.runtime_incarnation_id, INCARNATION);
            assert!(locator.private_locator.starts_with("unix-path-b64:"));
        }
        _ => panic!("scan requires exact current-incarnation local bindings"),
    }

    assert!(
        fixture
            .store
            .get_workspace_root_scan_candidate(
                "foreign-owner",
                WORKSPACE,
                ROOT,
                RUNTIME,
                INCARNATION
            )
            .expect("foreign lookup is a non-disclosing miss")
            .is_none()
    );
    assert!(
        fixture
            .store
            .get_workspace_root_scan_candidate(
                OWNER,
                "foreign-workspace",
                ROOT,
                RUNTIME,
                INCARNATION
            )
            .expect("foreign Workspace lookup is a non-disclosing miss")
            .is_none()
    );

    let connection = Connection::open(database_path(&fixture.directory)).expect("inspect SQLite");
    connection
        .execute(
            "UPDATE workspace_roots SET status='PAUSED' WHERE workspace_root_id=?1",
            [ROOT],
        )
        .expect("pause root for admission test");
    assert!(
        fixture
            .store
            .get_workspace_root_scan_candidate(OWNER, WORKSPACE, ROOT, RUNTIME, INCARNATION)
            .expect("paused root lookup")
            .is_none()
    );
    connection
        .execute(
            "UPDATE workspace_roots SET status='ACTIVE' WHERE workspace_root_id=?1",
            [ROOT],
        )
        .expect("reactivate root for next admission test");
    connection
        .execute(
            "DELETE FROM runtime_workspace_bindings WHERE runtime_id=?1 AND workspace_id=?2",
            params![RUNTIME, WORKSPACE],
        )
        .expect("remove Runtime authorization binding");
    assert!(
        fixture
            .store
            .get_workspace_root_scan_candidate(OWNER, WORKSPACE, ROOT, RUNTIME, INCARNATION)
            .expect("unbound Runtime lookup")
            .is_none()
    );
}

#[test]
fn workspace_root_scan_commit_persists_safe_receipt_without_events_or_resource_changes() {
    let fixture = fixture();
    let connection = Connection::open(database_path(&fixture.directory)).expect("inspect SQLite");
    let before = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM domain_events), (SELECT version FROM resources WHERE resource_id='resource-scan-test')",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .expect("capture pre-scan state");
    drop(connection);

    let expected = scan_commit("scan-receipt", RUNTIME).result;
    assert_eq!(
        fixture
            .store
            .commit_workspace_root_scan(scan_commit("scan-receipt", RUNTIME))
            .expect("scan receipt commits"),
        expected,
    );
    assert_eq!(
        fixture
            .store
            .get_workspace_root_scan_receipt(&scan_request("scan-receipt"))
            .expect("receipt lookup succeeds"),
        Some(expected.clone()),
    );
    assert_eq!(
        fixture
            .store
            .commit_workspace_root_scan(scan_commit("scan-receipt", RUNTIME))
            .expect("exact retry replays"),
        expected,
    );

    let connection = Connection::open(database_path(&fixture.directory)).expect("inspect SQLite");
    let after = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM domain_events), (SELECT version FROM resources WHERE resource_id='resource-scan-test'), (SELECT COUNT(*) FROM request_dedup WHERE request_id='scan-receipt')",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
        )
        .expect("capture post-scan state");
    assert_eq!(after, (before.0, before.1, 1));
    let json = serde_json::to_string(&expected).expect("safe result serializes");
    assert!(!json.contains("/tmp/"));
    assert!(!json.contains("scan-test-inode"));
}

#[test]
fn workspace_root_scan_rejects_counts_above_the_census_budget_without_receipt() {
    let fixture = fixture();
    let mut commit = scan_commit("scan-over-budget", RUNTIME);
    commit.result.entries_examined = 1_001;
    commit.result.files_observed = 1_000;
    let request = commit.request.clone();

    assert!(matches!(
        fixture.store.commit_workspace_root_scan(commit),
        Err(StoreError::Invalid(_))
    ));
    assert_eq!(
        fixture
            .store
            .get_workspace_root_scan_receipt(&request)
            .expect("invalid count leaves a readable receipt lookup"),
        None,
    );
}

#[test]
fn workspace_root_scan_same_request_id_with_changed_payload_is_idempotency_conflict() {
    let fixture = fixture();
    fixture
        .store
        .commit_workspace_root_scan(scan_commit("scan-key-reuse", RUNTIME))
        .expect("first scan receipt commits");
    let mut changed = scan_commit("scan-key-reuse", RUNTIME);
    changed.request.request_payload["workspace_root_id"] = json!("different-root");
    changed.workspace_root_id = "different-root".to_owned();
    changed.result.workspace_root_id = "different-root".to_owned();
    let changed_request = changed.request.clone();
    assert_eq!(
        fixture
            .store
            .commit_workspace_root_scan(changed)
            .unwrap_err(),
        StoreError::IdempotencyConflict,
    );
    assert_eq!(
        fixture
            .store
            .get_workspace_root_scan_receipt(&changed_request)
            .unwrap_err(),
        StoreError::IdempotencyConflict,
    );
}

#[test]
fn workspace_root_scan_receipt_replays_after_runtime_restart() {
    let fixture = fixture();
    let expected = scan_commit("scan-restart-replay", RUNTIME).result;
    fixture
        .store
        .commit_workspace_root_scan(scan_commit("scan-restart-replay", RUNTIME))
        .expect("first scan receipt commits");

    // Runtime identity is a commit-time fence, but it is not part of the caller's
    // idempotency payload. A daemon restart must still resolve the exact safe receipt.
    let request_after_restart = WorkspaceCreateRequest {
        principal_id: OWNER.to_owned(),
        request_id: "scan-restart-replay".to_owned(),
        request_payload: json!({
            "operation": "workspace.root.scan.v1",
            "workspace_id": WORKSPACE,
            "workspace_root_id": ROOT,
            "root_version": 1,
        }),
    };
    assert_eq!(
        fixture
            .store
            .get_workspace_root_scan_receipt(&request_after_restart)
            .expect("receipt lookup survives daemon restart"),
        Some(expected.clone()),
    );

    let mut retry_after_restart = scan_commit("scan-restart-replay", RUNTIME);
    retry_after_restart.request = request_after_restart;
    retry_after_restart.runtime_incarnation_id = "incarnation-after-restart".to_owned();
    assert_eq!(
        fixture
            .store
            .commit_workspace_root_scan(retry_after_restart)
            .expect("exact retry resolves committed receipt before current-incarnation fencing"),
        expected,
    );
}

#[test]
fn workspace_root_scan_commit_fences_root_owner_workspace_and_runtime_changes_without_receipt() {
    for (case, sql, params_for_sql, expected_error) in [
        (
            "root lifecycle",
            "UPDATE workspace_roots SET status='PAUSED', version=2 WHERE workspace_root_id=?1",
            vec![ROOT.to_owned()],
            "conflict",
        ),
        (
            "owner change",
            "UPDATE workspaces SET owner_principal_id='other-owner' WHERE workspace_id=?1",
            vec![WORKSPACE.to_owned()],
            "not-found",
        ),
        (
            "Workspace archive",
            "UPDATE workspaces SET status='ARCHIVED' WHERE workspace_id=?1",
            vec![WORKSPACE.to_owned()],
            "not-found",
        ),
        (
            "Runtime incarnation change",
            "UPDATE runtimes SET current_incarnation_id=NULL, availability='OFFLINE' WHERE runtime_id=?1",
            vec![RUNTIME.to_owned()],
            "not-found",
        ),
    ] {
        let fixture = fixture();
        let connection =
            Connection::open(database_path(&fixture.directory)).expect("inspect SQLite");
        connection
            .execute(sql, rusqlite::params_from_iter(params_for_sql.iter()))
            .unwrap_or_else(|error| panic!("mutate {case}: {error}"));
        let request_id = format!("scan-fence-{}", case.replace(' ', "-"));
        let error = fixture
            .store
            .commit_workspace_root_scan(scan_commit(&request_id, RUNTIME))
            .expect_err("stale authorization must reject commit");
        match expected_error {
            "conflict" => assert!(
                matches!(
                    error,
                    StoreError::Conflict {
                        expected: Some(1),
                        actual: Some(2)
                    }
                ),
                "wrong stale-version result for {case}: {error:?}"
            ),
            _ => assert_eq!(error, StoreError::NotFound, "wrong fence result for {case}"),
        }
        let receipts: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM request_dedup WHERE request_id=?1",
                [&request_id],
                |row| row.get(0),
            )
            .expect("inspect rejected receipt");
        assert_eq!(receipts, 0, "fenced {case} must not leave a receipt");
    }
}
