//! Authenticated local Operator routes for saved Routine definitions.
//!
//! These routes persist reusable definitions and immutable revisions, and support a
//! manual save-only Task materialization. They do not start planning or trigger execution;
//! Automation remains a separate aggregate.

use super::*;
use domain_responsibility::{
    Routine, RoutineCommand, RoutineError, RoutineOwnerScope, RoutineRevision,
    RoutineRevisionInput, RoutineService, materialize_routine_inputs,
};
use domain_task::{CreateStandaloneTask, TaskService};
use storage_core::{RoutineTaskAdmission, TaskStore, WorkspaceCreateRequest};
use storage_sqlite::{RoutineEventContext, SqliteRoutineStore};

const MAX_ROUTINE_BODY_BYTES: usize = 128 * 1024;
const ROUTINE_PAGE_SIZE: usize = 50;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/routines", get(list_routines).post(create_routine))
        .route("/v1/routines/{routine_id}", get(get_routine))
        .route(
            "/v1/routines/{routine_id}/revisions",
            get(list_revisions).post(revise_routine),
        )
        .route("/v1/routines/{routine_id}/run", post(run_routine))
        .route("/v1/routines/{routine_id}/archive", post(archive_routine))
        .layer(DefaultBodyLimit::max(MAX_ROUTINE_BODY_BYTES))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutineQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutineCursor {
    version: u32,
    workspace_id: String,
    updated_at: String,
    routine_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutineRevisionCursor {
    version: u32,
    workspace_id: String,
    routine_id: String,
    after_revision: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRoutineBody {
    workspace_id: String,
    name: String,
    revision: RoutineRevisionInput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviseRoutineBody {
    revision: RoutineRevisionInput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunRoutineBody {
    routine_revision: u64,
    inputs: serde_json::Value,
    conversation_id: Option<String>,
}

#[derive(Serialize)]
struct RoutinePageResponse {
    items: Vec<Routine>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct RoutineRevisionPageResponse {
    items: Vec<RoutineRevision>,
    next_cursor: Option<String>,
}

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Routine request is invalid",
    )
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Routine storage is unavailable",
    )
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

fn domain_error(error: RoutineError) -> Response {
    match error {
        RoutineError::NotFound => {
            operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Routine is unavailable")
        }
        RoutineError::Unauthorized => operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ),
        RoutineError::AlreadyExists
        | RoutineError::VersionConflict
        | RoutineError::IdempotencyConflict => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Routine version or idempotency key conflicts with current state",
        ),
        RoutineError::Archived => operator_error(
            StatusCode::CONFLICT,
            "ROUTINE_ARCHIVED",
            "Archived Routines cannot be revised",
        ),
        RoutineError::ArchiveBlocked => operator_error(
            StatusCode::CONFLICT,
            "ROUTINE_ARCHIVE_BLOCKED",
            "Pause, disable, or rebind enabled Automations before archiving this Routine",
        ),
        RoutineError::WorkspaceArchived => operator_error(
            StatusCode::CONFLICT,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces are read-only",
        ),
        RoutineError::InvalidDefinition => invalid(),
        RoutineError::VersionOverflow | RoutineError::RevisionOverflow | RoutineError::Storage => {
            unavailable()
        }
    }
}

fn authorized_workspace(state: &ApiState, headers: &HeaderMap) -> Result<Workspace, Response> {
    let workspace_id = selected_workspace(headers)?;
    let workspace = ensure_workspace_owner(state, &workspace_id)?;
    let binding = state
        .store
        .get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
            owner_principal_id: state.principal_id.clone(),
            workspace_id,
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
    Ok(workspace)
}

fn adapter(state: &ApiState, correlation_id: &str) -> Result<SqliteRoutineStore, Response> {
    let event = event_context(&state.runtime_id, correlation_id).map_err(|_| unavailable())?;
    SqliteRoutineStore::new(
        state.store.clone(),
        RoutineEventContext {
            event_id: event.event_id,
            origin_runtime_id: event.origin_runtime_id,
            hlc_timestamp: event.hlc_timestamp,
            correlation_id: event.correlation_id,
            causation_id: event.causation_id,
            recorded_at: event.recorded_at,
        },
    )
    .map_err(domain_error)
}

fn parse_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    if body.len() > MAX_ROUTINE_BODY_BYTES {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Routine request exceeds the local body limit",
        ));
    }
    serde_json::from_slice(body).map_err(|_| invalid())
}

