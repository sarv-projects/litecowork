//! Owner-scoped, read-only projections for durable CapabilityInvocation history.
//! This module does not create, transition, or dispatch an Invocation.

#![allow(clippy::result_large_err)]

use super::*;
use storage_core::{
    CapabilityInvocationPageRequest, CapabilityInvocationRecord, CapabilityInvocationStore,
    InvocationStatus,
};
use storage_sqlite::SqliteCapabilityInvocationStore;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvocationPageQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InvocationPageCursor {
    version: u8,
    workspace_id: String,
    task_id: String,
    created_at: String,
    invocation_id: String,
}

#[derive(Clone)]
struct CapabilityInvocationsApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
}

impl FromRef<ApiState> for CapabilityInvocationsApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
        }
    }
}

pub(super) fn routes() -> Router<ApiState> {
    routes_with_state::<ApiState>()
}

fn routes_with_state<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    CapabilityInvocationsApiState: FromRef<S>,
{
    Router::new()
        .route(
            "/v1/tasks/{task_id}/invocations",
            get(list_task_invocations),
        )
        .route(
            "/v1/capability-invocations/{invocation_id}",
            get(get_invocation),
        )
        .route_layer(middleware::from_fn(authenticate))
        .route_layer(middleware::from_fn(no_store_middleware))
}

async fn get_invocation(
    State(state): State<CapabilityInvocationsApiState>,
    Path(invocation_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    if !valid_invocation_id(&invocation_id) {
        return no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "CapabilityInvocation identifier is invalid",
        ));
    }
    let owner = state.principal_id;
    let store = SqliteCapabilityInvocationStore::new(state.store);
    match tokio::task::spawn_blocking(move || {
        store.get_invocation_for_owner(&owner, &workspace_id, &invocation_id)
    })
    .await
    {
        Ok(Ok(Some(record))) => no_store(Json(invocation_view(&record)).into_response()),
        Ok(Ok(None)) | Ok(Err(storage_core::StoreError::NotFound)) => no_store(operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "CapabilityInvocation is unavailable",
        )),
        Ok(Err(_)) | Err(_) => no_store(operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "CapabilityInvocation history is unavailable",
        )),
    }
}

async fn list_task_invocations(
    State(state): State<CapabilityInvocationsApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    query: Result<Query<InvocationPageQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    let Query(query) = match query {
        Ok(value) => value,
        Err(_) => {
            return no_store(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "CapabilityInvocation page query is invalid",
            ));
        }
    };
    if !valid_invocation_id(&workspace_id) || !valid_invocation_id(&task_id) {
        return no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "CapabilityInvocation scope is invalid",
        ));
    }
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "CapabilityInvocation page limit must be between 1 and 200",
        ));
    }
    let cursor = match query
        .cursor
        .as_deref()
        .map(|value| decode_cursor(value, &workspace_id, &task_id))
        .transpose()
    {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    let owner = state.principal_id;
    let store = SqliteCapabilityInvocationStore::new(state.store);
    let result = tokio::task::spawn_blocking(move || {
        store.list_task_invocations_for_owner(CapabilityInvocationPageRequest {
            owner_principal_id: owner,
            workspace_id,
            task_id,
            after_created_at: cursor.as_ref().map(|value| value.created_at.clone()),
            after_invocation_id: cursor.as_ref().map(|value| value.invocation_id.clone()),
            limit: limit + 1,
        })
    })
    .await;
    let mut records = match result {
        Ok(Ok(value)) => value,
        Ok(Err(storage_core::StoreError::NotFound)) => {
            return no_store(operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Task invocation history is unavailable",
            ));
        }
        Ok(Err(_)) | Err(_) => {
            return no_store(operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "Task invocation history is unavailable",
            ));
        }
    };
    let has_more = records.len() > limit;
    records.truncate(limit);
    let next_cursor = if has_more {
        records.last().map(|record| {
            encode_cursor(&InvocationPageCursor {
                version: 1,
                workspace_id: record.workspace_id.clone(),
                task_id: record.task_id.as_deref().unwrap_or_default().to_owned(),
                created_at: record.created_at.clone(),
                invocation_id: record.invocation_id.clone(),
            })
        })
    } else {
        None
    };
    let next_cursor = match next_cursor.transpose() {
        Ok(value) => value,
        Err(()) => {
            return no_store(operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "CapabilityInvocation cursor could not be created",
            ));
        }
    };
    no_store(
        Json(json!({
            "items": records.iter().map(invocation_view).collect::<Vec<_>>(),
            "next_cursor": next_cursor,
        }))
        .into_response(),
    )
}

