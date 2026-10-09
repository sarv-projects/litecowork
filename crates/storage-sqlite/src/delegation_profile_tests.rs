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
