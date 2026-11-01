use super::*;
use rusqlite::Connection;

#[test]
fn workspace_root_membership_migration_is_atomic_and_recorded() {
    let mut connection = Connection::open_in_memory().expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");

    migrate(&mut connection).expect("apply migration");

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, SCHEMA_VERSION);
    verify_migration_record(&connection, 23, V23_MIGRATION_NAME, SQLITE_V23_DDL, false)
        .expect("verify migration receipt");
    verify_migration_record(&connection, 26, V26_MIGRATION_NAME, SQLITE_V26_DDL, false)
        .expect("verify historical v26 migration receipt");
    verify_migration_record(&connection, 27, V27_MIGRATION_NAME, SQLITE_V27_DDL, true)
        .expect("verify latest v27 migration receipt");

    for table in [
        "workspace_root_file_locations",
        "workspace_root_cleanup_outbox",
    ] {
        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |row| row.get(0),
            )
            .expect("inspect migrated table");
        assert_eq!(table_count, 1, "missing table {table}");
    }
}

#[test]
fn workspace_root_membership_migration_rolls_back_and_replays_atomically() {
    let mut connection = Connection::open_in_memory().expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV23Migration)).is_err());
    let rolled_back_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read version after injected failure");
    assert_eq!(rolled_back_version, 22);
    let uncommitted_objects: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name IN (
               'workspace_root_file_locations', 'idx_workspace_root_file_locations_cleanup',
               'workspace_root_cleanup_outbox'
             )",
            [],
            |row| row.get(0),
        )
        .expect("inspect rolled-back migration objects");
    assert_eq!(uncommitted_objects, 0);
    let uncommitted_receipt: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version=23",
            [],
            |row| row.get(0),
        )
        .expect("inspect failed migration receipt");
    assert_eq!(uncommitted_receipt, 0);

    migrate(&mut connection).expect("retry migration");
    let migrated_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version after replay");
    assert_eq!(migrated_version, SCHEMA_VERSION);
    verify_migration_record(&connection, 23, V23_MIGRATION_NAME, SQLITE_V23_DDL, false)
        .expect("verify replayed v23 migration receipt");
    verify_migration_record(&connection, 24, V24_MIGRATION_NAME, SQLITE_V24_DDL, false)
        .expect("verify historical v24 migration receipt");
    verify_migration_record(&connection, 26, V26_MIGRATION_NAME, SQLITE_V26_DDL, false)
        .expect("verify historical v26 migration receipt");
    verify_migration_record(&connection, 27, V27_MIGRATION_NAME, SQLITE_V27_DDL, true)
        .expect("verify latest v27 migration receipt");
}

#[test]
fn workspace_root_membership_rows_are_workspace_scoped_and_cleanup_state_is_bounded() {
    let mut connection = Connection::open_in_memory().expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    migrate(&mut connection).expect("apply migrations");

    let relation_columns: Vec<String> = connection
        .prepare("PRAGMA table_info(workspace_root_file_locations)")
        .expect("inspect relation schema")
        .query_map([], |row| row.get(1))
        .expect("query relation columns")
        .collect::<Result<_, _>>()
        .expect("collect relation columns");
    assert_eq!(
        relation_columns,
        [
            "workspace_id",
            "workspace_root_id",
            "location_id",
            "resource_id",
            "observed_at"
        ]
    );

    let relation_sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='workspace_root_file_locations'",
            [],
            |row| row.get(0),
        )
        .expect("read membership table contract");
    assert!(relation_sql.contains("FOREIGN KEY (workspace_id, workspace_root_id)"));
    assert!(relation_sql.contains("FOREIGN KEY (resource_id, location_id)"));

    let outbox_sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='workspace_root_cleanup_outbox'",
            [],
            |row| row.get(0),
        )
        .expect("read cleanup outbox contract");
    assert!(outbox_sql.contains("status IN ('PENDING', 'COMPLETE')"));
    assert!(outbox_sql.contains("version > 0"));
}
