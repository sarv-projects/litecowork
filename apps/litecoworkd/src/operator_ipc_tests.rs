use super::dependency_invalidations_operator::{
    DependencyInvalidationsApiState, test_router as dependency_invalidations_test_router,
};
use super::presentation_operator::{StreamApiState, subscribe_stream};
#[path = "resource_dependencies_operator_tests.rs"]
mod resource_dependencies_operator_tests;
#[path = "runtime_capability_hosts_tests.rs"]
mod runtime_capability_hosts_tests;
use super::runtime_offers_operator::{RuntimeOffersApiState, runtime_offers_router};
use super::{
    AuthenticatedLocalPeer, OperatorIpcConnection, OperatorRequestSettlement,
    RuntimeLifecycleApiState, RuntimeLifecycleBlocker, TaskCreateApiState, UnixTransportError,
    WorkspaceArchiveApiState, WorkspaceCreateApiState, WorkspaceInstructionApiState,
    WorkspacePolicyApiState, authenticate, dispatch_ipc_exchange,
    is_expected_unix_stream_write_disconnect, runtime_lifecycle_router, workspace_archive_routes,
    workspace_instruction_routes, workspace_policy_routes,
};
use axum::{
    Json, Router,
    body::{Body, Bytes, to_bytes},
    http::{HeaderMap, HeaderValue, Request, StatusCode, header},
    middleware,
    response::Response,
    routing::{get, post},
};
use base64::Engine;
use domain_workspace::{ArchiveWorkspace, CreateWorkspace, EventContext, WorkspaceService};
use http_body::Body as HttpBody;
use operator_ipc::{
    DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, InFlightBodyBudget, LogicalHeader, MAX_STREAM_FRAME_BYTES,
    PROTOCOL_VERSION, RequestFrame, RequestHeader, unix::UnixEndpoint,
};
use serde_json::{Value, json};
use sha2::Digest;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use storage_core::{
    AgentCatalogStore, BlobPurpose, BlobStore, ConversationRecord, DeviceIdentityRecord,
    EventStore, LocalAgentEndpointBindingInput, LocalRuntimeWorkspaceEnrollmentRequest,
    RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
    RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeOfferRecord, RuntimeRecord,
    RuntimeWorkspaceBindingStore, StateStore, StoreError, TaskStore, WorkspaceCreateRequest,
};
use storage_sqlite::{
    FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider,
};
use tower::ServiceExt;
use zeroize::Zeroizing;

#[test]
fn runtime_inventory_revision_stops_at_json_safe_integer_limit() {
    let revision = std::sync::atomic::AtomicU64::new(super::MAX_SAFE_JSON_INTEGER - 1);
    super::advance_runtime_inventory_revision(&revision).expect("last safe revision advances");
    assert_eq!(
        revision.load(std::sync::atomic::Ordering::Acquire),
        super::MAX_SAFE_JSON_INTEGER
    );
    assert!(super::advance_runtime_inventory_revision(&revision).is_err());
    assert_eq!(
        revision.load(std::sync::atomic::Ordering::Acquire),
        super::MAX_SAFE_JSON_INTEGER
    );
}

#[test]
fn operator_idempotency_key_matches_the_canonical_200_byte_bound() {
    for length in [129, 200] {
        let key = "k".repeat(length);
        let mut headers = HeaderMap::new();
        headers.insert(
            "idempotency-key",
            HeaderValue::from_str(&key).expect("printable key is a valid HTTP header"),
        );
        assert_eq!(
            super::idempotency_key(&headers).expect("canonical-length key is accepted"),
            key
        );
    }

    let mut too_long = HeaderMap::new();
    too_long.insert(
        "idempotency-key",
        HeaderValue::from_str(&"k".repeat(201)).expect("printable key is a valid HTTP header"),
    );
    assert!(super::idempotency_key(&too_long).is_err());

    let mut malformed = HeaderMap::new();
    malformed.insert(
        "idempotency-key",
        HeaderValue::from_static("key with spaces"),
    );
    assert!(super::idempotency_key(&malformed).is_err());
}

#[test]
fn task_spec_keeps_its_workspace_instruction_pin_after_workspace_advances() {
    use domain_task::{CreateStandaloneTask, TaskService};
    use storage_core::{StateStore, TaskStore};

    const OWNER: &str = "instruction-pin-owner";
    const WORKSPACE: &str = "instruction-pin-workspace";
    const TASK: &str = "instruction-pin-task";
    const BINDING: &str = "instruction-pin-binding";
    const NOW: &str = "2026-10-11T12:00:00.000000000Z";

    let directory = tempfile::tempdir().expect("temporary instruction-pin data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private data directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
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
    let event = |event_id: &str, timestamp: &str| EventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: "instruction-pin-runtime".to_owned(),
        hlc_timestamp: timestamp.to_owned(),
        correlation_id: format!("correlation-{event_id}"),
        causation_id: None,
        recorded_at: timestamp.to_owned(),
    };
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Instruction pin fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: event("instruction-pin-workspace-created", NOW),
        })
        .expect("Workspace commits");

    let connection = rusqlite::Connection::open(&database).expect("fixture SQLite connection");
    connection
        .execute(
            "INSERT INTO workspace_instruction_revisions(workspace_id, revision, parent_revisions_json, content_ref_json, content_digest, authored_by_json, created_at)
             VALUES (?1, 1, '[]', ?2, ?3, ?4, ?5)",
            rusqlite::params![
                WORKSPACE,
                json!({"workspace_id": WORKSPACE, "resource_id": "instruction-resource-1", "revision_id": "instruction-resource-revision-1"}).to_string(),
                format!("sha256:{}", "a".repeat(64)),
                json!({"kind": "USER", "principal_id": OWNER}).to_string(),
                NOW,
            ],
        )
        .expect("seed first immutable instruction revision");
    connection
        .execute(
            "UPDATE workspaces SET current_instruction_revision = 1, version = 2, updated_at = ?1 WHERE workspace_id = ?2",
            rusqlite::params![NOW, WORKSPACE],
        )
        .expect("select first Workspace instruction revision");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at) VALUES ('instruction-pin-profile', 'fixture', 'Fixture Agent', ?1)",
            [NOW],
        )
        .expect("AgentProfile persists");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version) VALUES (?1, ?2, 'instruction-pin-profile', '{}', '{}', 1, 1, ?3, 1)",
            rusqlite::params![BINDING, WORKSPACE, NOW],
        )
        .expect("AgentBinding persists");
    drop(connection);

    let created = TaskService::new(store.clone())
        .create_standalone(CreateStandaloneTask {
            task_id: TASK.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            workspace_instruction_revision: Some(1),
            lead_agent_binding_id: BINDING.to_owned(),
            origin_coworker_id: None,
            origin_coworker_revision: None,
            expected_coworker_version: None,
            coworker_default_lead_failover_policy: None,
            principal_id: OWNER.to_owned(),
            request_id: "instruction-pin-task-create".to_owned(),
            request_payload: json!({
                "workspace_id": WORKSPACE,
                "objective": "Keep the instruction revision used at creation",
                "preferred_lead_agent_binding_id": BINDING,
                "source_message_refs": [],
                "constraints": [],
                "non_goals": [],
                "input_refs": [],
                "required_outputs": [],
                "acceptance_criteria": [],
                "approvals_required": [],
                "placement_preference": "AUTO"
            }),
            event: event(
                "instruction-pin-task-created",
                "2026-10-11T12:00:01.000000000Z",
            ),
        })
        .expect("Task creation pins the current Workspace instruction revision");
    assert_eq!(
        created
            .view
            .current_spec_revision
            .workspace_instruction_revision,
        Some(1)
    );

    let connection = rusqlite::Connection::open(&database).expect("fixture SQLite connection");
    connection
        .execute(
            "INSERT INTO workspace_instruction_revisions(workspace_id, revision, parent_revisions_json, content_ref_json, content_digest, authored_by_json, created_at)
             VALUES (?1, 2, '[1]', ?2, ?3, ?4, '2026-10-11T12:00:02.000000000Z')",
            rusqlite::params![
                WORKSPACE,
                json!({"workspace_id": WORKSPACE, "resource_id": "instruction-resource-2", "revision_id": "instruction-resource-revision-2"}).to_string(),
                format!("sha256:{}", "b".repeat(64)),
                json!({"kind": "USER", "principal_id": OWNER}).to_string(),
            ],
        )
        .expect("seed second immutable instruction revision");
    connection
        .execute(
            "UPDATE workspaces SET current_instruction_revision = 2, version = 3, updated_at = '2026-10-11T12:00:02.000000000Z' WHERE workspace_id = ?1",
            [WORKSPACE],
        )
        .expect("advance Workspace instructions after Task creation");
    drop(connection);

    let persisted = store
        .get_task(WORKSPACE, TASK)
        .expect("read persisted Task")
        .expect("Task remains stored");
    assert_eq!(
        persisted
            .current_spec_revision
            .workspace_instruction_revision,
        Some(1)
    );
    assert_eq!(
        store
            .get_workspace(WORKSPACE)
            .unwrap()
            .unwrap()
            .current_instruction_revision,
        Some(2)
    );
}

#[derive(Clone)]
struct IpcReplayTestKeys;

impl WorkspaceBlobKeyProvider for IpcReplayTestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([53_u8; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob("unknown IPC test key version".to_owned()));
        }
        self.current_key(workspace_id, purpose)
    }
}

fn readiness_router() -> Router {
    Router::new()
        .route(
            "/v1/operator/readiness",
            get(|| async { Json(json!({"operator_state": "SERVING"})) }),
        )
        .route_layer(middleware::from_fn(authenticate))
}

#[tokio::test]
async fn local_ipc_peer_marker_is_required_by_the_operator_authentication_middleware() {
    let unauthenticated = readiness_router()
        .oneshot(
            Request::get("/v1/operator/readiness")
                .body(Body::empty())
                .expect("request is valid"),
        )
        .await
        .expect("router returns a response");
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    // The marker is an internal extension, not a header. This branch models the
    // only trusted source: dispatch_ipc_exchange inserts it after the socket layer
    // has authenticated the kernel-reported peer credentials.
    let mut request = Request::get("/v1/operator/readiness")
        .body(Body::empty())
        .expect("request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let authenticated = readiness_router()
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(authenticated.status(), StatusCode::OK);
}

#[tokio::test]
async fn instruction_history_rejects_a_mismatched_workspace_header() {
    let directory = tempfile::tempdir().expect("temporary instruction route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
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
    let router = workspace_instruction_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceInstructionApiState {
            store,
            principal_id: "instruction-route-owner".to_owned(),
            runtime_id: "instruction-route-runtime".to_owned(),
        });
    let mut request = Request::get("/v1/workspaces/path-workspace/instructions/revisions?limit=10")
        .header("x-workspace-id", "header-workspace")
        .body(Body::empty())
        .expect("request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);

    let response = router
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(response.into_body(), 1024)
        .await
        .expect("bounded error response");
    let body: Value = serde_json::from_slice(&body).expect("structured error body");
    assert_eq!(body["code"], "FORBIDDEN");
    assert_eq!(body["message"], "Workspace access is unavailable");
}

#[tokio::test]
async fn instruction_history_does_not_disclose_another_owners_workspace() {
    let directory = tempfile::tempdir().expect("temporary instruction owner-scope directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
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
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: "private-instruction-workspace".to_owned(),
            name: "Private Workspace".to_owned(),
            owner_principal_id: "workspace-owner".to_owned(),
            event: EventContext {
                event_id: "private-workspace-created".to_owned(),
                origin_runtime_id: "instruction-route-runtime".to_owned(),
                hlc_timestamp: "2026-10-11T12:00:00Z".to_owned(),
                correlation_id: "private-workspace-create".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-11T12:00:00Z".to_owned(),
            },
        })
        .expect("Workspace belongs to another owner");
    let router = workspace_instruction_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceInstructionApiState {
            store,
            principal_id: "different-owner".to_owned(),
            runtime_id: "instruction-route-runtime".to_owned(),
        });
    let mut request = Request::get(
        "/v1/workspaces/private-instruction-workspace/instructions/revisions?limit=10",
    )
    .header("x-workspace-id", "private-instruction-workspace")
    .body(Body::empty())
    .expect("request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);

    let response = router
        .oneshot(request)
        .await
        .expect("router returns a response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(response.into_body(), 1024)
        .await
        .expect("bounded error response");
    let body: Value = serde_json::from_slice(&body).expect("structured error body");
    assert_eq!(body["code"], "FORBIDDEN");
    assert_eq!(body["message"], "Workspace access is unavailable");
}

#[tokio::test]
async fn dependency_invalidation_history_is_scoped_paginated_and_closed() {
    const WORKSPACE: &str = "invalidation-route-workspace";
    const OWNER: &str = "invalidation-route-owner";
    const NOW: &str = "2026-10-12T12:00:00Z";
    let directory = tempfile::tempdir().expect("temporary invalidation route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
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
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Invalidation route fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "invalidation-workspace-created".to_owned(),
                origin_runtime_id: "invalidation-route-runtime".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "invalidation-workspace-create".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: "invalidation-route-other-workspace".to_owned(),
            name: "Other owned Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "invalidation-other-workspace-created".to_owned(),
                origin_runtime_id: "invalidation-route-runtime".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "invalidation-other-workspace-create".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("second Workspace belongs to the same owner");
    let connection = rusqlite::Connection::open(&database).expect("fixture SQLite connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable fixture constraints");
    let transaction = connection
        .unchecked_transaction()
        .expect("fixture transaction");
    transaction.execute(
        "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('route-source', ?1, 'CONNECTOR_OBJECT', '{}', 'Source', 'route-source-r3', 'PERSONAL', '{}', ?2, ?2, 3)",
        rusqlite::params![WORKSPACE, NOW],
    ).expect("source Resource");
    for (revision, timestamp) in [
        ("route-source-r1", "2026-10-12T12:00:00Z"),
        ("route-source-r2", "2026-10-12T12:00:01Z"),
        ("route-source-r3", "2026-10-12T12:00:02Z"),
    ] {
        transaction.execute(
            "INSERT INTO resource_revisions(resource_revision_id, resource_id, provider_revision, observed_at, created_by_json) VALUES (?1, 'route-source', ?1, ?2, '{}')",
            rusqlite::params![revision, timestamp],
        ).expect("source revision");
    }
    transaction.execute(
        "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('route-artifact-resource', ?1, 'ARTIFACT', '{}', 'Artifact', 'route-artifact-r1', 'PERSONAL', '{}', ?2, ?2, 1)",
        rusqlite::params![WORKSPACE, NOW],
    ).expect("Artifact Resource");
    transaction.execute(
        "INSERT INTO resource_revisions(resource_revision_id, resource_id, observed_at, created_by_json) VALUES ('route-artifact-r1', 'route-artifact-resource', ?1, '{}')",
        [NOW],
    ).expect("Artifact Resource revision");
    transaction.execute(
        "INSERT INTO artifacts(artifact_id, workspace_id, resource_id, kind, display_name, current_version, library_status, created_at) VALUES ('route-artifact', ?1, 'route-artifact-resource', 'document', 'Artifact', 1, 'TRANSIENT', ?2)",
        rusqlite::params![WORKSPACE, NOW],
    ).expect("Artifact");
    transaction.execute(
        "INSERT INTO artifact_versions(artifact_id, resource_id, version, resource_revision_id, content_kind, content_digest, storage_ref_json, content_media_type, content_size_bytes, provenance_json, created_at) VALUES ('route-artifact', 'route-artifact-resource', 1, 'route-artifact-r1', 'MANAGED_BLOB', ?1, '{}', 'text/plain', 0, '{}', ?2)",
        rusqlite::params![format!("sha256:{}", "a".repeat(64)), NOW],
    ).expect("Artifact version");
    transaction.execute(
        "INSERT INTO dependency_edges(dependency_edge_id, workspace_id, source_resource_id, source_revision_id, dependent_kind, dependent_ref, artifact_id, artifact_version, created_at) VALUES ('route-dependency-edge', ?1, 'route-source', 'route-source-r1', 'ARTIFACT_VERSION', 'artifact://' || ?1 || '/route-artifact@v1', 'route-artifact', 1, ?2)",
        rusqlite::params![WORKSPACE, NOW],
    ).expect("dependency edge");
    transaction.execute(
        "INSERT INTO invalidation_records(invalidation_record_id, dependency_edge_id, observed_revision_id, reason_code, created_at) VALUES ('route-invalidation-a', 'route-dependency-edge', 'route-source-r2', 'SOURCE_RESOURCE_REVISION_ADVANCED', '2026-10-12T12:01:00Z'), ('route-invalidation-b', 'route-dependency-edge', 'route-source-r3', 'SOURCE_RESOURCE_REVISION_ADVANCED', '2026-10-12T12:01:00Z')",
        [],
    ).expect("invalidation history");
    transaction.execute(
        "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('other-route-source', 'invalidation-route-other-workspace', 'CONNECTOR_OBJECT', '{}', 'Other source', 'other-route-source-r1', 'PERSONAL', '{}', ?1, ?1, 1)",
        [NOW],
    ).expect("other Workspace source Resource");
    transaction.execute(
        "INSERT INTO resource_revisions(resource_revision_id, resource_id, observed_at, created_by_json) VALUES ('other-route-source-r1', 'other-route-source', ?1, '{}')",
        [NOW],
    ).expect("other Workspace source revision");
    transaction.execute(
        "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('other-route-artifact-resource', 'invalidation-route-other-workspace', 'ARTIFACT', '{}', 'Other artifact', 'other-route-artifact-r1', 'PERSONAL', '{}', ?1, ?1, 1)",
        [NOW],
    ).expect("other Workspace Artifact Resource");
    transaction.execute(
        "INSERT INTO resource_revisions(resource_revision_id, resource_id, observed_at, created_by_json) VALUES ('other-route-artifact-r1', 'other-route-artifact-resource', ?1, '{}')",
        [NOW],
    ).expect("other Workspace Artifact revision");
    transaction.execute(
        "INSERT INTO artifacts(artifact_id, workspace_id, resource_id, kind, display_name, current_version, library_status, created_at) VALUES ('other-route-artifact', 'invalidation-route-other-workspace', 'other-route-artifact-resource', 'document', 'Other artifact', 1, 'TRANSIENT', ?1)",
        [NOW],
    ).expect("other Workspace Artifact");
    transaction.execute(
        "INSERT INTO artifact_versions(artifact_id, resource_id, version, resource_revision_id, content_kind, content_digest, storage_ref_json, content_media_type, content_size_bytes, provenance_json, created_at) VALUES ('other-route-artifact', 'other-route-artifact-resource', 1, 'other-route-artifact-r1', 'MANAGED_BLOB', ?1, '{}', 'text/plain', 0, '{}', ?2)",
        rusqlite::params![format!("sha256:{}", "c".repeat(64)), NOW],
    ).expect("other Workspace Artifact version");
    transaction.execute(
        "INSERT INTO dependency_edges(dependency_edge_id, workspace_id, source_resource_id, source_revision_id, dependent_kind, dependent_ref, artifact_id, artifact_version, created_at) VALUES ('other-workspace-edge', 'invalidation-route-other-workspace', 'other-route-source', 'other-route-source-r1', 'ARTIFACT_VERSION', 'artifact://invalidation-route-other-workspace/other-route-artifact@v1', 'other-route-artifact', 1, ?1)",
        [NOW],
    ).expect("edge in another Workspace owned by the same principal");
    transaction
        .commit()
        .expect("fixture satisfies database constraints");

    let router = dependency_invalidations_test_router(DependencyInvalidationsApiState::new(
        store.clone(),
        OWNER.to_owned(),
    ));
    let mut first_request =
        Request::get("/v1/dependency-edges/route-dependency-edge/invalidations?limit=1")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("request is valid");
    first_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let first_response = router
        .clone()
        .oneshot(first_request)
        .await
        .expect("router returns first page");
    assert_eq!(first_response.status(), StatusCode::OK);
    assert_eq!(first_response.headers()[header::CACHE_CONTROL], "no-store");
    let first_body = to_bytes(first_response.into_body(), 4096)
        .await
        .expect("bounded first page");
    let first_page: Value = serde_json::from_slice(&first_body).expect("first page JSON");
    assert_eq!(first_page["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        first_page["items"][0]["invalidation_record_id"],
        "route-invalidation-a"
    );
    assert_eq!(
        first_page["items"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "created_at",
            "dependency_edge_id",
            "invalidation_record_id",
            "observed_revision_id",
            "reason_code"
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    let cursor = first_page["next_cursor"]
        .as_str()
        .expect("continuation cursor");

    let mut second_request = Request::get(format!(
        "/v1/dependency-edges/route-dependency-edge/invalidations?limit=1&cursor={cursor}"
    ))
    .header("x-workspace-id", WORKSPACE)
    .body(Body::empty())
    .expect("continuation request is valid");
    second_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let second_response = router
        .clone()
        .oneshot(second_request)
        .await
        .expect("router returns second page");
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body = to_bytes(second_response.into_body(), 4096)
        .await
        .expect("bounded second page");
    let second_page: Value = serde_json::from_slice(&second_body).expect("second page JSON");
    assert_eq!(
        second_page["items"][0]["invalidation_record_id"],
        "route-invalidation-b"
    );
    assert!(second_page["next_cursor"].is_null());

    let mut changed_limit_request = Request::get(format!(
        "/v1/dependency-edges/route-dependency-edge/invalidations?limit=2&cursor={cursor}"
    ))
    .header("x-workspace-id", WORKSPACE)
    .body(Body::empty())
    .expect("changed-limit request is valid");
    changed_limit_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let changed_limit_response = router
        .clone()
        .oneshot(changed_limit_request)
        .await
        .expect("router rejects changed page size");
    assert_eq!(changed_limit_response.status(), StatusCode::BAD_REQUEST);

    let mut mismatched_request = Request::get(format!(
        "/v1/dependency-edges/another-edge/invalidations?limit=1&cursor={cursor}"
    ))
    .header("x-workspace-id", WORKSPACE)
    .body(Body::empty())
    .expect("mismatched cursor request is valid");
    mismatched_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let mismatched_response = router
        .clone()
        .oneshot(mismatched_request)
        .await
        .expect("router rejects mismatched cursor");
    assert_eq!(mismatched_response.status(), StatusCode::BAD_REQUEST);

    let mut missing_edge_request =
        Request::get("/v1/dependency-edges/missing-edge/invalidations?limit=1")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("missing-edge request is valid");
    missing_edge_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let missing_edge_response = router
        .clone()
        .oneshot(missing_edge_request)
        .await
        .expect("router handles missing edge");
    assert_eq!(missing_edge_response.status(), StatusCode::NOT_FOUND);

    let mut other_workspace_edge_request =
        Request::get("/v1/dependency-edges/other-workspace-edge/invalidations?limit=1")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("other-Workspace edge request is valid");
    other_workspace_edge_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let other_workspace_edge_response = router
        .clone()
        .oneshot(other_workspace_edge_request)
        .await
        .expect("router hides edge from another owned Workspace");
    assert_eq!(
        other_workspace_edge_response.status(),
        StatusCode::NOT_FOUND
    );

    let foreign_owner_router = dependency_invalidations_test_router(
        DependencyInvalidationsApiState::new(store, "other-principal".to_owned()),
    );
    let mut foreign_owner_request =
        Request::get("/v1/dependency-edges/route-dependency-edge/invalidations?limit=1")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("foreign-owner request is valid");
    foreign_owner_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let foreign_owner_response = foreign_owner_router
        .oneshot(foreign_owner_request)
        .await
        .expect("router denies foreign Workspace owner");
    assert_eq!(foreign_owner_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn workspace_archive_route_is_owner_scoped_quiescent_and_idempotent() {
    const WORKSPACE_ID: &str = "archive-route-workspace";
    const OWNER: &str = "archive-route-owner";
    const NOW: &str = "2026-10-11T13:00:00Z";

    let directory = tempfile::tempdir().expect("temporary Workspace archive route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
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
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Archive route Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "archive-route-created".to_owned(),
                origin_runtime_id: "archive-route-runtime".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "archive-route-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner Workspace commits");
    let router = workspace_archive_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceArchiveApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: "archive-route-runtime".to_owned(),
        });

    async fn send_archive(
        router: Router,
        workspace_id: &str,
        key: &str,
        version: &str,
    ) -> Response {
        let mut request = Request::post(format!("/v1/workspaces/{workspace_id}/archive"))
            .header("x-workspace-id", workspace_id)
            .header("if-match", version)
            .header("idempotency-key", key)
            .body(Body::empty())
            .expect("archive request is valid");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .oneshot(request)
            .await
            .expect("archive route responds")
    }

    let missing_version = {
        let mut request = Request::post(format!("/v1/workspaces/{WORKSPACE_ID}/archive"))
            .header("x-workspace-id", WORKSPACE_ID)
            .header("idempotency-key", "archive-route-no-version")
            .body(Body::empty())
            .expect("missing-version request is structurally valid");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("route responds")
    };
    assert_eq!(missing_version.status(), StatusCode::BAD_REQUEST);
    let missing_version_body = to_bytes(missing_version.into_body(), 4096)
        .await
        .expect("bounded missing-version error");
    assert_eq!(
        serde_json::from_slice::<Value>(&missing_version_body).expect("error JSON")["code"],
        "INVALID_ARGUMENT"
    );

    let stale = send_archive(router.clone(), WORKSPACE_ID, "archive-route-stale", "0").await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let stale_body = to_bytes(stale.into_body(), 4096)
        .await
        .expect("bounded stale-version error");
    assert_eq!(
        serde_json::from_slice::<Value>(&stale_body).expect("error JSON")["code"],
        "STALE_WORKSPACE_VERSION"
    );

    let other_owner_router = workspace_archive_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceArchiveApiState {
            store: store.clone(),
            principal_id: "another-owner".to_owned(),
            runtime_id: "archive-route-runtime".to_owned(),
        });
    let other_owner = send_archive(
        other_owner_router,
        WORKSPACE_ID,
        "archive-route-other-owner",
        "1",
    )
    .await;
    assert_eq!(other_owner.status(), StatusCode::FORBIDDEN);

    let first = send_archive(router.clone(), WORKSPACE_ID, "archive-route-key", "1").await;
    assert_eq!(first.status(), StatusCode::OK);
    assert!(first.headers().contains_key("x-correlation-id"));
    let first_body = to_bytes(first.into_body(), 4096)
        .await
        .expect("bounded archive response");
    let first_workspace: Value = serde_json::from_slice(&first_body).expect("Workspace JSON");
    assert_eq!(first_workspace["status"], "ARCHIVED");
    assert_eq!(first_workspace["version"], 2);

    let replay = send_archive(router.clone(), WORKSPACE_ID, "archive-route-key", "1").await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay_body = to_bytes(replay.into_body(), 4096)
        .await
        .expect("bounded archive replay response");
    assert_eq!(
        serde_json::from_slice::<Value>(&replay_body).expect("replay JSON"),
        first_workspace
    );

    let changed_receipt =
        send_archive(router.clone(), WORKSPACE_ID, "archive-route-key", "2").await;
    assert_eq!(changed_receipt.status(), StatusCode::CONFLICT);
    let changed_receipt_body = to_bytes(changed_receipt.into_body(), 4096)
        .await
        .expect("bounded archive receipt conflict");
    assert_eq!(
        serde_json::from_slice::<Value>(&changed_receipt_body).expect("error JSON")["code"],
        "CONFLICT"
    );

    let changed_key = send_archive(router.clone(), WORKSPACE_ID, "archive-route-key-2", "2").await;
    assert_eq!(changed_key.status(), StatusCode::CONFLICT);
    let changed_key_body = to_bytes(changed_key.into_body(), 4096)
        .await
        .expect("bounded transition error");
    assert_eq!(
        serde_json::from_slice::<Value>(&changed_key_body).expect("error JSON")["code"],
        "INVALID_TRANSITION"
    );

    let mismatch = {
        let mut request = Request::post(format!("/v1/workspaces/{WORKSPACE_ID}/archive"))
            .header("x-workspace-id", "different-workspace")
            .header("if-match", "2")
            .header("idempotency-key", "archive-route-mismatch")
            .body(Body::empty())
            .expect("mismatched request is valid");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("route responds")
    };
    assert_eq!(mismatch.status(), StatusCode::FORBIDDEN);

    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open archive event ledger");
    let archived_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'workspace.archived.v1'",
            [WORKSPACE_ID],
            |row| row.get(0),
        )
        .expect("count archive events");
    assert_eq!(archived_events, 1);
}

#[tokio::test]
async fn conversation_create_replays_receipt_after_archive_and_rejects_fresh_create() {
    const WORKSPACE_ID: &str = "conversation-archive-replay-workspace";
    const OWNER: &str = "conversation-archive-replay-owner";
    const RUNTIME: &str = "conversation-archive-replay-runtime";
    const NOW: &str = "2026-10-11T14:00:00Z";

    let directory = tempfile::tempdir().expect("private Conversation route fixture directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
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
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Conversation archive replay Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "conversation-archive-replay-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "conversation-archive-replay-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits");

    let router = super::conversation_operator::test_router(
        store.clone(),
        OWNER.to_owned(),
        RUNTIME.to_owned(),
    );
    async fn create(router: Router, workspace_id: &str, key: &str, title: &str) -> Response {
        let mut request = Request::post("/v1/conversations")
            .header("x-workspace-id", workspace_id)
            .header("idempotency-key", key)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({"workspace_id":workspace_id,"title":title}).to_string(),
            ))
            .expect("Conversation create request is valid");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router.oneshot(request).await.expect("route responds")
    }

    let first = create(
        router.clone(),
        WORKSPACE_ID,
        "conversation-archive-replay-key",
        "Saved title",
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_body = to_bytes(first.into_body(), 1024 * 1024)
        .await
        .expect("first response body");
    let first_record: ConversationRecord =
        serde_json::from_slice(&first_body).expect("Conversation response is typed");

    WorkspaceService::new(store.clone())
        .archive_idempotent(
            ArchiveWorkspace {
                workspace_id: WORKSPACE_ID.to_owned(),
                expected_version: 1,
                principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "conversation-archive-replay-archived".to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "conversation-archive-replay-archived".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            },
            "conversation-archive-replay-archive-key".to_owned(),
            serde_json::json!({
                "operation":"workspace.archive.v1",
                "workspace_id":WORKSPACE_ID,
                "expected_version":1
            }),
        )
        .expect("quiescent Workspace archives");

    let replay = create(
        router.clone(),
        WORKSPACE_ID,
        "conversation-archive-replay-key",
        "Saved title",
    )
    .await;
    assert_eq!(replay.status(), StatusCode::CREATED);
    let replay_body = to_bytes(replay.into_body(), 1024 * 1024)
        .await
        .expect("replayed response body");
    let replay_record: ConversationRecord =
        serde_json::from_slice(&replay_body).expect("replayed Conversation response is typed");
    assert_eq!(replay_record, first_record);

    let changed_payload = create(
        router.clone(),
        WORKSPACE_ID,
        "conversation-archive-replay-key",
        "Changed title",
    )
    .await;
    assert_eq!(changed_payload.status(), StatusCode::CONFLICT);
    let conflict_body = to_bytes(changed_payload.into_body(), 1024 * 1024)
        .await
        .expect("conflict body");
    let conflict: Value = serde_json::from_slice(&conflict_body).expect("typed conflict");
    assert_eq!(conflict["code"], "CONFLICT");

    let fresh_create = create(
        router,
        WORKSPACE_ID,
        "conversation-archive-fresh-key",
        "New conversation",
    )
    .await;
    assert_eq!(fresh_create.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let archived_body = to_bytes(fresh_create.into_body(), 1024 * 1024)
        .await
        .expect("archived-create body");
    let archived: Value = serde_json::from_slice(&archived_body).expect("typed archive error");
    assert_eq!(archived["code"], "WORKSPACE_ARCHIVED");

    assert_eq!(
        store
            .read_workspace_events(WORKSPACE_ID)
            .expect("workspace events read")
            .iter()
            .filter(|event| event.event_type == "conversation.created.v1")
            .count(),
        1
    );
    let connection = rusqlite::Connection::open(database).expect("fixture database opens");
    let conversation_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE workspace_id=?1",
            [WORKSPACE_ID],
            |row| row.get(0),
        )
        .expect("count persisted Conversations");
    assert_eq!(conversation_count, 1);
}

#[tokio::test]
async fn workspace_archive_route_returns_typed_channel_host_blockers_without_writes() {
    const WORKSPACE_ID: &str = "archive-blocked-route-workspace";
    const OWNER: &str = "archive-blocked-route-owner";
    const NOW: &str = "2026-10-11T13:00:00Z";

    let directory = tempfile::tempdir().expect("temporary blocked archive route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
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
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Blocked archive route Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "archive-blocked-route-created".to_owned(),
                origin_runtime_id: "archive-blocked-route-runtime".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "archive-blocked-route-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner Workspace commits");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open route fixture database");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TRIGGER channel_host_assignment_requires_active_binding_insert;",
        )
        .expect("disable fixture-only channel admission validation");
    connection
        .execute(
            "INSERT INTO channel_host_assignments(channel_binding_id,workspace_id,runtime_id,host_epoch,status,assigned_at) VALUES('archive-blocked-channel-host',?1,'runtime-fixture',1,'ACTIVE',?2)",
            rusqlite::params![WORKSPACE_ID, NOW],
        )
        .expect("seed active channel-host blocker");
    drop(connection);

    let router = workspace_archive_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceArchiveApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: "archive-blocked-route-runtime".to_owned(),
        });
    let mut request = Request::post(format!("/v1/workspaces/{WORKSPACE_ID}/archive"))
        .header("x-workspace-id", WORKSPACE_ID)
        .header("if-match", "1")
        .header("idempotency-key", "archive-blocked-route-request")
        .body(Body::empty())
        .expect("archive request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let response = router
        .oneshot(request)
        .await
        .expect("archive route responds");
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = to_bytes(response.into_body(), 4096)
        .await
        .expect("bounded archive blocker response");
    let body: Value = serde_json::from_slice(&body).expect("structured archive error");
    assert_eq!(body["code"], "WORKSPACE_NOT_QUIESCENT");
    assert_eq!(
        body["details"]["blockers"][0]["kind"],
        "ACTIVE_CHANNEL_HOSTS"
    );
    assert_eq!(body["details"]["blockers"][0]["count"], 1);

    let workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("Workspace read succeeds")
        .expect("Workspace remains present");
    assert_eq!(workspace.status, "ACTIVE");
    assert_eq!(workspace.version, 1);
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect archive receipt");
    let receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id='archive-blocked-route-request'",
            [],
            |row| row.get(0),
        )
        .expect("count archive receipts");
    assert_eq!(receipts, 0);
}

struct InstructionRouteWrite<'a> {
    workspace_id: &'a str,
    expected_version: u64,
    request_id: &'a str,
    parent_revisions: Vec<u64>,
    resource_id: &'a str,
    resource_revision_id: &'a str,
    content_digest: &'a str,
    content_ref: Option<Value>,
}

