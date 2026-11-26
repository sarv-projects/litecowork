use super::*;
use storage_core::rich_presentation::*;
const AT: &str = "2026-10-09T12:00:00.000000000Z";
fn fixture() -> (
    tempfile::TempDir,
    SqliteWorkspaceStore,
    PublishRichPresentation,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = test_store(&dir, Duration::from_millis(100));
    create_workspace(&store, "workspace-rich");
    let conn = Connection::open(state_database(&dir)).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    conn.execute("INSERT INTO conversations(conversation_id,workspace_id,created_at) VALUES('conversation-rich','workspace-rich',?1)", [AT]).unwrap();
    conn.execute("INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('message-rich','conversation-rich','{}','AGENT','turn-rich','[{\"kind\":\"TEXT\",\"text\":\"Hello 🌍\"},{\"kind\":\"TEXT\",\"text\":\"## Summary\"},{\"kind\":\"TEXT\",\"text\":\"| Metric | Value |\"},{\"kind\":\"TEXT\",\"text\":\"| --- | --- |\"},{\"kind\":\"TEXT\",\"text\":\"| Cost | 12 |\"}]',?1)", [AT]).unwrap();
    // The semantic text is every TEXT block joined by newlines, so the fixture answer
    // carries one line per V1 block kind: a heading and a Markdown table a presentation
    // is allowed to repeat, and never anything it invents.
    let semantic = json!({"content":[{"kind":"TEXT","text":"Hello 🌍"},{"kind":"TEXT","text":"## Summary"},{"kind":"TEXT","text":"| Metric | Value |"},{"kind":"TEXT","text":"| --- | --- |"},{"kind":"TEXT","text":"| Cost | 12 |"}],"resource_refs":[]});
    let semantic_digest = digest(&canonical_json(&semantic).unwrap());
    let doc = json!({"schema_version":1,"renderer_contract_version":1,"presentation_id":"presentation-rich","message_id":"message-rich","semantic_content_digest":semantic_digest,
      "root_blocks":[{"kind":"TEXT_SLICE","source":{"start_utf8_byte":0,"end_utf8_byte_exclusive":10,"slice_digest":digest("Hello 🌍".as_bytes())}}],
      "block_provenance":[{"block_path":"/0","origin":"SEMANTIC_MESSAGE","resource_refs":[],"artifact_refs":[],"evidence_refs":[],"verification_refs":[]}]});
    let bytes = canonical_json(&doc).unwrap();
    let record = RichPresentationRecord {
        presentation_id: "presentation-rich".into(),
        workspace_id: "workspace-rich".into(),
        conversation_id: "conversation-rich".into(),
        message_id: "message-rich".into(),
        schema_version: 1,
        renderer_contract_version: 1,
        semantic_content_digest: semantic_digest,
        document_ref: BlobRef {
            digest: digest(&bytes),
            size_bytes: bytes.len() as u64,
            media_type: RICH_PRESENTATION_MEDIA_TYPE.into(),
        },
        document_digest: digest(&bytes),
        document_size_bytes: bytes.len() as u64,
        producer_agent_session_id: None,
        host_instruction_digest: None,
        host_skill_refs: vec![],
        created_at: AT.into(),
        version: 1,
    };
    let event = EventDraft {
        event_id: "event-rich".into(),
        workspace_id: record.workspace_id.clone(),
        entity_type: "RichPresentation".into(),
        entity_id: record.presentation_id.clone(),
        origin_runtime_id: "runtime-local".into(),
        entity_revision: 1,
        hlc_timestamp: AT.into(),
        correlation_id: "correlation-rich".into(),
        causation_id: None,
        schema_version: 1,
        event_type: "rich.presentation.published.v1".into(),
        payload: json!({"presentation_id":record.presentation_id,"conversation_id":record.conversation_id,"message_id":record.message_id,"schema_version":1,"renderer_contract_version":1,"semantic_content_digest":record.semantic_content_digest,"document_digest":record.document_digest,"document_size_bytes":record.document_size_bytes,"host_skill_refs":record.host_skill_refs,"aggregate_version":1}),
        recorded_at: AT.into(),
    };
    (
        dir,
        store,
        PublishRichPresentation {
            principal_id: "owner-local".into(),
            presentation: record,
            canonical_document: bytes,
            event,
        },
    )
}
#[test]
fn rich_publication_roundtrips_independently_of_semantic_message() {
    let (_dir, store, request) = fixture();
    let adapter = SqliteRichPresentationStore::new(store.clone());
    let committed = adapter.publish_rich_presentation(request.clone()).unwrap();
    assert_eq!(committed.presentation, request.presentation);
    let read = adapter
        .read_rich_presentation("owner-local", "workspace-rich", "presentation-rich")
        .unwrap()
        .unwrap();
    assert_eq!(read.canonical_document, request.canonical_document);
    assert_eq!(
        store
            .read_workspace_events("workspace-rich")
            .unwrap()
            .last()
            .unwrap()
            .event_type,
        "rich.presentation.published.v1"
    );
}

