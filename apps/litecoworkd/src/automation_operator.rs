//! Authenticated local Operator routes for stored Automation definitions.
//!
//! This router exposes the finite definition API and safe-stop commands over persisted
//! aggregates. Local owner ManualTrigger runs atomically admit a claimed occurrence and
//! an ordinary READY Task; recurring trigger hosting and `/resume` remain unavailable.
// Route handlers intentionally return full Axum Responses to preserve status, body, and headers.
#![allow(clippy::result_large_err)]

use super::*;
use domain_responsibility::{
    Automation, AutomationDefinition, AutomationRevision, AutomationStatus,
    CommittedResponsibility, DomainError, HostConcurrencyFacts, HostSlotDecision,
    OccurrenceIdentity, OwnerCommandScope, ResponsibilityCommand, ResponsibilityService,
    SettlementWindow, SlotOutcomeReason, TestRunIdentity, TriggerDefinition,
    materialize_routine_inputs, plan_host_settlement,
};
use domain_task::{CreateStandaloneTask, TaskService};
use storage_core::{
    AutomationTaskAdmission, AutomationTestAdmission, RoutineTaskAdmission,
    RuntimeWorkspaceBindingStore, TaskStore,
};
use storage_sqlite::{
    AutomationHostContext, AutomationRevisionPage, CoworkerEventContext, DueSchedule,
    RoutineEventContext, SqliteAutomationHostStore, SqliteCoworkerStore, SqliteRoutineStore,
    TriggerHostWorkspace,
};

const MAX_AUTOMATION_BODY_BYTES: usize = 128 * 1024;

/// How late a due slot may be and still count as an ordinary on-time run.
const TRIGGER_HOST_GRACE_MS: i64 = 5 * 60 * 1000;

pub(super) fn router() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/automations",
            get(list_automations).post(create_automation),
        )
        .route(
            "/v1/automations/{automation_id}",
            get(get_automation).patch(revise_automation),
        )
        .route(
            "/v1/automations/{automation_id}/revisions",
            get(list_automation_revisions),
        )
        .route(
            "/v1/automations/{automation_id}/occurrences",
            get(list_automation_occurrences),
        )
        .route(
            "/v1/automations/{automation_id}/health",
            get(get_automation_health),
        )
        .route(
            "/v1/automations/{automation_id}/pause",
            post(pause_automation),
        )
        .route(
            "/v1/automations/{automation_id}/disable",
            post(disable_automation),
        )
        .route(
            "/v1/automations/{automation_id}/run",
            post(run_automation_now),
        )
        .route(
            "/v1/automations/{automation_id}/test-runs",
            post(run_automation_test),
        )
        .route(
            "/v1/automations/host/settle",
            post(settle_automation_triggers),
        )
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
struct AutomationOccurrenceQuery {
    cursor: Option<String>,
    limit: Option<usize>,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunAutomationBody {
    automation_revision: u64,
    inputs: serde_json::Value,
}

#[derive(Serialize)]
struct ManualAutomationRunResponse {
    automation_id: String,
    automation_revision: u64,
    trigger_id: String,
    occurrence_id: String,
    occurrence_version: u64,
    occurrence_status: &'static str,
    task: storage_core::TaskView,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunAutomationTestBody {
    automation_revision: u64,
    inputs: serde_json::Value,
}

#[derive(Serialize)]
struct AutomationTestRunResponse {
    test_run_id: String,
    automation_id: String,
    automation_revision: u64,
    task: storage_core::TaskView,
}

fn parse_coworker_ref(
    value: serde_json::Value,
) -> Result<Option<domain_responsibility::CoworkerRevisionRef>, Response> {
    if value.is_null() {
        return Ok(None);
    }
    let Some(object) = value.as_object() else {
        return Err(invalid());
    };
    let Some(coworker_id) = object
        .get("coworker_id")
        .and_then(serde_json::Value::as_str)
    else {
        return Err(invalid());
    };
    let Some(revision) = object.get("revision").and_then(serde_json::Value::as_u64) else {
        return Err(invalid());
    };
    if object.len() != 2 || !valid_id(coworker_id) || revision == 0 {
        return Err(invalid());
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|_| invalid())
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
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Automation request is invalid",
    )
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Automation storage is unavailable",
    )
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

fn domain_error(error: DomainError) -> Response {
    match error {
        DomainError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Automation or pinned dependency is unavailable",
        ),
        DomainError::Unauthorized => operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ),
        DomainError::AlreadyExists
        | DomainError::VersionConflict
        | DomainError::IdempotencyConflict
        | DomainError::TriggerSourceChanged => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Automation version or idempotency key conflicts with current state",
        ),
        DomainError::AutomationDisabled => operator_error(
            StatusCode::CONFLICT,
            "AUTOMATION_DISABLED",
            "Disabled Automations cannot be changed",
        ),
        DomainError::AutomationNotPaused => operator_error(
            StatusCode::CONFLICT,
            "AUTOMATION_NOT_PAUSED",
            "Pause this Automation before changing its definition",
        ),
        DomainError::RoutineArchived => operator_error(
            StatusCode::CONFLICT,
            "ROUTINE_ARCHIVED",
            "The pinned Routine is unavailable for activation",
        ),
        DomainError::CoworkerInactive => operator_error(
            StatusCode::CONFLICT,
            "COWORKER_PAUSED",
            "The pinned Coworker is paused",
        ),
        DomainError::ReconciliationRequired => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "Automation activation requires a trigger host and occurrence reconciliation service that is not available",
        ),
        DomainError::TriggerUnsupported => operator_error(
            StatusCode::BAD_REQUEST,
            "TRIGGER_UNSUPPORTED",
            "This trigger type is not supported by the local Automation service",
        ),
        DomainError::WorkspaceArchived => operator_error(
            StatusCode::CONFLICT,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces are read-only",
        ),
        DomainError::InvalidDefinition => invalid(),
        DomainError::VersionOverflow
        | DomainError::Storage
        | DomainError::CoworkerArchived
        | DomainError::ArchiveBlocked => unavailable(),
    }
}

fn adapter(state: &ApiState, correlation_id: &str) -> Result<SqliteCoworkerStore, Response> {
    let event = event_context(&state.runtime_id, correlation_id).map_err(|_| unavailable())?;
    SqliteCoworkerStore::new(
        state.store.clone(),
        CoworkerEventContext {
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

fn automation_view(automation: Automation, workspace_id: &str) -> Result<Automation, Response> {
    if automation.workspace_id != workspace_id
        || automation.current_revision == 0
        || automation.version == 0
    {
        return Err(unavailable());
    }
    Ok(automation)
}

fn parse_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    if body.len() > MAX_AUTOMATION_BODY_BYTES {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Automation request exceeds the local body limit",
        ));
    }
    serde_json::from_slice(body).map_err(|_| invalid())
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

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}

fn decode_cursor(encoded: &str, workspace: &str) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: AutomationCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace
        || !valid_id(&cursor.automation_id)
        || cursor.updated_at.len() > 64
        || time::OffsetDateTime::parse(
            &cursor.updated_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return Err(invalid());
    }
    Ok((cursor.updated_at, cursor.automation_id))
}

