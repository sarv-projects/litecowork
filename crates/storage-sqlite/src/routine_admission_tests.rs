//! Transaction evidence for a manual Routine run.
//!
//! The served `POST /v1/routines/{routineId}/run` route materializes a saved
//! Routine revision into an ordinary READY Task and adds Routine admission to
//! the same Task transaction. These tests exercise that storage transaction
//! directly: the exact TaskSpec revision 1 and its pins, the `task.created.v1`
//! event, the idempotency receipt, replay, the changed-payload conflict, the
//! stale-head refusal, invalid inputs, and the absence of any Plan, Step,
//! Attempt, or AgentSession row.

use super::*;
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use rusqlite::{Connection, params};
use std::time::Duration;

const WORKSPACE: &str = "workspace-routine-admission";
const NOW: &str = "2026-10-21T10:00:00Z";

/// A Workspace with one enabled lead AgentBinding, that binding as the
/// Workspace default, and one ACTIVE Routine revision.
fn fixture(input_schema_json: &str) -> (tempfile::TempDir, SqliteWorkspaceStore) {
    fixture_with(input_schema_json, None, "[]")
}

/// The same fixture with an optional alternate lead pinned by the revision.
fn fixture_with_lead(
    input_schema_json: &str,
    preferred_lead: Option<&str>,
) -> (tempfile::TempDir, SqliteWorkspaceStore) {
    fixture_with(input_schema_json, preferred_lead, "[]")
}

/// The base fixture with explicit input bindings for ResourceRef coverage.
fn fixture_with(
    input_schema_json: &str,
    preferred_lead: Option<&str>,
    input_bindings_json: &str,
) -> (tempfile::TempDir, SqliteWorkspaceStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE);

    let connection = Connection::open(state_database(&directory)).unwrap();
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at)
             VALUES ('profile-routine', 'fixture', 'Fixture Agent', '2026-10-20T08:00:00Z')",
            [],
        )
        .expect("fixture AgentProfile");
    for binding_id in ["binding-routine", "binding-routine-2"] {
        connection
            .execute(
                "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version)
                 VALUES (?1, ?2, 'profile-routine', '{}', '{}', 1, 1, '2026-10-20T08:00:00Z', 1)",
                params![binding_id, WORKSPACE],
            )
            .expect("fixture AgentBinding");
    }
    connection
        .execute(
            "UPDATE workspaces SET default_agent_binding_id = 'binding-routine' WHERE workspace_id = ?1",
            params![WORKSPACE],
        )
        .expect("the Workspace default binding is set");
    // `routines.current_revision` is a deferred foreign key to the revision row,
    // so both inserts must share one transaction.
    let tx = connection.unchecked_transaction().unwrap();
    tx.execute(
        "INSERT INTO routines(routine_id, workspace_id, name, current_revision, status, created_at, updated_at, version)
         VALUES ('routine-1', ?1, 'Routine', 1, 'ACTIVE', '2026-10-20T08:00:00Z', '2026-10-20T08:00:00Z', 1)",
        params![WORKSPACE],
    )
    .expect("fixture Routine");
    tx.execute(
        "INSERT INTO routine_revisions(workspace_id, routine_id, revision, objective_template, instructions, input_schema_json, constraints_json, non_goals_json, required_outputs_json, acceptance_criteria_json, approvals_required_json, input_bindings_json, required_capabilities_json, preferred_agent_binding_id, placement_preference_json, budget_ceiling_json, verification_policy_json, authored_by_json, created_at)
         VALUES (?1, 'routine-1', 1, 'Review project', 'Review the project', ?2, '[]', '[]', '[]', '[]', '[]', ?4, '[]', ?3, '\"AUTO\"', NULL, '{}', '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', '2026-10-20T08:00:00Z')",
        params![WORKSPACE, input_schema_json, preferred_lead, input_bindings_json],
    )
    .expect("fixture Routine revision");
    tx.commit().unwrap();
    (directory, store)
}

/// The atomic Task commit the Run route builds for the fixture Routine.
fn routine_commit(
    task_id: &str,
    request_id: &str,
    routine_revision: u64,
    inputs: serde_json::Value,
) -> storage_core::TaskCreateCommit {
    routine_commit_with_lead(
        task_id,
        request_id,
        routine_revision,
        inputs,
        "binding-routine",
    )
}

