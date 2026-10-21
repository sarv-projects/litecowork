//! Core-internal Trust policy evaluation and approval-request production.
//!
//! Policy evaluation is a pure decision primitive. The approval producer persists
//! only an exact Task/Attempt-bound request through the transactional ApprovalStore;
//! neither path dispatches providers, issues grants/SecretLeases, nor creates an
//! independently reusable permit. CapabilityBroker admission remains fail-closed
//! until its authorization/effect/audit transaction is implemented.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use storage_core::{
    ApprovalActor, ApprovalRecord, ApprovalStore, EventDraft, RequestApprovalCommit, StoreError,
};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

const APPROVAL_TTL_MINUTES: i64 = 15;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        pub(crate) struct $name(pub(crate) String);

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

id_type!(WorkspaceId);
id_type!(PrincipalId);
id_type!(CapabilityGrantId);
id_type!(AgentSessionId);
id_type!(RuntimeId);
id_type!(RuntimeIncarnationId);
id_type!(InvocationId);
id_type!(ActivationId);
id_type!(OperationId);
id_type!(EffectId);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CapabilityIdentityKind {
    PackageComponent,
    McpSkill,
}

/// Canonical CapabilityRef value. Keeping the normalized source/version/digest
/// identity typed here prevents an approval target from degrading to a display name
/// or capability ID string.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilityRef {
    pub(crate) capability_id: String,
    pub(crate) identity_kind: CapabilityIdentityKind,
    pub(crate) source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) package_version: Option<String>,
    pub(crate) digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) component: Option<String>,
}

impl From<&str> for CapabilityRef {
    fn from(value: &str) -> Self {
        // Compact test fixture constructor. Production callers supply all fields
        // observed from the normalized provider/package identity.
        Self {
            capability_id: value.to_owned(),
            identity_kind: CapabilityIdentityKind::PackageComponent,
            source: "test:litespm".to_owned(),
            package_version: Some("1.0.0".to_owned()),
            digest: digest_bytes(b"test package digest"),
            component: None,
        }
    }
}