async fn post_instruction_revision(router: Router, command: InstructionRouteWrite<'_>) -> Response {
    let InstructionRouteWrite {
        workspace_id,
        expected_version,
        request_id,
        parent_revisions,
        resource_id,
        resource_revision_id,
        content_digest,
        content_ref,
    } = command;
    let content_ref = content_ref.unwrap_or_else(|| {
        json!({
            "workspace_id": workspace_id,
            "resource_id": resource_id,
            "revision_id": resource_revision_id,
        })
    });
    let request_body = json!({
        "parent_revisions": parent_revisions,
        "content_ref": content_ref,
        "content_digest": content_digest,
    });
    let mut request = Request::post(format!(
        "/v1/workspaces/{workspace_id}/instructions/revisions"
    ))
    .header("x-workspace-id", workspace_id)
    .header("if-match", expected_version.to_string())
    .header("idempotency-key", request_id)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
        serde_json::to_vec(&request_body).expect("serialize instruction request"),
    ))
    .expect("instruction request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .oneshot(request)
        .await
        .expect("router returns a response")
}

fn seed_route_instruction_resource(
    directory: &tempfile::TempDir,
    blobs: &FileBlobStore<IpcReplayTestKeys>,
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
    media_type: &str,
    content: &[u8],
) -> String {
    let blob = blobs
        .put(workspace_id, BlobPurpose::Resource, content, media_type)
        .expect("store candidate instruction Resource");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open Resource fixture database");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable Resource fixture constraints");
    let transaction = connection
        .unchecked_transaction()
        .expect("start Resource fixture transaction");
    transaction
        .execute(
            "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,context_document_json,provenance_json,created_at,updated_at,version)
             VALUES (?1,?2,'FILE','{}','candidate-instructions',?3,'PERSONAL',NULL,'{}','2026-10-11T13:30:00Z','2026-10-11T13:30:00Z',1)",
            rusqlite::params![resource_id, workspace_id, revision_id],
        )
        .expect("seed candidate instruction Resource");
    transaction
        .execute(
            "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
             VALUES (?1,?2,?3,?4,?5,'2026-10-11T13:30:00Z','{}')",
            rusqlite::params![
                revision_id,
                resource_id,
                blob.digest,
                i64::try_from(content.len()).expect("candidate instruction size"),
                media_type,
            ],
        )
        .expect("seed candidate instruction revision");
    transaction
        .execute(
            "INSERT INTO resource_locations(location_id,resource_id,provider_ref,locator_ref_id,availability,writable,observed_revision_id,observed_digest,observed_at,last_checked_at)
             VALUES (?1,?2,'litecowork.encrypted_blob',?3,'AVAILABLE',0,?4,?3,'2026-10-11T13:30:00Z','2026-10-11T13:30:00Z')",
            rusqlite::params![
                format!("location-{resource_id}"),
                resource_id,
                blob.digest,
                revision_id,
            ],
        )
        .expect("seed candidate encrypted Resource location");
    transaction
        .commit()
        .expect("candidate Resource fixture commits");
    blob.digest
}

struct InstructionResourceHistoryFixture<'a> {
    workspace_id: &'a str,
    resource_id: &'a str,
    old_revision_id: &'a str,
    head_revision_id: &'a str,
    old_content: &'a [u8],
    head_content: &'a [u8],
}

fn seed_route_instruction_resource_with_history(
    directory: &tempfile::TempDir,
    blobs: &FileBlobStore<IpcReplayTestKeys>,
    fixture: InstructionResourceHistoryFixture<'_>,
) -> String {
    let InstructionResourceHistoryFixture {
        workspace_id,
        resource_id,
        old_revision_id,
        head_revision_id,
        old_content,
        head_content,
    } = fixture;
    let old_blob = blobs
        .put(
            workspace_id,
            BlobPurpose::Resource,
            old_content,
            "text/plain",
        )
        .expect("store historical instruction Resource content");
    let head_blob = blobs
        .put(
            workspace_id,
            BlobPurpose::Resource,
            head_content,
            "text/plain",
        )
        .expect("store current instruction Resource content");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open Resource history fixture database");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable Resource history constraints");
    let transaction = connection
        .unchecked_transaction()
        .expect("start Resource history fixture transaction");
    transaction
        .execute(
            "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,context_document_json,provenance_json,created_at,updated_at,version)
             VALUES (?1,?2,'FILE','{}','candidate-instructions',?3,'PERSONAL',NULL,'{}','2026-10-11T13:30:00Z','2026-10-11T13:30:00Z',2)",
            rusqlite::params![resource_id, workspace_id, head_revision_id],
        )
        .expect("seed candidate instruction Resource at second head");
    for (revision_id, blob, content) in [
        (old_revision_id, &old_blob, old_content),
        (head_revision_id, &head_blob, head_content),
    ] {
        transaction
            .execute(
                "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
                 VALUES (?1,?2,?3,?4,'text/plain','2026-10-11T13:30:00Z','{}')",
                rusqlite::params![
                    revision_id,
                    resource_id,
                    blob.digest,
                    i64::try_from(content.len()).expect("candidate instruction size"),
                ],
            )
            .expect("seed candidate instruction Resource revision");
    }
    transaction
        .execute(
            "INSERT INTO resource_revision_parents(resource_id,child_revision_id,parent_revision_id) VALUES (?1,?2,?3)",
            rusqlite::params![resource_id, head_revision_id, old_revision_id],
        )
        .expect("seed current Resource ancestry");
    transaction
        .execute(
            "INSERT INTO resource_locations(location_id,resource_id,provider_ref,locator_ref_id,availability,writable,observed_revision_id,observed_digest,observed_at,last_checked_at)
             VALUES (?1,?2,'litecowork.encrypted_blob',?3,'AVAILABLE',0,?4,?3,'2026-10-11T13:30:00Z','2026-10-11T13:30:00Z')",
            rusqlite::params![
                format!("location-{resource_id}"),
                resource_id,
                old_blob.digest,
                old_revision_id,
            ],
        )
        .expect("seed historical encrypted Resource location");
    transaction
        .commit()
        .expect("Resource history fixture commits");
    old_blob.digest
}

async fn get_instruction_history(
    router: Router,
    workspace_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Response {
    let path = match cursor {
        Some(cursor) => format!(
            "/v1/workspaces/{workspace_id}/instructions/revisions?limit={limit}&cursor={cursor}"
        ),
        None => format!("/v1/workspaces/{workspace_id}/instructions/revisions?limit={limit}"),
    };
    let mut request = Request::get(path)
        .header("x-workspace-id", workspace_id)
        .body(Body::empty())
        .expect("instruction history request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .oneshot(request)
        .await
        .expect("router returns a response")
}

async fn patch_workspace_policy(
    router: Router,
    workspace_id: &str,
    expected_version: &str,
    request_id: &str,
    policy: &str,
) -> Response {
    let mut request = Request::patch(format!("/v1/workspaces/{workspace_id}"))
        .header("x-workspace-id", workspace_id)
        .header("if-match", expected_version)
        .header("idempotency-key", request_id)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "replication_policy": policy }).to_string(),
        ))
        .expect("Workspace policy request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .oneshot(request)
        .await
        .expect("policy route responds")
}

#[tokio::test]
async fn workspace_policy_route_is_owner_scoped_versioned_and_idempotent() {
    const WORKSPACE_ID: &str = "policy-route-workspace";
    const OWNER: &str = "policy-route-owner";
    const RUNTIME: &str = "policy-route-runtime";
    const NOW: &str = "2026-10-12T10:00:00Z";

    let directory = tempfile::tempdir().expect("temporary policy route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private policy route directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
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
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Policy route Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "policy-route-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "policy-route-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner Workspace commits");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: "policy-route-second-workspace".to_owned(),
            name: "Second policy route Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "policy-route-second-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "policy-route-second-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("second owner Workspace commits");

    let router = workspace_policy_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspacePolicyApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
        });
    let first = patch_workspace_policy(
        router.clone(),
        WORKSPACE_ID,
        "1",
        "policy-route-request-1",
        "METADATA_ONLY",
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first = to_bytes(first.into_body(), 4096)
        .await
        .expect("bounded policy response");
    let first: Value = serde_json::from_slice(&first).expect("policy response JSON");
    assert_eq!(first["replication_policy"], "METADATA_ONLY");
    assert_eq!(first["version"], 2);

    let replay = patch_workspace_policy(
        router.clone(),
        WORKSPACE_ID,
        "1",
        "policy-route-request-1",
        "METADATA_ONLY",
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = to_bytes(replay.into_body(), 4096)
        .await
        .expect("bounded replay response");
    let replay: Value = serde_json::from_slice(&replay).expect("policy replay JSON");
    assert_eq!(replay, first);

    let changed_version_replay = patch_workspace_policy(
        router.clone(),
        WORKSPACE_ID,
        "2",
        "policy-route-request-1",
        "METADATA_ONLY",
    )
    .await;
    assert_eq!(changed_version_replay.status(), StatusCode::CONFLICT);

    let cross_workspace_replay = patch_workspace_policy(
        router.clone(),
        "policy-route-second-workspace",
        "1",
        "policy-route-request-1",
        "METADATA_ONLY",
    )
    .await;
    assert_eq!(cross_workspace_replay.status(), StatusCode::CONFLICT);

    let changed_replay = patch_workspace_policy(
        router.clone(),
        WORKSPACE_ID,
        "1",
        "policy-route-request-1",
        "FULL_WORKSPACE",
    )
    .await;
    assert_eq!(changed_replay.status(), StatusCode::CONFLICT);
    let changed_replay_body = to_bytes(changed_replay.into_body(), 4096)
        .await
        .expect("bounded changed-receipt error");
    assert_eq!(
        serde_json::from_slice::<Value>(&changed_replay_body).expect("error JSON")["code"],
        "CONFLICT"
    );
    let stale = patch_workspace_policy(
        router.clone(),
        WORKSPACE_ID,
        "1",
        "policy-route-stale-version",
        "FULL_WORKSPACE",
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let stale_body = to_bytes(stale.into_body(), 4096)
        .await
        .expect("bounded stale policy error");
    assert_eq!(
        serde_json::from_slice::<Value>(&stale_body).expect("error JSON")["code"],
        "STALE_WORKSPACE_VERSION"
    );

    let foreign_owner_router = workspace_policy_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspacePolicyApiState {
            store: store.clone(),
            principal_id: "another-owner".to_owned(),
            runtime_id: RUNTIME.to_owned(),
        });
    let foreign_owner = patch_workspace_policy(
        foreign_owner_router,
        WORKSPACE_ID,
        "2",
        "policy-route-foreign-owner",
        "FULL_WORKSPACE",
    )
    .await;
    assert_eq!(foreign_owner.status(), StatusCode::FORBIDDEN);

    let mut mismatched_workspace = Request::patch(format!("/v1/workspaces/{WORKSPACE_ID}"))
        .header("x-workspace-id", "policy-route-other-workspace")
        .header("if-match", "2")
        .header("idempotency-key", "policy-route-mismatched-workspace")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "replication_policy": "FULL_WORKSPACE" }).to_string(),
        ))
        .expect("mismatched Workspace policy request is valid HTTP");
    mismatched_workspace
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let mismatched_workspace = router
        .clone()
        .oneshot(mismatched_workspace)
        .await
        .expect("policy route responds to mismatched Workspace");
    assert_eq!(mismatched_workspace.status(), StatusCode::FORBIDDEN);

    let current = store
        .get_workspace(WORKSPACE_ID)
        .expect("read policy route Workspace")
        .expect("Workspace remains persisted");
    assert_eq!(current.replication_policy.as_str(), "METADATA_ONLY");
    assert_eq!(current.version, 2);
    let second_workspace = store
        .get_workspace("policy-route-second-workspace")
        .expect("read second policy route Workspace")
        .expect("second Workspace remains persisted");
    assert_eq!(second_workspace.replication_policy.as_str(), "LOCAL_ONLY");
    assert_eq!(second_workspace.version, 1);
    let connection = rusqlite::Connection::open(database).expect("inspect policy route rows");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'workspace.replication_policy.changed.v1'",
            [WORKSPACE_ID],
            |row| row.get(0),
        )
        .expect("count committed policy events");
    assert_eq!(event_count, 1);
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'policy-route-%'",
            [],
            |row| row.get(0),
        )
        .expect("count policy receipts");
    assert_eq!(receipt_count, 1);
}

