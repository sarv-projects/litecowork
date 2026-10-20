use super::{AppHandle, operator_client, read_bounded_response};
use serde_json::Value;

const MAX_ID_BYTES: usize = 256;

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_ID_BYTES
        && !value.contains('/')
        && !value.contains('\\')
        && !matches!(value, "." | "..")
        && !value.chars().any(char::is_control)
}

fn encode_path_segment(value: &str) -> String {
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

fn descriptor_path(profile_id: &str, refresh: bool) -> Result<String, String> {
    if !valid_id(profile_id) {
        return Err("Agent profile ID is invalid".to_owned());
    }
    let suffix = if refresh {
        "refresh"
    } else {
        "control-descriptor"
    };
    Ok(format!(
        "/v1/agent-profiles/{}/{suffix}",
        encode_path_segment(profile_id)
    ))
}

fn decode_descriptor(body: &[u8]) -> Result<Value, String> {
    let descriptor: Value = serde_json::from_slice(body)
        .map_err(|_| "Local Operator returned an unsupported AgentControlDescriptor".to_owned())?;
    let object = descriptor.as_object().ok_or_else(|| {
        "Local Operator returned an unsupported AgentControlDescriptor".to_owned()
    })?;
    for key in [
        "descriptor_digest",
        "registry_agent_id",
        "module_id",
        "module_version",
        "installation",
        "auth",
        "configuration",
        "input_surface",
        "native_surface",
        "capabilities",
        "observed_at",
        "expires_at",
    ] {
        if !object.contains_key(key) {
            return Err("Local Operator returned an incomplete AgentControlDescriptor".to_owned());
        }
    }
    Ok(descriptor)
}

fn request_descriptor(
    app: &AppHandle,
    workspace_id: &str,
    profile_id: &str,
    refresh: bool,
) -> Result<Value, String> {
    if !valid_id(workspace_id) {
        return Err("Workspace ID is invalid".to_owned());
    }
    let path = descriptor_path(profile_id, refresh)?;
    let (client, _) = operator_client(app)?;
    let request = if refresh {
        client.post(path)
    } else {
        client.get(path)
    };
    let response = request
        .header("X-Workspace-ID", workspace_id)
        .header("Cache-Control", "no-store")
        .send()
        .map_err(|_| "Local agent control service is unavailable".to_owned())?;
    let body = read_bounded_response(response)?;
    decode_descriptor(&body)
}

#[tauri::command]
pub(super) async fn get_agent_control_descriptor(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        request_descriptor(&app, &workspace_id, &profile_id, false)
    })
    .await
    .map_err(|_| "Agent control descriptor request did not complete".to_owned())?
}

#[tauri::command]
pub(super) async fn refresh_agent_control_descriptor(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        request_descriptor(&app, &workspace_id, &profile_id, true)
    })
    .await
    .map_err(|_| "Agent control descriptor refresh did not complete".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_routes_keep_profile_id_inside_one_path_segment() {
        assert_eq!(
            descriptor_path("profile:1", true).unwrap(),
            "/v1/agent-profiles/profile%3A1/refresh"
        );
        assert!(descriptor_path("profile/with spaces", false).is_err());
        assert!(descriptor_path(r"profile\with-separators", false).is_err());
        assert!(descriptor_path("../foreign", false).is_err());
        assert!(descriptor_path("", false).is_err());
        assert!(descriptor_path(&"x".repeat(MAX_ID_BYTES + 1), false).is_err());
    }

    #[test]
    fn descriptor_response_must_have_required_contract_fields() {
        assert!(decode_descriptor(br#"{"descriptor_digest":"sha256:test"}"#).is_err());
        assert!(decode_descriptor(b"[]").is_err());
        assert!(decode_descriptor(b"not-json").is_err());
    }
}
