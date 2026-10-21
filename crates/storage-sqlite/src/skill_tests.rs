//! Durable storage checks for the E10 Skill, SkillProposal, and demonstration
//! capture aggregates.
//!
//! These are real SQLite transaction checks: every assertion reads the committed
//! database or the committed domain event, never an in-memory stand-in.

use super::*;
use storage_core::{
    DemonstrationAction, DemonstrationCaptureConsent, DemonstrationCapturePolicy,
    DemonstrationSessionRecord, DemonstrationStatus, McpAppFrameRecord, SensitiveRegionPolicy,
    SkillCapabilityRef, SkillKind, SkillOrigin, SkillPlaceholder, SkillPlaceholderKind,
    SkillProposalDecision, SkillProposalRecord, SkillProposalReviewer, SkillProposalSource,
    SkillProposalStatus, SkillRedactionStatus, SkillRedactionSummary, SkillRecord, SkillRevision,
    SkillStatus,
};

const OWNER: &str = "owner-local";
const OTHER_OWNER: &str = "owner-other";
const WORKSPACE: &str = "workspace-skill";
const TIMESTAMP: &str = "2026-10-20T10:00:00Z";

#[derive(Clone)]
struct TestKeys {
    key: [u8; 32],
}

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: zeroize::Zeroizing::new(self.key),
        })
    }

    fn key_by_version(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob("unknown test key version".to_owned()));
        }
        self.current_key("", BlobPurpose::AggregateState)
    }
}

fn state_database(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory.path().join("state").join("state.sqlite3")
}

fn test_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
    let blob_store = Arc::new(FileBlobStore::new(
        directory.path().join("blobs"),
        TestKeys { key: [23_u8; 32] },
    ));
    SqliteWorkspaceStore::open(
        state_database(directory),
        blob_store,
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(5),
        },
    )
    .expect("SQLite store opens")
}

fn adapter(store: &SqliteWorkspaceStore, event_id: &str) -> SqliteSkillStore {
    SqliteSkillStore::new(
        store.clone(),
        SkillEventContext {
            event_id: event_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            hlc_timestamp: TIMESTAMP.to_owned(),
            correlation_id: format!("correlation-{event_id}"),
            causation_id: None,
            recorded_at: TIMESTAMP.to_owned(),
        },
    )
    .expect("Skill adapter")
}

fn create_workspace(store: &SqliteWorkspaceStore, workspace_id: &str, owner: &str) {
    let event = domain_workspace::EventContext {
        event_id: format!("event-workspace-{workspace_id}"),
        origin_runtime_id: "runtime-local".to_owned(),
        hlc_timestamp: TIMESTAMP.to_owned(),
        correlation_id: "correlation-workspace".to_owned(),
        causation_id: None,
        recorded_at: TIMESTAMP.to_owned(),
    };
    domain_workspace::WorkspaceService::new(store.clone())
        .create(domain_workspace::CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: "Desktop workspace".to_owned(),
            owner_principal_id: owner.to_owned(),
            event,
        })
        .expect("Workspace creation commits");
}

fn seed_resource(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    resource_id: &str,
    content: &[u8],
) -> String {
    let revision_id = format!("revision-{resource_id}");
    let content_digest = digest(content);
    store
        .create_resource(
            WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: format!("request-{resource_id}"),
                request_payload: json!({
                    "operation": "resource.test.create.v1",
                    "workspace_id": workspace_id,
                    "resource_id": resource_id,
                }),
            },
            ResourceRecord {
                resource_id: resource_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                kind: "OTHER".to_owned(),
                provider_identity: json!({"kind": "TEST"}),
                identity_digest: None,
                display_name: resource_id.to_owned(),
                current_revision_id: Some(revision_id.clone()),
                sensitivity: "INTERNAL".to_owned(),
                provenance: json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
                created_at: TIMESTAMP.to_owned(),
                updated_at: TIMESTAMP.to_owned(),
                version: 1,
            },
            ResourceRevisionRecord {
                resource_revision_id: revision_id.clone(),
                resource_id: resource_id.to_owned(),
                parent_revision_ids: Vec::new(),
                provider_revision: None,
                content_digest: Some(content_digest),
                size_bytes: Some(content.len() as u64),
                media_type: Some("text/markdown".to_owned()),
                observed_at: TIMESTAMP.to_owned(),
                created_by: json!({"kind": "USER", "principal_id": OWNER}),
            },
            EventDraft {
                event_id: format!("event-resource-{resource_id}"),
                workspace_id: workspace_id.to_owned(),
                entity_type: "Resource".to_owned(),
                entity_id: resource_id.to_owned(),
                origin_runtime_id: "runtime-local".to_owned(),
                entity_revision: 1,
                hlc_timestamp: TIMESTAMP.to_owned(),
                correlation_id: "correlation-resource".to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "resource.created.v1".to_owned(),
                payload: json!({"resource_id": resource_id, "workspace_id": workspace_id}),
                recorded_at: TIMESTAMP.to_owned(),
            },
            content.to_vec(),
        )
        .expect("Resource creation commits")
        .resource
        .resource_id
}

