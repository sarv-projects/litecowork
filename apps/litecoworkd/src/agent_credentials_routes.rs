//! Authenticated Operator boundary for descriptor-scoped credential submission.
//!
//! This endpoint reads only a fresh stored AgentModule descriptor for a new
//! operation. Runtime-local receipts reconcile SQLite and the OS SecretStore;
//! catalog observations and client-supplied descriptor digests cannot authorize
//! writes to the local SecretStore.

#![allow(clippy::result_large_err)]

use crate::agent_credentials_operator::{
    AgentCredentialBindingResponse, AgentCredentialDescriptorSnapshot,
    AgentCredentialSecretSubmission, CredentialSubmissionError, decode_agent_credential_request,
};
use crate::agents::control_descriptor::AgentControlDescriptor;
use crate::agents::descriptor_service::AgentDescriptorService;
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, FromRef, Path, State},
    http::{HeaderMap, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use sha2::Digest;
use std::{sync::Arc, time::SystemTime};
use storage_core::{
    AgentCatalogStore, AgentCredentialSubmissionKey, AgentCredentialSubmissionReceipt,
    AgentCredentialSubmissionReservation, AgentCredentialSubmissionState,
    AgentCredentialSubmissionStore, SecretPlacement, SecretRef, SecretStore,
};

const MAX_PROFILE_ID_BYTES: usize = 512;
const MAX_SLOT_ID_BYTES: usize = 128;

/// A descriptor is accepted only from this Runtime-owned module boundary. The
/// production source reads the exact fresh descriptor stored for the current
/// Runtime incarnation; request data is never used to construct it.
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
    runtime_identity: Arc<storage_sqlite::RuntimeOsPrincipalIdentity>,
    secret_store: Arc<dyn SecretStore>,
    descriptor_source: Arc<dyn CurrentAgentDescriptorSource>,
}

impl FromRef<crate::operator::ApiState> for CredentialSubmissionApiState {
    fn from_ref(state: &crate::operator::ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
            runtime_identity: state.runtime_identity.clone(),
            secret_store: Arc::new(storage_sqlite::OsSecretStore),
            descriptor_source: Arc::new(StoredAgentDescriptorSource::new(
                state.store.clone(),
                state.principal_id.clone(),
                state.runtime_id.clone(),
                state.local_incarnation_id.clone(),
                AgentDescriptorService::shared(),
            )),
        }
    }
}

/// Reads only the latest fresh descriptor persisted for the trusted module and this
/// Runtime incarnation. Request data can select a profile but cannot author a descriptor.
pub(crate) struct StoredAgentDescriptorSource {
    store: crate::operator::SqliteWorkspaceStore,
    principal_id: String,
    runtime_id: String,
    local_incarnation_id: String,
    service: Arc<AgentDescriptorService>,
}

impl StoredAgentDescriptorSource {
    pub(crate) fn new(
        store: crate::operator::SqliteWorkspaceStore,
        principal_id: String,
        runtime_id: String,
        local_incarnation_id: String,
        service: Arc<AgentDescriptorService>,
    ) -> Self {
        Self {
            store,
            principal_id,
            runtime_id,
            local_incarnation_id,
            service,
        }
    }
}

impl CurrentAgentDescriptorSource for StoredAgentDescriptorSource {
    fn current_descriptor(
        &self,
        workspace_id: &str,
        agent_profile_id: &str,
        now: SystemTime,
    ) -> Result<Option<AgentControlDescriptor>, ()> {
        let now: time::OffsetDateTime = now.into();
        let now = now
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|_| ())?;
        crate::operator::ensure_workspace_owner_for(&self.store, &self.principal_id, workspace_id)
            .map_err(|_| ())?;
        let profile = self
            .store
            .list_agent_profiles(&self.principal_id, workspace_id, &now)
            .map_err(|_| ())?
            .into_iter()
            .find(|view| view.profile.agent_profile_id == agent_profile_id)
            .ok_or(())?;
        let module = self
            .service
            .module_for_provider(&profile.profile.provider_key)
            .cloned()
            .ok_or(())?;
        self.service
            .current(
                &self.store,
                &module,
                &self.runtime_id,
                &self.local_incarnation_id,
                &now,
            )
            .map_err(|_| ())
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
    let idempotency_key = match crate::operator::idempotency_key(&headers) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let workspace_id = match crate::operator::selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let recorded_at = match crate::operator::operator_now() {
        Ok(value) => value,
        Err(response) => return response,
    };

