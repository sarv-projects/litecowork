//! Authenticated Workspace catalog and versioned commands for worker profiles.
// Route handlers intentionally return full Axum Responses to preserve status, body, and headers.
#![allow(clippy::result_large_err)]

use super::*;
use domain_responsibility::{
    CommittedDelegationProfile, DelegationProfileCommand, DelegationProfileCommandScope,
    DelegationProfileError, DelegationProfileRevisionInput, DelegationProfileService,
    DelegationProfileStatus,
};
use domain_task::{
    WorkerCostObservation, WorkerObservationConfidence, WorkerPerformanceAttempt,
    WorkerPerformanceAttemptState, WorkerPerformanceQuery, WorkerUsageConfidence,
    WorkerUsageSource, aggregate_worker_performance,
};
use serde_json::Value;
use storage_sqlite::{DelegationProfileEventContext, SqliteDelegationProfileStore};

const MAX_BODY_BYTES: usize = 96 * 1024;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/delegation-profiles",
            get(list_delegation_profiles).post(create_delegation_profile),
        )
        .route(
            "/v1/delegation-profiles/{delegation_profile_id}",
            get(get_delegation_profile),
        )
        .route(
            "/v1/delegation-profiles/{delegation_profile_id}/revisions",
            get(list_delegation_profile_revisions).post(revise_delegation_profile),
        )
        .route(
            "/v1/delegation-profiles/{delegation_profile_id}/performance",
            get(get_delegation_profile_performance),
        )
        .route(
            "/v1/delegation-profiles/{delegation_profile_id}/duplicate",
            post(duplicate_delegation_profile),
        )
        .route(
            "/v1/delegation-profiles/{delegation_profile_id}/status",
            post(change_delegation_profile_status),
        )
        .route_layer(middleware::from_fn(profile_no_store_middleware))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryParams {
    agent_binding_id: Option<String>,
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u32,
    workspace_id: String,
    agent_binding_id: Option<String>,
    status: Option<String>,
    updated_at: String,
    delegation_profile_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    workspace_id: String,
    agent_binding_id: String,
    revision: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DuplicateBody {
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusBody {
    status: DelegationProfileStatus,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevisionQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceQuery {
    task_category: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RevisionCursor {
    version: u32,
    workspace_id: String,
    delegation_profile_id: String,
    before_revision: u64,
}

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Delegation profile query is invalid",
    )
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Delegation profile catalog is unavailable",
    )
}

fn profile_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

async fn profile_no_store_middleware(request: Request<Body>, next: Next) -> Response {
    profile_no_store(next.run(request).await)
}

#[cfg(test)]
mod response_header_tests {
    use super::*;
    use axum::http::HeaderValue;
    use axum::{body::Body, http::Request};
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::Connection;
    use std::{fs, sync::Arc, time::Duration};
    use storage_sqlite::{
        FileBlobStore, RuntimeOsPrincipalIdentity, SqliteConfig, WorkspaceBlobKey,
        WorkspaceBlobKeyProvider,
    };
    use tower::ServiceExt;
    use zeroize::Zeroizing;

    const OWNER: &str = "profile-route-owner";
    const WORKSPACE: &str = "profile-route-workspace";
    const RUNTIME: &str = "profile-route-runtime";
    const INCARNATION: &str = "profile-route-incarnation";
    const NOW: &str = "2026-10-18T10:00:00Z";

    #[derive(Clone)]
    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: storage_core::BlobPurpose,
        ) -> Result<WorkspaceBlobKey, storage_core::StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([31_u8; 32]),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: storage_core::BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, storage_core::StoreError> {
            if version != 1 {
                return Err(storage_core::StoreError::Blob(
                    "unknown test key version".to_owned(),
                ));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    fn test_state() -> (
        ApiState,
        tempfile::TempDir,
        tokio::sync::watch::Sender<bool>,
    ) {
        let directory = tempfile::tempdir().expect("private test directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
                .expect("restrict fixture directory");
        }
        let database = directory.path().join("state.sqlite3");
        let blobs = Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys));
        let store = SqliteWorkspaceStore::open(
            &database,
            blobs,
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("SQLite store opens");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Profile route fixture".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "profile-route-workspace-created".to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "profile-route-workspace-correlation".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("Workspace commits");

        let connection = Connection::open(&database).expect("fixture SQLite connection");
        connection
            .execute_batch(&format!(
                "BEGIN;
                 INSERT INTO runtimes(runtime_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,resource_capacity_json,last_seen,version)
                 VALUES ('{RUNTIME}','{{}}','test','LINUX','X86_64','[\"EXECUTOR\",\"OPERATOR_ENDPOINT\"]','PERSONAL_DEVICE','ONLINE','MANUAL','{INCARNATION}','{{}}','{NOW}',1);
                 INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state,ready_at,stopped_at,version)
                 VALUES ('{INCARNATION}','{RUNTIME}','{NOW}','test',0,'READY','{NOW}',NULL,1);
                 INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,revoked_at,version)
                 VALUES ('profile-route-binding','{RUNTIME}','{WORKSPACE}','LOCAL_ENROLLMENT','ACTIVE','[\"EXECUTOR\",\"OPERATOR_ENDPOINT\"]','{NOW}','{NOW}',NULL,1);
                 COMMIT;"
            ))
            .expect("seed current local Runtime and Workspace binding");
        drop(connection);

        let runtime_identity =
            RuntimeOsPrincipalIdentity::test_support(RUNTIME).expect("test-only OS identity");
        let (shutdown_sender, stream_shutdown) = tokio::sync::watch::channel(false);
        (
            ApiState {
                store,
                principal_id: OWNER.to_owned(),
                runtime_id: RUNTIME.to_owned(),
                local_incarnation_id: INCARNATION.to_owned(),
                lifecycle_blockers: Vec::new(),
                runtime_identity: Arc::new(runtime_identity),
                stream_admission: Arc::new(tokio::sync::Semaphore::new(1)),
                runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
                stream_shutdown,
            },
            directory,
            shutdown_sender,
        )
    }

    fn fork_state(state: &ApiState, principal_id: &str) -> ApiState {
        ApiState {
            store: state.store.clone(),
            principal_id: principal_id.to_owned(),
            runtime_id: state.runtime_id.clone(),
            local_incarnation_id: state.local_incarnation_id.clone(),
            lifecycle_blockers: state.lifecycle_blockers.clone(),
            runtime_identity: state.runtime_identity.clone(),
            stream_admission: state.stream_admission.clone(),
            runtime_inventory_revision: state.runtime_inventory_revision.clone(),
            stream_shutdown: state.stream_shutdown.clone(),
        }
    }

    #[test]
    fn profile_list_route_is_not_cacheable() {
        let (state, _directory, _shutdown) = test_state();
        let mut headers = HeaderMap::new();
        headers.insert("x-workspace-id", HeaderValue::from_static(WORKSPACE));
        let response = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(list_delegation_profiles(
                State(state),
                headers,
                Ok(Query(QueryParams {
                    agent_binding_id: None,
                    status: None,
                    cursor: None,
                    limit: None,
                })),
            ))
            .expect("owner can read profile list");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn registered_get_errors_are_not_cacheable() {
        let (state, _directory, _shutdown) = test_state();
        let app = routes().with_state(state);
        let request = Request::builder()
            .method("GET")
            .uri("/v1/delegation-profiles/invalid%0Aprofile")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("request builds");
        let response = app.oneshot(request).await.expect("route responds");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[test]
    fn fractional_usage_quantity_remains_unknown_without_an_exact_scale() {
        assert!(
            project_performance_cost("PROVIDER_REPORTED", "0.125", "USD", "USD", "EXACT",)
                .is_none()
        );
        assert_eq!(
            project_performance_cost("PROVIDER_REPORTED", "125", "USD", "USD", "EXACT",)
                .expect("integer quantity remains representable")
                .amount,
            125
        );
    }

    #[test]
    fn performance_route_is_owner_scoped_and_keeps_unattributed_verification_unknown() {
        let (state, directory, _shutdown) = test_state();
        let database = directory.path().join("state.sqlite3");
        Connection::open(&database)
            .expect("fixture SQLite connection")
            .execute_batch(&format!(
                "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES ('performance-agent-profile','fixture','Performance Fixture','{NOW}');
                 INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version) VALUES ('performance-binding','{WORKSPACE}','performance-agent-profile','{{}}','{{}}',1,0,'{NOW}',1);"
            ))
            .expect("seed an enabled profile binding");

        let mut headers = HeaderMap::new();
        headers.insert("x-workspace-id", HeaderValue::from_static(WORKSPACE));
        headers.insert(
            "idempotency-key",
            HeaderValue::from_static("performance-profile-create"),
        );
        let revision = serde_json::json!({
            "name":"Performance worker",
            "routing_description":"Bounded performance fixture.",
            "session_options":{},
            "session_options_descriptor_digest":null,
            "required_features":[],
            "preferred_features":[],
            "enforced_policy":{
                "capability_allowlist":[],
                "maximum_effect_risk":"SAFE",
                "filesystem_write_scope":"WORKTREE_ONLY",
                "external_effects":"DENY",
                "secret_access":"NONE"
            },
            "optimization_preference":"BALANCED",
            "max_concurrency":1,
            "max_host_delegation_depth":0,
            "latency_class":"STANDARD",
            "environment_policy":{
                "placement_preference":"AUTO",
                "isolation":"REQUIRED",
                "sharing_scope":"ATTEMPT_PRIVATE"
            },
            "native_delegation_policy":"INHERIT",
            "warm_policy":{
                "host":"COLD","native_session":"CLOSE_ON_SETTLE",
                "capability_hosts":"COLD","browser_environment":"COLD",
                "local_model":"PROVIDER_DEFAULT","ttl_ms":null,
                "max_memory_bytes":null,"max_idle_cost":null,"triggers":[]
            }
        });
        let create = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        let response = create
            .block_on(create_delegation_profile(
                State(fork_state(&state, OWNER)),
                headers.clone(),
                Bytes::from(
                    serde_json::json!({
                        "workspace_id":WORKSPACE,
                        "agent_binding_id":"performance-binding",
                        "revision":revision
                    })
                    .to_string(),
                ),
            ))
            .expect("profile is created disabled");
        assert_eq!(response.status(), StatusCode::CREATED);

        let profile_id = idempotent_id(
            "dp",
            OWNER,
            "performance-profile-create",
            &format!("delegation-profile.create.{WORKSPACE}"),
        );
        let revision_request = Request::builder()
            .method("GET")
            .uri(format!(
                "/v1/delegation-profiles/{profile_id}/revisions?limit=10"
            ))
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("revision-history request builds");
        let revision_response = create
            .block_on(
                routes()
                    .with_state(fork_state(&state, OWNER))
                    .oneshot(revision_request),
            )
            .expect("revision-history route responds");
        assert_eq!(revision_response.status(), StatusCode::OK);
        assert_eq!(
            revision_response.headers()[header::CACHE_CONTROL],
            "no-store"
        );
        let revision_body = create
            .block_on(axum::body::to_bytes(
                revision_response.into_body(),
                32 * 1024,
            ))
            .expect("bounded revision-history response");
        let revision_page: Value =
            serde_json::from_slice(&revision_body).expect("revision-history JSON");
        assert_eq!(revision_page["items"].as_array().unwrap().len(), 1);
        assert_eq!(revision_page["items"][0]["revision"], 1);

        let response = create
            .block_on(get_delegation_profile_performance(
                State(fork_state(&state, OWNER)),
                headers.clone(),
                Path(profile_id.clone()),
                Ok(Query(PerformanceQuery {
                    task_category: None,
                })),
            ))
            .expect("owner receives a performance projection");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = create
            .block_on(axum::body::to_bytes(response.into_body(), 32 * 1024))
            .expect("bounded response body");
        let value: Value = serde_json::from_slice(&body).expect("JSON response");
        assert_eq!(value["delegation_profile_id"], profile_id);
        assert_eq!(value["task_category"], "OTHER");
        assert_eq!(value["sample_count"], 0);
        assert_eq!(value["verifier_pass_rate"], Value::Null);
        assert_eq!(value["median_latency_ms"], Value::Null);
        assert_eq!(value["median_observed_cost"], Value::Null);
        assert_eq!(value["retry_rate"], Value::Null);
        assert_eq!(value["human_intervention_rate"], Value::Null);
        assert_eq!(value["failure_categories"], serde_json::json!([]));
        assert_eq!(value["confidence"], "LOW");

        let denied = create
            .block_on(get_delegation_profile_performance(
                State(fork_state(&state, "another-owner")),
                headers,
                Path(profile_id),
                Ok(Query(PerformanceQuery {
                    task_category: None,
                })),
            ))
            .expect_err("another Workspace principal cannot query profile metrics");
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    }
}

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}

