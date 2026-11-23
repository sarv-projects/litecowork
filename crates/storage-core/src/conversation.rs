use crate::{AgentSessionRecord, DomainEvent, EventDraft, StoreError, WorkspaceCreateRequest};
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

/// Strict single-flight lane (B-01): another turn of the Conversation is OPEN, RUNNING,
/// WAITING_USER, WAITING_DEPENDENCY or CANCEL_REQUESTED. Surfaces as HTTP 409.
pub const CONVERSATION_TURN_IN_PROGRESS: &str = "CONVERSATION_TURN_IN_PROGRESS";

/// Fresh Conversation dispatch is refused unless the exact process-owned Runtime
/// incarnation is still current, ONLINE and READY at the admission transaction.
pub const CONVERSATION_RUNTIME_UNAVAILABLE: &str = "RUNTIME_UNAVAILABLE";

/// Another response with a different provider identity, content or author was already
/// committed for this turn and retry ordinal (B-04).
pub const CONVERSATION_RESPONSE_IDENTITY_CONFLICT: &str = "CONVERSATION_RESPONSE_IDENTITY_CONFLICT";

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

/// Ephemeral internal admission metadata. Values come from the authenticated daemon
/// process, are never accepted from a desktop request, and are never persisted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExecutionFence {
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
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

/// Lifecycle status of a Conversation in the history list (spec 014). A Conversation
/// with no `conversation_history_state` row is `Active`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ConversationStatus {
    Active,
    Archived,
    DeletionPending,
    Deleted,
}

impl ConversationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Archived => "ARCHIVED",
            Self::DeletionPending => "DELETION_PENDING",
            Self::Deleted => "DELETED",
        }
    }
}

/// Stable codes for history-management refusals (014 T03). Surface as HTTP 409.
pub const CONVERSATION_ARCHIVED: &str = "CONVERSATION_ARCHIVED";
pub const CONVERSATION_DELETED: &str = "CONVERSATION_DELETED";
pub const CONVERSATION_NOT_QUIESCENT: &str = "CONVERSATION_NOT_QUIESCENT";

/// Version-fenced, idempotent Conversation rename. The event carries `title_digest`
/// only, never the title.
#[derive(Clone, Debug)]
pub struct RenameConversation {
    pub request: WorkspaceCreateRequest,
    pub principal_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub expected_version: u64,
    pub title: String,
    pub event: EventDraft,
}

/// Version-fenced, idempotent archive (`archived = true`) or unarchive.
#[derive(Clone, Debug)]
pub struct SetConversationArchived {
    pub request: WorkspaceCreateRequest,
    pub principal_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub expected_version: u64,
    pub archived: bool,
    pub event: EventDraft,
}

pub trait ConversationHistoryStore: Send + Sync {
    fn rename_conversation(
        &self,
        commit: RenameConversation,
    ) -> Result<CommittedConversation, StoreError>;

    fn set_conversation_archived(
        &self,
        commit: SetConversationArchived,
    ) -> Result<CommittedConversation, StoreError>;

