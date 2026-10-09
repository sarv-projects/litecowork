use super::*;
use std::io::Read;

const MAX_CONVERSATION_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_rich_presentation_response(
    value: &serde_json::Value,
    workspace_id: &str,
    conversation_id: &str,
    message_id: &str,
    presentation_id: &str,
    semantic_content_digest: &str,
) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 10
        && object.contains_key("workspace_id")
        && object.contains_key("conversation_id")
        && object.contains_key("message_id")
        && object.contains_key("presentation_id")
        && object.contains_key("schema_version")
        && object.contains_key("renderer_contract_version")
        && object.contains_key("semantic_content_digest")
        && object.contains_key("document_digest")
        && object.contains_key("document_size_bytes")
        && object.contains_key("document")
        && value
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            == Some(workspace_id)
        && value
            .get("conversation_id")
            .and_then(serde_json::Value::as_str)
            == Some(conversation_id)
        && value.get("message_id").and_then(serde_json::Value::as_str) == Some(message_id)
        && value
            .get("presentation_id")
            .and_then(serde_json::Value::as_str)
            == Some(presentation_id)
        && value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            == Some(1)
        && value
            .get("renderer_contract_version")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|version| version >= 1)
        && value
            .get("semantic_content_digest")
            .and_then(serde_json::Value::as_str)
            == Some(semantic_content_digest)
        && value
            .get("document_digest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(valid_digest)
        && value
            .get("document_size_bytes")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|size| (1..=1_048_576).contains(&size))
        && value
            .get("document")
            .is_some_and(serde_json::Value::is_object)
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
    if !(200..300).contains(&status) {
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
    cursor: Option<String>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !valid_id(&conversation_id)
            || cursor.as_ref().is_some_and(|value| !valid_id(value))
        {
            return Err("Conversation selection is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let mut request = client
            .get(format!("/v1/conversations/{conversation_id}/presentation"))
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("limit", "50")]);
        if let Some(cursor) = cursor {
            request = request.query(&[("cursor", cursor)]);
        }
        let value = read_json(
            request
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

#[tauri::command]
pub(crate) async fn get_rich_presentation(
    app: AppHandle,
    workspace_id: String,
    conversation_id: String,
    message_id: String,
    presentation_id: String,
    semantic_content_digest: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !valid_id(&conversation_id)
            || !valid_id(&message_id)
            || !valid_id(&presentation_id)
            || !valid_digest(&semantic_content_digest)
        {
            return Err("Rich presentation selection is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let value = read_json(
            client
                .get(format!("/v1/rich-presentations/{presentation_id}"))
                .header("X-Workspace-ID", &workspace_id)
                .send()
                .map_err(|_| "Local RichPresentation is unavailable".to_owned())?,
        )?;
        if !valid_rich_presentation_response(
            &value,
            &workspace_id,
            &conversation_id,
            &message_id,
            &presentation_id,
            &semantic_content_digest,
        ) {
            return Err(
                "Local Runtime returned a RichPresentation outside the selected Message".to_owned(),
            );
        }
        Ok(value)
    })
    .await
    .map_err(|_| "RichPresentation request did not complete".to_owned())?
}

#[cfg(test)]
mod rich_presentation_response_tests {
    use super::*;

    fn response() -> serde_json::Value {
        serde_json::json!({
            "workspace_id":"workspace-1", "conversation_id":"conversation-1", "message_id":"message-1",
            "presentation_id":"presentation-1", "schema_version":1, "renderer_contract_version":1,
            "semantic_content_digest":format!("sha256:{}", "a".repeat(64)),
            "document_digest":format!("sha256:{}", "b".repeat(64)), "document_size_bytes":128,
            "document":{"schema_version":1}
        })
    }

    #[test]
    fn rich_presentation_response_is_bound_to_selected_records_and_closed() {
        let expected_digest = format!("sha256:{}", "a".repeat(64));
        let value = response();
        assert!(valid_rich_presentation_response(
            &value,
            "workspace-1",
            "conversation-1",
            "message-1",
            "presentation-1",
            &expected_digest
        ));
        let mut foreign = value.clone();
        foreign["workspace_id"] = serde_json::json!("workspace-2");
        assert!(!valid_rich_presentation_response(
            &foreign,
            "workspace-1",
            "conversation-1",
            "message-1",
            "presentation-1",
            &expected_digest
        ));
        let mut unexpected = value;
        unexpected["agent_session_id"] = serde_json::json!("private");
        assert!(!valid_rich_presentation_response(
            &unexpected,
            "workspace-1",
            "conversation-1",
            "message-1",
            "presentation-1",
            &expected_digest
        ));
    }

    #[test]
    fn rich_presentation_route_ids_and_digests_are_path_safe() {
        assert!(valid_id("presentation_1-2"));
        assert!(!valid_id("../other"));
        assert!(!valid_id("presentation/other"));
        assert!(valid_digest(&format!("sha256:{}", "0".repeat(64))));
        assert!(!valid_digest(&format!("sha256:{}", "A".repeat(64))));
    }
}
