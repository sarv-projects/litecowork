//! Authenticated local Conversation catalog lifecycle and turn routes (submit, retry,
//! cancel). Submit commits the USER message, OPEN turn and dispatch intent atomically and
//! returns 202; native work (today the OpenCode v2.0.32 adapter path only) starts through
//! `AgentTurnCoordinator`, never directly from a handler. Without a supervisor the turn is
//! settled FAILED and the Operator reports the dependency as unavailable.
// Route handlers intentionally return full Axum Responses to preserve status, body, and headers.
#![allow(clippy::result_large_err)]

use super::*;
use storage_core::conversation::{
    CONVERSATION_ARCHIVED, CONVERSATION_DELETED, CONVERSATION_NOT_QUIESCENT,
    CONVERSATION_SESSION_ADMISSION_UNAVAILABLE, CONVERSATION_TURN_IN_PROGRESS, ConversationStatus,
    ConversationTurn, ConversationTurnStatus, PresentationPreference, RenameConversation,
    SetConversationArchived, TurnCommand, request_payload_value,
};
use storage_core::{
    AgentSessionRecord, AgentSessionStore, CommittedConversation, ConversationHistoryStore,
    ConversationMessageRecord, ConversationRecord, ConversationStore, ConversationTurnReceipt,
    ConversationTurnStore, CreateConversationCommit, CreateConversationTurnCommit, EventDraft,
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
struct RenameConversationBody {
    /// Contract shape is `{ "title": string or null }`. `null` is accepted and then
    /// refused: a durable Conversation title is never empty, so V1 has no un-rename.
    title: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListQuery {
    cursor: Option<String>,
    limit: Option<usize>,
    /// `ACTIVE` (default) or `ARCHIVED` (014 contract row 14).
    status: Option<String>,
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
    local_incarnation_id: String,
    supervisor: Option<Arc<crate::agents::AgentSessionSupervisor>>,
}

impl FromRef<ApiState> for ConversationApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
            runtime_id: state.runtime_id.clone(),
            local_incarnation_id: state.local_incarnation_id.clone(),
            // The Operator's single process-lifetime supervisor, never a new one: the
            // Stop registry must be the same map the admitted turn registered into.
            supervisor: Some(Arc::clone(&state.supervisor)),
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
            "/v1/conversations/{conversation_id}/turns/{turn_id}/activity",
            get(list_conversation_turn_activity),
        )
        .route(
            "/v1/conversations/{conversation_id}/turns/{turn_id}/permissions",
            get(list_live_conversation_turn_permissions),
        )
        .route(
            "/v1/conversation-permissions",
            get(list_workspace_live_conversation_permissions),
        )
        .route(
            "/v1/conversations/{conversation_id}/turns/{turn_id}/permissions/{prompt_token}/reply",
            post(reply_live_conversation_turn_permission),
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
        .route(
            "/v1/conversations/{conversation_id}/archive",
            post(archive_conversation),
        )
        .route(
            "/v1/conversations/{conversation_id}/unarchive",
            post(unarchive_conversation),
        )
        .route(
            "/v1/conversations/{conversation_id}",
            get(get_conversation).patch(rename_conversation),
        )
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
            local_incarnation_id: String::new(),
            supervisor: None,
        })
}

#[cfg(test)]
pub(super) fn test_router_with_supervisor(
    store: SqliteWorkspaceStore,
    principal_id: String,
    runtime_id: String,
    local_incarnation_id: String,
    supervisor: Arc<crate::agents::AgentSessionSupervisor>,
) -> Router {
    routes_with_state::<ConversationApiState>()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(ConversationApiState {
            store,
            principal_id,
            runtime_id,
            local_incarnation_id,
            supervisor: Some(supervisor),
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

/// `PATCH /v1/conversations/{conversation_id}` (014 contract row 16): version-fenced,
/// idempotent rename. The committed event carries `title_digest` only, never the title.
async fn rename_conversation(
    State(state): State<ConversationApiState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<RenameConversationBody>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&conversation_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation ID is invalid",
        ));
    }
    let title = match body.title {
        Value::String(title) => title.trim().to_owned(),
        // The contract shape admits `null`; a Conversation title is never empty, so
        // clearing it is refused rather than silently accepted as a no-op.
        Value::Null => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "A Conversation title cannot be cleared",
            ));
        }
        _ => return Err(invalid_conversation_title()),
    };
    if title.is_empty() || title.chars().count() > 160 || title.chars().any(char::is_control) {
        return Err(invalid_conversation_title());
    }
    let expected_version = super::parse_if_match(&headers)?;
    let request_id = idempotency_key(&headers)?;
    let request_payload = json!({
        "operation": "conversation.rename.v1",
        "workspace_id": workspace_id,
        "conversation_id": conversation_id,
        "expected_version": expected_version,
        "title": title,
    });
    let correlation_id = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        "conversation-rename",
    );
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| internal())?;
    let commit = RenameConversation {
        request: WorkspaceCreateRequest {
            principal_id: state.principal_id.clone(),
            request_id,
            request_payload,
        },
        principal_id: state.principal_id.clone(),
        workspace_id: workspace_id.clone(),
        conversation_id: conversation_id.clone(),
        expected_version,
        title,
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
            event_type: "conversation.renamed.v1".to_owned(),
            payload: json!({}),
            recorded_at: context.recorded_at,
        },
    };
    let committed = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(state.store.clone()).rename_conversation(commit)
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_history_store_error)?;
    conversation_history_response(committed)
}

