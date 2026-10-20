//! Read-only AgentControlDescriptor routes.
//!
//! The current local probes do not yet publish a trusted AgentModule descriptor. The
//! route reports that state explicitly and never manufactures a descriptor from a
//! catalog or RuntimeOffer. This keeps the composer and session-admission boundary
//! fail-closed while providing the canonical read path for qualified modules.
#![allow(clippy::result_large_err)]

use super::{
    ApiState, ensure_workspace_owner_for, map_agent_store_error, operator_error, selected_workspace,
};
use axum::{
    Router,
    body::Body,
    extract::{FromRef, Path, State},
    http::{HeaderMap, Request, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use storage_core::AgentCatalogStore;

#[derive(Clone)]
struct AgentControlApiState {
    store: super::SqliteWorkspaceStore,
    principal_id: String,
}

impl FromRef<ApiState> for AgentControlApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
        }
    }
}

pub(super) fn routes() -> Router<ApiState> {
    routes_with_state::<ApiState>()
        .route_layer(middleware::from_fn(super::authenticate))
        .route_layer(middleware::from_fn(no_store_middleware))
}

fn routes_with_state<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    AgentControlApiState: FromRef<S>,
{
    Router::new()
        .route(
            "/v1/agent-profiles/{agent_profile_id}/control-descriptor",
            get(get_control_descriptor),
        )
        .route(
            "/v1/agent-profiles/{agent_profile_id}/refresh",
            post(refresh_control_descriptor),
        )
}

#[cfg(test)]
fn test_router(store: super::SqliteWorkspaceStore, principal_id: String) -> Router {
    routes_with_state::<AgentControlApiState>()
        .route_layer(middleware::from_fn(super::authenticate))
        .route_layer(middleware::from_fn(no_store_middleware))
        .with_state(AgentControlApiState {
            store,
            principal_id,
        })
}

async fn get_control_descriptor(
    State(state): State<AgentControlApiState>,
    headers: HeaderMap,
    Path(agent_profile_id): Path<String>,
) -> Response {
    if let Err(response) = validate_profile_id(&agent_profile_id) {
        return response;
    }
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let now = match super::operator_now() {
        Ok(value) => value,
        Err(response) => return response,
    };
    let store = state.store.clone();
    let principal = state.principal_id.clone();
    let profile = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &principal, &workspace_id)?;
        let profiles = store
            .list_agent_profiles(&principal, &workspace_id, &now)
            .map_err(|error| map_agent_store_error(error, "Agent profiles are unavailable"))?;
        Ok::<_, Response>(
            profiles
                .into_iter()
                .find(|view| view.profile.agent_profile_id == agent_profile_id),
        )
    })
    .await;
    let profile = match profile {
        Ok(Ok(Some(profile))) => profile,
        Ok(Ok(None)) => return unavailable_profile(),
        Ok(Err(response)) => return response,
        Err(_) => return unavailable(),
    };

    // A discovered profile and its offer are observations, not a module-owned
    // AgentControlDescriptor. In particular, a model catalog does not establish
    // auth readiness, native command semantics, or safe session admission.
    let _ = profile;
    unavailable_descriptor()
}

async fn refresh_control_descriptor(
    State(state): State<AgentControlApiState>,
    headers: HeaderMap,
    Path(agent_profile_id): Path<String>,
) -> Response {
    if let Err(response) = validate_profile_id(&agent_profile_id) {
        return response;
    }
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let now = match super::operator_now() {
        Ok(value) => value,
        Err(response) => return response,
    };
    let store = state.store.clone();
    let principal = state.principal_id.clone();
    let profile_exists = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &principal, &workspace_id)?;
        let profiles = store
            .list_agent_profiles(&principal, &workspace_id, &now)
            .map_err(|error| map_agent_store_error(error, "Agent profiles are unavailable"))?;
        Ok::<_, Response>(
            profiles
                .iter()
                .any(|view| view.profile.agent_profile_id == agent_profile_id),
        )
    })
    .await;
    match profile_exists {
        Ok(Ok(true)) => {
            // Refresh would invoke a trusted lifecycle adapter. None is wired for
            // these transitional probes, so the request remains read-only.
            unavailable_descriptor()
        }
        Ok(Ok(false)) => unavailable_profile(),
        Ok(Err(response)) => response,
        Err(_) => unavailable(),
    }
}

