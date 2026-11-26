//! Builds a safe, immutable display companion only after Conversation storage has
//! committed the authoritative semantic AGENT message.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use storage_core::conversation::CommittedConversationAgentResponse;
use storage_core::rich_presentation::{
    CommittedRichPresentation, PublishRichPresentation, RICH_PRESENTATION_MEDIA_TYPE,
    RichPresentationRecord, RichPresentationStore,
};
use storage_core::{BlobRef, ConversationMessageRecord, DomainEvent, EventDraft, StoreError};
use storage_sqlite::{SqliteRichPresentationStore, SqliteWorkspaceStore};

type Publish =
    dyn Fn(PublishRichPresentation) -> Result<CommittedRichPresentation, StoreError> + Send + Sync;

#[derive(Clone)]
pub struct PresentationBinder {
    publish: Arc<Publish>,
    dropped: Arc<AtomicU64>,
}

impl PresentationBinder {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        let publisher = SqliteRichPresentationStore::new(store);
        Self::from_publisher(move |request| publisher.publish_rich_presentation(request))
    }

    #[cfg(test)]
    fn with_publisher(
        publish: impl Fn(PublishRichPresentation) -> Result<CommittedRichPresentation, StoreError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self::from_publisher(publish)
    }

    fn from_publisher(
        publish: impl Fn(PublishRichPresentation) -> Result<CommittedRichPresentation, StoreError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            publish: Arc::new(publish),
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Publish a companion from the exact result returned by the semantic commit.
    /// Any error is intentionally content-free and cannot change Conversation truth.
    pub fn bind_after_commit(
        &self,
        principal_id: &str,
        committed: &CommittedConversationAgentResponse,
    ) -> Option<CommittedRichPresentation> {
        if committed.turn.turn_id != committed.message.turn_id.as_deref().unwrap_or_default()
            || committed.turn.agent_session_id.as_deref()
                != Some(committed.session.agent_session_id.as_str())
            || committed.session.status != "CLOSED"
            || committed.session.scope_kind != "CONVERSATION"
            || committed.session.workspace_id != committed.message_event.workspace_id
            || committed.session.conversation_id.as_deref()
                != Some(committed.message.conversation_id.as_str())
            || committed.session.conversation_turn_id.as_deref()
                != Some(committed.turn.turn_id.as_str())
        {
            self.note_dropped();
            return None;
        }
        let request = match build_publish_request(
            principal_id,
            &committed.message,
            &committed.session.agent_session_id,
            &committed.message_event,
        ) {
            Ok(Some(request)) => request,
            Ok(None) => return None,
            Err(()) => {
                self.note_dropped();
                return None;
            }
        };
        self.publish_request(request)
    }

    fn publish_request(
        &self,
        request: PublishRichPresentation,
    ) -> Option<CommittedRichPresentation> {
        match (self.publish)(request) {
            Ok(committed) => Some(committed),
            Err(_) => {
                self.note_dropped();
                None
            }
        }
    }

    pub fn dropped_publications(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    fn note_dropped(&self) {
        let _ = self
            .dropped
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_add(1))
            });
    }
}