fn decode_revision_cursor(
    encoded: &str,
    workspace: &str,
    automation_id: &str,
) -> Result<u64, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: AutomationRevisionCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace
        || cursor.automation_id != automation_id
        || cursor.before_revision == 0
    {
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
    let result = ResponsibilityService::new(store.clone())
        .execute(
            &OwnerCommandScope {
                principal_id: state.principal_id.clone(),
                workspace_id: workspace.workspace_id.clone(),
                request_id,
            },
            command,
        )
        .map_err(domain_error)?;
    let CommittedResponsibility::Automation(head) = result else {
        return Err(unavailable());
    };
    // Reads and replay reconstruct the exact revision pinned by this command result.
    let revision = store
        .get_automation_revision(
            &state.principal_id,
            &workspace.workspace_id,
            &head.automation_id,
            head.current_revision,
        )
        .map_err(domain_error)?
        .ok_or_else(unavailable)?;
    if revision.revision != head.current_revision {
        return Err(unavailable());
    }
    ensure_workspace_owner(state, &workspace.workspace_id)?;
    let mut response = (
        response_status,
        Json(automation_view(head, &workspace.workspace_id)?),
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

async fn list_automations(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<AutomationQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(invalid());
    }
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| decode_cursor(cursor, &workspace.workspace_id))
        .transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let page = store
        .list_automations(
            &state.principal_id,
            &workspace.workspace_id,
            None,
            after,
            limit,
        )
        .map_err(domain_error)?;
    let items = page
        .items
        .into_iter()
        .map(|(head, revision)| {
            if revision.automation_id != head.automation_id
                || revision.revision != head.current_revision
            {
                return Err(unavailable());
            }
            automation_view(head, &workspace.workspace_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    let next_cursor = page
        .next
        .map(|(updated_at, automation_id)| {
            serde_json::to_vec(&AutomationCursor {
                version: 1,
                workspace_id: workspace.workspace_id.clone(),
                updated_at,
                automation_id,
            })
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        })
        .transpose()
        .map_err(|_| unavailable())?;
    let mut response = Json(AutomationPageResponse { items, next_cursor }).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

async fn get_automation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let (head, revision) = store
        .get_automation(&state.principal_id, &workspace.workspace_id, &id)
        .map_err(domain_error)?
        .ok_or_else(|| domain_error(DomainError::NotFound))?;
    if revision.automation_id != head.automation_id || revision.revision != head.current_revision {
        return Err(unavailable());
    }
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    Ok(no_store(
        Json(automation_view(head, &workspace.workspace_id)?).into_response(),
    ))
}

async fn list_automation_revisions(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    query: Result<Query<AutomationRevisionQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let Query(query) = query.map_err(|_| invalid())?;
    let before_revision = query
        .cursor
        .as_deref()
        .map(|cursor| decode_revision_cursor(cursor, &workspace.workspace_id, &id))
        .transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let page: AutomationRevisionPage = store
        .list_automation_revisions(
            &state.principal_id,
            &workspace.workspace_id,
            &id,
            before_revision,
            50,
        )
        .map_err(domain_error)?;
    if page
        .items
        .iter()
        .any(|revision| revision.automation_id != id || revision.revision == 0)
    {
        return Err(unavailable());
    }
    let next_cursor = page
        .next
        .map(|before_revision| {
            serde_json::to_vec(&AutomationRevisionCursor {
                version: 1,
                workspace_id: workspace.workspace_id.clone(),
                automation_id: id.clone(),
                before_revision,
            })
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        })
        .transpose()
        .map_err(|_| unavailable())?;
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    let mut response = Json(AutomationRevisionPageResponse {
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

/// One persisted occurrence in an Automation's run history.
#[derive(Serialize)]
struct AutomationOccurrenceView {
    occurrence_id: String,
    automation_id: String,
    automation_revision: u64,
    routine_id: String,
    routine_revision: u64,
    trigger_id: String,
    trigger_host_runtime_id: String,
    occurrence_key: String,
    version: u64,
    claim_epoch: u64,
    claim_expires_at: Option<String>,
    status: String,
    task_id: Option<String>,
    scheduled_for: Option<String>,
    covered_misfire_range: Option<serde_json::Value>,
    trigger_input_ref: Option<serde_json::Value>,
    trigger_payload_digest: Option<String>,
    blockers: Vec<serde_json::Value>,
    created_at: String,
    updated_at: String,
}

/// The bounded run history of one Automation.
#[derive(Serialize)]
struct AutomationOccurrencePageResponse {
    items: Vec<AutomationOccurrenceView>,
    next_cursor: Option<String>,
}

/// The opaque keyset cursor of one occurrence page.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutomationOccurrenceCursor {
    version: u32,
    workspace_id: String,
    automation_id: String,
    created_at: String,
    occurrence_id: String,
}

/// Reads the occurrence history of one Automation.
///
/// The history reports the persisted occurrence state and the ordinary Task it
/// links. A schedule that has no future slot is not evidence that the Tasks it
/// produced succeeded: the Task outcome stays a separate fact.
async fn list_automation_occurrences(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    query: Result<Query<AutomationOccurrenceQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(invalid());
    }
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| decode_occurrence_cursor(cursor, &workspace.workspace_id, &id))
        .transpose()?;
    let recorded_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| unavailable())?;
    let host = SqliteAutomationHostStore::new(
        state.store.clone(),
        AutomationHostContext {
            origin_runtime_id: state.runtime_id.clone(),
            recorded_at,
        },
    )
    .map_err(|_| unavailable())?;
    let rows = host
        .list_occurrence_page(&workspace.workspace_id, &id, after, limit + 1)
        .map_err(|_| unavailable())?;
    let more = rows.len() > limit;
    let items = rows
        .into_iter()
        .take(limit)
        .map(|item| AutomationOccurrenceView {
            occurrence_id: item.occurrence_id,
            automation_id: item.automation_id,
            automation_revision: item.automation_revision,
            routine_id: item.routine_id,
            routine_revision: item.routine_revision,
            trigger_id: item.trigger_id,
            trigger_host_runtime_id: item.trigger_host_runtime_id,
            occurrence_key: item.occurrence_key,
            version: item.version,
            claim_epoch: item.claim_epoch,
            claim_expires_at: item.claim_expires_at,
            status: item.status,
            task_id: item.task_id,
            scheduled_for: item.scheduled_for,
            covered_misfire_range: item.covered_misfire_range,
            trigger_input_ref: item.trigger_input_ref,
            trigger_payload_digest: item.trigger_payload_digest,
            blockers: item.blockers,
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
        .collect::<Vec<_>>();
    let next_cursor = if more {
        items
            .last()
            .map(|item| {
                serde_json::to_vec(&AutomationOccurrenceCursor {
                    version: 1,
                    workspace_id: workspace.workspace_id.clone(),
                    automation_id: id.clone(),
                    created_at: item.created_at.clone(),
                    occurrence_id: item.occurrence_id.clone(),
                })
                .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
            })
            .transpose()
            .map_err(|_| unavailable())?
    } else {
        None
    };
    let mut response = Json(AutomationOccurrencePageResponse { items, next_cursor }).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

/// Decodes the opaque occurrence page cursor of one Automation.
fn decode_occurrence_cursor(
    cursor: &str,
    workspace_id: &str,
    automation_id: &str,
) -> Result<(String, String), Response> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| invalid())?;
    let decoded: AutomationOccurrenceCursor =
        serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if decoded.version != 1
        || decoded.workspace_id != workspace_id
        || decoded.automation_id != automation_id
        || decoded.created_at.trim().is_empty()
        || decoded.occurrence_id.trim().is_empty()
    {
        return Err(invalid());
    }
    Ok((decoded.created_at, decoded.occurrence_id))
}

/// Reads the read-only health projection of one Automation.
///
/// The projection never repairs, never re-enables, and never advances a
/// cursor. It reports the host readiness, the occurrence counters, and the
/// drift state of the pinned Routine revision.
async fn get_automation_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    ensure_workspace_owner(&state, &workspace.workspace_id)?;
    let recorded_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| unavailable())?;
    let host = SqliteAutomationHostStore::new(
        state.store.clone(),
        AutomationHostContext {
            origin_runtime_id: state.runtime_id.clone(),
            recorded_at,
        },
    )
    .map_err(|_| unavailable())?;
    let facts = host
        .health_facts(&workspace.workspace_id, &id)
        .map_err(|_| unavailable())?
        .ok_or_else(|| domain_error(DomainError::NotFound))?;
    let projection = automation_health_projection(&facts)?;
    Ok(no_store(Json(projection).into_response()))
}

/// Builds the read-only health projection from the persisted facts.
///
/// The projection reports only what the durable records prove. A missing
/// observation is `UNKNOWN`, never `HEALTHY`, and a transient failure never
/// claims drift.
fn automation_health_projection(
    facts: &storage_sqlite::AutomationHealthFacts,
) -> Result<domain_responsibility::AutomationHealth, Response> {
    let status = match facts.status.as_str() {
        "ENABLED" => domain_responsibility::AutomationStatus::Enabled,
        "PAUSED" => domain_responsibility::AutomationStatus::Paused,
        "DISABLED" => domain_responsibility::AutomationStatus::Disabled,
        _ => return Err(unavailable()),
    };
    let routine_status = match facts.routine_status.as_str() {
        "ACTIVE" | "ARCHIVED" => facts.routine_status.clone(),
        _ => return Err(unavailable()),
    };
    let mut blockers: Vec<String> = Vec::new();
    if !facts.host_ready {
        blockers.push("TRIGGER_HOST_UNAVAILABLE".to_owned());
    }
    if routine_status != "ACTIVE" {
        blockers.push("ROUTINE_NOT_ACTIVE".to_owned());
    }
    if facts.routine_current_revision != facts.routine_revision {
        blockers.push("ROUTINE_REVISION_DRIFTED".to_owned());
    }
    let routine_health = routine_health_projection(facts)?;
    Ok(domain_responsibility::AutomationHealth {
        automation_id: facts.automation_id.clone(),
        current_revision: facts.current_revision,
        status,
        host_ready: facts.host_ready,
        blockers,
        next_occurrence_at: facts
            .next_occurrence_at
            .map(|value| domain_responsibility::format_instant(value))
            .transpose()
            .map_err(|_| unavailable())?,
        last_occurrence_at: facts
            .last_occurrence_at
            .map(|value| domain_responsibility::format_instant(value))
            .transpose()
            .map_err(|_| unavailable())?,
        nonterminal_occurrences: facts.nonterminal_occurrences,
        occurrence_counts: facts.counts,
        drift_state: routine_health.drift_state,
        drift_evidence_refs: routine_health.drift_evidence_refs.clone(),
        routine_health,
    })
}

/// Builds the read-only Routine health projection from the persisted facts.
///
/// The success rate uses the ten most recent terminal Tasks of the pinned
/// Routine revision. `CANCELLED` runs and nonterminal runs are excluded, and
/// fewer than three eligible runs are reported as "not enough runs yet"
/// instead of a percentage.
fn routine_health_projection(
    facts: &storage_sqlite::AutomationHealthFacts,
) -> Result<domain_responsibility::RoutineHealth, Response> {
    let mut eligible = 0_u32;
    let mut successes = 0_u32;
    let mut durations: Vec<u64> = Vec::new();
    let mut last_run_at: Option<String> = None;
    let mut last_success_at: Option<String> = None;
    for outcome in &facts.routine_tasks {
        match outcome.status.as_str() {
            "COMPLETED" => {
                eligible += 1;
                successes += 1;
                last_success_at = outcome.completed_at.clone().or(last_success_at);
                if let Some(duration) = task_duration_ms(outcome) {
                    durations.push(duration);
                }
            }
            "FAILED" | "INCOMPLETE" => {
                eligible += 1;
                if let Some(duration) = task_duration_ms(outcome) {
                    durations.push(duration);
                }
            }
            // CANCELLED runs are excluded from the rate and from the duration.
            _ => continue,
        }
        last_run_at = Some(
            outcome
                .completed_at
                .clone()
                .unwrap_or_else(|| outcome.created_at.clone()),
        );
    }
    let recent_success_rate = if eligible < 3 {
        None
    } else {
        Some(f64::from(successes) / f64::from(eligible))
    };
    let average_duration_ms = if durations.is_empty() {
        None
    } else {
        Some(
            durations
                .iter()
                .copied()
                .sum::<u64>()
                .checked_div(durations.len() as u64)
                .unwrap_or_default(),
        )
    };
    // A failed run is a warning. It never claims drift on its own, and the
    // absence of an observation never claims health.
    let transient_failures = facts
        .routine_tasks
        .iter()
        .filter(|outcome| matches!(outcome.status.as_str(), "FAILED" | "INCOMPLETE"))
        .count() as u32;
    let drift = domain_responsibility::evaluate_drift_state(&domain_responsibility::DriftInputs {
        explicit_drift_evidence: Vec::new(),
        transient_failures,
        dependencies: Vec::new(),
    });
    Ok(domain_responsibility::RoutineHealth {
        routine_id: facts.routine_id.clone(),
        current_revision: facts.routine_current_revision,
        last_run_at,
        last_success_at,
        recent_success_rate,
        sample_size: eligible,
        average_duration_ms,
        observed_costs: Vec::new(),
        required_dependency_health: Vec::new(),
        drift_state: drift,
        drift_evidence_refs: Vec::new(),
    })
}

/// The elapsed milliseconds of one terminal Task, when both instants parse.
fn task_duration_ms(outcome: &storage_sqlite::RoutineTaskOutcome) -> Option<u64> {
    let created = domain_responsibility::parse_instant(&outcome.created_at)?;
    let completed = domain_responsibility::parse_instant(outcome.completed_at.as_ref()?)?;
    u64::try_from(completed.checked_sub(created)?).ok()
}

async fn create_automation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: CreateAutomationBody = parse_body(&body)?;
    if body.workspace_id != workspace.workspace_id {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Automation Workspace does not match the selected Workspace",
        ));
    }
    let request_id = idempotency_key(&headers)?;
    let coworker_ref = parse_coworker_ref(body.coworker_ref.clone())?;
    let automation_id = idempotent_id(
        "aut",
        &state.principal_id,
        &request_id,
        &format!("automation.create.{}", workspace.workspace_id),
    );
    execute(
        &state,
        &workspace,
        request_id,
        ResponsibilityCommand::CreateAutomation {
            automation_id,
            name: body.name,
            definition: AutomationDefinition {
                routine_id: body.routine_id,
                routine_revision: body.routine_revision,
                triggers: body.triggers,
                execution_policy: body.execution_policy,
                coworker_ref,
            },
        },
        StatusCode::CREATED,
    )
}

