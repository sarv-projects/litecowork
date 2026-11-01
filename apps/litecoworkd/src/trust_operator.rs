//! Local owner administration for the narrowly supported Task-planning read grant.
//! This route never consumes an Approval, issues SecretLeases, or dispatches work.

#![allow(clippy::result_large_err)]

use super::*;
use storage_core::{
    ApprovalActor, CapabilityGrantRecord, CapabilityGrantRevocationCommit, CapabilityGrantScope,
    CapabilityGrantStatus, CapabilityGrantStore, EventDraft, IssueCapabilityGrantCommit,
};
use storage_sqlite::SqliteApprovalStore;

const MAX_GRANT_BODY_BYTES: usize = 24 * 1024;
const READ_OPERATION: &str = "workspace.resource.read";

#[derive(Clone)]
struct TrustApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
    runtime_id: String,
}

impl FromRef<ApiState> for TrustApiState {
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
struct CreateReadGrantBody {
    task_id: String,
    capability_ref: CapabilityRefBody,
    resource_refs: Vec<GrantResourceRefBody>,
    expires_at: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CapabilityRefBody {
    capability_id: String,
    identity_kind: CapabilityIdentityKindBody,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_version: Option<String>,
    digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    component: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CapabilityIdentityKindBody {
    PackageComponent,
    McpSkill,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantResourceRefBody {
    resource_id: String,
    revision_id: Option<String>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct GrantRevocationResponse {
    capability_grant_id: String,
    status: CapabilityGrantStatus,
    version: u64,
    replayed: bool,
    event_id: String,
}

pub(super) fn routes() -> Router<ApiState> {
    routes_with_state::<ApiState>()
}

fn routes_with_state<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    TrustApiState: FromRef<S>,
{
    Router::new()
        .route("/v1/capability-grants", post(create_read_grant))
        .route(
            "/v1/capability-grants/{grant_id}/revoke",
            post(revoke_grant),
        )
        .layer(DefaultBodyLimit::max(MAX_GRANT_BODY_BYTES))
        .route_layer(middleware::from_fn(authenticate))
        .route_layer(middleware::from_fn(no_store_middleware))
}

async fn no_store_middleware(request: Request<Body>, next: Next) -> Response {
    no_store(next.run(request).await)
}

async fn create_read_grant(
    State(state): State<TrustApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if body.len() > MAX_GRANT_BODY_BYTES {
        return no_store(operator_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INVALID_ARGUMENT",
            "Capability grant request exceeds its size limit",
        ));
    }
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    let request_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    let body: CreateReadGrantBody = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            return no_store(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Capability grant request body is invalid",
            ));
        }
    };
    if !valid_grant_id(&body.task_id)
        || !valid_grant_id(&body.expires_at)
        || body.resource_refs.is_empty()
        || body.resource_refs.len() > 256
        || !valid_capability_ref(&body.capability_ref)
        || body.resource_refs.iter().any(|reference| {
            !valid_grant_id(&reference.resource_id)
                || reference
                    .revision_id
                    .as_deref()
                    .is_some_and(|value| !valid_grant_id(value))
        })
    {
        return no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Only bounded, expiring Task-planning Resource read grants are supported",
        ));
    }
    let grant_id = match new_id("grant") {
        Ok(value) => value,
        Err(()) => return trust_unavailable(),
    };
    let correlation_id = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        "capability-grant-create",
    );
    let context = match event_context(&state.runtime_id, &correlation_id) {
        Ok(value) => value,
        Err(()) => return trust_unavailable(),
    };
    let response_correlation_id = context.correlation_id.clone();
    let recorded_at = context.recorded_at.clone();
    let expiry = match time::OffsetDateTime::parse(
        &body.expires_at,
        &time::format_description::well_known::Rfc3339,
    ) {
        Ok(value) => value,
        Err(_) => return invalid_grant_body(),
    };
    let now = match time::OffsetDateTime::parse(
        &recorded_at,
        &time::format_description::well_known::Rfc3339,
    ) {
        Ok(value) => value,
        Err(_) => return trust_unavailable(),
    };
    if expiry <= now || expiry - now > time::Duration::hours(24) {
        return invalid_grant_body();
    }
    let expires_at = match expiry.format(&time::format_description::well_known::Rfc3339) {
        Ok(value) => value,
        Err(_) => return trust_unavailable(),
    };
    let capability_ref = match serde_json::to_value(body.capability_ref) {
        Ok(value) => value,
        Err(_) => return invalid_grant_body(),
    };
    let resource_refs = body
        .resource_refs
        .into_iter()
        .map(|reference| {
            let mut value = serde_json::json!({
                "workspace_id": workspace_id,
                "resource_id": reference.resource_id,
            });
            if let Some(revision_id) = reference.revision_id {
                value["revision_id"] = serde_json::Value::String(revision_id);
            }
            value
        })
        .collect::<Vec<_>>();
    let actor = ApprovalActor {
        principal_id: state.principal_id.clone(),
        principal_kind: "USER".to_owned(),
        owner_principal_id: state.principal_id.clone(),
        // The handler is mounted only on authenticated local IPC. Body fields cannot
        // select the actor or assurance level.
        assurance_level: "LOCAL_STRONG".to_owned(),
    };
    let command = IssueCapabilityGrantCommit {
        workspace_id: workspace_id.clone(),
        actor,
        request_id,
        approval_id: None,
        grant: CapabilityGrantRecord {
            capability_grant_id: grant_id.clone(),
            scope: CapabilityGrantScope::TaskPlanning {
                task_id: body.task_id,
            },
            capability_ref,
            allowed_operations: vec![READ_OPERATION.to_owned()],
            resource_scope: serde_json::json!({
                "resource_refs": resource_refs,
                "operation_ids": [READ_OPERATION],
                "constraints": {}
            }),
            secret_refs: Vec::new(),
            granted_by: serde_json::json!({
                "principal_id": state.principal_id,
                "kind": "USER"
            }),
            expires_at: Some(expires_at),
            status: CapabilityGrantStatus::Active,
            created_at: recorded_at.clone(),
            version: 1,
        },
        event: EventDraft {
            event_id: context.event_id,
            workspace_id: workspace_id.clone(),
            entity_type: "capability_grant".to_owned(),
            entity_id: grant_id,
            origin_runtime_id: context.origin_runtime_id,
            entity_revision: 1,
            hlc_timestamp: context.hlc_timestamp,
            correlation_id: context.correlation_id,
            causation_id: context.causation_id,
            schema_version: 1,
            event_type: "capability.grant.created.v1".to_owned(),
            payload: serde_json::json!({}),
            recorded_at,
        },
    };
    let owner = state.principal_id.clone();
    let store = state.store;
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &owner, &workspace_id)?;
        SqliteApprovalStore::new(store)
            .issue_capability_grant(command)
            .map_err(grant_store_error)
    })
    .await;
    match result {
        Ok(Ok(committed)) => {
            let mut response = Json(committed.grant).into_response();
            *response.status_mut() = StatusCode::CREATED;
            if let Ok(value) = HeaderValue::from_str(&response_correlation_id) {
                response.headers_mut().insert("x-correlation-id", value);
            }
            no_store(response)
        }
        Ok(Err(response)) => no_store(response),
        Err(_) => trust_unavailable(),
    }
}

