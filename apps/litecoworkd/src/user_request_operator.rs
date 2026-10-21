//! Read-only, selected-Workspace UserRequest detail for the local Needs You surface.

#![allow(clippy::result_large_err)]

use super::*;
use axum::extract::FromRef;
use storage_core::{
    DismissUserRequestCommit, EventDraft, RespondUserRequestCommit, USER_REQUEST_DETAIL_MAX_BYTES,
    USER_REQUEST_ID_MAX_BYTES, USER_REQUEST_RESPONSE_MAX_BYTES, UserRequestDetailReadStore,
    UserRequestResolutionActor, UserRequestResolutionStore, UserRequestResponseInput,
    UserRequestResponseRecord, UserRequestSummary,
};
use storage_sqlite::SqliteUserRequestResolutionStore;

const MAX_USER_REQUEST_ACTION_BODY_BYTES: usize = USER_REQUEST_RESPONSE_MAX_BYTES + 1024;

#[derive(Clone)]
pub(super) struct UserRequestApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
    runtime_id: String,
}

impl FromRef<ApiState> for UserRequestApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
            runtime_id: state.runtime_id.clone(),
        }
    }
}

pub(super) fn routes() -> Router<UserRequestApiState> {
    Router::new()
        .route("/v1/user-requests", get(list_user_requests))
        .route(
            "/v1/user-requests/{request_id}",
            get(get_user_request_detail),
        )
        .route(
            "/v1/user-requests/{request_id}/respond",
            post(respond_to_user_request),
        )
        .route(
            "/v1/user-requests/{request_id}/dismiss",
            post(dismiss_user_request),
        )
        .layer(DefaultBodyLimit::max(MAX_USER_REQUEST_ACTION_BODY_BYTES))
        .route_layer(middleware::from_fn(authenticate))
        .route_layer(middleware::from_fn(user_request_no_store))
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RespondBody {
    Value(ResponseValueBody),
    Choice(ChoiceIdBody),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseValueBody {
    response: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChoiceIdBody {
    choice_id: String,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct UserRequestResolutionReceiptView {
    request_id: String,
    workspace_id: String,
    status: String,
    version: u64,
    resolved_at: String,
    resolved_by: UserRequestResolutionActor,
    response_digest: Option<String>,
    event_id: String,
}

async fn respond_to_user_request(
    State(state): State<UserRequestApiState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    body: Bytes,
) -> Response {
    if body.len() > MAX_USER_REQUEST_ACTION_BODY_BYTES {
        return add_no_store(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "USER_REQUEST_RESPONSE_TOO_LARGE",
            "UserRequest response exceeds its local size limit",
        ));
    }
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return add_no_store(response),
    };
    if !valid_user_request_id(&request_id) {
        return add_no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "UserRequest identifier is invalid",
        ));
    }
    let key = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(response) => return add_no_store(response),
    };
    let expected_version = match parse_if_match(&headers) {
        Ok(value) if value > 0 => value,
        Ok(_) => {
            return add_no_store(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "If-Match must contain a positive UserRequest version",
            ));
        }
        Err(response) => return add_no_store(response),
    };
    let body: RespondBody = match serde_json::from_slice::<RespondBody>(&body) {
        Ok(value) => value,
        Err(_) => {
            return add_no_store(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "UserRequest response body must contain exactly one supported input",
            ));
        }
    };
    let input = match body {
        RespondBody::Value(value) => UserRequestResponseInput::Value(value.response),
        RespondBody::Choice(choice) => UserRequestResponseInput::ChoiceId(choice.choice_id),
    };
    let actor = UserRequestResolutionActor {
        principal_id: state.principal_id.clone(),
        kind: "USER".to_owned(),
    };
    let correlation_id = idempotent_id("cor", &state.principal_id, &key, "user-request-respond");
    let event = match event_context(&state.runtime_id, &correlation_id) {
        Ok(context) => EventDraft {
            event_id: idempotent_id("evt", &state.principal_id, &key, "user-request-resolved"),
            workspace_id: workspace_id.clone(),
            entity_type: "user_request".to_owned(),
            entity_id: request_id.clone(),
            origin_runtime_id: state.runtime_id.clone(),
            entity_revision: expected_version.saturating_add(1),
            hlc_timestamp: context.hlc_timestamp,
            correlation_id: context.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "user.request.resolved.v1".to_owned(),
            payload: serde_json::json!({}),
            recorded_at: context.recorded_at,
        },
        Err(()) => {
            return add_no_store(operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "UserRequest event context is unavailable",
            ));
        }
    };
    let command = RespondUserRequestCommit {
        owner_principal_id: state.principal_id.clone(),
        actor,
        workspace_id: workspace_id.clone(),
        request_id: request_id.clone(),
        idempotency_key: key.clone(),
        expected_version,
        response_id: idempotent_id("urs", &state.principal_id, &key, "user-request-response"),
        input,
        event,
    };
    let owner = state.principal_id.clone();
    let store = state.store.clone();
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &owner, &workspace_id)?;
        SqliteUserRequestResolutionStore::new(store)
            .respond_to_user_request(command)
            .map_err(user_request_resolution_store_error)
    })
    .await;
    match result {
        Ok(Ok(receipt)) => {
            let Some(response_digest) = receipt.response_digest else {
                return add_no_store(operator_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTEGRITY_FAILURE",
                    "UserRequest response receipt is incomplete",
                ));
            };
            let response = UserRequestResponseRecord {
                request_id: receipt.request_id,
                response_digest,
                responded_by: receipt.resolved_by,
                response_channel_ref: None,
                responded_at: receipt.resolved_at,
            };
            add_no_store(Json(response).into_response())
        }
        Ok(Err(response)) => add_no_store(response),
        Err(_) => add_no_store(operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "UserRequest response could not be saved",
        )),
    }
}

