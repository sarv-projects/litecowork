//! Immutable, optional enhancements of an already committed semantic message.
use crate::{BlobRef, DomainEvent, EventDraft, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_RICH_PRESENTATION_BYTES: u64 = 1_048_576;
pub const RICH_PRESENTATION_MEDIA_TYPE: &str = "application/vnd.litecowork.rich-presentation+json";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RichPresentationRecord {
    pub presentation_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub message_id: String,
    pub schema_version: u32,
    pub renderer_contract_version: u32,
    pub semantic_content_digest: String,
    pub document_ref: BlobRef,
    pub document_digest: String,
    pub document_size_bytes: u64,
    pub producer_agent_session_id: Option<String>,
    pub host_instruction_digest: Option<String>,
    pub host_skill_refs: Vec<Value>,
    pub created_at: String,
    pub version: u64,
}

/// The publisher supplies canonical bytes and metadata; storage validates them against
/// the committed semantic message before independently appending presentation state.
/// This trusted application port is called only by the Core publisher. The publisher
/// allocates `presentation_id` and EventId; neither is copied from agent intent or
/// accepted as a client-controlled identifier. Identity collisions return an opaque
/// unavailable result with no other Workspace or aggregate revision information.
#[derive(Clone, Debug)]
pub struct PublishRichPresentation {
    pub principal_id: String,
    pub presentation: RichPresentationRecord,
    pub canonical_document: Vec<u8>,
    pub event: EventDraft,
}

#[derive(Clone, Debug)]
pub struct CommittedRichPresentation {
    pub presentation: RichPresentationRecord,
    pub event: DomainEvent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredRichPresentation {
    pub presentation: RichPresentationRecord,
    pub canonical_document: Vec<u8>,
}

/// This initial qualified adapter accepts semantic TEXT_SLICE, LAYOUT and DIVIDER
/// blocks only. All source-bearing/host-bound blocks require future owning binders.
/// Read failure never modifies or makes the underlying semantic message unavailable.
pub trait RichPresentationStore: Send + Sync {
    fn publish_rich_presentation(
        &self,
        request: PublishRichPresentation,
    ) -> Result<CommittedRichPresentation, StoreError>;
    fn read_rich_presentation(
        &self,
        principal_id: &str,
        workspace_id: &str,
        presentation_id: &str,
    ) -> Result<Option<StoredRichPresentation>, StoreError>;
}