fn validate_profile_id(value: &str) -> Result<(), Response> {
    if value.trim().is_empty()
        || value.len() > 256
        || value.chars().any(char::is_control)
        || value.contains('/')
    {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Agent profile ID is invalid",
        ));
    }
    Ok(())
}

async fn no_store_middleware(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

fn unavailable_profile() -> Response {
    operator_error(
        StatusCode::NOT_FOUND,
        "NOT_FOUND",
        "Agent profile is unavailable in this Workspace",
    )
}

fn unavailable_descriptor() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "A qualified AgentModule control descriptor is not available for this agent",
    )
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Agent control descriptor is unavailable",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http::Request};
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{fs, sync::Arc, time::Duration};
    use storage_core::{AgentEndpointRecord, AgentProfileRecord, BlobPurpose, StoreError};
    use storage_sqlite::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
    use tower::ServiceExt;
    use zeroize::Zeroizing;

    const OWNER: &str = "agent-control-owner";
    const WORKSPACE: &str = "agent-control-workspace";
    const RUNTIME: &str = "agent-control-runtime";
    const NOW: &str = "2026-10-18T12:00:00Z";

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([0x31; 32]),
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

    fn fixture() -> (tempfile::TempDir, super::super::SqliteWorkspaceStore) {
        let directory = tempfile::tempdir().expect("temporary descriptor directory");
        #[cfg(unix)]
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("restrict test directory");
        let store = super::super::SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("temporary SQLite store opens");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Agent descriptor tests".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "agent-control-workspace-created".to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "agent-control-workspace-created".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("Workspace commits");
        (directory, store)
    }

    async fn request(router: &Router, path: &str) -> Response {
        let mut request = Request::get(path)
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("request is valid");
        request
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("route responds")
    }

    fn seed_profile(store: &super::super::SqliteWorkspaceStore) {
        store
            .put_agent_profile(
                AgentProfileRecord {
                    agent_profile_id: "profile-control".to_owned(),
                    provider_key: "CODEX".to_owned(),
                    display_name: "Codex".to_owned(),
                    discovered_at: NOW.to_owned(),
                },
                vec![AgentEndpointRecord {
                    endpoint_id: "endpoint-control".to_owned(),
                    agent_profile_id: "profile-control".to_owned(),
                    protocol: "CLI".to_owned(),
                    topology: "PROCESS_ADAPTER".to_owned(),
                    protocol_version: None,
                    capabilities: serde_json::json!({"input.text":true}),
                }],
            )
            .expect("profile observation persists");
    }

    #[tokio::test]
    async fn profile_catalog_presence_never_manufactures_a_control_descriptor() {
        let (_directory, store) = fixture();
        seed_profile(&store);
        let router = test_router(store, OWNER.to_owned());
        let response = request(
            &router,
            "/v1/agent-profiles/profile-control/control-descriptor",
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = to_bytes(response.into_body(), 8192)
            .await
            .expect("bounded error body");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("JSON error");
        assert_eq!(value["code"], "DEPENDENCY_UNAVAILABLE");
        assert!(
            !body.windows(8).any(|window| window == b"endpoint"),
            "private endpoint details must not be returned"
        );
    }

    #[tokio::test]
    async fn refresh_without_a_module_is_read_only_and_no_store() {
        let (_directory, store) = fixture();
        seed_profile(&store);
        let router = test_router(store.clone(), OWNER.to_owned());
        let mut request = Request::post("/v1/agent-profiles/profile-control/refresh")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("request is valid");
        request
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
        let response = router.oneshot(request).await.expect("route responds");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            store
                .list_agent_profiles(OWNER, WORKSPACE, NOW)
                .expect("existing profile remains readable")
                .len(),
            1,
            "failed refresh must not replace the observed profile"
        );
    }
}
