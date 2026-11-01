use super::test_store;
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use sha2::{Digest, Sha256};
use std::time::Duration;
use storage_core::{
    AgentCatalogStore, DeviceIdentityRecord, LocalRuntimeWorkspaceEnrollmentRequest,
    RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
    RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
    RuntimeWorkspaceBindingStore, WorkspaceCreateRequest,
};

const OBSERVED: &str = "2026-10-10T10:00:00Z";
const NOW: &str = "2026-10-10T10:02:00Z";
const FUTURE: &str = "2026-10-10T10:03:00Z";
const EXPIRED: &str = "2026-10-10T10:01:00Z";
const EXPIRY: &str = "2026-10-10T10:10:00Z";
const RUNTIME_ID: &str = "runtime-capability-host-test";
const INCARNATION_ID: &str = "incarnation-capability-host-test";

fn prepare_runtime(workspace_id: &str) -> (tempfile::TempDir, crate::SqliteWorkspaceStore) {
    let directory = tempfile::tempdir().expect("temporary SQLite directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let workspace = create_fixture_workspace(&store, workspace_id, "event-host-owner");
    let public_key = [8_u8; 32];
    store
        .register_local_incarnation(
            RuntimeRecord {
                runtime_id: RUNTIME_ID.to_owned(),
                device_identity: DeviceIdentityRecord {
                    device_id: format!(
                        "ed25519-sha256:{}",
                        hex::encode(Sha256::digest(public_key))
                    ),
                    public_key: format!("ed25519:{}", hex::encode(public_key)),
                    key_version: 1,
                    issued_at: OBSERVED.to_owned(),
                    display_name: None,
                },
                runtime_version: "0.1.0-test".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                roles: vec!["OPERATOR_ENDPOINT".to_owned()],
                trust_zone: "PERSONAL_DEVICE".to_owned(),
                availability: "RECOVERING".to_owned(),
                startup_policy: "MANUAL".to_owned(),
                current_incarnation_id: INCARNATION_ID.to_owned(),
                resource_capacity: serde_json::json!({"sampled_at": OBSERVED}),
                last_seen: OBSERVED.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: INCARNATION_ID.to_owned(),
                runtime_id: RUNTIME_ID.to_owned(),
                process_started_at: OBSERVED.to_owned(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: INCARNATION_ID.to_owned(),
                os_boot_id: Some("boot-capability-host-test".to_owned()),
                observed_at: OBSERVED.to_owned(),
            },
        )
        .expect("Runtime registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("Runtime is available for control-plane reads");
    store
        .enroll_local_runtime(enrollment(workspace_id, workspace.workspace.version))
        .expect("Runtime enrolls in Workspace");
    (directory, store)
}

fn create_fixture_workspace(
    store: &crate::SqliteWorkspaceStore,
    workspace_id: &str,
    event_id: &str,
) -> storage_core::CommittedWorkspace {
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Capability host test workspace".to_owned(),
            owner_principal_id: "owner-local".to_owned(),
            event: EventContext {
                event_id: event_id.to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                hlc_timestamp: OBSERVED.to_owned(),
                correlation_id: format!("correlation-{event_id}"),
                causation_id: None,
                recorded_at: OBSERVED.to_owned(),
            },
        })
        .expect("fixture Workspace commits")
}

fn enrollment(
    workspace_id: &str,
    workspace_version: u64,
) -> LocalRuntimeWorkspaceEnrollmentRequest {
    LocalRuntimeWorkspaceEnrollmentRequest {
        request: WorkspaceCreateRequest {
            principal_id: "owner-local".to_owned(),
            request_id: format!("enroll-{workspace_id}"),
            request_payload: serde_json::json!({"workspace_id": workspace_id}),
        },
        expected_workspace_version: workspace_version,
        runtime_workspace_binding_id: format!("binding-{workspace_id}"),
        runtime_id: RUNTIME_ID.to_owned(),
        runtime_incarnation_id: INCARNATION_ID.to_owned(),
        workspace_id: workspace_id.to_owned(),
        now: NOW.to_owned(),
        correlation_id: format!("correlation-{workspace_id}"),
    }
}

fn seed_host_rows(directory: &tempfile::TempDir) {
    let mut connection = rusqlite::Connection::open(super::state_database(directory))
        .expect("fixture database opens");
    let transaction = connection
        .transaction()
        .expect("fixture transaction starts");
    let capability_ref = serde_json::json!({
        "capability_id": "capability.search",
        "identity_kind": "PACKAGE_COMPONENT",
        "source": "registry",
        "package_version": "1.0.0",
        "digest": format!("sha256:{}", "a".repeat(64)),
        "component": "search"
    })
    .to_string();
    for (host_id, observed_at, expires_at, health) in [
        ("host-a", OBSERVED, EXPIRY, "HEALTHY"),
        ("host-b", OBSERVED, EXPIRED, "DEGRADED"),
        ("host-c", FUTURE, EXPIRY, "HEALTHY"),
    ] {
        let digest = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(format!("{host_id}:capability").as_bytes()))
        );
        let isolation_digest =
            format!("sha256:{}", hex::encode(Sha256::digest(host_id.as_bytes())));
        transaction
            .execute(
                "INSERT INTO capability_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,capability_ref_json,capability_identity_digest,configuration_digest,isolation_partition_digest,sharing_policy,hosting_mode,provider_instance_ref,state,health,observed_at,expires_at,version) VALUES(?1,?2,?3,?4,?5,?6,?7,'TASK_ISOLATED','LOCAL_MANAGED',?8,'READY',?9,?10,?11,1)",
                rusqlite::params![
                    host_id,
                    RUNTIME_ID,
                    INCARNATION_ID,
                    capability_ref,
                    digest,
                    format!("sha256:{}", "c".repeat(64)),
                    isolation_digest,
                    format!("provider://private/{host_id}-locator-secret"),
                    health,
                    observed_at,
                    expires_at,
                ],
            )
            .expect("host observation seeds");
    }
    for (workspace_id, conversation_id) in [
        ("workspace-host-owner", "conversation-host-owner"),
        ("workspace-host-other", "conversation-host-other"),
    ] {
        transaction
            .execute(
                "INSERT INTO conversations(conversation_id,workspace_id,title,created_at,version) VALUES(?1,?2,NULL,?3,1)",
                rusqlite::params![conversation_id, workspace_id, OBSERVED],
            )
            .expect("Conversation fixture seeds");
    }
    for (activation_id, workspace_id, conversation_id, initial_status, final_status) in [
        (
            "activation-owner-1",
            "workspace-host-owner",
            "conversation-host-owner",
            "ACTIVE",
            "ACTIVE",
        ),
        (
            "activation-owner-2",
            "workspace-host-owner",
            "conversation-host-owner",
            "ACTIVE",
            "STOPPING",
        ),
        (
            "activation-owner-3",
            "workspace-host-owner",
            "conversation-host-owner",
            "ACTIVE",
            "STOPPED",
        ),
        (
            "activation-other",
            "workspace-host-other",
            "conversation-host-other",
            "ACTIVE",
            "ACTIVE",
        ),
    ] {
        transaction
            .execute(
                "INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,conversation_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at,version) VALUES(?1,?2,'CONVERSATION',?3,?4,?5,?6,'DIRECT_MCP',?7,'HEALTHY',?8,?8,1)",
                rusqlite::params![
                    activation_id,
                    workspace_id,
                    conversation_id,
                    capability_ref,
                    RUNTIME_ID,
                    INCARNATION_ID,
                    initial_status,
                    OBSERVED,
                ],
            )
            .expect("Activation fixture seeds");
        transaction
            .execute(
                "INSERT INTO capability_activation_host_bindings(activation_id,host_instance_id,provider_handle_ref,bound_at) VALUES(?1,'host-a',?2,?3)",
                rusqlite::params![
                    activation_id,
                    format!("handle://private/{activation_id}-secret"),
                    OBSERVED,
                ],
            )
            .expect("Activation host binding fixture seeds");
        if final_status != initial_status {
            transaction
                .execute(
                    "UPDATE capability_activations SET status=?1 WHERE activation_id=?2",
                    rusqlite::params![final_status, activation_id],
                )
                .expect("Activation fixture settles");
        }
    }
    transaction.commit().expect("fixture transaction commits");
}