fn seed_task_and_artifact(
    directory: &tempfile::TempDir,
    workspace_id: &str,
    task_id: &str,
    artifact_id: &str,
    resource_id: &str,
    resource_revision_id: &str,
) {
    let connection = Connection::open(state_database(directory)).expect("inspect store");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    let tx = connection.unchecked_transaction().expect("seed transaction");
    tx.execute(
        "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES ('profile-1','TEST','Test profile','2026-10-20T10:00:00Z')",
        [],
    )
    .expect("seed agent profile");
    tx.execute(
        "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,enabled,lead_eligible,created_at,version) VALUES ('binding-1',?1,'profile-1','{}',1,1,'2026-10-20T10:00:00Z',1)",
        params![workspace_id],
    )
    .expect("seed agent binding");
    tx.execute(
        "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,priority,lead_agent_binding_id,created_by_json,created_at,updated_at) VALUES (?1,?2,1,'READY','NORMAL','binding-1','{}','2026-10-20T10:00:00Z','2026-10-20T10:00:00Z')",
        params![task_id, workspace_id],
    )
    .expect("seed task");
    tx.execute(
        "INSERT INTO task_spec_revisions(task_id,workspace_id,revision,objective,placement_preference,authored_by_json,lead_failover_policy_json,created_at) VALUES (?1,?2,1,'Demonstrated workflow','{}','{}','{\"mode\":\"DISABLED\",\"triggers\":[],\"fallback_agent_binding_ids\":[],\"max_lead_changes\":0}','2026-10-20T10:00:00Z')",
        params![task_id, workspace_id],
    )
    .expect("seed task spec revision");
    // The Artifact row is written first: its FK to the ArtifactVersion is
    // deferred, while the ArtifactVersion's FK back to the Artifact is not.
    tx.execute(
        "INSERT INTO artifacts(artifact_id,workspace_id,resource_id,task_id,kind,display_name,current_version,library_status,created_at,version) VALUES (?1,?2,?3,?4,'FILE','Demonstrated artifact',1,'TRANSIENT','2026-10-20T10:00:00Z',1)",
        params![artifact_id, workspace_id, resource_id, task_id],
    )
    .expect("seed artifact");
    tx.execute(
        "INSERT INTO artifact_versions(artifact_id,resource_id,version,resource_revision_id,content_kind,content_digest,storage_ref_json,content_media_type,content_size_bytes,provenance_json,created_at) VALUES (?1,?2,1,?3,'MANAGED_BLOB',?4,'{}','text/plain',0,'{}','2026-10-20T10:00:00Z')",
        params![
            artifact_id,
            resource_id,
            resource_revision_id,
            format!("sha256:{}", "0".repeat(64))
        ],
    )
    .expect("seed artifact version");
    tx.commit().expect("seed transaction commits");
}

/// Seed the Task and Artifact every same-Workspace proposal names.
fn seed_source_work(
    directory: &tempfile::TempDir,
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
) {
    let resource_id = seed_resource(store, workspace_id, "resource-source", b"source");
    seed_task_and_artifact(
        directory,
        workspace_id,
        "task-1",
        "artifact-1",
        &resource_id,
        &format!("revision-{resource_id}"),
    );
}

fn skill_record(skill_id: &str) -> SkillRecord {
    SkillRecord {
        skill_id: skill_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        kind: SkillKind::LocalProcedure,
        status: SkillStatus::Active,
        capability_ref: SkillCapabilityRef {
            capability_id: format!("local:{skill_id}"),
            identity_kind: "MCP_SKILL".to_owned(),
            source: "litecowork:local".to_owned(),
            package_version: None,
            digest: format!("sha256:{}", "a".repeat(64)),
            component: format!("local:{skill_id}"),
        },
        current_revision: 1,
        revisions: vec![SkillRevision {
            revision: 1,
            title: "Invoice cleanup".to_owned(),
            summary: "Clean up an invoice".to_owned(),
            manifest_digest: format!("sha256:{}", "b".repeat(64)),
            content_digest: format!("sha256:{}", "c".repeat(64)),
            content_ref: "resource-draft".to_owned(),
            declared_capabilities: vec!["connector.invoice.read".to_owned()],
            declared_requirements: Vec::new(),
            origin: SkillOrigin::LocalProposal {
                proposal_id: "proposal-1".to_owned(),
            },
            created_at: TIMESTAMP.to_owned(),
            created_by: OWNER.to_owned(),
        }],
        created_at: TIMESTAMP.to_owned(),
        updated_at: TIMESTAMP.to_owned(),
        version: 1,
    }
}

