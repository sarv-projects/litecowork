use axum::{
    Json, Router,
    body::{Body, Bytes, to_bytes},
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64, URL_SAFE_NO_PAD},
};
use crate::agents::{
    AgentInstallation, PlanningDispatchBlocker, discover_local_installations,
    prepare_local_task_planning,
};
use crate::local_filesystem::open_selected_directory;
use domain_task::{CreateStandaloneTask, PreparePlanningAssignment, ReviseTaskSpec, TaskService};
use domain_workspace::{
    AddWorkspaceRoot, ChangeReplicationPolicy, ChangeWorkspaceRootStatus,
    CreateResourceUploadSession, CreateResourceRevisionUploadSession, CreateWorkspace,
    CreateWorkspaceInstructionRevision, EventContext, ResourceUploadService,
    SetWorkspaceDefaultAgentBinding, WorkspaceRootService, WorkspaceService,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    ffi::OsString,
    path::{Path as FsPath, PathBuf},
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
};
use operator_ipc::{
    DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, InFlightBodyBudget, LogicalHeader, PROTOCOL_VERSION,
    ResponseHeader,
};
#[cfg(unix)]
use operator_ipc::unix::{PeerCredentials, UnixEndpoint};
use storage_core::{
    AgentBindingCreateRequest, AgentBindingEnableRequest, AgentBindingRecord, AgentCatalogStore,
    AgentEndpointRecord, AgentProfileRecord, AgentProfileViewRecord, LocalAgentEndpointBindingInput,
    LocalRuntimeWorkspaceBindingLookup, LocalRuntimeWorkspaceEnrollmentRequest,
    RuntimeOfferRecord, RuntimeWorkspaceBindingRecord, RuntimeWorkspaceBindingStore,
    StoreError, WorkspaceCreateRequest, WorkspaceRootStatusAction,
    FolderImportMetadata, ReplicationPolicy, ResourceRecord, ResourceRevisionRecord, ResourceSearchRecord, ResourceStore, ResourceSummary,
    ResourceUploadChunkInput, ResourceUploadContentRange, ResourceUploadSessionRecord,
    ResourceUploadState, ResourceUploadStore, StateStore, TaskStore, TaskSummaryRecord, TaskView, Workspace,
    WorkspaceInstructionRevisionRecord, WorkspaceRootRecord, ResourceTextIndexRebuildRequest,
};
use storage_sqlite::{CoworkerEventContext, RuntimeOsPrincipalIdentity, SqliteCoworkerStore, SqliteWorkspaceStore};
use tokio::sync::oneshot as tokio_oneshot;
#[cfg(unix)]
use tokio::{task::JoinSet, time::timeout};
use tower::ServiceExt;

#[path = "artifact_operator.rs"]
mod artifact_operator;
#[path = "automation_operator.rs"]
mod automation_operator;
#[path = "coworker_operator.rs"]
mod coworker_operator;
#[path = "delegation_profiles_operator.rs"]
mod delegation_profiles_operator;
#[path = "goal_operator.rs"]
mod goal_operator;
#[path = "suggestions_operator.rs"]
mod suggestions_operator;
#[path = "presentation_operator.rs"]
mod presentation_operator;
#[path = "routine_operator.rs"]
mod routine_operator;
#[path = "zip_intake_operator.rs"]
mod zip_intake_operator;

#[derive(Clone)]
struct ApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
    runtime_id: String,
    local_incarnation_id: String,
    runtime_identity: Arc<RuntimeOsPrincipalIdentity>,
}

pub struct OperatorServer {
    shutdown: Option<tokio_oneshot::Sender<()>>,
    admission_stopped: Option<mpsc::Receiver<()>>,
    thread: Option<JoinHandle<()>>,
}

/// Marker added only after the local IPC adapter has authenticated the OS peer.
/// It is deliberately private so a transport caller cannot manufacture an
/// authenticated request by setting an HTTP header.
struct AuthenticatedLocalPeer;

const MAX_OPERATOR_RESPONSE_BYTES: usize = 10 * 1024 * 1024;
const OPERATOR_HANDLER_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceListResponse {
    items: Vec<Workspace>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeWorkspaceRootSelectionRequest {
    workspace_id: String,
    expected_workspace_version: u64,
    selected_path_base64: String,
    watch_policy: String,
    replication_policy: String,
}

#[derive(Serialize)]
struct WorkspaceRootCreatedResponse {
    workspace_id: String,
    workspace_root_id: String,
    resource_id: String,
    display_name: String,
    watch_policy: String,
    replication_policy: String,
    status: String,
    location_availability: String,
    version: u64,
}

#[derive(Serialize)]
struct WorkspaceRootStatusResponse {
    #[serde(flatten)]
    root: WorkspaceRootRecord,
    location_availability: String,
}

#[derive(Serialize)]
struct AgentInstallationListResponse {
    items: Vec<AgentInstallation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeLocalAgentProfileBody {
    provider_key: String,
}

#[derive(Serialize)]
struct AgentProfilePageResponse {
    items: Vec<AgentProfileResponse>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct AgentProfileResponse {
    agent_profile_id: String,
    provider_key: String,
    display_name: String,
    endpoints: Vec<AgentEndpointResponse>,
    discovered_at: String,
    observations: Vec<AgentProfileObservationResponse>,
}

#[derive(Serialize)]
struct AgentEndpointResponse {
    endpoint_id: String,
    agent_profile_id: String,
    protocol: String,
    topology: String,
    protocol_version: Option<String>,
    capabilities: serde_json::Value,
}

#[derive(Serialize)]
struct AgentProfileObservationResponse {
    endpoint_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
    compatible: bool,
    readiness: String,
    observed_at: String,
    offer_expires_at: String,
    constraints: serde_json::Value,
}

#[derive(Serialize)]
struct AgentBindingPageResponse {
    items: Vec<AgentBindingRecord>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct RuntimeWorkspaceBindingPageResponse {
    items: Vec<RuntimeWorkspaceBindingRecord>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateAgentBindingBody {
    workspace_id: String,
    agent_profile_id: String,
    lead_eligible: bool,
    runtime_id: Option<String>,
    endpoint_selection_policy: Option<serde_json::Value>,
    auth_ref: Option<serde_json::Value>,
    configuration: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskListQuery {
    status: Option<String>,
    conversation_id: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversationTaskListQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct TaskPageResponse {
    items: Vec<TaskSummaryRecord>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct TaskPlanningReadinessResponse {
    task_id: String,
    task_version: u64,
    task_spec_revision: u64,
    task_status: String,
    observed_at: String,
    dispatch_available: bool,
    planning_started: bool,
    agent_session_started: bool,
    plan_created: bool,
    blockers: Vec<PlanningDispatchBlocker>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviseTaskSpecBody {
    parent_revisions: Vec<u64>,
    objective: String,
    #[serde(default)]
    constraints: Option<Vec<String>>,
    #[serde(default)]
    non_goals: Option<Vec<String>>,
    #[serde(default)]
    input_refs: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    required_outputs: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    acceptance_criteria: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    approvals_required: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    placement_preference: Option<serde_json::Value>,
    #[serde(default)]
    preferred_lead_agent_binding_id: Option<String>,
    #[serde(default)]
    lead_failover_policy: Option<serde_json::Value>,
}


#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TaskListCursor {
    version: u8,
    workspace_id: String,
    status: Option<String>,
    conversation_id: Option<String>,
    created_at: String,
    task_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateWorkspaceBody {
    name: String,
    replication_policy: Option<ReplicationPolicy>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateWorkspacePolicyBody {
    replication_policy: ReplicationPolicy,
    #[serde(default)]
    replication_scope_root_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetWorkspaceDefaultAgentBindingBody {
    #[serde(deserialize_with = "Option::<String>::deserialize")]
    agent_binding_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateResourceBody {
    display_name: String,
    media_type: String,
    content_base64: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateWorkspaceInstructionRevisionBody {
    parent_revisions: Vec<u64>,
    content_ref: serde_json::Value,
    content_digest: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceCreatedResponse {
    resource_id: String,
    workspace_id: String,
    resource_revision_id: String,
    display_name: String,
    media_type: String,
    content_digest: String,
    size_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceListResponse {
    items: Vec<ResourceSummary>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRootListResponse {
    items: Vec<storage_core::WorkspaceRootListRecord>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceListQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceRootListQuery {
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceSearchQuery {
    q: Option<String>,
    mode: Option<String>,
    kind: Option<String>,
    freshness: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceContentQuery {
    revision_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RebuildResourceTextIndexBody {
    resource_revision_id: String,
    content_digest: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResourceSearchCursor {
    workspace_id: String,
    query: Option<String>,
    mode: String,
    kind: Option<String>,
    freshness: Option<String>,
    created_at: String,
    resource_id: String,
}

#[derive(Serialize)]
struct ResourceSearchPageResponse {
    items: Vec<ResourceSearchResultResponse>,
    next_cursor: Option<String>,
    mode: String,
    content_scan: Option<ResourceContentScanInfo>,
}

#[derive(Serialize)]
struct ResourceContentScanInfo {
    candidates_scanned: usize,
    text_resources_checked: usize,
    skipped_unsupported_type: usize,
    skipped_over_file_limit: usize,
    skipped_revision_changed: usize,
    byte_budget_exhausted: bool,
    candidate_budget_exhausted: bool,
    max_candidates: usize,
    max_file_bytes: u64,
    max_total_bytes: u64,
}

#[derive(Serialize)]
struct ResourceSearchResultResponse {
    resource_ref: ResourceSearchRefResponse,
    display_name: String,
    locations: Vec<ResourceSearchLocationResponse>,
    freshness: String,
    match_reasons: Vec<String>,
    snippet: Option<String>,
}

#[derive(Serialize)]
struct ResourceSearchRefResponse {
    workspace_id: String,
    resource_id: String,
    revision_id: Option<String>,
}

#[derive(Serialize)]
struct ResourceSearchLocationResponse {
    location_id: String,
    resource_id: String,
    runtime_id: Option<String>,
    environment_id: Option<String>,
    connection_id: Option<String>,
    availability: String,
    writable: bool,
    observed_revision_id: Option<String>,
    observed_digest: Option<String>,
    observed_at: String,
    last_checked_at: Option<String>,
    freshness: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateResourceUploadBody {
    workspace_id: String,
    display_name: String,
    media_type: String,
    size_bytes: u64,
    expected_digest: String,
    context_document: Option<serde_json::Value>,
    folder_import: Option<FolderImportMetadata>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateResourceRevisionUploadBody {
    media_type: String,
    size_bytes: u64,
    expected_digest: String,
    parent_revision_ids: Vec<String>,
}

#[derive(Serialize)]
struct CommittedResourceResponse {
    resource: ResourceRecord,
    revision: ResourceRevisionRecord,
    event: storage_core::DomainEvent,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResourceListCursor {
    workspace_id: String,
    created_at: String,
    resource_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceRevisionListQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResourceRevisionListCursor {
    workspace_id: String,
    resource_id: String,
    resource_revision_id: String,
}

#[derive(Serialize)]
struct ResourceRevisionPageResponse {
    items: Vec<storage_core::ResourceRevisionViewRecord>,
    next_cursor: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceRootListCursor {
    workspace_id: String,
    status: Option<String>,
    created_at: String,
    workspace_root_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstructionHistoryQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InstructionHistoryCursor {
    workspace_id: String,
    after_revision: u64,
}

#[derive(Serialize)]
struct InstructionHistoryPage {
    items: Vec<WorkspaceInstructionRevisionRecord>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct OperatorError {
    code: &'static str,
    message: &'static str,
    retryable: bool,
    correlation_id: String,
    details: Option<Value>,
}

impl OperatorServer {
    pub fn start(
        data_directory: &FsPath,
        store: SqliteWorkspaceStore,
        principal_id: String,
        runtime_id: String,
        runtime_incarnation_id: String,
        expected_peer_uid: u32,
        runtime_identity: storage_sqlite::RuntimeOsPrincipalIdentity,
    ) -> Result<Self, String> {
        #[cfg(unix)]
        {
        operator_ipc::require_os_local_ipc()
            .map_err(|_| "authenticated local Operator IPC is unsupported on this platform".to_owned())?;
        let endpoint = UnixEndpoint::derive(data_directory)
            .map_err(|_| "local Operator IPC endpoint is unsafe".to_owned())?;
        let state = ApiState {
            store,
            principal_id,
            runtime_id,
            local_incarnation_id: runtime_incarnation_id,
            runtime_identity: Arc::new(runtime_identity),
        };
        let sweeper_state = state.clone();
        let app = build_operator_router(state);
        let (shutdown, shutdown_receiver) = tokio_oneshot::channel();
        let (admission_stopped_sender, admission_stopped) = mpsc::sync_channel(1);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("litecowork-operator-ipc".to_owned())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        let _ = ready_sender.send(false);
                        return;
                    }
                };
                runtime.block_on(async move {
                    // Tokio's UnixListener must be created from_std inside the
                    // owning Runtime; no listener is created outside this thread.
                    let listener = match endpoint.bind() {
                        Ok(listener) => listener,
                        Err(_) => {
                            let _ = ready_sender.send(false);
                            return;
                        }
                    };
                    let budget = match InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES) {
                        Ok(budget) => budget,
                        Err(_) => {
                            let _ = ready_sender.send(false);
                            return;
                        }
                    };
                    if ready_sender.send(true).is_err() {
                        return;
                    }
                    let mut workers = JoinSet::new();
                    let mut expiry_tick = tokio::time::interval(std::time::Duration::from_secs(30));
                    expiry_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    loop {
                        tokio::select! {
                            _ = &mut shutdown_receiver => {
                                break;
                            }
                            worker = workers.join_next(), if !workers.is_empty() => {
                                let _ = worker;
                            }
                            accepted = listener.accept(|peer: PeerCredentials| peer.uid == expected_peer_uid) => {
                                match accepted {
                                    Ok(connection) => {
                                        let router = app.clone();
                                        let budget = budget.clone();
                                        workers.spawn(async move {
                                            let _ = dispatch_ipc_exchange(router, connection, budget).await;
                                        });
                                    }
                                    Err(operator_ipc::unix::TransportError::PeerRejected)
                                    | Err(operator_ipc::unix::TransportError::CapacityExceeded) => {}
                                    Err(_) => break,
                                }
                            }
                            _ = expiry_tick.tick() => {
                                expire_due_upload_sessions(&sweeper_state).await;
                            }
                        }
                    }
                    drop(listener);
                    let _ = admission_stopped_sender.send(());
                    if timeout(std::time::Duration::from_secs(60), async {
                        while workers.join_next().await.is_some() {}
                    }).await.is_err() {
                        workers.abort_all();
                        while workers.join_next().await.is_some() {}
                    }
                });
            })
            .map_err(|_| "could not start the local Operator endpoint".to_owned())?;
        match ready_receiver.recv() {
            Ok(true) => {
                Ok(Self {
                    shutdown: Some(shutdown),
                    admission_stopped: Some(admission_stopped),
                    thread: Some(thread),
                })
            }
            _ => {
                let _ = thread.join();
                Err("local Operator endpoint failed to initialize".to_owned())
            }
        }
        }
        #[cfg(not(unix))]
        {
            let _ = (
                data_directory,
                store,
                principal_id,
                runtime_id,
                runtime_incarnation_id,
                expected_peer_uid,
                runtime_identity,
            );
            Err("authenticated local Operator IPC is unsupported on this platform".to_owned())
        }
    }

    pub fn stop(mut self) {
        self.shutdown_server();
    }

    pub fn stop_admission(&mut self) -> Result<(), String> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(stopped) = self.admission_stopped.take() {
            stopped
                .recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|_| "local Operator did not stop admission in time".to_owned())?;
        }
        Ok(())
    }

    fn shutdown_server(&mut self) {
        let _ = self.stop_admission();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn build_operator_router(state: ApiState) -> Router {
    Router::new()
        .merge(artifact_operator::routes())
        .merge(automation_operator::router())
        .merge(coworker_operator::routes())
        .merge(delegation_profiles_operator::routes())
        .merge(goal_operator::routes())
        .merge(suggestions_operator::routes())
        .merge(presentation_operator::routes())
        .merge(routine_operator::routes())
        .merge(zip_intake_operator::routes())
        // This route is accepted only over authenticated local IPC. It is not part of
        // OpenAPI and no HTTP listener is mounted by the desktop Runtime.
        .route(
            operator_ipc::PRIVATE_LOCAL_ROOT_SELECTION_PATH,
            post(create_workspace_root_from_native_selection)
                .layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route("/v1/operator/readiness", get(operator_readiness))
        .route("/v1/agent-installations", get(list_agent_installations))
        .route("/v1/agent-profiles", get(list_agent_profiles))
        .route("/v1/agent-profiles/probe", post(probe_local_agent_profile))
        .route("/v1/agent-bindings", get(list_agent_bindings).post(create_agent_binding))
        .route("/v1/agent-bindings/{agent_binding_id}", get(get_agent_binding))
        .route("/v1/agent-bindings/{agent_binding_id}/enable", post(enable_agent_binding))
        .route("/v1/tasks", get(list_tasks).post(create_task))
        .route("/v1/tasks/{task_id}", get(get_task))
        .route("/v1/tasks/{task_id}/planning-readiness", get(get_task_planning_readiness))
        .route("/v1/tasks/{task_id}/spec-revisions", get(list_task_spec_revisions).post(revise_task_spec))
        .route("/v1/tasks/{task_id}/plan-revisions", get(list_task_plan_revisions))
        .route("/v1/tasks/{task_id}/steps", get(list_task_steps))
        .route("/v1/conversations/{conversation_id}/tasks", get(list_conversation_tasks))
        .route("/v1/resources/uploads", post(create_resource_upload))
        .route("/v1/resources/uploads/{upload_id}", get(get_resource_upload))
        .route("/v1/resources/uploads/{upload_id}/chunks/{chunk_index}", axum::routing::put(put_resource_upload_chunk))
        .route("/v1/resources/uploads/{upload_id}/commit", post(commit_resource_upload))
        .route("/v1/resources/quick-import", post(create_resource))
        .route("/v1/resources/{resource_id}/revisions", get(list_resource_revisions))
        .route("/v1/resources/{resource_id}/text-index/rebuild", post(rebuild_resource_text_index))
        .route("/v1/resources/{resource_id}/revision-uploads", post(create_resource_revision_upload))
        .route("/v1/resources/{resource_id}", get(get_resource_detail))
        .route("/v1/resources/search", get(search_resources))
        .route("/v1/resources", get(list_resources))
        .route("/v1/workspace-roots", get(list_workspace_roots))
        .route("/v1/workspace-roots/{workspace_root_id}/pause", post(pause_workspace_root))
        .route("/v1/workspace-roots/{workspace_root_id}/resume", post(resume_workspace_root))
        .route("/v1/workspace-roots/{workspace_root_id}/revoke", post(revoke_workspace_root))
        .route(
            "/v1/resources/{resource_id}/content",
            get(read_resource_content),
        )
        .route(
            "/v1/workspaces",
            post(create_workspace).get(list_workspaces),
        )
        .route(
            "/v1/workspaces/{workspace_id}",
            get(get_workspace).patch(update_workspace_policy),
        )
        .route(
            "/v1/workspaces/{workspace_id}/default-agent-binding",
            axum::routing::patch(set_workspace_default_agent_binding),
        )
        .route("/v1/workspaces/{workspace_id}/runtime-bindings/current-local", get(list_workspace_runtime_bindings))
        .route("/v1/workspaces/{workspace_id}/runtime-bindings/local-enrollment", post(enroll_local_runtime))
        .route(
            "/v1/workspaces/{workspace_id}/instructions/revisions",
            get(list_workspace_instruction_revisions)
                .post(create_workspace_instruction_revision),
        )
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .route_layer(middleware::from_fn(authenticate))
        .with_state(state)
}

#[cfg(unix)]
async fn dispatch_ipc_exchange(
    router: Router,
    mut connection: operator_ipc::unix::AuthenticatedUnixConnection,
    budget: std::sync::Arc<InFlightBodyBudget>,
) -> Result<(), String> {
    // Reserve bounded response capacity before admitting/dispatching the request. This
    // prevents concurrent handlers from buffering unaccounted response bodies and keeps
    // the 128 MiB shared frame budget meaningful under same-UID load.
    let response_permit = budget
        .reserve_bytes(MAX_OPERATOR_RESPONSE_BYTES)
        .map_err(|_| "local Operator IPC is at capacity".to_owned())?;
    let frame = connection
        .read_request(&budget)
        .await
        .map_err(|_| "local Operator IPC request could not be read".to_owned())?;
    let (frame_header, budgeted_body) = frame.into_parts();
    let method = axum::http::Method::from_bytes(frame_header.method.as_bytes())
        .map_err(|_| "local Operator IPC method is invalid".to_owned())?;
    let uri = frame_header
        .path_and_query
        .parse::<axum::http::Uri>()
        .map_err(|_| "local Operator IPC path is invalid".to_owned())?;
    let mut request = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::new(budgeted_body))
        .map_err(|_| "local Operator IPC request could not be constructed".to_owned())?;
    for logical_header in frame_header.headers {
        let name = axum::http::header::HeaderName::from_bytes(logical_header.name.as_bytes())
            .map_err(|_| "local Operator IPC header is invalid".to_owned())?;
        let value = axum::http::header::HeaderValue::from_str(&logical_header.value)
            .map_err(|_| "local Operator IPC header is invalid".to_owned())?;
        request.headers_mut().insert(name, value);
    }
    request.extensions_mut().insert(AuthenticatedLocalPeer);

    let response = match timeout(OPERATOR_HANDLER_DEADLINE, router.oneshot(request)).await {
        Ok(Ok(response)) => response,
        Ok(Err(never)) => match never {},
        Err(_) => operator_error(
            StatusCode::GATEWAY_TIMEOUT,
            "TIMEOUT",
            "The local Operator request exceeded its time limit; check current state before retrying",
        ),
    };
    let status = response.status().as_u16();
    let mut response_headers = Vec::new();
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CACHE_CONTROL,
        header::ETAG,
    ] {
        if let Some(value) = response.headers().get(&name)
            && let Ok(value) = value.to_str()
        {
            response_headers.push(LogicalHeader {
                name: name.as_str().to_owned(),
                value: value.to_owned(),
            });
        }
    }
    for name in ["x-content-type-options", "x-resource-media-type", "x-correlation-id"] {
        if let Some(value) = response.headers().get(name)
            && let Ok(value) = value.to_str()
        {
            response_headers.push(LogicalHeader {
                name: name.to_owned(),
                value: value.to_owned(),
            });
        }
    }
    let response_body = to_bytes(response.into_body(), MAX_OPERATOR_RESPONSE_BYTES)
        .await
        .map_err(|_| "local Operator IPC response exceeded its limit".to_owned())?;
    let response_frame = operator_ipc::ResponseFrame::new_bytes_with_reservation(
        ResponseHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: frame_header.request_id,
            status,
            headers: response_headers,
            body_length: response_body.len() as u64,
        },
        response_body,
        response_permit,
    )
    .map_err(|_| "local Operator IPC response could not be framed".to_owned())?;
    connection
        .write_response(&response_frame)
        .await
        .map_err(|_| "local Operator IPC response could not be sent".to_owned())
}

#[derive(Serialize)]
struct OperatorReadinessResponse {
    operator_state: &'static str,
    runtime_id: String,
    local_incarnation_id: String,
    api_contract_version: u32,
}

async fn operator_readiness(State(state): State<ApiState>) -> Json<OperatorReadinessResponse> {
    // This local bootstrap identity is returned only through the authenticated local
    // Operator handshake. It is not a registered Mesh Runtime/RuntimeIncarnation.
    Json(OperatorReadinessResponse {
        operator_state: "SERVING",
        runtime_id: state.runtime_id,
        local_incarnation_id: state.local_incarnation_id,
        api_contract_version: 1,
    })
}

async fn create_resource(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateResourceBody>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let workspace = state.store.get_workspace(&workspace_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace is unavailable",
        )
    })?;
    if !workspace.is_some_and(|workspace| {
        workspace.owner_principal_id == state.principal_id && workspace.status == "ACTIVE"
    }) {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    let name = body.display_name.trim();
    let media_type = body.media_type.trim();
    if name.is_empty()
        || name.chars().count() > 240
        || media_type.is_empty()
        || media_type.len() > 160
    {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource metadata is invalid",
        ));
    }
    let content = BASE64.decode(body.content_base64.as_bytes()).map_err(|_| {
        operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource content encoding is invalid",
        )
    })?;
    const MAX_RESOURCE_BYTES: usize = 10 * 1024 * 1024;
    if content.len() > MAX_RESOURCE_BYTES {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Resource must not exceed 10 MiB",
        ));
    }
    let request_id = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| {
            !v.is_empty() && v.len() <= 128 && v.bytes().all(|byte| byte.is_ascii_graphic())
        })
        .ok_or_else(|| {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "A valid Idempotency-Key is required",
            )
        })?;
    let correlation_id = new_id("cor").map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Resource request could not be initialized",
        )
    })?;
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Resource request could not be initialized",
        )
    })?;
    let resource_id = new_id("res").map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Resource request could not be initialized",
        )
    })?;
    let revision_id = new_id("rrev").map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Resource request could not be initialized",
        )
    })?;
    let digest = sha256_digest(&content);
    let resource = ResourceRecord {
        resource_id: resource_id.clone(),
        workspace_id: workspace_id.clone(),
        kind: "FILE".to_owned(),
        provider_identity: json!({"provider_instance_id":"litecowork.local-upload", "stable_object_id":resource_id, "identity_confidence":"WEAK"}),
        identity_digest: None,
        display_name: name.to_owned(),
        current_revision_id: Some(revision_id.clone()),
        sensitivity: "PERSONAL".to_owned(),
        provenance: json!({"source_inputs":[], "transformations":[], "tool_reports":[]}),
        created_at: context.recorded_at.clone(),
        updated_at: context.recorded_at.clone(),
        version: 1,
    };
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id.clone(),
        resource_id: resource_id.clone(),
        parent_revision_ids: Vec::new(),
        provider_revision: None,
        content_digest: Some(digest.clone()),
        size_bytes: Some(content.len() as u64),
        media_type: Some(media_type.to_owned()),
        observed_at: context.recorded_at.clone(),
        created_by: json!({"principal_id":state.principal_id.clone(), "kind":"USER"}),
    };
    let event = storage_core::EventDraft {
        event_id: context.event_id,
        workspace_id: workspace_id.clone(),
        entity_type: "Resource".to_owned(),
        entity_id: resource_id.clone(),
        origin_runtime_id: context.origin_runtime_id,
        entity_revision: 1,
        hlc_timestamp: context.hlc_timestamp,
        correlation_id: context.correlation_id.clone(),
        causation_id: None,
        schema_version: 1,
        event_type: "resource.created.v1".to_owned(),
        payload: json!({"resource_id":resource_id, "workspace_id":workspace_id, "kind":"FILE", "provenance":resource.provenance.clone(), "aggregate_version":1}),
        recorded_at: context.recorded_at,
    };
    let store_request = storage_core::WorkspaceCreateRequest {
        principal_id: state.principal_id.clone(),
        request_id: request_id.to_owned(),
        request_payload: json!({"operation":"resource.quick_import.v1", "workspace_id":workspace_id, "display_name":name, "media_type":media_type, "content_digest":digest, "size_bytes":content.len()}),
    };
    let store = state.store.clone();
    let result = tokio::task::spawn_blocking(move || {
        store.create_resource(store_request, resource, revision, event, content)
    })
    .await
    .map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Resource storage did not complete",
        )
    })?;
    let committed = result.map_err(|error| {
        if matches!(error, storage_core::StoreError::Conflict { .. }) {
            return operator_error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Idempotency key was already used for a different Resource",
            );
        }
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Resource could not be stored",
        )
    })?;
    let response = ResourceCreatedResponse {
        resource_id: committed.resource.resource_id,
        workspace_id: committed.resource.workspace_id,
        resource_revision_id: committed.revision.resource_revision_id,
        display_name: committed.resource.display_name,
        media_type: committed
            .revision
            .media_type
            .unwrap_or_else(|| "application/octet-stream".to_owned()),
        content_digest: digest,
        size_bytes: committed.revision.size_bytes.unwrap_or_default(),
    };
    let mut result = (StatusCode::CREATED, Json(response)).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
        result
            .headers_mut()
            .insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(result)
}