async fn dismiss_user_request(
    State(state): State<UserRequestApiState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
) -> Response {
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return add_no_store(response),
    };
    if !valid_user_request_id(&request_id) {
        return add_no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "UserRequest identifier is invalid",
        ));
    }
    let key = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(response) => return add_no_store(response),
    };
    let expected_version = match parse_if_match(&headers) {
        Ok(value) if value > 0 => value,
        Ok(_) => {
            return add_no_store(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "If-Match must contain a positive UserRequest version",
            ));
        }
        Err(response) => return add_no_store(response),
    };
    let actor = UserRequestResolutionActor {
        principal_id: state.principal_id.clone(),
        kind: "USER".to_owned(),
    };
    let correlation_id = idempotent_id("cor", &state.principal_id, &key, "user-request-dismiss");
    let event = match event_context(&state.runtime_id, &correlation_id) {
        Ok(context) => EventDraft {
            event_id: idempotent_id("evt", &state.principal_id, &key, "user-request-resolved"),
            workspace_id: workspace_id.clone(),
            entity_type: "user_request".to_owned(),
            entity_id: request_id.clone(),
            origin_runtime_id: state.runtime_id.clone(),
            entity_revision: expected_version.saturating_add(1),
            hlc_timestamp: context.hlc_timestamp,
            correlation_id: context.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "user.request.resolved.v1".to_owned(),
            payload: serde_json::json!({}),
            recorded_at: context.recorded_at,
        },
        Err(()) => {
            return add_no_store(operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "UserRequest event context is unavailable",
            ));
        }
    };
    let command = DismissUserRequestCommit {
        owner_principal_id: state.principal_id.clone(),
        actor,
        workspace_id: workspace_id.clone(),
        request_id: request_id.clone(),
        idempotency_key: key.clone(),
        expected_version,
        event,
    };
    let owner = state.principal_id.clone();
    let store = state.store.clone();
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &owner, &workspace_id)?;
        SqliteUserRequestResolutionStore::new(store)
            .dismiss_user_request(command)
            .map_err(user_request_resolution_store_error)
    })
    .await;
    match result {
        Ok(Ok(receipt)) => add_no_store(
            Json(UserRequestResolutionReceiptView {
                request_id: receipt.request_id,
                workspace_id: receipt.workspace_id,
                status: receipt.status,
                version: receipt.version,
                resolved_at: receipt.resolved_at,
                resolved_by: receipt.resolved_by,
                response_digest: receipt.response_digest,
                event_id: receipt.event.event_id,
            })
            .into_response(),
        ),
        Ok(Err(response)) => add_no_store(response),
        Err(_) => add_no_store(operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "UserRequest dismissal could not be saved",
        )),
    }
}