async fn revise_automation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let body: ReviseAutomationBody = parse_body(&body)?;
    let definition = AutomationDefinition {
        routine_id: body.routine_id,
        routine_revision: body.routine_revision,
        triggers: body.triggers,
        execution_policy: body.execution_policy,
        coworker_ref: parse_coworker_ref(body.coworker_ref.clone())?,
    };
    execute(
        &state,
        &workspace,
        idempotency_key(&headers)?,
        ResponsibilityCommand::ReviseAutomation {
            automation_id: id,
            expected_version: expected_version(&headers)?,
            name: body.name,
            definition,
        },
        StatusCode::OK,
    )
}

async fn pause_automation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    change_status(&state, &headers, id, AutomationStatus::Paused).await
}

async fn disable_automation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    change_status(&state, &headers, id, AutomationStatus::Disabled).await
}

async fn change_status(
    state: &ApiState,
    headers: &HeaderMap,
    id: String,
    status: AutomationStatus,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(state, headers)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    execute(
        state,
        &workspace,
        idempotency_key(headers)?,
        ResponsibilityCommand::SetAutomationStatus {
            automation_id: id,
            expected_version: expected_version(headers)?,
            status,
        },
        StatusCode::OK,
    )
}

/// Owner Run now is a one-shot ManualTrigger. It does not enable the Automation or
/// start planning; storage commits occurrence claim, READY Task, event, and receipt
/// in the same transaction.
async fn run_automation_now(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let workspace = ensure_workspace_owner(&state, &workspace_id)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let body: RunAutomationBody = parse_body(&body)?;
    if body.automation_revision == 0 || !body.inputs.is_object() {
        return Err(operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ARGUMENT",
            "Manual Automation inputs or revision are invalid",
        ));
    }
    let expected_automation_version = expected_version(&headers)?;
    let request_id = idempotency_key(&headers)?;
    let scope = format!("automation.run.{}.{}", workspace.workspace_id, id);
    let task_id = idempotent_id("tsk", &state.principal_id, &request_id, &scope);
    let correlation = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        &format!("{scope}.correlation"),
    );
    let mut event = event_context(&state.runtime_id, &correlation).map_err(|_| unavailable())?;
    event.event_id = idempotent_id(
        "ev",
        &state.principal_id,
        &request_id,
        &format!("{scope}.event"),
    );

    let automation_store = adapter(&state, &correlation)?;
    let automation_revision = automation_store
        .get_automation_revision(
            &state.principal_id,
            &workspace.workspace_id,
            &id,
            body.automation_revision,
        )
        .map_err(domain_error)?
        .ok_or_else(|| domain_error(DomainError::NotFound))?;
    if automation_revision.automation_id != id
        || automation_revision.revision != body.automation_revision
    {
        return Err(unavailable());
    }
    let mut manual_triggers = automation_revision
        .definition
        .triggers
        .iter()
        .filter(|trigger| matches!(trigger.trigger, TriggerDefinition::Manual));
    let trigger = manual_triggers.next().ok_or_else(|| {
        operator_error(
            StatusCode::CONFLICT,
            "INVALID_TRIGGER",
            "This Automation has no Manual trigger",
        )
    })?;
    if manual_triggers.next().is_some() {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "INVALID_TRIGGER",
            "This Automation has multiple Manual triggers; choose a definition with one unambiguous local trigger",
        ));
    }
    match &trigger.placement {
        domain_responsibility::TriggerPlacement::Hub => {
            return Err(operator_error(
                StatusCode::CONFLICT,
                "TRIGGER_HOST_UNAVAILABLE",
                "This Manual trigger is assigned to a Hub Runtime",
            ));
        }
        domain_responsibility::TriggerPlacement::SpecificRuntime
            if trigger.runtime_id.as_deref() != Some(state.runtime_id.as_str()) =>
        {
            return Err(operator_error(
                StatusCode::CONFLICT,
                "TRIGGER_HOST_UNAVAILABLE",
                "This Manual trigger is assigned to another Runtime",
            ));
        }
        _ => {}
    }

    // Resolve an exact replay before consulting mutable Coworker, Resource, Runtime
    // binding, or Automation-head state. The TaskStore receipt is owner/request scoped
    // and compares the canonical command payload; a changed payload conflicts.
    let occurrence_id = idempotent_id(
        "occ",
        &state.principal_id,
        &request_id,
        &format!("{scope}.{}", trigger.trigger_id),
    );
    let idempotency_payload = json!({
        "workspace_id": workspace.workspace_id,
        "automation_id": id,
        "automation_revision": body.automation_revision,
        "expected_automation_version": expected_automation_version,
        "trigger_id": trigger.trigger_id,
        "inputs": body.inputs,
    });
    let replay_store = state.store.clone();
    let principal_id = state.principal_id.clone();
    let replay_request_id = request_id.clone();
    let replay_payload = idempotency_payload.clone();
    let replay = tokio::task::spawn_blocking(move || {
        replay_store.get_task_create_receipt(&principal_id, &replay_request_id, &replay_payload)
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|error| match error {
        storage_core::StoreError::Conflict { .. } => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Idempotency key was already used for a different command",
        ),
        _ => unavailable(),
    })?;
    if let Some(committed) = replay {
        if committed.view.task.workspace_id != workspace.workspace_id
            || committed.view.task.status != "READY"
            || committed.view.task.automation_id.as_deref() != Some(id.as_str())
            || committed.view.task.routine_revision
                != Some(automation_revision.definition.routine_revision)
            || committed.view.task.automation_occurrence_id.as_deref()
                != Some(occurrence_id.as_str())
            || committed.event.entity_type != "Task"
            || committed.event.entity_id != committed.view.task.task_id
            || committed
                .event
                .payload
                .get("automation_id")
                .and_then(serde_json::Value::as_str)
                != Some(id.as_str())
            || committed
                .event
                .payload
                .get("automation_occurrence_id")
                .and_then(serde_json::Value::as_str)
                != Some(occurrence_id.as_str())
        {
            return Err(unavailable());
        }
        let mut response = (
            StatusCode::OK,
            Json(ManualAutomationRunResponse {
                automation_id: id,
                automation_revision: body.automation_revision,
                trigger_id: trigger.trigger_id.clone(),
                occurrence_id,
                occurrence_version: 3,
                occurrence_status: "STARTED",
                task: committed.view,
            }),
        )
            .into_response();
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        );
        if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
            response
                .headers_mut()
                .insert(header::HeaderName::from_static("x-correlation-id"), value);
        }
        return Ok(response);
    }

    if workspace.status != "ACTIVE" {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces cannot accept new Tasks",
        ));
    }
    let binding = state
        .store
        .get_current_local_binding(storage_core::LocalRuntimeWorkspaceBindingLookup {
            owner_principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            runtime_id: state.runtime_id.clone(),
            runtime_incarnation_id: state.local_incarnation_id.clone(),
        })
        .map_err(|_| unavailable())?;
    // A same-key retry must reach TaskStore's dedupe-first transaction even if the
    // local host binding changed after the original commit. A new admission is still
    // rejected atomically unless this exact binding/incarnation is active with role.
    let trigger_host_binding_version = binding
        .as_ref()
        .filter(|binding| {
            binding.status == "ACTIVE" && binding.roles.iter().any(|role| role == "TRIGGER_HOST")
        })
        .map(|binding| binding.version)
        .unwrap_or(1);

    let routine_context = RoutineEventContext {
        event_id: event.event_id.clone(),
        origin_runtime_id: event.origin_runtime_id.clone(),
        hlc_timestamp: event.hlc_timestamp.clone(),
        correlation_id: event.correlation_id.clone(),
        causation_id: event.causation_id.clone(),
        recorded_at: event.recorded_at.clone(),
    };
    let routine_store =
        SqliteRoutineStore::new(state.store.clone(), routine_context).map_err(|_| unavailable())?;
    let routine_revision = routine_store
        .get_revision(
            &state.principal_id,
            &workspace.workspace_id,
            &automation_revision.definition.routine_id,
            automation_revision.definition.routine_revision,
        )
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            operator_error(
                StatusCode::CONFLICT,
                "ROUTINE_REVISION_NOT_FOUND",
                "The pinned Routine revision is unavailable",
            )
        })?;
    let materialized =
        materialize_routine_inputs(&routine_revision, &body.inputs, &workspace.workspace_id)
            .map_err(|_| {
                operator_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "INVALID_ARGUMENT",
                    "Inputs do not match the pinned Routine schema and bindings",
                )
            })?;

    let coworker_ref = automation_revision.definition.coworker_ref.as_ref();
    let coworker_data = if let Some(pin) = coworker_ref {
        let store = adapter(&state, &correlation)?;
        let revision = store
            .get_revision(
                &state.principal_id,
                &workspace.workspace_id,
                &pin.coworker_id,
                pin.revision,
            )
            .map_err(domain_error)?
            .ok_or_else(|| {
                operator_error(
                    StatusCode::CONFLICT,
                    "NOT_FOUND",
                    "The pinned Coworker revision is unavailable",
                )
            })?;
        if revision.revision != pin.revision || revision.coworker_id != pin.coworker_id {
            return Err(unavailable());
        }
        Some((pin.coworker_id.clone(), pin.revision, revision))
    } else {
        None
    };
    let lead_binding_id = coworker_data
        .as_ref()
        .and_then(|(_, _, revision)| revision.definition.default_lead_agent_binding_id.clone())
        .or_else(|| {
            routine_revision
                .definition
                .preferred_agent_binding_id
                .clone()
        })
        .or_else(|| workspace.default_agent_binding_id.clone())
        .ok_or_else(|| {
            operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "AGENT_UNAVAILABLE",
                "Choose an enabled lead Agent before running this Automation",
            )
        })?;

    let occurrence_identity = OccurrenceIdentity::Manual {
        trigger_id: trigger.trigger_id.clone(),
        principal_id: state.principal_id.clone(),
        request_id: request_id.clone(),
        automation_id: id.clone(),
    };
    let occurrence_key = occurrence_identity.key().map_err(|_| invalid())?;
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
        "preferred_lead_agent_binding_id": lead_binding_id,
        "coworker_id": coworker_ref.map(|pin| pin.coworker_id.clone()),
        "routine_id": automation_revision.definition.routine_id,
        "routine_revision": automation_revision.definition.routine_revision,
        "routine_inputs": body.inputs.clone(),
        "automation_id": id.clone(),
        "automation_revision": body.automation_revision,
        "expected_automation_version": expected_automation_version,
        "trigger_id": trigger.trigger_id.clone(),
        "occurrence_id": occurrence_id.clone(),
    });
    let command = CreateStandaloneTask {
        task_id: task_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        workspace_instruction_revision: workspace.current_instruction_revision,
        lead_agent_binding_id: lead_binding_id,
        origin_coworker_id: coworker_data.as_ref().map(|(id, _, _)| id.clone()),
        origin_coworker_revision: coworker_data.as_ref().map(|(_, revision, _)| *revision),
        expected_coworker_version: None,
        coworker_default_lead_failover_policy: coworker_data
            .as_ref()
            .and_then(|(_, _, revision)| revision.definition.lead_failover_policy.as_ref())
            .map(|policy| serde_json::Value::Object(policy.clone().into_iter().collect())),
        principal_id: state.principal_id.clone(),
        request_id: request_id.clone(),
        request_payload,
        event: EventContext {
            event_id: event.event_id.clone(),
            origin_runtime_id: event.origin_runtime_id.clone(),
            hlc_timestamp: event.hlc_timestamp.clone(),
            correlation_id: event.correlation_id.clone(),
            causation_id: event.causation_id.clone(),
            recorded_at: event.recorded_at.clone(),
        },
    };
    let mut commit = TaskService::new(state.store.clone())
        .prepare_standalone(command)
        .map_err(|_| {
            operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_ARGUMENT",
                "The pinned Routine could not be materialized as a Task",
            )
        })?;
    let claim_expires_at = time::OffsetDateTime::parse(
        &event.recorded_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| unavailable())?
    .checked_add(time::Duration::minutes(2))
    .ok_or_else(unavailable)?
    .format(&time::format_description::well_known::Rfc3339)
    .map_err(|_| unavailable())?;
    commit.idempotency_payload = Some(idempotency_payload);
    commit.task.routine_id = Some(automation_revision.definition.routine_id.clone());
    commit.task.routine_revision = Some(automation_revision.definition.routine_revision);
    commit.task.automation_id = Some(id.clone());
    commit.task.automation_occurrence_id = Some(occurrence_id.clone());
    commit.routine_admission = Some(RoutineTaskAdmission {
        routine_id: automation_revision.definition.routine_id.clone(),
        routine_revision: automation_revision.definition.routine_revision,
        inputs: body.inputs.clone(),
    });
    commit.automation_admission = Some(AutomationTaskAdmission {
        automation_id: id.clone(),
        automation_revision: body.automation_revision,
        expected_automation_version,
        routine_id: automation_revision.definition.routine_id.clone(),
        routine_revision: automation_revision.definition.routine_revision,
        trigger_id: trigger.trigger_id.clone(),
        trigger_host_runtime_id: state.runtime_id.clone(),
        trigger_host_runtime_incarnation_id: state.local_incarnation_id.clone(),
        trigger_host_binding_version,
        occurrence_id: occurrence_id.clone(),
        occurrence_key,
        claim_expires_at,
        trigger_kind: "MANUAL".to_owned(),
        scheduled_epoch_ms: None,
        covered_misfire_range: None,
    });
    commit.event.payload["routine_id"] = json!(automation_revision.definition.routine_id);
    commit.event.payload["routine_revision"] =
        json!(automation_revision.definition.routine_revision);
    commit.event.payload["automation_id"] = json!(id);
    commit.event.payload["automation_occurrence_id"] = json!(occurrence_id);

    let store = state.store.clone();
    let committed = tokio::task::spawn_blocking(move || store.create_task(commit)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Manual Automation admission did not complete"))?
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "Manual trigger, pinned dependencies, inputs, or local host are no longer eligible"),
            storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "CONFLICT", "Automation or its dependencies changed while the run was being admitted; refresh and retry"),
            storage_core::StoreError::NotFound => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "A pinned Resource or lead binding is unavailable"),
            _ => unavailable(),
        })?;
    if committed.view.task.workspace_id != workspace.workspace_id
        || committed.view.task.status != "READY"
        || committed.view.task.automation_id.as_deref() != Some(id.as_str())
        || committed.view.task.automation_occurrence_id.as_deref() != Some(occurrence_id.as_str())
        || committed.view.task.routine_revision
            != Some(automation_revision.definition.routine_revision)
    {
        return Err(unavailable());
    }
    let response = ManualAutomationRunResponse {
        automation_id: id,
        automation_revision: body.automation_revision,
        trigger_id: trigger.trigger_id.clone(),
        occurrence_id,
        occurrence_version: 3,
        occurrence_status: "STARTED",
        task: committed.view,
    };
    let mut response = (StatusCode::CREATED, Json(response)).into_response();
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