fn store_error(error: storage_core::StoreError) -> Response {
    match error {
        storage_core::StoreError::NotFound => operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ),
        storage_core::StoreError::Invalid(_) => invalid(),
        _ => unavailable(),
    }
}

fn domain_error(error: DelegationProfileError) -> Response {
    match error {
        DelegationProfileError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Delegation profile is unavailable",
        ),
        DelegationProfileError::Unauthorized => operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ),
        DelegationProfileError::AlreadyExists
        | DelegationProfileError::VersionConflict
        | DelegationProfileError::IdempotencyConflict
        | DelegationProfileError::Archived => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Delegation profile version, name, or idempotency key conflicts with current state",
        ),
        DelegationProfileError::WorkspaceArchived => operator_error(
            StatusCode::CONFLICT,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces are read-only",
        ),
        DelegationProfileError::OptionsInvalid => operator_error(
            StatusCode::BAD_REQUEST,
            "DELEGATION_PROFILE_OPTIONS_INVALID",
            "Delegation profile options are invalid",
        ),
        DelegationProfileError::SessionOptionsUnsupported => operator_error(
            StatusCode::CONFLICT,
            "AGENT_SESSION_OVERRIDE_UNSUPPORTED",
            "This Runtime cannot validate the requested agent session options",
        ),
        DelegationProfileError::EnablementUnavailable => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "Enabling requires validated adapter, Trust, and Environment admission integration",
        ),
        DelegationProfileError::InvalidDefinition => invalid(),
        DelegationProfileError::Storage | DelegationProfileError::VersionOverflow => unavailable(),
    }
}

