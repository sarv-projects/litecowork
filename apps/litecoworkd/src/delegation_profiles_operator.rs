//! Authenticated Workspace catalog and versioned commands for worker profiles.
use super::*;
use domain_responsibility::{
    CommittedDelegationProfile, DelegationProfileCommand, DelegationProfileCommandScope,
    DelegationProfileError, DelegationProfileRevisionInput, DelegationProfileService,
    DelegationProfileStatus,
};
use storage_sqlite::{DelegationProfileEventContext, SqliteDelegationProfileStore};

const MAX_BODY_BYTES: usize = 96 * 1024;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/delegation-profiles", get(list_delegation_profiles).post(create_delegation_profile))
        .route("/v1/delegation-profiles/{delegation_profile_id}", get(get_delegation_profile))
        .route("/v1/delegation-profiles/{delegation_profile_id}/revisions", post(revise_delegation_profile))
        .route("/v1/delegation-profiles/{delegation_profile_id}/duplicate", post(duplicate_delegation_profile))
        .route("/v1/delegation-profiles/{delegation_profile_id}/status", post(change_delegation_profile_status))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryParams {
    agent_binding_id: Option<String>,
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u32,
    workspace_id: String,
    agent_binding_id: Option<String>,
    status: Option<String>,
    updated_at: String,
    delegation_profile_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody { workspace_id: String, agent_binding_id: String, revision: Value }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DuplicateBody { name: String }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusBody { status: DelegationProfileStatus }

fn invalid() -> Response {
    operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Delegation profile query is invalid")
}

fn unavailable() -> Response {
    operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Delegation profile catalog is unavailable")
}

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}

fn store_error(error: storage_core::StoreError) -> Response {
    match error {
        storage_core::StoreError::NotFound => operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"),
        storage_core::StoreError::Invalid(_) => invalid(),
        _ => unavailable(),
    }
}

fn domain_error(error: DelegationProfileError) -> Response {
    match error {
        DelegationProfileError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Delegation profile is unavailable"),
        DelegationProfileError::Unauthorized => operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"),
        DelegationProfileError::AlreadyExists | DelegationProfileError::VersionConflict | DelegationProfileError::IdempotencyConflict | DelegationProfileError::Archived => operator_error(StatusCode::CONFLICT, "CONFLICT", "Delegation profile version, name, or idempotency key conflicts with current state"),
        DelegationProfileError::WorkspaceArchived => operator_error(StatusCode::CONFLICT, "WORKSPACE_ARCHIVED", "Archived Workspaces are read-only"),
        DelegationProfileError::OptionsInvalid => operator_error(StatusCode::BAD_REQUEST, "DELEGATION_PROFILE_OPTIONS_INVALID", "Delegation profile options are invalid"),
        DelegationProfileError::SessionOptionsUnsupported => operator_error(StatusCode::CONFLICT, "AGENT_SESSION_OVERRIDE_UNSUPPORTED", "This Runtime cannot validate the requested agent session options"),
        DelegationProfileError::EnablementUnavailable => operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Enabling requires validated adapter, Trust, and Environment admission integration"),
        DelegationProfileError::InvalidDefinition => invalid(),
        DelegationProfileError::Storage | DelegationProfileError::VersionOverflow => unavailable(),
    }
}

fn authorized_workspace(state: &ApiState, headers: &HeaderMap) -> Result<Workspace, Response> {
    let workspace_id = selected_workspace(headers)?;
    let workspace = ensure_workspace_owner(state, &workspace_id)?;
    let binding = state.store.get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
        owner_principal_id: state.principal_id.clone(),
        workspace_id: workspace_id.clone(),
        runtime_id: state.runtime_id.clone(),
        runtime_incarnation_id: state.local_incarnation_id.clone(),
    }).map_err(|_| unavailable())?;
    if !binding.is_some_and(|binding| binding.status == "ACTIVE") {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Local Runtime Workspace access is unavailable"));
    }
    Ok(workspace)
}

