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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use storage_core::conversation::{
    CommitConversationAgentResponse, ConversationMessageRecord, ConversationSessionEnd,
    ConversationTurn, ConversationTurnStatus, ConversationTurnStore, ConversationTurnUsage,
    SettleConversationSession, SettleConversationTurn, TurnCommand, UsageAttributionSource,
};
use storage_core::{
    ActivateConversationAgentSession, AgentCatalogStore, AgentHostInstanceRecord, AgentHostStore,
    AgentSessionRecord, AgentSessionStore, ConversationAgentSessionStart, EventDraft,
    MarkStartingConversationAgentSessionLost, WorkspaceCreateRequest,
};
use storage_sqlite::{SqliteConversationStore, SqliteWorkspaceStore};

use super::adapter::{
    AgentAdapter, CancelSignal, ObserverSet, OpenCodeAdapter, StreamBudget, TurnObservation,
    UsageCollector,
};
use super::native_provider::NativeProviderKey;
use super::opencode_server::{
    OpenCodeEnvironment, OpenCodeServer, OpenCodeServerConfig, OpenCodeServerProtocol,
};

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

/// Live per-turn Stop signals keyed by `(turn_id, retry_ordinal)`.
type CancelRegistry = Arc<Mutex<HashMap<(String, u32), CancelSignal>>>;

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
}

