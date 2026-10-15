use super::*;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
    response::Response,
};
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use serde_json::{Value, json};
use sha2::Digest;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc, time::Duration};
use storage_core::{
    AgentEndpointRecord, AgentProfileRecord, BlobPurpose, DeviceIdentityRecord, EventStore,
    LocalAgentEndpointBindingInput, LocalRuntimeWorkspaceEnrollmentRequest,
    RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
    RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeOfferRecord, RuntimeRecord,
    RuntimeWorkspaceBindingStore, StoreError, WorkspaceCreateRequest,
};
use storage_sqlite::{
    FileBlobStore, RuntimeOsPrincipalIdentity, SqliteConfig, SqliteWorkspaceStore,
    WorkspaceBlobKey, WorkspaceBlobKeyProvider,
};
use tower::ServiceExt;
use zeroize::Zeroizing;

const WORKSPACE: &str = "agent-disable-route-workspace";
const OWNER: &str = "agent-disable-route-owner";
const FOREIGN_PRINCIPAL: &str = "agent-disable-route-foreign-owner";
const RUNTIME: &str = "agent-disable-route-runtime";
const PROFILE: &str = "agent-disable-route-profile";
const BINDING: &str = "agent-disable-route-binding";
const NOW: &str = "2026-10-15T12:00:00Z";

struct TestKeys;

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([71_u8; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob(
                "unknown Operator test key version".to_owned(),
            ));
        }
        self.current_key(workspace_id, purpose)
    }
}

struct Fixture {
    _directory: tempfile::TempDir,
    database: PathBuf,
    store: SqliteWorkspaceStore,
    owner_router: Router,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().expect("temporary AgentBinding route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private fixture directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
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
            name: "Agent binding disable route Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "agent-disable-route-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "agent-disable-route-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner Workspace commits");

    let connection = rusqlite::Connection::open(&database).expect("open fixture database");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at) VALUES (?1, 'test-provider', 'Test Agent', ?2)",
            rusqlite::params![PROFILE, NOW],
        )
        .expect("seed AgentProfile");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version) VALUES (?1, ?2, ?3, '{\"mode\":\"AUTO_COMPATIBLE\",\"required_features\":[],\"preferred_topologies\":[]}', '{}', 1, 1, ?4, 1)",
            rusqlite::params![BINDING, WORKSPACE, PROFILE, NOW],
        )
        .expect("seed enabled lead AgentBinding");
    drop(connection);

    Fixture {
        _directory: directory,
        database,
        store: store.clone(),
        owner_router: router_for_store(store.clone(), OWNER),
    }
}

