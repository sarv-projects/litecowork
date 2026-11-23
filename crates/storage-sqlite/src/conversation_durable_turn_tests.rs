//! Conversation durable turn storage: dispatch outbox, single-flight admission,
//! final-response commit identity and fault injection (001-T02, T10, T11).
use super::*;
use storage_core::conversation::{
    CommitConversationAgentResponse, ConversationDispatchState, ConversationMessageRecord,
    ConversationSessionEnd, ConversationTurnStatus, ConversationTurnStore, RuntimeExecutionFence,
    SettleConversationSession, SettleConversationTurn, TurnCommand,
};
use storage_core::{ActivateConversationAgentSession, AgentSessionStore, WorkspaceCreateRequest};

struct Activated {
    fixture: Fixture,
    store: crate::conversations::SqliteConversationStore,
    conversation_id: String,
    turn_id: String,
    session_id: String,
}

fn activated() -> Activated {
    let (fixture, _reserved, conversation_id, turn_id, session_id, host_id) =
        conversation_activation_fixture();
    fixture
        .store
        .activate_conversation_agent_session(ActivateConversationAgentSession {
            principal_id: OWNER_ID.to_owned(),
            workspace_id: WORKSPACE_ID.to_owned(),
            agent_session_id: session_id.clone(),
            expected_session_version: 1,
            expected_turn_version: 1,
            occurred_at: NOW.to_owned(),
            host_instance_id: host_id,
            native_session_ref: Some("ephemeral-native-session-token".to_owned()),
            session_event: draft(
                "event-session-started-durable",
                WORKSPACE_ID,
                "AgentSession",
                &session_id,
                2,
                "agent.session.started.v1",
                serde_json::json!({
                    "agent_session_id": session_id,
                    "task_spec_revision": null,
                    "scope": {"kind": "CONVERSATION", "conversation_id": conversation_id, "conversation_turn_id": turn_id},
                    "agent_binding_id": BINDING_ID,
                    "endpoint_id": ENDPOINT_ID,
                    "runtime_id": RUNTIME_ID,
                    "runtime_incarnation_id": INCARNATION_ID,
                    "session_state": "ACTIVE",
                    "reported_at": NOW,
                }),
            ),
            turn_event: draft(
                "event-turn-running-durable",
                WORKSPACE_ID,
                "ConversationTurn",
                &turn_id,
                2,
                "conversation.turn.status.changed.v1",
                serde_json::json!({
                    "turn_id": turn_id,
                    "from": "OPEN",
                    "to": "RUNNING",
                    "reason_code": "SESSION_READY",
                    "agent_session_id": session_id,
                    "aggregate_version": 2,
                }),
            ),
        })
        .expect("activation succeeds");
    let store = crate::conversations::SqliteConversationStore::new(fixture.store.clone());
    Activated {
        fixture,
        store,
        conversation_id,
        turn_id,
        session_id,
    }
}

fn retry_with_runtime_fence(
    activated: &Activated,
    expected_version: u64,
    event: EventDraft,
) -> Result<storage_core::conversation::ConversationTurn, StoreError> {
    activated
        .store
        .transition_conversation_turn_with_runtime_fence(
            OWNER_ID,
            WORKSPACE_ID,
            &activated.conversation_id,
            &activated.turn_id,
            expected_version,
            TurnCommand::Retry,
            event,
            Some(RuntimeExecutionFence {
                runtime_id: RUNTIME_ID.to_owned(),
                runtime_incarnation_id: INCARNATION_ID.to_owned(),
            }),
        )
}