#[test]
fn conversation_presentation_snapshot_is_bounded_paged_and_keeps_semantic_truth() {
    let (dir, store, request) = fixture();
    let adapter = adapter(&store);
    adapter.publish_rich_presentation(request).unwrap();
    let c = Connection::open(state_database(&dir)).unwrap();
    c.execute(
        "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,content_json,resource_refs_json,created_at) VALUES('message-z','conversation-rich','{}','USER','[]','[]','2026-10-09T12:00:01.000000000Z')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,status,retry_ordinal,created_at,version,presentation_preference) VALUES('turn-active','conversation-rich','message-z','RUNNING',0,'2026-10-09T12:00:01.000000000Z',3,'RICH')",
        [],
    )
    .unwrap();
    drop(c);

    let first = adapter
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            None,
            1,
        )
        .unwrap();
    let durable_head = store
        .read_workspace_events("workspace-rich")
        .unwrap()
        .last()
        .unwrap()
        .origin_sequence;
    assert_eq!(first.origin_sequence, durable_head.to_string());
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0]["message"]["message_id"], "message-rich");
    assert_eq!(
        first.items[0]["rich_presentation"]["presentation_id"],
        "presentation-rich"
    );
    assert_eq!(first.items[0]["linked_items"], json!([]));
    assert_eq!(first.next_cursor.as_deref(), Some("message-rich"));
    assert!(first.projection_revision >= 5);
    assert!(first.stream_cursor.contains("conversation-rich"));
    assert_eq!(
        first.active_turn.as_ref().unwrap()["turn_id"],
        "turn-active"
    );
    assert_eq!(
        first.active_turn.as_ref().unwrap()["presentation_preference"],
        "RICH"
    );
    assert_eq!(first.active_turn.as_ref().unwrap()["version"], 3);
    assert_eq!(
        serde_json::to_value(&first).unwrap()["activity_turn"],
        json!({"turn_id":"turn-active","retry_ordinal":0,"status":"RUNNING"}),
        "activity lookup identity is captured with the same snapshot even while active"
    );
    assert!(
        first.retryable_turn.is_none(),
        "a running turn is not retryable"
    );

    // Once that turn FAILS and is the latest, the snapshot exposes it for Retry with its
    // aggregate version and typed settlement reason.
    let c = Connection::open(state_database(&dir)).unwrap();
    c.execute(
        "UPDATE conversation_turns SET status='FAILED',settled_at='2026-10-09T12:00:02.000000000Z',version=4 WHERE turn_id='turn-active'",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO domain_events(event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,causation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest) VALUES('event-turn-failed','workspace-rich','ConversationTurn','turn-active','runtime-local',(SELECT COALESCE(MAX(origin_sequence),0)+1 FROM domain_events WHERE workspace_id='workspace-rich'),4,'2026-10-09T12:00:02.000000000Z','correlation-fail',NULL,1,'conversation.turn.settled.v1','{\"turn_id\":\"turn-active\",\"from\":\"RUNNING\",\"to\":\"FAILED\",\"reason_code\":\"AGENT_TURN_TIMEOUT\",\"aggregate_version\":4}','{}','2026-10-09T12:00:02.000000000Z','sha256:0000000000000000000000000000000000000000000000000000000000000000')",
        [],
    )
    .unwrap();
    c.execute(
        "UPDATE workspace_origin_sequences SET last_sequence=last_sequence+1 WHERE workspace_id='workspace-rich' AND origin_runtime_id='runtime-local'",
        [],
    )
    .unwrap();
    drop(c);
    let failed = adapter
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            None,
            1,
        )
        .unwrap();
    assert!(failed.active_turn.is_none());
    let retryable = failed
        .retryable_turn
        .as_ref()
        .expect("latest FAILED turn is retryable");
    assert_eq!(retryable["turn_id"], "turn-active");
    assert_eq!(retryable["version"], 4);
    assert_eq!(retryable["reason_code"], "AGENT_TURN_TIMEOUT");
    assert_eq!(
        serde_json::to_value(&failed).unwrap()["activity_turn"],
        json!({"turn_id":"turn-active","retry_ordinal":0,"status":"FAILED"}),
        "activity identity is not borrowed from the Retry projection"
    );

    // The latest successful turn is neither active nor retryable, but its REPORTED
    // activity must still be addressable after reopening the Conversation.
    let c = Connection::open(state_database(&dir)).unwrap();
    c.execute(
        "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,content_json,resource_refs_json,created_at) VALUES('message-later','conversation-rich','{}','USER','[]','[]','2026-10-09T12:00:03.000000000Z')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,status,retry_ordinal,created_at,version,presentation_preference) VALUES('turn-completed','conversation-rich','message-later','COMPLETED',2,'2026-10-09T12:00:03.000000000Z',6,'AUTO')",
        [],
    )
    .unwrap();
    drop(c);
    let completed = adapter
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            None,
            1,
        )
        .unwrap();
    assert!(completed.active_turn.is_none());
    assert!(completed.retryable_turn.is_none());
    assert_eq!(
        serde_json::to_value(&completed).unwrap()["activity_turn"],
        json!({"turn_id":"turn-completed","retry_ordinal":2,"status":"COMPLETED"}),
        "completed activity survives when active and retryable projections are empty"
    );

    let second = adapter
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            first.next_cursor.as_deref(),
            1,
        )
        .unwrap();
    assert_eq!(second.items[0]["message"]["message_id"], "message-z");
    assert_eq!(second.items[0]["rich_presentation"], Value::Null);
    assert_eq!(second.next_cursor.as_deref(), Some("message-z"));
    let third = adapter
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            second.next_cursor.as_deref(),
            1,
        )
        .unwrap();
    assert_eq!(third.items[0]["message"]["message_id"], "message-later");
    assert_eq!(third.next_cursor, None);

    assert!(matches!(
        adapter.read_conversation_presentation(
            "someone-else",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            None,
            1,
        ),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        adapter.read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            Some("foreign-message"),
            1,
        ),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        adapter.read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            None,
            MAX_CONVERSATION_PRESENTATION_MESSAGES + 1,
        ),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn conversation_message_projection_uses_exact_same_conversation_turn_status() {
    let (dir, store, _) = fixture();
    let connection = Connection::open(state_database(&dir)).unwrap();
    connection.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,resource_refs_json,created_at) VALUES('source-user','conversation-rich','{}','USER','turn-rich','[{\"kind\":\"TEXT\",\"text\":\"create a task\"}]','[]','2026-10-09T12:00:00.500000000Z')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,status,retry_ordinal,created_at,settled_at,version,presentation_preference) VALUES('turn-rich','conversation-rich','source-user','COMPLETED',0,'2026-10-09T12:00:00.500000000Z','2026-10-09T12:00:01.000000000Z',2,'AUTO')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO conversations(conversation_id,workspace_id,created_at) VALUES('conversation-foreign','workspace-rich',?1)",
            [AT],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,resource_refs_json,created_at) VALUES('foreign-user','conversation-foreign','{}','USER','turn-foreign','[]','[]','2026-10-09T12:00:02.000000000Z')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,status,retry_ordinal,created_at,version,presentation_preference) VALUES('turn-foreign','conversation-foreign','foreign-user','FAILED',0,'2026-10-09T12:00:02.000000000Z',3,'AUTO')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,resource_refs_json,created_at) VALUES('cross-conversation-agent','conversation-rich','{}','AGENT','turn-foreign','[]','[]','2026-10-09T12:00:03.000000000Z')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,resource_refs_json,created_at) VALUES('orphan-agent','conversation-rich','{}','AGENT','missing-turn','[]','[]','2026-10-09T12:00:04.000000000Z')",
            [],
        )
        .unwrap();
    drop(connection);

    let snapshot = adapter(&store)
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            None,
            100,
        )
        .unwrap();
    let status = |message_id: &str| {
        snapshot
            .items
            .iter()
            .find(|item| item["message"]["message_id"] == message_id)
            .map(|item| item["turn_status"].clone())
            .expect("message is in the bounded projection")
    };

    assert_eq!(status("source-user"), json!("COMPLETED"));
    assert_eq!(status("message-rich"), json!("COMPLETED"));
    assert_eq!(status("cross-conversation-agent"), Value::Null);
    assert_eq!(status("orphan-agent"), Value::Null);
}

