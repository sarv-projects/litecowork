use super::*;
use domain_conversation::{
    ConversationTurn, ConversationTurnStatus, PresentationPreference, TurnCommand,
};
use storage_core::{
    ConversationMessageRecord, ConversationRecord, ConversationStore, ConversationTurnStore,
    CreateConversationCommit, CreateConversationTurnCommit, WorkspaceCreateRequest,
};

const AT: &str = "2026-10-09T12:00:00Z";

fn fixture() -> (
    tempfile::TempDir,
    SqliteWorkspaceStore,
    CreateConversationTurnCommit,
) {
    let directory = tempfile::tempdir().expect("private temporary storage directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-turn");
    let conversation = ConversationRecord {
        conversation_id: "conversation-turn".into(),
        workspace_id: "workspace-turn".into(),
        title: None,
        active_agent_binding_id: None,
        version: 1,
        created_at: AT.into(),
    };
    let conversation_ctx = context("event-conversation-turn", 7);
    SqliteConversationStore::new(store.clone())
        .create_conversation(CreateConversationCommit {
            request: WorkspaceCreateRequest {
                principal_id: "owner-local".into(),
                request_id: "conversation-create-turn".into(),
                request_payload: serde_json::json!({"operation":"conversation.create.v1","workspace_id":"workspace-turn","title":null}),
            },
            event: EventDraft {
                event_id: conversation_ctx.event_id,
                workspace_id: conversation.workspace_id.clone(),
                entity_type: "Conversation".into(),
                entity_id: conversation.conversation_id.clone(),
                origin_runtime_id: conversation_ctx.origin_runtime_id,
                entity_revision: 1,
                hlc_timestamp: conversation_ctx.hlc_timestamp,
                correlation_id: conversation_ctx.correlation_id,
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.created.v1".into(),
                payload: serde_json::json!({"conversation_id":"conversation-turn","created_by":{"kind":"USER","principal_id":"owner-local"}}),
                recorded_at: conversation_ctx.recorded_at,
            },
            conversation,
        })
        .expect("Conversation persists");
    let connection = Connection::open(state_database(&directory)).expect("seed binding");
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    connection.execute(
        "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES('profile-turn','codex','Codex',?1)",
        [AT],
    ).unwrap();
    connection.execute(
        "INSERT INTO agent_endpoints(endpoint_id,agent_profile_id,protocol,topology,protocol_version,capabilities_json) VALUES('endpoint-turn','profile-turn','CLI','PROCESS_ADAPTER',NULL,'{}')",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,runtime_id,endpoint_selection_policy_json,auth_ref,configuration_json,enabled,lead_eligible,created_at,version) VALUES('binding-turn','workspace-turn','profile-turn',NULL,'{}',NULL,'{}',1,1,?1,1)",
        [AT],
    ).unwrap();
    connection.execute(
        "UPDATE conversations SET active_agent_binding_id='binding-turn' WHERE conversation_id='conversation-turn'",
        [],
    ).unwrap();
    drop(connection);

    let message = ConversationMessageRecord {
        message_id: "message-turn".into(),
        conversation_id: "conversation-turn".into(),
        author: serde_json::json!({"kind":"USER","principal_id":"owner-local"}),
        role: "USER".into(),
        agent_session_id: None,
        agent_binding_id: None,
        turn_id: Some("turn-one".into()),
        content: serde_json::json!([{"kind":"TEXT","text":"Please summarize this."}]),
        resource_refs: serde_json::json!([]),
        source_channel_ref: None,
        created_at: AT.into(),
    };
    let turn = ConversationTurn {
        turn_id: "turn-one".into(),
        conversation_id: "conversation-turn".into(),
        user_message_id: "message-turn".into(),
        agent_session_id: None,
        status: ConversationTurnStatus::Open,
        retry_ordinal: 0,
        presentation_preference: PresentationPreference::Auto,
        created_at: AT.into(),
        settled_at: None,
        version: 1,
    };
    let msgctx = context("event-user-message", 8);
    let turnctx = context("event-turn-created", 9);
    let commit = CreateConversationTurnCommit {
        request: WorkspaceCreateRequest {
            principal_id: "owner-local".into(),
            request_id: "t".repeat(200),
            request_payload: serde_json::json!({
                "operation":"conversation.turn.create.v1",
                "workspace_id":"workspace-turn",
                "conversation_id":"conversation-turn",
                "turn_id":"turn-one",
                "message_id":"message-turn",
                "author":{"kind":"USER","principal_id":"owner-local"},
                "content":[{"kind":"TEXT","text":"Please summarize this."}],
                "resource_refs":[],
                "presentation_preference":"AUTO"
            }),
        },
        principal_id: "owner-local".into(),
        workspace_id: "workspace-turn".into(),
        message: message.clone(),
        turn: turn.clone(),
        message_event: EventDraft {
            event_id: msgctx.event_id,
            workspace_id: "workspace-turn".into(),
            entity_type: "Conversation".into(),
            entity_id: "conversation-turn".into(),
            origin_runtime_id: msgctx.origin_runtime_id,
            entity_revision: 2,
            hlc_timestamp: msgctx.hlc_timestamp,
            correlation_id: msgctx.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.message.added.v1".into(),
            payload: serde_json::json!({
                "message_id":"message-turn","conversation_id":"conversation-turn",
                "author":message.author,"content_digest":digest(&canonical_json(&message.content).unwrap()),"resource_refs":[]
            }),
            recorded_at: msgctx.recorded_at,
        },
        turn_event: EventDraft {
            event_id: turnctx.event_id,
            workspace_id: "workspace-turn".into(),
            entity_type: "ConversationTurn".into(),
            entity_id: "turn-one".into(),
            origin_runtime_id: turnctx.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: turnctx.hlc_timestamp,
            correlation_id: turnctx.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.turn.created.v2".into(),
            payload: serde_json::json!({
                "turn_id":"turn-one","conversation_id":"conversation-turn",
                "user_message_id":"message-turn","agent_binding_id":"binding-turn",
                "aggregate_version":1,"presentation_preference":"AUTO"
            }),
            recorded_at: turnctx.recorded_at,
        },
    };
    (directory, store, commit)
}

