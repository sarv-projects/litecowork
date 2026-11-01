use super::*;
use sha2::{Digest, Sha256};

const NOW: &str = "2026-10-10T10:00:00Z";

fn registration(
    incarnation_id: &str,
) -> (
    RuntimeRecord,
    RuntimeIncarnationRecord,
    RuntimeIncarnationLocalObservationRecord,
) {
    let public_key = [7_u8; 32];
    let device_id = format!("ed25519-sha256:{}", hex::encode(Sha256::digest(public_key)));
    (
        RuntimeRecord {
            runtime_id: "runtime-lifecycle-test".to_owned(),
            device_identity: DeviceIdentityRecord {
                device_id,
                public_key: format!("ed25519:{}", hex::encode(public_key)),
                key_version: 1,
                issued_at: NOW.to_owned(),
                display_name: None,
            },
            runtime_version: "0.1.0-test".to_owned(),
            platform: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
            roles: vec!["OPERATOR_ENDPOINT".to_owned()],
            trust_zone: "PERSONAL_DEVICE".to_owned(),
            availability: "RECOVERING".to_owned(),
            startup_policy: "MANUAL".to_owned(),
            current_incarnation_id: incarnation_id.to_owned(),
            resource_capacity: json!({"sampled_at": NOW}),
            last_seen: NOW.to_owned(),
        },
        RuntimeIncarnationRecord {
            runtime_incarnation_id: incarnation_id.to_owned(),
            runtime_id: "runtime-lifecycle-test".to_owned(),
            process_started_at: NOW.to_owned(),
            litecowork_version: "0.1.0-test".to_owned(),
            recovered_from_unclean_shutdown: false,
            recovery_state: "RECOVERING".to_owned(),
            ready_at: None,
            stopped_at: None,
            version: 1,
        },
        RuntimeIncarnationLocalObservationRecord {
            runtime_incarnation_id: incarnation_id.to_owned(),
            os_boot_id: Some("boot-test".to_owned()),
            observed_at: NOW.to_owned(),
        },
    )
}

fn update(
    incarnation_id: &str,
    expected_version: u64,
    recovery_state: &str,
    stopped_at: Option<&str>,
) -> RuntimeIncarnationStateUpdate {
    RuntimeIncarnationStateUpdate {
        runtime_id: "runtime-lifecycle-test".to_owned(),
        runtime_incarnation_id: incarnation_id.to_owned(),
        expected_version,
        recovery_state: recovery_state.to_owned(),
        availability: match recovery_state {
            "RECOVERING" => "RECOVERING",
            "DEGRADED" => "DEGRADED",
            "DRAINING" | "STOPPING" => "DRAINING",
            "STOPPED" => "OFFLINE",
            _ => unreachable!("test transition is known"),
        }
        .to_owned(),
        observed_at: NOW.to_owned(),
        stopped_at: stopped_at.map(str::to_owned),
    }
}

