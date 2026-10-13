use super::super::resource_dependencies_operator::{
    ResourceDependenciesApiState, test_router as resource_dependencies_test_router,
};
use super::{AuthenticatedLocalPeer, IpcReplayTestKeys};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
use rusqlite::{Connection, params};
use serde_json::Value;
use std::{fs, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use storage_sqlite::{FileBlobStore, SqliteConfig, SqliteWorkspaceStore};
use tower::ServiceExt;

#[tokio::test]
async fn resource_dependents_route_is_owner_scoped_paginated_and_closed() {
    const WORKSPACE: &str = "dependents-route-workspace";
    const OWNER: &str = "dependents-route-owner";
    const NOW: &str = "2026-10-12T12:00:00Z";

    let directory = tempfile::tempdir().expect("temporary dependency route directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private route fixture directory");
    let database = directory.path().join("state.sqlite3");
    let store = SqliteWorkspaceStore::open(
        &database,
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
    for (workspace_id, name) in [
        (WORKSPACE, "Selected Workspace"),
        ("dependents-route-other-workspace", "Other owned Workspace"),
    ] {
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: workspace_id.to_owned(),
                name: name.to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: format!("{workspace_id}-created"),
                    origin_runtime_id: "dependents-route-runtime".to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: format!("{workspace_id}-create"),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("Workspace commits");
    }

    let mut connection = Connection::open(&database).expect("fixture SQLite connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable fixture constraints");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction.execute(
        "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('source-resource', ?1, 'FILE', '{}', 'Source', 'source-r2', 'PERSONAL', '{}', ?2, ?2, 2)",
        params![WORKSPACE, NOW],
    ).expect("source Resource");
    for revision_id in ["source-r1", "source-r2"] {
        transaction.execute(
            "INSERT INTO resource_revisions(resource_revision_id, resource_id, observed_at, created_by_json) VALUES (?1, 'source-resource', ?2, '{}')",
            params![revision_id, NOW],
        ).expect("source revision");
    }
    transaction.execute(
        "INSERT INTO resource_revision_parents(resource_id, child_revision_id, parent_revision_id) VALUES ('source-resource', 'source-r2', 'source-r1')",
        [],
    ).expect("source ancestry");
    for (suffix, consumed_revision) in [("a", "source-r1"), ("b", "source-r2")] {
        let resource_id = format!("dependent-resource-{suffix}");
        let revision_id = format!("dependent-resource-{suffix}-r1");
        let artifact_id = format!("dependent-artifact-{suffix}");
        let input_refs = serde_json::to_string(&[serde_json::json!({
            "workspace_id": WORKSPACE,
            "resource_id": "source-resource",
            "revision_id": consumed_revision,
        })])
        .expect("input reference JSON");
        transaction.execute(
            "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES (?1, ?2, 'ARTIFACT', '{}', ?1, ?3, 'PERSONAL', '{}', ?4, ?4, 1)",
            params![resource_id, WORKSPACE, revision_id, NOW],
        ).expect("dependent Artifact Resource");
        transaction.execute(
            "INSERT INTO resource_revisions(resource_revision_id, resource_id, observed_at, created_by_json) VALUES (?1, ?2, ?3, '{}')",
            params![revision_id, resource_id, NOW],
        ).expect("dependent Artifact revision");
        transaction.execute(
            "INSERT INTO artifacts(artifact_id, workspace_id, resource_id, kind, display_name, current_version, library_status, created_at) VALUES (?1, ?2, ?3, 'document', ?1, 1, 'TRANSIENT', ?4)",
            params![artifact_id, WORKSPACE, resource_id, NOW],
        ).expect("dependent Artifact");
        transaction.execute(
            "INSERT INTO artifact_versions(artifact_id, resource_id, version, resource_revision_id, input_refs_json, content_kind, content_digest, storage_ref_json, content_media_type, content_size_bytes, provenance_json, created_at) VALUES (?1, ?2, 1, ?3, ?4, 'MANAGED_BLOB', ?5, '{}', 'text/plain', 0, '{}', ?6)",
            params![artifact_id, resource_id, revision_id, input_refs, format!("sha256:{}", "a".repeat(64)), NOW],
        ).expect("dependent Artifact version");
        transaction.execute(
            "INSERT INTO dependency_edges(dependency_edge_id, workspace_id, source_resource_id, source_revision_id, dependent_kind, dependent_ref, artifact_id, artifact_version, created_at) VALUES (?1, ?2, 'source-resource', ?3, 'ARTIFACT_VERSION', ?4, ?5, 1, ?6)",
            params![
                format!("edge-{suffix}"),
                WORKSPACE,
                consumed_revision,
                format!("artifact://{WORKSPACE}/{artifact_id}@v1"),
                artifact_id,
                NOW,
            ],
        ).expect("dependency edge");
    }
    transaction.execute(
        "INSERT INTO invalidation_records(invalidation_record_id, dependency_edge_id, observed_revision_id, reason_code, created_at) VALUES ('source-r2-invalidated-edge-a', 'edge-a', 'source-r2', 'SOURCE_RESOURCE_REVISION_ADVANCED', '2026-10-12T12:01:00Z')",
        [],
    ).expect("immutable invalidation");
    transaction.execute(
        "INSERT INTO resources(resource_id, workspace_id, kind, provider_identity_json, display_name, current_revision_id, sensitivity, provenance_json, created_at, updated_at, version) VALUES ('other-workspace-source', 'dependents-route-other-workspace', 'FILE', '{}', 'Other source', NULL, 'PERSONAL', '{}', ?1, ?1, 1)",
        [NOW],
    ).expect("same-owner Resource in another Workspace");
    transaction.commit().expect("fixture commits");

    let router = resource_dependencies_test_router(ResourceDependenciesApiState::new(
        store.clone(),
        OWNER.to_owned(),
    ));
    let mut request = Request::get("/v1/resources/source-resource/dependents?limit=1")
        .header("x-workspace-id", WORKSPACE)
        .body(Body::empty())
        .expect("request is valid");
    request.extensions_mut().insert(AuthenticatedLocalPeer);
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("route returns first page");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = to_bytes(response.into_body(), 4096)
        .await
        .expect("bounded response body");
    let page: Value = serde_json::from_slice(&body).expect("valid page JSON");
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        page["items"][0]["dependency_edge"]["dependency_edge_id"],
        "edge-a"
    );
    assert_eq!(page["items"][0]["dependent_freshness"], "STALE");
    assert_eq!(
        page["items"][0]["latest_invalidation"]["invalidation_record_id"],
        "source-r2-invalidated-edge-a"
    );
    assert_eq!(
        page["items"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "dependency_edge",
            "dependent_freshness",
            "latest_invalidation"
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(
        page["items"][0]["dependency_edge"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "created_at",
            "dependency_edge_id",
            "dependent_kind",
            "dependent_ref",
            "source_resource_id",
            "source_revision_id",
            "workspace_id"
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    let cursor = page["next_cursor"].as_str().expect("continuation cursor");

    let mut second_request = Request::get(format!(
        "/v1/resources/source-resource/dependents?limit=1&cursor={cursor}"
    ))
    .header("x-workspace-id", WORKSPACE)
    .body(Body::empty())
    .expect("continuation request is valid");
    second_request
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    let second_response = router
        .clone()
        .oneshot(second_request)
        .await
        .expect("route returns continuation page");
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body = to_bytes(second_response.into_body(), 4096)
        .await
        .expect("bounded continuation body");
    let second_page: Value = serde_json::from_slice(&second_body).expect("valid continuation JSON");
    assert_eq!(
        second_page["items"][0]["dependency_edge"]["dependency_edge_id"],
        "edge-b"
    );
    assert_eq!(second_page["items"][0]["dependent_freshness"], "CURRENT");
    assert!(second_page["next_cursor"].is_null());

    let mut changed_limit = Request::get(format!(
        "/v1/resources/source-resource/dependents?limit=2&cursor={cursor}"
    ))
    .header("x-workspace-id", WORKSPACE)
    .body(Body::empty())
    .expect("changed limit request is valid");
    changed_limit
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    assert_eq!(
        router
            .clone()
            .oneshot(changed_limit)
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );

    let mut changed_resource = Request::get(format!(
        "/v1/resources/other-workspace-source/dependents?limit=1&cursor={cursor}"
    ))
    .header("x-workspace-id", WORKSPACE)
    .body(Body::empty())
    .expect("changed Resource cursor request is valid HTTP");
    changed_resource
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    assert_eq!(
        router
            .clone()
            .oneshot(changed_resource)
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );

    let mut foreign_workspace_resource =
        Request::get("/v1/resources/other-workspace-source/dependents")
            .header("x-workspace-id", WORKSPACE)
            .body(Body::empty())
            .expect("foreign Workspace Resource request is valid");
    foreign_workspace_resource
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    assert_eq!(
        router
            .clone()
            .oneshot(foreign_workspace_resource)
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    let mut missing_resource = Request::get("/v1/resources/missing-source/dependents")
        .header("x-workspace-id", WORKSPACE)
        .body(Body::empty())
        .expect("missing Resource request is valid");
    missing_resource
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    assert_eq!(
        router
            .clone()
            .oneshot(missing_resource)
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    let foreign_owner_router = resource_dependencies_test_router(
        ResourceDependenciesApiState::new(store, "other-principal".to_owned()),
    );
    let mut foreign_owner = Request::get("/v1/resources/source-resource/dependents")
        .header("x-workspace-id", WORKSPACE)
        .body(Body::empty())
        .expect("foreign owner request is valid");
    foreign_owner
        .extensions_mut()
        .insert(AuthenticatedLocalPeer);
    assert_eq!(
        foreign_owner_router
            .oneshot(foreign_owner)
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
}