/// `POST /v1/conversations/{conversation_id}/archive` (014 contract row 17).
async fn archive_conversation(
    State(state): State<ConversationApiState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    set_conversation_archived_state(state, conversation_id, headers, true).await
}

/// `POST /v1/conversations/{conversation_id}/unarchive` (014 contract row 18).
async fn unarchive_conversation(
    State(state): State<ConversationApiState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    set_conversation_archived_state(state, conversation_id, headers, false).await
}

/// Version-fenced, idempotent archive or unarchive of one Conversation. Archiving is
/// refused while a turn is still nonterminal, and only the storage transaction decides
/// that so the check cannot race a concurrent turn.
async fn set_conversation_archived_state(
    state: ConversationApiState,
    conversation_id: String,
    headers: HeaderMap,
    archived: bool,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&conversation_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation ID is invalid",
        ));
    }
    let expected_version = super::parse_if_match(&headers)?;
    let request_id = idempotency_key(&headers)?;
    let operation = if archived {
        "conversation.archive.v1"
    } else {
        "conversation.unarchive.v1"
    };
    let event_type = if archived {
        "conversation.archived.v1"
    } else {
        "conversation.unarchived.v1"
    };
    let request_payload = json!({
        "operation": operation,
        "workspace_id": workspace_id,
        "conversation_id": conversation_id,
        "expected_version": expected_version,
    });
    let correlation_id = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        "conversation-archive",
    );
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| internal())?;
    let commit = SetConversationArchived {
        request: WorkspaceCreateRequest {
            principal_id: state.principal_id.clone(),
            request_id,
            request_payload,
        },
        principal_id: state.principal_id.clone(),
        workspace_id: workspace_id.clone(),
        conversation_id: conversation_id.clone(),
        expected_version,
        archived,
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
            event_type: event_type.to_owned(),
            payload: json!({}),
            recorded_at: context.recorded_at,
        },
    };
    let committed = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(state.store.clone()).set_conversation_archived(commit)
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_history_store_error)?;
    conversation_history_response(committed)
}

/// 200 Conversation plus the correlation id of the committed history event.
fn conversation_history_response(committed: CommittedConversation) -> Result<Response, Response> {
    let correlation =
        header::HeaderValue::from_str(&committed.event.correlation_id).map_err(|_| internal())?;
    let mut response = Json(committed.conversation).into_response();
    response.headers_mut().insert(
        header::HeaderName::from_static("x-correlation-id"),
        correlation,
    );
    Ok(response)
}

