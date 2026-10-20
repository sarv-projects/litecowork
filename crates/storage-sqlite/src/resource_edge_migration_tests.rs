use super::*;
use rusqlite::Connection;

#[test]
fn resource_edge_migration_is_atomic_and_adds_immutable_guards() {
    let mut connection = Connection::open_in_memory().expect("open migration database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");

    assert!(migrate_with_failpoint(&mut connection, Some(Failpoint::DuringV25Migration)).is_err());
    let rolled_back_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read version after injected failure");
    assert_eq!(rolled_back_version, 24);
    let rolled_back_trigger: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name='resource_edges_immutable_update'",
            [],
            |row| row.get(0),
        )
        .expect("inspect rolled-back trigger");
    assert_eq!(rolled_back_trigger, 0);

    migrate(&mut connection).expect("apply ResourceEdge migration");
    verify_migration_record(&connection, 25, V25_MIGRATION_NAME, SQLITE_V25_DDL, true)
        .expect("verify migration receipt");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read current schema version");
    assert_eq!(version, 25);

    for trigger in [
        "resource_edges_immutable_update",
        "resource_edges_immutable_delete",
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
}