fn seed_binding(
    directory: &tempfile::TempDir,
    binding_id: &str,
    enabled: bool,
    lead_eligible: bool,
) {
    let connection = Connection::open(state_database(directory)).expect("seed binding");
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    let profile_id = format!("{binding_id}-profile");
    connection.execute(
        "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES(?1,'codex','Codex',?2)",
        rusqlite::params![profile_id, AT],
    ).unwrap();
    connection.execute(
        "INSERT INTO agent_endpoints(endpoint_id,agent_profile_id,protocol,topology,protocol_version,capabilities_json) VALUES(?1,?2,'CLI','PROCESS_ADAPTER',NULL,'{}')",
        rusqlite::params![format!("{binding_id}-endpoint"), profile_id],
    ).unwrap();
    connection.execute(
        "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,runtime_id,endpoint_selection_policy_json,auth_ref,configuration_json,enabled,lead_eligible,created_at,version) VALUES(?1,'workspace-turn',?2,NULL,'{}',NULL,'{}',?3,?4,?5,1)",
        rusqlite::params![binding_id, profile_id, i64::from(enabled), i64::from(lead_eligible), AT],
    ).unwrap();
}

/// A Conversation without its own override, one eligible AgentBinding, and that
/// binding configured as the Workspace default.
fn workspace_default_fixture() -> (
    tempfile::TempDir,
    SqliteWorkspaceStore,
    CreateConversationTurnCommit,
) {
    let directory = tempfile::tempdir().expect("private temporary storage directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-turn");
    let conversation = ConversationRecord {
        conversation_id: "conversation-turn".into(),
        workspace_id: "workspace-turn".into(),
        title: None,
        active_agent_binding_id: None,
        version: 1,
        created_at: AT.into(),
    };
    let conversation_ctx = context("event-conversation-turn", 7);
    SqliteConversationStore::new(store.clone())
        .create_conversation(CreateConversationCommit {
            request: WorkspaceCreateRequest {
                principal_id: "owner-local".into(),
                request_id: "conversation-create-turn".into(),
                request_payload: serde_json::json!({"operation":"conversation.create.v1","workspace_id":"workspace-turn","title":null}),
            },
            event: EventDraft {
                event_id: conversation_ctx.event_id,
                workspace_id: conversation.workspace_id.clone(),
                entity_type: "Conversation".into(),
                entity_id: conversation.conversation_id.clone(),
                origin_runtime_id: conversation_ctx.origin_runtime_id,
                entity_revision: 1,
                hlc_timestamp: conversation_ctx.hlc_timestamp,
                correlation_id: conversation_ctx.correlation_id,
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.created.v1".into(),
                payload: serde_json::json!({"conversation_id":"conversation-turn","created_by":{"kind":"USER","principal_id":"owner-local"}}),
                recorded_at: conversation_ctx.recorded_at,
            },
            conversation,
        })
        .expect("Conversation persists");
    seed_binding(&directory, "binding-default", true, true);
    let connection = Connection::open(state_database(&directory)).expect("set Workspace default");
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    connection.execute(
        "UPDATE workspaces SET default_agent_binding_id='binding-default' WHERE workspace_id='workspace-turn'",
        [],
    ).unwrap();
    drop(connection);

    let message = ConversationMessageRecord {
        message_id: "message-turn".into(),
        conversation_id: "conversation-turn".into(),
        author: serde_json::json!({"kind":"USER","principal_id":"owner-local"}),
        role: "USER".into(),
        agent_session_id: None,
        agent_binding_id: None,
        turn_id: Some("turn-one".into()),
        content: serde_json::json!([{"kind":"TEXT","text":"Please summarize this."}]),
        resource_refs: serde_json::json!([]),
        source_channel_ref: None,
        created_at: AT.into(),
    };
    let turn = ConversationTurn {
        turn_id: "turn-one".into(),
        conversation_id: "conversation-turn".into(),
        user_message_id: "message-turn".into(),
        agent_session_id: None,
        status: ConversationTurnStatus::Open,
        retry_ordinal: 0,
        presentation_preference: PresentationPreference::Auto,
        created_at: AT.into(),
        settled_at: None,
        version: 1,
    };
    let msgctx = context("event-user-message", 8);
    let turnctx = context("event-turn-created", 9);
    let commit = CreateConversationTurnCommit {
        request: WorkspaceCreateRequest {
            principal_id: "owner-local".into(),
            request_id: "t".repeat(200),
            request_payload: serde_json::json!({
                "operation":"conversation.turn.create.v1",
                "workspace_id":"workspace-turn",
                "conversation_id":"conversation-turn",
                "turn_id":"turn-one",
                "message_id":"message-turn",
                "author":{"kind":"USER","principal_id":"owner-local"},
                "content":[{"kind":"TEXT","text":"Please summarize this."}],
                "resource_refs":[],
                "presentation_preference":"AUTO"
            }),
        },
        principal_id: "owner-local".into(),
        workspace_id: "workspace-turn".into(),
        message: message.clone(),
        turn: turn.clone(),
        message_event: EventDraft {
            event_id: msgctx.event_id,
            workspace_id: "workspace-turn".into(),
            entity_type: "Conversation".into(),
            entity_id: "conversation-turn".into(),
            origin_runtime_id: msgctx.origin_runtime_id,
            entity_revision: 2,
            hlc_timestamp: msgctx.hlc_timestamp,
            correlation_id: msgctx.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.message.added.v1".into(),
            payload: serde_json::json!({
                "message_id":"message-turn","conversation_id":"conversation-turn",
                "author":message.author,"content_digest":digest(&canonical_json(&message.content).unwrap()),"resource_refs":[]
            }),
            recorded_at: msgctx.recorded_at,
        },
        turn_event: EventDraft {
            event_id: turnctx.event_id,
            workspace_id: "workspace-turn".into(),
            entity_type: "ConversationTurn".into(),
            entity_id: "turn-one".into(),
            origin_runtime_id: turnctx.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: turnctx.hlc_timestamp,
            correlation_id: turnctx.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.turn.created.v2".into(),
            payload: serde_json::json!({
                "turn_id":"turn-one","conversation_id":"conversation-turn",
                "user_message_id":"message-turn","agent_binding_id":"binding-default",
                "aggregate_version":1,"presentation_preference":"AUTO"
            }),
            recorded_at: turnctx.recorded_at,
        },
    };
    (directory, store, commit)
}

