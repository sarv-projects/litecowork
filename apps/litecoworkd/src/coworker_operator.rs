//! Authenticated Coworker configuration routes. No Automation hosting or Task
//! execution is started by these commands. Presence awaits its complete projection.
use super::*;
use domain_responsibility::{
    CommittedResponsibility, Coworker, CoworkerDefinition, CoworkerRevision,
    CoworkerStatus, DomainError, OwnerCommandScope, ResponsibilityCommand,
    ResponsibilityService,
};
use domain_workspace::{EventContext, SetWorkspacePrimaryCoworker, WorkspaceService};
use storage_sqlite::{CoworkerEventContext, SqliteCoworkerStore};

const MAX_COWORKER_BODY_BYTES: usize = 64 * 1024;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/coworkers", get(list_coworkers).post(create_coworker))
        .route("/v1/coworkers/{coworker_id}", get(get_coworker))
        .route("/v1/coworkers/{coworker_id}/revisions", post(revise_coworker))
        .route("/v1/coworkers/{coworker_id}/revisions/{revision}", get(get_coworker_revision))
        .route("/v1/coworkers/{coworker_id}/status", post(change_status))
        .route("/v1/coworkers/{coworker_id}/presence", get(presence_unavailable))
        .route("/v1/workspaces/{workspace_id}/primary-coworker", post(set_primary_coworker))
        .layer(DefaultBodyLimit::max(MAX_COWORKER_BODY_BYTES))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoworkerQuery { status: Option<CoworkerStatus>, cursor: Option<String>, limit: Option<usize> }

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoworkerCursor {
    version: u32, workspace_id: String, status: Option<CoworkerStatus>, updated_at: String, coworker_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody { workspace_id: String, revision: serde_json::Value }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusBody { status: CoworkerStatus }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrimaryCoworkerBody { coworker_id: serde_json::Value }

#[derive(Serialize)]
struct CoworkerView {
    coworker_id: String, workspace_id: String, current_revision: u64,
    revision: CoworkerDefinition, status: CoworkerStatus, is_primary: bool,
    created_at: String, updated_at: String, version: u64,
}

#[derive(Serialize)]
struct CoworkerRevisionView {
    coworker_id: String,
    revision: u64,
    definition: CoworkerDefinition,
    authored_by: domain_responsibility::PrincipalRef,
    created_at: String,
}

fn invalid() -> Response { operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Coworker request is invalid") }
fn unavailable() -> Response { operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Coworker projection is unavailable") }

fn domain_error(error: DomainError) -> Response {
    match error {
        DomainError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Coworker is unavailable"),
        DomainError::Unauthorized => operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"),
        DomainError::CoworkerArchived => operator_error(StatusCode::CONFLICT, "COWORKER_ARCHIVED", "Archived Coworkers cannot be changed"),
        DomainError::CoworkerInactive => operator_error(StatusCode::CONFLICT, "COWORKER_PAUSED", "Coworker is paused"),
        DomainError::ArchiveBlocked => operator_error(StatusCode::CONFLICT, "COWORKER_HAS_ACTIVE_WORK", "Clear the primary selection and settle Coworker work before archiving"),
        DomainError::VersionConflict | DomainError::AlreadyExists | DomainError::IdempotencyConflict => operator_error(StatusCode::CONFLICT, "CONFLICT", "Coworker version or idempotency key conflicts with current state"),
        DomainError::ReconciliationRequired => operator_error(StatusCode::CONFLICT, "CONFLICT", "Coworker dependencies require reconciliation before resume"),
        DomainError::WorkspaceArchived => operator_error(StatusCode::CONFLICT, "WORKSPACE_ARCHIVED", "Archived Workspaces are read-only"),
        DomainError::InvalidDefinition => invalid(),
        DomainError::Storage | DomainError::VersionOverflow => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Coworker storage is unavailable"),
        _ => unavailable(),
    }
}

fn authorized_workspace(state: &ApiState, headers: &HeaderMap) -> Result<Workspace, Response> {
    let workspace_id = selected_workspace(headers)?;
    let workspace = ensure_workspace_owner(state, &workspace_id)?;
    let binding = state.store.get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
        owner_principal_id: state.principal_id.clone(), workspace_id,
        runtime_id: state.runtime_id.clone(), runtime_incarnation_id: state.local_incarnation_id.clone(),
    }).map_err(|_| unavailable())?;
    if !binding.is_some_and(|binding| binding.status == "ACTIVE") {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Local Runtime Workspace access is unavailable"));
    }
    Ok(workspace)
}

fn adapter(state: &ApiState) -> Result<(SqliteCoworkerStore, String), Response> {
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    adapter_with_correlation(state, correlation)
}

fn adapter_with_correlation(state: &ApiState, correlation: String) -> Result<(SqliteCoworkerStore, String), Response> {
    let event = event_context(&state.runtime_id, &correlation).map_err(|_| unavailable())?;
    let context = CoworkerEventContext {
        event_id:event.event_id, origin_runtime_id:event.origin_runtime_id,
        hlc_timestamp:event.hlc_timestamp, correlation_id:event.correlation_id,
        causation_id:event.causation_id, recorded_at:event.recorded_at,
    };
    Ok((SqliteCoworkerStore::new(state.store.clone(), context).map_err(domain_error)?, correlation))
}

fn view(head: Coworker, revision: CoworkerRevision, workspace: &Workspace) -> Result<CoworkerView, Response> {
    if head.workspace_id != workspace.workspace_id || revision.coworker_id != head.coworker_id
        || revision.revision != head.current_revision { return Err(unavailable()); }
    Ok(CoworkerView {
        is_primary:workspace.primary_coworker_id.as_deref() == Some(head.coworker_id.as_str()),
        coworker_id:head.coworker_id, workspace_id:head.workspace_id, current_revision:head.current_revision,
        revision:revision.definition, status:head.status, created_at:head.created_at, updated_at:head.updated_at, version:head.version,
    })
}

fn definition(value: serde_json::Value) -> Result<CoworkerDefinition, Response> {
    const FIELDS: &[&str] = &["name", "avatar_ref", "role_description", "default_lead_agent_binding_id", "delegation_strategy", "enabled_delegation_profile_ids", "delegation_budget_policy", "lead_failover_policy", "interaction_policy", "context_policy", "notification_policy"];
    let object = value.as_object().ok_or_else(invalid)?;
    if object.keys().any(|key| !FIELDS.contains(&key.as_str())) { return Err(invalid()); }
    let result: CoworkerDefinition = serde_json::from_value(value).map_err(|_| invalid())?;
    domain_responsibility::validate_coworker_definition(&result).map_err(domain_error)?;
    Ok(result)
}

fn parse_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    if body.len() > MAX_COWORKER_BODY_BYTES { return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Coworker request exceeds the local body limit")); }
    serde_json::from_slice(body).map_err(|_| invalid())
}
fn key(headers: &HeaderMap) -> Result<String, Response> {
    headers.get("idempotency-key").and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 128 && value.bytes().all(|byte| byte.is_ascii_graphic()))
        .map(str::to_owned).ok_or_else(invalid)
}
fn version(headers: &HeaderMap) -> Result<u64, Response> {
    let text = headers.get(header::IF_MATCH).and_then(|value| value.to_str().ok()).ok_or_else(invalid)?.trim();
    let text = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 { &text[1..text.len()-1] } else { text };
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) { return Err(invalid()); }
    text.parse::<u64>().ok().filter(|version| *version > 0).ok_or_else(invalid)
}
fn valid_id(id: &str) -> bool { !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control) }

