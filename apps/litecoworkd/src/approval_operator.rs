//! Authenticated, local-only owner resolution of exact-action Trust approvals.
//! Resolution records a one-time decision; it never grants authority or dispatches.

#![allow(clippy::result_large_err)]

use super::*;
use storage_core::{
    ApprovalActor, ApprovalCursor, ApprovalDecision, ApprovalPage, ApprovalPageRequest,
    ApprovalStore, EventDraft, ResolveApprovalCommit,
};
use storage_sqlite::SqliteApprovalStore;

const MAX_RESOLVE_BODY_BYTES: usize = 8 * 1024;

#[derive(Clone)]
struct ApprovalApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
    runtime_id: String,
}

impl FromRef<ApiState> for ApprovalApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
            runtime_id: state.runtime_id.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveBody {
    decision: ResolveDecision,
    confirmation_digest: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalListQuery {
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct ApprovalPageResponse {
    items: Vec<storage_core::ApprovalRecord>,
    next_cursor: Option<String>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ResolveDecision {
    Approve,
    Deny,
}

impl From<ResolveDecision> for ApprovalDecision {
    fn from(value: ResolveDecision) -> Self {
        match value {
            ResolveDecision::Approve => Self::Approve,
            ResolveDecision::Deny => Self::Deny,
        }
    }
}

pub(super) fn routes() -> Router<ApiState> {
    routes_with_state::<ApiState>()
}

fn routes_with_state<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    ApprovalApiState: FromRef<S>,
{
    Router::new()
        .route("/v1/approvals", get(list_approvals))
        .route("/v1/approvals/{approval_id}", get(get_approval))
        .route(
            "/v1/approvals/{approval_id}/resolve",
            post(resolve_approval),
        )
        .layer(DefaultBodyLimit::max(MAX_RESOLVE_BODY_BYTES))
        .route_layer(middleware::from_fn(authenticate))
        .route_layer(middleware::from_fn(no_store_middleware))
}

async fn no_store_middleware(request: Request<Body>, next: Next) -> Response {
    no_store(next.run(request).await)
}

async fn get_approval(
    State(state): State<ApprovalApiState>,
    headers: HeaderMap,
    Path(approval_id): Path<String>,
) -> Response {
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    if !valid_approval_id(&approval_id) {
        return no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Approval identifier is invalid",
        ));
    }
    let owner_id = state.principal_id.clone();
    let workspace = workspace_id.clone();
    let approval = approval_id.clone();
    let store = state.store;
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &owner_id, &workspace)?;
        SqliteApprovalStore::new(store)
            .get_approval(&owner_id, &workspace, &approval)
            .map_err(resolve_store_error)
    })
    .await;
    match result {
        Ok(Ok(Some(approval))) => no_store(Json(approval).into_response()),
        Ok(Ok(None)) => no_store(operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Approval is unavailable",
        )),
        Ok(Err(response)) => no_store(response),
        Err(_) => no_store(operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "Approval details are unavailable",
        )),
    }
}

async fn list_approvals(
    State(state): State<ApprovalApiState>,
    headers: HeaderMap,
    query: Result<Query<ApprovalListQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<ApprovalPageResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let Query(query) = query.map_err(|_| invalid_approval_query())?;
    let status = match query.status.as_deref() {
        None => None,
        Some(value)
            if matches!(
                value,
                "PENDING" | "APPROVED" | "DENIED" | "EXPIRED" | "CANCELLED"
            ) =>
        {
            Some(value.to_owned())
        }
        Some(_) => return Err(invalid_approval_query()),
    };
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit)
        || query
            .cursor
            .as_deref()
            .is_some_and(|cursor| cursor.is_empty() || cursor.len() > 4096 || !cursor.is_ascii())
    {
        return Err(invalid_approval_query());
    }
    let cursor = query
        .cursor
        .as_deref()
        .map(|value| decode_approval_cursor(value, &workspace_id, status.as_deref(), limit))
        .transpose()?;
    let owner = state.principal_id.clone();
    let selected = workspace_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&state.store, &owner, &selected)?;
        SqliteApprovalStore::new(state.store)
            .list_approvals_page(
                &owner,
                ApprovalPageRequest {
                    workspace_id: selected,
                    status,
                    limit,
                    cursor,
                },
            )
            .map_err(resolve_store_error)
    })
    .await
    .map_err(|_| approval_list_unavailable())??;
    Ok(Json(approval_page_response(result)?))
}

