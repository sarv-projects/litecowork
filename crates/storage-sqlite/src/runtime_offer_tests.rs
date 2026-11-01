use super::{create_workspace, test_store};
use domain_workspace::{CreateWorkspace, WorkspaceService};
use sha2::{Digest, Sha256};
use std::time::Duration;
use storage_core::{
    AgentCatalogStore, AgentEndpointRecord, AgentProfileRecord, DeviceIdentityRecord,
    LocalAgentEndpointBindingInput, LocalRuntimeWorkspaceEnrollmentRequest,
    RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
    RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeOfferPageRequest,
    RuntimeOfferPosition, RuntimeOfferRecord, RuntimeRecord, RuntimeWorkspaceBindingStore,
    WorkspaceCreateRequest,
};

const OBSERVED: &str = "2026-10-10T10:00:00Z";
const NOW: &str = "2026-10-10T10:02:00Z";
const FUTURE: &str = "2026-10-10T10:03:00Z";
const EXPIRY: &str = "2026-10-10T10:10:00Z";

fn prepare_runtime(workspace_id: &str) -> (tempfile::TempDir, crate::SqliteWorkspaceStore, String) {
    let directory = tempfile::tempdir().expect("temporary SQLite directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let workspace = create_workspace(&store, workspace_id);
    let public_key = [7_u8; 32];
    let incarnation_id = "incarnation-offer-test";
    let runtime_id = "runtime-offer-test";
    let runtime = RuntimeRecord {
        runtime_id: runtime_id.to_owned(),
        device_identity: DeviceIdentityRecord {
            device_id: format!("ed25519-sha256:{}", hex::encode(Sha256::digest(public_key))),
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
        current_incarnation_id: incarnation_id.to_owned(),
        resource_capacity: serde_json::json!({"sampled_at": OBSERVED}),
        last_seen: OBSERVED.to_owned(),
    };
    let incarnation = RuntimeIncarnationRecord {
        runtime_incarnation_id: incarnation_id.to_owned(),
        runtime_id: runtime_id.to_owned(),
        process_started_at: OBSERVED.to_owned(),
        litecowork_version: "0.1.0-test".to_owned(),
        recovered_from_unclean_shutdown: false,
        recovery_state: "RECOVERING".to_owned(),
        ready_at: None,
        stopped_at: None,
        version: 1,
    };
    store
        .register_local_incarnation(
            runtime,
            incarnation,
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                os_boot_id: Some("boot-offer-test".to_owned()),
                observed_at: OBSERVED.to_owned(),
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
        .expect("Runtime becomes available for control-plane use");
    store
        .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: "owner-local".to_owned(),
                request_id: format!("enroll-{workspace_id}"),
                request_payload: serde_json::json!({"workspace_id": workspace_id}),
            },
            expected_workspace_version: workspace.workspace.version,
            runtime_workspace_binding_id: format!("binding-{workspace_id}"),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            now: NOW.to_owned(),
            correlation_id: format!("correlation-{workspace_id}"),
        })
        .expect("Runtime enrolls in Workspace");
    (directory, store, incarnation_id.to_owned())
}

fn publish_endpoint_offer(
    store: &crate::SqliteWorkspaceStore,
    incarnation_id: &str,
    endpoint_id: &str,
    observed_at: &str,
    expires_at: &str,
) {
    store
        .put_agent_profile(
            AgentProfileRecord {
                agent_profile_id: "profile-offer-test".to_owned(),
                provider_key: "test-provider".to_owned(),
                display_name: "Test provider".to_owned(),
                discovered_at: OBSERVED.to_owned(),
            },
            vec![AgentEndpointRecord {
                endpoint_id: endpoint_id.to_owned(),
                agent_profile_id: "profile-offer-test".to_owned(),
                protocol: "ACP".to_owned(),
                topology: "PROCESS_ADAPTER".to_owned(),
                protocol_version: Some("1".to_owned()),
                capabilities: serde_json::json!({"streaming": true}),
            }],
        )
        .expect("stable endpoint identity is stored");
    store
        .register_local_endpoint_binding(LocalAgentEndpointBindingInput {
            endpoint_id: endpoint_id.to_owned(),
            runtime_id: "runtime-offer-test".to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            endpoint_ref: format!("ipc:///private/{endpoint_id}-locator-secret"),
            observed_at: observed_at.to_owned(),
            expires_at: Some(expires_at.to_owned()),
        })
        .expect("Runtime keeps its private endpoint locator locally");
    store
        .publish_runtime_offer(RuntimeOfferRecord {
            runtime_id: "runtime-offer-test".to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            offer_kind: "AGENT_ENDPOINT".to_owned(),
            offer_ref: endpoint_id.to_owned(),
            compatible: true,
            readiness: "AVAILABLE".to_owned(),
            constraints: serde_json::json!({"test": true}),
            observed_at: observed_at.to_owned(),
            expires_at: expires_at.to_owned(),
        })
        .expect("Runtime offer is published");
}

