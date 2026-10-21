use super::*;
use domain_invocations::InvocationStatus;
use storage_core::{
    CapabilityInvocationEventContext, CapabilityInvocationPageRequest,
    CapabilityInvocationProviderObservation, CapabilityInvocationReconciliationCommit,
    CapabilityInvocationStore, CapabilityInvocationTransitionCommit,
    InvocationProviderObservationSource,
};

const AT: &str = "2026-10-09T10:00:00Z";

fn seed_runtime_and_invocation(directory: &tempfile::TempDir, invocation_status: &str) {
    let connection = Connection::open(state_database(directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("disable fixture foreign keys");
    // These persistence tests exercise transitions on an already-admitted Invocation.
    // The fixture seeds that historical row directly, so the version one scope and
    // authority trigger is relaxed for the seed only. No production creation path is
    // added: the combined Trust, ApprovalUse, Invocation and Effect writer is the
    // only way an Invocation is admitted.
    connection
        .execute_batch("DROP TRIGGER capability_invocation_admission_matches_scope;")
        .expect("seed an already-admitted Invocation row");
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
        "INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,conversation_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at,version) VALUES('activation-1','ws','CONVERSATION','conv-1','{\"package_id\":\"pkg\"}','runtime','incarnation','MCP','ACTIVE','HEALTHY',?1,?1,1)",
        [AT],
    ).expect("seed active capability activation");
    connection.execute(
        "INSERT INTO capability_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,capability_ref_json,capability_identity_digest,configuration_digest,isolation_partition_digest,sharing_policy,hosting_mode,state,health,observed_at,expires_at,version) VALUES('host-1','runtime','incarnation','{\"package_id\":\"pkg\"}',?1,?2,?3,'EXCLUSIVE','LOCAL_MANAGED','READY','HEALTHY',?4,'2026-10-10T10:00:00Z',1)",
        params![provider_identity_digest(), format!("sha256:{}", "b".repeat(64)), format!("sha256:{}", "c".repeat(64)), AT],
    ).expect("seed capability host identity");
    connection.execute(
        "INSERT INTO capability_activation_host_bindings(activation_id,host_instance_id,provider_handle_ref,bound_at) VALUES('activation-1','host-1','provider-handle-ref',?1)",
        [AT],
    ).expect("bind activation to provider host");
    connection.execute(
        "INSERT INTO capability_invocations(invocation_id,workspace_id,scope_kind,conversation_id,agent_session_id,activation_id,capability_ref_json,operation,request_digest,execution_method,capability_grant_id,status,partial_result_refs_json,result_refs_json,created_at,updated_at,version) VALUES('inv-1','ws','CONVERSATION','conv-1','session-1','activation-1','{\"package_id\":\"pkg\"}','read',?1,'DETERMINISTIC_LOCAL','grant-1',?2,'[]','[]',?3,?3,1)",
        params![format!("sha256:{}", "a".repeat(64)), invocation_status, AT],
    ).expect("seed invocation for transition writer");
}

fn seed_authorized_peer_runtime(directory: &tempfile::TempDir) {
    let connection = Connection::open(state_database(directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("isolate the peer Runtime fixture from unrelated foreign keys");
    connection.execute(
        "INSERT INTO runtimes(runtime_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,resource_capacity_json,last_seen,version) VALUES('runtime-other','{}','0.1','linux','x86_64','[]','LOCAL','ONLINE','MANUAL','incarnation-other','{}',?1,1)",
        [AT],
    ).expect("seed another Runtime");
    connection.execute(
        "INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state,ready_at,version) VALUES('incarnation-other','runtime-other',?1,'0.1',0,'READY',?1,1)",
        [AT],
    ).expect("seed other Runtime incarnation");
    connection.execute(
        "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,version) VALUES('binding-other','runtime-other','ws','LOCAL_ENROLLMENT','ACTIVE','[\"EXECUTOR\"]',?1,?1,1)",
        [AT],
    ).expect("authorize other Runtime in Workspace");
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

fn reconciliation(
    status: InvocationStatus,
    provider_identity_digest: String,
    request_id: &str,
) -> CapabilityInvocationReconciliationCommit {
    CapabilityInvocationReconciliationCommit {
        workspace_id: "ws".to_owned(),
        invocation_id: "inv-1".to_owned(),
        expected_version: 1,
        request_id: request_id.to_owned(),
        observation: CapabilityInvocationProviderObservation {
            invocation_id: "inv-1".to_owned(),
            request_digest: format!("sha256:{}", "a".repeat(64)),
            provider_identity_digest,
            terminal_status: status,
            observed_at: "2026-10-09T10:02:00Z".to_owned(),
            source: InvocationProviderObservationSource::AuthenticatedQuery,
        },
        event: CapabilityInvocationEventContext {
            event_id: format!("event-{request_id}"),
            origin_runtime_id: "runtime".to_owned(),
            origin_runtime_incarnation_id: "incarnation".to_owned(),
            hlc_timestamp: "2026-10-09T10:02:00Z".to_owned(),
            correlation_id: "reconcile-corr".to_owned(),
            causation_id: Some("inv-1".to_owned()),
            recorded_at: "2026-10-09T10:02:00Z".to_owned(),
        },
    }
}

fn provider_identity_digest() -> String {
    digest(
        &canonical_json(&json!({
            "component_id":"comp",
            "package_id":"pkg"
        }))
        .unwrap(),
    )
}

#[test]
fn owner_scoped_invocation_reads_return_bounded_task_history_and_hide_foreign_owner() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "WAITING");
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("seed historical Invocation without unrelated parents");
    connection
        .execute(
            "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version) VALUES('task-1','ws',1,'RUNNING','test-binding','NORMAL','{}',?1,?1,1)",
            [AT],
        )
        .expect("seed Task for Invocation history");
    connection
        .execute_batch(
            "INSERT INTO capability_invocations(
               invocation_id,workspace_id,scope_kind,task_id,agent_session_id,activation_id,
               capability_ref_json,operation,request_digest,execution_method,capability_grant_id,
               status,partial_result_refs_json,result_refs_json,created_at,updated_at,version)
             VALUES('inv-task-1','ws','TASK_PLANNING','task-1','session-1','activation-1',
               '{\"package_id\":\"pkg\"}','workspace.resource.read',
               'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'DETERMINISTIC_LOCAL','grant-1','WAITING','[]','[]','2026-10-09T10:01:00Z',
               '2026-10-09T10:01:00Z',1);
             INSERT INTO capability_invocations(
               invocation_id,workspace_id,scope_kind,task_id,agent_session_id,activation_id,
               capability_ref_json,operation,request_digest,execution_method,capability_grant_id,
               status,partial_result_refs_json,result_refs_json,created_at,updated_at,version)
             VALUES('inv-task-2','ws','TASK_PLANNING','task-1','session-1','activation-1',
               '{\"package_id\":\"pkg\"}','workspace.resource.read',
               'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
               'DETERMINISTIC_LOCAL','grant-1','WAITING','[]','[]','2026-10-09T10:01:00Z',
               '2026-10-09T10:01:00Z',1);",
        )
        .expect("seed already-admitted Task invocation");

    let reader = SqliteCapabilityInvocationStore::new(store);
    let page = reader
        .list_task_invocations_for_owner(CapabilityInvocationPageRequest {
            owner_principal_id: "owner".into(),
            workspace_id: "ws".into(),
            task_id: "task-1".into(),
            after_created_at: None,
            after_invocation_id: None,
            limit: 1,
        })
        .expect("owner can read Task invocation history");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].invocation_id, "inv-task-2");
    assert_eq!(page[0].task_id.as_deref(), Some("task-1"));

    let next_page = reader
        .list_task_invocations_for_owner(CapabilityInvocationPageRequest {
            owner_principal_id: "owner".into(),
            workspace_id: "ws".into(),
            task_id: "task-1".into(),
            after_created_at: Some(page[0].created_at.clone()),
            after_invocation_id: Some(page[0].invocation_id.clone()),
            limit: 1,
        })
        .expect("stable keyset cursor continues history");
    assert_eq!(next_page.len(), 1);
    assert_eq!(next_page[0].invocation_id, "inv-task-1");

    let single = reader
        .get_invocation_for_owner("owner", "ws", "inv-task-1")
        .expect("owner can read Invocation")
        .expect("admitted Invocation exists");
    assert_eq!(single.invocation_id, "inv-task-1");
    assert_eq!(
        reader
            .get_invocation_for_owner("owner", "other-ws", "inv-task-1")
            .expect_err("foreign Workspace is hidden"),
        StoreError::NotFound
    );
    assert!(
        reader
            .get_invocation_for_owner("owner", "ws", "missing-invocation")
            .expect("missing record is not a storage failure")
            .is_none()
    );

    assert!(
        reader
            .get_invocation_for_owner("foreign-owner", "ws", "inv-task-1")
            .expect_err("foreign Workspace owner is hidden")
            == StoreError::NotFound
    );
    assert!(
        reader
            .list_task_invocations_for_owner(CapabilityInvocationPageRequest {
                owner_principal_id: "owner".into(),
                workspace_id: "ws".into(),
                task_id: "task-1".into(),
                after_created_at: None,
                after_invocation_id: None,
                limit: 202,
            })
            .is_err(),
        "storage bounds page size before query execution"
    );
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
fn ambiguous_terminal_reconciliation_commits_observation_and_exactly_replays() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "AMBIGUOUS");
    let invocation_store = SqliteCapabilityInvocationStore::new(store.clone());
    let command = reconciliation(
        InvocationStatus::Succeeded,
        provider_identity_digest(),
        "req-reconcile",
    );

    let committed = invocation_store
        .reconcile_terminal_invocation(command.clone())
        .expect("authenticated provider observation settles the exact Invocation");
    assert_eq!(committed.invocation.status, InvocationStatus::Succeeded);
    assert_eq!(
        committed.invocation.provider_task_status.as_deref(),
        Some("COMPLETED")
    );
    assert_eq!(
        committed.invocation.provider_updated_at.as_deref(),
        Some("2026-10-09T10:02:00.000000000Z")
    );
    assert_eq!(
        committed.event.payload["provider_observation"]["source"],
        "AUTHENTICATED_QUERY"
    );
    assert_eq!(
        committed.event.payload["provider_observation"]["provider_identity_digest"],
        provider_identity_digest()
    );
    assert_eq!(
        committed.event.payload["provider_observation"]
            .as_object()
            .expect("provider observation is a JSON object")
            .len(),
        3,
        "event must omit raw provider response, handles, and credentials"
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect committed store");
    let state: (String, String, i64, i64) = connection
        .query_row(
            "SELECT i.status,i.provider_task_status,
                    (SELECT COUNT(*) FROM domain_events WHERE entity_id='inv-1'),
                    (SELECT COUNT(*) FROM request_dedup WHERE request_id='req-reconcile')
             FROM capability_invocations i WHERE i.invocation_id='inv-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read committed reconciliation");
    assert_eq!(
        state,
        ("SUCCEEDED".to_owned(), "COMPLETED".to_owned(), 1, 1)
    );

    let replay = invocation_store
        .reconcile_terminal_invocation(command)
        .expect("identical reconciliation retries replay the committed response");
    assert!(replay.replayed);
    assert_eq!(replay.invocation, committed.invocation);
}

#[test]
fn reconciliation_persists_all_supported_terminal_provider_states() {
    for (status, provider_status) in [
        (InvocationStatus::Succeeded, "COMPLETED"),
        (InvocationStatus::Failed, "FAILED"),
        (InvocationStatus::Cancelled, "CANCELLED"),
    ] {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory, Duration::from_secs(1));
        seed_runtime_and_invocation(&directory, "AMBIGUOUS");
        let committed = SqliteCapabilityInvocationStore::new(store)
            .reconcile_terminal_invocation(reconciliation(
                status,
                provider_identity_digest(),
                &format!("req-{provider_status}"),
            ))
            .expect("persist terminal provider observation");
        assert_eq!(committed.invocation.status, status);
        assert_eq!(
            committed.invocation.provider_task_status.as_deref(),
            Some(provider_status)
        );
        assert_eq!(
            committed.invocation.completed_at.as_deref(),
            Some("2026-10-09T10:02:00.000000000Z")
        );
    }
}