#[test]
fn runtime_shutdown_transitions_are_durable_versioned_and_fenced() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let (runtime, incarnation, observation) = registration("incarnation-first");
    assert_eq!(
        store
            .register_local_incarnation(runtime, incarnation, observation)
            .expect("Runtime incarnation registers")
            .version,
        1
    );

    assert_eq!(
        store
            .transition_local_incarnation(update("incarnation-first", 1, "DEGRADED", None,))
            .expect("registered incarnation degrades")
            .version,
        2
    );
    assert_eq!(
        store
            .transition_local_incarnation(update("incarnation-first", 2, "DRAINING", None,))
            .expect("degraded incarnation drains")
            .version,
        3
    );

    assert!(matches!(
        store.transition_local_incarnation(update("incarnation-first", 3, "STOPPED", Some(NOW),)),
        Err(StoreError::Invalid(_))
    ));
    let database = state_database(&directory);
    let connection = Connection::open(&database).expect("open lifecycle projection");
    let row = connection
        .query_row(
            "SELECT i.recovery_state, i.version, i.stopped_at, r.availability FROM runtime_incarnations i JOIN runtimes r ON r.runtime_id = i.runtime_id WHERE i.runtime_incarnation_id = 'incarnation-first'",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .expect("read lifecycle after rejected transition");
    assert_eq!(row, ("DRAINING".to_owned(), 3, None, "DRAINING".to_owned()));
    drop(connection);

    assert_eq!(
        store
            .transition_local_incarnation(update("incarnation-first", 3, "STOPPING", None,))
            .expect("draining incarnation stops")
            .version,
        4
    );
    assert!(matches!(
        store.transition_local_incarnation(update("incarnation-first", 3, "STOPPING", None,)),
        Err(StoreError::Conflict {
            expected: Some(3),
            actual: Some(4),
        })
    ));
    let stopped = store
        .transition_local_incarnation(update("incarnation-first", 4, "STOPPED", Some(NOW)))
        .expect("stopping incarnation becomes stopped");
    assert_eq!(stopped.version, 5);
    assert_eq!(stopped.recovery_state, "STOPPED");
    assert_eq!(stopped.stopped_at.as_deref(), Some(NOW));

    let (next_runtime, next_incarnation, next_observation) = registration("incarnation-next");
    store
        .register_local_incarnation(next_runtime, next_incarnation, next_observation)
        .expect("new process registers a new current incarnation");
    assert!(matches!(
        store.transition_local_incarnation(update("incarnation-first", 5, "STOPPING", None,)),
        Err(StoreError::Integrity(_))
    ));

    let connection = Connection::open(database).expect("reopen lifecycle projection");
    let stale_state: (String, i64) = connection
        .query_row(
            "SELECT recovery_state, version FROM runtime_incarnations WHERE runtime_incarnation_id = 'incarnation-first'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("old incarnation remains unchanged");
    assert_eq!(stale_state, ("STOPPED".to_owned(), 5));
}

#[test]
fn lifecycle_snapshot_is_current_incarnation_scoped_and_reads_latest_catalog_state() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let (runtime, incarnation, observation) = registration("incarnation-first");
    store
        .register_local_incarnation(runtime, incarnation, observation)
        .expect("Runtime incarnation registers");

    store
        .transition_local_incarnation(update("incarnation-first", 1, "DEGRADED", None))
        .expect("incarnation transitions to degraded");

    let snapshot = store
        .read_local_lifecycle_snapshot("runtime-lifecycle-test", "incarnation-first")
        .expect("lifecycle snapshot query succeeds")
        .expect("the exact current incarnation is present");
    assert_eq!(snapshot.runtime_id, "runtime-lifecycle-test");
    assert_eq!(snapshot.startup_policy, "MANUAL");
    assert_eq!(snapshot.availability, "DEGRADED");
    assert_eq!(
        snapshot.incarnation.runtime_incarnation_id,
        "incarnation-first"
    );
    assert_eq!(snapshot.incarnation.recovery_state, "DEGRADED");
    assert_eq!(snapshot.incarnation.version, 2);
    assert!(
        store
            .read_local_lifecycle_snapshot("another-runtime", "incarnation-first")
            .expect("unknown Runtime lookup succeeds")
            .is_none()
    );

    let (next_runtime, next_incarnation, next_observation) = registration("incarnation-next");
    store
        .register_local_incarnation(next_runtime, next_incarnation, next_observation)
        .expect("new process registers a new current incarnation");
    assert!(
        store
            .read_local_lifecycle_snapshot("runtime-lifecycle-test", "incarnation-first")
            .expect("superseded incarnation lookup succeeds")
            .is_none()
    );
    assert_eq!(
        store
            .read_local_lifecycle_snapshot("runtime-lifecycle-test", "incarnation-next")
            .expect("current incarnation lookup succeeds")
            .expect("new incarnation is current")
            .incarnation
            .runtime_incarnation_id,
        "incarnation-next"
    );
}

#[test]
fn superseded_runtime_incarnation_cannot_activate_an_execution_lease() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let (runtime, first_incarnation, first_observation) = registration("incarnation-first");
    store
        .register_local_incarnation(runtime, first_incarnation, first_observation)
        .expect("first Runtime incarnation registers");

    let database = state_database(&directory);
    let fixture_connection = Connection::open(&database).expect("open Runtime catalog fixture");
    fixture_connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable only unrelated fixture references");
    fixture_connection
        .execute(
            "INSERT INTO execution_leases(lease_id, task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id, epoch, issuer_key_version, fencing_token_digest, state, acquired_at, renew_by, expires_at, version)
             VALUES ('stale-lease', 'missing-task', 'missing-step', 'missing-attempt', 'runtime-lifecycle-test', 'incarnation-first', 1, 1, ?1, 'ACTIVE', ?2, ?2, ?2, 1)",
            params![format!("sha256:{}", "a".repeat(64)), NOW],
        )
        .expect("current incarnation can hold an active lease");
    drop(fixture_connection);

    let (next_runtime, next_incarnation, next_observation) = registration("incarnation-next");
    store
        .register_local_incarnation(next_runtime, next_incarnation, next_observation)
        .expect("new process becomes the current Runtime incarnation");

    let connection = Connection::open(database).expect("open Runtime catalog");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enable production foreign-key checks");
    let error = connection
        .execute(
            "INSERT INTO execution_leases(lease_id, task_id, step_id, attempt_id, runtime_id, runtime_incarnation_id, epoch, issuer_key_version, fencing_token_digest, state, acquired_at, renew_by, expires_at, version)
             VALUES ('stale-lease', 'missing-task', 'missing-step', 'missing-attempt', 'runtime-lifecycle-test', 'incarnation-first', 1, 1, ?1, 'ACTIVE', ?2, ?2, ?2, 1)",
            params![format!("sha256:{}", "a".repeat(64)), NOW],
        )
        .expect_err("the superseded process cannot acquire an active lease");
    assert!(
        error
            .to_string()
            .contains("EXECUTION_LEASE_STALE_RUNTIME_INCARNATION"),
        "expected the incarnation fence to reject before unrelated fixture references: {error}"
    );
    let error = connection
        .execute(
            "UPDATE execution_leases SET expires_at=?1 WHERE lease_id='stale-lease'",
            [NOW],
        )
        .expect_err("a superseded process cannot renew an active lease");
    assert!(
        error
            .to_string()
            .contains("EXECUTION_LEASE_STALE_RUNTIME_INCARNATION"),
        "expected the renewal fence to reject the stale incarnation: {error}"
    );
}
