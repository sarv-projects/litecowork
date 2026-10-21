//! Authenticated local Operator routes for Skills, SkillProposals, and
//! demonstration capture.
//!
//! Authority: [`docs/API.md`](../../../docs/API.md),
//! [`docs/CAPABILITY-FABRIC.md`](../../../docs/CAPABILITY-FABRIC.md),
//! [`docs/CAPABILITY-INVOCATIONS.md`](../../../docs/CAPABILITY-INVOCATIONS.md),
//! [`docs/LEARNING.md`](../../../docs/LEARNING.md).
//!
//! Every route is Workspace-scoped and owner-authenticated. Approving a
//! SkillProposal records the owner's decision only: it never installs a
//! package, never activates a Skill, and never grants authority. Package
//! publication stays with LiteSPM, whose package contract is not published, so
//! the routes report that publication is blocked instead of simulating it.

#![allow(clippy::result_large_err)] // Handlers return full Axum Responses to preserve status, body, and headers.

use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use storage_core::{
    ArtifactReadStore, DemonstrationCaptureConsent, DemonstrationCapturePolicy,
    DemonstrationSessionRecord, DemonstrationStatus, EnvironmentStore, EventDraft, McpAppBridgeEnvelope,
    McpAppCsp, McpAppFramePolicy, McpAppFrameRecord, RuntimeCapabilityHostPageRequest,
    RuntimeCapabilityRef, SensitiveRegionPolicy, SkillCapabilityRef, SkillGuardError, SkillKind,
    SkillOrigin, SkillProposalDecision, SkillProposalRecord, SkillProposalReviewer,
    SkillProposalSource, SkillProposalStatus, SkillRecord, SkillRedactionStatus,
    SkillRedactionSummary, SkillRevision, SkillStatus, build_app_frame, derive_skill_proposal_draft,
    validate_app_bridge_message,
};
use storage_sqlite::{
    AppFrameCreateRequest, DemonstrationAppendOutcome, DemonstrationAppendRequest,
    DemonstrationCreateRequest, DemonstrationTransition, DemonstrationTransitionRequest,
    SkillEventContext, SkillMutationRequest, SkillProposalCreateRequest, SkillProposalReviewRequest,
    SqliteEnvironmentStore, SqliteSkillStore,
};

const MAX_BODY_BYTES: usize = 256 * 1024;
const DEFAULT_PAGE_LIMIT: usize = 50;
const MAX_PAGE_LIMIT: usize = 200;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/skills", get(list_skills).post(create_skill))
        .route("/v1/skills/{skill_id}", get(get_skill).put(revise_skill))
        .route("/v1/skill-proposals", get(list_skill_proposals).post(create_skill_proposal))
        .route("/v1/skill-proposals/{proposal_id}/approve", post(approve_skill_proposal))
        .route("/v1/skill-proposals/{proposal_id}/reject", post(reject_skill_proposal))
        .route("/v1/demonstrations", post(start_demonstration))
        .route(
            "/v1/demonstrations/{demonstration_id}/actions",
            post(append_demonstration_action),
        )
        .route("/v1/demonstrations/{demonstration_id}/complete", post(complete_demonstration))
        .route("/v1/demonstrations/{demonstration_id}/pause", post(pause_demonstration))
        .route("/v1/demonstrations/{demonstration_id}/resume", post(resume_demonstration))
        .route("/v1/demonstrations/{demonstration_id}/abort", post(abort_demonstration))
        .route("/v1/mcp-apps/frames", post(issue_app_frame))
        .route("/v1/mcp-apps/messages", post(validate_app_message))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

// ---------------------------------------------------------------------------
// Wire shapes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillProposalQuery {
    status: Option<SkillProposalStatus>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

/// The capability identity one Skill revision is pinned to.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SkillCapabilityRefBody {
    capability_id: String,
    identity_kind: String,
    source: String,
    package_version: Option<String>,
    digest: String,
    component: String,
}

/// One immutable Skill revision as the owner writes it.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SkillRevisionBody {
    title: String,
    summary: String,
    manifest_digest: String,
    content_digest: String,
    content_ref: String,
    declared_capabilities: Vec<String>,
    declared_requirements: Vec<String>,
}

/// The body of a Skill definition write.
///
/// The body never carries a status: the host derives the persistable status
/// from [`storage_core::evaluate_skill_definition_write`], so a client cannot
/// promote a definition into the loadable state.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteSkillBody {
    skill_id: Option<String>,
    kind: SkillKind,
    capability_ref: SkillCapabilityRefBody,
    revision: SkillRevisionBody,
}

/// The body of an MCP App frame request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateAppFrameBody {
    /// The Runtime-observed MCP capability host that publishes the App.
    host_instance_id: String,
    /// The CSP domains the App itself declared.
    csp: McpAppCsp,
    /// Optional Task scope for the frame.
    task_id: Option<String>,
    /// How long the frame stays alive.
    ttl_seconds: u32,
}

/// The body of one App bridge message.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppBridgeMessageBody {
    frame_id: String,
    origin: String,
    nonce: String,
    method: String,
    payload: Value,
}

