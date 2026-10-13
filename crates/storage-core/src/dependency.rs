use crate::StoreError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyEdgeRecord {
    pub dependency_edge_id: String,
    pub workspace_id: String,
    pub source_resource_id: String,
    pub source_revision_id: String,
    pub dependent_kind: String,
    pub dependent_ref: String,
    pub created_at: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DependencyFreshness {
    Current,
    Stale,
    Conflicted,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceDependencyViewRecord {
    pub dependency_edge: DependencyEdgeRecord,
    pub dependent_freshness: DependencyFreshness,
    pub latest_invalidation: Option<crate::InvalidationRecord>,
}

/// Owner, Workspace, and source-Resource scoped keyset page request. The cursor is the
/// last `(created_at, dependency_edge_id)` position from the preceding page.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceDependencyPageRequest {
    pub owner_principal_id: String,
    pub workspace_id: String,
    pub resource_id: String,
    pub after_created_at: Option<String>,
    pub after_dependency_edge_id: Option<String>,
    pub limit: usize,
}

/// Bounded reverse-dependency reads over immutable ArtifactVersion and VerificationRun
/// inputs. `None` hides a missing or out-of-scope source Resource; an existing Resource
/// with no dependent edges returns `Some(Vec::new())`.
pub trait ResourceDependencyReadStore: Send + Sync {
    fn list_resource_dependents_page(
        &self,
        request: ResourceDependencyPageRequest,
    ) -> Result<Option<Vec<ResourceDependencyViewRecord>>, StoreError>;
}
