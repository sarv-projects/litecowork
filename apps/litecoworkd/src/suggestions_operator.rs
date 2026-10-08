//! Authenticated local Operator projection for Suggestions.
//!
//! Listing first runs bounded, event-backed SYSTEM_EXPIRY settlement using the domain
//! SuggestionService clock. Owners can resolve proposals, and TASK acceptance commits
//! one ordinary READY Task atomically without starting planning or execution.

use super::*;
use domain_responsibility::{CoworkerStatus, SuggestionAction, SuggestionClock, SuggestionOwnerAction, SuggestionService, SuggestionServiceError, SuggestionStatus, SuggestionVisibility};
use storage_sqlite::{CoworkerEventContext, SqliteCoworkerStore, SqliteSuggestionStore, SuggestionEventContext, SuggestionReadError};

struct OperatorSuggestionClock;
impl SuggestionClock for OperatorSuggestionClock {
    fn now(&self) -> Result<String, SuggestionServiceError> {
        time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|_| SuggestionServiceError::ClockUnavailable)
    }
}

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/suggestions", get(list_suggestions))
        .route("/v1/suggestions/{suggestion_id}/resolve", post(resolve_suggestion))
        .route("/v1/suggestions/{suggestion_id}/accept-task", post(accept_suggestion_task))
        .route("/v1/suggestions/{suggestion_id}/snooze", post(snooze_suggestion))
        .route("/v1/workspaces/{workspace_id}/suggestion-preferences", get(list_suggestion_preferences))
        .route("/v1/workspaces/{workspace_id}/suggestion-preferences/{kind}", put(set_suggestion_preference))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SuggestionQuery {
    status: Option<SuggestionStatus>,
    visibility: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuggestionCursor {
    version: u32,
    workspace_id: String,
    status: Option<SuggestionStatus>,
    visibility: String,
    created_at: String,
    suggestion_id: String,
}

#[derive(Serialize)]
struct SuggestionPageResponse {
    items: Vec<domain_responsibility::Suggestion>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveSuggestionBody { resolution: String }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptSuggestionTaskBody {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnoozeSuggestionBody { snoozed_until: serde_json::Value }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetSuggestionPreferenceBody { muted: bool }

fn invalid() -> Response {
    operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Suggestion list request is invalid")
}
fn unavailable() -> Response {
    operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Suggestion storage is unavailable")
}
fn expiry_pending() -> Response {
    operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Suggestion expiry is still being settled; retry after settlement completes")
}
fn read_error(error: SuggestionReadError) -> Response {
    match error {
        SuggestionReadError::Unauthorized => operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"),
        SuggestionReadError::InvalidRequest => invalid(),
        SuggestionReadError::ExpiryPending => expiry_pending(),
        SuggestionReadError::Storage => unavailable(),
    }
}
fn expiry_error(error: SuggestionServiceError) -> Response {
    match error {
        SuggestionServiceError::InvalidRequest => invalid(),
        SuggestionServiceError::Unauthorized => operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"),
        SuggestionServiceError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Suggestion is unavailable"),
        SuggestionServiceError::VersionConflict | SuggestionServiceError::IdempotencyConflict => operator_error(StatusCode::CONFLICT, "CONFLICT", "Suggestion version or idempotency key conflicts with current state"),
        SuggestionServiceError::InvalidTransition => operator_error(StatusCode::CONFLICT, "INVALID_TRANSITION", "This Suggestion can no longer be changed"),
        SuggestionServiceError::Expired => operator_error(StatusCode::CONFLICT, "SUGGESTION_EXPIRED", "This Suggestion expired and was settled without creating work"),
        SuggestionServiceError::ExpiryPending => expiry_pending(),
        SuggestionServiceError::ClockUnavailable | SuggestionServiceError::Storage => unavailable(),
    }
}
fn valid_id(id: &str) -> bool { !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control) }
fn visibility(value: Option<&str>) -> Result<SuggestionVisibility, Response> {
    match value.unwrap_or("VISIBLE") {
        "VISIBLE" => Ok(SuggestionVisibility::Visible),
        "SNOOZED" => Ok(SuggestionVisibility::Snoozed),
        "ALL" => Ok(SuggestionVisibility::All),
        _ => Err(invalid()),
    }
}
fn decode_cursor(encoded: &str, workspace: &str, status: Option<SuggestionStatus>, visibility: &str) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 { return Err(invalid()); }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: SuggestionCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1 || cursor.workspace_id != workspace || cursor.status != status || cursor.visibility != visibility
        || !valid_id(&cursor.suggestion_id) || cursor.created_at.len() > 64
        || time::OffsetDateTime::parse(&cursor.created_at, &time::format_description::well_known::Rfc3339).is_err()
    { return Err(invalid()); }
    Ok((cursor.created_at, cursor.suggestion_id))
}

fn idempotency_key(headers: &HeaderMap) -> Result<String, Response> {
    headers.get("idempotency-key").and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 128 && value.bytes().all(|byte| byte.is_ascii_graphic()))
        .map(str::to_owned).ok_or_else(invalid)
}