/// The body of one observed demonstration step.
///
/// The body carries a semantic description of the step. A raw pointer or
/// coordinate is never the sole authority, and the observed value is dropped by
/// the storage layer when the observation lands in a sensitive region.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DemonstrationActionBody {
    environment_id: String,
    sequence: u32,
    kind: String,
    target: String,
    value: Option<String>,
    sensitive: bool,
    coordinate_hint: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResourceRefBody {
    workspace_id: String,
    resource_id: String,
    revision_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSkillProposalBody {
    source_task_id: String,
    source_artifact_id: String,
    draft_resource_ref: ResourceRefBody,
    draft_digest: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartDemonstrationBody {
    workspace_id: String,
    environment_id: String,
    consent_semantic_capture: bool,
    capture_policy: DemonstrationCapturePolicyBody,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DemonstrationCapturePolicyBody {
    max_duration_ms: u32,
    max_actions: u32,
    max_trace_bytes: u32,
    allowed_environment_class: String,
    sensitive_region_policy: SensitiveRegionPolicy,
}

#[derive(Serialize)]
struct ResourceRefView {
    workspace_id: String,
    resource_id: String,
    revision_id: Option<String>,
}

/// The authenticated principal behind an owner decision.
///
/// The published contract models an approver as a principal reference, never
/// as a bare identifier that could be confused with a machine principal.
#[derive(Serialize, Debug, PartialEq)]
struct PrincipalRefView {
    principal_id: String,
    kind: String,
}

#[derive(Serialize)]
struct SkillProposalView {
    skill_proposal_id: String,
    workspace_id: String,
    source: SkillProposalSource,
    source_task_id: String,
    source_artifact_id: String,
    draft_resource_ref: ResourceRefView,
    draft_digest: String,
    redaction_status: SkillRedactionStatus,
    status: SkillProposalStatus,
    approved_by: Option<PrincipalRefView>,
    published_capability_ref: Option<Value>,
    placeholders: Vec<Value>,
    declared_capabilities: Vec<String>,
    drift: Option<Value>,
    created_at: String,
    updated_at: String,
    version: u64,
}

#[derive(Serialize)]
struct SkillProposalPageView {
    items: Vec<SkillProposalView>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct DemonstrationCapturePolicyView {
    max_duration_ms: u32,
    max_actions: u32,
    max_trace_bytes: u32,
    allowed_environment_class: String,
    sensitive_region_policy: SensitiveRegionPolicy,
}

#[derive(Serialize)]
struct DemonstrationSessionView {
    demonstration_session_id: String,
    workspace_id: String,
    environment_id: String,
    status: DemonstrationStatus,
    capture_policy: DemonstrationCapturePolicyView,
    captured_action_count: u32,
    captured_trace_bytes: u32,
    started_at: String,
    completed_at: Option<String>,
    trace_resource_id: Option<String>,
    skill_proposal_id: Option<String>,
    version: u64,
}

/// One Skill definition as the owner sees it.
#[derive(Serialize)]
struct SkillView {
    skill_id: String,
    workspace_id: String,
    kind: SkillKind,
    status: SkillStatus,
    capability_ref: SkillCapabilityRefBody,
    current_revision: u64,
    /// The current immutable revision. Never inline instruction text.
    revision: Option<SkillRevisionView>,
    created_at: String,
    updated_at: String,
    version: u64,
}

/// One immutable Skill revision as the owner sees it.
#[derive(Serialize)]
struct SkillRevisionView {
    revision: u64,
    title: String,
    summary: String,
    manifest_digest: String,
    content_digest: String,
    content_ref: String,
    declared_capabilities: Vec<String>,
    declared_requirements: Vec<String>,
    created_at: String,
    created_by: String,
}

#[derive(Serialize)]
struct SkillPageView {
    items: Vec<SkillView>,
    next_cursor: Option<String>,
}

/// One host-issued MCP App frame.
///
/// The nonce is the host-issued per-frame secret the desktop injects into the
/// isolated frame. It is bound to exactly one frame instance.
#[derive(Serialize)]
struct AppFrameView {
    frame_id: String,
    workspace_id: String,
    origin: String,
    sandbox: Vec<String>,
    csp: String,
    connect_domains: Vec<String>,
    resource_domains: Vec<String>,
    bridge_methods: Vec<String>,
    server_identity: String,
    resource_uri: String,
    content_digest: String,
    package_version: Option<String>,
    task_id: Option<String>,
    expires_at: String,
    nonce: String,
}

/// One accepted App bridge message.
#[derive(Serialize)]
struct AppBridgeRequestView {
    frame_id: String,
    method: String,
    payload: Value,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Skill request is invalid",
    )
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Skill storage is unavailable",
    )
}

fn not_found() -> Response {
    operator_error(
        StatusCode::NOT_FOUND,
        "NOT_FOUND",
        "Skill record is unavailable",
    )
}

fn conflict() -> Response {
    operator_error(
        StatusCode::CONFLICT,
        "CONFLICT",
        "Skill version or idempotency key conflicts with current state",
    )
}

fn forbidden() -> Response {
    operator_error(
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
        "Workspace access is unavailable",
    )
}

/// A deterministic domain rejection, reported with its stable reason code.
fn invalid_guard(error: SkillGuardError) -> Response {
    operator_error_with_details(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Skill request is invalid",
        Some(json!({"reason": error.reason_code()})),
    )
}

fn storage_error(error: StoreError) -> Response {
    match error {
        StoreError::NotFound => not_found(),
        StoreError::Conflict { .. } | StoreError::IdempotencyConflict => conflict(),
        StoreError::Invalid(message) => operator_error_with_details(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Skill request is invalid",
            Some(json!({"reason": message})),
        ),
        _ => unavailable(),
    }
}

fn adapter(state: &ApiState, correlation_id: &str) -> Result<SqliteSkillStore, Response> {
    let event = event_context(&state.runtime_id, correlation_id).map_err(|_| unavailable())?;
    SqliteSkillStore::new(
        state.store.clone(),
        SkillEventContext {
            event_id: event.event_id,
            origin_runtime_id: event.origin_runtime_id,
            hlc_timestamp: event.hlc_timestamp,
            correlation_id: event.correlation_id,
            causation_id: event.causation_id,
            recorded_at: event.recorded_at,
        },
    )
    .map_err(|_| unavailable())
}

fn parse_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    if body.len() > MAX_BODY_BYTES {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Skill request exceeds the local body limit",
        ));
    }
    serde_json::from_slice(body).map_err(|_| invalid())
}

