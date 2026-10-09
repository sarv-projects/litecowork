use super::*;
use std::io::Read;

const MAX_CONVERSATION_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn read_json(response: LocalOperatorResponse) -> Result<serde_json::Value, String> {
    let status = response.status();
    let mut bytes = Vec::new();
    response
        .take(MAX_CONVERSATION_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Local Conversation response could not be read".to_owned())?;
    if bytes.len() > MAX_CONVERSATION_RESPONSE_BYTES {
        return Err("Local Conversation response exceeds its size limit".to_owned());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Local Runtime returned an unsupported Conversation response".to_owned())?;
    if !status.is_success() {
        let message = value
            .pointer("/error/message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Local Conversation operation failed");
        return Err(message.to_owned());
    }
    Ok(value)
}

#[tauri::command]
pub(crate) async fn list_conversations(
    app: AppHandle,
    workspace_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id) {
            return Err("Conversation Workspace selection is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let value = read_json(
            client
                .get("/v1/conversations")
                .header("X-Workspace-ID", &workspace_id)
                .query(&[("limit", "50")])
                .send()
                .map_err(|_| "Local Conversations are unavailable".to_owned())?,
        )?;
        let items = value
            .get("items")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "Local Runtime returned an invalid Conversation list".to_owned())?;
        if items.iter().any(|item| {
            item.get("workspace_id").and_then(serde_json::Value::as_str)
                != Some(workspace_id.as_str())
        }) {
            return Err(
                "Local Runtime returned a Conversation outside the selected Workspace".to_owned(),
            );
        }
        Ok(value)
    })
    .await
    .map_err(|_| "Conversation list request did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn create_conversation(
    app: AppHandle,
    workspace_id: String,
    title: Option<String>,
    request_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || request_id.is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
            || title.as_ref().is_some_and(|value| {
                value.trim().is_empty()
                    || value.chars().count() > 160
                    || value.chars().any(char::is_control)
            })
        {
            return Err("Conversation creation request is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let value = read_json(
            client
                .post("/v1/conversations")
                .header("X-Workspace-ID", &workspace_id)
                .header("Idempotency-Key", request_id)
                .json(&serde_json::json!({"workspace_id":workspace_id,"title":title}))
                .send()
                .map_err(|_| "Local Conversation service is unavailable".to_owned())?,
        )?;
        if value
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            != Some(workspace_id.as_str())
            || value
                .get("conversation_id")
                .and_then(serde_json::Value::as_str)
                .is_none()
        {
            return Err(
                "Local Runtime returned a Conversation outside the selected Workspace".to_owned(),
            );
        }
        Ok(value)
    })
    .await
    .map_err(|_| "Conversation creation request did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn get_conversation_presentation(
    app: AppHandle,
    workspace_id: String,
    conversation_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id) || !valid_id(&conversation_id) {
            return Err("Conversation selection is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let value = read_json(
            client
                .get(format!("/v1/conversations/{conversation_id}/presentation"))
                .header("X-Workspace-ID", &workspace_id)
                .query(&[("limit", "50")])
                .send()
                .map_err(|_| "Local Conversation history is unavailable".to_owned())?,
        )?;
        if value
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            != Some(workspace_id.as_str())
            || value
                .get("conversation_id")
                .and_then(serde_json::Value::as_str)
                != Some(conversation_id.as_str())
            || !value.get("items").is_some_and(serde_json::Value::is_array)
        {
            return Err(
                "Local Runtime returned history outside the selected Conversation".to_owned(),
            );
        }
        Ok(value)
    })
    .await
    .map_err(|_| "Conversation history request did not complete".to_owned())?
}
