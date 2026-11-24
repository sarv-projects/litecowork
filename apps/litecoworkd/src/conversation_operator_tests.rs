use super::*;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    response::Response,
};
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use serde::Deserialize;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{collections::HashSet, sync::Arc, time::Duration};
use storage_core::{
    AgentSessionStore, BlobPurpose, ConversationRecord, ConversationStore, ConversationTurnStore,
    CreateConversationCommit, EventDraft, ResourceRecord, ResourceRevisionRecord, StoreError,
    WorkspaceCreateRequest, conversation::request_payload_value,
};
use storage_sqlite::{
    FileBlobStore, SqliteConfig, SqliteConversationStore, SqliteWorkspaceStore, WorkspaceBlobKey,
    WorkspaceBlobKeyProvider,
};
use zeroize::Zeroizing;

const OWNER: &str = "a06-pagination-owner";
const RUNTIME: &str = "a06-pagination-runtime";
const WORKSPACE_A: &str = "a06-pagination-workspace-a";
const WORKSPACE_B: &str = "a06-pagination-workspace-b";
const DISPATCHER_INCARNATION: &str = "turn-command-dispatcher-incarnation";
const TURN_COMMAND_DAEMON_INCARNATION: &str = "turn-command-daemon-incarnation";

#[derive(Clone)]
struct PaginationTestKeys;

impl WorkspaceBlobKeyProvider for PaginationTestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([0x61; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob(
                "unknown pagination test key version".to_owned(),
            ));
        }
        self.current_key(workspace_id, purpose)
    }
}

#[derive(Debug, Deserialize)]
struct TestConversationPage {
    items: Vec<ConversationRecord>,
    next_cursor: Option<String>,
}

fn create_workspace(store: &SqliteWorkspaceStore, workspace_id: &str, suffix: &str) {
    let timestamp = "2026-10-15T12:00:00.000000000Z";
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: workspace_id.to_owned(),
            name: format!("Conversation pagination {suffix}"),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: format!("a06-{suffix}-workspace-created"),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: timestamp.to_owned(),
                correlation_id: format!("a06-{suffix}-workspace-created"),
                causation_id: None,
                recorded_at: timestamp.to_owned(),
            },
        })
        .expect("Workspace commits to temporary SQLite");
}

fn seed_conversation(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    conversation_id: &str,
    title: &str,
    timestamp: &str,
) {
    let event_id = format!("{conversation_id}-created");
    SqliteConversationStore::new(store.clone())
        .create_conversation(CreateConversationCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: format!("{conversation_id}-request"),
                request_payload: request_payload_value(workspace_id, Some(title)),
            },
            conversation: ConversationRecord {
                conversation_id: conversation_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                title: Some(title.to_owned()),
                active_agent_binding_id: None,
                version: 1,
                created_at: timestamp.to_owned(),
            },
            event: EventDraft {
                event_id: event_id.clone(),
                workspace_id: workspace_id.to_owned(),
                entity_type: "Conversation".to_owned(),
                entity_id: conversation_id.to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                entity_revision: 1,
                hlc_timestamp: timestamp.to_owned(),
                correlation_id: event_id,
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.created.v1".to_owned(),
                payload: serde_json::json!({
                    "conversation_id": conversation_id,
                    "created_by": {"kind": "USER", "principal_id": OWNER}
                }),
                recorded_at: timestamp.to_owned(),
            },
        })
        .expect("Conversation, event, and receipt commit to SQLite");
}

fn seed_text_resource(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    resource_id: &str,
) -> String {
    use sha2::{Digest, Sha256};
    let content = b"durable attached text";
    let revision_id = format!("revision-{resource_id}");
    let digest = format!("sha256:{:x}", Sha256::digest(content));
    let timestamp = "2026-10-15T12:00:00.000000000Z";
    store
        .create_resource(
            WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: format!("create-{resource_id}"),
                request_payload: serde_json::json!({
                    "operation": "resource.quick_import.v1",
                    "workspace_id": workspace_id,
                    "resource_id": resource_id,
                    "content_digest": digest,
                    "size_bytes": content.len()
                }),
            },
            ResourceRecord {
                resource_id: resource_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                kind: "FILE".to_owned(),
                provider_identity: serde_json::json!({}),
                identity_digest: None,
                display_name: "attached.txt".to_owned(),
                current_revision_id: Some(revision_id.clone()),
                sensitivity: "PERSONAL".to_owned(),
                provenance: serde_json::json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
                created_at: timestamp.to_owned(),
                updated_at: timestamp.to_owned(),
                version: 1,
            },
            ResourceRevisionRecord {
                resource_revision_id: revision_id.clone(),
                resource_id: resource_id.to_owned(),
                parent_revision_ids: Vec::new(),
                provider_revision: None,
                content_digest: Some(digest),
                size_bytes: Some(content.len() as u64),
                media_type: Some("text/plain".to_owned()),
                observed_at: timestamp.to_owned(),
                created_by: serde_json::json!({"kind": "USER", "principal_id": OWNER}),
            },
            EventDraft {
                event_id: format!("event-{resource_id}"),
                workspace_id: workspace_id.to_owned(),
                entity_type: "Resource".to_owned(),
                entity_id: resource_id.to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                entity_revision: 1,
                hlc_timestamp: timestamp.to_owned(),
                correlation_id: format!("event-{resource_id}"),
                causation_id: None,
                schema_version: 1,
                event_type: "resource.created.v1".to_owned(),
                payload: serde_json::json!({
                    "resource_id": resource_id,
                    "workspace_id": workspace_id,
                    "kind": "FILE",
                    "aggregate_version": 1
                }),
                recorded_at: timestamp.to_owned(),
            },
            content.to_vec(),
        )
        .expect("text Resource commits for send-replay coverage");
    revision_id
}

async fn list_page(router: &Router, workspace_id: &str, cursor: Option<&str>) -> Response {
    let path = match cursor {
        Some(cursor) => format!("/v1/conversations?cursor={cursor}"),
        None => "/v1/conversations".to_owned(),
    };
    let mut request = Request::get(path)
        .header("x-workspace-id", workspace_id)
        .body(Body::empty())
        .expect("Conversation list request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated Conversation route responds")
}

async fn create_conversation_request(
    router: &Router,
    workspace_id: &str,
    request_id: &str,
    title: &str,
) -> Response {
    let mut request = Request::post("/v1/conversations")
        .header("x-workspace-id", workspace_id)
        .header("idempotency-key", request_id)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "workspace_id": workspace_id,
                "title": title,
            })
            .to_string(),
        ))
        .expect("Conversation creation request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated Conversation creation route responds")
}

async fn read_created_conversation(response: Response) -> ConversationRecord {
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = to_bytes(response.into_body(), 8192)
        .await
        .expect("Conversation creation response is bounded");
    serde_json::from_slice(&bytes).expect("Conversation creation response matches its contract")
}

async fn submit_turn(router: &Router, workspace_id: &str, conversation_id: &str) -> Response {
    submit_turn_body(
        router,
        workspace_id,
        conversation_id,
        r#"{"content":[{"kind":"TEXT","text":"Please summarize this project."}],"resource_refs":[],"agent_binding_id":null,"presentation_preference":"AUTO"}"#,
    )
    .await
}

async fn submit_turn_body(
    router: &Router,
    workspace_id: &str,
    conversation_id: &str,
    body: &str,
) -> Response {
    submit_turn_body_with_key(
        router,
        workspace_id,
        conversation_id,
        "conversation-turn-submit-1",
        body,
    )
    .await
}

async fn submit_turn_body_with_key(
    router: &Router,
    workspace_id: &str,
    conversation_id: &str,
    request_id: &str,
    body: &str,
) -> Response {
    submit_turn_with(
        router,
        workspace_id,
        conversation_id,
        request_id,
        body,
        true,
    )
    .await
}

async fn submit_turn_with(
    router: &Router,
    workspace_id: &str,
    conversation_id: &str,
    request_id: &str,
    body: &str,
    authenticated: bool,
) -> Response {
    let mut request = Request::post(format!("/v1/conversations/{conversation_id}/turns"))
        .header("x-workspace-id", workspace_id)
        .header("idempotency-key", request_id)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .expect("Conversation turn request is valid");
    if authenticated {
        request.extensions_mut().insert(AuthenticatedLocalPeer);
    }
    router
        .clone()
        .oneshot(request)
        .await
        .expect("Conversation turn route responds")
}

/// Seeds an enabled, lead-eligible AgentBinding and pins it as the Conversation's
/// already-admitted selection for turn-resolution tests.
fn bind_conversation_agent(
    database: &std::path::Path,
    workspace_id: &str,
    conversation_id: &str,
    binding_id: &str,
) {
    seed_agent_binding(database, workspace_id, binding_id, true, true);
    set_conversation_agent(database, workspace_id, conversation_id, binding_id);
}

/// Seeds an AgentBinding row with the exact admitted state the Workspace default
/// and the Conversation override resolve against.
fn seed_agent_binding(
    database: &std::path::Path,
    workspace_id: &str,
    binding_id: &str,
    enabled: bool,
    lead_eligible: bool,
) {
    let profile_id = format!("{binding_id}-profile");
    let connection = rusqlite::Connection::open(database).expect("seed AgentBinding");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable seed foreign keys");
    connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES(?1,'codex','Codex',?2)",
            rusqlite::params![profile_id, "2026-10-15T12:00:00.000000000Z"],
        )
        .expect("seed AgentProfile");
    connection
        .execute(
            "INSERT INTO agent_endpoints(endpoint_id,agent_profile_id,protocol,topology,protocol_version,capabilities_json) VALUES(?1,?2,'CLI','PROCESS_ADAPTER',NULL,'{}')",
            rusqlite::params![format!("{binding_id}-endpoint"), profile_id],
        )
        .expect("seed AgentEndpoint");
    connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,runtime_id,endpoint_selection_policy_json,auth_ref,configuration_json,enabled,lead_eligible,created_at,version) VALUES(?1,?2,?3,NULL,'{}',NULL,'{}',?4,?5,?6,1)",
            rusqlite::params![
                binding_id,
                workspace_id,
                profile_id,
                i64::from(enabled),
                i64::from(lead_eligible),
                "2026-10-15T12:00:00.000000000Z"
            ],
        )
        .expect("seed AgentBinding");
}

/// Seeds an online, READY, EXECUTOR-entitled Runtime for the Conversation's endpoint.
/// This deliberately proves that inventory readiness alone cannot authorize a turn.
fn seed_ready_runtime_for_conversation(
    database: &std::path::Path,
    workspace_id: &str,
    binding_id: &str,
) {
    let runtime_id = format!("{binding_id}-runtime");
    let incarnation_id = format!("{binding_id}-incarnation");
    let endpoint_id = format!("{binding_id}-endpoint");
    let timestamp = "2026-10-15T12:00:00.000000000Z";
    let mut connection = rusqlite::Connection::open(database).expect("seed ready Runtime");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable ready Runtime fixture foreign keys");
    let tx = connection
        .transaction()
        .expect("begin ready Runtime fixture");
    tx.execute(
        "INSERT INTO runtimes(runtime_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,resource_capacity_json,last_seen,version) VALUES(?1,'{}','0.1.0','linux','x86_64','[]','LOCAL','ONLINE','MANUAL',?2,'{}',?3,1)",
        rusqlite::params![runtime_id, incarnation_id, timestamp],
    )
    .expect("seed online Runtime inventory");
    tx.execute(
        "INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state,ready_at,stopped_at,version) VALUES(?1,?2,?3,'0.1.0',0,'READY',?3,NULL,1)",
        rusqlite::params![incarnation_id, runtime_id, timestamp],
    )
    .expect("seed ready Runtime incarnation");
    tx.execute(
        "INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,revoked_at,version) VALUES(?1,?2,?3,'LOCAL_ENROLLMENT','ACTIVE','[\"EXECUTOR\"]',?4,?4,NULL,1)",
        rusqlite::params![format!("{binding_id}-workspace-binding"), runtime_id, workspace_id, timestamp],
    )
    .expect("seed Runtime executor enrollment");
    tx.execute(
        "UPDATE agent_bindings SET endpoint_selection_policy_json=json_object('mode','AUTO_COMPATIBLE') WHERE agent_binding_id=?1",
        [binding_id],
    )
    .expect("set explicit endpoint policy");
    tx.execute(
        "INSERT INTO agent_endpoint_bindings(endpoint_id,runtime_id,runtime_incarnation_id,endpoint_ref,observed_at,expires_at) VALUES(?1,?2,?3,'local://codex',?4,NULL)",
        rusqlite::params![endpoint_id, runtime_id, incarnation_id, timestamp],
    )
    .expect("bind endpoint to ready Runtime");
    tx.execute(
        "INSERT INTO runtime_offers(runtime_id,runtime_incarnation_id,offer_kind,offer_ref,compatible,readiness,constraints_json,observed_at,expires_at) VALUES(?1,?2,'AGENT_ENDPOINT',?3,1,'AVAILABLE','{}',?4,'2099-10-15T12:00:00Z')",
        rusqlite::params![runtime_id, incarnation_id, endpoint_id, timestamp],
    )
    .expect("publish endpoint offer for ready Runtime");
    tx.commit().expect("commit ready Runtime fixture");
}

/// Configures the Workspace default AgentBinding through the same guarded column
/// the `PATCH /v1/workspaces/{id}/default-agent-binding` route writes.
fn set_workspace_default_agent(database: &std::path::Path, workspace_id: &str, binding_id: &str) {
    let connection = rusqlite::Connection::open(database).expect("seed Workspace default");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable seed foreign keys");
    let updated = connection
        .execute(
            "UPDATE workspaces SET default_agent_binding_id=?1 WHERE workspace_id=?2",
            rusqlite::params![binding_id, workspace_id],
        )
        .expect("set the Workspace default AgentBinding");
    assert_eq!(updated, 1, "the fixture Workspace must exist");
}

/// Pins the Conversation's own AgentBinding override.
fn set_conversation_agent(
    database: &std::path::Path,
    workspace_id: &str,
    conversation_id: &str,
    binding_id: &str,
) {
    let connection = rusqlite::Connection::open(database).expect("seed Conversation binding");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable seed foreign keys");
    let updated = connection
        .execute(
            "UPDATE conversations SET active_agent_binding_id=?1 WHERE workspace_id=?2 AND conversation_id=?3",
            rusqlite::params![binding_id, workspace_id, conversation_id],
        )
        .expect("pin Conversation AgentBinding");
    assert_eq!(updated, 1, "the fixture Conversation must exist");
}

async fn set_conversation_agent_binding_request(
    router: &Router,
    workspace_id: &str,
    conversation_id: &str,
    expected_version: u64,
    request_id: &str,
    agent_binding_id: serde_json::Value,
) -> Response {
    let mut request = Request::patch(format!("/v1/conversations/{conversation_id}/agent-binding"))
        .header("x-workspace-id", workspace_id)
        .header("if-match", format!("\"{expected_version}\""))
        .header("idempotency-key", request_id)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "agent_binding_id": agent_binding_id }).to_string(),
        ))
        .expect("Conversation AgentBinding mutation request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated Conversation AgentBinding route responds")
}

async fn read_updated_conversation(response: Response) -> ConversationRecord {
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 8192)
        .await
        .expect("Conversation AgentBinding response is bounded");
    serde_json::from_slice(&bytes).expect("Conversation AgentBinding response matches contract")
}

async fn read_operator_error(response: Response) -> (StatusCode, serde_json::Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 8192)
        .await
        .expect("bounded Operator error body");
    (
        status,
        serde_json::from_slice(&bytes).expect("Operator error JSON"),
    )
}

fn count_rows(connection: &rusqlite::Connection, table: &str, workspace_id: &str) -> i64 {
    let sql = match table {
        "tasks" => "SELECT COUNT(*) FROM tasks WHERE workspace_id=?1",
        "attempts" => {
            "SELECT COUNT(*) FROM attempts a JOIN tasks t ON t.task_id=a.task_id WHERE t.workspace_id=?1"
        }
        "agent_sessions" => "SELECT COUNT(*) FROM agent_sessions WHERE workspace_id=?1",
        "execution_leases" => {
            "SELECT COUNT(*) FROM execution_leases l JOIN tasks t ON t.task_id=l.task_id WHERE t.workspace_id=?1"
        }
        "environments" => "SELECT COUNT(*) FROM environments WHERE owner_workspace_id=?1",
        _ => panic!("unsupported test table"),
    };
    connection
        .query_row(sql, [workspace_id], |row| row.get(0))
        .expect("read execution-state row count")
}

fn assert_no_execution_state(connection: &rusqlite::Connection, workspace_id: &str) {
    for table in [
        "tasks",
        "attempts",
        "agent_sessions",
        "execution_leases",
        "environments",
    ] {
        assert_eq!(
            count_rows(connection, table, workspace_id),
            0,
            "ordinary Conversation submit must not create {table} rows"
        );
    }
}

async fn read_page(response: Response) -> TestConversationPage {
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("Conversation page is bounded");
    serde_json::from_slice(&bytes).expect("Conversation page matches its response contract")
}