    fn get_conversation_status(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationStatus>, StoreError>;

    /// Lists Conversations with exactly the requested status, newest first, using the
    /// same `(created_at, conversation_id)` cursor as `list_conversations`.
    fn list_conversations_by_status(
        &self,
        principal_id: &str,
        workspace_id: &str,
        status: ConversationStatus,
        after_created_at: Option<&str>,
        after_conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ConversationRecord>, StoreError>;
}

/// Maximum characters of an owner side note body (012 AC-006).
pub const OWNER_SIDE_NOTE_MAX_CHARS: usize = 4_000;
/// Side note refused because its correlated turn is already terminal (012 AC-005).
pub const SIDE_NOTE_TURN_TERMINAL: &str = "SIDE_NOTE_TURN_TERMINAL";
/// Side note body is empty or over the character limit (012 AC-006).
pub const SIDE_NOTE_OVER_BUDGET: &str = "SIDE_NOTE_OVER_BUDGET";

/// Immutable provenance row appended in the same transaction that settles a turn
/// CANCELLED after an owner take-over.
#[derive(Clone, Debug)]
pub struct OwnerControlEventInsert {
    pub control_event_id: String,
    pub occurred_at: String,
}

/// A bounded owner side note correlated to a non-terminal turn. Never a
/// `conversation_messages` row, so it can never enter a native prompt.
#[derive(Clone, Debug)]
pub struct AppendOwnerSideNote {
    pub request: WorkspaceCreateRequest,
    pub principal_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub correlated_turn_id: String,
    pub side_note_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnerSideNoteRecord {
    pub side_note_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub correlated_turn_id: String,
    pub body: String,
    pub created_at: String,
}

pub trait ConversationCoeditStore: Send + Sync {
    /// Idempotent. Fails with `SIDE_NOTE_TURN_TERMINAL`, `SIDE_NOTE_OVER_BUDGET` or
    /// `NotFound` (foreign owner, Workspace or cross-Conversation turn) and writes nothing.
    fn append_owner_side_note(
        &self,
        commit: AppendOwnerSideNote,
    ) -> Result<OwnerSideNoteRecord, StoreError>;

    fn list_owner_side_notes(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        correlated_turn_id: &str,
    ) -> Result<Vec<OwnerSideNoteRecord>, StoreError>;
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
    pub runtime_fence: RuntimeExecutionFence,
    pub message: ConversationMessageRecord,
    pub turn: ConversationTurn,
    pub message_event: EventDraft,
    pub turn_event: EventDraft,
}

/// Atomically creates a new Conversation and admits its first USER turn. The one request
/// identity covers the Conversation, message, turn, events, receipt and dispatch intent.
#[derive(Clone, Debug)]
pub struct CreateConversationWithFirstTurnCommit {
    pub request: WorkspaceCreateRequest,
    pub principal_id: String,
    pub workspace_id: String,
    pub runtime_fence: RuntimeExecutionFence,
    pub conversation: ConversationRecord,
    pub conversation_event: EventDraft,
    pub message: ConversationMessageRecord,
    pub turn: ConversationTurn,
    pub message_event: EventDraft,
    pub turn_event: EventDraft,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedConversationFirstTurn {
    pub conversation: ConversationRecord,
    pub message: ConversationMessageRecord,
    pub turn: ConversationTurn,
    pub events: Vec<DomainEvent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedConversationTurn {
    pub message: ConversationMessageRecord,
    pub turn: ConversationTurn,
    pub events: Vec<DomainEvent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationTurnReceipt {
    pub turn_id: String,
    pub conversation_id: String,
    pub user_message_id: String,
    pub status: ConversationTurnStatus,
    pub accepted_at: String,
    pub correlation_id: String,
    pub presentation_preference: PresentationPreference,
}

#[derive(Clone, Debug)]
pub struct CommitConversationAgentResponse {
    pub request: WorkspaceCreateRequest,
    pub principal_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub turn_id: String,
    pub agent_session_id: String,
    pub expected_turn_version: u64,
    pub expected_session_version: u64,
    /// Retry ordinal of the turn this response answers (B-04). Must equal the turn's
    /// current ordinal inside the commit transaction.
    pub retry_ordinal: u32,
    /// Provider-side identity of the final response. Together with the turn id and
    /// retry ordinal it is the commit identity: an exact repeat replays the stored
    /// receipt, any differing repeat is rejected without a partial write.
    pub provider_response_id: String,
    pub turn_status: ConversationTurnStatus,
    pub message: ConversationMessageRecord,
    /// Token usage reported for this turn (B-10); `None` records an UNKNOWN attribution.
    pub usage: Option<ConversationTurnUsage>,
    pub message_event: EventDraft,
    pub turn_event: EventDraft,
    pub session_event: EventDraft,
}

/// Token usage attributed to one settled turn (B-10).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UsageAttributionSource {
    ProviderReported,
    EstimatedLocal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationTurnUsage {
    pub attribution_source: UsageAttributionSource,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedConversationAgentResponse {
    pub message: ConversationMessageRecord,
    pub turn: ConversationTurn,
    pub session: AgentSessionRecord,
    pub message_event: DomainEvent,
    pub turn_event: DomainEvent,
    pub session_event: DomainEvent,
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

    /// Atomically commits the first USER message and OPEN turn with the new Conversation,
    /// their events, the idempotency receipt and the UNCLAIMED dispatch intent.
    fn create_conversation_with_first_turn(
        &self,
        commit: CreateConversationWithFirstTurnCommit,
    ) -> Result<CommittedConversationFirstTurn, StoreError>;

    /// Atomically commits the final AGENT response message, settles the turn,
    /// closes the agent session, cleans up the local host binding, and writes
    /// aggregate snapshots, domain events, and the request receipt.
    fn commit_conversation_agent_response(
        &self,
        commit: CommitConversationAgentResponse,
    ) -> Result<CommittedConversationAgentResponse, StoreError>;

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

    /// Retry uses the same state transition but requires an ephemeral current-Runtime
    /// fence. Other lifecycle transitions continue through `transition_conversation_turn`.
    #[allow(clippy::too_many_arguments)]
    fn transition_conversation_turn_with_runtime_fence(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        expected_version: u64,
        command: TurnCommand,
        event: EventDraft,
        runtime_fence: Option<RuntimeExecutionFence>,
    ) -> Result<ConversationTurn, StoreError>;

    /// Settles a turn without a final answer together with its AgentSession; see
    /// [`SettleConversationTurn`]. Stale turn or session versions fail with no write.
    fn settle_conversation_turn(
        &self,
        settlement: SettleConversationTurn,
    ) -> Result<SettledConversationTurn, StoreError>;

    /// Returns the turn that an earlier command with this idempotency key already
    /// produced (the `causation_id` of its canonical turn event), so an exact repeat of
    /// a Retry or Cancel request replays instead of failing on the moved version.
    fn find_conversation_turn_command_replay(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        request_id: &str,
    ) -> Result<Option<ConversationTurn>, StoreError>;

    /// Compare-and-swap claim of the durable dispatch intent for one admitted turn
    /// attempt (UNCLAIMED to CLAIMED). Returns `false` when another dispatcher already
    /// holds the claim or the intent is SETTLED. Must precede the STARTING reservation.
    fn claim_conversation_dispatch_intent(
        &self,
        turn_id: &str,
        retry_ordinal: u32,
        runtime_incarnation_id: &str,
        claimed_at: &str,
    ) -> Result<bool, StoreError>;

    /// Reads exactly one durable dispatch intent by its primary key. This is used only
    /// after an uncertain compare-and-set result to reconcile ownership; it must not be
    /// implemented by scanning the bounded global state listing.
    fn get_conversation_dispatch_intent(
        &self,
        turn_id: &str,
        retry_ordinal: u32,
    ) -> Result<Option<ConversationDispatchIntent>, StoreError>;

    /// Returns a CLAIMED intent to UNCLAIMED when the claiming incarnation is gone and no
    /// AgentSession was reserved for it. Returns `false` when the intent is not CLAIMED.
    fn release_conversation_dispatch_intent(
        &self,
        turn_id: &str,
        retry_ordinal: u32,
    ) -> Result<bool, StoreError>;

    /// Bounded, ordered read of dispatch intents in one state (oldest first). Used by the
    /// startup sweep and the dispatcher; never exposes provider-private handles.
    fn list_conversation_dispatch_intents(
        &self,
        state: ConversationDispatchState,
        limit: usize,
    ) -> Result<Vec<ConversationDispatchIntent>, StoreError>;

    /// Appends one REPORTED activity observation for a turn attempt (T25). Idempotent on
    /// `(turn_id, retry_ordinal, sequence)`: an exact repeat is a no-op. The row records
    /// what the native agent said it did; it is never an Effect, receipt or grant.
    fn append_conversation_turn_activity(
        &self,
        record: ConversationActivityRecord,
    ) -> Result<(), StoreError>;

    /// Bounded, ordered read (sequence ascending) of one turn's activity observations,
    /// scoped to the Workspace and Conversation. `retry_ordinal` selects one attempt.
    fn list_conversation_turn_activity(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        limit: usize,
    ) -> Result<Vec<ConversationActivityRecord>, StoreError>;
}

/// Maximum activity observations kept per turn attempt.
pub const CONVERSATION_ACTIVITY_MAX_PER_ATTEMPT: usize = 200;

/// A REPORTED activity observation. `label` is a bounded, sanitized display word (a tool
/// name or fixed phrase), never a path, argument, output or reasoning text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationActivityRecord {
    pub turn_id: String,
    pub retry_ordinal: u32,
    pub conversation_id: String,
    pub sequence: u32,
    /// `TOOL`, `FILE`, `PERMISSION`, `TODO`, `STEP` or `CONTEXT`.
    pub kind: String,
    /// `STARTED`, `PROGRESS`, `SUCCEEDED`, `FAILED`, `ASKED`, `UPDATED`, `ENDED` or `REDUCED`.
    pub status: String,
    pub call_ref: Option<String>,
    pub label: Option<String>,
    pub created_at: String,
}

/// Terminal state the settled turn's AgentSession reaches (no final answer is committed).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationSessionEnd {
    /// Observed native quiescence: the session ends CLOSED.
    Closed,
    /// The native session is unknown or gone: the session ends LOST.
    Lost,
}

/// The AgentSession ended together with a turn settlement.
#[derive(Clone, Debug)]
pub struct SettleConversationSession {
    pub agent_session_id: String,
    pub expected_session_version: u64,
    pub end: ConversationSessionEnd,
    pub occurred_at: String,
    pub session_event: EventDraft,
}

/// Atomically settles a turn that produced no final answer (FAILED, or CANCELLED after
/// observed quiescence), ends its AgentSession, releases the ephemeral host binding and
/// settles the dispatch intent. No AGENT message is ever written by this operation.
#[derive(Clone, Debug)]
pub struct SettleConversationTurn {
    pub principal_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub turn_id: String,
    pub expected_turn_version: u64,
    /// `TurnCommand::Fail` or `TurnCommand::SettleCancelled`.
    pub command: TurnCommand,
    pub turn_event: EventDraft,
    pub session: Option<SettleConversationSession>,
    /// Whatever usage was observed for this attempt (B-10); `None` records UNKNOWN. A
    /// missing or rejected usage row never blocks the settlement.
    pub usage: Option<ConversationTurnUsage>,
    /// When set (take-control), an `OWNER_TOOK_CONTROL` event is appended in the same
    /// transaction. Only valid with `TurnCommand::SettleCancelled`.
    pub control_event: Option<OwnerControlEventInsert>,
}

#[derive(Clone, Debug)]
pub struct SettledConversationTurn {
    pub turn: ConversationTurn,
    pub session: Option<AgentSessionRecord>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ConversationDispatchState {
    Unclaimed,
    Claimed,
    Settled,
}

impl ConversationDispatchState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unclaimed => "UNCLAIMED",
            Self::Claimed => "CLAIMED",
            Self::Settled => "SETTLED",
        }
    }
}

/// Durable record that dispatch is owed for one admitted turn attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationDispatchIntent {
    pub workspace_id: String,
    pub conversation_id: String,
    pub turn_id: String,
    pub retry_ordinal: u32,
    pub state: ConversationDispatchState,
    pub claimed_by_runtime_incarnation: Option<String>,
    pub created_at: String,
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
        || record
            .active_agent_binding_id
            .as_ref()
            .is_some_and(|binding_id| !valid(binding_id))
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
    /// Count of older committed messages that precede `messages` and were left out by
    /// the B-07 window (message count or byte bound). `0` means the full committed
    /// prefix is present. The oldest omitted message is message 1 of the Conversation.
    #[serde(default)]
    pub omitted_message_count: u32,
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
    let base_bytes =
        conversation_id.len() + target_agent_binding_id.len() + through_message_id.len() + 64;
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
            projected.push((context_message, encoded_len + 1));
        }
    }

