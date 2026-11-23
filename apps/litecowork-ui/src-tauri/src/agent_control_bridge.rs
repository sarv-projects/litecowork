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

fn authentication_path(profile_id: &str, method_id: &str) -> Result<String, String> {
    if !valid_id(profile_id) || !valid_id(method_id) || method_id.len() > 128 {
        return Err("Agent authentication selection is invalid".to_owned());
    }
    Ok(format!(
        "/v1/agent-profiles/{}/auth/{}/begin",
        encode_path_segment(profile_id),
        encode_path_segment(method_id)
    ))
}

fn decode_auth_challenge(body: &[u8], expected_method_id: &str) -> Result<Value, String> {
    let challenge: Value = serde_json::from_slice(body)
        .map_err(|_| "Local Operator returned an unsupported authentication response".to_owned())?;
    let object = challenge.as_object().ok_or_else(|| {
        "Local Operator returned an unsupported authentication response".to_owned()
    })?;
    let kind = object.get("kind").and_then(Value::as_str);
    let method_id = object.get("auth_method_id").and_then(Value::as_str);
    if !matches!(
        kind,
        Some("NATIVE_FLOW_STARTED" | "SECRET_REQUIRED" | "NATIVE_CONFIG_REQUIRED")
    ) || method_id != Some(expected_method_id)
        || object
            .get("expires_at")
            .is_some_and(|value| !value.is_null() && !value.is_string())
    {
        return Err("Local Operator returned an unsupported authentication response".to_owned());
    }
    // Do not pass provider-controlled extra fields, which could contain secret or
    // native-session material, across the Tauri boundary.
    let mut safe = serde_json::Map::new();
    safe.insert("kind".to_owned(), Value::String(kind.unwrap().to_owned()));
    safe.insert(
        "auth_method_id".to_owned(),
        Value::String(expected_method_id.to_owned()),
    );
    if let Some(expires_at) = object.get("expires_at") {
        safe.insert("expires_at".to_owned(), expires_at.clone());
    }
    Ok(Value::Object(safe))
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

fn request_authentication(
    app: &AppHandle,
    workspace_id: &str,
    profile_id: &str,
    method_id: &str,
) -> Result<Value, String> {
    if !valid_id(workspace_id) {
        return Err("Workspace ID is invalid".to_owned());
    }
    let path = authentication_path(profile_id, method_id)?;
    let (client, _) = operator_client(app)?;
    let response = client
        .post(path)
        .header("X-Workspace-ID", workspace_id)
        .header("Cache-Control", "no-store")
        .send()
        .map_err(|_| "Local agent authentication service is unavailable".to_owned())?;
    let body = read_bounded_response(response)?;
    decode_auth_challenge(&body, method_id)
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

#[tauri::command]
pub(super) async fn begin_agent_authentication(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
    auth_method_id: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        request_authentication(&app, &workspace_id, &profile_id, &auth_method_id)
    })
    .await
    .map_err(|_| "Agent authentication request did not complete".to_owned())?
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

    #[test]
    fn authentication_path_encodes_ids_and_rejects_path_input() {
        assert_eq!(
            authentication_path("profile:1", "native-login").unwrap(),
            "/v1/agent-profiles/profile%3A1/auth/native-login/begin"
        );
        assert!(authentication_path("../foreign", "native-login").is_err());
        assert!(authentication_path("profile-1", "../secret").is_err());
        assert!(authentication_path("profile-1", &"x".repeat(129)).is_err());
    }

    #[test]
    fn authentication_response_is_allowlisted_and_bound_to_requested_method() {
        let decoded = decode_auth_challenge(
            br#"{"kind":"NATIVE_FLOW_STARTED","auth_method_id":"native-login","expires_at":null,"native_handle":"private"}"#,
            "native-login",
        )
        .unwrap();
        assert_eq!(decoded.as_object().unwrap().len(), 3);
        assert!(decoded.get("native_handle").is_none());
        assert!(
            decode_auth_challenge(
                br#"{"kind":"NATIVE_FLOW_STARTED","auth_method_id":"other"}"#,
                "native-login"
            )
            .is_err()
        );
        assert!(
            decode_auth_challenge(
                br#"{"kind":"UNKNOWN","auth_method_id":"native-login"}"#,
                "native-login"
            )
            .is_err()
        );
    }
}