#[tokio::test]
async fn workspace_default_agent_route_distinguishes_receipt_conflict_from_stale_version() {
    const WORKSPACE_ID: &str = "default-agent-route-workspace";
    const OWNER: &str = "default-agent-route-owner";
    const RUNTIME: &str = "default-agent-route-runtime";
    const BINDING_ID: &str = "default-agent-route-binding";
    const NOW: &str = "2026-10-12T11:00:00Z";

    fn count_files(path: &std::path::Path) -> usize {
        let Ok(entries) = fs::read_dir(path) else {
            return 0;
        };
        entries
            .filter_map(Result::ok)
            .map(|entry| {
                let path = entry.path();
                if path.is_dir() { count_files(&path) } else { 1 }
            })
            .sum()
    }

    let directory = tempfile::tempdir().expect("temporary default AgentBinding route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
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
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Default AgentBinding route Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "default-agent-route-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "default-agent-route-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner Workspace commits");
    let connection = rusqlite::Connection::open(&database).expect("open binding fixture database");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at) VALUES ('default-agent-route-profile', 'fixture', 'Fixture Agent', ?1)",
            [NOW],
        )
        .expect("seed AgentProfile");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version) VALUES (?1, ?2, 'default-agent-route-profile', '{}', '{}', 1, 1, ?3, 1)",
            rusqlite::params![BINDING_ID, WORKSPACE_ID, NOW],
        )
        .expect("seed enabled lead AgentBinding");
    drop(connection);

    let router = Router::new()
        .route(
            "/v1/workspaces/{workspace_id}/default-agent-binding",
            axum::routing::patch(super::set_workspace_default_agent_binding),
        )
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspacePolicyApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
        });

    async fn set_default(
        router: Router,
        workspace_id: &str,
        version: &str,
        key: &str,
        binding_id: Value,
    ) -> Response {
        let mut request = Request::patch(format!(
            "/v1/workspaces/{workspace_id}/default-agent-binding"
        ))
        .header("x-workspace-id", workspace_id)
        .header("if-match", version)
        .header("idempotency-key", key)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"agent_binding_id": binding_id}).to_string(),
        ))
        .expect("Workspace default AgentBinding request is valid");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .oneshot(request)
            .await
            .expect("default AgentBinding route responds")
    }

    let first = set_default(
        router.clone(),
        WORKSPACE_ID,
        "1",
        "default-agent-route-key",
        json!(BINDING_ID),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = to_bytes(first.into_body(), 4096)
        .await
        .expect("bounded committed Workspace body");
    let first_workspace: Value = serde_json::from_slice(&first_body).expect("Workspace JSON");
    let blob_count_after_commit = count_files(&directory.path().join("blobs"));
    let replay = set_default(
        router.clone(),
        WORKSPACE_ID,
        "1",
        "default-agent-route-key",
        json!(BINDING_ID),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay_body = to_bytes(replay.into_body(), 4096)
        .await
        .expect("bounded replayed Workspace body");
    assert_eq!(
        serde_json::from_slice::<Value>(&replay_body).expect("Workspace JSON"),
        first_workspace
    );
    assert_eq!(
        count_files(&directory.path().join("blobs")),
        blob_count_after_commit,
        "an exact replay must not write unreferenced candidate aggregate state"
    );

    let changed_if_match = set_default(
        router.clone(),
        WORKSPACE_ID,
        "2",
        "default-agent-route-key",
        json!(BINDING_ID),
    )
    .await;
    assert_eq!(changed_if_match.status(), StatusCode::CONFLICT);
    let changed_if_match_body = to_bytes(changed_if_match.into_body(), 4096)
        .await
        .expect("bounded changed-If-Match receipt conflict");
    assert_eq!(
        serde_json::from_slice::<Value>(&changed_if_match_body).expect("error JSON")["code"],
        "CONFLICT",
        "the If-Match precondition is part of the idempotency payload"
    );

    let changed_receipt = set_default(
        router.clone(),
        WORKSPACE_ID,
        "1",
        "default-agent-route-key",
        Value::Null,
    )
    .await;
    assert_eq!(changed_receipt.status(), StatusCode::CONFLICT);
    let changed_body = to_bytes(changed_receipt.into_body(), 4096)
        .await
        .expect("bounded receipt conflict");
    assert_eq!(
        serde_json::from_slice::<Value>(&changed_body).expect("error JSON")["code"],
        "CONFLICT"
    );

    let clear_default = set_default(
        router.clone(),
        WORKSPACE_ID,
        "2",
        "default-agent-route-clear",
        Value::Null,
    )
    .await;
    assert_eq!(clear_default.status(), StatusCode::OK);

    let connection = rusqlite::Connection::open(&database).expect("open binding fixture database");
    connection
        .execute(
            "UPDATE agent_bindings SET enabled = 0, lead_eligible = 0, version = version + 1 WHERE agent_binding_id = ?1 AND workspace_id = ?2",
            rusqlite::params![BINDING_ID, WORKSPACE_ID],
        )
        .expect("disable the selected binding after the committed request");
    drop(connection);
    let blob_count_before_late_replay = count_files(&directory.path().join("blobs"));

    let replay_after_binding_change = set_default(
        router.clone(),
        WORKSPACE_ID,
        "1",
        "default-agent-route-key",
        json!(BINDING_ID),
    )
    .await;
    assert_eq!(
        replay_after_binding_change.status(),
        StatusCode::OK,
        "an exact receipt retry must be resolved before current binding eligibility"
    );
    let late_replay_body = to_bytes(replay_after_binding_change.into_body(), 4096)
        .await
        .expect("bounded replayed Workspace body");
    assert_eq!(
        serde_json::from_slice::<Value>(&late_replay_body).expect("Workspace JSON"),
        first_workspace
    );
    assert_eq!(
        count_files(&directory.path().join("blobs")),
        blob_count_before_late_replay,
        "a receipt replay after binding changes must not write candidate state"
    );

    let new_disabled_binding_selection = set_default(
        router.clone(),
        WORKSPACE_ID,
        "3",
        "default-agent-route-disabled-binding",
        json!(BINDING_ID),
    )
    .await;
    assert_eq!(
        new_disabled_binding_selection.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let disabled_binding_body = to_bytes(new_disabled_binding_selection.into_body(), 4096)
        .await
        .expect("bounded disabled-binding error");
    assert_eq!(
        serde_json::from_slice::<Value>(&disabled_binding_body).expect("error JSON")["code"],
        "AGENT_NOT_LEAD_ELIGIBLE"
    );

    let stale = set_default(
        router,
        WORKSPACE_ID,
        "1",
        "default-agent-route-stale",
        Value::Null,
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let stale_body = to_bytes(stale.into_body(), 4096)
        .await
        .expect("bounded stale-version error");
    assert_eq!(
        serde_json::from_slice::<Value>(&stale_body).expect("error JSON")["code"],
        "STALE_WORKSPACE_VERSION"
    );

    assert_eq!(
        store
            .read_workspace_events(WORKSPACE_ID)
            .expect("read Workspace events")
            .iter()
            .filter(|event| event.event_type == "workspace.default_agent_binding.changed.v1")
            .count(),
        2,
        "the first selection and clear are the only default-binding events"
    );
    let current_workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("Workspace lookup")
        .expect("Workspace exists");
    assert_eq!(current_workspace.version, 3);
    assert_eq!(current_workspace.default_agent_binding_id, None);
}

#[tokio::test]
async fn instruction_create_route_commits_first_revision_and_replays_after_advance() {
    const WORKSPACE_ID: &str = "instruction-route-replay-workspace";
    const RESOURCE_ID: &str = "route-instruction-resource";
    const RESOURCE_REVISION_ID: &str = "route-instruction-resource-rev-1";
    const OWNER: &str = "instruction-route-owner";
    const BINDING_ID: &str = "instruction-route-agent-binding";
    const CONTENT: &[u8] = b"Workspace instructions from the local owner.\n";

    let directory = tempfile::tempdir().expect("temporary instruction commit directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        IpcReplayTestKeys,
    ));
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        blobs.clone(),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    let now = "2026-10-11T13:00:00Z";
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Instruction route Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "instruction-route-workspace-created".to_owned(),
                origin_runtime_id: "instruction-route-runtime".to_owned(),
                hlc_timestamp: now.to_owned(),
                correlation_id: "instruction-route-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: now.to_owned(),
            },
        })
        .expect("owner Workspace commits");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open database to seed lead AgentBinding");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at) VALUES ('instruction-route-agent-profile', 'fixture', 'Fixture Agent', ?1)",
            [now],
        )
        .expect("seed lead AgentProfile");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version) VALUES (?1, ?2, 'instruction-route-agent-profile', '{}', '{}', 1, 1, ?3, 1)",
            rusqlite::params![BINDING_ID, WORKSPACE_ID, now],
        )
        .expect("seed enabled lead AgentBinding");
    connection
        .execute(
            "UPDATE workspaces SET default_agent_binding_id = ?1 WHERE workspace_id = ?2",
            rusqlite::params![BINDING_ID, WORKSPACE_ID],
        )
        .expect("select the Workspace default lead");
    drop(connection);
    let resource_blob = blobs
        .put(WORKSPACE_ID, BlobPurpose::Resource, CONTENT, "text/plain")
        .expect("store pinned instruction text");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open Resource fixture database");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable Resource fixture constraints");
    let transaction = connection
        .unchecked_transaction()
        .expect("start Resource fixture transaction");
    transaction
        .execute(
            "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,context_document_json,provenance_json,created_at,updated_at,version)
             VALUES (?1,?2,'FILE','{}','instructions.txt',?3,'PERSONAL',NULL,'{}',?4,?4,1)",
            rusqlite::params![RESOURCE_ID, WORKSPACE_ID, RESOURCE_REVISION_ID, now],
        )
        .expect("seed instruction Resource");
    transaction
        .execute(
            "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
             VALUES (?1,?2,?3,?4,'text/plain',?5,'{}')",
            rusqlite::params![
                RESOURCE_REVISION_ID,
                RESOURCE_ID,
                resource_blob.digest,
                i64::try_from(CONTENT.len()).expect("instruction length"),
                now,
            ],
        )
        .expect("seed pinned instruction revision");
    transaction
        .execute(
            "INSERT INTO resource_locations(location_id,resource_id,provider_ref,locator_ref_id,availability,writable,observed_revision_id,observed_digest,observed_at,last_checked_at)
             VALUES ('route-instruction-location',?1,'litecowork.encrypted_blob',?2,'AVAILABLE',0,?3,?2,?4,?4)",
            rusqlite::params![RESOURCE_ID, resource_blob.digest, RESOURCE_REVISION_ID, now],
        )
        .expect("seed encrypted local Resource location");
    transaction.commit().expect("Resource fixture commits");

    let router = workspace_instruction_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceInstructionApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: "instruction-route-runtime".to_owned(),
        });
    let first = post_instruction_revision(
        router.clone(),
        InstructionRouteWrite {
            workspace_id: WORKSPACE_ID,
            expected_version: 1,
            request_id: "instruction-route-request-1",
            parent_revisions: vec![],
            resource_id: RESOURCE_ID,
            resource_revision_id: RESOURCE_REVISION_ID,
            content_digest: &resource_blob.digest,
            content_ref: None,
        },
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_body = to_bytes(first.into_body(), 4096)
        .await
        .expect("bounded first revision response");
    let first_body: Value = serde_json::from_slice(&first_body).expect("first response JSON");
    assert_eq!(first_body["revision"], 1);
    assert_eq!(first_body["parent_revisions"], json!([]));

    let task_router = Router::new()
        .route("/v1/tasks", post(super::create_task))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(TaskCreateApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: "instruction-route-runtime".to_owned(),
        });
    let task_request_id = "k".repeat(200);
    let task_payload = json!({
        "workspace_id": WORKSPACE_ID,
        "objective": "Use the current Workspace instructions",
        "source_message_refs": [],
        "constraints": [],
        "non_goals": [],
        "input_refs": [],
        "required_outputs": [],
        "acceptance_criteria": [],
        "approvals_required": [],
        "placement_preference": "AUTO"
    });
    let mut task_request = Request::post("/v1/tasks")
        .header("x-workspace-id", WORKSPACE_ID)
        .header("idempotency-key", task_request_id.as_str())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(task_payload.to_string()))
        .expect("Task creation request is valid");
    task_request.extensions_mut().insert(AuthenticatedLocalPeer);
    let task_response = task_router
        .clone()
        .oneshot(task_request)
        .await
        .expect("Task route returns a response");
    assert_eq!(task_response.status(), StatusCode::CREATED);
    let task_body = to_bytes(task_response.into_body(), 16 * 1024)
        .await
        .expect("bounded Task response");
    let task_body: Value = serde_json::from_slice(&task_body).expect("Task response JSON");
    let task_id = task_body["task"]["task_id"]
        .as_str()
        .expect("Task response has an ID")
        .to_owned();
    assert_eq!(task_body["task"]["status"], "READY");
    assert_eq!(task_body["task"]["lead_agent_binding_id"], BINDING_ID);
    assert_eq!(
        task_body["current_spec_revision"]["workspace_instruction_revision"],
        1
    );

    let foreign_task_router = Router::new()
        .route("/v1/tasks", post(super::create_task))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(TaskCreateApiState {
            store: store.clone(),
            principal_id: "another-task-owner".to_owned(),
            runtime_id: "instruction-route-runtime".to_owned(),
        });
    let mut foreign_replay_request = Request::post("/v1/tasks")
        .header("x-workspace-id", WORKSPACE_ID)
        .header("idempotency-key", task_request_id.as_str())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(task_payload.to_string()))
        .expect("foreign replay request is valid HTTP");
    foreign_replay_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let foreign_replay = foreign_task_router
        .oneshot(foreign_replay_request)
        .await
        .expect("foreign Task replay request receives a response");
    assert_eq!(foreign_replay.status(), StatusCode::FORBIDDEN);

    let second = post_instruction_revision(
        router.clone(),
        InstructionRouteWrite {
            workspace_id: WORKSPACE_ID,
            expected_version: 2,
            request_id: "instruction-route-request-2",
            parent_revisions: vec![1],
            resource_id: RESOURCE_ID,
            resource_revision_id: RESOURCE_REVISION_ID,
            content_digest: &resource_blob.digest,
            content_ref: None,
        },
    )
    .await;
    assert_eq!(second.status(), StatusCode::CREATED);
    let second_body = to_bytes(second.into_body(), 4096)
        .await
        .expect("bounded second revision response");
    let second_body: Value = serde_json::from_slice(&second_body).expect("second response JSON");
    assert_eq!(second_body["revision"], 2);
    assert_eq!(second_body["parent_revisions"], json!([1]));

    let default_agent_router = Router::new()
        .route(
            "/v1/workspaces/{workspace_id}/default-agent-binding",
            axum::routing::patch(super::set_workspace_default_agent_binding),
        )
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspacePolicyApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: "instruction-route-runtime".to_owned(),
        });
    let mut clear_default_request = Request::patch(format!(
        "/v1/workspaces/{WORKSPACE_ID}/default-agent-binding"
    ))
    .header("x-workspace-id", WORKSPACE_ID)
    .header("if-match", "3")
    .header("idempotency-key", "instruction-route-clear-default")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({"agent_binding_id": null}).to_string()))
    .expect("default AgentBinding clear request is valid");
    clear_default_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let clear_default_response = default_agent_router
        .oneshot(clear_default_request)
        .await
        .expect("default AgentBinding route clears Workspace default");
    assert_eq!(clear_default_response.status(), StatusCode::OK);
    let cleared_workspace_body = to_bytes(clear_default_response.into_body(), 4096)
        .await
        .expect("bounded clear-default response");
    let cleared_workspace: Value =
        serde_json::from_slice(&cleared_workspace_body).expect("clear-default response JSON");
    assert_eq!(cleared_workspace["version"], 4);
    assert_eq!(cleared_workspace["default_agent_binding_id"], Value::Null);

    let mut replay_request = Request::post("/v1/tasks")
        .header("x-workspace-id", WORKSPACE_ID)
        .header("idempotency-key", task_request_id.as_str())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(task_payload.to_string()))
        .expect("exact Task replay request is valid");
    replay_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let replay_response = task_router
        .clone()
        .oneshot(replay_request)
        .await
        .expect("Task route returns the committed replay");
    assert_eq!(replay_response.status(), StatusCode::CREATED);
    let replay_body = to_bytes(replay_response.into_body(), 16 * 1024)
        .await
        .expect("bounded Task replay response");
    let replay_body: Value = serde_json::from_slice(&replay_body).expect("Task replay JSON");
    assert_eq!(
        replay_body, task_body,
        "replay returns original Task receipt"
    );
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect Task replay effects");
    let task_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1",
            [WORKSPACE_ID],
            |row| row.get(0),
        )
        .expect("count committed Tasks");
    let create_event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'task.created.v1'",
            [WORKSPACE_ID],
            |row| row.get(0),
        )
        .expect("count Task creation events");
    assert_eq!(task_count, 1, "exact retry must not duplicate the Task row");
    assert_eq!(
        create_event_count, 1,
        "exact retry must not append another Task creation event"
    );

    let mut malformed_key_request = Request::post("/v1/tasks")
        .header("x-workspace-id", WORKSPACE_ID)
        .header("idempotency-key", "invalid key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(task_payload.to_string()))
        .expect("malformed-key request is valid HTTP");
    malformed_key_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let malformed_key_response = task_router
        .clone()
        .oneshot(malformed_key_request)
        .await
        .expect("Task route responds to malformed key");
    assert_eq!(malformed_key_response.status(), StatusCode::BAD_REQUEST);
    let malformed_key_body = to_bytes(malformed_key_response.into_body(), 4096)
        .await
        .expect("bounded malformed-key error");
    assert_eq!(
        serde_json::from_slice::<Value>(&malformed_key_body).expect("error JSON")["code"],
        "INVALID_ARGUMENT"
    );

    let persisted_task = store
        .get_task(WORKSPACE_ID, &task_id)
        .expect("read Task created by authenticated route")
        .expect("Task remains persisted");
    assert_eq!(
        persisted_task
            .current_spec_revision
            .workspace_instruction_revision,
        Some(1)
    );
    assert_eq!(
        store
            .get_workspace(WORKSPACE_ID)
            .expect("read advanced Workspace")
            .expect("Workspace remains persisted")
            .current_instruction_revision,
        Some(2)
    );

    let advanced_resource_content = b"new Resource head after instruction save";
    let advanced_resource_blob = blobs
        .put(
            WORKSPACE_ID,
            BlobPurpose::Resource,
            advanced_resource_content,
            "text/plain",
        )
        .expect("store advanced Resource head");
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open route fixture for Resource head advance");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable Resource ancestry constraints");
    let transaction = connection
        .unchecked_transaction()
        .expect("start Resource head advance transaction");
    transaction
        .execute(
            "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
             VALUES ('route-instruction-resource-rev-2',?1,?2,?3,'text/plain',?4,'{}')",
            rusqlite::params![
                RESOURCE_ID,
                advanced_resource_blob.digest,
                i64::try_from(advanced_resource_content.len()).expect("advanced Resource size"),
                now,
            ],
        )
        .expect("append Resource revision for replay fixture");
    transaction
        .execute(
            "INSERT INTO resource_revision_parents(resource_id,child_revision_id,parent_revision_id) VALUES (?1,'route-instruction-resource-rev-2',?2)",
            rusqlite::params![RESOURCE_ID, RESOURCE_REVISION_ID],
        )
        .expect("link Resource revision ancestry for replay fixture");
    transaction
        .execute(
            "UPDATE resources SET current_revision_id = 'route-instruction-resource-rev-2', version = 2 WHERE resource_id = ?1",
            [RESOURCE_ID],
        )
        .expect("advance Resource current head for replay fixture");
    transaction
        .commit()
        .expect("Resource head advance fixture commits");

    let replay = post_instruction_revision(
        router.clone(),
        InstructionRouteWrite {
            workspace_id: WORKSPACE_ID,
            expected_version: 1,
            request_id: "instruction-route-request-1",
            parent_revisions: vec![],
            resource_id: RESOURCE_ID,
            resource_revision_id: RESOURCE_REVISION_ID,
            content_digest: &resource_blob.digest,
            content_ref: None,
        },
    )
    .await;
    assert_eq!(replay.status(), StatusCode::CREATED);
    let replay_body = to_bytes(replay.into_body(), 4096)
        .await
        .expect("bounded replay response");
    let replay_body: Value = serde_json::from_slice(&replay_body).expect("replay response JSON");
    assert_eq!(replay_body, first_body);

    let page_one = get_instruction_history(router.clone(), WORKSPACE_ID, None, 1).await;
    assert_eq!(page_one.status(), StatusCode::OK);
    let page_one = to_bytes(page_one.into_body(), 4096)
        .await
        .expect("bounded first history page");
    let page_one: Value = serde_json::from_slice(&page_one).expect("first history page JSON");
    assert_eq!(page_one["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(page_one["items"][0]["revision"], 1);
    let cursor = page_one["next_cursor"]
        .as_str()
        .expect("second history page has an opaque cursor")
        .to_owned();

    let page_two = get_instruction_history(router.clone(), WORKSPACE_ID, Some(&cursor), 1).await;
    assert_eq!(page_two.status(), StatusCode::OK);
    let page_two = to_bytes(page_two.into_body(), 4096)
        .await
        .expect("bounded second history page");
    let page_two: Value = serde_json::from_slice(&page_two).expect("second history page JSON");
    assert_eq!(page_two["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(page_two["items"][0]["revision"], 2);
    assert!(page_two["next_cursor"].is_null());

    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: "instruction-route-other-workspace".to_owned(),
            name: "Other instruction Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "instruction-route-other-created".to_owned(),
                origin_runtime_id: "instruction-route-runtime".to_owned(),
                hlc_timestamp: now.to_owned(),
                correlation_id: "instruction-route-other-created".to_owned(),
                causation_id: None,
                recorded_at: now.to_owned(),
            },
        })
        .expect("second owner Workspace commits");
    let foreign_cursor = get_instruction_history(
        router,
        "instruction-route-other-workspace",
        Some(&cursor),
        1,
    )
    .await;
    assert_eq!(foreign_cursor.status(), StatusCode::BAD_REQUEST);
    let foreign_cursor = to_bytes(foreign_cursor.into_body(), 1024)
        .await
        .expect("bounded invalid cursor response");
    let foreign_cursor: Value =
        serde_json::from_slice(&foreign_cursor).expect("invalid cursor response JSON");
    assert_eq!(foreign_cursor["code"], "INVALID_ARGUMENT");

    let workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("read instruction Workspace")
        .expect("Workspace persists");
    assert_eq!(workspace.current_instruction_revision, Some(2));
    assert_eq!(workspace.version, 4);
    assert_eq!(
        store
            .list_workspace_instruction_revisions(WORKSPACE_ID, 0, 10)
            .expect("list committed instruction history")
            .len(),
        2
    );
}

#[tokio::test]
async fn instruction_create_route_rejects_untrusted_resource_content_without_writes() {
    const WORKSPACE_ID: &str = "instruction-route-validation-workspace";
    const FOREIGN_WORKSPACE_ID: &str = "instruction-route-validation-foreign-workspace";
    const OWNER: &str = "instruction-route-validation-owner";
    const NOW: &str = "2026-10-11T14:00:00Z";

    let directory = tempfile::tempdir().expect("temporary instruction validation directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private validation fixture directory");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        IpcReplayTestKeys,
    ));
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        blobs.clone(),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Instruction validation Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "instruction-validation-workspace-created".to_owned(),
                origin_runtime_id: "instruction-validation-runtime".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "instruction-validation-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("owner Workspace commits");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: FOREIGN_WORKSPACE_ID.to_owned(),
            name: "Foreign instruction validation Workspace".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "instruction-validation-foreign-workspace-created".to_owned(),
                origin_runtime_id: "instruction-validation-runtime".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "instruction-validation-foreign-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("foreign fixture Workspace commits");

    let digest_resource = seed_route_instruction_resource(
        &directory,
        &blobs,
        WORKSPACE_ID,
        "instruction-bad-digest-resource",
        "instruction-bad-digest-revision",
        "text/plain",
        b"the actual instruction text",
    );
    let non_text_resource = seed_route_instruction_resource(
        &directory,
        &blobs,
        WORKSPACE_ID,
        "instruction-non-text-resource",
        "instruction-non-text-revision",
        "application/pdf",
        b"%PDF-1.4 fixture",
    );
    let invalid_utf8_resource = seed_route_instruction_resource(
        &directory,
        &blobs,
        WORKSPACE_ID,
        "instruction-invalid-utf8-resource",
        "instruction-invalid-utf8-revision",
        "text/plain",
        b"\xff\xfe",
    );
    let oversized_content = vec![b'x'; 64 * 1024 + 1];
    let oversized_resource = seed_route_instruction_resource(
        &directory,
        &blobs,
        WORKSPACE_ID,
        "instruction-oversized-resource",
        "instruction-oversized-revision",
        "text/plain",
        &oversized_content,
    );
    let historical_resource = seed_route_instruction_resource_with_history(
        &directory,
        &blobs,
        InstructionResourceHistoryFixture {
            workspace_id: WORKSPACE_ID,
            resource_id: "instruction-historical-resource",
            old_revision_id: "instruction-historical-revision",
            head_revision_id: "instruction-current-revision",
            old_content: b"historical instruction text",
            head_content: b"current instruction text",
        },
    );
    let foreign_resource = seed_route_instruction_resource(
        &directory,
        &blobs,
        FOREIGN_WORKSPACE_ID,
        "instruction-foreign-resource",
        "instruction-foreign-resource-revision",
        "text/plain",
        b"foreign Workspace instructions",
    );

    let router = workspace_instruction_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceInstructionApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: "instruction-validation-runtime".to_owned(),
        });
    let cases = [
        (
            "instruction-bad-digest-request",
            "instruction-bad-digest-resource",
            "instruction-bad-digest-revision",
            format!("sha256:{}", "0".repeat(64)),
        ),
        (
            "instruction-non-text-request",
            "instruction-non-text-resource",
            "instruction-non-text-revision",
            non_text_resource,
        ),
        (
            "instruction-invalid-utf8-request",
            "instruction-invalid-utf8-resource",
            "instruction-invalid-utf8-revision",
            invalid_utf8_resource,
        ),
        (
            "instruction-oversized-request",
            "instruction-oversized-resource",
            "instruction-oversized-revision",
            oversized_resource,
        ),
        (
            "instruction-historical-request",
            "instruction-historical-resource",
            "instruction-historical-revision",
            historical_resource,
        ),
    ];
    for (request_id, resource_id, revision_id, content_digest) in cases {
        let response = post_instruction_revision(
            router.clone(),
            InstructionRouteWrite {
                workspace_id: WORKSPACE_ID,
                expected_version: 1,
                request_id,
                parent_revisions: vec![],
                resource_id,
                resource_revision_id: revision_id,
                content_digest: &content_digest,
                content_ref: None,
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{request_id}");
        let body = to_bytes(response.into_body(), 1024)
            .await
            .expect("bounded Resource validation response");
        let body: Value = serde_json::from_slice(&body).expect("Resource validation JSON");
        assert_eq!(body["code"], "INVALID_ARGUMENT", "{request_id}");
    }
    let invalid_refs = [
        (
            "instruction-foreign-workspace-ref-request",
            json!({
                "workspace_id": FOREIGN_WORKSPACE_ID,
                "resource_id": "instruction-bad-digest-resource",
                "revision_id": "instruction-bad-digest-revision",
            }),
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
        ),
        (
            "instruction-cross-workspace-resource-request",
            json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": "instruction-foreign-resource",
                "revision_id": "instruction-foreign-resource-revision",
            }),
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
        ),
        (
            "instruction-missing-resource-request",
            json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": "instruction-does-not-exist",
                "revision_id": "instruction-does-not-exist-revision",
            }),
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
        ),
        (
            "instruction-missing-revision-request",
            json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": "instruction-bad-digest-resource",
                "revision_id": "instruction-does-not-exist-revision",
            }),
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
        ),
        (
            "instruction-unpinned-null-revision-request",
            json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": "instruction-bad-digest-resource",
                "revision_id": null,
            }),
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
        ),
        (
            "instruction-unpinned-missing-revision-request",
            json!({
                "workspace_id": WORKSPACE_ID,
                "resource_id": "instruction-bad-digest-resource",
            }),
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
        ),
    ];
    for (request_id, content_ref, expected_status, expected_code) in invalid_refs {
        let response = post_instruction_revision(
            router.clone(),
            InstructionRouteWrite {
                workspace_id: WORKSPACE_ID,
                expected_version: 1,
                request_id,
                parent_revisions: vec![],
                resource_id: "unused-default-resource",
                resource_revision_id: "unused-default-revision",
                content_digest: &foreign_resource,
                content_ref: Some(content_ref),
            },
        )
        .await;
        assert_eq!(response.status(), expected_status, "{request_id}");
        let body = to_bytes(response.into_body(), 1024)
            .await
            .expect("bounded ResourceRef rejection response");
        let body: Value = serde_json::from_slice(&body).expect("ResourceRef error JSON");
        assert_eq!(body["code"], expected_code, "{request_id}");
    }
    assert_ne!(digest_resource, format!("sha256:{}", "0".repeat(64)));

    let workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("read Workspace after rejected requests")
        .expect("Workspace remains present");
    assert_eq!(workspace.current_instruction_revision, None);
    assert_eq!(workspace.version, 1);
    assert!(
        store
            .list_workspace_instruction_revisions(WORKSPACE_ID, 0, 10)
            .expect("read rejected instruction history")
            .is_empty()
    );
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect rejected request state");
    let counts: (i64, i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM domain_events WHERE type = 'workspace.instructions.revision.created.v1'),
                (SELECT COUNT(*) FROM workspace_instruction_revisions WHERE workspace_id = ?1),
                (SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?2 AND request_id LIKE 'instruction-%-request')",
            rusqlite::params![WORKSPACE_ID, OWNER],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read event/revision/receipt counts");
    assert_eq!(counts, (0, 0, 0));
}