impl Activated {
    fn commit(
        &self,
        request_id: &str,
        response_id: &str,
        text: &str,
    ) -> CommitConversationAgentResponse {
        CommitConversationAgentResponse {
            request: WorkspaceCreateRequest {
                principal_id: OWNER_ID.to_owned(),
                request_id: request_id.to_owned(),
                request_payload: serde_json::json!({"answer": text}),
            },
            principal_id: OWNER_ID.to_owned(),
            workspace_id: WORKSPACE_ID.to_owned(),
            conversation_id: self.conversation_id.clone(),
            turn_id: self.turn_id.clone(),
            agent_session_id: self.session_id.clone(),
            expected_turn_version: 2,
            expected_session_version: 2,
            retry_ordinal: 0,
            provider_response_id: response_id.to_owned(),
            turn_status: ConversationTurnStatus::Completed,
            usage: None,
            message: ConversationMessageRecord {
                message_id: format!("message-{request_id}"),
                conversation_id: self.conversation_id.clone(),
                author: serde_json::json!({"kind": "AGENT", "agent_binding_id": BINDING_ID}),
                role: "AGENT".to_owned(),
                agent_session_id: Some(self.session_id.clone()),
                agent_binding_id: Some(BINDING_ID.to_owned()),
                turn_id: Some(self.turn_id.clone()),
                content: serde_json::json!([{"kind": "TEXT", "text": text}]),
                resource_refs: serde_json::json!([]),
                source_channel_ref: None,
                created_at: NOW.to_owned(),
            },
            message_event: draft(
                &format!("event-msg-{request_id}"),
                WORKSPACE_ID,
                "Conversation",
                &self.conversation_id,
                3,
                "conversation.message.added.v1",
                serde_json::json!({"conversation_id": self.conversation_id, "message_id": format!("message-{request_id}"), "role": "AGENT"}),
            ),
            turn_event: draft(
                &format!("event-turn-{request_id}"),
                WORKSPACE_ID,
                "ConversationTurn",
                &self.turn_id,
                3,
                "conversation.turn.settled.v1",
                serde_json::json!({"turn_id": self.turn_id, "from": "RUNNING", "to": "COMPLETED", "aggregate_version": 3}),
            ),
            session_event: draft(
                &format!("event-session-{request_id}"),
                WORKSPACE_ID,
                "AgentSession",
                &self.session_id,
                3,
                "agent.session.closed.v1",
                serde_json::json!({"agent_session_id": self.session_id, "from": "ACTIVE", "to": "CLOSED", "aggregate_version": 3}),
            ),
        }
    }

    fn connection(&self) -> Connection {
        Connection::open(database_path(&self.fixture.directory)).expect("open fixture database")
    }

