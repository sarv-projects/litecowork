use super::*;
use domain_responsibility::*;

fn coworker_definition(name: &str) -> CoworkerDefinition {
    CoworkerDefinition {
        name: name.into(),
        avatar_ref: None,
        role_description: String::new(),
        default_lead_agent_binding_id: None,
        delegation_strategy: DelegationStrategy::NativeDefault,
        enabled_delegation_profile_ids: vec![],
        delegation_budget_policy: None,
        lead_failover_policy: None,
        interaction_policy: CoworkerInteractionPolicy {
            read_only_work: InteractionDefault::StandardTrustPolicy,
            draft_creation: InteractionDefault::StandardTrustPolicy,
            external_mutation: InteractionDefault::RequireOwnerApproval,
            destructive_action: InteractionDefault::HandoffToOwner,
            financial_commitment: InteractionDefault::HandoffToOwner,
        },
        context_policy: CoworkerContextPolicy {
            allowed_context_kinds: vec![],
            max_retrieved_items: 0,
            retain_task_summaries: false,
            require_user_confirmation_for_memory: true,
        },
        notification_policy: NotificationPolicy {
            blockers: BinaryNotification::Always,
            completion: CompletionNotification::OnSuccess,
            failures: BinaryNotification::Always,
        },
    }
}
fn coworker_adapter(store: &SqliteWorkspaceStore, id: &str, minute: u8) -> SqliteCoworkerStore {
    let e = context(id, minute);
    SqliteCoworkerStore::new(
        store.clone(),
        CoworkerEventContext {
            event_id: e.event_id,
            origin_runtime_id: e.origin_runtime_id,
            hlc_timestamp: e.hlc_timestamp,
            correlation_id: e.correlation_id,
            causation_id: e.causation_id,
            recorded_at: e.recorded_at,
        },
    )
    .unwrap()
}
fn coworker_scope(request: &str) -> OwnerCommandScope {
    OwnerCommandScope {
        principal_id: "owner-local".into(),
        workspace_id: "workspace-coworker".into(),
        request_id: request.into(),
    }
}
fn coworker_command(
    store: &SqliteWorkspaceStore,
    request: &str,
    command: ResponsibilityCommand,
    minute: u8,
) -> Result<CommittedResponsibility, DomainError> {
    ResponsibilityService::new(coworker_adapter(
        store,
        &format!("event-{request}-{minute}"),
        minute,
    ))
    .execute(&coworker_scope(request), command)
}

fn automation_definition(routine_id: &str) -> AutomationDefinition {
    AutomationDefinition {
        routine_id: routine_id.into(),
        routine_revision: 1,
        triggers: vec![TriggerSpec {
            trigger_id: "manual".into(),
            placement: TriggerPlacement::Auto,
            runtime_id: None,
            trigger: TriggerDefinition::Manual,
        }],
        execution_policy: AutomationExecutionPolicy {
            placement_preference: PlacementPreference::Class(PlacementClass::Auto),
            max_concurrent_occurrences: 1,
            overlap_policy: OverlapPolicy::Skip,
            retry_policy: RetryPolicy {
                max_attempts: 1,
                initial_backoff_ms: 100,
                max_backoff_ms: 100,
                multiplier: 1.0,
                jitter: false,
                retryable_error_codes: vec![],
            },
            budget_ceiling: None,
            notification_policy: AutomationNotification::OnFailure,
            wake_policy: WakePolicy::Never,
        },
        coworker_ref: None,
    }
}

fn seed_routine(directory: &tempfile::TempDir, routine_id: &str) {
    let connection = Connection::open(state_database(directory)).unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    tx.execute("INSERT INTO routines(routine_id, workspace_id, name, current_revision, status, created_at, updated_at, version) VALUES (?1, 'workspace-coworker', 'Routine', 1, 'ACTIVE', '2026-10-06T10:00:00Z', '2026-10-06T10:00:00Z', 1)", [routine_id]).unwrap();
    tx.execute("INSERT INTO routine_revisions(workspace_id, routine_id, revision, objective_template, instructions, input_schema_json, constraints_json, non_goals_json, required_outputs_json, acceptance_criteria_json, approvals_required_json, input_bindings_json, required_capabilities_json, preferred_agent_binding_id, placement_preference_json, budget_ceiling_json, verification_policy_json, authored_by_json, created_at) VALUES ('workspace-coworker', ?1, 1, 'Review project', 'Review the project', '{}', '[]', '[]', '[]', '[]', '[]', '[]', '[]', NULL, '{}', NULL, '{}', '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', '2026-10-06T10:00:00Z')", [routine_id]).unwrap();
    tx.commit().unwrap();
}