fn decode_cursor(
    encoded: &str,
    workspace: &str,
    binding: Option<&str>,
    status: Option<&str>,
) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: Cursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace
        || cursor.agent_binding_id.as_deref() != binding
        || cursor.status.as_deref() != status
        || !valid_id(&cursor.delegation_profile_id)
        || cursor.updated_at.len() > 64
        || time::OffsetDateTime::parse(
            &cursor.updated_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return Err(invalid());
    }
    Ok((cursor.updated_at, cursor.delegation_profile_id))
}

fn decode_revision_cursor(
    encoded: &str,
    workspace: &str,
    profile_id: &str,
) -> Result<u64, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: RevisionCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace
        || cursor.delegation_profile_id != profile_id
        || cursor.before_revision == 0
    {
        return Err(invalid());
    }
    Ok(cursor.before_revision)
}

async fn list_delegation_profiles(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<QueryParams>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    if query
        .agent_binding_id
        .as_deref()
        .is_some_and(|id| !valid_id(id))
        || query
            .status
            .as_deref()
            .is_some_and(|status| !matches!(status, "ENABLED" | "DISABLED" | "ARCHIVED"))
    {
        return Err(invalid());
    }
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(invalid());
    }
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| {
            decode_cursor(
                cursor,
                &workspace.workspace_id,
                query.agent_binding_id.as_deref(),
                query.status.as_deref(),
            )
        })
        .transpose()?;

    let adapter = SqliteDelegationProfileStore::new(state.store.clone());
    let owner = state.principal_id.clone();
    let workspace_id = workspace.workspace_id.clone();
    let binding_filter = query.agent_binding_id.clone();
    let status_filter = query.status.clone();
    let page = tokio::task::spawn_blocking(move || {
        adapter.list(
            &owner,
            &workspace_id,
            binding_filter.as_deref(),
            status_filter.as_deref(),
            after,
            limit,
        )
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(store_error)?;
    let next_cursor = page
        .next
        .map(|(updated_at, delegation_profile_id)| {
            serde_json::to_vec(&Cursor {
                version: 1,
                workspace_id: workspace.workspace_id,
                agent_binding_id: query.agent_binding_id,
                status: query.status,
                updated_at,
                delegation_profile_id,
            })
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        })
        .transpose()
        .map_err(|_| unavailable())?;
    Ok(profile_no_store(
        Json(json!({"items": page.items, "next_cursor": next_cursor})).into_response(),
    ))
}

async fn get_delegation_profile(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let adapter = SqliteDelegationProfileStore::new(state.store.clone());
    let owner = state.principal_id.clone();
    let workspace_id = workspace.workspace_id.clone();
    let profile_id = id;
    let profile =
        tokio::task::spawn_blocking(move || adapter.get(&owner, &workspace_id, &profile_id))
            .await
            .map_err(|_| unavailable())?
            .map_err(store_error)?
            .ok_or_else(|| {
                operator_error(
                    StatusCode::NOT_FOUND,
                    "NOT_FOUND",
                    "Delegation profile is unavailable",
                )
            })?;
    Ok(profile_no_store(Json(profile).into_response()))
}

async fn list_delegation_profile_revisions(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    query: Result<Query<RevisionQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(invalid());
    }
    let before_revision = query
        .cursor
        .as_deref()
        .map(|cursor| decode_revision_cursor(cursor, &workspace.workspace_id, &id))
        .transpose()?;
    let adapter = SqliteDelegationProfileStore::new(state.store.clone());
    let owner = state.principal_id.clone();
    let workspace_id = workspace.workspace_id.clone();
    let profile_id = id.clone();
    let page = tokio::task::spawn_blocking(move || {
        adapter.list_revisions(&owner, &workspace_id, &profile_id, before_revision, limit)
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(store_error)?;
    let next_cursor = page
        .next
        .map(|before_revision| {
            serde_json::to_vec(&RevisionCursor {
                version: 1,
                workspace_id: workspace.workspace_id,
                delegation_profile_id: id,
                before_revision,
            })
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        })
        .transpose()
        .map_err(|_| unavailable())?;
    let mut response = Json(json!({
        "items": page.items,
        "next_cursor": next_cursor,
    }))
    .into_response();
    response = profile_no_store(response);
    Ok(response)
}

async fn get_delegation_profile_performance(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    query: Result<Query<PerformanceQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let Query(query) = query.map_err(|_| invalid())?;
    let category = query.task_category.as_deref().unwrap_or("OTHER");
    if !matches!(
        category,
        "SOFTWARE_ENGINEERING"
            | "RESEARCH"
            | "WRITING"
            | "DATA_ANALYSIS"
            | "OFFICE"
            | "BROWSER"
            | "PERSONAL_ADMIN"
            | "OTHER"
    ) {
        return Err(invalid());
    }
    let store = SqliteDelegationProfileStore::new(state.store.clone());
    let owner_principal_id = state.principal_id.clone();
    let workspace_id = workspace.workspace_id.clone();
    let profile_id = id.clone();
    let task_category = category.to_owned();
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| unavailable())?;
    let query_now = now.clone();
    let rows = tokio::task::spawn_blocking(move || {
        store.worker_performance_attempts(
            &owner_principal_id,
            &workspace_id,
            &profile_id,
            &task_category,
            &query_now,
        )
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(domain_error)?;
    let samples = rows
        .into_iter()
        .map(|row| {
            let state = match row.status.as_str() {
                "COMPLETED" => WorkerPerformanceAttemptState::Completed,
                "FAILED" => WorkerPerformanceAttemptState::Failed,
                "CANCELLED" => WorkerPerformanceAttemptState::Cancelled,
                "ABANDONED" => WorkerPerformanceAttemptState::Abandoned,
                "CREATED" | "PREPARING" | "RUNNING" | "WAITING_APPROVAL" | "WAITING_RESOURCE"
                | "CHECKPOINTING" | "CANCEL_REQUESTED" => WorkerPerformanceAttemptState::Running,
                _ => return Err(unavailable()),
            };
            let cost = row.cost.and_then(|cost| {
                project_performance_cost(
                    &cost.source,
                    &cost.quantity,
                    &cost.unit,
                    &cost.currency,
                    &cost.confidence,
                )
            });
            Ok(WorkerPerformanceAttempt {
                attempt_id: row.attempt_id,
                task_id: row.task_id,
                step_id: row.step_id,
                profile_id: id.clone(),
                task_category: row.task_category,
                state,
                admitted_at: row.admitted_at,
                terminal_at: row.terminal_at,
                terminal_verification: None,
                required_observations_complete: false,
                superseded_by_later_attempt: row.superseded_by_later_attempt,
                blocked_by_owner_request: row.blocked_by_owner_request,
                cost,
            })
        })
        .collect::<Result<Vec<_>, Response>>()?;
    // VerificationRun has no exact Attempt attribution in the served schema. Keeping
    // this source family incomplete forces LOW confidence and null verifier metrics.
    let projection = aggregate_worker_performance(
        &WorkerPerformanceQuery {
            profile_id: id.clone(),
            task_category: Some(category.to_owned()),
            now,
            source_observations_complete: false,
        },
        &samples,
    )
    .map_err(|_| unavailable())?;
    let confidence = match projection.confidence {
        WorkerObservationConfidence::Low => "LOW",
        WorkerObservationConfidence::Medium => "MEDIUM",
        WorkerObservationConfidence::High => "HIGH",
    };
    let cost = projection.median_observed_cost.map(|cost| {
        json!({
            "quantity": cost.amount,
            "unit": cost.unit,
            "currency": cost.currency,
            "confidence": match cost.confidence {
                domain_task::WorkerUsageConfidence::Exact => "EXACT",
                domain_task::WorkerUsageConfidence::Estimated => "ESTIMATED",
            },
        })
    });
    let response = Json(json!({
        "delegation_profile_id": projection.profile_id,
        "task_category": projection.task_category,
        "sample_count": projection.sample_count,
        "verifier_pass_rate": projection.verifier_pass_rate_basis_points.map(|value| f64::from(value) / 10_000.0),
        "median_latency_ms": projection.median_latency_ms,
        "median_observed_cost": cost,
        "retry_rate": projection.retry_rate_basis_points.map(|value| f64::from(value) / 10_000.0),
        "human_intervention_rate": projection.human_intervention_rate_basis_points.map(|value| f64::from(value) / 10_000.0),
        "failure_categories": [],
        "confidence": confidence,
    }))
    .into_response();
    Ok(profile_no_store(response))
}

fn project_performance_cost(
    source: &str,
    quantity: &str,
    unit: &str,
    currency: &str,
    confidence: &str,
) -> Option<WorkerCostObservation> {
    // The current aggregator stores integer quantities. Keep fractional UsageQuantity
    // values unknown until a scale-free exact-decimal median is part of its contract.
    let amount = quantity.parse::<u64>().ok()?;
    if currency.len() != 3
        || !currency.bytes().all(|byte| byte.is_ascii_uppercase())
        || unit.is_empty()
    {
        return None;
    }
    let source = match source {
        "AGENT_REPORTED" => WorkerUsageSource::AgentReported,
        "PROVIDER_REPORTED" => WorkerUsageSource::ProviderReported,
        "HOST_MEASURED" => WorkerUsageSource::HostMeasured,
        _ => return None,
    };
    let confidence = match confidence {
        "EXACT" => WorkerUsageConfidence::Exact,
        "ESTIMATED" => WorkerUsageConfidence::Estimated,
        _ => return None,
    };
    Some(WorkerCostObservation {
        amount,
        unit: unit.to_owned(),
        currency: currency.to_owned(),
        source,
        confidence,
    })
}

fn parse_body<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Response> {
    if bytes.len() > MAX_BODY_BYTES {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Delegation profile request exceeds the local body limit",
        ));
    }
    serde_json::from_slice(bytes).map_err(|_| invalid())
}

fn request_key(headers: &HeaderMap) -> Result<String, Response> {
    super::idempotency_key(headers)
}

fn expected_version(headers: &HeaderMap) -> Result<u64, Response> {
    let value = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(invalid)?
        .trim();
    let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        &value[1..value.len() - 1]
    } else {
        value
    };
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(invalid)
}

