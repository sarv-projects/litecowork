use crate::{
    BlobRef, DomainEvent, EventDraft, ResourceRecord, ResourceRevisionRecord, StoreError,
    WorkspaceCreateRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ArtifactRecord {
    pub artifact_id: String,
    pub workspace_id: String,
    pub resource_id: String,
    pub task_id: Option<String>,
    pub kind: String,
    pub display_name: String,
    pub current_version: u64,
    pub library_status: String,
    pub created_at: String,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ArtifactAppendHeads {
    pub artifact: ArtifactRecord,
    pub resource: ResourceRecord,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum ArtifactContentRecord {
    #[serde(rename = "MANAGED_BLOB")]
    ManagedBlob {
        storage_ref: BlobRef,
        content_digest: String,
        media_type: String,
        size_bytes: u64,
    },
    #[serde(rename = "EXTERNAL_RESOURCE")]
    ExternalResource {
        resource_ref: Value,
        provider_revision: Option<String>,
        observed_digest: Option<String>,
        observed_at: String,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ArtifactVersionRecord {
    pub artifact_id: String,
    pub version: u64,
    pub resource_revision_id: String,
    pub input_refs: Vec<Value>,
    pub content: ArtifactContentRecord,
    pub provenance: Value,
    pub created_by_attempt: Option<String>,
    pub verification_refs: Vec<String>,
    pub created_at: String,
}

/// Read boundary over already committed Artifact records.
pub trait ArtifactReadStore: Send + Sync {
    fn get_artifact(&self, workspace_id: &str, artifact_id: &str) -> Result<Option<ArtifactRecord>, StoreError>;
    /// Returns the Artifact and backing Resource heads from one storage read turn so
    /// an append command can pin the exact immutable ResourceRevision parent.
    fn get_artifact_append_heads(&self, workspace_id: &str, artifact_id: &str) -> Result<Option<ArtifactAppendHeads>, StoreError>;
    fn list_artifacts_page(
        &self, workspace_id: &str, library_status: Option<&str>, task_id: Option<&str>,
        after_created_at: Option<&str>, after_artifact_id: Option<&str>, limit: usize,
    ) -> Result<Vec<ArtifactRecord>, StoreError>;
    fn get_artifact_version(&self, workspace_id: &str, artifact_id: &str, version: u64) -> Result<Option<ArtifactVersionRecord>, StoreError>;
    fn read_artifact_content_bounded(&self, workspace_id: &str, artifact_id: &str, version: u64, maximum_bytes: u64) -> Result<Option<Vec<u8>>, StoreError>;
}

/// Prepared immutable append for an Artifact and its backing ARTIFACT Resource.
/// The BlobRef in `version.content` must already refer to committed, digest-verified
/// BlobStore bytes. The storage adapter must still verify that blob before opening the
/// database transaction and recheck every aggregate/head precondition inside the same
/// transaction that writes both versions and events.
#[derive(Clone, Debug)]
pub struct ArtifactVersionAppendCommit {
    /// Owner identity and RequestId used for command deduplication. `request_payload`
    /// must fingerprint all selected heads, content digest, provenance, and author data.
    pub request: WorkspaceCreateRequest,
    pub workspace_id: String,
    pub artifact_id: String,
    /// Aggregate version from `artifacts.version` (If-Match).
    pub expected_artifact_version: u64,
    /// Current content version. Distinct from the aggregate version because Library
    /// promotion/archive also advance the aggregate without changing content.
    pub expected_content_version: u64,
    pub expected_resource_version: u64,
    /// Exact current Resource head selected by the editor; the new revision must name
    /// this as its sole parent for an ordinary text edit.
    pub expected_parent_resource_revision_id: String,
    pub artifact: ArtifactRecord,
    pub version: ArtifactVersionRecord,
    pub resource: ResourceRecord,
    pub resource_revision: ResourceRevisionRecord,
    pub resource_event: EventDraft,
    pub artifact_event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommittedArtifactVersionAppend {
    pub artifact: ArtifactRecord,
    pub version: ArtifactVersionRecord,
    pub resource: ResourceRecord,
    pub resource_revision: ResourceRevisionRecord,
    /// Ordered as Resource revision observed, then Artifact version created.
    pub events: [DomainEvent; 2],
    pub replayed: bool,
}

/// Write boundary for immutable Artifact publication. A conforming adapter must:
/// verify committed BlobStore content before transaction entry; re-read current Artifact
/// and Resource heads and compare all expected versions/revision IDs; reject ARCHIVED
/// Artifacts; validate same-Workspace Artifact/Resource/input references and provenance;
/// append Resource ancestry and dependency edges; append immutable ArtifactVersion and
/// ResourceRevision rows; move the Resource head before Artifact.current_version; write
/// both aggregate snapshots and events plus the principal-scoped idempotency receipt;
/// and commit those changes atomically. A duplicate RequestId with a different payload
/// conflicts. No implementation may overwrite or delete an earlier version.
///
/// The SQLite adapter implements this bounded append contract for managed `text/plain`
/// blobs up to 1 MiB. The desktop Operator may expose only the corresponding authenticated
/// plain-text publication command; other media types and provider-backed Artifacts remain
/// read-only.
pub trait ArtifactVersionWriteStore: Send + Sync {
    /// Resolves an immutable append receipt before any fresh-head or staging check.
    /// `request_payload` must be the canonical logical payload that append would use.
    /// This permits an identical retry to return its original receipt after later
    /// Artifact/Workspace archival, while storage still authenticates the owner.
    fn resolve_artifact_version_append_replay(
        &self,
        principal_id: &str,
        workspace_id: &str,
        request_id: &str,
        request_payload: &Value,
    ) -> Result<Option<CommittedArtifactVersionAppend>, StoreError>;

    /// Stores and verifies bounded user-authored UTF-8 text under the Artifact blob
    /// purpose after checking Workspace ownership and Artifact eligibility. Exact head
    /// freshness is checked by append so a stale identical retry can resolve its receipt.
    /// This creates no ArtifactVersion; callers must immediately submit the returned
    /// BlobRef to `append_artifact_version`. Unreferenced content is eligible for
    /// content-addressed blob cleanup.
    fn stage_text_content(
        &self,
        principal_id: &str,
        workspace_id: &str,
        artifact_id: &str,
        bytes: &[u8],
    ) -> Result<BlobRef, StoreError>;

    fn append_artifact_version(
        &self,
        commit: ArtifactVersionAppendCommit,
    ) -> Result<CommittedArtifactVersionAppend, StoreError>;
}
