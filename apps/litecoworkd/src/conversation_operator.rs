//! Authenticated local Conversation catalog lifecycle. Turn dispatch is intentionally
//! absent here until the native AgentSession/runtime admission path is integrated.
// Route handlers intentionally return full Axum Responses to preserve status, body, and headers.
#![allow(clippy::result_large_err)]

use super::*;
use storage_core::conversation::{
    request_payload_value, ConversationTurn, ConversationTurnStatus, PresentationPreference,
    TurnCommand, CONVERSATION_SESSION_ADMISSION_UNAVAILABLE,
};
use storage_core::{
    ConversationMessageRecord, ConversationRecord, ConversationStore, ConversationTurnStore,
    CreateConversationCommit, CreateConversationTurnCommit, EventDraft,
    SetConversationAgentBinding, WorkspaceCreateRequest,
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
struct SetConversationAgentBindingBody {
    agent_binding_id: Value,
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

#[derive(Deserialize, Serialize)]
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

#[derive(Deserialize, Serialize)]
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
        .route(
            "/v1/conversations/{conversation_id}/agent-binding",
            axum::routing::patch(set_conversation_agent_binding),
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

async fn set_conversation_agent_binding(
    State(state): State<ConversationApiState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SetConversationAgentBindingBody>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&conversation_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation ID is invalid",
        ));
    }
    let agent_binding_id = match body.agent_binding_id {
        Value::Null => None,
        Value::String(binding_id) if valid_task_query_id(&binding_id) => Some(binding_id),
        _ => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Conversation AgentBinding selection is invalid",
            ));
        }
    };
    let expected_version = super::parse_if_match(&headers)?;
    let request_id = idempotency_key(&headers)?;
    let request_payload = json!({
        "operation": "conversation.agent_binding.set.v1",
        "workspace_id": workspace_id,
        "conversation_id": conversation_id,
        "expected_version": expected_version,
        "agent_binding_id": agent_binding_id,
    });
    let correlation_id = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        "conversation-agent-binding",
    );
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| internal())?;
    let commit = SetConversationAgentBinding {
        request: WorkspaceCreateRequest {
            principal_id: state.principal_id.clone(),
            request_id,
            request_payload,
        },
        principal_id: state.principal_id.clone(),
        workspace_id: workspace_id.clone(),
        conversation_id: conversation_id.clone(),
        expected_version,
        agent_binding_id,
        event: EventDraft {
            event_id: context.event_id,
            workspace_id,
            entity_type: "Conversation".to_owned(),
            entity_id: conversation_id,
            origin_runtime_id: context.origin_runtime_id,
            entity_revision: expected_version.checked_add(1).ok_or_else(internal)?,
            hlc_timestamp: context.hlc_timestamp,
            correlation_id: context.correlation_id,
            causation_id: context.causation_id,
            schema_version: 1,
            event_type: "conversation.agent_binding.changed.v1".to_owned(),
            payload: json!({}),
            recorded_at: context.recorded_at,
        },
    };
    let committed = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(state.store.clone())
            .set_conversation_agent_binding_idempotent(commit)
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?;
    let correlation =
        header::HeaderValue::from_str(&committed.event.correlation_id).map_err(|_| internal())?;
    let mut response = Json(committed.conversation).into_response();
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
    let request_id = idempotency_key(&headers)?;
    let workspace_id = selected_workspace(&headers)?;
    validate_turn_request(&body, &workspace_id)?;
    let conversation =
        load_authorized_conversation(&state, &workspace_id, &conversation_id).await?;
    let presentation_preference =
        parse_presentation_preference(body.presentation_preference.as_deref())?;
    let content = serde_json::to_value(&body.content).map_err(|_| invalid_turn_request())?;
    let resource_refs =
        serde_json::to_value(&body.resource_refs).map_err(|_| invalid_turn_request())?;
    let content_digest = canonical_digest(&content)?;
    let principal_id = state.principal_id.clone();
    let author = json!({"kind":"USER","principal_id":principal_id});
    // Entity identity is derived from the authenticated principal and request key so an
    // exact retry rebuilds the same canonical request payload instead of conflicting.
    let message_id = idempotent_id(
        "msg",
        &principal_id,
        &request_id,
        "conversation-turn-message",
    );
    let turn_id = idempotent_id("turn", &principal_id, &request_id, "conversation-turn");
    let request_payload = json!({
        "operation": "conversation.turn.create.v1",
        "workspace_id": workspace_id,
        "conversation_id": conversation_id,
        "turn_id": turn_id,
        "message_id": message_id,
        "author": author,
        "content": content,
        "resource_refs": resource_refs,
        "presentation_preference": serde_json::to_value(presentation_preference)
            .map_err(|_| invalid_turn_request())?,
    });
    let replay_store = state.store.clone();
    let replay_principal_id = principal_id.clone();
    let replay_request_id = request_id.clone();
    let replay_payload = request_payload.clone();
    let replayed = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(replay_store).replay_conversation_turn(
            &replay_principal_id,
            &replay_request_id,
            &replay_payload,
        )
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?;
    let agent_binding_id = match replayed {
        Some(receipt) => receipt
            .events
            .iter()
            .find(|event| event.event_type == "conversation.turn.created.v2")
            .and_then(|event| event.payload.get("agent_binding_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(internal)?,
        None => resolve_turn_agent_binding(&state, &workspace_id, &conversation)?,
    };
    // A caller-supplied AgentBinding is compared for identity only; it never
    // rewrites the Conversation and never overrides the resolved selection. An exact
    // receipt replay uses the originally admitted selection above.
    if body
        .agent_binding_id
        .as_deref()
        .is_some_and(|requested| requested != agent_binding_id)
    {
        return Err(agent_unavailable());
    }
    let correlation_id = idempotent_id("cor", &principal_id, &request_id, "conversation-turn");
    let created = event_context(&state.runtime_id, &correlation_id).map_err(|_| internal())?;
    let turn_created = event_context(&state.runtime_id, &correlation_id).map_err(|_| internal())?;
    let next_conversation_revision = conversation.version.checked_add(1).ok_or_else(internal)?;
    let message = ConversationMessageRecord {
        message_id: message_id.clone(),
        conversation_id: conversation_id.clone(),
        author: author.clone(),
        role: "USER".to_owned(),
        agent_session_id: None,
        agent_binding_id: None,
        turn_id: Some(turn_id.clone()),
        content: content.clone(),
        resource_refs: resource_refs.clone(),
        source_channel_ref: None,
        created_at: created.recorded_at.clone(),
    };
    let turn = ConversationTurn {
        turn_id: turn_id.clone(),
        conversation_id: conversation_id.clone(),
        user_message_id: message_id.clone(),
        agent_session_id: None,
        status: ConversationTurnStatus::Open,
        retry_ordinal: 0,
        presentation_preference,
        created_at: created.recorded_at.clone(),
        settled_at: None,
        version: 1,
    };
    let commit = CreateConversationTurnCommit {
        request: WorkspaceCreateRequest {
            principal_id: principal_id.clone(),
            request_id: request_id.clone(),
            request_payload: request_payload.clone(),
        },
        principal_id: principal_id.clone(),
        workspace_id: workspace_id.clone(),
        message,
        turn,
        message_event: EventDraft {
            event_id: created.event_id,
            workspace_id: workspace_id.clone(),
            entity_type: "Conversation".to_owned(),
            entity_id: conversation_id.clone(),
            origin_runtime_id: created.origin_runtime_id,
            entity_revision: next_conversation_revision,
            hlc_timestamp: created.hlc_timestamp,
            correlation_id: created.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.message.added.v1".to_owned(),
            payload: json!({
                "message_id": message_id,
                "conversation_id": conversation_id,
                "author": author,
                "content_digest": content_digest,
                "resource_refs": resource_refs,
            }),
            recorded_at: created.recorded_at,
        },
        turn_event: EventDraft {
            event_id: turn_created.event_id,
            workspace_id: workspace_id.clone(),
            entity_type: "ConversationTurn".to_owned(),
            entity_id: turn_id.clone(),
            origin_runtime_id: turn_created.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: turn_created.hlc_timestamp,
            correlation_id: turn_created.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.turn.created.v2".to_owned(),
            payload: json!({
                "turn_id": turn_id,
                "conversation_id": conversation_id,
                "user_message_id": message_id,
                "agent_binding_id": agent_binding_id,
                "aggregate_version": 1,
                "presentation_preference": serde_json::to_value(presentation_preference)
                    .map_err(|_| invalid_turn_request())?,
            }),
            recorded_at: turn_created.recorded_at,
        },
    };
    // The durable USER message, OPEN turn, Conversation version bump, both canonical
    // events, and the idempotency receipt commit atomically before any admission try.
    let store = state.store.clone();
    let committed = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(store).create_conversation_turn(commit)
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?;

    // A ready Runtime and EXECUTOR binding are inventory facts, not evidence that a
    // native AgentSession is ready or that the user's message was dispatched. This
    // Operator state has no qualified session supervisor, so the turn remains fail-closed.
    let expected_turn_version = committed.turn.version;
    let expected_turn_revision = expected_turn_version.checked_add(1).ok_or_else(internal)?;

    // Second transaction: settle the same durable turn FAILED so no OPEN turn can be
    // stranded by an unavailable admission.
    let settled_context =
        event_context(&state.runtime_id, &correlation_id).map_err(|_| internal())?;
    let settled_at = settled_context.recorded_at.clone();
    let settled_event = EventDraft {
        event_id: settled_context.event_id,
        workspace_id: workspace_id.clone(),
        entity_type: "ConversationTurn".to_owned(),
        entity_id: committed.turn.turn_id.clone(),
        origin_runtime_id: settled_context.origin_runtime_id,
        entity_revision: expected_turn_revision,
        hlc_timestamp: settled_context.hlc_timestamp,
        correlation_id: settled_context.correlation_id,
        causation_id: None,
        schema_version: 1,
        event_type: "conversation.turn.settled.v1".to_owned(),
        payload: json!({
            "turn_id": committed.turn.turn_id,
            "from": "OPEN",
            "to": "FAILED",
            "reason_code": CONVERSATION_SESSION_ADMISSION_UNAVAILABLE,
            "aggregate_version": expected_turn_revision,
        }),
        recorded_at: settled_context.recorded_at,
    };
    let settled = run_turn_transition(
        state.store.clone(),
        principal_id,
        workspace_id,
        conversation_id,
        committed.turn.turn_id.clone(),
        expected_turn_version,
        TurnCommand::Fail { settled_at },
        settled_event,
    )
    .await;
    match settled {
        Ok(turn) if turn.status == ConversationTurnStatus::Failed => {
            Err(turn_admission_unavailable())
        }
        Ok(_) => Err(settlement_inconsistent()),
        // Only the failure settlement above advances an OPEN turn of this build. A
        // replay or a concurrent duplicate that finds the settled version writes nothing.
        Err(StoreError::Conflict {
            expected: Some(expected),
            actual: Some(actual),
        }) if expected == expected_turn_version && actual > expected_turn_version => {
            Err(turn_admission_unavailable())
        }
        Err(_) => Err(turn_settlement_failed()),
    }
}

async fn run_turn_transition(
    store: SqliteWorkspaceStore,
    principal_id: String,
    workspace_id: String,
    conversation_id: String,
    turn_id: String,
    expected_version: u64,
    command: TurnCommand,
    event: EventDraft,
) -> Result<ConversationTurn, StoreError> {
    tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(store).transition_conversation_turn(
            &principal_id,
            &workspace_id,
            &conversation_id,
            &turn_id,
            expected_version,
            command,
            event,
        )
    })
    .await
    .unwrap_or(Err(StoreError::ExecutorStopped))
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
    Err(turn_command_unavailable())
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
    Err(turn_command_unavailable())
}