fn proposal_record(proposal_id: &str, draft_resource: &str) -> SkillProposalRecord {
    SkillProposalRecord {
        proposal_id: proposal_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        source: SkillProposalSource::Demonstration,
        source_task_id: "task-1".to_owned(),
        source_artifact_id: "artifact-1".to_owned(),
        draft_resource_ref: draft_resource.to_owned(),
        draft_digest: format!("sha256:{}", "d".repeat(64)),
        redaction: SkillRedactionSummary {
            status: SkillRedactionStatus::Passed,
            redacted_spans: 1,
            finding_codes: vec!["ACCESS_TOKEN".to_owned()],
            redacted_digest: format!("sha256:{}", "d".repeat(64)),
        },
        placeholders: vec![SkillPlaceholder {
            name: "PATH_1".to_owned(),
            kind: SkillPlaceholderKind::Path,
            sample: None,
        }],
        declared_capabilities: vec!["connector.invoice.read".to_owned()],
        status: SkillProposalStatus::Review,
        proposed_by: "service:demonstration-capture".to_owned(),
        approved_by: None,
        published_capability_ref: None,
        drift: None,
        created_at: TIMESTAMP.to_owned(),
        updated_at: TIMESTAMP.to_owned(),
        version: 1,
    }
}

fn demonstration_session(session_id: &str) -> DemonstrationSessionRecord {
    DemonstrationSessionRecord {
        session_id: session_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        environment_id: "environment-1".to_owned(),
        source_task_id: Some("task-1".to_owned()),
        source_artifact_id: Some("artifact-1".to_owned()),
        status: DemonstrationStatus::Capturing,
        capture_policy: DemonstrationCapturePolicy {
            max_duration_ms: 60_000,
            max_actions: 2,
            max_trace_bytes: 4_096,
            allowed_environment_class: "BROWSER".to_owned(),
            sensitive_region_policy: SensitiveRegionPolicy::PauseOnDetection,
        },
        consent: DemonstrationCaptureConsent {
            granted_by_principal_id: OWNER.to_owned(),
            granted_at: TIMESTAMP.to_owned(),
            scope: "SEMANTIC_CAPTURE".to_owned(),
        },
        captured_action_count: 0,
        captured_trace_bytes: 0,
        started_at: TIMESTAMP.to_owned(),
        completed_at: None,
        trace_resource_id: None,
        skill_proposal_id: None,
        actions: Vec::new(),
        created_at: TIMESTAMP.to_owned(),
        updated_at: TIMESTAMP.to_owned(),
        version: 1,
    }
}

fn action(sequence: u32, sensitive: bool) -> DemonstrationAction {
    DemonstrationAction {
        sequence,
        kind: "click".to_owned(),
        target: "invoices.download".to_owned(),
        value: Some("/home/owner/invoices/42.pdf".to_owned()),
        sensitive,
        coordinate_hint: Some("x=12,y=34".to_owned()),
    }
}

/// One host-issued MCP App frame record.
fn app_frame_record(frame_id: &str) -> McpAppFrameRecord {
    let descriptor = storage_core::build_app_frame(&storage_core::McpAppFramePolicy {
        frame_id: frame_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        server_identity: "mcp-server-1".to_owned(),
        resource_uri: "ui://invoice-app".to_owned(),
        content_digest: format!("sha256:{}", "1".repeat(64)),
        package_version: None,
        origin: format!("https://app.litecowork.local/mcp-app/{frame_id}"),
        csp: storage_core::McpAppCsp {
            connect_domains: vec!["https://api.example.com".to_owned()],
            resource_domains: Vec::new(),
        },
        task_id: Some("task-1".to_owned()),
        expires_at: "2026-10-21T10:00:00Z".to_owned(),
    })
    .expect("the frame policy is well formed");
    McpAppFrameRecord {
        frame_id: frame_id.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        descriptor,
        nonce: format!("nonce-{frame_id}"),
        created_at: TIMESTAMP.to_owned(),
        updated_at: TIMESTAMP.to_owned(),
        version: 1,
    }
}

#[test]
fn a_skill_revision_is_durable_event_backed_and_idempotent() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);

    let request = SkillMutationRequest {
        principal_id: OWNER.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        request_id: "request-skill-1".to_owned(),
        request_payload: json!({"operation": "skill.revise.v1", "skill_id": "skill-1"}),
        expected_version: None,
        skill: skill_record("skill-1"),
    };
    let committed = adapter(&store, "event-skill-1")
        .put_skill(request.clone())
        .expect("the first commit succeeds");
    assert!(!committed.replayed);
    assert_eq!(committed.skill.version, 1);

    let replay = adapter(&store, "event-skill-1-replay")
        .put_skill(request)
        .expect("the exact retry replays");
    assert!(replay.replayed);
    assert_eq!(replay.skill.skill_id, committed.skill.skill_id);

    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type = 'skill'",
            [],
            |row| row.get(0),
        )
        .expect("read skill event count");
    assert_eq!(events, 1, "the replay must not append a second event");
    let snapshots: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM aggregate_snapshots WHERE entity_type = 'skill'",
            [],
            |row| row.get(0),
        )
        .expect("read skill snapshot count");
    assert_eq!(snapshots, 1);
    let audits: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM audit_records WHERE action = 'SKILL_REVISION_COMMITTED'",
            [],
            |row| row.get(0),
        )
        .expect("read skill audit count");
    assert_eq!(audits, 1);
}

