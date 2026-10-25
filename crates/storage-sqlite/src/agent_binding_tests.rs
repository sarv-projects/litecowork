use super::*;
use domain_workspace::{
    CreateWorkspace, EventContext, SetWorkspaceDefaultAgentBinding, WorkspaceService,
};
use rusqlite::{Connection, params};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use storage_core::{
    ActivateTaskPlanningSession, AgentBindingCreateRequest, AgentBindingDisableRequest,
    AgentBindingEnableRequest, AgentBindingRecord, AgentCatalogStore, AgentEndpointRecord,
    AgentProfileRecord, AggregateStateRef, BlobPurpose, BlobRef, BlobStore, DeviceIdentityRecord,
    EventDraft, LocalAgentEndpointBindingInput, LocalRuntimeWorkspaceEnrollmentRequest,
    PlanAcceptanceCommit, PlanRevisionRecord, PlannedStepRecord,
    RuntimeIncarnationLocalObservationRecord, RuntimeIncarnationRecord,
    RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeOfferRecord, RuntimeRecord,
    RuntimeWorkspaceBindingStore, StepRecord, WorkspaceCreateRequest,
};

const WORKSPACE_ID: &str = "workspace-agent-binding-tests";
const OWNER_ID: &str = "owner-local";
const RUNTIME_ID: &str = "runtime-agent-binding-tests";
const INCARNATION_ID: &str = "incarnation-agent-binding-tests";
const PROFILE_ID: &str = "profile-agent-binding-tests";
const ENDPOINT_ID: &str = "endpoint-agent-binding-tests";
const BINDING_ID: &str = "binding-agent-binding-tests";
const NOW: &str = "2026-10-15T10:02:00Z";
const FUTURE: &str = "2099-10-15T10:02:00Z";

struct Fixture {
    directory: tempfile::TempDir,
    store: SqliteWorkspaceStore,
    binding: AgentBindingRecord,
}

#[derive(Clone)]
struct TestKeys([u8; 32]);

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: zeroize::Zeroizing::new(self.0),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob("unknown test key version".to_owned()));
        }
        self.current_key(workspace_id, purpose)
    }
}

struct PausingAgentBindingBlobStore {
    inner: FileBlobStore<TestKeys>,
    published: mpsc::Sender<()>,
    resume_put: Mutex<mpsc::Receiver<()>>,
}

impl BlobStore for PausingAgentBindingBlobStore {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
        let blob = self.inner.put(workspace_id, purpose, bytes, media_type)?;
        self.published
            .send(())
            .map_err(|error| StoreError::Io(error.to_string()))?;
        self.resume_put
            .lock()
            .map_err(|_| StoreError::Io("AgentBinding test gate is unavailable".to_owned()))?
            .recv()
            .map_err(|error| StoreError::Io(error.to_string()))?;
        Ok(blob)
    }

    fn get(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<Vec<u8>, StoreError> {
        self.inner.get(workspace_id, purpose, blob)
    }
}

fn database_path(directory: &tempfile::TempDir) -> PathBuf {
    directory.path().join("state").join("state.sqlite3")
}

fn event_context(event_id: &str, minute: u8) -> EventContext {
    let timestamp = format!("2026-10-15T10:{minute:02}:00Z");
    EventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: "runtime-local".to_owned(),
        hlc_timestamp: timestamp.clone(),
        correlation_id: format!("correlation-{event_id}"),
        causation_id: None,
        recorded_at: timestamp,
    }
}

fn draft(
    event_id: &str,
    workspace_id: &str,
    entity_type: &str,
    entity_id: &str,
    revision: u64,
    event_type: &str,
    payload: Value,
) -> EventDraft {
    EventDraft {
        event_id: event_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        entity_type: entity_type.to_owned(),
        entity_id: entity_id.to_owned(),
        origin_runtime_id: RUNTIME_ID.to_owned(),
        entity_revision: revision,
        hlc_timestamp: NOW.to_owned(),
        correlation_id: format!("correlation-{event_id}"),
        causation_id: None,
        schema_version: 1,
        event_type: event_type.to_owned(),
        payload,
        recorded_at: NOW.to_owned(),
    }
}

fn setup() -> Fixture {
    let directory = tempfile::tempdir().expect("temporary storage directory");
    let store = SqliteWorkspaceStore::open(
        database_path(&directory),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys([19_u8; 32]),
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(1),
        },
    )
    .expect("SQLite store opens");

    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Agent Binding Test Workspace".to_owned(),
            owner_principal_id: OWNER_ID.to_owned(),
            event: event_context("event-workspace-create", 0),
        })
        .expect("Workspace is created");

    let public_key = [7_u8; 32];
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
                    issued_at: "2026-10-15T10:00:00Z".to_owned(),
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
                resource_capacity: serde_json::json!({"sampled_at":"2026-10-15T10:00:00Z"}),
                last_seen: "2026-10-15T10:00:00Z".to_owned(),
            },
            RuntimeIncarnationRecord {
                runtime_incarnation_id: INCARNATION_ID.to_owned(),
                runtime_id: RUNTIME_ID.to_owned(),
                process_started_at: "2026-10-15T10:00:00Z".to_owned(),
                litecowork_version: "0.1.0-test".to_owned(),
                recovered_from_unclean_shutdown: false,
                recovery_state: "RECOVERING".to_owned(),
                ready_at: None,
                stopped_at: None,
                version: 1,
            },
            RuntimeIncarnationLocalObservationRecord {
                runtime_incarnation_id: INCARNATION_ID.to_owned(),
                os_boot_id: Some("boot-agent-binding-tests".to_owned()),
                observed_at: "2026-10-15T10:00:00Z".to_owned(),
            },
        )
        .expect("Runtime incarnation registers");
    store
        .transition_local_incarnation(RuntimeIncarnationStateUpdate {
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            expected_version: 1,
            recovery_state: "DEGRADED".to_owned(),
            availability: "DEGRADED".to_owned(),
            observed_at: "2026-10-15T10:01:00Z".to_owned(),
            stopped_at: None,
        })
        .expect("Runtime is available for endpoint setup");
    let workspace = store
        .get_workspace(WORKSPACE_ID)
        .expect("Workspace read")
        .expect("Workspace exists");
    store
        .enroll_local_runtime(LocalRuntimeWorkspaceEnrollmentRequest {
            request: WorkspaceCreateRequest {
                principal_id: OWNER_ID.to_owned(),
                request_id: "request-enroll-agent-binding-tests".to_owned(),
                request_payload: serde_json::json!({"workspace_id": WORKSPACE_ID}),
            },
            expected_workspace_version: workspace.version,
            runtime_workspace_binding_id: "runtime-workspace-binding-agent-tests".to_owned(),
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            workspace_id: WORKSPACE_ID.to_owned(),
            now: "2026-10-15T10:01:00Z".to_owned(),
            correlation_id: "correlation-enroll-agent-tests".to_owned(),
        })
        .expect("Runtime enrolls in the Workspace");

    store
        .put_agent_profile(
            AgentProfileRecord {
                agent_profile_id: PROFILE_ID.to_owned(),
                provider_key: "test-provider".to_owned(),
                display_name: "Test Agent".to_owned(),
                discovered_at: "2026-10-15T10:00:00Z".to_owned(),
            },
            vec![AgentEndpointRecord {
                endpoint_id: ENDPOINT_ID.to_owned(),
                agent_profile_id: PROFILE_ID.to_owned(),
                protocol: "ACP".to_owned(),
                topology: "PROCESS_ADAPTER".to_owned(),
                protocol_version: Some("1".to_owned()),
                capabilities: serde_json::json!({"streaming":true}),
            }],
        )
        .expect("Agent profile and endpoint are stored");
    store
        .register_local_endpoint_binding(LocalAgentEndpointBindingInput {
            endpoint_id: ENDPOINT_ID.to_owned(),
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            endpoint_ref: "ipc:///private/agent-binding-test".to_owned(),
            observed_at: NOW.to_owned(),
            expires_at: Some(FUTURE.to_owned()),
        })
        .expect("Runtime-local endpoint binding is stored");
    store
        .publish_runtime_offer(RuntimeOfferRecord {
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            offer_kind: "AGENT_ENDPOINT".to_owned(),
            offer_ref: ENDPOINT_ID.to_owned(),
            compatible: true,
            readiness: "AVAILABLE".to_owned(),
            constraints: serde_json::json!({"test":true}),
            observed_at: NOW.to_owned(),
            expires_at: FUTURE.to_owned(),
        })
        .expect("Runtime endpoint offer is stored");

    let binding = AgentBindingRecord {
        agent_binding_id: BINDING_ID.to_owned(),
        workspace_id: WORKSPACE_ID.to_owned(),
        agent_profile_id: PROFILE_ID.to_owned(),
        runtime_id: None,
        endpoint_selection_policy: serde_json::json!({
            "mode":"AUTO_COMPATIBLE",
            "required_features":[],
            "preferred_topologies":["PROCESS_ADAPTER"]
        }),
        auth_ref: None,
        configuration: serde_json::json!({}),
        enabled: false,
        lead_eligible: true,
        created_at: NOW.to_owned(),
        version: 1,
    };
    store
        .create_agent_binding(AgentBindingCreateRequest {
            request: WorkspaceCreateRequest {
                principal_id: OWNER_ID.to_owned(),
                request_id: "request-agent-binding-create".to_owned(),
                request_payload: serde_json::json!({
                    "operation":"agent.binding.create.v1",
                    "workspace_id":WORKSPACE_ID,
                    "agent_profile_id":PROFILE_ID
                }),
            },
            binding: binding.clone(),
            now: NOW.to_owned(),
            event: draft(
                "event-agent-binding-create",
                WORKSPACE_ID,
                "AgentBinding",
                BINDING_ID,
                1,
                "agent.binding.created.v1",
                serde_json::json!({
                    "agent_binding_id":BINDING_ID,
                    "workspace_id":WORKSPACE_ID,
                    "agent_profile_id":PROFILE_ID,
                    "from_enabled":false,
                    "to_enabled":false,
                    "aggregate_version":1,
                    "requested_by":{"principal_id":OWNER_ID,"kind":"USER"}
                }),
            ),
        })
        .expect("disabled AgentBinding is created");

    let enabled = AgentBindingRecord {
        enabled: true,
        version: 2,
        ..binding.clone()
    };
    store
        .enable_agent_binding(AgentBindingEnableRequest {
            request: WorkspaceCreateRequest {
                principal_id: OWNER_ID.to_owned(),
                request_id: "request-agent-binding-enable".to_owned(),
                request_payload: serde_json::json!({
                    "operation":"agent.binding.enable.v1",
                    "workspace_id":WORKSPACE_ID,
                    "agent_binding_id":BINDING_ID,
                    "expected_version":1
                }),
            },
            workspace_id: WORKSPACE_ID.to_owned(),
            agent_binding_id: BINDING_ID.to_owned(),
            expected_version: 1,
            now: NOW.to_owned(),
            event: draft(
                "event-agent-binding-enable",
                WORKSPACE_ID,
                "AgentBinding",
                BINDING_ID,
                2,
                "agent.binding.changed.v1",
                serde_json::json!({
                    "agent_binding_id":BINDING_ID,
                    "workspace_id":WORKSPACE_ID,
                    "agent_profile_id":PROFILE_ID,
                    "from_enabled":false,
                    "to_enabled":true,
                    "aggregate_version":2,
                    "requested_by":{"principal_id":OWNER_ID,"kind":"USER"}
                }),
            ),
        })
        .expect("AgentBinding is enabled");

    Fixture {
        directory,
        store,
        binding: enabled,
    }
}