    // B-07 window: keep the most recent contiguous messages, ending at the boundary,
    // that fit the message-count and byte bounds. Older messages are counted, never
    // silently dropped. The boundary message itself must fit or the turn is refused.
    let mut kept_bytes = base_bytes;
    let mut keep_from = projected.len();
    while keep_from > 0 {
        let (_, size) = &projected[keep_from - 1];
        if projected.len() - keep_from >= CONVERSATION_CONTEXT_MAX_MESSAGES
            || kept_bytes + size > CONVERSATION_CONTEXT_MAX_BYTES
        {
            break;
        }
        kept_bytes += size;
        keep_from -= 1;
    }
    if keep_from == projected.len() {
        return Err(StoreError::Invalid(
            "CONVERSATION_CONTEXT_LIMIT_EXCEEDED".to_owned(),
        ));
    }
    let omitted_message_count = u32::try_from(keep_from).unwrap_or(u32::MAX);
    let messages = projected
        .into_iter()
        .skip(keep_from)
        .map(|(message, _)| message)
        .collect();

    Ok(ConversationContextPackage {
        conversation_id: conversation_id.to_owned(),
        target_agent_binding_id: target_agent_binding_id.to_owned(),
        through_message_id: through_message_id.to_owned(),
        messages,
        omitted_message_count,
    })
}