#[test]
fn a_skill_revision_survives_a_restart() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    adapter(&store, "event-skill-restart")
        .put_skill(SkillMutationRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-skill-restart".to_owned(),
            request_payload: json!({"operation": "skill.revise.v1"}),
            expected_version: None,
            skill: skill_record("skill-restart"),
        })
        .expect("the first commit succeeds");

    let reopened = test_store(&directory);
    let loaded = adapter(&reopened, "event-read-restart")
        .get_skill(OWNER, WORKSPACE, "skill-restart")
        .expect("the skill reads back")
        .expect("the skill is durable");
    assert_eq!(loaded.current_revision, 1);
    assert_eq!(loaded.status, SkillStatus::Active);
}

#[test]
fn a_stale_skill_revision_is_rejected() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    adapter(&store, "event-skill-stale")
        .put_skill(SkillMutationRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-skill-stale-1".to_owned(),
            request_payload: json!({"operation": "skill.revise.v1"}),
            expected_version: None,
            skill: skill_record("skill-stale"),
        })
        .expect("the first commit succeeds");
    let error = adapter(&store, "event-skill-stale-2")
        .put_skill(SkillMutationRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-skill-stale-2".to_owned(),
            request_payload: json!({"operation": "skill.revise.v1", "revision": 2}),
            expected_version: Some(9),
            skill: skill_record("skill-stale"),
        })
        .expect_err("a stale revision is rejected");
    assert!(matches!(error, StoreError::Conflict { .. }));
}

#[test]
fn a_duplicate_skill_identity_is_rejected() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    adapter(&store, "event-skill-duplicate-1")
        .put_skill(SkillMutationRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-skill-duplicate-1".to_owned(),
            request_payload: json!({"operation": "skill.revise.v1"}),
            expected_version: None,
            skill: skill_record("skill-duplicate"),
        })
        .expect("the first commit succeeds");
    let mut duplicate = skill_record("skill-duplicate-other");
    duplicate.capability_ref.capability_id = "local:skill-duplicate".to_owned();
    duplicate.capability_ref.component = "local:skill-duplicate".to_owned();
    let error = adapter(&store, "event-skill-duplicate-2")
        .put_skill(SkillMutationRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-skill-duplicate-2".to_owned(),
            request_payload: json!({"operation": "skill.revise.v1", "revision": 2}),
            expected_version: None,
            skill: duplicate,
        })
        .expect_err("a second Skill may not claim the same capability identity");
    assert!(matches!(error, StoreError::Conflict { .. }));
}

#[test]
fn an_issued_app_frame_is_durable_event_backed_and_idempotent() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);

    let request = AppFrameCreateRequest {
        principal_id: OWNER.to_owned(),
        workspace_id: WORKSPACE.to_owned(),
        request_id: "request-frame-1".to_owned(),
        request_payload: json!({"operation": "mcp-app.frame.issue.v1", "frame_id": "frame-1"}),
        frame: app_frame_record("frame-1"),
    };
    let committed = adapter(&store, "event-frame-1")
        .put_app_frame(request.clone())
        .expect("the frame is issued");
    assert!(!committed.replayed);
    assert_eq!(committed.frame.version, 1);
    assert_eq!(committed.frame.descriptor.sandbox, vec!["allow-scripts".to_owned()]);
    assert!(
        !committed.frame.descriptor.csp.contains("allow-same-origin"),
        "the frame must never allow same-origin"
    );
    assert!(
        committed.frame.descriptor.csp.contains("default-src 'none'"),
        "the frame must default to denying every source"
    );

    let replay = adapter(&store, "event-frame-1-replay")
        .put_app_frame(request)
        .expect("the exact retry replays");
    assert!(replay.replayed);

    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type = 'mcp_app_frame'",
            [],
            |row| row.get(0),
        )
        .expect("read frame event count");
    assert_eq!(events, 1, "the replay must not append a second event");
    let audits: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM audit_records WHERE action = 'MCP_APP_FRAME_ISSUED'",
            [],
            |row| row.get(0),
        )
        .expect("read frame audit count");
    assert_eq!(audits, 1);
}

#[test]
fn an_issued_app_frame_never_leaks_its_nonce_into_the_event_journal() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    adapter(&store, "event-frame-nonce")
        .put_app_frame(AppFrameCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-frame-nonce".to_owned(),
            request_payload: json!({"operation": "mcp-app.frame.issue.v1"}),
            frame: app_frame_record("frame-nonce"),
        })
        .expect("the frame is issued");
    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let payload: String = connection
        .query_row(
            "SELECT payload_json FROM domain_events WHERE entity_type = 'mcp_app_frame'",
            [],
            |row| row.get(0),
        )
        .expect("read the frame event payload");
    assert!(
        !payload.contains("nonce-frame-nonce"),
        "the host-issued nonce must never reach the event journal"
    );
}

