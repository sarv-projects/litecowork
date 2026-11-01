use super::*;
use operator_ipc::unix::UnixEndpoint;
use std::os::unix::fs::MetadataExt;

async fn send_page(
    endpoint: &UnixEndpoint,
    expected_uid: u32,
    request_id: &str,
    path_and_query: String,
    workspace_id: &str,
) -> operator_ipc::ResponseFrame {
    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer authenticates");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            method: "GET".to_owned(),
            path_and_query,
            headers: vec![LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: workspace_id.to_owned(),
            }],
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("capability host request is bounded");
    client
        .write_request(&request)
        .await
        .expect("request frame writes");
    client
        .read_response(&client_budget)
        .await
        .expect("response frame is correlated")
}

#[tokio::test]
async fn authenticated_unix_capability_host_route_pages_public_workspace_view() {
    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = std::fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            IpcReplayTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    let runtime_id = "runtime-ipc-capability-hosts";
    let incarnation_id = "incarnation-ipc-capability-hosts";
    let workspace_id = "workspace-ipc-capability-hosts";
    let owner_id = "ipc-capability-host-owner";
    let now = "2026-10-12T10:00:00Z";
    let public_key = [13_u8; 32];
    store
        .register_local_incarnation(
            RuntimeRecord {
                runtime_id: runtime_id.to_owned(),
                device_identity: DeviceIdentityRecord {
                    device_id: format!(
                        "ed25519-sha256:{}",
                        hex::encode(sha2::Sha256::digest(public_key))
                    ),
                    public_key: format!("ed25519:{}", hex::encode(public_key)),
                    key_version: 1,
                    issued_at: now.to_owned(),
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
                resource_capacity: json!({"sampled_at": now}),
                last_seen: now.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                runtime_id: runtime_id.to_owned(),
                process_started_at: now.to_owned(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                os_boot_id: Some("boot-ipc-capability-hosts".to_owned()),
                observed_at: now.to_owned(),
            },
        )
        .expect("Runtime registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: now.to_owned(),
            stopped_at: None,
        })
        .expect("Runtime becomes available");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "IPC Capability hosts fixture".to_owned(),
            owner_principal_id: owner_id.to_owned(),
            event: EventContext {
                event_id: "ipc-capability-host-workspace-event".to_owned(),
                origin_runtime_id: runtime_id.to_owned(),
                hlc_timestamp: now.to_owned(),
                correlation_id: "ipc-capability-host-workspace-create".to_owned(),
                causation_id: None,
                recorded_at: now.to_owned(),
            },
        })
        .expect("Workspace commits");
    store
        .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: owner_id.to_owned(),
                request_id: "ipc-capability-host-enroll".to_owned(),
                request_payload: json!({"workspace_id": workspace_id}),
            },
            expected_workspace_version: 1,
            runtime_workspace_binding_id: "binding-ipc-capability-hosts".to_owned(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            now: now.to_owned(),
            correlation_id: "ipc-capability-host-enroll-correlation".to_owned(),
        })
        .expect("Workspace grants the local Runtime binding");

    let mut connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("fixture database opens");
    let transaction = connection
        .transaction()
        .expect("host fixture transaction starts");
    for host_id in ["host-ipc-a", "host-ipc-b"] {
        transaction
            .execute(
                "INSERT INTO capability_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,capability_ref_json,capability_identity_digest,configuration_digest,isolation_partition_digest,sharing_policy,hosting_mode,provider_instance_ref,state,health,observed_at,expires_at,version) VALUES(?1,?2,?3,?4,?5,?6,?7,'TASK_ISOLATED','LOCAL_MANAGED',?8,'READY','HEALTHY',?9,?10,1)",
                rusqlite::params![
                    host_id,
                    runtime_id,
                    incarnation_id,
                    json!({
                        "capability_id": "capability.search",
                        "identity_kind": "PACKAGE_COMPONENT",
                        "source": "registry",
                        "package_version": "1.0.0",
                        "digest": format!("sha256:{}", "a".repeat(64)),
                        "component": "search"
                    })
                    .to_string(),
                    format!(
                        "sha256:{}",
                        hex::encode(sha2::Sha256::digest(format!("{host_id}:identity").as_bytes()))
                    ),
                    format!("sha256:{}", "c".repeat(64)),
                    format!(
                        "sha256:{}",
                        hex::encode(sha2::Sha256::digest(format!("{host_id}:partition").as_bytes()))
                    ),
                    format!("provider://private/{host_id}-locator-secret"),
                    now,
                    "2026-10-12T11:00:00Z",
                ],
            )
            .expect("host observation seeds");
    }
    transaction.commit().expect("host fixture commits");

    let router = super::super::runtime_capability_hosts_operator::runtime_capability_hosts_router(
        super::super::runtime_capability_hosts_operator::RuntimeCapabilityHostsApiState::new(
            store,
            owner_id.to_owned(),
        ),
    );
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let connection = listener
                .accept(|peer| peer.uid == expected_uid)
                .await
                .expect("same-UID peer is accepted");
            dispatch_ipc_exchange(
                router.clone(),
                OperatorIpcConnection::Unix(connection),
                server_budget.clone(),
                Duration::from_secs(2),
            )
            .await
            .expect("capability host response is framed");
        }
    });

    let first = send_page(
        &endpoint,
        expected_uid,
        "capability-host-page-one",
        format!("/v1/runtimes/{runtime_id}/capability-hosts?limit=1"),
        workspace_id,
    )
    .await;
    assert_eq!(first.header.status, StatusCode::OK.as_u16());
    assert!(
        first
            .header
            .headers
            .iter()
            .any(|header| { header.name == "cache-control" && header.value == "no-store" })
    );
    let first_body: Value = serde_json::from_slice(first.body()).expect("first page JSON");
    assert_eq!(first_body["items"][0]["host_instance_id"], "host-ipc-a");
    assert_eq!(first_body["items"][0]["active_activation_count"], 0);
    let cursor = first_body["next_cursor"]
        .as_str()
        .expect("first page has a continuation cursor");
    assert!(!first_body.to_string().contains("locator-secret"));

    let second = send_page(
        &endpoint,
        expected_uid,
        "capability-host-page-two",
        format!("/v1/runtimes/{runtime_id}/capability-hosts?limit=1&cursor={cursor}"),
        workspace_id,
    )
    .await;
    assert_eq!(second.header.status, StatusCode::OK.as_u16());
    let second_body: Value = serde_json::from_slice(second.body()).expect("second page JSON");
    assert_eq!(second_body["items"][0]["host_instance_id"], "host-ipc-b");
    assert!(second_body["next_cursor"].is_null());
    assert!(!second_body.to_string().contains("locator-secret"));
    server.await.expect("capability host IPC requests complete");
}