fn routine_commit_with_lead(
    task_id: &str,
    request_id: &str,
    routine_revision: u64,
    inputs: serde_json::Value,
    lead_binding: &str,
) -> storage_core::TaskCreateCommit {
    routine_commit_with(
        task_id,
        request_id,
        routine_revision,
        inputs,
        lead_binding,
        Vec::new(),
    )
}

fn routine_commit_with(
    task_id: &str,
    request_id: &str,
    routine_revision: u64,
    inputs: serde_json::Value,
    lead_binding: &str,
    input_refs: Vec<serde_json::Value>,
) -> storage_core::TaskCreateCommit {
    let request_payload = serde_json::json!({
        "workspace_id": WORKSPACE,
        "conversation_id": null,
        "source_message_refs": [],
        "objective": "Review project",
        "constraints": ["Routine instructions (untrusted task context):\nReview the project"],
        "non_goals": [],
        "input_refs": input_refs.clone(),
        "required_outputs": [],
        "acceptance_criteria": [],
        "approvals_required": [],
        "budget": null,
        "placement_preference": "AUTO",
        "preferred_lead_agent_binding_id": lead_binding,
        "routine_id": "routine-1",
        "routine_revision": routine_revision,
        "routine_inputs": inputs,
    });
    // The route binds the receipt to the owner's canonical Run command rather
    // than to materialized defaults; the spec is still validated against the
    // saved revision inside the transaction.
    let receipt_payload = serde_json::json!({
        "operation": "routine.run.v1",
        "workspace_id": WORKSPACE,
        "routine_id": "routine-1",
        "routine_revision": routine_revision,
        "inputs": inputs,
    });
    let created_by = serde_json::json!({"kind": "USER", "principal_id": "owner-local"});
    storage_core::TaskCreateCommit {
        request: storage_core::WorkspaceCreateRequest {
            principal_id: "owner-local".to_owned(),
            request_id: request_id.to_owned(),
            request_payload: request_payload.clone(),
        },
        idempotency_payload: Some(receipt_payload),
        expected_coworker_version: None,
        routine_admission: Some(storage_core::RoutineTaskAdmission {
            routine_id: "routine-1".to_owned(),
            routine_revision,
            inputs: request_payload["routine_inputs"].clone(),
        }),
        automation_admission: None,
        task: storage_core::TaskRecord {
            task_id: task_id.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            conversation_id: None,
            current_spec_revision: 1,
            current_plan_revision: None,
            status: "READY".to_owned(),
            resume_status: None,
            routine_id: Some("routine-1".to_owned()),
            routine_revision: Some(routine_revision),
            automation_id: None,
            automation_occurrence_id: None,
            origin_coworker_id: None,
            origin_coworker_revision: None,
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
            input_refs,
            workspace_instruction_revision: None,
            required_outputs: Vec::new(),
            acceptance_criteria: Vec::new(),
            approvals_required: Vec::new(),
            budget: None,
            delegation_budget_policy: None,
            lead_failover_policy: serde_json::json!({
                "mode": "DISABLED",
                "triggers": [],
                "fallback_agent_binding_ids": [],
                "max_lead_changes": 0
            }),
            deadline: None,
            source_message_refs: Vec::new(),
            placement_preference: serde_json::json!("AUTO"),
            preferred_lead_agent_binding_id: Some(lead_binding.to_owned()),
            authored_by: created_by,
            created_at: NOW.to_owned(),
        },
        event: storage_core::EventDraft {
            event_id: format!("ev-routine-{task_id}"),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "Task".to_owned(),
            entity_id: task_id.to_owned(),
            origin_runtime_id: "runtime-routine-admission".to_owned(),
            entity_revision: 1,
            hlc_timestamp: NOW.to_owned(),
            correlation_id: "correlation-routine".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "task.created.v1".to_owned(),
            payload: serde_json::json!({
                "task_id": task_id,
                "initial_spec_revision": 1,
                "created_by": {"kind": "USER", "principal_id": "owner-local"},
                "origin_coworker_id": null,
                "origin_coworker_revision": null,
                "routine_id": "routine-1",
                "routine_revision": routine_revision,
            }),
            recorded_at: NOW.to_owned(),
        },
    }
}

