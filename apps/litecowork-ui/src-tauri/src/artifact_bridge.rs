use super::*;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtifactReadResponse {
    status: u16,
    content_type: String,
    body_base64: String,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 160 && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[tauri::command]
pub(crate) async fn artifact_library_command(
    app: AppHandle, workspace_id: String, artifact_id: String, action: String,
    expected_version: u64, idempotency_key: String,
) -> Result<ArtifactReadResponse, String> {
    if !valid_id(&workspace_id) || !valid_id(&artifact_id)
        || !matches!(action.as_str(), "promote" | "archive")
        || expected_version == 0 || expected_version > 9_007_199_254_740_991
        || idempotency_key.is_empty() || idempotency_key.len() > 128
        || !idempotency_key.bytes().all(|byte| byte.is_ascii_graphic())
    { return Err("Artifact Library request is invalid".to_owned()); }
    tauri::async_runtime::spawn_blocking(move || {
        let (client, _) = operator_client(&app)?;
        let response = client.post(format!("/v1/artifacts/{artifact_id}/{action}"))
            .header("X-Workspace-ID", workspace_id)
            .header("If-Match", format!("\"{expected_version}\""))
            .header("Idempotency-Key", idempotency_key)
            .send().map_err(|_| "Local Artifact Library service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response.header("content-type").unwrap_or("application/json").to_owned();
        let mut bytes = Vec::new();
        response.take(MAX_ARTIFACT_METADATA_BYTES as u64 + 1).read_to_end(&mut bytes)
            .map_err(|_| "Artifact Library response could not be read".to_owned())?;
        if bytes.len() > MAX_ARTIFACT_METADATA_BYTES { return Err("Artifact Library response exceeds its size limit".to_owned()); }
        Ok(ArtifactReadResponse { status, content_type, body_base64: BASE64_STANDARD.encode(bytes) })
    }).await.map_err(|_| "Artifact Library request did not complete".to_owned())?
}

pub(crate) const MAX_LOCAL_SAVE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_ARTIFACT_METADATA_BYTES: usize = 1024 * 1024;
const MAX_RESOURCE_REVISION_PAGES: usize = 100;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtifactSaveAsResult {
    status: &'static str,
}

/// Fetches one exact managed ArtifactVersion through the authenticated local Operator,
/// validates its immutable ResourceRevision and byte metadata, then writes it from the
/// native process. File bytes never cross Tauri's WebView command boundary.
#[tauri::command]
pub(crate) async fn artifact_save_as(
    app: AppHandle,
    workspace_id: String,
    artifact_id: String,
    version: u64,
    expected_resource_revision_id: String,
    expected_content_digest: String,
    expected_size_bytes: u64,
    expected_media_type: String,
) -> Result<ArtifactSaveAsResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !valid_id(&artifact_id)
            || version == 0
            || version > 9_007_199_254_740_991
            || !valid_id(&expected_resource_revision_id)
            || !valid_sha256_digest(&expected_content_digest)
            || expected_size_bytes > MAX_LOCAL_SAVE_BYTES
            || !valid_media_type(&expected_media_type)
        {
            return Err("Artifact selection is invalid".to_owned());
        }

        let (client, _) = operator_client(&app)?;
        let artifact_path = format!("/v1/artifacts/{artifact_id}");
        let artifact = get_json_bounded(&client, &artifact_path, &workspace_id)?;
        let artifact_id_from_service = json_string(&artifact, "artifact_id")?;
        let artifact_workspace = json_string(&artifact, "workspace_id")?;
        let resource_id = json_string(&artifact, "resource_id")?;
        let display_name = json_string(&artifact, "display_name")?;
        let current_version = json_u64(&artifact, "current_version")?;
        if artifact_id_from_service != artifact_id
            || artifact_workspace != workspace_id
            || !valid_id(resource_id)
            || current_version < version
        {
            return Err("Artifact identity or Workspace does not match the selected version".to_owned());
        }

        let version_path = format!("/v1/artifacts/{artifact_id}/versions/{version}");
        let artifact_version = get_json_bounded(&client, &version_path, &workspace_id)?;
        let version_artifact_id = json_string(&artifact_version, "artifact_id")?;
        let returned_version = json_u64(&artifact_version, "version")?;
        let resource_revision_id = json_string(&artifact_version, "resource_revision_id")?;
        if version_artifact_id != artifact_id
            || returned_version != version
            || !valid_id(resource_revision_id)
            || resource_revision_id != expected_resource_revision_id
        {
            return Err("Artifact version or ResourceRevision identity does not match".to_owned());
        }

        let content = artifact_version
            .get("content")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| "Artifact version content metadata is invalid".to_owned())?;
        if content.get("kind").and_then(serde_json::Value::as_str) != Some("MANAGED_BLOB") {
            return Err("Only managed Artifact versions can be saved from this desktop".to_owned());
        }
        let content_digest = content
            .get("content_digest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Artifact content digest is unavailable".to_owned())?;
        let content_size = content
            .get("size_bytes")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "Artifact content size is invalid".to_owned())?;
        let media_type = content
            .get("media_type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Artifact media type is invalid".to_owned())?;
        if !valid_sha256_digest(content_digest)
            || content_size > MAX_LOCAL_SAVE_BYTES
            || !valid_media_type(media_type)
            || content_digest != expected_content_digest
            || content_size != expected_size_bytes
            || media_type != expected_media_type
        {
            return Err("Artifact content is outside the supported Save As limits".to_owned());
        }
        let storage_ref = content
            .get("storage_ref")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| "Artifact blob metadata is invalid".to_owned())?;
        if storage_ref.get("digest").and_then(serde_json::Value::as_str) != Some(content_digest)
            || storage_ref.get("size_bytes").and_then(serde_json::Value::as_u64) != Some(content_size)
            || storage_ref.get("media_type").and_then(serde_json::Value::as_str) != Some(media_type)
        {
            return Err("Artifact blob metadata does not match its version".to_owned());
        }

        verify_artifact_resource_revision(
            &client,
            &workspace_id,
            resource_id,
            resource_revision_id,
            content_digest,
            content_size,
            media_type,
        )?;

        let safe_name = safe_export_name(display_name, &artifact_id, version);
        let Some(destination) = rfd::FileDialog::new()
            .set_file_name(&safe_name)
            .save_file()
        else {
            return Ok(ArtifactSaveAsResult { status: "CANCELLED" });
        };

        // The Operator's ArtifactReadStore resolves this exact ArtifactVersion and
        // ResourceRevision, then verifies the stored BlobRef digest and size before it
        // returns bytes. The desktop independently checks the response headers and
        // exact byte count against the metadata above before writing anything.
        let content_path = format!("{version_path}/content");
        let response = client
            .get(content_path)
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "Artifact content is unavailable from the local Runtime".to_owned())?;
        if !response.is_success() {
            return Err("Artifact content is unavailable from the local Runtime".to_owned());
        }
        let response_media_type = response.header("content-type").map(str::to_owned);
        let response_content_length = response.content_length();
        let mut bytes = Vec::with_capacity(content_size as usize);
        response
            .take(content_size.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| "Artifact content could not be read".to_owned())?;
        verify_artifact_download(
            response_media_type.as_deref(),
            response_content_length,
            media_type,
            content_size,
            content_digest,
            &bytes,
        )?;

        write_export_atomically(&destination, &bytes)?;
        Ok(ArtifactSaveAsResult { status: "SAVED" })
    })
    .await
    .map_err(|_| "Artifact Save As did not complete".to_owned())?
}

