use crate::{DomainEvent, EventDraft, StoreError, WorkspaceCreateRequest};
pub use domain_conversation::{
    ConversationTurn, ConversationTurnStatus, PresentationPreference, TurnCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONVERSATION_CONTEXT_MAX_MESSAGES: usize = 1_024;
pub const CONVERSATION_CONTEXT_MAX_BYTES: usize = 4 * 1024 * 1024;

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
    /// Reads an exact committed Conversation-turn request receipt before mutable admission
    /// state (such as the currently selected AgentBinding) is consulted. This preserves
    /// idempotent replay when configuration changes after the original commit.
    fn replay_conversation_turn(
        &self,
        principal_id: &str,
        request_id: &str,
        request_payload: &Value,
    ) -> Result<Option<CommittedConversationTurn>, StoreError>;

    fn create_conversation_turn(
        &self,
        commit: CreateConversationTurnCommit,
    ) -> Result<CommittedConversationTurn, StoreError>;

    /// Reads a stable prefix of the canonical Conversation transcript through the
    /// submitted user message. The target binding comes from that turn's immutable
    /// creation event; provider-private session identifiers are stripped.
    fn load_conversation_context_for_turn(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
    ) -> Result<ConversationContextPackage, StoreError>;

    /// Resolves the binding pinned to this committed turn from its unique immutable
    /// `conversation.turn.created.v2` event, plus the endpoint and Runtime incarnation
    /// currently eligible to host it.
    ///
    /// A Conversation selector change applies only to later turns. Resolution is
    /// observational and never falls back to the current selector or Workspace default.
    /// Returns `Ok(None)` when the pinned host is no longer eligible.
    fn resolve_conversation_session_host_for_turn(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
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



/// One durable message projected into the canonical Conversation context sent to an
/// agent. Native session identifiers are intentionally excluded.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationContextMessage {
    pub message_id: String,
    pub role: String,
    pub turn_id: Option<String>,
    pub source_agent_binding_id: Option<String>,
    pub content: Value,
    pub resource_refs: Value,
    pub created_at: String,
}

/// A stable prefix of the user-visible Conversation, scoped for one selected agent.
/// The target agent starts or resumes its own private session and receives this
/// canonical transcript; the previous agent's opaque session handle is never copied.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationContextPackage {
    pub conversation_id: String,
    pub target_agent_binding_id: String,
    pub through_message_id: String,
    pub messages: Vec<ConversationContextMessage>,
}

/// A committed message paired with its immutable Conversation aggregate revision.
/// Revisions, rather than client timestamps or random message IDs, define the turn prefix.
#[derive(Clone, Debug, PartialEq)]
pub struct ConversationContextSourceMessage {
    pub conversation_revision: u64,
    pub message: ConversationMessageRecord,
}

/// Projects committed Conversation messages through an exact message boundary for a
/// new agent. The caller supplies only messages authorized for this Conversation; this
/// function additionally rejects mixed Conversation identities and duplicate IDs.
/// SYSTEM_NOTICE messages stay product UI state and are not prompt context.
pub fn build_conversation_context_package(
    conversation_id: &str,
    target_agent_binding_id: &str,
    through_message_id: &str,
    messages: &[ConversationContextSourceMessage],
) -> Result<ConversationContextPackage, StoreError> {
    let valid_identity = |value: &str| {
        !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
    };
    if !valid_identity(conversation_id)
        || !valid_identity(target_agent_binding_id)
        || !valid_identity(through_message_id)
        || messages.iter().any(|source| {
            source.conversation_revision == 0
                || source.message.conversation_id != conversation_id
                || !valid_identity(&source.message.message_id)
                || source.message.created_at.trim().is_empty()
        })
    {
        return Err(StoreError::Invalid(
            "Conversation context identity is invalid".to_owned(),
        ));
    }
    if messages.len() > CONVERSATION_CONTEXT_MAX_MESSAGES {
        return Err(StoreError::Invalid(
            "CONVERSATION_CONTEXT_LIMIT_EXCEEDED".to_owned(),
        ));
    }

    let mut ordered = messages.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|source| source.conversation_revision);
    if ordered.windows(2).any(|pair| {
        pair[0].conversation_revision == pair[1].conversation_revision
            || pair[0].message.message_id == pair[1].message.message_id
    }) {
        return Err(StoreError::Invalid(
            "Conversation context contains duplicate message identity or revision".to_owned(),
        ));
    }
    let boundary_revision = ordered
        .iter()
        .find(|source| source.message.message_id == through_message_id)
        .map(|source| source.conversation_revision)
        .ok_or_else(|| {
            StoreError::Invalid("Conversation context boundary is unavailable".to_owned())
        })?;

    let mut projected = Vec::new();
    let mut projected_bytes = conversation_id.len()
        + target_agent_binding_id.len()
        + through_message_id.len()
        + 64;
    for source in ordered {
        if source.conversation_revision > boundary_revision {
            break;
        }
        let message = &source.message;
        let context_message = match message.role.as_str() {
            "USER" | "AGENT" | "CHANNEL" => Some(ConversationContextMessage {
                message_id: message.message_id.clone(),
                role: message.role.clone(),
                turn_id: message.turn_id.clone(),
                source_agent_binding_id: (message.role == "AGENT")
                    .then(|| message.agent_binding_id.clone())
                    .flatten(),
                content: message.content.clone(),
                resource_refs: message.resource_refs.clone(),
                created_at: message.created_at.clone(),
            }),
            "SYSTEM_NOTICE" => None,
            _ => {
                return Err(StoreError::Invalid(
                    "Conversation context contains an unsupported message role".to_owned(),
                ));
            }
        };
        if let Some(context_message) = context_message {
            let encoded_len = serde_json::to_vec(&context_message)
                .map_err(|_| StoreError::Invalid("Conversation context is invalid".to_owned()))?
                .len();
            projected_bytes = projected_bytes
                .checked_add(encoded_len + 1)
                .ok_or_else(|| StoreError::Invalid(
                    "CONVERSATION_CONTEXT_LIMIT_EXCEEDED".to_owned(),
                ))?;
            if projected_bytes > CONVERSATION_CONTEXT_MAX_BYTES {
                return Err(StoreError::Invalid(
                    "CONVERSATION_CONTEXT_LIMIT_EXCEEDED".to_owned(),
                ));
            }
            projected.push(context_message);
        }
    }

    Ok(ConversationContextPackage {
        conversation_id: conversation_id.to_owned(),
        target_agent_binding_id: target_agent_binding_id.to_owned(),
        through_message_id: through_message_id.to_owned(),
        messages: projected,
    })
}