async fn revoke_grant(
    State(state): State<TrustApiState>,
    headers: HeaderMap,
    Path(grant_id): Path<String>,
) -> Response {
    let workspace_id = match selected_workspace(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    if !valid_grant_id(&grant_id) {
        return no_store(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Capability grant identifier is invalid",
        ));
    }
    let request_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    let expected_version = match parse_if_match(&headers) {
        Ok(value) => value,
        Err(response) => return no_store(response),
    };
    let correlation_id = idempotent_id(
        "cor",
        &state.principal_id,
        &request_id,
        "capability-grant-revoke",
    );
    let context = match event_context(&state.runtime_id, &correlation_id) {
        Ok(value) => value,
        Err(()) => return trust_unavailable(),
    };
    let response_correlation_id = context.correlation_id.clone();
    let command = CapabilityGrantRevocationCommit {
        workspace_id: workspace_id.clone(),
        actor: ApprovalActor {
            principal_id: state.principal_id.clone(),
            principal_kind: "USER".to_owned(),
            owner_principal_id: state.principal_id.clone(),
            assurance_level: "LOCAL_STRONG".to_owned(),
        },
        request_id,
        capability_grant_id: grant_id.clone(),
        expected_version,
        event: EventDraft {
            event_id: context.event_id,
            workspace_id: workspace_id.clone(),
            entity_type: "capability_grant".to_owned(),
            entity_id: grant_id,
            origin_runtime_id: context.origin_runtime_id,
            entity_revision: expected_version.saturating_add(1),
            hlc_timestamp: context.hlc_timestamp,
            correlation_id: context.correlation_id,
            causation_id: context.causation_id,
            schema_version: 1,
            event_type: "capability.grant.revoked.v1".to_owned(),
            payload: serde_json::json!({}),
            recorded_at: context.recorded_at,
        },
    };
    let owner = state.principal_id.clone();
    let store = state.store;
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&store, &owner, &workspace_id)?;
        SqliteApprovalStore::new(store)
            .revoke_capability_grant(command)
            .map_err(grant_store_error)
    })
    .await;
    match result {
        Ok(Ok(committed)) => {
            let mut response = Json(GrantRevocationResponse {
                capability_grant_id: committed.capability_grant_id,
                status: committed.status,
                version: committed.version,
                replayed: committed.replayed,
                event_id: committed.event.event_id,
            })
            .into_response();
            if let Ok(value) = HeaderValue::from_str(&response_correlation_id) {
                response.headers_mut().insert("x-correlation-id", value);
            }
            no_store(response)
        }
        Ok(Err(response)) => no_store(response),
        Err(_) => trust_unavailable(),
    }
}