async fn ensure_conversation_available(
    state: &ConversationApiState,
    headers: &HeaderMap,
    conversation_id: &str,
) -> Result<(), Response> {
    let workspace_id = selected_workspace(headers)?;
    load_authorized_conversation(state, &workspace_id, conversation_id)
        .await
        .map(|_| ())
}

/// Resolve the AgentBinding a new Conversation turn is admitted against.
///
/// Precedence follows the accepted Conversation admission contract: the explicit
/// Conversation override first, then the Workspace default. The first configured
/// selection is authoritative, so an override that is no longer enabled or
/// lead-eligible is refused instead of silently falling through to the Workspace
/// default. This performs no write; the storage transaction resolves and rechecks
/// the same selection before it persists anything.
fn resolve_turn_agent_binding(
    state: &ConversationApiState,
    workspace_id: &str,
    conversation: &ConversationRecord,
) -> Result<String, Response> {
    let workspace = state
        .store
        .get_workspace(workspace_id)
        .map_err(|_| internal())?;
    let Some(workspace) = workspace else {
        return Err(not_found());
    };
    if workspace.owner_principal_id != state.principal_id {
        return Err(not_found());
    }
    // An archived Workspace rejects every domain mutation. The storage transaction
    // answers the same way, so the route must not report a binding problem first.
    if workspace.status != "ACTIVE" {
        return Err(not_found());
    }
    let selected = conversation
        .active_agent_binding_id
        .clone()
        .or_else(|| workspace.default_agent_binding_id.clone());
    let Some(selected) = selected else {
        return Err(agent_unavailable());
    };
    // The bound matches what the durable turn commit itself requires, so the route
    // never refuses a selection the storage would admit.
    if selected.trim().is_empty() || selected.chars().any(char::is_control) {
        return Err(agent_unavailable());
    }
    Ok(selected)
}