#[test]
fn latest_completed_activity_identity_is_reconstructed_after_storage_reopen() {
    let (dir, store, request) = fixture();
    let presentation_store = adapter(&store);
    presentation_store
        .publish_rich_presentation(request)
        .unwrap();
    let c = Connection::open(state_database(&dir)).unwrap();
    c.execute(
        "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,content_json,resource_refs_json,created_at) VALUES('message-after-restart','conversation-rich','{}','USER','[]','[]','2026-10-09T12:00:03.000000000Z')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,status,retry_ordinal,created_at,version,presentation_preference) VALUES('turn-completed-after-restart','conversation-rich','message-after-restart','COMPLETED',3,'2026-10-09T12:00:03.000000000Z',7,'AUTO')",
        [],
    )
    .unwrap();
    drop(c);
    drop(presentation_store);
    drop(store);

    let reopened_store = test_store(&dir, Duration::from_millis(100));
    let reopened = adapter(&reopened_store)
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            None,
            10,
        )
        .unwrap();
    assert!(reopened.active_turn.is_none());
    assert!(reopened.retryable_turn.is_none());
    assert_eq!(
        serde_json::to_value(&reopened).unwrap()["activity_turn"],
        json!({
            "turn_id":"turn-completed-after-restart",
            "retry_ordinal":3,
            "status":"COMPLETED"
        }),
        "the latest completed REPORTED activity identity is reconstructed from durable turn state"
    );
}

#[test]
fn conversation_stream_frontier_tracks_latest_message_outside_the_first_page() {
    let (dir, store, _) = fixture();
    let connection = Connection::open(state_database(&dir)).unwrap();
    for index in 0..100 {
        connection.execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,content_json,resource_refs_json,created_at) VALUES(?1,'conversation-rich','{}','USER','[]','[]',?2)",
            params![format!("message-{index:03}"), format!("2026-10-09T12:{:02}:{:02}.000000000Z", (index + 1) / 60, (index + 1) % 60)],
        ).unwrap();
    }
    drop(connection);

    let snapshot = adapter(&store)
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            None,
            100,
        )
        .unwrap();
    assert_eq!(snapshot.items.len(), 100);
    assert_eq!(snapshot.next_cursor.as_deref(), Some("message-098"));
    assert_eq!(
        snapshot.stream_message_frontier.as_deref(),
        Some("message-099")
    );
    assert!(
        serde_json::to_value(&snapshot)
            .unwrap()
            .get("stream_message_frontier")
            .is_none()
    );
}

#[test]
fn conversation_snapshot_frontier_replays_writer_committed_after_snapshot_capture() {
    use std::sync::mpsc;
    use storage_core::{AggregateStateRef, BlobRef, EventDraft, EventStore};

    let (dir, store, _) = fixture();
    let (captured_sender, captured_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let release_receiver = std::sync::Mutex::new(release_receiver);
    let hooked_adapter = adapter(&store).with_frontier_hook(move || {
        captured_sender
            .send(())
            .expect("test is waiting for the captured frontier");
        let _ = release_receiver
            .lock()
            .expect("snapshot release mutex is not poisoned")
            .recv();
    });
    let reader = std::thread::spawn(move || {
        hooked_adapter.read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            None,
            100,
        )
    });

    captured_receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("reader has captured its event frontier inside the transaction");

    let mut writer = Connection::open(state_database(&dir)).expect("concurrent writer opens");
    writer
        .execute_batch("PRAGMA foreign_keys=ON")
        .expect("writer enforces foreign keys");
    let transaction = writer
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("writer can commit alongside the WAL read snapshot");
    transaction
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,resource_refs_json,created_at) VALUES('message-race','conversation-rich','{}','AGENT','turn-race','[{\"kind\":\"TEXT\",\"text\":\"committed during snapshot\"}]','[]','2026-10-09T12:00:02.000000000Z')",
            [],
        )
        .expect("writer commits a message while snapshot read is paused");
    let state_ref = AggregateStateRef {
        blob: BlobRef {
            digest: digest(b"test snapshot race state"),
            size_bytes: b"test snapshot race state".len() as u64,
            media_type: "application/vnd.litecowork.aggregate-state+json".to_owned(),
        },
        entity_revision: 2,
        record_schema_version: 1,
    };
    let event = insert_domain_event(
        &transaction,
        EventDraft {
            event_id: "event-snapshot-race".to_owned(),
            workspace_id: "workspace-rich".to_owned(),
            entity_type: "Conversation".to_owned(),
            entity_id: "conversation-rich".to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 2,
            hlc_timestamp: "2026-10-09T12:00:02.000000000Z".to_owned(),
            correlation_id: "correlation-snapshot-race".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.message.added.v1".to_owned(),
            payload: json!({
                "message_id": "message-race",
                "conversation_id": "conversation-rich"
            }),
            recorded_at: "2026-10-09T12:00:02.000000000Z".to_owned(),
        },
        state_ref,
    )
    .expect("message event commits in the same transaction");
    transaction
        .commit()
        .expect("new message and event commit atomically");
    release_sender
        .send(())
        .expect("release the still-open snapshot transaction");

    let snapshot = reader
        .join()
        .expect("snapshot reader thread completes")
        .expect("real storage snapshot succeeds");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0]["message"]["message_id"], "message-rich");
    assert_eq!(
        snapshot.stream_message_frontier.as_deref(),
        Some("message-rich")
    );
    let snapshot_sequence = snapshot
        .origin_sequence
        .parse::<u64>()
        .expect("snapshot carries the captured durable frontier");
    assert!(event.origin_sequence > snapshot_sequence);

    let replay = store
        .read_workspace_event_page("workspace-rich", "runtime-local", snapshot_sequence, 10)
        .expect("bounded event catch-up reads");
    assert_eq!(replay.events.len(), 1);
    assert_eq!(replay.events[0].event_id, "event-snapshot-race");
    assert_eq!(replay.events[0].origin_sequence, event.origin_sequence);
    assert!(!replay.has_gap);

    let projection = adapter(&store)
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-rich",
            "runtime-local",
            snapshot.stream_message_frontier.as_deref(),
            100,
        )
        .expect("catch-up projection reads after the last message represented by snapshot");
    assert_eq!(projection.items.len(), 1);
    assert_eq!(projection.items[0]["message"]["message_id"], "message-race");
    assert_eq!(
        projection.origin_sequence,
        event.origin_sequence.to_string()
    );
    assert_eq!(projection.next_cursor, None);
}

