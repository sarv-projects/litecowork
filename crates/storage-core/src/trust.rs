//! Core persistence contracts for Trust decisions that can be durably recorded
//! without granting authority or dispatching a Capability.

use crate::{DomainEvent, EventDraft, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApprovalDecision {
    Approve,
    Deny,
}

impl ApprovalDecision {
    pub const fn status(self) -> &'static str {
        match self {
            Self::Approve => "APPROVED",
            Self::Deny => "DENIED",
        }
    }
}

/// Authenticated identity supplied by the Core service, never copied from a
/// request body. The owner binding is rechecked by the storage transaction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalActor {
    pub principal_id: String,
    pub principal_kind: String,
    pub owner_principal_id: String,
    pub assurance_level: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRecord {
    pub approval_id: String,
    pub task_id: String,
    pub requested_by_attempt: Option<String>,
    pub kind: String,
    pub action_summary: String,
    pub target_ref: Value,
    pub scope_digest: String,
    pub action_digest: String,
    pub risk: String,
    pub required_assurance: String,
    pub status: String,
    pub requested_at: String,
    pub expires_at: Option<String>,
    pub resolved_by: Option<Value>,
    pub resolved_at: Option<String>,
    pub version: u64,
}

#[derive(Clone, Debug)]
pub struct ResolveApprovalCommit {
    pub workspace_id: String,
    pub actor: ApprovalActor,
    pub request_id: String,
    pub approval_id: String,
    pub expected_version: u64,
    pub decision: ApprovalDecision,
    /// If present, this must equal the persisted exact-action digest. It is not
    /// an authorization credential and never substitutes for actor assurance.
    pub confirmation_digest: Option<String>,
    pub event: EventDraft,
}