pub fn request_payload_value(workspace_id: &str, title: Option<&str>) -> Value {
    serde_json::json!({"operation":"conversation.create.v1","workspace_id":workspace_id,"title":title})
}

#[cfg(test)]
mod conversation_context_tests {
    use super::{
        ConversationContextSourceMessage, ConversationMessageRecord,
        build_conversation_context_package,
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
        let messages = [
            message(
                "m-user-1",
                "same-conversation",
                "USER",
                "Build a report",
                "2026-01-01T00:00:01Z",
                None,
                None,
            ),
            message(
                "m-agent-1",
                "same-conversation",
                "AGENT",
                "I found three sources",
                "2026-01-01T00:00:02Z",
                Some("agent-a"),
                Some("private-session-a"),
            ),
            message(
                "m-user-2",
                "same-conversation",
                "USER",
                "Use the second source",
                "2026-01-01T00:00:03Z",
                None,
                None,
            ),
            message(
                "m-agent-2",
                "same-conversation",
                "AGENT",
                "Drafted from source two",
                "2026-01-01T00:00:04Z",
                Some("agent-b"),
                Some("private-session-b"),
            ),
            message(
                "m-later",
                "same-conversation",
                "USER",
                "This message is after the turn boundary",
                "2026-01-01T00:00:06Z",
                None,
                None,
            ),
            message(
                "m-other-chat",
                "other-conversation",
                "USER",
                "Never transfer this",
                "2026-01-01T00:00:01Z",
                None,
                None,
            ),
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
            context
                .messages
                .iter()
                .map(|message| message.message_id.as_str())
                .collect::<Vec<_>>(),
            ["m-user-1", "m-agent-1", "m-user-2", "m-agent-2"]
        );
        assert_eq!(
            context.messages[1].source_agent_binding_id.as_deref(),
            Some("agent-a")
        );
        assert_eq!(
            context.messages[1].content[0]["text"],
            "I found three sources"
        );
        let encoded = serde_json::to_string(&context).expect("serialize provider context");
        assert!(!encoded.contains("private-session-a"));
        assert!(!encoded.contains("private-session-b"));
    }