impl AgentSessionSupervisor {
    pub fn new(store: SqliteWorkspaceStore, runtime_id: String) -> Self {
        Self {
            store,
            runtime_id,
            cancels: Arc::new(Mutex::new(HashMap::new())),
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
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn: ConversationTurn,
        correlation_id: &str,
    ) -> Result<(), String> {
        let store = self.store.clone();
        let runtime_id = self.runtime_id.clone();
        let supervisor = self.clone();
        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let conversation = conversation_id.to_owned();
        let turn_id = turn.turn_id.clone();
        let correlation = correlation_id.to_owned();

        tokio::task::spawn_blocking(move || {
            let conv_store = SqliteConversationStore::new(store.clone());

            // 1. Resolve host for turn
            let host_opt = conv_store
                .resolve_conversation_session_host_for_turn(
                    &principal,
                    &workspace,
                    &conversation,
                    &turn_id,
                )
                .map_err(|e| format!("Host resolution error: {e:?}"))?;

            let Some(host) = host_opt else {
                // No eligible host: settle the attempt FAILED (AGENT_UNAVAILABLE).
                let _ = settle_attempt_failed(
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
                );
                return Ok(());
            };

            // 2. Query provider key and profile via published AgentCatalogStore trait
            let now = now_utc_rfc3339();
            let binding = store
                .get_agent_binding(&principal, &workspace, &host.agent_binding_id)
                .map_err(|e| format!("Query agent binding error: {e:?}"))?
                .ok_or_else(|| "AgentBinding not found".to_owned())?;

            let profiles = store
                .list_agent_profiles(&principal, &workspace, &now)
                .map_err(|e| format!("List agent profiles error: {e:?}"))?;

            let provider_key = profiles
                .iter()
                .find(|p| p.profile.agent_profile_id == binding.agent_profile_id)
                .map(|p| p.profile.provider_key.clone())
                .ok_or_else(|| {
                    format!(
                        "No agent profile found for binding {} with profile ID {}",
                        binding.agent_binding_id, binding.agent_profile_id
                    )
                })?;

            // 3. Ensure host instance exists in SQLite via published AgentHostStore trait
            let host_instance_id = format!("host-{}", host.endpoint_id);
            let existing_host = store
                .get_agent_host_instance(
                    &host.runtime_id,
                    &host.runtime_incarnation_id,
                    &host_instance_id,
                )
                .map_err(|e| format!("Get host instance error: {e:?}"))?;

            if existing_host.is_none() {
                let _ = store.create_agent_host_instance(AgentHostInstanceRecord {
                    host_instance_id: host_instance_id.clone(),
                    runtime_id: host.runtime_id.clone(),
                    runtime_incarnation_id: host.runtime_incarnation_id.clone(),
                    agent_profile_id: binding.agent_profile_id.clone(),
                    endpoint_id: host.endpoint_id.clone(),
                    hosting_mode: "LOCAL_PER_SESSION".to_owned(),
                    state: "READY".to_owned(),
                    process_identity_ref: None,
                    ownership: "LITECOWORK".to_owned(),
                    started_at: now.clone(),
                    last_used_at: now.clone(),
                    idle_since: Some(now.clone()),
                });
            }

            // 3b. Claim the durable dispatch intent (compare-and-swap) before the STARTING
            // reservation. A losing dispatcher owns nothing and must not reserve a session.
            let claimed = conv_store
                .claim_conversation_dispatch_intent(
                    &turn_id,
                    turn.retry_ordinal,
                    &host.runtime_incarnation_id,
                    &now,
                )
                .map_err(|e| format!("Claim dispatch intent error: {e:?}"))?;
            if !claimed {
                return Ok(());
            }
            let (cancel, _cancel_registration) =
                supervisor.register_cancel(&turn_id, turn.retry_ordinal);

            // 4. Reserve STARTING session
            let session_id = generate_id("ses");
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

            let _committed_session = store
                .start_conversation_agent_session(start)
                .map_err(|e| format!("Start session error: {e:?}"))?;

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
                &host_instance_id,
                &host.agent_binding_id,
                &provider_key,
                &host.endpoint_id,
                &host.runtime_id,
                &host.runtime_incarnation_id,
                &correlation,
                &cancel,
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
                Err(err) => {
                    // Settle whatever the store currently holds: the attempt FAILED and
                    // its session LOST, so Retry can reserve a fresh session. A turn the
                    // user already cancelled or a settled one is left untouched.
                    let _ = settle_attempt_failed(
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
                    );
                    return Err(err);
                }
            }

            Ok(())
        })
        .await
        .map_err(|e| format!("Task execution error: {e:?}"))?
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
) -> Result<(), String> {
    let conv_store = SqliteConversationStore::new(store.clone());
    let turn = conv_store
        .get_conversation_turn(workspace_id, conversation_id, turn_id)
        .map_err(|e| format!("Read turn error: {e:?}"))?
        .ok_or_else(|| "ConversationTurn not found".to_owned())?;
    let session = match session_id.or(turn.agent_session_id.as_deref()) {
        Some(id) => store
            .get_agent_session(workspace_id, id)
            .map_err(|e| format!("Read session error: {e:?}"))?,
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
            store
                .mark_starting_conversation_agent_session_lost(
                    MarkStartingConversationAgentSessionLost {
                        workspace_id: workspace_id.to_owned(),
                        agent_session_id: record.agent_session_id.clone(),
                        expected_version: record.version,
                        occurred_at: now.clone(),
                        event,
                    },
                )
                .map_err(|e| format!("Release starting session error: {e:?}"))?;
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
        .map_err(|e| format!("Settle failed turn error: {e:?}"))
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
    host_instance_id: &str,
    agent_binding_id: &str,
    provider_key: &str,
    endpoint_id: &str,
    host_runtime_id: &str,
    host_runtime_incarnation_id: &str,
    correlation_id: &str,
    cancel: &CancelSignal,
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
            let config = OpenCodeServerConfig::bounded(exe, working_dir)
                .with_protocol(OpenCodeServerProtocol::V2_0_32)
                .with_environment(OpenCodeEnvironment::from_current_process());

            let server = OpenCodeServer::spawn(config)
                .map_err(|e| format!("OpenCodeServer spawn failed: {e:?}"))?;

            // Drive the native boundary through the provider-neutral AgentAdapter
            // (001-T04) rather than hand-parsing SSE. The OpenCode v2 HTTP path is
            // the one non-ACP adapter; it owns the server for the life of the turn.
            let usage_collector = UsageCollector::default();
            let mut adapter = OpenCodeAdapter::new(server)
                .with_cancel(cancel.clone())
                .with_observer(Box::new(ObserverSet(vec![
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

            // Phase 1: version/capability probe (B-03). A failed probe is a typed
            // FAILED turn with the USER message kept and no fallback (AC-015).
            adapter.probe_version().map_err(|f| {
                format!("OpenCode adapter version probe failed: {}", f.reason_code())
            })?;

            // Phase 2: create the native session; the handle binds to SQLite only.
            let native_session = adapter
                .create_session()
                .map_err(|f| format!("OpenCode create_session failed: {}", f.reason_code()))?;

            // Activate session in SQLite
            let now = now_utc_rfc3339();
            let activation = ActivateConversationAgentSession {
                principal_id: principal_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                agent_session_id: session_id.to_owned(),
                expected_session_version: 1,
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
                    entity_revision: 2,
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
            let observation = adapter.drive_turn(&native_session, StreamBudget::default());

            // The server is reaped regardless of outcome.
            adapter.shutdown();

            let accumulated_text = match observation {
                Ok(TurnObservation::Completed { answer_text }) => answer_text,
                Ok(TurnObservation::Stopped) => {
                    // Observed quiescence after a requested Stop: only now is
                    // CANCELLED committed, together with the session CLOSED.
                    settle_attempt_cancelled(
                        store,
                        runtime_id,
                        principal_id,
                        workspace_id,
                        conversation_id,
                        turn_id,
                        session_id,
                        correlation_id,
                        turn_usage(usage_collector.snapshot(), prompt_bytes, 0),
                    )?;
                    return Ok(TurnOutcome::Cancelled);
                }
                Ok(TurnObservation::StopUnconfirmed) => {
                    // The interrupt was sent but quiescence was not observed in time.
                    // The turn stays CANCEL_REQUESTED; CANCELLED is never fabricated.
                    return Ok(TurnOutcome::StopUnconfirmed);
                }
                Ok(TurnObservation::Failed { reason_code }) | Err(reason_code) => {
                    // Settle FAILED here so whatever usage was observed is recorded;
                    // the dispatcher's later settle of an already-settled turn is a no-op.
                    let _ = settle_attempt_failed(
                        store,
                        runtime_id,
                        principal_id,
                        workspace_id,
                        conversation_id,
                        turn_id,
                        Some(session_id),
                        reason_code.reason_code(),
                        correlation_id,
                        turn_usage(usage_collector.snapshot(), prompt_bytes, 0),
                    );
                    return Err(format!(
                        "OpenCode turn failed closed: {}",
                        reason_code.reason_code()
                    ));
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

            Ok(TurnOutcome::Committed)
        }
        _ => Err(format!(
            "Agent provider '{provider_key}' is not supported or not configured for execution in this runtime"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage_core::StoreError;

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
    fn conversation_usage_collector_sums_reported_tokens_across_steps() {
        use super::super::adapter::{TurnObserver, UsageCollector};
        use super::super::normalizer::NormalizedAgentEvent;
        let mut collector = UsageCollector::default();
        collector.on_event(&NormalizedAgentEvent::UsageRecorded {
            input_tokens: Some(5),
            output_tokens: None,
        });
        collector.on_event(&NormalizedAgentEvent::UsageRecorded {
            input_tokens: None,
            output_tokens: None,
        });
        collector.on_event(&NormalizedAgentEvent::UsageRecorded {
            input_tokens: Some(7),
            output_tokens: Some(9),
        });
        let total = collector.snapshot();
        assert_eq!(
            (total.input_tokens, total.output_tokens, total.observed),
            (12, 9, true)
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
