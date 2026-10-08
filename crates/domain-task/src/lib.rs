mod execution;
pub use execution::{ExecutionDecision, ExecutionDenial, StepAttemptCoordinator, decide_attempt_admission, decide_lease_expiry, decide_lease_release, decide_lease_renewal};

mod planning;

pub use planning::{TaskPlanningPacket, initial_plan_output_schema, parse_initial_plan_output};

use domain_workspace::EventContext;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use storage_core::{
    ActivateTaskPlanningSession as ActivatePlanningSessionCommit, AgentBindingRecord,
    AgentCatalogStore, AgentEndpointRecord, AgentSessionRecord,
    AgentSessionStore, CommittedAgentSession, CommittedPlanningActivation, CommittedTask,
    CommittedTaskSpecRevision,
    EventDraft, PlanAcceptance, PlanAcceptanceCommit, PlanRevisionRecord, PlannedStepRecord,
    SuggestedTaskCreateCommit, SuggestionTaskAcceptanceStore,
    StepRecord, StoreError, TaskCreateCommit, TaskRecord, TaskPlanningSessionStart,
    TaskSpecRevisionCommit,
    TaskSpecRevisionRecord, TaskStore, TaskView, WorkspaceCreateRequest,
};

/// Input for a standalone durable Task. Conversation origin uses its own admission
/// seam; an optional Coworker origin is version-pinned and rechecked by Task storage.
#[derive(Clone, Debug)]
pub struct CreateStandaloneTask {
    pub task_id: String,
    pub workspace_id: String,
    pub workspace_instruction_revision: Option<u64>,
    pub lead_agent_binding_id: String,
    pub origin_coworker_id: Option<String>,
    pub origin_coworker_revision: Option<u64>,
    pub expected_coworker_version: Option<u64>,
    /// Snapshot value resolved from the selected CoworkerRevision. Explicit
    /// request policy still takes precedence; storage rechecks the pinned revision.
    pub coworker_default_lead_failover_policy: Option<Value>,
    pub principal_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub event: EventContext,
}

/// Owner-authored changes to a saved Task. The Operator constructs this only after
/// authenticating the Workspace owner; SQLite repeats the version, state, parent, and
/// Resource-scope checks in the atomic commit.
#[derive(Clone, Debug)]
pub struct ReviseTaskSpec {
    pub workspace_id: String,
    pub task_id: String,
    pub principal_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub expected_task_version: u64,
    pub parent_revisions: Vec<u64>,
    pub objective: String,
    pub constraints: Option<Vec<String>>,
    pub non_goals: Option<Vec<String>>,
    pub input_refs: Option<Vec<Value>>,
    pub required_outputs: Option<Vec<Value>>,
    pub acceptance_criteria: Option<Vec<Value>>,
    pub approvals_required: Option<Vec<Value>>,
    pub placement_preference: Option<Value>,
    pub preferred_lead_agent_binding_id: Option<String>,
    pub lead_failover_policy: Option<Value>,
    pub event: EventContext,
}

/// Runtime/endpoint selection has already been made by the placement layer. Storage
/// rechecks that the selected endpoint is still eligible for this Task and incarnation.
#[derive(Clone, Debug)]
pub struct ReserveTaskPlanningSession {
    pub workspace_id: String,
    pub task_id: String,
    pub expected_task_version: u64,
    pub expected_task_spec_revision: u64,
    pub agent_session_id: String,
    pub endpoint_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub configuration_digest: Option<String>,
    pub harness_descriptor_digest: Option<String>,
    pub principal_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub event: EventContext,
}

/// Adapter readiness has been observed by the caller. This command commits that
/// observation and the Task's first READY -> RUNNING transition atomically.
#[derive(Clone, Debug)]
pub struct ActivateTaskPlanningSessionCommand {
    pub workspace_id: String,
    pub agent_session_id: String,
    pub expected_session_version: u64,
    pub expected_task_version: u64,
    pub host_instance_id: String,
    /// Runtime-local handle only; storage keeps it out of domain events and snapshots.
    pub native_session_ref: Option<String>,
    pub session_event: EventContext,
    /// Required only while the Task is READY. Event timestamp/correlation/causation
    /// are derived from the session activation event, while this supplies its own ID.
    pub task_status_event: Option<EventContext>,
}

/// Authenticated local inputs for preparing a Task-planning assignment. The selected
/// Runtime identity comes from the daemon, never from an Operator request body.
#[derive(Clone, Debug)]
pub struct PreparePlanningAssignment {
    pub owner_principal_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub expected_task_version: u64,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub now: String,
}

/// Bounded, revision-pinned input to the internal PlanningCoordinator. It contains
/// stable endpoint identity but never the Runtime-local executable locator.
#[derive(Clone, Debug)]
pub struct PlanningAssignment {
    pub task: TaskView,
    pub binding: AgentBindingRecord,
    pub endpoint: AgentEndpointRecord,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
}

/// Proposed plan input from the currently active native planning session. Durable Step
/// IDs are allocated by TaskService; the agent submits only logical keys.
#[derive(Clone, Debug)]
pub struct SubmitInitialPlan {
    pub principal_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub agent_session_id: String,
    pub expected_task_version: u64,
    pub expected_task_spec_revision: u64,
    pub request_id: String,
    pub request_payload: Value,
    pub steps: Vec<ProposedStep>,
    pub reason_for_revision: Option<String>,
    pub event: EventContext,
    pub step_events: Vec<EventContext>,
}

