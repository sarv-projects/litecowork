use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionActionResponse {
    status: u16,
    body_base64: String,
}

/// Read-only Suggestions list bridge. It accepts a finite visibility/cursor set and
/// always calls the authenticated local Operator; no WebView-supplied URL is accepted.
#[tauri::command]
pub(crate) async fn list_suggestions(
    app: AppHandle,
    workspace_id: String,
    visibility: Option<String>,
    cursor: Option<String>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid_id = |value: &str| {
            !value.trim().is_empty() && value.len() <= 256
                && !value.chars().any(char::is_control)
        };
        if !valid_id(&workspace_id)
            || visibility.as_deref().is_some_and(|value| !["VISIBLE", "SNOOZED", "ALL"].contains(&value))
            || cursor.as_ref().is_some_and(|value| value.is_empty() || value.len() > 2048)
        { return Err("Suggestion list request is invalid".to_owned()); }

        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let mut request = client.get(format!("{base_url}/suggestions"))
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("limit", "50"), ("visibility", visibility.as_deref().unwrap_or("VISIBLE"))]);
        if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
        let response = request.send().map_err(|_| "Local Suggestions are unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported Suggestions page".to_owned())?;
        let items = page.get("items").and_then(serde_json::Value::as_array)
            .ok_or_else(|| "Local Runtime returned an invalid Suggestions page".to_owned())?;
        if items.iter().any(|item| item.get("workspace_id").and_then(serde_json::Value::as_str) != Some(workspace_id.as_str())) {
            return Err("Local Runtime returned a Suggestion outside the selected Workspace".to_owned());
        }
        Ok(page)
    }).await.map_err(|_| "Suggestion list request did not complete".to_owned())?
}

/// Reads all kind settings, including virtual unmuted defaults. The WebView cannot
/// select an arbitrary Operator URL or a Workspace outside this explicit request.
#[tauri::command]
pub(crate) async fn list_suggestion_preferences(
    app: AppHandle,
    workspace_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid_id = |value: &str| !value.trim().is_empty() && value.len() <= 256
            && !value.chars().any(char::is_control);
        if !valid_id(&workspace_id) { return Err("Suggestion preference request is invalid".to_owned()); }
        let (client, _) = operator_client(&app)?;
        let response = client.get(format!("/v1/workspaces/{workspace_id}/suggestion-preferences"))
            .header("X-Workspace-ID", &workspace_id)
            .send().map_err(|_| "Local Suggestion settings are unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned invalid Suggestion settings".to_owned())?;
        let items = page.get("items").and_then(serde_json::Value::as_array)
            .ok_or_else(|| "Local Runtime returned invalid Suggestion settings".to_owned())?;
        if items.iter().any(|item| item.get("workspace_id").and_then(serde_json::Value::as_str) != Some(workspace_id.as_str())) {
            return Err("Local Runtime returned settings outside the selected Workspace".to_owned());
        }
        Ok(page)
    }).await.map_err(|_| "Suggestion preference request did not complete".to_owned())?
}

/// Applies the Workspace owner's versioned kind-mute preference. Muting and dismissal
/// of currently proposed items are committed by the Operator as one transaction.
#[tauri::command]
pub(crate) async fn set_suggestion_preference(
    app: AppHandle,
    workspace_id: String,
    kind: String,
    muted: bool,
    expected_version: u64,
    request_id: String,
) -> Result<SuggestionActionResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid_id = |value: &str| !value.trim().is_empty() && value.len() <= 256
            && !value.chars().any(char::is_control);
        let valid_request_id = |value: &str| !value.is_empty() && value.len() <= 128
            && value.bytes().all(|byte| byte.is_ascii_graphic());
        if !valid_id(&workspace_id)
            || !["TASK_OPPORTUNITY", "ROUTINE_OPPORTUNITY", "AUTOMATION_OPPORTUNITY"].contains(&kind.as_str())
            || !valid_request_id(&request_id)
        { return Err("Suggestion preference request is invalid".to_owned()); }
        let (client, _) = operator_client(&app)?;
        let response = client.put(format!("/v1/workspaces/{workspace_id}/suggestion-preferences/{kind}"))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", format!("\"{expected_version}\""))
            .header("Idempotency-Key", &request_id)
            .json(&serde_json::json!({ "muted": muted }))
            .send().map_err(|_| "Local Suggestion settings are unavailable".to_owned())?;
        let status = response.status().as_u16();
        let body = read_bounded_response(response)?;
        Ok(SuggestionActionResponse { status, body_base64: BASE64_STANDARD.encode(body) })
    }).await.map_err(|_| "Suggestion preference update did not complete".to_owned())?
}