fn decode_cursor(encoded: &str, workspace: &str, binding: Option<&str>, status: Option<&str>) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 { return Err(invalid()); }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: Cursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace
        || cursor.agent_binding_id.as_deref() != binding
        || cursor.status.as_deref() != status
        || !valid_id(&cursor.delegation_profile_id)
        || cursor.updated_at.len() > 64
        || time::OffsetDateTime::parse(&cursor.updated_at, &time::format_description::well_known::Rfc3339).is_err()
    { return Err(invalid()); }
    Ok((cursor.updated_at, cursor.delegation_profile_id))
}

async fn list_delegation_profiles(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<QueryParams>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    if query.agent_binding_id.as_deref().is_some_and(|id| !valid_id(id))
        || query.status.as_deref().is_some_and(|status| !matches!(status, "ENABLED" | "DISABLED" | "ARCHIVED"))
    { return Err(invalid()); }
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) { return Err(invalid()); }
    let after = query.cursor.as_deref().map(|cursor| decode_cursor(
        cursor,
        &workspace.workspace_id,
        query.agent_binding_id.as_deref(),
        query.status.as_deref(),
    )).transpose()?;

    let adapter = SqliteDelegationProfileStore::new(state.store.clone());
    let page = adapter.list(
        &state.principal_id,
        &workspace.workspace_id,
        query.agent_binding_id.as_deref(),
        query.status.as_deref(),
        after,
        limit,
    ).map_err(store_error)?;
    let next_cursor = page.next.map(|(updated_at, delegation_profile_id)| serde_json::to_vec(&Cursor {
        version: 1,
        workspace_id: workspace.workspace_id,
        agent_binding_id: query.agent_binding_id,
        status: query.status,
        updated_at,
        delegation_profile_id,
    }).map(|bytes| URL_SAFE_NO_PAD.encode(bytes))).transpose().map_err(|_| unavailable())?;
    Ok(Json(json!({"items": page.items, "next_cursor": next_cursor})).into_response())
}

async fn get_delegation_profile(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let adapter = SqliteDelegationProfileStore::new(state.store.clone());
    let profile = adapter.get(&state.principal_id, &workspace.workspace_id, &id).map_err(store_error)?
        .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Delegation profile is unavailable"))?;
    Ok(Json(profile).into_response())
}

fn parse_body<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Response> {
    if bytes.len() > MAX_BODY_BYTES { return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Delegation profile request exceeds the local body limit")); }
    serde_json::from_slice(bytes).map_err(|_| invalid())
}

fn request_key(headers: &HeaderMap) -> Result<String, Response> {
    headers.get("idempotency-key").and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 128 && value.bytes().all(|byte| byte.is_ascii_graphic()))
        .map(str::to_owned).ok_or_else(invalid)
}

fn expected_version(headers: &HeaderMap) -> Result<u64, Response> {
    let value = headers.get(header::IF_MATCH).and_then(|value| value.to_str().ok()).ok_or_else(invalid)?.trim();
    let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 { &value[1..value.len()-1] } else { value };
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) { return Err(invalid()); }
    value.parse::<u64>().ok().filter(|version| *version > 0).ok_or_else(invalid)
}

fn revision_input(value: Value) -> Result<DelegationProfileRevisionInput, Response> {
    let object = value.as_object().ok_or_else(invalid)?;
    const REQUIRED: &[&str] = &["name", "routing_description", "session_options", "session_options_descriptor_digest", "required_features", "preferred_features", "enforced_policy", "optimization_preference", "max_concurrency", "max_host_delegation_depth", "latency_class", "environment_policy", "native_delegation_policy", "warm_policy"];
    if REQUIRED.iter().any(|key| !object.contains_key(*key)) { return Err(invalid()); }
    serde_json::from_value(value).map_err(|_| invalid())
}

fn profile_view(committed: &CommittedDelegationProfile) -> Result<Value, Response> {
    let mut revision = serde_json::to_value(&committed.revision).map_err(|_| unavailable())?;
    if let Some(object) = revision.as_object_mut() { object.remove("name_key"); }
    Ok(json!({
        "delegation_profile_id": committed.profile.delegation_profile_id,
        "workspace_id": committed.profile.workspace_id,
        "agent_binding_id": committed.profile.agent_binding_id,
        "name": committed.profile.name,
        "current_revision": committed.profile.current_revision,
        "revision": revision,
        "status": committed.profile.status,
        "created_at": committed.profile.created_at,
        "updated_at": committed.profile.updated_at,
        "version": committed.profile.version,
    }))
}

