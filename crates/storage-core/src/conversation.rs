use crate::{AgentSessionRecord, DomainEvent, EventDraft, StoreError, WorkspaceCreateRequest};
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

    /// Admits the Conversation-scoped AgentSession for an OPEN turn in one transaction.
    ///
    /// This is the only path that may move a turn OPEN -> RUNNING. It refuses when the
    /// binding is not enabled and lead-eligible, when the session is not scoped to this
    /// exact Conversation turn, when the turn is not OPEN at the expected version, or when
    /// the Workspace is not ACTIVE for the principal. A refusal commits no session, no
    /// turn transition, and no event.
    fn admit_conversation_turn(
        &self,
        commit: AdmitConversationTurnCommit,
    ) -> Result<AdmittedConversationTurn, StoreError>;

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
    /// The OPEN -> RUNNING start command is accepted only through
    /// [`ConversationTurnStore::admit_conversation_turn`].
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

/// Atomically admits a Conversation-scoped AgentSession for an OPEN turn and moves that
/// turn OPEN -> RUNNING.
///
/// This is the one operation whose absence made native Conversation Send impossible: the
/// turn may never be RUNNING without a bound session, and no session may exist for a turn
/// that did not start. Session creation, the turn transition and both events therefore
/// commit in a single transaction, and the idempotency receipt replays the same committed
/// result instead of admitting a second session.
///
/// `session.scope_kind` must be `CONVERSATION`, its `conversation_id` and
/// `conversation_turn_id` must name this turn, and its `agent_binding_id` must be the
/// binding the Conversation resolved. Native handles are excluded by `AgentSessionRecord`
/// and stay in Runtime-local bindings.
#[derive(Clone, Debug)]
pub struct AdmitConversationTurnCommit {
    pub request: WorkspaceCreateRequest,
    pub principal_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub session: AgentSessionRecord,
    pub turn_id: String,
    pub expected_turn_version: u64,
    pub session_event: EventDraft,
    pub turn_event: EventDraft,
}

/// The live host a Conversation-scoped session must use, resolved authoritatively.
///
/// Resolution and admission are separate on purpose: the Operator asks the store which
/// endpoint and Runtime incarnation are currently eligible for this Conversation's
/// binding, and the admission transaction then re-checks that same eligibility. A host
/// that expires between the two calls is refused honestly rather than silently
/// substituted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedConversationHost {
    pub agent_binding_id: String,
    pub endpoint_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedConversationTurn {
    pub session: AgentSessionRecord,
    pub turn: ConversationTurn,
    pub events: Vec<DomainEvent>,
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
