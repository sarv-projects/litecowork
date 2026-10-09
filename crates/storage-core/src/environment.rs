//! Canonical Environment records and transactional persistence port.
//!
//! Provider handles and locators are intentionally absent. They belong to
//! incarnation-scoped Runtime-local bindings, not this durable record.

use crate::{EventDraft, StoreError};
use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, StoreError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(StoreError::Invalid(
                        concat!(stringify!($name), " must be non-empty").to_owned(),
                    ));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

id_type!(EnvironmentId);
id_type!(RuntimeId);
id_type!(RuntimeIncarnationId);
id_type!(WorkspaceId);
id_type!(TaskId);
id_type!(AttemptId);
id_type!(CoworkerId);
id_type!(PrincipalId);
id_type!(ResourceId);
id_type!(ResourceRevisionId);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentStatus {
    New,
    Provisioning,
    Ready,
    Busy,
    Checkpointing,
    Suspended,
    Failed,
    Destroying,
    Destroyed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentClass {
    LocalWorkspace,
    GitWorktree,
    Container,
    Vm,
    CloudSandbox,
    RemoteMachine,
    Browser,
    Desktop,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentLifetime {
    Attempt,
    TaskRetained,
    WorkspacePersistent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentSharingScope {
    AttemptPrivate,
    TaskShared,
    CoworkerPrivate,
    WorkspaceShared,
    UserShared,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilesystemIsolation {
    SharedReadonly,
    PrivateCopy,
    Worktree,
    ContainerFs,
    VmFs,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessIsolation {
    Host,
    Namespace,
    Container,
    Vm,
    Remote,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NetworkMode {
    None,
    Restricted,
    Default,
    Custom,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceScope {
    pub workspace_id: WorkspaceId,
    pub resource_ids: Vec<ResourceId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsolationSpec {
    pub filesystem: FilesystemIsolation,
    pub process: ProcessIsolation,
    pub network: NetworkMode,
    pub write_scope: ResourceScope,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceLimits {
    pub cpu_millis: u32,
    pub memory_bytes: u64,
    pub storage_bytes: u64,
    pub max_processes: Option<u32>,
    pub max_lifetime_seconds: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkPolicy {
    pub mode: NetworkMode,
    pub allowed_domains: Vec<String>,
    pub max_response_bytes: u64,
    pub max_download_bytes: u64,
    pub deny_private_networks: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetCeiling {
    pub max_wall_time_ms: Option<u64>,
    pub max_cost_minor_units: Option<u64>,
    pub currency: Option<String>,
    pub max_tokens: Option<u64>,
    pub max_child_attempts: Option<u32>,
    pub max_concurrency: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BudgetEnforcementPolicy {
    RequireProviderEnforced,
    AllowHostMonitored,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BudgetEnforcement {
    ProviderEnforced,
    HostMonitored,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedSourceResource {
    pub resource_id: ResourceId,
    pub revision_id: ResourceRevisionId,
    pub content_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentOwner {
    pub task_id: Option<TaskId>,
    pub attempt_id: Option<AttemptId>,
    pub coworker_id: Option<CoworkerId>,
    pub principal_id: Option<PrincipalId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentIdentity {
    pub environment_id: EnvironmentId,
    pub runtime_id: RuntimeId,
    pub owner_workspace_id: WorkspaceId,
    pub owner: EnvironmentOwner,
    /// Incarnation that created the Environment, when known. This is immutable and
    /// is not the incarnation currently operating the Environment.
    pub created_by_incarnation_id: Option<RuntimeIncarnationId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentBackupPolicy {
    Excluded,
    IncludeCheckpoints,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentConfig {
    pub name: String,
    pub provider_kind: String,
    pub class: EnvironmentClass,
    pub lifetime: EnvironmentLifetime,
    pub sharing_scope: EnvironmentSharingScope,
    pub source_resources: Vec<PinnedSourceResource>,
    pub resource_limits: ResourceLimits,
    pub network_policy: NetworkPolicy,
    pub budget_ceiling: BudgetCeiling,
    pub budget_enforcement_policy: BudgetEnforcementPolicy,
    pub budget_enforcement: BudgetEnforcement,
    /// Set only when a persistent Environment was created by consuming a validated
    /// provision preview. V1 storage requests cannot create that lifetime yet.
    pub provision_preview_digest: Option<String>,
    pub retention_expires_at: Option<String>,
    pub backup_policy: EnvironmentBackupPolicy,
    pub isolation: IsolationSpec,
}

/// Immutable provision identity/config plus lifecycle projection. Construction and
/// persisted restoration go through `domain_environment` validation; storage adapters
/// should use `from_persisted_parts` and reject an error as corrupt state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EnvironmentRecord {
    identity: EnvironmentIdentity,
    config: EnvironmentConfig,
    status: EnvironmentStatus,
    health: EnvironmentHealth,
    created_at: String,
    updated_at: String,
    version: u64,
}

impl EnvironmentRecord {
    pub fn from_persisted_parts(
        identity: EnvironmentIdentity,
        config: EnvironmentConfig,
        status: EnvironmentStatus,
        health: EnvironmentHealth,
        created_at: String,
        updated_at: String,
        version: u64,
    ) -> Result<Self, StoreError> {
        if version == 0 {
            return Err(StoreError::Invalid(
                "Environment version must be positive".to_owned(),
            ));
        }
        Ok(Self {
            identity,
            config,
            status,
            health,
            created_at,
            updated_at,
            version,
        })
    }

    pub fn identity(&self) -> &EnvironmentIdentity {
        &self.identity
    }
    pub fn config(&self) -> &EnvironmentConfig {
        &self.config
    }
    pub const fn status(&self) -> EnvironmentStatus {
        self.status
    }
    pub const fn health(&self) -> EnvironmentHealth {
        self.health
    }
    pub fn created_at(&self) -> &str {
        &self.created_at
    }
    pub fn updated_at(&self) -> &str {
        &self.updated_at
    }

    /// Returns a copy stamped with the caller's transaction time. Lifecycle policy
    /// remains clock-free; persistence request construction requires this value to
    /// match the event's `recorded_at` exactly.
    pub fn with_updated_at(&self, updated_at: impl Into<String>) -> Result<Self, StoreError> {
        let updated_at = updated_at.into();
        if updated_at.trim().is_empty() {
            return Err(StoreError::Invalid(
                "Environment updated_at must be non-empty".to_owned(),
            ));
        }
        Ok(Self {
            identity: self.identity.clone(),
            config: self.config.clone(),
            status: self.status,
            health: self.health,
            created_at: self.created_at.clone(),
            updated_at,
            version: self.version,
        })
    }
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Only the domain lifecycle module may use this copy constructor to propose a
    /// changed state. Store adapters still recheck immutable identity/config on CAS.
    pub fn with_domain_state(
        &self,
        status: EnvironmentStatus,
        health: EnvironmentHealth,
        version: u64,
    ) -> Result<Self, StoreError> {
        if version != self.version.saturating_add(1) {
            return Err(StoreError::Invalid(
                "Environment transition must advance exactly one version".to_owned(),
            ));
        }
        Ok(Self {
            identity: self.identity.clone(),
            config: self.config.clone(),
            status,
            health,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            version,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedState {
    pub version: u64,
    pub status: EnvironmentStatus,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleHolds {
    pub active_attempts: u32,
    pub unsettled_invocations: u32,
    pub unsettled_control_leases: u32,
    pub checkpoint_holds: u32,
    pub unresolved_effects: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentRequestIdentity {
    /// Authenticated Operator principal. The canonical body represents only the
    /// caller's public command and never includes provider locators, credentials, or
    /// provider receipt bytes.
    principal_id: PrincipalId,
    request_id: String,
    canonical_request_body: Vec<u8>,
    canonical_request_digest: String,
}

impl EnvironmentRequestIdentity {
    pub fn new(
        principal_id: PrincipalId,
        request_id: impl Into<String>,
        canonical_request_body: Vec<u8>,
        canonical_request_digest: impl Into<String>,
    ) -> Result<Self, StoreError> {
        let request_id = request_id.into();
        let canonical_request_digest = canonical_request_digest.into();
        if request_id.trim().is_empty() || canonical_request_body.is_empty() {
            return Err(StoreError::Invalid(
                "request identity requires request_id and canonical body".to_owned(),
            ));
        }
        if !valid_digest(&canonical_request_digest) {
            return Err(StoreError::Invalid(
                "canonical request digest must be a lowercase SHA-256 digest".to_owned(),
            ));
        }
        Ok(Self {
            principal_id,
            request_id,
            canonical_request_body,
            canonical_request_digest,
        })
    }

    pub fn principal_id(&self) -> &PrincipalId {
        &self.principal_id
    }
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    pub fn canonical_request_body(&self) -> &[u8] {
        &self.canonical_request_body
    }
    pub fn canonical_request_digest(&self) -> &str {
        &self.canonical_request_digest
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Runtime-local opaque provider binding. It is written only to the private
/// `environment_provider_bindings` table in the same transaction as READY. Never
/// serialize this value into the request body, durable Environment aggregate, domain
/// event, backup, aggregate blob, or public projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderBindingCommit {
    environment_id: EnvironmentId,
    runtime_id: RuntimeId,
    runtime_incarnation_id: RuntimeIncarnationId,
    provider_kind: String,
    opaque_locator_ref: String,
    observed_at: String,
    expires_at: Option<String>,
}

impl ProviderBindingCommit {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        environment_id: EnvironmentId,
        runtime_id: RuntimeId,
        runtime_incarnation_id: RuntimeIncarnationId,
        provider_kind: impl Into<String>,
        opaque_locator_ref: impl Into<String>,
        observed_at: impl Into<String>,
        expires_at: Option<String>,
    ) -> Result<Self, StoreError> {
        let provider_kind = provider_kind.into();
        let opaque_locator_ref = opaque_locator_ref.into();
        let observed_at = observed_at.into();
        if provider_kind.trim().is_empty()
            || opaque_locator_ref.trim().is_empty()
            || observed_at.trim().is_empty()
            || expires_at
                .as_ref()
                .is_some_and(|timestamp| timestamp.trim().is_empty())
        {
            return Err(StoreError::Invalid(
                "provider binding fields must be non-empty".to_owned(),
            ));
        }
        Ok(Self {
            environment_id,
            runtime_id,
            runtime_incarnation_id,
            provider_kind,
            opaque_locator_ref,
            observed_at,
            expires_at,
        })
    }

    pub fn environment_id(&self) -> &EnvironmentId {
        &self.environment_id
    }
    pub fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }
    pub fn runtime_incarnation_id(&self) -> &RuntimeIncarnationId {
        &self.runtime_incarnation_id
    }
    pub fn provider_kind(&self) -> &str {
        &self.provider_kind
    }
    pub fn opaque_locator_ref(&self) -> &str {
        &self.opaque_locator_ref
    }
    pub fn observed_at(&self) -> &str {
        &self.observed_at
    }
    pub fn expires_at(&self) -> Option<&str> {
        self.expires_at.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentCreateRequest {
    identity: EnvironmentRequestIdentity,
    record: EnvironmentRecord,
    event: EventDraft,
}

impl EnvironmentCreateRequest {
    /// Creates only V1-supported scoped Environment requests. Workspace-persistent
    /// creation requires a typed, validated preview-consumption transaction and is
    /// deliberately rejected until that contract is implemented.
    pub fn new(
        identity: EnvironmentRequestIdentity,
        record: EnvironmentRecord,
        event: EventDraft,
    ) -> Result<Self, StoreError> {
        if record.config.lifetime == EnvironmentLifetime::WorkspacePersistent {
            return Err(StoreError::Invalid(
                "WORKSPACE_PERSISTENT creation requires typed preview consumption, which is not implemented".to_owned(),
            ));
        }
        if record.status != EnvironmentStatus::Provisioning {
            return Err(StoreError::Invalid(
                "Environment creation requires PROVISIONING state".to_owned(),
            ));
        }
        if record.updated_at != event.recorded_at {
            return Err(StoreError::Invalid(
                "Environment updated_at must equal event recorded_at".to_owned(),
            ));
        }
        Ok(Self {
            identity,
            record,
            event,
        })
    }

    pub fn request_id(&self) -> &str {
        self.identity.request_id()
    }
    pub fn identity(&self) -> &EnvironmentRequestIdentity {
        &self.identity
    }
    pub fn record(&self) -> &EnvironmentRecord {
        &self.record
    }
    pub fn event(&self) -> &EventDraft {
        &self.event
    }
}

/// Stable-key Environment listing. `after_environment_id` is an exclusive cursor;
/// callers must supply an explicit positive limit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentListRequest {
    workspace_id: WorkspaceId,
    after_environment_id: Option<EnvironmentId>,
    limit: u32,
}

impl EnvironmentListRequest {
    pub fn new(
        workspace_id: WorkspaceId,
        after_environment_id: Option<EnvironmentId>,
        limit: u32,
    ) -> Result<Self, StoreError> {
        if limit == 0 {
            return Err(StoreError::Invalid(
                "Environment list limit must be positive".to_owned(),
            ));
        }
        Ok(Self {
            workspace_id,
            after_environment_id,
            limit,
        })
    }

    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }
    pub fn after_environment_id(&self) -> Option<&EnvironmentId> {
        self.after_environment_id.as_ref()
    }
    pub const fn limit(&self) -> u32 {
        self.limit
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentLifecycleRequest {
    identity: EnvironmentRequestIdentity,
    environment_id: EnvironmentId,
    workspace_id: WorkspaceId,
    expected: ExpectedState,
    proposed: EnvironmentRecord,
    provider_binding: Option<ProviderBindingCommit>,
    event: EventDraft,
}

/// Explicit owner command for the F65 sharing-scope transition. The public request
/// digest covers the selected target and expected version; Runtime/provider data is
/// deliberately excluded.
#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentSharingScopeChangeRequest {
    identity: EnvironmentRequestIdentity,
    environment_id: EnvironmentId,
    workspace_id: WorkspaceId,
    expected_version: u64,
    target_scope: EnvironmentSharingScope,
    target_coworker_id: Option<CoworkerId>,
    event: EventDraft,
}

impl EnvironmentSharingScopeChangeRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: EnvironmentRequestIdentity,
        environment_id: EnvironmentId,
        workspace_id: WorkspaceId,
        expected_version: u64,
        target_scope: EnvironmentSharingScope,
        target_coworker_id: Option<CoworkerId>,
        event: EventDraft,
    ) -> Result<Self, StoreError> {
        if expected_version == 0 || expected_version == u64::MAX {
            return Err(StoreError::Invalid(
                "Environment sharing-scope expected version is invalid".to_owned(),
            ));
        }
        match (target_scope, target_coworker_id.as_ref()) {
            (EnvironmentSharingScope::CoworkerPrivate, Some(_))
            | (EnvironmentSharingScope::WorkspaceShared, None) => {}
            _ => {
                return Err(StoreError::Invalid(
                    "Environment sharing-scope target is unavailable or has invalid owner"
                        .to_owned(),
                ));
            }
        }
        Ok(Self {
            identity,
            environment_id,
            workspace_id,
            expected_version,
            target_scope,
            target_coworker_id,
            event,
        })
    }

    pub fn identity(&self) -> &EnvironmentRequestIdentity {
        &self.identity
    }
    pub fn environment_id(&self) -> &EnvironmentId {
        &self.environment_id
    }
    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }
    pub const fn expected_version(&self) -> u64 {
        self.expected_version
    }
    pub const fn target_scope(&self) -> EnvironmentSharingScope {
        self.target_scope
    }
    pub fn target_coworker_id(&self) -> Option<&CoworkerId> {
        self.target_coworker_id.as_ref()
    }
    pub fn event(&self) -> &EventDraft {
        &self.event
    }
}

impl EnvironmentLifecycleRequest {
    pub fn new(
        identity: EnvironmentRequestIdentity,
        environment_id: EnvironmentId,
        workspace_id: WorkspaceId,
        expected: ExpectedState,
        proposed: EnvironmentRecord,
        provider_binding: Option<ProviderBindingCommit>,
        event: EventDraft,
    ) -> Result<Self, StoreError> {
        if proposed.identity.environment_id != environment_id
            || proposed.identity.owner_workspace_id != workspace_id
        {
            return Err(StoreError::Invalid(
                "lifecycle request scope does not match proposed Environment".to_owned(),
            ));
        }
        if proposed.updated_at != event.recorded_at {
            return Err(StoreError::Invalid(
                "Environment updated_at must equal event recorded_at".to_owned(),
            ));
        }
        if (proposed.status == EnvironmentStatus::Ready) != provider_binding.is_some() {
            return Err(StoreError::Invalid(
                "READY transition requires a Runtime-local provider binding; other transitions must not supply one".to_owned(),
            ));
        }
        if let Some(binding) = &provider_binding {
            if binding.environment_id != environment_id
                || binding.runtime_id != proposed.identity.runtime_id
                || binding.provider_kind != proposed.config.provider_kind
            {
                return Err(StoreError::Invalid(
                    "provider binding does not match the Environment identity".to_owned(),
                ));
            }
        }
        Ok(Self {
            identity,
            environment_id,
            workspace_id,
            expected,
            proposed,
            provider_binding,
            event,
        })
    }

    pub fn identity(&self) -> &EnvironmentRequestIdentity {
        &self.identity
    }
    pub fn environment_id(&self) -> &EnvironmentId {
        &self.environment_id
    }
    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }
    pub const fn expected(&self) -> ExpectedState {
        self.expected
    }
    pub fn proposed(&self) -> &EnvironmentRecord {
        &self.proposed
    }
    pub fn provider_binding(&self) -> Option<&ProviderBindingCommit> {
        self.provider_binding.as_ref()
    }
    pub fn event(&self) -> &EventDraft {
        &self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedEnvironment {
    pub record: EnvironmentRecord,
    pub replayed: bool,
}

/// Transactional persistence seam. Implementations must re-read the current record
/// under the aggregate write lock, enforce exact version/status CAS and immutable
/// identity/config equality, and recheck authoritative Attempt/Invocation/control
/// lease/checkpoint/Effect holds in the same transaction before accepting transitions
/// into CHECKPOINTING or DESTROYING. Caller-supplied domain decisions are not storage
/// proof and must not replace these checks.
/// `create_environment` accepts only a previously authorized PROVISIONING record and
/// atomically commits that record, its initial event, and request receipt. This port
/// does not reserve budget or invoke providers; provider admission must remain blocked
/// until a separate budget reservation is atomically integrated. The idempotency receipt is owner-scoped by
/// `(principal_id, request_id)` and binds the canonical Operator command body/digest.
/// Adapters must recompute and compare the digest. Provider outputs and opaque binding
/// bytes are not part of that body. Its request constructor rejects WORKSPACE_PERSISTENT;
/// only ATTEMPT and TASK_RETAINED are supported until typed preview consumption exists.
/// `transition_environment` atomically commits the
/// versioned record, event, and idempotency receipt. Neither method invokes a provider.
/// A transition to READY must atomically persist the supplied `ProviderBindingCommit`
/// in Runtime-local storage. The adapter revalidates its Environment/Runtime/provider
/// match, active Runtime incarnation, and observed/expiry times in that transaction;
/// it must never put the locator in durable state, events, backups, or projections.
pub trait EnvironmentStore: Send + Sync {
    /// Return at most `request.limit()` records in stable Environment ID order,
    /// scoped to the requested Workspace.
    fn list_environments(
        &self,
        request: EnvironmentListRequest,
    ) -> Result<Vec<EnvironmentRecord>, StoreError>;
    fn get_environment(
        &self,
        workspace_id: &str,
        environment_id: &str,
    ) -> Result<Option<EnvironmentRecord>, StoreError>;
    fn create_environment(
        &self,
        request: EnvironmentCreateRequest,
    ) -> Result<CommittedEnvironment, StoreError>;
    fn transition_environment(
        &self,
        request: EnvironmentLifecycleRequest,
    ) -> Result<CommittedEnvironment, StoreError>;
    /// Atomically changes only a suspended persistent Environment's supported sharing
    /// boundary and owner. Must recheck all represented active-use holds under the
    /// same SQLite write transaction and append the scope event/receipt with the row.
    fn change_environment_sharing_scope(
        &self,
        request: EnvironmentSharingScopeChangeRequest,
    ) -> Result<CommittedEnvironment, StoreError>;
}
