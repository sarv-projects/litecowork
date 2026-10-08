use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DelegationProfileWriteResponse {
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
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn read_response(response: LocalOperatorResponse) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut body)
        .map_err(|_| "Worker profile response could not be read".to_owned())?;
    if body.len() > 1024 * 1024 {
        return Err("Worker profile response exceeded its size limit".to_owned());
    }
    Ok(body)
}

/// Narrow bridge for the saved worker-profile catalogue and its user-authored
/// revisions. It has fixed routes and methods; the WebView cannot choose a URL,
/// arbitrary headers, a model override, or an enabled state outside the declared
/// status operation. The daemon remains responsible for rejecting unsafe enablement.
#[tauri::command]
pub(crate) async fn delegation_profile_request(
    app: AppHandle,
    workspace_id: String,
    operation: String,
    profile_id: Option<String>,
    agent_binding_id: Option<String>,
    cursor: Option<String>,
    expected_version: Option<u64>,
    request_id: Option<String>,
    revision: Option<serde_json::Value>,
    name: Option<String>,
    status: Option<String>,
) -> Result<DelegationProfileWriteResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || profile_id.as_deref().is_some_and(|value| !valid_id(value))
            || agent_binding_id.as_deref().is_some_and(|value| !valid_id(value))
            || cursor.as_ref().is_some_and(|value| value.is_empty() || value.len() > 2048)
            || request_id.as_deref().is_some_and(|value| !valid_request_id(value))
            || revision
                .as_ref()
                .is_some_and(|value| super::bounded_json_bytes(value, 64 * 1024).is_err())
            || name.as_ref().is_some_and(|value| value.trim().is_empty() || value.len() > 256)
        {
            return Err("Worker profile request is invalid".to_owned());
        }

        let (client, _) = operator_client(&app)?;
        let request = match operation.as_str() {
            "list" => {
                if profile_id.is_some() || agent_binding_id.is_some() || expected_version.is_some()
                    || request_id.is_some() || revision.is_some() || name.is_some() || status.is_some()
                {
                    return Err("Worker profile request is invalid".to_owned());
                }
                let mut request = client
                    .get("/v1/delegation-profiles".to_owned())
                    .header("X-Workspace-ID", &workspace_id)
                    .header("Cache-Control", "no-store")
                    .query(&[("limit", "200")]);
                if let Some(cursor) = cursor.as_deref() {
                    request = request.query(&[("cursor", cursor)]);
                }
                request
            }
            "create" => {
                if profile_id.is_some() || cursor.is_some() || expected_version.is_some()
                    || name.is_some() || status.is_some()
                {
                    return Err("Worker profile request is invalid".to_owned());
                }
                let binding = agent_binding_id.as_deref().filter(|value| valid_id(value))
                    .ok_or_else(|| "Worker profile agent selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Worker profile request identity is invalid".to_owned())?;
                let revision = revision.ok_or_else(|| "Worker profile revision is missing".to_owned())?;
                client
                    .post("/v1/delegation-profiles".to_owned())
                    .header("X-Workspace-ID", &workspace_id)
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({
                        "workspace_id": workspace_id,
                        "agent_binding_id": binding,
                        "revision": revision,
                    }))
            }
            "revise" => {
                if agent_binding_id.is_some() || cursor.is_some() || name.is_some() || status.is_some() {
                    return Err("Worker profile request is invalid".to_owned());
                }
                let id = profile_id.as_deref().filter(|value| valid_id(value))
                    .ok_or_else(|| "Worker profile selection is invalid".to_owned())?;
                let version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Worker profile version is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Worker profile request identity is invalid".to_owned())?;
                let revision = revision.ok_or_else(|| "Worker profile revision is missing".to_owned())?;
                client
                    .post(format!("/v1/delegation-profiles/{id}/revisions"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{version}\""))
                    .header("Idempotency-Key", request_id)
                    .json(&revision)
            }
            "duplicate" => {
                if agent_binding_id.is_some() || cursor.is_some() || revision.is_some() || status.is_some() {
                    return Err("Worker profile request is invalid".to_owned());
                }
                let id = profile_id.as_deref().filter(|value| valid_id(value))
                    .ok_or_else(|| "Worker profile selection is invalid".to_owned())?;
                let version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Worker profile version is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Worker profile request identity is invalid".to_owned())?;
                let name = name.as_deref().filter(|value| !value.trim().is_empty() && value.len() <= 256)
                    .ok_or_else(|| "Worker profile name is invalid".to_owned())?;
                client
                    .post(format!("/v1/delegation-profiles/{id}/duplicate"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{version}\""))
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({ "name": name }))
            }
            "status" => {
                if agent_binding_id.is_some() || cursor.is_some() || revision.is_some() || name.is_some() {
                    return Err("Worker profile request is invalid".to_owned());
                }
                let id = profile_id.as_deref().filter(|value| valid_id(value))
                    .ok_or_else(|| "Worker profile selection is invalid".to_owned())?;
                let version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Worker profile version is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Worker profile request identity is invalid".to_owned())?;
                let status = status.as_deref().filter(|value| ["ENABLED", "DISABLED", "ARCHIVED"].contains(value))
                    .ok_or_else(|| "Worker profile status is invalid".to_owned())?;
                client
                    .post(format!("/v1/delegation-profiles/{id}/status"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{version}\""))
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({ "status": status }))
            }
            _ => return Err("Worker profile operation is unavailable".to_owned()),
        };
        let response = request
            .send()
            .map_err(|_| "Local worker profile service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response.header("content-type").unwrap_or("application/json").to_owned();
        let body = read_response(response)?;
        Ok(DelegationProfileWriteResponse { status, content_type, body_base64: BASE64_STANDARD.encode(body) })
    })
    .await
    .map_err(|_| "Worker profile request did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn create_delegation_profile(
    app: AppHandle,
    workspace_id: String,
    agent_binding_id: String,
    request_id: String,
    revision: serde_json::Value,
) -> Result<DelegationProfileWriteResponse, String> {
    delegation_profile_request(
        app,
        workspace_id,
        "create".to_owned(),
        None,
        Some(agent_binding_id),
        None,
        None,
        Some(request_id),
        Some(revision),
        None,
        None,
    )
    .await
}

