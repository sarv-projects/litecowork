use super::*;
use rusqlite::Connection;

#[test]
fn delegation_result_migration_is_atomic_and_records_immutable_storage() {
    let mut connection = Connection::open_in_memory().expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV24Migration)).is_err());
    let rolled_back_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read version after injected failure");
    assert_eq!(rolled_back_version, 23);
    let rolled_back_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='delegation_results'",
            [],
            |row| row.get(0),
        )
        .expect("inspect rolled-back table");
    assert_eq!(rolled_back_table, 0);
    let rolled_back_receipt: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version=24",
            [],
            |row| row.get(0),
        )
        .expect("inspect rolled-back migration receipt");
    assert_eq!(rolled_back_receipt, 0);

    migrate(&mut connection).expect("replay ResultEnvelope migration");
    verify_migration_record(&connection, 24, V24_MIGRATION_NAME, SQLITE_V24_DDL, false)
        .expect("verify historical v24 migration receipt");
    verify_migration_record(&connection, 26, V26_MIGRATION_NAME, SQLITE_V26_DDL, false)
        .expect("verify latest v26 migration receipt");
    verify_migration_record(&connection, 27, V27_MIGRATION_NAME, SQLITE_V27_DDL, true)
        .expect("verify latest v27 migration receipt");

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read current schema version");
    assert_eq!(version, SCHEMA_VERSION);

    for trigger in [
        "delegation_results_immutable_update",
        "delegation_results_immutable_delete",
    ] {
        let exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name=?1",
                [trigger],
                |row| row.get(0),
            )
            .expect("inspect immutable trigger");
        assert_eq!(exists, 1, "missing trigger {trigger}");
    }

    let foreign_key_violations: i64 = connection
        .prepare("PRAGMA foreign_key_check")
        .expect("prepare foreign key check")
        .query_map([], |_| Ok(()))
        .expect("run foreign key check")
        .count() as i64;
    assert_eq!(foreign_key_violations, 0);
}