async fn user_request_no_store(request: Request<Body>, next: Next) -> Response {
    add_no_store(next.run(request).await)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UserRequestListQuery {
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UserRequestListCursor {
    version: u8,
    workspace_id: String,
    status: Option<String>,
    limit: usize,
    created_at: String,
    request_id: String,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct UserRequestPageView {
    items: Vec<UserRequestSummary>,
    next_cursor: Option<String>,
}

async fn list_user_requests(
    State(state): State<UserRequestApiState>,
    headers: HeaderMap,
    query: Result<axum::extract::Query<UserRequestListQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let parsed = (|| -> Result<(String, Option<String>, usize, Option<(String, String)>), Response> {
        let workspace_id = selected_workspace(&headers)?;
        let query = query.map_err(|_| invalid_user_request("UserRequest page query is invalid"))?.0;
        let status = match query.status {
            Some(value)
                if matches!(
                    value.as_str(),
                    "PENDING" | "ANSWERED" | "DISMISSED" | "EXPIRED" | "CANCELLED"
                ) =>
            {
                Some(value)
            }
            Some(_) => return Err(invalid_user_request("UserRequest status is invalid")),
            None => None,
        };
        let limit = query.limit.unwrap_or(50);
        if !(1..=100).contains(&limit) {
            return Err(invalid_user_request("UserRequest page limit is invalid"));
        }
        let cursor = match query.cursor {
            None => None,
            Some(encoded) => {
                if encoded.is_empty() || encoded.len() > 2048 {
                    return Err(invalid_user_request("UserRequest page cursor is invalid"));
                }
                let bytes = URL_SAFE_NO_PAD
                    .decode(encoded.as_bytes())
                    .map_err(|_| invalid_user_request("UserRequest page cursor is invalid"))?;
                let cursor: UserRequestListCursor = serde_json::from_slice(&bytes)
                    .map_err(|_| invalid_user_request("UserRequest page cursor is invalid"))?;
                if cursor.version != 1
                    || cursor.workspace_id != workspace_id
                    || cursor.status != status
                    || cursor.limit != limit
                    || cursor.request_id.trim().is_empty()
                    || cursor.request_id.len() > USER_REQUEST_ID_MAX_BYTES
                    || !valid_cursor_timestamp(&cursor.created_at)
                {
                    return Err(invalid_user_request("UserRequest page cursor is invalid"));
                }
                Some((cursor.created_at, cursor.request_id))
            }
        };
        Ok((workspace_id, status, limit, cursor))
    })();
    let (workspace_id, status, limit, cursor) = match parsed {
        Ok(value) => value,
        Err(response) => return add_no_store(response),
    };
    let owner = state.principal_id.clone();
    let selected = workspace_id.clone();
    let after_created = cursor.as_ref().map(|(created_at, _)| created_at.clone());
    let after_request = cursor.as_ref().map(|(_, request_id)| request_id.clone());
    let store = state.store.clone();
    let status_for_read = status.clone();
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &owner, &selected)?;
        store
            .list_user_requests_page(
                &owner,
                &selected,
                status_for_read.as_deref(),
                after_created.as_deref(),
                after_request.as_deref(),
                limit + 1,
            )
            .map_err(user_request_detail_store_error)
    })
    .await;
    let mut items = match result {
        Ok(Ok(items)) => items,
        Ok(Err(response)) => return add_no_store(response),
        Err(_) => {
            return add_no_store(operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "UserRequest page is unavailable",
            ));
        }
    };
    let next_cursor = if items.len() > limit {
        items.truncate(limit);
        items.last().map(|last| {
            URL_SAFE_NO_PAD.encode(
                serde_json::to_vec(&UserRequestListCursor {
                    version: 1,
                    workspace_id,
                    status,
                    limit,
                    created_at: last.created_at.clone(),
                    request_id: last.request_id.clone(),
                })
                .expect("bounded UserRequest cursor serializes"),
            )
        })
    } else {
        None
    };
    add_no_store(
        Json(UserRequestPageView {
            items,
            next_cursor,
        })
        .into_response(),
    )
}

fn invalid_user_request(message: &'static str) -> Response {
    operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", message)
}

fn valid_cursor_timestamp(value: &str) -> bool {
    value.len() <= 64
        && time::OffsetDateTime::parse(
            value,
            &time::format_description::well_known::Rfc3339,
        )
        .is_ok()
}

