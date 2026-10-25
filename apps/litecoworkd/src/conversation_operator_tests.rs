use super::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::Response,
    Router,
};
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use serde::Deserialize;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{collections::HashSet, sync::Arc, time::Duration};
use storage_core::{
    conversation::request_payload_value, BlobPurpose, ConversationRecord, ConversationStore,
    ConversationTurnStore, CreateConversationCommit, EventDraft, StoreError,
    WorkspaceCreateRequest,
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
    tx.commit().expect("commit ready Runtime fixture");
}

/// Configures the Workspace default AgentBinding through the same guarded column
/// the `PATCH /v1/workspaces/{id}/default-agent-binding` route writes.
fn set_workspace_default_agent(
    database: &std::path::Path,
    workspace_id: &str,
    binding_id: &str,
) {
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
    let mut request = Request::patch(format!(
        "/v1/conversations/{conversation_id}/agent-binding"
    ))
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
    let router = test_router(store, OWNER.to_owned(), RUNTIME.to_owned());
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
    let router = test_router(store, OWNER.to_owned(), RUNTIME.to_owned());
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
async fn conversation_turn_submit_stays_fail_closed_when_runtime_is_ready_without_session_supervisor(
) {
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

    let resolved_host = SqliteConversationStore::new(store.clone())
        .resolve_conversation_session_host(WORKSPACE_A, conversation_id)
        .expect("Runtime inventory resolution succeeds");
    assert!(
        resolved_host.is_some(),
        "the fixture must satisfy the database inventory checks"
    );

    let router = test_router(store, OWNER.to_owned(), RUNTIME.to_owned());
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
    assert!(error["message"]
        .as_str()
        .unwrap_or_default()
        .contains("not sent"));

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

    // Exact replay resolves the committed receipt, writes nothing, and reports the same
    // unavailable outcome.
    let replay =
        submit_turn_body_with_key(&router, WORKSPACE_A, conversation_id, request_id, BODY).await;
    assert_eq!(replay.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(replay.headers()["cache-control"], "no-store");
    let (_, replay_error) = read_operator_error(replay).await;
    assert_eq!(replay_error["code"], "DEPENDENCY_UNAVAILABLE");
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
async fn conversation_turn_route_refuses_an_ineligible_conversation_override_without_falling_back() {
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
    seed_agent_binding(
        &database,
        WORKSPACE_A,
        "turn-override-binding",
        false,
        true,
    );
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
async fn conversation_agent_binding_route_commits_replays_clears_and_rejects_stale_or_ineligible_choices() {
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
    assert_eq!(first.active_agent_binding_id.as_deref(), Some("eligible-binding"));
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
        ("disabled-binding", "conversation-agent-binding-disabled-request"),
        ("non-lead-binding", "conversation-agent-binding-non-lead-request"),
        ("foreign-binding", "conversation-agent-binding-foreign-request"),
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
    let payload: serde_json::Value =
        serde_json::from_str(&payload).expect("event payload is JSON");
    assert_eq!(payload["from_agent_binding_id"], "eligible-binding");
    assert_eq!(payload["to_agent_binding_id"], serde_json::Value::Null);
    assert_eq!(payload["aggregate_version"], 3);
}
