//! Independent immutable presentation publication. This adapter intentionally qualifies
//! only text slices/layout/dividers; source-bearing blocks await their trusted binders.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
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
        if let Some(v) = doc.get(key)
            && !v.as_array().is_some_and(Vec::is_empty)
        {
            return Err(invalid("presentation source/action binders unavailable"));
        }
    }
    if let Some(v) = doc.get("accessibility_summary")
        && !v
            .as_str()
            .is_some_and(|s| within_code_units(s, 4_000) && semantic_line_matches(text, s))
    {
        return Err(invalid("invalid accessibility summary"));
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
/// One verified `source` text slice. The publisher supplies a UTF-8 byte range and the
/// digest of exactly those bytes; storage recomputes both, so a slice can only repeat
/// bytes the committed semantic answer already contains.
fn validate_text_source(source: &Value, text: &str) -> Result<(), StoreError> {
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
    Ok(())
}
/// The desktop validator bounds these strings by UTF-16 code units (`String.length`).
/// Counting code units here, not Unicode scalars, is what keeps both sides accepting and
/// refusing the same documents for text outside the BMP.
fn within_code_units(value: &str, max: usize) -> bool {
    value.encode_utf16().count() <= max && !value.contains('\0')
}
fn markdown_heading_stripped(line: &str) -> &str {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    match line.as_bytes().get(hashes) {
        Some(b' ' | b'\t') if (1..=6).contains(&hashes) => line[hashes..].trim(),
        _ => line,
    }
}
fn markdown_emphasis_inner(line: &str) -> Option<&str> {
    for delimiter in ["**", "__", "*", "_"] {
        if let Some(rest) = line.strip_prefix(delimiter)
            && let Some(inner) = rest.strip_suffix(delimiter)
            && !inner.trim().is_empty()
        {
            return Some(inner.trim());
        }
    }
    None
}
/// A presentation may only display text the committed semantic answer already contains.
/// This mirrors the desktop validator exactly, including one heading and one emphasis
/// layer, so a title or summary that exists only in the presentation fails on both sides.
fn semantic_line_matches(semantic_text: &str, candidate: &str) -> bool {
    let normalized = semantic_text.replace("\r\n", "\n").replace('\r', "\n");
    normalized.split('\n').any(|source_line| {
        let line = markdown_heading_stripped(source_line.trim());
        markdown_emphasis_inner(line).unwrap_or(line) == candidate
    })
}
fn markdown_pipe_row(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }
    let inner = trimmed
        .strip_prefix('|')
        .and_then(|rest| rest.strip_suffix('|'))
        .unwrap_or("");
    let cells: Vec<&str> = inner.split('|').map(str::trim).collect();
    // An escaped pipe needs a fuller Markdown parser; fail closed instead of guessing.
    if cells.iter().any(|cell| cell.contains('\\')) {
        return None;
    }
    Some(cells)
}
fn is_markdown_separator(cell: &str) -> bool {
    let body = cell.strip_prefix(':').unwrap_or(cell);
    let body = body.strip_suffix(':').unwrap_or(body);
    body.len() >= 3 && body.bytes().all(|b| b == b'-')
}
fn markdown_cell_text(cell: &Value) -> String {
    match cell {
        Value::Null => "—".to_owned(),
        Value::Bool(b) => b.to_string(),
        // The desktop compares `String(cell)`, so numbers are rendered with ECMAScript
        // notation. Rust's own float formatting uses different notation outside
        // ECMAScript's fixed range (`1e-7`, not `0.0000001`), which would refuse a table
        // the desktop renders; `es_number_string` reproduces the desktop's digits.
        Value::Number(n) => n
            .as_f64()
            .and_then(es_number_string)
            .unwrap_or_else(|| n.to_string()),
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => cell.to_string(),
    }
}
/// ECMAScript `Number::toString` (ECMA-262 §6.1.6.1.20) for a finite value, which is what
/// the desktop's `String(cell)` yields for a TABLE cell. Rust renders the same shortest
/// round-tripping digits but chooses its own notation, so the notation is rebuilt here:
/// fixed when the decimal point falls in `-6 < n <= 21`, exponent form otherwise.
fn es_number_string(value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    if value == 0.0 {
        return Some("0".to_owned());
    }
    // `<digits>[.<digits>]e<exp>` with the shortest digits that round-trip.
    let formatted = format!("{:e}", value.abs());
    let (mantissa, exponent) = formatted.split_once('e')?;
    let exponent: i32 = exponent.parse().ok()?;
    let digits: String = mantissa
        .chars()
        .filter(|character| *character != '.')
        .collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    // The value is `0.<digits>` shifted by `n` places; `k` is the digit count.
    let k = digits.len() as i32;
    let n = exponent + 1;
    let mut rendered = String::new();
    if k <= n && n <= 21 {
        rendered.push_str(digits);
        for _ in 0..(n - k) {
            rendered.push('0');
        }
    } else if 0 < n && n <= 21 {
        rendered.push_str(&digits[..n as usize]);
        rendered.push('.');
        rendered.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        rendered.push_str("0.");
        for _ in 0..(-n) {
            rendered.push('0');
        }
        rendered.push_str(digits);
    } else {
        rendered.push_str(&digits[..1]);
        if k > 1 {
            rendered.push('.');
            rendered.push_str(&digits[1..]);
        }
        rendered.push('e');
        rendered.push(if exponent >= 0 { '+' } else { '-' });
        rendered.push_str(&exponent.abs().to_string());
    }
    Some(if value.is_sign_negative() {
        format!("-{rendered}")
    } else {
        rendered
    })
}
/// A TABLE carries no source slice in V1, so storage accepts it only when every header and
/// cell exactly repeats a complete Markdown pipe table of the committed semantic message.
fn table_matches_semantic_markdown(
    semantic_text: &str,
    headers: &[&str],
    rows: &[Vec<Value>],
) -> bool {
    let normalized = semantic_text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    for index in 0..lines.len().saturating_sub(1) {
        let Some(semantic_headers) = markdown_pipe_row(lines[index]) else {
            continue;
        };
        let Some(separator) = markdown_pipe_row(lines[index + 1]) else {
            continue;
        };
        if semantic_headers.len() != headers.len() || separator.len() != headers.len() {
            continue;
        }
        if semantic_headers
            .iter()
            .zip(headers)
            .any(|(cell, header)| cell != header)
        {
            continue;
        }
        if !separator.iter().all(|cell| is_markdown_separator(cell)) {
            continue;
        }
        let mut semantic_rows: Vec<Vec<&str>> = Vec::new();
        let mut row_index = index + 2;
        while row_index < lines.len() {
            let Some(row) = markdown_pipe_row(lines[row_index]) else {
                break;
            };
            if row.len() != headers.len() {
                // The desktop clears what it collected and stops, so a zero-row TABLE still
                // matches a matching header/separator pair. Mirror that exactly rather than
                // failing closed, or storage would refuse a table the desktop renders.
                semantic_rows.clear();
                break;
            }
            semantic_rows.push(row);
            row_index += 1;
        }
        if semantic_rows.len() != rows.len() {
            continue;
        }
        if rows.iter().zip(&semantic_rows).all(|(row, semantic_row)| {
            row.iter()
                .zip(semantic_row)
                .all(|(cell, expected)| *expected == markdown_cell_text(cell))
        }) {
            return true;
        }
    }
    false
}
fn validate_table(b: &Value, text: &str) -> Result<(), StoreError> {
    closed_object(b, &["kind", "headers", "rows"], &["accessible_summary"])?;
    let headers = b["headers"]
        .as_array()
        .ok_or_else(|| invalid("headers array required"))?;
    if headers.is_empty() || headers.len() > 20 {
        return Err(invalid("invalid table header count"));
    }
    for header in headers {
        match header.as_str() {
            Some(header) if within_code_units(header, 300) => {}
            _ => return Err(invalid("invalid table header")),
        }
    }
    if let Some(v) = b.get("accessible_summary") {
        let summary = v.as_str().ok_or_else(|| invalid("invalid table summary"))?;
        if !within_code_units(summary, 2_000) || !semantic_line_matches(text, summary) {
            return Err(invalid("table summary must repeat a semantic line"));
        }
    }
    let rows = b["rows"]
        .as_array()
        .ok_or_else(|| invalid("rows array required"))?;
    if rows.len() > 500 {
        return Err(invalid("invalid table row count"));
    }
    let mut validated = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row.as_array().ok_or_else(|| invalid("invalid table row"))?;
        if row.len() != headers.len() {
            return Err(invalid("table row width mismatch"));
        }
        for cell in row {
            let valid = match cell {
                Value::Null | Value::Bool(_) => true,
                Value::Number(n) => n.as_f64().is_some_and(|f| f.is_finite()),
                Value::String(s) => within_code_units(s, 8_192),
                Value::Array(_) | Value::Object(_) => false,
            };
            if !valid {
                return Err(invalid("invalid table cell"));
            }
        }
        validated.push(row.clone());
    }
    let headers: Vec<&str> = headers
        .iter()
        .map(|h| h.as_str().unwrap_or_default())
        .collect();
    if !table_matches_semantic_markdown(text, &headers, &validated) {
        return Err(invalid(
            "table must repeat a semantic Markdown table exactly",
        ));
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
    if let Some(width) = b.get("width")
        && !matches!(width.as_str(), Some("READABLE" | "WIDE" | "FULL_AVAILABLE"))
    {
        return Err(invalid("invalid width"));
    }
    match b["kind"].as_str() {
        Some("TEXT_SLICE") => {
            closed_object(b, &["kind", "source"], &["width"])?;
            validate_text_source(&b["source"], text)?;
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
        Some("CARD") => {
            closed_object(b, &["kind", "title", "children"], &[])?;
            let title = b["title"]
                .as_str()
                .ok_or_else(|| invalid("invalid card title"))?;
            if !within_code_units(title, 300)
                || title.trim().is_empty()
                || !semantic_line_matches(text, title)
            {
                return Err(invalid("card title must repeat a semantic line"));
            }
            let children = b["children"]
                .as_array()
                .ok_or_else(|| invalid("children array required"))?;
            if children.len() > 20 {
                return Err(invalid("invalid card child count"));
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
        Some("CALLOUT") => {
            closed_object(b, &["kind", "tone", "source", "accessible_summary"], &[])?;
            if !matches!(b["tone"].as_str(), Some("NEUTRAL" | "INFO" | "WARNING")) {
                return Err(invalid("invalid callout tone"));
            }
            let summary = b["accessible_summary"]
                .as_str()
                .ok_or_else(|| invalid("invalid callout summary"))?;
            if !within_code_units(summary, 1_000) || !semantic_line_matches(text, summary) {
                return Err(invalid("callout summary must repeat a semantic line"));
            }
            validate_text_source(&b["source"], text)?;
        }
        Some("TABLE") => validate_table(b, text)?,
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

#[cfg(test)]
mod helper_tests {
    use super::*;

    /// The desktop compares a TABLE cell with `String(cell)`, so these expectations come from
    /// Node rather than from this implementation: `node -e 'console.log(String(x))'` for each
    /// value. Rust's own float formatting differs outside ECMAScript's fixed range (`1e+21`
    /// and `1e-7` are printed without an exponent), and a mismatch here refuses a table the
    /// desktop renders.
    #[test]
    fn es_number_string_matches_ecmascript_number_to_string() {
        for (value, expected) in [
            (0.0_f64, "0"),
            (100.0, "100"),
            (0.1, "0.1"),
            (12.5, "12.5"),
            (-0.5, "-0.5"),
            (1.0 / 3.0, "0.3333333333333333"),
            (0.000_001, "0.000001"),
            (1e-7, "1e-7"),
            (5e-7, "5e-7"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (-1e21, "-1e+21"),
            (1e22, "1e+22"),
            (9_007_199_254_740_991.0, "9007199254740991"),
            (1.234_567_890_123_456_8e22, "1.2345678901234568e+22"),
        ] {
            assert_eq!(
                es_number_string(value).as_deref(),
                Some(expected),
                "{value:e} must render exactly as the desktop's String({value:e})"
            );
        }
        assert_eq!(es_number_string(f64::INFINITY), None);
        assert_eq!(es_number_string(f64::NAN), None);
        // JavaScript renders negative zero as "0", and a canonical document never carries it.
        assert_eq!(es_number_string(-0.0).as_deref(), Some("0"));
    }

    /// The desktop clears the rows it collected when a following pipe row has the wrong width
    /// and then compares counts, so a zero-row TABLE still matches a header/separator pair.
    /// Storage must accept exactly that, or it refuses a table the desktop renders.
    #[test]
    fn table_matching_mirrors_the_desktop_row_collection() {
        let headers = ["Metric", "Value"];
        let two_rows = "| Metric | Value |\n| --- | --- |\n| Cost | 12 |\n| Tiny | 1e-27 |\n";
        assert!(table_matches_semantic_markdown(
            two_rows,
            &headers,
            &[
                vec![json!("Cost"), json!(12)],
                vec![json!("Tiny"), json!(1e-27)],
            ]
        ));
        // A wrong-width row directly after the separator clears the collection, leaving the
        // empty TABLE a match for a document with no rows and no match for one with a row.
        let truncated = "| Metric | Value |\n| --- | --- |\n| only-one-cell |\n";
        assert!(table_matches_semantic_markdown(truncated, &headers, &[]));
        assert!(!table_matches_semantic_markdown(
            truncated,
            &headers,
            &[vec![json!("only-one-cell"), json!("")]]
        ));
        // A non-pipe line ends the table without clearing it, and a shortened table is a
        // different shape rather than a match.
        let one_row = "| Metric | Value |\n| --- | --- |\n| Cost | 12 |\nProse after the table.\n";
        assert!(table_matches_semantic_markdown(
            one_row,
            &headers,
            &[vec![json!("Cost"), json!(12)]]
        ));
        assert!(!table_matches_semantic_markdown(
            one_row,
            &headers,
            &[
                vec![json!("Cost"), json!(12)],
                vec![json!("Cost"), json!(12)]
            ]
        ));
        // Escaped pipes need a fuller Markdown parser, so the row fails closed.
        assert!(!table_matches_semantic_markdown(
            "| Metric | Value |\n| --- | --- |\n| a\\|b | 12 |\n",
            &headers,
            &[vec![json!("a|b"), json!(12)]]
        ));
    }
}
