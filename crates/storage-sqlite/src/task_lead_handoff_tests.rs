use super::*;
use storage_core::{BlobRef, TaskLeadHandoffCommit};

const OWNER: &str = "owner-handoff";
const WORKSPACE: &str = "workspace-handoff";
const TASK: &str = "task-handoff";
const FROM: &str = "binding-from";
const TO: &str = "binding-to";
const NOW: &str = "2026-10-17T10:00:00Z";

fn connection() -> Connection {
    let connection = Connection::open_in_memory().expect("in-memory SQLite");
    connection
        .execute_batch(
            "CREATE TABLE workspaces(workspace_id TEXT PRIMARY KEY, owner_principal_id TEXT, status TEXT);
             CREATE TABLE tasks(
               task_id TEXT PRIMARY KEY, workspace_id TEXT, conversation_id TEXT,
               current_spec_revision INTEGER, current_plan_revision INTEGER, status TEXT,
               resume_status TEXT, routine_id TEXT, routine_revision INTEGER, automation_id TEXT,
               automation_occurrence_id TEXT, origin_coworker_id TEXT, origin_coworker_revision INTEGER,
               lead_agent_binding_id TEXT, blocking_conditions_json TEXT, priority TEXT,
               created_by_json TEXT, created_at TEXT, updated_at TEXT, completed_at TEXT, version INTEGER
             );
             CREATE TABLE task_spec_revisions(
               task_id TEXT, workspace_id TEXT, revision INTEGER, parent_revisions_json TEXT,
               objective TEXT, task_category TEXT, constraints_json TEXT, non_goals_json TEXT,
               input_refs_json TEXT, workspace_instruction_revision INTEGER,
               required_outputs_json TEXT, acceptance_criteria_json TEXT, approvals_required_json TEXT,
               budget_json TEXT, delegation_budget_policy_json TEXT, lead_failover_policy_json TEXT,
               deadline TEXT, source_message_refs_json TEXT, placement_preference TEXT,
               preferred_lead_agent_binding_id TEXT, authored_by_json TEXT, created_at TEXT
             );
             CREATE TABLE agent_bindings(workspace_id TEXT, agent_binding_id TEXT, enabled INTEGER, lead_eligible INTEGER);
             CREATE TABLE agent_sessions(workspace_id TEXT, task_id TEXT, scope_kind TEXT, status TEXT);
             CREATE TABLE attempts(task_id TEXT, status TEXT);
             CREATE TABLE effects(task_id TEXT, state TEXT);
             CREATE TABLE steps(step_id TEXT, task_id TEXT, plan_revision INTEGER, logical_key TEXT,
               title TEXT, objective TEXT, dependencies_json TEXT, required_capabilities_json TEXT,
               acceptance_criteria_json TEXT, status TEXT, current_attempt_id TEXT,
               created_at TEXT, updated_at TEXT, version INTEGER);
             CREATE TABLE workspace_origin_sequences(workspace_id TEXT, origin_runtime_id TEXT, last_sequence INTEGER,
               PRIMARY KEY(workspace_id, origin_runtime_id));
             CREATE TABLE domain_events(
               event_id TEXT PRIMARY KEY, workspace_id TEXT, entity_type TEXT, entity_id TEXT,
               origin_runtime_id TEXT, origin_sequence INTEGER, entity_revision INTEGER, hlc_timestamp TEXT,
               correlation_id TEXT, causation_id TEXT, schema_version INTEGER, type TEXT,
               payload_json TEXT, aggregate_state_ref_json TEXT, recorded_at TEXT, payload_digest TEXT
             );
             CREATE TABLE request_dedup(
               principal_id TEXT, request_id TEXT, request_digest TEXT, response_json TEXT,
               response_digest TEXT, created_at TEXT, expires_at TEXT,
               PRIMARY KEY(principal_id, request_id)
             );
             INSERT INTO workspaces VALUES ('workspace-handoff','owner-handoff','ACTIVE');
             INSERT INTO tasks VALUES (
               'task-handoff','workspace-handoff',NULL,1,NULL,'READY',NULL,NULL,NULL,NULL,NULL,
               NULL,NULL,'binding-from','[]','NORMAL','{\"kind\":\"USER\",\"principal_id\":\"owner-handoff\"}',
               '2026-10-17T09:00:00Z','2026-10-17T09:00:00Z',NULL,1
             );
             INSERT INTO task_spec_revisions VALUES (
               'task-handoff','workspace-handoff',1,'[]','Task objective',NULL,'[]','[]','[]',NULL,
               '[]','[]','[]',NULL,NULL,'{\"mode\":\"DISABLED\",\"triggers\":[],\"fallback_agent_binding_ids\":[],\"max_lead_changes\":0}',
               NULL,'[]','\"AUTO\"','binding-from','{\"kind\":\"USER\",\"principal_id\":\"owner-handoff\"}',
               '2026-10-17T09:00:00Z'
             );
             INSERT INTO agent_bindings VALUES ('workspace-handoff','binding-from',1,1);
             INSERT INTO agent_bindings VALUES ('workspace-handoff','binding-to',1,1);",
        )
        .expect("create handoff transaction fixture");
    connection
}

