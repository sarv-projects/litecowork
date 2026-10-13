//! Workspace-authorized reads of normalized Runtime-local provider host observations.
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
    AgentCatalogStore, RuntimeCapabilityHostPageRequest, RuntimeCapabilityHostPosition,
    RuntimeCapabilityHostRecord, StoreError,
};
use storage_sqlite::SqliteWorkspaceStore;

#[derive(Clone)]
pub(super) struct RuntimeCapabilityHostsApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
}

impl RuntimeCapabilityHostsApiState {
    #[cfg(test)]
    pub(super) fn new(store: SqliteWorkspaceStore, principal_id: String) -> Self {
        Self {
            store,
            principal_id,
        }
    }
}

impl FromRef<ApiState> for RuntimeCapabilityHostsApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
        }
    }
}

pub(super) fn runtime_capability_hosts_routes() -> Router<RuntimeCapabilityHostsApiState> {
    Router::new().route(
        "/v1/runtimes/{runtime_id}/capability-hosts",
        get(list_runtime_capability_hosts),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityHostQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityHostCursor {
    version: u8,
    workspace_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
    limit: usize,
    host_instance_id: String,
}

#[derive(Serialize)]
struct CapabilityHostPage {
    items: Vec<RuntimeCapabilityHostRecord>,
    next_cursor: Option<String>,
}

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Runtime capability host query is invalid",
    )
}

fn store_error(error: StoreError) -> Response {
    match error {
        StoreError::NotFound => operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Runtime capability hosts are unavailable in this Workspace",
        ),
        StoreError::Conflict { .. } => operator_error(
            StatusCode::CONFLICT,
            "STALE_VERSION",
            "Runtime capability host cursor no longer matches the current Runtime incarnation",
        ),
        StoreError::Invalid(_) => invalid(),
        _ => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Runtime capability hosts could not be read",
        ),
    }
}

fn decode_cursor(
    encoded: &str,
    workspace_id: &str,
    runtime_id: &str,
    limit: usize,
) -> Result<CapabilityHostCursor, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: CapabilityHostCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || cursor.runtime_id != runtime_id
        || cursor.limit != limit
        || cursor.runtime_incarnation_id.trim().is_empty()
        || cursor.host_instance_id.trim().is_empty()
        || cursor.host_instance_id.len() > 256
        || cursor.host_instance_id.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    Ok(cursor)
}

fn encode_cursor(cursor: &CapabilityHostCursor) -> Result<String, Response> {
    let bytes = serde_json::to_vec(cursor).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Runtime capability host cursor could not be prepared",
        )
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

async fn list_runtime_capability_hosts(
    State(state): State<RuntimeCapabilityHostsApiState>,
    Path(runtime_id): Path<String>,
    headers: HeaderMap,
    query: Result<Query<CapabilityHostQuery>, axum::extract::rejection::QueryRejection>,
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
    let expected_incarnation_id = cursor
        .as_ref()
        .map(|cursor| cursor.runtime_incarnation_id.as_str());
    let after = cursor.as_ref().map(|cursor| RuntimeCapabilityHostPosition {
        runtime_incarnation_id: cursor.runtime_incarnation_id.clone(),
        host_instance_id: cursor.host_instance_id.clone(),
    });
    let now = operator_now()?;
    let mut items = state
        .store
        .list_runtime_capability_hosts(RuntimeCapabilityHostPageRequest {
            owner_principal_id: state.principal_id,
            workspace_id: workspace_id.clone(),
            runtime_id: runtime_id.clone(),
            expected_incarnation_id: expected_incarnation_id.map(str::to_owned),
            now,
            after,
            limit: limit + 1,
        })
        .map_err(store_error)?;
    let has_more = items.len() > limit;
    items.truncate(limit);
    let next_cursor = if has_more {
        let last = items.last().ok_or_else(invalid)?;
        Some(encode_cursor(&CapabilityHostCursor {
            version: 1,
            workspace_id,
            runtime_id,
            runtime_incarnation_id: last.runtime_incarnation_id.clone(),
            limit,
            host_instance_id: last.host_instance_id.clone(),
        })?)
    } else {
        None
    };
    let mut response = (
        StatusCode::OK,
        Json(CapabilityHostPage { items, next_cursor }),
    )
        .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header"),
    );
    Ok(response)
}

#[cfg(test)]
pub(super) fn runtime_capability_hosts_router(state: RuntimeCapabilityHostsApiState) -> Router {
    use super::authenticate;
    runtime_capability_hosts_routes()
        .route_layer(axum::middleware::from_fn(authenticate))
        .with_state(state)
}