fn get_json_bounded(
    client: &LocalOperatorClient,
    path: &str,
    workspace_id: &str,
) -> Result<serde_json::Value, String> {
    let response = client
        .get(path)
        .header("X-Workspace-ID", workspace_id)
        .send()
        .map_err(|_| "Artifact metadata is unavailable from the local Runtime".to_owned())?;
    if !response.is_success() {
        return Err("Artifact metadata is unavailable from the local Runtime".to_owned());
    }
    let mut body = Vec::new();
    response
        .take(MAX_ARTIFACT_METADATA_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|_| "Artifact metadata could not be read".to_owned())?;
    if body.len() > MAX_ARTIFACT_METADATA_BYTES {
        return Err("Artifact metadata exceeds the desktop read limit".to_owned());
    }
    serde_json::from_slice(&body).map_err(|_| "Artifact metadata has an unsupported shape".to_owned())
}

fn verify_artifact_resource_revision(
    client: &LocalOperatorClient,
    workspace_id: &str,
    resource_id: &str,
    resource_revision_id: &str,
    content_digest: &str,
    content_size: u64,
    media_type: &str,
) -> Result<(), String> {
    let mut cursor: Option<String> = None;
    let mut seen_cursors = std::collections::HashSet::new();
    for _ in 0..MAX_RESOURCE_REVISION_PAGES {
        let mut request = client
            .get(format!("/v1/resources/{resource_id}/revisions"))
            .header("X-Workspace-ID", workspace_id)
            .query(&[("limit", "100")]);
        if let Some(value) = cursor.as_deref() {
            request = request.query(&[("cursor", value)]);
        }
        let response = request
            .send()
            .map_err(|_| "Artifact ResourceRevision is unavailable".to_owned())?;
        if !response.is_success() {
            return Err("Artifact ResourceRevision is unavailable".to_owned());
        }
        let mut body = Vec::new();
        response
            .take(MAX_ARTIFACT_METADATA_BYTES as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| "Artifact ResourceRevision could not be read".to_owned())?;
        if body.len() > MAX_ARTIFACT_METADATA_BYTES {
            return Err("Artifact ResourceRevision page exceeds the desktop read limit".to_owned());
        }
        let page: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| "Artifact ResourceRevision page has an unsupported shape".to_owned())?;
        let items = page
            .get("items")
            .and_then(serde_json::Value::as_array)
            .filter(|items| items.len() <= 100)
            .ok_or_else(|| "Artifact ResourceRevision page is invalid".to_owned())?;
        for item in items {
            let revision = item
                .get("revision")
                .and_then(serde_json::Value::as_object)
                .ok_or_else(|| "Artifact ResourceRevision record is invalid".to_owned())?;
            let found_resource_id = revision
                .get("resource_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "Artifact ResourceRevision identity is invalid".to_owned())?;
            let found_revision_id = revision
                .get("resource_revision_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "Artifact ResourceRevision identity is invalid".to_owned())?;
            if found_resource_id != resource_id {
                return Err("Artifact ResourceRevision belongs to a different Resource".to_owned());
            }
            if found_revision_id == resource_revision_id {
                if revision.get("content_digest").and_then(serde_json::Value::as_str)
                    != Some(content_digest)
                    || revision.get("size_bytes").and_then(serde_json::Value::as_u64)
                        != Some(content_size)
                    || revision.get("media_type").and_then(serde_json::Value::as_str)
                        != Some(media_type)
                {
                    return Err("Artifact ResourceRevision metadata does not match its version".to_owned());
                }
                return Ok(());
            }
        }
        let next = page
            .get("next_cursor")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        match next {
            Some(value)
                if !value.is_empty()
                    && value.len() <= 4096
                    && seen_cursors.insert(value.clone()) => cursor = Some(value),
            Some(_) => return Err("Artifact ResourceRevision pagination is invalid".to_owned()),
            None => return Err("Artifact ResourceRevision does not exist in this Workspace".to_owned()),
        }
    }
    Err("Artifact ResourceRevision is beyond the bounded desktop history lookup".to_owned())
}

fn json_string<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Artifact {key} is invalid"))
}

