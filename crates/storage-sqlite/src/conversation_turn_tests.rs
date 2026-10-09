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
            request_id: "turn-submit-1".into(),
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
