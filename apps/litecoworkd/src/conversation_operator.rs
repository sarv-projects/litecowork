//! Authenticated local Conversation catalog lifecycle. Turn dispatch is intentionally
//! absent here until the native AgentSession/runtime admission path is integrated.
// Route handlers intentionally return full Axum Responses to preserve status, body, and headers.
#![allow(clippy::result_large_err)]

use super::*;
use storage_core::conversation::request_payload_value;
use storage_core::{
    ConversationRecord, ConversationStore, CreateConversationCommit, WorkspaceCreateRequest,
};
use storage_sqlite::SqliteConversationStore;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    workspace_id: String,
    title: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitConversationTurnBody {
    content: Vec<ConversationTurnContentBlock>,
    #[serde(default)]
    resource_refs: Vec<ConversationTurnResourceRef>,
    #[serde(default)]
    agent_binding_id: Option<String>,
    #[serde(default)]
    presentation_preference: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum ConversationTurnContentBlock {
    #[serde(rename = "TEXT")]
    Text { text: String },
    #[serde(rename = "RESOURCE")]
    Resource {
        resource_ref: ConversationTurnResourceRef,
        #[serde(default)]
        display_name: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversationTurnResourceRef {
    workspace_id: String,
    resource_id: String,
    #[serde(default)]
    revision_id: Option<String>,
}

#[derive(Serialize)]
struct ConversationListResponse {
    items: Vec<ConversationRecord>,
    next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    workspace_id: String,
    created_at: String,
    conversation_id: String,
}

#[derive(Clone)]
struct ConversationApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
    runtime_id: String,
}

impl FromRef<ApiState> for ConversationApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
            runtime_id: state.runtime_id.clone(),
        }
    }
}

pub(super) fn routes() -> Router<ApiState> {
    routes_with_state::<ApiState>()
}

fn routes_with_state<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    ConversationApiState: FromRef<S>,
{
    Router::new()
        .route(
            "/v1/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route(
            "/v1/conversations/{conversation_id}/turns",
            post(submit_conversation_turn),
        )
        .route(
            "/v1/conversations/{conversation_id}/turns/{turn_id}/retry",
            post(retry_conversation_turn),
        )
        .route(
            "/v1/conversations/{conversation_id}/turns/{turn_id}/cancel",
            post(cancel_conversation_turn),
        )
        .route("/v1/conversations/{conversation_id}", get(get_conversation))
}

#[cfg(test)]
pub(super) fn test_router(
    store: SqliteWorkspaceStore,
    principal_id: String,
    runtime_id: String,
) -> Router {
    routes_with_state::<ConversationApiState>()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(ConversationApiState {
            store,
            principal_id,
            runtime_id,
        })
}

async fn create_conversation(
    State(state): State<ConversationApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if body.workspace_id != workspace_id {
        return Err(error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    ensure_workspace_owner_for(&state.store, &state.principal_id, &workspace_id)?;
    let title = body.title.map(|title| title.trim().to_owned());
    if title.as_ref().is_some_and(|value| {
        value.is_empty() || value.chars().count() > 160 || value.chars().any(char::is_control)
    }) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation title must contain 1 to 160 printable characters",
        ));
    }
    let request_id = idempotency_key(&headers)?;
    let conversation_id = new_id("con").map_err(|_| internal())?;
    let correlation_id = new_id("cor").map_err(|_| internal())?;
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| internal())?;
    let conversation = ConversationRecord {
        conversation_id: conversation_id.clone(),
        workspace_id: workspace_id.clone(),
        title: title.clone(),
        active_agent_binding_id: None,
        version: 1,
        created_at: context.recorded_at.clone(),
    };
    let commit = CreateConversationCommit {
        request: WorkspaceCreateRequest {
            principal_id: state.principal_id.clone(),
            request_id: request_id.to_owned(),
            request_payload: request_payload_value(&workspace_id, title.as_deref()),
        },
        conversation,
        event: storage_core::EventDraft {
            event_id: context.event_id,
            workspace_id,
            entity_type: "Conversation".to_owned(),
            entity_id: conversation_id.clone(),
            origin_runtime_id: context.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: context.hlc_timestamp,
            correlation_id: context.correlation_id,
            causation_id: context.causation_id,
            schema_version: 1,
            event_type: "conversation.created.v1".to_owned(),
            payload: json!({"conversation_id": conversation_id, "created_by":{"kind":"USER","principal_id":state.principal_id}}),
            recorded_at: context.recorded_at,
        },
    };
    // Conversation creation writes a content-addressed aggregate blob and waits for the
    // serialized SQLite writer. Keep that blocking persistence work off the Tokio
    // reactor, as the read routes and the other storage-backed write routes do.
    let store = state.store.clone();
    let committed = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(store).create_conversation(commit)
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?;
    let correlation =
        header::HeaderValue::from_str(&committed.event.correlation_id).map_err(|_| internal())?;
    let mut response = (StatusCode::CREATED, Json(committed.conversation)).into_response();
    response.headers_mut().insert(
        header::HeaderName::from_static("x-correlation-id"),
        correlation,
    );
    Ok(response)
}