#[test]
fn conversation_turn_creation_uses_the_workspace_default_when_the_conversation_has_no_override() {
    let (directory, store, commit) = workspace_default_fixture();
    let created = SqliteConversationStore::new(store)
        .create_conversation_turn(commit)
        .expect("the Workspace default admits the turn");

    let connection = Connection::open(state_database(&directory)).expect("inspect the turn");
    let (messages, turns, receipts): (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM conversation_messages),
               (SELECT COUNT(*) FROM conversation_turns),
               (SELECT COUNT(*) FROM request_dedup WHERE request_id=?1)",
            [&"t".repeat(200)],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read durable turn counts");
    assert_eq!((messages, turns, receipts), (1, 1, 1));

    // The turn records the binding it was admitted against.
    let payload: String = connection
        .query_row(
            "SELECT payload_json FROM domain_events WHERE type='conversation.turn.created.v2' AND entity_id='turn-one'",
            [],
            |row| row.get(0),
        )
        .expect("read the turn creation event");
    assert!(payload.contains("binding-default"));
    assert!(created.turn.status == ConversationTurnStatus::Open);

    // The Conversation keeps its own (absent) override. The Workspace default is a
    // preference for admission, so a later default change still reaches newly
    // admitted turns instead of being frozen by this snapshot.
    let (override_value, version): (Option<String>, i64) = connection
        .query_row(
            "SELECT active_agent_binding_id,version FROM conversations WHERE conversation_id='conversation-turn'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read the Conversation projection");
    assert_eq!(override_value, None);
    assert_eq!(version, 2);
}

