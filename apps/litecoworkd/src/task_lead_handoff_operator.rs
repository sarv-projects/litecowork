//! Owner-authorized Task lead handoff command adapter.
//!
//! `operator.rs` owns route registration. This module builds the authenticated
//! domain command and keeps the storage transition behind `TaskLeadHandoffStore`.
#![allow(clippy::result_large_err)]

use super::*;
use domain_task::{
    TaskLeadHandoffActor, TaskLeadHandoffCommand, TaskLeadHandoffMode, TaskLeadHandoffService,
    canonical_task_lead_handoff_payload,
};
use storage_core::{LocalRuntimeWorkspaceBindingLookup, TaskStore};

const MAX_HANDOFF_BODY_BYTES: usize = 8 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerHandoffBody {
    expected_task_spec_revision: u64,
    target_agent_binding_id: String,
}

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/tasks/{task_id}/lead-agent", post(owner_handoff))
        .layer(DefaultBodyLimit::max(MAX_HANDOFF_BODY_BYTES))
        .route_layer(middleware::from_fn(authenticate))
}

async fn owner_handoff(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(task_id): Path<String>,
    body: Bytes,
) -> Response {
    if body.len() > MAX_HANDOFF_BODY_BYTES {
        return invalid();
    }
    let command = match owner_handoff_command(&state, &headers, &task_id, &body) {
        Ok(command) => command,
        Err(response) => return response,
    };
    let workspace_id = command.workspace_id.clone();
    let correlation_id = command.event.correlation_id.clone();
    let store = state.store.clone();
    let committed = match tokio::task::spawn_blocking(move || execute_handoff(store, command)).await
    {
        Ok(Ok(committed)) => committed,
        Ok(Err(response)) => return response,
        Err(_) => return unavailable(),
    };
    match handoff_response(&workspace_id, &committed, &correlation_id) {
        Ok(response) => response,
        Err(response) => response,
    }
}

/// Called by the registered Operator handler after extracting a bounded body. The
/// helper authenticates Workspace ownership and active local Runtime membership,
/// derives the actor from the authenticated state, and does not accept a caller actor.
pub(super) fn owner_handoff_command(
    state: &ApiState,
    headers: &HeaderMap,
    task_id: &str,
    body_bytes: &[u8],
) -> Result<TaskLeadHandoffCommand, Response> {
    if body_bytes.len() > MAX_HANDOFF_BODY_BYTES || !valid_handoff_id(task_id) {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Task lead handoff request is invalid",
        ));
    }
    let workspace_id = selected_workspace(headers)?;
    let workspace = ensure_workspace_owner(state, &workspace_id)?;
    if workspace.status != "ACTIVE" {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace is not active",
        ));
    }
    let binding = state
        .store
        .get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
            owner_principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            runtime_id: state.runtime_id.clone(),
            runtime_incarnation_id: state.local_incarnation_id.clone(),
        })
        .map_err(|_| unavailable())?;
    if !binding.is_some_and(|binding| binding.status == "ACTIVE") {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Local Runtime Workspace access is unavailable",
        ));
    }

    let body: OwnerHandoffBody = serde_json::from_slice(body_bytes).map_err(|_| invalid())?;
    if body.expected_task_spec_revision == 0 || !valid_handoff_id(&body.target_agent_binding_id) {
        return Err(invalid());
    }
    let expected_task_version = parse_if_match(headers)?;
    let request_id = idempotency_key(headers)?;
    let event = owner_handoff_event_context(&state.runtime_id, &state.principal_id, &request_id)?;
    let mut command = TaskLeadHandoffCommand {
        workspace_id: workspace.workspace_id,
        task_id: task_id.to_owned(),
        request_scope_id: state.principal_id.clone(),
        request_id,
        request_payload: Value::Null,
        expected_task_version,
        expected_task_spec_revision: body.expected_task_spec_revision,
        target_agent_binding_id: body.target_agent_binding_id,
        mode: TaskLeadHandoffMode::OwnerRequested,
        actor: TaskLeadHandoffActor::Principal {
            principal_id: state.principal_id.clone(),
            kind: "USER".to_owned(),
        },
        trigger_observation: None,
        event,
    };
    command.request_payload = canonical_task_lead_handoff_payload(&command);
    Ok(command)
}

