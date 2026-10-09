//! Authenticated local Operator routes for passive, revisioned Goals.
//! Goal commands record intent and references only; they never schedule work.

use super::*;
use domain_responsibility::{
    Goal, GoalCommand, GoalError, GoalOwnerScope, GoalProgressProjection, GoalRevisionInput,
    GoalService, GoalStatus,
};
use storage_sqlite::{GoalEventContext, SqliteGoalStore};

const MAX_GOAL_BODY_BYTES: usize = 64 * 1024;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/goals", get(list_goals).post(create_goal))
        .route("/v1/goals/{goal_id}", get(get_goal))
        .route("/v1/goals/{goal_id}/revisions", post(revise_goal))
        .route("/v1/goals/{goal_id}/status", post(change_goal_status))
        .layer(DefaultBodyLimit::max(MAX_GOAL_BODY_BYTES))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalQuery {
    coworker_id: Option<String>,
    status: Option<GoalStatus>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalCursor {
    version: u32,
    workspace_id: String,
    coworker_id: Option<String>,
    status: Option<GoalStatus>,
    updated_at: String,
    goal_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateGoalBody {
    workspace_id: String,
    coworker_id: Option<String>,
    revision: GoalRevisionInput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalStatusBody {
    status: GoalStatus,
}

#[derive(Serialize)]
struct GoalView {
    goal_id: String,
    workspace_id: String,
    coworker_id: Option<String>,
    current_revision: u64,
    revision: GoalRevisionInput,
    status: GoalStatus,
    /// Read-only Task/Evidence projection; unavailable dimensions are explicitly null.
    progress: Option<GoalProgressProjection>,
    created_at: String,
    updated_at: String,
    version: u64,
}

#[derive(Serialize)]
struct GoalPageResponse {
    items: Vec<GoalView>,
    next_cursor: Option<String>,
}

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Goal request is invalid",
    )
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Goal storage is unavailable",
    )
}