fn expected_version(headers: &HeaderMap) -> Result<u64, Response> {
    let text = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(invalid)?
        .trim();
    let text = text.trim_matches('"');
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    text.parse::<u64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(invalid)
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn decode_cursor(
    encoded: &str,
    workspace: &str,
    scope: &str,
) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    let version = cursor.get("version").and_then(serde_json::Value::as_u64).unwrap_or(0);
    let cursor_workspace = cursor
        .get("workspace_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let cursor_scope = cursor.get("scope").and_then(serde_json::Value::as_str).unwrap_or_default();
    let created_at = cursor
        .get("created_at")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let entity_id = cursor
        .get("entity_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if version != 1
        || cursor_workspace != workspace
        || cursor_scope != scope
        || !valid_id(&entity_id)
        || created_at.len() > 64
    {
        return Err(invalid());
    }
    Ok((created_at, entity_id))
}

fn encode_cursor(
    workspace: &str,
    scope: &str,
    created_at: &str,
    entity_id: &str,
) -> Result<String, Response> {
    let bytes = serde_json::to_vec(&json!({
        "version": 1,
        "workspace_id": workspace,
        "scope": scope,
        "created_at": created_at,
        "entity_id": entity_id,
    }))
    .map_err(|_| unavailable())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn page_limit(limit: Option<usize>) -> Result<usize, Response> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(invalid());
    }
    Ok(limit)
}

fn proposal_view(proposal: &SkillProposalRecord) -> SkillProposalView {
    SkillProposalView {
        skill_proposal_id: proposal.proposal_id.clone(),
        workspace_id: proposal.workspace_id.clone(),
        source: proposal.source,
        source_task_id: proposal.source_task_id.clone(),
        source_artifact_id: proposal.source_artifact_id.clone(),
        draft_resource_ref: ResourceRefView {
            workspace_id: proposal.workspace_id.clone(),
            resource_id: proposal.draft_resource_ref.clone(),
            revision_id: None,
        },
        draft_digest: proposal.draft_digest.clone(),
        redaction_status: proposal.redaction.status,
        status: proposal.status,
        approved_by: proposal.approved_by.as_ref().map(|principal_id| PrincipalRefView {
            principal_id: principal_id.clone(),
            kind: "USER".to_owned(),
        }),
        published_capability_ref: None,
        placeholders: proposal
            .placeholders
            .iter()
            .map(|placeholder| {
                json!({
                    "name": placeholder.name,
                    "kind": placeholder.kind,
                    "sample": placeholder.sample,
                })
            })
            .collect(),
        declared_capabilities: proposal.declared_capabilities.clone(),
        drift: proposal.drift.as_ref().map(|drift| {
            json!({
                "expected_manifest_digest": drift.expected_manifest_digest,
                "observed_manifest_digest": drift.observed_manifest_digest,
                "drifted_resources": drift.drifted_resources,
                "detected_at": drift.detected_at,
            })
        }),
        created_at: proposal.created_at.clone(),
        updated_at: proposal.updated_at.clone(),
        version: proposal.version,
    }
}

fn skill_view(skill: &SkillRecord) -> SkillView {
    SkillView {
        skill_id: skill.skill_id.clone(),
        workspace_id: skill.workspace_id.clone(),
        kind: skill.kind,
        status: skill.status,
        capability_ref: SkillCapabilityRefBody {
            capability_id: skill.capability_ref.capability_id.clone(),
            identity_kind: skill.capability_ref.identity_kind.clone(),
            source: skill.capability_ref.source.clone(),
            package_version: skill.capability_ref.package_version.clone(),
            digest: skill.capability_ref.digest.clone(),
            component: skill.capability_ref.component.clone(),
        },
        current_revision: skill.current_revision,
        revision: skill.current_revision_record().map(|revision| SkillRevisionView {
            revision: revision.revision,
            title: revision.title.clone(),
            summary: revision.summary.clone(),
            manifest_digest: revision.manifest_digest.clone(),
            content_digest: revision.content_digest.clone(),
            content_ref: revision.content_ref.clone(),
            declared_capabilities: revision.declared_capabilities.clone(),
            declared_requirements: revision.declared_requirements.clone(),
            created_at: revision.created_at.clone(),
            created_by: revision.created_by.clone(),
        }),
        created_at: skill.created_at.clone(),
        updated_at: skill.updated_at.clone(),
        version: skill.version,
    }
}

fn demonstration_view(session: &DemonstrationSessionRecord) -> DemonstrationSessionView {    DemonstrationSessionView {
        demonstration_session_id: session.session_id.clone(),
        workspace_id: session.workspace_id.clone(),
        environment_id: session.environment_id.clone(),
        status: session.status,
        capture_policy: DemonstrationCapturePolicyView {
            max_duration_ms: session.capture_policy.max_duration_ms,
            max_actions: session.capture_policy.max_actions,
            max_trace_bytes: session.capture_policy.max_trace_bytes,
            allowed_environment_class: session.capture_policy.allowed_environment_class.clone(),
            sensitive_region_policy: session.capture_policy.sensitive_region_policy,
        },
        captured_action_count: session.captured_action_count,
        captured_trace_bytes: session.captured_trace_bytes,
        started_at: session.started_at.clone(),
        completed_at: session.completed_at.clone(),
        trace_resource_id: session.trace_resource_id.clone(),
        skill_proposal_id: session.skill_proposal_id.clone(),
        version: session.version,
    }
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

// ---------------------------------------------------------------------------
// Skill definition routes
// ---------------------------------------------------------------------------

async fn list_skills(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<SkillQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = page_limit(query.limit)?;
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| decode_cursor(cursor, &workspace.workspace_id, "skill"))
        .transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let page = store
        .list_skills(&state.principal_id, &workspace.workspace_id, after, limit)
        .map_err(storage_error)?;
    let items: Vec<SkillView> = page.items.iter().map(skill_view).collect();
    let next_cursor = match page.next {
        Some((created_at, entity_id)) => Some(encode_cursor(
            &workspace.workspace_id,
            "skill",
            &created_at,
            &entity_id,
        )?),
        None => None,
    };
    Ok(no_store(
        Json(SkillPageView { items, next_cursor }).into_response(),
    ))
}

async fn get_skill(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(skill_id): Path<String>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&skill_id) {
        return Err(invalid());
    }
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let skill = store
        .get_skill(&state.principal_id, &workspace.workspace_id, &skill_id)
        .map_err(storage_error)?
        .ok_or_else(not_found)?;
    Ok(no_store(Json(skill_view(&skill)).into_response()))
}

async fn create_skill(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: WriteSkillBody = parse_body(&body)?;
    let request_id = super::idempotency_key(&headers)?;
    let skill_id = match body.skill_id.as_deref() {
        Some(skill_id) => {
            if !valid_id(skill_id) {
                return Err(invalid());
            }
            skill_id.to_owned()
        }
        None => idempotent_id(
            "skl",
            &state.principal_id,
            &request_id,
            &format!("skill.create.{}", workspace.workspace_id),
        ),
    };
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let now = store.recorded_at();
    let revision = SkillRevision {
        revision: 1,
        title: body.revision.title,
        summary: body.revision.summary,
        manifest_digest: body.revision.manifest_digest,
        content_digest: body.revision.content_digest,
        content_ref: body.revision.content_ref,
        declared_capabilities: body.revision.declared_capabilities,
        declared_requirements: body.revision.declared_requirements,
        origin: SkillOrigin::LocalProposal {
            proposal_id: format!("owner-authored:{}", skill_id),
        },
        created_at: now.clone(),
        created_by: state.principal_id.clone(),
    };
    // A new definition is always held as a draft. No client-supplied status can
    // promote it: the loadable state needs a scoped CapabilityGrant.
    let status = storage_core::evaluate_skill_definition_write(None, SkillStatus::Draft)
        .map_err(|error| invalid_guard(error))?;
    let skill = SkillRecord {
        skill_id: skill_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        kind: body.kind,
        status,
        capability_ref: SkillCapabilityRef {
            capability_id: body.capability_ref.capability_id,
            identity_kind: body.capability_ref.identity_kind,
            source: body.capability_ref.source,
            package_version: body.capability_ref.package_version,
            digest: body.capability_ref.digest,
            component: body.capability_ref.component,
        },
        current_revision: 1,
        revisions: vec![revision],
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };
    let committed = store
        .put_skill(SkillMutationRequest {
            principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            request_id: request_id.clone(),
            request_payload: json!({
                "operation": "skill.create.v1",
                "workspace_id": workspace.workspace_id,
                "skill_id": skill_id,
                "kind": skill.kind,
                "capability_id": skill.capability_ref.capability_id,
            }),
            expected_version: None,
            skill,
        })
        .map_err(storage_error)?;
    Ok(no_store(
        (StatusCode::CREATED, Json(skill_view(&committed.skill))).into_response(),
    ))
}

