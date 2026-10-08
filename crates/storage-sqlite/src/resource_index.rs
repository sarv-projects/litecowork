//! Workspace-scoped deterministic lexical index for small plain-text Resources.
//!
//! Extracted text is persisted only as a `RESOURCE_INDEX` encrypted blob. SQLite index
//! rows contain revision metadata and HMAC term tokens, never extracted text or terms.
//! ZIP, PDF, Office, OCR, and semantic retrieval are intentionally not handled here.

use std::collections::BTreeSet;

use sha2::Digest;
use rusqlite::{Connection, params};
use storage_core::{
    BlobPurpose, BlobRef, BlobStore, PreparedResourceTextIndex, ResourceSearchRecord,
    ResourceSummary, ResourceTextIndexSkipReason, ResourceTextSearchRecord, StoreError,
};

pub const MAX_INDEXABLE_RESOURCE_BYTES: u64 = 1_048_576;
pub const MAX_TERM_COUNT: usize = 20_000;
pub const MAX_TERM_CHARS: usize = 128;
pub const MAX_QUERY_TERMS: usize = 32;
pub const MAX_SEARCHABLE_KEY_VERSIONS: usize = 8;
pub const TEXT_PARSER_ID: &str = "litecowork.plain-text.v1";
pub const INDEX_MEDIA_TYPE: &str = "application/vnd.litecowork.resource-index-text; charset=utf-8";

#[derive(Clone, Debug)]
pub(crate) struct IndexCandidate {
    pub record: ResourceSearchRecord,
    pub source_content_digest: String,
    pub extracted_blob: BlobRef,
    pub parser_id: String,
    pub token_key_version: u32,
    pub matched_term_count: u32,
}

/// Insert a prepared projection inside the Resource creation/revision SQL transaction.
/// Call only after the exact Resource and ResourceRevision rows have been inserted.
pub(crate) fn insert_prepared(
    connection: &Connection,
    index: &PreparedResourceTextIndex,
) -> Result<(), StoreError> {
    validate_prepared(index)?;
    connection.execute(
        "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2 AND resource_revision_id = ?3",
        params![index.workspace_id, index.resource_id, index.resource_revision_id],
    ).map_err(map_index_database_error)?;
    connection.execute(
        "INSERT INTO resource_text_indexes(workspace_id, resource_id, resource_revision_id, source_content_digest, extracted_blob_digest, extracted_size_bytes, parser_id, token_key_version, indexed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![index.workspace_id, index.resource_id, index.resource_revision_id, index.source_content_digest, index.extracted_text.digest, to_sql_i64(index.extracted_text.size_bytes)?, index.parser_id, i64::from(index.token_key_version), index.indexed_at],
    ).map_err(map_index_database_error)?;
    for token in &index.term_tokens {
        connection.execute(
            "INSERT INTO resource_text_index_terms(workspace_id, resource_id, resource_revision_id, token_key_version, term_token) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![index.workspace_id, index.resource_id, index.resource_revision_id, i64::from(index.token_key_version), token],
        ).map_err(map_index_database_error)?;
    }
    Ok(())
}

/// Replace (or remove when the current parser declines the format) one current
/// revision's local index. The head/digest/sensitivity guard is rechecked in the same
/// transaction as the projection update so stale source reads cannot win.
pub(crate) fn replace_current(
    connection: &mut Connection,
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
    source_digest: &str,
    index: Option<&PreparedResourceTextIndex>,
) -> Result<bool, StoreError> {
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(map_index_database_error)?;
    let updated = replace_current_in_transaction(
        &tx,
        workspace_id,
        resource_id,
        revision_id,
        source_digest,
        index,
    )?;
    if !updated {
        return Ok(false);
    }
    tx.commit().map_err(map_index_database_error)?;
    Ok(true)
}