fn invalid_conversation_title() -> Response {
    error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Conversation title must contain 1 to 160 printable characters",
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TurnActivityQuery {
    retry_ordinal: Option<u32>,
}

#[derive(Serialize)]
struct TurnActivityPage {
    turn_id: String,
    retry_ordinal: u32,
    /// Always `REPORTED`: these are the native agent's own statements, never Effects.
    attribution: &'static str,
    items: Vec<storage_core::conversation::ConversationActivityRecord>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct LiveConversationPermissionSnapshot {
    workspace_id: String,
    conversation_id: String,
    turn_id: String,
    retry_ordinal: u32,
    sequence: u32,
    items: Vec<crate::agents::supervisor::NativePermissionPromptView>,
}

#[derive(Serialize)]
struct WorkspaceLiveConversationPermissionSnapshot {
    workspace_id: String,
    items: Vec<WorkspaceLiveConversationPermissionItem>,
}

#[derive(Serialize)]
struct WorkspaceLiveConversationPermissionItem {
    conversation_id: String,
    conversation_title: Option<String>,
    turn_id: String,
    retry_ordinal: u32,
    sequence: u32,
    prompt: crate::agents::supervisor::NativePermissionPromptView,
}

/// Read only the exact attempt's ephemeral permission prompts. Historical REPORTED
/// activity is not consulted to reconstruct live prompts.
async fn list_live_conversation_turn_permissions(
    State(state): State<ConversationApiState>,
    Path((conversation_id, turn_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&turn_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Turn ID is invalid",
        ));
    }
    load_authorized_conversation(&state, &workspace_id, &conversation_id).await?;
    let store = state.store.clone();
    let turn_workspace = workspace_id.clone();
    let turn_conversation = conversation_id.clone();
    let turn_lookup = turn_id.clone();
    let turn = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(store).get_conversation_turn(
            &turn_workspace,
            &turn_conversation,
            &turn_lookup,
        )
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?
    .ok_or_else(not_found)?;
    let active_session =
        load_live_permission_session(&state, &workspace_id, &conversation_id, &turn).await?;
    let snapshot = match (state.supervisor.as_ref(), active_session.as_ref()) {
        (Some(supervisor), Some(session)) => supervisor.list_live_native_permissions(
            &workspace_id,
            &conversation_id,
            &turn_id,
            turn.retry_ordinal,
            &session.agent_session_id,
            &session.runtime_incarnation_id,
        ),
        _ => crate::agents::supervisor::NativePermissionAttemptSnapshot {
            sequence: 0,
            items: Vec::new(),
        },
    };
    Ok(no_store(
        Json(LiveConversationPermissionSnapshot {
            workspace_id,
            conversation_id,
            turn_id,
            retry_ordinal: turn.retry_ordinal,
            sequence: snapshot.sequence,
            items: snapshot.items,
        })
        .into_response(),
    ))
}

/// Rebuild only the ephemeral, currently unresolved native permission list for one
/// authenticated Workspace. Every registry scope is rechecked against the persisted
/// live turn and AgentSession before it is returned.
async fn list_workspace_live_conversation_permissions(
    State(state): State<ConversationApiState>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner_for(&state.store, &state.principal_id, &workspace_id)?;
    let live = state
        .supervisor
        .as_ref()
        .map(|supervisor| supervisor.list_workspace_live_native_permissions(&workspace_id))
        .unwrap_or_default();
    let mut items = Vec::with_capacity(live.len().min(256));
    for candidate in live.into_iter().take(256) {
        let conversation =
            load_authorized_conversation(&state, &workspace_id, &candidate.conversation_id).await?;
        let store = state.store.clone();
        let turn_workspace = workspace_id.clone();
        let turn_conversation = candidate.conversation_id.clone();
        let turn_lookup = candidate.turn_id.clone();
        let turn = tokio::task::spawn_blocking(move || {
            SqliteConversationStore::new(store).get_conversation_turn(
                &turn_workspace,
                &turn_conversation,
                &turn_lookup,
            )
        })
        .await
        .map_err(|_| internal())?
        .map_err(map_store_error)?;
        let Some(turn) = turn.filter(|turn| {
            turn.status == ConversationTurnStatus::Running
                && turn.retry_ordinal == candidate.retry_ordinal
        }) else {
            continue;
        };
        let Some(session) =
            load_live_permission_session(&state, &workspace_id, &candidate.conversation_id, &turn)
                .await?
        else {
            continue;
        };
        if session.agent_session_id != candidate.agent_session_id
            || session.runtime_incarnation_id != candidate.runtime_incarnation_id
        {
            continue;
        }
        items.push(WorkspaceLiveConversationPermissionItem {
            conversation_id: candidate.conversation_id,
            conversation_title: conversation.title,
            turn_id: candidate.turn_id,
            retry_ordinal: candidate.retry_ordinal,
            sequence: candidate.sequence,
            prompt: candidate.prompt,
        });
    }
    Ok(no_store(
        Json(WorkspaceLiveConversationPermissionSnapshot {
            workspace_id,
            items,
        })
        .into_response(),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativePermissionReplyRequest {
    decision: crate::agents::supervisor::NativePermissionDecision,
}

#[derive(Serialize)]
struct NativePermissionReplyReceipt {
    prompt_token: String,
    state: String,
    decision: Option<String>,
    sequence: u32,
}

async fn reply_live_conversation_turn_permission(
    State(state): State<ConversationApiState>,
    Path((conversation_id, turn_id, prompt_token)): Path<(String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<NativePermissionReplyRequest>,
) -> Result<Response, Response> {
    let request_id = idempotency_key(&headers)?;
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&turn_id)
        || !(32..=128).contains(&prompt_token.len())
        || !prompt_token.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Permission prompt token or turn ID is invalid",
        ));
    }
    load_authorized_conversation(&state, &workspace_id, &conversation_id).await?;
    let store = state.store.clone();
    let turn_workspace = workspace_id.clone();
    let turn_conversation = conversation_id.clone();
    let turn_lookup = turn_id.clone();
    let turn = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(store).get_conversation_turn(
            &turn_workspace,
            &turn_conversation,
            &turn_lookup,
        )
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?
    .ok_or_else(not_found)?;
    let active_session =
        load_live_permission_session(&state, &workspace_id, &conversation_id, &turn)
            .await?
            .ok_or_else(|| {
                error(
                    StatusCode::GONE,
                    "NOT_FOUND",
                    "Permission prompt is no longer active",
                )
            })?;
    let supervisor = state.supervisor.as_ref().ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "UNAVAILABLE",
            "The active Runtime cannot receive permission replies",
        )
    })?;
    let view = supervisor
        .reply_live_native_permission(
            &workspace_id,
            &conversation_id,
            &turn_id,
            turn.retry_ordinal,
            &state.principal_id,
            &active_session.agent_session_id,
            &active_session.runtime_incarnation_id,
            &prompt_token,
            &request_id,
            body.decision,
        )
        .await
        .map_err(|failure| match failure {
            crate::agents::supervisor::NativePermissionReplyError::SessionEnded => error(
                StatusCode::GONE,
                "NOT_FOUND",
                "Permission prompt is no longer active",
            ),
            crate::agents::supervisor::NativePermissionReplyError::Conflict => error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Permission prompt already has a different or unresolved decision",
            ),
            crate::agents::supervisor::NativePermissionReplyError::Unavailable => error(
                StatusCode::SERVICE_UNAVAILABLE,
                "UNAVAILABLE",
                "The active Runtime could not accept the permission reply",
            ),
        })?;
    let receipt = NativePermissionReplyReceipt {
        prompt_token: view.prompt_token().to_owned(),
        state: view.state_name().to_owned(),
        decision: view.decision().map(str::to_owned),
        sequence: view.sequence(),
    };
    Ok(no_store(Json(receipt).into_response()))
}

