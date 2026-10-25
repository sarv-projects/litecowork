use crate::{DomainEvent, EventDraft, StoreError, WorkspaceCreateRequest};
pub use domain_conversation::{
    ConversationTurn, ConversationTurnStatus, PresentationPreference, TurnCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stable reason code for a Conversation turn whose native AgentSession admission is
/// unavailable. The storage transition gate is the only authority that returns it; an
/// Operator route must not treat any other failure as an unavailable admission.
pub const CONVERSATION_SESSION_ADMISSION_UNAVAILABLE: &str =
    "CONVERSATION_SESSION_ADMISSION_UNAVAILABLE";

/// A concurrent turn commit moved the Conversation aggregate version between the
/// caller's read and its SQLite transaction. The caller must re-read and retry; the
/// write transaction rolls back.
pub const CONVERSATION_TURN_EVENT_REVISION_MISMATCH: &str =
    "ConversationTurn event revision mismatch";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationRecord {
    pub conversation_id: String,
    pub workspace_id: String,
    pub title: Option<String>,
    pub active_agent_binding_id: Option<String>,
    pub version: u64,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct CreateConversationCommit {
    pub request: WorkspaceCreateRequest,
    pub conversation: ConversationRecord,
    pub event: EventDraft,
}

/// Version-fenced Conversation AgentBinding override mutation. A null binding clears
/// the override and restores Workspace-default resolution for future turns.
#[derive(Clone, Debug)]
pub struct SetConversationAgentBinding {
    pub request: WorkspaceCreateRequest,
    pub principal_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub expected_version: u64,
    pub agent_binding_id: Option<String>,
    pub event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedConversation {
    pub conversation: ConversationRecord,
    pub event: DomainEvent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ConversationPage {
    pub items: Vec<ConversationRecord>,
    pub next_cursor: Option<String>,
}

pub trait ConversationStore: Send + Sync {
    fn create_conversation(
        &self,
        commit: CreateConversationCommit,
    ) -> Result<CommittedConversation, StoreError>;

    fn set_conversation_agent_binding_idempotent(
        &self,
        commit: SetConversationAgentBinding,
    ) -> Result<CommittedConversation, StoreError>;

    fn get_conversation(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationRecord>, StoreError>;

    fn list_conversations(
        &self,
        principal_id: &str,
        workspace_id: &str,
        after_created_at: Option<&str>,
        after_conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ConversationRecord>, StoreError>;
}

/// A USER message committed with its OPEN turn. JSON values retain the public
/// PrincipalRef, MessageContentBlock, ResourceRef, and channel reference shapes.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationMessageRecord {
    pub message_id: String,
    pub conversation_id: String,
    pub author: Value,
    pub role: String,
    pub agent_session_id: Option<String>,
    pub agent_binding_id: Option<String>,
    pub turn_id: Option<String>,
    pub content: Value,
    pub resource_refs: Value,
    pub source_channel_ref: Option<Value>,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct CreateConversationTurnCommit {
    pub request: WorkspaceCreateRequest,
    pub principal_id: String,
    pub workspace_id: String,
    pub message: ConversationMessageRecord,
    pub turn: ConversationTurn,
    pub message_event: EventDraft,
    pub turn_event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedConversationTurn {
    pub message: ConversationMessageRecord,
    pub turn: ConversationTurn,
    pub events: Vec<DomainEvent>,
}

pub trait ConversationTurnStore: Send + Sync {
    fn create_conversation_turn(
        &self,
        commit: CreateConversationTurnCommit,
    ) -> Result<CommittedConversationTurn, StoreError>;

    /// Resolves the binding this Conversation selected plus the endpoint and Runtime
    /// incarnation currently eligible to host its session.
    ///
    /// Returns `Ok(None)` when no eligible host exists, which the caller must report as
    /// `AGENT_UNAVAILABLE` rather than as an empty selection.
    fn resolve_conversation_session_host(
        &self,
        workspace_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ResolvedConversationHost>, StoreError>;

    /// Only lifecycle changes with a canonical event in EVENTS.md are accepted here.
    /// OPEN -> RUNNING is rejected until a supervisor-owned activation operation can
    /// persist observed native-session readiness atomically with the turn transition.
    #[allow(clippy::too_many_arguments)]
    fn transition_conversation_turn(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        expected_version: u64,
        command: TurnCommand,
        event: EventDraft,
    ) -> Result<ConversationTurn, StoreError>;
}

/// The live host a Conversation-scoped session must use, resolved authoritatively.
///
/// Resolution is observational: an Operator must not treat this inventory result as
/// adapter startup or session readiness. A future supervisor-owned reservation and
/// activation flow must revalidate the exact host and incarnation after native startup.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedConversationHost {
    pub agent_binding_id: String,
    pub endpoint_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
}


pub fn validate_conversation_record(record: &ConversationRecord) -> Result<(), StoreError> {
    let valid = |value: &str| {
        !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
    };
    if !valid(&record.conversation_id)
        || !valid(&record.workspace_id)
        || record.title.as_ref().is_some_and(|title| {
            title.trim().is_empty()
                || title.chars().count() > 160
                || title.chars().any(char::is_control)
        })
        || record.active_agent_binding_id.as_ref().is_some_and(|binding_id| !valid(binding_id))
        || record.version == 0
        || record.version > i64::MAX as u64
    {
        return Err(StoreError::Invalid(
            "Conversation record is invalid".to_owned(),
        ));
    }
    Ok(())
}

pub fn request_payload_value(workspace_id: &str, title: Option<&str>) -> Value {
    serde_json::json!({"operation":"conversation.create.v1","workspace_id":workspace_id,"title":title})
}
