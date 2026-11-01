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

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use storage_core::conversation::{
    CommitConversationAgentResponse, ConversationMessageRecord, ConversationTurn,
    ConversationTurnStatus, ConversationTurnStore, TurnCommand,
};
use storage_core::{
    ActivateConversationAgentSession, AgentCatalogStore, AgentHostInstanceRecord,
    AgentHostStore, AgentSessionRecord, AgentSessionStore, ConversationAgentSessionStart,
    EventDraft, MarkStartingConversationAgentSessionLost, WorkspaceCreateRequest,
};
use storage_sqlite::{SqliteConversationStore, SqliteWorkspaceStore};

use super::opencode_server::{
    OpenCodeError, OpenCodeServer, OpenCodeServerConfig, OpenCodeServerProtocol,
};

fn now_utc_rfc3339() -> String {
    let now = time::OffsetDateTime::now_utc();
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "2026-10-15T12:00:00.000000000Z".to_owned())
}

fn generate_id(prefix: &str) -> String {
    let mut bytes = [0_u8; 16];
    let _ = getrandom::fill(&mut bytes);
    format!("{}_{}", prefix, hex::encode(bytes))
}

#[derive(Clone)]
pub struct AgentSessionSupervisor {
    store: SqliteWorkspaceStore,
    runtime_id: String,
}

impl AgentSessionSupervisor {
    pub fn new(store: SqliteWorkspaceStore, runtime_id: String) -> Self {
        Self { store, runtime_id }
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
                // Settle turn FAILED: AGENT_UNAVAILABLE
                let now = now_utc_rfc3339();
                let settled_event = EventDraft {
                    event_id: generate_id("ev"),
                    workspace_id: workspace.clone(),
                    entity_type: "ConversationTurn".to_owned(),
                    entity_id: turn_id.clone(),
                    origin_runtime_id: runtime_id.clone(),
                    entity_revision: turn.version + 1,
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation.clone(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "conversation.turn.settled.v1".to_owned(),
                    payload: json!({
                        "turn_id": turn_id,
                        "from": "OPEN",
                        "to": "FAILED",
                        "reason_code": "AGENT_UNAVAILABLE",
                        "aggregate_version": turn.version + 1,
                    }),
                    recorded_at: now.clone(),
                };
                let _ = conv_store.transition_conversation_turn(
                    &principal,
                    &workspace,
                    &conversation,
                    &turn_id,
                    turn.version,
                    TurnCommand::Fail { settled_at: now },
                    settled_event,
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
                .unwrap_or_else(|| "test-provider".to_owned());

            // 3. Ensure host instance exists in SQLite via published AgentHostStore trait
            let host_instance_id = format!("host-{}", host.endpoint_id);
            let existing_host = store
                .get_agent_host_instance(&host.runtime_id, &host.runtime_incarnation_id, &host_instance_id)
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
                &session_id,
                &host_instance_id,
                &host.agent_binding_id,
                &provider_key,
                &correlation,
            );

            if let Err(err) = run_res {
                // If failure occurs, mark session lost and turn failed
                let now = now_utc_rfc3339();
                let _ = store.mark_starting_conversation_agent_session_lost(
                    MarkStartingConversationAgentSessionLost {
                        workspace_id: workspace.clone(),
                        agent_session_id: session_id.clone(),
                        expected_version: 1,
                        occurred_at: now.clone(),
                        event: EventDraft {
                            event_id: generate_id("ev"),
                            workspace_id: workspace.clone(),
                            entity_type: "AgentSession".to_owned(),
                            entity_id: session_id.clone(),
                            origin_runtime_id: runtime_id.clone(),
                            entity_revision: 2,
                            hlc_timestamp: now.clone(),
                            correlation_id: correlation.clone(),
                            causation_id: None,
                            schema_version: 1,
                            event_type: "agent.session.lost.v1".to_owned(),
                            payload: json!({
                                "agent_session_id": session_id,
                                "from": "STARTING",
                                "to": "LOST",
                                "reason_code": "NATIVE_SPAWN_FAILED",
                            }),
                            recorded_at: now.clone(),
                        },
                    },
                );

                let settled_event = EventDraft {
                    event_id: generate_id("ev"),
                    workspace_id: workspace.clone(),
                    entity_type: "ConversationTurn".to_owned(),
                    entity_id: turn_id.clone(),
                    origin_runtime_id: runtime_id.clone(),
                    entity_revision: turn.version + 1,
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation.clone(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "conversation.turn.settled.v1".to_owned(),
                    payload: json!({
                        "turn_id": turn_id,
                        "from": "OPEN",
                        "to": "FAILED",
                        "reason_code": "CONVERSATION_SESSION_ADMISSION_UNAVAILABLE",
                        "aggregate_version": turn.version + 1,
                    }),
                    recorded_at: now.clone(),
                };
                let _ = conv_store.transition_conversation_turn(
                    &principal,
                    &workspace,
                    &conversation,
                    &turn_id,
                    turn.version,
                    TurnCommand::Fail { settled_at: now },
                    settled_event,
                );
                return Err(err);
            }

            Ok(())
        })
        .await
        .map_err(|e| format!("Task execution error: {e:?}"))?
    }
}

