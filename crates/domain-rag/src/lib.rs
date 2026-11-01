//! Provider-agnostic retrieval, grounding, index-state, local-model, and placement
//! rules for LiteCowork Core.
//!
//! LiteCowork Core owns Resource scope, revision, provenance, authorization, and
//! deletion. Parsing and semantic retrieval are provider implementations behind the
//! [`provider::RetrievalProvider`] boundary. This crate defines that boundary, the
//! grounding rules every provider result must satisfy, the owner-visible index state,
//! the local model inventory rules, and the explicit local/cloud placement rules.
//!
//! The crate deliberately contains no endpoint, transport, embedding API, or third
//! party wire protocol. A provider that is absent, unconfigured, offline, or serving an
//! incompatible index generation fails closed.

pub mod grounding;
pub mod index_state;
pub mod limits;
pub mod local_lexical;
pub mod local_model;
pub mod placement;
pub mod provider;
pub mod scope;

pub use grounding::{
    GroundedClaim, GroundedRetrieval, GroundedStatus, GroundingError, InsufficientEvidenceReason,
    RetrievalCitation, RetrievalWarning, SegmentLocator, TextSpan, detect_embedded_instructions,
    validate_claims,
};
pub use index_state::{
    ResourceIndexFacts, ResourceIndexState, ResourceIndexStateView, resolve_index_state,
};
pub use limits::{
    MAX_EXCERPT_BYTES, MAX_QUERY_BYTES, MAX_RESULTS, MAX_TOTAL_EXCERPT_BYTES, RetrievalBudget,
    RetrievalBudgetError,
};
pub use local_lexical::{LOCAL_LEXICAL_PROVIDER_VERSION, LocalLexicalRetrievalProvider};
pub use local_model::{
    AgentModuleCompatibility, LOCAL_MODEL_AGENT_POLICY, LOCAL_MODEL_CAPABILITY_ID,
    LOCAL_MODEL_SURFACE, LocalModelAvailability, LocalModelInventoryEntry, LocalModelState,
    agent_module_compatible, resolve_local_model_state,
};
pub use placement::{
    PlacementBlocker, PlacementDecision, PlacementInputs, ResourcePlacement, TaskPlacement,
    WorkspaceReplicationPolicy, evaluate_task_placement, resolve_resource_placement,
};
pub use provider::{
    LOCAL_LEXICAL_PARSER_ID, LOCAL_LEXICAL_PROVIDER_ID, ProviderAvailability, ProviderPlacement,
    ProviderStatus, RetrievalError, RetrievalMode, RetrievalPage, RetrievalProvider,
    RetrievalRequest,
};
pub use scope::{AuthorizedResourceRef, RetrievalScope};
