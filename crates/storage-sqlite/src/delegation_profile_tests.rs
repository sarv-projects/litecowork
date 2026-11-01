use super::*;
use domain_responsibility::{
    DelegatedEnvironmentPolicy, DelegatedWorkerPolicy, DelegationLatencyClass,
    DelegationProfileCommand, DelegationProfileCommandScope, DelegationProfileError,
    DelegationProfileRevisionInput, DelegationProfileService, DelegationProfileStatus,
    NativeDelegationPolicy, OptimizationPreference,
};
use serde_json::json;

const AT: &str = "2026-10-09T10:00:00Z";

fn event_context(event_id: &str, minute: u8) -> DelegationProfileEventContext {
    let timestamp = format!("2026-10-09T10:{minute:02}:00Z");
    DelegationProfileEventContext {
        event_id: event_id.into(),
        origin_runtime_id: "runtime-local".into(),
        hlc_timestamp: timestamp.clone(),
        correlation_id: "delegation-profile-test".into(),
        causation_id: None,
        recorded_at: timestamp,
    }
}

fn profile_scope(request_id: &str) -> DelegationProfileCommandScope {
    DelegationProfileCommandScope {
        principal_id: "owner-local".into(),
        workspace_id: "workspace-profile".into(),
        request_id: request_id.into(),
    }
}

fn profile_input(name: &str, routing: &str) -> DelegationProfileRevisionInput {
    DelegationProfileRevisionInput {
        name: name.into(),
        routing_description: routing.into(),
        instructions: None,
        session_options: json!({}),
        session_options_descriptor_digest: None,
        required_features: vec![],
        preferred_features: vec![],
        enforced_policy: DelegatedWorkerPolicy {
            capability_allowlist: vec![],
            maximum_effect_risk: "SAFE".into(),
            filesystem_write_scope: "WORKTREE_ONLY".into(),
            external_effects: "DENY".into(),
            secret_access: "NONE".into(),
        },
        optimization_preference: OptimizationPreference::Balanced,
        quality_floor: None,
        max_concurrency: 1,
        max_host_delegation_depth: 0,
        budget_ceiling: None,
        latency_class: DelegationLatencyClass::Standard,
        environment_policy: DelegatedEnvironmentPolicy {
            placement_preference: json!("AUTO"),
            isolation: "REQUIRED".into(),
            sharing_scope: "ATTEMPT_PRIVATE".into(),
        },
        native_delegation_policy: NativeDelegationPolicy::Inherit,
        warm_policy: json!({
            "host":"COLD", "native_session":"CLOSE_ON_SETTLE", "capability_hosts":"COLD",
            "browser_environment":"COLD", "local_model":"PROVIDER_DEFAULT", "ttl_ms":null,
            "max_memory_bytes":null, "max_idle_cost":null, "triggers":[]
        }),
    }
}

fn execute_profile_command(
    store: &SqliteWorkspaceStore,
    request_id: &str,
    event_id: &str,
    minute: u8,
    command: DelegationProfileCommand,
) -> Result<domain_responsibility::CommittedDelegationProfile, DelegationProfileError> {
    let adapter = SqliteDelegationProfileStore::new_with_context(
        store.clone(),
        event_context(event_id, minute),
    )
    .unwrap();
    DelegationProfileService::new(adapter).execute(&profile_scope(request_id), command)
}

