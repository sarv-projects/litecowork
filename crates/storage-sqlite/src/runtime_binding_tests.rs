use super::{create_workspace, state_database, test_store};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
#[cfg(unix)]
use domain_workspace::{
    AddWorkspaceRoot, ChangeReplicationPolicy, ChangeWorkspaceRootStatus, CreateWorkspace,
    EventContext, WorkspaceRootService, WorkspaceService,
};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::path::Path;
use std::time::Duration;
#[cfg(unix)]
use storage_core::WorkspaceRootStatusAction;
use storage_core::{
    DeviceIdentityRecord, LocalFileIdentityBindingRecord, ReplicationPolicy,
    RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
    RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
    RuntimeWorkspaceBindingPageRequest, RuntimeWorkspaceBindingStore, StateStore,
};

const NOW: &str = "2026-10-11T10:00:00Z";

fn prepare_runtime() -> (tempfile::TempDir, crate::SqliteWorkspaceStore) {
    prepare_runtime_with_timeout(Duration::from_secs(1))
}

fn prepare_runtime_with_timeout(
    busy_timeout: Duration,
) -> (tempfile::TempDir, crate::SqliteWorkspaceStore) {
    let directory = tempfile::tempdir().expect("temporary SQLite directory");
    let store = test_store(&directory, busy_timeout);
    let workspace = create_workspace(&store, "workspace-bindings");
    let public_key = [17_u8; 32];
    let runtime_id = "runtime-bindings";
    let incarnation_id = "incarnation-bindings";
    store
        .register_local_incarnation(
            RuntimeRecord {
                runtime_id: runtime_id.to_owned(),
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
                last_seen: NOW.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                runtime_id: runtime_id.to_owned(),
                process_started_at: NOW.to_owned(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                os_boot_id: Some("boot-bindings".to_owned()),
                observed_at: NOW.to_owned(),
            },
        )
        .expect("local Runtime registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("Runtime enters control-plane serving state");
    store
        .enroll_local_runtime(storage_core::LocalRuntimeWorkspaceEnrollmentRequest {
            request: storage_core::WorkspaceCreateRequest {
                principal_id: "owner-local".to_owned(),
                request_id: "enroll-bindings".to_owned(),
                request_payload: serde_json::json!({"workspace_id": "workspace-bindings"}),
            },
            expected_workspace_version: workspace.workspace.version,
            runtime_workspace_binding_id: "binding-m".to_owned(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            workspace_id: "workspace-bindings".to_owned(),
            now: NOW.to_owned(),
            correlation_id: "correlation-bindings".to_owned(),
        })
        .expect("Runtime enrolls in Workspace");
    (directory, store)
}

#[cfg(unix)]
fn add_test_workspace_root(
    store: &crate::SqliteWorkspaceStore,
    suffix: &str,
    request_id: &str,
) -> Result<storage_core::CommittedWorkspaceRoot, storage_core::StoreError> {
    add_test_workspace_root_with_locator(
        store,
        suffix,
        request_id,
        "unix-path-b64:L3RtcC9zYW1lLWZvbGRlcg",
        "same-directory",
    )
}

#[cfg(unix)]
fn add_test_workspace_root_with_locator(
    store: &crate::SqliteWorkspaceStore,
    suffix: &str,
    request_id: &str,
    private_locator: &str,
    identity_marker: &str,
) -> Result<storage_core::CommittedWorkspaceRoot, storage_core::StoreError> {
    add_test_workspace_root_with_locator_in_workspace(
        store,
        "workspace-bindings",
        suffix,
        request_id,
        private_locator,
        identity_marker,
    )
}

#[cfg(unix)]
fn add_test_workspace_root_with_locator_in_workspace(
    store: &crate::SqliteWorkspaceStore,
    workspace_id: &str,
    suffix: &str,
    request_id: &str,
    private_locator: &str,
    identity_marker: &str,
) -> Result<storage_core::CommittedWorkspaceRoot, storage_core::StoreError> {
    let at = NOW.to_owned();
    let location_id = format!("location-root-{suffix}");
    let identity_digest = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(identity_marker.as_bytes()))
    );
    let keyed_file_identity =
        hex::encode(Sha256::digest(format!("file:{identity_marker}").as_bytes()));
    WorkspaceRootService::new(store.clone()).add_root(AddWorkspaceRoot {
        workspace_id: workspace_id.to_owned(),
        expected_workspace_version: 1,
        principal_id: "owner-local".to_owned(),
        request_id: request_id.to_owned(),
        resource_id: format!("resource-root-{suffix}"),
        identity_digest,
        file_identity: serde_json::json!({
            "filesystem_instance_id": format!("sha256:{}", keyed_file_identity),
            "volume_id": null,
            "file_id": format!("sha256:{}", keyed_file_identity),
            "generation": null,
            "platform_kind": "LINUX_DEVICE_INODE",
        }),
        location_id: location_id.clone(),
        locator_ref_id: format!("locator-ref-root-{suffix}"),
        workspace_root_id: format!("workspace-root-{suffix}"),
        runtime_id: "runtime-bindings".to_owned(),
        runtime_incarnation_id: "incarnation-bindings".to_owned(),
        private_locator: private_locator.to_owned(),
        file_identity_binding: LocalFileIdentityBindingRecord {
            location_id,
            runtime_id: "runtime-bindings".to_owned(),
            runtime_incarnation_id: "incarnation-bindings".to_owned(),
            raw_filesystem_instance_id: "17".to_owned(),
            raw_volume_id: None,
            raw_file_id: "23".to_owned(),
            raw_generation: None,
            platform_kind: "LINUX_DEVICE_INODE".to_owned(),
            observed_at: at.clone(),
        },
        display_name: "Selected folder".to_owned(),
        watch_policy: "METADATA".to_owned(),
        replication_policy: "NONE".to_owned(),
        event: EventContext {
            event_id: format!("event-root-{suffix}"),
            origin_runtime_id: "runtime-bindings".to_owned(),
            hlc_timestamp: at.clone(),
            correlation_id: format!("correlation-root-{suffix}"),
            causation_id: None,
            recorded_at: at,
        },
    })
}

#[cfg(unix)]
fn workspace_root_creation_row_counts(
    directory: &tempfile::TempDir,
) -> (i64, i64, i64, i64, i64, i64, i64) {
    let connection = Connection::open(state_database(directory)).expect("inspect root state");
    connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM resources WHERE kind='FOLDER'),
               (SELECT COUNT(*) FROM resource_locations),
               (SELECT COUNT(*) FROM resource_location_bindings),
               (SELECT COUNT(*) FROM file_identity_bindings),
               (SELECT COUNT(*) FROM workspace_roots),
               (SELECT COUNT(*) FROM domain_events WHERE entity_type IN ('Resource','WorkspaceRoot')),
               (SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'root-add-%')",
            [],
            |row| Ok((
                row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            )),
        )
        .expect("read WorkspaceRoot creation rows")
}