#[test]
fn a_foreign_workspace_never_reads_an_app_frame() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    create_workspace(&store, "workspace-other", OTHER_OWNER);
    adapter(&store, "event-frame-foreign")
        .put_app_frame(AppFrameCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-frame-foreign".to_owned(),
            request_payload: json!({"operation": "mcp-app.frame.issue.v1"}),
            frame: app_frame_record("frame-foreign"),
        })
        .expect("the frame is issued");
    assert!(
        adapter(&store, "event-frame-foreign-read")
            .get_app_frame(OTHER_OWNER, WORKSPACE, "frame-foreign")
            .expect_err("a foreign principal cannot read the Workspace")
            == StoreError::NotFound
    );
    assert!(
        adapter(&store, "event-frame-foreign-write")
            .put_app_frame(AppFrameCreateRequest {
                principal_id: OTHER_OWNER.to_owned(),
                workspace_id: WORKSPACE.to_owned(),
                request_id: "request-frame-foreign-write".to_owned(),
                request_payload: json!({"operation": "mcp-app.frame.issue.v1"}),
                frame: app_frame_record("frame-foreign-write"),
            })
            .expect_err("a foreign principal cannot write into the Workspace")
            == StoreError::NotFound
    );
}

#[test]
fn a_second_frame_with_the_same_identity_is_rejected() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    adapter(&store, "event-frame-duplicate-1")
        .put_app_frame(AppFrameCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-frame-duplicate-1".to_owned(),
            request_payload: json!({"operation": "mcp-app.frame.issue.v1"}),
            frame: app_frame_record("frame-duplicate"),
        })
        .expect("the first frame is issued");
    let error = adapter(&store, "event-frame-duplicate-2")
        .put_app_frame(AppFrameCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-frame-duplicate-2".to_owned(),
            request_payload: json!({"operation": "mcp-app.frame.issue.v1"}),
            frame: app_frame_record("frame-duplicate"),
        })
        .expect_err("a second frame may not claim the same identity");
    assert!(matches!(error, StoreError::Conflict { .. }));
}

#[test]
fn an_issued_app_frame_survives_a_restart() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    adapter(&store, "event-frame-restart")
        .put_app_frame(AppFrameCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-frame-restart".to_owned(),
            request_payload: json!({"operation": "mcp-app.frame.issue.v1"}),
            frame: app_frame_record("frame-restart"),
        })
        .expect("the frame is issued");

    let reopened = test_store(&directory);
    let loaded = adapter(&reopened, "event-frame-restart-read")
        .get_app_frame(OWNER, WORKSPACE, "frame-restart")
        .expect("the frame reads back")
        .expect("the frame is durable");
    assert_eq!(loaded.nonce, "nonce-frame-restart");
    assert_eq!(loaded.descriptor.frame_id, "frame-restart");
}

#[test]
fn a_foreign_workspace_never_reads_or_writes_a_skill() {    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    create_workspace(&store, "workspace-other", OTHER_OWNER);
    let error = adapter(&store, "event-skill-foreign")
        .put_skill(SkillMutationRequest {
            principal_id: OTHER_OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-skill-foreign".to_owned(),
            request_payload: json!({"operation": "skill.revise.v1"}),
            expected_version: None,
            skill: skill_record("skill-foreign"),
        })
        .expect_err("a foreign principal cannot write into the Workspace");
    assert!(matches!(error, StoreError::NotFound));
    assert!(
        adapter(&store, "event-skill-foreign-read")
            .get_skill(OTHER_OWNER, WORKSPACE, "skill-foreign")
            .expect_err("a foreign principal cannot read the Workspace")
            == StoreError::NotFound
    );
}

#[test]
fn a_skill_proposal_only_accepts_same_workspace_references() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    seed_source_work(&directory, &store, WORKSPACE);
    create_workspace(&store, "workspace-other", OTHER_OWNER);
    let foreign_resource = seed_resource(&store, "workspace-other", "resource-foreign", b"foreign");
    let local_resource = seed_resource(&store, WORKSPACE, "resource-local", b"local");

    let error = adapter(&store, "event-proposal-foreign")
        .create_skill_proposal(SkillProposalCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-foreign".to_owned(),
            request_payload: json!({"operation": "skill.proposal.create.v1"}),
            proposal: proposal_record("proposal-foreign", &foreign_resource),
        })
        .expect_err("a foreign draft Resource is rejected");
    assert!(matches!(error, StoreError::NotFound));

    let committed = adapter(&store, "event-proposal-local")
        .create_skill_proposal(SkillProposalCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-local".to_owned(),
            request_payload: json!({"operation": "skill.proposal.create.v1"}),
            proposal: proposal_record("proposal-local", &local_resource),
        })
        .expect("a same-Workspace proposal commits");
    assert_eq!(committed.proposal.status, SkillProposalStatus::Review);
}

