use super::*;
use rusqlite::Connection;
use storage_core::{
    NeedsYouFilter, NeedsYouItemKind, NeedsYouPageRequest, NeedsYouReadStore, NeedsYouStatus,
};

const AT: &str = "2026-10-14T00:00:00Z";
const SECRET_MARKER: &str = "PRIVATE_SOURCE_TEXT_MUST_NOT_ESCAPE";

fn request(workspace_id: &str, filter: NeedsYouFilter, limit: usize) -> NeedsYouPageRequest {
    NeedsYouPageRequest {
        workspace_id: workspace_id.to_owned(),
        filter,
        limit,
        cursor: None,
    }
}

fn insert_task(
    connection: &Connection,
    workspace_id: &str,
    task_id: &str,
    status: &str,
    blockers: &str,
) {
    connection
        .execute(
            "INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status,
                lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                created_at, updated_at, version)
             VALUES (?1, ?2, 1, ?3, 'binding-missing-in-test-fixture', ?4, 'NORMAL', '{}', ?5, ?5, 1)",
            rusqlite::params![task_id, workspace_id, status, blockers, AT],
        )
        .expect("test Task row inserts");
}

fn seed_sources(store: &SqliteWorkspaceStore, directory: &tempfile::TempDir) {
    create_workspace(store, "workspace-needs-you");
    service(store.clone())
        .create(CreateWorkspace {
            workspace_id: "workspace-other".to_owned(),
            name: "Other workspace".to_owned(),
            owner_principal_id: "owner-local".to_owned(),
            event: context("event-create-other", 1),
        })
        .expect("foreign fixture Workspace commits");
    let connection = Connection::open(state_database(directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable fixture foreign keys on this connection");
    connection
        .execute_batch("DROP TRIGGER user_request_admission_matches_scope;")
        .expect("disable admission-only test fixture trigger");

    insert_task(
        &connection,
        "workspace-needs-you",
        "task-main",
        "BLOCKED",
        "[]",
    );
    insert_task(
        &connection,
        "workspace-needs-you",
        "task-not-blocked",
        "READY",
        &format!("[{SECRET_MARKER}]"),
    );
    insert_task(
        &connection,
        "workspace-other",
        "task-foreign",
        "BLOCKED",
        "[]",
    );

    let digest = format!("sha256:{}", "a".repeat(64));
    for (id, status, requested_at, resolved_at) in [
        ("approval-pending", "PENDING", "2026-10-14T00:00:01Z", None),
        (
            "approval-approved",
            "APPROVED",
            "2026-10-14T00:00:02Z",
            Some(AT),
        ),
        (
            "approval-denied",
            "DENIED",
            "2026-10-14T00:00:03Z",
            Some(AT),
        ),
        (
            "approval-expired",
            "EXPIRED",
            "2026-10-14T00:00:04Z",
            Some(AT),
        ),
        (
            "approval-cancelled",
            "CANCELLED",
            "2026-10-14T00:00:05Z",
            Some(AT),
        ),
    ] {
        connection
            .execute(
                "INSERT INTO approvals(approval_id, task_id, kind, action_summary, target_ref_json,
                    scope_digest, action_digest, risk, required_assurance, status, requested_at, resolved_at)
                 VALUES (?1, 'task-main', 'CAPABILITY', ?2, ?3, ?4, ?4, 'SENSITIVE', 'LOCAL_OWNER', ?5, ?6, ?7)",
                rusqlite::params![id, SECRET_MARKER, format!(r#"{{"locator":"{SECRET_MARKER}"}}"#), digest, status, requested_at, resolved_at],
            )
            .expect("test Approval row inserts");
    }
    for (id, status, created_at, resolved_at, response_digest) in [
        (
            "request-pending",
            "PENDING",
            "2026-10-14T00:01:01Z",
            None,
            None,
        ),
        (
            "request-answered",
            "ANSWERED",
            "2026-10-14T00:01:02Z",
            Some(AT),
            Some(digest.as_str()),
        ),
        (
            "request-dismissed",
            "DISMISSED",
            "2026-10-14T00:01:03Z",
            Some(AT),
            None,
        ),
        (
            "request-expired",
            "EXPIRED",
            "2026-10-14T00:01:04Z",
            Some(AT),
            None,
        ),
        (
            "request-cancelled",
            "CANCELLED",
            "2026-10-14T00:01:05Z",
            Some(AT),
            None,
        ),
    ] {
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id, kind,
                    prompt, status, created_at, resolved_at, resolved_by_json, response_digest)
                 VALUES (?1, 'workspace-needs-you', 'task-main', 'session-missing-in-test-fixture',
                    'QUESTION', ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![id, SECRET_MARKER, status, created_at, resolved_at,
                    resolved_at.map(|_| "{}"), response_digest],
            )
            .expect("test UserRequest row inserts");
    }
}

#[test]
fn projection_maps_source_statuses_and_never_returns_private_source_text() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_sources(&store, &directory);

    let page = store
        .list_needs_you_page(request(
            "workspace-needs-you",
            NeedsYouFilter::default(),
            200,
        ))
        .expect("Needs You projection reads committed sources");
    assert_eq!(page.items.len(), 10);
    assert!(page.next_cursor.is_none());
    assert!(
        page.items
            .iter()
            .all(|item| !format!("{item:?}").contains(SECRET_MARKER))
    );

    let open_filter = NeedsYouFilter {
        status: Some(NeedsYouStatus::Open),
        ..NeedsYouFilter::default()
    };
    let open = store
        .list_needs_you_page(request("workspace-needs-you", open_filter, 50))
        .expect("OPEN filter succeeds");
    assert_eq!(open.items.len(), 2);
    assert!(
        open.items
            .iter()
            .all(|item| item.status == NeedsYouStatus::Open)
    );

    let resolved_filter = NeedsYouFilter {
        status: Some(NeedsYouStatus::Resolved),
        ..NeedsYouFilter::default()
    };
    let resolved = store
        .list_needs_you_page(request("workspace-needs-you", resolved_filter, 50))
        .expect("RESOLVED filter succeeds");
    assert_eq!(resolved.items.len(), 8);
    assert!(
        resolved
            .items
            .iter()
            .all(|item| item.status == NeedsYouStatus::Resolved)
    );

    for (kind, id, expected_status) in [
        (
            NeedsYouItemKind::Approval,
            "approval-pending",
            NeedsYouStatus::Open,
        ),
        (
            NeedsYouItemKind::Approval,
            "approval-approved",
            NeedsYouStatus::Resolved,
        ),
        (
            NeedsYouItemKind::Approval,
            "approval-denied",
            NeedsYouStatus::Resolved,
        ),
        (
            NeedsYouItemKind::Approval,
            "approval-expired",
            NeedsYouStatus::Resolved,
        ),
        (
            NeedsYouItemKind::Approval,
            "approval-cancelled",
            NeedsYouStatus::Resolved,
        ),
        (
            NeedsYouItemKind::UserRequest,
            "request-pending",
            NeedsYouStatus::Open,
        ),
        (
            NeedsYouItemKind::UserRequest,
            "request-answered",
            NeedsYouStatus::Resolved,
        ),
        (
            NeedsYouItemKind::UserRequest,
            "request-dismissed",
            NeedsYouStatus::Resolved,
        ),
        (
            NeedsYouItemKind::UserRequest,
            "request-expired",
            NeedsYouStatus::Resolved,
        ),
        (
            NeedsYouItemKind::UserRequest,
            "request-cancelled",
            NeedsYouStatus::Resolved,
        ),
    ] {
        let item = page
            .items
            .iter()
            .find(|item| match kind {
                NeedsYouItemKind::Approval => item.approval_id.as_deref() == Some(id),
                NeedsYouItemKind::UserRequest => item.user_request_id.as_deref() == Some(id),
                NeedsYouItemKind::TaskBlocker => false,
            })
            .expect("expected source item");
        assert_eq!(item.kind, kind);
        assert_eq!(item.status, expected_status);
        assert!(!item.title.contains(SECRET_MARKER));
        assert!(!item.safe_summary.contains(SECRET_MARKER));
        assert!(!item.safe_summary.contains("locator"));
        assert_eq!(
            item.resolved_at.is_some(),
            expected_status == NeedsYouStatus::Resolved
        );
        if id == "approval-pending" {
            assert_eq!(
                item.item_id,
                "sha256:b41b88ce8aac34f6176e22ab0aef0b2460d0a47d51aef3f33a693462d11b9b37"
            );
            let wire = serde_json::to_value(item).expect("NeedsYouItem serializes");
            for field in [
                "approval_id",
                "user_request_id",
                "task_id",
                "blocker_id",
                "resolved_at",
            ] {
                assert!(
                    wire.get(field).is_some(),
                    "nullable wire field {field} is required"
                );
            }
            assert!(wire["user_request_id"].is_null());
            assert!(wire["blocker_id"].is_null());
        }
    }
}

#[test]
fn projection_is_workspace_scoped_and_validates_filter_and_page_limit() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_sources(&store, &directory);

    let foreign = store
        .list_needs_you_page(request("workspace-other", NeedsYouFilter::default(), 200))
        .expect("foreign Workspace reads only its own projection");
    assert!(foreign.items.is_empty());

    let filtered = NeedsYouFilter {
        kind: Some(NeedsYouItemKind::Approval),
        task_id: Some("task-main".to_owned()),
        status: Some(NeedsYouStatus::Open),
    };
    let page = store
        .list_needs_you_page(request("workspace-needs-you", filtered, 1))
        .expect("normalized filters apply");
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].approval_id.as_deref(),
        Some("approval-pending")
    );

    assert!(
        store
            .list_needs_you_page(request("workspace-needs-you", NeedsYouFilter::default(), 0))
            .is_err()
    );
    assert!(
        store
            .list_needs_you_page(request("", NeedsYouFilter::default(), 1))
            .is_err()
    );
}

