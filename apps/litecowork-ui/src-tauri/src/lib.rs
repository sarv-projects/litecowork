use serde::Serialize;
use base64::{
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use operator_ipc::{
    BudgetedBody, InFlightBodyBudget, LogicalHeader, RequestFrame, RequestHeader,
    DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, MAX_BODY_BYTES, PROTOCOL_VERSION,
};
use tauri::{AppHandle, Manager};

mod artifact_bridge;
mod conversation_bridge;
mod resource_save_bridge;
mod automation_bridge;
mod coworker_bridge;
mod delegation_profile_bridge;
mod delegation_profile_write_bridge;
mod goal_bridge;
mod suggestions_bridge;
mod presentation_bridge;
mod routine_bridge;
mod zip_intake_bridge;
#[cfg(target_os = "linux")]
mod systemd_user_service;

#[cfg(unix)]
use operator_ipc::unix::{PeerCredentials, UnixEndpoint};
#[cfg(unix)]
use std::sync::{
    Arc,
    OnceLock,
    atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
static OPERATOR_REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);
#[cfg(unix)]
static OPERATOR_IPC_RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
#[cfg(unix)]
static OPERATOR_IPC_BODY_BUDGET: OnceLock<Arc<InFlightBodyBudget>> = OnceLock::new();

const RUNTIME_STARTUP_DEADLINE: Duration = Duration::from_secs(5);
const RUNTIME_STATUS_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const STATUS_COMMAND_PROBE_TIMEOUT: Duration = Duration::from_millis(400);
const OPERATOR_READINESS_PROBE_TIMEOUT: Duration = Duration::from_millis(400);
const MAX_RUNTIME_STATUS_BYTES: usize = 1024 * 1024;
const MAX_TASK_INPUT_REFS: usize = 256;
const MAX_INSTRUCTION_PARENTS: usize = 256;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    state: String,
    runtime_id: Option<String>,
    local_incarnation_id: Option<String>,
    blockers: Vec<String>,
    last_shutdown_clean: Option<bool>,
    process_running: bool,
    operator_ready: bool,
    daemon_available: bool,
    detail: Option<String>,
}