#[test]
fn conversation_batch_snapshot_shares_origin_frontier_and_read_snapshot() {
    use std::sync::mpsc;
    use storage_core::{AggregateStateRef, BlobRef, EventDraft, EventStore};

    let (dir, store, _) = fixture();
    let connection = Connection::open(state_database(&dir)).unwrap();
    connection
        .execute(
            "INSERT INTO conversations(conversation_id,workspace_id,created_at) VALUES('conversation-second','workspace-rich',?1)",
            [AT],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,content_json,resource_refs_json,created_at) VALUES('message-second','conversation-second','{}','USER','[]','[]','2026-10-09T12:00:01.000000000Z')",
            [],
        )
        .unwrap();
    drop(connection);

    let (captured_sender, captured_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let release_receiver = std::sync::Mutex::new(release_receiver);
    let hooked_adapter = adapter(&store).with_frontier_hook(move || {
        captured_sender
            .send(())
            .expect("test is waiting for the batch frontier");
        let _ = release_receiver
            .lock()
            .expect("batch snapshot release mutex is not poisoned")
            .recv();
    });
    let reader = std::thread::spawn(move || {
        hooked_adapter.read_conversation_presentation_batch(
            "owner-local",
            "workspace-rich",
            &[
                ConversationPresentationRead {
                    conversation_id: "conversation-rich".to_owned(),
                    after_message_id: None,
                },
                ConversationPresentationRead {
                    conversation_id: "conversation-second".to_owned(),
                    after_message_id: None,
                },
            ],
            "runtime-local",
            100,
        )
    });

    captured_receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("batch read has captured the shared frontier inside its transaction");

    let mut writer = Connection::open(state_database(&dir)).expect("concurrent writer opens");
    writer
        .execute_batch("PRAGMA foreign_keys=ON")
        .expect("writer enforces foreign keys");
    let transaction = writer
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("writer can commit alongside the WAL read snapshot");
    transaction
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,resource_refs_json,created_at) VALUES('message-second-race','conversation-second','{}','AGENT','turn-second-race','[{\"kind\":\"TEXT\",\"text\":\"committed during batch snapshot\"}]','[]','2026-10-09T12:00:02.000000000Z')",
            [],
        )
        .expect("writer adds a message while batch read is paused");
    let state_ref = AggregateStateRef {
        blob: BlobRef {
            digest: digest(b"test batch snapshot race state"),
            size_bytes: b"test batch snapshot race state".len() as u64,
            media_type: "application/vnd.litecowork.aggregate-state+json".to_owned(),
        },
        entity_revision: 2,
        record_schema_version: 1,
    };
    let event = insert_domain_event(
        &transaction,
        EventDraft {
            event_id: "event-batch-snapshot-race".to_owned(),
            workspace_id: "workspace-rich".to_owned(),
            entity_type: "Conversation".to_owned(),
            entity_id: "conversation-second".to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            entity_revision: 2,
            hlc_timestamp: "2026-10-09T12:00:02.000000000Z".to_owned(),
            correlation_id: "correlation-batch-snapshot-race".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.message.added.v1".to_owned(),
            payload: json!({
                "message_id": "message-second-race",
                "conversation_id": "conversation-second"
            }),
            recorded_at: "2026-10-09T12:00:02.000000000Z".to_owned(),
        },
        state_ref,
    )
    .expect("message event commits with the second Conversation update");
    transaction
        .commit()
        .expect("message and event commit atomically");
    release_sender
        .send(())
        .expect("release the shared batch snapshot transaction");

    let snapshots = reader
        .join()
        .expect("batch snapshot reader thread completes")
        .expect("authorized batch read succeeds");
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].conversation_id, "conversation-rich");
    assert_eq!(snapshots[1].conversation_id, "conversation-second");
    assert_eq!(
        snapshots[0].items[0]["message"]["message_id"],
        "message-rich"
    );
    assert_eq!(
        snapshots[1].items[0]["message"]["message_id"],
        "message-second"
    );
    assert_eq!(snapshots[0].origin_sequence, snapshots[1].origin_sequence);
    assert_eq!(
        snapshots[1].stream_message_frontier.as_deref(),
        Some("message-second")
    );
    assert_eq!(snapshots[1].items.len(), 1);

    let common_frontier = snapshots[0]
        .origin_sequence
        .parse::<u64>()
        .expect("both snapshots carry the shared durable frontier");
    assert!(event.origin_sequence > common_frontier);
    let replay = store
        .read_workspace_event_page("workspace-rich", "runtime-local", common_frontier, 10)
        .expect("bounded event catch-up reads after the common frontier");
    assert_eq!(replay.events.len(), 1);
    assert_eq!(replay.events[0].event_id, "event-batch-snapshot-race");
    assert!(!replay.has_gap);

    let catch_up = adapter(&store)
        .read_conversation_presentation(
            "owner-local",
            "workspace-rich",
            "conversation-second",
            "runtime-local",
            snapshots[1].stream_message_frontier.as_deref(),
            100,
        )
        .expect("catch-up projection can read after the accepted batch snapshot");
    assert_eq!(catch_up.items.len(), 1);
    assert_eq!(
        catch_up.items[0]["message"]["message_id"],
        "message-second-race"
    );
    assert_eq!(catch_up.origin_sequence, event.origin_sequence.to_string());
}

#[test]
fn conversation_batch_read_fails_closed_for_invalid_or_foreign_filters() {
    let (_dir, store, _) = fixture();
    let adapter = adapter(&store);

    assert!(matches!(
        adapter.read_conversation_presentation_batch(
            "owner-local",
            "workspace-rich",
            &[],
            "runtime-local",
            10,
        ),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        adapter.read_conversation_presentation_batch(
            "owner-local",
            "workspace-rich",
            &[
                ConversationPresentationRead {
                    conversation_id: "conversation-rich".to_owned(),
                    after_message_id: None,
                },
                ConversationPresentationRead {
                    conversation_id: "conversation-rich".to_owned(),
                    after_message_id: None,
                },
            ],
            "runtime-local",
            10,
        ),
        Err(StoreError::Invalid(_))
    ));
    assert_eq!(
        adapter
            .read_conversation_presentation_batch(
                "owner-local",
                "workspace-rich",
                &[ConversationPresentationRead {
                    conversation_id: "conversation-missing".to_owned(),
                    after_message_id: None,
                }],
                "runtime-local",
                10,
            )
            .unwrap_err(),
        StoreError::NotFound
    );
    assert_eq!(
        adapter
            .read_conversation_presentation_batch(
                "someone-else",
                "workspace-rich",
                &[ConversationPresentationRead {
                    conversation_id: "conversation-rich".to_owned(),
                    after_message_id: None,
                }],
                "runtime-local",
                10,
            )
            .unwrap_err(),
        StoreError::NotFound
    );
    assert!(matches!(
        adapter.read_conversation_presentation_batch(
            "owner-local",
            "workspace-rich",
            &[ConversationPresentationRead {
                conversation_id: "conversation-rich".to_owned(),
                after_message_id: None,
            }],
            "runtime-local",
            0,
        ),
        Err(StoreError::Invalid(_))
    ));
}