fn invocation_view(record: &CapabilityInvocationRecord) -> Value {
    let scope = match record.scope_kind.as_str() {
        "CONVERSATION" => {
            json!({ "kind": "CONVERSATION", "conversation_id": record.conversation_id })
        }
        "TASK_PLANNING" => json!({ "kind": "TASK_PLANNING", "task_id": record.task_id }),
        "ATTEMPT_EXECUTION" => {
            json!({ "kind": "ATTEMPT_EXECUTION", "task_id": record.task_id, "attempt_id": record.attempt_id })
        }
        _ => Value::Null,
    };
    let action_batch = record.action_batch_id.as_ref().map(|id| {
        json!({
            "action_batch_id": id,
            "ordinal": record.action_batch_ordinal,
            "operation_count": record.action_batch_operation_count,
            "batch_digest": record.action_batch_digest,
        })
    });
    json!({
        "invocation_id": record.invocation_id,
        "workspace_id": record.workspace_id,
        "scope": scope,
        "agent_session_id": record.agent_session_id,
        "capability_grant_id": record.capability_grant_id,
        "activation_id": record.activation_id,
        "capability_ref": record.capability_ref,
        "operation": record.operation,
        "request_digest": record.request_digest,
        "execution_method": record.execution_method,
        "action_batch": action_batch,
        "idempotency_key": record.idempotency_key,
        "status": record.status.as_str(),
        "provider_task_status": record.provider_task_status,
        "provider_task_created_at": record.provider_task_created_at,
        "provider_task_expires_at": record.provider_task_expires_at,
        "provider_task_ttl_ms": record.provider_task_ttl_ms,
        "provider_poll_after_ms": record.provider_poll_after_ms,
        "provider_updated_at": record.provider_updated_at,
        "partial_result_refs": record.partial_result_refs,
        "result_refs": record.result_refs,
        "effect_id": record.effect_id,
        "failure": failure_view(record.failure.as_ref()),
        "created_at": record.created_at,
        "updated_at": record.updated_at,
        "completed_at": record.completed_at,
        "version": record.version,
    })
}

/// The public FailureRecord contract permits arbitrary `details`. Keep that
/// provider/adapter-supplied object private so diagnostic payloads cannot smuggle
/// credentials or raw provider output into the Operator response.
fn failure_view(failure: Option<&Value>) -> Value {
    let Some(failure) = failure.and_then(Value::as_object) else {
        return Value::Null;
    };
    let Some(code) = failure.get("code").and_then(Value::as_str) else {
        return Value::Null;
    };
    let Some(category) = failure.get("category").and_then(Value::as_str) else {
        return Value::Null;
    };
    if !matches!(
        category,
        "TRANSIENT" | "PERMANENT" | "POLICY" | "USER_REQUIRED" | "AMBIGUOUS"
    ) {
        return Value::Null;
    }
    let Some(safe_message) = failure.get("safe_message").and_then(Value::as_str) else {
        return Value::Null;
    };
    let Some(retryable) = failure.get("retryable").and_then(Value::as_bool) else {
        return Value::Null;
    };
    let Some(signature) = failure.get("signature").and_then(Value::as_str) else {
        return Value::Null;
    };
    let Some(correlation_id) = failure.get("correlation_id").and_then(Value::as_str) else {
        return Value::Null;
    };
    json!({
        "code": code,
        "category": category,
        "safe_message": safe_message,
        "retryable": retryable,
        "signature": signature,
        "correlation_id": correlation_id,
    })
}

