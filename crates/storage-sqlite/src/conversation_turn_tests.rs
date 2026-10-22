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

/// Seeds a Runtime that is ONLINE, READY and EXECUTOR-entitled for this Workspace,
/// bound to the fixture endpoint, so the admission readiness check can pass.
fn seed_ready_runtime(directory: &tempfile::TempDir, expires_at: Option<&str>) {
    let mut connection = Connection::open(state_database(directory)).expect("seed runtime");
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    // The Runtime row first: `runtimes.current_incarnation_id` is a DEFERRABLE
    // INITIALLY DEFERRED foreign key, while `runtime_incarnations.runtime_id` is
    // immediate. A deferred foreign key only defers to COMMIT inside a transaction,
    // so all four rows are written in one.
    let tx = connection.transaction().unwrap();
    tx
        .execute(
            "INSERT INTO runtimes(runtime_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,resource_capacity_json,last_seen,version) VALUES('runtime-turn','{}','0.1.0','linux','x86_64','[]','LOCAL','ONLINE','MANUAL','inc-turn','{}',?1,1)",
            [AT],
        )
        .unwrap();
    tx
        .execute(
            "INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state,ready_at,stopped_at,version) VALUES('inc-turn','runtime-turn',?1,'0.1.0',0,'READY',?1,NULL,1)",
            [AT],
        )
        .unwrap();
    tx
        .execute(
            "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,revoked_at,version) VALUES('rwb-turn','runtime-turn','workspace-turn','LOCAL_ENROLLMENT','ACTIVE','[\"EXECUTOR\"]',?1,?1,NULL,1)",
            [AT],
        )
        .unwrap();
    // A binding that declares no endpoint-selection mode fails the readiness check
    // closed, because `json_extract(...,'$.mode') <> 'PINNED_ENDPOINT'` is NULL for
    // `{}`. The fixture binding is therefore given an explicit compatible mode.
    tx.execute(
        "UPDATE agent_bindings SET endpoint_selection_policy_json=json_object('mode','AUTO_COMPATIBLE') WHERE agent_binding_id='binding-turn'",
        [],
    )
    .unwrap();
    tx
        .execute(
            "INSERT INTO agent_endpoint_bindings(endpoint_id,runtime_id,runtime_incarnation_id,endpoint_ref,observed_at,expires_at) VALUES('endpoint-turn','runtime-turn','inc-turn','local://codex',?1,?2)",
            params![AT, expires_at],
        )
        .unwrap();
    tx.commit().unwrap();
    drop(connection);
}

/// Builds the Conversation-scoped session admission for the fixture turn.
fn admission_commit(
    session_id: &str,
    binding_id: &str,
    scope_kind: &str,
    with_task: bool,
) -> storage_core::conversation::AdmitConversationTurnCommit {
    let session_ctx = context("event-session-starting", 11);
    let turn_ctx = context("event-turn-started", 12);
    storage_core::conversation::AdmitConversationTurnCommit {
        request: WorkspaceCreateRequest {
            principal_id: "owner-local".into(),
            request_id: format!("admit-{session_id}"),
            request_payload: serde_json::json!({
                "operation":"conversation.turn.admit.v1",
                "workspace_id":"workspace-turn",
                "conversation_id":"conversation-turn",
                "turn_id":"turn-one",
                "agent_session_id":session_id
            }),
        },
        principal_id: "owner-local".into(),
        workspace_id: "workspace-turn".into(),
        conversation_id: "conversation-turn".into(),
        session: storage_core::AgentSessionRecord {
            agent_session_id: session_id.into(),
            workspace_id: "workspace-turn".into(),
            scope_kind: scope_kind.into(),
            conversation_id: Some("conversation-turn".into()),
            conversation_turn_id: Some("turn-one".into()),
            task_id: with_task.then(|| "task-foreign".into()),
            task_spec_revision: None,
            attempt_id: None,
            agent_binding_id: binding_id.into(),
            endpoint_id: "endpoint-turn".into(),
            runtime_id: "runtime-turn".into(),
            runtime_incarnation_id: "inc-turn".into(),
            configuration_digest: None,
            harness_descriptor_digest: None,
            status: "STARTING".into(),
            started_at: AT.into(),
            last_event_at: None,
            closed_at: None,
            version: 1,
        },
        turn_id: "turn-one".into(),
        expected_turn_version: 1,
        session_event: EventDraft {
            event_id: session_ctx.event_id,
            workspace_id: "workspace-turn".into(),
            entity_type: "AgentSession".into(),
            entity_id: session_id.into(),
            origin_runtime_id: session_ctx.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: session_ctx.hlc_timestamp,
            correlation_id: session_ctx.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "agent.session.starting.v1".into(),
            payload: serde_json::json!({
                "agent_session_id":session_id,"scope":"CONVERSATION",
                "agent_binding_id":binding_id,"endpoint_id":"endpoint-turn",
                "runtime_id":"runtime-turn","runtime_incarnation_id":"inc-turn",
                "session_state":"STARTING","reported_at":AT
            }),
            recorded_at: session_ctx.recorded_at,
        },
        turn_event: EventDraft {
            event_id: turn_ctx.event_id,
            workspace_id: "workspace-turn".into(),
            entity_type: "ConversationTurn".into(),
            entity_id: "turn-one".into(),
            origin_runtime_id: turn_ctx.origin_runtime_id,
            entity_revision: 2,
            hlc_timestamp: turn_ctx.hlc_timestamp,
            correlation_id: turn_ctx.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.turn.status.changed.v1".into(),
            payload: serde_json::json!({
                "turn_id":"turn-one","from":"OPEN","to":"RUNNING",
                "agent_session_id":session_id,"aggregate_version":2
            }),
            recorded_at: turn_ctx.recorded_at,
        },
    }
}

