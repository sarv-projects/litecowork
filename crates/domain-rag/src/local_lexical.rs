//! The served local lexical retrieval provider.
//!
//! This adapter is a provider, not a second index. It reuses the Core-owned encrypted
//! local text index and the Core-owned Resource scope, revision, authorization, and
//! deletion rules. It never re-implements Resource resolution, never widens the
//! authorized scope, and never returns a segment that is not bound to the exact
//! immutable revision it was read from.
//!
//! The provider is deterministic token equality over the local index. It is not
//! semantic retrieval, and it does not claim to be.

use std::sync::Arc;

use storage_core::{
    ResourceDetailRecord, ResourceStore, ResourceTextSearchRecord, StoreError,
};

use crate::grounding::{
    RetrievalCitation, RetrievalWarning, SegmentLocator, TextSpan, detect_embedded_instructions,
};
use crate::limits::RetrievalBudget;
use crate::provider::{
    LOCAL_LEXICAL_PARSER_ID, LOCAL_LEXICAL_PROVIDER_ID, ProviderAvailability, ProviderPlacement,
    ProviderStatus, RetrievalError, RetrievalMode, RetrievalPage, RetrievalProvider,
    RetrievalRequest,
};
use crate::scope::RetrievalScope;

/// Version of the served local lexical provider contract.
pub const LOCAL_LEXICAL_PROVIDER_VERSION: &str = "litecowork.local-lexical/1";

/// How many index candidates the provider reads for one request before the scope and
/// authorization filters are applied. The bound keeps one request inside the storage
/// search budget while still allowing a filtered page to fill.
const CANDIDATE_MULTIPLIER: usize = 4;
const MAX_CANDIDATES: usize = 100;

/// The qualified local provider over the Core-owned encrypted text index.
pub struct LocalLexicalRetrievalProvider {
    store: Arc<dyn ResourceStore>,
    provider_version: String,
}

impl LocalLexicalRetrievalProvider {
    pub fn new(store: Arc<dyn ResourceStore>) -> Self {
        Self {
            store,
            provider_version: LOCAL_LEXICAL_PROVIDER_VERSION.to_owned(),
        }
    }

    /// The parser identity this provider can compare. A stored segment produced by a
    /// different parser generation cannot be compared with this provider.
    pub fn supported_parser_id(&self) -> &'static str {
        LOCAL_LEXICAL_PARSER_ID
    }
}

/// True when a stored index segment was produced by the parser generation this
/// provider can compare. Segments from another generation, for example an older
/// embedding or extraction model, are never mixed into one result set.
pub fn parser_generation_is_compatible(parser_id: &str) -> bool {
    parser_id == LOCAL_LEXICAL_PARSER_ID
}

impl RetrievalProvider for LocalLexicalRetrievalProvider {
    fn status(&self) -> ProviderStatus {
        ProviderStatus {
            provider_id: LOCAL_LEXICAL_PROVIDER_ID.to_owned(),
            provider_version: self.provider_version.clone(),
            availability: ProviderAvailability::Qualified,
            retrieval_mode: RetrievalMode::LexicalOnly,
            placement: ProviderPlacement::Local,
            reason_code: None,
            reason: None,
        }
    }