fn seed_enabled_binding(directory: &tempfile::TempDir) {
    let mut connection = Connection::open(state_database(directory)).expect("fixture connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .unwrap();
    let tx = connection.transaction().unwrap();
    tx.execute(
        "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES ('agent-profile','fixture','Fixture Agent',?1)",
        [AT],
    ).unwrap();
    tx.execute(
        "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version) VALUES ('binding-profile','workspace-profile','agent-profile','{}','{}',1,0,?1,1)",
        [AT],
    ).unwrap();
    tx.commit().unwrap();
}

#[test]
fn delegation_profile_lifecycle_persists_revisions_and_never_enables_execution() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-profile");
    seed_enabled_binding(&directory);

    let created = execute_profile_command(
        &store,
        "create-fast",
        "event-create-fast",
        1,
        DelegationProfileCommand::Create {
            profile_id: "profile-fast".into(),
            agent_binding_id: "binding-profile".into(),
            revision: profile_input("Fast worker", "Small, bounded coding changes."),
        },
    )
    .expect("profile create persists");
    assert_eq!(created.profile.status, DelegationProfileStatus::Disabled);
    assert_eq!(created.profile.current_revision, 1);

    let revised = execute_profile_command(
        &store,
        "revise-fast",
        "event-revise-fast",
        2,
        DelegationProfileCommand::Revise {
            profile_id: "profile-fast".into(),
            expected_version: 1,
            revision: profile_input("Fast worker", "Quick bounded edits with relevant checks."),
        },
    )
    .expect("revision is appended");
    assert_eq!(revised.profile.version, 2);
    assert_eq!(revised.profile.current_revision, 2);
    assert_eq!(
        revised.revision.routing_description,
        "Quick bounded edits with relevant checks."
    );

    assert_eq!(
        execute_profile_command(
            &store,
            "stale-revision",
            "event-stale-revision",
            3,
            DelegationProfileCommand::Revise {
                profile_id: "profile-fast".into(),
                expected_version: 1,
                revision: profile_input("Fast worker", "Stale edit."),
            },
        )
        .unwrap_err(),
        DelegationProfileError::VersionConflict,
    );

    let duplicate = execute_profile_command(
        &store,
        "duplicate-fast",
        "event-duplicate-fast",
        4,
        DelegationProfileCommand::Duplicate {
            source_profile_id: "profile-fast".into(),
            expected_version: 2,
            profile_id: "profile-reviewer".into(),
            name: "Reviewer".into(),
        },
    )
    .expect("duplicate profile is created");
    assert_eq!(duplicate.profile.status, DelegationProfileStatus::Disabled);
    assert_eq!(duplicate.profile.current_revision, 1);
    assert_eq!(duplicate.profile.agent_binding_id, "binding-profile");
    assert_eq!(
        duplicate.revision.routing_description,
        "Quick bounded edits with relevant checks."
    );
    assert_eq!(duplicate.revision.revision, 1);

    assert_eq!(
        execute_profile_command(
            &store,
            "enable-fast",
            "event-enable-fast",
            5,
            DelegationProfileCommand::SetStatus {
                profile_id: "profile-fast".into(),
                expected_version: 2,
                status: DelegationProfileStatus::Enabled,
            },
        )
        .unwrap_err(),
        DelegationProfileError::EnablementUnavailable,
    );

    let adapter = SqliteDelegationProfileStore::new(store.clone());
    let listed = adapter
        .list(
            "owner-local",
            "workspace-profile",
            Some("binding-profile"),
            None,
            None,
            10,
        )
        .expect("owner can list Workspace profiles");
    assert_eq!(listed.items.len(), 2);
    assert!(listed.items.iter().all(|item| item["status"] == "DISABLED"));

    let archived = execute_profile_command(
        &store,
        "archive-fast",
        "event-archive-fast",
        6,
        DelegationProfileCommand::SetStatus {
            profile_id: "profile-fast".into(),
            expected_version: 2,
            status: DelegationProfileStatus::Archived,
        },
    )
    .expect("disabled profile can be archived");
    assert_eq!(archived.profile.status, DelegationProfileStatus::Archived);

    let events = store.read_workspace_events("workspace-profile").unwrap();
    let profile_events: Vec<_> = events
        .iter()
        .filter(|event| event.entity_type == "DelegationProfile")
        .collect();
    assert_eq!(
        profile_events.len(),
        4,
        "failed enable and stale edit append no event"
    );
    assert_eq!(
        profile_events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>(),
        [
            "delegation_profile.created.v1",
            "delegation_profile.revised.v1",
            "delegation_profile.created.v1",
            "delegation_profile.status.changed.v1",
        ]
    );
    assert!(
        profile_events
            .iter()
            .all(|event| event.origin_runtime_id == "runtime-local")
    );

    drop(adapter);
    drop(store);
    let reopened = test_store(&directory, Duration::from_secs(1));
    let restored = SqliteDelegationProfileStore::new(reopened)
        .get("owner-local", "workspace-profile", "profile-fast")
        .unwrap()
        .unwrap();
    assert_eq!(restored["status"], "ARCHIVED");
    assert_eq!(restored["current_revision"], 2);
    assert_eq!(
        restored["revision"]["routing_description"],
        "Quick bounded edits with relevant checks."
    );

    let connection = Connection::open(state_database(&directory)).expect("inspect revisions");
    let original_routing: String = connection
        .query_row(
            "SELECT routing_description FROM delegation_profile_revisions WHERE delegation_profile_id = 'profile-fast' AND revision = 1",
            [],
            |row| row.get(0),
        )
        .expect("original revision remains available");
    assert_eq!(original_routing, "Small, bounded coding changes.");
    assert!(connection
        .execute(
            "UPDATE delegation_profile_revisions SET routing_description = 'mutated' WHERE delegation_profile_id = 'profile-fast' AND revision = 1",
            [],
        )
        .is_err());
}

