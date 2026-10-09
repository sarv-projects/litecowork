use crate::{DomainEvent, EventDraft, StoreError, WorkspaceCreateRequest};
use domain_conversation::{ConversationTurn, TurnCommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

    /// Only lifecycle changes with a canonical event in EVENTS.md are accepted here.
    /// Native session admission and the OPEN -> RUNNING start command remain closed.
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
        || record.active_agent_binding_id.is_some()
        || record.version != 1
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