fn json_u64(value: &serde_json::Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("Artifact {key} is invalid"))
}

fn valid_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_media_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.is_ascii()
        && !value.bytes().any(|byte| byte.is_ascii_control())
        && value.contains('/')
}

fn verify_artifact_download(
    response_media_type: Option<&str>,
    response_content_length: Option<u64>,
    expected_media_type: &str,
    expected_size: u64,
    expected_digest: &str,
    bytes: &[u8],
) -> Result<(), String> {
    if response_media_type.is_none_or(|value| !value.eq_ignore_ascii_case(expected_media_type)) {
        return Err("Downloaded Artifact media type does not match its immutable version".to_owned());
    }
    if response_content_length != Some(expected_size) || bytes.len() as u64 != expected_size {
        return Err("Downloaded Artifact size does not match its immutable version".to_owned());
    }
    use sha2::{Digest, Sha256};
    let actual_digest = format!("sha256:{:x}", Sha256::digest(bytes));
    if actual_digest != expected_digest {
        return Err("Downloaded Artifact digest does not match its immutable version".to_owned());
    }
    Ok(())
}

fn safe_export_name(display_name: &str, artifact_id: &str, version: u64) -> String {
    let leaf = display_name.rsplit(['/', '\\']).next().unwrap_or_default();
    let mut safe: String = leaf
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(character, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
        .collect();
    safe = safe.trim().trim_end_matches('.').to_owned();
    if safe.is_empty() || safe == "." || safe == ".." {
        return format!("artifact-{artifact_id}-v{version}");
    }
    let (stem, extension) = safe.rsplit_once('.').filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map(|(stem, extension)| (stem.to_owned(), format!(".{extension}")))
        .unwrap_or((safe, String::new()));
    let suffix = format!("-v{version}");
    let maximum_stem_bytes = 240_usize.saturating_sub(suffix.len()).saturating_sub(extension.len());
    let mut stem = stem;
    if stem.len() > maximum_stem_bytes {
        let mut boundary = maximum_stem_bytes;
        while !stem.is_char_boundary(boundary) {
            boundary -= 1;
        }
        stem.truncate(boundary);
    }
    if stem.is_empty() {
        format!("artifact-{artifact_id}{suffix}{extension}")
    } else {
        format!("{stem}{suffix}{extension}")
    }
}

pub(crate) fn write_export_atomically(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "The selected save location is unavailable".to_owned())?;
    if destination.file_name().is_none() {
        return Err("The selected save location is invalid".to_owned());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "The selected save location is unavailable".to_owned())?
        .as_nanos();
    let mut temporary = None;
    for attempt in 0..8_u8 {
        let path = parent.join(format!(
            ".litecowork-export-{}-{nonce}-{attempt}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if file.write_all(bytes).and_then(|_| file.sync_all()).is_err() {
                    let _ = fs::remove_file(&path);
                    return Err("File could not be saved to the selected location".to_owned());
                }
                temporary = Some(path);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("File could not be saved to the selected location".to_owned()),
        }
    }
    let temporary = temporary.ok_or_else(|| "File could not be saved to the selected location".to_owned())?;
    if fs::rename(&temporary, destination).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("File could not be saved to the selected location".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod save_as_tests {
    use super::*;
    use sha2::Digest;

    #[test]
    fn native_save_as_rejects_bytes_that_do_not_match_the_pinned_digest() {
        let expected_digest = format!("sha256:{:x}", sha2::Sha256::digest(b"expected"));
        let result = verify_artifact_download(
            Some("text/plain"),
            Some(8),
            "text/plain",
            8,
            &expected_digest,
            b"tamper??",
        );

        assert!(result.is_err());
    }

    #[test]
    fn native_save_as_accepts_bytes_matching_pinned_media_size_and_digest() {
        let expected_digest = format!("sha256:{:x}", sha2::Sha256::digest(b"expected"));
        let result = verify_artifact_download(
            Some("text/plain"),
            Some(8),
            "text/plain",
            8,
            &expected_digest,
            b"expected",
        );

        assert!(result.is_ok());
    }

    #[test]
    fn native_save_as_rejects_media_and_size_mismatches() {
        let expected_digest = format!("sha256:{:x}", sha2::Sha256::digest(b"expected"));
        assert!(verify_artifact_download(
            Some("application/octet-stream"), Some(8), "text/plain", 8, &expected_digest, b"expected"
        ).is_err());
        assert!(verify_artifact_download(
            Some("text/plain"), Some(7), "text/plain", 8, &expected_digest, b"expected"
        ).is_err());
        assert!(verify_artifact_download(
            Some("text/plain"), Some(8), "text/plain", 8, &expected_digest, b"short"
        ).is_err());
    }

    #[test]
    fn failed_native_save_as_rename_preserves_existing_destination() {
        use std::{fs, time::SystemTime};

        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("litecowork-save-as-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let destination = root.join("existing");
        fs::create_dir(&destination).unwrap();
        let result = write_export_atomically(&destination, b"replacement");

        assert!(result.is_err());
        assert!(destination.is_dir());
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }
}

/// Narrow read-only bridge. Native transport details and arbitrary request paths are
/// never exposed to the WebView. Bytes use the existing IPC response budget.
#[tauri::command]
pub(crate) async fn artifact_read(
    app: AppHandle,
    workspace_id: String,
    operation: String,
    artifact_id: Option<String>,
    task_id: Option<String>,
    version: Option<u64>,
    library_status: Option<String>,
    cursor: Option<String>,
    limit: Option<u32>,
) -> Result<ArtifactReadResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id) || artifact_id.as_deref().is_some_and(|id| !valid_id(id)) || task_id.as_deref().is_some_and(|id| !valid_id(id))
            || cursor.as_ref().is_some_and(|value| value.is_empty() || value.len() > 2048)
            || limit.is_some_and(|value| !(1..=200).contains(&value))
            || library_status.as_deref().is_some_and(|status| !["TRANSIENT", "SAVED", "ARCHIVED"].contains(&status))
        { return Err("Artifact selection is invalid".to_owned()); }
        let artifact = artifact_id.as_deref().unwrap_or("");
        let path = match operation.as_str() {
            "list" => "/v1/artifacts".to_owned(),
            "library" => "/v1/library".to_owned(),
            "task" if task_id.is_some() => format!("/v1/tasks/{}/artifacts", task_id.as_deref().unwrap_or("")),
            "get" if artifact_id.is_some() => format!("/v1/artifacts/{artifact}"),
            "version" | "content" if artifact_id.is_some() && version.is_some_and(|value| value > 0 && value <= 9_007_199_254_740_991) => {
                let suffix = if operation == "content" { "/content" } else { "" };
                format!("/v1/artifacts/{artifact}/versions/{}{suffix}", version.unwrap_or(0))
            }
            _ => return Err("Artifact operation is unavailable".to_owned()),
        };
        let (client, _) = operator_client(&app)?;
        let mut request = client.get(path).header("X-Workspace-ID", workspace_id);
        if matches!(operation.as_str(), "list" | "library") {
            if let Some(status) = library_status { request = request.query(&[("library_status", status)]); }
            if let Some(cursor) = cursor { request = request.query(&[("cursor", cursor)]); }
            request = request.query(&[("limit", limit.unwrap_or(50).to_string())]);
        }
        let response = request.send().map_err(|_| "Local Artifact service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response.header("content-type").unwrap_or("application/octet-stream").to_owned();
        let maximum = if operation == "content" && response.is_success() { 10 * 1024 * 1024 } else { 1024 * 1024 };
        let mut bytes = Vec::new();
        response.take(maximum as u64 + 1).read_to_end(&mut bytes).map_err(|_| "Artifact response could not be read".to_owned())?;
        if bytes.len() > maximum { return Err("Artifact response exceeds the desktop transfer limit".to_owned()); }
        Ok(ArtifactReadResponse { status, content_type, body_base64: BASE64_STANDARD.encode(bytes) })
    }).await.map_err(|_| "Artifact request did not complete".to_owned())?
}