async fn get_conversation(
    State(state): State<ConversationApiState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ConversationRecord>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&conversation_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation ID is invalid",
        ));
    }
    let record = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(state.store.clone()).get_conversation(
            &state.principal_id,
            &workspace_id,
            &conversation_id,
        )
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?;
    record.map(Json).ok_or_else(not_found)
}

async fn submit_conversation_turn(
    State(state): State<ConversationApiState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SubmitConversationTurnBody>,
) -> Result<Response, Response> {
    let _request_id = idempotency_key(&headers)?;
    let workspace_id = selected_workspace(&headers)?;
    validate_turn_request(&body, &workspace_id)?;
    ensure_conversation_available(&state, &headers, &conversation_id).await?;
    // Native Conversation sessions are not yet admitted through a qualified
    // AgentModule + Trust + Environment path. Do not persist an OPEN turn or user
    // message that has no provider capable of settling it.
    Err(turn_dispatch_unavailable())
}

async fn retry_conversation_turn(
    State(state): State<ConversationApiState>,
    Path((conversation_id, turn_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let _request_id = idempotency_key(&headers)?;
    let _expected_version = super::parse_if_match(&headers)?;
    if !valid_task_query_id(&turn_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation turn ID is invalid",
        ));
    }
    ensure_conversation_available(&state, &headers, &conversation_id).await?;
    Err(turn_dispatch_unavailable())
}

async fn cancel_conversation_turn(
    State(state): State<ConversationApiState>,
    Path((conversation_id, turn_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let _request_id = idempotency_key(&headers)?;
    let _expected_version = super::parse_if_match(&headers)?;
    if !valid_task_query_id(&turn_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation turn ID is invalid",
        ));
    }
    ensure_conversation_available(&state, &headers, &conversation_id).await?;
    Err(turn_dispatch_unavailable())
}

async fn ensure_conversation_available(
    state: &ConversationApiState,
    headers: &HeaderMap,
    conversation_id: &str,
) -> Result<(), Response> {
    let workspace_id = selected_workspace(headers)?;
    if !valid_task_query_id(conversation_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation ID is invalid",
        ));
    }
    let store = state.store.clone();
    let principal_id = state.principal_id.clone();
    let conversation_id = conversation_id.to_owned();
    let conversation = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(store).get_conversation(
            &principal_id,
            &workspace_id,
            &conversation_id,
        )
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?;
    conversation.map(|_| ()).ok_or_else(not_found)
}