fn revision_input(value: Value) -> Result<DelegationProfileRevisionInput, Response> {
    let object = value.as_object().ok_or_else(invalid)?;
    const REQUIRED: &[&str] = &[
        "name",
        "routing_description",
        "session_options",
        "session_options_descriptor_digest",
        "required_features",
        "preferred_features",
        "enforced_policy",
        "optimization_preference",
        "max_concurrency",
        "max_host_delegation_depth",
        "latency_class",
        "environment_policy",
        "native_delegation_policy",
        "warm_policy",
    ];
    if REQUIRED.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid());
    }
    serde_json::from_value(value).map_err(|_| invalid())
}

fn profile_view(committed: &CommittedDelegationProfile) -> Result<Value, Response> {
    let mut revision = serde_json::to_value(&committed.revision).map_err(|_| unavailable())?;
    if let Some(object) = revision.as_object_mut() {
        object.remove("name_key");
    }
    Ok(json!({
        "delegation_profile_id": committed.profile.delegation_profile_id,
        "workspace_id": committed.profile.workspace_id,
        "agent_binding_id": committed.profile.agent_binding_id,
        "name": committed.profile.name,
        "current_revision": committed.profile.current_revision,
        "revision": revision,
        "status": committed.profile.status,
        "created_at": committed.profile.created_at,
        "updated_at": committed.profile.updated_at,
        "version": committed.profile.version,
    }))
}