fn commit(request_id: &str) -> TaskLeadHandoffCommit {
    let request_payload = json!({
        "workspace_id": WORKSPACE,
        "task_id": TASK,
        "request_scope_id": OWNER,
        "expected_task_version": 1,
        "expected_task_spec_revision": 1,
        "target_agent_binding_id": TO,
        "cause": "OWNER_REQUEST",
        "trigger_observation": null,
    });
    TaskLeadHandoffCommit {
        workspace_id: WORKSPACE.to_owned(),
        task_id: TASK.to_owned(),
        request_scope_id: OWNER.to_owned(),
        request_id: request_id.to_owned(),
        request_payload,
        expected_task_version: 1,
        expected_task_spec_revision: 1,
        target_agent_binding_id: TO.to_owned(),
        cause: "OWNER_REQUEST".to_owned(),
        actor: json!({"principal_id": OWNER, "kind": "USER"}),
        trigger_observation: None,
        event: EventDraft {
            event_id: format!("event-{request_id}"),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "Task".to_owned(),
            entity_id: TASK.to_owned(),
            origin_runtime_id: "runtime-handoff".to_owned(),
            entity_revision: 2,
            hlc_timestamp: NOW.to_owned(),
            correlation_id: request_id.to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "task.lead_agent.changed.v1".to_owned(),
            payload: json!({
                "task_id": TASK,
                "to_agent_binding_id": TO,
                "cause": "OWNER_REQUEST",
                "actor": {"principal_id": OWNER, "kind": "USER"},
                "task_spec_revision": 1,
                "aggregate_version": 2,
            }),
            recorded_at: NOW.to_owned(),
        },
    }
}

fn state_ref() -> AggregateStateRef {
    AggregateStateRef {
        blob: BlobRef {
            digest: format!("sha256:{}", "a".repeat(64)),
            size_bytes: 128,
            media_type: TASK_STATE_MEDIA_TYPE.to_owned(),
        },
        entity_revision: 2,
        record_schema_version: 1,
    }
}

#[test]
fn task_lead_handoff_commits_atomic_state_event_and_exact_replay_receipt() {
    let mut connection = connection();
    let result =
        commit_task_lead_handoff_transaction(&mut connection, commit("handoff-1"), state_ref())
            .expect("safe owner handoff commits");
    assert_eq!(result.task.lead_agent_binding_id, TO);
    assert_eq!(result.task.version, 2);
    assert_eq!(result.task.current_spec_revision, 1);
    assert_eq!(result.previous_lead_agent_binding_id, FROM);
    assert_eq!(result.aggregate_version, 2);
    assert!(!result.replayed);

    let event_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
        .expect("event count");
    let event_payload: String = connection
        .query_row("SELECT payload_json FROM domain_events", [], |row| {
            row.get(0)
        })
        .expect("event payload");
    let payload: Value = serde_json::from_str(&event_payload).expect("valid payload");
    assert_eq!(event_count, 1);
    assert_eq!(payload["from_agent_binding_id"], FROM);
    assert_eq!(payload["to_agent_binding_id"], TO);

    let replay =
        commit_task_lead_handoff_transaction(&mut connection, commit("handoff-1"), state_ref())
            .expect("lost response returns the original receipt");
    assert!(replay.replayed);
    assert_eq!(replay.task, result.task);
    let event_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
        .expect("event count after replay");
    assert_eq!(event_count, 1);

    let mut changed_replay = commit("handoff-1");
    changed_replay.request_payload["target_agent_binding_id"] = json!("other-binding");
    assert_eq!(
        commit_task_lead_handoff_transaction(&mut connection, changed_replay, state_ref())
            .unwrap_err(),
        StoreError::IdempotencyConflict
    );
}