/// Admits one ordinary READY Task pinned to an exact saved AutomationRevision.
/// A test run is not an occurrence: it requires no trigger identity or trigger-host
/// binding, creates no AutomationOccurrence, and never advances a trigger cursor.
async fn run_automation_test(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let workspace = ensure_workspace_owner(&state, &workspace_id)?;
    if !valid_id(&id) {
        return Err(invalid());
    }
    let body: RunAutomationTestBody = parse_body(&body)?;
    if body.automation_revision == 0 || !body.inputs.is_object() {
        return Err(operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ARGUMENT",
            "Automation test inputs or revision are invalid",
        ));
    }
    let expected_automation_version = expected_version(&headers)?;
    let request_id = idempotency_key(&headers)?;
    let scope = format!("automation.test_run.{}.{}", workspace.workspace_id, id);
    let task_id = idempotent_id("tsk", &state.principal_id, &request_id, &scope);
    let test_run_id = idempotent_id("trn", &state.principal_id, &request_id, &scope);
    let correlation = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        &format!("{scope}.correlation"),
    );
    let command_key = TestRunIdentity {
        automation_id: id.clone(),
        automation_revision: body.automation_revision,
        principal_id: state.principal_id.clone(),
        request_id: request_id.clone(),
    }
    .key()
    .map_err(|_| invalid())?;
    let mut event = event_context(&state.runtime_id, &correlation).map_err(|_| {
        unavailable()
    })?;
    event.event_id = idempotent_id(
        "ev",
        &state.principal_id,
        &request_id,
        &format!("{scope}.event"),
    );

    let automation_store = adapter(&state, &correlation)?;
    // The exact revision is immutable and is resolved before receipt replay. Mutable
    // head status/version/revision checks happen only after the receipt lookup.
    let automation_revision = automation_store
        .get_automation_revision(
            &state.principal_id,
            &workspace.workspace_id,
            &id,
            body.automation_revision,
        )
        .map_err(domain_error)?
        .ok_or_else(|| domain_error(DomainError::NotFound))?;
    if automation_revision.automation_id != id
        || automation_revision.revision != body.automation_revision
    {
        return Err(unavailable());
    }

    let idempotency_payload = json!({
        "operation": "automation.test_run.v1",
        "workspace_id": workspace.workspace_id,
        "automation_id": id,
        "automation_revision": body.automation_revision,
        "expected_automation_version": expected_automation_version,
        "routine_id": automation_revision.definition.routine_id,
        "routine_revision": automation_revision.definition.routine_revision,
        "inputs": body.inputs,
        "test_run_id": test_run_id,
    });
    let replay_store = state.store.clone();
    let principal_id = state.principal_id.clone();
    let replay_request_id = request_id.clone();
    let replay_payload = idempotency_payload.clone();
    let replay = tokio::task::spawn_blocking(move || {
        replay_store.get_task_create_receipt(&principal_id, &replay_request_id, &replay_payload)
    })
    .await
    .map_err(|_| {
        unavailable()
    })?
    .map_err(|error| match error {
        storage_core::StoreError::Conflict { .. } => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Idempotency key was already used for a different command",
        ),
        _ => unavailable(),
    })?;
    if let Some(committed) = replay {
        if committed.view.task.workspace_id != workspace.workspace_id
            || committed.view.task.status != "READY"
            || committed.view.task.automation_id.is_some()
            || committed.view.task.automation_occurrence_id.is_some()
            || committed.view.task.automation_test_run_id.as_deref() != Some(test_run_id.as_str())
            || committed.view.task.routine_id.as_deref()
                != Some(automation_revision.definition.routine_id.as_str())
            || committed.view.task.routine_revision
                != Some(automation_revision.definition.routine_revision)
            || committed.event.entity_type != "Task"
            || committed.event.entity_id != committed.view.task.task_id
            || committed
                .event
                .payload
                .get("automation_test_run_id")
                .and_then(serde_json::Value::as_str)
                != Some(test_run_id.as_str())
        {
            return Err(unavailable());
        }
        return Ok(no_store(
            Json(AutomationTestRunResponse {
                test_run_id,
                automation_id: id,
                automation_revision: body.automation_revision,
                task: committed.view,
            })
            .into_response(),
        ));
    }

    if workspace.status != "ACTIVE" {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces cannot accept new Tasks",
        ));
    }
    let (automation, current_revision) = automation_store
        .get_automation(&state.principal_id, &workspace.workspace_id, &id)
        .map_err(domain_error)?
        .ok_or_else(|| domain_error(DomainError::NotFound))?;
    if automation.automation_id != id
        || current_revision.automation_id != id
        || current_revision.revision != automation.current_revision
    {
        return Err(unavailable());
    }
    if current_revision.revision != body.automation_revision {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "STALE_VERSION",
            "The Automation revision is not the current saved revision",
        ));
    }
    if automation.status == AutomationStatus::Disabled {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "AUTOMATION_DISABLED",
            "A disabled Automation cannot accept a test run",
        ));
    }
    if automation.version != expected_automation_version
        || automation.current_revision != body.automation_revision
    {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "STALE_VERSION",
            "The Automation changed while the test run was being prepared",
        ));
    }

    let routine_context = RoutineEventContext {
        event_id: event.event_id.clone(),
        origin_runtime_id: event.origin_runtime_id.clone(),
        hlc_timestamp: event.hlc_timestamp.clone(),
        correlation_id: event.correlation_id.clone(),
        causation_id: event.causation_id.clone(),
        recorded_at: event.recorded_at.clone(),
    };
    let routine_store =
        SqliteRoutineStore::new(state.store.clone(), routine_context).map_err(|_| {
            unavailable()
        })?;
    let routine_revision = routine_store
        .get_revision(
            &state.principal_id,
            &workspace.workspace_id,
            &automation_revision.definition.routine_id,
            automation_revision.definition.routine_revision,
        )
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            operator_error(
                StatusCode::CONFLICT,
                "ROUTINE_REVISION_NOT_FOUND",
                "The pinned Routine revision is unavailable",
            )
        })?;
    let materialized =
        materialize_routine_inputs(&routine_revision, &body.inputs, &workspace.workspace_id)
            .map_err(|_| {
                operator_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "INVALID_ARGUMENT",
                    "Inputs do not match the pinned Routine schema and bindings",
                )
            })?;

    let coworker_ref = automation_revision.definition.coworker_ref.as_ref();
    let coworker_data = if let Some(pin) = coworker_ref {
        let store = adapter(&state, &correlation)?;
        let revision = store
            .get_revision(
                &state.principal_id,
                &workspace.workspace_id,
                &pin.coworker_id,
                pin.revision,
            )
            .map_err(domain_error)?
            .ok_or_else(|| {
                operator_error(
                    StatusCode::CONFLICT,
                    "NOT_FOUND",
                    "The pinned Coworker revision is unavailable",
                )
            })?;
        if revision.revision != pin.revision || revision.coworker_id != pin.coworker_id {
            return Err(unavailable());
        }
        Some((pin.coworker_id.clone(), pin.revision, revision))
    } else {
        None
    };
    let lead_binding_id = coworker_data
        .as_ref()
        .and_then(|(_, _, revision)| revision.definition.default_lead_agent_binding_id.clone())
        .or_else(|| {
            routine_revision
                .definition
                .preferred_agent_binding_id
                .clone()
        })
        .or_else(|| workspace.default_agent_binding_id.clone())
        .ok_or_else(|| {
            operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "AGENT_UNAVAILABLE",
                "Choose an enabled lead Agent before testing this Automation",
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
        "preferred_lead_agent_binding_id": lead_binding_id,
        "coworker_id": coworker_ref.map(|pin| pin.coworker_id.clone()),
        "routine_id": automation_revision.definition.routine_id,
        "routine_revision": automation_revision.definition.routine_revision,
        "routine_inputs": body.inputs.clone(),
        "automation_id": id.clone(),
        "automation_revision": body.automation_revision,
        "expected_automation_version": expected_automation_version,
        "automation_test_run_id": test_run_id,
    });
    let command = CreateStandaloneTask {
        task_id: task_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        workspace_instruction_revision: workspace.current_instruction_revision,
        lead_agent_binding_id: lead_binding_id,
        origin_coworker_id: coworker_data.as_ref().map(|(id, _, _)| id.clone()),
        origin_coworker_revision: coworker_data.as_ref().map(|(_, revision, _)| *revision),
        expected_coworker_version: None,
        coworker_default_lead_failover_policy: coworker_data
            .as_ref()
            .and_then(|(_, _, revision)| revision.definition.lead_failover_policy.as_ref())
            .map(|policy| serde_json::Value::Object(policy.clone().into_iter().collect())),
        principal_id: state.principal_id.clone(),
        request_id: request_id.clone(),
        request_payload,
        event: EventContext {
            event_id: event.event_id.clone(),
            origin_runtime_id: event.origin_runtime_id.clone(),
            hlc_timestamp: event.hlc_timestamp.clone(),
            correlation_id: event.correlation_id.clone(),
            causation_id: event.causation_id.clone(),
            recorded_at: event.recorded_at.clone(),
        },
    };
    let mut commit = TaskService::new(state.store.clone())
        .prepare_standalone(command)
        .map_err(|_| {
            operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_ARGUMENT",
                "The pinned Routine could not be materialized as a Task",
            )
        })?;
    commit.idempotency_payload = Some(idempotency_payload);
    commit.task.routine_id = Some(automation_revision.definition.routine_id.clone());
    commit.task.routine_revision = Some(automation_revision.definition.routine_revision);
    commit.task.automation_id = None;
    commit.task.automation_occurrence_id = None;
    commit.task.automation_test_run_id = Some(test_run_id.clone());
    commit.routine_admission = Some(RoutineTaskAdmission {
        routine_id: automation_revision.definition.routine_id.clone(),
        routine_revision: automation_revision.definition.routine_revision,
        inputs: body.inputs.clone(),
    });
    commit.automation_admission = None;
    commit.automation_test_admission = Some(AutomationTestAdmission {
        test_run_id: test_run_id.clone(),
        command_key,
        automation_id: id.clone(),
        automation_revision: body.automation_revision,
        expected_automation_version,
        routine_id: automation_revision.definition.routine_id.clone(),
        routine_revision: automation_revision.definition.routine_revision,
        inputs: body.inputs.clone(),
    });
    commit.event.payload["routine_id"] = json!(automation_revision.definition.routine_id);
    commit.event.payload["routine_revision"] =
        json!(automation_revision.definition.routine_revision);
    commit.event.payload["automation_test_run_id"] = json!(test_run_id);

    let store = state.store.clone();
    let committed = tokio::task::spawn_blocking(move || store.create_task(commit))
        .await
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Automation test-run admission did not complete",
            )
        })?
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_ARGUMENT",
                "The Automation, pinned dependencies, or inputs are no longer eligible",
            ),
            storage_core::StoreError::Conflict { .. } => operator_error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Automation or its dependencies changed while the test run was being admitted; refresh and retry",
            ),
            storage_core::StoreError::NotFound => operator_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_ARGUMENT",
                "A pinned Resource or lead binding is unavailable",
            ),
            _ => unavailable(),
        })?;
    if committed.view.task.workspace_id != workspace.workspace_id
        || committed.view.task.status != "READY"
        || committed.view.task.automation_id.is_some()
        || committed.view.task.automation_occurrence_id.is_some()
        || committed.view.task.automation_test_run_id.as_deref() != Some(test_run_id.as_str())
        || committed.view.task.routine_id.as_deref()
            != Some(automation_revision.definition.routine_id.as_str())
        || committed.view.task.routine_revision
            != Some(automation_revision.definition.routine_revision)
    {
        return Err(unavailable());
    }
    let response = AutomationTestRunResponse {
        test_run_id,
        automation_id: id,
        automation_revision: body.automation_revision,
        task: committed.view,
    };
    let mut response = (StatusCode::CREATED, Json(response)).into_response();
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