fn owner_handoff_event_context(
    runtime_id: &str,
    principal_id: &str,
    request_id: &str,
) -> Result<domain_workspace::EventContext, Response> {
    let correlation_id = idempotent_id(
        "cor",
        principal_id,
        request_id,
        "task-lead-handoff-correlation",
    );
    let mut event = event_context(runtime_id, &correlation_id).map_err(|_| unavailable())?;
    event.event_id = idempotent_id("ev", principal_id, request_id, "task-lead-handoff-event");
    Ok(event)
}

/// Executes an owner handoff using the independently implemented atomic persistence
/// adapter. The route wrapper is kept separate so the parent can register it only once
/// its SQLite transaction and canonical machine contract are available.
pub(super) fn execute_handoff<S: TaskStore>(
    store: S,
    command: TaskLeadHandoffCommand,
) -> Result<domain_task::CommittedTaskLeadHandoff, Response> {
    TaskLeadHandoffService::new(store)
        .execute(command)
        .map_err(handoff_store_error)
}

pub(super) fn handoff_response(
    workspace_id: &str,
    receipt: &domain_task::CommittedTaskLeadHandoff,
    correlation_id: &str,
) -> Result<Response, Response> {
    if !valid_handoff_id(workspace_id)
        || !valid_handoff_id(correlation_id)
        || receipt.task.workspace_id != workspace_id
        || receipt.task.task_id.trim().is_empty()
        || receipt.task.version != receipt.aggregate_version
        || receipt.task.lead_agent_binding_id != receipt.target_agent_binding_id
        || receipt.task.current_spec_revision != receipt.task_spec_revision
        || !valid_handoff_id(&receipt.previous_lead_agent_binding_id)
        || !valid_handoff_id(&receipt.target_agent_binding_id)
    {
        return Err(unavailable());
    }
    let mut response = Json(json!({
        "task_id": receipt.task.task_id,
        "workspace_id": receipt.task.workspace_id,
        "from_agent_binding_id": receipt.previous_lead_agent_binding_id,
        "to_agent_binding_id": receipt.target_agent_binding_id,
        "task_spec_revision": receipt.task_spec_revision,
        "aggregate_version": receipt.aggregate_version,
        "replayed": receipt.replayed,
    }))
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{}\"", receipt.aggregate_version))
            .map_err(|_| unavailable())?,
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("x-correlation-id"),
        HeaderValue::from_str(correlation_id).map_err(|_| unavailable())?,
    );
    Ok(response)
}

fn handoff_store_error(error: StoreError) -> Response {
    match error {
        StoreError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Task or target AgentBinding is unavailable",
        ),
        StoreError::Conflict { .. } => operator_error(
            StatusCode::CONFLICT,
            "STALE_TASK_VERSION",
            "Task changed before the lead handoff could be committed",
        ),
        StoreError::IdempotencyConflict => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Idempotency-Key was already used for another lead handoff",
        ),
        StoreError::Invalid(ref code) if code == "STALE_SPEC_REVISION" => operator_error(
            StatusCode::CONFLICT,
            "STALE_SPEC_REVISION",
            "Task specification changed before the lead handoff could be committed",
        ),
        StoreError::Invalid(_) => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Task lead handoff preconditions are not satisfied",
        ),
        StoreError::Busy => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORE_BUSY",
            "Task lead handoff could not be committed yet",
        ),
        _ => unavailable(),
    }
}

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Task lead handoff request is invalid",
    )
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Task lead handoff is unavailable",
    )
}

