use super::*;

/// Bounded read-only bridge for the authenticated Task presentation snapshot.
/// The WebView still validates each untrusted item before selecting a renderer.
#[tauri::command]
pub(crate) async fn get_task_presentation(
    app: AppHandle,
    workspace_id: String,
    task_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid = |value: &str| {
            !value.trim().is_empty()
                && value.len() <= 200
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        };
        if !valid(&workspace_id) || !valid(&task_id) {
            return Err("Task presentation selection is invalid".to_owned());
        }

        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .get(format!("{base_url}/tasks/{task_id}/presentation"))
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "Local Task presentation is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let snapshot: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported Task presentation".to_owned())?;
        if snapshot.get("workspace_id").and_then(serde_json::Value::as_str)
            != Some(workspace_id.as_str())
            || snapshot.get("task_id").and_then(serde_json::Value::as_str)
                != Some(task_id.as_str())
            || !snapshot.get("items").is_some_and(serde_json::Value::is_array)
        {
            return Err("Local Runtime returned a presentation outside the selected Task".to_owned());
        }
        Ok(snapshot)
    })
    .await
    .map_err(|_| "Task presentation request did not complete".to_owned())?
}

/// Fetches the same authenticated, bounded Task source snapshot through its factual
/// progress projection. No liveness or provider signal is synthesized by this bridge.
#[tauri::command]
pub(crate) async fn get_task_progress(
    app: AppHandle,
    workspace_id: String,
    task_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let valid = |value: &str| {
            !value.trim().is_empty()
                && value.len() <= 200
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        };
        if !valid(&workspace_id) || !valid(&task_id) {
            return Err("Task progress selection is invalid".to_owned());
        }

        let (client, workspace_url) = operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .get(format!("{base_url}/tasks/{task_id}/progress"))
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "Local Task progress is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let projection: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported Task progress".to_owned())?;
        if projection.get("task_id").and_then(serde_json::Value::as_str)
            != Some(task_id.as_str())
            || !projection.get("computed_at").is_some_and(serde_json::Value::is_string)
            || !projection.get("active_workstreams").is_some_and(serde_json::Value::is_array)
            || !projection.get("blockers").is_some_and(serde_json::Value::is_array)
        {
            return Err("Local Runtime returned progress outside the selected Task".to_owned());
        }
        Ok(projection)
    })
    .await
    .map_err(|_| "Task progress request did not complete".to_owned())?
}
