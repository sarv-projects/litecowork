use super::*;
use std::{collections::HashSet, path::Path};

const MAX_METADATA_BYTES: usize = 1024 * 1024;
const MAX_REVISION_PAGES: usize = 100;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourceSaveAsResult {
    status: &'static str,
}

/// Saves the exact current managed file Resource through an owner-authorized local read.
/// Bytes and the native destination stay in this process; only SAVED/CANCELLED crosses IPC.
#[tauri::command]
pub(crate) async fn resource_save_as(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
    expected_resource_revision_id: String,
    expected_content_digest: String,
    expected_size_bytes: u64,
    expected_media_type: String,
) -> Result<ResourceSaveAsResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !valid_id(&resource_id)
            || !valid_id(&expected_resource_revision_id)
            || !valid_digest(&expected_content_digest)
            || expected_size_bytes > artifact_bridge::MAX_LOCAL_SAVE_BYTES
            || !valid_media_type(&expected_media_type)
        {
            return Err("Resource selection is invalid or exceeds the 10 MiB Save As limit".to_owned());
        }

        let (client, _) = operator_client(&app)?;
        let detail = get_json(&client, &format!("/v1/resources/{resource_id}"), &workspace_id,
            "Resource metadata is unavailable")?;
        validate_detail(
            &detail,
            &workspace_id,
            &resource_id,
            &expected_resource_revision_id,
        )?;
        verify_current_revision(
            &client,
            &workspace_id,
            &resource_id,
            &expected_resource_revision_id,
            &expected_content_digest,
            expected_size_bytes,
            &expected_media_type,
        )?;

        let display_name = detail.get("display_name")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Resource filename is unavailable".to_owned())?;
        let safe_name = safe_resource_name(display_name, &resource_id);
        let Some(destination) = rfd::FileDialog::new().set_file_name(&safe_name).save_file() else {
            return Ok(ResourceSaveAsResult { status: "CANCELLED" });
        };

        // The Operator rechecks authorization, current-head identity, ContextDocument
        // status and BlobRef integrity at the storage boundary. Pin the read to the
        // selection so a changed head is a conflict rather than an implicit substitution.
        let response = client
            .get(format!("/v1/resources/{resource_id}/content"))
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("revision_id", expected_resource_revision_id.as_str())])
            .send()
            .map_err(|_| "Resource content is unavailable from the local Runtime".to_owned())?;
        if !response.is_success() {
            return Err(content_error(response));
        }
        if response.header("x-resource-media-type") != Some(expected_media_type.as_str()) {
            return Err("Resource media type changed or could not be verified".to_owned());
        }
        if response.content_length() != Some(expected_size_bytes) {
            return Err("Resource size changed or could not be verified".to_owned());
        }
        let mut bytes = Vec::with_capacity(expected_size_bytes as usize);
        response
            .take(expected_size_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| "Resource content could not be read".to_owned())?;
        if bytes.len() as u64 != expected_size_bytes {
            return Err("Resource size changed or could not be verified".to_owned());
        }

        artifact_bridge::write_export_atomically(Path::new(&destination), &bytes)?;
        Ok(ResourceSaveAsResult { status: "SAVED" })
    })
    .await
    .map_err(|_| "Resource Save As did not complete".to_owned())?
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_media_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.is_ascii()
        && !value.bytes().any(|byte| byte.is_ascii_control())
        && value.contains('/')
}

fn get_json(
    client: &LocalOperatorClient,
    path: &str,
    workspace_id: &str,
    unavailable: &str,
) -> Result<serde_json::Value, String> {
    let response = client.get(path)
        .header("X-Workspace-ID", workspace_id)
        .send()
        .map_err(|_| unavailable.to_owned())?;
    if !response.is_success() { return Err(unavailable.to_owned()); }
    let mut body = Vec::new();
    response.take(MAX_METADATA_BYTES as u64 + 1).read_to_end(&mut body)
        .map_err(|_| unavailable.to_owned())?;
    if body.len() > MAX_METADATA_BYTES { return Err("Resource metadata exceeds the desktop read limit".to_owned()); }
    serde_json::from_slice(&body).map_err(|_| "Local Runtime returned unsupported Resource metadata".to_owned())
}

fn validate_detail(
    detail: &serde_json::Value,
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
) -> Result<(), String> {
    if detail.get("resource_id").and_then(serde_json::Value::as_str) != Some(resource_id)
        || detail.get("workspace_id").and_then(serde_json::Value::as_str) != Some(workspace_id)
    {
        return Err("Resource belongs to a different Workspace".to_owned());
    }
    if detail.get("kind").and_then(serde_json::Value::as_str) != Some("FILE") {
        return Err("Save As is available only for managed file Resources".to_owned());
    }
    if detail.pointer("/provider_identity/provider_instance_id")
        .and_then(serde_json::Value::as_str) != Some("litecowork.local-upload")
    {
        return Err("Only locally managed Resource content can be saved from this Library".to_owned());
    }
    if detail.get("current_revision_id").and_then(serde_json::Value::as_str) != Some(revision_id) {
        return Err("Resource changed since it was selected. Reload the Library before saving".to_owned());
    }
    if let Some(context_document) = detail.get("context_document").filter(|value| !value.is_null()) {
        match context_document.get("status").and_then(serde_json::Value::as_str) {
            Some("ACTIVE") => {}
            Some("REVOKED") => return Err("This ContextDocument was revoked; its content cannot be saved until it is restored".to_owned()),
            Some("DELETION_PENDING") => return Err("This ContextDocument is being deleted; its content cannot be saved".to_owned()),
            Some("DELETED") => return Err("This ContextDocument has been deleted; its content cannot be saved".to_owned()),
            _ => return Err("ContextDocument status could not be verified; reload the Library".to_owned()),
        }
    }
    Ok(())
}