fn idempotency_key(headers: &HeaderMap) -> Result<String, Response> {
    headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| byte.is_ascii_graphic())
        })
        .map(str::to_owned)
        .ok_or_else(invalid)
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

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}

fn decode_routine_cursor(encoded: &str, workspace: &str) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: RoutineCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace
        || !valid_id(&cursor.routine_id)
        || cursor.updated_at.len() > 64
        || time::OffsetDateTime::parse(
            &cursor.updated_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return Err(invalid());
    }
    Ok((cursor.updated_at, cursor.routine_id))
}

fn decode_revision_cursor(
    encoded: &str,
    workspace: &str,
    routine_id: &str,
) -> Result<u64, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: RoutineRevisionCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace
        || cursor.routine_id != routine_id
        || cursor.after_revision == 0
    {
        return Err(invalid());
    }
    Ok(cursor.after_revision)
}

fn execute(
    state: &ApiState,
    workspace: &Workspace,
    request_id: String,
    command: RoutineCommand,
) -> Result<(Routine, String), Response> {
    let correlation = idempotent_id("cor", &state.principal_id, &request_id, "correlation");
    let store = adapter(state, &correlation)?;
    let routine = RoutineService::new(store)
        .execute(
            &RoutineOwnerScope {
                principal_id: state.principal_id.clone(),
                workspace_id: workspace.workspace_id.clone(),
                request_id,
            },
            command,
        )
        .map_err(domain_error)?;
    ensure_workspace_owner(state, &workspace.workspace_id)?;
    Ok((routine, correlation))
}

async fn list_routines(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<RoutineQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(ROUTINE_PAGE_SIZE);
    if !(1..=200).contains(&limit) {
        return Err(invalid());
    }
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| decode_routine_cursor(cursor, &workspace.workspace_id))
        .transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let page = store
        .list(
            &state.principal_id,
            &workspace.workspace_id,
            None,
            after,
            limit,
        )
        .map_err(domain_error)?;
    let next_cursor = page
        .next
        .map(|(updated_at, routine_id)| {
            serde_json::to_vec(&RoutineCursor {
                version: 1,
                workspace_id: workspace.workspace_id.clone(),
                updated_at,
                routine_id,
            })
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        })
        .transpose()
        .map_err(|_| unavailable())?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    Ok(no_store(
        Json(RoutinePageResponse {
            items: page.items.into_iter().map(|(routine, _)| routine).collect(),
            next_cursor,
        })
        .into_response(),
    ))
}

async fn get_routine(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(routine_id): Path<String>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&routine_id) {
        return Err(invalid());
    }
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let (routine, _) = store
        .get(&state.principal_id, &workspace.workspace_id, &routine_id)
        .map_err(domain_error)?
        .ok_or_else(|| domain_error(RoutineError::NotFound))?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    Ok(no_store(Json(routine).into_response()))
}

async fn list_revisions(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(routine_id): Path<String>,
    query: Result<
        Query<std::collections::HashMap<String, String>>,
        axum::extract::rejection::QueryRejection,
    >,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&routine_id) {
        return Err(invalid());
    }
    let Query(query) = query.map_err(|_| invalid())?;
    if query.len() > 1 || query.keys().any(|key| key != "cursor") {
        return Err(invalid());
    }
    let after = query
        .get("cursor")
        .map(|cursor| decode_revision_cursor(cursor, &workspace.workspace_id, &routine_id))
        .transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    store
        .get(&state.principal_id, &workspace.workspace_id, &routine_id)
        .map_err(domain_error)?
        .ok_or_else(|| domain_error(RoutineError::NotFound))?;
    let page = store
        .list_revisions(
            &state.principal_id,
            &workspace.workspace_id,
            &routine_id,
            after,
            ROUTINE_PAGE_SIZE,
        )
        .map_err(domain_error)?;
    let next_cursor = page
        .next
        .map(|after_revision| {
            serde_json::to_vec(&RoutineRevisionCursor {
                version: 1,
                workspace_id: workspace.workspace_id.clone(),
                routine_id: routine_id.clone(),
                after_revision,
            })
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        })
        .transpose()
        .map_err(|_| unavailable())?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    Ok(no_store(
        Json(RoutineRevisionPageResponse {
            items: page.items,
            next_cursor,
        })
        .into_response(),
    ))
}

