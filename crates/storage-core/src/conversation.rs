use crate::{DomainEvent, EventDraft, StoreError, WorkspaceCreateRequest};
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
