use super::*;
use rusqlite::{Connection, params};
use serde_json::json;

const WORKSPACE_ID: &str = "workspace-v18-migration";
const RESOURCE_ID: &str = "resource-v18-migration";
const RESOURCE_REVISION_ID: &str = "revision-v18-migration";
const CREATED_AT: &str = "2026-10-16T10:00:00Z";

type PurgeReceiptWithoutUploadRefs = (
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);
type PurgeReceiptWithUploadRefs = (
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
);

fn state_database(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory.path().join("state").join("state.sqlite3")
}

fn table_rows<T, F>(connection: &Connection, sql: &str, read_row: F) -> Vec<T>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut statement = connection
        .prepare(sql)
        .expect("prepare fixture snapshot query");
    statement
        .query_map([], read_row)
        .expect("query fixture snapshot")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect fixture snapshot")
}

fn purge_schema_objects(connection: &Connection) -> Vec<(String, String, String, String)> {
    table_rows(
        connection,
        "SELECT type, name, tbl_name, COALESCE(sql, '')
         FROM sqlite_master
         WHERE name LIKE 'context_document_purge_%' OR name = 'idx_context_purge_pending'
         ORDER BY type, name",
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
}

fn seed_v17_purge_and_upload_fixture(connection: &Connection) -> (String, String, String) {
    let resource_digest = digest(b"context document content");
    let target = json!({
        "replica_ref": resource_digest,
        "replica_kind": "BLOB",
        "runtime_id": null,
        "runtime_incarnation_id": null,
        "target_revision_ids": [RESOURCE_REVISION_ID],
    });
    let targets_json = String::from_utf8(canonical_json(&vec![target]).expect("canonical target"))
        .expect("target JSON is UTF-8");
    let manifest_digest = digest(targets_json.as_bytes());

    connection
        .execute(
            "INSERT INTO workspaces(
                 workspace_id, name, owner_principal_id, replication_policy, status,
                 created_at, updated_at, version
             ) VALUES (?1, 'Migration fixture', 'owner-local', 'LOCAL_ONLY', 'ACTIVE', ?2, ?2, 1)",
            params![WORKSPACE_ID, CREATED_AT],
        )
        .expect("insert fixture Workspace");
    connection
        .execute(
            "INSERT INTO resources(
                 resource_id, workspace_id, kind, provider_identity_json, display_name,
                 current_revision_id, sensitivity, context_document_json, provenance_json,
                 created_at, updated_at, version
             ) VALUES (
                 ?1, ?2, 'FILE', '{}', 'context.md', NULL, 'PERSONAL', ?3, '{}', ?4, ?4, 1
             )",
            params![
                RESOURCE_ID,
                WORKSPACE_ID,
                String::from_utf8(
                    canonical_json(&json!({
                        "kind": "WORKSPACE_NOTES",
                        "owner_ref": {"kind": "WORKSPACE", "workspace_id": WORKSPACE_ID},
                        "status": "ACTIVE",
                        "purge_manifest_digest": null,
                        "purge_target_count": null,
                    }))
                    .expect("canonical active ContextDocument")
                )
                .expect("ContextDocument JSON is UTF-8"),
                CREATED_AT,
            ],
        )
        .expect("insert active ContextDocument Resource");
    connection
        .execute(
            "INSERT INTO resource_revisions(
                 resource_revision_id, resource_id, content_digest, size_bytes,
                 media_type, observed_at, created_by_json
             ) VALUES (?1, ?2, ?3, 24, 'text/markdown', ?4, '{}')",
            params![
                RESOURCE_REVISION_ID,
                RESOURCE_ID,
                resource_digest,
                CREATED_AT
            ],
        )
        .expect("insert immutable Resource revision");
    connection
        .execute(
            "INSERT INTO resource_content_blob_registry(
                 workspace_id, digest, size_bytes, media_type, created_at,
                 expires_at, state, claimed_at
             ) VALUES (?1, ?2, 24, 'text/markdown', ?3, NULL, 'TRACKED', NULL)",
            params![WORKSPACE_ID, resource_digest, CREATED_AT],
        )
        .expect("register local Resource content");
    connection
        .execute(
            "UPDATE resources SET current_revision_id = ?1 WHERE resource_id = ?2",
            params![RESOURCE_REVISION_ID, RESOURCE_ID],
        )
        .expect("select current Resource revision");
    connection
        .execute(
            "INSERT INTO resource_locations(
                 location_id, resource_id, provider_ref, locator_ref_id, availability,
                 writable, observed_revision_id, observed_digest, observed_at, last_checked_at
             ) VALUES (
                 'location-v18-migration', ?1, 'litecowork.encrypted_blob', ?2,
                 'AVAILABLE', 0, ?3, ?2, ?4, ?4
             )",
            params![
                RESOURCE_ID,
                resource_digest,
                RESOURCE_REVISION_ID,
                CREATED_AT
            ],
        )
        .expect("insert managed local Resource location");
    connection
        .execute(
            "INSERT INTO context_document_purge_plans(
                 workspace_id, resource_id, manifest_digest, target_count, targets_json, sealed_at
             ) VALUES (?1, ?2, ?3, 1, ?4, ?5)",
            params![
                WORKSPACE_ID,
                RESOURCE_ID,
                manifest_digest,
                targets_json,
                CREATED_AT
            ],
        )
        .expect("insert valid v17 sealed purge plan");
    connection
        .execute(
            "UPDATE resources
             SET context_document_json = ?1, updated_at = '2026-10-16T10:00:01Z', version = 2
             WHERE workspace_id = ?2 AND resource_id = ?3",
            params![
                String::from_utf8(
                    canonical_json(&json!({
                        "kind": "WORKSPACE_NOTES",
                        "owner_ref": {"kind": "WORKSPACE", "workspace_id": WORKSPACE_ID},
                        "status": "DELETION_PENDING",
                        "purge_manifest_digest": manifest_digest,
                        "purge_target_count": 1,
                    }))
                    .expect("canonical pending ContextDocument")
                )
                .expect("ContextDocument JSON is UTF-8"),
                WORKSPACE_ID,
                RESOURCE_ID,
            ],
        )
        .expect("transition ContextDocument to deletion pending");
    let revision_ids_json = String::from_utf8(
        canonical_json(&vec![RESOURCE_REVISION_ID]).expect("canonical target revision list"),
    )
    .expect("target revision JSON is UTF-8");
    connection
        .execute(
            "INSERT INTO context_document_purge_receipts(
                 workspace_id, resource_id, replica_ref, replica_kind, runtime_id,
                 runtime_incarnation_id, target_revision_ids_json, status, requested_at,
                 acknowledged_at, receipt_digest
             ) VALUES (?1, ?2, ?3, 'BLOB', NULL, NULL, ?4, 'PENDING', ?5, NULL, NULL)",
            params![
                WORKSPACE_ID,
                RESOURCE_ID,
                resource_digest,
                revision_ids_json,
                "2026-10-16T10:00:01Z",
            ],
        )
        .expect("insert valid pre-v18 purge receipt");

    let accepted_upload_digest = digest(b"accepted-chunk");
    let accepted_whole_digest = accepted_upload_digest.clone();
    connection
        .execute(
            "INSERT INTO resource_upload_sessions(
                 upload_id, workspace_id, display_name, media_type, expected_size_bytes,
                 expected_digest, chunk_size_bytes, state, expires_at, created_at,
                 version, progress_version
             ) VALUES (
                 'upload-v18-accepted', ?1, 'accepted.bin', 'application/octet-stream',
                 14, ?2, 4194304, 'OPEN', '2026-10-17T10:00:00Z', ?3, 1, 1
             )",
            params![WORKSPACE_ID, accepted_whole_digest, CREATED_AT],
        )
        .expect("insert upload that already has an accepted chunk");
    connection
        .execute(
            "INSERT INTO resource_upload_chunks(
                 upload_id, chunk_index, start_offset, end_offset_exclusive,
                 sha256, temporary_blob_ref, received_at
             ) VALUES ('upload-v18-accepted', 0, 0, 14, ?1, ?1, ?2)",
            params![accepted_upload_digest, CREATED_AT],
        )
        .expect("insert accepted upload chunk");
    connection
        .execute(
            "INSERT INTO resource_upload_chunk_requests(
                 upload_id, request_id, chunk_index, request_payload_digest, created_at
             ) VALUES ('upload-v18-accepted', 'request-v18-accepted', 0, ?1, ?2)",
            params![digest(b"accepted chunk request"), CREATED_AT],
        )
        .expect("insert accepted chunk request receipt");
    connection
        .execute(
            "UPDATE resource_upload_sessions SET state = 'EXPIRED', version = 2
             WHERE upload_id = 'upload-v18-accepted'",
            [],
        )
        .expect("transition accepted upload to terminal state");

    let reserved_digest = digest(b"reserved-chunk");
    connection
        .execute(
            "INSERT INTO resource_upload_sessions(
                 upload_id, workspace_id, display_name, media_type, expected_size_bytes,
                 expected_digest, chunk_size_bytes, state, expires_at, created_at,
                 version, progress_version
             ) VALUES (
                 'upload-v18-reserved', ?1, 'reserved.bin', 'application/octet-stream',
                 14, ?2, 4194304, 'OPEN', '2026-10-17T10:00:00Z', ?3, 1, 1
             )",
            params![WORKSPACE_ID, digest(b"reserved-whole-file"), CREATED_AT],
        )
        .expect("insert upload with an interrupted chunk write");
    connection
        .execute(
            "INSERT INTO resource_upload_blob_reservations(
                 upload_id, request_id, workspace_id, chunk_index, digest, size_bytes,
                 created_at, expires_at, state
             ) VALUES (
                 'upload-v18-reserved', 'request-v18-reserved', ?1, 0, ?2, 14,
                 ?3, '2026-10-17T10:00:00Z', 'RESERVED'
             )",
            params![WORKSPACE_ID, reserved_digest, CREATED_AT],
        )
        .expect("insert upload blob write reservation");

    (resource_digest, accepted_upload_digest, reserved_digest)
}

