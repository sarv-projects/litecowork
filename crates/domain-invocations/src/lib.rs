mod capability_bridge;

pub use capability_bridge::{
    AgentCapabilityBridgeInput, AgentCapabilityRoute, CapabilityBridgeDecision,
    classify_agent_capability_route,
};

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
    ReconciliationProofRequired,
    InvalidReconciliationObservation,
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
        if self.status == InvocationStatus::Ambiguous {
            return Err(InvocationTransitionError::ReconciliationProofRequired);
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

    /// Apply a terminal result after the storage boundary has authenticated the
    /// caller and bound its provider observation to this exact Invocation.
    /// This is only a state transform: persistence and provider authentication
    /// remain the responsibility of the owning adapter and store transaction.
    pub fn reconcile_ambiguous_terminal(
        &self,
        next: InvocationStatus,
        observed_at: String,
    ) -> Result<Self, InvocationTransitionError> {
        self.validate()?;
        if self.status != InvocationStatus::Ambiguous || !next.terminal() {
            return Err(InvocationTransitionError::InvalidReconciliationObservation);
        }
        if observed_at.trim().is_empty()
            || observed_at.len() > 128
            || observed_at.chars().any(char::is_control)
        {
            return Err(InvocationTransitionError::InvalidRecord);
        }
        let version = self
            .version
            .checked_add(1)
            .ok_or(InvocationTransitionError::VersionOverflow)?;
        let mut reconciled = self.clone();
        reconciled.status = next;
        reconciled.provider_task_status = Some(match next {
            InvocationStatus::Succeeded => "COMPLETED".to_owned(),
            InvocationStatus::Failed => "FAILED".to_owned(),
            InvocationStatus::Cancelled => "CANCELLED".to_owned(),
            _ => unreachable!("only terminal statuses are accepted"),
        });
        reconciled.provider_updated_at = Some(observed_at.clone());
        reconciled.updated_at = observed_at.clone();
        reconciled.completed_at = Some(observed_at);
        reconciled.version = version;
        reconciled.validate()?;
        Ok(reconciled)
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
            || self
                .idempotency_key
                .as_deref()
                .is_some_and(|key| !valid_idempotency_key(key))
            || (self.status.terminal() != self.completed_at.is_some())
            || self
                .completed_at
                .as_ref()
                .is_some_and(|value| !valid(value))
        {
            return Err(InvocationTransitionError::InvalidRecord);
        }
        let batch_metadata = [
            self.action_batch_id.is_some(),
            self.action_batch_ordinal.is_some(),
            self.action_batch_operation_count.is_some(),
            self.action_batch_digest.is_some(),
        ];
        if batch_metadata.iter().any(|present| *present)
            && (!batch_metadata.iter().all(|present| *present)
                || self
                    .action_batch_id
                    .as_deref()
                    .is_none_or(|id| !valid(id) || id.len() > 256)
                || self
                    .action_batch_operation_count
                    .is_none_or(|count| !(1..=64).contains(&count))
                || self
                    .action_batch_ordinal
                    .zip(self.action_batch_operation_count)
                    .is_none_or(|(ordinal, count)| ordinal >= count)
                || self
                    .action_batch_digest
                    .as_deref()
                    .is_none_or(|digest| !valid_sha256_digest(digest))
                || self.idempotency_key.is_none())
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

fn valid_idempotency_key(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}

fn valid_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
    fn rejects_partial_or_oversized_action_batch_metadata() {
        let mut record = invocation(InvocationStatus::Created);
        record.action_batch_id = Some("batch-1".into());
        assert_eq!(
            record.validate(),
            Err(InvocationTransitionError::InvalidRecord)
        );

        let mut record = invocation(InvocationStatus::Created);
        record.action_batch_id = Some("batch-1".into());
        record.action_batch_ordinal = Some(0);
        record.action_batch_operation_count = Some(65);
        record.action_batch_digest = Some(format!("sha256:{}", "b".repeat(64)));
        record.idempotency_key = Some("idem-1".into());
        assert_eq!(
            record.validate(),
            Err(InvocationTransitionError::InvalidRecord)
        );
    }

    #[test]
    fn validates_batch_member_position_digest_and_idempotency_identity() {
        let mut record = invocation(InvocationStatus::Created);
        record.action_batch_id = Some("batch-1".into());
        record.action_batch_ordinal = Some(1);
        record.action_batch_operation_count = Some(2);
        record.action_batch_digest = Some(format!("sha256:{}", "b".repeat(64)));
        record.idempotency_key = Some("member-2".into());
        assert_eq!(record.validate(), Ok(()));

        record.action_batch_ordinal = Some(2);
        assert_eq!(
            record.validate(),
            Err(InvocationTransitionError::InvalidRecord)
        );
        record.action_batch_ordinal = Some(1);
        record.action_batch_digest = Some(format!("sha256:{}", "B".repeat(64)));
        assert_eq!(
            record.validate(),
            Err(InvocationTransitionError::InvalidRecord)
        );
        record.action_batch_digest = Some(format!("sha256:{}", "b".repeat(64)));
        record.idempotency_key = None;
        assert_eq!(
            record.validate(),
            Err(InvocationTransitionError::InvalidRecord)
        );
    }

    #[test]
    fn rejects_malformed_idempotency_key_even_without_action_batch() {
        let mut record = invocation(InvocationStatus::Created);
        record.idempotency_key = Some("\n".into());
        assert_eq!(
            record.validate(),
            Err(InvocationTransitionError::InvalidRecord)
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

    #[test]
    fn ambiguous_invocations_cannot_become_terminal_through_generic_transition() {
        for terminal in [
            InvocationStatus::Succeeded,
            InvocationStatus::Failed,
            InvocationStatus::Cancelled,
        ] {
            assert_eq!(
                invocation(InvocationStatus::Ambiguous)
                    .transition(terminal, "2026-10-09T10:01:00Z".to_owned()),
                Err(InvocationTransitionError::ReconciliationProofRequired),
                "generic transitions must not settle ambiguous provider state as {terminal:?}"
            );
        }
    }

    #[test]
    fn reconciliation_state_transform_sets_provider_terminal_projection() {
        for terminal in [
            InvocationStatus::Succeeded,
            InvocationStatus::Failed,
            InvocationStatus::Cancelled,
        ] {
            let record = invocation(InvocationStatus::Ambiguous);
            let reconciled = record
                .reconcile_ambiguous_terminal(terminal, "2026-10-09T10:02:00Z".to_owned())
                .unwrap();
            assert_eq!(reconciled.status, terminal);
            assert_eq!(reconciled.version, 2);
            assert_eq!(
                reconciled.completed_at.as_deref(),
                Some("2026-10-09T10:02:00Z")
            );
            assert_eq!(
                reconciled.provider_task_status.as_deref(),
                Some(match terminal {
                    InvocationStatus::Succeeded => "COMPLETED",
                    InvocationStatus::Failed => "FAILED",
                    InvocationStatus::Cancelled => "CANCELLED",
                    _ => unreachable!(),
                })
            );
        }
        assert_eq!(
            invocation(InvocationStatus::Ambiguous).reconcile_ambiguous_terminal(
                InvocationStatus::Waiting,
                "2026-10-09T10:02:00Z".to_owned()
            ),
            Err(InvocationTransitionError::InvalidReconciliationObservation)
        );
    }
}
