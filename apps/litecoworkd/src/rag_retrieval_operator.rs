//! Grounded retrieval and index-state routes for the authenticated local Operator.
//!
//! Core owns Resource scope, revision, provenance, authorization, and deletion. This
//! route authorizes the caller, hands an already authorized scope to the qualified
//! retrieval provider, and returns segments that are bound to the exact immutable
//! Resource revision they were read from. A request with no authorized evidence
//! returns an explicit insufficient-evidence outcome instead of an unreferenced answer.
//!
//! The served provider is the deterministic local lexical index. It is not semantic
//! retrieval. No endpoint, embedding API, or third party wire protocol is defined here.

#![allow(clippy::result_large_err)] // Handlers return full Axum Responses to preserve status, body, and headers.

use super::*;
use std::sync::Arc;

use axum::extract::Path;
use domain_rag::{
    AuthorizedResourceRef, GroundedStatus, InsufficientEvidenceReason, LocalLexicalRetrievalProvider,
    ProviderStatus, ResourceIndexFacts, RetrievalBudget, RetrievalError, RetrievalProvider,
    RetrievalRequest, RetrievalScope, resolve_index_state,
};
use serde::{Deserialize, Serialize};
use storage_core::{ResourceTextIndexSkipReason, ResourceStore};

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/resources/retrieval", post(grounded_retrieval))
        .route(
            "/v1/resources/{resource_id}/index-state",
            get(resource_index_state),
        )
        // Retrieval results and index state are private Workspace state.
        .route_layer(middleware::from_fn(retrieval_no_store))
}

async fn retrieval_no_store(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct GroundedRetrievalRequest {
    query: String,
    /// The authorized Resource scope. Absent means every ACTIVE Resource of the
    /// selected Workspace. An explicit empty scope is refused.
    #[serde(default)]
    scope: Option<Vec<AuthorizedResourceRef>>,
    #[serde(default)]
    max_results: Option<u16>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GroundedRetrievalResponse {
    request_id: String,
    workspace_id: String,
    status: GroundedStatus,
    provider: ProviderStatus,
    citations: Vec<domain_rag::RetrievalCitation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    insufficient_reason: Option<InsufficientEvidenceReason>,
    scanned_candidates: usize,
    truncated: bool,
}

/// A provider that cannot serve the request is a truthful degradation, not a claim of
/// an answer. The response keeps the documented shape and carries no citation.
fn provider_unavailable_response(
    request_id: &str,
    workspace_id: &str,
    provider: ProviderStatus,
    reason_code: String,
    reason: String,
) -> Response {
    let availability = match provider.availability {
        domain_rag::ProviderAvailability::Incompatible => {
            domain_rag::ProviderAvailability::Incompatible
        }
        _ => domain_rag::ProviderAvailability::NotConfigured,
    };
    Json(GroundedRetrievalResponse {
        request_id: request_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        status: GroundedStatus::ProviderUnavailable,
        provider: ProviderStatus {
            availability,
            reason_code: Some(reason_code),
            reason: Some(reason),
            ..provider
        },
        citations: Vec::new(),
        insufficient_reason: None,
        scanned_candidates: 0,
        truncated: false,
    })
    .into_response()
}

async fn grounded_retrieval(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner_for(&state.store, &state.principal_id, &workspace_id)?;
    let request: GroundedRetrievalRequest = serde_json::from_slice(&body).map_err(|_| {
        operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "The grounded retrieval request is invalid",
        )
    })?;
    let query = request.query.trim().to_owned();
    if query.is_empty() || query.len() > domain_rag::MAX_QUERY_BYTES {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "The grounded retrieval query is empty or too long",
        ));
    }
    let scope = match request.scope {
        None => RetrievalScope::Workspace,
        Some(resources) => {
            if resources.is_empty() {
                return Err(operator_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_ARGUMENT",
                    "The grounded retrieval scope authorizes no Resource",
                ));
            }
            RetrievalScope::Resources(resources)
        }
    };
    if scope.validate().is_err() {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "The grounded retrieval scope is invalid",
        ));
    }
    let mut budget = RetrievalBudget::local_defaults();
    if let Some(max_results) = request.max_results {
        budget.max_results = max_results;
    }
    if budget.validate().is_err() {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "The grounded retrieval budget is invalid",
        ));
    }
    let request_id = match request.request_id.as_deref() {
        Some(value) if valid_request_id(value) => value.to_owned(),
        Some(_) => {
            return Err(operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "The grounded retrieval request id is invalid",
            ));
        }
        None => derived_request_id(&workspace_id, &query),
    };

    let provider = LocalLexicalRetrievalProvider::new(Arc::new(state.store.clone()));
    let provider_status = provider.status();
    let page = match provider.search(&RetrievalRequest {
        workspace_id: workspace_id.clone(),
        query: query.clone(),
        scope: scope.clone(),
        budget,
    }) {
        Ok(page) => page,
        Err(error) => {
            return match error {
                RetrievalError::NotConfigured {
                    reason_code,
                    reason,
                }
                | RetrievalError::Incompatible {
                    reason_code,
                    reason,
                } => Ok(provider_unavailable_response(
                    &request_id,
                    &workspace_id,
                    provider_status,
                    reason_code,
                    reason,
                )),
                other => Err(retrieval_error_response(other)),
            };
        }
    };

    let (status, insufficient_reason) = if page.citations.is_empty() {
        let reason = if scope.has_revision_pins() {
            InsufficientEvidenceReason::PinnedRevisionNotIndexed
        } else {
            InsufficientEvidenceReason::NoAuthorizedMatch
        };
        (GroundedStatus::InsufficientEvidence, Some(reason))
    } else {
        (GroundedStatus::Grounded, None)
    };
    Ok(Json(GroundedRetrievalResponse {
        request_id,
        workspace_id,
        status,
        provider: provider_status,
        citations: page.citations,
        insufficient_reason,
        scanned_candidates: page.scanned_candidates,
        truncated: page.truncated,
    })
    .into_response())
}