/// One scheduled occurrence that was materialized into an ordinary READY Task.
#[derive(Serialize)]
struct AutomationHostMaterialization {
    occurrence_id: String,
    scheduled_for: String,
    task_id: String,
    task_status: &'static str,
}

/// One due slot that was recorded as a bounded skipped occurrence.
#[derive(Serialize)]
struct AutomationHostSkip {
    scheduled_for: String,
    reason: String,
    occurrence_recorded: bool,
}

/// One due slot that stays due because a dependency or a prior run is not
/// settled yet.
#[derive(Serialize)]
struct AutomationHostDeferral {
    scheduled_for: String,
    reason: String,
}

/// The settlement result of one Automation trigger.
#[derive(Serialize)]
struct AutomationHostTriggerSettlement {
    automation_id: String,
    automation_revision: u64,
    trigger_id: String,
    next_scheduled_at: Option<String>,
    materialized: Vec<AutomationHostMaterialization>,
    skipped: Vec<AutomationHostSkip>,
    deferred: Vec<AutomationHostDeferral>,
    blockers: Vec<String>,
}

/// The result of one bounded local trigger-host settlement pass.
#[derive(Serialize)]
struct AutomationHostSettlementResponse {
    runtime_id: String,
    checked_at: String,
    triggers: Vec<AutomationHostTriggerSettlement>,
}

/// The bounded local trigger host context.
///
/// It carries the durable store and the authenticated local identity. The
/// background trigger host and the authenticated settlement route share it so
/// both observe the same durable decisions.
#[derive(Clone)]
pub(crate) struct TriggerHostContext {
    store: SqliteWorkspaceStore,
    principal_id: String,
    runtime_id: String,
    local_incarnation_id: String,
}

impl TriggerHostContext {
    fn from_state(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
            runtime_id: state.runtime_id.clone(),
            local_incarnation_id: state.local_incarnation_id.clone(),
        }
    }
}

/// The trigger-host context of the local Runtime.
pub(crate) fn trigger_host_context(state: &ApiState) -> TriggerHostContext {
    TriggerHostContext::from_state(state)
}

/// The local Runtime must hold an active `TRIGGER_HOST` Workspace binding
/// before it may claim a recurring slot.
fn authorized_trigger_host(state: &ApiState, headers: &HeaderMap) -> Result<Workspace, Response> {
    let workspace = authorized_workspace(state, headers)?;
    let binding = state
        .store
        .get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
            owner_principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            runtime_id: state.runtime_id.clone(),
            runtime_incarnation_id: state.local_incarnation_id.clone(),
        })
        .map_err(|_| unavailable())?;
    if !binding.is_some_and(|binding| {
        binding.status == "ACTIVE" && binding.roles.iter().any(|role| role == "TRIGGER_HOST")
    }) {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "TRIGGER_HOST_UNAVAILABLE",
            "This Runtime is not an active trigger host for the selected Workspace",
        ));
    }
    Ok(workspace)
}