async fn revise_skill(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(skill_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&skill_id) {
        return Err(invalid());
    }
    let body: WriteSkillBody = parse_body(&body)?;
    if body.skill_id.as_deref().is_some_and(|value| value != skill_id) {
        return Err(invalid());
    }
    let request_id = super::idempotency_key(&headers)?;
    let expected = expected_version(&headers)?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let current = store
        .get_skill(&state.principal_id, &workspace.workspace_id, &skill_id)
        .map_err(storage_error)?
        .ok_or_else(not_found)?;
    if current.version != expected {
        return Err(conflict());
    }
    // A revision may only narrow the lifecycle. The manifest digest of the new
    // revision differs from the pinned one, so any grant that pinned the old
    // revision stops matching and the Skill is no longer loadable until it is
    // reviewed again.
    let status = storage_core::evaluate_skill_definition_write(Some(&current), current.status)
        .map_err(|error| invalid_guard(error))?;
    let now = store.recorded_at();
    // The new revision follows the current head, so the pinned manifest digest
    // of the previous revision can never be reused.
    let revision_number = current.current_revision + 1;
    let mut revisions = current.revisions.clone();
    revisions.push(SkillRevision {
        revision: revision_number,
        title: body.revision.title,
        summary: body.revision.summary,
        manifest_digest: body.revision.manifest_digest,
        content_digest: body.revision.content_digest,
        content_ref: body.revision.content_ref,
        declared_capabilities: body.revision.declared_capabilities,
        declared_requirements: body.revision.declared_requirements,
        origin: current
            .current_revision_record()
            .map(|revision| revision.origin.clone())
            .unwrap_or(SkillOrigin::LocalProposal {
                proposal_id: format!("owner-authored:{}", skill_id),
            }),
        created_at: now.clone(),
        created_by: state.principal_id.clone(),
    });
    let mut skill = current.clone();
    skill.status = status;
    skill.current_revision = revision_number;
    skill.revisions = revisions;
    skill.updated_at = now;
    let committed = store
        .put_skill(SkillMutationRequest {
            principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            request_id: request_id.clone(),
            request_payload: json!({
                "operation": "skill.revise.v1",
                "workspace_id": workspace.workspace_id,
                "skill_id": skill_id,
                "revision": revision_number,
                "expected_version": expected,
            }),
            expected_version: Some(expected),
            skill,
        })
        .map_err(storage_error)?;
    Ok(no_store(Json(skill_view(&committed.skill)).into_response()))
}

// ---------------------------------------------------------------------------
// SkillProposal routes
// ---------------------------------------------------------------------------

async fn list_skill_proposals(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<SkillProposalQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = page_limit(query.limit)?;
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| decode_cursor(cursor, &workspace.workspace_id, "skill-proposal"))
        .transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let page = store
        .list_skill_proposals(
            &state.principal_id,
            &workspace.workspace_id,
            query.status,
            after,
            limit,
        )
        .map_err(storage_error)?;
    let items: Vec<SkillProposalView> = page.items.iter().map(proposal_view).collect();
    let next_cursor = match page.next {
        Some((created_at, entity_id)) => Some(encode_cursor(
            &workspace.workspace_id,
            "skill-proposal",
            &created_at,
            &entity_id,
        )?),
        None => None,
    };
    Ok(no_store(
        Json(SkillProposalPageView { items, next_cursor }).into_response(),
    ))
}

async fn create_skill_proposal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: CreateSkillProposalBody = parse_body(&body)?;
    if body.draft_resource_ref.workspace_id != workspace.workspace_id {
        return Err(forbidden());
    }
    if !valid_id(&body.source_task_id)
        || !valid_id(&body.source_artifact_id)
        || !valid_id(&body.draft_resource_ref.resource_id)
        || !storage_core::is_sha256_digest(&body.draft_digest)
    {
        return Err(invalid());
    }
    let request_id = super::idempotency_key(&headers)?;
    let proposal_id = idempotent_id(
        "sp",
        &state.principal_id,
        &request_id,
        &format!("skill-proposal.create.{}", workspace.workspace_id),
    );
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;

    // The redaction attestation is bound to the exact draft digest. A draft that
    // cannot prove it was redacted stays a DRAFT and never becomes reviewable.
    let attestation = state
        .store
        .get_resource_record(&workspace.workspace_id, &body.draft_resource_ref.resource_id)
        .map_err(|_| unavailable())?
        .and_then(|resource| {
            resource
                .provenance
                .get("redaction")
                .cloned()
                .filter(|value| {
                    value.get("status").and_then(serde_json::Value::as_str) == Some("PASSED")
                        && value.get("digest").and_then(serde_json::Value::as_str)
                            == Some(body.draft_digest.as_str())
                })
        });
    let redaction = match attestation {
        Some(_) => SkillRedactionSummary {
            status: SkillRedactionStatus::Passed,
            redacted_spans: 0,
            finding_codes: vec!["REDACTION_ATTESTED_BY_CAPTURE".to_owned()],
            redacted_digest: body.draft_digest.clone(),
        },
        None => SkillRedactionSummary {
            status: SkillRedactionStatus::Pending,
            redacted_spans: 0,
            finding_codes: vec!["REDACTION_NOT_ATTESTED".to_owned()],
            redacted_digest: String::new(),
        },
    };
    let status = if redaction.status == SkillRedactionStatus::Passed {
        SkillProposalStatus::Review
    } else {
        SkillProposalStatus::Draft
    };
    let now = store.recorded_at();
    let proposal = SkillProposalRecord {
        proposal_id: proposal_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        source: SkillProposalSource::OwnerAuthored,
        source_task_id: body.source_task_id.clone(),
        source_artifact_id: body.source_artifact_id.clone(),
        draft_resource_ref: body.draft_resource_ref.resource_id.clone(),
        draft_digest: body.draft_digest.clone(),
        redaction,
        placeholders: Vec::new(),
        declared_capabilities: Vec::new(),
        status,
        proposed_by: state.principal_id.clone(),
        approved_by: None,
        published_capability_ref: None,
        drift: None,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };
    let committed = store
        .create_skill_proposal(SkillProposalCreateRequest {
            principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            request_id: request_id.clone(),
            request_payload: json!({
                "operation": "skill.proposal.create.v1",
                "workspace_id": workspace.workspace_id,
                "proposal_id": proposal_id,
                "source_task_id": body.source_task_id,
                "source_artifact_id": body.source_artifact_id,
                "draft_resource_ref": body.draft_resource_ref,
                "draft_digest": body.draft_digest,
            }),
            proposal,
        })
        .map_err(storage_error)?;
    Ok(no_store(
        (StatusCode::CREATED, Json(proposal_view(&committed.proposal))).into_response(),
    ))
}

async fn approve_skill_proposal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(proposal_id): Path<String>,
) -> Result<Response, Response> {
    review_skill_proposal(state, headers, proposal_id, SkillProposalDecision::Approve).await
}

async fn reject_skill_proposal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(proposal_id): Path<String>,
) -> Result<Response, Response> {
    review_skill_proposal(state, headers, proposal_id, SkillProposalDecision::Reject).await
}