#[tauri::command]
async fn get_runtime_status(app: AppHandle) -> Result<RuntimeStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let deadline = Instant::now() + RUNTIME_STATUS_PROBE_TIMEOUT;
        let mut status = runtime_status_until(&app, deadline);
        #[cfg(target_os = "linux")]
        if let Err(error) = app
            .path()
            .config_dir()
            .map_err(|_| "LiteCowork user configuration directory is unavailable".to_owned())
            .and_then(|config_dir| {
                let state_dir = app.path().app_data_dir()
                    .map_err(|_| "LiteCowork application data directory is unavailable".to_owned())?;
                let daemon = daemon_executable(&app)?;
                systemd_user_service::verify_active(&config_dir, &daemon, &state_dir, deadline)
            })
        {
            // Keep API readiness separate from execution-service readiness. The
            // authenticated Operator API may answer while Linux's delegated unit
            // contract is unverified; do not present that as a fully ready Runtime.
            status.state = "DEGRADED".to_owned();
            if !status.blockers.iter().any(|blocker| blocker == "LINUX_SYSTEMD_SERVICE_UNVERIFIED") {
                status.blockers.push("LINUX_SYSTEMD_SERVICE_UNVERIFIED".to_owned());
            }
            let api_detail = status.detail.take();
            status.detail = Some(match api_detail {
                Some(detail) => format!(
                    "{detail}; {error}. Linux delegated containment is unavailable"
                ),
                None => format!(
                    "{error}. Linux delegated containment is unavailable"
                ),
            });
        }
        status
    })
        .await
        .map_err(|_| "Runtime status operation did not complete".to_owned())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceView {
    workspace_id: String,
    name: String,
    replication_policy: String,
    default_agent_binding_id: Option<String>,
    primary_coworker_id: Option<String>,
    status: String,
    version: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRootView {
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

#[derive(serde::Deserialize)]
struct WorkspaceRootWire {
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

impl From<WorkspaceRootWire> for WorkspaceRootView {
    fn from(root: WorkspaceRootWire) -> Self {
        Self {
            workspace_id: root.workspace_id,
            workspace_root_id: root.workspace_root_id,
            resource_id: root.resource_id,
            display_name: root.display_name,
            watch_policy: root.watch_policy,
            replication_policy: root.replication_policy,
            status: root.status,
            location_availability: root.location_availability,
            version: root.version,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRootPageView {
    items: Vec<WorkspaceRootView>,
    next_cursor: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRootPageWire {
    items: Vec<WorkspaceRootWire>,
    next_cursor: Option<String>,
}

#[derive(serde::Deserialize)]
struct WorkspaceListWire {
    items: Vec<WorkspaceWire>,
}

#[derive(serde::Deserialize)]
struct WorkspaceWire {
    workspace_id: String,
    name: String,
    replication_policy: String,
    default_agent_binding_id: Option<String>,
    primary_coworker_id: Option<String>,
    status: String,
    version: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInstallationView {
    agent_id: String,
    display_name: String,
    protocol_candidate: String,
    installation: String,
    version: Option<String>,
    authentication: String,
    session_readiness: String,
}

#[derive(serde::Deserialize)]
struct AgentInstallationListWire {
    items: Vec<AgentInstallationWire>,
}

#[derive(serde::Deserialize)]
struct AgentInstallationWire {
    agent_id: String,
    display_name: String,
    protocol_candidate: String,
    installation: String,
    version: Option<String>,
    authentication: String,
    session_readiness: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentEndpointView {
    endpoint_id: String,
    agent_profile_id: String,
    protocol: String,
    topology: String,
    protocol_version: Option<String>,
    capabilities: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentProfileObservationView {
    endpoint_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
    compatible: bool,
    readiness: String,
    observed_at: String,
    offer_expires_at: String,
    constraints: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentProfileView {
    agent_profile_id: String,
    provider_key: String,
    display_name: String,
    endpoints: Vec<AgentEndpointView>,
    discovered_at: String,
    observations: Vec<AgentProfileObservationView>,
}

#[derive(serde::Deserialize)]
struct AgentEndpointWire {
    endpoint_id: String,
    agent_profile_id: String,
    protocol: String,
    topology: String,
    protocol_version: Option<String>,
    capabilities: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct AgentProfileObservationWire {
    endpoint_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
    compatible: bool,
    readiness: String,
    observed_at: String,
    offer_expires_at: String,
    constraints: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct AgentProfileWire {
    agent_profile_id: String,
    provider_key: String,
    display_name: String,
    endpoints: Vec<AgentEndpointWire>,
    discovered_at: String,
    observations: Vec<AgentProfileObservationWire>,
}

#[derive(serde::Deserialize)]
struct AgentProfilePageWire {
    items: Vec<AgentProfileWire>,
    next_cursor: Option<String>,
}

impl From<AgentProfileWire> for AgentProfileView {
    fn from(profile: AgentProfileWire) -> Self {
        Self {
            agent_profile_id: profile.agent_profile_id,
            provider_key: profile.provider_key,
            display_name: profile.display_name,
            endpoints: profile.endpoints.into_iter().map(|endpoint| AgentEndpointView {
                endpoint_id: endpoint.endpoint_id,
                agent_profile_id: endpoint.agent_profile_id,
                protocol: endpoint.protocol,
                topology: endpoint.topology,
                protocol_version: endpoint.protocol_version,
                capabilities: endpoint.capabilities,
            }).collect(),
            discovered_at: profile.discovered_at,
            observations: profile.observations.into_iter().map(|observation| AgentProfileObservationView {
                endpoint_id: observation.endpoint_id,
                runtime_id: observation.runtime_id,
                runtime_incarnation_id: observation.runtime_incarnation_id,
                compatible: observation.compatible,
                readiness: observation.readiness,
                observed_at: observation.observed_at,
                offer_expires_at: observation.offer_expires_at,
                constraints: observation.constraints,
            }).collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentProfilePageView {
    items: Vec<AgentProfileView>,
    next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentBindingView {
    agent_binding_id: String,
    workspace_id: String,
    agent_profile_id: String,
    runtime_id: Option<String>,
    endpoint_selection_policy: serde_json::Value,
    auth_ref: Option<serde_json::Value>,
    configuration: serde_json::Value,
    enabled: bool,
    lead_eligible: bool,
    created_at: String,
    version: u64,
}

#[derive(serde::Deserialize)]
struct AgentBindingWire {
    agent_binding_id: String,
    workspace_id: String,
    agent_profile_id: String,
    runtime_id: Option<String>,
    endpoint_selection_policy: serde_json::Value,
    auth_ref: Option<serde_json::Value>,
    configuration: serde_json::Value,
    enabled: bool,
    lead_eligible: bool,
    created_at: String,
    version: u64,
}

#[derive(serde::Deserialize)]
struct AgentBindingPageWire {
    items: Vec<AgentBindingWire>,
    next_cursor: Option<String>,
}

impl From<AgentBindingWire> for AgentBindingView {
    fn from(binding: AgentBindingWire) -> Self {
        Self {
            agent_binding_id: binding.agent_binding_id,
            workspace_id: binding.workspace_id,
            agent_profile_id: binding.agent_profile_id,
            runtime_id: binding.runtime_id,
            endpoint_selection_policy: binding.endpoint_selection_policy,
            auth_ref: binding.auth_ref,
            configuration: binding.configuration,
            enabled: binding.enabled,
            lead_eligible: binding.lead_eligible,
            created_at: binding.created_at,
            version: binding.version,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentBindingPageView {
    items: Vec<AgentBindingView>,
    next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeWorkspaceBindingView {
    runtime_workspace_binding_id: String,
    runtime_id: String,
    workspace_id: String,
    enrollment_mode: String,
    status: String,
    roles: Vec<String>,
    created_at: String,
    activated_at: Option<String>,
    revoked_at: Option<String>,
    version: u64,
}

#[derive(serde::Deserialize)]
struct RuntimeWorkspaceBindingWire {
    runtime_workspace_binding_id: String,
    runtime_id: String,
    workspace_id: String,
    enrollment_mode: String,
    status: String,
    roles: Vec<String>,
    created_at: String,
    activated_at: Option<String>,
    revoked_at: Option<String>,
    version: u64,
}

#[derive(serde::Deserialize)]
struct RuntimeWorkspaceBindingPageWire {
    items: Vec<RuntimeWorkspaceBindingWire>,
    next_cursor: Option<String>,
}

impl From<RuntimeWorkspaceBindingWire> for RuntimeWorkspaceBindingView {
    fn from(binding: RuntimeWorkspaceBindingWire) -> Self {
        Self {
            runtime_workspace_binding_id: binding.runtime_workspace_binding_id,
            runtime_id: binding.runtime_id,
            workspace_id: binding.workspace_id,
            enrollment_mode: binding.enrollment_mode,
            status: binding.status,
            roles: binding.roles,
            created_at: binding.created_at,
            activated_at: binding.activated_at,
            revoked_at: binding.revoked_at,
            version: binding.version,
        }
    }
}

impl From<AgentInstallationWire> for AgentInstallationView {
    fn from(agent: AgentInstallationWire) -> Self {
        Self {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
            protocol_candidate: agent.protocol_candidate,
            installation: agent.installation,
            version: agent.version,
            authentication: agent.authentication,
            session_readiness: agent.session_readiness,
        }
    }
}

impl From<WorkspaceWire> for WorkspaceView {
    fn from(workspace: WorkspaceWire) -> Self {
        Self {
            workspace_id: workspace.workspace_id,
            name: workspace.name,
            replication_policy: workspace.replication_policy,
            default_agent_binding_id: workspace.default_agent_binding_id,
            primary_coworker_id: workspace.primary_coworker_id,
            status: workspace.status,
            version: workspace.version,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskSummaryView {
    task_id: String,
    status: String,
    objective: String,
    created_at: String,
    updated_at: String,
}

#[derive(serde::Deserialize)]
struct TaskSummaryWire {
    task_id: String,
    status: String,
    objective: String,
    created_at: String,
    updated_at: String,
}

#[derive(serde::Deserialize)]
struct TaskPageWire {
    items: Vec<TaskSummaryWire>,
    next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskPageView {
    items: Vec<TaskSummaryView>,
    next_cursor: Option<String>,
}

impl From<TaskSummaryWire> for TaskSummaryView {
    fn from(task: TaskSummaryWire) -> Self {
        Self {
            task_id: task.task_id,
            status: task.status,
            objective: task.objective,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

#[derive(serde::Deserialize)]
struct TaskViewWire {
    task: TaskDetailRecordWire,
    current_spec_revision: TaskSpecDetailWire,
}

#[derive(serde::Deserialize)]
struct TaskDetailRecordWire {
    task_id: String,
    workspace_id: String,
    origin_coworker_id: Option<String>,
    origin_coworker_revision: Option<u64>,
    current_spec_revision: u64,
    current_plan_revision: Option<u64>,
    status: String,
    version: u64,
    created_at: String,
    updated_at: String,
}

#[derive(serde::Deserialize)]
struct TaskSpecDetailWire {
    task_id: String,
    revision: u64,
    objective: String,
    input_refs: Vec<PinnedResourceRefWire>,
}

#[derive(serde::Deserialize)]
struct TaskSpecHistoryRevisionWire {
    task_id: String,
    // Workspace ownership is established by the selected-workspace Operator route;
    // this extra storage field is optional because the public schema omits it.
    workspace_id: Option<String>,
    revision: u64,
    parent_revisions: Vec<u64>,
    objective: String,
    authored_by: TaskSpecHistoryPrincipalWire,
    created_at: String,
}

#[derive(serde::Deserialize)]
struct TaskSpecHistoryPrincipalWire {
    principal_id: String,
    kind: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskSpecHistoryRevisionView {
    task_id: String,
    workspace_id: String,
    revision: u64,
    parent_revisions: Vec<u64>,
    objective: String,
    authored_by: TaskSpecHistoryPrincipalView,
    created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskSpecHistoryPrincipalView {
    principal_id: String,
    kind: String,
}

impl TaskSpecHistoryRevisionView {
    fn from_wire(revision: TaskSpecHistoryRevisionWire, workspace_id: String) -> Self {
        Self {
            task_id: revision.task_id,
            workspace_id,
            revision: revision.revision,
            parent_revisions: revision.parent_revisions,
            objective: revision.objective,
            authored_by: TaskSpecHistoryPrincipalView {
                principal_id: revision.authored_by.principal_id,
                kind: revision.authored_by.kind,
            },
            created_at: revision.created_at,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskSpecRevisionReceiptView {
    task_id: String,
    revision: u64,
    objective: String,
}

#[derive(serde::Deserialize)]
struct TaskPlanRevisionWire {
    task_id: String,
    revision: u64,
    task_spec_revision: u64,
    steps: Vec<PlannedStepWire>,
}

#[derive(serde::Deserialize)]
struct PlannedStepWire {
    logical_key: String,
    title: String,
    objective: String,
}

#[derive(serde::Deserialize)]
struct TaskStepWire {
    step_id: String,
    task_id: String,
    plan_revision: u64,
    logical_key: Option<String>,
    title: String,
    objective: String,
    status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlannedStepView {
    step_id: String,
    logical_key: String,
    title: String,
    objective: String,
    status: String,
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct PinnedResourceRefWire {
    workspace_id: String,
    resource_id: String,
    revision_id: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PinnedResourceRefInput {
    workspace_id: String,
    resource_id: String,
    revision_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PinnedResourceRefView {
    workspace_id: String,
    resource_id: String,
    revision_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskDetailView {
    task_id: String,
    workspace_id: String,
    origin_coworker_id: Option<String>,
    origin_coworker_revision: Option<u64>,
    current_spec_revision: u64,
    current_plan_revision: Option<u64>,
    status: String,
    task_version: u64,
    objective: String,
    input_refs: Vec<PinnedResourceRefView>,
    plan_spec_revision: Option<u64>,
    plan_is_stale: bool,
    planned_steps: Vec<PlannedStepView>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum TaskPlanningBlocker {
    TaskStateNotEligible,
    PlanAlreadyAccepted,
    CurrentLeadOrEndpointUnavailable,
    TaskIsolationUnavailable,
    NativeCapabilitiesUnmediated,
    ProtocolUnqualified,
    ProcessContainmentUnqualified,
    PlanningContextResourceUnavailable,
    SessionSettlementUnavailable,
    ProviderUnsupported,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskPlanningReadinessWire {
    task_id: String,
    task_version: u64,
    task_spec_revision: u64,
    task_status: String,
    observed_at: String,
    dispatch_available: bool,
    planning_started: bool,
    agent_session_started: bool,
    plan_created: bool,
    blockers: Vec<TaskPlanningBlocker>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskPlanningReadinessView {
    task_id: String,
    task_version: u64,
    task_spec_revision: u64,
    task_status: String,
    observed_at: String,
    dispatch_available: bool,
    planning_started: bool,
    agent_session_started: bool,
    plan_created: bool,
    blockers: Vec<TaskPlanningBlocker>,
}

impl TryFrom<TaskPlanningReadinessWire> for TaskPlanningReadinessView {
    type Error = String;

    fn try_from(value: TaskPlanningReadinessWire) -> Result<Self, Self::Error> {
        let valid_status = matches!(value.task_status.as_str(),
            "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING"
                | "NEEDS_USER" | "INCOMPLETE" | "PAUSE_REQUESTED" | "PAUSED"
                | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED");
        let known_timestamp = value.observed_at.len() <= 128
            && value.observed_at.contains('T')
            && (value.observed_at.ends_with('Z') || value.observed_at.contains('+'));
        if value.task_id.is_empty()
            || value.task_id.len() > 200
            || value.task_version == 0
            || value.task_spec_revision == 0
            || !valid_status
            || !known_timestamp
            || value.dispatch_available
            || value.planning_started
            || value.agent_session_started
            || value.plan_created
            || value.blockers.len() > 10
        {
            return Err("Local Runtime returned an unsupported planning readiness response".to_owned());
        }
        let mut seen = std::collections::HashSet::new();
        if value.blockers.iter().any(|blocker| !seen.insert(*blocker)) {
            return Err("Local Runtime returned duplicate planning readiness blockers".to_owned());
        }
        Ok(Self {
            task_id: value.task_id,
            task_version: value.task_version,
            task_spec_revision: value.task_spec_revision,
            task_status: value.task_status,
            observed_at: value.observed_at,
            dispatch_available: false,
            planning_started: false,
            agent_session_started: false,
            plan_created: false,
            blockers: value.blockers,
        })
    }
}

fn validate_task_planning_readiness_identity(
    view: TaskPlanningReadinessView,
    expected_task_id: &str,
    expected_task_version: u64,
) -> Result<TaskPlanningReadinessView, String> {
    if view.task_id != expected_task_id || view.task_version != expected_task_version {
        return Err("Task changed; reload the latest Task before checking planning readiness".to_owned());
    }
    Ok(view)
}

impl From<TaskViewWire> for TaskDetailView {
    fn from(view: TaskViewWire) -> Self {
        Self {
            task_id: view.task.task_id,
            workspace_id: view.task.workspace_id,
            origin_coworker_id: view.task.origin_coworker_id,
            origin_coworker_revision: view.task.origin_coworker_revision,
            current_spec_revision: view.task.current_spec_revision,
            current_plan_revision: view.task.current_plan_revision,
            status: view.task.status,
            task_version: view.task.version,
            objective: view.current_spec_revision.objective,
            input_refs: view.current_spec_revision.input_refs.into_iter().map(|input| PinnedResourceRefView {
                workspace_id: input.workspace_id,
                resource_id: input.resource_id,
                revision_id: input.revision_id,
            }).collect(),
            plan_spec_revision: None,
            plan_is_stale: false,
            planned_steps: Vec::new(),
            created_at: view.task.created_at,
            updated_at: view.task.updated_at,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
struct OperatorReadinessWire {
    operator_state: String,
    runtime_id: String,
    local_incarnation_id: String,
    api_contract_version: u32,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceView {
    resource_id: String,
    workspace_id: String,
    resource_revision_id: String,
    display_name: String,
    media_type: String,
    content_digest: String,
    size_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceDetailView {
    resource_id: String,
    workspace_id: String,
    kind: String,
    display_name: String,
    current_revision_id: Option<String>,
    version: u64,
    context_document: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct ResourceDetailWire {
    resource_id: String,
    workspace_id: String,
    kind: String,
    display_name: String,
    current_revision_id: Option<String>,
    version: u64,
    context_document: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceRevisionView {
    resource_revision_id: String,
    resource_id: String,
    parent_revision_ids: Vec<String>,
    content_digest: Option<String>,
    size_bytes: Option<u64>,
    media_type: Option<String>,
    observed_at: String,
    is_head: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceRevisionPageView {
    items: Vec<ResourceRevisionView>,
    next_cursor: Option<String>,
}

#[derive(serde::Deserialize)]
struct ResourceRevisionPageWire {
    items: Vec<ResourceRevisionViewWire>,
    next_cursor: Option<String>,
}

#[derive(serde::Deserialize)]
struct ResourceRevisionViewWire {
    revision: ResourceRevisionWire,
    is_head: bool,
}

#[derive(serde::Deserialize)]
struct ResourceRevisionWire {
    resource_revision_id: String,
    resource_id: String,
    parent_revision_ids: Vec<String>,
    content_digest: Option<String>,
    size_bytes: Option<u64>,
    media_type: Option<String>,
    observed_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourcePageView {
    items: Vec<ResourceView>,
    next_cursor: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceListWire {
    items: Vec<ResourceView>,
    next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceSearchResultView {
    resource_id: String,
    resource_revision_id: String,
    source_content_digest: String,
    source_matches: Vec<ResourceTextMatchView>,
    display_name: String,
    freshness: String,
    match_reasons: Vec<String>,
    snippet: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceTextMatchView {
    term: String,
    start_utf8_byte: u64,
    end_utf8_byte_exclusive: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceTextPreviewWithProvenanceView {
    text: String,
    resource_revision_id: String,
    content_digest: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceSearchPageView {
    items: Vec<ResourceSearchResultView>,
    next_cursor: Option<String>,
    mode: String,
    content_scan: Option<ResourceContentScanView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceTextIndexRebuildView {
    request_id: String,
    correlation_id: String,
    workspace_id: String,
    resource_id: String,
    resource_revision_id: String,
    content_digest: String,
    outcome: String,
    reason: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceTextIndexRebuildWire {
    request_id: String,
    correlation_id: String,
    workspace_id: String,
    resource_id: String,
    resource_revision_id: String,
    content_digest: String,
    outcome: String,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceContentScanView {
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

#[derive(serde::Deserialize)]
struct ResourceContentScanWire {
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

#[derive(serde::Deserialize)]
struct ResourceSearchPageWire {
    items: Vec<ResourceSearchResultWire>,
    next_cursor: Option<String>,
    mode: String,
    content_scan: Option<ResourceContentScanWire>,
}

#[derive(serde::Deserialize)]
struct ResourceSearchResultWire {
    resource_ref: ResourceSearchRefWire,
    source_content_digest: String,
    source_matches: Vec<ResourceTextMatchWire>,
    display_name: String,
    freshness: String,
    match_reasons: Vec<String>,
    snippet: Option<String>,
}

#[derive(serde::Deserialize)]
struct ResourceSearchRefWire {
    workspace_id: String,
    resource_id: String,
    revision_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct ResourceTextMatchWire {
    term: String,
    start_utf8_byte: u64,
    end_utf8_byte_exclusive: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadRangeView {
    start_offset: u64,
    end_offset_inclusive: u64,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceUploadView {
    upload_id: String,
    workspace_id: String,
    display_name: String,
    media_type: String,
    expected_size_bytes: u64,
    expected_digest: Option<String>,
    context_document: Option<serde_json::Value>,
    folder_relative_path: Option<String>,
    resource_id: Option<String>,
    committed_resource_id: Option<String>,
    expected_resource_version: Option<u64>,
    parent_revision_ids: Vec<String>,
    chunk_size_bytes: u64,
    received_ranges: Vec<UploadRangeView>,
    next_missing_offset: u64,
    state: String,
    expires_at: String,
}

#[derive(serde::Deserialize)]
struct ResourceUploadWire {
    upload_id: String,
    workspace_id: String,
    display_name: String,
    media_type: String,
    expected_size_bytes: u64,
    expected_digest: Option<String>,
    context_document: Option<serde_json::Value>,
    folder_import: Option<FolderImportWire>,
    resource_id: Option<String>,
    committed_resource_id: Option<String>,
    expected_resource_version: Option<u64>,
    parent_revision_ids: Vec<String>,
    chunk_size_bytes: u64,
    received_ranges: Vec<UploadRangeWire>,
    next_missing_offset: u64,
    state: String,
    expires_at: String,
}

#[derive(serde::Deserialize)]
struct FolderImportWire {
    relative_path: String,
}

#[derive(serde::Deserialize)]
struct UploadRangeWire {
    start_offset: u64,
    end_offset_inclusive: u64,
    sha256: String,
}

impl From<ResourceUploadWire> for ResourceUploadView {
    fn from(wire: ResourceUploadWire) -> Self {
        Self {
            upload_id: wire.upload_id,
            workspace_id: wire.workspace_id,
            display_name: wire.display_name,
            media_type: wire.media_type,
            expected_size_bytes: wire.expected_size_bytes,
            expected_digest: wire.expected_digest,
            context_document: wire.context_document,
            folder_relative_path: wire.folder_import.map(|origin| origin.relative_path),
            resource_id: wire.resource_id,
            committed_resource_id: wire.committed_resource_id,
            expected_resource_version: wire.expected_resource_version,
            parent_revision_ids: wire.parent_revision_ids,
            chunk_size_bytes: wire.chunk_size_bytes,
            received_ranges: wire.received_ranges.into_iter().map(|range| UploadRangeView {
                start_offset: range.start_offset,
                end_offset_inclusive: range.end_offset_inclusive,
                sha256: range.sha256,
            }).collect(),
            next_missing_offset: wire.next_missing_offset,
            state: wire.state,
            expires_at: wire.expires_at,
        }
    }
}

#[derive(serde::Deserialize)]
struct CommittedResourceWire {
    resource: CommittedResourceMetadataWire,
    revision: CommittedResourceRevisionWire,
}

#[derive(serde::Deserialize)]
struct CommittedResourceMetadataWire {
    resource_id: String,
    workspace_id: String,
    display_name: String,
}

#[derive(serde::Deserialize)]
struct CommittedResourceRevisionWire {
    resource_revision_id: String,
    content_digest: Option<String>,
    size_bytes: Option<u64>,
    media_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceInstructionView {
    workspace_id: String,
    revision: u64,
    parent_revisions: Vec<u64>,
    content_ref: serde_json::Value,
    content_digest: String,
    authored_by: serde_json::Value,
    created_at: String,
}

#[derive(serde::Deserialize)]
struct WorkspaceInstructionWire {
    workspace_id: String,
    revision: u64,
    parent_revisions: Vec<u64>,
    content_ref: serde_json::Value,
    content_digest: String,
    authored_by: serde_json::Value,
    created_at: String,
}

#[derive(serde::Deserialize)]
struct WorkspaceInstructionPageWire {
    items: Vec<WorkspaceInstructionWire>,
    next_cursor: Option<String>,
}

impl From<WorkspaceInstructionWire> for WorkspaceInstructionView {
    fn from(revision: WorkspaceInstructionWire) -> Self {
        Self {
            workspace_id: revision.workspace_id,
            revision: revision.revision,
            parent_revisions: revision.parent_revisions,
            content_ref: revision.content_ref,
            content_digest: revision.content_digest,
            authored_by: revision.authored_by,
            created_at: revision.created_at,
        }
    }
}

#[tauri::command]
async fn list_workspace_instructions(
    app: AppHandle,
    workspace_id: String,
) -> Result<Vec<WorkspaceInstructionView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.is_empty() {
            return Err("Workspace selection is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let url = format!(
            "{}/{}/instructions/revisions",
            workspace_url.trim_end_matches("/workspaces"),
            workspace_id
        );
        let mut cursor: Option<String> = None;
        let mut seen_cursors = std::collections::HashSet::new();
        let mut revisions = Vec::new();
        for _ in 0..1000 {
            let mut request = client
                .get(&url)
                .header("X-Workspace-ID", &workspace_id)
                .query(&[("limit", "200")]);
            if let Some(value) = cursor.as_deref() {
                request = request.query(&[("cursor", value)]);
            }
            let response = request
                .send()
                .map_err(|_| "Workspace instruction history is unavailable".to_owned())?;
            let body = read_bounded_response(response)?;
            let mut page: WorkspaceInstructionPageWire = serde_json::from_slice(&body)
                .map_err(|_| "Local Runtime returned unsupported instruction history".to_owned())?;
            revisions.append(&mut page.items);
            match page.next_cursor {
                Some(next) if seen_cursors.insert(next.clone()) => cursor = Some(next),
                Some(_) => return Err("Workspace instruction pagination repeated a cursor".to_owned()),
                None => {
                    cursor = None;
                    break;
                }
            }
        }
        if cursor.is_some() {
            return Err("Workspace instruction history exceeds the desktop loading limit".to_owned());
        }
        Ok(revisions.into_iter().map(WorkspaceInstructionView::from).collect())
    })
    .await
    .map_err(|_| "Workspace instruction history request did not complete".to_owned())?
}

#[tauri::command]
async fn create_workspace_instruction_revision(
    app: AppHandle,
    workspace_id: String,
    expected_version: u64,
    parent_revisions: Vec<u64>,
    resource_id: String,
    resource_revision_id: String,
    content_digest: String,
    request_id: String,
) -> Result<WorkspaceInstructionView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.is_empty()
            || workspace_id.len() > 200
            || resource_id.is_empty()
            || resource_id.len() > 200
            || resource_revision_id.is_empty()
            || resource_revision_id.len() > 200
            || parent_revisions.is_empty()
            || parent_revisions.len() > MAX_INSTRUCTION_PARENTS
            || parent_revisions.iter().any(|revision| *revision == 0)
            || content_digest.len() > 71
            || request_id.is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err("Workspace instruction request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client
            .post(format!(
                "{}/{}/instructions/revisions",
                workspace_url.trim_end_matches("/workspaces"),
                workspace_id
            ))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_version.to_string())
            .header("Idempotency-Key", request_id)
            .json(&serde_json::json!({
                "parent_revisions": parent_revisions,
                "content_ref": {
                    "workspace_id": workspace_id,
                    "resource_id": resource_id,
                    "revision_id": resource_revision_id,
                },
                "content_digest": content_digest,
            }))
            .send()
            .map_err(|_| "Workspace instructions could not be saved".to_owned())?;
        if !response.is_success() {
            let mut body = Vec::new();
            response
                .take(8193)
                .read_to_end(&mut body)
                .map_err(|_| "Workspace instruction error response could not be read".to_owned())?;
            if body.len() > 8192 {
                return Err("Workspace instructions could not be saved".to_owned());
            }
            let code = serde_json::from_slice::<serde_json::Value>(&body)
                .ok()
                .and_then(|value| value.pointer("/error/code").and_then(serde_json::Value::as_str).map(str::to_owned));
            return Err(match code.as_deref() {
                Some("STALE_WORKSPACE_VERSION") => "STALE_WORKSPACE_VERSION".to_owned(),
                Some("INVALID_ARGUMENT") => "Workspace instructions are invalid; check the text and Resource size.".to_owned(),
                _ => "Workspace instructions could not be saved.".to_owned(),
            });
        }
        let body = read_bounded_response(response)?;
        let revision: WorkspaceInstructionWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported instruction revision".to_owned())?;
        Ok(WorkspaceInstructionView::from(revision))
    })
    .await
    .map_err(|_| "Workspace instruction update did not complete".to_owned())?
}

#[tauri::command]
async fn list_workspaces(app: AppHandle) -> Result<Vec<WorkspaceView>, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_workspaces(&app))
        .await
        .map_err(|_| "Workspace request did not complete".to_owned())?
}

#[tauri::command]
async fn list_workspace_roots(
    app: AppHandle,
    workspace_id: String,
    cursor: Option<String>,
) -> Result<WorkspaceRootPageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty() || cursor.as_deref().is_some_and(|value| value.is_empty() || value.len() > 2048) {
            return Err("Persistent folder list request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let mut request = client
            .get(format!("{}/workspace-roots", workspace_url.trim_end_matches("/workspaces")))
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("limit", "100")]);
        if let Some(cursor) = cursor.as_deref() {
            request = request.query(&[("cursor", cursor)]);
        }
        let response = request.send().map_err(|_| "Persistent folders are unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: WorkspaceRootPageWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported persistent folder list".to_owned())?;
        if page.items.len() > 100 || page.next_cursor.as_deref().is_some_and(|value| value.is_empty() || value.len() > 2048) {
            return Err("Local Runtime returned an invalid persistent folder page".to_owned());
        }
        let items: Vec<WorkspaceRootView> = page.items.into_iter().map(WorkspaceRootView::from).collect();
        if items.iter().any(|item| item.workspace_id != workspace_id) {
            return Err("Local Runtime returned a folder from a different Workspace".to_owned());
        }
        Ok(WorkspaceRootPageView { items, next_cursor: page.next_cursor })
    })
    .await
    .map_err(|_| "Persistent folder list request did not complete".to_owned())?
}

#[tauri::command]
async fn add_workspace_root(
    app: AppHandle,
    workspace_id: String,
    expected_workspace_version: u64,
    watch_policy: String,
    replication_policy: String,
    request_id: String,
) -> Result<Option<WorkspaceRootView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty()
            || workspace_id.len() > 200
            || !workspace_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || !matches!(watch_policy.as_str(), "METADATA" | "CONTENT_DIGESTS" | "SELECTED_TEXT_EXTRACTION")
            || !matches!(replication_policy.as_str(), "NONE" | "ACTIVE_TASKS" | "SELECTED_WORKSPACE_POLICY")
            || request_id.is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err("Persistent folder request is invalid".to_owned());
        }
        let Some(selected_path) = rfd::FileDialog::new()
            .set_title("Add a persistent folder to this Workspace")
            .pick_folder()
        else {
            return Ok(None);
        };
        #[cfg(unix)]
        let selected_path_base64 = {
            use std::os::unix::ffi::OsStrExt;
            URL_SAFE_NO_PAD.encode(selected_path.as_os_str().as_bytes())
        };
        #[cfg(not(unix))]
        let selected_path_base64: String = {
            let _ = selected_path;
            return Err("Persistent folders are not qualified on this platform".to_owned());
        };
        let (client, _) = operator_client(&app)?;
        let response = client
            .post(operator_ipc::PRIVATE_LOCAL_ROOT_SELECTION_PATH)
            .header("Idempotency-Key", &request_id)
            .json(&serde_json::json!({
                "workspace_id": workspace_id,
                "expected_workspace_version": expected_workspace_version,
                "selected_path_base64": selected_path_base64,
                "watch_policy": watch_policy,
                "replication_policy": replication_policy,
            }))
            .send()
            .map_err(|_| "Persistent folder could not be added".to_owned())?;
        let body = read_bounded_response(response)?;
        let root: WorkspaceRootWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported persistent folder".to_owned())?;
        if root.workspace_id != workspace_id
            || root.display_name.trim().is_empty()
            || root.status != "ACTIVE"
        {
            return Err("Local Runtime returned a persistent folder for a different request".to_owned());
        }
        Ok(Some(WorkspaceRootView::from(root)))
    })
    .await
    .map_err(|_| "Persistent folder selection did not complete".to_owned())?
}

#[tauri::command]
async fn revoke_workspace_root(
    app: AppHandle,
    workspace_id: String,
    workspace_root_id: String,
    expected_version: u64,
    request_id: String,
) -> Result<WorkspaceRootView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid_id = |value: &str| {
            !value.is_empty()
                && value.len() <= 200
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        };
        if !valid_id(&workspace_id)
            || !valid_id(&workspace_root_id)
            || request_id.is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err("Persistent folder removal request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let base = format!("{}/workspace-roots", workspace_url.trim_end_matches("/workspaces"));
        let response = client
            .post(format!("{base}/{workspace_root_id}/revoke"))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_version.to_string())
            .header("Idempotency-Key", request_id)
            .send()
            .map_err(|_| "Persistent folder access could not be removed".to_owned())?;
        let body = read_bounded_response(response)?;
        let root: WorkspaceRootWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported folder status".to_owned())?;
        if root.workspace_id != workspace_id
            || root.workspace_root_id != workspace_root_id
            || root.status != "REVOKED"
        {
            return Err("Local Runtime returned a different folder status".to_owned());
        }
        Ok(WorkspaceRootView::from(root))
    })
    .await
    .map_err(|_| "Persistent folder removal did not complete".to_owned())?
}

#[tauri::command]
async fn pause_workspace_root(
    app: AppHandle,
    workspace_id: String,
    workspace_root_id: String,
    expected_version: u64,
    request_id: String,
) -> Result<WorkspaceRootView, String> {
    change_workspace_root_status(
        app,
        workspace_id,
        workspace_root_id,
        expected_version,
        request_id,
        "pause",
        "ACTIVE",
        "PAUSED",
    )
    .await
}

#[tauri::command]
async fn resume_workspace_root(
    app: AppHandle,
    workspace_id: String,
    workspace_root_id: String,
    expected_version: u64,
    request_id: String,
) -> Result<WorkspaceRootView, String> {
    change_workspace_root_status(
        app,
        workspace_id,
        workspace_root_id,
        expected_version,
        request_id,
        "resume",
        "PAUSED",
        "ACTIVE",
    )
    .await
}

async fn change_workspace_root_status(
    app: AppHandle,
    workspace_id: String,
    workspace_root_id: String,
    expected_version: u64,
    request_id: String,
    action: &'static str,
    expected_status: &'static str,
    committed_status: &'static str,
) -> Result<WorkspaceRootView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid_id = |value: &str| {
            !value.is_empty()
                && value.len() <= 200
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        };
        if !valid_id(&workspace_id)
            || !valid_id(&workspace_root_id)
            || expected_version == 0
            || expected_version == u64::MAX
            || request_id.is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err("Persistent folder status request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let base = format!("{}/workspace-roots", workspace_url.trim_end_matches("/workspaces"));
        let response = client
            .post(format!("{base}/{workspace_root_id}/{action}"))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_version.to_string())
            .header("Idempotency-Key", request_id)
            .send()
            .map_err(|_| "Persistent folder status could not be updated".to_owned())?;
        if response.status() != 200 {
            return match read_bounded_response(response) {
                Err(error) => Err(error),
                Ok(_) => Err("Local Runtime returned an unexpected folder status response".to_owned()),
            };
        }
        let body = read_bounded_response(response)?;
        let root: WorkspaceRootWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported folder status".to_owned())?;
        if root.workspace_id != workspace_id
            || root.workspace_root_id != workspace_root_id
            || root.status != committed_status
            || root.version != expected_version + 1
            || root.resource_id.trim().is_empty()
            || root.display_name.trim().is_empty()
        {
            return Err(format!(
                "Local Runtime did not confirm that this folder changed from {expected_status} to {committed_status}"
            ));
        }
        Ok(WorkspaceRootView::from(root))
    })
    .await
    .map_err(|_| "Persistent folder status request did not complete".to_owned())?
}

#[tauri::command]
async fn list_tasks(
    app: AppHandle,
    workspace_id: String,
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<u16>,
) -> Result<TaskPageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        const STATUSES: &[&str] = &[
            "READY", "RUNNING", "WAITING_USER", "BLOCKED", "VERIFYING", "NEEDS_USER",
            "INCOMPLETE", "PAUSE_REQUESTED", "PAUSED", "COMPLETED", "FAILED",
            "CANCEL_REQUESTED", "CANCELLED",
        ];
        if workspace_id.trim().is_empty()
            || status.as_deref().is_some_and(|value| !STATUSES.contains(&value))
            || cursor.as_deref().is_some_and(|value| value.is_empty() || value.len() > 2048)
            || limit.is_some_and(|value| !(1..=100).contains(&value))
        {
            return Err("Task list request is invalid".to_owned());
        }
        let limit = limit.unwrap_or(50).to_string();
        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let mut request = client
            .get(format!("{base_url}/tasks"))
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("limit", limit.as_str())]);
        if let Some(status) = status.as_deref() {
            request = request.query(&[("status", status)]);
        }
        if let Some(cursor) = cursor.as_deref() {
            request = request.query(&[("cursor", cursor)]);
        }
        let response = request
            .send()
            .map_err(|_| "Local Task list is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: TaskPageWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported Task list".to_owned())?;
        Ok(TaskPageView {
            items: page.items.into_iter().map(TaskSummaryView::from).collect(),
            next_cursor: page.next_cursor,
        })
    })
    .await
    .map_err(|_| "Task list request did not complete".to_owned())?
}

/// Saves a standalone Task envelope. This command deliberately does not start an
/// AgentSession or imply that planning or execution has begun.
#[tauri::command]
async fn create_task(
    app: AppHandle,
    workspace_id: String,
    objective: String,
    preferred_lead_agent_binding_id: String,
    coworker_id: Option<String>,
    expected_coworker_version: Option<u64>,
    input_refs: Vec<PinnedResourceRefInput>,
    request_id: String,
) -> Result<TaskDetailView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let objective = objective.trim();
        if workspace_id.trim().is_empty()
            || workspace_id.len() > 200
            || !workspace_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || objective.is_empty()
            || objective.len() > 32 * 1024
            || preferred_lead_agent_binding_id.trim().is_empty()
            || preferred_lead_agent_binding_id.len() > 200
            || !preferred_lead_agent_binding_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || coworker_id.as_deref().is_some_and(|id| id.trim().is_empty() || id.len() > 200 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'))
            || (coworker_id.is_none() && expected_coworker_version.is_some())
            || expected_coworker_version.is_some_and(|version| version == 0)
            || input_refs.iter().any(|input| {
                input.workspace_id != workspace_id
                    || input.resource_id.trim().is_empty()
                    || input.resource_id.len() > 200
                    || !input.resource_id.bytes().all(|byte| byte.is_ascii_graphic())
                    || input.revision_id.trim().is_empty()
                    || input.revision_id.len() > 200
                    || !input.revision_id.bytes().all(|byte| byte.is_ascii_graphic())
            })
            || request_id.is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err("Task request is invalid".to_owned());
        }
        if input_refs.len() > MAX_TASK_INPUT_REFS {
            return Err(format!(
                "A Task can include at most {MAX_TASK_INPUT_REFS} pinned Resource inputs"
            ));
        }

        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .post(format!("{base_url}/tasks"))
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", request_id)
            .json(&serde_json::json!({
                "workspace_id": workspace_id,
                "objective": objective,
                "constraints": [],
                "non_goals": [],
                "source_message_refs": [],
                "input_refs": input_refs.iter().map(|input| serde_json::json!({
                    "workspace_id": input.workspace_id,
                    "resource_id": input.resource_id,
                    "revision_id": input.revision_id,
                })).collect::<Vec<_>>(),
                "required_outputs": [],
                "acceptance_criteria": [],
                "approvals_required": [],
                "preferred_lead_agent_binding_id": preferred_lead_agent_binding_id,
                "coworker_id": coworker_id,
                "expected_coworker_version": expected_coworker_version,
                "placement_preference": "AUTO",
            }))
            .send()
            .map_err(|_| "Local Task service is unavailable; retry this save to reconcile it".to_owned())?;
        let body = read_bounded_response(response)?;
        let view: TaskViewWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported Task details".to_owned())?;
        if view.task.workspace_id != workspace_id
            || view.task.origin_coworker_id != coworker_id
            || (view.task.origin_coworker_id.is_some() != view.task.origin_coworker_revision.is_some())
            || view.task.current_spec_revision != 1
            || view.current_spec_revision.task_id != view.task.task_id
            || view.current_spec_revision.revision != view.task.current_spec_revision
            || view.current_spec_revision.objective != objective
            || view.current_spec_revision.input_refs != input_refs.iter().map(|input| PinnedResourceRefWire {
                workspace_id: input.workspace_id.clone(),
                resource_id: input.resource_id.clone(),
                revision_id: input.revision_id.clone(),
            }).collect::<Vec<_>>()
        {
            return Err("Local Runtime returned a Task that does not match this save request".to_owned());
        }
        Ok(TaskDetailView::from(view))
    })
    .await
    .map_err(|_| "Task save request did not complete".to_owned())?
}

#[tauri::command]
async fn get_task(
    app: AppHandle,
    workspace_id: String,
    task_id: String,
) -> Result<TaskDetailView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty()
            || task_id.trim().is_empty()
            || task_id.len() > 200
            || !task_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err("Task selection is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .get(format!("{base_url}/tasks/{task_id}"))
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "Local Task details are unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let view: TaskViewWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported Task details".to_owned())?;
        if view.task.workspace_id != workspace_id || view.task.task_id != task_id {
            return Err("Local Runtime returned Task details outside the selected Workspace".to_owned());
        }
        if view.task.origin_coworker_id.is_some() != view.task.origin_coworker_revision.is_some() {
            return Err("Local Runtime returned Task details with an incomplete Coworker origin".to_owned());
        }
        if view.current_spec_revision.task_id != view.task.task_id
            || view.current_spec_revision.revision != view.task.current_spec_revision
            || view.current_spec_revision.input_refs.iter().any(|input| input.workspace_id != workspace_id)
        {
            return Err("Local Runtime returned Task details with inconsistent specification ownership or revision".to_owned());
        }
        let mut detail = TaskDetailView::from(view);
        if let Some(plan_revision) = detail.current_plan_revision {
            let plan_response = client
                .get(format!("{base_url}/tasks/{task_id}/plan-revisions"))
                .header("X-Workspace-ID", &workspace_id)
                .send()
                .map_err(|_| "Task plan history is unavailable".to_owned())?;
            let plan_body = read_bounded_response(plan_response)?;
            let plan_revisions: Vec<TaskPlanRevisionWire> = serde_json::from_slice(&plan_body)
                .map_err(|_| "Local Runtime returned unsupported Task plan history".to_owned())?;
            if plan_revisions.iter().any(|plan| plan.task_id != task_id) {
                return Err("Local Runtime returned plan history outside the selected Task".to_owned());
            }
            let mut matching_plans: Vec<TaskPlanRevisionWire> = plan_revisions.into_iter()
                .filter(|plan| plan.revision == plan_revision)
                .collect();
            if matching_plans.len() != 1 {
                return Err("Local Runtime did not return exactly one current Task plan revision".to_owned());
            }
            let current_plan = matching_plans.remove(0);
            if current_plan.steps.is_empty()
                || current_plan.task_spec_revision == 0
                || current_plan.task_spec_revision > detail.current_spec_revision
                || current_plan.steps.iter().enumerate().any(|(index, planned)| {
                    current_plan.steps[..index].iter().any(|previous| previous.logical_key == planned.logical_key)
                })
            {
                return Err("Local Runtime returned an invalid current plan revision".to_owned());
            }

            let steps_response = client
                .get(format!("{base_url}/tasks/{task_id}/steps"))
                .header("X-Workspace-ID", &workspace_id)
                .send()
                .map_err(|_| "Task Steps are unavailable".to_owned())?;
            let steps_body = read_bounded_response(steps_response)?;
            let step_rows: Vec<TaskStepWire> = serde_json::from_slice(&steps_body)
                .map_err(|_| "Local Runtime returned unsupported Task Steps".to_owned())?;
            if step_rows.iter().any(|step| step.task_id != task_id) {
                return Err("Local Runtime returned Steps outside the selected Task".to_owned());
            }
            let mut current_steps: Vec<TaskStepWire> = step_rows.into_iter()
                .filter(|step| step.plan_revision == plan_revision)
                .collect();
            if current_steps.len() != current_plan.steps.len() {
                return Err("Local Runtime returned an incomplete current Task plan".to_owned());
            }
            if current_steps.iter().enumerate().any(|(index, step)| {
                !matches!(step.status.as_str(), "PENDING" | "READY" | "RUNNING" | "WAITING_USER" | "BLOCKED" | "VERIFYING" | "COMPLETED" | "FAILED" | "CANCEL_REQUESTED" | "CANCELLED" | "SUPERSEDED")
                    || step.logical_key.as_ref().is_none_or(|logical_key| {
                    current_steps[..index].iter().any(|previous| {
                        previous.step_id == step.step_id
                            || previous.logical_key.as_ref() == Some(logical_key)
                    })
                })
            }) {
                return Err("Local Runtime returned duplicate or unkeyed Steps for the current plan".to_owned());
            }
            let mut planned_steps = Vec::with_capacity(current_plan.steps.len());
            for planned in current_plan.steps {
                let Some(index) = current_steps.iter().position(|step| {
                    step.logical_key.as_deref() == Some(planned.logical_key.as_str())
                }) else {
                    return Err("Local Runtime returned Task plan Steps with inconsistent identities".to_owned());
                };
                let step = current_steps.remove(index);
                if step.title != planned.title || step.objective != planned.objective {
                    return Err("Local Runtime returned Task plan content with inconsistent Step identities".to_owned());
                }
                planned_steps.push(PlannedStepView {
                    step_id: step.step_id,
                    logical_key: planned.logical_key,
                    title: step.title,
                    objective: step.objective,
                    status: step.status,
                });
            }
            if !current_steps.is_empty() {
                return Err("Local Runtime returned unexpected Steps for the current plan".to_owned());
            }
            detail.plan_spec_revision = Some(current_plan.task_spec_revision);
            detail.plan_is_stale = current_plan.task_spec_revision != detail.current_spec_revision;
            detail.planned_steps = planned_steps;
        }
        Ok(detail)
    })
    .await
    .map_err(|_| "Task detail request did not complete".to_owned())?
}

/// Returns immutable TaskSpec history through the authenticated, Workspace-scoped
/// Operator route. This command exposes history only; it cannot restore or revise it.
#[tauri::command]
async fn list_task_spec_revisions(
    app: AppHandle,
    workspace_id: String,
    task_id: String,
) -> Result<Vec<TaskSpecHistoryRevisionView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid_id = |value: &str| {
            !value.trim().is_empty()
                && value.len() <= 200
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        };
        if !valid_id(&workspace_id) || !valid_id(&task_id) {
            return Err("Task specification history selection is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .get(format!("{base_url}/tasks/{task_id}/spec-revisions"))
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "Task specification history is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let revisions: Vec<TaskSpecHistoryRevisionWire> = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported Task specification history".to_owned())?;
        if revisions.is_empty() {
            return Err("Local Runtime returned no initial Task specification revision".to_owned());
        }
        let mut previous_revision = 0;
        let mut views = Vec::with_capacity(revisions.len());
        for revision in revisions {
            if revision.task_id != task_id
                || revision.workspace_id.as_deref().is_some_and(|revision_workspace| revision_workspace != workspace_id)
                || revision.revision <= previous_revision
                || revision.revision == 0
                || revision.parent_revisions.iter().any(|parent| *parent >= revision.revision)
                || revision.objective.trim().is_empty()
                || revision.created_at.trim().is_empty()
                || revision.authored_by.principal_id.trim().is_empty()
                || !matches!(revision.authored_by.kind.as_str(), "USER" | "SERVICE" | "RUNTIME" | "AGENT" | "CHANNEL_IDENTITY")
            {
                return Err("Local Runtime returned inconsistent Task specification history".to_owned());
            }
            previous_revision = revision.revision;
            views.push(TaskSpecHistoryRevisionView::from_wire(revision, workspace_id.clone()));
        }
        Ok(views)
    })
    .await
    .map_err(|_| "Task specification history request did not complete".to_owned())?
}

/// Fetches only the daemon's sanitized, read-only planning preflight. This command
/// cannot reserve a planner, start an agent, or create Task execution state.
#[tauri::command]
async fn get_task_planning_readiness(
    app: AppHandle,
    workspace_id: String,
    task_id: String,
    expected_task_version: u64,
) -> Result<TaskPlanningReadinessView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty()
            || workspace_id.len() > 200
            || !workspace_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || task_id.trim().is_empty()
            || task_id.len() > 200
            || !task_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || expected_task_version == 0
        {
            return Err("Planning readiness request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .get(format!("{base_url}/tasks/{task_id}/planning-readiness"))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", format!("\"{expected_task_version}\""))
            .send()
            .map_err(|_| "Local planning readiness is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let wire: TaskPlanningReadinessWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported planning readiness response".to_owned())?;
        let view = TaskPlanningReadinessView::try_from(wire)?;
        validate_task_planning_readiness_identity(view, &task_id, expected_task_version)
    })
    .await
    .map_err(|_| "Planning readiness request did not complete".to_owned())?
}

/// Revises only the objective of a saved Task through the authenticated Operator.
/// The durable API preserves every omitted TaskSpec field and appends an immutable
/// revision; this command does not launch a planner or execution session.
#[tauri::command]
async fn revise_task_spec(
    app: AppHandle,
    workspace_id: String,
    task_id: String,
    expected_task_version: u64,
    parent_revision: u64,
    objective: String,
    request_id: String,
) -> Result<TaskSpecRevisionReceiptView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let objective = objective.trim();
        if workspace_id.trim().is_empty()
            || workspace_id.len() > 200
            || !workspace_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || task_id.trim().is_empty()
            || task_id.len() > 200
            || !task_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || expected_task_version == 0
            || parent_revision == 0
            || objective.is_empty()
            || objective.len() > 32 * 1024
            || request_id.is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err("Task edit request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .post(format!("{base_url}/tasks/{task_id}/spec-revisions"))
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", &request_id)
            .header("If-Match", expected_task_version.to_string())
            .json(&serde_json::json!({
                "parent_revisions": [parent_revision],
                "objective": objective,
            }))
            .send()
            .map_err(|_| "Task edit response is unknown; retry the unchanged edit to reconcile it".to_owned())?;
        let body = read_bounded_response(response)?;
        let revision: TaskSpecDetailWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported Task specification revision".to_owned())?;
        if revision.task_id != task_id
            || revision.revision != parent_revision.saturating_add(1)
            || revision.objective != objective
            || revision.input_refs.iter().any(|input| input.workspace_id != workspace_id)
        {
            return Err("Local Runtime returned a Task revision that does not match this edit".to_owned());
        }
        Ok(TaskSpecRevisionReceiptView {
            task_id: revision.task_id,
            revision: revision.revision,
            objective: revision.objective,
        })
    })
    .await
    .map_err(|_| "Task edit request did not complete".to_owned())?
}

#[tauri::command]
async fn list_agent_installations(app: AppHandle) -> Result<Vec<AgentInstallationView>, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_agent_installations(&app))
        .await
        .map_err(|_| "Agent inventory request failed".to_owned())?
}

fn fetch_agent_installations(app: &AppHandle) -> Result<Vec<AgentInstallationView>, String> {
    let (client, workspace_url) = operator_client(app)?;
    let url = workspace_url.replace("/v1/workspaces", "/v1/agent-installations");
    let response = client
        .get(url)
        .send()
        .map_err(|_| "Local agent inventory is unavailable".to_owned())?;
    let body = read_bounded_response(response)
        .map_err(|_| "Local Operator returned an invalid agent inventory".to_owned())?;
    let wire: AgentInstallationListWire = serde_json::from_slice(&body)
        .map_err(|_| "Local Operator returned an unsupported agent inventory".to_owned())?;
    Ok(wire.items.into_iter().map(AgentInstallationView::from).collect())
}

#[tauri::command]
async fn list_agent_profiles(app: AppHandle, workspace_id: String) -> Result<AgentProfilePageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty() { return Err("Workspace selection is required".to_owned()); }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client.get(workspace_url.replace("/v1/workspaces", "/v1/agent-profiles"))
            .header("X-Workspace-ID", &workspace_id)
            .send().map_err(|_| "Local agent profiles are unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: AgentProfilePageWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported agent profiles".to_owned())?;
        Ok(AgentProfilePageView { items: page.items.into_iter().map(AgentProfileView::from).collect(), next_cursor: page.next_cursor })
    }).await.map_err(|_| "Agent profile request did not complete".to_owned())?
}

#[tauri::command]
async fn probe_agent_profile(
    app: AppHandle,
    workspace_id: String,
    provider_key: String,
) -> Result<AgentProfileView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty()
            || workspace_id.len() > 200
            || !workspace_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err("Workspace selection is invalid".to_owned());
        }
        if !matches!(provider_key.as_str(), "CODEX" | "OPENCODE") {
            return Err("This agent profile probe is not available".to_owned());
        }
        let provider_name = if provider_key == "CODEX" { "Codex" } else { "OpenCode" };
        let (client, workspace_url) = operator_client(&app)?;
        let response = client.post(workspace_url.replace("/v1/workspaces", "/v1/agent-profiles/probe"))
            .header("X-Workspace-ID", &workspace_id)
            .json(&serde_json::json!({"provider_key": provider_key}))
            .send().map_err(|_| format!("{provider_name} profile probe is unavailable"))?;
        let body = read_bounded_response(response)?;
        let profile: AgentProfileWire = serde_json::from_slice(&body)
            .map_err(|_| format!("Local Runtime returned an unsupported {provider_name} profile"))?;
        if profile.provider_key != provider_key {
            return Err("Local Runtime returned a different agent profile than requested".to_owned());
        }
        Ok(AgentProfileView::from(profile))
    }).await.map_err(|_| "Agent profile probe did not complete".to_owned())?
}

#[tauri::command]
async fn list_agent_bindings(app: AppHandle, workspace_id: String) -> Result<AgentBindingPageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty() { return Err("Workspace selection is required".to_owned()); }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client.get(workspace_url.replace("/v1/workspaces", "/v1/agent-bindings"))
            .header("X-Workspace-ID", &workspace_id)
            .send().map_err(|_| "Workspace agent bindings are unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: AgentBindingPageWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported agent bindings".to_owned())?;
        Ok(AgentBindingPageView { items: page.items.into_iter().map(AgentBindingView::from).collect(), next_cursor: page.next_cursor })
    }).await.map_err(|_| "Agent binding request did not complete".to_owned())?
}

#[tauri::command]
async fn create_agent_binding(
    app: AppHandle,
    workspace_id: String,
    agent_profile_id: String,
    lead_eligible: bool,
    runtime_id: Option<String>,
    request_id: String,
) -> Result<AgentBindingView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty() || agent_profile_id.trim().is_empty() || request_id.trim().is_empty() || request_id.len() > 128 {
            return Err("Agent binding request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client.post(workspace_url.replace("/v1/workspaces", "/v1/agent-bindings"))
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", request_id)
            .json(&serde_json::json!({
                "workspace_id": workspace_id,
                "agent_profile_id": agent_profile_id,
                "lead_eligible": lead_eligible,
                "runtime_id": runtime_id,
                "endpoint_selection_policy": {"mode":"AUTO_COMPATIBLE","required_features":[],"preferred_topologies":[]},
                "auth_ref": null,
                "configuration": {},
            }))
            .send().map_err(|_| "Workspace agent binding could not be created".to_owned())?;
        let body = read_bounded_response(response)?;
        let binding: AgentBindingWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported agent binding".to_owned())?;
        Ok(AgentBindingView::from(binding))
    }).await.map_err(|_| "Agent binding creation did not complete".to_owned())?
}

#[tauri::command]
async fn enable_agent_binding(
    app: AppHandle,
    workspace_id: String,
    agent_binding_id: String,
    expected_version: u64,
    request_id: String,
) -> Result<AgentBindingView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty() || agent_binding_id.trim().is_empty() || request_id.trim().is_empty() || request_id.len() > 128 {
            return Err("Agent binding enable request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let base = workspace_url.replace("/v1/workspaces", "/v1/agent-bindings");
        let response = client.post(format!("{base}/{agent_binding_id}/enable"))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_version.to_string())
            .header("Idempotency-Key", request_id)
            .send().map_err(|_| "Workspace agent binding could not be enabled".to_owned())?;
        let body = read_bounded_response(response)?;
        let binding: AgentBindingWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported agent binding".to_owned())?;
        Ok(AgentBindingView::from(binding))
    }).await.map_err(|_| "Agent binding enable request did not complete".to_owned())?
}

#[tauri::command]
async fn list_workspace_runtime_bindings(app: AppHandle, workspace_id: String) -> Result<Vec<RuntimeWorkspaceBindingView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty() { return Err("Workspace selection is required".to_owned()); }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client.get(format!("{}/{workspace_id}/runtime-bindings/current-local", workspace_url.trim_end_matches("/workspaces")))
            .header("X-Workspace-ID", &workspace_id)
            .send().map_err(|_| "Workspace Runtime enrollment is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: RuntimeWorkspaceBindingPageWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported Workspace enrollment".to_owned())?;
        Ok(page.items.into_iter().map(RuntimeWorkspaceBindingView::from).collect())
    }).await.map_err(|_| "Workspace Runtime enrollment request did not complete".to_owned())?
}

#[tauri::command]
async fn enroll_local_runtime(
    app: AppHandle,
    workspace_id: String,
    expected_workspace_version: u64,
    request_id: String,
) -> Result<RuntimeWorkspaceBindingView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty() || request_id.trim().is_empty() || request_id.len() > 128 {
            return Err("Local Runtime enrollment request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client.post(format!("{}/{workspace_id}/runtime-bindings/local-enrollment", workspace_url.trim_end_matches("/workspaces")))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_workspace_version.to_string())
            .header("Idempotency-Key", request_id)
            .send().map_err(|_| "Local Runtime could not be enrolled in this Workspace".to_owned())?;
        let body = read_bounded_response(response)?;
        let binding: RuntimeWorkspaceBindingWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported enrollment state".to_owned())?;
        Ok(RuntimeWorkspaceBindingView::from(binding))
    }).await.map_err(|_| "Local Runtime enrollment request did not complete".to_owned())?
}

#[tauri::command]
async fn create_workspace(
    app: AppHandle,
    name: String,
    request_id: String,
) -> Result<WorkspaceView, String> {
    tauri::async_runtime::spawn_blocking(move || create_workspace_request(&app, name, request_id))
        .await
        .map_err(|_| "Workspace creation did not complete".to_owned())?
}

#[tauri::command]
async fn update_workspace_policy(
    app: AppHandle,
    workspace_id: String,
    replication_policy: String,
    expected_version: u64,
    request_id: String,
) -> Result<WorkspaceView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.is_empty()
            || !matches!(
                replication_policy.as_str(),
                "LOCAL_ONLY" | "METADATA_ONLY" | "ACTIVE_TASKS" | "FULL_WORKSPACE"
            )
            || request_id.is_empty()
        {
            return Err("Workspace policy request is invalid".to_owned());
        }
        let (client, url) = operator_client(&app)?;
        let response = client
            .patch(format!("{}/{}", url, workspace_id))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_version.to_string())
            .header("Idempotency-Key", request_id)
            .json(&serde_json::json!({ "replication_policy": replication_policy }))
            .send()
            .map_err(|_| "Workspace policy update did not complete".to_owned())?;
        if !response.is_success() {
            return Err("Workspace policy could not be updated; refresh and retry".to_owned());
        }
        let body = read_bounded_response(response)?;
        let response: WorkspaceWire = serde_json::from_slice(&body).map_err(|_| {
            "Local Operator API returned an unsupported Workspace response".to_owned()
        })?;
        Ok(WorkspaceView::from(response))
    })
    .await
    .map_err(|_| "Workspace policy update did not complete".to_owned())?
}

#[tauri::command]
async fn set_workspace_default_agent_binding(
    app: AppHandle,
    workspace_id: String,
    agent_binding_id: Option<String>,
    expected_version: u64,
    request_id: String,
) -> Result<WorkspaceView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty()
            || request_id.trim().is_empty()
            || agent_binding_id.as_ref().is_some_and(|id| id.trim().is_empty())
        {
            return Err("Workspace default agent request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let url = format!(
            "{}/{}/default-agent-binding",
            workspace_url.trim_end_matches("/workspaces"),
            workspace_id
        );
        let response = client
            .patch(url)
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_version.to_string())
            .header("Idempotency-Key", request_id)
            .json(&serde_json::json!({ "agent_binding_id": agent_binding_id }))
            .send()
            .map_err(|_| "Workspace default agent update did not complete".to_owned())?;
        if !response.is_success() {
            return Err("Workspace default agent could not be updated; refresh and retry".to_owned());
        }
        let body = read_bounded_response(response)?;
        let workspace: WorkspaceWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Operator API returned an unsupported Workspace response".to_owned())?;
        Ok(WorkspaceView::from(workspace))
    })
    .await
    .map_err(|_| "Workspace default agent update did not complete".to_owned())?
}

#[tauri::command]
async fn get_resource_detail(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
) -> Result<ResourceDetailView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_resource_selection(&workspace_id, &resource_id)?;
        let (client, workspace_url) = operator_client(&app)?;
        let response = client.get(format!("{}/resources/{}", workspace_url.trim_end_matches("/workspaces"), resource_id))
            .header("X-Workspace-ID", &workspace_id)
            .send().map_err(|_| "Resource metadata is unavailable".to_owned())?;
        if response.status() == 404 {
            return Err("Resource is unavailable in this Workspace".to_owned());
        }
        if !response.is_success() {
            return Err("Resource metadata is unavailable".to_owned());
        }
        let body = read_bounded_response(response)?;
        let wire: ResourceDetailWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported Resource metadata".to_owned())?;
        if wire.resource_id != resource_id || wire.workspace_id != workspace_id {
            return Err("Local Runtime returned metadata for a different Resource".to_owned());
        }
        Ok(ResourceDetailView {
            resource_id: wire.resource_id,
            workspace_id: wire.workspace_id,
            kind: wire.kind,
            display_name: wire.display_name,
            current_revision_id: wire.current_revision_id,
            version: wire.version,
            context_document: wire.context_document,
        })
    }).await.map_err(|_| "Resource metadata request did not complete".to_owned())?
}