async fn create_routine(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: CreateRoutineBody = parse_body(&body)?;
    if body.workspace_id != workspace.workspace_id {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Routine Workspace does not match the selected Workspace",
        ));
    }
    if body.name.trim().is_empty() || body.name.chars().count() > 120 {
        return Err(invalid());
    }
    let request_id = idempotency_key(&headers)?;
    let routine_id = idempotent_id(
        "routine",
        &state.principal_id,
        &request_id,
        &format!("routine.create.{}", workspace.workspace_id),
    );
    let (routine, correlation) = execute(
        &state,
        &workspace,
        request_id,
        RoutineCommand::Create {
            routine_id,
            name: body.name,
            revision: body.revision,
        },
    )?;
    let mut response = (StatusCode::CREATED, Json(routine)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(value) = header::HeaderValue::from_str(&correlation) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn revise_routine(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(routine_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&routine_id) {
        return Err(invalid());
    }
    let input: ReviseRoutineBody = parse_body(&body)?;
    let (routine, correlation) = execute(
        &state,
        &workspace,
        idempotency_key(&headers)?,
        RoutineCommand::Revise {
            routine_id: routine_id.clone(),
            expected_version: expected_version(&headers)?,
            revision: input.revision,
        },
    )?;
    let store = adapter(&state, &correlation)?;
    let revision = store
        .get_revision(
            &state.principal_id,
            &workspace.workspace_id,
            &routine_id,
            routine.current_revision,
        )
        .map_err(domain_error)?
        .ok_or_else(unavailable)?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    let mut response = (StatusCode::CREATED, Json(revision)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(value) = header::HeaderValue::from_str(&correlation) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn archive_routine(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(routine_id): Path<String>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&routine_id) {
        return Err(invalid());
    }
    let (routine, correlation) = execute(
        &state,
        &workspace,
        idempotency_key(&headers)?,
        RoutineCommand::Archive {
            routine_id,
            expected_version: expected_version(&headers)?,
        },
    )?;
    let mut response = (StatusCode::OK, Json(routine)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(value) = header::HeaderValue::from_str(&correlation) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

/// Materializes exactly one active Routine revision into an ordinary saved READY Task.
/// This route never dispatches planning or creates execution records.
async fn run_routine(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(routine_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if workspace.status != "ACTIVE" {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces cannot accept new Tasks",
        ));
    }
    if !valid_id(&routine_id) {
        return Err(invalid());
    }
    let body: RunRoutineBody = parse_body(&body)?;
    if body.routine_revision == 0 || body.conversation_id.is_some() || !body.inputs.is_object() {
        return Err(operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ARGUMENT",
            "Routine inputs or origin are not supported for this run",
        ));
    }
    let request_id = idempotency_key(&headers)?;
    let idempotency_scope = format!("routine.run.{}.{}", workspace.workspace_id, routine_id);
    let task_id = idempotent_id("task", &state.principal_id, &request_id, &idempotency_scope);
    let correlation = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        &format!("{idempotency_scope}.correlation"),
    );
    let mut event = event_context(&state.runtime_id, &correlation).map_err(|_| unavailable())?;
    event.event_id = idempotent_id(
        "ev",
        &state.principal_id,
        &request_id,
        &format!("{idempotency_scope}.event"),
    );

    let routine_store = adapter(&state, &correlation)?;
    let revision = routine_store
        .get_revision(
            &state.principal_id,
            &workspace.workspace_id,
            &routine_id,
            body.routine_revision,
        )
        .map_err(domain_error)?
        .ok_or_else(|| domain_error(RoutineError::NotFound))?;
    let materialized = materialize_routine_inputs(&revision, &body.inputs, &workspace.workspace_id)
        .map_err(|_| {
            operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_ARGUMENT",
                "Routine inputs do not match the saved input schema and bindings",
            )
        })?;
    let lead_binding_id = revision
        .definition
        .preferred_agent_binding_id
        .clone()
        .or_else(|| workspace.default_agent_binding_id.clone())
        .ok_or_else(|| {
            operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "AGENT_UNAVAILABLE",
                "Choose an enabled lead Agent before running this Routine",
            )
        })?;
    let request_payload = json!({
        "workspace_id": workspace.workspace_id,
        "conversation_id": null,
        "source_message_refs": [],
        "objective": materialized.objective,
        "constraints": materialized.constraints,
        "non_goals": materialized.non_goals,
        "input_refs": materialized.input_refs,
        "required_outputs": materialized.required_outputs,
        "acceptance_criteria": materialized.acceptance_criteria,
        "approvals_required": materialized.approvals_required,
        "budget": materialized.budget_ceiling,
        "placement_preference": materialized.placement_preference,
        "preferred_lead_agent_binding_id": lead_binding_id.clone(),
        "lead_failover_policy": {
            "mode": "DISABLED",
            "triggers": [],
            "fallback_agent_binding_ids": [],
            "max_lead_changes": 0
        },
        "routine_id": routine_id,
        "routine_revision": body.routine_revision,
        "routine_inputs": body.inputs.clone(),
    });
    let command = CreateStandaloneTask {
        task_id,
        workspace_id: workspace.workspace_id.clone(),
        workspace_instruction_revision: workspace.current_instruction_revision,
        lead_agent_binding_id: lead_binding_id.clone(),
        origin_coworker_id: None,
        origin_coworker_revision: None,
        expected_coworker_version: None,
        coworker_default_lead_failover_policy: None,
        principal_id: state.principal_id.clone(),
        request_id,
        request_payload,
        event: EventContext {
            event_id: event.event_id,
            origin_runtime_id: event.origin_runtime_id,
            hlc_timestamp: event.hlc_timestamp,
            correlation_id: event.correlation_id,
            causation_id: event.causation_id,
            recorded_at: event.recorded_at,
        },
    };
    let mut commit = TaskService::new(state.store.clone())
        .prepare_standalone(command)
        .map_err(|_| {
            operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_ARGUMENT",
                "Routine could not be materialized as a Task",
            )
        })?;
    // Bind idempotency to the authenticated owner's exact RunRoutine command rather
    // than mutable defaults resolved while materializing the ordinary Task envelope.
    commit.idempotency_payload = Some(json!({
        "workspace_id": workspace.workspace_id,
        "routine_id": routine_id,
        "routine_revision": body.routine_revision,
        "inputs": body.inputs.clone(),
        "conversation_id": null,
    }));
    commit.task.routine_id = Some(routine_id.clone());
    commit.task.routine_revision = Some(body.routine_revision);
    commit.routine_admission = Some(RoutineTaskAdmission {
        routine_id: routine_id.clone(),
        routine_revision: body.routine_revision,
        inputs: body.inputs.clone(),
    });
    commit.event.payload["routine_id"] = json!(routine_id);
    commit.event.payload["routine_revision"] = serde_json::json!(body.routine_revision);
    let store = state.store.clone();
    let committed = tokio::task::spawn_blocking(move || store.create_task(commit))
        .await
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Routine Task admission did not complete",
            )
        })?
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_ARGUMENT",
                "Routine Task inputs, policy, or lead are no longer eligible",
            ),
            storage_core::StoreError::Conflict { .. } => operator_error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Routine changed while the run was being admitted; refresh and retry",
            ),
            storage_core::StoreError::NotFound => operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_ARGUMENT",
                "A Routine input Resource is unavailable in this Workspace",
            ),
            _ => unavailable(),
        })?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    let mut response = (StatusCode::CREATED, Json(committed.view)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}