/// Apply a projection replacement inside a caller-owned transaction. The caller must
/// commit the transaction together with any request receipt that makes the operation
/// replayable.
pub(crate) fn replace_current_in_transaction(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
    source_digest: &str,
    index: Option<&PreparedResourceTextIndex>,
) -> Result<bool, StoreError> {
    let current: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM resources r JOIN resource_revisions rev ON rev.resource_id = r.resource_id AND rev.resource_revision_id = r.current_revision_id WHERE r.workspace_id = ?1 AND r.resource_id = ?2 AND rev.resource_revision_id = ?3 AND rev.content_digest = ?4 AND (r.context_document_json IS NULL OR json_extract(r.context_document_json, '$.status') = 'ACTIVE'))",
        params![workspace_id, resource_id, revision_id, source_digest],
        |row| row.get(0),
    ).map_err(map_index_database_error)?;
    if !current {
        return Ok(false);
    }
    connection.execute(
        "DELETE FROM resource_text_indexes WHERE workspace_id = ?1 AND resource_id = ?2 AND resource_revision_id = ?3",
        params![workspace_id, resource_id, revision_id],
    ).map_err(map_index_database_error)?;
    if let Some(index) = index {
        if index.workspace_id != workspace_id
            || index.resource_id != resource_id
            || index.resource_revision_id != revision_id
            || index.source_content_digest != source_digest
        {
            return Err(StoreError::Integrity("reindexed snapshot does not match its source revision".to_owned()));
        }
        insert_prepared(connection, index)?;
    }
    Ok(true)
}

pub(crate) fn not_indexable_reason(
    display_name: &str,
    media_type: &str,
    bytes: &[u8],
) -> Option<ResourceTextIndexSkipReason> {
    if !is_allowlisted_text(display_name, media_type) {
        return Some(ResourceTextIndexSkipReason::UnsupportedType);
    }
    if bytes.len() as u64 > MAX_INDEXABLE_RESOURCE_BYTES {
        return Some(ResourceTextIndexSkipReason::OverSizeLimit);
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Some(ResourceTextIndexSkipReason::InvalidUtf8);
    };
    if text.contains('\0')
        || text
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Some(ResourceTextIndexSkipReason::ControlCharacters);
    }
    if unique_terms(text, MAX_TERM_COUNT + 1).len() > MAX_TERM_COUNT {
        return Some(ResourceTextIndexSkipReason::TermLimitExceeded);
    }
    None
}

/// Read the index key versions in use so callers can MAC query terms before opening the
/// SQLite writer/read transaction. This keeps OS credential-store I/O outside SQL.
pub(crate) fn list_key_versions(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<u32>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT token_key_version FROM resource_text_indexes WHERE workspace_id = ?1 ORDER BY token_key_version",
    ).map_err(map_index_database_error)?;
    let rows = statement.query_map([workspace_id], |row| row.get::<_, i64>(0))
        .map_err(map_index_database_error)?;
    rows.map(|row| {
        let value = row.map_err(map_index_database_error)?;
        u32::try_from(value).map_err(|_| StoreError::Integrity("Resource index key version is invalid".to_owned()))
    }).collect()
}

