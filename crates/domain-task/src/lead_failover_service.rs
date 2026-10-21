//! Production caller for the pinned-lead failover policy.
//!
//! This service turns one authenticated provider/Runtime observation into either an
//! owner-choice request, a typed blocker, or a `POLICY_FAILOVER` handoff command. It is
//! not authority by itself: the SQLite handoff transaction replays the receipt, re-reads
//! the pinned policy, the current lead, the ordered fallback eligibility, the Task
//! version/spec, and the safe boundary before it writes anything. A new lead session is
//! still created separately through the ordinary planning-session admission path, which
//! rechecks endpoint, Runtime incarnation, auth and Trust.

use crate::{
    CommittedTaskLeadHandoff, FailoverCandidate, FailoverCandidateReadiness, FailoverDecision,
    FailoverPolicy, FailoverTrigger, FailoverTriggerObservation, FailoverTriggerState,
    FreshLeadTriggerObservation, LeadFailoverBlocker, LeadHandoffPacket, LeadHandoffSourcePins,
    PreparedLeadHandoffPacket, TaskLeadHandoffActor, TaskLeadHandoffCommand, TaskLeadHandoffMode,
    TaskLeadHandoffService, canonical_task_lead_handoff_payload, decide_lead_failover,
    prepare_lead_handoff_packet, validate_failover_policy_value,
};
use domain_workspace::EventContext;
use storage_core::{AgentCatalogStore, StoreError, TaskStore};

/// Caller-supplied proof that the placement layer checked this exact fallback binding
/// on the given Task version/TaskSpec revision and Runtime incarnation. The service
/// rechecks durable binding scope/enablement; the storage transaction rechecks policy,
/// order and the safe boundary. Trust, auth, budget and deadline remain admission
/// prerequisites for the replacement session and are not granted here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeadFailoverCandidateProof {
    pub agent_binding_id: String,
    pub placement_qualified: bool,
}

/// The durable facts a replacement lead resumes from, plus the caller's authorized
/// reference allowsets. The service derives the Task, TaskSpec, Plan and Step facts
/// itself from the store; the caller supplies the references it independently
/// resolved and authorized.
#[derive(Clone, Debug, PartialEq)]
pub struct LeadFailoverHandoffInputs {
    /// Exact accepted PlanRevision the replacement lead continues. `None` when the
    /// lead changes before any plan is accepted.
    pub plan_revision: Option<u64>,
    /// Committed decision events the caller resolved and authorized.
    pub relevant_decisions: Vec<crate::BoundedDecision>,
    /// Artifact versions the caller resolved and authorized.
    pub artifact_refs: Vec<crate::ArtifactVersionRef>,
    /// Evidence IDs the caller resolved and authorized.
    pub evidence_refs: Vec<String>,
    /// Open Effect IDs the caller resolved and authorized.
    pub open_effect_refs: Vec<String>,
    /// Current blockers the replacement lead must see.
    pub blockers: Vec<crate::DelegationBlocker>,
    /// Remaining budget the replacement lead must respect.
    pub remaining_budget: Option<crate::DelegationBudgetSpec>,
    /// Why the lead changed. Bounded and free of provider or native detail.
    pub reason: String,
    /// Context budget for the canonical projection bytes.
    pub context_budget_bytes: usize,
}