async fn create_resource_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateResourceUploadBody>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if body.workspace_id != workspace_id {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Upload Workspace does not match the selected Workspace"));
    }
    let display_name = body.display_name.trim();
    let media_type = body.media_type.trim();
    let request_id = idempotency_key(&headers)?;
    let upload_id = new_id("upl").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload could not be initialized"))?;
    let now = time::OffsetDateTime::now_utc();
    let expires_at = (now + time::Duration::hours(24)).format(&time::format_description::well_known::Rfc3339).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload could not be initialized"))?;
    let correlation_id = new_id("cor")
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload could not be initialized"))?;
    let event_context = event_context(&state.runtime_id, &correlation_id)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload could not be initialized"))?;
    let command = CreateResourceUploadSession {
        upload_id,
        workspace_id,
        principal_id: state.principal_id.clone(),
        request_id: request_id.to_owned(),
        display_name: display_name.to_owned(),
        media_type: media_type.to_owned(),
        size_bytes: body.size_bytes,
        expected_digest: body.expected_digest,
        context_document: body.context_document,
        folder_import: body.folder_import,
        expires_at,
        event: event_context,
    };
    let store = state.store.clone();
    let created = tokio::task::spawn_blocking(move || ResourceUploadService::new(store).create_session(command)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload session creation did not complete"))?
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource upload metadata is invalid or unsupported"),
            storage_core::StoreError::NotFound => operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"),
            storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "CONFLICT", "Upload request conflicts with an existing request"),
            _ => upload_store_error(error),
        })?;
    let mut response = (StatusCode::CREATED, Json(created)).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn create_resource_revision_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(resource_id): Path<String>,
    Json(body): Json<CreateResourceRevisionUploadBody>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let workspace = ensure_workspace_owner(&state, &workspace_id)?;
    if workspace.status != "ACTIVE" {
        return Err(operator_error(StatusCode::CONFLICT, "CONFLICT", "Archived Workspaces are read-only"));
    }
    let expected_resource_version = parse_if_match(&headers)?;
    let request_id = idempotency_key(&headers)?;
    let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Revision upload could not be initialized"))?;
    let event = event_context(&state.runtime_id, &correlation_id)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Revision upload could not be initialized"))?;
    let upload_id = new_id("upl").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Revision upload could not be initialized"))?;
    let now = time::OffsetDateTime::now_utc();
    let expires_at = (now + time::Duration::hours(24)).format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Revision upload could not be initialized"))?;
    let command = CreateResourceRevisionUploadSession {
        upload_id,
        workspace_id: workspace_id.clone(),
        resource_id: resource_id.clone(),
        expected_resource_version,
        parent_revision_ids: body.parent_revision_ids,
        principal_id: state.principal_id.clone(),
        request_id,
        media_type: body.media_type,
        size_bytes: body.size_bytes,
        expected_digest: body.expected_digest,
        expires_at,
        event,
    };
    let store = state.store.clone();
    let created = tokio::task::spawn_blocking(move || ResourceUploadService::new(store).create_revision_session(command)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Revision upload did not complete"))?
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource revision upload metadata is invalid"),
            storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "RESOURCE_CONFLICT", "Resource version or revision heads changed; reload and retry"),
            storage_core::StoreError::NotFound => operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Revision upload could not be created"),
        })?;
    let mut response = (StatusCode::CREATED, Json(created)).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn list_resource_revisions(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(resource_id): Path<String>,
    Query(query): Query<ResourceRevisionListQuery>,
) -> Result<Json<ResourceRevisionPageResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource revision page limit must be between 1 and 200"));
    }
    let after_revision_id = if let Some(encoded) = query.cursor.as_deref() {
        let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource revision cursor is invalid"))?;
        let cursor: ResourceRevisionListCursor = serde_json::from_slice(&bytes).map_err(|_| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource revision cursor is invalid"))?;
        if cursor.workspace_id != workspace_id || cursor.resource_id != resource_id {
            return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource revision cursor does not match this query"));
        }
        Some(cursor.resource_revision_id)
    } else { None };
    let mut items = state.store.list_resource_revisions_page(&workspace_id, &resource_id, after_revision_id.as_deref(), limit + 1).map_err(|error| match error {
        storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Resource is unavailable"),
        storage_core::StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource revision cursor is invalid"),
        _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource revisions are unavailable"),
    })?;
    let has_more = items.len() > limit;
    if has_more { items.pop(); }
    let next_cursor = if has_more {
        items.last().map(|item| serde_json::to_vec(&ResourceRevisionListCursor {
            workspace_id: workspace_id.clone(),
            resource_id: resource_id.clone(),
            resource_revision_id: item.revision.resource_revision_id.clone(),
        }).map(|bytes| URL_SAFE_NO_PAD.encode(bytes)).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource revision cursor could not be created"))).transpose()?
    } else { None };
    Ok(Json(ResourceRevisionPageResponse { items, next_cursor }))
}

async fn get_resource_detail(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(resource_id): Path<String>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let detail = state.store.get_resource_detail(&workspace_id, &resource_id).map_err(|_| {
        operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource metadata is unavailable")
    })?.ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Resource is unavailable"))?;
    let mut response = Json(detail).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn get_resource_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(upload_id): Path<String>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let store = state.store.clone();
    let session = tokio::task::spawn_blocking(move || store.get(&workspace_id, &upload_id)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload progress is unavailable"))?
        .map_err(upload_store_error)?
        .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Upload session is unavailable"))?;
    let mut response = Json(session).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn expire_due_upload_sessions(state: &ApiState) {
    let now = match time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
    {
        Ok(value) => value,
        Err(_) => return,
    };
    let store = state.store.clone();
    let now_for_expiry = now.clone();
    let sessions = match tokio::task::spawn_blocking(move || store.list_expired(&now_for_expiry, 100)).await {
        Ok(Ok(sessions)) => sessions,
        _ => return,
    };
    for session in sessions {
        let _ = expire_upload_if_due(state, session).await;
    }

    // ResourceUploadStore serializes the claim and reference check with chunk receipt
    // transactions. The daemon process lock guarantees a single periodic collector;
    // durable DELETING fences let the next daemon incarnation retry after a crash.
    let store = state.store.clone();
    let collected = tokio::task::spawn_blocking(move || store.collect_orphan_chunks(&now, 100)).await;
    if !matches!(collected, Ok(Ok(_))) {
        // Keep cleanup failure non-fatal. Any claimed object stays fenced and is retried
        // by the next bounded sweep; uploads and other Operator requests continue.
        eprintln!("litecoworkd: encrypted upload-chunk cleanup deferred; it will retry");
    }
}

async fn put_resource_upload_chunk(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((upload_id, chunk_index)): Path<(String, u64)>,
    body: Bytes,
) -> Result<StatusCode, Response> {
    const MAX_CHUNK: usize = 4 * 1024 * 1024;
    if body.is_empty() || body.len() > MAX_CHUNK {
        return Err(operator_error(StatusCode::PAYLOAD_TOO_LARGE, "INVALID_ARGUMENT", "Upload chunks must contain between 1 byte and 4 MiB"));
    }
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    if workspace.status != "ACTIVE" {
        return Err(operator_error(StatusCode::CONFLICT, "CONFLICT", "Archived Workspaces are read-only"));
    }
    let request_id = idempotency_key(&headers)?;
    let content_range = headers.get(header::CONTENT_RANGE).and_then(|value| value.to_str().ok()).and_then(parse_content_range)
        .ok_or_else(|| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Content-Range is invalid"))?;
    let supplied_digest = headers.get("x-chunk-sha256").and_then(|value| value.to_str().ok()).filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()))
        .ok_or_else(|| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Chunk digest is invalid"))?;
    if sha256_digest(&body) != format!("sha256:{supplied_digest}") {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INTEGRITY_FAILURE", "Chunk digest does not match its content"));
    }
    let store = state.store.clone();
    let scoped_workspace = workspace_id.clone();
    let found = tokio::task::spawn_blocking(move || store.get(&scoped_workspace, &upload_id)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload session is unavailable"))?
        .map_err(upload_store_error)?
        .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Upload session is unavailable"))?;
    if found.workspace_id != workspace_id {
        return Err(operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Upload session is unavailable"));
    }
    let received_at = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Chunk could not be accepted"))?;
    let found = expire_upload_if_due(&state, found).await?;
    if found.state == ResourceUploadState::Expired {
        return Err(operator_error(StatusCode::GONE, "UPLOAD_EXPIRED", "Upload session has expired"));
    }
    let chunk = ResourceUploadChunkInput {
        upload_id,
        chunk_index,
        request_id: request_id.to_owned(),
        content_range: ResourceUploadContentRange { start_offset: content_range.0, end_offset_inclusive: content_range.1, total_size_bytes: content_range.2 },
        sha256: format!("sha256:{supplied_digest}"),
        content: body.to_vec(),
        received_at,
        lifecycle_event: {
            let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Chunk transfer could not be initialized"))?;
            let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Chunk transfer could not be initialized"))?;
            storage_core::EventDraft {
                event_id: context.event_id,
                workspace_id: workspace_id.clone(),
                entity_type: "ResourceUpload".to_owned(),
                entity_id: upload_id.clone(),
                origin_runtime_id: context.origin_runtime_id,
                entity_revision: found.version.saturating_add(1),
                hlc_timestamp: context.hlc_timestamp,
                correlation_id: context.correlation_id,
                causation_id: None,
                schema_version: 1,
                event_type: "resource.upload.status.changed.v1".to_owned(),
                payload: json!({}),
                recorded_at: context.recorded_at,
            }
        },
    };
    let store = state.store.clone();
    tokio::task::spawn_blocking(move || store.put_chunk(chunk)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Chunk transfer did not complete"))?
        .map_err(upload_store_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn commit_resource_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(upload_id): Path<String>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    if workspace.status != "ACTIVE" {
        return Err(operator_error(StatusCode::CONFLICT, "CONFLICT", "Archived Workspaces are read-only"));
    }
    let request_id = idempotency_key(&headers)?;
    let store = state.store.clone();
    let scoped_workspace = workspace_id.clone();
    let session = tokio::task::spawn_blocking(move || store.get(&scoped_workspace, &upload_id)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload session is unavailable"))?
        .map_err(upload_store_error)?
        .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Upload session is unavailable"))?;
    let session = expire_upload_if_due(&state, session).await?;
    if session.state == ResourceUploadState::Expired {
        return Err(operator_error(StatusCode::GONE, "UPLOAD_EXPIRED", "Upload session has expired"));
    }
    if session.state == ResourceUploadState::Failed {
        return Err(operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INTEGRITY_FAILURE", "Upload content failed verification; start a new upload"));
    }
    if session.state == ResourceUploadState::Committed {
        let committed = state.store.committed_resource_for_upload(&state.principal_id, &session.upload_id)
            .map_err(upload_store_error)?
            .ok_or_else(|| operator_error(StatusCode::CONFLICT, "RESOURCE_CONFLICT", "The committed upload has no replayable receipt; review the Resource before retrying"))?;
        let correlation_id = committed.event.correlation_id.clone();
        let mut response = (StatusCode::CREATED, Json(CommittedResourceResponse {
            resource: committed.resource,
            revision: committed.revision,
            event: committed.event,
        })).into_response();
        response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
        if let Ok(value) = header::HeaderValue::from_str(&correlation_id) {
            response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
        }
        return Ok(response);
    }
    let expected_digest = session.expected_digest.as_deref().ok_or_else(|| {
        operator_error(StatusCode::CONFLICT, "RESOURCE_CONFLICT", "This older upload has no whole-file digest and cannot be safely resumed or committed; start a new upload")
    })?;
    let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource commit could not be initialized"))?;
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource commit could not be initialized"))?;
    let revision_upload = session.resource_id.is_some();
    let resource_id = match session.resource_id.as_deref() {
        Some(resource_id) => resource_id.to_owned(),
        None => new_id("res").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource commit could not be initialized"))?,
    };
    let revision_id = new_id("rrev").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource commit could not be initialized"))?;
    let resource = if revision_upload {
        let current = state.store.get_resource_record(&workspace_id, &resource_id)
            .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource is unavailable"))?
            .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Resource is unavailable"))?;
        if Some(current.version) != session.expected_resource_version || current.display_name != session.display_name {
            return Err(operator_error(StatusCode::CONFLICT, "RESOURCE_CONFLICT", "Resource changed after this revision upload was created"));
        }
        ResourceRecord {
            current_revision_id: Some(revision_id.clone()),
            updated_at: context.recorded_at.clone(),
            version: current.version.checked_add(1).ok_or_else(|| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource version overflow"))?,
            ..current
        }
    } else {
        let mut provenance = json!({"source_inputs":[], "transformations":[], "tool_reports":[]});
        if let Some(folder_import) = &session.folder_import {
            provenance["folder_import"] = json!({"relative_path": folder_import.relative_path});
        }
        ResourceRecord {
            resource_id: resource_id.clone(), workspace_id: workspace_id.clone(), kind: "FILE".to_owned(),
            provider_identity: json!({"provider_instance_id":"litecowork.local-upload", "stable_object_id":resource_id, "identity_confidence":"WEAK"}),
            identity_digest: None, display_name: session.display_name.clone(), current_revision_id: Some(revision_id.clone()),
            sensitivity: "PERSONAL".to_owned(), provenance,
            created_at: context.recorded_at.clone(), updated_at: context.recorded_at.clone(), version: 1,
        }
    };
    let revision = ResourceRevisionRecord {
        resource_revision_id: revision_id.clone(), resource_id: resource_id.clone(), parent_revision_ids: session.parent_revision_ids.clone(),
        provider_revision: None, content_digest: Some(expected_digest.to_owned()), size_bytes: Some(session.expected_size_bytes),
        media_type: Some(session.media_type.clone()), observed_at: context.recorded_at.clone(),
        created_by: json!({"principal_id":state.principal_id.clone(), "kind":"USER"}),
    };
    let event = storage_core::EventDraft {
        event_id: context.event_id, workspace_id: workspace_id.clone(), entity_type: "Resource".to_owned(), entity_id: resource_id.clone(),
        origin_runtime_id: context.origin_runtime_id, entity_revision: resource.version, hlc_timestamp: context.hlc_timestamp,
        correlation_id: context.correlation_id.clone(), causation_id: None, schema_version: 1,
        event_type: (if revision_upload { "resource.revision.created.v1" }
            else if session.folder_import.is_some() { "resource.created.v2" } else { "resource.created.v1" }).to_owned(),
        payload: if revision_upload {
            json!({"resource_id":resource_id, "resource_revision_id":revision_id, "parent_revision_ids":revision.parent_revision_ids, "content_digest":expected_digest, "size_bytes":session.expected_size_bytes, "media_type":session.media_type, "created_by":revision.created_by, "aggregate_version":resource.version})
        } else {
            json!({"resource_id":resource_id, "workspace_id":workspace_id, "kind":"FILE", "provenance":resource.provenance.clone(), "aggregate_version":1})
        },
        recorded_at: context.recorded_at,
    };
    let upload_status_event = if session.state == ResourceUploadState::ContentReceived {
        let lifecycle_context = event_context(&state.runtime_id, &correlation_id)
            .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource commit could not be initialized"))?;
        let aggregate_version = session.version.checked_add(1)
            .ok_or_else(|| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource commit could not be initialized"))?;
        Some(storage_core::EventDraft {
            event_id: lifecycle_context.event_id,
            workspace_id: workspace_id.clone(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: session.upload_id.clone(),
            origin_runtime_id: lifecycle_context.origin_runtime_id,
            entity_revision: aggregate_version,
            hlc_timestamp: lifecycle_context.hlc_timestamp,
            correlation_id: lifecycle_context.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.status.changed.v1".to_owned(),
            payload: json!({
                "upload_id": session.upload_id,
                "from": "CONTENT_RECEIVED",
                "to": "COMMITTED",
                "resource_id": resource_id,
                "aggregate_version": aggregate_version
            }),
            recorded_at: lifecycle_context.recorded_at,
        })
    } else {
        None
    };
    let upload_failure_event = if session.state == ResourceUploadState::ContentReceived {
        let lifecycle_context = event_context(&state.runtime_id, &correlation_id)
            .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource commit could not be initialized"))?;
        let aggregate_version = session.version.checked_add(1)
            .ok_or_else(|| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource commit could not be initialized"))?;
        Some(storage_core::EventDraft {
            event_id: lifecycle_context.event_id,
            workspace_id: workspace_id.clone(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: session.upload_id.clone(),
            origin_runtime_id: lifecycle_context.origin_runtime_id,
            entity_revision: aggregate_version,
            hlc_timestamp: lifecycle_context.hlc_timestamp,
            correlation_id: lifecycle_context.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "resource.upload.status.changed.v1".to_owned(),
            payload: json!({
                "upload_id": session.upload_id,
                "from": "CONTENT_RECEIVED",
                "to": "FAILED",
                "reason_code": "UPLOAD_CONTENT_INTEGRITY_FAILED",
                "aggregate_version": aggregate_version
            }),
            recorded_at: lifecycle_context.recorded_at,
        })
    } else {
        None
    };
    let request = WorkspaceCreateRequest {
        principal_id: state.principal_id.clone(), request_id: request_id.to_owned(),
        request_payload: json!({"operation":"resource.upload.commit.v1", "upload_id":session.upload_id.clone()}),
    };
    let store = state.store.clone();
    let upload_id = session.upload_id.clone();
    let committed = tokio::task::spawn_blocking(move || store.commit(request, &upload_id, resource, revision, event, upload_status_event, upload_failure_event)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource commit did not complete"))?
        .map_err(|error| match error {
            storage_core::StoreError::Conflict { .. } if revision_upload => operator_error(StatusCode::CONFLICT, "RESOURCE_CONFLICT", "Resource changed while the revision was uploading; start a new revision upload from current heads"),
            storage_core::StoreError::Invalid(message) if message.contains("ContextDocument is not active") => operator_error(StatusCode::CONFLICT, "CONTEXT_DOCUMENT_NOT_ACTIVE", "This ContextDocument is no longer active"),
            other => upload_store_error(other),
        })?;
    let correlation_id = committed.event.correlation_id.clone();
    let mut response = (StatusCode::CREATED, Json(CommittedResourceResponse { resource: committed.resource, revision: committed.revision, event: committed.event })).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    if let Ok(value) = header::HeaderValue::from_str(&correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn expire_upload_if_due(
    state: &ApiState,
    session: ResourceUploadSessionRecord,
) -> Result<ResourceUploadSessionRecord, Response> {
    if matches!(session.state, ResourceUploadState::Committed | ResourceUploadState::Failed | ResourceUploadState::Expired) {
        return Ok(session);
    }
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload expiry could not be recorded"))?;
    if now < session.expires_at {
        return Ok(session);
    }
    let next_version = session.version.checked_add(1)
        .ok_or_else(|| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload expiry could not be recorded"))?;
    let mut expired = session.clone();
    expired.state = ResourceUploadState::Expired;
    expired.version = next_version;
    let correlation_id = new_id("cor")
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload expiry could not be recorded"))?;
    let context = event_context(&state.runtime_id, &correlation_id)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload expiry could not be recorded"))?;
    let from = match session.state {
        ResourceUploadState::Open => "OPEN",
        ResourceUploadState::ContentReceived => "CONTENT_RECEIVED",
        _ => return Ok(session),
    };
    let event = storage_core::EventDraft {
        event_id: context.event_id,
        workspace_id: session.workspace_id.clone(),
        entity_type: "ResourceUpload".to_owned(),
        entity_id: session.upload_id.clone(),
        origin_runtime_id: context.origin_runtime_id,
        entity_revision: next_version,
        hlc_timestamp: context.hlc_timestamp,
        correlation_id: context.correlation_id,
        causation_id: None,
        schema_version: 1,
        event_type: "resource.upload.status.changed.v1".to_owned(),
        payload: json!({
            "upload_id": session.upload_id,
            "from": from,
            "to": "EXPIRED",
            "reason_code": "UPLOAD_TTL_ELAPSED",
            "aggregate_version": next_version
        }),
        recorded_at: context.recorded_at,
    };
    let store = state.store.clone();
    let fallback_store = store.clone();
    let expected_version = session.progress_version;
    let fallback_workspace_id = session.workspace_id.clone();
    let fallback_upload_id = session.upload_id.clone();
    match tokio::task::spawn_blocking(move || store.expire(expected_version, expired, event)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload expiry did not complete"))? {
        Ok(result) => Ok(result),
        Err(storage_core::StoreError::Conflict { .. }) => {
            let current = tokio::task::spawn_blocking(move || fallback_store.get(&fallback_workspace_id, &fallback_upload_id)).await
                .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload expiry could not be reconciled"))?
                .map_err(upload_store_error)?
                .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Upload session is unavailable"))?;
            if matches!(current.state, ResourceUploadState::Expired | ResourceUploadState::Committed | ResourceUploadState::Failed) {
                Ok(current)
            } else {
                Err(operator_error(StatusCode::CONFLICT, "CONFLICT", "Upload session changed while expiry was being recorded"))
            }
        }
        Err(error) => Err(upload_store_error(error)),
    }
}

/// Creates only the durable Task envelope. Planning and execution remain separate
/// admission steps; this route never starts a native agent session or labels the Task
/// as working.
async fn create_task(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let workspace = ensure_workspace_owner(&state, &workspace_id)?;
    if workspace.status != "ACTIVE" {
        return Err(operator_error(StatusCode::UNPROCESSABLE_ENTITY, "WORKSPACE_ARCHIVED", "Archived Workspaces cannot accept new Tasks"));
    }
    if body.get("workspace_id").and_then(serde_json::Value::as_str) != Some(workspace_id.as_str()) {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"));
    }
    let explicit_binding = match body.get("preferred_lead_agent_binding_id") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        Some(_) => return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task lead selection is invalid")),
    };
    let coworker_id = match body.get("coworker_id") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(value)) if valid_task_query_id(value) => Some(value.clone()),
        Some(_) => return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task Coworker selection is invalid")),
    };
    let expected_coworker_version = match body.get("expected_coworker_version") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Number(value)) => value.as_u64().filter(|version| *version > 0),
        Some(_) => None,
    };
    if body.get("expected_coworker_version").is_some_and(|value| !value.is_null())
        && expected_coworker_version.is_none()
    {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Expected Coworker version is invalid"));
    }
    if coworker_id.is_none() && expected_coworker_version.is_some() {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "A Coworker version requires a selected Coworker"));
    }

    // The client pins the selected Coworker ID and expected aggregate version, but
    // does not choose the origin revision. The server resolves the current immutable
    // revision; SQLite rechecks that head and version in the Task creation transaction.
    let coworker_origin = if let Some(coworker_id) = coworker_id.as_deref() {
        let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task request could not be initialized"))?;
        let context = event_context(&state.runtime_id, &correlation_id)
            .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task request could not be initialized"))?;
        let coworker_store = SqliteCoworkerStore::new(state.store.clone(), CoworkerEventContext {
            event_id: context.event_id,
            origin_runtime_id: context.origin_runtime_id,
            hlc_timestamp: context.hlc_timestamp,
            correlation_id: context.correlation_id,
            causation_id: context.causation_id,
            recorded_at: context.recorded_at,
        }).map_err(|_| operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Selected Coworker is unavailable"))?;
        let (head, revision) = coworker_store
            .get(&state.principal_id, &workspace_id, coworker_id)
            .map_err(|_| operator_error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE", "Selected Coworker is unavailable"))?
            .ok_or_else(|| operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "Selected Coworker is unavailable in this Workspace"))?;
        let failover_policy = revision.definition.lead_failover_policy
            .map(|policy| serde_json::Value::Object(policy.into_iter().collect()));
        Some((head.current_revision, revision.definition.default_lead_agent_binding_id, failover_policy))
    } else {
        None
    };
    let lead_binding_id = explicit_binding
        .or_else(|| coworker_origin.as_ref().and_then(|(_, binding, _)| binding.clone()))
        .or(workspace.default_agent_binding_id.clone())
        .ok_or_else(|| operator_error(StatusCode::UNPROCESSABLE_ENTITY, "AGENT_UNAVAILABLE", "Choose and enable a lead agent before creating a Task"))?;
    let request_id = idempotency_key(&headers)?;
    let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task request could not be initialized"))?;
    let task_id = new_id("tsk").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task request could not be initialized"))?;
    let event = event_context(&state.runtime_id, &correlation_id)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task request could not be initialized"))?;
    let command = CreateStandaloneTask {
        task_id,
        workspace_id: workspace_id.clone(),
        workspace_instruction_revision: workspace.current_instruction_revision,
        lead_agent_binding_id: lead_binding_id,
        origin_coworker_id: coworker_id,
        origin_coworker_revision: coworker_origin.as_ref().map(|(revision, _, _)| *revision),
        expected_coworker_version,
        coworker_default_lead_failover_policy: coworker_origin.as_ref().and_then(|(_, _, policy)| policy.clone()),
        principal_id: state.principal_id.clone(),
        request_id,
        request_payload: body,
        event,
    };
    let store = state.store.clone();
    let committed = tokio::task::spawn_blocking(move || TaskService::new(store).create_standalone(command))
        .await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task creation did not complete"))?
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(message) if message.contains("Coworker is archived") => operator_error(StatusCode::CONFLICT, "COWORKER_ARCHIVED", "An archived Coworker cannot be selected for new work"),
            storage_core::StoreError::Invalid(message) if message.contains("lead AgentBinding") => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "AGENT_UNAVAILABLE", "The selected lead AgentBinding is unavailable for new work"),
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task request is invalid or uses an unavailable origin path"),
            storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "CONFLICT", "Workspace or Task inputs changed; refresh and retry"),
            storage_core::StoreError::NotFound => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "A Task input or lead binding is unavailable"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task could not be saved"),
        })?;
    let mut response = (StatusCode::CREATED, Json(committed.view)).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

/// Task reads are historical reads: archive prevents new work, not reading
/// existing Tasks. Owner checks and SQLite reads run on the blocking pool.
async fn list_tasks(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<TaskListQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<TaskPageResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let Query(query) = query.map_err(|_| operator_error(
        StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task filters are invalid",
    ))?;
    if !valid_task_query_id(&workspace_id)
        || query.conversation_id.as_deref().is_some_and(|id| !valid_task_query_id(id))
        || query.status.as_deref().is_some_and(|status| !matches!(status,
            "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING"
            | "NEEDS_USER" | "INCOMPLETE" | "PAUSE_REQUESTED" | "PAUSED"
            | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED"))
    {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task filters are invalid"));
    }
    list_task_page(
        state,
        workspace_id,
        query.status,
        query.conversation_id,
        query.cursor,
        query.limit,
    ).await
}

async fn list_conversation_tasks(
    State(state): State<ApiState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
    query: Result<Query<ConversationTaskListQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<TaskPageResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let Query(query) = query.map_err(|_| operator_error(
        StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task page query is invalid",
    ))?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&conversation_id) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task query is invalid"));
    }
    list_task_page(
        state,
        workspace_id,
        None,
        Some(conversation_id),
        query.cursor,
        query.limit,
    ).await
}

async fn list_task_page(
    state: ApiState,
    workspace_id: String,
    status: Option<String>,
    conversation_id: Option<String>,
    encoded_cursor: Option<String>,
    requested_limit: Option<usize>,
) -> Result<Json<TaskPageResponse>, Response> {
    let limit = requested_limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task page limit must be between 1 and 200"));
    }
    let cursor = encoded_cursor.as_deref().map(|encoded| {
        decode_task_cursor(encoded, &workspace_id, status.as_deref(), conversation_id.as_deref())
    }).transpose()?;
    let scoped_workspace = workspace_id.clone();
    let status_filter = status.clone();
    let conversation_filter = conversation_id.clone();
    let mut rows = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner(&state, &scoped_workspace)?;
        state.store.list_tasks_page(
            &scoped_workspace, status_filter.as_deref(), conversation_filter.as_deref(),
            cursor.as_ref().map(|cursor| cursor.created_at.as_str()),
            cursor.as_ref().map(|cursor| cursor.task_id.as_str()), limit + 1,
        ).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task list is unavailable"))
    }).await.map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task list is unavailable"))??;
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = if has_more {
        rows.last().map(|last| {
            let cursor = TaskListCursor { version: 1, workspace_id,
                status, conversation_id,
                created_at: last.created_at.clone(), task_id: last.task_id.clone() };
            serde_json::to_vec(&cursor).map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
                .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task cursor could not be created"))
        }).transpose()?
    } else { None };
    Ok(Json(TaskPageResponse { items: rows, next_cursor }))
}

async fn get_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<TaskView>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&task_id) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task lookup is invalid"));
    }
    let view = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner(&state, &workspace_id)?;
        state.store.get_task(&workspace_id, &task_id)
            .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task is unavailable"))?
            .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Task is unavailable"))
    }).await.map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task is unavailable"))??;
    Ok(Json(view))
}

/// Read-only diagnostic. It prepares the same bounded assignment/packet as the
/// internal preflight, but never reserves a session or invokes a native provider.
async fn get_task_planning_readiness(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&task_id) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task planning readiness query is invalid"));
    }
    let expected_task_version = parse_if_match(&headers)?;
    if expected_task_version == 0 {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "If-Match must contain a positive Task version"));
    }
    let observed_at = operator_now()?;
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner(&state, &workspace_id)?;
        let task = state.store.get_task(&workspace_id, &task_id)
            .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task planning readiness is unavailable"))?
            .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Task is unavailable"))?;
        if task.task.version != expected_task_version {
            return Err(operator_error(StatusCode::CONFLICT, "STALE_TASK_VERSION", "Task changed; reload before checking planning readiness"));
        }

        let mut blockers = Vec::new();
        if task.task.current_plan_revision.is_some() {
            blockers.push(PlanningDispatchBlocker::PlanAlreadyAccepted);
        }
        if !matches!(task.task.status.as_str(), "READY" | "RUNNING") {
            blockers.push(PlanningDispatchBlocker::TaskStateNotEligible);
        }
        if blockers.is_empty() {
            let request = PreparePlanningAssignment {
                owner_principal_id: state.principal_id.clone(),
                workspace_id: workspace_id.clone(),
                task_id: task_id.clone(),
                expected_task_version,
                runtime_id: state.runtime_id.clone(),
                runtime_incarnation_id: state.local_incarnation_id.clone(),
                now: observed_at.clone(),
            };
            match prepare_local_task_planning(state.store.clone(), request) {
                Ok(preflight) => blockers.extend(preflight.view().dispatch_blockers.iter().copied()),
                Err(StoreError::Conflict { .. }) => {
                    return Err(operator_error(StatusCode::CONFLICT, "STALE_TASK_VERSION", "Task changed; reload before checking planning readiness"));
                }
                Err(StoreError::NotFound | StoreError::Invalid(_)) => {
                    blockers.push(PlanningDispatchBlocker::CurrentLeadOrEndpointUnavailable);
                }
                Err(_) => {
                    return Err(operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task planning readiness is unavailable"));
                }
            }
        }

        let response = TaskPlanningReadinessResponse {
            task_id: task.task.task_id,
            task_version: task.task.version,
            task_spec_revision: task.current_spec_revision.revision,
            task_status: task.task.status,
            observed_at,
            dispatch_available: false,
            planning_started: false,
            agent_session_started: false,
            plan_created: false,
            blockers,
        };
        Ok(Json(response).into_response())
    }).await.map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task planning readiness is unavailable"))??;
    let mut response = result;
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn list_task_spec_revisions(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<storage_core::TaskSpecRevisionRecord>>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&task_id) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task specification history lookup is invalid"));
    }
    let revisions = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner(&state, &workspace_id)?;
        state.store.list_task_spec_revisions(&workspace_id, &task_id)
    }).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task specification history is unavailable"))?
        .map_err(|error| match error {
            storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Task is unavailable"),
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task specification history lookup is invalid"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task specification history is unavailable"),
        })?;
    Ok(Json(revisions))
}