#[test]
fn user_request_with_foreign_task_fails_closed_even_when_filtered_by_that_task() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_sources(&store, &directory);
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable fixture foreign keys for the missing test AgentSession");
    connection
        .execute(
            "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                kind, prompt, status, created_at)
             VALUES ('request-cross-workspace-task', 'workspace-needs-you', 'task-foreign',
                'session-missing-in-test-fixture', 'QUESTION', ?1, 'PENDING', ?2)",
            rusqlite::params![SECRET_MARKER, AT],
        )
        .expect("controlled corruption fixture bypasses admission trigger removed by seed_sources");

    for filter in [
        NeedsYouFilter::default(),
        NeedsYouFilter {
            task_id: Some("task-foreign".to_owned()),
            ..NeedsYouFilter::default()
        },
    ] {
        assert!(matches!(
            store.list_needs_you_page(request("workspace-needs-you", filter, 50)),
            Err(StoreError::Integrity(_))
        ));
    }
}

#[test]
fn cursor_is_deterministic_and_bound_to_workspace_filter_and_limit() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_sources(&store, &directory);

    let first = store
        .list_needs_you_page(request("workspace-needs-you", NeedsYouFilter::default(), 3))
        .expect("first page");
    assert_eq!(first.items.len(), 3);
    let cursor = first.next_cursor.expect("more than one page");
    let mut second_request = request("workspace-needs-you", NeedsYouFilter::default(), 3);
    second_request.cursor = Some(cursor.clone());
    let second = store
        .list_needs_you_page(second_request)
        .expect("second page follows stable cursor");
    assert_eq!(second.items.len(), 3);
    assert!(
        first
            .items
            .iter()
            .all(|item| second.items.iter().all(|next| item.item_id != next.item_id))
    );

    let mut all_items = first.items.clone();
    all_items.extend(second.items.clone());
    let mut next_cursor = second.next_cursor.clone();
    while let Some(cursor) = next_cursor {
        let mut next_request = request("workspace-needs-you", NeedsYouFilter::default(), 3);
        next_request.cursor = Some(cursor);
        let page = store
            .list_needs_you_page(next_request)
            .expect("later page follows stable cursor");
        all_items.extend(page.items);
        next_cursor = page.next_cursor;
    }
    assert_eq!(all_items.len(), 10);
    let unique_ids: std::collections::HashSet<_> =
        all_items.iter().map(|item| &item.item_id).collect();
    assert_eq!(unique_ids.len(), all_items.len());
    assert!(all_items.windows(2).all(|pair| {
        pair[0].created_at > pair[1].created_at
            || (pair[0].created_at == pair[1].created_at && pair[0].item_id < pair[1].item_id)
    }));

    let mut bad_workspace = request("workspace-other", NeedsYouFilter::default(), 3);
    bad_workspace.cursor = Some(cursor.clone());
    assert!(matches!(
        store.list_needs_you_page(bad_workspace),
        Err(StoreError::Invalid(_))
    ));

    let mut bad_limit = request("workspace-needs-you", NeedsYouFilter::default(), 2);
    bad_limit.cursor = Some(cursor.clone());
    assert!(matches!(
        store.list_needs_you_page(bad_limit),
        Err(StoreError::Invalid(_))
    ));

    let changed_filter = NeedsYouFilter {
        kind: Some(NeedsYouItemKind::Approval),
        ..NeedsYouFilter::default()
    };
    let mut bad_filter = request("workspace-needs-you", changed_filter, 3);
    bad_filter.cursor = Some(cursor);
    assert!(matches!(
        store.list_needs_you_page(bad_filter),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn malformed_blocker_json_fails_closed_but_nonblocked_task_json_is_not_projected() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    create_workspace(&store, "workspace-needs-you");
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable fixture foreign keys");
    insert_task(
        &connection,
        "workspace-needs-you",
        "task-ready",
        "READY",
        "not valid json",
    );
    assert!(
        store
            .list_needs_you_page(request(
                "workspace-needs-you",
                NeedsYouFilter::default(),
                50
            ))
            .expect("non-BLOCKED task has no current blocker projection")
            .items
            .is_empty()
    );

    insert_task(
        &connection,
        "workspace-needs-you",
        "task-malformed",
        "BLOCKED",
        "not valid json",
    );
    assert!(matches!(
        store.list_needs_you_page(request(
            "workspace-needs-you",
            NeedsYouFilter::default(),
            50
        )),
        Err(StoreError::Integrity(_))
    ));
}

#[test]
fn actionable_blockers_from_needs_user_tasks_appear_in_the_open_projection() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_sources(&store, &directory);
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable fixture foreign keys");
    let blockers = serde_json::json!([{
        "blocker_id": "needs-user-choice",
        "code": "STALE_FENCE",
        "safe_message": "Choose how to continue.",
        "resolution_hint": "Review the available options.",
        "created_at": "2026-10-14T00:02:00Z"
    }]);
    insert_task(
        &connection,
        "workspace-needs-you",
        "task-needs-user",
        "NEEDS_USER",
        &blockers.to_string(),
    );

    let filter = NeedsYouFilter {
        kind: Some(NeedsYouItemKind::TaskBlocker),
        task_id: Some("task-needs-user".to_owned()),
        ..NeedsYouFilter::default()
    };
    let page = store
        .list_needs_you_page(request("workspace-needs-you", filter.clone(), 10))
        .expect("NEEDS_USER blocker projection succeeds");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].task_id.as_deref(), Some("task-needs-user"));
    assert_eq!(
        page.items[0].blocker_id.as_deref(),
        Some("needs-user-choice")
    );
    assert_eq!(page.items[0].status, NeedsYouStatus::Open);

    let resolved_filter = NeedsYouFilter {
        status: Some(NeedsYouStatus::Resolved),
        ..filter
    };
    let resolved = store
        .list_needs_you_page(request("workspace-needs-you", resolved_filter, 10))
        .expect("current blocker has no synthetic resolved history");
    assert!(resolved.items.is_empty());
}