#[cfg(test)]
mod conversation_context_tests {
    use super::{
        build_conversation_context_package, ConversationContextSourceMessage, ConversationMessageRecord,
    };
    use serde_json::json;

    fn message(
        message_id: &str,
        conversation_id: &str,
        role: &str,
        content: &str,
        created_at: &str,
        binding: Option<&str>,
        session: Option<&str>,
    ) -> ConversationMessageRecord {
        ConversationMessageRecord {
            message_id: message_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            author: json!({"kind": role}),
            role: role.to_owned(),
            agent_session_id: session.map(str::to_owned),
            agent_binding_id: binding.map(str::to_owned),
            turn_id: Some(format!("turn-{message_id}")),
            content: json!([{"kind":"TEXT","text":content}]),
            resource_refs: json!([]),
            source_channel_ref: None,
            created_at: created_at.to_owned(),
        }
    }

    fn ordered(messages: &[ConversationMessageRecord]) -> Vec<ConversationContextSourceMessage> {
        messages
            .iter()
            .enumerate()
            .map(|(index, message)| ConversationContextSourceMessage {
                conversation_revision: index as u64 + 1,
                message: message.clone(),
            })
            .collect()
    }

    #[test]
    fn switching_binding_replays_committed_history_without_old_native_session_handles() {
        let messages = vec![
            message("m-user-1", "same-conversation", "USER", "Build a report", "2026-01-01T00:00:01Z", None, None),
            message("m-agent-1", "same-conversation", "AGENT", "I found three sources", "2026-01-01T00:00:02Z", Some("agent-a"), Some("private-session-a")),
            message("m-user-2", "same-conversation", "USER", "Use the second source", "2026-01-01T00:00:03Z", None, None),
            message("m-agent-2", "same-conversation", "AGENT", "Drafted from source two", "2026-01-01T00:00:04Z", Some("agent-b"), Some("private-session-b")),
            message("m-later", "same-conversation", "USER", "This message is after the turn boundary", "2026-01-01T00:00:06Z", None, None),
            message("m-other-chat", "other-conversation", "USER", "Never transfer this", "2026-01-01T00:00:01Z", None, None),
        ];

        let sources = ordered(&messages[..5]);
        let context = build_conversation_context_package(
            "same-conversation",
            "agent-c",
            "m-agent-2",
            &sources,
        )
        .expect("build context for the newly selected agent");

        assert_eq!(context.conversation_id, "same-conversation");
        assert_eq!(context.target_agent_binding_id, "agent-c");
        assert_eq!(context.through_message_id, "m-agent-2");
        assert_eq!(
            context.messages.iter().map(|message| message.message_id.as_str()).collect::<Vec<_>>(),
            ["m-user-1", "m-agent-1", "m-user-2", "m-agent-2"]
        );
        assert_eq!(context.messages[1].source_agent_binding_id.as_deref(), Some("agent-a"));
        assert_eq!(context.messages[1].content[0]["text"], "I found three sources");
        let encoded = serde_json::to_string(&context).expect("serialize provider context");
        assert!(!encoded.contains("private-session-a"));
        assert!(!encoded.contains("private-session-b"));
    }

    #[test]
    fn context_rejects_a_boundary_or_message_from_another_conversation() {
        let messages = vec![message(
            "m-user",
            "conversation-a",
            "USER",
            "Keep context scoped",
            "2026-01-01T00:00:01Z",
            None,
            None,
        )];
        let sources = ordered(&messages);

        assert!(build_conversation_context_package(
            "conversation-a",
            "agent-a",
            "missing-boundary",
            &sources,
        )
        .is_err());
        assert!(build_conversation_context_package(
            "conversation-b",
            "agent-a",
            "m-user",
            &sources,
        )
        .is_err());
    }

    #[test]
    fn context_rejects_oversized_history_instead_of_silently_truncating() {
        let oversized = "x".repeat(4 * 1024 * 1024 + 1);
        let messages = vec![message(
            "m-large",
            "conversation-a",
            "USER",
            &oversized,
            "2026-01-01T00:00:01Z",
            None,
            None,
        )];
        let sources = ordered(&messages);

        assert!(build_conversation_context_package(
            "conversation-a",
            "agent-a",
            "m-large",
            &sources,
        )
        .is_err());
    }

}

pub fn request_payload_value(workspace_id: &str, title: Option<&str>) -> Value {
    serde_json::json!({"operation":"conversation.create.v1","workspace_id":workspace_id,"title":title})
}