#[test]
fn performance_samples_are_owner_scoped_and_use_the_settlement_window() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-profile");
    seed_enabled_binding(&directory);
    execute_profile_command(
        &store,
        "performance-create",
        "performance-create-event",
        5,
        DelegationProfileCommand::Create {
            profile_id: "profile-performance".into(),
            agent_binding_id: "binding-profile".into(),
            revision: profile_input("Performance worker", "Bounded measured work."),
        },
    )
    .expect("profile commits disabled");

    let adapter = SqliteDelegationProfileStore::new(store);
    let empty = adapter
        .worker_performance_attempts(
            "owner-local",
            "workspace-profile",
            "profile-performance",
            "OTHER",
            "2026-10-09T10:05:00Z",
        )
        .expect("owner can query profile performance");
    assert!(empty.is_empty());
    assert!(matches!(
        adapter.worker_performance_attempts(
            "different-owner",
            "workspace-profile",
            "profile-performance",
            "OTHER",
            "2026-10-09T10:05:00Z",
        ),
        Err(DelegationProfileError::Unauthorized)
    ));

    // Seed legacy terminal rows directly: this projection must use settlement time,
    // not admission/creation time. Keep this fixture detached from admission behavior;
    // those guards are covered by their own transaction tests.
    let connection = Connection::open(directory.path().join("state/state.sqlite3"))
        .expect("fixture SQLite connection");
    connection
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             DROP TRIGGER IF EXISTS attempt_delegation_profile_admission_guard;
             DROP TRIGGER IF EXISTS attempt_environment_owner_guard;
             UPDATE delegation_profiles SET status='ENABLED' WHERE delegation_profile_id='profile-performance';",
        )
        .expect("prepare isolated projection fixture");
    seed_performance_projection_attempt(
        &connection,
        "settled-in-window",
        "2026-06-01T00:00:00Z",
        "2026-10-01T00:00:00Z",
        "RESEARCH",
    );
    seed_performance_projection_attempt(
        &connection,
        "settled-before-window",
        "2026-05-01T00:00:00Z",
        "2026-07-01T00:00:00Z",
        "RESEARCH",
    );
    seed_performance_projection_attempt(
        &connection,
        "settled-in-future",
        "2026-10-01T00:00:00Z",
        "2026-10-10T00:00:00Z",
        "RESEARCH",
    );
    seed_performance_projection_attempt(
        &connection,
        "wrong-pinned-category",
        "2026-10-01T00:00:00Z",
        "2026-10-08T00:00:00Z",
        "WRITING",
    );

    let in_window = adapter
        .worker_performance_attempts(
            "owner-local",
            "workspace-profile",
            "profile-performance",
            "RESEARCH",
            "2026-10-09T10:05:00Z",
        )
        .expect("query settlement-window samples");
    assert_eq!(in_window.len(), 1);
    assert_eq!(in_window[0].attempt_id, "attempt-settled-in-window");
    assert_eq!(in_window[0].task_category, "RESEARCH");

    connection
        .execute(
            "INSERT INTO usage_observations(usage_observation_id,workspace_id,task_id,attempt_id,source,metric,quantity,unit,currency,confidence,observed_at)
             VALUES ('usage-cost-one','workspace-profile','task-settled-in-window','attempt-settled-in-window','PROVIDER_REPORTED','COST','125','minor_unit','USD','EXACT','2026-10-01T00:00:00Z')",
            [],
        )
        .expect("insert one qualified cost observation");
    let one_cost = adapter
        .worker_performance_attempts(
            "owner-local",
            "workspace-profile",
            "profile-performance",
            "RESEARCH",
            "2026-10-09T10:05:00Z",
        )
        .expect("query one exact cost observation");
    assert_eq!(
        one_cost[0].cost.as_ref().map(|cost| cost.quantity.as_str()),
        Some("125")
    );
    assert_eq!(
        one_cost[0].cost.as_ref().map(|cost| cost.currency.as_str()),
        Some("USD")
    );

    connection
        .execute(
            "INSERT INTO usage_observations(usage_observation_id,workspace_id,task_id,attempt_id,source,metric,quantity,unit,currency,confidence,observed_at)
             VALUES ('usage-cost-two','workspace-profile','task-settled-in-window','attempt-settled-in-window','HOST_MEASURED','COST','130','minor_unit','USD','EXACT','2026-10-02T00:00:00Z')",
            [],
        )
        .expect("insert second cost observation");
    let ambiguous_cost = adapter
        .worker_performance_attempts(
            "owner-local",
            "workspace-profile",
            "profile-performance",
            "RESEARCH",
            "2026-10-09T10:05:00Z",
        )
        .expect("query ambiguous cost observations");
    assert!(ambiguous_cost[0].cost.is_none());
}

