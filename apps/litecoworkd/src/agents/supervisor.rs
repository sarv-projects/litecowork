//! AgentSessionSupervisor and AgentTurnCoordinator for Conversation turns.
//!
//! Orchestrates the three-phase session lifecycle:
//! 1. Reservation: STARTING AgentSession committed in SQLite.
//! 2. Native Handshake: Spawn or connect to native agent adapter (OpenCode, Claude Code, etc.)
//!    outside SQLite.
//! 3. Activation: Atomic SQLite CAS `activate_conversation_agent_session` binding native handle and
//!    transitioning turn OPEN -> RUNNING.
//! 4. Prompt Dispatch: Canonical conversation context package materialized and sent to native agent.
//! 5. Response Streaming & Terminal Settlement: Stream deltas collected and atomically committed
//!    via `commit_conversation_agent_response`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use storage_core::conversation::{
    CommitConversationAgentResponse, ConversationDispatchIntent, ConversationDispatchState,
    ConversationMessageRecord, ConversationSessionEnd, ConversationTurn, ConversationTurnStatus,
    ConversationTurnStore, ConversationTurnUsage, SettleConversationSession,
    SettleConversationTurn, TurnCommand, UsageAttributionSource,
};
use storage_core::{
    ActivateConversationAgentSession, AgentCatalogStore, AgentHostInstanceRecord, AgentHostStore,
    AgentSessionRecord, AgentSessionStore, ConversationAgentSessionStart, EventDraft,
    MarkStartingConversationAgentSessionLost, StoreError, WorkspaceCreateRequest,
};
use storage_sqlite::{SqliteConversationStore, SqliteWorkspaceStore};

use super::adapter::{
    AgentAdapter, CancelSignal, ObserverSet, OpenCodeAdapter, StreamBudget, TurnFailure,
    TurnObservation, TurnObserver, UsageCollector,
};
use super::host_supervisor::{AgentHostSupervisor, HostStartError, start_host_process};
use super::native_provider::NativeProviderKey;
use super::normalizer::NormalizedAgentEvent;
use super::opencode_server::{
    OpenCodeEnvironment, OpenCodeError, OpenCodePermissionDecision, OpenCodeServer,
    OpenCodeServerConfig, OpenCodeServerProtocol,
};

const MAX_NATIVE_PERMISSION_PROMPTS_PER_TURN: usize = 32;
const MAX_NATIVE_PERMISSION_PROMPTS_PER_RUNTIME: usize = 256;
const MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION: usize = 8;
const MAX_NATIVE_PERMISSION_IDEMPOTENCY_KEYS_PER_PROMPT: usize = 16;
const MAX_NATIVE_PERMISSION_SEQUENCE_PER_TURN: u32 = 4_096;

/// The active turn worker is the only owner of the native server. Keep the
/// permission operations behind this narrow boundary so worker dispatch and
/// reconciliation can be exercised without launching an external process.
trait NativePermissionProvider {
    fn reply_session_permission(
        &mut self,
        session_id: &str,
        request_id: &str,
        decision: OpenCodePermissionDecision,
    ) -> Result<(), OpenCodeError>;

    fn list_session_permissions(
        &mut self,
        session_id: &str,
    ) -> Result<serde_json::Value, OpenCodeError>;
}

impl NativePermissionProvider for OpenCodeServer {
    fn reply_session_permission(
        &mut self,
        session_id: &str,
        request_id: &str,
        decision: OpenCodePermissionDecision,
    ) -> Result<(), OpenCodeError> {
        OpenCodeServer::reply_session_permission(self, session_id, request_id, decision)
    }