#[tokio::test]
async fn conversation_operator_cursor_paginates_more_than_fifty_and_fences_workspace_and_owner() {
    let directory = tempfile::tempdir().expect("temporary Conversation pagination directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation pagination directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "workspace-a");
    create_workspace(&store, WORKSPACE_B, "workspace-b");

    // Make Conversations 50 and 51 share the oldest timestamp. The ID tie-breaker
    // must stay stable across the 50-row page boundary so Conversation 50 remains
    // reachable on the next page.
    for ordinal in 1..=51_u32 {
        let seconds_after_oldest = if ordinal >= 50 { 0 } else { 51 - ordinal };
        let timestamp = format!("2026-10-15T12:00:{seconds_after_oldest:02}.000000000Z");
        let conversation_id = format!("a06-conversation-{ordinal:03}");
        let title = format!("Conversation {ordinal}");
        seed_conversation(&store, WORKSPACE_A, &conversation_id, &title, &timestamp);
    }
    seed_conversation(
        &store,
        WORKSPACE_B,
        "a06-workspace-b-conversation",
        "Workspace B conversation",
        "2026-10-15T12:01:00.000000000Z",
    );

    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());
    let first = read_page(list_page(&router, WORKSPACE_A, None).await).await;
    assert_eq!(first.items.len(), 50, "the default page is bounded at 50");
    let first_cursor = first
        .next_cursor
        .clone()
        .expect("51 durable Conversations require a continuation cursor");

    let mut pages = vec![first];
    let mut cursor = Some(first_cursor.clone());
    while let Some(current_cursor) = cursor.take() {
        assert!(
            pages.len() < 3,
            "pagination must terminate within two pages"
        );
        let page = read_page(list_page(&router, WORKSPACE_A, Some(&current_cursor)).await).await;
        assert!(
            page.items.len() <= 50,
            "every response respects the page bound"
        );
        assert_ne!(page.next_cursor.as_deref(), Some(current_cursor.as_str()));
        cursor = page.next_cursor.clone();
        pages.push(page);
    }

    assert_eq!(
        pages
            .iter()
            .map(|page| page.items.len())
            .collect::<Vec<_>>(),
        [50, 1]
    );
    assert_eq!(
        pages[1].items[0].title.as_deref(),
        Some("Conversation 50"),
        "the oldest tuple is reachable on the second page when timestamps tie"
    );
    let returned: Vec<_> = pages
        .iter()
        .flat_map(|page| page.items.iter().cloned())
        .collect();
    assert_eq!(returned.len(), 51);
    assert!(returned.iter().all(|item| item.workspace_id == WORKSPACE_A));
    let unique_ids: HashSet<_> = returned
        .iter()
        .map(|item| item.conversation_id.as_str())
        .collect();
    assert_eq!(
        unique_ids.len(),
        51,
        "cursor pages contain no duplicate records"
    );
    let mut expected_ids: Vec<_> = (1..=49_u32)
        .map(|ordinal| format!("a06-conversation-{ordinal:03}"))
        .collect();
    expected_ids.extend([
        "a06-conversation-051".to_owned(),
        "a06-conversation-050".to_owned(),
    ]);
    assert_eq!(
        returned
            .iter()
            .map(|item| item.conversation_id.clone())
            .collect::<Vec<_>>(),
        expected_ids,
        "pages preserve newest-first order and include every durable Conversation"
    );
    assert_eq!(
        returned
            .iter()
            .filter(|item| item.title.as_deref() == Some("Conversation 50"))
            .count(),
        1,
        "Conversation 50 is returned exactly once beyond the first page"
    );

    let replayed_second =
        read_page(list_page(&router, WORKSPACE_A, Some(&first_cursor)).await).await;
    assert_eq!(
        replayed_second.items, pages[1].items,
        "replaying a valid cursor returns the same deterministic page"
    );

    let workspace_b = read_page(list_page(&router, WORKSPACE_B, None).await).await;
    assert_eq!(workspace_b.items.len(), 1);
    assert_eq!(
        workspace_b.items[0].conversation_id,
        "a06-workspace-b-conversation"
    );
    assert!(
        workspace_b
            .items
            .iter()
            .all(|item| item.workspace_id == WORKSPACE_B)
    );

    let mismatched_cursor = list_page(&router, WORKSPACE_B, Some(&first_cursor)).await;
    assert_eq!(
        mismatched_cursor.status(),
        StatusCode::BAD_REQUEST,
        "a cursor issued for Workspace A cannot be reused in Workspace B"
    );

    let foreign_owner_router = test_router(
        store,
        "a06-pagination-foreign-owner".to_owned(),
        RUNTIME.to_owned(),
    );
    let foreign_owner_page = list_page(&foreign_owner_router, WORKSPACE_A, None).await;
    assert_eq!(
        foreign_owner_page.status(),
        StatusCode::NOT_FOUND,
        "a different authenticated principal cannot enumerate another owner's Workspace"
    );
}

#[tokio::test]
async fn conversation_create_retry_replays_the_committed_identity_without_duplicates() {
    let directory = tempfile::tempdir().expect("temporary Conversation replay directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation replay directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "create-replay");
    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());
    let request_id = "conversation-create-replay-request-1";

    // Model a lost acknowledgement: the exact same durable request is sent again
    // after the first server-side commit has already happened.
    let first = read_created_conversation(
        create_conversation_request(&router, WORKSPACE_A, request_id, "Research notes").await,
    )
    .await;
    let replay = read_created_conversation(
        create_conversation_request(&router, WORKSPACE_A, request_id, "Research notes").await,
    )
    .await;
    assert_eq!(
        replay, first,
        "a retry returns the original committed identity"
    );

    let page = read_page(list_page(&router, WORKSPACE_A, None).await).await;
    assert_eq!(
        page.items.as_slice(),
        std::slice::from_ref(&first),
        "one receipt produces one Conversation row"
    );

    let changed_payload = create_conversation_request(
        &router,
        WORKSPACE_A,
        request_id,
        "Different request with reused key",
    )
    .await;
    assert_eq!(
        changed_payload.status(),
        StatusCode::CONFLICT,
        "the same key cannot be reused for a different title"
    );
    let page_after_conflict = read_page(list_page(&router, WORKSPACE_A, None).await).await;
    assert_eq!(page_after_conflict.items.len(), 1);
}

#[tokio::test]
async fn new_chat_first_send_commits_and_replays_conversation_and_turn_atomically() {
    let directory = tempfile::tempdir().expect("temporary first-send directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private first-send directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "first-send");
    seed_agent_binding(&database, WORKSPACE_A, "first-send-binding", true, true);
    seed_agent_binding(
        &database,
        WORKSPACE_A,
        "first-send-disabled-binding",
        false,
        true,
    );
    set_workspace_default_agent(&database, WORKSPACE_A, "first-send-binding");
    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());
    let request_id = "conversation-first-send-request-1";
    let body = serde_json::json!({
        "workspace_id": WORKSPACE_A,
        "title": "Plan a quiet weekend.",
        "content": [{"kind":"TEXT","text":"Plan a quiet weekend."}],
        "resource_refs": [],
        "agent_binding_id": "first-send-binding",
        "presentation_preference": "RICH"
    });
    let send = |body: serde_json::Value| {
        let router = router.clone();
        async move {
            let mut request = Request::post("/v1/conversations/first-turn")
                .header("x-workspace-id", WORKSPACE_A)
                .header("idempotency-key", request_id)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("first-send request is valid");
            request.extensions_mut().insert(AuthenticatedLocalPeer);
            router
                .oneshot(request)
                .await
                .expect("first-send route responds")
        }
    };
    let counts = || {
        let connection =
            rusqlite::Connection::open(&database).expect("inspect first-send transaction");
        connection.query_row(
            "SELECT
                (SELECT COUNT(*) FROM conversations WHERE workspace_id=?1),
                (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id IN (SELECT conversation_id FROM conversations WHERE workspace_id=?1)),
                (SELECT COUNT(*) FROM conversation_turns WHERE conversation_id IN (SELECT conversation_id FROM conversations WHERE workspace_id=?1)),
                (SELECT COUNT(*) FROM request_dedup WHERE principal_id=?2 AND request_id=?3),
                (SELECT COUNT(*) FROM conversation_dispatch_intents WHERE conversation_id IN (SELECT conversation_id FROM conversations WHERE workspace_id=?1)),
                (SELECT COUNT(*) FROM domain_events WHERE workspace_id=?1 AND type IN ('conversation.created.v1','conversation.message.added.v1','conversation.turn.created.v2'))",
            rusqlite::params![WORKSPACE_A, OWNER, request_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?)),
        ).expect("read atomic first-send record counts")
    };
    let receipt_json = || {
        rusqlite::Connection::open(&database)
            .expect("inspect first-send receipt")
            .query_row(
                "SELECT response_json FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
                rusqlite::params![OWNER, request_id],
                |row| row.get::<_, String>(0),
            )
            .expect("read durable first-send receipt")
    };

    let foreign_owner_router = test_router(
        store.clone(),
        "first-send-foreign-owner".to_owned(),
        RUNTIME.to_owned(),
    );
    let mut foreign_owner_request = Request::post("/v1/conversations/first-turn")
        .header("x-workspace-id", WORKSPACE_A)
        .header("idempotency-key", request_id)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("foreign-owner first-send request is valid");
    foreign_owner_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    assert_eq!(
        foreign_owner_router
            .oneshot(foreign_owner_request)
            .await
            .expect("foreign-owner first-send route responds")
            .status(),
        StatusCode::FORBIDDEN,
        "another principal cannot create the Conversation"
    );
    assert_eq!(counts(), (0, 0, 0, 0, 0, 0));

    let workspace_mismatch = send(serde_json::json!({
        "workspace_id": WORKSPACE_B,
        "title": "Plan a quiet weekend.",
        "content": [{"kind":"TEXT","text":"Plan a quiet weekend."}],
        "resource_refs": [],
        "agent_binding_id": "first-send-binding",
        "presentation_preference": "RICH"
    }))
    .await;
    assert_eq!(workspace_mismatch.status(), StatusCode::FORBIDDEN);
    assert_eq!(counts(), (0, 0, 0, 0, 0, 0));

    let missing_resource = send(serde_json::json!({
        "workspace_id": WORKSPACE_A,
        "title": "Plan a quiet weekend.",
        "content": [{"kind":"TEXT","text":"Plan a quiet weekend."}],
        "resource_refs": [{
            "workspace_id": WORKSPACE_A,
            "resource_id": "missing-first-send-resource",
            "revision_id": "missing-first-send-revision"
        }],
        "agent_binding_id": "first-send-binding",
        "presentation_preference": "RICH"
    }))
    .await;
    let (missing_resource_status, missing_resource_body) =
        read_operator_error(missing_resource).await;
    assert_eq!(missing_resource_status, StatusCode::NOT_FOUND);
    assert_eq!(missing_resource_body["code"], "NOT_FOUND");
    assert_eq!(counts(), (0, 0, 0, 0, 0, 0));

    // A definitive pre-commit refusal leaves the exact request key unused, allowing
    // correction and retry without a partial Conversation, message, turn or receipt.
    let (rejected_status, rejected_body) = read_operator_error(
        send(serde_json::json!({
            "workspace_id": WORKSPACE_A,
            "title": "Plan a quiet weekend.",
            "content": [{"kind":"TEXT","text":"Plan a quiet weekend."}],
            "resource_refs": [],
            "agent_binding_id": "first-send-disabled-binding",
            "presentation_preference": "RICH"
        }))
        .await,
    )
    .await;
    assert_eq!(rejected_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(rejected_body["code"], "AGENT_UNAVAILABLE");
    assert_eq!(counts(), (0, 0, 0, 0, 0, 0));

    // The test router intentionally has no native supervisor. A successful atomic
    // admission is therefore followed by the existing typed unavailable settlement.
    let first = send(body.clone()).await;
    let first_status = first.status();
    let first_body = to_bytes(first.into_body(), 8192)
        .await
        .expect("read first-send response");
    assert_eq!(
        first_status,
        StatusCode::SERVICE_UNAVAILABLE,
        "first-send response body: {}",
        String::from_utf8_lossy(&first_body)
    );
    assert_eq!(counts(), (1, 1, 1, 1, 1, 3));
    let admitted_receipt_json = receipt_json();
    let admitted_receipt: serde_json::Value =
        serde_json::from_str(&admitted_receipt_json).expect("admission receipt is valid JSON");
    assert_eq!(admitted_receipt["conversation"]["version"], 2);
    assert_eq!(admitted_receipt["events"].as_array().map(Vec::len), Some(3));

    drop(router);
    drop(store);
    let restarted_store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("reopen the durable first-send database after Runtime restart");
    let router = test_router(restarted_store, OWNER.to_owned(), RUNTIME.to_owned());
    let send = |body: serde_json::Value| {
        let router = router.clone();
        async move {
            let mut request = Request::post("/v1/conversations/first-turn")
                .header("x-workspace-id", WORKSPACE_A)
                .header("idempotency-key", request_id)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("first-send retry request is valid");
            request.extensions_mut().insert(AuthenticatedLocalPeer);
            router
                .oneshot(request)
                .await
                .expect("first-send retry route responds")
        }
    };
    let replay = send(body.clone()).await;
    assert_eq!(replay.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        counts(),
        (1, 1, 1, 1, 1, 3),
        "exact retry must not duplicate durable create or turn records"
    );
    assert_eq!(
        receipt_json(),
        admitted_receipt_json,
        "restart replay preserves the exact original composite receipt"
    );

    let conflict = send(serde_json::json!({
        "workspace_id": WORKSPACE_A,
        "title": "Plan a quiet weekend.",
        "content": [{"kind":"TEXT","text":"Different text under the same key."}],
        "resource_refs": [],
        "agent_binding_id": "first-send-binding",
        "presentation_preference": "RICH"
    }))
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        counts(),
        (1, 1, 1, 1, 1, 3),
        "changed payload must not write partial or duplicate rows"
    );
}

#[tokio::test]
async fn new_chat_first_turn_requires_ready_runtime() {
    let directory = tempfile::tempdir().expect("temporary first-turn readiness directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private first-turn readiness directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "first-turn-readiness");
    seed_agent_binding(
        &database,
        WORKSPACE_A,
        "first-turn-readiness-binding",
        true,
        true,
    );
    set_workspace_default_agent(&database, WORKSPACE_A, "first-turn-readiness-binding");
    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());
    let local_incarnation_id = format!("{RUNTIME}-test-incarnation");
    let request_id = "first-turn-runtime-not-ready";
    let body = serde_json::json!({
        "workspace_id": WORKSPACE_A,
        "title": "Do not admit until the local Runtime is ready.",
        "content": [{"kind":"TEXT","text":"Check Runtime admission."}],
        "resource_refs": [],
        "agent_binding_id": "first-turn-readiness-binding",
        "presentation_preference": "AUTO"
    });
    let mut connection =
        rusqlite::Connection::open(&database).expect("open local Runtime readiness fixture");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable readiness fixture constraints");
    let tx = connection
        .transaction()
        .expect("begin Runtime degradation transition");
    tx.execute(
        "UPDATE runtimes SET availability='DEGRADED',version=version+1 WHERE runtime_id=?1",
        [RUNTIME],
    )
    .expect("degrade local Runtime");
    tx.execute(
        "UPDATE runtime_incarnations SET recovery_state='DEGRADED',version=version+1 WHERE runtime_id=?1 AND runtime_incarnation_id=?2",
        rusqlite::params![RUNTIME, local_incarnation_id],
    )
    .expect("degrade local Runtime incarnation");
    tx.commit().expect("commit local Runtime degradation");
    drop(connection);

    let mut request = Request::post("/v1/conversations/first-turn")
        .header("x-workspace-id", WORKSPACE_A)
        .header("idempotency-key", request_id)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("first-turn request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let response = router
        .oneshot(request)
        .await
        .expect("first-turn route responds");
    let (status, error) = read_operator_error(response).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["code"], "RUNTIME_UNAVAILABLE");

    let connection =
        rusqlite::Connection::open(&database).expect("inspect rejected first-turn admission");
    let counts: (i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM conversations WHERE workspace_id=?1),
                (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id IN (SELECT conversation_id FROM conversations WHERE workspace_id=?1)),
                (SELECT COUNT(*) FROM conversation_turns WHERE conversation_id IN (SELECT conversation_id FROM conversations WHERE workspace_id=?1)),
                (SELECT COUNT(*) FROM request_dedup WHERE principal_id=?2 AND request_id=?3),
                (SELECT COUNT(*) FROM conversation_dispatch_intents WHERE conversation_id IN (SELECT conversation_id FROM conversations WHERE workspace_id=?1)),
                (SELECT COUNT(*) FROM domain_events WHERE workspace_id=?1 AND type IN ('conversation.created.v1','conversation.message.added.v1','conversation.turn.created.v2'))",
            rusqlite::params![WORKSPACE_A, OWNER, request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .expect("read rejected first-turn side effects");
    assert_eq!(counts, (0, 0, 0, 0, 0, 0));
}

#[tokio::test]
async fn concurrent_conversation_create_retries_commit_only_one_identity() {
    let directory = tempfile::tempdir().expect("temporary concurrent Conversation directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private concurrent Conversation directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "create-race");
    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());
    let request_id = "conversation-create-race-request-1";

    // Both callers may be retries after a lost acknowledgement. Their generated
    // candidate IDs differ, but the durable receipt must choose one committed row.
    let (first_response, retry_response) = tokio::join!(
        create_conversation_request(&router, WORKSPACE_A, request_id, "Shared request"),
        create_conversation_request(&router, WORKSPACE_A, request_id, "Shared request"),
    );
    let first = read_created_conversation(first_response).await;
    let retry = read_created_conversation(retry_response).await;
    assert_eq!(
        retry, first,
        "concurrent retries resolve to one committed identity"
    );

    let page = read_page(list_page(&router, WORKSPACE_A, None).await).await;
    assert_eq!(page.items, vec![first]);
}

#[tokio::test]
async fn conversation_turn_route_requires_a_pinned_eligible_agent_before_persisting() {
    let directory = tempfile::tempdir().expect("temporary Conversation turn directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation turn directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-workspace");
    seed_conversation(
        &store,
        WORKSPACE_A,
        "a06-conversation-turn",
        "Draft conversation",
        "2026-10-15T12:00:00.000000000Z",
    );
    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());

    let (status, error) =
        read_operator_error(submit_turn(&router, WORKSPACE_A, "a06-conversation-turn").await).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["code"], "AGENT_UNAVAILABLE");
    let conversation = SqliteConversationStore::new(store.clone())
        .get_conversation(OWNER, WORKSPACE_A, "a06-conversation-turn")
        .expect("conversation read succeeds")
        .expect("conversation remains available");
    assert_eq!(
        conversation.version, 1,
        "an unavailable AgentBinding commits no User message, OPEN turn, event, or receipt"
    );
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
        .expect("inspect Conversation storage");
    let (messages, turns, receipts): (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM conversation_messages),
               (SELECT COUNT(*) FROM conversation_turns),
               (SELECT COUNT(*) FROM request_dedup WHERE request_id='conversation-turn-submit-1')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read durable turn counts");
    assert_eq!((messages, turns, receipts), (0, 0, 0));
    assert_no_execution_state(&connection, WORKSPACE_A);
}

#[tokio::test]
async fn conversation_turn_submit_stays_fail_closed_when_runtime_is_ready_without_session_supervisor()
 {
    let directory = tempfile::tempdir().expect("temporary Conversation turn directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation turn directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-ready-runtime-workspace");
    let conversation_id = "a06-conversation-turn-ready-runtime";
    let binding_id = "turn-ready-runtime-binding";
    seed_conversation(
        &store,
        WORKSPACE_A,
        conversation_id,
        "Ready Runtime without native session",
        "2026-10-15T12:00:00.000000000Z",
    );
    bind_conversation_agent(&database, WORKSPACE_A, conversation_id, binding_id);
    seed_ready_runtime_for_conversation(&database, WORKSPACE_A, binding_id);

    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());
    let response = submit_turn_body(
        &router,
        WORKSPACE_A,
        conversation_id,
        r#"{"content":[{"kind":"TEXT","text":"Send only after a native session is ready."}],"resource_refs":[],"agent_binding_id":null,"presentation_preference":"AUTO"}"#,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let (_, error) = read_operator_error(response).await;
    assert_eq!(error["code"], "DEPENDENCY_UNAVAILABLE");
    assert!(
        error["message"]
            .as_str()
            .unwrap_or_default()
            .contains("not sent")
    );

    let connection = rusqlite::Connection::open(&database).expect("read committed turn identity");
    let turn_id: String = connection
        .query_row(
            "SELECT turn_id FROM conversation_turns WHERE conversation_id=?1",
            [conversation_id],
            |row| row.get(0),
        )
        .expect("the unavailable turn remains committed");
    drop(connection);
    let resolved_host = SqliteConversationStore::new(store.clone())
        .resolve_conversation_session_host_for_turn(OWNER, WORKSPACE_A, conversation_id, &turn_id)
        .expect("turn-scoped Runtime inventory resolution succeeds");
    assert_eq!(
        resolved_host.map(|host| host.agent_binding_id),
        Some(binding_id.to_owned()),
        "the committed turn resolves only the binding recorded when it was created"
    );

    let connection = rusqlite::Connection::open(&database)
        .expect("inspect storage after unqualified Runtime submit");
    let (status, session_id): (String, Option<String>) = connection
        .query_row(
            "SELECT status,agent_session_id FROM conversation_turns WHERE conversation_id=?1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("durable turn is present");
    assert_eq!(status, "FAILED");
    assert_eq!(session_id, None);
    let starting_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id=?1 AND type='agent.session.starting.v1'",
            [WORKSPACE_A],
            |row| row.get(0),
        )
        .expect("inspect session-starting events");
    assert_eq!(starting_events, 0);
    assert_no_execution_state(&connection, WORKSPACE_A);
}