#[test]
fn v18_context_document_purge_migration_rolls_back_exactly_and_backfills_on_retry() {
    let directory = tempfile::tempdir().expect("temporary v18 migration directory");
    let database = state_database(&directory);
    std::fs::create_dir_all(database.parent().expect("state directory path"))
        .expect("create private state directory");
    let mut connection = Connection::open(database).expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign key enforcement");

    // Failing at v18 once leaves the already committed v17 schema boundary intact.
    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV18Migration)).is_err());
    let v17: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read v17 schema boundary");
    assert_eq!(v17, V17_SCHEMA_VERSION);

    let (resource_digest, accepted_upload_digest, reserved_digest) =
        seed_v17_purge_and_upload_fixture(&connection);
    let schema_before = purge_schema_objects(&connection);
    let plan_before: (String, i64, String, String) = connection
        .query_row(
            "SELECT manifest_digest, target_count, targets_json, sealed_at
             FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE_ID, RESOURCE_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read exact v17 purge plan");
    let receipt_before: PurgeReceiptWithoutUploadRefs = connection
        .query_row(
            "SELECT replica_ref, replica_kind, runtime_id, runtime_incarnation_id,
                    target_revision_ids_json, status, requested_at, acknowledged_at, receipt_digest
             FROM context_document_purge_receipts
             WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE_ID, RESOURCE_ID],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .expect("read exact v17 purge receipt");

    let migration_error =
        migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV18Migration))
            .expect_err("injected v18 failure must roll the DDL transaction back");
    assert_eq!(
        migration_error,
        StoreError::Database("injected DuringV18Migration".to_owned())
    );
    let version_after_failure: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version after failed v18 migration");
    assert_eq!(version_after_failure, V17_SCHEMA_VERSION);
    assert_eq!(purge_schema_objects(&connection), schema_before);
    let plan_after_failure: (String, i64, String, String) = connection
        .query_row(
            "SELECT manifest_digest, target_count, targets_json, sealed_at
             FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE_ID, RESOURCE_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read purge plan after failed migration");
    let receipt_after_failure: PurgeReceiptWithoutUploadRefs = connection
        .query_row(
            "SELECT replica_ref, replica_kind, runtime_id, runtime_incarnation_id,
                    target_revision_ids_json, status, requested_at, acknowledged_at, receipt_digest
             FROM context_document_purge_receipts
             WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE_ID, RESOURCE_ID],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .expect("read purge receipt after failed migration");
    assert_eq!(plan_after_failure, plan_before);
    assert_eq!(receipt_after_failure, receipt_before);
    let v18_partial_objects: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE name IN (
                 'resource_upload_chunk_blob_registry',
                 'resource_upload_chunk_blob_registry_gc',
                 'context_document_purge_receipts_v17',
                 'resource_upload_chunk_registry_insert_guard',
                 'resource_upload_chunk_reservation_resource_guard'
             )",
            [],
            |row| row.get(0),
        )
        .expect("inspect rolled-back v18 objects");
    assert_eq!(v18_partial_objects, 0);
    let v18_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 18",
            [],
            |row| row.get(0),
        )
        .expect("inspect v18 migration receipt after failure");
    assert_eq!(v18_receipts, 0);

    migrate(&mut connection).expect("retry migrations after v18 rollback");
    let final_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version after migration retry");
    assert_eq!(final_version, SCHEMA_VERSION);
    verify_migration_record(&connection, 18, V18_MIGRATION_NAME, SQLITE_V18_DDL, false)
        .expect("verify successful v18 migration receipt");

    let v18_registry_objects: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE name IN (
                 'resource_upload_chunk_blob_registry',
                 'resource_upload_chunk_blob_registry_gc',
                 'resource_upload_chunk_registry_insert_guard',
                 'resource_upload_chunk_reservation_resource_guard'
             )",
            [],
            |row| row.get(0),
        )
        .expect("inspect successful v18 registry objects");
    assert_eq!(v18_registry_objects, 4);
    let target_upload_column: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('context_document_purge_receipts')
             WHERE name = 'target_upload_chunk_refs_json'",
            [],
            |row| row.get(0),
        )
        .expect("inspect v18 purge receipt extension");
    assert_eq!(target_upload_column, 1);
    let migrated_plan: (String, i64, String, String) = connection
        .query_row(
            "SELECT manifest_digest, target_count, targets_json, sealed_at
             FROM context_document_purge_plans WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE_ID, RESOURCE_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read purge plan after successful migration");
    assert_eq!(migrated_plan, plan_before);
    let migrated_receipt: PurgeReceiptWithUploadRefs = connection
        .query_row(
            "SELECT replica_ref, replica_kind, runtime_id, runtime_incarnation_id,
                    target_revision_ids_json, status, requested_at, acknowledged_at,
                    receipt_digest, target_upload_chunk_refs_json
             FROM context_document_purge_receipts
             WHERE workspace_id = ?1 AND resource_id = ?2",
            params![WORKSPACE_ID, RESOURCE_ID],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                ))
            },
        )
        .expect("read migrated purge receipt");
    assert_eq!(
        migrated_receipt,
        (
            receipt_before.0.clone(),
            receipt_before.1.clone(),
            receipt_before.2.clone(),
            receipt_before.3.clone(),
            receipt_before.4.clone(),
            receipt_before.5.clone(),
            receipt_before.6.clone(),
            receipt_before.7.clone(),
            receipt_before.8.clone(),
            "[]".to_owned(),
        )
    );
    let accepted_registry: (i64, String, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT size_bytes, state, expires_at, claimed_at
             FROM resource_upload_chunk_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE_ID, accepted_upload_digest],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read backfilled accepted upload chunk registry");
    assert_eq!(accepted_registry, (14, "TRACKED".to_owned(), None, None));
    let reservation_registry: (i64, String, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT size_bytes, state, expires_at, claimed_at
             FROM resource_upload_chunk_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE_ID, reserved_digest],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read backfilled in-flight reservation registry");
    assert_eq!(
        reservation_registry,
        (
            14,
            "RESERVED".to_owned(),
            Some("2026-10-17T10:00:00Z".to_owned()),
            None,
        )
    );
    let accepted_chunk_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunks
             WHERE upload_id = 'upload-v18-accepted' AND temporary_blob_ref = ?1",
            [accepted_upload_digest],
            |row| row.get(0),
        )
        .expect("confirm accepted chunk ledger remains intact");
    assert_eq!(accepted_chunk_rows, 1);
    let upload_request_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunk_requests
             WHERE upload_id = 'upload-v18-accepted' AND request_id = 'request-v18-accepted'",
            [],
            |row| row.get(0),
        )
        .expect("confirm upload request ledger remains intact");
    assert_eq!(upload_request_rows, 1);
    assert_ne!(resource_digest, reserved_digest);
}