#[test]
fn an_unapproved_proposal_cannot_be_approved_and_a_stale_review_is_rejected() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    seed_source_work(&directory, &store, WORKSPACE);
    let resource = seed_resource(&store, WORKSPACE, "resource-review", b"draft");
    let mut proposal = proposal_record("proposal-review", &resource);
    proposal.status = SkillProposalStatus::Draft;
    adapter(&store, "event-proposal-review")
        .create_skill_proposal(SkillProposalCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-review".to_owned(),
            request_payload: json!({"operation": "skill.proposal.create.v1"}),
            proposal,
        })
        .expect("the draft commits");

    let error = adapter(&store, "event-proposal-approve-draft")
        .review_skill_proposal(SkillProposalReviewRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-approve-draft".to_owned(),
            request_payload: json!({"operation": "skill.proposal.review.v1"}),
            proposal_id: "proposal-review".to_owned(),
            expected_version: 1,
            decision: SkillProposalDecision::Approve,
            reviewer: SkillProposalReviewer {
                principal_id: OWNER.to_owned(),
                principal_kind: "USER".to_owned(),
                assurance_level: "APPROVE_SAFE".to_owned(),
            },
        })
        .expect_err("a draft cannot be approved");
    assert!(matches!(error, StoreError::Invalid(_)));

    let stale = adapter(&store, "event-proposal-approve-stale")
        .review_skill_proposal(SkillProposalReviewRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-approve-stale".to_owned(),
            request_payload: json!({"operation": "skill.proposal.review.v1"}),
            proposal_id: "proposal-review".to_owned(),
            expected_version: 7,
            decision: SkillProposalDecision::Approve,
            reviewer: SkillProposalReviewer {
                principal_id: OWNER.to_owned(),
                principal_kind: "USER".to_owned(),
                assurance_level: "APPROVE_SAFE".to_owned(),
            },
        })
        .expect_err("a stale review is rejected");
    assert!(matches!(stale, StoreError::Conflict { .. }));
}

#[test]
fn an_owner_approval_never_activates_or_publishes_a_skill() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    seed_source_work(&directory, &store, WORKSPACE);
    let resource = seed_resource(&store, WORKSPACE, "resource-approve", b"draft");
    adapter(&store, "event-proposal-approve")
        .create_skill_proposal(SkillProposalCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-approve".to_owned(),
            request_payload: json!({"operation": "skill.proposal.create.v1"}),
            proposal: proposal_record("proposal-approve", &resource),
        })
        .expect("the proposal commits");
    let committed = adapter(&store, "event-proposal-approved")
        .review_skill_proposal(SkillProposalReviewRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-approved".to_owned(),
            request_payload: json!({"operation": "skill.proposal.review.v1"}),
            proposal_id: "proposal-approve".to_owned(),
            expected_version: 1,
            decision: SkillProposalDecision::Approve,
            reviewer: SkillProposalReviewer {
                principal_id: OWNER.to_owned(),
                principal_kind: "USER".to_owned(),
                assurance_level: "APPROVE_SAFE".to_owned(),
            },
        })
        .expect("the owner decision commits");
    assert_eq!(committed.proposal.status, SkillProposalStatus::Approved);
    assert_eq!(committed.proposal.approved_by.as_deref(), Some(OWNER));
    assert_eq!(committed.proposal.published_capability_ref, None);

    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let activations: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM capability_activations",
            [],
            |row| row.get(0),
        )
        .expect("read activation count");
    assert_eq!(activations, 0, "approving a proposal must not activate a Skill");
    let grants: i64 = connection
        .query_row("SELECT COUNT(*) FROM capability_grants", [], |row| row.get(0))
        .expect("read grant count");
    assert_eq!(grants, 0, "approving a proposal must not grant authority");
}

#[test]
fn a_service_reviewer_cannot_approve_a_learned_skill() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    seed_source_work(&directory, &store, WORKSPACE);
    let resource = seed_resource(&store, WORKSPACE, "resource-service", b"draft");
    adapter(&store, "event-proposal-service")
        .create_skill_proposal(SkillProposalCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-service".to_owned(),
            request_payload: json!({"operation": "skill.proposal.create.v1"}),
            proposal: proposal_record("proposal-service", &resource),
        })
        .expect("the proposal commits");
    let error = adapter(&store, "event-proposal-service-approve")
        .review_skill_proposal(SkillProposalReviewRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-service-approve".to_owned(),
            request_payload: json!({"operation": "skill.proposal.review.v1"}),
            proposal_id: "proposal-service".to_owned(),
            expected_version: 1,
            decision: SkillProposalDecision::Approve,
            reviewer: SkillProposalReviewer {
                principal_id: OWNER.to_owned(),
                principal_kind: "SERVICE".to_owned(),
                assurance_level: "APPROVE_SAFE".to_owned(),
            },
        })
        .expect_err("a service may never approve a proposal");
    assert!(matches!(error, StoreError::Invalid(_)));
}