async fn load_authorized_conversation(
    state: &ConversationApiState,
    workspace_id: &str,
    conversation_id: &str,
) -> Result<ConversationRecord, Response> {
    if !valid_task_query_id(conversation_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation ID is invalid",
        ));
    }
    let store = state.store.clone();
    let principal_id = state.principal_id.clone();
    let workspace_id = workspace_id.to_owned();
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
    conversation.ok_or_else(not_found)
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

fn parse_presentation_preference(value: Option<&str>) -> Result<PresentationPreference, Response> {
    match value {
        None | Some("AUTO") => Ok(PresentationPreference::Auto),
        Some("SIMPLE") => Ok(PresentationPreference::Simple),
        Some("RICH") => Ok(PresentationPreference::Rich),
        Some(_) => Err(invalid_turn_request()),
    }
}

fn canonical_digest(value: &Value) -> Result<String, Response> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|_| invalid_turn_request())?;
    Ok(sha256_digest(&bytes))
}

fn invalid_turn_request() -> Response {
    error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Conversation turn request is invalid",
    )
}

fn agent_unavailable() -> Response {
    error(
        StatusCode::UNPROCESSABLE_ENTITY,
        "AGENT_UNAVAILABLE",
        "Choose and enable a lead agent for this Conversation before sending a message",
    )
}