fn decode_cursor(encoded: &str, workspace: &str, status: Option<CoworkerStatus>) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 { return Err(invalid()); }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: CoworkerCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1 || cursor.workspace_id != workspace || cursor.status != status || !valid_id(&cursor.coworker_id)
        || cursor.updated_at.len() > 64 || time::OffsetDateTime::parse(&cursor.updated_at, &time::format_description::well_known::Rfc3339).is_err() { return Err(invalid()); }
    Ok((cursor.updated_at, cursor.coworker_id))
}

async fn list_coworkers(State(state): State<ApiState>, headers: HeaderMap, query: Result<Query<CoworkerQuery>, axum::extract::rejection::QueryRejection>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) { return Err(invalid()); }
    let after = query.cursor.as_deref().map(|cursor| decode_cursor(cursor, &workspace.workspace_id, query.status)).transpose()?;
    let (adapter, _) = adapter(&state)?;
    let page = adapter.list(&state.principal_id, &workspace.workspace_id, query.status, after, limit).map_err(domain_error)?;
    let workspace = authorized_workspace(&state, &headers)?;
    let items = page.items.into_iter().map(|(head, revision)| view(head, revision, &workspace)).collect::<Result<Vec<_>, _>>()?;
    let next_cursor = page.next.map(|(updated_at, coworker_id)| serde_json::to_vec(&CoworkerCursor {
        version:1, workspace_id:workspace.workspace_id, status:query.status, updated_at, coworker_id,
    }).map(|bytes| URL_SAFE_NO_PAD.encode(bytes))).transpose().map_err(|_| unavailable())?;
    Ok(Json(json!({"items":items, "next_cursor":next_cursor})).into_response())
}