    fn counts(&self) -> (i64, i64, i64, i64, String, String) {
        self.connection()
            .query_row(
                "SELECT (SELECT COUNT(*) FROM conversation_messages WHERE role='AGENT'),
                        (SELECT COUNT(*) FROM conversation_response_commits),
                        (SELECT COUNT(*) FROM conversation_turn_usage_records),
                        (SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'req-%'),
                        (SELECT status FROM conversation_turns WHERE turn_id=?1),
                        (SELECT status FROM agent_sessions WHERE agent_session_id=?2)",
                params![self.turn_id, self.session_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("read commit state")
    }
}

#[test]
fn conversation_agent_response_commit_rejects_wrong_author_and_ordinal_without_partial_write() {
    let a = activated();
    let before = a.counts();
    assert_eq!(
        before,
        (0, 0, 0, 0, "RUNNING".to_owned(), "ACTIVE".to_owned())
    );

    let mut wrong_author = a.commit("req-author", "response-1", "hello");
    wrong_author.message.author =
        serde_json::json!({"kind": "AGENT", "agent_binding_id": "some-other-binding"});
    assert!(
        a.store
            .commit_conversation_agent_response(wrong_author)
            .is_err()
    );

    let mut user_author = a.commit("req-user-author", "response-1", "hello");
    user_author.message.author = serde_json::json!({"kind": "USER", "principal_id": OWNER_ID});
    assert!(
        a.store
            .commit_conversation_agent_response(user_author)
            .is_err()
    );

    let mut wrong_ordinal = a.commit("req-ordinal", "response-1", "hello");
    wrong_ordinal.retry_ordinal = 1;
    assert!(
        a.store
            .commit_conversation_agent_response(wrong_ordinal)
            .is_err()
    );

    let mut wrong_workspace = a.commit("req-workspace", "response-1", "hello");
    wrong_workspace.workspace_id = "another-workspace".to_owned();
    assert!(
        a.store
            .commit_conversation_agent_response(wrong_workspace)
            .is_err()
    );

    let mut wrong_session = a.commit("req-session", "response-1", "hello");
    wrong_session.agent_session_id = "another-session".to_owned();
    assert!(
        a.store
            .commit_conversation_agent_response(wrong_session)
            .is_err()
    );

    assert_eq!(a.counts(), before, "rejected commits write nothing");
}

#[test]
fn conversation_agent_response_commit_identity_replays_and_rejects_conflicts() {
    let a = activated();
    let first = a
        .store
        .commit_conversation_agent_response(a.commit("req-first", "response-1", "hello"))
        .expect("first commit");

    let replay = a
        .store
        .commit_conversation_agent_response(a.commit("req-second", "response-1", "hello"))
        .expect("same identity and content replays the stored receipt");
    assert_eq!(replay, first);

    let different_content =
        a.store
            .commit_conversation_agent_response(a.commit("req-third", "response-1", "changed"));
    assert!(matches!(
        different_content,
        Err(StoreError::Invalid(ref message)) if message == "CONVERSATION_RESPONSE_IDENTITY_CONFLICT"
    ));
    let different_response =
        a.store
            .commit_conversation_agent_response(a.commit("req-fourth", "response-2", "hello"));
    assert!(matches!(
        different_response,
        Err(StoreError::Invalid(ref message)) if message == "CONVERSATION_RESPONSE_IDENTITY_CONFLICT"
    ));
    let (agent_messages, identities, usage, _, turn, session) = a.counts();
    assert_eq!((agent_messages, identities, usage), (1, 1, 1));
    assert_eq!((turn.as_str(), session.as_str()), ("COMPLETED", "CLOSED"));
    let source: String = a
        .connection()
        .query_row(
            "SELECT attribution_source FROM conversation_turn_usage_records WHERE turn_id=?1",
            params![a.turn_id],
            |row| row.get(0),
        )
        .expect("usage record");
    assert_eq!(source, "UNKNOWN");
}

#[test]
fn conversation_agent_response_commit_rolls_back_on_fault_after_each_insert() {
    let a = activated();
    let before = a.counts();
    let faults = [
        ("conversation_messages", "INSERT", ""),
        ("conversation_response_commits", "INSERT", ""),
        ("conversation_turn_usage_records", "INSERT", ""),
        ("conversation_dispatch_intents", "UPDATE", ""),
        ("agent_session_host_bindings", "DELETE", ""),
        (
            "domain_events",
            "INSERT",
            "WHEN NEW.type = 'conversation.message.added.v1'",
        ),
        (
            "domain_events",
            "INSERT",
            "WHEN NEW.type = 'conversation.turn.settled.v1'",
        ),
        (
            "domain_events",
            "INSERT",
            "WHEN NEW.type = 'agent.session.closed.v1'",
        ),
        (
            "request_dedup",
            "INSERT",
            "WHEN NEW.request_id = 'req-fault'",
        ),
    ];
    for (index, (table, operation, condition)) in faults.iter().enumerate() {
        let connection = a.connection();
        connection
            .execute_batch(&format!(
                "CREATE TRIGGER fault_{index} BEFORE {operation} ON {table} {condition} BEGIN SELECT RAISE(ABORT, 'INJECTED_FAULT'); END;"
            ))
            .expect("install fault trigger");
        drop(connection);
        let result = a.store.commit_conversation_agent_response(a.commit(
            "req-fault",
            "response-1",
            "hello",
        ));
        assert!(
            result.is_err(),
            "fault on {operation} {table} must fail the commit"
        );
        assert_eq!(
            a.counts(),
            before,
            "fault on {operation} {table} rolls everything back"
        );
        a.connection()
            .execute_batch(&format!("DROP TRIGGER fault_{index};"))
            .expect("remove fault trigger");
    }
    a.store
        .commit_conversation_agent_response(a.commit("req-fault", "response-1", "hello"))
        .expect("commit succeeds once faults are removed");
}

#[test]
fn conversation_agent_response_commit_two_concurrent_commits_have_one_winner() {
    let a = Arc::new(activated());
    let handles: Vec<_> = (0..2)
        .map(|index| {
            let a = Arc::clone(&a);
            std::thread::spawn(move || {
                a.store.commit_conversation_agent_response(a.commit(
                    &format!("req-race-{index}"),
                    &format!("response-{index}"),
                    &format!("answer {index}"),
                ))
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("commit thread"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let (agent_messages, identities, ..) = a.counts();
    assert_eq!((agent_messages, identities), (1, 1));
}

#[test]
fn conversation_dispatch_intent_claim_is_compare_and_swap() {
    let a = activated();
    let unclaimed = a
        .store
        .list_conversation_dispatch_intents(ConversationDispatchState::Unclaimed, 10)
        .expect("list unclaimed");
    assert_eq!(unclaimed.len(), 1);
    assert_eq!(unclaimed[0].turn_id, a.turn_id);
    assert_eq!(unclaimed[0].workspace_id, WORKSPACE_ID);
    assert_eq!(unclaimed[0].retry_ordinal, 0);

    let store = Arc::new(a.store.clone());
    let wins: usize = (0..4)
        .map(|index| {
            let store = Arc::clone(&store);
            let turn_id = a.turn_id.clone();
            std::thread::spawn(move || {
                store
                    .claim_conversation_dispatch_intent(
                        &turn_id,
                        0,
                        &format!("incarnation-{index}"),
                        NOW,
                    )
                    .expect("claim attempt")
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|handle| usize::from(handle.join().expect("claim thread")))
        .sum();
    assert_eq!(wins, 1, "exactly one dispatcher wins the claim");
    assert!(
        a.store
            .list_conversation_dispatch_intents(ConversationDispatchState::Unclaimed, 10)
            .expect("list unclaimed")
            .is_empty()
    );
    let claimed = a
        .store
        .list_conversation_dispatch_intents(ConversationDispatchState::Claimed, 10)
        .expect("list claimed");
    assert_eq!(claimed.len(), 1);
    assert!(claimed[0].claimed_by_runtime_incarnation.is_some());

    assert!(
        a.store
            .release_conversation_dispatch_intent(&a.turn_id, 0)
            .expect("release")
    );
    assert!(
        !a.store
            .release_conversation_dispatch_intent(&a.turn_id, 0)
            .expect("release twice")
    );
    assert!(
        a.store
            .claim_conversation_dispatch_intent(&a.turn_id, 0, "incarnation-next", NOW)
            .expect("re-claim")
    );

    // Settlement through the final-response commit is terminal for the intent.
    a.store
        .commit_conversation_agent_response(a.commit("req-settle", "response-1", "hello"))
        .expect("commit settles");
    assert!(
        !a.store
            .claim_conversation_dispatch_intent(&a.turn_id, 0, "incarnation-late", NOW)
            .expect("late claim")
    );
    assert!(
        !a.store
            .release_conversation_dispatch_intent(&a.turn_id, 0)
            .expect("late release")
    );
    let delete = a
        .connection()
        .execute("DELETE FROM conversation_dispatch_intents", []);
    assert!(delete.is_err(), "intents are never deleted");
}

impl Activated {
    fn settle(
        &self,
        turn_version: u64,
        command: TurnCommand,
        to: &str,
        session_end: ConversationSessionEnd,
        session_version: u64,
    ) -> Result<storage_core::conversation::SettledConversationTurn, StoreError> {
        self.settle_with_usage(
            turn_version,
            command,
            to,
            session_end,
            session_version,
            None,
        )
    }

    fn settle_with_usage(
        &self,
        turn_version: u64,
        command: TurnCommand,
        to: &str,
        session_end: ConversationSessionEnd,
        session_version: u64,
        usage: Option<storage_core::conversation::ConversationTurnUsage>,
    ) -> Result<storage_core::conversation::SettledConversationTurn, StoreError> {
        let (session_event_type, session_to) = match session_end {
            ConversationSessionEnd::Closed => ("agent.session.closed.v1", "CLOSED"),
            ConversationSessionEnd::Lost => ("agent.session.lost.v1", "LOST"),
        };
        let from = if turn_version == 3 {
            "CANCEL_REQUESTED"
        } else {
            "RUNNING"
        };
        self.store.settle_conversation_turn(SettleConversationTurn {
            control_event: None,
            principal_id: OWNER_ID.to_owned(),
            workspace_id: WORKSPACE_ID.to_owned(),
            conversation_id: self.conversation_id.clone(),
            turn_id: self.turn_id.clone(),
            expected_turn_version: turn_version,
            command,
            turn_event: draft(
                &format!("event-settle-{to}-{turn_version}"),
                WORKSPACE_ID,
                "ConversationTurn",
                &self.turn_id,
                turn_version + 1,
                "conversation.turn.settled.v1",
                serde_json::json!({"turn_id": self.turn_id, "from": from, "to": to, "reason_code": "TEST", "agent_session_id": self.session_id, "aggregate_version": turn_version + 1}),
            ),
            session: Some(SettleConversationSession {
                agent_session_id: self.session_id.clone(),
                expected_session_version: session_version,
                end: session_end,
                occurred_at: NOW.to_owned(),
                session_event: draft(
                    &format!("event-session-end-{to}-{turn_version}"),
                    WORKSPACE_ID,
                    "AgentSession",
                    &self.session_id,
                    session_version + 1,
                    session_event_type,
                    serde_json::json!({"agent_session_id": self.session_id, "to": session_to}),
                ),
            }),
            usage,
        })
    }

    fn row(&self, sql: &str, key: &str) -> String {
        self.connection()
            .query_row(sql, params![key], |row| row.get::<_, String>(0))
            .expect("read row")
    }
}

#[test]
fn conversation_turn_settlement_fails_turn_ends_session_and_settles_intent_without_a_message() {
    let a = activated();
    let fail = || TurnCommand::Fail {
        settled_at: NOW.to_owned(),
    };

    assert!(
        a.settle(1, fail(), "FAILED", ConversationSessionEnd::Lost, 2)
            .is_err(),
        "stale turn version"
    );
    assert!(
        a.settle(2, fail(), "FAILED", ConversationSessionEnd::Lost, 1)
            .is_err(),
        "stale session version"
    );
    assert_eq!(
        a.counts().4,
        "RUNNING",
        "rejected settlements write nothing"
    );

    let settled = a
        .settle(2, fail(), "FAILED", ConversationSessionEnd::Lost, 2)
        .expect("failure settlement");
    assert_eq!(settled.turn.status, ConversationTurnStatus::Failed);
    assert_eq!(settled.session.expect("ended session").status, "LOST");
    assert_eq!(
        a.row(
            "SELECT status FROM agent_sessions WHERE agent_session_id=?1",
            &a.session_id
        ),
        "LOST"
    );
    assert_eq!(
        a.row(
            "SELECT state FROM conversation_dispatch_intents WHERE turn_id=?1",
            &a.turn_id
        ),
        "SETTLED"
    );
    assert_eq!(
        a.row("SELECT CAST(COUNT(*) AS TEXT) FROM agent_session_host_bindings WHERE agent_session_id=?1", &a.session_id),
        "0"
    );
    assert_eq!(a.counts().0, 0, "a failed turn leaves no AGENT message");
    assert_eq!(
        a.row(
            "SELECT attribution_source FROM conversation_turn_usage_records WHERE turn_id=?1",
            &a.turn_id
        ),
        "UNKNOWN",
        "a failure with nothing observed records UNKNOWN usage"
    );
}

#[test]
fn conversation_usage_record_written_at_settlement_keeps_the_reported_attribution_once() {
    use storage_core::conversation::{ConversationTurnUsage, UsageAttributionSource};
    let a = activated();
    let usage = ConversationTurnUsage {
        attribution_source: UsageAttributionSource::EstimatedLocal,
        input_tokens: 120,
        output_tokens: 7,
    };
    a.settle_with_usage(
        2,
        TurnCommand::Fail {
            settled_at: NOW.to_owned(),
        },
        "FAILED",
        ConversationSessionEnd::Lost,
        2,
        Some(usage),
    )
    .expect("failure settlement with observed usage");
    let (source, input, output): (String, i64, i64) = a
        .connection()
        .query_row(
            "SELECT attribution_source,input_tokens,output_tokens FROM conversation_turn_usage_records WHERE turn_id=?1",
            params![a.turn_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("usage record");
    assert_eq!(
        (source.as_str(), input, output),
        ("ESTIMATED_LOCAL", 120, 7)
    );
    assert_eq!(a.counts().2, 1, "exactly one usage row for the attempt");
}

#[test]
fn conversation_retry_reopens_failed_turn_with_new_ordinal_intent_and_single_flight() {
    let a = activated();
    a.settle(
        2,
        TurnCommand::Fail {
            settled_at: NOW.to_owned(),
        },
        "FAILED",
        ConversationSessionEnd::Lost,
        2,
    )
    .expect("failure settlement");
    let retry_event = |version: u64, ordinal: u32, id: &str| {
        draft(
            id,
            WORKSPACE_ID,
            "ConversationTurn",
            &a.turn_id,
            version,
            "conversation.turn.retried.v1",
            serde_json::json!({"turn_id": a.turn_id, "prior_agent_session_id": a.session_id, "retry_ordinal": ordinal, "aggregate_version": version}),
        )
    };
    // Stale version and a payload that still carries a session are refused.
    assert!(retry_with_runtime_fence(&a, 2, retry_event(4, 1, "event-retry-stale")).is_err());
    let mut with_session = retry_event(4, 1, "event-retry-session");
    with_session.payload["agent_session_id"] = serde_json::json!("fresh-session");
    assert!(retry_with_runtime_fence(&a, 3, with_session).is_err());

    let retried = retry_with_runtime_fence(&a, 3, retry_event(4, 1, "event-retry-ok"))
        .expect("retry reopens the failed turn");
    assert_eq!(retried.status, ConversationTurnStatus::Open);
    assert_eq!(retried.retry_ordinal, 1);
    assert_eq!(retried.agent_session_id, None);
    assert_eq!(retried.settled_at, None);

    let intents = a
        .store
        .list_conversation_dispatch_intents(ConversationDispatchState::Unclaimed, 10)
        .expect("list intents");
    assert_eq!(intents.len(), 1);
    assert_eq!(
        (intents[0].turn_id.as_str(), intents[0].retry_ordinal),
        (a.turn_id.as_str(), 1)
    );
    assert_eq!(a.row("SELECT CAST(COUNT(*) AS TEXT) FROM conversation_messages WHERE role='USER' AND turn_id=?1", &a.turn_id), "1", "no duplicate USER message");

    // Retry of a non-FAILED turn is rejected, and single-flight blocks a re-open while
    // another turn is in flight.
    assert!(retry_with_runtime_fence(&a, 4, retry_event(5, 2, "event-retry-open")).is_err());
    let replay = a
        .store
        .find_conversation_turn_command_replay(
            WORKSPACE_ID,
            &a.conversation_id,
            &a.turn_id,
            "no-such-request",
        )
        .expect("replay lookup");
    assert!(replay.is_none());
}

#[test]
fn retry_conversation_turn_requires_ready_runtime_and_replays_exact_receipt_after_degradation() {
    let a = activated();
    a.settle(
        2,
        TurnCommand::Fail {
            settled_at: NOW.to_owned(),
        },
        "FAILED",
        ConversationSessionEnd::Lost,
        2,
    )
    .expect("failure settlement");
    let mut retry = draft(
        "event-retry-runtime-fence",
        WORKSPACE_ID,
        "ConversationTurn",
        &a.turn_id,
        4,
        "conversation.turn.retried.v1",
        serde_json::json!({
            "turn_id": a.turn_id,
            "prior_agent_session_id": a.session_id,
            "retry_ordinal": 1,
            "aggregate_version": 4
        }),
    );
    retry.causation_id = Some("retry-runtime-fence-request".to_owned());
    let retried =
        retry_with_runtime_fence(&a, 3, retry.clone()).expect("ready Runtime admits Retry");
    assert_eq!(retried.status, ConversationTurnStatus::Open);
    let event_count_after_retry = a.row(
        "SELECT CAST(COUNT(*) AS TEXT) FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1",
        &a.turn_id,
    );
    let intent_count_after_retry = a.row(
        "SELECT CAST(COUNT(*) AS TEXT) FROM conversation_dispatch_intents WHERE turn_id=?1",
        &a.turn_id,
    );

    let current = a
        .fixture
        .store
        .read_local_lifecycle_snapshot(RUNTIME_ID, INCARNATION_ID)
        .expect("read the exact local Runtime lifecycle")
        .expect("local Runtime incarnation is current");
    a.fixture
        .store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            expected_version: current.incarnation.version,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("degrade the current local Runtime");

    let replay = retry_with_runtime_fence(&a, 3, retry)
        .expect("the exact Retry receipt replays before readiness is checked");
    assert_eq!(replay, retried);
    assert_eq!(
        a.row(
            "SELECT CAST(COUNT(*) AS TEXT) FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1",
            &a.turn_id,
        ),
        event_count_after_retry
    );
    assert_eq!(
        a.row(
            "SELECT CAST(COUNT(*) AS TEXT) FROM conversation_dispatch_intents WHERE turn_id=?1",
            &a.turn_id,
        ),
        intent_count_after_retry
    );

    // A separate failed turn proves a new Retry is refused while degraded, before
    // it can append an event or dispatch intent.
    let b = activated();
    b.settle(
        2,
        TurnCommand::Fail {
            settled_at: NOW.to_owned(),
        },
        "FAILED",
        ConversationSessionEnd::Lost,
        2,
    )
    .expect("failure settlement");
    let current = b
        .fixture
        .store
        .read_local_lifecycle_snapshot(RUNTIME_ID, INCARNATION_ID)
        .expect("read the exact local Runtime lifecycle")
        .expect("local Runtime incarnation is current");
    b.fixture
        .store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            expected_version: current.incarnation.version,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("degrade the current local Runtime");
    let before_refused_retry = b.row(
        "SELECT CAST(COUNT(*) AS TEXT) FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1",
        &b.turn_id,
    );
    let mut distinct_retry = draft(
        "event-retry-runtime-refused",
        WORKSPACE_ID,
        "ConversationTurn",
        &a.turn_id,
        4,
        "conversation.turn.retried.v1",
        serde_json::json!({
            "turn_id": b.turn_id,
            "prior_agent_session_id": b.session_id,
            "retry_ordinal": 1,
            "aggregate_version": 4
        }),
    );
    distinct_retry.causation_id = Some("retry-runtime-fence-new-request".to_owned());
    assert!(matches!(
        retry_with_runtime_fence(&b, 3, distinct_retry),
        Err(StoreError::Invalid(code)) if code == "RUNTIME_UNAVAILABLE"
    ));
    assert_eq!(
        b.row(
            "SELECT CAST(COUNT(*) AS TEXT) FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1",
            &b.turn_id,
        ),
        before_refused_retry
    );
    assert_eq!(
        b.row(
            "SELECT CAST(COUNT(*) AS TEXT) FROM conversation_dispatch_intents WHERE turn_id=?1",
            &b.turn_id,
        ),
        "1"
    );
}

#[test]
fn conversation_cancel_settles_cancelled_only_after_cancel_requested_and_keeps_racing_completion() {
    // Observed quiescence: RUNNING -> CANCEL_REQUESTED -> CANCELLED with the session CLOSED.
    let a = activated();
    let requested = a
        .store
        .transition_conversation_turn(
            OWNER_ID, WORKSPACE_ID, &a.conversation_id, &a.turn_id, 2, TurnCommand::RequestCancel,
            draft("event-cancel-request", WORKSPACE_ID, "ConversationTurn", &a.turn_id, 3, "conversation.turn.status.changed.v1",
                serde_json::json!({"turn_id": a.turn_id, "from": "RUNNING", "to": "CANCEL_REQUESTED", "reason_code": "USER_REQUESTED", "agent_session_id": a.session_id, "aggregate_version": 3})),
        )
        .expect("cancel request");
    assert_eq!(requested.status, ConversationTurnStatus::CancelRequested);
    // A failure that wins the race with Stop stays FAILED (AC-010).
    let failed = a
        .settle(
            3,
            TurnCommand::Fail {
                settled_at: NOW.to_owned(),
            },
            "FAILED",
            ConversationSessionEnd::Lost,
            2,
        )
        .expect("a failure observed during the stop stays FAILED");
    assert_eq!(failed.turn.status, ConversationTurnStatus::Failed);
    let b = activated();
    b.store
        .transition_conversation_turn(
            OWNER_ID, WORKSPACE_ID, &b.conversation_id, &b.turn_id, 2, TurnCommand::RequestCancel,
            draft("event-cancel-request-b", WORKSPACE_ID, "ConversationTurn", &b.turn_id, 3, "conversation.turn.status.changed.v1",
                serde_json::json!({"turn_id": b.turn_id, "from": "RUNNING", "to": "CANCEL_REQUESTED", "reason_code": "USER_REQUESTED", "agent_session_id": b.session_id, "aggregate_version": 3})),
        )
        .expect("cancel request");
    let settled = b
        .settle(
            3,
            TurnCommand::SettleCancelled {
                settled_at: NOW.to_owned(),
            },
            "CANCELLED",
            ConversationSessionEnd::Closed,
            2,
        )
        .expect("cancelled settlement after quiescence");
    assert_eq!(settled.turn.status, ConversationTurnStatus::Cancelled);
    assert_eq!(settled.session.expect("session").status, "CLOSED");
    assert_eq!(b.counts().0, 0, "a cancelled turn leaves no AGENT message");

    // A completion that raced the Stop request stays COMPLETED with its message.
    let c = activated();
    c.store
        .transition_conversation_turn(
            OWNER_ID, WORKSPACE_ID, &c.conversation_id, &c.turn_id, 2, TurnCommand::RequestCancel,
            draft("event-cancel-request-c", WORKSPACE_ID, "ConversationTurn", &c.turn_id, 3, "conversation.turn.status.changed.v1",
                serde_json::json!({"turn_id": c.turn_id, "from": "RUNNING", "to": "CANCEL_REQUESTED", "reason_code": "USER_REQUESTED", "agent_session_id": c.session_id, "aggregate_version": 3})),
        )
        .expect("cancel request");
    let mut raced = c.commit("req-race-complete", "response-1", "finished anyway");
    raced.expected_turn_version = 3;
    raced.turn_event.entity_revision = 4;
    raced.turn_event.payload["from"] = serde_json::json!("CANCEL_REQUESTED");
    raced.turn_event.payload["aggregate_version"] = serde_json::json!(4);
    let committed = c
        .store
        .commit_conversation_agent_response(raced)
        .expect("racing completion wins");
    assert_eq!(committed.turn.status, ConversationTurnStatus::Completed);
    assert_eq!(c.counts().0, 1);

    // A FAILED settlement of a CANCEL_REQUESTED commit is not accepted as a response.
    let d = activated();
    d.store
        .transition_conversation_turn(
            OWNER_ID, WORKSPACE_ID, &d.conversation_id, &d.turn_id, 2, TurnCommand::RequestCancel,
            draft("event-cancel-request-d", WORKSPACE_ID, "ConversationTurn", &d.turn_id, 3, "conversation.turn.status.changed.v1",
                serde_json::json!({"turn_id": d.turn_id, "from": "RUNNING", "to": "CANCEL_REQUESTED", "reason_code": "USER_REQUESTED", "agent_session_id": d.session_id, "aggregate_version": 3})),
        )
        .expect("cancel request");
    let mut failed_commit = d.commit("req-fail-commit", "response-1", "x");
    failed_commit.expected_turn_version = 3;
    failed_commit.turn_status = ConversationTurnStatus::Failed;
    assert!(
        d.store
            .commit_conversation_agent_response(failed_commit)
            .is_err()
    );
}

#[test]
fn turn_activity_records_are_reported_ordered_idempotent_scoped_and_immutable() {
    use storage_core::conversation::ConversationActivityRecord;
    let a = activated();
    let record =
        |sequence: u32, kind: &str, status: &str, label: &str| ConversationActivityRecord {
            turn_id: a.turn_id.clone(),
            retry_ordinal: 0,
            conversation_id: a.conversation_id.clone(),
            sequence,
            kind: kind.to_owned(),
            status: status.to_owned(),
            call_ref: Some(format!("call-{sequence}")),
            label: Some(label.to_owned()),
            created_at: NOW.to_owned(),
        };
    a.store
        .append_conversation_turn_activity(record(2, "TOOL", "SUCCEEDED", "bash"))
        .expect("append second");
    a.store
        .append_conversation_turn_activity(record(1, "TOOL", "STARTED", "bash"))
        .expect("append first");
    // An exact repeat of a sequence is a no-op, not a duplicate and not an error.
    a.store
        .append_conversation_turn_activity(record(1, "TOOL", "STARTED", "bash"))
        .expect("repeat is idempotent");

    let listed = a
        .store
        .list_conversation_turn_activity(WORKSPACE_ID, &a.conversation_id, &a.turn_id, 0, 50)
        .expect("list activity");
    assert_eq!(
        listed.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(listed[0].status, "STARTED");

    // Another Workspace and another retry ordinal see nothing.
    assert!(
        a.store
            .list_conversation_turn_activity(
                "other-workspace",
                &a.conversation_id,
                &a.turn_id,
                0,
                50
            )
            .expect("scoped")
            .is_empty()
    );
    assert!(
        a.store
            .list_conversation_turn_activity(WORKSPACE_ID, &a.conversation_id, &a.turn_id, 1, 50)
            .expect("ordinal")
            .is_empty()
    );

    // Always REPORTED, and rows cannot be rewritten into something else.
    let attribution: String = a
        .connection()
        .query_row(
            "SELECT attribution FROM conversation_turn_activity_records WHERE sequence=1",
            [],
            |row| row.get(0),
        )
        .expect("attribution");
    assert_eq!(attribution, "REPORTED");
    assert!(
        a.connection()
            .execute(
                "UPDATE conversation_turn_activity_records SET status='FAILED'",
                []
            )
            .is_err()
    );
    assert!(
        a.connection()
            .execute(
                "UPDATE conversation_turn_activity_records SET attribution='EFFECT'",
                []
            )
            .is_err()
    );

    // Invalid shapes are refused before any write.
    assert!(
        a.store
            .append_conversation_turn_activity(record(0, "TOOL", "STARTED", "bash"))
            .is_err()
    );
    assert!(
        a.store
            .append_conversation_turn_activity(record(1, "EFFECT", "STARTED", "bash"))
            .is_err(),
        "closed kind set"
    );
    assert!(
        a.store
            .append_conversation_turn_activity(record(3, "TOOL", "STARTED", &"x".repeat(129)))
            .is_err()
    );
}