#[test]
fn task_lead_handoff_rejects_unsafe_and_ineligible_targets_without_mutation() {
    let mut connection = connection();
    connection
        .execute(
            "INSERT INTO agent_sessions VALUES (?1, ?2, 'TASK_PLANNING', 'ACTIVE')",
            params![WORKSPACE, TASK],
        )
        .expect("insert live planner");
    let error =
        commit_task_lead_handoff_transaction(&mut connection, commit("handoff-live"), state_ref())
            .expect_err("live planner blocks handoff");
    assert!(matches!(error, StoreError::Invalid(_)));
    let lead: String = connection
        .query_row("SELECT lead_agent_binding_id FROM tasks", [], |row| {
            row.get(0)
        })
        .expect("lead remains unchanged");
    assert_eq!(lead, FROM);

    connection
        .execute("DELETE FROM agent_sessions", [])
        .expect("clear planner");
    connection
        .execute(
            "UPDATE agent_bindings SET enabled = 0 WHERE agent_binding_id = ?1",
            [TO],
        )
        .expect("disable target");
    let mut ineligible = commit("handoff-ineligible");
    ineligible.expected_task_version = 1;
    let error = commit_task_lead_handoff_transaction(&mut connection, ineligible, state_ref())
        .expect_err("disabled binding blocks handoff");
    assert!(matches!(error, StoreError::Invalid(_)));
    let event_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
        .expect("no event emitted on denial");
    assert_eq!(event_count, 0);
}

#[test]
fn task_version_and_task_spec_conflicts_remain_distinct_and_mutation_free() {
    let mut connection = connection();

    let mut stale_task = commit("handoff-stale-task");
    stale_task.expected_task_version = 2;
    stale_task.request_payload["expected_task_version"] = json!(2);
    stale_task.event.entity_revision = 3;
    stale_task.event.payload["aggregate_version"] = json!(3);
    let mut stale_task_state = state_ref();
    stale_task_state.entity_revision = 3;
    let error = commit_task_lead_handoff_transaction(&mut connection, stale_task, stale_task_state)
        .expect_err("stale Task version is rejected");
    assert!(matches!(error, StoreError::Conflict { .. }));

    let mut stale_spec = commit("handoff-stale-spec");
    stale_spec.expected_task_spec_revision = 2;
    stale_spec.request_payload["expected_task_spec_revision"] = json!(2);
    stale_spec.event.payload["task_spec_revision"] = json!(2);
    let error = commit_task_lead_handoff_transaction(&mut connection, stale_spec, state_ref())
        .expect_err("stale TaskSpec revision is rejected distinctly");
    assert!(matches!(error, StoreError::Invalid(code) if code == "STALE_SPEC_REVISION"));

    let lead: String = connection
        .query_row(
            "SELECT lead_agent_binding_id FROM tasks WHERE task_id = ?1",
            [TASK],
            |row| row.get(0),
        )
        .expect("read unchanged Task lead");
    assert_eq!(lead, FROM);
    let receipts: i64 = connection
        .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| row.get(0))
        .expect("count unchanged receipts");
    assert_eq!(receipts, 0);
}

#[test]
fn active_attempts_and_unresolved_effects_block_handoff_without_receipts() {
    let mut connection = connection();
    connection
        .execute("INSERT INTO attempts VALUES (?1, 'RUNNING')", [TASK])
        .expect("insert active Attempt");
    let error = commit_task_lead_handoff_transaction(
        &mut connection,
        commit("handoff-active-attempt"),
        state_ref(),
    )
    .expect_err("active Attempt blocks handoff");
    assert!(matches!(error, StoreError::Invalid(_)));
    connection
        .execute("DELETE FROM attempts", [])
        .expect("settle test Attempt");
    connection
        .execute("INSERT INTO effects VALUES (?1, 'STARTED')", [TASK])
        .expect("insert unresolved Effect");
    let error = commit_task_lead_handoff_transaction(
        &mut connection,
        commit("handoff-unresolved-effect"),
        state_ref(),
    )
    .expect_err("unresolved Effect blocks handoff");
    assert!(matches!(error, StoreError::Invalid(_)));

    let lead: String = connection
        .query_row("SELECT lead_agent_binding_id FROM tasks", [], |row| {
            row.get(0)
        })
        .expect("read unchanged Task");
    let receipts: i64 = connection
        .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| row.get(0))
        .expect("count receipts");
    assert_eq!(lead, FROM);
    assert_eq!(receipts, 0);
}

#[test]
fn event_collision_rolls_back_task_mutation_and_receipt() {
    let mut connection = connection();
    connection
        .execute(
            "INSERT INTO domain_events(event_id,workspace_id,entity_type,entity_id,origin_runtime_id,origin_sequence,entity_revision,hlc_timestamp,correlation_id,causation_id,schema_version,type,payload_json,aggregate_state_ref_json,recorded_at,payload_digest) VALUES ('event-handoff-event-collision','workspace-handoff','Task','another-task','runtime-handoff',1,1,?1,'prior',NULL,1,'prior.event.v1','{}',NULL,?1,'digest')",
            [NOW],
        )
        .expect("seed conflicting immutable event ID");
    let error = commit_task_lead_handoff_transaction(
        &mut connection,
        commit("handoff-event-collision"),
        state_ref(),
    )
    .expect_err("event identity conflict aborts the transaction");
    assert!(
        matches!(error, StoreError::Database(_)),
        "unexpected event insert error: {error:?}"
    );

    let (lead, version): (String, i64) = connection
        .query_row(
            "SELECT lead_agent_binding_id, version FROM tasks",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read unchanged Task");
    let receipts: i64 = connection
        .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| row.get(0))
        .expect("count receipts");
    assert_eq!(lead, FROM);
    assert_eq!(version, 1);
    assert_eq!(receipts, 0);
}