/// Select only exact current revisions in the requested Workspace and only active
/// ContextDocuments. `tokens_by_version` contains HMAC tokens derived before this call.
pub(crate) fn search_candidates(
    connection: &Connection,
    workspace_id: &str,
    tokens_by_version: &[(u32, Vec<String>)],
    kind: Option<&str>,
    freshness: Option<&str>,
    after_created_at: Option<&str>,
    after_resource_id: Option<&str>,
    limit: usize,
) -> Result<Vec<IndexCandidate>, StoreError> {
    if workspace_id.trim().is_empty()
        || !(1..=201).contains(&limit)
        || kind.is_some_and(|value| !matches!(value, "FILE" | "FOLDER" | "ARTIFACT" | "CONNECTOR_OBJECT" | "WEB_RESOURCE" | "OTHER"))
        || freshness.is_some_and(|value| !matches!(value, "CURRENT" | "STALE" | "UNKNOWN" | "UNAVAILABLE"))
        || after_created_at.is_some() != after_resource_id.is_some()
    {
        return Err(StoreError::Invalid("indexed Resource search filters are invalid".to_owned()));
    }
    let mut found = Vec::new();
    for (key_version, terms) in tokens_by_version {
        if terms.is_empty() || terms.len() > MAX_QUERY_TERMS {
            return Err(StoreError::Invalid("indexed Resource search terms are invalid".to_owned()));
        }
        let placeholders = (0..terms.len()).map(|index| format!("?{}", index + 3)).collect::<Vec<_>>().join(", ");
        let freshness_expr = "CASE
            WHEN l.availability IN ('OFFLINE', 'REVOKED', 'UNAVAILABLE') THEN 'UNAVAILABLE'
            WHEN l.availability IN ('PLACEHOLDER', 'UNKNOWN') THEN 'UNKNOWN'
            WHEN l.observed_revision_id = rev.resource_revision_id
             AND l.observed_digest = rev.content_digest THEN 'CURRENT'
            WHEN l.observed_revision_id IS NULL OR l.observed_digest IS NULL THEN 'UNKNOWN'
            ELSE 'STALE' END";
        let sql = format!(
            "WITH matched AS (
               SELECT resource_id, resource_revision_id
               FROM resource_text_index_terms
               WHERE workspace_id = ?1 AND token_key_version = ?2 AND term_token IN ({placeholders})
               GROUP BY resource_id, resource_revision_id
               HAVING COUNT(DISTINCT term_token) = {}
             )
             SELECT r.resource_id, r.workspace_id, rev.resource_revision_id, r.display_name,
                    rev.media_type, rev.content_digest, rev.size_bytes, r.created_at, r.kind,
                    l.location_id, l.availability, l.writable, l.observed_revision_id,
                    l.observed_digest, l.observed_at, l.last_checked_at, {freshness_expr},
                    i.extracted_blob_digest, i.extracted_size_bytes, i.source_content_digest,
                    i.parser_id, i.token_key_version
             FROM matched m
             JOIN resources r ON r.resource_id = m.resource_id AND r.workspace_id = ?1
             JOIN resource_revisions rev ON rev.resource_id = r.resource_id AND rev.resource_revision_id = m.resource_revision_id
             JOIN resource_text_indexes i ON i.workspace_id = r.workspace_id AND i.resource_id = r.resource_id AND i.resource_revision_id = rev.resource_revision_id
             JOIN resource_locations l ON l.location_id = (
               SELECT candidate.location_id FROM resource_locations candidate
               WHERE candidate.resource_id = r.resource_id AND candidate.provider_ref = 'litecowork.encrypted_blob'
               ORDER BY (candidate.availability = 'AVAILABLE') DESC, candidate.last_checked_at DESC, candidate.location_id ASC LIMIT 1
             )
             WHERE r.current_revision_id = rev.resource_revision_id
               AND rev.content_digest = i.source_content_digest
               AND rev.size_bytes = i.extracted_size_bytes
               AND (r.context_document_json IS NULL OR json_extract(r.context_document_json, '$.status') = 'ACTIVE')
               AND (?{} IS NULL OR r.kind = ?{})
               AND (?{} IS NULL OR ({freshness_expr}) = ?{})
               AND (?{} IS NULL OR r.created_at < ?{} OR (r.created_at = ?{} AND r.resource_id < ?{}))
             ORDER BY r.created_at DESC, r.resource_id DESC LIMIT ?{}",
            terms.len(),
            terms.len() + 3, terms.len() + 4,
            terms.len() + 5, terms.len() + 6,
            terms.len() + 7, terms.len() + 8, terms.len() + 9, terms.len() + 10,
            terms.len() + 11,
        );
        let mut values = vec![rusqlite::types::Value::Text(workspace_id.to_owned()), rusqlite::types::Value::Integer(i64::from(*key_version))];
        values.extend(terms.iter().cloned().map(rusqlite::types::Value::Text));
        values.push(kind.map_or(rusqlite::types::Value::Null, |value| rusqlite::types::Value::Text(value.to_owned())));
        values.push(kind.map_or(rusqlite::types::Value::Null, |value| rusqlite::types::Value::Text(value.to_owned())));
        values.push(freshness.map_or(rusqlite::types::Value::Null, |value| rusqlite::types::Value::Text(value.to_owned())));
        values.push(freshness.map_or(rusqlite::types::Value::Null, |value| rusqlite::types::Value::Text(value.to_owned())));
        values.push(after_created_at.map_or(rusqlite::types::Value::Null, |value| rusqlite::types::Value::Text(value.to_owned())));
        values.push(after_created_at.map_or(rusqlite::types::Value::Null, |value| rusqlite::types::Value::Text(value.to_owned())));
        values.push(after_created_at.map_or(rusqlite::types::Value::Null, |value| rusqlite::types::Value::Text(value.to_owned())));
        values.push(after_resource_id.map_or(rusqlite::types::Value::Null, |value| rusqlite::types::Value::Text(value.to_owned())));
        values.push(rusqlite::types::Value::Integer(to_sql_i64(limit as u64)?));
        let mut statement = connection.prepare(&sql).map_err(map_index_database_error)?;
        let rows = statement.query_map(rusqlite::params_from_iter(values.iter()), |row| {
            let size: i64 = row.get(6)?;
            let extracted_size: i64 = row.get(18)?;
            let display_name: String = row.get(3)?;
            let media_type: String = row.get(4)?;
            Ok((
                ResourceSearchRecord {
                    summary: ResourceSummary {
                        resource_id: row.get(0)?, workspace_id: row.get(1)?, resource_revision_id: row.get(2)?,
                        display_name, media_type, content_digest: row.get(5)?,
                        size_bytes: u64::try_from(size).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, size))?,
                        created_at: row.get(7)?,
                    },
                    kind: row.get(8)?, location_id: row.get(9)?, availability: row.get(10)?,
                    writable: row.get::<_, i64>(11)? != 0, observed_revision_id: row.get(12)?,
                    observed_digest: row.get(13)?, observed_at: row.get(14)?, last_checked_at: row.get(15)?,
                    freshness: row.get(16)?, match_reasons: vec!["CONTENT_INDEXED".to_owned()],
                },
                BlobRef { digest: row.get(17)?, size_bytes: u64::try_from(extracted_size).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(18, extracted_size))?, media_type: INDEX_MEDIA_TYPE.to_owned() },
                row.get::<_, String>(19)?, row.get::<_, String>(20)?, row.get::<_, i64>(21)?,
            ))
        }).map_err(map_index_database_error)?;
        for row in rows {
            let (record, extracted_blob, source_content_digest, parser_id, returned_key_version) = row.map_err(map_index_database_error)?;
            if returned_key_version != i64::from(*key_version) {
                return Err(StoreError::Integrity("Resource index token key version mismatch".to_owned()));
            }
            found.push(IndexCandidate {
                record, extracted_blob, source_content_digest, parser_id,
                token_key_version: *key_version,
                matched_term_count: u32::try_from(terms.len()).map_err(|_| StoreError::Invalid("query has too many terms".to_owned()))?,
            });
        }
    }
    found.sort_by(|left, right| {
        right.record.summary.created_at.cmp(&left.record.summary.created_at)
            .then_with(|| right.record.summary.resource_id.cmp(&left.record.summary.resource_id))
    });
    found.dedup_by(|right, left| right.record.summary.resource_id == left.record.summary.resource_id
        && right.record.summary.resource_revision_id == left.record.summary.resource_revision_id);
    found.truncate(limit);
    Ok(found)
}

