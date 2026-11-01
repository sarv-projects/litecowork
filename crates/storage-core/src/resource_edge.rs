use crate::{
    CommittedResource, DomainEvent, EventDraft, ResourceRecord, ResourceRevisionRecord,
    WorkspaceCreateRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Immutable same-Workspace structural/provenance relation between Resource revisions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceEdgeRecord {
    pub edge_id: String,
    pub workspace_id: String,
    pub from_resource_id: String,
    pub from_revision_id: Option<String>,
    pub relation: String,
    pub to_resource_id: String,
    pub to_revision_id: Option<String>,
    pub observed_at: String,
    pub provenance: Value,
}

/// A child Resource and its provenance edge, committed with both events and one request
/// receipt. The target must be the newly created Resource; source and target revision
/// references are validated against canonical same-Workspace rows in storage.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedResourceCreateCommit {
    pub request: WorkspaceCreateRequest,
    pub resource: ResourceRecord,
    pub revision: ResourceRevisionRecord,
    pub resource_event: EventDraft,
    pub edge: ResourceEdgeRecord,
    pub edge_event: EventDraft,
    pub content: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedDerivedResource {
    pub resource: CommittedResource,
    pub edge: ResourceEdgeRecord,
    pub edge_event: DomainEvent,
}
