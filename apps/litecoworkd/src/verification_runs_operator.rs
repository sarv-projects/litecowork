//! Read-only, owner-scoped VerificationRun projection.
//!
//! This route lists immutable runs already committed by the verifier settlement
//! boundary. It never starts, executes, settles, or repairs a run, and it emits no
//! events or receipts: the response is a bounded projection of existing state.

use super::*;
use axum::extract::FromRef;
use serde::{Deserialize, Serialize};
use storage_core::VerificationRunStore;

#[derive(Clone)]
pub(super) struct VerificationRunsApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
}

impl VerificationRunsApiState {
    #[cfg(test)]
    pub(super) fn new(store: SqliteWorkspaceStore, principal_id: String) -> Self {
        Self {
            store,
            principal_id,
        }
    }
}

impl FromRef<ApiState> for VerificationRunsApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
        }
    }
}

pub(super) fn routes() -> Router<VerificationRunsApiState> {
    Router::new().route(
        "/v1/tasks/{task_id}/verification-runs",
        get(list_task_verification_runs),
    )
}

#[cfg(test)]
pub(super) fn test_router(state: VerificationRunsApiState) -> Router {
    routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(state)
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationRunsQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VerificationRunCursor {
    version: u32,
    workspace_id: String,
    task_id: String,
    limit: usize,
    after_started_at: String,
    after_verification_run_id: String,
}

/// Exactly the fields declared by the OpenAPI `VerificationRun` schema. Internal
/// aggregate version and the typed failure code stay out of the shared projection, so
/// this route cannot leak new private fields added to the domain record later.
///
/// The object is built explicitly rather than serializing the domain record so a
/// future private field cannot silently join the Operator surface.
macro_rules! verification_run_projection {
    ($run:expr) => {
        json!({
            "verification_run_id": $run.verification_run_id,
            "task_id": $run.task_id,
            "criterion_id": $run.criterion_id,
            "task_spec_revision": $run.task_spec_revision,
            "criterion_digest": $run.criterion_digest,
            "verifier_kind": $run.verifier_kind,
            "verifier_version": $run.verifier_version,
            "subject_refs": $run.subject_refs,
            "inputs": $run.inputs,
            "status": $run.status,
            "evidence_refs": $run.evidence_refs,
            "started_at": $run.started_at,
            "completed_at": $run.completed_at,
        })
    };
}

async fn list_task_verification_runs(
    State(state): State<VerificationRunsApiState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<VerificationRunsQuery>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner_for(&state.store, &state.principal_id, &workspace_id)?;
    if !valid_identifier(&task_id, 256) {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Task identity is invalid",
        ));
    }
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "VerificationRun page limit must be between 1 and 200",
        ));
    }
    let cursor = query
        .cursor
        .as_deref()
        .map(|encoded| decode_cursor(encoded, &workspace_id, &task_id, limit))
        .transpose()?;
    let runs = state
        .store
        .list_verification_runs(
            &state.principal_id,
            &workspace_id,
            &task_id,
            cursor
                .as_ref()
                .map(|cursor| cursor.after_started_at.as_str()),
            cursor
                .as_ref()
                .map(|cursor| cursor.after_verification_run_id.as_str()),
            limit + 1,
        )
        .map_err(verification_run_read_error)?;
    let has_more = runs.len() > limit;
    let runs = runs.into_iter().take(limit).collect::<Vec<_>>();
    let next_cursor = if has_more {
        let last = runs.last().ok_or_else(|| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "VerificationRun page boundary is unavailable",
            )
        })?;
        let after_started_at = last.started_at.clone().ok_or_else(|| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "VerificationRun page boundary is unavailable",
            )
        })?;
        Some(encode_cursor(&VerificationRunCursor {
            version: 1,
            workspace_id,
            task_id,
            limit,
            after_started_at,
            after_verification_run_id: last.verification_run_id.clone(),
        })?)
    } else {
        None
    };
    Ok(no_store(
        Json(json!({
            "items": runs
                .into_iter()
                .map(|run| verification_run_projection!(run))
                .collect::<Vec<_>>(),
            "next_cursor": next_cursor,
        }))
        .into_response(),
    ))
}

