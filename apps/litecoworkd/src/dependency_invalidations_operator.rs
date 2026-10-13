use super::*;
use axum::extract::FromRef;
use serde::{Deserialize, Serialize};
use storage_core::{ArtifactReadStore, DependencyInvalidationPageRequest, InvalidationRecord};

#[derive(Clone)]
pub(super) struct DependencyInvalidationsApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
}

impl DependencyInvalidationsApiState {
    #[cfg(test)]
    pub(super) fn new(store: SqliteWorkspaceStore, principal_id: String) -> Self {
        Self {
            store,
            principal_id,
        }
    }
}

impl FromRef<ApiState> for DependencyInvalidationsApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
        }
    }
}

pub(super) fn routes() -> Router<DependencyInvalidationsApiState> {
    Router::new().route(
        "/v1/dependency-edges/{dependency_edge_id}/invalidations",
        get(list_dependency_invalidations),
    )
}

#[cfg(test)]
pub(super) fn test_router(state: DependencyInvalidationsApiState) -> Router {
    routes()
        .route_layer(middleware::from_fn(authenticate))
        .with_state(state)
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidationQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InvalidationCursor {
    version: u32,
    workspace_id: String,
    dependency_edge_id: String,
    limit: usize,
    after_created_at: String,
    after_invalidation_record_id: String,
}

#[derive(Serialize)]
struct InvalidationPage {
    items: Vec<InvalidationRecord>,
    next_cursor: Option<String>,
}

async fn list_dependency_invalidations(
    State(state): State<DependencyInvalidationsApiState>,
    Path(dependency_edge_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<InvalidationQuery>,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner_for(&state.store, &state.principal_id, &workspace_id)?;
    if !valid_identifier(&dependency_edge_id, 256) {
        return Err(invalid_cursor());
    }
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Invalidation page limit must be between 1 and 200",
        ));
    }
    let cursor = query
        .cursor
        .as_deref()
        .map(|encoded| decode_cursor(encoded, &workspace_id, &dependency_edge_id, limit))
        .transpose()?;
    let mut items = state
        .store
        .list_dependency_invalidations_page(DependencyInvalidationPageRequest {
            owner_principal_id: state.principal_id,
            workspace_id: workspace_id.clone(),
            dependency_edge_id: dependency_edge_id.clone(),
            after_created_at: cursor
                .as_ref()
                .map(|cursor| cursor.after_created_at.clone()),
            after_invalidation_record_id: cursor
                .as_ref()
                .map(|cursor| cursor.after_invalidation_record_id.clone()),
            limit: limit + 1,
        })
        .map_err(|_| {
            operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Dependency invalidation history is unavailable",
            )
        })?
        .ok_or_else(|| {
            operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Dependency edge is unavailable",
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
                encode_cursor(&InvalidationCursor {
                    version: 1,
                    workspace_id,
                    dependency_edge_id,
                    limit,
                    after_created_at: item.created_at.clone(),
                    after_invalidation_record_id: item.invalidation_record_id.clone(),
                })
            })
            .transpose()?
    } else {
        None
    };
    Ok(no_store(
        Json(InvalidationPage { items, next_cursor }).into_response(),
    ))
}

fn decode_cursor(
    encoded: &str,
    workspace_id: &str,
    dependency_edge_id: &str,
    limit: usize,
) -> Result<InvalidationCursor, Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid_cursor());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| invalid_cursor())?;
    let cursor: InvalidationCursor =
        serde_json::from_slice(&bytes).map_err(|_| invalid_cursor())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || cursor.dependency_edge_id != dependency_edge_id
        || cursor.limit != limit
        || !valid_identifier(&cursor.after_invalidation_record_id, 256)
        || cursor.after_created_at.is_empty()
        || cursor.after_created_at.len() > 64
        || cursor.after_created_at.chars().any(char::is_control)
    {
        return Err(invalid_cursor());
    }
    Ok(cursor)
}

fn encode_cursor(cursor: &InvalidationCursor) -> Result<String, Response> {
    let bytes = serde_json::to_vec(cursor).map_err(|_| {
        operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Dependency invalidation cursor could not be created",
        )
    })?;
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    if encoded.len() > 2048 {
        return Err(operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Dependency invalidation cursor exceeded its size limit",
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
        "Dependency invalidation cursor is invalid or does not match this query",
    )
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
