//! Independent immutable presentation publication. Domain validation lives in the pure
//! `domain-presentation` crate; this adapter owns authorization, persistence and recovery.
use super::*;
use std::collections::BTreeSet;
use storage_core::rich_presentation::*;

type SemanticMessageRow = (String, String, String, Option<String>, Option<String>);

#[derive(Clone)]
pub struct SqliteRichPresentationStore {
    store: SqliteWorkspaceStore,
    #[cfg(test)]
    after_origin_frontier: Option<Arc<dyn Fn() + Send + Sync>>,
}
impl SqliteRichPresentationStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self {
            store,
            #[cfg(test)]
            after_origin_frontier: None,
        }
    }

    #[cfg(test)]
    pub(super) fn with_frontier_hook(mut self, hook: impl Fn() + Send + Sync + 'static) -> Self {
        self.after_origin_frontier = Some(Arc::new(hook));
        self
    }
    fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let (send, recv) = mpsc::channel();
        self.store.execute_command(
            Command::Operation {
                operation: Box::new(move |c| {
                    let _ = send.send(f(c));
                }),
            },
            recv,
        )
    }
}
fn invalid(message: &str) -> StoreError {
    StoreError::Invalid(message.into())
}
fn authorize(
    c: &Connection,
    principal: &str,
    workspace: &str,
    require_active: bool,
) -> Result<(), StoreError> {
    let allowed:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND (?3=0 OR status='ACTIVE'))",params![workspace,principal,require_active],|r|r.get(0)).map_err(map_database_error)?;
    if allowed {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}