#[test]
#[cfg(unix)]
fn workspace_root_create_rejects_duplicate_active_file_identity_without_partial_rows() {
    let (directory, store) = prepare_runtime();
    let first =
        add_test_workspace_root(&store, "first", "root-add-first").expect("first root commits");
    let replay = add_test_workspace_root(&store, "first", "root-add-first")
        .expect("exact retry replays the original root");
    assert_eq!(replay, first);

    let before = {
        let connection = Connection::open(state_database(&directory)).expect("inspect root state");
        connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM resources WHERE kind='FOLDER'),
                   (SELECT COUNT(*) FROM resource_locations),
                   (SELECT COUNT(*) FROM resource_location_bindings),
                   (SELECT COUNT(*) FROM file_identity_bindings),
                   (SELECT COUNT(*) FROM workspace_roots),
                   (SELECT COUNT(*) FROM domain_events WHERE entity_type IN ('Resource','WorkspaceRoot')),
                   (SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'root-add-%')",
                [],
                |row| Ok((
                    row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                )),
            )
            .expect("read root state before duplicate")
    };

    let duplicate = add_test_workspace_root(&store, "second", "root-add-second");
    assert!(matches!(
        duplicate,
        Err(storage_core::StoreError::Conflict { .. })
    ));

    let after = {
        let connection = Connection::open(state_database(&directory)).expect("inspect root state");
        connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM resources WHERE kind='FOLDER'),
                   (SELECT COUNT(*) FROM resource_locations),
                   (SELECT COUNT(*) FROM resource_location_bindings),
                   (SELECT COUNT(*) FROM file_identity_bindings),
                   (SELECT COUNT(*) FROM workspace_roots),
                   (SELECT COUNT(*) FROM domain_events WHERE entity_type IN ('Resource','WorkspaceRoot')),
                   (SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'root-add-%')",
                [],
                |row| Ok((
                    row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                )),
            )
            .expect("read root state after duplicate")
    };
    assert_eq!(after, before);
}

#[test]
#[cfg(unix)]
fn workspace_root_create_rejects_parent_child_overlap_without_partial_rows() {
    let (directory, store) = prepare_runtime();
    add_test_workspace_root_with_locator(
        &store,
        "parent",
        "root-add-parent",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "parent-identity",
    )
    .expect("parent folder root commits");
    let before = workspace_root_creation_row_counts(&directory);

    let child = add_test_workspace_root_with_locator(
        &store,
        "child",
        "root-add-child",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQvY2hpbGQ",
        "child-identity",
    );

    assert!(matches!(
        child,
        Err(storage_core::StoreError::Conflict { .. })
    ));
    assert_eq!(workspace_root_creation_row_counts(&directory), before);
}