/// Recheck candidate head and sensitivity after decrypting the index snapshot.
pub(crate) fn candidate_is_current(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
    source_digest: &str,
) -> Result<bool, StoreError> {
    connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM resources r JOIN resource_revisions rev
             ON rev.resource_id = r.resource_id AND rev.resource_revision_id = r.current_revision_id
           JOIN resource_text_indexes i
             ON i.workspace_id = r.workspace_id AND i.resource_id = r.resource_id AND i.resource_revision_id = rev.resource_revision_id
           WHERE r.workspace_id = ?1 AND r.resource_id = ?2 AND rev.resource_revision_id = ?3
             AND rev.content_digest = ?4 AND i.source_content_digest = ?4
             AND (r.context_document_json IS NULL OR json_extract(r.context_document_json, '$.status') = 'ACTIVE')
         )",
        params![workspace_id, resource_id, revision_id, source_digest],
        |row| row.get(0),
    ).map_err(map_index_database_error)
}

/// Decrypt and verify selected candidates outside any SQLite transaction. A caller then
/// invokes `candidate_is_current` through the bounded writer before returning them.
pub(crate) fn materialize_matches(
    blobs: &dyn BlobStore,
    workspace_id: &str,
    candidates: Vec<IndexCandidate>,
    query_terms: &[String],
) -> Result<Vec<ResourceTextSearchRecord>, StoreError> {
    let mut matches = Vec::new();
    for candidate in candidates {
        if candidate.record.summary.workspace_id != workspace_id || candidate.token_key_version == 0 {
            return Err(StoreError::Integrity("Resource index candidate escaped its Workspace".to_owned()));
        }
        let revision_id = candidate.record.summary.resource_revision_id.clone();
        let text_bytes = blobs.get(workspace_id, BlobPurpose::ResourceIndex, &candidate.extracted_blob)?;
        if text_bytes.len() as u64 != candidate.extracted_blob.size_bytes
            || text_bytes.len() as u64 != candidate.record.summary.size_bytes
            || digest(&text_bytes) != candidate.source_content_digest
            || candidate.record.summary.content_digest != candidate.source_content_digest
        {
            return Err(StoreError::Integrity("Resource index snapshot does not match its source revision".to_owned()));
        }
        let text = std::str::from_utf8(&text_bytes)
            .map_err(|_| StoreError::Integrity("Resource index snapshot is no longer valid UTF-8".to_owned()))?;
        if !matches_all(text, query_terms) {
            return Err(StoreError::Integrity("Resource index terms do not match the encrypted snapshot".to_owned()));
        }
        let excerpt = snippet(text, query_terms, 320)
            .ok_or_else(|| StoreError::Integrity("matched Resource index has no matching excerpt".to_owned()))?;
        matches.push(ResourceTextSearchRecord {
            result: candidate.record,
            resource_revision_id: revision_id,
            source_content_digest: candidate.source_content_digest,
            snippet: excerpt,
            matched_term_count: candidate.matched_term_count,
            parser_id: candidate.parser_id,
        });
    }
    Ok(matches)
}