fn turn_command_unavailable() -> Response {
    no_store(error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Qualified native Conversation session admission is unavailable; the message was not sent",
    ))
}

fn turn_admission_unavailable() -> Response {
    no_store(error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "The message was saved, but qualified native Conversation session admission is unavailable; it was not sent to an agent",
    ))
}

fn turn_settlement_failed() -> Response {
    no_store(error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL",
        "The durable Conversation turn could not be settled after admission was unavailable; retry with the same Idempotency-Key",
    ))
}

fn settlement_inconsistent() -> Response {
    no_store(error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL",
        "The Conversation turn did not reach its unavailable-admission settlement state",
    ))
}

fn no_store(mut response: Response) -> Response {
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
        StoreError::Conflict {
            expected: None,
            actual: None,
        } => error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Idempotency key was already used for another Conversation request",
        ),
        StoreError::Conflict { .. } => error(
            StatusCode::CONFLICT,
            "STALE_CONVERSATION_VERSION",
            "Conversation changed; refresh before saving",
        ),
        StoreError::Invalid(message) if message == "AGENT_UNAVAILABLE" => agent_unavailable(),
        StoreError::Invalid(message) if message == "AGENT_NOT_LEAD_ELIGIBLE" => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "AGENT_NOT_LEAD_ELIGIBLE",
            "Choose an enabled AgentBinding that is allowed to lead work",
        ),
        StoreError::Invalid(message) if message == "WORKSPACE_ARCHIVED" => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces cannot be changed",
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
