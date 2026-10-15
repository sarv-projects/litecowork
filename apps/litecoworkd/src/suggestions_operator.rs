//! Authenticated local Operator projection for Suggestions.
//!
//! Listing first runs bounded, event-backed SYSTEM_EXPIRY settlement using the domain
//! SuggestionService clock. Owners can resolve proposals, and TASK acceptance commits
//! one ordinary READY Task atomically without starting planning or execution.

#![allow(clippy::result_large_err)] // Handlers return full Axum Responses to preserve status, body, and headers.

use super::*;
use domain_responsibility::{
    CoworkerStatus, SuggestionAction, SuggestionClock, SuggestionOwnerAction, SuggestionService,
    SuggestionServiceError, SuggestionStatus, SuggestionVisibility,
};
use storage_sqlite::{
    CoworkerEventContext, SqliteCoworkerStore, SqliteSuggestionStore, SuggestionEventContext,
    SuggestionReadError,
};

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
        .route(
            "/v1/suggestions/{suggestion_id}/resolve",
            post(resolve_suggestion),
        )
        .route(
            "/v1/suggestions/{suggestion_id}/accept-task",
            post(accept_suggestion_task),
        )
        .route(
            "/v1/suggestions/{suggestion_id}/snooze",
            post(snooze_suggestion),
        )
        .route(
            "/v1/workspaces/{workspace_id}/suggestion-preferences",
            get(list_suggestion_preferences),
        )
        .route(
            "/v1/workspaces/{workspace_id}/suggestion-preferences/{kind}",
            put(set_suggestion_preference),
        )
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
struct ResolveSuggestionBody {
    resolution: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptSuggestionTaskBody {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnoozeSuggestionBody {
    snoozed_until: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetSuggestionPreferenceBody {
    muted: bool,
}

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Suggestion list request is invalid",
    )
}
fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Suggestion storage is unavailable",
    )
}
fn expiry_pending() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Suggestion expiry is still being settled; retry after settlement completes",
    )
}
fn read_error(error: SuggestionReadError) -> Response {
    match error {
        SuggestionReadError::Unauthorized => operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ),
        SuggestionReadError::InvalidRequest => invalid(),
        SuggestionReadError::ExpiryPending => expiry_pending(),
        SuggestionReadError::Storage => unavailable(),
    }
}
fn expiry_error(error: SuggestionServiceError) -> Response {
    match error {
        SuggestionServiceError::InvalidRequest => invalid(),
        SuggestionServiceError::Unauthorized => operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ),
        SuggestionServiceError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Suggestion is unavailable",
        ),
        SuggestionServiceError::VersionConflict | SuggestionServiceError::IdempotencyConflict => {
            operator_error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Suggestion version or idempotency key conflicts with current state",
            )
        }
        SuggestionServiceError::InvalidTransition => operator_error(
            StatusCode::CONFLICT,
            "INVALID_TRANSITION",
            "This Suggestion can no longer be changed",
        ),
        SuggestionServiceError::Expired => operator_error(
            StatusCode::CONFLICT,
            "SUGGESTION_EXPIRED",
            "This Suggestion expired and was settled without creating work",
        ),
        SuggestionServiceError::ExpiryPending => expiry_pending(),
        SuggestionServiceError::ClockUnavailable | SuggestionServiceError::Storage => unavailable(),
    }
}
fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
fn visibility(value: Option<&str>) -> Result<SuggestionVisibility, Response> {
    match value.unwrap_or("VISIBLE") {
        "VISIBLE" => Ok(SuggestionVisibility::Visible),
        "SNOOZED" => Ok(SuggestionVisibility::Snoozed),
        "ALL" => Ok(SuggestionVisibility::All),
        _ => Err(invalid()),
    }
}
fn decode_cursor(
    encoded: &str,
    workspace: &str,
    status: Option<SuggestionStatus>,
    visibility: &str,
) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: SuggestionCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace
        || cursor.status != status
        || cursor.visibility != visibility
        || !valid_id(&cursor.suggestion_id)
        || cursor.created_at.len() > 64
        || time::OffsetDateTime::parse(
            &cursor.created_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return Err(invalid());
    }
    Ok((cursor.created_at, cursor.suggestion_id))
}