fn expected_version(headers: &HeaderMap) -> Result<u64, Response> {
    let text = headers.get(header::IF_MATCH).and_then(|value| value.to_str().ok()).ok_or_else(invalid)?.trim();
    let text = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 { &text[1..text.len()-1] } else { text };
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) { return Err(invalid()); }
    text.parse::<u64>().ok().filter(|version| *version > 0).ok_or_else(invalid)
}

fn expected_preference_version(headers: &HeaderMap) -> Result<u64, Response> {
    let text = headers.get(header::IF_MATCH).and_then(|value| value.to_str().ok()).ok_or_else(invalid)?.trim();
    let text = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 { &text[1..text.len()-1] } else { text };
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) { return Err(invalid()); }
    text.parse::<u64>().map_err(|_| invalid())
}

fn validate_action_workspace(state: &ApiState, headers: &HeaderMap) -> Result<String, Response> {
    let workspace_id = selected_workspace(headers)?;
    ensure_workspace_owner(state, &workspace_id)?;
    let binding = state.store.get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
        owner_principal_id: state.principal_id.clone(), workspace_id: workspace_id.clone(),
        runtime_id: state.runtime_id.clone(), runtime_incarnation_id: state.local_incarnation_id.clone(),
    }).map_err(|_| unavailable())?;
    if !binding.is_some_and(|binding| binding.status == "ACTIVE") {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Local Runtime Workspace access is unavailable"));
    }
    Ok(workspace_id)
}

fn action_service(state: &ApiState, request_id: &str) -> Result<SuggestionService<SqliteSuggestionStore, OperatorSuggestionClock>, Response> {
    let correlation_id = idempotent_id("cor", &state.principal_id, request_id, "suggestion-owner-action");
    let event = event_context(&state.runtime_id, &correlation_id).map_err(|_| unavailable())?;
    let store = SqliteSuggestionStore::with_event_context(
        state.store.clone(),
        SuggestionEventContext { origin_runtime_id: event.origin_runtime_id, correlation_id: event.correlation_id },
    ).map_err(expiry_error)?;
    Ok(SuggestionService::new(store, OperatorSuggestionClock))
}

fn suggestion_response(suggestion: domain_responsibility::Suggestion) -> Response {
    let version = suggestion.version;
    let mut response = Json(suggestion).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    if let Ok(value) = header::HeaderValue::from_str(&format!("\"{version}\"")) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}

fn preference_response(preference: domain_responsibility::SuggestionPreference) -> Response {
    let version = preference.version;
    let mut response = Json(preference).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    if let Ok(value) = header::HeaderValue::from_str(&format!("\"{version}\"")) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}