#[tokio::test]
async fn authenticated_unix_ipc_exchange_reaches_the_shared_operator_middleware() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = std::fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");

    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID peer is accepted");
        dispatch_ipc_exchange(
            readiness_router(),
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(1),
        )
        .await
        .expect("Operator response is framed and sent");
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-auth-boundary-1".to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/operator/readiness".to_owned(),
            headers: Vec::<LogicalHeader>::new(),
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded readiness request");
    client
        .write_request(&request)
        .await
        .expect("request writes");
    let response = client
        .read_response(&client_budget)
        .await
        .expect("correlated response reads");
    assert_eq!(response.header.status, StatusCode::OK.as_u16());
    let body: Value = serde_json::from_slice(response.body()).expect("JSON readiness body");
    assert_eq!(body, json!({"operator_state": "SERVING"}));
    server.await.expect("server task completes");
}

#[tokio::test]
async fn authenticated_unix_ipc_runtime_offer_route_requires_workspace_context() {
    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = fs::metadata(directory.path())
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
    let router = runtime_offers_router(RuntimeOffersApiState::new(
        store,
        "ipc-offer-owner".to_owned(),
    ));
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID peer is accepted");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await
        .expect("Runtime offer response is framed");
    });
    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "runtime-offers-context".to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/runtimes/runtime-ipc/offers".to_owned(),
            headers: Vec::<LogicalHeader>::new(),
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded Runtime offers request");
    client
        .write_request(&request)
        .await
        .expect("request frame writes");
    let response = client
        .read_response(&client_budget)
        .await
        .expect("response frame is correlated");
    assert_eq!(response.header.request_id, "runtime-offers-context");
    assert_eq!(response.header.status, StatusCode::BAD_REQUEST.as_u16());
    let body: Value = serde_json::from_slice(response.body()).expect("offer error is JSON");
    assert_eq!(body["code"], "INVALID_ARGUMENT");
    assert_eq!(body["message"], "X-Workspace-ID is required");
    server.await.expect("Runtime offers IPC exchange completes");
}

#[tokio::test]
async fn authenticated_unix_runtime_offers_returns_workspace_authorized_public_page() {
    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = fs::metadata(directory.path())
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
    let runtime_id = "runtime-ipc-offers";
    let incarnation_id = "incarnation-ipc-offers";
    let workspace_id = "workspace-ipc-offers";
    let owner_id = "ipc-offer-owner";
    let now = "2026-10-11T10:00:00Z";
    let expires_at = (time::OffsetDateTime::now_utc() + time::Duration::hours(1))
        .format(&time::format_description::well_known::Rfc3339)
        .expect("future expiration timestamp formats");
    let public_key = [9_u8; 32];
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
                os_boot_id: Some("boot-ipc-offers".to_owned()),
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
            name: "IPC offers fixture".to_owned(),
            owner_principal_id: owner_id.to_owned(),
            event: EventContext {
                event_id: "ipc-offers-workspace-event".to_owned(),
                origin_runtime_id: runtime_id.to_owned(),
                hlc_timestamp: now.to_owned(),
                correlation_id: "ipc-offers-create".to_owned(),
                causation_id: None,
                recorded_at: now.to_owned(),
            },
        })
        .expect("Workspace commits");
    store
        .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: owner_id.to_owned(),
                request_id: "ipc-offers-enroll".to_owned(),
                request_payload: json!({"workspace_id": workspace_id}),
            },
            expected_workspace_version: 1,
            runtime_workspace_binding_id: "binding-ipc-offers".to_owned(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            now: now.to_owned(),
            correlation_id: "ipc-offers-enroll-correlation".to_owned(),
        })
        .expect("Workspace grants the local Runtime control-plane binding");
    store
        .put_agent_profile(
            storage_core::AgentProfileRecord {
                agent_profile_id: "profile-ipc-offers".to_owned(),
                provider_key: "test-provider".to_owned(),
                display_name: "Test provider".to_owned(),
                discovered_at: now.to_owned(),
            },
            vec![storage_core::AgentEndpointRecord {
                endpoint_id: "endpoint-ipc-offers".to_owned(),
                agent_profile_id: "profile-ipc-offers".to_owned(),
                protocol: "ACP".to_owned(),
                topology: "PROCESS_ADAPTER".to_owned(),
                protocol_version: Some("1".to_owned()),
                capabilities: json!({"streaming": true}),
            }],
        )
        .expect("stable endpoint identity persists");
    store
        .register_local_endpoint_binding(LocalAgentEndpointBindingInput {
            endpoint_id: "endpoint-ipc-offers".to_owned(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            endpoint_ref: "ipc:///private/locator-secret".to_owned(),
            observed_at: now.to_owned(),
            expires_at: Some(expires_at.clone()),
        })
        .expect("private locator remains in Runtime-local binding");
    store
        .publish_runtime_offer(RuntimeOfferRecord {
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            offer_kind: "AGENT_ENDPOINT".to_owned(),
            offer_ref: "endpoint-ipc-offers".to_owned(),
            compatible: true,
            readiness: "AVAILABLE".to_owned(),
            constraints: json!({"test": true}),
            observed_at: now.to_owned(),
            expires_at,
        })
        .expect("Runtime publishes advisory offer");

    let router = runtime_offers_router(RuntimeOffersApiState::new(store, owner_id.to_owned()));
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID peer is accepted");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await
        .expect("offer response is framed");
    });
    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "runtime-offers-positive".to_owned(),
            method: "GET".to_owned(),
            path_and_query: format!("/v1/runtimes/{runtime_id}/offers?limit=10"),
            headers: vec![LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: workspace_id.to_owned(),
            }],
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded offer request");
    client
        .write_request(&request)
        .await
        .expect("request frame writes");
    let response = client
        .read_response(&client_budget)
        .await
        .expect("response frame is correlated");
    assert_eq!(response.header.status, StatusCode::OK.as_u16());
    assert!(
        response
            .header
            .headers
            .iter()
            .any(|header| { header.name == "cache-control" && header.value == "no-store" })
    );
    let body: Value = serde_json::from_slice(response.body()).expect("offer page is JSON");
    assert_eq!(body["items"].as_array().expect("items array").len(), 1);
    assert_eq!(body["items"][0]["offer_ref"], "endpoint-ipc-offers");
    assert!(body.get("next_cursor").is_some());
    assert!(!body.to_string().contains("locator-secret"));
    server.await.expect("offer IPC exchange completes");
}

#[tokio::test]
async fn runtime_inventory_stream_is_authorized_and_emits_bounded_invalidations() {
    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
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
    let runtime_id = "runtime-inventory-stream";
    let incarnation_id = "incarnation-inventory-stream";
    let workspace_id = "workspace-inventory-stream";
    let owner_id = "owner-inventory-stream";
    let now = "2026-10-11T10:00:00Z";
    let public_key = [17_u8; 32];
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
                os_boot_id: Some("boot-inventory-stream".to_owned()),
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
            name: "Inventory stream fixture".to_owned(),
            owner_principal_id: owner_id.to_owned(),
            event: EventContext {
                event_id: "inventory-stream-workspace-event".to_owned(),
                origin_runtime_id: runtime_id.to_owned(),
                hlc_timestamp: now.to_owned(),
                correlation_id: "inventory-stream-create".to_owned(),
                causation_id: None,
                recorded_at: now.to_owned(),
            },
        })
        .expect("Workspace commits");
    store
        .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: owner_id.to_owned(),
                request_id: "inventory-stream-enroll".to_owned(),
                request_payload: json!({"workspace_id": workspace_id}),
            },
            expected_workspace_version: 1,
            runtime_workspace_binding_id: "binding-inventory-stream".to_owned(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            now: now.to_owned(),
            correlation_id: "inventory-stream-enroll-correlation".to_owned(),
        })
        .expect("Workspace grants the local Runtime binding");

    let expires_at = (time::OffsetDateTime::now_utc() + time::Duration::hours(1))
        .format(&time::format_description::well_known::Rfc3339)
        .expect("offer expiration formats");
    store
        .put_agent_profile(
            storage_core::AgentProfileRecord {
                agent_profile_id: "profile-inventory-stream".to_owned(),
                provider_key: "test-provider".to_owned(),
                display_name: "Test provider".to_owned(),
                discovered_at: now.to_owned(),
            },
            vec![storage_core::AgentEndpointRecord {
                endpoint_id: "endpoint-inventory-stream".to_owned(),
                agent_profile_id: "profile-inventory-stream".to_owned(),
                protocol: "ACP".to_owned(),
                topology: "PROCESS_ADAPTER".to_owned(),
                protocol_version: Some("1".to_owned()),
                capabilities: json!({"streaming": true}),
            }],
        )
        .expect("stable endpoint identity persists");
    store
        .register_local_endpoint_binding(LocalAgentEndpointBindingInput {
            endpoint_id: "endpoint-inventory-stream".to_owned(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            endpoint_ref: "ipc:///private/endpoint-locator".to_owned(),
            observed_at: now.to_owned(),
            expires_at: Some(expires_at.clone()),
        })
        .expect("private endpoint locator remains Runtime-local");

    let revision = Arc::new(std::sync::atomic::AtomicU64::new(1));
    let (shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(false);
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let router = Router::new()
        .route("/v1/stream", post(subscribe_stream))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(StreamApiState {
            store: store.clone(),
            principal_id: owner_id.to_owned(),
            runtime_id: runtime_id.to_owned(),
            local_incarnation_id: incarnation_id.to_owned(),
            runtime_inventory_revision: Arc::clone(&revision),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(1)),
            stream_shutdown: shutdown_receiver,
            task_stream_test_gate: None,
            task_stream_snapshot_test_gate: None,
        });
    // Initial and resumed subscriptions, Runtime deauthorization, and a denied
    // Workspace request exercise the production IPC dispatcher on every path.
    let server = tokio::spawn(async move {
        for _ in 0..11 {
            let connection = listener
                .accept(|peer| peer.uid == expected_uid)
                .await
                .expect("same-UID stream peer is accepted");
            let _ = dispatch_ipc_exchange(
                router.clone(),
                OperatorIpcConnection::Unix(connection),
                server_budget.clone(),
                Duration::from_secs(2),
            )
            .await;
        }
    });
    let request_body = |resume_cursor: Option<&str>| {
        let mut body = json!({
            "workspace_id": workspace_id,
            "runtime_ids": [runtime_id],
            "projection_types": ["runtime_inventory"]
        });
        if let Some(cursor) = resume_cursor {
            body["resume_cursor"] = json!(cursor);
        }
        serde_json::to_vec(&body).expect("stream request serializes")
    };
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let stream_request = |request_id: &str, body: Vec<u8>| {
        RequestFrame::new(
            RequestHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: request_id.to_owned(),
                method: "POST".to_owned(),
                path_and_query: "/v1/stream".to_owned(),
                headers: vec![
                    LogicalHeader {
                        name: "x-workspace-id".to_owned(),
                        value: workspace_id.to_owned(),
                    },
                    LogicalHeader {
                        name: "content-type".to_owned(),
                        value: "application/json".to_owned(),
                    },
                ],
                body_length: body.len() as u64,
            },
            body,
            &client_budget,
        )
        .unwrap_or_else(|error| panic!("bounded stream request {request_id} failed: {error:?}"))
    };
    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    client
        .write_request(&stream_request(
            "runtime-inventory-initial",
            request_body(None),
        ))
        .await
        .expect("authorized stream request writes");
    let handshake = client
        .read_stream_response(&client_budget)
        .await
        .expect("stream handshake reads");
    assert_eq!(handshake.header.status, StatusCode::OK.as_u16());
    let ready: Value = serde_json::from_slice(
        client
            .read_stream_message(&client_budget)
            .await
            .expect("ready frame reads")
            .expect("authorized stream remains open")
            .body(),
    )
    .expect("ready frame is JSON");
    assert_eq!(ready["type"], "stream.ready");
    let initial: Value = serde_json::from_slice(
        client
            .read_stream_message(&client_budget)
            .await
            .expect("initial inventory invalidation arrives")
            .expect("stream emits initial invalidation")
            .body(),
    )
    .expect("initial invalidation is JSON");
    assert_eq!(initial["projection_type"], "runtime_inventory");
    assert_eq!(initial["payload"]["runtime_id"], runtime_id);
    assert_eq!(initial["payload"]["change_kind"], "OFFERS_CHANGED");
    assert_eq!(initial["projection_revision"], 1);
    assert_eq!(initial["cursor"], ready["cursor"]);
    assert_eq!(
        initial["payload"]
            .as_object()
            .expect("closed payload")
            .len(),
        2
    );

    super::publish_runtime_offer_and_notify(
        &store,
        &revision,
        RuntimeOfferRecord {
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            offer_kind: "AGENT_ENDPOINT".to_owned(),
            offer_ref: "endpoint-inventory-stream".to_owned(),
            compatible: true,
            readiness: "AVAILABLE".to_owned(),
            constraints: json!({"test": true}),
            observed_at: now.to_owned(),
            expires_at,
        },
        "test Runtime offer could not be recorded",
    )
    .expect("real offer persistence advances the stream revision");
    let update: Value = serde_json::from_slice(
        client
            .read_stream_message(&client_budget)
            .await
            .expect("offer-change invalidation arrives")
            .expect("stream emits changed inventory")
            .body(),
    )
    .expect("update is JSON");
    assert_eq!(update["projection_revision"], 2);
    assert_ne!(update["cursor"], initial["cursor"]);
    assert!(!update.to_string().contains("locator"));
    drop(client);

    let cursor_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(ready["cursor"].as_str().expect("ready cursor"))
        .expect("ready cursor decodes");
    let cursor_payload: Value =
        serde_json::from_slice(&cursor_bytes).expect("cursor payload is JSON");
    let encode_cursor = |payload: &Value| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(payload).expect("cursor serializes"))
    };
    let mut malformed_cursors = vec!["malformed".to_owned()];
    for (field, value) in [
        ("version", json!(2)),
        ("workspace_id", json!("workspace-other")),
        ("runtime_id", json!("runtime-other")),
        ("runtime_incarnation_id", json!("incarnation-old")),
        (
            "projection_version",
            json!(
                cursor_payload["projection_version"]
                    .as_u64()
                    .expect("projection version")
                    + 1
            ),
        ),
        ("revision", json!(3)),
    ] {
        let mut payload = cursor_payload.clone();
        payload[field] = value;
        malformed_cursors.push(encode_cursor(&payload));
    }
    for (index, malformed_cursor) in malformed_cursors.into_iter().enumerate() {
        let mut resync_client = endpoint
            .connect(|peer| peer.uid == expected_uid)
            .await
            .expect("daemon peer authenticates for resync");
        let request_id = format!("runtime-inventory-resync-{index}");
        resync_client
            .write_request(&stream_request(
                &request_id,
                request_body(Some(&malformed_cursor)),
            ))
            .await
            .expect("invalid-cursor request writes");
        let handshake = resync_client
            .read_stream_response(&client_budget)
            .await
            .expect("resync handshake reads");
        assert_eq!(handshake.header.status, StatusCode::OK.as_u16());
        let _ready: Value = serde_json::from_slice(
            resync_client
                .read_stream_message(&client_budget)
                .await
                .expect("resync ready frame arrives")
                .expect("stream emits ready")
                .body(),
        )
        .expect("ready frame is JSON");
        let resync: Value = serde_json::from_slice(
            resync_client
                .read_stream_message(&client_budget)
                .await
                .expect("resync marker arrives")
                .expect("stream emits resync marker")
                .body(),
        )
        .expect("resync marker is JSON");
        assert_eq!(resync["type"], "stream.resync_required");
        let current: Value = serde_json::from_slice(
            resync_client
                .read_stream_message(&client_budget)
                .await
                .expect("fresh invalidation arrives after resync")
                .expect("stream emits fresh invalidation")
                .body(),
        )
        .expect("fresh invalidation is JSON");
        assert_eq!(current["projection_revision"], 2);
        assert_eq!(current["payload"]["runtime_id"], runtime_id);
        assert!(!current.to_string().contains("locator"));
        drop(resync_client);
    }

    let mut revoked_client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer authenticates for the active authorization session");
    revoked_client
        .write_request(&stream_request(
            "runtime-inventory-revocation",
            request_body(None),
        ))
        .await
        .expect("active authorization request writes");
    let handshake = revoked_client
        .read_stream_response(&client_budget)
        .await
        .expect("active authorization handshake reads");
    assert_eq!(handshake.header.status, StatusCode::OK.as_u16());
    let _ready: Value = serde_json::from_slice(
        revoked_client
            .read_stream_message(&client_budget)
            .await
            .expect("active authorization ready frame reads")
            .expect("authorized stream remains open")
            .body(),
    )
    .expect("active authorization ready frame is JSON");
    let _initial: Value = serde_json::from_slice(
        revoked_client
            .read_stream_message(&client_budget)
            .await
            .expect("active authorization invalidation reads")
            .expect("authorized stream emits initial invalidation")
            .body(),
    )
    .expect("active authorization invalidation is JSON");

    let connection =
        rusqlite::Connection::open(&database).expect("fixture database opens for owner revocation");
    connection
        .execute(
            "UPDATE workspaces SET owner_principal_id='owner-revoked-during-inventory-stream' WHERE workspace_id=?1",
            [workspace_id],
        )
        .expect("Workspace ownership is revoked during the stream");
    drop(connection);
    let closed = tokio::time::timeout(
        Duration::from_secs(2),
        revoked_client.read_stream_message(&client_budget),
    )
    .await
    .expect("Runtime deauthorization closes the stream promptly")
    .expect("deauthorized stream reaches clean EOF");
    assert!(
        closed.is_none(),
        "a Runtime inventory frame followed deauthorization: {}",
        closed
            .as_ref()
            .map(|frame| String::from_utf8_lossy(frame.body()).into_owned())
            .unwrap_or_default()
    );
    drop(revoked_client);

    let connection = rusqlite::Connection::open(&database)
        .expect("fixture database opens to restore test ownership");
    connection
        .execute(
            "UPDATE workspaces SET owner_principal_id=?1 WHERE workspace_id=?2",
            rusqlite::params![owner_id, workspace_id],
        )
        .expect("fixture Workspace ownership is restored after observing closure");
    drop(connection);

    let mut post_revocation = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer authenticates after stream-slot cleanup");
    post_revocation
        .write_request(&stream_request(
            "runtime-inventory-slot-released",
            request_body(None),
        ))
        .await
        .expect("post-revocation request writes");
    let resumed_handshake = post_revocation
        .read_stream_response(&client_budget)
        .await
        .expect("post-revocation response reads");
    assert_eq!(resumed_handshake.header.status, StatusCode::OK.as_u16());
    let _ready: Value = serde_json::from_slice(
        post_revocation
            .read_stream_message(&client_budget)
            .await
            .expect("post-revocation ready frame reads")
            .expect("stream admission slot is reusable")
            .body(),
    )
    .expect("post-revocation ready frame is JSON");
    let _initial: Value = serde_json::from_slice(
        post_revocation
            .read_stream_message(&client_budget)
            .await
            .expect("post-revocation invalidation reads")
            .expect("reopened stream emits an initial invalidation")
            .body(),
    )
    .expect("post-revocation invalidation is JSON");
    drop(post_revocation);

    let mut denied_client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer authenticates for denied request");
    let denied_body = serde_json::to_vec(&json!({
        "workspace_id": "unbound-workspace",
        "runtime_ids": [runtime_id],
        "projection_types": ["runtime_inventory"],
        "resume_cursor": "malformed"
    }))
    .expect("denied request serializes");
    let denied_request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "runtime-inventory-denied".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: "unbound-workspace".to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: denied_body.len() as u64,
        },
        denied_body,
        &client_budget,
    )
    .expect("bounded denied request");
    denied_client
        .write_request(&denied_request)
        .await
        .expect("denied request writes");
    let denied = denied_client
        .read_response(&client_budget)
        .await
        .expect("unauthorized subscription response reads");
    assert_eq!(denied.header.status, StatusCode::NOT_FOUND.as_u16());
    drop(denied_client);
    let _ = shutdown_sender.send(true);
    server.await.expect("Runtime inventory IPC server exits");
}