async fn review_skill_proposal(
    state: ApiState,
    headers: HeaderMap,
    proposal_id: String,
    decision: SkillProposalDecision,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&proposal_id) {
        return Err(invalid());
    }
    let request_id = super::idempotency_key(&headers)?;
    let expected = expected_version(&headers)?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let committed = store
        .review_skill_proposal(SkillProposalReviewRequest {
            principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            request_id: request_id.clone(),
            request_payload: json!({
                "operation": "skill.proposal.review.v1",
                "workspace_id": workspace.workspace_id,
                "proposal_id": proposal_id,
                "decision": decision,
                "expected_version": expected,
            }),
            proposal_id: proposal_id.clone(),
            expected_version: expected,
            decision,
            reviewer: SkillProposalReviewer {
                principal_id: state.principal_id.clone(),
                principal_kind: "USER".to_owned(),
                assurance_level: "APPROVE_SAFE".to_owned(),
            },
        })
        .map_err(storage_error)?;
    let status = if decision == SkillProposalDecision::Approve {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };
    Ok(no_store(
        (status, Json(proposal_view(&committed.proposal))).into_response(),
    ))
}

// ---------------------------------------------------------------------------
// MCP App sandbox routes
// ---------------------------------------------------------------------------

/// The origin LiteCowork serves one isolated App frame from.
///
/// The origin is host-assigned. A client never declares it, so a forged App
/// cannot claim an origin that another App already owns.
fn app_frame_origin(frame_id: &str) -> String {
    format!("https://app.litecowork.local/mcp-app/{frame_id}")
}