fn set_default(fixture: &Fixture, binding_id: Option<&str>, request_id: &str, minute: u8) {
    let workspace = fixture
        .store
        .get_workspace(WORKSPACE_ID)
        .expect("Workspace read")
        .expect("Workspace exists");
    let event = event_context(&format!("event-default-{request_id}"), minute);
    WorkspaceService::new(fixture.store.clone())
        .set_default_agent_binding_idempotent(SetWorkspaceDefaultAgentBinding {
            workspace_id: WORKSPACE_ID.to_owned(),
            expected_version: workspace.version,
            agent_binding_id: binding_id.map(str::to_owned),
            principal_id: OWNER_ID.to_owned(),
            request_id: request_id.to_owned(),
            request_payload: serde_json::json!({
                "operation":"workspace.default_agent_binding.change.v1",
                "workspace_id":WORKSPACE_ID,
                "agent_binding_id":binding_id,
                "expected_version":workspace.version
            }),
            event,
        })
        .expect("Workspace default binding transition commits");
}

fn disable_request(
    binding: &AgentBindingRecord,
    expected_version: u64,
    request_id: &str,
) -> AgentBindingDisableRequest {
    let next_version = expected_version + 1;
    let mut payload = serde_json::json!({
        "agent_binding_id":binding.agent_binding_id,
        "workspace_id":binding.workspace_id,
        "agent_profile_id":binding.agent_profile_id,
        "from_enabled":true,
        "to_enabled":false,
        "aggregate_version":next_version,
        "requested_by":{"principal_id":OWNER_ID,"kind":"USER"}
    });
    if let Some(runtime_id) = binding.runtime_id.as_deref() {
        payload["runtime_id"] = Value::String(runtime_id.to_owned());
    }
    AgentBindingDisableRequest {
        request: WorkspaceCreateRequest {
            principal_id: OWNER_ID.to_owned(),
            request_id: request_id.to_owned(),
            request_payload: serde_json::json!({
                "operation":"agent.binding.disable.v1",
                "workspace_id":binding.workspace_id,
                "agent_binding_id":binding.agent_binding_id,
                "expected_version":expected_version
            }),
        },
        workspace_id: binding.workspace_id.clone(),
        agent_binding_id: binding.agent_binding_id.clone(),
        expected_version,
        now: NOW.to_owned(),
        event: draft(
            &format!("event-{request_id}"),
            &binding.workspace_id,
            "AgentBinding",
            &binding.agent_binding_id,
            next_version,
            "agent.binding.changed.v1",
            payload,
        ),
    }
}

fn count_files(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    fs::read_dir(path)
        .expect("read generated directory")
        .map(|entry| {
            let entry = entry.expect("read generated entry");
            if entry.path().is_dir() {
                count_files(&entry.path())
            } else {
                1
            }
        })
        .sum()
}

fn durable_counts(directory: &tempfile::TempDir) -> (i64, i64, usize) {
    let connection = Connection::open(database_path(directory)).expect("inspect SQLite DB");
    let events = connection
        .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
        .expect("count events");
    let receipts = connection
        .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| row.get(0))
        .expect("count receipts");
    (
        events,
        receipts,
        count_files(&directory.path().join("blobs")),
    )
}

fn agent_binding_audit_count(directory: &tempfile::TempDir, action: &str) -> i64 {
    Connection::open(database_path(directory))
        .expect("open audit DB")
        .query_row(
            "SELECT COUNT(*) FROM audit_records WHERE workspace_id = ?1 AND action = ?2",
            params![WORKSPACE_ID, action],
            |row| row.get(0),
        )
        .expect("count AgentBinding AuditRecords")
}

fn publish_test_offer(fixture: &Fixture, endpoint_id: &str, readiness: &str) {
    fixture
        .store
        .publish_runtime_offer(RuntimeOfferRecord {
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            offer_kind: "AGENT_ENDPOINT".to_owned(),
            offer_ref: endpoint_id.to_owned(),
            compatible: true,
            readiness: readiness.to_owned(),
            constraints: serde_json::json!({"test":true}),
            observed_at: NOW.to_owned(),
            expires_at: FUTURE.to_owned(),
        })
        .expect("Runtime endpoint offer is stored");
}

