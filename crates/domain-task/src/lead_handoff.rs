//! Domain commands for Task lead handoff. The storage adapter owns atomic receipt,
//! Task CAS, policy revalidation, and the durable handoff event.

use domain_workspace::EventContext;
use serde_json::{Value, json};
pub use storage_core::CommittedTaskLeadHandoff;
use storage_core::{EventDraft, StoreError, TaskLeadHandoffCommit, TaskStore};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskLeadHandoffMode {
    OwnerRequested,
    PolicyFailover,
}

impl TaskLeadHandoffMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OwnerRequested => "OWNER_REQUEST",
            Self::PolicyFailover => "POLICY_FAILOVER",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskLeadHandoffActor {
    Principal { principal_id: String, kind: String },
    Service { service_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreshLeadTriggerObservation {
    pub trigger: String,
    pub affected_agent_binding_id: String,
    pub source_service_id: String,
    pub source_observation_ref: String,
    pub runtime_incarnation_id: Option<String>,
    pub observed_at: String,
    pub expires_at: Option<String>,
}

/// Authenticated command for the Task aggregate's dedicated lead transition.
/// The storage commit must repeat all mutable eligibility and policy checks inside
/// the same transaction that writes the Task, event, and idempotency receipt.
#[derive(Clone, Debug)]
pub struct TaskLeadHandoffCommand {
    pub workspace_id: String,
    pub task_id: String,
    pub request_scope_id: String,
    pub request_id: String,
    /// Canonical operation input, including path and `If-Match`, used to bind replay.
    pub request_payload: Value,
    pub expected_task_version: u64,
    pub expected_task_spec_revision: u64,
    pub target_agent_binding_id: String,
    pub mode: TaskLeadHandoffMode,
    pub actor: TaskLeadHandoffActor,
    pub trigger_observation: Option<FreshLeadTriggerObservation>,
    pub event: EventContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandoffCommandError {
    Invalid,
}

/// Exact payload for the canonical `task.lead_agent.changed.v1` event.
pub fn lead_handoff_event_payload(
    command: &TaskLeadHandoffCommand,
    previous_lead_agent_binding_id: &str,
    aggregate_version: u64,
) -> Result<Value, HandoffCommandError> {
    validate_handoff_command(command)?;
    if !valid_id(previous_lead_agent_binding_id)
        || previous_lead_agent_binding_id == command.target_agent_binding_id
        || command.expected_task_version == u64::MAX
        || aggregate_version != command.expected_task_version.saturating_add(1)
    {
        return Err(HandoffCommandError::Invalid);
    }
    let mut payload = json!({
        "task_id": command.task_id,
        "from_agent_binding_id": previous_lead_agent_binding_id,
        "to_agent_binding_id": command.target_agent_binding_id,
        "cause": command.mode.as_str(),
        "actor": actor_value(&command.actor),
        "task_spec_revision": command.expected_task_spec_revision,
        "aggregate_version": aggregate_version,
    });
    if let Some(observation) = &command.trigger_observation {
        payload["trigger_observation"] = observation_value(observation);
    }
    Ok(payload)
}

pub struct TaskLeadHandoffService<S> {
    store: S,
}

impl<S: TaskStore> TaskLeadHandoffService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn execute(
        &self,
        command: TaskLeadHandoffCommand,
    ) -> Result<CommittedTaskLeadHandoff, StoreError> {
        validate_handoff_command(&command)
            .map_err(|_| StoreError::Invalid("Task lead handoff command is invalid".to_owned()))?;
        let aggregate_version = command
            .expected_task_version
            .checked_add(1)
            .ok_or_else(|| StoreError::Invalid("Task version is exhausted".to_owned()))?;
        let mut event_payload = json!({
            "task_id": command.task_id,
            "to_agent_binding_id": command.target_agent_binding_id,
            "cause": command.mode.as_str(),
            "actor": actor_value(&command.actor),
            "task_spec_revision": command.expected_task_spec_revision,
            "aggregate_version": aggregate_version,
        });
        if let Some(observation) = &command.trigger_observation {
            event_payload["trigger_observation"] = observation_value(observation);
        }
        // The transaction fills `from_agent_binding_id` from the current Task row
        // after receipt replay and Task CAS; domain code must not prefetch mutable state.
        let event = EventDraft {
            event_id: command.event.event_id.clone(),
            workspace_id: command.workspace_id.clone(),
            entity_type: "Task".to_owned(),
            entity_id: command.task_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id.clone(),
            entity_revision: aggregate_version,
            hlc_timestamp: command.event.hlc_timestamp.clone(),
            correlation_id: command.event.correlation_id.clone(),
            causation_id: command.event.causation_id.clone(),
            schema_version: 1,
            event_type: "task.lead_agent.changed.v1".to_owned(),
            payload: event_payload,
            recorded_at: command.event.recorded_at.clone(),
        };
        self.store.commit_task_lead_handoff(TaskLeadHandoffCommit {
            workspace_id: command.workspace_id,
            task_id: command.task_id,
            request_scope_id: command.request_scope_id,
            request_id: command.request_id,
            request_payload: command.request_payload,
            expected_task_version: command.expected_task_version,
            expected_task_spec_revision: command.expected_task_spec_revision,
            target_agent_binding_id: command.target_agent_binding_id,
            cause: command.mode.as_str().to_owned(),
            actor: actor_value(&command.actor),
            trigger_observation: command.trigger_observation.as_ref().map(observation_value),
            event,
        })
    }
}

pub fn validate_handoff_command(
    command: &TaskLeadHandoffCommand,
) -> Result<(), HandoffCommandError> {
    if ![
        &command.workspace_id,
        &command.task_id,
        &command.request_scope_id,
        &command.request_id,
        &command.target_agent_binding_id,
        &command.event.event_id,
        &command.event.origin_runtime_id,
        &command.event.hlc_timestamp,
        &command.event.correlation_id,
        &command.event.recorded_at,
    ]
    .iter()
    .all(|value| valid_id(value))
        || command.expected_task_version == 0
        || command.expected_task_spec_revision == 0
        || command
            .event
            .causation_id
            .as_ref()
            .is_some_and(|value| !valid_id(value))
        || !valid_time(&command.event.hlc_timestamp)
        || !valid_time(&command.event.recorded_at)
    {
        return Err(HandoffCommandError::Invalid);
    }

    match (&command.mode, &command.actor, &command.trigger_observation) {
        (
            TaskLeadHandoffMode::OwnerRequested,
            TaskLeadHandoffActor::Principal { principal_id, kind },
            None,
        ) if principal_id == &command.request_scope_id && kind == "USER" => {}
        (
            TaskLeadHandoffMode::PolicyFailover,
            TaskLeadHandoffActor::Service { service_id },
            Some(observation),
        ) if valid_id(service_id)
            && service_id == &command.request_scope_id
            && valid_observation(observation, &command.event.recorded_at) => {}
        _ => return Err(HandoffCommandError::Invalid),
    }

    if command.request_payload != canonical_task_lead_handoff_payload(command) {
        return Err(HandoffCommandError::Invalid);
    }
    Ok(())
}

pub fn canonical_task_lead_handoff_payload(command: &TaskLeadHandoffCommand) -> Value {
    json!({
        "workspace_id": command.workspace_id,
        "task_id": command.task_id,
        "request_scope_id": command.request_scope_id,
        "expected_task_version": command.expected_task_version,
        "expected_task_spec_revision": command.expected_task_spec_revision,
        "target_agent_binding_id": command.target_agent_binding_id,
        "cause": command.mode.as_str(),
        "trigger_observation": command.trigger_observation.as_ref().map(observation_value),
    })
}

fn observation_value(observation: &FreshLeadTriggerObservation) -> Value {
    json!({
        "trigger": observation.trigger,
        "affected_agent_binding_id": observation.affected_agent_binding_id,
        "source": {"service_id": observation.source_service_id},
        "source_observation_ref": observation.source_observation_ref,
        "runtime_incarnation_id": observation.runtime_incarnation_id,
        "observed_at": observation.observed_at,
        "expires_at": observation.expires_at,
    })
}

fn actor_value(actor: &TaskLeadHandoffActor) -> Value {
    match actor {
        TaskLeadHandoffActor::Principal { principal_id, kind } => {
            json!({"principal_id": principal_id, "kind": kind})
        }
        TaskLeadHandoffActor::Service { service_id } => json!({"service_id": service_id}),
    }
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn valid_time(value: &str) -> bool {
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).is_ok()
}

fn valid_observation(observation: &FreshLeadTriggerObservation, now: &str) -> bool {
    let now = time::OffsetDateTime::parse(now, &time::format_description::well_known::Rfc3339).ok();
    let observed_at = time::OffsetDateTime::parse(
        &observation.observed_at,
        &time::format_description::well_known::Rfc3339,
    )
    .ok();
    let expires_at = observation.expires_at.as_ref().and_then(|expires| {
        time::OffsetDateTime::parse(expires, &time::format_description::well_known::Rfc3339).ok()
    });
    matches!(
        observation.trigger.as_str(),
        "AGENT_UNAVAILABLE" | "QUOTA_EXHAUSTED" | "RUNTIME_UNAVAILABLE"
    ) && valid_id(&observation.affected_agent_binding_id)
        && valid_id(&observation.source_service_id)
        && valid_id(&observation.source_observation_ref)
        && observation
            .runtime_incarnation_id
            .as_ref()
            .is_none_or(|value| valid_id(value))
        && (observation.trigger != "RUNTIME_UNAVAILABLE"
            || observation.runtime_incarnation_id.is_some())
        && now.is_some()
        && observed_at.is_some()
        && expires_at.is_some()
        && observed_at <= now
        && now < expires_at
        && observed_at < expires_at
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_workspace::EventContext;

    fn command(mode: TaskLeadHandoffMode) -> TaskLeadHandoffCommand {
        TaskLeadHandoffCommand {
            workspace_id: "workspace-a".into(),
            task_id: "task-a".into(),
            request_scope_id: "owner-a".into(),
            request_id: "request-a".into(),
            request_payload: serde_json::json!({
                "workspace_id": "workspace-a",
                "task_id": "task-a",
                "request_scope_id": "owner-a",
                "expected_task_version": 9,
                "target_agent_binding_id": "binding-b",
                "cause": mode.as_str(),
                "expected_task_spec_revision": 4,
                "trigger_observation": null
            }),
            expected_task_version: 9,
            expected_task_spec_revision: 4,
            target_agent_binding_id: "binding-b".into(),
            mode,
            actor: TaskLeadHandoffActor::Principal {
                principal_id: "owner-a".into(),
                kind: "USER".into(),
            },
            trigger_observation: None,
            event: EventContext {
                event_id: "event-a".into(),
                origin_runtime_id: "runtime-a".into(),
                hlc_timestamp: "2026-10-17T12:00:00Z".into(),
                correlation_id: "request-a".into(),
                causation_id: None,
                recorded_at: "2026-10-17T12:00:00Z".into(),
            },
        }
    }

    #[test]
    fn owner_handoff_requires_complete_scoped_command_and_no_provider_trigger() {
        let command = command(TaskLeadHandoffMode::OwnerRequested);
        assert_eq!(validate_handoff_command(&command), Ok(()));

        let mut invalid = command.clone();
        invalid.trigger_observation = Some(observation());
        assert_eq!(
            validate_handoff_command(&invalid),
            Err(HandoffCommandError::Invalid)
        );
    }

    #[test]
    fn policy_handoff_requires_a_source_observation() {
        let mut command = command(TaskLeadHandoffMode::PolicyFailover);
        command.actor = TaskLeadHandoffActor::Service {
            service_id: "lead-failover".into(),
        };
        command.request_scope_id = "lead-failover".into();
        command.request_payload["request_scope_id"] = serde_json::json!("lead-failover");
        assert_eq!(
            validate_handoff_command(&command),
            Err(HandoffCommandError::Invalid)
        );

        command.trigger_observation = Some(observation());
        command.request_payload["trigger_observation"] = observation_value(&observation());
        assert_eq!(validate_handoff_command(&command), Ok(()));
    }

    #[test]
    fn policy_handoff_rejects_missing_expiry_and_observations_outside_server_time() {
        let mut command = command(TaskLeadHandoffMode::PolicyFailover);
        command.actor = TaskLeadHandoffActor::Service {
            service_id: "lead-failover".into(),
        };
        command.request_scope_id = "lead-failover".into();
        command.request_payload = canonical_task_lead_handoff_payload(&command);

        let mut source = observation();
        source.expires_at = None;
        command.trigger_observation = Some(source.clone());
        command.request_payload = canonical_task_lead_handoff_payload(&command);
        assert_eq!(
            validate_handoff_command(&command),
            Err(HandoffCommandError::Invalid)
        );

        source = observation();
        source.expires_at = Some("2026-10-17T11:59:59Z".into());
        command.trigger_observation = Some(source.clone());
        command.request_payload = canonical_task_lead_handoff_payload(&command);
        assert_eq!(
            validate_handoff_command(&command),
            Err(HandoffCommandError::Invalid)
        );

        source = observation();
        source.observed_at = "2026-10-17T12:00:01Z".into();
        command.trigger_observation = Some(source);
        command.request_payload = canonical_task_lead_handoff_payload(&command);
        assert_eq!(
            validate_handoff_command(&command),
            Err(HandoffCommandError::Invalid)
        );

        source = observation();
        source.observed_at = "NaN".into();
        command.trigger_observation = Some(source.clone());
        command.request_payload = canonical_task_lead_handoff_payload(&command);
        assert_eq!(
            validate_handoff_command(&command),
            Err(HandoffCommandError::Invalid)
        );

        source = observation();
        source.expires_at = Some("Infinity".into());
        command.trigger_observation = Some(source);
        command.request_payload = canonical_task_lead_handoff_payload(&command);
        assert_eq!(
            validate_handoff_command(&command),
            Err(HandoffCommandError::Invalid)
        );
    }

    #[test]
    fn request_payload_must_match_effective_command_identity() {
        let mut command = command(TaskLeadHandoffMode::OwnerRequested);
        command.target_agent_binding_id = "binding-c".into();
        assert_eq!(
            validate_handoff_command(&command),
            Err(HandoffCommandError::Invalid)
        );
    }

    #[test]
    fn handoff_event_context_allows_independent_correlation_and_requires_valid_times() {
        let mut command = command(TaskLeadHandoffMode::OwnerRequested);
        command.event.correlation_id = "another-request".into();
        assert_eq!(validate_handoff_command(&command), Ok(()));
        command.event.recorded_at = "not-a-time".into();
        assert_eq!(
            validate_handoff_command(&command),
            Err(HandoffCommandError::Invalid)
        );
    }

    #[test]
    fn event_payload_matches_the_canonical_shape_and_omits_empty_observation() {
        let command = command(TaskLeadHandoffMode::OwnerRequested);
        let payload = lead_handoff_event_payload(&command, "binding-a", 10).unwrap();
        assert_eq!(
            payload,
            serde_json::json!({
                "task_id": "task-a",
                "from_agent_binding_id": "binding-a",
                "to_agent_binding_id": "binding-b",
                "cause": "OWNER_REQUEST",
                "actor": {"principal_id": "owner-a", "kind": "USER"},
                "task_spec_revision": 4,
                "aggregate_version": 10
            })
        );
    }

    fn observation() -> FreshLeadTriggerObservation {
        FreshLeadTriggerObservation {
            trigger: "QUOTA_EXHAUSTED".into(),
            affected_agent_binding_id: "binding-a".into(),
            source_service_id: "codex-adapter".into(),
            source_observation_ref: "quota-observation-a".into(),
            runtime_incarnation_id: None,
            observed_at: "2026-10-17T11:59:00Z".into(),
            expires_at: Some("2026-10-17T12:01:00Z".into()),
        }
    }
}