#[tokio::test]
async fn conversation_turn_submit_commits_durably_then_settles_unavailable_and_replays_exactly() {
    let directory = tempfile::tempdir().expect("temporary durable Conversation turn directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private durable Conversation turn directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-durable-workspace");
    let conversation_id = "a06-conversation-turn-durable";
    let binding_id = "turn-durable-binding";
    seed_conversation(
        &store,
        WORKSPACE_A,
        conversation_id,
        "Durable conversation",
        "2026-10-15T12:00:00.000000000Z",
    );
    bind_conversation_agent(&database, WORKSPACE_A, conversation_id, binding_id);
    let router = test_router(store, OWNER.to_owned(), RUNTIME.to_owned());
    let request_id = "conversation-turn-durable-request-1";
    const BODY: &str = r#"{"content":[{"kind":"TEXT","text":"Please summarize this project."}],"resource_refs":[],"agent_binding_id":null,"presentation_preference":"SIMPLE"}"#;

    // The response is still a truthful unavailable admission, but the USER message and
    // the turn must already be durable and the turn must not remain OPEN.
    let response =
        submit_turn_body_with_key(&router, WORKSPACE_A, conversation_id, request_id, BODY).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let (status, error) = read_operator_error(response).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["code"], "DEPENDENCY_UNAVAILABLE");
    let message = error["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("was saved"),
        "response names the durable message"
    );
    assert!(
        message.contains("not sent"),
        "response does not claim provider dispatch"
    );

    let connection =
        rusqlite::Connection::open(&database).expect("inspect durable Conversation turn");
    let (turn_id, user_message_id, turn_status, agent_session_id, settled_at, turn_version): (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
    ) = connection
        .query_row(
            "SELECT turn_id,user_message_id,status,agent_session_id,settled_at,version FROM conversation_turns WHERE conversation_id=?1",
            [conversation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("the submitted turn is durable");
    assert_eq!(turn_status, "FAILED");
    assert_eq!(agent_session_id, None);
    assert!(
        settled_at.is_some(),
        "a FAILED turn carries its settlement time"
    );
    assert_eq!(
        turn_version, 2,
        "the failure settlement is a second transaction"
    );
    let presentation: String = connection
        .query_row(
            "SELECT presentation_preference FROM conversation_turns WHERE turn_id=?1",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("read pinned presentation preference");
    assert_eq!(presentation, "SIMPLE");
    let conversation_version: i64 = connection
        .query_row(
            "SELECT version FROM conversations WHERE conversation_id=?1",
            [conversation_id],
            |row| row.get(0),
        )
        .expect("read Conversation version");
    assert_eq!(conversation_version, 2);
    let message_content: String = connection
        .query_row(
            "SELECT content_json FROM conversation_messages WHERE message_id=?1",
            [&user_message_id],
            |row| row.get(0),
        )
        .expect("read durable USER message");
    assert!(message_content.contains("Please summarize this project."));
    let events: Vec<(String, String)> = connection
        .prepare("SELECT type,payload_json FROM domain_events WHERE workspace_id=?1 AND (entity_id=?2 OR entity_id=?3) ORDER BY origin_sequence")
        .expect("prepare event read")
        .query_map(
            rusqlite::params![WORKSPACE_A, conversation_id, turn_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read committed events")
        .collect::<Result<_, _>>()
        .expect("collect committed events");
    assert_eq!(
        events
            .iter()
            .map(|(kind, _)| kind.as_str())
            .collect::<Vec<_>>(),
        [
            "conversation.created.v1",
            "conversation.message.added.v1",
            "conversation.turn.created.v2",
            "conversation.turn.settled.v1",
        ],
        "the durable submit commits its canonical events and one failure settlement"
    );
    assert!(events[2].1.contains(binding_id));
    assert!(events[2].1.contains("SIMPLE"));
    assert!(
        events[3]
            .1
            .contains("CONVERSATION_SESSION_ADMISSION_UNAVAILABLE")
    );
    let receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
            rusqlite::params![OWNER, request_id],
            |row| row.get(0),
        )
        .expect("read idempotency receipt count");
    assert_eq!(receipts, 1);
    assert_no_execution_state(&connection, WORKSPACE_A);

    // Clear the current selection after the first commit. Exact replay belongs to the
    // original request and must return its receipt without revalidating mutable binding
    // state; otherwise a lost response becomes unrecoverable when the agent is disabled.
    let connection =
        rusqlite::Connection::open(&database).expect("open Conversation for binding drift");
    connection
        .execute(
            "UPDATE conversations SET active_agent_binding_id=NULL WHERE conversation_id=?1",
            [conversation_id],
        )
        .expect("remove current Conversation binding after the original commit");
    drop(connection);

    // Exact replay resolves the original admission receipt before mutable binding checks
    // and never retries failed setup or starts another dispatch.
    let replay =
        submit_turn_body_with_key(&router, WORKSPACE_A, conversation_id, request_id, BODY).await;
    assert_eq!(replay.status(), StatusCode::ACCEPTED);
    let replay_bytes = to_bytes(replay.into_body(), 64 * 1024)
        .await
        .expect("read exact replay receipt");
    let replay_receipt: ConversationTurnReceipt =
        serde_json::from_slice(&replay_bytes).expect("parse exact admission receipt");
    assert_eq!(replay_receipt.conversation_id, conversation_id);
    assert_eq!(replay_receipt.turn_id, turn_id);
    assert_eq!(replay_receipt.user_message_id, user_message_id);
    assert_eq!(replay_receipt.status, ConversationTurnStatus::Open);

    let changed_binding = r#"{"content":[{"kind":"TEXT","text":"Please summarize this project."}],"resource_refs":[],"agent_binding_id":"different-binding","presentation_preference":"SIMPLE"}"#;
    let binding_conflict = submit_turn_body_with_key(
        &router,
        WORKSPACE_A,
        conversation_id,
        request_id,
        changed_binding,
    )
    .await;
    assert_eq!(binding_conflict.status(), StatusCode::CONFLICT);
    let (_, binding_conflict_error) = read_operator_error(binding_conflict).await;
    assert_eq!(binding_conflict_error["code"], "IDEMPOTENCY_CONFLICT");

    let connection = rusqlite::Connection::open(&database).expect("inspect storage after replay");
    let (messages, turns, receipts, turn_events, settlements): (i64, i64, i64, i64, i64) =
        connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id=?1),
                   (SELECT COUNT(*) FROM conversation_turns WHERE conversation_id=?1),
                   (SELECT COUNT(*) FROM request_dedup WHERE principal_id=?2 AND request_id=?3),
                   (SELECT COUNT(*) FROM domain_events WHERE type='conversation.turn.created.v2' AND workspace_id=?4),
                   (SELECT COUNT(*) FROM domain_events WHERE type='conversation.turn.settled.v1' AND workspace_id=?4)",
                rusqlite::params![conversation_id, OWNER, request_id, WORKSPACE_A],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .expect("read counts after replay");
    assert_eq!((messages, turns, receipts), (1, 1, 1));
    assert_eq!((turn_events, settlements), (1, 1));
    let conversation_version: i64 = connection
        .query_row(
            "SELECT version FROM conversations WHERE conversation_id=?1",
            [conversation_id],
            |row| row.get(0),
        )
        .expect("read Conversation version after replay");
    assert_eq!(conversation_version, 2);
    assert_no_execution_state(&connection, WORKSPACE_A);

    // A changed payload under the same key conflicts without another durable write.
    let conflict = submit_turn_body_with_key(
        &router,
        WORKSPACE_A,
        conversation_id,
        request_id,
        r#"{"content":[{"kind":"TEXT","text":"Different message under the same key."}],"resource_refs":[],"agent_binding_id":null,"presentation_preference":"SIMPLE"}"#,
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let (_, conflict_error) = read_operator_error(conflict).await;
    assert_eq!(conflict_error["code"], "CONFLICT");
    let connection = rusqlite::Connection::open(&database).expect("inspect storage after conflict");
    let (messages, turns, receipts): (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id=?1),
               (SELECT COUNT(*) FROM conversation_turns WHERE conversation_id=?1),
               (SELECT COUNT(*) FROM request_dedup WHERE principal_id=?2 AND request_id=?3)",
            rusqlite::params![conversation_id, OWNER, request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read counts after changed-payload conflict");
    assert_eq!((messages, turns, receipts), (1, 1, 1));
}

#[tokio::test]
async fn existing_turn_exact_replay_precedes_resource_revocation_and_binding_drift() {
    let directory = tempfile::tempdir().expect("temporary send replay directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private send replay directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-resource-replay");
    let conversation_id = "a06-conversation-resource-replay";
    let resource_id = "a06-send-resource";
    let binding_id = "a06-send-binding";
    let revision_id = seed_text_resource(&store, WORKSPACE_A, resource_id);
    seed_conversation(
        &store,
        WORKSPACE_A,
        conversation_id,
        "Resource replay",
        "2026-10-15T12:00:00.000000000Z",
    );
    bind_conversation_agent(&database, WORKSPACE_A, conversation_id, binding_id);
    let router = test_router(store, OWNER.to_owned(), RUNTIME.to_owned());
    let request_id = "conversation-resource-replay-request";
    let body = serde_json::json!({
        "content": [{"kind": "TEXT", "text": "Summarize the attached file."}],
        "resource_refs": [{
            "workspace_id": WORKSPACE_A,
            "resource_id": resource_id,
            "revision_id": revision_id
        }],
        "agent_binding_id": binding_id,
        "presentation_preference": "RICH"
    })
    .to_string();

    let first =
        submit_turn_body_with_key(&router, WORKSPACE_A, conversation_id, request_id, &body).await;
    assert_eq!(first.status(), StatusCode::SERVICE_UNAVAILABLE);
    let (_, first_error) = read_operator_error(first).await;
    assert_eq!(first_error["code"], "DEPENDENCY_UNAVAILABLE");

    let connection = rusqlite::Connection::open(&database).expect("open send replay database");
    connection
        .execute(
            "UPDATE resources SET sensitivity='RESTRICTED' WHERE workspace_id=?1 AND resource_id=?2",
            rusqlite::params![WORKSPACE_A, resource_id],
        )
        .expect("revoke Resource for new admission");
    connection
        .execute(
            "UPDATE conversations SET active_agent_binding_id=NULL WHERE conversation_id=?1",
            [conversation_id],
        )
        .expect("change current Conversation binding after original admission");
    drop(connection);

    let replay =
        submit_turn_body_with_key(&router, WORKSPACE_A, conversation_id, request_id, &body).await;
    assert_eq!(replay.status(), StatusCode::ACCEPTED);
    let replay_bytes = to_bytes(replay.into_body(), 64 * 1024)
        .await
        .expect("read replay receipt");
    let replay_receipt: ConversationTurnReceipt =
        serde_json::from_slice(&replay_bytes).expect("parse original admission receipt");
    assert_eq!(replay_receipt.status, ConversationTurnStatus::Open);

    let changed_binding = serde_json::json!({
        "content": [{"kind": "TEXT", "text": "Summarize the attached file."}],
        "resource_refs": [{
            "workspace_id": WORKSPACE_A,
            "resource_id": resource_id,
            "revision_id": revision_id
        }],
        "agent_binding_id": "different-binding",
        "presentation_preference": "RICH"
    })
    .to_string();
    let conflict = submit_turn_body_with_key(
        &router,
        WORKSPACE_A,
        conversation_id,
        request_id,
        &changed_binding,
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let (_, conflict_error) = read_operator_error(conflict).await;
    assert_eq!(conflict_error["code"], "IDEMPOTENCY_CONFLICT");

    let fresh = submit_turn_body_with_key(
        &router,
        WORKSPACE_A,
        conversation_id,
        "conversation-resource-replay-fresh-request",
        &body,
    )
    .await;
    assert_eq!(fresh.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let (_, fresh_error) = read_operator_error(fresh).await;
    assert_eq!(fresh_error["code"], "INVALID_RESOURCE_REF");

    let connection = rusqlite::Connection::open(&database).expect("inspect persisted turn");
    let (messages, turns, turn_events): (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM conversation_messages WHERE conversation_id=?1),
               (SELECT COUNT(*) FROM conversation_turns WHERE conversation_id=?1),
               (SELECT COUNT(*) FROM domain_events
                WHERE entity_type='ConversationTurn'
                  AND entity_id=(SELECT turn_id FROM conversation_turns WHERE conversation_id=?1)
                  AND type='conversation.turn.created.v2')",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read original committed records");
    assert_eq!((messages, turns, turn_events), (1, 1, 1));
}

#[tokio::test]
async fn conversation_turn_submit_denies_foreign_owner_and_unauthenticated_callers_without_writing()
{
    let directory =
        tempfile::tempdir().expect("temporary Conversation turn authorization directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation turn authorization directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-authorization");
    let conversation_id = "a06-conversation-turn-authorization";
    seed_conversation(
        &store,
        WORKSPACE_A,
        conversation_id,
        "Authorized conversation",
        "2026-10-15T12:00:00.000000000Z",
    );
    bind_conversation_agent(
        &database,
        WORKSPACE_A,
        conversation_id,
        "turn-authorization-binding",
    );

    let unauthenticated_router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());
    let unauthenticated = submit_turn_with(
        &unauthenticated_router,
        WORKSPACE_A,
        conversation_id,
        "conversation-turn-unauthenticated",
        r#"{"content":[{"kind":"TEXT","text":"No peer identity."}]}"#,
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let foreign_owner_router = test_router(
        store,
        "a06-pagination-foreign-owner".to_owned(),
        RUNTIME.to_owned(),
    );
    let foreign_owner = submit_turn_body_with_key(
        &foreign_owner_router,
        WORKSPACE_A,
        conversation_id,
        "conversation-turn-foreign-owner",
        r#"{"content":[{"kind":"TEXT","text":"A foreign owner must not submit."}]}"#,
    )
    .await;
    assert_eq!(foreign_owner.status(), StatusCode::NOT_FOUND);

    let connection =
        rusqlite::Connection::open(&database).expect("inspect authorized Conversation storage");
    let (messages, turns, receipts): (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM conversation_messages),
               (SELECT COUNT(*) FROM conversation_turns),
               (SELECT COUNT(*) FROM request_dedup WHERE request_id IN ('conversation-turn-unauthenticated','conversation-turn-foreign-owner'))",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read counts after denied submissions");
    assert_eq!((messages, turns, receipts), (0, 0, 0));
    assert_no_execution_state(&connection, WORKSPACE_A);
}

#[tokio::test]
async fn conversation_turn_route_uses_the_workspace_default_binding_when_the_conversation_has_no_override()
 {
    let directory = tempfile::tempdir().expect("temporary Conversation default directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation default directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-workspace-default");
    let conversation_id = "a06-conversation-turn-workspace-default";
    seed_conversation(
        &store,
        WORKSPACE_A,
        conversation_id,
        "Default conversation",
        "2026-10-15T12:00:00.000000000Z",
    );
    // The Conversation has no override of its own, so the Workspace default is the
    // admitted selection.
    seed_agent_binding(&database, WORKSPACE_A, "turn-default-binding", true, true);
    set_workspace_default_agent(&database, WORKSPACE_A, "turn-default-binding");
    let router = test_router(store, OWNER.to_owned(), RUNTIME.to_owned());

    let response = submit_turn_body(
        &router,
        WORKSPACE_A,
        conversation_id,
        r#"{"content":[{"kind":"TEXT","text":"Please summarize this project."}],"resource_refs":[],"agent_binding_id":null,"presentation_preference":"AUTO"}"#,
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "the durable turn is still not dispatched to a native agent"
    );
    let (_, error) = read_operator_error(response).await;
    assert_eq!(error["code"], "DEPENDENCY_UNAVAILABLE");

    let connection =
        rusqlite::Connection::open(&database).expect("inspect the Workspace default turn");
    let (turn_status, binding_payload): (String, String) = connection
        .query_row(
            "SELECT
               (SELECT status FROM conversation_turns WHERE conversation_id=?1),
               (SELECT payload_json FROM domain_events WHERE type='conversation.turn.created.v2' AND entity_id=(SELECT turn_id FROM conversation_turns WHERE conversation_id=?1))",
            rusqlite::params![conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read the durable turn and its creation event");
    assert_eq!(turn_status, "FAILED");
    assert!(
        binding_payload.contains("turn-default-binding"),
        "the turn records the AgentBinding it was admitted against"
    );
    assert_no_execution_state(&connection, WORKSPACE_A);
}

#[tokio::test]
async fn conversation_turn_route_refuses_an_ineligible_conversation_override_without_falling_back()
{
    let directory = tempfile::tempdir().expect("temporary Conversation override directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation override directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-override-workspace");
    let conversation_id = "a06-conversation-turn-override";
    seed_conversation(
        &store,
        WORKSPACE_A,
        conversation_id,
        "Override conversation",
        "2026-10-15T12:00:00.000000000Z",
    );
    // The Conversation's own override is configured but no longer enabled. It stays
    // authoritative: the route must not silently use the Workspace default instead.
    seed_agent_binding(&database, WORKSPACE_A, "turn-override-binding", false, true);
    set_conversation_agent(
        &database,
        WORKSPACE_A,
        conversation_id,
        "turn-override-binding",
    );
    seed_agent_binding(&database, WORKSPACE_A, "turn-fallback-binding", true, true);
    set_workspace_default_agent(&database, WORKSPACE_A, "turn-fallback-binding");
    let router = test_router(store, OWNER.to_owned(), RUNTIME.to_owned());

    let (status, error) =
        read_operator_error(submit_turn(&router, WORKSPACE_A, conversation_id).await).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["code"], "AGENT_UNAVAILABLE");

    let connection =
        rusqlite::Connection::open(&database).expect("inspect the refused Conversation turn");
    let (messages, turns, receipts): (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM conversation_messages),
               (SELECT COUNT(*) FROM conversation_turns),
               (SELECT COUNT(*) FROM request_dedup WHERE request_id='conversation-turn-submit-1')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read durable turn counts");
    assert_eq!(
        (messages, turns, receipts),
        (0, 0, 0),
        "an ineligible selection persists no message, turn, event, or receipt"
    );
    assert_no_execution_state(&connection, WORKSPACE_A);
}

#[tokio::test]
async fn conversation_turn_route_rejects_cross_workspace_resource_pins_before_provider_gate() {
    let directory = tempfile::tempdir().expect("temporary Conversation turn validation directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation turn validation directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-validation-a");
    seed_conversation(
        &store,
        WORKSPACE_A,
        "a06-conversation-turn-validation",
        "Draft conversation",
        "2026-10-15T12:00:00.000000000Z",
    );
    let router = test_router(store, OWNER.to_owned(), RUNTIME.to_owned());

    let response = submit_turn_body(
        &router,
        WORKSPACE_A,
        "a06-conversation-turn-validation",
        r#"{"content":[{"kind":"RESOURCE","resource_ref":{"workspace_id":"foreign-workspace","resource_id":"resource-1","revision_id":"revision-1"}}],"resource_refs":[]}"#,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn conversation_agent_binding_route_commits_replays_clears_and_rejects_stale_or_ineligible_choices()
 {
    let directory = tempfile::tempdir().expect("temporary Conversation AgentBinding directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation AgentBinding directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "agent-binding-a");
    create_workspace(&store, WORKSPACE_B, "agent-binding-b");
    seed_conversation(
        &store,
        WORKSPACE_A,
        "conversation-agent-binding-route",
        "Agent choice",
        "2026-10-15T12:00:00.000000000Z",
    );
    seed_agent_binding(&database, WORKSPACE_A, "eligible-binding", true, true);
    seed_agent_binding(&database, WORKSPACE_A, "disabled-binding", false, true);
    seed_agent_binding(&database, WORKSPACE_A, "non-lead-binding", true, false);
    seed_agent_binding(&database, WORKSPACE_B, "foreign-binding", true, true);
    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());

    let request_id = "conversation-agent-binding-set-request";
    let first = read_updated_conversation(
        set_conversation_agent_binding_request(
            &router,
            WORKSPACE_A,
            "conversation-agent-binding-route",
            1,
            request_id,
            serde_json::json!("eligible-binding"),
        )
        .await,
    )
    .await;
    assert_eq!(
        first.active_agent_binding_id.as_deref(),
        Some("eligible-binding")
    );
    assert_eq!(first.version, 2);

    let replay = read_updated_conversation(
        set_conversation_agent_binding_request(
            &router,
            WORKSPACE_A,
            "conversation-agent-binding-route",
            1,
            request_id,
            serde_json::json!("eligible-binding"),
        )
        .await,
    )
    .await;
    assert_eq!(replay, first, "an exact retry returns the original commit");

    let reused_key = set_conversation_agent_binding_request(
        &router,
        WORKSPACE_A,
        "conversation-agent-binding-route",
        1,
        request_id,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(
        reused_key.status(),
        StatusCode::CONFLICT,
        "one idempotency key cannot describe two choices"
    );

    let cleared = read_updated_conversation(
        set_conversation_agent_binding_request(
            &router,
            WORKSPACE_A,
            "conversation-agent-binding-route",
            2,
            "conversation-agent-binding-clear-request",
            serde_json::Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(cleared.active_agent_binding_id, None);
    assert_eq!(cleared.version, 3);

    let stale = set_conversation_agent_binding_request(
        &router,
        WORKSPACE_A,
        "conversation-agent-binding-route",
        2,
        "conversation-agent-binding-stale-request",
        serde_json::json!("eligible-binding"),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);

    for (binding, key) in [
        (
            "disabled-binding",
            "conversation-agent-binding-disabled-request",
        ),
        (
            "non-lead-binding",
            "conversation-agent-binding-non-lead-request",
        ),
        (
            "foreign-binding",
            "conversation-agent-binding-foreign-request",
        ),
    ] {
        let rejected = set_conversation_agent_binding_request(
            &router,
            WORKSPACE_A,
            "conversation-agent-binding-route",
            3,
            key,
            serde_json::json!(binding),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    let final_record = SqliteConversationStore::new(store.clone())
        .get_conversation(OWNER, WORKSPACE_A, "conversation-agent-binding-route")
        .expect("read Conversation")
        .expect("Conversation remains present");
    assert_eq!(final_record, cleared);
    let connection = rusqlite::Connection::open(&database).expect("open committed SQLite");
    let (event_count, final_event_type, payload): (i64, String, String) = connection
        .query_row(
            "SELECT COUNT(*), MAX(type), MAX(payload_json) FROM domain_events WHERE entity_type='Conversation' AND entity_id=?1 AND type='conversation.agent_binding.changed.v1'",
            ["conversation-agent-binding-route"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read binding-change events");
    assert_eq!(event_count, 2, "replay and rejected writes emit no event");
    assert_eq!(final_event_type, "conversation.agent_binding.changed.v1");
    let payload: serde_json::Value = serde_json::from_str(&payload).expect("event payload is JSON");
    assert_eq!(payload["from_agent_binding_id"], "eligible-binding");
    assert_eq!(payload["to_agent_binding_id"], serde_json::Value::Null);
    assert_eq!(payload["aggregate_version"], 3);
}

#[tokio::test]
async fn conversation_turn_submit_returns_accepted_receipt_with_active_session_supervisor() {
    let directory = tempfile::tempdir().expect("temporary supervisor test directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private test directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-supervisor-workspace");
    let conversation_id = "a06-conversation-turn-supervisor";
    let binding_id = "turn-supervisor-binding";
    seed_conversation(
        &store,
        WORKSPACE_A,
        conversation_id,
        "Supervisor conversation",
        "2026-10-15T12:00:00.000000000Z",
    );
    bind_conversation_agent(&database, WORKSPACE_A, conversation_id, binding_id);
    seed_ready_runtime_for_conversation(&database, WORKSPACE_A, binding_id);

    let supervisor = Arc::new(crate::agents::AgentSessionSupervisor::new(
        store.clone(),
        RUNTIME.to_owned(),
    ));
    let router = test_router_with_supervisor(
        store.clone(),
        OWNER.to_owned(),
        RUNTIME.to_owned(),
        "turn-supervisor-local-incarnation".to_owned(),
        supervisor,
    );

    let request_id = "conv-turn-supervisor-req-1";
    let body = r#"{"content":[{"kind":"TEXT","text":"Hello native supervisor."}],"resource_refs":[],"agent_binding_id":null,"presentation_preference":"AUTO"}"#;
    let response =
        submit_turn_body_with_key(&router, WORKSPACE_A, conversation_id, request_id, body).await;
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "the turn is accepted for dispatch"
    );

    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read receipt body");
    let receipt: ConversationTurnReceipt = serde_json::from_slice(&bytes).expect("parse receipt");
    assert_eq!(receipt.conversation_id, conversation_id);
    assert_eq!(
        receipt.status,
        storage_core::conversation::ConversationTurnStatus::Open
    );
    assert_eq!(
        receipt.presentation_preference,
        storage_core::conversation::PresentationPreference::Auto
    );

    // Replay with exact same request key returns 202 Accepted with identical turn receipt
    let replay_response =
        submit_turn_body_with_key(&router, WORKSPACE_A, conversation_id, request_id, body).await;
    assert_eq!(
        replay_response.status(),
        StatusCode::ACCEPTED,
        "exact replay returns 202 Accepted"
    );
    let replay_bytes = to_bytes(replay_response.into_body(), 64 * 1024)
        .await
        .expect("read replay receipt body");
    let replay_receipt: ConversationTurnReceipt =
        serde_json::from_slice(&replay_bytes).expect("parse replay receipt");
    assert_eq!(replay_receipt.turn_id, receipt.turn_id);
    assert_eq!(replay_receipt.conversation_id, receipt.conversation_id);
    assert_eq!(replay_receipt.user_message_id, receipt.user_message_id);
}

fn single_flight_rows(connection: &rusqlite::Connection) -> (i64, i64, i64, i64) {
    connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM conversation_messages),(SELECT COUNT(*) FROM conversation_turns),(SELECT COUNT(*) FROM domain_events),(SELECT COUNT(*) FROM request_dedup)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read single-flight row counts")
}

#[tokio::test]
async fn conversation_turn_submit_returns_409_while_another_turn_is_in_flight_and_writes_nothing() {
    let directory = tempfile::tempdir().expect("temporary single-flight directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private test directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "turn-single-flight-workspace");
    let conversation_id = "a06-conversation-single-flight";
    let binding_id = "single-flight-binding";
    seed_conversation(
        &store,
        WORKSPACE_A,
        conversation_id,
        "Single flight",
        "2026-10-15T12:00:00.000000000Z",
    );
    bind_conversation_agent(&database, WORKSPACE_A, conversation_id, binding_id);
    seed_ready_runtime_for_conversation(&database, WORKSPACE_A, binding_id);
    let connection = rusqlite::Connection::open(&database).expect("open seeded database");
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,agent_session_id,agent_binding_id,turn_id,content_json,resource_refs_json,source_channel_ref_json,created_at) VALUES('seeded-user-message',?1,'{\"kind\":\"USER\"}','USER',NULL,NULL,NULL,'[]','[]',NULL,'2026-10-15T12:00:01Z')",
            [conversation_id],
        )
        .expect("seed the in-flight turn message");
    connection
        .execute(
            "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,agent_session_id,status,retry_ordinal,created_at,settled_at,version) VALUES('seeded-turn',?1,'seeded-user-message',NULL,'WAITING_USER',0,'2026-10-15T12:00:01Z',NULL,1)",
            [conversation_id],
        )
        .expect("seed the in-flight turn");
    let before = single_flight_rows(&connection);

    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());
    let response = submit_turn_body_with_key(
        &router,
        WORKSPACE_A,
        conversation_id,
        "single-flight-second-submit",
        r#"{"content":[{"kind":"TEXT","text":"A second message while one is running."}],"resource_refs":[],"agent_binding_id":null,"presentation_preference":"AUTO"}"#,
    )
    .await;
    let (status, error) = read_operator_error(response).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "CONVERSATION_TURN_IN_PROGRESS");
    let after = single_flight_rows(&connection);
    assert_eq!(after, before, "the rejected submit writes nothing");
}

struct TurnCommandFixture {
    _directory: tempfile::TempDir,
    database: std::path::PathBuf,
    store: SqliteWorkspaceStore,
    router: Router,
    supervisor: Arc<crate::agents::AgentSessionSupervisor>,
    conversation_id: &'static str,
}

fn turn_command_fixture(suffix: &str) -> TurnCommandFixture {
    let directory = tempfile::tempdir().expect("temporary turn command directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private test directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        database.clone(),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, suffix);
    let conversation_id = "a06-conversation-turn-commands";
    let binding_id = "turn-command-binding";
    seed_conversation(
        &store,
        WORKSPACE_A,
        conversation_id,
        "Turn commands",
        "2026-10-15T12:00:00.000000000Z",
    );
    bind_conversation_agent(&database, WORKSPACE_A, conversation_id, binding_id);
    seed_ready_runtime_for_conversation(&database, WORKSPACE_A, binding_id);
    let mut local_runtime =
        rusqlite::Connection::open(&database).expect("open daemon Runtime fence fixture");
    local_runtime
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable daemon Runtime fence fixture constraints");
    let local_runtime_tx = local_runtime
        .transaction()
        .expect("begin daemon Runtime fence fixture");
    local_runtime_tx
        .execute(
            "INSERT INTO runtimes(runtime_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,resource_capacity_json,last_seen,version) VALUES(?1,'{}','0.1.0','linux','x86_64','[]','LOCAL','ONLINE','MANUAL',?2,'{}','2026-10-15T12:00:00.000000000Z',1)",
            rusqlite::params![RUNTIME, TURN_COMMAND_DAEMON_INCARNATION],
        )
        .expect("seed online daemon Runtime");
    local_runtime_tx
        .execute(
            "INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state,ready_at,stopped_at,version) VALUES(?1,?2,'2026-10-15T12:00:00.000000000Z','0.1.0',0,'READY','2026-10-15T12:00:00.000000000Z',NULL,1)",
            rusqlite::params![TURN_COMMAND_DAEMON_INCARNATION, RUNTIME],
        )
        .expect("seed ready daemon Runtime incarnation");
    local_runtime_tx
        .commit()
        .expect("commit daemon Runtime fence fixture");
    let supervisor = Arc::new(crate::agents::AgentSessionSupervisor::new(
        store.clone(),
        RUNTIME.to_owned(),
    ));
    // No dispatch is spawned by the seeded turns; the router only needs the supervisor
    // handle to signal Stop. Retry spawns a dispatch that fails closed without a host.
    let router = test_router_with_supervisor(
        store.clone(),
        OWNER.to_owned(),
        RUNTIME.to_owned(),
        TURN_COMMAND_DAEMON_INCARNATION.to_owned(),
        Arc::clone(&supervisor),
    );
    TurnCommandFixture {
        _directory: directory,
        database,
        store,
        router,
        supervisor,
        conversation_id,
    }
}

async fn admitted_turn_for_dispatch_fixture(
    fixture: &TurnCommandFixture,
    request_id: &str,
) -> (storage_core::conversation::ConversationTurn, String) {
    use storage_core::conversation::RuntimeExecutionFence;
    use storage_core::{
        AgentHostInstanceRecord, AgentHostStore, ConversationMessageRecord,
        CreateConversationTurnCommit,
    };

    // The fixture intentionally bypasses the HTTP route because `test_router` has no
    // supervisor and correctly returns a settled 503 after admission. Commit the same
    // canonical admission records here so the test can invoke the supervisor directly.
    let turn_id = format!("turn-command-admitted-{request_id}");
    let message_id = format!("{turn_id}-user-message");
    let correlation_id = format!("{turn_id}-correlation");
    let content = serde_json::json!([{
        "kind": "TEXT",
        "text": "Dispatch fixture input."
    }]);
    let resource_refs = serde_json::json!([]);
    let author = serde_json::json!({"kind": "USER", "principal_id": OWNER});
    let payload = serde_json::json!({
        "operation": "conversation.turn.create.v1",
        "workspace_id": WORKSPACE_A,
        "conversation_id": fixture.conversation_id,
        "turn_id": turn_id,
        "message_id": message_id,
        "author": author,
        "content": content,
        "resource_refs": resource_refs,
        "presentation_preference": "AUTO",
    });
    let message_context = event_context(RUNTIME, &correlation_id).expect("message event context");
    let turn_context = event_context(RUNTIME, &correlation_id).expect("turn event context");
    let message = ConversationMessageRecord {
        message_id: message_id.clone(),
        conversation_id: fixture.conversation_id.to_owned(),
        author: author.clone(),
        role: "USER".to_owned(),
        agent_session_id: None,
        agent_binding_id: None,
        turn_id: Some(turn_id.clone()),
        content: content.clone(),
        resource_refs: resource_refs.clone(),
        source_channel_ref: None,
        created_at: message_context.recorded_at.clone(),
    };
    let turn = storage_core::conversation::ConversationTurn {
        turn_id: turn_id.clone(),
        conversation_id: fixture.conversation_id.to_owned(),
        user_message_id: message_id.clone(),
        agent_session_id: None,
        status: storage_core::conversation::ConversationTurnStatus::Open,
        retry_ordinal: 0,
        presentation_preference: storage_core::conversation::PresentationPreference::Auto,
        created_at: message_context.recorded_at.clone(),
        settled_at: None,
        version: 1,
    };
    SqliteConversationStore::new(fixture.store.clone())
        .create_conversation_turn(CreateConversationTurnCommit {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: request_id.to_owned(),
                request_payload: payload,
            },
            principal_id: OWNER.to_owned(),
            workspace_id: WORKSPACE_A.to_owned(),
            runtime_fence: RuntimeExecutionFence {
                runtime_id: RUNTIME.to_owned(),
                runtime_incarnation_id: TURN_COMMAND_DAEMON_INCARNATION.to_owned(),
            },
            message,
            turn,
            message_event: EventDraft {
                event_id: message_context.event_id,
                workspace_id: WORKSPACE_A.to_owned(),
                entity_type: "Conversation".to_owned(),
                entity_id: fixture.conversation_id.to_owned(),
                origin_runtime_id: message_context.origin_runtime_id,
                entity_revision: 2,
                hlc_timestamp: message_context.hlc_timestamp,
                correlation_id: message_context.correlation_id,
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.message.added.v1".to_owned(),
                payload: serde_json::json!({
                    "message_id": message_id,
                    "conversation_id": fixture.conversation_id,
                    "author": author,
                    "content_digest": canonical_digest(&content).expect("content digest"),
                    "resource_refs": resource_refs,
                }),
                recorded_at: message_context.recorded_at,
            },
            turn_event: EventDraft {
                event_id: turn_context.event_id,
                workspace_id: WORKSPACE_A.to_owned(),
                entity_type: "ConversationTurn".to_owned(),
                entity_id: turn_id.clone(),
                origin_runtime_id: turn_context.origin_runtime_id,
                entity_revision: 1,
                hlc_timestamp: turn_context.hlc_timestamp,
                correlation_id: turn_context.correlation_id,
                causation_id: None,
                schema_version: 1,
                event_type: "conversation.turn.created.v2".to_owned(),
                payload: serde_json::json!({
                    "turn_id": turn_id,
                    "conversation_id": fixture.conversation_id,
                    "user_message_id": message_id,
                    "agent_binding_id": "turn-command-binding",
                    "aggregate_version": 1,
                    "presentation_preference": "AUTO",
                }),
                recorded_at: turn_context.recorded_at,
            },
        })
        .expect("durable USER message, OPEN turn and dispatch intent commit together");
    // Model a host that has already completed its own startup/readiness transition.
    // The supervisor's lazy host-create branch is a distinct earlier stage and is not
    // the session-reservation failure this fixture is intended to exercise.
    let host_instance_id = "host-turn-command-binding-endpoint";
    fixture
        .store
        .create_agent_host_instance(AgentHostInstanceRecord {
            host_instance_id: host_instance_id.to_owned(),
            runtime_id: "turn-command-binding-runtime".to_owned(),
            runtime_incarnation_id: "turn-command-binding-incarnation".to_owned(),
            agent_profile_id: "turn-command-binding-profile".to_owned(),
            endpoint_id: "turn-command-binding-endpoint".to_owned(),
            hosting_mode: "LOCAL_PER_SESSION".to_owned(),
            state: "STARTING".to_owned(),
            process_identity_ref: None,
            ownership: "LITECOWORK".to_owned(),
            started_at: "2026-10-15T12:00:00.000000000Z".to_owned(),
            last_used_at: "2026-10-15T12:00:00.000000000Z".to_owned(),
            idle_since: None,
        })
        .expect("seed the AgentHost lifecycle at STARTING");
    fixture
        .store
        .transition_agent_host_instance(
            "turn-command-binding-runtime",
            "turn-command-binding-incarnation",
            host_instance_id,
            "STARTING",
            "READY",
            "2026-10-15T12:00:01.000000000Z",
            Some("fixture-host-process"),
        )
        .expect("host readiness is observed before Conversation dispatch");
    let turn = SqliteConversationStore::new(fixture.store.clone())
        .get_conversation_turn(WORKSPACE_A, fixture.conversation_id, &turn_id)
        .expect("load admitted turn")
        .expect("admitted turn exists");
    (turn, turn_id)
}

#[tokio::test]
async fn conversation_dispatch_host_resolution_error_settles_the_admitted_attempt() {
    use storage_core::conversation::RuntimeExecutionFence;
    use storage_core::{
        ConversationMessageRecord, CreateConversationTurnCommit, WorkspaceCreateRequest,
    };

    let fixture = turn_command_fixture("turn-early-host-resolution-error");
    let turn_id = "turn-early-host-resolution-error";
    let message_id = "turn-early-host-resolution-user";
    let correlation_id = "turn-early-host-resolution-correlation";
    let content = serde_json::json!([{"kind":"TEXT","text":"Fault-injected setup failure."}]);
    let resource_refs = serde_json::json!([]);
    let author = serde_json::json!({"kind":"USER","principal_id":OWNER});
    let request_payload = serde_json::json!({
        "operation": "conversation.turn.create.v1",
        "workspace_id": WORKSPACE_A,
        "conversation_id": fixture.conversation_id,
        "turn_id": turn_id,
        "message_id": message_id,
        "author": author,
        "content": content,
        "resource_refs": resource_refs,
        "presentation_preference": "AUTO",
    });
    let message_context = event_context(RUNTIME, correlation_id).expect("message event context");
    let turn_context = event_context(RUNTIME, correlation_id).expect("turn event context");
    let message = ConversationMessageRecord {
        message_id: message_id.to_owned(),
        conversation_id: fixture.conversation_id.to_owned(),
        author: author.clone(),
        role: "USER".to_owned(),
        agent_session_id: None,
        agent_binding_id: None,
        turn_id: Some(turn_id.to_owned()),
        content: content.clone(),
        resource_refs: resource_refs.clone(),
        source_channel_ref: None,
        created_at: message_context.recorded_at.clone(),
    };
    let turn = ConversationTurn {
        turn_id: turn_id.to_owned(),
        conversation_id: fixture.conversation_id.to_owned(),
        user_message_id: message_id.to_owned(),
        agent_session_id: None,
        status: ConversationTurnStatus::Open,
        retry_ordinal: 0,
        presentation_preference: PresentationPreference::Auto,
        created_at: message_context.recorded_at.clone(),
        settled_at: None,
        version: 1,
    };
    let commit = CreateConversationTurnCommit {
        request: WorkspaceCreateRequest {
            principal_id: OWNER.to_owned(),
            request_id: "turn-early-host-resolution-request".to_owned(),
            request_payload,
        },
        principal_id: OWNER.to_owned(),
        workspace_id: WORKSPACE_A.to_owned(),
        runtime_fence: RuntimeExecutionFence {
            runtime_id: RUNTIME.to_owned(),
            runtime_incarnation_id: TURN_COMMAND_DAEMON_INCARNATION.to_owned(),
        },
        message,
        turn,
        message_event: EventDraft {
            event_id: message_context.event_id,
            workspace_id: WORKSPACE_A.to_owned(),
            entity_type: "Conversation".to_owned(),
            entity_id: fixture.conversation_id.to_owned(),
            origin_runtime_id: message_context.origin_runtime_id,
            entity_revision: 2,
            hlc_timestamp: message_context.hlc_timestamp,
            correlation_id: message_context.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.message.added.v1".to_owned(),
            payload: serde_json::json!({
                "message_id": message_id,
                "conversation_id": fixture.conversation_id,
                "author": author,
                "content_digest": canonical_digest(&content).expect("content digest"),
                "resource_refs": resource_refs,
            }),
            recorded_at: message_context.recorded_at,
        },
        turn_event: EventDraft {
            event_id: turn_context.event_id,
            workspace_id: WORKSPACE_A.to_owned(),
            entity_type: "ConversationTurn".to_owned(),
            entity_id: turn_id.to_owned(),
            origin_runtime_id: turn_context.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: turn_context.hlc_timestamp,
            correlation_id: turn_context.correlation_id,
            causation_id: None,
            schema_version: 1,
            event_type: "conversation.turn.created.v2".to_owned(),
            payload: serde_json::json!({
                "turn_id": turn_id,
                "conversation_id": fixture.conversation_id,
                "user_message_id": message_id,
                "agent_binding_id": "turn-command-binding",
                "aggregate_version": 1,
                "presentation_preference": "AUTO",
            }),
            recorded_at: turn_context.recorded_at,
        },
    };
    let admitted = SqliteConversationStore::new(fixture.store.clone())
        .create_conversation_turn(commit)
        .expect("USER, OPEN turn and dispatch intent commit together");

    // Force the first supervisor setup lookup to fail while leaving the durable turn and
    // all records required for settlement intact.
    fixture
        .connection()
        .execute_batch("DROP TABLE runtime_offers")
        .expect("remove the probed offer table to inject host-resolution failure");
    let failure = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            admitted.turn,
            correlation_id,
        )
        .await;
    assert_eq!(
        failure,
        Err(crate::agents::supervisor::DispatchFailureClass::HostResolution)
    );
    assert_eq!(
        fixture.turn_row(turn_id).0,
        "FAILED",
        "post-admission host lookup errors cannot strand an OPEN turn"
    );
    assert_eq!(fixture.intents(turn_id), vec![(0, "SETTLED".to_owned())]);
    let reason: String = fixture
        .connection()
        .query_row(
            "SELECT json_extract(payload_json,'$.reason_code') FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1 AND type='conversation.turn.settled.v1'",
            [turn_id],
            |row| row.get(0),
        )
        .expect("read the typed settlement reason");
    assert_eq!(reason, "CONVERSATION_SESSION_ADMISSION_UNAVAILABLE");
}

#[tokio::test]
async fn conversation_dispatch_host_unavailable_after_admission_settles_attempt() {
    let fixture = turn_command_fixture("turn-host-unavailable-after-admission");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-host-unavailable-after-admission").await;

    // Admission observed an eligible endpoint. Model the offer becoming
    // unauthenticated before asynchronous dispatch resolves the pinned host.
    // The admitted USER message must remain durable, with no session or answer.
    let changed = fixture
        .connection()
        .execute(
            "UPDATE runtime_offers SET readiness='NEEDS_AUTH' WHERE runtime_id=?1 AND runtime_incarnation_id=?2 AND offer_kind='AGENT_ENDPOINT' AND offer_ref=?3",
            rusqlite::params![
                "turn-command-binding-runtime",
                "turn-command-binding-incarnation",
                "turn-command-binding-endpoint",
            ],
        )
        .expect("make the pinned endpoint unavailable after admission");
    assert_eq!(changed, 1, "the fixture has one offer for its binding");

    let result = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            turn,
            "turn-host-unavailable-after-admission-correlation",
        )
        .await;

    assert_eq!(result, Ok(()), "unavailable host is a settled outcome");
    assert_eq!(fixture.turn_row(&turn_id).0, "FAILED");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "SETTLED".to_owned())]);
    let (reason, user_count, answer_count, session_count): (String, i64, i64, i64) = fixture
        .connection()
        .query_row(
            "SELECT
               (SELECT json_extract(payload_json,'$.reason_code') FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1 AND type='conversation.turn.settled.v1'),
               (SELECT COUNT(*) FROM conversation_messages WHERE turn_id=?1 AND role='USER'),
               (SELECT COUNT(*) FROM conversation_messages WHERE turn_id=?1 AND role='AGENT'),
               (SELECT COUNT(*) FROM agent_sessions WHERE conversation_turn_id=?1)",
            [&turn_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read the post-admission unavailable outcome");
    assert_eq!(reason, "AGENT_UNAVAILABLE");
    assert_eq!(
        user_count, 1,
        "keep the admitted input for an explicit retry"
    );
    assert_eq!(answer_count, 0, "do not invent an agent answer");
    assert_eq!(session_count, 0, "do not reserve a session without a host");
}

#[tokio::test]
async fn conversation_dispatch_losing_no_host_preflight_preserves_the_claimed_attempt() {
    let fixture = turn_command_fixture("turn-no-host-losing-dispatch");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-no-host-losing-dispatch").await;
    let winner_incarnation = "turn-command-winner-incarnation";
    let claimed = SqliteConversationStore::new(fixture.store.clone())
        .claim_conversation_dispatch_intent(
            &turn_id,
            turn.retry_ordinal,
            winner_incarnation,
            "2026-10-15T12:00:02Z",
        )
        .expect("another dispatcher claims the durable intent first");
    assert!(claimed);

    let changed = fixture
        .connection()
        .execute(
            "UPDATE runtime_offers SET readiness='NEEDS_AUTH' WHERE runtime_id=?1 AND runtime_incarnation_id=?2 AND offer_kind='AGENT_ENDPOINT' AND offer_ref=?3",
            rusqlite::params![
                "turn-command-binding-runtime",
                "turn-command-binding-incarnation",
                "turn-command-binding-endpoint",
            ],
        )
        .expect("make a later dispatcher's host preflight unavailable");
    assert_eq!(changed, 1);

    let result = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            turn,
            "turn-no-host-losing-dispatch-correlation",
        )
        .await;

    assert_eq!(result, Ok(()), "a losing dispatcher performs no settlement");
    assert_eq!(
        fixture.turn_row(&turn_id).0,
        "OPEN",
        "a preflight failure cannot settle another dispatcher's turn"
    );
    let (intent_state, claimed_by): (String, Option<String>) = fixture
        .connection()
        .query_row(
            "SELECT state, claimed_by_runtime_incarnation FROM conversation_dispatch_intents WHERE turn_id=?1 AND retry_ordinal=0",
            [&turn_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read durable claimant after losing preflight");
    assert_eq!(intent_state, "CLAIMED");
    assert_eq!(claimed_by.as_deref(), Some(winner_incarnation));
}

#[tokio::test]
async fn conversation_dispatch_profile_lookup_error_settles_claimed_attempt() {
    let fixture = turn_command_fixture("turn-profile-lookup-error");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-profile-lookup-error").await;

    // Host resolution and binding lookup both succeed without consulting the
    // profile catalog. Remove that catalog to fault the later profile lookup.
    fixture
        .connection()
        .execute_batch("PRAGMA foreign_keys=OFF; DROP TABLE agent_profiles")
        .expect("remove the profile catalog to inject a profile lookup error");

    let failure = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            turn,
            "turn-profile-lookup-correlation",
        )
        .await;

    assert_eq!(
        failure,
        Err(crate::agents::supervisor::DispatchFailureClass::ProfileLookup)
    );
    assert_eq!(
        fixture.turn_row(&turn_id).0,
        "FAILED",
        "profile lookup failure after admission cannot strand the OPEN turn"
    );
    assert_eq!(
        fixture.intents(&turn_id),
        vec![(0, "SETTLED".to_owned())],
        "the claimed intent settles with the failed attempt"
    );
    let session_count: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE conversation_turn_id=?1",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("count sessions before reservation");
    assert_eq!(
        session_count, 0,
        "profile lookup fails before session reservation"
    );
}

#[tokio::test]
async fn conversation_dispatch_missing_profile_settles_claimed_attempt() {
    let fixture = turn_command_fixture("turn-profile-missing");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-profile-missing").await;

    // Host resolution reads the immutable pinned binding and endpoint/offer joins, not
    // the profile catalog. Remove the catalog row after admission so the later lookup
    // succeeds as a query but cannot find the pinned provider profile.
    let profile_id: String = fixture
        .connection()
        .query_row(
            "SELECT agent_profile_id FROM agent_bindings WHERE workspace_id=?1 AND agent_binding_id=?2",
            rusqlite::params![WORKSPACE_A, "turn-command-binding"],
            |row| row.get(0),
        )
        .expect("read the profile pinned by the binding");
    let catalog_connection = fixture.connection();
    catalog_connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .expect("disable fixture-only foreign key enforcement");
    let changed = catalog_connection
        .execute(
            "DELETE FROM agent_profiles WHERE agent_profile_id=?1",
            [&profile_id],
        )
        .expect("remove the profile row while preserving the admitted binding");
    assert_eq!(changed, 1, "the pinned profile row is removed");

    let failure = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            turn,
            "turn-profile-missing-correlation",
        )
        .await;

    assert_eq!(
        failure,
        Err(crate::agents::supervisor::DispatchFailureClass::ProfileLookup)
    );
    assert_eq!(fixture.turn_row(&turn_id).0, "FAILED");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "SETTLED".to_owned())]);
    let session_count: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE conversation_turn_id=?1",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("count sessions before reservation");
    assert_eq!(session_count, 0, "missing profile fails before reservation");
}

#[tokio::test]
async fn conversation_dispatch_binding_lookup_error_settles_admitted_attempt() {
    let fixture = turn_command_fixture("turn-binding-lookup-error");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-binding-lookup-error").await;

    // Host resolution only needs the binding's routing fields. Corrupt the unused
    // configuration payload so host resolution succeeds, but the subsequent full
    // AgentBinding decode fails at the supervisor's dedicated lookup boundary.
    fixture
        .connection()
        .execute(
            "UPDATE agent_bindings SET configuration_json='not-json' WHERE workspace_id=?1 AND agent_binding_id=?2",
            rusqlite::params![WORKSPACE_A, "turn-command-binding"],
        )
        .expect("inject a binding decode failure without breaking host resolution");

    let failure = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            turn,
            "turn-binding-lookup-correlation",
        )
        .await;

    assert_eq!(
        failure,
        Err(crate::agents::supervisor::DispatchFailureClass::BindingLookup)
    );
    assert_eq!(
        fixture.turn_row(&turn_id).0,
        "FAILED",
        "binding decode failure after admission cannot strand the OPEN turn"
    );
    assert_eq!(
        fixture.intents(&turn_id),
        vec![(0, "SETTLED".to_owned())],
        "the unclaimed intent settles with the failed attempt"
    );
    let session_count: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE conversation_turn_id=?1",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("count sessions before reservation");
    assert_eq!(
        session_count, 0,
        "binding lookup fails before session reservation"
    );
}

#[tokio::test]
async fn conversation_dispatch_session_reservation_error_settles_claimed_attempt() {
    let fixture = turn_command_fixture("turn-session-reservation-error");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-session-reservation-error").await;

    // Fail after the supervisor has claimed the intent but before a STARTING session
    // can be committed. This exercises settlement of the exact claimed attempt.
    fixture
        .connection()
        .execute_batch(
            "CREATE TRIGGER reject_starting_conversation_session BEFORE INSERT ON agent_sessions WHEN NEW.status='STARTING' BEGIN SELECT RAISE(ABORT, 'injected session reservation failure'); END;",
        )
        .expect("install session reservation fault");

    let failure = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            turn,
            "turn-session-reservation-correlation",
        )
        .await;

    assert_eq!(
        failure,
        Err(crate::agents::supervisor::DispatchFailureClass::SessionReservation)
    );
    assert_eq!(fixture.turn_row(&turn_id).0, "FAILED");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "SETTLED".to_owned())]);
    let session_count: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE conversation_turn_id=?1",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("count the failed reservation");
    assert_eq!(session_count, 0, "failed STARTING reservation is atomic");
}