/// A durable observation is confirmed only by the adapter/Runtime that owns the
/// affected binding. `source` must equal the authenticated service identity.
#[derive(Clone, Debug)]
pub struct LeadFailoverCommand {
    pub owner_principal_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub expected_task_version: u64,
    pub expected_task_spec_revision: u64,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    /// The current lead's Runtime incarnation when the caller knows it. Required for a
    /// `RUNTIME_UNAVAILABLE` observation, which must name the lost incarnation.
    pub current_lead_runtime_incarnation_id: Option<String>,
    pub service_id: String,
    pub request_id: String,
    pub observation: FailoverTriggerObservation,
    pub candidates: Vec<LeadFailoverCandidateProof>,
    /// The durable facts and authorized references the replacement lead resumes from.
    /// A committed failover always produces the bounded handoff projection.
    pub handoff: LeadFailoverHandoffInputs,
    pub event: EventContext,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LeadFailoverOutcome {
    Disabled,
    AskOwner {
        trigger: FailoverTrigger,
        observation_id: String,
        /// Ordered fallback choices to present to the owner, in pinned policy order.
        /// Only a fallback that was freshly checked for this exact Task version and
        /// TaskSpec revision appears here.
        fallback_agent_binding_ids: Vec<String>,
    },
    Committed {
        trigger: FailoverTrigger,
        observation_id: String,
        committed: CommittedTaskLeadHandoff,
        /// The bounded, portable projection the replacement lead resumes from. It
        /// carries only durable references and bounded summaries: no transcript,
        /// hidden reasoning, native session handle, provider handle, or secret.
        handoff_packet: PreparedLeadHandoffPacket,
    },
    Blocked(LeadFailoverBlocker),
}

pub struct LeadFailoverService<S> {
    store: S,
}

impl<S> LeadFailoverService<S>
where
    S: TaskStore + AgentCatalogStore + Clone,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Evaluate the pinned policy against one fresh observation and, only for an
    /// `ALLOW_LISTED` recommendation, execute the version-fenced handoff command.
    pub fn evaluate(
        &self,
        command: LeadFailoverCommand,
    ) -> Result<LeadFailoverOutcome, StoreError> {
        validate_command(&command)?;
        let view = self
            .store
            .get_task(&command.workspace_id, &command.task_id)?
            .ok_or(StoreError::NotFound)?;
        if view.task.version != command.expected_task_version {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_task_version),
                actual: Some(view.task.version),
            });
        }
        if view.task.current_spec_revision != command.expected_task_spec_revision {
            return Err(StoreError::Invalid("STALE_SPEC_REVISION".to_owned()));
        }
        if !validate_failover_policy_value(&view.current_spec_revision.lead_failover_policy) {
            return Err(StoreError::Integrity(
                "stored Task lead failover policy is invalid".to_owned(),
            ));
        }
        let policy: FailoverPolicy =
            serde_json::from_value(view.current_spec_revision.lead_failover_policy.clone())
                .map_err(|_| {
                    StoreError::Integrity("stored Task lead failover policy is invalid".to_owned())
                })?;
        let candidates = self.candidate_projections(&command, &policy, &view)?;
        // `TASK-RUNTIME.md` requires an automatic lead change to settle or safely
        // checkpoint the prior planning session and reconcile Invocations/Effects
        // while preserving active Attempts under their original bindings/leases. The
        // durable transaction independently rechecks that boundary; this check only
        // rejects a Task that can no longer make progress.
        let safe_boundary = matches!(view.task.status.as_str(), "READY" | "RUNNING")
            && view.task.completed_at.is_none();
        // The durable transaction owns the prior-change count. Passing zero here can
        // only produce a recommendation that the transaction then rejects when the
        // pinned change budget is exhausted; it cannot grant extra authority.
        let decision = decide_lead_failover(
            &policy,
            &command.observation,
            &view.task.lead_agent_binding_id,
            command.current_lead_runtime_incarnation_id.as_deref(),
            view.task.version,
            view.task.current_spec_revision,
            0,
            &command.event.recorded_at,
            safe_boundary,
            &candidates,
        );
        match decision {
            FailoverDecision::Disabled => Ok(LeadFailoverOutcome::Disabled),
            FailoverDecision::AskOwner {
                trigger,
                observation_id,
                fallback_agent_binding_ids,
            } => Ok(LeadFailoverOutcome::AskOwner {
                trigger,
                observation_id,
                fallback_agent_binding_ids,
            }),
            FailoverDecision::Blocked(blocker) => Ok(LeadFailoverOutcome::Blocked(blocker)),
            FailoverDecision::Recommend {
                agent_binding_id,
                trigger,
                observation_id,
                ..
            } => {
                let handoff = self.policy_handoff_command(
                    &command,
                    agent_binding_id,
                    trigger,
                    &observation_id,
                )?;
                let committed = TaskLeadHandoffService::new(self.store.clone()).execute(handoff)?;
                let handoff_packet = self.handoff_packet(&command, &view)?;
                Ok(LeadFailoverOutcome::Committed {
                    trigger,
                    observation_id,
                    committed,
                    handoff_packet,
                })
            }
        }
    }

    /// Build the bounded, portable projection the replacement lead resumes from. The
    /// Task, TaskSpec and Plan identity come from the durable store; the caller
    /// supplies the references it independently resolved and authorized. The
    /// projection never carries a transcript, hidden reasoning, native session handle,
    /// provider handle, or secret, and it is never truncated to fit its budget.
    fn handoff_packet(
        &self,
        command: &LeadFailoverCommand,
        view: &storage_core::TaskView,
    ) -> Result<PreparedLeadHandoffPacket, StoreError> {
        let plan_revision = view.task.current_plan_revision;
        let steps = match plan_revision {
            Some(revision) => self
                .store
                .list_steps(&command.workspace_id, &command.task_id, Some(revision))?
                .into_iter()
                .map(|step| crate::LeadHandoffStep {
                    step_id: step.step_id,
                    logical_key: step.logical_key,
                    title: step.title,
                    objective: step.objective,
                    status: step.status,
                })
                .collect(),
            None => Vec::new(),
        };
        let pins = LeadHandoffSourcePins {
            workspace_id: command.workspace_id.clone(),
            task_id: command.task_id.clone(),
            task_spec_revision: view.current_spec_revision.revision,
            plan_revision,
            authorized_artifacts: command.handoff.artifact_refs.iter().cloned().collect(),
            committed_decision_event_ids: command
                .handoff
                .relevant_decisions
                .iter()
                .map(|decision| decision.source_event_id.clone())
                .collect(),
            authorized_evidence_ids: command.handoff.evidence_refs.iter().cloned().collect(),
            open_effect_ids: command.handoff.open_effect_refs.iter().cloned().collect(),
        };
        let packet = LeadHandoffPacket {
            workspace_id: command.workspace_id.clone(),
            task_id: command.task_id.clone(),
            task_spec_revision: view.current_spec_revision.revision,
            plan_revision,
            objective: view.current_spec_revision.objective.clone(),
            constraints: view.current_spec_revision.constraints.clone(),
            steps,
            relevant_decisions: command.handoff.relevant_decisions.clone(),
            artifact_refs: command.handoff.artifact_refs.clone(),
            evidence_refs: command.handoff.evidence_refs.clone(),
            open_effect_refs: command.handoff.open_effect_refs.clone(),
            blockers: command.handoff.blockers.clone(),
            remaining_budget: command.handoff.remaining_budget.clone(),
            reason: command.handoff.reason.clone(),
        };
        prepare_lead_handoff_packet(packet, &pins, command.handoff.context_budget_bytes).map_err(
            |error| StoreError::Invalid(format!("handoff projection is invalid: {error:?}")),
        )
    }

    fn candidate_projections(
        &self,
        command: &LeadFailoverCommand,
        policy: &FailoverPolicy,
        view: &storage_core::TaskView,
    ) -> Result<Vec<FailoverCandidate>, StoreError> {
        let mut candidates = Vec::with_capacity(policy.fallback_agent_binding_ids.len());
        // The durable handoff transaction independently derives the next fallback from
        // the pinned policy order: the first same-Workspace binding that is enabled
        // and lead-eligible, excluding the current lead. This projection must select
        // the same binding so a domain recommendation and a transaction commit can
        // never disagree about which fallback is next.
        for binding_id in &policy.fallback_agent_binding_ids {
            if *binding_id == view.task.lead_agent_binding_id {
                continue;
            }
            let binding = self.store.get_agent_binding(
                &command.owner_principal_id,
                &command.workspace_id,
                binding_id,
            )?;
            let durable_eligible = binding.as_ref().is_some_and(|binding| {
                binding.workspace_id == command.workspace_id
                    && binding.enabled
                    && binding.lead_eligible
            });
            if !durable_eligible {
                // Not an eligible fallback at all. The durable transaction skips it, so
                // it must not appear as a candidate and must not block the change.
                continue;
            }
            let placement_qualified = command.candidates.iter().any(|candidate| {
                candidate.agent_binding_id == *binding_id && candidate.placement_qualified
            });
            // Runtime pinning, endpoint, auth, Trust, input availability and budget
            // are rechecked for the binding at use time. A binding that is pinned to
            // another Runtime cannot host the replacement lead.
            let runtime_compatible = binding.as_ref().is_none_or(|binding| {
                binding
                    .runtime_id
                    .as_deref()
                    .is_none_or(|runtime| runtime == command.runtime_id)
            });
            candidates.push(FailoverCandidate {
                agent_binding_id: binding_id.clone(),
                readiness: if placement_qualified && runtime_compatible {
                    FailoverCandidateReadiness::Qualified
                } else {
                    FailoverCandidateReadiness::Ineligible
                },
                checked_task_version: view.task.version,
                checked_task_spec_revision: view.task.current_spec_revision,
            });
        }
        Ok(candidates)
    }

    fn policy_handoff_command(
        &self,
        command: &LeadFailoverCommand,
        target_agent_binding_id: String,
        trigger: FailoverTrigger,
        observation_id: &str,
    ) -> Result<TaskLeadHandoffCommand, StoreError> {
        let expires_at = command.observation.expires_at.clone();
        if command.observation.source != command.service_id
            || command.observation.source_observation_id != observation_id
        {
            return Err(StoreError::Invalid(
                "lead failover observation source does not match the service".to_owned(),
            ));
        }
        let observation = FreshLeadTriggerObservation {
            trigger: trigger.as_str().to_owned(),
            affected_agent_binding_id: command.observation.affected_agent_binding_id.clone(),
            source_service_id: command.observation.source.clone(),
            source_observation_ref: command.observation.source_observation_id.clone(),
            runtime_incarnation_id: command.observation.runtime_incarnation_id.clone(),
            observed_at: command.observation.observed_at.clone(),
            expires_at: Some(expires_at),
        };
        let mut handoff = TaskLeadHandoffCommand {
            workspace_id: command.workspace_id.clone(),
            task_id: command.task_id.clone(),
            request_scope_id: command.service_id.clone(),
            request_id: command.request_id.clone(),
            request_payload: serde_json::Value::Null,
            expected_task_version: command.expected_task_version,
            expected_task_spec_revision: command.expected_task_spec_revision,
            target_agent_binding_id,
            mode: TaskLeadHandoffMode::PolicyFailover,
            actor: TaskLeadHandoffActor::Service {
                service_id: command.service_id.clone(),
            },
            trigger_observation: Some(observation),
            event: command.event.clone(),
        };
        handoff.request_payload = canonical_task_lead_handoff_payload(&handoff);
        Ok(handoff)
    }
}

