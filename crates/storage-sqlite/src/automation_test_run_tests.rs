use super::*;
use domain_responsibility::{
    AmbiguousLocalTime, MisfirePolicy, NonexistentLocalTime, RecurrenceFormat, TestRunIdentity,
    TriggerDefinition, TriggerPlacement, TriggerSpec,
};
use rusqlite::{Connection, params};
use std::time::Duration;

const WORKSPACE: &str = "workspace-automation-test-run";
const NOW: &str = "2026-10-21T10:00:00Z";
const AUTOMATION: &str = "automation-test-run";
const ROUTINE: &str = "routine-test-run";
const DEFAULT_BINDING: &str = "binding-default";
const COWORKER_BINDING: &str = "binding-coworker";
const COWORKER: &str = "coworker-test-run";

struct Fixture {
    directory: tempfile::TempDir,
    store: SqliteWorkspaceStore,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    let workspace = create_workspace(&store, WORKSPACE);
    let connection = Connection::open(state_database(&directory)).expect("fixture database");
    let tx = connection
        .unchecked_transaction()
        .expect("fixture transaction");
    tx.execute(
        "INSERT INTO routines(routine_id, workspace_id, name, current_revision, status, created_at, updated_at, version)
         VALUES (?1, ?2, 'Test Routine', 1, 'ACTIVE', ?3, ?3, 1)",
        params![ROUTINE, WORKSPACE, NOW],
    )
    .expect("fixture Routine");
    tx.execute(
        "INSERT INTO routine_revisions(workspace_id, routine_id, revision, objective_template, instructions,
            input_schema_json, constraints_json, non_goals_json, required_outputs_json,
            acceptance_criteria_json, approvals_required_json, input_bindings_json,
            required_capabilities_json, preferred_agent_binding_id, placement_preference_json,
            budget_ceiling_json, verification_policy_json, authored_by_json, created_at)
         VALUES (?1, ?2, 1, 'Review project', 'Review the project', '{}', '[]', '[]', '[]',
            '[]', '[]', '[]', '[]', NULL, '\"AUTO\"', NULL, '{}',
            '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', ?3)",
        params![WORKSPACE, ROUTINE, NOW],
    )
    .expect("fixture Routine revision");
    for (profile, binding, enabled, lead) in [
        ("profile-default", DEFAULT_BINDING, true, true),
        ("profile-coworker", COWORKER_BINDING, true, true),
    ] {
        tx.execute(
            "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at)
             VALUES (?1, 'fixture', ?1, ?2)",
            params![profile, NOW],
        )
        .expect("fixture AgentProfile");
        tx.execute(
            "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id,
                endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version)
             VALUES (?1, ?2, ?3, '{}', '{}', ?4, ?5, ?6, 1)",
            params![binding, WORKSPACE, profile, enabled, lead, NOW],
        )
        .expect("fixture AgentBinding");
    }
    tx.execute(
        "UPDATE workspaces SET default_agent_binding_id = ?1 WHERE workspace_id = ?2",
        params![DEFAULT_BINDING, WORKSPACE],
    )
    .expect("fixture workspace lead");
    tx.execute(
        "INSERT INTO automations(automation_id, workspace_id, name, current_revision, status,
            created_at, updated_at, version)
         VALUES (?1, ?2, 'Test Automation', 1, 'PAUSED', ?3, ?3, 1)",
        params![AUTOMATION, WORKSPACE, NOW],
    )
    .expect("fixture Automation");
    tx.execute(
        "INSERT INTO automation_revisions(workspace_id, automation_id, revision, routine_id,
            routine_revision, coworker_id, coworker_revision, triggers_json,
            execution_policy_json, authored_by_json, created_at)
         VALUES (?1, ?2, 1, ?3, 1, NULL, NULL, '[]', '{}',
            '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', ?4)",
        params![WORKSPACE, AUTOMATION, ROUTINE, NOW],
    )
    .expect("fixture Automation revision");
    tx.execute(
        "INSERT INTO coworkers(coworker_id, workspace_id, current_revision, status,
            created_at, updated_at, version)
         VALUES (?1, ?2, 1, 'ACTIVE', ?3, ?3, 1)",
        params![COWORKER, WORKSPACE, NOW],
    )
    .expect("fixture Coworker");
    tx.execute(
        "INSERT INTO coworker_revisions(coworker_id, revision, workspace_id, name, role_description,
            default_lead_agent_binding_id, delegation_strategy, enabled_delegation_profile_ids_json,
            interaction_policy_json, context_policy_json, notification_policy_json,
            authored_by_json, created_at)
         VALUES (?1, 1, ?2, 'Test Coworker', 'Fixture Coworker', ?3, 'BALANCED', '[]',
            '{\"read_only_work\":\"STANDARD_TRUST_POLICY\",\"draft_creation\":\"STANDARD_TRUST_POLICY\",\"external_mutation\":\"STANDARD_TRUST_POLICY\",\"destructive_action\":\"HANDOFF_TO_OWNER\",\"financial_commitment\":\"HANDOFF_TO_OWNER\"}',
            '{}', '{}', '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', ?4)",
        params![COWORKER, WORKSPACE, COWORKER_BINDING, NOW],
    )
    .expect("fixture Coworker revision");
    tx.commit().expect("fixture commits");
    let _ = workspace;
    Fixture { directory, store }
}

