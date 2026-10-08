use super::*;

/// Gets the read-only ZIP capability observation. This command never uploads or parses a ZIP.
#[tauri::command]
pub(crate) async fn get_zip_intake_readiness(
    app: AppHandle,
    workspace_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if workspace_id.trim().is_empty()
            || workspace_id.len() > 200
            || !workspace_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("Workspace selection is invalid".to_owned());
        }
        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .get(format!("{base_url}/capabilities/zip-intake"))
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "ZIP-intake status is unavailable from the local Runtime".to_owned())?;
        let body = read_bounded_response(response)?;
        let status: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported ZIP-intake status".to_owned())?;
        if status.get("capability_id").and_then(serde_json::Value::as_str)
            != Some("litecowork.zip-intake")
            || status.get("status").and_then(serde_json::Value::as_str) != Some("UNAVAILABLE")
            || status.get("provider_integrated").and_then(serde_json::Value::as_bool) != Some(false)
            || status.get("extraction_enabled").and_then(serde_json::Value::as_bool) != Some(false)
        {
            return Err("Local Runtime returned an unsupported ZIP-intake status".to_owned());
        }
        Ok(status)
    })
    .await
    .map_err(|_| "ZIP-intake status request did not complete".to_owned())?
}
