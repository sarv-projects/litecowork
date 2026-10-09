use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AutomationResponse {
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
fn read_automation_response(response: LocalOperatorResponse) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    response.take(1024 * 1024 + 1).read_to_end(&mut body)
        .map_err(|_| "Local Automation response could not be read".to_owned())?;
    if body.len() > 1024 * 1024 { return Err("Local Automation response exceeded its size limit".to_owned()); }
    Ok(body)
}

/// Finite Workspace-scoped Automation/Routine/Coworker bridge. Definition writes remain
/// paused; the explicit owner ManualTrigger operation may atomically create one ordinary
/// READY Task but never starts Task planning or agent execution. This bridge cannot host
/// recurring triggers.
#[tauri::command]
pub(crate) async fn automation_request(
    app: AppHandle,
    workspace_id: String,
    operation: String,
    automation_id: Option<String>,
    routine_id: Option<String>,
    coworker_id: Option<String>,
    cursor: Option<String>,
    expected_version: Option<u64>,
    request_id: Option<String>,
    name: Option<String>,
    routine_revision: Option<u64>,
    automation_revision: Option<u64>,
    inputs: Option<serde_json::Value>,
    triggers: Option<Vec<serde_json::Value>>,
    execution_policy: Option<serde_json::Value>,
    coworker_ref: serde_json::Value,
) -> Result<AutomationResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || automation_id.as_deref().is_some_and(|value| !valid_id(value))
            || routine_id.as_deref().is_some_and(|value| !valid_id(value))
            || coworker_id.as_deref().is_some_and(|value| !valid_id(value))
            || cursor.as_ref().is_some_and(|value| value.is_empty() || value.len() > 2048)
            || request_id.as_deref().is_some_and(|value| !valid_request_id(value))
            || name.as_ref().is_some_and(|value| value.trim().is_empty() || value.chars().count() > 120)
            || routine_revision.is_some_and(|value| value == 0 || value > 9_007_199_254_740_991)
            || automation_revision.is_some_and(|value| value == 0 || value > 9_007_199_254_740_991)
            || inputs.as_ref().is_some_and(|value| !value.is_object() || super::bounded_json_bytes(value, 128 * 1024).is_err())
            || triggers.as_ref().is_some_and(|value| value.is_empty() || value.len() > 10 || value.iter().any(|item| !item.is_object()))
            || execution_policy.as_ref().is_some_and(|value| !value.is_object() || super::bounded_json_bytes(value, 32 * 1024).is_err())
            || super::bounded_json_bytes(&coworker_ref, 4096).is_err()
        { return Err("Automation request is invalid".to_owned()); }

        let (client, _) = operator_client(&app)?;
        let request = match operation.as_str() {
            "list" => {
                let mut request = client.get("/v1/automations".to_owned())
                    .header("X-Workspace-ID", &workspace_id).query(&[("limit", "50")]);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "get" => {
                let id = automation_id.as_deref().ok_or_else(|| "Automation selection is invalid".to_owned())?;
                client.get(format!("/v1/automations/{id}")).header("X-Workspace-ID", &workspace_id)
            }
            "list_automation_revisions" => {
                let id = automation_id.as_deref().ok_or_else(|| "Automation selection is invalid".to_owned())?;
                let mut request = client.get(format!("/v1/automations/{id}/revisions")).header("X-Workspace-ID", &workspace_id);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "list_routines" => {
                let mut request = client.get("/v1/routines".to_owned()).header("X-Workspace-ID", &workspace_id).query(&[("limit", "50")]);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "get_routine" => {
                let id = routine_id.as_deref().ok_or_else(|| "Routine selection is invalid".to_owned())?;
                client.get(format!("/v1/routines/{id}")).header("X-Workspace-ID", &workspace_id)
            }
            "list_routine_revisions" => {
                let id = routine_id.as_deref().ok_or_else(|| "Routine selection is invalid".to_owned())?;
                let mut request = client.get(format!("/v1/routines/{id}/revisions")).header("X-Workspace-ID", &workspace_id);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "list_coworkers" => {
                let mut request = client.get("/v1/coworkers".to_owned()).header("X-Workspace-ID", &workspace_id).query(&[("limit", "50")]);
                if let Some(cursor) = cursor.as_deref() { request = request.query(&[("cursor", cursor)]); }
                request
            }
            "get_coworker" => {
                let id = coworker_id.as_deref().ok_or_else(|| "Coworker selection is invalid".to_owned())?;
                client.get(format!("/v1/coworkers/{id}")).header("X-Workspace-ID", &workspace_id)
            }
            "create" => {
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Automation request identity is invalid".to_owned())?;
                let name = name.as_deref().filter(|value| !value.trim().is_empty() && value.chars().count() <= 120)
                    .ok_or_else(|| "Automation name is invalid".to_owned())?;
                let routine_id = routine_id.as_deref().filter(|value| valid_id(value))
                    .ok_or_else(|| "Routine selection is invalid".to_owned())?;
                let routine_revision = routine_revision.filter(|value| *value > 0 && *value <= 9_007_199_254_740_991)
                    .ok_or_else(|| "Routine revision is invalid".to_owned())?;
                let triggers = triggers.filter(|value| !value.is_empty() && value.len() <= 10)
                    .ok_or_else(|| "Automation trigger set is invalid".to_owned())?;
                let policy = execution_policy.filter(serde_json::Value::is_object)
                    .ok_or_else(|| "Automation execution policy is invalid".to_owned())?;
                validate_coworker_ref(&coworker_ref)?;
                client.post("/v1/automations".to_owned())
                    .header("X-Workspace-ID", &workspace_id).header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({ "workspace_id": workspace_id, "name": name, "routine_id": routine_id,
                        "routine_revision": routine_revision, "triggers": triggers, "execution_policy": policy,
                        "coworker_ref": coworker_ref }))
            }
            "revise" => {
                let id = automation_id.as_deref().ok_or_else(|| "Automation selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Automation request identity is invalid".to_owned())?;
                let version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Automation version is invalid".to_owned())?;
                let name = name.as_deref().filter(|value| !value.trim().is_empty() && value.chars().count() <= 120)
                    .ok_or_else(|| "Automation name is invalid".to_owned())?;
                let routine_id = routine_id.as_deref().filter(|value| valid_id(value))
                    .ok_or_else(|| "Routine selection is invalid".to_owned())?;
                let routine_revision = routine_revision.filter(|value| *value > 0 && *value <= 9_007_199_254_740_991)
                    .ok_or_else(|| "Routine revision is invalid".to_owned())?;
                let triggers = triggers.filter(|value| !value.is_empty() && value.len() <= 10)
                    .ok_or_else(|| "Automation trigger set is invalid".to_owned())?;
                let policy = execution_policy.filter(serde_json::Value::is_object)
                    .ok_or_else(|| "Automation execution policy is invalid".to_owned())?;
                validate_coworker_ref(&coworker_ref)?;
                client.patch(format!("/v1/automations/{id}"))
                    .header("X-Workspace-ID", &workspace_id).header("If-Match", format!("\"{version}\""))
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({ "name": name, "routine_id": routine_id,
                        "routine_revision": routine_revision, "triggers": triggers, "execution_policy": policy,
                        "coworker_ref": coworker_ref }))
            }
            "pause" | "disable" => {
                let id = automation_id.as_deref().ok_or_else(|| "Automation selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Automation request identity is invalid".to_owned())?;
                let expected_version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Automation version is invalid".to_owned())?;
                client.post(format!("/v1/automations/{id}/{operation}"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{expected_version}\""))
                    .header("Idempotency-Key", request_id)
            }
            "run" => {
                let id = automation_id.as_deref().ok_or_else(|| "Automation selection is invalid".to_owned())?;
                let request_id = request_id.as_deref().filter(|value| valid_request_id(value))
                    .ok_or_else(|| "Automation request identity is invalid".to_owned())?;
                let version = expected_version.filter(|value| *value > 0)
                    .ok_or_else(|| "Automation version is invalid".to_owned())?;
                let revision = automation_revision.filter(|value| *value > 0 && *value <= 9_007_199_254_740_991)
                    .ok_or_else(|| "Automation revision is invalid".to_owned())?;
                let inputs = inputs.filter(serde_json::Value::is_object)
                    .ok_or_else(|| "Manual Automation inputs are invalid".to_owned())?;
                client.post(format!("/v1/automations/{id}/run"))
                    .header("X-Workspace-ID", &workspace_id)
                    .header("If-Match", format!("\"{version}\""))
                    .header("Idempotency-Key", request_id)
                    .json(&serde_json::json!({ "automation_revision": revision, "inputs": inputs }))
            }
            _ => return Err("Automation operation is unavailable".to_owned()),
        };
        let response = request.send().map_err(|_| "Local Automation service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response.header("content-type").unwrap_or("application/json").to_owned();
        let body = read_automation_response(response)?;
        Ok(AutomationResponse { status, content_type, body_base64: BASE64_STANDARD.encode(body) })
    }).await.map_err(|_| "Automation request did not complete".to_owned())?
}

fn validate_coworker_ref(value: &serde_json::Value) -> Result<(), String> {
    if value.is_null() { return Ok(()); }
    let object = value.as_object().ok_or_else(|| "Automation Coworker pin is invalid".to_owned())?;
    let coworker_id = object.get("coworker_id").and_then(serde_json::Value::as_str);
    let revision = object.get("revision").and_then(serde_json::Value::as_u64);
    if object.len() != 2
        || !coworker_id.is_some_and(valid_id)
        || !revision.is_some_and(|value| value > 0 && value <= 9_007_199_254_740_991)
    { return Err("Automation Coworker pin is invalid".to_owned()); }
    Ok(())
}
