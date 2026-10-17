//! Workspace authorized, current-incarnation Runtime offer inventory.
#![allow(clippy::result_large_err)]

use super::{ApiState, operator_error, operator_now, selected_workspace};
use axum::{
    Json, Router,
    extract::{FromRef, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use storage_core::{
    AgentCatalogStore, RuntimeOfferPageRequest, RuntimeOfferPosition, RuntimeOfferRecord,
    StoreError,
};
use storage_sqlite::SqliteWorkspaceStore;

#[derive(Clone)]
pub(super) struct RuntimeOffersApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
}

impl RuntimeOffersApiState {
    #[cfg(test)]
    pub(super) fn new(store: SqliteWorkspaceStore, principal_id: String) -> Self {
        Self {
            store,
            principal_id,
        }
    }
}

impl FromRef<ApiState> for RuntimeOffersApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
        }
    }
}

pub(super) fn runtime_offers_routes() -> Router<RuntimeOffersApiState> {
    Router::new().route("/v1/runtimes/{runtime_id}/offers", get(list_runtime_offers))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OfferQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfferCursor {
    version: u8,
    workspace_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
    limit: usize,
    offer_kind: String,
    offer_ref: String,
}

#[derive(Serialize)]
struct RuntimeOfferPage {
    items: Vec<RuntimeOfferRecord>,
    next_cursor: Option<String>,
}

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Runtime offer query is invalid",
    )
}

fn store_error(error: StoreError) -> Response {
    match error {
        StoreError::NotFound => operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Runtime offers are unavailable in this Workspace",
        ),
        StoreError::Conflict { .. } => operator_error(
            StatusCode::CONFLICT,
            "STALE_VERSION",
            "Runtime offer cursor no longer matches the current Runtime incarnation",
        ),
        StoreError::Invalid(_) => invalid(),
        _ => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Runtime offers could not be read",
        ),
    }
}

fn decode_cursor(
    encoded: &str,
    workspace_id: &str,
    runtime_id: &str,
    limit: usize,
) -> Result<OfferCursor, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: OfferCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || cursor.runtime_id != runtime_id
        || cursor.limit != limit
        || cursor.runtime_incarnation_id.trim().is_empty()
        || cursor.offer_kind != "AGENT_ENDPOINT"
        || cursor.offer_ref.trim().is_empty()
        || cursor.offer_ref.len() > 256
        || cursor.offer_ref.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    Ok(cursor)
}

fn encode_cursor(cursor: &OfferCursor) -> Result<String, Response> {
    let bytes = serde_json::to_vec(cursor).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Runtime offer cursor could not be prepared",
        )
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

async fn list_runtime_offers(
    State(state): State<RuntimeOffersApiState>,
    Path(runtime_id): Path<String>,
    headers: HeaderMap,
    query: Result<Query<OfferQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    if runtime_id.trim().is_empty()
        || runtime_id.len() > 256
        || runtime_id.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(invalid());
    }
    let cursor = query
        .cursor
        .as_deref()
        .map(|value| decode_cursor(value, &workspace_id, &runtime_id, limit))
        .transpose()?;
    let now = operator_now()?;
    let expected_incarnation_id = cursor
        .as_ref()
        .map(|cursor| cursor.runtime_incarnation_id.as_str());
    let after = cursor.as_ref().map(|cursor| RuntimeOfferPosition {
        runtime_incarnation_id: cursor.runtime_incarnation_id.clone(),
        offer_kind: cursor.offer_kind.clone(),
        offer_ref: cursor.offer_ref.clone(),
    });
    let mut offers = state
        .store
        .list_runtime_offers(RuntimeOfferPageRequest {
            owner_principal_id: state.principal_id,
            workspace_id: workspace_id.clone(),
            runtime_id: runtime_id.clone(),
            expected_incarnation_id: expected_incarnation_id.map(str::to_owned),
            now: now.clone(),
            after,
            limit: limit + 1,
        })
        .map_err(store_error)?;
    let has_more = offers.len() > limit;
    offers.truncate(limit);
    let next_cursor = if has_more {
        let last = offers.last().ok_or_else(invalid)?;
        Some(encode_cursor(&OfferCursor {
            version: 1,
            workspace_id,
            runtime_id,
            runtime_incarnation_id: last.runtime_incarnation_id.clone(),
            limit,
            offer_kind: last.offer_kind.clone(),
            offer_ref: last.offer_ref.clone(),
        })?)
    } else {
        None
    };
    let mut response = (
        StatusCode::OK,
        Json(RuntimeOfferPage {
            items: offers,
            next_cursor,
        }),
    )
        .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header"),
    );
    Ok(response)
}

#[cfg(test)]
pub(super) fn runtime_offers_router(state: RuntimeOffersApiState) -> Router {
    use super::authenticate;
    runtime_offers_routes()
        .route_layer(axum::middleware::from_fn(authenticate))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::{RuntimeOffersApiState, runtime_offers_router};
    use crate::operator::AuthenticatedLocalPeer;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use std::{sync::Arc, time::Duration};
    use storage_core::{BlobPurpose, StoreError};
    use storage_sqlite::{
        FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey,
        WorkspaceBlobKeyProvider,
    };
    use tower::ServiceExt;
    use zeroize::Zeroizing;

    #[derive(Clone)]
    struct TestKeys;

    fn restrict_test_directory(directory: &tempfile::TempDir) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("private test directory permissions");
        }
        #[cfg(not(unix))]
        let _ = directory;
    }

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

    fn test_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
        SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("temporary SQLite store opens")
    }

    #[tokio::test]
    async fn runtime_offer_route_requires_selected_workspace_context() {
        let directory = tempfile::tempdir().expect("temporary store directory");
        restrict_test_directory(&directory);
        let app = runtime_offers_router(RuntimeOffersApiState {
            store: test_store(&directory),
            principal_id: "owner-local".to_owned(),
        });
        let mut request = Request::get("/v1/runtimes/runtime-1/offers")
            .body(Body::empty())
            .expect("valid request");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        let response = app
            .oneshot(request)
            .await
            .expect("route returns a response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn runtime_offer_route_rejects_limits_above_the_contract_cap() {
        let directory = tempfile::tempdir().expect("temporary store directory");
        restrict_test_directory(&directory);
        let app = runtime_offers_router(RuntimeOffersApiState {
            store: test_store(&directory),
            principal_id: "owner-local".to_owned(),
        });
        let mut request = Request::get("/v1/runtimes/runtime-1/offers?limit=201")
            .header("x-workspace-id", "workspace-1")
            .body(Body::empty())
            .expect("valid request");
        request.extensions_mut().insert(AuthenticatedLocalPeer);
        let response = app
            .oneshot(request)
            .await
            .expect("route returns a response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
