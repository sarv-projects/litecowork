use super::*;
use rusqlite::{params, Connection};
use std::time::Duration;

const OWNER: &str = "owner-local";
const WORKSPACE: &str = "workspace-notifications";
const TIED_AT: &str = "2026-10-21T09:00:00Z";

fn fixture_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
    let store = test_store(directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE);
    store
}

fn fixture_connection(directory: &tempfile::TempDir) -> Connection {
    let connection = Connection::open(state_database(directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("fixture rows are inserted without their source event");
    connection
}

fn insert_delivery(
    connection: &Connection,
    delivery_id: &str,
    status: &str,
    created_at: &str,
) -> String {
    let dedupe_key = format!("{delivery_id}-dedupe");
    connection
        .execute(
            "INSERT INTO notification_deliveries(delivery_id, workspace_id, dedupe_key, \
                 source_event_id, status, attempt_count, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![
                delivery_id,
                WORKSPACE,
                dedupe_key,
                format!("{delivery_id}-event"),
                status,
                created_at
            ],
        )
        .expect("delivery fixture inserts");
    dedupe_key
}

fn insert_preference(
    connection: &Connection,
    event_class: &str,
    policy: &str,
    channels: &str,
    quiet_hours: Option<&str>,
    version: u64,
) {
    connection
        .execute(
            "INSERT INTO notification_preferences(workspace_id, event_class, policy, \
                 preferred_channels_json, quiet_hours_json, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                WORKSPACE,
                event_class,
                policy,
                channels,
                quiet_hours,
                i64::try_from(version).expect("test version fits")
            ],
        )
        .expect("preference fixture inserts");
}

fn delivery_ids(deliveries: &[storage_core::NotificationRecord]) -> Vec<&str> {
    deliveries
        .iter()
        .map(|delivery| delivery.delivery_id.as_str())
        .collect()
}

#[test]
fn a_page_boundary_on_a_tied_timestamp_does_not_repeat_or_skip_a_delivery() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    let connection = fixture_connection(&directory);
    insert_delivery(&connection, "delivery-b", "PENDING", TIED_AT);
    insert_delivery(&connection, "delivery-a", "SENT", TIED_AT);
    insert_delivery(&connection, "delivery-c", "FAILED", "2026-10-21T10:00:00Z");

    let first = store
        .list_notification_deliveries(OWNER, WORKSPACE, None, None, None, 2)
        .expect("first page reads");
    assert_eq!(delivery_ids(&first), ["delivery-a", "delivery-b"]);

    let boundary = first.last().expect("page is not empty");
    let second = store
        .list_notification_deliveries(
            OWNER,
            WORKSPACE,
            None,
            Some(&boundary.created_at),
            Some(&boundary.delivery_id),
            2,
        )
        .expect("second page reads");
    assert_eq!(delivery_ids(&second), ["delivery-c"]);
}

#[test]
fn the_status_filter_selects_only_the_requested_state() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    let connection = fixture_connection(&directory);
    insert_delivery(&connection, "delivery-a", "SENT", TIED_AT);
    insert_delivery(&connection, "delivery-b", "FAILED", TIED_AT);

    let failed = store
        .list_notification_deliveries(OWNER, WORKSPACE, Some("FAILED"), None, None, 50)
        .expect("filtered page reads");
    assert_eq!(delivery_ids(&failed), ["delivery-b"]);
}

#[test]
fn an_archived_workspace_keeps_its_delivery_history_readable() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    let connection = fixture_connection(&directory);
    insert_delivery(&connection, "delivery-a", "SENT", TIED_AT);
    connection
        .execute(
            "UPDATE workspaces SET status = 'ARCHIVED' WHERE workspace_id = ?1",
            params![WORKSPACE],
        )
        .expect("archive fixture applies");

    let deliveries = store
        .list_notification_deliveries(OWNER, WORKSPACE, None, None, None, 50)
        .expect("archived history stays readable");
    assert_eq!(delivery_ids(&deliveries), ["delivery-a"]);
}

#[test]
fn a_non_owner_cannot_distinguish_a_missing_workspace_from_a_foreign_one() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);

    assert_eq!(
        store.list_notification_deliveries(
            "owner-foreign",
            WORKSPACE,
            None,
            None,
            None,
            50
        ),
        Err(StoreError::NotFound)
    );
    assert_eq!(
        store.list_notification_preferences("owner-foreign", WORKSPACE),
        Err(StoreError::NotFound)
    );
}

#[test]
fn a_half_cursor_or_an_out_of_range_limit_is_rejected() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);

    assert!(matches!(
        store.list_notification_deliveries(OWNER, WORKSPACE, None, Some(TIED_AT), None, 50),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.list_notification_deliveries(OWNER, WORKSPACE, None, None, None, 0),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.list_notification_deliveries(OWNER, WORKSPACE, None, None, None, 202),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.list_notification_deliveries(OWNER, WORKSPACE, Some("DELIVERED"), None, None, 50),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn preferences_are_owner_scoped_ordered_and_parsed_from_their_stored_json() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    let connection = fixture_connection(&directory);
    insert_preference(
        &connection,
        "TASK_FAILED",
        "ON_FAILURE",
        r#"["channel-primary","channel-backup"]"#,
        Some(
            r#"{"timezone":"Europe/Berlin","weekdays":["MONDAY","TUESDAY"],"start_local_time":"22:00","end_local_time":"07:00"}"#,
        ),
        3,
    );
    insert_preference(
        &connection,
        "AUTOMATION_OCCURRENCE_SETTLED",
        "ALWAYS",
        "[]",
        None,
        1,
    );

    let preferences = store
        .list_notification_preferences(OWNER, WORKSPACE)
        .expect("preferences read");
    assert_eq!(preferences.len(), 2);
    assert_eq!(
        preferences[0].event_class,
        "AUTOMATION_OCCURRENCE_SETTLED"
    );
    assert_eq!(preferences[0].policy, "ALWAYS");
    assert!(preferences[0].preferred_channels.is_empty());
    assert!(preferences[0].quiet_hours.is_none());
    assert_eq!(preferences[1].event_class, "TASK_FAILED");
    assert_eq!(
        preferences[1].preferred_channels,
        ["channel-primary", "channel-backup"]
    );
    let quiet_hours = preferences[1].quiet_hours.as_ref().expect("quiet hours");
    assert_eq!(quiet_hours.timezone, "Europe/Berlin");
    assert_eq!(quiet_hours.start_local_time, "22:00");
    assert_eq!(quiet_hours.end_local_time, "07:00");
    assert_eq!(preferences[1].version, 3);
}

#[test]
fn unreadable_preference_json_fails_closed_instead_of_dropping_the_policy() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    let connection = fixture_connection(&directory);
    insert_preference(&connection, "TASK_FAILED", "ON_FAILURE", "not-json", None, 1);

    assert!(matches!(
        store.list_notification_preferences(OWNER, WORKSPACE),
        Err(StoreError::Integrity(_))
    ));
}