async fn get_coworker(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let (adapter, _) = adapter(&state)?;
    let (head, revision) = adapter.get(&state.principal_id, &workspace.workspace_id, &id).map_err(domain_error)?.ok_or_else(|| domain_error(DomainError::NotFound))?;
    let workspace = authorized_workspace(&state, &headers)?;
    Ok(Json(view(head, revision, &workspace)?).into_response())
}

async fn get_coworker_revision(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((id, revision)): Path<(String, String)>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) || revision.is_empty() || !revision.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    let expected_revision = revision.parse::<u64>().ok().filter(|revision| *revision > 0).ok_or_else(invalid)?;
    let (adapter, _) = adapter(&state)?;
    let revision = adapter
        .get_revision(&state.principal_id, &workspace.workspace_id, &id, expected_revision)
        .map_err(domain_error)?
        .ok_or_else(|| domain_error(DomainError::NotFound))?;
    // Re-check owner and active local Workspace binding after the immutable read so a
    // concurrent ownership/binding change cannot turn a stale authorization into data.
    let _workspace = authorized_workspace(&state, &headers)?;
    if revision.coworker_id != id || revision.revision != expected_revision {
        return Err(unavailable());
    }
    let mut response = Json(CoworkerRevisionView {
        coworker_id: revision.coworker_id,
        revision: revision.revision,
        definition: revision.definition,
        authored_by: revision.authored_by,
        created_at: revision.created_at,
    })
    .into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}

fn execute(state: &ApiState, workspace: &Workspace, request_id: String, command: ResponsibilityCommand, status: StatusCode) -> Result<Response, Response> {
    // The idempotency key determines the event correlation. A replay therefore
    // returns the same committed receipt identity instead of a fresh request ID.
    let correlation = idempotent_id("cor", &state.principal_id, &request_id, "correlation");
    let (adapter, correlation) = adapter_with_correlation(state, correlation)?;
    let result = ResponsibilityService::new(adapter.clone()).execute(&OwnerCommandScope {
        principal_id:state.principal_id.clone(), workspace_id:workspace.workspace_id.clone(), request_id,
    }, command).map_err(domain_error)?;
    let CommittedResponsibility::Coworker(head) = result else { return Err(unavailable()); };
    // Always reload the exact committed revision, including status-command replay
    // after later edits. Never mix a receipt's old head with today's definition.
    let revision = adapter.get_revision(&state.principal_id, &workspace.workspace_id, &head.coworker_id, head.current_revision)
        .map_err(domain_error)?.ok_or_else(unavailable)?;
    let workspace = ensure_workspace_owner(state, &workspace.workspace_id)?;
    let mut response = (status, Json(view(head, revision, &workspace)?)).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    if let Ok(value) = header::HeaderValue::from_str(&correlation) { response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value); }
    Ok(response)
}

