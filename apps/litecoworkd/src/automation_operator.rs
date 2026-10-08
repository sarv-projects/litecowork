//! Authenticated local Operator routes for stored Automation definitions.
//!
//! This router exposes the current finite definition API and safe-stop commands over
//! persisted aggregates. Trigger hosting, occurrence materialization, `/resume`, manual
//! run, and scheduler execution remain unavailable.

use super::*;
use domain_responsibility::{
    Automation, AutomationDefinition, AutomationRevision, AutomationStatus, CommittedResponsibility,
    DomainError, OwnerCommandScope, ResponsibilityCommand,
    ResponsibilityService,
};
use storage_sqlite::{AutomationRevisionPage, CoworkerEventContext, SqliteCoworkerStore};

const MAX_AUTOMATION_BODY_BYTES: usize = 128 * 1024;

/// Trigger execution is deliberately outside this router until trigger hosting,
/// occurrence materialization, and Task-origin admission are implemented.
pub(super) fn router() -> Router<ApiState> {
    Router::new()
        .route("/v1/automations", get(list_automations).post(create_automation))
        .route("/v1/automations/{automation_id}", get(get_automation).patch(revise_automation))
        .route("/v1/automations/{automation_id}/revisions", get(list_automation_revisions))
        .route("/v1/automations/{automation_id}/pause", post(pause_automation))
        .route("/v1/automations/{automation_id}/disable", post(disable_automation))
        .layer(DefaultBodyLimit::max(MAX_AUTOMATION_BODY_BYTES))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AutomationQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutomationCursor {
    version: u32,
    workspace_id: String,
    updated_at: String,
    automation_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutomationRevisionCursor {
    version: u32,
    workspace_id: String,
    automation_id: String,
    before_revision: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AutomationRevisionQuery {
    cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateAutomationBody {
    workspace_id: String,
    name: String,
    routine_id: String,
    routine_revision: u64,
    triggers: Vec<domain_responsibility::TriggerSpec>,
    execution_policy: domain_responsibility::AutomationExecutionPolicy,
    coworker_ref: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviseAutomationBody {
    name: String,
    routine_id: String,
    routine_revision: u64,
    triggers: Vec<domain_responsibility::TriggerSpec>,
    execution_policy: domain_responsibility::AutomationExecutionPolicy,
    coworker_ref: serde_json::Value,
}

fn parse_coworker_ref(value: serde_json::Value) -> Result<Option<domain_responsibility::CoworkerRevisionRef>, Response> {
    if value.is_null() {
        return Ok(None);
    }
    let Some(object) = value.as_object() else { return Err(invalid()); };
    let Some(coworker_id) = object.get("coworker_id").and_then(serde_json::Value::as_str) else { return Err(invalid()); };
    let Some(revision) = object.get("revision").and_then(serde_json::Value::as_u64) else { return Err(invalid()); };
    if object.len() != 2 || !valid_id(coworker_id) || revision == 0 { return Err(invalid()); }
    serde_json::from_value(value).map(Some).map_err(|_| invalid())
}

#[derive(Serialize)]
struct AutomationPageResponse {
    items: Vec<Automation>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct AutomationRevisionPageResponse {
    items: Vec<AutomationRevision>,
    next_cursor: Option<String>,
}

fn invalid() -> Response {
    operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Automation request is invalid")
}

fn unavailable() -> Response {
    operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Automation storage is unavailable")
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    response
}

fn domain_error(error: DomainError) -> Response {
    match error {
        DomainError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Automation or pinned dependency is unavailable"),
        DomainError::Unauthorized => operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"),
        DomainError::AlreadyExists | DomainError::VersionConflict | DomainError::IdempotencyConflict
        | DomainError::TriggerSourceChanged => operator_error(StatusCode::CONFLICT, "CONFLICT", "Automation version or idempotency key conflicts with current state"),
        DomainError::AutomationDisabled => operator_error(StatusCode::CONFLICT, "AUTOMATION_DISABLED", "Disabled Automations cannot be changed"),
        DomainError::AutomationNotPaused => operator_error(StatusCode::CONFLICT, "AUTOMATION_NOT_PAUSED", "Pause this Automation before changing its definition"),
        DomainError::RoutineArchived => operator_error(StatusCode::CONFLICT, "ROUTINE_ARCHIVED", "The pinned Routine is unavailable for activation"),
        DomainError::CoworkerInactive => operator_error(StatusCode::CONFLICT, "COWORKER_PAUSED", "The pinned Coworker is paused"),
        DomainError::ReconciliationRequired => operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Automation activation requires a trigger host and occurrence reconciliation service that is not available"),
        DomainError::TriggerUnsupported => operator_error(StatusCode::BAD_REQUEST, "TRIGGER_UNSUPPORTED", "This trigger type is not supported by the local Automation service"),
        DomainError::WorkspaceArchived => operator_error(StatusCode::CONFLICT, "WORKSPACE_ARCHIVED", "Archived Workspaces are read-only"),
        DomainError::InvalidDefinition => invalid(),
        DomainError::VersionOverflow | DomainError::Storage | DomainError::CoworkerArchived | DomainError::ArchiveBlocked => unavailable(),
    }
}

fn authorized_workspace(state: &ApiState, headers: &HeaderMap) -> Result<Workspace, Response> {
    let workspace_id = selected_workspace(headers)?;
    let workspace = ensure_workspace_owner(state, &workspace_id)?;
    let binding = state.store.get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
        owner_principal_id: state.principal_id.clone(),
        workspace_id,
        runtime_id: state.runtime_id.clone(),
        runtime_incarnation_id: state.local_incarnation_id.clone(),
    }).map_err(|_| unavailable())?;
    if !binding.is_some_and(|binding| binding.status == "ACTIVE") {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Local Runtime Workspace access is unavailable"));
    }
    Ok(workspace)
}

fn adapter(state: &ApiState, correlation_id: &str) -> Result<SqliteCoworkerStore, Response> {
    let event = event_context(&state.runtime_id, correlation_id).map_err(|_| unavailable())?;
    SqliteCoworkerStore::new(state.store.clone(), CoworkerEventContext {
        event_id: event.event_id,
        origin_runtime_id: event.origin_runtime_id,
        hlc_timestamp: event.hlc_timestamp,
        correlation_id: event.correlation_id,
        causation_id: event.causation_id,
        recorded_at: event.recorded_at,
    }).map_err(domain_error)
}

fn automation_view(automation: Automation, workspace_id: &str) -> Result<Automation, Response> {
    if automation.workspace_id != workspace_id || automation.current_revision == 0 || automation.version == 0 {
        return Err(unavailable());
    }
    Ok(automation)
}

fn parse_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    if body.len() > MAX_AUTOMATION_BODY_BYTES {
        return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Automation request exceeds the local body limit"));
    }
    serde_json::from_slice(body).map_err(|_| invalid())
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

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}

fn decode_cursor(encoded: &str, workspace: &str) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 { return Err(invalid()); }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: AutomationCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1 || cursor.workspace_id != workspace || !valid_id(&cursor.automation_id)
        || cursor.updated_at.len() > 64
        || time::OffsetDateTime::parse(&cursor.updated_at, &time::format_description::well_known::Rfc3339).is_err()
    { return Err(invalid()); }
    Ok((cursor.updated_at, cursor.automation_id))
}

fn decode_revision_cursor(encoded: &str, workspace: &str, automation_id: &str) -> Result<u64, Response> {
    if encoded.is_empty() || encoded.len() > 2048 { return Err(invalid()); }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: AutomationRevisionCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1 || cursor.workspace_id != workspace || cursor.automation_id != automation_id || cursor.before_revision == 0 {
        return Err(invalid());
    }
    Ok(cursor.before_revision)
}

fn execute(
    state: &ApiState,
    workspace: &Workspace,
    request_id: String,
    command: ResponsibilityCommand,
    response_status: StatusCode,
) -> Result<Response, Response> {
    let correlation = idempotent_id("cor", &state.principal_id, &request_id, "correlation");
    let store = adapter(state, &correlation)?;
    let result = ResponsibilityService::new(store.clone()).execute(&OwnerCommandScope {
        principal_id: state.principal_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        request_id,
    }, command).map_err(domain_error)?;
    let CommittedResponsibility::Automation(head) = result else { return Err(unavailable()); };
    // Reads and replay reconstruct the exact revision pinned by this command result.
    let revision = store.get_automation_revision(&state.principal_id, &workspace.workspace_id, &head.automation_id, head.current_revision)
        .map_err(domain_error)?.ok_or_else(unavailable)?;
    if revision.revision != head.current_revision { return Err(unavailable()); }
    ensure_workspace_owner(state, &workspace.workspace_id)?;
    let mut response = (response_status, Json(automation_view(head, &workspace.workspace_id)?)).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    if let Ok(value) = header::HeaderValue::from_str(&correlation) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn list_automations(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<AutomationQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) { return Err(invalid()); }
    let after = query.cursor.as_deref().map(|cursor| decode_cursor(cursor, &workspace.workspace_id)).transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let page = store.list_automations(&state.principal_id, &workspace.workspace_id, None, after, limit).map_err(domain_error)?;
    let items = page.items.into_iter().map(|(head, revision)| {
        if revision.automation_id != head.automation_id || revision.revision != head.current_revision { return Err(unavailable()); }
        automation_view(head, &workspace.workspace_id)
    }).collect::<Result<Vec<_>, _>>()?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    let next_cursor = page.next.map(|(updated_at, automation_id)| serde_json::to_vec(&AutomationCursor {
        version: 1,
        workspace_id: workspace.workspace_id.clone(),
        updated_at,
        automation_id,
    }).map(|bytes| URL_SAFE_NO_PAD.encode(bytes))).transpose().map_err(|_| unavailable())?;
    let mut response = Json(AutomationPageResponse { items, next_cursor }).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn get_automation(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let (head, revision) = store.get_automation(&state.principal_id, &workspace.workspace_id, &id)
        .map_err(domain_error)?.ok_or_else(|| domain_error(DomainError::NotFound))?;
    if revision.automation_id != head.automation_id || revision.revision != head.current_revision { return Err(unavailable()); }
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    Ok(no_store(Json(automation_view(head, &workspace.workspace_id)?).into_response()))
}

async fn list_automation_revisions(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    query: Result<Query<AutomationRevisionQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let Query(query) = query.map_err(|_| invalid())?;
    let before_revision = query.cursor.as_deref().map(|cursor| decode_revision_cursor(cursor, &workspace.workspace_id, &id)).transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let page: AutomationRevisionPage = store.list_automation_revisions(&state.principal_id, &workspace.workspace_id, &id, before_revision, 50).map_err(domain_error)?;
    if page.items.iter().any(|revision| revision.automation_id != id || revision.revision == 0) {
        return Err(unavailable());
    }
    let next_cursor = page.next.map(|before_revision| serde_json::to_vec(&AutomationRevisionCursor {
        version: 1,
        workspace_id: workspace.workspace_id.clone(),
        automation_id: id.clone(),
        before_revision,
    }).map(|bytes| URL_SAFE_NO_PAD.encode(bytes))).transpose().map_err(|_| unavailable())?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    let mut response = Json(AutomationRevisionPageResponse { items: page.items, next_cursor }).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn create_automation(State(state): State<ApiState>, headers: HeaderMap, body: Bytes) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: CreateAutomationBody = parse_body(&body)?;
    if body.workspace_id != workspace.workspace_id { return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Automation Workspace does not match the selected Workspace")); }
    let request_id = idempotency_key(&headers)?;
    let coworker_ref = parse_coworker_ref(body.coworker_ref.clone())?;
    let automation_id = idempotent_id("aut", &state.principal_id, &request_id, &format!("automation.create.{}", workspace.workspace_id));
    execute(&state, &workspace, request_id, ResponsibilityCommand::CreateAutomation {
        automation_id,
        name: body.name,
        definition: AutomationDefinition {
            routine_id: body.routine_id,
            routine_revision: body.routine_revision,
            triggers: body.triggers,
            execution_policy: body.execution_policy,
            coworker_ref,
        },
    }, StatusCode::CREATED)
}

async fn revise_automation(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>, body: Bytes) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let body: ReviseAutomationBody = parse_body(&body)?;
    let definition = AutomationDefinition {
        routine_id: body.routine_id,
        routine_revision: body.routine_revision,
        triggers: body.triggers,
        execution_policy: body.execution_policy,
        coworker_ref: parse_coworker_ref(body.coworker_ref.clone())?,
    };
    execute(&state, &workspace, idempotency_key(&headers)?, ResponsibilityCommand::ReviseAutomation {
        automation_id: id,
        expected_version: expected_version(&headers)?,
        name: body.name,
        definition,
    }, StatusCode::OK)
}

async fn pause_automation(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, Response> {
    change_status(&state, &headers, id, AutomationStatus::Paused).await
}

async fn disable_automation(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, Response> {
    change_status(&state, &headers, id, AutomationStatus::Disabled).await
}

async fn change_status(state: &ApiState, headers: &HeaderMap, id: String, status: AutomationStatus) -> Result<Response, Response> {
    let workspace = authorized_workspace(state, headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    execute(state, &workspace, idempotency_key(headers)?, ResponsibilityCommand::SetAutomationStatus {
        automation_id: id,
        expected_version: expected_version(headers)?,
        status,
    }, StatusCode::OK)
}