fn valid_capability_ref(value: &CapabilityRefBody) -> bool {
    let valid_text = |text: &str, maximum: usize| {
        !text.trim().is_empty() && text.len() <= maximum && !text.chars().any(char::is_control)
    };
    let digest = value.digest.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    });
    if !valid_text(&value.capability_id, 256) || !valid_text(&value.source, 1024) || !digest {
        return false;
    }
    match value.identity_kind {
        CapabilityIdentityKindBody::PackageComponent => {
            value
                .package_version
                .as_deref()
                .is_some_and(|version| valid_text(version, 256))
                && value
                    .component
                    .as_deref()
                    .is_none_or(|component| valid_text(component, 1024))
        }
        CapabilityIdentityKindBody::McpSkill => {
            value.package_version.is_none()
                && value
                    .component
                    .as_deref()
                    .is_some_and(|component| valid_text(component, 2048))
        }
    }
}

fn valid_grant_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn invalid_grant_body() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Capability grant request body is invalid",
    )
}

fn trust_unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Capability grant storage is unavailable",
    )
}

fn grant_store_error(error: StoreError) -> Response {
    match error {
        StoreError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Task or CapabilityGrant is unavailable",
        ),
        StoreError::Conflict { .. } => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "CapabilityGrant version is stale or already revoked",
        ),
        StoreError::IdempotencyConflict => operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "Idempotency-Key was reused for a different CapabilityGrant command",
        ),
        StoreError::Invalid(_) => operator_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ARGUMENT",
            "CapabilityGrant does not satisfy current Trust restrictions",
        ),
        StoreError::Integrity(_) => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "CapabilityGrant could not be verified",
        ),
        _ => trust_unavailable(),
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
    routes_with_state::<TrustApiState>().with_state(TrustApiState {
        store,
        principal_id,
        runtime_id: "trust-test-runtime".to_owned(),
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

    const OWNER: &str = "trust-route-owner";
    const FOREIGN_OWNER: &str = "trust-route-foreign-owner";
    const WORKSPACE: &str = "trust-route-workspace";
    const FOREIGN_WORKSPACE: &str = "trust-route-foreign-workspace";
    const TASK: &str = "trust-route-task";
    const AT: &str = "2026-10-17T10:00:00Z";

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([53; 32]),
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
        let directory = tempfile::tempdir().expect("temporary Trust route directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("restrict route state directory");
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
                    name: "Trust route fixture".to_owned(),
                    owner_principal_id: owner.to_owned(),
                    event: EventContext {
                        event_id: format!("trust-ws-{suffix}"),
                        origin_runtime_id: "trust-route-runtime".to_owned(),
                        hlc_timestamp: AT.to_owned(),
                        correlation_id: format!("trust-correlation-{suffix}"),
                        causation_id: None,
                        recorded_at: AT.to_owned(),
                    },
                })
                .expect("Workspace commits");
        }
        let connection = Connection::open(&database).expect("open fixture database");
        connection
            .pragma_update(None, "foreign_keys", false)
            .expect("use a bounded Task fixture without unrelated parents");
        connection
            .execute(
                "INSERT INTO tasks(task_id,workspace_id,current_spec_revision,status,lead_agent_binding_id,priority,created_by_json,created_at,updated_at,version) VALUES (?1,?2,1,'WAITING_USER','test-agent-binding','NORMAL',?3,?4,?4,1)",
                rusqlite::params![TASK, WORKSPACE, json!({"principal_id":OWNER,"kind":"USER"}).to_string(), AT],
            )
            .expect("seed an active Task in the selected Workspace");
        connection
            .execute(
                "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,provenance_json,created_at,updated_at,version) VALUES ('resource-1',?1,'FILE','{}','Grant test resource','revision-1','PERSONAL','{}',?2,?2,1)",
                rusqlite::params![WORKSPACE, AT],
            )
            .expect("seed the scoped Resource");
        connection
            .execute(
                "INSERT INTO resource_revisions(resource_revision_id,resource_id,observed_at,created_by_json) VALUES ('revision-1','resource-1',?1,'{}')",
                [AT],
            )
            .expect("seed the Resource revision pin");
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
            .header("x-workspace-id", workspace_id)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(request_id) = request_id {
            builder = builder.header("idempotency-key", request_id);
        }
        if let Some(version) = if_match {
            builder = builder.header(header::IF_MATCH, version);
        }
        let mut request = builder
            .body(Body::from(body.to_owned()))
            .expect("request is valid");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        request
    }

    async fn response_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), 32 * 1024)
            .await
            .expect("bounded grant response");
        serde_json::from_slice(&bytes).expect("valid response JSON")
    }

    fn grant_body() -> String {
        let expiry = time::OffsetDateTime::now_utc()
            .checked_add(time::Duration::hours(23))
            .expect("expiry range is valid")
            .format(&time::format_description::well_known::Rfc3339)
            .expect("expiry formats as RFC3339");
        json!({
            "task_id": TASK,
            "capability_ref": {
                "capability_id": "workspace.resource.read",
                "identity_kind": "PACKAGE_COMPONENT",
                "source": "test:litespm",
                "package_version": "1.0.0",
                "digest": format!("sha256:{}", "a".repeat(64))
            },
            "resource_refs": [{"resource_id":"resource-1","revision_id":"revision-1"}],
            "expires_at": expiry
        })
        .to_string()
    }

    #[tokio::test]
    async fn owner_can_issue_and_revoke_only_bounded_task_planning_read_grant() {
        let fixture = fixture();
        let router = test_router(fixture.store.clone(), OWNER.to_owned());
        let created = router
            .clone()
            .oneshot(request(
                "POST",
                "/v1/capability-grants",
                WORKSPACE,
                Some("grant-resource-read"),
                None,
                &grant_body(),
            ))
            .await
            .expect("route responds");
        let created_status = created.status();
        assert_eq!(created.headers()[header::CACHE_CONTROL], "no-store");
        assert!(!created.headers()["x-correlation-id"].is_empty());
        let create_correlation = created.headers()["x-correlation-id"].clone();
        let grant = response_json(created).await;
        assert_eq!(created_status, StatusCode::CREATED, "response: {grant:?}");
        assert_eq!(grant["scope"]["kind"], "TASK_PLANNING");
        assert_eq!(grant["allowed_operations"], json!([READ_OPERATION]));
        assert_eq!(grant["secret_refs"], json!([]));
        assert_eq!(grant["status"], "ACTIVE");
        assert_eq!(grant["granted_by"]["principal_id"], OWNER);

        let replay = router
            .clone()
            .oneshot(request(
                "POST",
                "/v1/capability-grants",
                WORKSPACE,
                Some("grant-resource-read"),
                None,
                &grant_body_from_json(&grant),
            ))
            .await
            .expect("exact grant retry responds");
        assert_eq!(replay.status(), StatusCode::CREATED);
        assert_eq!(replay.headers()["x-correlation-id"], create_correlation);
        let replay = response_json(replay).await;
        assert_eq!(replay["capability_grant_id"], grant["capability_grant_id"]);

        let mut changed_intent: Value =
            serde_json::from_str(&grant_body_from_json(&grant)).expect("valid grant intent");
        changed_intent["resource_refs"][0]["resource_id"] = json!("different-resource");
        let changed_replay = router
            .clone()
            .oneshot(request(
                "POST",
                "/v1/capability-grants",
                WORKSPACE,
                Some("grant-resource-read"),
                None,
                &changed_intent.to_string(),
            ))
            .await
            .expect("changed payload responds");
        assert_eq!(changed_replay.status(), StatusCode::CONFLICT);
        assert_eq!(changed_replay.headers()[header::CACHE_CONTROL], "no-store");

        let revoke = router
            .clone()
            .oneshot(request(
                "POST",
                &format!(
                    "/v1/capability-grants/{}/revoke",
                    grant["capability_grant_id"].as_str().unwrap()
                ),
                WORKSPACE,
                Some("revoke-resource-read"),
                Some("1"),
                "",
            ))
            .await
            .expect("revocation route responds");
        assert_eq!(revoke.status(), StatusCode::OK);
        assert_eq!(revoke.headers()[header::CACHE_CONTROL], "no-store");
        assert!(!revoke.headers()["x-correlation-id"].is_empty());
        let revoke_correlation = revoke.headers()["x-correlation-id"].clone();
        let revoked = response_json(revoke).await;
        assert_eq!(revoked["status"], "REVOKED");
        assert_eq!(revoked["version"], 2);

        let revoke_replay = router
            .clone()
            .oneshot(request(
                "POST",
                &format!(
                    "/v1/capability-grants/{}/revoke",
                    grant["capability_grant_id"].as_str().unwrap()
                ),
                WORKSPACE,
                Some("revoke-resource-read"),
                Some("1"),
                "",
            ))
            .await
            .expect("exact revocation retry responds");
        assert_eq!(revoke_replay.status(), StatusCode::OK);
        assert_eq!(
            revoke_replay.headers()["x-correlation-id"],
            revoke_correlation
        );
        let revoke_replay = response_json(revoke_replay).await;
        assert_eq!(revoke_replay["replayed"], true);
        assert_eq!(revoke_replay["event_id"], revoked["event_id"]);

        let stale_revoke = router
            .oneshot(request(
                "POST",
                &format!(
                    "/v1/capability-grants/{}/revoke",
                    grant["capability_grant_id"].as_str().unwrap()
                ),
                WORKSPACE,
                Some("revoke-with-stale-version"),
                Some("1"),
                "",
            ))
            .await
            .expect("stale version responds");
        assert_eq!(stale_revoke.status(), StatusCode::CONFLICT);
        assert_eq!(stale_revoke.headers()[header::CACHE_CONTROL], "no-store");

        let connection = Connection::open(fixture._directory.path().join("state.sqlite3"))
            .expect("inspect Trust state");
        let rows: (i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM capability_grants WHERE status='REVOKED'), (SELECT COUNT(*) FROM audit_records WHERE action IN ('CAPABILITY_GRANT_ISSUED','CAPABILITY_GRANT_REVOKED')), (SELECT COUNT(*) FROM domain_events WHERE entity_id=?1)",
                [grant["capability_grant_id"].as_str().unwrap()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read committed grant and audit");
        assert_eq!(rows, (1, 2, 2));
    }

    fn grant_body_from_json(grant: &Value) -> String {
        json!({
            "task_id": TASK,
            "capability_ref": grant["capability_ref"],
            "resource_refs": [{"resource_id":"resource-1","revision_id":"revision-1"}],
            "expires_at": grant["expires_at"]
        })
        .to_string()
    }

    #[tokio::test]
    async fn grant_route_rejects_approval_attempt_scope_secret_and_unbounded_authority() {
        let fixture = fixture();
        let router = test_router(fixture.store.clone(), OWNER.to_owned());
        let base: Value = serde_json::from_str(&grant_body()).unwrap();
        for (request_id, mutate) in [
            ("grant-with-approval", 0),
            ("grant-with-secret", 1),
            ("grant-unbounded", 2),
            ("grant-mutating-operation", 3),
            ("grant-attempt-scope", 4),
        ] {
            let mut body = base.clone();
            match mutate {
                0 => body["approval_id"] = json!("approval-1"),
                1 => body["secret_refs"] = json!([{"secret_ref_id":"secret-1"}]),
                2 => {
                    body["expires_at"] = json!(
                        time::OffsetDateTime::now_utc()
                            .checked_add(time::Duration::hours(25))
                            .unwrap()
                            .format(&time::format_description::well_known::Rfc3339)
                            .unwrap()
                    )
                }
                3 => body["operation"] = json!("external.message.send"),
                _ => body["attempt_id"] = json!("attempt-1"),
            }
            let response = router
                .clone()
                .oneshot(request(
                    "POST",
                    "/v1/capability-grants",
                    WORKSPACE,
                    Some(request_id),
                    None,
                    &body.to_string(),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "case {mutate}");
        }
        let no_grants: i64 = Connection::open(fixture._directory.path().join("state.sqlite3"))
            .unwrap()
            .query_row("SELECT COUNT(*) FROM capability_grants", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(no_grants, 0);
    }

    #[tokio::test]
    async fn grant_route_is_workspace_owner_scoped_and_revoke_requires_if_match() {
        let fixture = fixture();
        let foreign = test_router(fixture.store.clone(), FOREIGN_OWNER.to_owned());
        let denied = foreign
            .oneshot(request(
                "POST",
                "/v1/capability-grants",
                WORKSPACE,
                Some("foreign-grant"),
                None,
                &grant_body(),
            ))
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        assert_eq!(denied.headers()[header::CACHE_CONTROL], "no-store");

        let owner = test_router(fixture.store, OWNER.to_owned());
        let missing_version = owner
            .oneshot(request(
                "POST",
                "/v1/capability-grants/missing/revoke",
                WORKSPACE,
                Some("revoke-missing-version"),
                None,
                "",
            ))
            .await
            .unwrap();
        assert_eq!(missing_version.status(), StatusCode::BAD_REQUEST);
    }
}