fn valid_handoff_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use std::{sync::Arc, time::Duration};
    use storage_core::{BlobPurpose, StoreError};
    use storage_sqlite::{
        FileBlobStore, RuntimeOsPrincipalIdentity, SqliteConfig, WorkspaceBlobKey,
        WorkspaceBlobKeyProvider,
    };
    use tower::ServiceExt;
    use zeroize::Zeroizing;

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([37; 32]),
            })
        }

        fn key_by_version(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::NotFound);
            }
            self.current_key("", BlobPurpose::Artifact)
        }
    }

    fn test_router() -> (tempfile::TempDir, Router) {
        let directory = tempfile::tempdir().expect("temporary Task handoff route directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("restrict temporary SQLite state directory");
        }
        let store = SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
            SqliteConfig {
                writer_queue_capacity: 4,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("SQLite store opens");
        let state = ApiState {
            store,
            principal_id: "handoff-owner".to_owned(),
            runtime_id: "handoff-runtime".to_owned(),
            local_incarnation_id: "handoff-incarnation".to_owned(),
            lifecycle_blockers: Vec::new(),
            runtime_identity: Arc::new(
                RuntimeOsPrincipalIdentity::test_support("handoff-runtime")
                    .expect("test Runtime identity"),
            ),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(1)),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_shutdown: tokio::sync::watch::channel(false).1,
        };
        (directory, routes().with_state(state))
    }

    #[test]
    fn owner_body_rejects_unknown_fields_and_missing_versions() {
        assert!(serde_json::from_slice::<OwnerHandoffBody>(
            br#"{"expected_task_spec_revision":4,"target_agent_binding_id":"binding-a","actor":{"kind":"SERVICE"}}"#
        )
        .is_err());
        assert!(
            serde_json::from_slice::<OwnerHandoffBody>(
                br#"{"target_agent_binding_id":"binding-a"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn request_id_and_expected_version_helpers_reject_missing_or_malformed_headers() {
        let headers = HeaderMap::new();
        assert!(parse_if_match(&headers).is_err());
        assert!(idempotency_key(&headers).is_err());

        let mut headers = HeaderMap::new();
        headers.insert(header::IF_MATCH, HeaderValue::from_static("0"));
        headers.insert("idempotency-key", HeaderValue::from_static("request-a"));
        assert!(parse_if_match(&headers).is_err());
        assert!(idempotency_key(&headers).is_ok());
    }

    #[test]
    fn owner_event_uses_deterministic_correlation_distinct_from_idempotency_key() {
        let first = owner_handoff_event_context("runtime-a", "owner-a", "request-a")
            .expect("event context");
        let replay = owner_handoff_event_context("runtime-a", "owner-a", "request-a")
            .expect("replay event context");
        assert_ne!(first.correlation_id, "request-a");
        assert_eq!(first.correlation_id, replay.correlation_id);
        assert_eq!(first.event_id, replay.event_id);
    }

    #[tokio::test]
    async fn changed_idempotency_payload_uses_the_canonical_conflict_error() {
        let response = handoff_store_error(StoreError::IdempotencyConflict);
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("bounded typed error");
        let error: Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(error["code"], "CONFLICT");
        assert_eq!(error["retryable"], false);

        let response = handoff_store_error(StoreError::Conflict {
            expected: Some(4),
            actual: Some(5),
        });
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("bounded stale Task error");
        let error: Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(error["code"], "STALE_TASK_VERSION");

        let response = handoff_store_error(StoreError::Invalid("STALE_SPEC_REVISION".to_owned()));
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("bounded stale TaskSpec error");
        let error: Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(error["code"], "STALE_SPEC_REVISION");

        let response = handoff_store_error(StoreError::Invalid(
            "Task is outside the safe lead handoff boundary".to_owned(),
        ));
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("bounded handoff precondition error");
        let error: Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(error["code"], "CONFLICT");
        assert!(
            !error["message"]
                .as_str()
                .unwrap_or_default()
                .contains("safe lead handoff")
        );
    }

    #[tokio::test]
    async fn registered_route_reaches_owner_command_and_requires_workspace_scope() {
        let (_directory, app) = test_router();
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/tasks/task-a/lead-agent")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"expected_task_spec_revision":1,"target_agent_binding_id":"binding-b"}"#,
            ))
            .expect("request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        let response = app.oneshot(request).await.expect("route responds");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn registered_route_rejects_bodies_over_eight_kibibytes() {
        let (_directory, app) = test_router();
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/tasks/task-a/lead-agent")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(vec![b' '; MAX_HANDOFF_BODY_BYTES + 1]))
            .expect("request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        let response = app.oneshot(request).await.expect("route responds");
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
