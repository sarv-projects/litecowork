use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GoalResponse {
    status: u16,
    content_type: String,
    body_base64: String,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 200
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
fn valid_request_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|byte| byte.is_ascii_graphic())
}
fn read_goal_response(response: LocalOperatorResponse) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    response.take(1024 * 1024 + 1).read_to_end(&mut body)
        .map_err(|_| "Local Goal response could not be read".to_owned())?;
    if body.len() > 1024 * 1024 { return Err("Local Goal response exceeded its size limit".to_owned()); }
    Ok(body)
}

/// Narrow Goal bridge. The WebView selects from a finite operation set; it cannot
/// supply arbitrary URLs, methods, or headers. Native Operator IPC authenticates its
/// daemon peer, and every request is scoped to the explicitly selected Workspace.
#[tauri::command]
pub(crate) async fn goal_request(
    app: AppHandle,
    workspace_id: String,
    operation: String,
    goal_id: Option<String>,
    cursor: Option<String>,
    coworker_id: Option<String>,
    revision: Option<serde_json::Value>,
    status: Option<String>,
    expected_version: Option<u64>,
    request_id: Option<String>,
) -> Result<GoalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || goal_id.as_deref().is_some_and(|value| !valid_id(value))
            || coworker_id.as_deref().is_some_and(|value| !valid_id(value))
            || cursor.as_ref().is_some_and(|value| value.is_empty() || value.len() > 2048)
            || revision.as_ref().is_some_and(|value| super::bounded_json_bytes(value, 64 * 1024).is_err())
            || request_id.as_deref().is_some_and(|value| !valid_request_id(value))
        { return Err("Goal request is invalid".to_owned()); }

        let (client, _) = operator_client(&app)?;
        let request = match operation.as_str() {
            "related-tasks" => {
                let mut request = client.get("/v1/tasks".to_owned())
                    .header("X-Workspace-ID", &workspace_id).query(&[("limit", "100")]);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "related-artifacts" => {
                let mut request = client.get("/v1/artifacts".to_owned())
                    .header("X-Workspace-ID", &workspace_id).query(&[("limit", "100")]);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "list" => {
                let mut request = client.get("/v1/goals".to_owned())
                    .header("X-Workspace-ID", &workspace_id).query(&[("limit", "50")]);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "get" => {
                let id = goal_id.as_deref().ok_or_else(|| "Goal selection is invalid".to_owned())?;
                client.get(format!("/v1/goals/{id}")).header("X-Workspace-ID", &workspace_id)
            }
            "create" => {
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Goal request identity is invalid".to_owned())?;
                let revision = revision.ok_or_else(|| "Goal revision is missing".to_owned())?;
                client.post("/v1/goals".to_owned())
                    .header("X-Workspace-ID", &workspace_id)
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({ "workspace_id": workspace_id, "coworker_id": coworker_id, "revision": revision }))
            }
            "revise" => {
                let id = goal_id.as_deref().ok_or_else(|| "Goal selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Goal request identity is invalid".to_owned())?;
                let expected_version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Goal version is invalid".to_owned())?;
                let revision = revision.ok_or_else(|| "Goal revision is missing".to_owned())?;
                client.post(format!("/v1/goals/{id}/revisions"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{expected_version}\""))
                    .header("Idempotency-Key", request_id).json(&revision)
            }
            "status" => {
                let id = goal_id.as_deref().ok_or_else(|| "Goal selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Goal request identity is invalid".to_owned())?;
                let expected_version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Goal version is invalid".to_owned())?;
                let status = status.as_deref().filter(|value| ["ACTIVE", "PAUSED", "COMPLETED", "ARCHIVED"].contains(value))
                    .ok_or_else(|| "Goal status is invalid".to_owned())?;
                client.post(format!("/v1/goals/{id}/status"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{expected_version}\""))
                    .header("Idempotency-Key", request_id).json(&serde_json::json!({ "status": status }))
            }
            _ => return Err("Goal operation is unavailable".to_owned()),
        };
        let response = request.send().map_err(|_| "Local Goal service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response.header("content-type").unwrap_or("application/json").to_owned();
        let body = read_goal_response(response)?;
        Ok(GoalResponse { status, content_type, body_base64: BASE64_STANDARD.encode(body) })
    }).await.map_err(|_| "Goal request did not complete".to_owned())?
}