fn seed_performance_projection_attempt(
    connection: &Connection,
    suffix: &str,
    created_at: &str,
    settled_at: &str,
    category: &str,
) {
    let task_id = format!("task-{suffix}");
    let step_id = format!("step-{suffix}");
    let session_id = format!("session-{suffix}");
    let attempt_id = format!("attempt-{suffix}");
    let revision = "{\"mode\":\"DISABLED\",\"triggers\":[],\"fallback_agent_binding_ids\":[],\"max_lead_changes\":0}";
    connection
        .execute(
            "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,current_plan_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version)
             VALUES (?1,'workspace-profile',1,1,'READY','binding-profile','NORMAL','{}',?2,?2,1)",
            rusqlite::params![task_id, created_at],
        )
        .expect("insert performance task");
    connection
        .execute(
            "INSERT INTO task_spec_revisions(task_id,workspace_id,revision,objective,task_category,lead_failover_policy_json,placement_preference,authored_by_json,created_at)
             VALUES (?1,'workspace-profile',1,'bounded performance fixture',?2,?3,'LOCAL','{}',?4)",
            rusqlite::params![task_id, category, revision, created_at],
        )
        .expect("insert pinned task spec");
    connection
        .execute(
            "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,attempt_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at,version)
             VALUES (?1,'workspace-profile','ATTEMPT_EXECUTION',?2,1,?3,'binding-profile','endpoint-performance','runtime-performance','incarnation-performance','CLOSED',?4,1)",
            rusqlite::params![session_id, task_id, attempt_id, created_at],
        )
        .expect("insert pinned attempt session");
    connection
        .execute(
            "INSERT INTO plan_revisions(task_id,revision,task_spec_revision,produced_by_agent_session_id,produced_by_attempt_id,steps_json,created_at)
             VALUES (?1,1,1,?2,?3,'[]',?4)",
            rusqlite::params![task_id, session_id, attempt_id, created_at],
        )
        .expect("insert plan revision");
    connection
        .execute(
            "INSERT INTO steps(step_id,task_id,plan_revision,title,objective,status,created_at,updated_at,version)
             VALUES (?1,?2,1,'performance fixture','bounded work','COMPLETED',?3,?3,1)",
            rusqlite::params![step_id, task_id, created_at],
        )
        .expect("insert terminal step");
    connection
        .execute(
            "INSERT INTO attempts(attempt_id,task_id,step_id,parent_attempt_id,agent_binding_id,delegation_profile_id,delegation_profile_revision,agent_session_id,runtime_id,runtime_incarnation_id,environment_id,failover_class,status,started_at,settled_at,created_at,version)
             VALUES (?1,?2,?3,'parent-attempt','binding-profile','profile-performance',1,?4,'runtime-performance','incarnation-performance','environment-performance','SAFE_PORTABLE','COMPLETED',?5,?6,?5,1)",
            rusqlite::params![attempt_id, task_id, step_id, session_id, created_at, settled_at],
        )
        .expect("insert terminal child Attempt");
}