/// Appends an owner-authored immutable revision only while a saved Task remains READY
/// and unplanned. The Task service and SQLite transaction repeat these admission checks.
async fn revise_task_spec(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ReviseTaskSpecBody>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&task_id) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task specification identity is invalid"));
    }
    ensure_workspace_owner(&state, &workspace_id)?;
    let request_id = idempotency_key(&headers)?;
    let expected_task_version = parse_if_match(&headers)?;
    let correlation_id = new_id("cor")
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task edit could not be initialized"))?;
    let event = event_context(&state.runtime_id, &correlation_id)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task edit could not be initialized"))?;
    let normalized_body = serde_json::to_value(&body)
        .map_err(|_| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task edit body is invalid"))?;
    let request_payload = json!({
        "operation": "task.spec.revise.v1",
        "workspace_id": workspace_id,
        "task_id": task_id,
        "expected_task_version": expected_task_version,
        "body": normalized_body,
    });
    let command = ReviseTaskSpec {
        workspace_id: workspace_id.clone(),
        task_id: task_id.clone(),
        principal_id: state.principal_id.clone(),
        request_id,
        request_payload,
        expected_task_version,
        parent_revisions: body.parent_revisions,
        objective: body.objective,
        constraints: body.constraints,
        non_goals: body.non_goals,
        input_refs: body.input_refs,
        required_outputs: body.required_outputs,
        acceptance_criteria: body.acceptance_criteria,
        approvals_required: body.approvals_required,
        placement_preference: body.placement_preference,
        preferred_lead_agent_binding_id: body.preferred_lead_agent_binding_id,
        lead_failover_policy: body.lead_failover_policy,
        event,
    };
    let store = state.store.clone();
    let committed = tokio::task::spawn_blocking(move || TaskService::new(store).revise_saved_spec(command)).await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task edit did not complete"))?
        .map_err(|error| match error {
            storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Task or pinned input is unavailable"),
            storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "CONFLICT", "Task changed or is no longer editable; reload before retrying"),
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "Task edit fields are invalid"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task edit could not be saved"),
        })?;
    let mut response = (StatusCode::CREATED, Json(committed.revision)).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn list_task_plan_revisions(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<storage_core::PlanRevisionRecord>>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&task_id) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Plan history lookup is invalid"));
    }
    let rows = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner(&state, &workspace_id)?;
        state.store.list_plan_revisions(&workspace_id, &task_id)
    })
        .await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Plan history is unavailable"))?
        .map_err(|error| match error {
            storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Task is unavailable"),
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Plan history query is invalid"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Plan history is unavailable"),
        })?;
    Ok(Json(rows))
}