    fn list_session_permissions(
        &mut self,
        session_id: &str,
    ) -> Result<serde_json::Value, OpenCodeError> {
        OpenCodeServer::list_session_permissions(self, session_id)
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct NativePermissionScope {
    workspace_id: String,
    conversation_id: String,
    turn_id: String,
    retry_ordinal: u32,
    agent_session_id: String,
    runtime_incarnation_id: String,
    native_session_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum NativePermissionState {
    Pending,
    Replying,
    Answered,
    OutcomeUnknown,
    SessionEnded,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum NativePermissionDecision {
    AllowOnce,
    RejectSessionPending,
}

impl NativePermissionDecision {
    fn public_name(self) -> &'static str {
        match self {
            Self::AllowOnce => "ALLOW_ONCE",
            Self::RejectSessionPending => "REJECT_SESSION_PENDING",
        }
    }

    fn provider_decision(self) -> OpenCodePermissionDecision {
        match self {
            Self::AllowOnce => OpenCodePermissionDecision::Once,
            Self::RejectSessionPending => OpenCodePermissionDecision::Reject,
        }
    }
}

pub(crate) struct NativePermissionCommand {
    prompt_token: String,
    decision: NativePermissionDecision,
    completed: tokio::sync::oneshot::Sender<()>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct NativePermissionPromptView {
    sequence: u32,
    prompt_token: String,
    state: NativePermissionState,
    action: String,
    explanation: Option<String>,
    resource_count: usize,
    resources: Vec<String>,
    decision: Option<String>,
}

impl NativePermissionPromptView {
    pub(crate) fn sequence(&self) -> u32 {
        self.sequence
    }

    pub(crate) fn prompt_token(&self) -> &str {
        &self.prompt_token
    }

    pub(crate) fn state_name(&self) -> &'static str {
        match self.state {
            NativePermissionState::Pending => "PENDING",
            NativePermissionState::Replying => "REPLYING",
            NativePermissionState::Answered => "ANSWERED",
            NativePermissionState::OutcomeUnknown => "OUTCOME_UNKNOWN",
            NativePermissionState::SessionEnded => "SESSION_ENDED",
        }
    }

    pub(crate) fn decision(&self) -> Option<&str> {
        self.decision.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativePermissionAttemptSnapshot {
    pub(crate) sequence: u32,
    pub(crate) items: Vec<NativePermissionPromptView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativePermissionWorkspaceItemView {
    pub(crate) conversation_id: String,
    pub(crate) turn_id: String,
    pub(crate) retry_ordinal: u32,
    pub(crate) sequence: u32,
    pub(crate) agent_session_id: String,
    pub(crate) runtime_incarnation_id: String,
    pub(crate) prompt: NativePermissionPromptView,
}

#[derive(Clone, Debug)]
struct NativePermissionPrompt {
    scope: NativePermissionScope,
    native_request_id: String,
    native_resources: Vec<String>,
    view: NativePermissionPromptView,
    idempotency: Vec<(String, String, NativePermissionDecision)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativePermissionRegistryError {
    ForeignSession,
    InvalidPrompt,
    ConflictingDuplicate,
    Capacity,
    TokenCollision,
    StalePrompt,
    ReplyConflict,
    ReplyCapacity,
    MailboxUnavailable,
    Poisoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativePermissionReplyError {
    SessionEnded,
    Conflict,
    Unavailable,
}

impl From<NativePermissionRegistryError> for NativePermissionReplyError {
    fn from(error: NativePermissionRegistryError) -> Self {
        match error {
            NativePermissionRegistryError::StalePrompt
            | NativePermissionRegistryError::MailboxUnavailable => Self::SessionEnded,
            NativePermissionRegistryError::ReplyConflict => Self::Conflict,
            _ => Self::Unavailable,
        }
    }
}

/// Ephemeral, process-local permission correlation. Native provider identifiers
/// exist only in this registry and never in its serializable prompt projection.
#[derive(Clone, Default)]
struct NativePermissionRegistry {
    prompts: Arc<Mutex<HashMap<String, NativePermissionPrompt>>>,
    mailboxes: Arc<Mutex<HashMap<NativePermissionScope, SyncSender<NativePermissionCommand>>>>,
}

impl NativePermissionRegistry {
    fn register(
        &self,
        scope: &NativePermissionScope,
        event: &NormalizedAgentEvent,
        workspace_root: Option<&Path>,
    ) -> Result<NativePermissionPromptView, NativePermissionRegistryError> {
        let NormalizedAgentEvent::PermissionAsked {
            permission_id,
            native_session_id,
            action,
            resources,
            message,
        } = event
        else {
            return Err(NativePermissionRegistryError::InvalidPrompt);
        };
        for _ in 0..3 {
            let mut token_bytes = [0_u8; 32];
            getrandom::fill(&mut token_bytes)
                .map_err(|_| NativePermissionRegistryError::InvalidPrompt)?;
            let token = hex::encode(token_bytes);
            match self.register_with_token_and_display(
                scope,
                native_session_id,
                token,
                permission_id.clone(),
                action.clone(),
                message.clone(),
                resources.clone(),
                permission_resource_labels(workspace_root, resources),
            ) {
                Err(NativePermissionRegistryError::TokenCollision) => continue,
                result => return result,
            }
        }
        Err(NativePermissionRegistryError::TokenCollision)
    }

    fn register_with_token(
        &self,
        scope: &NativePermissionScope,
        observed_session_id: &str,
        token: String,
        native_request_id: String,
        action: String,
        explanation: Option<String>,
        native_resources: Vec<String>,
    ) -> Result<NativePermissionPromptView, NativePermissionRegistryError> {
        let display_resources = permission_resource_labels(None, &native_resources);
        self.register_with_token_and_display(
            scope,
            observed_session_id,
            token,
            native_request_id,
            action,
            explanation,
            native_resources,
            display_resources,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn register_with_token_and_display(
        &self,
        scope: &NativePermissionScope,
        observed_session_id: &str,
        token: String,
        native_request_id: String,
        action: String,
        explanation: Option<String>,
        native_resources: Vec<String>,
        display_resources: Vec<String>,
    ) -> Result<NativePermissionPromptView, NativePermissionRegistryError> {
        if observed_session_id != scope.native_session_id {
            return Err(NativePermissionRegistryError::ForeignSession);
        }
        if !(32..=128).contains(&token.len())
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
            || native_request_id.is_empty()
            || native_request_id.len() > 128
            || native_request_id.chars().any(char::is_control)
            || native_resources.len() > 64
            || display_resources.len() != native_resources.len().min(16)
            || display_resources.iter().any(|resource| {
                resource.is_empty()
                    || resource.len() > 128
                    || resource.chars().any(char::is_control)
            })
        {
            return Err(NativePermissionRegistryError::InvalidPrompt);
        }
        let action = permission_display_text(&action, 128)
            .filter(|value| !value.is_empty())
            .ok_or(NativePermissionRegistryError::InvalidPrompt)?;
        let explanation = explanation
            .as_deref()
            .and_then(|value| permission_display_text(value, 512));
        let mut prompts = self
            .prompts
            .lock()
            .map_err(|_| NativePermissionRegistryError::Poisoned)?;

        if let Some(existing) = prompts
            .values()
            .find(|prompt| prompt.scope == *scope && prompt.native_request_id == native_request_id)
        {
            if existing.view.action == action
                && existing.view.explanation == explanation
                && existing.native_resources == native_resources
            {
                return Ok(existing.view.clone());
            }
            return Err(NativePermissionRegistryError::ConflictingDuplicate);
        }

        if prompts.contains_key(&token) {
            return Err(NativePermissionRegistryError::TokenCollision);
        }
        if prompts.len() >= MAX_NATIVE_PERMISSION_PROMPTS_PER_RUNTIME
            || prompts
                .values()
                .filter(|prompt| prompt.scope == *scope)
                .count()
                >= MAX_NATIVE_PERMISSION_PROMPTS_PER_TURN
        {
            return Err(NativePermissionRegistryError::Capacity);
        }

        let sequence = next_permission_sequence(&prompts, scope)?;
        let view = NativePermissionPromptView {
            sequence,
            prompt_token: token.clone(),
            state: NativePermissionState::Pending,
            action,
            explanation,
            resource_count: native_resources.len(),
            resources: display_resources,
            decision: None,
        };
        prompts.insert(
            token,
            NativePermissionPrompt {
                scope: scope.clone(),
                native_request_id,
                native_resources,
                view: view.clone(),
                idempotency: Vec::new(),
            },
        );
        Ok(view)
    }

    fn register_mailbox(
        &self,
        scope: NativePermissionScope,
        sender: SyncSender<NativePermissionCommand>,
        bus: crate::transient_frames::TransientFrameBus,
    ) -> Result<NativePermissionMailboxRegistration, NativePermissionRegistryError> {
        let mut mailboxes = self
            .mailboxes
            .lock()
            .map_err(|_| NativePermissionRegistryError::Poisoned)?;
        if mailboxes.contains_key(&scope) {
            return Err(NativePermissionRegistryError::MailboxUnavailable);
        }
        mailboxes.insert(scope.clone(), sender);
        Ok(NativePermissionMailboxRegistration {
            registry: self.clone(),
            scope,
            bus,
        })
    }

    fn begin_reply(
        &self,
        scope: &NativePermissionScope,
        token: &str,
        principal_id: &str,
        idempotency_key: &str,
        decision: NativePermissionDecision,
    ) -> Result<NativePermissionReplyStart, NativePermissionRegistryError> {
        let mut prompts = self
            .prompts
            .lock()
            .map_err(|_| NativePermissionRegistryError::Poisoned)?;
        for (other_token, other) in &*prompts {
            if let Some((_, _, previous_decision)) = other
                .idempotency
                .iter()
                .find(|(principal, key, _)| principal == principal_id && key == idempotency_key)
            {
                if other_token == token && other.scope == *scope && *previous_decision == decision {
                    return Ok(NativePermissionReplyStart::Replay(other.view.clone()));
                }
                return Err(NativePermissionRegistryError::ReplyConflict);
            }
        }
        if prompts.values().any(|other| {
            other.scope == *scope
                && other.scope.native_session_id == scope.native_session_id
                && other.view.state == NativePermissionState::Replying
        }) {
            // One provider reply at a time preserves session-wide rejection semantics.
            return Err(NativePermissionRegistryError::ReplyConflict);
        }
        let sequence = next_permission_sequence(&prompts, scope)?;
        let prompt = prompts
            .get_mut(token)
            .filter(|prompt| prompt.scope == *scope)
            .ok_or(NativePermissionRegistryError::StalePrompt)?;

        if prompt.view.state != NativePermissionState::Pending {
            return Err(match prompt.view.state {
                NativePermissionState::SessionEnded => NativePermissionRegistryError::StalePrompt,
                _ => NativePermissionRegistryError::ReplyConflict,
            });
        }
        if prompt.idempotency.len() >= MAX_NATIVE_PERMISSION_IDEMPOTENCY_KEYS_PER_PROMPT {
            return Err(NativePermissionRegistryError::ReplyCapacity);
        }
        let sender = self
            .mailboxes
            .lock()
            .map_err(|_| NativePermissionRegistryError::Poisoned)?
            .get(scope)
            .cloned()
            .ok_or(NativePermissionRegistryError::MailboxUnavailable)?;

        prompt.view.state = NativePermissionState::Replying;
        prompt.view.decision = Some(decision.public_name().to_owned());
        prompt.view.sequence = sequence;
        prompt.idempotency.push((
            principal_id.to_owned(),
            idempotency_key.to_owned(),
            decision,
        ));
        Ok(NativePermissionReplyStart::Dispatch(sender))
    }

    fn rollback_unaccepted_reply(
        &self,
        scope: &NativePermissionScope,
        token: &str,
        principal_id: &str,
        idempotency_key: &str,
    ) {
        let Ok(mut prompts) = self.prompts.lock() else {
            return;
        };
        let sequence = next_permission_sequence(&prompts, scope).ok();
        if let Some(prompt) = prompts
            .get_mut(token)
            .filter(|prompt| prompt.scope == *scope)
            && prompt.view.state == NativePermissionState::Replying
        {
            prompt.view.state = NativePermissionState::Pending;
            prompt.view.decision = None;
            if let Some(sequence) = sequence {
                prompt.view.sequence = sequence;
            }
            prompt
                .idempotency
                .retain(|(principal, key, _)| principal != principal_id || key != idempotency_key);
        }
    }

    fn view_for_scope(
        &self,
        scope: &NativePermissionScope,
        token: &str,
    ) -> Option<NativePermissionPromptView> {
        self.prompts
            .lock()
            .ok()?
            .get(token)
            .filter(|prompt| prompt.scope == *scope)
            .map(|prompt| prompt.view.clone())
    }

    fn service_queued_commands(
        &self,
        server: &mut impl NativePermissionProvider,
        scope: &NativePermissionScope,
        receiver: &Receiver<NativePermissionCommand>,
        bus: &crate::transient_frames::TransientFrameBus,
    ) {
        for _ in 0..MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION {
            let command = match receiver.try_recv() {
                Ok(command) => command,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            self.apply_reply_command(server, scope, command, bus);
        }
    }

    fn apply_reply_command(
        &self,
        server: &mut impl NativePermissionProvider,
        scope: &NativePermissionScope,
        command: NativePermissionCommand,
        bus: &crate::transient_frames::TransientFrameBus,
    ) {
        let target = self.prompts.lock().ok().and_then(|prompts| {
            prompts.get(&command.prompt_token).and_then(|prompt| {
                (prompt.scope == *scope
                    && prompt.view.state == NativePermissionState::Replying
                    && prompt.view.decision.as_deref() == Some(command.decision.public_name()))
                .then(|| {
                    (
                        prompt.scope.native_session_id.clone(),
                        prompt.native_request_id.clone(),
                    )
                })
            })
        });
        let Some((native_session_id, native_request_id)) = target else {
            let _ = command.completed.send(());
            return;
        };

        let changed = if server
            .reply_session_permission(
                &native_session_id,
                &native_request_id,
                command.decision.provider_decision(),
            )
            .is_ok()
        {
            self.acknowledge_reply(scope, &command.prompt_token, command.decision)
        } else {
            // An error can mean the provider applied the reply but the acknowledgment
            // was lost. Read only the exact session's pending list; never replay POST.
            let pending_ids = server
                .list_session_permissions(&native_session_id)
                .ok()
                .and_then(|value| pending_permission_ids(&value, &native_session_id));
            self.reconcile_reply(scope, &command.prompt_token, command.decision, pending_ids)
        };
        for prompt in changed {
            publish_native_permission_prompt(bus, scope, &prompt);
        }
        let _ = command.completed.send(());
    }

    fn acknowledge_reply(
        &self,
        scope: &NativePermissionScope,
        token: &str,
        decision: NativePermissionDecision,
    ) -> Vec<NativePermissionPromptView> {
        let Ok(mut prompts) = self.prompts.lock() else {
            return Vec::new();
        };
        let mut tokens = prompts
            .iter()
            .filter_map(|(prompt_token, prompt)| {
                let affected = prompt.scope == *scope
                    && (prompt_token == token
                        || (decision == NativePermissionDecision::RejectSessionPending
                            && prompt.view.state == NativePermissionState::Pending));
                (affected
                    && matches!(
                        prompt.view.state,
                        NativePermissionState::Pending | NativePermissionState::Replying
                    ))
                .then(|| prompt_token.clone())
            })
            .collect::<Vec<_>>();
        tokens.sort();
        let mut changed = Vec::with_capacity(tokens.len());
        for prompt_token in tokens {
            let Ok(sequence) = next_permission_sequence(&prompts, scope) else {
                break;
            };
            if let Some(prompt) = prompts.get_mut(&prompt_token) {
                prompt.view.state = NativePermissionState::Answered;
                prompt.view.decision = Some(decision.public_name().to_owned());
                prompt.view.sequence = sequence;
                changed.push(prompt.view.clone());
            }
        }
        changed
    }

    fn reconcile_reply(
        &self,
        scope: &NativePermissionScope,
        token: &str,
        decision: NativePermissionDecision,
        pending_ids: Option<HashSet<String>>,
    ) -> Vec<NativePermissionPromptView> {
        let Ok(mut prompts) = self.prompts.lock() else {
            return Vec::new();
        };
        let mut tokens = prompts
            .iter()
            .filter_map(|(prompt_token, prompt)| {
                let affected = prompt.scope == *scope
                    && (prompt_token == token
                        || (decision == NativePermissionDecision::RejectSessionPending
                            && prompt.view.state == NativePermissionState::Pending));
                affected.then(|| prompt_token.clone())
            })
            .collect::<Vec<_>>();
        tokens.sort();
        let mut changed = Vec::with_capacity(tokens.len());
        for prompt_token in tokens {
            let Ok(sequence) = next_permission_sequence(&prompts, scope) else {
                break;
            };
            let Some(prompt) = prompts.get_mut(&prompt_token) else {
                continue;
            };
            let still_pending = pending_ids
                .as_ref()
                .is_some_and(|ids| ids.contains(&prompt.native_request_id));
            if still_pending {
                prompt.view.state = NativePermissionState::Pending;
                prompt.view.decision = None;
            } else {
                prompt.view.state = NativePermissionState::OutcomeUnknown;
                prompt.view.decision = Some(decision.public_name().to_owned());
            }
            prompt.view.sequence = sequence;
            changed.push(prompt.view.clone());
        }
        changed
    }

    fn scope_for_token(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        agent_session_id: &str,
        runtime_incarnation_id: &str,
        token: &str,
    ) -> Result<NativePermissionScope, NativePermissionRegistryError> {
        let prompts = self
            .prompts
            .lock()
            .map_err(|_| NativePermissionRegistryError::Poisoned)?;
        let prompt = prompts
            .get(token)
            .filter(|prompt| {
                prompt.scope.workspace_id == workspace_id
                    && prompt.scope.conversation_id == conversation_id
                    && prompt.scope.turn_id == turn_id
                    && prompt.scope.retry_ordinal == retry_ordinal
                    && prompt.scope.agent_session_id == agent_session_id
                    && prompt.scope.runtime_incarnation_id == runtime_incarnation_id
            })
            .ok_or(NativePermissionRegistryError::StalePrompt)?;
        Ok(prompt.scope.clone())
    }

    fn snapshot(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        agent_session_id: &str,
        runtime_incarnation_id: &str,
    ) -> NativePermissionAttemptSnapshot {
        let Ok(prompts) = self.prompts.lock() else {
            return NativePermissionAttemptSnapshot {
                sequence: 0,
                items: Vec::new(),
            };
        };
        let mut result = prompts
            .values()
            .filter(|prompt| {
                prompt.scope.workspace_id == workspace_id
                    && prompt.scope.conversation_id == conversation_id
                    && prompt.scope.turn_id == turn_id
                    && prompt.scope.retry_ordinal == retry_ordinal
                    && prompt.scope.agent_session_id == agent_session_id
                    && prompt.scope.runtime_incarnation_id == runtime_incarnation_id
            })
            .map(|prompt| prompt.view.clone())
            .collect::<Vec<_>>();
        result.sort_by(|left, right| left.sequence.cmp(&right.sequence));
        let sequence = result
            .iter()
            .map(|prompt| prompt.sequence)
            .max()
            .unwrap_or(0);
        NativePermissionAttemptSnapshot {
            sequence,
            items: result,
        }
    }

    fn workspace_snapshot(&self, workspace_id: &str) -> Vec<NativePermissionWorkspaceItemView> {
        let Ok(prompts) = self.prompts.lock() else {
            return Vec::new();
        };
        let mut result = prompts
            .values()
            .filter(|prompt| {
                prompt.scope.workspace_id == workspace_id
                    && matches!(
                        prompt.view.state,
                        NativePermissionState::Pending
                            | NativePermissionState::Replying
                            | NativePermissionState::OutcomeUnknown
                    )
            })
            .map(|prompt| NativePermissionWorkspaceItemView {
                conversation_id: prompt.scope.conversation_id.clone(),
                turn_id: prompt.scope.turn_id.clone(),
                retry_ordinal: prompt.scope.retry_ordinal,
                sequence: prompts
                    .values()
                    .filter(|candidate| candidate.scope == prompt.scope)
                    .map(|candidate| candidate.view.sequence)
                    .max()
                    .unwrap_or(0),
                agent_session_id: prompt.scope.agent_session_id.clone(),
                runtime_incarnation_id: prompt.scope.runtime_incarnation_id.clone(),
                prompt: prompt.view.clone(),
            })
            .collect::<Vec<_>>();
        result.sort_by(|left, right| {
            left.prompt
                .sequence
                .cmp(&right.prompt.sequence)
                .then_with(|| left.prompt.prompt_token.cmp(&right.prompt.prompt_token))
        });
        result
    }

    fn end_attempt(&self, scope: &NativePermissionScope) -> Vec<NativePermissionPromptView> {
        let Ok(mut prompts) = self.prompts.lock() else {
            return Vec::new();
        };
        let mut tokens = prompts
            .iter()
            .filter_map(|(token, prompt)| {
                (prompt.scope == *scope
                    && matches!(
                        prompt.view.state,
                        NativePermissionState::Pending
                            | NativePermissionState::Replying
                            | NativePermissionState::OutcomeUnknown
                    ))
                .then(|| token.clone())
            })
            .collect::<Vec<_>>();
        tokens.sort();
        let mut changed = Vec::with_capacity(tokens.len());
        for token in tokens {
            let Ok(sequence) = next_permission_sequence(&prompts, scope) else {
                break;
            };
            if let Some(prompt) = prompts.get_mut(&token) {
                prompt.view.state = NativePermissionState::SessionEnded;
                prompt.view.sequence = sequence;
                changed.push(prompt.view.clone());
            }
        }
        changed
    }

    fn discard_scope(&self, scope: &NativePermissionScope) {
        let Ok(mut prompts) = self.prompts.lock() else {
            return;
        };
        prompts.retain(|_, prompt| prompt.scope != *scope);
    }
}

fn next_permission_sequence(
    prompts: &HashMap<String, NativePermissionPrompt>,
    scope: &NativePermissionScope,
) -> Result<u32, NativePermissionRegistryError> {
    let last = prompts
        .values()
        .filter(|prompt| prompt.scope == *scope)
        .map(|prompt| prompt.view.sequence)
        .max()
        .unwrap_or(0);
    last.checked_add(1)
        .filter(|sequence| *sequence <= MAX_NATIVE_PERMISSION_SEQUENCE_PER_TURN)
        .ok_or(NativePermissionRegistryError::Capacity)
}

fn pending_permission_ids(
    value: &serde_json::Value,
    expected_session_id: &str,
) -> Option<HashSet<String>> {
    let items = value.get("data")?.as_array()?;
    if items.len() > MAX_NATIVE_PERMISSION_PROMPTS_PER_RUNTIME {
        return None;
    }
    let mut pending = HashSet::with_capacity(items.len());
    for item in items {
        let request_id = item.get("id")?.as_str()?;
        let session_id = item.get("sessionID")?.as_str()?;
        if request_id.is_empty()
            || request_id.len() > 128
            || request_id.chars().any(char::is_control)
            || session_id != expected_session_id
            || !pending.insert(request_id.to_owned())
        {
            return None;
        }
    }
    Some(pending)
}

enum NativePermissionReplyStart {
    Replay(NativePermissionPromptView),
    Dispatch(SyncSender<NativePermissionCommand>),
}

struct NativePermissionMailboxRegistration {
    registry: NativePermissionRegistry,
    scope: NativePermissionScope,
    bus: crate::transient_frames::TransientFrameBus,
}

impl Drop for NativePermissionMailboxRegistration {
    fn drop(&mut self) {
        for prompt in self.registry.end_attempt(&self.scope) {
            publish_native_permission_prompt(&self.bus, &self.scope, &prompt);
        }
        // Permission correlation is usable only while this exact native attempt is
        // alive. Retaining terminal rows would exhaust the Runtime cap across
        // sequential turns and leave stale tokens resident after session close.
        self.registry.discard_scope(&self.scope);
        if let Ok(mut mailboxes) = self.registry.mailboxes.lock() {
            mailboxes.remove(&self.scope);
        }
    }
}

struct NativePermissionObserver {
    registry: NativePermissionRegistry,
    scope: NativePermissionScope,
    bus: crate::transient_frames::TransientFrameBus,
    workspace_root: Option<PathBuf>,
}

impl NativePermissionObserver {
    fn new(
        registry: NativePermissionRegistry,
        scope: NativePermissionScope,
        bus: crate::transient_frames::TransientFrameBus,
        workspace_root: Option<PathBuf>,
    ) -> Self {
        Self {
            registry,
            scope,
            bus,
            workspace_root,
        }
    }
}

fn publish_native_permission_prompt(
    bus: &crate::transient_frames::TransientFrameBus,
    scope: &NativePermissionScope,
    prompt: &NativePermissionPromptView,
) {
    let frame = json!({
        "type": "turn.permission",
        "workspace_id": scope.workspace_id,
        "conversation_id": scope.conversation_id,
        "turn_id": scope.turn_id,
        "retry_ordinal": scope.retry_ordinal,
        "sequence": prompt.sequence,
        "prompt_token": prompt.prompt_token,
        "state": prompt.state_name(),
        "action": prompt.action,
        "explanation": prompt.explanation,
        "resource_count": prompt.resource_count,
        "resources": prompt.resources,
        "decision": prompt.decision,
    });
    if let Ok(bytes) = serde_json::to_vec(&frame) {
        bus.publish(crate::transient_frames::TransientFrame {
            workspace_id: scope.workspace_id.clone(),
            conversation_id: scope.conversation_id.clone(),
            bytes,
        });
    }
}

impl TurnObserver for NativePermissionObserver {
    fn on_event(&mut self, event: &NormalizedAgentEvent) {
        let Ok(prompt) = self
            .registry
            .register(&self.scope, event, self.workspace_root.as_deref())
        else {
            return;
        };
        publish_native_permission_prompt(&self.bus, &self.scope, &prompt);
    }

    fn finish(&mut self) {}
}

fn permission_display_text(raw: &str, max_bytes: usize) -> Option<String> {
    let mut cleaned = String::new();
    for character in raw.chars().filter(|character| !character.is_control()) {
        if cleaned.len() + character.len_utf8() > max_bytes {
            break;
        }
        cleaned.push(character);
    }
    let cleaned = cleaned.trim().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Show a native resource path only after proving it is an existing, non-link
/// path beneath the already-resolved Workspace root. The provider's path itself
/// remains private for exact pending-list reconciliation.
fn permission_resource_labels(workspace_root: Option<&Path>, resources: &[String]) -> Vec<String> {
    let canonical_root = workspace_root.and_then(|root| std::fs::canonicalize(root).ok());
    resources
        .iter()
        .take(16)
        .enumerate()
        .map(|(index, resource)| {
            let fallback = format!("Resource {}", index + 1);
            let Some(root) = canonical_root.as_deref() else {
                return fallback;
            };
            permission_resource_label(root, resource, index + 1).unwrap_or(fallback)
        })
        .collect()
}

fn permission_path_without_verbatim_prefix(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(value) = path.to_str() {
            let unc_prefix = "\\\\?\\UNC\\";
            if value
                .get(..unc_prefix.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(unc_prefix))
            {
                return PathBuf::from(format!("\\\\{}", &value[unc_prefix.len()..]));
            }
            if let Some(value) = value.strip_prefix("\\\\?\\") {
                return PathBuf::from(value);
            }
        }
    }
    path.to_path_buf()
}

fn permission_resource_label(root: &Path, raw: &str, ordinal: usize) -> Option<String> {
    if raw.is_empty() || raw.len() > 4_096 || raw.chars().any(char::is_control) {
        return None;
    }
    let root = permission_path_without_verbatim_prefix(root);
    let path = permission_path_without_verbatim_prefix(Path::new(raw));
    let relative = if path.is_absolute() {
        match path.strip_prefix(&root) {
            Ok(relative) => relative,
            Err(_) => return Some(format!("External resource {ordinal}")),
        }
    } else {
        path.as_path()
    };
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }

    let mut candidate = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        candidate.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&candidate).ok()?;
        if is_permission_resource_link_or_reparse(&metadata)
            || (index + 1 < components.len() && !metadata.is_dir())
        {
            return None;
        }
    }

    let display = relative.to_str()?.replace('\\', "/");
    if display.is_empty() || display.len() > 128 || display.chars().any(char::is_control) {
        return None;
    }
    Some(display)
}

#[cfg(windows)]
fn is_permission_resource_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_permission_resource_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

/// Closed diagnostic classes for failures after durable turn admission. Never add provider
/// or storage error text here: diagnostics are an operational index, not a detail channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DispatchFailureClass {
    HostResolution,
    BindingLookup,
    ProfileLookup,
    DispatchIntentClaim,
    DispatchIntentMissing,
    SessionReservation,
    ProviderExecution,
    Settlement,
    BlockingTask,
}

impl DispatchFailureClass {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::HostResolution => "HOST_RESOLUTION",
            Self::BindingLookup => "BINDING_LOOKUP",
            Self::ProfileLookup => "PROFILE_LOOKUP",
            Self::DispatchIntentClaim => "DISPATCH_INTENT_CLAIM",
            Self::DispatchIntentMissing => "DISPATCH_INTENT_MISSING",
            Self::SessionReservation => "SESSION_RESERVATION",
            Self::ProviderExecution => "PROVIDER_EXECUTION",
            Self::Settlement => "SETTLEMENT",
            Self::BlockingTask => "BLOCKING_TASK",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DispatchClaimReconciliation {
    Owned,
    NotOwned,
    Retry,
    MissingOrMalformed,
}

fn reconcile_dispatch_claim(
    intent: Option<&ConversationDispatchIntent>,
    local_incarnation_id: &str,
) -> DispatchClaimReconciliation {
    let Some(intent) = intent else {
        return DispatchClaimReconciliation::MissingOrMalformed;
    };
    if local_incarnation_id.trim().is_empty() {
        return DispatchClaimReconciliation::MissingOrMalformed;
    }
    match intent.state {
        ConversationDispatchState::Unclaimed => {
            if intent.claimed_by_runtime_incarnation.is_none() {
                DispatchClaimReconciliation::Retry
            } else {
                DispatchClaimReconciliation::MissingOrMalformed
            }
        }
        ConversationDispatchState::Claimed => match intent
            .claimed_by_runtime_incarnation
            .as_deref()
            .filter(|owner| !owner.trim().is_empty())
        {
            Some(owner) if owner == local_incarnation_id => DispatchClaimReconciliation::Owned,
            Some(_) => DispatchClaimReconciliation::NotOwned,
            None => DispatchClaimReconciliation::MissingOrMalformed,
        },
        ConversationDispatchState::Settled => DispatchClaimReconciliation::NotOwned,
    }
}

/// Retry only transient local-store failures. `settle_attempt_failed` re-reads durable
/// state on each attempt, so an acknowledgement lost after commit is recognized as an
/// already-settled terminal turn. A non-transient or exhausted failure stays typed for
/// startup recovery; error details never become diagnostics.
fn settle_dispatch_failure(
    failure: DispatchFailureClass,
    settle: impl FnMut() -> Result<(), storage_core::StoreError>,
) -> DispatchFailureClass {
    match retry_transient_store_commit(3, Duration::from_millis(100), settle) {
        Ok(()) => failure,
        Err(_) => DispatchFailureClass::Settlement,
    }
}

/// A claimed turn whose pinned host is unavailable has a known, non-provider outcome.
/// Retry transient local settlement failures so the current daemon can persist that outcome
/// without waiting for restart; if settlement still cannot be confirmed, the claimed
/// durable intent remains available to startup recovery.
fn settle_unavailable_host_attempt(
    settle: impl FnMut() -> Result<(), storage_core::StoreError>,
) -> Result<(), DispatchFailureClass> {
    retry_transient_store_commit(3, Duration::from_millis(100), settle)
        .map_err(|_| DispatchFailureClass::Settlement)
}

/// Host-normalized final-response identity. A native session handle must never
/// enter the durable commit key or its request payload. The existing internal
/// turn/retry scope disambiguates provider message identifiers across sessions.
fn provider_response_commit_identity(
    turn_id: &str,
    retry_ordinal: u32,
    message_id: &str,
) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(b"litecowork.response-commit.v1");
    digest.update((turn_id.len() as u64).to_be_bytes());
    digest.update(turn_id.as_bytes());
    digest.update(retry_ordinal.to_be_bytes());
    digest.update((message_id.len() as u64).to_be_bytes());
    digest.update(message_id.as_bytes());
    format!("response-sha256:{}", hex::encode(digest.finalize()))
}

fn now_utc_rfc3339() -> String {
    let now = time::OffsetDateTime::now_utc();
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| {
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                now.year(),
                now.month() as u8,
                now.day(),
                now.hour(),
                now.minute(),
                now.second()
            )
        })
}

fn generate_id(prefix: &str) -> String {
    let mut bytes = [0_u8; 16];
    if let Err(e) = getrandom::fill(&mut bytes) {
        eprintln!("WARNING: getrandom failed in generate_id: {e:?}");
        let nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
        return format!("{}_{:x}", prefix, nanos);
    }
    format!("{}_{}", prefix, hex::encode(bytes))
}

fn resolve_workspace_working_dir(
    store: &SqliteWorkspaceStore,
    principal_id: &str,
    workspace_id: &str,
    runtime_id: &str,
) -> Option<PathBuf> {
    use storage_core::{WorkspaceRootRevalidationBindings, WorkspaceRootStore};
    if let Ok(roots) = store.list_workspace_roots(workspace_id, Some("ACTIVE"), None, None, 1)
        && let Some(first) = roots.first()
        && let Ok(Some(candidate)) = store.get_workspace_root_scan_candidate(
            principal_id,
            workspace_id,
            &first.root.workspace_root_id,
            runtime_id,
            "default",
        )
    {
        match candidate.bindings {
            WorkspaceRootRevalidationBindings::CurrentIncarnation { locator, .. }
            | WorkspaceRootRevalidationBindings::Previous { locator, .. } => {
                // `private_locator` is an opaque encoded locator (for example
                // `unix-path-b64:`), never a filesystem path. Building a PathBuf
                // from the encoded string can never name an existing directory,
                // so the Workspace root was silently ignored and the agent child
                // ran in the daemon's own working directory instead.
                if let Some(path) = crate::local_filesystem::decode_workspace_root_locator_path(
                    &locator.private_locator,
                ) && path.is_dir()
                {
                    return Some(path);
                }
            }
            _ => {}
        }
    }
    // Fail closed. A turn that cannot resolve its own Workspace root must not run an
    // agent in an unrelated directory: the daemon's current directory is not an
    // authorization the user granted, and silently using it hid a decoding defect.
    None
}

/// Returns the exact CAS version committed by STARTING reservation and the next
/// AgentSession event revision for activation. SQLite stores versions as signed i64.
fn session_activation_versions(starting_version: u64) -> Option<(u64, u64)> {
    if starting_version == 0 {
        return None;
    }
    let active_version = starting_version.checked_add(1)?;
    if active_version > i64::MAX as u64 {
        return None;
    }
    Some((starting_version, active_version))
}

/// Live per-turn Stop signals keyed by `(turn_id, retry_ordinal)`.
type CancelRegistry = Arc<Mutex<HashMap<(String, u32), CancelSignal>>>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct DispatchAttemptKey {
    workspace_id: String,
    conversation_id: String,
    turn_id: String,
    retry_ordinal: u32,
}

/// Prevents exact idempotent request replays from running concurrent pre-claim setup
/// paths in the same Runtime. The durable dispatch-intent CAS remains the cross-process
/// owner; this registry closes the earlier in-process race before that CAS is reached.
#[derive(Clone, Default)]
struct DispatchRegistry {
    active: Arc<Mutex<HashSet<DispatchAttemptKey>>>,
}

impl DispatchRegistry {
    fn register(&self, key: DispatchAttemptKey) -> Option<DispatchRegistration> {
        let mut active = match self.active.lock() {
            Ok(active) => active,
            Err(poisoned) => {
                let active = poisoned.into_inner();
                self.active.clear_poison();
                active
            }
        };
        if !active.insert(key.clone()) {
            return None;
        }
        Some(DispatchRegistration {
            registry: self.clone(),
            key,
        })
    }
}

/// Releases an in-process dispatch owner when its blocking lifecycle exits, including
/// panic and cancellation paths. The guard is moved into the blocking task so dropping
/// an HTTP waiter cannot release ownership while native work is still running.
struct DispatchRegistration {
    registry: DispatchRegistry,
    key: DispatchAttemptKey,
}

impl Drop for DispatchRegistration {
    fn drop(&mut self) {
        let mut active = match self.registry.active.lock() {
            Ok(active) => active,
            Err(poisoned) => {
                let active = poisoned.into_inner();
                self.registry.active.clear_poison();
                active
            }
        };
        active.remove(&self.key);
    }
}

/// Removes a turn attempt's Stop signal when its dispatch ends, however it ends.
struct CancelRegistration {
    registry: CancelRegistry,
    key: (String, u32),
}

impl Drop for CancelRegistration {
    fn drop(&mut self) {
        if let Ok(mut registry) = self.registry.lock() {
            registry.remove(&self.key);
        }
    }
}

#[derive(Clone)]
pub struct AgentSessionSupervisor {
    store: SqliteWorkspaceStore,
    runtime_id: String,
    cancels: CancelRegistry,
    dispatches: DispatchRegistry,
    native_permissions: NativePermissionRegistry,
    host_supervisor: AgentHostSupervisor<OpenCodeServer>,
}

impl AgentSessionSupervisor {
    pub fn new(store: SqliteWorkspaceStore, runtime_id: String) -> Self {
        Self {
            store,
            runtime_id,
            cancels: Arc::new(Mutex::new(HashMap::new())),
            dispatches: DispatchRegistry::default(),
            native_permissions: NativePermissionRegistry::default(),
            host_supervisor: AgentHostSupervisor::default(),
        }
    }

    pub(crate) fn list_live_native_permissions(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        agent_session_id: &str,
        runtime_incarnation_id: &str,
    ) -> NativePermissionAttemptSnapshot {
        self.native_permissions.snapshot(
            workspace_id,
            conversation_id,
            turn_id,
            retry_ordinal,
            agent_session_id,
            runtime_incarnation_id,
        )
    }

    pub(crate) fn list_workspace_live_native_permissions(
        &self,
        workspace_id: &str,
    ) -> Vec<NativePermissionWorkspaceItemView> {
        self.native_permissions.workspace_snapshot(workspace_id)
    }

    pub(crate) async fn reply_live_native_permission(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        principal_id: &str,
        agent_session_id: &str,
        runtime_incarnation_id: &str,
        token: &str,
        idempotency_key: &str,
        decision: NativePermissionDecision,
    ) -> Result<NativePermissionPromptView, NativePermissionReplyError> {
        let scope = self
            .native_permissions
            .scope_for_token(
                workspace_id,
                conversation_id,
                turn_id,
                retry_ordinal,
                agent_session_id,
                runtime_incarnation_id,
                token,
            )
            .map_err(NativePermissionReplyError::from)?;
        match self
            .native_permissions
            .begin_reply(&scope, token, principal_id, idempotency_key, decision)
            .map_err(NativePermissionReplyError::from)?
        {
            NativePermissionReplyStart::Replay(view) => Ok(view),
            NativePermissionReplyStart::Dispatch(sender) => {
                let (completed, response) = tokio::sync::oneshot::channel();
                let command = NativePermissionCommand {
                    prompt_token: token.to_owned(),
                    decision,
                    completed,
                };
                if let Err(error) = sender.try_send(command) {
                    self.native_permissions.rollback_unaccepted_reply(
                        &scope,
                        token,
                        principal_id,
                        idempotency_key,
                    );
                    return Err(match error {
                        TrySendError::Full(_) => NativePermissionReplyError::Unavailable,
                        TrySendError::Disconnected(_) => NativePermissionReplyError::SessionEnded,
                    });
                }
                if let Some(view) = self.native_permissions.view_for_scope(&scope, token) {
                    publish_native_permission_prompt(
                        crate::transient_frames::transient_bus(),
                        &scope,
                        &view,
                    );
                }
                // Canceling the HTTP waiter does not cancel an already-admitted owner
                // command. The Runtime still settles its local state and never replays.
                let _ = response.await;
                self.native_permissions
                    .view_for_scope(&scope, token)
                    .ok_or(NativePermissionReplyError::SessionEnded)
            }
        }
    }

    /// Signals the live dispatch of one turn attempt to interrupt its native session.
    /// Returns `false` when no dispatch of this Runtime currently owns the attempt
    /// (for example after a restart), in which case the startup recovery sweep settles
    /// it. The signal is a request only; CANCELLED is committed after observed quiescence.
    pub fn request_cancel(&self, turn_id: &str, retry_ordinal: u32) -> bool {
        let Ok(registry) = self.cancels.lock() else {
            return false;
        };
        match registry.get(&(turn_id.to_owned(), retry_ordinal)) {
            Some(signal) => {
                signal.request();
                true
            }
            None => false,
        }
    }

    fn register_cancel(
        &self,
        turn_id: &str,
        retry_ordinal: u32,
    ) -> (CancelSignal, CancelRegistration) {
        let signal = CancelSignal::new();
        let key = (turn_id.to_owned(), retry_ordinal);
        if let Ok(mut registry) = self.cancels.lock() {
            registry.insert(key.clone(), signal.clone());
        }
        (
            signal,
            CancelRegistration {
                registry: Arc::clone(&self.cancels),
                key,
            },
        )
    }

    /// Dispatches a submitted ConversationTurn through the full native agent lifecycle.
    pub async fn dispatch_turn(
        &self,
        local_incarnation_id: &str,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn: ConversationTurn,
        correlation_id: &str,
    ) -> Result<(), DispatchFailureClass> {
        let Some(dispatch_registration) = self.dispatches.register(DispatchAttemptKey {
            workspace_id: workspace_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            turn_id: turn.turn_id.clone(),
            retry_ordinal: turn.retry_ordinal,
        }) else {
            // An exact idempotent replay is already owned by this Runtime. It must not
            // perform pre-claim lookups whose failure could settle the active attempt.
            return Ok(());
        };
        let store = self.store.clone();
        let runtime_id = self.runtime_id.clone();
        let supervisor = self.clone();
        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let conversation = conversation_id.to_owned();
        let turn_id = turn.turn_id.clone();
        let local_incarnation_id = local_incarnation_id.to_owned();
        let correlation = correlation_id.to_owned();
        let join_recovery_session_id = Arc::new(std::sync::OnceLock::<String>::new());
        let worker_recovery_session_id = Arc::clone(&join_recovery_session_id);
        let join_recovery_claimed = Arc::new(std::sync::OnceLock::<()>::new());
        let worker_recovery_claimed = Arc::clone(&join_recovery_claimed);
        let recovery_store = store.clone();
        let recovery_runtime_id = runtime_id.clone();
        let recovery_principal = principal.clone();
        let recovery_workspace = workspace.clone();
        let recovery_conversation = conversation.clone();
        let recovery_turn_id = turn_id.clone();
        let recovery_correlation = correlation.clone();

        let dispatch_task = tokio::task::spawn_blocking(move || {
            let _dispatch_registration = dispatch_registration;
            let mut session_id_for_failure: Option<String> = None;
            let mut owns_dispatch_intent = false;
            let dispatch_result = (|| -> Result<(), DispatchFailureClass> {
                let conv_store = SqliteConversationStore::new(store.clone());

                // The durable intent is the cross-Runtime ownership fence. Claim it
                // before any preflight lookup can fail and settle the attempt. A losing
                // dispatcher must leave the winning Runtime's live turn untouched.
                let now = now_utc_rfc3339();
                let claim_result = conv_store.claim_conversation_dispatch_intent(
                    &turn_id,
                    turn.retry_ordinal,
                    &local_incarnation_id,
                    &now,
                );
                let claim_resolution = match claim_result {
                    Ok(true) => DispatchClaimReconciliation::Owned,
                    Ok(false) | Err(_) => {
                        match conv_store
                            .get_conversation_dispatch_intent(&turn_id, turn.retry_ordinal)
                        {
                            Ok(intent) => {
                                reconcile_dispatch_claim(intent.as_ref(), &local_incarnation_id)
                            }
                            Err(StoreError::Integrity(_) | StoreError::Invalid(_)) => {
                                DispatchClaimReconciliation::MissingOrMalformed
                            }
                            Err(_) => DispatchClaimReconciliation::Retry,
                        }
                    }
                };
                match claim_resolution {
                    DispatchClaimReconciliation::Owned => {}
                    DispatchClaimReconciliation::NotOwned => return Ok(()),
                    DispatchClaimReconciliation::Retry => {
                        return Err(DispatchFailureClass::DispatchIntentClaim);
                    }
                    DispatchClaimReconciliation::MissingOrMalformed => {
                        return Err(DispatchFailureClass::DispatchIntentMissing);
                    }
                }
                owns_dispatch_intent = true;
                let _ = worker_recovery_claimed.set(());
                let (cancel, _cancel_registration) =
                    supervisor.register_cancel(&turn_id, turn.retry_ordinal);

                // 1. Resolve host for turn
                let host_opt = conv_store
                    .resolve_conversation_session_host_for_turn(
                        &principal,
                        &workspace,
                        &conversation,
                        &turn_id,
                    )
                    .map_err(|_| DispatchFailureClass::HostResolution)?;

                let Some(host) = host_opt else {
                    // No eligible host: settle the attempt FAILED (AGENT_UNAVAILABLE).
                    settle_unavailable_host_attempt(|| {
                        settle_attempt_failed(
                            &store,
                            &runtime_id,
                            &principal,
                            &workspace,
                            &conversation,
                            &turn_id,
                            None,
                            "AGENT_UNAVAILABLE",
                            &correlation,
                            None,
                        )
                    })?;
                    return Ok(());
                };

                // 2. Query provider key and profile via published AgentCatalogStore trait
                let binding = store
                    .get_agent_binding(&principal, &workspace, &host.agent_binding_id)
                    .map_err(|_| DispatchFailureClass::BindingLookup)?
                    .ok_or(DispatchFailureClass::BindingLookup)?;

                let profiles = store
                    .list_agent_profiles(&principal, &workspace, &now)
                    .map_err(|_| DispatchFailureClass::ProfileLookup)?;

                let provider_key = profiles
                    .iter()
                    .find(|p| p.profile.agent_profile_id == binding.agent_profile_id)
                    .map(|p| p.profile.provider_key.clone())
                    .ok_or(DispatchFailureClass::ProfileLookup)?;

                // 4. Reserve STARTING session
                let session_id = generate_id("ses");
                // This path currently launches one locally owned process per session.
                // Give each process a distinct durable host identity; a terminal host
                // record is never recycled as a live process on a later turn.
                let host_instance_id = format!("host-{session_id}");
                session_id_for_failure = Some(session_id.clone());
                let _ = worker_recovery_session_id.set(session_id.clone());
                let start = ConversationAgentSessionStart {
                    request: WorkspaceCreateRequest {
                        principal_id: principal.clone(),
                        request_id: format!("req-start-{}", session_id),
                        request_payload: json!({ "agent_session_id": session_id }),
                    },
                    session: AgentSessionRecord {
                        agent_session_id: session_id.clone(),
                        workspace_id: workspace.clone(),
                        scope_kind: "CONVERSATION".to_owned(),
                        conversation_id: Some(conversation.clone()),
                        conversation_turn_id: Some(turn_id.clone()),
                        task_id: None,
                        task_spec_revision: None,
                        attempt_id: None,
                        agent_binding_id: host.agent_binding_id.clone(),
                        endpoint_id: host.endpoint_id.clone(),
                        runtime_id: host.runtime_id.clone(),
                        runtime_incarnation_id: host.runtime_incarnation_id.clone(),
                        configuration_digest: None,
                        harness_descriptor_digest: None,
                        status: "STARTING".to_owned(),
                        started_at: now.clone(),
                        last_event_at: Some(now.clone()),
                        closed_at: None,
                        version: 1,
                    },
                    event: EventDraft {
                        event_id: generate_id("ev"),
                        workspace_id: workspace.clone(),
                        entity_type: "AgentSession".to_owned(),
                        entity_id: session_id.clone(),
                        origin_runtime_id: runtime_id.clone(),
                        entity_revision: 1,
                        hlc_timestamp: now.clone(),
                        correlation_id: correlation.clone(),
                        causation_id: None,
                        schema_version: 1,
                        event_type: "agent.session.starting.v1".to_owned(),
                        payload: json!({
                            "agent_session_id": session_id,
                            "task_spec_revision": null,
                            "scope": {
                                "kind": "CONVERSATION",
                                "conversation_id": conversation,
                                "conversation_turn_id": turn_id,
                            },
                            "agent_binding_id": host.agent_binding_id,
                            "endpoint_id": host.endpoint_id,
                            "runtime_id": host.runtime_id,
                            "runtime_incarnation_id": host.runtime_incarnation_id,
                            "session_state": "STARTING",
                            "reported_at": now,
                        }),
                        recorded_at: now.clone(),
                    },
                };

                let committed_session = store
                    .start_conversation_agent_session(start)
                    .map_err(|_| DispatchFailureClass::SessionReservation)?;

                // 5. Execute native interaction based on provider_key
                let run_res = execute_agent_turn(
                    &store,
                    &runtime_id,
                    &principal,
                    &workspace,
                    &conversation,
                    &turn_id,
                    turn.version,
                    turn.retry_ordinal,
                    &session_id,
                    committed_session.session.version,
                    &host_instance_id,
                    &host.agent_binding_id,
                    &provider_key,
                    &binding.agent_profile_id,
                    &host.endpoint_id,
                    &host.runtime_id,
                    &host.runtime_incarnation_id,
                    &correlation,
                    &cancel,
                    &supervisor.native_permissions,
                    &supervisor.host_supervisor,
                );
                // Transient presentation control frame: a committed message supersedes the
                // transient text; any other end leaves it marked incomplete in the UI.
                {
                    let committed = matches!(run_res, Ok(TurnOutcome::Committed))
                        .then(|| format!("msg-agent-{turn_id}-{}", turn.retry_ordinal));
                    crate::transient_frames::TurnFramePublisher::new(
                        crate::transient_frames::transient_bus().clone(),
                        &workspace,
                        &conversation,
                        &turn_id,
                        turn.retry_ordinal,
                    )
                    .settle(committed.as_deref());
                }
                match run_res {
                    Ok(_outcome) => {}
                    Err(_err) => {
                        // Settle whatever the store currently holds: the attempt FAILED and
                        // its session LOST, so Retry can reserve a fresh session. A turn the
                        // user already cancelled or a settled one is left untouched.
                        settle_attempt_failed(
                            &store,
                            &runtime_id,
                            &principal,
                            &workspace,
                            &conversation,
                            &turn_id,
                            Some(&session_id),
                            "CONVERSATION_SESSION_ADMISSION_UNAVAILABLE",
                            &correlation,
                            None,
                        )
                        .map_err(|_| DispatchFailureClass::Settlement)?;
                        // Preserve only a closed classification; the error can contain native
                        // provider text, prompt fragments, or implementation details.
                        return Err(DispatchFailureClass::ProviderExecution);
                    }
                }

                Ok(())
            })();

            if let Err(failure) = dispatch_result {
                if !owns_dispatch_intent {
                    // A failed/ambiguous CAS does not prove ownership. Keep the durable
                    // intent for reconciliation; this dispatcher may not settle it.
                    return Err(failure);
                }
                // Every post-admission setup/dispatch error attempts to settle the exact
                // attempt. If storage is unavailable, the durable intent remains fenced
                // for the existing startup recovery sweep; report only a typed class.
                return Err(settle_dispatch_failure(failure, || {
                    settle_attempt_failed(
                        &store,
                        &runtime_id,
                        &principal,
                        &workspace,
                        &conversation,
                        &turn_id,
                        session_id_for_failure.as_deref(),
                        "CONVERSATION_SESSION_ADMISSION_UNAVAILABLE",
                        &correlation,
                        None,
                    )
                }));
            }
            Ok(())
        });

        join_dispatch_or_recover(dispatch_task.await, move || {
            if join_recovery_claimed.get().is_none() {
                // A panic before a confirmed claim is not authority to settle another
                // Runtime's attempt. The durable startup sweep owns reconciliation.
                return DispatchFailureClass::BlockingTask;
            }
            let session_id = join_recovery_session_id.get().cloned();
            recover_blocking_dispatch_failure(
                &recovery_store,
                &recovery_runtime_id,
                &recovery_principal,
                &recovery_workspace,
                &recovery_conversation,
                &recovery_turn_id,
                session_id.as_deref(),
                &recovery_correlation,
            )
        })
        .await
    }
}

/// A failed blocking task has ended, so its exact admitted turn can be settled without
/// guessing whether the native session is still active. The session identity is held
/// Runtime-locally and is used only to close a reservation that may have committed just
/// before the worker failed. If settlement storage is unavailable, its durable intent
/// remains for startup recovery.
pub(crate) fn recover_blocking_dispatch_failure(
    store: &SqliteWorkspaceStore,
    runtime_id: &str,
    principal_id: &str,
    workspace_id: &str,
    conversation_id: &str,
    turn_id: &str,
    session_id: Option<&str>,
    correlation_id: &str,
) -> DispatchFailureClass {
    settle_dispatch_failure(DispatchFailureClass::BlockingTask, || {
        settle_attempt_failed(
            store,
            runtime_id,
            principal_id,
            workspace_id,
            conversation_id,
            turn_id,
            session_id,
            "CONVERSATION_SESSION_ADMISSION_UNAVAILABLE",
            correlation_id,
            None,
        )
    })
}

/// Converts a blocking-worker panic/cancellation into an exact-attempt reconciliation.
/// The JoinError itself is never formatted because its payload is outside the diagnostic
/// contract and may contain implementation or provider details.
pub(crate) async fn join_dispatch_or_recover(
    joined: Result<Result<(), DispatchFailureClass>, tokio::task::JoinError>,
    recover: impl FnOnce() -> DispatchFailureClass + Send + 'static,
) -> Result<(), DispatchFailureClass> {
    match joined {
        Ok(result) => result,
        Err(_) => {
            let recovered = tokio::task::spawn_blocking(recover)
                .await
                .unwrap_or(DispatchFailureClass::Settlement);
            Err(recovered)
        }
    }
}

/// How a dispatched turn attempt ended when it did not fail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TurnOutcome {
    /// The final response was committed atomically (COMPLETED).
    Committed,
    /// A requested Stop was followed by observed quiescence and CANCELLED was committed.
    Cancelled,
    /// A Stop was requested but quiescence was not observed in time. The turn stays
    /// CANCEL_REQUESTED and the startup recovery sweep owns the rest; nothing is faked.
    StopUnconfirmed,
}

#[derive(Clone, Copy)]
pub(crate) struct TurnSettlementScope<'a> {
    pub(crate) store: &'a SqliteWorkspaceStore,
    pub(crate) runtime_id: &'a str,
    pub(crate) principal_id: &'a str,
    pub(crate) workspace_id: &'a str,
    pub(crate) conversation_id: &'a str,
    pub(crate) turn_id: &'a str,
    pub(crate) session_id: &'a str,
    pub(crate) correlation_id: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ObservedTurnDisposition {
    Response(String),
    Cancelled,
    Unconfirmed,
    Failed {
        reason_code: &'static str,
        settlement_succeeded: bool,
    },
}

pub(crate) fn apply_observed_turn(
    scope: TurnSettlementScope<'_>,
    observation: Result<TurnObservation, TurnFailure>,
    usage: Option<ConversationTurnUsage>,
) -> Result<ObservedTurnDisposition, String> {
    match observation {
        Ok(TurnObservation::Completed { answer_text }) => {
            Ok(ObservedTurnDisposition::Response(answer_text))
        }
        Ok(TurnObservation::Stopped) => {
            settle_attempt_cancelled(
                scope.store,
                scope.runtime_id,
                scope.principal_id,
                scope.workspace_id,
                scope.conversation_id,
                scope.turn_id,
                scope.session_id,
                scope.correlation_id,
                usage,
            )?;
            Ok(ObservedTurnDisposition::Cancelled)
        }
        Ok(TurnObservation::StopUnconfirmed) => Ok(ObservedTurnDisposition::Unconfirmed),
        Ok(TurnObservation::Failed { reason_code }) | Err(reason_code) => {
            let failure_settlement = settle_attempt_failed(
                scope.store,
                scope.runtime_id,
                scope.principal_id,
                scope.workspace_id,
                scope.conversation_id,
                scope.turn_id,
                Some(scope.session_id),
                reason_code.reason_code(),
                scope.correlation_id,
                usage,
            );
            Ok(ObservedTurnDisposition::Failed {
                reason_code: reason_code.reason_code(),
                settlement_succeeded: failure_settlement.is_ok(),
            })
        }
    }
}

/// Keeps the Runtime-local host row from claiming READY after the per-turn
/// process has been dropped on an early return. The guard is declared before
/// the adapter so the adapter's process owner drops first.
struct ReadyHostStateGuard {
    store: SqliteWorkspaceStore,
    runtime_id: String,
    runtime_incarnation_id: String,
    host_instance_id: String,
    state: &'static str,
    active: bool,
}

impl ReadyHostStateGuard {
    fn new(
        store: SqliteWorkspaceStore,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        host_instance_id: &str,
    ) -> Self {
        Self {
            store,
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: runtime_incarnation_id.to_owned(),
            host_instance_id: host_instance_id.to_owned(),
            state: "READY",
            active: true,
        }
    }

    fn mark_busy(&mut self) {
        self.state = "BUSY";
    }

    /// A durable CLOSED-session transaction returns the host row to READY.
    /// The per-session process is still owned here until observed shutdown.
    fn mark_ready_after_session_close(&mut self) {
        self.state = "READY";
    }

    fn mark_degraded_after_session_loss(&mut self) -> Result<(), String> {
        self.transition("DEGRADED")
    }

    fn begin_stopping(&mut self) -> Result<(), String> {
        self.transition("STOPPING")
    }

    fn finish_stopped(&mut self) -> Result<(), String> {
        self.transition("STOPPED")?;
        self.active = false;
        Ok(())
    }

    fn retain_stopping_pending(&mut self) {
        // STOPPING remains durable when exit is unknown; startup reconciliation
        // owns any later retry and must not infer success from this stack.
        self.active = false;
    }

    fn retain_live_session_reference(&mut self) {
        // StopUnconfirmed leaves the AgentSession and its durable host binding
        // nonterminal. Keep the host's last confirmed BUSY state; a stack guard
        // must not mark it DEGRADED merely because this worker returned.
        self.active = false;
    }

    fn transition(&mut self, next_state: &'static str) -> Result<(), String> {
        self.store
            .transition_agent_host_instance(
                &self.runtime_id,
                &self.runtime_incarnation_id,
                &self.host_instance_id,
                self.state,
                next_state,
                &now_utc_rfc3339(),
                None,
            )
            .map_err(|_| "Could not reconcile the native host process state".to_owned())?;
        self.state = next_state;
        Ok(())
    }
}

impl Drop for ReadyHostStateGuard {
    fn drop(&mut self) {
        if self.active && self.state != "STOPPING" {
            let _ = self.store.transition_agent_host_instance(
                &self.runtime_id,
                &self.runtime_incarnation_id,
                &self.host_instance_id,
                self.state,
                "DEGRADED",
                &now_utc_rfc3339(),
                None,
            );
        }
    }
}

fn status_wire(status: ConversationTurnStatus) -> &'static str {
    match status {
        ConversationTurnStatus::Open => "OPEN",
        ConversationTurnStatus::Running => "RUNNING",
        ConversationTurnStatus::WaitingUser => "WAITING_USER",
        ConversationTurnStatus::WaitingDependency => "WAITING_DEPENDENCY",
        ConversationTurnStatus::Completed => "COMPLETED",
        ConversationTurnStatus::Failed => "FAILED",
        ConversationTurnStatus::CancelRequested => "CANCEL_REQUESTED",
        ConversationTurnStatus::Cancelled => "CANCELLED",
    }
}

/// UTC timestamp in the storage layer's canonical nine-digit fractional form.
pub(crate) fn canonical_now() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.nanosecond()
    )
}

/// Schema-shaped `agent.session.closed.v1` / `agent.session.lost.v1` payload.
fn session_end_event(
    workspace_id: &str,
    runtime_id: &str,
    session: &AgentSessionRecord,
    end: ConversationSessionEnd,
    correlation_id: &str,
    now: &str,
) -> EventDraft {
    let (event_type, state) = match end {
        ConversationSessionEnd::Closed => ("agent.session.closed.v1", "CLOSED"),
        ConversationSessionEnd::Lost => ("agent.session.lost.v1", "LOST"),
    };
    EventDraft {
        event_id: generate_id("ev"),
        workspace_id: workspace_id.to_owned(),
        entity_type: "AgentSession".to_owned(),
        entity_id: session.agent_session_id.clone(),
        origin_runtime_id: runtime_id.to_owned(),
        entity_revision: session.version + 1,
        hlc_timestamp: now.to_owned(),
        correlation_id: correlation_id.to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: event_type.to_owned(),
        payload: json!({
            "agent_session_id": session.agent_session_id,
            "scope": {
                "kind": "CONVERSATION",
                "conversation_id": session.conversation_id,
                "conversation_turn_id": session.conversation_turn_id,
            },
            "task_spec_revision": null,
            "agent_binding_id": session.agent_binding_id,
            "endpoint_id": session.endpoint_id,
            "runtime_id": session.runtime_id,
            "runtime_incarnation_id": session.runtime_incarnation_id,
            "session_state": state,
            "reported_at": now,
        }),
        recorded_at: now.to_owned(),
    }
}

/// Settles a turn attempt FAILED (no answer is committed) and ends its AgentSession
/// LOST, from whatever state the store currently holds, so Retry can reserve a fresh
/// session. A turn that is already settled is left untouched; only a session still
/// STARTING behind it is released.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settle_attempt_failed(
    store: &SqliteWorkspaceStore,
    runtime_id: &str,
    principal_id: &str,
    workspace_id: &str,
    conversation_id: &str,
    turn_id: &str,
    session_id: Option<&str>,
    reason_code: &str,
    correlation_id: &str,
    usage: Option<ConversationTurnUsage>,
) -> Result<(), storage_core::StoreError> {
    let conv_store = SqliteConversationStore::new(store.clone());
    let turn = conv_store
        .get_conversation_turn(workspace_id, conversation_id, turn_id)?
        .ok_or(storage_core::StoreError::NotFound)?;
    let session = match session_id.or(turn.agent_session_id.as_deref()) {
        Some(id) => store.get_agent_session(workspace_id, id)?,
        None => None,
    }
    .filter(|record| !matches!(record.status.as_str(), "CLOSED" | "LOST"));
    let now = canonical_now();
    if turn.status.is_terminal() {
        if let Some(record) = session.filter(|record| record.status == "STARTING") {
            let event = session_end_event(
                workspace_id,
                runtime_id,
                &record,
                ConversationSessionEnd::Lost,
                correlation_id,
                &now,
            );
            store.mark_starting_conversation_agent_session_lost(
                MarkStartingConversationAgentSessionLost {
                    workspace_id: workspace_id.to_owned(),
                    agent_session_id: record.agent_session_id.clone(),
                    expected_version: record.version,
                    occurred_at: now.clone(),
                    event,
                },
            )?;
        }
        return Ok(());
    }
    let mut payload = json!({
        "turn_id": turn_id,
        "from": status_wire(turn.status),
        "to": "FAILED",
        "reason_code": reason_code,
        "aggregate_version": turn.version + 1,
    });
    if let Some(pointer) = turn.agent_session_id.as_deref() {
        payload["agent_session_id"] = json!(pointer);
    }
    let turn_event = EventDraft {
        event_id: generate_id("ev"),
        workspace_id: workspace_id.to_owned(),
        entity_type: "ConversationTurn".to_owned(),
        entity_id: turn_id.to_owned(),
        origin_runtime_id: runtime_id.to_owned(),
        entity_revision: turn.version + 1,
        hlc_timestamp: now.clone(),
        correlation_id: correlation_id.to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "conversation.turn.settled.v1".to_owned(),
        payload,
        recorded_at: now.clone(),
    };
    let ended = session.map(|record| SettleConversationSession {
        expected_session_version: record.version,
        end: ConversationSessionEnd::Lost,
        occurred_at: now.clone(),
        session_event: session_end_event(
            workspace_id,
            runtime_id,
            &record,
            ConversationSessionEnd::Lost,
            correlation_id,
            &now,
        ),
        agent_session_id: record.agent_session_id,
    });
    conv_store
        .settle_conversation_turn(SettleConversationTurn {
            control_event: None,
            principal_id: principal_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            turn_id: turn_id.to_owned(),
            expected_turn_version: turn.version,
            command: TurnCommand::Fail {
                settled_at: now.clone(),
            },
            turn_event,
            session: ended,
            usage,
        })
        .map(|_| ())
}

