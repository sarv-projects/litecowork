//! Core-owned Effect and Evidence values.
//!
//! This crate defines domain validation and transitions only. It does not authorize
//! provider calls, dispatch capabilities, issue grants, or reconcile external state.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EffectState {
    Proposed,
    Started,
    Acknowledged,
    Reconciling,
    Observed,
    Verified,
    Failed,
    Ambiguous,
}

impl EffectState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "PROPOSED",
            Self::Started => "STARTED",
            Self::Acknowledged => "ACKNOWLEDGED",
            Self::Reconciling => "RECONCILING",
            Self::Observed => "OBSERVED",
            Self::Verified => "VERIFIED",
            Self::Failed => "FAILED",
            Self::Ambiguous => "AMBIGUOUS",
        }
    }

    pub const fn allows(self, next: Self) -> bool {
        use EffectState::*;
        matches!(
            (self, next),
            (Proposed, Started | Failed)
                | (Started, Acknowledged | Failed | Ambiguous)
                | (Acknowledged, Observed | Failed | Ambiguous)
                | (Observed, Verified | Ambiguous)
                | (Ambiguous, Reconciling)
                | (Reconciling, Observed | Failed | Ambiguous | Started)
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceLevel {
    Reported,
    Observed,
    Verified,
}

impl EvidenceLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reported => "REPORTED",
            Self::Observed => "OBSERVED",
            Self::Verified => "VERIFIED",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionMethod {
    StructuredApi,
    StructuredBrowser,
    AccessibilityBrowser,
    ScreenComputerUse,
    DeterministicLocal,
    NativeAgentTool,
    Unknown,
}

impl ExecutionMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StructuredApi => "STRUCTURED_API",
            Self::StructuredBrowser => "STRUCTURED_BROWSER",
            Self::AccessibilityBrowser => "ACCESSIBILITY_BROWSER",
            Self::ScreenComputerUse => "SCREEN_COMPUTER_USE",
            Self::DeterministicLocal => "DETERMINISTIC_LOCAL",
            Self::NativeAgentTool => "NATIVE_AGENT_TOOL",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRecord {
    pub effect_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub capability_ref: Option<Value>,
    pub operation: String,
    /// A `ResourceRef` object or an opaque, non-empty provider target string.
    pub target: Value,
    pub idempotency_key: Option<String>,
    pub state: EffectState,
    pub request_digest: String,
    pub capability_invocation_id: String,
    pub execution_method: ExecutionMethod,
    pub dispatch_ordinal: u32,
    pub result_ref: Option<Value>,
    pub observed_state: Option<Value>,
    pub verification_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

impl EffectRecord {
    pub fn validate(&self) -> Result<(), EffectError> {
        for value in [
            self.effect_id.as_str(),
            self.task_id.as_str(),
            self.attempt_id.as_str(),
            self.operation.as_str(),
            self.capability_invocation_id.as_str(),
            self.created_at.as_str(),
            self.updated_at.as_str(),
        ] {
            if value.trim().is_empty() || value.len() > 4096 || value.chars().any(char::is_control)
            {
                return Err(EffectError::InvalidRecord);
            }
        }
        if !valid_digest(&self.request_digest)
            || self.version == 0
            || self.dispatch_ordinal > 1_000_000
        {
            return Err(EffectError::InvalidRecord);
        }
        if self
            .verification_ref
            .as_ref()
            .is_some_and(|reference| invalid_bounded(reference, 256))
            || (self.state == EffectState::Verified && self.verification_ref.is_none())
        {
            return Err(EffectError::InvalidRecord);
        }
        if self.operation.len() > 512
            || self
                .idempotency_key
                .as_ref()
                .is_some_and(|key| invalid_bounded(key, 1024))
        {
            return Err(EffectError::InvalidRecord);
        }
        if !valid_target(&self.target)
            || !self.capability_ref.as_ref().is_none_or(Value::is_object)
            || !self.result_ref.as_ref().is_none_or(Value::is_object)
            || !self.observed_state.as_ref().is_none_or(Value::is_object)
        {
            return Err(EffectError::InvalidRecord);
        }
        if (self.state == EffectState::Proposed && self.dispatch_ordinal != 0)
            || (!matches!(self.state, EffectState::Proposed | EffectState::Failed)
                && self.dispatch_ordinal == 0)
        {
            return Err(EffectError::InvalidRecord);
        }
        Ok(())
    }

    pub fn transition(&self, next: EffectState, at: String) -> Result<Self, EffectError> {
        if !self.state.allows(next) {
            return Err(EffectError::InvalidTransition);
        }
        if self.state == EffectState::Reconciling && next == EffectState::Started {
            return Err(EffectError::RetryNotAuthorized);
        }
        self.transition_validated(next, at)
    }

    /// Return to STARTED only after a persisted reconciliation decision proves the
    /// prior dispatch did not occur or a stable provider idempotency key makes retry safe.
    pub fn authorize_retry(
        &self,
        at: String,
        reconciliation_evidence_ref: &str,
        confirmed_not_occurred: bool,
        same_key_idempotent: bool,
    ) -> Result<Self, EffectError> {
        if self.state != EffectState::Reconciling || reconciliation_evidence_ref.trim().is_empty() {
            return Err(EffectError::RetryNotAuthorized);
        }
        if !confirmed_not_occurred && !(same_key_idempotent && self.idempotency_key.is_some()) {
            return Err(EffectError::RetryNotAuthorized);
        }
        self.transition_validated(EffectState::Started, at)
    }

    fn transition_validated(&self, next: EffectState, at: String) -> Result<Self, EffectError> {
        if invalid_bounded(&at, 128) {
            return Err(EffectError::InvalidRecord);
        }
        let mut updated = self.clone();
        updated.state = next;
        updated.updated_at = at;
        updated.version = self
            .version
            .checked_add(1)
            .ok_or(EffectError::VersionOverflow)?;
        if next == EffectState::Started {
            updated.dispatch_ordinal = self
                .dispatch_ordinal
                .checked_add(1)
                .ok_or(EffectError::VersionOverflow)?;
        }
        updated.validate()?;
        Ok(updated)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub evidence_id: String,
    pub task_id: String,
    pub subject_ref: String,
    pub level: EvidenceLevel,
    pub kind: String,
    pub producer: Value,
    pub payload_ref: Option<Value>,
    pub payload_digest: Option<String>,
    pub created_at: String,
}

impl EvidenceRecord {
    pub fn validate(&self) -> Result<(), EffectError> {
        for value in [
            self.evidence_id.as_str(),
            self.task_id.as_str(),
            self.subject_ref.as_str(),
            self.kind.as_str(),
            self.created_at.as_str(),
        ] {
            if invalid_bounded(value, 4096) {
                return Err(EffectError::InvalidRecord);
            }
        }
        if !valid_producer(&self.producer)
            || self
                .payload_ref
                .as_ref()
                .is_some_and(|value| !valid_resource_ref(value))
            || self
                .payload_digest
                .as_ref()
                .is_some_and(|digest| !valid_digest(digest))
        {
            return Err(EffectError::InvalidRecord);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectError {
    InvalidRecord,
    InvalidTransition,
    RetryNotAuthorized,
    MissingVerificationEvidence,
    VersionOverflow,
}

impl std::fmt::Display for EffectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for EffectError {}

fn invalid_bounded(value: &str, max: usize) -> bool {
    value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control)
}
fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_target(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|target| !invalid_bounded(target, 4096))
        || value.as_object().is_some_and(|object| {
            object
                .keys()
                .all(|key| ["workspace_id", "resource_id", "revision_id"].contains(&key.as_str()))
                && valid_resource_ref(value)
        })
}
fn valid_resource_ref(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        object
            .keys()
            .all(|key| ["workspace_id", "resource_id", "revision_id"].contains(&key.as_str()))
            && object
                .get("workspace_id")
                .and_then(Value::as_str)
                .is_some_and(|s| !invalid_bounded(s, 256))
            && object
                .get("resource_id")
                .and_then(Value::as_str)
                .is_some_and(|s| !invalid_bounded(s, 256))
            && object
                .get("revision_id")
                .is_none_or(|r| r.is_null() || r.as_str().is_some_and(|s| !invalid_bounded(s, 256)))
    })
}
fn valid_producer(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    match (
        object.get("principal_id"),
        object.get("kind"),
        object.get("service_id"),
    ) {
        (Some(Value::String(id)), Some(Value::String(kind)), None) => {
            object.len() == 2
                && !invalid_bounded(id, 256)
                && ["USER", "SERVICE", "RUNTIME", "AGENT", "CHANNEL_IDENTITY"]
                    .contains(&kind.as_str())
        }
        (None, None, Some(Value::String(id))) => object.len() == 1 && !invalid_bounded(id, 256),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn proposed() -> EffectRecord {
        EffectRecord {
            effect_id: "effect-1".into(),
            task_id: "task-1".into(),
            attempt_id: "attempt-1".into(),
            capability_ref: Some(json!({"package_id":"mail","capability_id":"send"})),
            operation: "message.send".into(),
            target: json!("mailbox:user@example.test"),
            idempotency_key: Some("stable-1".into()),
            state: EffectState::Proposed,
            request_digest: format!("sha256:{}", "a".repeat(64)),
            capability_invocation_id: "inv-1".into(),
            execution_method: ExecutionMethod::StructuredApi,
            dispatch_ordinal: 0,
            result_ref: None,
            observed_state: None,
            verification_ref: None,
            created_at: "2026-10-08T00:00:00Z".into(),
            updated_at: "2026-10-08T00:00:00Z".into(),
            version: 1,
        }
    }

    #[test]
    fn first_dispatch_follows_persisted_proposal_and_advances_ordinal() {
        let started = proposed()
            .transition(EffectState::Started, "2026-10-08T00:01:00Z".into())
            .unwrap();
        assert_eq!(started.dispatch_ordinal, 1);
        assert_eq!(started.version, 2);
    }

    #[test]
    fn proposed_effect_cannot_skip_started_to_observed() {
        assert_eq!(
            proposed().transition(EffectState::Observed, "2026-10-08T00:01:00Z".into()),
            Err(EffectError::InvalidTransition)
        );
    }

    #[test]
    fn proposal_can_fail_before_dispatch_without_claiming_a_dispatch() {
        let failed = proposed()
            .transition(EffectState::Failed, "2026-10-08T00:01:00Z".into())
            .unwrap();
        assert_eq!(failed.dispatch_ordinal, 0);
        assert!(failed.validate().is_ok());
    }

    #[test]
    fn retry_requires_reconciliation_proof_and_stable_key_when_using_idempotency_basis() {
        let started = proposed()
            .transition(EffectState::Started, "2026-10-08T00:01:00Z".into())
            .unwrap();
        let ambiguous = started
            .transition(EffectState::Ambiguous, "2026-10-08T00:02:00Z".into())
            .unwrap();
        let reconciling = ambiguous
            .transition(EffectState::Reconciling, "2026-10-08T00:03:00Z".into())
            .unwrap();
        assert_eq!(
            reconciling.transition(EffectState::Started, "2026-10-08T00:04:00Z".into()),
            Err(EffectError::RetryNotAuthorized),
        );
        assert_eq!(
            reconciling.authorize_retry("2026-10-08T00:04:00Z".into(), "evidence-1", false, false),
            Err(EffectError::RetryNotAuthorized),
        );
        let retried = reconciling
            .authorize_retry("2026-10-08T00:04:00Z".into(), "evidence-1", true, false)
            .unwrap();
        assert_eq!(retried.dispatch_ordinal, 2);
    }

    #[test]
    fn evidence_requires_digest_shape_and_producer_object() {
        let evidence = EvidenceRecord {
            evidence_id: "ev-1".into(),
            task_id: "task-1".into(),
            subject_ref: "effect:effect-1".into(),
            level: EvidenceLevel::Reported,
            kind: "provider_result".into(),
            producer: json!({"service_id":"adapter"}),
            payload_ref: None,
            payload_digest: None,
            created_at: "2026-10-08T00:00:00Z".into(),
        };
        assert!(evidence.validate().is_ok());
    }
}