#[test]
fn a_conversation_scoped_session_admits_the_turn_and_starts_it() {
    let (directory, store, commit) = fixture();
    let conversation_store = SqliteConversationStore::new(store.clone());
    conversation_store
        .create_conversation_turn(commit)
        .expect("the user message and OPEN turn persist");
    seed_ready_runtime(&directory, None);

    let admitted = conversation_store
        .admit_conversation_turn(admission_commit("session-one", "binding-turn", "CONVERSATION", false))
        .expect("a ready Runtime admits the Conversation session");

    assert_eq!(admitted.turn.status, ConversationTurnStatus::Running);
    assert_eq!(
        admitted.turn.agent_session_id.as_deref(),
        Some("session-one"),
        "the RUNNING turn is bound to the admitted session"
    );
    assert_eq!(admitted.session.status, "STARTING");
    assert_eq!(admitted.events.len(), 2, "session event and turn event");

    let connection = Connection::open(state_database(&directory)).unwrap();
    let (scope, status, turn_ref): (String, String, Option<String>) = connection
        .query_row(
            "SELECT scope_kind,status,conversation_turn_id FROM agent_sessions WHERE agent_session_id='session-one'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(scope, "CONVERSATION");
    assert_eq!(status, "STARTING");
    assert_eq!(turn_ref.as_deref(), Some("turn-one"));

    let (turn_status, bound): (String, Option<String>) = connection
        .query_row(
            "SELECT status,agent_session_id FROM conversation_turns WHERE turn_id='turn-one'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(turn_status, "RUNNING");
    assert_eq!(bound.as_deref(), Some("session-one"));

    // conversation.turn.created.v2 (from turn creation) + agent.session.starting.v1 +
    // conversation.turn.status.changed.v1 — admission adds exactly the last two.
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_id IN ('session-one','turn-one')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(events, 3);
    let types: Vec<String> = connection
        .prepare("SELECT type FROM domain_events WHERE entity_id IN ('session-one','turn-one') ORDER BY type")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        types,
        vec![
            "agent.session.starting.v1",
            "conversation.turn.created.v2",
            "conversation.turn.status.changed.v1"
        ]
    );

    // A replay with the same request id returns the committed result and writes nothing.
    let replay = conversation_store
        .admit_conversation_turn(admission_commit("session-one", "binding-turn", "CONVERSATION", false))
        .expect("the replayed admission returns the committed result");
    assert_eq!(replay.turn.agent_session_id.as_deref(), Some("session-one"));
    let sessions: i64 = connection
        .query_row("SELECT COUNT(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sessions, 1, "a replay never admits a second session");
}

#[test]
fn admission_refuses_a_session_that_is_not_scoped_to_this_conversation_turn() {
    let (directory, store, commit) = fixture();
    let conversation_store = SqliteConversationStore::new(store.clone());
    conversation_store
        .create_conversation_turn(commit)
        .expect("the user message and OPEN turn persist");
    seed_ready_runtime(&directory, None);

    // A Task-scoped session may never start a Conversation turn.
    let error = conversation_store
        .admit_conversation_turn(admission_commit("session-task", "binding-turn", "TASK_PLANNING", false))
        .expect_err("a planning-scoped session is refused");
    assert!(matches!(error, StoreError::Invalid(_)), "{error:?}");

    // Nor may a session that also claims a Task.
    let error = conversation_store
        .admit_conversation_turn(admission_commit("session-tasked", "binding-turn", "CONVERSATION", true))
        .expect_err("a session carrying a Task is refused");
    assert!(matches!(error, StoreError::Invalid(_)), "{error:?}");

    let connection = Connection::open(state_database(&directory)).unwrap();
    let sessions: i64 = connection
        .query_row("SELECT COUNT(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sessions, 0, "a refused admission writes no session row");
}

#[test]
fn admission_refuses_an_unready_runtime_and_a_foreign_binding() {
    let (directory, store, commit) = fixture();
    let conversation_store = SqliteConversationStore::new(store.clone());
    conversation_store
        .create_conversation_turn(commit)
        .expect("the user message and OPEN turn persists");

    // No Runtime readiness rows at all: the endpoint has no live binding.
    let error = conversation_store
        .admit_conversation_turn(admission_commit("session-unready", "binding-turn", "CONVERSATION", false))
        .expect_err("an unready Runtime is refused before any write");
    assert!(matches!(error, StoreError::Invalid(_)), "{error:?}");

    // A ready Runtime whose endpoint binding already expired is still refused.
    seed_ready_runtime(&directory, Some("2000-01-01T00:00:00Z"));
    let error = conversation_store
        .admit_conversation_turn(admission_commit("session-expired", "binding-turn", "CONVERSATION", false))
        .expect_err("an expired endpoint binding is refused");
    assert!(matches!(error, StoreError::Invalid(_)), "{error:?}");

    // A binding that is not the one this Conversation resolved is never substituted.
    let error = conversation_store
        .admit_conversation_turn(admission_commit("session-foreign", "binding-other", "CONVERSATION", false))
        .expect_err("a foreign binding is refused");
    assert!(matches!(error, StoreError::Invalid(_)), "{error:?}");

    let connection = Connection::open(state_database(&directory)).unwrap();
    let sessions: i64 = connection
        .query_row("SELECT COUNT(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sessions, 0);

    // Once the binding is live again the same shape of admission succeeds, which
    // proves the expiry alone caused the refusal above.
    connection
        .execute(
            "UPDATE agent_endpoint_bindings SET expires_at=NULL WHERE endpoint_id='endpoint-turn'",
            [],
        )
        .unwrap();
    conversation_store
        .admit_conversation_turn(admission_commit("session-live", "binding-turn", "CONVERSATION", false))
        .expect("a live endpoint binding admits the session");
}