    #[test]
    fn relay_context_preserves_each_producer_across_a_to_b_to_a() {
        let messages = [
            message(
                "user-1",
                "relay-chat",
                "USER",
                "First request",
                "2026-01-01T00:00:01Z",
                None,
                None,
            ),
            message(
                "agent-a-1",
                "relay-chat",
                "AGENT",
                "A response",
                "2026-01-01T00:00:02Z",
                Some("binding-a"),
                Some("private-session-a1"),
            ),
            message(
                "user-2",
                "relay-chat",
                "USER",
                "Now ask B",
                "2026-01-01T00:00:03Z",
                None,
                None,
            ),
            message(
                "agent-b-1",
                "relay-chat",
                "AGENT",
                "B response",
                "2026-01-01T00:00:04Z",
                Some("binding-b"),
                Some("private-session-b1"),
            ),
            message(
                "user-3",
                "relay-chat",
                "USER",
                "Return to A",
                "2026-01-01T00:00:05Z",
                None,
                None,
            ),
            message(
                "agent-a-2",
                "relay-chat",
                "AGENT",
                "A response after relay",
                "2026-01-01T00:00:06Z",
                Some("binding-a"),
                Some("private-session-a2"),
            ),
            message(
                "user-4",
                "relay-chat",
                "USER",
                "Continue with B",
                "2026-01-01T00:00:07Z",
                None,
                None,
            ),
        ];

        let sources = ordered(&messages);
        let context =
            build_conversation_context_package("relay-chat", "binding-b", "user-4", &sources)
                .expect(
                    "relay context keeps the committed transcript through the latest user turn",
                );

        let agent_producers = context
            .messages
            .iter()
            .filter(|message| message.role == "AGENT")
            .map(|message| message.source_agent_binding_id.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(
            agent_producers,
            [Some("binding-a"), Some("binding-b"), Some("binding-a")]
        );
        let encoded = serde_json::to_string(&context).expect("serialize portable relay context");
        assert!(!encoded.contains("private-session-a1"));
        assert!(!encoded.contains("private-session-b1"));
        assert!(!encoded.contains("private-session-a2"));
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

        assert!(
            build_conversation_context_package(
                "conversation-a",
                "agent-a",
                "missing-boundary",
                &sources,
            )
            .is_err()
        );
        assert!(
            build_conversation_context_package("conversation-b", "agent-a", "m-user", &sources,)
                .is_err()
        );
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

        assert!(
            build_conversation_context_package("conversation-a", "agent-a", "m-large", &sources,)
                .is_err()
        );
    }

    #[test]
    fn context_window_boundary_at_1024_and_1025_messages_omits_only_the_oldest() {
        let build = |count: usize| {
            let messages = (0..count)
                .map(|index| {
                    message(
                        &format!("m{index:05}"),
                        "c",
                        if index % 2 == 0 { "USER" } else { "AGENT" },
                        "x",
                        "2026-01-01T00:00:01Z",
                        (index % 2 == 1).then_some("agent-a"),
                        None,
                    )
                })
                .collect::<Vec<_>>();
            let through = format!("m{:05}", count - 1);
            build_conversation_context_package("c", "agent-b", &through, &ordered(&messages))
        };
        // The boundary message must be a USER message for dispatch; here only the
        // window arithmetic is under test.
        let exact = build(1_024).expect("exactly the bound");
        assert_eq!(
            (exact.messages.len(), exact.omitted_message_count),
            (1_024, 0)
        );
        let over = build(1_025).expect("one over is windowed, not refused");
        assert_eq!(
            (over.messages.len(), over.omitted_message_count),
            (1_024, 1)
        );
        assert_eq!(over.messages[0].message_id, "m00001");
        assert_eq!(over.messages.last().expect("boundary").message_id, "m01024");
    }

    #[test]
    fn context_window_byte_bound_keeps_the_boundary_and_refuses_when_it_alone_is_too_large() {
        let big = "x".repeat(1024 * 1024);
        let messages = [
            message("m-1", "c", "USER", &big, "2026-01-01T00:00:01Z", None, None),
            message(
                "m-2",
                "c",
                "AGENT",
                &big,
                "2026-01-01T00:00:02Z",
                Some("a"),
                None,
            ),
            message("m-3", "c", "USER", &big, "2026-01-01T00:00:03Z", None, None),
            message(
                "m-4",
                "c",
                "AGENT",
                &big,
                "2026-01-01T00:00:04Z",
                Some("a"),
                None,
            ),
            message("m-5", "c", "USER", &big, "2026-01-01T00:00:05Z", None, None),
        ];
        let windowed = build_conversation_context_package("c", "b", "m-5", &ordered(&messages))
            .expect("older messages are windowed out");
        assert_eq!(windowed.messages.len(), 3);
        assert_eq!(windowed.omitted_message_count, 2);
        assert_eq!(windowed.messages[0].message_id, "m-3");

        let too_big = [message(
            "m-only",
            "c",
            "USER",
            &"x".repeat(4 * 1024 * 1024),
            "2026-01-01T00:00:01Z",
            None,
            None,
        )];
        let refused = build_conversation_context_package("c", "b", "m-only", &ordered(&too_big));
        assert!(matches!(
            refused,
            Err(crate::StoreError::Invalid(ref code)) if code == "CONVERSATION_CONTEXT_LIMIT_EXCEEDED"
        ));
    }
}