#[tokio::test]
async fn authenticated_unix_runtime_lifecycle_uses_current_sqlite_incarnation() {
    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = fs::metadata(directory.path())
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

    let now = "2026-10-11T10:00:00Z".to_owned();
    let public_key = [7_u8; 32];
    let device_id = format!(
        "ed25519-sha256:{}",
        hex::encode(sha2::Sha256::digest(public_key))
    );
    let runtime_id = "runtime-ipc-lifecycle";
    let incarnation_id = "incarnation-ipc-current";
    store
        .register_local_incarnation(
            storage_core::RuntimeRecord {
                runtime_id: runtime_id.to_owned(),
                device_identity: storage_core::DeviceIdentityRecord {
                    device_id,
                    public_key: format!("ed25519:{}", hex::encode(public_key)),
                    key_version: 1,
                    issued_at: now.clone(),
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
                last_seen: now.clone(),
            },
            storage_core::RuntimeIncarnationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                runtime_id: runtime_id.to_owned(),
                process_started_at: now.clone(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            storage_core::RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: incarnation_id.to_owned(),
                os_boot_id: Some("test-boot-id".to_owned()),
                observed_at: now.clone(),
            },
        )
        .expect("Runtime and incarnation register atomically");
    store
        .transition_local_incarnation(storage_core::RuntimeIncarnationStateUpdate {
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: incarnation_id.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: now.clone(),
            stopped_at: None,
        })
        .expect("current incarnation becomes degraded");

    let blockers = vec![RuntimeLifecycleBlocker {
        code: "TASK_ATTEMPT_ADMISSION_NOT_INTEGRATED".to_owned(),
        created_at: now,
    }];
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let current_router = runtime_lifecycle_router(RuntimeLifecycleApiState::new(
        store.clone(),
        runtime_id.to_owned(),
        incarnation_id.to_owned(),
        blockers.clone(),
    ));
    let stale_router = runtime_lifecycle_router(RuntimeLifecycleApiState::new(
        store.clone(),
        runtime_id.to_owned(),
        "incarnation-ipc-stale".to_owned(),
        blockers,
    ));
    let server = tokio::spawn(async move {
        for router in [current_router, stale_router] {
            let connection = listener
                .accept(|peer| peer.uid == expected_uid)
                .await
                .expect("same-UID peer is accepted");
            dispatch_ipc_exchange(
                router,
                OperatorIpcConnection::Unix(connection),
                server_budget.clone(),
                Duration::from_secs(2),
            )
            .await
            .expect("lifecycle response is framed");
        }
    });

    async fn send_lifecycle_request(
        endpoint: &UnixEndpoint,
        expected_uid: u32,
        request_id: &str,
    ) -> operator_ipc::ResponseFrame {
        let mut client = endpoint
            .connect(|peer| peer.uid == expected_uid)
            .await
            .expect("daemon peer is authenticated");
        let budget =
            InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
        let request = RequestFrame::new(
            RequestHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: request_id.to_owned(),
                method: "GET".to_owned(),
                path_and_query: "/v1/runtime-lifecycle".to_owned(),
                headers: Vec::<LogicalHeader>::new(),
                body_length: 0,
            },
            Vec::new(),
            &budget,
        )
        .expect("bounded lifecycle request");
        client
            .write_request(&request)
            .await
            .expect("request frame writes");
        client
            .read_response(&budget)
            .await
            .expect("response frame is correlated")
    }

    let current = send_lifecycle_request(&endpoint, expected_uid, "lifecycle-current").await;
    assert_eq!(current.header.request_id, "lifecycle-current");
    assert_eq!(current.header.status, StatusCode::OK.as_u16());
    assert!(
        current
            .header
            .headers
            .iter()
            .any(|header| header.name == "cache-control" && header.value == "no-store")
    );
    let body: Value = serde_json::from_slice(current.body()).expect("lifecycle response JSON");
    assert_eq!(body["runtime_id"], runtime_id);
    assert_eq!(body["startup_policy"], "MANUAL");
    assert_eq!(body["availability"], "DEGRADED");
    assert_eq!(
        body["incarnation"]["runtime_incarnation_id"],
        incarnation_id
    );
    assert_eq!(body["incarnation"]["recovery_state"], "DEGRADED");
    assert_eq!(body["incarnation"]["version"], 2);
    assert_eq!(body["blockers"][0]["code"], "DEPENDENCY_UNAVAILABLE");
    assert_eq!(
        body["blockers"][0]["safe_message"],
        "Task execution admission is unavailable in this Runtime."
    );
    assert!(
        body.to_string()
            .find("TASK_ATTEMPT_ADMISSION_NOT_INTEGRATED")
            .is_none()
    );
    assert!(body.get("dependent_tasks").is_none());
    assert!(body.to_string().find("test-boot-id").is_none());

    let stale = send_lifecycle_request(&endpoint, expected_uid, "lifecycle-stale").await;
    assert_eq!(stale.header.request_id, "lifecycle-stale");
    assert_eq!(
        stale.header.status,
        StatusCode::SERVICE_UNAVAILABLE.as_u16()
    );
    let body: Value = serde_json::from_slice(stale.body()).expect("stale error JSON");
    assert_eq!(body["code"], "RUNTIME_UNAVAILABLE");
    assert_eq!(
        body["message"],
        "Current Runtime incarnation could not be verified"
    );
    server
        .await
        .expect("server handles both lifecycle requests");
    let persisted = store
        .read_local_lifecycle_snapshot(runtime_id, incarnation_id)
        .expect("post-request lifecycle read succeeds")
        .expect("the same incarnation remains current");
    assert_eq!(persisted.availability, "DEGRADED");
    assert_eq!(persisted.incarnation.version, 2);
}

#[tokio::test]
async fn unix_ipc_replay_uses_stable_idempotency_key_across_distinct_frame_ids() {
    tokio::time::timeout(
        // Keep a hang guard while allowing the real SQLite/socket fixture to settle
        // when the full workspace test runner is compiling and executing in parallel.
        Duration::from_secs(60),
        unix_ipc_replay_uses_stable_idempotency_key_across_distinct_frame_ids_inner(),
    )
    .await
    .expect("Unix IPC idempotency replay test must settle within its deadline");
}

async fn unix_ipc_replay_uses_stable_idempotency_key_across_distinct_frame_ids_inner() {
    let directory = tempfile::tempdir().expect("temporary IPC data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let database = directory.path().join("state.sqlite3");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        IpcReplayTestKeys,
    ));
    let store = SqliteWorkspaceStore::open(
        &database,
        blobs,
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite Workspace store opens");

    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let router = Router::new()
        .route("/v1/workspaces", post(super::create_workspace))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceCreateApiState {
            store: store.clone(),
            principal_id: "ipc-replay-test-owner".to_owned(),
            runtime_id: "ipc-replay-test-runtime".to_owned(),
        });
    let server = tokio::spawn(async move {
        for _ in 0..3 {
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
            .expect("Workspace command response is framed");
        }
    });

    async fn send_workspace_create(
        endpoint: &UnixEndpoint,
        expected_uid: u32,
        frame_request_id: &str,
        idempotency_key: &str,
        name: &str,
    ) -> operator_ipc::ResponseFrame {
        let mut client = endpoint
            .connect(|peer| peer.uid == expected_uid)
            .await
            .expect("daemon peer is authenticated");
        let client_budget =
            InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
        let body = serde_json::to_vec(&json!({"name": name})).expect("serialize request body");
        let request = RequestFrame::new(
            RequestHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: frame_request_id.to_owned(),
                method: "POST".to_owned(),
                path_and_query: "/v1/workspaces".to_owned(),
                headers: vec![
                    LogicalHeader {
                        name: "content-type".to_owned(),
                        value: "application/json".to_owned(),
                    },
                    LogicalHeader {
                        name: "idempotency-key".to_owned(),
                        value: idempotency_key.to_owned(),
                    },
                ],
                body_length: body.len() as u64,
            },
            body,
            &client_budget,
        )
        .expect("bounded Workspace command frame");
        client
            .write_request(&request)
            .await
            .expect("request frame writes");
        client
            .read_response(&client_budget)
            .await
            .expect("response frame correlates to this exchange")
    }

    let first = send_workspace_create(
        &endpoint,
        expected_uid,
        "workspace-create-frame-1",
        "workspace-create-command-1",
        "First workspace",
    )
    .await;
    let replay = send_workspace_create(
        &endpoint,
        expected_uid,
        "workspace-create-frame-2",
        "workspace-create-command-1",
        "First workspace",
    )
    .await;
    let changed_payload = send_workspace_create(
        &endpoint,
        expected_uid,
        "workspace-create-frame-3",
        "workspace-create-command-1",
        "Changed workspace",
    )
    .await;

    assert_eq!(first.header.request_id, "workspace-create-frame-1");
    assert_eq!(replay.header.request_id, "workspace-create-frame-2");
    assert_eq!(
        changed_payload.header.request_id,
        "workspace-create-frame-3"
    );
    assert_eq!(first.header.status, StatusCode::CREATED.as_u16());
    assert_eq!(replay.header.status, StatusCode::CREATED.as_u16());
    assert_eq!(first.body(), replay.body());
    let first_correlation = first
        .header
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("x-correlation-id"))
        .map(|header| header.value.as_str());
    let replay_correlation = replay
        .header
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("x-correlation-id"))
        .map(|header| header.value.as_str());
    assert!(first_correlation.is_some());
    assert_eq!(replay_correlation, first_correlation);
    assert_eq!(changed_payload.header.status, StatusCode::CONFLICT.as_u16());
    let conflict_body: Value =
        serde_json::from_slice(changed_payload.body()).expect("JSON conflict body");
    assert_eq!(conflict_body["code"], "CONFLICT");
    server.await.expect("all IPC exchanges complete");

    let workspaces = store.list_workspaces().expect("list committed Workspaces");
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].name, "First workspace");
}

#[tokio::test]
async fn stream_session_admission_denies_when_capacity_is_exhausted() {
    let directory = tempfile::tempdir().expect("temporary stream data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory");
    let database = directory.path().join("state.sqlite3");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        IpcReplayTestKeys,
    ));
    let store = SqliteWorkspaceStore::open(
        &database,
        blobs,
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("SQLite store opens");
    let (_shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(false);
    let router = Router::new()
        .route("/v1/stream", post(subscribe_stream))
        .with_state(StreamApiState {
            store,
            principal_id: "stream-capacity-owner".to_owned(),
            runtime_id: "stream-capacity-runtime".to_owned(),
            local_incarnation_id: "stream-capacity-incarnation".to_owned(),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(0)),
            stream_shutdown: shutdown_receiver,
            task_stream_test_gate: None,
            task_stream_snapshot_test_gate: None,
        });
    let body = serde_json::to_vec(&json!({
        "workspace_id": "stream-capacity-workspace",
        "conversation_ids": ["stream-capacity-conversation"],
        "projection_types": ["conversation_presentation"]
    }))
    .expect("subscription serializes");

    let response = router
        .oneshot(
            Request::post("/v1/stream")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-workspace-id", "stream-capacity-workspace")
                .body(Body::from(body))
                .expect("request is valid"),
        )
        .await
        .expect("router responds");
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let body = to_bytes(response.into_body(), 4096)
        .await
        .expect("bounded rejection body reads");
    let error: Value = serde_json::from_slice(&body).expect("error response is JSON");
    assert_eq!(error["code"], "CAPACITY_EXCEEDED");
}

#[tokio::test]
async fn authenticated_unix_stream_sends_correlated_handshake_and_authorized_snapshot() {
    use domain_workspace::{
        ChangeReplicationPolicy, CreateWorkspace, EventContext, WorkspaceService,
    };
    use rusqlite::Connection;
    use storage_core::ReplicationPolicy;

    const OWNER: &str = "ipc-stream-owner";
    const WORKSPACE: &str = "ipc-stream-workspace";
    const CONVERSATION: &str = "ipc-stream-conversation";
    const RUNTIME: &str = "ipc-stream-runtime";
    const NOW: &str = "2026-10-11T12:00:00.000000000Z";

    let directory = tempfile::tempdir().expect("temporary IPC data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory");
    let expected_uid = fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let database = directory.path().join("state.sqlite3");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        IpcReplayTestKeys,
    ));
    let store = SqliteWorkspaceStore::open(
        &database,
        blobs,
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "IPC stream fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "ipc-stream-workspace-event".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "ipc-stream-correlation".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits");
    let connection = Connection::open(&database).expect("fixture database opens");
    connection
        .execute(
            "INSERT INTO conversations(conversation_id,workspace_id,created_at) VALUES(?1,?2,?3)",
            rusqlite::params![CONVERSATION, WORKSPACE, NOW],
        )
        .expect("Conversation fixture persists");
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('ipc-stream-message',?1,'{}','AGENT','ipc-stream-turn','[{\"kind\":\"TEXT\",\"text\":\"authorized snapshot\"}]',?2)",
            rusqlite::params![CONVERSATION, NOW],
        )
        .expect("semantic message fixture persists");
    for index in 0..100 {
        connection.execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,content_json,resource_refs_json,created_at) VALUES(?1,?2,'{}','USER','[]','[]',?3)",
            rusqlite::params![
                format!("ipc-stream-history-{index:03}"),
                CONVERSATION,
                format!("2026-10-11T12:{:02}:{:02}.000000000Z", (index + 1) / 60, (index + 1) % 60),
            ],
        ).expect("bounded prior Conversation history persists");
    }
    drop(connection);

    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let stream_admission = Arc::new(tokio::sync::Semaphore::new(2));
    let stream_admission_observer = Arc::clone(&stream_admission);
    let (idle_disconnect_sender, idle_disconnect_receiver) = tokio::sync::oneshot::channel();
    let (_shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(false);
    let router = Router::new()
        .route("/v1/stream", post(subscribe_stream))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(StreamApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: "ipc-stream-incarnation".to_owned(),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_admission,
            stream_shutdown: shutdown_receiver,
            task_stream_test_gate: None,
            task_stream_snapshot_test_gate: None,
        });

    let race_router = router.clone();
    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID peer accepted");
        let settlement = dispatch_ipc_exchange(
            router.clone(),
            OperatorIpcConnection::Unix(connection),
            server_budget.clone(),
            Duration::from_secs(2),
        )
        .await;
        let _ = idle_disconnect_sender.send(settlement);

        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID peer accepted for shutdown session");
        let settlement = dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(
            settlement,
            Ok(OperatorRequestSettlement::Complete),
            "reconnect client disconnect ends its stream cleanly"
        );
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "conversation_ids": [CONVERSATION],
        "projection_types": ["conversation_presentation"]
    }))
    .expect("subscribe body serializes");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-stream-request-1".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: WORKSPACE.to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body,
        &client_budget,
    )
    .expect("bounded stream subscription");
    client
        .write_request(&request)
        .await
        .expect("request writes");
    let handshake = client
        .read_stream_response(&client_budget)
        .await
        .expect("stream handshake reads");
    assert_eq!(handshake.header.request_id, "ipc-stream-request-1");
    assert_eq!(handshake.header.status, StatusCode::OK.as_u16());
    assert_eq!(handshake.body().len(), 0);

    let ready = client
        .read_stream_message(&client_budget)
        .await
        .expect("ready frame reads")
        .expect("session remains open");
    let ready: Value = serde_json::from_slice(ready.body()).expect("ready JSON");
    assert_eq!(ready["type"], "stream.ready");
    assert_eq!(ready["workspace_id"], WORKSPACE);

    let snapshot = client
        .read_stream_message(&client_budget)
        .await
        .expect("snapshot frame reads")
        .expect("session remains open");
    let snapshot: Value = serde_json::from_slice(snapshot.body()).expect("snapshot JSON");
    assert_eq!(snapshot["type"], "projection");
    assert_eq!(snapshot["workspace_id"], WORKSPACE);
    assert_eq!(snapshot["payload"]["conversation_id"], CONVERSATION);
    assert_eq!(
        snapshot["payload"]["items"][0]["message"]["message_id"],
        "ipc-stream-message"
    );
    assert_eq!(snapshot["payload"]["items"][0]["message"]["role"], "AGENT");

    let connection = Connection::open(&database).expect("fixture database opens for live message");
    connection.execute(
        "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('ipc-stream-message-live',?1,'{}','AGENT','ipc-stream-turn-live','[{\"kind\":\"TEXT\",\"text\":\"new committed answer\"}]','2026-10-11T12:02:00.000000000Z')",
        [CONVERSATION],
    ).expect("new semantic message persists");
    drop(connection);

    WorkspaceService::new(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: WORKSPACE.to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: Vec::new(),
            event: EventContext {
                event_id: "ipc-stream-policy-event".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: "2026-10-11T12:00:01.000000000Z".to_owned(),
                correlation_id: "ipc-stream-policy-correlation".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-11T12:00:01.000000000Z".to_owned(),
            },
        })
        .expect("Workspace event commits");
    let update = tokio::time::timeout(
        Duration::from_secs(2),
        client.read_stream_message(&client_budget),
    )
    .await
    .expect("committed event is projected within deadline")
    .expect("projection frame reads")
    .expect("stream remains open");
    let update: Value = serde_json::from_slice(update.body()).expect("update JSON");
    assert_eq!(update["type"], "projection");
    assert_eq!(update["payload"]["conversation_id"], CONVERSATION);
    assert_eq!(update["payload"]["origin_sequence"], "2");
    assert_eq!(
        update["payload"]["item"]["message"]["message_id"],
        "ipc-stream-message-live"
    );

    // Closing an otherwise idle duplex subscription must be observed without
    // waiting for another Workspace event; the route-owned admission permit drops.
    drop(client);
    let settlement = tokio::time::timeout(Duration::from_secs(2), idle_disconnect_receiver)
        .await
        .expect("idle peer disconnect is detected promptly")
        .expect("disconnect result is reported");
    assert_eq!(
        settlement,
        Ok(OperatorRequestSettlement::Complete),
        "clean disconnect must settle the active stream exchange successfully"
    );
    assert_eq!(stream_admission_observer.available_permits(), 2);

    let connection =
        Connection::open(&database).expect("fixture database opens for reconnect message");
    connection.execute(
        "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('ipc-stream-message-reconnect',?1,'{}','AGENT','ipc-stream-turn-reconnect','[{\"kind\":\"TEXT\",\"text\":\"missed while disconnected\"}]','2026-10-11T12:03:00.000000000Z')",
        [CONVERSATION],
    ).expect("reconnect message persists");
    for index in 0..100 {
        connection.execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,content_json,resource_refs_json,created_at) VALUES(?1,?2,'{}','USER','[]','[]',?3)",
            rusqlite::params![
                format!("ipc-stream-message-reconnect-{index:03}"),
                CONVERSATION,
                format!("2026-10-11T12:{:02}:{:02}.000000000Z", 3 + (index + 1) / 60, (index + 1) % 60),
            ],
        ).expect("bounded missed Conversation history persists");
    }
    drop(connection);
    WorkspaceService::new(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: WORKSPACE.to_owned(),
            expected_version: 2,
            policy: ReplicationPolicy::FullWorkspace,
            replication_scope_root_ids: Vec::new(),
            event: EventContext {
                event_id: "ipc-stream-policy-event-reconnect".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: "2026-10-11T12:00:02.000000000Z".to_owned(),
                correlation_id: "ipc-stream-policy-correlation-reconnect".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-11T12:00:02.000000000Z".to_owned(),
            },
        })
        .expect("reconnect Workspace event commits");

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated for a fresh subscription");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "conversation_ids": [CONVERSATION],
        "projection_types": ["conversation_presentation"],
        "resume_cursor": update["cursor"],
    }))
    .expect("subscribe body serializes");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-stream-request-2".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: WORKSPACE.to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body,
        &client_budget,
    )
    .expect("bounded second stream subscription");
    client
        .write_request(&request)
        .await
        .expect("request writes");
    let handshake = client
        .read_stream_response(&client_budget)
        .await
        .expect("second stream handshake reads");
    assert_eq!(handshake.header.status, StatusCode::OK.as_u16());
    for _ in 0..2 {
        client
            .read_stream_message(&client_budget)
            .await
            .expect("second stream frame reads")
            .expect("second stream remains open");
    }
    let replayed = client
        .read_stream_message(&client_budget)
        .await
        .expect("reconnected message frame reads")
        .expect("reconnected stream remains open");
    let replayed: Value = serde_json::from_slice(replayed.body()).expect("replayed upsert JSON");
    assert_eq!(
        replayed["payload"]["item"]["message"]["message_id"],
        "ipc-stream-message-reconnect"
    );
    let mut last_replayed_id = replayed["payload"]["item"]["message"]["message_id"]
        .as_str()
        .unwrap()
        .to_owned();
    for _ in 0..100 {
        let replayed = client
            .read_stream_message(&client_budget)
            .await
            .expect("bounded reconnect catch-up frame reads")
            .expect("reconnected stream remains open");
        let replayed: Value = serde_json::from_slice(replayed.body()).expect("catch-up frame JSON");
        assert_eq!(replayed["type"], "projection");
        last_replayed_id = replayed["payload"]["item"]["message"]["message_id"]
            .as_str()
            .expect("catch-up carries a committed message")
            .to_owned();
    }
    assert_eq!(last_replayed_id, "ipc-stream-message-reconnect-099");
    let checkpoint = client
        .read_stream_message(&client_budget)
        .await
        .expect("reconnect projection summary reads")
        .expect("reconnected stream remains open");
    let checkpoint: Value =
        serde_json::from_slice(checkpoint.body()).expect("reconnect checkpoint is JSON");
    assert_eq!(checkpoint["type"], "projection");

    // Build a valid resume cursor, then poll the lazy response stream only after
    // admission has validated the cursor. This gives the test a deterministic point
    // to remove the required retained event before catch-up begins.
    {
        let connection = Connection::open(&database).expect("database opens for gap fixture");
        connection
            .execute(
                "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('ipc-stream-message-z-retention-gap',?1,'{}','AGENT','ipc-stream-turn-gap','[{\"kind\":\"TEXT\",\"text\":\"current after retention gap\"}]','2026-10-11T12:10:00.000000000Z')",
                [CONVERSATION],
            )
            .expect("message after resume checkpoint persists");
    }
    WorkspaceService::new(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: WORKSPACE.to_owned(),
            expected_version: 3,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: Vec::new(),
            event: EventContext {
                event_id: "ipc-stream-policy-event-retention-gap".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: "2026-10-11T12:00:03.000000000Z".to_owned(),
                correlation_id: "ipc-stream-policy-correlation-retention-gap".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-11T12:00:03.000000000Z".to_owned(),
            },
        })
        .expect("event after resume checkpoint commits");
    let checkpoint_cursor = checkpoint["cursor"]
        .as_str()
        .expect("checkpoint cursor is present");
    let body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "conversation_ids": [CONVERSATION],
        "projection_types": ["conversation_presentation"],
        "resume_cursor": checkpoint_cursor,
    }))
    .expect("retained-history resume serializes");
    let response = race_router
        .oneshot(
            Request::post("/v1/stream")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-workspace-id", WORKSPACE)
                .extension(AuthenticatedLocalPeer)
                .body(Body::from(body))
                .expect("authenticated in-process stream request is valid"),
        )
        .await
        .expect("production stream route responds");
    assert_eq!(response.status(), StatusCode::OK);
    let mut stream_body = response.into_body();
    async fn next_json_frame(body: &mut Body) -> Value {
        let frame = futures_util::future::poll_fn(|context| {
            std::pin::Pin::new(&mut *body).poll_frame(context)
        })
        .await
        .expect("stream frame arrives")
        .expect("stream frame has no body error")
        .into_data()
        .expect("stream output frame is data");
        serde_json::from_slice(&frame).expect("stream frame is JSON")
    }
    assert_eq!(
        next_json_frame(&mut stream_body).await["type"],
        "stream.ready"
    );
    assert_eq!(
        next_json_frame(&mut stream_body).await["type"],
        "projection"
    );
    {
        let connection = Connection::open(&database).expect("database opens for retention loss");
        let removed = connection
            .execute(
                "DELETE FROM domain_events WHERE workspace_id = ?1 AND origin_runtime_id = ?2 AND origin_sequence = 4",
                rusqlite::params![WORKSPACE, RUNTIME],
            )
            .expect("test removes one retained replay event");
        assert_eq!(removed, 1, "exactly the required replay event is removed");
    }
    let resync = next_json_frame(&mut stream_body).await;
    assert_eq!(resync["type"], "stream.resync_required");
    assert_eq!(resync["reason"], "replay_history_gap");
    let replacement = next_json_frame(&mut stream_body).await;
    assert_eq!(replacement["type"], "projection");
    assert_eq!(replacement["payload"]["conversation_id"], CONVERSATION);
    assert_eq!(replacement["payload"]["origin_sequence"], "4");
    let replacement_cursor = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(
            replacement["cursor"]
                .as_str()
                .expect("replacement cursor is present"),
        )
        .expect("replacement cursor decodes");
    let replacement_cursor: Value =
        serde_json::from_slice(&replacement_cursor).expect("replacement cursor is JSON");
    assert_eq!(replacement_cursor["origin_sequence"], 4);
    assert_eq!(replacement_cursor["conversation_id"], CONVERSATION);
    let recovered_message = next_json_frame(&mut stream_body).await;
    assert_eq!(recovered_message["type"], "projection");
    assert_eq!(
        recovered_message["payload"]["item"]["message"]["message_id"],
        "ipc-stream-message-z-retention-gap",
        "replacement snapshot catch-up includes messages missed with the lost event"
    );
    drop(stream_body);

    drop(client);
    server.await.expect("server task completes");
}

#[test]
fn unix_stream_write_disconnect_errors_are_clean_session_cancellation() {
    use std::io::{Error, ErrorKind};

    let closed_during_socket_write = UnixTransportError::Io(Error::from(ErrorKind::BrokenPipe));
    let closed_during_frame_write = UnixTransportError::Protocol(operator_ipc::ProtocolError::Io(
        Error::from(ErrorKind::ConnectionReset),
    ));
    let timed_out = UnixTransportError::DeadlineExceeded;
    let timed_out_write = UnixTransportError::Io(Error::from(ErrorKind::TimedOut));
    let invalid_frame = UnixTransportError::Protocol(operator_ipc::ProtocolError::InvalidFrame);

    assert!(is_expected_unix_stream_write_disconnect(
        &closed_during_socket_write
    ));
    assert!(is_expected_unix_stream_write_disconnect(
        &closed_during_frame_write
    ));
    assert!(!is_expected_unix_stream_write_disconnect(&timed_out));
    assert!(!is_expected_unix_stream_write_disconnect(&timed_out_write));
    assert!(!is_expected_unix_stream_write_disconnect(&invalid_frame));
}

