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

fn performance_path(profile_id: &str, task_category: Option<&str>) -> Result<String, String> {
    const CATEGORIES: &[&str] = &[
        "SOFTWARE_ENGINEERING",
        "RESEARCH",
        "WRITING",
        "DATA_ANALYSIS",
        "OFFICE",
        "BROWSER",
        "PERSONAL_ADMIN",
        "OTHER",
    ];
    if !valid_id(profile_id)
        || profile_id == "."
        || profile_id == ".."
        || profile_id.contains('/')
        || profile_id.contains('\\')
        || task_category.is_some_and(|value| !CATEGORIES.contains(&value))
    {
        return Err("Delegation profile performance query is invalid".to_owned());
    }
    let mut path = format!(
        "/v1/delegation-profiles/{}/performance",
        encode_path_segment(profile_id)
    );
    if let Some(category) = task_category {
        path.push_str("?task_category=");
        path.push_str(category);
    }
    Ok(path)
}

#[cfg(test)]
mod performance_path_tests {
    use super::performance_path;

    #[test]
    fn builds_only_the_fixed_profile_performance_path_and_known_categories() {
        assert_eq!(
            performance_path("profile_1", Some("RESEARCH")).unwrap(),
            "/v1/delegation-profiles/profile_1/performance?task_category=RESEARCH"
        );
        assert_eq!(
            performance_path("profile:1", None).unwrap(),
            "/v1/delegation-profiles/profile%3A1/performance"
        );
        assert!(performance_path("profile_1", Some("UNBOUNDED")).is_err());
        assert!(performance_path("../foreign", None).is_err());
    }
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

/// Fixed, non-cacheable profile performance projection. Unknown observations stay null.
#[tauri::command]
pub(crate) async fn get_delegation_profile_performance(
    app: AppHandle,
    workspace_id: String,
    profile_id: String,
    task_category: Option<String>,
) -> Result<DelegationProfileResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id) {
            return Err("Delegation profile performance query is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let path = performance_path(&profile_id, task_category.as_deref())?;
        let request = client
            .get(path)
            .header("X-Workspace-ID", &workspace_id)
            .header("Cache-Control", "no-store");
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
            .map_err(|_| "Delegation profile performance response could not be read".to_owned())?;
        if body.len() > 1024 * 1024 {
            return Err(
                "Delegation profile performance response exceeded its size limit".to_owned(),
            );
        }
        Ok(DelegationProfileResponse {
            status,
            content_type,
            body_base64: BASE64_STANDARD.encode(body),
        })
    })
    .await
    .map_err(|_| "Delegation profile performance query did not complete".to_owned())?
}
