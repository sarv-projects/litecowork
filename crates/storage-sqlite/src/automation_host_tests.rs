use super::*;
use crate::automation_host::{schedule_occurrence_key, settlement_window};
use domain_responsibility::*;
use domain_responsibility::parse_instant;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::time::Duration;

const WORKSPACE: &str = "workspace-automation";
const RUNTIME_ID: &str = "runtime-automation";
const INCARNATION_ID: &str = "incarnation-automation";
const NOW: &str = "2026-10-20T09:00:00Z";

fn host_adapter(store: &SqliteWorkspaceStore) -> SqliteAutomationHostStore {
    SqliteAutomationHostStore::new(
        store.clone(),
        AutomationHostContext {
            origin_runtime_id: RUNTIME_ID.to_owned(),
            recorded_at: NOW.to_owned(),
        },
    )
    .expect("Automation host adapter")
}

fn schedule_trigger_spec(trigger_id: &str, expression: &str) -> TriggerSpec {
    TriggerSpec {
        trigger_id: trigger_id.into(),
        placement: TriggerPlacement::Auto,
        runtime_id: None,
        trigger: TriggerDefinition::Schedule {
            recurrence_format: RecurrenceFormat::Cron5,
            timezone: "UTC".into(),
            rrule_or_cron: expression.into(),
            recurrence_semantics_version: 1,
            start_at: None,
            end_at: None,
            misfire_policy: MisfirePolicy::Skip,
            ambiguous_local_time: AmbiguousLocalTime::Earlier,
            nonexistent_local_time: NonexistentLocalTime::Skip,
        },
    }
}