fn retrieval_error_response(error: RetrievalError) -> Response {
    match error {
        RetrievalError::InvalidRequest { .. } | RetrievalError::BudgetExceeded { .. } => {
            operator_error(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "The grounded retrieval request is invalid",
            )
        }
        RetrievalError::Unavailable { .. } => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "RESOURCE_INDEX_UNAVAILABLE",
            "The local Resource index is unavailable",
        ),
        RetrievalError::NotConfigured { .. } | RetrievalError::Incompatible { .. } => {
            operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "PROVIDER_UNAVAILABLE",
                "No qualified retrieval provider can serve this request",
            )
        }
        RetrievalError::Integrity { .. } => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTEGRITY_FAILURE",
            "The grounded retrieval could not verify its source",
        ),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceIndexStateResponse {
    workspace_id: String,
    resource_id: String,
    state: domain_rag::ResourceIndexState,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<ResourceTextIndexSkipReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_revision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    indexed_revision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    indexed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parser_id: Option<String>,
    superseded_index_count: u32,
}

async fn resource_index_state(
    State(state): State<ApiState>,
    Path(resource_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner_for(&state.store, &state.principal_id, &workspace_id)?;
    if resource_id.trim().is_empty() || resource_id.len() > 256 {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource identifier is invalid",
        ));
    }
    let row = state
        .store
        .resource_index_state(&workspace_id, &resource_id)
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "The Resource index state is unavailable",
            )
        })?
        .ok_or_else(|| {
            operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Resource is unavailable",
            )
        })?;
    let not_indexable_reason = if row.indexed_revision_id.is_none() {
        storage_sqlite::rag_retrieval::not_indexable_reason(
            &row.display_name,
            &row.media_type,
            row.current_size_bytes,
        )
    } else {
        None
    };
    let view = resolve_index_state(
        &resource_id,
        &ResourceIndexFacts {
            current_revision_id: row.current_revision_id.clone(),
            current_content_digest: row.current_content_digest.clone(),
            indexed_revision_id: row.indexed_revision_id.clone(),
            indexed_source_digest: row.indexed_source_digest.clone(),
            parser_id: row.parser_id.clone(),
            indexed_at: row.indexed_at.clone(),
            superseded_index_count: row.superseded_index_count,
            not_indexable_reason,
        },
    );
    Ok(Json(ResourceIndexStateResponse {
        workspace_id,
        resource_id,
        state: view.state,
        reason: view.reason,
        current_revision_id: view.current_revision_id,
        content_digest: row.current_content_digest,
        indexed_revision_id: view.indexed_revision_id,
        indexed_at: view.indexed_at,
        parser_id: view.parser_id,
        superseded_index_count: view.superseded_index_count,
    })
    .into_response())
}