/// Internal TrustService-owned request to persist a Task/Attempt-scoped
/// approval. The caller must bind `approval.action_digest` to the canonical
/// action tuple before submitting it; SQLite recomputes and checks that binding.
#[derive(Clone, Debug)]
pub struct RequestApprovalCommit {
    pub workspace_id: String,
    pub actor: ApprovalActor,
    pub request_id: String,
    pub approval: ApprovalRecord,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedApproval {
    pub approval: ApprovalRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalCursor {
    pub workspace_id: String,
    pub status: Option<String>,
    pub limit: usize,
    pub after_requested_at: String,
    pub after_approval_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalPageRequest {
    pub workspace_id: String,
    pub status: Option<String>,
    pub limit: usize,
    pub cursor: Option<ApprovalCursor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalPage {
    pub items: Vec<ApprovalRecord>,
    pub next_cursor: Option<ApprovalCursor>,
}

/// Durable approval decision storage. Resolving an Approval does not consume it,
/// create a CapabilityGrant, issue a SecretLease, or authorize Effect dispatch.
pub trait ApprovalStore: Send + Sync {
    fn request_approval(
        &self,
        command: RequestApprovalCommit,
    ) -> Result<CommittedApproval, StoreError>;

    fn get_approval(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        approval_id: &str,
    ) -> Result<Option<ApprovalRecord>, StoreError>;

    fn list_approvals_page(
        &self,
        owner_principal_id: &str,
        request: ApprovalPageRequest,
    ) -> Result<ApprovalPage, StoreError>;

    fn resolve_approval(
        &self,
        command: ResolveApprovalCommit,
    ) -> Result<CommittedApproval, StoreError>;
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CapabilityGrantStatus {
    Active,
    Revoked,
    Expired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum CapabilityGrantScope {
    Conversation { conversation_id: String },
    TaskPlanning { task_id: String },
    AttemptExecution { task_id: String, attempt_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrantRecord {
    pub capability_grant_id: String,
    pub scope: CapabilityGrantScope,
    pub capability_ref: Value,
    pub allowed_operations: Vec<String>,
    pub resource_scope: Value,
    pub secret_refs: Vec<SecretRef>,
    pub granted_by: Value,
    pub expires_at: Option<String>,
    pub status: CapabilityGrantStatus,
    pub created_at: String,
    pub version: u64,
}

#[derive(Clone, Debug)]
pub struct IssueCapabilityGrantCommit {
    pub workspace_id: String,
    pub actor: ApprovalActor,
    pub request_id: String,
    /// This owner-action path does not consume an Approval. Until a matching
    /// grant-specific ApprovalUse contract is served, a supplied Approval ID
    /// is rejected instead of being treated as authority.
    pub approval_id: Option<String>,
    pub grant: CapabilityGrantRecord,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedCapabilityGrant {
    pub grant: CapabilityGrantRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

#[derive(Clone, Debug)]
pub struct CapabilityGrantRevocationCommit {
    pub workspace_id: String,
    pub actor: ApprovalActor,
    pub request_id: String,
    pub capability_grant_id: String,
    pub expected_version: u64,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedCapabilityGrantRevocation {
    pub capability_grant_id: String,
    pub status: CapabilityGrantStatus,
    pub version: u64,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// Owner-authorized revocation path. It only removes authority; creating a grant
/// and dispatching Invocations remain separate Trust-admission operations.
pub trait CapabilityGrantStore: Send + Sync {
    fn issue_capability_grant(
        &self,
        command: IssueCapabilityGrantCommit,
    ) -> Result<CommittedCapabilityGrant, StoreError>;

    fn revoke_capability_grant(
        &self,
        command: CapabilityGrantRevocationCommit,
    ) -> Result<CommittedCapabilityGrantRevocation, StoreError>;
}

/// Non-secret reference to a credential held by an implementation-owned store.
/// The provider reference is opaque and is not itself permission to resolve bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecretRef {
    pub secret_ref_id: String,
    pub provider_ref: String,
    pub placement: SecretPlacement,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecretPlacement {
    LocalOnly,
    CloudAvailable,
    RuntimeBound,
    ExternalAgentOwned,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecretKind {
    ApiKey,
    Token,
    Password,
    Other,
}

/// Write-only SecretStore boundary. Implementations must consume and zeroize the
/// mutable input on both success and failure. Secret bytes have no durable Core
/// representation and this interface intentionally has no read/get operation.
pub trait SecretStore: Send + Sync {
    fn write(
        &self,
        workspace_id: &str,
        secret_kind: SecretKind,
        secret_bytes: &mut [u8],
    ) -> Result<SecretRef, StoreError>;
}

/// Attempt-scoped, expiring permission for a provider boundary to use one secret.
/// The returned record is Core-only metadata: Agents must receive neither its
/// SecretRef locator nor secret bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecretLeaseRecord {
    pub secret_lease_id: String,
    pub secret_ref: SecretRef,
    pub task_id: String,
    pub attempt_id: String,
    pub capability_ref: Value,
    pub runtime_id: String,
    pub allowed_usage: Vec<String>,
    pub status: String,
    pub issued_at: String,
    pub expires_at: String,
    pub version: u64,
}

#[derive(Clone, Debug)]
pub struct IssueSecretLeaseCommit {
    pub workspace_id: String,
    pub actor: ApprovalActor,
    pub request_id: String,
    pub lease: SecretLeaseRecord,
    pub event: EventDraft,
}

#[derive(Clone, Debug)]
pub struct RevokeSecretLeaseCommit {
    pub workspace_id: String,
    pub actor: ApprovalActor,
    pub request_id: String,
    pub secret_lease_id: String,
    pub expected_version: u64,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedSecretLease {
    pub lease: SecretLeaseRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// Durable SecretLease metadata. This trait deliberately has no credential
/// resolution method; provider delivery requires a separate authorized port.
pub trait SecretLeaseStore: Send + Sync {
    fn issue_secret_lease(
        &self,
        command: IssueSecretLeaseCommit,
    ) -> Result<CommittedSecretLease, StoreError>;

    fn revoke_secret_lease(
        &self,
        command: RevokeSecretLeaseCommit,
    ) -> Result<CommittedSecretLease, StoreError>;
}