async fn get_user_request_detail(
    State(state): State<UserRequestApiState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
) -> Response {
    let result = (|| -> Result<_, Response> {
        let workspace_id = selected_workspace(&headers)?;
        if !valid_user_request_id(&request_id) {
            return Err(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "UserRequest identifier is invalid",
            ));
        }
        Ok(workspace_id)
    })();
    let workspace_id = match result {
        Ok(workspace_id) => workspace_id,
        Err(response) => return add_no_store(response),
    };

    let owner = state.principal_id.clone();
    let selected = workspace_id.clone();
    let request_for_read = request_id.clone();
    let store = state.store.clone();
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &owner, &selected)?;
        store
            .get_user_request_detail(&selected, &request_for_read)
            .map_err(user_request_detail_store_error)
    })
    .await;
    let detail = match result {
        Ok(Ok(Some(detail))) => detail,
        Ok(Ok(None)) => {
            return add_no_store(operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "UserRequest is unavailable",
            ));
        }
        Ok(Err(response)) => return add_no_store(response),
        Err(_) => {
            return add_no_store(operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "UserRequest details are unavailable",
            ));
        }
    };
    let encoded = match serde_json::to_vec(&detail) {
        Ok(encoded) if encoded.len() <= USER_REQUEST_DETAIL_MAX_BYTES => encoded,
        _ => {
            return add_no_store(operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "UserRequest details exceed the local read boundary",
            ));
        }
    };
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(encoded))
        .unwrap_or_else(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "UserRequest details could not be prepared",
            )
        });
    add_no_store(response)
}

fn valid_user_request_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= USER_REQUEST_ID_MAX_BYTES
        && !value.chars().any(char::is_control)
}

fn user_request_detail_store_error(error: StoreError) -> Response {
    match error {
        StoreError::Invalid(_) => operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "UserRequest identifier is invalid",
        ),
        StoreError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "UserRequest is unavailable",
        ),
        StoreError::Integrity(_) => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "UserRequest details could not be verified",
        ),
        _ => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "UserRequest details are unavailable",
        ),
    }
}

fn user_request_resolution_store_error(error: StoreError) -> Response {
    match error {
        StoreError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "UserRequest is unavailable",
        ),
        StoreError::Conflict { .. } => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "UserRequest is stale or no longer pending",
        ),
        StoreError::IdempotencyConflict => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Idempotency-Key was reused for a different UserRequest action",
        ),
        StoreError::Invalid(code) if code == "USER_REQUEST_EXPIRED" => operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "USER_REQUEST_EXPIRED",
            "UserRequest has expired",
        ),
        StoreError::Invalid(code) if code == "SENSITIVE_INPUT_UNSUPPORTED" => operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SENSITIVE_INPUT_UNSUPPORTED",
            "This request cannot safely collect sensitive information",
        ),
        StoreError::Invalid(code) if code == "PROVIDER_INPUT_UNSUPPORTED" => operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PROVIDER_INPUT_UNSUPPORTED",
            "This provider request uses an unsupported response contract",
        ),
        StoreError::Invalid(code) if code == "USER_REQUEST_RESPONSE_TOO_LARGE" => operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "USER_REQUEST_RESPONSE_TOO_LARGE",
            "UserRequest response exceeds its size or nesting limit",
        ),
        StoreError::Invalid(_) => operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ARGUMENT",
            "UserRequest response does not match the current request",
        ),
        StoreError::Integrity(_) => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "UserRequest state could not be verified",
        ),
        _ => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "UserRequest storage is unavailable",
        ),
    }
}