#[test]
fn conversation_turn_creation_refuses_an_ineligible_conversation_override_without_falling_back() {
    let (directory, store, mut commit) = workspace_default_fixture();
    // The Conversation's own override is authoritative. It is configured but no
    // longer eligible, so the turn is refused instead of silently using the
    // lower-precedence Workspace default.
    seed_binding(&directory, "binding-override", false, true);
    let connection = Connection::open(state_database(&directory)).expect("pin the override");
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    connection.execute(
        "UPDATE conversations SET active_agent_binding_id='binding-override' WHERE conversation_id='conversation-turn'",
        [],
    ).unwrap();
    drop(connection);
    commit.turn_event.payload["agent_binding_id"] = serde_json::json!("binding-override");

    assert!(matches!(
        SqliteConversationStore::new(store).create_conversation_turn(commit),
        Err(StoreError::Invalid(reason)) if reason == "AGENT_UNAVAILABLE"
    ));

    let connection = Connection::open(state_database(&directory)).expect("inspect the refusal");
    let (messages, turns, receipts, events): (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM conversation_messages),
               (SELECT COUNT(*) FROM conversation_turns),
               (SELECT COUNT(*) FROM request_dedup WHERE request_id=?1),
               (SELECT COUNT(*) FROM domain_events WHERE entity_id='turn-one')",
            [&"t".repeat(200)],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read durable turn counts");
    assert_eq!(
        (messages, turns, receipts, events),
        (0, 0, 0, 0),
        "an ineligible selection persists no message, turn, event, or receipt"
    );
}

#[test]
fn conversation_turn_creation_refuses_a_missing_selection_without_persisting_anything() {
    let (directory, store, mut commit) = workspace_default_fixture();
    // No Workspace default and no Conversation override.
    let connection = Connection::open(state_database(&directory)).expect("clear the default");
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    connection.execute(
        "UPDATE workspaces SET default_agent_binding_id=NULL WHERE workspace_id='workspace-turn'",
        [],
    ).unwrap();
    drop(connection);
    commit.turn_event.payload["agent_binding_id"] = serde_json::json!("binding-default");

    assert!(matches!(
        SqliteConversationStore::new(store).create_conversation_turn(commit),
        Err(StoreError::Invalid(reason)) if reason == "AGENT_UNAVAILABLE"
    ));

    let connection = Connection::open(state_database(&directory)).expect("inspect the refusal");
    let (messages, turns, receipts): (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM conversation_messages),
               (SELECT COUNT(*) FROM conversation_turns),
               (SELECT COUNT(*) FROM request_dedup WHERE request_id=?1)",
            [&"t".repeat(200)],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read durable turn counts");
    assert_eq!((messages, turns, receipts), (0, 0, 0));
}

