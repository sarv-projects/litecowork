use super::*;
use rusqlite::Connection;

#[test]
fn schema_pins_evidence_to_its_verification_run() {
    let mut connection = Connection::open_in_memory().expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV22Migration)).is_err());

    let rolled_back_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read version after injected failure");
    assert_eq!(rolled_back_version, 21);
    let rolled_back_column: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('evidence') WHERE name='verification_run_id'",
            [],
            |row| row.get(0),
        )
        .expect("inspect rolled-back Evidence schema");
    assert_eq!(rolled_back_column, 0);
    let uncommitted_receipt: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version=22",
            [],
            |row| row.get(0),
        )
        .expect("inspect failed migration receipt");
    assert_eq!(uncommitted_receipt, 0);

    migrate(&mut connection).expect("retry migration");

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, SCHEMA_VERSION);

    let column_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('evidence') WHERE name='verification_run_id'",
            [],
            |row| row.get(0),
        )
        .expect("inspect Evidence run binding");
    assert_eq!(column_count, 1);

    verify_migration_record(&connection, 22, V22_MIGRATION_NAME, SQLITE_V22_DDL, false)
        .expect("verify v22 migration receipt");
    verify_migration_record(&connection, 23, V23_MIGRATION_NAME, SQLITE_V23_DDL, false)
        .expect("verify v23 migration receipt");
    verify_migration_record(&connection, 24, V24_MIGRATION_NAME, SQLITE_V24_DDL, false)
        .expect("verify historical v24 migration receipt");
    verify_migration_record(&connection, 26, V26_MIGRATION_NAME, SQLITE_V26_DDL, true)
        .expect("verify latest migration receipt");
}

#[test]
fn verification_pass_requires_every_verified_evidence_ref_to_match_the_exact_run() {
    let connection = Connection::open_in_memory().expect("open minimal contract database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    connection
        .execute_batch(
            "CREATE TABLE verification_runs(
               verification_run_id TEXT PRIMARY KEY,
               task_id TEXT NOT NULL,
               verifier_kind TEXT NOT NULL,
               status TEXT NOT NULL,
               evidence_refs_json TEXT NOT NULL
             );
             CREATE TABLE evidence(
               evidence_id TEXT PRIMARY KEY,
               task_id TEXT NOT NULL,
               level TEXT NOT NULL,
               subject_ref TEXT NOT NULL,
               producer_json TEXT NOT NULL
             );
             CREATE TABLE effects(
               effect_id TEXT PRIMARY KEY,
               task_id TEXT NOT NULL,
               state TEXT NOT NULL,
               verification_ref TEXT
             );",
        )
        .expect("create minimal pre-migration schema");
    connection
        .execute_batch(SQLITE_V22_DDL)
        .expect("apply run-binding guards");

    connection
        .execute(
            "INSERT INTO verification_runs VALUES ('run-1', 'task-1', 'verifier-1', 'RUNNING', '[]')",
            [],
        )
        .expect("insert active run");
    connection
        .execute(
            "INSERT INTO evidence(evidence_id, task_id, level, subject_ref, producer_json, verification_run_id)
             VALUES ('evidence-1', 'task-1', 'VERIFIED', 'task:task-1', '{\"service_id\":\"verifier-1\"}', 'run-1')",
            [],
        )
        .expect("insert evidence for exact active run");
    connection
        .execute(
            "UPDATE verification_runs SET status='PASSED', evidence_refs_json='[\"evidence-1\"]'
             WHERE verification_run_id='run-1'",
            [],
        )
        .expect("settle only with exact same-run verified evidence");

    connection
        .execute(
            "INSERT INTO verification_runs VALUES ('run-2', 'task-1', 'verifier-2', 'RUNNING', '[]')",
            [],
        )
        .expect("insert second active run");
    let mismatched_task = connection.execute(
        "INSERT INTO evidence(evidence_id, task_id, level, subject_ref, producer_json, verification_run_id)
         VALUES ('evidence-2', 'task-2', 'VERIFIED', 'task:task-2', '{\"service_id\":\"verifier-2\"}', 'run-2')",
        [],
    );
    assert!(mismatched_task.is_err());

    let cross_run_pass = connection.execute(
        "UPDATE verification_runs SET status='PASSED', evidence_refs_json='[\"evidence-1\"]'
         WHERE verification_run_id='run-2'",
        [],
    );
    assert!(cross_run_pass.is_err());
}