#[test]
fn coworker_sqlite_revisions_events_replay_and_restart() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-coworker");
    let create = ResponsibilityCommand::CreateCoworker {
        coworker_id: "coworker-1".into(),
        definition: coworker_definition("Coworker"),
    };
    let first = coworker_command(&store, "create-coworker", create.clone(), 1).unwrap();
    assert_eq!(
        coworker_command(&store, "create-coworker", create, 2).unwrap(),
        first
    );
    assert_eq!(
        coworker_command(
            &store,
            "create-coworker",
            ResponsibilityCommand::CreateCoworker {
                coworker_id: "coworker-1".into(),
                definition: coworker_definition("Changed")
            },
            2
        ),
        Err(DomainError::IdempotencyConflict)
    );
    coworker_command(
        &store,
        "revise-coworker",
        ResponsibilityCommand::ReviseCoworker {
            coworker_id: "coworker-1".into(),
            expected_version: 1,
            definition: coworker_definition("Research"),
        },
        3,
    )
    .unwrap();
    assert_eq!(
        coworker_command(
            &store,
            "stale",
            ResponsibilityCommand::ReviseCoworker {
                coworker_id: "coworker-1".into(),
                expected_version: 1,
                definition: coworker_definition("Stale")
            },
            4
        ),
        Err(DomainError::VersionConflict)
    );
    coworker_command(
        &store,
        "pause-coworker",
        ResponsibilityCommand::SetCoworkerStatus {
            coworker_id: "coworker-1".into(),
            expected_version: 2,
            status: CoworkerStatus::Paused,
        },
        4,
    )
    .unwrap();
    let adapter = coworker_adapter(&store, "read", 5);
    let head = adapter
        .get("owner-local", "workspace-coworker", "coworker-1")
        .unwrap()
        .unwrap();
    assert_eq!(
        (head.0.version, head.0.current_revision, head.0.status),
        (3, 2, CoworkerStatus::Paused)
    );
    assert_eq!(head.1.definition.name, "Research");
    assert_eq!(
        adapter
            .list(
                "owner-local",
                "workspace-coworker",
                Some(CoworkerStatus::Paused),
                None,
                1
            )
            .unwrap()
            .items,
        vec![head.clone()]
    );
    assert_eq!(
        adapter.get("other-owner", "workspace-coworker", "coworker-1"),
        Err(DomainError::Unauthorized)
    );
    let events = store.read_workspace_events("workspace-coworker").unwrap();
    let coworker_events: Vec<_> = events
        .iter()
        .filter(|e| e.entity_type == "Coworker")
        .collect();
    assert_eq!(coworker_events.len(), 3);
    let snapshot = store
        .inner
        .blobs
        .get(
            "workspace-coworker",
            BlobPurpose::AggregateState,
            &coworker_events[2].aggregate_state_ref.blob,
        )
        .unwrap();
    let value: Value = serde_json::from_slice(&snapshot).unwrap();
    assert_eq!(value["coworker"]["current_revision"], 2);
    assert_eq!(value["revision"]["name"], "Research");
    drop(adapter);
    drop(store);
    let reopened = test_store(&directory, Duration::from_secs(1));
    assert_eq!(
        coworker_adapter(&reopened, "read-again", 6)
            .get("owner-local", "workspace-coworker", "coworker-1")
            .unwrap()
            .unwrap(),
        head
    );
    let db = Connection::open(state_database(&directory)).unwrap();
    let old_name: String = db
        .query_row(
            "SELECT name FROM coworker_revisions WHERE coworker_id = 'coworker-1' AND revision = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(old_name, "Coworker");
    assert!(db.execute("UPDATE coworker_revisions SET name = 'Overwrite' WHERE coworker_id = 'coworker-1' AND revision = 1", []).is_err());
}

#[test]
fn coworker_archive_and_replay_recheck_authoritative_owner_and_primary() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-coworker");
    let command = ResponsibilityCommand::CreateCoworker {
        coworker_id: "coworker-1".into(),
        definition: coworker_definition("Coworker"),
    };
    coworker_command(&store, "create-coworker", command.clone(), 1).unwrap();
    let db = Connection::open(state_database(&directory)).unwrap();
    // Fixtures alter authoritative state through a second connection to exercise
    // current-state guards; these are not proposed product mutation paths.
    db.execute("UPDATE workspaces SET primary_coworker_id = 'coworker-1' WHERE workspace_id = 'workspace-coworker'", []).unwrap();
    let archive = ResponsibilityCommand::SetCoworkerStatus {
        coworker_id: "coworker-1".into(),
        expected_version: 1,
        status: CoworkerStatus::Archived,
    };
    assert_eq!(
        coworker_command(&store, "archive", archive.clone(), 2),
        Err(DomainError::ArchiveBlocked)
    );
    db.execute("UPDATE workspaces SET primary_coworker_id = NULL WHERE workspace_id = 'workspace-coworker'", []).unwrap();
    coworker_command(&store, "archive", archive, 3).unwrap();
    assert_eq!(
        coworker_command(
            &store,
            "unarchive",
            ResponsibilityCommand::SetCoworkerStatus {
                coworker_id: "coworker-1".into(),
                expected_version: 2,
                status: CoworkerStatus::Active
            },
            4
        ),
        Err(DomainError::CoworkerArchived)
    );
    db.execute("UPDATE workspaces SET owner_principal_id = 'new-owner' WHERE workspace_id = 'workspace-coworker'", []).unwrap();
    assert_eq!(
        coworker_command(&store, "create-coworker", command, 5),
        Err(DomainError::Unauthorized)
    );
}