fn enable_request(
    binding: &AgentBindingRecord,
    expected_version: u64,
    request_id: &str,
) -> AgentBindingEnableRequest {
    let next_version = expected_version + 1;
    AgentBindingEnableRequest {
        request: WorkspaceCreateRequest {
            principal_id: OWNER_ID.to_owned(),
            request_id: request_id.to_owned(),
            request_payload: serde_json::json!({
                "operation":"agent.binding.enable.v1",
                "workspace_id":binding.workspace_id,
                "agent_binding_id":binding.agent_binding_id,
                "expected_version":expected_version
            }),
        },
        workspace_id: binding.workspace_id.clone(),
        agent_binding_id: binding.agent_binding_id.clone(),
        expected_version,
        now: NOW.to_owned(),
        event: draft(
            &format!("event-{request_id}"),
            &binding.workspace_id,
            "AgentBinding",
            &binding.agent_binding_id,
            next_version,
            "agent.binding.changed.v1",
            serde_json::json!({
                "agent_binding_id":binding.agent_binding_id,
                "workspace_id":binding.workspace_id,
                "agent_profile_id":binding.agent_profile_id,
                "from_enabled":false,
                "to_enabled":true,
                "aggregate_version":next_version,
                "requested_by":{"principal_id":OWNER_ID,"kind":"USER"}
            }),
        ),
    }
}

fn register_candidate_endpoint(fixture: &Fixture, suffix: &str) -> (String, String) {
    let profile_id = format!("profile-agent-binding-{suffix}");
    let endpoint_id = format!("endpoint-agent-binding-{suffix}");
    fixture
        .store
        .put_agent_profile(
            AgentProfileRecord {
                agent_profile_id: profile_id.clone(),
                provider_key: "test-provider".to_owned(),
                display_name: "Test Agent Candidate".to_owned(),
                discovered_at: NOW.to_owned(),
            },
            vec![AgentEndpointRecord {
                endpoint_id: endpoint_id.clone(),
                agent_profile_id: profile_id.clone(),
                protocol: "ACP".to_owned(),
                topology: "PROCESS_ADAPTER".to_owned(),
                protocol_version: Some("1".to_owned()),
                capabilities: serde_json::json!({"streaming":true}),
            }],
        )
        .expect("candidate AgentProfile and endpoint are stored");
    fixture
        .store
        .register_local_endpoint_binding(LocalAgentEndpointBindingInput {
            endpoint_id: endpoint_id.clone(),
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
            endpoint_ref: format!("ipc:///private/{endpoint_id}"),
            observed_at: NOW.to_owned(),
            expires_at: Some(FUTURE.to_owned()),
        })
        .expect("candidate Runtime-local endpoint binding is stored");
    (profile_id, endpoint_id)
}