fn validate_turn_request(
    body: &SubmitConversationTurnBody,
    workspace_id: &str,
) -> Result<(), Response> {
    let invalid = || {
        error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation turn request is invalid",
        )
    };
    if body.content.is_empty()
        || body.content.len() > 256
        || body.resource_refs.len() > 256
        || body
            .agent_binding_id
            .as_ref()
            .is_some_and(|id| !valid_task_query_id(id))
        || body
            .presentation_preference
            .as_deref()
            .is_some_and(|value| !matches!(value, "AUTO" | "SIMPLE" | "RICH"))
    {
        return Err(invalid());
    }
    let mut total_text_bytes = 0_usize;
    for block in &body.content {
        match block {
            ConversationTurnContentBlock::Text { text } => {
                total_text_bytes = total_text_bytes
                    .checked_add(text.len())
                    .ok_or_else(invalid)?;
            }
            ConversationTurnContentBlock::Resource {
                resource_ref,
                display_name,
            } => {
                if display_name.as_ref().is_some_and(|name| {
                    name.chars().count() > 512 || name.chars().any(char::is_control)
                }) {
                    return Err(invalid());
                }
                validate_turn_resource_ref(resource_ref, workspace_id).map_err(|_| invalid())?;
            }
        }
    }
    if total_text_bytes > 1024 * 1024 {
        return Err(invalid());
    }
    for resource_ref in &body.resource_refs {
        validate_turn_resource_ref(resource_ref, workspace_id).map_err(|_| invalid())?;
    }
    Ok(())
}

fn validate_turn_resource_ref(
    resource_ref: &ConversationTurnResourceRef,
    workspace_id: &str,
) -> Result<(), ()> {
    if resource_ref.workspace_id != workspace_id
        || !valid_task_query_id(&resource_ref.workspace_id)
        || !valid_task_query_id(&resource_ref.resource_id)
        || resource_ref
            .revision_id
            .as_ref()
            .is_some_and(|revision_id| !valid_task_query_id(revision_id))
    {
        return Err(());
    }
    Ok(())
}

fn turn_dispatch_unavailable() -> Response {
    let mut response = error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Qualified native Conversation session admission is unavailable; the message was not sent",
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

async fn list_conversations(
    State(state): State<ConversationApiState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<ConversationListResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation page limit must be between 1 and 100",
        ));
    }
    let cursor = query
        .cursor
        .as_deref()
        .map(|value| decode_cursor(value, &workspace_id))
        .transpose()?;
    let rows = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(state.store.clone()).list_conversations(
            &state.principal_id,
            &workspace_id,
            cursor.as_ref().map(|c| c.created_at.as_str()),
            cursor.as_ref().map(|c| c.conversation_id.as_str()),
            limit + 1,
        )
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?;
    let mut items = rows;
    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit);
    }
    let next_cursor = if has_more {
        items.last().map(encode_cursor).transpose()?
    } else {
        None
    };
    Ok(Json(ConversationListResponse { items, next_cursor }))
}

fn encode_cursor(row: &ConversationRecord) -> Result<String, Response> {
    serde_json::to_vec(&Cursor {
        version: 1,
        workspace_id: row.workspace_id.clone(),
        created_at: row.created_at.clone(),
        conversation_id: row.conversation_id.clone(),
    })
    .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
    .map_err(|_| internal())
}
fn decode_cursor(value: &str, workspace_id: &str) -> Result<Cursor, Response> {
    let invalid = || {
        error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation cursor is invalid",
        )
    };
    if value.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid())?;
    let cursor: Cursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || !valid_task_query_id(&cursor.conversation_id)
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
fn map_store_error(error_value: StoreError) -> Response {
    match error_value {
        StoreError::NotFound => not_found(),
        StoreError::Conflict { .. } => error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Idempotency key was already used for another Conversation request",
        ),
        StoreError::Invalid(message) if message == "WORKSPACE_ARCHIVED" => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces cannot accept new Conversations",
        ),
        StoreError::Invalid(_) => error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation request is invalid",
        ),
        _ => internal(),
    }
}
fn error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    operator_error(status, code, message)
}
fn not_found() -> Response {
    error(
        StatusCode::NOT_FOUND,
        "NOT_FOUND",
        "Conversation is unavailable",
    )
}
fn internal() -> Response {
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL",
        "Conversation service is unavailable",
    )
}

#[cfg(test)]
#[path = "conversation_operator_tests.rs"]
mod tests;