async fn list_task_steps(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<storage_core::StepRecord>>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if !valid_task_query_id(&workspace_id) || !valid_task_query_id(&task_id) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Step lookup is invalid"));
    }
    let rows = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner(&state, &workspace_id)?;
        state.store.list_steps(&workspace_id, &task_id, None)
    })
        .await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task Steps are unavailable"))?
        .map_err(|error| match error {
            storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Task is unavailable"),
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Step query is invalid"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Task Steps are unavailable"),
        })?;
    Ok(Json(rows))
}

fn valid_task_query_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 200 && !value.chars().any(char::is_control)
}

fn decode_task_cursor(encoded: &str, workspace_id: &str, status: Option<&str>,
    conversation_id: Option<&str>) -> Result<TaskListCursor, Response>
{
    let invalid = || operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Task cursor is invalid or does not match these filters");
    if encoded.is_empty() || encoded.len() > 2048 { return Err(invalid()); }
    let bytes = URL_SAFE_NO_PAD.decode(encoded.as_bytes()).map_err(|_| invalid())?;
    let cursor: TaskListCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1 || cursor.workspace_id != workspace_id
        || cursor.status.as_deref() != status || cursor.conversation_id.as_deref() != conversation_id
        || !valid_task_query_id(&cursor.task_id) || cursor.created_at.len() > 64
        || time::OffsetDateTime::parse(&cursor.created_at, &time::format_description::well_known::Rfc3339).is_err()
    { return Err(invalid()); }
    Ok(cursor)
}

fn ensure_workspace_owner(state: &ApiState, workspace_id: &str) -> Result<Workspace, Response> {
    let workspace = state.store.get_workspace(workspace_id).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Workspace is unavailable"))?;
    workspace.filter(|workspace| workspace.owner_principal_id == state.principal_id)
        .ok_or_else(|| operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"))
}

fn idempotency_key(headers: &HeaderMap) -> Result<&str, Response> {
    headers.get("idempotency-key").and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 128 && value.bytes().all(|byte| byte.is_ascii_graphic()))
        .ok_or_else(|| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "A valid Idempotency-Key is required"))
}

fn parse_content_range(value: &str) -> Option<(u64, u64, u64)> {
    let value = value.strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse().ok()?;
    let end = end.parse().ok()?;
    let total = total.parse().ok()?;
    (end >= start && end < total).then_some((start, end, total))
}

fn upload_store_error(error: storage_core::StoreError) -> Response {
    match error {
        storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Upload session is unavailable"),
        storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "UPLOAD_OFFSET_CONFLICT", "Upload chunk conflicts with previously accepted content"),
        storage_core::StoreError::LegacyUploadCommitNeedsReview { committed_resource_id: Some(_) } => operator_error(StatusCode::CONFLICT, "RESOURCE_CONFLICT", "An older upload is already committed. Review its recorded Resource before retrying; LiteCowork did not create another Resource."),
        storage_core::StoreError::LegacyUploadCommitNeedsReview { committed_resource_id: None } => operator_error(StatusCode::CONFLICT, "RESOURCE_CONFLICT", "An older upload is already committed, but its Resource mapping is unavailable. Review the Workspace before retrying; LiteCowork did not create another Resource."),
        storage_core::StoreError::Invalid(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "Upload state or content is invalid"),
        storage_core::StoreError::Integrity(_) | storage_core::StoreError::Blob(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INTEGRITY_FAILURE", "Upload content failed integrity checks"),
        _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource upload could not be completed"),
    }
}

async fn list_resources(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ResourceListQuery>,
) -> Result<Json<ResourceListResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let workspace = state.store.get_workspace(&workspace_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace is unavailable",
        )
    })?;
    if !workspace.is_some_and(|workspace| workspace.owner_principal_id == state.principal_id) {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=100).contains(&limit) {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource page limit must be between 1 and 100",
        ));
    }
    let cursor = query
        .cursor
        .as_deref()
        .map(|encoded| decode_resource_cursor(encoded, &workspace_id))
        .transpose()?;
    let rows = state
        .store
        .list_resources_page(
            &workspace_id,
            cursor.as_ref().map(|value| value.created_at.as_str()),
            cursor.as_ref().map(|value| value.resource_id.as_str()),
            limit + 1,
        )
        .map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Resource list is unavailable",
        )
    })?;
    let has_more = rows.len() > limit;
    let mut items = rows;
    if has_more {
        items.truncate(limit);
    }
    let next_cursor = if has_more {
        items.last().map(encode_resource_cursor).transpose()?
    } else {
        None
    };
    Ok(Json(ResourceListResponse { items, next_cursor }))
}

async fn list_workspace_roots(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<WorkspaceRootListQuery>,
) -> Result<Json<WorkspaceRootListResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let status = query.status.as_deref();
    if status.is_some_and(|value| !matches!(value, "ACTIVE" | "PAUSED" | "REVOKED" | "UNAVAILABLE")) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "WorkspaceRoot status filter is invalid"));
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=100).contains(&limit) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "WorkspaceRoot page limit must be between 1 and 100"));
    }
    let cursor = query.cursor.as_deref().map(|encoded| decode_workspace_root_cursor(encoded, &workspace_id, status)).transpose()?;
    let rows = WorkspaceRootService::new(state.store.clone())
        .list_roots(
            &workspace_id,
            &state.principal_id,
            status,
            cursor.as_ref().map(|value| value.created_at.as_str()),
            cursor.as_ref().map(|value| value.workspace_root_id.as_str()),
            limit + 1,
        )
        .map_err(|error| match error {
            StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Workspace is unavailable"),
            StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "WorkspaceRoot query is invalid"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "WorkspaceRoot list is unavailable"),
        })?;
    let has_more = rows.len() > limit;
    let mut items = rows;
    if has_more {
        items.truncate(limit);
    }
    let next_cursor = if has_more {
        items.last().map(|item| encode_workspace_root_cursor(&item.root, status)).transpose()?
    } else {
        None
    };
    Ok(Json(WorkspaceRootListResponse { items, next_cursor }))
}

async fn revoke_workspace_root(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(workspace_root_id): Path<String>,
) -> Result<Response, Response> {
    change_workspace_root_status(state, headers, workspace_root_id, WorkspaceRootStatusAction::Revoke).await
}

async fn pause_workspace_root(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(workspace_root_id): Path<String>,
) -> Result<Response, Response> {
    change_workspace_root_status(state, headers, workspace_root_id, WorkspaceRootStatusAction::Pause).await
}

async fn resume_workspace_root(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(workspace_root_id): Path<String>,
) -> Result<Response, Response> {
    change_workspace_root_status(state, headers, workspace_root_id, WorkspaceRootStatusAction::Resume).await
}