#[test]
fn blocker_projection_validates_shape_uses_safe_message_and_deduplicates_linked_sources() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_sources(&store, &directory);
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    let blockers = serde_json::json!([
        {
            "blocker_id": "safe-blocker",
            "code": "STALE_FENCE",
            "safe_message": "Reconcile the expired lease before continuing.",
            "resolution_hint": SECRET_MARKER,
            "created_at": "2026-10-14T00:02:00Z"
        },
        {
            "blocker_id": "linked-approval",
            "code": "APPROVAL_REQUIRED",
            "safe_message": "Approval is pending.",
            "resolution_hint": "Review the source record.",
            "approval_id": "approval-pending",
            "created_at": "2026-10-14T00:02:01Z"
        },
        {
            "blocker_id": "linked-request",
            "code": "USER_REQUEST_NOT_FOUND",
            "safe_message": "A response is pending.",
            "resolution_hint": "Review the source record.",
            "user_request_id": "request-pending",
            "created_at": "2026-10-14T00:02:02Z"
        }
    ]);
    connection
        .execute(
            "UPDATE tasks SET blocking_conditions_json = ?1 WHERE task_id = 'task-main'",
            [blockers.to_string()],
        )
        .expect("write valid test blockers");

    let page = store
        .list_needs_you_page(request(
            "workspace-needs-you",
            NeedsYouFilter::default(),
            50,
        ))
        .expect("blocker projection succeeds");
    assert_eq!(page.items.len(), 11);
    assert_eq!(
        page.items
            .iter()
            .filter(|item| item.kind == NeedsYouItemKind::TaskBlocker)
            .count(),
        1,
        "linked blockers resolve to their canonical source item"
    );
    let blocker = page
        .items
        .iter()
        .find(|item| item.kind == NeedsYouItemKind::TaskBlocker)
        .expect("unlinked blocker appears");
    assert_eq!(blocker.blocker_id.as_deref(), Some("safe-blocker"));
    assert_eq!(
        blocker.item_id,
        "sha256:efadf1022fec97ae5c895c033fac49a56e4d65152cf757594e6f87bf1dca9bec"
    );
    assert_eq!(
        blocker.safe_summary,
        "Reconcile the expired lease before continuing."
    );
    assert!(!blocker.safe_summary.contains(SECRET_MARKER));
    assert_eq!(blocker.resolved_at, None);
    let wire = serde_json::to_value(blocker).expect("blocker NeedsYouItem serializes");
    for field in [
        "approval_id",
        "user_request_id",
        "task_id",
        "blocker_id",
        "resolved_at",
    ] {
        assert!(
            wire.get(field).is_some(),
            "nullable wire field {field} is required"
        );
    }
    assert!(wire["approval_id"].is_null());
    assert!(wire["user_request_id"].is_null());

    let filter = NeedsYouFilter {
        kind: Some(NeedsYouItemKind::TaskBlocker),
        ..NeedsYouFilter::default()
    };
    let blockers_only = store
        .list_needs_you_page(request("workspace-needs-you", filter, 50))
        .expect("blocker filter succeeds");
    assert_eq!(blockers_only.items.len(), 1);
}

