//! Workspace-scoped reads for provider quota observations.
//!
//! No AgentModule currently publishes an authoritative quota observation. The
//! OpenAPI contract defines JSON `null` for that state; do not infer a quota from
//! generic rate limits, local estimates, or profile metadata.

use super::{
    ApiState, ensure_workspace_owner_for, map_agent_store_error, operator_error, selected_workspace,
};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::Value;
use storage_core::AgentCatalogStore;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/agent-bindings/{agent_binding_id}/quota-observation",
            get(get_agent_quota_observation),
        )
        .route_layer(middleware::from_fn(super::authenticate))
        .route_layer(middleware::from_fn(no_store_middleware))
}

async fn no_store_middleware(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

async fn get_agent_quota_observation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(agent_binding_id): Path<String>,
) -> Result<Response, Response> {
    if agent_binding_id.trim().is_empty()
        || agent_binding_id.len() > 256
        || agent_binding_id.chars().any(char::is_control)
    {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Agent binding ID is invalid",
        ));
    }

    let workspace_id = selected_workspace(&headers)?;
    let store = state.store.clone();
    let principal_id = state.principal_id.clone();
    tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &principal_id, &workspace_id)?;
        store
            .get_agent_binding(&principal_id, &workspace_id, &agent_binding_id)
            .map_err(|error| {
                map_agent_store_error(error, "Workspace agent binding is unavailable")
            })?
            .ok_or_else(|| {
                operator_error(
                    StatusCode::NOT_FOUND,
                    "NOT_FOUND",
                    "Workspace agent binding is unavailable",
                )
            })
    })
    .await
    .map_err(|_| {
        operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "Workspace agent binding is unavailable",
        )
    })??;

    // The latest observation remains null until a selected AgentModule provides a
    // typed source, observation timestamp, freshness window, and optional reset hint.
    // This read does not probe providers or create a budget/failover signal.
    Ok(Json(Option::<Value>::None).into_response())
}

#[cfg(test)]
mod tests {
    use super::super::SqliteWorkspaceStore;
    use super::*;
    use axum::{body::to_bytes, http::HeaderValue};
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::Connection;
    use std::{fs, sync::Arc, time::Duration};
    use storage_sqlite::{
        FileBlobStore, RuntimeOsPrincipalIdentity, SqliteConfig, WorkspaceBlobKey,
        WorkspaceBlobKeyProvider,
    };
    use tower::ServiceExt;
    use zeroize::Zeroizing;

    const WORKSPACE: &str = "quota-observation-workspace";
    const OWNER: &str = "quota-observation-owner";
    const RUNTIME: &str = "quota-observation-runtime";
    const INCARNATION: &str = "quota-observation-incarnation";
    const NOW: &str = "2026-10-18T12:00:00Z";

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: storage_core::BlobPurpose,
        ) -> Result<WorkspaceBlobKey, storage_core::StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([47_u8; 32]),
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
                    "unknown quota test key version".to_owned(),
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
        let directory = tempfile::tempdir().expect("private quota fixture directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
                .expect("restrict fixture directory");
        }
        let database = directory.path().join("state.sqlite3");
        let store = SqliteWorkspaceStore::open(
            &database,
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("SQLite store opens");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Quota observation fixture".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "quota-workspace-created".to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "quota-workspace-correlation".to_owned(),
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
                 VALUES ('quota-runtime-workspace-binding','{RUNTIME}','{WORKSPACE}','LOCAL_ENROLLMENT','ACTIVE','[\"EXECUTOR\",\"OPERATOR_ENDPOINT\"]','{NOW}','{NOW}',NULL,1);
                 INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at)
                 VALUES ('quota-agent-profile','fixture','Quota fixture','{NOW}');
                 INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version)
                 VALUES ('quota-agent-binding','{WORKSPACE}','quota-agent-profile','{{\"mode\":\"AUTO_COMPATIBLE\",\"required_features\":[],\"preferred_topologies\":[]}}','{{}}',1,0,'{NOW}',1);
                 COMMIT;"
            ))
            .expect("seed local Runtime, binding, and AgentBinding");
        drop(connection);

        let runtime_identity =
            RuntimeOsPrincipalIdentity::test_support(RUNTIME).expect("test OS identity");
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

    #[tokio::test]
    async fn registered_route_returns_uncached_null_for_owned_binding() {
        let (state, _directory, _shutdown) = test_state();
        let app = routes().with_state(state);
        let mut request = axum::http::Request::builder()
            .method("GET")
            .uri("/v1/agent-bindings/quota-agent-binding/quota-observation")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("request builds");
        request
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
        let response = app.oneshot(request).await.expect("route responds");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = to_bytes(response.into_body(), 1024)
            .await
            .expect("bounded quota response");
        assert_eq!(body.as_ref(), b"null");
    }

    #[tokio::test]
    async fn registered_route_denies_foreign_workspace_owner_without_cache() {
        let (state, _directory, _shutdown) = test_state();
        let app = routes().with_state(fork_state(&state, "another-principal"));
        let mut request = axum::http::Request::builder()
            .method("GET")
            .uri("/v1/agent-bindings/quota-agent-binding/quota-observation")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("request builds");
        request
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
        let denied = app.oneshot(request).await.expect("route responds");
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        assert_eq!(denied.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[test]
    fn quota_observation_route_hides_bindings_outside_selected_workspace() {
        let (state, _directory, _shutdown) = test_state();
        let mut headers = HeaderMap::new();
        headers.insert("x-workspace-id", HeaderValue::from_static(WORKSPACE));
        let missing = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(get_agent_quota_observation(
                State(state),
                headers,
                Path("foreign-agent-binding".to_owned()),
            ))
            .expect_err("foreign or missing binding is not disclosed");
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }
}
