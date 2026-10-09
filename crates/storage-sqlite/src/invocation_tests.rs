use super::*;
use domain_invocations::InvocationStatus;
use storage_core::{
    CapabilityInvocationEventContext, CapabilityInvocationStore,
    CapabilityInvocationTransitionCommit,
};

const AT: &str = "2026-10-09T10:00:00Z";

fn seed_runtime_and_invocation(directory: &tempfile::TempDir, invocation_status: &str) {
    let connection = Connection::open(state_database(directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("disable fixture foreign keys");
    // These persistence tests exercise transitions on an already-admitted Invocation.
    // The atomic creator is intentionally unavailable, so seed this existing row past
    // the admission-only insert guard without adding a production creation path.
    connection
        .execute_batch("DROP TRIGGER capability_invocation_admission_matches_scope; DROP TRIGGER capability_invocation_creation_admission_closed;")
        .expect("bypass unavailable creator for admitted-row fixture");
    connection.execute(
        "INSERT INTO workspaces(workspace_id,name,owner_principal_id,replication_policy,status,created_at,updated_at,version) VALUES('ws','Workspace','owner','LOCAL_ONLY','ACTIVE',?1,?1,1)",
        [AT],
    ).expect("seed Workspace");
    connection.execute(
        "INSERT INTO runtimes(runtime_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,resource_capacity_json,last_seen,version) VALUES('runtime','{}','0.1','linux','x86_64','[]','LOCAL','ONLINE','MANUAL','incarnation','{}',?1,1)",
        [AT],
    ).expect("seed Runtime");
    connection.execute(
        "INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state,ready_at,version) VALUES('incarnation','runtime',?1,'0.1',0,'READY',?1,1)",
        [AT],
    ).expect("seed Runtime incarnation");
    connection.execute(
        "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,version) VALUES('binding','runtime','ws','LOCAL_ENROLLMENT','ACTIVE','[\"EXECUTOR\"]',?1,?1,1)",
        [AT],
    ).expect("seed active Runtime Workspace binding");
    connection.execute(
        "INSERT INTO capability_invocations(invocation_id,workspace_id,scope_kind,conversation_id,agent_session_id,activation_id,capability_ref_json,operation,request_digest,execution_method,capability_grant_id,status,partial_result_refs_json,result_refs_json,created_at,updated_at,version) VALUES('inv-1','ws','CONVERSATION','conv-1','session-1','activation-1','{\"package_id\":\"pkg\"}','read',?1,'DETERMINISTIC_LOCAL','grant-1',?2,'[]','[]',?3,?3,1)",
        params![format!("sha256:{}", "a".repeat(64)), invocation_status, AT],
    ).expect("seed invocation for transition writer");
    // Restore the production fail-closed INSERT guard after seeding a historical row.
    connection.execute_batch("CREATE TRIGGER capability_invocation_creation_admission_closed BEFORE INSERT ON capability_invocations BEGIN SELECT RAISE(ABORT, 'INVOCATION_CREATION_ADMISSION_UNAVAILABLE'); END;")
        .expect("restore the creation admission guard");
}

fn transition(
    status: InvocationStatus,
    event_id: &str,
    request_id: &str,
) -> CapabilityInvocationTransitionCommit {
    CapabilityInvocationTransitionCommit {
        workspace_id: "ws".to_owned(),
        invocation_id: "inv-1".to_owned(),
        expected_version: 1,
        next_status: status,
        request_id: request_id.to_owned(),
        event: CapabilityInvocationEventContext {
            event_id: event_id.to_owned(),
            origin_runtime_id: "runtime".to_owned(),
            origin_runtime_incarnation_id: "incarnation".to_owned(),
            hlc_timestamp: AT.to_owned(),
            correlation_id: "corr".to_owned(),
            causation_id: None,
            recorded_at: AT.to_owned(),
        },
    }
}

#[test]
fn status_transition_atomically_updates_snapshot_event_and_receipt() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "CREATED");

    let committed = SqliteCapabilityInvocationStore::new(store.clone())
        .transition_invocation(transition(
            InvocationStatus::Cancelled,
            "event-cancel",
            "req-cancel",
        ))
        .expect("persist local pre-dispatch cancellation");
    assert_eq!(committed.invocation.status, InvocationStatus::Cancelled);
    assert_eq!(committed.invocation.version, 2);
    assert_eq!(
        committed.event.event_type,
        "capability.invocation.status.changed.v1"
    );
    assert_eq!(committed.event.payload["from"], "CREATED");
    assert_eq!(committed.event.payload["to"], "CANCELLED");
    assert!(!committed.replayed);

    let replay = SqliteCapabilityInvocationStore::new(store.clone())
        .transition_invocation(transition(
            InvocationStatus::Cancelled,
            "event-cancel",
            "req-cancel",
        ))
        .expect("exact request replays");
    assert!(replay.replayed);
    assert_eq!(replay.invocation, committed.invocation);

    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let (status, version, events, receipts): (String, i64, i64, i64) = connection.query_row(
        "SELECT i.status,i.version,(SELECT COUNT(*) FROM domain_events WHERE entity_id='inv-1'),(SELECT COUNT(*) FROM request_dedup WHERE request_id='req-cancel') FROM capability_invocations i WHERE invocation_id='inv-1'",
        [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).expect("read committed state");
    assert_eq!(
        (status.as_str(), version, events, receipts),
        ("CANCELLED", 2, 1, 1)
    );
}

#[test]
fn dispatch_transition_fails_before_blob_database_event_or_receipt_write() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "CREATED");
    let blob_count_before = std::fs::read_dir(directory.path().join("blobs"))
        .map(|entries| entries.count())
        .unwrap_or(0);

    let error = SqliteCapabilityInvocationStore::new(store.clone())
        .transition_invocation(transition(
            InvocationStatus::Dispatched,
            "event-dispatch",
            "req-dispatch",
        ))
        .expect_err("the missing combined admission keeps dispatch closed");
    assert_eq!(
        error,
        StoreError::Invalid("INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE".to_owned())
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let values: (String, i64, i64, i64) = connection.query_row(
        "SELECT status,version,(SELECT COUNT(*) FROM domain_events),(SELECT COUNT(*) FROM request_dedup)
         FROM capability_invocations WHERE invocation_id='inv-1'",
        [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).expect("read unchanged state");
    assert_eq!(values, ("CREATED".to_owned(), 1, 0, 0));
    let blob_count_after = std::fs::read_dir(directory.path().join("blobs"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(blob_count_after, blob_count_before);
}

#[test]
fn stale_expected_version_leaves_invocation_and_receipts_unchanged() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "CREATED");
    let before = std::fs::read_dir(directory.path().join("blobs"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    let mut stale = transition(InvocationStatus::Cancelled, "event-stale", "req-stale");
    stale.expected_version = 2;

    let error = SqliteCapabilityInvocationStore::new(store)
        .transition_invocation(stale)
        .expect_err("stale callers must reload before transitioning");
    assert_eq!(
        error,
        StoreError::Conflict {
            expected: Some(2),
            actual: Some(1)
        }
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let values: (String, i64, i64, i64) = connection
        .query_row(
            "SELECT status,version,(SELECT COUNT(*) FROM domain_events),(SELECT COUNT(*) FROM request_dedup)
             FROM capability_invocations WHERE invocation_id='inv-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read unchanged state");
    assert_eq!(values, ("CREATED".to_owned(), 1, 0, 0));
    let after = std::fs::read_dir(directory.path().join("blobs"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(after, before);
}

#[test]
fn sqlite_trigger_rejects_direct_status_rewrite_and_dispatch() {
    let directory = tempfile::tempdir().expect("temporary store");
    let _store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "CREATED");
    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    assert!(connection.execute("UPDATE capability_invocations SET status='DISPATCHED',version=2 WHERE invocation_id='inv-1'", []).is_err());
    assert!(connection.execute("UPDATE capability_invocations SET status='SUCCEEDED',version=2 WHERE invocation_id='inv-1'", []).is_err());
}

#[test]
fn sqlite_trigger_rejects_invocation_creation_until_combined_admission_exists() {
    let directory = tempfile::tempdir().expect("temporary store");
    let _store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "CREATED");
    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    // Exercise the authorization trigger rather than a missing-fixture FK constraint.
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("isolate the authorization trigger in this fixture");
    let error = connection
        .execute(
            "INSERT INTO capability_invocations(
                invocation_id,workspace_id,scope_kind,conversation_id,agent_session_id,
                activation_id,capability_ref_json,operation,request_digest,execution_method,
                capability_grant_id,status,partial_result_refs_json,result_refs_json,created_at,
                updated_at,version
             ) VALUES('inv-2','ws','CONVERSATION','conv-1','session-1','activation-1',
                '{\"package_id\":\"pkg\"}','read',?1,'DETERMINISTIC_LOCAL','grant-1',
                'CREATED','[]','[]',?2,?2,1)",
            params![format!("sha256:{}", "b".repeat(64)), AT],
        )
        .expect_err("an invocation without a live authorized session must be rejected");
    assert!(
        error
            .to_string()
            .contains("INVOCATION_CREATION_ADMISSION_UNAVAILABLE"),
        "unexpected database rejection: {error}"
    );
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM capability_invocations", [], |row| {
            row.get(0)
        })
        .expect("count unchanged invocations");
    assert_eq!(count, 1);
}