#[derive(Clone, Debug)]
pub struct ProposedStep {
    pub logical_key: String,
    pub title: String,
    pub objective: String,
    pub depends_on_logical_keys: Vec<String>,
    pub required_capabilities: Vec<Value>,
    pub acceptance_criteria: Vec<Value>,
}

pub struct TaskService<S> {
    store: S,
}

impl<S: TaskStore> TaskService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Persists one READY Task, its first immutable spec, event, aggregate snapshot and
    /// idempotency receipt. This method does not start planning or imply execution.
    pub fn create_standalone(
        &self,
        command: CreateStandaloneTask,
    ) -> Result<CommittedTask, StoreError> {
        let commit = self.prepare_standalone(command)?;
        self.store.create_task(commit)
    }

    /// Builds the exact ordinary Task creation commit without persisting it. The
    /// Suggestion acceptance path uses this same constructor and submits the commit
    /// to a storage port that atomically resolves the Suggestion alongside it.
    pub fn prepare_standalone(
        &self,
        command: CreateStandaloneTask,
    ) -> Result<TaskCreateCommit, StoreError> {
        let request = &command.request_payload;
        let objective = required_string(request, "objective")?.trim().to_owned();
        if objective.is_empty() || objective.len() > 32 * 1024 {
            return Err(StoreError::Invalid("Task objective must contain 1 to 32768 bytes".to_owned()));
        }
        if optional_string(request, "conversation_id")?.is_some()
            || !string_array(request, "source_message_refs")?.is_empty()
        {
            return Err(StoreError::Invalid(
                "Conversation-origin Task creation requires its atomic admission service".to_owned(),
            ));
        }
        let request_coworker_id = optional_string(request, "coworker_id")?.map(str::to_owned);
        let request_coworker_version = request.get("expected_coworker_version").and_then(Value::as_u64);
        if request_coworker_id != command.origin_coworker_id
            || request_coworker_version != command.expected_coworker_version
            || command.origin_coworker_id.is_some() != command.origin_coworker_revision.is_some()
            || (command.expected_coworker_version.is_some() && command.origin_coworker_id.is_none())
        {
            return Err(StoreError::Invalid("Task Coworker origin does not match the accepted request".to_owned()));
        }
        if request.get("workspace_id").and_then(Value::as_str) != Some(command.workspace_id.as_str()) {
            return Err(StoreError::Invalid("Task Workspace does not match its command".to_owned()));
        }

        let created_by = json!({ "kind": "USER", "principal_id": command.principal_id });
        let timestamp = command.event.recorded_at.clone();
        let spec = TaskSpecRevisionRecord {
            task_id: command.task_id.clone(),
            workspace_id: command.workspace_id.clone(),
            revision: 1,
            parent_revisions: Vec::new(),
            objective,
            task_category: optional_string(request, "task_category")?.map(str::to_owned),
            constraints: string_array(request, "constraints")?,
            non_goals: string_array(request, "non_goals")?,
            input_refs: value_array(request, "input_refs")?,
            workspace_instruction_revision: command.workspace_instruction_revision,
            required_outputs: value_array(request, "required_outputs")?,
            acceptance_criteria: value_array(request, "acceptance_criteria")?,
            approvals_required: value_array(request, "approvals_required")?,
            budget: optional_value(request, "budget"),
            delegation_budget_policy: optional_value(request, "delegation_budget_policy"),
            lead_failover_policy: request.get("lead_failover_policy").cloned()
                .or(command.coworker_default_lead_failover_policy)
                .unwrap_or_else(|| json!({
                "mode": "DISABLED",
                "triggers": [],
                "fallback_agent_binding_ids": [],
                "max_lead_changes": 0
            })),
            deadline: optional_string(request, "deadline")?.map(str::to_owned),
            source_message_refs: Vec::new(),
            placement_preference: request.get("placement_preference").cloned().unwrap_or_else(|| json!("AUTO")),
            preferred_lead_agent_binding_id: Some(command.lead_agent_binding_id.clone()),
            authored_by: created_by.clone(),
            created_at: timestamp.clone(),
        };
        let task = TaskRecord {
            task_id: command.task_id.clone(),
            workspace_id: command.workspace_id.clone(),
            conversation_id: None,
            current_spec_revision: 1,
            current_plan_revision: None,
            status: "READY".to_owned(),
            resume_status: None,
            routine_id: None,
            routine_revision: None,
            automation_id: None,
            automation_occurrence_id: None,
            origin_coworker_id: command.origin_coworker_id,
            origin_coworker_revision: command.origin_coworker_revision,
            lead_agent_binding_id: command.lead_agent_binding_id,
            blocking_conditions: Vec::new(),
            priority: "NORMAL".to_owned(),
            created_by: created_by.clone(),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
            completed_at: None,
            version: 1,
        };
        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: command.workspace_id.clone(),
            entity_type: "Task".to_owned(),
            entity_id: command.task_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "task.created.v1".to_owned(),
            payload: json!({
                "task_id": command.task_id,
                "initial_spec_revision": 1,
                "created_by": created_by,
                "origin_coworker_id": task.origin_coworker_id.clone(),
                "origin_coworker_revision": task.origin_coworker_revision,
            }),
            recorded_at: command.event.recorded_at,
        };
        Ok(TaskCreateCommit {
            request: WorkspaceCreateRequest {
                principal_id: command.principal_id,
                request_id: command.request_id,
                request_payload: command.request_payload,
            },
            expected_coworker_version: command.expected_coworker_version,
            task,
            initial_spec_revision: spec,
            event,
        })
    }

    /// Creates the next immutable TaskSpecRevision for a saved, not-yet-planned Task.
    /// A Task with an accepted plan or a live planning session must use the full
    /// steering/replanning lifecycle instead of this bounded editor path.
    pub fn revise_saved_spec(
        &self,
        command: ReviseTaskSpec,
    ) -> Result<CommittedTaskSpecRevision, StoreError> {
        if command.workspace_id.trim().is_empty()
            || command.task_id.trim().is_empty()
            || command.principal_id.trim().is_empty()
            || command.request_id.trim().is_empty()
            || command.expected_task_version == 0
        {
            return Err(StoreError::Invalid("Task specification revision identity is invalid".to_owned()));
        }
        if let Some(receipt) = self.store.get_task_spec_revision_receipt(
            &command.principal_id,
            &command.request_id,
            &command.request_payload,
        )? {
            return Ok(receipt);
        }
        let task = self.store.get_task(&command.workspace_id, &command.task_id)?
            .ok_or(StoreError::NotFound)?;
        if task.task.version != command.expected_task_version {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_task_version),
                actual: Some(task.task.version),
            });
        }
        if task.task.status != "READY" || task.task.current_plan_revision.is_some() {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_task_version),
                actual: Some(task.task.version),
            });
        }
        if command.parent_revisions.len() != 1
            || command.parent_revisions.first().copied() != Some(task.current_spec_revision.revision)
        {
            return Err(StoreError::Conflict {
                expected: Some(task.current_spec_revision.revision),
                actual: command.parent_revisions.last().copied(),
            });
        }
        let objective = command.objective.trim().to_owned();
        if objective.is_empty() || objective.len() > 32 * 1024 {
            return Err(StoreError::Invalid("Task objective must contain 1 to 32768 bytes".to_owned()));
        }

        let parent_spec = task.current_spec_revision.clone();
        let mut revision = parent_spec.clone();
        revision.revision = revision.revision.checked_add(1)
            .ok_or_else(|| StoreError::Integrity("TaskSpec revision exhausted".to_owned()))?;
        revision.parent_revisions = command.parent_revisions.clone();
        revision.objective = objective;
        if let Some(value) = command.constraints { validate_task_text_list("constraints", &value)?; revision.constraints = value; }
        if let Some(value) = command.non_goals { validate_task_text_list("non_goals", &value)?; revision.non_goals = value; }
        if let Some(value) = command.input_refs { validate_json_list("input_refs", &value, 100)?; revision.input_refs = value; }
        if let Some(value) = command.required_outputs { validate_json_list("required_outputs", &value, 100)?; revision.required_outputs = value; }
        if let Some(value) = command.acceptance_criteria { validate_json_list("acceptance_criteria", &value, 100)?; revision.acceptance_criteria = value; }
        if let Some(value) = command.approvals_required { validate_json_list("approvals_required", &value, 100)?; revision.approvals_required = value; }
        if let Some(value) = command.placement_preference { revision.placement_preference = value; }
        if let Some(value) = command.preferred_lead_agent_binding_id {
            if value.trim().is_empty() || value.len() > 200 || value.chars().any(char::is_control) {
                return Err(StoreError::Invalid("preferred lead AgentBinding is invalid".to_owned()));
            }
            if value != task.task.lead_agent_binding_id {
                return Err(StoreError::Invalid("lead changes require the dedicated Task lead transition".to_owned()));
            }
            revision.preferred_lead_agent_binding_id = Some(value);
        }
        if let Some(value) = command.lead_failover_policy { revision.lead_failover_policy = value; }
        if same_task_spec_content(&parent_spec, &revision) {
            return Err(StoreError::Invalid("TaskSpec revision does not change any specification fields".to_owned()));
        }
        revision.authored_by = json!({"kind":"USER", "principal_id":command.principal_id.clone()});
        revision.created_at = command.event.recorded_at.clone();

        let spec_value = serde_json::to_value(&revision)
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
        let spec_bytes = serde_json_canonicalizer::to_vec(&spec_value)
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
        let spec_digest = format!("sha256:{}", hex::encode(Sha256::digest(spec_bytes)));
        let next_task_version = command.expected_task_version.checked_add(1)
            .ok_or_else(|| StoreError::Integrity("Task version exhausted".to_owned()))?;
        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: command.workspace_id.clone(),
            entity_type: "Task".to_owned(),
            entity_id: command.task_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: next_task_version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "task.spec.revised.v1".to_owned(),
            payload: json!({
                "task_id": command.task_id.clone(),
                "revision": revision.revision,
                "parent_revisions": revision.parent_revisions.clone(),
                "spec_digest": spec_digest,
                "authored_by": revision.authored_by.clone(),
            }),
            recorded_at: command.event.recorded_at,
        };
        let mut next_task = task.task;
        next_task.current_spec_revision = revision.revision;
        next_task.updated_at = command.event.recorded_at.clone();
        next_task.version = next_task_version;
        self.store.revise_task_spec(TaskSpecRevisionCommit {
            request: WorkspaceCreateRequest {
                principal_id: command.principal_id,
                request_id: command.request_id,
                request_payload: command.request_payload,
            },
            expected_task_version: command.expected_task_version,
            task: next_task,
            task_spec_revision: revision,
            event,
        })
    }

    /// Claims one durable TASK_PLANNING session in STARTING state after rereading the
    /// Task. This does not start the native adapter or mark the Task RUNNING.
    pub fn reserve_planning_session(
        &self,
        command: ReserveTaskPlanningSession,
    ) -> Result<CommittedAgentSession, StoreError>
    where
        S: AgentSessionStore,
    {
        if command.expected_task_version == 0 || command.expected_task_spec_revision == 0 {
            return Err(StoreError::Invalid("planning assignment revision is invalid".to_owned()));
        }
        let view = self.store.get_task(&command.workspace_id, &command.task_id)?
            .ok_or(StoreError::NotFound)?;
        if view.task.version != command.expected_task_version
            || view.task.current_spec_revision != command.expected_task_spec_revision
            || !matches!(view.task.status.as_str(), "READY" | "RUNNING")
        {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_task_version),
                actual: Some(view.task.version),
            });
        }
        let timestamp = command.event.recorded_at.clone();
        let session = AgentSessionRecord {
            agent_session_id: command.agent_session_id.clone(),
            workspace_id: command.workspace_id.clone(),
            scope_kind: "TASK_PLANNING".to_owned(),
            conversation_id: None,
            conversation_turn_id: None,
            task_id: Some(command.task_id.clone()),
            task_spec_revision: Some(command.expected_task_spec_revision),
            attempt_id: None,
            agent_binding_id: view.task.lead_agent_binding_id,
            endpoint_id: command.endpoint_id,
            runtime_id: command.runtime_id,
            runtime_incarnation_id: command.runtime_incarnation_id,
            configuration_digest: command.configuration_digest,
            harness_descriptor_digest: command.harness_descriptor_digest,
            status: "STARTING".to_owned(),
            started_at: timestamp.clone(),
            last_event_at: Some(timestamp.clone()),
            closed_at: None,
            version: 1,
        };
        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: command.workspace_id,
            entity_type: "AgentSession".to_owned(),
            entity_id: command.agent_session_id,
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "agent.session.starting.v1".to_owned(),
            payload: json!({
                "agent_session_id": session.agent_session_id,
                "scope": { "kind": "TASK_PLANNING", "task_id": command.task_id },
                "agent_binding_id": session.agent_binding_id,
                "endpoint_id": session.endpoint_id,
                "runtime_id": session.runtime_id,
                "runtime_incarnation_id": session.runtime_incarnation_id,
                "task_spec_revision": command.expected_task_spec_revision,
                "session_state": "STARTING",
                "reported_at": timestamp,
            }),
            recorded_at: command.event.recorded_at,
        };
        self.store.start_task_planning_session(TaskPlanningSessionStart {
            principal_id: command.principal_id,
            request_id: command.request_id,
            request_payload: command.request_payload,
            expected_task_version: command.expected_task_version,
            session,
            event,
        })
    }

    /// Accepts the first PlanRevision from the active Task-planning session. This
    /// implementation intentionally rejects replanning until execution Attempt/lease
    /// authority is available in the durable Task runtime.
    pub fn submit_initial_plan(
        &self,
        command: SubmitInitialPlan,
    ) -> Result<PlanAcceptance, StoreError> {
        if command.steps.is_empty() || command.steps.len() > 100
            || command.steps.len() != command.step_events.len() {
            return Err(StoreError::Invalid("plan Step/event counts do not match".to_owned()));
        }
        if command.principal_id.trim().is_empty()
            || command.workspace_id.trim().is_empty()
            || command.task_id.trim().is_empty()
            || command.agent_session_id.trim().is_empty()
            || command.request_id.trim().is_empty()
            || command.expected_task_version == 0
            || command.expected_task_spec_revision == 0
        {
            return Err(StoreError::Invalid("plan submission identity is invalid".to_owned()));
        }
        if command.reason_for_revision.as_ref().is_some_and(|reason| reason.len() > 8192 || reason.chars().any(char::is_control)) {
            return Err(StoreError::Invalid("plan revision reason is invalid".to_owned()));
        }
        let step_ids = (0..command.steps.len()).map(|_| new_step_id())
            .collect::<Result<Vec<_>, _>>()?;
        validate_plan_proposal(&command.steps, &step_ids)?;
        // Bind the idempotency receipt to the typed plan accepted by this method.
        // Caller-provided request JSON is retained for audit compatibility, but it
        // cannot substitute for the actual validated plan payload.
        let normalized_plan = json!({
            "reason_for_revision": &command.reason_for_revision,
            "steps": command.steps.iter().map(|step| json!({
                "logical_key": &step.logical_key,
                "title": &step.title,
                "objective": &step.objective,
                "depends_on_logical_keys": &step.depends_on_logical_keys,
                "required_capabilities": &step.required_capabilities,
                "acceptance_criteria": &step.acceptance_criteria,
            })).collect::<Vec<_>>(),
        });
        let logical_ids = command.steps.iter().zip(&step_ids)
            .map(|(step, id)| (step.logical_key.as_str(), id.as_str()))
            .collect::<std::collections::HashMap<_, _>>();
        let materialized_steps = command.steps.iter().zip(&step_ids).map(|(step, step_id)| {
            let dependencies = step.depends_on_logical_keys.iter()
                .map(|key| logical_ids.get(key.as_str()).copied()
                    .ok_or_else(|| StoreError::Invalid("plan dependency key is missing".to_owned())))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(StepRecord {
                step_id: step_id.clone(),
                task_id: command.task_id.clone(),
                plan_revision: 1,
                logical_key: Some(step.logical_key.clone()),
                title: step.title.clone(),
                objective: step.objective.clone(),
                dependencies,
                required_capabilities: step.required_capabilities.clone(),
                acceptance_criteria: step.acceptance_criteria.clone(),
                status: if step.depends_on_logical_keys.is_empty() { "READY" } else { "PENDING" }.to_owned(),
                current_attempt_id: None,
                created_at: command.event.recorded_at.clone(),
                updated_at: command.event.recorded_at.clone(),
                version: 1,
            })
        }).collect::<Result<Vec<_>, StoreError>>()?;
        let planned_steps = command.steps.iter().map(|step| PlannedStepRecord {
            logical_key: step.logical_key.clone(),
            title: step.title.clone(),
            objective: step.objective.clone(),
            depends_on_logical_keys: step.depends_on_logical_keys.clone(),
            required_capabilities: step.required_capabilities.clone(),
            acceptance_criteria: step.acceptance_criteria.clone(),
        }).collect::<Vec<_>>();
        let plan = PlanRevisionRecord {
            task_id: command.task_id.clone(),
            revision: 1,
            task_spec_revision: command.expected_task_spec_revision,
            produced_by_agent_session_id: command.agent_session_id.clone(),
            produced_by_attempt_id: None,
            steps: planned_steps,
            reason_for_revision: command.reason_for_revision,
            created_at: command.event.recorded_at.clone(),
        };
        let updated_task_version = command.expected_task_version.checked_add(1)
            .ok_or_else(|| StoreError::Integrity("Task version exhausted".to_owned()))?;
        let plan_event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: command.workspace_id.clone(),
            entity_type: "Task".to_owned(),
            entity_id: command.task_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: updated_task_version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "task.plan.revised.v1".to_owned(),
            payload: json!({
                "task_id": command.task_id,
                "revision": 1,
                "task_spec_revision": command.expected_task_spec_revision,
                "produced_by_agent_session_id": command.agent_session_id,
                "step_ids": step_ids,
                "aggregate_version": updated_task_version,
            }),
            recorded_at: command.event.recorded_at.clone(),
        };
        let step_events = materialized_steps.iter().zip(command.step_events).map(|(step, event)| EventDraft {
            event_id: event.event_id,
            workspace_id: command.workspace_id.clone(),
            entity_type: "Step".to_owned(),
            entity_id: step.step_id.clone(),
            origin_runtime_id: event.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: event.hlc_timestamp,
            correlation_id: event.correlation_id,
            causation_id: event.causation_id,
            schema_version: 1,
            event_type: "step.created.v1".to_owned(),
            payload: json!({
                "step_id": step.step_id,
                "task_id": step.task_id,
                "plan_revision": step.plan_revision,
                "logical_key": step.logical_key,
                "dependencies": step.dependencies,
            }),
            recorded_at: event.recorded_at,
        }).collect();
        let request_id = format!("task.plan.{}:{}", command.task_id, command.request_id);
        let request_payload = json!({
            "route": "POST /v1/tasks/{task_id}/plan-revisions",
            "task_id": command.task_id,
            "producer_agent_session_id": command.agent_session_id,
            "expected_task_version": command.expected_task_version,
            "expected_task_spec_revision": command.expected_task_spec_revision,
            "request": command.request_payload,
            "plan": normalized_plan,
        });
        self.store.accept_initial_plan(PlanAcceptanceCommit {
            principal_id: command.principal_id,
            request_id,
            request_payload,
            workspace_id: command.workspace_id,
            task_id: command.task_id,
            expected_task_version: command.expected_task_version,
            expected_task_spec_revision: command.expected_task_spec_revision,
            plan_revision: plan,
            materialized_steps,
            plan_event,
            step_events,
        })
    }
}