#[test]
fn enable_transition_records_an_append_only_owner_audit() {
    let fixture = setup();
    let connection = Connection::open(database_path(&fixture.directory)).expect("open audit DB");
    let (principal, resource, decision, reason, correlation_id, occurred_at, payload_digest): (
        String, String, String, String, String, String, String,
    ) = connection
        .query_row(
            "SELECT principal_json, resource_ref_json, decision, reason_code, correlation_id, occurred_at, payload_digest FROM audit_records WHERE workspace_id = ?1 AND action = 'agent.binding.enable'",
            [WORKSPACE_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
        )
        .expect("successful enable has a durable AuditRecord");

    assert_eq!(
        serde_json::from_str::<Value>(&principal).expect("principal JSON"),
        serde_json::json!({"principal_id":OWNER_ID,"kind":"USER"})
    );
    assert_eq!(
        serde_json::from_str::<Value>(&resource).expect("resource JSON"),
        serde_json::json!({"kind":"AGENT_BINDING","workspace_id":WORKSPACE_ID,"agent_binding_id":BINDING_ID,"agent_profile_id":PROFILE_ID})
    );
    assert_eq!(decision, "ALLOW");
    assert_eq!(reason, "OWNER_REQUESTED_AGENT_BINDING_ENABLE");
    assert_eq!(correlation_id, "correlation-event-agent-binding-enable");
    assert_eq!(occurred_at, NOW);
    assert!(payload_digest.starts_with("sha256:"));
}

#[test]
fn disabling_current_default_returns_stable_conflict_without_writes() {
    let fixture = setup();
    set_default(&fixture, Some(BINDING_ID), "request-default-set", 3);
    let before_counts = durable_counts(&fixture.directory);
    let before_binding = fixture
        .store
        .get_agent_binding(OWNER_ID, WORKSPACE_ID, BINDING_ID)
        .expect("binding read")
        .expect("binding exists");

    let error = fixture
        .store
        .disable_agent_binding(disable_request(
            &fixture.binding,
            2,
            "request-disable-default",
        ))
        .expect_err("current Workspace default cannot be disabled");

    assert_eq!(
        error,
        StoreError::Conflict {
            expected: None,
            actual: None
        }
    );
    assert_eq!(durable_counts(&fixture.directory), before_counts);
    assert_eq!(
        fixture
            .store
            .get_agent_binding(OWNER_ID, WORKSPACE_ID, BINDING_ID)
            .expect("binding read")
            .expect("binding exists"),
        before_binding,
    );
    assert_eq!(
        agent_binding_audit_count(&fixture.directory, "agent.binding.disable"),
        0
    );
}

#[test]
fn disable_receipt_replays_exactly_rejects_payload_reuse_and_checks_stale_version() {
    let fixture = setup();
    let before = durable_counts(&fixture.directory);
    let stale = fixture
        .store
        .disable_agent_binding(disable_request(
            &fixture.binding,
            1,
            "request-disable-stale",
        ))
        .expect_err("stale binding version must fail");
    assert_eq!(
        stale,
        StoreError::Conflict {
            expected: Some(1),
            actual: Some(2)
        }
    );
    assert_eq!(durable_counts(&fixture.directory), before);

    let request = disable_request(&fixture.binding, 2, "request-disable-replay");
    let committed = fixture
        .store
        .disable_agent_binding(request.clone())
        .expect("disable commits");
    let after_commit = durable_counts(&fixture.directory);
    let audit_record: (String, String, String, String, String, String, String) = Connection::open(database_path(&fixture.directory))
        .expect("open audit database")
        .query_row(
            "SELECT principal_json, resource_ref_json, action, decision, reason_code, correlation_id, occurred_at FROM audit_records WHERE workspace_id = ?1 AND action = 'agent.binding.disable'",
            [WORKSPACE_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
        )
        .expect("successful disable commits a security AuditRecord");
    assert_eq!(
        serde_json::from_str::<Value>(&audit_record.0).expect("principal JSON"),
        serde_json::json!({"principal_id":OWNER_ID,"kind":"USER"})
    );
    assert_eq!(
        serde_json::from_str::<Value>(&audit_record.1).expect("resource JSON"),
        serde_json::json!({"kind":"AGENT_BINDING","workspace_id":WORKSPACE_ID,"agent_binding_id":BINDING_ID,"agent_profile_id":PROFILE_ID})
    );
    assert_eq!(audit_record.2, "agent.binding.disable");
    assert_eq!(audit_record.3, "ALLOW");
    assert_eq!(audit_record.4, "OWNER_REQUESTED_AGENT_BINDING_DISABLE");
    assert_eq!(audit_record.5, request.event.correlation_id);
    assert_eq!(audit_record.6, NOW);
    assert_eq!(
        agent_binding_audit_count(&fixture.directory, "agent.binding.disable"),
        1
    );
    let replay = fixture
        .store
        .disable_agent_binding(request.clone())
        .expect("exact request replays before state checks");
    assert_eq!(replay, committed);
    assert_eq!(durable_counts(&fixture.directory), after_commit);
    let audit_count = agent_binding_audit_count(&fixture.directory, "agent.binding.disable");
    assert_eq!(
        audit_count, 1,
        "an exact idempotent replay does not duplicate audit"
    );

    let mut altered = request;
    altered.request.request_payload["reason"] = Value::String("changed payload".to_owned());
    let conflict = fixture
        .store
        .disable_agent_binding(altered)
        .expect_err("same key with changed payload must conflict");
    assert_eq!(
        conflict,
        StoreError::Conflict {
            expected: None,
            actual: None
        }
    );

    let stored = fixture
        .store
        .get_agent_binding(OWNER_ID, WORKSPACE_ID, BINDING_ID)
        .expect("binding read")
        .expect("binding remains in history");
    assert!(!stored.enabled);
    assert!(stored.lead_eligible);
    assert_eq!(stored.configuration, fixture.binding.configuration);
    assert_eq!(stored.version, 3);
    assert_eq!(after_commit.0, before.0 + 1, "one domain event commits");
    assert_eq!(after_commit.1, before.1 + 1, "one receipt commits");
}

#[test]
fn failed_event_insert_rolls_back_the_binding_change_audit_and_receipt() {
    let fixture = setup();
    let request_id = "request-disable-event-conflict";
    let mut request = disable_request(&fixture.binding, 2, request_id);
    request.event.event_id = "event-agent-binding-create".to_owned();
    let before = durable_counts(&fixture.directory);

    assert!(fixture.store.disable_agent_binding(request).is_err());
    assert_eq!(
        durable_counts(&fixture.directory),
        before,
        "a rejected AgentBinding transition must not leave an unreferenced state blob"
    );
    let stored = fixture
        .store
        .get_agent_binding(OWNER_ID, WORKSPACE_ID, BINDING_ID)
        .expect("binding read after rollback")
        .expect("binding remains stored");
    assert!(
        stored.enabled,
        "the failed event insert rolls back the binding update"
    );
    assert_eq!(stored.version, 2);
    assert_eq!(
        agent_binding_audit_count(&fixture.directory, "agent.binding.disable"),
        0
    );
    assert_eq!(
        agent_binding_audit_count(&fixture.directory, "agent.binding.enable"),
        1
    );
    let connection = Connection::open(database_path(&fixture.directory)).expect("open receipt DB");
    let receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            params![OWNER_ID, request_id],
            |row| row.get(0),
        )
        .expect("count failed-transition receipts");
    assert_eq!(
        receipts, 0,
        "the failed event insert also rolls back the receipt"
    );
}

#[test]
fn duplicate_agent_binding_id_rejection_does_not_write_aggregate_state() {
    let fixture = setup();
    let mut binding = AgentBindingRecord {
        enabled: false,
        version: 1,
        lead_eligible: false,
        ..fixture.binding.clone()
    };
    binding.created_at = "2026-10-15T10:03:00Z".to_owned();
    let request_id = "request-duplicate-agent-binding-id";
    let request = AgentBindingCreateRequest {
        request: WorkspaceCreateRequest {
            principal_id: OWNER_ID.to_owned(),
            request_id: request_id.to_owned(),
            request_payload: serde_json::json!({
                "operation":"agent.binding.create.v1",
                "workspace_id":WORKSPACE_ID,
                "agent_profile_id":PROFILE_ID,
                "lead_eligible":false
            }),
        },
        binding: binding.clone(),
        now: NOW.to_owned(),
        event: draft(
            "event-duplicate-agent-binding-id",
            WORKSPACE_ID,
            "AgentBinding",
            BINDING_ID,
            1,
            "agent.binding.created.v1",
            agent_binding_created_payload(&binding, OWNER_ID),
        ),
    };
    let before = durable_counts(&fixture.directory);

    assert!(fixture.store.create_agent_binding(request).is_err());

    assert_eq!(
        durable_counts(&fixture.directory),
        before,
        "a duplicate binding identity must fail before publishing aggregate state"
    );
}

#[test]
fn agent_binding_create_rejects_auth_refs_without_writing_any_state() {
    let fixture = setup();
    assert_eq!(fixture.binding.auth_ref, None);
    assert_eq!(
        fixture
            .store
            .get_agent_binding(OWNER_ID, WORKSPACE_ID, BINDING_ID)
            .expect("read existing auth-free binding")
            .expect("None auth_ref creation remains supported")
            .auth_ref,
        None
    );

    let rejected_refs = [
        (
            "well-shaped",
            serde_json::json!({
                "secret_ref_id":"secret-ref-foreign-caller",
                "provider_ref":"opaque-foreign-store-key",
                "placement":"LOCAL_ONLY"
            }),
        ),
        ("malformed", serde_json::json!({"provider_ref":"unowned"})),
    ];

    for (suffix, auth_ref) in rejected_refs {
        let mut binding = AgentBindingRecord {
            agent_binding_id: format!("binding-auth-ref-{suffix}"),
            auth_ref: Some(auth_ref),
            enabled: false,
            version: 1,
            ..fixture.binding.clone()
        };
        binding.created_at = NOW.to_owned();
        let request_id = format!("request-auth-ref-{suffix}");
        let event_id = format!("event-auth-ref-{suffix}");
        let request = AgentBindingCreateRequest {
            request: WorkspaceCreateRequest {
                principal_id: OWNER_ID.to_owned(),
                request_id: request_id.clone(),
                request_payload: serde_json::json!({
                    "operation":"agent.binding.create.v1",
                    "workspace_id":WORKSPACE_ID,
                    "agent_profile_id":PROFILE_ID,
                    "agent_binding_id":binding.agent_binding_id
                }),
            },
            binding: binding.clone(),
            now: NOW.to_owned(),
            event: draft(
                &event_id,
                WORKSPACE_ID,
                "AgentBinding",
                &binding.agent_binding_id,
                1,
                "agent.binding.created.v1",
                serde_json::json!({
                    "agent_binding_id":binding.agent_binding_id,
                    "workspace_id":WORKSPACE_ID,
                    "agent_profile_id":PROFILE_ID,
                    "from_enabled":false,
                    "to_enabled":false,
                    "aggregate_version":1,
                    "requested_by":{"principal_id":OWNER_ID,"kind":"USER"}
                }),
            ),
        };
        let before_counts = durable_counts(&fixture.directory);
        let before_audit = agent_binding_audit_count(&fixture.directory, "agent.binding.create");

        assert!(matches!(
            fixture.store.create_agent_binding(request),
            Err(StoreError::Invalid(_))
        ));

        assert_eq!(durable_counts(&fixture.directory), before_counts);
        assert_eq!(
            agent_binding_audit_count(&fixture.directory, "agent.binding.create"),
            before_audit
        );
        assert_eq!(
            fixture
                .store
                .get_agent_binding(OWNER_ID, WORKSPACE_ID, &binding.agent_binding_id)
                .expect("read rejected binding"),
            None
        );
        let connection = Connection::open(database_path(&fixture.directory))
            .expect("open SQLite DB to inspect rejected request");
        let receipt_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2)",
                params![OWNER_ID, request_id],
                |row| row.get(0),
            )
            .expect("inspect rejected request receipt");
        let event_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM domain_events WHERE event_id = ?1)",
                [&event_id],
                |row| row.get(0),
            )
            .expect("inspect rejected event");
        assert!(!receipt_exists, "rejected create must not leave a receipt");
        assert!(!event_exists, "rejected create must not append an event");
    }
}

#[test]
fn invalid_agent_binding_event_schema_does_not_write_aggregate_state() {
    let fixture = setup();
    let mut request = disable_request(
        &fixture.binding,
        fixture.binding.version,
        "request-disable-invalid-event-schema",
    );
    request.event.schema_version = 2;
    let before = durable_counts(&fixture.directory);

    assert!(matches!(
        fixture.store.disable_agent_binding(request),
        Err(StoreError::Invalid(_))
    ));

    assert_eq!(
        durable_counts(&fixture.directory),
        before,
        "an unsupported event schema must be rejected before publishing aggregate state"
    );
}