#[test]
fn reconciliation_rejects_wrong_request_digest_without_writes() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "AMBIGUOUS");
    let mut command = reconciliation(
        InvocationStatus::Succeeded,
        provider_identity_digest(),
        "req-wrong-request",
    );
    command.observation.request_digest = format!("sha256:{}", "e".repeat(64));

    let error = SqliteCapabilityInvocationStore::new(store)
        .reconcile_terminal_invocation(command)
        .expect_err("provider evidence must bind the immutable Invocation request digest");
    assert_eq!(
        error,
        StoreError::Invalid("INVOCATION_RECONCILIATION_OBSERVATION_MISMATCH".to_owned())
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let state: (String, i64, i64, i64) = connection
        .query_row(
            "SELECT status,version,(SELECT COUNT(*) FROM domain_events),(SELECT COUNT(*) FROM request_dedup)
             FROM capability_invocations WHERE invocation_id='inv-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read unchanged state");
    assert_eq!(state, ("AMBIGUOUS".to_owned(), 1, 0, 0));
}

#[test]
fn reconciliation_rejects_wrong_provider_identity_without_writes() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "AMBIGUOUS");
    let before = std::fs::read_dir(directory.path().join("blobs"))
        .map(|entries| entries.count())
        .unwrap_or(0);

    let error = SqliteCapabilityInvocationStore::new(store)
        .reconcile_terminal_invocation(reconciliation(
            InvocationStatus::Succeeded,
            format!("sha256:{}", "f".repeat(64)),
            "req-wrong-provider",
        ))
        .expect_err("an observation from another provider must not settle this activation");
    assert_eq!(
        error,
        StoreError::Invalid("INVOCATION_PROVIDER_OBSERVATION_MISMATCH".to_owned())
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
    assert_eq!(values, ("AMBIGUOUS".to_owned(), 1, 0, 0));
    let after = std::fs::read_dir(directory.path().join("blobs"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(after, before);
}

#[test]
fn reconciliation_rejects_stale_version_without_writes() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "AMBIGUOUS");
    let mut command = reconciliation(
        InvocationStatus::Succeeded,
        provider_identity_digest(),
        "req-stale-reconciliation",
    );
    command.expected_version = 2;

    let error = SqliteCapabilityInvocationStore::new(store)
        .reconcile_terminal_invocation(command)
        .expect_err("stale observers must reload the Invocation");
    assert_eq!(
        error,
        StoreError::Conflict {
            expected: Some(2),
            actual: Some(1)
        }
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let state: (String, i64, i64, i64) = connection
        .query_row(
            "SELECT status,version,(SELECT COUNT(*) FROM domain_events),(SELECT COUNT(*) FROM request_dedup)
             FROM capability_invocations WHERE invocation_id='inv-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read unchanged state");
    assert_eq!(state, ("AMBIGUOUS".to_owned(), 1, 0, 0));
}

#[test]
fn reconciliation_runtime_must_match_activation_host_runtime() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "AMBIGUOUS");
    seed_authorized_peer_runtime(&directory);
    let mut command = reconciliation(
        InvocationStatus::Succeeded,
        provider_identity_digest(),
        "req-wrong-runtime",
    );
    command.event.origin_runtime_id = "runtime-other".to_owned();
    command.event.origin_runtime_incarnation_id = "incarnation-other".to_owned();

    let error = SqliteCapabilityInvocationStore::new(store)
        .reconcile_terminal_invocation(command)
        .expect_err("a Workspace peer Runtime cannot reconcile another activation's provider");
    assert_eq!(
        error,
        StoreError::Invalid("INVOCATION_PROVIDER_OBSERVATION_MISMATCH".to_owned())
    );
}