#[test]
fn a_demonstration_capture_stops_at_its_caps_and_on_a_sensitive_region() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    adapter(&store, "event-demonstration")
        .create_demonstration_session(DemonstrationCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-demonstration".to_owned(),
            request_payload: json!({"operation": "demonstration.start.v1"}),
            session: demonstration_session("demonstration-1"),
        })
        .expect("the capture session commits");

    let first = adapter(&store, "event-demonstration-1")
        .append_demonstration_action(DemonstrationAppendRequest {
            workspace_id: WORKSPACE.to_owned(),
            session_id: "demonstration-1".to_owned(),
            environment_id: "environment-1".to_owned(),
            action: action(1, false),
            now: TIMESTAMP.to_owned(),
        })
        .expect("the first observation commits");
    let DemonstrationAppendOutcome::Recorded { session } = first else {
        panic!("the first observation is recorded");
    };
    assert_eq!(session.captured_action_count, 1);
    assert!(session.captured_trace_bytes > 0);

    let sensitive = adapter(&store, "event-demonstration-sensitive")
        .append_demonstration_action(DemonstrationAppendRequest {
            workspace_id: WORKSPACE.to_owned(),
            session_id: "demonstration-1".to_owned(),
            environment_id: "environment-1".to_owned(),
            action: action(2, true),
            now: TIMESTAMP.to_owned(),
        })
        .expect("the sensitive observation is refused");
    assert!(matches!(
        sensitive,
        DemonstrationAppendOutcome::Refused { .. }
    ));

    // The sensitive region pauses the capture durably, so a later observation
    // cannot slip past the pause.
    let paused = adapter(&store, "event-demonstration-read")
        .get_demonstration_session(OWNER, WORKSPACE, "demonstration-1")
        .expect("the capture session reads back")
        .expect("the capture session is durable");
    assert_eq!(paused.status, DemonstrationStatus::Paused);

    let after_pause = adapter(&store, "event-demonstration-after-pause")
        .append_demonstration_action(DemonstrationAppendRequest {
            workspace_id: WORKSPACE.to_owned(),
            session_id: "demonstration-1".to_owned(),
            environment_id: "environment-1".to_owned(),
            action: action(3, false),
            now: TIMESTAMP.to_owned(),
        })
        .expect("the observation after the pause is refused");
    assert!(matches!(
        after_pause,
        DemonstrationAppendOutcome::Refused { .. }
    ));

    let foreign = adapter(&store, "event-demonstration-foreign")
        .append_demonstration_action(DemonstrationAppendRequest {
            workspace_id: WORKSPACE.to_owned(),
            session_id: "demonstration-1".to_owned(),
            environment_id: "environment-other".to_owned(),
            action: action(4, false),
            now: TIMESTAMP.to_owned(),
        })
        .expect("an observation from another Environment is refused");
    assert!(matches!(
        foreign,
        DemonstrationAppendOutcome::Refused { .. }
    ));
}

#[test]
fn a_demonstration_capture_stops_at_its_action_cap() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    let mut session = demonstration_session("demonstration-cap");
    session.capture_policy.max_actions = 1;
    adapter(&store, "event-demonstration-cap-session")
        .create_demonstration_session(DemonstrationCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-demonstration-cap-session".to_owned(),
            request_payload: json!({"operation": "demonstration.start.v1"}),
            session,
        })
        .expect("the capture session commits");

    let first = adapter(&store, "event-demonstration-cap-1")
        .append_demonstration_action(DemonstrationAppendRequest {
            workspace_id: WORKSPACE.to_owned(),
            session_id: "demonstration-cap".to_owned(),
            environment_id: "environment-1".to_owned(),
            action: action(1, false),
            now: TIMESTAMP.to_owned(),
        })
        .expect("the first observation commits");
    assert!(matches!(
        first,
        DemonstrationAppendOutcome::Recorded { .. }
    ));

    let capped = adapter(&store, "event-demonstration-cap-2")
        .append_demonstration_action(DemonstrationAppendRequest {
            workspace_id: WORKSPACE.to_owned(),
            session_id: "demonstration-cap".to_owned(),
            environment_id: "environment-1".to_owned(),
            action: action(2, false),
            now: TIMESTAMP.to_owned(),
        })
        .expect("the capped observation is refused");
    assert!(matches!(
        capped,
        DemonstrationAppendOutcome::Refused { .. }
    ));
}