fn idempotency_key(headers: &HeaderMap) -> Result<String, Response> {
    super::idempotency_key(headers)
}

fn expected_version(headers: &HeaderMap) -> Result<u64, Response> {
    let text = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(invalid)?
        .trim();
    let text = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
        &text[1..text.len() - 1]
    } else {
        text
    };
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    text.parse::<u64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(invalid)
}

fn expected_preference_version(headers: &HeaderMap) -> Result<u64, Response> {
    let text = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(invalid)?
        .trim();
    let text = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
        &text[1..text.len() - 1]
    } else {
        text
    };
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    text.parse::<u64>().map_err(|_| invalid())
}

fn validate_action_workspace(state: &ApiState, headers: &HeaderMap) -> Result<String, Response> {
    let workspace_id = selected_workspace(headers)?;
    ensure_workspace_owner(state, &workspace_id)?;
    let binding = state
        .store
        .get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
            owner_principal_id: state.principal_id.clone(),
            workspace_id: workspace_id.clone(),
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
    Ok(workspace_id)
}

fn action_service(
    state: &ApiState,
    request_id: &str,
) -> Result<SuggestionService<SqliteSuggestionStore, OperatorSuggestionClock>, Response> {
    let correlation_id = idempotent_id(
        "cor",
        &state.principal_id,
        request_id,
        "suggestion-owner-action",
    );
    let event = event_context(&state.runtime_id, &correlation_id).map_err(|_| unavailable())?;
    let store = SqliteSuggestionStore::with_event_context(
        state.store.clone(),
        SuggestionEventContext {
            origin_runtime_id: event.origin_runtime_id,
            correlation_id: event.correlation_id,
        },
    )
    .map_err(expiry_error)?;
    Ok(SuggestionService::new(store, OperatorSuggestionClock))
}

fn suggestion_response(suggestion: domain_responsibility::Suggestion) -> Response {
    let version = suggestion.version;
    let mut response = Json(suggestion).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(value) = header::HeaderValue::from_str(&format!("\"{version}\"")) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}

fn preference_response(preference: domain_responsibility::SuggestionPreference) -> Response {
    let version = preference.version;
    let mut response = Json(preference).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(value) = header::HeaderValue::from_str(&format!("\"{version}\"")) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}

fn parse_suggestion_kind(value: &str) -> Result<domain_responsibility::SuggestionKind, Response> {
    match value {
        "TASK_OPPORTUNITY" => Ok(domain_responsibility::SuggestionKind::TaskOpportunity),
        "ROUTINE_OPPORTUNITY" => Ok(domain_responsibility::SuggestionKind::RoutineOpportunity),
        "AUTOMATION_OPPORTUNITY" => {
            Ok(domain_responsibility::SuggestionKind::AutomationOpportunity)
        }
        _ => Err(invalid()),
    }
}

async fn list_suggestion_preferences(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Response, Response> {
    if !valid_id(&workspace_id) {
        return Err(invalid());
    }
    let selected = selected_workspace(&headers)?;
    if selected != workspace_id {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    validate_action_workspace(&state, &headers)?;
    let store = SqliteSuggestionStore::new(state.store.clone());
    let service = SuggestionService::new(store, OperatorSuggestionClock);
    let items = service
        .kind_preferences(&state.principal_id, &workspace_id)
        .map_err(expiry_error)?;
    let mut response = Json(serde_json::json!({"items": items})).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

async fn set_suggestion_preference(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((workspace_id, kind)): Path<(String, String)>,
    body: Bytes,
) -> Result<Response, Response> {
    if !valid_id(&workspace_id) {
        return Err(invalid());
    }
    let selected = selected_workspace(&headers)?;
    if selected != workspace_id {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    validate_action_workspace(&state, &headers)?;
    let kind = parse_suggestion_kind(&kind)?;
    if body.len() > 4096 {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Suggestion preference request exceeds the local body limit",
        ));
    }
    let body: SetSuggestionPreferenceBody = serde_json::from_slice(&body).map_err(|_| invalid())?;
    let request_id = idempotency_key(&headers)?;
    let version = expected_preference_version(&headers)?;
    let mut service = action_service(&state, &request_id)?;
    let preference = service
        .set_kind_preference(
            &state.principal_id,
            &workspace_id,
            kind,
            body.muted,
            version,
            &request_id,
        )
        .map_err(expiry_error)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    Ok(preference_response(preference))
}

async fn resolve_suggestion(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(suggestion_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = validate_action_workspace(&state, &headers)?;
    if !valid_id(&suggestion_id) {
        return Err(invalid());
    }
    if body.len() > 4096 {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Suggestion request exceeds the local body limit",
        ));
    }
    let body: ResolveSuggestionBody = serde_json::from_slice(&body).map_err(|_| invalid())?;
    if body.resolution != "DISMISSED" {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "INVALID_TRANSITION",
            "Only dismissal is available in this Suggestion slice",
        ));
    }
    let request_id = idempotency_key(&headers)?;
    let version = expected_version(&headers)?;
    let mut service = action_service(&state, &request_id)?;
    let suggestion = service
        .owner_action(
            &state.principal_id,
            &workspace,
            &suggestion_id,
            version,
            &request_id,
            SuggestionOwnerAction::Dismiss,
        )
        .map_err(expiry_error)?;
    ensure_workspace_owner(&state, &workspace)?;
    Ok(suggestion_response(suggestion))
}