fn automation_definition(triggers: Vec<TriggerSpec>) -> AutomationDefinition {
    AutomationDefinition {
        routine_id: "routine-1".into(),
        routine_revision: 1,
        triggers,
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

fn coworker_adapter(store: &SqliteWorkspaceStore, id: &str) -> SqliteCoworkerStore {
    let event = context(id, 1);
    SqliteCoworkerStore::new(
        store.clone(),
        CoworkerEventContext {
            event_id: event.event_id,
            origin_runtime_id: RUNTIME_ID.to_owned(),
            hlc_timestamp: event.hlc_timestamp,
            correlation_id: event.correlation_id,
            causation_id: event.causation_id,
            recorded_at: event.recorded_at,
        },
    )
    .unwrap()
}

fn coworker_command(
    store: &SqliteWorkspaceStore,
    request: &str,
    command: ResponsibilityCommand,
) -> Result<CommittedResponsibility, DomainError> {
    ResponsibilityService::new(coworker_adapter(
        store,
        &format!("event-{request}"),
    ))
    .execute(
        &OwnerCommandScope {
            principal_id: "owner-local".into(),
            workspace_id: WORKSPACE.into(),
            request_id: request.into(),
        },
        command,
    )
}

fn seed_routine(directory: &tempfile::TempDir) {
    let connection = Connection::open(state_database(directory)).unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    tx.execute("INSERT INTO routines(routine_id, workspace_id, name, current_revision, status, created_at, updated_at, version) VALUES ('routine-1', ?1, 'Routine', 1, 'ACTIVE', '2026-10-20T08:00:00Z', '2026-10-20T08:00:00Z', 1)", [WORKSPACE]).unwrap();
    tx.execute("INSERT INTO routine_revisions(workspace_id, routine_id, revision, objective_template, instructions, input_schema_json, constraints_json, non_goals_json, required_outputs_json, acceptance_criteria_json, approvals_required_json, input_bindings_json, required_capabilities_json, preferred_agent_binding_id, placement_preference_json, budget_ceiling_json, verification_policy_json, authored_by_json, created_at) VALUES (?1, 'routine-1', 1, 'Review project', 'Review the project', '{}', '[]', '[]', '[]', '[]', '[]', '[]', '[]', NULL, '\"AUTO\"', NULL, '{}', '{\"principal_id\":\"owner-local\",\"kind\":\"USER\"}', '2026-10-20T08:00:00Z')", [WORKSPACE]).unwrap();
    tx.commit().unwrap();
}

/// Registers and enrolls a local Runtime so the Workspace has an ACTIVE
/// `TRIGGER_HOST` binding owned by this process.
fn enroll_trigger_host(store: &SqliteWorkspaceStore, workspace_version: u64) {
    let public_key = [23_u8; 32];
    store
        .register_local_incarnation(
            RuntimeRecord {
                runtime_id: RUNTIME_ID.to_owned(),
                device_identity: DeviceIdentityRecord {
                    device_id: format!(
                        "ed25519-sha256:{}",
                        hex::encode(Sha256::digest(public_key))
                    ),
                    public_key: format!("ed25519:{}", hex::encode(public_key)),
                    key_version: 1,
                    issued_at: NOW.to_owned(),
                    display_name: None,
                },
                runtime_version: "0.1.0-test".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                roles: vec!["OPERATOR_ENDPOINT".to_owned()],
                trust_zone: "PERSONAL_DEVICE".to_owned(),
                availability: "RECOVERING".to_owned(),
                startup_policy: "MANUAL".to_owned(),
                current_incarnation_id: INCARNATION_ID.to_owned(),
                resource_capacity: serde_json::json!({"sampled_at": NOW}),
                last_seen: NOW.to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: INCARNATION_ID.to_owned(),
                runtime_id: RUNTIME_ID.to_owned(),
                process_started_at: NOW.to_owned(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: INCARNATION_ID.to_owned(),
                os_boot_id: Some("boot-automation".to_owned()),
                observed_at: NOW.to_owned(),
            },
        )
        .expect("local Runtime registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: NOW.to_owned(),
            stopped_at: None,
        })
        .expect("Runtime enters control-plane serving state");
    store
        .enroll_local_runtime(storage_core::LocalRuntimeWorkspaceEnrollmentRequest {
            request: storage_core::WorkspaceCreateRequest {
                principal_id: "owner-local".to_owned(),
                request_id: "enroll-automation".to_owned(),
                request_payload: serde_json::json!({"workspace_id": WORKSPACE}),
            },
            expected_workspace_version: workspace_version,
            runtime_workspace_binding_id: "binding-automation".to_owned(),
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            now: NOW.to_owned(),
            correlation_id: "correlation-automation".to_owned(),
        })
        .expect("Runtime enrolls in Workspace");
}

struct Fixture {
    directory: tempfile::TempDir,
    store: SqliteWorkspaceStore,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    let workspace = create_workspace(&store, WORKSPACE);
    seed_routine(&directory);
    enroll_trigger_host(&store, workspace.workspace.version);
    Fixture { directory, store }
}

fn create_automation(store: &SqliteWorkspaceStore, triggers: Vec<TriggerSpec>) {
    coworker_command(
        store,
        "create-automation",
        ResponsibilityCommand::CreateAutomation {
            automation_id: "automation-1".into(),
            name: "Daily review".into(),
            definition: automation_definition(triggers),
        },
    )
    .expect("Automation definition is created in a PAUSED state");
}

fn enable_automation(store: &SqliteWorkspaceStore, expected_version: u64) {
    coworker_command(
        store,
        "enable-automation",
        ResponsibilityCommand::SetAutomationStatus {
            automation_id: "automation-1".into(),
            expected_version,
            status: AutomationStatus::Enabled,
        },
    )
    .expect("Automation is enabled");
}

#[test]
fn a_schedule_trigger_cannot_be_enabled_without_a_local_trigger_host() {
    // A Workspace without an enrolled local Runtime has no `TRIGGER_HOST`
    // binding, so a recurring trigger must fail closed.
    let directory = tempfile::tempdir().unwrap();
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, WORKSPACE);
    seed_routine(&directory);
    create_automation(
        &store,
        vec![schedule_trigger_spec("trigger-daily", "0 9 * * *")],
    );
    assert_eq!(
        coworker_command(
            &store,
            "enable-automation",
            ResponsibilityCommand::SetAutomationStatus {
                automation_id: "automation-1".into(),
                expected_version: 1,
                status: AutomationStatus::Enabled,
            },
        ),
        Err(DomainError::ReconciliationRequired)
    );
    let connection = Connection::open(state_database(&directory)).unwrap();
    let status: String = connection
        .query_row(
            "SELECT status FROM automations WHERE automation_id = 'automation-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "PAUSED");
    let cursors: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM automation_cursors WHERE automation_id = 'automation-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cursors, 0, "a refused enable must not leave a cursor behind");
}

#[test]
fn enabling_a_schedule_trigger_creates_its_durable_cursor_atomically() {
    let fixture = fixture();
    create_automation(
        &fixture.store,
        vec![schedule_trigger_spec("trigger-daily", "0 9 * * *")],
    );
    enable_automation(&fixture.store, 1);
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    let (host, epoch, revision): (String, i64, i64) = connection
        .query_row(
            "SELECT trigger_host_runtime_id, host_epoch, active_automation_revision FROM automation_cursors WHERE automation_id = 'automation-1' AND trigger_id = 'trigger-daily'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(host, RUNTIME_ID);
    assert_eq!(epoch, 1);
    assert_eq!(revision, 1);
}

#[test]
fn a_cursor_is_created_once_and_a_second_host_cannot_take_it_over() {
    let fixture = fixture();
    create_automation(
        &fixture.store,
        vec![schedule_trigger_spec("trigger-daily", "0 9 * * *")],
    );
    let host = host_adapter(&fixture.store);
    let cursor = host
        .create_cursor(WORKSPACE, "automation-1", 1, "trigger-daily", RUNTIME_ID, 1, Some(1_000))
        .expect("the local host creates its cursor");
    assert_eq!(cursor.host_epoch, 1);
    assert_eq!(cursor.next_scheduled_at, Some(1_000));
    assert_eq!(cursor.version, 1);
    // A repeated creation for the same owner is a conflict, not a second cursor.
    assert!(matches!(
        host.create_cursor(
            WORKSPACE,
            "automation-1",
            1,
            "trigger-daily",
            RUNTIME_ID,
            1,
            Some(1_000)
        ),
        Err(StoreError::Conflict { .. })
    ));
    // Another Runtime cannot take the same trigger over without an explicit
    // cursor handoff.
    assert!(matches!(
        host.create_cursor(
            WORKSPACE,
            "automation-1",
            1,
            "trigger-daily",
            "runtime-other",
            2,
            Some(1_000)
        ),
        Err(StoreError::Conflict { .. })
    ));
}

#[test]
fn an_enabled_schedule_trigger_is_listed_for_its_owning_runtime_only() {
    let fixture = fixture();
    create_automation(
        &fixture.store,
        vec![schedule_trigger_spec("trigger-daily", "0 9 * * *")],
    );
    let host = host_adapter(&fixture.store);
    host.create_cursor(
        WORKSPACE,
        "automation-1",
        1,
        "trigger-daily",
        RUNTIME_ID,
        1,
        Some(1_000),
    )
    .expect("cursor");
    enable_automation(&fixture.store, 1);

    let due = host
        .list_due_schedules(RUNTIME_ID, 5_000)
        .expect("due schedules are listed");
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].automation_id, "automation-1");
    assert_eq!(due[0].trigger.trigger_id, "trigger-daily");
    assert_eq!(due[0].routine_id, "routine-1");
    assert_eq!(due[0].cursor.as_ref().map(|cursor| cursor.host_epoch), Some(1));
    // Another Runtime owns no trigger here.
    assert!(host.list_due_schedules("runtime-other", 5_000).unwrap().is_empty());

    // Pausing the Automation stops new claims without discarding the cursor.
    coworker_command(
        &fixture.store,
        "pause-automation",
        ResponsibilityCommand::SetAutomationStatus {
            automation_id: "automation-1".into(),
            expected_version: 2,
            status: AutomationStatus::Paused,
        },
    )
    .expect("Automation pauses");
    assert!(host.list_due_schedules(RUNTIME_ID, 5_000).unwrap().is_empty());
}

#[test]
fn a_misfire_records_a_bounded_skipped_occurrence_and_advances_the_cursor() {
    let fixture = fixture();
    create_automation(
        &fixture.store,
        vec![schedule_trigger_spec("trigger-daily", "0 9 * * *")],
    );
    let host = host_adapter(&fixture.store);
    enable_automation(&fixture.store, 1);
    // The host observes the first slot and persists it as the next deadline.
    let first_slot = parse_instant("2026-03-01T09:00:00Z").unwrap();
    host.advance_cursor(
        WORKSPACE,
        "automation-1",
        "trigger-daily",
        Some(first_slot),
        first_slot,
        false,
    )
    .expect("the cursor records the first observed slot");

    // The host was asleep for three days and wakes up after the fourth slot.
    let now = parse_instant("2026-03-04T09:00:30Z").unwrap();
    let due = host.list_due_schedules(RUNTIME_ID, now).unwrap();
    let schedule = &due[0];
    let settlement = schedule
        .schedule
        .plan_settlement(
            schedule.cursor.as_ref().and_then(|cursor| cursor.next_scheduled_at),
            settlement_window(now, 60_000),
        )
        .expect("the schedule plans a settlement");
    assert!(settlement.misfired);
    assert!(!settlement.intents.is_empty());

    let mut recorded = 0_usize;
    let mut first_missed = None;
    for intent in &settlement.intents {
        let OccurrenceIntent::Skip { instant, reason } = intent else {
            continue;
        };
        first_missed.get_or_insert(*instant);
        let key = schedule_occurrence_key("trigger-daily", *instant).unwrap();
        let created = host
            .record_skipped_occurrence(
                WORKSPACE,
                "automation-1",
                1,
                "routine-1",
                1,
                "trigger-daily",
                RUNTIME_ID,
                &format!("occurrence-{instant}"),
                &key,
                *instant,
                &format!("MISFIRE_{reason:?}"),
            )
            .expect("the missed slot is recorded");
        assert!(created);
        recorded += 1;
    }
    assert!(recorded > 0, "a missed slot must never be dropped");
    assert_eq!(recorded, 3, "three daily slots elapsed while the host was asleep");

    // The same slot is never recorded twice.
    let missed = first_missed.expect("at least one missed slot");
    let key = schedule_occurrence_key("trigger-daily", missed).unwrap();
    assert!(
        !host
            .record_skipped_occurrence(
                WORKSPACE,
                "automation-1",
                1,
                "routine-1",
                1,
                "trigger-daily",
                RUNTIME_ID,
                &format!("occurrence-{missed}"),
                &key,
                missed,
                "MISFIRE_MissedSlot",
            )
            .expect("a repeated settlement records nothing new")
    );

    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    let (count, status): (i64, String) = connection
        .query_row(
            "SELECT COUNT(*), MAX(status) FROM automation_occurrences WHERE automation_id = 'automation-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(count, i64::try_from(recorded).unwrap());
    assert_eq!(status, "SKIPPED");
    let (tasks,): (i64,) = connection
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE automation_id = 'automation-1'",
            [],
            |row| Ok((row.get(0)?,)),
        )
        .unwrap();
    assert_eq!(tasks, 0, "a skipped misfire must not create a Task");
}

#[test]
fn the_cursor_advance_is_monotonic_and_never_rewinds_on_a_backwards_clock() {
    let fixture = fixture();
    create_automation(
        &fixture.store,
        vec![schedule_trigger_spec("trigger-daily", "0 9 * * *")],
    );
    let host = host_adapter(&fixture.store);
    host.create_cursor(
        WORKSPACE,
        "automation-1",
        1,
        "trigger-daily",
        RUNTIME_ID,
        1,
        Some(1_000),
    )
    .expect("cursor");
    let advanced = host
        .advance_cursor(WORKSPACE, "automation-1", "trigger-daily", Some(2_000), 2_000, false)
        .expect("the cursor advances");
    assert_eq!(advanced.next_scheduled_at, Some(2_000));
    assert_eq!(advanced.version, 2);

    // A clock that moved backwards must not rewind the persisted deadline.
    let unchanged = host
        .advance_cursor(WORKSPACE, "automation-1", "trigger-daily", Some(1_000), 500, false)
        .expect("the cursor is unchanged");
    assert_eq!(unchanged.next_scheduled_at, Some(2_000));

    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    let (next, version): (Option<String>, i64) = connection
        .query_row(
            "SELECT next_scheduled_at, version FROM automation_cursors WHERE automation_id = 'automation-1' AND trigger_id = 'trigger-daily'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(next.as_deref(), Some("1970-01-01T00:00:02.000000000Z"));
    assert_eq!(version, 2);
}

#[test]
fn an_occurrence_key_is_stable_for_the_same_trigger_and_slot() {
    let first = schedule_occurrence_key("trigger-daily", 1_000).unwrap();
    let second = schedule_occurrence_key("trigger-daily", 1_000).unwrap();
    let other = schedule_occurrence_key("trigger-daily", 2_000).unwrap();
    assert_eq!(first, second);
    assert_ne!(first, other);
    assert_eq!(first.len(), 64);
    assert!(first.chars().all(|value| value.is_ascii_hexdigit()));
}

#[test]
fn a_cron_schedule_trigger_parses_into_an_executable_plan() {
    let spec = schedule_trigger_spec("trigger-daily", "0 9 * * *");
    let schedule = crate::automation_host::schedule_trigger(&spec);
    assert!(
        schedule.is_some(),
        "a bounded cron trigger must parse into an executable schedule"
    );
}

/// Registers the lead Agent binding that a materialized occurrence Task needs.
fn seed_agent_binding(directory: &tempfile::TempDir) {
    let connection = Connection::open(state_database(directory)).unwrap();
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id, provider_key, display_name, discovered_at)
             VALUES ('profile-automation', 'fixture', 'Fixture Agent', '2026-10-20T08:00:00Z')",
            [],
        )
        .expect("fixture AgentProfile");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id, workspace_id, agent_profile_id, endpoint_selection_policy_json, configuration_json, enabled, lead_eligible, created_at, version)
             VALUES ('binding-automation', ?1, 'profile-automation', '{}', '{}', 1, 1, '2026-10-20T08:00:00Z', 1)",
            params![WORKSPACE],
        )
        .expect("fixture AgentBinding");
}

