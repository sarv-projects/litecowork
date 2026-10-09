mod artifacts;
mod conversations;
mod automation_admission;
mod blob;
mod coworkers;
mod delegation_profiles;
mod effect_evidence;
mod environments;
mod rich_presentations;
pub use rich_presentations::SqliteRichPresentationStore;
mod execution;
mod goals;
mod os_key_provider;
mod os_principal_binding;
mod resource_index;
mod routines;
mod runtime_identity;
mod suggestions;

pub use blob::{FileBlobStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
pub use conversations::SqliteConversationStore;
pub use coworkers::{
    AutomationPage, AutomationRevisionPage, CoworkerEventContext, CoworkerPage, SqliteCoworkerStore,
};
pub use delegation_profiles::{
    DelegationProfileEventContext, DelegationProfilePage, SqliteDelegationProfileStore,
};
pub use effect_evidence::SqliteEffectEvidenceStore;
pub use environments::SqliteEnvironmentStore;
pub use execution::SqliteStepAttemptStore;
pub use goals::{GoalEventContext, GoalPage, SqliteGoalStore};
pub use os_key_provider::OsWorkspaceBlobKeyProvider;
pub use os_principal_binding::{
    OsRuntimePrincipalBindingProvider, RuntimeOsPlatform, RuntimeOsPrincipalBinding,
    RuntimeOsPrincipalIdentity, UnixPrincipalId,
};
pub use routines::{RoutineEventContext, RoutinePage, RoutineRevisionPage, SqliteRoutineStore};
pub use runtime_identity::OsRuntimeDeviceIdentityProvider;
pub use suggestions::{SqliteSuggestionStore, SuggestionEventContext, SuggestionReadError};

use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior, params,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use storage_core::{
    ActivateTaskPlanningSession, AgentBindingCreateRequest, AgentBindingEnableRequest,
    AgentBindingRecord, AgentCatalogStore, AgentEndpointRecord, AgentEndpointViewRecord,
    AgentHostInstanceRecord, AgentHostStore, AgentProfileRecord, AgentProfileViewRecord,
    AgentSessionRecord, AgentSessionStore, AggregateStateRef, AutomationOccurrenceReadStore,
    AutomationOccurrenceRecord, AutomationTaskAdmission, BlobPurpose, BlobRef, BlobStore,
    CommittedAgentBinding, CommittedAgentSession, CommittedContextDocumentStatus,
    CommittedPlanningActivation, CommittedResource, CommittedResourceUpload,
    CommittedTaskSpecRevision, CommittedWorkspace, CommittedWorkspaceInstructionRevision,
    CommittedWorkspaceRoot, CommittedWorkspaceRootRevalidation, CommittedWorkspaceRootStatus,
    ContextDocumentOwnerStatus, ContextDocumentStatusCommand, ContextDocumentStatusStore,
    DeviceIdentityRecord, DomainEvent, EventDraft, EventStore, IdempotentWorkspaceStore,
    LocalAgentEndpointBindingRecord, LocalRuntimeWorkspaceBindingLookup,
    LocalRuntimeWorkspaceEnrollmentRequest, MarkStartingAgentSessionLost, PinnedResourceRef,
    PlanAcceptance, PlanAcceptanceCommit, PlanRevisionRecord, PreparedResourceTextIndex,
    ReplicationPolicy, ResourceLocationRecord, ResourceRecord, ResourceRevisionRecord,
    ResourceSearchRecord, ResourceStore, ResourceSummary, ResourceTextIndexRebuildOutcome,
    ResourceTextIndexRebuildRequest, ResourceTextIndexRebuildResult, ResourceTextIndexSkipReason,
    ResourceTextSearchRecord, ResourceUploadChunkInput, ResourceUploadSessionRecord,
    ResourceUploadState, ResourceUploadStore, RuntimeIncarnationLocalObservationRecord,
    RuntimeIncarnationRecord, RuntimeIncarnationStateUpdate, RuntimeLifecycleStore,
    RuntimeOfferRecord, RuntimeRecord, RuntimeWorkspaceBindingRecord, RuntimeWorkspaceBindingStore,
    StateStore, StepRecord, StoreError, StoredResourceContent, TaskAggregateSnapshot,
    TaskCreateCommit, TaskPlanningSessionStart, TaskRecord, TaskSpecRevisionCommit,
    TaskSpecRevisionRecord, TaskStore, TaskSummaryRecord, TaskView, Workspace,
    WorkspaceCreateRequest, WorkspaceInstructionRevisionRecord, WorkspaceRootCreateCommit,
    WorkspaceRootRecord, WorkspaceRootResumeCommit, WorkspaceRootRevalidationBindings,
    WorkspaceRootRevalidationCandidate, WorkspaceRootRevalidationCommit,
    WorkspaceRootRevalidationFailure, WorkspaceRootStatusAction, WorkspaceRootStatusCommit,
    WorkspaceRootStore,
};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const SQLITE_V1_DDL: &str = include_str!("../../../docs/schemas/sqlite-v1.sql");
const SQLITE_V2_DDL: &str = include_str!("../../../docs/schemas/sqlite-v2.sql");
const SQLITE_V3_DDL: &str = include_str!("../../../docs/schemas/sqlite-v3.sql");
const SQLITE_V4_DDL: &str = include_str!("../../../docs/schemas/sqlite-v4.sql");
const SQLITE_V5_DDL: &str = include_str!("../../../docs/schemas/sqlite-v5.sql");
const SQLITE_V6_DDL: &str = include_str!("../../../docs/schemas/sqlite-v6.sql");
const SQLITE_V7_DDL: &str = include_str!("../../../docs/schemas/sqlite-v7.sql");
const SQLITE_V8_DDL: &str = include_str!("../../../docs/schemas/sqlite-v8.sql");
const SQLITE_V9_DDL: &str = include_str!("../../../docs/schemas/sqlite-v9.sql");
const SQLITE_V10_DDL: &str = include_str!("../../../docs/schemas/sqlite-v10.sql");
const SQLITE_V11_DDL: &str = include_str!("../../../docs/schemas/sqlite-v11.sql");
const SQLITE_V12_DDL: &str = include_str!("../../../docs/schemas/sqlite-v12.sql");
const SQLITE_V13_DDL: &str = include_str!("../../../docs/schemas/sqlite-v13.sql");
const SCHEMA_VERSION: i64 = 13;
const V3_SCHEMA_VERSION: i64 = 3;
const V4_SCHEMA_VERSION: i64 = 4;
const V5_SCHEMA_VERSION: i64 = 5;
const V6_SCHEMA_VERSION: i64 = 6;
const V7_SCHEMA_VERSION: i64 = 7;
const V8_SCHEMA_VERSION: i64 = 8;
const V9_SCHEMA_VERSION: i64 = 9;
const V10_SCHEMA_VERSION: i64 = 10;
const V11_SCHEMA_VERSION: i64 = 11;
const V12_SCHEMA_VERSION: i64 = 12;

fn require_task_planning_isolation_admission() -> Result<(), StoreError> {
    // No qualified Runtime-owned admission proof producer is connected in this
    // implementation. Keep the concrete storage boundary closed until it is.
    Err(StoreError::Invalid(
        "TASK_PLANNING_ISOLATION_UNAVAILABLE".to_owned(),
    ))
}
const V1_MIGRATION_NAME: &str = "baseline_v1";
const V2_MIGRATION_NAME: &str = "resource_upload_lifecycle_v2";
const V3_MIGRATION_NAME: &str = "runtime_workspace_bindings_v3";
const V4_MIGRATION_NAME: &str = "installation_scoped_runtimes_v4";
const V5_MIGRATION_NAME: &str = "immutable_task_plan_records_v5";
const V6_MIGRATION_NAME: &str = "resource_location_unavailable_v6";
const V7_MIGRATION_NAME: &str = "encrypted_resource_text_index_v7";
const V8_MIGRATION_NAME: &str = "immutable_routine_revisions_v8";
const V9_MIGRATION_NAME: &str = "effect_evidence_guards_v9";
const V10_MIGRATION_NAME: &str = "goal_artifact_revision_links_v10";
const V11_MIGRATION_NAME: &str = "automation_occurrence_aggregate_revision_v11";
const V12_MIGRATION_NAME: &str = "rich_presentation_optional_v12";
const V13_MIGRATION_NAME: &str = "environment_identity_immutable_v13";
const MIGRATION_NAME: &str = V1_MIGRATION_NAME; // version-one compatibility for schema baseline assertions
const STATE_MEDIA_TYPE: &str = "application/vnd.litecowork.aggregate-state+json";
const TASK_STATE_MEDIA_TYPE: &str = "application/vnd.litecowork.task+json";

#[derive(Clone, Debug)]
pub struct SqliteConfig {
    pub writer_queue_capacity: usize,
    pub busy_timeout: Duration,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SqliteWriterMetricsSnapshot {
    /// Public storage commands submitted, including callers waiting for queue capacity.
    pub outstanding_commands: u64,
    /// Maximum simultaneous submitted commands observed since this adapter opened.
    pub outstanding_commands_peak: u64,
    /// Total elapsed time inside bounded-channel `send` calls, in nanoseconds.
    pub send_wait_nanos_total: u64,
    /// Maximum elapsed time inside one bounded-channel `send` call, in nanoseconds.
    pub send_wait_nanos_max: u64,
}

#[derive(Default)]
struct SqliteWriterMetrics {
    outstanding_commands: AtomicU64,
    outstanding_commands_peak: AtomicU64,
    send_wait_nanos_total: AtomicU64,
    send_wait_nanos_max: AtomicU64,
}

impl Default for SqliteConfig {
    fn default() -> Self {
        Self {
            writer_queue_capacity: 64,
            busy_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Clone)]
pub struct SqliteWorkspaceStore {
    inner: Arc<Inner>,
}

/// Local desktop storage composition using the OS credential store for blob keys.
/// The event/projection store and immutable blob store share one private state root.
pub struct LocalWorkspaceStorage {
    pub store: SqliteWorkspaceStore,
    pub blobs: Arc<dyn BlobStore>,
}

impl LocalWorkspaceStorage {
    pub fn open(
        state_directory: impl AsRef<Path>,
        config: SqliteConfig,
    ) -> Result<Self, StoreError> {
        let state_directory = state_directory.as_ref();
        let blob_root = state_directory.join("blobs");
        let blobs: Arc<dyn BlobStore> =
            Arc::new(FileBlobStore::new(blob_root, OsWorkspaceBlobKeyProvider));
        let store = SqliteWorkspaceStore::open(
            state_directory.join("litecowork.sqlite3"),
            Arc::clone(&blobs),
            config,
        )?;
        Ok(Self { store, blobs })
    }
}

struct Inner {
    sender: SyncSender<Command>,
    join: Mutex<Option<JoinHandle<()>>>,
    blobs: Arc<dyn BlobStore>,
    writer_metrics: SqliteWriterMetrics,
}

enum Command {
    ListResourceIndexKeyVersions {
        workspace_id: String,
        reply: mpsc::Sender<Result<Vec<u32>, StoreError>>,
    },
    SearchResourceIndexCandidates {
        workspace_id: String,
        tokens_by_version: Vec<(u32, Vec<String>)>,
        kind: Option<String>,
        freshness: Option<String>,
        after_created_at: Option<String>,
        after_resource_id: Option<String>,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<resource_index::IndexCandidate>, StoreError>>,
    },
    RecheckResourceIndexCandidate {
        workspace_id: String,
        resource_id: String,
        revision_id: String,
        source_digest: String,
        reply: mpsc::Sender<Result<bool, StoreError>>,
    },
    ReplaceResourceTextIndex {
        workspace_id: String,
        resource_id: String,
        revision_id: String,
        source_digest: String,
        index: Option<PreparedResourceTextIndex>,
        reply: mpsc::Sender<Result<bool, StoreError>>,
    },
    GetCurrentResourceSummary {
        workspace_id: String,
        resource_id: String,
        reply: mpsc::Sender<Result<Option<ResourceSummary>, StoreError>>,
    },
    CheckResourceTextIndexRebuildReceipt {
        principal_id: String,
        request_id: String,
        request_digest: String,
        reply: mpsc::Sender<Result<Option<ResourceTextIndexRebuildResult>, StoreError>>,
    },
    CommitResourceTextIndexRebuild {
        request: ResourceTextIndexRebuildRequest,
        request_digest: String,
        result: ResourceTextIndexRebuildResult,
        index: Option<PreparedResourceTextIndex>,
        reply: mpsc::Sender<Result<ResourceTextIndexRebuildResult, StoreError>>,
    },
    CoworkerOperation {
        operation: coworkers::WriterOperation,
    },
    DelegationProfileOperation {
        operation: delegation_profiles::WriterOperation,
    },
    GoalOperation {
        operation: goals::WriterOperation,
    },
    SuggestionOperation {
        operation: suggestions::WriterOperation,
    },
    RoutineOperation {
        operation: routines::WriterOperation,
    },
    ExecutionOperation {
        operation: execution::WriterOperation,
    },
    EffectEvidenceOperation {
        operation: effect_evidence::WriterOperation,
    },
    ArtifactOperation {
        operation: artifacts::WriterOperation,
    },
    RichPresentationOperation {
        operation: rich_presentations::WriterOperation,
    },
    ConversationOperation {
        operation: conversations::WriterOperation,
    },
    EnvironmentOperation {
        operation: environments::WriterOperation,
    },
    AuthorizeArtifactAppend {
        workspace_id: String,
        artifact_id: String,
        principal_id: String,
        expected_artifact_version: Option<u64>,
        expected_content_version: Option<u64>,
        reply: mpsc::Sender<Result<(), StoreError>>,
    },
    GetArtifactAppendHeads {
        workspace_id: String,
        artifact_id: String,
        reply: mpsc::Sender<Result<Option<storage_core::ArtifactAppendHeads>, StoreError>>,
    },
    ResolveArtifactAppendReplay {
        workspace_id: String,
        principal_id: String,
        request_id: String,
        request_digest: String,
        reply:
            mpsc::Sender<Result<Option<storage_core::CommittedArtifactVersionAppend>, StoreError>>,
    },
    GetArtifact {
        workspace_id: String,
        artifact_id: String,
        reply: mpsc::Sender<Result<Option<storage_core::ArtifactRecord>, StoreError>>,
    },
    ListArtifacts {
        workspace_id: String,
        library_status: Option<String>,
        task_id: Option<String>,
        after_created_at: Option<String>,
        after_artifact_id: Option<String>,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<storage_core::ArtifactRecord>, StoreError>>,
    },
    GetArtifactVersion {
        workspace_id: String,
        artifact_id: String,
        version: u64,
        reply: mpsc::Sender<Result<Option<storage_core::ArtifactVersionRecord>, StoreError>>,
    },
    GetTaskPresentation {
        workspace_id: String,
        task_id: String,
        reply: mpsc::Sender<Result<Option<storage_core::TaskPresentationReadModel>, StoreError>>,
    },
    EnrollLocalRuntimeInWorkspace {
        request: LocalRuntimeWorkspaceEnrollmentRequest,
        request_digest: String,
        binding: RuntimeWorkspaceBindingRecord,
        reply: mpsc::Sender<Result<RuntimeWorkspaceBindingRecord, StoreError>>,
    },
    GetCurrentLocalRuntimeWorkspaceBinding {
        lookup: LocalRuntimeWorkspaceBindingLookup,
        reply: mpsc::Sender<Result<Option<RuntimeWorkspaceBindingRecord>, StoreError>>,
    },
    PutAgentProfile {
        profile: AgentProfileRecord,
        endpoints: Vec<AgentEndpointRecord>,
        reply: mpsc::Sender<Result<(), StoreError>>,
    },
    RegisterLocalAgentEndpointBinding {
        binding: storage_core::LocalAgentEndpointBindingInput,
        reply: mpsc::Sender<Result<(), StoreError>>,
    },
    GetLocalAgentEndpointBinding {
        runtime_id: String,
        runtime_incarnation_id: String,
        endpoint_id: String,
        now: String,
        reply: mpsc::Sender<Result<Option<LocalAgentEndpointBindingRecord>, StoreError>>,
    },
    PublishRuntimeOffer {
        offer: RuntimeOfferRecord,
        reply: mpsc::Sender<Result<(), StoreError>>,
    },
    ListAgentProfiles {
        owner_principal_id: String,
        workspace_id: String,
        now: String,
        reply: mpsc::Sender<Result<Vec<AgentProfileViewRecord>, StoreError>>,
    },
    CreateAgentBinding {
        request: AgentBindingCreateRequest,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedAgentBinding, StoreError>>,
    },
    GetAgentBinding {
        owner_principal_id: String,
        workspace_id: String,
        agent_binding_id: String,
        reply: mpsc::Sender<Result<Option<AgentBindingRecord>, StoreError>>,
    },
    ListAgentBindings {
        owner_principal_id: String,
        workspace_id: String,
        reply: mpsc::Sender<Result<Vec<AgentBindingRecord>, StoreError>>,
    },
    EnableAgentBinding {
        request: AgentBindingEnableRequest,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedAgentBinding, StoreError>>,
    },
    GetAgentBindingReceipt {
        principal_id: String,
        request_id: String,
        request_digest: String,
        reply: mpsc::Sender<Result<Option<CommittedAgentBinding>, StoreError>>,
    },
    RegisterLocalRuntimeIncarnation {
        runtime: RuntimeRecord,
        incarnation: RuntimeIncarnationRecord,
        observation: RuntimeIncarnationLocalObservationRecord,
        reply: mpsc::Sender<Result<RuntimeIncarnationRecord, StoreError>>,
    },
    TransitionLocalRuntimeIncarnation {
        update: RuntimeIncarnationStateUpdate,
        reply: mpsc::Sender<Result<RuntimeIncarnationRecord, StoreError>>,
    },
    ListWorkspaces {
        reply: mpsc::Sender<Result<Vec<Workspace>, StoreError>>,
    },
    GetWorkspace {
        workspace_id: String,
        reply: mpsc::Sender<Result<Option<Workspace>, StoreError>>,
    },
    ListResourcesPage {
        workspace_id: String,
        after_created_at: Option<String>,
        after_resource_id: Option<String>,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<ResourceSummary>, StoreError>>,
    },
    GetResourceRecord {
        workspace_id: String,
        resource_id: String,
        reply: mpsc::Sender<Result<Option<ResourceRecord>, StoreError>>,
    },
    GetResourceDetail {
        workspace_id: String,
        resource_id: String,
        reply: mpsc::Sender<Result<Option<storage_core::ResourceDetailRecord>, StoreError>>,
    },
    ListResourceRevisionsPage {
        workspace_id: String,
        resource_id: String,
        after_revision_id: Option<String>,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<storage_core::ResourceRevisionViewRecord>, StoreError>>,
    },
    SearchResourcesPage {
        workspace_id: String,
        query: Option<String>,
        kind: Option<String>,
        freshness: Option<String>,
        after_created_at: Option<String>,
        after_resource_id: Option<String>,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<ResourceSearchRecord>, StoreError>>,
    },
    ReadResourceContent {
        workspace_id: String,
        resource_id: String,
        revision_id: Option<String>,
        maximum_bytes: Option<u64>,
        reply: mpsc::Sender<Result<Option<(ResourceSummary, BlobRef)>, StoreError>>,
    },
    GetResourceContextDocumentStatus {
        workspace_id: String,
        resource_id: String,
        reply: mpsc::Sender<Result<Option<String>, StoreError>>,
    },
    SetContextDocumentStatus {
        command: ContextDocumentStatusCommand,
        blobs: Arc<dyn BlobStore>,
        reply: mpsc::Sender<Result<CommittedContextDocumentStatus, StoreError>>,
    },
    ListWorkspaceInstructionRevisions {
        workspace_id: String,
        after_revision: u64,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<WorkspaceInstructionRevisionRecord>, StoreError>>,
    },
    CreateTask {
        commit: Box<TaskCreateCommit>,
        state_ref: AggregateStateRef,
        occurrence_state_refs: Option<[AggregateStateRef; 3]>,
        reply: mpsc::Sender<Result<storage_core::CommittedTask, StoreError>>,
    },
    GetTaskCreateReceipt {
        principal_id: String,
        request_id: String,
        request_digest: String,
        reply: mpsc::Sender<Result<Option<storage_core::CommittedTask>, StoreError>>,
    },
    GetTaskSpecRevisionReceipt {
        principal_id: String,
        request_id: String,
        request_payload: Value,
        reply: mpsc::Sender<Result<Option<CommittedTaskSpecRevision>, StoreError>>,
    },
    ReviseTaskSpec {
        commit: Box<TaskSpecRevisionCommit>,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedTaskSpecRevision, StoreError>>,
    },
    AcceptInitialPlan {
        commit: Box<PlanAcceptanceCommit>,
        task_state_ref: AggregateStateRef,
        step_state_refs: Vec<AggregateStateRef>,
        reply: mpsc::Sender<Result<PlanAcceptance, StoreError>>,
    },
    ListPlanRevisions {
        workspace_id: String,
        task_id: String,
        reply: mpsc::Sender<Result<Vec<PlanRevisionRecord>, StoreError>>,
    },
    ListSteps {
        workspace_id: String,
        task_id: String,
        plan_revision: Option<u64>,
        reply: mpsc::Sender<Result<Vec<StepRecord>, StoreError>>,
    },
    StartTaskPlanningSession {
        start: TaskPlanningSessionStart,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedAgentSession, StoreError>>,
    },
    GetAgentSession {
        workspace_id: String,
        agent_session_id: String,
        reply: mpsc::Sender<Result<Option<AgentSessionRecord>, StoreError>>,
    },
    MarkStartingAgentSessionLost {
        transition: MarkStartingAgentSessionLost,
        next: AgentSessionRecord,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedAgentSession, StoreError>>,
    },
    ActivateTaskPlanningSession {
        activation: ActivateTaskPlanningSession,
        session: AgentSessionRecord,
        task: TaskView,
        session_state_ref: AggregateStateRef,
        task_state_ref: Option<AggregateStateRef>,
        reply: mpsc::Sender<Result<CommittedPlanningActivation, StoreError>>,
    },
    CreateAgentHostInstance {
        host: AgentHostInstanceRecord,
        reply: mpsc::Sender<Result<(), StoreError>>,
    },
    TransitionAgentHostInstance {
        runtime_id: String,
        runtime_incarnation_id: String,
        host_instance_id: String,
        expected_state: String,
        next_state: String,
        occurred_at: String,
        process_identity_ref: Option<String>,
        reply: mpsc::Sender<Result<AgentHostInstanceRecord, StoreError>>,
    },
    GetAgentHostInstance {
        runtime_id: String,
        runtime_incarnation_id: String,
        host_instance_id: String,
        reply: mpsc::Sender<Result<Option<AgentHostInstanceRecord>, StoreError>>,
    },
    ListAgentHostInstances {
        runtime_id: String,
        runtime_incarnation_id: String,
        reply: mpsc::Sender<Result<Vec<AgentHostInstanceRecord>, StoreError>>,
    },
    ListStartingTaskPlanningSessions {
        workspace_id: String,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<AgentSessionRecord>, StoreError>>,
    },
    GetTask {
        workspace_id: String,
        task_id: String,
        reply: mpsc::Sender<Result<Option<TaskView>, StoreError>>,
    },
    ListTaskSpecRevisions {
        workspace_id: String,
        task_id: String,
        reply: mpsc::Sender<Result<Vec<TaskSpecRevisionRecord>, StoreError>>,
    },
    ListTasksPage {
        workspace_id: String,
        status: Option<String>,
        conversation_id: Option<String>,
        after_created_at: Option<String>,
        after_task_id: Option<String>,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<TaskSummaryRecord>, StoreError>>,
    },
    CommitWorkspace {
        commit: Box<WorkspaceCommit>,
        reply: mpsc::Sender<Result<CommittedWorkspace, StoreError>>,
    },
    ReadWorkspaceEvents {
        workspace_id: String,
        reply: mpsc::Sender<Result<Vec<DomainEvent>, StoreError>>,
    },
    SqliteVersion {
        reply: mpsc::Sender<Result<String, StoreError>>,
    },
    CreateResource {
        request: WorkspaceCreateRequest,
        resource: ResourceRecord,
        revision: ResourceRevisionRecord,
        draft: EventDraft,
        state_ref: AggregateStateRef,
        content_blob: storage_core::BlobRef,
        text_index: Option<PreparedResourceTextIndex>,
        location_id: String,
        reply: mpsc::Sender<Result<CommittedResource, StoreError>>,
    },
    CreateWorkspaceRoot {
        commit: WorkspaceRootCreateCommit,
        resource_state_ref: AggregateStateRef,
        root_state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedWorkspaceRoot, StoreError>>,
    },
    ListWorkspaceRoots {
        workspace_id: String,
        status: Option<String>,
        after_created_at: Option<String>,
        after_workspace_root_id: Option<String>,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<storage_core::WorkspaceRootListRecord>, StoreError>>,
    },
    GetWorkspaceRoot {
        workspace_id: String,
        workspace_root_id: String,
        reply: mpsc::Sender<Result<Option<WorkspaceRootRecord>, StoreError>>,
    },
    UpdateWorkspaceRootStatus {
        commit: WorkspaceRootStatusCommit,
        root_state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedWorkspaceRootStatus, StoreError>>,
    },
    ResumeWorkspaceRoot {
        commit: WorkspaceRootResumeCommit,
        root_state_ref: AggregateStateRef,
        resource_state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedWorkspaceRootStatus, StoreError>>,
    },
    GetWorkspaceRootStatusReceipt {
        request: WorkspaceCreateRequest,
        reply: mpsc::Sender<Result<Option<CommittedWorkspaceRootStatus>, StoreError>>,
    },
    ListWorkspaceRootRevalidationCandidates {
        runtime_id: String,
        runtime_incarnation_id: String,
        after_created_at: Option<String>,
        after_workspace_root_id: Option<String>,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<WorkspaceRootRevalidationCandidate>, StoreError>>,
    },
    CommitWorkspaceRootRevalidation {
        commit: WorkspaceRootRevalidationCommit,
        root_state_ref: Option<AggregateStateRef>,
        resource_state_ref: Option<AggregateStateRef>,
        reply: mpsc::Sender<Result<CommittedWorkspaceRootRevalidation, StoreError>>,
    },
    CreateResourceUpload {
        request: WorkspaceCreateRequest,
        session: ResourceUploadSessionRecord,
        draft: EventDraft,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<ResourceUploadSessionRecord, StoreError>>,
    },
    GetResourceUpload {
        workspace_id: String,
        upload_id: String,
        reply: mpsc::Sender<Result<Option<ResourceUploadSessionRecord>, StoreError>>,
    },
    GetCommittedResourceUpload {
        principal_id: String,
        upload_id: String,
        reply: mpsc::Sender<Result<Option<CommittedResource>, StoreError>>,
    },
    ListExpiredResourceUploads {
        now: String,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<ResourceUploadSessionRecord>, StoreError>>,
    },
    ExpireResourceUpload {
        expected_progress_version: u64,
        expired: ResourceUploadSessionRecord,
        draft: EventDraft,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<ResourceUploadSessionRecord, StoreError>>,
    },
    FailResourceUpload {
        expected_version: u64,
        expected_progress_version: u64,
        failed: ResourceUploadSessionRecord,
        draft: EventDraft,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<ResourceUploadSessionRecord, StoreError>>,
    },
    ReserveResourceUploadBlob {
        workspace_id: String,
        upload_id: String,
        request_id: String,
        chunk_index: u64,
        digest: String,
        size_bytes: u64,
        created_at: String,
        expires_at: String,
        reply: mpsc::Sender<Result<(), StoreError>>,
    },
    ClaimOrphanResourceUploadBlobs {
        now: String,
        limit: usize,
        reply: mpsc::Sender<Result<Vec<(String, BlobRef)>, StoreError>>,
    },
    FinishOrphanResourceUploadBlob {
        workspace_id: String,
        digest: String,
        reply: mpsc::Sender<Result<(), StoreError>>,
    },
    GetResourceUploadChunks {
        workspace_id: String,
        upload_id: String,
        reply: mpsc::Sender<Result<Vec<(u64, BlobRef, String, u64)>, StoreError>>,
    },
    PutResourceUploadChunk {
        chunk: ResourceUploadChunkInput,
        blob: BlobRef,
        expected_progress_version: u64,
        completion_event: Option<EventDraft>,
        completion_state_ref: Option<AggregateStateRef>,
        reply: mpsc::Sender<Result<ResourceUploadSessionRecord, StoreError>>,
    },
    CommitResourceUpload {
        request: WorkspaceCreateRequest,
        resource: ResourceRecord,
        revision: ResourceRevisionRecord,
        draft: EventDraft,
        state_ref: AggregateStateRef,
        upload_commit: Option<UploadCommit>,
        content_blob: BlobRef,
        text_index: Option<PreparedResourceTextIndex>,
        location_id: String,
        context_document: Option<Value>,
        reply: mpsc::Sender<Result<CommittedResource, StoreError>>,
    },
    CreateWorkspaceInstructionRevision {
        request: WorkspaceCreateRequest,
        expected_version: u64,
        workspace: Workspace,
        instruction_revision: WorkspaceInstructionRevisionRecord,
        draft: EventDraft,
        state_ref: AggregateStateRef,
        reply: mpsc::Sender<Result<CommittedWorkspaceInstructionRevision, StoreError>>,
    },
    Shutdown,
}

struct UploadCommit {
    expected_version: u64,
    expected_progress_version: u64,
    committed_session: ResourceUploadSessionRecord,
    status_event: EventDraft,
    state_ref: AggregateStateRef,
}

struct WorkspaceCommit {
    expected_version: Option<u64>,
    workspace: Workspace,
    draft: EventDraft,
    state_ref: AggregateStateRef,
    request: Option<RequestDeduplication>,
}

struct RequestDeduplication {
    principal_id: String,
    request_id: String,
    request_digest: String,
    created_at: String,
}

impl SqliteWorkspaceStore {
    pub fn open(
        database_path: impl AsRef<Path>,
        blobs: Arc<dyn BlobStore>,
        config: SqliteConfig,
    ) -> Result<Self, StoreError> {
        if config.writer_queue_capacity == 0 {
            return Err(StoreError::Invalid(
                "writer queue capacity must be greater than zero".to_owned(),
            ));
        }
        let database_path = database_path.as_ref().to_path_buf();
        let parent = database_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| {
                StoreError::Invalid(
                    "SQLite database must be inside an explicit private state directory".to_owned(),
                )
            })?;
        ensure_private_directory(parent)?;

        let (sender, receiver) = mpsc::sync_channel(config.writer_queue_capacity);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker_path = database_path.clone();
        let worker_config = config.clone();
        let join = thread::Builder::new()
            .name("litecowork-sqlite-writer".to_owned())
            .spawn(move || writer_loop(worker_path, worker_config, receiver, ready_sender))
            .map_err(|error| StoreError::Io(error.to_string()))?;

        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                inner: Arc::new(Inner {
                    sender,
                    join: Mutex::new(Some(join)),
                    blobs,
                    writer_metrics: SqliteWriterMetrics::default(),
                }),
            }),
            Ok(Err(error)) => {
                let _ = join.join();
                Err(error)
            }
            Err(_) => {
                let _ = join.join();
                Err(StoreError::ExecutorStopped)
            }
        }
    }

    pub fn rebuild_workspace_projection(
        &self,
        workspace_id: &str,
    ) -> Result<Option<Workspace>, StoreError> {
        let events = self.read_workspace_events(workspace_id)?;
        if events.is_empty() {
            return Ok(None);
        }

        let mut projection: Option<Workspace> = None;
        let mut previous_revision = 0_u64;
        for event in events {
            let state_ref = &event.aggregate_state_ref;
            if event.entity_revision != state_ref.entity_revision
                || event.entity_revision != previous_revision + 1
            {
                return Err(StoreError::Integrity(
                    "Workspace event revisions are not contiguous".to_owned(),
                ));
            }
            let bytes =
                self.inner
                    .blobs
                    .get(workspace_id, BlobPurpose::AggregateState, &state_ref.blob)?;
            let value: Workspace = serde_json::from_slice(&bytes)
                .map_err(|error| StoreError::Integrity(error.to_string()))?;
            let canonical = canonical_json(&value)?;
            if canonical != bytes
                || value.workspace_id != workspace_id
                || value.version != event.entity_revision
            {
                return Err(StoreError::Integrity(
                    "Workspace aggregate-state blob does not match its event".to_owned(),
                ));
            }
            let expected_payload_digest = digest(&canonical_json(&event.payload)?);
            if expected_payload_digest != event.payload_digest {
                return Err(StoreError::Integrity(
                    "event payload digest does not match canonical payload".to_owned(),
                ));
            }
            previous_revision = event.entity_revision;
            projection = Some(value);
        }
        Ok(projection)
    }

    pub fn sqlite_version(&self) -> Result<String, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::SqliteVersion {
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    /// Lists all stored Workspaces by `created_at` ascending, then ID ascending.
    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListWorkspaces {
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    /// Returns a stable descending page. The pair must come from a previous row so
    /// concurrent inserts before the cursor do not shift already-read results.
    pub fn list_resources_page(
        &self,
        workspace_id: &str,
        after_created_at: Option<&str>,
        after_resource_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ResourceSummary>, StoreError> {
        if workspace_id.trim().is_empty()
            || !(1..=101).contains(&limit)
            || after_created_at.is_some() != after_resource_id.is_some()
        {
            return Err(StoreError::Invalid(
                "Resource page query is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListResourcesPage {
                workspace_id: workspace_id.to_owned(),
                after_created_at: after_created_at.map(str::to_owned),
                after_resource_id: after_resource_id.map(str::to_owned),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    pub fn get_resource_record(
        &self,
        workspace_id: &str,
        resource_id: &str,
    ) -> Result<Option<ResourceRecord>, StoreError> {
        validate_nonempty(&[workspace_id, resource_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetResourceRecord {
                workspace_id: workspace_id.to_owned(),
                resource_id: resource_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    pub fn get_resource_detail(
        &self,
        workspace_id: &str,
        resource_id: &str,
    ) -> Result<Option<storage_core::ResourceDetailRecord>, StoreError> {
        validate_nonempty(&[workspace_id, resource_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetResourceDetail {
                workspace_id: workspace_id.to_owned(),
                resource_id: resource_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    pub fn list_resource_revisions_page(
        &self,
        workspace_id: &str,
        resource_id: &str,
        after_revision_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<storage_core::ResourceRevisionViewRecord>, StoreError> {
        validate_nonempty(&[workspace_id, resource_id])?;
        if !(1..=201).contains(&limit)
            || after_revision_id.is_some_and(|value| value.trim().is_empty())
        {
            return Err(StoreError::Invalid(
                "Resource revision page query is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListResourceRevisionsPage {
                workspace_id: workspace_id.to_owned(),
                resource_id: resource_id.to_owned(),
                after_revision_id: after_revision_id.map(str::to_owned),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    pub fn search_resources_page(
        &self,
        workspace_id: &str,
        query: Option<&str>,
        kind: Option<&str>,
        freshness: Option<&str>,
        after_created_at: Option<&str>,
        after_resource_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ResourceSearchRecord>, StoreError> {
        let query = query.filter(|value| !value.is_empty());
        let kind = kind.map(str::trim).filter(|value| !value.is_empty());
        if workspace_id.trim().is_empty()
            || query.is_some_and(|value| value.len() > 256 || value.contains('\0'))
            || kind.is_some_and(|value| value.len() > 64 || value.contains('\0'))
            || kind.is_some_and(|value| {
                !matches!(
                    value,
                    "FILE" | "FOLDER" | "ARTIFACT" | "CONNECTOR_OBJECT" | "WEB_RESOURCE" | "OTHER"
                )
            })
            || freshness.is_some_and(|value| {
                !matches!(value, "CURRENT" | "STALE" | "UNKNOWN" | "UNAVAILABLE")
            })
            || !(1..=201).contains(&limit)
            || after_created_at.is_some() != after_resource_id.is_some()
        {
            return Err(StoreError::Invalid(
                "Resource search query is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::SearchResourcesPage {
                workspace_id: workspace_id.to_owned(),
                query: query.map(str::to_owned),
                kind: kind.map(str::to_owned),
                freshness: freshness.map(str::to_owned),
                after_created_at: after_created_at.map(str::to_owned),
                after_resource_id: after_resource_id.map(str::to_owned),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    pub fn list_workspace_instruction_revisions(
        &self,
        workspace_id: &str,
        after_revision: u64,
        limit: usize,
    ) -> Result<Vec<WorkspaceInstructionRevisionRecord>, StoreError> {
        if workspace_id.trim().is_empty() || !(1..=201).contains(&limit) {
            return Err(StoreError::Invalid(
                "Workspace instruction query is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListWorkspaceInstructionRevisions {
                workspace_id: workspace_id.to_owned(),
                after_revision,
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn commit_workspace_with_request(
        &self,
        request: Option<WorkspaceCreateRequest>,
        expected_version: Option<u64>,
        workspace: Workspace,
        event: EventDraft,
    ) -> Result<CommittedWorkspace, StoreError> {
        if request.is_some() {
            // Idempotent transaction handling must be allowed to inspect an existing
            // receipt before enforcing the caller's old expected version. In
            // particular, a retry may arrive after this Workspace has advanced.
            validate_workspace_event_identity(&workspace, &event)?;
        } else {
            validate_commit(&workspace, &event, expected_version)?;
        }
        let request_dedup = request
            .map(|request| {
                if request.principal_id.trim().is_empty() || request.request_id.trim().is_empty() {
                    return Err(StoreError::Invalid(
                        "principal and request IDs must not be empty".to_owned(),
                    ));
                }
                let payload = canonical_json(&request.request_payload)?;
                Ok(RequestDeduplication {
                    principal_id: request.principal_id,
                    request_id: request.request_id,
                    request_digest: digest(&payload),
                    created_at: event.recorded_at.clone(),
                })
            })
            .transpose()?;
        let state_bytes = canonical_json(&workspace)?;
        let state_blob = self.inner.blobs.put(
            &workspace.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            STATE_MEDIA_TYPE,
        )?;
        if state_blob.size_bytes != state_bytes.len() as u64
            || self.inner.blobs.get(
                &workspace.workspace_id,
                BlobPurpose::AggregateState,
                &state_blob,
            )? != state_bytes
        {
            return Err(StoreError::Integrity(
                "BlobStore did not durably verify aggregate state".to_owned(),
            ));
        }
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: workspace.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CommitWorkspace {
                commit: Box::new(WorkspaceCommit {
                    expected_version,
                    workspace,
                    draft: event,
                    state_ref,
                    request: request_dedup,
                }),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    /// Reports process-local writer dispatch pressure; this is operational telemetry,
    /// not durable Workspace or Task state.
    pub fn writer_metrics_snapshot(&self) -> SqliteWriterMetricsSnapshot {
        SqliteWriterMetricsSnapshot {
            outstanding_commands: self
                .inner
                .writer_metrics
                .outstanding_commands
                .load(Ordering::Relaxed),
            outstanding_commands_peak: self
                .inner
                .writer_metrics
                .outstanding_commands_peak
                .load(Ordering::Relaxed),
            send_wait_nanos_total: self
                .inner
                .writer_metrics
                .send_wait_nanos_total
                .load(Ordering::Relaxed),
            send_wait_nanos_max: self
                .inner
                .writer_metrics
                .send_wait_nanos_max
                .load(Ordering::Relaxed),
        }
    }

    /// Refuse to create new tokens when any key version already referenced by this
    /// Workspace's index is unavailable. Otherwise an OS credential-store reset could
    /// silently provision a fresh key under the old version number and make old index
    /// rows ambiguous. Initial provisioning remains allowed only when no index rows
    /// exist for the Workspace.
    fn ensure_resource_index_key_history_available(
        &self,
        workspace_id: &str,
    ) -> Result<(), StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        let versions = self.execute_command(
            Command::ListResourceIndexKeyVersions {
                workspace_id: workspace_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )?;
        if versions.len() > resource_index::MAX_SEARCHABLE_KEY_VERSIONS {
            return Err(StoreError::Blob(
                "Resource index has too many retained key versions; reindex before writing"
                    .to_owned(),
            ));
        }
        for version in versions {
            self.inner.blobs.resource_index_token(
                workspace_id,
                Some(version),
                "__lc_key_availability_check_v1",
            )?;
        }
        Ok(())
    }

    fn execute_command<T>(
        &self,
        command: Command,
        reply_receiver: Receiver<Result<T, StoreError>>,
    ) -> Result<T, StoreError> {
        let metrics = &self.inner.writer_metrics;
        let outstanding = metrics
            .outstanding_commands
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        metrics
            .outstanding_commands_peak
            .fetch_max(outstanding, Ordering::Relaxed);

        let send_started = Instant::now();
        let sent = self.inner.sender.send(command);
        let send_wait = duration_as_nanos(send_started.elapsed());
        atomic_saturating_add(&metrics.send_wait_nanos_total, send_wait);
        metrics
            .send_wait_nanos_max
            .fetch_max(send_wait, Ordering::Relaxed);
        if sent.is_err() {
            metrics.outstanding_commands.fetch_sub(1, Ordering::Relaxed);
            return Err(StoreError::ExecutorStopped);
        }

        let response = reply_receiver.recv();
        metrics.outstanding_commands.fetch_sub(1, Ordering::Relaxed);
        response.map_err(|_| StoreError::ExecutorStopped)?
    }

    fn resource_read_error_after_blob_failure(
        &self,
        workspace_id: &str,
        resource_id: &str,
        blob_error: StoreError,
    ) -> StoreError {
        let (reply_sender, reply_receiver) = mpsc::channel();
        let status = self.execute_command(
            Command::GetResourceContextDocumentStatus {
                workspace_id: workspace_id.to_owned(),
                resource_id: resource_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        );
        prefer_context_document_status_error_after_read_failure(blob_error, status)
    }
}

impl StateStore for SqliteWorkspaceStore {
    fn get_workspace(&self, workspace_id: &str) -> Result<Option<Workspace>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetWorkspace {
                workspace_id: workspace_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_workspaces(&self) -> Result<Vec<Workspace>, StoreError> {
        SqliteWorkspaceStore::list_workspaces(self)
    }

    fn commit_workspace(
        &self,
        expected_version: Option<u64>,
        workspace: Workspace,
        event: EventDraft,
    ) -> Result<CommittedWorkspace, StoreError> {
        validate_commit(&workspace, &event, expected_version)?;
        let state_bytes = canonical_json(&workspace)?;
        let state_blob = self.inner.blobs.put(
            &workspace.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            STATE_MEDIA_TYPE,
        )?;
        if state_blob.size_bytes != state_bytes.len() as u64
            || self.inner.blobs.get(
                &workspace.workspace_id,
                BlobPurpose::AggregateState,
                &state_blob,
            )? != state_bytes
        {
            return Err(StoreError::Integrity(
                "BlobStore did not durably verify aggregate state".to_owned(),
            ));
        }
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: workspace.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CommitWorkspace {
                commit: Box::new(WorkspaceCommit {
                    expected_version,
                    workspace,
                    draft: event,
                    state_ref,
                    request: None,
                }),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl IdempotentWorkspaceStore for SqliteWorkspaceStore {
    fn create_workspace_idempotent(
        &self,
        request: WorkspaceCreateRequest,
        workspace: Workspace,
        event: EventDraft,
    ) -> Result<CommittedWorkspace, StoreError> {
        self.commit_workspace_with_request(Some(request), None, workspace, event)
    }

    fn commit_workspace_idempotent(
        &self,
        request: WorkspaceCreateRequest,
        expected_version: u64,
        workspace: Workspace,
        event: EventDraft,
    ) -> Result<CommittedWorkspace, StoreError> {
        self.commit_workspace_with_request(Some(request), Some(expected_version), workspace, event)
    }

    fn create_workspace_instruction_revision_idempotent(
        &self,
        request: WorkspaceCreateRequest,
        expected_version: u64,
        workspace: Workspace,
        instruction_revision: WorkspaceInstructionRevisionRecord,
        event: EventDraft,
    ) -> Result<CommittedWorkspaceInstructionRevision, StoreError> {
        if workspace.workspace_id != instruction_revision.workspace_id
            || workspace.workspace_id != event.workspace_id
            || workspace.workspace_id != event.entity_id
            || event.entity_type != "Workspace"
            || workspace.version != expected_version.saturating_add(1)
            || event.entity_revision != workspace.version
            || workspace.current_instruction_revision != Some(instruction_revision.revision)
        {
            return Err(StoreError::Invalid(
                "Workspace instruction revision commit is inconsistent".to_owned(),
            ));
        }
        let state_bytes = canonical_json(&workspace)?;
        let state_blob = self.inner.blobs.put(
            &workspace.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            STATE_MEDIA_TYPE,
        )?;
        if state_blob.size_bytes != state_bytes.len() as u64
            || self.inner.blobs.get(
                &workspace.workspace_id,
                BlobPurpose::AggregateState,
                &state_blob,
            )? != state_bytes
        {
            return Err(StoreError::Integrity(
                "Workspace instruction aggregate state failed verification".to_owned(),
            ));
        }
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: workspace.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CreateWorkspaceInstructionRevision {
                request,
                expected_version,
                workspace,
                instruction_revision,
                draft: event,
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl EventStore for SqliteWorkspaceStore {
    fn read_workspace_events(&self, workspace_id: &str) -> Result<Vec<DomainEvent>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ReadWorkspaceEvents {
                workspace_id: workspace_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl RuntimeLifecycleStore for SqliteWorkspaceStore {
    fn register_local_incarnation(
        &self,
        runtime: RuntimeRecord,
        incarnation: RuntimeIncarnationRecord,
        observation: RuntimeIncarnationLocalObservationRecord,
    ) -> Result<RuntimeIncarnationRecord, StoreError> {
        validate_local_runtime_registration(&runtime, &incarnation, &observation)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::RegisterLocalRuntimeIncarnation {
                runtime,
                incarnation,
                observation,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn transition_local_incarnation(
        &self,
        update: RuntimeIncarnationStateUpdate,
    ) -> Result<RuntimeIncarnationRecord, StoreError> {
        validate_runtime_state_update(&update)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::TransitionLocalRuntimeIncarnation {
                update,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl RuntimeWorkspaceBindingStore for SqliteWorkspaceStore {
    fn enroll_local_runtime(
        &self,
        request: LocalRuntimeWorkspaceEnrollmentRequest,
    ) -> Result<RuntimeWorkspaceBindingRecord, StoreError> {
        validate_local_runtime_workspace_enrollment(&request)?;
        let binding = RuntimeWorkspaceBindingRecord {
            runtime_workspace_binding_id: request.runtime_workspace_binding_id.clone(),
            runtime_id: request.runtime_id.clone(),
            workspace_id: request.workspace_id.clone(),
            enrollment_mode: "LOCAL_ENROLLMENT".to_owned(),
            status: "ACTIVE".to_owned(),
            roles: vec![
                "EXECUTOR".to_owned(),
                "OPERATOR_ENDPOINT".to_owned(),
                "TRIGGER_HOST".to_owned(),
            ],
            created_at: request.now.clone(),
            activated_at: Some(request.now.clone()),
            revoked_at: None,
            version: 1,
        };
        let request_digest = digest(&canonical_json(&json!({
        "request_payload": request.request.request_payload,
        "workspace_id": request.workspace_id,
        "expected_workspace_version": request.expected_workspace_version,
        "runtime_id": request.runtime_id,
        "runtime_incarnation_id": request.runtime_incarnation_id,
        }))?);
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::EnrollLocalRuntimeInWorkspace {
                request,
                request_digest,
                binding,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_current_local_binding(
        &self,
        lookup: LocalRuntimeWorkspaceBindingLookup,
    ) -> Result<Option<RuntimeWorkspaceBindingRecord>, StoreError> {
        validate_local_runtime_workspace_binding_lookup(&lookup)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetCurrentLocalRuntimeWorkspaceBinding {
                lookup,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl AgentCatalogStore for SqliteWorkspaceStore {
    fn put_agent_profile(
        &self,
        profile: AgentProfileRecord,
        endpoints: Vec<AgentEndpointRecord>,
    ) -> Result<(), StoreError> {
        validate_agent_profile(&profile, &endpoints)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::PutAgentProfile {
                profile,
                endpoints,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn register_local_endpoint_binding(
        &self,
        binding: storage_core::LocalAgentEndpointBindingInput,
    ) -> Result<(), StoreError> {
        validate_local_endpoint_binding(&binding)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::RegisterLocalAgentEndpointBinding {
                binding,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_local_endpoint_binding(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        endpoint_id: &str,
        now: &str,
    ) -> Result<Option<LocalAgentEndpointBindingRecord>, StoreError> {
        validate_nonempty(&[runtime_id, runtime_incarnation_id, endpoint_id])?;
        validate_timestamp(now)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetLocalAgentEndpointBinding {
                runtime_id: runtime_id.to_owned(),
                runtime_incarnation_id: runtime_incarnation_id.to_owned(),
                endpoint_id: endpoint_id.to_owned(),
                now: now.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn publish_runtime_offer(&self, offer: RuntimeOfferRecord) -> Result<(), StoreError> {
        validate_runtime_offer(&offer)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::PublishRuntimeOffer {
                offer,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_agent_profiles(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        now: &str,
    ) -> Result<Vec<AgentProfileViewRecord>, StoreError> {
        validate_nonempty(&[owner_principal_id, workspace_id])?;
        validate_timestamp(now)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListAgentProfiles {
                owner_principal_id: owner_principal_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                now: now.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn create_agent_binding(
        &self,
        request: AgentBindingCreateRequest,
    ) -> Result<CommittedAgentBinding, StoreError> {
        validate_agent_binding_create(&request)?;
        let state_ref = self.agent_binding_state_ref(&request.binding)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CreateAgentBinding {
                request,
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_agent_binding(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        agent_binding_id: &str,
    ) -> Result<Option<AgentBindingRecord>, StoreError> {
        validate_nonempty(&[owner_principal_id, workspace_id, agent_binding_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetAgentBinding {
                owner_principal_id: owner_principal_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                agent_binding_id: agent_binding_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_agent_bindings(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<AgentBindingRecord>, StoreError> {
        validate_nonempty(&[owner_principal_id, workspace_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListAgentBindings {
                owner_principal_id: owner_principal_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn enable_agent_binding(
        &self,
        request: AgentBindingEnableRequest,
    ) -> Result<CommittedAgentBinding, StoreError> {
        validate_agent_binding_enable(&request)?;
        let request_digest = digest(&canonical_json(&request.request.request_payload)?);
        let (receipt_sender, receipt_receiver) = mpsc::channel();
        if let Some(replay) = self.execute_command(
            Command::GetAgentBindingReceipt {
                principal_id: request.request.principal_id.clone(),
                request_id: request.request.request_id.clone(),
                request_digest,
                reply: receipt_sender,
            },
            receipt_receiver,
        )? {
            return Ok(replay);
        }
        let binding = self
            .get_agent_binding(
                &request.request.principal_id,
                &request.workspace_id,
                &request.agent_binding_id,
            )?
            .ok_or(StoreError::NotFound)?;
        let mut next = binding;
        if next.version != request.expected_version {
            return Err(StoreError::Conflict {
                expected: Some(request.expected_version),
                actual: Some(next.version),
            });
        }
        if next.enabled {
            return Err(StoreError::Invalid(
                "AgentBinding is already enabled".to_owned(),
            ));
        }
        next.enabled = true;
        next.version = next
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("AgentBinding version exhausted".to_owned()))?;
        let state_ref = self.agent_binding_state_ref(&next)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::EnableAgentBinding {
                request,
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl SqliteWorkspaceStore {
    fn agent_binding_state_ref(
        &self,
        binding: &AgentBindingRecord,
    ) -> Result<AggregateStateRef, StoreError> {
        let bytes = canonical_json(binding)?;
        let blob = self.inner.blobs.put(
            &binding.workspace_id,
            BlobPurpose::AggregateState,
            &bytes,
            "application/vnd.litecowork.agent-binding+json",
        )?;
        if blob.size_bytes != bytes.len() as u64
            || self
                .inner
                .blobs
                .get(&binding.workspace_id, BlobPurpose::AggregateState, &blob)?
                != bytes
        {
            return Err(StoreError::Integrity(
                "BlobStore did not durably verify AgentBinding state".to_owned(),
            ));
        }
        Ok(AggregateStateRef {
            blob,
            entity_revision: binding.version,
            record_schema_version: 1,
        })
    }
}

impl AgentSessionStore for SqliteWorkspaceStore {
    fn start_task_planning_session(
        &self,
        mut start: TaskPlanningSessionStart,
    ) -> Result<CommittedAgentSession, StoreError> {
        // Reject before canonicalization, BlobStore writes, idempotency receipts, or
        // AgentSession/event persistence so direct internal callers cannot bypass the
        // PlanningCoordinator preflight gate.
        require_task_planning_isolation_admission()?;
        start.session.started_at = canonicalize_utc_timestamp(&start.session.started_at)?;
        if start.session.version != 1
            || start.session.status != "STARTING"
            || start.session.scope_kind != "TASK_PLANNING"
            || start.session.conversation_id.is_some()
            || start.session.conversation_turn_id.is_some()
            || start.session.task_id.is_none()
            || start.session.task_spec_revision.is_none()
            || start.session.attempt_id.is_some()
            || start.expected_task_version == 0
        {
            return Err(StoreError::Invalid(
                "Task planning session start is invalid".to_owned(),
            ));
        }
        let bytes = canonical_json(&start.session)?;
        let blob = self.inner.blobs.put(
            &start.session.workspace_id,
            BlobPurpose::AggregateState,
            &bytes,
            "application/vnd.litecowork.agent-session+json",
        )?;
        if blob.size_bytes != bytes.len() as u64
            || self.inner.blobs.get(
                &start.session.workspace_id,
                BlobPurpose::AggregateState,
                &blob,
            )? != bytes
        {
            return Err(StoreError::Integrity(
                "AgentSession aggregate state failed blob verification".to_owned(),
            ));
        }
        let state_ref = AggregateStateRef {
            blob,
            entity_revision: start.session.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::StartTaskPlanningSession {
                start,
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_agent_session(
        &self,
        workspace_id: &str,
        agent_session_id: &str,
    ) -> Result<Option<AgentSessionRecord>, StoreError> {
        if workspace_id.trim().is_empty() || agent_session_id.trim().is_empty() {
            return Err(StoreError::Invalid(
                "AgentSession identity is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetAgentSession {
                workspace_id: workspace_id.to_owned(),
                agent_session_id: agent_session_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn mark_starting_task_planning_session_lost(
        &self,
        mut transition: MarkStartingAgentSessionLost,
    ) -> Result<CommittedAgentSession, StoreError> {
        transition.occurred_at = canonicalize_utc_timestamp(&transition.occurred_at)?;
        let current = self
            .get_agent_session(&transition.workspace_id, &transition.agent_session_id)?
            .ok_or(StoreError::NotFound)?;
        if current.scope_kind != "TASK_PLANNING"
            || current.status != "STARTING"
            || current.version != transition.expected_version
        {
            return Err(StoreError::Conflict {
                expected: Some(transition.expected_version),
                actual: Some(current.version),
            });
        }
        let next_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("AgentSession version exhausted".to_owned()))?;
        let mut next = current;
        next.status = "LOST".to_owned();
        next.last_event_at = Some(transition.occurred_at.clone());
        next.closed_at = Some(transition.occurred_at.clone());
        next.version = next_version;
        let task_id = next
            .task_id
            .as_deref()
            .ok_or_else(|| StoreError::Integrity("planning session Task is missing".to_owned()))?;
        let task_spec_revision = next.task_spec_revision.ok_or_else(|| {
            StoreError::Integrity("planning session TaskSpec is missing".to_owned())
        })?;
        if transition.event.workspace_id != next.workspace_id
            || transition.event.entity_type != "AgentSession"
            || transition.event.entity_id != next.agent_session_id
            || transition.event.event_type != "agent.session.lost.v1"
            || transition.event.entity_revision != next.version
            || transition.event.payload
                != json!({
                    "agent_session_id": next.agent_session_id,
                    "scope": {"kind":"TASK_PLANNING", "task_id":task_id},
                    "agent_binding_id": next.agent_binding_id,
                    "endpoint_id": next.endpoint_id,
                    "runtime_id": next.runtime_id,
                    "runtime_incarnation_id": next.runtime_incarnation_id,
                    "task_spec_revision": task_spec_revision,
                    "session_state": "LOST",
                    "reported_at": transition.occurred_at,
                })
        {
            return Err(StoreError::Invalid(
                "AgentSession lost transition is inconsistent".to_owned(),
            ));
        }
        let bytes = canonical_json(&next)?;
        let blob = self.inner.blobs.put(
            &next.workspace_id,
            BlobPurpose::AggregateState,
            &bytes,
            "application/vnd.litecowork.agent-session+json",
        )?;
        if blob.size_bytes != bytes.len() as u64
            || self
                .inner
                .blobs
                .get(&next.workspace_id, BlobPurpose::AggregateState, &blob)?
                != bytes
        {
            return Err(StoreError::Integrity(
                "AgentSession lost state failed blob verification".to_owned(),
            ));
        }
        let state_ref = AggregateStateRef {
            blob,
            entity_revision: next.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::MarkStartingAgentSessionLost {
                transition,
                next,
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn activate_task_planning_session(
        &self,
        mut activation: ActivateTaskPlanningSession,
    ) -> Result<CommittedPlanningActivation, StoreError> {
        // Keep the storage boundary fail-closed as well as session reservation: a
        // direct caller cannot activate a planner or transition a READY Task to RUNNING
        // until Runtime-local isolation attestation is integrated and rechecked here.
        require_task_planning_isolation_admission()?;
        activation.occurred_at = canonicalize_utc_timestamp(&activation.occurred_at)?;
        if activation.expected_session_version == 0
            || activation.expected_task_version == 0
            || activation.workspace_id.trim().is_empty()
            || activation.agent_session_id.trim().is_empty()
            || activation.host_instance_id.trim().is_empty()
            || activation
                .native_session_ref
                .as_ref()
                .is_some_and(|handle| handle.len() > 4096 || handle.is_empty())
        {
            return Err(StoreError::Invalid(
                "planning session activation is invalid".to_owned(),
            ));
        }
        let mut session = self
            .get_agent_session(&activation.workspace_id, &activation.agent_session_id)?
            .ok_or(StoreError::NotFound)?;
        let task_id = session
            .task_id
            .as_deref()
            .ok_or_else(|| StoreError::Integrity("planning session Task is missing".to_owned()))?;
        if session.scope_kind != "TASK_PLANNING"
            || session.status != "STARTING"
            || session.version != activation.expected_session_version
        {
            return Err(StoreError::Conflict {
                expected: Some(activation.expected_session_version),
                actual: Some(session.version),
            });
        }
        let mut task = self
            .get_task(&activation.workspace_id, task_id)?
            .ok_or(StoreError::NotFound)?;
        if task.task.version != activation.expected_task_version
            || task.task.current_spec_revision != session.task_spec_revision.unwrap_or_default()
            || task.task.lead_agent_binding_id != session.agent_binding_id
            || !matches!(task.task.status.as_str(), "READY" | "RUNNING")
        {
            return Err(StoreError::Conflict {
                expected: Some(activation.expected_task_version),
                actual: Some(task.task.version),
            });
        }
        let task_needs_running_transition = task.task.status == "READY";
        if task_needs_running_transition {
            task.task.status = "RUNNING".to_owned();
            task.task.updated_at = activation.occurred_at.clone();
            task.task.version = task
                .task
                .version
                .checked_add(1)
                .ok_or_else(|| StoreError::Integrity("Task version exhausted".to_owned()))?;
        }
        session.status = "ACTIVE".to_owned();
        session.last_event_at = Some(activation.occurred_at.clone());
        session.version = session
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("AgentSession version exhausted".to_owned()))?;
        let task_spec_revision = session.task_spec_revision.ok_or_else(|| {
            StoreError::Integrity("planning session TaskSpec is missing".to_owned())
        })?;
        if activation.session_event.workspace_id != session.workspace_id
            || activation.session_event.schema_version != 1
            || activation.session_event.entity_type != "AgentSession"
            || activation.session_event.entity_id != session.agent_session_id
            || activation.session_event.event_type != "agent.session.started.v1"
            || activation.session_event.entity_revision != session.version
            || activation.session_event.payload
                != json!({
                    "agent_session_id": session.agent_session_id,
                    "scope": {"kind":"TASK_PLANNING", "task_id":task_id},
                    "agent_binding_id": session.agent_binding_id,
                    "endpoint_id": session.endpoint_id,
                    "runtime_id": session.runtime_id,
                    "runtime_incarnation_id": session.runtime_incarnation_id,
                    "task_spec_revision": task_spec_revision,
                    "session_state": "ACTIVE",
                    "reported_at": activation.occurred_at,
                })
        {
            return Err(StoreError::Invalid(
                "planning session readiness event is inconsistent".to_owned(),
            ));
        }
        if task_needs_running_transition {
            let event = activation.task_status_event.as_ref().ok_or_else(|| {
                StoreError::Invalid("Task RUNNING transition event is required".to_owned())
            })?;
            if event.workspace_id != task.task.workspace_id
                || event.schema_version != 1
                || event.entity_type != "Task"
                || event.entity_id != task.task.task_id
                || event.event_type != "task.status.changed.v1"
                || event.entity_revision != task.task.version
                || event.payload
                    != json!({
                        "task_id": task.task.task_id,
                        "from": "READY",
                        "to": "RUNNING",
                        "reason_code": "LEAD_PLANNING_SESSION_READY",
                        "actor": {"service_id":"PlanningCoordinator"},
                        "aggregate_version": task.task.version,
                        "blocking_conditions": task.task.blocking_conditions,
                    })
            {
                return Err(StoreError::Invalid(
                    "Task RUNNING transition event is inconsistent".to_owned(),
                ));
            }
        } else if activation.task_status_event.is_some() {
            return Err(StoreError::Invalid(
                "unchanged Task must not append a status event".to_owned(),
            ));
        }

        let session_bytes = canonical_json(&session)?;
        let session_blob = self.inner.blobs.put(
            &session.workspace_id,
            BlobPurpose::AggregateState,
            &session_bytes,
            "application/vnd.litecowork.agent-session+json",
        )?;
        if session_blob.size_bytes != session_bytes.len() as u64
            || self.inner.blobs.get(
                &session.workspace_id,
                BlobPurpose::AggregateState,
                &session_blob,
            )? != session_bytes
        {
            return Err(StoreError::Integrity(
                "active AgentSession aggregate state failed blob verification".to_owned(),
            ));
        }
        let session_state_ref = AggregateStateRef {
            blob: session_blob,
            entity_revision: session.version,
            record_schema_version: 1,
        };
        let task_state_ref = if task_needs_running_transition {
            let snapshot = TaskAggregateSnapshot {
                task: task.task.clone(),
                current_spec_revision: task.current_spec_revision.clone(),
                current_plan_revision: None,
                current_steps: Vec::new(),
            };
            let task_bytes = canonical_json(&snapshot)?;
            let task_blob = self.inner.blobs.put(
                &task.task.workspace_id,
                BlobPurpose::AggregateState,
                &task_bytes,
                TASK_STATE_MEDIA_TYPE,
            )?;
            if task_blob.size_bytes != task_bytes.len() as u64
                || self.inner.blobs.get(
                    &task.task.workspace_id,
                    BlobPurpose::AggregateState,
                    &task_blob,
                )? != task_bytes
            {
                return Err(StoreError::Integrity(
                    "running Task aggregate state failed blob verification".to_owned(),
                ));
            }
            Some(AggregateStateRef {
                blob: task_blob,
                entity_revision: task.task.version,
                record_schema_version: 1,
            })
        } else {
            None
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ActivateTaskPlanningSession {
                activation,
                session,
                task,
                session_state_ref,
                task_state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_starting_task_planning_sessions(
        &self,
        workspace_id: &str,
        limit: usize,
    ) -> Result<Vec<AgentSessionRecord>, StoreError> {
        if workspace_id.trim().is_empty() || !(1..=500).contains(&limit) {
            return Err(StoreError::Invalid(
                "AgentSession recovery query is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListStartingTaskPlanningSessions {
                workspace_id: workspace_id.to_owned(),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl AgentHostStore for SqliteWorkspaceStore {
    fn create_agent_host_instance(
        &self,
        mut host: AgentHostInstanceRecord,
    ) -> Result<(), StoreError> {
        host.started_at = canonicalize_utc_timestamp(&host.started_at)?;
        host.last_used_at = canonicalize_utc_timestamp(&host.last_used_at)?;
        host.idle_since = host
            .idle_since
            .as_deref()
            .map(canonicalize_utc_timestamp)
            .transpose()?;
        validate_agent_host_instance(&host)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CreateAgentHostInstance {
                host,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn transition_agent_host_instance(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        host_instance_id: &str,
        expected_state: &str,
        next_state: &str,
        occurred_at: &str,
        process_identity_ref: Option<&str>,
    ) -> Result<AgentHostInstanceRecord, StoreError> {
        validate_nonempty(&[runtime_id, runtime_incarnation_id, host_instance_id])?;
        let occurred_at = canonicalize_utc_timestamp(occurred_at)?;
        if !valid_agent_host_transition(expected_state, next_state)
            || process_identity_ref
                .is_some_and(|value| value.len() > 512 || value.chars().any(char::is_control))
        {
            return Err(StoreError::Invalid(
                "AgentHostInstance transition is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::TransitionAgentHostInstance {
                runtime_id: runtime_id.to_owned(),
                runtime_incarnation_id: runtime_incarnation_id.to_owned(),
                host_instance_id: host_instance_id.to_owned(),
                expected_state: expected_state.to_owned(),
                next_state: next_state.to_owned(),
                occurred_at,
                process_identity_ref: process_identity_ref.map(str::to_owned),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_agent_host_instance(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        host_instance_id: &str,
    ) -> Result<Option<AgentHostInstanceRecord>, StoreError> {
        validate_nonempty(&[runtime_id, runtime_incarnation_id, host_instance_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetAgentHostInstance {
                runtime_id: runtime_id.to_owned(),
                runtime_incarnation_id: runtime_incarnation_id.to_owned(),
                host_instance_id: host_instance_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_agent_host_instances(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
    ) -> Result<Vec<AgentHostInstanceRecord>, StoreError> {
        validate_nonempty(&[runtime_id, runtime_incarnation_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListAgentHostInstances {
                runtime_id: runtime_id.to_owned(),
                runtime_incarnation_id: runtime_incarnation_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl TaskStore for SqliteWorkspaceStore {
    fn create_task(
        &self,
        mut commit: TaskCreateCommit,
    ) -> Result<storage_core::CommittedTask, StoreError> {
        canonicalize_task_timestamps(&mut commit)?;
        validate_task_create_commit(&commit)?;
        let snapshot = TaskAggregateSnapshot {
            task: commit.task.clone(),
            current_spec_revision: commit.initial_spec_revision.clone(),
            current_plan_revision: None,
            current_steps: Vec::new(),
        };
        let state_bytes = canonical_json(&snapshot)?;
        let state_blob = self.inner.blobs.put(
            &commit.task.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            TASK_STATE_MEDIA_TYPE,
        )?;
        if state_blob.size_bytes != state_bytes.len() as u64
            || self.inner.blobs.get(
                &commit.task.workspace_id,
                BlobPurpose::AggregateState,
                &state_blob,
            )? != state_bytes
        {
            return Err(StoreError::Integrity(
                "Task aggregate state failed blob verification".to_owned(),
            ));
        }
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: commit.task.version,
            record_schema_version: 1,
        };
        let occurrence_state_refs = commit
            .automation_admission
            .as_ref()
            .map(|admission| {
                build_automation_occurrence_snapshots(admission, &commit.task).and_then(
                    |snapshots| {
                        snapshots
                            .into_iter()
                            .enumerate()
                            .map(|(index, snapshot)| {
                                let bytes = canonical_json(&snapshot)?;
                                let blob = self.inner.blobs.put(
                                    &commit.task.workspace_id,
                                    BlobPurpose::AggregateState,
                                    &bytes,
                                    "application/vnd.litecow.automation-occurrence+json",
                                )?;
                                if blob.size_bytes != bytes.len() as u64
                                    || self.inner.blobs.get(
                                        &commit.task.workspace_id,
                                        BlobPurpose::AggregateState,
                                        &blob,
                                    )? != bytes
                                {
                                    return Err(StoreError::Integrity(
                                        "AutomationOccurrence snapshot failed blob verification"
                                            .to_owned(),
                                    ));
                                }
                                Ok(AggregateStateRef {
                                    blob,
                                    entity_revision: index as u64 + 1,
                                    record_schema_version: 1,
                                })
                            })
                            .collect::<Result<Vec<_>, StoreError>>()?
                            .try_into()
                            .map_err(|_| {
                                StoreError::Integrity(
                                    "AutomationOccurrence snapshot count is invalid".to_owned(),
                                )
                            })
                    },
                )
            })
            .transpose()?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CreateTask {
                commit: Box::new(commit),
                state_ref,
                occurrence_state_refs,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_task_create_receipt(
        &self,
        principal_id: &str,
        request_id: &str,
        request_payload: &Value,
    ) -> Result<Option<storage_core::CommittedTask>, StoreError> {
        validate_nonempty(&[principal_id, request_id])?;
        let request_digest = digest(&canonical_json(request_payload)?);
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetTaskCreateReceipt {
                principal_id: principal_id.to_owned(),
                request_id: request_id.to_owned(),
                request_digest,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_task_spec_revision_receipt(
        &self,
        principal_id: &str,
        request_id: &str,
        request_payload: &Value,
    ) -> Result<Option<CommittedTaskSpecRevision>, StoreError> {
        validate_nonempty(&[principal_id, request_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetTaskSpecRevisionReceipt {
                principal_id: principal_id.to_owned(),
                request_id: request_id.to_owned(),
                request_payload: request_payload.clone(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn revise_task_spec(
        &self,
        mut commit: TaskSpecRevisionCommit,
    ) -> Result<CommittedTaskSpecRevision, StoreError> {
        commit.task_spec_revision.created_at =
            canonicalize_utc_timestamp(&commit.task_spec_revision.created_at)?;
        commit.event.recorded_at = canonicalize_utc_timestamp(&commit.event.recorded_at)?;
        validate_task_spec_revision_commit(&commit)?;
        let snapshot = TaskAggregateSnapshot {
            task: commit.task.clone(),
            current_spec_revision: commit.task_spec_revision.clone(),
            current_plan_revision: None,
            current_steps: Vec::new(),
        };
        let state_bytes = canonical_json(&snapshot)?;
        let state_blob = self.inner.blobs.put(
            &commit.task.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            TASK_STATE_MEDIA_TYPE,
        )?;
        if state_blob.size_bytes != state_bytes.len() as u64
            || self.inner.blobs.get(
                &commit.task.workspace_id,
                BlobPurpose::AggregateState,
                &state_blob,
            )? != state_bytes
        {
            return Err(StoreError::Integrity(
                "revised Task aggregate state failed verification".to_owned(),
            ));
        }
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: commit.task.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ReviseTaskSpec {
                commit: Box::new(commit),
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn accept_initial_plan(
        &self,
        mut commit: PlanAcceptanceCommit,
    ) -> Result<PlanAcceptance, StoreError> {
        commit.plan_revision.created_at =
            canonicalize_utc_timestamp(&commit.plan_revision.created_at)?;
        validate_plan_acceptance_commit(&commit)?;

        let mut task = self
            .get_task(&commit.workspace_id, &commit.task_id)?
            .ok_or(StoreError::NotFound)?;
        task.task.current_plan_revision = Some(1);
        task.task.version = task
            .task
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Integrity("Task version exhausted".to_owned()))?;
        task.task.updated_at = commit.plan_revision.created_at.clone();
        let snapshot = TaskAggregateSnapshot {
            task: task.task.clone(),
            current_spec_revision: task.current_spec_revision,
            current_plan_revision: Some(commit.plan_revision.clone()),
            current_steps: commit.materialized_steps.clone(),
        };
        let snapshot_bytes = canonical_json(&snapshot)?;
        let task_blob = self.inner.blobs.put(
            &commit.workspace_id,
            BlobPurpose::AggregateState,
            &snapshot_bytes,
            TASK_STATE_MEDIA_TYPE,
        )?;
        if task_blob.size_bytes != snapshot_bytes.len() as u64
            || self.inner.blobs.get(
                &commit.workspace_id,
                BlobPurpose::AggregateState,
                &task_blob,
            )? != snapshot_bytes
        {
            return Err(StoreError::Integrity(
                "accepted Task plan snapshot failed verification".to_owned(),
            ));
        }
        let task_state_ref = AggregateStateRef {
            blob: task_blob,
            entity_revision: task.task.version,
            record_schema_version: 1,
        };
        let mut step_state_refs = Vec::with_capacity(commit.materialized_steps.len());
        for step in &commit.materialized_steps {
            let bytes = canonical_json(step)?;
            let blob = self.inner.blobs.put(
                &commit.workspace_id,
                BlobPurpose::AggregateState,
                &bytes,
                "application/vnd.litecowork.step+json",
            )?;
            if blob.size_bytes != bytes.len() as u64
                || self
                    .inner
                    .blobs
                    .get(&commit.workspace_id, BlobPurpose::AggregateState, &blob)?
                    != bytes
            {
                return Err(StoreError::Integrity(
                    "accepted Step snapshot failed verification".to_owned(),
                ));
            }
            step_state_refs.push(AggregateStateRef {
                blob,
                entity_revision: step.version,
                record_schema_version: 1,
            });
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::AcceptInitialPlan {
                commit: Box::new(commit),
                task_state_ref,
                step_state_refs,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_plan_revisions(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Vec<PlanRevisionRecord>, StoreError> {
        validate_nonempty(&[workspace_id, task_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListPlanRevisions {
                workspace_id: workspace_id.to_owned(),
                task_id: task_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_steps(
        &self,
        workspace_id: &str,
        task_id: &str,
        plan_revision: Option<u64>,
    ) -> Result<Vec<StepRecord>, StoreError> {
        validate_nonempty(&[workspace_id, task_id])?;
        if plan_revision == Some(0) {
            return Err(StoreError::Invalid(
                "PlanRevision must be positive".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListSteps {
                workspace_id: workspace_id.to_owned(),
                task_id: task_id.to_owned(),
                plan_revision,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_task(&self, workspace_id: &str, task_id: &str) -> Result<Option<TaskView>, StoreError> {
        if workspace_id.trim().is_empty() || task_id.trim().is_empty() {
            return Err(StoreError::Invalid(
                "Task identity must not be empty".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetTask {
                workspace_id: workspace_id.to_owned(),
                task_id: task_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_task_spec_revisions(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Vec<TaskSpecRevisionRecord>, StoreError> {
        validate_nonempty(&[workspace_id, task_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListTaskSpecRevisions {
                workspace_id: workspace_id.to_owned(),
                task_id: task_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_tasks_page(
        &self,
        workspace_id: &str,
        status: Option<&str>,
        conversation_id: Option<&str>,
        after_created_at: Option<&str>,
        after_task_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TaskSummaryRecord>, StoreError> {
        const TASK_STATUSES: &[&str] = &[
            "READY",
            "RUNNING",
            "WAITING_USER",
            "BLOCKED",
            "VERIFYING",
            "NEEDS_USER",
            "INCOMPLETE",
            "PAUSE_REQUESTED",
            "PAUSED",
            "COMPLETED",
            "FAILED",
            "CANCEL_REQUESTED",
            "CANCELLED",
        ];
        if workspace_id.trim().is_empty()
            || status.is_some_and(|value| !TASK_STATUSES.contains(&value))
            || conversation_id.is_some_and(|value| value.trim().is_empty())
            || !(1..=201).contains(&limit)
            || after_created_at.is_some() != after_task_id.is_some()
        {
            return Err(StoreError::Invalid("Task page query is invalid".to_owned()));
        }
        let canonical_cursor = after_created_at
            .map(canonicalize_utc_timestamp)
            .transpose()?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListTasksPage {
                workspace_id: workspace_id.to_owned(),
                status: status.map(str::to_owned),
                conversation_id: conversation_id.map(str::to_owned),
                after_created_at: canonical_cursor,
                after_task_id: after_task_id.map(str::to_owned),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl ResourceStore for SqliteWorkspaceStore {
    fn create_resource(
        &self,
        request: WorkspaceCreateRequest,
        resource: ResourceRecord,
        revision: ResourceRevisionRecord,
        event: EventDraft,
        content: Vec<u8>,
    ) -> Result<CommittedResource, StoreError> {
        let content_digest = digest(&content);
        if resource.version != 1
            || revision.resource_id != resource.resource_id
            || revision.size_bytes != Some(content.len() as u64)
            || revision.content_digest.as_deref() != Some(content_digest.as_str())
            || event.workspace_id != resource.workspace_id
            || event.entity_type != "Resource"
            || event.entity_id != resource.resource_id
            || event.entity_revision != resource.version
        {
            return Err(StoreError::Invalid(
                "Resource create payload is inconsistent".to_owned(),
            ));
        }
        let state_bytes = canonical_json(&resource)?;
        let state_blob = self.inner.blobs.put(
            &resource.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            "application/vnd.litecowork.resource+json",
        )?;
        if self.inner.blobs.get(
            &resource.workspace_id,
            BlobPurpose::AggregateState,
            &state_blob,
        )? != state_bytes
        {
            return Err(StoreError::Integrity(
                "Resource aggregate state failed blob verification".to_owned(),
            ));
        }
        let content_blob = self.inner.blobs.put(
            &resource.workspace_id,
            BlobPurpose::Resource,
            &content,
            revision
                .media_type
                .as_deref()
                .unwrap_or("application/octet-stream"),
        )?;
        if content_blob.digest != revision.content_digest.clone().unwrap_or_default()
            || content_blob.size_bytes != content.len() as u64
            || self
                .inner
                .blobs
                .get(&resource.workspace_id, BlobPurpose::Resource, &content_blob)?
                != content
        {
            return Err(StoreError::Integrity(
                "Resource content failed blob verification".to_owned(),
            ));
        }
        let media_type = revision
            .media_type
            .as_deref()
            .unwrap_or("application/octet-stream");
        let text_index =
            if resource_index::not_indexable_reason(&resource.display_name, media_type, &content)
                .is_some()
            {
                None
            } else {
                self.ensure_resource_index_key_history_available(&resource.workspace_id)?;
                resource_index::prepare(
                    self.inner.blobs.as_ref(),
                    &resource.workspace_id,
                    &resource.resource_id,
                    &revision.resource_revision_id,
                    &resource.display_name,
                    media_type,
                    revision.content_digest.as_deref().unwrap_or_default(),
                    &revision.observed_at,
                    &content,
                )?
            };
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: resource.version,
            record_schema_version: 1,
        };
        let location_id = format!("loc_{}", resource.resource_id);
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CreateResource {
                request,
                resource,
                revision,
                draft: event,
                state_ref,
                content_blob,
                text_index,
                location_id,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn read_resource_content(
        &self,
        workspace_id: &str,
        resource_id: &str,
    ) -> Result<Option<StoredResourceContent>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        let resolved = self.execute_command(
            Command::ReadResourceContent {
                workspace_id: workspace_id.to_owned(),
                resource_id: resource_id.to_owned(),
                revision_id: None,
                maximum_bytes: None,
                reply: reply_sender,
            },
            reply_receiver,
        )?;
        let Some((summary, blob)) = resolved else {
            return Ok(None);
        };
        let content = match self
            .inner
            .blobs
            .get(workspace_id, BlobPurpose::Resource, &blob)
        {
            Ok(content) => content,
            Err(error) => {
                return Err(self.resource_read_error_after_blob_failure(
                    workspace_id,
                    resource_id,
                    error,
                ));
            }
        };
        if content.len() as u64 != summary.size_bytes || digest(&content) != summary.content_digest
        {
            return Err(StoreError::Integrity(
                "Resource content does not match its immutable revision".to_owned(),
            ));
        }
        Ok(Some(StoredResourceContent { summary, content }))
    }

    fn get_resource_record(
        &self,
        workspace_id: &str,
        resource_id: &str,
    ) -> Result<Option<ResourceRecord>, StoreError> {
        SqliteWorkspaceStore::get_resource_record(self, workspace_id, resource_id)
    }

    fn get_resource_detail(
        &self,
        workspace_id: &str,
        resource_id: &str,
    ) -> Result<Option<storage_core::ResourceDetailRecord>, StoreError> {
        SqliteWorkspaceStore::get_resource_detail(self, workspace_id, resource_id)
    }

    fn list_resource_revisions_page(
        &self,
        workspace_id: &str,
        resource_id: &str,
        after_revision_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<storage_core::ResourceRevisionViewRecord>, StoreError> {
        SqliteWorkspaceStore::list_resource_revisions_page(
            self,
            workspace_id,
            resource_id,
            after_revision_id,
            limit,
        )
    }

    fn read_resource_content_bounded(
        &self,
        workspace_id: &str,
        resource_id: &str,
        revision_id: Option<&str>,
        maximum_bytes: u64,
    ) -> Result<Option<StoredResourceContent>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        let resolved = self.execute_command(
            Command::ReadResourceContent {
                workspace_id: workspace_id.to_owned(),
                resource_id: resource_id.to_owned(),
                revision_id: revision_id.map(str::to_owned),
                maximum_bytes: Some(maximum_bytes),
                reply: reply_sender,
            },
            reply_receiver,
        )?;
        let Some((summary, blob)) = resolved else {
            return Ok(None);
        };
        let content = match self
            .inner
            .blobs
            .get(workspace_id, BlobPurpose::Resource, &blob)
        {
            Ok(content) => content,
            Err(error) => {
                return Err(self.resource_read_error_after_blob_failure(
                    workspace_id,
                    resource_id,
                    error,
                ));
            }
        };
        if content.len() as u64 != summary.size_bytes || digest(&content) != summary.content_digest
        {
            return Err(StoreError::Integrity(
                "Resource content does not match its immutable revision".to_owned(),
            ));
        }
        Ok(Some(StoredResourceContent { summary, content }))
    }

    fn search_resources_page(
        &self,
        workspace_id: &str,
        query: Option<&str>,
        kind: Option<&str>,
        freshness: Option<&str>,
        after_created_at: Option<&str>,
        after_resource_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ResourceSearchRecord>, StoreError> {
        SqliteWorkspaceStore::search_resources_page(
            self,
            workspace_id,
            query,
            kind,
            freshness,
            after_created_at,
            after_resource_id,
            limit,
        )
    }

    fn search_indexed_resource_text(
        &self,
        workspace_id: &str,
        query: &str,
        kind: Option<&str>,
        freshness: Option<&str>,
        after_created_at: Option<&str>,
        after_resource_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ResourceTextSearchRecord>, StoreError> {
        if workspace_id.trim().is_empty()
            || !(1..=201).contains(&limit)
            || after_created_at.is_some() != after_resource_id.is_some()
        {
            return Err(StoreError::Invalid(
                "indexed Resource search query is invalid".to_owned(),
            ));
        }
        let terms = resource_index::query_terms(query)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        let versions = self.execute_command(
            Command::ListResourceIndexKeyVersions {
                workspace_id: workspace_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )?;
        if versions.is_empty() {
            return Ok(Vec::new());
        }
        if versions.len() > resource_index::MAX_SEARCHABLE_KEY_VERSIONS {
            return Err(StoreError::Blob(
                "Resource index has too many retained key versions; reindex before searching"
                    .to_owned(),
            ));
        }
        let mut tokens_by_version = Vec::with_capacity(versions.len());
        for version in versions {
            let (resolved_version, tokens) =
                self.inner
                    .blobs
                    .resource_index_tokens(workspace_id, Some(version), &terms)?;
            if resolved_version != version || tokens.len() != terms.len() {
                return Err(StoreError::Integrity(
                    "Resource index key version changed during search".to_owned(),
                ));
            }
            tokens_by_version.push((version, tokens));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        let candidates = self.execute_command(
            Command::SearchResourceIndexCandidates {
                workspace_id: workspace_id.to_owned(),
                tokens_by_version,
                kind: kind.map(str::to_owned),
                freshness: freshness.map(str::to_owned),
                after_created_at: after_created_at.map(str::to_owned),
                after_resource_id: after_resource_id.map(str::to_owned),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )?;
        let materialized = resource_index::materialize_matches(
            self.inner.blobs.as_ref(),
            workspace_id,
            candidates,
            &terms,
        )?;
        let mut current = Vec::with_capacity(materialized.len());
        for match_record in materialized {
            let (reply_sender, reply_receiver) = mpsc::channel();
            let is_current = self.execute_command(
                Command::RecheckResourceIndexCandidate {
                    workspace_id: workspace_id.to_owned(),
                    resource_id: match_record.result.summary.resource_id.clone(),
                    revision_id: match_record.resource_revision_id.clone(),
                    source_digest: match_record.source_content_digest.clone(),
                    reply: reply_sender,
                },
                reply_receiver,
            )?;
            if is_current {
                current.push(match_record);
            }
        }
        Ok(current)
    }

    fn reindex_resource_text(
        &self,
        mut request: ResourceTextIndexRebuildRequest,
    ) -> Result<ResourceTextIndexRebuildResult, StoreError> {
        validate_nonempty(&[
            &request.principal_id,
            &request.request_id,
            &request.workspace_id,
            &request.resource_id,
            &request.resource_revision_id,
            &request.content_digest,
            &request.indexed_at,
            &request.correlation_id,
        ])?;
        if request.request_id.len() > 128
            || !request
                .request_id
                .bytes()
                .all(|byte| byte.is_ascii_graphic())
            || !storage_core::is_sha256_digest(&request.content_digest)
        {
            return Err(StoreError::Invalid(
                "Resource text reindex identity is invalid".to_owned(),
            ));
        }
        request.indexed_at = canonicalize_utc_timestamp(&request.indexed_at)?;
        let payload = json!({
            "operation": "resource.text_index.rebuild.v1",
            "workspace_id": &request.workspace_id,
            "resource_id": &request.resource_id,
            "resource_revision_id": &request.resource_revision_id,
            "content_digest": &request.content_digest,
        });
        let request_digest = digest(&canonical_json(&payload)?);

        // Check the durable receipt before requiring source bytes or a live head. A
        // retry after a lost response must replay the original typed outcome exactly.
        let (reply_sender, reply_receiver) = mpsc::channel();
        if let Some(receipt) = self.execute_command(
            Command::CheckResourceTextIndexRebuildReceipt {
                principal_id: request.principal_id.clone(),
                request_id: request.request_id.clone(),
                request_digest: request_digest.clone(),
                reply: reply_sender,
            },
            reply_receiver,
        )? {
            return Ok(receipt);
        }

        let workspace = self
            .get_workspace(&request.workspace_id)?
            .ok_or(StoreError::NotFound)?;
        if workspace.owner_principal_id != request.principal_id {
            return Err(StoreError::NotFound);
        }
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid(
                "an archived Workspace is read-only".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        let summary = self
            .execute_command(
                Command::GetCurrentResourceSummary {
                    workspace_id: request.workspace_id.clone(),
                    resource_id: request.resource_id.clone(),
                    reply: reply_sender,
                },
                reply_receiver,
            )?
            .ok_or(StoreError::NotFound)?;
        if summary.resource_revision_id != request.resource_revision_id
            || summary.content_digest != request.content_digest
        {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }

        let (outcome, reason, index) = if summary.size_bytes
            > resource_index::MAX_INDEXABLE_RESOURCE_BYTES
        {
            (
                ResourceTextIndexRebuildOutcome::NotIndexable,
                Some(ResourceTextIndexSkipReason::OverSizeLimit),
                None,
            )
        } else if !resource_index::is_allowlisted_text(&summary.display_name, &summary.media_type) {
            (
                ResourceTextIndexRebuildOutcome::NotIndexable,
                Some(ResourceTextIndexSkipReason::UnsupportedType),
                None,
            )
        } else {
            let content = self
                .read_resource_content_bounded(
                    &request.workspace_id,
                    &request.resource_id,
                    Some(&request.resource_revision_id),
                    resource_index::MAX_INDEXABLE_RESOURCE_BYTES,
                )?
                .ok_or(StoreError::NotFound)?;
            if content.summary.resource_revision_id != request.resource_revision_id
                || content.summary.content_digest != request.content_digest
            {
                return Err(StoreError::Conflict {
                    expected: None,
                    actual: None,
                });
            }
            if let Some(reason) = resource_index::not_indexable_reason(
                &summary.display_name,
                &summary.media_type,
                &content.content,
            ) {
                (
                    ResourceTextIndexRebuildOutcome::NotIndexable,
                    Some(reason),
                    None,
                )
            } else {
                self.ensure_resource_index_key_history_available(&request.workspace_id)?;
                let index = resource_index::prepare(
                    self.inner.blobs.as_ref(),
                    &request.workspace_id,
                    &request.resource_id,
                    &request.resource_revision_id,
                    &summary.display_name,
                    &summary.media_type,
                    &request.content_digest,
                    &request.indexed_at,
                    &content.content,
                )?
                .ok_or_else(|| {
                    StoreError::Integrity(
                        "Resource index preparation declined an eligible source".to_owned(),
                    )
                })?;
                (ResourceTextIndexRebuildOutcome::Indexed, None, Some(index))
            }
        };
        let result = ResourceTextIndexRebuildResult {
            request_id: request.request_id.clone(),
            correlation_id: request.correlation_id.clone(),
            workspace_id: request.workspace_id.clone(),
            resource_id: request.resource_id.clone(),
            resource_revision_id: request.resource_revision_id.clone(),
            content_digest: request.content_digest.clone(),
            outcome,
            reason,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CommitResourceTextIndexRebuild {
                request,
                request_digest,
                result,
                index,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl WorkspaceRootStore for SqliteWorkspaceStore {
    fn create_workspace_root(
        &self,
        commit: WorkspaceRootCreateCommit,
    ) -> Result<CommittedWorkspaceRoot, StoreError> {
        validate_workspace_root_commit(&commit)?;
        let resource_bytes = canonical_json(&commit.resource)?;
        let resource_blob = self.inner.blobs.put(
            &commit.resource.workspace_id,
            BlobPurpose::AggregateState,
            &resource_bytes,
            "application/vnd.litecowork.resource+json",
        )?;
        if self.inner.blobs.get(
            &commit.resource.workspace_id,
            BlobPurpose::AggregateState,
            &resource_blob,
        )? != resource_bytes
        {
            return Err(StoreError::Integrity(
                "Resource state blob failed verification".to_owned(),
            ));
        }
        let root_bytes = canonical_json(&commit.root)?;
        let root_blob = self.inner.blobs.put(
            &commit.root.workspace_id,
            BlobPurpose::AggregateState,
            &root_bytes,
            "application/vnd.litecowork.workspace-root+json",
        )?;
        if self.inner.blobs.get(
            &commit.root.workspace_id,
            BlobPurpose::AggregateState,
            &root_blob,
        )? != root_bytes
        {
            return Err(StoreError::Integrity(
                "WorkspaceRoot state blob failed verification".to_owned(),
            ));
        }
        let resource_state_ref = AggregateStateRef {
            blob: resource_blob,
            entity_revision: commit.resource.version,
            record_schema_version: 1,
        };
        let root_state_ref = AggregateStateRef {
            blob: root_blob,
            entity_revision: commit.root.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CreateWorkspaceRoot {
                commit,
                resource_state_ref,
                root_state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_workspace_roots(
        &self,
        workspace_id: &str,
        status: Option<&str>,
        after_created_at: Option<&str>,
        after_workspace_root_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<storage_core::WorkspaceRootListRecord>, StoreError> {
        if workspace_id.trim().is_empty()
            || !(1..=101).contains(&limit)
            || after_created_at.is_some() != after_workspace_root_id.is_some()
            || status.is_some_and(|value| {
                !matches!(value, "ACTIVE" | "PAUSED" | "REVOKED" | "UNAVAILABLE")
            })
        {
            return Err(StoreError::Invalid(
                "WorkspaceRoot page query is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListWorkspaceRoots {
                workspace_id: workspace_id.to_owned(),
                status: status.map(str::to_owned),
                after_created_at: after_created_at.map(str::to_owned),
                after_workspace_root_id: after_workspace_root_id.map(str::to_owned),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_workspace_root(
        &self,
        workspace_id: &str,
        workspace_root_id: &str,
    ) -> Result<Option<WorkspaceRootRecord>, StoreError> {
        if workspace_id.trim().is_empty() || workspace_root_id.trim().is_empty() {
            return Err(StoreError::Invalid(
                "WorkspaceRoot identity is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetWorkspaceRoot {
                workspace_id: workspace_id.to_owned(),
                workspace_root_id: workspace_root_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn update_workspace_root_status(
        &self,
        commit: WorkspaceRootStatusCommit,
    ) -> Result<CommittedWorkspaceRootStatus, StoreError> {
        validate_workspace_root_status_commit(&commit)?;
        let root_bytes = canonical_json(&commit.root)?;
        let root_blob = self.inner.blobs.put(
            &commit.root.workspace_id,
            BlobPurpose::AggregateState,
            &root_bytes,
            "application/vnd.litecowork.workspace-root+json",
        )?;
        if self.inner.blobs.get(
            &commit.root.workspace_id,
            BlobPurpose::AggregateState,
            &root_blob,
        )? != root_bytes
        {
            return Err(StoreError::Integrity(
                "WorkspaceRoot state blob failed verification".to_owned(),
            ));
        }
        let root_state_ref = AggregateStateRef {
            blob: root_blob,
            entity_revision: commit.root.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::UpdateWorkspaceRootStatus {
                commit,
                root_state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn resume_workspace_root(
        &self,
        commit: WorkspaceRootResumeCommit,
    ) -> Result<CommittedWorkspaceRootStatus, StoreError> {
        validate_workspace_root_resume_commit(&commit)?;
        let root_bytes = canonical_json(&commit.root)?;
        let root_blob = self.inner.blobs.put(
            &commit.root.workspace_id,
            BlobPurpose::AggregateState,
            &root_bytes,
            "application/vnd.litecowork.workspace-root+json",
        )?;
        if self.inner.blobs.get(
            &commit.root.workspace_id,
            BlobPurpose::AggregateState,
            &root_blob,
        )? != root_bytes
        {
            return Err(StoreError::Integrity(
                "WorkspaceRoot state blob failed verification".to_owned(),
            ));
        }
        let resource_bytes = canonical_json(&commit.resource)?;
        let resource_blob = self.inner.blobs.put(
            &commit.resource.workspace_id,
            BlobPurpose::AggregateState,
            &resource_bytes,
            "application/vnd.litecowork.resource+json",
        )?;
        if self.inner.blobs.get(
            &commit.resource.workspace_id,
            BlobPurpose::AggregateState,
            &resource_blob,
        )? != resource_bytes
        {
            return Err(StoreError::Integrity(
                "Resource state blob failed verification".to_owned(),
            ));
        }
        let root_state_ref = AggregateStateRef {
            blob: root_blob,
            entity_revision: commit.root.version,
            record_schema_version: 1,
        };
        let resource_state_ref = AggregateStateRef {
            blob: resource_blob,
            entity_revision: commit.resource.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ResumeWorkspaceRoot {
                commit,
                root_state_ref,
                resource_state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get_workspace_root_status_receipt(
        &self,
        request: &WorkspaceCreateRequest,
    ) -> Result<Option<CommittedWorkspaceRootStatus>, StoreError> {
        if request.principal_id.trim().is_empty()
            || request.request_id.trim().is_empty()
            || contains_private_path_field(&request.request_payload)
        {
            return Err(StoreError::Invalid(
                "WorkspaceRoot status receipt query is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetWorkspaceRootStatusReceipt {
                request: request.clone(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_workspace_root_revalidation_candidates(
        &self,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        after_created_at: Option<&str>,
        after_workspace_root_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkspaceRootRevalidationCandidate>, StoreError> {
        if runtime_id.trim().is_empty()
            || runtime_incarnation_id.trim().is_empty()
            || !(1..=101).contains(&limit)
            || after_created_at.is_some() != after_workspace_root_id.is_some()
        {
            return Err(StoreError::Invalid(
                "WorkspaceRoot revalidation page is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListWorkspaceRootRevalidationCandidates {
                runtime_id: runtime_id.to_owned(),
                runtime_incarnation_id: runtime_incarnation_id.to_owned(),
                after_created_at: after_created_at.map(str::to_owned),
                after_workspace_root_id: after_workspace_root_id.map(str::to_owned),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn commit_workspace_root_revalidation(
        &self,
        commit: WorkspaceRootRevalidationCommit,
    ) -> Result<CommittedWorkspaceRootRevalidation, StoreError> {
        validate_workspace_root_revalidation_commit(&commit)?;
        let root_state_ref = if commit.root_event.is_some() {
            let bytes = canonical_json(&commit.root)?;
            let blob = self.inner.blobs.put(
                &commit.root.workspace_id,
                BlobPurpose::AggregateState,
                &bytes,
                "application/vnd.litecowork.workspace-root+json",
            )?;
            if self.inner.blobs.get(
                &commit.root.workspace_id,
                BlobPurpose::AggregateState,
                &blob,
            )? != bytes
            {
                return Err(StoreError::Integrity(
                    "WorkspaceRoot state blob failed verification".to_owned(),
                ));
            }
            Some(AggregateStateRef {
                blob,
                entity_revision: commit.root.version,
                record_schema_version: 1,
            })
        } else {
            None
        };
        let resource_state_ref = if commit.location_event.is_some() {
            let bytes = canonical_json(&commit.resource)?;
            let blob = self.inner.blobs.put(
                &commit.resource.workspace_id,
                BlobPurpose::AggregateState,
                &bytes,
                "application/vnd.litecow.resource+json",
            )?;
            if self.inner.blobs.get(
                &commit.resource.workspace_id,
                BlobPurpose::AggregateState,
                &blob,
            )? != bytes
            {
                return Err(StoreError::Integrity(
                    "Resource state blob failed verification".to_owned(),
                ));
            }
            Some(AggregateStateRef {
                blob,
                entity_revision: commit.resource.version,
                record_schema_version: 1,
            })
        } else {
            None
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CommitWorkspaceRootRevalidation {
                commit,
                root_state_ref,
                resource_state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl ContextDocumentStatusStore for SqliteWorkspaceStore {
    fn set_context_document_status(
        &self,
        command: ContextDocumentStatusCommand,
    ) -> Result<CommittedContextDocumentStatus, StoreError> {
        for value in [
            command.workspace_id.as_str(),
            command.resource_id.as_str(),
            command.principal_id.as_str(),
            command.request_id.as_str(),
            command.event.event_id.as_str(),
            command.event.origin_runtime_id.as_str(),
            command.event.hlc_timestamp.as_str(),
            command.event.correlation_id.as_str(),
            command.event.recorded_at.as_str(),
        ] {
            if value.trim().is_empty() || value.contains('\0') {
                return Err(StoreError::Invalid(
                    "ContextDocument status command identity is invalid".to_owned(),
                ));
            }
        }
        if command.expected_version == 0 {
            return Err(StoreError::Invalid(
                "expected Resource version must be positive".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::SetContextDocumentStatus {
                command,
                blobs: self.inner.blobs.clone(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

impl ResourceUploadStore for SqliteWorkspaceStore {
    fn create(
        &self,
        request: WorkspaceCreateRequest,
        mut session: ResourceUploadSessionRecord,
        event: EventDraft,
    ) -> Result<ResourceUploadSessionRecord, StoreError> {
        if session.expected_size_bytes > 104_857_600
            || session.chunk_size_bytes != 4_194_304
            || session.state != ResourceUploadState::Open
            || session.resource_id.is_some()
            || session.expected_resource_version.is_some()
            || !session.parent_revision_ids.is_empty()
            || session.version != 1
            || session.progress_version != 1
        {
            return Err(StoreError::Invalid(
                "Resource upload session is invalid".to_owned(),
            ));
        }
        session.state = if session.expected_size_bytes == 0 {
            ResourceUploadState::ContentReceived
        } else {
            ResourceUploadState::Open
        };
        if event.entity_type != "ResourceUpload"
            || event.entity_id != session.upload_id
            || event.workspace_id != session.workspace_id
            || event.entity_revision != session.version
            || event.event_type != "resource.upload.created.v1"
        {
            return Err(StoreError::Invalid(
                "Resource upload creation event is invalid".to_owned(),
            ));
        }
        let state_bytes = canonical_json(&session)?;
        let state_blob = self.inner.blobs.put(
            &session.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            STATE_MEDIA_TYPE,
        )?;
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: session.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CreateResourceUpload {
                request,
                session,
                draft: event,
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn create_revision(
        &self,
        request: WorkspaceCreateRequest,
        mut session: ResourceUploadSessionRecord,
        event: EventDraft,
    ) -> Result<ResourceUploadSessionRecord, StoreError> {
        let resource_id = session.resource_id.as_deref().ok_or_else(|| {
            StoreError::Invalid("Resource revision upload has no Resource identity".to_owned())
        })?;
        let expected_version = session.expected_resource_version.ok_or_else(|| {
            StoreError::Invalid(
                "Resource revision upload has no expected Resource version".to_owned(),
            )
        })?;
        if session.workspace_id.trim().is_empty()
            || session.upload_id.trim().is_empty()
            || request.principal_id.trim().is_empty()
            || request.request_id.trim().is_empty()
            || expected_version == 0
            || session.expected_size_bytes > 104_857_600
            || session.chunk_size_bytes != 4_194_304
            || session.state != ResourceUploadState::Open
            || session.context_document.is_some()
            || session.folder_import.is_some()
            || session.committed_resource_id.is_some()
            || session.version != 1
            || session.progress_version != 1
            || session.parent_revision_ids.is_empty()
            || session.parent_revision_ids.len() > 16
            || session
                .parent_revision_ids
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != session.parent_revision_ids.len()
            || !session
                .expected_digest
                .as_deref()
                .is_some_and(storage_core::is_sha256_digest)
            || event.entity_type != "ResourceUpload"
            || event.entity_id != session.upload_id
            || event.workspace_id != session.workspace_id
            || event.entity_revision != session.version
            || event.event_type != "resource.upload.created.v1"
        {
            return Err(StoreError::Invalid(
                "Resource revision upload session is invalid".to_owned(),
            ));
        }
        let resource = self
            .get_resource_record(&session.workspace_id, resource_id)?
            .ok_or(StoreError::NotFound)?;
        session.display_name = resource.display_name;
        session.state = if session.expected_size_bytes == 0 {
            ResourceUploadState::ContentReceived
        } else {
            ResourceUploadState::Open
        };
        let state_bytes = canonical_json(&session)?;
        let state_blob = self.inner.blobs.put(
            &session.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            STATE_MEDIA_TYPE,
        )?;
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: session.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::CreateResourceUpload {
                request,
                session,
                draft: event,
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn get(
        &self,
        workspace_id: &str,
        upload_id: &str,
    ) -> Result<Option<ResourceUploadSessionRecord>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetResourceUpload {
                workspace_id: workspace_id.to_owned(),
                upload_id: upload_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn committed_resource_for_upload(
        &self,
        principal_id: &str,
        upload_id: &str,
    ) -> Result<Option<CommittedResource>, StoreError> {
        validate_nonempty(&[principal_id, upload_id])?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetCommittedResourceUpload {
                principal_id: principal_id.to_owned(),
                upload_id: upload_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn list_expired(
        &self,
        now: &str,
        limit: usize,
    ) -> Result<Vec<ResourceUploadSessionRecord>, StoreError> {
        if limit == 0 || limit > 100 || now.trim().is_empty() {
            return Err(StoreError::Invalid(
                "expired upload page is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ListExpiredResourceUploads {
                now: now.to_owned(),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn collect_orphan_chunks(&self, now: &str, limit: usize) -> Result<usize, StoreError> {
        if now.trim().is_empty() || limit == 0 || limit > 100 {
            return Err(StoreError::Invalid(
                "orphan upload blob sweep request is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        let candidates = self.execute_command(
            Command::ClaimOrphanResourceUploadBlobs {
                now: now.to_owned(),
                limit,
                reply: reply_sender,
            },
            reply_receiver,
        )?;
        let mut removed = 0;
        for (workspace_id, blob) in candidates {
            self.inner
                .blobs
                .remove(&workspace_id, BlobPurpose::ResourceUploadChunk, &blob)?;
            let (reply_sender, reply_receiver) = mpsc::channel();
            self.execute_command(
                Command::FinishOrphanResourceUploadBlob {
                    workspace_id,
                    digest: blob.digest,
                    reply: reply_sender,
                },
                reply_receiver,
            )?;
            removed += 1;
        }
        Ok(removed)
    }

    fn expire(
        &self,
        expected_progress_version: u64,
        expired: ResourceUploadSessionRecord,
        event: EventDraft,
    ) -> Result<ResourceUploadSessionRecord, StoreError> {
        if expired.state != ResourceUploadState::Expired
            || event.entity_type != "ResourceUpload"
            || event.entity_id != expired.upload_id
            || event.workspace_id != expired.workspace_id
            || event.entity_revision != expired.version
            || event.event_type != "resource.upload.status.changed.v1"
        {
            return Err(StoreError::Invalid(
                "Resource upload expiry transition is invalid".to_owned(),
            ));
        }
        let state_bytes = canonical_json(&expired)?;
        let state_blob = self.inner.blobs.put(
            &expired.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            STATE_MEDIA_TYPE,
        )?;
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: expired.version,
            record_schema_version: 1,
        };
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ExpireResourceUpload {
                expected_progress_version,
                expired,
                draft: event,
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn put_chunk(
        &self,
        chunk: ResourceUploadChunkInput,
    ) -> Result<ResourceUploadSessionRecord, StoreError> {
        let session = self
            .get_for_chunk(&chunk.upload_id)?
            .ok_or(StoreError::NotFound)?;
        if chunk.request_id.trim().is_empty()
            || chunk.request_id.len() > 128
            || chunk.sha256 != digest(&chunk.content)
        {
            return Err(StoreError::Invalid(
                "Resource upload chunk request or digest is invalid".to_owned(),
            ));
        }
        validate_upload_chunk(&session, &chunk)?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::ReserveResourceUploadBlob {
                workspace_id: session.workspace_id.clone(),
                upload_id: session.upload_id.clone(),
                request_id: chunk.request_id.clone(),
                chunk_index: chunk.chunk_index,
                digest: chunk.sha256.clone(),
                size_bytes: chunk.content.len() as u64,
                created_at: chunk.received_at.clone(),
                expires_at: session.expires_at.clone(),
                reply: reply_sender,
            },
            reply_receiver,
        )?;
        let blob = self.inner.blobs.put(
            &session.workspace_id,
            BlobPurpose::ResourceUploadChunk,
            &chunk.content,
            "application/octet-stream",
        )?;
        if blob.digest != chunk.sha256
            || self.inner.blobs.get(
                &session.workspace_id,
                BlobPurpose::ResourceUploadChunk,
                &blob,
            )? != chunk.content
        {
            return Err(StoreError::Integrity(
                "upload chunk blob verification failed".to_owned(),
            ));
        }
        let mut completion_event = None;
        let mut completion_state_ref = None;
        if let Some(mut completed) = predict_content_received_session(&session, &chunk)? {
            let expected_progress_version =
                session.progress_version.checked_add(1).ok_or_else(|| {
                    StoreError::Invalid("upload progress version overflow".to_owned())
                })?;
            completed.progress_version = expected_progress_version;
            let mut draft = chunk.lifecycle_event.clone();
            draft.workspace_id = session.workspace_id.clone();
            draft.entity_type = "ResourceUpload".to_owned();
            draft.entity_id = session.upload_id.clone();
            draft.entity_revision = completed.version;
            draft.event_type = "resource.upload.status.changed.v1".to_owned();
            draft.payload = json!({
                "upload_id": session.upload_id,
                "from": "OPEN",
                "to": "CONTENT_RECEIVED",
                "aggregate_version": completed.version
            });
            let state_bytes = canonical_json(&completed)?;
            let state_blob = self.inner.blobs.put(
                &session.workspace_id,
                BlobPurpose::AggregateState,
                &state_bytes,
                STATE_MEDIA_TYPE,
            )?;
            completion_state_ref = Some(AggregateStateRef {
                blob: state_blob,
                entity_revision: completed.version,
                record_schema_version: 1,
            });
            completion_event = Some(draft);
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::PutResourceUploadChunk {
                expected_progress_version: session.progress_version,
                chunk,
                blob,
                completion_event,
                completion_state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn commit(
        &self,
        mut request: WorkspaceCreateRequest,
        upload_id: &str,
        resource: ResourceRecord,
        mut revision: ResourceRevisionRecord,
        event: EventDraft,
        mut upload_status_event: Option<EventDraft>,
        mut upload_failure_event: Option<EventDraft>,
    ) -> Result<CommittedResourceUpload, StoreError> {
        let session = self
            .get(&resource.workspace_id, upload_id)?
            .ok_or(StoreError::NotFound)?;
        if session.state == ResourceUploadState::Committed {
            if let Some(committed) =
                self.committed_resource_for_upload(&request.principal_id, upload_id)?
            {
                return self.finish_upload_result(session, committed);
            }
            return Err(StoreError::LegacyUploadCommitNeedsReview {
                committed_resource_id: session.committed_resource_id,
            });
        }
        if session.state != ResourceUploadState::ContentReceived
            && session.state != ResourceUploadState::Committed
        {
            return Err(StoreError::Invalid(
                "Resource upload is not complete".to_owned(),
            ));
        }
        let mut content = Vec::with_capacity(session.expected_size_bytes as usize);
        if let Err(error) = self.append_upload_chunk_blobs(&session, &mut content) {
            if session.state == ResourceUploadState::ContentReceived
                && matches!(&error, StoreError::Integrity(_) | StoreError::NotFound)
            {
                let failure_event = upload_failure_event.take().ok_or_else(|| {
                    StoreError::Invalid("Resource upload failure event is required".to_owned())
                })?;
                self.fail_upload_integrity(&session, failure_event)?;
                if matches!(&error, StoreError::NotFound) {
                    return Err(StoreError::Integrity(
                        "stored upload content is unavailable".to_owned(),
                    ));
                }
            }
            return Err(error);
        }
        if content.len() as u64 != session.expected_size_bytes {
            let error =
                StoreError::Integrity("upload content size does not match declaration".to_owned());
            if session.state == ResourceUploadState::ContentReceived {
                let failure_event = upload_failure_event.take().ok_or_else(|| {
                    StoreError::Invalid("Resource upload failure event is required".to_owned())
                })?;
                self.fail_upload_integrity(&session, failure_event)?;
            }
            return Err(error);
        }
        let content_digest = digest(&content);
        if session.expected_digest.as_deref() != Some(content_digest.as_str()) {
            let error = StoreError::Integrity(
                "upload content digest does not match declaration".to_owned(),
            );
            if session.state == ResourceUploadState::ContentReceived {
                let failure_event = upload_failure_event.take().ok_or_else(|| {
                    StoreError::Invalid("Resource upload failure event is required".to_owned())
                })?;
                self.fail_upload_integrity(&session, failure_event)?;
            }
            return Err(error);
        }
        let folder_import_matches = match session.folder_import.as_ref() {
            Some(value) => {
                resource.provenance.get("folder_import")
                    == Some(&json!({"relative_path": value.relative_path.clone()}))
            }
            None => resource
                .provenance
                .as_object()
                .is_some_and(|provenance| !provenance.contains_key("folder_import")),
        };
        let revision_upload = session.resource_id.is_some();
        let expected_revision_event = if revision_upload {
            "resource.revision.created.v1"
        } else if session.folder_import.is_some() {
            "resource.created.v2"
        } else {
            "resource.created.v1"
        };
        let resource_version_matches = if revision_upload {
            session
                .expected_resource_version
                .is_some_and(|version| resource.version == version.saturating_add(1))
                && session.resource_id.as_deref() == Some(resource.resource_id.as_str())
                && revision.parent_revision_ids == session.parent_revision_ids
                && resource.current_revision_id.as_deref()
                    == Some(revision.resource_revision_id.as_str())
        } else {
            resource.version == 1
                && session.expected_resource_version.is_none()
                && session.parent_revision_ids.is_empty()
                && session.resource_id.is_none()
                && revision.parent_revision_ids.is_empty()
        };
        if resource.workspace_id != session.workspace_id
            || resource.display_name != session.display_name
            || !folder_import_matches
            || revision.media_type.as_deref() != Some(session.media_type.as_str())
            || revision.content_digest.as_deref() != Some(content_digest.as_str())
            || revision.size_bytes != Some(content.len() as u64)
            || !resource_version_matches
            || revision.resource_id != resource.resource_id
            || event.workspace_id != resource.workspace_id
            || event.entity_type != "Resource"
            || event.entity_id != resource.resource_id
            || event.entity_revision != resource.version
            || event.event_type != expected_revision_event
        {
            return Err(StoreError::Invalid(
                "upload commit metadata is inconsistent".to_owned(),
            ));
        }
        let upload_commit = if session.state == ResourceUploadState::ContentReceived {
            let status_event = upload_status_event.take().ok_or_else(|| {
                StoreError::Invalid("Resource upload commit event is required".to_owned())
            })?;
            let mut committed_session = session.clone();
            committed_session.state = ResourceUploadState::Committed;
            committed_session.committed_resource_id = Some(resource.resource_id.clone());
            committed_session.version = session.version.checked_add(1).ok_or_else(|| {
                StoreError::Invalid("upload lifecycle version overflow".to_owned())
            })?;
            let state_bytes = canonical_json(&committed_session)?;
            let state_blob = self.inner.blobs.put(
                &session.workspace_id,
                BlobPurpose::AggregateState,
                &state_bytes,
                STATE_MEDIA_TYPE,
            )?;
            let state_ref = AggregateStateRef {
                blob: state_blob,
                entity_revision: committed_session.version,
                record_schema_version: 1,
            };
            if status_event.workspace_id != session.workspace_id
                || status_event.entity_type != "ResourceUpload"
                || status_event.entity_id != session.upload_id
                || status_event.entity_revision != committed_session.version
                || status_event.event_type != "resource.upload.status.changed.v1"
                || status_event
                    .payload
                    .get("upload_id")
                    .and_then(Value::as_str)
                    != Some(session.upload_id.as_str())
                || status_event.payload.get("from").and_then(Value::as_str)
                    != Some("CONTENT_RECEIVED")
                || status_event.payload.get("to").and_then(Value::as_str) != Some("COMMITTED")
                || status_event
                    .payload
                    .get("resource_id")
                    .and_then(Value::as_str)
                    != Some(resource.resource_id.as_str())
                || status_event
                    .payload
                    .get("aggregate_version")
                    .and_then(Value::as_u64)
                    != Some(committed_session.version)
            {
                return Err(StoreError::Invalid(
                    "Resource upload commit event is invalid".to_owned(),
                ));
            }
            Some(UploadCommit {
                expected_version: session.version,
                expected_progress_version: session.progress_version,
                committed_session,
                status_event,
                state_ref,
            })
        } else {
            if upload_status_event.is_some() || upload_failure_event.is_some() {
                return Err(StoreError::Invalid(
                    "committed upload cannot emit a second lifecycle event".to_owned(),
                ));
            }
            None
        };
        revision.content_digest = Some(content_digest.clone());

        request.request_id = format!("resource-upload-commit:{}", upload_id);
        request.request_payload = if revision_upload {
            json!({"operation":"resource.revision.upload.commit.v1", "upload_id":upload_id, "resource_id":resource.resource_id, "expected_resource_version":session.expected_resource_version, "parent_revision_ids":session.parent_revision_ids})
        } else {
            json!({"operation":"resource.upload.commit.v1", "upload_id":upload_id})
        };
        let state_bytes = canonical_json(&resource)?;
        let state_blob = self.inner.blobs.put(
            &resource.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            "application/vnd.litecow.resource+json",
        )?;
        let content_blob = self.inner.blobs.put(
            &resource.workspace_id,
            BlobPurpose::Resource,
            &content,
            &session.media_type,
        )?;
        if content_blob.digest != content_digest
            || self
                .inner
                .blobs
                .get(&resource.workspace_id, BlobPurpose::Resource, &content_blob)?
                != content
        {
            return Err(StoreError::Integrity(
                "Resource content failed blob verification".to_owned(),
            ));
        }
        let media_type = revision
            .media_type
            .as_deref()
            .unwrap_or("application/octet-stream");
        let text_index =
            if resource_index::not_indexable_reason(&resource.display_name, media_type, &content)
                .is_some()
            {
                None
            } else {
                self.ensure_resource_index_key_history_available(&resource.workspace_id)?;
                resource_index::prepare(
                    self.inner.blobs.as_ref(),
                    &resource.workspace_id,
                    &resource.resource_id,
                    &revision.resource_revision_id,
                    &resource.display_name,
                    media_type,
                    &content_digest,
                    &revision.observed_at,
                    &content,
                )?
            };
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: resource.version,
            record_schema_version: 1,
        };
        let location_id = format!("loc_{}", resource.resource_id);
        let (reply_sender, reply_receiver) = mpsc::channel();
        let committed = self.execute_command(
            Command::CommitResourceUpload {
                request,
                resource,
                revision,
                draft: event,
                state_ref,
                upload_commit,
                content_blob,
                text_index,
                location_id,
                context_document: session.context_document.clone(),
                reply: reply_sender,
            },
            reply_receiver,
        )?;
        let mut committed_session = self
            .get(&committed.resource.workspace_id, upload_id)?
            .ok_or(StoreError::NotFound)?;
        self.finish_upload_result(committed_session, committed)
    }
}

impl SqliteWorkspaceStore {
    fn get_for_chunk(
        &self,
        upload_id: &str,
    ) -> Result<Option<ResourceUploadSessionRecord>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetResourceUpload {
                workspace_id: String::new(),
                upload_id: upload_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn append_upload_chunk_blobs(
        &self,
        session: &ResourceUploadSessionRecord,
        content: &mut Vec<u8>,
    ) -> Result<(), StoreError> {
        let chunks = self.upload_chunk_refs(&session.workspace_id, &session.upload_id)?;
        for (index, blob, expected_digest, size) in chunks {
            let bytes = self.inner.blobs.get(
                &session.workspace_id,
                BlobPurpose::ResourceUploadChunk,
                &blob,
            )?;
            if blob.size_bytes != size
                || bytes.len() as u64 != size
                || digest(&bytes) != expected_digest
            {
                return Err(StoreError::Integrity(
                    "stored upload chunk failed verification".to_owned(),
                ));
            }
            let expected_offset = index.checked_mul(session.chunk_size_bytes).ok_or_else(|| {
                StoreError::Integrity("stored upload chunk index overflows".to_owned())
            })?;
            let range = session
                .received_ranges
                .iter()
                .find(|range| range.start_offset == expected_offset)
                .ok_or_else(|| {
                    StoreError::Integrity("stored upload chunk range is missing".to_owned())
                })?;
            if range.sha256 != expected_digest
                || range.end_offset_inclusive.saturating_add(1) != expected_offset + size
            {
                return Err(StoreError::Integrity(
                    "stored upload chunk does not match its indexed range".to_owned(),
                ));
            }
            content.extend_from_slice(&bytes);
        }
        Ok(())
    }

    fn upload_chunk_refs(
        &self,
        workspace_id: &str,
        upload_id: &str,
    ) -> Result<Vec<(u64, BlobRef, String, u64)>, StoreError> {
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::GetResourceUploadChunks {
                workspace_id: workspace_id.to_owned(),
                upload_id: upload_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }

    fn finish_upload_result(
        &self,
        session: ResourceUploadSessionRecord,
        committed: CommittedResource,
    ) -> Result<CommittedResourceUpload, StoreError> {
        Ok(CommittedResourceUpload {
            session,
            resource: committed.resource,
            revision: committed.revision,
            event: committed.event,
        })
    }

    fn fail_upload_integrity(
        &self,
        session: &ResourceUploadSessionRecord,
        event: EventDraft,
    ) -> Result<ResourceUploadSessionRecord, StoreError> {
        let mut failed = session.clone();
        failed.state = ResourceUploadState::Failed;
        failed.version = session
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Invalid("upload lifecycle version overflow".to_owned()))?;
        let state_bytes = canonical_json(&failed)?;
        let state_blob = self.inner.blobs.put(
            &failed.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            STATE_MEDIA_TYPE,
        )?;
        let state_ref = AggregateStateRef {
            blob: state_blob,
            entity_revision: failed.version,
            record_schema_version: 1,
        };
        if event.workspace_id != failed.workspace_id
            || event.entity_type != "ResourceUpload"
            || event.entity_id != failed.upload_id
            || event.entity_revision != failed.version
            || event.event_type != "resource.upload.status.changed.v1"
            || event.payload.get("upload_id").and_then(Value::as_str)
                != Some(failed.upload_id.as_str())
            || event.payload.get("from").and_then(Value::as_str) != Some("CONTENT_RECEIVED")
            || event.payload.get("to").and_then(Value::as_str) != Some("FAILED")
            || event.payload.get("reason_code").and_then(Value::as_str)
                != Some("UPLOAD_CONTENT_INTEGRITY_FAILED")
            || event
                .payload
                .get("aggregate_version")
                .and_then(Value::as_u64)
                != Some(failed.version)
        {
            return Err(StoreError::Invalid(
                "Resource upload failure event is invalid".to_owned(),
            ));
        }
        let (reply_sender, reply_receiver) = mpsc::channel();
        self.execute_command(
            Command::FailResourceUpload {
                expected_version: session.version,
                expected_progress_version: session.progress_version,
                failed,
                draft: event,
                state_ref,
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}

fn validate_upload_chunk(
    session: &ResourceUploadSessionRecord,
    chunk: &ResourceUploadChunkInput,
) -> Result<(), StoreError> {
    validate_upload_chunk_geometry(session, chunk)?;
    if session.expected_digest.is_none()
        || session.expected_size_bytes > 104_857_600
        || session.chunk_size_bytes == 0
        || session.chunk_size_bytes > 4_194_304
        || !matches!(
            session.state,
            ResourceUploadState::Open
                | ResourceUploadState::ContentReceived
                | ResourceUploadState::Committed
        )
        || chunk.received_at >= session.expires_at
    {
        return Err(StoreError::Invalid(
            "Resource upload session is not resumable or has expired".to_owned(),
        ));
    }
    Ok(())
}

fn validate_upload_chunk_geometry(
    session: &ResourceUploadSessionRecord,
    chunk: &ResourceUploadChunkInput,
) -> Result<(), StoreError> {
    let expected_start = chunk
        .chunk_index
        .checked_mul(session.chunk_size_bytes)
        .ok_or_else(|| StoreError::Invalid("chunk index is out of range".to_owned()))?;
    let expected_end_exclusive = expected_start
        .saturating_add(session.chunk_size_bytes)
        .min(session.expected_size_bytes);
    if chunk.upload_id != session.upload_id
        || chunk.content.is_empty()
        || chunk.content_range.total_size_bytes != session.expected_size_bytes
        || chunk.content_range.start_offset != expected_start
        || expected_end_exclusive <= expected_start
        || chunk.content_range.end_offset_inclusive.checked_add(1) != Some(expected_end_exclusive)
        || chunk.content.len() as u64 != expected_end_exclusive - expected_start
        || chunk.sha256 != digest(&chunk.content)
    {
        return Err(StoreError::Invalid(
            "Resource upload chunk range or digest is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn predict_content_received_session(
    session: &ResourceUploadSessionRecord,
    chunk: &ResourceUploadChunkInput,
) -> Result<Option<ResourceUploadSessionRecord>, StoreError> {
    if session.state != ResourceUploadState::Open
        || session.received_ranges.iter().any(|range| {
            range.start_offset == chunk.content_range.start_offset
                && range.end_offset_inclusive == chunk.content_range.end_offset_inclusive
                && range.sha256 == chunk.sha256
        })
    {
        return Ok(None);
    }
    let mut completed = session.clone();
    completed
        .received_ranges
        .push(storage_core::ResourceUploadRange {
            start_offset: chunk.content_range.start_offset,
            end_offset_inclusive: chunk.content_range.end_offset_inclusive,
            sha256: chunk.sha256.clone(),
        });
    completed
        .received_ranges
        .sort_by_key(|range| range.start_offset);
    let mut next_missing_offset = 0_u64;
    for range in &completed.received_ranges {
        if range.start_offset != next_missing_offset {
            break;
        }
        next_missing_offset = range.end_offset_inclusive.saturating_add(1);
    }
    if next_missing_offset != session.expected_size_bytes {
        return Ok(None);
    }
    completed.state = ResourceUploadState::ContentReceived;
    completed.version = session
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("upload lifecycle version overflow".to_owned()))?;
    completed.next_missing_offset = next_missing_offset;
    Ok(Some(completed))
}

fn create_resource_upload_transaction(
    connection: &mut Connection,
    request: WorkspaceCreateRequest,
    session: ResourceUploadSessionRecord,
    draft: EventDraft,
    state_ref: AggregateStateRef,
) -> Result<ResourceUploadSessionRecord, StoreError> {
    if request.principal_id.trim().is_empty()
        || request.request_id.trim().is_empty()
        || session.upload_id.trim().is_empty()
        || session.workspace_id.trim().is_empty()
        || session.display_name.trim().is_empty()
        || !session.expected_digest.as_deref().is_some_and(|value| {
            value.len() == 71
                && value.starts_with("sha256:")
                && value[7..]
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
    {
        return Err(StoreError::Invalid("upload identity is invalid".to_owned()));
    }
    if let Some(resource_id) = session.resource_id.as_deref() {
        if request
            .request_payload
            .get("operation")
            .and_then(Value::as_str)
            != Some("resource.revision.upload.create.v1")
            || request
                .request_payload
                .get("workspace_id")
                .and_then(Value::as_str)
                != Some(session.workspace_id.as_str())
            || request
                .request_payload
                .get("resource_id")
                .and_then(Value::as_str)
                != Some(resource_id)
            || request
                .request_payload
                .get("expected_resource_version")
                .and_then(Value::as_u64)
                != session.expected_resource_version
            || request.request_payload.get("parent_revision_ids").cloned()
                != Some(json!(session.parent_revision_ids))
            || request
                .request_payload
                .get("media_type")
                .and_then(Value::as_str)
                != Some(session.media_type.as_str())
            || request
                .request_payload
                .get("size_bytes")
                .and_then(Value::as_u64)
                != Some(session.expected_size_bytes)
            || request
                .request_payload
                .get("expected_digest")
                .and_then(Value::as_str)
                != session.expected_digest.as_deref()
        {
            return Err(StoreError::Invalid(
                "revision upload idempotency payload does not match its pinned session".to_owned(),
            ));
        }
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let request_digest = digest(&canonical_json(&request.request_payload)?);
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![request.principal_id, request.request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((prior_digest, response, response_digest)) = prior {
        if prior_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response = response.ok_or_else(|| {
            StoreError::Integrity("upload idempotency receipt is incomplete".to_owned())
        })?;
        if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "upload idempotency receipt digest is invalid".to_owned(),
            ));
        }
        return serde_json::from_str(&response)
            .map_err(|error| StoreError::Integrity(error.to_string()));
    }
    let active: Option<String> = tx
        .query_row(
            "SELECT status FROM workspaces WHERE workspace_id = ?1",
            [&session.workspace_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    if active.as_deref() != Some("ACTIVE") {
        return Err(StoreError::NotFound);
    }
    if let (Some(resource_id), Some(expected_version)) = (
        session.resource_id.as_deref(),
        session.expected_resource_version,
    ) {
        let current: Option<(i64, Option<String>)> = tx.query_row(
            "SELECT version, json_extract(context_document_json, '$.status') FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
            params![session.workspace_id, resource_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional().map_err(map_database_error)?;
        let Some((actual_version, context_status)) = current else {
            return Err(StoreError::NotFound);
        };
        let actual_version = from_sql_i64(actual_version, "Resource version")?;
        if actual_version != expected_version {
            return Err(StoreError::Conflict {
                expected: Some(expected_version),
                actual: Some(actual_version),
            });
        }
        if context_status
            .as_deref()
            .is_some_and(|status| status != "ACTIVE")
        {
            return Err(StoreError::Invalid(
                "ContextDocument is not active".to_owned(),
            ));
        }
        let mut statement = tx.prepare(
            "SELECT candidate.resource_revision_id FROM resource_revisions candidate
             WHERE candidate.resource_id = ?1 AND NOT EXISTS (
               SELECT 1 FROM resource_revision_parents edge
               WHERE edge.resource_id = candidate.resource_id AND edge.parent_revision_id = candidate.resource_revision_id
             ) ORDER BY candidate.resource_revision_id"
        ).map_err(map_database_error)?;
        let heads = statement
            .query_map([resource_id], |row| row.get::<_, String>(0))
            .map_err(map_database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?;
        let mut parents = session.parent_revision_ids.clone();
        parents.sort();
        if parents != heads {
            return Err(StoreError::Conflict {
                expected: Some(expected_version),
                actual: Some(actual_version),
            });
        }
    }
    let state = if session.expected_size_bytes == 0 {
        "CONTENT_RECEIVED"
    } else {
        "OPEN"
    };
    if session
        .folder_import
        .as_ref()
        .is_some_and(|origin| origin.relative_path != session.display_name)
    {
        return Err(StoreError::Invalid(
            "folder import path does not match Resource display name".to_owned(),
        ));
    }
    let folder_import_json = session
        .folder_import
        .as_ref()
        .map(canonical_json)
        .transpose()?
        .map(String::from_utf8)
        .transpose()
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let parent_ids_json = String::from_utf8(canonical_json(&session.parent_revision_ids)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO resource_upload_sessions(upload_id, workspace_id, display_name, media_type, expected_size_bytes, expected_digest, context_document_json, folder_import_json, chunk_size_bytes, state, expires_at, resource_id, committed_resource_id, expected_resource_version, parent_revision_ids_json, created_at, version, progress_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, ?13, ?14, ?15, 1, 1)",
        params![session.upload_id, session.workspace_id, session.display_name, session.media_type, to_sql_i64(session.expected_size_bytes, "upload size")?, session.expected_digest, session.context_document.as_ref().map(canonical_json).transpose()?.map(String::from_utf8).transpose().map_err(|error| StoreError::Invalid(error.to_string()))?, folder_import_json, to_sql_i64(session.chunk_size_bytes, "chunk size")?, state, session.expires_at, session.resource_id, session.expected_resource_version.map(|value| to_sql_i64(value, "Resource version")).transpose()?, parent_ids_json, session.created_at],
    ).map_err(map_database_error)?;
    let mut created = session;
    created.state = if created.expected_size_bytes == 0 {
        ResourceUploadState::ContentReceived
    } else {
        ResourceUploadState::Open
    };
    if draft.workspace_id != created.workspace_id
        || draft.entity_type != "ResourceUpload"
        || draft.entity_id != created.upload_id
        || draft.entity_revision != created.version
        || draft.event_type != "resource.upload.created.v1"
        || draft.payload.get("upload_id").and_then(Value::as_str)
            != Some(created.upload_id.as_str())
        || draft.payload.get("workspace_id").and_then(Value::as_str)
            != Some(created.workspace_id.as_str())
        || draft
            .payload
            .get("expected_size_bytes")
            .and_then(Value::as_u64)
            != Some(created.expected_size_bytes)
        || draft
            .payload
            .get("chunk_size_bytes")
            .and_then(Value::as_u64)
            != Some(created.chunk_size_bytes)
        || draft.payload.get("expires_at").and_then(Value::as_str)
            != Some(created.expires_at.as_str())
        || draft
            .payload
            .get("aggregate_version")
            .and_then(Value::as_u64)
            != Some(created.version)
        || state_ref.entity_revision != created.version
    {
        return Err(StoreError::Invalid(
            "Resource upload creation event does not match its session".to_owned(),
        ));
    }
    if created.expected_digest.as_deref()
        != draft.payload.get("expected_digest").and_then(Value::as_str)
    {
        return Err(StoreError::Invalid(
            "Resource upload creation digest does not match".to_owned(),
        ));
    }
    if draft.payload.get("resource_id").and_then(Value::as_str) != created.resource_id.as_deref()
        || draft
            .payload
            .get("expected_resource_version")
            .and_then(Value::as_u64)
            != created.expected_resource_version
        || draft
            .payload
            .get("parent_revision_ids")
            .cloned()
            .unwrap_or_else(|| json!([]))
            != json!(created.parent_revision_ids)
    {
        return Err(StoreError::Invalid(
            "Resource upload revision pin does not match its session".to_owned(),
        ));
    }
    insert_upload_domain_event(&tx, draft, state_ref)?;
    let response = String::from_utf8(canonical_json(&created)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![request.principal_id, request.request_id, request_digest, response, digest(response.as_bytes()), created.created_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(created)
}

fn load_resource_upload(
    connection: &Connection,
    workspace_id: &str,
    upload_id: &str,
) -> Result<Option<ResourceUploadSessionRecord>, StoreError> {
    let row: Option<(String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, String, String, String, String, i64, i64, i64)> = connection.query_row(
        "SELECT workspace_id, display_name, media_type, expected_size_bytes, expected_digest, context_document_json, folder_import_json, resource_id, committed_resource_id, expected_resource_version, parent_revision_ids_json, state, expires_at, created_at, version, progress_version, chunk_size_bytes FROM resource_upload_sessions WHERE upload_id = ?1 AND (?2 = '' OR workspace_id = ?2)",
        params![upload_id, workspace_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?, row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?, row.get(16)?)),
    ).optional().map_err(map_database_error)?;
    let Some((
        workspace_id,
        display_name,
        media_type,
        expected_size,
        expected_digest,
        context_document,
        folder_import,
        resource_id,
        committed_resource_id,
        expected_resource_version,
        parents,
        state,
        expires_at,
        created_at,
        version,
        progress_version,
        chunk_size,
    )) = row
    else {
        return Ok(None);
    };
    let chunks = load_resource_upload_ranges(connection, upload_id)?;
    let size = from_sql_i64(expected_size, "upload size")?;
    let mut next_missing_offset = 0_u64;
    for range in &chunks {
        if range.start_offset != next_missing_offset {
            break;
        }
        next_missing_offset = range.end_offset_inclusive.saturating_add(1);
    }
    let state = match state.as_str() {
        "OPEN" => ResourceUploadState::Open,
        "CONTENT_RECEIVED" => ResourceUploadState::ContentReceived,
        "COMMITTED" => ResourceUploadState::Committed,
        "FAILED" => ResourceUploadState::Failed,
        "EXPIRED" => ResourceUploadState::Expired,
        _ => {
            return Err(StoreError::CorruptSchema(
                "unknown Resource upload state".to_owned(),
            ));
        }
    };
    Ok(Some(ResourceUploadSessionRecord {
        upload_id: upload_id.to_owned(),
        workspace_id,
        display_name,
        media_type,
        expected_size_bytes: size,
        expected_digest,
        context_document: context_document
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|error| StoreError::CorruptSchema(error.to_string()))?,
        folder_import: folder_import
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|error| StoreError::CorruptSchema(error.to_string()))?,
        resource_id,
        committed_resource_id,
        expected_resource_version: expected_resource_version
            .map(|value| from_sql_i64(value, "Resource version"))
            .transpose()?,
        parent_revision_ids: serde_json::from_str(&parents)
            .map_err(|error| StoreError::CorruptSchema(error.to_string()))?,
        chunk_size_bytes: from_sql_i64(chunk_size, "upload chunk size")?,
        received_ranges: chunks,
        next_missing_offset,
        state,
        expires_at,
        created_at,
        version: from_sql_i64(version, "upload lifecycle version")?,
        progress_version: from_sql_i64(progress_version, "upload progress version")?,
    }))
}

fn load_resource_upload_commit_receipt(
    connection: &Connection,
    principal_id: &str,
    upload_id: &str,
) -> Result<Option<CommittedResource>, StoreError> {
    let request_id = format!("resource-upload-commit:{upload_id}");
    let row: Option<(Option<String>, Option<String>)> = connection.query_row(
        "SELECT response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![principal_id, request_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(map_database_error)?;
    let Some((response, response_digest)) = row else {
        return Ok(None);
    };
    let response = response.ok_or_else(|| {
        StoreError::Integrity("Resource upload commit receipt is incomplete".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "Resource upload commit receipt digest is invalid".to_owned(),
        ));
    }
    serde_json::from_str(&response)
        .map(Some)
        .map_err(|error| StoreError::Integrity(error.to_string()))
}

fn list_expired_resource_uploads(
    connection: &Connection,
    now: &str,
    limit: usize,
) -> Result<Vec<ResourceUploadSessionRecord>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT upload_id FROM resource_upload_sessions WHERE state IN ('OPEN', 'CONTENT_RECEIVED') AND expires_at <= ?1 ORDER BY expires_at, upload_id LIMIT ?2",
    ).map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![now, to_sql_i64(limit as u64, "upload page size")?],
            |row| row.get::<_, String>(0),
        )
        .map_err(map_database_error)?;
    let mut sessions = Vec::new();
    for row in rows {
        let upload_id = row.map_err(map_database_error)?;
        if let Some(session) = load_resource_upload(connection, "", &upload_id)? {
            sessions.push(session);
        }
    }
    Ok(sessions)
}

fn insert_upload_domain_event(
    transaction: &Transaction<'_>,
    draft: EventDraft,
    state_ref: AggregateStateRef,
) -> Result<DomainEvent, StoreError> {
    if draft.workspace_id.trim().is_empty()
        || draft.event_id.trim().is_empty()
        || draft.origin_runtime_id.trim().is_empty()
        || draft.entity_revision != state_ref.entity_revision
    {
        return Err(StoreError::Invalid(
            "upload lifecycle event is invalid".to_owned(),
        ));
    }
    transaction.execute(
        "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1",
        params![draft.workspace_id, draft.origin_runtime_id],
    ).map_err(map_database_error)?;
    let origin_sequence: i64 = transaction.query_row(
        "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
        params![draft.workspace_id, draft.origin_runtime_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    let payload_json = String::from_utf8(canonical_json(&draft.payload)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let event = DomainEvent {
        event_id: draft.event_id,
        workspace_id: draft.workspace_id,
        entity_type: draft.entity_type,
        entity_id: draft.entity_id,
        origin_runtime_id: draft.origin_runtime_id,
        origin_sequence: from_sql_i64(origin_sequence, "origin sequence")?,
        entity_revision: draft.entity_revision,
        hlc_timestamp: draft.hlc_timestamp,
        correlation_id: draft.correlation_id,
        causation_id: draft.causation_id,
        schema_version: draft.schema_version,
        event_type: draft.event_type,
        payload: draft.payload,
        aggregate_state_ref: state_ref,
        recorded_at: draft.recorded_at,
        payload_digest: digest(payload_json.as_bytes()),
    };
    transaction.execute(
        "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![event.event_id, event.workspace_id, event.entity_type, event.entity_id, event.origin_runtime_id, to_sql_i64(event.origin_sequence, "origin sequence")?, to_sql_i64(event.entity_revision, "event revision")?, event.hlc_timestamp, event.correlation_id, event.causation_id, i64::from(event.schema_version), event.event_type, payload_json, String::from_utf8(canonical_json(&event.aggregate_state_ref)?).map_err(|error| StoreError::Invalid(error.to_string()))?, event.recorded_at, event.payload_digest],
    ).map_err(map_database_error)?;
    Ok(event)
}

fn expire_resource_upload_transaction(
    connection: &mut Connection,
    expected_progress_version: u64,
    expired: ResourceUploadSessionRecord,
    draft: EventDraft,
    state_ref: AggregateStateRef,
) -> Result<ResourceUploadSessionRecord, StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let current = load_resource_upload(&transaction, &expired.workspace_id, &expired.upload_id)?
        .ok_or(StoreError::NotFound)?;
    if matches!(
        current.state,
        ResourceUploadState::Expired | ResourceUploadState::Committed | ResourceUploadState::Failed
    ) {
        transaction.commit().map_err(map_database_error)?;
        return Ok(current);
    }
    if current.progress_version != expected_progress_version
        || !matches!(
            current.state,
            ResourceUploadState::Open | ResourceUploadState::ContentReceived
        )
        || expired.version != current.version.saturating_add(1)
        || expired.progress_version != current.progress_version
        || expired.state != ResourceUploadState::Expired
        || current.workspace_id != expired.workspace_id
        || current.display_name != expired.display_name
        || current.media_type != expired.media_type
        || current.expected_size_bytes != expired.expected_size_bytes
        || current.expected_digest != expired.expected_digest
        || current.context_document != expired.context_document
        || current.folder_import != expired.folder_import
        || current.expires_at != expired.expires_at
        || current.received_ranges != expired.received_ranges
        || current.expires_at > draft.recorded_at
        || draft.entity_type != "ResourceUpload"
        || draft.entity_id != expired.upload_id
        || draft.workspace_id != expired.workspace_id
        || draft.entity_revision != expired.version
        || draft.event_type != "resource.upload.status.changed.v1"
        || draft.payload.get("upload_id").and_then(Value::as_str)
            != Some(expired.upload_id.as_str())
        || draft.payload.get("from").and_then(Value::as_str)
            != Some(match current.state {
                ResourceUploadState::Open => "OPEN",
                ResourceUploadState::ContentReceived => "CONTENT_RECEIVED",
                _ => "",
            })
        || draft.payload.get("to").and_then(Value::as_str) != Some("EXPIRED")
        || draft
            .payload
            .get("aggregate_version")
            .and_then(Value::as_u64)
            != Some(expired.version)
        || state_ref.entity_revision != expired.version
    {
        return Err(StoreError::Conflict {
            expected: Some(expected_progress_version),
            actual: Some(current.progress_version),
        });
    }
    let changed = transaction.execute(
        "UPDATE resource_upload_sessions SET state = 'EXPIRED', version = version + 1 WHERE upload_id = ?1 AND workspace_id = ?2 AND progress_version = ?3 AND version = ?4 AND state IN ('OPEN', 'CONTENT_RECEIVED')",
        params![expired.upload_id, expired.workspace_id, to_sql_i64(expected_progress_version, "expected upload progress version")?, to_sql_i64(current.version, "upload lifecycle version")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(expected_progress_version),
            actual: None,
        });
    }
    insert_upload_domain_event(&transaction, draft, state_ref)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(expired)
}

fn fail_resource_upload_transaction(
    connection: &mut Connection,
    expected_version: u64,
    expected_progress_version: u64,
    failed: ResourceUploadSessionRecord,
    draft: EventDraft,
    state_ref: AggregateStateRef,
) -> Result<ResourceUploadSessionRecord, StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let current = load_resource_upload(&transaction, &failed.workspace_id, &failed.upload_id)?
        .ok_or(StoreError::NotFound)?;
    if current.state != ResourceUploadState::ContentReceived
        || current.version != expected_version
        || current.progress_version != expected_progress_version
    {
        return Err(StoreError::Conflict {
            expected: Some(expected_progress_version),
            actual: Some(current.progress_version),
        });
    }
    let mut expected_failed = current.clone();
    expected_failed.state = ResourceUploadState::Failed;
    expected_failed.version = current
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("upload lifecycle version overflow".to_owned()))?;
    if failed != expected_failed
        || draft.workspace_id != failed.workspace_id
        || draft.entity_type != "ResourceUpload"
        || draft.entity_id != failed.upload_id
        || draft.entity_revision != failed.version
        || draft.event_type != "resource.upload.status.changed.v1"
        || draft.payload.get("upload_id").and_then(Value::as_str) != Some(failed.upload_id.as_str())
        || draft.payload.get("from").and_then(Value::as_str) != Some("CONTENT_RECEIVED")
        || draft.payload.get("to").and_then(Value::as_str) != Some("FAILED")
        || draft.payload.get("reason_code").and_then(Value::as_str)
            != Some("UPLOAD_CONTENT_INTEGRITY_FAILED")
        || draft
            .payload
            .get("aggregate_version")
            .and_then(Value::as_u64)
            != Some(failed.version)
        || state_ref.entity_revision != failed.version
        || state_ref.record_schema_version != 1
    {
        return Err(StoreError::Invalid(
            "Resource upload failure transition is inconsistent".to_owned(),
        ));
    }
    let changed = transaction.execute(
        "UPDATE resource_upload_sessions SET state = 'FAILED', version = ?1 WHERE upload_id = ?2 AND workspace_id = ?3 AND state = 'CONTENT_RECEIVED' AND version = ?4 AND progress_version = ?5",
        params![to_sql_i64(failed.version, "upload lifecycle version")?, failed.upload_id, failed.workspace_id, to_sql_i64(expected_version, "expected upload lifecycle version")?, to_sql_i64(expected_progress_version, "expected upload progress version")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(expected_progress_version),
            actual: None,
        });
    }
    let persisted = load_resource_upload(&transaction, &failed.workspace_id, &failed.upload_id)?
        .ok_or(StoreError::NotFound)?;
    if persisted != failed {
        return Err(StoreError::Integrity(
            "failed upload state does not match its aggregate snapshot".to_owned(),
        ));
    }
    insert_upload_domain_event(&transaction, draft, state_ref)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(failed)
}

fn reserve_resource_upload_blob_transaction(
    connection: &mut Connection,
    workspace_id: &str,
    upload_id: &str,
    request_id: &str,
    chunk_index: u64,
    chunk_digest: &str,
    size_bytes: u64,
    created_at: &str,
    expires_at: &str,
) -> Result<(), StoreError> {
    if request_id.trim().is_empty()
        || request_id.len() > 128
        || chunk_digest.len() != 71
        || !chunk_digest.starts_with("sha256:")
        || !chunk_digest[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || size_bytes == 0
        || size_bytes > 4_194_304
    {
        return Err(StoreError::Invalid(
            "upload blob reservation is invalid".to_owned(),
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let session =
        load_resource_upload(&transaction, workspace_id, upload_id)?.ok_or(StoreError::NotFound)?;
    let expected_start = chunk_index
        .checked_mul(session.chunk_size_bytes)
        .ok_or_else(|| StoreError::Invalid("chunk index is out of range".to_owned()))?;
    let expected_size = session
        .expected_size_bytes
        .saturating_sub(expected_start)
        .min(session.chunk_size_bytes);
    if expected_size == 0 || size_bytes != expected_size || session.chunk_size_bytes != 4_194_304 {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let expected_end = expected_start
        .saturating_add(expected_size)
        .saturating_sub(1);
    let request_payload_digest = digest(&canonical_json(&json!({
        "upload_id": upload_id,
        "chunk_index": chunk_index,
        "start_offset": expected_start,
        "end_offset_inclusive": expected_end,
        "total_size_bytes": session.expected_size_bytes,
        "sha256": chunk_digest,
    }))?);
    let prior_request: Option<String> = transaction.query_row(
        "SELECT request_payload_digest FROM resource_upload_chunk_requests WHERE upload_id = ?1 AND request_id = ?2",
        params![upload_id, request_id],
        |row| row.get(0),
    ).optional().map_err(map_database_error)?;
    if let Some(prior_digest) = prior_request {
        if prior_digest != request_payload_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        transaction.commit().map_err(map_database_error)?;
        return Ok(());
    }

    let existing_chunk: Option<(i64, i64, String)> = transaction.query_row(
        "SELECT start_offset, end_offset_exclusive, sha256 FROM resource_upload_chunks WHERE upload_id = ?1 AND chunk_index = ?2",
        params![upload_id, to_sql_i64(chunk_index, "chunk index")?],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((start, end, existing_digest)) = existing_chunk {
        if from_sql_i64(start, "chunk start")? == expected_start
            && from_sql_i64(end, "chunk end")? == expected_end.saturating_add(1)
            && existing_digest == chunk_digest
            && matches!(
                session.state,
                ResourceUploadState::Open
                    | ResourceUploadState::ContentReceived
                    | ResourceUploadState::Committed
            )
            && created_at < session.expires_at.as_str()
        {
            transaction.commit().map_err(map_database_error)?;
            return Ok(());
        }
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }

    if session.state != ResourceUploadState::Open
        || session.expires_at != expires_at
        || session.chunk_size_bytes != 4_194_304
        || size_bytes != expected_size
        || created_at >= session.expires_at.as_str()
    {
        return Err(StoreError::Conflict {
            expected: Some(session.progress_version),
            actual: Some(session.progress_version),
        });
    }
    let gc_fenced: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM resource_upload_blob_gc_fences WHERE workspace_id = ?1 AND digest = ?2)",
        params![workspace_id, chunk_digest],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if gc_fenced {
        return Err(StoreError::Busy);
    }
    let existing: Option<(i64, String, i64, String)> = transaction.query_row(
        "SELECT chunk_index, digest, size_bytes, state FROM resource_upload_blob_reservations WHERE upload_id = ?1 AND request_id = ?2",
        params![upload_id, request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((prior_index, prior_digest, prior_size, prior_state)) = existing {
        if from_sql_i64(prior_index, "reserved chunk index")? != chunk_index
            || prior_digest != chunk_digest
            || from_sql_i64(prior_size, "reserved chunk size")? != size_bytes
            || prior_state != "RESERVED"
        {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        transaction.execute(
            "UPDATE resource_upload_blob_reservations SET created_at = ?1, expires_at = ?2 WHERE upload_id = ?3 AND request_id = ?4 AND state = 'RESERVED'",
            params![created_at, expires_at, upload_id, request_id],
        ).map_err(map_database_error)?;
    } else {
        transaction.execute(
            "INSERT INTO resource_upload_blob_reservations(upload_id, request_id, workspace_id, chunk_index, digest, size_bytes, created_at, expires_at, state) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'RESERVED')",
            params![upload_id, request_id, workspace_id, to_sql_i64(chunk_index, "reserved chunk index")?, chunk_digest, to_sql_i64(size_bytes, "reserved chunk size")?, created_at, expires_at],
        ).map_err(map_database_error)?;
    }
    transaction.commit().map_err(map_database_error)
}

fn claim_orphan_resource_upload_blobs_transaction(
    connection: &mut Connection,
    now: &str,
    limit: usize,
) -> Result<Vec<(String, BlobRef)>, StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let mut statement = transaction.prepare(
        "SELECT r.workspace_id, r.digest, MAX(r.size_bytes) FROM resource_upload_blob_reservations r WHERE (r.state = 'DELETING' OR (r.state = 'RESERVED' AND r.expires_at <= ?1)) AND NOT EXISTS (SELECT 1 FROM resource_upload_chunks c JOIN resource_upload_sessions s ON s.upload_id = c.upload_id WHERE s.workspace_id = r.workspace_id AND c.temporary_blob_ref = r.digest) AND NOT EXISTS (SELECT 1 FROM resource_upload_blob_reservations active WHERE active.workspace_id = r.workspace_id AND active.digest = r.digest AND active.state = 'RESERVED' AND active.expires_at > ?1) GROUP BY r.workspace_id, r.digest ORDER BY MIN(r.created_at), r.workspace_id, r.digest LIMIT ?2",
    ).map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![now, to_sql_i64(limit as u64, "orphan blob sweep limit")?],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(map_database_error)?;
    let mut candidates = Vec::new();
    for row in rows {
        let (workspace_id, digest, size_bytes) = row.map_err(map_database_error)?;
        candidates.push((
            workspace_id,
            digest,
            from_sql_i64(size_bytes, "orphan blob size")?,
        ));
    }
    drop(statement);
    let mut claimed = Vec::with_capacity(candidates.len());
    for (workspace_id, digest, size_bytes) in candidates {
        transaction.execute(
            "INSERT INTO resource_upload_blob_gc_fences(workspace_id, digest, size_bytes, claimed_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(workspace_id, digest) DO NOTHING",
            params![workspace_id, digest, to_sql_i64(size_bytes, "orphan blob size")?, now],
        ).map_err(map_database_error)?;
        transaction.execute(
            "UPDATE resource_upload_blob_reservations SET state = 'DELETING' WHERE workspace_id = ?1 AND digest = ?2 AND (state = 'DELETING' OR expires_at <= ?3)",
            params![workspace_id, digest, now],
        ).map_err(map_database_error)?;
        let still_referenced: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM resource_upload_chunks c JOIN resource_upload_sessions s ON s.upload_id = c.upload_id WHERE s.workspace_id = ?1 AND c.temporary_blob_ref = ?2)",
            params![workspace_id, digest],
            |row| row.get(0),
        ).map_err(map_database_error)?;
        if still_referenced {
            return Err(StoreError::Integrity(
                "orphan blob acquired a chunk reference while being claimed".to_owned(),
            ));
        }
        claimed.push((
            workspace_id,
            BlobRef {
                digest,
                size_bytes,
                media_type: "application/octet-stream".to_owned(),
            },
        ));
    }
    transaction.commit().map_err(map_database_error)?;
    Ok(claimed)
}

fn finish_orphan_resource_upload_blob_transaction(
    connection: &mut Connection,
    workspace_id: &str,
    digest: &str,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let fenced: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM resource_upload_blob_gc_fences WHERE workspace_id = ?1 AND digest = ?2)",
        params![workspace_id, digest],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !fenced {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let referenced: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM resource_upload_chunks c JOIN resource_upload_sessions s ON s.upload_id = c.upload_id WHERE s.workspace_id = ?1 AND c.temporary_blob_ref = ?2)",
        params![workspace_id, digest],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if referenced {
        return Err(StoreError::Integrity(
            "refusing to release a referenced upload blob".to_owned(),
        ));
    }
    transaction.execute(
        "DELETE FROM resource_upload_blob_reservations WHERE workspace_id = ?1 AND digest = ?2 AND state = 'DELETING'",
        params![workspace_id, digest],
    ).map_err(map_database_error)?;
    transaction
        .execute(
            "DELETE FROM resource_upload_blob_gc_fences WHERE workspace_id = ?1 AND digest = ?2",
            params![workspace_id, digest],
        )
        .map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)
}

fn load_resource_upload_ranges(
    connection: &Connection,
    upload_id: &str,
) -> Result<Vec<storage_core::ResourceUploadRange>, StoreError> {
    let mut statement = connection.prepare("SELECT start_offset, end_offset_exclusive, sha256 FROM resource_upload_chunks WHERE upload_id = ?1 ORDER BY start_offset").map_err(map_database_error)?;
    let rows = statement
        .query_map([upload_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(map_database_error)?;
    let mut ranges = Vec::new();
    for row in rows {
        let (start, end, sha256) = row.map_err(map_database_error)?;
        if end <= start {
            return Err(StoreError::CorruptSchema(
                "invalid persisted Resource upload range".to_owned(),
            ));
        }
        ranges.push(storage_core::ResourceUploadRange {
            start_offset: from_sql_i64(start, "chunk start")?,
            end_offset_inclusive: from_sql_i64(end - 1, "chunk end")?,
            sha256,
        });
    }
    Ok(ranges)
}

fn load_resource_upload_chunks(
    connection: &Connection,
    workspace_id: &str,
    upload_id: &str,
) -> Result<Vec<(u64, BlobRef, String, u64)>, StoreError> {
    let exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM resource_upload_sessions WHERE upload_id = ?1 AND workspace_id = ?2)", params![upload_id, workspace_id], |row| row.get(0)).map_err(map_database_error)?;
    if !exists {
        return Err(StoreError::NotFound);
    }
    let mut statement = connection.prepare("SELECT chunk_index, start_offset, end_offset_exclusive, sha256, temporary_blob_ref FROM resource_upload_chunks WHERE upload_id = ?1 ORDER BY chunk_index").map_err(map_database_error)?;
    let rows = statement
        .query_map([upload_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(map_database_error)?;
    let mut chunks = Vec::new();
    for row in rows {
        let (index, start, end, sha256, blob_digest) = row.map_err(map_database_error)?;
        let size = from_sql_i64(end - start, "chunk size")?;
        chunks.push((
            from_sql_i64(index, "chunk index")?,
            BlobRef {
                digest: blob_digest,
                size_bytes: size,
                media_type: "application/octet-stream".to_owned(),
            },
            sha256,
            size,
        ));
    }
    Ok(chunks)
}

fn consume_upload_blob_reservation(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    chunk: &ResourceUploadChunkInput,
) -> Result<(), StoreError> {
    let changed = transaction.execute(
        "DELETE FROM resource_upload_blob_reservations WHERE upload_id = ?1 AND request_id = ?2 AND workspace_id = ?3 AND chunk_index = ?4 AND digest = ?5 AND size_bytes = ?6 AND state = 'RESERVED'",
        params![chunk.upload_id, chunk.request_id, workspace_id, to_sql_i64(chunk.chunk_index, "chunk index")?, chunk.sha256, to_sql_i64(chunk.content.len() as u64, "chunk size")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    Ok(())
}

fn put_resource_upload_chunk_transaction(
    connection: &mut Connection,
    chunk: ResourceUploadChunkInput,
    blob: BlobRef,
    expected_progress_version: u64,
    completion_event: Option<EventDraft>,
    completion_state_ref: Option<AggregateStateRef>,
) -> Result<ResourceUploadSessionRecord, StoreError> {
    if chunk.request_id.trim().is_empty() || chunk.request_id.len() > 128 {
        return Err(StoreError::Invalid("chunk RequestId is invalid".to_owned()));
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let session = load_resource_upload(&tx, "", &chunk.upload_id)?.ok_or(StoreError::NotFound)?;
    if chunk.sha256 != digest(&chunk.content) {
        return Err(StoreError::Invalid(
            "Resource upload chunk digest does not match content".to_owned(),
        ));
    }
    let request_payload_digest = digest(&canonical_json(&json!({
        "upload_id": chunk.upload_id.clone(),
        "chunk_index": chunk.chunk_index,
        "start_offset": chunk.content_range.start_offset,
        "end_offset_inclusive": chunk.content_range.end_offset_inclusive,
        "total_size_bytes": chunk.content_range.total_size_bytes,
        "sha256": chunk.sha256.clone(),
    }))?);
    let prior_request: Option<String> = tx.query_row(
        "SELECT request_payload_digest FROM resource_upload_chunk_requests WHERE upload_id = ?1 AND request_id = ?2",
        params![chunk.upload_id, chunk.request_id],
        |row| row.get(0),
    ).optional().map_err(map_database_error)?;
    if let Some(prior_digest) = prior_request {
        if prior_digest != request_payload_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        tx.commit().map_err(map_database_error)?;
        return Ok(session);
    }
    validate_upload_chunk(&session, &chunk)?;
    let existing: Option<(i64, i64, String)> = tx.query_row("SELECT start_offset, end_offset_exclusive, sha256 FROM resource_upload_chunks WHERE upload_id = ?1 AND chunk_index = ?2", params![chunk.upload_id, to_sql_i64(chunk.chunk_index, "chunk index")?], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional().map_err(map_database_error)?;
    if let Some((start, end, sha256)) = existing {
        if from_sql_i64(start, "chunk start")? == chunk.content_range.start_offset
            && from_sql_i64(end - 1, "chunk end")? == chunk.content_range.end_offset_inclusive
            && sha256 == chunk.sha256
        {
            let reservation: Option<(String, i64, String)> = tx.query_row(
                "SELECT digest, size_bytes, state FROM resource_upload_blob_reservations WHERE upload_id = ?1 AND request_id = ?2 AND workspace_id = ?3 AND chunk_index = ?4",
                params![chunk.upload_id, chunk.request_id, session.workspace_id, to_sql_i64(chunk.chunk_index, "chunk index")?],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).optional().map_err(map_database_error)?;
            if let Some((reserved_digest, reserved_size, reservation_state)) = reservation {
                if reservation_state != "RESERVED"
                    || reserved_digest != chunk.sha256
                    || from_sql_i64(reserved_size, "reserved upload chunk size")?
                        != chunk.content.len() as u64
                {
                    return Err(StoreError::Conflict {
                        expected: None,
                        actual: None,
                    });
                }
                let removed = tx.execute(
                    "DELETE FROM resource_upload_blob_reservations WHERE upload_id = ?1 AND request_id = ?2 AND workspace_id = ?3 AND chunk_index = ?4 AND state = 'RESERVED'",
                    params![chunk.upload_id, chunk.request_id, session.workspace_id, to_sql_i64(chunk.chunk_index, "chunk index")?],
                ).map_err(map_database_error)?;
                if removed != 1 {
                    return Err(StoreError::Conflict {
                        expected: None,
                        actual: None,
                    });
                }
            }
            tx.execute(
                "INSERT INTO resource_upload_chunk_requests(upload_id, request_id, chunk_index, request_payload_digest, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![chunk.upload_id, chunk.request_id, to_sql_i64(chunk.chunk_index, "chunk index")?, request_payload_digest, chunk.received_at],
            ).map_err(map_database_error)?;
            tx.commit().map_err(map_database_error)?;
            return Ok(session);
        }
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    if session.progress_version != expected_progress_version {
        return Err(StoreError::Conflict {
            expected: Some(expected_progress_version),
            actual: Some(session.progress_version),
        });
    }
    let reservation: Option<(String, i64, String)> = tx.query_row(
        "SELECT digest, size_bytes, state FROM resource_upload_blob_reservations WHERE upload_id = ?1 AND request_id = ?2 AND workspace_id = ?3 AND chunk_index = ?4",
        params![chunk.upload_id, chunk.request_id, session.workspace_id, to_sql_i64(chunk.chunk_index, "chunk index")?],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    let Some((reserved_digest, reserved_size, reservation_state)) = reservation else {
        return Err(StoreError::Conflict {
            expected: Some(expected_progress_version),
            actual: Some(session.progress_version),
        });
    };
    if reservation_state != "RESERVED"
        || reserved_digest != chunk.sha256
        || from_sql_i64(reserved_size, "reserved upload chunk size")? != chunk.content.len() as u64
    {
        return Err(StoreError::Busy);
    }
    tx.execute(
        "INSERT INTO resource_upload_chunks(upload_id, chunk_index, start_offset, end_offset_exclusive, sha256, temporary_blob_ref, received_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![chunk.upload_id, to_sql_i64(chunk.chunk_index, "chunk index")?, to_sql_i64(chunk.content_range.start_offset, "chunk start")?, to_sql_i64(chunk.content_range.end_offset_inclusive + 1, "chunk end")?, chunk.sha256, blob.digest, chunk.received_at],
    ).map_err(map_database_error)?;
    consume_upload_blob_reservation(&tx, &session.workspace_id, &chunk)?;
    tx.execute(
        "INSERT INTO resource_upload_chunk_requests(upload_id, request_id, chunk_index, request_payload_digest, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chunk.upload_id, chunk.request_id, to_sql_i64(chunk.chunk_index, "chunk index")?, request_payload_digest, chunk.received_at],
    ).map_err(map_database_error)?;
    let session_size = to_sql_i64(session.expected_size_bytes, "upload size")?;
    let contiguous: i64 = tx.query_row("SELECT COALESCE(SUM(end_offset_exclusive - start_offset), 0) FROM resource_upload_chunks WHERE upload_id = ?1", [&chunk.upload_id], |row| row.get(0)).map_err(map_database_error)?;
    let completed = contiguous == session_size;
    if completed != completion_event.is_some() || completed != completion_state_ref.is_some() {
        return Err(StoreError::Integrity(
            "upload completion event does not match accepted coverage".to_owned(),
        ));
    }
    let next_progress_version = session
        .progress_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("upload progress version overflow".to_owned()))?;
    let next_version = if completed {
        session
            .version
            .checked_add(1)
            .ok_or_else(|| StoreError::Invalid("upload lifecycle version overflow".to_owned()))?
    } else {
        session.version
    };
    let new_state = if completed {
        "CONTENT_RECEIVED"
    } else {
        "OPEN"
    };
    let changed = tx.execute(
        "UPDATE resource_upload_sessions SET state = ?1, version = ?2, progress_version = ?3 WHERE upload_id = ?4 AND state = 'OPEN' AND version = ?5 AND progress_version = ?6",
        params![new_state, to_sql_i64(next_version, "upload lifecycle version")?, to_sql_i64(next_progress_version, "upload progress version")?, chunk.upload_id, to_sql_i64(session.version, "upload lifecycle version")?, to_sql_i64(expected_progress_version, "expected upload progress version")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(expected_progress_version),
            actual: None,
        });
    }
    let updated = load_resource_upload(&tx, "", &chunk.upload_id)?.ok_or(StoreError::NotFound)?;
    if completed {
        let event = completion_event.ok_or_else(|| {
            StoreError::Integrity("upload completion event is missing".to_owned())
        })?;
        let state_ref = completion_state_ref.ok_or_else(|| {
            StoreError::Integrity("upload completion state reference is missing".to_owned())
        })?;
        if event.workspace_id != updated.workspace_id
            || event.entity_type != "ResourceUpload"
            || event.entity_id != updated.upload_id
            || event.entity_revision != updated.version
            || event.event_type != "resource.upload.status.changed.v1"
            || event.payload.get("upload_id").and_then(Value::as_str)
                != Some(updated.upload_id.as_str())
            || event.payload.get("from").and_then(Value::as_str) != Some("OPEN")
            || event.payload.get("to").and_then(Value::as_str) != Some("CONTENT_RECEIVED")
            || event
                .payload
                .get("aggregate_version")
                .and_then(Value::as_u64)
                != Some(updated.version)
            || state_ref.entity_revision != updated.version
            || updated.progress_version != next_progress_version
        {
            return Err(StoreError::Integrity(
                "upload completion event does not match the committed session".to_owned(),
            ));
        }
        insert_upload_domain_event(&tx, event, state_ref)?;
    }
    tx.commit().map_err(map_database_error)?;
    Ok(updated)
}

fn validate_workspace_root_commit(commit: &WorkspaceRootCreateCommit) -> Result<(), StoreError> {
    let resource = &commit.resource;
    let location = &commit.location;
    let binding = &commit.private_binding;
    let identity_binding = &commit.file_identity_binding;
    let root = &commit.root;
    let request = &commit.request;
    if request.principal_id.trim().is_empty()
        || request.request_id.trim().is_empty()
        || resource.workspace_id.trim().is_empty()
        || resource.resource_id.trim().is_empty()
        || root.workspace_root_id.trim().is_empty()
        || resource.kind != "FOLDER"
        || resource.version != 1
        || resource.current_revision_id.is_some()
        || resource
            .identity_digest
            .as_deref()
            .is_none_or(|value| !storage_core::is_sha256_digest(value))
        || resource
            .provider_identity
            .as_object()
            .is_none_or(|object| object.len() != 4)
        || resource
            .provider_identity
            .get("provider_instance_id")
            .and_then(Value::as_str)
            != Some("litecowork.local_filesystem")
        || resource
            .provider_identity
            .get("stable_object_id")
            .and_then(Value::as_str)
            != resource.identity_digest.as_deref()
        || resource
            .provider_identity
            .get("identity_confidence")
            .and_then(Value::as_str)
            != Some("PROVIDER_SCOPED")
        || resource
            .provider_identity
            .get("file_identity")
            .and_then(Value::as_object)
            .is_none_or(|identity| {
                identity.len() != 5
                    || identity
                        .get("filesystem_instance_id")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                    || !(identity.get("volume_id").is_some_and(Value::is_null)
                        || identity
                            .get("volume_id")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.is_empty()))
                    || identity
                        .get("file_id")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                    || !(identity.get("generation").is_some_and(Value::is_null)
                        || identity
                            .get("generation")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.is_empty()))
                    || !matches!(
                        identity.get("platform_kind").and_then(Value::as_str),
                        Some("LINUX_DEVICE_INODE" | "MACOS_DEVICE_INODE")
                    )
            })
        || resource.sensitivity != "PERSONAL"
        || contains_private_path_field(&resource.provider_identity)
        || contains_private_path_field(&resource.provenance)
        || resource.display_name.is_empty()
        || resource.display_name.len() > 255
        || resource.display_name.contains('/')
        || resource.display_name.contains('\\')
        || resource.display_name.chars().any(char::is_control)
        || location.location_id.trim().is_empty()
        || location.runtime_id.trim().is_empty()
        || location.locator_ref_id.trim().is_empty()
        || location.observed_at.trim().is_empty()
        || location.provider_ref != "litecowork.local_filesystem"
        || location.availability != "AVAILABLE"
        || location.writable
        || location.observed_revision_id.is_some()
        || location.observed_digest.is_some()
        || binding.private_locator.trim().is_empty()
        || binding.private_locator.len() > 32_768
        || binding.private_locator.contains('\0')
        || binding.location_id != location.location_id
        || binding.locator_ref_id != location.locator_ref_id
        || binding.runtime_id != location.runtime_id
        || binding.runtime_incarnation_id.trim().is_empty()
        || binding.observed_at != location.observed_at
        || identity_binding.location_id != location.location_id
        || identity_binding.runtime_id != location.runtime_id
        || identity_binding.runtime_incarnation_id != binding.runtime_incarnation_id
        || identity_binding
            .raw_filesystem_instance_id
            .trim()
            .is_empty()
        || identity_binding.raw_filesystem_instance_id.len() > 128
        || identity_binding.raw_file_id.trim().is_empty()
        || identity_binding.raw_file_id.len() > 128
        || identity_binding
            .raw_volume_id
            .as_ref()
            .is_some_and(|value| value.len() > 128 || value.contains('\0'))
        || identity_binding
            .raw_generation
            .as_ref()
            .is_some_and(|value| value.len() > 128 || value.contains('\0'))
        || !matches!(
            identity_binding.platform_kind.as_str(),
            "LINUX_DEVICE_INODE" | "MACOS_DEVICE_INODE"
        )
        || identity_binding.observed_at != location.observed_at
        || root.workspace_id != resource.workspace_id
        || root.resource_id != resource.resource_id
        || root.location_id != location.location_id
        || root.display_name != resource.display_name
        || root.status != "ACTIVE"
        || root.version != 1
        || root.created_at != root.updated_at
        || resource.created_at != resource.updated_at
        || !matches!(
            root.watch_policy.as_str(),
            "METADATA" | "CONTENT_DIGESTS" | "SELECTED_TEXT_EXTRACTION"
        )
        || !matches!(
            root.replication_policy.as_str(),
            "NONE" | "ACTIVE_TASKS" | "SELECTED_WORKSPACE_POLICY"
        )
        || root.added_by.get("kind").and_then(Value::as_str) != Some("USER")
        || root
            .added_by
            .as_object()
            .is_none_or(|object| object.len() != 2)
        || root.added_by.get("principal_id").and_then(Value::as_str)
            != Some(request.principal_id.as_str())
        || request
            .request_payload
            .get("operation")
            .and_then(Value::as_str)
            != Some("workspace.root.create.v1")
        || request
            .request_payload
            .as_object()
            .is_none_or(|object| object.len() != 13)
        || request
            .request_payload
            .get("workspace_id")
            .and_then(Value::as_str)
            != Some(root.workspace_id.as_str())
        || request
            .request_payload
            .get("expected_workspace_version")
            .and_then(Value::as_u64)
            .is_none()
        || request
            .request_payload
            .get("resource_id")
            .and_then(Value::as_str)
            != Some(resource.resource_id.as_str())
        || request
            .request_payload
            .get("identity_digest")
            .and_then(Value::as_str)
            != resource.identity_digest.as_deref()
        || request
            .request_payload
            .get("location_id")
            .and_then(Value::as_str)
            != Some(location.location_id.as_str())
        || request
            .request_payload
            .get("locator_ref_id")
            .and_then(Value::as_str)
            != Some(location.locator_ref_id.as_str())
        || request
            .request_payload
            .get("workspace_root_id")
            .and_then(Value::as_str)
            != Some(root.workspace_root_id.as_str())
        || request
            .request_payload
            .get("runtime_id")
            .and_then(Value::as_str)
            != Some(binding.runtime_id.as_str())
        || request
            .request_payload
            .get("runtime_incarnation_id")
            .and_then(Value::as_str)
            != Some(binding.runtime_incarnation_id.as_str())
        || request
            .request_payload
            .get("display_name")
            .and_then(Value::as_str)
            != Some(root.display_name.as_str())
        || request
            .request_payload
            .get("watch_policy")
            .and_then(Value::as_str)
            != Some(root.watch_policy.as_str())
        || request
            .request_payload
            .get("replication_policy")
            .and_then(Value::as_str)
            != Some(root.replication_policy.as_str())
        || contains_private_path_field(&request.request_payload)
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot creation payload is inconsistent".to_owned(),
        ));
    }
    let resource_event = &commit.resource_created_event;
    let location_event = &commit.location_observed_event;
    let root_event = &commit.root_created_event;
    if resource_event.event_id == location_event.event_id
        || resource_event.event_id == root_event.event_id
        || location_event.event_id == root_event.event_id
        || [resource_event, location_event].iter().any(|event| {
            event.workspace_id != resource.workspace_id
                || event.schema_version != 1
                || event.entity_type != "Resource"
                || event.entity_id != resource.resource_id
                || event.entity_revision != resource.version
                || event.origin_runtime_id != binding.runtime_id
                || event.recorded_at != root.created_at
        })
        || resource_event.event_type != "resource.created.v1"
        || resource_event
            .payload
            .as_object()
            .is_none_or(|object| object.len() != 6)
        || contains_private_path_field(&resource_event.payload)
        || resource_event
            .payload
            .get("resource_id")
            .and_then(Value::as_str)
            != Some(resource.resource_id.as_str())
        || resource_event
            .payload
            .get("workspace_id")
            .and_then(Value::as_str)
            != Some(resource.workspace_id.as_str())
        || resource_event.payload.get("kind").and_then(Value::as_str) != Some("FOLDER")
        || resource_event
            .payload
            .get("identity_digest")
            .and_then(Value::as_str)
            != resource.identity_digest.as_deref()
        || resource_event.payload.get("provenance") != Some(&resource.provenance)
        || resource_event
            .payload
            .get("aggregate_version")
            .and_then(Value::as_u64)
            != Some(resource.version)
        || location_event.event_type != "resource.location.changed.v1"
        || location_event
            .payload
            .as_object()
            .is_none_or(|object| object.len() != 4)
        || contains_private_path_field(&location_event.payload)
        || location_event
            .payload
            .get("location_id")
            .and_then(Value::as_str)
            != Some(location.location_id.as_str())
        || location_event
            .payload
            .get("resource_id")
            .and_then(Value::as_str)
            != Some(resource.resource_id.as_str())
        || location_event
            .payload
            .get("availability")
            .and_then(Value::as_str)
            != Some("AVAILABLE")
        || location_event
            .payload
            .get("observed_at")
            .and_then(Value::as_str)
            != Some(location.observed_at.as_str())
        || root_event.workspace_id != root.workspace_id
        || root_event.entity_type != "WorkspaceRoot"
        || root_event.entity_id != root.workspace_root_id
        || root_event.entity_revision != root.version
        || root_event.origin_runtime_id != binding.runtime_id
        || root_event.schema_version != 1
        || root_event.recorded_at != root.created_at
        || root_event.event_type != "workspace.root.created.v1"
        || root_event
            .payload
            .as_object()
            .is_none_or(|object| object.len() != 6)
        || contains_private_path_field(&root_event.payload)
        || root_event
            .payload
            .get("workspace_root_id")
            .and_then(Value::as_str)
            != Some(root.workspace_root_id.as_str())
        || root_event
            .payload
            .get("workspace_id")
            .and_then(Value::as_str)
            != Some(root.workspace_id.as_str())
        || root_event
            .payload
            .get("resource_id")
            .and_then(Value::as_str)
            != Some(root.resource_id.as_str())
        || root_event
            .payload
            .get("location_id")
            .and_then(Value::as_str)
            != Some(root.location_id.as_str())
        || root_event.payload.get("added_by") != Some(&root.added_by)
        || root_event
            .payload
            .get("aggregate_version")
            .and_then(Value::as_u64)
            != Some(root.version)
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot creation events do not match their records".to_owned(),
        ));
    }
    Ok(())
}

fn contains_private_path_field(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| {
            matches!(
                key.as_str(),
                "private_locator" | "absolute_path" | "filesystem_path"
            ) || (key == "path" && !child.is_null())
                || contains_private_path_field(child)
        }),
        Value::Array(items) => items.iter().any(contains_private_path_field),
        _ => false,
    }
}

fn create_workspace_root_transaction(
    connection: &mut Connection,
    commit: WorkspaceRootCreateCommit,
    resource_state_ref: AggregateStateRef,
    root_state_ref: AggregateStateRef,
) -> Result<CommittedWorkspaceRoot, StoreError> {
    validate_workspace_root_commit(&commit)?;
    if resource_state_ref.entity_revision != commit.resource.version
        || root_state_ref.entity_revision != commit.root.version
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot aggregate state revision is inconsistent".to_owned(),
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let request_digest = digest(&canonical_json(&commit.request.request_payload)?);
    let prior: Option<(String, Option<String>, Option<String>)> = transaction.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![commit.request.principal_id, commit.request.request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((prior_digest, response_json, response_digest)) = prior {
        if prior_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response_json = response_json.ok_or_else(|| {
            StoreError::Integrity("WorkspaceRoot idempotency receipt is incomplete".to_owned())
        })?;
        if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "WorkspaceRoot idempotency response digest does not match".to_owned(),
            ));
        }
        return serde_json::from_str(&response_json)
            .map_err(|error| StoreError::Integrity(error.to_string()));
    }
    let owner_and_status: Option<(String, String)> = transaction
        .query_row(
            "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
            [&commit.root.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    match owner_and_status {
        Some((owner, status)) if owner == commit.request.principal_id && status == "ACTIVE" => {}
        Some((owner, _)) if owner != commit.request.principal_id => {
            return Err(StoreError::NotFound);
        }
        Some(_) | None => return Err(StoreError::NotFound),
    }
    let expected_workspace_version = commit
        .request
        .request_payload
        .get("expected_workspace_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            StoreError::Invalid("WorkspaceRoot expected version is missing".to_owned())
        })?;
    let actual_workspace_version: i64 = transaction
        .query_row(
            "SELECT version FROM workspaces WHERE workspace_id = ?1",
            [&commit.root.workspace_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let actual_workspace_version = from_sql_i64(actual_workspace_version, "Workspace version")?;
    if actual_workspace_version != expected_workspace_version {
        return Err(StoreError::Conflict {
            expected: Some(expected_workspace_version),
            actual: Some(actual_workspace_version),
        });
    }
    let runtime_authorized: bool = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1
           FROM runtimes r
           JOIN runtime_incarnations i
             ON i.runtime_id = r.runtime_id
            AND i.runtime_incarnation_id = r.current_incarnation_id
           JOIN runtime_workspace_bindings b
             ON b.runtime_id = r.runtime_id
            AND b.workspace_id = ?1
            AND b.status = 'ACTIVE'
           WHERE r.runtime_id = ?2
             AND r.current_incarnation_id = ?3
             AND r.trust_zone = 'PERSONAL_DEVICE'
             AND r.availability IN ('ONLINE', 'DEGRADED')
             AND i.recovery_state IN ('READY', 'DEGRADED')
             AND EXISTS (SELECT 1 FROM json_each(r.roles_json) role WHERE role.value = 'OPERATOR_ENDPOINT')
             AND EXISTS (SELECT 1 FROM json_each(b.roles_json) role WHERE role.value = 'EXECUTOR')
         )",
        params![commit.root.workspace_id, commit.private_binding.runtime_id, commit.private_binding.runtime_incarnation_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !runtime_authorized {
        return Err(StoreError::NotFound);
    }

    let resource = &commit.resource;
    let location = &commit.location;
    let binding = &commit.private_binding;
    let identity_binding = &commit.file_identity_binding;
    let root = &commit.root;
    transaction.execute(
        "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, identity_digest, display_name, current_revision_id, sensitivity, context_document_json, provenance_json, created_at, updated_at, version) VALUES (?1, ?2, 'FOLDER', ?3, ?4, ?5, NULL, ?6, NULL, ?7, ?8, ?9, 1)",
        params![resource.resource_id, resource.workspace_id,
            String::from_utf8(canonical_json(&resource.provider_identity)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
            resource.identity_digest, resource.display_name, resource.sensitivity,
            String::from_utf8(canonical_json(&resource.provenance)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
            resource.created_at, resource.updated_at],
    ).map_err(map_database_error)?;
    transaction.execute(
        "INSERT INTO resource_locations(location_id, resource_id, runtime_id, environment_id, connection_id, provider_ref, locator_ref_id, availability, writable, observed_revision_id, observed_digest, observed_at, last_checked_at) VALUES (?1, ?2, ?3, NULL, NULL, ?4, ?5, 'AVAILABLE', 0, NULL, NULL, ?6, ?6)",
        params![location.location_id, location.resource_id, location.runtime_id,
            location.provider_ref, location.locator_ref_id, location.observed_at],
    ).map_err(map_database_error)?;
    transaction.execute(
        "INSERT INTO resource_location_bindings(location_id, locator_ref_id, runtime_id, runtime_incarnation_id, private_locator, observed_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![binding.location_id, binding.locator_ref_id, binding.runtime_id,
            binding.runtime_incarnation_id, binding.private_locator, binding.observed_at],
    ).map_err(map_database_error)?;
    transaction.execute(
        "INSERT INTO file_identity_bindings(location_id, runtime_id, runtime_incarnation_id, raw_filesystem_instance_id, raw_volume_id, raw_file_id, raw_generation, platform_kind, observed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![identity_binding.location_id, identity_binding.runtime_id,
            identity_binding.runtime_incarnation_id, identity_binding.raw_filesystem_instance_id,
            identity_binding.raw_volume_id, identity_binding.raw_file_id,
            identity_binding.raw_generation, identity_binding.platform_kind, identity_binding.observed_at],
    ).map_err(map_database_error)?;
    transaction.execute(
        "INSERT INTO workspace_roots(workspace_root_id, workspace_id, resource_id, location_id, display_name, watch_policy, replication_policy, status, added_by_json, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ACTIVE', ?8, ?9, ?10, 1)",
        params![root.workspace_root_id, root.workspace_id, root.resource_id, root.location_id,
            root.display_name, root.watch_policy, root.replication_policy,
            String::from_utf8(canonical_json(&root.added_by)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
            root.created_at, root.updated_at],
    ).map_err(map_database_error)?;

    let resource_event = insert_domain_event(
        &transaction,
        &commit.resource_created_event,
        &resource_state_ref,
    )?;
    let location_event = insert_domain_event(
        &transaction,
        &commit.location_observed_event,
        &resource_state_ref,
    )?;
    let root_event =
        insert_domain_event(&transaction, &commit.root_created_event, &root_state_ref)?;
    let root_created_at = commit.root.created_at.clone();
    let result = CommittedWorkspaceRoot {
        resource: commit.resource,
        location: commit.location,
        root: commit.root,
        events: vec![resource_event, location_event, root_event],
    };
    let response_json = String::from_utf8(canonical_json(&result)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    transaction.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![commit.request.principal_id, commit.request.request_id, request_digest,
            response_json, digest(response_json.as_bytes()), root_created_at],
    ).map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(result)
}

fn validate_workspace_root_status_commit(
    commit: &WorkspaceRootStatusCommit,
) -> Result<(), StoreError> {
    if commit.action == WorkspaceRootStatusAction::Resume {
        return Err(StoreError::Invalid(
            "root Resume requires a fresh filesystem identity proof".to_owned(),
        ));
    }
    let expected_next_version = commit
        .expected_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("WorkspaceRoot version overflow".to_owned()))?;
    let event = &commit.event;
    let from_status_valid = match commit.action {
        WorkspaceRootStatusAction::Pause => {
            event.payload.get("from").and_then(Value::as_str) == Some("ACTIVE")
        }
        WorkspaceRootStatusAction::Resume => false,
        WorkspaceRootStatusAction::Revoke => matches!(
            event.payload.get("from").and_then(Value::as_str),
            Some("ACTIVE" | "PAUSED" | "UNAVAILABLE")
        ),
    };
    let runtime_fields_valid = match commit.action {
        _ => {
            commit
                .request
                .request_payload
                .as_object()
                .is_some_and(|payload| payload.len() == 4)
                && commit.runtime_id.is_none()
                && commit.runtime_incarnation_id.is_none()
                && commit.request.request_payload.get("runtime_id").is_none()
                && commit
                    .request
                    .request_payload
                    .get("runtime_incarnation_id")
                    .is_none()
        }
    };
    let expected_reason = commit.action.reason_code();
    let expected_operation = commit.action.operation();
    if commit.request.principal_id.trim().is_empty()
        || commit.request.request_id.trim().is_empty()
        || commit.root.workspace_id.trim().is_empty()
        || commit.root.workspace_root_id.trim().is_empty()
        || commit.root.status != commit.action.target_status()
        || commit.root.version != expected_next_version
        || commit.root.updated_at.trim().is_empty()
        || commit.root.updated_at != event.recorded_at
        || event.workspace_id != commit.root.workspace_id
        || event.entity_type != "WorkspaceRoot"
        || event.entity_id != commit.root.workspace_root_id
        || event.entity_revision != commit.root.version
        || event.event_type != "workspace.root.status.changed.v1"
        || event
            .payload
            .as_object()
            .is_none_or(|payload| payload.len() != 5)
        || event
            .payload
            .get("workspace_root_id")
            .and_then(Value::as_str)
            != Some(commit.root.workspace_root_id.as_str())
        || !from_status_valid
        || event.payload.get("to").and_then(Value::as_str) != Some(commit.action.target_status())
        || event.payload.get("reason_code").and_then(Value::as_str) != Some(expected_reason)
        || event
            .payload
            .get("aggregate_version")
            .and_then(Value::as_u64)
            != Some(commit.root.version)
        || commit
            .request
            .request_payload
            .get("operation")
            .and_then(Value::as_str)
            != Some(expected_operation)
        || commit
            .request
            .request_payload
            .get("workspace_id")
            .and_then(Value::as_str)
            != Some(commit.root.workspace_id.as_str())
        || commit
            .request
            .request_payload
            .get("workspace_root_id")
            .and_then(Value::as_str)
            != Some(commit.root.workspace_root_id.as_str())
        || commit
            .request
            .request_payload
            .get("expected_version")
            .and_then(Value::as_u64)
            != Some(commit.expected_version)
        || !runtime_fields_valid
        || contains_private_path_field(&commit.request.request_payload)
        || contains_private_path_field(&event.payload)
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot status transition is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn get_workspace_root_status_receipt(
    connection: &Connection,
    request: &WorkspaceCreateRequest,
) -> Result<Option<CommittedWorkspaceRootStatus>, StoreError> {
    let request_digest = digest(&canonical_json(&request.request_payload)?);
    let prior: Option<(String, Option<String>, Option<String>)> = connection.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![request.principal_id, request.request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    let Some((prior_digest, response_json, response_digest)) = prior else {
        return Ok(None);
    };
    if prior_digest != request_digest {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let response_json = response_json.ok_or_else(|| {
        StoreError::Integrity("WorkspaceRoot status receipt is incomplete".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "WorkspaceRoot status receipt digest does not match".to_owned(),
        ));
    }
    let result: CommittedWorkspaceRootStatus = serde_json::from_str(&response_json)
        .map_err(|error| StoreError::Integrity(error.to_string()))?;
    if result.root.workspace_id
        != request
            .request_payload
            .get("workspace_id")
            .and_then(Value::as_str)
            .unwrap_or("")
        || result.root.workspace_root_id
            != request
                .request_payload
                .get("workspace_root_id")
                .and_then(Value::as_str)
                .unwrap_or("")
    {
        return Err(StoreError::Integrity(
            "WorkspaceRoot status receipt identifies another root".to_owned(),
        ));
    }
    Ok(Some(result))
}

fn validate_workspace_root_resume_commit(
    commit: &WorkspaceRootResumeCommit,
) -> Result<(), StoreError> {
    let expected_next_version = commit
        .expected_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("WorkspaceRoot version overflow".to_owned()))?;
    let request = &commit.request.request_payload;
    let root_event = &commit.root_event;
    let location_event = &commit.location_event;
    let same_identity = commit
        .previous_file_identity_binding
        .raw_filesystem_instance_id
        == commit.file_identity_binding.raw_filesystem_instance_id
        && commit.previous_file_identity_binding.raw_volume_id
            == commit.file_identity_binding.raw_volume_id
        && commit.previous_file_identity_binding.raw_file_id
            == commit.file_identity_binding.raw_file_id
        && commit.previous_file_identity_binding.raw_generation
            == commit.file_identity_binding.raw_generation
        && commit.previous_file_identity_binding.platform_kind
            == commit.file_identity_binding.platform_kind;
    if commit.request.principal_id.trim().is_empty()
        || commit.request.request_id.trim().is_empty()
        || commit.runtime_id.trim().is_empty()
        || commit.runtime_incarnation_id.trim().is_empty()
        || commit.root.workspace_id.trim().is_empty()
        || commit.root.workspace_root_id.trim().is_empty()
        || commit.root.status != "ACTIVE"
        || commit.root.version != expected_next_version
        || commit.root.updated_at.trim().is_empty()
        || commit.root.updated_at != commit.location.observed_at
        || commit.location.availability != "AVAILABLE"
        || commit.location.resource_id != commit.resource.resource_id
        || commit.location.runtime_id != commit.runtime_id
        || commit.location.writable
        || commit.location.observed_revision_id.is_some()
        || commit.location.observed_digest.is_some()
        || commit.resource.workspace_id != commit.root.workspace_id
        || commit.root.resource_id != commit.resource.resource_id
        || commit.root.location_id != commit.location.location_id
        || commit.previous_locator_binding.location_id != commit.location.location_id
        || commit.previous_file_identity_binding.location_id != commit.location.location_id
        || commit.previous_locator_binding.locator_ref_id != commit.location.locator_ref_id
        || commit.previous_locator_binding.runtime_id != commit.runtime_id
        || commit
            .previous_locator_binding
            .runtime_incarnation_id
            .trim()
            .is_empty()
        || commit
            .previous_locator_binding
            .private_locator
            .trim()
            .is_empty()
        || commit.previous_file_identity_binding.runtime_id != commit.runtime_id
        || commit.previous_file_identity_binding.runtime_incarnation_id
            != commit.previous_locator_binding.runtime_incarnation_id
        || commit
            .previous_file_identity_binding
            .raw_filesystem_instance_id
            .trim()
            .is_empty()
        || commit
            .previous_file_identity_binding
            .raw_file_id
            .trim()
            .is_empty()
        || !matches!(
            commit.previous_file_identity_binding.platform_kind.as_str(),
            "LINUX_DEVICE_INODE" | "MACOS_DEVICE_INODE"
        )
        || commit.locator_binding.location_id != commit.location.location_id
        || commit.locator_binding.locator_ref_id != commit.location.locator_ref_id
        || commit.locator_binding.runtime_id != commit.runtime_id
        || commit.locator_binding.runtime_incarnation_id != commit.runtime_incarnation_id
        || commit.locator_binding.private_locator != commit.previous_locator_binding.private_locator
        || commit.locator_binding.observed_at != commit.location.observed_at
        || commit.file_identity_binding.location_id != commit.location.location_id
        || commit.file_identity_binding.runtime_id != commit.runtime_id
        || commit.file_identity_binding.runtime_incarnation_id != commit.runtime_incarnation_id
        || commit.file_identity_binding.observed_at != commit.location.observed_at
        || !same_identity
        || request.as_object().is_none_or(|payload| payload.len() != 4)
        || request.get("operation").and_then(Value::as_str) != Some("workspace.root.resume.v2")
        || request.get("workspace_id").and_then(Value::as_str)
            != Some(commit.root.workspace_id.as_str())
        || request.get("workspace_root_id").and_then(Value::as_str)
            != Some(commit.root.workspace_root_id.as_str())
        || request.get("expected_version").and_then(Value::as_u64) != Some(commit.expected_version)
        || root_event.workspace_id != commit.root.workspace_id
        || root_event.entity_type != "WorkspaceRoot"
        || root_event.entity_id != commit.root.workspace_root_id
        || root_event.entity_revision != commit.root.version
        || root_event.event_type != "workspace.root.status.changed.v1"
        || root_event.recorded_at != commit.root.updated_at
        || root_event
            .payload
            .as_object()
            .is_none_or(|payload| payload.len() != 5)
        || root_event
            .payload
            .get("workspace_root_id")
            .and_then(Value::as_str)
            != Some(commit.root.workspace_root_id.as_str())
        || root_event.payload.get("from").and_then(Value::as_str) != Some("PAUSED")
        || root_event.payload.get("to").and_then(Value::as_str) != Some("ACTIVE")
        || root_event
            .payload
            .get("reason_code")
            .and_then(Value::as_str)
            != Some("USER_RESUMED")
        || root_event
            .payload
            .get("aggregate_version")
            .and_then(Value::as_u64)
            != Some(commit.root.version)
        || location_event.workspace_id != commit.root.workspace_id
        || location_event.entity_type != "Resource"
        || location_event.entity_id != commit.resource.resource_id
        || location_event.entity_revision != commit.resource.version
        || location_event.event_type != "resource.location.changed.v1"
        || location_event.recorded_at != commit.location.observed_at
        || location_event
            .payload
            .as_object()
            .is_none_or(|payload| payload.len() != 4)
        || location_event
            .payload
            .get("location_id")
            .and_then(Value::as_str)
            != Some(commit.location.location_id.as_str())
        || location_event
            .payload
            .get("resource_id")
            .and_then(Value::as_str)
            != Some(commit.resource.resource_id.as_str())
        || location_event
            .payload
            .get("availability")
            .and_then(Value::as_str)
            != Some("AVAILABLE")
        || location_event
            .payload
            .get("observed_at")
            .and_then(Value::as_str)
            != Some(commit.location.observed_at.as_str())
        || contains_private_path_field(request)
        || contains_private_path_field(&root_event.payload)
        || contains_private_path_field(&location_event.payload)
    {
        return Err(StoreError::Invalid(
            "atomic WorkspaceRoot Resume proof is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn resume_workspace_root_transaction(
    connection: &mut Connection,
    commit: WorkspaceRootResumeCommit,
    root_state_ref: AggregateStateRef,
    resource_state_ref: AggregateStateRef,
) -> Result<CommittedWorkspaceRootStatus, StoreError> {
    validate_workspace_root_resume_commit(&commit)?;
    if root_state_ref.entity_revision != commit.root.version
        || resource_state_ref.entity_revision != commit.resource.version
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot Resume state references are inconsistent".to_owned(),
        ));
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    if let Some(receipt) = get_workspace_root_status_receipt(&tx, &commit.request)? {
        return Ok(receipt);
    }
    let workspace: Option<(String, String)> = tx
        .query_row(
            "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
            [&commit.root.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    match workspace {
        Some((owner, status)) if status == "ACTIVE" && owner == commit.request.principal_id => {}
        Some((owner, _)) if owner != commit.request.principal_id => {
            return Err(StoreError::NotFound);
        }
        Some(_) | None => return Err(StoreError::NotFound),
    }
    let runtime_ready: bool = tx.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM runtimes r
           JOIN runtime_incarnations i ON i.runtime_id = r.runtime_id AND i.runtime_incarnation_id = r.current_incarnation_id
           WHERE r.runtime_id = ?1 AND r.current_incarnation_id = ?2
             AND r.trust_zone = 'PERSONAL_DEVICE'
             AND r.availability IN ('ONLINE', 'DEGRADED')
             AND i.recovery_state IN ('READY', 'DEGRADED')
         )",
        params![commit.runtime_id, commit.runtime_incarnation_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !runtime_ready {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let current_root = load_workspace_root(
        &tx,
        &commit.root.workspace_id,
        &commit.root.workspace_root_id,
    )?
    .ok_or(StoreError::NotFound)?;
    if current_root.version != commit.expected_version
        || current_root.status != "PAUSED"
        || commit.root.resource_id != current_root.resource_id
        || commit.root.location_id != current_root.location_id
        || commit.root.display_name != current_root.display_name
        || commit.root.watch_policy != current_root.watch_policy
        || commit.root.replication_policy != current_root.replication_policy
        || commit.root.added_by != current_root.added_by
        || commit.root.created_at != current_root.created_at
    {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_version),
            actual: Some(current_root.version),
        });
    }
    let current_resource = load_resource_record(
        &tx,
        &commit.resource.workspace_id,
        &commit.resource.resource_id,
    )?
    .ok_or(StoreError::NotFound)?;
    if current_resource != commit.resource {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let current_location = load_resource_location_record(
        &tx,
        &commit.resource.resource_id,
        &commit.location.location_id,
    )?
    .ok_or(StoreError::NotFound)?;
    if current_location.runtime_id != commit.runtime_id
        || current_location.locator_ref_id != commit.location.locator_ref_id
        || current_location.provider_ref != commit.location.provider_ref
        || current_location.availability == "REVOKED"
        || current_location.writable
        || current_location.observed_revision_id.is_some()
        || current_location.observed_digest.is_some()
    {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_version),
            actual: Some(current_root.version),
        });
    }
    let previous_bindings = load_workspace_root_revalidation_bindings(
        &tx,
        &commit.runtime_id,
        &commit.runtime_incarnation_id,
        &current_location,
    )?;
    let source_matches = match previous_bindings {
        WorkspaceRootRevalidationBindings::Previous {
            locator,
            file_identity,
        }
        | WorkspaceRootRevalidationBindings::CurrentIncarnation {
            locator,
            file_identity,
        } => {
            private_locator_binding_matches(&locator, &commit.previous_locator_binding)
                && file_identity_binding_matches(
                    &file_identity,
                    &commit.previous_file_identity_binding,
                )
        }
        WorkspaceRootRevalidationBindings::Unavailable(_) => false,
    };
    if !source_matches {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let current_locator: Option<(String, String, String, String, String)> = tx.query_row(
        "SELECT locator_ref_id, runtime_id, runtime_incarnation_id, private_locator, observed_at
         FROM resource_location_bindings WHERE location_id = ?1 AND runtime_id = ?2 AND runtime_incarnation_id = ?3",
        params![commit.location.location_id, commit.runtime_id, commit.runtime_incarnation_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).optional().map_err(map_database_error)?;
    let current_identity: Option<(String, String, Option<String>, String, Option<String>, String, String)> = tx.query_row(
        "SELECT runtime_id, raw_filesystem_instance_id, raw_volume_id, raw_file_id, raw_generation, platform_kind, observed_at
         FROM file_identity_bindings WHERE location_id = ?1 AND runtime_id = ?2 AND runtime_incarnation_id = ?3",
        params![commit.location.location_id, commit.runtime_id, commit.runtime_incarnation_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
    ).optional().map_err(map_database_error)?;
    match (current_locator, current_identity) {
        (None, None) => {
            tx.execute(
                "INSERT INTO resource_location_bindings(location_id, locator_ref_id, runtime_id, runtime_incarnation_id, private_locator, observed_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                params![commit.locator_binding.location_id, commit.locator_binding.locator_ref_id,
                    commit.locator_binding.runtime_id, commit.locator_binding.runtime_incarnation_id,
                    commit.locator_binding.private_locator, commit.locator_binding.observed_at],
            ).map_err(map_database_error)?;
            tx.execute(
                "INSERT INTO file_identity_bindings(location_id, runtime_id, runtime_incarnation_id, raw_filesystem_instance_id, raw_volume_id, raw_file_id, raw_generation, platform_kind, observed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![commit.file_identity_binding.location_id, commit.file_identity_binding.runtime_id,
                    commit.file_identity_binding.runtime_incarnation_id, commit.file_identity_binding.raw_filesystem_instance_id,
                    commit.file_identity_binding.raw_volume_id, commit.file_identity_binding.raw_file_id,
                    commit.file_identity_binding.raw_generation, commit.file_identity_binding.platform_kind,
                    commit.file_identity_binding.observed_at],
            ).map_err(map_database_error)?;
        }
        (
            Some((locator_ref, runtime, incarnation, private_locator, _)),
            Some((identity_runtime, raw_fs, volume, raw_file, generation, platform, _)),
        ) => {
            let existing_locator = storage_core::LocalResourceLocationBindingRecord {
                location_id: commit.location.location_id.clone(),
                locator_ref_id: locator_ref,
                runtime_id: runtime,
                runtime_incarnation_id: incarnation,
                private_locator,
                observed_at: String::new(),
            };
            let proposed_locator_identity = storage_core::LocalResourceLocationBindingRecord {
                observed_at: String::new(),
                ..commit.locator_binding.clone()
            };
            let existing_file_identity = storage_core::LocalFileIdentityBindingRecord {
                location_id: commit.location.location_id.clone(),
                runtime_id: identity_runtime,
                runtime_incarnation_id: commit.runtime_incarnation_id.clone(),
                raw_filesystem_instance_id: raw_fs,
                raw_volume_id: volume,
                raw_file_id: raw_file,
                raw_generation: generation,
                platform_kind: platform,
                observed_at: String::new(),
            };
            let proposed_file_identity = storage_core::LocalFileIdentityBindingRecord {
                observed_at: String::new(),
                ..commit.file_identity_binding
            };
            if !private_locator_binding_matches(&existing_locator, &proposed_locator_identity)
                || !file_identity_binding_matches(&existing_file_identity, &proposed_file_identity)
            {
                return Err(StoreError::Integrity(
                    "current-incarnation WorkspaceRoot binding conflicts".to_owned(),
                ));
            }
            tx.execute(
                "UPDATE resource_location_bindings SET observed_at = ?1 WHERE location_id = ?2 AND runtime_id = ?3 AND runtime_incarnation_id = ?4",
                params![commit.location.observed_at, commit.location.location_id, commit.runtime_id, commit.runtime_incarnation_id],
            ).map_err(map_database_error)?;
            tx.execute(
                "UPDATE file_identity_bindings SET observed_at = ?1 WHERE location_id = ?2 AND runtime_id = ?3 AND runtime_incarnation_id = ?4",
                params![commit.location.observed_at, commit.location.location_id, commit.runtime_id, commit.runtime_incarnation_id],
            ).map_err(map_database_error)?;
        }
        _ => {
            return Err(StoreError::Integrity(
                "current-incarnation WorkspaceRoot bindings are incomplete".to_owned(),
            ));
        }
    }
    let changed_root = tx.execute(
        "UPDATE workspace_roots SET status = 'ACTIVE', updated_at = ?1, version = ?2 WHERE workspace_id = ?3 AND workspace_root_id = ?4 AND status = 'PAUSED' AND version = ?5",
        params![commit.root.updated_at, to_sql_i64(commit.root.version, "WorkspaceRoot version")?,
            commit.root.workspace_id, commit.root.workspace_root_id,
            to_sql_i64(commit.expected_version, "expected WorkspaceRoot version")?],
    ).map_err(map_database_error)?;
    if changed_root != 1 {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_version),
            actual: None,
        });
    }
    let changed_location = tx.execute(
        "UPDATE resource_locations SET availability = 'AVAILABLE', observed_at = ?1, last_checked_at = ?1 WHERE location_id = ?2 AND resource_id = ?3 AND runtime_id = ?4 AND availability <> 'REVOKED'",
        params![commit.location.observed_at, commit.location.location_id, commit.location.resource_id, commit.runtime_id],
    ).map_err(map_database_error)?;
    if changed_location != 1 {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_version),
            actual: Some(commit.expected_version),
        });
    }
    let event = insert_domain_event(&tx, &commit.root_event, &root_state_ref)?;
    insert_domain_event(&tx, &commit.location_event, &resource_state_ref)?;
    let root_updated_at = commit.root.updated_at.clone();
    let result = CommittedWorkspaceRootStatus {
        root: commit.root,
        location_availability: Some("AVAILABLE".to_owned()),
        event,
    };
    let response_json = String::from_utf8(canonical_json(&result)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![commit.request.principal_id, commit.request.request_id,
            digest(&canonical_json(&commit.request.request_payload)?), response_json,
            digest(response_json.as_bytes()), root_updated_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(result)
}

fn update_workspace_root_status_transaction(
    connection: &mut Connection,
    commit: WorkspaceRootStatusCommit,
    root_state_ref: AggregateStateRef,
) -> Result<CommittedWorkspaceRootStatus, StoreError> {
    validate_workspace_root_status_commit(&commit)?;
    if root_state_ref.entity_revision != commit.root.version {
        return Err(StoreError::Invalid(
            "WorkspaceRoot aggregate state revision is inconsistent".to_owned(),
        ));
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let request_digest = digest(&canonical_json(&commit.request.request_payload)?);
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![commit.request.principal_id, commit.request.request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((prior_digest, response_json, response_digest)) = prior {
        if prior_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response_json = response_json.ok_or_else(|| {
            StoreError::Integrity("WorkspaceRoot status receipt is incomplete".to_owned())
        })?;
        if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "WorkspaceRoot status receipt digest does not match".to_owned(),
            ));
        }
        return serde_json::from_str(&response_json)
            .map_err(|error| StoreError::Integrity(error.to_string()));
    }
    let workspace: Option<(String, String)> = tx
        .query_row(
            "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
            [&commit.root.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    match workspace {
        Some((owner, status)) if owner == commit.request.principal_id && status == "ACTIVE" => {}
        Some((owner, _)) if owner != commit.request.principal_id => {
            return Err(StoreError::NotFound);
        }
        Some(_) | None => return Err(StoreError::NotFound),
    }
    let current = load_workspace_root(
        &tx,
        &commit.root.workspace_id,
        &commit.root.workspace_root_id,
    )?
    .ok_or(StoreError::NotFound)?;
    if current.version != commit.expected_version {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_version),
            actual: Some(current.version),
        });
    }
    let transition_allowed = match commit.action {
        WorkspaceRootStatusAction::Pause => current.status == "ACTIVE",
        WorkspaceRootStatusAction::Resume => current.status == "PAUSED",
        WorkspaceRootStatusAction::Revoke => {
            matches!(current.status.as_str(), "ACTIVE" | "PAUSED" | "UNAVAILABLE")
        }
    };
    if !transition_allowed
        || commit.event.payload.get("from").and_then(Value::as_str) != Some(current.status.as_str())
        || commit.root.workspace_id != current.workspace_id
        || commit.root.resource_id != current.resource_id
        || commit.root.location_id != current.location_id
        || commit.root.display_name != current.display_name
        || commit.root.watch_policy != current.watch_policy
        || commit.root.replication_policy != current.replication_policy
        || commit.root.added_by != current.added_by
        || commit.root.created_at != current.created_at
    {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_version),
            actual: Some(current.version),
        });
    }
    if commit.action == WorkspaceRootStatusAction::Resume {
        let runtime_id = commit
            .runtime_id
            .as_deref()
            .ok_or_else(|| StoreError::Invalid("root resume requires a Runtime".to_owned()))?;
        let runtime_incarnation_id = commit.runtime_incarnation_id.as_deref().ok_or_else(|| {
            StoreError::Invalid("root resume requires a Runtime incarnation".to_owned())
        })?;
        let identity_ready: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM resource_locations l JOIN resource_location_bindings lb ON lb.location_id = l.location_id AND lb.locator_ref_id = l.locator_ref_id JOIN file_identity_bindings fib ON fib.location_id = lb.location_id AND fib.runtime_id = lb.runtime_id AND fib.runtime_incarnation_id = lb.runtime_incarnation_id JOIN runtimes r ON r.runtime_id = lb.runtime_id AND r.current_incarnation_id = lb.runtime_incarnation_id JOIN runtime_incarnations i ON i.runtime_id = r.runtime_id AND i.runtime_incarnation_id = r.current_incarnation_id WHERE l.location_id = ?1 AND l.resource_id = ?2 AND l.runtime_id = ?3 AND l.availability = 'AVAILABLE' AND lb.runtime_id = ?3 AND lb.runtime_incarnation_id = ?4 AND i.recovery_state IN ('READY', 'DEGRADED'))",
            params![commit.root.location_id, commit.root.resource_id, runtime_id, runtime_incarnation_id],
            |row| row.get(0),
        ).map_err(map_database_error)?;
        if !identity_ready {
            return Err(StoreError::Conflict {
                expected: Some(commit.expected_version),
                actual: Some(current.version),
            });
        }
    }
    let changed = tx.execute(
        "UPDATE workspace_roots SET status = ?1, updated_at = ?2, version = ?3 WHERE workspace_id = ?4 AND workspace_root_id = ?5 AND status = ?6 AND version = ?7",
        params![commit.action.target_status(), commit.root.updated_at, to_sql_i64(commit.root.version, "WorkspaceRoot version")?, commit.root.workspace_id, commit.root.workspace_root_id, current.status, to_sql_i64(commit.expected_version, "expected WorkspaceRoot version")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_version),
            actual: None,
        });
    }
    if commit.action == WorkspaceRootStatusAction::Revoke {
        tx.execute(
            "UPDATE resource_locations SET availability = 'REVOKED', writable = 0 WHERE location_id = ?1 AND resource_id = ?2",
            params![commit.root.location_id, commit.root.resource_id],
        ).map_err(map_database_error)?;
        tx.execute(
            "DELETE FROM resource_location_bindings WHERE location_id = ?1",
            [&commit.root.location_id],
        )
        .map_err(map_database_error)?;
        tx.execute(
            "DELETE FROM file_identity_bindings WHERE location_id = ?1",
            [&commit.root.location_id],
        )
        .map_err(map_database_error)?;
        tx.execute(
            "DELETE FROM workspace_replication_roots WHERE workspace_id = ?1 AND workspace_root_id = ?2",
            params![commit.root.workspace_id, commit.root.workspace_root_id],
        ).map_err(map_database_error)?;
    }
    let event = insert_domain_event(&tx, &commit.event, &root_state_ref)?;
    let location_availability: String = tx.query_row(
        "SELECT availability FROM resource_locations WHERE location_id = ?1 AND resource_id = ?2",
        params![commit.root.location_id, commit.root.resource_id],
        |row| row.get(0),
    ).optional().map_err(map_database_error)?
        .ok_or_else(|| StoreError::Integrity("WorkspaceRoot ResourceLocation disappeared during status change".to_owned()))?;
    let result = CommittedWorkspaceRootStatus {
        root: commit.root,
        location_availability: Some(location_availability),
        event,
    };
    let response_json = String::from_utf8(canonical_json(&result)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![commit.request.principal_id, commit.request.request_id, request_digest, response_json, digest(response_json.as_bytes()), result.root.updated_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(result)
}

const ROOT_REVALIDATOR_PRINCIPAL: &str = "service:workspace-root-revalidator";

fn validate_workspace_root_revalidation_commit(
    commit: &WorkspaceRootRevalidationCommit,
) -> Result<(), StoreError> {
    let has_locator = commit.locator_binding.is_some();
    let has_identity = commit.file_identity_binding.is_some();
    let verified = has_locator && has_identity;
    if has_locator != has_identity
        || commit.request.principal_id != ROOT_REVALIDATOR_PRINCIPAL
        || commit.request.request_id.trim().is_empty()
        || commit.runtime_id.trim().is_empty()
        || commit.runtime_incarnation_id.trim().is_empty()
        || commit.root.workspace_id.trim().is_empty()
        || commit.root.workspace_root_id.trim().is_empty()
        || commit.root.resource_id != commit.resource.resource_id
        || commit.resource.workspace_id != commit.root.workspace_id
        || commit.location.resource_id != commit.resource.resource_id
        || commit.root.location_id != commit.location.location_id
        || commit.location.runtime_id != commit.runtime_id
        || commit.location.provider_ref != "litecowork.local_filesystem"
        || commit.location.writable
        || commit.location.observed_revision_id.is_some()
        || commit.location.observed_digest.is_some()
        || commit.location.observed_at.trim().is_empty()
        || !matches!(
            commit.root.status.as_str(),
            "ACTIVE" | "PAUSED" | "UNAVAILABLE"
        )
        || commit
            .request
            .request_payload
            .as_object()
            .is_none_or(|object| object.len() != 8)
        || commit
            .request
            .request_payload
            .get("operation")
            .and_then(Value::as_str)
            != Some("workspace.root.revalidate.v1")
        || commit
            .request
            .request_payload
            .get("workspace_id")
            .and_then(Value::as_str)
            != Some(commit.root.workspace_id.as_str())
        || commit
            .request
            .request_payload
            .get("workspace_root_id")
            .and_then(Value::as_str)
            != Some(commit.root.workspace_root_id.as_str())
        || commit
            .request
            .request_payload
            .get("runtime_id")
            .and_then(Value::as_str)
            != Some(commit.runtime_id.as_str())
        || commit
            .request
            .request_payload
            .get("runtime_incarnation_id")
            .and_then(Value::as_str)
            != Some(commit.runtime_incarnation_id.as_str())
        || commit
            .request
            .request_payload
            .get("expected_root_version")
            .and_then(Value::as_u64)
            != Some(commit.expected_root_version)
        || commit
            .request
            .request_payload
            .get("outcome")
            .and_then(Value::as_str)
            != Some(if verified { "VERIFIED" } else { "UNAVAILABLE" })
        || contains_private_path_field(&commit.request.request_payload)
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot revalidation commit is inconsistent".to_owned(),
        ));
    }
    let reason_code = commit
        .request
        .request_payload
        .get("reason_code")
        .and_then(Value::as_str);
    if (verified && reason_code != Some("ROOT_IDENTITY_REVALIDATED"))
        || (!verified
            && !matches!(
                reason_code,
                Some(
                    "NO_PRIOR_BINDING"
                        | "LOCATOR_BINDING_MISSING"
                        | "FILE_IDENTITY_BINDING_MISSING"
                        | "BINDING_MISMATCH"
                        | "UNSUPPORTED_PLATFORM"
                        | "INVALID_LOCATOR"
                        | "IDENTITY_CHANGED"
                        | "IDENTITY_UNAVAILABLE"
                )
            ))
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot revalidation reason is invalid".to_owned(),
        ));
    }
    if verified {
        let locator = commit
            .locator_binding
            .as_ref()
            .expect("checked binding pair");
        let identity = commit
            .file_identity_binding
            .as_ref()
            .expect("checked binding pair");
        if commit.location.availability != "AVAILABLE"
            || locator.location_id != commit.location.location_id
            || locator.locator_ref_id != commit.location.locator_ref_id
            || locator.runtime_id != commit.runtime_id
            || locator.runtime_incarnation_id != commit.runtime_incarnation_id
            || locator.observed_at != commit.location.observed_at
            || locator.private_locator.trim().is_empty()
            || identity.location_id != commit.location.location_id
            || identity.runtime_id != commit.runtime_id
            || identity.runtime_incarnation_id != commit.runtime_incarnation_id
            || identity.observed_at != commit.location.observed_at
            || identity.raw_filesystem_instance_id.trim().is_empty()
            || identity.raw_file_id.trim().is_empty()
            || !matches!(
                identity.platform_kind.as_str(),
                "LINUX_DEVICE_INODE" | "MACOS_DEVICE_INODE"
            )
        {
            return Err(StoreError::Invalid(
                "verified WorkspaceRoot bindings are inconsistent".to_owned(),
            ));
        }
    } else if commit.location.availability != "UNAVAILABLE"
        || !matches!(commit.root.status.as_str(), "PAUSED" | "UNAVAILABLE")
    {
        return Err(StoreError::Invalid(
            "failed WorkspaceRoot revalidation must fail closed".to_owned(),
        ));
    }
    let root_status_changed = commit.root_event.is_some();
    if let Some(event) = &commit.root_event {
        if event.workspace_id != commit.root.workspace_id
            || event.entity_type != "WorkspaceRoot"
            || event.entity_id != commit.root.workspace_root_id
            || event.entity_revision != commit.root.version
            || event.origin_runtime_id != commit.runtime_id
            || event.recorded_at != commit.location.observed_at
            || event.event_type != "workspace.root.status.changed.v1"
            || event
                .payload
                .as_object()
                .is_none_or(|payload| payload.len() != 5)
            || event
                .payload
                .get("workspace_root_id")
                .and_then(Value::as_str)
                != Some(commit.root.workspace_root_id.as_str())
            || event.payload.get("to").and_then(Value::as_str) != Some(commit.root.status.as_str())
            || event.payload.get("reason_code").and_then(Value::as_str) != reason_code
            || event
                .payload
                .get("aggregate_version")
                .and_then(Value::as_u64)
                != Some(commit.root.version)
            || contains_private_path_field(&event.payload)
        {
            return Err(StoreError::Invalid(
                "WorkspaceRoot revalidation event is inconsistent".to_owned(),
            ));
        }
    }
    let location_event = commit.location_event.as_ref().ok_or_else(|| {
        StoreError::Invalid("WorkspaceRoot revalidation location event is missing".to_owned())
    })?;
    if location_event.workspace_id != commit.root.workspace_id
        || location_event.entity_type != "Resource"
        || location_event.entity_id != commit.resource.resource_id
        || location_event.entity_revision != commit.resource.version
        || location_event.origin_runtime_id != commit.runtime_id
        || location_event.recorded_at != commit.location.observed_at
        || location_event.event_type != "resource.location.changed.v1"
        || location_event
            .payload
            .as_object()
            .is_none_or(|payload| payload.len() != 4)
        || location_event
            .payload
            .get("location_id")
            .and_then(Value::as_str)
            != Some(commit.location.location_id.as_str())
        || location_event
            .payload
            .get("resource_id")
            .and_then(Value::as_str)
            != Some(commit.resource.resource_id.as_str())
        || location_event
            .payload
            .get("availability")
            .and_then(Value::as_str)
            != Some(commit.location.availability.as_str())
        || location_event
            .payload
            .get("observed_at")
            .and_then(Value::as_str)
            != Some(commit.location.observed_at.as_str())
        || contains_private_path_field(&location_event.payload)
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot location event is inconsistent".to_owned(),
        ));
    }
    if root_status_changed && commit.root.version != commit.expected_root_version.saturating_add(1)
        || !root_status_changed && commit.root.version != commit.expected_root_version
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot revalidation version is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn private_locator_binding_matches(
    left: &storage_core::LocalResourceLocationBindingRecord,
    right: &storage_core::LocalResourceLocationBindingRecord,
) -> bool {
    left.location_id == right.location_id
        && left.locator_ref_id == right.locator_ref_id
        && left.runtime_id == right.runtime_id
        && left.runtime_incarnation_id == right.runtime_incarnation_id
        && left.private_locator == right.private_locator
        && left.observed_at == right.observed_at
}

fn file_identity_binding_matches(
    left: &storage_core::LocalFileIdentityBindingRecord,
    right: &storage_core::LocalFileIdentityBindingRecord,
) -> bool {
    left.location_id == right.location_id
        && left.runtime_id == right.runtime_id
        && left.runtime_incarnation_id == right.runtime_incarnation_id
        && left.raw_filesystem_instance_id == right.raw_filesystem_instance_id
        && left.raw_volume_id == right.raw_volume_id
        && left.raw_file_id == right.raw_file_id
        && left.raw_generation == right.raw_generation
        && left.platform_kind == right.platform_kind
        && left.observed_at == right.observed_at
}

fn commit_workspace_root_revalidation_transaction(
    connection: &mut Connection,
    commit: WorkspaceRootRevalidationCommit,
    root_state_ref: Option<AggregateStateRef>,
    resource_state_ref: Option<AggregateStateRef>,
) -> Result<CommittedWorkspaceRootRevalidation, StoreError> {
    validate_workspace_root_revalidation_commit(&commit)?;
    if root_state_ref
        .as_ref()
        .is_some_and(|state| state.entity_revision != commit.root.version)
        || resource_state_ref
            .as_ref()
            .is_none_or(|state| state.entity_revision != commit.resource.version)
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot revalidation state reference is inconsistent".to_owned(),
        ));
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let request_digest = digest(&canonical_json(&commit.request.request_payload)?);
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![commit.request.principal_id, commit.request.request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((prior_digest, response_json, response_digest)) = prior {
        if prior_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response_json = response_json.ok_or_else(|| {
            StoreError::Integrity("WorkspaceRoot revalidation receipt is incomplete".to_owned())
        })?;
        if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "WorkspaceRoot revalidation receipt digest does not match".to_owned(),
            ));
        }
        return serde_json::from_str(&response_json).map_err(|_| {
            StoreError::Integrity("WorkspaceRoot revalidation receipt is invalid".to_owned())
        });
    }
    let runtime_current: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM runtimes WHERE runtime_id = ?1 AND current_incarnation_id = ?2)",
        params![commit.runtime_id, commit.runtime_incarnation_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !runtime_current {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let current_root = load_workspace_root(
        &tx,
        &commit.root.workspace_id,
        &commit.root.workspace_root_id,
    )?
    .ok_or(StoreError::NotFound)?;
    if current_root.status == "REVOKED" {
        return Err(StoreError::NotFound);
    }
    if current_root.version != commit.expected_root_version {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_root_version),
            actual: Some(current_root.version),
        });
    }
    if current_root.resource_id != commit.root.resource_id
        || current_root.location_id != commit.root.location_id
        || current_root.display_name != commit.root.display_name
        || current_root.watch_policy != commit.root.watch_policy
        || current_root.replication_policy != commit.root.replication_policy
        || current_root.added_by != commit.root.added_by
        || current_root.created_at != commit.root.created_at
        || (commit.root_event.is_none() && current_root.updated_at != commit.root.updated_at)
    {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_root_version),
            actual: Some(current_root.version),
        });
    }
    let current_resource = load_resource_record(
        &tx,
        &commit.resource.workspace_id,
        &commit.resource.resource_id,
    )?
    .ok_or(StoreError::NotFound)?;
    if current_resource != commit.resource {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let current_location = load_resource_location_record(
        &tx,
        &commit.resource.resource_id,
        &commit.location.location_id,
    )?
    .ok_or(StoreError::NotFound)?;
    if current_location.runtime_id != commit.runtime_id
        || current_location.locator_ref_id != commit.location.locator_ref_id
        || current_location.provider_ref != commit.location.provider_ref
        || current_location.availability == "REVOKED"
        || current_location.writable
        || current_location.observed_revision_id.is_some()
        || current_location.observed_digest.is_some()
    {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let verified = commit.locator_binding.is_some();
    let expected_status = if verified {
        if current_root.status == "UNAVAILABLE" {
            "ACTIVE"
        } else {
            current_root.status.as_str()
        }
    } else if current_root.status == "PAUSED" {
        // Preserve explicit owner intent independently from the location's observed
        // availability. A paused root stays paused even if its directory is offline.
        "PAUSED"
    } else {
        "UNAVAILABLE"
    };
    if commit.root.status != expected_status {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_root_version),
            actual: Some(current_root.version),
        });
    }
    let root_changed = commit.root.status != current_root.status;
    if root_changed != commit.root_event.is_some() {
        return Err(StoreError::Invalid(
            "WorkspaceRoot status event does not match the transition".to_owned(),
        ));
    }
    if (root_changed && commit.root.updated_at != commit.location.observed_at)
        || (!root_changed && commit.root.updated_at != current_root.updated_at)
    {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_root_version),
            actual: Some(current_root.version),
        });
    }
    if let Some(event) = &commit.root_event {
        if event.payload.get("from").and_then(Value::as_str) != Some(current_root.status.as_str()) {
            return Err(StoreError::Conflict {
                expected: Some(commit.expected_root_version),
                actual: Some(current_root.version),
            });
        }
    }
    if let Some(locator) = &commit.locator_binding {
        let current_locator: Option<(String, String, String, String, String)> = tx.query_row(
            "SELECT locator_ref_id, runtime_id, runtime_incarnation_id, private_locator, observed_at
             FROM resource_location_bindings WHERE location_id = ?1 AND runtime_incarnation_id = ?2",
            params![commit.location.location_id, commit.runtime_incarnation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        ).optional().map_err(map_database_error)?;
        let current_identity: Option<(String, String, Option<String>, String, Option<String>, String, String)> = tx.query_row(
            "SELECT runtime_id, raw_filesystem_instance_id, raw_volume_id, raw_file_id, raw_generation, platform_kind, observed_at
             FROM file_identity_bindings WHERE location_id = ?1 AND runtime_incarnation_id = ?2",
            params![commit.location.location_id, commit.runtime_incarnation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
        ).optional().map_err(map_database_error)?;
        match (current_locator, current_identity) {
            (None, None) => {
                tx.execute(
                    "INSERT INTO resource_location_bindings(location_id, locator_ref_id, runtime_id, runtime_incarnation_id, private_locator, observed_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                    params![locator.location_id, locator.locator_ref_id, locator.runtime_id, locator.runtime_incarnation_id, locator.private_locator, locator.observed_at],
                ).map_err(map_database_error)?;
                let identity = commit
                    .file_identity_binding
                    .as_ref()
                    .expect("validated binding pair");
                tx.execute(
                    "INSERT INTO file_identity_bindings(location_id, runtime_id, runtime_incarnation_id, raw_filesystem_instance_id, raw_volume_id, raw_file_id, raw_generation, platform_kind, observed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![identity.location_id, identity.runtime_id, identity.runtime_incarnation_id, identity.raw_filesystem_instance_id, identity.raw_volume_id, identity.raw_file_id, identity.raw_generation, identity.platform_kind, identity.observed_at],
                ).map_err(map_database_error)?;
            }
            (
                Some((locator_ref_id, runtime_id, incarnation_id, private_locator, observed_at)),
                Some((
                    identity_runtime,
                    raw_fs,
                    raw_volume,
                    raw_file,
                    raw_generation,
                    platform,
                    identity_observed_at,
                )),
            ) => {
                let proposed_identity = commit
                    .file_identity_binding
                    .as_ref()
                    .expect("validated binding pair");
                let existing_locator = storage_core::LocalResourceLocationBindingRecord {
                    location_id: commit.location.location_id.clone(),
                    locator_ref_id,
                    runtime_id,
                    runtime_incarnation_id: incarnation_id,
                    private_locator,
                    observed_at,
                };
                let existing_identity = storage_core::LocalFileIdentityBindingRecord {
                    location_id: commit.location.location_id.clone(),
                    runtime_id: identity_runtime,
                    runtime_incarnation_id: commit.runtime_incarnation_id.clone(),
                    raw_filesystem_instance_id: raw_fs,
                    raw_volume_id: raw_volume,
                    raw_file_id: raw_file,
                    raw_generation,
                    platform_kind: platform,
                    observed_at: identity_observed_at,
                };
                if !private_locator_binding_matches(&existing_locator, locator)
                    || !file_identity_binding_matches(&existing_identity, proposed_identity)
                {
                    return Err(StoreError::Integrity(
                        "current-incarnation WorkspaceRoot binding conflicts".to_owned(),
                    ));
                }
            }
            _ => {
                return Err(StoreError::Integrity(
                    "current-incarnation WorkspaceRoot bindings are incomplete".to_owned(),
                ));
            }
        }
    } else {
        tx.execute("DELETE FROM resource_location_bindings WHERE location_id = ?1 AND runtime_id = ?2 AND runtime_incarnation_id = ?3", params![commit.location.location_id, commit.runtime_id, commit.runtime_incarnation_id]).map_err(map_database_error)?;
        tx.execute("DELETE FROM file_identity_bindings WHERE location_id = ?1 AND runtime_id = ?2 AND runtime_incarnation_id = ?3", params![commit.location.location_id, commit.runtime_id, commit.runtime_incarnation_id]).map_err(map_database_error)?;
    }
    if root_changed {
        let changed = tx.execute(
            "UPDATE workspace_roots SET status = ?1, updated_at = ?2, version = ?3 WHERE workspace_id = ?4 AND workspace_root_id = ?5 AND version = ?6 AND status = ?7",
            params![commit.root.status, commit.root.updated_at, to_sql_i64(commit.root.version, "WorkspaceRoot version")?, commit.root.workspace_id, commit.root.workspace_root_id, to_sql_i64(commit.expected_root_version, "expected WorkspaceRoot version")?, current_root.status],
        ).map_err(map_database_error)?;
        if changed != 1 {
            return Err(StoreError::Conflict {
                expected: Some(commit.expected_root_version),
                actual: None,
            });
        }
    }
    tx.execute(
        "UPDATE resource_locations SET availability = ?1, observed_at = ?2, last_checked_at = ?2 WHERE location_id = ?3 AND resource_id = ?4 AND runtime_id = ?5",
        params![commit.location.availability, commit.location.observed_at, commit.location.location_id, commit.location.resource_id, commit.runtime_id],
    ).map_err(map_database_error)?;
    let mut events = Vec::with_capacity(2);
    if let Some(event) = commit.root_event {
        events.push(insert_domain_event(
            &tx,
            event,
            root_state_ref.expect("validated root state"),
        )?);
    }
    events.push(insert_domain_event(
        &tx,
        commit.location_event.expect("validated location event"),
        resource_state_ref.expect("validated resource state"),
    )?);
    let result = CommittedWorkspaceRootRevalidation {
        root: commit.root,
        location: commit.location,
        events,
    };
    let response_json = String::from_utf8(canonical_json(&result)?).map_err(|_| {
        StoreError::Invalid("WorkspaceRoot revalidation result is invalid".to_owned())
    })?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![commit.request.principal_id, commit.request.request_id, request_digest, response_json, digest(response_json.as_bytes()), result.location.observed_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(result)
}

fn load_workspace_root(
    connection: &Connection,
    workspace_id: &str,
    workspace_root_id: &str,
) -> Result<Option<WorkspaceRootRecord>, StoreError> {
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
    )> = connection
        .query_row(
            "SELECT workspace_root_id, workspace_id, resource_id, location_id, display_name,
                watch_policy, replication_policy, status, added_by_json, created_at,
                updated_at, version
         FROM workspace_roots WHERE workspace_id = ?1 AND workspace_root_id = ?2",
            params![workspace_id, workspace_root_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    row.map(
        |(
            workspace_root_id,
            workspace_id,
            resource_id,
            location_id,
            display_name,
            watch_policy,
            replication_policy,
            status,
            added_by_json,
            created_at,
            updated_at,
            version,
        )| {
            let added_by = serde_json::from_str(&added_by_json).map_err(|error| {
                StoreError::Integrity(format!(
                    "WorkspaceRoot principal projection is invalid: {error}"
                ))
            })?;
            Ok(WorkspaceRootRecord {
                workspace_root_id,
                workspace_id,
                resource_id,
                location_id,
                display_name,
                watch_policy,
                replication_policy,
                status,
                added_by,
                created_at,
                updated_at,
                version: from_sql_i64(version, "WorkspaceRoot version")?,
            })
        },
    )
    .transpose()
}

fn list_workspace_roots_page(
    connection: &Connection,
    workspace_id: &str,
    status: Option<&str>,
    after_created_at: Option<&str>,
    after_workspace_root_id: Option<&str>,
    limit: usize,
) -> Result<Vec<storage_core::WorkspaceRootListRecord>, StoreError> {
    if workspace_id.trim().is_empty()
        || !(1..=101).contains(&limit)
        || after_created_at.is_some() != after_workspace_root_id.is_some()
        || status
            .is_some_and(|value| !matches!(value, "ACTIVE" | "PAUSED" | "REVOKED" | "UNAVAILABLE"))
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot page query is invalid".to_owned(),
        ));
    }
    let mut statement = connection.prepare(
        "SELECT w.workspace_root_id, w.workspace_id, w.resource_id, w.location_id, w.display_name,
                w.watch_policy, w.replication_policy, w.status, w.added_by_json, w.created_at,
                w.updated_at, w.version, l.availability
         FROM workspace_roots w
         JOIN resource_locations l ON l.location_id = w.location_id AND l.resource_id = w.resource_id
         WHERE workspace_id = ?1
           AND (?2 IS NULL OR w.status = ?2)
           AND (?3 IS NULL OR w.created_at < ?3 OR (w.created_at = ?3 AND w.workspace_root_id < ?4))
         ORDER BY w.created_at DESC, w.workspace_root_id DESC
         LIMIT ?5",
    ).map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![
                workspace_id,
                status,
                after_created_at,
                after_workspace_root_id,
                to_sql_i64(limit as u64, "WorkspaceRoot page limit")?,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, String>(12)?,
                ))
            },
        )
        .map_err(map_database_error)?;
    rows.map(|row| {
        let (
            workspace_root_id,
            workspace_id,
            resource_id,
            location_id,
            display_name,
            watch_policy,
            replication_policy,
            status,
            added_by_json,
            created_at,
            updated_at,
            version,
            location_availability,
        ) = row.map_err(map_database_error)?;
        let added_by = serde_json::from_str(&added_by_json).map_err(|error| {
            StoreError::Integrity(format!(
                "WorkspaceRoot principal projection is invalid: {error}"
            ))
        })?;
        Ok(storage_core::WorkspaceRootListRecord {
            root: WorkspaceRootRecord {
                workspace_root_id,
                workspace_id,
                resource_id,
                location_id,
                display_name,
                watch_policy,
                replication_policy,
                status,
                added_by,
                created_at,
                updated_at,
                version: from_sql_i64(version, "WorkspaceRoot version")?,
            },
            location_availability,
        })
    })
    .collect()
}

fn list_workspace_root_revalidation_candidates(
    connection: &Connection,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    after_created_at: Option<&str>,
    after_workspace_root_id: Option<&str>,
    limit: usize,
) -> Result<Vec<WorkspaceRootRevalidationCandidate>, StoreError> {
    if runtime_id.trim().is_empty()
        || runtime_incarnation_id.trim().is_empty()
        || !(1..=101).contains(&limit)
        || after_created_at.is_some() != after_workspace_root_id.is_some()
    {
        return Err(StoreError::Invalid(
            "WorkspaceRoot revalidation page is invalid".to_owned(),
        ));
    }
    let current: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM runtimes WHERE runtime_id = ?1 AND current_incarnation_id = ?2)",
        params![runtime_id, runtime_incarnation_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !current {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let mut statement = connection.prepare(
        "SELECT w.created_at, w.workspace_root_id
         FROM workspace_roots w
         JOIN resource_locations l ON l.location_id = w.location_id AND l.resource_id = w.resource_id
         WHERE l.runtime_id = ?1
           AND w.status <> 'REVOKED'
           AND (?2 IS NULL OR w.created_at < ?2 OR (w.created_at = ?2 AND w.workspace_root_id < ?3))
         ORDER BY w.created_at DESC, w.workspace_root_id DESC
         LIMIT ?4",
    ).map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![
                runtime_id,
                after_created_at,
                after_workspace_root_id,
                to_sql_i64(limit as u64, "WorkspaceRoot revalidation page size")?
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(map_database_error)?;
    let mut identities = Vec::new();
    for row in rows {
        identities.push(row.map_err(map_database_error)?);
    }
    let mut candidates = Vec::with_capacity(identities.len());
    for (_, workspace_root_id) in identities {
        let root = find_workspace_root_by_runtime(connection, runtime_id, &workspace_root_id)?
            .ok_or_else(|| {
                StoreError::Integrity("WorkspaceRoot projection is incomplete".to_owned())
            })?;
        let resource = load_resource_record(connection, &root.workspace_id, &root.resource_id)?
            .ok_or_else(|| {
                StoreError::Integrity("WorkspaceRoot Resource projection is incomplete".to_owned())
            })?;
        let location =
            load_resource_location_record(connection, &root.resource_id, &root.location_id)?
                .ok_or_else(|| {
                    StoreError::Integrity(
                        "WorkspaceRoot location projection is incomplete".to_owned(),
                    )
                })?;
        let bindings = load_workspace_root_revalidation_bindings(
            connection,
            runtime_id,
            runtime_incarnation_id,
            &location,
        )?;
        candidates.push(WorkspaceRootRevalidationCandidate {
            root,
            resource,
            location,
            bindings,
        });
    }
    Ok(candidates)
}

fn find_workspace_root_by_runtime(
    connection: &Connection,
    runtime_id: &str,
    workspace_root_id: &str,
) -> Result<Option<WorkspaceRootRecord>, StoreError> {
    let workspace_id: Option<String> = connection
        .query_row(
            "SELECT w.workspace_id FROM workspace_roots w JOIN resource_locations l
           ON l.location_id = w.location_id AND l.resource_id = w.resource_id
         WHERE w.workspace_root_id = ?1 AND l.runtime_id = ?2 AND w.status <> 'REVOKED'",
            params![workspace_root_id, runtime_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    match workspace_id {
        Some(workspace_id) => load_workspace_root(connection, &workspace_id, workspace_root_id),
        None => Ok(None),
    }
}

fn load_resource_record(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
) -> Result<Option<ResourceRecord>, StoreError> {
    let row: Option<(String, String, Option<String>, String, Option<String>, String, String, String, String, i64)> = connection.query_row(
        "SELECT kind, provider_identity_json, identity_digest, display_name, current_revision_id,
                sensitivity, provenance_json, created_at, updated_at, version
         FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
        params![workspace_id, resource_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?)),
    ).optional().map_err(map_database_error)?;
    row.map(
        |(
            kind,
            provider_identity,
            identity_digest,
            display_name,
            current_revision_id,
            sensitivity,
            provenance,
            created_at,
            updated_at,
            version,
        )| {
            Ok(ResourceRecord {
                resource_id: resource_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                kind,
                provider_identity: serde_json::from_str(&provider_identity).map_err(|_| {
                    StoreError::CorruptSchema("Resource identity projection is invalid".to_owned())
                })?,
                identity_digest,
                display_name,
                current_revision_id,
                sensitivity,
                provenance: serde_json::from_str(&provenance).map_err(|_| {
                    StoreError::CorruptSchema(
                        "Resource provenance projection is invalid".to_owned(),
                    )
                })?,
                created_at,
                updated_at,
                version: from_sql_i64(version, "Resource version")?,
            })
        },
    )
    .transpose()
}

fn load_resource_detail_record(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
) -> Result<Option<storage_core::ResourceDetailRecord>, StoreError> {
    let Some(resource) = load_resource_record(connection, workspace_id, resource_id)? else {
        return Ok(None);
    };
    let context_document_json: Option<String> = connection.query_row(
        "SELECT context_document_json FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
        params![workspace_id, resource_id],
        |row| row.get(0),
    ).optional().map_err(map_database_error)?.flatten();
    let context_document = context_document_json
        .map(|value| {
            serde_json::from_str(&value).map_err(|_| {
                StoreError::CorruptSchema("ContextDocument metadata is invalid".to_owned())
            })
        })
        .transpose()?;
    Ok(Some(storage_core::ResourceDetailRecord {
        resource,
        context_document,
    }))
}

fn list_resource_revision_records_page(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
    after_revision_id: Option<&str>,
    limit: usize,
) -> Result<Vec<storage_core::ResourceRevisionViewRecord>, StoreError> {
    if !(1..=201).contains(&limit) || after_revision_id.is_some_and(|value| value.trim().is_empty())
    {
        return Err(StoreError::Invalid(
            "Resource revision page query is invalid".to_owned(),
        ));
    }
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM resources WHERE workspace_id = ?1 AND resource_id = ?2)",
            params![workspace_id, resource_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !exists {
        return Err(StoreError::NotFound);
    }
    let after_rowid = if let Some(cursor_revision_id) = after_revision_id {
        connection.query_row(
            "SELECT rowid FROM resource_revisions WHERE resource_id = ?1 AND resource_revision_id = ?2",
            params![resource_id, cursor_revision_id],
            |row| row.get::<_, i64>(0),
        ).optional().map_err(map_database_error)?
            .ok_or_else(|| StoreError::Invalid("Resource revision cursor is no longer valid".to_owned()))?
    } else {
        0
    };
    let mut statement = connection.prepare(
        "SELECT revision.rowid, revision.resource_revision_id, revision.provider_revision, revision.content_digest,
                revision.size_bytes, revision.media_type, revision.observed_at, revision.created_by_json,
                NOT EXISTS(SELECT 1 FROM resource_revision_parents child
                           WHERE child.resource_id = revision.resource_id
                             AND child.parent_revision_id = revision.resource_revision_id) AS is_head
         FROM resource_revisions revision
         WHERE revision.resource_id = ?1 AND revision.rowid > ?2
         ORDER BY revision.rowid
         LIMIT ?3"
    ).map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![
                resource_id,
                after_rowid,
                to_sql_i64(limit as u64, "Resource revision page size")?
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, bool>(8)?,
                ))
            },
        )
        .map_err(map_database_error)?;
    let mut result = Vec::new();
    for row in rows {
        let (
            _rowid,
            revision_id,
            provider_revision,
            content_digest,
            size_bytes,
            media_type,
            observed_at,
            created_by,
            is_head,
        ) = row.map_err(map_database_error)?;
        let mut parents_statement = connection.prepare(
            "SELECT parent_revision_id FROM resource_revision_parents WHERE resource_id = ?1 AND child_revision_id = ?2 ORDER BY parent_revision_id"
        ).map_err(map_database_error)?;
        let parents = parents_statement
            .query_map(params![resource_id, revision_id], |row| {
                row.get::<_, String>(0)
            })
            .map_err(map_database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?;
        result.push(storage_core::ResourceRevisionViewRecord {
            revision: ResourceRevisionRecord {
                resource_revision_id: revision_id,
                resource_id: resource_id.to_owned(),
                parent_revision_ids: parents,
                provider_revision,
                content_digest,
                size_bytes: size_bytes
                    .map(|value| from_sql_i64(value, "Resource revision size"))
                    .transpose()?,
                media_type,
                observed_at,
                created_by: serde_json::from_str(&created_by).map_err(|_| {
                    StoreError::CorruptSchema("Resource revision author is invalid".to_owned())
                })?,
            },
            is_head,
        });
    }
    Ok(result)
}

fn load_resource_location_record(
    connection: &Connection,
    resource_id: &str,
    location_id: &str,
) -> Result<Option<ResourceLocationRecord>, StoreError> {
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        i64,
        Option<String>,
        Option<String>,
        String,
    )> = connection
        .query_row(
            "SELECT resource_id, runtime_id, locator_ref_id, provider_ref, availability, writable,
                observed_revision_id, observed_digest, observed_at
         FROM resource_locations WHERE location_id = ?1 AND resource_id = ?2",
            params![location_id, resource_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    row.map(
        |(
            resource_id,
            runtime_id,
            locator_ref_id,
            provider_ref,
            availability,
            writable,
            observed_revision_id,
            observed_digest,
            observed_at,
        )| {
            Ok(ResourceLocationRecord {
                location_id: location_id.to_owned(),
                resource_id,
                runtime_id,
                locator_ref_id,
                provider_ref,
                availability,
                writable: writable != 0,
                observed_revision_id,
                observed_digest,
                observed_at,
            })
        },
    )
    .transpose()
}

fn load_workspace_root_revalidation_bindings(
    connection: &Connection,
    runtime_id: &str,
    current_incarnation_id: &str,
    location: &ResourceLocationRecord,
) -> Result<WorkspaceRootRevalidationBindings, StoreError> {
    if location.runtime_id != runtime_id {
        return Ok(WorkspaceRootRevalidationBindings::Unavailable(
            WorkspaceRootRevalidationFailure::BindingMismatch,
        ));
    }
    let source_incarnation: Option<String> = connection
        .query_row(
            "SELECT runtime_incarnation_id FROM (
           SELECT b.runtime_incarnation_id, i.process_started_at
           FROM resource_location_bindings b JOIN runtime_incarnations i
             ON i.runtime_id = b.runtime_id AND i.runtime_incarnation_id = b.runtime_incarnation_id
           WHERE b.location_id = ?1 AND b.runtime_id = ?2
           UNION
           SELECT b.runtime_incarnation_id, i.process_started_at
           FROM file_identity_bindings b JOIN runtime_incarnations i
             ON i.runtime_id = b.runtime_id AND i.runtime_incarnation_id = b.runtime_incarnation_id
           WHERE b.location_id = ?1 AND b.runtime_id = ?2
         ) ORDER BY process_started_at DESC, runtime_incarnation_id DESC LIMIT 1",
            params![location.location_id, runtime_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some(source_incarnation) = source_incarnation else {
        return Ok(WorkspaceRootRevalidationBindings::Unavailable(
            WorkspaceRootRevalidationFailure::NoPriorBinding,
        ));
    };
    let locator: Option<(String, String, String, String, String)> = connection.query_row(
        "SELECT locator_ref_id, runtime_id, runtime_incarnation_id, private_locator, observed_at
         FROM resource_location_bindings WHERE location_id = ?1 AND runtime_id = ?2 AND runtime_incarnation_id = ?3",
        params![location.location_id, runtime_id, source_incarnation],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).optional().map_err(map_database_error)?;
    let identity: Option<(String, Option<String>, String, Option<String>, String, String, String, String)> = connection.query_row(
        "SELECT runtime_id, raw_volume_id, raw_filesystem_instance_id, raw_generation,
                raw_file_id, platform_kind, runtime_incarnation_id, observed_at
         FROM file_identity_bindings WHERE location_id = ?1 AND runtime_id = ?2 AND runtime_incarnation_id = ?3",
        params![location.location_id, runtime_id, source_incarnation],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
    ).optional().map_err(map_database_error)?;
    let Some((
        locator_ref_id,
        locator_runtime_id,
        locator_incarnation,
        private_locator,
        locator_observed_at,
    )) = locator
    else {
        let failure = if identity.is_some() {
            WorkspaceRootRevalidationFailure::LocatorBindingMissing
        } else {
            WorkspaceRootRevalidationFailure::NoPriorBinding
        };
        return Ok(WorkspaceRootRevalidationBindings::Unavailable(failure));
    };
    let Some((
        identity_runtime_id,
        raw_volume_id,
        raw_filesystem_instance_id,
        raw_generation,
        raw_file_id,
        platform_kind,
        identity_incarnation,
        identity_observed_at,
    )) = identity
    else {
        return Ok(WorkspaceRootRevalidationBindings::Unavailable(
            WorkspaceRootRevalidationFailure::FileIdentityBindingMissing,
        ));
    };
    if locator_ref_id != location.locator_ref_id
        || locator_runtime_id != runtime_id
        || locator_incarnation != source_incarnation
        || locator_observed_at.trim().is_empty()
        || identity_runtime_id != runtime_id
        || identity_incarnation != source_incarnation
        || identity_observed_at.trim().is_empty()
        || private_locator.trim().is_empty()
    {
        return Ok(WorkspaceRootRevalidationBindings::Unavailable(
            WorkspaceRootRevalidationFailure::BindingMismatch,
        ));
    }
    let locator = storage_core::LocalResourceLocationBindingRecord {
        location_id: location.location_id.clone(),
        locator_ref_id,
        runtime_id: locator_runtime_id,
        runtime_incarnation_id: locator_incarnation,
        private_locator,
        observed_at: locator_observed_at,
    };
    let file_identity = storage_core::LocalFileIdentityBindingRecord {
        location_id: location.location_id.clone(),
        runtime_id: identity_runtime_id,
        runtime_incarnation_id: identity_incarnation,
        raw_filesystem_instance_id,
        raw_volume_id,
        raw_file_id,
        raw_generation,
        platform_kind,
        observed_at: identity_observed_at,
    };
    if source_incarnation == current_incarnation_id {
        Ok(WorkspaceRootRevalidationBindings::CurrentIncarnation {
            locator,
            file_identity,
        })
    } else {
        Ok(WorkspaceRootRevalidationBindings::Previous {
            locator,
            file_identity,
        })
    }
}

fn create_resource_transaction(
    connection: &mut Connection,
    request: WorkspaceCreateRequest,
    resource: ResourceRecord,
    revision: ResourceRevisionRecord,
    draft: EventDraft,
    state_ref: AggregateStateRef,
    content_blob: storage_core::BlobRef,
    text_index: Option<PreparedResourceTextIndex>,
    location_id: String,
    upload_commit: Option<UploadCommit>,
    context_document: Option<Value>,
) -> Result<CommittedResource, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let request_digest = digest(&canonical_json(&request.request_payload)?);
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![request.principal_id, request.request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((prior_digest, response_json, response_digest)) = prior {
        if prior_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response_json = response_json.ok_or_else(|| {
            StoreError::Integrity("Resource idempotency receipt is incomplete".to_owned())
        })?;
        if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "Resource idempotency response digest does not match".to_owned(),
            ));
        }
        return serde_json::from_str(&response_json)
            .map_err(|error| StoreError::Integrity(error.to_string()));
    }
    let upload_id = request
        .request_payload
        .get("upload_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            StoreError::Invalid("Resource upload commit identity is missing".to_owned())
        })?;
    let upload_state: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT state, committed_resource_id FROM resource_upload_sessions WHERE upload_id = ?1 AND workspace_id = ?2",
            params![upload_id, resource.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    if let Some((state, committed_resource_id)) = upload_state {
        if state == "COMMITTED" {
            return Err(StoreError::LegacyUploadCommitNeedsReview {
                committed_resource_id,
            });
        }
    }
    let workspace_status: Option<String> = tx
        .query_row(
            "SELECT status FROM workspaces WHERE workspace_id = ?1",
            [&resource.workspace_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    match workspace_status.as_deref() {
        Some("ACTIVE") => {}
        Some(_) => {
            return Err(StoreError::Invalid(
                "archived Workspace is read-only".to_owned(),
            ));
        }
        None => return Err(StoreError::NotFound),
    }
    let revision_session = upload_commit.as_ref().and_then(|commit| {
        commit
            .committed_session
            .resource_id
            .as_ref()
            .map(|id| (commit, id))
    });
    if let Some((upload, revision_resource_id)) = revision_session {
        let session = &upload.committed_session;
        let expected_version = session.expected_resource_version.ok_or_else(|| {
            StoreError::Invalid(
                "revision upload is missing its expected Resource version".to_owned(),
            )
        })?;
        if revision_resource_id != &resource.resource_id
            || resource.version
                != expected_version
                    .checked_add(1)
                    .ok_or_else(|| StoreError::Invalid("Resource version overflow".to_owned()))?
            || revision.parent_revision_ids != session.parent_revision_ids
            || resource.current_revision_id.as_deref()
                != Some(revision.resource_revision_id.as_str())
            || draft.event_type != "resource.revision.created.v1"
            || draft.payload.get("resource_id").and_then(Value::as_str)
                != Some(resource.resource_id.as_str())
            || draft
                .payload
                .get("resource_revision_id")
                .and_then(Value::as_str)
                != Some(revision.resource_revision_id.as_str())
            || draft.payload.get("parent_revision_ids").cloned()
                != Some(json!(revision.parent_revision_ids))
            || draft.payload.get("content_digest").and_then(Value::as_str)
                != revision.content_digest.as_deref()
            || draft.payload.get("size_bytes").and_then(Value::as_u64) != revision.size_bytes
            || draft.payload.get("media_type").and_then(Value::as_str)
                != revision.media_type.as_deref()
            || draft.payload.get("created_by") != Some(&revision.created_by)
            || draft
                .payload
                .get("aggregate_version")
                .and_then(Value::as_u64)
                != Some(resource.version)
        {
            return Err(StoreError::Invalid(
                "revision commit does not match its pinned upload".to_owned(),
            ));
        }
        let current_resource =
            load_resource_record(&tx, &resource.workspace_id, &resource.resource_id)?
                .ok_or(StoreError::NotFound)?;
        if current_resource.version != expected_version {
            return Err(StoreError::Conflict {
                expected: Some(expected_version),
                actual: Some(current_resource.version),
            });
        }
        if current_resource.kind != resource.kind
            || current_resource.provider_identity != resource.provider_identity
            || current_resource.identity_digest != resource.identity_digest
            || current_resource.display_name != resource.display_name
            || current_resource.sensitivity != resource.sensitivity
            || current_resource.provenance != resource.provenance
            || current_resource.created_at != resource.created_at
            || resource.updated_at != revision.observed_at
        {
            return Err(StoreError::Invalid(
                "revision commit attempted to change immutable Resource metadata".to_owned(),
            ));
        }
        let context_state: Option<String> = tx.query_row(
            "SELECT json_extract(context_document_json, '$.status') FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
            params![resource.workspace_id, resource.resource_id],
            |row| row.get(0),
        ).optional().map_err(map_database_error)?.flatten();
        if context_state
            .as_deref()
            .is_some_and(|status| status != "ACTIVE")
        {
            return Err(StoreError::Invalid(
                "ContextDocument is not active".to_owned(),
            ));
        }
        let mut head_statement = tx.prepare(
            "SELECT candidate.resource_revision_id FROM resource_revisions candidate
             WHERE candidate.resource_id = ?1 AND NOT EXISTS (
               SELECT 1 FROM resource_revision_parents edge
               WHERE edge.resource_id = candidate.resource_id AND edge.parent_revision_id = candidate.resource_revision_id
             ) ORDER BY candidate.resource_revision_id"
        ).map_err(map_database_error)?;
        let heads = head_statement
            .query_map([&resource.resource_id], |row| row.get::<_, String>(0))
            .map_err(map_database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?;
        let mut pinned_heads = session.parent_revision_ids.clone();
        pinned_heads.sort();
        if heads != pinned_heads {
            return Err(StoreError::Conflict {
                expected: Some(expected_version),
                actual: Some(current_resource.version),
            });
        }
        tx.execute(
            "INSERT INTO resource_revisions(resource_revision_id, resource_id, provider_revision, content_digest, size_bytes, media_type, observed_at, created_by_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![revision.resource_revision_id, revision.resource_id, revision.provider_revision, revision.content_digest, revision.size_bytes.map(|n| to_sql_i64(n, "Resource size")).transpose()?, revision.media_type, revision.observed_at, String::from_utf8(canonical_json(&revision.created_by)?).map_err(|e| StoreError::Invalid(e.to_string()))?],
        ).map_err(map_database_error)?;
        for parent_id in &revision.parent_revision_ids {
            tx.execute(
                "INSERT INTO resource_revision_parents(resource_id, child_revision_id, parent_revision_id) VALUES (?1, ?2, ?3)",
                params![resource.resource_id, revision.resource_revision_id, parent_id],
            ).map_err(map_database_error)?;
        }
        let changed = tx.execute(
            "UPDATE resources SET current_revision_id = ?1, updated_at = ?2, version = ?3 WHERE workspace_id = ?4 AND resource_id = ?5 AND version = ?6",
            params![revision.resource_revision_id, resource.updated_at, to_sql_i64(resource.version, "Resource version")?, resource.workspace_id, resource.resource_id, to_sql_i64(expected_version, "expected Resource version")?],
        ).map_err(map_database_error)?;
        if changed != 1 {
            return Err(StoreError::Conflict {
                expected: Some(expected_version),
                actual: None,
            });
        }
        let changed_location = tx.execute(
            "UPDATE resource_locations SET locator_ref_id = ?1, observed_revision_id = ?2, observed_digest = ?3, observed_at = ?4, last_checked_at = ?4 WHERE location_id = ?5 AND resource_id = ?6 AND provider_ref = 'litecowork.encrypted_blob' AND availability = 'AVAILABLE' AND runtime_id IS NULL AND environment_id IS NULL AND connection_id IS NULL",
            params![content_blob.digest, revision.resource_revision_id, revision.content_digest, revision.observed_at, location_id, resource.resource_id],
        ).map_err(map_database_error)?;
        if changed_location != 1 {
            return Err(StoreError::Invalid(
                "Resource has no available managed local content location to revise".to_owned(),
            ));
        }
        let invalidation_edges: Vec<String> = {
            let mut statement = tx.prepare(
                "SELECT dependency_edge_id FROM dependency_edges WHERE source_resource_id = ?1 AND source_revision_id <> ?2 ORDER BY dependency_edge_id"
            ).map_err(map_database_error)?;
            let rows = statement
                .query_map(
                    params![resource.resource_id, revision.resource_revision_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_database_error)?
        };
        for dependency_edge_id in invalidation_edges {
            let invalidation_id = format!(
                "invalidation-{}",
                digest(&canonical_json(&json!([
                    dependency_edge_id,
                    revision.resource_revision_id
                ]))?)
                .trim_start_matches("sha256:")
            );
            tx.execute(
                "INSERT OR IGNORE INTO invalidation_records(invalidation_record_id, dependency_edge_id, observed_revision_id, reason_code, created_at) VALUES (?1, ?2, ?3, 'SOURCE_RESOURCE_REVISION_ADVANCED', ?4)",
                params![invalidation_id, dependency_edge_id, revision.resource_revision_id, revision.observed_at],
            ).map_err(map_database_error)?;
        }
    } else {
        tx.execute(
            "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, identity_digest, display_name, current_revision_id, sensitivity, context_document_json, provenance_json, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![resource.resource_id, resource.workspace_id, resource.kind, String::from_utf8(canonical_json(&resource.provider_identity)?).map_err(|e| StoreError::Invalid(e.to_string()))?, resource.identity_digest, resource.display_name, revision.resource_revision_id, resource.sensitivity, context_document.as_ref().map(canonical_json).transpose()?.map(String::from_utf8).transpose().map_err(|error| StoreError::Invalid(error.to_string()))?, String::from_utf8(canonical_json(&resource.provenance)?).map_err(|e| StoreError::Invalid(e.to_string()))?, resource.created_at, resource.updated_at, to_sql_i64(resource.version, "Resource version")?],
        ).map_err(map_database_error)?;
        tx.execute(
            "INSERT INTO resource_revisions(resource_revision_id, resource_id, provider_revision, content_digest, size_bytes, media_type, observed_at, created_by_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![revision.resource_revision_id, revision.resource_id, revision.provider_revision, revision.content_digest, revision.size_bytes.map(|n| to_sql_i64(n, "Resource size")).transpose()?, revision.media_type, revision.observed_at, String::from_utf8(canonical_json(&revision.created_by)?).map_err(|e| StoreError::Invalid(e.to_string()))?],
        ).map_err(map_database_error)?;
    }
    if let Some(index) = text_index.as_ref() {
        if index.workspace_id != resource.workspace_id
            || index.resource_id != resource.resource_id
            || index.resource_revision_id != revision.resource_revision_id
            || index.source_content_digest != revision.content_digest.as_deref().unwrap_or_default()
            || index.extracted_text.size_bytes != revision.size_bytes.unwrap_or_default()
        {
            return Err(StoreError::Integrity(
                "prepared Resource index does not match the committed revision".to_owned(),
            ));
        }
        resource_index::insert_prepared(&tx, index)?;
    }
    if revision_session.is_none() {
        tx.execute(
            "INSERT INTO resource_locations(location_id, resource_id, runtime_id, environment_id, connection_id, provider_ref, locator_ref_id, availability, writable, observed_revision_id, observed_digest, observed_at, last_checked_at) VALUES (?1, ?2, NULL, NULL, NULL, 'litecowork.encrypted_blob', ?3, 'AVAILABLE', 0, ?4, ?5, ?6, ?6)",
            params![location_id, resource.resource_id, content_blob.digest, revision.resource_revision_id, revision.content_digest, revision.observed_at],
        ).map_err(map_database_error)?;
    }
    tx.execute(
        "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1",
        params![draft.workspace_id, draft.origin_runtime_id],
    ).map_err(map_database_error)?;
    let origin_sequence_sql: i64 = tx.query_row("SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2", params![draft.workspace_id, draft.origin_runtime_id], |row| row.get(0)).map_err(map_database_error)?;
    let payload_json = String::from_utf8(canonical_json(&draft.payload)?)
        .map_err(|e| StoreError::Invalid(e.to_string()))?;
    let event = DomainEvent {
        event_id: draft.event_id,
        workspace_id: draft.workspace_id,
        entity_type: draft.entity_type,
        entity_id: draft.entity_id,
        origin_runtime_id: draft.origin_runtime_id,
        origin_sequence: from_sql_i64(origin_sequence_sql, "origin sequence")?,
        entity_revision: draft.entity_revision,
        hlc_timestamp: draft.hlc_timestamp,
        correlation_id: draft.correlation_id,
        causation_id: draft.causation_id,
        schema_version: draft.schema_version,
        event_type: draft.event_type,
        payload: draft.payload,
        aggregate_state_ref: state_ref,
        recorded_at: draft.recorded_at,
        payload_digest: digest(payload_json.as_bytes()),
    };
    tx.execute(
        "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![event.event_id, event.workspace_id, event.entity_type, event.entity_id, event.origin_runtime_id, to_sql_i64(event.origin_sequence, "origin sequence")?, to_sql_i64(event.entity_revision, "event revision")?, event.hlc_timestamp, event.correlation_id, event.causation_id, i64::from(event.schema_version), event.event_type, payload_json, String::from_utf8(canonical_json(&event.aggregate_state_ref)?).map_err(|e| StoreError::Invalid(e.to_string()))?, event.recorded_at, event.payload_digest],
    ).map_err(map_database_error)?;
    if let Some(upload_commit) = upload_commit {
        let status_event = &upload_commit.status_event;
        let committed_session = &upload_commit.committed_session;
        let current =
            load_resource_upload(&tx, &resource.workspace_id, &committed_session.upload_id)?
                .ok_or(StoreError::NotFound)?;
        if current.state != ResourceUploadState::ContentReceived
            || current.version != upload_commit.expected_version
            || current.progress_version != upload_commit.expected_progress_version
            || current.workspace_id != resource.workspace_id
            || committed_session.state != ResourceUploadState::Committed
            || committed_session.version != current.version.saturating_add(1)
            || committed_session.progress_version != current.progress_version
            || committed_session.committed_resource_id.as_deref()
                != Some(resource.resource_id.as_str())
            || committed_session.expected_size_bytes != revision.size_bytes.unwrap_or_default()
            || status_event.entity_revision != committed_session.version
            || upload_commit.state_ref.entity_revision != committed_session.version
            || status_event.workspace_id != resource.workspace_id
            || status_event.entity_type != "ResourceUpload"
            || status_event.entity_id != committed_session.upload_id
            || status_event.event_type != "resource.upload.status.changed.v1"
            || status_event
                .payload
                .get("upload_id")
                .and_then(Value::as_str)
                != Some(committed_session.upload_id.as_str())
            || status_event.payload.get("from").and_then(Value::as_str) != Some("CONTENT_RECEIVED")
            || status_event.payload.get("to").and_then(Value::as_str) != Some("COMMITTED")
            || status_event
                .payload
                .get("resource_id")
                .and_then(Value::as_str)
                != Some(resource.resource_id.as_str())
            || status_event
                .payload
                .get("aggregate_version")
                .and_then(Value::as_u64)
                != Some(committed_session.version)
        {
            return Err(StoreError::Conflict {
                expected: Some(upload_commit.expected_progress_version),
                actual: Some(current.progress_version),
            });
        }
        let changed = tx.execute(
            "UPDATE resource_upload_sessions SET state = 'COMMITTED', committed_resource_id = ?1, version = ?2 WHERE upload_id = ?3 AND workspace_id = ?4 AND state = 'CONTENT_RECEIVED' AND version = ?5 AND progress_version = ?6 AND expected_size_bytes = ?7",
            params![resource.resource_id, to_sql_i64(committed_session.version, "upload lifecycle version")?, committed_session.upload_id, resource.workspace_id, to_sql_i64(upload_commit.expected_version, "expected upload lifecycle version")?, to_sql_i64(upload_commit.expected_progress_version, "expected upload progress version")?, to_sql_i64(revision.size_bytes.unwrap_or_default(), "Resource size")?],
        ).map_err(map_database_error)?;
        if changed != 1 {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let persisted =
            load_resource_upload(&tx, &resource.workspace_id, &committed_session.upload_id)?
                .ok_or(StoreError::NotFound)?;
        if persisted != *committed_session {
            return Err(StoreError::Integrity(
                "committed upload state does not match its aggregate snapshot".to_owned(),
            ));
        }
        insert_upload_domain_event(&tx, upload_commit.status_event, upload_commit.state_ref)?;
    }
    let committed = CommittedResource {
        resource,
        revision,
        event,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|e| StoreError::Invalid(e.to_string()))?;
    let response_digest = digest(response_json.as_bytes());
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![request.principal_id, request.request_id, request_digest, response_json, response_digest, committed.event.recorded_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn validate_task_create_commit(commit: &TaskCreateCommit) -> Result<(), StoreError> {
    let task = &commit.task;
    let spec = &commit.initial_spec_revision;
    let event = &commit.event;
    let request_object = commit.request.request_payload.as_object().ok_or_else(|| {
        StoreError::Invalid("CreateTask request payload must be an object".to_owned())
    })?;
    match request_object.get("deadline") {
        None | Some(Value::Null) => {}
        Some(Value::String(value)) => {
            canonicalize_utc_timestamp(value)?;
        }
        Some(_) => {
            return Err(StoreError::Invalid(
                "Task deadline must be a timestamp or null".to_owned(),
            ));
        }
    }
    let request_coworker_id = match request_object.get("coworker_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.as_str()),
        Some(_) => {
            return Err(StoreError::Invalid(
                "Coworker ID must be a string or null".to_owned(),
            ));
        }
    };
    let request_expected_coworker_version = match request_object.get("expected_coworker_version") {
        None | Some(Value::Null) => None,
        Some(Value::Number(value)) => value.as_u64(),
        Some(_) => None,
    };
    if request_object
        .get("expected_coworker_version")
        .is_some_and(|value| !value.is_null() && request_expected_coworker_version.is_none())
        || request_expected_coworker_version.is_some_and(|version| version == 0)
    {
        return Err(StoreError::Invalid(
            "expected_coworker_version must be a positive integer or null".to_owned(),
        ));
    }
    if matches!(
        request_object.get("lead_failover_policy"),
        Some(Value::Null)
    ) {
        return Err(StoreError::Invalid(
            "lead_failover_policy must be omitted or an object".to_owned(),
        ));
    }
    let conversation_requested = request_object
        .get("conversation_id")
        .is_some_and(|value| !value.is_null())
        || request_object
            .get("source_message_refs")
            .is_some_and(|value| value != &json!([]))
        || event.payload.get("conversation_id").is_some()
        || task.conversation_id.is_some()
        || !spec.source_message_refs.is_empty();
    if conversation_requested {
        return Err(StoreError::Invalid(
            "Conversation-origin Task creation is unavailable until its ConversationMessage can be appended atomically".to_owned(),
        ));
    }
    const CREATE_TASK_FIELDS: &[&str] = &[
        "workspace_id",
        "conversation_id",
        "source_message_refs",
        "objective",
        "task_category",
        "constraints",
        "non_goals",
        "input_refs",
        "required_outputs",
        "acceptance_criteria",
        "approvals_required",
        "budget",
        "delegation_budget_policy",
        "lead_failover_policy",
        "deadline",
        "placement_preference",
        "preferred_lead_agent_binding_id",
        "coworker_id",
        "expected_coworker_version",
        "routine_id",
        "routine_revision",
        "routine_inputs",
        "automation_id",
        "automation_revision",
        "expected_automation_version",
        "trigger_id",
        "occurrence_id",
    ];
    if request_object
        .keys()
        .any(|field| !CREATE_TASK_FIELDS.contains(&field.as_str()))
    {
        return Err(StoreError::Invalid(
            "CreateTask request payload contains an unknown field".to_owned(),
        ));
    }
    let principal = task.created_by.get("principal_id").and_then(Value::as_str);
    let principal_kind = task.created_by.get("kind").and_then(Value::as_str);
    if commit.request.principal_id.trim().is_empty()
        || commit.request.request_id.trim().is_empty()
        || task.task_id.trim().is_empty()
        || task.workspace_id.trim().is_empty()
        || task.lead_agent_binding_id.trim().is_empty()
        || task.status != "READY"
        || task.current_spec_revision != 1
        || task.current_plan_revision.is_some()
        || task.resume_status.is_some()
        || task.routine_id.is_some() != task.routine_revision.is_some()
        || task.automation_id.is_some() != task.automation_occurrence_id.is_some()
        || task.version != 1
        || task.blocking_conditions.len() != 0
        || task.priority != "NORMAL"
        || task.created_at != task.updated_at
        || event.recorded_at != task.created_at
        || task.completed_at.is_some()
        || spec.task_id != task.task_id
        || spec.workspace_id != task.workspace_id
        || spec.revision != 1
        || !spec.parent_revisions.is_empty()
        || spec.objective.trim().is_empty()
        || spec.preferred_lead_agent_binding_id.as_deref()
            != Some(task.lead_agent_binding_id.as_str())
        || spec.authored_by != task.created_by
        || spec.created_at != task.created_at
        || principal != Some(commit.request.principal_id.as_str())
        || principal_kind != Some("USER")
        || request_object.get("workspace_id").and_then(Value::as_str)
            != Some(task.workspace_id.as_str())
        || request_object.get("objective").and_then(Value::as_str) != Some(spec.objective.as_str())
        || request_coworker_id != task.origin_coworker_id.as_deref()
        || request_expected_coworker_version != commit.expected_coworker_version
        || event.workspace_id != task.workspace_id
        || event.entity_type != "Task"
        || event.entity_id != task.task_id
        || event.entity_revision != task.version
        || event.schema_version != 1
        || event.event_type != "task.created.v1"
        || event.payload.get("task_id").and_then(Value::as_str) != Some(task.task_id.as_str())
        || event
            .payload
            .get("initial_spec_revision")
            .and_then(Value::as_u64)
            != Some(1)
        || event.payload.get("created_by") != Some(&task.created_by)
        || event
            .payload
            .get("origin_coworker_id")
            .and_then(Value::as_str)
            != task.origin_coworker_id.as_deref()
        || event
            .payload
            .get("origin_coworker_revision")
            .and_then(Value::as_u64)
            != task.origin_coworker_revision
        || event.payload.get("routine_id").and_then(Value::as_str) != task.routine_id.as_deref()
        || event
            .payload
            .get("routine_revision")
            .and_then(Value::as_u64)
            != task.routine_revision
        || event.payload.get("automation_id").and_then(Value::as_str)
            != task.automation_id.as_deref()
        || event
            .payload
            .get("automation_occurrence_id")
            .and_then(Value::as_str)
            != task.automation_occurrence_id.as_deref()
    {
        return Err(StoreError::Invalid(
            "Task create payload is inconsistent with the initial Task contract".to_owned(),
        ));
    }
    if let Some(expected_version) = commit.expected_coworker_version {
        if task.origin_coworker_id.is_none() || expected_version == 0 {
            return Err(StoreError::Invalid(
                "expected Coworker version requires a selected Coworker".to_owned(),
            ));
        }
    }
    if task.origin_coworker_id.is_some() != task.origin_coworker_revision.is_some() {
        return Err(StoreError::Invalid(
            "Task Coworker identity and revision must be supplied together".to_owned(),
        ));
    }
    if commit.routine_admission.as_ref().is_some_and(|admission| {
        Some(admission.routine_id.as_str()) != task.routine_id.as_deref()
            || Some(admission.routine_revision) != task.routine_revision
    }) || commit.routine_admission.is_none() && task.routine_id.is_some()
    {
        return Err(StoreError::Invalid(
            "Routine provenance requires exact atomic admission".to_owned(),
        ));
    }
    match (
        &commit.automation_admission,
        task.automation_id.as_deref(),
        task.automation_occurrence_id.as_deref(),
    ) {
        (None, None, None) => {
            if [
                "automation_id",
                "automation_revision",
                "expected_automation_version",
                "trigger_id",
                "occurrence_id",
            ]
            .iter()
            .any(|field| request_object.contains_key(*field))
            {
                return Err(StoreError::Invalid(
                    "Automation provenance requires atomic occurrence admission".to_owned(),
                ));
            }
        }
        (Some(admission), Some(task_automation_id), Some(task_occurrence_id)) => {
            if admission.automation_id != task_automation_id
                || admission.occurrence_id != task_occurrence_id
                || admission.automation_revision == 0
                || admission.routine_id != task.routine_id.as_deref().unwrap_or_default()
                || Some(admission.routine_revision) != task.routine_revision
                || request_object.get("automation_id").and_then(Value::as_str)
                    != Some(admission.automation_id.as_str())
                || request_object
                    .get("automation_revision")
                    .and_then(Value::as_u64)
                    != Some(admission.automation_revision)
                || request_object
                    .get("expected_automation_version")
                    .and_then(Value::as_u64)
                    != Some(admission.expected_automation_version)
                || request_object.get("trigger_id").and_then(Value::as_str)
                    != Some(admission.trigger_id.as_str())
                || request_object.get("occurrence_id").and_then(Value::as_str)
                    != Some(admission.occurrence_id.as_str())
                || admission.trigger_host_runtime_id.trim().is_empty()
                || admission
                    .trigger_host_runtime_incarnation_id
                    .trim()
                    .is_empty()
                || admission.expected_automation_version == 0
                || admission.trigger_host_binding_version == 0
                || admission.occurrence_key.len() != 64
                || !admission
                    .occurrence_key
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err(StoreError::Invalid(
                    "Automation occurrence provenance is inconsistent".to_owned(),
                ));
            }
        }
        _ => {
            return Err(StoreError::Invalid(
                "Automation and occurrence identity must be supplied with atomic admission"
                    .to_owned(),
            ));
        }
    }
    Ok(())
}

/// Validates the identity and current read eligibility of every pinned Task input while
/// the caller's Task admission transaction is still open. ContextDocument state may
/// change after this commit, so ResourceResolver must repeat the status check when the
/// pinned bytes are actually requested.
fn validate_task_resource_inputs(
    connection: &Connection,
    workspace_id: &str,
    inputs: &[Value],
) -> Result<(), StoreError> {
    let mut seen = std::collections::HashSet::new();
    for input in inputs {
        let pin: PinnedResourceRef = serde_json::from_value(input.clone()).map_err(|_| {
            StoreError::Invalid(
                "Task inputs must be unique, pinned Resource revisions in this Workspace"
                    .to_owned(),
            )
        })?;
        if pin.workspace_id != workspace_id
            || pin.resource_id.is_empty()
            || pin.revision_id.is_empty()
            || !seen.insert((pin.resource_id.clone(), pin.revision_id.clone()))
        {
            return Err(StoreError::Invalid(
                "Task inputs must be unique, pinned Resource revisions in this Workspace"
                    .to_owned(),
            ));
        }
        let resource_context_document_json: Option<Option<String>> = connection
            .query_row(
                "SELECT r.context_document_json
                 FROM resources r
                 JOIN resource_revisions rr ON rr.resource_id = r.resource_id
                 WHERE r.workspace_id = ?1 AND r.resource_id = ?2
                   AND rr.resource_revision_id = ?3",
                params![workspace_id, pin.resource_id, pin.revision_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_database_error)?;
        let Some(context_document_json) = resource_context_document_json else {
            return Err(StoreError::NotFound);
        };
        ensure_context_document_content_readable(context_document_json.as_deref())?;
    }
    Ok(())
}

fn canonicalize_task_timestamps(commit: &mut TaskCreateCommit) -> Result<(), StoreError> {
    commit.task.created_at = canonicalize_utc_timestamp(&commit.task.created_at)?;
    commit.task.updated_at = canonicalize_utc_timestamp(&commit.task.updated_at)?;
    if let Some(value) = commit.task.completed_at.as_mut() {
        *value = canonicalize_utc_timestamp(value)?;
    }
    commit.initial_spec_revision.created_at =
        canonicalize_utc_timestamp(&commit.initial_spec_revision.created_at)?;
    if let Some(value) = commit.initial_spec_revision.deadline.as_mut() {
        *value = canonicalize_utc_timestamp(value)?;
    }
    commit.event.recorded_at = canonicalize_utc_timestamp(&commit.event.recorded_at)?;
    Ok(())
}

/// Parses an RFC 3339 instant and emits fixed-width UTC nanoseconds. Fixed precision and
/// a single timezone representation make SQLite's lexical keyset ordering chronological.
/// The Event HLC is intentionally separate: it is a hybrid-clock value, not wall time.
fn canonicalize_utc_timestamp(value: &str) -> Result<String, StoreError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| StoreError::Invalid("timestamp must be a valid RFC 3339 instant".to_owned()))?
        .to_offset(UtcOffset::UTC);
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        parsed.year(),
        u8::from(parsed.month()),
        parsed.day(),
        parsed.hour(),
        parsed.minute(),
        parsed.second(),
        parsed.nanosecond(),
    ))
}

fn validate_task_request_mapping(
    commit: &TaskCreateCommit,
    selected_binding: &str,
    effective_failover_policy: &Value,
) -> Result<(), StoreError> {
    let request = &commit.request.request_payload;
    let spec = &commit.initial_spec_revision;
    let arrays_match = [
        ("source_message_refs", json!(&spec.source_message_refs)),
        ("constraints", json!(&spec.constraints)),
        ("non_goals", json!(&spec.non_goals)),
        ("input_refs", json!(&spec.input_refs)),
        ("required_outputs", json!(&spec.required_outputs)),
        ("acceptance_criteria", json!(&spec.acceptance_criteria)),
        ("approvals_required", json!(&spec.approvals_required)),
    ]
    .into_iter()
    .all(|(field, expected)| request.get(field).cloned().unwrap_or_else(|| json!([])) == expected);

    let optional_json_matches = |field: &str, expected: Option<&Value>| match request.get(field) {
        None | Some(Value::Null) => expected.is_none(),
        Some(value) => expected == Some(value),
    };
    let optional_string_matches = |field: &str, expected: Option<&str>| match request.get(field) {
        None | Some(Value::Null) => expected.is_none(),
        Some(Value::String(value)) => expected == Some(value.as_str()),
        Some(_) => false,
    };
    let request_deadline = match request.get("deadline") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(canonicalize_utc_timestamp(value)?),
        Some(_) => {
            return Err(StoreError::Invalid(
                "Task deadline must be a timestamp or null".to_owned(),
            ));
        }
    };
    let expected_deadline = spec.deadline.as_deref();
    let placement = request
        .get("placement_preference")
        .cloned()
        .unwrap_or_else(|| json!("AUTO"));
    let failover = request
        .get("lead_failover_policy")
        .unwrap_or(effective_failover_policy);
    let expected_placement = &spec.placement_preference;
    let explicit_binding = request.get("preferred_lead_agent_binding_id");
    let explicit_binding_matches = match explicit_binding {
        None | Some(Value::Null) => true,
        Some(Value::String(value)) => value == selected_binding,
        Some(_) => false,
    };

    let match_request = arrays_match
        && optional_string_matches("task_category", spec.task_category.as_deref())
        && optional_json_matches("budget", spec.budget.as_ref())
        && optional_json_matches(
            "delegation_budget_policy",
            spec.delegation_budget_policy.as_ref(),
        )
        && request_deadline.as_deref() == expected_deadline
        && &placement == expected_placement
        && failover == &spec.lead_failover_policy
        && request.get("workspace_id").and_then(Value::as_str)
            == Some(commit.task.workspace_id.as_str())
        && request.get("objective").and_then(Value::as_str) == Some(spec.objective.as_str())
        && request.get("coworker_id").and_then(Value::as_str)
            == commit.task.origin_coworker_id.as_deref()
        && request
            .get("expected_coworker_version")
            .and_then(Value::as_u64)
            == commit.expected_coworker_version
        && explicit_binding_matches
        && spec.preferred_lead_agent_binding_id.as_deref() == Some(selected_binding);
    if !match_request {
        return Err(StoreError::Invalid(
            "CreateTask request does not match the persisted TaskSpec intent or resolved defaults"
                .to_owned(),
        ));
    }
    Ok(())
}

fn start_task_planning_session_transaction(
    connection: &mut Connection,
    start: TaskPlanningSessionStart,
    state_ref: AggregateStateRef,
) -> Result<CommittedAgentSession, StoreError> {
    let session = &start.session;
    let task_id = session
        .task_id
        .as_deref()
        .ok_or_else(|| StoreError::Invalid("planning session Task is missing".to_owned()))?;
    let task_spec_revision = session.task_spec_revision.ok_or_else(|| {
        StoreError::Invalid("planning session TaskSpec revision is missing".to_owned())
    })?;
    if session.workspace_id.trim().is_empty()
        || session.agent_session_id.trim().is_empty()
        || start.principal_id.trim().is_empty()
        || start.request_id.trim().is_empty()
        || session.agent_binding_id.trim().is_empty()
        || session.endpoint_id.trim().is_empty()
        || session.runtime_id.trim().is_empty()
        || session.runtime_incarnation_id.trim().is_empty()
        || start.event.workspace_id != session.workspace_id
        || start.event.entity_type != "AgentSession"
        || start.event.entity_id != session.agent_session_id
        || start.event.event_type != "agent.session.starting.v1"
        || start.event.schema_version != 1
        || start.event.entity_revision != 1
        || state_ref.entity_revision != 1
        || state_ref.record_schema_version != 1
        || start.event.payload
            != json!({
                "agent_session_id": session.agent_session_id,
                "scope": {"kind":"TASK_PLANNING", "task_id":task_id},
                "agent_binding_id": session.agent_binding_id,
                "endpoint_id": session.endpoint_id,
                "runtime_id": session.runtime_id,
                "runtime_incarnation_id": session.runtime_incarnation_id,
                "task_spec_revision": task_spec_revision,
                "session_state": "STARTING",
                "reported_at": session.started_at,
            })
    {
        return Err(StoreError::Invalid(
            "Task planning session identity or starting event is inconsistent".to_owned(),
        ));
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let request_digest = digest(&canonical_json(&json!({
        "operation": "agent-session.start-task-planning.v1",
        "request": &start.request_payload,
        "workspace_id": session.workspace_id,
        "task_id": task_id,
        "expected_task_version": start.expected_task_version,
        "task_spec_revision": task_spec_revision,
        "agent_binding_id": session.agent_binding_id,
        "endpoint_id": session.endpoint_id,
        "runtime_id": session.runtime_id,
        "runtime_incarnation_id": session.runtime_incarnation_id,
    }))?);
    let prior: Option<(String, Option<String>, Option<String>)> = tx.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![start.principal_id, start.request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((prior_digest, response_json, response_digest)) = prior {
        if prior_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response_json = response_json.ok_or_else(|| {
            StoreError::Integrity("AgentSession idempotency receipt is incomplete".to_owned())
        })?;
        if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "AgentSession idempotency response digest does not match".to_owned(),
            ));
        }
        return serde_json::from_str(&response_json)
            .map_err(|error| StoreError::Integrity(error.to_string()));
    }
    let current: Option<(String, i64, i64, String, String)> = tx.query_row(
        "SELECT t.workspace_id, t.version, t.current_spec_revision, t.status, t.lead_agent_binding_id
         FROM tasks t WHERE t.task_id = ?1",
        [task_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).optional().map_err(map_database_error)?;
    let Some((workspace_id, task_version, current_spec_revision, task_status, lead_binding_id)) =
        current
    else {
        return Err(StoreError::NotFound);
    };
    let task_version = from_sql_i64(task_version, "Task version")?;
    let current_spec_revision = from_sql_i64(current_spec_revision, "TaskSpec revision")?;
    if workspace_id != session.workspace_id {
        return Err(StoreError::NotFound);
    }
    if task_version != start.expected_task_version
        || current_spec_revision != task_spec_revision
        || lead_binding_id != session.agent_binding_id
    {
        return Err(StoreError::Conflict {
            expected: Some(start.expected_task_version),
            actual: Some(task_version),
        });
    }
    if !matches!(task_status.as_str(), "READY" | "RUNNING") {
        return Err(StoreError::Invalid(
            "Task is not eligible for planning session admission".to_owned(),
        ));
    }
    let workspace: Option<(String, String)> = tx
        .query_row(
            "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
            [&session.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((workspace_owner, workspace_status)) = workspace else {
        return Err(StoreError::NotFound);
    };
    if workspace_owner != start.principal_id {
        return Err(StoreError::Invalid(
            "authenticated Principal does not own this Workspace".to_owned(),
        ));
    }
    if workspace_status != "ACTIVE" {
        return Err(StoreError::Invalid("Workspace is not active".to_owned()));
    }
    let eligible: bool = tx.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM agent_bindings b
           JOIN agent_endpoints e ON e.agent_profile_id = b.agent_profile_id AND e.endpoint_id = ?2
           JOIN agent_endpoint_bindings eb ON eb.endpoint_id = e.endpoint_id
           JOIN runtimes r ON r.runtime_id = eb.runtime_id
           JOIN runtime_incarnations ri ON ri.runtime_id = r.runtime_id AND ri.runtime_incarnation_id = ?5
           WHERE b.workspace_id = ?1 AND b.agent_binding_id = ?3
             AND b.enabled = 1 AND b.lead_eligible = 1
             AND (b.runtime_id IS NULL OR b.runtime_id = ?4)
             AND (json_extract(b.endpoint_selection_policy_json, '$.mode') <> 'PINNED_ENDPOINT'
                  OR json_extract(b.endpoint_selection_policy_json, '$.endpoint_id') = ?2)
             AND eb.runtime_id = ?4 AND eb.runtime_incarnation_id = ?5
             AND (eb.expires_at IS NULL OR eb.expires_at > ?6)
             AND r.workspace_id = ?1 AND r.availability = 'ONLINE'
             AND r.current_incarnation_id = ?5 AND ri.recovery_state = 'READY'
         )",
        params![session.workspace_id, session.endpoint_id, session.agent_binding_id,
            session.runtime_id, session.runtime_incarnation_id, session.started_at],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !eligible {
        return Err(StoreError::Invalid(
            "AgentBinding endpoint or Runtime is not ready for planning".to_owned(),
        ));
    }
    tx.execute(
        "INSERT INTO agent_sessions(agent_session_id, workspace_id, scope_kind, conversation_id,
          conversation_turn_id, task_id, task_spec_revision, attempt_id, agent_binding_id, endpoint_id,
          runtime_id, runtime_incarnation_id, configuration_digest, harness_descriptor_digest,
          status, started_at, last_event_at, closed_at, version)
         VALUES (?1, ?2, 'TASK_PLANNING', NULL, NULL, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, ?10,
          'STARTING', ?11, ?11, NULL, 1)",
        params![session.agent_session_id, session.workspace_id, task_id,
            to_sql_i64(task_spec_revision, "TaskSpec revision")?, session.agent_binding_id,
            session.endpoint_id, session.runtime_id, session.runtime_incarnation_id,
            session.configuration_digest, session.harness_descriptor_digest, session.started_at],
    ).map_err(map_database_error)?;
    let event = insert_domain_event(&tx, start.event, state_ref)?;
    let committed = CommittedAgentSession {
        session: start.session,
        event,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![start.principal_id, start.request_id, request_digest, response_json,
            digest(response_json.as_bytes()), committed.event.recorded_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn mark_starting_agent_session_lost_transaction(
    connection: &mut Connection,
    transition: MarkStartingAgentSessionLost,
    next: AgentSessionRecord,
    state_ref: AggregateStateRef,
) -> Result<CommittedAgentSession, StoreError> {
    if transition.workspace_id != next.workspace_id
        || transition.agent_session_id != next.agent_session_id
        || transition.expected_version.checked_add(1) != Some(next.version)
        || next.scope_kind != "TASK_PLANNING"
        || next.status != "LOST"
        || next.version != state_ref.entity_revision
        || state_ref.record_schema_version != 1
        || transition.event.schema_version != 1
        || transition.event.entity_revision != next.version
        || transition.event.event_type != "agent.session.lost.v1"
    {
        return Err(StoreError::Invalid(
            "AgentSession lost transition identity is inconsistent".to_owned(),
        ));
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let current: Option<(String, i64)> = tx.query_row(
        "SELECT status, version FROM agent_sessions WHERE workspace_id = ?1 AND agent_session_id = ?2 AND scope_kind = 'TASK_PLANNING'",
        params![transition.workspace_id, transition.agent_session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(map_database_error)?;
    let Some((status, version)) = current else {
        return Err(StoreError::NotFound);
    };
    let version = from_sql_i64(version, "AgentSession version")?;
    if status != "STARTING" || version != transition.expected_version {
        return Err(StoreError::Conflict {
            expected: Some(transition.expected_version),
            actual: Some(version),
        });
    }
    let changed = tx.execute(
        "UPDATE agent_sessions SET status = 'LOST', last_event_at = ?1, closed_at = ?1, version = ?2
         WHERE workspace_id = ?3 AND agent_session_id = ?4 AND status = 'STARTING' AND version = ?5",
        params![transition.occurred_at, to_sql_i64(next.version, "AgentSession version")?,
            transition.workspace_id, transition.agent_session_id,
            to_sql_i64(transition.expected_version, "AgentSession version")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(transition.expected_version),
            actual: Some(version),
        });
    }
    let event = insert_domain_event(&tx, transition.event, state_ref)?;
    tx.commit().map_err(map_database_error)?;
    Ok(CommittedAgentSession {
        session: next,
        event,
    })
}

fn activate_task_planning_session_transaction(
    connection: &mut Connection,
    activation: ActivateTaskPlanningSession,
    session: AgentSessionRecord,
    task: TaskView,
    session_state_ref: AggregateStateRef,
    task_state_ref: Option<AggregateStateRef>,
) -> Result<CommittedPlanningActivation, StoreError> {
    let task_id = session
        .task_id
        .as_deref()
        .ok_or_else(|| StoreError::Invalid("planning session Task is missing".to_owned()))?;
    let task_spec_revision = session
        .task_spec_revision
        .ok_or_else(|| StoreError::Invalid("planning session TaskSpec is missing".to_owned()))?;
    let task_changed = activation.task_status_event.is_some();
    if activation.workspace_id != session.workspace_id
        || activation.agent_session_id != session.agent_session_id
        || activation.expected_session_version.checked_add(1) != Some(session.version)
        || activation.expected_task_version > task.task.version
        || session.scope_kind != "TASK_PLANNING"
        || session.status != "ACTIVE"
        || session.version != session_state_ref.entity_revision
        || session_state_ref.record_schema_version != 1
        || task.task.task_id != task_id
        || task.task.workspace_id != session.workspace_id
        || task.task.current_spec_revision != task_spec_revision
        || task.task.lead_agent_binding_id != session.agent_binding_id
        || (task_changed
            && (activation.expected_task_version.checked_add(1) != Some(task.task.version)
                || task_state_ref.as_ref().is_none_or(|state_ref| {
                    state_ref.entity_revision != task.task.version
                        || state_ref.record_schema_version != 1
                })))
        || (!task_changed
            && (activation.expected_task_version != task.task.version || task_state_ref.is_some()))
    {
        return Err(StoreError::Invalid(
            "planning session activation state is inconsistent".to_owned(),
        ));
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let current_session: Option<(String, i64)> = tx.query_row(
        "SELECT status, version FROM agent_sessions WHERE workspace_id = ?1 AND agent_session_id = ?2 AND scope_kind = 'TASK_PLANNING'",
        params![activation.workspace_id, activation.agent_session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(map_database_error)?;
    let Some((session_status, session_version)) = current_session else {
        return Err(StoreError::NotFound);
    };
    let session_version = from_sql_i64(session_version, "AgentSession version")?;
    if session_status != "STARTING" || session_version != activation.expected_session_version {
        return Err(StoreError::Conflict {
            expected: Some(activation.expected_session_version),
            actual: Some(session_version),
        });
    }
    let current_task: Option<(String, i64, i64, String, String)> = tx.query_row(
        "SELECT workspace_id, version, current_spec_revision, status, lead_agent_binding_id FROM tasks WHERE task_id = ?1",
        [task_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).optional().map_err(map_database_error)?;
    let Some((workspace_id, task_version, spec_revision, task_status, lead_binding_id)) =
        current_task
    else {
        return Err(StoreError::NotFound);
    };
    let task_version = from_sql_i64(task_version, "Task version")?;
    if workspace_id != session.workspace_id
        || task_version != activation.expected_task_version
        || from_sql_i64(spec_revision, "TaskSpec revision")? != task_spec_revision
        || lead_binding_id != session.agent_binding_id
    {
        return Err(StoreError::Conflict {
            expected: Some(activation.expected_task_version),
            actual: Some(task_version),
        });
    }
    let needs_running_transition = task_status == "READY";
    if !needs_running_transition && task_status != "RUNNING" {
        return Err(StoreError::Invalid(
            "Task stopped being eligible during native startup".to_owned(),
        ));
    }
    if needs_running_transition != task_changed {
        return Err(StoreError::Conflict {
            expected: Some(activation.expected_task_version),
            actual: Some(task_version),
        });
    }
    // Native startup occurs outside this transaction. Revalidate that the host
    // and its Runtime are still ready/current before admitting the session.
    let host_ready: bool = tx.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM agent_host_instances h
           JOIN runtimes r ON r.runtime_id = h.runtime_id
           JOIN runtime_incarnations ri
             ON ri.runtime_id = h.runtime_id
            AND ri.runtime_incarnation_id = h.runtime_incarnation_id
           JOIN agent_bindings b ON b.agent_binding_id = ?1
           JOIN agent_endpoints e
             ON e.endpoint_id = h.endpoint_id
            AND e.agent_profile_id = b.agent_profile_id
           JOIN agent_endpoint_bindings eb
             ON eb.endpoint_id = h.endpoint_id
            AND eb.runtime_id = h.runtime_id
            AND eb.runtime_incarnation_id = h.runtime_incarnation_id
           WHERE h.host_instance_id = ?2
             AND h.runtime_id = ?3
             AND h.runtime_incarnation_id = ?4
             AND h.endpoint_id = ?5
             AND b.agent_profile_id = h.agent_profile_id
             AND b.workspace_id = ?6
             AND b.enabled = 1
             AND b.lead_eligible = 1
             AND (b.runtime_id IS NULL OR b.runtime_id = h.runtime_id)
             AND (json_extract(b.endpoint_selection_policy_json, '$.mode') <> 'PINNED_ENDPOINT'
                  OR json_extract(b.endpoint_selection_policy_json, '$.endpoint_id') = h.endpoint_id)
             AND (eb.expires_at IS NULL OR eb.expires_at > ?7)
             AND r.workspace_id = ?6
             AND r.availability = 'ONLINE'
             AND r.current_incarnation_id = h.runtime_incarnation_id
             AND ri.recovery_state = 'READY'
             AND h.state IN ('READY', 'BUSY')
         )",
        params![session.agent_binding_id, activation.host_instance_id, session.runtime_id,
            session.runtime_incarnation_id, session.endpoint_id, session.workspace_id, activation.occurred_at],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !host_ready {
        return Err(StoreError::Invalid(
            "native host or Runtime is no longer ready for planning activation".to_owned(),
        ));
    }
    let session_changed = tx.execute(
        "UPDATE agent_sessions SET status = 'ACTIVE', last_event_at = ?1, version = ?2
         WHERE workspace_id = ?3 AND agent_session_id = ?4 AND status = 'STARTING' AND version = ?5",
        params![activation.occurred_at, to_sql_i64(session.version, "AgentSession version")?,
            activation.workspace_id, activation.agent_session_id,
            to_sql_i64(activation.expected_session_version, "AgentSession version")?],
    ).map_err(map_database_error)?;
    if session_changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(activation.expected_session_version),
            actual: Some(session_version),
        });
    }
    let host_changed = tx
        .execute(
            "UPDATE agent_host_instances SET state = 'BUSY', last_used_at = ?1, idle_since = NULL
         WHERE host_instance_id = ?2 AND runtime_id = ?3 AND runtime_incarnation_id = ?4
           AND endpoint_id = ?5 AND state IN ('READY', 'BUSY')",
            params![
                activation.occurred_at,
                activation.host_instance_id,
                session.runtime_id,
                session.runtime_incarnation_id,
                session.endpoint_id
            ],
        )
        .map_err(map_database_error)?;
    if host_changed != 1 {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    if needs_running_transition {
        let changed = tx
            .execute(
                "UPDATE tasks SET status = 'RUNNING', updated_at = ?1, version = ?2
             WHERE workspace_id = ?3 AND task_id = ?4 AND status = 'READY' AND version = ?5
               AND current_spec_revision = ?6 AND lead_agent_binding_id = ?7",
                params![
                    task.task.updated_at,
                    to_sql_i64(task.task.version, "Task version")?,
                    task.task.workspace_id,
                    task.task.task_id,
                    to_sql_i64(activation.expected_task_version, "Task version")?,
                    to_sql_i64(task_spec_revision, "TaskSpec revision")?,
                    session.agent_binding_id
                ],
            )
            .map_err(map_database_error)?;
        if changed != 1 {
            return Err(StoreError::Conflict {
                expected: Some(activation.expected_task_version),
                actual: Some(task_version),
            });
        }
    }
    tx.execute(
        "INSERT INTO agent_session_host_bindings(agent_session_id, host_instance_id, native_session_ref, bound_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![session.agent_session_id, activation.host_instance_id,
            activation.native_session_ref, activation.occurred_at],
    ).map_err(map_database_error)?;
    let session_event = insert_domain_event(&tx, activation.session_event, session_state_ref)?;
    let task_status_event = match (activation.task_status_event, task_state_ref) {
        (Some(event), Some(state_ref)) => Some(insert_domain_event(&tx, event, state_ref)?),
        (None, None) => None,
        _ => {
            return Err(StoreError::Invalid(
                "Task status event and aggregate snapshot must be paired".to_owned(),
            ));
        }
    };
    tx.commit().map_err(map_database_error)?;
    Ok(CommittedPlanningActivation {
        session,
        session_event,
        task,
        task_status_event,
    })
}

fn validate_agent_host_instance(host: &AgentHostInstanceRecord) -> Result<(), StoreError> {
    if host.host_instance_id.trim().is_empty()
        || host.runtime_id.trim().is_empty()
        || host.runtime_incarnation_id.trim().is_empty()
        || host.agent_profile_id.trim().is_empty()
        || host.endpoint_id.trim().is_empty()
        || host.state != "STARTING"
        || !matches!(
            host.hosting_mode.as_str(),
            "REMOTE_API"
                | "REMOTE_A2A"
                | "LOCAL_SHARED_DAEMON"
                | "LOCAL_PER_SESSION"
                | "EMBEDDED_SDK"
                | "EXTERNAL_PROCESS"
        )
        || !matches!(
            host.ownership.as_str(),
            "LITECOWORK" | "EXTERNAL" | "REMOTE"
        )
        || host
            .process_identity_ref
            .as_ref()
            .is_some_and(|value| value.len() > 512 || value.chars().any(char::is_control))
        || host.idle_since.is_some()
    {
        return Err(StoreError::Invalid(
            "AgentHostInstance creation is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn valid_agent_host_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        (
            "STARTING",
            "READY" | "DEGRADED" | "STOPPING" | "STOPPED" | "FAILED"
        ) | (
            "READY",
            "BUSY" | "DEGRADED" | "STOPPING" | "STOPPED" | "FAILED"
        ) | (
            "BUSY",
            "READY" | "DEGRADED" | "STOPPING" | "STOPPED" | "FAILED"
        ) | ("DEGRADED", "READY" | "STOPPING" | "STOPPED" | "FAILED")
            | ("STOPPING", "STOPPED" | "FAILED")
    )
}

fn create_agent_host_instance_transaction(
    connection: &mut Connection,
    host: AgentHostInstanceRecord,
) -> Result<(), StoreError> {
    validate_agent_host_instance(&host)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let eligible: bool = transaction
        .query_row(
            "SELECT EXISTS(
           SELECT 1 FROM runtimes r
           JOIN runtime_incarnations ri
             ON ri.runtime_id = r.runtime_id
            AND ri.runtime_incarnation_id = r.current_incarnation_id
           JOIN agent_endpoints e ON e.endpoint_id = ?3 AND e.agent_profile_id = ?4
           WHERE r.runtime_id = ?1
             AND r.current_incarnation_id = ?2
             AND r.availability = 'ONLINE'
             AND ri.recovery_state = 'READY'
         )",
            params![
                host.runtime_id,
                host.runtime_incarnation_id,
                host.endpoint_id,
                host.agent_profile_id
            ],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !eligible {
        return Err(StoreError::Invalid(
            "Agent host requires a current ready Runtime and matching endpoint".to_owned(),
        ));
    }
    transaction
        .execute(
            "INSERT INTO agent_host_instances(host_instance_id, runtime_id, runtime_incarnation_id,
          agent_profile_id, endpoint_id, hosting_mode, state, process_identity_ref, ownership,
          started_at, last_used_at, idle_since)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'STARTING', ?7, ?8, ?9, ?10, NULL)",
            params![
                host.host_instance_id,
                host.runtime_id,
                host.runtime_incarnation_id,
                host.agent_profile_id,
                host.endpoint_id,
                host.hosting_mode,
                host.process_identity_ref,
                host.ownership,
                host.started_at,
                host.last_used_at
            ],
        )
        .map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)
}

fn transition_agent_host_instance_transaction(
    connection: &mut Connection,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    host_instance_id: &str,
    expected_state: &str,
    next_state: &str,
    occurred_at: &str,
    process_identity_ref: Option<&str>,
) -> Result<AgentHostInstanceRecord, StoreError> {
    if !valid_agent_host_transition(expected_state, next_state) {
        return Err(StoreError::Invalid(
            "AgentHostInstance transition is invalid".to_owned(),
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let idle_since = if next_state == "READY" {
        Some(occurred_at)
    } else {
        None
    };
    let changed = transaction
        .execute(
            "UPDATE agent_host_instances
         SET state = ?1, last_used_at = ?2, idle_since = ?3,
             process_identity_ref = COALESCE(?4, process_identity_ref)
         WHERE runtime_id = ?5 AND runtime_incarnation_id = ?6
           AND host_instance_id = ?7 AND state = ?8",
            params![
                next_state,
                occurred_at,
                idle_since,
                process_identity_ref,
                runtime_id,
                runtime_incarnation_id,
                host_instance_id,
                expected_state
            ],
        )
        .map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let host = load_agent_host_instance(
        &transaction,
        runtime_id,
        runtime_incarnation_id,
        host_instance_id,
    )?
    .ok_or(StoreError::NotFound)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(host)
}

fn load_agent_host_instance(
    connection: &Connection,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    host_instance_id: &str,
) -> Result<Option<AgentHostInstanceRecord>, StoreError> {
    connection
        .query_row(
            "SELECT host_instance_id, runtime_id, runtime_incarnation_id, agent_profile_id,
          endpoint_id, hosting_mode, state, process_identity_ref, ownership, started_at,
          last_used_at, idle_since FROM agent_host_instances
         WHERE runtime_id = ?1 AND runtime_incarnation_id = ?2 AND host_instance_id = ?3",
            params![runtime_id, runtime_incarnation_id, host_instance_id],
            |row| {
                Ok(AgentHostInstanceRecord {
                    host_instance_id: row.get(0)?,
                    runtime_id: row.get(1)?,
                    runtime_incarnation_id: row.get(2)?,
                    agent_profile_id: row.get(3)?,
                    endpoint_id: row.get(4)?,
                    hosting_mode: row.get(5)?,
                    state: row.get(6)?,
                    process_identity_ref: row.get(7)?,
                    ownership: row.get(8)?,
                    started_at: row.get(9)?,
                    last_used_at: row.get(10)?,
                    idle_since: row.get(11)?,
                })
            },
        )
        .optional()
        .map_err(map_database_error)
}

fn list_agent_host_instances(
    connection: &Connection,
    runtime_id: &str,
    runtime_incarnation_id: &str,
) -> Result<Vec<AgentHostInstanceRecord>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT host_instance_id, runtime_id, runtime_incarnation_id, agent_profile_id,
          endpoint_id, hosting_mode, state, process_identity_ref, ownership, started_at,
          last_used_at, idle_since FROM agent_host_instances
         WHERE runtime_id = ?1 AND runtime_incarnation_id = ?2
         ORDER BY started_at ASC, host_instance_id ASC",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(params![runtime_id, runtime_incarnation_id], |row| {
            Ok(AgentHostInstanceRecord {
                host_instance_id: row.get(0)?,
                runtime_id: row.get(1)?,
                runtime_incarnation_id: row.get(2)?,
                agent_profile_id: row.get(3)?,
                endpoint_id: row.get(4)?,
                hosting_mode: row.get(5)?,
                state: row.get(6)?,
                process_identity_ref: row.get(7)?,
                ownership: row.get(8)?,
                started_at: row.get(9)?,
                last_used_at: row.get(10)?,
                idle_since: row.get(11)?,
            })
        })
        .map_err(map_database_error)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)
}

fn create_task_transaction(
    connection: &mut Connection,
    commit: TaskCreateCommit,
    state_ref: AggregateStateRef,
    occurrence_state_refs: Option<[AggregateStateRef; 3]>,
) -> Result<storage_core::CommittedTask, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let committed =
        create_task_in_transaction(&tx, commit, state_ref, None, occurrence_state_refs)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

pub(crate) fn create_task_in_transaction(
    tx: &Transaction<'_>,
    commit: TaskCreateCommit,
    state_ref: AggregateStateRef,
    idempotency_context: Option<Value>,
    occurrence_state_refs: Option<[AggregateStateRef; 3]>,
) -> Result<storage_core::CommittedTask, StoreError> {
    let idempotency_payload = commit
        .idempotency_payload
        .as_ref()
        .unwrap_or(&commit.request.request_payload);
    let request_digest = match idempotency_context {
        None => digest(&canonical_json(idempotency_payload)?),
        Some(context) => digest(&canonical_json(&json!({
            "request": idempotency_payload.clone(),
            "context": context,
        }))?),
    };
    let prior: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            params![commit.request.principal_id, commit.request.request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    if let Some((prior_digest, response_json, response_digest)) = prior {
        if prior_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response_json = response_json.ok_or_else(|| {
            StoreError::Integrity("Task idempotency receipt is incomplete".to_owned())
        })?;
        if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "Task idempotency response digest does not match".to_owned(),
            ));
        }
        return serde_json::from_str(&response_json)
            .map_err(|error| StoreError::Integrity(error.to_string()));
    }

    let task = &commit.task;
    let spec = &commit.initial_spec_revision;
    let workspace: Option<(String, String, Option<String>, Option<i64>)> = tx
        .query_row(
            "SELECT owner_principal_id, status, default_agent_binding_id, current_instruction_revision FROM workspaces WHERE workspace_id = ?1",
            [&task.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((owner, workspace_status, workspace_default_binding, instruction_revision)) =
        workspace
    else {
        return Err(StoreError::NotFound);
    };
    if owner != commit.request.principal_id {
        return Err(StoreError::Invalid(
            "authenticated Principal does not own this Workspace".to_owned(),
        ));
    }
    if workspace_status != "ACTIVE" {
        return Err(StoreError::Invalid(
            "archived Workspace is read-only".to_owned(),
        ));
    }
    if spec.workspace_instruction_revision
        != instruction_revision
            .map(|revision| from_sql_i64(revision, "Workspace instruction revision"))
            .transpose()?
    {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }

    let explicit_binding = commit
        .request
        .request_payload
        .get("preferred_lead_agent_binding_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let (coworker_default_binding, coworker_default_failover_policy) = if let (
        Some(coworker_id),
        Some(coworker_revision),
    ) = (
        task.origin_coworker_id.as_deref(),
        task.origin_coworker_revision,
    ) {
        let coworker: Option<(i64, String, Option<String>, Option<String>)> = tx
            .query_row(
                "SELECT c.version, c.status, r.default_lead_agent_binding_id, r.lead_failover_policy_json
                 FROM coworkers c JOIN coworker_revisions r
                   ON r.workspace_id = c.workspace_id AND r.coworker_id = c.coworker_id
                 WHERE c.workspace_id = ?1 AND c.coworker_id = ?2 AND r.revision = ?3",
                params![task.workspace_id, coworker_id, to_sql_i64(coworker_revision, "Coworker revision")?],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(map_database_error)?;
        let Some((coworker_version, coworker_status, default_binding, default_failover_json)) =
            coworker
        else {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        };
        let coworker_version = from_sql_i64(coworker_version, "Coworker version")?;
        if commit
            .expected_coworker_version
            .is_some_and(|expected| expected != coworker_version)
        {
            return Err(StoreError::Conflict {
                expected: commit.expected_coworker_version,
                actual: Some(coworker_version),
            });
        }
        if coworker_status == "ARCHIVED" {
            return Err(StoreError::Invalid("Coworker is archived".to_owned()));
        }
        let default_failover = default_failover_json
            .map(|json| {
                serde_json::from_str::<Value>(&json).map_err(|error| {
                    StoreError::Integrity(format!(
                        "Coworker lead failover policy is invalid: {error}"
                    ))
                })
            })
            .transpose()?;
        (Some(default_binding), default_failover)
    } else {
        if commit.expected_coworker_version.is_some() {
            return Err(StoreError::Invalid(
                "expected Coworker version requires a selected Coworker".to_owned(),
            ));
        }
        (None, None)
    };
    let selected_binding = explicit_binding
        .or_else(|| coworker_default_binding.flatten())
        .or(workspace_default_binding)
        .ok_or_else(|| {
            StoreError::Invalid("no eligible lead AgentBinding is configured".to_owned())
        })?;
    if selected_binding != task.lead_agent_binding_id
        || spec.preferred_lead_agent_binding_id.as_deref() != Some(selected_binding.as_str())
    {
        return Err(StoreError::Invalid(
            "Task lead does not match the configured binding precedence".to_owned(),
        ));
    }
    let disabled_failover = json!({
        "mode": "DISABLED",
        "triggers": [],
        "fallback_agent_binding_ids": [],
        "max_lead_changes": 0
    });
    let effective_failover_policy = commit
        .request
        .request_payload
        .get("lead_failover_policy")
        .or(coworker_default_failover_policy.as_ref())
        .unwrap_or(&disabled_failover);
    validate_task_request_mapping(&commit, &selected_binding, effective_failover_policy)?;
    let binding_eligible: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2 AND enabled = 1 AND lead_eligible = 1)",
            params![task.workspace_id, selected_binding],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !binding_eligible {
        return Err(StoreError::Invalid(
            "selected lead AgentBinding is not enabled and lead-eligible in this Workspace"
                .to_owned(),
        ));
    }
    routines::validate_task_admission(&tx, &commit)?;
    automation_admission::validate_task_admission(&tx, &commit)?;
    begin_automation_occurrence_claim(&tx, &commit, occurrence_state_refs.as_ref())?;
    validate_task_resource_inputs(&tx, &task.workspace_id, &spec.input_refs)?;
    let state_ref = state_ref;
    if state_ref.entity_revision != task.version || state_ref.record_schema_version != 1 {
        return Err(StoreError::Integrity(
            "Task aggregate-state reference is inconsistent".to_owned(),
        ));
    }
    let principal_json = String::from_utf8(canonical_json(&task.created_by)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let blocking_json = String::from_utf8(canonical_json(&task.blocking_conditions)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO tasks(task_id, workspace_id, conversation_id, routine_id, routine_revision, automation_id, automation_occurrence_id, origin_coworker_id, origin_coworker_revision, current_spec_revision, current_plan_revision, resume_status, status, lead_agent_binding_id, blocking_conditions_json, priority, created_by_json, created_at, updated_at, completed_at, version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, NULL, NULL, 'READY', ?10, ?11, ?12, ?13, ?14, ?15, NULL, 1)",
        params![
            task.task_id, task.workspace_id, task.conversation_id, task.routine_id,
            task.routine_revision.map(|value| to_sql_i64(value, "Routine revision")).transpose()?,
            task.automation_id, task.automation_occurrence_id, task.origin_coworker_id,
            task.origin_coworker_revision.map(|value| to_sql_i64(value, "Coworker revision")).transpose()?,
            task.lead_agent_binding_id, blocking_json, task.priority, principal_json,
            task.created_at, task.updated_at,
        ],
    ).map_err(map_database_error)?;

    let parent_json = String::from_utf8(canonical_json(&spec.parent_revisions)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let input_refs_json = String::from_utf8(canonical_json(&spec.input_refs)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let required_outputs_json = String::from_utf8(canonical_json(&spec.required_outputs)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let criteria_json = String::from_utf8(canonical_json(&spec.acceptance_criteria)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let approvals_json = String::from_utf8(canonical_json(&spec.approvals_required)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let budget_json = spec
        .budget
        .as_ref()
        .map(canonical_json)
        .transpose()?
        .map(String::from_utf8)
        .transpose()
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let delegation_budget_json = spec
        .delegation_budget_policy
        .as_ref()
        .map(canonical_json)
        .transpose()?
        .map(String::from_utf8)
        .transpose()
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let failover_json = String::from_utf8(canonical_json(&spec.lead_failover_policy)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let source_refs_json = String::from_utf8(canonical_json(&spec.source_message_refs)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let constraints_json = String::from_utf8(canonical_json(&spec.constraints)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let non_goals_json = String::from_utf8(canonical_json(&spec.non_goals)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let placement_json = String::from_utf8(canonical_json(&spec.placement_preference)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let spec_author_json = String::from_utf8(canonical_json(&spec.authored_by)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO task_spec_revisions(task_id, workspace_id, revision, parent_revisions_json, objective, task_category, constraints_json, non_goals_json, input_refs_json, workspace_instruction_revision, required_outputs_json, acceptance_criteria_json, approvals_required_json, budget_json, delegation_budget_policy_json, lead_failover_policy_json, deadline, source_message_refs_json, placement_preference, preferred_lead_agent_binding_id, authored_by_json, created_at)
         VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
        params![
            spec.task_id, spec.workspace_id, parent_json, spec.objective, spec.task_category,
            constraints_json, non_goals_json, input_refs_json,
            spec.workspace_instruction_revision.map(|value| to_sql_i64(value, "Workspace instruction revision")).transpose()?,
            required_outputs_json, criteria_json, approvals_json, budget_json,
            delegation_budget_json, failover_json, spec.deadline, source_refs_json,
            placement_json, spec.preferred_lead_agent_binding_id, spec_author_json, spec.created_at,
        ],
    ).map_err(map_database_error)?;

    tx.execute(
        "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1",
        params![commit.event.workspace_id, commit.event.origin_runtime_id],
    ).map_err(map_database_error)?;
    let origin_sequence: i64 = tx.query_row(
        "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
        params![commit.event.workspace_id, commit.event.origin_runtime_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    let payload_json = String::from_utf8(canonical_json(&commit.event.payload)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let event = DomainEvent {
        event_id: commit.event.event_id.clone(),
        workspace_id: commit.event.workspace_id.clone(),
        entity_type: commit.event.entity_type.clone(),
        entity_id: commit.event.entity_id.clone(),
        origin_runtime_id: commit.event.origin_runtime_id.clone(),
        origin_sequence: from_sql_i64(origin_sequence, "origin sequence")?,
        entity_revision: commit.event.entity_revision,
        hlc_timestamp: commit.event.hlc_timestamp.clone(),
        correlation_id: commit.event.correlation_id.clone(),
        causation_id: commit.event.causation_id.clone(),
        schema_version: commit.event.schema_version,
        event_type: commit.event.event_type.clone(),
        payload: commit.event.payload.clone(),
        aggregate_state_ref: state_ref,
        recorded_at: commit.event.recorded_at.clone(),
        payload_digest: digest(payload_json.as_bytes()),
    };
    tx.execute(
        "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            event.event_id, event.workspace_id, event.entity_type, event.entity_id,
            event.origin_runtime_id, to_sql_i64(event.origin_sequence, "origin sequence")?,
            to_sql_i64(event.entity_revision, "event revision")?, event.hlc_timestamp,
            event.correlation_id, event.causation_id, i64::from(event.schema_version),
            event.event_type, payload_json,
            String::from_utf8(canonical_json(&event.aggregate_state_ref)?)
                .map_err(|error| StoreError::Invalid(error.to_string()))?,
            event.recorded_at, event.payload_digest,
        ],
    ).map_err(map_database_error)?;
    settle_automation_occurrence_materialization(&tx, &commit, occurrence_state_refs.as_ref())?;
    let view = TaskView {
        task: commit.task,
        current_spec_revision: commit.initial_spec_revision,
    };
    let committed = storage_core::CommittedTask { view, event };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![commit.request.principal_id, commit.request.request_id, request_digest, response_json, digest(response_json.as_bytes()), committed.event.recorded_at],
    ).map_err(map_database_error)?;
    Ok(committed)
}

fn build_automation_occurrence_snapshots(
    admission: &AutomationTaskAdmission,
    task: &TaskRecord,
) -> Result<Vec<Value>, StoreError> {
    let states = [
        ("PENDING", 1_u64, 0_u64, None),
        ("CLAIMED", 2, 1, None),
        ("STARTED", 3, 1, Some(task.task_id.as_str())),
    ];
    states.into_iter().map(|(status, version, claim_epoch, task_id)| {
        Ok(json!({
            "workspace_id": task.workspace_id,
            "occurrence_id": admission.occurrence_id,
            "automation_id": admission.automation_id,
            "automation_revision": admission.automation_revision,
            "routine_id": admission.routine_id,
            "routine_revision": admission.routine_revision,
            "trigger_id": admission.trigger_id,
            "trigger_host_runtime_id": admission.trigger_host_runtime_id,
            "occurrence_key": admission.occurrence_key,
            "status": status,
            "version": version,
            "claim_epoch": claim_epoch,
            "claim_expires_at": if claim_epoch == 0 { Value::Null } else { json!(admission.claim_expires_at) },
            "task_id": task_id,
            "scheduled_for": Value::Null,
            "covered_misfire_range": Value::Null,
            "trigger_input_ref": Value::Null,
            "trigger_payload_digest": Value::Null,
            "blockers": [],
            "created_at": task.created_at,
            "updated_at": task.updated_at
        }))
    }).collect()
}

fn begin_automation_occurrence_claim(
    tx: &Transaction<'_>,
    commit: &TaskCreateCommit,
    state_refs: Option<&[AggregateStateRef; 3]>,
) -> Result<(), StoreError> {
    let Some(admission) = commit.automation_admission.as_ref() else {
        if state_refs.is_some() {
            return Err(StoreError::Integrity(
                "unexpected AutomationOccurrence snapshots".to_owned(),
            ));
        }
        return Ok(());
    };
    let Some(state_refs) = state_refs else {
        return Err(StoreError::Integrity(
            "AutomationOccurrence snapshots are missing".to_owned(),
        ));
    };
    let occurrence = json!({
        "workspace_id": commit.task.workspace_id,
        "occurrence_id": admission.occurrence_id,
        "automation_id": admission.automation_id,
        "automation_revision": admission.automation_revision,
        "routine_id": admission.routine_id,
        "routine_revision": admission.routine_revision,
        "trigger_id": admission.trigger_id,
        "trigger_host_runtime_id": admission.trigger_host_runtime_id,
        "occurrence_key": admission.occurrence_key,
        "scheduled_for": Value::Null,
        "covered_misfire_range_json": Value::Null,
        "trigger_input_ref_json": Value::Null,
        "trigger_payload_digest": Value::Null,
        "blockers_json": "[]",
        "claim_epoch": 0,
        "claim_expires_at": Value::Null,
        "task_id": Value::Null,
        "status": "PENDING",
        "version": 1,
        "created_at": commit.task.created_at,
        "updated_at": commit.task.updated_at,
    });
    if state_refs[0].entity_revision != 1
        || state_refs[1].entity_revision != 2
        || state_refs[2].entity_revision != 3
    {
        return Err(StoreError::Integrity(
            "AutomationOccurrence snapshot revisions are invalid".to_owned(),
        ));
    }
    tx.execute(
        "INSERT INTO automation_occurrences(workspace_id, occurrence_id, automation_id, automation_revision, routine_id, routine_revision, trigger_id, trigger_host_runtime_id, occurrence_key, scheduled_for, covered_misfire_range_json, trigger_input_ref_json, trigger_payload_digest, blockers_json, claim_epoch, claim_expires_at, task_id, status, created_at, updated_at, version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL, NULL, NULL, '[]', 0, NULL, NULL, 'PENDING', ?10, ?10, 1)",
        params![commit.task.workspace_id, admission.occurrence_id, admission.automation_id,
            to_sql_i64(admission.automation_revision, "Automation revision")?, admission.routine_id,
            to_sql_i64(admission.routine_revision, "Routine revision")?,
            admission.trigger_id, admission.trigger_host_runtime_id, admission.occurrence_key,
            commit.task.created_at],
    ).map_err(map_database_error)?;
    insert_automation_occurrence_event(
        tx,
        commit,
        admission,
        &occurrence,
        &state_refs[0],
        "automation.occurrence.created.v1",
        None,
        "PENDING",
        0,
    )?;
    let changed = tx.execute(
        "UPDATE automation_occurrences SET status = 'CLAIMED', claim_epoch = 1, claim_expires_at = ?1, updated_at = ?2, version = 2 WHERE workspace_id = ?3 AND occurrence_id = ?4 AND status = 'PENDING' AND version = 1",
        params![admission.claim_expires_at, commit.task.created_at, commit.task.workspace_id, admission.occurrence_id],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(1),
            actual: None,
        });
    }
    let claimed = build_occurrence_state(admission, &commit.task, "CLAIMED", 2, 1, None);
    insert_automation_occurrence_event(
        tx,
        commit,
        admission,
        &claimed,
        &state_refs[1],
        "automation.occurrence.claimed.v1",
        Some("PENDING"),
        "CLAIMED",
        1,
    )?;
    Ok(())
}

fn settle_automation_occurrence_materialization(
    tx: &Transaction<'_>,
    commit: &TaskCreateCommit,
    state_refs: Option<&[AggregateStateRef; 3]>,
) -> Result<(), StoreError> {
    let Some(admission) = commit.automation_admission.as_ref() else {
        return Ok(());
    };
    let refs = state_refs.ok_or_else(|| {
        StoreError::Integrity("AutomationOccurrence snapshots are missing".to_owned())
    })?;
    let changed = tx.execute(
        "UPDATE automation_occurrences SET status = 'STARTED', task_id = ?1, updated_at = ?2, version = 3 WHERE workspace_id = ?3 AND occurrence_id = ?4 AND status = 'CLAIMED' AND claim_epoch = 1 AND version = 2 AND claim_expires_at = ?5",
        params![commit.task.task_id, commit.task.created_at, commit.task.workspace_id, admission.occurrence_id, admission.claim_expires_at],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(2),
            actual: None,
        });
    }
    let started = build_occurrence_state(
        admission,
        &commit.task,
        "STARTED",
        3,
        1,
        Some(&commit.task.task_id),
    );
    insert_automation_occurrence_event(
        tx,
        commit,
        admission,
        &started,
        &refs[2],
        "automation.occurrence.status.changed.v1",
        Some("CLAIMED"),
        "STARTED",
        1,
    )
}

fn build_occurrence_state(
    admission: &AutomationTaskAdmission,
    task: &TaskRecord,
    status: &str,
    version: u64,
    claim_epoch: u64,
    task_id: Option<&str>,
) -> Value {
    json!({
        "workspace_id": task.workspace_id,
        "occurrence_id": admission.occurrence_id,
        "automation_id": admission.automation_id,
        "automation_revision": admission.automation_revision,
        "routine_id": admission.routine_id,
        "routine_revision": admission.routine_revision,
        "trigger_id": admission.trigger_id,
        "trigger_host_runtime_id": admission.trigger_host_runtime_id,
        "occurrence_key": admission.occurrence_key,
        "status": status,
        "version": version,
        "claim_epoch": claim_epoch,
        "claim_expires_at": admission.claim_expires_at,
        "task_id": task_id,
        "scheduled_for": Value::Null,
        "covered_misfire_range": Value::Null,
        "trigger_input_ref": Value::Null,
        "trigger_payload_digest": Value::Null,
        "blockers": [],
        "created_at": task.created_at,
        "updated_at": task.updated_at
    })
}

fn insert_automation_occurrence_event(
    tx: &Transaction<'_>,
    commit: &TaskCreateCommit,
    admission: &AutomationTaskAdmission,
    snapshot: &Value,
    state_ref: &AggregateStateRef,
    event_type: &str,
    from: Option<&str>,
    to: &str,
    claim_epoch: u64,
) -> Result<(), StoreError> {
    if snapshot.get("version").and_then(Value::as_u64) != Some(state_ref.entity_revision)
        || snapshot.get("occurrence_id").and_then(Value::as_str)
            != Some(admission.occurrence_id.as_str())
    {
        return Err(StoreError::Integrity(
            "AutomationOccurrence snapshot and event revision disagree".to_owned(),
        ));
    }
    let event_id = format!(
        "{}:occ:{}",
        commit.event.event_id, state_ref.entity_revision
    );
    let payload = json!({
        "occurrence_id": admission.occurrence_id,
        "automation_id": admission.automation_id,
        "automation_revision": admission.automation_revision,
        "routine_id": admission.routine_id,
        "routine_revision": admission.routine_revision,
        "trigger_id": admission.trigger_id,
        "trigger_host_runtime_id": admission.trigger_host_runtime_id,
        "occurrence_key": admission.occurrence_key,
        "version": state_ref.entity_revision,
        "claim_epoch": claim_epoch,
        "claim_expires_at": if claim_epoch == 0 { Value::Null } else { json!(admission.claim_expires_at) },
        "task_id": if to == "STARTED" { json!(commit.task.task_id) } else { Value::Null },
        "from": from,
        "to": to,
    });
    let draft = EventDraft {
        event_id,
        workspace_id: commit.task.workspace_id.clone(),
        entity_type: "AutomationOccurrence".to_owned(),
        entity_id: admission.occurrence_id.clone(),
        origin_runtime_id: commit.event.origin_runtime_id.clone(),
        entity_revision: state_ref.entity_revision,
        hlc_timestamp: commit.event.hlc_timestamp.clone(),
        correlation_id: commit.event.correlation_id.clone(),
        causation_id: Some(commit.event.event_id.clone()),
        schema_version: 1,
        event_type: event_type.to_owned(),
        payload,
        recorded_at: commit.event.recorded_at.clone(),
    };
    insert_domain_event(tx, draft, state_ref.clone())?;
    Ok(())
}

fn validate_task_spec_revision_commit(commit: &TaskSpecRevisionCommit) -> Result<(), StoreError> {
    let revision = &commit.task_spec_revision;
    let next_task_version = commit
        .expected_task_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Integrity("Task version exhausted".to_owned()))?;
    let spec_digest = digest(&canonical_json(revision)?);
    if commit.request.principal_id.trim().is_empty()
        || commit.request.request_id.trim().is_empty()
        || commit.expected_task_version == 0
        || commit.task.task_id != revision.task_id
        || commit.task.workspace_id != revision.workspace_id
        || commit.task.current_spec_revision != revision.revision
        || commit.task.current_plan_revision.is_some()
        || commit.task.status != "READY"
        || commit.task.version != next_task_version
        || commit.task.updated_at != commit.event.recorded_at
        || revision.revision <= 1
        || revision.parent_revisions.len() != 1
        || revision.parent_revisions.first().copied() != Some(revision.revision - 1)
        || revision.objective.trim().is_empty()
        || revision.objective.len() > 32 * 1024
        || revision.created_at != commit.event.recorded_at
        || revision.authored_by.get("kind").and_then(Value::as_str) != Some("USER")
        || revision
            .authored_by
            .get("principal_id")
            .and_then(Value::as_str)
            != Some(commit.request.principal_id.as_str())
        || revision
            .preferred_lead_agent_binding_id
            .as_deref()
            .is_some_and(|binding| binding != commit.task.lead_agent_binding_id)
        || commit.event.workspace_id != revision.workspace_id
        || commit.event.entity_type != "Task"
        || commit.event.entity_id != revision.task_id
        || commit.event.entity_revision != next_task_version
        || commit.event.event_type != "task.spec.revised.v1"
        || !payload_has_exact_keys(
            &commit.event.payload,
            &[
                "task_id",
                "revision",
                "parent_revisions",
                "spec_digest",
                "authored_by",
            ],
        )
        || commit.event.payload.get("task_id").and_then(Value::as_str)
            != Some(revision.task_id.as_str())
        || commit.event.payload.get("revision").and_then(Value::as_u64) != Some(revision.revision)
        || commit
            .event
            .payload
            .get("parent_revisions")
            .and_then(Value::as_array)
            != Some(
                &revision
                    .parent_revisions
                    .iter()
                    .map(|parent| Value::from(*parent))
                    .collect::<Vec<_>>(),
            )
        || commit
            .event
            .payload
            .get("spec_digest")
            .and_then(Value::as_str)
            != Some(spec_digest.as_str())
        || commit.event.payload.get("authored_by") != Some(&revision.authored_by)
    {
        return Err(StoreError::Invalid(
            "TaskSpec revision commit identity or event is inconsistent".to_owned(),
        ));
    }
    for values in [
        &revision.required_outputs,
        &revision.acceptance_criteria,
        &revision.approvals_required,
    ] {
        if values.len() > 100 || values.iter().any(|value| !value.is_object()) {
            return Err(StoreError::Invalid(
                "TaskSpec revision contains an invalid structured requirement".to_owned(),
            ));
        }
    }
    if revision.constraints.len() > 100
        || revision.non_goals.len() > 100
        || revision
            .constraints
            .iter()
            .chain(&revision.non_goals)
            .any(|value| value.len() > 8192 || value.chars().any(char::is_control))
        || revision.input_refs.len() > 100
    {
        return Err(StoreError::Invalid(
            "TaskSpec revision contains an oversized field".to_owned(),
        ));
    }
    Ok(())
}

fn load_task_spec_revision_receipt(
    connection: &Connection,
    principal_id: &str,
    request_id: &str,
    request_payload: &Value,
) -> Result<Option<CommittedTaskSpecRevision>, StoreError> {
    let request_digest = digest(&canonical_json(request_payload)?);
    let prior: Option<(String, Option<String>, Option<String>)> = connection.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![principal_id, request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    let Some((prior_digest, response_json, response_digest)) = prior else {
        return Ok(None);
    };
    if prior_digest != request_digest {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let response_json = response_json.ok_or_else(|| {
        StoreError::Integrity("TaskSpec idempotency receipt is incomplete".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "TaskSpec idempotency response digest does not match".to_owned(),
        ));
    }
    serde_json::from_str(&response_json)
        .map(Some)
        .map_err(|error| StoreError::Integrity(error.to_string()))
}

fn revise_task_spec_transaction(
    connection: &mut Connection,
    commit: TaskSpecRevisionCommit,
    state_ref: AggregateStateRef,
) -> Result<CommittedTaskSpecRevision, StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    if let Some(receipt) = load_task_spec_revision_receipt(
        &transaction,
        &commit.request.principal_id,
        &commit.request.request_id,
        &commit.request.request_payload,
    )? {
        return Ok(receipt);
    }

    let revision = &commit.task_spec_revision;
    if state_ref.entity_revision != commit.task.version || state_ref.record_schema_version != 1 {
        return Err(StoreError::Integrity(
            "TaskSpec revision aggregate-state reference is inconsistent".to_owned(),
        ));
    }
    let workspace: Option<(String, String)> = transaction
        .query_row(
            "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id = ?1",
            [&revision.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((owner, workspace_status)) = workspace else {
        return Err(StoreError::NotFound);
    };
    if owner != commit.request.principal_id {
        return Err(StoreError::Invalid(
            "authenticated Principal does not own this Workspace".to_owned(),
        ));
    }
    if workspace_status != "ACTIVE" {
        return Err(StoreError::Invalid(
            "archived Workspace is read-only".to_owned(),
        ));
    }
    let current = load_task_view(&transaction, &revision.workspace_id, &revision.task_id)?
        .ok_or(StoreError::NotFound)?;
    if current.task.version != commit.expected_task_version
        || current.task.current_spec_revision.checked_add(1) != Some(revision.revision)
        || revision.parent_revisions.len() != 1
        || revision.parent_revisions.first().copied() != Some(current.task.current_spec_revision)
        || current.task.status != "READY"
        || current.task.current_plan_revision.is_some()
        || commit.task.lead_agent_binding_id != current.task.lead_agent_binding_id
    {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_task_version),
            actual: Some(current.task.version),
        });
    }
    let live_planner: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE workspace_id = ?1 AND task_id = ?2 AND scope_kind = 'TASK_PLANNING' AND status IN ('STARTING', 'ACTIVE', 'INTERRUPTING', 'CLOSING'))",
        params![revision.workspace_id, revision.task_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if live_planner {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_task_version),
            actual: Some(current.task.version),
        });
    }
    let parent_exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_spec_revisions WHERE task_id = ?1 AND workspace_id = ?2 AND revision = ?3)",
        params![revision.task_id, revision.workspace_id, to_sql_i64(current.task.current_spec_revision, "TaskSpec parent revision")?],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !parent_exists {
        return Err(StoreError::Integrity(
            "TaskSpec parent revision is missing".to_owned(),
        ));
    }

    validate_task_resource_inputs(&transaction, &revision.workspace_id, &revision.input_refs)?;
    if let Some(binding_id) = revision.preferred_lead_agent_binding_id.as_deref() {
        let eligible: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2 AND enabled = 1 AND lead_eligible = 1)",
            params![revision.workspace_id, binding_id],
            |row| row.get(0),
        ).map_err(map_database_error)?;
        if !eligible {
            return Err(StoreError::Invalid(
                "preferred lead AgentBinding is not eligible".to_owned(),
            ));
        }
    }

    insert_task_spec_revision_row(&transaction, revision)?;
    let next_task_version = commit
        .expected_task_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Integrity("Task version exhausted".to_owned()))?;
    let changed = transaction.execute(
        "UPDATE tasks SET current_spec_revision = ?1, updated_at = ?2, version = ?3 WHERE workspace_id = ?4 AND task_id = ?5 AND version = ?6 AND current_spec_revision = ?7 AND current_plan_revision IS NULL AND status = 'READY'",
        params![to_sql_i64(revision.revision, "TaskSpec revision")?, revision.created_at,
            to_sql_i64(next_task_version, "Task version")?, revision.workspace_id, revision.task_id,
            to_sql_i64(commit.expected_task_version, "Task version")?,
            to_sql_i64(current.task.current_spec_revision, "TaskSpec revision")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_task_version),
            actual: None,
        });
    }

    let event = insert_domain_event(&transaction, &commit.event, &state_ref)?;
    let committed = CommittedTaskSpecRevision {
        revision: revision.clone(),
        task_version: next_task_version,
        event,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    transaction.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![commit.request.principal_id, commit.request.request_id,
            digest(&canonical_json(&commit.request.request_payload)?), response_json,
            digest(response_json.as_bytes()), commit.event.recorded_at],
    ).map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn insert_task_spec_revision_row(
    transaction: &rusqlite::Transaction<'_>,
    revision: &TaskSpecRevisionRecord,
) -> Result<(), StoreError> {
    let encode = |value: &Value| -> Result<String, StoreError> {
        String::from_utf8(canonical_json(value)?)
            .map_err(|error| StoreError::Invalid(error.to_string()))
    };
    let encode_strings = |value: &[String]| -> Result<String, StoreError> {
        let value =
            serde_json::to_value(value).map_err(|error| StoreError::Invalid(error.to_string()))?;
        String::from_utf8(canonical_json(&value)?)
            .map_err(|error| StoreError::Invalid(error.to_string()))
    };
    let parent_json = encode(
        &serde_json::to_value(&revision.parent_revisions)
            .map_err(|error| StoreError::Invalid(error.to_string()))?,
    )?;
    let constraints_json = encode_strings(&revision.constraints)?;
    let non_goals_json = encode_strings(&revision.non_goals)?;
    let inputs_json = encode(
        &serde_json::to_value(&revision.input_refs)
            .map_err(|error| StoreError::Invalid(error.to_string()))?,
    )?;
    let outputs_json = encode(
        &serde_json::to_value(&revision.required_outputs)
            .map_err(|error| StoreError::Invalid(error.to_string()))?,
    )?;
    let criteria_json = encode(
        &serde_json::to_value(&revision.acceptance_criteria)
            .map_err(|error| StoreError::Invalid(error.to_string()))?,
    )?;
    let approvals_json = encode(
        &serde_json::to_value(&revision.approvals_required)
            .map_err(|error| StoreError::Invalid(error.to_string()))?,
    )?;
    let budget_json = revision.budget.as_ref().map(encode).transpose()?;
    let delegation_budget_json = revision
        .delegation_budget_policy
        .as_ref()
        .map(encode)
        .transpose()?;
    let failover_json = encode(&revision.lead_failover_policy)?;
    let source_refs_json = encode_strings(&revision.source_message_refs)?;
    let placement_json = encode(&revision.placement_preference)?;
    let authored_by_json = encode(&revision.authored_by)?;
    transaction.execute(
        "INSERT INTO task_spec_revisions(task_id, workspace_id, revision, parent_revisions_json, objective, task_category, constraints_json, non_goals_json, input_refs_json, workspace_instruction_revision, required_outputs_json, acceptance_criteria_json, approvals_required_json, budget_json, delegation_budget_policy_json, lead_failover_policy_json, deadline, source_message_refs_json, placement_preference, preferred_lead_agent_binding_id, authored_by_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
        params![revision.task_id, revision.workspace_id, to_sql_i64(revision.revision, "TaskSpec revision")?,
            parent_json, revision.objective, revision.task_category, constraints_json, non_goals_json,
            inputs_json, revision.workspace_instruction_revision.map(|value| to_sql_i64(value, "Workspace instruction revision")).transpose()?,
            outputs_json, criteria_json, approvals_json, budget_json, delegation_budget_json, failover_json,
            revision.deadline, source_refs_json, placement_json, revision.preferred_lead_agent_binding_id,
            authored_by_json, revision.created_at],
    ).map_err(map_database_error)?;
    Ok(())
}

fn payload_has_exact_keys(payload: &Value, expected: &[&str]) -> bool {
    let Some(object) = payload.as_object() else {
        return false;
    };
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn validate_plan_acceptance_commit(commit: &PlanAcceptanceCommit) -> Result<(), StoreError> {
    if commit.principal_id.trim().is_empty()
        || commit.request_id.trim().is_empty()
        || commit.workspace_id.trim().is_empty()
        || commit.task_id.trim().is_empty()
        || commit.expected_task_version == 0
        || commit.expected_task_spec_revision == 0
        || commit.plan_revision.task_id != commit.task_id
        || commit.plan_revision.revision != 1
        || commit.plan_revision.task_spec_revision != commit.expected_task_spec_revision
        || commit
            .plan_revision
            .produced_by_agent_session_id
            .trim()
            .is_empty()
        || commit.plan_revision.produced_by_attempt_id.is_some()
        || commit.plan_revision.steps.len() != commit.materialized_steps.len()
        || commit.step_events.len() != commit.materialized_steps.len()
    {
        return Err(StoreError::Invalid(
            "initial plan commit identity is inconsistent".to_owned(),
        ));
    }
    if commit.plan_event.workspace_id != commit.workspace_id
        || commit.plan_event.entity_type != "Task"
        || commit.plan_event.entity_id != commit.task_id
        || commit.plan_event.entity_revision != commit.expected_task_version.saturating_add(1)
        || commit.plan_event.event_type != "task.plan.revised.v1"
        || commit.plan_event.schema_version != 1
        || !payload_has_exact_keys(
            &commit.plan_event.payload,
            &[
                "task_id",
                "revision",
                "task_spec_revision",
                "produced_by_agent_session_id",
                "step_ids",
                "aggregate_version",
            ],
        )
        || commit
            .plan_event
            .payload
            .get("task_id")
            .and_then(Value::as_str)
            != Some(commit.task_id.as_str())
        || commit
            .plan_event
            .payload
            .get("revision")
            .and_then(Value::as_u64)
            != Some(1)
        || commit
            .plan_event
            .payload
            .get("task_spec_revision")
            .and_then(Value::as_u64)
            != Some(commit.expected_task_spec_revision)
        || commit
            .plan_event
            .payload
            .get("produced_by_agent_session_id")
            .and_then(Value::as_str)
            != Some(commit.plan_revision.produced_by_agent_session_id.as_str())
        || commit
            .plan_event
            .payload
            .get("step_ids")
            .and_then(Value::as_array)
            .is_none_or(|ids| {
                ids.len() != commit.materialized_steps.len()
                    || ids
                        .iter()
                        .zip(&commit.materialized_steps)
                        .any(|(id, step)| id.as_str() != Some(step.step_id.as_str()))
            })
        || commit
            .plan_event
            .payload
            .get("aggregate_version")
            .and_then(Value::as_u64)
            != Some(commit.expected_task_version.saturating_add(1))
        || commit.plan_event.event_id.trim().is_empty()
        || commit.plan_event.origin_runtime_id.trim().is_empty()
        || commit.plan_event.hlc_timestamp.trim().is_empty()
        || commit.plan_event.correlation_id.trim().is_empty()
        || commit.plan_event.recorded_at != commit.plan_revision.created_at
    {
        return Err(StoreError::Invalid(
            "Task plan event identity is inconsistent".to_owned(),
        ));
    }
    let planned = &commit.plan_revision.steps;
    let materialized = &commit.materialized_steps;
    let mut keys = std::collections::HashSet::new();
    let mut ids = std::collections::HashSet::new();
    let mut event_ids = std::collections::HashSet::new();
    event_ids.insert(commit.plan_event.event_id.as_str());
    for ((planned, step), event) in planned.iter().zip(materialized).zip(&commit.step_events) {
        if planned.logical_key.is_empty()
            || planned.logical_key.len() > 128
            || !planned
                .logical_key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            || !keys.insert(planned.logical_key.as_str())
            || step.task_id != commit.task_id
            || step.plan_revision != 1
            || step.logical_key.as_deref() != Some(planned.logical_key.as_str())
            || step.step_id.trim().is_empty()
            || step.step_id.len() > 200
            || step.step_id.chars().any(char::is_control)
            || !ids.insert(step.step_id.as_str())
            || step.title != planned.title
            || step.objective != planned.objective
            || step.dependencies.len() != planned.depends_on_logical_keys.len()
            || step.required_capabilities != planned.required_capabilities
            || step.acceptance_criteria != planned.acceptance_criteria
            || !matches!(step.status.as_str(), "READY" | "PENDING")
            || step.status
                != if step.dependencies.is_empty() {
                    "READY"
                } else {
                    "PENDING"
                }
            || step.version != 1
            || step.current_attempt_id.is_some()
            || step.created_at != commit.plan_revision.created_at
            || step.updated_at != commit.plan_revision.created_at
            || planned.title.trim().is_empty()
            || planned.title.len() > 512
            || planned.objective.trim().is_empty()
            || planned.objective.len() > 16 * 1024
            || planned.title.chars().any(char::is_control)
            || planned.objective.chars().any(char::is_control)
            || planned.required_capabilities.iter().any(|capability| {
                !capability.is_object()
                    || capability
                        .get("semantic_requirement")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                    || capability
                        .get("operation_ids")
                        .and_then(Value::as_array)
                        .is_none_or(|operations| {
                            operations
                                .iter()
                                .any(|operation| operation.as_str().is_none_or(str::is_empty))
                        })
                    || capability
                        .get("resource_scope")
                        .is_some_and(|scope| !scope.is_object())
            })
            || planned.acceptance_criteria.iter().any(|criterion| {
                !criterion.is_object()
                    || criterion
                        .get("criterion_id")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                    || criterion
                        .get("description")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                    || !matches!(
                        criterion.get("required_evidence").and_then(Value::as_str),
                        Some("REPORTED" | "OBSERVED" | "VERIFIED")
                    )
                    || criterion
                        .get("mandatory")
                        .and_then(Value::as_bool)
                        .is_none()
                    || criterion
                        .get("subject_refs")
                        .is_some_and(|refs| !refs.is_array())
                    || criterion
                        .get("verifier_hint")
                        .is_some_and(|hint| !hint.is_null() && hint.as_str().is_none())
            })
            || event.workspace_id != commit.workspace_id
            || event.entity_type != "Step"
            || event.entity_id != step.step_id
            || event.entity_revision != 1
            || event.event_type != "step.created.v1"
            || event.schema_version != 1
            || !payload_has_exact_keys(
                &event.payload,
                &[
                    "step_id",
                    "task_id",
                    "plan_revision",
                    "logical_key",
                    "dependencies",
                ],
            )
            || event.payload.get("step_id").and_then(Value::as_str) != Some(step.step_id.as_str())
            || event.payload.get("task_id").and_then(Value::as_str) != Some(step.task_id.as_str())
            || event.payload.get("plan_revision").and_then(Value::as_u64) != Some(1)
            || event.payload.get("logical_key").and_then(Value::as_str)
                != Some(planned.logical_key.as_str())
            || event
                .payload
                .get("dependencies")
                .and_then(Value::as_array)
                .is_none_or(|deps| {
                    deps.len() != step.dependencies.len()
                        || deps
                            .iter()
                            .zip(&step.dependencies)
                            .any(|(dependency, expected)| {
                                dependency.as_str() != Some(expected.as_str())
                            })
                })
            || event.event_id.trim().is_empty()
            || !event_ids.insert(event.event_id.as_str())
            || event.origin_runtime_id != commit.plan_event.origin_runtime_id
            || event.hlc_timestamp.trim().is_empty()
            || event.correlation_id != commit.plan_event.correlation_id
        {
            return Err(StoreError::Invalid(
                "materialized plan Steps are inconsistent".to_owned(),
            ));
        }
    }
    let key_to_id = planned
        .iter()
        .zip(materialized)
        .map(|(proposal, step)| (proposal.logical_key.as_str(), step.step_id.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    for (proposal, step) in planned.iter().zip(materialized) {
        let expected_dependencies = proposal
            .depends_on_logical_keys
            .iter()
            .map(|key| {
                key_to_id
                    .get(key.as_str())
                    .copied()
                    .ok_or_else(|| StoreError::Invalid("plan dependency key is missing".to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if step
            .dependencies
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != expected_dependencies
        {
            return Err(StoreError::Invalid(
                "Step dependencies do not match the proposed graph".to_owned(),
            ));
        }
    }
    let by_key = planned
        .iter()
        .map(|step| (step.logical_key.as_str(), step))
        .collect::<std::collections::HashMap<_, _>>();
    fn visit<'a>(
        key: &'a str,
        by_key: &std::collections::HashMap<&'a str, &'a storage_core::PlannedStepRecord>,
        visiting: &mut std::collections::HashSet<&'a str>,
        visited: &mut std::collections::HashSet<&'a str>,
    ) -> bool {
        if visited.contains(key) {
            return true;
        }
        if !visiting.insert(key) {
            return false;
        }
        let acyclic = by_key.get(key).is_some_and(|step| {
            let mut unique = std::collections::HashSet::new();
            step.depends_on_logical_keys.iter().all(|dependency| {
                dependency != key
                    && unique.insert(dependency.as_str())
                    && visit(dependency, by_key, visiting, visited)
            })
        });
        visiting.remove(key);
        if acyclic {
            visited.insert(key);
        }
        acyclic
    }
    let mut visiting = std::collections::HashSet::new();
    let mut visited = std::collections::HashSet::new();
    if planned.is_empty()
        || planned.len() > 100
        || !planned
            .iter()
            .all(|step| visit(&step.logical_key, &by_key, &mut visiting, &mut visited))
    {
        return Err(StoreError::Invalid(
            "initial plan is empty, oversized, or cyclic".to_owned(),
        ));
    }
    Ok(())
}

fn accept_initial_plan_transaction(
    connection: &mut Connection,
    commit: PlanAcceptanceCommit,
    task_state_ref: AggregateStateRef,
    step_state_refs: Vec<AggregateStateRef>,
) -> Result<PlanAcceptance, StoreError> {
    if task_state_ref.entity_revision != commit.expected_task_version.saturating_add(1)
        || task_state_ref.record_schema_version != 1
        || step_state_refs.len() != commit.materialized_steps.len()
        || step_state_refs
            .iter()
            .zip(&commit.materialized_steps)
            .any(|(state, step)| {
                state.entity_revision != step.version || state.record_schema_version != 1
            })
    {
        return Err(StoreError::Invalid(
            "plan aggregate state references are inconsistent".to_owned(),
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let request_digest = digest(&canonical_json(&commit.request_payload)?);
    let owner: Option<(String, String)> = transaction
        .query_row(
            "SELECT w.owner_principal_id, w.status FROM workspaces w WHERE w.workspace_id = ?1",
            [&commit.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((owner, workspace_status)) = owner else {
        return Err(StoreError::NotFound);
    };
    if owner != commit.principal_id {
        return Err(StoreError::Invalid(
            "authenticated Principal does not own this Workspace".to_owned(),
        ));
    }
    let prior: Option<(String, Option<String>, Option<String>)> = transaction.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![commit.principal_id, commit.request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    if let Some((prior_digest, response_json, response_digest)) = prior {
        if prior_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response_json = response_json.ok_or_else(|| {
            StoreError::Integrity("Plan idempotency receipt is incomplete".to_owned())
        })?;
        if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "Plan idempotency response digest does not match".to_owned(),
            ));
        }
        return serde_json::from_str(&response_json)
            .map_err(|error| StoreError::Integrity(error.to_string()));
    }
    if workspace_status != "ACTIVE" {
        return Err(StoreError::Invalid(
            "archived Workspace is read-only".to_owned(),
        ));
    }
    let task: Option<(i64, i64, Option<i64>, String, String)> = transaction.query_row(
        "SELECT version, current_spec_revision, current_plan_revision, status, lead_agent_binding_id
         FROM tasks WHERE workspace_id = ?1 AND task_id = ?2",
        params![commit.workspace_id, commit.task_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).optional().map_err(map_database_error)?;
    let Some((task_version, spec_revision, current_plan_revision, task_status, lead_binding_id)) =
        task
    else {
        return Err(StoreError::NotFound);
    };
    let task_version = from_sql_i64(task_version, "Task version")?;
    let spec_revision = from_sql_i64(spec_revision, "TaskSpec revision")?;
    if task_version != commit.expected_task_version
        || spec_revision != commit.expected_task_spec_revision
        || current_plan_revision.is_some()
        || task_status != "RUNNING"
    {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_task_version),
            actual: Some(task_version),
        });
    }
    // Expiry is evaluated at the durable admission boundary, never against an
    // event/proposal timestamp controlled by the caller or adapter.
    let clock_now = OffsetDateTime::now_utc();
    let admission_now = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        clock_now.year(),
        u8::from(clock_now.month()),
        clock_now.day(),
        clock_now.hour(),
        clock_now.minute(),
        clock_now.second(),
        clock_now.nanosecond(),
    );
    let producer_active: bool = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM agent_sessions s
           JOIN agent_bindings b ON b.workspace_id = s.workspace_id AND b.agent_binding_id = s.agent_binding_id
           JOIN runtimes r ON r.runtime_id = s.runtime_id
           JOIN runtime_incarnations ri
             ON ri.runtime_id = r.runtime_id
            AND ri.runtime_incarnation_id = s.runtime_incarnation_id
           JOIN agent_session_host_bindings shb ON shb.agent_session_id = s.agent_session_id
           JOIN agent_host_instances h
             ON h.host_instance_id = shb.host_instance_id
            AND h.runtime_id = s.runtime_id
            AND h.runtime_incarnation_id = s.runtime_incarnation_id
            AND h.endpoint_id = s.endpoint_id
           JOIN agent_endpoints e
             ON e.endpoint_id = s.endpoint_id
            AND e.agent_profile_id = b.agent_profile_id
           JOIN agent_endpoint_bindings eb
             ON eb.endpoint_id = e.endpoint_id
            AND eb.runtime_id = s.runtime_id
            AND eb.runtime_incarnation_id = s.runtime_incarnation_id
           JOIN runtime_workspace_bindings rwb
             ON rwb.runtime_id = s.runtime_id
            AND rwb.workspace_id = s.workspace_id
            AND rwb.status = 'ACTIVE'
           WHERE s.agent_session_id = ?1 AND s.workspace_id = ?2 AND s.task_id = ?3
             AND s.task_spec_revision = ?4 AND s.scope_kind = 'TASK_PLANNING'
             AND s.status = 'ACTIVE' AND s.attempt_id IS NULL
             AND s.agent_binding_id = ?5 AND b.enabled = 1 AND b.lead_eligible = 1
             AND (b.runtime_id IS NULL OR b.runtime_id = s.runtime_id)
             AND (json_extract(b.endpoint_selection_policy_json, '$.mode') <> 'PINNED_ENDPOINT'
                  OR json_extract(b.endpoint_selection_policy_json, '$.endpoint_id') = s.endpoint_id)
             AND r.current_incarnation_id = s.runtime_incarnation_id
             AND r.availability = 'ONLINE'
             AND ri.recovery_state = 'READY'
             AND h.state IN ('READY', 'BUSY')
             AND (eb.expires_at IS NULL OR eb.expires_at > ?6)
             AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'EXECUTOR')
             AND s.runtime_id = ?7
         )",
        params![commit.plan_revision.produced_by_agent_session_id, commit.workspace_id, commit.task_id,
            to_sql_i64(commit.expected_task_spec_revision, "TaskSpec revision")?, lead_binding_id,
            admission_now, commit.plan_event.origin_runtime_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !producer_active {
        return Err(StoreError::Invalid(
            "plan producer is no longer the active lead planning session".to_owned(),
        ));
    }

    let steps_json = String::from_utf8(canonical_json(&commit.plan_revision.steps)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    transaction.execute(
        "INSERT INTO plan_revisions(task_id, revision, task_spec_revision, produced_by_agent_session_id, produced_by_attempt_id, steps_json, reason_for_revision, created_at)
         VALUES (?1, 1, ?2, ?3, NULL, ?4, ?5, ?6)",
        params![commit.task_id, to_sql_i64(commit.expected_task_spec_revision, "TaskSpec revision")?,
            commit.plan_revision.produced_by_agent_session_id, steps_json,
            commit.plan_revision.reason_for_revision, commit.plan_revision.created_at],
    ).map_err(map_database_error)?;
    for step in &commit.materialized_steps {
        transaction.execute(
            "INSERT INTO steps(step_id, task_id, plan_revision, logical_key, title, objective, dependencies_json, required_capabilities_json, acceptance_criteria_json, status, current_attempt_id, created_at, updated_at, version)
             VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?10, 1)",
            params![step.step_id, step.task_id, step.logical_key, step.title, step.objective,
                String::from_utf8(canonical_json(&step.dependencies)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
                String::from_utf8(canonical_json(&step.required_capabilities)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
                String::from_utf8(canonical_json(&step.acceptance_criteria)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
                step.status, step.created_at],
        ).map_err(map_database_error)?;
    }
    let next_task_version = commit
        .expected_task_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Integrity("Task version exhausted".to_owned()))?;
    let changed = transaction
        .execute(
            "UPDATE tasks SET current_plan_revision = 1, updated_at = ?1, version = ?2
         WHERE workspace_id = ?3 AND task_id = ?4 AND version = ?5
           AND current_spec_revision = ?6 AND current_plan_revision IS NULL AND status = 'RUNNING'",
            params![
                commit.plan_revision.created_at,
                to_sql_i64(next_task_version, "Task version")?,
                commit.workspace_id,
                commit.task_id,
                to_sql_i64(commit.expected_task_version, "Task version")?,
                to_sql_i64(commit.expected_task_spec_revision, "TaskSpec revision")?
            ],
        )
        .map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_task_version),
            actual: None,
        });
    }

    let accepted = PlanAcceptance {
        plan_revision: commit.plan_revision.clone(),
        materialized_steps: commit.materialized_steps.clone(),
        task_version: next_task_version,
    };
    let plan_event = insert_domain_event(&transaction, &commit.plan_event, &task_state_ref)?;
    for ((event, step), state_ref) in commit
        .step_events
        .iter()
        .zip(&commit.materialized_steps)
        .zip(&step_state_refs)
    {
        if event.entity_id != step.step_id {
            return Err(StoreError::Invalid(
                "Step event order/identity mismatch".to_owned(),
            ));
        }
        insert_domain_event(&transaction, event, state_ref)?;
    }
    let response_json = String::from_utf8(canonical_json(&accepted)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    transaction.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![commit.principal_id, commit.request_id, request_digest, response_json,
            digest(response_json.as_bytes()), plan_event.recorded_at],
    ).map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(accepted)
}

fn insert_domain_event<D, S>(
    transaction: &rusqlite::Transaction<'_>,
    draft: D,
    state_ref: S,
) -> Result<storage_core::DomainEvent, StoreError>
where
    D: std::borrow::Borrow<EventDraft>,
    S: std::borrow::Borrow<AggregateStateRef>,
{
    let draft = draft.borrow();
    let state_ref = state_ref.borrow();
    if draft.schema_version != 1
        || !draft.event_type.ends_with(".v1")
        || draft.workspace_id.trim().is_empty()
        || draft.entity_id.trim().is_empty()
        || draft.origin_runtime_id.trim().is_empty()
        || state_ref.entity_revision != draft.entity_revision
    {
        return Err(StoreError::Invalid(
            "domain event identity/state reference is invalid".to_owned(),
        ));
    }
    transaction
        .execute(
            "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence)
         VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id)
         DO UPDATE SET last_sequence = last_sequence + 1",
            params![draft.workspace_id, draft.origin_runtime_id],
        )
        .map_err(map_database_error)?;
    let sequence: i64 = transaction.query_row(
        "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
        params![draft.workspace_id, draft.origin_runtime_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    let payload_json = String::from_utf8(canonical_json(&draft.payload)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let event = storage_core::DomainEvent {
        event_id: draft.event_id.clone(),
        workspace_id: draft.workspace_id.clone(),
        entity_type: draft.entity_type.clone(),
        entity_id: draft.entity_id.clone(),
        origin_runtime_id: draft.origin_runtime_id.clone(),
        origin_sequence: from_sql_i64(sequence, "origin sequence")?,
        entity_revision: draft.entity_revision,
        hlc_timestamp: draft.hlc_timestamp.clone(),
        correlation_id: draft.correlation_id.clone(),
        causation_id: draft.causation_id.clone(),
        schema_version: draft.schema_version,
        event_type: draft.event_type.clone(),
        payload: draft.payload.clone(),
        aggregate_state_ref: state_ref.clone(),
        recorded_at: draft.recorded_at.clone(),
        payload_digest: digest(payload_json.as_bytes()),
    };
    transaction.execute(
        "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![event.event_id, event.workspace_id, event.entity_type, event.entity_id,
            event.origin_runtime_id, to_sql_i64(event.origin_sequence, "origin sequence")?,
            to_sql_i64(event.entity_revision, "event revision")?, event.hlc_timestamp,
            event.correlation_id, event.causation_id, i64::from(event.schema_version),
            event.event_type, payload_json,
            String::from_utf8(canonical_json(&event.aggregate_state_ref)?).map_err(|error| StoreError::Invalid(error.to_string()))?,
            event.recorded_at, event.payload_digest],
    ).map_err(map_database_error)?;
    Ok(event)
}

fn load_agent_session(
    connection: &Connection,
    workspace_id: &str,
    agent_session_id: &str,
) -> Result<Option<AgentSessionRecord>, StoreError> {
    connection.query_row(
        "SELECT agent_session_id, workspace_id, scope_kind, conversation_id, conversation_turn_id,
                task_id, task_spec_revision, attempt_id, agent_binding_id, endpoint_id, runtime_id,
                runtime_incarnation_id, configuration_digest, harness_descriptor_digest, status,
                started_at, last_event_at, closed_at, version
         FROM agent_sessions WHERE workspace_id = ?1 AND agent_session_id = ?2",
        params![workspace_id, agent_session_id],
        agent_session_from_row,
    ).optional().map_err(map_database_error)
}

fn list_starting_task_planning_sessions(
    connection: &Connection,
    workspace_id: &str,
    limit: usize,
) -> Result<Vec<AgentSessionRecord>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT agent_session_id, workspace_id, scope_kind, conversation_id, conversation_turn_id,
                task_id, task_spec_revision, attempt_id, agent_binding_id, endpoint_id, runtime_id,
                runtime_incarnation_id, configuration_digest, harness_descriptor_digest, status,
                started_at, last_event_at, closed_at, version
         FROM agent_sessions
         WHERE scope_kind = 'TASK_PLANNING' AND status = 'STARTING'
           AND workspace_id = ?1
         ORDER BY started_at, agent_session_id LIMIT ?3",
    ).map_err(map_database_error)?;
    statement
        .query_map(
            params![
                workspace_id,
                to_sql_i64(limit as u64, "AgentSession recovery page size")?
            ],
            agent_session_from_row,
        )
        .map_err(map_database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)
}

fn agent_session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSessionRecord> {
    Ok(AgentSessionRecord {
        agent_session_id: row.get(0)?,
        workspace_id: row.get(1)?,
        scope_kind: row.get(2)?,
        conversation_id: row.get(3)?,
        conversation_turn_id: row.get(4)?,
        task_id: row.get(5)?,
        task_spec_revision: row_u64_opt(row, 6)?,
        attempt_id: row.get(7)?,
        agent_binding_id: row.get(8)?,
        endpoint_id: row.get(9)?,
        runtime_id: row.get(10)?,
        runtime_incarnation_id: row.get(11)?,
        configuration_digest: row.get(12)?,
        harness_descriptor_digest: row.get(13)?,
        status: row.get(14)?,
        started_at: row.get(15)?,
        last_event_at: row.get(16)?,
        closed_at: row.get(17)?,
        version: from_row_u64(row, 18)?,
    })
}

fn load_task_view(
    connection: &Connection,
    workspace_id: &str,
    task_id: &str,
) -> Result<Option<TaskView>, StoreError> {
    let task = connection.query_row(
        "SELECT task_id, workspace_id, conversation_id, current_spec_revision, current_plan_revision,
                status, resume_status, routine_id, routine_revision, automation_id,
                automation_occurrence_id, origin_coworker_id, origin_coworker_revision,
                lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                created_at, updated_at, completed_at, version
         FROM tasks WHERE workspace_id = ?1 AND task_id = ?2",
        params![workspace_id, task_id],
        task_from_row,
    ).optional().map_err(map_database_error)?;
    let Some(task) = task else {
        return Ok(None);
    };
    let spec = connection.query_row(
        "SELECT task_id, workspace_id, revision, parent_revisions_json, objective, task_category,
                constraints_json, non_goals_json, input_refs_json, workspace_instruction_revision,
                required_outputs_json, acceptance_criteria_json, approvals_required_json,
                budget_json, delegation_budget_policy_json, lead_failover_policy_json, deadline,
                source_message_refs_json, placement_preference, preferred_lead_agent_binding_id,
                authored_by_json, created_at
         FROM task_spec_revisions WHERE task_id = ?1 AND revision = ?2",
        params![task.task_id, to_sql_i64(task.current_spec_revision, "TaskSpec revision")?],
        task_spec_from_row,
    ).optional().map_err(map_database_error)?
        .ok_or_else(|| StoreError::Integrity("Task current spec revision is missing".to_owned()))?;
    Ok(Some(TaskView {
        task,
        current_spec_revision: spec,
    }))
}

fn list_task_spec_revisions(
    connection: &Connection,
    workspace_id: &str,
    task_id: &str,
) -> Result<Vec<TaskSpecRevisionRecord>, StoreError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE workspace_id = ?1 AND task_id = ?2)",
            params![workspace_id, task_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !exists {
        return Err(StoreError::NotFound);
    }
    let mut statement = connection.prepare(
        "SELECT task_id, workspace_id, revision, parent_revisions_json, objective, task_category,
                constraints_json, non_goals_json, input_refs_json, workspace_instruction_revision,
                required_outputs_json, acceptance_criteria_json, approvals_required_json,
                budget_json, delegation_budget_policy_json, lead_failover_policy_json, deadline,
                source_message_refs_json, placement_preference, preferred_lead_agent_binding_id,
                authored_by_json, created_at
         FROM task_spec_revisions WHERE task_id = ?1 ORDER BY revision ASC",
    ).map_err(map_database_error)?;
    statement
        .query_map([task_id], task_spec_from_row)
        .map_err(map_database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)
}

fn list_plan_revisions(
    connection: &Connection,
    workspace_id: &str,
    task_id: &str,
) -> Result<Vec<PlanRevisionRecord>, StoreError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE workspace_id = ?1 AND task_id = ?2)",
            params![workspace_id, task_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !exists {
        return Err(StoreError::NotFound);
    }
    let mut statement = connection
        .prepare(
            "SELECT p.task_id, p.revision, p.task_spec_revision, p.produced_by_agent_session_id,
                p.produced_by_attempt_id, p.steps_json, p.reason_for_revision, p.created_at
         FROM plan_revisions p JOIN tasks t ON t.task_id = p.task_id
         WHERE t.workspace_id = ?1 AND p.task_id = ?2 ORDER BY p.revision ASC",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(params![workspace_id, task_id], |row| {
            let steps_json: String = row.get(5)?;
            let steps = serde_json::from_str(&steps_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(PlanRevisionRecord {
                task_id: row.get(0)?,
                revision: from_row_u64(row, 1)?,
                task_spec_revision: from_row_u64(row, 2)?,
                produced_by_agent_session_id: row.get(3)?,
                produced_by_attempt_id: row.get(4)?,
                steps,
                reason_for_revision: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(map_database_error)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)
}

fn list_steps(
    connection: &Connection,
    workspace_id: &str,
    task_id: &str,
    plan_revision: Option<u64>,
) -> Result<Vec<StepRecord>, StoreError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE workspace_id = ?1 AND task_id = ?2)",
            params![workspace_id, task_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !exists {
        return Err(StoreError::NotFound);
    }
    let mut statement = connection
        .prepare(
            "SELECT s.step_id, s.task_id, s.plan_revision, s.logical_key, s.title, s.objective,
                s.dependencies_json, s.required_capabilities_json, s.acceptance_criteria_json,
                s.status, s.current_attempt_id, s.created_at, s.updated_at, s.version
         FROM steps s JOIN tasks t ON t.task_id = s.task_id
         WHERE t.workspace_id = ?1 AND s.task_id = ?2 AND (?3 IS NULL OR s.plan_revision = ?3)
         ORDER BY s.plan_revision ASC, s.created_at ASC, s.step_id ASC",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![
                workspace_id,
                task_id,
                plan_revision
                    .map(|value| to_sql_i64(value, "PlanRevision"))
                    .transpose()?
            ],
            |row| {
                let parse_json = |index: usize| -> rusqlite::Result<Value> {
                    let text: String = row.get(index)?;
                    serde_json::from_str(&text).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            index,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })
                };
                let dependencies: Vec<String> =
                    serde_json::from_value(parse_json(6)?).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                let required_capabilities: Vec<Value> = serde_json::from_value(parse_json(7)?)
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                let acceptance_criteria: Vec<Value> = serde_json::from_value(parse_json(8)?)
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            8,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                Ok(StepRecord {
                    step_id: row.get(0)?,
                    task_id: row.get(1)?,
                    plan_revision: from_row_u64(row, 2)?,
                    logical_key: row.get(3)?,
                    title: row.get(4)?,
                    objective: row.get(5)?,
                    dependencies,
                    required_capabilities,
                    acceptance_criteria,
                    status: row.get(9)?,
                    current_attempt_id: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    version: from_row_u64(row, 13)?,
                })
            },
        )
        .map_err(map_database_error)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)
}

fn list_tasks_page(
    connection: &Connection,
    workspace_id: &str,
    status: Option<&str>,
    conversation_id: Option<&str>,
    after_created_at: Option<&str>,
    after_task_id: Option<&str>,
    limit: usize,
) -> Result<Vec<TaskSummaryRecord>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT t.task_id, t.status, s.objective, t.created_at, t.updated_at
         FROM tasks t JOIN task_spec_revisions s
           ON s.task_id = t.task_id AND s.revision = t.current_spec_revision
         WHERE t.workspace_id = ?1
           AND (?2 IS NULL OR t.status = ?2)
           AND (?3 IS NULL OR t.conversation_id = ?3)
           AND (?4 IS NULL OR t.created_at < ?4 OR (t.created_at = ?4 AND t.task_id < ?5))
         ORDER BY t.created_at DESC, t.task_id DESC LIMIT ?6",
        )
        .map_err(map_database_error)?;
    statement
        .query_map(
            params![
                workspace_id,
                status,
                conversation_id,
                after_created_at,
                after_task_id,
                to_sql_i64(limit as u64, "Task page limit")?
            ],
            |row| {
                Ok(TaskSummaryRecord {
                    task_id: row.get(0)?,
                    status: row.get(1)?,
                    objective: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .map_err(map_database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)
}

fn task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRecord> {
    let blocking_json: String = row.get(14)?;
    let created_by_json: String = row.get(16)?;
    Ok(TaskRecord {
        task_id: row.get(0)?,
        workspace_id: row.get(1)?,
        conversation_id: row.get(2)?,
        current_spec_revision: from_row_u64(row, 3)?,
        current_plan_revision: row_u64_opt(row, 4)?,
        status: row.get(5)?,
        resume_status: row.get(6)?,
        routine_id: row.get(7)?,
        routine_revision: row_u64_opt(row, 8)?,
        automation_id: row.get(9)?,
        automation_occurrence_id: row.get(10)?,
        origin_coworker_id: row.get(11)?,
        origin_coworker_revision: row_u64_opt(row, 12)?,
        lead_agent_binding_id: row.get(13)?,
        blocking_conditions: serde_json::from_str(&blocking_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                14,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        priority: row.get(15)?,
        created_by: serde_json::from_str(&created_by_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                16,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
        completed_at: row.get(19)?,
        version: from_row_u64(row, 20)?,
    })
}

fn task_spec_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskSpecRevisionRecord> {
    use rusqlite::types::Type;
    let parse = |index: usize, value: String| -> rusqlite::Result<Value> {
        serde_json::from_str(&value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
        })
    };
    let parse_vec = |index: usize, value: String| -> rusqlite::Result<Vec<Value>> {
        serde_json::from_str(&value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
        })
    };
    let parent_json: String = row.get(3)?;
    let constraints_json: String = row.get(6)?;
    let non_goals_json: String = row.get(7)?;
    let input_refs_json: String = row.get(8)?;
    let outputs_json: String = row.get(10)?;
    let criteria_json: String = row.get(11)?;
    let approvals_json: String = row.get(12)?;
    let budget_json: Option<String> = row.get(13)?;
    let delegation_budget_json: Option<String> = row.get(14)?;
    let failover_json: String = row.get(15)?;
    let source_refs_json: String = row.get(17)?;
    let placement_json: String = row.get(18)?;
    let authored_by_json: String = row.get(20)?;
    Ok(TaskSpecRevisionRecord {
        task_id: row.get(0)?,
        workspace_id: row.get(1)?,
        revision: from_row_u64(row, 2)?,
        parent_revisions: serde_json::from_str(&parent_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(error))
        })?,
        objective: row.get(4)?,
        task_category: row.get(5)?,
        constraints: serde_json::from_str(&constraints_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(6, Type::Text, Box::new(error))
        })?,
        non_goals: serde_json::from_str(&non_goals_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(7, Type::Text, Box::new(error))
        })?,
        input_refs: parse_vec(8, input_refs_json)?,
        workspace_instruction_revision: row_u64_opt(row, 9)?,
        required_outputs: parse_vec(10, outputs_json)?,
        acceptance_criteria: parse_vec(11, criteria_json)?,
        approvals_required: parse_vec(12, approvals_json)?,
        budget: budget_json.map(|value| parse(13, value)).transpose()?,
        delegation_budget_policy: delegation_budget_json
            .map(|value| parse(14, value))
            .transpose()?,
        lead_failover_policy: parse(15, failover_json)?,
        deadline: row.get(16)?,
        source_message_refs: serde_json::from_str(&source_refs_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(17, Type::Text, Box::new(error))
        })?,
        placement_preference: parse(18, placement_json)?,
        preferred_lead_agent_binding_id: row.get(19)?,
        authored_by: parse(20, authored_by_json)?,
        created_at: row.get(21)?,
    })
}

fn from_row_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

fn row_u64_opt(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    let value: Option<i64> = row.get(index)?;
    value
        .map(|value| {
            u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
        })
        .transpose()
}

fn create_workspace_instruction_revision_transaction(
    connection: &mut Connection,
    request: WorkspaceCreateRequest,
    expected_version: u64,
    workspace: Workspace,
    instruction: WorkspaceInstructionRevisionRecord,
    draft: EventDraft,
    state_ref: AggregateStateRef,
) -> Result<CommittedWorkspaceInstructionRevision, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let request_digest = digest(&canonical_json(&request.request_payload)?);
    let prior: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            params![request.principal_id, request.request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    if let Some((prior_digest, response_json, response_digest)) = prior {
        if prior_digest != request_digest {
            return Err(StoreError::Conflict {
                expected: None,
                actual: None,
            });
        }
        let response_json = response_json.ok_or_else(|| {
            StoreError::Integrity(
                "Workspace instruction idempotency receipt is incomplete".to_owned(),
            )
        })?;
        if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
            return Err(StoreError::Integrity(
                "Workspace instruction idempotency response digest does not match".to_owned(),
            ));
        }
        return serde_json::from_str(&response_json)
            .map_err(|error| StoreError::Integrity(error.to_string()));
    }
    if request.principal_id.trim().is_empty() || request.request_id.trim().is_empty() {
        return Err(StoreError::Invalid(
            "principal and request IDs must not be empty".to_owned(),
        ));
    }
    if instruction.workspace_id != workspace.workspace_id
        || draft.workspace_id != workspace.workspace_id
        || draft.entity_id != workspace.workspace_id
        || draft.entity_type != "Workspace"
        || draft.event_type != "workspace.instructions.revision.created.v1"
        || draft.entity_revision != workspace.version
        || state_ref.entity_revision != workspace.version
        || state_ref.record_schema_version != 1
        || workspace.current_instruction_revision != Some(instruction.revision)
        || expected_version.checked_add(1) != Some(workspace.version)
    {
        return Err(StoreError::Invalid(
            "Workspace instruction event and aggregate are inconsistent".to_owned(),
        ));
    }

    let (actual_version, current_revision, status): (i64, Option<i64>, String) = tx
        .query_row(
            "SELECT version, current_instruction_revision, status FROM workspaces WHERE workspace_id = ?1",
            [&workspace.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;
    let actual_version = from_sql_i64(actual_version, "Workspace version")?;
    if actual_version != expected_version {
        return Err(StoreError::Conflict {
            expected: Some(expected_version),
            actual: Some(actual_version),
        });
    }
    if status != "ACTIVE" {
        return Err(StoreError::Invalid(
            "an archived Workspace is read-only".to_owned(),
        ));
    }
    let current_revision = current_revision
        .map(|value| from_sql_i64(value, "Workspace instruction revision"))
        .transpose()?;
    let expected_revision = current_revision
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("instruction revision overflow".to_owned()))?;
    if instruction.revision != expected_revision || instruction.parent_revisions.len() > 16 {
        return Err(StoreError::Conflict {
            expected: Some(expected_version),
            actual: Some(actual_version),
        });
    }
    let mut parents = instruction.parent_revisions.clone();
    parents.sort_unstable();
    parents.dedup();
    if parents.len() != instruction.parent_revisions.len()
        || parents.first().copied().unwrap_or_default() == 0
        || current_revision.is_some_and(|current| !parents.contains(&current))
        || (current_revision.is_none() && !parents.is_empty())
    {
        return Err(StoreError::Invalid(
            "instruction revision parents are invalid".to_owned(),
        ));
    }
    for parent in &parents {
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM workspace_instruction_revisions WHERE workspace_id = ?1 AND revision = ?2)",
                params![workspace.workspace_id, to_sql_i64(*parent, "instruction parent revision")?],
                |row| row.get(0),
            )
            .map_err(map_database_error)?;
        if !exists {
            return Err(StoreError::Invalid(
                "instruction parent does not exist in this Workspace".to_owned(),
            ));
        }
    }

    let resource_workspace = instruction
        .content_ref
        .get("workspace_id")
        .and_then(Value::as_str);
    let resource_id = instruction
        .content_ref
        .get("resource_id")
        .and_then(Value::as_str);
    let revision_id = instruction
        .content_ref
        .get("revision_id")
        .and_then(Value::as_str);
    if resource_workspace != Some(workspace.workspace_id.as_str()) {
        return Err(StoreError::Invalid(
            "instruction ResourceRef must be pinned to this Workspace".to_owned(),
        ));
    }
    let (stored_digest, size_bytes, media_type, context_document): (String, i64, String, Option<String>) = tx
        .query_row(
            "SELECT revision.content_digest, revision.size_bytes, revision.media_type, resource.context_document_json FROM resources resource JOIN resource_revisions revision ON revision.resource_id = resource.resource_id WHERE resource.workspace_id = ?1 AND resource.resource_id = ?2 AND revision.resource_revision_id = ?3",
            params![workspace.workspace_id, resource_id, revision_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;
    let active_context_document = match context_document {
        None => true,
        Some(metadata) => serde_json::from_str::<Value>(&metadata)
            .ok()
            .and_then(|value| {
                value
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .is_some_and(|status| status == "ACTIVE"),
    };
    if stored_digest != instruction.content_digest
        || size_bytes < 0
        || size_bytes > 64 * 1024
        || !media_type.starts_with("text/")
        || !active_context_document
    {
        return Err(StoreError::Invalid(
            "instruction Resource content is not eligible".to_owned(),
        ));
    }

    let parent_json = String::from_utf8(canonical_json(&instruction.parent_revisions)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let content_ref_json = String::from_utf8(canonical_json(&instruction.content_ref)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let authored_by_json = String::from_utf8(canonical_json(&instruction.authored_by)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO workspace_instruction_revisions(workspace_id, revision, parent_revisions_json, content_ref_json, content_digest, authored_by_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![workspace.workspace_id, to_sql_i64(instruction.revision, "instruction revision")?, parent_json, content_ref_json, instruction.content_digest, authored_by_json, instruction.created_at],
    ).map_err(map_database_error)?;
    let changed = tx.execute(
        "UPDATE workspaces SET name = ?1, owner_principal_id = ?2, replication_policy = ?3, current_instruction_revision = ?4, default_agent_binding_id = ?5, primary_coworker_id = ?6, hub_runtime_id = ?7, status = ?8, updated_at = ?9, version = ?10 WHERE workspace_id = ?11 AND version = ?12",
        params![workspace.name, workspace.owner_principal_id, workspace.replication_policy.as_str(), to_sql_i64(instruction.revision, "instruction revision")?, workspace.default_agent_binding_id, workspace.primary_coworker_id, workspace.hub_runtime_id, workspace.status, workspace.updated_at, to_sql_i64(workspace.version, "Workspace version")?, workspace.workspace_id, to_sql_i64(expected_version, "expected Workspace version")?],
    ).map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(expected_version),
            actual: None,
        });
    }

    tx.execute(
        "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1",
        params![draft.workspace_id, draft.origin_runtime_id],
    ).map_err(map_database_error)?;
    let origin_sequence: i64 = tx.query_row(
        "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
        params![draft.workspace_id, draft.origin_runtime_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    let payload_json = String::from_utf8(canonical_json(&draft.payload)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let event = DomainEvent {
        event_id: draft.event_id,
        workspace_id: draft.workspace_id,
        entity_type: draft.entity_type,
        entity_id: draft.entity_id,
        origin_runtime_id: draft.origin_runtime_id,
        origin_sequence: from_sql_i64(origin_sequence, "origin sequence")?,
        entity_revision: draft.entity_revision,
        hlc_timestamp: draft.hlc_timestamp,
        correlation_id: draft.correlation_id,
        causation_id: draft.causation_id,
        schema_version: draft.schema_version,
        event_type: draft.event_type,
        payload: draft.payload,
        aggregate_state_ref: state_ref,
        recorded_at: draft.recorded_at,
        payload_digest: digest(payload_json.as_bytes()),
    };
    tx.execute(
        "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![event.event_id, event.workspace_id, event.entity_type, event.entity_id, event.origin_runtime_id, to_sql_i64(event.origin_sequence, "origin sequence")?, to_sql_i64(event.entity_revision, "event revision")?, event.hlc_timestamp, event.correlation_id, event.causation_id, i64::from(event.schema_version), event.event_type, payload_json, String::from_utf8(canonical_json(&event.aggregate_state_ref)?).map_err(|error| StoreError::Invalid(error.to_string()))?, event.recorded_at, event.payload_digest],
    ).map_err(map_database_error)?;
    let committed = CommittedWorkspaceInstructionRevision {
        workspace,
        instruction_revision: instruction,
        event,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![request.principal_id, request.request_id, request_digest, response_json, digest(response_json.as_bytes()), committed.event.recorded_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn duration_as_nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

fn atomic_saturating_add(counter: &AtomicU64, value: u64) {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.saturating_add(value);
        match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Shutdown);
        if let Ok(mut join) = self.join.lock() {
            if let Some(handle) = join.take() {
                let _ = handle.join();
            }
        }
    }
}

fn validate_local_runtime_registration(
    runtime: &RuntimeRecord,
    incarnation: &RuntimeIncarnationRecord,
    observation: &RuntimeIncarnationLocalObservationRecord,
) -> Result<(), StoreError> {
    if runtime.runtime_id.trim().is_empty()
        || incarnation.runtime_incarnation_id.trim().is_empty()
        || runtime.runtime_id != incarnation.runtime_id
        || runtime.current_incarnation_id != incarnation.runtime_incarnation_id
        || observation.runtime_incarnation_id != incarnation.runtime_incarnation_id
        || runtime.availability != "RECOVERING"
        || incarnation.recovery_state != "RECOVERING"
        || incarnation.version != 1
        || incarnation.ready_at.is_some()
        || incarnation.stopped_at.is_some()
        || !(runtime.startup_policy == "MANUAL"
            || runtime.startup_policy == "LOGIN_BACKGROUND"
            || runtime.startup_policy == "ALWAYS_ON_SERVICE")
        || runtime.trust_zone != "PERSONAL_DEVICE"
        || !runtime.roles.iter().any(|role| role == "OPERATOR_ENDPOINT")
        || runtime.resource_capacity.get("sampled_at").is_none()
    {
        return Err(StoreError::Invalid(
            "local Runtime registration is inconsistent".to_owned(),
        ));
    }
    validate_timestamp(&runtime.device_identity.issued_at)?;
    validate_timestamp(&runtime.last_seen)?;
    validate_timestamp(&incarnation.process_started_at)?;
    validate_timestamp(&observation.observed_at)?;

    let public_key = runtime
        .device_identity
        .public_key
        .strip_prefix("ed25519:")
        .ok_or_else(|| StoreError::Invalid("Runtime public key encoding is invalid".to_owned()))?;
    let public_key = hex::decode(public_key)
        .map_err(|_| StoreError::Invalid("Runtime public key encoding is invalid".to_owned()))?;
    if public_key.len() != 32 || runtime.device_identity.key_version != 1 {
        return Err(StoreError::Invalid(
            "Runtime device identity is unsupported".to_owned(),
        ));
    }
    let public_key_digest = Sha256::digest(&public_key);
    if runtime.device_identity.device_id
        != format!("ed25519-sha256:{}", hex::encode(public_key_digest))
    {
        return Err(StoreError::Integrity(
            "Runtime device ID does not match its public key".to_owned(),
        ));
    }
    Ok(())
}

fn validate_runtime_state_update(update: &RuntimeIncarnationStateUpdate) -> Result<(), StoreError> {
    let expected_availability = match update.recovery_state.as_str() {
        "RECOVERING" => "RECOVERING",
        "DEGRADED" => "DEGRADED",
        "DRAINING" | "STOPPING" => "DRAINING",
        "STOPPED" => "OFFLINE",
        _ => {
            return Err(StoreError::Invalid(
                "Runtime lifecycle transition is unsupported".to_owned(),
            ));
        }
    };
    if update.runtime_id.trim().is_empty()
        || update.runtime_incarnation_id.trim().is_empty()
        || update.expected_version == 0
        || update.availability != expected_availability
        || (update.recovery_state == "STOPPED") != update.stopped_at.is_some()
    {
        return Err(StoreError::Invalid(
            "Runtime lifecycle update is inconsistent".to_owned(),
        ));
    }
    validate_timestamp(&update.observed_at)?;
    if let Some(stopped_at) = update.stopped_at.as_deref() {
        validate_timestamp(stopped_at)?;
    }
    Ok(())
}

fn validate_timestamp(value: &str) -> Result<(), StoreError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map(|_| ())
        .map_err(|_| StoreError::Invalid("Runtime timestamp is invalid".to_owned()))
}

fn validate_local_runtime_workspace_enrollment(
    request: &LocalRuntimeWorkspaceEnrollmentRequest,
) -> Result<(), StoreError> {
    validate_nonempty(&[
        &request.request.principal_id,
        &request.request.request_id,
        &request.runtime_workspace_binding_id,
        &request.runtime_id,
        &request.runtime_incarnation_id,
        &request.workspace_id,
        &request.correlation_id,
    ])?;
    validate_timestamp(&request.now)?;
    canonical_json(&request.request.request_payload)?;
    if request.expected_workspace_version == 0
        || request.runtime_workspace_binding_id.len() > 256
        || request.runtime_id.len() > 256
        || request.runtime_incarnation_id.len() > 256
        || request.workspace_id.len() > 256
        || request.correlation_id.len() > 256
    {
        return Err(StoreError::Invalid(
            "local Runtime enrollment request is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_local_runtime_workspace_binding_lookup(
    lookup: &LocalRuntimeWorkspaceBindingLookup,
) -> Result<(), StoreError> {
    validate_nonempty(&[
        &lookup.owner_principal_id,
        &lookup.workspace_id,
        &lookup.runtime_id,
        &lookup.runtime_incarnation_id,
    ])?;
    if lookup.owner_principal_id.len() > 256
        || lookup.workspace_id.len() > 256
        || lookup.runtime_id.len() > 256
        || lookup.runtime_incarnation_id.len() > 256
    {
        return Err(StoreError::Invalid(
            "local Runtime binding lookup is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn get_current_local_runtime_workspace_binding(
    connection: &Connection,
    lookup: &LocalRuntimeWorkspaceBindingLookup,
) -> Result<Option<RuntimeWorkspaceBindingRecord>, StoreError> {
    let row: Option<(String, String, String, String, String, String, String, Option<String>, i64)> = connection.query_row(
        "SELECT b.runtime_workspace_binding_id, b.runtime_id, b.workspace_id, b.enrollment_mode, b.status, b.roles_json, b.created_at, b.activated_at, b.version FROM runtime_workspace_bindings b JOIN workspaces w ON w.workspace_id = b.workspace_id JOIN runtimes r ON r.runtime_id = b.runtime_id JOIN runtime_incarnations i ON i.runtime_id = r.runtime_id AND i.runtime_incarnation_id = r.current_incarnation_id WHERE w.owner_principal_id = ?1 AND w.workspace_id = ?2 AND w.status = 'ACTIVE' AND b.workspace_id = ?2 AND b.runtime_id = ?3 AND b.status = 'ACTIVE' AND b.revoked_at IS NULL AND r.current_incarnation_id = ?4 AND i.runtime_incarnation_id = ?4 AND ((r.availability = 'ONLINE' AND i.recovery_state = 'READY') OR (r.availability = 'DEGRADED' AND i.recovery_state = 'DEGRADED')) AND r.trust_zone = 'PERSONAL_DEVICE' AND EXISTS (SELECT 1 FROM json_each(r.roles_json) rr WHERE rr.value = 'OPERATOR_ENDPOINT') AND EXISTS (SELECT 1 FROM json_each(b.roles_json) br WHERE br.value = 'EXECUTOR') AND EXISTS (SELECT 1 FROM json_each(b.roles_json) br WHERE br.value = 'OPERATOR_ENDPOINT')",
        params![lookup.owner_principal_id, lookup.workspace_id, lookup.runtime_id, lookup.runtime_incarnation_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?)),
    ).optional().map_err(map_database_error)?;
    let Some((
        runtime_workspace_binding_id,
        runtime_id,
        workspace_id,
        enrollment_mode,
        status,
        roles_json,
        created_at,
        activated_at,
        version,
    )) = row
    else {
        return Ok(None);
    };
    let roles = serde_json::from_str(&roles_json).map_err(|error| {
        StoreError::Integrity(format!(
            "stored Runtime Workspace roles are malformed: {error}"
        ))
    })?;
    Ok(Some(RuntimeWorkspaceBindingRecord {
        runtime_workspace_binding_id,
        runtime_id,
        workspace_id,
        enrollment_mode,
        status,
        roles,
        created_at,
        activated_at,
        revoked_at: None,
        version: from_sql_i64(version, "Runtime Workspace binding version")?,
    }))
}

fn enroll_local_runtime_transaction(
    connection: &mut Connection,
    request: LocalRuntimeWorkspaceEnrollmentRequest,
    request_digest: String,
    binding: RuntimeWorkspaceBindingRecord,
) -> Result<RuntimeWorkspaceBindingRecord, StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    if let Some(replay) = verify_request_receipt::<RuntimeWorkspaceBindingRecord>(
        &tx,
        &request.request.principal_id,
        &request.request.request_id,
        &request_digest,
    )? {
        return Ok(replay);
    }

    let workspace: Option<(String, String, i64)> = tx
        .query_row(
            "SELECT owner_principal_id, status, version FROM workspaces WHERE workspace_id = ?1",
            [&request.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((owner, status, actual_version)) = workspace else {
        return Err(StoreError::NotFound);
    };
    if owner != request.request.principal_id || status != "ACTIVE" {
        return Err(StoreError::NotFound);
    }
    let actual_version = from_sql_i64(actual_version, "Workspace version")?;
    if actual_version != request.expected_workspace_version {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_workspace_version),
            actual: Some(actual_version),
        });
    }

    let runtime: Option<(Option<String>, String, String, String, Option<String>)> = tx.query_row(
        "SELECT r.current_incarnation_id, r.availability, r.trust_zone, r.roles_json, i.recovery_state FROM runtimes r LEFT JOIN runtime_incarnations i ON i.runtime_id = r.runtime_id AND i.runtime_incarnation_id = r.current_incarnation_id WHERE r.runtime_id = ?1",
        [&request.runtime_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).optional().map_err(map_database_error)?;
    let Some((
        current_incarnation,
        availability,
        trust_zone,
        runtime_roles_json,
        incarnation_state,
    )) = runtime
    else {
        return Err(StoreError::NotFound);
    };
    if current_incarnation.as_deref() != Some(request.runtime_incarnation_id.as_str()) {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let runtime_roles: Vec<String> =
        serde_json::from_str(&runtime_roles_json).map_err(|error| {
            StoreError::Integrity(format!("stored Runtime roles are malformed: {error}"))
        })?;
    if !matches!(
        (availability.as_str(), incarnation_state.as_deref()),
        ("ONLINE", Some("READY")) | ("DEGRADED", Some("DEGRADED"))
    ) || trust_zone != "PERSONAL_DEVICE"
        || !runtime_roles.iter().any(|role| role == "OPERATOR_ENDPOINT")
    {
        return Err(StoreError::Invalid(
            "local Runtime must be the current serving PERSONAL_DEVICE Operator Runtime".to_owned(),
        ));
    }

    let duplicate_open_binding: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM runtime_workspace_bindings WHERE runtime_id = ?1 AND workspace_id = ?2 AND status IN ('PENDING', 'ACTIVE'))",
        params![request.runtime_id, request.workspace_id],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if duplicate_open_binding {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }

    let roles_json = String::from_utf8(canonical_json(&binding.roles)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id, runtime_id, workspace_id, enrollment_mode, status, roles_json, created_at, activated_at, revoked_at, version) VALUES (?1, ?2, ?3, 'LOCAL_ENROLLMENT', 'ACTIVE', ?4, ?5, ?5, NULL, 1)",
        params![binding.runtime_workspace_binding_id, binding.runtime_id, binding.workspace_id, roles_json, binding.created_at],
    ).map_err(map_database_error)?;

    let principal_json = String::from_utf8(canonical_json(&json!({
        "principal_id": request.request.principal_id,
        "kind": "USER",
    }))?)
    .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let resource_ref_json = String::from_utf8(canonical_json(&json!({
        "kind": "RUNTIME_ROLE",
        "runtime_id": binding.runtime_id,
        "runtime_workspace_binding_id": binding.runtime_workspace_binding_id,
        "roles": binding.roles,
    }))?)
    .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let audit_identity = json!({
        "runtime_workspace_binding_id": binding.runtime_workspace_binding_id,
        "runtime_id": binding.runtime_id,
        "runtime_incarnation_id": request.runtime_incarnation_id,
        "workspace_id": binding.workspace_id,
        "enrollment_mode": binding.enrollment_mode,
        "status": binding.status,
        "roles": binding.roles,
    });
    let audit_digest = digest(&canonical_json(&audit_identity)?);
    let audit_record_id = format!("audit_runtime_enrollment_{}", &audit_digest[7..]);
    tx.execute(
        "INSERT INTO audit_records(audit_record_id, workspace_id, principal_json, action, resource_ref_json, decision, reason_code, correlation_id, occurred_at, payload_digest) VALUES (?1, ?2, ?3, 'runtime.workspace.local_enrollment', ?4, 'ALLOW', 'OWNER_REQUESTED_LOCAL_ENROLLMENT', ?5, ?6, ?7)",
        params![audit_record_id, binding.workspace_id, principal_json, resource_ref_json, request.correlation_id, request.now, audit_digest],
    ).map_err(map_database_error)?;

    let response_json = String::from_utf8(canonical_json(&binding)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![request.request.principal_id, request.request.request_id, request_digest, response_json, digest(response_json.as_bytes()), request.now],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(binding)
}

fn validate_nonempty(values: &[&str]) -> Result<(), StoreError> {
    if values
        .iter()
        .any(|value| value.trim().is_empty() || value.contains('\0'))
    {
        return Err(StoreError::Invalid(
            "required identifier is empty or invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_agent_profile(
    profile: &AgentProfileRecord,
    endpoints: &[AgentEndpointRecord],
) -> Result<(), StoreError> {
    validate_nonempty(&[
        &profile.agent_profile_id,
        &profile.provider_key,
        &profile.display_name,
    ])?;
    validate_timestamp(&profile.discovered_at)?;
    if profile.agent_profile_id.len() > 256
        || profile.provider_key.len() > 128
        || profile.display_name.len() > 256
        || endpoints.is_empty()
    {
        return Err(StoreError::Invalid(
            "AgentProfile identity or endpoint set is invalid".to_owned(),
        ));
    }
    let mut ids = std::collections::HashSet::new();
    for endpoint in endpoints {
        validate_nonempty(&[
            &endpoint.endpoint_id,
            &endpoint.agent_profile_id,
            &endpoint.protocol,
            &endpoint.topology,
        ])?;
        if endpoint.agent_profile_id != profile.agent_profile_id
            || !matches!(
                endpoint.protocol.as_str(),
                "ACP" | "A2A" | "SDK" | "API" | "CLI" | "TERMINAL"
            )
            || !matches!(
                endpoint.topology.as_str(),
                "LOCAL_INTERACTIVE" | "REMOTE_AGENT_SERVICE" | "VENDOR_SERVICE" | "PROCESS_ADAPTER"
            )
            || endpoint.endpoint_id.len() > 256
            || !endpoint.capabilities.is_object()
            || !ids.insert(endpoint.endpoint_id.as_str())
        {
            return Err(StoreError::Invalid(
                "AgentEndpoint identity or capabilities are invalid".to_owned(),
            ));
        }
        if endpoint
            .protocol_version
            .as_ref()
            .is_some_and(|value| value.len() > 128 || value.contains('\0'))
        {
            return Err(StoreError::Invalid(
                "AgentEndpoint protocol version is invalid".to_owned(),
            ));
        }
        canonical_json(&endpoint.capabilities)?;
    }
    Ok(())
}

fn validate_local_endpoint_binding(
    binding: &storage_core::LocalAgentEndpointBindingInput,
) -> Result<(), StoreError> {
    validate_nonempty(&[
        &binding.endpoint_id,
        &binding.runtime_id,
        &binding.runtime_incarnation_id,
        &binding.endpoint_ref,
    ])?;
    validate_timestamp(&binding.observed_at)?;
    if binding.endpoint_id.len() > 256
        || binding.runtime_id.len() > 256
        || binding.runtime_incarnation_id.len() > 256
        || binding.endpoint_ref.len() > 4096
        || binding.endpoint_ref.chars().any(char::is_control)
    {
        return Err(StoreError::Invalid(
            "local AgentEndpoint locator is invalid".to_owned(),
        ));
    }
    if let Some(expiry) = &binding.expires_at {
        validate_timestamp(expiry)?;
        if parse_timestamp(expiry)? <= parse_timestamp(&binding.observed_at)? {
            return Err(StoreError::Invalid(
                "local AgentEndpoint binding expiry must follow observation".to_owned(),
            ));
        }
    }
    // URL user-info is credential material. Runtime locators must refer to credentials
    // held by a separate broker, never embed username/password or bearer tokens.
    if binding.endpoint_ref.find("://").is_some_and(|scheme| {
        let authority = binding.endpoint_ref[scheme + 3..]
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("");
        authority.contains('@')
    }) {
        return Err(StoreError::Invalid(
            "endpoint URL must not embed credentials".to_owned(),
        ));
    }
    Ok(())
}

fn validate_runtime_offer(offer: &RuntimeOfferRecord) -> Result<(), StoreError> {
    validate_nonempty(&[
        &offer.runtime_id,
        &offer.runtime_incarnation_id,
        &offer.offer_kind,
        &offer.offer_ref,
        &offer.readiness,
    ])?;
    validate_timestamp(&offer.observed_at)?;
    validate_timestamp(&offer.expires_at)?;
    if offer.offer_kind != "AGENT_ENDPOINT"
        || !matches!(
            offer.readiness.as_str(),
            "AVAILABLE"
                | "STARTABLE"
                | "STARTING"
                | "READY"
                | "BUSY"
                | "DEGRADED"
                | "OFFLINE"
                | "NEEDS_AUTH"
                | "UNAVAILABLE"
        )
        || offer.offer_ref.len() > 256
        || !offer.constraints.is_object()
        || parse_timestamp(&offer.expires_at)? <= parse_timestamp(&offer.observed_at)?
    {
        return Err(StoreError::Invalid(
            "Agent RuntimeOffer is invalid".to_owned(),
        ));
    }
    canonical_json(&offer.constraints)?;
    Ok(())
}

fn validate_agent_binding_create(request: &AgentBindingCreateRequest) -> Result<(), StoreError> {
    let binding = &request.binding;
    validate_nonempty(&[
        &request.request.principal_id,
        &request.request.request_id,
        &binding.agent_binding_id,
        &binding.workspace_id,
        &binding.agent_profile_id,
    ])?;
    validate_timestamp(&request.now)?;
    validate_timestamp(&binding.created_at)?;
    if binding.enabled || binding.version != 1 || binding.configuration.as_object().is_none() {
        return Err(StoreError::Invalid(
            "new AgentBinding must be disabled at version 1 with object configuration".to_owned(),
        ));
    }
    // No adapter-specific, non-secret configuration schema is implemented yet.
    // Reject all keys until one exists instead of persisting arbitrary input that
    // could contain credential bytes in SQLite or aggregate-state blobs.
    if !binding
        .configuration
        .as_object()
        .is_some_and(|configuration| configuration.is_empty())
    {
        return Err(StoreError::Invalid("AgentBinding configuration is unavailable until an adapter-specific allowlist is implemented".to_owned()));
    }
    if request.request.principal_id.trim().is_empty() {
        return Err(StoreError::Invalid(
            "AgentBinding owner is required".to_owned(),
        ));
    }
    validate_endpoint_selection_policy(&binding.endpoint_selection_policy)?;
    if let Some(reference) = &binding.auth_ref {
        validate_secret_ref(reference)?;
    }
    let payload = agent_binding_created_payload(binding, &request.request.principal_id);
    validate_binding_event(
        &request.event,
        binding,
        1,
        "agent.binding.created.v1",
        &payload,
    )?;
    Ok(())
}

fn validate_agent_binding_enable(request: &AgentBindingEnableRequest) -> Result<(), StoreError> {
    validate_nonempty(&[
        &request.request.principal_id,
        &request.request.request_id,
        &request.workspace_id,
        &request.agent_binding_id,
    ])?;
    validate_timestamp(&request.now)?;
    canonical_json(&request.request.request_payload)?;
    if request.expected_version == u64::MAX
        || request.event.workspace_id != request.workspace_id
        || request.event.entity_type != "AgentBinding"
        || request.event.entity_id != request.agent_binding_id
        || request.event.entity_revision != request.expected_version + 1
        || request.event.event_type != "agent.binding.changed.v1"
    {
        return Err(StoreError::Invalid(
            "AgentBinding enable event does not match the requested transition".to_owned(),
        ));
    }
    Ok(())
}

fn validate_secret_ref(value: &Value) -> Result<(), StoreError> {
    let object = value.as_object().ok_or_else(|| {
        StoreError::Invalid("AgentBinding auth_ref must be a SecretRef object".to_owned())
    })?;
    let id = object
        .get("secret_ref_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let provider_ref = object
        .get("provider_ref")
        .and_then(Value::as_str)
        .unwrap_or("");
    let placement = object
        .get("placement")
        .and_then(Value::as_str)
        .unwrap_or("");
    if id.trim().is_empty()
        || provider_ref.trim().is_empty()
        || provider_ref.len() > 1024
        || id.len() > 256
        || id.contains('\0')
        || provider_ref.contains('\0')
        || !matches!(
            placement,
            "LOCAL_ONLY" | "CLOUD_AVAILABLE" | "RUNTIME_BOUND" | "EXTERNAL_AGENT_OWNED"
        )
        || object
            .keys()
            .any(|key| !matches!(key.as_str(), "secret_ref_id" | "provider_ref" | "placement"))
    {
        return Err(StoreError::Invalid(
            "AgentBinding SecretRef is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_binding_event(
    event: &EventDraft,
    binding: &AgentBindingRecord,
    revision: u64,
    event_type: &str,
    payload: &Value,
) -> Result<(), StoreError> {
    validate_nonempty(&[
        &event.event_id,
        &event.origin_runtime_id,
        &event.correlation_id,
    ])?;
    if event.workspace_id != binding.workspace_id
        || event.entity_type != "AgentBinding"
        || event.entity_id != binding.agent_binding_id
        || event.entity_revision != revision
        || event.event_type != event_type
        || event.payload != *payload
    {
        return Err(StoreError::Invalid(
            "AgentBinding event identity or payload is invalid".to_owned(),
        ));
    }
    validate_timestamp(&event.hlc_timestamp)?;
    validate_timestamp(&event.recorded_at)?;
    if event.schema_version == 0 {
        return Err(StoreError::Invalid(
            "AgentBinding event schema version is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn agent_binding_created_payload(binding: &AgentBindingRecord, principal_id: &str) -> Value {
    let mut payload = json!({
        "agent_binding_id": binding.agent_binding_id,
        "workspace_id": binding.workspace_id,
        "agent_profile_id": binding.agent_profile_id,
        "from_enabled": false,
        "to_enabled": false,
        "aggregate_version": binding.version,
        "requested_by": {"principal_id": principal_id, "kind": "USER"},
    });
    if let Some(runtime_id) = &binding.runtime_id {
        payload["runtime_id"] = Value::String(runtime_id.clone());
    }
    payload
}

fn agent_binding_enabled_payload(binding: &AgentBindingRecord, principal_id: &str) -> Value {
    let mut payload = json!({
        "agent_binding_id": binding.agent_binding_id,
        "workspace_id": binding.workspace_id,
        "agent_profile_id": binding.agent_profile_id,
        "from_enabled": false,
        "to_enabled": true,
        "aggregate_version": binding.version,
        "requested_by": {"principal_id": principal_id, "kind": "USER"},
    });
    if let Some(runtime_id) = &binding.runtime_id {
        payload["runtime_id"] = Value::String(runtime_id.clone());
    }
    payload
}

fn validate_endpoint_selection_policy(policy: &Value) -> Result<(), StoreError> {
    let object = policy.as_object().ok_or_else(|| {
        StoreError::Invalid("endpoint selection policy must be an object".to_owned())
    })?;
    let mode = object.get("mode").and_then(Value::as_str).unwrap_or("");
    let valid_mode = match mode {
        "AUTO_COMPATIBLE" => object.get("endpoint_id").is_none(),
        "PINNED_ENDPOINT" => object
            .get("endpoint_id")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty()),
        _ => false,
    };
    let valid_string_array = |key: &str| {
        object.get(key).is_some_and(|value| {
            value.as_array().is_some_and(|items| {
                items
                    .iter()
                    .all(|item| item.as_str().is_some_and(|text| !text.trim().is_empty()))
            })
        })
    };
    if !valid_mode
        || !valid_string_array("required_features")
        || !valid_string_array("preferred_topologies")
    {
        return Err(StoreError::Invalid(
            "endpoint selection policy is invalid".to_owned(),
        ));
    }
    let topologies = object
        .get("preferred_topologies")
        .and_then(Value::as_array)
        .expect("validated array");
    if topologies.iter().any(|item| {
        !matches!(
            item.as_str(),
            Some(
                "LOCAL_INTERACTIVE" | "REMOTE_AGENT_SERVICE" | "VENDOR_SERVICE" | "PROCESS_ADAPTER"
            )
        )
    }) {
        return Err(StoreError::Invalid(
            "endpoint selection policy contains an unknown topology".to_owned(),
        ));
    }
    Ok(())
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, StoreError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| StoreError::Invalid("timestamp is invalid".to_owned()))
}

fn put_agent_profile_transaction(
    connection: &mut Connection,
    profile: AgentProfileRecord,
    endpoints: Vec<AgentEndpointRecord>,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let stored: Option<(String, String, String)> = transaction.query_row(
        "SELECT provider_key, display_name, discovered_at FROM agent_profiles WHERE agent_profile_id = ?1",
        [&profile.agent_profile_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    match stored {
        // `discovered_at` is the first successful durable insertion time, not a
        // caller-controlled identity field. Concurrent first probes may propose
        // different observation times; preserve the row that won the transaction.
        Some((provider, name, _discovered))
            if provider == profile.provider_key && name == profile.display_name => {}
        Some(_) => {
            return Err(StoreError::Integrity(
                "stable AgentProfile identity cannot be rewritten".to_owned(),
            ));
        }
        None => {
            transaction.execute(
                "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at) VALUES (?1, ?2, ?3, ?4)",
                params![profile.agent_profile_id, profile.provider_key, profile.display_name, profile.discovered_at],
            ).map_err(map_database_error)?;
        }
    }
    for endpoint in endpoints {
        let capabilities = String::from_utf8(canonical_json(&endpoint.capabilities)?)
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
        let stored: Option<(String, String, String, Option<String>, String)> = transaction.query_row(
            "SELECT agent_profile_id, protocol, topology, protocol_version, capabilities_json FROM agent_endpoints WHERE endpoint_id = ?1",
            [&endpoint.endpoint_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        ).optional().map_err(map_database_error)?;
        // Query tuple order is normalized below; this explicit branch keeps stable
        // endpoint identity immutable across discovery refreshes.
        match stored {
            Some((profile_id, protocol, topology, protocol_version, old_capabilities))
                if profile_id == endpoint.agent_profile_id
                    && protocol == endpoint.protocol
                    && topology == endpoint.topology
                    && protocol_version == endpoint.protocol_version
                    && old_capabilities == capabilities => {}
            Some(_) => {
                return Err(StoreError::Integrity(
                    "stable AgentEndpoint identity cannot be rewritten".to_owned(),
                ));
            }
            None => {
                transaction.execute(
                    "INSERT INTO agent_endpoints(endpoint_id, agent_profile_id, protocol, topology, protocol_version, capabilities_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![endpoint.endpoint_id, endpoint.agent_profile_id, endpoint.protocol, endpoint.topology, endpoint.protocol_version, capabilities],
                ).map_err(map_database_error)?;
            }
        }
    }
    transaction.commit().map_err(map_database_error)
}

fn register_local_endpoint_binding_transaction(
    connection: &mut Connection,
    binding: storage_core::LocalAgentEndpointBindingInput,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let current: Option<String> = transaction
        .query_row(
            "SELECT current_incarnation_id FROM runtimes WHERE runtime_id = ?1",
            [&binding.runtime_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?
        .flatten();
    if current.as_deref() != Some(binding.runtime_incarnation_id.as_str()) {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let endpoint_exists: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_endpoints WHERE endpoint_id = ?1)",
            [&binding.endpoint_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !endpoint_exists {
        return Err(StoreError::NotFound);
    }
    transaction.execute(
        "INSERT INTO agent_endpoint_bindings(endpoint_id, runtime_id, runtime_incarnation_id, endpoint_ref, observed_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(endpoint_id, runtime_incarnation_id) DO UPDATE SET endpoint_ref = excluded.endpoint_ref, observed_at = excluded.observed_at, expires_at = excluded.expires_at WHERE agent_endpoint_bindings.runtime_id = excluded.runtime_id",
        params![binding.endpoint_id, binding.runtime_id, binding.runtime_incarnation_id, binding.endpoint_ref, binding.observed_at, binding.expires_at],
    ).map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)
}

fn get_local_endpoint_binding(
    connection: &Connection,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    endpoint_id: &str,
    now: &str,
) -> Result<Option<LocalAgentEndpointBindingRecord>, StoreError> {
    connection.query_row(
        "SELECT b.endpoint_ref, b.observed_at, b.expires_at FROM agent_endpoint_bindings b JOIN runtimes r ON r.runtime_id = b.runtime_id WHERE b.runtime_id = ?1 AND b.runtime_incarnation_id = ?2 AND b.endpoint_id = ?3 AND r.current_incarnation_id = b.runtime_incarnation_id AND julianday(b.observed_at) <= julianday(?4) AND (b.expires_at IS NULL OR julianday(b.expires_at) > julianday(?4))",
        params![runtime_id, runtime_incarnation_id, endpoint_id, now],
        |row| Ok(LocalAgentEndpointBindingRecord {
            endpoint_id: endpoint_id.to_owned(), runtime_id: runtime_id.to_owned(), runtime_incarnation_id: runtime_incarnation_id.to_owned(),
            endpoint_ref: row.get(0)?, observed_at: row.get(1)?, expires_at: row.get(2)?,
        }),
    ).optional().map_err(map_database_error)
}

fn publish_runtime_offer_transaction(
    connection: &mut Connection,
    offer: RuntimeOfferRecord,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let current: Option<String> = transaction
        .query_row(
            "SELECT current_incarnation_id FROM runtimes WHERE runtime_id = ?1",
            [&offer.runtime_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?
        .flatten();
    if current.as_deref() != Some(offer.runtime_incarnation_id.as_str()) {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let endpoint_bound: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM agent_endpoint_bindings b WHERE b.endpoint_id = ?1 AND b.runtime_id = ?2 AND b.runtime_incarnation_id = ?3 AND (b.expires_at IS NULL OR julianday(b.expires_at) > julianday(?4)))",
        params![offer.offer_ref, offer.runtime_id, offer.runtime_incarnation_id, offer.observed_at], |row| row.get(0),
    ).map_err(map_database_error)?;
    if !endpoint_bound {
        return Err(StoreError::Invalid(
            "RuntimeOffer requires a current local endpoint binding".to_owned(),
        ));
    }
    let constraints = String::from_utf8(canonical_json(&offer.constraints)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    transaction.execute(
        "INSERT INTO runtime_offers(runtime_id, runtime_incarnation_id, offer_kind, offer_ref, compatible, readiness, constraints_json, observed_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) ON CONFLICT(runtime_id, runtime_incarnation_id, offer_kind, offer_ref) DO UPDATE SET compatible = excluded.compatible, readiness = excluded.readiness, constraints_json = excluded.constraints_json, observed_at = excluded.observed_at, expires_at = excluded.expires_at",
        params![offer.runtime_id, offer.runtime_incarnation_id, offer.offer_kind, offer.offer_ref, if offer.compatible { 1_i64 } else { 0_i64 }, offer.readiness, constraints, offer.observed_at, offer.expires_at],
    ).map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)
}

fn workspace_owner_active(
    connection: &Connection,
    owner: &str,
    workspace_id: &str,
) -> Result<(), StoreError> {
    let valid: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2 AND status = 'ACTIVE')",
        params![workspace_id, owner], |row| row.get(0),
    ).map_err(map_database_error)?;
    if valid {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

fn list_agent_profiles(
    connection: &Connection,
    owner: &str,
    workspace_id: &str,
    now: &str,
) -> Result<Vec<AgentProfileViewRecord>, StoreError> {
    workspace_owner_active(connection, owner, workspace_id)?;
    let mut statement = connection.prepare("SELECT agent_profile_id, provider_key, display_name, discovered_at FROM agent_profiles ORDER BY display_name COLLATE NOCASE, agent_profile_id")
        .map_err(map_database_error)?;
    let profiles = statement
        .query_map([], |row| {
            Ok(AgentProfileRecord {
                agent_profile_id: row.get(0)?,
                provider_key: row.get(1)?,
                display_name: row.get(2)?,
                discovered_at: row.get(3)?,
            })
        })
        .map_err(map_database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)?;
    let mut result = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let mut endpoint_statement = connection.prepare("SELECT endpoint_id, agent_profile_id, protocol, topology, protocol_version, capabilities_json FROM agent_endpoints WHERE agent_profile_id = ?1 ORDER BY endpoint_id")
            .map_err(map_database_error)?;
        let endpoint_rows = endpoint_statement
            .query_map([&profile.agent_profile_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(map_database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?;
        let mut endpoints = Vec::with_capacity(endpoint_rows.len());
        for (
            endpoint_id,
            agent_profile_id,
            protocol,
            topology,
            protocol_version,
            capabilities_json,
        ) in endpoint_rows
        {
            let endpoint = AgentEndpointRecord {
                endpoint_id: endpoint_id.clone(),
                agent_profile_id,
                protocol,
                topology,
                protocol_version,
                capabilities: serde_json::from_str(&capabilities_json)
                    .map_err(|error| StoreError::Integrity(error.to_string()))?,
            };
            let mut offer_statement = connection.prepare("SELECT o.runtime_id, o.runtime_incarnation_id, o.compatible, o.readiness, o.constraints_json, o.observed_at, o.expires_at FROM runtime_offers o JOIN runtimes r ON r.runtime_id = o.runtime_id AND r.current_incarnation_id = o.runtime_incarnation_id AND r.availability IN ('ONLINE', 'DEGRADED') JOIN runtime_incarnations i ON i.runtime_id = o.runtime_id AND i.runtime_incarnation_id = o.runtime_incarnation_id AND i.recovery_state IN ('READY', 'DEGRADED') JOIN runtime_workspace_bindings rwb ON rwb.runtime_id = o.runtime_id AND rwb.workspace_id = ?1 AND rwb.status = 'ACTIVE' AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'EXECUTOR') JOIN agent_endpoint_bindings eb ON eb.endpoint_id = o.offer_ref AND eb.runtime_id = o.runtime_id AND eb.runtime_incarnation_id = o.runtime_incarnation_id WHERE o.offer_kind = 'AGENT_ENDPOINT' AND o.offer_ref = ?2 AND julianday(o.observed_at) <= julianday(?3) AND julianday(o.expires_at) > julianday(?3) AND (eb.expires_at IS NULL OR julianday(eb.expires_at) > julianday(?3)) ORDER BY o.runtime_id")
                .map_err(map_database_error)?;
            let offers = offer_statement
                .query_map(params![workspace_id, endpoint_id, now], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)? != 0,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                })
                .map_err(map_database_error)?
                .map(|row| {
                    let (
                        runtime_id,
                        runtime_incarnation_id,
                        compatible,
                        readiness,
                        constraints_json,
                        observed_at,
                        expires_at,
                    ) = row.map_err(map_database_error)?;
                    Ok(RuntimeOfferRecord {
                        runtime_id,
                        runtime_incarnation_id,
                        offer_kind: "AGENT_ENDPOINT".to_owned(),
                        offer_ref: endpoint_id.clone(),
                        compatible,
                        readiness,
                        constraints: serde_json::from_str(&constraints_json)
                            .map_err(|error| StoreError::Integrity(error.to_string()))?,
                        observed_at,
                        expires_at,
                    })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            endpoints.push(AgentEndpointViewRecord { endpoint, offers });
        }
        result.push(AgentProfileViewRecord { profile, endpoints });
    }
    Ok(result)
}

fn endpoint_supports_features(endpoint: &AgentEndpointRecord, policy: &Value) -> bool {
    let required = policy
        .get("required_features")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    required.iter().all(|feature| {
        let Some(feature) = feature.as_str() else {
            return false;
        };
        let mut current = &endpoint.capabilities;
        for part in feature.split('.') {
            let Some(next) = current.get(part) else {
                return false;
            };
            current = next;
        }
        current.as_bool() == Some(true)
    })
}

fn has_fresh_compatible_endpoint(
    connection: &Connection,
    binding: &AgentBindingRecord,
    now: &str,
) -> Result<bool, StoreError> {
    let policy = &binding.endpoint_selection_policy;
    let pinned_id = policy.get("endpoint_id").and_then(Value::as_str);
    let preferred = policy
        .get("preferred_topologies")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut statement = connection.prepare("SELECT e.endpoint_id, e.agent_profile_id, e.protocol, e.topology, e.protocol_version, e.capabilities_json, o.observed_at, o.expires_at FROM agent_endpoints e JOIN runtime_offers o ON o.offer_kind = 'AGENT_ENDPOINT' AND o.offer_ref = e.endpoint_id AND o.compatible = 1 JOIN runtimes r ON r.runtime_id = o.runtime_id AND r.current_incarnation_id = o.runtime_incarnation_id AND r.availability IN ('ONLINE', 'DEGRADED') JOIN runtime_incarnations i ON i.runtime_id = o.runtime_id AND i.runtime_incarnation_id = o.runtime_incarnation_id AND i.recovery_state IN ('READY', 'DEGRADED') JOIN runtime_workspace_bindings rwb ON rwb.runtime_id = o.runtime_id AND rwb.workspace_id = ?1 AND rwb.status = 'ACTIVE' AND EXISTS (SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'EXECUTOR') JOIN agent_endpoint_bindings eb ON eb.endpoint_id = e.endpoint_id AND eb.runtime_id = o.runtime_id AND eb.runtime_incarnation_id = o.runtime_incarnation_id WHERE e.agent_profile_id = ?2 AND (?3 IS NULL OR e.endpoint_id = ?3) AND (?4 IS NULL OR o.runtime_id = ?4) AND o.readiness IN ('AVAILABLE', 'STARTABLE', 'READY') AND julianday(o.observed_at) <= julianday(?5) AND julianday(o.expires_at) > julianday(?5) AND (eb.expires_at IS NULL OR julianday(eb.expires_at) > julianday(?5)) ORDER BY o.readiness = 'READY' DESC, o.observed_at DESC")
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![
                binding.workspace_id,
                binding.agent_profile_id,
                pinned_id,
                binding.runtime_id,
                now
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .map_err(map_database_error)?;
    for row in rows {
        let (
            endpoint_id,
            agent_profile_id,
            protocol,
            topology,
            protocol_version,
            capabilities_json,
            observed_at,
            expires_at,
        ) = row.map_err(map_database_error)?;
        if parse_timestamp(&expires_at)? <= parse_timestamp(now)?
            || parse_timestamp(&observed_at)? > parse_timestamp(now)?
        {
            continue;
        }
        if !preferred.is_empty()
            && !preferred
                .iter()
                .any(|item| item.as_str() == Some(topology.as_str()))
        {
            continue;
        }
        let capabilities: Value = serde_json::from_str(&capabilities_json)
            .map_err(|error| StoreError::Integrity(error.to_string()))?;
        let endpoint = AgentEndpointRecord {
            endpoint_id,
            agent_profile_id,
            protocol,
            topology,
            protocol_version,
            capabilities,
        };
        if endpoint_supports_features(&endpoint, policy) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_agent_binding(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentBindingRecord> {
    let policy: String = row.get(4)?;
    let configuration: String = row.get(6)?;
    Ok(AgentBindingRecord {
        agent_binding_id: row.get(0)?,
        workspace_id: row.get(1)?,
        agent_profile_id: row.get(2)?,
        runtime_id: row.get(3)?,
        endpoint_selection_policy: serde_json::from_str(&policy)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        auth_ref: row
            .get::<_, Option<String>>(5)?
            .map(|value| serde_json::from_str(&value).map_err(|_| rusqlite::Error::InvalidQuery))
            .transpose()?,
        configuration: serde_json::from_str(&configuration)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        enabled: row.get::<_, i64>(7)? != 0,
        lead_eligible: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
        version: row.get::<_, i64>(10)? as u64,
    })
}

const AGENT_BINDING_COLUMNS: &str = "agent_binding_id, workspace_id, agent_profile_id, runtime_id, endpoint_selection_policy_json, auth_ref, configuration_json, enabled, lead_eligible, created_at, version";

fn get_agent_binding(
    connection: &Connection,
    owner: &str,
    workspace_id: &str,
    binding_id: &str,
) -> Result<Option<AgentBindingRecord>, StoreError> {
    workspace_owner_active(connection, owner, workspace_id)?;
    let sql = format!(
        "SELECT {AGENT_BINDING_COLUMNS} FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2"
    );
    connection
        .query_row(&sql, params![workspace_id, binding_id], parse_agent_binding)
        .optional()
        .map_err(map_database_error)
}

fn list_agent_bindings(
    connection: &Connection,
    owner: &str,
    workspace_id: &str,
) -> Result<Vec<AgentBindingRecord>, StoreError> {
    workspace_owner_active(connection, owner, workspace_id)?;
    let sql = format!(
        "SELECT {AGENT_BINDING_COLUMNS} FROM agent_bindings WHERE workspace_id = ?1 ORDER BY created_at, agent_binding_id"
    );
    let mut statement = connection.prepare(&sql).map_err(map_database_error)?;
    statement
        .query_map([workspace_id], parse_agent_binding)
        .map_err(map_database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)
}

fn verify_request_receipt<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    principal: &str,
    request_id: &str,
    request_digest: &str,
) -> Result<Option<T>, StoreError> {
    let prior: Option<(String, Option<String>, Option<String>)> = connection.query_row(
        "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
        params![principal, request_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(map_database_error)?;
    let Some((prior_digest, response_json, response_digest)) = prior else {
        return Ok(None);
    };
    if prior_digest != request_digest {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let response_json = response_json.ok_or_else(|| {
        StoreError::Integrity("idempotency receipt has no committed response".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "idempotency response digest does not match".to_owned(),
        ));
    }
    serde_json::from_str(&response_json)
        .map(Some)
        .map_err(|error| StoreError::Integrity(error.to_string()))
}

fn load_current_resource_summary(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
) -> Result<Option<ResourceSummary>, StoreError> {
    connection.query_row(
        "SELECT r.resource_id, r.workspace_id, rev.resource_revision_id, r.display_name,
                rev.media_type, rev.content_digest, rev.size_bytes, r.created_at,
                r.context_document_json
         FROM resources r
         JOIN resource_revisions rev
           ON rev.resource_id = r.resource_id AND rev.resource_revision_id = r.current_revision_id
         WHERE r.workspace_id = ?1 AND r.resource_id = ?2
           AND rev.media_type IS NOT NULL AND rev.content_digest IS NOT NULL AND rev.size_bytes IS NOT NULL",
        params![workspace_id, resource_id],
        |row| {
            let size: i64 = row.get(6)?;
            Ok((
                ResourceSummary {
                    resource_id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    resource_revision_id: row.get(2)?,
                    display_name: row.get(3)?,
                    media_type: row.get(4)?,
                    content_digest: row.get(5)?,
                    size_bytes: u64::try_from(size)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, size))?,
                    created_at: row.get(7)?,
                },
                row.get::<_, Option<String>>(8)?,
            ))
        },
    ).optional().map_err(map_database_error)
        .and_then(|row| {
            let Some((summary, context_document)) = row else {
                return Ok(None);
            };
            ensure_context_document_content_readable(context_document.as_deref())?;
            Ok(Some(summary))
        })
}

fn commit_resource_text_index_rebuild_transaction(
    connection: &mut Connection,
    request: ResourceTextIndexRebuildRequest,
    request_digest: String,
    result: ResourceTextIndexRebuildResult,
    index: Option<PreparedResourceTextIndex>,
) -> Result<ResourceTextIndexRebuildResult, StoreError> {
    if result.workspace_id != request.workspace_id
        || result.request_id != request.request_id
        || result.correlation_id != request.correlation_id
        || result.resource_id != request.resource_id
        || result.resource_revision_id != request.resource_revision_id
        || result.content_digest != request.content_digest
        || (result.outcome == ResourceTextIndexRebuildOutcome::Indexed) != index.is_some()
        || (result.outcome == ResourceTextIndexRebuildOutcome::Indexed && result.reason.is_some())
        || (result.outcome == ResourceTextIndexRebuildOutcome::NotIndexable
            && result.reason.is_none())
    {
        return Err(StoreError::Invalid(
            "Resource text-index rebuild result does not match its request".to_owned(),
        ));
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    if let Some(replay) = verify_request_receipt::<ResourceTextIndexRebuildResult>(
        &tx,
        &request.principal_id,
        &request.request_id,
        &request_digest,
    )? {
        return Ok(replay);
    }
    workspace_owner_active(&tx, &request.principal_id, &request.workspace_id)?;
    let current: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM resources r JOIN resource_revisions rev
           ON rev.resource_id = r.resource_id AND rev.resource_revision_id = r.current_revision_id
           WHERE r.workspace_id = ?1 AND r.resource_id = ?2
             AND rev.resource_revision_id = ?3 AND rev.content_digest = ?4
             AND (r.context_document_json IS NULL OR json_extract(r.context_document_json, '$.status') = 'ACTIVE'))",
        params![request.workspace_id, request.resource_id, request.resource_revision_id, request.content_digest],
        |row| row.get(0),
    ).map_err(map_database_error)?;
    if !current {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let committed = resource_index::replace_current_in_transaction(
        &tx,
        &request.workspace_id,
        &request.resource_id,
        &request.resource_revision_id,
        &request.content_digest,
        index.as_ref(),
    )?;
    if !committed {
        return Err(StoreError::Conflict {
            expected: None,
            actual: None,
        });
    }
    let response = String::from_utf8(canonical_json(&result)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![request.principal_id, request.request_id, request_digest, response, digest(response.as_bytes()), request.indexed_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(result)
}

fn set_context_document_status_transaction(
    connection: &mut Connection,
    blobs: &dyn BlobStore,
    command: ContextDocumentStatusCommand,
) -> Result<CommittedContextDocumentStatus, StoreError> {
    let request_payload = json!({
        "workspace_id": &command.workspace_id,
        "resource_id": &command.resource_id,
        "status": context_document_owner_status_str(command.target_status),
        "expected_version": command.expected_version,
    });
    if command.request_payload != request_payload {
        return Err(StoreError::Invalid(
            "ContextDocument status request payload is inconsistent".to_owned(),
        ));
    }
    let request_digest = digest(&canonical_json(&command.request_payload)?);
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    if let Some(replay) = verify_request_receipt::<CommittedContextDocumentStatus>(
        &tx,
        &command.principal_id,
        &command.request_id,
        &request_digest,
    )? {
        if replay.resource.resource.workspace_id != command.workspace_id
            || replay.resource.resource.resource_id != command.resource_id
            || replay.event.entity_id != command.resource_id
            || replay.event.entity_type != "Resource"
        {
            return Err(StoreError::Integrity(
                "ContextDocument status receipt identity does not match its request".to_owned(),
            ));
        }
        return Ok(replay);
    }

    let workspace_status: Option<String> = tx
        .query_row(
            "SELECT status FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2",
            params![command.workspace_id, command.principal_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    match workspace_status.as_deref() {
        Some("ACTIVE") => {}
        Some(_) => {
            return Err(StoreError::Invalid(
                "an archived Workspace is read-only".to_owned(),
            ));
        }
        None => return Err(StoreError::NotFound),
    }
    let mut resource =
        load_resource_detail_record(&tx, &command.workspace_id, &command.resource_id)?
            .ok_or(StoreError::NotFound)?;
    let mut metadata = resource
        .context_document
        .take()
        .ok_or(StoreError::NotFound)?;
    if resource.resource.version != command.expected_version {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(resource.resource.version),
        });
    }
    let metadata_object = metadata.as_object_mut().ok_or_else(|| {
        StoreError::Integrity("ContextDocument metadata is not an object".to_owned())
    })?;
    let current_status = metadata_object
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::Integrity("ContextDocument status is missing".to_owned()))?;
    let target_status = context_document_owner_status_str(command.target_status);
    if !context_document_owner_transition_allowed(current_status, command.target_status) {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: Some(resource.resource.version),
        });
    }
    if command.event.recorded_at <= resource.resource.updated_at {
        return Err(StoreError::Invalid(
            "ContextDocument status timestamp must advance the Resource".to_owned(),
        ));
    }
    if metadata_object
        .get("purge_manifest_digest")
        .is_some_and(|value| !value.is_null())
        || metadata_object
            .get("purge_target_count")
            .is_some_and(|value| !value.is_null())
    {
        return Err(StoreError::Integrity(
            "owner status transition cannot modify purge state".to_owned(),
        ));
    }
    let from_status = current_status.to_owned();
    metadata_object.insert("status".to_owned(), Value::String(target_status.to_owned()));
    resource.resource.version = resource
        .resource
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("Resource version overflow".to_owned()))?;
    resource.resource.updated_at = command.event.recorded_at.clone();
    resource.context_document = Some(metadata);

    let payload = json!({
        "resource_id": command.resource_id,
        "from": from_status,
        "to": target_status,
        "changed_by": {"kind": "USER", "principal_id": &command.principal_id},
        "aggregate_version": resource.resource.version,
    });
    let recorded_at = command.event.recorded_at.clone();
    let event_draft = EventDraft {
        event_id: command.event.event_id,
        workspace_id: command.workspace_id.clone(),
        entity_type: "Resource".to_owned(),
        entity_id: command.resource_id.clone(),
        origin_runtime_id: command.event.origin_runtime_id,
        entity_revision: resource.resource.version,
        hlc_timestamp: command.event.hlc_timestamp,
        correlation_id: command.event.correlation_id,
        causation_id: command.event.causation_id,
        schema_version: 1,
        event_type: "resource.context_document.status.changed.v1".to_owned(),
        payload,
        recorded_at: recorded_at.clone(),
    };
    let state_bytes = canonical_json(&resource)?;
    let state_blob = blobs.put(
        &command.workspace_id,
        BlobPurpose::AggregateState,
        &state_bytes,
        "application/vnd.litecowork.resource+json",
    )?;
    if blobs.get(
        &command.workspace_id,
        BlobPurpose::AggregateState,
        &state_blob,
    )? != state_bytes
    {
        return Err(StoreError::Integrity(
            "ContextDocument status snapshot failed verification".to_owned(),
        ));
    }
    let state_ref = AggregateStateRef {
        blob: state_blob,
        entity_revision: resource.resource.version,
        record_schema_version: 1,
    };
    let metadata_json = String::from_utf8(canonical_json(&resource.context_document)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let changed = tx
        .execute(
            "UPDATE resources SET context_document_json = ?1, updated_at = ?2, version = ?3
         WHERE workspace_id = ?4 AND resource_id = ?5 AND version = ?6",
            params![
                metadata_json,
                resource.resource.updated_at,
                to_sql_i64(resource.resource.version, "Resource version")?,
                command.workspace_id,
                command.resource_id,
                to_sql_i64(command.expected_version, "expected Resource version")?
            ],
        )
        .map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(command.expected_version),
            actual: None,
        });
    }
    let event = insert_domain_event(&tx, &event_draft, &state_ref)?;
    let committed = CommittedContextDocumentStatus { resource, event };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute(
        "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![command.principal_id, command.request_id, request_digest, response_json, digest(response_json.as_bytes()), recorded_at],
    ).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn context_document_owner_transition_allowed(
    current: &str,
    target: ContextDocumentOwnerStatus,
) -> bool {
    matches!(
        (current, target),
        ("ACTIVE", ContextDocumentOwnerStatus::Revoked)
            | ("REVOKED", ContextDocumentOwnerStatus::Active)
    )
}

fn context_document_owner_status_str(status: ContextDocumentOwnerStatus) -> &'static str {
    match status {
        ContextDocumentOwnerStatus::Active => "ACTIVE",
        ContextDocumentOwnerStatus::Revoked => "REVOKED",
    }
}

fn create_agent_binding_transaction(
    connection: &mut Connection,
    request: AgentBindingCreateRequest,
    state_ref: AggregateStateRef,
) -> Result<CommittedAgentBinding, StoreError> {
    if state_ref.entity_revision != 1 || state_ref.record_schema_version != 1 {
        return Err(StoreError::Invalid(
            "AgentBinding state reference is invalid".to_owned(),
        ));
    }
    let request_digest = digest(&canonical_json(&request.request.request_payload)?);
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    if let Some(replay) = verify_request_receipt::<CommittedAgentBinding>(
        &tx,
        &request.request.principal_id,
        &request.request.request_id,
        &request_digest,
    )? {
        return Ok(replay);
    }
    workspace_owner_active(
        &tx,
        &request.request.principal_id,
        &request.binding.workspace_id,
    )?;
    let profile_exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_profiles WHERE agent_profile_id = ?1)",
            [&request.binding.agent_profile_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !profile_exists {
        return Err(StoreError::NotFound);
    }
    if !has_fresh_compatible_endpoint(&tx, &request.binding, &request.now)? {
        return Err(StoreError::Invalid(
            "no fresh compatible endpoint offer is available for this AgentBinding".to_owned(),
        ));
    }
    let policy_json =
        String::from_utf8(canonical_json(&request.binding.endpoint_selection_policy)?)
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let configuration_json = String::from_utf8(canonical_json(&request.binding.configuration)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let auth_ref_json = request
        .binding
        .auth_ref
        .as_ref()
        .map(canonical_json)
        .transpose()?
        .map(String::from_utf8)
        .transpose()
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute("INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, runtime_id, endpoint_selection_policy_json, auth_ref, configuration_json, enabled, lead_eligible, created_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, 1)", params![request.binding.agent_binding_id, request.binding.workspace_id, request.binding.agent_profile_id, request.binding.runtime_id, policy_json, auth_ref_json, configuration_json, if request.binding.lead_eligible { 1_i64 } else { 0_i64 }, request.binding.created_at]).map_err(map_database_error)?;
    let event = insert_domain_event(&tx, request.event, state_ref)?;
    let committed = CommittedAgentBinding {
        binding: request.binding,
        event,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute("INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)", params![request.request.principal_id, request.request.request_id, request_digest, response_json, digest(response_json.as_bytes()), request.now]).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn enable_agent_binding_transaction(
    connection: &mut Connection,
    request: AgentBindingEnableRequest,
    state_ref: AggregateStateRef,
) -> Result<CommittedAgentBinding, StoreError> {
    let request_digest = digest(&canonical_json(&request.request.request_payload)?);
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    if let Some(replay) = verify_request_receipt::<CommittedAgentBinding>(
        &tx,
        &request.request.principal_id,
        &request.request.request_id,
        &request_digest,
    )? {
        return Ok(replay);
    }
    workspace_owner_active(&tx, &request.request.principal_id, &request.workspace_id)?;
    let sql = format!(
        "SELECT {AGENT_BINDING_COLUMNS} FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2"
    );
    let current = tx
        .query_row(
            &sql,
            params![request.workspace_id, request.agent_binding_id],
            parse_agent_binding,
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;
    if current.version != request.expected_version {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_version),
            actual: Some(current.version),
        });
    }
    if current.enabled {
        return Err(StoreError::Invalid(
            "AgentBinding is already enabled".to_owned(),
        ));
    }
    let mut next = current.clone();
    next.enabled = true;
    next.version = current
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Integrity("AgentBinding version exhausted".to_owned()))?;
    if state_ref.entity_revision != next.version || state_ref.record_schema_version != 1 {
        return Err(StoreError::Invalid(
            "AgentBinding state reference is invalid".to_owned(),
        ));
    }
    if !has_fresh_compatible_endpoint(&tx, &current, &request.now)? {
        return Err(StoreError::Invalid(
            "no fresh compatible endpoint offer is available to enable this AgentBinding"
                .to_owned(),
        ));
    }
    let payload = agent_binding_enabled_payload(&next, &request.request.principal_id);
    validate_binding_event(
        &request.event,
        &next,
        next.version,
        "agent.binding.changed.v1",
        &payload,
    )?;
    let policy_json = String::from_utf8(canonical_json(&next.endpoint_selection_policy)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let config_json = String::from_utf8(canonical_json(&next.configuration)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let auth_ref_json = next
        .auth_ref
        .as_ref()
        .map(canonical_json)
        .transpose()?
        .map(String::from_utf8)
        .transpose()
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let updated = tx.execute("UPDATE agent_bindings SET endpoint_selection_policy_json = ?1, auth_ref = ?2, configuration_json = ?3, enabled = 1, lead_eligible = ?4, created_at = ?5, version = ?6 WHERE workspace_id = ?7 AND agent_binding_id = ?8 AND version = ?9 AND enabled = 0", params![policy_json, auth_ref_json, config_json, if next.lead_eligible { 1_i64 } else { 0_i64 }, next.created_at, to_sql_i64(next.version, "AgentBinding version")?, next.workspace_id, next.agent_binding_id, to_sql_i64(request.expected_version, "expected AgentBinding version")?]).map_err(map_database_error)?;
    if updated != 1 {
        return Err(StoreError::Conflict {
            expected: Some(request.expected_version),
            actual: None,
        });
    }
    let event = insert_domain_event(&tx, request.event, state_ref)?;
    let committed = CommittedAgentBinding {
        binding: next,
        event,
    };
    let response_json = String::from_utf8(canonical_json(&committed)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    tx.execute("INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)", params![request.request.principal_id, request.request.request_id, request_digest, response_json, digest(response_json.as_bytes()), request.now]).map_err(map_database_error)?;
    tx.commit().map_err(map_database_error)?;
    Ok(committed)
}

fn register_local_runtime_incarnation_transaction(
    connection: &mut Connection,
    runtime: RuntimeRecord,
    incarnation: RuntimeIncarnationRecord,
    observation: RuntimeIncarnationLocalObservationRecord,
) -> Result<RuntimeIncarnationRecord, StoreError> {
    validate_local_runtime_registration(&runtime, &incarnation, &observation)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let existing_identity = transaction
        .query_row(
            "SELECT device_identity_json, availability FROM runtimes WHERE runtime_id = ?1",
            [&runtime.runtime_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    match existing_identity {
        Some((value, availability)) => {
            if availability == "REVOKED" {
                return Err(StoreError::Integrity(
                    "a revoked Runtime identity cannot be reactivated by local startup".to_owned(),
                ));
            }
            let existing: DeviceIdentityRecord = serde_json::from_str(&value).map_err(|_| {
                StoreError::Integrity("stored Runtime device identity is malformed".to_owned())
            })?;
            if existing != runtime.device_identity {
                return Err(StoreError::Integrity(
                    "bootstrap Runtime identity does not match the durable Runtime identity"
                        .to_owned(),
                ));
            }
            let conflict: Option<String> = transaction
                .query_row(
                    "SELECT runtime_id FROM runtimes WHERE runtime_id <> ?1 AND json_extract(device_identity_json, '$.device_id') = ?2 LIMIT 1",
                    params![runtime.runtime_id, runtime.device_identity.device_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            if conflict.is_some() {
                return Err(StoreError::Integrity(
                    "device identity is already bound to another Runtime".to_owned(),
                ));
            }
        }
        None => {
            let conflict: Option<String> = transaction
                .query_row(
                    "SELECT runtime_id FROM runtimes WHERE json_extract(device_identity_json, '$.device_id') = ?1 LIMIT 1",
                    [&runtime.device_identity.device_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            if conflict.is_some() {
                return Err(StoreError::Integrity(
                    "device identity is already bound to another Runtime".to_owned(),
                ));
            }
        }
    }

    let identity_json = String::from_utf8(canonical_json(&runtime.device_identity)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let roles_json = String::from_utf8(canonical_json(&runtime.roles)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let capacity_json = String::from_utf8(canonical_json(&runtime.resource_capacity)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    transaction.execute(
        "INSERT INTO runtimes(runtime_id, device_identity_json, runtime_version, platform, architecture, roles_json, trust_zone, availability, startup_policy, current_incarnation_id, resource_capacity_json, last_seen, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'RECOVERING', ?8, ?9, ?10, ?11, 1) ON CONFLICT(runtime_id) DO UPDATE SET runtime_version = excluded.runtime_version, platform = excluded.platform, architecture = excluded.architecture, roles_json = excluded.roles_json, trust_zone = excluded.trust_zone, availability = excluded.availability, startup_policy = excluded.startup_policy, current_incarnation_id = excluded.current_incarnation_id, resource_capacity_json = excluded.resource_capacity_json, last_seen = excluded.last_seen, version = runtimes.version + 1",
        params![runtime.runtime_id, identity_json, runtime.runtime_version, runtime.platform, runtime.architecture, roles_json, runtime.trust_zone, runtime.startup_policy, runtime.current_incarnation_id, capacity_json, runtime.last_seen],
    ).map_err(map_database_error)?;
    transaction.execute(
        "INSERT INTO runtime_incarnations(runtime_incarnation_id, runtime_id, process_started_at, litecowork_version, recovered_from_unclean_shutdown, recovery_state, ready_at, stopped_at, version) VALUES (?1, ?2, ?3, ?4, ?5, 'RECOVERING', NULL, NULL, 1)",
        params![incarnation.runtime_incarnation_id, incarnation.runtime_id, incarnation.process_started_at, incarnation.litecowork_version, if incarnation.recovered_from_unclean_shutdown { 1_i64 } else { 0_i64 }],
    ).map_err(map_database_error)?;
    transaction.execute(
        "INSERT INTO runtime_incarnation_local_observations(runtime_incarnation_id, os_boot_id, observed_at, diagnostic_ref_json) VALUES (?1, ?2, ?3, NULL)",
        params![observation.runtime_incarnation_id, observation.os_boot_id, observation.observed_at],
    ).map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(incarnation)
}

fn transition_local_runtime_incarnation_transaction(
    connection: &mut Connection,
    update: RuntimeIncarnationStateUpdate,
) -> Result<RuntimeIncarnationRecord, StoreError> {
    validate_runtime_state_update(&update)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    let row = transaction
        .query_row(
            "SELECT i.process_started_at, i.litecowork_version, i.recovered_from_unclean_shutdown, i.recovery_state, i.ready_at, i.stopped_at, i.version, r.current_incarnation_id FROM runtime_incarnations i JOIN runtimes r ON r.runtime_id = i.runtime_id WHERE i.runtime_id = ?1 AND i.runtime_incarnation_id = ?2",
            params![update.runtime_id, update.runtime_incarnation_id],
            |row| Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?, row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?, row.get::<_, i64>(6)?, row.get::<_, String>(7)?,
            )),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;
    let (
        started_at,
        version,
        recovered,
        current_state,
        ready_at,
        old_stopped_at,
        actual_version,
        current_id,
    ) = row;
    let actual_version = u64::try_from(actual_version)
        .map_err(|_| StoreError::Integrity("Runtime incarnation version is invalid".to_owned()))?;
    if current_id != update.runtime_incarnation_id {
        return Err(StoreError::Integrity(
            "Runtime lifecycle update targets a stale incarnation".to_owned(),
        ));
    }
    if actual_version != update.expected_version {
        return Err(StoreError::Conflict {
            expected: Some(update.expected_version),
            actual: Some(actual_version),
        });
    }
    let allowed = matches!(
        (current_state.as_str(), update.recovery_state.as_str()),
        ("RECOVERING", "DEGRADED")
            | ("DEGRADED", "DRAINING")
            | ("DRAINING", "STOPPING")
            | ("STOPPING", "STOPPED")
    );
    if !allowed || old_stopped_at.is_some() {
        return Err(StoreError::Invalid(
            "Runtime lifecycle transition is not allowed".to_owned(),
        ));
    }
    let next_version = actual_version
        .checked_add(1)
        .ok_or_else(|| StoreError::Integrity("Runtime incarnation version exhausted".to_owned()))?;
    transaction.execute(
        "UPDATE runtime_incarnations SET recovery_state = ?1, stopped_at = ?2, version = ?3 WHERE runtime_id = ?4 AND runtime_incarnation_id = ?5 AND version = ?6",
        params![update.recovery_state, update.stopped_at, to_sql_i64(next_version, "Runtime incarnation version")?, update.runtime_id, update.runtime_incarnation_id, to_sql_i64(update.expected_version, "Runtime incarnation version")?],
    ).map_err(map_database_error)?;
    transaction.execute(
        "UPDATE runtimes SET availability = ?1, last_seen = ?2, version = version + 1 WHERE runtime_id = ?3 AND current_incarnation_id = ?4",
        params![update.availability, update.observed_at, update.runtime_id, update.runtime_incarnation_id],
    ).map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(RuntimeIncarnationRecord {
        runtime_incarnation_id: update.runtime_incarnation_id,
        runtime_id: update.runtime_id,
        process_started_at: started_at,
        litecowork_version: version,
        recovered_from_unclean_shutdown: recovered != 0,
        recovery_state: update.recovery_state,
        ready_at,
        stopped_at: update.stopped_at,
        version: next_version,
    })
}

fn writer_loop(
    path: PathBuf,
    config: SqliteConfig,
    receiver: Receiver<Command>,
    ready: SyncSender<Result<(), StoreError>>,
) {
    let connection = open_and_migrate(&path, &config);
    match connection {
        Ok(mut connection) => {
            if ready.send(Ok(())).is_err() {
                return;
            }
            while let Ok(command) = receiver.recv() {
                match command {
                    Command::CoworkerOperation { operation } => operation(&mut connection),
                    Command::DelegationProfileOperation { operation } => operation(&mut connection),
                    Command::GoalOperation { operation } => operation(&mut connection),
                    Command::SuggestionOperation { operation } => operation(&mut connection),
                    Command::RoutineOperation { operation } => operation(&mut connection),
                    Command::ExecutionOperation { operation } => operation(&mut connection),
                    Command::EnvironmentOperation { operation } => operation(&mut connection),
                    Command::RichPresentationOperation { operation } => operation(&mut connection),
                    Command::ConversationOperation { operation } => operation(&mut connection),
                    Command::EffectEvidenceOperation { operation } => operation(&mut connection),
                    Command::ArtifactOperation { operation } => operation(&mut connection),
                    Command::AuthorizeArtifactAppend {
                        workspace_id,
                        artifact_id,
                        principal_id,
                        expected_artifact_version,
                        expected_content_version,
                        reply,
                    } => {
                        let _ = reply.send(artifacts::authorize_artifact_append(
                            &connection,
                            &workspace_id,
                            &artifact_id,
                            &principal_id,
                            expected_artifact_version,
                            expected_content_version,
                        ));
                    }
                    Command::GetArtifactAppendHeads {
                        workspace_id,
                        artifact_id,
                        reply,
                    } => {
                        let _ = reply.send(artifacts::get_artifact_append_heads(
                            &connection,
                            &workspace_id,
                            &artifact_id,
                        ));
                    }
                    Command::ResolveArtifactAppendReplay {
                        workspace_id,
                        principal_id,
                        request_id,
                        request_digest,
                        reply,
                    } => {
                        let _ = reply.send(artifacts::resolve_artifact_append_replay(
                            &connection,
                            &workspace_id,
                            &principal_id,
                            &request_id,
                            &request_digest,
                        ));
                    }
                    Command::GetArtifact {
                        workspace_id,
                        artifact_id,
                        reply,
                    } => {
                        let _ = reply.send(artifacts::get_artifact(
                            &connection,
                            &workspace_id,
                            &artifact_id,
                        ));
                    }
                    Command::ListArtifacts {
                        workspace_id,
                        library_status,
                        task_id,
                        after_created_at,
                        after_artifact_id,
                        limit,
                        reply,
                    } => {
                        let _ = reply.send(artifacts::list_artifacts(
                            &connection,
                            &workspace_id,
                            library_status.as_deref(),
                            task_id.as_deref(),
                            after_created_at.as_deref(),
                            after_artifact_id.as_deref(),
                            limit,
                        ));
                    }
                    Command::GetArtifactVersion {
                        workspace_id,
                        artifact_id,
                        version,
                        reply,
                    } => {
                        let _ = reply.send(artifacts::get_artifact_version(
                            &connection,
                            &workspace_id,
                            &artifact_id,
                            version,
                        ));
                    }
                    Command::GetTaskPresentation {
                        workspace_id,
                        task_id,
                        reply,
                    } => {
                        let _ = reply.send(artifacts::read_task_presentation(
                            &mut connection,
                            &workspace_id,
                            &task_id,
                        ));
                    }
                    Command::EnrollLocalRuntimeInWorkspace {
                        request,
                        request_digest,
                        binding,
                        reply,
                    } => {
                        let _ = reply.send(enroll_local_runtime_transaction(
                            &mut connection,
                            request,
                            request_digest,
                            binding,
                        ));
                    }
                    Command::GetCurrentLocalRuntimeWorkspaceBinding { lookup, reply } => {
                        let _ = reply.send(get_current_local_runtime_workspace_binding(
                            &connection,
                            &lookup,
                        ));
                    }
                    Command::PutAgentProfile {
                        profile,
                        endpoints,
                        reply,
                    } => {
                        let _ = reply.send(put_agent_profile_transaction(
                            &mut connection,
                            profile,
                            endpoints,
                        ));
                    }
                    Command::RegisterLocalAgentEndpointBinding { binding, reply } => {
                        let _ = reply.send(register_local_endpoint_binding_transaction(
                            &mut connection,
                            binding,
                        ));
                    }
                    Command::GetLocalAgentEndpointBinding {
                        runtime_id,
                        runtime_incarnation_id,
                        endpoint_id,
                        now,
                        reply,
                    } => {
                        let _ = reply.send(get_local_endpoint_binding(
                            &connection,
                            &runtime_id,
                            &runtime_incarnation_id,
                            &endpoint_id,
                            &now,
                        ));
                    }
                    Command::PublishRuntimeOffer { offer, reply } => {
                        let _ =
                            reply.send(publish_runtime_offer_transaction(&mut connection, offer));
                    }
                    Command::ListAgentProfiles {
                        owner_principal_id,
                        workspace_id,
                        now,
                        reply,
                    } => {
                        let _ = reply.send(list_agent_profiles(
                            &connection,
                            &owner_principal_id,
                            &workspace_id,
                            &now,
                        ));
                    }
                    Command::CreateAgentBinding {
                        request,
                        state_ref,
                        reply,
                    } => {
                        let _ = reply.send(create_agent_binding_transaction(
                            &mut connection,
                            request,
                            state_ref,
                        ));
                    }
                    Command::GetAgentBinding {
                        owner_principal_id,
                        workspace_id,
                        agent_binding_id,
                        reply,
                    } => {
                        let _ = reply.send(get_agent_binding(
                            &connection,
                            &owner_principal_id,
                            &workspace_id,
                            &agent_binding_id,
                        ));
                    }
                    Command::ListAgentBindings {
                        owner_principal_id,
                        workspace_id,
                        reply,
                    } => {
                        let _ = reply.send(list_agent_bindings(
                            &connection,
                            &owner_principal_id,
                            &workspace_id,
                        ));
                    }
                    Command::EnableAgentBinding {
                        request,
                        state_ref,
                        reply,
                    } => {
                        let _ = reply.send(enable_agent_binding_transaction(
                            &mut connection,
                            request,
                            state_ref,
                        ));
                    }
                    Command::GetAgentBindingReceipt {
                        principal_id,
                        request_id,
                        request_digest,
                        reply,
                    } => {
                        let _ = reply.send(verify_request_receipt::<CommittedAgentBinding>(
                            &connection,
                            &principal_id,
                            &request_id,
                            &request_digest,
                        ));
                    }
                    Command::RegisterLocalRuntimeIncarnation {
                        runtime,
                        incarnation,
                        observation,
                        reply,
                    } => {
                        let _ = reply.send(register_local_runtime_incarnation_transaction(
                            &mut connection,
                            runtime,
                            incarnation,
                            observation,
                        ));
                    }
                    Command::TransitionLocalRuntimeIncarnation { update, reply } => {
                        let _ = reply.send(transition_local_runtime_incarnation_transaction(
                            &mut connection,
                            update,
                        ));
                    }
                    Command::ListWorkspaces { reply } => {
                        let _ = reply.send(list_workspaces(&connection));
                    }
                    Command::GetWorkspace {
                        workspace_id,
                        reply,
                    } => {
                        let _ = reply.send(load_workspace(&connection, &workspace_id));
                    }
                    Command::ListResourcesPage {
                        workspace_id,
                        after_created_at,
                        after_resource_id,
                        limit,
                        reply,
                    } => {
                        let _ = reply.send(list_resources_page(
                            &connection,
                            &workspace_id,
                            after_created_at.as_deref(),
                            after_resource_id.as_deref(),
                            limit,
                        ));
                    }
                    Command::GetResourceRecord {
                        workspace_id,
                        resource_id,
                        reply,
                    } => {
                        let _ = reply.send(load_resource_record(
                            &connection,
                            &workspace_id,
                            &resource_id,
                        ));
                    }
                    Command::GetResourceDetail {
                        workspace_id,
                        resource_id,
                        reply,
                    } => {
                        let _ = reply.send(load_resource_detail_record(
                            &connection,
                            &workspace_id,
                            &resource_id,
                        ));
                    }
                    Command::ListResourceRevisionsPage {
                        workspace_id,
                        resource_id,
                        after_revision_id,
                        limit,
                        reply,
                    } => {
                        let _ = reply.send(list_resource_revision_records_page(
                            &connection,
                            &workspace_id,
                            &resource_id,
                            after_revision_id.as_deref(),
                            limit,
                        ));
                    }
                    Command::SearchResourcesPage {
                        workspace_id,
                        query,
                        kind,
                        freshness,
                        after_created_at,
                        after_resource_id,
                        limit,
                        reply,
                    } => {
                        let _ = reply.send(search_resources_page(
                            &connection,
                            &workspace_id,
                            query.as_deref(),
                            kind.as_deref(),
                            freshness.as_deref(),
                            after_created_at.as_deref(),
                            after_resource_id.as_deref(),
                            limit,
                        ));
                    }
                    Command::ListResourceIndexKeyVersions {
                        workspace_id,
                        reply,
                    } => {
                        let result = resource_index::list_key_versions(&connection, &workspace_id);
                        let _ = reply.send(result);
                    }
                    Command::SearchResourceIndexCandidates {
                        workspace_id,
                        tokens_by_version,
                        kind,
                        freshness,
                        after_created_at,
                        after_resource_id,
                        limit,
                        reply,
                    } => {
                        let result = resource_index::search_candidates(
                            &connection,
                            &workspace_id,
                            &tokens_by_version,
                            kind.as_deref(),
                            freshness.as_deref(),
                            after_created_at.as_deref(),
                            after_resource_id.as_deref(),
                            limit,
                        );
                        let _ = reply.send(result);
                    }
                    Command::RecheckResourceIndexCandidate {
                        workspace_id,
                        resource_id,
                        revision_id,
                        source_digest,
                        reply,
                    } => {
                        let result = resource_index::candidate_is_current(
                            &connection,
                            &workspace_id,
                            &resource_id,
                            &revision_id,
                            &source_digest,
                        );
                        let _ = reply.send(result);
                    }
                    Command::ReplaceResourceTextIndex {
                        workspace_id,
                        resource_id,
                        revision_id,
                        source_digest,
                        index,
                        reply,
                    } => {
                        let result = resource_index::replace_current(
                            &mut connection,
                            &workspace_id,
                            &resource_id,
                            &revision_id,
                            &source_digest,
                            index.as_ref(),
                        );
                        let _ = reply.send(result);
                    }
                    Command::GetCurrentResourceSummary {
                        workspace_id,
                        resource_id,
                        reply,
                    } => {
                        let _ = reply.send(load_current_resource_summary(
                            &connection,
                            &workspace_id,
                            &resource_id,
                        ));
                    }
                    Command::CheckResourceTextIndexRebuildReceipt {
                        principal_id,
                        request_id,
                        request_digest,
                        reply,
                    } => {
                        let result = verify_request_receipt::<ResourceTextIndexRebuildResult>(
                            &connection,
                            &principal_id,
                            &request_id,
                            &request_digest,
                        );
                        let _ = reply.send(result);
                    }
                    Command::CommitResourceTextIndexRebuild {
                        request,
                        request_digest,
                        result,
                        index,
                        reply,
                    } => {
                        let outcome = commit_resource_text_index_rebuild_transaction(
                            &mut connection,
                            request,
                            request_digest,
                            result,
                            index,
                        );
                        let _ = reply.send(outcome);
                    }
                    Command::ReadResourceContent {
                        workspace_id,
                        resource_id,
                        revision_id,
                        maximum_bytes,
                        reply,
                    } => {
                        let _ = reply.send(read_resource_content_metadata(
                            &connection,
                            &workspace_id,
                            &resource_id,
                            revision_id.as_deref(),
                            maximum_bytes,
                        ));
                    }
                    Command::GetResourceContextDocumentStatus {
                        workspace_id,
                        resource_id,
                        reply,
                    } => {
                        let status = connection.query_row(
                            "SELECT json_extract(context_document_json, '$.status') FROM resources WHERE workspace_id = ?1 AND resource_id = ?2",
                            params![workspace_id, resource_id],
                            |row| {
                                let status: Option<String> = row.get(0)?;
                                Ok(status)
                            },
                        ).optional().map(|status| status.flatten()).map_err(map_database_error);
                        let _ = reply.send(status);
                    }
                    Command::SetContextDocumentStatus {
                        command,
                        blobs,
                        reply,
                    } => {
                        let result = set_context_document_status_transaction(
                            &mut connection,
                            blobs.as_ref(),
                            command,
                        );
                        let _ = reply.send(result);
                    }
                    Command::ListWorkspaceInstructionRevisions {
                        workspace_id,
                        after_revision,
                        limit,
                        reply,
                    } => {
                        let _ = reply.send(list_workspace_instruction_revisions(
                            &connection,
                            &workspace_id,
                            after_revision,
                            limit,
                        ));
                    }
                    Command::CreateTask {
                        commit,
                        state_ref,
                        occurrence_state_refs,
                        reply,
                    } => {
                        let _ = reply.send(create_task_transaction(
                            &mut connection,
                            *commit,
                            state_ref,
                            occurrence_state_refs,
                        ));
                    }
                    Command::GetTaskCreateReceipt {
                        principal_id,
                        request_id,
                        request_digest,
                        reply,
                    } => {
                        let _ = reply.send(verify_request_receipt::<storage_core::CommittedTask>(
                            &connection,
                            &principal_id,
                            &request_id,
                            &request_digest,
                        ));
                    }
                    Command::GetTaskSpecRevisionReceipt {
                        principal_id,
                        request_id,
                        request_payload,
                        reply,
                    } => {
                        let _ = reply.send(load_task_spec_revision_receipt(
                            &connection,
                            &principal_id,
                            &request_id,
                            &request_payload,
                        ));
                    }
                    Command::ReviseTaskSpec {
                        commit,
                        state_ref,
                        reply,
                    } => {
                        let _ = reply.send(revise_task_spec_transaction(
                            &mut connection,
                            *commit,
                            state_ref,
                        ));
                    }
                    Command::AcceptInitialPlan {
                        commit,
                        task_state_ref,
                        step_state_refs,
                        reply,
                    } => {
                        let _ = reply.send(accept_initial_plan_transaction(
                            &mut connection,
                            *commit,
                            task_state_ref,
                            step_state_refs,
                        ));
                    }
                    Command::ListPlanRevisions {
                        workspace_id,
                        task_id,
                        reply,
                    } => {
                        let _ =
                            reply.send(list_plan_revisions(&connection, &workspace_id, &task_id));
                    }
                    Command::ListSteps {
                        workspace_id,
                        task_id,
                        plan_revision,
                        reply,
                    } => {
                        let _ = reply.send(list_steps(
                            &connection,
                            &workspace_id,
                            &task_id,
                            plan_revision,
                        ));
                    }
                    Command::StartTaskPlanningSession {
                        start,
                        state_ref,
                        reply,
                    } => {
                        let _ = reply.send(start_task_planning_session_transaction(
                            &mut connection,
                            start,
                            state_ref,
                        ));
                    }
                    Command::GetAgentSession {
                        workspace_id,
                        agent_session_id,
                        reply,
                    } => {
                        let _ = reply.send(load_agent_session(
                            &connection,
                            &workspace_id,
                            &agent_session_id,
                        ));
                    }
                    Command::MarkStartingAgentSessionLost {
                        transition,
                        next,
                        state_ref,
                        reply,
                    } => {
                        let _ = reply.send(mark_starting_agent_session_lost_transaction(
                            &mut connection,
                            transition,
                            next,
                            state_ref,
                        ));
                    }
                    Command::ActivateTaskPlanningSession {
                        activation,
                        session,
                        task,
                        session_state_ref,
                        task_state_ref,
                        reply,
                    } => {
                        let _ = reply.send(activate_task_planning_session_transaction(
                            &mut connection,
                            activation,
                            session,
                            task,
                            session_state_ref,
                            task_state_ref,
                        ));
                    }
                    Command::CreateAgentHostInstance { host, reply } => {
                        let _ = reply.send(create_agent_host_instance_transaction(
                            &mut connection,
                            host,
                        ));
                    }
                    Command::TransitionAgentHostInstance {
                        runtime_id,
                        runtime_incarnation_id,
                        host_instance_id,
                        expected_state,
                        next_state,
                        occurred_at,
                        process_identity_ref,
                        reply,
                    } => {
                        let _ = reply.send(transition_agent_host_instance_transaction(
                            &mut connection,
                            &runtime_id,
                            &runtime_incarnation_id,
                            &host_instance_id,
                            &expected_state,
                            &next_state,
                            &occurred_at,
                            process_identity_ref.as_deref(),
                        ));
                    }
                    Command::GetAgentHostInstance {
                        runtime_id,
                        runtime_incarnation_id,
                        host_instance_id,
                        reply,
                    } => {
                        let _ = reply.send(load_agent_host_instance(
                            &connection,
                            &runtime_id,
                            &runtime_incarnation_id,
                            &host_instance_id,
                        ));
                    }
                    Command::ListAgentHostInstances {
                        runtime_id,
                        runtime_incarnation_id,
                        reply,
                    } => {
                        let _ = reply.send(list_agent_host_instances(
                            &connection,
                            &runtime_id,
                            &runtime_incarnation_id,
                        ));
                    }
                    Command::ListStartingTaskPlanningSessions {
                        workspace_id,
                        limit,
                        reply,
                    } => {
                        let _ = reply.send(list_starting_task_planning_sessions(
                            &connection,
                            &workspace_id,
                            limit,
                        ));
                    }
                    Command::GetTask {
                        workspace_id,
                        task_id,
                        reply,
                    } => {
                        let _ = reply.send(load_task_view(&connection, &workspace_id, &task_id));
                    }
                    Command::ListTaskSpecRevisions {
                        workspace_id,
                        task_id,
                        reply,
                    } => {
                        let _ = reply.send(list_task_spec_revisions(
                            &connection,
                            &workspace_id,
                            &task_id,
                        ));
                    }
                    Command::ListTasksPage {
                        workspace_id,
                        status,
                        conversation_id,
                        after_created_at,
                        after_task_id,
                        limit,
                        reply,
                    } => {
                        let _ = reply.send(list_tasks_page(
                            &connection,
                            &workspace_id,
                            status.as_deref(),
                            conversation_id.as_deref(),
                            after_created_at.as_deref(),
                            after_task_id.as_deref(),
                            limit,
                        ));
                    }
                    Command::CommitWorkspace { commit, reply } => {
                        let _ = reply.send(commit_workspace_transaction_with_dedup(
                            &mut connection,
                            commit.expected_version,
                            commit.workspace,
                            commit.draft,
                            commit.state_ref,
                            commit.request,
                            None,
                        ));
                    }
                    Command::ReadWorkspaceEvents {
                        workspace_id,
                        reply,
                    } => {
                        let _ = reply.send(read_workspace_events(&connection, &workspace_id));
                    }
                    Command::SqliteVersion { reply } => {
                        let version = connection
                            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
                            .map_err(map_database_error);
                        let _ = reply.send(version);
                    }
                    Command::CreateResource {
                        request,
                        resource,
                        revision,
                        draft,
                        state_ref,
                        content_blob,
                        text_index,
                        location_id,
                        reply,
                    } => {
                        let result = create_resource_transaction(
                            &mut connection,
                            request,
                            resource,
                            revision,
                            draft,
                            state_ref,
                            content_blob,
                            text_index,
                            location_id,
                            None,
                            None,
                        );
                        let _ = reply.send(result);
                    }
                    Command::CreateWorkspaceRoot {
                        commit,
                        resource_state_ref,
                        root_state_ref,
                        reply,
                    } => {
                        let result = create_workspace_root_transaction(
                            &mut connection,
                            commit,
                            resource_state_ref,
                            root_state_ref,
                        );
                        let _ = reply.send(result);
                    }
                    Command::ListWorkspaceRoots {
                        workspace_id,
                        status,
                        after_created_at,
                        after_workspace_root_id,
                        limit,
                        reply,
                    } => {
                        let _ = reply.send(list_workspace_roots_page(
                            &connection,
                            &workspace_id,
                            status.as_deref(),
                            after_created_at.as_deref(),
                            after_workspace_root_id.as_deref(),
                            limit,
                        ));
                    }
                    Command::GetWorkspaceRoot {
                        workspace_id,
                        workspace_root_id,
                        reply,
                    } => {
                        let _ = reply.send(load_workspace_root(
                            &connection,
                            &workspace_id,
                            &workspace_root_id,
                        ));
                    }
                    Command::UpdateWorkspaceRootStatus {
                        commit,
                        root_state_ref,
                        reply,
                    } => {
                        let _ = reply.send(update_workspace_root_status_transaction(
                            &mut connection,
                            commit,
                            root_state_ref,
                        ));
                    }
                    Command::ResumeWorkspaceRoot {
                        commit,
                        root_state_ref,
                        resource_state_ref,
                        reply,
                    } => {
                        let _ = reply.send(resume_workspace_root_transaction(
                            &mut connection,
                            commit,
                            root_state_ref,
                            resource_state_ref,
                        ));
                    }
                    Command::GetWorkspaceRootStatusReceipt { request, reply } => {
                        let _ =
                            reply.send(get_workspace_root_status_receipt(&connection, &request));
                    }
                    Command::ListWorkspaceRootRevalidationCandidates {
                        runtime_id,
                        runtime_incarnation_id,
                        after_created_at,
                        after_workspace_root_id,
                        limit,
                        reply,
                    } => {
                        let _ = reply.send(list_workspace_root_revalidation_candidates(
                            &connection,
                            &runtime_id,
                            &runtime_incarnation_id,
                            after_created_at.as_deref(),
                            after_workspace_root_id.as_deref(),
                            limit,
                        ));
                    }
                    Command::CommitWorkspaceRootRevalidation {
                        commit,
                        root_state_ref,
                        resource_state_ref,
                        reply,
                    } => {
                        let _ = reply.send(commit_workspace_root_revalidation_transaction(
                            &mut connection,
                            commit,
                            root_state_ref,
                            resource_state_ref,
                        ));
                    }
                    Command::CreateResourceUpload {
                        request,
                        session,
                        draft,
                        state_ref,
                        reply,
                    } => {
                        let _ = reply.send(create_resource_upload_transaction(
                            &mut connection,
                            request,
                            session,
                            draft,
                            state_ref,
                        ));
                    }
                    Command::GetResourceUpload {
                        workspace_id,
                        upload_id,
                        reply,
                    } => {
                        let _ = reply.send(load_resource_upload(
                            &connection,
                            &workspace_id,
                            &upload_id,
                        ));
                    }
                    Command::GetCommittedResourceUpload {
                        principal_id,
                        upload_id,
                        reply,
                    } => {
                        let _ = reply.send(load_resource_upload_commit_receipt(
                            &connection,
                            &principal_id,
                            &upload_id,
                        ));
                    }
                    Command::ListExpiredResourceUploads { now, limit, reply } => {
                        let _ = reply.send(list_expired_resource_uploads(&connection, &now, limit));
                    }
                    Command::ExpireResourceUpload {
                        expected_progress_version,
                        expired,
                        draft,
                        state_ref,
                        reply,
                    } => {
                        let _ = reply.send(expire_resource_upload_transaction(
                            &mut connection,
                            expected_progress_version,
                            expired,
                            draft,
                            state_ref,
                        ));
                    }
                    Command::FailResourceUpload {
                        expected_version,
                        expected_progress_version,
                        failed,
                        draft,
                        state_ref,
                        reply,
                    } => {
                        let _ = reply.send(fail_resource_upload_transaction(
                            &mut connection,
                            expected_version,
                            expected_progress_version,
                            failed,
                            draft,
                            state_ref,
                        ));
                    }
                    Command::ReserveResourceUploadBlob {
                        workspace_id,
                        upload_id,
                        request_id,
                        chunk_index,
                        digest,
                        size_bytes,
                        created_at,
                        expires_at,
                        reply,
                    } => {
                        let _ = reply.send(reserve_resource_upload_blob_transaction(
                            &mut connection,
                            &workspace_id,
                            &upload_id,
                            &request_id,
                            chunk_index,
                            &digest,
                            size_bytes,
                            &created_at,
                            &expires_at,
                        ));
                    }
                    Command::ClaimOrphanResourceUploadBlobs { now, limit, reply } => {
                        let _ = reply.send(claim_orphan_resource_upload_blobs_transaction(
                            &mut connection,
                            &now,
                            limit,
                        ));
                    }
                    Command::FinishOrphanResourceUploadBlob {
                        workspace_id,
                        digest,
                        reply,
                    } => {
                        let _ = reply.send(finish_orphan_resource_upload_blob_transaction(
                            &mut connection,
                            &workspace_id,
                            &digest,
                        ));
                    }
                    Command::GetResourceUploadChunks {
                        workspace_id,
                        upload_id,
                        reply,
                    } => {
                        let _ = reply.send(load_resource_upload_chunks(
                            &connection,
                            &workspace_id,
                            &upload_id,
                        ));
                    }
                    Command::PutResourceUploadChunk {
                        chunk,
                        blob,
                        expected_progress_version,
                        completion_event,
                        completion_state_ref,
                        reply,
                    } => {
                        let _ = reply.send(put_resource_upload_chunk_transaction(
                            &mut connection,
                            chunk,
                            blob,
                            expected_progress_version,
                            completion_event,
                            completion_state_ref,
                        ));
                    }
                    Command::CommitResourceUpload {
                        request,
                        resource,
                        revision,
                        draft,
                        state_ref,
                        upload_commit,
                        content_blob,
                        text_index,
                        location_id,
                        context_document,
                        reply,
                    } => {
                        let result = create_resource_transaction(
                            &mut connection,
                            request,
                            resource,
                            revision,
                            draft,
                            state_ref,
                            content_blob,
                            text_index,
                            location_id,
                            upload_commit,
                            context_document,
                        );
                        let _ = reply.send(result);
                    }
                    Command::CreateWorkspaceInstructionRevision {
                        request,
                        expected_version,
                        workspace,
                        instruction_revision,
                        draft,
                        state_ref,
                        reply,
                    } => {
                        let result = create_workspace_instruction_revision_transaction(
                            &mut connection,
                            request,
                            expected_version,
                            workspace,
                            instruction_revision,
                            draft,
                            state_ref,
                        );
                        let _ = reply.send(result);
                    }
                    Command::Shutdown => break,
                }
            }
        }
        Err(error) => {
            let _ = ready.send(Err(error));
        }
    }
}

fn open_and_migrate(path: &Path, config: &SqliteConfig) -> Result<Connection, StoreError> {
    validate_database_file_path(path)?;
    let mut connection = Connection::open(path).map_err(map_database_error)?;
    restrict_database_file(path)?;
    connection
        .busy_timeout(config.busy_timeout)
        .map_err(map_database_error)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(map_database_error)?;
    let foreign_keys: i64 = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(map_database_error)?;
    if foreign_keys != 1 {
        return Err(StoreError::CorruptSchema(
            "foreign-key enforcement is unavailable".to_owned(),
        ));
    }
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(map_database_error)?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        let selected: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(map_database_error)?;
        if !selected.eq_ignore_ascii_case("wal") {
            return Err(StoreError::CorruptSchema(
                "SQLite did not enable WAL mode".to_owned(),
            ));
        }
    }
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(map_database_error)?;
    validate_sqlite_runtime(&connection)?;
    migrate(&mut connection)?;
    validate_schema(&connection)?;
    Ok(connection)
}

fn ensure_private_directory(path: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(StoreError::Io(
                    "SQLite state path must be a real directory".to_owned(),
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o077 != 0 {
                    return Err(StoreError::Io(
                        "SQLite state directory grants group or other access".to_owned(),
                    ));
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = fs::DirBuilder::new();
                builder.recursive(true).mode(0o700);
                builder.create(path).map_err(map_io_error)?;
            }
            #[cfg(not(unix))]
            fs::create_dir_all(path).map_err(map_io_error)?;
            ensure_private_directory(path)
        }
        Err(error) => Err(map_io_error(error)),
    }
}

fn restrict_database_file(path: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(path).map_err(map_io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StoreError::Io(
            "SQLite database path must be a regular file".to_owned(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(map_io_error)?;
    }
    Ok(())
}

fn validate_database_file_path(path: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
            StoreError::Io("SQLite database path must be a regular file".to_owned()),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(map_io_error(error)),
    }
}

fn map_io_error(error: std::io::Error) -> StoreError {
    StoreError::Io(error.to_string())
}

fn validate_sqlite_runtime(connection: &Connection) -> Result<(), StoreError> {
    let version: String = connection
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .map_err(map_database_error)?;
    let parts: Vec<u32> = version
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect();
    let actual = (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
        parts.get(2).copied().unwrap_or(0),
    );
    if actual < (3, 38, 0) {
        return Err(StoreError::CorruptSchema(format!(
            "SQLite {version} is older than the required 3.38"
        )));
    }
    let json_functions: i64 = connection
        .query_row("SELECT json_valid('{}')", [], |row| row.get(0))
        .map_err(map_database_error)?;
    if json_functions != 1 {
        return Err(StoreError::CorruptSchema(
            "SQLite JSON functions are unavailable".to_owned(),
        ));
    }
    Ok(())
}

fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_with_failpoint(connection, None)
}

fn migrate_with_failpoint(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let schema_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if schema_version > SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema(schema_version));
    }

    if schema_version == SCHEMA_VERSION {
        verify_migration_record(connection, 1, V1_MIGRATION_NAME, SQLITE_V1_DDL, false)?;
        verify_migration_record(connection, 2, V2_MIGRATION_NAME, SQLITE_V2_DDL, false)?;
        verify_migration_record(connection, 3, V3_MIGRATION_NAME, SQLITE_V3_DDL, false)?;
        verify_migration_record(connection, 4, V4_MIGRATION_NAME, SQLITE_V4_DDL, false)?;
        verify_migration_record(connection, 5, V5_MIGRATION_NAME, SQLITE_V5_DDL, false)?;
        verify_migration_record(connection, 6, V6_MIGRATION_NAME, SQLITE_V6_DDL, false)?;
        verify_migration_record(connection, 7, V7_MIGRATION_NAME, SQLITE_V7_DDL, false)?;
        verify_migration_record(connection, 8, V8_MIGRATION_NAME, SQLITE_V8_DDL, false)?;
        verify_migration_record(connection, 9, V9_MIGRATION_NAME, SQLITE_V9_DDL, false)?;
        verify_migration_record(connection, 10, V10_MIGRATION_NAME, SQLITE_V10_DDL, false)?;
        verify_migration_record(connection, 11, V11_MIGRATION_NAME, SQLITE_V11_DDL, false)?;
        verify_migration_record(connection, 12, V12_MIGRATION_NAME, SQLITE_V12_DDL, true)?;
        verify_migration_record(connection, 13, V13_MIGRATION_NAME, SQLITE_V13_DDL, true)?;
        return Ok(());
    }

    if schema_version == 0 {
        let object_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .map_err(map_database_error)?;
        if object_count != 0 {
            return Err(StoreError::CorruptSchema(
                "unversioned database is not empty".to_owned(),
            ));
        }

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_database_error)?;
        transaction
            .execute_batch(SQLITE_V1_DDL)
            .map_err(map_database_error)?;
        record_migration(&transaction, 1, V1_MIGRATION_NAME, SQLITE_V1_DDL)?;
        transaction
            .execute_batch(SQLITE_V2_DDL)
            .map_err(map_database_error)?;
        record_migration(&transaction, 2, V2_MIGRATION_NAME, SQLITE_V2_DDL)?;
        transaction
            .execute_batch(SQLITE_V3_DDL)
            .map_err(map_database_error)?;
        fail_if(failpoint, Failpoint::DuringMigration)?;
        record_migration(&transaction, 3, V3_MIGRATION_NAME, SQLITE_V3_DDL)?;
        transaction
            .pragma_update(None, "user_version", V3_SCHEMA_VERSION)
            .map_err(map_database_error)?;
        transaction.commit().map_err(map_database_error)?;
    } else {
        verify_migration_record(connection, 1, V1_MIGRATION_NAME, SQLITE_V1_DDL, false)?;
        if schema_version >= 2 {
            verify_migration_record(connection, 2, V2_MIGRATION_NAME, SQLITE_V2_DDL, false)?;
        }
        if schema_version >= 3 {
            verify_migration_record(
                connection,
                3,
                V3_MIGRATION_NAME,
                SQLITE_V3_DDL,
                schema_version == 3,
            )?;
        }
        if schema_version >= 4 {
            verify_migration_record(
                connection,
                4,
                V4_MIGRATION_NAME,
                SQLITE_V4_DDL,
                schema_version == 4,
            )?;
        }
        if schema_version >= 5 {
            verify_migration_record(
                connection,
                5,
                V5_MIGRATION_NAME,
                SQLITE_V5_DDL,
                schema_version == 5,
            )?;
        }
        if schema_version >= 6 {
            verify_migration_record(
                connection,
                6,
                V6_MIGRATION_NAME,
                SQLITE_V6_DDL,
                schema_version == 6,
            )?;
        }
        if schema_version >= 7 {
            verify_migration_record(
                connection,
                7,
                V7_MIGRATION_NAME,
                SQLITE_V7_DDL,
                schema_version == 7,
            )?;
        }
        if schema_version >= 8 {
            verify_migration_record(
                connection,
                8,
                V8_MIGRATION_NAME,
                SQLITE_V8_DDL,
                schema_version == 8,
            )?;
        }
        if schema_version >= 9 {
            verify_migration_record(
                connection,
                9,
                V9_MIGRATION_NAME,
                SQLITE_V9_DDL,
                schema_version == 9,
            )?;
        }
        if schema_version >= 10 {
            verify_migration_record(
                connection,
                10,
                V10_MIGRATION_NAME,
                SQLITE_V10_DDL,
                schema_version == 10,
            )?;
        }
        if schema_version >= 11 {
            verify_migration_record(
                connection,
                11,
                V11_MIGRATION_NAME,
                SQLITE_V11_DDL,
                schema_version == 11,
            )?;
        }
        if schema_version == 1 {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            transaction
                .execute_batch(SQLITE_V2_DDL)
                .map_err(map_database_error)?;
            record_migration(&transaction, 2, V2_MIGRATION_NAME, SQLITE_V2_DDL)?;
            transaction
                .execute_batch(SQLITE_V3_DDL)
                .map_err(map_database_error)?;
            fail_if(failpoint, Failpoint::DuringMigration)?;
            record_migration(&transaction, 3, V3_MIGRATION_NAME, SQLITE_V3_DDL)?;
            transaction
                .pragma_update(None, "user_version", V3_SCHEMA_VERSION)
                .map_err(map_database_error)?;
            transaction.commit().map_err(map_database_error)?;
        } else if schema_version == 2 {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            transaction
                .execute_batch(SQLITE_V3_DDL)
                .map_err(map_database_error)?;
            fail_if(failpoint, Failpoint::DuringMigration)?;
            record_migration(&transaction, 3, V3_MIGRATION_NAME, SQLITE_V3_DDL)?;
            transaction
                .pragma_update(None, "user_version", V3_SCHEMA_VERSION)
                .map_err(map_database_error)?;
            transaction.commit().map_err(map_database_error)?;
        }
    }

    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V3_SCHEMA_VERSION {
        migrate_installation_scoped_runtimes(connection, failpoint)?;
    }
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V4_SCHEMA_VERSION {
        migrate_plan_integrity(connection, failpoint)?;
    }
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V5_SCHEMA_VERSION {
        migrate_resource_location_unavailable(connection, failpoint)?;
    }
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V6_SCHEMA_VERSION {
        migrate_encrypted_resource_text_index(connection, failpoint)?;
    }
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V7_SCHEMA_VERSION {
        migrate_immutable_routine_revisions(connection, failpoint)?;
    }
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V8_SCHEMA_VERSION {
        migrate_effect_evidence_guards(connection, failpoint)?;
    }
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V9_SCHEMA_VERSION {
        migrate_goal_artifact_links(connection, failpoint)?;
    }
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V10_SCHEMA_VERSION {
        migrate_automation_occurrence_revision(connection, failpoint)?;
    }
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V11_SCHEMA_VERSION {
        migrate_rich_presentation(connection, failpoint)?;
    }
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if current_version == V12_SCHEMA_VERSION {
        migrate_environment_identity_guard(connection, failpoint)?;
    } else if current_version != SCHEMA_VERSION {
        return Err(StoreError::CorruptSchema(format!(
            "migration stopped at unexpected schema version {current_version}"
        )));
    }
    Ok(())
}

/// Version twelve adds the optional immutable RichPresentation enhancement. Semantic
/// ConversationMessages remain independently readable if their presentation blob is
/// unavailable.
fn migrate_rich_presentation(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    transaction
        .execute_batch(SQLITE_V12_DDL)
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::DuringMigration)?;
    record_migration(&transaction, 12, V12_MIGRATION_NAME, SQLITE_V12_DDL)?;
    fail_if(failpoint, Failpoint::DuringV12Migration)?;
    transaction
        .pragma_update(None, "user_version", V12_SCHEMA_VERSION)
        .map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(())
}

/// Version thirteen makes Environment sharing and principal/Coworker ownership fields
/// immutable at the database boundary, including direct SQL writers.
fn migrate_environment_identity_guard(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    transaction
        .execute_batch(SQLITE_V13_DDL)
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::DuringV13Migration)?;
    record_migration(&transaction, 13, V13_MIGRATION_NAME, SQLITE_V13_DDL)?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(map_database_error)?;
    validate_schema(&transaction)?;
    transaction.commit().map_err(map_database_error)?;
    validate_schema(connection)
}

/// Version four removes the legacy Runtime→Workspace ownership column and rebuilds
/// dependent tables. SQLite only permits changing `foreign_keys` outside a transaction,
/// so enforcement is disabled before the immediate migration transaction and restored
/// on every exit path. `foreign_key_check` runs inside the transaction before commit.
fn migrate_installation_scoped_runtimes(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let foreign_keys: i64 = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(map_database_error)?;
    if foreign_keys != 1 {
        return Err(StoreError::CorruptSchema(
            "foreign key enforcement must be enabled before Runtime migration".to_owned(),
        ));
    }
    let legacy_alter_table: i64 = connection
        .pragma_query_value(None, "legacy_alter_table", |row| row.get(0))
        .map_err(map_database_error)?;

    connection
        .pragma_update(None, "foreign_keys", false)
        .map_err(map_database_error)?;
    if let Err(error) = connection.pragma_update(None, "legacy_alter_table", true) {
        let _ = connection.pragma_update(None, "foreign_keys", true);
        return Err(map_database_error(error));
    }
    let migration_result = (|| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_database_error)?;
        // Recheck under the migration write lock so an assignment cannot acquire
        // unrecorded continuity state between preflight and the v4 table rebuild.
        validate_legacy_channel_host_provenance(&transaction)?;
        transaction
            .execute_batch(SQLITE_V4_DDL)
            .map_err(map_database_error)?;
        validate_runtime_workspace_data(&transaction)?;
        fail_if(failpoint, Failpoint::DuringV4Migration)?;
        record_migration(&transaction, 4, V4_MIGRATION_NAME, SQLITE_V4_DDL)?;
        transaction
            .pragma_update(None, "user_version", V4_SCHEMA_VERSION)
            .map_err(map_database_error)?;
        validate_schema(&transaction)?;
        transaction.commit().map_err(map_database_error)
    })();

    let restore_result = (|| {
        connection
            .pragma_update(None, "legacy_alter_table", legacy_alter_table != 0)
            .map_err(map_database_error)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(map_database_error)?;
        let restored: i64 = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .map_err(map_database_error)?;
        if restored != 1 {
            return Err(StoreError::CorruptSchema(
                "foreign key enforcement was not restored after Runtime migration".to_owned(),
            ));
        }
        Ok(())
    })();

    match (migration_result, restore_result) {
        (Ok(()), Ok(())) => validate_schema(connection),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(migration), Err(restore)) => Err(StoreError::CorruptSchema(format!(
            "Runtime migration failed ({migration}); restoring foreign-key enforcement failed ({restore})"
        ))),
    }
}

fn migrate_plan_integrity(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    transaction
        .execute_batch(SQLITE_V5_DDL)
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::DuringV5Migration)?;
    record_migration(&transaction, 5, V5_MIGRATION_NAME, SQLITE_V5_DDL)?;
    transaction
        .pragma_update(None, "user_version", V5_SCHEMA_VERSION)
        .map_err(map_database_error)?;
    validate_schema(&transaction)?;
    transaction.commit().map_err(map_database_error)
}

/// Version six extends location availability without changing existing records. Both
/// `legacy_alter_table` and FK enforcement are controlled outside the write transaction
/// so immutable dependent triggers keep their original table-name bindings.
fn migrate_resource_location_unavailable(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let foreign_keys: i64 = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(map_database_error)?;
    if foreign_keys != 1 {
        return Err(StoreError::CorruptSchema(
            "foreign key enforcement must be enabled before ResourceLocation migration".to_owned(),
        ));
    }
    let legacy_alter_table: i64 = connection
        .pragma_query_value(None, "legacy_alter_table", |row| row.get(0))
        .map_err(map_database_error)?;
    connection
        .pragma_update(None, "foreign_keys", false)
        .map_err(map_database_error)?;
    if let Err(error) = connection.pragma_update(None, "legacy_alter_table", true) {
        let _ = connection.pragma_update(None, "foreign_keys", true);
        return Err(map_database_error(error));
    }
    let migration_result = (|| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_database_error)?;
        transaction
            .execute_batch(SQLITE_V6_DDL)
            .map_err(map_database_error)?;
        fail_if(failpoint, Failpoint::DuringV6Migration)?;
        record_migration(&transaction, 6, V6_MIGRATION_NAME, SQLITE_V6_DDL)?;
        transaction
            .pragma_update(None, "user_version", V6_SCHEMA_VERSION)
            .map_err(map_database_error)?;
        validate_schema(&transaction)?;
        transaction.commit().map_err(map_database_error)
    })();
    let restore_result = (|| {
        connection
            .pragma_update(None, "legacy_alter_table", legacy_alter_table != 0)
            .map_err(map_database_error)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(map_database_error)?;
        let restored: i64 = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .map_err(map_database_error)?;
        if restored != 1 {
            return Err(StoreError::CorruptSchema(
                "foreign key enforcement was not restored after ResourceLocation migration"
                    .to_owned(),
            ));
        }
        Ok(())
    })();
    match (migration_result, restore_result) {
        (Ok(()), Ok(())) => validate_schema(connection),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(migration), Err(restore)) => Err(StoreError::CorruptSchema(format!(
            "ResourceLocation migration failed ({migration}); restoring foreign-key enforcement failed ({restore})"
        ))),
    }
}

/// Version seven adds an encrypted, revision-scoped lexical Resource projection. The
/// projection starts empty and is rebuilt only from currently authorized source bytes.
fn migrate_encrypted_resource_text_index(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    transaction
        .execute_batch(SQLITE_V7_DDL)
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::DuringV7Migration)?;
    record_migration(&transaction, 7, V7_MIGRATION_NAME, SQLITE_V7_DDL)?;
    transaction
        .pragma_update(None, "user_version", V7_SCHEMA_VERSION)
        .map_err(map_database_error)?;
    validate_schema(&transaction)?;
    transaction.commit().map_err(map_database_error)?;
    validate_schema(connection)
}

/// Version eight prevents direct SQL callers from rewriting or deleting a Routine
/// revision and enforces the head's append-only/archive transition contract.
fn migrate_immutable_routine_revisions(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    transaction
        .execute_batch(SQLITE_V8_DDL)
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::DuringV8Migration)?;
    record_migration(&transaction, 8, V8_MIGRATION_NAME, SQLITE_V8_DDL)?;
    transaction
        .pragma_update(None, "user_version", V8_SCHEMA_VERSION)
        .map_err(map_database_error)?;
    validate_schema(&transaction)?;
    transaction.commit().map_err(map_database_error)?;
    validate_schema(connection)
}

/// Version nine makes the already documented Effect lifecycle and append-only Evidence
/// contract resistant to direct SQL updates/deletes.
fn migrate_effect_evidence_guards(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    preflight_effect_evidence_v9(&transaction)?;
    transaction
        .execute_batch(SQLITE_V9_DDL)
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::DuringV9Migration)?;
    record_migration(&transaction, 9, V9_MIGRATION_NAME, SQLITE_V9_DDL)?;
    transaction
        .pragma_update(None, "user_version", V9_SCHEMA_VERSION)
        .map_err(map_database_error)?;
    validate_schema(&transaction)?;
    transaction.commit().map_err(map_database_error)?;
    validate_schema(connection)
}

/// Version ten adds append-only Goal links to exact same-Workspace Artifact versions.
fn migrate_goal_artifact_links(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    transaction
        .execute_batch(SQLITE_V10_DDL)
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::DuringV10Migration)?;
    record_migration(&transaction, 10, V10_MIGRATION_NAME, SQLITE_V10_DDL)?;
    transaction
        .pragma_update(None, "user_version", V10_SCHEMA_VERSION)
        .map_err(map_database_error)?;
    validate_schema(&transaction)?;
    transaction.commit().map_err(map_database_error)?;
    validate_schema(connection)
}

/// Version eleven gives each AutomationOccurrence an aggregate revision independent
/// of its claim_epoch fencing counter.
fn migrate_automation_occurrence_revision(
    connection: &mut Connection,
    failpoint: Option<Failpoint>,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    transaction
        .execute_batch(SQLITE_V11_DDL)
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::DuringV11Migration)?;
    record_migration(&transaction, 11, V11_MIGRATION_NAME, SQLITE_V11_DDL)?;
    transaction
        .pragma_update(None, "user_version", V11_SCHEMA_VERSION)
        .map_err(map_database_error)?;
    validate_schema(&transaction)?;
    transaction.commit().map_err(map_database_error)?;
    validate_schema(connection)
}

/// Migration nine cannot prove that rows written before these triggers were append-only
/// or that higher-assurance Evidence came from an authenticated producer. Refuse a
/// populated legacy Effect/Evidence history instead of blessing unverifiable provenance.
fn preflight_effect_evidence_v9(connection: &Connection) -> Result<(), StoreError> {
    let existing: i64 = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM effects) + (SELECT COUNT(*) FROM evidence)",
            [],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if existing != 0 {
        return Err(StoreError::Integrity(
            "schema v9 Effect/Evidence migration requires an empty Effect/Evidence history; existing rows lack enforceable producer and append-only provenance".to_owned(),
        ));
    }
    Ok(())
}

/// The legacy schema records continuity state but not its owner decision or replay proof.
/// Do not manufacture provenance while migrating a previously reassigned host.
fn validate_legacy_channel_host_provenance(connection: &Connection) -> Result<(), StoreError> {
    let requires_repair: i64 = connection
        .query_row(
            "SELECT EXISTS (
               SELECT 1 FROM channel_host_assignments
               WHERE ingress_continuity = 'GAP_ACCEPTED' OR host_epoch > 1
             ) OR EXISTS (
               SELECT 1 FROM channel_host_assignments a
               WHERE NOT EXISTS (
                 SELECT 1 FROM channel_host_lease_records l
                 WHERE l.channel_binding_id = a.channel_binding_id
                   AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id
                   AND l.host_epoch = a.host_epoch
               )
             ) OR EXISTS (
               SELECT 1 FROM channel_host_assignments a
               JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
               WHERE l.workspace_id <> a.workspace_id OR l.runtime_id <> a.runtime_id
                 OR l.host_epoch <> a.host_epoch
             ) OR EXISTS (
               SELECT 1 FROM channel_host_assignments a
               JOIN channel_host_lease_records l ON l.channel_binding_id = a.channel_binding_id
                 AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id
                 AND l.host_epoch = a.host_epoch
               WHERE a.status = 'ACTIVE'
                 AND (julianday(l.lease_expires_at) IS NULL
                   OR julianday(l.lease_expires_at) <= julianday('now'))
             )",
            [],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if requires_repair != 0 {
        return Err(StoreError::CorruptSchema(
            "legacy ChannelHost assignments lack verifiable continuity or a live matching lease; repair before Runtime migration"
                .to_owned(),
        ));
    }
    Ok(())
}

/// The v4 table rebuild replaces composite foreign keys with role-aware admission
/// guards. Those guards do not inspect rows copied during migration, so validate their
/// existing scope relationships explicitly before the migration can commit.
fn validate_runtime_workspace_data(connection: &Connection) -> Result<(), StoreError> {
    let invalid: i64 = connection
        .query_row(
            "WITH invalid_runtime_scope AS (
               SELECT 1 FROM environments e
               WHERE NOT EXISTS (
                 SELECT 1 FROM runtime_workspace_bindings b
                 WHERE b.runtime_id = e.runtime_id AND b.workspace_id = e.owner_workspace_id
                   AND EXISTS (SELECT 1 FROM json_each(b.roles_json) r WHERE r.value = 'EXECUTOR')
                   AND (e.status IN ('FAILED', 'DESTROYED') OR b.status = 'ACTIVE')
               )
               UNION ALL
               SELECT 1 FROM channel_host_assignments a
               WHERE NOT EXISTS (
                 SELECT 1 FROM runtime_workspace_bindings b
                 WHERE b.runtime_id = a.runtime_id AND b.workspace_id = a.workspace_id
                   AND EXISTS (SELECT 1 FROM json_each(b.roles_json) r WHERE r.value = 'CHANNEL_HOST')
                   AND (b.status = 'ACTIVE' OR (
                     a.status <> 'ACTIVE' AND NOT EXISTS (
                       SELECT 1 FROM channel_host_lease_records l
                       WHERE l.channel_binding_id = a.channel_binding_id
                         AND l.workspace_id = a.workspace_id AND l.runtime_id = a.runtime_id
                         AND l.host_epoch = a.host_epoch
                         AND (julianday(l.lease_expires_at) > julianday('now')
                           OR julianday(l.lease_expires_at) IS NULL)
                     )
                   ))
               )
               UNION ALL
               SELECT 1 FROM automation_occurrences o
               WHERE NOT EXISTS (
                 SELECT 1 FROM runtime_workspace_bindings b
                 WHERE b.runtime_id = o.trigger_host_runtime_id AND b.workspace_id = o.workspace_id
                   AND EXISTS (SELECT 1 FROM json_each(b.roles_json) r WHERE r.value = 'TRIGGER_HOST')
                   AND (o.status IN ('COMPLETED', 'SKIPPED', 'FAILED') OR b.status = 'ACTIVE')
               )
               UNION ALL
               SELECT 1 FROM automation_cursors c
               JOIN automations a ON a.automation_id = c.automation_id AND a.workspace_id = c.workspace_id
               WHERE a.status = 'ENABLED' AND NOT EXISTS (
                 SELECT 1 FROM runtime_workspace_bindings b
                 WHERE b.runtime_id = c.trigger_host_runtime_id AND b.workspace_id = c.workspace_id
                   AND b.status = 'ACTIVE'
                   AND EXISTS (SELECT 1 FROM json_each(b.roles_json) r WHERE r.value = 'TRIGGER_HOST')
               )
               UNION ALL
               SELECT 1 FROM automation_occurrences o
               LEFT JOIN tasks t ON t.task_id = o.task_id
               WHERE o.task_id IS NOT NULL AND (
                 t.task_id IS NULL OR t.workspace_id IS NOT o.workspace_id
                 OR t.automation_id IS NOT o.automation_id
                   OR t.automation_occurrence_id IS NOT o.occurrence_id
               )
               UNION ALL
               SELECT 1 FROM channel_host_lease_records l
               WHERE l.control_version < 1 OR julianday(l.lease_expires_at) IS NULL
               UNION ALL
               SELECT 1 FROM automation_occurrences o
               WHERE o.claim_epoch < 0
                 OR (o.claim_expires_at IS NOT NULL AND julianday(o.claim_expires_at) IS NULL)
                 OR (o.status = 'PENDING' AND NOT (
                   (o.claim_epoch = 0 AND o.claim_expires_at IS NULL AND o.task_id IS NULL)
                   OR (o.claim_epoch >= 1 AND o.claim_expires_at IS NOT NULL
                     AND julianday(o.claim_expires_at) <= julianday('now') AND o.task_id IS NULL)
                 ))
                 OR (o.status = 'CLAIMED' AND (
                   o.claim_epoch < 1 OR o.claim_expires_at IS NULL OR o.task_id IS NOT NULL
                 ))
                 OR (o.status IN ('STARTED', 'WAITING_DEPENDENCY', 'COMPLETED', 'FAILED')
                   AND (o.claim_epoch < 1 OR o.claim_expires_at IS NULL OR o.task_id IS NULL))
                 OR (o.status = 'SKIPPED' AND NOT (
                   (o.claim_epoch = 0 AND o.claim_expires_at IS NULL AND o.task_id IS NULL)
                   OR (o.claim_epoch >= 1 AND o.claim_expires_at IS NOT NULL AND o.task_id IS NOT NULL)
                 ))
               UNION ALL
               SELECT 1 FROM tasks t
               WHERE t.automation_occurrence_id IS NOT NULL AND NOT EXISTS (
                 SELECT 1 FROM automation_occurrences o
                 WHERE o.occurrence_id = t.automation_occurrence_id
                   AND o.automation_id = t.automation_id AND o.workspace_id = t.workspace_id
                   AND o.task_id = t.task_id
               )
               UNION ALL
               SELECT 1 FROM workspaces w
               WHERE w.hub_runtime_id IS NOT NULL AND NOT EXISTS (
                 SELECT 1 FROM runtime_workspace_bindings b
                 WHERE b.runtime_id = w.hub_runtime_id AND b.workspace_id = w.workspace_id
                   AND b.status = 'ACTIVE' AND b.enrollment_mode = 'MESH_PAIRING'
                   AND EXISTS (SELECT 1 FROM json_each(b.roles_json) r WHERE r.value = 'WORKSPACE_HUB')
               )
             ) SELECT EXISTS(SELECT 1 FROM invalid_runtime_scope)",
            [],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if invalid != 0 {
        return Err(StoreError::CorruptSchema(
            "Runtime/Workspace scoped rows cannot be migrated without explicit binding repair"
                .to_owned(),
        ));
    }
    Ok(())
}

fn record_migration(
    connection: &Connection,
    version: i64,
    name: &str,
    source: &str,
) -> Result<(), StoreError> {
    let fingerprint = schema_fingerprint(connection)?;
    connection
        .execute(
            "INSERT INTO schema_migrations(version, name, source_checksum, schema_fingerprint, applied_at) VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![version, name, digest(source.as_bytes()), fingerprint],
        )
        .map_err(map_database_error)?;
    Ok(())
}

fn verify_migration_record(
    connection: &Connection,
    version: i64,
    expected_name: &str,
    source: &str,
    verify_fingerprint: bool,
) -> Result<(), StoreError> {
    let (name, source_checksum, stored_fingerprint): (String, String, String) = connection
        .query_row(
            "SELECT name, source_checksum, schema_fingerprint FROM schema_migrations WHERE version = ?1",
            [version],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or_else(|| StoreError::CorruptSchema("migration record is missing".to_owned()))?;
    if name != expected_name || source_checksum != digest(source.as_bytes()) {
        return Err(StoreError::CorruptSchema(
            "applied migration source checksum changed".to_owned(),
        ));
    }
    if verify_fingerprint && stored_fingerprint != schema_fingerprint(connection)? {
        return Err(StoreError::CorruptSchema(
            "database schema objects differ from the applied migration".to_owned(),
        ));
    }
    Ok(())
}

fn validate_schema(connection: &Connection) -> Result<(), StoreError> {
    let violations: Vec<String> = {
        let mut statement = connection
            .prepare("PRAGMA foreign_key_check")
            .map_err(map_database_error)?;
        let rows = statement
            .query_map([], |row| {
                let table: String = row.get(0)?;
                let rowid: Option<i64> = row.get(1)?;
                Ok(format!("{table}:{rowid:?}"))
            })
            .map_err(map_database_error)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?
    };
    if !violations.is_empty() {
        return Err(StoreError::CorruptSchema(format!(
            "foreign-key violations: {}",
            violations.join(", ")
        )));
    }
    Ok(())
}

fn schema_fingerprint(connection: &Connection) -> Result<String, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT type, name, tbl_name, COALESCE(sql, '') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(json!({
                "type": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "table": row.get::<_, String>(2)?,
                "sql": row.get::<_, String>(3)?,
            }))
        })
        .map_err(map_database_error)?;
    let objects = rows
        .collect::<Result<Vec<Value>, _>>()
        .map_err(map_database_error)?;
    Ok(digest(&canonical_json(&objects)?))
}

fn validate_commit(
    workspace: &Workspace,
    event: &EventDraft,
    expected_version: Option<u64>,
) -> Result<(), StoreError> {
    validate_workspace_event_identity(workspace, event)?;
    let next_version = match expected_version {
        Some(version) => version
            .checked_add(1)
            .ok_or_else(|| StoreError::Invalid("aggregate version overflow".to_owned()))?,
        None => 1,
    };
    if workspace.workspace_id != event.workspace_id || workspace.version != next_version {
        return Err(StoreError::Invalid(
            "aggregate and event identity/revision do not match".to_owned(),
        ));
    }
    if event.schema_version != 1 || !event.event_type.ends_with(".v1") {
        return Err(StoreError::Invalid(
            "unsupported domain event version".to_owned(),
        ));
    }
    Ok(())
}

fn validate_workspace_event_identity(
    workspace: &Workspace,
    event: &EventDraft,
) -> Result<(), StoreError> {
    if workspace.workspace_id != event.workspace_id
        || workspace.workspace_id != event.entity_id
        || event.entity_type != "Workspace"
        || workspace.version != event.entity_revision
    {
        return Err(StoreError::Invalid(
            "aggregate and event identity/revision do not match".to_owned(),
        ));
    }
    if event.schema_version != 1 || !event.event_type.ends_with(".v1") {
        return Err(StoreError::Invalid(
            "unsupported domain event version".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failpoint {
    DuringMigration,
    DuringV4Migration,
    DuringV5Migration,
    DuringV6Migration,
    DuringV7Migration,
    DuringV8Migration,
    DuringV9Migration,
    DuringV10Migration,
    DuringV11Migration,
    DuringV12Migration,
    DuringV13Migration,
    AfterAggregate,
    AfterSequence,
    AfterEvent,
    BeforeCommit,
}

fn commit_workspace_transaction(
    connection: &mut Connection,
    expected_version: Option<u64>,
    workspace: Workspace,
    draft: EventDraft,
    state_ref: AggregateStateRef,
    failpoint: Option<Failpoint>,
) -> Result<CommittedWorkspace, StoreError> {
    commit_workspace_transaction_with_dedup(
        connection,
        expected_version,
        workspace,
        draft,
        state_ref,
        None,
        failpoint,
    )
}

fn commit_workspace_transaction_with_dedup(
    connection: &mut Connection,
    expected_version: Option<u64>,
    workspace: Workspace,
    draft: EventDraft,
    state_ref: AggregateStateRef,
    request: Option<RequestDeduplication>,
    failpoint: Option<Failpoint>,
) -> Result<CommittedWorkspace, StoreError> {
    if state_ref.entity_revision != workspace.version || state_ref.record_schema_version != 1 {
        return Err(StoreError::Invalid(
            "aggregate state reference revision/schema mismatch".to_owned(),
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    if let Some(request) = request.as_ref() {
        let prior: Option<(String, Option<String>, Option<String>)> = transaction
            .query_row(
                "SELECT request_digest, response_json, response_digest FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
                params![request.principal_id, request.request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(map_database_error)?;
        if let Some((prior_digest, response_json, response_digest)) = prior {
            if prior_digest != request.request_digest {
                return Err(StoreError::Conflict {
                    expected: None,
                    actual: None,
                });
            }
            let response_json = response_json.ok_or_else(|| {
                StoreError::Integrity("idempotency receipt has no committed response".to_owned())
            })?;
            if response_digest.as_deref() != Some(digest(response_json.as_bytes()).as_str()) {
                return Err(StoreError::Integrity(
                    "idempotency response digest does not match".to_owned(),
                ));
            }
            return serde_json::from_str(&response_json)
                .map_err(|error| StoreError::Integrity(error.to_string()));
        }
    }
    let actual_version = transaction
        .query_row(
            "SELECT version FROM workspaces WHERE workspace_id = ?1",
            [&workspace.workspace_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(map_database_error)?
        .map(|version| from_sql_i64(version, "Workspace version"))
        .transpose()?;
    if actual_version != expected_version {
        return Err(StoreError::Conflict {
            expected: expected_version,
            actual: actual_version,
        });
    }
    validate_commit(&workspace, &draft, expected_version)?;

    match expected_version {
        None => {
            let version = to_sql_i64(workspace.version, "Workspace version")?;
            transaction
                .execute(
                    "INSERT INTO workspaces(workspace_id, name, owner_principal_id, replication_policy, current_instruction_revision, default_agent_binding_id, primary_coworker_id, hub_runtime_id, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        workspace.workspace_id,
                        workspace.name,
                        workspace.owner_principal_id,
                        workspace.replication_policy.as_str(),
                        workspace.current_instruction_revision.map(|value| to_sql_i64(value, "instruction revision")).transpose()?,
                        workspace.default_agent_binding_id,
                        workspace.primary_coworker_id,
                        workspace.hub_runtime_id,
                        workspace.status,
                        workspace.created_at,
                        workspace.updated_at,
                        version,
                    ],
                )
                .map_err(map_database_error)?;
        }
        Some(expected) => {
            let version = to_sql_i64(workspace.version, "Workspace version")?;
            let expected_sql = to_sql_i64(expected, "expected Workspace version")?;
            let affected = transaction
                .execute(
                    "UPDATE workspaces SET name = ?1, replication_policy = ?2, current_instruction_revision = ?3, default_agent_binding_id = ?4, primary_coworker_id = ?5, hub_runtime_id = ?6, status = ?7, updated_at = ?8, version = ?9 WHERE workspace_id = ?10 AND version = ?11",
                    params![
                        workspace.name,
                        workspace.replication_policy.as_str(),
                        workspace.current_instruction_revision.map(|value| to_sql_i64(value, "instruction revision")).transpose()?,
                        workspace.default_agent_binding_id,
                        workspace.primary_coworker_id,
                        workspace.hub_runtime_id,
                        workspace.status,
                        workspace.updated_at,
                        version,
                        workspace.workspace_id,
                        expected_sql,
                    ],
                )
                .map_err(map_database_error)?;
            if affected != 1 {
                return Err(StoreError::Conflict {
                    expected: Some(expected),
                    actual: None,
                });
            }
            transaction
                .execute(
                    "DELETE FROM workspace_replication_roots WHERE workspace_id = ?1",
                    [&workspace.workspace_id],
                )
                .map_err(map_database_error)?;
        }
    }
    fail_if(failpoint, Failpoint::AfterAggregate)?;

    for root_id in &workspace.replication_scope_root_ids {
        let active: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM workspace_roots WHERE workspace_id = ?1 AND workspace_root_id = ?2 AND status = 'ACTIVE'",
                params![workspace.workspace_id, root_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_database_error)?;
        if active.is_none() || workspace.replication_policy != ReplicationPolicy::SelectedFolders {
            return Err(StoreError::Invalid(
                "replication scope must reference active roots in SELECTED_FOLDERS mode".to_owned(),
            ));
        }
        transaction
            .execute(
                "INSERT INTO workspace_replication_roots(workspace_id, workspace_root_id) VALUES (?1, ?2)",
                params![workspace.workspace_id, root_id],
            )
            .map_err(map_database_error)?;
    }

    transaction
        .execute(
            "INSERT INTO workspace_origin_sequences(workspace_id, origin_runtime_id, last_sequence) VALUES (?1, ?2, 1) ON CONFLICT(workspace_id, origin_runtime_id) DO UPDATE SET last_sequence = last_sequence + 1",
            params![draft.workspace_id, draft.origin_runtime_id],
        )
        .map_err(map_database_error)?;
    let origin_sequence_sql: i64 = transaction
        .query_row(
            "SELECT last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 AND origin_runtime_id = ?2",
            params![draft.workspace_id, draft.origin_runtime_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let origin_sequence = from_sql_i64(origin_sequence_sql, "origin sequence")?;
    fail_if(failpoint, Failpoint::AfterSequence)?;

    let payload_json = String::from_utf8(canonical_json(&draft.payload)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let payload_digest = digest(payload_json.as_bytes());
    let state_ref_json = String::from_utf8(canonical_json(&state_ref)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))?;
    let event = DomainEvent {
        event_id: draft.event_id,
        workspace_id: draft.workspace_id,
        entity_type: draft.entity_type,
        entity_id: draft.entity_id,
        origin_runtime_id: draft.origin_runtime_id,
        origin_sequence,
        entity_revision: draft.entity_revision,
        hlc_timestamp: draft.hlc_timestamp,
        correlation_id: draft.correlation_id,
        causation_id: draft.causation_id,
        schema_version: draft.schema_version,
        event_type: draft.event_type,
        payload: draft.payload,
        aggregate_state_ref: state_ref,
        recorded_at: draft.recorded_at,
        payload_digest,
    };
    transaction
        .execute(
            "INSERT INTO domain_events(event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                event.event_id,
                event.workspace_id,
                event.entity_type,
                event.entity_id,
                event.origin_runtime_id,
                to_sql_i64(event.origin_sequence, "origin sequence")?,
                to_sql_i64(event.entity_revision, "event revision")?,
                event.hlc_timestamp,
                event.correlation_id,
                event.causation_id,
                i64::from(event.schema_version),
                event.event_type,
                payload_json,
                state_ref_json,
                event.recorded_at,
                event.payload_digest,
            ],
        )
        .map_err(map_database_error)?;
    fail_if(failpoint, Failpoint::AfterEvent)?;
    let committed = CommittedWorkspace { workspace, event };
    if let Some(request) = request {
        let response_json = String::from_utf8(canonical_json(&committed)?)
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
        let response_digest = digest(response_json.as_bytes());
        transaction
            .execute(
                "INSERT INTO request_dedup(principal_id, request_id, request_digest, response_json, response_digest, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                params![
                    request.principal_id,
                    request.request_id,
                    request.request_digest,
                    response_json,
                    response_digest,
                    request.created_at,
                ],
            )
            .map_err(map_database_error)?;
    }
    fail_if(failpoint, Failpoint::BeforeCommit)?;
    transaction.commit().map_err(map_database_error)?;

    Ok(committed)
}

fn fail_if(actual: Option<Failpoint>, at: Failpoint) -> Result<(), StoreError> {
    if actual == Some(at) {
        return Err(StoreError::Database(format!("injected {at:?}")));
    }
    Ok(())
}

type WorkspaceRow = (
    String,
    String,
    String,
    String,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    i64,
);

fn load_workspace(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Option<Workspace>, StoreError> {
    let base: Option<WorkspaceRow> = connection
        .query_row(
            "SELECT workspace_id, name, owner_principal_id, replication_policy, current_instruction_revision, default_agent_binding_id, primary_coworker_id, hub_runtime_id, status, created_at, updated_at, version FROM workspaces WHERE workspace_id = ?1",
            [workspace_id],
            |row| Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?,
                row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
            )),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        id,
        name,
        owner,
        policy,
        instruction_revision,
        default_agent,
        primary_coworker,
        hub_runtime,
        status,
        created_at,
        updated_at,
        version_sql,
    )) = base
    else {
        return Ok(None);
    };

    let roots = {
        let mut statement = connection
            .prepare("SELECT workspace_root_id FROM workspace_replication_roots WHERE workspace_id = ?1 ORDER BY workspace_root_id")
            .map_err(map_database_error)?;
        statement
            .query_map([workspace_id], |row| row.get::<_, String>(0))
            .map_err(map_database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?
    };

    Ok(Some(Workspace {
        workspace_id: id,
        name,
        owner_principal_id: owner,
        replication_policy: parse_policy(&policy)?,
        replication_scope_root_ids: roots,
        current_instruction_revision: instruction_revision
            .map(|value| from_sql_i64(value, "instruction revision"))
            .transpose()?,
        default_agent_binding_id: default_agent,
        primary_coworker_id: primary_coworker,
        hub_runtime_id: hub_runtime,
        status,
        created_at,
        updated_at,
        version: from_sql_i64(version_sql, "Workspace version")?,
    }))
}

fn list_workspaces(connection: &Connection) -> Result<Vec<Workspace>, StoreError> {
    let workspace_ids = {
        let mut statement = connection
            .prepare(
                "SELECT workspace_id FROM workspaces
                 ORDER BY created_at ASC, workspace_id ASC",
            )
            .map_err(map_database_error)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(map_database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_database_error)?
    };

    workspace_ids
        .into_iter()
        .map(|workspace_id| {
            load_workspace(connection, &workspace_id)?.ok_or_else(|| {
                StoreError::Integrity(
                    "Workspace disappeared during serialized catalog read".to_owned(),
                )
            })
        })
        .collect()
}

fn list_resources_page(
    connection: &Connection,
    workspace_id: &str,
    after_created_at: Option<&str>,
    after_resource_id: Option<&str>,
    limit: usize,
) -> Result<Vec<ResourceSummary>, StoreError> {
    if !(1..=101).contains(&limit) || after_created_at.is_some() != after_resource_id.is_some() {
        return Err(StoreError::Invalid(
            "Resource page query is invalid".to_owned(),
        ));
    }
    let mut statement = connection.prepare(
        "SELECT r.resource_id, r.workspace_id, rev.resource_revision_id, r.display_name,
                rev.media_type, rev.content_digest, rev.size_bytes, r.created_at
         FROM resources r
         JOIN resource_revisions rev
           ON rev.resource_id = r.resource_id AND rev.resource_revision_id = r.current_revision_id
         WHERE r.workspace_id = ?1
           AND rev.media_type IS NOT NULL AND rev.content_digest IS NOT NULL AND rev.size_bytes IS NOT NULL
           AND (?2 IS NULL OR r.created_at < ?2 OR (r.created_at = ?2 AND r.resource_id < ?3))
         ORDER BY r.created_at DESC, r.resource_id DESC LIMIT ?4",
    ).map_err(map_database_error)?;
    statement
        .query_map(
            params![
                workspace_id,
                after_created_at,
                after_resource_id,
                to_sql_i64(limit as u64, "Resource page limit")?,
            ],
            |row| {
                let size: i64 = row.get(6)?;
                Ok(ResourceSummary {
                    resource_id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    resource_revision_id: row.get(2)?,
                    display_name: row.get(3)?,
                    media_type: row.get(4)?,
                    content_digest: row.get(5)?,
                    size_bytes: u64::try_from(size)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, size))?,
                    created_at: row.get(7)?,
                })
            },
        )
        .map_err(map_database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)
}

fn search_resources_page(
    connection: &Connection,
    workspace_id: &str,
    query: Option<&str>,
    kind: Option<&str>,
    freshness: Option<&str>,
    after_created_at: Option<&str>,
    after_resource_id: Option<&str>,
    limit: usize,
) -> Result<Vec<ResourceSearchRecord>, StoreError> {
    if workspace_id.trim().is_empty()
        || query.is_some_and(|value| value.len() > 256 || value.contains('\0'))
        || kind.is_some_and(|value| value.len() > 64 || value.contains('\0'))
        || kind.is_some_and(|value| {
            !matches!(
                value,
                "FILE" | "FOLDER" | "ARTIFACT" | "CONNECTOR_OBJECT" | "WEB_RESOURCE" | "OTHER"
            )
        })
        || freshness
            .is_some_and(|value| !matches!(value, "CURRENT" | "STALE" | "UNKNOWN" | "UNAVAILABLE"))
        || !(1..=201).contains(&limit)
        || after_created_at.is_some() != after_resource_id.is_some()
    {
        return Err(StoreError::Invalid(
            "Resource search query is invalid".to_owned(),
        ));
    }
    let freshness_expr = "CASE
        WHEN l.availability IN ('OFFLINE', 'REVOKED', 'UNAVAILABLE') THEN 'UNAVAILABLE'
        WHEN l.availability IN ('PLACEHOLDER', 'UNKNOWN') THEN 'UNKNOWN'
        WHEN l.observed_revision_id = rev.resource_revision_id
         AND l.observed_digest = rev.content_digest THEN 'CURRENT'
        WHEN l.observed_revision_id IS NULL OR l.observed_digest IS NULL THEN 'UNKNOWN'
        ELSE 'STALE' END";
    let sql = format!(
        "SELECT r.resource_id, r.workspace_id, rev.resource_revision_id, r.display_name,
                rev.media_type, rev.content_digest, rev.size_bytes, r.created_at, r.kind,
                l.location_id, l.availability, l.writable, l.observed_revision_id,
                l.observed_digest, l.observed_at, l.last_checked_at, {freshness_expr}
         FROM resources r
         JOIN resource_revisions rev
           ON rev.resource_id = r.resource_id AND rev.resource_revision_id = r.current_revision_id
         JOIN resource_locations l
           ON l.location_id = (
                SELECT candidate.location_id FROM resource_locations candidate
                WHERE candidate.resource_id = r.resource_id
                  AND candidate.provider_ref = 'litecowork.encrypted_blob'
                ORDER BY (candidate.availability = 'AVAILABLE') DESC,
                         candidate.last_checked_at DESC, candidate.location_id ASC
                LIMIT 1
           )
         WHERE r.workspace_id = ?1
           AND (?2 IS NULL OR instr(lower(r.display_name), lower(?2)) > 0
                OR instr(lower(COALESCE(rev.media_type, '')), lower(?2)) > 0)
           AND (?3 IS NULL OR upper(r.kind) = upper(?3))
           AND (?4 IS NULL OR r.created_at < ?4 OR (r.created_at = ?4 AND r.resource_id < ?5))
           AND (?6 IS NULL OR ({freshness_expr}) = ?6)
         ORDER BY r.created_at DESC, r.resource_id DESC LIMIT ?7"
    );
    let mut statement = connection.prepare(&sql).map_err(map_database_error)?;
    statement
        .query_map(
            params![
                workspace_id,
                query,
                kind,
                after_created_at,
                after_resource_id,
                freshness,
                to_sql_i64(limit as u64, "Resource search page limit")?,
            ],
            |row| {
                let size: i64 = row.get(6)?;
                let display_name: String = row.get(3)?;
                let media_type: String = row.get(4)?;
                let mut match_reasons = Vec::new();
                if let Some(needle) = query {
                    if display_name
                        .to_ascii_lowercase()
                        .contains(&needle.to_ascii_lowercase())
                    {
                        match_reasons.push("NAME".to_owned());
                    }
                    if media_type
                        .to_ascii_lowercase()
                        .contains(&needle.to_ascii_lowercase())
                    {
                        match_reasons.push("MEDIA_TYPE".to_owned());
                    }
                }
                Ok(ResourceSearchRecord {
                    summary: ResourceSummary {
                        resource_id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        resource_revision_id: row.get(2)?,
                        display_name,
                        media_type,
                        content_digest: row.get(5)?,
                        size_bytes: u64::try_from(size)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, size))?,
                        created_at: row.get(7)?,
                    },
                    kind: row.get(8)?,
                    location_id: row.get(9)?,
                    availability: row.get(10)?,
                    writable: row.get::<_, i64>(11)? != 0,
                    observed_revision_id: row.get(12)?,
                    observed_digest: row.get(13)?,
                    observed_at: row.get(14)?,
                    last_checked_at: row.get(15)?,
                    freshness: row.get(16)?,
                    match_reasons,
                })
            },
        )
        .map_err(map_database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)
}

fn list_workspace_instruction_revisions(
    connection: &Connection,
    workspace_id: &str,
    after_revision: u64,
    limit: usize,
) -> Result<Vec<WorkspaceInstructionRevisionRecord>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT revision, parent_revisions_json, content_ref_json, content_digest, authored_by_json, created_at
             FROM workspace_instruction_revisions
             WHERE workspace_id = ?1 AND revision > ?2
             ORDER BY revision ASC
             LIMIT ?3",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![
                workspace_id,
                to_sql_i64(after_revision, "instruction cursor revision")?,
                to_sql_i64(limit as u64, "instruction page limit")?,
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .map_err(map_database_error)?;
    rows.map(|row| {
        let (revision, parents, content_ref, content_digest, authored_by, created_at) =
            row.map_err(map_database_error)?;
        Ok(WorkspaceInstructionRevisionRecord {
            workspace_id: workspace_id.to_owned(),
            revision: from_sql_i64(revision, "instruction revision")?,
            parent_revisions: serde_json::from_str(&parents)
                .map_err(|error| StoreError::Integrity(error.to_string()))?,
            content_ref: serde_json::from_str(&content_ref)
                .map_err(|error| StoreError::Integrity(error.to_string()))?,
            content_digest,
            authored_by: serde_json::from_str(&authored_by)
                .map_err(|error| StoreError::Integrity(error.to_string()))?,
            created_at,
        })
    })
    .collect()
}

fn read_resource_content_metadata(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
    revision_id: Option<&str>,
    maximum_bytes: Option<u64>,
) -> Result<Option<(ResourceSummary, BlobRef)>, StoreError> {
    let row = connection
        .query_row(
            "SELECT r.resource_id, r.workspace_id, rev.resource_revision_id, r.display_name,
                    rev.media_type, rev.content_digest, rev.size_bytes, r.created_at,
                    l.availability, r.context_document_json,
                    EXISTS(SELECT 1 FROM resource_locations managed
                           WHERE managed.resource_id = r.resource_id
                             AND managed.provider_ref = 'litecowork.encrypted_blob'
                             AND managed.runtime_id IS NULL
                             AND managed.environment_id IS NULL
                             AND managed.connection_id IS NULL),
                    EXISTS(SELECT 1 FROM resource_locations managed
                           WHERE managed.resource_id = r.resource_id
                             AND managed.provider_ref = 'litecowork.encrypted_blob'
                             AND managed.runtime_id IS NULL
                             AND managed.environment_id IS NULL
                             AND managed.connection_id IS NULL
                             AND managed.availability = 'AVAILABLE')
             FROM resources r
             JOIN resource_revisions rev
               ON rev.resource_id = r.resource_id
              AND rev.resource_revision_id = COALESCE(?3, r.current_revision_id)
             LEFT JOIN resource_locations l
               ON l.resource_id = r.resource_id
              AND l.provider_ref = 'litecowork.encrypted_blob'
              AND l.runtime_id IS NULL
              AND l.environment_id IS NULL
              AND l.connection_id IS NULL
             WHERE r.workspace_id = ?1 AND r.resource_id = ?2
             ORDER BY (l.availability = 'AVAILABLE') DESC, l.observed_at DESC LIMIT 1",
            params![workspace_id, resource_id, revision_id],
            |row| {
                let size: i64 = row.get(6)?;
                Ok((
                    ResourceSummary {
                        resource_id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        resource_revision_id: row.get(2)?,
                        display_name: row.get(3)?,
                        media_type: row.get(4)?,
                        content_digest: row.get(5)?,
                        size_bytes: u64::try_from(size)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, size))?,
                        created_at: row.get(7)?,
                    },
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, bool>(10)?,
                    row.get::<_, bool>(11)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((summary, availability, context_document, has_managed_location, managed_available)) =
        row
    else {
        return Ok(None);
    };
    ensure_context_document_content_readable(context_document.as_deref())?;
    if maximum_bytes.is_some_and(|maximum| summary.size_bytes > maximum) {
        return Err(StoreError::Invalid(
            "RESOURCE_READ_LIMIT_EXCEEDED".to_owned(),
        ));
    }
    if !has_managed_location {
        return Err(StoreError::Invalid("RESOURCE_CONTENT_EXTERNAL".to_owned()));
    }
    if !managed_available
        || availability
            .as_deref()
            .is_some_and(|value| value != "AVAILABLE")
    {
        return Err(StoreError::Invalid(
            "RESOURCE_LOCATION_UNAVAILABLE".to_owned(),
        ));
    }
    let blob = BlobRef {
        digest: summary.content_digest.clone(),
        size_bytes: summary.size_bytes,
        media_type: summary.media_type.clone(),
    };
    Ok(Some((summary, blob)))
}

/// An exact-revision content read is admitted only while a ContextDocument is ACTIVE. This check is
/// performed by the SQLite writer before the caller fetches/decrypts the referenced
/// BlobStore object. A read admitted while ACTIVE may finish if status changes afterward;
/// callers that publish derived state must recheck status in their final transaction.
fn ensure_context_document_content_readable(
    context_document_json: Option<&str>,
) -> Result<(), StoreError> {
    let Some(context_document_json) = context_document_json else {
        return Ok(());
    };
    let metadata: Value = serde_json::from_str(context_document_json).map_err(|error| {
        StoreError::Integrity(format!("ContextDocument metadata is invalid: {error}"))
    })?;
    let status = metadata
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            StoreError::Integrity("ContextDocument status is missing or invalid".to_owned())
        })?;
    match status {
        "ACTIVE" => Ok(()),
        "REVOKED" | "DELETION_PENDING" | "DELETED" => {
            match inactive_context_document_read_error(Some(status)) {
                Some(error) => Err(error),
                None => Err(StoreError::Integrity(
                    "ContextDocument status is unknown".to_owned(),
                )),
            }
        }
        _ => Err(StoreError::Integrity(
            "ContextDocument status is unknown".to_owned(),
        )),
    }
}

fn inactive_context_document_read_error(status: Option<&str>) -> Option<StoreError> {
    match status {
        Some("REVOKED") => Some(StoreError::Invalid("CONTEXT_DOCUMENT_REVOKED".to_owned())),
        Some("DELETION_PENDING") => Some(StoreError::Invalid(
            "CONTEXT_DOCUMENT_DELETION_PENDING".to_owned(),
        )),
        Some("DELETED") => Some(StoreError::Invalid("CONTEXT_DOCUMENT_DELETED".to_owned())),
        _ => None,
    }
}

fn prefer_context_document_status_error_after_read_failure(
    blob_error: StoreError,
    status_read: Result<Option<String>, StoreError>,
) -> StoreError {
    match status_read {
        Ok(status) => inactive_context_document_read_error(status.as_deref()).unwrap_or(blob_error),
        Err(_) => blob_error,
    }
}

fn parse_policy(value: &str) -> Result<ReplicationPolicy, StoreError> {
    match value {
        "LOCAL_ONLY" => Ok(ReplicationPolicy::LocalOnly),
        "METADATA_ONLY" => Ok(ReplicationPolicy::MetadataOnly),
        "ACTIVE_TASK_INPUTS" => Ok(ReplicationPolicy::ActiveTaskInputs),
        "SELECTED_FOLDERS" => Ok(ReplicationPolicy::SelectedFolders),
        "FULL_WORKSPACE" => Ok(ReplicationPolicy::FullWorkspace),
        _ => Err(StoreError::Integrity(
            "unknown stored replication policy".to_owned(),
        )),
    }
}

fn read_workspace_events(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<DomainEvent>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT event_id, workspace_id, entity_type, entity_id, origin_runtime_id, origin_sequence, entity_revision, hlc_timestamp, correlation_id, causation_id, schema_version, type, payload_json, aggregate_state_ref_json, recorded_at, payload_digest FROM domain_events WHERE workspace_id = ?1 ORDER BY hlc_timestamp, origin_runtime_id, origin_sequence, event_id",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map([workspace_id], |row| {
            let payload_json: String = row.get(12)?;
            let state_ref_json: String = row.get(13)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
                payload_json,
                state_ref_json,
                row.get::<_, String>(14)?,
                row.get::<_, String>(15)?,
            ))
        })
        .map_err(map_database_error)?;
    let mut events = Vec::new();
    for row in rows {
        let row = row.map_err(map_database_error)?;
        events.push(DomainEvent {
            event_id: row.0,
            workspace_id: row.1,
            entity_type: row.2,
            entity_id: row.3,
            origin_runtime_id: row.4,
            origin_sequence: from_sql_i64(row.5, "origin sequence")?,
            entity_revision: from_sql_i64(row.6, "event revision")?,
            hlc_timestamp: row.7,
            correlation_id: row.8,
            causation_id: row.9,
            schema_version: u32::try_from(row.10)
                .map_err(|_| StoreError::Integrity("event schema version is invalid".to_owned()))?,
            event_type: row.11,
            payload: serde_json::from_str(&row.12)
                .map_err(|error| StoreError::Integrity(error.to_string()))?,
            aggregate_state_ref: serde_json::from_str(&row.13)
                .map_err(|error| StoreError::Integrity(error.to_string()))?,
            recorded_at: row.14,
            payload_digest: row.15,
        });
    }
    Ok(events)
}

fn canonical_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    let value = serde_json::to_value(value)
        .map_err(|error| StoreError::Invalid(format!("value is not valid JSON: {error}")))?;
    validate_jcs_numbers(&value)?;
    serde_json_canonicalizer::to_vec(&value)
        .map_err(|error| StoreError::Invalid(format!("value is not canonical JSON: {error}")))
}

fn validate_jcs_numbers(value: &Value) -> Result<(), StoreError> {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    match value {
        Value::Number(number) if number.is_u64() => {
            if number
                .as_u64()
                .is_some_and(|integer| integer > MAX_SAFE_INTEGER)
            {
                return Err(StoreError::Invalid(
                    "integer exceeds the exact RFC 8785/I-JSON range; encode it as a string"
                        .to_owned(),
                ));
            }
        }
        Value::Number(number) if number.is_i64() => {
            if number
                .as_i64()
                .is_some_and(|integer| integer.unsigned_abs() > MAX_SAFE_INTEGER)
            {
                return Err(StoreError::Invalid(
                    "integer exceeds the exact RFC 8785/I-JSON range; encode it as a string"
                        .to_owned(),
                ));
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_jcs_numbers(item)?;
            }
        }
        Value::Object(fields) => {
            for item in fields.values() {
                validate_jcs_numbers(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn to_sql_i64(value: u64, field: &str) -> Result<i64, StoreError> {
    i64::try_from(value)
        .map_err(|_| StoreError::Invalid(format!("{field} exceeds SQLite INTEGER range")))
}

fn from_sql_i64(value: i64, field: &str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::Integrity(format!("stored {field} is negative")))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn map_database_error(error: rusqlite::Error) -> StoreError {
    if let rusqlite::Error::SqliteFailure(code, _) = &error {
        match code.code {
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => return StoreError::Busy,
            ErrorCode::DiskFull => return StoreError::Io("database disk is full".to_owned()),
            ErrorCode::ConstraintViolation => {
                return StoreError::Database("database constraint rejected the write".to_owned());
            }
            _ => {}
        }
    }
    StoreError::Database(error.to_string())
}

#[cfg(test)]
mod tests;