fn parse_suggestion_kind(value: &str) -> Result<domain_responsibility::SuggestionKind, Response> {
    match value {
        "TASK_OPPORTUNITY" => Ok(domain_responsibility::SuggestionKind::TaskOpportunity),
        "ROUTINE_OPPORTUNITY" => Ok(domain_responsibility::SuggestionKind::RoutineOpportunity),
        "AUTOMATION_OPPORTUNITY" => Ok(domain_responsibility::SuggestionKind::AutomationOpportunity),
        _ => Err(invalid()),
    }
}

async fn list_suggestion_preferences(
    State(state): State<ApiState>, headers: HeaderMap, Path(workspace_id): Path<String>,
) -> Result<Response, Response> {
    if !valid_id(&workspace_id) { return Err(invalid()); }
    let selected = selected_workspace(&headers)?;
    if selected != workspace_id { return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable")); }
    validate_action_workspace(&state, &headers)?;
    let store = SqliteSuggestionStore::new(state.store.clone());
    let service = SuggestionService::new(store, OperatorSuggestionClock);
    let items = service.kind_preferences(&state.principal_id, &workspace_id).map_err(expiry_error)?;
    let mut response = Json(serde_json::json!({"items": items})).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn set_suggestion_preference(
    State(state): State<ApiState>, headers: HeaderMap, Path((workspace_id, kind)): Path<(String, String)>, body: Bytes,
) -> Result<Response, Response> {
    if !valid_id(&workspace_id) { return Err(invalid()); }
    let selected = selected_workspace(&headers)?;
    if selected != workspace_id { return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable")); }
    validate_action_workspace(&state, &headers)?;
    let kind = parse_suggestion_kind(&kind)?;
    if body.len() > 4096 { return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Suggestion preference request exceeds the local body limit")); }
    let body: SetSuggestionPreferenceBody = serde_json::from_slice(&body).map_err(|_| invalid())?;
    let request_id = idempotency_key(&headers)?;
    let version = expected_preference_version(&headers)?;
    let mut service = action_service(&state, &request_id)?;
    let preference = service.set_kind_preference(&state.principal_id, &workspace_id, kind, body.muted, version, &request_id).map_err(expiry_error)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    Ok(preference_response(preference))
}

async fn resolve_suggestion(
    State(state): State<ApiState>, headers: HeaderMap, Path(suggestion_id): Path<String>, body: Bytes,
) -> Result<Response, Response> {
    let workspace = validate_action_workspace(&state, &headers)?;
    if !valid_id(&suggestion_id) { return Err(invalid()); }
    if body.len() > 4096 { return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Suggestion request exceeds the local body limit")); }
    let body: ResolveSuggestionBody = serde_json::from_slice(&body).map_err(|_| invalid())?;
    if body.resolution != "DISMISSED" {
        return Err(operator_error(StatusCode::CONFLICT, "INVALID_TRANSITION", "Only dismissal is available in this Suggestion slice"));
    }
    let request_id = idempotency_key(&headers)?;
    let version = expected_version(&headers)?;
    let mut service = action_service(&state, &request_id)?;
    let suggestion = service.owner_action(
        &state.principal_id, &workspace, &suggestion_id, version, &request_id, SuggestionOwnerAction::Dismiss,
    ).map_err(expiry_error)?;
    ensure_workspace_owner(&state, &workspace)?;
    Ok(suggestion_response(suggestion))
}

/// Turns an actionable proposal into the exact ordinary TaskSpec it proposed. The
/// storage operation commits Task creation and Suggestion resolution atomically. It
/// deliberately returns a READY Task and never dispatches a planner or agent.
async fn accept_suggestion_task(
    State(state): State<ApiState>, headers: HeaderMap, Path(suggestion_id): Path<String>, body: Bytes,
) -> Result<Response, Response> {
    let workspace_id = validate_action_workspace(&state, &headers)?;
    if !valid_id(&suggestion_id) { return Err(invalid()); }
    if body.len() > 1024 { return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Suggestion request exceeds the local body limit")); }
    let _: AcceptSuggestionTaskBody = serde_json::from_slice(&body).map_err(|_| invalid())?;
    let request_id = idempotency_key(&headers)?;
    let expected_suggestion_version = expected_version(&headers)?;

    let mut expiry_service = action_service(&state, &request_id)?;
    let expiry = expiry_service.settle_expired(&state.principal_id, &workspace_id).map_err(expiry_error)?;
    if expiry.more_due { return Err(expiry_pending()); }

    let suggestion = SqliteSuggestionStore::new(state.store.clone())
        .get(&state.principal_id, &workspace_id, &suggestion_id)
        .map_err(read_error)?
        .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Suggestion is unavailable"))?;
    let accepted_task_id = if suggestion.status == SuggestionStatus::Accepted
        && expected_suggestion_version.checked_add(1).is_some_and(|next| suggestion.version == next)
    { suggestion.result_task_id.as_deref() } else { None };
    if let Some(task_id) = accepted_task_id {
        // A committed acceptance may have lost its response. Return the already linked
        // ordinary Task on retry, even if the Coworker head has since advanced.
        let replay = state.store.get_task(&workspace_id, task_id)
            .map_err(|_| unavailable())?
            .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "The accepted Task is unavailable"))?;
        ensure_workspace_owner(&state, &workspace_id)?;
        let task_version = replay.task.version;
        let mut response = (StatusCode::OK, Json(replay)).into_response();
        response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
        if let Ok(value) = header::HeaderValue::from_str(&format!("\"{task_version}\"")) {
            response.headers_mut().insert(header::ETAG, value);
        }
        return Ok(response);
    }
    if suggestion.status != SuggestionStatus::Proposed || suggestion.version != expected_suggestion_version {
        if suggestion.status == SuggestionStatus::Expired {
            return Err(operator_error(StatusCode::CONFLICT, "SUGGESTION_EXPIRED", "This Suggestion expired and was settled without creating work"));
        }
        return Err(operator_error(StatusCode::CONFLICT, "CONFLICT", "Suggestion changed; refresh before accepting it"));
    }
    if suggestion.proposed_action != SuggestionAction::Task || suggestion.proposed_task_spec.is_none() {
        return Err(operator_error(StatusCode::CONFLICT, "INVALID_TRANSITION", "This Suggestion does not contain an actionable Task proposal"));
    }

    let workspace = ensure_workspace_owner(&state, &workspace_id)?;
    if workspace.status != "ACTIVE" {
        return Err(operator_error(StatusCode::UNPROCESSABLE_ENTITY, "WORKSPACE_ARCHIVED", "Archived Workspaces cannot accept new Tasks"));
    }
    let proposal = suggestion.proposed_task_spec.as_ref().and_then(serde_json::Value::as_object)
        .ok_or_else(invalid)?;

    let coworker_origin = if let Some(coworker_id) = suggestion.coworker_id.as_deref() {
        let read_event = event_context(&state.runtime_id, &new_id("cor").map_err(|_| unavailable())?)
            .map_err(|_| unavailable())?;
        let coworker_store = SqliteCoworkerStore::new(state.store.clone(), CoworkerEventContext {
            event_id: read_event.event_id,
            origin_runtime_id: read_event.origin_runtime_id,
            hlc_timestamp: read_event.hlc_timestamp,
            correlation_id: read_event.correlation_id,
            causation_id: read_event.causation_id,
            recorded_at: read_event.recorded_at,
        }).map_err(|_| unavailable())?;
        let (head, revision) = coworker_store.get(&state.principal_id, &workspace_id, coworker_id)
            .map_err(|_| unavailable())?
            .ok_or_else(|| operator_error(StatusCode::CONFLICT, "CONFLICT", "The Suggestion's Coworker is no longer available"))?;
        if head.status == CoworkerStatus::Archived {
            return Err(operator_error(StatusCode::CONFLICT, "COWORKER_ARCHIVED", "The Suggestion's Coworker is archived"));
        }
        (Some(head.coworker_id), Some(head.version), Some(revision.revision), revision.definition.default_lead_agent_binding_id,
            revision.definition.lead_failover_policy)
    } else {
        (None, None, None, None, None)
    };

    let lead_binding_id = coworker_origin.3.clone()
        .or(workspace.default_agent_binding_id.clone())
        .ok_or_else(|| operator_error(StatusCode::UNPROCESSABLE_ENTITY, "AGENT_UNAVAILABLE", "Choose and enable a lead agent before accepting this Task"))?;
    let binding = state.store.get_agent_binding(&state.principal_id, &workspace_id, &lead_binding_id)
        .map_err(|error| map_agent_store_error(error, "Task lead AgentBinding is unavailable"))?
        .ok_or_else(|| operator_error(StatusCode::UNPROCESSABLE_ENTITY, "AGENT_UNAVAILABLE", "Task lead AgentBinding is unavailable"))?;
    if !binding.enabled || !binding.lead_eligible {
        return Err(operator_error(StatusCode::UNPROCESSABLE_ENTITY, "AGENT_UNAVAILABLE", "The selected Coworker lead agent is not enabled for lead work"));
    }

    let correlation_id = idempotent_id("cor", &state.principal_id, &request_id, "suggestion-accept-task");
    let task_event = event_context(&state.runtime_id, &correlation_id)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task request could not be initialized"))?;
    let mut suggestion_event = task_event.clone();
    suggestion_event.event_id = new_id("ev")
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task request could not be initialized"))?;
    let task_id = idempotent_id("tsk", &state.principal_id, &request_id, &format!("suggestion:{suggestion_id}"));

    let request_payload = json!({
        "workspace_id": workspace_id,
        "objective": proposal.get("objective").ok_or_else(invalid)?,
        "task_category": proposal.get("task_category").cloned().unwrap_or(serde_json::Value::Null),
        "constraints": proposal.get("constraints").cloned().unwrap_or_else(|| json!([])),
        "non_goals": [],
        "input_refs": proposal.get("input_refs").cloned().unwrap_or_else(|| json!([])),
        "required_outputs": proposal.get("required_outputs").cloned().unwrap_or_else(|| json!([])),
        "acceptance_criteria": proposal.get("acceptance_criteria").cloned().unwrap_or_else(|| json!([])),
        "approvals_required": [],
        "budget": proposal.get("budget").cloned().unwrap_or(serde_json::Value::Null),
        "delegation_budget_policy": proposal.get("delegation_budget_policy").cloned().unwrap_or(serde_json::Value::Null),
        "deadline": proposal.get("deadline").cloned().unwrap_or(serde_json::Value::Null),
        "placement_preference": "AUTO",
        "preferred_lead_agent_binding_id": lead_binding_id,
        "lead_failover_policy": coworker_origin.4.clone().unwrap_or_else(|| json!({
            "mode": "DISABLED", "triggers": [], "fallback_agent_binding_ids": [], "max_lead_changes": 0
        })),
        "coworker_id": coworker_origin.0,
        "expected_coworker_version": coworker_origin.1,
    });
    let command = CreateStandaloneTask {
        task_id,
        workspace_id: workspace_id.clone(),
        workspace_instruction_revision: workspace.current_instruction_revision,
        lead_agent_binding_id,
        origin_coworker_id: coworker_origin.0,
        origin_coworker_revision: coworker_origin.2,
        expected_coworker_version: coworker_origin.1,
        coworker_default_lead_failover_policy: None,
        principal_id: state.principal_id.clone(),
        request_id: request_id.clone(),
        request_payload,
        event: task_event,
    };
    let store = state.store.clone();
    let principal = state.principal_id.clone();
    let accepted_at = suggestion_event.recorded_at.clone();
    let committed = tokio::task::spawn_blocking(move || {
        TaskService::new(store).create_from_suggestion(
            command,
            suggestion_id,
            expected_suggestion_version,
            accepted_at,
            suggestion_event,
        )
    }).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Suggestion acceptance did not complete"))?
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "The Task proposal is invalid or its lead is unavailable"),
            storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "CONFLICT", "Suggestion, Coworker, or Task inputs changed; refresh before accepting"),
            storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Suggestion or a pinned Task input is unavailable"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Suggestion and Task could not be committed"),
        })?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let task_version = committed.view.task.version;
    let correlation_id = committed.event.correlation_id;
    let mut response = (StatusCode::CREATED, Json(committed.view)).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    if let Ok(value) = header::HeaderValue::from_str(&format!("\"{task_version}\"")) {
        response.headers_mut().insert(header::ETAG, value);
    }
    if let Ok(value) = header::HeaderValue::from_str(&correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn snooze_suggestion(
    State(state): State<ApiState>, headers: HeaderMap, Path(suggestion_id): Path<String>, body: Bytes,
) -> Result<Response, Response> {
    let workspace = validate_action_workspace(&state, &headers)?;
    if !valid_id(&suggestion_id) { return Err(invalid()); }
    if body.len() > 4096 { return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Suggestion request exceeds the local body limit")); }
    let body: SnoozeSuggestionBody = serde_json::from_slice(&body).map_err(|_| invalid())?;
    let snoozed_until = match body.snoozed_until {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) => Some(value),
        _ => return Err(invalid()),
    };
    if snoozed_until.as_deref().is_some_and(|value| time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).is_err()) {
        return Err(invalid());
    }
    let request_id = idempotency_key(&headers)?;
    let version = expected_version(&headers)?;
    let mut service = action_service(&state, &request_id)?;
    let suggestion = service.owner_action(
        &state.principal_id, &workspace, &suggestion_id, version, &request_id,
        SuggestionOwnerAction::Snooze { snoozed_until },
    ).map_err(expiry_error)?;
    ensure_workspace_owner(&state, &workspace)?;
    Ok(suggestion_response(suggestion))
}

