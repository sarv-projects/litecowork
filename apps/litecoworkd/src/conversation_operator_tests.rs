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
    BlobPurpose, ConversationRecord, ConversationStore, CreateConversationCommit, EventDraft,
    StoreError, WorkspaceCreateRequest, conversation::request_payload_value,
};
use storage_sqlite::{
    FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider,
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
    let mut request = Request::post(format!("/v1/conversations/{conversation_id}/turns"))
        .header("x-workspace-id", workspace_id)
        .header("idempotency-key", "conversation-turn-submit-1")
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .expect("Conversation turn request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    router
        .clone()
        .oneshot(request)
        .await
        .expect("authenticated Conversation turn route responds")
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
async fn conversation_turn_route_fails_closed_without_creating_an_unsettleable_turn() {
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

    let response = submit_turn(&router, WORKSPACE_A, "a06-conversation-turn").await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = to_bytes(response.into_body(), 8192)
        .await
        .expect("bounded unavailable response");
    let error: serde_json::Value = serde_json::from_slice(&body).expect("Operator error JSON");
    assert_eq!(error["code"], "DEPENDENCY_UNAVAILABLE");
    assert!(
        error["message"]
            .as_str()
            .unwrap_or_default()
            .contains("not sent"),
        "the unavailable response must say the message was not sent"
    );
    let conversation = SqliteConversationStore::new(store.clone())
        .get_conversation(OWNER, WORKSPACE_A, "a06-conversation-turn")
        .expect("conversation read succeeds")
        .expect("conversation remains available");
    assert_eq!(
        conversation.version, 1,
        "no User message or OPEN turn was committed"
    );
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