#[test]
#[cfg(unix)]
fn workspace_root_create_allows_disjoint_sibling_directories() {
    let (_directory, store) = prepare_runtime();
    add_test_workspace_root_with_locator(
        &store,
        "parent",
        "root-add-parent",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "parent-identity",
    )
    .expect("first sibling root commits");

    let sibling = add_test_workspace_root_with_locator(
        &store,
        "sibling",
        "root-add-sibling",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQtZXh0cmE",
        "sibling-identity",
    );

    assert!(
        sibling.is_ok(),
        "disjoint sibling roots must remain independent"
    );
}

#[test]
#[cfg(unix)]
fn workspace_root_create_rejects_parent_when_child_scope_already_exists() {
    let (_directory, store) = prepare_runtime();
    add_test_workspace_root_with_locator(
        &store,
        "child",
        "root-add-child",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQvY2hpbGQ",
        "child-identity",
    )
    .expect("child folder root commits first");

    let parent = add_test_workspace_root_with_locator(
        &store,
        "parent",
        "root-add-parent",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "parent-identity",
    );

    assert!(matches!(
        parent,
        Err(storage_core::StoreError::Conflict { .. })
    ));
}

#[test]
#[cfg(unix)]
fn workspace_root_create_keeps_paused_scope_reserved_until_revocation() {
    let (_directory, store) = prepare_runtime();
    let root = add_test_workspace_root_with_locator(
        &store,
        "parent",
        "root-add-parent",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "parent-identity",
    )
    .expect("parent folder root commits");
    WorkspaceRootService::new(store.clone())
        .change_root_status(ChangeWorkspaceRootStatus {
            workspace_id: root.root.workspace_id.clone(),
            workspace_root_id: root.root.workspace_root_id.clone(),
            principal_id: "owner-local".to_owned(),
            request_id: "root-pause-parent".to_owned(),
            expected_version: root.root.version,
            action: WorkspaceRootStatusAction::Pause,
            runtime_id: None,
            runtime_incarnation_id: None,
            event: EventContext {
                event_id: "event-pause-parent".to_owned(),
                origin_runtime_id: "runtime-bindings".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-pause-parent".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner pauses the root without revoking its scope");

    let child = add_test_workspace_root_with_locator(
        &store,
        "child",
        "root-add-child",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQvY2hpbGQ",
        "child-identity",
    );

    assert!(matches!(
        child,
        Err(storage_core::StoreError::Conflict { .. })
    ));
}

#[test]
#[cfg(unix)]
fn workspace_root_create_keeps_unavailable_scope_reserved() {
    let (directory, store) = prepare_runtime();
    add_test_workspace_root_with_locator(
        &store,
        "parent",
        "root-add-parent",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "parent-identity",
    )
    .expect("parent folder root commits");
    Connection::open(state_database(&directory))
        .expect("open root state")
        .execute(
            "UPDATE workspace_roots SET status = 'UNAVAILABLE' WHERE workspace_root_id = 'workspace-root-parent'",
            [],
        )
        .expect("simulate startup revalidation making root unavailable");

    let child = add_test_workspace_root_with_locator(
        &store,
        "child",
        "root-add-child",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQvY2hpbGQ",
        "child-identity",
    );

    assert!(matches!(
        child,
        Err(storage_core::StoreError::Conflict { .. })
    ));
}

#[test]
#[cfg(unix)]
fn workspace_root_create_scopes_path_overlap_to_its_workspace() {
    let (_directory, store) = prepare_runtime();
    let other_workspace = domain_workspace::WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: "workspace-other".to_owned(),
            name: "Second Workspace".to_owned(),
            owner_principal_id: "owner-local".to_owned(),
            event: EventContext {
                event_id: "event-create-other-workspace".to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-create-other-workspace".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("second Workspace commits");
    store
        .enroll_local_runtime(storage_core::LocalRuntimeWorkspaceEnrollmentRequest {
            request: storage_core::WorkspaceCreateRequest {
                principal_id: "owner-local".to_owned(),
                request_id: "enroll-other-workspace".to_owned(),
                request_payload: serde_json::json!({"workspace_id": "workspace-other"}),
            },
            expected_workspace_version: other_workspace.workspace.version,
            runtime_workspace_binding_id: "binding-other".to_owned(),
            runtime_id: "runtime-bindings".to_owned(),
            runtime_incarnation_id: "incarnation-bindings".to_owned(),
            workspace_id: "workspace-other".to_owned(),
            now: NOW.to_owned(),
            correlation_id: "correlation-other".to_owned(),
        })
        .expect("Runtime enrolls in second Workspace");
    add_test_workspace_root_with_locator(
        &store,
        "first-workspace",
        "root-add-first-workspace",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "first-workspace-identity",
    )
    .expect("first Workspace root commits");

    let same_path_elsewhere = add_test_workspace_root_with_locator_in_workspace(
        &store,
        "workspace-other",
        "second-workspace",
        "root-add-second-workspace",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "second-workspace-identity",
    );

    assert!(
        same_path_elsewhere.is_ok(),
        "root scopes are Workspace-local"
    );
}

#[test]
#[cfg(unix)]
fn workspace_root_create_releases_scope_after_revocation() {
    let (_directory, store) = prepare_runtime();
    let root = add_test_workspace_root_with_locator(
        &store,
        "parent",
        "root-add-parent",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "parent-identity",
    )
    .expect("parent folder root commits");
    WorkspaceRootService::new(store.clone())
        .change_root_status(ChangeWorkspaceRootStatus {
            workspace_id: root.root.workspace_id.clone(),
            workspace_root_id: root.root.workspace_root_id.clone(),
            principal_id: "owner-local".to_owned(),
            request_id: "root-revoke-parent".to_owned(),
            expected_version: root.root.version,
            action: WorkspaceRootStatusAction::Revoke,
            runtime_id: None,
            runtime_incarnation_id: None,
            event: EventContext {
                event_id: "event-revoke-parent".to_owned(),
                origin_runtime_id: "runtime-bindings".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-revoke-parent".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner revokes the root and releases its scope");

    let child = add_test_workspace_root_with_locator(
        &store,
        "child",
        "root-add-child",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQvY2hpbGQ",
        "child-identity",
    );

    assert!(
        child.is_ok(),
        "a revoked root no longer reserves its former scope"
    );
}

#[test]
#[cfg(unix)]
fn workspace_root_create_fails_closed_for_malformed_persisted_locator() {
    let (directory, store) = prepare_runtime();
    add_test_workspace_root_with_locator(
        &store,
        "parent",
        "root-add-parent",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "parent-identity",
    )
    .expect("parent folder root commits");
    Connection::open(state_database(&directory))
        .expect("open root state")
        .execute(
            "UPDATE resource_location_bindings
             SET private_locator = 'unix-path-b64:%%%'
             WHERE location_id = 'location-root-parent'",
            [],
        )
        .expect("simulate corrupted private locator");
    let before = workspace_root_creation_row_counts(&directory);

    let sibling = add_test_workspace_root_with_locator(
        &store,
        "sibling",
        "root-add-sibling",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQtZXh0cmE",
        "sibling-identity",
    );

    assert!(matches!(
        sibling,
        Err(storage_core::StoreError::Integrity(_))
    ));
    assert_eq!(workspace_root_creation_row_counts(&directory), before);
}

#[test]
#[cfg(unix)]
fn workspace_root_create_rejects_noncanonical_candidate_locator() {
    let (_directory, store) = prepare_runtime();

    let result = add_test_workspace_root_with_locator(
        &store,
        "noncanonical",
        "root-add-noncanonical",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQvL2NoaWxk",
        "noncanonical-identity",
    );

    assert!(matches!(
        result,
        Err(storage_core::StoreError::Integrity(_))
    ));
}

#[cfg(unix)]
fn private_locator_for_test_path(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    format!(
        "unix-path-b64:{}",
        URL_SAFE_NO_PAD.encode(path.as_os_str().as_bytes())
    )
}

fn windows_private_locator(path: &str) -> String {
    let bytes = path
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    format!("windows-path-u16le-b64:{}", URL_SAFE_NO_PAD.encode(bytes))
}

#[test]
fn windows_workspace_root_locator_requires_canonical_drive_rooted_utf16() {
    let valid = windows_private_locator("C:\\Users\\Owner\\Documents");
    assert_eq!(
        super::super::decode_windows_workspace_root_locator(&valid)
            .expect("canonical Windows path decodes"),
        "C:\\Users\\Owner\\Documents"
            .encode_utf16()
            .collect::<Vec<_>>()
    );

    for path in [
        "C:relative",
        "\\\\server\\share",
        "\\\\?\\C:\\device",
        "C:\\folder\\..\\secret",
        "C:\\folder\\.",
        "C:\\folder\\",
        "C:\\folder//child",
        "C:\\folder\\bad.",
        "C:\\folder\\bad ",
    ] {
        assert!(
            super::super::decode_windows_workspace_root_locator(&windows_private_locator(path))
                .is_err(),
            "rejected noncanonical path {path:?}"
        );
    }

    let odd_bytes = URL_SAFE_NO_PAD.encode([b'C', 0, b':']);
    assert!(
        super::super::decode_windows_workspace_root_locator(&format!(
            "windows-path-u16le-b64:{odd_bytes}"
        ))
        .is_err()
    );

    let nul_path = windows_private_locator("C:\\folder\\\0name");
    assert!(super::super::decode_windows_workspace_root_locator(&nul_path).is_err());

    let unpaired_surrogate = URL_SAFE_NO_PAD.encode([b'C', 0, b':', 0, b'\\', 0, 0, 0xd8]);
    assert!(
        super::super::decode_windows_workspace_root_locator(&format!(
            "windows-path-u16le-b64:{unpaired_surrogate}"
        ))
        .is_err()
    );

    let oversized = windows_private_locator(&format!("C:\\{}", "a".repeat(16_896)));
    assert!(super::super::decode_windows_workspace_root_locator(&oversized).is_err());
}

#[test]
fn windows_workspace_root_identity_binding_requires_full_canonical_file_id() {
    let mut binding = LocalFileIdentityBindingRecord {
        location_id: "location".to_owned(),
        runtime_id: "runtime".to_owned(),
        runtime_incarnation_id: "incarnation".to_owned(),
        raw_filesystem_instance_id: "0123456789abcdef".to_owned(),
        raw_volume_id: None,
        raw_file_id: "0123456789abcdef0123456789abcdef".to_owned(),
        raw_generation: None,
        platform_kind: "WINDOWS_VOLUME_FILE_ID_128".to_owned(),
        observed_at: NOW.to_owned(),
    };
    assert!(super::super::valid_workspace_root_raw_identity(&binding));

    binding.raw_file_id = "0123456789ABCDEF0123456789ABCDEF".to_owned();
    assert!(!super::super::valid_workspace_root_raw_identity(&binding));
    binding.raw_file_id = "0123456789abcdef0123456789abcdef".to_owned();
    binding.raw_volume_id = Some("0123456789abcdef".to_owned());
    assert!(!super::super::valid_workspace_root_raw_identity(&binding));
}

#[cfg(unix)]
#[test]
fn workspace_root_create_compares_real_canonical_directory_paths() {
    let (_directory, store) = prepare_runtime();
    let filesystem = tempfile::tempdir().expect("temporary filesystem root");
    let parent = filesystem.path().join("parent");
    let child = parent.join("child");
    let sibling = filesystem.path().join("parent-sibling");
    std::fs::create_dir(&parent).expect("create parent directory");
    std::fs::create_dir(&child).expect("create child directory");
    std::fs::create_dir(&sibling).expect("create sibling directory");
    let parent = parent.canonicalize().expect("canonical parent path");
    let child = child.canonicalize().expect("canonical child path");
    let sibling = sibling.canonicalize().expect("canonical sibling path");
    add_test_workspace_root_with_locator(
        &store,
        "parent",
        "root-add-parent",
        &private_locator_for_test_path(&parent),
        "parent-identity",
    )
    .expect("parent folder root commits");

    let nested = add_test_workspace_root_with_locator(
        &store,
        "child",
        "root-add-child",
        &private_locator_for_test_path(&child),
        "child-identity",
    );
    assert!(matches!(
        nested,
        Err(storage_core::StoreError::Conflict { .. })
    ));

    let textual_prefix_sibling = add_test_workspace_root_with_locator(
        &store,
        "sibling",
        "root-add-sibling",
        &private_locator_for_test_path(&sibling),
        "sibling-identity",
    );
    assert!(
        textual_prefix_sibling.is_ok(),
        "a path with the parent's textual prefix is a separate component scope"
    );
}

#[test]
#[cfg(unix)]
fn workspace_root_create_rejects_oversized_persisted_locator_before_scope_comparison() {
    let (directory, store) = prepare_runtime();
    add_test_workspace_root_with_locator(
        &store,
        "parent",
        "root-add-parent",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
        "parent-identity",
    )
    .expect("parent folder root commits");
    let oversized_path = format!("/{}", "a".repeat(32 * 1024));
    let oversized_locator = format!(
        "unix-path-b64:{}",
        URL_SAFE_NO_PAD.encode(oversized_path.as_bytes())
    );
    Connection::open(state_database(&directory))
        .expect("open root state")
        .execute(
            "UPDATE resource_location_bindings SET private_locator = ?1
             WHERE location_id = 'location-root-parent'",
            [oversized_locator],
        )
        .expect("simulate oversized persisted locator");
    let before = workspace_root_creation_row_counts(&directory);

    let sibling = add_test_workspace_root_with_locator(
        &store,
        "sibling",
        "root-add-sibling",
        "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQtZXh0cmE",
        "sibling-identity",
    );

    assert!(matches!(
        sibling,
        Err(storage_core::StoreError::Integrity(_))
    ));
    assert_eq!(workspace_root_creation_row_counts(&directory), before);
}

#[test]
#[cfg(unix)]
fn workspace_root_create_serializes_concurrent_parent_child_requests() {
    let busy_timeout = Duration::from_secs(30);
    let (directory, first_store) = prepare_runtime_with_timeout(busy_timeout);
    let second_store = crate::SqliteWorkspaceStore::open(
        state_database(&directory),
        first_store.inner.blobs.clone(),
        crate::SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout,
        },
    )
    .expect("second SQLite writer opens the same database");
    let first = std::thread::spawn(move || {
        add_test_workspace_root_with_locator(
            &first_store,
            "parent",
            "root-add-parent",
            "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
            "parent-identity",
        )
    });
    let second = std::thread::spawn(move || {
        add_test_workspace_root_with_locator(
            &second_store,
            "child",
            "root-add-child",
            "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQvY2hpbGQ",
            "child-identity",
        )
    });
    let results = [
        first.join().expect("parent root task completes"),
        second.join().expect("child root task completes"),
    ];

    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        1,
        "only one overlapping root request may commit"
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(storage_core::StoreError::Conflict { .. })))
            .count(),
        1,
        "the overlapping request must receive a conflict; got {results:?}"
    );
    assert_eq!(
        workspace_root_creation_row_counts(&directory),
        (1, 1, 1, 1, 1, 3, 1)
    );
}

#[test]
#[cfg(unix)]
fn workspace_root_create_rolls_back_each_projection_event_and_receipt_failure() {
    let failure_triggers = [
        (
            "resource",
            "CREATE TRIGGER inject_root_failure BEFORE INSERT ON resources
             BEGIN SELECT RAISE(ABORT, 'injected WorkspaceRoot failure'); END",
        ),
        (
            "location",
            "CREATE TRIGGER inject_root_failure BEFORE INSERT ON resource_locations
             BEGIN SELECT RAISE(ABORT, 'injected WorkspaceRoot failure'); END",
        ),
        (
            "private_locator",
            "CREATE TRIGGER inject_root_failure BEFORE INSERT ON resource_location_bindings
             BEGIN SELECT RAISE(ABORT, 'injected WorkspaceRoot failure'); END",
        ),
        (
            "file_identity",
            "CREATE TRIGGER inject_root_failure BEFORE INSERT ON file_identity_bindings
             BEGIN SELECT RAISE(ABORT, 'injected WorkspaceRoot failure'); END",
        ),
        (
            "root",
            "CREATE TRIGGER inject_root_failure BEFORE INSERT ON workspace_roots
             BEGIN SELECT RAISE(ABORT, 'injected WorkspaceRoot failure'); END",
        ),
        (
            "resource_event",
            "CREATE TRIGGER inject_root_failure BEFORE INSERT ON domain_events
             WHEN NEW.event_type = 'resource.created.v1'
             BEGIN SELECT RAISE(ABORT, 'injected WorkspaceRoot failure'); END",
        ),
        (
            "location_event",
            "CREATE TRIGGER inject_root_failure BEFORE INSERT ON domain_events
             WHEN NEW.event_type = 'resource.location.changed.v1'
             BEGIN SELECT RAISE(ABORT, 'injected WorkspaceRoot failure'); END",
        ),
        (
            "root_event",
            "CREATE TRIGGER inject_root_failure BEFORE INSERT ON domain_events
             WHEN NEW.event_type = 'workspace.root.created.v1'
             BEGIN SELECT RAISE(ABORT, 'injected WorkspaceRoot failure'); END",
        ),
        (
            "receipt",
            "CREATE TRIGGER inject_root_failure BEFORE INSERT ON request_dedup
             WHEN NEW.request_id LIKE 'root-add-%'
             BEGIN SELECT RAISE(ABORT, 'injected WorkspaceRoot failure'); END",
        ),
    ];

    for (boundary, trigger_sql) in failure_triggers {
        let (directory, store) = prepare_runtime();
        Connection::open(state_database(&directory))
            .expect("open SQLite fixture")
            .execute_batch(trigger_sql)
            .unwrap_or_else(|error| panic!("install {boundary} failure trigger: {error}"));

        let result = add_test_workspace_root_with_locator(
            &store,
            "injected",
            "root-add-injected",
            "unix-path-b64:L3RtcC9saXRlY293b3JrLXJvb3Qtb3ZlcmxhcC9wYXJlbnQ",
            "injected-identity",
        );

        assert!(
            result.is_err(),
            "{boundary} failure must reject the transaction"
        );
        assert_eq!(
            workspace_root_creation_row_counts(&directory),
            (0, 0, 0, 0, 0, 0, 0),
            "{boundary} failure left a partial root projection or receipt"
        );
    }
}

#[test]
fn workspace_runtime_binding_page_is_owner_scoped_ordered_and_includes_revoked_history() {
    let (directory, store) = prepare_runtime();
    let database = state_database(&directory);
    let connection = Connection::open(database).expect("open SQLite fixture connection");
    connection
        .execute(
            "INSERT INTO runtimes(runtime_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,resource_capacity_json,last_seen,version) SELECT 'runtime-pending',device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,'OFFLINE',startup_policy,NULL,resource_capacity_json,last_seen,version FROM runtimes WHERE runtime_id='runtime-bindings'",
            [],
        )
        .expect("second Runtime inventory fixture is inserted");
    connection
        .execute(
            "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,revoked_at,version) VALUES('binding-b','runtime-pending','workspace-bindings','MESH_PAIRING','PENDING','[\"EXECUTOR\"]',?1,NULL,NULL,1)",
            [NOW],
        )
        .expect("retained pending binding is inserted");
    for id in ["binding-a", "binding-z"] {
        connection
            .execute(
                "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,revoked_at,version) VALUES(?1,'runtime-bindings','workspace-bindings','MESH_PAIRING','REVOKED','[\"EXECUTOR\"]',?2,?2,?2,1)",
                rusqlite::params![id, NOW],
            )
            .expect("retained revoked binding is inserted");
    }

    let first = store
        .list_workspace_runtime_bindings(RuntimeWorkspaceBindingPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-bindings".to_owned(),
            after_binding_id: None,
            limit: 2,
        })
        .expect("owner can read first page");
    assert_eq!(
        first
            .iter()
            .map(|binding| binding.runtime_workspace_binding_id.as_str())
            .collect::<Vec<_>>(),
        ["binding-a", "binding-b"]
    );
    assert_eq!(first[0].status, "REVOKED");
    assert_eq!(first[0].revoked_at.as_deref(), Some(NOW));
    assert_eq!(first[1].status, "PENDING");
    assert_eq!(first[1].activated_at, None);

    let second = store
        .list_workspace_runtime_bindings(RuntimeWorkspaceBindingPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-bindings".to_owned(),
            after_binding_id: Some("binding-b".to_owned()),
            limit: 2,
        })
        .expect("owner can read keyset continuation");
    assert_eq!(
        second
            .iter()
            .map(|binding| binding.runtime_workspace_binding_id.as_str())
            .collect::<Vec<_>>(),
        ["binding-m", "binding-z"]
    );

    let unauthorized = store
        .list_workspace_runtime_bindings(RuntimeWorkspaceBindingPageRequest {
            owner_principal_id: "another-principal".to_owned(),
            workspace_id: "workspace-bindings".to_owned(),
            after_binding_id: None,
            limit: 3,
        })
        .expect("foreign owner receives no binding data");
    assert!(unauthorized.is_empty());

    for invalid_limit in [0, 202] {
        let invalid = store.list_workspace_runtime_bindings(RuntimeWorkspaceBindingPageRequest {
            owner_principal_id: "owner-local".to_owned(),
            workspace_id: "workspace-bindings".to_owned(),
            after_binding_id: None,
            limit: invalid_limit,
        });
        assert!(matches!(invalid, Err(storage_core::StoreError::Invalid(_))));
    }
}