fn command_key(request_id: &str, automation_revision: u64) -> String {
    TestRunIdentity {
        automation_id: AUTOMATION.to_owned(),
        automation_revision,
        principal_id: "owner-local".to_owned(),
        request_id: request_id.to_owned(),
    }
    .key()
    .expect("fixture test-run identity")
}

fn test_run_commit(
    task_id: &str,
    request_id: &str,
    automation_revision: u64,
    expected_automation_version: u64,
    test_run_id: &str,
    coworker: Option<(&str, u64)>,
    lead_binding: &str,
) -> storage_core::TaskCreateCommit {
    let created_by = serde_json::json!({"kind": "USER", "principal_id": "owner-local"});
    let inputs = serde_json::json!({});
    let failover = serde_json::json!({
        "mode": "DISABLED",
        "triggers": [],
        "fallback_agent_binding_ids": [],
        "max_lead_changes": 0
    });
    let request_payload = serde_json::json!({
        "workspace_id": WORKSPACE,
        "conversation_id": null,
        "source_message_refs": [],
        "objective": "Review project",
        "constraints": ["Routine instructions (untrusted task context):\nReview the project"],
        "non_goals": [],
        "input_refs": [],
        "required_outputs": [],
        "acceptance_criteria": [],
        "approvals_required": [],
        "budget": null,
        "placement_preference": "AUTO",
        "preferred_lead_agent_binding_id": lead_binding,
        "coworker_id": coworker.map(|(id, _)| id),
        "routine_id": ROUTINE,
        "routine_revision": 1,
        "routine_inputs": inputs,
        "automation_id": AUTOMATION,
        "automation_revision": automation_revision,
        "expected_automation_version": expected_automation_version,
        "automation_test_run_id": test_run_id,
    });
    let idempotency_payload = serde_json::json!({
        "operation": "automation.test_run.v1",
        "workspace_id": WORKSPACE,
        "automation_id": AUTOMATION,
        "automation_revision": automation_revision,
        "expected_automation_version": expected_automation_version,
        "routine_id": ROUTINE,
        "routine_revision": 1,
        "inputs": inputs,
        "test_run_id": test_run_id,
    });
    storage_core::TaskCreateCommit {
        request: storage_core::WorkspaceCreateRequest {
            principal_id: "owner-local".to_owned(),
            request_id: request_id.to_owned(),
            request_payload: request_payload.clone(),
        },
        idempotency_payload: Some(idempotency_payload),
        expected_coworker_version: None,
        routine_admission: Some(storage_core::RoutineTaskAdmission {
            routine_id: ROUTINE.to_owned(),
            routine_revision: 1,
            inputs,
        }),
        automation_admission: None,
        automation_test_admission: Some(storage_core::AutomationTestAdmission {
            test_run_id: test_run_id.to_owned(),
            command_key: command_key(request_id, automation_revision),
            automation_id: AUTOMATION.to_owned(),
            automation_revision,
            expected_automation_version,
            routine_id: ROUTINE.to_owned(),
            routine_revision: 1,
            inputs: serde_json::json!({}),
        }),
        task: storage_core::TaskRecord {
            task_id: task_id.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            conversation_id: None,
            current_spec_revision: 1,
            current_plan_revision: None,
            status: "READY".to_owned(),
            resume_status: None,
            routine_id: Some(ROUTINE.to_owned()),
            routine_revision: Some(1),
            automation_id: None,
            automation_occurrence_id: None,
            automation_test_run_id: Some(test_run_id.to_owned()),
            origin_coworker_id: coworker.map(|(id, _)| id.to_owned()),
            origin_coworker_revision: coworker.map(|(_, revision)| revision),
            lead_agent_binding_id: lead_binding.to_owned(),
            blocking_conditions: Vec::new(),
            priority: "NORMAL".to_owned(),
            created_by: created_by.clone(),
            created_at: NOW.to_owned(),
            updated_at: NOW.to_owned(),
            completed_at: None,
            version: 1,
        },
        initial_spec_revision: storage_core::TaskSpecRevisionRecord {
            task_id: task_id.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            revision: 1,
            parent_revisions: Vec::new(),
            objective: "Review project".to_owned(),
            task_category: None,
            constraints: vec![
                "Routine instructions (untrusted task context):\nReview the project".to_owned(),
            ],
            non_goals: Vec::new(),
            input_refs: Vec::new(),
            workspace_instruction_revision: None,
            required_outputs: Vec::new(),
            acceptance_criteria: Vec::new(),
            approvals_required: Vec::new(),
            budget: None,
            delegation_budget_policy: None,
            lead_failover_policy: failover,
            deadline: None,
            source_message_refs: Vec::new(),
            placement_preference: serde_json::json!("AUTO"),
            preferred_lead_agent_binding_id: Some(lead_binding.to_owned()),
            authored_by: created_by,
            created_at: NOW.to_owned(),
        },
        event: storage_core::EventDraft {
            event_id: format!("ev-test-run-{task_id}"),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "Task".to_owned(),
            entity_id: task_id.to_owned(),
            origin_runtime_id: "runtime-automation-test-run".to_owned(),
            entity_revision: 1,
            hlc_timestamp: NOW.to_owned(),
            correlation_id: "correlation-automation-test-run".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "task.created.v1".to_owned(),
            payload: serde_json::json!({
                "task_id": task_id,
                "initial_spec_revision": 1,
                "created_by": {"kind": "USER", "principal_id": "owner-local"},
                "origin_coworker_id": coworker.map(|(id, _)| id),
                "origin_coworker_revision": coworker.map(|(_, revision)| revision),
                "routine_id": ROUTINE,
                "routine_revision": 1,
                "automation_test_run_id": test_run_id,
            }),
            recorded_at: NOW.to_owned(),
        },
    }
}

