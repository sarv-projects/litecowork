//! Authenticated local Conversation catalog lifecycle. Turn dispatch is intentionally
//! absent here until the native AgentSession/runtime admission path is integrated.
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

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route("/v1/conversations/{conversation_id}", get(get_conversation))
}

async fn create_conversation(
    State(state): State<ApiState>,
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
    let workspace = ensure_workspace_owner(&state, &workspace_id)?;
    if workspace.status != "ACTIVE" {
        return Err(error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces cannot accept new Conversations",
        ));
    }
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
    let committed = SqliteConversationStore::new(state.store.clone())
        .create_conversation(commit)
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
    State(state): State<ApiState>,
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

async fn list_conversations(
    State(state): State<ApiState>,
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