/// Returns only immutable publication preconditions; the bridge never accepts a path,
/// URL, arbitrary HTTP method, or provider write target from the WebView.
#[tauri::command]
pub(crate) async fn artifact_edit_head(
    app: AppHandle,
    workspace_id: String,
    artifact_id: String,
) -> Result<ArtifactReadResponse, String> {
    artifact_operator_request(app, workspace_id, artifact_id, None).await
}

#[tauri::command]
pub(crate) async fn artifact_append_text_version(
    app: AppHandle,
    workspace_id: String,
    artifact_id: String,
    expected_artifact_version: u64,
    expected_content_version: u64,
    expected_resource_version: u64,
    expected_parent_resource_revision_id: String,
    idempotency_key: String,
    content: String,
) -> Result<ArtifactReadResponse, String> {
    if expected_artifact_version == 0 || expected_content_version == 0 || expected_resource_version == 0
        || idempotency_key.is_empty() || idempotency_key.len() > 128
        || !idempotency_key.bytes().all(|byte| byte.is_ascii_graphic())
        || content.len() > 1_048_576
    { return Err("Artifact text publication request is invalid".to_owned()); }
    let body = serde_json::json!({
        "expected_content_version": expected_content_version,
        "expected_resource_version": expected_resource_version,
        "expected_parent_resource_revision_id": expected_parent_resource_revision_id,
        "content": content,
    });
    artifact_operator_request(app, workspace_id, artifact_id, Some((expected_artifact_version, idempotency_key, body))).await
}