#[test]
fn delegation_profile_revision_history_is_owner_scoped_and_descending() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-profile");
    seed_enabled_binding(&directory);
    execute_profile_command(
        &store,
        "history-create",
        "history-event-create",
        1,
        DelegationProfileCommand::Create {
            profile_id: "profile-history".into(),
            agent_binding_id: "binding-profile".into(),
            revision: profile_input("History", "revision one"),
        },
    )
    .unwrap();
    for revision in 2_u32..=3 {
        execute_profile_command(
            &store,
            &format!("history-revise-{revision}"),
            &format!("history-event-revise-{revision}"),
            revision as u8,
            DelegationProfileCommand::Revise {
                profile_id: "profile-history".into(),
                expected_version: u64::from(revision - 1),
                revision: profile_input("History", &format!("revision {revision}")),
            },
        )
        .unwrap();
    }

    let adapter = SqliteDelegationProfileStore::new(store.clone());
    let first = adapter
        .list_revisions(
            "owner-local",
            "workspace-profile",
            "profile-history",
            None,
            2,
        )
        .expect("owner can read revisions");
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.items[0]["revision"], 3);
    assert_eq!(first.items[1]["revision"], 2);
    assert_eq!(first.next, Some(2));
    assert!(
        first
            .items
            .iter()
            .all(|item| item.get("name_key").is_none())
    );

    let second = adapter
        .list_revisions(
            "owner-local",
            "workspace-profile",
            "profile-history",
            first.next,
            2,
        )
        .expect("history page continues");
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0]["revision"], 1);
    assert_eq!(second.next, None);

    assert!(matches!(
        adapter.list_revisions(
            "other-owner",
            "workspace-profile",
            "profile-history",
            None,
            2
        ),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn delegation_profile_create_replays_exactly_and_changed_request_conflicts() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-profile");
    seed_enabled_binding(&directory);
    let create = || DelegationProfileCommand::Create {
        profile_id: "profile-replay".into(),
        agent_binding_id: "binding-profile".into(),
        revision: profile_input("Replay", "same request replays its immutable response"),
    };

    let first = execute_profile_command(
        &store,
        "profile-create-replay",
        "event-profile-create-replay",
        1,
        create(),
    )
    .expect("first create commits");
    let replay = execute_profile_command(
        &store,
        "profile-create-replay",
        "event-profile-create-replay-retry",
        2,
        create(),
    )
    .expect("exact replay returns original committed profile");
    assert_eq!(replay, first);

    let changed = execute_profile_command(
        &store,
        "profile-create-replay",
        "event-profile-create-replay-changed",
        3,
        DelegationProfileCommand::Create {
            profile_id: "profile-replay".into(),
            agent_binding_id: "binding-profile".into(),
            revision: profile_input("Replay", "changed payload conflicts"),
        },
    )
    .expect_err("same RequestId cannot change the profile payload");
    assert_eq!(changed, DelegationProfileError::IdempotencyConflict);

    let events = store.read_workspace_events("workspace-profile").unwrap();
    let profile_events = events
        .iter()
        .filter(|event| event.event_type == "delegation_profile.created.v1")
        .collect::<Vec<_>>();
    assert_eq!(
        profile_events.len(),
        1,
        "replay and conflict append no profile event"
    );
    assert_eq!(profile_events[0].event_id, "event-profile-create-replay");
}

#[test]
fn delegation_profile_duplicate_replays_exact_snapshot_and_binds_source_version() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-profile");
    seed_enabled_binding(&directory);
    execute_profile_command(
        &store,
        "duplicate-source-create",
        "event-duplicate-source-create",
        1,
        DelegationProfileCommand::Create {
            profile_id: "profile-copy-source".into(),
            agent_binding_id: "binding-profile".into(),
            revision: profile_input("Source", "revision one"),
        },
    )
    .expect("source profile created");
    execute_profile_command(
        &store,
        "duplicate-source-revise",
        "event-duplicate-source-revise",
        2,
        DelegationProfileCommand::Revise {
            profile_id: "profile-copy-source".into(),
            expected_version: 1,
            revision: profile_input("Source", "reviewed current revision two"),
        },
    )
    .expect("source current revision updated");

    let duplicate = || DelegationProfileCommand::Duplicate {
        source_profile_id: "profile-copy-source".into(),
        expected_version: 2,
        profile_id: "profile-copy-target".into(),
        name: "Copied profile".into(),
    };
    let first = execute_profile_command(
        &store,
        "duplicate-snapshot-request",
        "event-duplicate-snapshot",
        3,
        duplicate(),
    )
    .expect("duplicate committed from the reviewed source version");
    assert_eq!(
        first.revision.routing_description,
        "reviewed current revision two"
    );
    assert_eq!(first.revision.revision, 1);
    assert_eq!(first.profile.status, DelegationProfileStatus::Disabled);

    let replay = execute_profile_command(
        &store,
        "duplicate-snapshot-request",
        "event-duplicate-snapshot-retry",
        4,
        duplicate(),
    )
    .expect("exact duplicate retry returns the original committed snapshot");
    assert_eq!(replay, first);

    let changed_name = execute_profile_command(
        &store,
        "duplicate-snapshot-request",
        "event-duplicate-snapshot-conflict",
        5,
        DelegationProfileCommand::Duplicate {
            source_profile_id: "profile-copy-source".into(),
            expected_version: 2,
            profile_id: "profile-copy-target".into(),
            name: "Different copied profile".into(),
        },
    )
    .expect_err("changed duplicate payload cannot reuse the request ID");
    assert_eq!(changed_name, DelegationProfileError::IdempotencyConflict);
}