#[tokio::test]
async fn conversation_dispatch_does_not_admit_a_fabricated_ready_host() {
    let fixture = turn_command_fixture("turn-host-instance-create-error");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-host-instance-create-error").await;

    // The fixture seeds a lifecycle-ready host for other dispatch-stage tests. Remove
    // it here to confirm dispatch no longer fabricates READY before any native process
    // exists. This fixture's Codex endpoint is intentionally not Send-capable, so the
    // turn must settle at the provider boundary without creating a false host record.
    fixture
        .connection()
        .execute(
            "DELETE FROM agent_host_instances WHERE host_instance_id=?1",
            ["host-turn-command-binding-endpoint"],
        )
        .expect("remove the fixture host to exercise lazy host creation");

    let failure = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            turn,
            "turn-host-instance-create-error-correlation",
        )
        .await;

    assert_eq!(
        failure,
        Err(crate::agents::supervisor::DispatchFailureClass::ProviderExecution)
    );
    assert_eq!(fixture.turn_row(&turn_id).0, "FAILED");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "SETTLED".to_owned())]);
    let session_count: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE conversation_turn_id=?1",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("count any session reservation");
    assert_eq!(
        session_count, 1,
        "the claimed attempt has one reserved session"
    );
    let host_count: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM agent_host_instances WHERE host_instance_id LIKE 'host-ses-%' AND state='READY'",
            [],
            |row| row.get(0),
        )
        .expect("count any falsely-ready hosts");
    assert_eq!(
        host_count, 0,
        "a host is not READY until native readiness is observed"
    );
}