/// Build a revision-pinned encrypted text snapshot and keyed lexical terms.
/// Unsupported, oversized, invalid UTF-8, or control-heavy inputs are skipped without
/// creating a partial index. Blob/key failures remain errors so callers can surface a
/// repairable indexing failure rather than silently claiming the revision was indexed.
pub fn prepare(
    blobs: &dyn BlobStore,
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
    display_name: &str,
    media_type: &str,
    source_digest: &str,
    indexed_at: &str,
    bytes: &[u8],
) -> Result<Option<PreparedResourceTextIndex>, StoreError> {
    if workspace_id.trim().is_empty()
        || resource_id.trim().is_empty()
        || revision_id.trim().is_empty()
        || indexed_at.trim().is_empty()
    {
        return Err(StoreError::Invalid("Resource index scope is invalid".to_owned()));
    }
    if bytes.len() as u64 > MAX_INDEXABLE_RESOURCE_BYTES
        || !is_allowlisted_text(display_name, media_type)
    {
        return Ok(None);
    }
    let Some(text) = std::str::from_utf8(bytes).ok() else {
        return Ok(None);
    };
    if text.contains('\0')
        || text
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Ok(None);
    }
    let actual_digest = format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes)));
    if source_digest != actual_digest {
        return Err(StoreError::Integrity(
            "Resource index source does not match the pinned revision digest".to_owned(),
        ));
    }

    let normalized_terms = unique_terms(text, MAX_TERM_COUNT + 1);
    if normalized_terms.len() > MAX_TERM_COUNT {
        // Don't make an incomplete index look authoritative. The Resource remains
        // available through metadata and bounded on-demand search.
        return Ok(None);
    }
    let normalized_terms = normalized_terms.into_iter().collect::<Vec<_>>();
    // Empty text has no terms but still gets an encrypted snapshot and a nonzero key
    // version. Use a domain-separated sentinel that cannot match a user query token.
    let (token_key_version, mut term_tokens) = if normalized_terms.is_empty() {
        let (version, _) = blobs.resource_index_token(workspace_id, None, "__lc_empty_v1")?;
        (version, Vec::new())
    } else {
        blobs.resource_index_tokens(workspace_id, None, &normalized_terms)?
    };
    term_tokens.sort_unstable();
    term_tokens.dedup();

    let extracted_text = blobs.put(workspace_id, BlobPurpose::ResourceIndex, bytes, INDEX_MEDIA_TYPE)?;
    if extracted_text.size_bytes != bytes.len() as u64
        || blobs.get(workspace_id, BlobPurpose::ResourceIndex, &extracted_text)? != bytes
    {
        return Err(StoreError::Integrity(
            "encrypted Resource index snapshot failed verification".to_owned(),
        ));
    }
    Ok(Some(PreparedResourceTextIndex {
        workspace_id: workspace_id.to_owned(),
        resource_id: resource_id.to_owned(),
        resource_revision_id: revision_id.to_owned(),
        source_content_digest: actual_digest,
        extracted_text,
        parser_id: TEXT_PARSER_ID.to_owned(),
        token_key_version,
        term_tokens,
        indexed_at: indexed_at.to_owned(),
    }))
}