#[tokio::test]
async fn unix_stream_write_after_client_close_returns_peer_disconnected() {
    let directory = tempfile::tempdir().expect("temporary IPC stream directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory");
    let expected_uid = fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_accept = tokio::spawn(async move {
        listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID client connects")
    });
    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("client authenticates server");
    let mut server =
        OperatorIpcConnection::Unix(server_accept.await.expect("server accept task joins"));
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client body budget");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server body budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "closed-stream-write".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: Vec::new(),
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded stream request is valid");
    client
        .write_request(&request)
        .await
        .expect("stream request writes");
    server
        .read_request(&server_budget)
        .await
        .expect("server reads stream request");

    let handshake = operator_ipc::ResponseFrame::new(
        operator_ipc::ResponseHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "closed-stream-write".to_owned(),
            status: StatusCode::OK.as_u16(),
            headers: vec![LogicalHeader {
                name: "content-type".to_owned(),
                value: "application/vnd.litecowork.operator-stream+json".to_owned(),
            }],
            body_length: 0,
        },
        Vec::new(),
        &server_budget,
    )
    .expect("stream handshake is valid");
    server
        .write_stream_response(&handshake)
        .await
        .expect("server sends stream handshake");
    client
        .read_stream_response(&client_budget)
        .await
        .expect("client reads stream handshake");
    drop(client);

    let payload = vec![b'x'; 64 * 1024];
    let outcome = tokio::time::timeout(Duration::from_secs(2), async {
        let mut outcome = Ok(super::StreamWriteOutcome::Sent);
        for _ in 0..8 {
            outcome = server.write_stream_message(&payload, &server_budget).await;
            if outcome != Ok(super::StreamWriteOutcome::Sent) {
                break;
            }
        }
        outcome
    })
    .await
    .expect("closed peer write is bounded");
    assert_eq!(outcome, Ok(super::StreamWriteOutcome::PeerDisconnected));
}

#[tokio::test]
async fn unix_stream_write_disconnect_settles_dispatch_and_releases_body_budget() {
    use tokio::sync::oneshot;

    let directory = tempfile::tempdir().expect("temporary IPC stream dispatch directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory");
    let expected_uid = fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server body budget");
    let dispatch_budget = Arc::clone(&server_budget);
    let (frame_sender, frame_receiver) = oneshot::channel::<()>();
    let frame_receiver = Arc::new(std::sync::Mutex::new(Some(frame_receiver)));
    let route_frame_receiver = Arc::clone(&frame_receiver);
    let router = Router::new()
        .route(
            "/v1/stream",
            post(move || {
                let frame_receiver = route_frame_receiver
                    .lock()
                    .expect("stream frame receiver lock")
                    .take()
                    .expect("one stream frame receiver");
                async move {
                    let body = Body::from_stream(futures_util::stream::once(async move {
                        frame_receiver.await.expect("test releases stream frame");
                        Ok::<Bytes, std::convert::Infallible>(Bytes::from(vec![
                            b'x';
                            MAX_STREAM_FRAME_BYTES
                        ]))
                    }));
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(
                            header::CONTENT_TYPE,
                            "application/vnd.litecowork.operator-stream+json",
                        )
                        .body(body)
                        .expect("stream response is valid")
                }
            }),
        )
        .route_layer(middleware::from_fn(authenticate));
    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID stream client is accepted");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            dispatch_budget,
            Duration::from_secs(2),
        )
        .await
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("client authenticates server");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client body budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "stream-write-disconnect-dispatch".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: Vec::new(),
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded stream request is valid");
    client
        .write_request(&request)
        .await
        .expect("stream request writes");
    client
        .read_stream_response(&client_budget)
        .await
        .expect("stream handshake arrives before update");
    // Keep exactly one maximum-size frame reservation available before the body
    // yields. The dispatcher already holds its fixed stream reservation, so the
    // writer can claim the remaining slot without racing this test for capacity.
    let mut held_budget = Vec::new();
    let slots_without_frame = (DEFAULT_MAX_IN_FLIGHT_BODY_BYTES
        - super::OPERATOR_STREAM_OUTPUT_RESERVATION_BYTES)
        / MAX_STREAM_FRAME_BYTES;
    for _ in 1..slots_without_frame {
        held_budget.push(
            server_budget
                .reserve_bytes(MAX_STREAM_FRAME_BYTES)
                .expect("reserve all but one stream-frame slot"),
        );
    }
    frame_sender
        .send(())
        .expect("server body accepts one large update");
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match server_budget.reserve_bytes(MAX_STREAM_FRAME_BYTES) {
                Ok(permit) => drop(permit),
                Err(_) => break,
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("dispatcher claims the remaining frame-budget slot");
    assert_eq!(
        held_budget.len(),
        slots_without_frame - 1,
        "the remaining frame slot belongs to the active stream writer"
    );
    // No await occurs between releasing the test reservations and closing the
    // client. On the current-thread test runtime, dispatch cannot finish the
    // stalled write in that gap.
    drop(held_budget);
    drop(client);

    let settlement = tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("peer write disconnect settles within deadline")
        .expect("dispatch task joins");
    assert_eq!(settlement, Ok(OperatorRequestSettlement::Complete));
    let first_budget_reservation = server_budget
        .reserve_bytes(64 * 1024 * 1024)
        .expect("first half of in-flight capacity is returned");
    let second_budget_reservation = server_budget
        .reserve_bytes(64 * 1024 * 1024)
        .expect("all stream reservations are released after settlement");
    drop((first_budget_reservation, second_budget_reservation));
}

#[tokio::test]
async fn authenticated_unix_stream_subscribes_to_two_conversations_and_scopes_updates() {
    use domain_workspace::ChangeReplicationPolicy;
    use rusqlite::Connection;
    use storage_core::ReplicationPolicy;

    const OWNER: &str = "ipc-multi-stream-owner";
    const WORKSPACE: &str = "ipc-multi-stream-workspace";
    const FIRST: &str = "ipc-multi-stream-first";
    const SECOND: &str = "ipc-multi-stream-second";
    const THIRD: &str = "ipc-multi-stream-third";
    const RUNTIME: &str = "ipc-multi-stream-runtime";
    const NOW: &str = "2026-10-11T12:00:00.000000000Z";

    let directory = tempfile::tempdir().expect("temporary IPC data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory");
    let expected_uid = fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let database = directory.path().join("state.sqlite3");
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        IpcReplayTestKeys,
    ));
    let store = SqliteWorkspaceStore::open(
        &database,
        blobs,
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Multi Conversation IPC fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "ipc-multi-stream-workspace-event".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "ipc-multi-stream-correlation".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits");

    let connection = Connection::open(&database).expect("fixture database opens");
    for conversation_id in [FIRST, SECOND, THIRD] {
        connection
            .execute(
                "INSERT INTO conversations(conversation_id,workspace_id,created_at) VALUES(?1,?2,?3)",
                rusqlite::params![conversation_id, WORKSPACE, NOW],
            )
            .expect("Conversation fixture persists");
    }
    drop(connection);

    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let (_shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(false);
    let router = Router::new()
        .route("/v1/stream", post(subscribe_stream))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(StreamApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: "ipc-multi-stream-incarnation".to_owned(),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(1)),
            stream_shutdown: shutdown_receiver,
            task_stream_test_gate: None,
            task_stream_snapshot_test_gate: None,
        });
    let (_gap_shutdown_sender, gap_shutdown_receiver) = tokio::sync::watch::channel(false);
    let gap_router = Router::new()
        .route("/v1/stream", post(subscribe_stream))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(StreamApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: "ipc-multi-stream-gap-incarnation".to_owned(),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(1)),
            stream_shutdown: gap_shutdown_receiver,
            task_stream_test_gate: None,
            task_stream_snapshot_test_gate: None,
        });
    for invalid_subscription in [
        json!({
            "workspace_id": WORKSPACE,
            "conversation_ids": [FIRST, FIRST],
            "projection_types": ["conversation_presentation"]
        }),
        json!({
            "workspace_id": WORKSPACE,
            "conversation_ids": [FIRST],
            "task_ids": ["ipc-mixed-task-filter"],
            "projection_types": ["conversation_presentation"]
        }),
        json!({
            "workspace_id": WORKSPACE,
            "conversation_ids": [],
            "projection_types": ["conversation_presentation"]
        }),
        json!({
            "workspace_id": WORKSPACE,
            "conversation_ids": (0..21).map(|index| format!("ipc-over-limit-{index}")).collect::<Vec<_>>(),
            "projection_types": ["conversation_presentation"]
        }),
    ] {
        let invalid_response = router
            .clone()
            .oneshot(
                Request::post("/v1/stream")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("x-workspace-id", WORKSPACE)
                    .extension(AuthenticatedLocalPeer)
                    .body(Body::from(
                        serde_json::to_vec(&invalid_subscription)
                            .expect("invalid subscription serializes"),
                    ))
                    .expect("invalid request is well formed"),
            )
            .await
            .expect("router rejects invalid filter combination");
        assert!(!invalid_response.status().is_success());
    }

    let server = tokio::spawn(async move {
        for connection_index in 0..3 {
            let connection = listener
                .accept(|peer| peer.uid == expected_uid)
                .await
                .expect("same-UID peer accepted");
            let result = dispatch_ipc_exchange(
                router.clone(),
                OperatorIpcConnection::Unix(connection),
                server_budget.clone(),
                Duration::from_secs(2),
            )
            .await;
            if connection_index < 2 {
                assert_eq!(
                    result,
                    Ok(OperatorRequestSettlement::Complete),
                    "a normal client disconnect must not fail request settlement"
                );
            } else {
                result.expect("reconnected multi-Conversation stream completes cleanly");
            }
        }
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "conversation_ids": [SECOND, FIRST],
        "projection_types": ["conversation_presentation"],
        "resume_cursor": "malformed-multi-filter-cursor"
    }))
    .expect("multi-Conversation subscription serializes");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-multi-stream-request".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: WORKSPACE.to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body,
        &client_budget,
    )
    .expect("bounded multi-Conversation subscription");
    client
        .write_request(&request)
        .await
        .expect("subscription writes");
    let handshake = client
        .read_stream_response(&client_budget)
        .await
        .expect("stream handshake reads");
    assert_eq!(handshake.header.request_id, "ipc-multi-stream-request");
    assert_eq!(handshake.header.status, StatusCode::OK.as_u16());
    assert!(handshake.body().is_empty());

    let ready = client
        .read_stream_message(&client_budget)
        .await
        .expect("ready frame reads")
        .expect("stream remains open");
    let ready: Value = serde_json::from_slice(ready.body()).expect("ready JSON");
    assert_eq!(ready["type"], "stream.ready");
    assert_eq!(ready["workspace_id"], WORKSPACE);

    let resync_required = client
        .read_stream_message(&client_budget)
        .await
        .expect("malformed cursor resync frame reads")
        .expect("stream remains open for replacement snapshots");
    let resync_required: Value =
        serde_json::from_slice(resync_required.body()).expect("resync-required JSON");
    assert_eq!(resync_required["type"], "stream.resync_required");
    assert_eq!(
        resync_required["reason"],
        "resume_cursor_invalid_or_expired"
    );

    let mut snapshots = Vec::new();
    for expected_conversation in [FIRST, SECOND] {
        let frame = client
            .read_stream_message(&client_budget)
            .await
            .expect("snapshot frame reads")
            .expect("stream remains open");
        assert!(frame.body().len() <= operator_ipc::MAX_STREAM_FRAME_BYTES);
        let snapshot: Value = serde_json::from_slice(frame.body()).expect("snapshot JSON");
        assert_eq!(snapshot["type"], "projection");
        assert_eq!(snapshot["projection_type"], "conversation_presentation");
        assert_eq!(snapshot["workspace_id"], WORKSPACE);
        assert_eq!(
            snapshot["payload"]["conversation_id"],
            expected_conversation
        );
        assert_eq!(snapshot["payload"]["origin_sequence"], "1");
        snapshots.push(snapshot);
    }
    assert_ne!(
        snapshots[0]["payload"]["conversation_id"],
        snapshots[1]["payload"]["conversation_id"]
    );
    let multi_resume_cursor = snapshots[1]["cursor"]
        .as_str()
        .expect("complete multi-Conversation snapshot cursor is present")
        .to_owned();

    let connection = Connection::open(&database).expect("fixture database opens for live message");
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('ipc-multi-message',?1,'{}','AGENT','ipc-multi-turn','[{\"kind\":\"TEXT\",\"text\":\"only the second Conversation changed\"}]','2026-10-11T12:00:01.000000000Z')",
            [SECOND],
        )
        .expect("one Conversation message persists");
    drop(connection);
    WorkspaceService::new(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: WORKSPACE.to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: Vec::new(),
            event: EventContext {
                event_id: "ipc-multi-stream-trigger-event".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: "2026-10-11T12:00:01.000000000Z".to_owned(),
                correlation_id: "ipc-multi-stream-trigger-correlation".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-11T12:00:01.000000000Z".to_owned(),
            },
        })
        .expect("Workspace event commits after the single-Conversation message");

    let update = tokio::time::timeout(
        Duration::from_secs(2),
        client.read_stream_message(&client_budget),
    )
    .await
    .expect("Conversation event is projected within deadline")
    .expect("upsert frame reads")
    .expect("stream remains open");
    assert!(update.body().len() <= operator_ipc::MAX_STREAM_FRAME_BYTES);
    let update: Value = serde_json::from_slice(update.body()).expect("upsert JSON");
    assert_eq!(update["type"], "projection");
    assert_eq!(update["projection_type"], "conversation_presentation");
    assert_eq!(update["payload"]["conversation_id"], SECOND, "{update}");
    assert_eq!(
        update["payload"]["item"]["message"]["message_id"],
        "ipc-multi-message"
    );
    assert_ne!(update["payload"]["conversation_id"], FIRST);
    let resume_cursor = update["cursor"]
        .as_str()
        .expect("live upsert includes resumable multi-filter cursor")
        .to_owned();
    drop(client);

    let connection = Connection::open(&database).expect("fixture database opens for replay");
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('ipc-multi-replayed-message',?1,'{}','AGENT','ipc-multi-turn-2','[{\"kind\":\"TEXT\",\"text\":\"replayed from the second Conversation\"}]','2026-10-11T12:00:02.000000000Z')",
            [SECOND],
        )
        .expect("post-disconnect Conversation message persists");
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('ipc-multi-replayed-message-2',?1,'{}','AGENT','ipc-multi-turn-2','[{\"kind\":\"TEXT\",\"text\":\"second replay page from the second Conversation\"}]','2026-10-11T12:00:03.000000000Z')",
            [SECOND],
        )
        .expect("second post-disconnect Conversation message persists");
    drop(connection);
    WorkspaceService::new(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: WORKSPACE.to_owned(),
            expected_version: 2,
            policy: ReplicationPolicy::MetadataOnly,
            replication_scope_root_ids: Vec::new(),
            event: EventContext {
                event_id: "ipc-multi-stream-replay-event".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: "2026-10-11T12:00:02.000000000Z".to_owned(),
                correlation_id: "ipc-multi-stream-replay-correlation".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-11T12:00:02.000000000Z".to_owned(),
            },
        })
        .expect("post-disconnect Workspace event commits");

    let mut reconnect = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("reconnect authenticates daemon peer");
    let reconnect_body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "conversation_ids": [FIRST, SECOND],
        "projection_types": ["conversation_presentation"],
        "resume_cursor": resume_cursor
    }))
    .expect("multi-filter reconnect subscription serializes");
    let reconnect_request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-multi-stream-reconnect".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: WORKSPACE.to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: reconnect_body.len() as u64,
        },
        reconnect_body,
        &client_budget,
    )
    .expect("bounded multi-filter reconnect request");
    reconnect
        .write_request(&reconnect_request)
        .await
        .expect("multi-filter reconnect writes");
    let reconnect_handshake = reconnect
        .read_stream_response(&client_budget)
        .await
        .expect("multi-filter reconnect handshake reads");
    assert_eq!(reconnect_handshake.header.status, StatusCode::OK.as_u16());
    let ready = reconnect
        .read_stream_message(&client_budget)
        .await
        .expect("reconnect ready frame reads")
        .expect("reconnected stream remains open");
    let ready: Value = serde_json::from_slice(ready.body()).expect("reconnect ready JSON");
    assert_eq!(ready["type"], "stream.ready");
    for expected_conversation in [FIRST, SECOND] {
        let frame = reconnect
            .read_stream_message(&client_budget)
            .await
            .expect("reconnect snapshot frame reads")
            .expect("reconnected stream remains open");
        let snapshot: Value =
            serde_json::from_slice(frame.body()).expect("reconnect snapshot JSON");
        assert_eq!(snapshot["type"], "projection");
        assert_eq!(
            snapshot["payload"]["conversation_id"],
            expected_conversation
        );
        assert_eq!(snapshot["payload"]["origin_sequence"], "3");
    }
    let replay = tokio::time::timeout(
        Duration::from_secs(2),
        reconnect.read_stream_message(&client_budget),
    )
    .await
    .expect("replayed Conversation event arrives within deadline")
    .expect("replayed message frame reads")
    .expect("reconnected stream remains open");
    let replay: Value = serde_json::from_slice(replay.body()).expect("replayed message JSON");
    assert_eq!(replay["payload"]["conversation_id"], SECOND);
    assert_eq!(
        replay["payload"]["item"]["message"]["message_id"],
        "ipc-multi-replayed-message"
    );
    let replay_cursor = replay["cursor"]
        .as_str()
        .expect("replay frame carries a complete cursor")
        .to_owned();
    let replay_second_page = tokio::time::timeout(
        Duration::from_secs(2),
        reconnect.read_stream_message(&client_budget),
    )
    .await
    .expect("second replay page arrives within deadline")
    .expect("second replay frame reads")
    .expect("reconnected stream remains open");
    let replay_second_page: Value =
        serde_json::from_slice(replay_second_page.body()).expect("second replay page JSON");
    assert_eq!(replay_second_page["payload"]["conversation_id"], SECOND);
    assert_eq!(
        replay_second_page["payload"]["item"]["message"]["message_id"],
        "ipc-multi-replayed-message-2"
    );
    let checkpoint = reconnect
        .read_stream_message(&client_budget)
        .await
        .expect("replay checkpoint reads")
        .expect("reconnected stream remains open");
    let checkpoint: Value = serde_json::from_slice(checkpoint.body()).expect("checkpoint JSON");
    assert_eq!(checkpoint["type"], "projection");
    assert_eq!(checkpoint["payload"]["conversation_id"], SECOND);
    drop(reconnect);

    // Re-admit a valid two-Conversation cursor while event 2 exists, then remove that
    // retained row before polling the lazy body. SECOND has multiple committed fixture
    // projections so replacement-page progress must preserve the later messages.
    {
        let connection = Connection::open(&database).expect("fixture database opens for gap");
        connection
            .execute(
                "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('ipc-multi-gap-first-message',?1,'{}','AGENT','ipc-multi-gap-first-turn','[{\"kind\":\"TEXT\",\"text\":\"first Conversation after cursor\"}]','2026-10-11T12:00:04.000000000Z')",
                [FIRST],
            )
            .expect("first Conversation message after cursor persists");
    }
    WorkspaceService::new(store.clone())
        .change_replication_policy(ChangeReplicationPolicy {
            workspace_id: WORKSPACE.to_owned(),
            expected_version: 3,
            policy: ReplicationPolicy::FullWorkspace,
            replication_scope_root_ids: Vec::new(),
            event: EventContext {
                event_id: "ipc-multi-stream-retention-gap-event".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: "2026-10-11T12:00:04.000000000Z".to_owned(),
                correlation_id: "ipc-multi-stream-retention-gap-correlation".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-11T12:00:04.000000000Z".to_owned(),
            },
        })
        .expect("event after the saved multi-filter cursor commits");
    let gap_body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "conversation_ids": [FIRST, SECOND],
        "projection_types": ["conversation_presentation"],
        "resume_cursor": multi_resume_cursor,
    }))
    .expect("multi-Conversation gap subscription serializes");
    let gap_response = gap_router
        .oneshot(
            Request::post("/v1/stream")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-workspace-id", WORKSPACE)
                .extension(AuthenticatedLocalPeer)
                .body(Body::from(gap_body))
                .expect("authenticated multi-filter gap request is valid"),
        )
        .await
        .expect("production route accepts the valid multi-filter cursor");
    assert_eq!(gap_response.status(), StatusCode::OK);
    let mut gap_stream = gap_response.into_body();
    async fn next_gap_json_frame(body: &mut Body) -> Value {
        let frame = futures_util::future::poll_fn(|context| {
            std::pin::Pin::new(&mut *body).poll_frame(context)
        })
        .await
        .expect("multi-filter stream frame arrives")
        .expect("multi-filter stream frame has no body error")
        .into_data()
        .expect("multi-filter stream output frame is data");
        serde_json::from_slice(&frame).expect("multi-filter stream frame is JSON")
    }
    assert_eq!(
        next_gap_json_frame(&mut gap_stream).await["type"],
        "stream.ready"
    );
    for expected_conversation in [FIRST, SECOND] {
        let initial_snapshot = next_gap_json_frame(&mut gap_stream).await;
        assert_eq!(initial_snapshot["type"], "projection");
        assert_eq!(
            initial_snapshot["payload"]["conversation_id"],
            expected_conversation
        );
    }
    {
        let connection =
            Connection::open(&database).expect("database opens for gap retention loss");
        let removed = connection
            .execute(
                "DELETE FROM domain_events WHERE workspace_id = ?1 AND origin_runtime_id = ?2 AND origin_sequence = 2",
                rusqlite::params![WORKSPACE, RUNTIME],
            )
            .expect("test removes one retained event after admission");
        assert_eq!(
            removed, 1,
            "exactly the first event after the cursor is removed"
        );
    }
    let gap_required = next_gap_json_frame(&mut gap_stream).await;
    assert_eq!(gap_required["type"], "stream.resync_required");
    assert_eq!(gap_required["reason"], "replay_history_gap");
    let mut recovered_ids = Vec::new();
    for expected_conversation in [FIRST, SECOND] {
        let snapshot = next_gap_json_frame(&mut gap_stream).await;
        assert_eq!(snapshot["type"], "projection");
        assert_eq!(
            snapshot["payload"]["conversation_id"],
            expected_conversation
        );
        assert_eq!(snapshot["payload"]["origin_sequence"], "4");
        let snapshot_ids = snapshot["payload"]["items"]
            .as_array()
            .expect("replacement snapshot items are present")
            .iter()
            .map(|item| item["message"]["message_id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            snapshot_ids.len(),
            1,
            "resync snapshots remain one-page bounded"
        );
        recovered_ids.extend(snapshot_ids);
    }
    for _ in 0..4 {
        let frame = next_gap_json_frame(&mut gap_stream).await;
        if frame["payload"].get("item").is_none() {
            break;
        }
        recovered_ids.push(
            frame["payload"]["item"]["message"]["message_id"]
                .as_str()
                .expect("catch-up upsert carries a message")
                .to_owned(),
        );
    }
    recovered_ids.sort();
    assert_eq!(
        recovered_ids,
        vec![
            "ipc-multi-gap-first-message",
            "ipc-multi-message",
            "ipc-multi-replayed-message",
            "ipc-multi-replayed-message-2",
        ],
        "two-filter resync plus catch-up preserves every Conversation message exactly once"
    );
    drop(gap_stream);

    let mut mismatched = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("filter-mismatch recovery authenticates daemon peer");
    let mismatched_body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "conversation_ids": [FIRST, THIRD],
        "projection_types": ["conversation_presentation"],
        "resume_cursor": replay_cursor
    }))
    .expect("filter-mismatch subscription serializes");
    let mismatched_request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-multi-stream-filter-mismatch".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: WORKSPACE.to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: mismatched_body.len() as u64,
        },
        mismatched_body,
        &client_budget,
    )
    .expect("bounded filter-mismatch subscription");
    mismatched
        .write_request(&mismatched_request)
        .await
        .expect("filter-mismatch subscription writes");
    let mismatch_handshake = mismatched
        .read_stream_response(&client_budget)
        .await
        .expect("filter-mismatch handshake reads");
    assert_eq!(mismatch_handshake.header.status, StatusCode::OK.as_u16());
    let ready = mismatched
        .read_stream_message(&client_budget)
        .await
        .expect("filter-mismatch ready reads")
        .expect("mismatch recovery stream remains open");
    let ready: Value = serde_json::from_slice(ready.body()).expect("mismatch ready JSON");
    assert_eq!(ready["type"], "stream.ready");
    let required = mismatched
        .read_stream_message(&client_budget)
        .await
        .expect("filter-mismatch resync frame reads")
        .expect("mismatch recovery stream remains open");
    let required: Value = serde_json::from_slice(required.body()).expect("mismatch resync JSON");
    assert_eq!(required["type"], "stream.resync_required");
    assert_eq!(required["reason"], "resume_cursor_invalid_or_expired");
    for expected_conversation in [FIRST, THIRD] {
        let replacement = mismatched
            .read_stream_message(&client_budget)
            .await
            .expect("filter-mismatch replacement snapshot reads")
            .expect("mismatch recovery stream remains open");
        let replacement: Value =
            serde_json::from_slice(replacement.body()).expect("replacement snapshot JSON");
        assert_eq!(replacement["type"], "projection");
        assert_eq!(
            replacement["payload"]["conversation_id"],
            expected_conversation
        );
    }

    let connection = Connection::open(&database).expect("fixture database opens for revocation");
    connection
        .execute(
            "UPDATE workspaces SET owner_principal_id='ipc-multi-stream-revoked-owner' WHERE workspace_id=?1",
            [WORKSPACE],
        )
        .expect("Workspace ownership is revoked during the active stream");
    drop(connection);
    let revoked = tokio::time::timeout(
        Duration::from_secs(2),
        mismatched.read_stream_message(&client_budget),
    )
    .await
    .expect("owner revocation closes the idle stream promptly")
    .expect("revoked stream closes without sending another projection");
    assert!(
        revoked.is_none(),
        "unexpected projection after owner revocation: {}",
        revoked
            .as_ref()
            .map(|frame| String::from_utf8_lossy(frame.body()).into_owned())
            .unwrap_or_default()
    );

    drop(mismatched);
    server
        .await
        .expect("multi-Conversation stream server exits");
}

