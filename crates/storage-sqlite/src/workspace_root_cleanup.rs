use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::json;
use storage_core::{AggregateStateRef, EventDraft, StoreError};

impl super::SqliteWorkspaceStore {
    /// Commits one bounded pending WorkspaceRoot cleanup batch through the serialized
    /// SQLite writer. The writer owns the IMMEDIATE transaction and commits the location
    /// events, membership deletion, and outbox cursor together.
    pub fn process_next_workspace_root_cleanup_batch(
        &self,
        origin_runtime_id: &str,
        now: &str,
    ) -> Result<bool, StoreError> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.execute_command(
            super::Command::ProcessNextWorkspaceRootCleanupBatch {
                origin_runtime_id: origin_runtime_id.to_owned(),
                now: now.to_owned(),
                reply,
            },
            receiver,
        )
    }
}

/// Processes at most one bounded batch from the oldest pending root. Callers should
/// repeat this from a supervised Runtime worker and sleep between empty/error polls.
pub(crate) fn process_next_pending_batch(
    transaction: &Transaction<'_>,
    origin_runtime_id: &str,
    now: &str,
) -> Result<bool, StoreError> {
    let workspace_root_id: Option<String> = transaction
        .query_row(
            "SELECT workspace_root_id FROM workspace_root_cleanup_outbox
             WHERE status = 'PENDING'
             ORDER BY created_at, workspace_root_id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(super::map_database_error)?;
    let Some(workspace_root_id) = workspace_root_id else {
        return Ok(false);
    };
    process_next_batch(
        transaction,
        &workspace_root_id,
        origin_runtime_id,
        now,
        super::resource_index::workspace_root_membership::MAX_ROOT_FILE_CLEANUP_BATCH,
    )
}

/// Applies one bounded revoke-cleanup batch in the caller's existing IMMEDIATE
/// transaction. Location state, its established Resource event, relation removal, and
/// cursor movement therefore commit together and replay after process restart.
pub(crate) fn process_next_batch(
    transaction: &Transaction<'_>,
    workspace_root_id: &str,
    origin_runtime_id: &str,
    now: &str,
    limit: usize,
) -> Result<bool, StoreError> {
    if origin_runtime_id.trim().is_empty() || now.trim().is_empty() {
        return Err(StoreError::Invalid(
            "WorkspaceRoot cleanup event identity is invalid".to_owned(),
        ));
    }
    let Some(batch) =
        super::resource_index::workspace_root_membership::next_workspace_root_file_cleanup_batch(
            transaction,
            workspace_root_id,
            limit,
        )?
    else {
        return Ok(false);
    };
    let root_status: Option<String> = transaction
        .query_row(
            "SELECT status FROM workspace_roots
             WHERE workspace_id = ?1 AND workspace_root_id = ?2",
            params![batch.workspace_id, batch.workspace_root_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(super::map_database_error)?;
    if root_status.as_deref() != Some("REVOKED") {
        return Err(StoreError::Integrity(
            "WorkspaceRoot cleanup cursor is not behind a revoked policy fence".to_owned(),
        ));
    }

    for member in &batch.locations {
        let location: Option<(String, i64)> = transaction
            .query_row(
                "SELECT resource.workspace_id, resource.version
                 FROM resource_locations location
                 JOIN resources resource ON resource.resource_id = location.resource_id
                 WHERE location.location_id = ?1 AND location.resource_id = ?2",
                params![member.location_id, member.resource_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(super::map_database_error)?;
        let Some((workspace_id, resource_version)) = location else {
            return Err(StoreError::Integrity(
                "WorkspaceRoot cleanup member lost its canonical ResourceLocation".to_owned(),
            ));
        };
        if workspace_id != batch.workspace_id || resource_version <= 0 {
            return Err(StoreError::Integrity(
                "WorkspaceRoot cleanup member is outside its canonical Workspace".to_owned(),
            ));
        }

        transaction
            .execute(
                "UPDATE resource_locations
                 SET availability = 'REVOKED', writable = 0, observed_at = ?1
                 WHERE location_id = ?2 AND resource_id = ?3",
                params![now, member.location_id, member.resource_id],
            )
            .map_err(super::map_database_error)?;

        let expected_payload = json!({
            "location_id": member.location_id,
            "resource_id": member.resource_id,
            "availability": "REVOKED",
            "observed_at": now,
        });
        let state_ref_json: String = transaction
            .query_row(
                "SELECT aggregate_state_ref_json FROM domain_events
                 WHERE workspace_id = ?1 AND entity_type = 'Resource' AND entity_id = ?2
                   AND entity_revision = ?3
                 ORDER BY event_id ASC LIMIT 1",
                params![batch.workspace_id, member.resource_id, resource_version],
                |row| row.get(0),
            )
            .optional()
            .map_err(super::map_database_error)?
            .ok_or_else(|| {
                StoreError::Integrity(
                    "WorkspaceRoot cleanup cannot resolve canonical Resource state".to_owned(),
                )
            })?;
        let state_ref: AggregateStateRef = serde_json::from_str(&state_ref_json)
            .map_err(|error| StoreError::Integrity(error.to_string()))?;
        if i64::try_from(state_ref.entity_revision).ok() != Some(resource_version) {
            return Err(StoreError::Integrity(
                "WorkspaceRoot cleanup Resource state revision is inconsistent".to_owned(),
            ));
        }
        let event_seed = super::canonical_json(&json!({
            "workspace_root_id": batch.workspace_root_id,
            "location_id": member.location_id,
            "event_type": "resource.location.changed.v1",
        }))?;
        let event = EventDraft {
            event_id: format!(
                "workspace-root-cleanup-{}",
                super::digest(&event_seed).trim_start_matches("sha256:")
            ),
            workspace_id: batch.workspace_id.clone(),
            entity_type: "Resource".to_owned(),
            entity_id: member.resource_id.clone(),
            origin_runtime_id: origin_runtime_id.to_owned(),
            entity_revision: state_ref.entity_revision,
            hlc_timestamp: now.to_owned(),
            correlation_id: format!("workspace-root-cleanup:{}", batch.workspace_root_id),
            causation_id: None,
            schema_version: 1,
            event_type: "resource.location.changed.v1".to_owned(),
            payload: expected_payload,
            recorded_at: now.to_owned(),
        };
        super::insert_domain_event(transaction, &event, &state_ref)?;
    }

    super::resource_index::workspace_root_membership::commit_workspace_root_file_cleanup_batch(
        transaction,
        &batch,
        now,
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, TransactionBehavior, params};
    use storage_core::{AggregateStateRef, BlobRef};

    const NOW: &str = "2026-10-17T12:00:00Z";

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().expect("open cleanup database");
        connection
            .execute_batch(
                "CREATE TABLE workspace_roots(
                   workspace_id TEXT NOT NULL, workspace_root_id TEXT PRIMARY KEY,
                   status TEXT NOT NULL
                 );
                 CREATE TABLE resources(
                   resource_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL,
                   version INTEGER NOT NULL
                 );
                 CREATE TABLE resource_locations(
                   location_id TEXT PRIMARY KEY, resource_id TEXT NOT NULL,
                   availability TEXT NOT NULL, writable INTEGER NOT NULL,
                   observed_at TEXT NOT NULL
                 );
                 CREATE TABLE workspace_root_file_locations(
                   workspace_id TEXT NOT NULL, workspace_root_id TEXT NOT NULL,
                   location_id TEXT NOT NULL UNIQUE, resource_id TEXT NOT NULL,
                   observed_at TEXT NOT NULL,
                   PRIMARY KEY(workspace_root_id, location_id)
                 );
                 CREATE TABLE workspace_root_cleanup_outbox(
                   workspace_id TEXT NOT NULL, workspace_root_id TEXT PRIMARY KEY,
                   after_location_id TEXT, status TEXT NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   version INTEGER NOT NULL
                 );
                 CREATE TABLE resource_location_bindings(
                   location_id TEXT NOT NULL, binding TEXT
                 );
                 CREATE TABLE file_identity_bindings(
                   location_id TEXT NOT NULL, binding TEXT
                 );
                 CREATE TABLE workspace_origin_sequences(
                   workspace_id TEXT NOT NULL, origin_runtime_id TEXT NOT NULL,
                   last_sequence INTEGER NOT NULL,
                   PRIMARY KEY(workspace_id, origin_runtime_id)
                 );
                 CREATE TABLE domain_events(
                   event_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL,
                   entity_type TEXT NOT NULL, entity_id TEXT NOT NULL,
                   origin_runtime_id TEXT NOT NULL, origin_sequence INTEGER NOT NULL,
                   entity_revision INTEGER NOT NULL, hlc_timestamp TEXT NOT NULL,
                   correlation_id TEXT NOT NULL, causation_id TEXT,
                   schema_version INTEGER NOT NULL, type TEXT NOT NULL,
                   payload_json TEXT NOT NULL, aggregate_state_ref_json TEXT NOT NULL,
                   recorded_at TEXT NOT NULL, payload_digest TEXT NOT NULL
                 );
                 INSERT INTO workspace_roots VALUES ('ws-cleanup', 'root-cleanup', 'REVOKED');
                 INSERT INTO resources VALUES ('resource-cleanup', 'ws-cleanup', 1);
                 INSERT INTO resource_locations VALUES ('location-cleanup', 'resource-cleanup', 'AVAILABLE', 0, '2026-10-17T11:00:00Z');
                 INSERT INTO workspace_root_file_locations VALUES ('ws-cleanup', 'root-cleanup', 'location-cleanup', 'resource-cleanup', '2026-10-17T11:00:00Z');
                 INSERT INTO workspace_root_cleanup_outbox VALUES ('ws-cleanup', 'root-cleanup', NULL, 'PENDING', '2026-10-17T11:30:00Z', '2026-10-17T11:30:00Z', 1);",
            )
            .expect("create cleanup fixture");
        let state_ref = AggregateStateRef {
            blob: BlobRef {
                digest: format!("sha256:{}", "a".repeat(64)),
                size_bytes: 1,
                media_type: "application/vnd.litecowork.resource+json".to_owned(),
            },
            entity_revision: 1,
            record_schema_version: 1,
        };
        let state_ref_json = String::from_utf8(
            super::super::canonical_json(&state_ref).expect("canonical state ref"),
        )
        .expect("UTF-8 state ref");
        connection
            .execute(
                "INSERT INTO domain_events(
                   event_id, workspace_id, entity_type, entity_id, origin_runtime_id,
                   origin_sequence, entity_revision, hlc_timestamp, correlation_id,
                   causation_id, schema_version, type, payload_json,
                   aggregate_state_ref_json, recorded_at, payload_digest
                 ) VALUES ('resource-created', 'ws-cleanup', 'Resource', 'resource-cleanup',
                   'runtime-cleanup', 1, 1, ?1, 'create', NULL, 1, 'resource.created.v1',
                   '{}', ?2, ?1, ?3)",
                params![NOW, state_ref_json, format!("sha256:{}", "b".repeat(64))],
            )
            .expect("seed resource state reference");
        connection
    }

    #[test]
    fn cleanup_transitions_location_event_relation_and_cursor_atomically() {
        let mut connection = fixture();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("begin batch");
        assert!(
            process_next_batch(&transaction, "root-cleanup", "runtime-cleanup", NOW, 10)
                .expect("process cleanup batch")
        );
        transaction.commit().expect("commit cleanup batch");

        let state: (String, String, i64, i64, String) = connection
            .query_row(
                "SELECT location.availability, outbox.status, outbox.version,
                        (SELECT COUNT(*) FROM workspace_root_file_locations),
                        (SELECT type FROM domain_events WHERE event_id <> 'resource-created' LIMIT 1)
                 FROM resource_locations location
                 JOIN workspace_root_cleanup_outbox outbox ON outbox.workspace_root_id='root-cleanup'
                 WHERE location.location_id='location-cleanup'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .expect("read cleanup result");
        assert_eq!(
            state,
            (
                "REVOKED".to_owned(),
                "COMPLETE".to_owned(),
                2,
                0,
                "resource.location.changed.v1".to_owned()
            )
        );
    }

    #[test]
    fn pending_worker_selects_oldest_batch_and_reports_empty_after_completion() {
        let mut connection = fixture();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("begin worker batch");
        assert!(
            process_next_pending_batch(&transaction, "runtime-cleanup", NOW)
                .expect("process next pending root")
        );
        transaction.commit().expect("commit worker batch");

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("begin empty poll");
        assert!(
            !process_next_pending_batch(&transaction, "runtime-cleanup", NOW)
                .expect("completed outbox has no pending batch")
        );
        transaction.commit().expect("commit empty poll");
    }

    #[test]
    fn failed_resource_event_resolution_leaves_the_entire_cleanup_batch_uncommitted() {
        let mut connection = fixture();
        connection
            .execute(
                "UPDATE domain_events SET aggregate_state_ref_json = 'not-json'
                 WHERE event_id = 'resource-created'",
                [],
            )
            .expect("corrupt the pinned Resource state reference");

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("begin cleanup batch");
        assert!(matches!(
            process_next_batch(&transaction, "root-cleanup", "runtime-cleanup", NOW, 10),
            Err(StoreError::Integrity(_))
        ));
        drop(transaction);

        let state: (String, String, i64, i64, i64) = connection
            .query_row(
                "SELECT location.availability, outbox.status, outbox.version,
                        (SELECT COUNT(*) FROM workspace_root_file_locations),
                        (SELECT COUNT(*) FROM domain_events WHERE event_id <> 'resource-created')
                 FROM resource_locations location
                 JOIN workspace_root_cleanup_outbox outbox ON outbox.workspace_root_id='root-cleanup'
                 WHERE location.location_id='location-cleanup'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .expect("read rolled-back cleanup state");
        assert_eq!(
            state,
            ("AVAILABLE".to_owned(), "PENDING".to_owned(), 1, 1, 0)
        );
    }
}