fn adapter(store: &SqliteWorkspaceStore) -> SqliteRichPresentationStore {
    SqliteRichPresentationStore::new(store.clone())
}
fn update_document(request: &mut PublishRichPresentation, f: impl FnOnce(&mut Value)) {
    let mut doc: Value = serde_json::from_slice(&request.canonical_document).unwrap();
    f(&mut doc);
    request.canonical_document = canonical_json(&doc).unwrap();
    request.presentation.document_digest = digest(&request.canonical_document);
    request.presentation.document_size_bytes = request.canonical_document.len() as u64;
    request.presentation.document_ref.digest = request.presentation.document_digest.clone();
    request.presentation.document_ref.size_bytes = request.presentation.document_size_bytes;
    request.event.payload["document_digest"] = json!(request.presentation.document_digest);
    request.event.payload["document_size_bytes"] = json!(request.presentation.document_size_bytes);
}
/// Replaces the document's blocks and the provenance entry each block path requires.
fn set_blocks(request: &mut PublishRichPresentation, blocks: Value, paths: &[&str]) {
    update_document(request, |doc| {
        doc["root_blocks"] = blocks;
        doc["block_provenance"] = Value::Array(
            paths
                .iter()
                .map(|path| {
                    json!({"block_path":path,"origin":"SEMANTIC_MESSAGE","resource_refs":[],
                    "artifact_refs":[],"evidence_refs":[],"verification_refs":[]})
                })
                .collect(),
        );
    });
}
fn bind_two_row_semantic_table(
    dir: &tempfile::TempDir,
    request: &mut PublishRichPresentation,
) -> (String, usize, usize) {
    let lines = [
        "Hello 🌍",
        "Revenue by Month is reported in INR at lakh scale for the monthly interval with sum aggregation.",
        "| Month | Revenue |",
        "| --- | --- |",
        "| Jan | 12 |",
        "| Feb | 18 |",
    ];
    let text = lines.join("\n");
    let blocks: Vec<Value> = lines
        .iter()
        .map(|line| json!({"kind":"TEXT","text":line}))
        .collect();
    let content_json = serde_json::to_string(&blocks).unwrap();
    let conn = Connection::open(state_database(dir)).unwrap();
    conn.execute(
        "UPDATE conversation_messages SET content_json=?1 WHERE message_id=?2",
        params![content_json, request.presentation.message_id],
    )
    .unwrap();
    let semantic = json!({"content":blocks,"resource_refs":[]});
    let semantic_digest = digest(&canonical_json(&semantic).unwrap());
    request.presentation.semantic_content_digest = semantic_digest.clone();
    request.event.payload["semantic_content_digest"] = json!(semantic_digest);
    let start = text.find("| Month | Revenue |").unwrap();
    let end = text.len();
    update_document(request, |document| {
        document["semantic_content_digest"] = json!(semantic_digest);
        document["renderer_contract_version"] = json!(2);
    });
    request.presentation.renderer_contract_version = 2;
    request.event.payload["renderer_contract_version"] = json!(2);
    (text, start, end)
}
fn hello_slice() -> Value {
    json!({"kind":"TEXT_SLICE","source":{"start_utf8_byte":0,"end_utf8_byte_exclusive":10,
        "slice_digest":digest("Hello 🌍".as_bytes())}})
}
/// The single document cap must stay 512 KiB on every surface that publishes or consumes
/// a RichPresentation. A larger cap in storage, or in the wire schema, is the 016 defect:
/// the document publishes and the desktop then refuses it and falls back for every read.
#[test]
fn rich_presentation_document_cap_agrees_with_the_desktop_and_the_wire_schema() {
    assert_eq!(MAX_RICH_PRESENTATION_BYTES, 524_288);
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let desktop = format!("{root}/apps/litecowork-ui/src/presentation/rich-presentation.ts");
    if let Ok(source) = std::fs::read_to_string(desktop) {
        assert!(
            source.contains("const MAX_DOCUMENT_BYTES = 512 * 1024;"),
            "the desktop document cap must stay 512 KiB to match storage"
        );
        for kind in [
            "\"TEXT_SLICE\"",
            "\"LAYOUT\"",
            "\"CARD\"",
            "\"CALLOUT\"",
            "\"TABLE\"",
        ] {
            assert!(
                source.contains(kind),
                "the desktop allowlist must keep {kind}"
            );
        }
        assert!(
            source.contains("value.kind === \"CALLOUT\"")
                && source.contains("value.kind === \"TABLE\""),
            "the desktop must keep validating CALLOUT and TABLE"
        );
    }
    let schema =
        std::fs::read_to_string(format!("{root}/docs/schemas/operator-stream.schema.json"))
            .expect("the operator stream schema is part of this repository");
    assert!(
        schema.contains(
            "\"document_size_bytes\": {\"type\": \"integer\", \"minimum\": 1, \"maximum\": 524288}"
        ),
        "the served wire schema must cap the presentation document at the same 512 KiB"
    );
}
#[test]
fn rich_publication_accepts_card_callout_and_table_which_the_desktop_renders() {
    let (_dir, store, mut request) = fixture();
    set_blocks(
        &mut request,
        json!([
            {"kind":"CARD","title":"Summary","children":[hello_slice()]},
            {"kind":"CALLOUT","tone":"INFO","accessible_summary":"Summary","source":{"start_utf8_byte":0,"end_utf8_byte_exclusive":10,"slice_digest":digest("Hello 🌍".as_bytes())}},
            {"kind":"TABLE","headers":["Metric","Value"],"rows":[["Cost",12]],"accessible_summary":"Summary"}
        ]),
        &["/0", "/0/children/0", "/1", "/2"],
    );
    let committed = adapter(&store)
        .publish_rich_presentation(request.clone())
        .expect("the V1 allowlist the desktop renders must be publishable");
    assert_eq!(committed.presentation, request.presentation);
    let read = adapter(&store)
        .read_rich_presentation("owner-local", "workspace-rich", "presentation-rich")
        .unwrap()
        .unwrap();
    assert_eq!(read.canonical_document, request.canonical_document);
}