#[tokio::test]
async fn conversation_dispatch_intent_claim_error_leaves_attempt_for_reconciliation() {
    let fixture = turn_command_fixture("turn-dispatch-intent-claim-error");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-dispatch-intent-claim-error").await;

    // Fail the durable compare-and-swap before any AgentSession reservation. A
    // failed/ambiguous claim does not prove this dispatcher owns the turn, so it
    // must leave the durable intent and OPEN turn for reconciliation.
    fixture
        .connection()
        .execute_batch(
            "CREATE TRIGGER reject_conversation_dispatch_claim BEFORE UPDATE OF state ON conversation_dispatch_intents WHEN NEW.state='CLAIMED' BEGIN SELECT RAISE(ABORT, 'injected dispatch claim failure'); END;",
        )
        .expect("install dispatch-intent claim fault");

    let failure = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            turn,
            "turn-dispatch-intent-claim-correlation",
        )
        .await;

    assert_eq!(
        failure,
        Err(crate::agents::supervisor::DispatchFailureClass::DispatchIntentClaim)
    );
    assert_eq!(fixture.turn_row(&turn_id).0, "OPEN");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "UNCLAIMED".to_owned())]);
    let claimed_by: Option<String> = fixture
        .connection()
        .query_row(
            "SELECT claimed_by_runtime_incarnation FROM conversation_dispatch_intents WHERE turn_id=?1 AND retry_ordinal=0",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("read unclaimed intent owner");
    assert_eq!(claimed_by, None, "failed claim has no proven owner");
    let session_count: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE conversation_turn_id=?1",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("count sessions for an unclaimed dispatch");
    assert_eq!(session_count, 0, "unclaimed intent owns no AgentSession");

    // The failed dispatcher has no settlement authority. Once the owning recovery
    // sweep runs, it resolves this durable UNCLAIMED intent to a typed terminal state.
    let recovered = crate::agents::conversation_recovery::recover_conversation_sessions(
        &fixture.store,
        RUNTIME,
        OWNER,
    );
    assert_eq!(recovered.intents_settled, 1);
    assert_eq!(recovered.failures, 0);
    assert_eq!(fixture.turn_row(&turn_id).0, "FAILED");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "SETTLED".to_owned())]);
}

#[tokio::test]
async fn conversation_dispatch_coordinator_retries_sqlite_claim_after_store_recovers() {
    let fixture = turn_command_fixture("turn-dispatch-claim-coordinator-retry");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-dispatch-claim-coordinator-retry").await;

    fixture
        .connection()
        .execute_batch(
            "CREATE TABLE dispatch_claim_fault (attempts INTEGER NOT NULL);
             INSERT INTO dispatch_claim_fault VALUES (0);
             CREATE TRIGGER defer_conversation_dispatch_claim BEFORE UPDATE OF state ON conversation_dispatch_intents
             WHEN NEW.state='CLAIMED'
             BEGIN
               UPDATE dispatch_claim_fault SET attempts=attempts+1;
               SELECT RAISE(IGNORE);
             END;
             CREATE TRIGGER reject_starting_conversation_session BEFORE INSERT ON agent_sessions
             WHEN NEW.status='STARTING'
             BEGIN SELECT RAISE(ABORT, 'injected session reservation refusal'); END;",
        )
        .expect("inject one retryable claim miss and a pre-provider session denial");

    let coordinator =
        crate::agents::turn_coordinator::AgentTurnCoordinator::new(Arc::clone(&fixture.supervisor));
    coordinator.dispatch_admitted_turn(
        DISPATCHER_INCARNATION,
        OWNER,
        WORKSPACE_A,
        fixture.conversation_id,
        turn,
        "turn-dispatch-claim-coordinator-retry-correlation",
    );

    let connection = fixture.connection();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let attempts: i64 = connection
                .query_row("SELECT attempts FROM dispatch_claim_fault", [], |row| {
                    row.get(0)
                })
                .expect("read the injected claim count");
            if attempts > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("coordinator reaches the transient SQLite claim miss");
    assert_eq!(fixture.turn_row(&turn_id).0, "OPEN");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "UNCLAIMED".to_owned())]);
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM agent_sessions WHERE conversation_turn_id=?1",
                [&turn_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("count sessions before claim recovers"),
        0,
        "no session/provider work starts while the intent remains unclaimed"
    );

    connection
        .execute_batch("DROP TRIGGER defer_conversation_dispatch_claim;")
        .expect("restore the dispatch claim store");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if fixture.turn_row(&turn_id).0 == "FAILED" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("coordinator retries the exact claim after storage recovers");

    assert_eq!(fixture.intents(&turn_id), vec![(0, "SETTLED".to_owned())]);
    assert_eq!(
        connection
            .query_row("SELECT attempts FROM dispatch_claim_fault", [], |row| row
                .get::<_, i64>(
                0
            ),)
            .expect("read final claim count"),
        1,
        "the successful retry proceeds once without replaying a provider prompt"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM agent_sessions WHERE conversation_turn_id=?1",
                [&turn_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("confirm the provider path was never opened"),
        0
    );
    connection
        .execute_batch("DROP TRIGGER reject_starting_conversation_session;")
        .expect("remove the test-only session denial");
}