#[tauri::command]
async fn set_context_document_status(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
    expected_version: u64,
    request_id: String,
    target_status: String,
) -> Result<ResourceDetailView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_resource_selection(&workspace_id, &resource_id)?;
        if expected_version == 0 || expected_version == u64::MAX
            || request_id.is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
            || !matches!(target_status.as_str(), "ACTIVE" | "REVOKED")
        {
            return Err("ContextDocument status request is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client
            .patch(format!("{}/resources/{}/context-document/status", workspace_url.trim_end_matches("/workspaces"), resource_id))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_version.to_string())
            .header("Idempotency-Key", request_id)
            .json(&serde_json::json!({ "status": target_status }))
            .send()
            .map_err(|_| "ContextDocument status could not be updated. Retry to resolve the same request.".to_owned())?;
        if response.status() != 200 {
            let body = read_bounded_response(response)?;
            let code = serde_json::from_slice::<serde_json::Value>(&body)
                .ok()
                .and_then(|value| value.pointer("/error/code").and_then(serde_json::Value::as_str).map(str::to_owned));
            return Err(match code.as_deref() {
                Some("RESOURCE_CONFLICT") => "RESOURCE_CONFLICT: Resource changed. Refresh its current status before trying again.".to_owned(),
                Some("WORKSPACE_ARCHIVED") => "WORKSPACE_ARCHIVED: Archived Workspaces are read-only.".to_owned(),
                Some("NOT_FOUND") => "ContextDocument is unavailable in this Workspace.".to_owned(),
                _ => "ContextDocument status could not be updated.".to_owned(),
            });
        }
        let body = read_bounded_response(response)?;
        let wire: ResourceDetailWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported ContextDocument metadata".to_owned())?;
        if wire.resource_id != resource_id || wire.workspace_id != workspace_id
            || wire.context_document.as_ref().and_then(|value| value.get("status")).and_then(serde_json::Value::as_str) != Some(target_status.as_str())
            || wire.version != expected_version + 1
        {
            return Err("Local Runtime returned a different ContextDocument status".to_owned());
        }
        Ok(ResourceDetailView {
            resource_id: wire.resource_id,
            workspace_id: wire.workspace_id,
            kind: wire.kind,
            display_name: wire.display_name,
            current_revision_id: wire.current_revision_id,
            version: wire.version,
            context_document: wire.context_document,
        })
    }).await.map_err(|_| "ContextDocument status request did not complete".to_owned())?
}