#[test]
fn rich_publication_accepts_renderer_v2_chart_bound_to_two_semantic_rows() {
    let (dir, store, mut request) = fixture();
    let (text, start, end) = bind_two_row_semantic_table(&dir, &mut request);
    let source = text.as_bytes().get(start..end).unwrap();
    set_blocks(
        &mut request,
        json!([{
            "kind":"CHART",
            "chart_kind":"BAR",
            "binding":{
                "source":"SEMANTIC_TABLE",
                "semantic_table_slice":{
                    "start_utf8_byte":start,
                    "end_utf8_byte_exclusive":end,
                    "slice_digest":digest(source)
                }
            },
            "category_column_index":0,
            "series_column_indexes":[1],
            "accessible_summary":"Revenue by Month is reported in INR at lakh scale for the monthly interval with sum aggregation.",
            "semantics":{"unit":"INR","scale":"LAKH","time_interval":"monthly","aggregation":"SUM"}
        }]),
        &["/0"],
    );

    let committed = adapter(&store)
        .publish_rich_presentation(request.clone())
        .expect("renderer v2 accepts a bounded chart bound to exact semantic table bytes");
    assert_eq!(committed.presentation.renderer_contract_version, 2);
    let read = adapter(&store)
        .read_rich_presentation("owner-local", "workspace-rich", "presentation-rich")
        .unwrap()
        .unwrap();
    assert_eq!(read.canonical_document, request.canonical_document);
}

#[test]
fn rich_publication_refuses_a_card_summary_or_table_that_invents_a_fact() {
    let (_dir, store, request) = fixture();
    for (blocks, paths) in [
        (
            json!([{"kind":"CARD","title":"Guaranteed savings","children":[]}]),
            vec!["/0"],
        ),
        (
            json!([{"kind":"CARD","title":"   ","children":[]}]),
            vec!["/0"],
        ),
        (
            json!([{"kind":"CALLOUT","tone":"WARNING","accessible_summary":"Invented warning","source":{"start_utf8_byte":0,"end_utf8_byte_exclusive":10,"slice_digest":digest("Hello 🌍".as_bytes())}}]),
            vec!["/0"],
        ),
        (
            json!([{"kind":"TABLE","headers":["Metric","Value"],"rows":[["Cost",13]]}]),
            vec!["/0"],
        ),
        (
            json!([{"kind":"TABLE","headers":["Metric","Total"],"rows":[["Cost",12]]}]),
            vec!["/0"],
        ),
        (
            json!([{"kind":"TABLE","headers":["Metric","Value"],"rows":[["Cost",12]],"accessible_summary":"Invented summary"}]),
            vec!["/0"],
        ),
    ] {
        let mut bad = request.clone();
        set_blocks(&mut bad, blocks, &paths);
        assert!(
            adapter(&store).publish_rich_presentation(bad).is_err(),
            "a presentation must not introduce a fact the semantic answer lacks"
        );
    }
    // A document-level accessibility summary is held to the same rule as a block summary.
    let mut bad = request;
    update_document(&mut bad, |doc| {
        doc["accessibility_summary"] = json!("Invented accessibility summary")
    });
    assert!(adapter(&store).publish_rich_presentation(bad).is_err());
}
#[test]
fn rich_publication_refuses_malformed_card_callout_and_table_shapes() {
    let (_dir, store, request) = fixture();
    for (blocks, paths) in [
        // CARD does not carry width, and its children are capped at 20.
        (
            json!([{"kind":"CARD","title":"Summary","width":"WIDE","children":[]}]),
            vec!["/0"],
        ),
        (
            json!([{
                "kind":"CARD",
                "title":"Summary",
                "children":(0..21).map(|_| hello_slice()).collect::<Vec<_>>()
            }]),
            vec!["/0"],
        ),
        // CALLOUT tone is closed.
        (
            json!([{"kind":"CALLOUT","tone":"LOUD","accessible_summary":"Summary","source":{"start_utf8_byte":0,"end_utf8_byte_exclusive":10,"slice_digest":digest("Hello 🌍".as_bytes())}}]),
            vec!["/0"],
        ),
        // TABLE needs at least one header, row width must match, cells are scalar-only.
        (json!([{"kind":"TABLE","headers":[],"rows":[]}]), vec!["/0"]),
        (
            json!([{"kind":"TABLE","headers":["Metric","Value"],"rows":[["Cost"]]}]),
            vec!["/0"],
        ),
        (
            json!([{"kind":"TABLE","headers":["Metric","Value"],"rows":[["Cost",["x"]]]}]),
            vec!["/0"],
        ),
    ] {
        let mut bad = request.clone();
        set_blocks(&mut bad, blocks, &paths);
        assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    }
}
fn assert_no_publication(dir: &tempfile::TempDir) {
    let c = Connection::open(state_database(dir)).unwrap();
    let rows: i64 = c
        .query_row("SELECT COUNT(*) FROM rich_presentations", [], |r| r.get(0))
        .unwrap();
    let events: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE type='rich.presentation.published.v1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((rows, events), (0, 0));
}
#[test]
fn rich_publication_owner_is_checked_before_message_lookup() {
    let (dir, store, mut request) = fixture();
    request.principal_id = "intruder".into();
    assert_eq!(
        adapter(&store)
            .publish_rich_presentation(request.clone())
            .unwrap_err(),
        StoreError::NotFound
    );
    request.presentation.message_id = "missing".into();
    assert_eq!(
        adapter(&store)
            .publish_rich_presentation(request)
            .unwrap_err(),
        StoreError::NotFound
    );
    assert_eq!(
        adapter(&store)
            .read_rich_presentation("intruder", "workspace-rich", "presentation-rich")
            .unwrap_err(),
        StoreError::NotFound
    );
    assert_no_publication(&dir);
}
#[test]
fn rich_publication_rejects_missing_and_foreign_conversation_messages() {
    let (dir, store, request) = fixture();
    for foreign in ["message-other", "message-missing"] {
        let mut bad = request.clone();
        bad.presentation.message_id = foreign.into();
        assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    }
    let c = Connection::open(state_database(&dir)).unwrap();
    c.execute("INSERT INTO conversations(conversation_id,workspace_id,created_at) VALUES('conversation-other','workspace-rich',?1)",[AT]).unwrap();
    let mut bad = request;
    bad.presentation.conversation_id = "conversation-other".into();
    assert_eq!(
        adapter(&store).publish_rich_presentation(bad).unwrap_err(),
        StoreError::NotFound
    );
    assert_no_publication(&dir);
}
#[test]
fn rich_publication_rejects_cross_workspace_binding_and_non_agent_message() {
    let (dir, store, mut request) = fixture();
    // Same owner, another Workspace: authorization succeeds, the source binding does not.
    let c = Connection::open(state_database(&dir)).unwrap();
    c.execute("INSERT INTO workspaces(workspace_id,name,owner_principal_id,replication_policy,status,created_at,updated_at,version) VALUES('workspace-other','Other','owner-local','LOCAL_ONLY','ACTIVE',?1,?1,1)",[AT]).unwrap();
    request.presentation.workspace_id = "workspace-other".into();
    assert_eq!(
        adapter(&store)
            .publish_rich_presentation(request.clone())
            .unwrap_err(),
        StoreError::NotFound
    );
    request.presentation.workspace_id = "workspace-rich".into();
    c.execute(
        "UPDATE conversation_messages SET role='USER' WHERE message_id='message-rich'",
        [],
    )
    .unwrap();
    assert!(adapter(&store).publish_rich_presentation(request).is_err());
    assert_no_publication(&dir);
}
#[test]
fn rich_publication_rejects_semantic_and_blob_metadata_digest_mismatch() {
    let (dir, store, request) = fixture();
    for field in [0, 1, 2] {
        let mut bad = request.clone();
        match field {
            0 => bad.presentation.semantic_content_digest = format!("sha256:{}", "a".repeat(64)),
            1 => bad.presentation.document_ref.digest = format!("sha256:{}", "a".repeat(64)),
            _ => bad.canonical_document[0] = b'!',
        }
        assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    }
    assert_no_publication(&dir);
}
#[test]
fn rich_publication_rejects_non_boundary_or_wrong_digest_text_slices() {
    let (dir, store, request) = fixture();
    for end in [0, 7, 11] {
        let mut bad = request.clone();
        update_document(&mut bad, |doc| {
            doc["root_blocks"][0]["source"]["end_utf8_byte_exclusive"] = json!(end)
        });
        assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    }
    let mut bad = request;
    update_document(&mut bad, |doc| {
        doc["root_blocks"][0]["source"]["slice_digest"] =
            json!(format!("sha256:{}", "a".repeat(64)))
    });
    assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    assert_no_publication(&dir);
}
#[test]
fn rich_publication_rejects_noncanonical_unknown_oversize_and_unsupported_document() {
    let (dir, store, request) = fixture();
    let mut bad = request.clone();
    bad.canonical_document.push(b' ');
    bad.presentation.document_digest = digest(&bad.canonical_document);
    bad.presentation.document_ref.digest = bad.presentation.document_digest.clone();
    bad.presentation.document_size_bytes += 1;
    bad.presentation.document_ref.size_bytes += 1;
    assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    let mut bad = request.clone();
    update_document(&mut bad, |doc| {
        doc["arbitrary_html"] = json!("<script>bad</script>")
    });
    assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    let mut bad = request.clone();
    bad.presentation.document_size_bytes = MAX_RICH_PRESENTATION_BYTES + 1;
    bad.presentation.document_ref.size_bytes = bad.presentation.document_size_bytes;
    assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    let mut bad = request.clone();
    bad.presentation.renderer_contract_version = 2;
    assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    let mut bad = request;
    update_document(&mut bad, |doc| {
        doc["root_blocks"][0] = json!({"kind":"APPROVAL","status":"APPROVED"})
    });
    assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    assert_no_publication(&dir);
}