impl<S> TaskService<S>
where
    S: TaskStore + SuggestionTaskAcceptanceStore,
{
    /// Creates a normal READY Task while atomically resolving its accepted Suggestion.
    /// This method deliberately does not start planning, an AgentSession, or execution.
    pub fn create_from_suggestion(
        &self,
        task: CreateStandaloneTask,
        suggestion_id: String,
        expected_suggestion_version: u64,
        accepted_at: String,
        suggestion_event: EventContext,
    ) -> Result<CommittedTask, StoreError> {
        let commit = self.prepare_standalone(task)?;
        let workspace_id = commit.task.workspace_id.clone();
        let task_id = commit.task.task_id.clone();
        let owner_principal_id = commit.request.principal_id.clone();
        let next_suggestion_version = expected_suggestion_version.checked_add(1)
            .ok_or_else(|| StoreError::Integrity("Suggestion version exhausted".to_owned()))?;
        let suggestion_event = EventDraft {
            event_id: suggestion_event.event_id,
            workspace_id,
            entity_type: "Suggestion".to_owned(),
            entity_id: suggestion_id.clone(),
            origin_runtime_id: suggestion_event.origin_runtime_id,
            entity_revision: next_suggestion_version,
            hlc_timestamp: suggestion_event.hlc_timestamp,
            correlation_id: suggestion_event.correlation_id,
            causation_id: suggestion_event.causation_id,
            schema_version: 1,
            event_type: "suggestion.resolved.v1".to_owned(),
            payload: json!({
                "suggestion_id": suggestion_id,
                "from": "PROPOSED",
                "to": "ACCEPTED",
                "resolved_by": { "principal_id": owner_principal_id, "kind": "USER" },
                "resolution_reason": "ACCEPTED_BY_OWNER",
                "result_task_id": task_id,
                "aggregate_version": next_suggestion_version,
            }),
            recorded_at: accepted_at.clone(),
        };
        let acceptance = SuggestedTaskCreateCommit {
            task: commit,
            suggestion_id,
            expected_suggestion_version,
            accepted_at,
            suggestion_event,
        };
        self.store.create_task_from_suggestion(acceptance)
    }
}

