use super::*;

use axum::http::header::CONTENT_TYPE;
use base64::Engine as _;
use std::{fs, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tower::ServiceExt;

#[tokio::test]
async fn indexed_resource_search_route_returns_exact_pinned_source_evidence_and_enforces_scope() {
    const WORKSPACE: &str = "indexed-search-route-workspace";
    const OWNER: &str = "indexed-search-route-owner";
    const RUNTIME: &str = "indexed-search-route-runtime";
    const NOW: &str = "2026-10-15T12:00:00Z";
    const CONTENT: &str = "alpha café needle beta";

    let directory = tempfile::tempdir().expect("temporary indexed-search directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private test directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            IpcReplayTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Indexed search route fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "indexed-search-route-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "indexed-search-route-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits");

    fn router_for(store: SqliteWorkspaceStore, principal_id: &str, runtime_id: &str) -> Router {
        let (_, stream_shutdown) = tokio::sync::watch::channel(false);
        super::super::build_operator_router(super::super::ApiState {
            store,
            principal_id: principal_id.to_owned(),
            runtime_id: runtime_id.to_owned(),
            local_incarnation_id: "indexed-search-route-incarnation".to_owned(),
            lifecycle_blockers: vec![],
            runtime_identity: Arc::new(
                storage_sqlite::RuntimeOsPrincipalIdentity::test_support(runtime_id)
                    .expect("test Runtime identity"),
            ),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_shutdown,
        })
    }

    async fn request(
        router: &Router,
        method: axum::http::Method,
        uri: &str,
        workspace_id: Option<&str>,
        idempotency_key: Option<&str>,
        body: Body,
    ) -> Response {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(CONTENT_TYPE, "application/json");
        if let Some(workspace_id) = workspace_id {
            request = request.header("x-workspace-id", workspace_id);
        }
        if let Some(idempotency_key) = idempotency_key {
            request = request.header("idempotency-key", idempotency_key);
        }
        let mut request = request.body(body).expect("Operator request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("Operator responds")
    }

    let owner_router = router_for(store.clone(), OWNER, RUNTIME);
    let import = request(
        &owner_router,
        axum::http::Method::POST,
        "/v1/resources/quick-import",
        Some(WORKSPACE),
        Some("indexed-search-route-import"),
        Body::from(
            serde_json::to_vec(&serde_json::json!({
                "display_name": "notes.txt",
                "media_type": "text/plain",
                "content_base64": base64::engine::general_purpose::STANDARD.encode(CONTENT),
            }))
            .expect("serialize quick-import request"),
        ),
    )
    .await;
    assert_eq!(import.status(), StatusCode::CREATED);
    let imported: serde_json::Value = serde_json::from_slice(
        &to_bytes(import.into_body(), 4096)
            .await
            .expect("bounded quick-import response"),
    )
    .expect("quick-import receipt is JSON");
    let resource_id = imported["resourceId"]
        .as_str()
        .expect("Resource ID is returned")
        .to_owned();
    let revision_id = imported["resourceRevisionId"]
        .as_str()
        .expect("Resource revision ID is returned")
        .to_owned();
    let digest = imported["contentDigest"]
        .as_str()
        .expect("content digest is returned")
        .to_owned();
    assert_eq!(
        digest,
        format!(
            "sha256:{}",
            hex::encode(sha2::Sha256::digest(CONTENT.as_bytes()))
        )
    );

    let rebuild = request(
        &owner_router,
        axum::http::Method::POST,
        &format!("/v1/resources/{resource_id}/text-index/rebuild"),
        Some(WORKSPACE),
        Some("indexed-search-route-rebuild"),
        Body::from(
            serde_json::to_vec(&serde_json::json!({
                "resource_revision_id": revision_id,
                "content_digest": digest,
            }))
            .expect("serialize rebuild request"),
        ),
    )
    .await;
    assert_eq!(rebuild.status(), StatusCode::OK);
    let rebuild: serde_json::Value = serde_json::from_slice(
        &to_bytes(rebuild.into_body(), 4096)
            .await
            .expect("bounded index rebuild response"),
    )
    .expect("index rebuild response is JSON");
    assert_eq!(rebuild["outcome"], "INDEXED");
    assert_eq!(rebuild["resourceId"], resource_id);
    assert_eq!(
        rebuild["resourceRevisionId"],
        imported["resourceRevisionId"]
    );
    assert_eq!(rebuild["contentDigest"], imported["contentDigest"]);

    let search = request(
        &owner_router,
        axum::http::Method::GET,
        "/v1/resources/search?mode=INDEXED_CONTENT&q=caf%C3%A9+needle",
        Some(WORKSPACE),
        None,
        Body::empty(),
    )
    .await;
    assert_eq!(search.status(), StatusCode::OK);
    let page: serde_json::Value = serde_json::from_slice(
        &to_bytes(search.into_body(), 16 * 1024)
            .await
            .expect("bounded indexed-search response"),
    )
    .expect("indexed-search response is JSON");
    assert_eq!(page["mode"], "INDEXED_CONTENT");
    assert!(page["content_scan"].is_null());
    assert!(page["next_cursor"].is_null());
    let items = page["items"].as_array().expect("search items are an array");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item["resource_ref"]["workspace_id"], WORKSPACE);
    assert_eq!(item["resource_ref"]["resource_id"], resource_id);
    assert_eq!(
        item["resource_ref"]["revision_id"],
        imported["resourceRevisionId"]
    );
    assert_eq!(item["source_content_digest"], imported["contentDigest"]);
    assert_eq!(item["snippet"], CONTENT);
    assert!(item["snippet"].as_str().unwrap().len() <= 320);
    assert_eq!(
        item["match_reasons"],
        serde_json::json!(["CONTENT_INDEXED"])
    );
    let spans = item["source_matches"]
        .as_array()
        .expect("indexed results include source spans");
    assert_eq!(spans.len(), 2);
    let span_for = |term: &str| {
        spans
            .iter()
            .find(|span| span["term"] == term)
            .unwrap_or_else(|| panic!("missing source span for {term}"))
    };
    assert_eq!(span_for("café")["start_utf8_byte"], 6);
    assert_eq!(span_for("café")["end_utf8_byte_exclusive"], 11);
    assert_eq!(span_for("needle")["start_utf8_byte"], 12);
    assert_eq!(span_for("needle")["end_utf8_byte_exclusive"], 18);

    let empty_query = request(
        &owner_router,
        axum::http::Method::GET,
        "/v1/resources/search?mode=INDEXED_CONTENT&q=",
        Some(WORKSPACE),
        None,
        Body::empty(),
    )
    .await;
    assert_eq!(empty_query.status(), StatusCode::BAD_REQUEST);
    let empty_error: serde_json::Value = serde_json::from_slice(
        &to_bytes(empty_query.into_body(), 4096)
            .await
            .expect("bounded empty-query error"),
    )
    .expect("typed empty-query error is JSON");
    assert_eq!(empty_error["code"], "INVALID_ARGUMENT");

    let missing_workspace = request(
        &owner_router,
        axum::http::Method::GET,
        "/v1/resources/search?mode=INDEXED_CONTENT&q=needle",
        None,
        None,
        Body::empty(),
    )
    .await;
    assert_eq!(missing_workspace.status(), StatusCode::BAD_REQUEST);

    let foreign_router = router_for(store, "indexed-search-route-foreign-owner", RUNTIME);
    let foreign_workspace = request(
        &foreign_router,
        axum::http::Method::GET,
        "/v1/resources/search?mode=INDEXED_CONTENT&q=needle",
        Some(WORKSPACE),
        None,
        Body::empty(),
    )
    .await;
    assert_eq!(foreign_workspace.status(), StatusCode::FORBIDDEN);
    let foreign_error: serde_json::Value = serde_json::from_slice(
        &to_bytes(foreign_workspace.into_body(), 4096)
            .await
            .expect("bounded foreign-workspace error"),
    )
    .expect("typed foreign-workspace error is JSON");
    assert_eq!(foreign_error["code"], "FORBIDDEN");
}

#[tokio::test]
async fn indexed_resource_search_route_paginates_without_duplicates() {
    const WORKSPACE: &str = "indexed-search-pages-workspace";
    const OWNER: &str = "indexed-search-pages-owner";
    const RUNTIME: &str = "indexed-search-pages-runtime";
    const NOW: &str = "2026-10-15T12:00:00Z";
    const QUERY: &str = "sharedneedle";
    const SEARCH_PREFIX: &str =
        "/v1/resources/search?mode=INDEXED_CONTENT&q=sharedneedle&kind=FILE&limit=1";

    let directory = tempfile::tempdir().expect("temporary indexed-search pagination directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private test directory");
    let store = SqliteWorkspaceStore::open(
        directory.path().join("state.sqlite3"),
        Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            IpcReplayTestKeys,
        )),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens");
    WorkspaceService::new(store.clone())
        .create(CreateWorkspace {
            workspace_id: WORKSPACE.to_owned(),
            name: "Indexed search pagination fixture".to_owned(),
            owner_principal_id: OWNER.to_owned(),
            event: EventContext {
                event_id: "indexed-search-pages-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME.to_owned(),
                hlc_timestamp: NOW.to_owned(),
                correlation_id: "indexed-search-pages-workspace-created".to_owned(),
                causation_id: None,
                recorded_at: NOW.to_owned(),
            },
        })
        .expect("Workspace commits");

    fn router_for(store: SqliteWorkspaceStore) -> Router {
        let (_, stream_shutdown) = tokio::sync::watch::channel(false);
        super::super::build_operator_router(super::super::ApiState {
            store,
            principal_id: OWNER.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: "indexed-search-pages-incarnation".to_owned(),
            lifecycle_blockers: vec![],
            runtime_identity: Arc::new(
                storage_sqlite::RuntimeOsPrincipalIdentity::test_support(RUNTIME)
                    .expect("test Runtime identity"),
            ),
            stream_admission: Arc::new(tokio::sync::Semaphore::new(2)),
            runtime_inventory_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            stream_shutdown,
        })
    }

    async fn request(router: &Router, uri: &str) -> Response {
        let mut request = Request::get(uri)
            .header(CONTENT_TYPE, "application/json")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("Operator request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("Operator responds")
    }

    let router = router_for(store);
    let fixtures = [
        ("pagination-alpha.txt", "alpha sharedneedle source"),
        ("pagination-bravo.txt", "bravo sharedneedle source"),
        ("pagination-charlie.txt", "charlie sharedneedle source"),
    ];
    let mut expected_pins = std::collections::BTreeMap::new();
    for (ordinal, (display_name, content)) in fixtures.into_iter().enumerate() {
        let mut import_request = Request::post("/v1/resources/quick-import")
            .header(CONTENT_TYPE, "application/json")
            .header("x-workspace-id", WORKSPACE)
            .header(
                "idempotency-key",
                match ordinal {
                    0 => "indexed-search-pages-import-alpha",
                    1 => "indexed-search-pages-import-bravo",
                    _ => "indexed-search-pages-import-charlie",
                },
            )
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "display_name": display_name,
                    "media_type": "text/plain",
                    "content_base64": base64::engine::general_purpose::STANDARD.encode(content),
                }))
                .expect("serialize quick-import request"),
            ))
            .expect("quick-import request builds");
        import_request
            .extensions_mut()
            .insert(AuthenticatedLocalPeer);
        let import = router
            .clone()
            .oneshot(import_request)
            .await
            .expect("quick-import route responds");
        assert_eq!(import.status(), StatusCode::CREATED);
        let imported: serde_json::Value = serde_json::from_slice(
            &to_bytes(import.into_body(), 4096)
                .await
                .expect("bounded quick-import response"),
        )
        .expect("quick-import receipt is JSON");
        let resource_id = imported["resourceId"]
            .as_str()
            .expect("Resource ID is returned")
            .to_owned();
        let revision_id = imported["resourceRevisionId"]
            .as_str()
            .expect("Resource revision ID is returned")
            .to_owned();
        let digest = imported["contentDigest"]
            .as_str()
            .expect("content digest is returned")
            .to_owned();
        assert_eq!(
            digest,
            format!(
                "sha256:{}",
                hex::encode(sha2::Sha256::digest(content.as_bytes()))
            )
        );

        let rebuild = Request::post(format!("/v1/resources/{resource_id}/text-index/rebuild"))
            .header(CONTENT_TYPE, "application/json")
            .header("x-workspace-id", WORKSPACE)
            .header(
                "idempotency-key",
                format!("indexed-search-pages-rebuild-{ordinal}"),
            )
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "resource_revision_id": revision_id,
                    "content_digest": digest,
                }))
                .expect("serialize index rebuild request"),
            ))
            .expect("index rebuild request builds");
        let mut rebuild = rebuild;
        rebuild.extensions_mut().insert(AuthenticatedLocalPeer);
        let rebuilt = router
            .clone()
            .oneshot(rebuild)
            .await
            .expect("index rebuild route responds");
        assert_eq!(rebuilt.status(), StatusCode::OK);
        let rebuilt: serde_json::Value = serde_json::from_slice(
            &to_bytes(rebuilt.into_body(), 4096)
                .await
                .expect("bounded index rebuild response"),
        )
        .expect("index rebuild response is JSON");
        assert_eq!(rebuilt["outcome"], "INDEXED");
        assert_eq!(rebuilt["resourceId"], resource_id);
        assert_eq!(rebuilt["resourceRevisionId"], revision_id);
        assert_eq!(rebuilt["contentDigest"], digest);
        expected_pins.insert(resource_id, (revision_id, digest, display_name.to_owned()));
    }

    let resource_list = request(&router, "/v1/resources?limit=100").await;
    assert_eq!(resource_list.status(), StatusCode::OK);
    let resource_list: serde_json::Value = serde_json::from_slice(
        &to_bytes(resource_list.into_body(), 16 * 1024)
            .await
            .expect("bounded resource-list response"),
    )
    .expect("resource-list response is JSON");
    let expected_order = resource_list["items"]
        .as_array()
        .expect("Resource items are an array")
        .iter()
        .filter_map(|item| {
            let resource_id = item["resourceId"].as_str()?;
            expected_pins.contains_key(resource_id).then_some((
                item["createdAt"].as_str()?.to_owned(),
                resource_id.to_owned(),
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(expected_order.len(), 3, "all imported Resources are listed");
    let expected_order = {
        let mut order = expected_order;
        order.sort_by(|(left_time, left_id), (right_time, right_id)| {
            right_time
                .cmp(left_time)
                .then_with(|| right_id.cmp(left_id))
        });
        order
            .into_iter()
            .map(|(_, resource_id)| resource_id)
            .collect::<Vec<_>>()
    };

    let mut cursor: Option<String> = None;
    let mut observed = Vec::new();
    let mut seen_cursors = std::collections::BTreeSet::new();
    for page_number in 0..4 {
        let uri = cursor.as_ref().map_or_else(
            || SEARCH_PREFIX.to_owned(),
            |cursor| format!("{SEARCH_PREFIX}&cursor={cursor}"),
        );
        let response = request(&router, &uri).await;
        assert_eq!(response.status(), StatusCode::OK);
        let page: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), 16 * 1024)
                .await
                .expect("bounded indexed-search page"),
        )
        .expect("indexed-search page is JSON");
        assert_eq!(page["mode"], "INDEXED_CONTENT");
        assert!(page["content_scan"].is_null());
        let items = page["items"].as_array().expect("items are an array");
        assert!(items.len() <= 1, "page respects limit=1");
        for item in items {
            let resource_id = item["resource_ref"]["resource_id"]
                .as_str()
                .expect("result pins its Resource")
                .to_owned();
            let (revision_id, digest, display_name) = expected_pins
                .get(&resource_id)
                .expect("search result is one of the imported Resources");
            assert_eq!(item["resource_ref"]["workspace_id"], WORKSPACE);
            assert_eq!(
                item["resource_ref"]["revision_id"].as_str(),
                Some(revision_id.as_str())
            );
            assert_eq!(
                item["source_content_digest"].as_str(),
                Some(digest.as_str())
            );
            assert_eq!(item["display_name"].as_str(), Some(display_name.as_str()));
            assert_eq!(
                item["match_reasons"],
                serde_json::json!(["CONTENT_INDEXED"])
            );
            let spans = item["source_matches"]
                .as_array()
                .expect("indexed result includes source spans");
            assert_eq!(spans.len(), 1);
            assert_eq!(spans[0]["term"], QUERY);
            observed.push(resource_id);
        }

        let next_cursor = page["next_cursor"].as_str().map(str::to_owned);
        if page_number == 0 {
            let first_cursor = next_cursor
                .as_ref()
                .expect("first one-item page has a continuation");
            let changed_query = request(
                &router,
                &format!(
                    "/v1/resources/search?mode=INDEXED_CONTENT&q=otherneedle&kind=FILE&limit=1&cursor={first_cursor}"
                ),
            )
            .await;
            assert_eq!(changed_query.status(), StatusCode::BAD_REQUEST);
            let changed_filter = request(
                &router,
                &format!(
                    "/v1/resources/search?mode=INDEXED_CONTENT&q=sharedneedle&kind=OTHER&limit=1&cursor={first_cursor}"
                ),
            )
            .await;
            assert_eq!(changed_filter.status(), StatusCode::BAD_REQUEST);
        }
        match (&cursor, &next_cursor) {
            (Some(previous), Some(next)) => assert_ne!(previous, next),
            (_, Some(next)) => assert!(seen_cursors.insert(next.clone())),
            (_, None) => {}
        }
        cursor = next_cursor;
        if cursor.is_none() {
            break;
        }
    }

    assert_eq!(
        observed.len(),
        3,
        "all matching Resources are paged exactly once"
    );
    assert_eq!(
        observed
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(
        observed, expected_order,
        "pages follow the Resource keyset order"
    );
    assert!(
        cursor.is_none(),
        "the final page has a null continuation cursor"
    );
}