#[test]
fn a_demonstration_that_omits_sensitive_fields_never_stores_the_value() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    let mut session = demonstration_session("demonstration-omit");
    session.capture_policy.sensitive_region_policy = SensitiveRegionPolicy::OmitSensitiveFields;
    adapter(&store, "event-demonstration-omit-session")
        .create_demonstration_session(DemonstrationCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-demonstration-omit-session".to_owned(),
            request_payload: json!({"operation": "demonstration.start.v1"}),
            session,
        })
        .expect("the capture session commits");

    let recorded = adapter(&store, "event-demonstration-omit-1")
        .append_demonstration_action(DemonstrationAppendRequest {
            workspace_id: WORKSPACE.to_owned(),
            session_id: "demonstration-omit".to_owned(),
            environment_id: "environment-1".to_owned(),
            action: action(1, true),
            now: TIMESTAMP.to_owned(),
        })
        .expect("the sensitive observation is recorded without its value");
    let DemonstrationAppendOutcome::Recorded { session } = recorded else {
        panic!("the omitted-fields observation is recorded");
    };
    assert_eq!(session.captured_action_count, 1);
    assert_eq!(session.actions.len(), 1);
    assert_eq!(session.actions[0].value, None);
    assert_eq!(session.actions[0].coordinate_hint, None);

    // The omitted value must not survive anywhere in the durable state.
    let connection = Connection::open(state_database(&directory)).expect("inspect store");
    let blobs: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM aggregate_snapshots WHERE entity_type = 'demonstration_session' AND blob_ref_json LIKE '%home/owner/invoices%'",
            [],
            |row| row.get(0),
        )
        .expect("read snapshot count");
    assert_eq!(blobs, 0, "the observed value must not reach durable state");
}

#[test]
fn a_demonstration_pause_resume_and_abort_are_durable() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    adapter(&store, "event-demonstration-lifecycle")
        .create_demonstration_session(DemonstrationCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-demonstration-lifecycle".to_owned(),
            request_payload: json!({"operation": "demonstration.start.v1"}),
            session: demonstration_session("demonstration-lifecycle"),
        })
        .expect("the capture session commits");

    let paused = adapter(&store, "event-demonstration-pause")
        .transition_demonstration_session(DemonstrationTransitionRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-demonstration-pause".to_owned(),
            request_payload: json!({"operation": "demonstration.pause.v1"}),
            session_id: "demonstration-lifecycle".to_owned(),
            expected_version: 1,
            transition: DemonstrationTransition::Pause,
        })
        .expect("the pause commits");
    assert_eq!(paused.session.status, DemonstrationStatus::Paused);

    let stale = adapter(&store, "event-demonstration-pause-stale")
        .transition_demonstration_session(DemonstrationTransitionRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-demonstration-pause-stale".to_owned(),
            request_payload: json!({"operation": "demonstration.pause.v1"}),
            session_id: "demonstration-lifecycle".to_owned(),
            expected_version: 1,
            transition: DemonstrationTransition::Resume,
        })
        .expect_err("a stale transition is rejected");
    assert!(matches!(stale, StoreError::Conflict { .. }));

    let resumed = adapter(&store, "event-demonstration-resume")
        .transition_demonstration_session(DemonstrationTransitionRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-demonstration-resume".to_owned(),
            request_payload: json!({"operation": "demonstration.resume.v1"}),
            session_id: "demonstration-lifecycle".to_owned(),
            expected_version: 2,
            transition: DemonstrationTransition::Resume,
        })
        .expect("the resume commits");
    assert_eq!(resumed.session.status, DemonstrationStatus::Capturing);

    let aborted = adapter(&store, "event-demonstration-abort")
        .transition_demonstration_session(DemonstrationTransitionRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-demonstration-abort".to_owned(),
            request_payload: json!({"operation": "demonstration.abort.v1"}),
            session_id: "demonstration-lifecycle".to_owned(),
            expected_version: 3,
            transition: DemonstrationTransition::Abort,
        })
        .expect("the abort commits");
    assert_eq!(aborted.session.status, DemonstrationStatus::Aborted);
    assert!(aborted.session.completed_at.is_some());
}

#[test]
fn a_proposal_and_a_capture_survive_a_restart() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    create_workspace(&store, WORKSPACE, OWNER);
    seed_source_work(&directory, &store, WORKSPACE);
    let resource = seed_resource(&store, WORKSPACE, "resource-restart", b"draft");
    adapter(&store, "event-proposal-restart")
        .create_skill_proposal(SkillProposalCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-proposal-restart".to_owned(),
            request_payload: json!({"operation": "skill.proposal.create.v1"}),
            proposal: proposal_record("proposal-restart", &resource),
        })
        .expect("the proposal commits");
    adapter(&store, "event-demonstration-restart")
        .create_demonstration_session(DemonstrationCreateRequest {
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            request_id: "request-demonstration-restart".to_owned(),
            request_payload: json!({"operation": "demonstration.start.v1"}),
            session: demonstration_session("demonstration-restart"),
        })
        .expect("the capture session commits");

    let reopened = test_store(&directory);
    let proposal = adapter(&reopened, "event-read-proposal-restart")
        .get_skill_proposal(OWNER, WORKSPACE, "proposal-restart")
        .expect("the proposal reads back")
        .expect("the proposal is durable");
    assert_eq!(proposal.status, SkillProposalStatus::Review);
    let session = adapter(&reopened, "event-read-demonstration-restart")
        .get_demonstration_session(OWNER, WORKSPACE, "demonstration-restart")
        .expect("the capture session reads back")
        .expect("the capture session is durable");
    assert_eq!(session.status, DemonstrationStatus::Capturing);
    assert_eq!(session.capture_policy.max_actions, 2);
}