/// Commits CANCELLED for a CANCEL_REQUESTED turn after the native session was observed
/// quiescent, closing its AgentSession in the same transaction. Fails (so the caller
/// takes the failure path) if the turn is not CANCEL_REQUESTED.
#[allow(clippy::too_many_arguments)]
fn settle_attempt_cancelled(
    store: &SqliteWorkspaceStore,
    runtime_id: &str,
    principal_id: &str,
    workspace_id: &str,
    conversation_id: &str,
    turn_id: &str,
    session_id: &str,
    correlation_id: &str,
    usage: Option<ConversationTurnUsage>,
) -> Result<(), String> {
    let conv_store = SqliteConversationStore::new(store.clone());
    let turn = conv_store
        .get_conversation_turn(workspace_id, conversation_id, turn_id)
        .map_err(|e| format!("Read turn error: {e:?}"))?
        .ok_or_else(|| "ConversationTurn not found".to_owned())?;
    if turn.status != ConversationTurnStatus::CancelRequested {
        return Err(format!(
            "native stop observed but the turn is {} rather than CANCEL_REQUESTED",
            status_wire(turn.status)
        ));
    }
    let session = store
        .get_agent_session(workspace_id, session_id)
        .map_err(|e| format!("Read session error: {e:?}"))?
        .ok_or_else(|| "AgentSession not found".to_owned())?;
    let now = canonical_now();
    let mut payload = json!({
        "turn_id": turn_id,
        "from": "CANCEL_REQUESTED",
        "to": "CANCELLED",
        "reason_code": "USER_REQUESTED",
        "aggregate_version": turn.version + 1,
    });
    if let Some(pointer) = turn.agent_session_id.as_deref() {
        payload["agent_session_id"] = json!(pointer);
    }
    let turn_event = EventDraft {
        event_id: generate_id("ev"),
        workspace_id: workspace_id.to_owned(),
        entity_type: "ConversationTurn".to_owned(),
        entity_id: turn_id.to_owned(),
        origin_runtime_id: runtime_id.to_owned(),
        entity_revision: turn.version + 1,
        hlc_timestamp: now.clone(),
        correlation_id: correlation_id.to_owned(),
        causation_id: None,
        schema_version: 1,
        event_type: "conversation.turn.settled.v1".to_owned(),
        payload,
        recorded_at: now.clone(),
    };
    conv_store
        .settle_conversation_turn(SettleConversationTurn {
            control_event: None,
            principal_id: principal_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            turn_id: turn_id.to_owned(),
            expected_turn_version: turn.version,
            command: TurnCommand::SettleCancelled {
                settled_at: now.clone(),
            },
            turn_event,
            session: Some(SettleConversationSession {
                expected_session_version: session.version,
                end: ConversationSessionEnd::Closed,
                occurred_at: now.clone(),
                session_event: session_end_event(
                    workspace_id,
                    runtime_id,
                    &session,
                    ConversationSessionEnd::Closed,
                    correlation_id,
                    &now,
                ),
                agent_session_id: session.agent_session_id,
            }),
            usage,
        })
        .map(|_| ())
        .map_err(|e| format!("Settle cancelled turn error: {e:?}"))
}