#[tauri::command]
async fn list_resource_revisions(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
    cursor: Option<String>,
) -> Result<ResourceRevisionPageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_resource_selection(&workspace_id, &resource_id)?;
        if cursor.as_deref().is_some_and(|value| value.is_empty() || value.len() > 4096) {
            return Err("Resource revision cursor is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let mut request = client.get(format!("{}/resources/{}/revisions", workspace_url.trim_end_matches("/workspaces"), resource_id))
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("limit", "100")]);
        if let Some(cursor) = cursor { request = request.query(&[("cursor", cursor)]); }
        let response = request.send().map_err(|_| "Resource revision history is unavailable".to_owned())?;
        if !response.is_success() { return Err("Resource revision history is unavailable".to_owned()); }
        let body = read_bounded_response(response)?;
        let wire: ResourceRevisionPageWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported Resource history".to_owned())?;
        let items = wire.items.into_iter().map(|item| {
            if item.revision.resource_id != resource_id || item.revision.resource_revision_id.is_empty() {
                return Err("Local Runtime returned history for a different Resource".to_owned());
            }
            Ok(ResourceRevisionView {
                resource_revision_id: item.revision.resource_revision_id,
                resource_id: item.revision.resource_id,
                parent_revision_ids: item.revision.parent_revision_ids,
                content_digest: item.revision.content_digest,
                size_bytes: item.revision.size_bytes,
                media_type: item.revision.media_type,
                observed_at: item.revision.observed_at,
                is_head: item.is_head,
            })
        }).collect::<Result<Vec<_>, String>>()?;
        Ok(ResourceRevisionPageView { items, next_cursor: wire.next_cursor })
    }).await.map_err(|_| "Resource revision history request did not complete".to_owned())?
}

