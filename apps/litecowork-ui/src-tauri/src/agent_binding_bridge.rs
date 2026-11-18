use super::{AppHandle, operator_client, read_bounded_response};
use serde_json::Value;

fn valid_segment(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !matches!(value, "." | "..")
        && !value.contains('/')
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
}

fn encode_segment(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[(byte >> 4) as usize]));
            encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

fn native_config_path(profile_id: &str, target_id: &str) -> Result<String, String> {
    if !valid_segment(profile_id, 256) || !valid_segment(target_id, 128) {
        return Err("Native configuration selection is invalid".to_owned());
    }
    Ok(format!(
        "/v1/agent-profiles/{}/native-config/{}/open",
        encode_segment(profile_id),
        encode_segment(target_id)
    ))
}

fn decode_open_receipt(
    body: &[u8],
    expected_profile_id: &str,
    expected_target_id: &str,
) -> Result<Value, String> {
    let receipt: Value = serde_json::from_slice(body)
        .map_err(|_| "Local Operator returned an unsupported native configuration receipt".to_owned())?;
    let object = receipt.as_object().ok_or_else(|| {
        "Local Operator returned an unsupported native configuration receipt".to_owned()
    })?;
    if object.get("agent_profile_id").and_then(Value::as_str) != Some(expected_profile_id)
        || object.get("target_id").and_then(Value::as_str) != Some(expected_target_id)
        || object.get("opened").and_then(Value::as_bool) != Some(true)
    {
        return Err("Local Operator returned an unsupported native configuration receipt".to_owned());
    }
    Ok(serde_json::json!({
        "agent_profile_id": expected_profile_id,
        "target_id": expected_target_id,
        "opened": true
    }))
}

fn request_native_config(
    app: &AppHandle,
    workspace_id: &str,
    profile_id: &str,
    target_id: &str,
) -> Result<Value, String> {
    if !valid_segment(workspace_id, 256) {
        return Err("Workspace ID is invalid".to_owned());
    }
    let path = native_config_path(profile_id, target_id)?;
    let (client, _) = operator_client(app)?;
    let response = client
        .post(path)
        .header("X-Workspace-ID", workspace_id)
        .header("Cache-Control", "no-store")
        .send()
        .map_err(|_| "Local agent configuration service is unavailable".to_owned())?;
    let body = read_bounded_response(response)?;
    decode_open_receipt(&body, profile_id, target_id)
}

#[tauri::command]
pub(super) async fn open_agent_native_configuration(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
    target_id: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        request_native_config(&app, &workspace_id, &profile_id, &target_id)
    })
    .await
    .map_err(|_| "Native configuration request did not complete".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_configuration_path_keeps_ids_in_single_segments() {
        assert_eq!(
            native_config_path("profile:1", "settings panel").unwrap(),
            "/v1/agent-profiles/profile%3A1/native-config/settings%20panel/open"
        );
        assert!(native_config_path("../foreign", "settings").is_err());
        assert!(native_config_path("profile", r"C:\\secret").is_err());
        assert!(native_config_path("profile", &"x".repeat(129)).is_err());
    }

    #[test]
    fn native_configuration_receipt_is_bound_to_the_requested_target() {
        let receipt = decode_open_receipt(
            br#"{"agent_profile_id":"profile-1","target_id":"settings","opened":true,"native_path":"private"}"#,
            "profile-1",
            "settings",
        )
        .unwrap();
        assert_eq!(receipt.as_object().unwrap().len(), 3);
        assert!(receipt.get("native_path").is_none());
        assert!(decode_open_receipt(
            br#"{"agent_profile_id":"other","target_id":"settings","opened":true}"#,
            "profile-1",
            "settings"
        )
        .is_err());
    }
}