fn list_offers(
    store: &crate::SqliteWorkspaceStore,
    owner: &str,
    workspace: &str,
    expected_incarnation_id: Option<String>,
    after: Option<RuntimeOfferPosition>,
    limit: usize,
) -> Result<Vec<RuntimeOfferRecord>, storage_core::StoreError> {
    store.list_runtime_offers(RuntimeOfferPageRequest {
        owner_principal_id: owner.to_owned(),
        workspace_id: workspace.to_owned(),
        runtime_id: "runtime-offer-test".to_owned(),
        expected_incarnation_id,
        now: NOW.to_owned(),
        after,
        limit,
    })
}

#[test]
fn runtime_offer_page_is_workspace_authorized_fresh_ordered_and_locator_free() {
    let (_directory, store, incarnation_id) = prepare_runtime("workspace-offer-test");
    publish_endpoint_offer(&store, &incarnation_id, "endpoint-c", OBSERVED, EXPIRY);
    publish_endpoint_offer(&store, &incarnation_id, "endpoint-a", OBSERVED, EXPIRY);
    publish_endpoint_offer(&store, &incarnation_id, "endpoint-expired", OBSERVED, NOW);
    publish_endpoint_offer(&store, &incarnation_id, "endpoint-future", FUTURE, EXPIRY);

    let first = list_offers(&store, "owner-local", "workspace-offer-test", None, None, 1)
        .expect("first authorized page is read");
    assert_eq!(
        first
            .iter()
            .map(|offer| offer.offer_ref.as_str())
            .collect::<Vec<_>>(),
        ["endpoint-a"]
    );
    assert!(
        !serde_json::to_string(&first)
            .expect("offer serialization")
            .contains("locator-secret")
    );
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: "workspace-unbound".to_owned(),
            name: "Unbound workspace".to_owned(),
            owner_principal_id: "owner-local".to_owned(),
            event: super::context("event-unbound-workspace", 3),
        })
        .expect("second active Workspace commits without a Runtime binding");

    let next = list_offers(
        &store,
        "owner-local",
        "workspace-offer-test",
        Some(incarnation_id.clone()),
        Some(RuntimeOfferPosition {
            runtime_incarnation_id: incarnation_id.clone(),
            offer_kind: first[0].offer_kind.clone(),
            offer_ref: first[0].offer_ref.clone(),
        }),
        2,
    )
    .expect("continuation page is read");
    assert_eq!(
        next.iter()
            .map(|offer| offer.offer_ref.as_str())
            .collect::<Vec<_>>(),
        ["endpoint-c"]
    );

    assert!(matches!(
        list_offers(
            &store,
            "another-owner",
            "workspace-offer-test",
            None,
            None,
            10
        ),
        Err(storage_core::StoreError::NotFound)
    ));
    assert!(matches!(
        list_offers(&store, "owner-local", "workspace-unbound", None, None, 10),
        Err(storage_core::StoreError::NotFound)
    ));
    assert!(matches!(
        list_offers(
            &store,
            "owner-local",
            "workspace-offer-test",
            Some("old-incarnation".to_owned()),
            None,
            10,
        ),
        Err(storage_core::StoreError::Conflict { .. })
    ));
}