async fn load_live_permission_session(
    state: &ConversationApiState,
    workspace_id: &str,
    conversation_id: &str,
    turn: &ConversationTurn,
) -> Result<Option<AgentSessionRecord>, Response> {
    if turn.status != ConversationTurnStatus::Running {
        return Ok(None);
    }
    let Some(agent_session_id) = turn.agent_session_id.as_deref() else {
        return Ok(None);
    };
    let store = state.store.clone();
    let workspace = workspace_id.to_owned();
    let session_id = agent_session_id.to_owned();
    let session = tokio::task::spawn_blocking(move || {
        AgentSessionStore::get_agent_session(&store, &workspace, &session_id)
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?;
    Ok(session.filter(|session| {
        session.workspace_id == workspace_id
            && session.scope_kind == "CONVERSATION"
            && session.conversation_id.as_deref() == Some(conversation_id)
            && session.conversation_turn_id.as_deref() == Some(turn.turn_id.as_str())
            && session.runtime_id == state.runtime_id
            && session.status == "ACTIVE"
    }))
}

/// Read-only, bounded, ordered REPORTED activity observations for one turn attempt
/// (001-T25, AC-029). The records let the UI rebuild activity cards after a restart.
async fn list_conversation_turn_activity(
    State(state): State<ConversationApiState>,
    Path((conversation_id, turn_id)): Path<(String, String)>,
    headers: HeaderMap,
    Query(query): Query<TurnActivityQuery>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&turn_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Turn ID is invalid",
        ));
    }
    load_authorized_conversation(&state, &workspace_id, &conversation_id).await?;
    let retry_ordinal = query.retry_ordinal.unwrap_or(0);
    let store = state.store.clone();
    let turn_for_read = turn_id.clone();
    let items = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(store).list_conversation_turn_activity(
            &workspace_id,
            &conversation_id,
            &turn_for_read,
            retry_ordinal,
            storage_core::conversation::CONVERSATION_ACTIVITY_MAX_PER_ATTEMPT,
        )
    })
    .await
    .map_err(|_| internal())?
    .map_err(map_store_error)?;
    Ok(no_store(
        Json(TurnActivityPage {
            turn_id,
            retry_ordinal,
            attribution: "REPORTED",
            items,
        })
        .into_response(),
    ))
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
    check_turn_resources(&state, &workspace_id, &body).await?;
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
    let correlation_id = idempotent_id("cor", &principal_id, &request_id, "conversation-turn");
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
    if let Some(ref receipt) = replayed
        && state.supervisor.is_some()
        && receipt.turn.status != ConversationTurnStatus::Failed
    {
        let correlation_id = receipt
            .events
            .iter()
            .find(|e| e.event_type == "conversation.turn.created.v2")
            .map(|e| e.correlation_id.clone())
            .unwrap_or_else(|| correlation_id.clone());
        let correlation = header::HeaderValue::from_str(&correlation_id).map_err(|_| internal())?;
        let turn_receipt = ConversationTurnReceipt {
            turn_id: receipt.turn.turn_id.clone(),
            conversation_id: receipt.turn.conversation_id.clone(),
            user_message_id: receipt.turn.user_message_id.clone(),
            status: receipt.turn.status,
            accepted_at: receipt.turn.created_at.clone(),
            correlation_id,
            presentation_preference: receipt.turn.presentation_preference,
        };
        let mut response = (StatusCode::ACCEPTED, Json(turn_receipt)).into_response();
        response.headers_mut().insert(
            header::HeaderName::from_static("x-correlation-id"),
            correlation,
        );
        return Ok(response);
    }
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
            recorded_at: created.recorded_at.clone(),
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

    if let Some(ref supervisor) = state.supervisor {
        let turn = committed.turn.clone();
        let tid = turn.turn_id.clone();
        let cor = correlation_id.clone();
        crate::agents::turn_coordinator::AgentTurnCoordinator::new(supervisor.clone())
            .dispatch_admitted_turn(
                &state.local_incarnation_id,
                &principal_id,
                &workspace_id,
                &conversation_id,
                turn,
                &cor,
            );

        let receipt = ConversationTurnReceipt {
            turn_id: tid,
            conversation_id,
            user_message_id: message_id,
            status: ConversationTurnStatus::Open,
            accepted_at: created.recorded_at,
            correlation_id: cor,
            presentation_preference,
        };

        let correlation =
            header::HeaderValue::from_str(&receipt.correlation_id).map_err(|_| internal())?;
        let mut response = (StatusCode::ACCEPTED, Json(receipt)).into_response();
        response.headers_mut().insert(
            header::HeaderName::from_static("x-correlation-id"),
            correlation,
        );
        return Ok(response);
    }

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