#[tokio::test]
async fn conversation_dispatch_settlement_failure_stays_fenced_until_startup_recovery() {
    let fixture = turn_command_fixture("turn-dispatch-settlement-recovery");
    let (turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-dispatch-settlement-recovery").await;

    // Fail the STARTING-session reservation after the dispatch intent is claimed,
    // then prevent its settlement transaction from committing. The claimed intent
    // must fence dispatch until startup recovery.
    fixture
        .connection()
        .execute_batch(
            "CREATE TRIGGER reject_starting_conversation_session BEFORE INSERT ON agent_sessions
             WHEN NEW.status='STARTING'
             BEGIN SELECT RAISE(ABORT, 'injected session reservation failure'); END;
             CREATE TRIGGER reject_conversation_settlement BEFORE INSERT ON domain_events
             WHEN NEW.type='conversation.turn.settled.v1'
             BEGIN SELECT RAISE(ABORT, 'injected settlement outage'); END;",
        )
        .expect("inject a persistent settlement failure");

    let failure = fixture
        .supervisor
        .dispatch_turn(
            DISPATCHER_INCARNATION,
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            turn,
            "turn-dispatch-settlement-recovery-correlation",
        )
        .await;

    assert_eq!(
        failure,
        Err(crate::agents::supervisor::DispatchFailureClass::Settlement)
    );
    assert_eq!(fixture.turn_row(&turn_id).0, "OPEN");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "CLAIMED".to_owned())]);
    assert!(
        fixture
            .connection()
            .query_row(
                "SELECT NOT EXISTS(SELECT 1 FROM agent_sessions WHERE conversation_turn_id=?1)",
                [&turn_id],
                |row| row.get::<_, bool>(0),
            )
            .expect("check no native session was reserved")
    );

    fixture
        .connection()
        .execute_batch(
            "DROP TRIGGER reject_starting_conversation_session;
             DROP TRIGGER reject_conversation_settlement;",
        )
        .expect("restore settlement storage for restart recovery");
    let report = crate::agents::conversation_recovery::recover_conversation_sessions(
        &fixture.store,
        RUNTIME,
        OWNER,
    );

    assert_eq!((report.failures, report.intents_settled), (0, 1));
    assert_eq!(fixture.turn_row(&turn_id).0, "FAILED");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "SETTLED".to_owned())]);
    let reason: String = fixture
        .connection()
        .query_row(
            "SELECT json_extract(payload_json,'$.reason_code') FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1 AND type='conversation.turn.settled.v1'",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("read the recovery reason");
    assert_eq!(reason, "RUNTIME_RESTARTED");
}

