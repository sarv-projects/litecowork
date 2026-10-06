use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

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
    Checkpoint,
}

impl BlobPurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AggregateState => "AGGREGATE_STATE",
            Self::Artifact => "ARTIFACT",
            Self::Resource => "RESOURCE",
            Self::Checkpoint => "CHECKPOINT",
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

    fn verify(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<(), StoreError> {
        self.get(workspace_id, purpose, blob).map(|_| ())
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
            Self::Blob(message) => write!(f, "blob operation failed: {message}"),
            Self::Io(message) => write!(f, "storage I/O failed: {message}"),
            Self::Database(message) => write!(f, "database operation failed: {message}"),
            Self::ExecutorStopped => f.write_str("storage executor stopped"),
        }
    }
}

impl std::error::Error for StoreError {}