fn validate_plan_proposal(
    steps: &[ProposedStep],
    step_ids: &[String],
) -> Result<(), StoreError> {
    if steps.is_empty() || steps.len() > 100 || steps.len() != step_ids.len() {
        return Err(StoreError::Invalid("a plan must contain 1 to 100 steps with allocated IDs".to_owned()));
    }
    let mut keys = std::collections::HashSet::new();
    let mut ids = std::collections::HashSet::new();
    let mut serialized_bytes = 16_usize;
    for (step, id) in steps.iter().zip(step_ids) {
        if step.logical_key.is_empty() || step.logical_key.len() > 128
            || !step.logical_key.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            || !keys.insert(step.logical_key.as_str())
            || id.trim().is_empty() || id.len() > 200 || id.chars().any(char::is_control) || !ids.insert(id.as_str())
            || step.title.trim().is_empty() || step.title.len() > 512
            || step.objective.trim().is_empty() || step.objective.len() > 16 * 1024
            || step.title.chars().any(char::is_control)
            || step.objective.chars().any(char::is_control)

        {
            return Err(StoreError::Invalid("plan step identity or acceptance criteria are invalid".to_owned()));
        }
        planning::validate_plan_step_content(step)?;
        serialized_bytes = serialized_bytes.checked_add(planning::plan_step_serialized_size(step)?)
            .ok_or_else(|| StoreError::Invalid("plan exceeds its total size limit".to_owned()))?;
        if serialized_bytes > 256 * 1024 {
            return Err(StoreError::Invalid("plan exceeds its total size limit".to_owned()));
        }
    }
    let key_set = steps.iter().map(|step| step.logical_key.as_str()).collect::<std::collections::HashSet<_>>();
    for step in steps {
        let mut deps = std::collections::HashSet::new();
        if step.depends_on_logical_keys.iter().any(|key| {
            key == &step.logical_key || !key_set.contains(key.as_str()) || !deps.insert(key.as_str())
        }) {
            return Err(StoreError::Invalid("plan dependencies must be unique, present, and non-self-referential".to_owned()));
        }
    }
    let by_key = steps.iter().map(|step| (step.logical_key.as_str(), step)).collect::<std::collections::HashMap<_, _>>();
    fn visit<'a>(
        key: &'a str,
        by_key: &std::collections::HashMap<&'a str, &'a ProposedStep>,
        visiting: &mut std::collections::HashSet<&'a str>,
        visited: &mut std::collections::HashSet<&'a str>,
    ) -> bool {
        if visited.contains(key) { return true; }
        if !visiting.insert(key) { return false; }
        let acyclic = by_key.get(key).is_some_and(|step| step.depends_on_logical_keys.iter().all(|dependency| visit(dependency, by_key, visiting, visited)));
        visiting.remove(key);
        if acyclic { visited.insert(key); }
        acyclic
    }
    let mut visiting = std::collections::HashSet::new();
    let mut visited = std::collections::HashSet::new();
    if !steps.iter().all(|step| visit(&step.logical_key, &by_key, &mut visiting, &mut visited)) {
        return Err(StoreError::Invalid("plan dependency graph contains a cycle".to_owned()));
    }
    Ok(())
}