async fn list_suggestions(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<SuggestionQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let binding = state.store.get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
        owner_principal_id: state.principal_id.clone(), workspace_id: workspace_id.clone(),
        runtime_id: state.runtime_id.clone(), runtime_incarnation_id: state.local_incarnation_id.clone(),
    }).map_err(|_| unavailable())?;
    if !binding.is_some_and(|binding| binding.status == "ACTIVE") {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Local Runtime Workspace access is unavailable"));
    }
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) { return Err(invalid()); }
    let status = query.status.or(Some(SuggestionStatus::Proposed));
    let visibility_value = visibility(query.visibility.as_deref())?;
    let visibility_name = query.visibility.as_deref().unwrap_or("VISIBLE");
    let after = query.cursor.as_deref().map(|value| decode_cursor(value, &workspace_id, status, visibility_name)).transpose()?;
    let correlation_id = new_id("cor").map_err(|_| unavailable())?;
    let event = event_context(&state.runtime_id, &correlation_id).map_err(|_| unavailable())?;
    let expiry_store = SqliteSuggestionStore::with_event_context(
        state.store.clone(),
        SuggestionEventContext { origin_runtime_id: event.origin_runtime_id, correlation_id: event.correlation_id },
    ).map_err(expiry_error)?;
    let mut service = SuggestionService::new(expiry_store, OperatorSuggestionClock);
    let expiry = service.settle_expired(&state.principal_id, &workspace_id).map_err(expiry_error)?;
    if expiry.more_due { return Err(expiry_pending()); }
    let page = SqliteSuggestionStore::new(state.store.clone())
        .list(&state.principal_id, &workspace_id, status, visibility_value, &expiry.as_of, after, limit)
        .map_err(read_error)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let next_cursor = page.next.map(|(created_at, suggestion_id)| serde_json::to_vec(&SuggestionCursor {
        version: 1, workspace_id, status, visibility: visibility_name.to_owned(), created_at, suggestion_id,
    }).map(|bytes| URL_SAFE_NO_PAD.encode(bytes)).map_err(|_| unavailable())).transpose()?;
    let mut response = Json(SuggestionPageResponse { items: page.items, next_cursor }).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}