#[tauri::command]
async fn create_resource_revision_upload(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
    expected_resource_version: u64,
    parent_revision_ids: Vec<String>,
    media_type: String,
    size_bytes: u64,
    expected_digest: String,
    request_id: String,
) -> Result<ResourceUploadView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_resource_selection(&workspace_id, &resource_id)?;
        if expected_resource_version == 0 || parent_revision_ids.is_empty() || parent_revision_ids.len() > 16
            || parent_revision_ids.iter().any(|id| id.is_empty() || id.len() > 200)
            || parent_revision_ids.iter().collect::<std::collections::HashSet<_>>().len() != parent_revision_ids.len()
            || media_type.is_empty() || media_type.len() > 160 || size_bytes > 100 * 1024 * 1024
            || expected_digest.len() != 71 || !expected_digest.starts_with("sha256:")
            || !expected_digest[7..].bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || request_id.is_empty() || request_id.len() > 128
        {
            return Err("Resource revision selection is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client.post(format!("{}/resources/{}/revision-uploads", workspace_url.trim_end_matches("/workspaces"), resource_id))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_resource_version.to_string())
            .header("Idempotency-Key", request_id)
            .json(&serde_json::json!({
                "media_type": media_type,
                "size_bytes": size_bytes,
                "expected_digest": expected_digest,
                "parent_revision_ids": parent_revision_ids,
            }))
            .send().map_err(|_| "Resource revision upload could not be started".to_owned())?;
        if response.status() == 409 {
            return Err("RESOURCE_CONFLICT: Resource changed. Reload its current revision and history, then choose the file again to start a new upload.".to_owned());
        }
        if !response.is_success() { return Err("Resource revision upload could not be started".to_owned()); }
        let body = read_bounded_response(response)?;
        let wire: ResourceUploadWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported revision upload session".to_owned())?;
        if wire.workspace_id != workspace_id || wire.resource_id.as_deref() != Some(resource_id.as_str())
            || wire.expected_resource_version != Some(expected_resource_version)
            || wire.parent_revision_ids != parent_revision_ids || wire.media_type != media_type
            || wire.expected_size_bytes != size_bytes || wire.expected_digest.as_deref() != Some(expected_digest.as_str())
        {
            return Err("Local Runtime returned an upload session for a different revision request".to_owned());
        }
        Ok(ResourceUploadView::from(wire))
    }).await.map_err(|_| "Resource revision upload request did not complete".to_owned())?
}

#[tauri::command]
async fn commit_resource_revision_upload(
    app: AppHandle,
    workspace_id: String,
    upload_id: String,
    request_id: String,
) -> Result<ResourceView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_upload_scope(&workspace_id, &upload_id)?;
        if request_id.is_empty() || request_id.len() > 128 { return Err("Resource commit identity is invalid".to_owned()); }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client.post(format!("{}/resources/uploads/{}/commit", workspace_url.trim_end_matches("/workspaces"), upload_id))
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", request_id)
            .send().map_err(|_| "Resource revision could not be committed".to_owned())?;
        if response.status() == 409 {
            return Err("RESOURCE_CONFLICT: The Resource changed while this upload was in progress. No revision was committed. Reload history and explicitly choose the content to retry.".to_owned());
        }
        if !response.is_success() { return Err("Resource revision could not be committed".to_owned()); }
        let body = read_bounded_response(response)?;
        let committed: CommittedResourceWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported committed revision".to_owned())?;
        if committed.resource.workspace_id != workspace_id { return Err("Resource revision committed to a different Workspace".to_owned()); }
        Ok(ResourceView {
            resource_id: committed.resource.resource_id,
            workspace_id: committed.resource.workspace_id,
            resource_revision_id: committed.revision.resource_revision_id,
            display_name: committed.resource.display_name,
            media_type: committed.revision.media_type.unwrap_or_else(|| "application/octet-stream".to_owned()),
            content_digest: committed.revision.content_digest.unwrap_or_default(),
            size_bytes: committed.revision.size_bytes.unwrap_or_default(),
        })
    }).await.map_err(|_| "Resource revision commit request did not complete".to_owned())?
}

fn validate_resource_selection(workspace_id: &str, resource_id: &str) -> Result<(), String> {
    if workspace_id.is_empty() || resource_id.is_empty() || resource_id.len() > 160
        || !resource_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    { return Err("Resource selection is invalid".to_owned()); }
    Ok(())
}

#[tauri::command]
async fn import_resource(
    app: AppHandle,
    workspace_id: String,
    display_name: String,
    media_type: String,
    content_base64: String,
    request_id: String,
) -> Result<ResourceView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let display_name = display_name.trim();
        if workspace_id.is_empty() || display_name.is_empty() || display_name.len() > 240 {
            return Err("Resource selection is invalid".to_owned());
        }
        if content_base64.len() > 14 * 1024 * 1024 {
            return Err("Resource exceeds the 10 MiB local intake limit".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client
            .post(workspace_url.replace("/v1/workspaces", "/v1/resources/quick-import"))
            .header("X-Workspace-ID", workspace_id)
            .header("Idempotency-Key", request_id)
            .json(&serde_json::json!({
                "display_name": display_name,
                "media_type": media_type,
                "content_base64": content_base64,
            }))
            .send()
            .map_err(|_| "Local Resource intake did not complete".to_owned())?;
        let body = read_bounded_response(response)?;
        serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported Resource response".to_owned())
    })
    .await
    .map_err(|_| "Resource intake did not complete".to_owned())?
}

#[tauri::command]
async fn create_resource_upload(
    app: AppHandle,
    workspace_id: String,
    display_name: String,
    media_type: String,
    size_bytes: u64,
    expected_digest: String,
    folder_relative_path: Option<String>,
    context_document: Option<serde_json::Value>,
    request_id: String,
) -> Result<ResourceUploadView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let name = display_name.trim();
        if workspace_id.is_empty() || name.is_empty() || name.len() > 240 {
            return Err("Resource selection is invalid".to_owned());
        }
        if size_bytes > 100 * 1024 * 1024 {
            return Err("A Resource upload cannot exceed 100 MiB".to_owned());
        }
        if expected_digest.len() != 71
            || !expected_digest.starts_with("sha256:")
            || !expected_digest[7..].bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err("Resource upload requires a lowercase SHA-256 content digest".to_owned());
        }
        if folder_relative_path.as_deref().is_some_and(|path| path != name) {
            return Err("Folder import path must match the selected Resource display name".to_owned());
        }
        if let Some(metadata) = &context_document {
            let valid_workspace_notes = metadata.as_object().is_some_and(|object| {
                object.len() == 2
                    && metadata.get("kind").and_then(serde_json::Value::as_str) == Some("WORKSPACE_NOTES")
                    && metadata.get("owner_ref").and_then(serde_json::Value::as_object).is_some_and(|owner| {
                        owner.len() == 2
                            && owner.get("kind").and_then(serde_json::Value::as_str) == Some("WORKSPACE")
                            && owner.get("workspace_id").and_then(serde_json::Value::as_str) == Some(workspace_id.as_str())
                    })
            });
            if !valid_workspace_notes || folder_relative_path.is_some() {
                return Err("Only Workspace notes can be created here; choose Workspace notes without folder-import metadata".to_owned());
            }
        }
        if request_id.is_empty() || request_id.len() > 128 {
            return Err("Resource upload request identity is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let expected_folder_relative_path = folder_relative_path.clone();
        let expected_media_type = media_type.clone();
        let expected_digest = expected_digest.clone();
        let expected_context_document = context_document.clone();
        let mut upload_request = serde_json::json!({
            "workspace_id": workspace_id,
            "display_name": name,
            "media_type": media_type,
            "size_bytes": size_bytes,
            "expected_digest": expected_digest,
            "folder_import": folder_relative_path.map(|relative_path| serde_json::json!({ "relative_path": relative_path })),
        });
        if let Some(metadata) = &context_document {
            upload_request["context_document"] = metadata.clone();
        }
        let response = client
            .post(workspace_url.replace("/v1/workspaces", "/v1/resources/uploads"))
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", request_id)
            .json(&upload_request)
            .send()
            .map_err(|_| "Local Resource upload could not be started".to_owned())?;
        let body = read_bounded_response(response)?;
        let wire: ResourceUploadWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported upload session".to_owned())?;
        if wire.workspace_id != workspace_id
            || wire.display_name != name
            || wire.media_type != expected_media_type
            || wire.expected_size_bytes != size_bytes
            || wire.expected_digest.as_deref() != Some(expected_digest.as_str())
            || wire.folder_import.as_ref().map(|origin| origin.relative_path.as_str()) != expected_folder_relative_path.as_deref()
            || !resource_upload_context_metadata_matches(wire.context_document.as_ref(), expected_context_document.as_ref())
        {
            return Err("Local Runtime returned an upload session for a different request".to_owned());
        }
        Ok(ResourceUploadView::from(wire))
    })
    .await
    .map_err(|_| "Resource upload session request did not complete".to_owned())?
}

fn resource_upload_context_metadata_matches(
    actual: Option<&serde_json::Value>,
    expected: Option<&serde_json::Value>,
) -> bool {
    match (actual, expected) {
        (None, None) => true,
        (Some(actual), Some(expected)) => {
            actual.as_object().is_some_and(|object| object.len() == 2
                && object.contains_key("kind")
                && object.contains_key("owner_ref"))
                && expected.as_object().is_some_and(|object| object.len() == 2
                    && object.contains_key("kind")
                    && object.contains_key("owner_ref"))
                && actual.get("kind") == expected.get("kind")
                && actual.get("owner_ref") == expected.get("owner_ref")
        }
        _ => false,
    }
}

#[tauri::command]
async fn get_resource_upload(
    app: AppHandle,
    workspace_id: String,
    upload_id: String,
) -> Result<ResourceUploadView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_upload_scope(&workspace_id, &upload_id)?;
        let (client, workspace_url) = operator_client(&app)?;
        let response = client
            .get(format!(
                "{}/resources/uploads/{}",
                workspace_url.trim_end_matches("/workspaces"),
                upload_id
            ))
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "Local Resource upload progress is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let wire: ResourceUploadWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported upload progress".to_owned())?;
        if wire.workspace_id != workspace_id || wire.upload_id != upload_id {
            return Err("Local Runtime returned upload progress for a different request".to_owned());
        }
        Ok(ResourceUploadView::from(wire))
    })
    .await
    .map_err(|_| "Resource upload progress request did not complete".to_owned())?
}

#[tauri::command]
async fn upload_resource_chunk(
    app: AppHandle,
    workspace_id: String,
    upload_id: String,
    chunk_index: u64,
    content_range: String,
    chunk_sha256: String,
    content_base64: String,
    request_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_upload_scope(&workspace_id, &upload_id)?;
        if request_id.is_empty() || request_id.len() > 128 {
            return Err("Resource chunk request identity is invalid".to_owned());
        }
        if chunk_sha256.len() != 64
            || !chunk_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("Resource chunk digest is invalid".to_owned());
        }
        if content_base64.len() > 5_592_408 {
            return Err("Resource chunk encoding exceeds the 4 MiB transfer limit".to_owned());
        }
        let bytes = BASE64_STANDARD
            .decode(content_base64.as_bytes())
            .map_err(|_| "Resource chunk encoding is invalid".to_owned())?;
        if bytes.is_empty() || bytes.len() > 4 * 1024 * 1024 {
            return Err("Resource chunk must contain between 1 byte and 4 MiB".to_owned());
        }
        validate_content_range(&content_range, bytes.len())?;
        let (client, workspace_url) = operator_client(&app)?;
        let response = client
            .put(format!(
                "{}/resources/uploads/{}/chunks/{}",
                workspace_url.trim_end_matches("/workspaces"),
                upload_id,
                chunk_index
            ))
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", request_id)
            .header("Content-Range", content_range)
            .header("X-Chunk-SHA256", chunk_sha256.to_ascii_lowercase())
            .header("Content-Type", "application/octet-stream")
            .body(bytes)
            .send()
            .map_err(|_| "Resource chunk transfer did not complete".to_owned())?;
        if !response.is_success() {
            return Err("Local Runtime rejected this Resource chunk; progress is safe to resume".to_owned());
        }
        Ok(())
    })
    .await
    .map_err(|_| "Resource chunk transfer did not complete".to_owned())?
}

#[tauri::command]
async fn commit_resource_upload(
    app: AppHandle,
    workspace_id: String,
    upload_id: String,
    request_id: String,
) -> Result<ResourceView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_upload_scope(&workspace_id, &upload_id)?;
        if request_id.is_empty() || request_id.len() > 128 {
            return Err("Resource commit request identity is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client
            .post(format!(
                "{}/resources/uploads/{}/commit",
                workspace_url.trim_end_matches("/workspaces"),
                upload_id
            ))
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", request_id)
            .send()
            .map_err(|_| "Resource upload could not be committed".to_owned())?;
        let body = read_bounded_response(response)?;
        let committed: CommittedResourceWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported committed Resource".to_owned())?;
        if committed.resource.workspace_id != workspace_id {
            return Err("Local Runtime committed a Resource to a different Workspace".to_owned());
        }
        Ok(ResourceView {
            resource_id: committed.resource.resource_id,
            workspace_id: committed.resource.workspace_id,
            resource_revision_id: committed.revision.resource_revision_id,
            display_name: committed.resource.display_name,
            media_type: committed.revision.media_type.unwrap_or_else(|| "application/octet-stream".to_owned()),
            content_digest: committed.revision.content_digest.unwrap_or_default(),
            size_bytes: committed.revision.size_bytes.unwrap_or_default(),
        })
    })
    .await
    .map_err(|_| "Resource upload commit request did not complete".to_owned())?
}

fn validate_upload_scope(workspace_id: &str, upload_id: &str) -> Result<(), String> {
    if workspace_id.is_empty()
        || upload_id.is_empty()
        || upload_id.len() > 160
        || !upload_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err("Resource upload selection is invalid".to_owned());
    }
    Ok(())
}

fn validate_content_range(value: &str, body_size: usize) -> Result<(), String> {
    if value.len() > 128 {
        return Err("Resource chunk range is invalid".to_owned());
    }
    let Some(range) = value.strip_prefix("bytes ") else {
        return Err("Resource chunk range is invalid".to_owned());
    };
    let Some((offsets, total)) = range.split_once('/') else {
        return Err("Resource chunk range is invalid".to_owned());
    };
    let Some((start, end)) = offsets.split_once('-') else {
        return Err("Resource chunk range is invalid".to_owned());
    };
    let start = start.parse::<u64>().map_err(|_| "Resource chunk range is invalid".to_owned())?;
    let end = end.parse::<u64>().map_err(|_| "Resource chunk range is invalid".to_owned())?;
    let total = total.parse::<u64>().map_err(|_| "Resource chunk range is invalid".to_owned())?;
    if end < start || end.saturating_sub(start).saturating_add(1) != body_size as u64 || end >= total || total > 100 * 1024 * 1024 {
        return Err("Resource chunk range does not match its bounded body".to_owned());
    }
    Ok(())
}