#[tokio::test]
async fn authenticated_unix_stream_rejects_request_bytes_after_subscription() {
    use tokio::io::AsyncWriteExt;

    let directory = tempfile::tempdir().expect("temporary stream data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private data directory");
    let expected_uid = fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server body budget");
    let router = Router::new()
        .route(
            "/v1/stream",
            post(|| async {
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        header::CONTENT_TYPE,
                        "application/vnd.litecowork.operator-stream+json",
                    )
                    .body(Body::from_stream(futures_util::stream::pending::<
                        Result<Bytes, std::convert::Infallible>,
                    >()))
                    .expect("stream response is valid")
            }),
        )
        .route_layer(middleware::from_fn(authenticate));

    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID stream client is accepted");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await
    });

    let mut client = tokio::net::UnixStream::connect(endpoint.socket_path())
        .await
        .expect("raw same-UID socket connects for malformed-frame probe");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client body budget");
    let body = br#"{"workspace_id":"workspace","conversation_ids":["conversation"],"projection_types":["conversation_presentation"]}"#.to_vec();
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "stream-extra-request-byte".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: "workspace".to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body,
        &client_budget,
    )
    .expect("bounded stream request is valid");
    operator_ipc::async_io::write_request(&mut client, &request)
        .await
        .expect("stream request frame writes");
    let handshake = operator_ipc::async_io::read_response(
        &mut client,
        "stream-extra-request-byte",
        &client_budget,
    )
    .await
    .expect("stream handshake reads");
    assert_eq!(handshake.header.status, StatusCode::OK.as_u16());

    client
        .write_all(b"unexpected")
        .await
        .expect("unexpected request-side byte writes");
    let settlement = server.await.expect("stream server task joins");
    assert!(
        settlement
            .expect_err("request-side bytes after subscription fail closed")
            .contains("peer closed or sent invalid data")
    );
}

#[tokio::test]
async fn authenticated_unix_task_stream_catches_up_after_operator_endpoint_rebinds() {
    use domain_task::{CreateStandaloneTask, ReviseTaskSpec, TaskService};
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::Connection;

    const OWNER: &str = "ipc-task-stream-owner";
    const WORKSPACE: &str = "ipc-task-stream-workspace";
    const TASK: &str = "ipc-task-stream-task";
    const RUNTIME: &str = "ipc-task-stream-runtime";
    const BINDING: &str = "ipc-task-stream-binding";
    const NOW: &str = "2026-10-11T12:00:00.000000000Z";

    let directory = tempfile::tempdir().expect("temporary task-stream data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory");
    let expected_uid = fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
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
    let event = |event_id: &str, recorded_at: &str| EventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: RUNTIME.to_owned(),
        hlc_timestamp: recorded_at.to_owned(),
        correlation_id: format!("correlation-{event_id}"),
        causation_id: None,
        recorded_at: recorded_at.to_owned(),
    };
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Task stream fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: event("ipc-task-stream-workspace-event", NOW),
        })
        .expect("Workspace commits");

    let connection = Connection::open(&database).expect("fixture SQLite connection opens");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES('ipc-task-stream-profile','fixture','Fixture Agent',?1)",
            [NOW],
        )
        .expect("AgentProfile persists");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version) VALUES(?1,?2,'ipc-task-stream-profile','{}','{}',1,1,?3,1)",
            rusqlite::params![BINDING, WORKSPACE, NOW],
        )
        .expect("AgentBinding persists");
    drop(connection);

    let request_payload = json!({
        "workspace_id": WORKSPACE,
        "objective": "Read the original Task objective",
        "preferred_lead_agent_binding_id": BINDING,
        "source_message_refs": [],
        "constraints": [],
        "non_goals": [],
        "input_refs": [],
        "required_outputs": [],
        "acceptance_criteria": [],
        "approvals_required": [],
        "placement_preference": "AUTO"
    });
    TaskService::new(store.clone())
        .create_standalone(CreateStandaloneTask {
            task_id: TASK.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            workspace_instruction_revision: None,
            lead_agent_binding_id: BINDING.to_owned(),
            origin_coworker_id: None,
            origin_coworker_revision: None,
            expected_coworker_version: None,
            coworker_default_lead_failover_policy: None,
            principal_id: OWNER.to_owned(),
            request_id: "ipc-task-stream-create".to_owned(),
            request_payload: request_payload.clone(),
            event: event(
                "ipc-task-stream-task-created",
                "2026-10-11T12:00:01.000000000Z",
            ),
        })
        .expect("READY Task commits through TaskService");

    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("initial endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let (_initial_shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(false);
    let router = Router::new()
        .route("/v1/stream", post(subscribe_stream))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(StreamApiState {
            store: store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: "ipc-task-stream-incarnation".to_owned(),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(1)),
            stream_shutdown: shutdown_receiver.clone(),
            task_stream_test_gate: None,
            task_stream_snapshot_test_gate: None,
        });
    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID peer is accepted");
        let _ = dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await;
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "task_ids": [TASK],
        "projection_types": ["task_presentation"]
    }))
    .expect("Task subscription serializes");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-task-stream-request".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: WORKSPACE.to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body,
        &client_budget,
    )
    .expect("bounded Task subscription");
    client
        .write_request(&request)
        .await
        .expect("Task subscription writes");
    let handshake = client
        .read_stream_response(&client_budget)
        .await
        .expect("Task stream handshake reads");
    assert_eq!(handshake.header.status, StatusCode::OK.as_u16());
    assert_eq!(handshake.body().len(), 0);

    let ready = client
        .read_stream_message(&client_budget)
        .await
        .expect("ready frame reads")
        .expect("Task stream remains open");
    let ready: Value = serde_json::from_slice(ready.body()).expect("ready JSON");
    assert_eq!(ready["type"], "stream.ready");
    assert_eq!(ready["workspace_id"], WORKSPACE);

    let snapshot = client
        .read_stream_message(&client_budget)
        .await
        .expect("Task snapshot frame reads")
        .expect("Task stream remains open");
    let snapshot: Value = serde_json::from_slice(snapshot.body()).expect("snapshot JSON");
    assert_eq!(snapshot["type"], "projection");
    assert_eq!(snapshot["projection_type"], "task_presentation");
    assert_eq!(ready["cursor"], snapshot["cursor"]);
    assert_eq!(snapshot["payload"]["task_id"], TASK);
    assert_eq!(snapshot["payload"]["origin_sequence"], "2");
    assert_eq!(
        snapshot["payload"]["items"][0]["payload"]["objective"],
        "Read the original Task objective"
    );

    TaskService::new(store.clone())
        .revise_saved_spec(ReviseTaskSpec {
            workspace_id: WORKSPACE.to_owned(),
            task_id: TASK.to_owned(),
            principal_id: OWNER.to_owned(),
            request_id: "ipc-task-stream-revise".to_owned(),
            request_payload: json!({"operation":"task.spec.revise.v1","task_id":TASK,"revision":2}),
            expected_task_version: 1,
            parent_revisions: vec![1],
            objective: "Read the revised Task objective".to_owned(),
            constraints: None,
            non_goals: None,
            input_refs: None,
            required_outputs: None,
            acceptance_criteria: None,
            approvals_required: None,
            placement_preference: None,
            preferred_lead_agent_binding_id: None,
            lead_failover_policy: None,
            event: event(
                "ipc-task-stream-task-revised",
                "2026-10-11T12:00:02.000000000Z",
            ),
        })
        .expect("Task revision commits through TaskService");

    let update = tokio::time::timeout(
        Duration::from_secs(2),
        client.read_stream_message(&client_budget),
    )
    .await
    .expect("committed Task event is projected within deadline")
    .expect("Task projection frame reads")
    .expect("Task stream remains open");
    let update: Value = serde_json::from_slice(update.body()).expect("Task update JSON");
    assert_eq!(update["type"], "projection");
    assert_eq!(update["projection_type"], "task_presentation");
    assert_eq!(update["payload"]["task_id"], TASK);
    assert_eq!(update["payload"]["origin_sequence"], "3");
    assert_eq!(
        update["payload"]["items"][0]["payload"]["objective"],
        "Read the revised Task objective"
    );
    assert_ne!(update["cursor"], snapshot["cursor"]);

    drop(client);
    server.await.expect("initial Task stream server exits");

    TaskService::new(store.clone())
        .revise_saved_spec(ReviseTaskSpec {
            workspace_id: WORKSPACE.to_owned(),
            task_id: TASK.to_owned(),
            principal_id: OWNER.to_owned(),
            request_id: "ipc-task-stream-revise-after-disconnect".to_owned(),
            request_payload: json!({"operation":"task.spec.revise.v1","task_id":TASK,"revision":3}),
            expected_task_version: 2,
            parent_revisions: vec![2],
            objective: "Read the Task after reconnect".to_owned(),
            constraints: None,
            non_goals: None,
            input_refs: None,
            required_outputs: None,
            acceptance_criteria: None,
            approvals_required: None,
            placement_preference: None,
            preferred_lead_agent_binding_id: None,
            lead_failover_policy: None,
            event: event(
                "ipc-task-stream-task-revised-after-disconnect",
                "2026-10-11T12:00:03.000000000Z",
            ),
        })
        .expect("disconnected Task revision commits through TaskService");

    // Close the original route and SQLite connection before rebinding the same local
    // endpoint. The resumed stream must use only durable Task/journal state.
    drop(store);
    let store = SqliteWorkspaceStore::open(
        &database,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            IpcReplayTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("SQLite store reopens after the prior endpoint exits");
    let race_store = store.clone();
    let (page_reached_sender, mut page_reached_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (page_release_sender, page_release_receiver) = tokio::sync::oneshot::channel();
    let race_gate = Arc::new(super::presentation_operator::TaskStreamTestGate {
        reached: page_reached_sender,
        release: tokio::sync::Mutex::new(Some(page_release_receiver)),
    });
    let (snapshot_reached_sender, mut snapshot_reached_receiver) =
        tokio::sync::mpsc::unbounded_channel();
    let (snapshot_release_sender, snapshot_release_receiver) = tokio::sync::oneshot::channel();
    let snapshot_gate = Arc::new(super::presentation_operator::TaskStreamTestGate {
        reached: snapshot_reached_sender,
        release: tokio::sync::Mutex::new(Some(snapshot_release_receiver)),
    });
    let (race_shutdown_sender, race_shutdown_receiver) = tokio::sync::watch::channel(false);
    let race_router = Router::new()
        .route("/v1/stream", post(subscribe_stream))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(StreamApiState {
            store: race_store.clone(),
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: "ipc-task-stream-race-incarnation".to_owned(),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(1)),
            stream_shutdown: race_shutdown_receiver,
            task_stream_test_gate: Some(race_gate),
            task_stream_snapshot_test_gate: Some(snapshot_gate),
        });
    let listener = endpoint.bind().expect("endpoint rebinds after restart");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let (shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(false);
    let shutdown_guard = shutdown_receiver.clone();
    let restarted_router = Router::new()
        .route("/v1/stream", post(subscribe_stream))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(StreamApiState {
            store,
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: "ipc-task-stream-incarnation-after-restart".to_owned(),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(1)),
            stream_shutdown: shutdown_receiver,
            task_stream_test_gate: None,
            task_stream_snapshot_test_gate: None,
        });
    let restarted_server = tokio::spawn(async move {
        for _ in 0..2 {
            let connection = listener
                .accept(|peer| peer.uid == expected_uid)
                .await
                .expect("same-UID peer is accepted after endpoint rebind");
            let _ = dispatch_ipc_exchange(
                restarted_router.clone(),
                OperatorIpcConnection::Unix(connection),
                server_budget.clone(),
                Duration::from_secs(2),
            )
            .await;
        }
    });

    let mut reconnect = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer authenticates after reconnect");
    let resume_cursor = update["cursor"].as_str().expect("update has cursor");
    let body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "task_ids": [TASK],
        "projection_types": ["task_presentation"],
        "resume_cursor": resume_cursor
    }))
    .expect("resume subscription serializes");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-task-stream-reconnect-request".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: WORKSPACE.to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body,
        &client_budget,
    )
    .expect("bounded reconnect subscription");
    reconnect
        .write_request(&request)
        .await
        .expect("reconnect request writes");
    let handshake = reconnect
        .read_stream_response(&client_budget)
        .await
        .expect("reconnect handshake reads");
    assert_eq!(handshake.header.status, StatusCode::OK.as_u16());
    let ready = reconnect
        .read_stream_message(&client_budget)
        .await
        .expect("reconnect ready frame reads")
        .expect("reconnected Task stream remains open");
    let ready: Value = serde_json::from_slice(ready.body()).expect("reconnect ready JSON");
    assert_eq!(ready["type"], "stream.ready");
    let caught_up = reconnect
        .read_stream_message(&client_budget)
        .await
        .expect("reconnect snapshot reads")
        .expect("reconnected Task stream remains open");
    let caught_up: Value =
        serde_json::from_slice(caught_up.body()).expect("reconnect snapshot JSON");
    assert_eq!(ready["cursor"], caught_up["cursor"]);
    assert_eq!(caught_up["type"], "projection");
    assert_eq!(caught_up["payload"]["origin_sequence"], "4");
    assert_eq!(
        caught_up["payload"]["items"][0]["payload"]["objective"],
        "Read the Task after reconnect"
    );

    TaskService::new(race_store.clone())
        .revise_saved_spec(ReviseTaskSpec {
            workspace_id: WORKSPACE.to_owned(),
            task_id: TASK.to_owned(),
            principal_id: OWNER.to_owned(),
            request_id: "ipc-task-stream-revise-after-restart-live".to_owned(),
            request_payload: json!({"operation":"task.spec.revise.v1","task_id":TASK,"revision":4}),
            expected_task_version: 3,
            parent_revisions: vec![3],
            objective: "Read the live Task update after restart".to_owned(),
            constraints: None,
            non_goals: None,
            input_refs: None,
            required_outputs: None,
            acceptance_criteria: None,
            approvals_required: None,
            placement_preference: None,
            preferred_lead_agent_binding_id: None,
            lead_failover_policy: None,
            event: event(
                "ipc-task-stream-task-revised-after-restart-live",
                "2026-10-11T12:00:04.000000000Z",
            ),
        })
        .expect("post-restart Task revision commits through TaskService");
    let live_update = tokio::time::timeout(
        Duration::from_secs(3),
        reconnect.read_stream_message(&client_budget),
    )
    .await
    .expect("post-restart live Task event is projected after catch-up")
    .expect("live Task projection frame reads")
    .expect("reconnected Task stream remains open");
    let live_update: Value =
        serde_json::from_slice(live_update.body()).expect("post-restart live Task update JSON");
    assert_eq!(live_update["type"], "projection");
    assert_eq!(live_update["payload"]["origin_sequence"], "5");
    assert_eq!(
        live_update["payload"]["items"][0]["payload"]["objective"],
        "Read the live Task update after restart"
    );
    drop(reconnect);

    let mut foreign_cursor = update["cursor"].as_str().unwrap().to_owned();
    let mut decoded_cursor = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&foreign_cursor)
        .expect("cursor decodes");
    let mut cursor_json: Value = serde_json::from_slice(&decoded_cursor).expect("cursor JSON");
    cursor_json["task_id"] = json!("ipc-task-stream-another-task");
    decoded_cursor = serde_json::to_vec(&cursor_json).expect("foreign cursor serializes");
    foreign_cursor = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(decoded_cursor);
    let mut resync_client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer authenticates for cursor resync");
    let body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "task_ids": [TASK],
        "projection_types": ["task_presentation"],
        "resume_cursor": foreign_cursor
    }))
    .expect("foreign cursor subscription serializes");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-task-stream-foreign-cursor-request".to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: WORKSPACE.to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body,
        &client_budget,
    )
    .expect("bounded cursor-resync subscription");
    resync_client
        .write_request(&request)
        .await
        .expect("cursor-resync request writes");
    let handshake = resync_client
        .read_stream_response(&client_budget)
        .await
        .expect("cursor-resync handshake reads");
    assert_eq!(handshake.header.status, StatusCode::OK.as_u16());
    let ready = resync_client
        .read_stream_message(&client_budget)
        .await
        .expect("resync ready frame reads")
        .expect("resync Task stream remains open");
    let ready: Value = serde_json::from_slice(ready.body()).expect("resync ready JSON");
    assert_eq!(ready["type"], "stream.ready");
    let required = resync_client
        .read_stream_message(&client_budget)
        .await
        .expect("resync requirement frame reads")
        .expect("resync Task stream remains open");
    let required: Value = serde_json::from_slice(required.body()).expect("resync reason JSON");
    assert_eq!(required["type"], "stream.resync_required");
    assert_eq!(required["reason"], "resume_cursor_invalid_or_expired");
    let resync = resync_client
        .read_stream_message(&client_budget)
        .await
        .expect("resync snapshot frame reads")
        .expect("resync Task stream remains open");
    let resync: Value = serde_json::from_slice(resync.body()).expect("resync snapshot JSON");
    assert_eq!(resync["type"], "projection");
    assert_eq!(resync["payload"]["origin_sequence"], "5");
    drop(resync_client);

    let race_body = serde_json::to_vec(&json!({
        "workspace_id": WORKSPACE,
        "task_ids": [TASK],
        "projection_types": ["task_presentation"]
    }))
    .expect("Task race subscription serializes");
    let race_response = race_router
        .oneshot(
            Request::post("/v1/stream")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-workspace-id", WORKSPACE)
                .extension(AuthenticatedLocalPeer)
                .body(Body::from(race_body))
                .expect("authenticated Task race request is valid"),
        )
        .await
        .expect("production Task stream route responds for the race fixture");
    assert_eq!(race_response.status(), StatusCode::OK);
    let race_stream = Arc::new(tokio::sync::Mutex::new(race_response.into_body()));
    async fn next_task_json_frame(body: &mut Body) -> Value {
        let frame = futures_util::future::poll_fn(|context| {
            std::pin::Pin::new(&mut *body).poll_frame(context)
        })
        .await
        .expect("Task stream frame arrives")
        .expect("Task stream frame has no body error")
        .into_data()
        .expect("Task stream frame is data");
        serde_json::from_slice(&frame).expect("Task stream frame is JSON")
    }
    assert_eq!(
        next_task_json_frame(&mut *race_stream.lock().await).await["type"],
        "stream.ready"
    );
    let race_snapshot = next_task_json_frame(&mut *race_stream.lock().await).await;
    assert_eq!(race_snapshot["type"], "projection");
    assert_eq!(race_snapshot["payload"]["origin_sequence"], "5");

    TaskService::new(race_store.clone())
        .revise_saved_spec(ReviseTaskSpec {
            workspace_id: WORKSPACE.to_owned(),
            task_id: TASK.to_owned(),
            principal_id: OWNER.to_owned(),
            request_id: "ipc-task-stream-retention-race-revision".to_owned(),
            request_payload: json!({"operation":"task.spec.revise.v1","task_id":TASK,"revision":5}),
            expected_task_version: 4,
            parent_revisions: vec![4],
            objective: "Task revision during event retention race".to_owned(),
            constraints: None,
            non_goals: None,
            input_refs: None,
            required_outputs: None,
            acceptance_criteria: None,
            approvals_required: None,
            placement_preference: None,
            preferred_lead_agent_binding_id: None,
            lead_failover_policy: None,
            event: event(
                "ipc-task-stream-retention-race-revised",
                "2026-10-11T12:00:05.000000000Z",
            ),
        })
        .expect("Task revision commits while the stream is active");

    let race_stream_for_poll = Arc::clone(&race_stream);
    let pending_frame =
        tokio::spawn(
            async move { next_task_json_frame(&mut *race_stream_for_poll.lock().await).await },
        );
    tokio::time::timeout(Duration::from_secs(2), page_reached_receiver.recv())
        .await
        .expect("poll reaches the replay-page/snapshot boundary")
        .expect("test gate is notified");
    let connection = rusqlite::Connection::open(&database)
        .expect("retention fixture opens a second SQLite connection");
    let removed = connection
        .execute(
            "DELETE FROM domain_events WHERE workspace_id = ?1 AND origin_runtime_id = ?2 AND origin_sequence = 6",
            rusqlite::params![WORKSPACE, RUNTIME],
        )
        .expect("remove the event after replay-page read");
    assert_eq!(
        removed, 1,
        "the event read by the stream is retained until this point"
    );
    drop(connection);
    page_release_sender
        .send(())
        .expect("release the stream after event retention changes");

    tokio::time::timeout(Duration::from_secs(2), snapshot_reached_receiver.recv())
        .await
        .expect("poll reaches the snapshot/confirmation boundary")
        .expect("snapshot test gate is notified");
    TaskService::new(race_store.clone())
        .revise_saved_spec(ReviseTaskSpec {
            workspace_id: WORKSPACE.to_owned(),
            task_id: TASK.to_owned(),
            principal_id: OWNER.to_owned(),
            request_id: "ipc-task-stream-retention-race-follow-up".to_owned(),
            request_payload: json!({"operation":"task.spec.revise.v1","task_id":TASK,"revision":6}),
            expected_task_version: 5,
            parent_revisions: vec![5],
            objective: "Newer Task revision before replay confirmation".to_owned(),
            constraints: None,
            non_goals: None,
            input_refs: None,
            required_outputs: None,
            acceptance_criteria: None,
            approvals_required: None,
            placement_preference: None,
            preferred_lead_agent_binding_id: None,
            lead_failover_policy: None,
            event: event(
                "ipc-task-stream-retention-race-follow-up",
                "2026-10-11T12:00:06.000000000Z",
            ),
        })
        .expect("new Task revision commits after snapshot and before replay confirmation");
    snapshot_release_sender
        .send(())
        .expect("release the stream after the newer event commits");

    let required = tokio::time::timeout(Duration::from_secs(2), pending_frame)
        .await
        .expect("gap marker arrives after the stream rechecks replay history")
        .expect("Task stream poll task completes");
    assert_eq!(required["type"], "stream.resync_required");
    assert_eq!(required["reason"], "replay_history_gap");
    let replacement = tokio::time::timeout(Duration::from_secs(2), async {
        next_task_json_frame(&mut *race_stream.lock().await).await
    })
    .await
    .expect("replacement snapshot arrives after the gap marker");
    assert_eq!(replacement["type"], "projection");
    assert_eq!(replacement["payload"]["origin_sequence"], "7");
    assert_eq!(
        replacement["payload"]["items"][0]["payload"]["objective"],
        "Newer Task revision before replay confirmation"
    );
    drop(race_stream);
    drop(race_shutdown_sender);
    shutdown_sender.send(true).expect("shutdown signal sends");
    drop(shutdown_guard);
    restarted_server
        .await
        .expect("restarted Task stream server exits");
}

#[tokio::test]
async fn exact_resource_revision_response_header_survives_authenticated_ipc() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = std::fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let router = Router::new()
        .route(
            "/v1/resources/resource-1/content",
            get(|| async {
                Response::builder()
                    .status(StatusCode::OK)
                    .header("x-resource-revision-id", "revision-7")
                    .header("x-private-test-header", "must-not-cross-ipc")
                    .body(Body::from("verified bytes"))
                    .expect("test response builds")
            }),
        )
        .route_layer(middleware::from_fn(authenticate));

    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID peer is accepted");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(1),
        )
        .await
        .expect("revision header response is framed and sent")
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-resource-revision-1".to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/resources/resource-1/content?revision_id=revision-7".to_owned(),
            headers: vec![LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: "workspace-1".to_owned(),
            }],
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded exact-revision request");
    client
        .write_request(&request)
        .await
        .expect("request writes");
    let response = client
        .read_response(&client_budget)
        .await
        .expect("correlated response reads");
    assert_eq!(response.header.status, StatusCode::OK.as_u16());
    assert_eq!(
        response
            .header
            .headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case("x-resource-revision-id"))
            .map(|header| header.value.as_str()),
        Some("revision-7")
    );
    assert!(
        response
            .header
            .headers
            .iter()
            .all(|header| !header.name.eq_ignore_ascii_case("x-private-test-header"))
    );
    assert_eq!(response.body(), b"verified bytes");
    server.await.expect("server exchange task completes");
}

