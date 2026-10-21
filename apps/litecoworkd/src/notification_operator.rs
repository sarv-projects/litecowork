//! Authenticated local Operator reads for notification delivery and policy.
//!
//! Authority: [`docs/API.md`](../../../docs/API.md) "User requests and
//! notifications", [`docs/DATA-MODEL.md`](../../../docs/DATA-MODEL.md)
//! "Notifications", and the `Notification` / `NotificationPage` /
//! `NotificationPreference` OpenAPI components.
//!
//! These routes read what the canonical `notification_deliveries` and
//! `notification_preferences` tables already contain. They do not decide whether
//! a notification is due; that is the domain `NotificationService`. A `SENT`
//! status is transport acknowledgement only and is never presented as Task
//! completion.
//!
//! Both routes are owner-scoped Core data, not Workspace-scoped execution, so
//! they use `authorized_data_workspace` and stay readable after the Workspace is
//! archived.

// Route handlers intentionally return full Axum Responses to preserve status, body, and headers.
#![allow(clippy::result_large_err)]

use super::*;
use serde::{Deserialize, Serialize};
use storage_core::{NotificationPreferenceRecord, NotificationRecord};

const DEFAULT_PAGE_LIMIT: usize = 50;
const MAX_PAGE_LIMIT: usize = 200;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/notifications", get(list_notifications))
        .route(
            "/v1/notification-preferences",
            get(list_notification_preferences),
        )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NotificationQuery {
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct NotificationPage {
    items: Vec<NotificationRecord>,
    next_cursor: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeliveryCursor {
    version: u8,
    workspace_id: String,
    created_at: String,
    delivery_id: String,
}

async fn list_notifications(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<NotificationQuery>,
) -> Result<Response, Response> {
    let workspace = authorized_data_workspace(&state, &headers)?;
    let limit = query.limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(invalid());
    }
    let (after_created_at, after_delivery_id) = match query.cursor.as_deref() {
        Some(encoded) => {
            let (created_at, delivery_id) = decode_cursor(encoded, &workspace.workspace_id)?;
            (Some(created_at), Some(delivery_id))
        }
        None => (None, None),
    };
    // Ask for one row beyond the page so `next_cursor` reflects a real next row
    // instead of an empty follow-up page. The storage limit is 201 for this.
    let mut items = state
        .store
        .list_notification_deliveries(
            &state.principal_id,
            &workspace.workspace_id,
            query.status.as_deref(),
            after_created_at.as_deref(),
            after_delivery_id.as_deref(),
            limit + 1,
        )
        .map_err(store_error)?;
    let next_cursor = if items.len() > limit {
        items.truncate(limit);
        let boundary = items.last().expect("a truncated page is not empty");
        Some(encode_cursor(
            &workspace.workspace_id,
            &boundary.created_at,
            &boundary.delivery_id,
        )?)
    } else {
        None
    };
    Ok(no_store(
        Json(NotificationPage { items, next_cursor }).into_response(),
    ))
}

async fn list_notification_preferences(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace = authorized_data_workspace(&state, &headers)?;
    let preferences: Vec<NotificationPreferenceRecord> = state
        .store
        .list_notification_preferences(&state.principal_id, &workspace.workspace_id)
        .map_err(store_error)?;
    Ok(no_store(Json(preferences).into_response()))
}

fn encode_cursor(
    workspace_id: &str,
    created_at: &str,
    delivery_id: &str,
) -> Result<String, Response> {
    let bytes = serde_json::to_vec(&DeliveryCursor {
        version: 1,
        workspace_id: workspace_id.to_owned(),
        created_at: created_at.to_owned(),
        delivery_id: delivery_id.to_owned(),
    })
    .map_err(|_| internal())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn decode_cursor(encoded: &str, workspace_id: &str) -> Result<(String, String), Response> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let cursor: DeliveryCursor = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if cursor.version != 1
        || cursor.workspace_id != workspace_id
        || !valid_text(&cursor.created_at, 64)
        || !valid_text(&cursor.delivery_id, 256)
    {
        return Err(invalid());
    }
    Ok((cursor.created_at, cursor.delivery_id))
}

fn valid_text(value: &str, max_len: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_len && !value.chars().any(char::is_control)
}

fn invalid() -> Response {
    operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Notification request is invalid",
    )
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Notification storage is unavailable",
    )
}

fn internal() -> Response {
    operator_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL",
        "Notification read failed",
    )
}

fn store_error(error: storage_core::StoreError) -> Response {
    match error {
        storage_core::StoreError::NotFound => operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Notification history is unavailable",
        ),
        storage_core::StoreError::Invalid(_) => invalid(),
        _ => unavailable(),
    }
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_round_trips_only_for_its_own_workspace() {
        let Ok(encoded) = encode_cursor("workspace-1", "2026-10-21T09:00:00Z", "delivery-1")
        else {
            panic!("cursor encodes");
        };
        let Ok((created_at, delivery_id)) = decode_cursor(&encoded, "workspace-1") else {
            panic!("cursor decodes");
        };
        assert_eq!(created_at, "2026-10-21T09:00:00Z");
        assert_eq!(delivery_id, "delivery-1");
        assert!(decode_cursor(&encoded, "workspace-2").is_err());
    }

    #[test]
    fn a_malformed_or_tampered_cursor_is_rejected() {
        assert!(decode_cursor("", "workspace-1").is_err());
        assert!(decode_cursor("not-base64", "workspace-1").is_err());
        let Ok(encoded) = encode_cursor("workspace-1", "2026-10-21T09:00:00Z", "delivery-1")
        else {
            panic!("cursor encodes");
        };
        assert!(decode_cursor(&encoded, "workspace-1").is_ok());
        assert!(decode_cursor(&format!("{encoded}x"), "workspace-1").is_err());
    }

    #[test]
    fn a_cursor_carrying_a_control_character_is_rejected() {
        let bytes = serde_json::to_vec(&DeliveryCursor {
            version: 1,
            workspace_id: "workspace-1".to_owned(),
            created_at: "2026-10-21T09:00:00Z".to_owned(),
            delivery_id: "delivery\u{0007}".to_owned(),
        })
        .expect("cursor serializes");
        let encoded = URL_SAFE_NO_PAD.encode(bytes);
        assert!(decode_cursor(&encoded, "workspace-1").is_err());
    }

    #[test]
    fn a_cursor_for_another_version_is_rejected() {
        let bytes = serde_json::to_vec(&DeliveryCursor {
            version: 2,
            workspace_id: "workspace-1".to_owned(),
            created_at: "2026-10-21T09:00:00Z".to_owned(),
            delivery_id: "delivery-1".to_owned(),
        })
        .expect("cursor serializes");
        let encoded = URL_SAFE_NO_PAD.encode(bytes);
        assert!(decode_cursor(&encoded, "workspace-1").is_err());
    }
}