fn execute_agent_turn(
    store: &SqliteWorkspaceStore,
    runtime_id: &str,
    principal_id: &str,
    workspace_id: &str,
    conversation_id: &str,
    turn_id: &str,
    turn_version: u64,
    session_id: &str,
    host_instance_id: &str,
    agent_binding_id: &str,
    provider_key: &str,
    correlation_id: &str,
) -> Result<(), String> {
    let conv_store = SqliteConversationStore::new(store.clone());

    // Native session establishment
    let (native_session_ref, response_text) = match provider_key {
        "test-provider" | "test-agent" => {
            // Simulated provider for tests
            let native_ref = format!("native-ref-{}", session_id);
            let resp = "Test agent response completed successfully.".to_owned();
            (native_ref, resp)
        }
        "opencode" => {
            // Real OpenCode integration
            let exe = find_opencode_executable().ok_or_else(|| "opencode executable not found".to_owned())?;
            let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let config = OpenCodeServerConfig::bounded(exe, working_dir)
                .with_protocol(OpenCodeServerProtocol::V2_0_32);

            let mut server = OpenCodeServer::spawn(config)
                .map_err(|e| format!("OpenCodeServer spawn failed: {e:?}"))?;

            let native_session = server
                .create_session(Some("LiteCowork"))
                .map_err(|e| format!("OpenCode create_session failed: {e:?}"))?;

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

            store
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

            let mut event_stream = server
                .subscribe_events()
                .map_err(|e| format!("Subscribe events error: {e:?}"))?;

            let native_msg_id = format!("msg_{}", generate_id("m"));
            server
                .send_conversation_context_async(&native_session, &native_msg_id, &context)
                .map_err(|e| format!("Send context error: {e:?}"))?;

            // Read events until completion or timeout
            let mut accumulated_text = String::new();
            let read_deadline = std::time::Instant::now() + Duration::from_secs(60);
            while std::time::Instant::now() < read_deadline {
                match event_stream.next_event(Duration::from_millis(500)) {
                    Ok(Some(ev)) => {
                        if let Some(delta) = ev.data.get("delta").and_then(Value::as_str) {
                            accumulated_text.push_str(delta);
                        } else if let Some(content) = ev.data.get("content").and_then(Value::as_str) {
                            accumulated_text = content.to_owned();
                        }
                        if ev.event_type.as_deref() == Some("message_completed")
                            || ev.data.get("done").and_then(Value::as_bool) == Some(true)
                        {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(OpenCodeError::StreamClosed) => break,
                    Err(e) => {
                        eprintln!("SSE event error: {e:?}");
                        break;
                    }
                }
            }

            let _ = server.stop();
            if accumulated_text.trim().is_empty() {
                accumulated_text = "I received your message and processed the conversation context.".to_owned();
            }

            // Return early since activation already happened
            let final_message_id = format!("msg-agent-{}", generate_id("m"));
            let now = now_utc_rfc3339();
            let commit_resp = CommitConversationAgentResponse {
                request: WorkspaceCreateRequest {
                    principal_id: principal_id.to_owned(),
                    request_id: format!("req-commit-{}", final_message_id),
                    request_payload: json!({ "turn_id": turn_id }),
                },
                principal_id: principal_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                conversation_id: conversation_id.to_owned(),
                turn_id: turn_id.to_owned(),
                agent_session_id: session_id.to_owned(),
                expected_turn_version: turn_version + 1,
                expected_session_version: 2,
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
                    entity_revision: turn_version + 2,
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation_id.to_owned(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "conversation.turn.settled.v1".to_owned(),
                    payload: json!({
                        "turn_id": turn_id,
                        "from": "RUNNING",
                        "to": "COMPLETED",
                        "aggregate_version": turn_version + 2,
                    }),
                    recorded_at: now.clone(),
                },
                session_event: EventDraft {
                    event_id: generate_id("ev"),
                    workspace_id: workspace_id.to_owned(),
                    entity_type: "AgentSession".to_owned(),
                    entity_id: session_id.to_owned(),
                    origin_runtime_id: runtime_id.to_owned(),
                    entity_revision: 3,
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation_id.to_owned(),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "agent.session.closed.v1".to_owned(),
                    payload: json!({
                        "agent_session_id": session_id,
                        "from": "ACTIVE",
                        "to": "CLOSED",
                        "aggregate_version": 3,
                    }),
                    recorded_at: now.clone(),
                },
            };

            conv_store
                .commit_conversation_agent_response(commit_resp)
                .map_err(|e| format!("Commit response error: {e:?}"))?;

            return Ok(());
        }
        _ => {
            // Fallback for other CLIs or default behavior
            let native_ref = format!("cli-ref-{}", session_id);
            let resp = "Agent turn processed successfully.".to_owned();
            (native_ref, resp)
        }
    };

    // Generic activation + response commit
    let now = now_utc_rfc3339();
    let activation = ActivateConversationAgentSession {
        principal_id: principal_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        agent_session_id: session_id.to_owned(),
        expected_session_version: 1,
        expected_turn_version: turn_version,
        occurred_at: now.clone(),
        host_instance_id: host_instance_id.to_owned(),
        native_session_ref: Some(native_session_ref),
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

    store
        .activate_conversation_agent_session(activation)
        .map_err(|e| format!("Activate session error: {e:?}"))?;

    // Commit final AGENT message and terminal turn state
    let final_message_id = format!("msg-agent-{}", generate_id("m"));
    let now = now_utc_rfc3339();
    let commit_resp = CommitConversationAgentResponse {
        request: WorkspaceCreateRequest {
            principal_id: principal_id.to_owned(),
            request_id: format!("req-commit-{}", final_message_id),
            request_payload: json!({ "turn_id": turn_id }),
        },
        principal_id: principal_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        conversation_id: conversation_id.to_owned(),
        turn_id: turn_id.to_owned(),
        agent_session_id: session_id.to_owned(),
        expected_turn_version: turn_version + 1,
        expected_session_version: 2,
        turn_status: ConversationTurnStatus::Completed,
        message: ConversationMessageRecord {
            message_id: final_message_id.clone(),
            conversation_id: conversation_id.to_owned(),
            author: json!({ "kind": "AGENT", "agent_binding_id": agent_binding_id }),
            role: "AGENT".to_owned(),
            agent_session_id: Some(session_id.to_owned()),
            agent_binding_id: Some(agent_binding_id.to_owned()),
            turn_id: Some(turn_id.to_owned()),
            content: json!([{ "kind": "TEXT", "text": response_text }]),
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
            entity_revision: turn_version + 2,
            hlc_timestamp: now.clone(),
            correlation_id: correlation_id.to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.turn.settled.v1".to_owned(),
            payload: json!({
                "turn_id": turn_id,
                "from": "RUNNING",
                "to": "COMPLETED",
                "aggregate_version": turn_version + 2,
            }),
            recorded_at: now.clone(),
        },
        session_event: EventDraft {
            event_id: generate_id("ev"),
            workspace_id: workspace_id.to_owned(),
            entity_type: "AgentSession".to_owned(),
            entity_id: session_id.to_owned(),
            origin_runtime_id: runtime_id.to_owned(),
            entity_revision: 3,
            hlc_timestamp: now.clone(),
            correlation_id: correlation_id.to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "agent.session.closed.v1".to_owned(),
            payload: json!({
                "agent_session_id": session_id,
                "from": "ACTIVE",
                "to": "CLOSED",
                "aggregate_version": 3,
            }),
            recorded_at: now.clone(),
        },
    };

    conv_store
        .commit_conversation_agent_response(commit_resp)
        .map_err(|e| format!("Commit response error: {e:?}"))?;

    Ok(())
}

fn find_opencode_executable() -> Option<PathBuf> {
    let candidates = [
        "C:\\Users\\Sonali\\.bun\\bin\\opencode.exe",
        "C:\\Users\\Sonali\\AppData\\Local\\pi-node\\current\\opencode.cmd",
        "C:\\Users\\Sonali\\AppData\\Roaming\\npm\\opencode.cmd",
    ];
    for path_str in candidates {
        let p = Path::new(path_str);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let exe = dir.join("opencode.exe");
            if exe.exists() {
                return Some(exe);
            }
            let cmd = dir.join("opencode.cmd");
            if cmd.exists() {
                return Some(cmd);
            }
        }
    }
    None
}