fn domain_error(error: GoalError) -> Response {
    match error {
        GoalError::NotFound => {
            operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Goal is unavailable")
        }
        GoalError::Unauthorized => operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ),
        GoalError::AlreadyExists | GoalError::VersionConflict | GoalError::IdempotencyConflict => {
            operator_error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Goal version or idempotency key conflicts with current state",
            )
        }
        GoalError::Archived => operator_error(
            StatusCode::CONFLICT,
            "GOAL_ARCHIVED",
            "Archived Goals cannot be changed",
        ),
        GoalError::InvalidTransition => operator_error(
            StatusCode::CONFLICT,
            "INVALID_TRANSITION",
            "Goal status transition is not allowed",
        ),
        GoalError::ReferenceUnavailable => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "A linked Task, Routine revision, Artifact version, or Coworker is unavailable",
        ),
        GoalError::WorkspaceArchived => operator_error(
            StatusCode::CONFLICT,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces are read-only",
        ),
        GoalError::InvalidDefinition => invalid(),
        GoalError::VersionOverflow | GoalError::RevisionOverflow | GoalError::Storage => {
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

fn adapter(state: &ApiState, correlation_id: &str) -> Result<SqliteGoalStore, Response> {
    let event = event_context(&state.runtime_id, correlation_id).map_err(|_| unavailable())?;
    SqliteGoalStore::new(
        state.store.clone(),
        GoalEventContext {
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

fn goal_view(
    goal: Goal,
    revision: domain_responsibility::GoalRevision,
    workspace_id: &str,
    progress: Option<GoalProgressProjection>,
) -> Result<GoalView, Response> {
    if goal.workspace_id != workspace_id
        || goal.goal_id != revision.goal_id
        || goal.current_revision != revision.revision
    {
        return Err(unavailable());
    }
    Ok(GoalView {
        goal_id: goal.goal_id,
        workspace_id: goal.workspace_id,
        coworker_id: goal.coworker_id,
        current_revision: goal.current_revision,
        revision: revision.definition,
        status: goal.status,
        progress,
        created_at: goal.created_at,
        updated_at: goal.updated_at,
        version: goal.version,
    })
}

fn parse_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    if body.len() > MAX_GOAL_BODY_BYTES {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Goal request exceeds the local body limit",
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

fn decode_cursor(
    encoded: &str,
    workspace: &str,
    coworker: Option<&str>,
    status: Option<GoalStatus>,
) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: GoalCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace
        || cursor.coworker_id.as_deref() != coworker
        || cursor.status != status
        || !valid_id(&cursor.goal_id)
        || cursor.updated_at.len() > 64
        || time::OffsetDateTime::parse(
            &cursor.updated_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return Err(invalid());
    }
    Ok((cursor.updated_at, cursor.goal_id))
}

fn execute(
    state: &ApiState,
    workspace: &Workspace,
    request_id: String,
    command: GoalCommand,
    response_status: StatusCode,
) -> Result<Response, Response> {
    let correlation = idempotent_id("cor", &state.principal_id, &request_id, "correlation");
    let mut store = adapter(state, &correlation)?;
    let goal = GoalService::new(store.clone())
        .execute(
            &GoalOwnerScope {
                principal_id: state.principal_id.clone(),
                workspace_id: workspace.workspace_id.clone(),
                request_id,
            },
            command,
        )
        .map_err(domain_error)?;
    let revision = store
        .get_revision(
            &state.principal_id,
            &workspace.workspace_id,
            &goal.goal_id,
            goal.current_revision,
        )
        .map_err(domain_error)?
        .ok_or_else(unavailable)?;
    let progress = store
        .progress(
            &state.principal_id,
            &workspace.workspace_id,
            &goal.goal_id,
            goal.current_revision,
        )
        .map_err(domain_error)?;
    // Recheck the current owner after command commit before disclosing the receipt.
    ensure_workspace_owner(state, &workspace.workspace_id)?;
    let mut response = (
        response_status,
        Json(goal_view(
            goal,
            revision,
            &workspace.workspace_id,
            progress,
        )?),
    )
        .into_response();
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

async fn list_goals(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<GoalQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) || query.coworker_id.as_deref().is_some_and(|id| !valid_id(id)) {
        return Err(invalid());
    }
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| {
            decode_cursor(
                cursor,
                &workspace.workspace_id,
                query.coworker_id.as_deref(),
                query.status,
            )
        })
        .transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let page = store
        .list(
            &state.principal_id,
            &workspace.workspace_id,
            query.coworker_id.as_deref(),
            query.status,
            after,
            limit,
        )
        .map_err(domain_error)?;
    let items = page
        .items
        .into_iter()
        .map(|(goal, revision)| {
            let progress = store
                .progress(
                    &state.principal_id,
                    &workspace.workspace_id,
                    &goal.goal_id,
                    goal.current_revision,
                )
                .map_err(domain_error)?;
            goal_view(goal, revision, &workspace.workspace_id, progress)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    let next_cursor = page
        .next
        .map(|(updated_at, goal_id)| {
            serde_json::to_vec(&GoalCursor {
                version: 1,
                workspace_id: workspace.workspace_id.clone(),
                coworker_id: query.coworker_id.clone(),
                status: query.status,
                updated_at,
                goal_id,
            })
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        })
        .transpose()
        .map_err(|_| unavailable())?;
    let mut response = Json(GoalPageResponse { items, next_cursor }).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

async fn get_goal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(goal_id): Path<String>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&goal_id) {
        return Err(invalid());
    }
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let (goal, revision) = store
        .get(&state.principal_id, &workspace.workspace_id, &goal_id)
        .map_err(domain_error)?
        .ok_or_else(|| domain_error(GoalError::NotFound))?;
    let progress = store
        .progress(
            &state.principal_id,
            &workspace.workspace_id,
            &goal_id,
            goal.current_revision,
        )
        .map_err(domain_error)?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    Ok(Json(goal_view(
        goal,
        revision,
        &workspace.workspace_id,
        progress,
    )?)
    .into_response())
}

async fn create_goal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: CreateGoalBody = parse_body(&body)?;
    if body.workspace_id != workspace.workspace_id {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Goal Workspace does not match the selected Workspace",
        ));
    }
    let request_id = idempotency_key(&headers)?;
    let goal_id = idempotent_id(
        "goal",
        &state.principal_id,
        &request_id,
        &format!("goal.create.{}", workspace.workspace_id),
    );
    execute(
        &state,
        &workspace,
        request_id,
        GoalCommand::Create {
            goal_id,
            coworker_id: body.coworker_id,
            revision: body.revision,
        },
        StatusCode::CREATED,
    )
}

async fn revise_goal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(goal_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&goal_id) {
        return Err(invalid());
    }
    let revision: GoalRevisionInput = parse_body(&body)?;
    execute(
        &state,
        &workspace,
        idempotency_key(&headers)?,
        GoalCommand::Revise {
            goal_id,
            expected_version: expected_version(&headers)?,
            revision,
        },
        StatusCode::CREATED,
    )
}

async fn change_goal_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(goal_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&goal_id) {
        return Err(invalid());
    }
    let input: GoalStatusBody = parse_body(&body)?;
    execute(
        &state,
        &workspace,
        idempotency_key(&headers)?,
        GoalCommand::SetStatus {
            goal_id,
            expected_version: expected_version(&headers)?,
            status: input.status,
        },
        StatusCode::OK,
    )
}