async fn issue_app_frame(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: CreateAppFrameBody = parse_body(&body)?;
    if !valid_id(&body.host_instance_id) {
        return Err(invalid());
    }
    if body.ttl_seconds == 0 || body.ttl_seconds > 3_600 {
        return Err(invalid());
    }
    if let Some(task_id) = body.task_id.as_deref() {
        if !valid_id(task_id) {
            return Err(invalid());
        }
    }
    // The App's server identity, resource URI, and content digest come from the
    // MCP capability host this Runtime already observes. The host-authenticated
    // identity is never taken from the request body, so a client cannot claim
    // another server's identity.
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    // The host observation is compared against the current instant, so a
    // correlation identity would make every observation look expired.
    let observed_at = store.recorded_at();
    let hosts = state
        .store
        .list_runtime_capability_hosts(RuntimeCapabilityHostPageRequest {
            owner_principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            runtime_id: state.runtime_id.clone(),
            expected_incarnation_id: Some(state.local_incarnation_id.clone()),
            now: observed_at.clone(),
            after: None,
            limit: 200,
        })
        .map_err(|_| unavailable())?;
    let Some(host) = hosts
        .iter()
        .find(|host| host.host_instance_id == body.host_instance_id)
    else {
        return Err(operator_error_with_details(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "The MCP provider that publishes this App is not observed by this Runtime",
            Some(json!({
                "reason": "MCP_PROVIDER_NOT_OBSERVED",
                "host_instance_id": body.host_instance_id,
            })),
        ));
    };
    if host.health != "HEALTHY" {
        return Err(operator_error_with_details(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "The MCP provider that publishes this App is not healthy",
            Some(json!({
                "reason": "MCP_PROVIDER_UNHEALTHY",
                "host_instance_id": body.host_instance_id,
                "health": host.health,
            })),
        ));
    }
    let RuntimeCapabilityRef::McpSkill {
        capability_id: _,
        source,
        digest,
        component,
    } = &host.capability_ref
    else {
        return Err(operator_error_with_details(
            StatusCode::CONFLICT,
            "NOT_AN_APP_PUBLISHER",
            "The observed capability host does not publish an MCP App",
            Some(json!({
                "reason": "NOT_AN_APP_PUBLISHER",
                "host_instance_id": body.host_instance_id,
            })),
        ));
    };
    let request_id = super::idempotency_key(&headers)?;
    let frame_id = idempotent_id(
        "frm",
        &state.principal_id,
        &request_id,
        &format!("mcp-app-frame.{}", workspace.workspace_id),
    );
    let issued_at = observed_at.clone();
    let expires_at = time::OffsetDateTime::parse(&issued_at, &time::format_description::well_known::Rfc3339)
        .ok()
        .and_then(|instant| instant.checked_add(time::Duration::seconds(i64::from(body.ttl_seconds))))
        .and_then(|instant| {
            instant
                .format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .ok_or_else(invalid)?;
    let policy = McpAppFramePolicy {
        frame_id: frame_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        server_identity: source.clone(),
        resource_uri: component.clone(),
        content_digest: digest.clone(),
        package_version: None,
        origin: app_frame_origin(&frame_id),
        csp: body.csp,
        task_id: body.task_id.clone(),
        expires_at: expires_at.clone(),
    };
    let descriptor = build_app_frame(&policy).map_err(|error| invalid_guard(error))?;
    let nonce = new_id("nonce").map_err(|_| unavailable())?;
    let frame = McpAppFrameRecord {
        frame_id: frame_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        descriptor: descriptor.clone(),
        nonce: nonce.clone(),
        created_at: issued_at.clone(),
        updated_at: issued_at,
        version: 1,
    };
    let committed = store
        .put_app_frame(AppFrameCreateRequest {
            principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            request_id: request_id.clone(),
            request_payload: json!({
                "operation": "mcp-app.frame.issue.v1",
                "workspace_id": workspace.workspace_id,
                "frame_id": frame_id,
                "host_instance_id": body.host_instance_id,
                "server_identity": source,
                "resource_uri": component,
                "content_digest": digest,
                "expires_at": expires_at,
            }),
            frame,
        })
        .map_err(storage_error)?;
    Ok(no_store(
        (
            StatusCode::CREATED,
            Json(AppFrameView {
                frame_id: committed.frame.frame_id.clone(),
                workspace_id: committed.frame.workspace_id.clone(),
                origin: committed.frame.descriptor.origin.clone(),
                sandbox: committed.frame.descriptor.sandbox.clone(),
                csp: committed.frame.descriptor.csp.clone(),
                connect_domains: committed.frame.descriptor.connect_domains.clone(),
                resource_domains: committed.frame.descriptor.resource_domains.clone(),
                bridge_methods: committed.frame.descriptor.bridge_methods.clone(),
                server_identity: committed.frame.descriptor.server_identity.clone(),
                resource_uri: committed.frame.descriptor.resource_uri.clone(),
                content_digest: committed.frame.descriptor.content_digest.clone(),
                package_version: committed.frame.descriptor.package_version.clone(),
                task_id: committed.frame.descriptor.task_id.clone(),
                expires_at: committed.frame.descriptor.expires_at.clone(),
                nonce: committed.frame.nonce.clone(),
            }),
        )
            .into_response(),
    ))
}

async fn validate_app_message(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: AppBridgeMessageBody = parse_body(&body)?;
    if !valid_id(&body.frame_id) {
        return Err(invalid());
    }
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let frame = store
        .get_app_frame(&state.principal_id, &workspace.workspace_id, &body.frame_id)
        .map_err(storage_error)?
        .ok_or_else(not_found)?;
    let envelope = McpAppBridgeEnvelope {
        frame_id: body.frame_id,
        origin: body.origin,
        nonce: body.nonce,
        method: body.method,
        payload: body.payload,
    };
    let now = store.recorded_at();
    // The host-issued frame and its nonce decide. A message from another origin,
    // a replayed nonce, an unknown method, or a payload carrying a credential
    // is refused before anything is dispatched.
    let request = validate_app_bridge_message(&frame.descriptor, &envelope, &frame.nonce, &now)
        .map_err(|error| invalid_guard(error))?;
    Ok(no_store(
        Json(AppBridgeRequestView {
            frame_id: request.frame_id,
            method: request.method,
            payload: request.payload,
        })
        .into_response(),
    ))
}

async fn append_demonstration_action(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(demonstration_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&demonstration_id) {
        return Err(invalid());
    }
    let body: DemonstrationActionBody = parse_body(&body)?;
    if !valid_id(&body.environment_id) {
        return Err(invalid());
    }
    if body.sequence == 0 {
        return Err(invalid());
    }
    if body.kind.trim().is_empty() || body.kind.len() > 120 {
        return Err(invalid());
    }
    if body.target.trim().is_empty() || body.target.len() > 512 {
        return Err(invalid());
    }
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let session = store
        .get_demonstration_session(
            &state.principal_id,
            &workspace.workspace_id,
            &demonstration_id,
        )
        .map_err(storage_error)?
        .ok_or_else(not_found)?;
    // The observation must name the very Environment the capture runs in. An
    // observation from another Environment is refused before it reaches the
    // storage transaction.
    if session.environment_id != body.environment_id {
        return Err(operator_error_with_details(
            StatusCode::CONFLICT,
            "ENVIRONMENT_MISMATCH",
            "The observation does not come from the capture Environment",
            Some(json!({
                "reason": "ENVIRONMENT_MISMATCH",
                "demonstration_session_id": demonstration_id,
            })),
        ));
    }
    let action = storage_core::DemonstrationAction {
        sequence: body.sequence,
        kind: body.kind,
        target: body.target,
        value: body.value,
        sensitive: body.sensitive,
        coordinate_hint: body.coordinate_hint,
    };
    let outcome = append_captured_action(
        &state,
        &workspace.workspace_id,
        &demonstration_id,
        &body.environment_id,
        action,
    )
    .map_err(storage_error)?;
    match outcome {
        DemonstrationAppendOutcome::Recorded { session } => {
            Ok(no_store(Json(demonstration_view(&session)).into_response()))
        }
        DemonstrationAppendOutcome::Refused { reason } => Err(operator_error_with_details(
            StatusCode::CONFLICT,
            "CAPTURE_REFUSED",
            "The capture refused the observation",
            Some(json!({"reason": reason})),
        )),
    }
}

// ---------------------------------------------------------------------------
// Demonstration capture routes
// ---------------------------------------------------------------------------

async fn start_demonstration(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let body: StartDemonstrationBody = parse_body(&body)?;
    if body.workspace_id != workspace.workspace_id {
        return Err(forbidden());
    }
    if !body.consent_semantic_capture {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "CONSENT_REQUIRED",
            "Semantic capture requires explicit owner consent",
        ));
    }
    if !valid_id(&body.environment_id) {
        return Err(invalid());
    }
    let policy = DemonstrationCapturePolicy {
        max_duration_ms: body.capture_policy.max_duration_ms,
        max_actions: body.capture_policy.max_actions,
        max_trace_bytes: body.capture_policy.max_trace_bytes,
        allowed_environment_class: body.capture_policy.allowed_environment_class.clone(),
        sensitive_region_policy: body.capture_policy.sensitive_region_policy,
    };
    if policy.validate().is_err() {
        return Err(invalid());
    }
    // The capture runs inside exactly one Environment. Its class must match the
    // immutable policy, and it must be owned by the Task whose workflow is
    // demonstrated: a SkillProposal always names its source Task.
    let environment = SqliteEnvironmentStore::new(state.store.clone())
        .get_environment(&workspace.workspace_id, &body.environment_id)
        .map_err(|_| unavailable())?
        .ok_or_else(not_found)?;
    let environment_class = serde_json::to_value(environment.config().class)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    if environment_class != policy.allowed_environment_class {
        return Err(operator_error_with_details(
            StatusCode::CONFLICT,
            "ENVIRONMENT_CLASS_MISMATCH",
            "The capture Environment class does not match the capture policy",
            Some(json!({
                "reason": "ENVIRONMENT_CLASS_MISMATCH",
                "environment_class": environment_class,
                "allowed_environment_class": policy.allowed_environment_class,
            })),
        ));
    }
    let Some(source_task_id) = environment
        .identity()
        .owner
        .task_id
        .as_ref()
        .map(|task_id| task_id.as_str().to_owned())
    else {
        return Err(operator_error_with_details(
            StatusCode::CONFLICT,
            "SOURCE_REFERENCE_REQUIRED",
            "The capture Environment must be owned by the Task whose workflow is demonstrated",
            Some(json!({"reason": "SOURCE_REFERENCE_REQUIRED"})),
        ));
    };
    let request_id = super::idempotency_key(&headers)?;
    let session_id = idempotent_id(
        "demo",
        &state.principal_id,
        &request_id,
        &format!("demonstration.start.{}", workspace.workspace_id),
    );
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let now = store.recorded_at();
    let session = DemonstrationSessionRecord {
        session_id: session_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        environment_id: body.environment_id.clone(),
        source_task_id: Some(source_task_id),
        source_artifact_id: None,
        status: DemonstrationStatus::Capturing,
        capture_policy: policy,
        consent: DemonstrationCaptureConsent {
            granted_by_principal_id: state.principal_id.clone(),
            granted_at: now.clone(),
            scope: "SEMANTIC_CAPTURE".to_owned(),
        },
        captured_action_count: 0,
        captured_trace_bytes: 0,
        started_at: now.clone(),
        completed_at: None,
        trace_resource_id: None,
        skill_proposal_id: None,
        actions: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };
    let committed = store
        .create_demonstration_session(DemonstrationCreateRequest {
            principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            request_id: request_id.clone(),
            request_payload: json!({
                "operation": "demonstration.start.v1",
                "workspace_id": workspace.workspace_id,
                "demonstration_session_id": session_id,
                "environment_id": body.environment_id,
                "capture_policy": body.capture_policy,
            }),
            session,
        })
        .map_err(storage_error)?;
    Ok(no_store(
        (
            StatusCode::CREATED,
            Json(demonstration_view(&committed.session)),
        )
            .into_response(),
    ))
}

async fn pause_demonstration(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(demonstration_id): Path<String>,
) -> Result<Response, Response> {
    transition_demonstration(
        state,
        headers,
        demonstration_id,
        DemonstrationTransition::Pause,
    )
    .await
}

async fn resume_demonstration(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(demonstration_id): Path<String>,
) -> Result<Response, Response> {
    transition_demonstration(
        state,
        headers,
        demonstration_id,
        DemonstrationTransition::Resume,
    )
    .await
}

async fn abort_demonstration(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(demonstration_id): Path<String>,
) -> Result<Response, Response> {
    transition_demonstration(
        state,
        headers,
        demonstration_id,
        DemonstrationTransition::Abort,
    )
    .await
}