fn seed_running_task_and_planner(fixture: &Fixture, session_status: &str) {
    let mut connection = Connection::open(database_path(&fixture.directory)).expect("open DB");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    let tx = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("begin fixture transaction");
    tx.execute_batch("PRAGMA defer_foreign_keys = ON;")
        .expect("defer cyclic Task/spec references");
    tx.execute(
        "UPDATE runtimes SET availability='ONLINE' WHERE runtime_id=?1",
        [RUNTIME_ID],
    )
    .expect("Runtime is online for the admitted session");
    tx.execute(
        "UPDATE runtime_incarnations SET recovery_state='READY' WHERE runtime_id=?1 AND runtime_incarnation_id=?2",
        params![RUNTIME_ID, INCARNATION_ID],
    )
    .expect("Runtime incarnation is ready for the admitted session");
    tx.execute(
        "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,current_plan_revision,status,lead_agent_binding_id,blocking_conditions_json,priority,created_by_json,created_at,updated_at,version)
         VALUES('task-pinned-planner',?1,1,NULL,'RUNNING',?2,'[]','NORMAL',?3,?4,?4,1)",
        params![
            WORKSPACE_ID,
            BINDING_ID,
            serde_json::to_string(&serde_json::json!({"kind":"USER","principal_id":OWNER_ID}))
                .expect("Task actor JSON"),
            NOW,
        ],
    )
    .expect("Task with the admitted lead is present");
    tx.execute(
        "INSERT INTO task_spec_revisions(task_id,workspace_id,revision,objective,lead_failover_policy_json,placement_preference,preferred_lead_agent_binding_id,authored_by_json,created_at)
         VALUES('task-pinned-planner',?1,1,'Keep planning session pinned',?2,'{}',?3,?4,?5)",
        params![
            WORKSPACE_ID,
            r#"{"mode":"DISABLED","triggers":[],"fallback_agent_binding_ids":[],"max_lead_changes":0}"#,
            BINDING_ID,
            serde_json::to_string(&serde_json::json!({"kind":"USER","principal_id":OWNER_ID}))
                .expect("TaskSpec actor JSON"),
            NOW,
        ],
    )
    .expect("TaskSpec revision is present");
    tx.execute(
        "INSERT INTO agent_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,agent_profile_id,endpoint_id,hosting_mode,state,ownership,started_at,last_used_at)
         VALUES('host-pinned-planner',?1,?2,?3,?4,'LOCAL_PER_SESSION','READY','LITECOWORK',?5,?5)",
        params![RUNTIME_ID, INCARNATION_ID, PROFILE_ID, ENDPOINT_ID, NOW],
    )
    .expect("native host is present");
    tx.execute(
        "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at,last_event_at,version)
         VALUES('session-pinned-planner',?1,'TASK_PLANNING','task-pinned-planner',1,?2,?3,?4,?5,?6,?7,?7,1)",
        params![WORKSPACE_ID, BINDING_ID, ENDPOINT_ID, RUNTIME_ID, INCARNATION_ID, session_status, NOW],
    )
    .expect("Task-planning session is pinned");
    if session_status == "ACTIVE" {
        tx.execute(
            "INSERT INTO agent_session_host_bindings(agent_session_id,host_instance_id,bound_at)
             VALUES('session-pinned-planner','host-pinned-planner',?1)",
            [NOW],
        )
        .expect("active session remains attached to its host");
    }
    tx.commit().expect("persist active planner fixture");
}

fn plan_commit() -> (
    PlanAcceptanceCommit,
    AggregateStateRef,
    Vec<AggregateStateRef>,
) {
    let timestamp = NOW.to_owned();
    let plan = PlanRevisionRecord {
        task_id: "task-pinned-planner".to_owned(),
        revision: 1,
        task_spec_revision: 1,
        produced_by_agent_session_id: "session-pinned-planner".to_owned(),
        produced_by_attempt_id: None,
        steps: vec![PlannedStepRecord {
            logical_key: "first".to_owned(),
            title: "First step".to_owned(),
            objective: "A focused initial step".to_owned(),
            depends_on_logical_keys: Vec::new(),
            required_capabilities: Vec::new(),
            acceptance_criteria: Vec::new(),
        }],
        reason_for_revision: None,
        created_at: timestamp.clone(),
    };
    let step = StepRecord {
        step_id: "step-pinned-planner".to_owned(),
        task_id: "task-pinned-planner".to_owned(),
        plan_revision: 1,
        logical_key: Some("first".to_owned()),
        title: "First step".to_owned(),
        objective: "A focused initial step".to_owned(),
        dependencies: Vec::new(),
        required_capabilities: Vec::new(),
        acceptance_criteria: Vec::new(),
        status: "READY".to_owned(),
        current_attempt_id: None,
        created_at: timestamp.clone(),
        updated_at: timestamp.clone(),
        version: 1,
    };
    let mut step_event = draft(
        "event-step-after-binding-change",
        WORKSPACE_ID,
        "Step",
        "step-pinned-planner",
        1,
        "step.created.v1",
        serde_json::json!({
            "step_id":"step-pinned-planner",
            "task_id":"task-pinned-planner",
            "plan_revision":1,
            "logical_key":"first",
            "dependencies":[]
        }),
    );
    step_event.correlation_id = "correlation-event-plan-after-binding-change".to_owned();
    let commit = PlanAcceptanceCommit {
        principal_id: OWNER_ID.to_owned(),
        request_id: "request-plan-after-binding-change".to_owned(),
        request_payload: serde_json::json!({"plan":{"logical_key":"first"}}),
        workspace_id: WORKSPACE_ID.to_owned(),
        task_id: "task-pinned-planner".to_owned(),
        expected_task_version: 1,
        expected_task_spec_revision: 1,
        plan_revision: plan,
        materialized_steps: vec![step],
        plan_event: draft(
            "event-plan-after-binding-change",
            WORKSPACE_ID,
            "Task",
            "task-pinned-planner",
            2,
            "task.plan.revised.v1",
            serde_json::json!({
                "task_id":"task-pinned-planner",
                "revision":1,
                "task_spec_revision":1,
                "produced_by_agent_session_id":"session-pinned-planner",
                "step_ids":["step-pinned-planner"],
                "aggregate_version":2
            }),
        ),
        step_events: vec![step_event],
    };
    let task_state_ref = AggregateStateRef {
        blob: BlobRef {
            digest: format!("sha256:{}", "a".repeat(64)),
            size_bytes: 1,
            media_type: "application/vnd.litecow.task+json".to_owned(),
        },
        entity_revision: 2,
        record_schema_version: 1,
    };
    let step_state_refs = vec![AggregateStateRef {
        blob: BlobRef {
            digest: format!("sha256:{}", "b".repeat(64)),
            size_bytes: 1,
            media_type: "application/vnd.litecow.step+json".to_owned(),
        },
        entity_revision: 1,
        record_schema_version: 1,
    }];
    (commit, task_state_ref, step_state_refs)
}

#[test]
fn admitted_planner_can_commit_after_binding_is_disabled_and_lead_eligibility_changes() {
    let fixture = setup();
    set_default(
        &fixture,
        Some(BINDING_ID),
        "request-default-set-for-plan",
        3,
    );
    seed_running_task_and_planner(&fixture, "ACTIVE");
    set_default(&fixture, None, "request-default-clear-for-plan", 4);
    let disabled = fixture
        .store
        .disable_agent_binding(disable_request(
            &fixture.binding,
            2,
            "request-disable-for-plan",
        ))
        .expect("cleared default binding can be disabled");
    assert!(!disabled.binding.enabled);
    assert!(disabled.binding.lead_eligible);

    let connection = Connection::open(database_path(&fixture.directory)).expect("open SQLite");
    connection
        .execute(
            "UPDATE agent_bindings SET lead_eligible=0,version=version+1 WHERE workspace_id=?1 AND agent_binding_id=?2",
            params![WORKSPACE_ID, BINDING_ID],
        )
        .expect("future lead eligibility can change independently");
    drop(connection);

    let (commit, task_state_ref, step_state_refs) = plan_commit();
    validate_plan_acceptance_commit(&commit).expect("plan commit fixture matches the contract");
    let mut connection = Connection::open(database_path(&fixture.directory)).expect("open SQLite");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    let accepted =
        accept_initial_plan_transaction(&mut connection, commit, task_state_ref, step_state_refs)
            .expect("an already-active planner remains pinned and may settle");

    assert_eq!(
        accepted.plan_revision.produced_by_agent_session_id,
        "session-pinned-planner"
    );
    assert_eq!(accepted.task_version, 2);
    let (session_state, session_binding): (String, String) = connection
        .query_row(
            "SELECT status,agent_binding_id FROM agent_sessions WHERE agent_session_id='session-pinned-planner'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read pinned session");
    assert_eq!(session_state, "ACTIVE");
    assert_eq!(session_binding, BINDING_ID);
}