/// Normalize one bounded query into unique deterministic terms.
pub fn query_terms(query: &str) -> Result<Vec<String>, StoreError> {
    if query.trim().is_empty() || query.len() > 256 || query.contains('\0') {
        return Err(StoreError::Invalid("indexed Resource search query is invalid".to_owned()));
    }
    let terms = unique_terms(query, MAX_QUERY_TERMS + 1);
    if terms.is_empty() || terms.len() > MAX_QUERY_TERMS {
        return Err(StoreError::Invalid("indexed Resource search query has too many terms".to_owned()));
    }
    Ok(terms)
}

/// Search semantics are AND over distinct Unicode alphanumeric tokens. Term equality is
/// exact after lowercase normalization; substring/semantic relevance is not implied.
pub fn matches_all(document: &str, terms: &[String]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let document_terms = unique_terms(document, MAX_TERM_COUNT);
    terms.iter().all(|term| document_terms.contains(term))
}

/// Return a bounded plain-text excerpt around the first matching token. Callers must
/// encode this as text, never HTML, and must recheck the Resource revision before return.
pub fn snippet(text: &str, terms: &[String], maximum_chars: usize) -> Option<String> {
    if maximum_chars == 0 || text.contains('\0') {
        return None;
    }
    let (byte_start, byte_end) = text
        .char_indices()
        .filter(|(_, character)| character.is_alphanumeric())
        .fold(Vec::<(usize, usize)>::new(), |mut ranges, (start, character)| {
            if let Some((_, prior_end)) = ranges.last_mut() {
                if *prior_end == start {
                    *prior_end = start + character.len_utf8();
                    return ranges;
                }
            }
            ranges.push((start, start + character.len_utf8()));
            ranges
        })
        .into_iter()
        .find_map(|(start, end)| {
            let token = &text[start..end];
            terms.iter().any(|term| token.to_lowercase() == *term).then_some((start, end))
        })?;
    let mut start = byte_start.saturating_sub(maximum_chars / 2);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = byte_end.saturating_add(maximum_chars / 2).min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    let mut excerpt = text[start..end]
        .chars()
        .take(maximum_chars)
        .map(|character| if matches!(character, '\n' | '\r' | '\t') { ' ' } else { character })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if start > 0 {
        excerpt.insert(0, '…');
    }
    if end < text.len() {
        excerpt.push('…');
    }
    Some(excerpt)
}

pub fn is_allowlisted_text(display_name: &str, media_type: &str) -> bool {
    let name = display_name.to_ascii_lowercase();
    let media = media_type
        .split(';')
        .next()
        .unwrap_or(media_type)
        .trim()
        .to_ascii_lowercase();
    if [
        ".zip", ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
        ".odt", ".ods", ".odp",
    ]
    .iter()
    .any(|extension| name.ends_with(extension))
        || media == "application/zip"
        || media == "application/x-zip-compressed"
        || media == "application/pdf"
        || media.contains("officedocument")
        || matches!(media.as_str(), "application/msword" | "application/vnd.ms-excel" | "application/vnd.ms-powerpoint")
        || media.starts_with("image/")
        || media.starts_with("audio/")
        || media.starts_with("video/")
    {
        return false;
    }
    let extension_ok = [
        ".txt", ".md", ".markdown", ".csv", ".json", ".jsonl", ".ndjson", ".rs", ".py",
        ".toml", ".yaml", ".yml", ".js", ".jsx", ".ts", ".tsx", ".css",
    ]
    .iter()
    .any(|extension| name.ends_with(extension));
    let media_ok = matches!(
        media.as_str(),
        "text/plain"
            | "text/markdown"
            | "text/csv"
            | "application/json"
            | "application/x-ndjson"
            | "application/jsonl"
            | "application/yaml"
            | "application/toml"
    ) || (media.starts_with("text/")
        && matches!(
            media.as_str(),
            "text/x-rust"
                | "text/x-python"
                | "text/javascript"
                | "text/typescript"
                | "text/x-toml"
                | "text/yaml"
        ));
    extension_ok || media_ok
}