#[tauri::command]
async fn list_resources(
    app: AppHandle,
    workspace_id: String,
    cursor: Option<String>,
) -> Result<ResourcePageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (client, workspace_url) = operator_client(&app)?;
        let url = workspace_url.replace("/v1/workspaces", "/v1/resources");
        let mut request = client
            .get(&url)
            .header("X-Workspace-ID", workspace_id)
            .query(&[("limit", "100")]);
        if let Some(cursor) = cursor {
            if cursor.is_empty() || cursor.len() > 2048 {
                return Err("Resource page cursor is invalid".to_owned());
            }
            request = request.query(&[("cursor", cursor)]);
        }
        let response = request
            .send()
            .map_err(|_| "Local Resource list is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: ResourceListWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported Resource list".to_owned())?;
        Ok(ResourcePageView {
            items: page.items,
            next_cursor: page.next_cursor,
        })
    })
    .await
    .map_err(|_| "Resource list request did not complete".to_owned())?
}

const MAX_RESOURCE_INDEX_MATCHES: usize = 32;
const MAX_INDEXED_RESOURCE_BYTES: u64 = 1_048_576;

fn valid_resource_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn indexed_query_terms(query: &str) -> Result<Vec<String>, String> {
    let mut terms = std::collections::BTreeSet::new();
    let mut current = String::new();
    for character in query.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            current.push(character);
            if current.chars().count() > 128 {
                return Err("Resource search query contains an overlong indexed term".to_owned());
            }
        } else if !current.is_empty() {
            terms.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        terms.insert(current);
    }
    if terms.is_empty() || terms.len() > MAX_RESOURCE_INDEX_MATCHES {
        return Err("Resource search query has an invalid indexed term count".to_owned());
    }
    Ok(terms.into_iter().collect())
}

fn map_resource_search_page(
    selected_workspace_id: &str,
    requested_mode: &str,
    query: &str,
    page: ResourceSearchPageWire,
) -> Result<ResourceSearchPageView, String> {
    if page.mode != requested_mode {
        return Err("Local Runtime returned a Resource search mode mismatch".to_owned());
    }
    let expected_terms = if requested_mode == "INDEXED_CONTENT" {
        Some(indexed_query_terms(query)?)
    } else {
        None
    };
    let items = page
        .items
        .into_iter()
        .map(|item| {
            if item.resource_ref.workspace_id != selected_workspace_id
                || item.resource_ref.resource_id.trim().is_empty()
            {
                return Err("Local Runtime returned a Resource search result outside the selected Workspace".to_owned());
            }
            let resource_revision_id = item
                .resource_ref
                .revision_id
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Local Runtime returned an unpinned Resource search result".to_owned())?;
            if !valid_resource_sha256(&item.source_content_digest) {
                return Err("Local Runtime returned an invalid Resource source digest".to_owned());
            }
            if item.source_matches.len() > MAX_RESOURCE_INDEX_MATCHES {
                return Err("Local Runtime returned too many Resource source matches".to_owned());
            }
            if let Some(expected_terms) = expected_terms.as_ref() {
                if !item.match_reasons.iter().any(|reason| reason == "CONTENT_INDEXED")
                    || item.source_matches.len() != expected_terms.len()
                {
                    return Err("Local Runtime returned incomplete indexed Resource source matches".to_owned());
                }
                let mut returned_terms = std::collections::BTreeSet::new();
                let mut ranges = Vec::with_capacity(item.source_matches.len());
                for source_match in &item.source_matches {
                    let term_chars = source_match.term.chars().count();
                    if term_chars == 0
                        || term_chars > 128
                        || !source_match.term.chars().all(char::is_alphanumeric)
                        || source_match.term.chars().flat_map(char::to_lowercase).collect::<String>() != source_match.term
                        || !returned_terms.insert(source_match.term.as_str())
                        || source_match.start_utf8_byte >= source_match.end_utf8_byte_exclusive
                        || source_match.end_utf8_byte_exclusive > MAX_INDEXED_RESOURCE_BYTES
                    {
                        return Err("Local Runtime returned an invalid or duplicate Resource source span".to_owned());
                    }
                    ranges.push((source_match.start_utf8_byte, source_match.end_utf8_byte_exclusive));
                }
                ranges.sort_unstable();
                if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0)
                    || returned_terms.iter().copied().ne(expected_terms.iter().map(String::as_str))
                {
                    return Err("Local Runtime returned overlapping or query-mismatched Resource source spans".to_owned());
                }
            } else if !item.source_matches.is_empty() {
                return Err("Local Runtime returned indexed source spans for a non-indexed Resource search".to_owned());
            }
            Ok(ResourceSearchResultView {
                resource_id: item.resource_ref.resource_id,
                resource_revision_id,
                source_content_digest: item.source_content_digest,
                source_matches: item
                    .source_matches
                    .into_iter()
                    .map(|source_match| ResourceTextMatchView {
                        term: source_match.term,
                        start_utf8_byte: source_match.start_utf8_byte,
                        end_utf8_byte_exclusive: source_match.end_utf8_byte_exclusive,
                    })
                    .collect(),
                display_name: item.display_name,
                freshness: item.freshness,
                match_reasons: item.match_reasons,
                snippet: item.snippet,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ResourceSearchPageView {
        items,
        next_cursor: page.next_cursor,
        mode: page.mode,
        content_scan: page.content_scan.map(|scan| ResourceContentScanView {
            candidates_scanned: scan.candidates_scanned,
            text_resources_checked: scan.text_resources_checked,
            skipped_unsupported_type: scan.skipped_unsupported_type,
            skipped_over_file_limit: scan.skipped_over_file_limit,
            skipped_revision_changed: scan.skipped_revision_changed,
            byte_budget_exhausted: scan.byte_budget_exhausted,
            candidate_budget_exhausted: scan.candidate_budget_exhausted,
            max_candidates: scan.max_candidates,
            max_file_bytes: scan.max_file_bytes,
            max_total_bytes: scan.max_total_bytes,
        }),
    })
}

#[tauri::command]
async fn search_resources(
    app: AppHandle,
    workspace_id: String,
    query: String,
    mode: String,
    kind: Option<String>,
    freshness: Option<String>,
    cursor: Option<String>,
) -> Result<ResourceSearchPageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.is_empty() || query.len() > 256 || query.contains('\0')
            || !matches!(mode.as_str(), "METADATA" | "ON_DEMAND_CONTENT" | "INDEXED_CONTENT")
            || kind.as_deref().is_some_and(|value| !matches!(value, "FILE" | "FOLDER" | "ARTIFACT" | "CONNECTOR_OBJECT" | "WEB_RESOURCE" | "OTHER"))
            || freshness.as_deref().is_some_and(|value| !matches!(value, "CURRENT" | "STALE" | "UNKNOWN" | "UNAVAILABLE")) {
            return Err("Resource search query is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let url = workspace_url.replace("/v1/workspaces", "/v1/resources/search");
        let mut request = client
            .get(&url)
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("q", query.as_str()), ("mode", mode.as_str()), ("limit", "100")]);
        if let Some(kind) = kind.as_deref() {
            request = request.query(&[("kind", kind)]);
        }
        if let Some(freshness) = freshness.as_deref() {
            request = request.query(&[("freshness", freshness)]);
        }
        if let Some(cursor) = cursor {
            if cursor.is_empty() || cursor.len() > 4096 {
                return Err("Resource search cursor is invalid".to_owned());
            }
            request = request.query(&[("cursor", cursor)]);
        }
        let response = request.send().map_err(|_| "Local Resource search is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: ResourceSearchPageWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported Resource search page".to_owned())?;
        map_resource_search_page(&workspace_id, &mode, &query, page)
    })
    .await
    .map_err(|_| "Resource search request did not complete".to_owned())?
}

#[tauri::command]
async fn rebuild_resource_text_index(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
    resource_revision_id: String,
    content_digest: String,
    request_id: String,
) -> Result<ResourceTextIndexRebuildView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid_id = |value: &str| {
            !value.is_empty()
                && value.len() <= 160
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        };
        if workspace_id.is_empty()
            || !valid_id(&resource_id)
            || !valid_id(&resource_revision_id)
            || content_digest.len() != 71
            || !content_digest.starts_with("sha256:")
            || !content_digest[7..].bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || request_id.is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err("Resource index rebuild selection is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let url = workspace_url.replace(
            "/v1/workspaces",
            &format!("/v1/resources/{resource_id}/text-index/rebuild"),
        );
        let response = client
            .post(&url)
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", &request_id)
            .json(&serde_json::json!({
                "resource_revision_id": resource_revision_id,
                "content_digest": content_digest,
            }))
            .send()
            .map_err(|_| "Local Resource index rebuild is unavailable".to_owned())?;
        let body = read_bounded_response(response).map_err(|error| {
            if error.contains("[CONFLICT;") {
                "The Resource head, index eligibility, or request ID changed. Reload the Library before retrying the index rebuild.".to_owned()
            } else {
                error
            }
        })?;
        let wire: ResourceTextIndexRebuildWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported Resource index result".to_owned())?;
        let valid_result = wire.request_id == request_id
            && wire.workspace_id == workspace_id
            && wire.resource_id == resource_id
            && wire.resource_revision_id == resource_revision_id
            && wire.content_digest == content_digest
            && !wire.correlation_id.is_empty()
            && wire.correlation_id.len() <= 128
            && wire.correlation_id.bytes().all(|byte| byte.is_ascii_graphic())
            && match (wire.outcome.as_str(), wire.reason.as_deref()) {
                ("INDEXED", None) => true,
                ("NOT_INDEXABLE", Some(reason)) => matches!(
                    reason,
                    "UNSUPPORTED_TYPE" | "OVER_SIZE_LIMIT" | "INVALID_UTF8"
                        | "CONTROL_CHARACTERS" | "TERM_LIMIT_EXCEEDED"
                ),
                _ => false,
            };
        if !valid_result {
            return Err("Local Runtime returned an invalid Resource index result".to_owned());
        }
        Ok(ResourceTextIndexRebuildView {
            request_id: wire.request_id,
            correlation_id: wire.correlation_id,
            workspace_id: wire.workspace_id,
            resource_id: wire.resource_id,
            resource_revision_id: wire.resource_revision_id,
            content_digest: wire.content_digest,
            outcome: wire.outcome,
            reason: wire.reason,
        })
    })
    .await
    .map_err(|_| "Resource index rebuild request did not complete".to_owned())?
}

#[tauri::command]
async fn preview_resource_text(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
    revision_id: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.is_empty()
            || resource_id.is_empty()
            || resource_id.len() > 160
            || !resource_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || revision_id.is_empty()
            || revision_id.len() > 160
            || !revision_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err("Resource selection is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client
            .get(format!(
                "{}/resources/{}/content",
                workspace_url.trim_end_matches("/workspaces"),
                resource_id
            ))
            .header("X-Workspace-ID", workspace_id)
            .query(&[("revision_id", revision_id.as_str()), ("max_bytes", "1048576")])
            .send()
            .map_err(|_| "Local Resource content is unavailable".to_owned())?;
        if !response.is_success() {
            return Err(read_bounded_response(response).err().unwrap_or_else(|| "Local Runtime could not read this Resource revision".to_owned()));
        }
        if response.header("x-resource-revision-id") != Some(revision_id.as_str()) {
            return Err("Local Runtime returned content for a different Resource revision".to_owned());
        }
        let media_type = response.header("x-resource-media-type").unwrap_or("");
        let normalized_media_type = media_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        let is_text = normalized_media_type.starts_with("text/")
            || matches!(normalized_media_type.as_str(), "application/json" | "application/xml" | "application/yaml" | "application/x-yaml" | "application/javascript");
        if !is_text {
            return Err("Preview is available for text files only".to_owned());
        }
        if response
            .content_length()
            .is_some_and(|length| length > 1024 * 1024)
        {
            return Err("Text preview is limited to 1 MiB".to_owned());
        }
        let mut bytes = Vec::new();
        response
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Resource text preview could not be read".to_owned())?;
        if bytes.len() > 1024 * 1024 {
            return Err("Text preview is limited to 1 MiB".to_owned());
        }
        String::from_utf8(bytes).map_err(|_| "This text file is not valid UTF-8".to_owned())
    })
    .await
    .map_err(|_| "Resource preview request did not complete".to_owned())?
}

fn verify_resource_preview_bytes(
    bytes: Vec<u8>,
    response_revision_id: Option<&str>,
    expected_revision_id: &str,
    expected_content_digest: &str,
) -> Result<ResourceTextPreviewWithProvenanceView, String> {
    if response_revision_id != Some(expected_revision_id) {
        return Err("Local Runtime returned content for a different Resource revision".to_owned());
    }
    if !valid_resource_sha256(expected_content_digest) {
        return Err("The indexed Resource content digest is invalid".to_owned());
    }
    use sha2::{Digest, Sha256};
    let actual_digest = format!("sha256:{:x}", Sha256::digest(&bytes));
    if actual_digest != expected_content_digest {
        return Err("Resource content no longer matches the indexed search result".to_owned());
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| "This text file is not valid UTF-8".to_owned())?;
    Ok(ResourceTextPreviewWithProvenanceView {
        text,
        resource_revision_id: expected_revision_id.to_owned(),
        content_digest: actual_digest,
    })
}

/// Search-derived spans must use this exact-revision, digest-verified preview path.
/// Ordinary Library previews continue to use `preview_resource_text` above.
#[tauri::command]
async fn preview_resource_text_with_provenance(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
    revision_id: String,
    expected_content_digest: String,
) -> Result<ResourceTextPreviewWithProvenanceView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.is_empty()
            || resource_id.is_empty()
            || resource_id.len() > 160
            || !resource_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || revision_id.is_empty()
            || revision_id.len() > 160
            || !revision_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            || !valid_resource_sha256(&expected_content_digest)
        {
            return Err("Resource search preview selection is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let response = client
            .get(format!(
                "{}/resources/{}/content",
                workspace_url.trim_end_matches("/workspaces"),
                resource_id
            ))
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("revision_id", revision_id.as_str()), ("max_bytes", "1048576")])
            .send()
            .map_err(|_| "Local Resource content is unavailable".to_owned())?;
        if !response.is_success() {
            return Err(read_bounded_response(response).err().unwrap_or_else(|| "Local Runtime could not read this Resource revision".to_owned()));
        }
        let response_revision_id = response.header("x-resource-revision-id").map(str::to_owned);
        if response_revision_id.as_deref() != Some(revision_id.as_str()) {
            return Err("Local Runtime returned content for a different Resource revision".to_owned());
        }
        let media_type = response.header("x-resource-media-type").unwrap_or("");
        let normalized_media_type = media_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        let is_text = normalized_media_type.starts_with("text/")
            || matches!(normalized_media_type.as_str(), "application/json" | "application/xml" | "application/yaml" | "application/x-yaml" | "application/javascript");
        if !is_text {
            return Err("Preview is available for text files only".to_owned());
        }
        if response.content_length().is_some_and(|length| length > 1024 * 1024) {
            return Err("Text preview is limited to 1 MiB".to_owned());
        }
        let mut bytes = Vec::new();
        response
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Resource text preview could not be read".to_owned())?;
        if bytes.len() > 1024 * 1024 {
            return Err("Text preview is limited to 1 MiB".to_owned());
        }
        verify_resource_preview_bytes(
            bytes,
            response_revision_id.as_deref(),
            &revision_id,
            &expected_content_digest,
        )
    })
    .await
    .map_err(|_| "Resource preview request did not complete".to_owned())?
}

fn fetch_workspaces(app: &AppHandle) -> Result<Vec<WorkspaceView>, String> {
    let (client, url) = operator_client(app)?;
    let response = client
        .get(url)
        .send()
        .map_err(|_| "Local Operator API is not responding".to_owned())?;
    let body = read_bounded_response(response)?;
    let response: WorkspaceListWire = serde_json::from_slice(&body)
        .map_err(|_| "Local Operator API returned an unsupported Workspace response".to_owned())?;
    Ok(response
        .items
        .into_iter()
        .map(WorkspaceView::from)
        .collect())
}

fn create_workspace_request(
    app: &AppHandle,
    name: String,
    request_id: String,
) -> Result<WorkspaceView, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 160 {
        return Err("Workspace name must contain 1 to 160 bytes".to_owned());
    }
    if request_id.is_empty()
        || request_id.len() > 128
        || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err("Workspace request identifier is invalid".to_owned());
    }
    let (client, url) = operator_client(app)?;
    let response = client
        .post(url)
        .header("Idempotency-Key", request_id)
        .json(&serde_json::json!({ "name": name }))
        .send()
        .map_err(|_| "Local Operator API is not responding".to_owned())?;
    if !response.is_success() {
        return Err("Workspace could not be created by the local Runtime".to_owned());
    }
    let body = read_bounded_response(response)?;
    let response: WorkspaceWire = serde_json::from_slice(&body)
        .map_err(|_| "Local Operator API returned an unsupported Workspace response".to_owned())?;
    Ok(WorkspaceView::from(response))
}

fn operator_client(app: &AppHandle) -> Result<(LocalOperatorClient, String), String> {
    let (client, _) = validated_operator_connection(app, None, None)?;
    Ok((client, "/v1/workspaces".to_owned()))
}

fn validated_operator_connection(
    app: &AppHandle,
    expected_runtime_id: Option<&str>,
    expected_incarnation_id: Option<&str>,
) -> Result<(LocalOperatorClient, OperatorReadinessWire), String> {
    validated_operator_connection_until(
        app,
        expected_runtime_id,
        expected_incarnation_id,
        Instant::now() + OPERATOR_READINESS_PROBE_TIMEOUT,
    )
}

