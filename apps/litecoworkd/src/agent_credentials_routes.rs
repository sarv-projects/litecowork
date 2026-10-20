//! Authenticated Operator boundary for descriptor-scoped credential submission.
//!
//! This endpoint intentionally fails closed until a production AgentModule
//! descriptor source is registered. Catalog observations and client-supplied
//! descriptor digests cannot authorize writes to the local OS SecretStore.

#![allow(clippy::result_large_err)]

use crate::agent_credentials_operator::{
    AgentCredentialDescriptorSnapshot, AgentCredentialSecretSubmission, CredentialSubmissionError,
    validate_agent_credential_request,
};
use crate::agents::control_descriptor::AgentControlDescriptor;
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, FromRef, Path, State},
    http::{HeaderMap, Request, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::post,
};
use std::{sync::Arc, time::SystemTime};
use storage_core::{AgentCatalogStore, SecretStore};

const MAX_PROFILE_ID_BYTES: usize = 512;
const MAX_SLOT_ID_BYTES: usize = 128;

/// A descriptor is accepted only from this Runtime-owned module boundary. The
/// production source returns unavailable until a trusted AgentModule registry is
/// wired; request data is never used to construct the descriptor.
pub(crate) trait CurrentAgentDescriptorSource: Send + Sync {
    fn current_descriptor(
        &self,
        workspace_id: &str,
        agent_profile_id: &str,
        now: SystemTime,
    ) -> Result<Option<AgentControlDescriptor>, ()>;
}

#[derive(Clone)]
struct CredentialSubmissionApiState {
    store: crate::operator::SqliteWorkspaceStore,
    principal_id: String,
    secret_store: Arc<dyn SecretStore>,
    descriptor_source: Arc<dyn CurrentAgentDescriptorSource>,
}

impl FromRef<crate::operator::ApiState> for CredentialSubmissionApiState {
    fn from_ref(state: &crate::operator::ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
            secret_store: Arc::new(storage_sqlite::OsSecretStore),
            descriptor_source: Arc::new(NoAgentDescriptorSource),
        }
    }
}

struct NoAgentDescriptorSource;

impl CurrentAgentDescriptorSource for NoAgentDescriptorSource {
    fn current_descriptor(
        &self,
        _workspace_id: &str,
        _agent_profile_id: &str,
        _now: SystemTime,
    ) -> Result<Option<AgentControlDescriptor>, ()> {
        Ok(None)
    }
}

pub(crate) fn routes() -> Router<crate::operator::ApiState> {
    routes_with_state::<crate::operator::ApiState>()
        .route_layer(middleware::from_fn(crate::operator::authenticate))
        .route_layer(middleware::from_fn(no_store_middleware))
}

fn routes_with_state<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    CredentialSubmissionApiState: FromRef<S>,
{
    Router::new()
        .route(
            "/v1/agent-profiles/{agent_profile_id}/credentials/{credential_slot_id}",
            post(submit_credential),
        )
        .layer(DefaultBodyLimit::max(70 * 1024))
}