#[test]
fn durable_starting_planner_can_activate_after_its_binding_is_disabled() {
    let fixture = setup();
    seed_running_task_and_planner(&fixture, "STARTING");

    let disabled = fixture
        .store
        .disable_agent_binding(disable_request(
            &fixture.binding,
            2,
            "request-disable-during-planner-startup",
        ))
        .expect("disable commits after the durable planner reservation");
    assert!(!disabled.binding.enabled);

    let mut connection = Connection::open(database_path(&fixture.directory)).expect("open DB");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    connection
        .execute(
            "UPDATE agent_bindings SET lead_eligible=0, version=version+1 WHERE workspace_id=?1 AND agent_binding_id=?2",
            params![WORKSPACE_ID, BINDING_ID],
        )
        .expect("future lead eligibility can change after reservation");
    let mut session = load_agent_session(&connection, WORKSPACE_ID, "session-pinned-planner")
        .expect("read reserved planner")
        .expect("reserved planner exists");
    session.status = "ACTIVE".to_owned();
    session.last_event_at = Some(NOW.to_owned());
    session.version = 2;
    let task = load_task_view(&connection, WORKSPACE_ID, "task-pinned-planner")
        .expect("read pinned Task")
        .expect("Task exists");
    let state_ref = AggregateStateRef {
        blob: BlobRef {
            digest: format!("sha256:{}", "c".repeat(64)),
            size_bytes: 1,
            media_type: "application/vnd.litecow.agent-session+json".to_owned(),
        },
        entity_revision: 2,
        record_schema_version: 1,
    };
    let session_event = draft(
        "event-start-pinned-planner-after-disable",
        WORKSPACE_ID,
        "AgentSession",
        "session-pinned-planner",
        2,
        "agent.session.started.v1",
        serde_json::json!({
            "agent_session_id":"session-pinned-planner",
            "scope":{"kind":"TASK_PLANNING","task_id":"task-pinned-planner"},
            "agent_binding_id":BINDING_ID,
            "endpoint_id":ENDPOINT_ID,
            "runtime_id":RUNTIME_ID,
            "runtime_incarnation_id":INCARNATION_ID,
            "task_spec_revision":1,
            "session_state":"ACTIVE",
            "reported_at":NOW,
        }),
    );
    let activated = activate_task_planning_session_transaction(
        &mut connection,
        ActivateTaskPlanningSession {
            workspace_id: WORKSPACE_ID.to_owned(),
            agent_session_id: "session-pinned-planner".to_owned(),
            expected_session_version: 1,
            expected_task_version: 1,
            occurred_at: NOW.to_owned(),
            host_instance_id: "host-pinned-planner".to_owned(),
            native_session_ref: None,
            session_event,
            task_status_event: None,
        },
        session,
        task,
        state_ref,
        None,
    )
    .expect("a committed STARTING reservation remains pinned through activation");

    assert_eq!(activated.session.status, "ACTIVE");
    assert_eq!(activated.session.agent_binding_id, BINDING_ID);
    let current_binding = fixture
        .store
        .get_agent_binding(OWNER_ID, WORKSPACE_ID, BINDING_ID)
        .expect("read binding after activation")
        .expect("binding remains available in active Workspace");
    assert!(
        !current_binding.enabled,
        "activation does not re-enable the binding"
    );
}

#[test]
fn nonusable_compatible_offer_cannot_create_an_agent_binding() {
    let fixture = setup();

    for readiness in ["NEEDS_AUTH", "UNAVAILABLE"] {
        let suffix = readiness.to_ascii_lowercase();
        let (profile_id, endpoint_id) = register_candidate_endpoint(&fixture, &suffix);
        publish_test_offer(&fixture, &endpoint_id, readiness);

        let binding = AgentBindingRecord {
            agent_binding_id: format!("binding-agent-binding-{suffix}"),
            workspace_id: WORKSPACE_ID.to_owned(),
            agent_profile_id: profile_id.clone(),
            runtime_id: None,
            endpoint_selection_policy: serde_json::json!({
                "mode":"AUTO_COMPATIBLE",
                "required_features":[],
                "preferred_topologies":["PROCESS_ADAPTER"]
            }),
            auth_ref: None,
            configuration: serde_json::json!({}),
            enabled: false,
            lead_eligible: true,
            created_at: NOW.to_owned(),
            version: 1,
        };
        let before = durable_counts(&fixture.directory);
        let audit_before = agent_binding_audit_count(&fixture.directory, "agent.binding.create");
        let request_id = format!("request-create-nonusable-{suffix}");

        let result = fixture
            .store
            .create_agent_binding(AgentBindingCreateRequest {
                request: WorkspaceCreateRequest {
                    principal_id: OWNER_ID.to_owned(),
                    request_id: request_id.clone(),
                    request_payload: serde_json::json!({
                        "operation":"agent.binding.create.v1",
                        "workspace_id":WORKSPACE_ID,
                        "agent_profile_id":profile_id
                    }),
                },
                binding: binding.clone(),
                now: NOW.to_owned(),
                event: draft(
                    &format!("event-{request_id}"),
                    WORKSPACE_ID,
                    "AgentBinding",
                    &binding.agent_binding_id,
                    1,
                    "agent.binding.created.v1",
                    serde_json::json!({
                        "agent_binding_id":binding.agent_binding_id,
                        "workspace_id":WORKSPACE_ID,
                        "agent_profile_id":binding.agent_profile_id,
                        "from_enabled":false,
                        "to_enabled":false,
                        "aggregate_version":1,
                        "requested_by":{"principal_id":OWNER_ID,"kind":"USER"}
                    }),
                ),
            });

        assert!(matches!(
            result,
            Err(StoreError::Invalid(message))
                if message.contains("no fresh compatible endpoint offer")
        ));
        assert_eq!(durable_counts(&fixture.directory), before);
        assert_eq!(
            agent_binding_audit_count(&fixture.directory, "agent.binding.create"),
            audit_before
        );
        assert_eq!(
            fixture
                .store
                .get_agent_binding(OWNER_ID, WORKSPACE_ID, &binding.agent_binding_id)
                .expect("check rejected binding"),
            None
        );
    }
}

#[test]
fn nonusable_compatible_offer_cannot_enable_an_agent_binding() {
    let fixture = setup();
    let disabled = fixture
        .store
        .disable_agent_binding(disable_request(
            &fixture.binding,
            fixture.binding.version,
            "request-disable-before-nonusable-offer",
        ))
        .expect("clear the current enablement state")
        .binding;

    for readiness in ["NEEDS_AUTH", "UNAVAILABLE"] {
        publish_test_offer(&fixture, ENDPOINT_ID, readiness);
        let before = durable_counts(&fixture.directory);
        let audit_before = agent_binding_audit_count(&fixture.directory, "agent.binding.enable");
        let request_id = format!(
            "request-enable-nonusable-{}",
            readiness.to_ascii_lowercase()
        );

        let result = fixture.store.enable_agent_binding(enable_request(
            &disabled,
            disabled.version,
            &request_id,
        ));

        assert!(matches!(
            result,
            Err(StoreError::Invalid(message))
                if message.contains("no fresh compatible endpoint offer")
        ));
        assert_eq!(durable_counts(&fixture.directory), before);
        assert_eq!(
            agent_binding_audit_count(&fixture.directory, "agent.binding.enable"),
            audit_before
        );
        assert_eq!(
            fixture
                .store
                .get_agent_binding(OWNER_ID, WORKSPACE_ID, BINDING_ID)
                .expect("read unchanged binding"),
            Some(disabled.clone())
        );
    }
}