fn validated_operator_connection_until(
    app: &AppHandle,
    expected_runtime_id: Option<&str>,
    expected_incarnation_id: Option<&str>,
    deadline: Instant,
) -> Result<(LocalOperatorClient, OperatorReadinessWire), String> {
    let client = LocalOperatorClient::new(app)?;
    let response = client
        .get("/v1/operator/readiness")
        .send_with_deadline(deadline.min(Instant::now() + OPERATOR_READINESS_PROBE_TIMEOUT))
        .map_err(|_| "Local Operator API is not ready".to_owned())?;
    let body = read_bounded_response(response)?;
    let readiness: OperatorReadinessWire = serde_json::from_slice(&body)
        .map_err(|_| "Local Operator API returned an unsupported readiness response".to_owned())?;
    if readiness.operator_state != "SERVING" || readiness.api_contract_version != 1 {
        return Err("Local Operator API is not ready".to_owned());
    }
    if expected_runtime_id.is_some_and(|id| id != readiness.runtime_id)
        || expected_incarnation_id.is_some_and(|id| id != readiness.local_incarnation_id)
    {
        return Err("Local Operator connection does not match the current daemon status".to_owned());
    }
    Ok((client, readiness))
}

/// Native-only Operator client. It derives the endpoint from Tauri's trusted data
/// directory, authenticates the daemon's Unix peer UID before writing any frame,
/// and performs one framed request per connection. No endpoint or handle crosses
/// into the WebView.
struct LocalOperatorClient {
    #[cfg(unix)]
    endpoint: UnixEndpoint,
    #[cfg(unix)]
    expected_peer_uid: u32,
}

impl LocalOperatorClient {
    fn new(app: &AppHandle) -> Result<Self, String> {
        operator_ipc::require_os_local_ipc()
            .map_err(|_| "Authenticated local Operator IPC is unsupported on this platform".to_owned())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|_| "LiteCowork application data directory is unavailable".to_owned())?;
            let endpoint = UnixEndpoint::derive(&data_dir)
                .map_err(|_| "Local Operator IPC data directory is unavailable or unsafe".to_owned())?;
            let metadata = fs::symlink_metadata(endpoint.private_data_dir())
                .map_err(|_| "Local Operator IPC data directory is unavailable".to_owned())?;
            Ok(Self {
                endpoint,
                expected_peer_uid: metadata.uid(),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = app;
            Err("Authenticated local Operator IPC is unsupported on this platform".to_owned())
        }
    }

    fn get(&self, path: impl Into<String>) -> LocalOperatorRequest {
        LocalOperatorRequest::new(self, "GET", path.into())
    }

    fn post(&self, path: impl Into<String>) -> LocalOperatorRequest {
        LocalOperatorRequest::new(self, "POST", path.into())
    }

    fn put(&self, path: impl Into<String>) -> LocalOperatorRequest {
        LocalOperatorRequest::new(self, "PUT", path.into())
    }

    fn patch(&self, path: impl Into<String>) -> LocalOperatorRequest {
        LocalOperatorRequest::new(self, "PATCH", path.into())
    }

    #[cfg(unix)]
    fn exchange(
        &self,
        method: String,
        path_and_query: String,
        headers: Vec<LogicalHeader>,
        body: Vec<u8>,
    ) -> Result<LocalOperatorResponse, String> {
        self.exchange_with_deadline(method, path_and_query, headers, body, None)
    }

    #[cfg(unix)]
    fn exchange_with_deadline(
        &self,
        method: String,
        path_and_query: String,
        headers: Vec<LogicalHeader>,
        body: Vec<u8>,
        deadline: Option<Instant>,
    ) -> Result<LocalOperatorResponse, String> {
        let request_id = format!(
            "tauri-{}-{}",
            std::process::id(),
            OPERATOR_REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let body_length = u64::try_from(body.len())
            .map_err(|_| "Local Operator request exceeds its size limit".to_owned())?;
        let body_budget = operator_ipc_body_budget();
        let frame = RequestFrame::new(
            RequestHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: request_id.clone(),
                method,
                path_and_query,
                headers,
                body_length,
            },
            body,
            &body_budget,
        )
        .map_err(|_| "Local Operator request is invalid or exceeds its size limit".to_owned())?;

        let endpoint = self.endpoint.clone();
        let expected_peer_uid = self.expected_peer_uid;
        let budget = Arc::clone(&body_budget);
        let runtime = operator_ipc_runtime()?;
        let exchange = async move {
            // The callback runs after SO_PEERCRED capture and before frame IO.
            let mut connection = endpoint
                .connect(|peer: PeerCredentials| peer.uid == expected_peer_uid)
                .await
                .map_err(|_| "Local Operator API is not responding".to_owned())?;
            connection
                .write_request(&frame)
                .await
                .map_err(|_| "Local Operator request could not be sent".to_owned())?;
            connection
                .read_response(&budget)
                .await
                .map_err(|_| "Local Operator response could not be read".to_owned())
        };
        let response = match deadline {
            Some(deadline) => runtime
                .block_on(async move {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err("Local Operator readiness deadline elapsed".to_owned());
                    }
                    tokio::time::timeout(remaining, exchange)
                        .await
                        .map_err(|_| "Local Operator readiness deadline elapsed".to_owned())?
                })?,
            None => runtime.block_on(exchange)?,
        };
        let (header, body) = response.into_parts();
        Ok(LocalOperatorResponse {
            status: header.status,
            headers: header.headers,
            body,
            body_offset: 0,
        })
    }

    #[cfg(not(unix))]
    fn exchange(
        &self,
        _method: String,
        _path_and_query: String,
        _headers: Vec<LogicalHeader>,
        _body: Vec<u8>,
    ) -> Result<LocalOperatorResponse, String> {
        Err("Authenticated local Operator IPC is unsupported on this platform".to_owned())
    }

    #[cfg(not(unix))]
    fn exchange_with_deadline(
        &self,
        method: String,
        path_and_query: String,
        headers: Vec<LogicalHeader>,
        body: Vec<u8>,
        _deadline: Option<Instant>,
    ) -> Result<LocalOperatorResponse, String> {
        self.exchange(method, path_and_query, headers, body)
    }
}

#[cfg(unix)]
fn operator_ipc_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    match OPERATOR_IPC_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|_| "Local Operator IPC client could not start".to_owned())
    }) {
        Ok(runtime) => Ok(runtime),
        Err(error) => Err(error.clone()),
    }
}

#[cfg(unix)]
fn operator_ipc_body_budget() -> Arc<InFlightBodyBudget> {
    Arc::clone(OPERATOR_IPC_BODY_BUDGET.get_or_init(|| {
        InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
            .expect("the fixed Operator IPC body budget is within protocol limits")
    }))
}

struct LocalOperatorRequest<'a> {
    client: &'a LocalOperatorClient,
    method: String,
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<LogicalHeader>,
    body: Vec<u8>,
    serialization_error: Option<String>,
}

impl<'a> LocalOperatorRequest<'a> {
    fn new(client: &'a LocalOperatorClient, method: &str, path: String) -> Self {
        Self {
            client,
            method: method.to_owned(),
            path,
            query: Vec::new(),
            headers: Vec::new(),
            body: Vec::new(),
            serialization_error: None,
        }
    }

    fn header(mut self, name: impl AsRef<str>, value: impl ToString) -> Self {
        self.headers.push(LogicalHeader {
            name: name.as_ref().to_owned(),
            value: value.to_string(),
        });
        self
    }

    fn query<K, V>(mut self, pairs: &[(K, V)]) -> Self
    where
        K: AsRef<str>,
        V: ToString,
    {
        self.query.extend(
            pairs
                .iter()
                .map(|(key, value)| (key.as_ref().to_owned(), value.to_string())),
        );
        self
    }

    fn json<T: serde::Serialize + ?Sized>(mut self, value: &T) -> Self {
        match bounded_json_bytes(value, MAX_BODY_BYTES) {
            Ok(body) => self.body = body,
            Err(_) => self.serialization_error = Some(
                "Local Operator request could not be encoded or exceeds its size limit".to_owned(),
            ),
        }
        if !self.headers.iter().any(|header| header.name.eq_ignore_ascii_case("content-type")) {
            self.headers.push(LogicalHeader {
                name: "Content-Type".to_owned(),
                value: "application/json".to_owned(),
            });
        }
        self
    }

    fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    fn send(self) -> Result<LocalOperatorResponse, String> {
        self.send_with_deadline_option(None)
    }

    fn send_with_deadline(
        self,
        deadline: Instant,
    ) -> Result<LocalOperatorResponse, String> {
        self.send_with_deadline_option(Some(deadline))
    }

    fn send_with_deadline_option(
        self,
        deadline: Option<Instant>,
    ) -> Result<LocalOperatorResponse, String> {
        if let Some(error) = self.serialization_error {
            return Err(error);
        }
        let path_and_query = append_query(&self.path, &self.query)?;
        self.client.exchange_with_deadline(
            self.method,
            path_and_query,
            self.headers,
            self.body,
            deadline,
        )
    }
}

struct BoundedJsonWriter {
    bytes: Vec<u8>,
    maximum_bytes: usize,
}

impl Write for BoundedJsonWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next_length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "JSON body too large"))?;
        if next_length > self.maximum_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "JSON body too large",
            ));
        }
        self.bytes.try_reserve(bytes.len()).map_err(|_| {
            io::Error::new(io::ErrorKind::OutOfMemory, "JSON body allocation failed")
        })?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn bounded_json_bytes<T: serde::Serialize + ?Sized>(
    value: &T,
    maximum_bytes: usize,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut writer = BoundedJsonWriter {
        bytes: Vec::new(),
        maximum_bytes,
    };
    serde_json::to_writer(&mut writer, value)?;
    Ok(writer.bytes)
}

fn append_query(path: &str, query: &[(String, String)]) -> Result<String, String> {
    if query.is_empty() {
        return Ok(path.to_owned());
    }
    if path.contains('?') {
        return Err("Local Operator request path is invalid".to_owned());
    }
    let mut output = path.to_owned();
    for (index, (key, value)) in query.iter().enumerate() {
        output.push(if index == 0 { '?' } else { '&' });
        append_query_component(&mut output, key);
        output.push('=');
        append_query_component(&mut output, value);
    }
    Ok(output)
}

fn append_query_component(output: &mut String, value: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(char::from(byte));
        } else if byte == b' ' {
            output.push('+');
        } else {
            output.push('%');
            output.push(char::from(HEX[(byte >> 4) as usize]));
            output.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
}

struct LocalOperatorResponse {
    status: u16,
    headers: Vec<LogicalHeader>,
    body: BudgetedBody,
    body_offset: usize,
}

impl LocalOperatorResponse {
    fn status(&self) -> u16 {
        self.status
    }

    fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }

    fn content_length(&self) -> Option<u64> {
        self.header("content-length")?.parse().ok()
    }
}

impl Read for LocalOperatorResponse {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let bytes = self.body.as_ref();
        let remaining = &bytes[self.body_offset..];
        let count = remaining.len().min(buffer.len());
        buffer[..count].copy_from_slice(&remaining[..count]);
        self.body_offset += count;
        Ok(count)
    }
}

fn read_bounded_response(response: LocalOperatorResponse) -> Result<Vec<u8>, String> {
    let status = response.status();
    let mut body = Vec::new();
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut body)
        .map_err(|_| "Local Operator API response could not be read".to_owned())?;
    if body.len() > 1024 * 1024 {
        return Err("Local Operator API response exceeded its size limit".to_owned());
    }
    if !(200..300).contains(&status) {
        #[derive(serde::Deserialize)]
        struct ApiErrorBody {
            code: String,
            message: String,
            retryable: bool,
            correlation_id: String,
            details: Option<serde_json::Value>,
        }
        if let Ok(error) = serde_json::from_slice::<ApiErrorBody>(&body) {
            if !error.message.trim().is_empty() {
                let code_is_safe = !error.code.is_empty()
                    && error.code.len() <= 64
                    && error.code.bytes().all(|byte| {
                        byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
                    });
                let correlation_is_safe = !error.correlation_id.is_empty()
                    && error.correlation_id.len() <= 128
                    && error
                        .correlation_id
                        .bytes()
                        .all(|byte| byte.is_ascii_graphic());
                if code_is_safe && correlation_is_safe {
                    return Err(format!(
                        "{} [{}; retryable={}; reference={}]",
                        error.message.trim(),
                        error.code,
                        error.retryable,
                        error.correlation_id
                    ));
                }
                return Err(error.message.trim().to_owned());
            }
            let _ = error.details; // Details may contain internal context; keep them native-only.
        }
        return Err(format!("Local Operator API returned HTTP {status}"));
    }
    Ok(body)
}

#[tauri::command]
async fn start_local_runtime(app: AppHandle) -> Result<RuntimeStatus, String> {
    tauri::async_runtime::spawn_blocking(move || start_runtime(&app))
        .await
        .map_err(|_| "Runtime startup operation did not complete".to_owned())?
}

fn start_runtime(app: &AppHandle) -> Result<RuntimeStatus, String> {
    let deadline = Instant::now() + RUNTIME_STARTUP_DEADLINE;
    let state_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "LiteCowork application data directory is unavailable".to_owned())?;
    let daemon = daemon_executable(app)?;
    #[cfg(target_os = "linux")]
    {
        let config_dir = app
            .path()
            .config_dir()
            .map_err(|_| "LiteCowork user configuration directory is unavailable".to_owned())?;
        systemd_user_service::start(&config_dir, &daemon, &state_dir, deadline)?;
    }
    let initial = runtime_status_until(app, deadline);
    if initial.process_running && initial.operator_ready {
        #[cfg(target_os = "linux")]
        systemd_user_service::verify_active(
            &app.path().config_dir().map_err(|_| "LiteCowork user configuration directory is unavailable".to_owned())?,
            &daemon,
            &state_dir,
            deadline,
        )?;
        return Ok(initial);
    }
    if Instant::now() >= deadline {
        return Err("LiteCowork Runtime startup status probe exceeded its deadline".to_owned());
    }

    let mut launched_child: Option<std::process::Child> = if !initial.process_running {
        #[cfg(target_os = "linux")]
        {
            None
        }
        #[cfg(not(target_os = "linux"))]
        {
        Some(Command::new(daemon)
            .arg("run")
            .arg("--data-dir")
            .arg(&state_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "LiteCowork Runtime could not be started".to_owned())?)
        }
    } else {
        None
    };

    loop {
        // Keep the process handle only through startup. This lets the Operator
        // distinguish a daemon that failed immediately from one that is still
        // recovering; dropping a live Child does not terminate the independent
        // Runtime after readiness succeeds.
        let child_exited = match launched_child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(_)) => true,
                Ok(None) => false,
                Err(_) => {
                    return Err("LiteCowork Runtime startup process could not be observed".to_owned());
                }
            },
            None => false,
        };
        if child_exited {
            launched_child.take();
            let status = runtime_status_until(app, deadline);
            if status.process_running && status.operator_ready {
                return Ok(status);
            }
            if !status.process_running {
                return Err("LiteCowork Runtime exited before its authenticated Operator API became ready".to_owned());
            }
            // Another process may have won the single-instance race after the
            // initial status probe. Continue observing that lock owner rather than
            // misreporting its in-progress startup as a failure.
        }
        let status = runtime_status_until(app, deadline);
        if status.process_running && status.operator_ready {
            #[cfg(target_os = "linux")]
            systemd_user_service::verify_active(
                &app.path().config_dir().map_err(|_| "LiteCowork user configuration directory is unavailable".to_owned())?,
                &daemon,
                &state_dir,
                deadline,
            )?;
            // The Runtime has its own single-instance lock and lifecycle. The
            // desktop intentionally does not retain process ownership after the
            // authenticated readiness handshake.
            drop(launched_child.take());
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err("LiteCowork Runtime process is present, but its authenticated Operator API did not become ready in time".to_owned());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn runtime_status_until(app: &AppHandle, deadline: Instant) -> RuntimeStatus {
    if Instant::now() >= deadline {
        return unavailable_runtime_status("Runtime status probe deadline elapsed", true);
    }
    let state_dir = match app.path().app_data_dir() {
        Ok(path) => path,
        Err(_) => {
            return RuntimeStatus {
                state: "UNAVAILABLE".to_owned(),
                runtime_id: None,
                local_incarnation_id: None,
                blockers: Vec::new(),
                last_shutdown_clean: None,
                process_running: false,
                operator_ready: false,
                daemon_available: false,
                detail: Some("Application data directory is unavailable".to_owned()),
            };
        }
    };
    let daemon = match daemon_executable(app) {
        Ok(path) => path,
        Err(error) => {
            return RuntimeStatus {
                state: "UNAVAILABLE".to_owned(),
                runtime_id: None,
                local_incarnation_id: None,
                blockers: Vec::new(),
                last_shutdown_clean: None,
                process_running: false,
                operator_ready: false,
                daemon_available: false,
                detail: Some(error),
            };
        }
    };

    let command_deadline = (Instant::now() + STATUS_COMMAND_PROBE_TIMEOUT).min(deadline);
    let status_bytes = match run_bounded_status_command(daemon, state_dir, command_deadline) {
        Ok(bytes) => bytes,
        Err(_) => return unavailable_runtime_status("Runtime status could not be read", true),
    };
    match serde_json::from_slice::<RuntimeStatusWire>(&status_bytes) {
        Ok(status) => {
            let process_running = status.process_running.unwrap_or(false);
            let operator_ready = process_running
                && probe_operator_readiness_until(
                    app,
                    status.runtime_id.as_deref(),
                    status.local_incarnation_id.as_deref(),
                    deadline,
                );
            RuntimeStatus {
                state: status.state,
                runtime_id: status.runtime_id,
                local_incarnation_id: status.local_incarnation_id,
                blockers: status.blockers.unwrap_or_default(),
                last_shutdown_clean: status.last_shutdown_clean,
                process_running,
                operator_ready,
                daemon_available: true,
                detail: (process_running && !operator_ready).then(|| {
                    "Runtime process is present, but authenticated Operator IPC is unavailable"
                        .to_owned()
                }),
            }
        }
        Err(_) => unavailable_runtime_status("Runtime returned an unsupported status document", true),
    }
}

fn unavailable_runtime_status(detail: &str, daemon_available: bool) -> RuntimeStatus {
    RuntimeStatus {
        state: "UNAVAILABLE".to_owned(),
        runtime_id: None,
        local_incarnation_id: None,
        blockers: Vec::new(),
        last_shutdown_clean: None,
        process_running: false,
        operator_ready: false,
        daemon_available,
        detail: Some(detail.to_owned()),
    }
}

fn run_bounded_status_command(
    daemon: PathBuf,
    state_dir: PathBuf,
    deadline: Instant,
) -> Result<Vec<u8>, String> {
    if Instant::now() >= deadline {
        return Err("Runtime status deadline elapsed".to_owned());
    }
    let mut child = Command::new(daemon)
        .arg("status")
        .arg("--data-dir")
        .arg(state_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Runtime status process could not start".to_owned())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Runtime status output is unavailable".to_owned())?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    if thread::Builder::new()
        .name("litecowork-runtime-status-output".to_owned())
        .spawn(move || {
            let mut output = Vec::new();
            let mut reader = stdout.take((MAX_RUNTIME_STATUS_BYTES + 1) as u64);
            let result = reader
                .read_to_end(&mut output)
                .map(|_| output)
                .map_err(|_| "Runtime status output could not be read".to_owned());
            let _ = sender.send(result);
        })
        .is_err()
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Runtime status output reader could not start".to_owned());
    }

    let exit_status = loop {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Runtime status command exceeded its deadline".to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                thread::sleep(Duration::from_millis(10).min(remaining));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Runtime status process could not be observed".to_owned());
            }
        }
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    let output = receiver
        .recv_timeout(remaining)
        .map_err(|_| "Runtime status output exceeded its deadline".to_owned())??;
    if !exit_status.success() || output.len() > MAX_RUNTIME_STATUS_BYTES {
        return Err("Runtime status response is invalid or oversized".to_owned());
    }
    Ok(output)
}

