//! Authenticated local Operator routes for the Discover catalog.
//!
//! Authority: [`docs/API.md`](../../../docs/API.md),
//! [`docs/CAPABILITY-FABRIC.md`](../../../docs/CAPABILITY-FABRIC.md),
//! [`docs/CONNECTORS-WORKFLOWS.md`](../../../docs/CONNECTORS-WORKFLOWS.md).
//!
//! Discover is a user-facing projection over capabilities LiteCowork already
//! knows. It never calls a package manager from the client, never exposes
//! package-manager internals, and never treats a catalog entry as authority.
//! The LiteSPM package service is not qualified yet, so the connect-or-install
//! and capability-test commands report that truthfully instead of simulating a
//! package lifecycle.

#![allow(clippy::result_large_err)] // Handlers return full Axum Responses to preserve status, body, and headers.

use super::*;
use serde::Deserialize;
use serde_json::json;
use storage_core::{
    DiscoverAuthState, DiscoverAvailability, DiscoverCatalogEntry, DiscoverCompatibility,
    DiscoverEntryKind, DiscoverEntrySource, DiscoverInstallState,
};
use storage_sqlite::{DiscoverProviderStatus, SqliteDiscoverStore};

const MAX_BODY_BYTES: usize = 64 * 1024;
const DEFAULT_PAGE_LIMIT: usize = 50;
const MAX_PAGE_LIMIT: usize = 200;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/discover/search", get(search_discover))
        .route("/v1/discover/items/{item_id}", get(describe_discover_item))
        .route(
            "/v1/capabilities/{capability_id}/connect-or-install",
            post(connect_or_install_capability),
        )
        .route(
            "/v1/capabilities/{capability_id}/test",
            post(test_capability_connection),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscoverQuery {
    q: Option<String>,
    kind: Option<DiscoverEntryKind>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityRequest {
    workspace_id: String,
    runtime_id: Option<String>,
    /// The scope the owner would grant. The package path is not qualified, so
    /// the request is refused before the scope is ever honoured.
    requested_scope: Option<Value>,
}

/// Refuse a request that names another Runtime.
///
/// The capability would be resolved for a Runtime this session does not own, so
/// the request is refused instead of being silently resolved locally.
fn check_requested_runtime(state: &ApiState, request: &CapabilityRequest) -> Result<(), Response> {
    if let Some(runtime_id) = request.runtime_id.as_deref() {
        if !runtime_id.trim().is_empty() && runtime_id != state.runtime_id {
            return Err(operator_error_with_details(
                StatusCode::CONFLICT,
                "RUNTIME_MISMATCH",
                "The request names another Runtime",
                Some(json!({"reason": "RUNTIME_MISMATCH"})),
            ));
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct DiscoverItemView {
    item_ref: String,
    kind: DiscoverEntryKind,
    display_name: String,
    summary: String,
    description: Option<String>,
    compatibility: DiscoverCompatibility,
    compatibility_reason: Option<String>,
    requires_user_action: bool,
    availability: DiscoverAvailability,
    availability_reason: String,
    install_state: DiscoverInstallState,
    auth_state: DiscoverAuthState,
    source: DiscoverEntrySource,
    publisher: Option<String>,
    version: Option<String>,
    digest: Option<String>,
    grants_required: Vec<String>,
    /// Always false. A catalog entry is never an authorization.
    authoritative: bool,
}

#[derive(Serialize)]
struct DiscoverItemPageView {
    items: Vec<DiscoverItemView>,
    next_cursor: Option<String>,
    provider_status: DiscoverProviderStatus,
    /// The Runtime's observed capability hosts. Observations, not catalog rows.
    runtime_capabilities: Vec<DiscoverItemView>,
}

#[derive(Serialize)]
struct CapabilityHealthView {
    health: String,
    checked_at: String,
    safe_summary: Option<String>,
}

#[derive(Serialize)]
struct DiscoverItemDetailView {
    item: DiscoverItemView,
    capabilities: Vec<Value>,
    provider_note: Option<String>,
}

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Discover request is invalid",
    )
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Discover storage is unavailable",
    )
}

fn not_found() -> Response {
    operator_error(
        StatusCode::NOT_FOUND,
        "NOT_FOUND",
        "Discover entry is unavailable",
    )
}

fn forbidden() -> Response {
    operator_error(
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
        "Workspace access is unavailable",
    )
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn decode_cursor(encoded: &str, workspace: &str) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    let version = cursor.get("version").and_then(serde_json::Value::as_u64).unwrap_or(0);
    let cursor_workspace = cursor
        .get("workspace_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let created_at = cursor
        .get("created_at")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let entity_id = cursor
        .get("entity_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if version != 1
        || cursor_workspace != workspace
        || !valid_id(&entity_id)
        || created_at.len() > 64
    {
        return Err(invalid());
    }
    Ok((created_at, entity_id))
}

fn encode_cursor(workspace: &str, created_at: &str, entity_id: &str) -> Result<String, Response> {
    let bytes = serde_json::to_vec(&json!({
        "version": 1,
        "workspace_id": workspace,
        "created_at": created_at,
        "entity_id": entity_id,
    }))
    .map_err(|_| unavailable())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn item_view(entry: &DiscoverCatalogEntry) -> DiscoverItemView {
    DiscoverItemView {
        item_ref: entry.item_ref.clone(),
        kind: entry.kind,
        display_name: entry.display_name.clone(),
        summary: entry.summary.clone(),
        description: entry.description.clone(),
        compatibility: entry.compatibility,
        compatibility_reason: entry.compatibility_reason.clone(),
        requires_user_action: entry.requires_user_action(),
        availability: entry.availability,
        availability_reason: entry.availability_reason.clone(),
        install_state: entry.install_state,
        auth_state: entry.auth_state,
        source: entry.source,
        publisher: entry.publisher.clone(),
        version: entry.version.clone(),
        digest: entry.digest.clone(),
        grants_required: entry.grants_required.clone(),
        authoritative: entry.authoritative,
    }
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

async fn search_discover(
    State(state): State<ApiState>,
    headers: HeaderMap,
    query: Result<Query<DiscoverQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let limit = query.limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(invalid());
    }
    if query
        .q
        .as_deref()
        .is_some_and(|value| value.is_empty() || value.len() > 512)
    {
        return Err(invalid());
    }
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| decode_cursor(cursor, &workspace.workspace_id))
        .transpose()?;
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let now = correlation.clone();
    let store = SqliteDiscoverStore::new(state.store.clone());
    let page = store
        .search_catalog(
            &state.principal_id,
            &workspace.workspace_id,
            &state.runtime_id,
            &state.local_incarnation_id,
            query.q.as_deref(),
            query.kind,
            &now,
            after,
            limit,
        )
        .map_err(|_| unavailable())?;
    let items: Vec<DiscoverItemView> = page.items.iter().map(item_view).collect();
    let next_cursor = match page.next_cursor {
        Some((created_at, entity_id)) => {
            Some(encode_cursor(&workspace.workspace_id, &created_at, &entity_id)?)
        }
        None => None,
    };
    Ok(no_store(
        Json(DiscoverItemPageView {
            items,
            next_cursor,
            provider_status: page.provider_status,
            runtime_capabilities: page
                .runtime_capabilities
                .iter()
                .map(item_view)
                .collect(),
        })
        .into_response(),
    ))
}

async fn describe_discover_item(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&item_id) {
        return Err(invalid());
    }
    let correlation = new_id("cor").map_err(|_| unavailable())?;
    let store = SqliteDiscoverStore::new(state.store.clone());
    let detail = store
        .describe_entry(
            &state.principal_id,
            &workspace.workspace_id,
            &state.runtime_id,
            &state.local_incarnation_id,
            &item_id,
            &correlation,
        )
        .map_err(|_| unavailable())?
        .ok_or_else(not_found)?;
    Ok(no_store(
        Json(DiscoverItemDetailView {
            item: item_view(&detail.entry),
            capabilities: detail
                .capabilities
                .iter()
                .map(|capability| {
                    json!({
                        "capability_id": capability.capability_id,
                        "display_name": capability.display_name,
                        "operation_class": capability.operation_class,
                        "required_scopes": capability.required_scopes,
                    })
                })
                .collect(),
            provider_note: detail.provider_note,
        })
        .into_response(),
    ))
}

async fn connect_or_install_capability(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(capability_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&capability_id) {
        return Err(invalid());
    }
    let request: CapabilityRequest = serde_json::from_slice(&body).map_err(|_| invalid())?;
    if request.workspace_id != workspace.workspace_id {
        return Err(forbidden());
    }
    check_requested_runtime(&state, &request)?;
    // LiteSPM owns package discovery, verification, and installation. Its API,
    // authentication, and package model are not published, so the command
    // reports the truthful state instead of pretending to install anything.
    // The requested scope is echoed back as *not granted*: the scope was never
    // resolved, because no qualified provider resolved it.
    Err(operator_error_with_details(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Package installation is unavailable until the LiteSPM package contract is published and qualified",
        Some(json!({
            "reason": "LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED",
            "capability_id": capability_id,
            "requested_scope_granted": false,
            "requested_scope": request.requested_scope,
        })),
    ))
}

async fn test_capability_connection(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(capability_id): Path<String>,
    body: Bytes,
) -> Result<Response, Response> {
    let workspace = authorized_workspace(&state, &headers)?;
    if !valid_id(&capability_id) {
        return Err(invalid());
    }
    let request: CapabilityRequest = serde_json::from_slice(&body).map_err(|_| invalid())?;
    if request.workspace_id != workspace.workspace_id {
        return Err(forbidden());
    }
    check_requested_runtime(&state, &request)?;
    // A readiness probe against a provider LiteCowork does not qualify would
    // be an invented probe, so no provider is contacted. The truthful answer is
    // that the connection health is unknown, and why.
    Ok(no_store(
        Json(CapabilityHealthView {
            health: "UNKNOWN".to_owned(),
            checked_at: new_id("cor").map_err(|_| unavailable())?,
            safe_summary: Some(
                "Connection readiness was not observed: the LiteSPM package service is not qualified, so no provider was contacted."
                    .to_owned(),
            ),
        })
        .into_response(),
    ))
}