/// Turns an actionable proposal into the exact ordinary TaskSpec it proposed. The
/// storage operation commits Task creation and Suggestion resolution atomically. It
/// deliberately returns a READY Task and never dispatches a planner or agent.
async fn accept_suggestion_task(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(suggestion_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace_id = validate_action_workspace(&state, &headers)?;
    if !valid_id(&suggestion_id) {
        return Err(invalid());
    }
    if body.len() > 1024 {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Suggestion request exceeds the local body limit",
        ));
    }
    let _: AcceptSuggestionTaskBody = serde_json::from_slice(&body).map_err(|_| invalid())?;
    let request_id = idempotency_key(&headers)?;
    let expected_suggestion_version = expected_version(&headers)?;
    let expected_task_id = idempotent_id(
        "tsk",
        &state.principal_id,
        &request_id,
        &format!("suggestion:{suggestion_id}"),
    );

    let mut expiry_service = action_service(&state, &request_id)?;
    let expiry = expiry_service
        .settle_expired(&state.principal_id, &workspace_id)
        .map_err(expiry_error)?;
    if expiry.more_due {
        return Err(expiry_pending());
    }

    let suggestion = SqliteSuggestionStore::new(state.store.clone())
        .get(&state.principal_id, &workspace_id, &suggestion_id)
        .map_err(read_error)?
        .ok_or_else(|| {
            operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Suggestion is unavailable",
            )
        })?;
    let is_replay = suggestion.status == SuggestionStatus::Accepted
        && expected_suggestion_version
            .checked_add(1)
            .is_some_and(|next| suggestion.version == next);
    if is_replay {
        // The request-key-derived Task ID and stored Task response receipt prove this
        // is the original idempotent request. Storage reads the Suggestion and request
        // receipt together, so a changed Coworker head cannot alter the replay result.
        let replay = state
            .store
            .get_suggestion_task_acceptance_receipt(
                &state.principal_id,
                &workspace_id,
                &suggestion_id,
                expected_suggestion_version,
                &request_id,
                &expected_task_id,
            )
            .map_err(|_| unavailable())?;
        let Some(receipt) = replay else {
            return Err(operator_error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Suggestion acceptance does not match this idempotency request; refresh before continuing",
            ));
        };
        return Ok(suggestion_task_acceptance_response(
            StatusCode::OK,
            receipt,
            &request_id,
            &state.principal_id,
        ));
    }
    if suggestion.status != SuggestionStatus::Proposed
        || suggestion.version != expected_suggestion_version
    {
        if suggestion.status == SuggestionStatus::Expired {
            return Err(operator_error(
                StatusCode::CONFLICT,
                "SUGGESTION_EXPIRED",
                "This Suggestion expired and was settled without creating work",
            ));
        }
        return Err(operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Suggestion changed; refresh before accepting it",
        ));
    }
    if suggestion.proposed_action != SuggestionAction::Task
        || suggestion.proposed_task_spec.is_none()
    {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "INVALID_TRANSITION",
            "This Suggestion does not contain an actionable Task proposal",
        ));
    }

    let workspace = ensure_workspace_owner(&state, &workspace_id)?;
    if workspace.status != "ACTIVE" {
        return Err(operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces cannot accept new Tasks",
        ));
    }
    let proposal = suggestion
        .proposed_task_spec
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid)?;

    let coworker_origin = if let Some(coworker_id) = suggestion.coworker_id.as_deref() {
        let read_event = event_context(
            &state.runtime_id,
            &new_id("cor").map_err(|_| unavailable())?,
        )
        .map_err(|_| unavailable())?;
        let coworker_store = SqliteCoworkerStore::new(
            state.store.clone(),
            CoworkerEventContext {
                event_id: read_event.event_id,
                origin_runtime_id: read_event.origin_runtime_id,
                hlc_timestamp: read_event.hlc_timestamp,
                correlation_id: read_event.correlation_id,
                causation_id: read_event.causation_id,
                recorded_at: read_event.recorded_at,
            },
        )
        .map_err(|_| unavailable())?;
        let (head, revision) = coworker_store
            .get(&state.principal_id, &workspace_id, coworker_id)
            .map_err(|_| unavailable())?
            .ok_or_else(|| {
                operator_error(
                    StatusCode::CONFLICT,
                    "CONFLICT",
                    "The Suggestion's Coworker is no longer available",
                )
            })?;
        if head.status == CoworkerStatus::Archived {
            return Err(operator_error(
                StatusCode::CONFLICT,
                "COWORKER_ARCHIVED",
                "The Suggestion's Coworker is archived",
            ));
        }
        let failover_policy = revision
            .definition
            .lead_failover_policy
            .map(|policy| serde_json::Value::Object(policy.into_iter().collect()));
        (
            Some(head.coworker_id),
            Some(head.version),
            Some(revision.revision),
            revision.definition.default_lead_agent_binding_id,
            failover_policy,
        )
    } else {
        (None, None, None, None, None)
    };

    let lead_binding_id = coworker_origin
        .3
        .clone()
        .or(workspace.default_agent_binding_id.clone())
        .ok_or_else(|| {
            operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "AGENT_UNAVAILABLE",
                "Choose and enable a lead agent before accepting this Task",
            )
        })?;
    let binding = state
        .store
        .get_agent_binding(&state.principal_id, &workspace_id, &lead_binding_id)
        .map_err(|error| map_agent_store_error(error, "Task lead AgentBinding is unavailable"))?
        .ok_or_else(|| {
            operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "AGENT_UNAVAILABLE",
                "Task lead AgentBinding is unavailable",
            )
        })?;
    if !binding.enabled || !binding.lead_eligible {
        return Err(operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "AGENT_UNAVAILABLE",
            "The selected Coworker lead agent is not enabled for lead work",
        ));
    }

    let correlation_id = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        "suggestion-accept-task",
    );
    let task_event = event_context(&state.runtime_id, &correlation_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Task request could not be initialized",
        )
    })?;
    let mut suggestion_event = task_event.clone();
    suggestion_event.event_id = new_id("ev").map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Task request could not be initialized",
        )
    })?;
    let task_id = expected_task_id;

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
        "preferred_lead_agent_binding_id": lead_binding_id.clone(),
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
        lead_agent_binding_id: lead_binding_id.clone(),
        origin_coworker_id: coworker_origin.0,
        origin_coworker_revision: coworker_origin.2,
        expected_coworker_version: coworker_origin.1,
        coworker_default_lead_failover_policy: coworker_origin.4.clone(),
        principal_id: state.principal_id.clone(),
        request_id: request_id.clone(),
        request_payload,
        event: task_event,
    };
    let store = state.store.clone();
    let accepted_at = suggestion_event.recorded_at.clone();
    let committed = tokio::task::spawn_blocking(move || {
        TaskService::new(store).create_from_suggestion(
            command,
            suggestion_id,
            expected_suggestion_version,
            accepted_at,
            suggestion_event,
        )
    })
    .await
    .map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Suggestion acceptance did not complete",
        )
    })?
    .map_err(|error| match error {
        storage_core::StoreError::Invalid(_) => operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ARGUMENT",
            "The Task proposal is invalid or its lead is unavailable",
        ),
        storage_core::StoreError::Conflict { .. } => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Suggestion, Coworker, or Task inputs changed; refresh before accepting",
        ),
        storage_core::StoreError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Suggestion or a pinned Task input is unavailable",
        ),
        _ => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Suggestion and Task could not be committed",
        ),
    })?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let response_status = match committed.disposition {
        storage_core::SuggestionAcceptanceDisposition::Created => StatusCode::CREATED,
        storage_core::SuggestionAcceptanceDisposition::Replayed => StatusCode::OK,
    };
    Ok(suggestion_task_acceptance_response(
        response_status,
        committed,
        &request_id,
        &state.principal_id,
    ))
}