#[allow(
    clippy::too_many_arguments,
    reason = "this spawn_blocking shim mirrors the store call it forwards; a parameter struct would only rename the arguments at all six call sites"
)]
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
    let request_id = idempotency_key(&headers)?;
    let expected_version = super::parse_if_match(&headers)?;
    if !valid_task_query_id(&turn_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation turn ID is invalid",
        ));
    }
    ensure_conversation_available(&state, &headers, &conversation_id).await?;
    let workspace_id = selected_workspace(&headers)?;
    let correlation_id = idempotent_id("cor", &state.principal_id, &request_id, "retry-turn");

    let conv_store = SqliteConversationStore::new(state.store.clone());
    let turn = conv_store
        .get_conversation_turn(&workspace_id, &conversation_id, &turn_id)
        .map_err(map_store_error)?
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Conversation turn not found",
            )
        })?;

    // An exact repeat of an already applied Retry key replays its receipt (no second
    // attempt, no second intent) even though the turn version has moved on.
    if let Some(replayed) = conv_store
        .find_conversation_turn_command_replay(
            &workspace_id,
            &conversation_id,
            &turn_id,
            &request_id,
        )
        .map_err(map_store_error)?
    {
        return Ok(turn_command_receipt(replayed, correlation_id));
    }

    if turn.version != expected_version {
        return Err(error(
            StatusCode::PRECONDITION_FAILED,
            "STALE_VERSION",
            "Conversation turn version does not match If-Match precondition",
        ));
    }

    if turn.status != ConversationTurnStatus::Failed {
        return Err(error(
            StatusCode::CONFLICT,
            "INVALID_STATE",
            "Conversation turn can only be retried from FAILED status",
        ));
    }

    let next_version = turn.version + 1;
    let next_retry_ordinal = turn.retry_ordinal + 1;
    let ctx = event_context(&state.runtime_id, &correlation_id).map_err(|_| internal())?;

    let event = EventDraft {
        event_id: ctx.event_id,
        workspace_id: workspace_id.clone(),
        entity_type: "ConversationTurn".to_owned(),
        entity_id: turn_id.clone(),
        origin_runtime_id: state.runtime_id.clone(),
        entity_revision: next_version,
        hlc_timestamp: ctx.hlc_timestamp,
        correlation_id: ctx.correlation_id,
        causation_id: Some(request_id),
        schema_version: 1,
        event_type: "conversation.turn.retried.v1".to_owned(),
        payload: retry_event_payload(
            &turn_id,
            turn.agent_session_id.as_deref(),
            next_retry_ordinal,
            next_version,
        ),
        recorded_at: ctx.recorded_at.clone(),
    };

    let updated_turn = run_turn_transition(
        state.store.clone(),
        state.principal_id.clone(),
        workspace_id.clone(),
        conversation_id.clone(),
        turn_id.clone(),
        turn.version,
        TurnCommand::Retry,
        event,
    )
    .await
    .map_err(map_store_error)?;

    if let Some(ref supervisor) = state.supervisor {
        crate::agents::turn_coordinator::AgentTurnCoordinator::new(supervisor.clone())
            .dispatch_admitted_turn(
                &state.local_incarnation_id,
                &state.principal_id,
                &workspace_id,
                &conversation_id,
                updated_turn.clone(),
                &correlation_id,
            );
    }

    let receipt = ConversationTurnReceipt {
        turn_id: updated_turn.turn_id,
        conversation_id: updated_turn.conversation_id,
        user_message_id: updated_turn.user_message_id,
        status: updated_turn.status,
        accepted_at: ctx.recorded_at,
        correlation_id,
        presentation_preference: updated_turn.presentation_preference,
    };

    Ok((StatusCode::ACCEPTED, Json(receipt)).into_response())
}