fn build_publish_request(
    principal_id: &str,
    message: &ConversationMessageRecord,
    producer_agent_session_id: &str,
    source_event: &DomainEvent,
) -> Result<Option<PublishRichPresentation>, ()> {
    if principal_id.trim().is_empty()
        || producer_agent_session_id.trim().is_empty()
        || message.role != "AGENT"
        || message.turn_id.as_deref().is_none_or(str::is_empty)
        || message.agent_session_id.as_deref() != Some(producer_agent_session_id)
        || message.conversation_id != source_event.entity_id
        || source_event.entity_type != "Conversation"
        || source_event.event_type != "conversation.message.added.v1"
        || source_event.payload["message_id"] != message.message_id
    {
        return Err(());
    }

    let content = message.content.as_array().ok_or(())?;
    let text = content
        .iter()
        .filter(|block| block["kind"] == "TEXT")
        .map(|block| block["text"].as_str().ok_or(()))
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    if text.len() > domain_presentation::MAX_SEMANTIC_BYTES || !message.resource_refs.is_array() {
        return Ok(None);
    }

    let tables = domain_presentation::extract_semantic_markdown_tables(&text);
    if tables.is_empty() {
        return Ok(None);
    }

    let semantic_content_digest = digest_json(&json!({
        "content": message.content,
        "resource_refs": message.resource_refs,
    }))?;
    let (root_blocks, block_provenance) = source_bound_blocks(&text, &tables)?;
    if root_blocks.is_empty() || root_blocks.len() > 200 {
        return Ok(None);
    }

    let identity_digest = digest_json(&json!({
        "workspace_id": source_event.workspace_id,
        "conversation_id": message.conversation_id,
        "message_id": message.message_id,
    }))?;
    let presentation_id = format!(
        "presentation-{}",
        identity_digest.trim_start_matches("sha256:")
    );
    let event_id = format!(
        "event-rich-presentation-{}",
        identity_digest.trim_start_matches("sha256:")
    );
    let document = json!({
        "schema_version": 1,
        "renderer_contract_version": 1,
        "presentation_id": presentation_id,
        "message_id": message.message_id,
        "semantic_content_digest": semantic_content_digest,
        "root_blocks": root_blocks,
        "block_provenance": block_provenance,
    });
    let canonical_document = serde_json_canonicalizer::to_vec(&document).map_err(|_| ())?;
    if canonical_document.is_empty()
        || canonical_document.len() > domain_presentation::MAX_DOCUMENT_BYTES
    {
        return Ok(None);
    }
    let document_digest = sha256_digest(&canonical_document);
    domain_presentation::validate_document(
        domain_presentation::ValidationContext {
            schema_version: 1,
            renderer_contract_version: 1,
            presentation_id: &presentation_id,
            message_id: &message.message_id,
            semantic_content_digest: &semantic_content_digest,
            document_digest: &document_digest,
            document_size_bytes: canonical_document.len() as u64,
            semantic_text: &text,
        },
        &canonical_document,
    )
    .map_err(|_| ())?;

    let document_size_bytes = canonical_document.len() as u64;
    let record = RichPresentationRecord {
        presentation_id: presentation_id.clone(),
        workspace_id: source_event.workspace_id.clone(),
        conversation_id: message.conversation_id.clone(),
        message_id: message.message_id.clone(),
        schema_version: 1,
        renderer_contract_version: 1,
        semantic_content_digest: semantic_content_digest.clone(),
        document_ref: BlobRef {
            digest: document_digest.clone(),
            size_bytes: document_size_bytes,
            media_type: RICH_PRESENTATION_MEDIA_TYPE.to_owned(),
        },
        document_digest: document_digest.clone(),
        document_size_bytes,
        producer_agent_session_id: Some(producer_agent_session_id.to_owned()),
        host_instruction_digest: None,
        host_skill_refs: Vec::new(),
        created_at: message.created_at.clone(),
        version: 1,
    };
    let event = EventDraft {
        event_id,
        workspace_id: record.workspace_id.clone(),
        entity_type: "RichPresentation".to_owned(),
        entity_id: presentation_id,
        origin_runtime_id: source_event.origin_runtime_id.clone(),
        entity_revision: 1,
        hlc_timestamp: source_event.hlc_timestamp.clone(),
        correlation_id: source_event.correlation_id.clone(),
        causation_id: Some(source_event.event_id.clone()),
        schema_version: 1,
        event_type: "rich.presentation.published.v1".to_owned(),
        payload: json!({
            "presentation_id": record.presentation_id,
            "conversation_id": record.conversation_id,
            "message_id": record.message_id,
            "schema_version": record.schema_version,
            "renderer_contract_version": record.renderer_contract_version,
            "semantic_content_digest": record.semantic_content_digest,
            "document_digest": record.document_digest,
            "document_size_bytes": record.document_size_bytes,
            "host_skill_refs": record.host_skill_refs,
            "aggregate_version": 1,
        }),
        recorded_at: record.created_at.clone(),
    };
    Ok(Some(PublishRichPresentation {
        principal_id: principal_id.to_owned(),
        presentation: record,
        canonical_document,
        event,
    }))
}

fn source_bound_blocks(
    semantic_text: &str,
    tables: &[domain_presentation::SemanticMarkdownTable],
) -> Result<(Vec<Value>, Vec<Value>), ()> {
    let mut blocks = Vec::new();
    let mut provenance = Vec::new();
    let mut cursor = 0usize;
    for table in tables {
        if table.start_utf8_byte < cursor
            || table.end_utf8_byte_exclusive <= table.start_utf8_byte
            || table.end_utf8_byte_exclusive > semantic_text.len()
            || !semantic_text.is_char_boundary(table.start_utf8_byte)
            || !semantic_text.is_char_boundary(table.end_utf8_byte_exclusive)
        {
            return Err(());
        }
        if cursor < table.start_utf8_byte {
            push_text_slice(
                semantic_text,
                cursor,
                table.start_utf8_byte,
                &mut blocks,
                &mut provenance,
            );
        }
        blocks.push(json!({
            "kind": "TABLE",
            "headers": table.headers,
            "rows": table.rows,
        }));
        provenance.push(semantic_provenance(blocks.len() - 1));
        cursor = table.end_utf8_byte_exclusive;
    }
    if cursor < semantic_text.len() {
        push_text_slice(
            semantic_text,
            cursor,
            semantic_text.len(),
            &mut blocks,
            &mut provenance,
        );
    }
    Ok((blocks, provenance))
}