fn validate_command(command: &LeadFailoverCommand) -> Result<(), StoreError> {
    let ids = [
        command.owner_principal_id.as_str(),
        command.workspace_id.as_str(),
        command.task_id.as_str(),
        command.runtime_id.as_str(),
        command.runtime_incarnation_id.as_str(),
        command.service_id.as_str(),
        command.request_id.as_str(),
        command.event.event_id.as_str(),
        command.event.origin_runtime_id.as_str(),
        command.event.hlc_timestamp.as_str(),
        command.event.correlation_id.as_str(),
        command.event.recorded_at.as_str(),
    ];
    if ids.iter().any(|value| {
        value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control)
    }) || command.expected_task_version == 0
        || command.expected_task_spec_revision == 0
        || command
            .event
            .causation_id
            .as_ref()
            .is_some_and(|value| !valid_id(value))
        || command.observation.state != FailoverTriggerState::Confirmed
        || command.observation.source != command.service_id
        || command
            .current_lead_runtime_incarnation_id
            .as_deref()
            .is_some_and(|value| !valid_id(value))
        || !valid_id(&command.observation.affected_agent_binding_id)
        || !valid_id(&command.observation.source_observation_id)
        || command
            .observation
            .runtime_incarnation_id
            .as_deref()
            .is_some_and(|value| !valid_id(value))
        || (command.observation.trigger == FailoverTrigger::RuntimeUnavailable
            && command.observation.runtime_incarnation_id.is_none())
        || parse_time(&command.observation.observed_at).is_none()
        || parse_time(&command.observation.expires_at).is_none()
    {
        return Err(StoreError::Invalid(
            "lead failover command is invalid".to_owned(),
        ));
    }
    let unique_candidates = command
        .candidates
        .iter()
        .map(|candidate| candidate.agent_binding_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if unique_candidates.len() != command.candidates.len()
        || command
            .candidates
            .iter()
            .any(|candidate| !valid_id(&candidate.agent_binding_id))
    {
        return Err(StoreError::Invalid(
            "lead failover candidate proofs are ambiguous".to_owned(),
        ));
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn parse_time(value: &str) -> Option<time::OffsetDateTime> {
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return None;
    }
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_workspace::EventContext;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use storage_core::{
        AgentBindingCreateRequest, AgentBindingDisableRequest, AgentBindingEnableRequest,
        AgentBindingRecord, AgentProfileRecord, AgentProfileViewRecord, CommittedAgentBinding,
        CommittedTask, CommittedTaskSpecRevision, LocalAgentEndpointBindingInput,
        LocalAgentEndpointBindingRecord, PlanAcceptance, PlanAcceptanceCommit, PlanRevisionRecord,
        PlannedStepRecord, RuntimeCapabilityHostPageRequest, RuntimeCapabilityHostRecord,
        RuntimeOfferPageRequest, RuntimeOfferRecord, StepRecord, TaskRecord,
        TaskSpecRevisionCommit, TaskSpecRevisionRecord, TaskView,
    };

    #[derive(Clone, Default)]
    struct CaptureStore {
        task: Arc<Mutex<Option<TaskView>>>,
        bindings: Arc<Mutex<Vec<AgentBindingRecord>>>,
        commits: Arc<Mutex<Vec<storage_core::TaskLeadHandoffCommit>>>,
        plans: Arc<Mutex<Vec<PlanRevisionRecord>>>,
        steps: Arc<Mutex<Vec<StepRecord>>>,
    }

    impl CaptureStore {
        fn with_policy(policy: Value) -> Self {
            let store = Self::default();
            *store.task.lock().unwrap() = Some(view(policy));
            for id in ["fallback-a", "fallback-b"] {
                store.bindings.lock().unwrap().push(binding(id, true));
            }
            store.plans.lock().unwrap().push(PlanRevisionRecord {
                task_id: "task-a".into(),
                revision: 2,
                task_spec_revision: 3,
                produced_by_agent_session_id: "session-a".into(),
                produced_by_attempt_id: None,
                steps: vec![PlannedStepRecord {
                    logical_key: "scan".into(),
                    title: "Scan".into(),
                    objective: "Scan the repository".into(),
                    depends_on_logical_keys: vec![],
                    required_capabilities: vec![],
                    acceptance_criteria: vec![],
                }],
                reason_for_revision: None,
                created_at: "2026-10-17T09:00:00Z".into(),
            });
            store.steps.lock().unwrap().push(StepRecord {
                step_id: "step-a".into(),
                task_id: "task-a".into(),
                plan_revision: 2,
                logical_key: Some("scan".into()),
                title: "Scan".into(),
                objective: "Scan the repository".into(),
                dependencies: vec![],
                required_capabilities: vec![],
                acceptance_criteria: vec![],
                status: "READY".into(),
                current_attempt_id: None,
                created_at: "2026-10-17T09:00:00Z".into(),
                updated_at: "2026-10-17T09:00:00Z".into(),
                version: 1,
            });
            store
        }

        fn commits(&self) -> Vec<storage_core::TaskLeadHandoffCommit> {
            self.commits.lock().unwrap().clone()
        }
    }

    fn binding(id: &str, eligible: bool) -> AgentBindingRecord {
        AgentBindingRecord {
            agent_binding_id: id.into(),
            workspace_id: "workspace-a".into(),
            agent_profile_id: "profile-a".into(),
            runtime_id: Some("runtime-a".into()),
            endpoint_selection_policy: json!({"mode":"AUTO_COMPATIBLE"}),
            auth_ref: None,
            configuration: json!({}),
            enabled: eligible,
            lead_eligible: eligible,
            created_at: "2026-10-17T09:00:00Z".into(),
            version: 1,
        }
    }

    fn view(policy: Value) -> TaskView {
        TaskView {
            task: TaskRecord {
                task_id: "task-a".into(),
                workspace_id: "workspace-a".into(),
                conversation_id: None,
                current_spec_revision: 3,
                current_plan_revision: Some(2),
                status: "READY".into(),
                resume_status: None,
                routine_id: None,
                routine_revision: None,
                automation_id: None,
                automation_occurrence_id: None,
                origin_coworker_id: None,
                origin_coworker_revision: None,
                lead_agent_binding_id: "lead".into(),
                blocking_conditions: vec![],
                priority: "NORMAL".into(),
                created_by: json!({"kind":"USER","principal_id":"owner-a"}),
                created_at: "2026-10-17T09:00:00Z".into(),
                updated_at: "2026-10-17T09:00:00Z".into(),
                completed_at: None,
                version: 7,
            },
            current_spec_revision: TaskSpecRevisionRecord {
                task_id: "task-a".into(),
                workspace_id: "workspace-a".into(),
                revision: 3,
                parent_revisions: vec![2],
                objective: "objective".into(),
                task_category: None,
                constraints: vec![],
                non_goals: vec![],
                input_refs: vec![],
                workspace_instruction_revision: None,
                required_outputs: vec![],
                acceptance_criteria: vec![],
                approvals_required: vec![],
                budget: None,
                delegation_budget_policy: None,
                lead_failover_policy: policy,
                deadline: None,
                source_message_refs: vec![],
                placement_preference: json!("AUTO"),
                preferred_lead_agent_binding_id: Some("lead".into()),
                authored_by: json!({"kind":"USER","principal_id":"owner-a"}),
                created_at: "2026-10-17T09:00:00Z".into(),
            },
        }
    }

    fn policy() -> Value {
        json!({
            "mode":"ALLOW_LISTED",
            "triggers":["QUOTA_EXHAUSTED"],
            "fallback_agent_binding_ids":["fallback-a","fallback-b"],
            "max_lead_changes":2
        })
    }

    fn command(_policy: Value) -> LeadFailoverCommand {
        LeadFailoverCommand {
            owner_principal_id: "owner-a".into(),
            workspace_id: "workspace-a".into(),
            task_id: "task-a".into(),
            expected_task_version: 7,
            expected_task_spec_revision: 3,
            runtime_id: "runtime-a".into(),
            runtime_incarnation_id: "incarnation-a".into(),
            current_lead_runtime_incarnation_id: Some("incarnation-a".into()),
            service_id: "quota-observer".into(),
            request_id: "request-a".into(),
            observation: observation(),
            candidates: vec![
                LeadFailoverCandidateProof {
                    agent_binding_id: "fallback-a".into(),
                    placement_qualified: true,
                },
                LeadFailoverCandidateProof {
                    agent_binding_id: "fallback-b".into(),
                    placement_qualified: true,
                },
            ],
            handoff: LeadFailoverHandoffInputs {
                plan_revision: Some(2),
                relevant_decisions: vec![],
                artifact_refs: vec![],
                evidence_refs: vec![],
                open_effect_refs: vec![],
                blockers: vec![],
                remaining_budget: None,
                reason: "The pinned lead reported a quota exhaustion.".into(),
                context_budget_bytes: 64 * 1024,
            },
            event: EventContext {
                event_id: "event-a".into(),
                origin_runtime_id: "runtime-a".into(),
                hlc_timestamp: "2026-10-17T12:00:30Z".into(),
                correlation_id: "correlation-a".into(),
                causation_id: None,
                recorded_at: "2026-10-17T12:00:30Z".into(),
            },
        }
    }

    fn observation() -> FailoverTriggerObservation {
        FailoverTriggerObservation {
            trigger: FailoverTrigger::QuotaExhausted,
            state: FailoverTriggerState::Confirmed,
            affected_agent_binding_id: "lead".into(),
            source: "quota-observer".into(),
            source_observation_id: "quota-1".into(),
            runtime_incarnation_id: Some("incarnation-a".into()),
            observed_at: "2026-10-17T12:00:00Z".into(),
            expires_at: "2026-10-17T12:01:00Z".into(),
        }
    }

    impl TaskStore for CaptureStore {
        fn create_task(
            &self,
            _: storage_core::TaskCreateCommit,
        ) -> Result<CommittedTask, StoreError> {
            unimplemented!()
        }
        fn get_task_create_receipt(
            &self,
            _: &str,
            _: &str,
            _: &Value,
        ) -> Result<Option<CommittedTask>, StoreError> {
            unimplemented!()
        }
        fn get_task_spec_revision_receipt(
            &self,
            _: &str,
            _: &str,
            _: &Value,
        ) -> Result<Option<CommittedTaskSpecRevision>, StoreError> {
            unimplemented!()
        }
        fn revise_task_spec(
            &self,
            _: TaskSpecRevisionCommit,
        ) -> Result<CommittedTaskSpecRevision, StoreError> {
            unimplemented!()
        }
        fn commit_task_lead_handoff(
            &self,
            commit: storage_core::TaskLeadHandoffCommit,
        ) -> Result<CommittedTaskLeadHandoff, StoreError> {
            self.commits.lock().unwrap().push(commit.clone());
            let mut view = self.task.lock().unwrap().clone().unwrap();
            view.task.lead_agent_binding_id = commit.target_agent_binding_id.clone();
            view.task.version += 1;
            Ok(CommittedTaskLeadHandoff {
                previous_lead_agent_binding_id: "lead".into(),
                target_agent_binding_id: commit.target_agent_binding_id,
                task: view.task,
                task_spec_revision: commit.expected_task_spec_revision,
                aggregate_version: commit.expected_task_version + 1,
                replayed: false,
            })
        }
        fn accept_initial_plan(
            &self,
            _: PlanAcceptanceCommit,
        ) -> Result<PlanAcceptance, StoreError> {
            unimplemented!()
        }
        fn list_plan_revisions(
            &self,
            _workspace_id: &str,
            task_id: &str,
        ) -> Result<Vec<PlanRevisionRecord>, StoreError> {
            Ok(self
                .plans
                .lock()
                .unwrap()
                .iter()
                .filter(|plan| plan.task_id == task_id)
                .cloned()
                .collect())
        }
        fn list_steps(
            &self,
            _workspace_id: &str,
            task_id: &str,
            plan_revision: Option<u64>,
        ) -> Result<Vec<StepRecord>, StoreError> {
            Ok(self
                .steps
                .lock()
                .unwrap()
                .iter()
                .filter(|step| step.task_id == task_id)
                .filter(|step| plan_revision.is_none_or(|revision| step.plan_revision == revision))
                .cloned()
                .collect())
        }
        fn get_task(&self, _: &str, _: &str) -> Result<Option<TaskView>, StoreError> {
            Ok(self.task.lock().unwrap().clone())
        }
        fn list_task_spec_revisions(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Vec<TaskSpecRevisionRecord>, StoreError> {
            unimplemented!()
        }
        fn list_tasks_page(
            &self,
            _: &str,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<&str>,
            _: usize,
        ) -> Result<Vec<storage_core::TaskSummaryRecord>, StoreError> {
            unimplemented!()
        }
    }

    impl AgentCatalogStore for CaptureStore {
        fn put_agent_profile(
            &self,
            _: AgentProfileRecord,
            _: Vec<storage_core::AgentEndpointRecord>,
        ) -> Result<(), StoreError> {
            unimplemented!()
        }
        fn register_local_endpoint_binding(
            &self,
            _: LocalAgentEndpointBindingInput,
        ) -> Result<(), StoreError> {
            unimplemented!()
        }
        fn get_local_endpoint_binding(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<Option<LocalAgentEndpointBindingRecord>, StoreError> {
            unimplemented!()
        }
        fn publish_runtime_offer(&self, _: RuntimeOfferRecord) -> Result<(), StoreError> {
            unimplemented!()
        }
        fn list_runtime_offers(
            &self,
            _: RuntimeOfferPageRequest,
        ) -> Result<Vec<RuntimeOfferRecord>, StoreError> {
            unimplemented!()
        }
        fn list_runtime_capability_hosts(
            &self,
            _: RuntimeCapabilityHostPageRequest,
        ) -> Result<Vec<RuntimeCapabilityHostRecord>, StoreError> {
            unimplemented!()
        }
        fn list_agent_profiles(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<Vec<AgentProfileViewRecord>, StoreError> {
            unimplemented!()
        }
        fn create_agent_binding(
            &self,
            _: AgentBindingCreateRequest,
        ) -> Result<CommittedAgentBinding, StoreError> {
            unimplemented!()
        }
        fn get_agent_binding(
            &self,
            _: &str,
            _: &str,
            agent_binding_id: &str,
        ) -> Result<Option<AgentBindingRecord>, StoreError> {
            Ok(self
                .bindings
                .lock()
                .unwrap()
                .iter()
                .find(|binding| binding.agent_binding_id == agent_binding_id)
                .cloned())
        }
        fn list_agent_bindings(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Vec<AgentBindingRecord>, StoreError> {
            Ok(self.bindings.lock().unwrap().clone())
        }
        fn get_agent_binding_mutation_receipt(
            &self,
            _: &str,
            _: &str,
            _: &Value,
        ) -> Result<Option<CommittedAgentBinding>, StoreError> {
            unimplemented!()
        }
        fn enable_agent_binding(
            &self,
            _: AgentBindingEnableRequest,
        ) -> Result<CommittedAgentBinding, StoreError> {
            unimplemented!()
        }
        fn disable_agent_binding(
            &self,
            _: AgentBindingDisableRequest,
        ) -> Result<CommittedAgentBinding, StoreError> {
            unimplemented!()
        }
    }

    #[test]
    fn qualified_ordered_candidate_commits_a_policy_failover_command() {
        let store = CaptureStore::with_policy(policy());
        let outcome = LeadFailoverService::new(store.clone())
            .evaluate(command(policy()))
            .expect("policy failover commits");
        match outcome {
            LeadFailoverOutcome::Committed {
                trigger,
                observation_id,
                committed,
                handoff_packet,
            } => {
                assert_eq!(trigger, FailoverTrigger::QuotaExhausted);
                assert_eq!(observation_id, "quota-1");
                assert_eq!(committed.target_agent_binding_id, "fallback-a");
                assert_eq!(handoff_packet.packet.task_id, "task-a");
                assert_eq!(handoff_packet.packet.task_spec_revision, 3);
                assert_eq!(handoff_packet.packet.plan_revision, Some(2));
                assert_eq!(handoff_packet.packet.steps[0].step_id, "step-a");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let commits = store.commits();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].cause, "POLICY_FAILOVER");
        assert_eq!(commits[0].target_agent_binding_id, "fallback-a");
        assert_eq!(commits[0].actor, json!({"service_id":"quota-observer"}));
        assert_eq!(
            commits[0].trigger_observation.as_ref().unwrap()["source_observation_ref"],
            "quota-1"
        );
        assert_eq!(commits[0].event.payload["cause"], "POLICY_FAILOVER");
    }

    #[test]
    fn ask_and_disabled_outcomes_never_commit() {
        let ask = json!({
            "mode":"ASK",
            "triggers":["QUOTA_EXHAUSTED"],
            "fallback_agent_binding_ids":["fallback-a"],
            "max_lead_changes":0
        });
        let store = CaptureStore::with_policy(ask.clone());
        assert!(matches!(
            LeadFailoverService::new(store.clone())
                .evaluate(command(ask))
                .unwrap(),
            LeadFailoverOutcome::AskOwner { .. }
        ));
        assert!(store.commits().is_empty());

        let disabled = json!({
            "mode":"DISABLED",
            "triggers":[],
            "fallback_agent_binding_ids":[],
            "max_lead_changes":0
        });
        let store = CaptureStore::with_policy(disabled.clone());
        assert_eq!(
            LeadFailoverService::new(store.clone())
                .evaluate(command(disabled))
                .unwrap(),
            LeadFailoverOutcome::Disabled
        );
        assert!(store.commits().is_empty());
    }

    #[test]
    fn stale_wrong_lead_or_unqualified_observations_block_before_storage() {
        let store = CaptureStore::with_policy(policy());
        let mut stale = command(policy());
        stale.observation.expires_at = "2026-10-17T12:00:00Z".into();
        assert_eq!(
            LeadFailoverService::new(store.clone())
                .evaluate(stale)
                .unwrap(),
            LeadFailoverOutcome::Blocked(LeadFailoverBlocker::ObservationStale)
        );

        let mut wrong_lead = command(policy());
        wrong_lead.observation.affected_agent_binding_id = "other-lead".into();
        assert_eq!(
            LeadFailoverService::new(store.clone())
                .evaluate(wrong_lead)
                .unwrap(),
            LeadFailoverOutcome::Blocked(LeadFailoverBlocker::ObservationDoesNotMatchCurrentLead)
        );

        let mut unqualified = command(policy());
        unqualified.candidates.clear();
        assert_eq!(
            LeadFailoverService::new(store.clone())
                .evaluate(unqualified)
                .unwrap(),
            LeadFailoverOutcome::Blocked(LeadFailoverBlocker::NoEligibleFallback)
        );
        assert!(store.commits().is_empty());
    }

    #[test]
    fn unavailable_pinned_fallback_is_skipped_only_for_a_later_eligible_candidate() {
        let store = CaptureStore::with_policy(policy());
        *store.bindings.lock().unwrap() =
            vec![binding("fallback-a", false), binding("fallback-b", true)];
        let mut command = command(policy());
        command.candidates = vec![LeadFailoverCandidateProof {
            agent_binding_id: "fallback-b".into(),
            placement_qualified: true,
        }];
        let outcome = LeadFailoverService::new(store.clone())
            .evaluate(command)
            .unwrap();
        match outcome {
            LeadFailoverOutcome::Committed { committed, .. } => {
                assert_eq!(committed.target_agent_binding_id, "fallback-b");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn placement_unavailable_first_fallback_blocks_instead_of_promoting_a_later_one() {
        let store = CaptureStore::with_policy(policy());
        let mut command = command(policy());
        command.candidates = vec![LeadFailoverCandidateProof {
            agent_binding_id: "fallback-b".into(),
            placement_qualified: true,
        }];
        assert_eq!(
            LeadFailoverService::new(store.clone())
                .evaluate(command)
                .unwrap(),
            LeadFailoverOutcome::Blocked(LeadFailoverBlocker::NoEligibleFallback)
        );
        assert!(store.commits().is_empty());
    }

    #[test]
    fn malformed_or_mismatched_source_identity_is_rejected_before_any_commit() {
        let store = CaptureStore::with_policy(policy());
        let mut mismatched_source = command(policy());
        mismatched_source.observation.source = "another-service".into();
        assert!(
            LeadFailoverService::new(store.clone())
                .evaluate(mismatched_source)
                .is_err()
        );

        let mut duplicate_candidate = command(policy());
        duplicate_candidate
            .candidates
            .push(LeadFailoverCandidateProof {
                agent_binding_id: "fallback-a".into(),
                placement_qualified: true,
            });
        assert!(
            LeadFailoverService::new(store.clone())
                .evaluate(duplicate_candidate)
                .is_err()
        );
        assert!(store.commits().is_empty());
    }

    #[test]
    fn stale_task_or_spec_versions_fail_before_evaluation() {
        let store = CaptureStore::with_policy(policy());
        let mut stale_task = command(policy());
        stale_task.expected_task_version = 6;
        assert!(matches!(
            LeadFailoverService::new(store.clone()).evaluate(stale_task),
            Err(StoreError::Conflict { .. })
        ));
        let mut stale_spec = command(policy());
        stale_spec.expected_task_spec_revision = 2;
        assert!(matches!(
            LeadFailoverService::new(store.clone()).evaluate(stale_spec),
            Err(StoreError::Invalid(ref code)) if code == "STALE_SPEC_REVISION"
        ));
        assert!(store.commits().is_empty());
    }

    #[test]
    fn storage_failure_propagates_and_never_reports_a_commit() {
        let store = CaptureStore::with_policy(policy());
        // An ineligible persisted policy is an integrity problem, not a silent switch.
        *store.task.lock().unwrap() = Some(view(json!({"mode":"ALLOW_LISTED"})));
        assert!(matches!(
            LeadFailoverService::new(store.clone()).evaluate(command(policy())),
            Err(StoreError::Integrity(_))
        ));
        assert!(store.commits().is_empty());
    }
}