#[test]
fn capability_host_page_is_workspace_scoped_paged_and_marks_stale_health_unknown() {
    use storage_core::{RuntimeCapabilityHostPageRequest, RuntimeCapabilityHostPosition};

    let (directory, store) = prepare_runtime("workspace-host-owner");
    let other = create_fixture_workspace(&store, "workspace-host-other", "event-host-other");
    store
        .enroll_local_runtime(enrollment("workspace-host-other", other.workspace.version))
        .expect("Runtime enrolls in second Workspace");
    create_fixture_workspace(&store, "workspace-host-unbound", "event-host-unbound");
    seed_host_rows(&directory);

    let list = |owner: &str,
                workspace: &str,
                expected_incarnation_id: Option<String>,
                after: Option<RuntimeCapabilityHostPosition>,
                limit: usize| {
        store.list_runtime_capability_hosts(RuntimeCapabilityHostPageRequest {
            owner_principal_id: owner.to_owned(),
            workspace_id: workspace.to_owned(),
            runtime_id: RUNTIME_ID.to_owned(),
            expected_incarnation_id,
            now: NOW.to_owned(),
            after,
            limit,
        })
    };

    let first =
        list("owner-local", "workspace-host-owner", None, None, 2).expect("authorized page reads");
    assert_eq!(
        first
            .iter()
            .map(|host| host.host_instance_id.as_str())
            .collect::<Vec<_>>(),
        ["host-a", "host-b"]
    );
    assert_eq!(first[0].active_activation_count, 2);
    assert_eq!(first[0].health, "HEALTHY");
    assert_eq!(first[1].health, "UNKNOWN");
    assert_eq!(first[1].state, "READY");
    assert!(
        !serde_json::to_string(&first)
            .expect("host views serialize")
            .contains("private")
    );

    let next = list(
        "owner-local",
        "workspace-host-owner",
        Some(INCARNATION_ID.to_owned()),
        Some(RuntimeCapabilityHostPosition {
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            host_instance_id: "host-a".to_owned(),
        }),
        2,
    )
    .expect("cursor page reads");
    assert_eq!(
        next.iter()
            .map(|host| host.host_instance_id.as_str())
            .collect::<Vec<_>>(),
        ["host-b", "host-c"]
    );
    assert_eq!(next[1].health, "UNKNOWN");
    assert_eq!(next[1].state, "READY");

    assert!(matches!(
        list("another-owner", "workspace-host-owner", None, None, 10),
        Err(storage_core::StoreError::NotFound)
    ));
    assert!(matches!(
        list("owner-local", "workspace-host-unbound", None, None, 10),
        Err(storage_core::StoreError::NotFound)
    ));
    assert!(matches!(
        list(
            "owner-local",
            "workspace-host-owner",
            Some("old-incarnation".to_owned()),
            None,
            10
        ),
        Err(storage_core::StoreError::Conflict { .. })
    ));
}