fn validate_task_text_list(name: &str, values: &[String]) -> Result<(), StoreError> {
    if values.len() > 100 || values.iter().any(|value| value.len() > 8192 || value.chars().any(char::is_control)) {
        return Err(StoreError::Invalid(format!("Task {name} contains an invalid or oversized value")));
    }
    Ok(())
}

fn validate_json_list(name: &str, values: &[Value], maximum: usize) -> Result<(), StoreError> {
    if values.len() > maximum || values.iter().any(|value| value.is_null() || !value.is_object()) {
        return Err(StoreError::Invalid(format!("Task {name} contains an invalid or oversized value")));
    }
    Ok(())
}

fn same_task_spec_content(left: &TaskSpecRevisionRecord, right: &TaskSpecRevisionRecord) -> bool {
    left.objective == right.objective
        && left.task_category == right.task_category
        && left.constraints == right.constraints
        && left.non_goals == right.non_goals
        && left.input_refs == right.input_refs
        && left.workspace_instruction_revision == right.workspace_instruction_revision
        && left.required_outputs == right.required_outputs
        && left.acceptance_criteria == right.acceptance_criteria
        && left.approvals_required == right.approvals_required
        && left.budget == right.budget
        && left.delegation_budget_policy == right.delegation_budget_policy
        && left.lead_failover_policy == right.lead_failover_policy
        && left.deadline == right.deadline
        && left.source_message_refs == right.source_message_refs
        && left.placement_preference == right.placement_preference
        && left.preferred_lead_agent_binding_id == right.preferred_lead_agent_binding_id
}

