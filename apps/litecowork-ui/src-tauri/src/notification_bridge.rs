use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NotificationResponse {
    status: u16,
    content_type: String,
    body_base64: String,
}

const MAX_CHANNELS: usize = 64;
const MAX_QUIET_HOURS_BYTES: usize = 8 * 1024;

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// Event classes are path segments on the daemon route. The allowlist keeps a
/// caller-supplied class from changing the request path; the daemon still
/// validates the value it receives.
fn valid_event_class(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_status(value: &str) -> bool {
    [
        "PENDING",
        "SENDING",
        "SENT",
        "FAILED",
        "AMBIGUOUS",
        "SUPPRESSED",
    ]
    .contains(&value)
}

fn valid_policy(value: &str) -> bool {
    [
        "ALWAYS",
        "ON_SUCCESS",
        "ON_FAILURE",
        "ON_CONDITION",
        "SILENT",
    ]
    .contains(&value)
}

fn valid_channel(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn read_notification_response(response: LocalOperatorResponse) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut body)
        .map_err(|_| "Local Notification response could not be read".to_owned())?;
    if body.len() > 1024 * 1024 {
        return Err("Local Notification response exceeded its size limit".to_owned());
    }
    Ok(body)
}

#[cfg(test)]
mod request_validation_tests {
    use super::*;

    #[test]
    fn an_event_class_is_one_safe_path_segment() {
        assert!(valid_event_class("TASK_FAILED"));
        assert!(valid_event_class("AUTOMATION_OCCURRENCE_SETTLED"));
        assert!(!valid_event_class(""));
        assert!(!valid_event_class("TASK/FAILED"));
        assert!(!valid_event_class(".."));
        assert!(!valid_event_class("TASK%2FFAILED"));
        assert!(!valid_event_class(&"A".repeat(129)));
    }

    #[test]
    fn a_delivery_status_and_policy_are_the_contracted_tokens() {
        assert!(valid_status("SUPPRESSED"));
        assert!(!valid_status("DELIVERED"));
        assert!(valid_policy("ON_CONDITION"));
        assert!(!valid_policy("SOMETIMES"));
    }

    #[test]
    fn a_channel_display_value_is_bounded_and_control_free() {
        assert!(valid_channel("Primary desktop"));
        assert!(!valid_channel(""));
        assert!(!valid_channel("   "));
        assert!(!valid_channel("line\nbreak"));
        assert!(!valid_channel(&"c".repeat(257)));
    }
}

/// Narrow Notification bridge. The WebView selects from a finite operation set;
/// it cannot supply arbitrary URLs, methods, or headers. Native Operator IPC
/// authenticates its daemon peer, and every request is scoped to the explicitly
/// selected Workspace.
#[tauri::command]
pub(crate) async fn notification_request(
    app: AppHandle,
    workspace_id: String,
    operation: String,
    event_class: Option<String>,
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<u32>,
    policy: Option<String>,
    preferred_channels: Option<Vec<String>>,
    quiet_hours: Option<serde_json::Value>,
    expected_version: Option<u64>,
    request_id: Option<String>,
) -> Result<NotificationResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || event_class
                .as_deref()
                .is_some_and(|value| !valid_event_class(value))
            || status.as_deref().is_some_and(|value| !valid_status(value))
            || cursor
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > 2048)
            || limit.is_some_and(|value| !(1..=200).contains(&value))
            || policy.as_deref().is_some_and(|value| !valid_policy(value))
            || preferred_channels.as_ref().is_some_and(|channels| {
                channels.len() > MAX_CHANNELS
                    || channels.iter().any(|channel| !valid_channel(channel))
            })
            || quiet_hours.as_ref().is_some_and(|value| {
                !value.is_object()
                    || super::bounded_json_bytes(value, MAX_QUIET_HOURS_BYTES).is_err()
            })
            || request_id
                .as_deref()
                .is_some_and(|value| !super::valid_idempotency_key(value))
        {
            return Err("Notification request is invalid".to_owned());
        }

        let (client, _) = operator_client(&app)?;
        let request = match operation.as_str() {
            "list" => {
                let mut request = client
                    .get("/v1/notifications".to_owned())
                    .header("X-Workspace-ID", &workspace_id);
                if let Some(status) = status.as_deref() {
                    request = request.query(&[("status", status)]);
                }
                if let Some(cursor) = cursor.as_deref() {
                    request = request.query(&[("cursor", cursor)]);
                }
                request.query(&[("limit", limit.unwrap_or(50).to_string())])
            }
            "preferences" => client
                .get("/v1/notification-preferences".to_owned())
                .header("X-Workspace-ID", &workspace_id),
            "set-preference" => {
                let event_class = event_class
                    .as_deref()
                    .filter(|value| valid_event_class(value))
                    .ok_or_else(|| "Notification event class is invalid".to_owned())?;
                let policy = policy
                    .as_deref()
                    .filter(|value| valid_policy(value))
                    .ok_or_else(|| "Notification policy is invalid".to_owned())?;
                let preferred_channels = preferred_channels
                    .ok_or_else(|| "Notification channels are missing".to_owned())?;
                let expected_version = expected_version
                    .ok_or_else(|| "Notification preference version is missing".to_owned())?;
                let request_id = request_id
                    .as_deref()
                    .filter(|value| super::valid_idempotency_key(value))
                    .ok_or_else(|| "Notification request identity is invalid".to_owned())?;
                client
                    .put(format!("/v1/notification-preferences/{event_class}"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{expected_version}\""))
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({
                        "policy": policy,
                        "preferred_channels": preferred_channels,
                        "quiet_hours": quiet_hours.unwrap_or(serde_json::Value::Null),
                    }))
            }
            _ => return Err("Notification operation is unavailable".to_owned()),
        };
        let response = request
            .send()
            .map_err(|_| "Local Notification service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response
            .header("content-type")
            .unwrap_or("application/json")
            .to_owned();
        let body = read_notification_response(response)?;
        Ok(NotificationResponse {
            status,
            content_type,
            body_base64: BASE64_STANDARD.encode(body),
        })
    })
    .await
    .map_err(|_| "Notification request did not complete".to_owned())?
}