impl CapabilityRef {
    fn validate(&self) -> bool {
        let valid_text = |value: &str, maximum: usize| {
            !value.trim().is_empty()
                && value.len() <= maximum
                && !value.chars().any(char::is_control)
        };
        if !valid_text(&self.capability_id, 256)
            || !valid_text(&self.source, 1024)
            || !is_sha256_digest(&self.digest)
        {
            return false;
        }
        match self.identity_kind {
            CapabilityIdentityKind::PackageComponent => {
                self.package_version
                    .as_deref()
                    .is_some_and(|value| valid_text(value, 256))
                    && self
                        .component
                        .as_deref()
                        .is_none_or(|value| valid_text(value, 1024))
            }
            CapabilityIdentityKind::McpSkill => {
                self.package_version.is_none()
                    && self
                        .component
                        .as_deref()
                        .is_some_and(|value| valid_text(value, 2048))
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub(crate) enum AuthorizationScope {
    Conversation {
        conversation_id: String,
        conversation_turn_id: String,
    },
    TaskPlanning {
        task_id: String,
    },
    AttemptExecution {
        task_id: String,
        attempt_id: String,
    },
}

impl AuthorizationScope {
    pub(crate) fn conversation(conversation_id: &str, conversation_turn_id: &str) -> Self {
        Self::Conversation {
            conversation_id: conversation_id.to_owned(),
            conversation_turn_id: conversation_turn_id.to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ExecutionMethod {
    StructuredApi,
    StructuredBrowser,
    AccessibilityBrowser,
    ScreenComputerUse,
    DeterministicLocal,
    NativeAgentTool,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum PrincipalKind {
    User,
    Service,
    Runtime,
    Agent,
    ChannelIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrincipalRef {
    pub(crate) principal_id: PrincipalId,
    pub(crate) kind: PrincipalKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuthorizationRequest {
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) principal: PrincipalRef,
    pub(crate) agent_session_id: AgentSessionId,
    pub(crate) runtime_id: RuntimeId,
    pub(crate) runtime_incarnation_id: RuntimeIncarnationId,
    pub(crate) scope: AuthorizationScope,
    pub(crate) invocation_id: InvocationId,
    pub(crate) capability_grant_id: CapabilityGrantId,
    pub(crate) activation_id: ActivationId,
    pub(crate) capability_ref: CapabilityRef,
    pub(crate) operation: OperationId,
    pub(crate) input_digest: String,
    pub(crate) request_digest: String,
    pub(crate) target_digest: String,
    pub(crate) execution_method: ExecutionMethod,
    pub(crate) effect_id: Option<EffectId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrustPolicyVersion {
    pub(crate) policy_id: String,
    pub(crate) revision: u32,
    pub(crate) content_digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum DecisionKind {
    Allow,
    Deny,
    RequireApproval,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ReasonCode {
    PrincipalMismatch,
    SessionMismatch,
    ScopeMismatch,
    GrantMismatch,
    ActivationMismatch,
    OperationMismatch,
    RuntimeMismatch,
    WorkspaceMismatch,
    InvocationMismatch,
    CapabilityMismatch,
    RequestDigestMismatch,
    TargetDigestMismatch,
    EffectMismatch,
    SessionNotActive,
    ScopeNotActive,
    GrantNotActive,
    ActivationNotActive,
    EffectRequired,
    EffectUnexpected,
    OperationNotClassified,
    ExecutionMethodUnknown,
    SecretLeaseInvalid,
    EffectRequiresAttemptScope,
    WorkspacePolicyDenied,
    ApprovalBindingMismatch,
    ApprovalDenied,
    ExecutionMethodNotAllowed,
    ApprovalRequired,
    ApprovalRequiredByPinnedPolicy,
    AllowedReadOnlyOperation,
    PolicyUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionDetails {
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) request_digest: String,
    pub(crate) action_digest: String,
    pub(crate) scope_digest: String,
    pub(crate) evaluation_context_digest: String,
    pub(crate) invocation_id: InvocationId,
    pub(crate) capability_grant_id: CapabilityGrantId,
    pub(crate) activation_id: ActivationId,
    pub(crate) effect_id: Option<EffectId>,
    pub(crate) decision: DecisionKind,
    pub(crate) reason_code: ReasonCode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VersionedEvaluation {
    pub(crate) policy_version: TrustPolicyVersion,
    pub(crate) details: DecisionDetails,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PolicyUnavailableDenial {
    pub(crate) details: DecisionDetails,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "evaluation",
    content = "result",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub(crate) enum PolicyEvaluation {
    Versioned(VersionedEvaluation),
    Unavailable(PolicyUnavailableDenial),
}

impl PolicyEvaluation {
    pub(crate) fn decision(&self) -> DecisionKind {
        self.details().decision
    }

    pub(crate) fn reason_code(&self) -> ReasonCode {
        self.details().reason_code
    }

    pub(crate) fn details(&self) -> &DecisionDetails {
        match self {
            Self::Versioned(decision) => &decision.details,
            Self::Unavailable(denial) => &denial.details,
        }
    }

    fn request_digest(&self) -> &str {
        &self.details().request_digest
    }

    fn action_digest(&self) -> &str {
        &self.details().action_digest
    }

    fn scope_digest(&self) -> &str {
        &self.details().scope_digest
    }

    fn evaluation_context_digest(&self) -> &str {
        &self.details().evaluation_context_digest
    }

    fn invocation_id(&self) -> &InvocationId {
        &self.details().invocation_id
    }

    fn capability_grant_id(&self) -> &CapabilityGrantId {
        &self.details().capability_grant_id
    }

    fn activation_id(&self) -> &ActivationId {
        &self.details().activation_id
    }

    fn effect_id(&self) -> Option<&EffectId> {
        self.details().effect_id.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum OperationDisposition {
    Allow,
    RequireApproval,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OperationRule {
    disposition: OperationDisposition,
    allowed_execution_methods: Vec<ExecutionMethod>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PolicyBundleBody {
    policy_id: String,
    revision: u32,
    operation_rules: BTreeMap<String, OperationRule>,
}

#[derive(Clone, Debug)]
struct PolicyBundle {
    body: PolicyBundleBody,
    content_digest: String,
}

impl PolicyBundle {
    #[cfg(test)]
    fn test_bundle(policy_id: &str, disposition: OperationDisposition) -> Self {
        let mut operation_rules = BTreeMap::new();
        operation_rules.insert(
            "workspace.resource.read".to_owned(),
            OperationRule {
                disposition,
                allowed_execution_methods: vec![ExecutionMethod::StructuredApi],
            },
        );
        operation_rules.insert(
            "external.message.send".to_owned(),
            OperationRule {
                disposition: OperationDisposition::RequireApproval,
                allowed_execution_methods: vec![ExecutionMethod::StructuredApi],
            },
        );
        Self::from_body(PolicyBundleBody {
            policy_id: policy_id.to_owned(),
            revision: 1,
            operation_rules,
        })
    }

    fn from_body(body: PolicyBundleBody) -> Self {
        let content_digest = digest_serializable(&body);
        Self {
            body,
            content_digest,
        }
    }

    fn version(&self) -> TrustPolicyVersion {
        TrustPolicyVersion {
            policy_id: self.body.policy_id.clone(),
            revision: self.body.revision,
            content_digest: self.content_digest.clone(),
        }
    }

    fn verifies(&self) -> bool {
        self.body.revision > 0
            && is_sha256_digest(&self.content_digest)
            && digest_serializable(&self.body) == self.content_digest
    }
}

/// A Core-owned, immutable rule bundle. Construction is private to this module;
/// no caller can submit or replace rules through an Operator/Gateway command.
pub(crate) struct PolicyEngine {
    bundle: Option<PolicyBundle>,
}

impl PolicyEngine {
    pub(crate) fn core_v1() -> Self {
        let mut operation_rules = BTreeMap::new();
        // These initial classifications exercise the internal policy path only. No
        // provider dispatch is wired to this evaluator in this milestone.
        operation_rules.insert(
            "workspace.resource.read".to_owned(),
            OperationRule {
                disposition: OperationDisposition::Allow,
                allowed_execution_methods: vec![ExecutionMethod::StructuredApi],
            },
        );
        operation_rules.insert(
            "external.message.send".to_owned(),
            OperationRule {
                disposition: OperationDisposition::RequireApproval,
                allowed_execution_methods: vec![ExecutionMethod::StructuredApi],
            },
        );
        Self {
            bundle: Some(PolicyBundle::from_body(PolicyBundleBody {
                policy_id: "litecowork.standard".to_owned(),
                revision: 1,
                operation_rules,
            })),
        }
    }

    #[cfg(test)]
    fn from_core_bundle(bundle: PolicyBundle) -> Self {
        Self {
            bundle: Some(bundle),
        }
    }

    #[cfg(test)]
    fn unavailable() -> Self {
        Self { bundle: None }
    }

    pub(crate) fn evaluate(
        &self,
        request: &AuthorizationRequest,
        current: &StoredEvaluationContext,
    ) -> PolicyEvaluation {
        let request_digest = request.request_digest.clone();
        let action_digest = digest_serializable(&ActionDigestInput::from(request));
        let scope_digest = digest_serializable(&ScopeDigestInput {
            workspace_id: request.workspace_id.clone(),
            scope: request.scope.clone(),
        });
        let evaluation_context_digest = current.context_digest();

        let bundle = self.bundle.as_ref();
        let unavailable = bundle.is_none_or(|bundle| {
            !bundle.verifies() || bundle.body.policy_id != "litecowork.standard"
        });
        let result = |decision, reason_code| DecisionDetails {
            workspace_id: request.workspace_id.clone(),
            request_digest: request_digest.clone(),
            action_digest: action_digest.clone(),
            scope_digest: scope_digest.clone(),
            evaluation_context_digest: evaluation_context_digest.clone(),
            invocation_id: request.invocation_id.clone(),
            capability_grant_id: request.capability_grant_id.clone(),
            activation_id: request.activation_id.clone(),
            effect_id: request.effect_id.clone(),
            decision,
            reason_code,
        };

        if unavailable {
            return PolicyEvaluation::Unavailable(PolicyUnavailableDenial {
                details: result(DecisionKind::Deny, ReasonCode::PolicyUnavailable),
            });
        }

        let (decision, reason_code) = match request.matches(current) {
            Ok(()) => {
                // A persisted approval is bound to this exact action by `matches`.
                // Denial and revocation must remain authoritative even when a
                // read-only operation would otherwise be allowed by the policy.
                match current.approval_status {
                    Some(
                        ApprovalStatus::Denied
                        | ApprovalStatus::Expired
                        | ApprovalStatus::Revoked
                        | ApprovalStatus::Cancelled,
                    ) => {
                        return versioned(
                            self.bundle.as_ref().expect("bundle verified above"),
                            result(DecisionKind::Deny, ReasonCode::ApprovalDenied),
                        );
                    }
                    Some(ApprovalStatus::Pending) => {
                        return versioned(
                            self.bundle.as_ref().expect("bundle verified above"),
                            result(DecisionKind::RequireApproval, ReasonCode::ApprovalRequired),
                        );
                    }
                    Some(ApprovalStatus::Approved) | None => {}
                }
                let Some(rule) = self
                    .bundle
                    .as_ref()
                    .expect("bundle verified above")
                    .body
                    .operation_rules
                    .get(request.operation.0.as_str())
                else {
                    return versioned(
                        self.bundle.as_ref().expect("bundle verified above"),
                        result(DecisionKind::Deny, ReasonCode::OperationNotClassified),
                    );
                };
                if !rule
                    .allowed_execution_methods
                    .contains(&request.execution_method)
                {
                    return versioned(
                        self.bundle.as_ref().expect("bundle verified above"),
                        result(DecisionKind::Deny, ReasonCode::ExecutionMethodNotAllowed),
                    );
                }
                if rule.disposition == OperationDisposition::RequireApproval
                    && request.effect_id.is_none()
                {
                    return versioned(
                        self.bundle.as_ref().expect("bundle verified above"),
                        result(DecisionKind::Deny, ReasonCode::EffectRequired),
                    );
                }
                if current.coworker_policy_requires_approval {
                    return versioned(
                        self.bundle.as_ref().expect("bundle verified above"),
                        result(
                            DecisionKind::RequireApproval,
                            ReasonCode::ApprovalRequiredByPinnedPolicy,
                        ),
                    );
                }
                match rule.disposition {
                    OperationDisposition::Allow => {
                        (DecisionKind::Allow, ReasonCode::AllowedReadOnlyOperation)
                    }
                    OperationDisposition::RequireApproval => {
                        (DecisionKind::RequireApproval, ReasonCode::ApprovalRequired)
                    }
                }
            }
            Err(reason) => (DecisionKind::Deny, reason),
        };

        versioned(
            self.bundle.as_ref().expect("bundle verified above"),
            result(decision, reason_code),
        )
    }
}

fn versioned(bundle: &PolicyBundle, details: DecisionDetails) -> PolicyEvaluation {
    PolicyEvaluation::Versioned(VersionedEvaluation {
        policy_version: bundle.version(),
        details,
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ActionDigestInput {
    capability_ref: CapabilityRef,
    operation: OperationId,
    target_digest: String,
    execution_method: ExecutionMethod,
    effect_id: Option<EffectId>,
}

/// Approval binding is deliberately distinct from the invocation digest. It
/// binds an exact Task/Attempt, requested operation, target digest and scope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ApprovalActionDigestInput {
    task_id: String,
    attempt_id: Option<String>,
    action: String,
    target: serde_json::Value,
    scope_digest: String,
}

fn approval_action_digest(request: &AuthorizationRequest) -> Option<String> {
    let (task_id, attempt_id) = match &request.scope {
        AuthorizationScope::TaskPlanning { task_id } => (task_id.clone(), None),
        AuthorizationScope::AttemptExecution {
            task_id,
            attempt_id,
        } => (task_id.clone(), Some(attempt_id.clone())),
        AuthorizationScope::Conversation { .. } => return None,
    };
    let target = serde_json::json!({
        "capability_ref": request.capability_ref,
        "target_digest": request.target_digest,
        "execution_method": request.execution_method,
        "effect_id": request.effect_id,
    });
    let scope_digest = digest_serializable(&ScopeDigestInput {
        workspace_id: request.workspace_id.clone(),
        scope: request.scope.clone(),
    });
    Some(digest_serializable(&ApprovalActionDigestInput {
        task_id,
        attempt_id,
        action: request.operation.0.clone(),
        target,
        scope_digest,
    }))
}

/// Trusted Core metadata needed to produce a pending Approval after policy
/// evaluation. IDs and event clock values are allocated by the caller's Core
/// command boundary; all authorization-bearing fields are derived below.
pub(crate) struct ApprovalRequestEnvelope {
    pub(crate) approval_id: String,
    pub(crate) request_id: String,
    pub(crate) event_id: String,
    pub(crate) hlc_timestamp: String,
    pub(crate) correlation_id: String,
    pub(crate) causation_id: Option<String>,
    pub(crate) requested_at: String,
}

/// Persist an approval only when the canonical policy evaluator requires one.
/// This producer records an auditable request; it does not issue authority,
/// consume an Approval, create a grant/lease, or dispatch an Effect.
pub(crate) fn request_approval(
    store: &impl ApprovalStore,
    policy: &PolicyEngine,
    request: &AuthorizationRequest,
    current: &StoredEvaluationContext,
    envelope: ApprovalRequestEnvelope,
) -> Result<storage_core::CommittedApproval, StoreError> {
    let evaluation = policy.evaluate(request, current);
    if evaluation.decision() != DecisionKind::RequireApproval
        || !matches!(
            evaluation.reason_code(),
            ReasonCode::ApprovalRequired | ReasonCode::ApprovalRequiredByPinnedPolicy
        )
    {
        return Err(StoreError::Invalid(
            "Trust policy did not require a new Approval for this exact request".to_owned(),
        ));
    }
    let (task_id, attempt_id) = match &request.scope {
        AuthorizationScope::TaskPlanning { task_id } => (task_id.clone(), None),
        AuthorizationScope::AttemptExecution {
            task_id,
            attempt_id,
        } => (task_id.clone(), Some(attempt_id.clone())),
        AuthorizationScope::Conversation { .. } => {
            return Err(StoreError::Invalid(
                "Approvals require a Task or Attempt scope".to_owned(),
            ));
        }
    };
    if current.approval_status.is_some()
        || current.workspace_owner.kind != PrincipalKind::User
        || request.principal.kind != PrincipalKind::Agent
        || request.workspace_id != current.workspace_id
        || request.principal != current.principal
    {
        return Err(StoreError::Invalid(
            "Approval request actor, owner, scope, or existing decision is invalid".to_owned(),
        ));
    }
    let action_digest = approval_action_digest(request).ok_or_else(|| {
        StoreError::Invalid("Approval action cannot be bound to a Conversation".to_owned())
    })?;
    let target_ref = serde_json::json!({
        "capability_ref": request.capability_ref,
        "target_digest": request.target_digest,
        "execution_method": request.execution_method,
        "effect_id": request.effect_id,
    });
    let risk = if request.operation.0 == "workspace.resource.read" {
        "SAFE"
    } else {
        // Unknown or external operations take the stronger supported assurance.
        "SENSITIVE"
    };
    let required_assurance = if risk == "SAFE" {
        "APPROVE_SAFE"
    } else {
        "APPROVE_SENSITIVE"
    };
    let recorded_at = canonical_trust_timestamp(&envelope.requested_at)?;
    let expires_at = OffsetDateTime::parse(&recorded_at, &Rfc3339)
        .map_err(|_| StoreError::Invalid("Approval request time is invalid".to_owned()))?
        .checked_add(Duration::minutes(APPROVAL_TTL_MINUTES))
        .ok_or_else(|| StoreError::Invalid("Approval expiry is out of range".to_owned()))?
        .format(&Rfc3339)
        .map_err(|_| StoreError::Invalid("Approval expiry is invalid".to_owned()))?;
    let actor_kind = match request.principal.kind {
        PrincipalKind::Agent => "AGENT",
        _ => unreachable!("request actor kind was validated above"),
    };
    let approval = ApprovalRecord {
        approval_id: envelope.approval_id.clone(),
        task_id: task_id.clone(),
        requested_by_attempt: attempt_id,
        kind: request.operation.0.clone(),
        action_summary: format!("Approve operation {}", request.operation.0),
        target_ref,
        scope_digest: evaluation.scope_digest().to_owned(),
        action_digest,
        risk: risk.to_owned(),
        required_assurance: required_assurance.to_owned(),
        status: "PENDING".to_owned(),
        requested_at: recorded_at.clone(),
        expires_at: Some(expires_at),
        resolved_by: None,
        resolved_at: None,
        version: 1,
    };
    let event = EventDraft {
        event_id: envelope.event_id,
        workspace_id: request.workspace_id.0.clone(),
        entity_type: "approval".to_owned(),
        entity_id: approval.approval_id.clone(),
        origin_runtime_id: request.runtime_id.0.clone(),
        entity_revision: 1,
        hlc_timestamp: envelope.hlc_timestamp,
        correlation_id: envelope.correlation_id,
        causation_id: envelope.causation_id,
        schema_version: 1,
        event_type: "approval.requested.v1".to_owned(),
        payload: serde_json::json!({
            "approval_id": approval.approval_id,
            "task_id": task_id,
            "action_digest": approval.action_digest,
            "required_assurance": required_assurance,
            "from": request.principal.principal_id.0,
            "to": current.workspace_owner.principal_id.0,
        }),
        recorded_at,
    };
    store.request_approval(RequestApprovalCommit {
        workspace_id: request.workspace_id.0.clone(),
        actor: ApprovalActor {
            principal_id: request.principal.principal_id.0.clone(),
            principal_kind: actor_kind.to_owned(),
            owner_principal_id: current.workspace_owner.principal_id.0.clone(),
            assurance_level: "VIEW_ONLY".to_owned(),
        },
        request_id: envelope.request_id,
        approval,
        event,
    })
}

fn canonical_trust_timestamp(value: &str) -> Result<String, StoreError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| StoreError::Invalid("Approval request time is invalid".to_owned()))?;
    parsed
        .format(&Rfc3339)
        .map_err(|_| StoreError::Invalid("Approval request time is invalid".to_owned()))
}

impl From<&AuthorizationRequest> for ActionDigestInput {
    fn from(request: &AuthorizationRequest) -> Self {
        Self {
            capability_ref: request.capability_ref.clone(),
            operation: request.operation.clone(),
            target_digest: request.target_digest.clone(),
            execution_method: request.execution_method,
            effect_id: request.effect_id.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ScopeDigestInput {
    workspace_id: WorkspaceId,
    scope: AuthorizationScope,
}

/// Storage supplies this typed snapshot from the same transaction snapshot that
/// will later perform dispatch admission. It deliberately contains only IDs,
/// versions, statuses and digests; secret bytes have no representation here.
#[derive(Clone, Debug)]
pub(crate) struct StoredEvaluationContext {
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) workspace_owner: PrincipalRef,
    pub(crate) workspace_active: bool,
    pub(crate) workspace_policy_digest: String,
    pub(crate) workspace_policy_allows_operation: bool,
    pub(crate) principal: PrincipalRef,
    pub(crate) agent_session_id: AgentSessionId,
    pub(crate) runtime_id: RuntimeId,
    pub(crate) runtime_incarnation_id: RuntimeIncarnationId,
    pub(crate) scope: AuthorizationScope,
    pub(crate) invocation_id: InvocationId,
    pub(crate) capability_grant_id: CapabilityGrantId,
    pub(crate) grant_scope: AuthorizationScope,
    pub(crate) grant_capability_ref: CapabilityRef,
    pub(crate) grant_allowed_operations: Vec<OperationId>,
    pub(crate) resource_scope_digest: String,
    pub(crate) target_within_grant_resource_scope: bool,
    pub(crate) activation_id: ActivationId,
    pub(crate) activation_capability_ref: CapabilityRef,
    pub(crate) activation_manifest_digest: String,
    pub(crate) capability_ref: CapabilityRef,
    pub(crate) capability_manifest_digest: String,
    pub(crate) operation: OperationId,
    pub(crate) input_digest: String,
    pub(crate) request_digest: String,
    pub(crate) target_digest: String,
    pub(crate) execution_method: ExecutionMethod,
    pub(crate) effect_id: Option<EffectId>,
    pub(crate) workspace_policy_revision: u64,
    pub(crate) coworker_revision: Option<u64>,
    pub(crate) task_spec_revision: Option<u64>,
    pub(crate) lease_epoch: Option<u64>,
    pub(crate) session_active: bool,
    pub(crate) scope_active: bool,
    pub(crate) grant_active: bool,
    pub(crate) activation_active: bool,
    pub(crate) effect_proposed: bool,
    pub(crate) secret_lease_state: SecretLeaseState,
    pub(crate) secret_lease_digest: Option<String>,
    pub(crate) coworker_policy_requires_approval: bool,
    pub(crate) scope_version: u64,
    pub(crate) coworker_revision_digest: Option<String>,
    pub(crate) task_spec_revision_digest: Option<String>,
    pub(crate) approval_status: Option<ApprovalStatus>,
    pub(crate) approval_action_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum SecretLeaseState {
    NotRequired,
    Valid,
    Invalid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
    Revoked,
    Cancelled,
}

impl StoredEvaluationContext {
    fn authorization_request(&self) -> AuthorizationRequest {
        AuthorizationRequest {
            workspace_id: self.workspace_id.clone(),
            principal: self.principal.clone(),
            agent_session_id: self.agent_session_id.clone(),
            runtime_id: self.runtime_id.clone(),
            runtime_incarnation_id: self.runtime_incarnation_id.clone(),
            scope: self.scope.clone(),
            invocation_id: self.invocation_id.clone(),
            capability_grant_id: self.capability_grant_id.clone(),
            activation_id: self.activation_id.clone(),
            capability_ref: self.capability_ref.clone(),
            operation: self.operation.clone(),
            input_digest: self.input_digest.clone(),
            request_digest: self.request_digest.clone(),
            target_digest: self.target_digest.clone(),
            execution_method: self.execution_method,
            effect_id: self.effect_id.clone(),
        }
    }

    fn digest_for(&self, request: &AuthorizationRequest) -> String {
        digest_serializable(&RequestDigestInput {
            invocation_id: request.invocation_id.clone(),
            scope: request.scope.clone(),
            capability_ref: request.capability_ref.clone(),
            operation: request.operation.clone(),
            input_digest: request.input_digest.clone(),
            target_digest: request.target_digest.clone(),
            execution_method: request.execution_method,
            effect_id: request.effect_id.clone(),
        })
    }

    fn scope_digest(&self) -> String {
        digest_serializable(&ScopeDigestInput {
            workspace_id: self.workspace_id.clone(),
            scope: self.scope.clone(),
        })
    }

    fn context_digest(&self) -> String {
        digest_serializable(&EvaluationContextDigestInput {
            workspace_id: self.workspace_id.clone(),
            workspace_owner: self.workspace_owner.clone(),
            workspace_active: self.workspace_active,
            workspace_policy_digest: self.workspace_policy_digest.clone(),
            workspace_policy_allows_operation: self.workspace_policy_allows_operation,
            principal: self.principal.clone(),
            agent_session_id: self.agent_session_id.clone(),
            runtime_id: self.runtime_id.clone(),
            runtime_incarnation_id: self.runtime_incarnation_id.clone(),
            scope: self.scope.clone(),
            invocation_id: self.invocation_id.clone(),
            capability_grant_id: self.capability_grant_id.clone(),
            grant_scope: self.grant_scope.clone(),
            grant_capability_ref: self.grant_capability_ref.clone(),
            grant_allowed_operations: self.grant_allowed_operations.clone(),
            resource_scope_digest: self.resource_scope_digest.clone(),
            target_within_grant_resource_scope: self.target_within_grant_resource_scope,
            activation_id: self.activation_id.clone(),
            activation_capability_ref: self.activation_capability_ref.clone(),
            activation_manifest_digest: self.activation_manifest_digest.clone(),
            capability_ref: self.capability_ref.clone(),
            capability_manifest_digest: self.capability_manifest_digest.clone(),
            operation: self.operation.clone(),
            input_digest: self.input_digest.clone(),
            request_digest: self.request_digest.clone(),
            target_digest: self.target_digest.clone(),
            execution_method: self.execution_method,
            effect_id: self.effect_id.clone(),
            workspace_policy_revision: self.workspace_policy_revision,
            coworker_revision: self.coworker_revision,
            task_spec_revision: self.task_spec_revision,
            lease_epoch: self.lease_epoch,
            session_active: self.session_active,
            scope_active: self.scope_active,
            grant_active: self.grant_active,
            activation_active: self.activation_active,
            effect_proposed: self.effect_proposed,
            secret_lease_state: self.secret_lease_state,
            secret_lease_digest: self.secret_lease_digest.clone(),
            coworker_policy_requires_approval: self.coworker_policy_requires_approval,
            scope_version: self.scope_version,
            coworker_revision_digest: self.coworker_revision_digest.clone(),
            task_spec_revision_digest: self.task_spec_revision_digest.clone(),
            approval_status: self.approval_status,
            approval_action_digest: self.approval_action_digest.clone(),
        })
    }

    #[cfg(test)]
    fn test_valid() -> Self {
        let mut context = Self {
            workspace_id: WorkspaceId::from("workspace-1"),
            workspace_owner: PrincipalRef {
                principal_id: PrincipalId::from("principal-1"),
                kind: PrincipalKind::User,
            },
            workspace_active: true,
            workspace_policy_digest: digest_bytes(b"workspace policy"),
            workspace_policy_allows_operation: true,
            principal: PrincipalRef {
                principal_id: PrincipalId::from("principal-1"),
                kind: PrincipalKind::User,
            },
            agent_session_id: AgentSessionId::from("session-1"),
            runtime_id: RuntimeId::from("runtime-1"),
            runtime_incarnation_id: RuntimeIncarnationId::from("incarnation-1"),
            scope: AuthorizationScope::conversation("conversation-1", "turn-1"),
            invocation_id: InvocationId::from("invocation-1"),
            capability_grant_id: CapabilityGrantId::from("grant-1"),
            grant_scope: AuthorizationScope::conversation("conversation-1", "turn-1"),
            grant_capability_ref: CapabilityRef::from("capability.resource"),
            grant_allowed_operations: vec![OperationId::from("workspace.resource.read")],
            resource_scope_digest: digest_bytes(b"resource scope"),
            target_within_grant_resource_scope: true,
            activation_id: ActivationId::from("activation-1"),
            activation_capability_ref: CapabilityRef::from("capability.resource"),
            activation_manifest_digest: digest_bytes(b"activation manifest"),
            capability_ref: CapabilityRef::from("capability.resource"),
            capability_manifest_digest: digest_bytes(b"capability manifest"),
            operation: OperationId::from("workspace.resource.read"),
            input_digest: digest_bytes(b"normalized input"),
            request_digest: String::new(),
            target_digest: digest_bytes(b"target-plaintext"),
            execution_method: ExecutionMethod::StructuredApi,
            effect_id: None,
            workspace_policy_revision: 1,
            coworker_revision: Some(1),
            task_spec_revision: None,
            lease_epoch: None,
            session_active: true,
            scope_active: true,
            grant_active: true,
            activation_active: true,
            effect_proposed: false,
            secret_lease_state: SecretLeaseState::NotRequired,
            secret_lease_digest: None,
            coworker_policy_requires_approval: false,
            scope_version: 1,
            coworker_revision_digest: Some(digest_bytes(b"coworker revision")),
            task_spec_revision_digest: None,
            approval_status: None,
            approval_action_digest: None,
        };
        context.request_digest = context.digest_for(&context.authorization_request());
        context
    }
}

impl AuthorizationRequest {
    fn matches(&self, current: &StoredEvaluationContext) -> Result<(), ReasonCode> {
        if self.workspace_id != current.workspace_id {
            return Err(ReasonCode::WorkspaceMismatch);
        }
        if !current.workspace_active || !current.workspace_policy_allows_operation {
            return Err(ReasonCode::WorkspacePolicyDenied);
        }
        if !is_sha256_digest(&current.workspace_policy_digest) {
            return Err(ReasonCode::PolicyUnavailable);
        }
        if self.principal != current.principal {
            return Err(ReasonCode::PrincipalMismatch);
        }
        if self.agent_session_id != current.agent_session_id {
            return Err(ReasonCode::SessionMismatch);
        }
        if self.runtime_id != current.runtime_id
            || self.runtime_incarnation_id != current.runtime_incarnation_id
        {
            return Err(ReasonCode::RuntimeMismatch);
        }
        if self.scope != current.scope {
            return Err(ReasonCode::ScopeMismatch);
        }
        let scope_bound = match &current.scope {
            AuthorizationScope::Conversation { .. } => current.scope_version > 0,
            AuthorizationScope::TaskPlanning { .. } => current
                .task_spec_revision
                .is_some_and(|revision| revision > 0),
            AuthorizationScope::AttemptExecution { .. } => {
                current
                    .task_spec_revision
                    .is_some_and(|revision| revision > 0)
                    && current.lease_epoch.is_some_and(|epoch| epoch > 0)
            }
        };
        if !scope_bound {
            return Err(ReasonCode::ScopeNotActive);
        }
        if self.invocation_id != current.invocation_id {
            return Err(ReasonCode::InvocationMismatch);
        }
        if self.capability_grant_id != current.capability_grant_id {
            return Err(ReasonCode::GrantMismatch);
        }
        if self.activation_id != current.activation_id {
            return Err(ReasonCode::ActivationMismatch);
        }
        if self.capability_ref != current.capability_ref {
            return Err(ReasonCode::CapabilityMismatch);
        }
        if !self.capability_ref.validate() {
            return Err(ReasonCode::CapabilityMismatch);
        }
        if current.grant_scope != current.scope
            || current.grant_capability_ref != self.capability_ref
            || !current.grant_allowed_operations.contains(&self.operation)
            || !current.target_within_grant_resource_scope
            || !is_sha256_digest(&current.resource_scope_digest)
        {
            return Err(ReasonCode::GrantMismatch);
        }
        if current.activation_capability_ref != self.capability_ref
            || !is_sha256_digest(&current.activation_manifest_digest)
            || !is_sha256_digest(&current.capability_manifest_digest)
        {
            return Err(ReasonCode::ActivationMismatch);
        }
        if self.operation != current.operation {
            return Err(ReasonCode::OperationMismatch);
        }
        if self.input_digest != current.input_digest {
            return Err(ReasonCode::RequestDigestMismatch);
        }
        if self.target_digest != current.target_digest {
            return Err(ReasonCode::TargetDigestMismatch);
        }
        if self.effect_id != current.effect_id {
            return Err(ReasonCode::EffectMismatch);
        }
        if self.effect_id.is_some()
            && !matches!(&self.scope, AuthorizationScope::AttemptExecution { .. })
        {
            return Err(ReasonCode::EffectRequiresAttemptScope);
        }
        if !is_sha256_digest(&self.request_digest)
            || self.request_digest != current.request_digest
            || self.request_digest != current.digest_for(self)
        {
            return Err(ReasonCode::RequestDigestMismatch);
        }
        if !is_sha256_digest(&self.input_digest) || !is_sha256_digest(&self.target_digest) {
            return Err(ReasonCode::RequestDigestMismatch);
        }
        if !current.session_active {
            return Err(ReasonCode::SessionNotActive);
        }
        if !current.scope_active {
            return Err(ReasonCode::ScopeNotActive);
        }
        if !current.grant_active {
            return Err(ReasonCode::GrantNotActive);
        }
        if !current.activation_active {
            return Err(ReasonCode::ActivationNotActive);
        }
        if current.coworker_revision.is_some()
            && current
                .coworker_revision_digest
                .as_deref()
                .is_none_or(|digest| !is_sha256_digest(digest))
        {
            return Err(ReasonCode::ScopeNotActive);
        }
        if current.task_spec_revision.is_some()
            && current
                .task_spec_revision_digest
                .as_deref()
                .is_none_or(|digest| !is_sha256_digest(digest))
        {
            return Err(ReasonCode::ScopeNotActive);
        }
        if current.approval_status.is_some()
            && current
                .approval_action_digest
                .as_deref()
                .is_none_or(|digest| {
                    !is_sha256_digest(digest)
                        || approval_action_digest(self).as_deref() != Some(digest)
                })
        {
            return Err(ReasonCode::ApprovalBindingMismatch);
        }
        if self.effect_id.is_some() && !current.effect_proposed {
            return Err(ReasonCode::EffectRequired);
        }
        if self.effect_id.is_none() && current.effect_proposed {
            return Err(ReasonCode::EffectUnexpected);
        }
        if self.execution_method == ExecutionMethod::Unknown {
            return Err(ReasonCode::ExecutionMethodUnknown);
        }
        if current.secret_lease_state == SecretLeaseState::Invalid {
            return Err(ReasonCode::SecretLeaseInvalid);
        }
        if current.secret_lease_state == SecretLeaseState::Valid
            && current
                .secret_lease_digest
                .as_deref()
                .is_none_or(|digest| !is_sha256_digest(digest))
        {
            return Err(ReasonCode::SecretLeaseInvalid);
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct RequestDigestInput {
    invocation_id: InvocationId,
    scope: AuthorizationScope,
    capability_ref: CapabilityRef,
    operation: OperationId,
    input_digest: String,
    target_digest: String,
    execution_method: ExecutionMethod,
    effect_id: Option<EffectId>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct EvaluationContextDigestInput {
    workspace_id: WorkspaceId,
    workspace_owner: PrincipalRef,
    workspace_active: bool,
    workspace_policy_digest: String,
    workspace_policy_allows_operation: bool,
    principal: PrincipalRef,
    agent_session_id: AgentSessionId,
    runtime_id: RuntimeId,
    runtime_incarnation_id: RuntimeIncarnationId,
    scope: AuthorizationScope,
    invocation_id: InvocationId,
    capability_grant_id: CapabilityGrantId,
    grant_scope: AuthorizationScope,
    grant_capability_ref: CapabilityRef,
    grant_allowed_operations: Vec<OperationId>,
    resource_scope_digest: String,
    target_within_grant_resource_scope: bool,
    activation_id: ActivationId,
    activation_capability_ref: CapabilityRef,
    activation_manifest_digest: String,
    capability_ref: CapabilityRef,
    capability_manifest_digest: String,
    operation: OperationId,
    input_digest: String,
    request_digest: String,
    target_digest: String,
    execution_method: ExecutionMethod,
    effect_id: Option<EffectId>,
    workspace_policy_revision: u64,
    coworker_revision: Option<u64>,
    task_spec_revision: Option<u64>,
    lease_epoch: Option<u64>,
    session_active: bool,
    scope_active: bool,
    grant_active: bool,
    activation_active: bool,
    effect_proposed: bool,
    secret_lease_state: SecretLeaseState,
    secret_lease_digest: Option<String>,
    coworker_policy_requires_approval: bool,
    scope_version: u64,
    coworker_revision_digest: Option<String>,
    task_spec_revision_digest: Option<String>,
    approval_status: Option<ApprovalStatus>,
    approval_action_digest: Option<String>,
}

fn digest_serializable(value: &impl Serialize) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .expect("closed Trust digest inputs must always be canonicalizable");
    digest_bytes(&bytes)
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct CaptureApprovalStore(Mutex<Option<RequestApprovalCommit>>);

    impl ApprovalStore for CaptureApprovalStore {
        fn request_approval(
            &self,
            command: RequestApprovalCommit,
        ) -> Result<storage_core::CommittedApproval, StoreError> {
            *self.0.lock().expect("capture mutex") = Some(command);
            Err(StoreError::Busy)
        }

        fn get_approval(
            &self,
            _owner_principal_id: &str,
            _workspace_id: &str,
            _approval_id: &str,
        ) -> Result<Option<ApprovalRecord>, StoreError> {
            Ok(None)
        }

        fn list_approvals_page(
            &self,
            _owner_principal_id: &str,
            _request: storage_core::ApprovalPageRequest,
        ) -> Result<storage_core::ApprovalPage, StoreError> {
            Ok(storage_core::ApprovalPage {
                items: Vec::new(),
                next_cursor: None,
            })
        }

        fn resolve_approval(
            &self,
            _command: storage_core::ResolveApprovalCommit,
        ) -> Result<storage_core::CommittedApproval, StoreError> {
            Err(StoreError::Busy)
        }
    }

    fn fixture() -> (PolicyEngine, AuthorizationRequest, StoredEvaluationContext) {
        let engine = PolicyEngine::core_v1();
        let context = StoredEvaluationContext::test_valid();
        let request = context.authorization_request();
        (engine, request, context)
    }

    fn task_attempt_fixture() -> (PolicyEngine, AuthorizationRequest, StoredEvaluationContext) {
        let engine = PolicyEngine::core_v1();
        let mut context = StoredEvaluationContext::test_valid();
        context.scope = AuthorizationScope::AttemptExecution {
            task_id: "task-1".into(),
            attempt_id: "attempt-1".into(),
        };
        context.grant_scope = context.scope.clone();
        context.task_spec_revision = Some(1);
        context.task_spec_revision_digest = Some(digest_bytes(b"task spec revision"));
        context.lease_epoch = Some(1);
        context.effect_id = Some(EffectId::from("effect-1"));
        context.effect_proposed = true;
        context.operation = OperationId::from("external.message.send");
        context
            .grant_allowed_operations
            .push(context.operation.clone());
        let mut request = context.authorization_request();
        request.request_digest = context.digest_for(&request);
        context.request_digest = request.request_digest.clone();
        (engine, request, context)
    }

    fn approval_envelope() -> ApprovalRequestEnvelope {
        ApprovalRequestEnvelope {
            approval_id: "approval-1".to_owned(),
            request_id: "approval-request-1".to_owned(),
            event_id: "approval-event-1".to_owned(),
            hlc_timestamp: "2026-10-17T10:00:00Z".to_owned(),
            correlation_id: "invocation-1".to_owned(),
            causation_id: Some("invocation-1".to_owned()),
            requested_at: "2026-10-17T10:00:00Z".to_owned(),
        }
    }

    #[test]
    fn request_approval_persists_exact_task_attempt_action_and_expiry() {
        let (policy, request, mut current) = task_attempt_fixture();
        current.principal.kind = PrincipalKind::Agent;
        current.principal.principal_id = PrincipalId::from("agent-1");
        let mut request = request;
        request.principal = current.principal.clone();
        request.request_digest = current.digest_for(&request);
        current.request_digest = request.request_digest.clone();
        let store = CaptureApprovalStore::default();

        assert!(matches!(
            request_approval(&store, &policy, &request, &current, approval_envelope()),
            Err(StoreError::Busy)
        ));
        let commit = store.0.lock().expect("capture mutex").take().unwrap();
        assert_eq!(commit.approval.task_id, "task-1");
        assert_eq!(
            commit.approval.requested_by_attempt.as_deref(),
            Some("attempt-1")
        );
        assert_eq!(commit.approval.kind, "external.message.send");
        assert_eq!(commit.approval.risk, "SENSITIVE");
        assert_eq!(commit.approval.required_assurance, "APPROVE_SENSITIVE");
        assert_eq!(commit.approval.status, "PENDING");
        assert_eq!(commit.approval.version, 1);
        assert_eq!(
            commit.approval.action_digest,
            approval_action_digest(&request).unwrap()
        );
        assert_eq!(
            commit.approval.scope_digest,
            policy.evaluate(&request, &current).scope_digest()
        );
        assert_eq!(
            commit.approval.target_ref,
            serde_json::json!({
                "capability_ref": request.capability_ref,
                "target_digest": request.target_digest,
                "execution_method": request.execution_method,
                "effect_id": request.effect_id,
            })
        );
        assert_eq!(
            commit.approval.target_ref["capability_ref"],
            serde_json::to_value(&request.capability_ref).unwrap()
        );
        assert_eq!(
            commit.approval.target_ref["capability_ref"]["identity_kind"],
            "PACKAGE_COMPONENT"
        );
        assert!(commit.approval.target_ref["capability_ref"]["package_version"].is_string());
        assert_eq!(commit.actor.principal_id, "agent-1");
        assert_eq!(commit.actor.owner_principal_id, "principal-1");
        assert_eq!(commit.event.event_type, "approval.requested.v1");
        assert_eq!(commit.event.payload["from"], "agent-1");
        assert_eq!(commit.event.payload["to"], "principal-1");
        assert_eq!(commit.approval.requested_at, "2026-10-17T10:00:00Z");
        assert_eq!(
            commit.approval.expires_at.as_deref(),
            Some("2026-10-17T10:15:00Z")
        );
    }

    #[test]
    fn capability_ref_requires_complete_canonical_identity_and_lowercase_digest() {
        let mut ref_value = CapabilityRef::from("capability-1");
        assert!(ref_value.validate());
        ref_value.package_version = None;
        assert!(!ref_value.validate());

        let mut skill = CapabilityRef::from("skill-1");
        skill.identity_kind = CapabilityIdentityKind::McpSkill;
        skill.package_version = None;
        skill.component = Some("https://mcp.example.test/skills/exact/SKILL.md".into());
        assert!(skill.validate());
        skill.package_version = Some("1.0.0".into());
        assert!(!skill.validate());

        let mut malformed_digest = CapabilityRef::from("capability-1");
        malformed_digest.digest = format!("sha256:{}", "A".repeat(64));
        assert!(!malformed_digest.validate());
    }

    #[test]
    fn request_approval_refuses_policy_denial_without_storage_write() {
        let (policy, request, mut current) = task_attempt_fixture();
        current.scope_active = false;
        let store = CaptureApprovalStore::default();

        assert!(matches!(
            request_approval(&store, &policy, &request, &current, approval_envelope()),
            Err(StoreError::Invalid(_))
        ));
        assert!(store.0.lock().expect("capture mutex").is_none());
    }

    fn deny_for_context_mismatch(
        mutate: impl FnOnce(&mut StoredEvaluationContext),
        expected: ReasonCode,
    ) {
        let (engine, request, mut context) = fixture();
        mutate(&mut context);
        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), expected);
    }

    #[test]
    fn mismatched_authenticated_principal_is_denied() {
        deny_for_context_mismatch(
            |context| context.principal.principal_id = PrincipalId::from("principal-other"),
            ReasonCode::PrincipalMismatch,
        );
    }

    #[test]
    fn mismatched_agent_session_is_denied() {
        deny_for_context_mismatch(
            |context| context.agent_session_id = AgentSessionId::from("session-other"),
            ReasonCode::SessionMismatch,
        );
    }

    #[test]
    fn mismatched_scope_is_denied() {
        deny_for_context_mismatch(
            |context| {
                context.scope = AuthorizationScope::conversation("conversation-other", "turn-1")
            },
            ReasonCode::ScopeMismatch,
        );
    }

    #[test]
    fn mismatched_grant_is_denied() {
        deny_for_context_mismatch(
            |context| context.capability_grant_id = CapabilityGrantId::from("grant-other"),
            ReasonCode::GrantMismatch,
        );
    }

    #[test]
    fn mismatched_activation_is_denied() {
        deny_for_context_mismatch(
            |context| context.activation_id = ActivationId::from("activation-other"),
            ReasonCode::ActivationMismatch,
        );
    }

    #[test]
    fn mismatched_operation_is_denied() {
        deny_for_context_mismatch(
            |context| context.operation = OperationId::from("workspace.resource.delete"),
            ReasonCode::OperationMismatch,
        );
    }

    #[test]
    fn unknown_core_policy_bundle_fails_closed_without_versioned_decision() {
        let context = StoredEvaluationContext::test_valid();
        let request = context.authorization_request();
        let engine = PolicyEngine::from_core_bundle(PolicyBundle::test_bundle(
            "unrecognized-core-policy",
            OperationDisposition::Allow,
        ));

        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::PolicyUnavailable);
        assert!(matches!(result, PolicyEvaluation::Unavailable(_)));
    }

    #[test]
    fn missing_policy_fails_closed_without_versioned_decision() {
        let context = StoredEvaluationContext::test_valid();
        let request = context.authorization_request();
        let result = PolicyEngine::unavailable().evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::PolicyUnavailable);
        assert!(matches!(result, PolicyEvaluation::Unavailable(_)));
    }

    #[test]
    fn corrupt_policy_digest_fails_closed_without_versioned_decision() {
        let context = StoredEvaluationContext::test_valid();
        let request = context.authorization_request();
        let mut bundle =
            PolicyBundle::test_bundle("litecowork.standard", OperationDisposition::Allow);
        bundle.content_digest = digest_bytes(b"different rules");
        let result = PolicyEngine::from_core_bundle(bundle).evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::PolicyUnavailable);
        assert!(matches!(result, PolicyEvaluation::Unavailable(_)));
    }

    #[test]
    fn unknown_operation_under_a_valid_bundle_is_exact_action_deny() {
        let (engine, mut request, mut context) = fixture();
        request.operation = OperationId::from("unknown.provider.operation");
        context.operation = request.operation.clone();
        context
            .grant_allowed_operations
            .push(request.operation.clone());
        request.request_digest = context.digest_for(&request);
        context.request_digest = request.request_digest.clone();

        let result = engine.evaluate(&request, &context);
        assert!(matches!(result, PolicyEvaluation::Versioned(_)));
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::OperationNotClassified);
        assert!(is_sha256_digest(result.action_digest()));
    }

    #[test]
    fn allow_binds_exact_action_scope_request_and_storage_context_digests() {
        let (engine, request, context) = fixture();
        let result = engine.evaluate(&request, &context);

        assert_eq!(result.decision(), DecisionKind::Allow);
        assert_eq!(result.reason_code(), ReasonCode::AllowedReadOnlyOperation);
        assert_eq!(result.request_digest(), request.request_digest);
        assert_eq!(result.scope_digest(), context.scope_digest());
        assert_eq!(result.evaluation_context_digest(), context.context_digest());
        assert_eq!(
            result.action_digest(),
            digest_serializable(&ActionDigestInput::from(&request))
        );
        assert_eq!(result.invocation_id(), &request.invocation_id);
        assert_eq!(result.capability_grant_id(), &request.capability_grant_id);
        assert_eq!(result.activation_id(), &request.activation_id);
        let PolicyEvaluation::Versioned(versioned) = &result else {
            panic!("available Core policy must produce a versioned evaluation");
        };
        assert_eq!(versioned.policy_version.policy_id, "litecowork.standard");
        assert_eq!(versioned.policy_version.revision, 1);
        assert_eq!(
            versioned.policy_version.content_digest,
            digest_serializable(&engine.bundle.as_ref().unwrap().body)
        );
    }

    #[test]
    fn grant_context_is_part_of_the_explicit_evaluation_context_digest() {
        let (engine, request, context) = fixture();
        let initial_digest = context.context_digest();
        let initial = engine.evaluate(&request, &context);

        let mut revoked_context = context.clone();
        revoked_context.grant_active = false;
        let changed_digest = revoked_context.context_digest();
        let revoked = engine.evaluate(&request, &revoked_context);

        assert_ne!(initial_digest, changed_digest);
        assert_eq!(initial.evaluation_context_digest(), initial_digest);
        assert_eq!(revoked.evaluation_context_digest(), changed_digest);
        assert_eq!(revoked.decision(), DecisionKind::Deny);
        assert_eq!(revoked.reason_code(), ReasonCode::GrantNotActive);
    }

    #[test]
    fn consequential_operation_requires_approval_and_never_becomes_allow() {
        // A proposed Effect is only admissible inside an admitted Attempt scope:
        // Conversation- and planning-scoped grants are read-only, so a request that
        // attaches an Effect outside `ATTEMPT_EXECUTION` is denied earlier with
        // `EffectRequiresAttemptScope` (see
        // `conversation_scope_cannot_attach_an_effect`). Evaluating the policy
        // disposition therefore has to happen in the scope where the Effect is
        // legal, otherwise the scope gate answers before the disposition does.
        let (engine, request, context) = task_attempt_fixture();

        let result = engine.evaluate(&request, &context);
        assert_ne!(
            result.decision(),
            DecisionKind::Allow,
            "a consequential operation must never become an allow"
        );
        assert_eq!(result.decision(), DecisionKind::RequireApproval);
        assert_eq!(result.reason_code(), ReasonCode::ApprovalRequired);
        assert_eq!(result.effect_id(), request.effect_id.as_ref());
    }

    #[test]
    fn consequential_operation_without_a_proposed_effect_is_denied() {
        let (engine, mut request, mut context) = fixture();
        request.operation = OperationId::from("external.message.send");
        context.operation = request.operation.clone();
        context
            .grant_allowed_operations
            .push(request.operation.clone());
        request.request_digest = context.digest_for(&request);
        context.request_digest = request.request_digest.clone();

        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::EffectRequired);
    }

    #[test]
    fn task_planning_scope_denies_consequential_operations_before_approval() {
        let (engine, mut request, mut context) = task_attempt_fixture();
        request.scope = AuthorizationScope::TaskPlanning {
            task_id: "task-1".into(),
        };
        request.operation = OperationId::from("external.message.send");
        request.effect_id = Some(EffectId::from("effect-planning-send"));
        context.scope = request.scope.clone();
        context.grant_scope = request.scope.clone();
        context.operation = request.operation.clone();
        context
            .grant_allowed_operations
            .push(request.operation.clone());
        context.effect_id = request.effect_id.clone();
        context.effect_proposed = true;
        context.request_digest = context.digest_for(&request);
        request.request_digest = context.request_digest.clone();

        let decision = engine.evaluate(&request, &context);
        assert_eq!(decision.decision(), DecisionKind::Deny);
        assert_eq!(
            decision.reason_code(),
            ReasonCode::EffectRequiresAttemptScope
        );
    }

    #[test]
    fn conversation_scope_cannot_attach_an_effect() {
        let (engine, mut request, mut context) = fixture();
        request.operation = OperationId::from("external.message.send");
        request.effect_id = Some(EffectId::from("effect-conversation-send"));
        context.operation = request.operation.clone();
        context
            .grant_allowed_operations
            .push(request.operation.clone());
        context.effect_id = request.effect_id.clone();
        context.effect_proposed = true;
        request.request_digest = context.digest_for(&request);
        context.request_digest = request.request_digest.clone();

        let decision = engine.evaluate(&request, &context);
        assert_eq!(decision.decision(), DecisionKind::Deny);
        assert_eq!(
            decision.reason_code(),
            ReasonCode::EffectRequiresAttemptScope
        );
    }

    #[test]
    fn denied_approval_for_the_exact_action_is_not_treated_as_a_read_allow() {
        let (engine, request, mut context) = task_attempt_fixture();
        context.approval_status = Some(ApprovalStatus::Denied);
        context.approval_action_digest = approval_action_digest(&request);

        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::ApprovalDenied);
    }

    #[test]
    fn expired_or_revoked_approval_for_the_exact_action_is_denied() {
        let (engine, request, context) = task_attempt_fixture();
        let approval_action_digest = approval_action_digest(&request);

        for status in [ApprovalStatus::Expired, ApprovalStatus::Revoked] {
            let mut context = context.clone();
            context.approval_status = Some(status);
            context.approval_action_digest = approval_action_digest.clone();

            let result = engine.evaluate(&request, &context);
            assert_eq!(result.decision(), DecisionKind::Deny);
            assert_eq!(result.reason_code(), ReasonCode::ApprovalDenied);
        }
    }

    #[test]
    fn pending_approval_for_the_exact_action_does_not_become_a_read_allow() {
        let (engine, request, mut context) = task_attempt_fixture();
        context.approval_status = Some(ApprovalStatus::Pending);
        context.approval_action_digest = approval_action_digest(&request);

        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::RequireApproval);
        assert_eq!(result.reason_code(), ReasonCode::ApprovalRequired);
    }

    #[test]
    fn cancelled_approval_for_the_exact_action_is_denied() {
        let (engine, request, mut context) = task_attempt_fixture();
        context.approval_status = Some(
            serde_json::from_value::<ApprovalStatus>(serde_json::json!("CANCELLED"))
                .expect("stored CANCELLED approval state must be representable"),
        );
        context.approval_action_digest = approval_action_digest(&request);

        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::ApprovalDenied);
    }

    #[test]
    fn task_attempt_approval_binds_exact_action_target_and_scope() {
        let (engine, request, mut context) = task_attempt_fixture();
        context.approval_status = Some(ApprovalStatus::Approved);
        let expected =
            approval_action_digest(&request).expect("Task Attempt has an approval binding");
        context.approval_action_digest = Some(expected.clone());
        let accepted = engine.evaluate(&request, &context);
        assert_eq!(accepted.decision(), DecisionKind::RequireApproval);
        assert_ne!(accepted.reason_code(), ReasonCode::ApprovalBindingMismatch);

        let mut changed_action = request.clone();
        changed_action.operation = OperationId::from("external.message.delete");
        context.operation = changed_action.operation.clone();
        context
            .grant_allowed_operations
            .push(changed_action.operation.clone());
        changed_action.request_digest = context.digest_for(&changed_action);
        context.request_digest = changed_action.request_digest.clone();
        let rejected = engine.evaluate(&changed_action, &context);
        assert_eq!(rejected.reason_code(), ReasonCode::ApprovalBindingMismatch);

        let (engine, request, mut context) = task_attempt_fixture();
        context.approval_status = Some(ApprovalStatus::Approved);
        context.approval_action_digest = approval_action_digest(&request);
        let mut changed_target = request.clone();
        changed_target.target_digest = digest_bytes(b"different target revision");
        context.target_digest = changed_target.target_digest.clone();
        changed_target.request_digest = context.digest_for(&changed_target);
        context.request_digest = changed_target.request_digest.clone();
        assert_eq!(
            engine.evaluate(&changed_target, &context).reason_code(),
            ReasonCode::ApprovalBindingMismatch
        );

        let (engine, request, mut context) = task_attempt_fixture();
        context.approval_status = Some(ApprovalStatus::Approved);
        context.approval_action_digest = approval_action_digest(&request);
        let mut changed_scope = request.clone();
        changed_scope.scope = AuthorizationScope::AttemptExecution {
            task_id: "task-2".into(),
            attempt_id: "attempt-2".into(),
        };
        context.scope = changed_scope.scope.clone();
        context.grant_scope = context.scope.clone();
        changed_scope.request_digest = context.digest_for(&changed_scope);
        context.request_digest = changed_scope.request_digest.clone();
        assert_eq!(
            engine.evaluate(&changed_scope, &context).reason_code(),
            ReasonCode::ApprovalBindingMismatch
        );
    }

    #[test]
    fn decision_value_contains_digests_and_never_input_or_secret_bytes() {
        let (engine, request, context) = fixture();
        let result = engine.evaluate(&request, &context);
        let encoded = serde_json::to_string(&result).expect("decision value serializes");

        assert!(encoded.contains("sha256:"));
        assert!(!encoded.contains("secret bytes"));
        assert!(!encoded.contains("Bearer"));
        assert!(!encoded.contains("token-value"));
        assert!(!encoded.contains("target-plaintext"));
        assert!(!encoded.contains("input-plaintext"));
    }

    #[test]
    fn unknown_execution_method_is_denied() {
        let (engine, mut request, mut context) = fixture();
        request.execution_method = ExecutionMethod::Unknown;
        context.execution_method = ExecutionMethod::Unknown;
        request.request_digest = context.digest_for(&request);
        context.request_digest = request.request_digest.clone();
        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::ExecutionMethodUnknown);
    }

    #[test]
    fn operation_incompatible_execution_method_is_denied() {
        let (engine, mut request, mut context) = fixture();
        request.execution_method = ExecutionMethod::NativeAgentTool;
        context.execution_method = ExecutionMethod::NativeAgentTool;
        request.request_digest = context.digest_for(&request);
        context.request_digest = request.request_digest.clone();
        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::ExecutionMethodNotAllowed);
    }

    #[test]
    fn pinned_approval_does_not_mask_unknown_operation_or_method() {
        let (engine, mut request, mut context) = fixture();
        context.coworker_policy_requires_approval = true;
        request.operation = OperationId::from("unknown.operation");
        context.operation = request.operation.clone();
        context
            .grant_allowed_operations
            .push(request.operation.clone());
        request.request_digest = context.digest_for(&request);
        context.request_digest = request.request_digest.clone();

        let unknown_operation = engine.evaluate(&request, &context);
        assert_eq!(unknown_operation.decision(), DecisionKind::Deny);
        assert_eq!(
            unknown_operation.reason_code(),
            ReasonCode::OperationNotClassified
        );

        request.operation = OperationId::from("workspace.resource.read");
        context.operation = request.operation.clone();
        request.execution_method = ExecutionMethod::NativeAgentTool;
        context.execution_method = request.execution_method;
        request.request_digest = context.digest_for(&request);
        context.request_digest = request.request_digest.clone();

        let incompatible_method = engine.evaluate(&request, &context);
        assert_eq!(incompatible_method.decision(), DecisionKind::Deny);
        assert_eq!(
            incompatible_method.reason_code(),
            ReasonCode::ExecutionMethodNotAllowed
        );
    }

    #[test]
    fn request_input_digest_must_match_the_stored_invocation() {
        let (engine, mut request, context) = fixture();
        request.input_digest = digest_bytes(b"different normalized input");
        request.request_digest = context.digest_for(&request);

        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::RequestDigestMismatch);
    }

    #[test]
    fn malformed_request_digest_is_denied_before_policy_allow() {
        let (engine, mut request, context) = fixture();
        request.request_digest = "sha256:deadbeef".to_owned();
        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::Deny);
        assert_eq!(result.reason_code(), ReasonCode::RequestDigestMismatch);
    }
}