fn new_step_id() -> Result<String, StoreError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|_| StoreError::Integrity("Step ID generation failed".to_owned()))?;
    Ok(format!("stp_{}", hex::encode(bytes)))
}

impl<S: TaskStore + AgentCatalogStore> TaskService<S> {
    /// Resolves the current Task lead to one fresh compatible endpoint on the exact
    /// local Runtime incarnation. Storage repeats all mutable admission checks later.
    pub fn prepare_planning_assignment(
        &self,
        request: PreparePlanningAssignment,
    ) -> Result<PlanningAssignment, StoreError> {
        if request.owner_principal_id.trim().is_empty()
            || request.workspace_id.trim().is_empty()
            || request.task_id.trim().is_empty()
            || request.runtime_id.trim().is_empty()
            || request.runtime_incarnation_id.trim().is_empty()
            || request.expected_task_version == 0
        {
            return Err(StoreError::Invalid("planning assignment identity is invalid".to_owned()));
        }
        let task = self.store.get_task(&request.workspace_id, &request.task_id)?
            .ok_or(StoreError::NotFound)?;
        if task.task.version != request.expected_task_version
            || task.task.current_plan_revision.is_some()
            || !matches!(task.task.status.as_str(), "READY" | "RUNNING")
        {
            return Err(StoreError::Conflict {
                expected: Some(request.expected_task_version),
                actual: Some(task.task.version),
            });
        }
        let binding = self.store.get_agent_binding(
            &request.owner_principal_id,
            &request.workspace_id,
            &task.task.lead_agent_binding_id,
        )?.ok_or(StoreError::NotFound)?;
        if !binding.enabled || !binding.lead_eligible
            || binding.runtime_id.as_deref().is_some_and(|runtime| runtime != request.runtime_id)
        {
            return Err(StoreError::Invalid("Task lead binding is not eligible on this Runtime".to_owned()));
        }
        let profile = self.store.list_agent_profiles(
            &request.owner_principal_id,
            &request.workspace_id,
            &request.now,
        )?.into_iter().find(|profile| profile.profile.agent_profile_id == binding.agent_profile_id)
            .ok_or(StoreError::NotFound)?;
        let policy = &binding.endpoint_selection_policy;
        let mode = policy.get("mode").and_then(Value::as_str).unwrap_or("");
        let pinned_endpoint = match mode {
            "AUTO_COMPATIBLE" => None,
            "PINNED_ENDPOINT" => Some(policy.get("endpoint_id").and_then(Value::as_str)
                .ok_or_else(|| StoreError::Integrity("pinned endpoint policy is malformed".to_owned()))?),
            _ => return Err(StoreError::Integrity("AgentBinding endpoint policy is malformed".to_owned())),
        };
        let required_features = policy.get("required_features").and_then(Value::as_array)
            .ok_or_else(|| StoreError::Integrity("AgentBinding required features are malformed".to_owned()))?;
        let preferred_topologies = policy.get("preferred_topologies").and_then(Value::as_array)
            .ok_or_else(|| StoreError::Integrity("AgentBinding topology preferences are malformed".to_owned()))?;
        let mut candidates = Vec::new();
        for view in profile.endpoints {
            let endpoint = view.endpoint;
            if endpoint.agent_profile_id != binding.agent_profile_id
                || pinned_endpoint.is_some_and(|pinned| pinned != endpoint.endpoint_id)
                || !capability_enabled(&endpoint.capabilities, "input.text")
                || !required_features.iter().all(|feature| feature.as_str()
                    .is_some_and(|feature| capability_enabled(&endpoint.capabilities, feature)))
            {
                continue;
            }
            let topology_rank = preferred_topologies.iter()
                .position(|topology| topology.as_str() == Some(endpoint.topology.as_str()))
                .unwrap_or(usize::MAX);
            for offer in view.offers {
                if offer.runtime_id != request.runtime_id
                    || offer.runtime_incarnation_id != request.runtime_incarnation_id
                    || !offer.compatible
                    || !matches!(offer.readiness.as_str(), "READY" | "STARTABLE")
                {
                    continue;
                }
                candidates.push((
                    offer.readiness != "READY",
                    topology_rank,
                    std::cmp::Reverse(offer.observed_at),
                    endpoint.clone(),
                ));
            }
        }
        candidates.sort_by(|left, right| {
            (left.0, left.1, &left.2).cmp(&(right.0, right.1, &right.2))
                .then_with(|| left.3.endpoint_id.cmp(&right.3.endpoint_id))
        });
        let endpoint = candidates.into_iter().next().map(|candidate| candidate.3)
            .ok_or_else(|| StoreError::Invalid("no fresh compatible planning endpoint is available on the current Runtime".to_owned()))?;
        Ok(PlanningAssignment {
            task,
            binding,
            endpoint,
            runtime_id: request.runtime_id,
            runtime_incarnation_id: request.runtime_incarnation_id,
        })
    }
}