fn decode_cursor(
    encoded: &str,
    workspace_id: &str,
    task_id: &str,
    limit: usize,
) -> Result<VerificationRunCursor, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid_cursor());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| invalid_cursor())?;
    let cursor: VerificationRunCursor =
        serde_json::from_slice(&bytes).map_err(|_| invalid_cursor())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || cursor.task_id != task_id
        || cursor.limit != limit
        || !valid_identifier(&cursor.after_started_at, 64)
        || !valid_identifier(&cursor.after_verification_run_id, 256)
    {
        return Err(invalid_cursor());
    }
    Ok(cursor)
}

fn encode_cursor(cursor: &VerificationRunCursor) -> Result<String, Response> {
    let encoded = serde_json::to_vec(cursor)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "VerificationRun cursor could not be created",
            )
        })?;
    if encoded.len() > 2048 {
        return Err(operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "VerificationRun cursor exceeded its size limit",
        ));
    }
    Ok(encoded)
}

fn verification_run_read_error(error: StoreError) -> Response {
    match error {
        // A Task absent from the selected owned Workspace is indistinguishable from
        // an absent Task, including when it belongs to another Workspace.
        StoreError::NotFound => {
            operator_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Task is unavailable")
        }
        StoreError::Invalid(_) => operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "VerificationRun query is invalid",
        ),
        _ => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "VerificationRun history is unavailable",
        ),
    }
}

