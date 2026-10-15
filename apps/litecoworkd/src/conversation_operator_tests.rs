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
