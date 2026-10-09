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
    conn.execute("INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,turn_id,content_json,created_at) VALUES('message-rich','conversation-rich','{}','AGENT','turn-rich','[{\"kind\":\"TEXT\",\"text\":\"Hello 🌍\"}]',?1)", [AT]).unwrap();
    let semantic = json!({"content":[{"kind":"TEXT","text":"Hello 🌍"}],"resource_refs":[]});
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
fn rich_publication_duplicate_and_database_mutations_are_rejected() {
    let (dir, store, request) = fixture();
    adapter(&store)
        .publish_rich_presentation(request.clone())
        .unwrap();
    assert!(matches!(
        adapter(&store).publish_rich_presentation(request),
        Err(StoreError::Invalid(ref message)) if message == "presentation identity unavailable"
    ));
    let c = Connection::open(state_database(&dir)).unwrap();
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
    assert!(
        adapter(&store)
            .read_rich_presentation("owner-local", "workspace-rich", "presentation-rich")
            .unwrap()
            .is_some()
    );
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
    let own_error = adapter(&store)
        .publish_rich_presentation(request.clone())
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