fn suggestion_task_acceptance_response(
    status: StatusCode,
    receipt: storage_core::SuggestionTaskAcceptanceReceipt,
    request_id: &str,
    principal_id: &str,
) -> Response {
    let suggestion_version = receipt.suggestion.version;
    let correlation_id = idempotent_id("cor", principal_id, request_id, "suggestion-accept-task");
    let mut response = (status, Json(receipt)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(value) = header::HeaderValue::from_str(&format!("\"{suggestion_version}\"")) {
        response.headers_mut().insert(header::ETAG, value);
    }
    if let Ok(value) = header::HeaderValue::from_str(&correlation_id) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    response
}

async fn snooze_suggestion(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(suggestion_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = validate_action_workspace(&state, &headers)?;
    if !valid_id(&suggestion_id) {
        return Err(invalid());
    }
    if body.len() > 4096 {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Suggestion request exceeds the local body limit",
        ));
    }
    let body: SnoozeSuggestionBody = serde_json::from_slice(&body).map_err(|_| invalid())?;
    let snoozed_until = match body.snoozed_until {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) => Some(value),
        _ => return Err(invalid()),
    };
    if snoozed_until.as_deref().is_some_and(|value| {
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).is_err()
    }) {
        return Err(invalid());
    }
    let request_id = idempotency_key(&headers)?;
    let version = expected_version(&headers)?;
    let mut service = action_service(&state, &request_id)?;
    let suggestion = service
        .owner_action(
            &state.principal_id,
            &workspace,
            &suggestion_id,
            version,
            &request_id,
            SuggestionOwnerAction::Snooze { snoozed_until },
        )
        .map_err(expiry_error)?;
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
    let binding = state
        .store
        .get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
            owner_principal_id: state.principal_id.clone(),
            workspace_id: workspace_id.clone(),
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
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(invalid());
    }
    let status = query.status.or(Some(SuggestionStatus::Proposed));
    let visibility_value = visibility(query.visibility.as_deref())?;
    let visibility_name = query.visibility.as_deref().unwrap_or("VISIBLE");
    let after = query
        .cursor
        .as_deref()
        .map(|value| decode_cursor(value, &workspace_id, status, visibility_name))
        .transpose()?;
    let correlation_id = new_id("cor").map_err(|_| unavailable())?;
    let event = event_context(&state.runtime_id, &correlation_id).map_err(|_| unavailable())?;
    let expiry_store = SqliteSuggestionStore::with_event_context(
        state.store.clone(),
        SuggestionEventContext {
            origin_runtime_id: event.origin_runtime_id,
            correlation_id: event.correlation_id,
        },
    )
    .map_err(expiry_error)?;
    let mut service = SuggestionService::new(expiry_store, OperatorSuggestionClock);
    let expiry = service
        .settle_expired(&state.principal_id, &workspace_id)
        .map_err(expiry_error)?;
    if expiry.more_due {
        return Err(expiry_pending());
    }
    let page = SqliteSuggestionStore::new(state.store.clone())
        .list(
            &state.principal_id,
            &workspace_id,
            status,
            visibility_value,
            &expiry.as_of,
            after,
            limit,
        )
        .map_err(read_error)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let next_cursor = page
        .next
        .map(|(created_at, suggestion_id)| {
            serde_json::to_vec(&SuggestionCursor {
                version: 1,
                workspace_id,
                status,
                visibility: visibility_name.to_owned(),
                created_at,
                suggestion_id,
            })
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
            .map_err(|_| unavailable())
        })
        .transpose()?;
    let mut response = Json(SuggestionPageResponse {
        items: page.items,
        next_cursor,
    })
    .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
        middleware,
    };
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use sha2::Digest;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        sync::{Arc, atomic::AtomicU64},
        time::Duration,
    };
    use storage_core::{
        BlobPurpose, DeviceIdentityRecord, LocalRuntimeWorkspaceEnrollmentRequest,
        RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
        RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
        RuntimeWorkspaceBindingStore, StoreError, WorkspaceCreateRequest,
    };
    use storage_sqlite::{
        FileBlobStore, RuntimeOsPrincipalIdentity, SqliteConfig, SqliteWorkspaceStore,
        WorkspaceBlobKey, WorkspaceBlobKeyProvider,
    };
    use tower::ServiceExt;
    use zeroize::Zeroizing;

    #[derive(Clone)]
    struct PreferenceRouteTestKeys;

    impl WorkspaceBlobKeyProvider for PreferenceRouteTestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([73_u8; 32]),
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

    #[tokio::test]
    async fn preference_route_accepts_200_byte_key_and_replays_one_commit() {
        const OWNER: &str = "suggestion-preference-route-owner";
        const WORKSPACE: &str = "suggestion-preference-route-workspace";
        const RUNTIME: &str = "suggestion-preference-route-runtime";
        const INCARNATION: &str = "suggestion-preference-route-incarnation";
        const NOW: &str = "2026-10-15T10:00:00Z";

        let directory = tempfile::tempdir().expect("temporary Runtime data directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private data directory permissions");
        let database = directory.path().join("state.sqlite3");
        let store = SqliteWorkspaceStore::open(
            &database,
            Arc::new(FileBlobStore::new(
                directory.path().join("blobs"),
                PreferenceRouteTestKeys,
            )),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("real SQLite store opens");

        let public_key = [23_u8; 32];
        store
            .register_local_incarnation(
                RuntimeRecord {
                    runtime_id: RUNTIME.to_owned(),
                    device_identity: DeviceIdentityRecord {
                        device_id: format!(
                            "ed25519-sha256:{}",
                            hex::encode(sha2::Sha256::digest(public_key))
                        ),
                        public_key: format!("ed25519:{}", hex::encode(public_key)),
                        key_version: 1,
                        issued_at: NOW.to_owned(),
                        display_name: None,
                    },
                    runtime_version: "0.1.0-test".to_owned(),
                    platform: "linux".to_owned(),
                    architecture: "x86_64".to_owned(),
                    roles: vec!["OPERATOR_ENDPOINT".to_owned()],
                    trust_zone: "PERSONAL_DEVICE".to_owned(),
                    availability: "RECOVERING".to_owned(),
                    startup_policy: "MANUAL".to_owned(),
                    current_incarnation_id: INCARNATION.to_owned(),
                    resource_capacity: json!({"sampled_at": NOW}),
                    last_seen: NOW.to_owned(),
                },
                RuntimeIncarnationRecord {
                    runtime_incarnation_id: INCARNATION.to_owned(),
                    runtime_id: RUNTIME.to_owned(),
                    process_started_at: NOW.to_owned(),
                    litecowork_version: "0.1.0-test".to_owned(),
                    recovered_from_unclean_shutdown: false,
                    recovery_state: "RECOVERING".to_owned(),
                    ready_at: None,
                    stopped_at: None,
                    version: 1,
                },
                RuntimeIncarnationLocalObservationRecord {
                    runtime_incarnation_id: INCARNATION.to_owned(),
                    os_boot_id: Some("boot-suggestion-preference-route".to_owned()),
                    observed_at: NOW.to_owned(),
                },
            )
            .expect("Runtime registers");
        store
            .transition_local_incarnation(RuntimeIncarnationStateUpdate {
                runtime_id: RUNTIME.to_owned(),
                runtime_incarnation_id: INCARNATION.to_owned(),
                expected_version: 1,
                recovery_state: "DEGRADED".to_owned(),
                availability: "DEGRADED".to_owned(),
                observed_at: NOW.to_owned(),
                stopped_at: None,
            })
            .expect("Runtime becomes available");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Suggestion preference route fixture".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "suggestion-preference-route-workspace-created".to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "suggestion-preference-route-workspace-create".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("Workspace commits");
        store
            .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
                request: WorkspaceCreateRequest {
                    principal_id: OWNER.to_owned(),
                    request_id: "suggestion-preference-route-enrollment".to_owned(),
                    request_payload: json!({"workspace_id": WORKSPACE}),
                },
                expected_workspace_version: 1,
                runtime_workspace_binding_id: "suggestion-preference-route-binding".to_owned(),
                runtime_id: RUNTIME.to_owned(),
                runtime_incarnation_id: INCARNATION.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                now: NOW.to_owned(),
                correlation_id: "suggestion-preference-route-enroll-correlation".to_owned(),
            })
            .expect("Workspace grants this local Runtime binding");

        let (_shutdown_sender, stream_shutdown) = tokio::sync::watch::channel(false);
        let state = ApiState {
            store,
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: INCARNATION.to_owned(),
            lifecycle_blockers: Vec::new(),
            runtime_identity: Arc::new(
                RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                    .expect("test-only OS identity is available"),
            ),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
            runtime_inventory_revision: Arc::new(AtomicU64::new(1)),
            stream_shutdown,
        };
        let router = super::routes()
            .route_layer(middleware::from_fn(super::super::authenticate))
            .with_state(state);
        let idempotency_key = "k".repeat(200);
        let request = |key: &str, muted: bool| {
            let mut request = Request::put(format!(
                "/v1/workspaces/{WORKSPACE}/suggestion-preferences/TASK_OPPORTUNITY"
            ))
            .header("x-workspace-id", WORKSPACE)
            .header("if-match", "0")
            .header("idempotency-key", key)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"muted": muted}).to_string()))
            .expect("preference request is valid HTTP");
            request
                .extensions_mut()
                .insert(super::super::AuthenticatedLocalPeer);
            request
        };

        let first = router
            .clone()
            .oneshot(request(&idempotency_key, true))
            .await
            .expect("first preference mutation returns a response");
        assert_eq!(first.status(), StatusCode::OK);
        let first_body: serde_json::Value = serde_json::from_slice(
            &to_bytes(first.into_body(), 4096)
                .await
                .expect("bounded preference response"),
        )
        .expect("preference response is valid JSON");
        assert_eq!(first_body["muted"], true);
        assert_eq!(first_body["version"], 1);

        let replay = router
            .clone()
            .oneshot(request(&idempotency_key, true))
            .await
            .expect("same key and payload resolve the saved receipt");
        assert_eq!(replay.status(), StatusCode::OK);
        let replay_body: serde_json::Value = serde_json::from_slice(
            &to_bytes(replay.into_body(), 4096)
                .await
                .expect("bounded replay response"),
        )
        .expect("replay response is valid JSON");
        assert_eq!(replay_body, first_body);

        let conflict = router
            .clone()
            .oneshot(request(&idempotency_key, false))
            .await
            .expect("changed payload returns a receipt conflict");
        assert_eq!(conflict.status(), StatusCode::CONFLICT);

        let too_long_key = "k".repeat(201);
        let too_long = router
            .oneshot(request(&too_long_key, true))
            .await
            .expect("201-byte key returns validation error");
        assert_eq!(too_long.status(), StatusCode::BAD_REQUEST);
        let too_long_body: serde_json::Value = serde_json::from_slice(
            &to_bytes(too_long.into_body(), 4096)
                .await
                .expect("bounded validation error"),
        )
        .expect("validation error is valid JSON");
        assert_eq!(too_long_body["code"], "INVALID_ARGUMENT");

        let connection =
            rusqlite::Connection::open(&database).expect("inspect committed preference mutation");
        let preference_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM suggestion_preferences WHERE workspace_id = ?1 AND kind = 'TASK_OPPORTUNITY'",
                [WORKSPACE],
                |row| row.get(0),
            )
            .expect("count persisted preference");
        let event_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'suggestion.preference.changed.v1'",
                [WORKSPACE],
                |row| row.get(0),
            )
            .expect("count preference events");
        let receipt_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
                rusqlite::params![OWNER, idempotency_key],
                |row| row.get(0),
            )
            .expect("count preference receipts");
        assert_eq!(preference_count, 1);
        assert_eq!(event_count, 1);
        assert_eq!(receipt_count, 1);
    }
}
