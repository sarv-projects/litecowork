use super::StoreError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootObservationScanStatus {
    Complete,
    Partial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootObservationFreshness {
    Current,
    Stale,
    Unknown,
}

impl RootObservationFreshness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "CURRENT",
            Self::Stale => "STALE",
            Self::Unknown => "UNKNOWN",
        }
    }
}

pub struct WorkspaceRootFileObservation {
    /// Runtime-keyed root-relative membership pseudonym, never a path.
    pub observation_id: String,
    /// Runtime-keyed stable file identity pseudonym, never the raw OS identity tuple.
    pub file_identity_digest: String,
    /// Null until a canonical Resource aggregate is atomically linked.
    pub resource_id: Option<String>,
    /// Safe basename for deterministic search.
    pub display_name: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub observed_at: String,
    /// Runtime-local relative locator. Never serialize or expose this field.
    pub private_relative_locator: Vec<u8>,
}

impl std::fmt::Debug for WorkspaceRootFileObservation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkspaceRootFileObservation")
            .field("observation_id", &self.observation_id)
            .field("file_identity_digest", &self.file_identity_digest)
            .field("resource_id", &self.resource_id)
            .field("display_name", &self.display_name)
            .field("size_bytes", &self.size_bytes)
            .field("modified_at", &self.modified_at)
            .field("observed_at", &self.observed_at)
            .field("private_relative_locator", &"<private>")
            .finish()
    }
}

pub struct WorkspaceRootObservationBatch {
    pub workspace_id: String,
    pub workspace_root_id: String,
    pub expected_root_version: u64,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub request_id: String,
    pub request_digest: String,
    pub status: RootObservationScanStatus,
    pub observations: Vec<WorkspaceRootFileObservation>,
    pub completed_at: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootObservationCommitSummary {
    pub complete_generation: u64,
    pub observations_upserted: u32,
    pub observations_marked_stale: u32,
}

pub struct RootObservationSearchRequest {
    pub owner_principal_id: String,
    pub workspace_id: String,
    pub workspace_root_id: String,
    pub expected_root_version: u64,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub query: String,
    pub freshness: Option<RootObservationFreshness>,
    pub after_display_name: Option<String>,
    pub after_observation_id: Option<String>,
    pub limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootObservationSearchRecord {
    pub observation_id: String,
    pub file_identity_digest: String,
    pub resource_id: Option<String>,
    pub display_name: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub observed_at: String,
    pub freshness: RootObservationFreshness,
}

pub trait WorkspaceRootObservationStore: Send + Sync {
    /// Returns at most `limit + 1` records so the caller can determine whether to
    /// emit an opaque keyset cursor. The store never returns a private locator.
    fn search_workspace_root_observations(
        &self,
        request: RootObservationSearchRequest,
    ) -> Result<Vec<RootObservationSearchRecord>, StoreError>;
}