fn seed_compatible_endpoint(fixture: &Fixture) {
    let public_key = [19_u8; 32];
    fixture
        .store
        .register_local_incarnation(
            RuntimeRecord {
                runtime_id: RUNTIME.to_owned(),
                device_identity: DeviceIdentityRecord {
                    device_id: format!(
                        "ed25519-sha256:{}",
                        hex::encode(sha2::Sha256::digest(public_key))
                    ),
                    public_key: format!("ed25519:{}", hex::encode(public_key)),
                    key_version: 1,
                    issued_at: NOW.to_owned(),
                    display_name: None,
                },
                runtime_version: "test".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                roles: vec!["OPERATOR_ENDPOINT".to_owned()],
                trust_zone: "PERSONAL_DEVICE".to_owned(),
                availability: "RECOVERING".to_owned(),
                startup_policy: "MANUAL".to_owned(),
                current_incarnation_id: "agent-disable-route-incarnation".to_owned(),
                resource_capacity: json!({"sampled_at":NOW}),
                last_seen: NOW.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: "agent-disable-route-incarnation".to_owned(),
                runtime_id: RUNTIME.to_owned(),
                process_started_at: NOW.to_owned(),
                litecowork_version: "test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: "agent-disable-route-incarnation".to_owned(),
                os_boot_id: Some("agent-disable-route-boot".to_owned()),
                observed_at: NOW.to_owned(),
            },
        )
        .expect("Runtime incarnation registers");
    fixture
        .store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: "agent-disable-route-incarnation".to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("Runtime is available for endpoint setup");

    let workspace = fixture
        .store
        .get_workspace(WORKSPACE)
        .expect("Workspace reads")
        .expect("Workspace exists");
    fixture
        .store
        .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: "agent-disable-route-runtime-enrollment".to_owned(),
                request_payload: json!({"workspace_id":WORKSPACE,"runtime_id":RUNTIME}),
            },
            expected_workspace_version: workspace.version,
            runtime_workspace_binding_id: "agent-disable-route-runtime-binding".to_owned(),
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: "agent-disable-route-incarnation".to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            now: NOW.to_owned(),
            correlation_id: "agent-disable-route-runtime-enrollment".to_owned(),
        })
        .expect("Runtime enrolls in the Workspace");

    fixture
        .store
        .put_agent_profile(
            AgentProfileRecord {
                agent_profile_id: PROFILE.to_owned(),
                provider_key: "test-provider".to_owned(),
                display_name: "Test Agent".to_owned(),
                discovered_at: NOW.to_owned(),
            },
            vec![AgentEndpointRecord {
                endpoint_id: "agent-disable-route-endpoint".to_owned(),
                agent_profile_id: PROFILE.to_owned(),
                protocol: "ACP".to_owned(),
                topology: "PROCESS_ADAPTER".to_owned(),
                protocol_version: Some("1".to_owned()),
                capabilities: json!({"streaming":true}),
            }],
        )
        .expect("Agent profile and endpoint are stored");
    fixture
        .store
        .register_local_endpoint_binding(LocalAgentEndpointBindingInput {
            endpoint_id: "agent-disable-route-endpoint".to_owned(),
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: "agent-disable-route-incarnation".to_owned(),
            endpoint_ref: "ipc:///private/agent-disable-route".to_owned(),
            observed_at: NOW.to_owned(),
            expires_at: Some("2099-10-15T12:00:00Z".to_owned()),
        })
        .expect("Runtime endpoint binding is stored");
    fixture
        .store
        .publish_runtime_offer(RuntimeOfferRecord {
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: "agent-disable-route-incarnation".to_owned(),
            offer_kind: "AGENT_ENDPOINT".to_owned(),
            offer_ref: "agent-disable-route-endpoint".to_owned(),
            compatible: true,
            readiness: "AVAILABLE".to_owned(),
            constraints: json!({"test":true}),
            observed_at: NOW.to_owned(),
            expires_at: "2099-10-15T12:00:00Z".to_owned(),
        })
        .expect("Runtime endpoint offer is stored");
}

fn router_for_store(store: SqliteWorkspaceStore, principal_id: &str) -> Router {
    let (_shutdown_sender, stream_shutdown) = tokio::sync::watch::channel(false);
    super::super::build_operator_router(super::super::ApiState {
        store,
        principal_id: principal_id.to_owned(),
        runtime_id: RUNTIME.to_owned(),
        local_incarnation_id: "agent-disable-route-incarnation".to_owned(),
        lifecycle_blockers: vec![],
        runtime_identity: Arc::new(
            RuntimeOsPrincipalIdentity::test_support(RUNTIME).expect("test identity"),
        ),
        stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
        runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        stream_shutdown,
    })
}

fn disable_request(
    workspace_id: &str,
    binding_id: &str,
    version: Option<&str>,
    key: Option<&str>,
) -> Request<Body> {
    let mut request = Request::post(format!("/v1/agent-bindings/{binding_id}/disable"))
        .header("x-workspace-id", workspace_id);
    if let Some(version) = version {
        request = request.header("if-match", version);
    }
    if let Some(key) = key {
        request = request.header("idempotency-key", key);
    }
    request
        .body(Body::empty())
        .expect("disable request is valid HTTP")
}