#[cfg(unix)]
#[test]
fn selected_folders_policy_commits_active_workspace_roots_and_replays_exactly() {
    let (_directory, store) = prepare_runtime();
    let first = add_test_workspace_root_with_locator(
        &store,
        "policy-a",
        "root-add-policy-a",
        "unix-path-b64:L3RtcC9wb2xpY3ktYQ",
        "policy-directory-a",
    )
    .expect("first active WorkspaceRoot commits");
    let second = add_test_workspace_root_with_locator(
        &store,
        "policy-b",
        "root-add-policy-b",
        "unix-path-b64:L3RtcC9wb2xpY3ktYg",
        "policy-directory-b",
    )
    .expect("second active WorkspaceRoot commits");

    let selected_ids = vec![
        second.root.workspace_root_id.clone(),
        first.root.workspace_root_id.clone(),
    ];
    let request_payload = serde_json::json!({
        "operation": "workspace.policy.update.v1",
        "workspace_id": "workspace-bindings",
        "expected_version": 1,
        "replication_policy": "SELECTED_FOLDERS",
        "replication_scope_root_ids": selected_ids,
    });
    let command = ChangeReplicationPolicy {
        workspace_id: "workspace-bindings".to_owned(),
        expected_version: 1,
        policy: ReplicationPolicy::SelectedFolders,
        replication_scope_root_ids: vec![
            second.root.workspace_root_id,
            first.root.workspace_root_id,
        ],
        event: EventContext {
            event_id: "event-selected-policy".to_owned(),
            origin_runtime_id: "runtime-bindings".to_owned(),
            hlc_timestamp: NOW.to_owned(),
            correlation_id: "correlation-selected-policy".to_owned(),
            causation_id: None,
            recorded_at: NOW.to_owned(),
        },
    };
    let committed = WorkspaceService::new(store.clone())
        .change_replication_policy_idempotent(
            command.clone(),
            "owner-local".to_owned(),
            "policy-selected-roots-request".to_owned(),
            request_payload.clone(),
        )
        .expect("selected roots policy commits after root admission");

    assert_eq!(committed.workspace.version, 2);
    assert_eq!(
        committed.workspace.replication_policy,
        ReplicationPolicy::SelectedFolders
    );
    assert_eq!(
        committed.workspace.replication_scope_root_ids,
        ["workspace-root-policy-a", "workspace-root-policy-b"]
    );
    assert_eq!(
        committed.event.payload["replication_scope_root_ids"],
        serde_json::json!(["workspace-root-policy-a", "workspace-root-policy-b"])
    );

    let replay = WorkspaceService::new(store.clone())
        .change_replication_policy_idempotent(
            command,
            "owner-local".to_owned(),
            "policy-selected-roots-request".to_owned(),
            request_payload,
        )
        .expect("exact selected-roots retry replays after version advance");
    assert_eq!(replay, committed);
    assert_eq!(
        store
            .get_workspace("workspace-bindings")
            .expect("read persisted selected-roots policy")
            .expect("Workspace remains")
            .replication_scope_root_ids,
        ["workspace-root-policy-a", "workspace-root-policy-b"]
    );

    WorkspaceRootService::new(store.clone())
        .change_root_status(ChangeWorkspaceRootStatus {
            workspace_id: "workspace-bindings".to_owned(),
            workspace_root_id: "workspace-root-policy-a".to_owned(),
            principal_id: "owner-local".to_owned(),
            request_id: "root-pause-policy-a".to_owned(),
            expected_version: first.root.version,
            action: storage_core::WorkspaceRootStatusAction::Pause,
            runtime_id: None,
            runtime_incarnation_id: None,
            event: EventContext {
                event_id: "event-pause-policy-a".to_owned(),
                origin_runtime_id: "runtime-bindings".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-pause-policy-a".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("pausing a selected root preserves the saved scope preference");
    assert_eq!(
        store
            .get_workspace("workspace-bindings")
            .expect("read Workspace after root pause")
            .expect("Workspace remains")
            .replication_scope_root_ids,
        ["workspace-root-policy-a", "workspace-root-policy-b"]
    );

    let paused_selection = WorkspaceService::new(store.clone()).change_replication_policy_idempotent(
        ChangeReplicationPolicy {
            workspace_id: "workspace-bindings".to_owned(),
            expected_version: 2,
            policy: ReplicationPolicy::SelectedFolders,
            replication_scope_root_ids: vec![
                "workspace-root-policy-a".to_owned(),
                "workspace-root-policy-b".to_owned(),
            ],
            event: EventContext {
                event_id: "event-select-paused-root".to_owned(),
                origin_runtime_id: "runtime-bindings".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-select-paused-root".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        },
        "owner-local".to_owned(),
        "policy-select-paused-root-request".to_owned(),
        serde_json::json!({
            "operation": "workspace.policy.update.v1",
            "workspace_id": "workspace-bindings",
            "expected_version": 2,
            "replication_policy": "SELECTED_FOLDERS",
            "replication_scope_root_ids": ["workspace-root-policy-a", "workspace-root-policy-b"],
        }),
    );
    assert!(matches!(
        paused_selection,
        Err(storage_core::StoreError::Invalid(_))
    ));
    let connection = Connection::open(state_database(&_directory)).expect("inspect policy events");
    let policy_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'workspace.replication_policy.changed.v1'",
            ["workspace-bindings"],
            |row| row.get(0),
        )
        .expect("count committed policy events");
    assert_eq!(policy_events, 1);
}

#[cfg(unix)]
#[test]
fn selected_folders_policy_rejects_unknown_roots_without_partial_commit() {
    let (directory, store) = prepare_runtime();
    let request_payload = serde_json::json!({
        "operation": "workspace.policy.update.v1",
        "workspace_id": "workspace-bindings",
        "expected_version": 1,
        "replication_policy": "SELECTED_FOLDERS",
        "replication_scope_root_ids": ["workspace-root-not-in-this-workspace"],
    });
    let result = WorkspaceService::new(store.clone()).change_replication_policy_idempotent(
        ChangeReplicationPolicy {
            workspace_id: "workspace-bindings".to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::SelectedFolders,
            replication_scope_root_ids: vec!["workspace-root-not-in-this-workspace".to_owned()],
            event: EventContext {
                event_id: "event-selected-policy-invalid-root".to_owned(),
                origin_runtime_id: "runtime-bindings".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-selected-policy-invalid-root".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        },
        "owner-local".to_owned(),
        "policy-selected-roots-invalid-request".to_owned(),
        request_payload,
    );

    assert!(matches!(result, Err(storage_core::StoreError::Invalid(_))));
    let current = store
        .get_workspace("workspace-bindings")
        .expect("read Workspace after invalid policy request")
        .expect("Workspace remains");
    assert_eq!(current.version, 1);
    assert_eq!(current.replication_policy, ReplicationPolicy::LocalOnly);
    assert!(current.replication_scope_root_ids.is_empty());

    let connection = Connection::open(state_database(&directory)).expect("inspect policy rows");
    let policy_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'workspace.replication_policy.changed.v1'",
            ["workspace-bindings"],
            |row| row.get(0),
        )
        .expect("count policy events");
    let request_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            ["owner-local", "policy-selected-roots-invalid-request"],
            |row| row.get(0),
        )
        .expect("count policy receipts");
    assert_eq!(policy_events, 0);
    assert_eq!(request_receipts, 0);
}
