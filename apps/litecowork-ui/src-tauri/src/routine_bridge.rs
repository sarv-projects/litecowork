use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoutineResponse {
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
fn read_routine_response(response: LocalOperatorResponse) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    response.take(1024 * 1024 + 1).read_to_end(&mut body)
        .map_err(|_| "Local Routine response could not be read".to_owned())?;
    if body.len() > 1024 * 1024 { return Err("Local Routine response exceeded its size limit".to_owned()); }
    Ok(body)
}

/// Exposes only bounded saved-Routine CRUD. It deliberately has no run or trigger command.
#[tauri::command]
pub(crate) async fn routine_request(
    app: AppHandle,
    workspace_id: String,
    operation: String,
    routine_id: Option<String>,
    cursor: Option<String>,
    expected_version: Option<u64>,
    request_id: Option<String>,
    body: Option<String>,
) -> Result<RoutineResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || routine_id.as_deref().is_some_and(|value| !valid_id(value))
            || cursor.as_ref().is_some_and(|value| value.is_empty() || value.len() > 2048)
            || request_id.as_deref().is_some_and(|value| !valid_request_id(value))
            || body.as_ref().is_some_and(|value| value.len() > 128 * 1024)
        { return Err("Routine request is invalid".to_owned()); }

        let (client, _) = operator_client(&app)?;
        let mut request = match operation.as_str() {
            "list" if routine_id.is_none() => {
                if expected_version.is_some() || request_id.is_some() || body.is_some() { return Err("Routine request is invalid".to_owned()); }
                let mut request = client.get("/v1/routines".to_owned())
                    .header("X-Workspace-ID", &workspace_id).query(&[("limit", "50")]);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "get" if cursor.is_none() => {
                if expected_version.is_some() || request_id.is_some() || body.is_some() { return Err("Routine request is invalid".to_owned()); }
                let id = routine_id.as_deref().ok_or_else(|| "Routine selection is invalid".to_owned())?;
                client.get(format!("/v1/routines/{id}")).header("X-Workspace-ID", &workspace_id)
            }
            "revisions" if cursor.as_ref().map_or(true, |value| !value.is_empty()) => {
                if expected_version.is_some() || request_id.is_some() || body.is_some() { return Err("Routine request is invalid".to_owned()); }
                let id = routine_id.as_deref().ok_or_else(|| "Routine selection is invalid".to_owned())?;
                let mut request = client.get(format!("/v1/routines/{id}/revisions")).header("X-Workspace-ID", &workspace_id);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "create" => {
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Routine request identity is invalid".to_owned())?;
                let body = body.as_deref().ok_or_else(|| "Routine definition is missing".to_owned())?;
                if routine_id.is_some() || expected_version.is_some() || cursor.is_some() { return Err("Routine request is invalid".to_owned()); }
                client.post("/v1/routines".to_owned()).header("X-Workspace-ID", &workspace_id)
                    .header("Idempotency-Key", request_id).header("Content-Type", "application/json").body(body.to_owned())
            }
            "revise" => {
                let id = routine_id.as_deref().ok_or_else(|| "Routine selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Routine request identity is invalid".to_owned())?;
                let version = expected_version.filter(|value| *value > 0).ok_or_else(|| "Routine version is invalid".to_owned())?;
                let body = body.as_deref().ok_or_else(|| "Routine revision is missing".to_owned())?;
                if cursor.is_some() { return Err("Routine request is invalid".to_owned()); }
                client.post(format!("/v1/routines/{id}/revisions"))
                    .header("X-Workspace-ID", &workspace_id).header("If-Match", format!("\"{version}\""))
                    .header("Idempotency-Key", request_id).header("Content-Type", "application/json").body(body.to_owned())
            }
            "archive" => {
                let id = routine_id.as_deref().ok_or_else(|| "Routine selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Routine request identity is invalid".to_owned())?;
                let version = expected_version.filter(|value| *value > 0).ok_or_else(|| "Routine version is invalid".to_owned())?;
                if cursor.is_some() || body.is_some() { return Err("Routine request is invalid".to_owned()); }
                client.post(format!("/v1/routines/{id}/archive"))
                    .header("X-Workspace-ID", &workspace_id).header("If-Match", format!("\"{version}\""))
                    .header("Idempotency-Key", request_id)
            }
            _ => return Err("Routine operation is unavailable".to_owned()),
        };
        request = request.header("Cache-Control", "no-store");
        let response = request.send().map_err(|_| "Local Routine service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response.header("content-type").unwrap_or("application/json").to_owned();
        let body = read_routine_response(response)?;
        Ok(RoutineResponse { status, content_type, body_base64: BASE64_STANDARD.encode(body) })
    }).await.map_err(|_| "Routine request did not complete".to_owned())?
}