    // Decode once into zeroizing memory before any mutable profile or descriptor
    // read. The same bytes are fingerprinted and later passed directly to the
    // idempotent SecretStore operation.
    let mut secret_bytes = match decode_agent_credential_request(
        &workspace_id,
        &agent_profile_id,
        &credential_slot_id,
        &request,
    ) {
        Ok(secret_bytes) => secret_bytes,
        Err(CredentialSubmissionError::InvalidRequest) => {
            return crate::operator::operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Agent credential submission is invalid",
            );
        }
        Err(_) => return credential_unavailable(),
    };

    let idempotency_key_digest = format!(
        "sha256:{}",
        hex::encode(sha2::Sha256::digest(idempotency_key.as_bytes()))
    );
    let request_fingerprint = state.runtime_identity.keyed_pseudonym_parts(
        "agent-credential-request-v1",
        &[
            request.descriptor_digest.as_bytes(),
            secret_bytes.as_slice(),
        ],
    );
    let key = AgentCredentialSubmissionKey {
        workspace_id: workspace_id.clone(),
        principal_id: state.principal_id.clone(),
        agent_profile_id: agent_profile_id.clone(),
        credential_slot_id: credential_slot_id.clone(),
        idempotency_key_digest,
    };

    // Exact replay is resolved before mutable profile and descriptor state. The
    // receipt read verifies Workspace ownership and is safe for an archived
    // Workspace so an already-reserved external write can be reconciled.
    let store = state.store.clone();
    let lookup_key = key.clone();
    let lookup =
        tokio::task::spawn_blocking(move || store.get_agent_credential_submission(lookup_key))
            .await;
    let existing = match lookup {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => return credential_store_error(error),
        Err(_) => return credential_unavailable(),
    };
    let mut replay = false;
    let mut created = false;
    let (secret_kind, secret_ref_id) = if let Some(receipt) = existing {
        replay = true;
        if receipt.request_fingerprint != request_fingerprint {
            return credential_idempotency_conflict();
        }
        if receipt.state == AgentCredentialSubmissionState::Committed {
            return credential_binding_response(StatusCode::OK, &credential_slot_id, &receipt);
        }
        (receipt.secret_kind, receipt.secret_ref_id)
    } else {
        // New operations must be admitted against current mutable state. The
        // reservation below is the authorization linearization point; no SQLite
        // transaction remains open while the OS keyring is touched.
        let store = state.store.clone();
        let principal_id = state.principal_id.clone();
        let profile_workspace = workspace_id.clone();
        let profile_id = agent_profile_id.clone();
        let now_text = recorded_at.clone();
        let profile_exists = tokio::task::spawn_blocking(move || {
            crate::operator::ensure_workspace_owner_for(&store, &principal_id, &profile_workspace)?;
            let profiles = store
                .list_agent_profiles(&principal_id, &profile_workspace, &now_text)
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
            Err(_) => return credential_unavailable(),
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
            Ok(Ok(None)) | Ok(Err(())) | Err(_) => return credential_unavailable(),
        };
        let snapshot = match AgentCredentialDescriptorSnapshot::from_control_descriptor(
            &agent_profile_id,
            &descriptor,
            now,
        ) {
            Ok(snapshot) => snapshot,
            Err(_) => return credential_unavailable(),
        };
        if snapshot.digest != request.descriptor_digest {
            return crate::operator::operator_error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Agent credential descriptor changed; refresh it before retrying",
            );
        }
        let secret_kind = match snapshot.secret_kind_for_slot(
            &agent_profile_id,
            &request.descriptor_digest,
            &credential_slot_id,
            now,
        ) {
            Ok(secret_kind) => secret_kind,
            Err(CredentialSubmissionError::SlotUnavailable) => {
                return crate::operator::operator_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "INVALID_ARGUMENT",
                    "Agent credential slot is unavailable",
                );
            }
            Err(_) => return credential_unavailable(),
        };
        let secret_ref_id = match new_secret_ref_id() {
            Some(value) => value,
            None => return credential_unavailable(),
        };
        let reservation = AgentCredentialSubmissionReservation {
            key: key.clone(),
            request_fingerprint: request_fingerprint.clone(),
            secret_kind,
            secret_ref_id,
            created_at: recorded_at,
        };
        let store = state.store.clone();
        let reserve = tokio::task::spawn_blocking(move || {
            store.reserve_agent_credential_submission(reservation)
        })
        .await;
        let outcome = match reserve {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(error)) => return credential_store_error(error),
            Err(_) => return credential_unavailable(),
        };
        created = outcome.created;
        if outcome.receipt.request_fingerprint != request_fingerprint {
            return credential_idempotency_conflict();
        }
        if outcome.receipt.state == AgentCredentialSubmissionState::Committed {
            return credential_binding_response(
                StatusCode::OK,
                &credential_slot_id,
                &outcome.receipt,
            );
        }
        (outcome.receipt.secret_kind, outcome.receipt.secret_ref_id)
    };

    // Idempotent SecretStore work occurs after the PENDING reservation and outside
    // SQLite. If settlement fails, the receipt remains PENDING and an exact retry
    // compares against the same stable keyring account before committing it.
    let store = state.store.clone();
    let secret_store = state.secret_store.clone();
    let workspace_for_write = workspace_id.clone();
    let key_for_commit = key.clone();
    let fingerprint_for_commit = request_fingerprint.clone();
    let recorded_at = match crate::operator::operator_now() {
        Ok(value) => value,
        Err(response) => return response,
    };
    let settled_at = recorded_at.clone();
    let write = tokio::task::spawn_blocking(move || {
        let secret_ref = secret_store.write_idempotent(
            &workspace_for_write,
            secret_kind,
            &secret_ref_id,
            secret_bytes.as_mut_slice(),
        );
        let secret_ref = match secret_ref {
            Ok(value) => value,
            Err(_) => return Err(()),
        };
        if !valid_secret_ref(&secret_ref, &secret_ref_id) {
            return Err(());
        }
        let receipt = store
            .commit_agent_credential_submission(
                key_for_commit,
                fingerprint_for_commit,
                secret_ref.provider_ref,
                settled_at,
            )
            .map_err(|_| ())?;
        if receipt.state != AgentCredentialSubmissionState::Committed {
            return Err(());
        }
        Ok(receipt)
    })
    .await;
    let receipt = match write {
        Ok(Ok(receipt)) => receipt,
        Ok(Err(())) | Err(_) => return credential_unavailable(),
    };
    let status = if created && !replay {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    credential_binding_response(status, &credential_slot_id, &receipt)
}