async fn artifact_operator_request(
    app: AppHandle,
    workspace_id: String,
    artifact_id: String,
    mutation: Option<(u64, String, serde_json::Value)>,
) -> Result<ArtifactReadResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id) || !valid_id(&artifact_id) {
            return Err("Artifact selection is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let response = match mutation {
            Some((expected_version, request_id, body)) => client
                .post(format!("/v1/artifacts/{artifact_id}/text-version"))
                .header("X-Workspace-ID", &workspace_id)
                .header("If-Match", format!("\"{expected_version}\""))
                .header("Idempotency-Key", request_id)
                .json(&body)
                .send(),
            None => client.get(format!("/v1/artifacts/{artifact_id}/edit-head"))
                .header("X-Workspace-ID", &workspace_id)
                .send(),
        }.map_err(|_| "Local Artifact service is unavailable".to_owned())?;
        let status = response.status();
        let content_type = response.header("content-type").unwrap_or("application/json").to_owned();
        let mut bytes = Vec::new();
        response.take(1024 * 1024 + 1).read_to_end(&mut bytes)
            .map_err(|_| "Artifact response could not be read".to_owned())?;
        if bytes.len() > 1024 * 1024 { return Err("Artifact response exceeds the desktop transfer limit".to_owned()); }
        Ok(ArtifactReadResponse { status, content_type, body_base64: BASE64_STANDARD.encode(bytes) })
    }).await.map_err(|_| "Artifact request did not complete".to_owned())?
}