#[test]
fn task_lead_handoff_receipt_corruption_is_not_replayed_as_authority() {
    let mut connection = connection();
    let committed = commit_task_lead_handoff_transaction(
        &mut connection,
        commit("handoff-corrupt-receipt"),
        state_ref(),
    )
    .expect("initial transaction commits");
    connection
        .execute(
            "UPDATE request_dedup SET response_digest = 'sha256:wrong' WHERE principal_id = ?1 AND request_id = ?2",
            params![OWNER, "handoff-corrupt-receipt"],
        )
        .expect("tamper with test receipt digest");
    let error = commit_task_lead_handoff_transaction(
        &mut connection,
        commit("handoff-corrupt-receipt"),
        state_ref(),
    )
    .expect_err("corrupt receipt is an integrity error");
    assert!(matches!(error, StoreError::Integrity(_)));
    let lead: String = connection
        .query_row("SELECT lead_agent_binding_id FROM tasks", [], |row| {
            row.get(0)
        })
        .expect("Task remains at committed state");
    assert_eq!(lead, committed.target_agent_binding_id);
}

#[test]
fn fresh_looking_policy_observation_cannot_bypass_missing_authenticated_source_authority() {
    let mut connection = connection();
    let observation = json!({
        "trigger": "QUOTA_EXHAUSTED",
        "affected_agent_binding_id": FROM,
        "source": {"service_id": "codex-adapter"},
        "source_observation_ref": "quota-observation-123",
        "runtime_incarnation_id": null,
        "observed_at": NOW,
        "expires_at": "2026-10-17T10:01:00Z",
    });
    let mut policy = commit("policy-handoff-untrusted-observation");
    policy.request_scope_id = "lead-failover".to_owned();
    policy.request_payload["request_scope_id"] = json!("lead-failover");
    policy.request_payload["cause"] = json!("POLICY_FAILOVER");
    policy.request_payload["trigger_observation"] = observation.clone();
    policy.cause = "POLICY_FAILOVER".to_owned();
    policy.actor = json!({"service_id": "lead-failover"});
    policy.trigger_observation = Some(observation.clone());
    policy.event.payload["cause"] = json!("POLICY_FAILOVER");
    policy.event.payload["actor"] = policy.actor.clone();
    policy.event.payload["trigger_observation"] = observation;

    let error = commit_task_lead_handoff_transaction(&mut connection, policy, state_ref())
        .expect_err("fresh-looking caller input is not an authenticated provider proof");
    assert!(matches!(error, StoreError::Invalid(_)));
    let (lead, version): (String, i64) = connection
        .query_row(
            "SELECT lead_agent_binding_id, version FROM tasks",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read unchanged Task");
    let events: i64 = connection
        .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
        .expect("count events");
    let receipts: i64 = connection
        .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| row.get(0))
        .expect("count receipts");
    assert_eq!(lead, FROM);
    assert_eq!(version, 1);
    assert_eq!(events, 0);
    assert_eq!(receipts, 0);
}

#[test]
fn non_owner_cannot_commit_task_lead_handoff_even_with_a_canonical_request() {
    let mut connection = connection();
    let mut unauthorized = commit("handoff-non-owner");
    unauthorized.request_scope_id = "different-principal".to_owned();
    unauthorized.request_payload["request_scope_id"] = json!("different-principal");
    unauthorized.actor = json!({"principal_id": "different-principal", "kind": "USER"});
    unauthorized.event.payload["actor"] = unauthorized.actor.clone();

    let error = commit_task_lead_handoff_transaction(&mut connection, unauthorized, state_ref())
        .expect_err("Workspace ownership is enforced in the transaction");
    assert_eq!(error, StoreError::NotFound);
    let lead: String = connection
        .query_row("SELECT lead_agent_binding_id FROM tasks", [], |row| {
            row.get(0)
        })
        .expect("read unchanged Task");
    let receipts: i64 = connection
        .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| row.get(0))
        .expect("count receipts");
    assert_eq!(lead, FROM);
    assert_eq!(receipts, 0);
}