fn execute_command(state: &ApiState, workspace: &Workspace, request_id: String, command: DelegationProfileCommand, response_status: StatusCode) -> Result<Response, Response> {
    let correlation_id = idempotent_id("cor", &state.principal_id, &request_id, "delegation-profile-correlation");
    let response_correlation = correlation_id.clone();
    let event_id = idempotent_id("ev", &state.principal_id, &request_id, "delegation-profile-event");
    let event = event_context(&state.runtime_id, &correlation_id).map_err(|_| unavailable())?;
    let context = DelegationProfileEventContext {
        event_id,
        origin_runtime_id: event.origin_runtime_id,
        hlc_timestamp: event.hlc_timestamp,
        correlation_id: event.correlation_id,
        causation_id: event.causation_id,
        recorded_at: event.recorded_at,
    };
    let adapter = SqliteDelegationProfileStore::new_with_context(state.store.clone(), context).map_err(domain_error)?;
    let result = DelegationProfileService::new(adapter).execute(&DelegationProfileCommandScope {
        principal_id: state.principal_id.clone(), workspace_id: workspace.workspace_id.clone(), request_id,
    }, command).map_err(domain_error)?;
    // The command receipt includes the exact revision. Reauthorize after commit before
    // returning a Workspace-scoped projection.
    let current_workspace = ensure_workspace_owner(state, &workspace.workspace_id)?;
    let current_binding = state.store.get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
        owner_principal_id: state.principal_id.clone(), workspace_id: workspace.workspace_id.clone(),
        runtime_id: state.runtime_id.clone(), runtime_incarnation_id: state.local_incarnation_id.clone(),
    }).map_err(|_| unavailable())?;
    if current_workspace.status != "ACTIVE" || !current_binding.is_some_and(|binding| binding.status == "ACTIVE") {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Local Runtime Workspace access is unavailable"));
    }
    let mut response = (response_status, Json(profile_view(&result)?)).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    if let Ok(value) = header::HeaderValue::from_str(&response_correlation) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn create_delegation_profile(State(state): State<ApiState>, headers: HeaderMap, body: Bytes) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: CreateBody = parse_body(&body)?;
    if body.workspace_id != workspace.workspace_id || !valid_id(&body.agent_binding_id) { return Err(invalid()); }
    let request_id = request_key(&headers)?;
    let profile_id = idempotent_id("dp", &state.principal_id, &request_id, &format!("delegation-profile.create.{}", workspace.workspace_id));
    execute_command(&state, &workspace, request_id, DelegationProfileCommand::Create {
        profile_id, agent_binding_id: body.agent_binding_id, revision: revision_input(body.revision)?,
    }, StatusCode::CREATED)
}

async fn revise_delegation_profile(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>, body: Bytes) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let request_id = request_key(&headers)?;
    let expected = expected_version(&headers)?;
    let body: Value = parse_body(&body)?;
    execute_command(&state, &workspace, request_id, DelegationProfileCommand::Revise { profile_id: id, expected_version: expected, revision: revision_input(body)? }, StatusCode::CREATED)
}

async fn duplicate_delegation_profile(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>, body: Bytes) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let request_id = request_key(&headers)?;
    let expected = expected_version(&headers)?;
    let body: DuplicateBody = parse_body(&body)?;
    let profile_id = idempotent_id("dp", &state.principal_id, &request_id, &format!("delegation-profile.duplicate.{}", workspace.workspace_id));
    execute_command(&state, &workspace, request_id, DelegationProfileCommand::Duplicate { source_profile_id: id, expected_version: expected, profile_id, name: body.name }, StatusCode::CREATED)
}

async fn change_delegation_profile_status(State(state): State<ApiState>, headers: HeaderMap, Path(id): Path<String>, body: Bytes) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) { return Err(invalid()); }
    let request_id = request_key(&headers)?;
    let expected = expected_version(&headers)?;
    let body: StatusBody = parse_body(&body)?;
    execute_command(&state, &workspace, request_id, DelegationProfileCommand::SetStatus { profile_id: id, expected_version: expected, status: body.status }, StatusCode::OK)
}