fn capability_enabled(capabilities: &Value, feature: &str) -> bool {
    let mut current = capabilities;
    for segment in feature.split('.') {
        let Some(next) = current.get(segment) else { return false; };
        current = next;
    }
    current.as_bool() == Some(true)
}

impl<S: TaskStore + AgentSessionStore> TaskService<S> {
    /// Commits a native planner's observed readiness. The storage adapter repeats
    /// version/state checks inside one transaction with the session and Task writes.
    pub fn activate_planning_session(
        &self,
        command: ActivateTaskPlanningSessionCommand,
    ) -> Result<CommittedPlanningActivation, StoreError> {
        if command.expected_session_version == 0 || command.expected_task_version == 0
            || command.workspace_id.trim().is_empty()
            || command.agent_session_id.trim().is_empty()
            || command.host_instance_id.trim().is_empty()
            || command.native_session_ref.as_ref().is_some_and(|value| value.is_empty() || value.len() > 4096)
        {
            return Err(StoreError::Invalid("planning session activation command is invalid".to_owned()));
        }
        let session = self.store.get_agent_session(&command.workspace_id, &command.agent_session_id)?
            .ok_or(StoreError::NotFound)?;
        if session.scope_kind != "TASK_PLANNING" || session.status != "STARTING"
            || session.version != command.expected_session_version
            || session.task_id.is_none()
        {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_session_version),
                actual: Some(session.version),
            });
        }
        let task_id = session.task_id.as_deref().expect("checked above");
        let task = self.store.get_task(&command.workspace_id, task_id)?.ok_or(StoreError::NotFound)?;
        if task.task.version != command.expected_task_version
            || task.task.current_spec_revision != session.task_spec_revision.unwrap_or_default()
            || task.task.lead_agent_binding_id != session.agent_binding_id
            || !matches!(task.task.status.as_str(), "READY" | "RUNNING")
        {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_task_version),
                actual: Some(task.task.version),
            });
        }
        let event = command.session_event;
        let occurred_at = event.recorded_at.clone();
        let next_session_version = session.version.checked_add(1)
            .ok_or_else(|| StoreError::Integrity("AgentSession version exhausted".to_owned()))?;
        let next_task_version = task.task.version.checked_add(1)
            .ok_or_else(|| StoreError::Integrity("Task version exhausted".to_owned()))?;
        let task_status_event = if task.task.status == "READY" {
            let task_event = command.task_status_event.ok_or_else(|| {
                StoreError::Invalid("Task status event is required for first planner activation".to_owned())
            })?;
            Some(EventDraft {
                event_id: task_event.event_id,
                workspace_id: command.workspace_id.clone(),
                entity_type: "Task".to_owned(),
                entity_id: task.task.task_id.clone(),
                origin_runtime_id: task_event.origin_runtime_id,
                entity_revision: next_task_version,
                hlc_timestamp: task_event.hlc_timestamp,
                correlation_id: event.correlation_id.clone(),
                causation_id: Some(event.event_id.clone()),
                schema_version: 1,
                event_type: "task.status.changed.v1".to_owned(),
                payload: json!({
                    "task_id": task.task.task_id,
                    "from": "READY",
                    "to": "RUNNING",
                    "reason_code": "LEAD_PLANNING_SESSION_READY",
                    "actor": {"service_id":"PlanningCoordinator"},
                    "aggregate_version": next_task_version,
                    "blocking_conditions": task.task.blocking_conditions,
                }),
                recorded_at: occurred_at.clone(),
            })
        } else {
            if command.task_status_event.is_some() {
                return Err(StoreError::Invalid("Task already RUNNING; status event must be absent".to_owned()));
            }
            None
        };
        let session_event = EventDraft {
            event_id: event.event_id,
            workspace_id: command.workspace_id.clone(),
            entity_type: "AgentSession".to_owned(),
            entity_id: session.agent_session_id.clone(),
            origin_runtime_id: event.origin_runtime_id,
            entity_revision: next_session_version,
            hlc_timestamp: event.hlc_timestamp,
            correlation_id: event.correlation_id,
            causation_id: event.causation_id,
            schema_version: 1,
            event_type: "agent.session.started.v1".to_owned(),
            payload: json!({
                "agent_session_id": session.agent_session_id,
                "scope": {"kind":"TASK_PLANNING", "task_id":task.task.task_id},
                "agent_binding_id": session.agent_binding_id,
                "endpoint_id": session.endpoint_id,
                "runtime_id": session.runtime_id,
                "runtime_incarnation_id": session.runtime_incarnation_id,
                "task_spec_revision": session.task_spec_revision,
                "session_state": "ACTIVE",
                "reported_at": occurred_at,
            }),
            recorded_at: occurred_at.clone(),
        };
        self.store.activate_task_planning_session(ActivatePlanningSessionCommit {
            workspace_id: command.workspace_id,
            agent_session_id: command.agent_session_id,
            expected_session_version: command.expected_session_version,
            expected_task_version: command.expected_task_version,
            occurred_at,
            host_instance_id: command.host_instance_id,
            native_session_ref: command.native_session_ref,
            session_event,
            task_status_event,
        })
    }
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, StoreError> {
    value.get(key).and_then(Value::as_str).ok_or_else(|| {
        StoreError::Invalid(format!("Task field {key} must be a string"))
    })
}

fn optional_string<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>, StoreError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(StoreError::Invalid(format!("Task field {key} must be a string or null"))),
    }
}

fn string_array(value: &Value, key: &str) -> Result<Vec<String>, StoreError> {
    let Some(value) = value.get(key) else { return Ok(Vec::new()) };
    let values = value.as_array().ok_or_else(|| StoreError::Invalid(format!("Task field {key} must be an array")))?;
    values.iter().map(|item| item.as_str().map(str::to_owned).ok_or_else(|| {
        StoreError::Invalid(format!("Task field {key} must contain only strings"))
    })).collect()
}

fn value_array(value: &Value, key: &str) -> Result<Vec<Value>, StoreError> {
    let Some(value) = value.get(key) else { return Ok(Vec::new()) };
    value.as_array().cloned().ok_or_else(|| StoreError::Invalid(format!("Task field {key} must be an array")))
}

fn optional_value(value: &Value, key: &str) -> Option<Value> {
    value.get(key).filter(|value| !value.is_null()).cloned()
}