/// The occurrence identity of one scheduled slot.
fn slot_identity(trigger_id: &str, instant: i64) -> String {
    OccurrenceIdentity::Schedule {
        trigger_id: trigger_id.to_owned(),
        scheduled_epoch_ms: instant,
    }
    .key()
    .expect("the occurrence identity is valid")
}

/// Builds the atomic Task commit of one scheduled slot.
///
/// The payload mirrors the canonical Operator payload: only the fields the
/// storage contract accepts are present, and the Automation provenance matches
/// the admission exactly.
fn scheduled_task_commit(
    task_id: &str,
    occurrence_id: &str,
    occurrence_key: &str,
    trigger_id: &str,
    scheduled_epoch_ms: i64,
    automation_revision: u64,
    automation_version: u64,
) -> storage_core::TaskCreateCommit {
    // The database claim fence compares the persisted claim window with the real
    // clock, so a fixture that pinned a fixed past instant could never leave the
    // CLAIMED state. The recorded instant and the claim window are therefore
    // derived from the current instant.
    let now = time::OffsetDateTime::now_utc();
    let recorded_at = now
        .format(&time::format_description::well_known::Rfc3339)
        .expect("the recorded instant is representable");
    let claim_expires_at = (now + time::Duration::minutes(2))
        .format(&time::format_description::well_known::Rfc3339)
        .expect("the live claim window is representable");
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
        "preferred_lead_agent_binding_id": "binding-automation",
        "routine_id": "routine-1",
        "routine_revision": 1,
        "routine_inputs": {},
        "automation_id": "automation-1",
        "automation_revision": automation_revision,
        "expected_automation_version": automation_version,
        "trigger_id": trigger_id,
        "occurrence_id": occurrence_id,
    });
    let created_by = serde_json::json!({"kind": "USER", "principal_id": "owner-local"});
    let lead_failover_policy = serde_json::json!({
        "mode": "DISABLED",
        "triggers": [],
        "fallback_agent_binding_ids": [],
        "max_lead_changes": 0
    });
    storage_core::TaskCreateCommit {
        request: storage_core::WorkspaceCreateRequest {
            principal_id: "owner-local".to_owned(),
            request_id: format!("automation-host:{occurrence_key}"),
            request_payload: request_payload.clone(),
        },
        idempotency_payload: Some(request_payload),
        expected_coworker_version: None,
        routine_admission: Some(storage_core::RoutineTaskAdmission {
            routine_id: "routine-1".to_owned(),
            routine_revision: 1,
            inputs: serde_json::json!({}),
        }),
        automation_admission: Some(storage_core::AutomationTaskAdmission {
            automation_id: "automation-1".to_owned(),
            automation_revision,
            expected_automation_version: automation_version,
            routine_id: "routine-1".to_owned(),
            routine_revision: 1,
            trigger_id: trigger_id.to_owned(),
            trigger_host_runtime_id: RUNTIME_ID.to_owned(),
            trigger_host_runtime_incarnation_id: INCARNATION_ID.to_owned(),
            trigger_host_binding_version: 1,
            occurrence_id: occurrence_id.to_owned(),
            occurrence_key: occurrence_key.to_owned(),
            claim_expires_at,
            trigger_kind: "SCHEDULE".to_owned(),
            scheduled_epoch_ms: Some(scheduled_epoch_ms),
            covered_misfire_range: None,
        }),
        task: storage_core::TaskRecord {
            task_id: task_id.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            conversation_id: None,
            current_spec_revision: 1,
            current_plan_revision: None,
            status: "READY".to_owned(),
            resume_status: None,
            routine_id: Some("routine-1".to_owned()),
            routine_revision: Some(1),
            automation_id: Some("automation-1".to_owned()),
            automation_occurrence_id: Some(occurrence_id.to_owned()),
            origin_coworker_id: None,
            origin_coworker_revision: None,
            lead_agent_binding_id: "binding-automation".to_owned(),
            blocking_conditions: Vec::new(),
            priority: "NORMAL".to_owned(),
            created_by: created_by.clone(),
            created_at: recorded_at.to_owned(),
            updated_at: recorded_at.to_owned(),
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
                "Routine instructions (untrusted task context):\nReview the project".to_owned()
            ],
            non_goals: Vec::new(),
            input_refs: Vec::new(),
            workspace_instruction_revision: None,
            required_outputs: Vec::new(),
            acceptance_criteria: Vec::new(),
            approvals_required: Vec::new(),
            budget: None,
            delegation_budget_policy: None,
            lead_failover_policy,
            deadline: None,
            source_message_refs: Vec::new(),
            placement_preference: serde_json::json!("AUTO"),
            preferred_lead_agent_binding_id: Some("binding-automation".to_owned()),
            authored_by: created_by,
            created_at: recorded_at.to_owned(),
        },
        event: storage_core::EventDraft {
            event_id: format!("ev-automation-{occurrence_id}"),
            workspace_id: WORKSPACE.to_owned(),
            entity_type: "Task".to_owned(),
            entity_id: task_id.to_owned(),
            origin_runtime_id: RUNTIME_ID.to_owned(),
            entity_revision: 1,
            hlc_timestamp: recorded_at.to_owned(),
            correlation_id: "correlation-automation".to_owned(),
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
                "routine_revision": 1,
                "automation_id": "automation-1",
                "automation_occurrence_id": occurrence_id,
            }),
            recorded_at: recorded_at.to_owned(),
        },
    }
}