async fn submit_credential(
    State(state): State<CredentialSubmissionApiState>,
    headers: HeaderMap,
    Path((agent_profile_id, credential_slot_id)): Path<(String, String)>,
    Json(request): Json<AgentCredentialSecretSubmission>,
) -> Response {
    if !valid_path_id(&agent_profile_id, MAX_PROFILE_ID_BYTES)
        || !valid_path_id(&credential_slot_id, MAX_SLOT_ID_BYTES)
    {
        return crate::operator::operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Agent credential target is invalid",
        );
    }
    if let Err(response) = crate::operator::idempotency_key(&headers) {
        return response;
    }
    let workspace_id = match crate::operator::selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let now = match crate::operator::operator_now() {
        Ok(value) => value,
        Err(response) => return response,
    };

    let store = state.store.clone();
    let principal_id = state.principal_id.clone();
    let profile_id = agent_profile_id.clone();
    let workspace_for_lookup = workspace_id.clone();
    let profile_exists = tokio::task::spawn_blocking(move || {
        crate::operator::ensure_workspace_owner_for(&store, &principal_id, &workspace_for_lookup)?;
        let profiles = store
            .list_agent_profiles(&principal_id, &workspace_for_lookup, &now)
            .map_err(|error| {
                crate::operator::map_agent_store_error(error, "Agent profiles are unavailable")
            })?;
        Ok::<_, Response>(
            profiles
                .iter()
                .any(|view| view.profile.agent_profile_id == profile_id),
        )
    })
    .await;
    match profile_exists {
        Ok(Ok(true)) => {}
        Ok(Ok(false)) => {
            return crate::operator::operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Agent profile is unavailable in this Workspace",
            );
        }
        Ok(Err(response)) => return response,
        Err(_) => {
            return crate::operator::operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "Agent credential submission is unavailable",
            );
        }
    }

    // Validate the write-only payload without consulting a descriptor or
    // touching SecretStore. This preserves deterministic invalid-request errors
    // even while the qualified module producer is unavailable.
    match validate_agent_credential_request(
        &workspace_id,
        &agent_profile_id,
        &credential_slot_id,
        &request,
    ) {
        Ok(()) => {}
        Err(CredentialSubmissionError::InvalidRequest) => {
            return crate::operator::operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Agent credential submission is invalid",
            );
        }
        Err(_) => {
            return crate::operator::operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Agent credential request validation failed",
            );
        }
    }

    let now = SystemTime::now();
    let descriptor_source = state.descriptor_source.clone();
    let descriptor_workspace = workspace_id.clone();
    let descriptor_profile = agent_profile_id.clone();
    let descriptor = tokio::task::spawn_blocking(move || {
        descriptor_source.current_descriptor(&descriptor_workspace, &descriptor_profile, now)
    })
    .await;
    let descriptor = match descriptor {
        Ok(Ok(Some(descriptor))) => descriptor,
        Ok(Ok(None)) | Ok(Err(())) | Err(_) => {
            return crate::operator::operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "A fresh qualified AgentModule credential descriptor is unavailable",
            );
        }
    };
    let snapshot = match AgentCredentialDescriptorSnapshot::from_control_descriptor(
        &agent_profile_id,
        &descriptor,
        now,
    ) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return crate::operator::operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "A fresh qualified AgentModule credential descriptor is unavailable",
            );
        }
    };

    // Secret writes are an external mutation. Until the route has durable
    // idempotency/replay receipts, fail closed instead of risking a second
    // keyring entry after a lost response and client retry.
    let _secret_store_without_write = &state.secret_store;
    let _ = snapshot;
    crate::operator::operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Credential writes are unavailable until durable replay protection is ready",
    )
}

fn valid_path_id(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= maximum
        && !value.chars().any(char::is_control)
        && !value.contains('/')
        && !value.contains('\\')
}

