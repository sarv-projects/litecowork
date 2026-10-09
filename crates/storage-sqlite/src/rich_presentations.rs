//! Independent immutable presentation publication. This adapter intentionally qualifies
//! only text slices/layout/dividers; source-bearing blocks await their trusted binders.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use storage_core::rich_presentation::*;

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;
#[derive(Clone)]
pub struct SqliteRichPresentationStore {
    store: SqliteWorkspaceStore,
}
impl SqliteRichPresentationStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store }
    }
    fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let (send, recv) = mpsc::channel();
        self.store.execute_command(
            Command::RichPresentationOperation {
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
    let row:Option<(String,String,String,Option<String>,Option<String>)>=c.query_row(
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
    if p.schema_version != 1 || p.renderer_contract_version != 1 || p.version != 1 {
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
fn closed_object(v: &Value, required: &[&str], optional: &[&str]) -> Result<(), StoreError> {
    let o = v
        .as_object()
        .ok_or_else(|| invalid("presentation object required"))?;
    if required.iter().any(|k| !o.contains_key(*k))
        || o.keys()
            .any(|k| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(invalid("presentation object has missing/unknown fields"));
    }
    Ok(())
}
fn validate_document(
    p: &RichPresentationRecord,
    bytes: &[u8],
    text: &str,
) -> Result<(), StoreError> {
    if bytes.len() as u64 != p.document_size_bytes || digest(bytes) != p.document_digest {
        return Err(StoreError::Integrity(
            "presentation document digest/length mismatch".into(),
        ));
    }
    let doc: Value =
        serde_json::from_slice(bytes).map_err(|_| invalid("invalid presentation JSON"))?;
    if canonical_json(&doc)? != bytes {
        return Err(invalid(
            "presentation bytes must use RFC 8785 canonical JSON",
        ));
    }
    closed_object(
        &doc,
        &[
            "schema_version",
            "renderer_contract_version",
            "presentation_id",
            "message_id",
            "semantic_content_digest",
            "root_blocks",
            "block_provenance",
        ],
        &["citations", "actions", "accessibility_summary"],
    )?;
    if doc["schema_version"] != 1
        || doc["renderer_contract_version"] != 1
        || doc["presentation_id"] != p.presentation_id
        || doc["message_id"] != p.message_id
        || doc["semantic_content_digest"] != p.semantic_content_digest
    {
        return Err(invalid("presentation document binding/version mismatch"));
    }
    for key in ["citations", "actions"] {
        if let Some(v) = doc.get(key) {
            if !v.as_array().is_some_and(Vec::is_empty) {
                return Err(invalid("presentation source/action binders unavailable"));
            }
        }
    }
    if let Some(v) = doc.get("accessibility_summary") {
        if !v.as_str().is_some_and(|s| s.chars().count() <= 4000) {
            return Err(invalid("invalid accessibility summary"));
        }
    }
    let roots = doc["root_blocks"]
        .as_array()
        .ok_or_else(|| invalid("root blocks array required"))?;
    if roots.len() > 200 {
        return Err(invalid("presentation block limit exceeded"));
    }
    let mut paths = BTreeSet::new();
    for (i, b) in roots.iter().enumerate() {
        validate_block(b, &format!("/{i}"), text, 0, &mut paths)?;
    }
    let entries = doc["block_provenance"]
        .as_array()
        .ok_or_else(|| invalid("block provenance array required"))?;
    let mut provenance = BTreeMap::new();
    if entries.len() > 200 {
        return Err(invalid("presentation provenance limit exceeded"));
    }
    for e in entries {
        closed_object(
            e,
            &[
                "block_path",
                "origin",
                "resource_refs",
                "artifact_refs",
                "evidence_refs",
                "verification_refs",
            ],
            &[],
        )?;
        let path = e["block_path"]
            .as_str()
            .ok_or_else(|| invalid("invalid block provenance path"))?;
        if !paths.contains(path)
            || provenance.insert(path, ()).is_some()
            || !matches!(
                e["origin"].as_str(),
                Some("SEMANTIC_MESSAGE" | "MODEL_INTENT")
            )
        {
            return Err(invalid("invalid/duplicate/untrusted block provenance"));
        }
        for k in [
            "resource_refs",
            "artifact_refs",
            "evidence_refs",
            "verification_refs",
        ] {
            if !e[k].as_array().is_some_and(Vec::is_empty) {
                return Err(invalid(
                    "source-bearing provenance needs a qualified binder",
                ));
            }
        }
    }
    if paths.len() != provenance.len() {
        return Err(invalid("every block requires exactly one provenance entry"));
    }
    Ok(())
}
fn validate_block(
    b: &Value,
    path: &str,
    text: &str,
    depth: usize,
    paths: &mut BTreeSet<String>,
) -> Result<(), StoreError> {
    if depth > 8 || paths.len() >= 200 {
        return Err(invalid("presentation nesting/block limit exceeded"));
    }
    paths.insert(path.to_owned());
    if let Some(width) = b.get("width") {
        if !matches!(width.as_str(), Some("READABLE" | "WIDE" | "FULL_AVAILABLE")) {
            return Err(invalid("invalid width"));
        }
    }
    match b["kind"].as_str() {
        Some("TEXT_SLICE") => {
            closed_object(b, &["kind", "source"], &["width"])?;
            let source = &b["source"];
            closed_object(
                source,
                &["start_utf8_byte", "end_utf8_byte_exclusive", "slice_digest"],
                &[],
            )?;
            let start = source["start_utf8_byte"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| invalid("invalid text slice start"))?;
            let end = source["end_utf8_byte_exclusive"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| invalid("invalid text slice end"))?;
            if start >= end || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                return Err(invalid("text slice exceeds exact UTF-8 boundaries"));
            }
            let slice = text
                .get(start..end)
                .ok_or_else(|| invalid("text slice outside semantic text"))?;
            if source["slice_digest"] != digest(slice.as_bytes()) {
                return Err(StoreError::Integrity(
                    "semantic text slice digest mismatch".into(),
                ));
            }
        }
        Some("LAYOUT") => {
            closed_object(b, &["kind", "layout", "children"], &["width"])?;
            if !matches!(b["layout"].as_str(), Some("STACK" | "ROW" | "GRID")) {
                return Err(invalid("invalid layout"));
            }
            let children = b["children"]
                .as_array()
                .ok_or_else(|| invalid("children array required"))?;
            if children.is_empty() || children.len() > 20 {
                return Err(invalid("invalid layout child count"));
            }
            for (i, child) in children.iter().enumerate() {
                validate_block(
                    child,
                    &format!("{path}/children/{i}"),
                    text,
                    depth + 1,
                    paths,
                )?;
            }
        }
        _ => {
            return Err(invalid(
                "unqualified presentation block; trusted/source binder required",
            ));
        }
    }
    Ok(())
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
impl RichPresentationStore for SqliteRichPresentationStore {
    fn publish_rich_presentation(
        &self,
        request: PublishRichPresentation,
    ) -> Result<CommittedRichPresentation, StoreError> {
        validate_metadata(&request.presentation)?;
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |c| {
            let p=request.presentation;
            let tx=c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_database_error)?;
            authorize(&tx,&request.principal_id,&p.workspace_id,true)?;
            let (semantic_digest,text,session)=semantic(&tx,&p)?;
            if semantic_digest!=p.semantic_content_digest || p.producer_agent_session_id.as_ref().is_some_and(|s|Some(s)!=session.as_ref()) {return Err(StoreError::Integrity("presentation semantic/producer binding mismatch".into()))}
            validate_document(&p,&request.canonical_document,&text)?;
            let event=&request.event;
            if event.workspace_id!=p.workspace_id || event.entity_id!=p.presentation_id || event.entity_type!="RichPresentation" || event.entity_revision!=1 || event.schema_version!=1 || event.event_type!="rich.presentation.published.v1" || event.recorded_at!=p.created_at || event.payload!=expected_payload(&p) {return Err(invalid("invalid presentation publication event"))}
            let duplicate:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM rich_presentations WHERE presentation_id=?1 OR message_id=?2)",params![p.presentation_id,p.message_id],|r|r.get(0)).map_err(map_database_error)?;
            if duplicate {return Err(invalid("presentation identity unavailable"))}
            let blob=blobs.put(&p.workspace_id,BlobPurpose::RichPresentation,&request.canonical_document,RICH_PRESENTATION_MEDIA_TYPE)?;
            if blob!=p.document_ref || blobs.get(&p.workspace_id,BlobPurpose::RichPresentation,&blob)?!=request.canonical_document {return Err(StoreError::Integrity("presentation blob verification failed".into()))}
            tx.execute("INSERT INTO rich_presentations(presentation_id,workspace_id,conversation_id,message_id,schema_version,renderer_contract_version,semantic_content_digest,document_ref_json,document_digest,document_size_bytes,producer_agent_session_id,host_instruction_digest,host_skill_refs_json,created_at,version) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,1)",params![p.presentation_id,p.workspace_id,p.conversation_id,p.message_id,p.schema_version,p.renderer_contract_version,p.semantic_content_digest,String::from_utf8(canonical_json(&p.document_ref)?).map_err(|_|invalid("invalid blob JSON"))?,p.document_digest,to_sql_i64(p.document_size_bytes,"presentation size")?,p.producer_agent_session_id,p.host_instruction_digest,String::from_utf8(canonical_json(&p.host_skill_refs)?).map_err(|_|invalid("invalid skills JSON"))?,p.created_at]).map_err(map_database_error)?;
            let state_bytes=canonical_json(&p)?;
            let state_blob=blobs.put(&p.workspace_id,BlobPurpose::AggregateState,&state_bytes,"application/vnd.litecowork.rich-presentation-state+json")?;
            if state_blob.digest!=digest(&state_bytes) || state_blob.size_bytes!=state_bytes.len() as u64 || blobs.get(&p.workspace_id,BlobPurpose::AggregateState,&state_blob)?!=state_bytes {return Err(StoreError::Integrity("presentation aggregate blob verification failed".into()))}
            let event=insert_domain_event(&tx,event,AggregateStateRef{blob:state_blob,entity_revision:1,record_schema_version:1})?;
            tx.commit().map_err(map_database_error)?;
            Ok(CommittedRichPresentation{presentation:p,event})
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
}