fn scalar(connection: &Connection, sql: &str, task_id: &str) -> i64 {
    connection
        .query_row(sql, params![task_id], |row| row.get(0))
        .unwrap()
}

#[test]
fn a_manual_routine_run_persists_one_spec_pinned_ready_task_and_receipt() {
    let (directory, store) = fixture("{}");
    let commit = routine_commit("task-routine-1", "routine-run-1", 1, serde_json::json!({}));
    let committed = store
        .create_task(commit.clone())
        .expect("the saved Routine revision materializes one READY Task");

    assert_eq!(committed.view.task.status, "READY");
    assert_eq!(committed.view.task.routine_id.as_deref(), Some("routine-1"));
    assert_eq!(committed.view.task.routine_revision, Some(1));
    assert_eq!(committed.view.task.automation_id, None);
    assert_eq!(committed.view.task.automation_occurrence_id, None);
    assert_eq!(committed.view.task.version, 1);
    assert_eq!(committed.view.task.lead_agent_binding_id, "binding-routine");
    assert_eq!(committed.view.current_spec_revision.revision, 1);
    assert!(
        committed
            .view
            .current_spec_revision
            .parent_revisions
            .is_empty()
    );
    assert_eq!(
        committed.view.current_spec_revision.objective,
        "Review project"
    );
    assert_eq!(committed.event.event_type, "task.created.v1");
    assert_eq!(committed.event.entity_revision, 1);
    assert_eq!(committed.event.payload["routine_id"], "routine-1");
    assert_eq!(committed.event.payload["routine_revision"], 1);
    assert_eq!(
        committed.event.payload["automation_id"],
        serde_json::Value::Null
    );

    let connection = Connection::open(state_database(&directory)).unwrap();
    let (tasks, specs, events): (i64, i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1), (SELECT COUNT(*) FROM task_spec_revisions WHERE workspace_id = ?1), (SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'task.created.v1')",
            params![WORKSPACE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((tasks, specs, events), (1, 1, 1));
    for sql in [
        "SELECT COUNT(*) FROM plan_revisions WHERE task_id = ?1",
        "SELECT COUNT(*) FROM steps WHERE task_id = ?1",
        "SELECT COUNT(*) FROM attempts WHERE task_id = ?1",
        "SELECT COUNT(*) FROM agent_sessions WHERE task_id = ?1",
    ] {
        assert_eq!(scalar(&connection, sql, "task-routine-1"), 0);
    }
    let (receipts, response): (i64, Option<String>) = connection
        .query_row(
            "SELECT COUNT(*), MAX(response_json) FROM request_dedup WHERE principal_id = 'owner-local' AND request_id = 'routine-run-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(receipts, 1);
    assert!(response.is_some_and(|value| value.contains("task-routine-1")));

    // An exact replay returns the committed Task and event and writes nothing.
    let replay = store
        .create_task(commit)
        .expect("the exact Run request replays its committed receipt");
    assert_eq!(replay.view.task.task_id, committed.view.task.task_id);
    assert_eq!(replay.event.event_id, committed.event.event_id);
    let (tasks, specs, events): (i64, i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1), (SELECT COUNT(*) FROM task_spec_revisions WHERE workspace_id = ?1), (SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'task.created.v1')",
            params![WORKSPACE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((tasks, specs, events), (1, 1, 1));
}

#[test]
fn a_changed_routine_run_payload_under_the_same_request_key_is_a_conflict() {
    let (_directory, store) = fixture("{}");
    let commit = routine_commit("task-routine-1", "routine-run-1", 1, serde_json::json!({}));
    store
        .create_task(commit.clone())
        .expect("the saved Routine revision materializes one READY Task");

    let mut changed = commit.clone();
    changed.idempotency_payload = Some(serde_json::json!({
        "operation": "routine.run.v1",
        "workspace_id": WORKSPACE,
        "routine_id": "routine-1",
        "routine_revision": 1,
        "inputs": {"topic": "a different request"}
    }));
    assert!(matches!(
        store.create_task(changed),
        Err(StoreError::Conflict { .. })
    ));

    // The conflict leaves the original receipt and Task replayable.
    let replay = store
        .create_task(commit)
        .expect("the original Run request still replays its committed receipt");
    assert_eq!(replay.view.task.task_id, "task-routine-1");
}

#[test]
fn a_stale_routine_head_is_refused_without_writing_a_task() {
    let (directory, store) = fixture("{}");
    let connection = Connection::open(state_database(&directory)).unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    // The append guard reads the head, so move it to revision 2 before the
    // revision row exists; the head's deferred foreign key resolves at commit.
    tx.execute(
        "UPDATE routines SET current_revision = 2, version = 2 WHERE workspace_id = ?1 AND routine_id = 'routine-1'",
        params![WORKSPACE],
    )
    .expect("the Routine head moves to the later revision");
    tx.execute(
        "INSERT INTO routine_revisions(workspace_id, routine_id, revision, objective_template, instructions, input_schema_json, constraints_json, non_goals_json, required_outputs_json, acceptance_criteria_json, approvals_required_json, input_bindings_json, required_capabilities_json, preferred_agent_binding_id, placement_preference_json, budget_ceiling_json, verification_policy_json, authored_by_json, created_at)
         VALUES (?1, 'routine-1', 2, 'Review project', 'Review the project', '{}', '[]', '[]', '[]', '[]', '[]', '[]', '[]', NULL, '\"AUTO\"', NULL, '{}', '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', '2026-10-20T09:00:00Z')",
        params![WORKSPACE],
    )
    .expect("a later Routine revision exists");
    tx.commit().unwrap();

    let error = store
        .create_task(routine_commit(
            "task-routine-1",
            "routine-run-1",
            1,
            serde_json::json!({}),
        ))
        .expect_err("a run pinned to the superseded revision is refused");
    assert!(matches!(error, StoreError::Conflict { .. }));

    let (tasks, receipts): (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1), (SELECT COUNT(*) FROM request_dedup WHERE request_id = 'routine-run-1')",
            params![WORKSPACE],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((tasks, receipts), (0, 0));
}

#[test]
fn an_ineligible_routine_lead_is_refused_without_a_task() {
    let (directory, store) = fixture_with_lead("{}", Some("binding-routine-2"));
    let connection = Connection::open(state_database(&directory)).unwrap();
    connection
        .execute(
            "UPDATE agent_bindings SET enabled = 0 WHERE agent_binding_id = 'binding-routine-2'",
            [],
        )
        .expect("the pinned lead binding becomes ineligible");

    let error = store
        .create_task(routine_commit_with_lead(
            "task-routine-1",
            "routine-run-1",
            1,
            serde_json::json!({}),
            "binding-routine-2",
        ))
        .expect_err("an ineligible lead is refused");
    assert!(matches!(error, StoreError::Invalid(_)));

    let (tasks, receipts): (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1), (SELECT COUNT(*) FROM request_dedup WHERE request_id = 'routine-run-1')",
            params![WORKSPACE],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((tasks, receipts), (0, 0));
}

#[test]
fn routine_inputs_that_do_not_materialize_are_refused_without_a_task() {
    let (directory, store) = fixture(
        r#"{"type":"object","required":["topic"],"properties":{"topic":{"type":"string"}},"additionalProperties":false}"#,
    );
    let error = store
        .create_task(routine_commit(
            "task-routine-1",
            "routine-run-1",
            1,
            serde_json::json!({}),
        ))
        .expect_err("missing required Routine input is refused");
    assert!(matches!(error, StoreError::Invalid(_)));

    let connection = Connection::open(state_database(&directory)).unwrap();
    let (tasks, events, receipts): (i64, i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1), (SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'task.created.v1'), (SELECT COUNT(*) FROM request_dedup WHERE request_id = 'routine-run-1')",
            params![WORKSPACE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((tasks, events, receipts), (0, 0, 0));
}

#[test]
fn a_foreign_principal_cannot_replay_a_routine_task_receipt() {
    let (directory, store) = fixture("{}");
    store
        .create_task(routine_commit(
            "task-routine-1",
            "routine-run-1",
            1,
            serde_json::json!({}),
        ))
        .expect("the saved Routine revision materializes one READY Task");

    let mut foreign = routine_commit("task-routine-2", "routine-run-1", 1, serde_json::json!({}));
    foreign.request.principal_id = "intruder".to_owned();
    foreign.task.created_by = serde_json::json!({"kind": "USER", "principal_id": "intruder"});
    foreign.initial_spec_revision.authored_by =
        serde_json::json!({"kind": "USER", "principal_id": "intruder"});
    foreign.event.payload["created_by"] =
        serde_json::json!({"kind": "USER", "principal_id": "intruder"});
    assert!(store.create_task(foreign).is_err());

    let connection = Connection::open(state_database(&directory)).unwrap();
    let (tasks, receipts): (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1), (SELECT COUNT(*) FROM request_dedup WHERE request_id = 'routine-run-1' AND principal_id = 'owner-local')",
            params![WORKSPACE],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((tasks, receipts), (1, 1));
}

const RESOURCE_SCHEMA: &str = r#"{"type":"object","properties":{"ref":{"type":"object","properties":{"workspace_id":{"type":"string"},"resource_id":{"type":"string"},"revision_id":{"type":"string"}},"required":["workspace_id","resource_id","revision_id"],"additionalProperties":false}},"required":["ref"],"additionalProperties":false}"#;
const RESOURCE_BINDINGS: &str = r#"[{"source_pointer":"/ref","template_variable":"ref","value_kind":"RESOURCE_REF","required":true}]"#;

fn create_other_workspace(store: &SqliteWorkspaceStore, workspace_id: &str) {
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Other workspace".to_owned(),
            owner_principal_id: "owner-local".to_owned(),
            event: EventContext {
                event_id: format!("event-create-{workspace_id}"),
                origin_runtime_id: "runtime-routine-admission".to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: format!("correlation-{workspace_id}"),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("other Workspace commits");
}

fn create_resource(store: &SqliteWorkspaceStore, workspace_id: &str, resource_id: &str) {
    let revision_id = format!("revision-{resource_id}");
    let content = format!("routine resource content for {resource_id}");
    let content_digest = digest(content.as_bytes());
    let resource = storage_core::ResourceRecord {
        resource_id: resource_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        kind: "FILE".to_owned(),
        provider_identity: serde_json::json!({}),
        identity_digest: None,
        display_name: resource_id.to_owned(),
        current_revision_id: Some(revision_id.clone()),
        sensitivity: "PERSONAL".to_owned(),
        provenance: serde_json::json!({
            "source_inputs": [],
            "transformations": [],
            "tool_reports": []
        }),
        created_at: NOW.to_owned(),
        updated_at: NOW.to_owned(),
        version: 1,
    };
    let revision = storage_core::ResourceRevisionRecord {
        resource_revision_id: revision_id,
        resource_id: resource_id.to_owned(),
        parent_revision_ids: Vec::new(),
        provider_revision: None,
        content_digest: Some(content_digest.clone()),
        size_bytes: Some(content.len() as u64),
        media_type: Some("text/plain".to_owned()),
        observed_at: NOW.to_owned(),
        created_by: serde_json::json!({"kind": "USER", "principal_id": "owner-local"}),
    };
    let event = storage_core::EventDraft {
        event_id: format!("event-resource-{resource_id}"),
        workspace_id: workspace_id.to_owned(),
        entity_type: "Resource".to_owned(),
        entity_id: resource_id.to_owned(),
        origin_runtime_id: "runtime-routine-admission".to_owned(),
        entity_revision: 1,
        hlc_timestamp: NOW.to_owned(),
        correlation_id: format!("event-resource-{resource_id}"),
        causation_id: None,
        schema_version: 1,
        event_type: "resource.created.v1".to_owned(),
        payload: serde_json::json!({
            "resource_id": resource_id,
            "workspace_id": workspace_id,
            "kind": "FILE",
            "provenance": {
                "source_inputs": [],
                "transformations": [],
                "tool_reports": []
            },
            "aggregate_version": 1
        }),
        recorded_at: NOW.to_owned(),
    };
    store
        .create_resource(
            storage_core::WorkspaceCreateRequest {
                principal_id: "owner-local".to_owned(),
                request_id: format!("request-resource-{resource_id}"),
                request_payload: serde_json::json!({
                    "operation": "resource.quick_import.v1",
                    "workspace_id": workspace_id,
                    "resource_id": resource_id,
                    "display_name": resource_id,
                    "media_type": "text/plain",
                    "content_digest": content_digest,
                    "size_bytes": content.len(),
                }),
            },
            resource,
            revision,
            event,
            content.into_bytes(),
        )
        .expect("fixture Resource commits");
}

#[test]
fn a_resource_ref_input_pins_the_exact_workspace_revision() {
    let (directory, store) = fixture_with(RESOURCE_SCHEMA, None, RESOURCE_BINDINGS);
    create_resource(&store, WORKSPACE, "resource-routine");
    let pin = serde_json::json!({
        "workspace_id": WORKSPACE,
        "resource_id": "resource-routine",
        "revision_id": "revision-resource-routine"
    });
    let commit = routine_commit_with(
        "task-routine-ref",
        "routine-run-ref",
        1,
        serde_json::json!({"ref": pin}),
        "binding-routine",
        vec![pin.clone()],
    );
    let committed = store
        .create_task(commit.clone())
        .expect("the exact ResourceRef pin materializes one READY Task");
    assert_eq!(committed.view.task.status, "READY");
    assert_eq!(
        committed.view.current_spec_revision.input_refs,
        vec![pin.clone()]
    );

    let connection = Connection::open(state_database(&directory)).unwrap();
    let (tasks, refs): (i64, String) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1), input_refs_json FROM task_spec_revisions WHERE task_id = 'task-routine-ref'",
            params![WORKSPACE],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(tasks, 1);
    assert!(refs.contains("resource-routine"));
    assert!(refs.contains("revision-resource-routine"));

    let replay = store
        .create_task(commit)
        .expect("the exact ResourceRef request replays its receipt");
    assert_eq!(replay.view.task.task_id, "task-routine-ref");
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1",
                params![WORKSPACE],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn foreign_or_unknown_resource_pins_are_refused_without_a_task() {
    const OTHER: &str = "workspace-routine-other";
    let (directory, store) = fixture_with(RESOURCE_SCHEMA, None, RESOURCE_BINDINGS);
    create_other_workspace(&store, OTHER);
    create_resource(&store, OTHER, "resource-foreign");

    let foreign = serde_json::json!({
        "workspace_id": WORKSPACE,
        "resource_id": "resource-foreign",
        "revision_id": "revision-resource-foreign"
    });
    let error = store
        .create_task(routine_commit_with(
            "task-routine-foreign",
            "routine-run-foreign",
            1,
            serde_json::json!({"ref": foreign}),
            "binding-routine",
            vec![serde_json::json!({
                "workspace_id": WORKSPACE,
                "resource_id": "resource-foreign",
                "revision_id": "revision-resource-foreign"
            })],
        ))
        .expect_err("a pin that exists only in another Workspace is refused");
    assert!(matches!(error, StoreError::NotFound));

    create_resource(&store, WORKSPACE, "resource-routine");
    let stale = serde_json::json!({
        "workspace_id": WORKSPACE,
        "resource_id": "resource-routine",
        "revision_id": "revision-missing"
    });
    let error = store
        .create_task(routine_commit_with(
            "task-routine-stale",
            "routine-run-stale",
            1,
            serde_json::json!({"ref": stale}),
            "binding-routine",
            vec![serde_json::json!({
                "workspace_id": WORKSPACE,
                "resource_id": "resource-routine",
                "revision_id": "revision-missing"
            })],
        ))
        .expect_err("an unknown Resource revision is refused");
    assert!(matches!(error, StoreError::NotFound));

    let mislabelled = serde_json::json!({
        "workspace_id": OTHER,
        "resource_id": "resource-foreign",
        "revision_id": "revision-resource-foreign"
    });
    let error = store
        .create_task(routine_commit_with(
            "task-routine-mislabelled",
            "routine-run-mislabelled",
            1,
            serde_json::json!({"ref": mislabelled}),
            "binding-routine",
            vec![serde_json::json!({
                "workspace_id": OTHER,
                "resource_id": "resource-foreign",
                "revision_id": "revision-resource-foreign"
            })],
        ))
        .expect_err("a pin labelled for another Workspace is refused");
    assert!(matches!(error, StoreError::Invalid(_)));

    let connection = Connection::open(state_database(&directory)).unwrap();
    let (tasks, receipts): (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1), (SELECT COUNT(*) FROM request_dedup WHERE request_id LIKE 'routine-run-%')",
            params![WORKSPACE],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((tasks, receipts), (0, 0));
}

#[test]
fn late_failures_roll_back_the_task_spec_event_and_receipt() {
    let (directory, store) = fixture("{}");
    let connection = Connection::open(state_database(&directory)).unwrap();

    fn counts(connection: &Connection) -> (i64, i64, i64, i64, i64) {
        connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM tasks WHERE workspace_id = 'workspace-routine-admission'),
                   (SELECT COUNT(*) FROM task_spec_revisions WHERE workspace_id = 'workspace-routine-admission'),
                   (SELECT COUNT(*) FROM domain_events WHERE workspace_id = 'workspace-routine-admission' AND type = 'task.created.v1'),
                   (SELECT COUNT(*) FROM request_dedup WHERE principal_id = 'owner-local' AND request_id LIKE 'routine-run-atomic%'),
                   COALESCE((SELECT MAX(last_sequence) FROM workspace_origin_sequences WHERE workspace_id = 'workspace-routine-admission' AND origin_runtime_id = 'runtime-routine-admission'), 0)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap()
    }
    assert_eq!(counts(&connection), (0, 0, 0, 0, 0));

    // Fail at the domain-event insert: the Task and TaskSpec rows are already
    // written inside the transaction and must roll back with it.
    connection
        .execute_batch(
            "CREATE TRIGGER fail_routine_task_event BEFORE INSERT ON domain_events
             WHEN NEW.event_id = 'ev-routine-task-routine-atomic'
             BEGIN SELECT RAISE(ABORT, 'injected event failure'); END;",
        )
        .unwrap();
    let first = routine_commit(
        "task-routine-atomic",
        "routine-run-atomic-1",
        1,
        serde_json::json!({}),
    );
    let error = store
        .create_task(first.clone())
        .expect_err("the injected event failure aborts the commit");
    assert!(matches!(error, StoreError::Database(_)));
    assert_eq!(counts(&connection), (0, 0, 0, 0, 0));

    // The failed attempt does not poison the request key: after the fault is
    // removed, the same commit succeeds and records one complete aggregate.
    connection
        .execute_batch("DROP TRIGGER fail_routine_task_event;")
        .unwrap();
    let committed = store
        .create_task(first)
        .expect("the same request commits after the fault is removed");
    assert_eq!(committed.view.task.task_id, "task-routine-atomic");
    assert_eq!(counts(&connection), (1, 1, 1, 1, 1));

    // Fail at the receipt insert, after the Task, TaskSpec, event, and origin
    // sequence were written. The whole transaction must still roll back.
    connection
        .execute_batch(
            "CREATE TRIGGER fail_routine_task_receipt BEFORE INSERT ON request_dedup
             WHEN NEW.request_id = 'routine-run-atomic-2'
             BEGIN SELECT RAISE(ABORT, 'injected receipt failure'); END;",
        )
        .unwrap();
    let second = routine_commit(
        "task-routine-atomic-2",
        "routine-run-atomic-2",
        1,
        serde_json::json!({}),
    );
    let error = store
        .create_task(second.clone())
        .expect_err("the injected receipt failure aborts the commit");
    assert!(matches!(error, StoreError::Database(_)));
    assert_eq!(counts(&connection), (1, 1, 1, 1, 1));

    connection
        .execute_batch("DROP TRIGGER fail_routine_task_receipt;")
        .unwrap();
    let committed = store
        .create_task(second)
        .expect("the second request commits after the receipt fault is removed");
    assert_eq!(committed.view.task.task_id, "task-routine-atomic-2");
    assert_eq!(counts(&connection), (2, 2, 2, 2, 2));
}