/// B-10 usage attribution: provider-reported tokens when the native stream carried
/// non-zero counts, else a local estimate of `max(1, bytes / 4)` per direction from the
/// bytes actually exchanged, else UNKNOWN (`None`). A zero-token report is not treated
/// as provider-reported usage. No cost is inferred.
fn turn_usage(
    reported: super::adapter::ReportedUsage,
    prompt_bytes: usize,
    output_bytes: usize,
) -> Option<ConversationTurnUsage> {
    if reported.observed && (reported.input_tokens > 0 || reported.output_tokens > 0) {
        return Some(ConversationTurnUsage {
            attribution_source: UsageAttributionSource::ProviderReported,
            input_tokens: reported.input_tokens,
            output_tokens: reported.output_tokens,
        });
    }
    if prompt_bytes == 0 && output_bytes == 0 {
        return None;
    }
    let estimate = |bytes: usize| {
        if bytes == 0 {
            0
        } else {
            (bytes as u64 / 4).max(1)
        }
    };
    Some(ConversationTurnUsage {
        attribution_source: UsageAttributionSource::EstimatedLocal,
        input_tokens: estimate(prompt_bytes),
        output_tokens: estimate(output_bytes),
    })
}

/// Runs one store commit and retries only transient storage failures. Every retry
/// repeats the same deterministic commit, so an earlier attempt whose acknowledgement
/// was lost is answered from its stored receipt (exact replay) rather than written
/// twice, and the provider prompt is never resent to repair a local write (T06).
fn retry_transient_store_commit<T>(
    attempts: usize,
    backoff: Duration,
    mut commit: impl FnMut() -> Result<T, storage_core::StoreError>,
) -> Result<T, storage_core::StoreError> {
    use storage_core::StoreError;
    let mut last = None;
    for attempt in 0..attempts.max(1) {
        match commit() {
            Ok(value) => return Ok(value),
            Err(
                error @ (StoreError::Busy
                | StoreError::Database(_)
                | StoreError::Io(_)
                | StoreError::Blob(_)
                | StoreError::ExecutorStopped),
            ) => {
                last = Some(error);
                if attempt + 1 < attempts {
                    std::thread::sleep(backoff);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(last.unwrap_or(StoreError::ExecutorStopped))
}

// Twelve distinct turn-identity inputs; a parameter struct would only relabel
// them at this executor's single call site in this module.
#[allow(clippy::too_many_arguments)]
fn execute_agent_turn(
    store: &SqliteWorkspaceStore,
    runtime_id: &str,
    principal_id: &str,
    workspace_id: &str,
    conversation_id: &str,
    turn_id: &str,
    turn_version: u64,
    retry_ordinal: u32,
    session_id: &str,
    starting_session_version: u64,
    host_instance_id: &str,
    agent_binding_id: &str,
    provider_key: &str,
    agent_profile_id: &str,
    endpoint_id: &str,
    host_runtime_id: &str,
    host_runtime_incarnation_id: &str,
    correlation_id: &str,
    cancel: &CancelSignal,
    native_permissions: &NativePermissionRegistry,
    host_supervisor: &AgentHostSupervisor<OpenCodeServer>,
) -> Result<TurnOutcome, String> {
    let conv_store = SqliteConversationStore::new(store.clone());

    // Parse the stored provider key once at the boundary. The canonical stored
    // form is upper case (`OPENCODE`); an unknown key fails closed below.
    let provider = NativeProviderKey::from_profile_key(provider_key);

    // Native session establishment
    match provider {
        Some(NativeProviderKey::OpenCode) => {
            // Dispatch exactly the executable the probe registered for this
            // endpoint and re-validate it is the same canonical path. Probe or
            // dispatch executable drift fails the turn closed with no native spawn.
            let now = now_utc_rfc3339();
            let binding = store
                .get_local_endpoint_binding(
                    host_runtime_id,
                    host_runtime_incarnation_id,
                    endpoint_id,
                    &now,
                )
                .map_err(|e| format!("Query endpoint binding error: {e:?}"))?
                .ok_or_else(|| {
                    "no fresh Runtime-local endpoint binding for the OpenCode endpoint".to_owned()
                })?;
            let registered = PathBuf::from(&binding.endpoint_ref);
            let canonical_registered = std::fs::canonicalize(&registered).map_err(|_| {
                "registered OpenCode endpoint executable no longer resolves".to_owned()
            })?;
            if !canonical_registered.is_file() {
                return Err("registered OpenCode endpoint executable is not a file".to_owned());
            }
            // Re-resolve through the shared resolver and require the same canonical
            // path, so a changed PATH or a replaced executable fails closed.
            let resolved = crate::agents::resolve_opencode_executable()
                .ok_or_else(|| "opencode executable not found".to_owned())?;
            if resolved != canonical_registered {
                return Err(
                    "OpenCode executable drifted from the registered endpoint reference".to_owned(),
                );
            }
            let exe = canonical_registered;
            let working_dir =
                resolve_workspace_working_dir(store, principal_id, workspace_id, runtime_id)
                    .ok_or_else(|| {
                        "no active Workspace root resolves to an existing directory".to_owned()
                    })?;
            // The child is given the same bounded variable allow list that the adapter
            // clears the environment to, sourced from this process. Without it the agent
            // child starts with no HOME and no PATH and cannot run at all.
            let config = OpenCodeServerConfig::bounded(exe, working_dir.clone())
                .with_protocol(OpenCodeServerProtocol::V2_0_32)
                .with_environment(OpenCodeEnvironment::from_current_process());

            let host_started_at = now_utc_rfc3339();
            let mut native_start_error = None;
            let server = start_host_process(
                store,
                AgentHostInstanceRecord {
                    host_instance_id: host_instance_id.to_owned(),
                    runtime_id: host_runtime_id.to_owned(),
                    runtime_incarnation_id: host_runtime_incarnation_id.to_owned(),
                    agent_profile_id: agent_profile_id.to_owned(),
                    endpoint_id: endpoint_id.to_owned(),
                    hosting_mode: "LOCAL_PER_SESSION".to_owned(),
                    state: "STARTING".to_owned(),
                    process_identity_ref: None,
                    ownership: "LITECOWORK".to_owned(),
                    started_at: host_started_at.clone(),
                    last_used_at: host_started_at.clone(),
                    idle_since: None,
                },
                || now_utc_rfc3339(),
                || {
                    OpenCodeServer::spawn(config).map_err(|error| {
                        native_start_error = Some(error);
                    })
                },
                OpenCodeServer::process_identity_ref,
                |mut process| match process.stop() {
                    super::opencode_server::OpenCodeProcessState::Exited => Ok(()),
                    super::opencode_server::OpenCodeProcessState::Running
                    | super::opencode_server::OpenCodeProcessState::ObservationFailed => {
                        Err(process)
                    }
                },
            )
            .map_err(|error| match error {
                HostStartError::InvalidStartingRecord => {
                    "OpenCode host lifecycle rejected its STARTING record".to_owned()
                }
                HostStartError::StoreUnavailable => {
                    "OpenCode host lifecycle state could not be committed".to_owned()
                }
                HostStartError::ProcessStartFailed => format!(
                    "OpenCode host process failed readiness: {:?}",
                    native_start_error
                        .unwrap_or(super::opencode_server::OpenCodeError::SpawnFailed)
                ),
                HostStartError::StateUnconfirmed => {
                    "OpenCode host readiness state is unconfirmed".to_owned()
                }
                HostStartError::CleanupPending(mut process) => {
                    let process_identity_ref = process.process_identity_ref();
                    let exit_observed = matches!(
                        process.stop(),
                        super::opencode_server::OpenCodeProcessState::Exited
                    );
                    let cleanup_settled = super::host_supervisor::settle_host_cleanup_retry(
                        store,
                        host_runtime_id,
                        host_runtime_incarnation_id,
                        host_instance_id,
                        &process_identity_ref,
                        exit_observed,
                        &now_utc_rfc3339,
                    );
                    if cleanup_settled {
                        "OpenCode readiness commit failed; startup process was stopped".to_owned()
                    } else {
                        "OpenCode host cleanup remains unconfirmed".to_owned()
                    }
                }
            })?;
            let mut host_state_guard = ReadyHostStateGuard::new(
                store.clone(),
                host_runtime_id,
                host_runtime_incarnation_id,
                host_instance_id,
            );

            // Drive the native boundary through the provider-neutral AgentAdapter
            // (001-T04) rather than hand-parsing SSE. The host process is durable
            // STARTING before spawn and READY only after native health is observed.
            let mut adapter = OpenCodeAdapter::new(server).with_cancel(cancel.clone());

            // Phase 1: version/capability probe (B-03). A failed probe is a typed
            // FAILED turn with the USER message kept and no fallback (AC-015).
            adapter.probe_version().map_err(|f| {
                format!("OpenCode adapter version probe failed: {}", f.reason_code())
            })?;

            // Phase 2: create the native session; the handle binds to SQLite only.
            let native_session = adapter
                .create_session(&working_dir)
                .map_err(|f| format!("OpenCode create_session failed: {}", f.reason_code()))?;

            // Attach observers only after the provider returned its exact native
            // session ID; permission correlation must never infer this identity.
            let usage_collector = UsageCollector::default();
            let permission_scope = NativePermissionScope {
                workspace_id: workspace_id.to_owned(),
                conversation_id: conversation_id.to_owned(),
                turn_id: turn_id.to_owned(),
                retry_ordinal,
                agent_session_id: session_id.to_owned(),
                runtime_incarnation_id: host_runtime_incarnation_id.to_owned(),
                native_session_id: native_session.clone(),
            };
            let (permission_sender, permission_receiver) =
                sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
            let permission_bus = crate::transient_frames::transient_bus().clone();
            let _permission_mailbox = native_permissions
                .register_mailbox(
                    permission_scope.clone(),
                    permission_sender,
                    permission_bus.clone(),
                )
                .map_err(|_| "OpenCode permission command mailbox unavailable".to_owned())?;
            let permission_registry = native_permissions.clone();
            let control_scope = permission_scope.clone();
            let control_cancel = cancel.clone();
            let control_bus = permission_bus.clone();
            adapter = adapter.with_control_service(Box::new(move |server| {
                if !control_cancel.is_requested() {
                    permission_registry.service_queued_commands(
                        server,
                        &control_scope,
                        &permission_receiver,
                        &control_bus,
                    );
                }
                Ok(())
            }));
            adapter = adapter.with_observer(Box::new(ObserverSet(vec![
                Box::new(NativePermissionObserver::new(
                    native_permissions.clone(),
                    permission_scope,
                    permission_bus.clone(),
                    std::fs::canonicalize(&working_dir).ok(),
                )),
                Box::new(crate::transient_frames::TurnFramePublisher::new(
                    crate::transient_frames::transient_bus().clone(),
                    workspace_id,
                    conversation_id,
                    turn_id,
                    retry_ordinal,
                )),
                Box::new(usage_collector.clone()),
                Box::new(crate::transient_frames::TurnActivityRecorder::new(
                    crate::transient_frames::transient_bus().clone(),
                    workspace_id,
                    conversation_id,
                    turn_id,
                    retry_ordinal,
                    {
                        let activity_store = conv_store.clone();
                        Box::new(move |record| {
                            activity_store
                                .append_conversation_turn_activity(record)
                                .is_ok()
                        })
                    },
                )),
            ])));

            // Activate session in SQLite
            let now = now_utc_rfc3339();
            let (expected_session_version, active_session_version) =
                session_activation_versions(starting_session_version).ok_or_else(|| {
                    "AgentSession version cannot advance for activation".to_owned()
                })?;
            let activation = ActivateConversationAgentSession {
                principal_id: principal_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                agent_session_id: session_id.to_owned(),
                expected_session_version,
                expected_turn_version: turn_version,
                occurred_at: now.clone(),
                host_instance_id: host_instance_id.to_owned(),
                native_session_ref: Some(native_session.clone()),
                session_event: EventDraft {
                    event_id: generate_id("ev"),
                    workspace_id: workspace_id.to_owned(),
                    entity_type: "AgentSession".to_owned(),
                    entity_id: session_id.to_owned(),
                    origin_runtime_id: runtime_id.to_owned(),
                    entity_revision: active_session_version,
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation_id.to_owned(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "agent.session.started.v1".to_owned(),
                    payload: json!({
                        "agent_session_id": session_id,
                        "task_spec_revision": null,
                        "scope": {
                            "kind": "CONVERSATION",
                            "conversation_id": conversation_id,
                            "conversation_turn_id": turn_id,
                        },
                        "agent_binding_id": agent_binding_id,
                        "session_state": "ACTIVE",
                        "reported_at": now,
                    }),
                    recorded_at: now.clone(),
                },
                turn_event: EventDraft {
                    event_id: generate_id("ev"),
                    workspace_id: workspace_id.to_owned(),
                    entity_type: "ConversationTurn".to_owned(),
                    entity_id: turn_id.to_owned(),
                    origin_runtime_id: runtime_id.to_owned(),
                    entity_revision: turn_version + 1,
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation_id.to_owned(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "conversation.turn.status.changed.v1".to_owned(),
                    payload: json!({
                        "turn_id": turn_id,
                        "from": "OPEN",
                        "to": "RUNNING",
                        "reason_code": "SESSION_READY",
                        "agent_session_id": session_id,
                        "aggregate_version": turn_version + 1,
                    }),
                    recorded_at: now.clone(),
                },
            };

            let activated = store
                .activate_conversation_agent_session(activation)
                .map_err(|e| format!("Activate session error: {e:?}"))?;
            host_state_guard.mark_busy();

            // Load context package
            let context = conv_store
                .load_conversation_context_for_turn(
                    principal_id,
                    workspace_id,
                    conversation_id,
                    turn_id,
                )
                .map_err(|e| format!("Load context error: {e:?}"))?;
            // B-08: reauthorize every attached Resource and inline its text before the
            // window runs; any refusal fails the turn before dispatch with a typed reason.
            let context = super::resource_materializer::materialize_conversation_resources(
                store,
                workspace_id,
                context,
            )
            .map_err(|e| format!("Resource materialization failed: {}", e.reason_code()))?;
            // B-07: reduce an over-budget prefix with the explicit notice, or refuse
            // the turn with a typed error. Never a silent truncation.
            let (context, _context_reduction) =
                super::conversation_prompt::window_conversation_context(
                    context,
                    super::conversation_prompt::MAX_NATIVE_CONVERSATION_INPUT_BYTES,
                )
                .map_err(|e| format!("Context window error: {e:?}"))?;
            let prompt_bytes =
                super::conversation_prompt::encode_conversation_context_prompt(&context)
                    .map(|prompt| prompt.len())
                    .unwrap_or(0);

            // Phase 3: dispatch the bounded committed context only after the
            // activation commit above. The native message id correlates native
            // events; it is not treated as an idempotency guarantee.
            let native_msg_id = format!("msg_{}", generate_id("m"));
            adapter
                .send_context(&native_session, &native_msg_id, &context)
                .map_err(|f| format!("Send context error: {}", f.reason_code()))?;

            // Phase 4: consume the correlated, bounded event stream through the
            // normalizer under a line-resetting idle/heartbeat watchdog (replaces
            // the old hard 60 s cap). Every event is correlated to the native
            // session; uncorrelated, foreign-session, oversized and unknown frames
            // are dropped and counted, never projected (AC-008).
            let observation = match adapter.drive_turn(&native_session, StreamBudget::default()) {
                Ok(TurnObservation::StopUnconfirmed) => adapter
                    .reconcile_stop(&native_session, &native_msg_id, Duration::from_secs(5))
                    .ok()
                    .flatten()
                    .map(Ok)
                    .unwrap_or(Ok(TurnObservation::StopUnconfirmed)),
                observation => observation,
            };

            let disposition = apply_observed_turn(
                TurnSettlementScope {
                    store,
                    runtime_id,
                    principal_id,
                    workspace_id,
                    conversation_id,
                    turn_id,
                    session_id,
                    correlation_id,
                },
                observation,
                turn_usage(usage_collector.snapshot(), prompt_bytes, 0),
            )?;

            let accumulated_text = match disposition {
                ObservedTurnDisposition::Response(answer_text) => answer_text,
                ObservedTurnDisposition::Cancelled => {
                    // The close transaction atomically returns the host row to
                    // READY. Stop the per-session process only after that durable
                    // settlement, then publish STOPPED only after exit is observed.
                    host_state_guard.mark_ready_after_session_close();
                    finish_per_session_host(&mut adapter, &mut host_state_guard)?;
                    return Ok(TurnOutcome::Cancelled);
                }
                ObservedTurnDisposition::Unconfirmed => {
                    // The interrupt was sent but quiescence was not observed in time.
                    // The turn stays CANCEL_REQUESTED; CANCELLED is never fabricated.
                    // Keep the durable AgentSessionHostBinding live and transfer the
                    // native process handle to the Runtime host owner. Dropping the
                    // per-turn adapter here would kill a process whose session is
                    // still unresolved and discard the only private wait handle.
                    let process = adapter.into_server();
                    match host_supervisor.retain_process(host_instance_id, session_id, process) {
                        Ok(()) => host_state_guard.retain_live_session_reference(),
                        Err(mut process) => {
                            // Host identity is unique per AgentSession. If durable
                            // corruption or a collision violates that invariant,
                            // attempt owner cleanup but do not claim cancellation.
                            let _ = process.stop();
                            return Err(
                                "OpenCode host process could not be retained for reconciliation"
                                    .to_owned(),
                            );
                        }
                    }
                    return Ok(TurnOutcome::StopUnconfirmed);
                }
                ObservedTurnDisposition::Failed {
                    reason_code,
                    settlement_succeeded,
                } => {
                    // Settle FAILED here so whatever usage was observed is recorded;
                    // the dispatcher's later settle of an already-settled turn is a no-op.
                    if settlement_succeeded {
                        // LOST has released the AgentSession reference. Mark this
                        // per-turn host degraded, then stop it through STOPPING.
                        host_state_guard.mark_degraded_after_session_loss()?;
                        finish_per_session_host(&mut adapter, &mut host_state_guard)?;
                    }
                    return Err(format!("OpenCode turn failed closed: {reason_code}"));
                }
            };

            // The commit identity is deterministic in (turn, retry ordinal, native
            // message) so a repeated commit attempt replays the stored receipt instead
            // of writing a second answer (B-04).
            let final_message_id = format!("msg-agent-{turn_id}-{retry_ordinal}");
            let provider_response_id =
                provider_response_commit_identity(turn_id, retry_ordinal, &native_msg_id);
            let now = now_utc_rfc3339();
            let commit_resp = CommitConversationAgentResponse {
                request: WorkspaceCreateRequest {
                    principal_id: principal_id.to_owned(),
                    request_id: format!("req-commit-{turn_id}-{retry_ordinal}"),
                    request_payload: json!({
                        "turn_id": turn_id,
                        "retry_ordinal": retry_ordinal,
                        "provider_response_id": provider_response_id,
                    }),
                },
                principal_id: principal_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                conversation_id: conversation_id.to_owned(),
                turn_id: turn_id.to_owned(),
                agent_session_id: session_id.to_owned(),
                expected_turn_version: activated.turn.version,
                expected_session_version: activated.session.version,
                retry_ordinal,
                provider_response_id,
                usage: turn_usage(
                    usage_collector.snapshot(),
                    prompt_bytes,
                    accumulated_text.len(),
                ),
                turn_status: ConversationTurnStatus::Completed,
                message: ConversationMessageRecord {
                    message_id: final_message_id.clone(),
                    conversation_id: conversation_id.to_owned(),
                    author: json!({ "kind": "AGENT", "agent_binding_id": agent_binding_id }),
                    role: "AGENT".to_owned(),
                    agent_session_id: Some(session_id.to_owned()),
                    agent_binding_id: Some(agent_binding_id.to_owned()),
                    turn_id: Some(turn_id.to_owned()),
                    content: json!([{ "kind": "TEXT", "text": accumulated_text }]),
                    resource_refs: json!([]),
                    source_channel_ref: None,
                    created_at: now.clone(),
                },
                message_event: EventDraft {
                    event_id: generate_id("ev"),
                    workspace_id: workspace_id.to_owned(),
                    entity_type: "Conversation".to_owned(),
                    entity_id: conversation_id.to_owned(),
                    origin_runtime_id: runtime_id.to_owned(),
                    entity_revision: 3,
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation_id.to_owned(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "conversation.message.added.v1".to_owned(),
                    payload: json!({
                        "conversation_id": conversation_id,
                        "message_id": final_message_id,
                        "role": "AGENT",
                    }),
                    recorded_at: now.clone(),
                },
                turn_event: EventDraft {
                    event_id: generate_id("ev"),
                    workspace_id: workspace_id.to_owned(),
                    entity_type: "ConversationTurn".to_owned(),
                    entity_id: turn_id.to_owned(),
                    origin_runtime_id: runtime_id.to_owned(),
                    entity_revision: activated.turn.version + 1,
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation_id.to_owned(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "conversation.turn.settled.v1".to_owned(),
                    payload: json!({
                        "turn_id": turn_id,
                        "from": "RUNNING",
                        "to": "COMPLETED",
                        "aggregate_version": activated.turn.version + 1,
                    }),
                    recorded_at: now.clone(),
                },
                session_event: EventDraft {
                    event_id: generate_id("ev"),
                    workspace_id: workspace_id.to_owned(),
                    entity_type: "AgentSession".to_owned(),
                    entity_id: session_id.to_owned(),
                    origin_runtime_id: runtime_id.to_owned(),
                    entity_revision: activated.session.version + 1,
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation_id.to_owned(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "agent.session.closed.v1".to_owned(),
                    payload: json!({
                        "agent_session_id": session_id,
                        "from": "ACTIVE",
                        "to": "CLOSED",
                        "aggregate_version": activated.session.version + 1,
                    }),
                    recorded_at: now.clone(),
                },
            };

            retry_transient_store_commit(3, Duration::from_millis(100), || {
                conv_store.commit_conversation_agent_response(commit_resp.clone())
            })
            .map_err(|e| format!("Commit response error: {e:?}"))?;

            // The response transaction closes the AgentSession and returns its
            // host row to READY. The process remains owned until after this commit;
            // only observed process exit can move the per-session row to STOPPED.
            host_state_guard.mark_ready_after_session_close();
            finish_per_session_host(&mut adapter, &mut host_state_guard)?;

            Ok(TurnOutcome::Committed)
        }
        _ => Err(format!(
            "Agent provider '{provider_key}' is not supported or not configured for execution in this runtime"
        )),
    }
}

fn finish_per_session_host(
    adapter: &mut OpenCodeAdapter,
    host_state_guard: &mut ReadyHostStateGuard,
) -> Result<(), String> {
    let host_state_guard = RefCell::new(host_state_guard);
    let exit_observed = reconcile_per_session_host_shutdown(
        || host_state_guard.borrow_mut().begin_stopping(),
        || adapter.shutdown(),
        || host_state_guard.borrow_mut().finish_stopped(),
    )?;
    if !exit_observed {
        host_state_guard.borrow_mut().retain_stopping_pending();
    }
    Ok(())
}

fn reconcile_per_session_host_shutdown(
    persist_stopping: impl FnOnce() -> Result<(), String>,
    stop: impl FnOnce() -> bool,
    persist_stopped: impl FnOnce() -> Result<(), String>,
) -> Result<bool, String> {
    persist_stopping()?;
    if !stop() {
        return Ok(false);
    }
    persist_stopped()?;
    Ok(true)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::thread::JoinHandle;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct ProviderReply {
        pub(crate) session_id: String,
        pub(crate) request_id: String,
        pub(crate) decision: &'static str,
    }

    struct RecordingProvider {
        replies: Arc<Mutex<Vec<ProviderReply>>>,
        reply_started: Option<SyncSender<()>>,
        release_reply: Option<Receiver<()>>,
    }

    impl NativePermissionProvider for RecordingProvider {
        fn reply_session_permission(
            &mut self,
            session_id: &str,
            request_id: &str,
            decision: OpenCodePermissionDecision,
        ) -> Result<(), OpenCodeError> {
            let decision = match decision {
                OpenCodePermissionDecision::Once => "once",
                OpenCodePermissionDecision::Reject => "reject",
            };
            self.replies
                .lock()
                .map_err(|_| OpenCodeError::InvalidResponse)?
                .push(ProviderReply {
                    session_id: session_id.to_owned(),
                    request_id: request_id.to_owned(),
                    decision,
                });
            if let Some(reply_started) = &self.reply_started {
                let _ = reply_started.send(());
            }
            if let Some(release_reply) = &self.release_reply {
                let _ = release_reply.recv();
            }
            Ok(())
        }

        fn list_session_permissions(
            &mut self,
            _session_id: &str,
        ) -> Result<serde_json::Value, OpenCodeError> {
            Ok(json!({ "data": [] }))
        }
    }

    pub(crate) struct PermissionWorker {
        mailbox: Option<NativePermissionMailboxRegistration>,
        worker: Option<JoinHandle<()>>,
        replies: Arc<Mutex<Vec<ProviderReply>>>,
        reply_started: Option<Receiver<()>>,
        release_reply: Option<SyncSender<()>>,
    }

    impl PermissionWorker {
        pub(crate) async fn wait_for_provider_reply(&self) {
            let reply_started = self
                .reply_started
                .as_ref()
                .expect("blocked test worker has a reply-start notification");
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    match reply_started.try_recv() {
                        Ok(()) => return,
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            panic!("fake provider worker ended before the reply call")
                        }
                    }
                }
            })
            .await
            .expect("worker reaches the fake provider reply boundary");
        }

        pub(crate) fn close_mailbox(&mut self) {
            drop(self.mailbox.take());
        }

        pub(crate) fn release_provider_reply(&mut self) {
            if let Some(release_reply) = self.release_reply.take() {
                let _ = release_reply.send(());
            }
        }

        pub(crate) fn finish(mut self) -> Vec<ProviderReply> {
            self.close_mailbox();
            self.release_provider_reply();
            self.worker
                .take()
                .expect("test permission worker thread exists")
                .join()
                .expect("test permission worker exits after mailbox closes");
            self.replies
                .lock()
                .expect("test provider reply log remains available")
                .clone()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_permission_worker(
        supervisor: &AgentSessionSupervisor,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        agent_session_id: &str,
        runtime_incarnation_id: &str,
        native_session_id: &str,
        prompt_token: &str,
        native_request_id: &str,
    ) -> PermissionWorker {
        start_permission_worker_internal(
            supervisor,
            workspace_id,
            conversation_id,
            turn_id,
            retry_ordinal,
            agent_session_id,
            runtime_incarnation_id,
            native_session_id,
            prompt_token,
            native_request_id,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_blocked_permission_worker(
        supervisor: &AgentSessionSupervisor,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        agent_session_id: &str,
        runtime_incarnation_id: &str,
        native_session_id: &str,
        prompt_token: &str,
        native_request_id: &str,
    ) -> PermissionWorker {
        start_permission_worker_internal(
            supervisor,
            workspace_id,
            conversation_id,
            turn_id,
            retry_ordinal,
            agent_session_id,
            runtime_incarnation_id,
            native_session_id,
            prompt_token,
            native_request_id,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn start_permission_worker_internal(
        supervisor: &AgentSessionSupervisor,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        agent_session_id: &str,
        runtime_incarnation_id: &str,
        native_session_id: &str,
        prompt_token: &str,
        native_request_id: &str,
        block_reply: bool,
    ) -> PermissionWorker {
        let scope = NativePermissionScope {
            workspace_id: workspace_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            turn_id: turn_id.to_owned(),
            retry_ordinal,
            agent_session_id: agent_session_id.to_owned(),
            runtime_incarnation_id: runtime_incarnation_id.to_owned(),
            native_session_id: native_session_id.to_owned(),
        };
        let registry = supervisor.native_permissions.clone();
        let bus = crate::transient_frames::TransientFrameBus::for_tests();
        let (sender, receiver) = sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
        let mailbox = registry
            .register_mailbox(scope.clone(), sender, bus.clone())
            .expect("test permission mailbox registers");
        registry
            .register_with_token(
                &scope,
                native_session_id,
                prompt_token.to_owned(),
                native_request_id.to_owned(),
                "writeFile".to_owned(),
                Some("Update the selected file".to_owned()),
                Vec::new(),
            )
            .expect("test permission prompt registers in the Runtime registry");

        let replies = Arc::new(Mutex::new(Vec::new()));
        let provider_replies = Arc::clone(&replies);
        let worker_registry = registry.clone();
        let worker_scope = scope.clone();
        let (reply_started, provider_reply_started) = if block_reply {
            let (sender, receiver) = sync_channel(1);
            (Some(receiver), Some(sender))
        } else {
            (None, None)
        };
        let (release_reply, provider_release_reply) = if block_reply {
            let (sender, receiver) = sync_channel(1);
            (Some(sender), Some(receiver))
        } else {
            (None, None)
        };
        let worker = std::thread::spawn(move || {
            let mut provider = RecordingProvider {
                replies: provider_replies,
                reply_started: provider_reply_started,
                release_reply: provider_release_reply,
            };
            while let Ok(command) = receiver.recv() {
                worker_registry.apply_reply_command(&mut provider, &worker_scope, command, &bus);
            }
        });

        PermissionWorker {
            mailbox: Some(mailbox),
            worker: Some(worker),
            replies,
            reply_started,
            release_reply,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeNativePermissionProvider {
        replies: Vec<(String, String, OpenCodePermissionDecision)>,
        reply_result: Result<(), OpenCodeError>,
        pending: serde_json::Value,
        pending_sessions: Vec<String>,
    }

    impl NativePermissionProvider for FakeNativePermissionProvider {
        fn reply_session_permission(
            &mut self,
            session_id: &str,
            request_id: &str,
            decision: OpenCodePermissionDecision,
        ) -> Result<(), super::super::opencode_server::OpenCodeError> {
            self.replies
                .push((session_id.to_owned(), request_id.to_owned(), decision));
            self.reply_result
        }

        fn list_session_permissions(
            &mut self,
            session_id: &str,
        ) -> Result<serde_json::Value, super::super::opencode_server::OpenCodeError> {
            self.pending_sessions.push(session_id.to_owned());
            Ok(self.pending.clone())
        }
    }

    #[test]
    fn native_permission_worker_dispatches_exact_private_identifiers_once_and_publishes_ack() {
        let registry = NativePermissionRegistry::default();
        let bus = crate::transient_frames::TransientFrameBus::for_tests();
        let mut frames = bus.subscribe();
        let scope = NativePermissionScope {
            workspace_id: "ws-worker".to_owned(),
            conversation_id: "conv-worker".to_owned(),
            turn_id: "turn-worker".to_owned(),
            retry_ordinal: 3,
            agent_session_id: "agent-session-worker".to_owned(),
            runtime_incarnation_id: "inc-worker".to_owned(),
            native_session_id: "native-session-private".to_owned(),
        };
        let (sender, receiver) = sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
        let _mailbox = registry
            .register_mailbox(scope.clone(), sender, bus.clone())
            .expect("active attempt mailbox is registered");
        let token = "a".repeat(64);
        registry
            .register_with_token(
                &scope,
                &scope.native_session_id,
                token.clone(),
                "native-request-private".to_owned(),
                "writeFile".to_owned(),
                Some("Update the selected file".to_owned()),
                vec!["C:/private/workspace/file.txt".to_owned()],
            )
            .expect("provider request is registered");

        let NativePermissionReplyStart::Dispatch(reply_sender) = registry
            .begin_reply(
                &scope,
                &token,
                "owner-worker",
                "permission-reply-key",
                NativePermissionDecision::AllowOnce,
            )
            .expect("reply is admitted once")
        else {
            panic!("first decision must dispatch to the active worker");
        };
        let (completed, response) = tokio::sync::oneshot::channel();
        reply_sender
            .try_send(NativePermissionCommand {
                prompt_token: token.clone(),
                decision: NativePermissionDecision::AllowOnce,
                completed,
            })
            .expect("reply command fits the bounded mailbox");

        let mut provider = FakeNativePermissionProvider {
            replies: Vec::new(),
            reply_result: Ok(()),
            pending: json!({ "data": [] }),
            pending_sessions: Vec::new(),
        };
        registry.service_queued_commands(&mut provider, &scope, &receiver, &bus);
        response
            .blocking_recv()
            .expect("worker acknowledges local command completion");

        assert_eq!(
            provider.replies,
            [(
                "native-session-private".to_owned(),
                "native-request-private".to_owned(),
                OpenCodePermissionDecision::Once,
            )],
            "the worker must reply once to the exact private provider scope and request"
        );
        let settled = registry
            .view_for_scope(&scope, &token)
            .expect("selected prompt remains in active attempt scope");
        assert_eq!(settled.state_name(), "ANSWERED");
        assert_eq!(settled.decision(), Some("ALLOW_ONCE"));

        let frame = frames
            .try_recv()
            .expect("worker publishes the acknowledgment");
        let frame_json: serde_json::Value =
            serde_json::from_slice(&frame.bytes).expect("permission frame is valid JSON");
        assert_eq!(frame_json["state"], "ANSWERED");
        assert_eq!(frame_json["decision"], "ALLOW_ONCE");
        let serialized = frame_json.to_string();
        assert!(!serialized.contains("native-session-private"));
        assert!(!serialized.contains("native-request-private"));
        assert!(!serialized.contains("C:/private/workspace/file.txt"));
    }

    #[test]
    fn native_permission_worker_rejects_same_session_pending_requests_once() {
        let registry = NativePermissionRegistry::default();
        let bus = crate::transient_frames::TransientFrameBus::for_tests();
        let scope = NativePermissionScope {
            workspace_id: "ws-reject-worker".to_owned(),
            conversation_id: "conv-reject-worker".to_owned(),
            turn_id: "turn-reject-worker".to_owned(),
            retry_ordinal: 0,
            agent_session_id: "agent-session-reject-worker".to_owned(),
            runtime_incarnation_id: "inc-reject-worker".to_owned(),
            native_session_id: "native-session-reject-private".to_owned(),
        };
        let (sender, receiver) = sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
        let _mailbox = registry
            .register_mailbox(scope.clone(), sender, bus.clone())
            .expect("active attempt mailbox is registered");
        let selected = "a".repeat(64);
        let other = "b".repeat(64);
        for (token, request_id) in [
            (&selected, "native-request-selected"),
            (&other, "native-request-other-pending"),
        ] {
            registry
                .register_with_token(
                    &scope,
                    &scope.native_session_id,
                    token.clone(),
                    request_id.to_owned(),
                    "writeFile".to_owned(),
                    None,
                    vec![],
                )
                .expect("provider request is registered");
        }

        let NativePermissionReplyStart::Dispatch(reply_sender) = registry
            .begin_reply(
                &scope,
                &selected,
                "owner-reject-worker",
                "reject-reply-key",
                NativePermissionDecision::RejectSessionPending,
            )
            .expect("session rejection is admitted once")
        else {
            panic!("first decision must dispatch to the active worker");
        };
        let (completed, response) = tokio::sync::oneshot::channel();
        reply_sender
            .try_send(NativePermissionCommand {
                prompt_token: selected.clone(),
                decision: NativePermissionDecision::RejectSessionPending,
                completed,
            })
            .expect("reply command fits the bounded mailbox");

        let mut provider = FakeNativePermissionProvider {
            replies: Vec::new(),
            reply_result: Ok(()),
            pending: json!({ "data": [] }),
            pending_sessions: Vec::new(),
        };
        registry.service_queued_commands(&mut provider, &scope, &receiver, &bus);
        response
            .blocking_recv()
            .expect("worker acknowledges local command completion");

        assert_eq!(
            provider.replies,
            [(
                "native-session-reject-private".to_owned(),
                "native-request-selected".to_owned(),
                OpenCodePermissionDecision::Reject,
            )],
            "the provider receives one rejection for the selected native request"
        );
        assert!(provider.pending_sessions.is_empty());
        for token in [&selected, &other] {
            let settled = registry
                .view_for_scope(&scope, token)
                .expect("same-session prompt remains in active scope");
            assert_eq!(settled.state_name(), "ANSWERED");
            assert_eq!(settled.decision(), Some("REJECT_SESSION_PENDING"));
        }
    }

    #[test]
    fn native_permission_worker_reconciles_ambiguous_reply_once_without_replay() {
        let registry = NativePermissionRegistry::default();
        let bus = crate::transient_frames::TransientFrameBus::for_tests();
        let scope = NativePermissionScope {
            workspace_id: "ws-ambiguous-worker".to_owned(),
            conversation_id: "conv-ambiguous-worker".to_owned(),
            turn_id: "turn-ambiguous-worker".to_owned(),
            retry_ordinal: 1,
            agent_session_id: "agent-session-ambiguous-worker".to_owned(),
            runtime_incarnation_id: "inc-ambiguous-worker".to_owned(),
            native_session_id: "native-session-ambiguous-private".to_owned(),
        };
        let (sender, receiver) = sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
        let _mailbox = registry
            .register_mailbox(scope.clone(), sender, bus.clone())
            .expect("active attempt mailbox is registered");
        let token = "c".repeat(64);
        registry
            .register_with_token(
                &scope,
                &scope.native_session_id,
                token.clone(),
                "native-request-still-pending".to_owned(),
                "writeFile".to_owned(),
                None,
                vec![],
            )
            .expect("provider request is registered");
        let NativePermissionReplyStart::Dispatch(reply_sender) = registry
            .begin_reply(
                &scope,
                &token,
                "owner-ambiguous-worker",
                "ambiguous-reply-key",
                NativePermissionDecision::AllowOnce,
            )
            .expect("reply is admitted once")
        else {
            panic!("first decision must dispatch to the active worker");
        };
        let (completed, response) = tokio::sync::oneshot::channel();
        reply_sender
            .try_send(NativePermissionCommand {
                prompt_token: token.clone(),
                decision: NativePermissionDecision::AllowOnce,
                completed,
            })
            .expect("reply command fits the bounded mailbox");

        let mut provider = FakeNativePermissionProvider {
            replies: Vec::new(),
            reply_result: Err(OpenCodeError::Timeout),
            pending: json!({
                "data": [{
                    "id": "native-request-still-pending",
                    "sessionID": "native-session-ambiguous-private"
                }]
            }),
            pending_sessions: Vec::new(),
        };
        registry.service_queued_commands(&mut provider, &scope, &receiver, &bus);
        response
            .blocking_recv()
            .expect("worker acknowledges local command completion");

        assert_eq!(
            provider.replies.len(),
            1,
            "the uncertain reply is never replayed"
        );
        assert_eq!(
            provider.pending_sessions,
            ["native-session-ambiguous-private"]
        );
        let still_pending = registry
            .view_for_scope(&scope, &token)
            .expect("prompt remains in active attempt scope");
        assert_eq!(still_pending.state_name(), "PENDING");
        assert_eq!(still_pending.decision(), None);
    }

    #[test]
    fn native_permission_registry_projects_opaque_scoped_prompt_without_native_identifiers() {
        let registry = NativePermissionRegistry::default();
        let token = "a".repeat(64);
        let scope = NativePermissionScope {
            workspace_id: "ws-one".to_owned(),
            conversation_id: "conv-one".to_owned(),
            turn_id: "turn-one".to_owned(),
            retry_ordinal: 0,
            agent_session_id: "agent-session-one".to_owned(),
            runtime_incarnation_id: "inc-one".to_owned(),
            native_session_id: "native-session-secret".to_owned(),
        };
        let prompt = registry
            .register_with_token(
                &scope,
                &scope.native_session_id,
                token.clone(),
                "native-request-secret".to_owned(),
                "writeFile".to_owned(),
                Some("Update the selected file".to_owned()),
                vec!["C:/private/workspace/secret.txt".to_owned()],
            )
            .expect("register normalized native permission");

        let serialized = serde_json::to_string(&prompt).expect("serialize public prompt");
        assert!(serialized.contains(&token));
        assert!(serialized.contains("writeFile"));
        assert_eq!(prompt.resource_count, 1);
        assert_eq!(prompt.resources, ["Resource 1"]);
        assert!(!serialized.contains("native-session-secret"));
        assert!(!serialized.contains("native-request-secret"));
        assert!(!serialized.contains("C:/private/workspace/secret.txt"));

        assert_eq!(
            registry.register_with_token(
                &scope,
                &scope.native_session_id,
                "b".repeat(64),
                "native-request-secret".to_owned(),
                "writeFile".to_owned(),
                Some("Update the selected file".to_owned()),
                vec!["C:/private/workspace/secret.txt".to_owned()],
            ),
            Ok(prompt.clone()),
            "an identical provider notification is idempotent"
        );
        assert_eq!(
            registry.register_with_token(
                &scope,
                &scope.native_session_id,
                "c".repeat(64),
                "native-request-secret".to_owned(),
                "deleteFile".to_owned(),
                Some("Update the selected file".to_owned()),
                vec!["C:/private/workspace/secret.txt".to_owned()],
            ),
            Err(NativePermissionRegistryError::ConflictingDuplicate)
        );

        let wrong_turn = NativePermissionScope {
            turn_id: "turn-other".to_owned(),
            ..scope.clone()
        };
        assert!(
            registry
                .snapshot(
                    &wrong_turn.workspace_id,
                    &wrong_turn.conversation_id,
                    &wrong_turn.turn_id,
                    wrong_turn.retry_ordinal,
                    &wrong_turn.agent_session_id,
                    &wrong_turn.runtime_incarnation_id,
                )
                .items
                .is_empty()
        );
        let active_snapshot = registry.snapshot(
            &scope.workspace_id,
            &scope.conversation_id,
            &scope.turn_id,
            scope.retry_ordinal,
            &scope.agent_session_id,
            &scope.runtime_incarnation_id,
        );
        assert_eq!(active_snapshot.sequence, 1);
        assert_eq!(active_snapshot.items, [prompt]);
        let ended = registry.end_attempt(&scope);
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].sequence(), 2);
        assert_eq!(
            registry
                .snapshot(
                    &scope.workspace_id,
                    &scope.conversation_id,
                    &scope.turn_id,
                    scope.retry_ordinal,
                    &scope.agent_session_id,
                    &scope.runtime_incarnation_id,
                )
                .items[0]
                .state,
            NativePermissionState::SessionEnded
        );
    }

    #[test]
    fn native_permission_resource_projection_shows_only_existing_workspace_relative_paths() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let source = workspace.path().join("src");
        std::fs::create_dir_all(&source).expect("create workspace subdirectory");
        std::fs::write(source.join("main.rs"), "fn main() {}").expect("create workspace file");
        let outside = tempfile::tempdir().expect("temporary outside directory");
        let external_file = outside.path().join("secret.txt");
        std::fs::write(&external_file, "secret").expect("create external file");
        let canonical_root = std::fs::canonicalize(workspace.path()).expect("canonical root");

        let labels = permission_resource_labels(
            Some(&canonical_root),
            &[
                "src/main.rs".to_owned(),
                external_file.to_string_lossy().into_owned(),
                "../outside.txt".to_owned(),
                "missing.txt".to_owned(),
            ],
        );

        assert_eq!(
            labels,
            [
                "src/main.rs",
                "External resource 2",
                "Resource 3",
                "Resource 4"
            ]
        );
        assert!(
            labels
                .iter()
                .all(|label| !label.contains(workspace.path().to_string_lossy().as_ref()))
        );
        assert!(
            labels
                .iter()
                .all(|label| !label.contains(outside.path().to_string_lossy().as_ref()))
        );
    }

    #[cfg(unix)]
    #[test]
    fn native_permission_resource_projection_does_not_follow_workspace_symlinks() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("temporary workspace");
        let outside = tempfile::tempdir().expect("temporary outside directory");
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, "secret").expect("create external file");
        symlink(&secret, workspace.path().join("secret-link")).expect("create workspace symlink");
        let canonical_root = std::fs::canonicalize(workspace.path()).expect("canonical root");

        assert_eq!(
            permission_resource_labels(Some(&canonical_root), &["secret-link".to_owned()]),
            ["Resource 1"],
            "a symlink target must not disclose a path outside the Workspace"
        );
    }

    #[test]
    fn native_permission_attempt_end_closes_unknown_outcome_without_claiming_a_reply() {
        let registry = NativePermissionRegistry::default();
        let scope = NativePermissionScope {
            workspace_id: "ws-unknown-end".to_owned(),
            conversation_id: "conv-unknown-end".to_owned(),
            turn_id: "turn-unknown-end".to_owned(),
            retry_ordinal: 0,
            agent_session_id: "agent-session-unknown-end".to_owned(),
            runtime_incarnation_id: "inc-unknown-end".to_owned(),
            native_session_id: "native-session-unknown-end".to_owned(),
        };
        let (sender, _receiver) = sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
        let _mailbox = registry
            .register_mailbox(
                scope.clone(),
                sender,
                crate::transient_frames::TransientFrameBus::for_tests(),
            )
            .expect("active attempt mailbox is registered");
        let token = "d".repeat(64);
        registry
            .register_with_token(
                &scope,
                &scope.native_session_id,
                token.clone(),
                "native-request-unknown-end".to_owned(),
                "writeFile".to_owned(),
                None,
                Vec::new(),
            )
            .expect("register prompt");
        registry
            .begin_reply(
                &scope,
                &token,
                "owner-unknown-end",
                "reply-key-unknown-end",
                NativePermissionDecision::AllowOnce,
            )
            .expect("start one decision");
        let uncertain =
            registry.reconcile_reply(&scope, &token, NativePermissionDecision::AllowOnce, None);
        assert_eq!(uncertain[0].state, NativePermissionState::OutcomeUnknown);
        assert_eq!(uncertain[0].decision(), Some("ALLOW_ONCE"));

        let ended = registry.end_attempt(&scope);

        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].state, NativePermissionState::SessionEnded);
        assert_eq!(ended[0].decision(), Some("ALLOW_ONCE"));
        assert_eq!(ended[0].sequence(), 4);
    }

    #[test]
    fn native_permission_registry_reclaims_prompt_capacity_when_attempt_ends() {
        let registry = NativePermissionRegistry::default();
        let mut registrations = Vec::new();
        let mut receivers = Vec::new();
        let mut next_prompt = 0_usize;

        for attempt in 0..8 {
            let scope = NativePermissionScope {
                workspace_id: "ws-capacity".to_owned(),
                conversation_id: format!("conv-{attempt}"),
                turn_id: format!("turn-{attempt}"),
                retry_ordinal: 0,
                agent_session_id: format!("agent-session-{attempt}"),
                runtime_incarnation_id: "inc-capacity".to_owned(),
                native_session_id: format!("native-session-{attempt}"),
            };
            let (sender, receiver) = sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
            registrations.push(
                registry
                    .register_mailbox(
                        scope.clone(),
                        sender,
                        crate::transient_frames::TransientFrameBus::for_tests(),
                    )
                    .expect("active attempt mailbox is registered"),
            );
            receivers.push(receiver);

            for _ in 0..MAX_NATIVE_PERMISSION_PROMPTS_PER_TURN {
                registry
                    .register_with_token(
                        &scope,
                        &scope.native_session_id,
                        format!("{next_prompt:064x}"),
                        format!("permission-{next_prompt}"),
                        "writeFile".to_owned(),
                        None,
                        vec![],
                    )
                    .expect("active prompts fit the Runtime bound");
                next_prompt += 1;
            }
        }

        let overflow_scope = NativePermissionScope {
            workspace_id: "ws-capacity".to_owned(),
            conversation_id: "conv-overflow".to_owned(),
            turn_id: "turn-overflow".to_owned(),
            retry_ordinal: 0,
            agent_session_id: "agent-session-overflow".to_owned(),
            runtime_incarnation_id: "inc-capacity".to_owned(),
            native_session_id: "native-session-overflow".to_owned(),
        };
        assert_eq!(
            registry.register_with_token(
                &overflow_scope,
                &overflow_scope.native_session_id,
                "f".repeat(64),
                "permission-overflow".to_owned(),
                "writeFile".to_owned(),
                None,
                vec![],
            ),
            Err(NativePermissionRegistryError::Capacity),
            "the Runtime bound remains effective while all retained attempts are active"
        );

        drop(registrations.remove(0));
        let (sender, receiver) = sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
        let _registration = registry
            .register_mailbox(
                overflow_scope.clone(),
                sender,
                crate::transient_frames::TransientFrameBus::for_tests(),
            )
            .expect("a new attempt can register after the ended attempt is released");
        receivers.push(receiver);
        assert!(
            registry
                .register_with_token(
                    &overflow_scope,
                    &overflow_scope.native_session_id,
                    "f".repeat(64),
                    "permission-after-release".to_owned(),
                    "writeFile".to_owned(),
                    None,
                    vec![],
                )
                .is_ok()
        );
    }

    #[test]
    fn native_permission_reply_fences_replay_and_reconciles_session_rejection() {
        let registry = NativePermissionRegistry::default();
        let scope = NativePermissionScope {
            workspace_id: "ws-reply".to_owned(),
            conversation_id: "conv-reply".to_owned(),
            turn_id: "turn-reply".to_owned(),
            retry_ordinal: 1,
            agent_session_id: "agent-session-reply".to_owned(),
            runtime_incarnation_id: "inc-reply".to_owned(),
            native_session_id: "native-session-reply".to_owned(),
        };
        let (sender, _receiver) = sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
        let _mailbox = registry
            .register_mailbox(
                scope.clone(),
                sender,
                crate::transient_frames::TransientFrameBus::for_tests(),
            )
            .expect("attempt mailbox is registered");
        let register = |token: &str, request_id: &str| {
            registry
                .register_with_token(
                    &scope,
                    &scope.native_session_id,
                    token.to_owned(),
                    request_id.to_owned(),
                    "writeFile".to_owned(),
                    None,
                    vec![],
                )
                .expect("prompt is registered")
        };
        let selected = "a".repeat(64);
        let second = "b".repeat(64);
        let third = "c".repeat(64);
        assert_eq!(register(&selected, "permission-selected").sequence(), 1);
        assert_eq!(register(&second, "permission-second").sequence(), 2);
        assert_eq!(register(&third, "permission-third").sequence(), 3);

        assert!(matches!(
            registry.begin_reply(
                &scope,
                &selected,
                "owner-a",
                "permission-key-1",
                NativePermissionDecision::AllowOnce,
            ),
            Ok(NativePermissionReplyStart::Dispatch(_))
        ));
        let replay = registry
            .begin_reply(
                &scope,
                &selected,
                "owner-a",
                "permission-key-1",
                NativePermissionDecision::AllowOnce,
            )
            .expect("identical idempotency replay returns current state");
        assert!(matches!(
            replay,
            NativePermissionReplyStart::Replay(NativePermissionPromptView {
                sequence: 4,
                state: NativePermissionState::Replying,
                ..
            })
        ));
        assert!(matches!(
            registry.begin_reply(
                &scope,
                &selected,
                "owner-a",
                "permission-key-1",
                NativePermissionDecision::RejectSessionPending,
            ),
            Err(NativePermissionRegistryError::ReplyConflict)
        ));
        assert!(
            matches!(
                registry.begin_reply(
                    &scope,
                    &second,
                    "owner-a",
                    "permission-key-1",
                    NativePermissionDecision::AllowOnce,
                ),
                Err(NativePermissionRegistryError::ReplyConflict)
            ),
            "one principal cannot reuse a reply key for another prompt"
        );
        let acknowledged =
            registry.acknowledge_reply(&scope, &selected, NativePermissionDecision::AllowOnce);
        assert_eq!(acknowledged.len(), 1);
        assert_eq!(acknowledged[0].sequence(), 5);

        assert!(matches!(
            registry.begin_reply(
                &scope,
                &second,
                "owner-b",
                "permission-key-1",
                NativePermissionDecision::RejectSessionPending,
            ),
            Ok(NativePermissionReplyStart::Dispatch(_))
        ));
        let mut pending = HashSet::new();
        pending.insert("permission-third".to_owned());
        registry.reconcile_reply(
            &scope,
            &second,
            NativePermissionDecision::RejectSessionPending,
            Some(pending),
        );
        assert_eq!(
            registry
                .view_for_scope(&scope, &second)
                .expect("selected prompt")
                .state,
            NativePermissionState::OutcomeUnknown
        );
        assert_eq!(
            registry
                .view_for_scope(&scope, &second)
                .expect("unknown prompt")
                .sequence(),
            7
        );
        assert_eq!(
            registry
                .view_for_scope(&scope, &third)
                .expect("other pending prompt")
                .state,
            NativePermissionState::Pending,
            "a pending-list result keeps other still-pending session prompts actionable"
        );
        assert_eq!(
            registry
                .view_for_scope(&scope, &third)
                .expect("pending prompt")
                .sequence(),
            8
        );
        assert_eq!(
            registry
                .view_for_scope(&scope, &second)
                .expect("selected prompt")
                .decision(),
            Some("REJECT_SESSION_PENDING"),
            "an unresolved outcome retains the attempted decision for honest display"
        );
        assert_eq!(
            registry
                .view_for_scope(&scope, &third)
                .expect("other pending prompt")
                .decision(),
            None,
            "a confirmed still-pending prompt does not retain a stale attempted decision"
        );

        assert!(matches!(
            registry.begin_reply(
                &scope,
                &third,
                "owner-b",
                "permission-key-2",
                NativePermissionDecision::RejectSessionPending,
            ),
            Ok(NativePermissionReplyStart::Dispatch(_))
        ));
        registry.reconcile_reply(
            &scope,
            &third,
            NativePermissionDecision::RejectSessionPending,
            None,
        );
        assert_eq!(
            registry
                .view_for_scope(&scope, &third)
                .expect("third prompt")
                .state,
            NativePermissionState::OutcomeUnknown,
            "an unreadable pending list disables every possibly affected rejection"
        );
        assert_eq!(
            registry
                .view_for_scope(&scope, &third)
                .expect("unknown prompt")
                .sequence(),
            10
        );
    }

    #[test]
    fn native_permission_registry_rejects_foreign_session_and_conflicting_duplicate() {
        let registry = NativePermissionRegistry::default();
        let scope = NativePermissionScope {
            workspace_id: "ws-one".to_owned(),
            conversation_id: "conv-one".to_owned(),
            turn_id: "turn-one".to_owned(),
            retry_ordinal: 0,
            agent_session_id: "agent-session-one".to_owned(),
            runtime_incarnation_id: "inc-one".to_owned(),
            native_session_id: "native-session-one".to_owned(),
        };
        assert_eq!(
            registry.register_with_token(
                &scope,
                "foreign-native-session",
                "opaque-prompt-token-000000000000000000000001".to_owned(),
                "native-request-one".to_owned(),
                "readFile".to_owned(),
                None,
                vec![],
            ),
            Err(NativePermissionRegistryError::ForeignSession)
        );
        let event = NormalizedAgentEvent::PermissionAsked {
            permission_id: "native-request-one".to_owned(),
            native_session_id: scope.native_session_id.clone(),
            action: "readFile".to_owned(),
            resources: vec!["/private/path".to_owned()],
            message: None,
        };
        let registered = registry
            .register(&scope, &event, None)
            .expect("generate an opaque prompt token");
        assert_eq!(registered.prompt_token.len(), 64);
        assert!(
            registered
                .prompt_token
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        );
    }

    #[test]
    fn native_permission_observer_publishes_only_scoped_sanitized_frame() {
        let registry = NativePermissionRegistry::default();
        let bus = crate::transient_frames::TransientFrameBus::for_tests();
        let mut receiver = bus.subscribe();
        let scope = NativePermissionScope {
            workspace_id: "ws-one".to_owned(),
            conversation_id: "conv-one".to_owned(),
            turn_id: "turn-one".to_owned(),
            retry_ordinal: 2,
            agent_session_id: "agent-session-one".to_owned(),
            runtime_incarnation_id: "inc-one".to_owned(),
            native_session_id: "native-session-secret".to_owned(),
        };
        let workspace = tempfile::tempdir().expect("temporary Workspace root");
        let resource = workspace.path().join("src").join("main.rs");
        std::fs::create_dir_all(resource.parent().expect("resource parent"))
            .expect("create resource directory");
        std::fs::write(&resource, "fn main() {}").expect("create resource");
        let canonical_root = std::fs::canonicalize(workspace.path()).expect("canonical root");
        let mut observer = NativePermissionObserver::new(
            registry.clone(),
            scope.clone(),
            bus,
            Some(canonical_root),
        );
        let event = NormalizedAgentEvent::PermissionAsked {
            permission_id: "native-request-secret".to_owned(),
            native_session_id: "native-session-secret".to_owned(),
            action: "writeFile".to_owned(),
            resources: vec![resource.to_string_lossy().into_owned()],
            message: Some("Please update this file".to_owned()),
        };
        observer.on_event(&event);

        let frame = receiver.try_recv().expect("permission event published");
        let value: serde_json::Value =
            serde_json::from_slice(&frame.bytes).expect("valid permission frame");
        assert_eq!(value["type"], "turn.permission");
        assert_eq!(value["workspace_id"], "ws-one");
        assert_eq!(value["conversation_id"], "conv-one");
        assert_eq!(value["turn_id"], "turn-one");
        assert_eq!(value["retry_ordinal"], 2);
        assert_eq!(value["sequence"], 1);
        assert_eq!(value["state"], "PENDING");
        assert_eq!(value["action"], "writeFile");
        assert_eq!(value["resources"], serde_json::json!(["src/main.rs"]));
        let serialized = value.to_string();
        assert!(!serialized.contains("native-session-secret"));
        assert!(!serialized.contains("native-request-secret"));
        assert!(!serialized.contains(resource.to_string_lossy().as_ref()));

        let token = value["prompt_token"].as_str().expect("opaque token");
        registry.acknowledge_reply(&scope, token, NativePermissionDecision::AllowOnce);
        observer.on_event(&event);
        let duplicate = receiver
            .try_recv()
            .expect("duplicate permission event republishes its settled state");
        let duplicate_value: serde_json::Value =
            serde_json::from_slice(&duplicate.bytes).expect("valid duplicate frame");
        assert_eq!(duplicate_value["sequence"], 2);
        assert_eq!(duplicate_value["state"], "ANSWERED");
    }

    #[test]
    fn native_permission_mailbox_publishes_session_end_before_discarding_prompt_scope() {
        let registry = NativePermissionRegistry::default();
        let bus = crate::transient_frames::TransientFrameBus::for_tests();
        let mut receiver = bus.subscribe();
        let scope = NativePermissionScope {
            workspace_id: "ws-end".to_owned(),
            conversation_id: "conv-end".to_owned(),
            turn_id: "turn-end".to_owned(),
            retry_ordinal: 0,
            agent_session_id: "agent-session-end".to_owned(),
            runtime_incarnation_id: "inc-end".to_owned(),
            native_session_id: "native-session-private".to_owned(),
        };
        let (sender, _receiver) = sync_channel(MAX_NATIVE_PERMISSION_COMMANDS_PER_SESSION);
        let registration = registry
            .register_mailbox(scope.clone(), sender, bus.clone())
            .expect("attempt mailbox is registered");
        let mut observer =
            NativePermissionObserver::new(registry.clone(), scope.clone(), bus, None);
        observer.on_event(&NormalizedAgentEvent::PermissionAsked {
            permission_id: "native-request-private".to_owned(),
            native_session_id: scope.native_session_id.clone(),
            action: "writeFile".to_owned(),
            resources: vec!["C:/private/file.txt".to_owned()],
            message: None,
        });
        let pending = receiver.try_recv().expect("pending prompt frame");
        let pending_value: serde_json::Value =
            serde_json::from_slice(&pending.bytes).expect("pending JSON");
        assert_eq!(pending_value["sequence"], 1);
        assert_eq!(pending_value["state"], "PENDING");

        drop(registration);
        let ended = receiver
            .try_recv()
            .expect("terminal prompt frame precedes scope removal");
        let ended_value: serde_json::Value =
            serde_json::from_slice(&ended.bytes).expect("terminal JSON");
        assert_eq!(ended_value["sequence"], 2);
        assert_eq!(ended_value["state"], "SESSION_ENDED");
        let serialized = ended_value.to_string();
        assert!(!serialized.contains("native-session-private"));
        assert!(!serialized.contains("native-request-private"));
        assert!(!serialized.contains("C:/private/file.txt"));

        let snapshot = registry.snapshot(
            &scope.workspace_id,
            &scope.conversation_id,
            &scope.turn_id,
            scope.retry_ordinal,
            &scope.agent_session_id,
            &scope.runtime_incarnation_id,
        );
        assert!(
            snapshot.items.is_empty(),
            "prompt tokens are discarded after terminal delivery"
        );
    }

    #[test]
    fn activation_versions_derive_from_committed_starting_version() {
        assert_eq!(session_activation_versions(17), Some((17, 18)));
        assert_eq!(session_activation_versions(0), None);
        assert_eq!(session_activation_versions(u64::MAX), None);
    }
    use storage_core::StoreError;

    #[test]
    fn per_session_host_state_follows_observed_process_shutdown() {
        let order = std::cell::RefCell::new(Vec::new());
        let stopped = reconcile_per_session_host_shutdown(
            || {
                order.borrow_mut().push("STOPPING");
                Ok(())
            },
            || {
                order.borrow_mut().push("stop");
                true
            },
            || {
                order.borrow_mut().push("STOPPED");
                Ok(())
            },
        )
        .expect("observed process exit should persist STOPPED after STOPPING");
        assert!(stopped);
        assert_eq!(*order.borrow(), ["STOPPING", "stop", "STOPPED"]);

        let order = std::cell::RefCell::new(Vec::new());
        let stopped = reconcile_per_session_host_shutdown(
            || {
                order.borrow_mut().push("STOPPING");
                Ok(())
            },
            || {
                order.borrow_mut().push("stop");
                false
            },
            || {
                order.borrow_mut().push("STOPPED");
                Ok(())
            },
        )
        .expect("unconfirmed exit remains durably STOPPING");
        assert!(!stopped);
        assert_eq!(*order.borrow(), ["STOPPING", "stop"]);

        let order = std::cell::RefCell::new(Vec::new());
        let result = reconcile_per_session_host_shutdown(
            || Err("cannot persist STOPPING".to_owned()),
            || {
                order.borrow_mut().push("stop");
                true
            },
            || Ok(()),
        );
        assert!(result.is_err());
        assert!(
            order.borrow().is_empty(),
            "do not stop before durable STOPPING"
        );
    }

    #[test]
    fn response_commit_identity_is_stable_scoped_and_contains_no_native_identifier() {
        let private_message = "native-message-canary-do-not-persist";
        let identity = provider_response_commit_identity("turn-a", 0, private_message);
        assert_eq!(
            identity,
            provider_response_commit_identity("turn-a", 0, private_message)
        );
        assert!(!identity.contains(private_message));
        assert_eq!(identity.len(), "response-sha256:".len() + 64);
        assert_ne!(
            identity,
            provider_response_commit_identity("turn-b", 0, private_message)
        );
        assert_ne!(
            identity,
            provider_response_commit_identity("turn-a", 1, private_message)
        );
        assert_ne!(
            identity,
            provider_response_commit_identity("turn-a", 0, "other-message")
        );
        assert_ne!(
            provider_response_commit_identity("a", 0, "bc"),
            provider_response_commit_identity("ab", 0, "c")
        );
    }

    #[test]
    fn conversation_usage_attribution_prefers_reported_then_estimate_then_unknown() {
        use super::super::adapter::ReportedUsage;
        let reported = ReportedUsage {
            input_tokens: 10,
            output_tokens: 20,
            observed: true,
        };
        let usage = turn_usage(reported, 4_000, 400).expect("reported usage");
        assert_eq!(
            usage.attribution_source,
            UsageAttributionSource::ProviderReported
        );
        assert_eq!((usage.input_tokens, usage.output_tokens), (10, 20));

        // A zero-token report (fixture-style) is not provider usage: estimate instead.
        let zero = ReportedUsage {
            input_tokens: 0,
            output_tokens: 0,
            observed: true,
        };
        let usage = turn_usage(zero, 4_000, 3).expect("estimated usage");
        assert_eq!(
            usage.attribution_source,
            UsageAttributionSource::EstimatedLocal
        );
        assert_eq!(
            (usage.input_tokens, usage.output_tokens),
            (1_000, 1),
            "max(1, bytes / 4)"
        );

        // Nothing exchanged and nothing reported is UNKNOWN (None).
        assert!(turn_usage(ReportedUsage::default(), 0, 0).is_none());
        // A failed attempt with only the prompt sent estimates input and no output.
        let usage = turn_usage(ReportedUsage::default(), 40, 0).expect("input only");
        assert_eq!((usage.input_tokens, usage.output_tokens), (10, 0));
    }

    #[test]
    fn conversation_usage_collector_uses_step_deltas_not_session_snapshots() {
        use super::super::adapter::{TurnObserver, UsageCollector};
        use super::super::normalizer::NormalizedAgentEvent;
        let mut collector = UsageCollector::default();
        collector.on_event(&NormalizedAgentEvent::UsageSnapshot {
            input_tokens: Some(100),
            output_tokens: Some(200),
        });
        collector.on_event(&NormalizedAgentEvent::UsageSnapshot {
            input_tokens: Some(120),
            output_tokens: Some(230),
        });
        collector.on_event(&NormalizedAgentEvent::UsageSnapshot {
            input_tokens: Some(130),
            output_tokens: Some(240),
        });
        collector.on_event(&NormalizedAgentEvent::StepEnded {
            input_tokens: Some(10),
            output_tokens: Some(20),
        });
        collector.on_event(&NormalizedAgentEvent::StepFailed {
            input_tokens: Some(5),
            output_tokens: Some(0),
        });
        let total = collector.snapshot();
        assert_eq!(
            (total.input_tokens, total.output_tokens, total.observed),
            (15, 20, true)
        );
    }

    #[test]
    fn conversation_final_settlement_retries_only_transient_store_failures() {
        let mut calls = 0;
        let committed = retry_transient_store_commit(3, Duration::from_millis(1), || {
            calls += 1;
            if calls < 3 {
                Err(StoreError::Busy)
            } else {
                Ok("receipt")
            }
        });
        assert_eq!(committed.expect("third attempt commits"), "receipt");
        assert_eq!(calls, 3);

        let mut calls = 0;
        let rejected: Result<(), _> =
            retry_transient_store_commit(3, Duration::from_millis(1), || {
                calls += 1;
                Err(StoreError::Invalid(
                    "CONVERSATION_RESPONSE_IDENTITY_CONFLICT".to_owned(),
                ))
            });
        assert!(matches!(rejected, Err(StoreError::Invalid(_))));
        assert_eq!(calls, 1, "a deterministic rejection is never retried");

        let mut calls = 0;
        let exhausted: Result<(), _> =
            retry_transient_store_commit(2, Duration::from_millis(1), || {
                calls += 1;
                Err(StoreError::Database("busy".to_owned()))
            });
        assert!(matches!(exhausted, Err(StoreError::Database(_))));
        assert_eq!(calls, 2);
    }

    #[test]
    fn conversation_early_dispatch_failure_retries_settlement_and_keeps_closed_class() {
        let mut calls = 0;
        let class = settle_dispatch_failure(DispatchFailureClass::ProfileLookup, || {
            calls += 1;
            if calls == 1 {
                Err(StoreError::Busy)
            } else {
                Ok(())
            }
        });
        assert_eq!(class, DispatchFailureClass::ProfileLookup);
        assert_eq!(calls, 2, "transient settlement errors are retried");

        let class = settle_dispatch_failure(DispatchFailureClass::HostResolution, || {
            Err(StoreError::Invalid(
                "provider secret must not leak".to_owned(),
            ))
        });
        assert_eq!(class, DispatchFailureClass::Settlement);
        assert_eq!(
            class.as_str(),
            "SETTLEMENT",
            "permanent settlement details collapse to a closed class for recovery"
        );
    }

    #[test]
    fn conversation_unavailable_host_settlement_retries_transient_store_failure() {
        let mut calls = 0;
        let result = settle_unavailable_host_attempt(|| {
            calls += 1;
            if calls == 1 {
                Err(StoreError::Busy)
            } else {
                Ok(())
            }
        });

        assert_eq!(result, Ok(()));
        assert_eq!(calls, 2, "transient no-host settlement is retried");
    }

    #[test]
    fn conversation_dispatch_owner_registry_suppresses_exact_replay_until_worker_exits() {
        let registry = DispatchRegistry::default();
        let key = DispatchAttemptKey {
            workspace_id: "ws-1".to_owned(),
            conversation_id: "conv-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            retry_ordinal: 0,
        };

        let first = registry
            .register(key.clone())
            .expect("first dispatcher owns the attempt");
        assert!(
            registry.register(key.clone()).is_none(),
            "an exact idempotent replay must not start a competing pre-claim worker"
        );
        let another_retry = registry
            .register(DispatchAttemptKey {
                retry_ordinal: 1,
                ..key.clone()
            })
            .expect("a later retry is a distinct attempt");
        let another_conversation = registry
            .register(DispatchAttemptKey {
                conversation_id: "conv-2".to_owned(),
                ..key.clone()
            })
            .expect("attempt ownership is scoped to its Conversation");

        drop(first);
        let after_worker_exit = registry
            .register(key)
            .expect("a settled worker releases local ownership for recovery/retry");
        drop((after_worker_exit, another_retry, another_conversation));
    }

    #[test]
    fn conversation_dispatch_claim_reconciliation_requires_exact_local_durable_owner() {
        use storage_core::conversation::{ConversationDispatchIntent, ConversationDispatchState};

        let intent = |state, owner: Option<&str>| ConversationDispatchIntent {
            workspace_id: "workspace-1".to_owned(),
            conversation_id: "conversation-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            retry_ordinal: 2,
            state,
            claimed_by_runtime_incarnation: owner.map(str::to_owned),
            created_at: "2026-11-23T00:00:00Z".to_owned(),
        };
        let local_claim = intent(ConversationDispatchState::Claimed, Some("runtime-local"));
        let remote_claim = intent(ConversationDispatchState::Claimed, Some("runtime-other"));
        let unclaimed = intent(ConversationDispatchState::Unclaimed, None);
        let malformed_claim = intent(ConversationDispatchState::Claimed, None);
        let settled = intent(ConversationDispatchState::Settled, Some("runtime-local"));

        assert_eq!(
            reconcile_dispatch_claim(Some(&local_claim), "runtime-local"),
            DispatchClaimReconciliation::Owned
        );
        assert_eq!(
            reconcile_dispatch_claim(Some(&remote_claim), "runtime-local"),
            DispatchClaimReconciliation::NotOwned
        );
        assert_eq!(
            reconcile_dispatch_claim(Some(&unclaimed), "runtime-local"),
            DispatchClaimReconciliation::Retry
        );
        assert_eq!(
            reconcile_dispatch_claim(Some(&malformed_claim), "runtime-local"),
            DispatchClaimReconciliation::MissingOrMalformed
        );
        assert_eq!(
            reconcile_dispatch_claim(Some(&settled), "runtime-local"),
            DispatchClaimReconciliation::NotOwned
        );
        assert_eq!(
            reconcile_dispatch_claim(None, "runtime-local"),
            DispatchClaimReconciliation::MissingOrMalformed
        );
    }

    #[test]
    fn conversation_stop_signal_reaches_only_the_registered_attempt_and_is_released() {
        let directory = tempfile::tempdir().expect("temporary directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("private test directory");
        }
        let store = storage_sqlite::SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            std::sync::Arc::new(storage_sqlite::FileBlobStore::new(
                directory.path().join("blobs"),
                NoKeys,
            )),
            storage_sqlite::SqliteConfig {
                writer_queue_capacity: 2,
                busy_timeout: Duration::from_secs(1),
            },
        )
        .expect("store opens");
        let supervisor = AgentSessionSupervisor::new(store, "runtime".to_owned());
        assert!(
            !supervisor.request_cancel("turn-1", 0),
            "nothing registered"
        );
        let (signal, registration) = supervisor.register_cancel("turn-1", 0);
        assert!(!signal.is_requested());
        assert!(
            !supervisor.request_cancel("turn-1", 1),
            "another retry ordinal"
        );
        assert!(!supervisor.request_cancel("turn-2", 0), "another turn");
        assert!(supervisor.request_cancel("turn-1", 0));
        assert!(signal.is_requested());
        drop(registration);
        assert!(
            !supervisor.request_cancel("turn-1", 0),
            "released when dispatch ends"
        );
    }

    struct NoKeys;

    impl storage_sqlite::WorkspaceBlobKeyProvider for NoKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: storage_core::BlobPurpose,
        ) -> Result<storage_sqlite::WorkspaceBlobKey, StoreError> {
            Ok(storage_sqlite::WorkspaceBlobKey {
                version: 1,
                bytes: zeroize::Zeroizing::new([7_u8; 32]),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: storage_core::BlobPurpose,
            _version: u32,
        ) -> Result<storage_sqlite::WorkspaceBlobKey, StoreError> {
            self.current_key(workspace_id, purpose)
        }
    }
}