async fn execute_command(
    state: &ApiState,
    workspace: &Workspace,
    request_id: String,
    command: DelegationProfileCommand,
    response_status: StatusCode,
) -> Result<Response, Response> {
    let correlation_id = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        "delegation-profile-correlation",
    );
    let response_correlation = correlation_id.clone();
    let event_id = idempotent_id(
        "ev",
        &state.principal_id,
        &request_id,
        "delegation-profile-event",
    );
    let event = event_context(&state.runtime_id, &correlation_id).map_err(|_| unavailable())?;
    let context = DelegationProfileEventContext {
        event_id,
        origin_runtime_id: event.origin_runtime_id,
        hlc_timestamp: event.hlc_timestamp,
        correlation_id: event.correlation_id,
        causation_id: event.causation_id,
        recorded_at: event.recorded_at,
    };
    let adapter = SqliteDelegationProfileStore::new_with_context(state.store.clone(), context)
        .map_err(domain_error)?;
    let store = state.store.clone();
    let principal_id = state.principal_id.clone();
    let workspace_id = workspace.workspace_id.clone();
    let runtime_id = state.runtime_id.clone();
    let runtime_incarnation_id = state.local_incarnation_id.clone();
    let scope = DelegationProfileCommandScope {
        principal_id: principal_id.clone(),
        workspace_id: workspace_id.clone(),
        request_id,
    };
    let committed = tokio::task::spawn_blocking(move || {
        let committed = DelegationProfileService::new(adapter)
            .execute(&scope, command)
            .map_err(domain_error)?;
        // The command receipt includes the exact revision. Reauthorize after commit
        // before returning a Workspace-scoped projection.
        let current_workspace = ensure_workspace_owner_for(&store, &principal_id, &workspace_id)?;
        let current_binding = store
            .get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
                owner_principal_id: principal_id,
                workspace_id,
                runtime_id,
                runtime_incarnation_id,
            })
            .map_err(|_| unavailable())?;
        if current_workspace.status != "ACTIVE"
            || !current_binding.is_some_and(|binding| binding.status == "ACTIVE")
        {
            return Err(operator_error(
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Local Runtime Workspace access is unavailable",
            ));
        }
        Ok::<_, Response>(committed)
    })
    .await
    .map_err(|_| unavailable())??;
    let mut response = (response_status, Json(profile_view(&committed)?)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(value) = header::HeaderValue::from_str(&response_correlation) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn create_delegation_profile(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: CreateBody = parse_body(&body)?;
    if body.workspace_id != workspace.workspace_id || !valid_id(&body.agent_binding_id) {
        return Err(invalid());
    }
    let request_id = request_key(&headers)?;
    let profile_id = idempotent_id(
        "dp",
        &state.principal_id,
        &request_id,
        &format!("delegation-profile.create.{}", workspace.workspace_id),
    );
    execute_command(
        &state,
        &workspace,
        request_id,
        DelegationProfileCommand::Create {
            profile_id,
            agent_binding_id: body.agent_binding_id,
            revision: revision_input(body.revision)?,
        },
        StatusCode::CREATED,
    )
    .await
}

async fn revise_delegation_profile(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let request_id = request_key(&headers)?;
    let expected = expected_version(&headers)?;
    let body: Value = parse_body(&body)?;
    execute_command(
        &state,
        &workspace,
        request_id,
        DelegationProfileCommand::Revise {
            profile_id: id,
            expected_version: expected,
            revision: revision_input(body)?,
        },
        StatusCode::CREATED,
    )
    .await
}

async fn duplicate_delegation_profile(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let request_id = request_key(&headers)?;
    let expected = expected_version(&headers)?;
    let body: DuplicateBody = parse_body(&body)?;
    let profile_id = idempotent_id(
        "dp",
        &state.principal_id,
        &request_id,
        &format!("delegation-profile.duplicate.{}", workspace.workspace_id),
    );
    execute_command(
        &state,
        &workspace,
        request_id,
        DelegationProfileCommand::Duplicate {
            source_profile_id: id,
            expected_version: expected,
            profile_id,
            name: body.name,
        },
        StatusCode::CREATED,
    )
    .await
}

async fn change_delegation_profile_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let request_id = request_key(&headers)?;
    let expected = expected_version(&headers)?;
    let body: StatusBody = parse_body(&body)?;
    execute_command(
        &state,
        &workspace,
        request_id,
        DelegationProfileCommand::SetStatus {
            profile_id: id,
            expected_version: expected,
            status: body.status,
        },
        StatusCode::OK,
    )
    .await
}

#[cfg(test)]
mod revision_cursor_tests {
    use super::*;

    fn cursor(workspace_id: &str, profile_id: &str, before_revision: u64) -> String {
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&RevisionCursor {
                version: 1,
                workspace_id: workspace_id.to_owned(),
                delegation_profile_id: profile_id.to_owned(),
                before_revision,
            })
            .unwrap(),
        )
    }

    #[test]
    fn revision_cursor_is_bound_to_workspace_and_profile() {
        let encoded = cursor("workspace-a", "profile-a", 2);
        assert_eq!(
            decode_revision_cursor(&encoded, "workspace-a", "profile-a").unwrap(),
            2
        );
        assert!(decode_revision_cursor(&encoded, "workspace-b", "profile-a").is_err());
        assert!(decode_revision_cursor(&encoded, "workspace-a", "profile-b").is_err());
        assert!(
            decode_revision_cursor(
                &cursor("workspace-a", "profile-a", 0),
                "workspace-a",
                "profile-a"
            )
            .is_err()
        );
    }
}