#[test]
fn conversation_turn_creation_replays_atomically_and_survives_reopen() {
    let (directory, store, commit) = fixture();
    let adapter = SqliteConversationStore::new(store.clone());
    let created = adapter
        .create_conversation_turn(commit.clone())
        .expect("message and turn commit");
    let replay = adapter
        .create_conversation_turn(commit)
        .expect("same idempotency request replays");
    assert_eq!(replay, created);
    assert_eq!(created.turn.status, ConversationTurnStatus::Open);
    assert_eq!(created.turn.agent_session_id, None);
    assert_eq!(created.message.turn_id.as_deref(), Some("turn-one"));
    assert_eq!(created.events.len(), 2);

    // A plain Conversation remains a distinct domain flow. In particular, accepting
    // its durable user message must not implicitly manufacture Task execution state.
    let connection = Connection::open(state_database(&directory)).expect("inspect chat state");
    let (tasks, attempts): (i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM tasks WHERE workspace_id='workspace-turn'),
               (SELECT COUNT(*) FROM attempts a JOIN tasks t ON t.task_id=a.task_id
                WHERE t.workspace_id='workspace-turn')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read Task and Attempt counts");
    assert_eq!((tasks, attempts), (0, 0));

    drop(adapter);
    drop(store);
    let reopened = test_store(&directory, Duration::from_secs(1));
    let connection = Connection::open(state_database(&directory)).expect("read persisted DB");
    let persisted: (String, String, i64) = connection
        .query_row(
            "SELECT t.status,t.turn_id,(SELECT COUNT(*) FROM conversation_messages m WHERE m.message_id=t.user_message_id) FROM conversation_turns t WHERE t.turn_id='turn-one'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("persisted turn and message");
    assert_eq!(persisted, ("OPEN".into(), "turn-one".into(), 1));
    assert_eq!(
        reopened
            .read_workspace_events("workspace-turn")
            .unwrap()
            .iter()
            .filter(|event| event.event_type.starts_with("conversation.turn.created"))
            .count(),
        1
    );
}

#[test]
fn conversation_turn_creation_rolls_back_message_when_event_insert_fails() {
    let (directory, store, mut commit) = fixture();
    // Duplicate the already persisted Conversation event id. The failure occurs after
    // the message, turn, Conversation version, and first event have been written in-tx.
    commit.turn_event.event_id = "event-conversation-turn".into();
    assert!(
        SqliteConversationStore::new(store.clone())
            .create_conversation_turn(commit)
            .is_err()
    );
    let connection = Connection::open(state_database(&directory)).expect("read DB");
    let messages: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_messages WHERE message_id='message-turn'",
            [],
            |row| row.get(0),
        )
        .expect("message count");
    let turns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_turns WHERE turn_id='turn-one'",
            [],
            |row| row.get(0),
        )
        .expect("turn count");
    let (conversation_version, turn_events): (i64, i64) = connection.query_row(
        "SELECT c.version,(SELECT COUNT(*) FROM domain_events WHERE entity_id='turn-one') FROM conversations c WHERE c.conversation_id='conversation-turn'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).expect("Conversation projection and event count");
    assert_eq!(
        (messages, turns, conversation_version, turn_events),
        (0, 0, 1, 0)
    );
}

