//! Core-owned index state projection for managed Resources.
//!
//! The local text index is a rebuildable projection owned by Core. This module reads
//! the facts Core needs to tell an owner whether a Resource is searchable, stale, or
//! not indexable, and whether a superseded projection is still registered for it.
//!
//! This module never returns extracted text, term tokens, provider locators, or blob
//! digests. It exposes only revision identity, digests already visible on the
//! Resource, the parser identity, and the index timestamp.

use rusqlite::{Connection, OptionalExtension, params};
use storage_core::StoreError;

use crate::SqliteWorkspaceStore;

/// Maximum bytes of one Resource that the local text index accepts.
pub const MAX_INDEXABLE_RESOURCE_BYTES: u64 = 1_048_576;

/// The index facts Core observed for one managed Resource.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceIndexStateRow {
    pub resource_id: String,
    pub current_revision_id: Option<String>,
    pub current_content_digest: Option<String>,
    pub current_size_bytes: Option<u64>,
    pub display_name: String,
    pub media_type: String,
    pub indexed_revision_id: Option<String>,
    pub indexed_source_digest: Option<String>,
    pub parser_id: Option<String>,
    pub indexed_at: Option<String>,
    /// Number of registered index rows that belong to superseded revisions.
    pub superseded_index_count: u32,
}

/// Reads the index facts of one Resource inside the selected Workspace.
pub(crate) fn read_index_state(
    connection: &Connection,
    workspace_id: &str,
    resource_id: &str,
) -> Result<Option<ResourceIndexStateRow>, StoreError> {
    if workspace_id.trim().is_empty() || resource_id.trim().is_empty() {
        return Err(StoreError::Invalid(
            "Resource index state identity is invalid".to_owned(),
        ));
    }
    let resource = connection
        .query_row(
            "SELECT r.current_revision_id, rev.content_digest, rev.size_bytes, r.display_name,
                    rev.media_type
             FROM resources r
             LEFT JOIN resource_revisions rev
               ON rev.resource_id = r.resource_id AND rev.resource_revision_id = r.current_revision_id
             WHERE r.workspace_id = ?1 AND r.resource_id = ?2",
            params![workspace_id, resource_id],
            |row| {
                let size: Option<i64> = row.get(2)?;
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    size,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(map_index_state_error)?;
    let Some((current_revision_id, current_content_digest, size_bytes, display_name, media_type)) =
        resource
    else {
        return Ok(None);
    };
    let media_type = media_type.unwrap_or_default();

    let indexed_row =
        match current_revision_id.as_deref() {
            Some(revision_id) => connection
                .query_row(
                    "SELECT resource_revision_id, source_content_digest, parser_id, indexed_at
                     FROM resource_text_indexes
                     WHERE workspace_id = ?1 AND resource_id = ?2 AND resource_revision_id = ?3",
                    params![workspace_id, resource_id, revision_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()
                .map_err(map_index_state_error)?,
            None => None,
        };
    let (indexed_revision_id, indexed_source_digest, parser_id, indexed_at) =
        match indexed_row {
            Some((revision_id, digest, parser, indexed_at)) => (
                Some(revision_id),
                Some(digest),
                Some(parser),
                Some(indexed_at),
            ),
            None => (None, None, None, None),
        };
    let superseded_index_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_text_indexes
             WHERE workspace_id = ?1 AND resource_id = ?2
               AND (?3 IS NULL OR resource_revision_id <> ?3)",
            params![workspace_id, resource_id, current_revision_id],
            |row| row.get(0),
        )
        .map_err(map_index_state_error)?;
    let superseded_index_count = u32::try_from(superseded_index_count).unwrap_or(u32::MAX);

    Ok(Some(ResourceIndexStateRow {
        resource_id: resource_id.to_owned(),
        current_revision_id,
        current_content_digest,
        current_size_bytes: size_bytes.and_then(|size| u64::try_from(size).ok()),
        display_name,
        media_type,
        indexed_revision_id,
        indexed_source_digest,
        parser_id,
        indexed_at,
        superseded_index_count,
    }))
}

/// True when the qualified local provider cannot index this Resource at all.
pub fn not_indexable_reason(
    display_name: &str,
    media_type: &str,
    size_bytes: Option<u64>,
) -> Option<storage_core::ResourceTextIndexSkipReason> {
    if !crate::resource_index::is_allowlisted_text(display_name, media_type) {
        return Some(storage_core::ResourceTextIndexSkipReason::UnsupportedType);
    }
    if size_bytes.is_some_and(|size| size > MAX_INDEXABLE_RESOURCE_BYTES) {
        return Some(storage_core::ResourceTextIndexSkipReason::OverSizeLimit);
    }
    None
}

fn map_index_state_error(error: rusqlite::Error) -> StoreError {
    match error {
        rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
        other => StoreError::Database(other.to_string()),
    }
}

impl SqliteWorkspaceStore {
    /// Returns the index facts of one managed Resource, or `None` when the Resource is
    /// outside the selected Workspace or does not exist.
    pub fn resource_index_state(
        &self,
        workspace_id: &str,
        resource_id: &str,
    ) -> Result<Option<ResourceIndexStateRow>, StoreError> {
        let (reply_sender, reply_receiver) = std::sync::mpsc::channel();
        self.execute_command(
            crate::Command::GetResourceIndexState {
                workspace_id: workspace_id.to_owned(),
                resource_id: resource_id.to_owned(),
                reply: reply_sender,
            },
            reply_receiver,
        )
    }
}
