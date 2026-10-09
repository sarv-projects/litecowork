use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InvocationStatus {
    Created,
    Dispatched,
    Waiting,
    InputRequired,
    CancelRequested,
    Succeeded,
    Failed,
    Cancelled,
    Ambiguous,
}

impl InvocationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Dispatched => "DISPATCHED",
            Self::Waiting => "WAITING",
            Self::InputRequired => "INPUT_REQUIRED",
            Self::CancelRequested => "CANCEL_REQUESTED",
            Self::Succeeded => "SUCCEEDED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
            Self::Ambiguous => "AMBIGUOUS",
        }
    }

    pub fn parse(value: &str) -> Result<Self, InvocationTransitionError> {
        match value {
            "CREATED" => Ok(Self::Created),
            "DISPATCHED" => Ok(Self::Dispatched),
            "WAITING" => Ok(Self::Waiting),
            "INPUT_REQUIRED" => Ok(Self::InputRequired),
            "CANCEL_REQUESTED" => Ok(Self::CancelRequested),
            "SUCCEEDED" => Ok(Self::Succeeded),
            "FAILED" => Ok(Self::Failed),
            "CANCELLED" => Ok(Self::Cancelled),
            "AMBIGUOUS" => Ok(Self::Ambiguous),
            _ => Err(InvocationTransitionError::InvalidRecord),
        }
    }

    pub const fn terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }

    pub const fn allows(self, next: Self) -> bool {
        use InvocationStatus::*;
        matches!(
            (self, next),
            (Created, Cancelled)
                | (
                    Dispatched,
                    Succeeded | Failed | Waiting | InputRequired | CancelRequested | Ambiguous
                )
                | (
                    Waiting,
                    InputRequired | Dispatched | CancelRequested | Ambiguous
                )
                | (
                    InputRequired,
                    Waiting | Dispatched | CancelRequested | Ambiguous
                )
                | (CancelRequested, Cancelled | Succeeded | Failed | Ambiguous)
                | (
                    Ambiguous,
                    Dispatched
                        | Waiting
                        | InputRequired
                        | CancelRequested
                        | Succeeded
                        | Failed
                        | Cancelled
                )
        )
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityInvocationRecord {
    pub invocation_id: String,
    pub workspace_id: String,
    pub scope_kind: String,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub attempt_id: Option<String>,
    pub agent_session_id: String,
    pub capability_grant_id: String,
    pub activation_id: String,
    pub capability_ref: Value,
    pub operation: String,
    pub request_digest: String,
    pub execution_method: String,
    pub action_batch_id: Option<String>,
    pub action_batch_ordinal: Option<u32>,
    pub action_batch_operation_count: Option<u32>,
    pub action_batch_digest: Option<String>,
    pub idempotency_key: Option<String>,
    pub status: InvocationStatus,
    pub provider_task_status: Option<String>,
    pub provider_task_created_at: Option<String>,
    pub provider_task_expires_at: Option<String>,
    pub provider_task_ttl_ms: Option<u64>,
    pub provider_poll_after_ms: Option<u64>,
    pub provider_updated_at: Option<String>,
    pub partial_result_refs: Value,
    pub result_refs: Value,
    pub effect_id: Option<String>,
    pub failure: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvocationTransitionError {
    InvalidRecord,
    InvalidTransition,
    DispatchAdmissionUnavailable,
    VersionOverflow,
}

impl CapabilityInvocationRecord {
    pub fn transition(
        &self,
        next: InvocationStatus,
        at: String,
    ) -> Result<Self, InvocationTransitionError> {
        self.validate()?;
        if next == InvocationStatus::Dispatched {
            return Err(InvocationTransitionError::DispatchAdmissionUnavailable);
        }
        if !self.status.allows(next) {
            return Err(InvocationTransitionError::InvalidTransition);
        }
        if at.trim().is_empty() || at.len() > 128 || at.chars().any(char::is_control) {
            return Err(InvocationTransitionError::InvalidRecord);
        }
        let version = self
            .version
            .checked_add(1)
            .ok_or(InvocationTransitionError::VersionOverflow)?;
        let mut updated = self.clone();
        updated.status = next;
        updated.updated_at = at.clone();
        updated.completed_at = next.terminal().then_some(at);
        updated.version = version;
        updated.validate()?;
        Ok(updated)
    }

    pub fn validate(&self) -> Result<(), InvocationTransitionError> {
        let valid = |value: &str| {
            !value.trim().is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
        };
        if !valid(&self.invocation_id)
            || !valid(&self.workspace_id)
            || !valid(&self.agent_session_id)
            || !valid(&self.capability_grant_id)
            || !valid(&self.activation_id)
            || !valid(&self.operation)
            || !valid(&self.request_digest)
            || !valid(&self.execution_method)
            || !valid(&self.created_at)
            || !valid(&self.updated_at)
            || self.version == 0
            || self.capability_ref.is_null()
            || !self.capability_ref.is_object()
            || !self.partial_result_refs.is_array()
            || !self.result_refs.is_array()
            || (self.status.terminal() != self.completed_at.is_some())
            || self
                .completed_at
                .as_ref()
                .is_some_and(|value| !valid(value))
        {
            return Err(InvocationTransitionError::InvalidRecord);
        }
        match self.scope_kind.as_str() {
            "CONVERSATION"
                if self.conversation_id.is_some()
                    && self.task_id.is_none()
                    && self.attempt_id.is_none()
                    && self.effect_id.is_none() => {}
            "TASK_PLANNING"
                if self.conversation_id.is_none()
                    && self.task_id.is_some()
                    && self.attempt_id.is_none()
                    && self.effect_id.is_none() => {}
            "ATTEMPT_EXECUTION"
                if self.conversation_id.is_none()
                    && self.task_id.is_some()
                    && self.attempt_id.is_some() => {}
            _ => return Err(InvocationTransitionError::InvalidRecord),
        }
        if self.status.terminal() != self.completed_at.is_some() {
            return Err(InvocationTransitionError::InvalidRecord);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation(status: InvocationStatus) -> CapabilityInvocationRecord {
        CapabilityInvocationRecord {
            invocation_id: "inv-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            scope_kind: "CONVERSATION".to_owned(),
            conversation_id: Some("conv-1".to_owned()),
            task_id: None,
            attempt_id: None,
            agent_session_id: "session-1".to_owned(),
            capability_grant_id: "grant-1".to_owned(),
            activation_id: "activation-1".to_owned(),
            capability_ref: serde_json::json!({"package_id":"pkg","component_id":"comp"}),
            operation: "read".to_owned(),
            request_digest: format!("sha256:{}", "a".repeat(64)),
            execution_method: "DETERMINISTIC_LOCAL".to_owned(),
            action_batch_id: None,
            action_batch_ordinal: None,
            action_batch_operation_count: None,
            action_batch_digest: None,
            idempotency_key: None,
            status,
            provider_task_status: None,
            provider_task_created_at: None,
            provider_task_expires_at: None,
            provider_task_ttl_ms: None,
            provider_poll_after_ms: None,
            provider_updated_at: None,
            partial_result_refs: serde_json::json!([]),
            result_refs: serde_json::json!([]),
            effect_id: None,
            failure: None,
            created_at: "2026-10-09T10:00:00Z".to_owned(),
            updated_at: "2026-10-09T10:00:00Z".to_owned(),
            completed_at: None,
            version: 1,
        }
    }

    #[test]
    fn permits_pre_dispatch_local_cancellation() {
        let next = invocation(InvocationStatus::Created)
            .transition(
                InvocationStatus::Cancelled,
                "2026-10-09T10:01:00Z".to_owned(),
            )
            .expect("undispatched invocation can settle locally");
        assert_eq!(next.status, InvocationStatus::Cancelled);
        assert_eq!(next.version, 2);
        assert_eq!(next.completed_at.as_deref(), Some("2026-10-09T10:01:00Z"));
    }

    #[test]
    fn rejects_any_dispatch_transition_without_combined_admission() {
        assert_eq!(
            invocation(InvocationStatus::Created)
                .transition(
                    InvocationStatus::Dispatched,
                    "2026-10-09T10:01:00Z".to_owned()
                )
                .unwrap_err(),
            InvocationTransitionError::DispatchAdmissionUnavailable
        );
    }

    #[test]
    fn terminal_invocation_cannot_be_rewritten() {
        let terminal = CapabilityInvocationRecord {
            completed_at: Some("2026-10-09T10:00:00Z".to_owned()),
            ..invocation(InvocationStatus::Failed)
        };
        assert_eq!(
            terminal.transition(
                InvocationStatus::Succeeded,
                "2026-10-09T10:01:00Z".to_owned()
            ),
            Err(InvocationTransitionError::InvalidTransition)
        );
    }
}
