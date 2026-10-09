use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

mod execution;
pub use execution::{
    AdmitStepAttempt, AttemptAdmissionChecks, AttemptAdmissionEvents, AttemptBudgetAdmission,
    AttemptBudgetReservationRecord, AttemptRecord, AttemptState, CommittedExecutionLeaseMutation,
    CommittedStepAttempt, ExecutionBudgetReservationState, ExecutionBudgetScope, ExecutionCheck,
    ExecutionEventContext, ExecutionLeaseCommand, ExecutionLeaseMutationSnapshot,
    ExecutionLeaseRecord, ExecutionLeaseState, ExpireExecutionLease, ExpiredAttemptEvents,
    ExpiredExecutionLeaseCandidate, LeaseReleasePhase, ReleaseExecutionLease, RenewExecutionLease,
    StepAttemptAdmissionSnapshot, StepAttemptStore,
};

mod artifact;
pub use artifact::{
    ArtifactAppendHeads, ArtifactContentRecord, ArtifactLibraryAction, ArtifactLibraryCommand,
    ArtifactLibraryWriteStore, ArtifactReadStore, ArtifactRecord, ArtifactVersionAppendCommit,
    ArtifactVersionRecord, ArtifactVersionWriteStore, CommittedArtifactLibraryCommand,
    CommittedArtifactVersionAppend,
};

mod effect_evidence;
pub use effect_evidence::{
    AppendEvidenceCommit, CommittedEffect, CommittedEvidence, EffectEvidenceEventContext,
    EffectEvidenceStore, EffectFenceBinding, EffectRetryAuthorization, EffectRetryBasis,
    EffectTransitionMetadata, ProposeEffectCommit, TransitionEffectCommit,
};
mod task_presentation;
pub use task_presentation::{
    MAX_TASK_PRESENTATION_ARTIFACTS, MAX_TASK_PRESENTATION_STEPS, TaskPresentationActivityEvent,
    TaskPresentationArtifactVersion, TaskPresentationCurrentAttempt, TaskPresentationReadModel,
    TaskPresentationReadStore,
};
pub mod rich_presentation;

mod environment;
pub use environment::{
    AttemptId, BudgetCeiling, BudgetEnforcement, BudgetEnforcementPolicy, CommittedEnvironment,
    CoworkerId, EnvironmentBackupPolicy, EnvironmentClass, EnvironmentConfig,
    EnvironmentCreateRequest, EnvironmentHealth, EnvironmentId, EnvironmentIdentity,
    EnvironmentLifecycleRequest, EnvironmentLifetime, EnvironmentListRequest, EnvironmentOwner,
    EnvironmentRecord, EnvironmentRequestIdentity, EnvironmentSharingScope,
    EnvironmentSharingScopeChangeRequest, EnvironmentStatus, EnvironmentStore, ExpectedState,
    FilesystemIsolation, IsolationSpec, LifecycleHolds, NetworkMode, NetworkPolicy,
    PinnedSourceResource, PrincipalId, ProcessIsolation, ProviderBindingCommit, ResourceId,
    ResourceLimits, ResourceRevisionId, ResourceScope, RuntimeId, RuntimeIncarnationId, TaskId,
    WorkspaceId,
};
// Domain command values and their transactional write port are defined with the
// domain service; storage-core re-exports the port as the adapter boundary.
pub use domain_responsibility::{
    CommittedDelegationProfile, DelegationProfile, DelegationProfileAppend,
    DelegationProfileCommand, DelegationProfileCommandScope, DelegationProfileError,
    DelegationProfileEvent, DelegationProfileMutation, DelegationProfileRevision,
    DelegationProfileStore, DelegationProfileTransaction,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlobRef {
    pub digest: String,
    pub size_bytes: u64,
    pub media_type: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobPurpose {
    AggregateState,
    Artifact,
    Resource,
    ResourceIndex,
    Checkpoint,
    ResourceUploadChunk,
    RichPresentation,
}

impl BlobPurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AggregateState => "AGGREGATE_STATE",
            Self::Artifact => "ARTIFACT",
            Self::Resource => "RESOURCE",
            Self::ResourceIndex => "RESOURCE_INDEX",
            Self::Checkpoint => "CHECKPOINT",
            Self::ResourceUploadChunk => "RESOURCE_UPLOAD_CHUNK",
            Self::RichPresentation => "RICH_PRESENTATION",
        }
    }
}

pub trait BlobStore: Send + Sync {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError>;