fn new_secret_ref_id() -> Option<String> {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).ok()?;
    let value = format!("secret_{}", hex::encode(random));
    zeroize::Zeroize::zeroize(&mut random);
    Some(value)
}

fn valid_secret_ref(secret_ref: &SecretRef, expected_id: &str) -> bool {
    secret_ref.secret_ref_id == expected_id
        && !secret_ref.secret_ref_id.is_empty()
        && secret_ref.secret_ref_id.len() <= 512
        && !secret_ref.secret_ref_id.chars().any(char::is_control)
        && !secret_ref.provider_ref.is_empty()
        && secret_ref.provider_ref.len() <= 512
        && !secret_ref.provider_ref.chars().any(char::is_control)
        && secret_ref.placement == SecretPlacement::LocalOnly
}

fn credential_binding_response(
    status: StatusCode,
    slot_id: &str,
    receipt: &AgentCredentialSubmissionReceipt,
) -> Response {
    let Some(provider_ref) = receipt.provider_ref.clone() else {
        return credential_unavailable();
    };
    if receipt.state != AgentCredentialSubmissionState::Committed {
        return credential_unavailable();
    }
    let binding = AgentCredentialBindingResponse {
        credential_slot_id: slot_id.to_owned(),
        secret_ref: SecretRef {
            secret_ref_id: receipt.secret_ref_id.clone(),
            provider_ref,
            placement: SecretPlacement::LocalOnly,
        },
    };
    (status, Json(binding)).into_response()
}

