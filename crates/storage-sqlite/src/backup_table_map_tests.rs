use rusqlite::Connection;

use crate::backup::validate_backup_table_map;

fn migrated_connection() -> Connection {
    let mut connection = Connection::open_in_memory().expect("open in-memory SQLite");
    super::migrate(&mut connection).expect("apply the current SQLite migration chain");
    connection
}

#[test]
fn every_current_migrated_application_table_has_one_valid_backup_classification() {
    let connection = migrated_connection();

    validate_backup_table_map(&connection)
        .expect("all application tables are classified and their filters compile");
}

#[test]
fn backup_table_map_rejects_a_new_unclassified_application_table() {
    let connection = migrated_connection();
    connection
        .execute_batch("CREATE TABLE unclassified_backup_probe (value TEXT)")
        .expect("create an unclassified application table");

    let error = validate_backup_table_map(&connection)
        .expect_err("a new table must not be included by default");
    assert!(
        error.to_string().contains("unclassified_backup_probe"),
        "the error identifies the table that needs an explicit classification: {error}"
    );
}

#[test]
fn credential_submission_recovery_receipts_are_excluded_from_workspace_backups() {
    let connection = migrated_connection();

    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'agent_credential_submission_receipts'",
            [],
            |row| row.get(0),
        )
        .expect("inspect the migrated credential receipt table");
    assert_eq!(count, 1, "v36 must create the durable replay receipt");

    validate_backup_table_map(&connection)
        .expect("the Runtime-local receipt has an explicit backup classification");
    assert!(storage_core::BACKUP_EXCLUDED_TABLES.contains(&"agent_credential_submission_receipts"));
    assert!(
        !storage_core::BACKUP_WORKSPACE_SCOPED_TABLES
            .iter()
            .any(|table| table.table == "agent_credential_submission_receipts")
    );
}
