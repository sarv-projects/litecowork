//! Read-only, Workspace-scoped Needs You projection over canonical source rows.

#![allow(clippy::result_large_err)]

use super::*;
use axum::extract::FromRef;
use storage_core::{
    NeedsYouCursor, NeedsYouFilter, NeedsYouItemKind, NeedsYouPage, NeedsYouPageRequest,
    NeedsYouReadStore, NeedsYouStatus,
};

#[derive(Clone)]
pub(super) struct NeedsYouApiState {
    store: SqliteWorkspaceStore,
    principal_id: String,
}

impl FromRef<ApiState> for NeedsYouApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            principal_id: state.principal_id.clone(),
        }
    }
}

pub(super) fn routes() -> Router<NeedsYouApiState> {
    Router::new()
        .route("/v1/needs-you", get(list_needs_you))
        .route_layer(middleware::from_fn(authenticate))
        // The inbox projection is private Workspace state. Stamp every response, including
        // cursor/validation failures, so no local cache or intermediary retains it.
        .route_layer(middleware::from_fn(needs_you_no_store))
}

async fn needs_you_no_store(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct NeedsYouQuery {
    status: Option<String>,
    kind: Option<String>,
    task_id: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct NeedsYouPageResponse {
    items: Vec<storage_core::NeedsYouItem>,
    next_cursor: Option<String>,
}

async fn list_needs_you(
    State(state): State<NeedsYouApiState>,
    headers: HeaderMap,
    query: Result<Query<NeedsYouQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<NeedsYouPageResponse>, Response> {
    let workspace_id = selected_workspace(&headers)?;
    let Query(query) = query.map_err(|_| invalid_needs_you_query())?;
    let status = parse_needs_you_status(query.status.as_deref())?;
    let kind = parse_needs_you_kind(query.kind.as_deref())?;
    if query
        .task_id
        .as_deref()
        .is_some_and(|task_id| !valid_task_query_id(task_id))
    {
        return Err(invalid_needs_you_query());
    }
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit)
        || query
            .cursor
            .as_deref()
            .is_some_and(|cursor| cursor.is_empty() || cursor.len() > 4096 || !cursor.is_ascii())
    {
        return Err(invalid_needs_you_query());
    }
    let filter = NeedsYouFilter {
        status: Some(status),
        kind,
        task_id: query.task_id,
    };
    let cursor = query
        .cursor
        .as_deref()
        .map(|value| decode_needs_you_cursor(value, &workspace_id, &filter, limit))
        .transpose()?;
    let owner = state.principal_id.clone();
    let selected = workspace_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        ensure_workspace_owner_for(&state.store, &owner, &selected)?;
        state
            .store
            .list_needs_you_page(NeedsYouPageRequest {
                workspace_id: selected,
                filter,
                limit,
                cursor,
            })
            .map_err(|_| needs_you_unavailable())
    })
    .await
    .map_err(|_| needs_you_unavailable())??;
    Ok(Json(needs_you_page_response(result)?))
}

fn needs_you_page_response(page: NeedsYouPage) -> Result<NeedsYouPageResponse, Response> {
    let next_cursor = page
        .next_cursor
        .map(|cursor| {
            serde_json::to_vec(&cursor)
                .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
                .map_err(|_| needs_you_unavailable())
        })
        .transpose()?;
    Ok(NeedsYouPageResponse {
        items: page.items,
        next_cursor,
    })
}

fn decode_needs_you_cursor(
    encoded: &str,
    workspace_id: &str,
    filter: &NeedsYouFilter,
    limit: usize,
) -> Result<NeedsYouCursor, Response> {
    let invalid = invalid_needs_you_query;
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|_| invalid())?;
    let cursor: NeedsYouCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.workspace_id != workspace_id
        || &cursor.filter != filter
        || cursor.limit != limit
        || !canonical_needs_you_cursor_timestamp(&cursor.after_created_at)
        || !storage_core::is_sha256_digest(&cursor.after_item_id)
    {
        return Err(invalid());
    }
    Ok(cursor)
}

fn canonical_needs_you_cursor_timestamp(value: &str) -> bool {
    let Ok(parsed) =
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
    else {
        return false;
    };
    let parsed = parsed.to_offset(time::UtcOffset::UTC);
    let canonical = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        parsed.year(),
        u8::from(parsed.month()),
        parsed.day(),
        parsed.hour(),
        parsed.minute(),
        parsed.second(),
        parsed.nanosecond(),
    );
    canonical == value
}

fn parse_needs_you_status(value: Option<&str>) -> Result<NeedsYouStatus, Response> {
    match value.unwrap_or("OPEN") {
        "OPEN" => Ok(NeedsYouStatus::Open),
        "RESOLVED" => Ok(NeedsYouStatus::Resolved),
        _ => Err(invalid_needs_you_query()),
    }
}

fn parse_needs_you_kind(value: Option<&str>) -> Result<Option<NeedsYouItemKind>, Response> {
    match value {
        None => Ok(None),
        Some("APPROVAL") => Ok(Some(NeedsYouItemKind::Approval)),
        Some("USER_REQUEST") => Ok(Some(NeedsYouItemKind::UserRequest)),
        Some("TASK_BLOCKER") => Ok(Some(NeedsYouItemKind::TaskBlocker)),
        Some(_) => Err(invalid_needs_you_query()),
    }
}

fn invalid_needs_you_query() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Needs You filters or cursor are invalid",
    )
}

fn needs_you_unavailable() -> Response {
    operator_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL",
        "Needs You is unavailable",
    )
}

#[cfg(test)]
pub(super) fn test_router(store: SqliteWorkspaceStore, principal_id: String) -> Router {
    routes().with_state(NeedsYouApiState {
        store,
        principal_id,
    })
}