fn probe_operator_readiness_until(
    app: &AppHandle,
    expected_runtime_id: Option<&str>,
    expected_incarnation_id: Option<&str>,
    deadline: Instant,
) -> bool {
    let (Some(expected_runtime_id), Some(expected_incarnation_id)) =
        (expected_runtime_id, expected_incarnation_id)
    else {
        return false;
    };
    validated_operator_connection_until(
        app,
        Some(expected_runtime_id),
        Some(expected_incarnation_id),
        deadline,
    )
    .is_ok()
}

#[derive(serde::Deserialize)]
struct RuntimeStatusWire {
    state: String,
    runtime_id: Option<String>,
    local_incarnation_id: Option<String>,
    blockers: Option<Vec<String>>,
    last_shutdown_clean: Option<bool>,
    #[serde(default)]
    process_running: Option<bool>,
}

fn daemon_executable(app: &AppHandle) -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("LITECOWORKD_PATH") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
        return Err("Configured LiteCowork Runtime executable is unavailable".to_owned());
    }

    let filename = if cfg!(windows) {
        "litecoworkd.exe"
    } else {
        "litecoworkd"
    };
    let mut candidates = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("binaries").join(filename));
        candidates.push(resource_dir.join(filename));
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join(filename));
        }
    }
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|directory| directory.join(filename)));
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| "LiteCowork Runtime executable was not found".to_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_runtime_status,
            list_agent_installations,
            list_agent_profiles,
            probe_agent_profile,
            list_agent_bindings,
            create_agent_binding,
            enable_agent_binding,
            list_workspace_runtime_bindings,
            enroll_local_runtime,
            list_workspaces,
            list_workspace_roots,
            add_workspace_root,
            revoke_workspace_root,
            pause_workspace_root,
            resume_workspace_root,
            list_tasks,
            create_task,
            get_task,
            list_task_spec_revisions,
            get_task_planning_readiness,
            revise_task_spec,
            create_workspace,
            update_workspace_policy,
            set_workspace_default_agent_binding,
            list_workspace_instructions,
            create_workspace_instruction_revision,
            get_resource_detail,
            set_context_document_status,
            list_resource_revisions,
            create_resource_revision_upload,
            commit_resource_revision_upload,
            import_resource,
            create_resource_upload,
            get_resource_upload,
            upload_resource_chunk,
            commit_resource_upload,
            list_resources,
            search_resources,
            rebuild_resource_text_index,
            preview_resource_text,
            preview_resource_text_with_provenance,
            artifact_bridge::artifact_read,
            artifact_bridge::artifact_library_command,
            conversation_bridge::list_conversations,
            conversation_bridge::create_conversation,
            conversation_bridge::get_conversation_presentation,
            conversation_bridge::get_rich_presentation,
            artifact_bridge::artifact_save_as,
            resource_save_bridge::resource_save_as,
            artifact_bridge::artifact_edit_head,
            artifact_bridge::artifact_append_text_version,
            automation_bridge::automation_request,
            coworker_bridge::coworker_request,
            delegation_profile_bridge::list_delegation_profiles,
            delegation_profile_write_bridge::create_delegation_profile,
            delegation_profile_write_bridge::revise_delegation_profile,
            delegation_profile_write_bridge::duplicate_delegation_profile,
            delegation_profile_write_bridge::disable_delegation_profile,
            delegation_profile_write_bridge::archive_delegation_profile,
            goal_bridge::goal_request,
            suggestions_bridge::list_suggestions,
            suggestions_bridge::suggestion_owner_action,
            suggestions_bridge::accept_suggestion_task,
            suggestions_bridge::list_suggestion_preferences,
            suggestions_bridge::set_suggestion_preference,
            presentation_bridge::get_task_presentation,
            presentation_bridge::get_task_progress,
            routine_bridge::routine_request,
            zip_intake_bridge::get_zip_intake_readiness,
            start_local_runtime
        ])
        .run(tauri::generate_context!())
        .expect("failed to start LiteCowork desktop shell");
}

#[cfg(test)]
mod runtime_status_bridge_tests {
    use super::RuntimeStatus;
    use serde_json::json;

    #[test]
    fn tauri_status_bridge_keeps_process_api_and_execution_readiness_distinct() {
        let status = RuntimeStatus {
            state: "DEGRADED".to_owned(),
            runtime_id: Some("runtime-1".to_owned()),
            local_incarnation_id: Some("incarnation-2".to_owned()),
            blockers: vec!["TASK_RECOVERY_UNAVAILABLE".to_owned()],
            last_shutdown_clean: Some(true),
            process_running: false,
            operator_ready: false,
            daemon_available: true,
            detail: Some("managed service is not verified".to_owned()),
        };

        let payload = serde_json::to_value(status).expect("serialize Tauri command result");
        assert_eq!(
            payload,
            json!({
                "state": "DEGRADED",
                "runtimeId": "runtime-1",
                "localIncarnationId": "incarnation-2",
                "blockers": ["TASK_RECOVERY_UNAVAILABLE"],
                "lastShutdownClean": true,
                "processRunning": false,
                "operatorReady": false,
                "daemonAvailable": true,
                "detail": "managed service is not verified"
            })
        );
    }
}

#[cfg(test)]
mod bounded_json_tests {
    use super::bounded_json_bytes;

    #[test]
    fn json_serialization_stops_at_the_configured_limit() {
        assert_eq!(bounded_json_bytes(&"abc", 5).unwrap(), br#""abc""#);
        assert!(bounded_json_bytes(&"abcd", 5).is_err());
    }
}

#[cfg(test)]
mod resource_search_provenance_tests {
    use super::{
        MAX_INDEXED_RESOURCE_BYTES, ResourceSearchPageWire, ResourceSearchRefWire,
        ResourceSearchResultWire, ResourceTextMatchWire, map_resource_search_page,
    };

    fn span(term: &str, start: u64, end: u64) -> ResourceTextMatchWire {
        ResourceTextMatchWire {
            term: term.to_owned(),
            start_utf8_byte: start,
            end_utf8_byte_exclusive: end,
        }
    }

    fn result(source_matches: Vec<ResourceTextMatchWire>) -> ResourceSearchResultWire {
        ResourceSearchResultWire {
            resource_ref: ResourceSearchRefWire {
                workspace_id: "workspace-1".to_owned(),
                resource_id: "resource-1".to_owned(),
                revision_id: Some("revision-4".to_owned()),
            },
            source_content_digest: format!("sha256:{}", "a".repeat(64)),
            source_matches,
            display_name: "notes.md".to_owned(),
            freshness: "CURRENT".to_owned(),
            match_reasons: vec!["CONTENT_INDEXED".to_owned()],
            snippet: Some("mutex and threads".to_owned()),
        }
    }

    fn page(mode: &str, item: ResourceSearchResultWire) -> ResourceSearchPageWire {
        ResourceSearchPageWire {
            items: vec![item],
            next_cursor: None,
            mode: mode.to_owned(),
            content_scan: None,
        }
    }

    #[test]
    fn indexed_search_preserves_digest_and_validated_source_matches() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let view = map_resource_search_page(
            "workspace-1",
            "INDEXED_CONTENT",
            "mutex THREADS",
            page("INDEXED_CONTENT", result(vec![span("mutex", 8, 13), span("threads", 18, 25)])),
        )
        .expect("valid indexed result");

        assert_eq!(view.items[0].resource_revision_id, "revision-4");
        assert_eq!(view.items[0].source_content_digest, digest);
        assert_eq!(view.items[0].source_matches.len(), 2);
        assert_eq!(view.items[0].source_matches[1].end_utf8_byte_exclusive, 25);
        let serialized = serde_json::to_value(&view).expect("serialize Tauri view");
        assert_eq!(serialized["items"][0]["sourceContentDigest"], view.items[0].source_content_digest);
        assert_eq!(serialized["items"][0]["sourceMatches"][0]["startUtf8Byte"], 8);
    }

    #[test]
    fn metadata_and_on_demand_results_keep_an_empty_match_list() {
        for mode in ["METADATA", "ON_DEMAND_CONTENT"] {
            let mut item = result(Vec::new());
            item.match_reasons = vec!["NAME".to_owned()];
            let view = map_resource_search_page(
                "workspace-1",
                mode,
                "notes",
                page(mode, item),
            )
            .expect("non-indexed search remains usable");
            assert!(view.items[0].source_matches.is_empty());
        }
    }

    #[test]
    fn malformed_digest_unpinned_or_cross_workspace_result_is_rejected() {
        let mut item = result(Vec::new());
        item.source_content_digest = "sha256:ABC".to_owned();
        assert!(map_resource_search_page("workspace-1", "METADATA", "", page("METADATA", item)).is_err());

        let mut item = result(Vec::new());
        item.resource_ref.revision_id = None;
        assert!(map_resource_search_page("workspace-1", "METADATA", "", page("METADATA", item)).is_err());

        let mut item = result(Vec::new());
        item.resource_ref.workspace_id = "workspace-other".to_owned();
        assert!(map_resource_search_page("workspace-1", "METADATA", "", page("METADATA", item)).is_err());
    }

    #[test]
    fn duplicate_invalid_overlapping_and_out_of_bounds_spans_are_rejected() {
        for spans in [
            vec![span("mutex", 1, 4), span("mutex", 8, 11)],
            vec![span("Mutex", 1, 6), span("threads", 8, 15)],
            vec![span("mutex", 1, 1), span("threads", 8, 15)],
            vec![span("mutex", 1, 6), span("threads", 5, 12)],
            vec![span("mutex", 1, 6), span("threads", 8, MAX_INDEXED_RESOURCE_BYTES + 1)],
        ] {
            let mut item = result(spans);
            item.match_reasons = vec!["CONTENT_INDEXED".to_owned()];
            assert!(map_resource_search_page(
                "workspace-1",
                "INDEXED_CONTENT",
                "mutex threads",
                page("INDEXED_CONTENT", item),
            )
            .is_err());
        }
    }

    #[test]
    fn indexed_spans_must_match_the_query_and_mode_must_match_response() {
        let item = result(vec![span("mutex", 8, 13)]);
        assert!(map_resource_search_page(
            "workspace-1",
            "INDEXED_CONTENT",
            "mutex threads",
            page("INDEXED_CONTENT", item),
        )
        .is_err());

        let item = result(vec![span("mutex", 8, 13)]);
        assert!(map_resource_search_page(
            "workspace-1",
            "METADATA",
            "mutex",
            page("INDEXED_CONTENT", item),
        )
        .is_err());
    }
}

#[cfg(test)]
mod resource_preview_provenance_tests {
    use super::verify_resource_preview_bytes;
    use sha2::{Digest, Sha256};

    fn digest(bytes: &[u8]) -> String {
        format!("sha256:{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn verified_preview_requires_exact_revision_and_content_digest() {
        let bytes = "hello 🌍".as_bytes().to_vec();
        let content_digest = digest(&bytes);
        let preview = verify_resource_preview_bytes(
            bytes.clone(),
            Some("revision-7"),
            "revision-7",
            &content_digest,
        )
        .expect("matching exact revision and bytes");
        assert_eq!(preview.text, "hello 🌍");
        assert_eq!(preview.resource_revision_id, "revision-7");
        assert_eq!(preview.content_digest, content_digest);

        assert!(verify_resource_preview_bytes(
            bytes.clone(),
            Some("revision-8"),
            "revision-7",
            &content_digest,
        )
        .is_err());
        assert!(verify_resource_preview_bytes(
            bytes,
            Some("revision-7"),
            "revision-7",
            &format!("sha256:{}", "a".repeat(64)),
        )
        .is_err());
    }
}

#[cfg(test)]
mod task_planning_readiness_tests {
    use super::{
        TaskPlanningReadinessView, TaskPlanningReadinessWire,
        validate_task_planning_readiness_identity,
    };
    use serde_json::json;

    fn valid_wire() -> serde_json::Value {
        json!({
            "task_id": "task-1",
            "task_version": 3,
            "task_spec_revision": 2,
            "task_status": "READY",
            "observed_at": "2026-10-09T00:00:00Z",
            "dispatch_available": false,
            "planning_started": false,
            "agent_session_started": false,
            "plan_created": false,
            "blockers": ["TASK_ISOLATION_UNAVAILABLE"]
        })
    }

    fn parse(value: serde_json::Value) -> Result<TaskPlanningReadinessView, String> {
        let wire: TaskPlanningReadinessWire = serde_json::from_value(value)
            .map_err(|_| "wire decode failed".to_owned())?;
        TaskPlanningReadinessView::try_from(wire)
    }

    #[test]
    fn accepts_a_read_only_blocked_readiness_projection() {
        let view = parse(valid_wire()).expect("valid read-only projection");
        assert_eq!(view.task_id, "task-1");
        assert_eq!(view.task_version, 3);
        assert_eq!(view.blockers.len(), 1);
        assert!(!view.dispatch_available);
        assert!(!view.planning_started);
        assert!(!view.agent_session_started);
        assert!(!view.plan_created);
    }

    #[test]
    fn rejects_unknown_fields_and_unknown_blockers() {
        let mut extra_field = valid_wire();
        extra_field["future_authority"] = json!(true);
        assert!(parse(extra_field).is_err());

        let mut unknown_blocker = valid_wire();
        unknown_blocker["blockers"] = json!(["SOMETHING_NEW"]);
        assert!(parse(unknown_blocker).is_err());
    }

    #[test]
    fn rejects_duplicate_blockers_and_any_dispatch_claim() {
        let mut duplicate = valid_wire();
        duplicate["blockers"] = json!([
            "TASK_ISOLATION_UNAVAILABLE",
            "TASK_ISOLATION_UNAVAILABLE"
        ]);
        assert!(parse(duplicate).is_err());

        for field in ["dispatch_available", "planning_started", "agent_session_started", "plan_created"] {
            let mut claims_execution = valid_wire();
            claims_execution[field] = json!(true);
            assert!(parse(claims_execution).is_err(), "accepted {field}=true");
        }
    }

    #[test]
    fn rejects_a_projection_for_another_task_or_version() {
        let view = parse(valid_wire()).expect("valid read-only projection");
        assert!(validate_task_planning_readiness_identity(view.clone(), "task-2", 3).is_err());
        assert!(validate_task_planning_readiness_identity(view.clone(), "task-1", 4).is_err());
        assert!(validate_task_planning_readiness_identity(view, "task-1", 3).is_ok());
    }
}