/// Runs one bounded settlement pass over the schedule slots this Runtime owns.
///
/// The pass materializes the due slots the concurrency policy allows, records
/// every missed slot as a bounded skipped occurrence, and advances the persisted
/// cursor. It creates no Plan, Step, AgentSession, Attempt, lease, Environment,
/// Effect, or capability grant: every materialized occurrence is an ordinary
/// READY Task.
async fn settle_automation_triggers(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace = authorized_trigger_host(&state, &headers)?;
    let recorded_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| unavailable())?;
    let context = TriggerHostContext::from_state(&state);
    let hosted = TriggerHostWorkspace {
        workspace_id: workspace.workspace_id.clone(),
        current_instruction_revision: workspace.current_instruction_revision,
        default_agent_binding_id: workspace.default_agent_binding_id.clone(),
    };
    let triggers = tokio::task::spawn_blocking({
        let recorded_at = recorded_at.clone();
        move || settle_hosted_triggers(&context, &[hosted], &recorded_at)
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;
    let mut response = (
        StatusCode::OK,
        Json(AutomationHostSettlementResponse {
            runtime_id: state.runtime_id.clone(),
            checked_at: recorded_at,
            triggers,
        }),
    )
        .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

/// Settles every trigger of the supplied hosted Workspaces.
///
/// The function is synchronous on purpose: the background trigger host runs it
/// on a supervised thread and the authenticated route runs it inside
/// `spawn_blocking`. Both callers observe the same durable decisions.
fn settle_hosted_triggers(
    context: &TriggerHostContext,
    workspaces: &[TriggerHostWorkspace],
    recorded_at: &str,
) -> Result<Vec<AutomationHostTriggerSettlement>, String> {
    let now_epoch_ms = domain_responsibility::parse_instant(recorded_at)
        .ok_or_else(|| "TRIGGER_HOST_CLOCK_UNAVAILABLE".to_owned())?;
    let host = SqliteAutomationHostStore::new(
        context.store.clone(),
        AutomationHostContext {
            origin_runtime_id: context.runtime_id.clone(),
            recorded_at: recorded_at.to_owned(),
        },
    )
    .map_err(|_| "TRIGGER_HOST_STORE_UNAVAILABLE".to_owned())?;
    let mut settlements = Vec::new();
    let due = host
        .list_due_schedules(&context.runtime_id, now_epoch_ms)
        .map_err(|_| "TRIGGER_HOST_LOOKUP_FAILED".to_owned())?;
    for workspace in workspaces {
        for schedule in due
            .iter()
            .filter(|schedule| schedule.workspace_id == workspace.workspace_id)
        {
            settlements.push(settle_one_trigger(
                context,
                workspace,
                &host,
                schedule,
                now_epoch_ms,
                recorded_at,
            ));
        }
    }
    Ok(settlements)
}

/// Settles one Automation trigger. A failure of one trigger never stops the
/// others: the failure is reported as a blocker and the slot stays due.
fn settle_one_trigger(
    context: &TriggerHostContext,
    workspace: &TriggerHostWorkspace,
    host: &SqliteAutomationHostStore,
    due: &DueSchedule,
    now_epoch_ms: i64,
    recorded_at: &str,
) -> AutomationHostTriggerSettlement {
    let trigger_id = due.trigger.trigger_id.clone();
    let mut settlement = AutomationHostTriggerSettlement {
        automation_id: due.automation_id.clone(),
        automation_revision: due.automation_revision,
        trigger_id: trigger_id.clone(),
        next_scheduled_at: None,
        materialized: Vec::new(),
        skipped: Vec::new(),
        deferred: Vec::new(),
        blockers: Vec::new(),
    };
    let Some(cursor) = due.cursor.as_ref() else {
        settlement
            .blockers
            .push("TRIGGER_CURSOR_MISSING".to_owned());
        return settlement;
    };
    let nonterminal = match host.nonterminal_occurrence_count(&due.workspace_id, &due.automation_id)
    {
        Ok(count) => count,
        Err(_) => {
            settlement
                .blockers
                .push("OCCURRENCE_COUNT_UNAVAILABLE".to_owned());
            return settlement;
        }
    };
    let plan = match plan_host_settlement(
        &due.schedule,
        cursor.next_scheduled_at,
        SettlementWindow {
            now: now_epoch_ms,
            grace_ms: TRIGGER_HOST_GRACE_MS,
        },
        HostConcurrencyFacts {
            nonterminal_occurrences: nonterminal,
            max_concurrent_occurrences: due.execution_policy.max_concurrent_occurrences,
            overlap_policy: due.execution_policy.overlap_policy,
        },
    ) {
        Ok(plan) => plan,
        Err(_) => {
            settlement
                .blockers
                .push("SCHEDULE_PLAN_UNAVAILABLE".to_owned());
            return settlement;
        }
    };
    let mut earliest_deferred: Option<i64> = None;
    for decision in plan.decisions.iter() {
        match decision {
            HostSlotDecision::Materialize {
                instant,
                covered_from,
                covered_to,
                covered_count,
            } => {
                match materialize_scheduled_slot(
                    context,
                    workspace,
                    host,
                    due,
                    *instant,
                    *covered_from,
                    *covered_to,
                    *covered_count,
                    recorded_at,
                ) {
                    Ok(materialization) => settlement.materialized.push(materialization),
                    Err(reason) => {
                        settlement.deferred.push(AutomationHostDeferral {
                            scheduled_for: format_slot_instant(*instant),
                            reason: reason.clone(),
                        });
                        settlement.blockers.push(reason);
                        earliest_deferred = Some(match earliest_deferred {
                            Some(current) => current.min(*instant),
                            None => *instant,
                        });
                        break;
                    }
                }
            }
            HostSlotDecision::Skip { instant, reason } => {
                let reason_token = skip_reason_token(*reason);
                let recorded = record_skipped_slot(
                    context,
                    host,
                    due,
                    *instant,
                    &reason_token,
                );
                settlement.skipped.push(AutomationHostSkip {
                    scheduled_for: format_slot_instant(*instant),
                    reason: reason_token,
                    occurrence_recorded: recorded,
                });
            }
            HostSlotDecision::Defer { instant, reason } => {
                settlement.deferred.push(AutomationHostDeferral {
                    scheduled_for: format_slot_instant(*instant),
                    reason: defer_reason_token(*reason).to_owned(),
                });
                earliest_deferred = Some(match earliest_deferred {
                    Some(current) => current.min(*instant),
                    None => *instant,
                });
                break;
            }
        }
    }
    // A deferred slot was not materialized. The cursor keeps naming it so a
    // later pass can still create its occurrence.
    let next_scheduled_at = match (earliest_deferred, plan.next_scheduled_at) {
        (Some(deferred), Some(next)) => Some(deferred.min(next)),
        (Some(deferred), None) => Some(deferred),
        (None, next) => next,
    };
    match host.advance_cursor(
        &due.workspace_id,
        &due.automation_id,
        &trigger_id,
        next_scheduled_at,
        now_epoch_ms,
        plan.misfired,
    ) {
        Ok(advanced) => {
            settlement.next_scheduled_at = advanced
                .next_scheduled_at
                .map(|value| format_slot_instant(value));
        }
        Err(_) => settlement
            .blockers
            .push("TRIGGER_CURSOR_ADVANCE_FAILED".to_owned()),
    }
    settlement
}

/// The stable diagnostic token of a skipped slot.
fn skip_reason_token(reason: SlotOutcomeReason) -> String {
    match reason {
        SlotOutcomeReason::Missed(_) => "MISFIRED_SLOT_SKIPPED".to_owned(),
        SlotOutcomeReason::OverlapSkipped => "OVERLAP_SKIPPED".to_owned(),
        SlotOutcomeReason::OverlapQueued => "OVERLAP_QUEUED".to_owned(),
        SlotOutcomeReason::OverlapCancelOld => "OVERLAP_CANCEL_OLD".to_owned(),
    }
}

/// The stable diagnostic token of a deferred slot.
fn defer_reason_token(reason: SlotOutcomeReason) -> &'static str {
    match reason {
        SlotOutcomeReason::Missed(_) => "MISFIRED_SLOT_DEFERRED",
        SlotOutcomeReason::OverlapSkipped => "OVERLAP_SKIPPED",
        SlotOutcomeReason::OverlapQueued => "OVERLAP_QUEUED",
        SlotOutcomeReason::OverlapCancelOld => "OVERLAP_CANCEL_OLD",
    }
}

/// Formats a slot instant as the canonical UTC timestamp.
fn format_slot_instant(instant: i64) -> String {
    domain_responsibility::format_instant(instant).unwrap_or_else(|_| instant.to_string())
}

/// Records one bounded skipped occurrence for a due slot.
fn record_skipped_slot(
    context: &TriggerHostContext,
    host: &SqliteAutomationHostStore,
    due: &DueSchedule,
    instant: i64,
    reason: &str,
) -> bool {
    let trigger_id = due.trigger.trigger_id.clone();
    let occurrence_key = domain_responsibility::OccurrenceIdentity::Schedule {
        trigger_id: trigger_id.clone(),
        scheduled_epoch_ms: instant,
    }
    .key()
    .unwrap_or_default();
    if occurrence_key.is_empty() {
        return false;
    }
    let scope = format!("automation.host.{}.{}", due.workspace_id, due.automation_id);
    let slot_request_id = format!("automation-host:{occurrence_key}");
    let occurrence_id = idempotent_id(
        "occ",
        &context.principal_id,
        &slot_request_id,
        &format!("{scope}.{trigger_id}"),
    );
    host.record_skipped_occurrence(
        &due.workspace_id,
        &due.automation_id,
        due.automation_revision,
        &due.routine_id,
        due.routine_revision,
        &trigger_id,
        &context.runtime_id,
        &occurrence_id,
        &occurrence_key,
        instant,
        reason,
    )
    .unwrap_or(false)
}

/// Materializes one due slot into an ordinary READY Task.
///
/// The Task, its TaskSpecRevision, the occurrence claim, the occurrence
/// events, and the idempotency receipt commit in one storage transaction. A
/// repeated pass for the same slot finds the committed occurrence and returns
/// the original Task instead of creating a second one.
fn materialize_scheduled_slot(
    context: &TriggerHostContext,
    workspace: &TriggerHostWorkspace,
    host: &SqliteAutomationHostStore,
    due: &DueSchedule,
    instant: i64,
    covered_from: Option<i64>,
    covered_to: Option<i64>,
    covered_count: u32,
    recorded_at: &str,
) -> Result<AutomationHostMaterialization, String> {
    let trigger_id = due.trigger.trigger_id.clone();
    let occurrence_key = domain_responsibility::OccurrenceIdentity::Schedule {
        trigger_id: trigger_id.clone(),
        scheduled_epoch_ms: instant,
    }
    .key()
    .map_err(|_| "OCCURRENCE_IDENTITY_INVALID".to_owned())?;
    if let Some(existing) = host
        .occurrence_for_slot(
            &due.workspace_id,
            &due.automation_id,
            &trigger_id,
            &occurrence_key,
        )
        .map_err(|_| "OCCURRENCE_LOOKUP_FAILED".to_owned())?
    {
        // The slot was already settled. A materialized occurrence returns its
        // original Task; a skipped occurrence stays skipped.
        if existing.status == domain_responsibility::OccurrenceStatus::Skipped {
            return Err("OCCURRENCE_ALREADY_SKIPPED".to_owned());
        }
        let task_id = existing
            .task_id
            .ok_or_else(|| "OCCURRENCE_TASK_MISSING".to_owned())?;
        return Ok(AutomationHostMaterialization {
            occurrence_id: existing.occurrence_id,
            scheduled_for: format_slot_instant(instant),
            task_id,
            task_status: "READY",
        });
    }
    let routine_store = SqliteRoutineStore::new(
        context.store.clone(),
        RoutineEventContext {
            event_id: format!("automation-host:routine:{}", due.automation_id),
            origin_runtime_id: context.runtime_id.clone(),
            hlc_timestamp: recorded_at.to_owned(),
            correlation_id: format!("automation-host:{}", due.automation_id),
            causation_id: None,
            recorded_at: recorded_at.to_owned(),
        },
    )
    .map_err(|_| "ROUTINE_STORE_UNAVAILABLE".to_owned())?;
    let routine_revision = routine_store
        .get_revision(
            &context.principal_id,
            &workspace.workspace_id,
            &due.routine_id,
            due.routine_revision,
        )
        .map_err(|_| "ROUTINE_LOOKUP_FAILED".to_owned())?
        .ok_or_else(|| "ROUTINE_REVISION_UNAVAILABLE".to_owned())?;
    let inputs = serde_json::json!({});
    let materialized = match materialize_routine_inputs(&routine_revision, &inputs, &workspace.workspace_id)
    {
        Ok(materialized) => materialized,
        Err(_) => {
            // A schedule trigger carries no inputs. A Routine that requires one
            // cannot be materialized by a slot, and the contract records the
            // occurrence as skipped with a typed diagnostic.
            if routine_requires_input(&routine_revision) {
                return Err("MISSING_REQUIRED_INPUT".to_owned());
            }
            return Err("INPUT_MATERIALIZATION_FAILED".to_owned());
        }
    };
    let coworker_store = SqliteCoworkerStore::new(
        context.store.clone(),
        CoworkerEventContext {
            event_id: format!("automation-host:coworker:{}", due.automation_id),
            origin_runtime_id: context.runtime_id.clone(),
            hlc_timestamp: recorded_at.to_owned(),
            correlation_id: format!("automation-host:{}", due.automation_id),
            causation_id: None,
            recorded_at: recorded_at.to_owned(),
        },
    )
    .map_err(|_| "COWORKER_STORE_UNAVAILABLE".to_owned())?;
    let coworker_revision = match due.coworker_ref.as_ref() {
        Some(pin) => coworker_store
            .get_revision(
                &context.principal_id,
                &workspace.workspace_id,
                &pin.coworker_id,
                pin.revision,
            )
            .map_err(|_| "COWORKER_LOOKUP_FAILED".to_owned())?
            .ok_or_else(|| "COWORKER_REVISION_UNAVAILABLE".to_owned())?,
        None => return Err("COWORKER_PIN_MISSING".to_owned()),
    };
    let lead_binding_id = coworker_revision
        .definition
        .default_lead_agent_binding_id
        .clone()
        .or_else(|| routine_revision.definition.preferred_agent_binding_id.clone())
        .or_else(|| workspace.default_agent_binding_id.clone())
        .ok_or_else(|| "AGENT_UNAVAILABLE".to_owned())?;
    let scope = format!("automation.host.{}.{}", due.workspace_id, due.automation_id);
    let slot_request_id = format!("automation-host:{occurrence_key}");
    let task_id = idempotent_id("tsk", &context.principal_id, &slot_request_id, &scope);
    let occurrence_id = idempotent_id(
        "occ",
        &context.principal_id,
        &slot_request_id,
        &format!("{scope}.{trigger_id}"),
    );
    let binding = context
        .store
        .get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
            owner_principal_id: context.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            runtime_id: context.runtime_id.clone(),
            runtime_incarnation_id: context.local_incarnation_id.clone(),
        })
        .map_err(|_| "TRIGGER_HOST_BINDING_UNAVAILABLE".to_owned())?
        .ok_or_else(|| "TRIGGER_HOST_BINDING_UNAVAILABLE".to_owned())?;
    let trigger_host_binding_version = binding.version;
    let claim_expires_at = time::OffsetDateTime::parse(
        recorded_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| "CLAIM_WINDOW_UNAVAILABLE".to_owned())?
    .checked_add(time::Duration::minutes(2))
    .ok_or_else(|| "CLAIM_WINDOW_UNAVAILABLE".to_owned())?
    .format(&time::format_description::well_known::Rfc3339)
    .map_err(|_| "CLAIM_WINDOW_UNAVAILABLE".to_owned())?;
    let scheduled_for = format_slot_instant(instant);
    let covered_misfire_range = if covered_count > 1 || covered_from.is_some() {
        Some(json!({
            "from": covered_from.map(format_slot_instant),
            "to": covered_to.map(format_slot_instant),
            "count": covered_count,
        }))
    } else {
        None
    };
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
        "preferred_lead_agent_binding_id": lead_binding_id,
        "coworker_id": due.coworker_ref.as_ref().map(|pin| pin.coworker_id.clone()),
        "routine_id": due.routine_id,
        "routine_revision": due.routine_revision,
        "routine_inputs": inputs,
        "automation_id": due.automation_id,
        "automation_revision": due.automation_revision,
        "expected_automation_version": due.automation_version,
        "trigger_id": trigger_id,
        "occurrence_id": occurrence_id,
    });
    let idempotency_payload = json!({
        "workspace_id": workspace.workspace_id,
        "automation_id": due.automation_id,
        "automation_revision": due.automation_revision,
        "expected_automation_version": due.automation_version,
        "trigger_id": trigger_id,
        "scheduled_for": scheduled_for,
        "inputs": inputs,
    });
    let command = CreateStandaloneTask {
        task_id: task_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        workspace_instruction_revision: workspace.current_instruction_revision,
        lead_agent_binding_id: lead_binding_id,
        origin_coworker_id: due.coworker_ref.as_ref().map(|pin| pin.coworker_id.clone()),
        origin_coworker_revision: due.coworker_ref.as_ref().map(|pin| pin.revision),
        expected_coworker_version: None,
        coworker_default_lead_failover_policy: coworker_revision
            .definition
            .lead_failover_policy
            .as_ref()
            .map(|policy| serde_json::Value::Object(policy.clone().into_iter().collect())),
        principal_id: context.principal_id.clone(),
        request_id: slot_request_id.clone(),
        request_payload,
        event: EventContext {
            event_id: idempotent_id("ev", &context.principal_id, &slot_request_id, &format!("{scope}.event")),
            origin_runtime_id: context.runtime_id.clone(),
            hlc_timestamp: recorded_at.to_owned(),
            correlation_id: format!("automation-host:{}", due.automation_id),
            causation_id: None,
            recorded_at: recorded_at.to_owned(),
        },
    };
    let mut commit = TaskService::new(context.store.clone())
        .prepare_standalone(command)
        .map_err(|_| "TASK_ADMISSION_REJECTED".to_owned())?;
    commit.idempotency_payload = Some(idempotency_payload);
    commit.task.routine_id = Some(due.routine_id.clone());
    commit.task.routine_revision = Some(due.routine_revision);
    commit.task.automation_id = Some(due.automation_id.clone());
    commit.task.automation_occurrence_id = Some(occurrence_id.clone());
    commit.routine_admission = Some(RoutineTaskAdmission {
        routine_id: due.routine_id.clone(),
        routine_revision: due.routine_revision,
        inputs: inputs.clone(),
    });
    commit.automation_admission = Some(AutomationTaskAdmission {
        automation_id: due.automation_id.clone(),
        automation_revision: due.automation_revision,
        expected_automation_version: due.automation_version,
        routine_id: due.routine_id.clone(),
        routine_revision: due.routine_revision,
        trigger_id: trigger_id.clone(),
        trigger_host_runtime_id: context.runtime_id.clone(),
        trigger_host_runtime_incarnation_id: context.local_incarnation_id.clone(),
        trigger_host_binding_version,
        occurrence_id: occurrence_id.clone(),
        occurrence_key: occurrence_key.clone(),
        claim_expires_at,
        trigger_kind: "SCHEDULE".to_owned(),
        scheduled_epoch_ms: Some(instant),
        covered_misfire_range,
    });
    commit.event.payload["routine_id"] = json!(due.routine_id);
    commit.event.payload["routine_revision"] = json!(due.routine_revision);
    commit.event.payload["automation_id"] = json!(due.automation_id);
    commit.event.payload["automation_occurrence_id"] = json!(occurrence_id);
    let committed = context
        .store
        .create_task(commit)
        .map_err(|error| match error {
            storage_core::StoreError::Conflict { .. } => {
                "AUTOMATION_OR_DEPENDENCY_CHANGED".to_owned()
            }
            storage_core::StoreError::Invalid(_) => "AUTOMATION_ADMISSION_REJECTED".to_owned(),
            storage_core::StoreError::NotFound => "PINNED_DEPENDENCY_UNAVAILABLE".to_owned(),
            _ => "TASK_ADMISSION_FAILED".to_owned(),
        })?;
    if committed.view.task.automation_occurrence_id.as_deref() != Some(occurrence_id.as_str())
        || committed.view.task.automation_id.as_deref() != Some(due.automation_id.as_str())
    {
        return Err("TASK_PROVENANCE_MISMATCH".to_owned());
    }
    Ok(AutomationHostMaterialization {
        occurrence_id,
        scheduled_for: format_slot_instant(instant),
        task_id: committed.view.task.task_id,
        task_status: "READY",
    })
}