fn add_no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
pub(super) fn test_router(store: SqliteWorkspaceStore, principal_id: String) -> Router {
    routes().with_state(UserRequestApiState {
        store,
        principal_id,
        runtime_id: "user-request-test-runtime".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::Connection;
    use std::{sync::Arc, time::Duration};
    use storage_core::{BlobPurpose, StoreError};
    use storage_sqlite::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
    use zeroize::Zeroizing;

    const OWNER: &str = "user-request-detail-owner";
    const WORKSPACE: &str = "user-request-detail-workspace";
    const FOREIGN_WORKSPACE: &str = "user-request-detail-other";
    const OTHER_OWNER_WORKSPACE: &str = "user-request-detail-other-owner";
    const OTHER_OWNER: &str = "user-request-detail-other-principal";
    const AT: &str = "2026-10-16T10:00:00Z";

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([59; 32]),
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

    fn fixture() -> (tempfile::TempDir, SqliteWorkspaceStore, Router) {
        let directory = tempfile::tempdir().expect("temporary UserRequest route directory");
        let database = directory.path().join("state.sqlite3");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("restrict test state directory to its owner");
        }
        let store = SqliteWorkspaceStore::open(
            &database,
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("real SQLite store opens");
        for (workspace_id, suffix, owner) in [
            (WORKSPACE, "main", OWNER),
            (FOREIGN_WORKSPACE, "foreign", OWNER),
            (OTHER_OWNER_WORKSPACE, "other-owner", OTHER_OWNER),
        ] {
            WorkspaceService::new(store.clone())
                .create(CreateWorkspace {
                    workspace_id: workspace_id.to_owned(),
                    name: "UserRequest details fixture".to_owned(),
                    owner_principal_id: owner.to_owned(),
                    event: EventContext {
                        event_id: format!("user-request-ws-{suffix}"),
                        origin_runtime_id: "user-request-runtime".to_owned(),
                        hlc_timestamp: AT.to_owned(),
                        correlation_id: format!("user-request-correlation-{suffix}"),
                        causation_id: None,
                        recorded_at: AT.to_owned(),
                    },
                })
                .expect("Workspace commits");
        }
        let connection = Connection::open(&database).expect("open fixture SQLite database");
        // This projection fixture avoids constructing the full Task/Attempt/session
        // admission graph. Disable only the fixture's FK and scope-admission guards to
        // seed synthetic rows for read isolation checks; production connections retain them.
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .expect("disable fixture-only foreign keys");
        connection
            .execute_batch("DROP TRIGGER user_request_admission_matches_scope;")
            .expect("remove admission-only test guard");
        for (task_id, workspace_id) in [
            ("task-visible", WORKSPACE),
            ("task-other", FOREIGN_WORKSPACE),
            ("task-private", OTHER_OWNER_WORKSPACE),
        ] {
            connection
                .execute(
                    "INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status,
                        lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                        created_at, updated_at, version)
                     VALUES (?1, ?2, 1, 'READY', 'missing-test-binding',
                        '[]', 'NORMAL', '{}', ?3, ?3, 1)",
                    rusqlite::params![task_id, workspace_id, AT],
                )
                .expect("same-Workspace Task parent exists for the request detail fixture");
        }
        connection
            .execute(
                "INSERT INTO agent_sessions(agent_session_id, workspace_id, scope_kind, task_id,
                task_spec_revision, agent_binding_id, endpoint_id, runtime_id,
                runtime_incarnation_id, status, started_at)
             VALUES ('session-internal', ?1, 'TASK_PLANNING', 'task-visible', 1,
                'binding-fixture', 'endpoint-fixture', 'runtime-fixture',
                'incarnation-fixture', 'ACTIVE', ?2)",
                rusqlite::params![WORKSPACE, AT],
            )
            .expect("synthetic Task planning session exists for mutation checks");
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                    kind, interaction_mode, prompt, choices_json, status, created_at, version)
                 VALUES ('request-visible', ?1, 'task-visible', 'session-internal', 'QUESTION',
                    'FORM', '<img src=x onerror=alert(1)>',
                    '[{\"choice_id\":\"choice-visible\",\"label\":\"Option\",\"value\":\"one\"}]',
                    'PENDING', ?2, 1)",
                rusqlite::params![WORKSPACE, AT],
            )
            .expect("valid UserRequest fixture inserts");
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                    kind, interaction_mode, prompt, response_schema_json, choices_json,
                    status, created_at, version)
                 VALUES ('request-choice', ?1, 'task-visible', 'session-internal', 'DECISION',
                    'FORM', 'Choose one',
                    '{\"type\":\"object\",\"required\":[\"choice\"],\"properties\":{\"choice\":{\"type\":\"string\",\"enum\":[\"one\",\"two\"]}},\"additionalProperties\":false}',
                    '[{\"choice_id\":\"choice-one\",\"label\":\"One\",\"description\":null,\"value\":{\"choice\":\"one\"}},{\"choice_id\":\"choice-two\",\"label\":\"Two\",\"description\":null,\"value\":{\"choice\":\"two\"}}]',
                    'PENDING', ?2, 1)",
                rusqlite::params![WORKSPACE, AT],
            )
            .expect("choice-backed UserRequest fixture inserts");
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                    kind, interaction_mode, prompt, choices_json, status, created_at, version)
                 VALUES ('request-dismiss', ?1, 'task-visible', 'session-internal', 'QUESTION',
                    'FORM', 'Dismiss this request', '[]', 'PENDING', ?2, 1)",
                rusqlite::params![WORKSPACE, AT],
            )
            .expect("dismissal UserRequest fixture inserts");
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                    kind, interaction_mode, prompt, response_schema_json, choices_json,
                    status, created_at, version)
                 VALUES ('request-null-response', ?1, 'task-visible', 'session-internal', 'QUESTION',
                    'FORM', 'Confirm the empty value', '{\"type\":\"null\"}', '[]',
                    'PENDING', ?2, 1)",
                rusqlite::params![WORKSPACE, AT],
            )
            .expect("null-response UserRequest fixture inserts");
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                    kind, interaction_mode, prompt, choices_json, status, created_at, version)
                 VALUES ('request-foreign', ?1, 'task-other', 'session-other', 'QUESTION',
                    'FORM', 'Not for this Workspace', '[]', 'PENDING', ?2, 1)",
                rusqlite::params![FOREIGN_WORKSPACE, AT],
            )
            .expect("foreign UserRequest fixture inserts");
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                    kind, interaction_mode, prompt, choices_json, status, created_at, version)
                 VALUES ('request-protected', ?1, 'task-private', 'session-private', 'QUESTION',
                    'FORM', 'Private owner data', '[]', 'PENDING', ?2, 1)",
                rusqlite::params![OTHER_OWNER_WORKSPACE, AT],
            )
            .expect("other-owner UserRequest fixture inserts");
        drop(connection);
        let router = test_router(store.clone(), OWNER.to_owned());
        (directory, store, router)
    }

    async fn list(
        router: Router,
        workspace_id: &str,
        query: &str,
        authenticated: bool,
    ) -> Response {
        let mut request = Request::get(format!("/v1/user-requests{query}"))
            .header("x-workspace-id", workspace_id)
            .body(Body::empty())
            .expect("list request builds");
        if authenticated {
            request.extensions_mut().insert(AuthenticatedLocalPeer);
        }
        router
            .oneshot(request)
            .await
            .expect("router returns list response")
    }

    async fn list_json(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 256 * 1024)
            .await
            .expect("list body reads");
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn get(router: Router, workspace_id: &str, request_id: &str) -> Response {
        let mut request = Request::get(format!("/v1/user-requests/{request_id}"))
            .header("x-workspace-id", workspace_id)
            .body(Body::empty())
            .expect("request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .oneshot(request)
            .await
            .expect("router returns response")
    }

    async fn post_action(
        router: Router,
        path: &str,
        workspace_id: &str,
        key: &str,
        body: &str,
    ) -> Response {
        let mut request = Request::post(path)
            .header("x-workspace-id", workspace_id)
            .header("idempotency-key", key)
            .header("if-match", "1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .expect("request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .oneshot(request)
            .await
            .expect("router returns action response")
    }

    #[tokio::test]
    async fn response_choice_id_commits_safely_and_exact_replay_is_idempotent() {
        let (directory, _store, router) = fixture();
        let path = "/v1/user-requests/request-choice/respond";
        let first = post_action(
            router.clone(),
            path,
            WORKSPACE,
            "choice-key-1",
            r#"{"choice_id":"choice-one"}"#,
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()[header::CACHE_CONTROL], "no-store");
        let bytes = to_bytes(first.into_body(), 16 * 1024)
            .await
            .expect("bounded response record");
        let response: Value = serde_json::from_slice(&bytes).expect("response receipt JSON");
        assert_eq!(response["request_id"], "request-choice");
        assert_eq!(response["response_digest"].as_str().unwrap().len(), 71);
        assert_eq!(response["responded_by"]["kind"], "USER");
        assert!(response.get("response").is_none());
        assert!(response.get("choice_id").is_none());

        let replay = post_action(
            router.clone(),
            path,
            WORKSPACE,
            "choice-key-1",
            r#"{"choice_id":"choice-one"}"#,
        )
        .await;
        assert_eq!(
            replay.status(),
            StatusCode::OK,
            "exact retry returns its committed result after the request is terminal"
        );
        let changed_payload = post_action(
            router,
            path,
            WORKSPACE,
            "choice-key-1",
            r#"{"choice_id":"choice-two"}"#,
        )
        .await;
        assert_eq!(changed_payload.status(), StatusCode::CONFLICT);
        let changed_payload_body = to_bytes(changed_payload.into_body(), 16 * 1024)
            .await
            .expect("bounded conflict body");
        let changed_payload_error: Value =
            serde_json::from_slice(&changed_payload_body).expect("conflict error JSON");
        assert_eq!(changed_payload_error["code"], "CONFLICT");
        let connection =
            Connection::open(directory.path().join("state.sqlite3")).expect("open action fixture");
        let (status, version, count): (String, i64, i64) = connection.query_row(
            "SELECT status, version, (SELECT count(*) FROM user_request_responses WHERE request_id='request-choice')
             FROM user_requests WHERE request_id='request-choice'", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).expect("read committed request state");
        assert_eq!((status.as_str(), version, count), ("ANSWERED", 2, 1));
        let resolved_events: i64 = connection.query_row(
            "SELECT count(*) FROM domain_events WHERE entity_id='request-choice' AND type='user.request.resolved.v1'",
            [], |row| row.get(0),
        ).expect("count committed resolution events");
        assert_eq!(resolved_events, 1);
    }

    #[tokio::test]
    async fn explicit_json_null_response_is_distinct_from_a_missing_response() {
        let (directory, _store, router) = fixture();
        let response = post_action(
            router,
            "/v1/user-requests/request-null-response/respond",
            WORKSPACE,
            "null-response-key",
            r#"{"response":null}"#,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("bounded response receipt");
        let receipt: Value = serde_json::from_slice(&body).expect("response record JSON");
        assert_eq!(receipt["request_id"], "request-null-response");
        assert_eq!(receipt["response_digest"].as_str().unwrap().len(), 71);

        let connection =
            Connection::open(directory.path().join("state.sqlite3")).expect("open action fixture");
        let (status, response_json): (String, String) = connection
            .query_row(
                "SELECT u.status, r.response_json FROM user_requests u
                 JOIN user_request_responses r USING (request_id)
                 WHERE u.request_id='request-null-response'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read committed explicit-null response");
        assert_eq!(
            (status.as_str(), response_json.as_str()),
            ("ANSWERED", "null")
        );

        let missing = post_action(
            test_router(_store, OWNER.to_owned()),
            "/v1/user-requests/request-null-response/respond",
            WORKSPACE,
            "missing-response-key",
            "{}",
        )
        .await;
        assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn dismissal_returns_only_minimal_receipt_and_commits_once() {
        let (directory, _store, router) = fixture();
        let path = "/v1/user-requests/request-dismiss/dismiss";
        let response = post_action(router.clone(), path, WORKSPACE, "dismiss-key-1", "").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let bytes = to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("bounded dismissal receipt");
        let receipt: Value = serde_json::from_slice(&bytes).expect("minimal receipt JSON");
        assert_eq!(receipt["request_id"], "request-dismiss");
        assert_eq!(receipt["status"], "DISMISSED");
        assert_eq!(receipt["version"], 2);
        assert!(receipt.get("response_schema_json").is_none());
        assert!(receipt.get("agent_session_id").is_none());
        assert!(receipt.get("choices").is_none());
        assert!(receipt.get("response").is_none());

        let replay = post_action(router, path, WORKSPACE, "dismiss-key-1", "").await;
        assert_eq!(replay.status(), StatusCode::OK);
        let connection =
            Connection::open(directory.path().join("state.sqlite3")).expect("open action fixture");
        let version: i64 = connection
            .query_row(
                "SELECT version FROM user_requests WHERE request_id='request-dismiss'",
                [],
                |row| row.get(0),
            )
            .expect("read dismissed version");
        let events: i64 = connection.query_row(
            "SELECT count(*) FROM domain_events WHERE entity_id='request-dismiss' AND type='user.request.resolved.v1'",
            [], |row| row.get(0),
        ).expect("count dismissal events");
        assert_eq!((version, events), (2, 1));
    }

    #[tokio::test]
    async fn selected_workspace_detail_is_exact_current_and_no_store() {
        let (_directory, _store, router) = fixture();
        let response = get(router, WORKSPACE, "request-visible").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("bounded request detail");
        let body: Value = serde_json::from_slice(&body).expect("valid UserRequest JSON");
        assert_eq!(body["request_id"], "request-visible");
        assert_eq!(body["workspace_id"], WORKSPACE);
        assert_eq!(body["scope"]["kind"], "TASK_PLANNING");
        assert_eq!(body["scope"]["task_id"], "task-visible");
        assert_eq!(body["status"], "PENDING");
        assert_eq!(body["version"], 1);
        assert_eq!(body["choices"][0]["label"], "Option");
        assert!(body["choices"][0].get("value").is_none());
        assert!(body.get("provider_url").is_none());
        assert!(body.get("provider_input_binding").is_none());
        assert!(body.get("agent_session_id").is_none());
    }

    #[tokio::test]
    async fn missing_and_foreign_workspace_requests_are_nondisclosing_not_found() {
        let (_directory, _store, router) = fixture();
        let foreign = get(router.clone(), WORKSPACE, "request-foreign").await;
        let missing = get(router.clone(), WORKSPACE, "request-absent").await;
        for response in [&foreign, &missing] {
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
        let foreign = to_bytes(foreign.into_body(), 16 * 1024)
            .await
            .expect("bounded error");
        let missing = to_bytes(missing.into_body(), 16 * 1024)
            .await
            .expect("bounded error");
        let foreign: Value = serde_json::from_slice(&foreign).expect("foreign error JSON");
        let missing: Value = serde_json::from_slice(&missing).expect("missing error JSON");
        assert_eq!(foreign["code"], missing["code"]);
        assert_eq!(foreign["message"], missing["message"]);

        let wrong_owner = get(router, OTHER_OWNER_WORKSPACE, "request-protected").await;
        assert_eq!(wrong_owner.status(), StatusCode::FORBIDDEN);
        assert_eq!(wrong_owner.headers()[header::CACHE_CONTROL], "no-store");
        let wrong_owner = to_bytes(wrong_owner.into_body(), 16 * 1024)
            .await
            .expect("bounded owner error");
        let wrong_owner = String::from_utf8_lossy(&wrong_owner);
        assert!(!wrong_owner.contains("request-visible"));
        assert!(!wrong_owner.contains("request-protected"));
        assert!(!wrong_owner.contains("Which option?"));
    }

    #[tokio::test]
    async fn route_rejects_unauthenticated_missing_workspace_and_malformed_ids() {
        let (_directory, _store, router) = fixture();
        let response = router
            .clone()
            .oneshot(
                Request::get("/v1/user-requests/request-visible")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("auth response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        let mut missing_workspace = Request::get("/v1/user-requests/request-visible")
            .body(Body::empty())
            .expect("request builds");
        missing_workspace
            .extensions_mut()
            .insert(AuthenticatedLocalPeer);
        let response = router
            .clone()
            .oneshot(missing_workspace)
            .await
            .expect("missing Workspace response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        let response = get(
            router.clone(),
            WORKSPACE,
            &"x".repeat(USER_REQUEST_ID_MAX_BYTES),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        let response = get(
            router,
            WORKSPACE,
            &"x".repeat(USER_REQUEST_ID_MAX_BYTES + 1),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn route_reads_the_latest_terminal_status_and_version() {
        let (directory, _store, router) = fixture();
        let connection = Connection::open(directory.path().join("state.sqlite3"))
            .expect("open request fixture before route read");
        connection
            .execute(
                "UPDATE user_requests SET status = 'DISMISSED', resolved_at = ?1, version = 2
             WHERE request_id = 'request-visible'",
                [AT],
            )
            .expect("request resolves before route read");
        let response = get(router, WORKSPACE, "request-visible").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("bounded body");
        let body: Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(body["status"], "DISMISSED");
        assert_eq!(body["version"], 2);
    }

    #[tokio::test]
    async fn owner_lists_prompt_free_user_requests_with_bound_cursor_pages() {
        let (_directory, _store, router) = fixture();
        let (status, _) = list_json(list(router.clone(), WORKSPACE, "", false).await).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let status = list(
            router.clone(),
            OTHER_OWNER_WORKSPACE,
            "?status=PENDING",
            true,
        )
        .await
        .status();
        assert_eq!(status, StatusCode::FORBIDDEN);

        let first_response = list(
            router.clone(),
            WORKSPACE,
            "?status=PENDING&limit=2",
            true,
        )
        .await;
        assert_eq!(
            first_response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let (status, first) = list_json(first_response).await;
        assert_eq!(status, StatusCode::OK);
        let items = first["items"].as_array().expect("items");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["request_id"], "request-choice");
        assert_eq!(items[1]["request_id"], "request-dismiss");
        assert!(items[0].get("prompt").is_none());
        assert!(items[0].get("choices").is_none());
        assert!(items[0].get("agent_session_id").is_none());
        let cursor = first["next_cursor"].as_str().expect("next cursor").to_owned();

        let (status, second) = list_json(
            list(
                router.clone(),
                WORKSPACE,
                &format!("?status=PENDING&limit=2&cursor={cursor}"),
                true,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let second_items = second["items"].as_array().expect("items");
        assert_eq!(second_items.len(), 2);
        assert_eq!(second_items[0]["request_id"], "request-null-response");
        assert_eq!(second_items[1]["request_id"], "request-visible");
        assert!(second["next_cursor"].is_null());

        let status = list(
            router.clone(),
            WORKSPACE,
            &format!("?status=PENDING&limit=1&cursor={cursor}"),
            true,
        )
        .await
        .status();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let status = list(router.clone(), WORKSPACE, "?status=UNKNOWN", true)
            .await
            .status();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, empty) = list_json(
            list(router, WORKSPACE, "?status=ANSWERED&limit=10", true).await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(empty["items"], serde_json::json!([]));
        assert!(empty["next_cursor"].is_null());
    }

}