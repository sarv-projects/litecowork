use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DelegationProfileResponse {
    status: u16,
    content_type: String,
    body_base64: String,
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
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

/// Finite desktop bridge for the read-only Workspace worker-profile catalogue.
/// The renderer cannot select an arbitrary Operator URL or HTTP method.
#[tauri::command]
pub(crate) async fn list_delegation_profiles(
    app: AppHandle,
    workspace_id: String,
    cursor: Option<String>,
) -> Result<DelegationProfileResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || cursor
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > 2048)
        {
            return Err("Delegation profile query is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let mut request = client
            .get("/v1/delegation-profiles".to_owned())
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("limit", "200")]);
        if let Some(cursor) = cursor.as_deref() {
            request = request.query(&[("cursor", cursor)]);
        }
        let response = request
            .send()
            .map_err(|_| "Local delegation profile service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response
            .header("content-type")
            .unwrap_or("application/json")
            .to_owned();
        let mut body = Vec::new();
        response
            .take(1024 * 1024 + 1)
            .read_to_end(&mut body)
            .map_err(|_| "Delegation profile response could not be read".to_owned())?;
        if body.len() > 1024 * 1024 {
            return Err("Delegation profile response exceeded its size limit".to_owned());
        }
        Ok(DelegationProfileResponse {
            status,
            content_type,
            body_base64: BASE64_STANDARD.encode(body),
        })
    })
    .await
    .map_err(|_| "Delegation profile query did not complete".to_owned())?
}

/// Fixed read path for immutable revisions of one Workspace-owned worker profile.
#[tauri::command]
pub(crate) async fn list_delegation_profile_revisions(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
    cursor: Option<String>,
) -> Result<DelegationProfileResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !valid_id(&profile_id)
            || cursor
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > 2048)
        {
            return Err("Delegation profile revision query is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let mut request = client
            .get(format!(
                "/v1/delegation-profiles/{}/revisions",
                encode_path_segment(&profile_id)
            ))
            .header("X-Workspace-ID", &workspace_id)
            .header("Cache-Control", "no-store")
            .query(&[("limit", "200")]);
        if let Some(cursor) = cursor.as_deref() {
            request = request.query(&[("cursor", cursor)]);
        }
        let response = request
            .send()
            .map_err(|_| "Local delegation profile service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response
            .header("content-type")
            .unwrap_or("application/json")
            .to_owned();
        let mut body = Vec::new();
        response
            .take(1024 * 1024 + 1)
            .read_to_end(&mut body)
            .map_err(|_| "Delegation profile revision response could not be read".to_owned())?;
        if body.len() > 1024 * 1024 {
            return Err("Delegation profile revision response exceeded its size limit".to_owned());
        }
        Ok(DelegationProfileResponse {
            status,
            content_type,
            body_base64: BASE64_STANDARD.encode(body),
        })
    })
    .await
    .map_err(|_| "Delegation profile revision query did not complete".to_owned())?
}