fn push_text_slice(
    semantic_text: &str,
    start: usize,
    end: usize,
    blocks: &mut Vec<Value>,
    provenance: &mut Vec<Value>,
) {
    if start >= end {
        return;
    }
    blocks.push(json!({
        "kind": "TEXT_SLICE",
        "source": {
            "start_utf8_byte": start,
            "end_utf8_byte_exclusive": end,
            "slice_digest": sha256_digest(&semantic_text.as_bytes()[start..end]),
        }
    }));
    provenance.push(semantic_provenance(blocks.len() - 1));
}

fn semantic_provenance(index: usize) -> Value {
    json!({
        "block_path": format!("/{index}"),
        "origin": "SEMANTIC_MESSAGE",
        "resource_refs": [],
        "artifact_refs": [],
        "evidence_refs": [],
        "verification_refs": [],
    })
}

fn digest_json(value: &Value) -> Result<String, ()> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|_| ())?;
    Ok(sha256_digest(&bytes))
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;
    use storage_core::{AggregateStateRef, BlobRef};

    const AT: &str = "2026-11-25T12:00:00.000000000Z";

    fn source() -> (ConversationMessageRecord, DomainEvent) {
        let text = "Revenue snapshot 🌍\n\n| Month | Revenue |\n| --- | --- |\n| Jan | 12 |\n| Feb | 18 |\n\nSource: committed rows.";
        let message = ConversationMessageRecord {
            message_id: "message-binder".into(),
            conversation_id: "conversation-binder".into(),
            author: json!({"kind":"AGENT","agent_binding_id":"binding"}),
            role: "AGENT".into(),
            agent_session_id: Some("session-binder".into()),
            agent_binding_id: Some("binding".into()),
            turn_id: Some("turn-binder".into()),
            content: json!([{"kind":"TEXT","text":text}]),
            resource_refs: json!([]),
            source_channel_ref: None,
            created_at: AT.into(),
        };
        let event = DomainEvent {
            event_id: "event-message-binder".into(),
            workspace_id: "workspace-binder".into(),
            entity_type: "Conversation".into(),
            entity_id: "conversation-binder".into(),
            origin_runtime_id: "runtime-binder".into(),
            origin_sequence: 1,
            entity_revision: 1,
            hlc_timestamp: AT.into(),
            correlation_id: "correlation-binder".into(),
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.message.added.v1".into(),
            payload: json!({"message_id":"message-binder"}),
            aggregate_state_ref: AggregateStateRef {
                blob: BlobRef {
                    digest:
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .into(),
                    size_bytes: 1,
                    media_type: "application/json".into(),
                },
                entity_revision: 1,
                record_schema_version: 1,
            },
            recorded_at: AT.into(),
            payload_digest:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
        };
        (message, event)
    }

    fn candidate() -> PublishRichPresentation {
        let (message, event) = source();
        build_publish_request("owner-binder", &message, "session-binder", &event)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn derives_exact_table_and_retains_surrounding_committed_text() {
        let request = candidate();
        let document: Value = serde_json::from_slice(&request.canonical_document).unwrap();
        let roots = document["root_blocks"].as_array().unwrap();
        assert_eq!(roots.len(), 3);
        assert_eq!(roots[0]["kind"], "TEXT_SLICE");
        assert_eq!(roots[1]["kind"], "TABLE");
        assert_eq!(roots[1]["headers"], json!(["Month", "Revenue"]));
        assert_eq!(roots[1]["rows"], json!([["Jan", "12"], ["Feb", "18"]]));
        assert_eq!(roots[2]["kind"], "TEXT_SLICE");
        assert_eq!(document["renderer_contract_version"], 1);
        assert!(roots.iter().all(|block| block["kind"] != "CHART"));
        assert!(
            request
                .event
                .event_id
                .starts_with("event-rich-presentation-")
        );
    }

    #[test]
    fn publish_failure_is_counted_and_does_not_expose_semantic_text() {
        let observed = Arc::new(Mutex::new(None));
        let capture = Arc::clone(&observed);
        let binder = PresentationBinder::with_publisher(move |request| {
            *capture.lock().unwrap() = Some(request.presentation.message_id.clone());
            Err(StoreError::NotFound)
        });
        assert!(binder.publish_request(candidate()).is_none());
        assert_eq!(binder.dropped_publications(), 1);
        assert_eq!(*observed.lock().unwrap(), Some("message-binder".into()));
    }

    #[test]
    fn answers_without_supported_tables_do_not_create_or_count_a_presentation() {
        let (mut message, event) = source();
        message.content = json!([{"kind":"TEXT","text":"A complete prose-only answer."}]);
        assert!(
            build_publish_request("owner-binder", &message, "session-binder", &event)
                .unwrap()
                .is_none()
        );
    }
}