fn valid_invocation_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn decode_cursor(
    encoded: &str,
    workspace_id: &str,
    task_id: &str,
) -> Result<InvocationPageCursor, Response> {
    let invalid = || {
        operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "CapabilityInvocation cursor is invalid or belongs to another scope",
        )
    };
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|_| invalid())?;
    let cursor: InvocationPageCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || cursor.task_id != task_id
        || !valid_invocation_id(&cursor.invocation_id)
        || time::OffsetDateTime::parse(
            &cursor.created_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return Err(invalid());
    }
    Ok(cursor)
}

fn encode_cursor(cursor: &InvocationPageCursor) -> Result<String, ()> {
    serde_json::to_vec(cursor)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| ())
}

async fn no_store_middleware(request: Request<Body>, next: Next) -> Response {
    no_store(next.run(request).await)
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::FromRef;
    use storage_core::BlobPurpose;
    use storage_sqlite::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
    use zeroize::Zeroizing;

    #[derive(Clone)]
    struct TestApiState {
        store: SqliteWorkspaceStore,
        principal_id: String,
    }

    impl FromRef<TestApiState> for CapabilityInvocationsApiState {
        fn from_ref(state: &TestApiState) -> Self {
            Self {
                store: state.store.clone(),
                principal_id: state.principal_id.clone(),
            }
        }
    }

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, storage_core::StoreError> {
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
        ) -> Result<WorkspaceBlobKey, storage_core::StoreError> {
            if version != 1 {
                return Err(storage_core::StoreError::Blob(
                    "unknown Invocation route test key version".to_owned(),
                ));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    fn test_router() -> (Router, tempfile::TempDir) {
        let directory = tempfile::tempdir().expect("private Invocation route fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("restrict route fixture directory");
        }
        let store = SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
            SqliteConfig {
                writer_queue_capacity: 4,
                busy_timeout: std::time::Duration::from_secs(2),
            },
        )
        .expect("SQLite store opens");
        let router = routes_with_state::<TestApiState>().with_state(TestApiState {
            store,
            principal_id: "invocation-route-owner".to_owned(),
        });
        (router, directory)
    }
    fn invocation() -> CapabilityInvocationRecord {
        CapabilityInvocationRecord {
            invocation_id: "invocation-1".into(),
            workspace_id: "workspace-1".into(),
            scope_kind: "TASK_PLANNING".into(),
            conversation_id: None,
            task_id: Some("task-1".into()),
            attempt_id: None,
            agent_session_id: "session-1".into(),
            capability_grant_id: "grant-1".into(),
            activation_id: "activation-1".into(),
            capability_ref: json!({"capability_id":"resource.read"}),
            operation: "workspace.resource.read".into(),
            request_digest: format!("sha256:{}", "a".repeat(64)),
            execution_method: "DETERMINISTIC_LOCAL".into(),
            action_batch_id: Some("batch-1".into()),
            action_batch_ordinal: Some(0),
            action_batch_operation_count: Some(1),
            action_batch_digest: Some(format!("sha256:{}", "b".repeat(64))),
            idempotency_key: Some("member-1".into()),
            status: InvocationStatus::Waiting,
            provider_task_status: Some("WORKING".into()),
            provider_task_created_at: None,
            provider_task_expires_at: None,
            provider_task_ttl_ms: None,
            provider_poll_after_ms: None,
            provider_updated_at: None,
            partial_result_refs: json!([]),
            result_refs: json!([]),
            effect_id: None,
            failure: None,
            created_at: "2026-10-18T00:00:00Z".into(),
            updated_at: "2026-10-18T00:00:01Z".into(),
            completed_at: None,
            version: 1,
        }
    }

    #[test]
    fn invocation_projection_matches_nested_contract_and_omits_private_provider_bindings() {
        let mut record = invocation();
        record.failure = Some(json!({
            "code":"PROVIDER_UNAVAILABLE",
            "category":"AMBIGUOUS",
            "safe_message":"The provider response needs reconciliation.",
            "retryable":false,
            "signature":"sha256:deadbeef",
            "correlation_id":"correlation-1",
            "details":{"access_token":"must-not-be-exposed","raw_response":"private"}
        }));
        let view = invocation_view(&record);
        assert_eq!(
            view["scope"],
            json!({"kind":"TASK_PLANNING","task_id":"task-1"})
        );
        assert_eq!(
            view["action_batch"],
            json!({
                "action_batch_id":"batch-1",
                "ordinal":0,
                "operation_count":1,
                "batch_digest":format!("sha256:{}", "b".repeat(64))
            })
        );
        assert_eq!(view["status"], "WAITING");
        assert_eq!(view["provider_task_status"], "WORKING");
        assert!(view.get("provider_task_id").is_none());
        assert!(view.get("provider_handle_ref").is_none());
        assert_eq!(view["failure"]["code"], "PROVIDER_UNAVAILABLE");
        assert!(view["failure"].get("details").is_none());
        assert!(
            serde_json::to_string(&view)
                .expect("projection serializes")
                .find("must-not-be-exposed")
                .is_none()
        );
    }

    #[test]
    fn invocation_cursor_is_bound_to_workspace_and_task() {
        let cursor = InvocationPageCursor {
            version: 1,
            workspace_id: "workspace-1".into(),
            task_id: "task-1".into(),
            created_at: "2026-10-18T00:00:00Z".into(),
            invocation_id: "invocation-1".into(),
        };
        let encoded = encode_cursor(&cursor).expect("encode cursor");
        let decoded = decode_cursor(&encoded, "workspace-1", "task-1")
            .expect("cursor is valid for its original scope");
        assert_eq!(decoded.workspace_id, cursor.workspace_id);
        assert_eq!(decoded.task_id, cursor.task_id);
        assert_eq!(decoded.created_at, cursor.created_at);
        assert_eq!(decoded.invocation_id, cursor.invocation_id);
        assert!(decode_cursor(&encoded, "workspace-2", "task-1").is_err());
        assert!(decode_cursor(&encoded, "workspace-1", "task-2").is_err());
    }

    #[test]
    fn invocation_responses_are_never_cacheable() {
        let response =
            no_store((StatusCode::NOT_FOUND, Json(json!({"error": "NOT_FOUND"}))).into_response());
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn mounted_invocation_routes_hide_missing_scope_and_disable_caching() {
        let (router, _directory) = test_router();
        let unauthenticated = router
            .clone()
            .oneshot(
                Request::get("/v1/capability-invocations/invocation-1")
                    .body(Body::empty())
                    .expect("valid unauthenticated request"),
            )
            .await
            .expect("unauthenticated request returns response");
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(unauthenticated.headers()[header::CACHE_CONTROL], "no-store");

        let mut missing_workspace = Request::get("/v1/tasks/task-1/invocations?limit=10")
            .body(Body::empty())
            .expect("valid scoped request");
        missing_workspace.headers_mut().insert(
            "x-workspace-id",
            HeaderValue::from_static("workspace-missing"),
        );
        missing_workspace
            .extensions_mut()
            .insert(AuthenticatedLocalPeer);
        let missing_task = router
            .clone()
            .oneshot(missing_workspace)
            .await
            .expect("missing Workspace returns response");
        assert_eq!(missing_task.status(), StatusCode::NOT_FOUND);
        assert_eq!(missing_task.headers()[header::CACHE_CONTROL], "no-store");

        let mut missing_header = Request::get("/v1/capability-invocations/invocation-1")
            .body(Body::empty())
            .expect("valid request without scope header");
        missing_header
            .extensions_mut()
            .insert(AuthenticatedLocalPeer);
        let invalid_scope = router
            .oneshot(missing_header)
            .await
            .expect("missing scope returns response");
        assert_eq!(invalid_scope.status(), StatusCode::BAD_REQUEST);
        assert_eq!(invalid_scope.headers()[header::CACHE_CONTROL], "no-store");
    }
}