async fn transition_demonstration(
    state: ApiState,
    headers: HeaderMap,
    demonstration_id: String,
    transition: DemonstrationTransition,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&demonstration_id) {
        return Err(invalid());
    }
    let request_id = super::idempotency_key(&headers)?;
    let expected = expected_version(&headers)?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let committed = store
        .transition_demonstration_session(DemonstrationTransitionRequest {
            principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            request_id: request_id.clone(),
            request_payload: json!({
                "operation": "demonstration.transition.v1",
                "workspace_id": workspace.workspace_id,
                "demonstration_session_id": demonstration_id,
                "transition": transition.as_str(),
                "expected_version": expected,
            }),
            session_id: demonstration_id.clone(),
            expected_version: expected,
            transition,
        })
        .map_err(storage_error)?;
    Ok(no_store(
        Json(demonstration_view(&committed.session)).into_response(),
    ))
}

async fn complete_demonstration(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(demonstration_id): Path<String>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&demonstration_id) {
        return Err(invalid());
    }
    let request_id = super::idempotency_key(&headers)?;
    let expected = expected_version(&headers)?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = adapter(&state, &correlation)?;
    let session = store
        .get_demonstration_session(
            &state.principal_id,
            &workspace.workspace_id,
            &demonstration_id,
        )
        .map_err(storage_error)?
        .ok_or_else(not_found)?;
    if session.version != expected {
        return Err(conflict());
    }
    if session.captured_action_count == 0 {
        return Err(operator_error_with_details(
            StatusCode::CONFLICT,
            "EMPTY_CAPTURE",
            "The demonstration recorded no steps to convert",
            Some(json!({"reason": "EMPTY_CAPTURE"})),
        ));
    }
    let (source_task_id, source_artifact_id) = match (
        session.source_task_id.clone(),
        session.source_artifact_id.clone(),
    ) {
        (Some(task_id), Some(artifact_id)) => (task_id, artifact_id),
        _ => {
            // The demonstrated result is the Task's own current output Artifact.
            // The capture never invents one and never borrows another Task's.
            let Some(source_task_id) = session.source_task_id.clone() else {
                return Err(operator_error_with_details(
                    StatusCode::CONFLICT,
                    "SOURCE_REFERENCE_REQUIRED",
                    "The demonstration must name the Task it demonstrates before it can become a SkillProposal",
                    Some(json!({"reason": "SOURCE_REFERENCE_REQUIRED"})),
                ));
            };
            let artifacts = state
                .store
                .list_artifacts_page(
                    &workspace.workspace_id,
                    None,
                    Some(&source_task_id),
                    None,
                    None,
                    1,
                )
                .map_err(|_| unavailable())?;
            let Some(artifact) = artifacts.first() else {
                return Err(operator_error_with_details(
                    StatusCode::CONFLICT,
                    "SOURCE_REFERENCE_REQUIRED",
                    "The demonstrated Task has no output Artifact to bind the SkillProposal to",
                    Some(json!({"reason": "SOURCE_REFERENCE_REQUIRED"})),
                ));
            };
            (source_task_id, artifact.artifact_id.clone())
        }
    };
    let draft = derive_skill_proposal_draft(
        &session.trace(),
        &session.capture_policy,
        &format!("Demonstration {}", session.session_id),
    );
    if draft.redaction.status == SkillRedactionStatus::Failed {
        return Err(operator_error_with_details(
            StatusCode::CONFLICT,
            "REDACTION_FAILED",
            "The captured procedure still carries forbidden content and was not stored",
            Some(json!({"reason": "REDACTION_FAILED"})),
        ));
    }
    let draft_bytes = draft.body.clone().into_bytes();
    let draft_digest = format!("sha256:{}", hex::encode(Sha256::digest(&draft_bytes)));
    let resource_id = idempotent_id(
        "res",
        &state.principal_id,
        &request_id,
        &format!("demonstration.draft.{demonstration_id}"),
    );
    let revision_id = format!("{resource_id}-rev-1");
    let now = store.recorded_at();
    let created_resource = state
        .store
        .create_resource(
            WorkspaceCreateRequest {
                principal_id: state.principal_id.clone(),
                request_id: format!("demonstration-draft-{demonstration_id}"),
                request_payload: json!({
                    "operation": "demonstration.draft.create.v1",
                    "workspace_id": workspace.workspace_id,
                    "demonstration_session_id": demonstration_id,
                    "content_digest": draft_digest,
                    "size_bytes": draft_bytes.len(),
                }),
            },
            storage_core::ResourceRecord {
                resource_id: resource_id.clone(),
                workspace_id: workspace.workspace_id.clone(),
                kind: "OTHER".to_owned(),
                provider_identity: json!({"kind": "LITECOWORK_DEMONSTRATION_DRAFT"}),
                identity_digest: None,
                display_name: format!("Demonstration draft {demonstration_id}"),
                current_revision_id: Some(revision_id.clone()),
                sensitivity: "INTERNAL".to_owned(),
                provenance: json!({
                    "source_inputs": [],
                    "transformations": ["DEMONSTRATION_CAPTURE_REDACTION"],
                    "tool_reports": [],
                    "redaction": {
                        "status": "PASSED",
                        "digest": draft_digest,
                    },
                }),
                created_at: now.clone(),
                updated_at: now.clone(),
                version: 1,
            },
            storage_core::ResourceRevisionRecord {
                resource_revision_id: revision_id,
                resource_id: resource_id.clone(),
                parent_revision_ids: Vec::new(),
                provider_revision: None,
                content_digest: Some(draft_digest.clone()),
                size_bytes: Some(draft_bytes.len() as u64),
                media_type: Some("text/markdown".to_owned()),
                observed_at: now.clone(),
                created_by: json!({"kind": "USER", "principal_id": state.principal_id}),
            },
            EventDraft {
                event_id: format!("event-demonstration-draft-{demonstration_id}"),
                workspace_id: workspace.workspace_id.clone(),
                entity_type: "Resource".to_owned(),
                entity_id: resource_id.clone(),
                origin_runtime_id: state.runtime_id.clone(),
                entity_revision: 1,
                hlc_timestamp: now.clone(),
                correlation_id: correlation.clone(),
                causation_id: None,
                schema_version: 1,
                event_type: "resource.created.v1".to_owned(),
                payload: json!({
                    "resource_id": resource_id,
                    "workspace_id": workspace.workspace_id,
                    "kind": "OTHER",
                    "aggregate_version": 1,
                }),
                recorded_at: now.clone(),
            },
            draft_bytes,
        )
        .map_err(|_| unavailable())?;

    let proposal_id = idempotent_id(
        "sp",
        &state.principal_id,
        &request_id,
        &format!("demonstration.proposal.{demonstration_id}"),
    );
    let proposal_correlation = new_id("cor").map_err(|_| unavailable())?;
    let proposal_store = adapter(&state, &proposal_correlation)?;
    let proposal_now = proposal_store.recorded_at();
    let proposal = SkillProposalRecord {
        proposal_id: proposal_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        source: SkillProposalSource::Demonstration,
        source_task_id,
        source_artifact_id,
        draft_resource_ref: created_resource.resource.resource_id.clone(),
        draft_digest: draft_digest.clone(),
        redaction: SkillRedactionSummary {
            status: SkillRedactionStatus::Passed,
            redacted_spans: draft.redaction.redacted_spans,
            finding_codes: draft.redaction.finding_codes.clone(),
            redacted_digest: draft_digest.clone(),
        },
        placeholders: draft.placeholders.clone(),
        declared_capabilities: draft.declared_capabilities.clone(),
        status: SkillProposalStatus::Review,
        proposed_by: format!("service:demonstration-capture:{}", state.runtime_id),
        approved_by: None,
        published_capability_ref: None,
        drift: None,
        created_at: proposal_now.clone(),
        updated_at: proposal_now,
        version: 1,
    };
    let committed_proposal = proposal_store
        .create_skill_proposal(SkillProposalCreateRequest {
            principal_id: state.principal_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            request_id: format!("demonstration-proposal-{demonstration_id}"),
            request_payload: json!({
                "operation": "demonstration.proposal.create.v1",
                "workspace_id": workspace.workspace_id,
                "demonstration_session_id": demonstration_id,
                "draft_resource_id": created_resource.resource.resource_id,
            }),
            proposal,
        })
        .map_err(storage_error)?;

    let complete_correlation = new_id("cor").map_err(|_| unavailable())?;
    let complete_store = adapter(&state, &complete_correlation)?;
    let completed = complete_store
        .complete_demonstration_session(
            &state.principal_id,
            &workspace.workspace_id,
            &demonstration_id,
            session.version,
            Some(created_resource.resource.resource_id.clone()),
            Some(committed_proposal.proposal.proposal_id.clone()),
        )
        .map_err(storage_error)?;
    let _ = completed;
    Ok(no_store(
        Json(demonstration_view(
            &complete_store
                .get_demonstration_session(
                    &state.principal_id,
                    &workspace.workspace_id,
                    &demonstration_id,
                )
                .map_err(storage_error)?
                .ok_or_else(not_found)?,
        ))
        .into_response(),
    ))
}