fn validate_prepared(index: &PreparedResourceTextIndex) -> Result<(), StoreError> {
    if index.workspace_id.trim().is_empty()
        || index.resource_id.trim().is_empty()
        || index.resource_revision_id.trim().is_empty()
        || index.source_content_digest.len() != 71
        || !index.source_content_digest.starts_with("sha256:")
        || index.source_content_digest[7..].bytes().any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
        || index.extracted_text.size_bytes > MAX_INDEXABLE_RESOURCE_BYTES
        || index.token_key_version == 0
        || index.parser_id != TEXT_PARSER_ID
        || index.term_tokens.len() > MAX_TERM_COUNT
        || index.term_tokens.iter().any(|token| token.len() != 64 || token.bytes().any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase()))
        || index.indexed_at.trim().is_empty()
    {
        return Err(StoreError::Invalid("prepared Resource text index is invalid".to_owned()));
    }
    if index.term_tokens.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(StoreError::Invalid("prepared Resource index terms must be sorted and unique".to_owned()));
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes)))
}

fn to_sql_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::Invalid("Resource index value exceeds SQLite range".to_owned()))
}

fn map_index_database_error(error: rusqlite::Error) -> StoreError {
    if let rusqlite::Error::SqliteFailure(code, _) = &error {
        match code.code {
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => return StoreError::Busy,
            rusqlite::ErrorCode::DiskFull => return StoreError::Io("database disk is full".to_owned()),
            rusqlite::ErrorCode::ConstraintViolation => return StoreError::Database("Resource index constraint rejected the operation".to_owned()),
            _ => {}
        }
    }
    StoreError::Database(error.to_string())
}

fn unique_terms(text: &str, maximum: usize) -> BTreeSet<String> {
    let mut terms = BTreeSet::new();
    let mut current = String::new();
    let mut too_long = false;
    for character in text.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            if !too_long && current.chars().count() < MAX_TERM_CHARS {
                current.push(character);
            } else {
                too_long = true;
            }
        } else if !current.is_empty() || too_long {
            if !too_long {
                terms.insert(std::mem::take(&mut current));
                if terms.len() >= maximum {
                    return terms;
                }
            } else {
                current.clear();
            }
            too_long = false;
        }
    }
    if !too_long && !current.is_empty() && terms.len() < maximum {
        terms.insert(current);
    }
    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_and_rich_documents_are_never_indexed_as_plain_text() {
        assert!(!is_allowlisted_text("archive.zip", "text/plain"));
        assert!(!is_allowlisted_text("report.pdf", "text/plain"));
        assert!(!is_allowlisted_text("report.docx", "application/octet-stream"));
    }

    #[test]
    fn terms_are_unicode_lowercase_unique_and_and_matched() {
        let terms = query_terms("HELLO café hello").expect("valid query");
        assert_eq!(terms, vec!["café".to_owned(), "hello".to_owned()]);
        assert!(matches_all("Hello, CAFÉ world", &terms));
        assert!(!matches_all("Hello world", &terms));
    }

    #[test]
    fn snippets_are_bounded_and_sanitize_line_breaks() {
        let terms = vec!["needle".to_owned()];
        let value = snippet("first line\nneedle in the middle\nlast", &terms, 22).expect("match");
        assert!(value.chars().count() <= 24);
        assert!(!value.contains('\n'));
    }

    #[test]
    fn query_rejects_empty_and_overly_fragmented_input() {
        assert!(query_terms("  ").is_err());
        let too_many_terms = (0..33).map(|index| format!("term{index}")).collect::<Vec<_>>().join(" ");
        assert!(query_terms(&too_many_terms).is_err());
    }
}
