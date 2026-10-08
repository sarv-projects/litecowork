use super::*;
use domain_responsibility::*;

fn adapter(store: &SqliteWorkspaceStore, event_id: &str, minute: u8) -> SqliteGoalStore {
    let event = context(event_id, minute);
    SqliteGoalStore::new(store.clone(), GoalEventContext {
        event_id: event.event_id,
        origin_runtime_id: event.origin_runtime_id,
        hlc_timestamp: event.hlc_timestamp,
        correlation_id: event.correlation_id,
        causation_id: event.causation_id,
        recorded_at: event.recorded_at,
    }).expect("Goal adapter")
}

fn scope(request_id: &str) -> GoalOwnerScope {
    GoalOwnerScope {
        principal_id: "owner-local".into(),
        workspace_id: "workspace-goal".into(),
        request_id: request_id.into(),
    }
}

fn definition(objective: &str) -> GoalRevisionInput {
    GoalRevisionInput {
        objective: objective.into(),
        success_criteria: vec!["Owner accepts a verified local release".into()],
        constraints: vec!["No autonomous execution".into()],
        horizon: Some("V1".into()),
        related_task_ids: vec![],
        related_routine_refs: vec![],
        related_artifact_refs: vec![],
    }
}

fn seed_routine(directory: &tempfile::TempDir, workspace_id: &str, routine_id: &str) {
    let connection = Connection::open(state_database(directory)).unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    tx.execute("INSERT INTO routines(routine_id, workspace_id, name, current_revision, status, created_at, updated_at, version) VALUES (?1, ?2, 'Routine', 1, 'ACTIVE', '2026-10-06T10:00:00Z', '2026-10-06T10:00:00Z', 1)", params![routine_id, workspace_id]).unwrap();
    tx.execute("INSERT INTO routine_revisions(workspace_id, routine_id, revision, objective_template, instructions, input_schema_json, constraints_json, non_goals_json, required_outputs_json, acceptance_criteria_json, approvals_required_json, input_bindings_json, required_capabilities_json, preferred_agent_binding_id, placement_preference_json, budget_ceiling_json, verification_policy_json, authored_by_json, created_at) VALUES (?1, ?2, 1, 'Review project', 'Review the project', '{}', '[]', '[]', '[]', '[]', '[]', '[]', '[]', NULL, '{}', NULL, '{}', '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', '2026-10-06T10:00:00Z')", params![workspace_id, routine_id]).unwrap();
    tx.commit().unwrap();
}

fn execute(
    store: &SqliteWorkspaceStore,
    request_id: &str,
    command: GoalCommand,
    minute: u8,
) -> Result<Goal, GoalError> {
    GoalService::new(adapter(store, &format!("event-{request_id}-{minute}"), minute))
        .execute(&scope(request_id), command)
}

#[test]
fn goal_mutations_are_durable_revisioned_idempotent_and_event_backed() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-goal");

    let create = GoalCommand::Create {
        goal_id: "goal-v1".into(),
        coworker_id: None,
        revision: definition("Ship LiteCowork V1"),
    };
    let created = execute(&store, "goal-create", create.clone(), 1).unwrap();
    assert_eq!(execute(&store, "goal-create", create, 2).unwrap(), created);
    assert_eq!(
        execute(&store, "goal-create", GoalCommand::Create {
            goal_id: "goal-v1".into(), coworker_id: None, revision: definition("Changed")
        }, 2),
        Err(GoalError::IdempotencyConflict)
    );

    let revised = execute(&store, "goal-revise", GoalCommand::Revise {
        goal_id: "goal-v1".into(), expected_version: 1, revision: definition("Ship LiteCowork V1 with verified local Tasks"),
    }, 3).unwrap();
    assert_eq!((revised.version, revised.current_revision), (2, 2));

    let paused = execute(&store, "goal-pause", GoalCommand::SetStatus {
        goal_id: "goal-v1".into(), expected_version: 2, status: GoalStatus::Paused,
    }, 4).unwrap();
    assert_eq!((paused.version, paused.current_revision, paused.status), (3, 2, GoalStatus::Paused));

    let read = adapter(&store, "goal-read", 5).get("owner-local", "workspace-goal", "goal-v1").unwrap().unwrap();
    assert_eq!(read.0, paused);
    assert_eq!(read.1.definition.objective, "Ship LiteCowork V1 with verified local Tasks");
    let old = adapter(&store, "goal-rev1", 5).get_revision("owner-local", "workspace-goal", "goal-v1", 1).unwrap().unwrap();
    assert_eq!(old.definition.objective, "Ship LiteCowork V1");
    assert_eq!(adapter(&store, "goal-list", 5).list("owner-local", "workspace-goal", None, Some(GoalStatus::Paused), None, 10).unwrap().items, vec![read.clone()]);

    let events = store.read_workspace_events("workspace-goal").unwrap();
    let goal_events: Vec<_> = events.iter().filter(|event| event.entity_type == "Goal").collect();
    assert_eq!(goal_events.len(), 3);
    let snapshot = store.inner.blobs.get("workspace-goal", BlobPurpose::AggregateState, &goal_events[2].aggregate_state_ref.blob).unwrap();
    let value: Value = serde_json::from_slice(&snapshot).unwrap();
    assert_eq!(value["goal"]["status"], "PAUSED");
    assert_eq!(value["revision"]["revision"], 2);

    let connection = Connection::open(state_database(&directory)).unwrap();
    assert!(connection.execute("UPDATE goal_revisions SET objective = 'mutated' WHERE goal_id = 'goal-v1' AND revision = 1", []).is_err());

    drop(store);
    let reopened = test_store(&directory, Duration::from_secs(1));
    assert_eq!(adapter(&reopened, "goal-reopened", 6).get("owner-local", "workspace-goal", "goal-v1").unwrap(), Some(read));
}