fn approval_page_response(page: ApprovalPage) -> Result<ApprovalPageResponse, Response> {
    let next_cursor = page
        .next_cursor
        .map(|cursor| {
            serde_json::to_vec(&cursor)
                .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
                .map_err(|_| approval_list_unavailable())
        })
        .transpose()?;
    Ok(ApprovalPageResponse {
        items: page.items,
        next_cursor,
    })
}

fn decode_approval_cursor(
    encoded: &str,
    workspace_id: &str,
    status: Option<&str>,
    limit: usize,
) -> Result<ApprovalCursor, Response> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|_| invalid_approval_query())?;
    let cursor: ApprovalCursor =
        serde_json::from_slice(&bytes).map_err(|_| invalid_approval_query())?;
    if cursor.workspace_id != workspace_id
        || cursor.status.as_deref() != status
        || cursor.limit != limit
        || !valid_approval_id(&cursor.after_approval_id)
        || time::OffsetDateTime::parse(
            &cursor.after_requested_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return Err(invalid_approval_query());
    }
    Ok(cursor)
}

fn invalid_approval_query() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Approval query or cursor is invalid",
    )
}

fn approval_list_unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Approvals are unavailable",
    )
}

async fn resolve_approval(
    State(state): State<ApprovalApiState>,
    headers: HeaderMap,
    Path(approval_id): Path<String>,
    body: Bytes,
) -> Response {
    if body.len() > MAX_RESOLVE_BODY_BYTES {
        return no_store(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Approval decision exceeds its size limit",
        ));
    }
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    if !valid_approval_id(&approval_id) {
        return no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Approval identifier is invalid",
        ));
    }
    let request_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    let expected_version = match parse_if_match(&headers) {
        Ok(value) if value > 0 => value,
        Ok(_) => {
            return no_store(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "If-Match must contain a positive Approval version",
            ));
        }
        Err(response) => return no_store(response),
    };
    let body: ResolveBody = match serde_json::from_slice(&body) {
        Ok(body) => body,
        Err(_) => {
            return no_store(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Approval decision body is invalid",
            ));
        }
    };
    if body.confirmation_digest.as_deref().is_some_and(|value| {
        value.len() != 71
            || !value.starts_with("sha256:")
            || !value[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    }) {
        return no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Approval confirmation digest is invalid",
        ));
    }
    if matches!(body.decision, ResolveDecision::Approve) && body.confirmation_digest.is_none() {
        return no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Approving an action requires its exact reviewed action digest",
        ));
    }

    let correlation_id = idempotent_id("cor", &state.principal_id, &request_id, "approval-resolve");
    let event_context = match event_context(&state.runtime_id, &correlation_id) {
        Ok(context) => context,
        Err(()) => {
            return no_store(operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Approval event context is unavailable",
            ));
        }
    };
    let decision = ApprovalDecision::from(body.decision);
    let command = ResolveApprovalCommit {
        workspace_id: workspace_id.clone(),
        actor: ApprovalActor {
            principal_id: state.principal_id.clone(),
            principal_kind: "USER".to_owned(),
            owner_principal_id: state.principal_id.clone(),
            // The route is mounted only on the authenticated local IPC transport.
            // This comes from the transport boundary; request JSON cannot select it.
            assurance_level: "LOCAL_STRONG".to_owned(),
        },
        request_id,
        approval_id: approval_id.clone(),
        expected_version,
        decision,
        confirmation_digest: body.confirmation_digest,
        event: EventDraft {
            event_id: event_context.event_id,
            workspace_id,
            entity_type: "approval".to_owned(),
            entity_id: approval_id,
            origin_runtime_id: event_context.origin_runtime_id,
            entity_revision: expected_version.saturating_add(1),
            hlc_timestamp: event_context.hlc_timestamp,
            correlation_id: event_context.correlation_id,
            causation_id: event_context.causation_id,
            schema_version: 1,
            event_type: "approval.resolved.v1".to_owned(),
            payload: json!({}),
            recorded_at: event_context.recorded_at,
        },
    };
    let owner = state.principal_id.clone();
    let store = state.store;
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &owner, &command.workspace_id)?;
        SqliteApprovalStore::new(store)
            .resolve_approval(command)
            .map_err(resolve_store_error)
    })
    .await;
    let committed = match result {
        Ok(Ok(committed)) => committed,
        Ok(Err(response)) => return no_store(response),
        Err(_) => {
            return no_store(operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "Approval decision could not be committed",
            ));
        }
    };
    let response = Json(committed.approval).into_response();
    no_store(response)
}