/// The first daily slot of the fixture schedule.
fn first_slot() -> i64 {
    parse_instant("2026-03-01T09:00:00Z").expect("the slot instant is valid")
}

/// Prepares an enabled Automation whose cursor names the first daily slot.
fn enabled_schedule_fixture() -> Fixture {
    let fixture = fixture();
    seed_agent_binding(&fixture.directory);
    create_automation(
        &fixture.store,
        vec![schedule_trigger_spec("trigger-daily", "0 9 * * *")],
    );
    enable_automation(&fixture.store, 1);
    let host = host_adapter(&fixture.store);
    host.advance_cursor(
        WORKSPACE,
        "automation-1",
        "trigger-daily",
        Some(first_slot()),
        first_slot(),
        false,
    )
    .expect("the cursor records the first observed slot");
    fixture
}

#[test]
fn a_due_schedule_slot_materializes_an_ordinary_ready_task() {
    let fixture = enabled_schedule_fixture();
    let slot = first_slot();
    let key = slot_identity("trigger-daily", slot);
    let commit = scheduled_task_commit(
        "task-automation-1",
        "occurrence-automation-1",
        &key,
        "trigger-daily",
        slot,
        1,
        2,
    );
    let committed = fixture
        .store
        .create_task(commit)
        .expect("the due slot materializes into an ordinary Task");
    assert_eq!(committed.view.task.status, "READY");
    assert_eq!(committed.view.task.routine_id.as_deref(), Some("routine-1"));
    assert_eq!(committed.view.task.routine_revision, Some(1));
    assert_eq!(
        committed.view.task.automation_id.as_deref(),
        Some("automation-1")
    );
    assert_eq!(
        committed.view.task.automation_occurrence_id.as_deref(),
        Some("occurrence-automation-1")
    );
    assert_eq!(committed.view.current_spec_revision.revision, 1);

    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    let (status, version, claim_epoch, scheduled_for, task_id): (
        String,
        i64,
        i64,
        Option<String>,
        Option<String>,
    ) = connection
        .query_row(
            "SELECT status, version, claim_epoch, scheduled_for, task_id FROM automation_occurrences WHERE occurrence_key = ?1",
            params![key],
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
        .unwrap();
    assert_eq!(status, "STARTED");
    assert_eq!(version, 3);
    assert_eq!(claim_epoch, 1);
    assert_eq!(scheduled_for.as_deref(), Some("2026-03-01T09:00:00.000000000Z"));
    assert_eq!(task_id.as_deref(), Some("task-automation-1"));

    // The occurrence commits no planning or execution record.
    let (plans, steps, attempts, effects): (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM plan_revisions), (SELECT COUNT(*) FROM steps), (SELECT COUNT(*) FROM attempts), (SELECT COUNT(*) FROM effects)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!((plans, steps, attempts, effects), (0, 0, 0, 0));
}

#[test]
fn a_repeated_settlement_of_the_same_slot_never_creates_a_second_task() {
    let fixture = enabled_schedule_fixture();
    let slot = first_slot();
    let key = slot_identity("trigger-daily", slot);
    let commit = scheduled_task_commit(
        "task-automation-1",
        "occurrence-automation-1",
        &key,
        "trigger-daily",
        slot,
        1,
        2,
    );
    let first = fixture
        .store
        .create_task(commit.clone())
        .expect("the due slot materializes");
    let second = fixture
        .store
        .create_task(commit)
        .expect("the repeated settlement replays the committed receipt");
    assert_eq!(first.view.task.task_id, second.view.task.task_id);
    assert_eq!(first.event.event_id, second.event.event_id);

    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    let (tasks, occurrences): (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM tasks WHERE automation_id = 'automation-1'), (SELECT COUNT(*) FROM automation_occurrences WHERE automation_id = 'automation-1')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((tasks, occurrences), (1, 1));
}

#[test]
fn a_slot_before_the_persisted_cursor_deadline_is_rejected() {
    let fixture = enabled_schedule_fixture();
    // The persisted cursor already names 2026-03-01 as the earliest slot that
    // has not been materialized. A host must not claim an earlier slot.
    let stale = parse_instant("2026-02-25T09:00:00Z").unwrap();
    let commit = scheduled_task_commit(
        "task-automation-stale",
        "occurrence-automation-stale",
        &slot_identity("trigger-daily", stale),
        "trigger-daily",
        stale,
        1,
        2,
    );
    assert!(matches!(
        fixture.store.create_task(commit),
        Err(StoreError::Conflict { .. })
    ));
    // The due slot itself is accepted.
    let slot = first_slot();
    let commit = scheduled_task_commit(
        "task-automation-1",
        "occurrence-automation-1",
        &slot_identity("trigger-daily", slot),
        "trigger-daily",
        slot,
        1,
        2,
    );
    assert!(fixture.store.create_task(commit).is_ok());
}

#[test]
fn a_slot_that_is_not_a_slot_of_the_pinned_recurrence_is_rejected() {
    let fixture = enabled_schedule_fixture();
    // 09:30 is not a slot of the `0 9 * * *` recurrence.
    let invented = parse_instant("2026-03-01T09:30:00Z").unwrap();
    let commit = scheduled_task_commit(
        "task-automation-invented",
        "occurrence-automation-invented",
        &slot_identity("trigger-daily", invented),
        "trigger-daily",
        invented,
        1,
        2,
    );
    assert!(matches!(
        fixture.store.create_task(commit),
        Err(StoreError::Invalid(_))
    ));
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    let tasks: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE automation_id = 'automation-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tasks, 0, "a rejected slot must not create a Task");
}

#[test]
fn a_paused_automation_never_materializes_a_slot() {
    let fixture = enabled_schedule_fixture();
    coworker_command(
        &fixture.store,
        "pause-automation",
        ResponsibilityCommand::SetAutomationStatus {
            automation_id: "automation-1".into(),
            expected_version: 2,
            status: AutomationStatus::Paused,
        },
    )
    .expect("Automation pauses");
    let slot = first_slot();
    let commit = scheduled_task_commit(
        "task-automation-paused",
        "occurrence-automation-paused",
        &slot_identity("trigger-daily", slot),
        "trigger-daily",
        slot,
        1,
        2,
    );
    assert!(matches!(
        fixture.store.create_task(commit),
        Err(StoreError::Conflict { .. })
    ));
}

#[test]
fn a_slot_claimed_by_another_runtime_is_rejected() {
    let fixture = enabled_schedule_fixture();
    let slot = first_slot();
    let mut commit = scheduled_task_commit(
        "task-automation-foreign",
        "occurrence-automation-foreign",
        &slot_identity("trigger-daily", slot),
        "trigger-daily",
        slot,
        1,
        2,
    );
    let admission = commit.automation_admission.as_mut().expect("admission");
    admission.trigger_host_runtime_id = "runtime-other".to_owned();
    assert!(matches!(
        fixture.store.create_task(commit),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn a_slot_claimed_without_the_local_trigger_host_binding_is_rejected() {
    let fixture = enabled_schedule_fixture();
    let slot = first_slot();
    let mut commit = scheduled_task_commit(
        "task-automation-unbound",
        "occurrence-automation-unbound",
        &slot_identity("trigger-daily", slot),
        "trigger-daily",
        slot,
        1,
        2,
    );
    let admission = commit.automation_admission.as_mut().expect("admission");
    admission.trigger_host_binding_version = 99;
    assert!(matches!(
        fixture.store.create_task(commit),
        Err(StoreError::Conflict { .. })
    ));
}

#[test]
fn the_host_reads_back_the_occurrence_of_a_materialized_slot() {
    let fixture = enabled_schedule_fixture();
    let slot = first_slot();
    let key = slot_identity("trigger-daily", slot);
    let commit = scheduled_task_commit(
        "task-automation-1",
        "occurrence-automation-1",
        &key,
        "trigger-daily",
        slot,
        1,
        2,
    );
    fixture.store.create_task(commit).expect("materialization");
    let host = host_adapter(&fixture.store);
    let existing = host
        .occurrence_for_slot(WORKSPACE, "automation-1", "trigger-daily", &key)
        .expect("the occurrence is readable")
        .expect("the occurrence exists");
    assert_eq!(existing.occurrence_id, "occurrence-automation-1");
    assert_eq!(existing.status, OccurrenceStatus::Started);
    assert_eq!(existing.task_id.as_deref(), Some("task-automation-1"));
    assert_eq!(existing.version, 3);
    assert_eq!(
        existing.scheduled_for.as_deref(),
        Some("2026-03-01T09:00:00.000000000Z")
    );
    assert_eq!(
        host.nonterminal_occurrence_count(WORKSPACE, "automation-1")
            .expect("the nonterminal count is readable"),
        1
    );
}

#[test]
fn a_trigger_without_its_own_cursor_is_never_settled_against_another_cursor() {
    let fixture = fixture();
    create_automation(
        &fixture.store,
        vec![
            schedule_trigger_spec("trigger-daily", "0 9 * * *"),
            schedule_trigger_spec("trigger-weekly", "0 9 * * 1"),
        ],
    );
    let host = host_adapter(&fixture.store);
    enable_automation(&fixture.store, 1);
    let now = parse_instant("2026-03-01T09:00:00Z").unwrap();
    let before = host
        .list_due_schedules(RUNTIME_ID, now)
        .expect("the due schedules are readable");
    assert_eq!(
        before.len(),
        2,
        "both triggers own a cursor after the definition is enabled"
    );
    // Remove one cursor so the remaining trigger must not borrow it.
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    connection
        .execute(
            "DELETE FROM automation_cursors WHERE automation_id = 'automation-1' AND trigger_id = 'trigger-weekly'",
            [],
        )
        .unwrap();
    let after = host
        .list_due_schedules(RUNTIME_ID, now)
        .expect("the due schedules are readable");
    assert_eq!(
        after.len(),
        1,
        "a trigger without its own cursor must not be hosted here"
    );
    assert_eq!(after[0].trigger.trigger_id, "trigger-daily");
    assert_eq!(
        after[0]
            .cursor
            .as_ref()
            .map(|cursor| cursor.trigger_id.as_str()),
        Some("trigger-daily")
    );
}

#[test]
fn the_trigger_host_list_only_names_workspaces_this_runtime_still_hosts() {
    let fixture = fixture();
    let host = host_adapter(&fixture.store);
    let hosted = host
        .list_trigger_host_workspaces(RUNTIME_ID)
        .expect("the host workspace list is readable");
    assert_eq!(hosted.len(), 1);
    assert_eq!(hosted[0].workspace_id, WORKSPACE);
    assert!(
        hosted[0]
            .default_agent_binding_id
            .as_deref()
            .is_none_or(|value| !value.is_empty())
    );
    // Another Runtime never inherits the host role of this Workspace.
    assert!(
        host.list_trigger_host_workspaces("runtime-other")
            .expect("the host workspace list is readable")
            .is_empty()
    );
}

#[test]
fn a_revoked_trigger_host_binding_removes_the_workspace_from_the_host_list() {
    let fixture = fixture();
    let connection = Connection::open(state_database(&fixture.directory)).unwrap();
    connection
        .execute(
            "UPDATE runtime_workspace_bindings SET status = 'REVOKED', revoked_at = '2026-10-20T10:00:00Z', version = version + 1 WHERE runtime_id = ?1",
            [RUNTIME_ID],
        )
        .unwrap();
    let host = host_adapter(&fixture.store);
    assert!(
        host.list_trigger_host_workspaces(RUNTIME_ID)
            .expect("the host workspace list is readable")
            .is_empty(),
        "a revoked binding must stop the recurring owner immediately"
    );
}

#[test]
fn the_occurrence_history_reports_the_persisted_occurrence_and_its_task() {
    let fixture = enabled_schedule_fixture();
    let slot = first_slot();
    let key = slot_identity("trigger-daily", slot);
    let commit = scheduled_task_commit(
        "task-automation-1",
        "occurrence-automation-1",
        &key,
        "trigger-daily",
        slot,
        1,
        2,
    );
    fixture.store.create_task(commit).expect("materialization");
    let host = host_adapter(&fixture.store);
    let page = host
        .list_occurrence_page(WORKSPACE, "automation-1", None, 50)
        .expect("the occurrence history is readable");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].occurrence_id, "occurrence-automation-1");
    assert_eq!(page[0].status, "STARTED");
    assert_eq!(page[0].task_id.as_deref(), Some("task-automation-1"));
    assert_eq!(page[0].trigger_id, "trigger-daily");
    assert_eq!(page[0].version, 3);
    assert_eq!(page[0].claim_epoch, 1);
    assert!(
        page[0]
            .scheduled_for
            .as_deref()
            .is_some_and(|value| value.starts_with("2026-03-01T09:00:00")),
        "a scheduled occurrence keeps the resolved slot instant"
    );
    // The occurrence carries the exact pins the Automation occurrence contract
    // requires, so the run history never reports an unbound occurrence.
    assert_eq!(page[0].automation_id, "automation-1");
    assert_eq!(page[0].automation_revision, 1);
    assert_eq!(page[0].routine_id, "routine-1");
    assert_eq!(page[0].routine_revision, 1);
    assert_eq!(page[0].trigger_host_runtime_id, RUNTIME_ID);
    assert_eq!(page[0].occurrence_key.len(), 64);
    assert!(page[0].blockers.is_empty());
}

#[test]
fn the_health_facts_report_host_readiness_the_pinned_routine_and_the_occurrences() {
    let fixture = enabled_schedule_fixture();
    let host = host_adapter(&fixture.store);
    let facts = host
        .health_facts(WORKSPACE, "automation-1")
        .expect("the health facts are readable")
        .expect("the Automation exists");
    assert_eq!(facts.automation_id, "automation-1");
    assert_eq!(facts.status, "ENABLED");
    assert_eq!(facts.routine_id, "routine-1");
    assert_eq!(facts.routine_revision, 1);
    assert_eq!(facts.routine_status, "ACTIVE");
    assert!(facts.host_ready, "the enrolled Runtime hosts this Workspace");
    assert_eq!(facts.counts.completed, 0);
    assert_eq!(facts.nonterminal_occurrences, 0);
    assert!(facts.routine_tasks.is_empty());

    // A disabled or missing Automation has no health projection at all.
    assert!(
        host.health_facts(WORKSPACE, "automation-missing")
            .expect("the health facts are readable")
            .is_none()
    );
}

#[test]
fn the_health_facts_count_the_recorded_skipped_misfires() {
    let fixture = fixture();
    create_automation(
        &fixture.store,
        vec![schedule_trigger_spec("trigger-daily", "0 9 * * *")],
    );
    let host = host_adapter(&fixture.store);
    enable_automation(&fixture.store, 1);
    let first_slot = parse_instant("2026-03-01T09:00:00Z").unwrap();
    host.advance_cursor(
        WORKSPACE,
        "automation-1",
        "trigger-daily",
        Some(first_slot),
        first_slot,
        false,
    )
    .expect("the cursor records the first observed slot");
    let missed = parse_instant("2026-03-02T09:00:00Z").unwrap();
    let key = schedule_occurrence_key("trigger-daily", missed).unwrap();
    assert!(
        host.record_skipped_occurrence(
            WORKSPACE,
            "automation-1",
            1,
            "routine-1",
            1,
            "trigger-daily",
            RUNTIME_ID,
            "occurrence-missed",
            &key,
            missed,
            "MISFIRED_SLOT_SKIPPED",
        )
        .expect("the missed slot is recorded")
    );
    let facts = host
        .health_facts(WORKSPACE, "automation-1")
        .expect("the health facts are readable")
        .expect("the Automation exists");
    assert_eq!(facts.counts.skipped, 1);
    assert_eq!(facts.nonterminal_occurrences, 0);
}