#[test]
fn enable_rechecks_offer_after_blob_publication_before_sql_commit() {
    let fixture = setup();
    let disabled = fixture
        .store
        .disable_agent_binding(disable_request(
            &fixture.binding,
            fixture.binding.version,
            "request-disable-before-offer-race",
        ))
        .expect("binding is disabled before the race")
        .binding;
    let before = durable_counts(&fixture.directory);
    let audit_before = agent_binding_audit_count(&fixture.directory, "agent.binding.enable");
    let (published_tx, published_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let gated_store = SqliteWorkspaceStore::open(
        database_path(&fixture.directory),
        Arc::new(PausingAgentBindingBlobStore {
            inner: FileBlobStore::new(
                fixture.directory.path().join("blobs"),
                TestKeys([19_u8; 32]),
            ),
            published: published_tx,
            resume_put: Mutex::new(resume_rx),
        }),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("gated SQLite store opens");

    let request = enable_request(&disabled, disabled.version, "request-enable-offer-race");
    let writer = gated_store.clone();
    let result = std::thread::spawn(move || writer.enable_agent_binding(request));

    published_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("state blob is published before the SQL transaction starts");
    let readiness_update = Connection::open(database_path(&fixture.directory))
        .map_err(|error| error.to_string())
        .and_then(|concurrent| {
            concurrent
                .busy_timeout(Duration::from_secs(2))
                .map_err(|error| error.to_string())?;
            concurrent
                .execute(
                    "UPDATE runtime_offers SET readiness = 'NEEDS_AUTH' WHERE runtime_id = ?1 AND runtime_incarnation_id = ?2 AND offer_kind = 'AGENT_ENDPOINT' AND offer_ref = ?3",
                    params![RUNTIME_ID, INCARNATION_ID, ENDPOINT_ID],
                )
                .map_err(|error| error.to_string())
        });
    let release_result = resume_tx.send(());
    assert!(release_result.is_ok(), "release the BlobStore writer");
    let operation_result = result.join().expect("AgentBinding writer thread exits");
    assert_eq!(
        readiness_update.expect("offer update commits without a SQL writer lock"),
        1
    );

    assert!(matches!(
        operation_result,
        Err(StoreError::Invalid(message))
            if message.contains("no fresh compatible endpoint offer")
    ));

    let after = durable_counts(&fixture.directory);
    assert_eq!(after.0, before.0, "the rejected enable emits no event");
    assert_eq!(after.1, before.1, "the rejected enable stores no receipt");
    assert_eq!(
        after.2,
        before.2 + 1,
        "the already-published immutable candidate may await delayed garbage collection"
    );
    assert_eq!(
        agent_binding_audit_count(&fixture.directory, "agent.binding.enable"),
        audit_before,
        "the rejected enable adds no security audit record"
    );
    assert_eq!(
        fixture
            .store
            .get_agent_binding(OWNER_ID, WORKSPACE_ID, BINDING_ID)
            .expect("read unchanged binding"),
        Some(disabled),
        "the authoritative transaction recheck preserves the disabled binding"
    );
}

#[test]
fn conversation_host_resolution_uses_turn_pinned_binding_after_selection_changes() {
    use storage_core::conversation::{
        CommittedConversationTurn, ConversationMessageRecord, ConversationRecord,
        ConversationStore, ConversationTurn, ConversationTurnStatus, ConversationTurnStore,
        CreateConversationCommit, CreateConversationTurnCommit, PresentationPreference,
        ResolvedConversationHost, SetConversationAgentBinding,
    };

    let fixture = setup();
    let connection =
        Connection::open(database_path(&fixture.directory)).expect("open host-resolution fixture");
    connection
        .execute(
            "UPDATE runtimes SET availability='ONLINE' WHERE runtime_id=?1",
            [RUNTIME_ID],
        )
        .expect("Runtime is online");
    connection
        .execute(
            "UPDATE runtime_incarnations SET recovery_state='READY' WHERE runtime_id=?1 AND runtime_incarnation_id=?2",
            params![RUNTIME_ID, INCARNATION_ID],
        )
        .expect("Runtime incarnation is ready");
    drop(connection);

    let conversation_id = "conversation-host-resolution";
    let conversation_store = crate::SqliteConversationStore::new(fixture.store.clone());
    let conversation = ConversationRecord {
        conversation_id: conversation_id.to_owned(),
        workspace_id: WORKSPACE_ID.to_owned(),
        title: None,
        active_agent_binding_id: None,
        version: 1,
        created_at: NOW.to_owned(),
    };
    conversation_store
        .create_conversation(CreateConversationCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER_ID.to_owned(),
                request_id: "request-create-host-resolution".to_owned(),
                request_payload: serde_json::json!({
                    "operation":"conversation.create.v1",
                    "workspace_id":WORKSPACE_ID,
                    "title":null
                }),
            },
            conversation: conversation.clone(),
            event: draft(
                "event-create-host-resolution",
                WORKSPACE_ID,
                "Conversation",
                conversation_id,
                1,
                "conversation.created.v1",
                serde_json::json!({
                    "conversation_id":conversation_id,
                    "created_by":{"kind":"USER","principal_id":OWNER_ID}
                }),
            ),
        })
        .expect("Conversation is created");

    conversation_store
        .set_conversation_agent_binding_idempotent(SetConversationAgentBinding {
            request: WorkspaceCreateRequest {
                principal_id: OWNER_ID.to_owned(),
                request_id: "request-select-original-agent".to_owned(),
                request_payload: serde_json::json!({
                    "operation":"conversation.agent_binding.set.v1",
                    "workspace_id":WORKSPACE_ID,
                    "conversation_id":conversation_id,
                    "expected_version":1,
                    "agent_binding_id":BINDING_ID
                }),
            },
            principal_id: OWNER_ID.to_owned(),
            workspace_id: WORKSPACE_ID.to_owned(),
            conversation_id: conversation_id.to_owned(),
            expected_version: 1,
            agent_binding_id: Some(BINDING_ID.to_owned()),
            event: draft(
                "event-select-original-agent",
                WORKSPACE_ID,
                "Conversation",
                conversation_id,
                2,
                "conversation.agent_binding.changed.v1",
                serde_json::json!({}),
            ),
        })
        .expect("original AgentBinding is selected");

    let message_id = "message-host-resolution";
    let turn_id = "turn-host-resolution";
    let author = serde_json::json!({"kind":"USER","principal_id":OWNER_ID});
    let content = serde_json::json!([{"kind":"TEXT","text":"Keep this turn on agent A."}]);
    let resource_refs = serde_json::json!([]);
    let presentation_preference = PresentationPreference::Auto;
    let message = ConversationMessageRecord {
        message_id: message_id.to_owned(),
        conversation_id: conversation_id.to_owned(),
        author: author.clone(),
        role: "USER".to_owned(),
        agent_session_id: None,
        agent_binding_id: None,
        turn_id: Some(turn_id.to_owned()),
        content: content.clone(),
        resource_refs: resource_refs.clone(),
        source_channel_ref: None,
        created_at: NOW.to_owned(),
    };
    let turn = ConversationTurn {
        turn_id: turn_id.to_owned(),
        conversation_id: conversation_id.to_owned(),
        user_message_id: message_id.to_owned(),
        agent_session_id: None,
        status: ConversationTurnStatus::Open,
        retry_ordinal: 0,
        presentation_preference,
        created_at: NOW.to_owned(),
        settled_at: None,
        version: 1,
    };
    let message_digest = digest(&canonical_json(&content).expect("canonical message content"));
    let committed: CommittedConversationTurn = conversation_store
        .create_conversation_turn(CreateConversationTurnCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER_ID.to_owned(),
                request_id: "request-turn-host-resolution".to_owned(),
                request_payload: serde_json::json!({
                    "operation":"conversation.turn.create.v1",
                    "workspace_id":WORKSPACE_ID,
                    "conversation_id":conversation_id,
                    "turn_id":turn_id,
                    "message_id":message_id,
                    "author":author,
                    "content":content,
                    "resource_refs":resource_refs,
                    "presentation_preference":"AUTO"
                }),
            },
            principal_id: OWNER_ID.to_owned(),
            workspace_id: WORKSPACE_ID.to_owned(),
            message,
            turn,
            message_event: draft(
                "event-message-host-resolution",
                WORKSPACE_ID,
                "Conversation",
                conversation_id,
                3,
                "conversation.message.added.v1",
                serde_json::json!({
                    "message_id":message_id,
                    "conversation_id":conversation_id,
                    "author":{"kind":"USER","principal_id":OWNER_ID},
                    "content_digest":message_digest,
                    "resource_refs":[]
                }),
            ),
            turn_event: draft(
                "event-turn-host-resolution",
                WORKSPACE_ID,
                "ConversationTurn",
                turn_id,
                1,
                "conversation.turn.created.v2",
                serde_json::json!({
                    "turn_id":turn_id,
                    "conversation_id":conversation_id,
                    "user_message_id":message_id,
                    "agent_binding_id":BINDING_ID,
                    "aggregate_version":1,
                    "presentation_preference":"AUTO"
                }),
            ),
        })
        .expect("turn is committed");
    assert_eq!(committed.turn.status, ConversationTurnStatus::Open);

    let connection =
        Connection::open(database_path(&fixture.directory)).expect("seed newly selected binding");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES('profile-next-host','test-provider','Next Agent',?1)",
            [NOW],
        )
        .expect("next agent profile exists");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,runtime_id,endpoint_selection_policy_json,auth_ref,configuration_json,enabled,lead_eligible,created_at,version) VALUES('binding-next-host',?1,'profile-next-host',NULL,'{}',NULL,'{}',1,1,?2,1)",
            params![WORKSPACE_ID, NOW],
        )
        .expect("next binding is enabled but has no eligible endpoint");
    drop(connection);

    conversation_store
        .set_conversation_agent_binding_idempotent(SetConversationAgentBinding {
            request: WorkspaceCreateRequest {
                principal_id: OWNER_ID.to_owned(),
                request_id: "request-switch-host-resolution".to_owned(),
                request_payload: serde_json::json!({
                    "operation":"conversation.agent_binding.set.v1",
                    "workspace_id":WORKSPACE_ID,
                    "conversation_id":conversation_id,
                    "expected_version":3,
                    "agent_binding_id":"binding-next-host"
                }),
            },
            principal_id: OWNER_ID.to_owned(),
            workspace_id: WORKSPACE_ID.to_owned(),
            conversation_id: conversation_id.to_owned(),
            expected_version: 3,
            agent_binding_id: Some("binding-next-host".to_owned()),
            event: draft(
                "event-switch-host-resolution",
                WORKSPACE_ID,
                "Conversation",
                conversation_id,
                4,
                "conversation.agent_binding.changed.v1",
                serde_json::json!({}),
            ),
        })
        .expect("Conversation selects agent B after turn A committed");

    let host = conversation_store
        .resolve_conversation_session_host_for_turn(
            OWNER_ID,
            WORKSPACE_ID,
            conversation_id,
            turn_id,
        )
        .expect("resolve the turn-pinned host");
    assert_eq!(
        host,
        Some(ResolvedConversationHost {
            agent_binding_id: BINDING_ID.to_owned(),
            endpoint_id: ENDPOINT_ID.to_owned(),
            runtime_id: RUNTIME_ID.to_owned(),
            runtime_incarnation_id: INCARNATION_ID.to_owned(),
        }),
        "a committed turn must keep agent A even when the live selector now points to B"
    );

    assert!(matches!(
        conversation_store.resolve_conversation_session_host_for_turn(
            "different-owner",
            WORKSPACE_ID,
            conversation_id,
            turn_id
        ),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        conversation_store.resolve_conversation_session_host_for_turn(
            OWNER_ID,
            WORKSPACE_ID,
            "different-conversation",
            turn_id
        ),
        Err(StoreError::NotFound)
    ));

    let connection =
        Connection::open(database_path(&fixture.directory)).expect("expire pinned endpoint");
    connection
        .execute(
            "UPDATE agent_endpoint_bindings SET expires_at='2000-01-01T00:00:00Z' WHERE endpoint_id=?1 AND runtime_incarnation_id=?2",
            params![ENDPOINT_ID, INCARNATION_ID],
        )
        .expect("pinned endpoint binding expires");
    drop(connection);
    assert_eq!(
        conversation_store
            .resolve_conversation_session_host_for_turn(
                OWNER_ID,
                WORKSPACE_ID,
                conversation_id,
                turn_id,
            )
            .expect("expired endpoint yields no host"),
        None,
        "an expired endpoint must not be replaced by the newly selected agent B"
    );

    let connection =
        Connection::open(database_path(&fixture.directory)).expect("take Runtime offline");
    connection
        .execute(
            "UPDATE agent_endpoint_bindings SET expires_at=?1 WHERE endpoint_id=?2 AND runtime_incarnation_id=?3",
            params![FUTURE, ENDPOINT_ID, INCARNATION_ID],
        )
        .expect("endpoint binding is refreshed");
    connection
        .execute(
            "UPDATE runtimes SET availability='OFFLINE' WHERE runtime_id=?1",
            [RUNTIME_ID],
        )
        .expect("pinned Runtime is offline");
    drop(connection);
    assert_eq!(
        conversation_store
            .resolve_conversation_session_host_for_turn(
                OWNER_ID,
                WORKSPACE_ID,
                conversation_id,
                turn_id,
            )
            .expect("offline Runtime yields no host"),
        None,
        "an offline pinned Runtime must not be replaced by the newly selected agent B"
    );

    let connection =
        Connection::open(database_path(&fixture.directory)).expect("restore pinned host");
    connection
        .execute(
            "UPDATE runtimes SET availability='ONLINE' WHERE runtime_id=?1",
            [RUNTIME_ID],
        )
        .expect("pinned Runtime is online again");
    drop(connection);

    let connection =
        Connection::open(database_path(&fixture.directory)).expect("disable pinned binding");
    connection
        .execute(
            "UPDATE agent_bindings SET enabled=0 WHERE workspace_id=?1 AND agent_binding_id=?2",
            params![WORKSPACE_ID, BINDING_ID],
        )
        .expect("pinned binding is disabled");
    drop(connection);
    assert_eq!(
        conversation_store
            .resolve_conversation_session_host_for_turn(
                OWNER_ID,
                WORKSPACE_ID,
                conversation_id,
                turn_id,
            )
            .expect("disabled pinned binding yields no host"),
        None,
        "an unavailable pinned agent must not fall back to the newly selected agent B"
    );

    let connection =
        Connection::open(database_path(&fixture.directory)).expect("corrupt turn creation event");
    connection
        .execute(
            "UPDATE domain_events SET payload_json='{}' WHERE event_id='event-turn-host-resolution'",
            [],
        )
        .expect("turn creation event payload is corrupted");
    drop(connection);
    assert!(matches!(
        conversation_store.resolve_conversation_session_host_for_turn(
            OWNER_ID,
            WORKSPACE_ID,
            conversation_id,
            turn_id,
        ),
        Err(StoreError::Integrity(_))
    ));

    let connection =
        Connection::open(database_path(&fixture.directory)).expect("remove turn creation event");
    connection
        .execute(
            "DELETE FROM domain_events WHERE event_id='event-turn-host-resolution'",
            [],
        )
        .expect("turn creation event is missing");
    drop(connection);
    assert!(matches!(
        conversation_store.resolve_conversation_session_host_for_turn(
            OWNER_ID,
            WORKSPACE_ID,
            conversation_id,
            turn_id,
        ),
        Err(StoreError::Integrity(_))
    ));
}