#[tokio::test]
async fn handler_deadline_reports_possible_background_work_as_unsettled() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private directory permissions");
    let expected_uid = std::fs::metadata(directory.path())
        .expect("directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe endpoint");
    let listener = endpoint.bind().expect("endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let operation_finished = Arc::new(AtomicBool::new(false));
    let route_finished = operation_finished.clone();
    let router = Router::new()
        .route(
            "/v1/operator/slow-operation",
            get(move || {
                let route_finished = route_finished.clone();
                async move {
                    tokio::task::spawn_blocking(move || {
                        std::thread::sleep(Duration::from_millis(150));
                        route_finished.store(true, Ordering::SeqCst);
                    })
                    .await
                    .expect("blocking operation completes");
                    StatusCode::OK
                }
            }),
        )
        .route_layer(middleware::from_fn(authenticate));

    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID peer is accepted");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_millis(10),
        )
        .await
        .expect("timeout response is framed and sent")
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-slow-operation-1".to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/operator/slow-operation".to_owned(),
            headers: Vec::<LogicalHeader>::new(),
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded slow-operation request");
    client
        .write_request(&request)
        .await
        .expect("request writes");
    let response = client
        .read_response(&client_budget)
        .await
        .expect("timeout response reads");
    assert_eq!(response.header.status, StatusCode::GATEWAY_TIMEOUT.as_u16());
    assert_eq!(
        server.await.expect("server exchange task completes"),
        OperatorRequestSettlement::MayStillBeRunning
    );

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        operation_finished.load(Ordering::SeqCst),
        "the timed-out async handler can leave blocking work running"
    );
}

#[test]
fn ipc_request_frame_rejects_authorization_headers_as_an_authentication_path() {
    // Authorization is intentionally absent from the IPC logical-header allowlist.
    // `RequestFrame::new` runs the same validation used by the wire decoder.
    let budget =
        Arc::new(InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("test budget"));
    let result = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "ipc-auth-header-1".to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/operator/readiness".to_owned(),
            headers: vec![LogicalHeader {
                name: "authorization".to_owned(),
                value: "Bearer attacker-controlled".to_owned(),
            }],
            body_length: 0,
        },
        Vec::new(),
        &budget,
    );
    assert!(
        result.is_err(),
        "bearer headers must be rejected by IPC framing"
    );
}

fn workspace_instruction_ipc_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
    SqliteWorkspaceStore::open(
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
    .expect("real SQLite instruction store opens")
}

fn create_workspace_for_instruction_ipc(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    owner_id: &str,
    runtime_id: &str,
) {
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Instructions over authenticated IPC".to_owned(),
            owner_principal_id: owner_id.to_owned(),
            event: EventContext {
                event_id: format!("{workspace_id}-created"),
                origin_runtime_id: runtime_id.to_owned(),
                hlc_timestamp: "2026-10-13T12:00:00Z".to_owned(),
                correlation_id: format!("{workspace_id}-created"),
                causation_id: None,
                recorded_at: "2026-10-13T12:00:00Z".to_owned(),
            },
        })
        .expect("owner Workspace commits");
}

fn workspace_instruction_ipc_router(
    store: SqliteWorkspaceStore,
    owner_id: &str,
    runtime_id: &str,
) -> Router {
    workspace_instruction_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceInstructionApiState {
            store,
            principal_id: owner_id.to_owned(),
            runtime_id: runtime_id.to_owned(),
        })
}

struct WorkspaceInstructionIpcCall<'a> {
    frame_request_id: &'a str,
    method: &'a str,
    path_and_query: &'a str,
    headers: Vec<LogicalHeader>,
    body: Vec<u8>,
    observe_response: bool,
}

async fn send_workspace_instruction_over_unix_ipc(
    directory: &tempfile::TempDir,
    router: Router,
    call: WorkspaceInstructionIpcCall<'_>,
) -> Option<operator_ipc::ResponseFrame> {
    let WorkspaceInstructionIpcCall {
        frame_request_id,
        method,
        path_and_query,
        headers,
        body,
        observe_response,
    } = call;
    let expected_uid = fs::metadata(directory.path())
        .expect("private IPC directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe IPC endpoint");
    let listener = endpoint.bind().expect("IPC endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID client is authenticated");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await
        .expect("Workspace instruction response is framed and sent")
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: frame_request_id.to_owned(),
            method: method.to_owned(),
            path_and_query: path_and_query.to_owned(),
            headers,
            body_length: body.len() as u64,
        },
        body,
        &client_budget,
    )
    .expect("bounded Workspace instruction request");
    client
        .write_request(&request)
        .await
        .expect("Workspace instruction request frame writes");

    if observe_response {
        let response = client
            .read_response(&client_budget)
            .await
            .expect("Workspace instruction response is correlated");
        assert_eq!(
            server.await.expect("server exchange task completes"),
            OperatorRequestSettlement::Complete
        );
        Some(response)
    } else {
        assert_eq!(
            server.await.expect("server exchange task completes"),
            OperatorRequestSettlement::Complete,
            "the response write completes even though the caller does not read it"
        );
        drop(client);
        None
    }
}

fn workspace_instruction_ipc_ledger_counts(
    directory: &tempfile::TempDir,
    workspace_id: &str,
    owner_id: &str,
) -> (i64, i64, i64) {
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect instruction ledger");
    connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'workspace.instructions.revision.created.v1'),
                (SELECT COUNT(*) FROM workspace_instruction_revisions WHERE workspace_id = ?1),
                (SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?2 AND request_id LIKE 'instruction-ipc-%')",
            rusqlite::params![workspace_id, owner_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read instruction event/revision/receipt counts")
}

#[tokio::test]
async fn workspace_instruction_unix_ipc_replays_unobserved_response_and_paginates_history() {
    const WORKSPACE_ID: &str = "instruction-ipc-workspace";
    const OTHER_WORKSPACE_ID: &str = "instruction-ipc-other-workspace";
    const RESOURCE_ID: &str = "instruction-ipc-resource";
    const RESOURCE_REVISION_ID: &str = "instruction-ipc-resource-revision-1";
    const OWNER: &str = "instruction-ipc-owner";
    const RUNTIME: &str = "instruction-ipc-runtime";
    const REQUEST_ID: &str = "instruction-ipc-create-revision-1";
    const CONTENT: &[u8] = b"Keep this local instruction content out of domain events.\n";

    let directory = tempfile::tempdir().expect("temporary instruction IPC data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private instruction IPC data directory");
    let store = workspace_instruction_ipc_store(&directory);
    create_workspace_for_instruction_ipc(&store, WORKSPACE_ID, OWNER, RUNTIME);
    let blobs = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        IpcReplayTestKeys,
    ));
    let content_digest = seed_route_instruction_resource(
        &directory,
        &blobs,
        WORKSPACE_ID,
        RESOURCE_ID,
        RESOURCE_REVISION_ID,
        "text/plain",
        CONTENT,
    );
    drop(blobs);

    let create_body = serde_json::to_vec(&json!({
        "parent_revisions": [],
        "content_ref": {
            "workspace_id": WORKSPACE_ID,
            "resource_id": RESOURCE_ID,
            "revision_id": RESOURCE_REVISION_ID,
        },
        "content_digest": content_digest,
    }))
    .expect("instruction create request serializes");
    let create_headers = || {
        vec![
            LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: WORKSPACE_ID.to_owned(),
            },
            LogicalHeader {
                name: "if-match".to_owned(),
                value: "1".to_owned(),
            },
            LogicalHeader {
                name: "idempotency-key".to_owned(),
                value: REQUEST_ID.to_owned(),
            },
            LogicalHeader {
                name: "content-type".to_owned(),
                value: "application/json".to_owned(),
            },
        ]
    };

    let first = send_workspace_instruction_over_unix_ipc(
        &directory,
        workspace_instruction_ipc_router(store.clone(), OWNER, RUNTIME),
        WorkspaceInstructionIpcCall {
            frame_request_id: "instruction-ipc-frame-before-response-loss",
            method: "POST",
            path_and_query: &format!("/v1/workspaces/{WORKSPACE_ID}/instructions/revisions"),
            headers: create_headers(),
            body: create_body.clone(),
            observe_response: false,
        },
    )
    .await;
    assert!(
        first.is_none(),
        "the caller never reads the committed response"
    );
    assert_eq!(
        workspace_instruction_ipc_ledger_counts(&directory, WORKSPACE_ID, OWNER),
        (1, 1, 1),
        "the instruction event, revision, and durable receipt commit before the unread response is discarded"
    );
    let stored_response: String = rusqlite::Connection::open(
        directory.path().join("state.sqlite3"),
    )
    .expect("open committed instruction receipt")
    .query_row(
        "SELECT response_json FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        rusqlite::params![OWNER, REQUEST_ID],
        |row| row.get(0),
    )
    .expect("committed instruction receipt contains its original response");
    let stored_response: Value =
        serde_json::from_str(&stored_response).expect("stored instruction response is JSON");
    drop(store);

    let reopened = workspace_instruction_ipc_store(&directory);
    let persisted = reopened
        .get_workspace(WORKSPACE_ID)
        .expect("reopened Workspace read succeeds")
        .expect("Workspace persists after SQLite reopen");
    assert_eq!(persisted.current_instruction_revision, Some(1));
    assert_eq!(persisted.version, 2);

    let replay = send_workspace_instruction_over_unix_ipc(
        &directory,
        workspace_instruction_ipc_router(reopened.clone(), OWNER, RUNTIME),
        WorkspaceInstructionIpcCall {
            frame_request_id: "instruction-ipc-frame-after-response-loss",
            method: "POST",
            path_and_query: &format!("/v1/workspaces/{WORKSPACE_ID}/instructions/revisions"),
            headers: create_headers(),
            body: create_body,
            observe_response: true,
        },
    )
    .await
    .expect("the retry observes the original commit response");
    assert_eq!(replay.header.status, StatusCode::CREATED.as_u16());
    assert_eq!(
        replay.header.request_id, "instruction-ipc-frame-after-response-loss",
        "transport correlation uses the new frame ID while idempotency uses the persisted key"
    );
    let replay_body: Value =
        serde_json::from_slice(replay.body()).expect("replayed instruction response is JSON");
    assert_eq!(replay_body, stored_response["instruction_revision"]);
    assert_eq!(
        replay
            .header
            .headers
            .iter()
            .find(|header| header.name == "x-correlation-id")
            .map(|header| header.value.as_str()),
        stored_response["event"]["correlation_id"].as_str(),
        "replay preserves the committed event correlation header"
    );
    assert_eq!(
        workspace_instruction_ipc_ledger_counts(&directory, WORKSPACE_ID, OWNER),
        (1, 1, 1),
        "replay does not add an event, revision, or receipt"
    );

    let second_body = serde_json::to_vec(&json!({
        "parent_revisions": [1],
        "content_ref": {
            "workspace_id": WORKSPACE_ID,
            "resource_id": RESOURCE_ID,
            "revision_id": RESOURCE_REVISION_ID,
        },
        "content_digest": content_digest,
    }))
    .expect("second instruction create request serializes");
    let mut second_headers = create_headers();
    second_headers
        .iter_mut()
        .find(|header| header.name == "if-match")
        .expect("If-Match header exists")
        .value = "2".to_owned();
    second_headers
        .iter_mut()
        .find(|header| header.name == "idempotency-key")
        .expect("idempotency key exists")
        .value = "instruction-ipc-create-revision-2".to_owned();
    let second = send_workspace_instruction_over_unix_ipc(
        &directory,
        workspace_instruction_ipc_router(reopened.clone(), OWNER, RUNTIME),
        WorkspaceInstructionIpcCall {
            frame_request_id: "instruction-ipc-frame-revision-2",
            method: "POST",
            path_and_query: &format!("/v1/workspaces/{WORKSPACE_ID}/instructions/revisions"),
            headers: second_headers,
            body: second_body,
            observe_response: true,
        },
    )
    .await
    .expect("second instruction response is observed");
    assert_eq!(second.header.status, StatusCode::CREATED.as_u16());
    let second_body: Value =
        serde_json::from_slice(second.body()).expect("second instruction response is JSON");
    assert_eq!(second_body["revision"], 2);
    assert_eq!(second_body["parent_revisions"], json!([1]));
    assert_eq!(
        workspace_instruction_ipc_ledger_counts(&directory, WORKSPACE_ID, OWNER),
        (2, 2, 2)
    );

    let page_one = send_workspace_instruction_over_unix_ipc(
        &directory,
        workspace_instruction_ipc_router(reopened.clone(), OWNER, RUNTIME),
        WorkspaceInstructionIpcCall {
            frame_request_id: "instruction-ipc-frame-history-1",
            method: "GET",
            path_and_query: &format!(
                "/v1/workspaces/{WORKSPACE_ID}/instructions/revisions?limit=1"
            ),
            headers: vec![LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: WORKSPACE_ID.to_owned(),
            }],
            body: Vec::new(),
            observe_response: true,
        },
    )
    .await
    .expect("first instruction history response is observed");
    assert_eq!(page_one.header.status, StatusCode::OK.as_u16());
    let page_one: Value =
        serde_json::from_slice(page_one.body()).expect("first instruction history page is JSON");
    assert_eq!(page_one["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(page_one["items"][0]["revision"], 1);
    let cursor = page_one["next_cursor"]
        .as_str()
        .expect("first page has a continuation cursor")
        .to_owned();

    let page_two = send_workspace_instruction_over_unix_ipc(
        &directory,
        workspace_instruction_ipc_router(reopened.clone(), OWNER, RUNTIME),
        WorkspaceInstructionIpcCall {
            frame_request_id: "instruction-ipc-frame-history-2",
            method: "GET",
            path_and_query: &format!(
                "/v1/workspaces/{WORKSPACE_ID}/instructions/revisions?limit=1&cursor={cursor}"
            ),
            headers: vec![LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: WORKSPACE_ID.to_owned(),
            }],
            body: Vec::new(),
            observe_response: true,
        },
    )
    .await
    .expect("second instruction history response is observed");
    assert_eq!(page_two.header.status, StatusCode::OK.as_u16());
    let page_two: Value =
        serde_json::from_slice(page_two.body()).expect("second instruction history page is JSON");
    assert_eq!(page_two["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(page_two["items"][0]["revision"], 2);
    assert!(page_two["next_cursor"].is_null());

    create_workspace_for_instruction_ipc(&reopened, OTHER_WORKSPACE_ID, OWNER, RUNTIME);
    let foreign_cursor = send_workspace_instruction_over_unix_ipc(
        &directory,
        workspace_instruction_ipc_router(reopened.clone(), OWNER, RUNTIME),
        WorkspaceInstructionIpcCall {
            frame_request_id: "instruction-ipc-frame-foreign-cursor",
            method: "GET",
            path_and_query: &format!(
                "/v1/workspaces/{OTHER_WORKSPACE_ID}/instructions/revisions?limit=1&cursor={cursor}"
            ),
            headers: vec![LogicalHeader {
                name: "x-workspace-id".to_owned(),
                value: OTHER_WORKSPACE_ID.to_owned(),
            }],
            body: Vec::new(),
            observe_response: true,
        },
    )
    .await
    .expect("cross-Workspace cursor response is observed");
    assert_eq!(
        foreign_cursor.header.status,
        StatusCode::BAD_REQUEST.as_u16()
    );
    let foreign_cursor: Value = serde_json::from_slice(foreign_cursor.body())
        .expect("cross-Workspace cursor error is JSON");
    assert_eq!(foreign_cursor["code"], "INVALID_ARGUMENT");

    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect instruction event payload");
    let event_payload: String = connection
        .query_row(
            "SELECT payload_json FROM domain_events WHERE workspace_id = ?1 AND type = 'workspace.instructions.revision.created.v1' ORDER BY entity_revision LIMIT 1",
            [WORKSPACE_ID],
            |row| row.get(0),
        )
        .expect("instruction event exists");
    assert!(
        !event_payload.contains(std::str::from_utf8(CONTENT).expect("fixture content is UTF-8")),
        "instruction text is retained in encrypted Resource storage, not the domain event"
    );
    drop(reopened);
}

fn workspace_archive_ipc_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
    SqliteWorkspaceStore::open(
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
    .expect("real SQLite Workspace store opens")
}

fn create_workspace_for_archive_ipc(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    owner_id: &str,
    runtime_id: &str,
) {
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Archive over authenticated IPC".to_owned(),
            owner_principal_id: owner_id.to_owned(),
            event: EventContext {
                event_id: format!("{workspace_id}-created"),
                origin_runtime_id: runtime_id.to_owned(),
                hlc_timestamp: "2026-10-13T12:00:00Z".to_owned(),
                correlation_id: format!("{workspace_id}-created"),
                causation_id: None,
                recorded_at: "2026-10-13T12:00:00Z".to_owned(),
            },
        })
        .expect("owner Workspace commits");
}

fn workspace_archive_ipc_router(
    store: SqliteWorkspaceStore,
    owner_id: &str,
    runtime_id: &str,
) -> Router {
    workspace_archive_routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(WorkspaceArchiveApiState {
            store,
            principal_id: owner_id.to_owned(),
            runtime_id: runtime_id.to_owned(),
        })
}

async fn send_workspace_archive_over_unix_ipc(
    directory: &tempfile::TempDir,
    router: Router,
    workspace_id: &str,
    frame_request_id: &str,
    idempotency_key: &str,
    expected_version: &str,
) -> operator_ipc::ResponseFrame {
    let expected_uid = fs::metadata(directory.path())
        .expect("private IPC directory metadata")
        .uid();
    let endpoint = UnixEndpoint::derive(directory.path()).expect("safe IPC endpoint");
    let listener = endpoint.bind().expect("IPC endpoint binds");
    let server_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("server budget");
    let server = tokio::spawn(async move {
        let connection = listener
            .accept(|peer| peer.uid == expected_uid)
            .await
            .expect("same-UID client is authenticated");
        dispatch_ipc_exchange(
            router,
            OperatorIpcConnection::Unix(connection),
            server_budget,
            Duration::from_secs(2),
        )
        .await
        .expect("Workspace archive response is framed");
    });

    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .expect("daemon peer is authenticated");
    let client_budget =
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES).expect("client budget");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: frame_request_id.to_owned(),
            method: "POST".to_owned(),
            path_and_query: format!("/v1/workspaces/{workspace_id}/archive"),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: workspace_id.to_owned(),
                },
                LogicalHeader {
                    name: "if-match".to_owned(),
                    value: expected_version.to_owned(),
                },
                LogicalHeader {
                    name: "idempotency-key".to_owned(),
                    value: idempotency_key.to_owned(),
                },
            ],
            body_length: 0,
        },
        Vec::new(),
        &client_budget,
    )
    .expect("bounded Workspace archive request");
    client
        .write_request(&request)
        .await
        .expect("archive request frame writes");
    let response = client
        .read_response(&client_budget)
        .await
        .expect("archive response is correlated");
    server.await.expect("archive IPC server task completes");
    response
}

fn workspace_archive_ipc_ledger_counts(
    directory: &tempfile::TempDir,
    workspace_id: &str,
    owner_id: &str,
    request_id: &str,
) -> (i64, i64) {
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect archive ledger");
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'workspace.archived.v1'",
            [workspace_id],
            |row| row.get(0),
        )
        .expect("count archive events");
    let receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            rusqlite::params![owner_id, request_id],
            |row| row.get(0),
        )
        .expect("count archive receipts");
    (events, receipts)
}

#[tokio::test]
async fn workspace_archive_unix_ipc_replays_receipt_after_store_reopen() {
    const WORKSPACE_ID: &str = "archive-ipc-restart-workspace";
    const OWNER: &str = "archive-ipc-restart-owner";
    const RUNTIME: &str = "archive-ipc-restart-runtime";
    const REQUEST_ID: &str = "archive-ipc-restart-request";

    let directory = tempfile::tempdir().expect("temporary archive IPC data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private archive IPC data directory");
    let store = workspace_archive_ipc_store(&directory);
    create_workspace_for_archive_ipc(&store, WORKSPACE_ID, OWNER, RUNTIME);

    let first = send_workspace_archive_over_unix_ipc(
        &directory,
        workspace_archive_ipc_router(store.clone(), OWNER, RUNTIME),
        WORKSPACE_ID,
        "archive-ipc-frame-before-restart",
        REQUEST_ID,
        "1",
    )
    .await;
    assert_eq!(first.header.status, StatusCode::OK.as_u16());
    assert_eq!(first.header.request_id, "archive-ipc-frame-before-restart");
    let first_workspace: Value =
        serde_json::from_slice(first.body()).expect("archived Workspace response JSON");
    assert_eq!(first_workspace["status"], "ARCHIVED");
    assert_eq!(first_workspace["version"], 2);
    let first_headers = first.header.headers.clone();
    drop(store);

    let reopened = workspace_archive_ipc_store(&directory);
    let persisted = reopened
        .get_workspace(WORKSPACE_ID)
        .expect("reopened Workspace read succeeds")
        .expect("Workspace remains stored after reopening SQLite");
    assert_eq!(persisted.status, "ARCHIVED");
    assert_eq!(persisted.version, 2);
    assert_eq!(
        workspace_archive_ipc_ledger_counts(&directory, WORKSPACE_ID, OWNER, REQUEST_ID),
        (1, 1),
        "one archive event and its replay receipt survive storage reopen"
    );

    let replay = send_workspace_archive_over_unix_ipc(
        &directory,
        workspace_archive_ipc_router(reopened.clone(), OWNER, RUNTIME),
        WORKSPACE_ID,
        "archive-ipc-frame-after-restart",
        REQUEST_ID,
        "1",
    )
    .await;
    assert_eq!(replay.header.status, StatusCode::OK.as_u16());
    assert_eq!(replay.header.request_id, "archive-ipc-frame-after-restart");
    assert_eq!(replay.body(), first.body());
    assert_eq!(replay.header.headers, first_headers);
    assert_eq!(
        workspace_archive_ipc_ledger_counts(&directory, WORKSPACE_ID, OWNER, REQUEST_ID),
        (1, 1),
        "exact replay adds neither an archive event nor a request receipt"
    );
    assert_eq!(
        reopened
            .get_workspace(WORKSPACE_ID)
            .expect("final Workspace read succeeds")
            .expect("Workspace remains stored")
            .version,
        2
    );
    drop(reopened);
}

#[tokio::test]
async fn workspace_archive_unix_ipc_blocker_is_fail_closed_without_writes() {
    const WORKSPACE_ID: &str = "archive-ipc-blocked-workspace";
    const OWNER: &str = "archive-ipc-blocked-owner";
    const RUNTIME: &str = "archive-ipc-blocked-runtime";
    const REQUEST_ID: &str = "archive-ipc-blocked-request";
    const NOW: &str = "2026-10-13T12:00:00Z";

    let directory = tempfile::tempdir().expect("temporary blocked archive IPC data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private blocked archive IPC data directory");
    let store = workspace_archive_ipc_store(&directory);
    create_workspace_for_archive_ipc(&store, WORKSPACE_ID, OWNER, RUNTIME);
    let fixture = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("open channel-host fixture database");
    fixture
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TRIGGER channel_host_assignment_requires_active_binding_insert;",
        )
        .expect("disable fixture-only channel admission validation");
    fixture
        .execute(
            "INSERT INTO channel_host_assignments(channel_binding_id,workspace_id,runtime_id,host_epoch,status,assigned_at) VALUES('archive-ipc-channel-host',?1,'runtime-fixture',1,'ACTIVE',?2)",
            rusqlite::params![WORKSPACE_ID, NOW],
        )
        .expect("seed durable active channel-host blocker");
    drop(fixture);

    let response = send_workspace_archive_over_unix_ipc(
        &directory,
        workspace_archive_ipc_router(store.clone(), OWNER, RUNTIME),
        WORKSPACE_ID,
        "archive-ipc-blocked-frame",
        REQUEST_ID,
        "1",
    )
    .await;
    assert_eq!(response.header.status, StatusCode::CONFLICT.as_u16());
    let body: Value = serde_json::from_slice(response.body()).expect("blocker response JSON");
    assert_eq!(body["code"], "WORKSPACE_NOT_QUIESCENT");
    assert_eq!(
        body["details"]["blockers"][0]["kind"],
        "ACTIVE_CHANNEL_HOSTS"
    );
    assert_eq!(body["details"]["blockers"][0]["count"], 1);

    let persisted = store
        .get_workspace(WORKSPACE_ID)
        .expect("blocked Workspace read succeeds")
        .expect("blocked Workspace remains stored");
    assert_eq!(persisted.status, "ACTIVE");
    assert_eq!(persisted.version, 1);
    assert_eq!(
        workspace_archive_ipc_ledger_counts(&directory, WORKSPACE_ID, OWNER, REQUEST_ID),
        (0, 0),
        "blocked archive adds neither an event nor an idempotency receipt"
    );
    drop(store);
}