async fn change_workspace_root_status(
    state: ApiState,
    headers: HeaderMap,
    workspace_root_id: String,
    action: WorkspaceRootStatusAction,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if workspace_root_id.trim().is_empty() || workspace_root_id.len() > 160 {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "WorkspaceRoot selection is invalid"));
    }
    ensure_workspace_owner(&state, &workspace_id)?;
    let expected_version = parse_if_match(&headers)?;
    let request_id = idempotency_key(&headers)?;
    let workspace_id_for_errors = workspace_id.clone();
    let service = WorkspaceRootService::new(state.store.clone());
    let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "WorkspaceRoot request could not be initialized"))?;
    let event = event_context(&state.runtime_id, &correlation_id)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "WorkspaceRoot request could not be initialized"))?;
    if action == WorkspaceRootStatusAction::Resume {
        let committed = crate::runtime::resume_workspace_root_live(
            &state.store,
            &state.runtime_id,
            &state.local_incarnation_id,
            &state.runtime_identity,
            &workspace_id,
            &workspace_root_id,
            &state.principal_id,
            request_id,
            expected_version,
            event,
        ).map_err(|error| match error {
            crate::runtime::WorkspaceRootResumeError::Stale => operator_error(StatusCode::CONFLICT, "STALE_WORKSPACE_VERSION", "Folder state changed; refresh before retrying"),
            crate::runtime::WorkspaceRootResumeError::Unavailable => operator_error(StatusCode::CONFLICT, "RESOURCE_IDENTITY_UNAVAILABLE", "The saved folder could not be verified. Check the original folder or remove and add it again."),
            crate::runtime::WorkspaceRootResumeError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "WorkspaceRoot is unavailable"),
            crate::runtime::WorkspaceRootResumeError::Internal => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Folder identity could not be checked"),
        })?;
        let mut response = Json(WorkspaceRootStatusResponse {
            root: committed.root,
            location_availability: committed.location_availability.unwrap_or_else(|| "UNKNOWN".to_owned()),
        }).into_response();
        if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
            response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
        }
        return Ok(response);
    }
    let committed = service.change_root_status(ChangeWorkspaceRootStatus {
            workspace_id,
            workspace_root_id,
            principal_id: state.principal_id,
            request_id,
            expected_version,
            action,
            runtime_id: None,
            runtime_incarnation_id: None,
            event,
        })
        .map_err(|error| match error {
            StoreError::Conflict { expected: Some(expected), actual: Some(actual) } if expected == actual => operator_error(StatusCode::CONFLICT, "CONFLICT", "Folder is no longer in a state that supports this action. Refresh the Workspace."),
            StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "STALE_WORKSPACE_VERSION", "Folder state changed; refresh before retrying"),
            StoreError::NotFound => {
                let archived = state.store.get_workspace(&workspace_id_for_errors).ok().flatten()
                    .is_some_and(|workspace| workspace.status == "ARCHIVED");
                if archived {
                    operator_error(StatusCode::CONFLICT, "WORKSPACE_ARCHIVED", "Archived Workspaces are read-only")
                } else {
                    operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "WorkspaceRoot is unavailable")
                }
            }
            StoreError::Invalid(message) if message == "an archived Workspace is read-only" => {
                operator_error(StatusCode::CONFLICT, "WORKSPACE_ARCHIVED", "Archived Workspaces are read-only")
            }
            StoreError::Invalid(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "WorkspaceRoot status change is invalid"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Folder status could not be changed"),
        })?;
    let mut response = Json(WorkspaceRootStatusResponse {
        root: committed.root,
        location_availability: committed.location_availability.unwrap_or_else(|| "UNKNOWN".to_owned()),
    }).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn rebuild_resource_text_index(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(resource_id): Path<String>,
    Json(body): Json<RebuildResourceTextIndexBody>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let request_id = idempotency_key(&headers)?.to_owned();
    if resource_id.is_empty()
        || resource_id.len() > 160
        || !resource_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        || body.resource_revision_id.is_empty()
        || body.resource_revision_id.len() > 160
        || !body.resource_revision_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        || !storage_core::is_sha256_digest(&body.content_digest)
    {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource index rebuild identity is invalid"));
    }
    let indexed_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource index rebuild could not be initialized"))?;
    let correlation_id = new_id("cor")
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource index rebuild could not be initialized"))?;
    let request = ResourceTextIndexRebuildRequest {
        principal_id: state.principal_id.clone(),
        request_id,
        workspace_id: workspace_id.clone(),
        resource_id,
        resource_revision_id: body.resource_revision_id,
        content_digest: body.content_digest,
        indexed_at,
        correlation_id,
    };
    let store = state.store.clone();
    let result = tokio::task::spawn_blocking(move || store.reindex_resource_text(request))
        .await
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource index rebuild did not complete"))?
        .map_err(|error| match error {
            StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Resource is unavailable"),
            StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "CONFLICT", "The Resource head or index eligibility changed, or this request ID was reused with a different pin. Reload the Library before retrying."),
            StoreError::Invalid(message) if message == "CONTEXT_DOCUMENT_REVOKED" => operator_error(StatusCode::CONFLICT, "CONTEXT_DOCUMENT_NOT_ACTIVE", "This ContextDocument was revoked. Its retained content is unavailable until it is restored."),
            StoreError::Invalid(message) if message == "CONTEXT_DOCUMENT_DELETION_PENDING" => operator_error(StatusCode::CONFLICT, "CONTEXT_DOCUMENT_NOT_ACTIVE", "This ContextDocument is being deleted; its content is unavailable."),
            StoreError::Invalid(message) if message == "CONTEXT_DOCUMENT_DELETED" => operator_error(StatusCode::CONFLICT, "CONTEXT_DOCUMENT_NOT_ACTIVE", "This ContextDocument has been deleted; its content is unavailable."),
            StoreError::Invalid(message) if message == "an archived Workspace is read-only" => operator_error(StatusCode::CONFLICT, "WORKSPACE_ARCHIVED", "Archived Workspaces are read-only"),
            StoreError::Invalid(_) => operator_error(StatusCode::CONFLICT, "RESOURCE_UNAVAILABLE", "Resource index rebuild is unavailable for this Workspace or source."),
            StoreError::Blob(_) => operator_error(StatusCode::SERVICE_UNAVAILABLE, "RESOURCE_UNAVAILABLE", "The local encrypted Resource index is unavailable."),
            StoreError::Integrity(_) => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTEGRITY_FAILURE", "Resource index rebuild could not verify its source."),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource index rebuild could not be completed."),
        })?;
    let response_correlation_id = result.correlation_id.clone();
    let mut response = Json(result).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    if let Ok(value) = header::HeaderValue::from_str(&response_correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn search_resources(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ResourceSearchQuery>,
) -> Result<Json<ResourceSearchPageResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let workspace = state.store.get_workspace(&workspace_id).map_err(|_| {
        operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Workspace is unavailable")
    })?;
    if !workspace.is_some_and(|workspace| workspace.owner_principal_id == state.principal_id) {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"));
    }
    if query.q.as_ref().is_some_and(|value| value.len() > 256 || value.contains('\0'))
        || query.mode.as_deref().is_some_and(|value| !matches!(value, "METADATA" | "ON_DEMAND_CONTENT" | "INDEXED_CONTENT"))
        || query.kind.as_ref().is_some_and(|value| value.len() > 64 || value.contains('\0'))
        || query.kind.as_deref().is_some_and(|value| !matches!(value, "FILE" | "FOLDER" | "ARTIFACT" | "CONNECTOR_OBJECT" | "WEB_RESOURCE" | "OTHER"))
        || query.freshness.as_deref().is_some_and(|value| !matches!(value, "CURRENT" | "STALE" | "UNKNOWN" | "UNAVAILABLE"))
    {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource search filters are invalid"));
    }
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource search limit must be between 1 and 200"));
    }
    let mode = query.mode.as_deref().unwrap_or("METADATA");
    let content_scan = mode == "ON_DEMAND_CONTENT";
    let indexed_content = mode == "INDEXED_CONTENT";
    if (content_scan || indexed_content) && query.q.as_deref().is_none_or(|value| value.trim().is_empty()) {
        let mode_label = if indexed_content { "Indexed content" } else { "On-demand content" };
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", &format!("{mode_label} search requires a non-empty query")));
    }
    let cursor = query.cursor.as_deref().map(|encoded| {
        if encoded.is_empty() || encoded.len() > 4096 {
            return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource search cursor is invalid"));
        }
        let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| {
            operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource search cursor is invalid")
        })?;
        let decoded: ResourceSearchCursor = serde_json::from_slice(&bytes).map_err(|_| {
            operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource search cursor is invalid")
        })?;
        if decoded.workspace_id != workspace_id
            || decoded.query != query.q
            || decoded.mode != mode
            || decoded.kind != query.kind
            || decoded.freshness != query.freshness
            || decoded.created_at.is_empty()
            || decoded.created_at.len() > 64
            || decoded.resource_id.is_empty()
            || decoded.resource_id.len() > 160
        {
            return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource search cursor does not match these filters"));
        }
        Ok(decoded)
    }).transpose()?;
    if indexed_content {
        let rows = state.store.search_indexed_resource_text(
            &workspace_id,
            query.q.as_deref().unwrap_or_default(),
            query.kind.as_deref(),
            query.freshness.as_deref(),
            cursor.as_ref().map(|value| value.created_at.as_str()),
            cursor.as_ref().map(|value| value.resource_id.as_str()),
            limit + 1,
        ).map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Indexed Resource search query is invalid"),
            storage_core::StoreError::Blob(_) => operator_error(StatusCode::SERVICE_UNAVAILABLE, "RESOURCE_UNAVAILABLE", "The local encrypted Resource index is unavailable"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTEGRITY_FAILURE", "Indexed Resource search could not verify its source"),
        })?;
        let has_more = rows.len() > limit;
        let rows = rows.into_iter().take(limit).collect::<Vec<_>>();
        let next_cursor = if has_more {
            rows.last().map(|row| encode_resource_search_cursor(
                &workspace_id,
                query.q.as_deref(),
                mode,
                query.kind.as_deref(),
                query.freshness.as_deref(),
                &row.result.summary,
            )).transpose()?
        } else { None };
        let items = rows.into_iter().map(|row| {
            search_result_response(&row.result, Some(row.snippet), row.result.match_reasons.clone())
        }).collect();
        return Ok(Json(ResourceSearchPageResponse {
            items,
            next_cursor,
            mode: mode.to_owned(),
            content_scan: None,
        }));
    }
    let candidate_limit = if content_scan { limit.min(MAX_CONTENT_SCAN_CANDIDATES) } else { limit };
    let rows = state.store.search_resources_page(
        &workspace_id,
        if content_scan { None } else { query.q.as_deref() },
        query.kind.as_deref(),
        query.freshness.as_deref(),
        cursor.as_ref().map(|value| value.created_at.as_str()),
        cursor.as_ref().map(|value| value.resource_id.as_str()),
        candidate_limit + 1,
    ).map_err(|error| match error {
        storage_core::StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Resource search query is invalid"),
        _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource search is unavailable"),
    })?;
    let has_candidate_page = rows.len() > candidate_limit;
    let rows = rows.into_iter().take(candidate_limit).collect::<Vec<_>>();
    let mut items = Vec::new();
    let mut scan_info = ResourceContentScanInfo {
        candidates_scanned: 0,
        text_resources_checked: 0,
        skipped_unsupported_type: 0,
        skipped_over_file_limit: 0,
        skipped_revision_changed: 0,
        byte_budget_exhausted: false,
        candidate_budget_exhausted: content_scan && candidate_limit == MAX_CONTENT_SCAN_CANDIDATES && has_candidate_page,
        max_candidates: MAX_CONTENT_SCAN_CANDIDATES,
        max_file_bytes: MAX_CONTENT_SCAN_FILE_BYTES,
        max_total_bytes: MAX_CONTENT_SCAN_TOTAL_BYTES,
    };
    let mut bytes_scanned = 0_u64;
    let mut last_scanned = None;
    let mut stopped_for_byte_budget = false;

    for row in &rows {
        if !content_scan {
            items.push(search_result_response(row, None, row.match_reasons.clone()));
            last_scanned = Some(row);
            continue;
        }

        let metadata_reasons = metadata_match_reasons(row, query.q.as_deref());
        let mut snippet = None;

        if !is_allowlisted_plain_text(&row.summary.display_name, &row.summary.media_type) {
            scan_info.candidates_scanned += 1;
            scan_info.skipped_unsupported_type += 1;
        } else if row.summary.size_bytes > MAX_CONTENT_SCAN_FILE_BYTES {
            scan_info.candidates_scanned += 1;
            scan_info.skipped_over_file_limit += 1;
        } else if bytes_scanned.saturating_add(row.summary.size_bytes) > MAX_CONTENT_SCAN_TOTAL_BYTES {
            scan_info.byte_budget_exhausted = true;
            stopped_for_byte_budget = true;
            break;
        } else {
            scan_info.candidates_scanned += 1;
            let content = state.store.read_resource_content_bounded(
                &workspace_id,
                &row.summary.resource_id,
                MAX_CONTENT_SCAN_FILE_BYTES,
            ).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTEGRITY_FAILURE", "Resource content search could not verify a candidate"))?;
            if let Some(content) = content {
                if content.summary.resource_revision_id != row.summary.resource_revision_id
                    || content.summary.content_digest != row.summary.content_digest
                {
                    scan_info.skipped_revision_changed += 1;
                } else {
                    bytes_scanned = bytes_scanned.saturating_add(content.content.len() as u64);
                    scan_info.text_resources_checked += 1;
                    if let (Some(text), Some(needle)) = (
                        std::str::from_utf8(&content.content).ok(),
                        query.q.as_deref(),
                    ) {
                        snippet = content_match_snippet(text, needle, MAX_CONTENT_SCAN_SNIPPET_CHARS);
                    }
                }
            }
        }

        let mut reasons = metadata_reasons;
        if snippet.is_some() {
            reasons.push("CONTENT_ON_DEMAND".to_owned());
        }
        if !reasons.is_empty() {
            items.push(search_result_response(row, snippet, reasons));
        }
        last_scanned = Some(row);
    }

    let page_was_truncated = content_scan && (has_candidate_page || stopped_for_byte_budget);
    let next_cursor = if page_was_truncated {
        last_scanned.map(|row| encode_resource_search_cursor(
            &workspace_id,
            query.q.as_deref(),
            mode,
            query.kind.as_deref(),
            query.freshness.as_deref(),
            &row.summary,
        )).transpose()?
    } else if !content_scan && has_candidate_page {
        rows.last().map(|row| encode_resource_search_cursor(
            &workspace_id,
            query.q.as_deref(),
            mode,
            query.kind.as_deref(),
            query.freshness.as_deref(),
            &row.summary,
        )).transpose()?
    } else { None };
    Ok(Json(ResourceSearchPageResponse {
        items,
        next_cursor,
        mode: mode.to_owned(),
        content_scan: content_scan.then_some(scan_info),
    }))
}

const MAX_CONTENT_SCAN_CANDIDATES: usize = 20;
const MAX_CONTENT_SCAN_FILE_BYTES: u64 = 1_048_576;
const MAX_CONTENT_SCAN_TOTAL_BYTES: u64 = 8_388_608;
const MAX_CONTENT_SCAN_SNIPPET_CHARS: usize = 320;

fn is_allowlisted_plain_text(display_name: &str, media_type: &str) -> bool {
    let name = display_name.to_ascii_lowercase();
    let media_type = media_type.split(';').next().unwrap_or(media_type).trim().to_ascii_lowercase();
    // A misleading text extension must not make a known archive/rich/binary format
    // searchable as text. The extension allowlist below is useful for local files whose
    // upload media type is generic, but it never overrides a known non-text type.
    if name.ends_with(".zip")
        || media_type == "application/zip"
        || media_type == "application/x-zip-compressed"
        || media_type == "application/pdf"
        || media_type.contains("officedocument")
        || media_type == "application/msword"
        || media_type == "application/vnd.ms-excel"
        || media_type == "application/vnd.ms-powerpoint"
        || media_type.starts_with("image/")
        || media_type.starts_with("audio/")
        || media_type.starts_with("video/")
    {
        return false;
    }
    let allowed_extension = [
        ".txt", ".md", ".markdown", ".csv", ".json", ".jsonl", ".ndjson",
        ".rs", ".py", ".toml", ".yaml", ".yml", ".js", ".jsx", ".ts", ".tsx", ".css",
    ].iter().any(|extension| name.ends_with(extension));
    let allowed_media_type = matches!(media_type.as_str(),
        "text/plain" | "text/markdown" | "text/csv" | "application/json" | "application/x-ndjson" | "application/jsonl"
    ) || (media_type.starts_with("text/") && matches!(media_type.as_str(), "text/x-rust" | "text/x-python" | "text/javascript" | "text/typescript" | "text/x-toml" | "text/yaml"));
    allowed_extension || allowed_media_type
}

fn metadata_match_reasons(row: &ResourceSearchRecord, query: Option<&str>) -> Vec<String> {
    let Some(query) = query else { return Vec::new(); };
    let needle = query.to_ascii_lowercase();
    let mut reasons = Vec::new();
    if row.summary.display_name.to_ascii_lowercase().contains(&needle) {
        reasons.push("NAME".to_owned());
    }
    if row.summary.media_type.to_ascii_lowercase().contains(&needle) {
        reasons.push("MEDIA_TYPE".to_owned());
    }
    reasons
}

fn content_match_snippet(text: &str, query: &str, maximum_chars: usize) -> Option<String> {
    if query.is_empty()
        || text.contains('\0')
        || text.chars().any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return None;
    }
    let lowered = text.to_ascii_lowercase();
    let needle = query.to_ascii_lowercase();
    let match_start = lowered.find(&needle)?;
    let match_end = match_start.checked_add(needle.len())?;
    let mut start = match_start.saturating_sub(maximum_chars / 2);
    while !text.is_char_boundary(start) { start = start.saturating_sub(1); }
    let mut end = (match_end + maximum_chars / 2).min(text.len());
    while end < text.len() && !text.is_char_boundary(end) { end += 1; }
    let excerpt = &text[start..end];
    let ellipsis_count = usize::from(start > 0) + usize::from(end < text.len());
    let mut snippet = excerpt.chars().take(maximum_chars.saturating_sub(ellipsis_count)).map(|character| {
        if matches!(character, '\n' | '\r' | '\t') { ' ' } else { character }
    }).collect::<String>();
    snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
    if start > 0 { snippet.insert(0, '…'); }
    if end < text.len() { snippet.push('…'); }
    Some(snippet)
}

fn search_result_response(row: &ResourceSearchRecord, snippet: Option<String>, match_reasons: Vec<String>) -> ResourceSearchResultResponse {
    let location = ResourceSearchLocationResponse {
        location_id: row.location_id.clone(),
        resource_id: row.summary.resource_id.clone(),
        runtime_id: None,
        environment_id: None,
        connection_id: None,
        availability: row.availability.clone(),
        writable: row.writable,
        observed_revision_id: row.observed_revision_id.clone(),
        observed_digest: row.observed_digest.clone(),
        observed_at: row.observed_at.clone(),
        last_checked_at: row.last_checked_at.clone(),
        freshness: row.freshness.clone(),
    };
    ResourceSearchResultResponse {
        resource_ref: ResourceSearchRefResponse {
            workspace_id: row.summary.workspace_id.clone(),
            resource_id: row.summary.resource_id.clone(),
            revision_id: Some(row.summary.resource_revision_id.clone()),
        },
        display_name: row.summary.display_name.clone(),
        locations: vec![location],
        freshness: row.freshness.clone(),
        match_reasons,
        snippet,
    }
}