fn enable_request(
    workspace_id: &str,
    binding_id: &str,
    version: Option<&str>,
    key: Option<&str>,
) -> Request<Body> {
    let mut request = Request::post(format!("/v1/agent-bindings/{binding_id}/enable"))
        .header("x-workspace-id", workspace_id);
    if let Some(version) = version {
        request = request.header("if-match", version);
    }
    if let Some(key) = key {
        request = request.header("idempotency-key", key);
    }
    request
        .body(Body::empty())
        .expect("enable request is valid HTTP")
}

fn archive_request(workspace_id: &str, version: &str, key: &str) -> Request<Body> {
    Request::post(format!("/v1/workspaces/{workspace_id}/archive"))
        .header("x-workspace-id", workspace_id)
        .header("if-match", version)
        .header("idempotency-key", key)
        .body(Body::empty())
        .expect("Workspace archive request is valid HTTP")
}

fn default_binding_request(
    workspace_id: &str,
    version: &str,
    key: &str,
    binding: Value,
) -> Request<Body> {
    Request::patch(format!(
        "/v1/workspaces/{workspace_id}/default-agent-binding"
    ))
    .header("x-workspace-id", workspace_id)
    .header("if-match", version)
    .header("idempotency-key", key)
    .header(axum::http::header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({"agent_binding_id": binding}).to_string()))
    .expect("default AgentBinding request is valid HTTP")
}

async fn send(router: &Router, mut request: Request<Body>, authenticated: bool) -> Response {
    if authenticated {
        request
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
    }
    router
        .clone()
        .oneshot(request)
        .await
        .expect("Operator router responds")
}

async fn response_json(response: Response) -> Value {
    let bytes = to_bytes(response.into_body(), 16 * 1024)
        .await
        .expect("bounded Operator response body");
    serde_json::from_slice(&bytes).expect("Operator response is JSON")
}

fn event_count(store: &SqliteWorkspaceStore, event_type: &str) -> usize {
    store
        .read_workspace_events(WORKSPACE)
        .expect("Workspace events are readable")
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .count()
}

fn receipt_count(database: &PathBuf, principal_id: &str, request_id: &str) -> i64 {
    rusqlite::Connection::open(database)
        .expect("open receipt database")
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            rusqlite::params![principal_id, request_id],
            |row| row.get(0),
        )
        .expect("count command receipt")
}

fn receipt_request_digest(database: &PathBuf, principal_id: &str, request_id: &str) -> String {
    rusqlite::Connection::open(database)
        .expect("open receipt database")
        .query_row(
            "SELECT request_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            rusqlite::params![principal_id, request_id],
            |row| row.get(0),
        )
        .expect("read command receipt digest")
}

fn request_digest(payload: &Value) -> String {
    format!(
        "sha256:{}",
        hex::encode(sha2::Sha256::digest(
            serde_json_canonicalizer::to_vec(payload).expect("canonicalize request payload")
        ))
    )
}