#[tauri::command]
pub(crate) async fn revise_delegation_profile(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
    expected_version: u64,
    request_id: String,
    revision: serde_json::Value,
) -> Result<DelegationProfileWriteResponse, String> {
    delegation_profile_request(
        app,
        workspace_id,
        "revise".to_owned(),
        Some(profile_id),
        None,
        None,
        Some(expected_version),
        Some(request_id),
        Some(revision),
        None,
        None,
    )
    .await
}

#[tauri::command]
pub(crate) async fn duplicate_delegation_profile(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
    expected_version: u64,
    request_id: String,
    name: String,
) -> Result<DelegationProfileWriteResponse, String> {
    delegation_profile_request(
        app,
        workspace_id,
        "duplicate".to_owned(),
        Some(profile_id),
        None,
        None,
        Some(expected_version),
        Some(request_id),
        None,
        Some(name),
        None,
    )
    .await
}

#[tauri::command]
pub(crate) async fn disable_delegation_profile(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
    expected_version: u64,
    request_id: String,
) -> Result<DelegationProfileWriteResponse, String> {
    delegation_profile_request(
        app,
        workspace_id,
        "status".to_owned(),
        Some(profile_id),
        None,
        None,
        Some(expected_version),
        Some(request_id),
        None,
        None,
        Some("DISABLED".to_owned()),
    )
    .await
}

#[tauri::command]
pub(crate) async fn archive_delegation_profile(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
    expected_version: u64,
    request_id: String,
) -> Result<DelegationProfileWriteResponse, String> {
    delegation_profile_request(
        app,
        workspace_id,
        "status".to_owned(),
        Some(profile_id),
        None,
        None,
        Some(expected_version),
        Some(request_id),
        None,
        None,
        Some("ARCHIVED".to_owned()),
    )
    .await
}