fn encode_resource_search_cursor(
    workspace_id: &str,
    query: Option<&str>,
    mode: &str,
    kind: Option<&str>,
    freshness: Option<&str>,
    summary: &ResourceSummary,
) -> Result<String, Response> {
    let cursor = ResourceSearchCursor {
        workspace_id: workspace_id.to_owned(),
        query: query.map(str::to_owned),
        mode: mode.to_owned(),
        kind: kind.map(str::to_owned),
        freshness: freshness.map(str::to_owned),
        created_at: summary.created_at.clone(),
        resource_id: summary.resource_id.clone(),
    };
    serde_json::to_vec(&cursor).map(|bytes| URL_SAFE_NO_PAD.encode(bytes)).map_err(|_| {
        operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Resource search cursor could not be created")
    })
}

fn encode_resource_cursor(summary: &ResourceSummary) -> Result<String, Response> {
    let cursor = ResourceListCursor {
        workspace_id: summary.workspace_id.clone(),
        created_at: summary.created_at.clone(),
        resource_id: summary.resource_id.clone(),
    };
    serde_json::to_vec(&cursor)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Resource cursor could not be created",
            )
        })
}

fn encode_workspace_root_cursor(
    root: &WorkspaceRootRecord,
    status: Option<&str>,
) -> Result<String, Response> {
    let cursor = WorkspaceRootListCursor {
        workspace_id: root.workspace_id.clone(),
        status: status.map(str::to_owned),
        created_at: root.created_at.clone(),
        workspace_root_id: root.workspace_root_id.clone(),
    };
    serde_json::to_vec(&cursor)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "WorkspaceRoot cursor could not be created"))
}

fn decode_workspace_root_cursor(
    encoded: &str,
    workspace_id: &str,
    status: Option<&str>,
) -> Result<WorkspaceRootListCursor, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "WorkspaceRoot cursor is invalid"));
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded.as_bytes()).map_err(|_| {
        operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "WorkspaceRoot cursor is invalid")
    })?;
    let cursor: WorkspaceRootListCursor = serde_json::from_slice(&bytes).map_err(|_| {
        operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "WorkspaceRoot cursor is invalid")
    })?;
    if cursor.workspace_id != workspace_id
        || cursor.status.as_deref() != status
        || cursor.created_at.is_empty()
        || cursor.created_at.len() > 64
        || cursor.workspace_root_id.is_empty()
        || cursor.workspace_root_id.len() > 160
    {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "WorkspaceRoot cursor is invalid"));
    }
    Ok(cursor)
}

fn decode_resource_cursor(
    encoded: &str,
    workspace_id: &str,
) -> Result<ResourceListCursor, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource cursor is invalid",
        ));
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| {
        operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource cursor is invalid",
        )
    })?;
    let cursor: ResourceListCursor = serde_json::from_slice(&bytes).map_err(|_| {
        operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource cursor is invalid",
        )
    })?;
    if cursor.workspace_id != workspace_id
        || cursor.created_at.is_empty()
        || cursor.created_at.len() > 64
        || cursor.resource_id.is_empty()
        || cursor.resource_id.len() > 160
    {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource cursor does not belong to this Workspace",
        ));
    }
    Ok(cursor)
}

async fn read_resource_content(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(resource_id): Path<String>,
    Query(query): Query<ResourceContentQuery>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let workspace = state.store.get_workspace(&workspace_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace is unavailable",
        )
    })?;
    if !workspace.is_some_and(|workspace| workspace.owner_principal_id == state.principal_id) {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    if query.revision_id.as_ref().is_some_and(|revision_id| {
        revision_id.is_empty() || revision_id.len() > 160 || revision_id.contains('\0')
    }) {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource revision selection is invalid",
        ));
    }
    let content = state
        .store
        .read_resource_content_bounded(
            &workspace_id,
            &resource_id,
            MAX_OPERATOR_RESPONSE_BYTES as u64,
        )
        .map_err(|error| {
            match error {
                storage_core::StoreError::Invalid(message) if message == "RESOURCE_READ_LIMIT_EXCEEDED" => {
                    operator_error(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "INVALID_ARGUMENT",
                        "Resource exceeds the local Operator read limit",
                    )
                }
                storage_core::StoreError::Invalid(message) if message == "CONTEXT_DOCUMENT_REVOKED" => {
                    operator_error(
                        StatusCode::CONFLICT,
                        "CONTEXT_DOCUMENT_NOT_ACTIVE",
                        "This ContextDocument was revoked. Its retained content is unavailable until it is restored.",
                    )
                }
                storage_core::StoreError::Invalid(message) if message == "CONTEXT_DOCUMENT_DELETION_PENDING" => {
                    operator_error(
                        StatusCode::CONFLICT,
                        "CONTEXT_DOCUMENT_NOT_ACTIVE",
                        "This ContextDocument is being deleted; its content is unavailable.",
                    )
                }
                storage_core::StoreError::Invalid(message) if message == "CONTEXT_DOCUMENT_DELETED" => {
                    operator_error(
                        StatusCode::CONFLICT,
                        "CONTEXT_DOCUMENT_NOT_ACTIVE",
                        "This ContextDocument has been deleted; its content is unavailable.",
                    )
                }
                storage_core::StoreError::NotFound
                | storage_core::StoreError::Blob(_)
                | storage_core::StoreError::Io(_) => operator_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "RESOURCE_LOCATION_UNAVAILABLE",
                    "Resource content is temporarily unavailable. Refresh its status and retry.",
                ),
                storage_core::StoreError::Integrity(_) => operator_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTEGRITY_FAILURE",
                    "Resource content could not be verified",
                ),
                _ => operator_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL",
                    "Resource content could not be read",
                ),
            }
        })?
        .ok_or_else(|| {
            operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Resource content is unavailable",
            )
        })?;
    if query.revision_id.as_ref().is_some_and(|revision_id| revision_id != &content.summary.resource_revision_id) {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "RESOURCE_CONFLICT",
            "Resource revision changed since it was selected",
        ));
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, content.content.len().to_string())
        .header("x-content-type-options", "nosniff")
        .header("x-resource-media-type", content.summary.media_type)
        .header("cache-control", "no-store")
        .body(Body::from(content.content))
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Resource response could not be prepared",
            )
        })
}

fn selected_workspace(headers: &HeaderMap) -> Result<String, Response> {
    headers
        .get("x-workspace-id")
        .and_then(|value| value.to_str().ok())
        .filter(|id| !id.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "X-Workspace-ID is required",
            )
        })
}

fn sha256_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn idempotent_id(prefix: &str, principal_id: &str, request_id: &str, label: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"litecowork.idempotent-entity-id.v1");
    for value in [principal_id.as_bytes(), request_id.as_bytes(), label.as_bytes()] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value);
    }
    format!("{prefix}_{}", hex::encode(hasher.finalize()))
}

impl Drop for OperatorServer {
    fn drop(&mut self) {
        self.shutdown_server();
    }
}

async fn authenticate(request: Request<Body>, next: Next) -> Response {
    if request.extensions().get::<AuthenticatedLocalPeer>().is_some() {
        return next.run(request).await;
    }
    operator_error(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Operator authentication failed",
    )
}

/// Native-only local IPC operation. The path arrives only from a Tauri command after a
/// native folder picker; this route is not part of the public Operator/OpenAPI surface.
async fn create_workspace_root_from_native_selection(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WorkspaceRootCreatedResponse>, Response> {
    const MAX_SELECTION_BODY_BYTES: usize = 64 * 1024;
    if body.len() > MAX_SELECTION_BODY_BYTES {
        return Err(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Folder selection request exceeds its size limit",
        ));
    }
    let request: NativeWorkspaceRootSelectionRequest = serde_json::from_slice(&body).map_err(|_| {
        operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Folder selection request is invalid",
        )
    })?;
    let request_id = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty() && value.len() <= 128 && value.bytes().all(|byte| byte.is_ascii_graphic())
        })
        .ok_or_else(|| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "A valid Idempotency-Key is required"))?;
    if request.workspace_id.trim().is_empty()
        || !matches!(request.watch_policy.as_str(), "METADATA" | "CONTENT_DIGESTS" | "SELECTED_TEXT_EXTRACTION")
        || !matches!(request.replication_policy.as_str(), "NONE" | "ACTIVE_TASKS" | "SELECTED_WORKSPACE_POLICY")
        || request.selected_path_base64.is_empty()
        || request.selected_path_base64.len() > 44 * 1024
    {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Folder selection settings are invalid",
        ));
    }
    let workspace = state.store.get_workspace(&request.workspace_id).map_err(|_| {
        operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Workspace is unavailable")
    })?;
    let Some(workspace) = workspace.filter(|workspace| {
        workspace.owner_principal_id == state.principal_id && workspace.status == "ACTIVE"
    }) else {
        return Err(operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Workspace is unavailable"));
    };
    if workspace.version != request.expected_workspace_version {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "STALE_WORKSPACE_VERSION",
            "Workspace changed; refresh it before adding a folder",
        ));
    }

    #[cfg(unix)]
    let selected_path = {
        use std::os::unix::ffi::OsStringExt;
        let raw_path = URL_SAFE_NO_PAD.decode(request.selected_path_base64.as_bytes()).map_err(|_| {
            operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Selected folder is invalid")
        })?;
        if raw_path.is_empty() || raw_path.len() > 32 * 1024 || raw_path.contains(&0) {
            return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Selected folder is invalid"));
        }
        PathBuf::from(OsString::from_vec(raw_path))
    };
    #[cfg(not(unix))]
    let selected_path: PathBuf = {
        return Err(operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "UNAVAILABLE",
            "Persistent folder access is not qualified on this platform",
        ));
    };

    let opened = open_selected_directory(&selected_path).map_err(|_| {
        operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Selected folder could not be opened safely")
    })?;
    opened.revalidate().map_err(|_| {
        operator_error(StatusCode::CONFLICT, "RESOURCE_IDENTITY_CHANGED", "Selected folder changed before it could be added")
    })?;
    let identity = opened.identity();
    let (identity_digest, file_identity) = identity.keyed_projection(&state.runtime_identity);
    let correlation_id = idempotent_id("cor", &state.principal_id, request_id, "correlation");
    let mut context = event_context(&state.runtime_id, &correlation_id).map_err(|_| {
        operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Folder request could not be initialized")
    })?;
    context.event_id = idempotent_id("ev", &state.principal_id, request_id, "event");

    let resource_id = idempotent_id("res", &state.principal_id, request_id, "resource");
    let location_id = idempotent_id("loc", &state.principal_id, request_id, "location");
    let locator_ref_id = idempotent_id("lr", &state.principal_id, request_id, "locator");
    let workspace_root_id = idempotent_id("wroot", &state.principal_id, request_id, "workspace-root");
    let file_identity_binding = identity.binding_record(
        location_id.clone(),
        state.runtime_id.clone(),
        state.local_incarnation_id.clone(),
        context.recorded_at.clone(),
    );
    let committed = WorkspaceRootService::new(state.store.clone())
        .add_root(AddWorkspaceRoot {
            workspace_id: request.workspace_id,
            expected_workspace_version: request.expected_workspace_version,
            principal_id: state.principal_id,
            request_id: request_id.to_owned(),
            resource_id,
            identity_digest,
            file_identity,
            location_id,
            locator_ref_id,
            workspace_root_id,
            runtime_id: state.runtime_id,
            runtime_incarnation_id: state.local_incarnation_id,
            private_locator: opened.private_locator().to_owned(),
            file_identity_binding,
            display_name: opened.display_name().to_owned(),
            watch_policy: request.watch_policy,
            replication_policy: request.replication_policy,
            event: context,
        })
        .map_err(|error| match error {
            StoreError::Conflict { .. } => operator_error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Folder root conflicts with current Workspace state",
            ),
            StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Workspace is unavailable"),
            StoreError::Invalid(_) => operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Folder root request is invalid"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Folder root could not be saved"),
    })?;
    Ok(Json(WorkspaceRootCreatedResponse {
        workspace_id: committed.root.workspace_id,
        workspace_root_id: committed.root.workspace_root_id,
        resource_id: committed.resource.resource_id,
        display_name: committed.root.display_name,
        watch_policy: committed.root.watch_policy,
        replication_policy: committed.root.replication_policy,
        status: committed.root.status,
        location_availability: committed.location.availability,
        version: committed.root.version,
    }))
}

async fn list_workspaces(
    State(state): State<ApiState>,
) -> Result<Json<WorkspaceListResponse>, Response> {
    let workspaces = state.store.list_workspaces().map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace list is unavailable",
        )
    })?;
    Ok(Json(WorkspaceListResponse {
        items: workspaces
            .into_iter()
            .filter(|workspace| workspace.owner_principal_id == state.principal_id)
            .collect(),
    }))
}

async fn list_agent_installations() -> Result<Json<AgentInstallationListResponse>, Response> {
    let items = tokio::task::spawn_blocking(discover_local_installations)
        .await
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Agent inventory is unavailable",
            )
        })?;
    Ok(Json(AgentInstallationListResponse { items }))
}

fn agent_profile_response(profile: AgentProfileViewRecord) -> AgentProfileResponse {
    let mut endpoints = Vec::with_capacity(profile.endpoints.len());
    let mut observations = Vec::new();
    for endpoint_view in profile.endpoints {
        let endpoint = endpoint_view.endpoint;
        for offer in endpoint_view.offers {
            observations.push(AgentProfileObservationResponse {
                endpoint_id: endpoint.endpoint_id.clone(),
                runtime_id: offer.runtime_id,
                runtime_incarnation_id: offer.runtime_incarnation_id,
                compatible: offer.compatible,
                readiness: offer.readiness,
                observed_at: offer.observed_at,
                offer_expires_at: offer.expires_at,
                constraints: offer.constraints,
            });
        }
        endpoints.push(AgentEndpointResponse {
            endpoint_id: endpoint.endpoint_id,
            agent_profile_id: endpoint.agent_profile_id,
            protocol: endpoint.protocol,
            topology: endpoint.topology,
            protocol_version: endpoint.protocol_version,
            capabilities: endpoint.capabilities,
        });
    }
    AgentProfileResponse {
        agent_profile_id: profile.profile.agent_profile_id,
        provider_key: profile.profile.provider_key,
        display_name: profile.profile.display_name,
        endpoints,
        discovered_at: profile.profile.discovered_at,
        observations,
    }
}

fn codex_endpoint_capabilities() -> serde_json::Value {
    json!({
        "protocol": "CODEX_APP_SERVER",
        "protocol_version": null,
        "session": {"resume":true,"steer":true,"interrupt":true,"cancel":false,"fork":false},
        "input": {"text":true,"image":false,"file":false,"resources":false},
        "extension": {"mcp_stdio":false,"mcp_http":false,"skills":false,"plugins":false,"dynamic_attach":false},
        "reporting": {"tool_calls":false,"plan":false,"usage":false,"native_subagents":false,"approvals":false},
        "environment": {"cwd":true,"extra_directories":false},
        "limits": {"max_context_hint":null,"max_concurrent_sessions":null}
    })
}

fn resolve_local_executable(name: &str) -> Option<std::path::PathBuf> {
    let search_path = std::env::var_os("PATH")?;
    std::env::split_paths(&search_path)
        .map(|directory| directory.join(name))
        .find_map(|candidate| {
            let canonical = std::fs::canonicalize(candidate).ok()?;
            canonical.is_file().then_some(canonical)
        })
}

fn codex_native_environment() -> crate::agents::NativeEnvironment {
    crate::agents::NativeEnvironment {
        home: std::env::var_os("HOME"),
        user_profile: std::env::var_os("USERPROFILE"),
        app_data: std::env::var_os("APPDATA"),
        local_app_data: std::env::var_os("LOCALAPPDATA"),
        codex_home: std::env::var_os("CODEX_HOME"),
        path: std::env::var_os("PATH"),
        system_root: std::env::var_os("SYSTEMROOT"),
        temporary_directory: Some(std::env::temp_dir().into_os_string()),
    }
}

fn operator_now_value() -> Result<String, ()> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| ())
}

fn operator_now() -> Result<String, Response> {
    operator_now_value().map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Current time is unavailable"))
}

fn add_seconds(timestamp: &str, seconds: i64) -> Result<String, Response> {
    let parsed = time::OffsetDateTime::parse(timestamp, &time::format_description::well_known::Rfc3339)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Observation expiry could not be calculated"))?;
    let expires = parsed.checked_add(time::Duration::seconds(seconds))
        .ok_or_else(|| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Observation expiry is out of range"))?;
    expires.format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Observation expiry could not be formatted"))
}

fn idempotency_key(headers: &HeaderMap) -> Result<String, Response> {
    headers.get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 200 && value.bytes().all(|byte| byte.is_ascii_graphic()))
        .map(str::to_owned)
        .ok_or_else(|| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "A valid Idempotency-Key is required"))
}

fn parse_if_match(headers: &HeaderMap) -> Result<u64, Response> {
    headers.get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().trim_matches('"').parse::<u64>())
        .transpose()
        .map_err(|_| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "If-Match must contain the current aggregate version"))?
        .ok_or_else(|| operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "If-Match is required"))
}