    fn get(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<Vec<u8>, StoreError>;

    /// Remove exactly this content-addressed object. Callers must fence new references
    /// and prove it is unreferenced before invoking deletion. Missing objects are a no-op.
    fn remove(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
        _blob: &BlobRef,
    ) -> Result<(), StoreError> {
        Err(StoreError::Blob(
            "blob deletion is unsupported by this provider".to_owned(),
        ))
    }

    fn verify(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<(), StoreError> {
        self.get(workspace_id, purpose, blob).map(|_| ())
    }

    /// Produce a workspace-scoped blind token for deterministic local index terms.
    /// Implementations must use a keyed MAC derived from the exact Workspace and
    /// ResourceIndex key scope. The token must not disclose the source term. A version
    /// is returned so indexes remain searchable across key rotation while old keys are
    /// retained. Generic BlobStore providers may leave this unsupported.
    fn resource_index_token(
        &self,
        _workspace_id: &str,
        _key_version: Option<u32>,
        _normalized_term: &str,
    ) -> Result<(u32, String), StoreError> {
        Err(StoreError::Blob(
            "workspace-scoped Resource index tokens are unsupported by this provider".to_owned(),
        ))
    }

    /// Batch variant that lets credential-backed providers load/derive the Workspace
    /// HMAC key once for a bounded set of terms.
    fn resource_index_tokens(
        &self,
        workspace_id: &str,
        key_version: Option<u32>,
        normalized_terms: &[String],
    ) -> Result<(u32, Vec<String>), StoreError> {
        let mut version = key_version;
        let mut tokens = Vec::with_capacity(normalized_terms.len());
        for term in normalized_terms {
            let (resolved_version, token) =
                self.resource_index_token(workspace_id, version, term)?;
            if version.is_some_and(|known| known != resolved_version) {
                return Err(StoreError::Integrity(
                    "Resource index key version changed during token derivation".to_owned(),
                ));
            }
            version = Some(resolved_version);
            tokens.push(token);
        }
        version.map(|value| (value, tokens)).ok_or_else(|| {
            StoreError::Invalid(
                "cannot derive Resource index tokens for an empty term set".to_owned(),
            )
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AggregateStateRef {
    pub blob: BlobRef,
    pub entity_revision: u64,
    pub record_schema_version: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainEvent {
    pub event_id: String,
    pub workspace_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub origin_runtime_id: String,
    pub origin_sequence: u64,
    pub entity_revision: u64,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    pub schema_version: u32,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Value,
    pub aggregate_state_ref: AggregateStateRef,
    pub recorded_at: String,
    pub payload_digest: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventDraft {
    pub event_id: String,
    pub workspace_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub origin_runtime_id: String,
    pub entity_revision: u64,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    pub schema_version: u32,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Value,
    pub recorded_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReplicationPolicy {
    LocalOnly,
    MetadataOnly,
    ActiveTaskInputs,
    SelectedFolders,
    FullWorkspace,
}

impl ReplicationPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LocalOnly => "LOCAL_ONLY",
            Self::MetadataOnly => "METADATA_ONLY",
            Self::ActiveTaskInputs => "ACTIVE_TASK_INPUTS",
            Self::SelectedFolders => "SELECTED_FOLDERS",
            Self::FullWorkspace => "FULL_WORKSPACE",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub workspace_id: String,
    pub name: String,
    pub owner_principal_id: String,
    pub replication_policy: ReplicationPolicy,
    pub replication_scope_root_ids: Vec<String>,
    pub current_instruction_revision: Option<u64>,
    pub default_agent_binding_id: Option<String>,
    pub primary_coworker_id: Option<String>,
    pub hub_runtime_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommittedWorkspace {
    pub workspace: Workspace,
    pub event: DomainEvent,
}

pub trait StateStore: Send + Sync {
    fn get_workspace(&self, workspace_id: &str) -> Result<Option<Workspace>, StoreError>;

    /// Lists all stored Workspaces by `created_at` ascending, then ID ascending.
    fn list_workspaces(&self) -> Result<Vec<Workspace>, StoreError>;

    /// Commits aggregate state and its immutable event through one transaction owner.
    fn commit_workspace(
        &self,
        expected_version: Option<u64>,
        workspace: Workspace,
        event: EventDraft,
    ) -> Result<CommittedWorkspace, StoreError>;
}

pub trait EventStore: Send + Sync {
    fn read_workspace_events(&self, workspace_id: &str) -> Result<Vec<DomainEvent>, StoreError>;
}

pub trait WorkspaceStore: StateStore + EventStore {}

/// Public installation identity stored with a Runtime descriptor. Private signing
/// material is owned by the platform credential-store adapter and never crosses this
/// storage port.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceIdentityRecord {
    pub device_id: String,
    /// `ed25519:` followed by lowercase hexadecimal encoding of the raw public key.
    pub public_key: String,
    pub key_version: u32,
    pub issued_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRecord {
    pub runtime_id: String,
    pub device_identity: DeviceIdentityRecord,
    pub runtime_version: String,
    pub platform: String,
    pub architecture: String,
    pub roles: Vec<String>,
    pub trust_zone: String,
    pub availability: String,
    pub startup_policy: String,
    pub current_incarnation_id: String,
    pub resource_capacity: Value,
    pub last_seen: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIncarnationRecord {
    pub runtime_incarnation_id: String,
    pub runtime_id: String,
    pub process_started_at: String,
    pub litecowork_version: String,
    pub recovered_from_unclean_shutdown: bool,
    pub recovery_state: String,
    pub ready_at: Option<String>,
    pub stopped_at: Option<String>,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIncarnationLocalObservationRecord {
    pub runtime_incarnation_id: String,
    pub os_boot_id: Option<String>,
    pub observed_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIncarnationStateUpdate {
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub expected_version: u64,
    pub recovery_state: String,
    pub availability: String,
    pub observed_at: String,
    pub stopped_at: Option<String>,
}

/// Local Runtime lifecycle persistence. Registration commits the Runtime descriptor,
/// one fresh incarnation, and its local observation atomically. It does not enroll a
/// Workspace or publish Mesh presence.
pub trait RuntimeLifecycleStore: Send + Sync {
    fn register_local_incarnation(
        &self,
        runtime: RuntimeRecord,
        incarnation: RuntimeIncarnationRecord,
        observation: RuntimeIncarnationLocalObservationRecord,
    ) -> Result<RuntimeIncarnationRecord, StoreError>;

    fn transition_local_incarnation(
        &self,
        update: RuntimeIncarnationStateUpdate,
    ) -> Result<RuntimeIncarnationRecord, StoreError>;
}

/// Explicit authorization for one Runtime installation to act within one Workspace.
/// Local enrollment is control-plane state: it is not a Workspace domain event and
/// does not imply Mesh pairing, replication, or Task/AgentSession execution readiness.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkspaceBindingRecord {
    pub runtime_workspace_binding_id: String,
    pub runtime_id: String,
    pub workspace_id: String,
    pub enrollment_mode: String,
    pub status: String,
    pub roles: Vec<String>,
    pub created_at: String,
    pub activated_at: Option<String>,
    pub revoked_at: Option<String>,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalRuntimeWorkspaceEnrollmentRequest {
    pub request: WorkspaceCreateRequest,
    pub expected_workspace_version: u64,
    pub runtime_workspace_binding_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub workspace_id: String,
    pub now: String,
    pub correlation_id: String,
}

/// Identifies one exact local Runtime incarnation when refreshing Workspace
/// enrollment state. The lookup never falls back to another Runtime or incarnation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalRuntimeWorkspaceBindingLookup {
    pub owner_principal_id: String,
    pub workspace_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
}

/// Creates an audited LOCAL_ENROLLMENT control-plane record for the exact current
/// local Runtime incarnation. An active local Runtime may be ONLINE/READY or
/// DEGRADED/DEGRADED; this binding is not proof that Task/AgentSession admission is
/// ready. The operation is idempotent and leaves Workspace version and Workspace
/// event history unchanged.
pub trait RuntimeWorkspaceBindingStore: Send + Sync {
    fn enroll_local_runtime(
        &self,
        request: LocalRuntimeWorkspaceEnrollmentRequest,
    ) -> Result<RuntimeWorkspaceBindingRecord, StoreError>;

    /// Returns the active, nonrevoked enrollment for this owner, Workspace, and
    /// exact current local Runtime incarnation, if one exists and is serving control
    /// plane requests. This does not establish Task/AgentSession execution readiness.
    fn get_current_local_binding(
        &self,
        lookup: LocalRuntimeWorkspaceBindingLookup,
    ) -> Result<Option<RuntimeWorkspaceBindingRecord>, StoreError>;
}

/// Stable agent software identity. This is a catalog record, not proof that any
/// particular Runtime currently has a usable endpoint for it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProfileRecord {
    pub agent_profile_id: String,
    pub provider_key: String,
    pub display_name: String,
    pub discovered_at: String,
}

/// Stable protocol route identity. It deliberately contains no command, socket,
/// URL, credentials, or native session handle.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentEndpointRecord {
    pub endpoint_id: String,
    pub agent_profile_id: String,
    pub protocol: String,
    pub topology: String,
    pub protocol_version: Option<String>,
    pub capabilities: Value,
}

/// Private endpoint locator accepted only from the exact local Runtime incarnation.
/// Callers must keep this type inside Runtime-local adapter code; it is never returned
/// by profile, offer, binding, or Workspace Operator projections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalAgentEndpointBindingInput {
    pub endpoint_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub endpoint_ref: String,
    pub observed_at: String,
    pub expires_at: Option<String>,
}

/// Locator-bearing result for Runtime-local use only. Never serialize this into an
/// event, Workspace response, diagnostic, or backup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalAgentEndpointBindingRecord {
    pub endpoint_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub endpoint_ref: String,
    pub observed_at: String,
    pub expires_at: Option<String>,
}

/// Runtime-operational readiness observation; it expires and is not a domain event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOfferRecord {
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub offer_kind: String,
    pub offer_ref: String,
    pub compatible: bool,
    pub readiness: String,
    pub constraints: Value,
    pub observed_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentEndpointViewRecord {
    pub endpoint: AgentEndpointRecord,
    pub offers: Vec<RuntimeOfferRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProfileViewRecord {
    pub profile: AgentProfileRecord,
    pub endpoints: Vec<AgentEndpointViewRecord>,
}

/// Workspace-scoped agent authorization. `endpoint_selection_policy` is structured;
/// configuration is accepted only when validated against an adapter-specific
/// non-secret allowlist. Until such an allowlist exists, storage requires `{}`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentBindingRecord {
    pub agent_binding_id: String,
    pub workspace_id: String,
    pub agent_profile_id: String,
    pub runtime_id: Option<String>,
    pub endpoint_selection_policy: Value,
    pub auth_ref: Option<Value>,
    pub configuration: Value,
    pub enabled: bool,
    pub lead_eligible: bool,
    pub created_at: String,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedAgentBinding {
    pub binding: AgentBindingRecord,
    pub event: DomainEvent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentBindingCreateRequest {
    pub request: WorkspaceCreateRequest,
    pub binding: AgentBindingRecord,
    pub now: String,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentBindingEnableRequest {
    pub request: WorkspaceCreateRequest,
    pub workspace_id: String,
    pub agent_binding_id: String,
    pub expected_version: u64,
    pub now: String,
    pub event: EventDraft,
}

/// Agent catalog persistence is split by ownership: stable profiles/endpoints are
/// non-secret identities; offers and endpoint locators are Runtime-local observations;
/// AgentBindings are Workspace authorization state and therefore carry domain events.
pub trait AgentCatalogStore: Send + Sync {
    fn put_agent_profile(
        &self,
        profile: AgentProfileRecord,
        endpoints: Vec<AgentEndpointRecord>,
    ) -> Result<(), StoreError>;

    fn register_local_endpoint_binding(
        &self,
        binding: LocalAgentEndpointBindingInput,
    ) -> Result<(), StoreError>;

    /// The only locator-bearing read. The implementation enforces that the requested
    /// Runtime/incarnation is still current and that the local binding has not expired.
    fn get_local_endpoint_binding(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        endpoint_id: &str,
        now: &str,
    ) -> Result<Option<LocalAgentEndpointBindingRecord>, StoreError>;

    fn publish_runtime_offer(&self, offer: RuntimeOfferRecord) -> Result<(), StoreError>;

    fn list_agent_profiles(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        now: &str,
    ) -> Result<Vec<AgentProfileViewRecord>, StoreError>;

    fn create_agent_binding(
        &self,
        request: AgentBindingCreateRequest,
    ) -> Result<CommittedAgentBinding, StoreError>;

    fn get_agent_binding(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        agent_binding_id: &str,
    ) -> Result<Option<AgentBindingRecord>, StoreError>;

    fn list_agent_bindings(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<AgentBindingRecord>, StoreError>;

    fn enable_agent_binding(
        &self,
        request: AgentBindingEnableRequest,
    ) -> Result<CommittedAgentBinding, StoreError>;
}

/// Durable Task aggregate projection. Task creation begins in READY/planning and does
/// not imply a planning session, Step, Attempt, lease, or Environment exists.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskRecord {
    pub task_id: String,
    pub workspace_id: String,
    pub conversation_id: Option<String>,
    pub current_spec_revision: u64,
    pub current_plan_revision: Option<u64>,
    pub status: String,
    pub resume_status: Option<String>,
    pub routine_id: Option<String>,
    pub routine_revision: Option<u64>,
    pub automation_id: Option<String>,
    pub automation_occurrence_id: Option<String>,
    pub origin_coworker_id: Option<String>,
    pub origin_coworker_revision: Option<u64>,
    pub lead_agent_binding_id: String,
    pub blocking_conditions: Vec<Value>,
    pub priority: String,
    pub created_by: Value,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub version: u64,
}

/// Immutable Task intent revision. JSON-valued fields retain the canonical wire contract
/// without making the storage port depend on a particular domain crate's value types.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpecRevisionRecord {
    pub task_id: String,
    pub workspace_id: String,
    pub revision: u64,
    pub parent_revisions: Vec<u64>,
    pub objective: String,
    pub task_category: Option<String>,
    pub constraints: Vec<String>,
    pub non_goals: Vec<String>,
    pub input_refs: Vec<Value>,
    pub workspace_instruction_revision: Option<u64>,
    pub required_outputs: Vec<Value>,
    pub acceptance_criteria: Vec<Value>,
    pub approvals_required: Vec<Value>,
    pub budget: Option<Value>,
    pub delegation_budget_policy: Option<Value>,
    pub lead_failover_policy: Value,
    pub deadline: Option<String>,
    pub source_message_refs: Vec<String>,
    pub placement_preference: Value,
    pub preferred_lead_agent_binding_id: Option<String>,
    pub authored_by: Value,
    pub created_at: String,
}

/// The Task aggregate snapshot referenced by its event includes the current immutable
/// intent revision, allowing a receiver to reconstruct creation without guessing fields.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskAggregateSnapshot {
    pub task: TaskRecord,
    pub current_spec_revision: TaskSpecRevisionRecord,
    #[serde(default)]
    pub current_plan_revision: Option<PlanRevisionRecord>,
    #[serde(default)]
    pub current_steps: Vec<StepRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskView {
    pub task: TaskRecord,
    pub current_spec_revision: TaskSpecRevisionRecord,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSummaryRecord {
    pub task_id: String,
    pub status: String,
    pub objective: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A create command whose deduplication receipt, Task, first spec revision and event are
/// committed by one SQLite transaction. The expected Coworker version is an optimistic
/// concurrency check, not a caller-selected revision.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCreateCommit {
    pub request: WorkspaceCreateRequest,
    /// Optional source-command identity when normalized Task materialization contains
    /// server-resolved defaults. It binds retries to the user's exact Routine request.
    #[serde(default)]
    pub idempotency_payload: Option<serde_json::Value>,
    pub expected_coworker_version: Option<u64>,
    /// Present only for a manual Routine run. SQLite revalidates this exact active
    /// revision and its materialized bounded inputs in the same Task commit.
    #[serde(default)]
    pub routine_admission: Option<RoutineTaskAdmission>,
    /// Present only for a Task atomically admitted from a claimed Automation
    /// occurrence. Storage owns the occurrence insert/claim/materialization writes.
    #[serde(default)]
    pub automation_admission: Option<AutomationTaskAdmission>,
    pub task: TaskRecord,
    pub initial_spec_revision: TaskSpecRevisionRecord,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RoutineTaskAdmission {
    pub routine_id: String,
    pub routine_revision: u64,
    pub inputs: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationTaskAdmission {
    pub automation_id: String,
    pub automation_revision: u64,
    pub expected_automation_version: u64,
    pub routine_id: String,
    pub routine_revision: u64,
    pub trigger_id: String,
    pub trigger_host_runtime_id: String,
    pub trigger_host_runtime_incarnation_id: String,
    /// Fences this one-shot admission against revocation/re-enrollment of the local
    /// Workspace binding. Recurring cursor host_epoch remains a separate fence.
    pub trigger_host_binding_version: u64,
    pub occurrence_id: String,
    pub occurrence_key: String,
    pub claim_expires_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationOccurrenceRecord {
    pub workspace_id: String,
    pub occurrence_id: String,
    pub automation_id: String,
    pub automation_revision: u64,
    pub routine_id: String,
    pub routine_revision: u64,
    pub trigger_id: String,
    pub trigger_host_runtime_id: String,
    pub occurrence_key: String,
    pub status: String,
    pub version: u64,
    pub claim_epoch: u64,
    pub claim_expires_at: Option<String>,
    pub task_id: Option<String>,
    pub scheduled_for: Option<String>,
    pub trigger_input_ref: Option<serde_json::Value>,
    pub trigger_payload_digest: Option<String>,
    pub covered_misfire_range: Option<serde_json::Value>,
    pub blockers: Vec<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

pub trait AutomationOccurrenceReadStore: Send + Sync {
    fn get_automation_occurrence(
        &self,
        principal_id: &str,
        workspace_id: &str,
        automation_id: &str,
        occurrence_id: &str,
    ) -> Result<Option<AutomationOccurrenceRecord>, StoreError>;
}

/// One owner acceptance that must create an ordinary READY Task and resolve the
/// source Suggestion in the same storage transaction. The Suggestion event is
/// independently versioned; the Task event and request receipt remain the normal
/// Task creation contract.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestedTaskCreateCommit {
    pub task: TaskCreateCommit,
    pub suggestion_id: String,
    pub expected_suggestion_version: u64,
    pub accepted_at: String,
    pub suggestion_event: EventDraft,
}

/// Bounded response proof for the atomic Suggestion -> READY Task transition.
/// The duplicated Task ID is intentional: callers must verify that the accepted
/// Suggestion's result link names the same READY Task in the same Workspace.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuggestionAcceptanceDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionAcceptanceLink {
    pub suggestion_id: String,
    pub status: domain_responsibility::SuggestionStatus,
    pub result_task_id: String,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuggestedTaskAcceptanceStatus {
    Ready,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestedTaskAcceptanceTaskLink {
    pub task_id: String,
    pub workspace_id: String,
    pub status: SuggestedTaskAcceptanceStatus,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionTaskAcceptanceReceipt {
    pub workspace_id: String,
    pub disposition: SuggestionAcceptanceDisposition,
    pub suggestion: SuggestionAcceptanceLink,
    pub task: SuggestedTaskAcceptanceTaskLink,
}

pub trait SuggestionTaskAcceptanceStore: Send + Sync {
    fn create_task_from_suggestion(
        &self,
        commit: SuggestedTaskCreateCommit,
    ) -> Result<SuggestionTaskAcceptanceReceipt, StoreError>;

    /// Read the durable result of a prior acceptance retry in one SQLite read
    /// transaction. `expected_task_id` is derived from the authenticated principal,
    /// request id, and Suggestion identity; this prevents a different idempotency key
    /// from using the accepted Suggestion as a lookup oracle.
    fn get_suggestion_task_acceptance_receipt(
        &self,
        principal_id: &str,
        workspace_id: &str,
        suggestion_id: &str,
        expected_suggestion_version: u64,
        request_id: &str,
        expected_task_id: &str,
    ) -> Result<Option<SuggestionTaskAcceptanceReceipt>, StoreError>;
}

/// An atomic owner-authored TaskSpec revision. The Task aggregate version is pinned
/// separately from the immutable spec parent so concurrent edits cannot both advance
/// the current head.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpecRevisionCommit {
    pub request: WorkspaceCreateRequest,
    pub expected_task_version: u64,
    pub task: TaskRecord,
    pub task_spec_revision: TaskSpecRevisionRecord,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedTaskSpecRevision {
    pub revision: TaskSpecRevisionRecord,
    pub task_version: u64,
    pub event: DomainEvent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedTask {
    pub view: TaskView,
    pub event: DomainEvent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedStepRecord {
    pub logical_key: String,
    pub title: String,
    pub objective: String,
    pub depends_on_logical_keys: Vec<String>,
    pub required_capabilities: Vec<Value>,
    pub acceptance_criteria: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRevisionRecord {
    pub task_id: String,
    pub revision: u64,
    pub task_spec_revision: u64,
    pub produced_by_agent_session_id: String,
    pub produced_by_attempt_id: Option<String>,
    pub steps: Vec<PlannedStepRecord>,
    pub reason_for_revision: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StepRecord {
    pub step_id: String,
    pub task_id: String,
    pub plan_revision: u64,
    pub logical_key: Option<String>,
    pub title: String,
    pub objective: String,
    pub dependencies: Vec<String>,
    pub required_capabilities: Vec<Value>,
    pub acceptance_criteria: Vec<Value>,
    pub status: String,
    pub current_attempt_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanAcceptance {
    pub plan_revision: PlanRevisionRecord,
    pub materialized_steps: Vec<StepRecord>,
    pub task_version: u64,
}

/// One atomic initial-plan admission. `request_payload` is the normalized submission,
/// including authenticated producer identity and typed plan input; generated Step IDs are
/// intentionally excluded from its idempotency digest.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanAcceptanceCommit {
    pub principal_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub workspace_id: String,
    pub task_id: String,
    pub expected_task_version: u64,
    pub expected_task_spec_revision: u64,
    pub plan_revision: PlanRevisionRecord,
    pub materialized_steps: Vec<StepRecord>,
    pub plan_event: EventDraft,
    pub step_events: Vec<EventDraft>,
}

pub trait TaskStore: Send + Sync {
    fn create_task(&self, commit: TaskCreateCommit) -> Result<CommittedTask, StoreError>;

    /// Resolve the original Task-creation receipt before mutable admission checks. The
    /// caller must still authenticate the principal and verify the Workspace scope.
    /// A matching payload returns the exact committed response; reusing the RequestId
    /// with another payload returns Conflict.
    fn get_task_create_receipt(
        &self,
        principal_id: &str,
        request_id: &str,
        request_payload: &Value,
    ) -> Result<Option<CommittedTask>, StoreError>;

    fn get_task_spec_revision_receipt(
        &self,
        principal_id: &str,
        request_id: &str,
        request_payload: &Value,
    ) -> Result<Option<CommittedTaskSpecRevision>, StoreError>;

    fn revise_task_spec(
        &self,
        commit: TaskSpecRevisionCommit,
    ) -> Result<CommittedTaskSpecRevision, StoreError>;

    fn accept_initial_plan(
        &self,
        commit: PlanAcceptanceCommit,
    ) -> Result<PlanAcceptance, StoreError>;

    fn list_plan_revisions(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Vec<PlanRevisionRecord>, StoreError>;

    fn list_steps(
        &self,
        workspace_id: &str,
        task_id: &str,
        plan_revision: Option<u64>,
    ) -> Result<Vec<StepRecord>, StoreError>;

    fn get_task(&self, workspace_id: &str, task_id: &str) -> Result<Option<TaskView>, StoreError>;

    fn list_task_spec_revisions(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Vec<TaskSpecRevisionRecord>, StoreError>;

    /// Lists a stable descending page ordered by `(created_at, task_id)`.
    fn list_tasks_page(
        &self,
        workspace_id: &str,
        status: Option<&str>,
        conversation_id: Option<&str>,
        after_created_at: Option<&str>,
        after_task_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TaskSummaryRecord>, StoreError>;
}

/// Durable provenance for one native or hosted agent interaction. Native resume
/// handles are intentionally excluded; they live in Runtime-local bindings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSessionRecord {
    pub agent_session_id: String,
    pub workspace_id: String,
    pub scope_kind: String,
    pub conversation_id: Option<String>,
    pub conversation_turn_id: Option<String>,
    pub task_id: Option<String>,
    pub task_spec_revision: Option<u64>,
    pub attempt_id: Option<String>,
    pub agent_binding_id: String,
    pub endpoint_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub configuration_digest: Option<String>,
    pub harness_descriptor_digest: Option<String>,
    pub status: String,
    pub started_at: String,
    pub last_event_at: Option<String>,
    pub closed_at: Option<String>,
    pub version: u64,
}

/// Runtime-local process/provider host state. It is operational inventory, not a
/// replicated domain aggregate; native process identity stays in the local store.
#[derive(Clone, Eq, PartialEq)]
pub struct AgentHostInstanceRecord {
    pub host_instance_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub agent_profile_id: String,
    pub endpoint_id: String,
    pub hosting_mode: String,
    pub state: String,
    pub process_identity_ref: Option<String>,
    pub ownership: String,
    pub started_at: String,
    pub last_used_at: String,
    pub idle_since: Option<String>,
}

/// Compare-and-set state transition for one Runtime-owned host. This interface
/// carries no provider handles and never creates domain Events.
pub trait AgentHostStore: Send + Sync {
    fn create_agent_host_instance(&self, host: AgentHostInstanceRecord) -> Result<(), StoreError>;

    fn transition_agent_host_instance(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        host_instance_id: &str,
        expected_state: &str,
        next_state: &str,
        occurred_at: &str,
        process_identity_ref: Option<&str>,
    ) -> Result<AgentHostInstanceRecord, StoreError>;

    fn get_agent_host_instance(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        host_instance_id: &str,
    ) -> Result<Option<AgentHostInstanceRecord>, StoreError>;

    fn list_agent_host_instances(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
    ) -> Result<Vec<AgentHostInstanceRecord>, StoreError>;
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskPlanningSessionStart {
    pub principal_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub expected_task_version: u64,
    pub session: AgentSessionRecord,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedAgentSession {
    pub session: AgentSessionRecord,
    pub event: DomainEvent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MarkStartingAgentSessionLost {
    pub workspace_id: String,
    pub agent_session_id: String,
    pub expected_version: u64,
    pub occurred_at: String,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActivateTaskPlanningSession {
    pub workspace_id: String,
    pub agent_session_id: String,
    pub expected_session_version: u64,
    pub expected_task_version: u64,
    pub occurred_at: String,
    pub host_instance_id: String,
    /// Runtime-local native handle. Never serialized into an event or response.
    pub native_session_ref: Option<String>,
    pub session_event: EventDraft,
    /// Required exactly when this activation changes Task READY -> RUNNING.
    pub task_status_event: Option<EventDraft>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedPlanningActivation {
    pub session: AgentSessionRecord,
    pub session_event: DomainEvent,
    pub task: TaskView,
    pub task_status_event: Option<DomainEvent>,
}

/// Storage boundary for claiming durable agent-session identities. Starting a session
/// claims the unique Task planner slot; it does not mean the native harness is ready.
/// The current desktop SQLite implementation fails both Task-planning admission methods
/// closed until Runtime-owned isolation evidence can be revalidated at this boundary.
pub trait AgentSessionStore: Send + Sync {
    /// Claims a planner slot only after the implementation's current admission gates pass.
    /// SQLite currently returns `TASK_PLANNING_ISOLATION_UNAVAILABLE` before persistence.
    fn start_task_planning_session(
        &self,
        start: TaskPlanningSessionStart,
    ) -> Result<CommittedAgentSession, StoreError>;

    fn get_agent_session(
        &self,
        workspace_id: &str,
        agent_session_id: &str,
    ) -> Result<Option<AgentSessionRecord>, StoreError>;

    /// Settles only a stranded STARTING planner as LOST. It never changes Task status.
    fn mark_starting_task_planning_session_lost(
        &self,
        transition: MarkStartingAgentSessionLost,
    ) -> Result<CommittedAgentSession, StoreError>;

    /// Commits adapter readiness, the Runtime-local host binding, and (for the first
    /// planner) Task READY -> RUNNING as one transaction. SQLite currently rejects this
    /// operation before lookup or mutation because no isolated planning Environment is
    /// admitted yet.
    fn activate_task_planning_session(
        &self,
        activation: ActivateTaskPlanningSession,
    ) -> Result<CommittedPlanningActivation, StoreError>;

    /// Returns only durable STARTING planning sessions, used after daemon recovery to
    /// reconcile sessions whose native startup may have been interrupted.
    fn list_starting_task_planning_sessions(
        &self,
        workspace_id: &str,
        limit: usize,
    ) -> Result<Vec<AgentSessionRecord>, StoreError>;
}

/// A Workspace creation request whose deduplication receipt is committed in the same
/// transaction as the aggregate projection and event. `request_payload` is canonicalized
/// by the storage adapter before its digest is recorded.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceCreateRequest {
    pub principal_id: String,
    pub request_id: String,
    pub request_payload: Value,
}

/// Canonical local Resource projection. Paths and provider handles are deliberately
/// excluded; callers use a ResourceLocation binding to resolve local content.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRecord {
    pub resource_id: String,
    pub workspace_id: String,
    pub kind: String,
    pub provider_identity: Value,
    pub identity_digest: Option<String>,
    pub display_name: String,
    pub current_revision_id: Option<String>,
    pub sensitivity: String,
    pub provenance: Value,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

/// Owner-scoped Resource detail needed by ContextDocument and revision surfaces. The
/// ContextDocument metadata is safe classification/owner/status data, never content or a
/// private location binding.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ResourceDetailRecord {
    #[serde(flatten)]
    pub resource: ResourceRecord,
    pub context_document: Option<Value>,
}

/// The only ContextDocument status writes currently implemented by the local Resource
/// service. Purge states remain owned by the deferred purge reconciler contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContextDocumentOwnerStatus {
    Active,
    Revoked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextDocumentStatusEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

/// Owner command sent through the domain boundary. The storage implementation checks
/// owner, Workspace state, Resource version and current ContextDocument status in one
/// transaction; it also commits the event, snapshot and idempotency receipt atomically.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextDocumentStatusCommand {
    pub workspace_id: String,
    pub resource_id: String,
    pub principal_id: String,
    pub request_id: String,
    pub expected_version: u64,
    pub target_status: ContextDocumentOwnerStatus,
    pub request_payload: Value,
    pub event: ContextDocumentStatusEventContext,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedContextDocumentStatus {
    pub resource: ResourceDetailRecord,
    pub event: DomainEvent,
}

pub trait ContextDocumentStatusStore: Send + Sync {
    fn set_context_document_status(
        &self,
        command: ContextDocumentStatusCommand,
    ) -> Result<CommittedContextDocumentStatus, StoreError>;
}

/// One immutable revision and its derived head marker, returned in ancestry order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRevisionViewRecord {
    pub revision: ResourceRevisionRecord,
    pub is_head: bool,
}

/// Public projection for a provider-backed Resource location. The private locator is
/// deliberately a different, non-serializable type so it cannot leak through ordinary
/// event/state serialization by accident.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceLocationRecord {
    pub location_id: String,
    pub resource_id: String,
    pub runtime_id: String,
    pub locator_ref_id: String,
    pub provider_ref: String,
    pub availability: String,
    pub writable: bool,
    pub observed_revision_id: Option<String>,
    pub observed_digest: Option<String>,
    pub observed_at: String,
}

/// Runtime-local provider handle. This type intentionally has no Serde implementation;
/// callers must not place its private locator in public API responses, event payloads, or
/// aggregate state.
#[derive(Clone, Eq, PartialEq)]
pub struct LocalResourceLocationBindingRecord {
    pub location_id: String,
    pub locator_ref_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub private_locator: String,
    pub observed_at: String,
}

/// Runtime-local raw operating-system identity for a Resource location. This value is
/// stored only in `file_identity_bindings`; it must never be serialized into aggregate
/// state, events, API responses, logs, or backups.
pub struct LocalFileIdentityBindingRecord {
    pub location_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub raw_filesystem_instance_id: String,
    pub raw_volume_id: Option<String>,
    pub raw_file_id: String,
    pub raw_generation: Option<String>,
    pub platform_kind: String,
    pub observed_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRootRecord {
    pub workspace_root_id: String,
    pub workspace_id: String,
    pub resource_id: String,
    pub location_id: String,
    pub display_name: String,
    pub watch_policy: String,
    pub replication_policy: String,
    pub status: String,
    pub added_by: Value,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

/// Read projection for the WorkspaceRoot list. `location_availability` is the last
/// committed ResourceLocation availability observed by the same bounded list query; it
/// is not a live Runtime probe. Keeping it on the projection lets the Operator explain a
/// paused or unavailable root without exposing its private locator or identity binding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkspaceRootListRecord {
    #[serde(flatten)]
    pub root: WorkspaceRootRecord,
    pub location_availability: String,
}

/// All identities needed to materialize a user-selected local folder as one persistent
/// root. `private_binding` is stored only in the incarnation-scoped local binding table;
/// it is excluded from request digests, events, and the returned value.
pub struct WorkspaceRootCreateCommit {
    pub request: WorkspaceCreateRequest,
    pub resource: ResourceRecord,
    pub location: ResourceLocationRecord,
    pub private_binding: LocalResourceLocationBindingRecord,
    pub file_identity_binding: LocalFileIdentityBindingRecord,
    pub root: WorkspaceRootRecord,
    pub resource_created_event: EventDraft,
    pub location_observed_event: EventDraft,
    pub root_created_event: EventDraft,
}

pub struct WorkspaceRootStatusCommit {
    pub request: WorkspaceCreateRequest,
    pub expected_version: u64,
    pub action: WorkspaceRootStatusAction,
    /// Required only for `Resume`, which is accepted only after this Runtime
    /// incarnation has an atomically revalidated local identity binding.
    pub runtime_id: Option<String>,
    pub runtime_incarnation_id: Option<String>,
    pub root: WorkspaceRootRecord,
    pub event: EventDraft,
}

/// One owner-authorized Resume that commits the freshly checked local identity and the
/// PAUSED -> ACTIVE transition together. The previous bindings are compare inputs only;
/// the new bindings are installed for this Runtime incarnation.
pub struct WorkspaceRootResumeCommit {
    pub request: WorkspaceCreateRequest,
    pub expected_version: u64,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub previous_locator_binding: LocalResourceLocationBindingRecord,
    pub previous_file_identity_binding: LocalFileIdentityBindingRecord,
    pub resource: ResourceRecord,
    pub location: ResourceLocationRecord,
    pub locator_binding: LocalResourceLocationBindingRecord,
    pub file_identity_binding: LocalFileIdentityBindingRecord,
    pub root: WorkspaceRootRecord,
    pub root_event: EventDraft,
    pub location_event: EventDraft,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceRootStatusAction {
    Pause,
    Resume,
    Revoke,
}

impl WorkspaceRootStatusAction {
    pub fn target_status(self) -> &'static str {
        match self {
            Self::Pause => "PAUSED",
            Self::Resume => "ACTIVE",
            Self::Revoke => "REVOKED",
        }
    }

    pub fn operation(self) -> &'static str {
        match self {
            Self::Pause => "workspace.root.pause.v1",
            // v2 requires live filesystem identity proof and one atomic Resume commit.
            Self::Resume => "workspace.root.resume.v2",
            Self::Revoke => "workspace.root.revoke.v1",
        }
    }

    pub fn reason_code(self) -> &'static str {
        match self {
            Self::Pause => "USER_PAUSED",
            Self::Resume => "USER_RESUMED",
            Self::Revoke => "USER_REVOKED",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedWorkspaceRoot {
    pub resource: ResourceRecord,
    pub location: ResourceLocationRecord,
    pub root: WorkspaceRootRecord,
    pub events: Vec<DomainEvent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedWorkspaceRootStatus {
    pub root: WorkspaceRootRecord,
    /// Point-in-time ResourceLocation availability returned with the status receipt.
    /// `None` supports receipts written before this projection field existed.
    #[serde(default)]
    pub location_availability: Option<String>,
    pub event: DomainEvent,
}

/// Safe explanation for a root whose prior local identity cannot be trusted. This enum
/// contains no locator or operating-system identity material.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum WorkspaceRootRevalidationFailure {
    NoPriorBinding,
    LocatorBindingMissing,
    FileIdentityBindingMissing,
    BindingMismatch,
    UnsupportedPlatform,
    InvalidLocator,
    IdentityChanged,
    IdentityUnavailable,
}

/// Runtime-local inputs returned for one non-revoked root. Private bindings are
/// intentionally carried in a non-Debug, non-Serde envelope and are trusted only after
/// the caller reopens the locator and verifies both identity projections.
pub enum WorkspaceRootRevalidationBindings {
    Previous {
        locator: LocalResourceLocationBindingRecord,
        file_identity: LocalFileIdentityBindingRecord,
    },
    Unavailable(WorkspaceRootRevalidationFailure),
    CurrentIncarnation {
        locator: LocalResourceLocationBindingRecord,
        file_identity: LocalFileIdentityBindingRecord,
    },
}

pub struct WorkspaceRootRevalidationCandidate {
    pub root: WorkspaceRootRecord,
    pub resource: ResourceRecord,
    pub location: ResourceLocationRecord,
    pub bindings: WorkspaceRootRevalidationBindings,
}

/// One atomic startup decision for a root. The caller includes fresh private bindings
/// only after exact no-follow identity verification; failure commits contain none.
pub struct WorkspaceRootRevalidationCommit {
    pub request: WorkspaceCreateRequest,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub expected_root_version: u64,
    pub root: WorkspaceRootRecord,
    pub resource: ResourceRecord,
    pub location: ResourceLocationRecord,
    pub locator_binding: Option<LocalResourceLocationBindingRecord>,
    pub file_identity_binding: Option<LocalFileIdentityBindingRecord>,
    pub root_event: Option<EventDraft>,
    pub location_event: Option<EventDraft>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedWorkspaceRootRevalidation {
    pub root: WorkspaceRootRecord,
    pub location: ResourceLocationRecord,
    pub events: Vec<DomainEvent>,
}

/// Atomic first registration of a local folder Resource, its current-incarnation
/// locator, and the WorkspaceRoot grant that authorizes future observation beneath it.
pub trait WorkspaceRootStore: Send + Sync {
    fn create_workspace_root(
        &self,
        commit: WorkspaceRootCreateCommit,
    ) -> Result<CommittedWorkspaceRoot, StoreError>;

    fn list_workspace_roots(
        &self,
        workspace_id: &str,
        status: Option<&str>,
        after_created_at: Option<&str>,
        after_workspace_root_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkspaceRootListRecord>, StoreError>;

    fn get_workspace_root(
        &self,
        workspace_id: &str,
        workspace_root_id: &str,
    ) -> Result<Option<WorkspaceRootRecord>, StoreError>;

    fn update_workspace_root_status(
        &self,
        commit: WorkspaceRootStatusCommit,
    ) -> Result<CommittedWorkspaceRootStatus, StoreError>;

    /// Atomically commits a freshly revalidated Resume, both current-incarnation local
    /// bindings, ResourceLocation availability, events, aggregate state, and the owner
    /// idempotency receipt.
    fn resume_workspace_root(
        &self,
        commit: WorkspaceRootResumeCommit,
    ) -> Result<CommittedWorkspaceRootStatus, StoreError>;

    /// Replays a prior owner status action only when the same principal, request ID,
    /// and canonical request payload match its committed receipt.
    fn get_workspace_root_status_receipt(
        &self,
        request: &WorkspaceCreateRequest,
    ) -> Result<Option<CommittedWorkspaceRootStatus>, StoreError>;

    /// Enumerates non-revoked roots on this Runtime in stable bounded order. Private
    /// locator and raw identity values remain inside the non-serializable candidate.
    fn list_workspace_root_revalidation_candidates(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        after_created_at: Option<&str>,
        after_workspace_root_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkspaceRootRevalidationCandidate>, StoreError>;

    /// Atomically installs current-incarnation bindings after caller verification, or
    /// marks the root and location unavailable after a failed verification.
    fn commit_workspace_root_revalidation(
        &self,
        commit: WorkspaceRootRevalidationCommit,
    ) -> Result<CommittedWorkspaceRootRevalidation, StoreError>;
}

/// Provenance for a file included through the browser's one-time folder picker.
/// This relative display path does not identify or authorize a filesystem location.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FolderImportMetadata {
    pub relative_path: String,
}

impl FolderImportMetadata {
    /// Validates the one-time folder-attachment path carried as provenance.
    ///
    /// This value is display/provenance metadata only. It must never be opened,
    /// joined to a filesystem path, or interpreted as a WorkspaceRoot grant.
    pub fn validate_for_display_name(&self, display_name: &str) -> Result<(), StoreError> {
        let value = self.relative_path.as_str();
        let segments: Vec<&str> = value.split('/').collect();
        let drive_qualified = segments.first().is_some_and(|segment| {
            let bytes = segment.as_bytes();
            bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
        });
        let valid = !value.is_empty()
            && value.chars().count() <= 240
            && !value.starts_with('/')
            && !value.ends_with('/')
            && !value.contains('\\')
            && !value.chars().any(char::is_control)
            && !drive_qualified
            && (1..=128).contains(&segments.len())
            && segments.iter().all(|segment| {
                !segment.is_empty() && *segment != "." && *segment != ".." && segment.len() <= 255
            })
            && value == display_name;
        if !valid {
            return Err(StoreError::Invalid(
                "folder import path must be a normalized relative path matching the Resource display name".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Canonical digest syntax used for upload identity and content verification.
pub fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRevisionRecord {
    pub resource_revision_id: String,
    pub resource_id: String,
    pub parent_revision_ids: Vec<String>,
    pub provider_revision: Option<String>,
    pub content_digest: Option<String>,
    pub size_bytes: Option<u64>,
    pub media_type: Option<String>,
    pub observed_at: String,
    pub created_by: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommittedResource {
    pub resource: ResourceRecord,
    pub revision: ResourceRevisionRecord,
    pub event: DomainEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResourceSummary {
    pub resource_id: String,
    pub workspace_id: String,
    pub resource_revision_id: String,
    pub display_name: String,
    pub media_type: String,
    pub content_digest: String,
    pub size_bytes: u64,
    pub created_at: String,
}

/// A Resource revision selected as an immutable input. The closed serde shape matches
/// the Operator `PinnedResourceRef` schema so storage and preparation boundaries reject
/// caller-supplied paths or other authority-bearing extensions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedResourceRef {
    pub workspace_id: String,
    pub resource_id: String,
    pub revision_id: String,
}

/// Search result for an imported, managed Resource. Search metadata is derived from
/// the current Resource revision and its local encrypted-blob location; it contains no
/// extracted text or provider locator.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceSearchRecord {
    pub summary: ResourceSummary,
    pub kind: String,
    pub location_id: String,
    pub availability: String,
    pub writable: bool,
    pub observed_revision_id: Option<String>,
    pub observed_digest: Option<String>,
    pub observed_at: String,
    pub last_checked_at: Option<String>,
    pub freshness: String,
    pub match_reasons: Vec<String>,
}

/// Rebuildable, revision-pinned encrypted text snapshot prepared before the SQLite
/// writer transaction. `term_tokens` contains only workspace-keyed MACs, never words.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedResourceTextIndex {
    pub workspace_id: String,
    pub resource_id: String,
    pub resource_revision_id: String,
    pub source_content_digest: String,
    pub extracted_text: BlobRef,
    pub parser_id: String,
    pub token_key_version: u32,
    pub term_tokens: Vec<String>,
    pub indexed_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceTextIndexSkipReason {
    UnsupportedType,
    OverSizeLimit,
    InvalidUtf8,
    ControlCharacters,
    TermLimitExceeded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceTextIndexRebuildOutcome {
    Indexed,
    NotIndexable,
}

/// Owner-triggered rebuild of the current managed Resource's derived lexical index.
/// The immutable source identity is explicit so a stale Library row cannot rebuild a
/// newer Resource head by accident. `request_id` is durably replayed with the outcome.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResourceTextIndexRebuildRequest {
    pub principal_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub resource_id: String,
    pub resource_revision_id: String,
    pub content_digest: String,
    pub indexed_at: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResourceTextIndexRebuildResult {
    pub request_id: String,
    pub correlation_id: String,
    pub workspace_id: String,
    pub resource_id: String,
    pub resource_revision_id: String,
    pub content_digest: String,
    pub outcome: ResourceTextIndexRebuildOutcome,
    pub reason: Option<ResourceTextIndexSkipReason>,
}

/// A deterministic lexical match with a snippet recovered from its encrypted,
/// digest-verified revision snapshot. The ResourceRef must pin `resource_revision_id`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceTextSearchRecord {
    pub result: ResourceSearchRecord,
    pub resource_revision_id: String,
    pub source_content_digest: String,
    pub snippet: String,
    /// Exact zero-based, half-open UTF-8 byte spans for the first occurrence of each
    /// distinct query term in the verified source text. These are transient retrieval
    /// provenance: callers must keep them paired with `resource_revision_id` and
    /// `source_content_digest`, and must never resolve them against a newer head.
    pub matched_spans: Vec<ResourceTextMatchSpan>,
    pub matched_term_count: u32,
    pub parser_id: String,
}

/// A source-grounded lexical match into the exact immutable UTF-8 Resource revision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResourceTextMatchSpan {
    pub term: String,
    pub start_utf8_byte: u64,
    pub end_utf8_byte_exclusive: u64,
}

/// Content resolved from one immutable local Resource revision. The provider locator
/// remains inside the storage adapter; callers receive only verified bytes and metadata.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoredResourceContent {
    pub summary: ResourceSummary,
    pub content: Vec<u8>,
}

/// Upload lifecycle values are persisted in the same uppercase form used by the
/// Operator API and SQLite contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceUploadState {
    Open,
    ContentReceived,
    Committed,
    Failed,
    Expired,
}

/// One accepted, non-empty chunk range using the inclusive offsets exposed by HTTP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceUploadRange {
    pub start_offset: u64,
    pub end_offset_inclusive: u64,
    pub sha256: String,
}

/// Derived transfer progress. An empty upload has no ranges and a missing offset of
/// zero; this represents a valid zero-byte session without inventing a chunk.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceUploadProgress {
    pub received_ranges: Vec<ResourceUploadRange>,
    pub next_missing_offset: u64,
}

/// Persistent upload-session projection. `context_document` and `folder_import` are
/// optional metadata pinned by an initial Resource upload. Revision uploads instead pin
/// `resource_id`, `expected_resource_version`, and `parent_revision_ids`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceUploadSessionRecord {
    pub upload_id: String,
    pub workspace_id: String,
    pub display_name: String,
    pub media_type: String,
    pub expected_size_bytes: u64,
    /// Legacy schema-v1 sessions may have no digest. Such sessions are retained as
    /// historical records but cannot be resumed or committed under the current contract.
    pub expected_digest: Option<String>,
    pub context_document: Option<Value>,
    #[serde(default)]
    pub folder_import: Option<FolderImportMetadata>,
    pub resource_id: Option<String>,
    pub committed_resource_id: Option<String>,
    pub expected_resource_version: Option<u64>,
    pub parent_revision_ids: Vec<String>,
    pub chunk_size_bytes: u64,
    pub received_ranges: Vec<ResourceUploadRange>,
    pub next_missing_offset: u64,
    pub state: ResourceUploadState,
    pub expires_at: String,
    pub created_at: String,
    /// Revision of the durable lifecycle aggregate. Chunk acceptance does not advance it.
    pub version: u64,
    /// Advances for progress changes and fences concurrent chunk submissions.
    pub progress_version: u64,
}

impl ResourceUploadSessionRecord {
    /// Returns the nested progress value without changing the flat snake_case session
    /// shape used by the Operator API and storage projection.
    pub fn progress(&self) -> ResourceUploadProgress {
        ResourceUploadProgress {
            received_ranges: self.received_ranges.clone(),
            next_missing_offset: self.next_missing_offset,
        }
    }

    /// Validates immutable metadata required by a new desktop/local upload session.
    /// Storage adapters must call this again at their transactional write boundary.
    pub fn validate_initial_metadata(&self) -> Result<(), StoreError> {
        if self.upload_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.display_name.trim().is_empty()
            || self.display_name.chars().count() > 240
            || self.media_type.trim().is_empty()
            || self.media_type.len() > 160
            || self.expected_size_bytes > 104_857_600
            || self.chunk_size_bytes != 4_194_304
            || !self
                .expected_digest
                .as_deref()
                .is_some_and(is_sha256_digest)
            || self.state != ResourceUploadState::Open
            || self.resource_id.is_some()
            || self.committed_resource_id.is_some()
            || self.expected_resource_version.is_some()
            || !self.parent_revision_ids.is_empty()
            || self.version != 1
            || self.progress_version != 1
        {
            return Err(StoreError::Invalid(
                "Resource upload initial metadata is invalid".to_owned(),
            ));
        }
        if let Some(folder_import) = &self.folder_import {
            folder_import.validate_for_display_name(&self.display_name)?;
        }
        Ok(())
    }
}

/// A validated HTTP Content-Range. The end is inclusive, including at the API/storage
/// boundary; adapters convert to a half-open range only when writing chunk records.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceUploadContentRange {
    pub start_offset: u64,
    pub end_offset_inclusive: u64,
    pub total_size_bytes: u64,
}

/// Bytes and declared identity for one chunk. `sha256` uses the canonical
/// `sha256:<lowercase hex>` storage form; the HTTP adapter normalizes the raw digest
/// header before constructing this value. `content` is transient input to the local
/// storage adapter and must be persisted through the encrypted temporary-blob path.
#[derive(Clone, PartialEq)]
pub struct ResourceUploadChunkInput {
    pub upload_id: String,
    pub chunk_index: u64,
    pub request_id: String,
    pub content_range: ResourceUploadContentRange,
    pub sha256: String,
    pub content: Vec<u8>,
    pub received_at: String,
    /// Event identity/context for the OPEN -> CONTENT_RECEIVED transition. Storage
    /// supplies the final payload and aggregate revision only when this chunk completes it.
    pub lifecycle_event: EventDraft,
}

/// Result of atomically finalizing an upload into a Resource and its initial revision.
/// `session` is the committed session projection; the Event and Resource records must
/// become visible in the same transaction as its COMMITTED state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedResourceUpload {
    pub session: ResourceUploadSessionRecord,
    pub resource: ResourceRecord,
    pub revision: ResourceRevisionRecord,
    pub event: DomainEvent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceInstructionRevisionRecord {
    pub workspace_id: String,
    pub revision: u64,
    pub parent_revisions: Vec<u64>,
    pub content_ref: Value,
    pub content_digest: String,
    pub authored_by: Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommittedWorkspaceInstructionRevision {
    pub workspace: Workspace,
    pub instruction_revision: WorkspaceInstructionRevisionRecord,
    pub event: DomainEvent,
}

pub trait ResourceStore: Send + Sync {
    fn create_resource(
        &self,
        request: WorkspaceCreateRequest,
        resource: ResourceRecord,
        revision: ResourceRevisionRecord,
        event: EventDraft,
        content: Vec<u8>,
    ) -> Result<CommittedResource, StoreError>;

    fn read_resource_content(
        &self,
        workspace_id: &str,
        resource_id: &str,
    ) -> Result<Option<StoredResourceContent>, StoreError>;

    /// Reads canonical Resource metadata without exposing private provider locators.
    fn get_resource_record(
        &self,
        workspace_id: &str,
        resource_id: &str,
    ) -> Result<Option<ResourceRecord>, StoreError>;

    /// Reads the canonical Resource plus non-content ContextDocument classification.
    fn get_resource_detail(
        &self,
        workspace_id: &str,
        resource_id: &str,
    ) -> Result<Option<ResourceDetailRecord>, StoreError>;

    /// Returns one bounded Resource revision page with derived head markers. Revisions
    /// are ordered by immutable append order, which is ancestry-safe because parent rows
    /// must already exist before a child can be inserted. The cursor is the last revision
    /// ID from the preceding page and is scoped to this Workspace and Resource.
    fn list_resource_revisions_page(
        &self,
        workspace_id: &str,
        resource_id: &str,
        after_revision_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ResourceRevisionViewRecord>, StoreError>;

    /// Reads the exact `revision_id` or current head when omitted, only after same-Resource
    /// metadata proves the content is within `maximum_bytes`; providers enforce this
    /// before allocating/decrypting content and verify the immutable digest and length.
    fn read_resource_content_bounded(
        &self,
        workspace_id: &str,
        resource_id: &str,
        revision_id: Option<&str>,
        maximum_bytes: u64,
    ) -> Result<Option<StoredResourceContent>, StoreError>;

    /// Search the current managed Resource catalog using bounded metadata fields only.
    /// `query` is a literal substring matched against the display name and media type;
    /// `kind` is an optional exact Resource kind and `freshness` an optional exact
    /// freshness value. Results are workspace-scoped and keyset-paginated in the same
    /// stable order as catalog listing.
    fn search_resources_page(
        &self,
        workspace_id: &str,
        query: Option<&str>,
        kind: Option<&str>,
        freshness: Option<&str>,
        after_created_at: Option<&str>,
        after_resource_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ResourceSearchRecord>, StoreError>;

    /// Rebuild the pinned current Resource's local lexical projection from verified
    /// authorized bytes using the active Workspace ResourceIndex key version. The
    /// operation durably replays its typed outcome by RequestId; a revision/digest CAS
    /// prevents stale Library state from targeting a newer head. It never mutates
    /// Resource or Task history.
    fn reindex_resource_text(
        &self,
        _request: ResourceTextIndexRebuildRequest,
    ) -> Result<ResourceTextIndexRebuildResult, StoreError> {
        Err(StoreError::Blob(
            "Resource text reindexing is unsupported by this provider".to_owned(),
        ))
    }

    /// Search revision-scoped encrypted local text indexes. Implementations must enforce
    /// Workspace/current-revision/ContextDocument scope, derive terms using the current
    /// or retained ResourceIndex key version, verify/decrypt the exact indexed snapshot,
    /// and return pinned ResourceRefs. No plaintext fallback is permitted.
    fn search_indexed_resource_text(
        &self,
        _workspace_id: &str,
        _query: &str,
        _kind: Option<&str>,
        _freshness: Option<&str>,
        _after_created_at: Option<&str>,
        _after_resource_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<ResourceTextSearchRecord>, StoreError> {
        Err(StoreError::Blob(
            "encrypted local Resource text search is unsupported by this provider".to_owned(),
        ))
    }
}

/// Durable resumable-transfer port. Chunk bytes are accepted only after the adapter
/// verifies the declared range/digest and stores them in encrypted temporary blobs.
/// `commit` is responsible for atomically materializing the Resource, revision, event,
/// and committed session state; upload chunks themselves are not domain events.
pub trait ResourceUploadStore: ResourceStore {
    fn create(
        &self,
        request: WorkspaceCreateRequest,
        session: ResourceUploadSessionRecord,
        event: EventDraft,
    ) -> Result<ResourceUploadSessionRecord, StoreError>;

    /// Creates a resumable upload pinned to the complete current Resource head set and
    /// expected Resource aggregate version. Implementations check both inside the same
    /// immediate transaction that creates the upload session/event/receipt.
    fn create_revision(
        &self,
        request: WorkspaceCreateRequest,
        session: ResourceUploadSessionRecord,
        event: EventDraft,
    ) -> Result<ResourceUploadSessionRecord, StoreError>;

    fn get(
        &self,
        workspace_id: &str,
        upload_id: &str,
    ) -> Result<Option<ResourceUploadSessionRecord>, StoreError>;

    /// Loads the exact immutable Resource commit receipt for a committed upload so a
    /// lost response can be replayed even after the Resource head has advanced again.
    fn committed_resource_for_upload(
        &self,
        principal_id: &str,
        upload_id: &str,
    ) -> Result<Option<CommittedResource>, StoreError>;

    /// Return a bounded page of sessions whose TTL elapsed and that still need a
    /// durable EXPIRED transition.
    fn list_expired(
        &self,
        now: &str,
        limit: usize,
    ) -> Result<Vec<ResourceUploadSessionRecord>, StoreError>;

    /// Delete a bounded batch of stale encrypted chunk blobs only after the adapter has
    /// fenced new references and confirmed that no chunk receipt points at each object.
    fn collect_orphan_chunks(&self, now: &str, limit: usize) -> Result<usize, StoreError>;

    /// Persist the TTL transition for an upload using optimistic version fencing.
    /// The supplied record is the proposed EXPIRED state; an already terminal or
    /// concurrently changed session is returned/ rejected by the adapter.
    fn expire(
        &self,
        expected_progress_version: u64,
        expired: ResourceUploadSessionRecord,
        event: EventDraft,
    ) -> Result<ResourceUploadSessionRecord, StoreError>;

    fn put_chunk(
        &self,
        chunk: ResourceUploadChunkInput,
    ) -> Result<ResourceUploadSessionRecord, StoreError>;

    fn commit(
        &self,
        request: WorkspaceCreateRequest,
        upload_id: &str,
        resource: ResourceRecord,
        revision: ResourceRevisionRecord,
        event: EventDraft,
        upload_status_event: Option<EventDraft>,
        upload_failure_event: Option<EventDraft>,
    ) -> Result<CommittedResourceUpload, StoreError>;
}

pub trait IdempotentWorkspaceStore: WorkspaceStore {
    fn create_workspace_idempotent(
        &self,
        request: WorkspaceCreateRequest,
        workspace: Workspace,
        event: EventDraft,
    ) -> Result<CommittedWorkspace, StoreError>;

    fn commit_workspace_idempotent(
        &self,
        request: WorkspaceCreateRequest,
        expected_version: u64,
        workspace: Workspace,
        event: EventDraft,
    ) -> Result<CommittedWorkspace, StoreError>;

    fn create_workspace_instruction_revision_idempotent(
        &self,
        request: WorkspaceCreateRequest,
        expected_version: u64,
        workspace: Workspace,
        instruction_revision: WorkspaceInstructionRevisionRecord,
        event: EventDraft,
    ) -> Result<CommittedWorkspaceInstructionRevision, StoreError>;
}

impl<T: StateStore + EventStore> WorkspaceStore for T {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum StoreError {
    Conflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    NotFound,
    Invalid(String),
    Busy,
    UnsupportedSchema(i64),
    CorruptSchema(String),
    Integrity(String),
    LegacyUploadCommitNeedsReview {
        committed_resource_id: Option<String>,
    },
    Blob(String),
    Io(String),
    Database(String),
    ExecutorStopped,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { expected, actual } => {
                write!(
                    f,
                    "stale aggregate version: expected {expected:?}, found {actual:?}"
                )
            }
            Self::NotFound => f.write_str("record not found"),
            Self::Invalid(message) => write!(f, "invalid storage command: {message}"),
            Self::Busy => f.write_str("storage is busy"),
            Self::UnsupportedSchema(version) => {
                write!(f, "database schema version {version} is unsupported")
            }
            Self::CorruptSchema(message) => write!(f, "database schema is inconsistent: {message}"),
            Self::Integrity(message) => write!(f, "stored data failed integrity checks: {message}"),
            Self::LegacyUploadCommitNeedsReview {
                committed_resource_id,
            } => write!(
                f,
                "legacy committed upload requires review (Resource: {committed_resource_id:?})"
            ),
            Self::Blob(message) => write!(f, "blob operation failed: {message}"),
            Self::Io(message) => write!(f, "storage I/O failed: {message}"),
            Self::Database(message) => write!(f, "database operation failed: {message}"),
            Self::ExecutorStopped => f.write_str("storage executor stopped"),
        }
    }
}

impl std::error::Error for StoreError {}
