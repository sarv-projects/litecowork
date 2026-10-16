//! Read-only, selected-Workspace UserRequest detail for the local Needs You surface.

#![allow(clippy::result_large_err)]

use super::*;
use axum::extract::FromRef;
use storage_core::{
    USER_REQUEST_DETAIL_MAX_BYTES, USER_REQUEST_ID_MAX_BYTES, UserRequestDetailReadStore,
};

#[derive(Clone)]
pub(super) struct UserRequestApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
}

impl FromRef<ApiState> for UserRequestApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
        }
    }
}

pub(super) fn routes() -> Router<UserRequestApiState> {
    Router::new()
        .route(
            "/v1/user-requests/{request_id}",
            get(get_user_request_detail),
        )
        .route_layer(middleware::from_fn(authenticate))
        .route_layer(middleware::from_fn(user_request_no_store))
}

async fn user_request_no_store(request: Request<Body>, next: Next) -> Response {
    add_no_store(next.run(request).await)
}

async fn get_user_request_detail(
    State(state): State<UserRequestApiState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
) -> Response {
    let result = (|| -> Result<_, Response> {
        let workspace_id = selected_workspace(&headers)?;
        if !valid_user_request_id(&request_id) {
            return Err(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "UserRequest identifier is invalid",
            ));
        }
        Ok(workspace_id)
    })();
    let workspace_id = match result {
        Ok(workspace_id) => workspace_id,
        Err(response) => return add_no_store(response),
    };

    let owner = state.principal_id.clone();
    let selected = workspace_id.clone();
    let request_for_read = request_id.clone();
    let store = state.store.clone();
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &owner, &selected)?;
        store
            .get_user_request_detail(&selected, &request_for_read)
            .map_err(user_request_detail_store_error)
    })
    .await;
    let detail = match result {
        Ok(Ok(Some(detail))) => detail,
        Ok(Ok(None)) => {
            return add_no_store(operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "UserRequest is unavailable",
            ));
        }
        Ok(Err(response)) => return add_no_store(response),
        Err(_) => {
            return add_no_store(operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "UserRequest details are unavailable",
            ));
        }
    };
    let encoded = match serde_json::to_vec(&detail) {
        Ok(encoded) if encoded.len() <= USER_REQUEST_DETAIL_MAX_BYTES => encoded,
        _ => {
            return add_no_store(operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "UserRequest details exceed the local read boundary",
            ));
        }
    };
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(encoded))
        .unwrap_or_else(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "UserRequest details could not be prepared",
            )
        });
    add_no_store(response)
}

fn valid_user_request_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= USER_REQUEST_ID_MAX_BYTES
        && !value.chars().any(char::is_control)
}

fn user_request_detail_store_error(error: StoreError) -> Response {
    match error {
        StoreError::Invalid(_) => operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "UserRequest identifier is invalid",
        ),
        StoreError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "UserRequest is unavailable",
        ),
        StoreError::Integrity(_) => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "UserRequest details could not be verified",
        ),
        _ => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "UserRequest details are unavailable",
        ),
    }
}