fn map_agent_store_error(error: storage_core::StoreError, message: &'static str) -> Response {
    match error {
        storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", message),
        storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "CONFLICT", message),
        storage_core::StoreError::Invalid(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", message),
        storage_core::StoreError::Integrity(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INTEGRITY_FAILURE", message),
        _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", message),
    }
}

async fn list_workspace_runtime_bindings(
    State(state): State<ApiState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<RuntimeWorkspaceBindingPageResponse>, Response> {
    let workspace = ensure_workspace_owner(&state, &workspace_id)?;
    let lookup = LocalRuntimeWorkspaceBindingLookup {
        owner_principal_id: state.principal_id.clone(),
        workspace_id: workspace_id.clone(),
        runtime_id: state.runtime_id.clone(),
        runtime_incarnation_id: state.local_incarnation_id.clone(),
    };
    let items = state.store.get_current_local_binding(lookup)
        .map_err(|error| map_agent_store_error(error, "Workspace Runtime enrollment is unavailable"))?
        .into_iter()
        .collect();
    Ok(Json(RuntimeWorkspaceBindingPageResponse { items, next_cursor: None }))
}

async fn enroll_local_runtime(
    State(state): State<ApiState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    ensure_workspace_owner(&state, &workspace_id)?;
    let expected_workspace_version = parse_if_match(&headers)?;
    let request_id = idempotency_key(&headers)?;
    let now = operator_now()?;
    let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Runtime enrollment could not be initialized"))?;
    let binding_id = new_id("rwb").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Runtime enrollment could not be initialized"))?;
    let request_payload = json!({
        "operation": "runtime.workspace.local_enrollment.v1",
        "workspace_id": workspace_id,
        "runtime_id": state.runtime_id,
        "runtime_incarnation_id": state.local_incarnation_id,
        "expected_workspace_version": expected_workspace_version,
    });
    let binding = state.store.enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
        request: WorkspaceCreateRequest { principal_id: state.principal_id.clone(), request_id, request_payload },
        expected_workspace_version,
        runtime_workspace_binding_id: binding_id,
        runtime_id: state.runtime_id.clone(),
        runtime_incarnation_id: state.local_incarnation_id.clone(),
        workspace_id,
        now,
        correlation_id,
    }).map_err(|error| map_agent_store_error(error, "Local Runtime could not be enrolled in this Workspace"))?;
    Ok((StatusCode::CREATED, Json(binding)).into_response())
}

async fn list_agent_profiles(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<AgentProfilePageResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let now = operator_now()?;
    let items = state
        .store
        .list_agent_profiles(&state.principal_id, &workspace_id, &now)
        .map_err(|error| map_agent_store_error(error, "Agent profiles are unavailable"))?
        .into_iter()
        .map(agent_profile_response)
        .collect();
    Ok(Json(AgentProfilePageResponse { items, next_cursor: None }))
}

async fn probe_local_agent_profile(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<ProbeLocalAgentProfileBody>,
) -> Result<Json<AgentProfileResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    if !matches!(body.provider_key.as_str(), "CODEX" | "OPENCODE") {
        return Err(operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "This coding-agent profile probe is unavailable for the selected provider"));
    }
    let now = operator_now()?;
    let runtime_binding = state.store.get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
        owner_principal_id: state.principal_id.clone(),
        workspace_id: workspace_id.clone(),
        runtime_id: state.runtime_id.clone(),
        runtime_incarnation_id: state.local_incarnation_id.clone(),
    }).map_err(|error| map_agent_store_error(error, "Workspace Runtime enrollment is unavailable"))?;
    if runtime_binding.is_none() {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Enroll this local Runtime in the Workspace before probing coding agents"));
    }
    if body.provider_key == "OPENCODE" {
        return probe_local_opencode_profile(state, workspace_id, now).await;
    }
    // The explicit owner action is the only code path that resolves and records this
    // private executable locator. It is admitted only for the current local Runtime.
    let executable = resolve_local_executable("codex").ok_or_else(|| {
        operator_error(StatusCode::NOT_FOUND, "AGENT_UNAVAILABLE", "Codex is not available on this local Runtime")
    })?;
    let cwd = std::env::current_dir().map_err(|_| operator_error(StatusCode::SERVICE_UNAVAILABLE, "AGENT_UNAVAILABLE", "The local Runtime working directory is unavailable"))?;
    let environment = codex_native_environment();
    let state_for_probe = state.clone();
    let scoped_workspace = workspace_id.clone();
    let runtime_id = state.runtime_id.clone();
    let incarnation_id = state.local_incarnation_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut now = now;
        let profile_id = "agent_profile_codex_cli".to_owned();
        let endpoint_id = "agent_endpoint_codex_app_server".to_owned();
        let profile = AgentProfileRecord {
            agent_profile_id: profile_id.clone(),
            provider_key: "CODEX".to_owned(),
            display_name: "Codex".to_owned(),
            discovered_at: now.clone(),
        };
        let endpoint = AgentEndpointRecord {
            endpoint_id: endpoint_id.clone(),
            agent_profile_id: profile_id.clone(),
            protocol: "API".to_owned(),
            topology: "PROCESS_ADAPTER".to_owned(),
            protocol_version: None,
            capabilities: codex_endpoint_capabilities(),
        };
        let existing = state_for_probe.store.list_agent_profiles(
            &state_for_probe.principal_id,
            &scoped_workspace,
            &now,
        ).map_err(|error| map_agent_store_error(error, "Agent profile could not be read"))?
            .into_iter()
            .find(|entry| entry.profile.agent_profile_id == profile_id);
        let stable_profile = existing.map_or(profile.clone(), |entry| entry.profile);
        state_for_probe.store.put_agent_profile(stable_profile, vec![endpoint]).map_err(
            |error| map_agent_store_error(error, "Codex profile identity could not be recorded"),
        )?;
        let locator = executable.to_string_lossy().into_owned();
        state_for_probe.store.register_local_endpoint_binding(LocalAgentEndpointBindingInput {
            endpoint_id: endpoint_id.clone(),
            runtime_id: runtime_id.clone(),
            runtime_incarnation_id: incarnation_id.clone(),
            endpoint_ref: locator,
            observed_at: now.clone(),
            expires_at: Some(add_seconds(&now, 300)?),
        }).map_err(|error| map_agent_store_error(error, "Codex endpoint could not be admitted on this Runtime"))?;

        let probe = crate::agents::probe_codex_profile(&executable, &cwd, environment);
        now = operator_now_value().map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Probe time could not be recorded"))?;
        let readiness = if !probe.host_process_stopped || probe.readiness == crate::agents::ProbeReadiness::Failed {
            "UNAVAILABLE"
        } else if probe.authentication == crate::agents::AuthenticationObservation::NeedsAuth {
            "NEEDS_AUTH"
        } else if probe.readiness == crate::agents::ProbeReadiness::Complete
            && probe.authentication == crate::agents::AuthenticationObservation::Configured
        {
            "STARTABLE"
        } else {
            "UNAVAILABLE"
        };
        let compatible = readiness == "STARTABLE";
        let constraints = serde_json::to_value(&probe).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Probe observations could not be normalized"))?;
        state_for_probe.store.publish_runtime_offer(RuntimeOfferRecord {
            runtime_id: runtime_id.clone(),
            runtime_incarnation_id: incarnation_id.clone(),
            offer_kind: "AGENT_ENDPOINT".to_owned(),
            offer_ref: endpoint_id,
            compatible,
            readiness: readiness.to_owned(),
            constraints,
            observed_at: now.clone(),
            expires_at: add_seconds(&now, 120)?,
        }).map_err(|error| map_agent_store_error(error, "Codex Runtime offer could not be recorded"))?;
        let profiles = state_for_probe.store.list_agent_profiles(
            &state_for_probe.principal_id,
            &scoped_workspace,
            &now,
        ).map_err(|error| map_agent_store_error(error, "Codex profile observation is unavailable"))?;
        profiles.into_iter()
            .find(|entry| entry.profile.agent_profile_id == profile_id)
            .map(agent_profile_response)
            .ok_or_else(|| operator_error(StatusCode::SERVICE_UNAVAILABLE, "AGENT_UNAVAILABLE", "The current Workspace has no eligible local Runtime offer"))
    }).await.map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Codex profile probe did not complete"))??;
    Ok(Json(result))
}

async fn probe_local_opencode_profile(
    state: ApiState,
    workspace_id: String,
    now: String,
) -> Result<Json<AgentProfileResponse>, Response> {
    // This explicit owner action is the only code path that resolves and records
    // the private OpenCode executable locator. Inventory alone never probes it.
    let executable = resolve_local_executable("opencode").ok_or_else(|| {
        operator_error(StatusCode::NOT_FOUND, "AGENT_UNAVAILABLE", "OpenCode is not available on this local Runtime")
    })?;
    let cwd = std::env::current_dir().map_err(|_| operator_error(StatusCode::SERVICE_UNAVAILABLE, "AGENT_UNAVAILABLE", "The local Runtime working directory is unavailable"))?;
    let environment = crate::agents::opencode_native_environment();
    let state_for_probe = state.clone();
    let scoped_workspace = workspace_id.clone();
    let runtime_id = state.runtime_id.clone();
    let incarnation_id = state.local_incarnation_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut observed_at = now;
        let profile_id = "agent_profile_opencode_server".to_owned();
        let endpoint_id = "agent_endpoint_opencode_server".to_owned();
        let profile = AgentProfileRecord {
            agent_profile_id: profile_id.clone(),
            provider_key: "OPENCODE".to_owned(),
            display_name: "OpenCode".to_owned(),
            discovered_at: observed_at.clone(),
        };
        let endpoint = AgentEndpointRecord {
            endpoint_id: endpoint_id.clone(),
            agent_profile_id: profile_id.clone(),
            protocol: "API".to_owned(),
            topology: "PROCESS_ADAPTER".to_owned(),
            protocol_version: None,
            capabilities: opencode_endpoint_capabilities(),
        };
        let existing = state_for_probe.store.list_agent_profiles(
            &state_for_probe.principal_id,
            &scoped_workspace,
            &observed_at,
        ).map_err(|error| map_agent_store_error(error, "Agent profile could not be read"))?
            .into_iter()
            .find(|entry| entry.profile.agent_profile_id == profile_id);
        let stable_profile = existing.map_or(profile, |entry| entry.profile);
        state_for_probe.store.put_agent_profile(stable_profile, vec![endpoint]).map_err(
            |error| map_agent_store_error(error, "OpenCode profile identity could not be recorded"),
        )?;
        let locator = executable.to_string_lossy().into_owned();
        state_for_probe.store.register_local_endpoint_binding(LocalAgentEndpointBindingInput {
            endpoint_id: endpoint_id.clone(),
            runtime_id: runtime_id.clone(),
            runtime_incarnation_id: incarnation_id.clone(),
            endpoint_ref: locator,
            observed_at: observed_at.clone(),
            expires_at: Some(add_seconds(&observed_at, 300)?),
        }).map_err(|error| map_agent_store_error(error, "OpenCode endpoint could not be admitted on this Runtime"))?;

        let probe = crate::agents::probe_opencode_profile(&executable, &cwd, environment);
        observed_at = operator_now_value().map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Probe time could not be recorded"))?;
        let readiness = if probe.host_process_stopped
            && probe.probe_readiness == crate::agents::OpenCodeProbeReadiness::Complete
        {
            // Catalog observation is useful for display, but no compatible
            // session/model-selection semantics have been qualified. This
            // deliberately cannot admit or enable an AgentBinding.
            "DEGRADED"
        } else {
            "UNAVAILABLE"
        };
        let constraints = serde_json::to_value(&probe).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Probe observations could not be normalized"))?;
        state_for_probe.store.publish_runtime_offer(RuntimeOfferRecord {
            runtime_id: runtime_id.clone(),
            runtime_incarnation_id: incarnation_id.clone(),
            offer_kind: "AGENT_ENDPOINT".to_owned(),
            offer_ref: endpoint_id,
            compatible: false,
            readiness: readiness.to_owned(),
            constraints,
            observed_at: observed_at.clone(),
            expires_at: add_seconds(&observed_at, 120)?,
        }).map_err(|error| map_agent_store_error(error, "OpenCode Runtime offer could not be recorded"))?;
        let profiles = state_for_probe.store.list_agent_profiles(
            &state_for_probe.principal_id,
            &scoped_workspace,
            &observed_at,
        ).map_err(|error| map_agent_store_error(error, "OpenCode profile observation is unavailable"))?;
        profiles.into_iter()
            .find(|entry| entry.profile.agent_profile_id == profile_id)
            .map(agent_profile_response)
            .map(Json)
            .ok_or_else(|| operator_error(StatusCode::SERVICE_UNAVAILABLE, "AGENT_UNAVAILABLE", "The current Workspace has no eligible local Runtime offer"))
    }).await.map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "OpenCode profile probe did not complete"))??;
    Ok(result)
}

fn opencode_endpoint_capabilities() -> serde_json::Value {
    json!({
        "protocol": "OPENCODE_SERVER",
        "protocol_version": null,
        "session": {"resume":false,"steer":false,"interrupt":false,"cancel":false,"fork":false},
        "input": {"text":false,"image":false,"file":false,"resources":false},
        "extension": {"mcp_stdio":false,"mcp_http":false,"skills":false,"plugins":false,"dynamic_attach":false},
        "reporting": {"tool_calls":false,"plan":false,"usage":false,"native_subagents":false,"approvals":false},
        "environment": {"cwd":false,"extra_directories":false},
        "limits": {"max_context_hint":null,"max_concurrent_sessions":null}
    })
}

async fn list_agent_bindings(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<AgentBindingPageResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let items = state.store.list_agent_bindings(&state.principal_id, &workspace_id)
        .map_err(|error| map_agent_store_error(error, "Workspace agent bindings are unavailable"))?;
    Ok(Json(AgentBindingPageResponse { items, next_cursor: None }))
}

async fn get_agent_binding(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(agent_binding_id): Path<String>,
) -> Result<Json<AgentBindingRecord>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let binding = state.store.get_agent_binding(&state.principal_id, &workspace_id, &agent_binding_id)
        .map_err(|error| map_agent_store_error(error, "Workspace agent binding is unavailable"))?
        .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Workspace agent binding is unavailable"))?;
    Ok(Json(binding))
}

async fn create_agent_binding(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateAgentBindingBody>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if body.workspace_id != workspace_id {
        return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "Agent binding Workspace does not match the selected Workspace"));
    }
    ensure_workspace_owner(&state, &workspace_id)?;
    let request_id = idempotency_key(&headers)?;
    let now = operator_now()?;
    let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Agent binding request could not be initialized"))?;
    let binding_id = new_id("ab").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Agent binding request could not be initialized"))?;
    let payload = json!({
        "operation": "agent.binding.create.v1",
        "workspace_id": workspace_id,
        "agent_profile_id": body.agent_profile_id,
        "runtime_id": body.runtime_id,
        "lead_eligible": body.lead_eligible,
        "endpoint_selection_policy": body.endpoint_selection_policy,
        "auth_ref": body.auth_ref,
        "configuration": body.configuration,
    });
    let policy = body.endpoint_selection_policy.unwrap_or_else(|| json!({"mode":"AUTO_COMPATIBLE","required_features":[],"preferred_topologies":[]}));
    let configuration = body.configuration.unwrap_or_else(|| json!({}));
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Agent binding event could not be initialized"))?;
    let binding = AgentBindingRecord {
        agent_binding_id: binding_id.clone(), workspace_id: workspace_id.clone(),
        agent_profile_id: body.agent_profile_id, runtime_id: body.runtime_id,
        endpoint_selection_policy: policy, auth_ref: body.auth_ref, configuration,
        enabled: false, lead_eligible: body.lead_eligible, created_at: now.clone(), version: 1,
    };
    let mut created_payload = json!({
        "agent_binding_id": binding.agent_binding_id,
        "workspace_id": binding.workspace_id,
        "agent_profile_id": binding.agent_profile_id,
        "from_enabled": false,
        "to_enabled": false,
        "aggregate_version": binding.version,
        "requested_by": {"principal_id": state.principal_id, "kind":"USER"},
    });
    if let Some(runtime_id) = &binding.runtime_id {
        created_payload["runtime_id"] = json!(runtime_id);
    }
    let event = storage_core::EventDraft {
        event_id: context.event_id, workspace_id: workspace_id.clone(), entity_type: "AgentBinding".to_owned(),
        entity_id: binding_id, origin_runtime_id: context.origin_runtime_id, entity_revision: 1,
        hlc_timestamp: context.hlc_timestamp, correlation_id: context.correlation_id,
        causation_id: None, schema_version: 1, event_type: "agent.binding.created.v1".to_owned(),
        payload: created_payload,
        recorded_at: context.recorded_at,
    };
    let committed = state.store.create_agent_binding(AgentBindingCreateRequest {
        request: WorkspaceCreateRequest { principal_id: state.principal_id.clone(), request_id, request_payload: payload },
        binding, now, event,
    }).map_err(|error| map_agent_store_error(error, "Agent binding could not be created"))?;
    Ok((StatusCode::CREATED, Json(committed.binding)).into_response())
}

async fn enable_agent_binding(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(agent_binding_id): Path<String>,
) -> Result<Json<AgentBindingRecord>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let expected_version = parse_if_match(&headers)?;
    let request_id = idempotency_key(&headers)?;
    let now = operator_now()?;
    let binding = state.store.get_agent_binding(&state.principal_id, &workspace_id, &agent_binding_id)
        .map_err(|error| map_agent_store_error(error, "Workspace agent binding is unavailable"))?
        .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Workspace agent binding is unavailable"))?;
    let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Agent binding event could not be initialized"))?;
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Agent binding event could not be initialized"))?;
    let request_payload = json!({"operation":"agent.binding.enable.v1", "workspace_id":workspace_id, "agent_binding_id":agent_binding_id, "expected_version":expected_version});
    let mut enabled_payload = json!({
        "agent_binding_id": agent_binding_id,
        "workspace_id": workspace_id,
        "agent_profile_id": binding.agent_profile_id,
        "from_enabled": false,
        "to_enabled": true,
        "aggregate_version": expected_version.saturating_add(1),
        "requested_by": {"principal_id": state.principal_id, "kind":"USER"},
    });
    if let Some(runtime_id) = &binding.runtime_id {
        enabled_payload["runtime_id"] = json!(runtime_id);
    }
    let event = storage_core::EventDraft {
        event_id: context.event_id, workspace_id: workspace_id.clone(), entity_type: "AgentBinding".to_owned(),
        entity_id: agent_binding_id.clone(), origin_runtime_id: context.origin_runtime_id,
        entity_revision: expected_version.saturating_add(1), hlc_timestamp: context.hlc_timestamp,
        correlation_id: context.correlation_id, causation_id: None, schema_version: 1,
        event_type: "agent.binding.changed.v1".to_owned(),
        payload: enabled_payload,
        recorded_at: context.recorded_at,
    };
    let committed = state.store.enable_agent_binding(AgentBindingEnableRequest {
        request: WorkspaceCreateRequest { principal_id: state.principal_id.clone(), request_id, request_payload },
        workspace_id, agent_binding_id, expected_version, now, event,
    }).map_err(|error| map_agent_store_error(error, "Agent binding could not be enabled"))?;
    Ok(Json(committed.binding))
}