fn verify_current_revision(
    client: &LocalOperatorClient,
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
    digest: &str,
    size: u64,
    media_type: &str,
) -> Result<(), String> {
    let mut cursor: Option<String> = None;
    let mut seen = HashSet::new();
    for _ in 0..MAX_REVISION_PAGES {
        let mut request = client.get(format!("/v1/resources/{resource_id}/revisions"))
            .header("X-Workspace-ID", workspace_id)
            .query(&[("limit", "100")]);
        if let Some(value) = cursor.as_deref() { request = request.query(&[("cursor", value)]); }
        let response = request.send().map_err(|_| "Resource history is unavailable".to_owned())?;
        if !response.is_success() { return Err("Resource history is unavailable".to_owned()); }
        let mut body = Vec::new();
        response.take(MAX_METADATA_BYTES as u64 + 1).read_to_end(&mut body)
            .map_err(|_| "Resource history could not be read".to_owned())?;
        if body.len() > MAX_METADATA_BYTES { return Err("Resource history exceeds the desktop read limit".to_owned()); }
        let page: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned unsupported Resource history".to_owned())?;
        let items = page.get("items").and_then(serde_json::Value::as_array)
            .filter(|items| items.len() <= 100)
            .ok_or_else(|| "Resource history is invalid".to_owned())?;
        for item in items {
            let revision = item.get("revision").ok_or_else(|| "Resource history record is invalid".to_owned())?;
            if revision.get("resource_id").and_then(serde_json::Value::as_str) != Some(resource_id) {
                return Err("Resource history belongs to a different Resource".to_owned());
            }
            if revision.get("resource_revision_id").and_then(serde_json::Value::as_str) == Some(revision_id) {
                if item.get("is_head").and_then(serde_json::Value::as_bool) != Some(true) {
                    return Err("Only the current Resource version can be saved from the Library".to_owned());
                }
                if revision.get("content_digest").and_then(serde_json::Value::as_str) != Some(digest)
                    || revision.get("size_bytes").and_then(serde_json::Value::as_u64) != Some(size)
                    || revision.get("media_type").and_then(serde_json::Value::as_str) != Some(media_type)
                {
                    return Err("Resource version metadata changed since it was selected".to_owned());
                }
                return Ok(());
            }
        }
        let next = page.get("next_cursor").and_then(serde_json::Value::as_str).map(str::to_owned);
        match next {
            Some(value) if !value.is_empty() && value.len() <= 4096 && seen.insert(value.clone()) => cursor = Some(value),
            Some(_) => return Err("Resource history pagination is invalid".to_owned()),
            None => return Err("Selected Resource version is no longer available".to_owned()),
        }
    }
    Err("Resource history is beyond the bounded desktop lookup".to_owned())
}

fn content_error(mut response: LocalOperatorResponse) -> String {
    let status = response.status();
    if status == 409 {
        let mut body = Vec::new();
        if response.take(16 * 1024 + 1).read_to_end(&mut body).is_ok() && body.len() <= 16 * 1024 {
            if let Ok(error) = serde_json::from_slice::<serde_json::Value>(&body) {
                if error.get("code").and_then(serde_json::Value::as_str) == Some("CONTEXT_DOCUMENT_NOT_ACTIVE") {
                    return match error.get("message").and_then(serde_json::Value::as_str) {
                        Some(message) if message.contains("was revoked") => "This ContextDocument was revoked; its retained content cannot be saved until it is restored".to_owned(),
                        Some(message) if message.contains("being deleted") => "This ContextDocument is being deleted; its content cannot be saved".to_owned(),
                        Some(message) if message.contains("has been deleted") => "This ContextDocument has been deleted; its content cannot be saved".to_owned(),
                        _ => "This ContextDocument is no longer active; its content cannot be saved".to_owned(),
                    };
                }
            }
        }
    }
    match status {
        404 => "Resource content is no longer available".to_owned(),
        409 => "Resource changed or its ContextDocument is no longer active; reload the Library before saving".to_owned(),
        413 => "Resource exceeds the 10 MiB desktop Save As limit".to_owned(),
        503 => "Resource content is temporarily unavailable from this Runtime".to_owned(),
        500 => "Resource content failed integrity verification and was not saved".to_owned(),
        401 | 403 => "Workspace authorization for Resource content was denied".to_owned(),
        _ => "Resource content could not be saved from this Runtime".to_owned(),
    }
}

fn safe_resource_name(display_name: &str, resource_id: &str) -> String {
    let leaf = display_name.rsplit(['/', '\\']).next().unwrap_or_default();
    let mut safe: String = leaf.chars().filter(|character| {
        !character.is_control() && !matches!(character, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }).collect();
    safe = safe.trim().trim_end_matches('.').to_owned();
    if safe.is_empty() || safe == "." || safe == ".." {
        return format!("resource-{resource_id}");
    }
    if safe.len() > 240 {
        let mut boundary = 240;
        while !safe.is_char_boundary(boundary) { boundary -= 1; }
        safe.truncate(boundary);
    }
    safe
}