/// The capture-provider seam: a qualified Environment capture appends one
/// observed semantic action to a running demonstration.
///
/// This is intentionally not an HTTP route. The observation must come from the
/// qualified capture provider inside this Runtime, never from a client that
/// could forge a demonstration.
#[allow(dead_code)]
pub(super) fn append_captured_action(
    state: &ApiState,
    workspace_id: &str,
    session_id: &str,
    environment_id: &str,
    action: storage_core::DemonstrationAction,
) -> Result<DemonstrationAppendOutcome, StoreError> {
    let correlation = new_id("cor").map_err(|_| StoreError::Invalid("correlation".to_owned()))?;
    let store = adapter(state, &correlation).map_err(|_| StoreError::ExecutorStopped)?;
    let now = store.recorded_at();
    store.append_demonstration_action(DemonstrationAppendRequest {
        workspace_id: workspace_id.to_owned(),
        session_id: session_id.to_owned(),
        environment_id: environment_id.to_owned(),
        action,
        now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_frame_origin_is_host_assigned_and_unique_per_frame() {
        let first = app_frame_origin("frame-1");
        let second = app_frame_origin("frame-2");
        assert_eq!(first, "https://app.litecowork.local/mcp-app/frame-1");
        assert_ne!(first, second);
        assert!(storage_core::is_valid_uri(&first));
    }

    #[test]
    fn the_app_frame_origin_never_contains_a_workspace_or_a_secret() {
        let origin = app_frame_origin("frame-1");
        assert!(!origin.contains("workspace"));
        assert!(!origin.to_lowercase().contains("token"));
        assert!(!origin.to_lowercase().contains("secret"));
    }

    #[test]
    fn a_skill_view_never_inlines_the_instruction_content() {
        let revision = SkillRevision {
            revision: 1,
            title: "Invoice cleanup".to_owned(),
            summary: "Clean up an invoice".to_owned(),
            manifest_digest: format!("sha256:{}", "a".repeat(64)),
            content_digest: format!("sha256:{}", "b".repeat(64)),
            content_ref: "res-1".to_owned(),
            declared_capabilities: Vec::new(),
            declared_requirements: Vec::new(),
            origin: SkillOrigin::LocalProposal {
                proposal_id: "proposal-1".to_owned(),
            },
            created_at: "2026-10-20T00:00:00Z".to_owned(),
            created_by: "owner-1".to_owned(),
        };
        let skill = SkillRecord {
            skill_id: "skill-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            kind: SkillKind::LocalProcedure,
            status: SkillStatus::Draft,
            capability_ref: SkillCapabilityRef {
                capability_id: "local:skill-1".to_owned(),
                identity_kind: "LOCAL".to_owned(),
                source: "litecowork:local".to_owned(),
                package_version: None,
                digest: format!("sha256:{}", "c".repeat(64)),
                component: "local:skill-1".to_owned(),
            },
            current_revision: 1,
            revisions: vec![revision],
            created_at: "2026-10-20T00:00:00Z".to_owned(),
            updated_at: "2026-10-20T00:00:00Z".to_owned(),
            version: 1,
        };
        let view = skill_view(&skill);
        let encoded = serde_json::to_string(&view).expect("the view encodes");
        assert!(!encoded.contains("content\":"));
        assert!(encoded.contains("content_ref"));
        assert_eq!(view.status, SkillStatus::Draft);
    }

    #[test]
    fn a_skill_proposal_view_reports_the_publication_blocker_truthfully() {
        let proposal = SkillProposalRecord {
            proposal_id: "proposal-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            source: SkillProposalSource::Demonstration,
            source_task_id: "task-1".to_owned(),
            source_artifact_id: "artifact-1".to_owned(),
            draft_resource_ref: "res-1".to_owned(),
            draft_digest: format!("sha256:{}", "d".repeat(64)),
            redaction: SkillRedactionSummary {
                status: SkillRedactionStatus::Passed,
                redacted_spans: 1,
                finding_codes: vec!["SECRET_REDACTED".to_owned()],
                redacted_digest: format!("sha256:{}", "d".repeat(64)),
            },
            placeholders: Vec::new(),
            declared_capabilities: Vec::new(),
            status: SkillProposalStatus::Review,
            proposed_by: "owner-1".to_owned(),
            approved_by: None,
            published_capability_ref: None,
            drift: None,
            created_at: "2026-10-20T00:00:00Z".to_owned(),
            updated_at: "2026-10-20T00:00:00Z".to_owned(),
            version: 1,
        };
        let view = proposal_view(&proposal);
        assert_eq!(view.status, SkillProposalStatus::Review);
        assert_eq!(view.redaction_status, SkillRedactionStatus::Passed);
        assert!(
            view.published_capability_ref.is_none(),
            "an unpublished proposal never names a published capability"
        );
        assert_eq!(view.approved_by, None);
    }
}