async fn create_workspace(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateWorkspaceBody>,
) -> Result<Response, Response> {
    let request_id = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| byte.is_ascii_graphic())
        })
        .ok_or_else(|| {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "A valid Idempotency-Key is required",
            )
        })?;
    let normalized_name = body.name.trim().to_owned();
    if normalized_name.is_empty() || normalized_name.chars().count() > 160 {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Workspace name must contain 1 to 160 bytes",
        ));
    }
    let policy = body
        .replication_policy
        .unwrap_or(ReplicationPolicy::LocalOnly);
    if policy == ReplicationPolicy::SelectedFolders {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Selected-folder replication requires Workspace roots after creation",
        ));
    }
    let correlation_id = new_id("cor").map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace request could not be initialized",
        )
    })?;
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace request could not be initialized",
        )
    })?;
    let command = CreateWorkspace {
        workspace_id: new_id("ws").map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Workspace request could not be initialized",
            )
        })?,
        name: normalized_name.clone(),
        owner_principal_id: state.principal_id.clone(),
        event: context,
    };
    let service = WorkspaceService::new(state.store.clone());
    let committed = service
        .create_idempotent_with_policy(
            command,
            policy.clone(),
            request_id.to_owned(),
            json!({ "operation": "workspace.create.v1", "name": normalized_name, "replication_policy": policy.as_str() }),
        )
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Workspace request is invalid",
            ),
            storage_core::StoreError::Conflict { .. } => operator_error(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Idempotency key was already used for a different request",
            ),
            _ => operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Workspace could not be created",
            ),
        })?;
    let mut response = (StatusCode::CREATED, Json(committed.workspace)).into_response();
    let correlation_id = header::HeaderValue::from_bytes(committed.event.correlation_id.as_bytes())
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Workspace response could not be created",
            )
        })?;
    response.headers_mut().insert(
        header::HeaderName::from_static("x-correlation-id"),
        correlation_id,
    );
    Ok(response)
}

async fn get_workspace(
    State(state): State<ApiState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Workspace>, Response> {
    let selected = headers
        .get("x-workspace-id")
        .and_then(|value| value.to_str().ok());
    if selected != Some(workspace_id.as_str()) {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    let workspace = state.store.get_workspace(&workspace_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace is unavailable",
        )
    })?;
    match workspace.filter(|workspace| workspace.owner_principal_id == state.principal_id) {
        Some(workspace) => Ok(Json(workspace)),
        None => Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        )),
    }
}

async fn update_workspace_policy(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Json(body): Json<UpdateWorkspacePolicyBody>,
) -> Result<Response, Response> {
    if selected_workspace(&headers)? != workspace_id {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    let _workspace = state
        .store
        .get_workspace(&workspace_id)
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Workspace is unavailable",
            )
        })?
        .filter(|workspace| workspace.owner_principal_id == state.principal_id)
        .ok_or_else(|| {
            operator_error(
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Workspace access is unavailable",
            )
        })?;
    let expected_version = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().trim_matches('"').parse::<u64>())
        .transpose()
        .map_err(|_| {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "If-Match must contain a Workspace version",
            )
        })?
        .ok_or_else(|| {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "If-Match is required",
            )
        })?;
    let request_id = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 200
                && value.bytes().all(|byte| byte.is_ascii_graphic())
        })
        .ok_or_else(|| {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "A valid Idempotency-Key is required",
            )
        })?;
    let correlation_id = new_id("cor").map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace request could not be initialized",
        )
    })?;
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace request could not be initialized",
        )
    })?;
    let policy = body.replication_policy;
    let roots = body.replication_scope_root_ids;
    let request_payload = json!({
        "operation": "workspace.policy.update.v1",
        "replication_policy": policy.as_str(),
        "replication_scope_root_ids": roots.clone(),
    });
    let service = WorkspaceService::new(state.store.clone());
    let committed = service
        .change_replication_policy_idempotent(
            ChangeReplicationPolicy {
                workspace_id: workspace_id.clone(),
                expected_version,
                policy,
                replication_scope_root_ids: roots,
                event: context,
            },
            state.principal_id.clone(),
            request_id.to_owned(),
            request_payload,
        )
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Workspace policy is invalid",
            ),
            storage_core::StoreError::Conflict { .. } => operator_error(
                StatusCode::CONFLICT,
                "STALE_WORKSPACE_VERSION",
                "Workspace changed; refresh before saving",
            ),
            storage_core::StoreError::NotFound => operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Workspace is unavailable",
            ),
            _ => operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Workspace policy could not be saved",
            ),
        })?;
    let mut response = Json(committed.workspace).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn set_workspace_default_agent_binding(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Json(body): Json<SetWorkspaceDefaultAgentBindingBody>,
) -> Result<Response, Response> {
    if selected_workspace(&headers)? != workspace_id {
        return Err(operator_error(StatusCode::FORBIDDEN, "FORBIDDEN", "Workspace access is unavailable"));
    }
    ensure_workspace_owner(&state, &workspace_id)?;
    let expected_version = parse_if_match(&headers)?;
    let request_id = idempotency_key(&headers)?;
    if let Some(binding_id) = body.agent_binding_id.as_deref() {
        if binding_id.trim().is_empty() || binding_id.len() > 200 {
            return Err(operator_error(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", "AgentBinding selection is invalid"));
        }
        let binding = state.store.get_agent_binding(&state.principal_id, &workspace_id, binding_id)
            .map_err(|error| map_agent_store_error(error, "AgentBinding is unavailable"))?
            .ok_or_else(|| operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "AgentBinding is unavailable"))?;
        if !binding.enabled || !binding.lead_eligible {
            return Err(operator_error(StatusCode::UNPROCESSABLE_ENTITY, "AGENT_NOT_LEAD_ELIGIBLE", "Choose an enabled AgentBinding that is allowed to lead work"));
        }
    }
    let correlation_id = new_id("cor").map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Workspace request could not be initialized"))?;
    let context = event_context(&state.runtime_id, &correlation_id)
        .map_err(|_| operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Workspace request could not be initialized"))?;
    let request_payload = json!({
        "operation": "workspace.default_agent_binding.set.v1",
        "workspace_id": workspace_id,
        "agent_binding_id": body.agent_binding_id,
    });
    let committed = WorkspaceService::new(state.store.clone())
        .set_default_agent_binding_idempotent(SetWorkspaceDefaultAgentBinding {
            workspace_id: workspace_id.clone(),
            expected_version,
            agent_binding_id: body.agent_binding_id,
            principal_id: state.principal_id.clone(),
            request_id,
            request_payload,
            event: context,
        })
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_ARGUMENT", "Workspace default AgentBinding could not be selected"),
            storage_core::StoreError::Conflict { .. } => operator_error(StatusCode::CONFLICT, "STALE_WORKSPACE_VERSION", "Workspace changed; refresh before selecting a default"),
            storage_core::StoreError::NotFound => operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Workspace or AgentBinding is unavailable"),
            _ => operator_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Workspace default AgentBinding could not be saved"),
        })?;
    let mut response = Json(committed.workspace).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn create_workspace_instruction_revision(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Json(body): Json<CreateWorkspaceInstructionRevisionBody>,
) -> Result<Response, Response> {
    if selected_workspace(&headers)? != workspace_id {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    let workspace = state
        .store
        .get_workspace(&workspace_id)
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Workspace is unavailable",
            )
        })?
        .filter(|workspace| workspace.owner_principal_id == state.principal_id)
        .ok_or_else(|| {
            operator_error(
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Workspace access is unavailable",
            )
        })?;
    let expected_version = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().trim_matches('"').parse::<u64>())
        .transpose()
        .map_err(|_| {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "If-Match must contain a Workspace version",
            )
        })?
        .ok_or_else(|| {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "If-Match is required",
            )
        })?;
    let request_id = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 200
                && value.bytes().all(|byte| byte.is_ascii_graphic())
        })
        .ok_or_else(|| {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "A valid Idempotency-Key is required",
            )
        })?;
    let ref_workspace = body
        .content_ref
        .get("workspace_id")
        .and_then(serde_json::Value::as_str);
    let resource_id = body
        .content_ref
        .get("resource_id")
        .and_then(serde_json::Value::as_str);
    let revision_id = body
        .content_ref
        .get("revision_id")
        .and_then(serde_json::Value::as_str);
    let ref_has_only_expected_fields = body
        .content_ref
        .as_object()
        .is_some_and(|object| {
            object.len() == 3
                && object.contains_key("workspace_id")
                && object.contains_key("resource_id")
                && object.contains_key("revision_id")
        });
    if !ref_has_only_expected_fields
        || ref_workspace != Some(workspace_id.as_str())
        || resource_id.is_none_or(str::is_empty)
        || revision_id.is_none_or(str::is_empty)
    {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Instructions must reference a pinned Resource in this Workspace",
        ));
    }
    let resource_id = resource_id.unwrap_or_default();
    let content = state
        .store
        .read_resource_content_bounded(&workspace_id, resource_id, 64 * 1024)
        .map_err(|error| {
            if matches!(error, storage_core::StoreError::Invalid(ref message) if message == "RESOURCE_READ_LIMIT_EXCEEDED") {
                operator_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_ARGUMENT",
                    "Instructions must reference a Resource no larger than 64 KiB",
                )
            } else {
                operator_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTEGRITY_FAILURE",
                    "Instruction Resource could not be verified",
                )
            }
        })?
        .ok_or_else(|| {
            operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Instruction Resource is unavailable",
            )
        })?;
    if Some(content.summary.resource_revision_id.as_str()) != revision_id
        || content.summary.content_digest != body.content_digest
        || content.content.len() > 64 * 1024
        || !content.summary.media_type.starts_with("text/")
        || std::str::from_utf8(&content.content).is_err()
    {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Instructions must be a pinned UTF-8 text Resource no larger than 64 KiB",
        ));
    }
    if workspace.status != "ACTIVE" {
        return Err(operator_error(
            StatusCode::CONFLICT,
            "WORKSPACE_ARCHIVED",
            "Archived Workspaces cannot change instructions",
        ));
    }
    let correlation_id = new_id("cor").map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace instruction request could not be initialized",
        )
    })?;
    let context = event_context(&state.runtime_id, &correlation_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace instruction request could not be initialized",
        )
    })?;
    let request_payload = json!({
        "operation": "workspace.instructions.revision.create.v1",
        "workspace_id": workspace_id.clone(),
        "expected_version": expected_version,
        "parent_revisions": body.parent_revisions.clone(),
        "content_ref": body.content_ref.clone(),
        "content_digest": body.content_digest.clone(),
    });
    let service = WorkspaceService::new(state.store.clone());
    let committed = service
        .create_instruction_revision_idempotent(
            CreateWorkspaceInstructionRevision {
                workspace_id,
                expected_version,
                parent_revisions: body.parent_revisions,
                content_ref: body.content_ref,
                content_digest: body.content_digest,
                authored_by_principal_id: state.principal_id,
                event: context,
            },
            request_id.to_owned(),
            request_payload,
        )
        .map_err(|error| match error {
            storage_core::StoreError::Invalid(_) => operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Workspace instruction revision is invalid",
            ),
            storage_core::StoreError::Conflict { .. } => operator_error(
                StatusCode::CONFLICT,
                "STALE_WORKSPACE_VERSION",
                "Workspace changed; refresh before saving instructions",
            ),
            storage_core::StoreError::NotFound => operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Workspace instruction Resource is unavailable",
            ),
            _ => operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Workspace instructions could not be saved",
            ),
        })?;
    let mut response = (StatusCode::CREATED, Json(committed.instruction_revision)).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&committed.event.correlation_id) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    Ok(response)
}

async fn list_workspace_instruction_revisions(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Query(query): Query<InstructionHistoryQuery>,
) -> Result<Json<InstructionHistoryPage>, Response> {
    if selected_workspace(&headers)? != workspace_id {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    let workspace = state.store.get_workspace(&workspace_id).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace is unavailable",
        )
    })?;
    if !workspace.is_some_and(|workspace| workspace.owner_principal_id == state.principal_id) {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Workspace access is unavailable",
        ));
    }
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Instruction history limit must be between 1 and 200",
        ));
    }
    let after_revision = match query.cursor {
        None => 0,
        Some(cursor) => {
            if cursor.len() > 1024 {
                return Err(operator_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_ARGUMENT",
                    "Instruction history cursor is invalid",
                ));
            }
            let decoded = URL_SAFE_NO_PAD.decode(cursor.as_bytes()).map_err(|_| {
                operator_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_ARGUMENT",
                    "Instruction history cursor is invalid",
                )
            })?;
            let cursor: InstructionHistoryCursor = serde_json::from_slice(&decoded).map_err(|_| {
                operator_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_ARGUMENT",
                    "Instruction history cursor is invalid",
                )
            })?;
            if cursor.workspace_id != workspace_id || cursor.after_revision == 0 {
                return Err(operator_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_ARGUMENT",
                    "Instruction history cursor does not match this Workspace",
                ));
            }
            cursor.after_revision
        }
    };
    let mut revisions = state
        .store
        .list_workspace_instruction_revisions(&workspace_id, after_revision, limit + 1)
        .map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Workspace instruction history is unavailable",
        )
    })?;
    let has_more = revisions.len() > limit;
    revisions.truncate(limit);
    let next_cursor = if has_more {
        let after_revision = revisions.last().map(|revision| revision.revision).ok_or_else(|| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Instruction history cursor could not be created",
            )
        })?;
        let encoded = serde_json::to_vec(&InstructionHistoryCursor {
            workspace_id,
            after_revision,
        })
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Instruction history cursor could not be created",
            )
        })?;
        Some(URL_SAFE_NO_PAD.encode(encoded))
    } else {
        None
    };
    Ok(Json(InstructionHistoryPage {
        items: revisions,
        next_cursor,
    }))
}

fn operator_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    let correlation_id = new_id("cor").unwrap_or_else(|_| "cor_unavailable".to_owned());
    let retryable = status.is_server_error()
        || matches!(status, StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_MANY_REQUESTS);
    let mut response = (
        status,
        Json(OperatorError { code, message, retryable, correlation_id: correlation_id.clone(), details: None }),
    )
        .into_response();
    if let Ok(value) = header::HeaderValue::from_str(&correlation_id) {
        response.headers_mut().insert(header::HeaderName::from_static("x-correlation-id"), value);
    }
    response
}

fn new_id(prefix: &str) -> Result<String, ()> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| ())?;
    Ok(format!("{prefix}_{}", hex::encode(bytes)))
}

fn event_context(runtime_id: &str, correlation_id: &str) -> Result<EventContext, ()> {
    let now = time::OffsetDateTime::now_utc();
    let recorded_at = now
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| ())?;
    Ok(EventContext {
        event_id: new_id("ev")?,
        origin_runtime_id: runtime_id.to_owned(),
        // Existing event/schema fixtures encode this ordering hint as an RFC 3339
        // timestamp. The durable local HLC allocator is a separate lifecycle task.
        hlc_timestamp: recorded_at.clone(),
        correlation_id: correlation_id.to_owned(),
        causation_id: None,
        recorded_at,
    })
}

#[cfg(test)]
mod resource_content_search_tests {
    use super::{content_match_snippet, is_allowlisted_plain_text, MAX_CONTENT_SCAN_SNIPPET_CHARS};

    #[test]
    fn content_scan_accepts_allowlisted_utf8_text_and_rejects_containers() {
        assert!(is_allowlisted_plain_text("notes.md", "text/markdown"));
        assert!(is_allowlisted_plain_text("code.rs", "text/plain"));
        assert!(!is_allowlisted_plain_text("archive.zip", "application/zip"));
        assert!(!is_allowlisted_plain_text("report.pdf", "application/pdf"));
        assert!(!is_allowlisted_plain_text("unknown.bin", "application/octet-stream"));
    }

    #[test]
    fn content_scan_returns_bounded_case_insensitive_utf8_snippet() {
        let text = format!("{} LiteCowork keeps this result local. {}", "a".repeat(500), "b".repeat(500));
        let snippet = content_match_snippet(&text, "LITEcowork", MAX_CONTENT_SCAN_SNIPPET_CHARS).expect("matching snippet");
        assert!(snippet.to_ascii_lowercase().contains("litecowork"));
        assert!(snippet.chars().count() <= 320);
    }

    #[test]
    fn content_scan_rejects_binary_control_bytes() {
        assert!(content_match_snippet("one\ntwo", "two", MAX_CONTENT_SCAN_SNIPPET_CHARS).is_some());
        assert!(content_match_snippet("one\0two", "two", MAX_CONTENT_SCAN_SNIPPET_CHARS).is_none());
    }

    #[test]
    fn content_scan_rejects_misleading_text_extensions_for_known_rich_media() {
        assert!(!is_allowlisted_plain_text("report.txt", "application/pdf"));
        assert!(!is_allowlisted_plain_text("report.md", "application/vnd.openxmlformats-officedocument.wordprocessingml.document"));
        assert!(!is_allowlisted_plain_text("notes.txt", "image/png"));
        assert!(is_allowlisted_plain_text("notes.txt", "application/octet-stream"));
    }
}