#[test]
fn conversation_turn_creation_enforces_workspace_owner_fence() {
    let (_directory, store, mut commit) = fixture();
    commit.request.principal_id = "other-principal".into();
    commit.principal_id = "other-principal".into();
    commit.message.author = serde_json::json!({
        "kind":"USER","principal_id":"other-principal"
    });
    commit.message_event.payload["author"] = commit.message.author.clone();
    commit.request.request_payload["author"] = commit.message.author.clone();
    assert!(matches!(
        SqliteConversationStore::new(store).create_conversation_turn(commit),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn turn_transition_uses_version_cas_and_preserves_session_identity() {
    let (_directory, store, commit) = fixture();
    let adapter = SqliteConversationStore::new(store.clone());
    adapter
        .create_conversation_turn(commit)
        .expect("turn created");
    let start_context = context("event-turn-start-rejected", 10);
    let start_event = EventDraft {
        event_id: start_context.event_id,
        workspace_id: "workspace-turn".into(),
        entity_type: "ConversationTurn".into(),
        entity_id: "turn-one".into(),
        origin_runtime_id: start_context.origin_runtime_id,
        entity_revision: 2,
        hlc_timestamp: start_context.hlc_timestamp,
        correlation_id: start_context.correlation_id,
        causation_id: None,
        schema_version: 1,
        event_type: "conversation.turn.status.changed.v1".into(),
        payload: serde_json::json!({
            "turn_id":"turn-one","from":"OPEN","to":"RUNNING",
            "reason_code":"SESSION_READY","aggregate_version":2
        }),
        recorded_at: start_context.recorded_at,
    };
    assert!(matches!(
        adapter.transition_conversation_turn(
            "owner-local",
            "workspace-turn",
            "conversation-turn",
            "turn-one",
            1,
            TurnCommand::Start {
                agent_session_id: "unadmitted-session".into()
            },
            start_event,
        ),
        Err(StoreError::Invalid(message)) if message == "CONVERSATION_SESSION_ADMISSION_UNAVAILABLE"
    ));
    let ctx = context("event-turn-failed", 10);
    let event = EventDraft {
        event_id: ctx.event_id,
        workspace_id: "workspace-turn".into(),
        entity_type: "ConversationTurn".into(),
        entity_id: "turn-one".into(),
        origin_runtime_id: ctx.origin_runtime_id,
        entity_revision: 2,
        hlc_timestamp: ctx.hlc_timestamp,
        correlation_id: ctx.correlation_id,
        causation_id: None,
        schema_version: 1,
        event_type: "conversation.turn.settled.v1".into(),
        payload: serde_json::json!({"turn_id":"turn-one","from":"OPEN","to":"FAILED","reason_code":"TEST_FAILURE","aggregate_version":2}),
        recorded_at: ctx.recorded_at,
    };
    let failed = adapter
        .transition_conversation_turn(
            "owner-local",
            "workspace-turn",
            "conversation-turn",
            "turn-one",
            1,
            TurnCommand::Fail {
                settled_at: AT.into(),
            },
            event.clone(),
        )
        .expect("transition commits");
    assert_eq!(failed.status, ConversationTurnStatus::Failed);
    assert_eq!(failed.agent_session_id, None);
    assert_eq!(failed.version, 2);
    assert!(matches!(
        adapter.transition_conversation_turn(
            "owner-local",
            "workspace-turn",
            "conversation-turn",
            "turn-one",
            1,
            TurnCommand::Fail {
                settled_at: AT.into()
            },
            event,
        ),
        Err(StoreError::Conflict { .. })
    ));
    let persisted: (String, Option<String>, i64, i64) = Connection::open(state_database(&_directory))
        .expect("open database")
        .query_row(
            "SELECT status,agent_session_id,version,(SELECT COUNT(*) FROM domain_events WHERE entity_id='turn-one') FROM conversation_turns WHERE turn_id='turn-one'",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
        ).expect("read current state");
    assert_eq!(persisted, ("FAILED".into(), None, 2, 2));
}

#[test]
fn unavailable_admission_settles_the_durable_turn_and_exact_replay_writes_nothing() {
    use storage_core::conversation::CONVERSATION_SESSION_ADMISSION_UNAVAILABLE;

    let (directory, store, commit) = fixture();
    let adapter = SqliteConversationStore::new(store);
    let created = adapter
        .create_conversation_turn(commit.clone())
        .expect("message and turn commit");

    let start_context = context("event-turn-admission-unavailable", 10);
    let start_event = EventDraft {
        event_id: start_context.event_id,
        workspace_id: "workspace-turn".into(),
        entity_type: "ConversationTurn".into(),
        entity_id: "turn-one".into(),
        origin_runtime_id: start_context.origin_runtime_id,
        entity_revision: 2,
        hlc_timestamp: start_context.hlc_timestamp,
        correlation_id: start_context.correlation_id,
        causation_id: None,
        schema_version: 1,
        event_type: "conversation.turn.status.changed.v1".into(),
        payload: serde_json::json!({
            "turn_id":"turn-one","from":"OPEN","to":"RUNNING",
            "reason_code":"SESSION_READY","aggregate_version":2
        }),
        recorded_at: start_context.recorded_at,
    };
    assert!(matches!(
        adapter.transition_conversation_turn(
            "owner-local",
            "workspace-turn",
            "conversation-turn",
            "turn-one",
            created.turn.version,
            TurnCommand::Start {
                agent_session_id: "candidate-conversation-session".into()
            },
            start_event,
        ),
        Err(StoreError::Invalid(reason)) if reason == CONVERSATION_SESSION_ADMISSION_UNAVAILABLE
    ));

    let settled_context = context("event-turn-settled-unavailable", 11);
    let settled_event = EventDraft {
        event_id: settled_context.event_id,
        workspace_id: "workspace-turn".into(),
        entity_type: "ConversationTurn".into(),
        entity_id: "turn-one".into(),
        origin_runtime_id: settled_context.origin_runtime_id,
        entity_revision: 2,
        hlc_timestamp: settled_context.hlc_timestamp,
        correlation_id: settled_context.correlation_id,
        causation_id: None,
        schema_version: 1,
        event_type: "conversation.turn.settled.v1".into(),
        payload: serde_json::json!({
            "turn_id":"turn-one","from":"OPEN","to":"FAILED",
            "reason_code":CONVERSATION_SESSION_ADMISSION_UNAVAILABLE,
            "aggregate_version":2
        }),
        recorded_at: settled_context.recorded_at,
    };
    let settled = adapter
        .transition_conversation_turn(
            "owner-local",
            "workspace-turn",
            "conversation-turn",
            "turn-one",
            created.turn.version,
            TurnCommand::Fail {
                settled_at: AT.into(),
            },
            settled_event,
        )
        .expect("the unavailable-admission settlement commits");
    assert_eq!(settled.status, ConversationTurnStatus::Failed);
    assert_eq!(settled.agent_session_id, None);
    assert_eq!(settled.version, 2);

    // The receipt stays the original OPEN snapshot; replay resolves it before any
    // aggregate blob, message, turn, event, or receipt write.
    let replay = adapter
        .create_conversation_turn(commit)
        .expect("an exact replay resolves the committed receipt");
    assert_eq!(replay, created);
    assert_eq!(replay.turn.status, ConversationTurnStatus::Open);

    let connection = Connection::open(state_database(&directory)).expect("inspect final state");
    let (status, version, settled_at, messages, created_events, settled_events, receipts): (
        String,
        i64,
        Option<String>,
        i64,
        i64,
        i64,
        i64,
    ) = connection
        .query_row(
            "SELECT t.status,t.version,t.settled_at,
               (SELECT COUNT(*) FROM conversation_messages m WHERE m.turn_id=t.turn_id),
               (SELECT COUNT(*) FROM domain_events e WHERE e.entity_id=t.turn_id AND e.type='conversation.turn.created.v2'),
               (SELECT COUNT(*) FROM domain_events e WHERE e.entity_id=t.turn_id AND e.type='conversation.turn.settled.v1'),
               (SELECT COUNT(*) FROM request_dedup WHERE principal_id='owner-local' AND request_id=?1)
             FROM conversation_turns t WHERE t.turn_id='turn-one'",
            [&"t".repeat(200)],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .expect("read the settled turn and its receipts");
    assert_eq!(status, "FAILED");
    assert_eq!(version, 2);
    assert!(settled_at.is_some());
    assert_eq!(
        (messages, created_events, settled_events, receipts),
        (1, 1, 1, 1),
        "settlement and replay add no duplicate message, event, or receipt"
    );
    let conversation_version: i64 = connection
        .query_row(
            "SELECT version FROM conversations WHERE conversation_id='conversation-turn'",
            [],
            |row| row.get(0),
        )
        .expect("read Conversation version");
    assert_eq!(conversation_version, 2);
    let (tasks, attempts, sessions, leases, environments): (i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM tasks WHERE workspace_id='workspace-turn'),
               (SELECT COUNT(*) FROM attempts a JOIN tasks t ON t.task_id=a.task_id WHERE t.workspace_id='workspace-turn'),
               (SELECT COUNT(*) FROM agent_sessions WHERE workspace_id='workspace-turn'),
               (SELECT COUNT(*) FROM execution_leases l JOIN tasks t ON t.task_id=l.task_id WHERE t.workspace_id='workspace-turn'),
               (SELECT COUNT(*) FROM environments WHERE owner_workspace_id='workspace-turn')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .expect("read execution-state counts");
    assert_eq!(
        (tasks, attempts, sessions, leases, environments),
        (0, 0, 0, 0, 0),
        "a durable ordinary Conversation turn creates no Task, Attempt, session, lease, or Environment"
    );
}


#[test]
fn conversation_turn_creation_serializes_appends_from_the_same_stale_snapshot() {
    let (directory, store, first) = fixture();
    let adapter = SqliteConversationStore::new(store);
    adapter
        .create_conversation_turn(first.clone())
        .expect("first turn commits from Conversation revision 1");

    // A second request was prepared from the same revision-1 snapshot before the first
    // append committed. Its event therefore also carries revision 2; the serialized
    // storage transaction must assign the actual next Conversation revision.
    let mut second = first;
    second.request.request_id = "second-conversation-turn-request".into();
    second.request.request_payload["turn_id"] = serde_json::json!("turn-two");
    second.request.request_payload["message_id"] = serde_json::json!("message-two");
    second.message.message_id = "message-two".into();
    second.message.turn_id = Some("turn-two".into());
    second.turn.turn_id = "turn-two".into();
    second.turn.user_message_id = "message-two".into();

    let message_context = context("event-user-message-two", 10);
    second.message_event.event_id = message_context.event_id;
    second.message_event.origin_runtime_id = message_context.origin_runtime_id;
    second.message_event.hlc_timestamp = message_context.hlc_timestamp;
    second.message_event.correlation_id = message_context.correlation_id;
    second.message_event.recorded_at = message_context.recorded_at;
    second.message_event.payload["message_id"] = serde_json::json!("message-two");

    let turn_context = context("event-turn-created-two", 11);
    second.turn_event.event_id = turn_context.event_id;
    second.turn_event.entity_id = "turn-two".into();
    second.turn_event.origin_runtime_id = turn_context.origin_runtime_id;
    second.turn_event.hlc_timestamp = turn_context.hlc_timestamp;
    second.turn_event.correlation_id = turn_context.correlation_id;
    second.turn_event.recorded_at = turn_context.recorded_at;
    second.turn_event.payload["turn_id"] = serde_json::json!("turn-two");
    second.turn_event.payload["user_message_id"] = serde_json::json!("message-two");

    assert_eq!(second.message_event.entity_revision, 2);
    adapter
        .create_conversation_turn(second)
        .expect("a concurrent append must be serialized at the current revision");

    let connection = Connection::open(state_database(&directory)).expect("inspect serialized turns");
    let (conversation_version, message_revision, messages, turns): (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT version FROM conversations WHERE conversation_id='conversation-turn'),
               (SELECT entity_revision FROM domain_events WHERE event_id='event-user-message-two'),
               (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id='conversation-turn'),
               (SELECT COUNT(*) FROM conversation_turns WHERE conversation_id='conversation-turn')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read serialized Conversation and event revisions");
    assert_eq!((conversation_version, message_revision, messages, turns), (3, 3, 2, 2));
}


fn insert_context_message_event(
    connection: &Connection,
    event_id: &str,
    message_id: &str,
    revision: i64,
    origin_sequence: i64,
) {
    connection
        .execute(
            "INSERT INTO domain_events(event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,causation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest) SELECT ?1,workspace_id,entity_type,entity_id,origin_runtime_id,?2,?3,hlc_timestamp,correlation_id,NULL,schema_version,type,json_set(payload_json,'$.message_id',?4),aggregate_state_ref_json,recorded_at,payload_digest FROM domain_events WHERE event_id='event-user-message'",
            params![event_id, origin_sequence, revision, message_id],
        )
        .expect("insert an append-revision fixture for a Conversation message");
}

#[test]
fn switched_agent_context_is_a_turn_pinned_prefix_of_the_same_conversation() {
    let (directory, store, commit) = fixture();
    let turn_id = commit.turn.turn_id.clone();
    let connection = Connection::open(state_database(&directory)).expect("open conversation DB");
    connection.execute(
        "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,agent_session_id,agent_binding_id,turn_id,content_json,resource_refs_json,source_channel_ref_json,created_at) VALUES('message-old-user','conversation-turn','{}','USER',NULL,NULL,'turn-old','[{\"kind\":\"TEXT\",\"text\":\"Keep this request\"}]','[]',NULL,'2026-10-09T11:59:58Z')",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,agent_session_id,status,retry_ordinal,created_at,settled_at,version,presentation_preference) VALUES('turn-old','conversation-turn','message-old-user',NULL,'COMPLETED',0,'2026-10-09T11:59:58Z','2026-10-09T11:59:59Z',2,'AUTO')",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,agent_session_id,agent_binding_id,turn_id,content_json,resource_refs_json,source_channel_ref_json,created_at) VALUES('message-old-agent','conversation-turn','{}','AGENT','private-session-a','binding-a','turn-old','[{\"kind\":\"TEXT\",\"text\":\"The first agent found an answer\"}]','[]',NULL,'2026-10-09T11:59:59Z')",
        [],
    ).unwrap();
    drop(connection);

    SqliteConversationStore::new(store.clone())
        .create_conversation_turn(commit)
        .expect("persist the turn for the newly selected agent");
    let connection = Connection::open(state_database(&directory)).expect("reopen conversation DB");
    connection.execute(
        "UPDATE domain_events SET entity_revision=4 WHERE event_id='event-user-message'",
        [],
    ).unwrap();
    insert_context_message_event(&connection, "event-old-user", "message-old-user", 2, 900_001);
    insert_context_message_event(&connection, "event-old-agent", "message-old-agent", 3, 900_002);
    connection.execute(
        "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,agent_session_id,agent_binding_id,turn_id,content_json,resource_refs_json,source_channel_ref_json,created_at) VALUES('aaa-later-message','conversation-turn','{}','AGENT','private-session-b','binding-turn','turn-later','[{\"kind\":\"TEXT\",\"text\":\"Future answer must not leak backwards\"}]','[]',NULL,'2026-10-09T12:00:00Z')",
        [],
    ).unwrap();
    insert_context_message_event(&connection, "event-later-agent", "aaa-later-message", 5, 900_003);

    let context = SqliteConversationStore::new(store)
        .load_conversation_context_for_turn(
            "owner-local",
            "workspace-turn",
            "conversation-turn",
            &turn_id,
        )
        .expect("load the pinned context for the current turn");

    assert_eq!(context.conversation_id, "conversation-turn");
    assert_eq!(context.target_agent_binding_id, "binding-turn");
    assert_eq!(context.through_message_id, "message-turn");
    assert_eq!(
        context.messages.iter().map(|message| message.message_id.as_str()).collect::<Vec<_>>(),
        ["message-old-user", "message-old-agent", "message-turn"]
    );
    let serialized = serde_json::to_string(&context).expect("serialize context package");
    assert!(!serialized.contains("private-session-a"));
    assert!(!serialized.contains("private-session-b"));
}