/// True when the Routine revision declares at least one required input.
fn routine_requires_input(revision: &domain_responsibility::RoutineRevision) -> bool {
    revision
        .definition
        .input_schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|required| !required.is_empty())
}

/// How often the local trigger host polls its persisted cursors.
pub(crate) const TRIGGER_HOST_POLL_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(30);

/// The supervised local trigger-host worker.
///
/// The worker is the single recurring scheduler for local Automations. It polls
/// the persisted cursors, asks the domain which slots are due, and commits the
/// decision the domain handed it. It never becomes a second scheduler: it owns
/// no queue, no delivery guarantee, and no separate authority. A Workspace it
/// no longer hosts is dropped before the next pass, so a revoked binding never
/// keeps firing.
pub(crate) struct AutomationTriggerHostWorker {
    stop: std::sync::mpsc::SyncSender<()>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl AutomationTriggerHostWorker {
    /// Starts the supervised worker thread.
    pub(crate) fn start(context: TriggerHostContext) -> std::io::Result<Self> {
        let (stop, receiver) = std::sync::mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("litecowork-automation-trigger-host".to_owned())
            .spawn(move || run_trigger_host_loop(receiver, context))?;
        Ok(Self {
            stop,
            join: Some(join),
        })
    }

    /// Stops the worker and joins its thread.
    pub(crate) fn stop(mut self) -> Result<(), String> {
        self.request_stop();
        self.join
            .take()
            .ok_or_else(|| "Automation trigger host worker was already stopped".to_owned())?
            .join()
            .map_err(|_| "Automation trigger host worker terminated unexpectedly".to_owned())
    }

    fn request_stop(&self) {
        let _ = self.stop.try_send(());
    }
}

impl Drop for AutomationTriggerHostWorker {
    fn drop(&mut self) {
        self.request_stop();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// The supervised settlement loop.
///
/// One pass settles every Automation trigger this Runtime hosts. A failing
/// trigger never stops the others: its slot stays due and the next pass
/// retries it.
fn run_trigger_host_loop(stop: std::sync::mpsc::Receiver<()>, context: TriggerHostContext) {
    run_worker_loop(stop, TRIGGER_HOST_POLL_INTERVAL, || {
        let recorded_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        if recorded_at.is_empty() {
            return Err("TRIGGER_HOST_CLOCK_UNAVAILABLE".to_owned());
        }
        let host = SqliteAutomationHostStore::new(
            context.store.clone(),
            AutomationHostContext {
                origin_runtime_id: context.runtime_id.clone(),
                recorded_at: recorded_at.clone(),
            },
        )
        .map_err(|_| "TRIGGER_HOST_STORE_UNAVAILABLE".to_owned())?;
        let workspaces = host
            .list_trigger_host_workspaces(&context.runtime_id)
            .map_err(|_| "TRIGGER_HOST_LOOKUP_FAILED".to_owned())?;
        settle_hosted_triggers(&context, &workspaces, &recorded_at)
            .map(|_| ())
            .map_err(|error| error)
    });
}

/// The supervised worker loop.
///
/// The loop stops on the first stop signal and never blocks the Runtime for
/// longer than the poll interval.
fn run_worker_loop(
    stop: std::sync::mpsc::Receiver<()>,
    idle_poll: std::time::Duration,
    mut settle_once: impl FnMut() -> Result<(), String>,
) {
    loop {
        match stop.try_recv() {
            Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        let _ = settle_once();
        match stop.recv_timeout(idle_poll) {
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

#[cfg(test)]
mod trigger_host_worker_tests {
    use super::*;

    #[test]
    fn the_worker_stops_without_settling_again() {
        let (stop, receiver) = std::sync::mpsc::sync_channel(1);
        let mut calls = 0_u32;
        run_worker_loop(receiver, std::time::Duration::from_millis(1), || {
            calls += 1;
            stop.try_send(()).expect("request worker stop");
            Ok(())
        });
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_failing_settlement_pass_never_stops_the_worker() {
        let (stop, receiver) = std::sync::mpsc::sync_channel(1);
        let mut calls = 0_u32;
        run_worker_loop(receiver, std::time::Duration::from_millis(1), || {
            calls += 1;
            if calls == 1 {
                return Err("TRIGGER_HOST_LOOKUP_FAILED".to_owned());
            }
            stop.try_send(()).expect("request worker stop");
            Ok(())
        });
        assert_eq!(calls, 2);
    }
}

#[cfg(test)]
mod automation_route_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use domain_responsibility::{
        AutomationDefinition, AutomationExecutionPolicy, ContractObject, PlacementClass,
        PlacementPreference, RetryPolicy, RoutineCommand, RoutineOwnerScope, RoutineRevisionInput,
        RoutineService, TriggerDefinition, TriggerPlacement, TriggerSpec,
    };
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use sha2::Digest;
    use storage_core::{
        BlobPurpose, DeviceIdentityRecord, LocalRuntimeWorkspaceEnrollmentRequest,
        RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
        RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
        RuntimeWorkspaceBindingStore, StoreError, WorkspaceCreateRequest,
    };
    use storage_sqlite::{
        FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider,
    };
    use std::sync::Arc;
    use std::time::Duration;
    use tower::ServiceExt;
    use zeroize::Zeroizing;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    const WORKSPACE: &str = "automation-route-workspace";
    const OWNER: &str = "automation-route-owner";
    const RUNTIME: &str = "automation-route-runtime";
    const INCARNATION: &str = "automation-route-incarnation";
    const NOW: &str = "2026-10-20T09:00:00Z";

    struct RouteTestKeys;

    impl WorkspaceBlobKeyProvider for RouteTestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([17_u8; 32]),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob(
                    "unknown Automation route test key version".to_owned(),
                ));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    fn fixture() -> (tempfile::TempDir, SqliteWorkspaceStore) {
        let directory = tempfile::tempdir().expect("temporary Automation route directory");
        #[cfg(unix)]
        std::fs::set_permissions(
            directory.path(),
            std::fs::Permissions::from_mode(0o700),
        )
        .expect("private Automation route directory");
        let store = SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            Arc::new(FileBlobStore::new(
                directory.path().join("blobs"),
                RouteTestKeys,
            )),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("real SQLite store opens");
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Automation route Workspace".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: "event-automation-route-workspace".to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: "correlation-automation-route-workspace".to_owned(),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("owner Workspace commits");
        enroll_local_runtime(&store);
        (directory, store)
    }

    /// Enrolls this local Runtime in the fixture Workspace.
    ///
    /// The Workspace access check requires an `ACTIVE` local binding, so a route
    /// test has to prove the same runtime identity the daemon uses.
    fn enroll_local_runtime(store: &SqliteWorkspaceStore) {
        let public_key = [31_u8; 32];
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
                    resource_capacity: serde_json::json!({"sampled_at": NOW}),
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
                    os_boot_id: Some("boot-automation-route".to_owned()),
                    observed_at: NOW.to_owned(),
                },
            )
            .expect("the local Runtime registers");
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
            .expect("the Runtime enters its serving state");
        store
            .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
                request: WorkspaceCreateRequest {
                    principal_id: OWNER.to_owned(),
                    request_id: "enroll-automation-route".to_owned(),
                    request_payload: serde_json::json!({"workspace_id": WORKSPACE}),
                },
                expected_workspace_version: 1,
                runtime_workspace_binding_id: "binding-automation-route".to_owned(),
                runtime_id: RUNTIME.to_owned(),
                runtime_incarnation_id: INCARNATION.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                now: NOW.to_owned(),
                correlation_id: "correlation-automation-route-enrollment".to_owned(),
            })
            .expect("the Runtime enrolls in the Workspace");
    }

    fn routine_revision_input() -> RoutineRevisionInput {
        RoutineRevisionInput {
            objective_template: "Review the project".to_owned(),
            instructions: "Review the project and report the findings".to_owned(),
            input_schema: ContractObject::new(),
            constraints: Vec::new(),
            non_goals: Vec::new(),
            required_outputs: Vec::new(),
            acceptance_criteria: Vec::new(),
            approvals_required: Vec::new(),
            input_bindings: Vec::new(),
            required_capabilities: Vec::new(),
            preferred_agent_binding_id: None,
            placement_preference: PlacementPreference::Class(PlacementClass::Auto),
            budget_ceiling: None,
            verification_policy: ContractObject::new(),
        }
    }

    /// Creates the Routine and the paused Automation the read routes project.
    fn seed_definition(store: &SqliteWorkspaceStore) -> String {
        let routine_store = SqliteRoutineStore::new(
            store.clone(),
            RoutineEventContext {
                event_id: "event-automation-route-routine".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-automation-route-routine".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        )
        .expect("Routine adapter");
        RoutineService::new(routine_store)
            .execute(
                &RoutineOwnerScope {
                    principal_id: OWNER.to_owned(),
                    workspace_id: WORKSPACE.to_owned(),
                    request_id: "routine-automation-route".to_owned(),
                },
                RoutineCommand::Create {
                    routine_id: "routine-automation-route".to_owned(),
                    name: "Route Routine".to_owned(),
                    revision: routine_revision_input(),
                },
            )
            .expect("the Routine revision commits");
        let trigger = TriggerSpec {
            trigger_id: "trigger-manual".to_owned(),
            placement: TriggerPlacement::Auto,
            runtime_id: None,
            trigger: TriggerDefinition::Manual,
        };
        let definition = AutomationDefinition {
            routine_id: "routine-automation-route".to_owned(),
            routine_revision: 1,
            triggers: vec![trigger],
            execution_policy: AutomationExecutionPolicy {
                placement_preference: PlacementPreference::Class(PlacementClass::Auto),
                max_concurrent_occurrences: 1,
                overlap_policy: domain_responsibility::OverlapPolicy::Skip,
                retry_policy: RetryPolicy {
                    max_attempts: 1,
                    initial_backoff_ms: 100,
                    max_backoff_ms: 100,
                    multiplier: 1.0,
                    jitter: false,
                    retryable_error_codes: Vec::new(),
                },
                budget_ceiling: None,
                notification_policy: domain_responsibility::AutomationNotification::OnFailure,
                wake_policy: domain_responsibility::WakePolicy::Never,
            },
            coworker_ref: None,
        };
        let coworker_store = SqliteCoworkerStore::new(
            store.clone(),
            CoworkerEventContext {
                event_id: "event-automation-route-automation".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "correlation-automation-route-automation".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        )
        .expect("Automation adapter");
        ResponsibilityService::new(coworker_store)
            .execute(
                &OwnerCommandScope {
                    principal_id: OWNER.to_owned(),
                    workspace_id: WORKSPACE.to_owned(),
                    request_id: "automation-automation-route".to_owned(),
                },
                ResponsibilityCommand::CreateAutomation {
                    automation_id: "automation-automation-route".to_owned(),
                    name: "Route Automation".to_owned(),
                    definition,
                },
            )
            .expect("the paused Automation commits");
        "automation-automation-route".to_owned()
    }

    fn router(store: SqliteWorkspaceStore) -> axum::Router {
        let (_, stream_shutdown) = tokio::sync::watch::channel(false);
        super::build_operator_router(super::ApiState {
            store,
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: INCARNATION.to_owned(),
            lifecycle_blockers: Vec::new(),
            runtime_identity: Arc::new(
                storage_sqlite::RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                    .expect("test identity"),
            ),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_shutdown,
        })
    }

    async fn get(router: axum::Router, path: &str) -> (StatusCode, serde_json::Value) {
        let mut request = Request::builder()
            .method("GET")
            .uri(path)
            .header("X-Workspace-ID", WORKSPACE)
            .body(Body::empty())
            .expect("the request is well formed");
        // The Operator API only answers an authenticated local peer.
        request
            .extensions_mut()
            .insert(super::super::AuthenticatedLocalPeer);
        let response = router.oneshot(request).await.expect("the route responds");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("the response body is readable");
        let body = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, body)
    }

    #[tokio::test]
    async fn the_health_route_reports_the_projection_of_a_stored_automation() {
        let (_directory, store) = fixture();
        let automation_id = seed_definition(&store);
        let (status, body) = get(
            router(store),
            &format!("/v1/automations/{automation_id}/health"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["automation_id"], automation_id.as_str());
        assert_eq!(body["status"], "PAUSED");
        // The enrolled local Runtime is the trigger host of this Workspace.
        assert_eq!(body["host_ready"], true);
        assert_eq!(body["routine_health"]["routine_id"], "routine-automation-route");
        assert_eq!(body["routine_health"]["sample_size"], 0);
        assert_eq!(body["routine_health"]["drift_state"], "UNKNOWN");
        assert_eq!(body["drift_state"], "UNKNOWN");
        assert!(body["next_occurrence_at"].is_null());
    }

    #[tokio::test]
    async fn the_occurrence_route_reports_an_empty_history_and_a_missing_automation_is_not_found() {
        let (_directory, store) = fixture();
        let automation_id = seed_definition(&store);
        let (status, body) = get(
            router(store.clone()),
            &format!("/v1/automations/{automation_id}/occurrences"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["items"].as_array().map(Vec::len), Some(0));
        assert!(body["next_cursor"].is_null());

        let (missing, _) = get(
            router(store),
            "/v1/automations/automation-missing/health",
        )
        .await;
        assert_eq!(missing, StatusCode::NOT_FOUND);
    }
}
