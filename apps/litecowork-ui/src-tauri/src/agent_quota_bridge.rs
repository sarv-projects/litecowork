use super::{AppHandle, operator_client, read_bounded_response};
use serde_json::Value;

const MAX_ID_BYTES: usize = 256;

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_ID_BYTES
        && !value.chars().any(char::is_control)
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
}

fn observation_path(binding_id: &str) -> Result<String, String> {
    if !valid_id(binding_id) {
        return Err("Agent binding ID is invalid".to_owned());
    }
    Ok(format!(
        "/v1/agent-bindings/{}/quota-observation",
        encode_path_segment(binding_id)
    ))
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn decode_observation(body: &[u8], expected_binding_id: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| "Local Operator returned an invalid quota observation".to_owned())?;
    if value.is_null() {
        return Ok(Value::Null);
    }
    let object = value
        .as_object()
        .ok_or_else(|| "Local Operator returned an invalid quota observation".to_owned())?;
    if object.get("agent_binding_id").and_then(Value::as_str) != Some(expected_binding_id) {
        return Err("Local Operator returned quota data for a different agent binding".to_owned());
    }
    Ok(value)
}

fn request_observation(
    app: &AppHandle,
    workspace_id: &str,
    binding_id: &str,
) -> Result<Value, String> {
    if !valid_id(workspace_id) {
        return Err("Workspace ID is invalid".to_owned());
    }
    let path = observation_path(binding_id)?;
    let (client, _) = operator_client(app)?;
    let response = client
        .get(path)
        .header("X-Workspace-ID", workspace_id)
        .header("Cache-Control", "no-store")
        .send()
        .map_err(|_| "Local agent quota observation service is unavailable".to_owned())?;
    let body = read_bounded_response(response)?;
    decode_observation(&body, binding_id)
}

#[tauri::command]
pub(crate) async fn get_agent_quota_observation(
    app: AppHandle,
    workspace_id: String,
    agent_binding_id: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        request_observation(&app, &workspace_id, &agent_binding_id)
    })
    .await
    .map_err(|_| "Agent quota observation request did not complete".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_path_is_a_fixed_binding_scoped_get_target() {
        assert_eq!(
            observation_path("binding-1").unwrap(),
            "/v1/agent-bindings/binding-1/quota-observation"
        );
        assert!(observation_path("../other").is_err());
        assert!(observation_path("binding/other").is_err());
        assert!(observation_path("").is_err());
    }

    #[test]
    fn quota_response_allows_explicit_null_and_rejects_cross_binding_data() {
        assert_eq!(
            decode_observation(b"null", "binding-1").unwrap(),
            Value::Null
        );
        assert_eq!(
            decode_observation(
                br#"{"agent_binding_id":"binding-1","state":"UNKNOWN"}"#,
                "binding-1"
            )
            .unwrap()["state"],
            "UNKNOWN"
        );
        assert!(decode_observation(br#"{"agent_binding_id":"binding-2"}"#, "binding-1").is_err());
        assert!(decode_observation(b"[]", "binding-1").is_err());
    }
}