fn credential_idempotency_conflict() -> Response {
    crate::operator::operator_error(
        StatusCode::CONFLICT,
        "IDEMPOTENCY_CONFLICT",
        "Idempotency-Key was already used with a different credential request",
    )
}

fn credential_unavailable() -> Response {
    crate::operator::operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Agent credential submission is unavailable",
    )
}

fn credential_store_error(error: storage_core::StoreError) -> Response {
    match error {
        storage_core::StoreError::IdempotencyConflict => credential_idempotency_conflict(),
        storage_core::StoreError::NotFound => crate::operator::operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Agent credential receipt is unavailable in this Workspace",
        ),
        storage_core::StoreError::Conflict { .. } => crate::operator::operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Agent credential submission conflicts with current Workspace state",
        ),
        _ => credential_unavailable(),
    }
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
    use crate::agents::control_descriptor::CredentialStorageOwner;
    use axum::{body::to_bytes, http::Request};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{sync::Arc, time::Duration};
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

        fn write_idempotent(
            &self,
            _workspace_id: &str,
            _kind: SecretKind,
            secret_ref_id: &str,
            secret: &mut [u8],
        ) -> Result<SecretRef, StoreError> {
            self.0
                .lock()
                .expect("secret store mutex")
                .push(secret.to_vec());
            secret.fill(0);
            Ok(SecretRef {
                secret_ref_id: secret_ref_id.to_owned(),
                provider_ref: format!("test-keyring:{secret_ref_id}"),
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

    struct TestDescriptorSource {
        descriptor: AgentControlDescriptor,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl CurrentAgentDescriptorSource for TestDescriptorSource {
        fn current_descriptor(
            &self,
            _workspace_id: &str,
            _agent_profile_id: &str,
            _now: SystemTime,
        ) -> Result<Option<AgentControlDescriptor>, ()> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Some(self.descriptor.clone()))
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
        test_router_with_descriptor_source(store, secret_store, Arc::new(NoDescriptor))
    }

    fn test_router_with_descriptor_source(
        store: crate::operator::SqliteWorkspaceStore,
        secret_store: Arc<dyn SecretStore>,
        descriptor_source: Arc<dyn CurrentAgentDescriptorSource>,
    ) -> Router {
        routes_with_state::<CredentialSubmissionApiState>()
            .route_layer(middleware::from_fn(crate::operator::authenticate))
            .route_layer(middleware::from_fn(no_store_middleware))
            .with_state(CredentialSubmissionApiState {
                store,
                principal_id: OWNER.to_owned(),
                runtime_identity: Arc::new(
                    storage_sqlite::RuntimeOsPrincipalIdentity::test_support(
                        "credential-test-runtime",
                    )
                    .expect("test runtime identity"),
                ),
                secret_store,
                descriptor_source,
            })
    }

    fn body() -> String {
        r#"{"descriptor_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","secret_base64":"c2VjcmV0LW11c3Qtbm90LWxlYWs="}"#.to_owned()
    }

    fn body_with_digest(digest: &str) -> String {
        body_with_secret(digest, "c2VjcmV0LW11c3Qtbm90LWxlYWs=")
    }

    fn body_with_secret(digest: &str, encoded_secret: &str) -> String {
        format!("{{\"descriptor_digest\":\"{digest}\",\"secret_base64\":\"{encoded_secret}\"}}")
    }

    fn valid_test_descriptor() -> AgentControlDescriptor {
        let now = time::OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("whole second timestamp");
        let format = &time::format_description::well_known::Rfc3339;
        let observed_at = now.format(format).expect("format observed time");
        let expires_at = (now + time::Duration::minutes(15))
            .format(format)
            .expect("format descriptor expiry");
        let value = serde_json::json!({
            "descriptor_digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "descriptor_api_version": 1,
            "registry_agent_id": "agent.test",
            "module_id": "litecowork/test-agent",
            "module_version": "1.2.3",
            "harness_version": null,
            "protocol_version": "1.0",
            "installation": {
                "registry_agent_id": "agent.test", "module_id": "litecowork/test-agent",
                "runtime_id": "runtime-test", "runtime_incarnation_id": "runtime-incarnation-test",
                "state": "INSTALLED", "installed_version": "1.2.3", "available_version": null,
                "installation_source": "test", "observed_at": observed_at, "expires_at": expires_at
            },
            "auth": {
                "state": "NEEDS_AUTH", "account_label": null,
                "methods": [{"auth_method_id":"api-key","display_name":"API key","kind":"SECRET_SLOT","credential_slot_id":"api-key","requires_restart":false}],
                "credential_slots": [{"credential_slot_id":"api-key","display_name":"API key","secret_kind":"API_KEY","storage_owner":"LITECOWORK_SECRET_STORE","required":true}]
            },
            "configuration": {"schema_digest":"sha256:1111111111111111111111111111111111111111111111111111111111111111","fields":[],"native_config_targets":[]},
            "session_options": null,
            "input_surface": {"text":true,"images":false,"files":false,"resources":false,"slash_commands":[],"reference_kinds":[],"supports_command_discovery":false,"supports_reference_discovery":false},
            "native_surface": {"native_config_preserved":true,"native_extensions_preserved":true,"native_subagents_preserved":true,"summary":null},
            "capabilities": {
                "protocol":"ACP", "protocol_version":"1.0",
                "session":{"resume":false,"steer":false,"interrupt":false,"cancel":false,"fork":false,"model_switch":"UNSUPPORTED"},
                "input":{"text":true,"image":false,"file":false,"resources":false},
                "extension":{"mcp_stdio":false,"mcp_http":false,"skills":false,"plugins":false,"dynamic_attach":false},
                "reporting":{"tool_calls":false,"plan":false,"usage":false,"native_subagents":false,"approvals":false},
                "environment":{"cwd":false,"extra_directories":false},
                "limits":{"max_context_hint":null,"max_concurrent_sessions":null},
                "host_instruction_delivery":null
            },
            "observed_at": observed_at, "expires_at": expires_at
        });
        let mut descriptor: AgentControlDescriptor =
            serde_json::from_value(value).expect("descriptor fixture matches model");
        descriptor.descriptor_digest = descriptor
            .computed_digest()
            .expect("descriptor digest computes");
        descriptor.validate().expect("descriptor validates");
        descriptor
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

    #[tokio::test]
    async fn fresh_litecowork_owned_slot_writes_once_and_returns_only_secret_reference() {
        let (_directory, store) = fixture();
        let secret_store = Arc::new(RecordingSecretStore::default());
        let descriptor = valid_test_descriptor();
        let digest = descriptor.descriptor_digest.clone();
        let source = Arc::new(TestDescriptorSource {
            descriptor,
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let router =
            test_router_with_descriptor_source(store, secret_store.clone(), source.clone());
        let response = submit_body(&router, true, body_with_digest(&digest)).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = to_bytes(response.into_body(), 8192)
            .await
            .expect("bounded credential response");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("response JSON");
        assert_eq!(value["credential_slot_id"], "api-key");
        assert_eq!(
            value["secret_ref"]["secret_ref_id"].as_str().unwrap().len(),
            71
        );
        assert!(!String::from_utf8_lossy(&body).contains("secret-must-not-leak"));
        assert_eq!(
            secret_store
                .0
                .lock()
                .expect("secret store mutex")
                .as_slice(),
            &[b"secret-must-not-leak".to_vec()]
        );

        let replay = submit_body(&router, true, body_with_digest(&digest)).await;
        assert_eq!(replay.status(), StatusCode::OK);
        let replay_body = to_bytes(replay.into_body(), 8192)
            .await
            .expect("bounded replay response");
        let replay_value: serde_json::Value =
            serde_json::from_slice(&replay_body).expect("replay response JSON");
        assert_eq!(
            replay_value["secret_ref"]["secret_ref_id"],
            value["secret_ref"]["secret_ref_id"]
        );
        assert_eq!(source.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(
            secret_store.0.lock().expect("secret store mutex").len(),
            1,
            "committed replay returns the same ref without another keyring call"
        );

        let changed = body_with_secret(&digest, &STANDARD.encode(b"different-secret"));
        let conflict = submit_body(&router, true, changed).await;
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        let conflict_body = to_bytes(conflict.into_body(), 8192)
            .await
            .expect("bounded conflict response");
        let conflict_value: serde_json::Value =
            serde_json::from_slice(&conflict_body).expect("conflict response JSON");
        assert_eq!(conflict_value["code"], "IDEMPOTENCY_CONFLICT");
        assert_eq!(
            secret_store.0.lock().expect("secret store mutex").len(),
            1,
            "changed request does not mutate the keyring"
        );
    }

    #[tokio::test]
    async fn native_agent_owned_slot_is_rejected_before_secret_store_mutation() {
        let (_directory, store) = fixture();
        let secret_store = Arc::new(RecordingSecretStore::default());
        let mut descriptor = valid_test_descriptor();
        descriptor.auth.credential_slots[0].storage_owner = CredentialStorageOwner::NativeAgent;
        descriptor.auth.methods.clear();
        descriptor.descriptor_digest = descriptor
            .computed_digest()
            .expect("native-owned descriptor digest computes");
        descriptor
            .validate()
            .expect("native-owned descriptor remains valid");
        let digest = descriptor.descriptor_digest.clone();
        let source = Arc::new(TestDescriptorSource {
            descriptor,
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let router =
            test_router_with_descriptor_source(store, secret_store.clone(), source.clone());

        let response = submit_body(&router, true, body_with_digest(&digest)).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = to_bytes(response.into_body(), 8192)
            .await
            .expect("bounded denial response");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(value["code"], "INVALID_ARGUMENT");
        assert_eq!(source.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(
            secret_store
                .0
                .lock()
                .expect("secret store mutex")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn pending_receipt_recovery_reuses_the_reserved_secret_ref_without_descriptor() {
        let (_directory, store) = fixture();
        let secret_store = Arc::new(RecordingSecretStore::default());
        let identity =
            storage_sqlite::RuntimeOsPrincipalIdentity::test_support("credential-test-runtime")
                .expect("test runtime identity");
        let body = body();
        let request: AgentCredentialSecretSubmission =
            serde_json::from_str(&body).expect("credential request parses");
        let secret =
            decode_agent_credential_request(WORKSPACE, "profile-credential", "api-key", &request)
                .expect("credential request decodes");
        let fingerprint = identity.keyed_pseudonym_parts(
            "agent-credential-request-v1",
            &[request.descriptor_digest.as_bytes(), secret.as_slice()],
        );
        let key = AgentCredentialSubmissionKey {
            workspace_id: WORKSPACE.to_owned(),
            principal_id: OWNER.to_owned(),
            agent_profile_id: "profile-credential".to_owned(),
            credential_slot_id: "api-key".to_owned(),
            idempotency_key_digest: format!(
                "sha256:{}",
                hex::encode(sha2::Sha256::digest(b"credential-submit-test-1"))
            ),
        };
        let reservation = AgentCredentialSubmissionReservation {
            key: key.clone(),
            request_fingerprint: fingerprint,
            secret_kind: SecretKind::ApiKey,
            secret_ref_id: format!("secret_{}", "a".repeat(64)),
            created_at: NOW.to_owned(),
        };
        assert!(
            store
                .reserve_agent_credential_submission(reservation)
                .expect("pending receipt reserves")
                .created
        );

        let router = test_router(store.clone(), secret_store.clone());
        let response = submit(&router, true).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 8192)
            .await
            .expect("bounded recovery response");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("response JSON");
        assert_eq!(
            value["secret_ref"]["secret_ref_id"],
            format!("secret_{}", "a".repeat(64))
        );
        assert_eq!(
            store
                .get_agent_credential_submission(key)
                .expect("read reconciled receipt")
                .expect("receipt persists")
                .state,
            AgentCredentialSubmissionState::Committed
        );
        assert_eq!(secret_store.0.lock().expect("secret store mutex").len(), 1);
    }
}