#[tokio::test]
async fn conversation_blocking_dispatch_panic_settles_admitted_attempt() {
    let fixture = turn_command_fixture("turn-blocking-dispatch-panic");
    let (_turn, turn_id) =
        admitted_turn_for_dispatch_fixture(&fixture, "turn-blocking-dispatch-panic").await;
    let host = SqliteConversationStore::new(fixture.store.clone())
        .resolve_conversation_session_host_for_turn(
            OWNER,
            WORKSPACE_A,
            fixture.conversation_id,
            &turn_id,
        )
        .expect("resolve turn-pinned host")
        .expect("fixture host is eligible");
    let session_id = format!("{turn_id}-starting-session");
    let now = "2026-11-15T12:00:00Z";
    fixture
        .store
        .start_conversation_agent_session(storage_core::ConversationAgentSessionStart {
            request: WorkspaceCreateRequest {
                principal_id: OWNER.to_owned(),
                request_id: format!("req-{session_id}"),
                request_payload: serde_json::json!({
                    "operation":"agent-session.start-conversation.v1",
                    "workspace_id":WORKSPACE_A,
                    "conversation_id":fixture.conversation_id,
                    "conversation_turn_id":turn_id,
                    "agent_session_id":session_id,
                }),
            },
            session: storage_core::AgentSessionRecord {
                agent_session_id: session_id.clone(),
                workspace_id: WORKSPACE_A.to_owned(),
                scope_kind: "CONVERSATION".to_owned(),
                conversation_id: Some(fixture.conversation_id.to_owned()),
                conversation_turn_id: Some(turn_id.clone()),
                task_id: None,
                task_spec_revision: None,
                attempt_id: None,
                agent_binding_id: host.agent_binding_id.clone(),
                endpoint_id: host.endpoint_id.clone(),
                runtime_id: host.runtime_id.clone(),
                runtime_incarnation_id: host.runtime_incarnation_id.clone(),
                configuration_digest: None,
                harness_descriptor_digest: None,
                status: "STARTING".to_owned(),
                started_at: now.to_owned(),
                last_event_at: Some(now.to_owned()),
                closed_at: None,
                version: 1,
            },
            event: EventDraft {
                event_id: format!("event-{session_id}-starting"),
                workspace_id: WORKSPACE_A.to_owned(),
                entity_type: "AgentSession".to_owned(),
                entity_id: session_id.clone(),
                origin_runtime_id: RUNTIME.to_owned(),
                entity_revision: 1,
                hlc_timestamp: now.to_owned(),
                correlation_id: "turn-blocking-dispatch-panic-correlation".to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "agent.session.starting.v1".to_owned(),
                payload: serde_json::json!({
                    "agent_session_id":session_id,
                    "task_spec_revision":null,
                    "scope":{
                        "kind":"CONVERSATION",
                        "conversation_id":fixture.conversation_id,
                        "conversation_turn_id":turn_id,
                    },
                    "agent_binding_id":host.agent_binding_id,
                    "endpoint_id":host.endpoint_id,
                    "runtime_id":host.runtime_id,
                    "runtime_incarnation_id":host.runtime_incarnation_id,
                    "session_state":"STARTING",
                    "reported_at":now,
                }),
                recorded_at: now.to_owned(),
            },
        })
        .expect("commit the STARTING session before the worker failure");
    let join_result = tokio::task::spawn_blocking(
        || -> Result<(), crate::agents::supervisor::DispatchFailureClass> {
            panic!("injected worker panic with private detail");
        },
    )
    .await;
    let store = fixture.store.clone();
    let conversation_id = fixture.conversation_id.to_owned();
    let turn_id_for_recovery = turn_id.clone();
    let session_id_for_recovery = session_id.clone();
    let recovered = crate::agents::supervisor::join_dispatch_or_recover(join_result, move || {
        crate::agents::supervisor::recover_blocking_dispatch_failure(
            &store,
            RUNTIME,
            OWNER,
            WORKSPACE_A,
            &conversation_id,
            &turn_id_for_recovery,
            Some(&session_id_for_recovery),
            "turn-blocking-dispatch-panic-correlation",
        )
    })
    .await;

    assert_eq!(
        recovered,
        Err(crate::agents::supervisor::DispatchFailureClass::BlockingTask)
    );
    assert_eq!(fixture.turn_row(&turn_id).0, "FAILED");
    assert_eq!(fixture.intents(&turn_id), vec![(0, "SETTLED".to_owned())]);
    let session_status: String = fixture
        .connection()
        .query_row(
            "SELECT status FROM agent_sessions WHERE agent_session_id=?1",
            [&session_id],
            |row| row.get(0),
        )
        .expect("read the stranded reservation");
    assert_eq!(
        session_status, "LOST",
        "the exact STARTING session is released"
    );
    let message_counts: (i64, i64) = fixture
        .connection()
        .query_row(
            "SELECT SUM(role='USER'), SUM(role='AGENT') FROM conversation_messages WHERE turn_id=?1",
            [&turn_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("count messages for the failed turn");
    assert_eq!(
        message_counts,
        (1, 0),
        "the USER message remains; no answer is fabricated"
    );
    let reason: String = fixture
        .connection()
        .query_row(
            "SELECT json_extract(payload_json,'$.reason_code') FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1 AND type='conversation.turn.settled.v1'",
            [&turn_id],
            |row| row.get(0),
        )
        .expect("read the typed settlement reason");
    assert_eq!(reason, "CONVERSATION_SESSION_ADMISSION_UNAVAILABLE");
}

impl TurnCommandFixture {
    fn connection(&self) -> rusqlite::Connection {
        let connection = rusqlite::Connection::open(&self.database).expect("open fixture database");
        connection
            .execute_batch("PRAGMA foreign_keys=ON;")
            .expect("enable foreign keys");
        connection
    }

    /// Seeds a USER message and a turn directly in `status`; RUNNING and CANCEL_REQUESTED
    /// turns also get an ACTIVE Conversation AgentSession.
    fn seed_turn(&self, turn_id: &str, status: &str, version: i64, retry_ordinal: i64) {
        let connection = self.connection();
        let message_id = format!("{turn_id}-user-message");
        connection
            .execute(
                "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,agent_session_id,agent_binding_id,turn_id,content_json,resource_refs_json,source_channel_ref_json,created_at) VALUES(?1,?2,'{\"kind\":\"USER\"}','USER',NULL,NULL,NULL,'[]','[]',NULL,'2026-10-15T12:00:01Z')",
                rusqlite::params![message_id, self.conversation_id],
            )
            .expect("seed message");
        connection
            .execute(
                "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,agent_session_id,status,retry_ordinal,created_at,settled_at,version) VALUES(?1,?2,?3,NULL,'OPEN',?4,'2026-10-15T12:00:01Z',NULL,?5)",
                rusqlite::params![turn_id, self.conversation_id, message_id, retry_ordinal, version],
            )
            .expect("seed turn");
        let settled = matches!(status, "COMPLETED" | "FAILED" | "CANCELLED");
        let session_id = format!("{turn_id}-session");
        if matches!(status, "RUNNING" | "CANCEL_REQUESTED") {
            connection
                .execute(
                    "INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,conversation_id,conversation_turn_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at,last_event_at,version) VALUES(?1,?2,'CONVERSATION',?3,?4,'turn-command-binding','turn-command-binding-endpoint','turn-command-binding-runtime','turn-command-binding-incarnation','ACTIVE','2026-10-15T12:00:02Z','2026-10-15T12:00:02Z',2)",
                    rusqlite::params![session_id, WORKSPACE_A, self.conversation_id, turn_id],
                )
                .expect("seed active session");
            connection
                .execute(
                    "UPDATE conversation_turns SET agent_session_id=?1 WHERE turn_id=?2",
                    rusqlite::params![session_id, turn_id],
                )
                .expect("point the turn at its session");
        }
        connection
            .execute(
                "UPDATE conversation_turns SET status=?1, settled_at=?2 WHERE turn_id=?3",
                rusqlite::params![status, settled.then_some("2026-10-15T12:00:03Z"), turn_id],
            )
            .expect("move the turn to its status");
        connection
            .execute(
                "INSERT INTO conversation_dispatch_intents(turn_id,retry_ordinal,conversation_id,state,claimed_by_runtime_incarnation,claimed_at,settled_at,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'2026-10-15T12:00:01Z')",
                rusqlite::params![
                    turn_id,
                    retry_ordinal,
                    self.conversation_id,
                    if settled { "SETTLED" } else { "CLAIMED" },
                    if settled { None } else { Some("incarnation") },
                    if settled { None } else { Some("2026-10-15T12:00:02Z") },
                    settled.then_some("2026-10-15T12:00:03Z"),
                ],
            )
            .expect("seed dispatch intent");
    }

    fn seed_busy_host_binding(&self, turn_id: &str) -> (String, String) {
        let session_id = format!("{turn_id}-session");
        let host_id = format!("{turn_id}-host");
        let connection = self.connection();
        let transaction = connection
            .unchecked_transaction()
            .expect("begin host binding fixture transaction");
        let inserted = transaction
            .execute(
                "INSERT INTO agent_host_instances(host_instance_id,runtime_id,runtime_incarnation_id,agent_profile_id,endpoint_id,hosting_mode,state,ownership,started_at,last_used_at) SELECT ?1,s.runtime_id,s.runtime_incarnation_id,b.agent_profile_id,s.endpoint_id,'LOCAL_PER_SESSION','BUSY','LITECOWORK','2026-10-15T12:00:02Z','2026-10-15T12:00:02Z' FROM agent_sessions s JOIN agent_bindings b ON b.agent_binding_id=s.agent_binding_id WHERE s.agent_session_id=?2",
                rusqlite::params![host_id, session_id],
            )
            .expect("seed busy LiteCowork-owned host");
        assert_eq!(inserted, 1, "the active session has one pinned binding");
        transaction
            .execute(
                "INSERT INTO agent_session_host_bindings(agent_session_id,host_instance_id,native_session_ref,bound_at) VALUES(?1,?2,'native-session-private','2026-10-15T12:00:02Z')",
                rusqlite::params![session_id, host_id],
            )
            .expect("bind the active AgentSession to its host");
        transaction.commit().expect("commit host binding fixture");
        (session_id, host_id)
    }

    async fn command(&self, turn_id: &str, action: &str, key: &str, if_match: u64) -> Response {
        let mut request = Request::post(format!(
            "/v1/conversations/{}/turns/{turn_id}/{action}",
            self.conversation_id
        ))
        .header("x-workspace-id", WORKSPACE_A)
        .header("idempotency-key", key)
        .header("if-match", format!("\"{if_match}\""))
        .body(Body::empty())
        .expect("turn command request");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("turn command route responds")
    }

    fn turn_row(&self, turn_id: &str) -> (String, i64, i64, Option<String>) {
        self.connection()
            .query_row(
                "SELECT status,retry_ordinal,version,agent_session_id FROM conversation_turns WHERE turn_id=?1",
                [turn_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read turn row")
    }

    fn intents(&self, turn_id: &str) -> Vec<(i64, String)> {
        let connection = self.connection();
        let mut statement = connection
            .prepare("SELECT retry_ordinal,state FROM conversation_dispatch_intents WHERE turn_id=?1 ORDER BY retry_ordinal")
            .expect("prepare intents");
        statement
            .query_map([turn_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query intents")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect intents")
    }
}

#[tokio::test]
async fn conversation_stop_and_retry_failed_turn_lifecycle() {
    let fixture = turn_command_fixture("turn-retry-workspace");
    fixture.seed_turn("turn-failed", "FAILED", 3, 0);

    // Retry reopens the FAILED turn: OPEN, ordinal + 1, no session, one new UNCLAIMED
    // intent, and no second USER message.
    let response = fixture
        .command("turn-failed", "retry", "retry-key-1", 3)
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("receipt body");
    let receipt: ConversationTurnReceipt = serde_json::from_slice(&bytes).expect("receipt");
    assert_eq!(
        receipt.status,
        storage_core::conversation::ConversationTurnStatus::Open
    );
    let (status, ordinal, version, session) = fixture.turn_row("turn-failed");
    assert_eq!(
        (status.as_str(), ordinal, version, session),
        ("OPEN", 1, 4, None)
    );
    let intents = fixture.intents("turn-failed");
    assert_eq!(intents[0], (0, "SETTLED".to_owned()));
    assert_eq!(intents[1].0, 1);
    assert_eq!(intents.len(), 2);

    // A duplicate retry key replays the receipt and creates no second attempt.
    let replay = fixture
        .command("turn-failed", "retry", "retry-key-1", 3)
        .await;
    assert_eq!(replay.status(), StatusCode::ACCEPTED);
    assert_eq!(fixture.turn_row("turn-failed").1, 1);
    assert_eq!(fixture.intents("turn-failed").len(), 2);

    // A stale If-Match with a new key returns 412; a non-FAILED turn returns 409.
    let stale = fixture
        .command("turn-failed", "retry", "retry-key-2", 3)
        .await;
    assert_eq!(stale.status(), StatusCode::PRECONDITION_FAILED);
    // Keep the status-conflict assertion independent both from asynchronous dispatch
    // settling the retried attempt and from the one-active-turn Conversation guard.
    let running_fixture = turn_command_fixture("turn-retry-running-workspace");
    running_fixture.seed_turn("turn-running", "RUNNING", 4, 0);
    let not_failed = running_fixture
        .command("turn-running", "retry", "retry-key-3", 4)
        .await;
    assert_eq!(not_failed.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn turn_activity_records_route_returns_reported_rows_scoped_to_the_turn_attempt() {
    let fixture = turn_command_fixture("turn-activity-route-workspace");
    fixture.seed_turn("turn-activity", "RUNNING", 2, 0);
    for (sequence, kind, status, label) in [
        (2, "TOOL", "SUCCEEDED", None),
        (1, "TOOL", "STARTED", Some("bash")),
    ] {
        fixture
            .connection()
            .execute(
                "INSERT INTO conversation_turn_activity_records(activity_id,turn_id,retry_ordinal,conversation_id,sequence,kind,status,call_ref,label,created_at) VALUES(?1,'turn-activity',0,?2,?3,?4,?5,'call-1',?6,'2026-10-15T12:00:04.000000000Z')",
                rusqlite::params![format!("a-{sequence}"), fixture.conversation_id, sequence, kind, status, label],
            )
            .expect("seed activity");
    }
    let get = |workspace: &'static str, uri: String| {
        let mut request = Request::get(uri)
            .header("x-workspace-id", workspace)
            .body(Body::empty())
            .expect("activity request");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        let router = fixture.router.clone();
        async move {
            router
                .oneshot(request)
                .await
                .expect("activity route responds")
        }
    };
    let uri = format!(
        "/v1/conversations/{}/turns/turn-activity/activity",
        fixture.conversation_id
    );
    let response = get(WORKSPACE_A, uri.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let page: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(page["attribution"], "REPORTED");
    assert_eq!(page["items"].as_array().expect("items").len(), 2);
    assert_eq!(page["items"][0]["sequence"], 1);
    assert_eq!(page["items"][0]["label"], "bash");

    // Another retry ordinal has no records, and another Workspace cannot read this one.
    let other_attempt = get(WORKSPACE_A, format!("{uri}?retry_ordinal=1")).await;
    let bytes = to_bytes(other_attempt.into_body(), 64 * 1024)
        .await
        .expect("body");
    assert!(
        serde_json::from_slice::<serde_json::Value>(&bytes).expect("json")["items"]
            .as_array()
            .expect("items")
            .is_empty()
    );
    let foreign = get(WORKSPACE_B, uri).await;
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn live_permission_snapshot_route_is_owner_scoped_and_attempt_bound() {
    let fixture = turn_command_fixture("live-permission-snapshot-route-workspace");
    fixture.seed_turn("turn-live-permission", "RUNNING", 2, 1);
    let get = |workspace: &'static str, uri: String| {
        let mut request = Request::get(uri)
            .header("x-workspace-id", workspace)
            .body(Body::empty())
            .expect("permission snapshot request");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        let router = fixture.router.clone();
        async move {
            router
                .oneshot(request)
                .await
                .expect("permission snapshot route responds")
        }
    };

    let uri = format!(
        "/v1/conversations/{}/turns/turn-live-permission/permissions",
        fixture.conversation_id
    );
    let response = get(WORKSPACE_A, uri.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("snapshot body");
    let snapshot: serde_json::Value = serde_json::from_slice(&bytes).expect("snapshot JSON");
    assert_eq!(snapshot["workspace_id"], WORKSPACE_A);
    assert_eq!(snapshot["conversation_id"], fixture.conversation_id);
    assert_eq!(snapshot["turn_id"], "turn-live-permission");
    assert_eq!(snapshot["retry_ordinal"], 1);
    assert_eq!(snapshot["sequence"], 0);
    assert!(
        snapshot["items"]
            .as_array()
            .expect("snapshot items")
            .is_empty()
    );

    let foreign = get(WORKSPACE_B, uri.clone()).await;
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
    let missing = get(
        WORKSPACE_A,
        format!(
            "/v1/conversations/{}/turns/turn-missing/permissions",
            fixture.conversation_id
        ),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workspace_live_permission_snapshot_is_owner_scoped_bounded_and_ephemeral() {
    let fixture = turn_command_fixture("workspace-live-permission-snapshot");
    let get = |workspace: &'static str| {
        let mut request = Request::get("/v1/conversation-permissions")
            .header("x-workspace-id", workspace)
            .body(Body::empty())
            .expect("Workspace permission snapshot request");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        let router = fixture.router.clone();
        async move {
            router
                .oneshot(request)
                .await
                .expect("Workspace permission route responds")
        }
    };

    let response = get(WORKSPACE_A).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    let bytes = to_bytes(response.into_body(), 256 * 1024)
        .await
        .expect("Workspace snapshot body");
    let snapshot: serde_json::Value =
        serde_json::from_slice(&bytes).expect("Workspace snapshot JSON");
    assert_eq!(snapshot["workspace_id"], WORKSPACE_A);
    assert!(
        snapshot["items"]
            .as_array()
            .is_some_and(|items| items.len() <= 256 && items.is_empty())
    );
    assert_eq!(
        snapshot.as_object().expect("closed response shape").len(),
        2
    );

    let foreign = get(WORKSPACE_B).await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn native_permission_reply_route_is_mounted_and_rejects_settled_turns() {
    let fixture = turn_command_fixture("native-permission-reply-settled-workspace");
    fixture.seed_turn("turn-permission-settled", "COMPLETED", 3, 0);
    let mut request = Request::post(format!(
        "/v1/conversations/{}/turns/turn-permission-settled/permissions/{}/reply",
        fixture.conversation_id,
        "a".repeat(64)
    ))
    .header("x-workspace-id", WORKSPACE_A)
    .header("idempotency-key", "permission-reply-settled-1")
    .header("content-type", "application/json")
    .body(Body::from(r#"{"decision":"ALLOW_ONCE"}"#))
    .expect("permission reply request");
    request.extensions_mut().insert(AuthenticatedLocalPeer);

    let response = fixture
        .router
        .clone()
        .oneshot(request)
        .await
        .expect("permission reply route responds");
    assert_eq!(response.status(), StatusCode::GONE);
}

#[tokio::test]
async fn native_permission_reply_requires_a_live_registered_prompt() {
    let fixture = turn_command_fixture("native-permission-reply-no-prompt-workspace");
    fixture.seed_turn("turn-permission-running", "RUNNING", 2, 0);
    let mut request = Request::post(format!(
        "/v1/conversations/{}/turns/turn-permission-running/permissions/{}/reply",
        fixture.conversation_id,
        "b".repeat(64)
    ))
    .header("x-workspace-id", WORKSPACE_A)
    .header("idempotency-key", "permission-reply-no-prompt-1")
    .header("content-type", "application/json")
    .body(Body::from(r#"{"decision":"ALLOW_ONCE"}"#))
    .expect("permission reply request");
    request.extensions_mut().insert(AuthenticatedLocalPeer);

    let response = fixture
        .router
        .clone()
        .oneshot(request)
        .await
        .expect("permission reply route responds");
    assert_eq!(response.status(), StatusCode::GONE);
}

#[tokio::test]
async fn native_permission_reply_route_dispatches_once_to_active_worker_and_replays_receipt() {
    let fixture = turn_command_fixture("native-permission-reply-active-worker-workspace");
    let turn_id = "turn-permission-active-worker";
    fixture.seed_turn(turn_id, "RUNNING", 2, 0);
    let agent_session_id = format!("{turn_id}-session");
    let runtime_id = "turn-command-binding-runtime";
    let runtime_incarnation_id = "turn-command-binding-incarnation";
    let supervisor = Arc::new(crate::agents::AgentSessionSupervisor::new(
        fixture.store.clone(),
        runtime_id.to_owned(),
    ));
    let router = test_router_with_supervisor(
        fixture.store.clone(),
        OWNER.to_owned(),
        runtime_id.to_owned(),
        runtime_incarnation_id.to_owned(),
        Arc::clone(&supervisor),
    );

    let token = "d".repeat(64);
    let native_session_id = "native-session-route-private";
    let native_request_id = "native-request-route-private";
    let worker = crate::agents::supervisor::test_support::start_permission_worker(
        &supervisor,
        WORKSPACE_A,
        fixture.conversation_id,
        turn_id,
        0,
        &agent_session_id,
        runtime_incarnation_id,
        native_session_id,
        &token,
        native_request_id,
    );

    let mut request = Request::post(format!(
        "/v1/conversations/{}/turns/{turn_id}/permissions/{token}/reply",
        fixture.conversation_id,
    ))
    .header("x-workspace-id", WORKSPACE_A)
    .header("idempotency-key", "permission-reply-active-worker-1")
    .header("content-type", "application/json")
    .body(Body::from(r#"{"decision":"ALLOW_ONCE"}"#))
    .expect("permission reply request");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated permission reply route responds");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read permission receipt");
    let receipt: serde_json::Value =
        serde_json::from_slice(&bytes).expect("permission receipt is JSON");
    assert_eq!(receipt["prompt_token"], token);
    assert_eq!(receipt["state"], "ANSWERED");
    assert_eq!(receipt["decision"], "ALLOW_ONCE");
    assert_eq!(receipt["sequence"], 3);
    let receipt_text = receipt.to_string();
    assert!(!receipt_text.contains(native_session_id));
    assert!(!receipt_text.contains(native_request_id));

    let mut replay = Request::post(format!(
        "/v1/conversations/{}/turns/{turn_id}/permissions/{token}/reply",
        fixture.conversation_id,
    ))
    .header("x-workspace-id", WORKSPACE_A)
    .header("idempotency-key", "permission-reply-active-worker-1")
    .header("content-type", "application/json")
    .body(Body::from(r#"{"decision":"ALLOW_ONCE"}"#))
    .expect("permission reply replay request");
    replay.extensions_mut().insert(AuthenticatedLocalPeer);
    let replay_response = router
        .clone()
        .oneshot(replay)
        .await
        .expect("permission reply replay route responds");
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_bytes = to_bytes(replay_response.into_body(), 64 * 1024)
        .await
        .expect("read replay receipt");
    let replay_receipt: serde_json::Value =
        serde_json::from_slice(&replay_bytes).expect("replay receipt is JSON");
    assert_eq!(replay_receipt, receipt);

    let provider_replies = worker.finish();
    assert_eq!(
        provider_replies,
        [crate::agents::supervisor::test_support::ProviderReply {
            session_id: native_session_id.to_owned(),
            request_id: native_request_id.to_owned(),
            decision: "once",
        }],
        "the HTTP request and its idempotent replay dispatch one exact native reply"
    );
}

#[tokio::test]
async fn native_permission_rejection_route_reaches_agent_once_without_creating_capability_grant() {
    let fixture = turn_command_fixture("native-permission-reject-route-workspace");
    let turn_id = "turn-permission-reject-route";
    fixture.seed_turn(turn_id, "RUNNING", 2, 0);
    let agent_session_id = format!("{turn_id}-session");
    let runtime_id = "turn-command-binding-runtime";
    let runtime_incarnation_id = "turn-command-binding-incarnation";
    let supervisor = Arc::new(crate::agents::AgentSessionSupervisor::new(
        fixture.store.clone(),
        runtime_id.to_owned(),
    ));
    let router = test_router_with_supervisor(
        fixture.store.clone(),
        OWNER.to_owned(),
        runtime_id.to_owned(),
        runtime_incarnation_id.to_owned(),
        Arc::clone(&supervisor),
    );

    let token = "f".repeat(64);
    let native_session_id = "native-session-reject-route-private";
    let native_request_id = "native-request-reject-route-private";
    let worker = crate::agents::supervisor::test_support::start_permission_worker(
        &supervisor,
        WORKSPACE_A,
        fixture.conversation_id,
        turn_id,
        0,
        &agent_session_id,
        runtime_incarnation_id,
        native_session_id,
        &token,
        native_request_id,
    );
    let grants_before: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM capability_grants WHERE conversation_id=?1",
            [fixture.conversation_id],
            |row| row.get(0),
        )
        .expect("read pre-existing Conversation capability grants");

    let uri = format!(
        "/v1/conversations/{}/turns/{turn_id}/permissions/{token}/reply",
        fixture.conversation_id,
    );
    let mut request = Request::post(&uri)
        .header("x-workspace-id", WORKSPACE_A)
        .header("idempotency-key", "permission-reply-reject-route-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"decision":"REJECT_SESSION_PENDING"}"#))
        .expect("permission rejection request");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated permission rejection responds");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read rejection receipt");
    let receipt: serde_json::Value =
        serde_json::from_slice(&bytes).expect("rejection receipt is JSON");
    assert_eq!(receipt["prompt_token"], token);
    assert_eq!(receipt["state"], "ANSWERED");
    assert_eq!(receipt["decision"], "REJECT_SESSION_PENDING");
    let receipt_text = receipt.to_string();
    assert!(!receipt_text.contains(native_session_id));
    assert!(!receipt_text.contains(native_request_id));

    let mut replay = Request::post(&uri)
        .header("x-workspace-id", WORKSPACE_A)
        .header("idempotency-key", "permission-reply-reject-route-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"decision":"REJECT_SESSION_PENDING"}"#))
        .expect("permission rejection replay request");
    replay.extensions_mut().insert(AuthenticatedLocalPeer);
    let replay_response = router
        .clone()
        .oneshot(replay)
        .await
        .expect("permission rejection replay responds");
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_bytes = to_bytes(replay_response.into_body(), 64 * 1024)
        .await
        .expect("read rejection replay receipt");
    let replay_receipt: serde_json::Value =
        serde_json::from_slice(&replay_bytes).expect("replay receipt is JSON");
    assert_eq!(replay_receipt, receipt);

    assert_eq!(
        worker.finish(),
        [crate::agents::supervisor::test_support::ProviderReply {
            session_id: native_session_id.to_owned(),
            request_id: native_request_id.to_owned(),
            decision: "reject",
        }],
        "the authenticated rejection and replay dispatch one exact native reject"
    );
    let grants_after: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM capability_grants WHERE conversation_id=?1",
            [fixture.conversation_id],
            |row| row.get(0),
        )
        .expect("read Conversation capability grants after native rejection");
    assert_eq!(
        grants_after, grants_before,
        "native rejection is not a LiteCowork grant"
    );
}

#[tokio::test]
async fn native_permission_reply_route_reports_session_end_when_mailbox_closes_after_dispatch() {
    let fixture = turn_command_fixture("native-permission-reply-close-race-workspace");
    let turn_id = "turn-permission-close-race";
    fixture.seed_turn(turn_id, "RUNNING", 2, 0);
    let agent_session_id = format!("{turn_id}-session");
    let runtime_id = "turn-command-binding-runtime";
    let runtime_incarnation_id = "turn-command-binding-incarnation";
    let supervisor = Arc::new(crate::agents::AgentSessionSupervisor::new(
        fixture.store.clone(),
        runtime_id.to_owned(),
    ));
    let router = test_router_with_supervisor(
        fixture.store.clone(),
        OWNER.to_owned(),
        runtime_id.to_owned(),
        runtime_incarnation_id.to_owned(),
        Arc::clone(&supervisor),
    );

    let token = "e".repeat(64);
    let mut worker = crate::agents::supervisor::test_support::start_blocked_permission_worker(
        &supervisor,
        WORKSPACE_A,
        fixture.conversation_id,
        turn_id,
        0,
        &agent_session_id,
        runtime_incarnation_id,
        "native-session-close-race-private",
        &token,
        "native-request-close-race-private",
    );

    let mut request = Request::post(format!(
        "/v1/conversations/{}/turns/{turn_id}/permissions/{token}/reply",
        fixture.conversation_id,
    ))
    .header("x-workspace-id", WORKSPACE_A)
    .header("idempotency-key", "permission-reply-close-race-1")
    .header("content-type", "application/json")
    .body(Body::from(r#"{"decision":"ALLOW_ONCE"}"#))
    .expect("permission reply request");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let reply_router = router.clone();
    let response_task = tokio::spawn(async move {
        reply_router
            .oneshot(request)
            .await
            .expect("authenticated permission route responds")
    });

    worker.wait_for_provider_reply().await;
    worker.close_mailbox();
    worker.release_provider_reply();
    let response = tokio::time::timeout(Duration::from_secs(5), response_task)
        .await
        .expect("route settles after the attempt mailbox closes")
        .expect("permission route task does not panic");
    assert_eq!(response.status(), StatusCode::GONE);

    let provider_replies = worker.finish();
    assert_eq!(provider_replies.len(), 1);
    assert_eq!(provider_replies[0].decision, "once");
    assert_eq!(
        provider_replies[0].session_id,
        "native-session-close-race-private"
    );
    assert_eq!(
        provider_replies[0].request_id,
        "native-request-close-race-private"
    );
}

#[tokio::test]
async fn conversation_retry_is_refused_while_another_turn_is_in_flight() {
    let fixture = turn_command_fixture("turn-retry-in-flight-workspace");
    fixture.seed_turn("turn-old-failed", "FAILED", 3, 0);
    fixture.seed_turn("turn-waiting", "WAITING_USER", 2, 0);
    let response = fixture
        .command("turn-old-failed", "retry", "retry-blocked", 3)
        .await;
    let (status, error) = read_operator_error(response).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "CONVERSATION_TURN_IN_PROGRESS");
    assert_eq!(fixture.turn_row("turn-old-failed").0, "FAILED");
    assert_eq!(fixture.intents("turn-old-failed").len(), 1);
}

#[tokio::test]
async fn conversation_cancel_commits_only_the_request_and_never_fabricates_cancelled() {
    let fixture = turn_command_fixture("turn-cancel-workspace");
    fixture.seed_turn("turn-running", "RUNNING", 2, 0);

    // The route commits CANCEL_REQUESTED only; CANCELLED needs the supervisor's observed
    // quiescence, and no live dispatch owns this seeded attempt.
    assert!(!fixture.supervisor.request_cancel("turn-running", 0));
    let response = fixture
        .command("turn-running", "cancel", "cancel-key-1", 2)
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("receipt body");
    let receipt: ConversationTurnReceipt = serde_json::from_slice(&bytes).expect("receipt");
    assert_eq!(
        receipt.status,
        storage_core::conversation::ConversationTurnStatus::CancelRequested
    );
    let (status, _, version, _) = fixture.turn_row("turn-running");
    assert_eq!((status.as_str(), version), ("CANCEL_REQUESTED", 3));

    // An exact repeat replays; a repeat with a new key re-signals but never settles.
    let replay = fixture
        .command("turn-running", "cancel", "cancel-key-1", 2)
        .await;
    assert_eq!(replay.status(), StatusCode::ACCEPTED);
    let again = fixture
        .command("turn-running", "cancel", "cancel-key-2", 3)
        .await;
    assert_eq!(again.status(), StatusCode::ACCEPTED);
    let (status, _, version, _) = fixture.turn_row("turn-running");
    assert_eq!((status.as_str(), version), ("CANCEL_REQUESTED", 3));
    assert_eq!(
        fixture.intents("turn-running"),
        vec![(0, "CLAIMED".to_owned())]
    );
}

#[tokio::test]
async fn recovered_stop_settles_the_exact_turn_and_session_once() {
    let fixture = turn_command_fixture("turn-recovered-stop-settles-once");
    let turn_id = "turn-recovered-stop";
    fixture.seed_turn(turn_id, "RUNNING", 2, 0);
    let (session_id, host_id) = fixture.seed_busy_host_binding(turn_id);

    let request = fixture
        .command(turn_id, "cancel", "recovered-stop-cancel", 2)
        .await;
    assert_eq!(request.status(), StatusCode::ACCEPTED);
    assert_eq!(fixture.turn_row(turn_id).0, "CANCEL_REQUESTED");

    let scope = crate::agents::supervisor::TurnSettlementScope {
        store: &fixture.store,
        runtime_id: RUNTIME,
        principal_id: OWNER,
        workspace_id: WORKSPACE_A,
        conversation_id: fixture.conversation_id,
        turn_id,
        session_id: &session_id,
        correlation_id: "recovered-stop-correlation",
    };
    let disposition = crate::agents::supervisor::apply_observed_turn(
        scope,
        Ok(crate::agents::adapter::TurnObservation::Stopped),
        None,
    )
    .expect("a validated recovered stop settles through the supervisor path");
    assert!(matches!(
        disposition,
        crate::agents::supervisor::ObservedTurnDisposition::Cancelled
    ));

    let (status, _, version, active_session) = fixture.turn_row(turn_id);
    assert_eq!((status.as_str(), version), ("CANCELLED", 4));
    assert_eq!(active_session.as_deref(), Some(session_id.as_str()));
    let rows = fixture.connection();
    let session_status: String = rows
        .query_row(
            "SELECT status FROM agent_sessions WHERE agent_session_id=?1",
            [&session_id],
            |row| row.get(0),
        )
        .expect("read settled session");
    let (binding_count, host_status, settled_event_count): (i64, String, i64) = rows
        .query_row(
            "SELECT (SELECT COUNT(*) FROM agent_session_host_bindings WHERE agent_session_id=?1), (SELECT state FROM agent_host_instances WHERE host_instance_id=?2), (SELECT COUNT(*) FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?3 AND type='conversation.turn.settled.v1')",
            rusqlite::params![session_id, host_id, turn_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read atomic session binding and settlement");
    assert_eq!(session_status, "CLOSED");
    assert_eq!(binding_count, 0, "terminal settlement releases the binding");
    assert_eq!(
        host_status, "READY",
        "terminal settlement releases host use"
    );
    assert_eq!(settled_event_count, 1);

    let replay = crate::agents::supervisor::apply_observed_turn(
        scope,
        Ok(crate::agents::adapter::TurnObservation::Stopped),
        None,
    );
    assert!(
        replay.is_err(),
        "a duplicate observation cannot settle twice"
    );
    let repeated_events: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?1 AND type='conversation.turn.settled.v1'",
            [turn_id],
            |row| row.get(0),
        )
        .expect("count settlement events after duplicate observation");
    assert_eq!(repeated_events, 1);
}

#[tokio::test]
async fn unresolved_recovery_keeps_cancel_request_session_binding_and_busy_host() {
    let fixture = turn_command_fixture("turn-recovered-stop-unresolved");
    let turn_id = "turn-unresolved-stop";
    fixture.seed_turn(turn_id, "RUNNING", 2, 0);
    let (session_id, host_id) = fixture.seed_busy_host_binding(turn_id);

    let request = fixture
        .command(turn_id, "cancel", "unresolved-stop-cancel", 2)
        .await;
    assert_eq!(request.status(), StatusCode::ACCEPTED);

    let scope = crate::agents::supervisor::TurnSettlementScope {
        store: &fixture.store,
        runtime_id: RUNTIME,
        principal_id: OWNER,
        workspace_id: WORKSPACE_A,
        conversation_id: fixture.conversation_id,
        turn_id,
        session_id: &session_id,
        correlation_id: "unresolved-stop-correlation",
    };
    let disposition = crate::agents::supervisor::apply_observed_turn(
        scope,
        Ok(crate::agents::adapter::TurnObservation::StopUnconfirmed),
        None,
    )
    .expect("unresolved evidence remains a nonterminal disposition");
    assert!(matches!(
        disposition,
        crate::agents::supervisor::ObservedTurnDisposition::Unconfirmed
    ));

    assert_eq!(fixture.turn_row(turn_id).0, "CANCEL_REQUESTED");
    let rows = fixture.connection();
    let (session_status, binding_count, host_status, settled_event_count):
        (String, i64, String, i64) = rows
        .query_row(
            "SELECT (SELECT status FROM agent_sessions WHERE agent_session_id=?1), (SELECT COUNT(*) FROM agent_session_host_bindings WHERE agent_session_id=?1), (SELECT state FROM agent_host_instances WHERE host_instance_id=?2), (SELECT COUNT(*) FROM domain_events WHERE entity_type='ConversationTurn' AND entity_id=?3 AND type='conversation.turn.settled.v1')",
            rusqlite::params![session_id, host_id, turn_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read unresolved session and host ownership");
    assert_eq!(session_status, "ACTIVE");
    assert_eq!(binding_count, 1, "the AgentSession remains bound");
    assert_eq!(host_status, "BUSY", "the host remains reserved");
    assert_eq!(settled_event_count, 0, "unknown is not a terminal outcome");
}

#[test]
fn conversation_session_recovery_settles_prior_incarnation_attempts_idempotently() {
    let fixture = turn_command_fixture("turn-recovery-workspace");
    let store = SqliteWorkspaceStore::open(
        fixture.database.clone(),
        Arc::new(FileBlobStore::new(
            fixture._directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("reopen the store as a restarted daemon would");
    // Single-flight admission allows one nonterminal turn per Conversation, so each
    // crash window is seeded and swept in turn.
    fixture.seed_turn("turn-recover-completed", "COMPLETED", 4, 0);
    let recover = || {
        crate::agents::conversation_recovery::recover_conversation_sessions(&store, RUNTIME, OWNER)
    };
    for (turn, status, version) in [
        ("turn-recover-running", "RUNNING", 2),
        ("turn-recover-cancel", "CANCEL_REQUESTED", 3),
        ("turn-recover-open", "OPEN", 1),
    ] {
        fixture.seed_turn(turn, status, version, 0);
        if status == "OPEN" {
            // Admitted but never claimed: the intent is UNCLAIMED.
            fixture
                .connection()
                .execute(
                    "UPDATE conversation_dispatch_intents SET state='UNCLAIMED',claimed_by_runtime_incarnation=NULL,claimed_at=NULL WHERE turn_id=?1",
                    [turn],
                )
                .expect("mark intent unclaimed");
        }
        let report = recover();
        assert_eq!((report.failures, report.intents_settled), (0, 1), "{turn}");
        assert_eq!(fixture.turn_row(turn).0, "FAILED", "{turn} must be FAILED");
        assert_eq!(fixture.intents(turn), vec![(0, "SETTLED".to_owned())]);
    }
    // A completed answer is never rewritten.
    assert_eq!(fixture.turn_row("turn-recover-completed").0, "COMPLETED");
    let sessions: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE status NOT IN ('LOST','CLOSED')",
            [],
            |row| row.get(0),
        )
        .expect("count live sessions");
    assert_eq!(sessions, 0);
    let messages: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM conversation_messages WHERE role='AGENT'",
            [],
            |row| row.get(0),
        )
        .expect("count agent messages");
    assert_eq!(messages, 0, "recovery never invents an answer");

    // Running the sweep again changes nothing, and Retry is then offered.
    let again = recover();
    assert_eq!(again.intents_settled, 0);
    assert_eq!(again.failures, 0);
}

#[test]
fn conversation_routes_project_the_operators_single_native_session_supervisor() {
    // A Stop can only interrupt the dispatch that registered the attempt, so every
    // Conversation route projection must carry the Operator's one process-lifetime
    // supervisor and its live Stop registry. A supervisor rebuilt per request holds an
    // empty registry: Stop would report success while the turn kept running, and the
    // registered signal would never be requested.
    let directory = tempfile::tempdir().expect("temporary supervisor projection directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private test directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    let (_shutdown_sender, stream_shutdown) = tokio::sync::watch::channel(false);
    // Built before the literal, exactly as the serving Operator builds it: the store
    // moves into the state, so the supervisor must be constructed from its clone first.
    let supervisor = Arc::new(crate::agents::AgentSessionSupervisor::new(
        store.clone(),
        RUNTIME.to_owned(),
    ));
    let state = super::super::ApiState {
        store,
        principal_id: OWNER.to_owned(),
        runtime_id: RUNTIME.to_owned(),
        local_incarnation_id: "supervisor-projection-incarnation".to_owned(),
        lifecycle_blockers: Vec::new(),
        runtime_identity: Arc::new(
            storage_sqlite::RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                .expect("test-only OS identity is available"),
        ),
        stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
        runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        stream_shutdown,
        supervisor,
    };

    let first = ConversationApiState::from_ref(&state)
        .supervisor
        .expect("the Operator state serves native dispatch");
    let second = ConversationApiState::from_ref(&state)
        .supervisor
        .expect("the Operator state serves native dispatch");
    assert!(
        Arc::ptr_eq(&first, &second),
        "every Conversation request must observe the same live Stop registry"
    );
    assert!(
        Arc::ptr_eq(&first, &state.supervisor),
        "the projection must carry the Operator's supervisor, not a freshly built one"
    );
    // Nothing is registered for this attempt, so no live dispatch owns it and there is
    // nothing to signal: the route must report that honestly rather than claim a Stop.
    assert!(!first.request_cancel("turn-not-dispatched", 0));
}

async fn rename_request(
    router: &Router,
    workspace_id: &str,
    conversation_id: &str,
    expected_version: u64,
    request_id: &str,
    title: serde_json::Value,
) -> Response {
    let mut request = Request::patch(format!("/v1/conversations/{conversation_id}"))
        .header("x-workspace-id", workspace_id)
        .header("if-match", format!("\"{expected_version}\""))
        .header("idempotency-key", request_id)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "title": title }).to_string(),
        ))
        .expect("Conversation rename request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated Conversation rename route responds")
}

/// POSTs one of the bodyless Conversation history commands (`archive`/`unarchive`).
async fn history_command_request(
    router: &Router,
    workspace_id: &str,
    conversation_id: &str,
    action: &str,
    expected_version: u64,
    request_id: &str,
) -> Response {
    let mut request = Request::post(format!("/v1/conversations/{conversation_id}/{action}"))
        .header("x-workspace-id", workspace_id)
        .header("if-match", format!("\"{expected_version}\""))
        .header("idempotency-key", request_id)
        .body(Body::empty())
        .expect("Conversation history command request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated Conversation history route responds")
}

async fn history_list_page(router: &Router, workspace_id: &str, status: &str) -> Response {
    let mut request = Request::get(format!("/v1/conversations?status={status}"))
        .header("x-workspace-id", workspace_id)
        .body(Body::empty())
        .expect("Conversation history list request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated Conversation list responds")
}

/// Reads the durable history state row the archive routes own.
fn history_state(database: &std::path::Path, conversation_id: &str) -> Option<String> {
    rusqlite::Connection::open(database)
        .expect("inspect Conversation history state")
        .query_row(
            "SELECT status FROM conversation_history_state WHERE conversation_id=?1",
            [conversation_id],
            |row| row.get(0),
        )
        .ok()
}

fn history_events(database: &std::path::Path, conversation_id: &str) -> Vec<(String, String)> {
    let connection =
        rusqlite::Connection::open(database).expect("inspect Conversation history events");
    connection
        .prepare(
            "SELECT type,payload_json FROM domain_events WHERE entity_type='Conversation' AND entity_id=?1 ORDER BY origin_sequence",
        )
        .expect("prepare history event read")
        .query_map([conversation_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("read history events")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect history events")
}

/// Seeds one nonterminal turn directly so the archive gate can be exercised without a
/// live dispatch: only a turn row makes the Conversation non-quiescent.
fn seed_nonterminal_turn(database: &std::path::Path, conversation_id: &str) {
    let connection = rusqlite::Connection::open(database).expect("seed nonterminal turn");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable seed foreign keys");
    connection
        .execute(
            "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,agent_session_id,agent_binding_id,turn_id,content_json,resource_refs_json,source_channel_ref_json,created_at) VALUES('history-seeded-message',?1,'{\"kind\":\"USER\"}','USER',NULL,NULL,NULL,'[]','[]',NULL,'2026-10-15T12:00:01Z')",
            [conversation_id],
        )
        .expect("seed the in-flight turn message");
    connection
        .execute(
            "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,agent_session_id,status,retry_ordinal,created_at,settled_at,version) VALUES('history-seeded-turn',?1,'history-seeded-message',NULL,'WAITING_USER',0,'2026-10-15T12:00:01Z',NULL,1)",
            [conversation_id],
        )
        .expect("seed the in-flight turn");
}

#[tokio::test]
async fn conversation_rename_route_commits_a_digest_only_event_and_fences_version_and_key() {
    let directory = tempfile::tempdir().expect("temporary Conversation rename directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation rename directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "rename-a");
    seed_conversation(
        &store,
        WORKSPACE_A,
        "conversation-rename-route",
        "Original title",
        "2026-10-15T12:00:00.000000000Z",
    );
    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());
    let request_id = "conversation-rename-request-1";

    let renamed = read_updated_conversation(
        rename_request(
            &router,
            WORKSPACE_A,
            "conversation-rename-route",
            1,
            request_id,
            serde_json::json!("Renamed title"),
        )
        .await,
    )
    .await;
    assert_eq!(renamed.title.as_deref(), Some("Renamed title"));
    assert_eq!(renamed.version, 2);

    // An exact retry replays the original commit and writes no second event.
    let replay = read_updated_conversation(
        rename_request(
            &router,
            WORKSPACE_A,
            "conversation-rename-route",
            1,
            request_id,
            serde_json::json!("Renamed title"),
        )
        .await,
    )
    .await;
    assert_eq!(
        replay, renamed,
        "an exact retry returns the original commit"
    );

    // One key cannot describe two titles.
    let reused = rename_request(
        &router,
        WORKSPACE_A,
        "conversation-rename-route",
        1,
        request_id,
        serde_json::json!("A different title"),
    )
    .await;
    let (status, error) = read_operator_error(reused).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "IDEMPOTENCY_CONFLICT");

    // A stale If-Match with a new key is refused with the 014 code.
    let stale = rename_request(
        &router,
        WORKSPACE_A,
        "conversation-rename-route",
        1,
        "conversation-rename-request-2",
        serde_json::json!("Stale title"),
    )
    .await;
    let (status, error) = read_operator_error(stale).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(error["code"], "STALE_VERSION");

    // An empty, over-long or control-character title is refused before any write.
    for (value, label) in [
        (serde_json::json!("   "), "blank"),
        (serde_json::json!("x".repeat(161)), "over-long"),
        (serde_json::json!("bad\u{7}title"), "control"),
        (serde_json::Value::Null, "null"),
        (serde_json::json!(7), "non-string"),
    ] {
        let refused = rename_request(
            &router,
            WORKSPACE_A,
            "conversation-rename-route",
            2,
            &format!("conversation-rename-invalid-{label}"),
            value,
        )
        .await;
        let (status, error) = read_operator_error(refused).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{label}");
        assert_eq!(error["code"], "INVALID_ARGUMENT", "{label}");
    }

    // Exactly one rename event is durable, and it carries the digest, never the title.
    let events = history_events(&database, "conversation-rename-route");
    let renamed_events: Vec<_> = events
        .iter()
        .filter(|(kind, _)| kind == "conversation.renamed.v1")
        .collect();
    assert_eq!(
        renamed_events.len(),
        1,
        "retries and refusals emit no event"
    );
    assert!(renamed_events[0].1.contains("title_digest"));
    assert!(!renamed_events[0].1.contains("Renamed title"));

    let final_record = SqliteConversationStore::new(store)
        .get_conversation(OWNER, WORKSPACE_A, "conversation-rename-route")
        .expect("read Conversation")
        .expect("Conversation remains present");
    assert_eq!(final_record, renamed);
}

#[tokio::test]
async fn conversation_archive_routes_gate_on_turn_quiescence_and_filter_the_history_list() {
    let directory = tempfile::tempdir().expect("temporary Conversation archive directory");
    #[cfg(unix)]
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Conversation archive directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            PaginationTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real temporary SQLite store opens");
    create_workspace(&store, WORKSPACE_A, "archive-a");
    seed_conversation(
        &store,
        WORKSPACE_A,
        "conversation-archive-route",
        "Archive me",
        "2026-10-15T12:00:00.000000000Z",
    );
    seed_conversation(
        &store,
        WORKSPACE_A,
        "conversation-archive-blocked",
        "Still working",
        "2026-10-15T12:00:01.000000000Z",
    );
    seed_nonterminal_turn(&database, "conversation-archive-blocked");
    let router = test_router(store.clone(), OWNER.to_owned(), RUNTIME.to_owned());

    // A Conversation with a nonterminal turn cannot be archived. The keys below avoid
    // the fixture's own `{conversation_id}-request` creation key: reusing it would be
    // answered from the creation receipt instead of reaching the archive check.
    let blocked = history_command_request(
        &router,
        WORKSPACE_A,
        "conversation-archive-blocked",
        "archive",
        1,
        "a06-history-archive-blocked-key",
    )
    .await;
    let (status, error) = read_operator_error(blocked).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "CONVERSATION_TURN_IN_PROGRESS");
    assert_eq!(
        history_state(&database, "conversation-archive-blocked"),
        None
    );

    let archived = read_updated_conversation(
        history_command_request(
            &router,
            WORKSPACE_A,
            "conversation-archive-route",
            "archive",
            1,
            "a06-history-archive-key-1",
        )
        .await,
    )
    .await;
    assert_eq!(archived.version, 2);
    assert_eq!(
        history_state(&database, "conversation-archive-route").as_deref(),
        Some("ARCHIVED")
    );

    // Archiving again is a stale version: the row already moved past If-Match 1.
    let archived_again = history_command_request(
        &router,
        WORKSPACE_A,
        "conversation-archive-route",
        "archive",
        1,
        "a06-history-archive-key-2",
    )
    .await;
    let (status, error) = read_operator_error(archived_again).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(error["code"], "STALE_VERSION");

    // An archived Conversation refuses a rename with the 014 code.
    let rename_on_archived = rename_request(
        &router,
        WORKSPACE_A,
        "conversation-archive-route",
        2,
        "a06-history-rename-on-archived-key",
        serde_json::json!("Cannot rename while archived"),
    )
    .await;
    let (status, error) = read_operator_error(rename_on_archived).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "CONVERSATION_ARCHIVED");

    // The pre-existing AgentBinding route reports the same history code, not
    // INVALID_ARGUMENT: an archived Conversation refuses every domain mutation.
    let binding_on_archived = set_conversation_agent_binding_request(
        &router,
        WORKSPACE_A,
        "conversation-archive-route",
        2,
        "a06-history-binding-on-archived-key",
        serde_json::Value::Null,
    )
    .await;
    let (status, error) = read_operator_error(binding_on_archived).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "CONVERSATION_ARCHIVED");

    // The history list is filtered: the archived row leaves the default page.
    let active = read_page(history_list_page(&router, WORKSPACE_A, "ACTIVE").await).await;
    let active_ids: Vec<_> = active
        .items
        .iter()
        .map(|item| item.conversation_id.as_str())
        .collect();
    assert!(
        !active_ids.contains(&"conversation-archive-route"),
        "an archived Conversation is not on the ACTIVE page"
    );
    assert!(active_ids.contains(&"conversation-archive-blocked"));
    let archived_page = read_page(history_list_page(&router, WORKSPACE_A, "ARCHIVED").await).await;
    assert_eq!(
        archived_page
            .items
            .iter()
            .map(|item| item.conversation_id.as_str())
            .collect::<Vec<_>>(),
        ["conversation-archive-route"]
    );
    let unsupported = history_list_page(&router, WORKSPACE_A, "DELETED").await;
    assert_eq!(unsupported.status(), StatusCode::BAD_REQUEST);

    // Unarchive returns the Conversation to ACTIVE on the next version.
    let unarchived = read_updated_conversation(
        history_command_request(
            &router,
            WORKSPACE_A,
            "conversation-archive-route",
            "unarchive",
            2,
            "a06-history-unarchive-key-1",
        )
        .await,
    )
    .await;
    assert_eq!(unarchived.version, 3);
    assert_eq!(
        history_state(&database, "conversation-archive-route").as_deref(),
        Some("ACTIVE")
    );

    // Unarchiving an ACTIVE Conversation is refused: the storage answers a version
    // conflict rather than a silent no-op, and a foreign owner never reaches storage.
    let active_unarchive = history_command_request(
        &router,
        WORKSPACE_A,
        "conversation-archive-blocked",
        "unarchive",
        1,
        "a06-history-unarchive-active-key",
    )
    .await;
    let (status, error) = read_operator_error(active_unarchive).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(error["code"], "STALE_VERSION");
    let foreign = test_router(
        store,
        "a06-pagination-foreign-owner".to_owned(),
        RUNTIME.to_owned(),
    );
    let foreign_unarchive = history_command_request(
        &foreign,
        WORKSPACE_A,
        "conversation-archive-route",
        "unarchive",
        3,
        "a06-history-unarchive-foreign-key",
    )
    .await;
    assert_eq!(foreign_unarchive.status(), StatusCode::NOT_FOUND);

    let events = history_events(&database, "conversation-archive-route");
    let kinds: Vec<&str> = events
        .iter()
        .map(|(kind, _)| kind.as_str())
        // `conversation.unarchived.v1` does not share the `archiv` prefix of
        // `conversation.archived.v1`, so match on the stem instead.
        .filter(|kind| kind.contains("archiv"))
        .collect();
    assert_eq!(
        kinds,
        ["conversation.archived.v1", "conversation.unarchived.v1"],
        "the refused and replayed commands emit no history event"
    );
}