#[test]
fn archived_workspace_keeps_owner_reads_but_rejects_responsibility_mutations() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-coworker");
    coworker_command(
        &store,
        "create-coworker",
        ResponsibilityCommand::CreateCoworker {
            coworker_id: "coworker-1".into(),
            definition: coworker_definition("Coworker"),
        },
        1,
    )
    .unwrap();
    let db = Connection::open(state_database(&directory)).unwrap();
    db.execute("UPDATE workspaces SET status = 'ARCHIVED', updated_at = '2026-10-06T10:02:00Z', version = version + 1 WHERE workspace_id = 'workspace-coworker'", []).unwrap();
    let adapter = coworker_adapter(&store, "archived-read", 3);
    assert!(
        adapter
            .get("owner-local", "workspace-coworker", "coworker-1")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        coworker_command(
            &store,
            "archived-revision",
            ResponsibilityCommand::ReviseCoworker {
                coworker_id: "coworker-1".into(),
                expected_version: 1,
                definition: coworker_definition("Changed")
            },
            4
        ),
        Err(DomainError::WorkspaceArchived)
    );
}

#[test]
fn automation_definitions_are_persisted_but_trigger_activation_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-coworker");
    seed_routine(&directory, "routine-1");
    let definition = automation_definition("routine-1");
    let create = ResponsibilityCommand::CreateAutomation {
        automation_id: "automation-1".into(),
        name: "Daily review".into(),
        definition: definition.clone(),
    };
    let first = coworker_command(&store, "create-automation", create.clone(), 1).unwrap();
    assert_eq!(
        coworker_command(&store, "create-automation", create, 2).unwrap(),
        first
    );
    coworker_command(
        &store,
        "revise-automation",
        ResponsibilityCommand::ReviseAutomation {
            automation_id: "automation-1".into(),
            expected_version: 1,
            name: "Morning review".into(),
            definition,
        },
        3,
    )
    .unwrap();
    assert_eq!(
        coworker_command(
            &store,
            "enable-automation",
            ResponsibilityCommand::SetAutomationStatus {
                automation_id: "automation-1".into(),
                expected_version: 2,
                status: AutomationStatus::Enabled
            },
            4
        ),
        Err(DomainError::ReconciliationRequired)
    );
    let db = Connection::open(state_database(&directory)).unwrap();
    let (name, status, revision, count): (String, String, i64, i64) = db.query_row("SELECT a.name, a.status, a.current_revision, (SELECT COUNT(*) FROM automation_revisions r WHERE r.automation_id = a.automation_id) FROM automations a WHERE a.automation_id = 'automation-1'", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap();
    assert_eq!(
        (name.as_str(), status.as_str(), revision, count),
        ("Morning review", "PAUSED", 2, 2)
    );
    let events = store.read_workspace_events("workspace-coworker").unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.entity_type == "Automation")
            .count(),
        2
    );
}

#[test]
fn automation_cursor_reads_and_revision_updates_are_workspace_scoped() {
    // A reduced fixture permits same-ID records in separate Workspaces so this test
    // protects the tenant predicate even if the production schema later changes from
    // globally unique Automation IDs to Workspace-local IDs.
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(
        "CREATE TABLE automation_cursors(
            workspace_id TEXT NOT NULL,
            automation_id TEXT NOT NULL,
            trigger_id TEXT NOT NULL,
            trigger_host_runtime_id TEXT NOT NULL,
            host_epoch INTEGER NOT NULL,
            version INTEGER NOT NULL,
            active_automation_revision INTEGER NOT NULL,
            cursor_digest TEXT NOT NULL,
            last_checked_at TEXT NOT NULL,
            PRIMARY KEY(workspace_id, automation_id, trigger_id)
         );
         INSERT INTO automation_cursors VALUES
           ('workspace-a','automation-shared','manual','runtime-a',3,4,2,'digest-a','2026-10-09T00:00:00Z'),
           ('workspace-b','automation-shared','manual','runtime-b',8,9,5,'digest-b','2026-10-09T00:00:00Z');",
    ).unwrap();

    let before = super::super::coworkers::load_automation_cursor(
        &connection,
        "workspace-a",
        "automation-shared",
        "manual",
    )
    .unwrap();
    assert_eq!(before, Some(("runtime-a".into(), 3, 4)));

    super::super::coworkers::update_automation_cursor_revision(
        &connection,
        "workspace-a",
        "automation-shared",
        "manual",
        4,
        3,
        "digest-a-next",
        "2026-10-09T00:01:00Z",
    )
    .unwrap();

    let rows: Vec<(String, i64, String, i64)> = {
        let mut statement = connection.prepare(
            "SELECT workspace_id, active_automation_revision, cursor_digest, version
             FROM automation_cursors WHERE automation_id = 'automation-shared' AND trigger_id = 'manual'
             ORDER BY workspace_id",
        ).unwrap();
        statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(
        rows,
        vec![
            ("workspace-a".into(), 3, "digest-a-next".into(), 5),
            ("workspace-b".into(), 5, "digest-b".into(), 9),
        ]
    );
}
