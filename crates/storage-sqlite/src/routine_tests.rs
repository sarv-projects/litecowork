use super::*;
use domain_responsibility::*;
use serde_json::json;

fn adapter(store: &SqliteWorkspaceStore, event_id: &str, minute: u8) -> SqliteRoutineStore {
    let event = context(event_id, minute);
    SqliteRoutineStore::new(store.clone(), RoutineEventContext {
        event_id: event.event_id,
        origin_runtime_id: event.origin_runtime_id,
        hlc_timestamp: event.hlc_timestamp,
        correlation_id: event.correlation_id,
        causation_id: event.causation_id,
        recorded_at: event.recorded_at,
    }).expect("Routine adapter")
}

fn scope(request_id: &str) -> RoutineOwnerScope {
    RoutineOwnerScope {
        principal_id: "owner-local".into(),
        workspace_id: "workspace-routine".into(),
        request_id: request_id.into(),
    }
}

fn definition(objective: &str) -> RoutineRevisionInput {
    RoutineRevisionInput {
        objective_template: objective.into(),
        instructions: "Review the supplied project and return findings.".into(),
        input_schema: Default::default(),
        constraints: vec!["Do not execute the workflow automatically".into()],
        non_goals: vec![],
        required_outputs: vec![std::collections::BTreeMap::from([("kind".into(), json!("REPORT"))])],
        acceptance_criteria: vec![],
        approvals_required: vec![],
        input_bindings: vec![],
        required_capabilities: vec![],
        preferred_agent_binding_id: None,
        placement_preference: PlacementPreference::Class(PlacementClass::Auto),
        budget_ceiling: None,
        verification_policy: Default::default(),
    }
}

fn execute(store: &SqliteWorkspaceStore, request_id: &str, command: RoutineCommand, minute: u8) -> Result<Routine, RoutineError> {
    RoutineService::new(adapter(store, &format!("event-{request_id}-{minute}"), minute))
        .execute(&scope(request_id), command)
}

#[test]
fn routine_create_revise_archive_is_durable_revisioned_and_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-routine");

    let create = RoutineCommand::Create {
        routine_id: "routine-v1".into(),
        name: "Weekly review".into(),
        revision: definition("Review project changes"),
    };
    let created = execute(&store, "routine-create", create.clone(), 1).unwrap();
    assert_eq!(execute(&store, "routine-create", create, 2).unwrap(), created);

    let revised = execute(&store, "routine-revise", RoutineCommand::Revise {
        routine_id: "routine-v1".into(),
        expected_version: 1,
        revision: definition("Review project changes and open decisions"),
    }, 3).unwrap();
    assert_eq!((revised.current_revision, revised.version), (2, 2));

    let archived = execute(&store, "routine-archive", RoutineCommand::Archive {
        routine_id: "routine-v1".into(),
        expected_version: 2,
    }, 4).unwrap();
    assert_eq!((archived.status, archived.version), (RoutineStatus::Archived, 3));

    let current = adapter(&store, "routine-read-current", 5)
        .get("owner-local", "workspace-routine", "routine-v1").unwrap().unwrap();
    assert_eq!(current.0, archived);
    assert_eq!(current.1.definition.objective_template, "Review project changes and open decisions");
    let previous = adapter(&store, "routine-read-v1", 5)
        .get_revision("owner-local", "workspace-routine", "routine-v1", 1).unwrap().unwrap();
    assert_eq!(previous.definition.objective_template, "Review project changes");
}

#[test]
fn routine_revision_rows_reject_update_and_delete() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-routine");
    execute(&store, "routine-create", RoutineCommand::Create {
        routine_id: "routine-immutable".into(),
        name: "Review".into(),
        revision: definition("Review project"),
    }, 1).unwrap();

    let connection = Connection::open(state_database(&directory)).unwrap();
    let update = connection.execute(
        "UPDATE routine_revisions SET instructions = 'rewritten' WHERE workspace_id = ?1 AND routine_id = ?2 AND revision = 1",
        params!["workspace-routine", "routine-immutable"],
    );
    assert!(update.is_err());
    let delete = connection.execute(
        "DELETE FROM routine_revisions WHERE workspace_id = ?1 AND routine_id = ?2 AND revision = 1",
        params!["workspace-routine", "routine-immutable"],
    );
    assert!(delete.is_err());
}

#[test]
fn routine_rows_are_workspace_scoped() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-routine");
    create_workspace(&store, "workspace-other");
    execute(&store, "routine-create", RoutineCommand::Create {
        routine_id: "routine-scoped".into(),
        name: "Review".into(),
        revision: definition("Review project"),
    }, 1).unwrap();
    assert!(adapter(&store, "routine-other-workspace", 2)
        .get("owner-local", "workspace-other", "routine-scoped").unwrap().is_none());
}