    fn search(&self, request: &RetrievalRequest) -> Result<RetrievalPage, RetrievalError> {
        if request.workspace_id.trim().is_empty() {
            return Err(RetrievalError::InvalidRequest {
                reason_code: "RETRIEVAL_WORKSPACE_INVALID".to_owned(),
                reason: "the retrieval request has no Workspace".to_owned(),
            });
        }
        let query = request.query.trim();
        if query.is_empty() {
            return Err(RetrievalError::InvalidRequest {
                reason_code: "RETRIEVAL_QUERY_EMPTY".to_owned(),
                reason: "the retrieval request has no query".to_owned(),
            });
        }
        request
            .budget
            .validate()
            .map_err(|error| RetrievalError::BudgetExceeded {
                reason_code: error.reason_code().to_owned(),
                reason: error.reason().to_owned(),
            })?;
        request
            .budget
            .validate_query(query)
            .map_err(|error| RetrievalError::BudgetExceeded {
                reason_code: error.reason_code().to_owned(),
                reason: error.reason().to_owned(),
            })?;
        request
            .scope
            .validate()
            .map_err(|reason| RetrievalError::InvalidRequest {
                reason_code: "RETRIEVAL_SCOPE_INVALID".to_owned(),
                reason,
            })?;
        if let RetrievalScope::Resources(resources) = &request.scope {
            if resources.is_empty() {
                return Err(RetrievalError::InvalidRequest {
                    reason_code: "RETRIEVAL_SCOPE_EMPTY".to_owned(),
                    reason: "the retrieval request authorized no Resource".to_owned(),
                });
            }
        }

        let candidate_limit = CANDIDATE_MULTIPLIER
            .saturating_mul(usize::from(request.budget.max_results))
            .clamp(1, MAX_CANDIDATES);
        let page = self
            .store
            .search_indexed_resource_text(
                &request.workspace_id,
                query,
                None,
                None,
                None,
                None,
                candidate_limit,
            )
            .map_err(map_store_error)?;

        let mut citations: Vec<RetrievalCitation> = Vec::new();
        let mut scanned_candidates = 0_usize;
        let mut truncated = page.continuation.is_some();
        let mut total_excerpt_bytes = 0_usize;
        for record in page.items {
            scanned_candidates += 1;
            if !parser_generation_is_compatible(&record.parser_id) {
                return Err(RetrievalError::Incompatible {
                    reason_code: "PROVIDER_INDEX_GENERATION_INCOMPATIBLE".to_owned(),
                    reason: "the stored index was produced by an incompatible parser generation"
                        .to_owned(),
                });
            }
            if !request
                .scope
                .allows(&record.result.summary.resource_id, &record.resource_revision_id)
            {
                continue;
            }
            if !self
                .is_still_authorized(&request.workspace_id, &record)?
            {
                continue;
            }
            let Some(citation) =
                build_citation(&record, &self.provider_version, request.budget)?
            else {
                continue;
            };
            total_excerpt_bytes = total_excerpt_bytes.saturating_add(citation.excerpt.len());
            if total_excerpt_bytes > request.budget.max_total_excerpt_bytes {
                truncated = true;
                break;
            }
            citations.push(citation);
            if citations.len() >= usize::from(request.budget.max_results) {
                truncated = true;
                break;
            }
        }
        Ok(RetrievalPage {
            citations,
            scanned_candidates,
            truncated,
        })
    }
}

impl LocalLexicalRetrievalProvider {
    /// Last-boundary authorization recheck. The stored index is a rebuildable
    /// projection: a Resource that was revoked, deleted, superseded, or moved out of the
    /// Workspace must never be served from it.
    fn is_still_authorized(
        &self,
        workspace_id: &str,
        record: &ResourceTextSearchRecord,
    ) -> Result<bool, RetrievalError> {
        let Some(detail) = self
            .store
            .get_resource_detail(workspace_id, &record.result.summary.resource_id)
            .map_err(map_store_error)?
        else {
            return Ok(false);
        };
        if !context_document_is_active(&detail) {
            return Ok(false);
        }
        if detail.resource.current_revision_id.as_deref()
            != Some(record.resource_revision_id.as_str())
        {
            return Ok(false);
        }
        if record.result.summary.content_digest != record.source_content_digest {
            return Err(RetrievalError::Integrity {
                reason_code: "RETRIEVAL_SOURCE_DIGEST_MISMATCH".to_owned(),
                reason: "the retrieved segment does not match its source revision digest"
                    .to_owned(),
            });
        }
        Ok(true)
    }
}

/// A ContextDocument is readable only while it is `ACTIVE`. A Resource without a
/// ContextDocument is an ordinary managed Resource and follows the Workspace scope.
pub fn context_document_is_active(detail: &ResourceDetailRecord) -> bool {
    let Some(context_document) = detail.context_document.as_ref() else {
        return true;
    };
    match context_document.get("status").and_then(|value| value.as_str()) {
        Some(status) => status == "ACTIVE",
        None => false,
    }
}

fn build_citation(
    record: &ResourceTextSearchRecord,
    provider_version: &str,
    budget: RetrievalBudget,
) -> Result<Option<RetrievalCitation>, RetrievalError> {
    let spans: Vec<TextSpan> = record
        .matched_spans
        .iter()
        .map(|span| TextSpan {
            start_utf8_byte: span.start_utf8_byte,
            end_utf8_byte_exclusive: span.end_utf8_byte_exclusive,
        })
        .collect();
    let mut ordered = spans.clone();
    ordered.sort_by_key(|span| span.start_utf8_byte);
    let Some(primary) = ordered.first().copied() else {
        return Ok(None);
    };
    let (excerpt, truncated) = truncate_excerpt(&record.snippet, budget.max_excerpt_bytes);
    let mut warnings = Vec::new();
    if truncated {
        warnings.push(RetrievalWarning::ExcerptTruncated);
    }
    if detect_embedded_instructions(&excerpt) {
        warnings.push(RetrievalWarning::EmbeddedInstructionDetected);
    }
    let citation = RetrievalCitation {
        citation_id: format!(
            "{}:{}:{}",
            record.result.summary.resource_id, record.resource_revision_id, primary.start_utf8_byte
        ),
        resource_id: record.result.summary.resource_id.clone(),
        resource_revision_id: record.resource_revision_id.clone(),
        content_digest: record.source_content_digest.clone(),
        locator: SegmentLocator::TextSpan { span: primary },
        matched_spans: ordered,
        excerpt,
        provider_id: LOCAL_LEXICAL_PROVIDER_ID.to_owned(),
        provider_version: provider_version.to_owned(),
        warnings,
    };
    citation
        .validate()
        .map_err(|reason| RetrievalError::Integrity {
            reason_code: "RETRIEVAL_CITATION_INVALID".to_owned(),
            reason,
        })?;
    Ok(Some(citation))
}

