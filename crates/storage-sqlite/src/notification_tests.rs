use super::*;
use rusqlite::{Connection, params};
use std::time::Duration;
use storage_core::QuietHours;

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
        store.list_notification_deliveries("owner-foreign", WORKSPACE, None, None, None, 50),
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
    assert_eq!(preferences[0].event_class, "AUTOMATION_OCCURRENCE_SETTLED");
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
    insert_preference(
        &connection,
        "TASK_FAILED",
        "ON_FAILURE",
        "not-json",
        None,
        1,
    );

    assert!(matches!(
        store.list_notification_preferences(OWNER, WORKSPACE),
        Err(StoreError::Integrity(_))
    ));
}

fn event(event_id: &str) -> NotificationEventContext {
    NotificationEventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        hlc_timestamp: TIED_AT.to_owned(),
        correlation_id: "request-correlation".to_owned(),
        causation_id: None,
        recorded_at: TIED_AT.to_owned(),
    }
}

fn write_policy(
    store: &SqliteWorkspaceStore,
    policy: &str,
    expected_version: u64,
    request_id: &str,
    event_id: &str,
) -> Result<storage_core::NotificationPreferenceRecord, StoreError> {
    store.set_notification_preference(
        OWNER,
        WORKSPACE,
        "TASK_FAILED",
        policy,
        &["channel-primary".to_owned()],
        None,
        expected_version,
        request_id,
        &event(event_id),
    )
}

#[test]
fn a_first_policy_write_records_the_row_the_event_and_the_receipt() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    let record = write_policy(&store, "ON_FAILURE", 0, "request-first", "event-first")
        .expect("first write commits");
    assert_eq!(record.version, 1);
    assert_eq!(record.policy, "ON_FAILURE");
    assert_eq!(record.preferred_channels, ["channel-primary"]);
    assert!(record.quiet_hours.is_none());

    let connection = fixture_connection(&directory);
    let (policy, channels, version): (String, String, i64) = connection
        .query_row(
            "SELECT policy, preferred_channels_json, version FROM notification_preferences \
             WHERE workspace_id = ?1 AND event_class = ?2",
            params![WORKSPACE, "TASK_FAILED"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("policy row reads");
    assert_eq!(policy, "ON_FAILURE");
    assert_eq!(channels, r#"["channel-primary"]"#);
    assert_eq!(version, 1);

    let (event_type, payload): (String, String) = connection
        .query_row(
            "SELECT type, payload_json FROM domain_events \
             WHERE entity_type = 'NotificationPreference' AND entity_id = 'TASK_FAILED'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("event reads");
    assert_eq!(event_type, "notification.preference.changed.v1");
    let payload: serde_json::Value = serde_json::from_str(&payload).expect("payload parses");
    let mut keys: Vec<&str> = payload
        .as_object()
        .expect("payload is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "aggregate_version",
            "event_class",
            "policy",
            "preferred_channels",
            "workspace_id",
        ]
    );
    assert_eq!(payload["aggregate_version"], 1);
    assert_eq!(payload["policy"], "ON_FAILURE");
    assert_eq!(payload["preferred_channels"][0], "channel-primary");
}

#[test]
fn a_stale_expected_version_is_a_conflict_and_writes_nothing() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    write_policy(&store, "ON_FAILURE", 0, "request-first", "event-first")
        .expect("first write commits");

    let error = write_policy(&store, "ALWAYS", 0, "request-stale", "event-stale")
        .expect_err("a stale version is refused");
    assert_eq!(
        error,
        StoreError::Conflict {
            expected: Some(0),
            actual: Some(1),
        }
    );

    let connection = fixture_connection(&directory);
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type = 'NotificationPreference'",
            [],
            |row| row.get(0),
        )
        .expect("event count reads");
    assert_eq!(events, 1);
    let policy: String = connection
        .query_row(
            "SELECT policy FROM notification_preferences WHERE workspace_id = ?1 AND event_class = ?2",
            params![WORKSPACE, "TASK_FAILED"],
            |row| row.get(0),
        )
        .expect("policy row reads");
    assert_eq!(policy, "ON_FAILURE");
}

#[test]
fn an_idempotent_replay_returns_the_committed_record_without_a_second_event() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    let first = write_policy(&store, "ON_FAILURE", 0, "request-replay", "event-first")
        .expect("first write commits");
    let replay = write_policy(&store, "ON_FAILURE", 0, "request-replay", "event-replay")
        .expect("the replay returns the committed record");

    assert_eq!(first, replay);
    let connection = fixture_connection(&directory);
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type = 'NotificationPreference'",
            [],
            |row| row.get(0),
        )
        .expect("event count reads");
    assert_eq!(events, 1);
    let replays: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE event_id = 'event-replay'",
            [],
            |row| row.get(0),
        )
        .expect("replay event count reads");
    assert_eq!(replays, 0);
}