async fn create_coworker(State(state): State<ApiState>, headers: HeaderMap, body: Bytes) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: CreateBody = parse_body(&body)?;
    if body.workspace_id != workspace.workspace_id { return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Coworker Workspace does not match the selected Workspace")); }
    let request_id = key(&headers)?;
    let id = idempotent_id("cw", &state.principal_id, &request_id, &format!("coworker.create.{}", workspace.workspace_id));
    execute(&state, &workspace, request_id, ResponsibilityCommand::CreateCoworker { coworker_id:id, definition:definition(body.revision)? }, StatusCode::CREATED)
}

async fn revise_coworker(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>, body: Bytes) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let input = definition(parse_body(&body)?)?;
    execute(&state, &workspace, key(&headers)?, ResponsibilityCommand::ReviseCoworker { coworker_id:id, expected_version:version(&headers)?, definition:input }, StatusCode::CREATED)
}

async fn change_status(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>, body: Bytes) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let input: StatusBody = parse_body(&body)?;
    execute(&state, &workspace, key(&headers)?, ResponsibilityCommand::SetCoworkerStatus { coworker_id:id, expected_version:version(&headers)?, status:input.status }, StatusCode::OK)
}

async fn presence_unavailable(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let (adapter, _) = adapter(&state)?;
    adapter.get(&state.principal_id, &workspace.workspace_id, &id).map_err(domain_error)?.ok_or_else(|| domain_error(DomainError::NotFound))?;
    Err(operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Coworker presence requires committed Task, UserRequest, Attempt and Runtime projections that are not yet available"))
}

async fn set_primary_coworker(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if workspace.workspace_id != workspace_id {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"));
    }
    let body: PrimaryCoworkerBody = parse_body(&body)?;
    let coworker_id = match body.coworker_id {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) => Some(value),
        _ => return Err(invalid()),
    };
    if coworker_id.as_deref().is_some_and(|id| !valid_id(id)) {
        return Err(invalid());
    }
    let expected_version = version(&headers)?;
    let request_id = key(&headers)?;
    let correlation_id = idempotent_id("cor", &state.principal_id, &request_id, "correlation");
    let event = event_context(&state.runtime_id, &correlation_id).map_err(|_| unavailable())?;
    let command = SetWorkspacePrimaryCoworker {
        workspace_id: workspace_id.clone(),
        expected_version,
        coworker_id: coworker_id.clone(),
        principal_id: state.principal_id.clone(),
        request_id: request_id.clone(),
        request_payload: json!({
            "operation": "workspace.primary_coworker.set.v1",
            "workspace_id": workspace_id,
            "coworker_id": coworker_id,
        }),
        event: EventContext {
            event_id: event.event_id,
            origin_runtime_id: event.origin_runtime_id,
            hlc_timestamp: event.hlc_timestamp,
            correlation_id: event.correlation_id,
            causation_id: event.causation_id,
            recorded_at: event.recorded_at,
        },
    };
    let committed = WorkspaceService::new(state.store.clone())
        .set_primary_coworker_idempotent(command)
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "Primary Coworker selection is invalid or unavailable"),
            storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "STALE_WORKSPACE_VERSION", "Workspace changed; refresh before selecting a primary Coworker"),
            storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Workspace or Coworker is unavailable"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Primary Coworker could not be saved"),
        })?;
    let mut response = Json(committed.workspace).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::PrimaryCoworkerBody;

    #[test]
    fn primary_request_requires_explicit_nullable_selection() {
        assert!(serde_json::from_str::<PrimaryCoworkerBody>(r#"{"coworker_id":null}"#).is_ok());
        assert!(serde_json::from_str::<PrimaryCoworkerBody>(r#"{"coworker_id":"cw_1"}"#).is_ok());
        assert!(serde_json::from_str::<PrimaryCoworkerBody>(r#"{}"#).is_err());
        assert!(serde_json::from_str::<PrimaryCoworkerBody>(r#"{"coworker_id":null,"other":true}"#).is_err());
    }
}