fn semantic(
    c: &Connection,
    p: &RichPresentationRecord,
) -> Result<(String, String, Option<String>), StoreError> {
    let row: Option<SemanticMessageRow> = c.query_row(
      "SELECT m.role,m.content_json,m.resource_refs_json,m.turn_id,m.agent_session_id FROM conversation_messages m JOIN conversations c ON c.conversation_id=m.conversation_id WHERE m.message_id=?1 AND m.conversation_id=?2 AND c.workspace_id=?3",
      params![p.message_id,p.conversation_id,p.workspace_id], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(map_database_error)?;
    let Some((role, content, refs, turn, session)) = row else {
        return Err(StoreError::NotFound);
    };
    if role != "AGENT" || turn.as_ref().is_none_or(|s| s.is_empty()) {
        return Err(invalid(
            "presentation requires a committed AGENT turn message",
        ));
    }
    let content: Value = serde_json::from_str(&content)
        .map_err(|_| StoreError::Integrity("invalid message content".into()))?;
    let refs: Value = serde_json::from_str(&refs)
        .map_err(|_| StoreError::Integrity("invalid message resources".into()))?;
    let blocks = content
        .as_array()
        .ok_or_else(|| StoreError::Integrity("message content is not an array".into()))?;
    if !refs.is_array() {
        return Err(StoreError::Integrity(
            "message resources are not an array".into(),
        ));
    }
    let text = blocks
        .iter()
        .filter(|v| v["kind"] == "TEXT")
        .map(|v| {
            v["text"]
                .as_str()
                .ok_or_else(|| StoreError::Integrity("invalid message text".into()))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    let digest = digest(&canonical_json(
        &json!({"content":content,"resource_refs":refs}),
    )?);
    Ok((digest, text, session))
}
fn validate_metadata(p: &RichPresentationRecord) -> Result<(), StoreError> {
    for id in [
        &p.presentation_id,
        &p.workspace_id,
        &p.conversation_id,
        &p.message_id,
    ] {
        if id.trim().is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
            return Err(invalid("invalid presentation identity"));
        }
    }
    if p.schema_version != 1 || !matches!(p.renderer_contract_version, 1 | 2) || p.version != 1 {
        return Err(invalid("unsupported presentation version"));
    }
    if p.document_size_bytes == 0
        || p.document_size_bytes > MAX_RICH_PRESENTATION_BYTES
        || p.document_ref.size_bytes != p.document_size_bytes
        || p.document_ref.digest != p.document_digest
        || p.document_ref.media_type != RICH_PRESENTATION_MEDIA_TYPE
        || !storage_core::is_sha256_digest(&p.document_digest)
        || !storage_core::is_sha256_digest(&p.semantic_content_digest)
        || p.host_instruction_digest
            .as_ref()
            .is_some_and(|d| !storage_core::is_sha256_digest(d))
    {
        return Err(invalid("invalid presentation digest, BlobRef or size"));
    }
    if !p.host_skill_refs.is_empty() || p.host_instruction_digest.is_some() {
        return Err(invalid(
            "host-guidance provenance requires a qualified registry",
        ));
    }
    if canonicalize_utc_timestamp(&p.created_at)? != p.created_at {
        return Err(invalid("presentation timestamp must be canonical UTC"));
    }
    Ok(())
}
fn validate_document(
    p: &RichPresentationRecord,
    bytes: &[u8],
    text: &str,
) -> Result<(), StoreError> {
    domain_presentation::validate_document(
        domain_presentation::ValidationContext {
            schema_version: p.schema_version,
            renderer_contract_version: p.renderer_contract_version,
            presentation_id: &p.presentation_id,
            message_id: &p.message_id,
            semantic_content_digest: &p.semantic_content_digest,
            document_digest: &p.document_digest,
            document_size_bytes: p.document_size_bytes,
            semantic_text: text,
        },
        bytes,
    )
    .map_err(|error| match error {
        domain_presentation::ValidationError::Invalid(message) => {
            StoreError::Invalid(message.into())
        }
        domain_presentation::ValidationError::Integrity(message) => {
            StoreError::Integrity(message.into())
        }
        domain_presentation::ValidationError::UnsupportedBlock => {
            StoreError::Invalid("unsupported presentation block".into())
        }
    })
}

fn expected_payload(p: &RichPresentationRecord) -> Value {
    json!({"presentation_id":p.presentation_id,"conversation_id":p.conversation_id,"message_id":p.message_id,"schema_version":p.schema_version,"renderer_contract_version":p.renderer_contract_version,"semantic_content_digest":p.semantic_content_digest,"document_digest":p.document_digest,"document_size_bytes":p.document_size_bytes,"host_skill_refs":p.host_skill_refs,"aggregate_version":1})
}
fn read_record(
    c: &Connection,
    workspace: &str,
    id: &str,
) -> Result<Option<RichPresentationRecord>, StoreError> {
    let json:Option<String>=c.query_row("SELECT json_object('presentation_id',presentation_id,'workspace_id',workspace_id,'conversation_id',conversation_id,'message_id',message_id,'schema_version',schema_version,'renderer_contract_version',renderer_contract_version,'semantic_content_digest',semantic_content_digest,'document_ref',json(document_ref_json),'document_digest',document_digest,'document_size_bytes',document_size_bytes,'producer_agent_session_id',producer_agent_session_id,'host_instruction_digest',host_instruction_digest,'host_skill_refs',json(host_skill_refs_json),'created_at',created_at,'version',version) FROM rich_presentations WHERE workspace_id=?1 AND presentation_id=?2",params![workspace,id],|r|r.get(0)).optional().map_err(map_database_error)?;
    json.map(|s| {
        serde_json::from_str(&s)
            .map_err(|_| StoreError::Integrity("invalid presentation metadata".into()))
    })
    .transpose()
}

fn read_record_by_message(
    c: &Connection,
    workspace: &str,
    message_id: &str,
) -> Result<Option<RichPresentationRecord>, StoreError> {
    let encoded: Option<String> = c
        .query_row(
            "SELECT json_object('presentation_id',presentation_id,'workspace_id',workspace_id,'conversation_id',conversation_id,'message_id',message_id,'schema_version',schema_version,'renderer_contract_version',renderer_contract_version,'semantic_content_digest',semantic_content_digest,'document_ref',json(document_ref_json),'document_digest',document_digest,'document_size_bytes',document_size_bytes,'producer_agent_session_id',producer_agent_session_id,'host_instruction_digest',host_instruction_digest,'host_skill_refs',json(host_skill_refs_json),'created_at',created_at,'version',version) FROM rich_presentations WHERE workspace_id=?1 AND message_id=?2",
            params![workspace, message_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    encoded
        .map(|value| {
            serde_json::from_str(&value)
                .map_err(|_| StoreError::Integrity("invalid presentation metadata".into()))
        })
        .transpose()
}

fn read_publication_event(
    c: &Connection,
    workspace: &str,
    event_id: &str,
) -> Result<Option<DomainEvent>, StoreError> {
    type Row = (
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        String,
        String,
        Option<String>,
        i64,
        String,
        String,
        String,
        String,
        String,
    );
    let row: Option<Row> = c
        .query_row(
            "SELECT event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,causation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest FROM domain_events WHERE workspace_id=?1 AND event_id=?2",
            params![workspace, event_id],
            |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                    row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?,
                    row.get(10)?, row.get(11)?, row.get(12)?, row.get(13)?, row.get(14)?,
                    row.get(15)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    row.map(|row| {
        Ok(DomainEvent {
            event_id: row.0,
            workspace_id: row.1,
            entity_type: row.2,
            entity_id: row.3,
            origin_runtime_id: row.4,
            origin_sequence: from_sql_i64(row.5, "origin sequence")?,
            entity_revision: from_sql_i64(row.6, "event revision")?,
            hlc_timestamp: row.7,
            correlation_id: row.8,
            causation_id: row.9,
            schema_version: u32::try_from(row.10)
                .map_err(|_| StoreError::Integrity("event schema version is invalid".into()))?,
            event_type: row.11,
            payload: serde_json::from_str(&row.12)
                .map_err(|_| StoreError::Integrity("invalid event payload".into()))?,
            aggregate_state_ref: serde_json::from_str(&row.13)
                .map_err(|_| StoreError::Integrity("invalid aggregate state ref".into()))?,
            recorded_at: row.14,
            payload_digest: row.15,
        })
    })
    .transpose()
}

fn event_matches_draft(event: &DomainEvent, draft: &EventDraft) -> bool {
    event.event_id == draft.event_id
        && event.workspace_id == draft.workspace_id
        && event.entity_type == draft.entity_type
        && event.entity_id == draft.entity_id
        && event.origin_runtime_id == draft.origin_runtime_id
        && event.entity_revision == draft.entity_revision
        && event.hlc_timestamp == draft.hlc_timestamp
        && event.correlation_id == draft.correlation_id
        && event.causation_id == draft.causation_id
        && event.schema_version == draft.schema_version
        && event.event_type == draft.event_type
        && event.payload == draft.payload
        && event.recorded_at == draft.recorded_at
}

fn replay_publication(
    tx: &Transaction<'_>,
    blobs: &dyn BlobStore,
    request: &PublishRichPresentation,
) -> Result<Option<CommittedRichPresentation>, StoreError> {
    let p = &request.presentation;
    let collision: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM rich_presentations WHERE presentation_id=?1 OR message_id=?2)",
            params![p.presentation_id, p.message_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !collision {
        return Ok(None);
    }

    let by_id = read_record(tx, &p.workspace_id, &p.presentation_id)?;
    let by_message = read_record_by_message(tx, &p.workspace_id, &p.message_id)?;
    if by_id.as_ref() != Some(p) || by_message.as_ref() != Some(p) {
        return Err(invalid("presentation identity unavailable"));
    }

    let stored_bytes = blobs.get(
        &p.workspace_id,
        BlobPurpose::RichPresentation,
        &p.document_ref,
    )?;
    if stored_bytes != request.canonical_document {
        return Err(invalid(
            "presentation replay conflicts with stored document",
        ));
    }
    let (semantic_digest, text, session) = semantic(tx, p)?;
    if semantic_digest != p.semantic_content_digest
        || p.producer_agent_session_id
            .as_ref()
            .is_some_and(|expected| Some(expected) != session.as_ref())
    {
        return Err(StoreError::Integrity(
            "stored presentation semantic binding mismatch".into(),
        ));
    }
    validate_document(p, &stored_bytes, &text)?;

    let event = read_publication_event(tx, &p.workspace_id, &request.event.event_id)?
        .ok_or_else(|| StoreError::Integrity("publication replay event is missing".into()))?;
    if !event_matches_draft(&event, &request.event) {
        return Err(invalid("presentation replay conflicts with stored event"));
    }
    Ok(Some(CommittedRichPresentation {
        presentation: p.clone(),
        event,
    }))
}

impl RichPresentationStore for SqliteRichPresentationStore {
    fn publish_rich_presentation(
        &self,
        request: PublishRichPresentation,
    ) -> Result<CommittedRichPresentation, StoreError> {
        validate_metadata(&request.presentation)?;
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |c| {
            let tx=c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_database_error)?;
            let p=&request.presentation;
            authorize(&tx,&request.principal_id,&p.workspace_id,false)?;
            let (semantic_digest,text,session)=semantic(&tx,p)?;
            if semantic_digest!=p.semantic_content_digest || p.producer_agent_session_id.as_ref().is_some_and(|s|Some(s)!=session.as_ref()) {return Err(StoreError::Integrity("presentation semantic/producer binding mismatch".into()))}
            validate_document(p,&request.canonical_document,&text)?;
            let event=&request.event;
            if event.workspace_id!=p.workspace_id || event.entity_id!=p.presentation_id || event.entity_type!="RichPresentation" || event.entity_revision!=1 || event.schema_version!=1 || event.event_type!="rich.presentation.published.v1" || event.recorded_at!=p.created_at || event.payload!=expected_payload(&p) {return Err(invalid("invalid presentation publication event"))}
            if let Some(committed) = replay_publication(&tx, blobs.as_ref(), &request)? {
                tx.commit().map_err(map_database_error)?;
                return Ok(committed);
            }
            authorize(&tx,&request.principal_id,&p.workspace_id,true)?;
            let blob=blobs.put(&p.workspace_id,BlobPurpose::RichPresentation,&request.canonical_document,RICH_PRESENTATION_MEDIA_TYPE)?;
            if blob!=p.document_ref || blobs.get(&p.workspace_id,BlobPurpose::RichPresentation,&blob)?!=request.canonical_document {return Err(StoreError::Integrity("presentation blob verification failed".into()))}
            tx.execute("INSERT INTO rich_presentations(presentation_id,workspace_id,conversation_id,message_id,schema_version,renderer_contract_version,semantic_content_digest,document_ref_json,document_digest,document_size_bytes,producer_agent_session_id,host_instruction_digest,host_skill_refs_json,created_at,version) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,1)",params![p.presentation_id,p.workspace_id,p.conversation_id,p.message_id,p.schema_version,p.renderer_contract_version,p.semantic_content_digest,String::from_utf8(canonical_json(&p.document_ref)?).map_err(|_|invalid("invalid blob JSON"))?,p.document_digest,to_sql_i64(p.document_size_bytes,"presentation size")?,p.producer_agent_session_id,p.host_instruction_digest,String::from_utf8(canonical_json(&p.host_skill_refs)?).map_err(|_|invalid("invalid skills JSON"))?,p.created_at]).map_err(map_database_error)?;
            let state_bytes=canonical_json(p)?;
            let state_blob=blobs.put(&p.workspace_id,BlobPurpose::AggregateState,&state_bytes,"application/vnd.litecowork.rich-presentation-state+json")?;
            if state_blob.digest!=digest(&state_bytes) || state_blob.size_bytes!=state_bytes.len() as u64 || blobs.get(&p.workspace_id,BlobPurpose::AggregateState,&state_blob)?!=state_bytes {return Err(StoreError::Integrity("presentation aggregate blob verification failed".into()))}
            let event=insert_domain_event(&tx,event,AggregateStateRef{blob:state_blob,entity_revision:1,record_schema_version:1})?;
            tx.commit().map_err(map_database_error)?;
            Ok(CommittedRichPresentation{presentation:p.clone(),event})
        })
    }
    fn read_rich_presentation(
        &self,
        principal_id: &str,
        workspace_id: &str,
        presentation_id: &str,
    ) -> Result<Option<StoredRichPresentation>, StoreError> {
        let (principal, workspace, id) = (
            principal_id.to_owned(),
            workspace_id.to_owned(),
            presentation_id.to_owned(),
        );
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |c| {
            let tx = c.transaction().map_err(map_database_error)?;
            authorize(&tx, &principal, &workspace, false)?;
            let Some(p) = read_record(&tx, &workspace, &id)? else {
                return Ok(None);
            };
            validate_metadata(&p)?;
            let (semantic_digest, text, session) = semantic(&tx, &p)?;
            if semantic_digest != p.semantic_content_digest
                || p.producer_agent_session_id
                    .as_ref()
                    .is_some_and(|s| Some(s) != session.as_ref())
            {
                return Err(StoreError::Integrity(
                    "stored presentation semantic binding mismatch".into(),
                ));
            }
            let bytes = blobs.get(&workspace, BlobPurpose::RichPresentation, &p.document_ref)?;
            validate_document(&p, &bytes, &text)?;
            Ok(Some(StoredRichPresentation {
                presentation: p,
                canonical_document: bytes,
            }))
        })
    }

    fn read_conversation_presentation(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        origin_runtime_id: &str,
        after_message_id: Option<&str>,
        limit: usize,
    ) -> Result<ConversationPresentationSnapshot, StoreError> {
        let reads = [ConversationPresentationRead {
            conversation_id: conversation_id.to_owned(),
            after_message_id: after_message_id.map(str::to_owned),
        }];
        self.read_conversation_presentation_batch(
            principal_id,
            workspace_id,
            &reads,
            origin_runtime_id,
            limit,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| StoreError::Integrity("Conversation presentation batch was empty".into()))
    }

    fn read_conversation_presentation_batch(
        &self,
        principal_id: &str,
        workspace_id: &str,
        reads: &[ConversationPresentationRead],
        origin_runtime_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationPresentationSnapshot>, StoreError> {
        if reads.is_empty() || reads.len() > MAX_CONVERSATION_PRESENTATION_BATCH {
            return Err(invalid("invalid Conversation presentation batch size"));
        }
        if limit == 0 || limit > MAX_CONVERSATION_PRESENTATION_MESSAGES {
            return Err(invalid("invalid Conversation presentation page size"));
        }
        let (principal, workspace, origin_runtime) = (
            principal_id.to_owned(),
            workspace_id.to_owned(),
            origin_runtime_id.to_owned(),
        );
        validate_nonempty(&[&principal, &workspace, &origin_runtime])?;
        let reads = reads.to_vec();
        let mut seen = BTreeSet::new();
        for read in &reads {
            validate_nonempty(&[&read.conversation_id])?;
            if !seen.insert(read.conversation_id.clone()) {
                return Err(invalid("duplicate Conversation presentation filter"));
            }
        }
        #[cfg(test)]
        let after_origin_frontier = self.after_origin_frontier.clone();
        self.run(move |c| {
            let tx = c.transaction().map_err(map_database_error)?;
            authorize(&tx, &principal, &workspace, false)?;
            let origin_sequence =
                super::read_workspace_origin_frontier(&tx, &workspace, &origin_runtime)?;
            #[cfg(test)]
            if let Some(hook) = &after_origin_frontier {
                hook();
            }
            reads
                .iter()
                .map(|read| {
                    read_conversation_presentation_in_transaction(
                        &tx,
                        &workspace,
                        &read.conversation_id,
                        read.after_message_id.as_deref(),
                        origin_sequence,
                        limit,
                    )
                })
                .collect()
        })
    }
}

fn read_conversation_presentation_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    workspace: &str,
    conversation: &str,
    cursor: Option<&str>,
    origin_sequence: u64,
    limit: usize,
) -> Result<ConversationPresentationSnapshot, StoreError> {
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE workspace_id=?1 AND conversation_id=?2)",
            params![workspace, conversation],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !exists {
        return Err(StoreError::NotFound);
    }
    let stream_message_frontier: Option<String> = tx
        .query_row(
            "SELECT message_id FROM conversation_messages WHERE conversation_id=?1 ORDER BY created_at DESC,message_id DESC LIMIT 1",
            params![conversation],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    if let Some(cursor_id) = cursor {
        let cursor_exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2)",
                params![conversation, cursor_id],
                |row| row.get(0),
            )
            .map_err(map_database_error)?;
        if !cursor_exists {
            return Err(invalid("invalid Conversation presentation cursor"));
        }
    }
    let projection_revision: i64 = tx
        .query_row(
            "SELECT MAX(1, (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id=?1) + (SELECT COUNT(*) FROM rich_presentations WHERE workspace_id=?2 AND conversation_id=?1) + (SELECT COALESCE(SUM(version),0) FROM conversation_turns WHERE conversation_id=?1))",
            params![conversation, workspace],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let mut statement = tx
        .prepare(
            "SELECT json_object('message_id',m.message_id,'conversation_id',m.conversation_id,'author',json(m.author_json),'role',m.role,'agent_session_id',m.agent_session_id,'agent_binding_id',m.agent_binding_id,'turn_id',m.turn_id,'content',json(m.content_json),'resource_refs',json(m.resource_refs_json),'created_at',m.created_at), m.message_id, m.role, r.presentation_id, r.schema_version, r.renderer_contract_version, r.semantic_content_digest, r.document_digest, r.document_size_bytes, t.status FROM conversation_messages m LEFT JOIN rich_presentations r ON r.message_id=m.message_id AND r.workspace_id=?1 AND r.conversation_id=m.conversation_id LEFT JOIN conversation_turns t ON t.turn_id=m.turn_id AND t.conversation_id=m.conversation_id WHERE m.conversation_id=?2 AND (?3 IS NULL OR (m.created_at,m.message_id) > (SELECT created_at,message_id FROM conversation_messages WHERE conversation_id=?2 AND message_id=?3)) ORDER BY m.created_at,m.message_id LIMIT ?4",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![workspace, conversation, cursor, (limit + 1) as i64],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .map_err(map_database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)?;
    drop(statement);
    let mut rows = rows;
    let has_more = rows.len() > limit;
    if has_more {
        rows.pop();
    }
    let next_cursor = has_more
        .then(|| rows.last().map(|row| row.1.clone()))
        .flatten();
    let mut items = Vec::with_capacity(rows.len());
    let mut page_bytes = 0usize;
    for (
        message_json,
        message_id,
        role,
        presentation_id,
        schema_version,
        renderer_contract_version,
        semantic_digest,
        document_digest,
        document_size_bytes,
        turn_status,
    ) in rows
    {
        if message_json.len() > MAX_CONVERSATION_PRESENTATION_MESSAGE_BYTES {
            return Err(invalid(
                "Conversation message exceeds presentation read limit",
            ));
        }
        page_bytes = page_bytes.saturating_add(message_json.len());
        if page_bytes > MAX_CONVERSATION_PRESENTATION_PAGE_BYTES {
            return Err(invalid("Conversation presentation page exceeds read limit"));
        }
        let message: Value = serde_json::from_str(&message_json)
            .map_err(|_| StoreError::Integrity("invalid Conversation message".into()))?;
        let rich_presentation = match (
            presentation_id,
            schema_version,
            renderer_contract_version,
            semantic_digest,
            document_digest,
            document_size_bytes,
        ) {
            (
                Some(id),
                Some(schema),
                Some(renderer),
                Some(semantic),
                Some(document),
                Some(size),
            ) if role == "AGENT" => Some(json!({
                "presentation_id": id,
                "message_id": message_id,
                "schema_version": schema,
                "renderer_contract_version": renderer,
                "semantic_content_digest": semantic,
                "document_digest": document,
                "document_size_bytes": size,
                "availability": "AVAILABLE"
            })),
            (None, None, None, None, None, None) => None,
            _ => {
                return Err(StoreError::Integrity(
                    "invalid Conversation presentation reference".into(),
                ));
            }
        };
        items.push(json!({
            "message": message,
            "rich_presentation": rich_presentation,
            "linked_items": [],
            "turn_status": turn_status
        }));
    }
    let active_turn: Option<Value> = tx
        .query_row(
            "SELECT json_object('turn_id',turn_id,'status',status,'version',version,'retry_ordinal',retry_ordinal,'presentation_preference',presentation_preference) FROM conversation_turns WHERE conversation_id=?1 AND status IN ('OPEN','RUNNING','WAITING_USER','WAITING_DEPENDENCY','CANCEL_REQUESTED') ORDER BY created_at DESC,turn_id DESC LIMIT 1",
            params![conversation],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(map_database_error)?
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|_| StoreError::Integrity("invalid active Conversation turn".into()))?;
    let retryable_turn: Option<Value> = tx
        .query_row(
            "SELECT json_object('turn_id',t.turn_id,'version',t.version,'retry_ordinal',t.retry_ordinal,'reason_code',(SELECT json_extract(e.payload_json,'$.reason_code') FROM domain_events e WHERE e.entity_type='ConversationTurn' AND e.entity_id=t.turn_id AND e.type='conversation.turn.settled.v1' ORDER BY e.entity_revision DESC LIMIT 1)) FROM conversation_turns t WHERE t.conversation_id=?1 AND t.status='FAILED' AND t.turn_id=(SELECT turn_id FROM conversation_turns WHERE conversation_id=?1 ORDER BY created_at DESC,turn_id DESC LIMIT 1)",
            params![conversation],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(map_database_error)?
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|_| StoreError::Integrity("invalid retryable Conversation turn".into()))?;
    let activity_turn: Option<Value> = tx
        .query_row(
            "SELECT json_object('turn_id',turn_id,'retry_ordinal',retry_ordinal,'status',status) FROM conversation_turns WHERE conversation_id=?1 ORDER BY created_at DESC,turn_id DESC LIMIT 1",
            params![conversation],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(map_database_error)?
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|_| StoreError::Integrity("invalid latest Conversation activity turn".into()))?;
    Ok(ConversationPresentationSnapshot {
        workspace_id: workspace.to_owned(),
        conversation_id: conversation.to_owned(),
        projection_revision: projection_revision.max(1) as u64,
        stream_cursor: format!("conversation:{conversation}:{projection_revision}"),
        origin_sequence: origin_sequence.to_string(),
        stream_message_frontier,
        items,
        next_cursor,
        active_turn,
        retryable_turn,
        activity_turn,
    })
}