fn counts(connection: &Connection) -> (i64, i64, i64, i64, i64) {
    connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1),
               (SELECT COUNT(*) FROM automation_test_runs WHERE workspace_id = ?1),
               (SELECT COUNT(*) FROM automation_occurrences WHERE workspace_id = ?1),
               (SELECT COUNT(*) FROM automation_cursors WHERE workspace_id = ?1),
               (SELECT COUNT(*) FROM request_dedup WHERE principal_id = 'owner-local')",
            [WORKSPACE],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("counts")
}

#[test]
fn paused_test_run_uses_workspace_lead_and_no_occurrence() {
    let fixture = fixture();
    let commit = test_run_commit(
        "task-test-run-default",
        "request-test-run-default",
        1,
        1,
        "test-run-default",
        None,
        DEFAULT_BINDING,
    );
    let committed = fixture
        .store
        .create_task(commit.clone())
        .expect("the paused Automation admits one READY test Task");
    assert_eq!(committed.view.task.status, "READY");
    assert_eq!(committed.view.task.automation_id, None);
    assert_eq!(committed.view.task.automation_occurrence_id, None);
    assert_eq!(
        committed.view.task.automation_test_run_id.as_deref(),
        Some("test-run-default")
    );
    assert_eq!(committed.view.task.lead_agent_binding_id, DEFAULT_BINDING);
    assert_eq!(committed.view.task.origin_coworker_id, None);
    assert_eq!(
        committed.event.payload["automation_test_run_id"],
        "test-run-default"
    );
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    assert_eq!(counts(&connection), (1, 1, 0, 0, 1));
    let linked: (String, String, i64) = connection
        .query_row(
            "SELECT task_id, routine_id, version FROM automation_test_runs WHERE test_run_id = ?1",
            ["test-run-default"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("test-run row");
    assert_eq!(
        linked,
        ("task-test-run-default".to_owned(), ROUTINE.to_owned(), 2)
    );

    let replay = fixture
        .store
        .create_task(commit)
        .expect("the exact request replays the committed Task");
    assert_eq!(replay.view.task.task_id, "task-test-run-default");
    assert_eq!(counts(&connection), (1, 1, 0, 0, 1));
}

#[test]
fn enabled_test_run_uses_the_pinned_coworker_lead() {
    let fixture = fixture();
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    connection
        .execute(
            "UPDATE automations SET status = 'ENABLED', version = 2 WHERE automation_id = ?1",
            [AUTOMATION],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE automation_revisions SET coworker_id = ?1, coworker_revision = 1
             WHERE automation_id = ?2 AND revision = 1",
            params![COWORKER, AUTOMATION],
        )
        .unwrap();
    let committed = fixture
        .store
        .create_task(test_run_commit(
            "task-test-run-coworker",
            "request-test-run-coworker",
            1,
            2,
            "test-run-coworker",
            Some((COWORKER, 1)),
            COWORKER_BINDING,
        ))
        .expect("the pinned Coworker admits the test Task");
    assert_eq!(
        committed.view.task.origin_coworker_id.as_deref(),
        Some(COWORKER)
    );
    assert_eq!(committed.view.task.origin_coworker_revision, Some(1));
    assert_eq!(committed.view.task.lead_agent_binding_id, COWORKER_BINDING);
    assert_eq!(committed.view.task.automation_id, None);
    assert_eq!(
        committed.view.task.automation_test_run_id.as_deref(),
        Some("test-run-coworker")
    );
}

#[test]
fn a_schedule_trigger_does_not_create_a_cursor_for_a_test_run() {
    let fixture = fixture();
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    let trigger = TriggerSpec {
        trigger_id: "schedule-1".to_owned(),
        placement: TriggerPlacement::Auto,
        runtime_id: None,
        trigger: TriggerDefinition::Schedule {
            recurrence_format: RecurrenceFormat::Cron5,
            timezone: "UTC".to_owned(),
            rrule_or_cron: "0 9 * * *".to_owned(),
            recurrence_semantics_version: 1,
            start_at: None,
            end_at: None,
            misfire_policy: MisfirePolicy::Skip,
            ambiguous_local_time: AmbiguousLocalTime::Earlier,
            nonexistent_local_time: NonexistentLocalTime::Skip,
        },
    };
    connection
        .execute(
            "UPDATE automation_revisions SET triggers_json = ?1
             WHERE automation_id = ?2 AND revision = 1",
            params![serde_json::to_string(&vec![trigger]).unwrap(), AUTOMATION],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE automations SET status = 'ENABLED', version = 2 WHERE automation_id = ?1",
            [AUTOMATION],
        )
        .unwrap();
    let committed = fixture
        .store
        .create_task(test_run_commit(
            "task-test-run-schedule",
            "request-test-run-schedule",
            1,
            2,
            "test-run-schedule",
            None,
            DEFAULT_BINDING,
        ))
        .expect("a test run does not require a trigger host or cursor");
    assert_eq!(committed.view.task.status, "READY");
    assert_eq!(counts(&connection), (1, 1, 0, 0, 1));
}

#[test]
fn disabled_automation_test_run_is_refused_with_no_rows() {
    let fixture = fixture();
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    connection
        .execute(
            "UPDATE automations SET status = 'DISABLED', version = 2 WHERE automation_id = ?1",
            [AUTOMATION],
        )
        .unwrap();
    let error = fixture
        .store
        .create_task(test_run_commit(
            "task-test-run-disabled",
            "request-test-run-disabled",
            1,
            2,
            "test-run-disabled",
            None,
            DEFAULT_BINDING,
        ))
        .expect_err("a DISABLED Automation cannot accept a test run");
    assert!(matches!(error, StoreError::Invalid(_)));
    assert_eq!(counts(&connection), (0, 0, 0, 0, 0));
}

#[test]
fn a_stale_automation_version_or_revision_is_refused_without_rows() {
    let fixture = fixture();
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    let stale_version = fixture
        .store
        .create_task(test_run_commit(
            "task-test-run-stale-version",
            "request-test-run-stale-version",
            1,
            2,
            "test-run-stale-version",
            None,
            DEFAULT_BINDING,
        ))
        .expect_err("the expected Automation version must match");
    assert!(matches!(stale_version, StoreError::Conflict { .. }));
    let stale_revision = fixture
        .store
        .create_task(test_run_commit(
            "task-test-run-stale-revision",
            "request-test-run-stale-revision",
            2,
            1,
            "test-run-stale-revision",
            None,
            DEFAULT_BINDING,
        ))
        .expect_err("the exact current revision must match");
    assert!(matches!(
        stale_revision,
        StoreError::Conflict { .. } | StoreError::NotFound
    ));
    assert_eq!(counts(&connection), (0, 0, 0, 0, 0));
}

#[test]
fn test_run_and_occurrence_admission_are_mutually_exclusive() {
    let fixture = fixture();
    let mut commit = test_run_commit(
        "task-test-run-mixed",
        "request-test-run-mixed",
        1,
        1,
        "test-run-mixed",
        None,
        DEFAULT_BINDING,
    );
    commit.automation_admission = Some(storage_core::AutomationTaskAdmission {
        automation_id: AUTOMATION.to_owned(),
        automation_revision: 1,
        expected_automation_version: 1,
        routine_id: ROUTINE.to_owned(),
        routine_revision: 1,
        trigger_id: "trigger-mixed".to_owned(),
        trigger_host_runtime_id: "runtime-automation-test-run".to_owned(),
        trigger_host_runtime_incarnation_id: "incarnation-mixed".to_owned(),
        trigger_host_binding_version: 1,
        occurrence_id: "occurrence-mixed".to_owned(),
        occurrence_key: "b".repeat(64),
        claim_expires_at: "2026-10-21T10:02:00Z".to_owned(),
        trigger_kind: "MANUAL".to_owned(),
        scheduled_epoch_ms: None,
        covered_misfire_range: None,
    });
    let error = fixture
        .store
        .create_task(commit)
        .expect_err("occurrence and test-run admission cannot be combined");
    assert!(matches!(error, StoreError::Invalid(_)));
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    assert_eq!(counts(&connection), (0, 0, 0, 0, 0));
}

#[test]
fn a_changed_test_run_payload_under_one_request_key_is_a_conflict() {
    let fixture = fixture();
    let commit = test_run_commit(
        "task-test-run-replay",
        "request-test-run-replay",
        1,
        1,
        "test-run-replay",
        None,
        DEFAULT_BINDING,
    );
    fixture
        .store
        .create_task(commit)
        .expect("the first test run commits");
    let changed = test_run_commit(
        "task-test-run-replay",
        "request-test-run-replay",
        1,
        1,
        "test-run-replay-changed",
        None,
        DEFAULT_BINDING,
    );
    let error = fixture
        .store
        .create_task(changed)
        .expect_err("a changed request payload conflicts");
    assert!(matches!(error, StoreError::Conflict { .. }));
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    assert_eq!(counts(&connection), (1, 1, 0, 0, 1));
}

#[test]
fn a_late_test_run_failure_rolls_back_the_task_provenance_and_receipt() {
    let fixture = fixture();
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_test_run_receipt BEFORE INSERT ON request_dedup
             WHEN NEW.request_id = 'request-test-run-late-failure'
             BEGIN SELECT RAISE(ABORT, 'injected test-run receipt failure'); END;",
        )
        .unwrap();
    let error = fixture
        .store
        .create_task(test_run_commit(
            "task-test-run-late-failure",
            "request-test-run-late-failure",
            1,
            1,
            "test-run-late-failure",
            None,
            DEFAULT_BINDING,
        ))
        .expect_err("the injected receipt failure aborts the commit");
    assert!(matches!(error, StoreError::Database(_)));
    assert_eq!(counts(&connection), (0, 0, 0, 0, 0));

    connection
        .execute_batch("DROP TRIGGER fail_test_run_receipt;")
        .unwrap();
    let committed = fixture
        .store
        .create_task(test_run_commit(
            "task-test-run-late-failure",
            "request-test-run-late-failure",
            1,
            1,
            "test-run-late-failure",
            None,
            DEFAULT_BINDING,
        ))
        .expect("the same request commits after the fault is removed");
    assert_eq!(committed.view.task.task_id, "task-test-run-late-failure");
    assert_eq!(counts(&connection), (1, 1, 0, 0, 1));
}