#[test]
fn generic_status_observation_runtime_must_match_activation_runtime() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "DISPATCHED");
    seed_authorized_peer_runtime(&directory);
    let mut command = transition(
        InvocationStatus::Succeeded,
        "event-peer-status",
        "req-peer-status",
    );
    command.event.origin_runtime_id = "runtime-other".to_owned();
    command.event.origin_runtime_incarnation_id = "incarnation-other".to_owned();

    let error = SqliteCapabilityInvocationStore::new(store)
        .transition_invocation(command)
        .expect_err("Workspace peer Runtime cannot report another Activation's Invocation");
    assert_eq!(
        error,
        StoreError::Invalid("INVOCATION_RUNTIME_MISMATCH".to_owned())
    );
    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let state: (String, i64, i64, i64) = connection
        .query_row(
            "SELECT status,version,(SELECT COUNT(*) FROM domain_events),(SELECT COUNT(*) FROM request_dedup)
             FROM capability_invocations WHERE invocation_id='inv-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read unchanged Invocation");
    assert_eq!(state, ("DISPATCHED".to_owned(), 1, 0, 0));
}

#[test]
fn changed_observation_under_reconciliation_request_id_conflicts() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_runtime_and_invocation(&directory, "AMBIGUOUS");
    let invocation_store = SqliteCapabilityInvocationStore::new(store);
    invocation_store
        .reconcile_terminal_invocation(reconciliation(
            InvocationStatus::Failed,
            provider_identity_digest(),
            "req-reconcile-conflict",
        ))
        .expect("first observation commits");
    let mut changed = reconciliation(
        InvocationStatus::Succeeded,
        provider_identity_digest(),
        "req-reconcile-conflict",
    );
    changed.observation.source = InvocationProviderObservationSource::AuthenticatedCallback;
    assert_eq!(
        invocation_store.reconcile_terminal_invocation(changed),
        Err(StoreError::IdempotencyConflict)
    );
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
    // A direct status rewrite is refused by the transition guard.
    assert!(connection.execute("UPDATE capability_invocations SET status='SUCCEEDED',version=2 WHERE invocation_id='inv-1'", []).is_err());
    // A direct dispatch is refused because the version twenty-six guard requires the
    // exact Effect to be bound to a consumed approval. Only the combined Trust,
    // ApprovalUse, Invocation and Effect writer may dispatch, and it records that
    // ApprovalUse in the same transaction.
    let dispatch_error = connection
        .execute("UPDATE capability_invocations SET status='DISPATCHED',version=2 WHERE invocation_id='inv-1'", [])
        .expect_err("a dispatch without a consumed approval must be refused");
    let dispatch_message = dispatch_error.to_string();
    assert!(
        dispatch_message.contains("INVOCATION_DISPATCH_REQUIRES_CONSUMED_APPROVAL")
            || dispatch_message.contains("INVOCATION_OWNER_NOT_LIVE_AT_DISPATCH"),
        "unexpected database rejection: {dispatch_message}"
    );
}

#[test]
fn sqlite_trigger_rejects_invocation_creation_without_live_authorized_authority() {
    let directory = tempfile::tempdir().expect("temporary store");
    let _store = test_store(&directory, Duration::from_secs(1));
    // The shipped authority trigger is the thing under test here, so no fixture row is
    // seeded and no trigger is dropped. Version twenty-six removed the blanket creation
    // block, which means the version one scope and authority trigger is now the only
    // thing that can refuse an unauthorized Invocation.
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
            .contains("INVOCATION_SCOPE_OR_AUTHORIZATION_MISMATCH"),
        "unexpected database rejection: {error}"
    );
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM capability_invocations", [], |row| {
            row.get(0)
        })
        .expect("count unchanged invocations");
    assert_eq!(count, 0);
}
