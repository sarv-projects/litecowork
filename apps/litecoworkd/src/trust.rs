//! Core-internal Trust policy evaluation tests and implementation.
//!
//! This module is deliberately not an Operator API. Evaluation does not dispatch
//! providers, issue grants/secrets, persist an audit row, or create an independently
//! reusable permit. It is a pure decision primitive only; the CapabilityBroker's
//! atomic admission transaction and audit persistence are not implemented here.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

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
id_type!(CapabilityRef);
id_type!(OperationId);
id_type!(EffectId);

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
    WorkspacePolicyDenied,
    ApprovalBindingMismatch,
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
                        || digest != digest_serializable(&ActionDigestInput::from(self))
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
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PolicyEngine, AuthorizationRequest, StoredEvaluationContext) {
        let engine = PolicyEngine::core_v1();
        let context = StoredEvaluationContext::test_valid();
        let request = context.authorization_request();
        (engine, request, context)
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
        let (engine, mut request, mut context) = fixture();
        request.operation = OperationId::from("external.message.send");
        request.effect_id = Some(EffectId::from("effect-1"));
        context.operation = request.operation.clone();
        context.effect_id = request.effect_id.clone();
        context.effect_proposed = true;
        context
            .grant_allowed_operations
            .push(request.operation.clone());
        request.request_digest = context.digest_for(&request);
        context.request_digest = request.request_digest.clone();

        let result = engine.evaluate(&request, &context);
        assert_eq!(result.decision(), DecisionKind::RequireApproval);
        assert_eq!(result.reason_code(), ReasonCode::ApprovalRequired);
        assert_eq!(result.effect_id(), request.effect_id.as_ref());
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
