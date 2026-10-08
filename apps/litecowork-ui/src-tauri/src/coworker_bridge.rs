use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoworkerResponse {
    status: u16,
    content_type: String,
    body_base64: String,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

/// Narrow Coworker command bridge. The WebView selects a typed operation; it cannot
/// choose an arbitrary URL, header, or HTTP method. Native Operator IPC authenticates
/// the daemon peer, and every request carries the selected Workspace scope.
#[tauri::command]
pub(crate) async fn coworker_request(
    app: AppHandle,
    workspace_id: String,
    operation: String,
    coworker_id: Option<String>,
    cursor: Option<String>,
    revision: Option<serde_json::Value>,
    revision_number: Option<u64>,
    status: Option<String>,
    primary_coworker_id: Option<String>,
    expected_version: Option<u64>,
    request_id: Option<String>,
) -> Result<CoworkerResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || coworker_id.as_deref().is_some_and(|id| !valid_id(id))
            || primary_coworker_id.as_deref().is_some_and(|id| !valid_id(id))
            || cursor.as_ref().is_some_and(|value| value.is_empty() || value.len() > 2048)
            || revision.as_ref().is_some_and(|value| super::bounded_json_bytes(value, 64 * 1024).is_err())
            || revision_number.is_some_and(|value| value == 0)
            || request_id.as_deref().is_some_and(|value| !valid_request_id(value))
        {
            return Err("Coworker request is invalid".to_owned());
        }

        let (client, _) = operator_client(&app)?;
        let mut request = match operation.as_str() {
            "list" => {
                let mut request = client.get("/v1/coworkers".to_owned())
                    .header("X-Workspace-ID", &workspace_id)
                    .query(&[("limit", "50")]);
                if let Some(cursor) = cursor.as_deref() {
                    request = request.query(&[("cursor", cursor)]);
                }
                request
            }
            "get" | "presence" | "get_revision" => {
                let id = coworker_id.as_deref().ok_or_else(|| "Coworker selection is invalid".to_owned())?;
                let path = match operation.as_str() {
                    "get" => format!("/v1/coworkers/{id}"),
                    "presence" => format!("/v1/coworkers/{id}/presence"),
                    "get_revision" => {
                        let revision = revision_number.filter(|value| *value > 0)
                            .ok_or_else(|| "Coworker revision selection is invalid".to_owned())?;
                        format!("/v1/coworkers/{id}/revisions/{revision}")
                    }
                    _ => unreachable!(),
                };
                client.get(path)
                    .header("X-Workspace-ID", &workspace_id)
            }
            "create" => {
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Coworker request identity is invalid".to_owned())?;
                let revision = revision.ok_or_else(|| "Coworker revision is missing".to_owned())?;
                client.post("/v1/coworkers".to_owned())
                    .header("X-Workspace-ID", &workspace_id)
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({ "workspace_id": workspace_id, "revision": revision }))
            }
            "revise" => {
                let id = coworker_id.as_deref().ok_or_else(|| "Coworker selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Coworker request identity is invalid".to_owned())?;
                let expected_version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Coworker version is invalid".to_owned())?;
                let revision = revision.ok_or_else(|| "Coworker revision is missing".to_owned())?;
                client.post(format!("/v1/coworkers/{id}/revisions"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{expected_version}\""))
                    .header("Idempotency-Key", request_id)
                    .json(&revision)
            }
            "status" => {
                let id = coworker_id.as_deref().ok_or_else(|| "Coworker selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Coworker request identity is invalid".to_owned())?;
                let expected_version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Coworker version is invalid".to_owned())?;
                let status = status.as_deref().filter(|value| ["ACTIVE", "PAUSED", "ARCHIVED"].contains(value))
                    .ok_or_else(|| "Coworker status is invalid".to_owned())?;
                client.post(format!("/v1/coworkers/{id}/status"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{expected_version}\""))
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({ "status": status }))
            }
            "primary" => {
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Coworker request identity is invalid".to_owned())?;
                let expected_version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Workspace version is invalid".to_owned())?;
                client.post(format!("/v1/workspaces/{workspace_id}/primary-coworker"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{expected_version}\""))
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({ "coworker_id": primary_coworker_id }))
            }
            _ => return Err("Coworker operation is unavailable".to_owned()),
        };

        let response = request.send().map_err(|_| "Local Coworker service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response.header("content-type").unwrap_or("application/json").to_owned();
        let body = read_bounded_response_preserving_status(response)?;
        Ok(CoworkerResponse { status, content_type, body_base64: BASE64_STANDARD.encode(body) })
    })
    .await
    .map_err(|_| "Coworker request did not complete".to_owned())?
}

/// CoworkerApiError needs the Operator status and structured code to preserve its
/// user-safe conflict/authorization messages, so unlike the ordinary JSON helper this
/// bounded reader returns non-2xx bodies without exposing them as text.
fn read_bounded_response_preserving_status(response: LocalOperatorResponse) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut body)
        .map_err(|_| "Local Coworker response could not be read".to_owned())?;
    if body.len() > 1024 * 1024 {
        return Err("Local Coworker response exceeded its size limit".to_owned());
    }
    Ok(body)
}