/// Only the documented owner actions are accepted. The WebView cannot select a URL,
/// method, arbitrary headers, or another Workspace's authority.
#[tauri::command]
pub(crate) async fn suggestion_owner_action(
    app: AppHandle,
    workspace_id: String,
    suggestion_id: String,
    operation: String,
    expected_version: u64,
    request_id: String,
    snoozed_until: Option<String>,
) -> Result<SuggestionActionResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid_id = |value: &str| !value.trim().is_empty() && value.len() <= 256
            && !value.chars().any(char::is_control);
        let valid_request_id = |value: &str| !value.is_empty() && value.len() <= 128
            && value.bytes().all(|byte| byte.is_ascii_graphic());
        if !valid_id(&workspace_id) || !valid_id(&suggestion_id) || expected_version == 0
            || !valid_request_id(&request_id)
            || !["dismiss", "snooze", "unsnooze"].contains(&operation.as_str())
            || (operation == "snooze" && snoozed_until.is_none())
            || (operation != "snooze" && snoozed_until.is_some())
        { return Err("Suggestion action request is invalid".to_owned()); }

        let (client, _) = operator_client(&app)?;
        let request = match operation.as_str() {
            "dismiss" => client.post(format!("/v1/suggestions/{suggestion_id}/resolve"))
                .header("X-Workspace-ID", &workspace_id)
                .header("If-Match", format!("\"{expected_version}\""))
                .header("Idempotency-Key", &request_id)
                .json(&serde_json::json!({ "resolution": "DISMISSED" })),
            "snooze" | "unsnooze" => client.post(format!("/v1/suggestions/{suggestion_id}/snooze"))
                .header("X-Workspace-ID", &workspace_id)
                .header("If-Match", format!("\"{expected_version}\""))
                .header("Idempotency-Key", &request_id)
                .json(&serde_json::json!({ "snoozed_until": snoozed_until })),
            _ => unreachable!(),
        };
        let response = request.send().map_err(|_| "Local Suggestion service is unavailable".to_owned())?;
        let status = response.status().as_u16();
        let body = read_bounded_response(response)?;
        Ok(SuggestionActionResponse { status, body_base64: BASE64_STANDARD.encode(body) })
    }).await.map_err(|_| "Suggestion action request did not complete".to_owned())?
}

/// Accepts an actionable Suggestion through the authenticated Operator API. The
/// returned Task remains READY; this bridge never starts a planner or agent.
#[tauri::command]
pub(crate) async fn accept_suggestion_task(
    app: AppHandle,
    workspace_id: String,
    suggestion_id: String,
    expected_version: u64,
    request_id: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid_id = |value: &str| !value.trim().is_empty() && value.len() <= 256
            && !value.chars().any(char::is_control);
        let valid_request_id = |value: &str| !value.is_empty() && value.len() <= 128
            && value.bytes().all(|byte| byte.is_ascii_graphic());
        if !valid_id(&workspace_id) || !valid_id(&suggestion_id) || expected_version == 0
            || !valid_request_id(&request_id)
        { return Err("Suggestion acceptance request is invalid".to_owned()); }

        let (client, _) = operator_client(&app)?;
        let response = client.post(format!("/v1/suggestions/{suggestion_id}/accept-task"))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", format!("\"{expected_version}\""))
            .header("Idempotency-Key", &request_id)
            .json(&serde_json::json!({}))
            .send()
            .map_err(|_| "Local Suggestion service is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let view: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an invalid accepted Task".to_owned())?;
        let task = view.get("task").ok_or_else(|| "Local Runtime returned an invalid accepted Task".to_owned())?;
        if task.get("workspace_id").and_then(serde_json::Value::as_str) != Some(workspace_id.as_str()) {
            return Err("Accepted Task belongs to a different Workspace".to_owned());
        }
        task.get("task_id").and_then(serde_json::Value::as_str)
            .filter(|value| valid_id(value))
            .map(str::to_owned)
            .ok_or_else(|| "Local Runtime returned an invalid accepted Task identity".to_owned())
    }).await.map_err(|_| "Suggestion acceptance did not complete".to_owned())?
}