fn add_no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
fn test_router(store: SqliteWorkspaceStore, principal_id: String) -> Router {
    routes().with_state(UserRequestApiState {
        store,
        principal_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::Connection;
    use std::{sync::Arc, time::Duration};
    use storage_core::{BlobPurpose, StoreError};
    use storage_sqlite::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
    use zeroize::Zeroizing;

    const OWNER: &str = "user-request-detail-owner";
    const WORKSPACE: &str = "user-request-detail-workspace";
    const FOREIGN_WORKSPACE: &str = "user-request-detail-other";
    const OTHER_OWNER_WORKSPACE: &str = "user-request-detail-other-owner";
    const OTHER_OWNER: &str = "user-request-detail-other-principal";
    const AT: &str = "2026-10-16T10:00:00Z";

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([59; 32]),
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

    fn fixture() -> (tempfile::TempDir, SqliteWorkspaceStore, Router) {
        let directory = tempfile::tempdir().expect("temporary UserRequest route directory");
        let database = directory.path().join("state.sqlite3");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("restrict test state directory to its owner");
        }
        let store = SqliteWorkspaceStore::open(
            &database,
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("real SQLite store opens");
        for (workspace_id, suffix, owner) in [
            (WORKSPACE, "main", OWNER),
            (FOREIGN_WORKSPACE, "foreign", OWNER),
            (OTHER_OWNER_WORKSPACE, "other-owner", OTHER_OWNER),
        ] {
            WorkspaceService::new(store.clone())
                .create(CreateWorkspace {
                    workspace_id: workspace_id.to_owned(),
                    name: "UserRequest details fixture".to_owned(),
                    owner_principal_id: owner.to_owned(),
                    event: EventContext {
                        event_id: format!("user-request-ws-{suffix}"),
                        origin_runtime_id: "user-request-runtime".to_owned(),
                        hlc_timestamp: AT.to_owned(),
                        correlation_id: format!("user-request-correlation-{suffix}"),
                        causation_id: None,
                        recorded_at: AT.to_owned(),
                    },
                })
                .expect("Workspace commits");
        }
        let connection = Connection::open(&database).expect("open fixture SQLite database");
        // This projection fixture avoids constructing the full Task/Attempt/session
        // admission graph. Disable only the fixture's FK and scope-admission guards to
        // seed synthetic rows for read isolation checks; production connections retain them.
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .expect("disable fixture-only foreign keys");
        connection
            .execute_batch("DROP TRIGGER user_request_admission_matches_scope;")
            .expect("remove admission-only test guard");
        for (task_id, workspace_id) in [
            ("task-visible", WORKSPACE),
            ("task-other", FOREIGN_WORKSPACE),
            ("task-private", OTHER_OWNER_WORKSPACE),
        ] {
            connection
                .execute(
                    "INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status,
                        lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                        created_at, updated_at, version)
                     VALUES (?1, ?2, 1, 'READY', 'missing-test-binding',
                        '[]', 'NORMAL', '{}', ?3, ?3, 1)",
                    rusqlite::params![task_id, workspace_id, AT],
                )
                .expect("same-Workspace Task parent exists for the request detail fixture");
        }
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                    kind, interaction_mode, prompt, choices_json, status, created_at, version)
                 VALUES ('request-visible', ?1, 'task-visible', 'session-internal', 'QUESTION',
                    'FORM', '<img src=x onerror=alert(1)>',
                    '[{\"choice_id\":\"choice-visible\",\"label\":\"Option\",\"value\":\"one\"}]',
                    'PENDING', ?2, 1)",
                rusqlite::params![WORKSPACE, AT],
            )
            .expect("valid UserRequest fixture inserts");
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                    kind, interaction_mode, prompt, choices_json, status, created_at, version)
                 VALUES ('request-foreign', ?1, 'task-other', 'session-other', 'QUESTION',
                    'FORM', 'Not for this Workspace', '[]', 'PENDING', ?2, 1)",
                rusqlite::params![FOREIGN_WORKSPACE, AT],
            )
            .expect("foreign UserRequest fixture inserts");
        connection
            .execute(
                "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id,
                    kind, interaction_mode, prompt, choices_json, status, created_at, version)
                 VALUES ('request-protected', ?1, 'task-private', 'session-private', 'QUESTION',
                    'FORM', 'Private owner data', '[]', 'PENDING', ?2, 1)",
                rusqlite::params![OTHER_OWNER_WORKSPACE, AT],
            )
            .expect("other-owner UserRequest fixture inserts");
        drop(connection);
        let router = test_router(store.clone(), OWNER.to_owned());
        (directory, store, router)
    }

    async fn get(router: Router, workspace_id: &str, request_id: &str) -> Response {
        let mut request = Request::get(format!("/v1/user-requests/{request_id}"))
            .header("x-workspace-id", workspace_id)
            .body(Body::empty())
            .expect("request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .oneshot(request)
            .await
            .expect("router returns response")
    }

    #[tokio::test]
    async fn selected_workspace_detail_is_exact_current_and_no_store() {
        let (_directory, _store, router) = fixture();
        let response = get(router, WORKSPACE, "request-visible").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("bounded request detail");
        let body: Value = serde_json::from_slice(&body).expect("valid UserRequest JSON");
        assert_eq!(body["request_id"], "request-visible");
        assert_eq!(body["workspace_id"], WORKSPACE);
        assert_eq!(body["scope"]["kind"], "TASK_PLANNING");
        assert_eq!(body["scope"]["task_id"], "task-visible");
        assert_eq!(body["status"], "PENDING");
        assert_eq!(body["version"], 1);
        assert_eq!(body["choices"][0]["label"], "Option");
        assert!(body["choices"][0].get("value").is_none());
        assert!(body.get("provider_url").is_none());
        assert!(body.get("provider_input_binding").is_none());
        assert!(body.get("agent_session_id").is_none());
    }

    #[tokio::test]
    async fn missing_and_foreign_workspace_requests_are_nondisclosing_not_found() {
        let (_directory, _store, router) = fixture();
        let foreign = get(router.clone(), WORKSPACE, "request-foreign").await;
        let missing = get(router.clone(), WORKSPACE, "request-absent").await;
        for response in [&foreign, &missing] {
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
        let foreign = to_bytes(foreign.into_body(), 16 * 1024)
            .await
            .expect("bounded error");
        let missing = to_bytes(missing.into_body(), 16 * 1024)
            .await
            .expect("bounded error");
        let foreign: Value = serde_json::from_slice(&foreign).expect("foreign error JSON");
        let missing: Value = serde_json::from_slice(&missing).expect("missing error JSON");
        assert_eq!(foreign["code"], missing["code"]);
        assert_eq!(foreign["message"], missing["message"]);

        let wrong_owner = get(router, OTHER_OWNER_WORKSPACE, "request-protected").await;
        assert_eq!(wrong_owner.status(), StatusCode::FORBIDDEN);
        assert_eq!(wrong_owner.headers()[header::CACHE_CONTROL], "no-store");
        let wrong_owner = to_bytes(wrong_owner.into_body(), 16 * 1024)
            .await
            .expect("bounded owner error");
        let wrong_owner = String::from_utf8_lossy(&wrong_owner);
        assert!(!wrong_owner.contains("request-visible"));
        assert!(!wrong_owner.contains("request-protected"));
        assert!(!wrong_owner.contains("Which option?"));
    }

    #[tokio::test]
    async fn route_rejects_unauthenticated_missing_workspace_and_malformed_ids() {
        let (_directory, _store, router) = fixture();
        let response = router
            .clone()
            .oneshot(
                Request::get("/v1/user-requests/request-visible")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("auth response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        let mut missing_workspace = Request::get("/v1/user-requests/request-visible")
            .body(Body::empty())
            .expect("request builds");
        missing_workspace
            .extensions_mut()
            .insert(AuthenticatedLocalPeer);
        let response = router
            .clone()
            .oneshot(missing_workspace)
            .await
            .expect("missing Workspace response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        let response = get(
            router.clone(),
            WORKSPACE,
            &"x".repeat(USER_REQUEST_ID_MAX_BYTES),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        let response = get(
            router,
            WORKSPACE,
            &"x".repeat(USER_REQUEST_ID_MAX_BYTES + 1),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn route_reads_the_latest_terminal_status_and_version() {
        let (directory, _store, router) = fixture();
        let connection = Connection::open(directory.path().join("state.sqlite3"))
            .expect("open request fixture before route read");
        connection
            .execute(
                "UPDATE user_requests SET status = 'DISMISSED', resolved_at = ?1, version = 2
             WHERE request_id = 'request-visible'",
                [AT],
            )
            .expect("request resolves before route read");
        let response = get(router, WORKSPACE, "request-visible").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("bounded body");
        let body: Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(body["status"], "DISMISSED");
        assert_eq!(body["version"], 2);
    }
}
