//! Index state projection for managed Resources.
//!
//! The local text index is a rebuildable projection. This module turns the Core-owned
//! index facts into the state an owner sees, so that "uploaded" is never confused with
//! "searchable" and a superseded projection is reported as stale instead of served.

use serde::{Deserialize, Serialize};
use storage_core::ResourceTextIndexSkipReason;

/// The observable index state of one Resource.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceIndexState {
    /// The index matches the current immutable revision and its content digest.
    Ready,
    /// An index exists for an older revision. The stale projection is never served.
    Stale,
    /// No index exists for this Resource. Uploaded content is not searchable yet.
    Absent,
    /// The qualified provider cannot index this Resource. The reason is typed.
    NotIndexable,
}

/// The index facts Core observed for one Resource.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResourceIndexFacts {
    /// The current immutable revision of the Resource.
    pub current_revision_id: Option<String>,
    /// The content digest of the current immutable revision.
    pub current_content_digest: Option<String>,
    /// The revision an existing index projection was built from.
    pub indexed_revision_id: Option<String>,
    /// The source digest an existing index projection was built from.
    pub indexed_source_digest: Option<String>,
    /// The parser that produced the existing projection.
    pub parser_id: Option<String>,
    /// When the existing projection was built.
    pub indexed_at: Option<String>,
    /// Number of registered index rows that belong to superseded revisions.
    pub superseded_index_count: u32,
    /// The typed reason the qualified provider cannot index this Resource.
    pub not_indexable_reason: Option<ResourceTextIndexSkipReason>,
}

/// The owner-visible index state of one Resource.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResourceIndexStateView {
    pub resource_id: String,
    pub state: ResourceIndexState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<ResourceTextIndexSkipReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_revision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_revision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parser_id: Option<String>,
    /// Number of registered index rows that belong to superseded revisions. These rows
    /// are never served; they are reported so that an owner can see that a changed
    /// file still carries an outdated projection.
    #[serde(default)]
    pub superseded_index_count: u32,
}

/// Resolves the observable index state from Core-owned index facts.
///
/// A projection that does not match the current revision and digest is stale and is
/// never served. A Resource without any projection is absent, which keeps "uploaded"
/// distinct from "searchable".
pub fn resolve_index_state(
    resource_id: &str,
    facts: &ResourceIndexFacts,
) -> ResourceIndexStateView {
    let state = match facts.not_indexable_reason {
        Some(_) => ResourceIndexState::NotIndexable,
        None => match (
            facts.indexed_revision_id.as_deref(),
            facts.current_revision_id.as_deref(),
        ) {
            (Some(indexed), Some(current)) if indexed == current => {
                match (
                    facts.indexed_source_digest.as_deref(),
                    facts.current_content_digest.as_deref(),
                ) {
                    (Some(indexed_digest), Some(current_digest))
                        if indexed_digest == current_digest =>
                    {
                        ResourceIndexState::Ready
                    }
                    _ => ResourceIndexState::Stale,
                }
            }
            (Some(_), _) => ResourceIndexState::Stale,
            (None, _) => {
                if facts.superseded_index_count > 0 {
                    ResourceIndexState::Stale
                } else {
                    ResourceIndexState::Absent
                }
            }
        },
    };
    ResourceIndexStateView {
        resource_id: resource_id.to_owned(),
        state,
        reason: facts.not_indexable_reason.clone(),
        current_revision_id: facts.current_revision_id.clone(),
        indexed_revision_id: facts.indexed_revision_id.clone(),
        indexed_at: facts.indexed_at.clone(),
        parser_id: facts.parser_id.clone(),
        superseded_index_count: facts.superseded_index_count,
    }
}

#[cfg(test)]
mod tests {
    use super::{ResourceIndexFacts, ResourceIndexState, resolve_index_state};
    use storage_core::ResourceTextIndexSkipReason;

    fn facts() -> ResourceIndexFacts {
        ResourceIndexFacts {
            current_revision_id: Some("revision-b".to_owned()),
            current_content_digest: Some(format!("sha256:{}", "b".repeat(64))),
            indexed_revision_id: Some("revision-b".to_owned()),
            indexed_source_digest: Some(format!("sha256:{}", "b".repeat(64))),
            parser_id: Some("litecowork.plain-text.v1".to_owned()),
            indexed_at: Some("2026-01-01T00:00:00Z".to_owned()),
            superseded_index_count: 0,
            not_indexable_reason: None,
        }
    }

    #[test]
    fn a_matching_projection_is_ready() {
        let view = resolve_index_state("resource-a", &facts());
        assert_eq!(view.state, ResourceIndexState::Ready);
    }

    #[test]
    fn a_projection_for_an_older_revision_is_stale() {
        let mut facts = facts();
        facts.indexed_revision_id = Some("revision-a".to_owned());
        facts.indexed_source_digest = Some(format!("sha256:{}", "a".repeat(64)));
        let view = resolve_index_state("resource-a", &facts);
        assert_eq!(view.state, ResourceIndexState::Stale);
    }

    #[test]
    fn a_matching_revision_with_a_different_digest_is_stale() {
        let mut facts = facts();
        facts.indexed_source_digest = Some(format!("sha256:{}", "a".repeat(64)));
        let view = resolve_index_state("resource-a", &facts);
        assert_eq!(view.state, ResourceIndexState::Stale);
    }

    #[test]
    fn a_resource_without_a_projection_is_absent_not_ready() {
        let mut facts = facts();
        facts.indexed_revision_id = None;
        facts.indexed_source_digest = None;
        facts.indexed_at = None;
        let view = resolve_index_state("resource-a", &facts);
        assert_eq!(view.state, ResourceIndexState::Absent);
    }

    #[test]
    fn a_superseded_projection_makes_a_changed_file_stale() {
        let mut facts = facts();
        facts.indexed_revision_id = None;
        facts.indexed_source_digest = None;
        facts.indexed_at = None;
        facts.superseded_index_count = 1;
        let view = resolve_index_state("resource-a", &facts);
        assert_eq!(view.state, ResourceIndexState::Stale);
        assert_eq!(view.superseded_index_count, 1);
    }

    #[test]
    fn a_typed_not_indexable_reason_wins() {
        let mut facts = facts();
        facts.not_indexable_reason = Some(ResourceTextIndexSkipReason::UnsupportedType);
        let view = resolve_index_state("resource-a", &facts);
        assert_eq!(view.state, ResourceIndexState::NotIndexable);
        assert_eq!(
            view.reason,
            Some(ResourceTextIndexSkipReason::UnsupportedType)
        );
    }
}
