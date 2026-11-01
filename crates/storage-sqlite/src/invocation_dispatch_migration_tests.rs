//! Version twenty-six opens Invocation creation to the combined Trust, ApprovalUse,
//! Invocation and Effect admission writer and closes the direct-dispatch path with the
//! consumed-approval guard. These tests state the boundary that the migration
//! establishes, so a later change cannot quietly reopen it.

use super::*;
use rusqlite::Connection;

fn trigger_exists(connection: &Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name=?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .expect("inspect trigger")
        > 0
}

#[test]
fn invocation_dispatch_migration_is_atomic_and_installs_the_approval_bound() {
    let mut connection = Connection::open_in_memory().expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");

    // The migration is one transaction: an interruption before its commit leaves the
    // database at the previous version with none of the new triggers installed.
    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV26Migration)).is_err());
    let rolled_back_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read version after injected failure");
    assert_eq!(rolled_back_version, 25);
    assert!(
        !trigger_exists(&connection, "capability_invocation_dispatch_requires_consumed_approval"),
        "an interrupted migration must not leave the dispatch guard installed"
    );

    migrate(&mut connection).expect("apply the invocation dispatch migration");
    verify_migration_record(&connection, 26, V26_MIGRATION_NAME, SQLITE_V26_DDL, false)
        .expect("verify the latest migration receipt");
    verify_migration_record(&connection, 27, V27_MIGRATION_NAME, SQLITE_V27_DDL, true)
        .expect("verify latest v27 migration receipt");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read current schema version");
    assert_eq!(version, SCHEMA_VERSION);

    // The blanket creation block is gone, so the combined writer may create an
    // Invocation, and the consumed-approval guard is installed in its place.
    assert!(
        !trigger_exists(&connection, "capability_invocation_creation_admission_closed"),
        "the blanket creation block must be removed by the migration"
    );
    assert!(
        trigger_exists(&connection, "capability_invocation_dispatch_requires_consumed_approval"),
        "the dispatch approval guard must be installed by the migration"
    );
    assert!(
        trigger_exists(&connection, "capability_invocation_status_transition_guard"),
        "the transition guard must still be installed after the migration"
    );

    // The version one authority boundary is untouched by the migration.
    for trigger in [
        "capability_invocation_admission_matches_scope",
        "capability_invocation_first_dispatch_revalidates_owner",
        "capability_invocation_identity_immutable",
    ] {
        assert!(
            trigger_exists(&connection, trigger),
            "the migration must not remove the authority trigger {trigger}"
        );
    }
}

#[test]
fn the_migration_keeps_the_version_one_authority_boundary_installed() {
    let mut connection = Connection::open_in_memory().expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    migrate(&mut connection).expect("apply the invocation dispatch migration");

    // The version one authority boundary is untouched by the migration.
    for trigger in [
        "capability_invocation_admission_matches_scope",
        "capability_invocation_first_dispatch_revalidates_owner",
        "capability_invocation_identity_immutable",
    ] {
        assert!(
            trigger_exists(&connection, trigger),
            "the migration must not remove the authority trigger {trigger}"
        );
    }

    // The transition guard keeps its version rule and its state machine, and only adds
    // the CREATED to DISPATCHED step.
    let guard_sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='capability_invocation_status_transition_guard'",
            [],
            |row| row.get(0),
        )
        .expect("read the transition guard");
    assert!(guard_sql.contains("OLD.status = 'CREATED' AND NEW.status IN ('DISPATCHED', 'CANCELLED')"));
    assert!(guard_sql.contains("NEW.version <> OLD.version + 1"));
    assert!(!guard_sql.contains("NEW.status = 'DISPATCHED' OR"));
}