/// Truncates an excerpt on a UTF-8 character boundary.
fn truncate_excerpt(excerpt: &str, maximum_bytes: usize) -> (String, bool) {
    if excerpt.len() <= maximum_bytes {
        return (excerpt.to_owned(), false);
    }
    let mut end = maximum_bytes;
    while end > 0 && !excerpt.is_char_boundary(end) {
        end -= 1;
    }
    (excerpt[..end].to_owned(), true)
}

fn map_store_error(error: StoreError) -> RetrievalError {
    match error {
        StoreError::Invalid(message) => RetrievalError::InvalidRequest {
            reason_code: "RETRIEVAL_REQUEST_INVALID".to_owned(),
            reason: message,
        },
        StoreError::Blob(message) => RetrievalError::Unavailable {
            reason_code: "RETRIEVAL_INDEX_UNAVAILABLE".to_owned(),
            reason: message,
        },
        StoreError::NotFound => RetrievalError::Unavailable {
            reason_code: "RETRIEVAL_INDEX_UNAVAILABLE".to_owned(),
            reason: "the local Resource index is unavailable".to_owned(),
        },
        StoreError::Io(message) => RetrievalError::Unavailable {
            reason_code: "RETRIEVAL_INDEX_UNAVAILABLE".to_owned(),
            reason: message,
        },
        StoreError::Busy => RetrievalError::Unavailable {
            reason_code: "RETRIEVAL_INDEX_BUSY".to_owned(),
            reason: "the local Resource index is busy".to_owned(),
        },
        StoreError::Integrity(message) => RetrievalError::Integrity {
            reason_code: "RETRIEVAL_SOURCE_INTEGRITY_FAILURE".to_owned(),
            reason: message,
        },
        StoreError::CorruptSchema(message) => RetrievalError::Integrity {
            reason_code: "RETRIEVAL_SOURCE_INTEGRITY_FAILURE".to_owned(),
            reason: message,
        },
        other => RetrievalError::Integrity {
            reason_code: "RETRIEVAL_SOURCE_INTEGRITY_FAILURE".to_owned(),
            reason: format!("the local Resource index failed: {other:?}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{context_document_is_active, parser_generation_is_compatible, truncate_excerpt};
    use serde_json::json;
    use storage_core::{ResourceDetailRecord, ResourceRecord};

    fn detail(status: Option<&str>) -> ResourceDetailRecord {
        ResourceDetailRecord {
            resource: ResourceRecord {
                resource_id: "resource-a".to_owned(),
                workspace_id: "workspace-a".to_owned(),
                kind: "FILE".to_owned(),
                provider_identity: json!({}),
                identity_digest: None,
                display_name: "a.txt".to_owned(),
                current_revision_id: Some("revision-a".to_owned()),
                sensitivity: "PERSONAL".to_owned(),
                provenance: json!({}),
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
                version: 1,
            },
            context_document: status.map(|status| json!({ "status": status })),
        }
    }

    #[test]
    fn a_revoked_or_deleted_context_document_is_not_active() {
        assert!(context_document_is_active(&detail(Some("ACTIVE"))));
        assert!(!context_document_is_active(&detail(Some("REVOKED"))));
        assert!(!context_document_is_active(&detail(Some("DELETED"))));
        assert!(!context_document_is_active(&detail(Some("DELETION_PENDING"))));
    }

    #[test]
    fn a_resource_without_a_context_document_is_readable() {
        assert!(context_document_is_active(&detail(None)));
    }

    #[test]
    fn a_context_document_without_a_status_is_not_readable() {
        let mut record = detail(None);
        record.context_document = Some(json!({ "kind": "NOTE" }));
        assert!(!context_document_is_active(&record));
    }

    #[test]
    fn truncation_keeps_a_valid_utf8_boundary() {
        let (text, truncated) = truncate_excerpt("aäb", 2);
        assert!(truncated);
        assert_eq!(text, "a");
        let (text, truncated) = truncate_excerpt("abc", 8);
        assert!(!truncated);
        assert_eq!(text, "abc");
    }

    #[test]
    fn only_the_qualified_parser_generation_can_be_compared() {
        assert!(parser_generation_is_compatible("litecowork.plain-text.v1"));
        assert!(!parser_generation_is_compatible(
            "litecowork.plain-text.v0"
        ));
        assert!(!parser_generation_is_compatible(""));
    }
}