#[test]
fn goal_reads_and_idempotent_replay_recheck_current_workspace_owner() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-goal");
    let command = GoalCommand::Create { goal_id: "goal-owner".into(), coworker_id: None, revision: definition("Owner scope") };
    execute(&store, "owner-create", command.clone(), 1).unwrap();
    assert_eq!(adapter(&store, "foreign-read", 2).get("other-owner", "workspace-goal", "goal-owner"), Err(GoalError::Unauthorized));

    let db = Connection::open(state_database(&directory)).unwrap();
    db.execute("UPDATE workspaces SET owner_principal_id = 'new-owner' WHERE workspace_id = 'workspace-goal'", []).unwrap();
    assert_eq!(execute(&store, "owner-create", command, 3), Err(GoalError::Unauthorized));
}

#[test]
fn goal_routine_links_are_pinned_and_reject_cross_workspace_references() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-goal");
    let other_workspace_event = context("event-create-other", 1);
    service(store.clone()).create(CreateWorkspace {
        workspace_id: "workspace-other".into(),
        name: "Other workspace".into(),
        owner_principal_id: "owner-local".into(),
        event: other_workspace_event,
    }).unwrap();
    seed_routine(&directory, "workspace-goal", "routine-local");
    seed_routine(&directory, "workspace-other", "routine-foreign");

    let mut local_definition = definition("Use the local routine");
    local_definition.related_routine_refs.push(RoutineRevisionRef { routine_id: "routine-local".into(), revision: 1 });
    execute(&store, "goal-local-routine", GoalCommand::Create {
        goal_id: "goal-local-routine".into(), coworker_id: None, revision: local_definition,
    }, 2).unwrap();
    let loaded = adapter(&store, "goal-local-read", 3).get("owner-local", "workspace-goal", "goal-local-routine").unwrap().unwrap();
    assert_eq!(loaded.1.definition.related_routine_refs, vec![RoutineRevisionRef { routine_id: "routine-local".into(), revision: 1 }]);

    let mut foreign_definition = definition("Do not link across workspaces");
    foreign_definition.related_routine_refs.push(RoutineRevisionRef { routine_id: "routine-foreign".into(), revision: 1 });
    assert_eq!(execute(&store, "goal-foreign-routine", GoalCommand::Create {
        goal_id: "goal-foreign-routine".into(), coworker_id: None, revision: foreign_definition,
    }, 4), Err(GoalError::ReferenceUnavailable));
}

#[test]
fn archived_workspace_keeps_owner_reads_but_rejects_goal_mutations() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-goal");
    execute(&store, "archive-create", GoalCommand::Create { goal_id: "goal-archived-ws".into(), coworker_id: None, revision: definition("Archived workspace") }, 1).unwrap();
    let db = Connection::open(state_database(&directory)).unwrap();
    db.execute("UPDATE workspaces SET status = 'ARCHIVED', updated_at = '2026-10-06T10:02:00Z', version = version + 1 WHERE workspace_id = 'workspace-goal'", []).unwrap();
    assert!(adapter(&store, "archived-read", 3).get("owner-local", "workspace-goal", "goal-archived-ws").unwrap().is_some());
    assert_eq!(execute(&store, "new-goal", GoalCommand::Create { goal_id: "goal-rejected".into(), coworker_id: None, revision: definition("Must not write") }, 3), Err(GoalError::WorkspaceArchived));
}