#[test]
fn rich_publication_rejects_divider_which_is_not_in_the_v1_wire_schema() {
    let (dir, store, mut request) = fixture();
    update_document(&mut request, |doc| {
        doc["root_blocks"][0] = json!({"kind":"DIVIDER"});
    });
    assert!(adapter(&store).publish_rich_presentation(request).is_err());
    assert_no_publication(&dir);
}

#[test]
fn rich_publication_rejects_missing_duplicate_and_forged_provenance() {
    let (dir, store, request) = fixture();
    for mode in [0, 1, 2] {
        let mut bad = request.clone();
        update_document(&mut bad, |doc| match mode {
            0 => doc["block_provenance"] = json!([]),
            1 => {
                let entry = doc["block_provenance"][0].clone();
                doc["block_provenance"].as_array_mut().unwrap().push(entry)
            }
            _ => doc["block_provenance"][0]["origin"] = json!("CORE_PROJECTION"),
        });
        assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    }
    assert_no_publication(&dir);
}
#[test]
fn rich_publication_rejects_event_identity_or_payload_mismatch_atomically() {
    let (dir, store, request) = fixture();
    for mode in [0, 1, 2] {
        let mut bad = request.clone();
        match mode {
            0 => bad.event.entity_id = "wrong".into(),
            1 => bad.event.workspace_id = "wrong".into(),
            _ => bad.event.payload["message_id"] = json!("wrong"),
        };
        assert!(adapter(&store).publish_rich_presentation(bad).is_err());
    }
    assert_no_publication(&dir);
}
#[test]
fn rich_publication_exact_replay_returns_the_stored_row_and_database_mutations_are_rejected() {
    let (dir, store, request) = fixture();
    let first = adapter(&store)
        .publish_rich_presentation(request.clone())
        .unwrap();
    let replay = adapter(&store)
        .publish_rich_presentation(request.clone())
        .unwrap();
    assert_eq!(replay.presentation, first.presentation);
    assert_eq!(replay.event, first.event);
    let c = Connection::open(state_database(&dir)).unwrap();
    let (presentations, events) = c
        .query_row(
            "SELECT (SELECT COUNT(*) FROM rich_presentations), (SELECT COUNT(*) FROM domain_events WHERE event_id='event-rich')",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .unwrap();
    assert_eq!((presentations, events), (1, 1));
    assert!(
        c.execute(
            "UPDATE rich_presentations SET renderer_contract_version=2",
            []
        )
        .is_err()
    );
    assert!(c.execute("DELETE FROM rich_presentations", []).is_err());
    assert!(
        adapter(&store)
            .read_rich_presentation("owner-local", "workspace-rich", "presentation-rich")
            .unwrap()
            .is_some()
    );
    c.execute(
        "UPDATE workspaces SET status='ARCHIVED' WHERE workspace_id='workspace-rich'",
        [],
    )
    .unwrap();
    let archived_replay = adapter(&store).publish_rich_presentation(request).unwrap();
    assert_eq!(
        archived_replay.presentation.presentation_id,
        "presentation-rich"
    );
    assert!(
        adapter(&store)
            .read_rich_presentation("owner-local", "workspace-rich", "presentation-rich")
            .unwrap()
            .is_some()
    );
}

#[test]
fn rich_publication_rejects_changed_document_under_the_same_message_identity() {
    let (dir, store, request) = fixture();
    adapter(&store)
        .publish_rich_presentation(request.clone())
        .unwrap();
    let mut changed = request;
    update_document(&mut changed, |doc| {
        doc["accessibility_summary"] = json!("Hello 🌍")
    });
    assert!(matches!(
        adapter(&store).publish_rich_presentation(changed),
        Err(StoreError::Invalid(ref message)) if message == "presentation identity unavailable"
    ));
    assert_no_additional_publication(&dir);
}

fn assert_no_additional_publication(dir: &tempfile::TempDir) {
    let connection = Connection::open(state_database(dir)).unwrap();
    let (presentations, rich_events): (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM rich_presentations), (SELECT COUNT(*) FROM domain_events WHERE type='rich.presentation.published.v1')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((presentations, rich_events), (1, 1));
}

#[test]
fn rich_publication_missing_blob_does_not_remove_or_change_semantic_message() {
    let (dir, store, request) = fixture();
    adapter(&store)
        .publish_rich_presentation(request.clone())
        .unwrap();
    store
        .inner
        .blobs
        .remove(
            "workspace-rich",
            BlobPurpose::RichPresentation,
            &request.presentation.document_ref,
        )
        .unwrap();
    assert_eq!(
        adapter(&store)
            .read_rich_presentation("owner-local", "workspace-rich", "presentation-rich")
            .unwrap_err(),
        StoreError::NotFound
    );
    let c = Connection::open(state_database(&dir)).unwrap();
    let text: String = c
        .query_row(
            "SELECT content_json FROM conversation_messages WHERE message_id='message-rich'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(text.contains("Hello 🌍"));
    let count: i64 = c
        .query_row("SELECT COUNT(*) FROM rich_presentations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
struct FailingBlobs {
    inner: Arc<dyn BlobStore>,
    corrupt_read: bool,
}
impl BlobStore for FailingBlobs {
    fn put(&self, w: &str, p: BlobPurpose, b: &[u8], m: &str) -> Result<BlobRef, StoreError> {
        if p == BlobPurpose::RichPresentation && !self.corrupt_read {
            Err(StoreError::Io("injected disk failure".into()))
        } else {
            self.inner.put(w, p, b, m)
        }
    }
    fn get(&self, w: &str, p: BlobPurpose, b: &BlobRef) -> Result<Vec<u8>, StoreError> {
        if p == BlobPurpose::RichPresentation && self.corrupt_read {
            Ok(b"corrupt".to_vec())
        } else {
            self.inner.get(w, p, b)
        }
    }
}
#[test]
fn rich_publication_blob_failure_never_commits_metadata_or_event() {
    for corrupt_read in [false, true] {
        let (dir, store, request) = fixture();
        let faulty = SqliteWorkspaceStore::open(
            state_database(&dir),
            Arc::new(FailingBlobs {
                inner: Arc::clone(&store.inner.blobs),
                corrupt_read,
            }),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_millis(100),
            },
        )
        .unwrap();
        assert!(adapter(&faulty).publish_rich_presentation(request).is_err());
        assert_no_publication(&dir);
    }
}

#[test]
fn rich_publication_archived_workspace_denies_before_message_lookup() {
    let (dir, store, request) = fixture();
    let c = Connection::open(state_database(&dir)).unwrap();
    c.execute(
        "UPDATE workspaces SET status='ARCHIVED' WHERE workspace_id='workspace-rich'",
        [],
    )
    .unwrap();
    assert_eq!(
        adapter(&store)
            .publish_rich_presentation(request.clone())
            .unwrap_err(),
        StoreError::NotFound
    );
    let mut missing = request.clone();
    missing.presentation.message_id = "missing".into();
    assert_eq!(
        adapter(&store)
            .publish_rich_presentation(missing)
            .unwrap_err(),
        StoreError::NotFound
    );
    let mut foreign = request;
    foreign.presentation.workspace_id = "foreign".into();
    assert_eq!(
        adapter(&store)
            .publish_rich_presentation(foreign)
            .unwrap_err(),
        StoreError::NotFound
    );
    assert_no_publication(&dir);
}
#[test]
fn rich_publication_event_insert_failure_rolls_back_metadata_and_event() {
    let (dir, store, mut request) = fixture();
    // Workspace creation already committed this EventId. The collision occurs only
    // after the presentation metadata/blob are staged, inside the same transaction.
    request.event.event_id = "event-create".into();
    assert!(adapter(&store).publish_rich_presentation(request).is_err());
    assert_no_publication(&dir);
    let c = Connection::open(state_database(&dir)).unwrap();
    let messages: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM conversation_messages WHERE message_id='message-rich'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(messages, 1);
}
#[test]
fn rich_publication_duplicate_identity_error_does_not_reveal_scope_or_revision() {
    let (dir, store, request) = fixture();
    adapter(&store)
        .publish_rich_presentation(request.clone())
        .unwrap();
    let mut conflicting = request.clone();
    conflicting.presentation.presentation_id = "presentation-other".into();
    conflicting.event.entity_id = "presentation-other".into();
    conflicting.event.payload["presentation_id"] = json!("presentation-other");
    update_document(&mut conflicting, |doc| {
        doc["presentation_id"] = json!("presentation-other")
    });
    let own_error = adapter(&store)
        .publish_rich_presentation(conflicting)
        .unwrap_err();
    let c = Connection::open(state_database(&dir)).unwrap();
    c.execute("INSERT INTO workspaces(workspace_id,name,owner_principal_id,replication_policy,status,created_at,updated_at,version) VALUES('workspace-second','Second','owner-local','LOCAL_ONLY','ACTIVE',?1,?1,1)",[AT]).unwrap();
    c.execute("INSERT INTO conversations(conversation_id,workspace_id,created_at) VALUES('conversation-second','workspace-second',?1)",[AT]).unwrap();
    c.execute("INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) SELECT 'message-second','conversation-second',author_json,role,'turn-second',content_json,created_at FROM conversation_messages WHERE message_id='message-rich'",[]).unwrap();
    let mut foreign = request;
    foreign.presentation.workspace_id = "workspace-second".into();
    foreign.presentation.conversation_id = "conversation-second".into();
    foreign.presentation.message_id = "message-second".into();
    foreign.event.workspace_id = "workspace-second".into();
    foreign.event.payload["conversation_id"] = json!("conversation-second");
    foreign.event.payload["message_id"] = json!("message-second");
    update_document(&mut foreign, |doc| {
        doc["message_id"] = json!("message-second")
    });
    let foreign_error = adapter(&store)
        .publish_rich_presentation(foreign)
        .unwrap_err();
    assert_eq!(
        own_error,
        StoreError::Invalid("presentation identity unavailable".into())
    );
    assert_eq!(foreign_error, own_error);
    let count: i64 = c
        .query_row("SELECT COUNT(*) FROM rich_presentations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