fn valid_approval_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn resolve_store_error(error: StoreError) -> Response {
    match error {
        StoreError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Approval is unavailable",
        ),
        StoreError::Conflict { .. } => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Approval version is stale or already resolved",
        ),
        StoreError::IdempotencyConflict => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Idempotency-Key was reused for a different Approval decision",
        ),
        StoreError::Invalid(_) => operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ARGUMENT",
            "Approval decision does not satisfy the current Trust requirements",
        ),
        StoreError::Integrity(_) => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "Approval decision could not be verified",
        ),
        _ => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "Approval decision storage is unavailable",
        ),
    }
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
pub(super) fn test_router(store: SqliteWorkspaceStore, principal_id: String) -> Router {
    routes_with_state::<ApprovalApiState>().with_state(ApprovalApiState {
        store,
        principal_id,
        runtime_id: "approval-test-runtime".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::Connection;
    use std::{sync::Arc, time::Duration};
    use storage_core::{BlobPurpose, StoreError};
    use storage_sqlite::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
    use zeroize::Zeroizing;

    const OWNER: &str = "approval-route-owner";
    const FOREIGN_OWNER: &str = "approval-route-foreign-owner";
    const WORKSPACE: &str = "approval-route-workspace";
    const FOREIGN_WORKSPACE: &str = "approval-route-foreign-workspace";
    const APPROVAL: &str = "approval-route-approval";
    const AT: &str = "2026-10-17T10:00:00Z";
    const ACTION_DIGEST: &str =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([47; 32]),
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

    struct Fixture {
        _directory: tempfile::TempDir,
        store: SqliteWorkspaceStore,
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().expect("temporary Approval route directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("restrict test state directory to its owner");
        }
        let database = directory.path().join("state.sqlite3");
        let store = SqliteWorkspaceStore::open(
            &database,
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("real SQLite store opens");
        for (workspace_id, owner, suffix) in [
            (WORKSPACE, OWNER, "owner"),
            (FOREIGN_WORKSPACE, FOREIGN_OWNER, "foreign"),
        ] {
            WorkspaceService::new(store.clone())
                .create(CreateWorkspace {
                    workspace_id: workspace_id.to_owned(),
                    name: "Approval route fixture".to_owned(),
                    owner_principal_id: owner.to_owned(),
                    event: EventContext {
                        event_id: format!("approval-ws-{suffix}"),
                        origin_runtime_id: "approval-route-runtime".to_owned(),
                        hlc_timestamp: AT.to_owned(),
                        correlation_id: format!("approval-correlation-{suffix}"),
                        causation_id: None,
                        recorded_at: AT.to_owned(),
                    },
                })
                .expect("Workspace commits");
        }
        let connection = Connection::open(&database).expect("open fixture SQLite database");
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .expect("disable fixture foreign keys for the minimal Task seed");
        connection
            .execute(
                "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version) VALUES ('approval-route-task',?1,1,'WAITING_USER','test-agent-binding','NORMAL','{\"principal_id\":\"approval-route-owner\",\"kind\":\"USER\"}',?2,?2,1)",
                rusqlite::params![WORKSPACE, AT],
            )
            .expect("seed Task");
        connection
            .execute(
                "INSERT INTO approvals(approval_id,task_id,kind,action_summary,target_ref_json,scope_digest,action_digest,risk,required_assurance,status,requested_at,version) VALUES (?1,'approval-route-task','CAPABILITY_ESCALATION','Send this exact message','{\"capability_ref\":{\"capability_id\":\"mail.send\",\"identity_kind\":\"PACKAGE_COMPONENT\",\"source\":\"test:litespm\",\"package_version\":\"1.0.0\",\"digest\":\"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"target_digest\":\"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\",\"execution_method\":\"STRUCTURED_API\",\"effect_id\":null}','sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',?2,'HIGH_IMPACT','LOCAL_STRONG','PENDING',?3,1)",
                rusqlite::params![APPROVAL, ACTION_DIGEST, AT],
            )
            .expect("seed Approval");
        connection
            .execute(
                "INSERT INTO approvals(approval_id,task_id,kind,action_summary,target_ref_json,scope_digest,action_digest,risk,required_assurance,status,requested_at,version) VALUES ('approval-route-approval-2','approval-route-task','CAPABILITY_ESCALATION','Send another exact message','{\"capability_ref\":{\"capability_id\":\"mail.send\",\"identity_kind\":\"PACKAGE_COMPONENT\",\"source\":\"test:litespm\",\"package_version\":\"1.0.0\",\"digest\":\"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"target_digest\":\"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\",\"execution_method\":\"STRUCTURED_API\",\"effect_id\":null}','sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd','HIGH_IMPACT','LOCAL_STRONG','PENDING','2026-10-17T10:00:00Z',1)",
                [],
            )
            .expect("seed second Approval for pagination");
        Fixture {
            _directory: directory,
            store,
        }
    }

    fn request(
        method: &str,
        uri: &str,
        workspace_id: &str,
        request_id: Option<&str>,
        if_match: Option<&str>,
        body: &str,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("x-workspace-id", workspace_id);
        if let Some(request_id) = request_id {
            builder = builder.header("idempotency-key", request_id);
        }
        if let Some(version) = if_match {
            builder = builder.header("if-match", version);
        }
        let mut request = builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .expect("valid test request");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        request
    }

    async fn json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("bounded response body");
        serde_json::from_slice(&bytes).expect("JSON response")
    }

    #[tokio::test]
    async fn detail_is_owner_scoped_and_responses_are_no_store() {
        let fixture = fixture();
        let owner = test_router(fixture.store.clone(), OWNER.to_owned());
        let response = owner
            .oneshot(request(
                "GET",
                &format!("/v1/approvals/{APPROVAL}"),
                WORKSPACE,
                None,
                None,
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(json(response).await["approval_id"], APPROVAL);

        let foreign = test_router(fixture.store.clone(), FOREIGN_OWNER.to_owned());
        let response = foreign
            .oneshot(request(
                "GET",
                &format!("/v1/approvals/{APPROVAL}"),
                WORKSPACE,
                None,
                None,
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn list_is_owner_scoped_filters_status_and_paginates_stably() {
        let fixture = fixture();
        let owner = test_router(fixture.store.clone(), OWNER.to_owned());
        let first = owner
            .clone()
            .oneshot(request(
                "GET",
                "/v1/approvals?status=PENDING&limit=1",
                WORKSPACE,
                None,
                None,
                "",
            ))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()[header::CACHE_CONTROL], "no-store");
        let first_json = json(first).await;
        assert_eq!(first_json["items"].as_array().unwrap().len(), 1);
        assert_eq!(
            first_json["items"][0]["approval_id"],
            "approval-route-approval-2"
        );
        let cursor = first_json["next_cursor"]
            .as_str()
            .expect("second page cursor");

        let second = owner
            .oneshot(request(
                "GET",
                &format!("/v1/approvals?status=PENDING&limit=1&cursor={cursor}"),
                WORKSPACE,
                None,
                None,
                "",
            ))
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::OK);
        let second_json = json(second).await;
        assert_eq!(second_json["items"].as_array().unwrap().len(), 1);
        assert_eq!(second_json["items"][0]["approval_id"], APPROVAL);
        assert!(second_json["next_cursor"].is_null());

        let foreign = test_router(fixture.store.clone(), FOREIGN_OWNER.to_owned());
        let response = foreign
            .oneshot(request(
                "GET",
                "/v1/approvals?status=PENDING&limit=1",
                WORKSPACE,
                None,
                None,
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn resolution_replays_exactly_and_rejects_changed_payload_and_stale_version() {
        let fixture = fixture();
        let router = test_router(fixture.store.clone(), OWNER.to_owned());
        let missing_confirmation = router
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/approvals/{APPROVAL}/resolve"),
                WORKSPACE,
                Some("approve-without-digest"),
                Some("1"),
                r#"{"decision":"APPROVE"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(missing_confirmation.status(), StatusCode::BAD_REQUEST);
        let unchanged = router
            .clone()
            .oneshot(request(
                "GET",
                &format!("/v1/approvals/{APPROVAL}"),
                WORKSPACE,
                None,
                None,
                "",
            ))
            .await
            .unwrap();
        assert_eq!(json(unchanged).await["status"], "PENDING");

        let body = r#"{"decision":"APPROVE","confirmation_digest":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#;
        let first = router
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/approvals/{APPROVAL}/resolve"),
                WORKSPACE,
                Some("approve-once"),
                Some("1"),
                body,
            ))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()[header::CACHE_CONTROL], "no-store");
        let first_json = json(first).await;
        assert_eq!(first_json["status"], "APPROVED");
        assert_eq!(first_json["version"], 2);

        let replay = router
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/approvals/{APPROVAL}/resolve"),
                WORKSPACE,
                Some("approve-once"),
                Some("1"),
                body,
            ))
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::OK);
        assert_eq!(json(replay).await, first_json);

        let changed = router
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/approvals/{APPROVAL}/resolve"),
                WORKSPACE,
                Some("approve-once"),
                Some("1"),
                r#"{"decision":"DENY"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(changed.status(), StatusCode::CONFLICT);
        assert_eq!(json(changed).await["code"], "CONFLICT");

        let stale = router
            .oneshot(request(
                "POST",
                &format!("/v1/approvals/{APPROVAL}/resolve"),
                WORKSPACE,
                Some("fresh-key"),
                Some("1"),
                r#"{"decision":"DENY"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(stale.status(), StatusCode::CONFLICT);
        assert_eq!(json(stale).await["code"], "CONFLICT");
    }

    #[tokio::test]
    async fn local_assurance_is_not_client_selectable_and_malformed_requests_fail_closed() {
        let fixture = fixture();
        let router = test_router(fixture.store, OWNER.to_owned());
        let client_assurance = router
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/approvals/{APPROVAL}/resolve"),
                WORKSPACE,
                Some("client-assurance"),
                Some("1"),
                r#"{"decision":"APPROVE","assurance_level":"LOCAL_STRONG"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(client_assurance.status(), StatusCode::BAD_REQUEST);

        let malformed = router
            .oneshot(request(
                "POST",
                &format!("/v1/approvals/{APPROVAL}/resolve"),
                WORKSPACE,
                Some("bad-digest"),
                Some("1"),
                r#"{"decision":"APPROVE","confirmation_digest":"not-a-digest"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    }
}