#[tokio::test]
async fn disable_route_authenticates_and_requires_workspace_owner_and_mutation_headers() {
    let fixture = fixture();

    let unauthenticated = send(
        &fixture.owner_router,
        disable_request(
            WORKSPACE,
            BINDING,
            Some("1"),
            Some("disable-unauthenticated"),
        ),
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let foreign_workspace = send(
        &fixture.owner_router,
        disable_request(
            "foreign-workspace",
            BINDING,
            Some("1"),
            Some("disable-foreign-workspace"),
        ),
        true,
    )
    .await;
    assert_eq!(foreign_workspace.status(), StatusCode::FORBIDDEN);

    let non_owner_router = router_for_store(fixture.store.clone(), FOREIGN_PRINCIPAL);
    let non_owner = send(
        &non_owner_router,
        disable_request(WORKSPACE, BINDING, Some("1"), Some("disable-non-owner")),
        true,
    )
    .await;
    assert_eq!(non_owner.status(), StatusCode::FORBIDDEN);

    let missing_version = send(
        &fixture.owner_router,
        disable_request(WORKSPACE, BINDING, None, Some("disable-no-version")),
        true,
    )
    .await;
    assert_eq!(missing_version.status(), StatusCode::BAD_REQUEST);

    let missing_key = send(
        &fixture.owner_router,
        disable_request(WORKSPACE, BINDING, Some("1"), None),
        true,
    )
    .await;
    assert_eq!(missing_key.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn disable_route_commits_versioned_event_and_exact_replay_without_duplicate_receipts() {
    let fixture = fixture();
    let key = "disable-route-replay-key";
    let first = send(
        &fixture.owner_router,
        disable_request(WORKSPACE, BINDING, Some("1"), Some(key)),
        true,
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = response_json(first).await;
    assert_eq!(first_body["agent_binding_id"], BINDING);
    assert_eq!(first_body["workspace_id"], WORKSPACE);
    assert_eq!(first_body["enabled"], false);
    assert_eq!(first_body["version"], 2);

    let changed_events = fixture
        .store
        .read_workspace_events(WORKSPACE)
        .expect("read committed events")
        .into_iter()
        .filter(|event| event.event_type == "agent.binding.changed.v1")
        .collect::<Vec<_>>();
    assert_eq!(changed_events.len(), 1);
    assert_eq!(changed_events[0].payload["from_enabled"], true);
    assert_eq!(changed_events[0].payload["to_enabled"], false);
    assert_eq!(changed_events[0].payload["aggregate_version"], 2);
    let audit_record: (String, String, String, String, String, String, String) = rusqlite::Connection::open(&fixture.database)
        .expect("open audit database")
        .query_row(
            "SELECT principal_json, resource_ref_json, action, decision, reason_code, correlation_id, payload_digest FROM audit_records WHERE workspace_id = ?1 AND action = 'agent.binding.disable'",
            [WORKSPACE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
        )
        .expect("owner disable is audited with the event");
    assert_eq!(
        serde_json::from_str::<Value>(&audit_record.0).expect("principal JSON"),
        json!({"principal_id":OWNER,"kind":"USER"})
    );
    assert_eq!(
        serde_json::from_str::<Value>(&audit_record.1).expect("resource JSON"),
        json!({"kind":"AGENT_BINDING","workspace_id":WORKSPACE,"agent_binding_id":BINDING,"agent_profile_id":PROFILE})
    );
    assert_eq!(audit_record.2, "agent.binding.disable");
    assert_eq!(audit_record.3, "ALLOW");
    assert_eq!(audit_record.4, "OWNER_REQUESTED_AGENT_BINDING_DISABLE");
    assert_eq!(audit_record.5, changed_events[0].correlation_id);
    assert!(audit_record.6.starts_with("sha256:"));
    assert_eq!(receipt_count(&fixture.database, OWNER, key), 1);
    assert_eq!(
        receipt_request_digest(&fixture.database, OWNER, key),
        request_digest(&json!({
            "operation": "agent.binding.disable.v1",
            "workspace_id": WORKSPACE,
            "agent_binding_id": BINDING,
            "expected_version": 1,
        })),
        "the durable idempotency identity must name the disable operation"
    );

    let replay = send(
        &fixture.owner_router,
        disable_request(WORKSPACE, BINDING, Some("1"), Some(key)),
        true,
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(response_json(replay).await, first_body);
    assert_eq!(event_count(&fixture.store, "agent.binding.changed.v1"), 1);
    assert_eq!(receipt_count(&fixture.database, OWNER, key), 1);
    let audit_count: i64 = rusqlite::Connection::open(&fixture.database)
        .expect("reopen audit database")
        .query_row("SELECT COUNT(*) FROM audit_records WHERE workspace_id = ?1 AND action = 'agent.binding.disable'", [WORKSPACE], |row| row.get(0))
        .expect("count disable audit records");
    assert_eq!(
        audit_count, 1,
        "replaying does not duplicate the AuditRecord"
    );

    let altered_payload = send(
        &fixture.owner_router,
        disable_request(WORKSPACE, BINDING, Some("2"), Some(key)),
        true,
    )
    .await;
    assert_eq!(altered_payload.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(altered_payload).await["code"],
        "CONFLICT",
        "If-Match is part of the idempotent request payload"
    );
    assert_eq!(event_count(&fixture.store, "agent.binding.changed.v1"), 1);
    assert_eq!(
        audit_count, 1,
        "stale or changed-payload requests add no AuditRecord"
    );
    assert_eq!(receipt_count(&fixture.database, OWNER, key), 1);

    let stale = send(
        &fixture.owner_router,
        disable_request(
            WORKSPACE,
            BINDING,
            Some("1"),
            Some("disable-route-stale-key"),
        ),
        true,
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(stale).await["code"],
        "CONFLICT",
        "a stale AgentBinding version is a conflict"
    );
    assert_eq!(event_count(&fixture.store, "agent.binding.changed.v1"), 1);
    assert_eq!(
        receipt_count(&fixture.database, OWNER, "disable-route-stale-key"),
        0
    );
}

#[tokio::test]
async fn disable_exact_receipt_replays_after_workspace_archive_but_fresh_mutations_are_rejected() {
    let fixture = fixture();
    let disable_key = "disable-route-replay-after-archive";
    let first = send(
        &fixture.owner_router,
        disable_request(WORKSPACE, BINDING, Some("1"), Some(disable_key)),
        true,
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = response_json(first).await;

    let archived = send(
        &fixture.owner_router,
        archive_request(WORKSPACE, "1", "archive-after-agent-binding-disable"),
        true,
    )
    .await;
    assert_eq!(archived.status(), StatusCode::OK);
    assert_eq!(response_json(archived).await["status"], "ARCHIVED");

    let replay = send(
        &fixture.owner_router,
        disable_request(WORKSPACE, BINDING, Some("1"), Some(disable_key)),
        true,
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(response_json(replay).await, first_body);
    assert_eq!(event_count(&fixture.store, "agent.binding.changed.v1"), 1);
    assert_eq!(receipt_count(&fixture.database, OWNER, disable_key), 1);
    let audits: i64 = rusqlite::Connection::open(&fixture.database)
        .expect("open audit database")
        .query_row(
            "SELECT COUNT(*) FROM audit_records WHERE workspace_id = ?1 AND action = 'agent.binding.disable'",
            [WORKSPACE],
            |row| row.get(0),
        )
        .expect("count disable audit records");
    assert_eq!(audits, 1);

    let fresh_mutation = send(
        &fixture.owner_router,
        disable_request(
            WORKSPACE,
            BINDING,
            Some("2"),
            Some("fresh-disable-after-archive"),
        ),
        true,
    )
    .await;
    assert_eq!(fresh_mutation.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        receipt_count(&fixture.database, OWNER, "fresh-disable-after-archive"),
        0,
        "archival blocks a fresh mutation without creating a receipt"
    );
}

#[tokio::test]
async fn enable_exact_receipt_replays_after_workspace_archive_but_fresh_mutations_are_rejected() {
    let fixture = fixture();
    seed_compatible_endpoint(&fixture);
    let disabled = send(
        &fixture.owner_router,
        disable_request(
            WORKSPACE,
            BINDING,
            Some("1"),
            Some("disable-before-enable-archive"),
        ),
        true,
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    assert_eq!(response_json(disabled).await["version"], 2);

    let enable_key = "enable-route-replay-after-archive";
    let first = send(
        &fixture.owner_router,
        enable_request(WORKSPACE, BINDING, Some("2"), Some(enable_key)),
        true,
    )
    .await;
    let first_status = first.status();
    let first_body = response_json(first).await;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "enable response: {first_body}"
    );
    assert_eq!(first_body["enabled"], true);
    assert_eq!(first_body["version"], 3);
    assert_eq!(
        receipt_request_digest(&fixture.database, OWNER, enable_key),
        request_digest(&json!({
            "operation": "agent.binding.enable.v1",
            "workspace_id": WORKSPACE,
            "agent_binding_id": BINDING,
            "expected_version": 2,
        })),
        "the durable idempotency identity must name the enable operation"
    );

    let archived = send(
        &fixture.owner_router,
        archive_request(WORKSPACE, "1", "archive-after-agent-binding-enable"),
        true,
    )
    .await;
    assert_eq!(archived.status(), StatusCode::OK);
    assert_eq!(response_json(archived).await["status"], "ARCHIVED");

    let replay = send(
        &fixture.owner_router,
        enable_request(WORKSPACE, BINDING, Some("2"), Some(enable_key)),
        true,
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(response_json(replay).await, first_body);
    assert_eq!(event_count(&fixture.store, "agent.binding.changed.v1"), 2);
    assert_eq!(receipt_count(&fixture.database, OWNER, enable_key), 1);

    let fresh_mutation = send(
        &fixture.owner_router,
        enable_request(
            WORKSPACE,
            BINDING,
            Some("3"),
            Some("fresh-enable-after-archive"),
        ),
        true,
    )
    .await;
    assert_eq!(fresh_mutation.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        receipt_count(&fixture.database, OWNER, "fresh-enable-after-archive"),
        0,
        "archival blocks a fresh enable without creating a receipt"
    );
}

fn seed_active_pinned_session(database: &PathBuf) {
    let mut connection =
        rusqlite::Connection::open(database).expect("open session fixture database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable fixture foreign keys");
    let tx = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("begin session fixture transaction");
    tx.execute_batch("PRAGMA defer_foreign_keys=ON;")
        .expect("defer cyclic Task and Runtime references");
    tx.execute(
        "INSERT INTO runtimes(runtime_id, device_identity_json, runtime_version, platform, architecture, roles_json, trust_zone, availability, startup_policy, current_incarnation_id, resource_capacity_json, last_seen, version) VALUES ('agent-disable-session-runtime', '{}', 'test', 'linux', 'x86_64', '[\"OPERATOR_ENDPOINT\"]', 'PERSONAL_DEVICE', 'ONLINE', 'MANUAL', 'agent-disable-session-incarnation', '{}', ?1, 1)",
        [NOW],
    )
    .expect("seed Runtime");
    tx.execute(
        "INSERT INTO runtime_incarnations(runtime_incarnation_id, runtime_id, process_started_at, litecowork_version, recovered_from_unclean_shutdown, recovery_state, ready_at, version) VALUES ('agent-disable-session-incarnation', 'agent-disable-session-runtime', ?1, 'test', 0, 'READY', ?1, 1)",
        [NOW],
    )
    .expect("seed Runtime incarnation");
    tx.execute(
        "INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status, lead_agent_binding_id, blocking_conditions_json, priority, created_by_json, created_at, updated_at, version) VALUES ('agent-disable-session-task', ?1, 1, 'RUNNING', ?2, '[]', 'NORMAL', ?3, ?4, ?4, 1)",
        rusqlite::params![WORKSPACE, BINDING, json!({"kind":"USER","principal_id":OWNER}).to_string(), NOW],
    )
    .expect("seed Task pinned to the AgentBinding");
    tx.execute(
        "INSERT INTO task_spec_revisions(task_id, workspace_id, revision, objective, lead_failover_policy_json, placement_preference, preferred_lead_agent_binding_id, authored_by_json, created_at) VALUES ('agent-disable-session-task', ?1, 1, 'Keep this admitted session pinned', '{\"mode\":\"DISABLED\",\"triggers\":[],\"fallback_agent_binding_ids\":[],\"max_lead_changes\":0}', '{}', ?2, ?3, ?4)",
        rusqlite::params![WORKSPACE, BINDING, json!({"kind":"USER","principal_id":OWNER}).to_string(), NOW],
    )
    .expect("seed immutable TaskSpec revision");
    tx.execute(
        "INSERT INTO agent_sessions(agent_session_id, workspace_id, scope_kind, task_id, task_spec_revision, agent_binding_id, endpoint_id, runtime_id, runtime_incarnation_id, configuration_digest, status, started_at, last_event_at, version) VALUES ('agent-disable-pinned-session', ?1, 'TASK_PLANNING', 'agent-disable-session-task', 1, ?2, 'agent-disable-session-endpoint', 'agent-disable-session-runtime', 'agent-disable-session-incarnation', NULL, 'ACTIVE', ?3, ?3, 1)",
        rusqlite::params![WORKSPACE, BINDING, NOW],
    )
    .expect("seed admitted ACTIVE AgentSession");
    tx.commit().expect("persist valid pinned session fixture");
}

#[tokio::test]
async fn default_binding_must_be_cleared_before_disable_and_existing_session_stays_pinned() {
    let fixture = fixture();
    seed_active_pinned_session(&fixture.database);
    let connection =
        rusqlite::Connection::open(&fixture.database).expect("open config fixture database");
    connection
        .execute(
            "UPDATE agent_bindings SET configuration_json=?1 WHERE workspace_id=?2 AND agent_binding_id=?3",
            rusqlite::params![json!({"legacy_non_secret_option":"preserve"}).to_string(), WORKSPACE, BINDING],
        )
        .expect("set persisted non-secret binding configuration for preservation check");
    drop(connection);

    let selected_default = send(
        &fixture.owner_router,
        default_binding_request(WORKSPACE, "1", "disable-route-set-default", json!(BINDING)),
        true,
    )
    .await;
    assert_eq!(selected_default.status(), StatusCode::OK);

    let before_rejected_disable_events = fixture
        .store
        .read_workspace_events(WORKSPACE)
        .expect("read events before default conflict")
        .len();
    let rejected = send(
        &fixture.owner_router,
        disable_request(
            WORKSPACE,
            BINDING,
            Some("1"),
            Some("disable-route-default-conflict"),
        ),
        true,
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(rejected).await["code"],
        "CONFLICT",
        "the owner must clear the current Workspace default explicitly first"
    );
    assert_eq!(
        fixture
            .store
            .read_workspace_events(WORKSPACE)
            .expect("read events after rejected disable")
            .len(),
        before_rejected_disable_events,
        "a rejected default-binding disable writes no event"
    );
    assert_eq!(
        receipt_count(&fixture.database, OWNER, "disable-route-default-conflict"),
        0,
        "a rejected default-binding disable writes no idempotency receipt"
    );
    let before = fixture
        .store
        .get_agent_binding(OWNER, WORKSPACE, BINDING)
        .expect("binding read")
        .expect("binding exists");
    assert!(before.enabled);
    assert_eq!(before.version, 1);

    let cleared_default = send(
        &fixture.owner_router,
        default_binding_request(WORKSPACE, "2", "disable-route-clear-default", Value::Null),
        true,
    )
    .await;
    assert_eq!(cleared_default.status(), StatusCode::OK);
    assert_eq!(
        response_json(cleared_default).await["default_agent_binding_id"],
        Value::Null
    );

    let disabled = send(
        &fixture.owner_router,
        disable_request(
            WORKSPACE,
            BINDING,
            Some("1"),
            Some("disable-route-after-clear"),
        ),
        true,
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    let disabled_body = response_json(disabled).await;
    assert_eq!(disabled_body["enabled"], false);
    assert_eq!(disabled_body["lead_eligible"], true);
    assert_eq!(
        disabled_body["configuration"]["legacy_non_secret_option"],
        "preserve"
    );
    assert_eq!(disabled_body["version"], 2);

    let connection =
        rusqlite::Connection::open(&fixture.database).expect("open pinned session database");
    let session: (String, String, String, String) = connection
        .query_row(
            "SELECT status, agent_binding_id, runtime_id, runtime_incarnation_id FROM agent_sessions WHERE agent_session_id='agent-disable-pinned-session'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("existing active AgentSession remains present");
    assert_eq!(session.0, "ACTIVE");
    assert_eq!(session.1, BINDING);
    assert_eq!(session.2, "agent-disable-session-runtime");
    assert_eq!(session.3, "agent-disable-session-incarnation");
    assert_eq!(event_count(&fixture.store, "agent.binding.changed.v1"), 1);
    assert_eq!(
        receipt_count(&fixture.database, OWNER, "disable-route-after-clear"),
        1
    );
}