async fn no_store_middleware(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http::Request};
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{fs, sync::Arc, time::Duration};
    use storage_core::{
        AgentCatalogStore, AgentEndpointRecord, AgentProfileRecord, BlobPurpose, SecretKind,
        SecretPlacement, SecretRef, SecretStore, StoreError,
    };
    use storage_sqlite::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
    use tower::ServiceExt;
    use zeroize::Zeroizing;

    const OWNER: &str = "agent-credential-owner";
    const WORKSPACE: &str = "agent-credential-workspace";
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
                bytes: Zeroizing::new([0x42; 32]),
            })
        }
        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob("unknown key version".to_owned()));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    #[derive(Default)]
    struct RecordingSecretStore(std::sync::Mutex<Vec<Vec<u8>>>);

    impl SecretStore for RecordingSecretStore {
        fn write(
            &self,
            _workspace_id: &str,
            _kind: SecretKind,
            secret: &mut [u8],
        ) -> Result<SecretRef, StoreError> {
            self.0
                .lock()
                .expect("secret store mutex")
                .push(secret.to_vec());
            secret.fill(0);
            Ok(SecretRef {
                secret_ref_id: "secret_test".to_owned(),
                provider_ref: "test-keyring:secret_test".to_owned(),
                placement: SecretPlacement::LocalOnly,
            })
        }
    }

    struct NoDescriptor;

    impl CurrentAgentDescriptorSource for NoDescriptor {
        fn current_descriptor(
            &self,
            _workspace_id: &str,
            _agent_profile_id: &str,
            _now: SystemTime,
        ) -> Result<Option<AgentControlDescriptor>, ()> {
            Ok(None)
        }
    }

    fn fixture() -> (tempfile::TempDir, crate::operator::SqliteWorkspaceStore) {
        let directory = tempfile::tempdir().expect("temporary credential directory");
        #[cfg(unix)]
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("restrict directory");
        let store = crate::operator::SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
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
                name: "Credential route tests".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "credential-workspace-created".to_owned(),
                    origin_runtime_id: "credential-runtime".to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "credential-workspace-created".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("Workspace commits");
        store
            .put_agent_profile(
                AgentProfileRecord {
                    agent_profile_id: "profile-credential".to_owned(),
                    provider_key: "CODEX".to_owned(),
                    display_name: "Codex".to_owned(),
                    discovered_at: NOW.to_owned(),
                },
                vec![AgentEndpointRecord {
                    endpoint_id: "endpoint-credential".to_owned(),
                    agent_profile_id: "profile-credential".to_owned(),
                    protocol: "CLI".to_owned(),
                    topology: "PROCESS_ADAPTER".to_owned(),
                    protocol_version: None,
                    capabilities: serde_json::json!({"input.text":true}),
                }],
            )
            .expect("profile persists");
        (directory, store)
    }

    fn test_router(
        store: crate::operator::SqliteWorkspaceStore,
        secret_store: Arc<dyn SecretStore>,
    ) -> Router {
        routes_with_state::<CredentialSubmissionApiState>()
            .route_layer(middleware::from_fn(crate::operator::authenticate))
            .route_layer(middleware::from_fn(no_store_middleware))
            .with_state(CredentialSubmissionApiState {
                store,
                principal_id: OWNER.to_owned(),
                secret_store,
                descriptor_source: Arc::new(NoDescriptor),
            })
    }

    fn body() -> String {
        r#"{"descriptor_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","secret_base64":"c2VjcmV0LW11c3Qtbm90LWxlYWs="}"#.to_owned()
    }

    async fn submit(router: &Router, authenticated: bool) -> Response {
        submit_body(router, authenticated, body()).await
    }

    async fn submit_body(router: &Router, authenticated: bool, body: String) -> Response {
        let mut request =
            Request::post("/v1/agent-profiles/profile-credential/credentials/api-key")
                .header("x-workspace-id", WORKSPACE)
                .header("idempotency-key", "credential-submit-test-1")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("valid request");
        if authenticated {
            request
                .extensions_mut()
                .insert(crate::operator::AuthenticatedLocalPeer);
        }
        router
            .clone()
            .oneshot(request)
            .await
            .expect("route responds")
    }

    #[tokio::test]
    async fn repeated_unqualified_submission_fails_closed_without_secret_writes() {
        let (_directory, store) = fixture();
        let secret_store = Arc::new(RecordingSecretStore::default());
        let router = test_router(store, secret_store.clone());
        for _ in 0..2 {
            let response = submit(&router, true).await;
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            let body = to_bytes(response.into_body(), 8192)
                .await
                .expect("bounded error");
            let value: serde_json::Value = serde_json::from_slice(&body).expect("error JSON");
            assert_eq!(value["code"], "DEPENDENCY_UNAVAILABLE");
        }
        assert!(
            secret_store
                .0
                .lock()
                .expect("secret store mutex")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn credential_submission_requires_idempotency_before_dependency_lookup() {
        let (_directory, store) = fixture();
        let secret_store = Arc::new(RecordingSecretStore::default());
        let router = test_router(store, secret_store.clone());
        let mut request =
            Request::post("/v1/agent-profiles/profile-credential/credentials/api-key")
                .header("x-workspace-id", WORKSPACE)
                .header("content-type", "application/json")
                .body(Body::from(body()))
                .expect("valid request");
        request
            .extensions_mut()
            .insert(crate::operator::AuthenticatedLocalPeer);

        let response = router.oneshot(request).await.expect("route responds");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(
            secret_store
                .0
                .lock()
                .expect("secret store mutex")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn unauthenticated_credential_submission_is_denied_without_echoing_secret() {
        let (_directory, store) = fixture();
        let secret_store = Arc::new(RecordingSecretStore::default());
        let router = test_router(store, secret_store.clone());
        let response = submit(&router, false).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = to_bytes(response.into_body(), 8192)
            .await
            .expect("bounded error");
        assert!(!String::from_utf8_lossy(&body).contains("secret-must-not-leak"));
        assert!(
            secret_store
                .0
                .lock()
                .expect("secret store mutex")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn malformed_secret_is_rejected_before_descriptor_lookup_or_secret_store_write() {
        let (_directory, store) = fixture();
        let secret_store = Arc::new(RecordingSecretStore::default());
        let router = test_router(store, secret_store.clone());
        let malformed = body().replace("c2VjcmV0LW11c3Qtbm90LWxlYWs=", "not-base64");
        let response = submit_body(&router, true, malformed).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(
            secret_store
                .0
                .lock()
                .expect("secret store mutex")
                .is_empty()
        );
    }
}