fn valid_request_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

/// Derives a stable request identity for a read-only retrieval request that did not
/// carry one. It is a correlation handle, not an idempotency receipt.
fn derived_request_id(workspace_id: &str, query: &str) -> String {
    let payload = serde_json::json!({
        "operation": "resource.retrieval.v1",
        "workspace_id": workspace_id,
        "query": query,
    });
    let bytes = serde_json::to_vec(&payload).unwrap_or_default();
    format!("retrieval-{}", sha256_digest(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::header::CONTENT_TYPE;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use storage_core::{BlobPurpose, EventDraft};
    use storage_sqlite::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};

    const WORKSPACE: &str = "rag-retrieval-workspace";
    const OWNER: &str = "rag-retrieval-owner";
    const OTHER_OWNER: &str = "rag-retrieval-other-owner";
    const RUNTIME: &str = "rag-retrieval-runtime";
    const NOW: &str = "2026-10-20T12:00:00Z";
    const CONTENT: &[u8] = b"alpha cafe needle beta gamma";
    const OTHER_CONTENT: &[u8] = b"second workspace secret needle";

    struct RetrievalTestKeys;

    impl WorkspaceBlobKeyProvider for RetrievalTestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: zeroize::Zeroizing::new([71_u8; 32]),
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

    fn test_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private test directory");
        SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            Arc::new(FileBlobStore::new(
                directory.path().join("blobs"),
                RetrievalTestKeys,
            )),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: std::time::Duration::from_secs(2),
            },
        )
        .expect("SQLite store opens")
    }

    fn create_workspace(store: &SqliteWorkspaceStore, workspace_id: &str, owner: &str) {
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: workspace_id.to_owned(),
                name: "Grounded retrieval fixture".to_owned(),
                owner_principal_id: owner.to_owned(),
                event: EventContext {
                    event_id: format!("event-{workspace_id}"),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: format!("event-{workspace_id}"),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("Workspace commits");
    }

    fn create_text_resource(
        store: &SqliteWorkspaceStore,
        workspace_id: &str,
        owner: &str,
        resource_id: &str,
        display_name: &str,
        content: &[u8],
    ) {
        let revision_id = format!("revision-{resource_id}");
        let content_digest = sha256_digest(content);
        let resource = ResourceRecord {
            resource_id: resource_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            kind: "FILE".to_owned(),
            provider_identity: json!({}),
            identity_digest: None,
            display_name: display_name.to_owned(),
            current_revision_id: Some(revision_id.clone()),
            sensitivity: "PERSONAL".to_owned(),
            provenance: json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
            created_at: NOW.to_owned(),
            updated_at: NOW.to_owned(),
            version: 1,
        };
        let revision = ResourceRevisionRecord {
            resource_revision_id: revision_id,
            resource_id: resource_id.to_owned(),
            parent_revision_ids: Vec::new(),
            provider_revision: None,
            content_digest: Some(content_digest.clone()),
            size_bytes: Some(content.len() as u64),
            media_type: Some("text/plain".to_owned()),
            observed_at: NOW.to_owned(),
            created_by: json!({"kind": "USER", "principal_id": owner}),
        };
        let event_id = format!("event-{resource_id}");
        store
            .create_resource(
                WorkspaceCreateRequest {
                    principal_id: owner.to_owned(),
                    request_id: format!("request-{resource_id}"),
                    request_payload: json!({
                        "operation": "resource.quick_import.v1",
                        "workspace_id": workspace_id,
                        "display_name": display_name,
                        "media_type": "text/plain",
                        "content_digest": content_digest,
                        "size_bytes": content.len(),
                    }),
                },
                resource,
                revision,
                EventDraft {
                    event_id: event_id.clone(),
                    workspace_id: workspace_id.to_owned(),
                    entity_type: "Resource".to_owned(),
                    entity_id: resource_id.to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    entity_revision: 1,
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: event_id,
                    causation_id: None,
                    schema_version: 1,
                    event_type: "resource.created.v1".to_owned(),
                    payload: json!({
                        "resource_id": resource_id,
                        "workspace_id": workspace_id,
                        "kind": "FILE",
                        "aggregate_version": 1,
                    }),
                    recorded_at: NOW.to_owned(),
                },
                content.to_vec(),
            )
            .expect("Resource commits");
    }

    fn router_for(store: SqliteWorkspaceStore, principal_id: &str) -> Router {
        let (_, stream_shutdown) = tokio::sync::watch::channel(false);
        super::super::build_operator_router(super::super::ApiState {
            store,
            principal_id: principal_id.to_owned(),
            runtime_id: RUNTIME.to_owned(),
            local_incarnation_id: "rag-retrieval-incarnation".to_owned(),
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

    async fn post_retrieval(
        router: &Router,
        workspace_id: Option<&str>,
        body: serde_json::Value,
    ) -> Response {
        let mut request = Request::builder()
            .method(axum::http::Method::POST)
            .uri("/v1/resources/retrieval")
            .header(CONTENT_TYPE, "application/json");
        if let Some(workspace_id) = workspace_id {
            request = request.header("x-workspace-id", workspace_id);
        }
        let mut request = request
            .body(Body::from(body.to_string()))
            .expect("request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("router responds")
    }

    async fn get_index_state(router: &Router, workspace_id: &str, resource_id: &str) -> Response {
        let mut request = Request::builder()
            .method(axum::http::Method::GET)
            .uri(format!("/v1/resources/{resource_id}/index-state"))
            .header("x-workspace-id", workspace_id)
            .body(Body::empty())
            .expect("request builds");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        router
            .clone()
            .oneshot(request)
            .await
            .expect("router responds")
    }

    #[tokio::test]
    async fn grounded_retrieval_returns_exact_revision_bound_citations() {
        let directory = tempfile::tempdir().expect("temporary retrieval directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private test directory");
        let store = test_store(&directory);
        create_workspace(&store, WORKSPACE, OWNER);
        create_text_resource(
            &store,
            WORKSPACE,
            OWNER,
            "resource-needle",
            "needle.txt",
            CONTENT,
        );
        let router = router_for(store, OWNER);

        let response = post_retrieval(
            &router,
            Some(WORKSPACE),
            json!({"query": "needle", "requestId": "request-1"}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let body = to_bytes(response.into_body(), 256 * 1024)
            .await
            .expect("bounded body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(body["status"], "GROUNDED");
        assert_eq!(body["requestId"], "request-1");
        assert_eq!(body["provider"]["providerId"], "litecowork.local-lexical");
        assert_eq!(body["provider"]["retrievalMode"], "LEXICAL_ONLY");
        assert_eq!(body["provider"]["placement"], "LOCAL");
        let citations = body["citations"].as_array().expect("citations array");
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0]["resourceId"], "resource-needle");
        assert_eq!(
            citations[0]["resourceRevisionId"],
            "revision-resource-needle"
        );
        assert_eq!(
            citations[0]["contentDigest"],
            sha256_digest(CONTENT),
            "a citation names the digest of the exact revision it came from"
        );
        assert_eq!(citations[0]["locator"]["kind"], "TEXT_SPAN");
        let span = &citations[0]["locator"]["span"];
        assert!(span["startUtf8Byte"].as_u64().is_some());
        assert!(span["endUtf8ByteExclusive"].as_u64().is_some());
    }

    #[tokio::test]
    async fn grounded_retrieval_reports_insufficient_evidence_instead_of_an_unreferenced_answer() {
        let directory = tempfile::tempdir().expect("temporary retrieval directory");
        let store = test_store(&directory);
        create_workspace(&store, WORKSPACE, OWNER);
        create_text_resource(
            &store,
            WORKSPACE,
            OWNER,
            "resource-needle",
            "needle.txt",
            CONTENT,
        );
        let router = router_for(store, OWNER);

        let response = post_retrieval(
            &router,
            Some(WORKSPACE),
            json!({"query": "absent-term"}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 256 * 1024)
            .await
            .expect("bounded body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(body["status"], "INSUFFICIENT_EVIDENCE");
        assert_eq!(body["insufficientReason"], "NO_AUTHORIZED_MATCH");
        assert!(
            body["citations"].as_array().expect("citations array").is_empty(),
            "an unsupported answer must not carry an unreferenced citation"
        );
    }

    #[tokio::test]
    async fn grounded_retrieval_never_returns_another_workspaces_resource() {
        let directory = tempfile::tempdir().expect("temporary retrieval directory");
        let store = test_store(&directory);
        create_workspace(&store, WORKSPACE, OWNER);
        create_workspace(&store, "workspace-other-rag", OTHER_OWNER);
        create_text_resource(
            &store,
            WORKSPACE,
            OWNER,
            "resource-needle",
            "needle.txt",
            CONTENT,
        );
        create_text_resource(
            &store,
            "workspace-other-rag",
            OTHER_OWNER,
            "resource-secret",
            "secret.txt",
            OTHER_CONTENT,
        );
        let router = router_for(store, OWNER);

        let response = post_retrieval(
            &router,
            Some(WORKSPACE),
            json!({"query": "needle"}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 256 * 1024)
            .await
            .expect("bounded body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        let citations = body["citations"].as_array().expect("citations array");
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0]["resourceId"], "resource-needle");
        assert!(
            !body.to_string().contains("resource-secret"),
            "a cross-Workspace Resource must never appear in the response"
        );
    }

    #[tokio::test]
    async fn grounded_retrieval_refuses_a_non_owner_and_an_empty_scope() {
        let directory = tempfile::tempdir().expect("temporary retrieval directory");
        let store = test_store(&directory);
        create_workspace(&store, WORKSPACE, OWNER);
        create_text_resource(
            &store,
            WORKSPACE,
            OWNER,
            "resource-needle",
            "needle.txt",
            CONTENT,
        );

        let other_router = router_for(store.clone(), OTHER_OWNER);
        let denied = post_retrieval(
            &other_router,
            Some(WORKSPACE),
            json!({"query": "needle"}),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);

        let router = router_for(store, OWNER);
        let refused = post_retrieval(
            &router,
            Some(WORKSPACE),
            json!({"query": "needle", "scope": []}),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn a_revoked_context_document_is_never_served_from_the_index() {
        let directory = tempfile::tempdir().expect("temporary retrieval directory");
        let store = test_store(&directory);
        create_workspace(&store, WORKSPACE, OWNER);
        create_text_resource(
            &store,
            WORKSPACE,
            OWNER,
            "resource-needle",
            "needle.txt",
            CONTENT,
        );
        // The upload path stores the ContextDocument metadata; the owner status
        // transition is the Core-owned way to revoke it.
        let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
            .expect("open the state database");
        connection
            .execute(
                "UPDATE resources SET context_document_json = ?1, version = 2 WHERE workspace_id = ?2 AND resource_id = ?3",
                rusqlite::params![
                    serde_json::json!({
                        "kind": "WORKSPACE_NOTES",
                        "owner_ref": {"kind": "WORKSPACE", "workspace_id": WORKSPACE},
                        "status": "ACTIVE",
                    })
                    .to_string(),
                    WORKSPACE,
                    "resource-needle",
                ],
            )
            .expect("attach the ContextDocument metadata");
        drop(connection);

        let router = router_for(store.clone(), OWNER);
        let active = post_retrieval(
            &router,
            Some(WORKSPACE),
            json!({"query": "needle"}),
        )
        .await;
        assert_eq!(active.status(), StatusCode::OK);
        let body = to_bytes(active.into_body(), 256 * 1024)
            .await
            .expect("bounded body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(body["status"], "GROUNDED");

        ResourceService::new(store)
            .set_context_document_status(SetContextDocumentStatusCommand {
                workspace_id: WORKSPACE.to_owned(),
                resource_id: "resource-needle".to_owned(),
                principal_id: OWNER.to_owned(),
                request_id: "request-revoke-needle".to_owned(),
                expected_version: 2,
                target_status: storage_core::ContextDocumentOwnerStatus::Revoked,
                event: EventContext {
                    event_id: "event-revoke-needle".to_owned(),
                    origin_runtime_id: RUNTIME.to_owned(),
                    hlc_timestamp: "2026-10-20T13:00:00Z".to_owned(),
                    correlation_id: "event-revoke-needle".to_owned(),
                    causation_id: None,
                    recorded_at: "2026-10-20T13:00:00Z".to_owned(),
                },
            })
            .expect("the owner revokes the ContextDocument");

        let revoked = post_retrieval(
            &router,
            Some(WORKSPACE),
            json!({"query": "needle"}),
        )
        .await;
        assert_eq!(revoked.status(), StatusCode::OK);
        let body = to_bytes(revoked.into_body(), 256 * 1024)
            .await
            .expect("bounded body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(
            body["status"], "INSUFFICIENT_EVIDENCE",
            "a revoked Resource must never be served from the local index"
        );
        assert!(
            body["citations"].as_array().expect("citations array").is_empty(),
            "a revoked Resource must not produce a citation"
        );
    }

    #[tokio::test]
    async fn the_index_state_route_reports_ready_stale_and_not_indexable() {        let directory = tempfile::tempdir().expect("temporary retrieval directory");
        let store = test_store(&directory);
        create_workspace(&store, WORKSPACE, OWNER);
        create_text_resource(
            &store,
            WORKSPACE,
            OWNER,
            "resource-needle",
            "needle.txt",
            CONTENT,
        );
        let router = router_for(store, OWNER);

        let response = get_index_state(&router, WORKSPACE, "resource-needle").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("bounded body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(body["state"], "READY");
        assert_eq!(body["indexedRevisionId"], "revision-resource-needle");
        assert_eq!(body["parserId"], "litecowork.plain-text.v1");
        assert_eq!(body["supersededIndexCount"], 0);

        let missing = get_index_state(&router, WORKSPACE, "resource-missing").await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_local_model_route_reports_inventory_only_without_probing_an_endpoint() {
        let directory = tempfile::tempdir().expect("temporary retrieval directory");
        let store = test_store(&directory);
        create_workspace(&store, WORKSPACE, OWNER);
        let router = router_for(store, OWNER);

        let response = router
            .clone()
            .oneshot(
                {
                    let mut request = Request::builder()
                        .method(axum::http::Method::GET)
                        .uri("/v1/capabilities/local-models")
                        .header("x-workspace-id", WORKSPACE)
                        .body(Body::empty())
                        .expect("request builds");
                    request.extensions_mut().insert(AuthenticatedLocalPeer);
                    request
                },
            )
            .await
            .expect("router responds");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("bounded body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(body["capabilityId"], "litecowork.local-models");
        assert_eq!(body["status"], "NOT_CONFIGURED");
        assert_eq!(body["reasonCode"], "LOCAL_MODEL_PROVIDER_NOT_QUALIFIED");
        assert_eq!(body["inventoryOnly"], true);
        assert_eq!(body["surface"], "AGENT_SCOPED");
        assert_eq!(
            body["agentBindingPolicy"],
            "AGENT_MODULE_COMPATIBILITY_REQUIRED"
        );
        assert!(body["models"].as_array().expect("models array").is_empty());
    }
}
