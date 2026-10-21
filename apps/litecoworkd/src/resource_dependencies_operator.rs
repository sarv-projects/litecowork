use super::*;
use axum::extract::FromRef;
use serde::{Deserialize, Serialize};
use storage_core::{
    ResourceDependencyPageRequest, ResourceDependencyReadStore, ResourceDependencyViewRecord,
};

#[derive(Clone)]
pub(super) struct ResourceDependenciesApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
}

impl ResourceDependenciesApiState {
    #[cfg(test)]
    pub(super) fn new(store: SqliteWorkspaceStore, principal_id: String) -> Self {
        Self {
            store,
            principal_id,
        }
    }
}

impl FromRef<ApiState> for ResourceDependenciesApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
        }
    }
}

pub(super) fn routes() -> Router<ResourceDependenciesApiState> {
    Router::new()
        .route(
            "/v1/resources/{resource_id}/dependents",
            get(list_resource_dependents),
        )
        // Dependency history is private Workspace state. Stamp rejected reads no-store too.
        .route_layer(middleware::from_fn(dependents_no_store))
}

async fn dependents_no_store(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
pub(super) fn test_router(state: ResourceDependenciesApiState) -> Router {
    routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(state)
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DependentsQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DependentsCursor {
    version: u32,
    workspace_id: String,
    resource_id: String,
    limit: usize,
    after_created_at: String,
    after_dependency_edge_id: String,
}

#[derive(Serialize)]
struct DependentsPage {
    items: Vec<ResourceDependencyViewRecord>,
    next_cursor: Option<String>,
}

async fn list_resource_dependents(
    State(state): State<ResourceDependenciesApiState>,
    Path(resource_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<DependentsQuery>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner_for(&state.store, &state.principal_id, &workspace_id)?;
    if !valid_identifier(&resource_id, 256) {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource identifier is invalid",
        ));
    }
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Resource dependent page limit must be between 1 and 200",
        ));
    }
    let cursor = query
        .cursor
        .as_deref()
        .map(|value| decode_cursor(value, &workspace_id, &resource_id, limit))
        .transpose()?;
    let mut items = state
        .store
        .list_resource_dependents_page(ResourceDependencyPageRequest {
            owner_principal_id: state.principal_id,
            workspace_id: workspace_id.clone(),
            resource_id: resource_id.clone(),
            after_created_at: cursor
                .as_ref()
                .map(|cursor| cursor.after_created_at.clone()),
            after_dependency_edge_id: cursor
                .as_ref()
                .map(|cursor| cursor.after_dependency_edge_id.clone()),
            limit: limit + 1,
        })
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Resource dependency history is unavailable",
            )
        })?
        .ok_or_else(|| {
            operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Resource is unavailable",
            )
        })?;
    let has_more = items.len() > limit;
    if has_more {
        items.pop();
    }
    let next_cursor = if has_more {
        items
            .last()
            .map(|item| {
                encode_cursor(&DependentsCursor {
                    version: 1,
                    workspace_id,
                    resource_id,
                    limit,
                    after_created_at: item.dependency_edge.created_at.clone(),
                    after_dependency_edge_id: item.dependency_edge.dependency_edge_id.clone(),
                })
            })
            .transpose()?
    } else {
        None
    };
    let mut response = Json(DependentsPage { items, next_cursor }).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

fn decode_cursor(
    encoded: &str,
    workspace_id: &str,
    resource_id: &str,
    limit: usize,
) -> Result<DependentsCursor, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid_cursor());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| invalid_cursor())?;
    let cursor: DependentsCursor = serde_json::from_slice(&bytes).map_err(|_| invalid_cursor())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || cursor.resource_id != resource_id
        || cursor.limit != limit
        || !valid_identifier(&cursor.after_dependency_edge_id, 256)
        || cursor.after_created_at.is_empty()
        || cursor.after_created_at.len() > 64
        || cursor.after_created_at.chars().any(char::is_control)
    {
        return Err(invalid_cursor());
    }
    Ok(cursor)
}

fn encode_cursor(cursor: &DependentsCursor) -> Result<String, Response> {
    let bytes = serde_json::to_vec(cursor).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Resource dependency cursor could not be created",
        )
    })?;
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    if encoded.len() > 2048 {
        return Err(operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Resource dependency cursor exceeded its size limit",
        ));
    }
    Ok(encoded)
}

fn valid_identifier(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn invalid_cursor() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Resource dependent cursor is invalid or does not match this query",
    )
}