async fn cancel_conversation_turn(
    State(state): State<ConversationApiState>,
    Path((conversation_id, turn_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let request_id = idempotency_key(&headers)?;
    let expected_version = super::parse_if_match(&headers)?;
    if !valid_task_query_id(&turn_id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Conversation turn ID is invalid",
        ));
    }
    ensure_conversation_available(&state, &headers, &conversation_id).await?;
    let workspace_id = selected_workspace(&headers)?;
    let correlation_id = idempotent_id("cor", &state.principal_id, &request_id, "cancel-turn");

    let conv_store = SqliteConversationStore::new(state.store.clone());
    let turn = conv_store
        .get_conversation_turn(&workspace_id, &conversation_id, &turn_id)
        .map_err(map_store_error)?
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Conversation turn not found",
            )
        })?;

    if let Some(replayed) = conv_store
        .find_conversation_turn_command_replay(
            &workspace_id,
            &conversation_id,
            &turn_id,
            &request_id,
        )
        .map_err(map_store_error)?
    {
        return Ok(turn_command_receipt(replayed, correlation_id));
    }

    if turn.status == ConversationTurnStatus::CancelRequested {
        // A Stop is already in flight. Repeating it never settles the turn: only the
        // supervisor's observed quiescence commits CANCELLED. Re-signal and report.
        if let Some(ref supervisor) = state.supervisor {
            supervisor.request_cancel(&turn_id, turn.retry_ordinal);
        }
        return Ok(turn_command_receipt(turn, correlation_id));
    }

    if turn.version != expected_version {
        return Err(error(
            StatusCode::PRECONDITION_FAILED,
            "STALE_VERSION",
            "Conversation turn version does not match If-Match precondition",
        ));
    }

    if matches!(
        turn.status,
        ConversationTurnStatus::Completed
            | ConversationTurnStatus::Failed
            | ConversationTurnStatus::Cancelled
    ) {
        return Err(error(
            StatusCode::CONFLICT,
            "INVALID_STATE",
            "Conversation turn is already settled",
        ));
    }

    let ctx = event_context(&state.runtime_id, &correlation_id).map_err(|_| internal())?;

    let updated_turn = match turn.status {
        ConversationTurnStatus::Running => {
            let req_event = EventDraft {
                event_id: ctx.event_id.clone(),
                workspace_id: workspace_id.clone(),
                entity_type: "ConversationTurn".to_owned(),
                entity_id: turn_id.clone(),
                origin_runtime_id: state.runtime_id.clone(),
                entity_revision: turn.version + 1,
                hlc_timestamp: ctx.hlc_timestamp.clone(),
                correlation_id: ctx.correlation_id.clone(),
                causation_id: Some(request_id.clone()),
                schema_version: 1,
                event_type: "conversation.turn.status.changed.v1".to_owned(),
                payload: json!({
                    "turn_id": turn_id,
                    "from": "RUNNING",
                    "to": "CANCEL_REQUESTED",
                    "reason_code": "USER_REQUESTED",
                    "agent_session_id": turn.agent_session_id,
                    "aggregate_version": turn.version + 1,
                }),
                recorded_at: ctx.recorded_at.clone(),
            };

            // Only the request is committed here. CANCELLED is committed by the
            // supervisor after it observes the native session quiescent (AC-010).
            let requested = run_turn_transition(
                state.store.clone(),
                state.principal_id.clone(),
                workspace_id.clone(),
                conversation_id.clone(),
                turn_id.clone(),
                turn.version,
                TurnCommand::RequestCancel,
                req_event,
            )
            .await
            .map_err(map_store_error)?;
            if let Some(ref supervisor) = state.supervisor {
                supervisor.request_cancel(&turn_id, requested.retry_ordinal);
            }
            requested
        }
        ConversationTurnStatus::Open => {
            let fail_event = EventDraft {
                event_id: ctx.event_id,
                workspace_id: workspace_id.clone(),
                entity_type: "ConversationTurn".to_owned(),
                entity_id: turn_id.clone(),
                origin_runtime_id: state.runtime_id.clone(),
                entity_revision: turn.version + 1,
                hlc_timestamp: ctx.hlc_timestamp,
                correlation_id: ctx.correlation_id,
                causation_id: Some(request_id),
                schema_version: 1,
                event_type: "conversation.turn.settled.v1".to_owned(),
                payload: json!({
                    "turn_id": turn_id,
                    "from": "OPEN",
                    "to": "FAILED",
                    "reason_code": "USER_CANCELLED",
                    "aggregate_version": turn.version + 1,
                }),
                recorded_at: ctx.recorded_at.clone(),
            };

            run_turn_transition(
                state.store.clone(),
                state.principal_id.clone(),
                workspace_id.clone(),
                conversation_id.clone(),
                turn_id.clone(),
                turn.version,
                TurnCommand::Fail {
                    settled_at: ctx.recorded_at.clone(),
                },
                fail_event,
            )
            .await
            .map_err(map_store_error)?
        }
        _ => return Err(turn_command_unavailable()),
    };

    let receipt = ConversationTurnReceipt {
        turn_id: updated_turn.turn_id,
        conversation_id: updated_turn.conversation_id,
        user_message_id: updated_turn.user_message_id,
        status: updated_turn.status,
        accepted_at: ctx.recorded_at,
        correlation_id,
        presentation_preference: updated_turn.presentation_preference,
    };

    Ok((StatusCode::ACCEPTED, Json(receipt)).into_response())
}