fn valid_identifier(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn invalid_cursor() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "VerificationRun cursor is invalid or does not match this query",
    )
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::AuthenticatedLocalPeer;
    use super::{VerificationRunsApiState, test_router};
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::{Connection, params};
    use serde_json::Value;
    use std::{collections::BTreeSet, fs, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
    use storage_core::{BlobPurpose, StoreError};
    use storage_sqlite::{
        FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey,
        WorkspaceBlobKeyProvider,
    };
    use tower::ServiceExt;
    use zeroize::Zeroizing;

    const WORKSPACE: &str = "verification-runs-workspace";
    const OTHER_WORKSPACE: &str = "verification-runs-other-workspace";
    const FOREIGN_WORKSPACE: &str = "verification-runs-foreign-workspace";
    const OWNER: &str = "verification-runs-owner";
    const FOREIGN_OWNER: &str = "verification-runs-foreign-owner";
    const NOW: &str = "2026-10-20T10:00:00Z";

    #[derive(Clone)]
    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([41_u8; 32]),
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
        let directory = tempfile::tempdir().expect("temporary VerificationRun route directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private route fixture directory");
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
        for (workspace_id, name, owner, suffix) in [
            (WORKSPACE, "Selected Workspace", OWNER, "selected"),
            (OTHER_WORKSPACE, "Other Workspace", OWNER, "other"),
            (
                FOREIGN_WORKSPACE,
                "Foreign Workspace",
                FOREIGN_OWNER,
                "foreign",
            ),
        ] {
            WorkspaceService::new(store.clone())
                .create(CreateWorkspace {
                    workspace_id: workspace_id.to_owned(),
                    name: name.to_owned(),
                    owner_principal_id: owner.to_owned(),
                    event: EventContext {
                        event_id: format!("verification-runs-workspace-{suffix}"),
                        origin_runtime_id: "verification-runs-runtime".to_owned(),
                        hlc_timestamp: NOW.to_owned(),
                        correlation_id: format!("verification-runs-workspace-{suffix}"),
                        causation_id: None,
                        recorded_at: NOW.to_owned(),
                    },
                })
                .expect("Workspace commits");
        }
        let connection = Connection::open(&database).expect("fixture SQLite connection");
        // The minimal Task seed is not produced by TaskService. Disable fixture-only
        // foreign keys rather than fabricating an unrelated AgentBinding graph; the
        // route itself still reads through the migrated production schema.
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .expect("disable fixture foreign keys for the minimal Task seed");
        for (task_id, workspace_id) in [
            ("verification-task", WORKSPACE),
            ("other-workspace-task", OTHER_WORKSPACE),
            ("foreign-workspace-task", FOREIGN_WORKSPACE),
        ] {
            connection
                .execute(
                    "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version) VALUES (?1,?2,1,'VERIFYING','fixture-agent-binding','NORMAL','{\"principal_id\":\"verification-runs-owner\",\"kind\":\"USER\"}',?3,?3,1)",
                    params![task_id, workspace_id, NOW],
                )
                .expect("minimal Task fixture");
        }
        // VerificationRuns are immutable lifecycle records: seed a legal PENDING row,
        // start it, and close the one run that carries a typed failure code. This is
        // fixture sequencing of the real v20 triggers, not a bypass of them.
        for (run_id, task_id, started_at) in [
            (
                "verification-run-a",
                "verification-task",
                "2026-10-20T10:00:04Z",
            ),
            (
                "verification-run-b",
                "verification-task",
                "2026-10-20T10:00:03Z",
            ),
            (
                "verification-run-c",
                "verification-task",
                "2026-10-20T10:00:02Z",
            ),
            (
                "verification-run-d",
                "verification-task",
                "2026-10-20T10:00:02Z",
            ),
            (
                "verification-run-other",
                "other-workspace-task",
                "2026-10-20T10:00:01Z",
            ),
            (
                "verification-run-foreign",
                "foreign-workspace-task",
                "2026-10-20T10:00:01Z",
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO verification_runs(verification_run_id,task_id,criterion_id,task_spec_revision,criterion_digest,verifier_kind,verifier_version,subject_refs_json,inputs_json,status,evidence_refs_json,started_at,completed_at,version,failure_code) VALUES (?1,?2,'criterion-1',1,?3,'fixture-verifier','1.0.0','[]','[]','PENDING','[]',NULL,NULL,1,NULL)",
                    params![run_id, task_id, format!("sha256:{}", "a".repeat(64))],
                )
                .expect("pending VerificationRun fixture");
            connection
                .execute(
                    "UPDATE verification_runs SET status='RUNNING',started_at=?1,version=2 WHERE verification_run_id=?2",
                    params![started_at, run_id],
                )
                .expect("started VerificationRun fixture");
        }
        connection
            .execute(
                "UPDATE verification_runs SET status='INCONCLUSIVE',completed_at='2026-10-20T10:00:05Z',version=3,failure_code='TIMEOUT' WHERE verification_run_id='verification-run-b'",
                [],
            )
            .expect("closed VerificationRun fixture");
        Fixture {
            _directory: directory,
            store,
        }
    }

    fn request(uri: &str, workspace_id: &str) -> Request<Body> {
        let mut request = Request::get(uri)
            .header("x-workspace-id", workspace_id)
            .body(Body::empty())
            .expect("valid VerificationRun request");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        request
    }

    async fn page(router: &axum::Router, uri: &str, workspace_id: &str) -> (StatusCode, Value) {
        let response = router
            .clone()
            .oneshot(request(uri, workspace_id))
            .await
            .expect("route responds");
        let status = response.status();
        if status == StatusCode::OK {
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "no-store",
                "VerificationRun reads must never be cached"
            );
        }
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("bounded response body");
        let value: Value = serde_json::from_slice(&body).expect("valid JSON response");
        (status, value)
    }

    #[tokio::test]
    async fn archived_workspace_keeps_its_verification_runs_readable() {
        // docs/API.md: "Archived Workspaces remain readable". Before the shared
        // Workspace access policy, this read applied the mutation-only ACTIVE
        // filter and answered NOT_FOUND for a Workspace that was merely archived.
        let Fixture { _directory, store } = fixture();
        Connection::open(_directory.path().join("state.sqlite3"))
            .expect("archive connection")
            .execute(
                "UPDATE workspaces SET status='ARCHIVED', version=version+1 WHERE workspace_id=?1",
                params![WORKSPACE],
            )
            .expect("archive the Workspace");
        let router = test_router(VerificationRunsApiState::new(store, OWNER.to_owned()));
        let (status, body) = page(
            &router,
            "/v1/tasks/verification-task/verification-runs?limit=2",
            WORKSPACE,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "an archived Workspace must keep its VerificationRun reads"
        );
        assert_eq!(body["items"][0]["verification_run_id"], "verification-run-a");
    }

    #[tokio::test]
    async fn verification_runs_route_is_owner_scoped_paginated_and_closed() {
        let fixture = fixture();
        let router = test_router(VerificationRunsApiState::new(
            fixture.store,
            OWNER.to_owned(),
        ));
        let first_uri = "/v1/tasks/verification-task/verification-runs?limit=2";
        let (status, first) = page(&router, first_uri, WORKSPACE).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            first["items"][0]["verification_run_id"],
            "verification-run-a"
        );
        assert_eq!(
            first["items"][1]["verification_run_id"],
            "verification-run-b"
        );
        // Only OpenAPI-declared fields are projected; internal aggregate version and
        // the typed failure code never leave Core through this read route.
        let projected_fields = first["items"][1]
            .as_object()
            .expect("run projection object")
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            projected_fields,
            [
                "verification_run_id",
                "task_id",
                "criterion_id",
                "task_spec_revision",
                "criterion_digest",
                "verifier_kind",
                "verifier_version",
                "subject_refs",
                "inputs",
                "status",
                "evidence_refs",
                "started_at",
                "completed_at",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
        );
        assert!(first["items"][1].get("failure_code").is_none());
        assert!(first["items"][1].get("version").is_none());
        let cursor = first["next_cursor"].as_str().expect("continuation cursor");

        // The same started_at on c and d is ordered by descending run ID, so the
        // keyset page is deterministic even at a timestamp tie.
        let (status, second) = page(
            &router,
            &format!("/v1/tasks/verification-task/verification-runs?limit=2&cursor={cursor}"),
            WORKSPACE,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            second["items"][0]["verification_run_id"],
            "verification-run-d"
        );
        assert_eq!(
            second["items"][1]["verification_run_id"],
            "verification-run-c"
        );
        assert!(second["next_cursor"].is_null());

        // Bounds: changing the page size invalidates an existing cursor.
        let (status, _) = page(
            &router,
            &format!("/v1/tasks/verification-task/verification-runs?limit=1&cursor={cursor}"),
            WORKSPACE,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Bounds: a cursor cannot be replayed against another Task.
        let (status, _) = page(
            &router,
            &format!("/v1/tasks/other-workspace-task/verification-runs?limit=2&cursor={cursor}"),
            WORKSPACE,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Bounds: the declared minimum and maximum page sizes are enforced.
        for limit in ["0", "201"] {
            let (status, _) = page(
                &router,
                &format!("/v1/tasks/verification-task/verification-runs?limit={limit}"),
                WORKSPACE,
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
        // Bounds: a tampered cursor is rejected, not silently treated as page one.
        let (status, _) = page(
            &router,
            "/v1/tasks/verification-task/verification-runs?cursor=not-a-cursor",
            WORKSPACE,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // A Task owned by the same principal but in another Workspace is unseen.
        let (status, _) = page(
            &router,
            "/v1/tasks/other-workspace-task/verification-runs",
            WORKSPACE,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        // An absent Task is identical to an out-of-scope Task.
        let (status, _) = page(
            &router,
            "/v1/tasks/absent-task/verification-runs",
            WORKSPACE,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        // A Workspace owned by another principal is denied before Task lookup.
        let (status, _) = page(
            &router,
            "/v1/tasks/foreign-workspace-task/verification-runs",
            FOREIGN_WORKSPACE,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // The other owned Workspace is readable when it is the selected scope.
        let (status, other) = page(
            &router,
            "/v1/tasks/other-workspace-task/verification-runs",
            OTHER_WORKSPACE,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            other["items"][0]["verification_run_id"],
            "verification-run-other"
        );
    }
}