#[test]
fn blocker_links_that_are_dangling_or_outside_the_task_or_workspace_fail_closed() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory, Duration::from_secs(1));
    seed_sources(&store, &directory);
    let connection = Connection::open(state_database(&directory)).expect("open fixture database");
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable fixture foreign keys");
    let blocker = |approval_id: &str| {
        serde_json::json!([{
            "blocker_id": "linked",
            "code": "APPROVAL_REQUIRED",
            "safe_message": "An approval is pending.",
            "resolution_hint": "Review the source.",
            "approval_id": approval_id,
            "created_at": AT
        }])
        .to_string()
    };

    insert_task(
        &connection,
        "workspace-needs-you",
        "task-cross-scope",
        "BLOCKED",
        &blocker("approval-pending"),
    );
    assert!(matches!(
        store.list_needs_you_page(request(
            "workspace-needs-you",
            NeedsYouFilter::default(),
            50
        )),
        Err(StoreError::Integrity(_))
    ));

    connection
        .execute(
            "UPDATE tasks SET blocking_conditions_json = ?1 WHERE task_id = 'task-cross-scope'",
            ["[]"],
        )
        .expect("clear cross-scope blocker for next case");
    connection
        .execute(
            "UPDATE tasks SET blocking_conditions_json = ?1 WHERE task_id = 'task-main'",
            [blocker("approval-missing")],
        )
        .expect("write dangling link");
    assert!(matches!(
        store.list_needs_you_page(request(
            "workspace-needs-you",
            NeedsYouFilter::default(),
            50
        )),
        Err(StoreError::Integrity(_))
    ));

    connection
        .execute(
            "UPDATE tasks SET blocking_conditions_json = ?1 WHERE task_id = 'task-foreign'",
            [blocker("approval-pending")],
        )
        .expect("write cross-Workspace link");
    assert!(matches!(
        store.list_needs_you_page(request("workspace-other", NeedsYouFilter::default(), 50)),
        Err(StoreError::Integrity(_))
    ));
}