fn retry_event_payload(
    turn_id: &str,
    prior_agent_session_id: Option<&str>,
    retry_ordinal: u32,
    aggregate_version: u64,
) -> Value {
    let mut payload = json!({
        "turn_id": turn_id,
        "retry_ordinal": retry_ordinal,
        "aggregate_version": aggregate_version,
    });
    if let Some(prior) = prior_agent_session_id {
        payload["prior_agent_session_id"] = json!(prior);
    }
    payload
}

/// 202 receipt for a Retry or Cancel command that was already applied or is in flight.
fn turn_command_receipt(turn: ConversationTurn, correlation_id: String) -> Response {
    let receipt = ConversationTurnReceipt {
        accepted_at: turn
            .settled_at
            .clone()
            .unwrap_or_else(|| turn.created_at.clone()),
        turn_id: turn.turn_id,
        conversation_id: turn.conversation_id,
        user_message_id: turn.user_message_id,
        status: turn.status,
        correlation_id,
        presentation_preference: turn.presentation_preference,
    };
    (StatusCode::ACCEPTED, Json(receipt)).into_response()
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

/// Refuses a bad attachment while the draft and chips are still on screen (001-T14, B-08).
/// The same rules run again at dispatch; this only moves the refusal earlier.
async fn check_turn_resources(
    state: &ConversationApiState,
    workspace_id: &str,
    body: &SubmitConversationTurnBody,
) -> Result<(), Response> {
    use crate::agents::resource_materializer::{MaterializeError, check_resource_refs};
    let mut refs: Vec<Value> = Vec::new();
    for resource_ref in &body.resource_refs {
        refs.push(serde_json::to_value(resource_ref).map_err(|_| invalid_turn_request())?);
    }
    for block in &body.content {
        if let ConversationTurnContentBlock::Resource { resource_ref, .. } = block {
            refs.push(serde_json::to_value(resource_ref).map_err(|_| invalid_turn_request())?);
        }
    }
    if refs.is_empty() {
        return Ok(());
    }
    let store = state.store.clone();
    let workspace_id = workspace_id.to_owned();
    let outcome =
        tokio::task::spawn_blocking(move || check_resource_refs(&store, &workspace_id, &refs))
            .await
            .map_err(|_| internal())?;
    outcome.map_err(|refusal| match refusal {
        MaterializeError::NotFound => error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "An attached Resource was not found in this Workspace",
        ),
        MaterializeError::StaleRevision => error(
            StatusCode::CONFLICT,
            "RESOURCE_STALE",
            "An attached Resource changed; reattach its current revision",
        ),
        MaterializeError::StoreUnavailable => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "RESOURCE_UNAVAILABLE",
            "Attached Resources could not be read",
        ),
        MaterializeError::BinaryUnavailable => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_RESOURCE_REF",
            "Only text Resources can be attached to a Conversation turn",
        ),
        MaterializeError::TooLarge => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_RESOURCE_REF",
            "An attached Resource is too large",
        ),
        MaterializeError::NotText => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_RESOURCE_REF",
            "An attached Resource is not valid text",
        ),
        MaterializeError::Restricted => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_RESOURCE_REF",
            "A restricted Resource cannot be attached to a Conversation turn",
        ),
        MaterializeError::TooMany => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_RESOURCE_REF",
            "Too many Resources are attached",
        ),
        MaterializeError::NotAuthorized => error(
            StatusCode::BAD_REQUEST,
            "INVALID_RESOURCE_REF",
            "An attached Resource reference is not valid for this Workspace",
        ),
    })
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
    // 014 contract row 14: the history list is filtered by status, ACTIVE by default.
    // Archived Conversations can only exist once this feature writes one, so the
    // default changes nothing for rows this build can have created.
    let status = match query.status.as_deref() {
        None | Some("ACTIVE") => ConversationStatus::Active,
        Some("ARCHIVED") => ConversationStatus::Archived,
        Some(_) => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Conversation status filter must be ACTIVE or ARCHIVED",
            ));
        }
    };
    let rows = tokio::task::spawn_blocking(move || {
        SqliteConversationStore::new(state.store.clone()).list_conversations_by_status(
            &state.principal_id,
            &workspace_id,
            status,
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
        StoreError::Invalid(message) if message == CONVERSATION_TURN_IN_PROGRESS => error(
            StatusCode::CONFLICT,
            CONVERSATION_TURN_IN_PROGRESS,
            "Another message in this Conversation is still being answered; wait for it to finish or stop it",
        ),
        StoreError::Invalid(message) if message == "AGENT_UNAVAILABLE" => agent_unavailable(),
        StoreError::Invalid(message) if message == "AGENT_NOT_LEAD_ELIGIBLE" => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "AGENT_NOT_LEAD_ELIGIBLE",
            "Choose an enabled AgentBinding that is allowed to lead work",
        ),
        // 014 contract rows 17-18: once a Conversation can be archived or tombstoned,
        // the pre-existing create, submit, retry and binding-change routes must report
        // the history codes instead of collapsing them into INVALID_ARGUMENT.
        StoreError::Invalid(message) if message == CONVERSATION_ARCHIVED => error(
            StatusCode::CONFLICT,
            CONVERSATION_ARCHIVED,
            "Archived Conversations cannot be changed; unarchive it first",
        ),
        StoreError::Invalid(message) if message == CONVERSATION_DELETED => error(
            StatusCode::GONE,
            CONVERSATION_DELETED,
            "This Conversation was deleted",
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
/// Store-error mapping for the Conversation history mutations (014 rows 16-18). These
/// routes own their own mapper so the accepted 014 codes are returned without changing
/// the codes the pre-existing create, agent-binding and turn routes already report.
fn map_history_store_error(error_value: StoreError) -> Response {
    match error_value {
        StoreError::NotFound => not_found(),
        StoreError::Conflict {
            expected: None,
            actual: None,
        } => error(
            StatusCode::CONFLICT,
            "IDEMPOTENCY_CONFLICT",
            "Idempotency key was already used for another Conversation request",
        ),
        StoreError::Conflict { .. } => error(
            StatusCode::PRECONDITION_FAILED,
            "STALE_VERSION",
            "Conversation changed; refresh before saving",
        ),
        StoreError::Invalid(message) if message == CONVERSATION_ARCHIVED => error(
            StatusCode::CONFLICT,
            CONVERSATION_ARCHIVED,
            "Archived Conversations cannot be changed; unarchive it first",
        ),
        // The storage refuses an archive while any turn of the Conversation is still
        // nonterminal; the wire code for that case is CONVERSATION_TURN_IN_PROGRESS.
        StoreError::Invalid(message) if message == CONVERSATION_NOT_QUIESCENT => error(
            StatusCode::CONFLICT,
            CONVERSATION_TURN_IN_PROGRESS,
            "This Conversation still has a turn in progress; wait for it to finish or stop it",
        ),
        StoreError::Invalid(message) if message == CONVERSATION_DELETED => error(
            StatusCode::GONE,
            CONVERSATION_DELETED,
            "This Conversation was deleted",
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