#[test]
fn reusing_a_key_for_a_different_policy_is_an_idempotency_conflict() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    write_policy(&store, "ON_FAILURE", 0, "request-reused", "event-first")
        .expect("first write commits");

    let error = write_policy(&store, "ALWAYS", 0, "request-reused", "event-reused")
        .expect_err("a reused key with a different request is refused");
    assert_eq!(error, StoreError::IdempotencyConflict);
}

#[test]
fn an_archived_workspace_rejects_a_policy_write_without_recording_anything() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    let connection = fixture_connection(&directory);
    connection
        .execute(
            "UPDATE workspaces SET status = 'ARCHIVED' WHERE workspace_id = ?1",
            params![WORKSPACE],
        )
        .expect("archive fixture applies");

    let error = write_policy(&store, "ALWAYS", 0, "request-archived", "event-archived")
        .expect_err("an archived Workspace refuses the write");
    assert!(
        matches!(error, StoreError::Invalid(ref message) if message == "WORKSPACE_ARCHIVED"),
        "{error:?}"
    );

    let policies: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM notification_preferences WHERE workspace_id = ?1",
            params![WORKSPACE],
            |row| row.get(0),
        )
        .expect("policy count reads");
    assert_eq!(policies, 0);
    let receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id = 'request-archived'",
            [],
            |row| row.get(0),
        )
        .expect("receipt count reads");
    assert_eq!(receipts, 0);
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type = 'NotificationPreference'",
            [],
            |row| row.get(0),
        )
        .expect("event count reads");
    assert_eq!(events, 0);
}

#[test]
fn a_foreign_owner_cannot_write_a_policy() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);

    let error = store
        .set_notification_preference(
            "owner-foreign",
            WORKSPACE,
            "TASK_FAILED",
            "ALWAYS",
            &[],
            None,
            0,
            "request-foreign",
            &event("event-foreign"),
        )
        .expect_err("a foreign owner is refused");
    assert_eq!(error, StoreError::NotFound);
}

#[test]
fn a_quiet_window_is_persisted_and_read_back() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);
    let quiet_hours = QuietHours {
        timezone: "Europe/Berlin".to_owned(),
        weekdays: vec!["MONDAY".to_owned(), "TUESDAY".to_owned()],
        start_local_time: "22:00".to_owned(),
        end_local_time: "07:00".to_owned(),
    };
    store
        .set_notification_preference(
            OWNER,
            WORKSPACE,
            "TASK_FAILED",
            "ON_FAILURE",
            &["channel-primary".to_owned()],
            Some(&quiet_hours),
            0,
            "request-quiet",
            &event("event-quiet"),
        )
        .expect("quiet window writes");

    let preferences = store
        .list_notification_preferences(OWNER, WORKSPACE)
        .expect("preferences read");
    assert_eq!(preferences.len(), 1);
    assert_eq!(preferences[0].quiet_hours.as_ref(), Some(&quiet_hours));
}

#[test]
fn invalid_policy_values_are_rejected_before_any_write() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = fixture_store(&directory);

    let cases = [
        QuietHours {
            timezone: "Europe/Berlin".to_owned(),
            weekdays: vec!["MONDAY".to_owned()],
            start_local_time: "24:00".to_owned(),
            end_local_time: "07:00".to_owned(),
        },
        QuietHours {
            timezone: "Europe/Berlin".to_owned(),
            weekdays: vec!["FUNDAY".to_owned()],
            start_local_time: "22:00".to_owned(),
            end_local_time: "07:00".to_owned(),
        },
        QuietHours {
            timezone: "Europe/Berlin".to_owned(),
            weekdays: vec!["MONDAY".to_owned(), "MONDAY".to_owned()],
            start_local_time: "22:00".to_owned(),
            end_local_time: "07:00".to_owned(),
        },
        QuietHours {
            timezone: "Europe/Berlin".to_owned(),
            weekdays: vec!["MONDAY".to_owned()],
            // A five-byte value whose first character is not ASCII must not panic
            // in the fixed-offset time parser.
            start_local_time: "\u{20ac}:0".to_owned(),
            end_local_time: "07:00".to_owned(),
        },
    ];
    for (index, quiet_hours) in cases.iter().enumerate() {
        let error = store
            .set_notification_preference(
                OWNER,
                WORKSPACE,
                "TASK_FAILED",
                "ON_FAILURE",
                &["channel-primary".to_owned()],
                Some(quiet_hours),
                0,
                &format!("request-invalid-{index}"),
                &event(&format!("event-invalid-{index}")),
            )
            .expect_err("invalid quiet hours are refused");
        assert!(
            matches!(error, StoreError::Invalid(_)),
            "case {index}: {error:?}"
        );
    }

    assert!(matches!(
        write_policy(&store, "SOMETIMES", 0, "request-policy", "event-policy"),
        Err(StoreError::Invalid(_))
    ));

    let connection = fixture_connection(&directory);
    let policies: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM notification_preferences WHERE workspace_id = ?1",
            params![WORKSPACE],
            |row| row.get(0),
        )
        .expect("policy count reads");
    assert_eq!(policies, 0);
}
