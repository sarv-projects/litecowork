use super::*;
use serde::Deserialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextDocumentDeletionStatusView {
    resource_id: String,
    status: String,
    manifest_digest: Option<String>,
    required_replica_count: u32,
    acknowledged_replica_count: u32,
    updated_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextDocumentDeletionStatusWire {
    resource_id: String,
    status: String,
    manifest_digest: Option<String>,
    required_replica_count: u32,
    acknowledged_replica_count: u32,
    updated_at: String,
}

const STATUS_FIELDS: [&str; 6] = [
    "resource_id",
    "status",
    "manifest_digest",
    "required_replica_count",
    "acknowledged_replica_count",
    "updated_at",
];

#[tauri::command]
pub(crate) async fn begin_context_document_deletion(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
    expected_version: u64,
    request_id: String,
) -> Result<ContextDocumentDeletionStatusView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        super::validate_resource_selection(&workspace_id, &resource_id)?;
        if expected_version == 0 || expected_version == u64::MAX
            || !super::valid_idempotency_key(&request_id)
        {
            return Err("ContextDocument deletion request is invalid".to_owned());
        }
        let (client, workspace_url) = super::operator_client(&app)?;
        let response = client
            .post(format!(
                "{}/resources/{}/context-document/deletion",
                workspace_url.trim_end_matches("/workspaces"),
                resource_id
            ))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", expected_version.to_string())
            .header("Idempotency-Key", request_id)
            .json(&serde_json::json!({}))
            .send()
            .map_err(|_| {
                "ContextDocument deletion outcome is unknown. Check its status or retry the same request."
                    .to_owned()
            })?;
        if response.status() != 202 {
            return Err(operator_deletion_error(response));
        }
        ensure_no_store(&response)?;
        let body = super::read_bounded_response(response)?;
        let status = decode_context_document_deletion_status(&body, &resource_id)?;
        if !matches!(status.status.as_str(), "DELETION_PENDING" | "DELETED") {
            return Err("Local Runtime did not confirm ContextDocument deletion admission".to_owned());
        }
        Ok(status)
    })
    .await
    .map_err(|_| "ContextDocument deletion request did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn get_context_document_deletion_status(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
) -> Result<ContextDocumentDeletionStatusView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        super::validate_resource_selection(&workspace_id, &resource_id)?;
        let (client, workspace_url) = super::operator_client(&app)?;
        let response = client
            .get(format!(
                "{}/resources/{}/context-document/deletion",
                workspace_url.trim_end_matches("/workspaces"),
                resource_id
            ))
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "ContextDocument deletion status is unavailable".to_owned())?;
        if response.status() != 200 {
            return Err(operator_deletion_error(response));
        }
        ensure_no_store(&response)?;
        let body = super::read_bounded_response(response)?;
        decode_context_document_deletion_status(&body, &resource_id)
    })
    .await
    .map_err(|_| "ContextDocument deletion status request did not complete".to_owned())?
}

fn operator_deletion_error(response: LocalOperatorResponse) -> String {
    super::read_bounded_response(response)
        .err()
        .unwrap_or_else(|| {
            "ContextDocument deletion could not be confirmed by the local Runtime".to_owned()
        })
}

fn ensure_no_store(response: &LocalOperatorResponse) -> Result<(), String> {
    if response
        .header("cache-control")
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("no-store"))
    {
        Ok(())
    } else {
        Err("Local Runtime returned a cacheable ContextDocument deletion response".to_owned())
    }
}

fn decode_context_document_deletion_status(
    body: &[u8],
    expected_resource_id: &str,
) -> Result<ContextDocumentDeletionStatusView, String> {
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|_| {
        "Local Runtime returned unsupported ContextDocument deletion status".to_owned()
    })?;
    let object = value.as_object().ok_or_else(|| {
        "Local Runtime returned unsupported ContextDocument deletion status".to_owned()
    })?;
    if object.len() != STATUS_FIELDS.len()
        || STATUS_FIELDS.iter().any(|key| !object.contains_key(*key))
    {
        return Err(
            "Local Runtime returned unsupported ContextDocument deletion status".to_owned(),
        );
    }
    let wire: ContextDocumentDeletionStatusWire = serde_json::from_value(value).map_err(|_| {
        "Local Runtime returned unsupported ContextDocument deletion status".to_owned()
    })?;
    if wire.resource_id != expected_resource_id
        || !matches!(
            wire.status.as_str(),
            "ACTIVE" | "REVOKED" | "DELETION_PENDING" | "DELETED"
        )
        || wire.acknowledged_replica_count > wire.required_replica_count
        || wire.updated_at.len() > 64
        || !valid_rfc3339_timestamp(&wire.updated_at)
        || (matches!(wire.status.as_str(), "ACTIVE" | "REVOKED")
            && (wire.manifest_digest.is_some()
                || wire.required_replica_count != 0
                || wire.acknowledged_replica_count != 0))
        || (matches!(wire.status.as_str(), "DELETION_PENDING" | "DELETED")
            && wire
                .manifest_digest
                .as_deref()
                .is_none_or(|digest| !valid_sha256_digest(digest)))
        || (wire.status == "DELETED"
            && wire.acknowledged_replica_count != wire.required_replica_count)
    {
        return Err("Local Runtime returned invalid ContextDocument deletion status".to_owned());
    }
    Ok(ContextDocumentDeletionStatusView {
        resource_id: wire.resource_id,
        status: wire.status,
        manifest_digest: wire.manifest_digest,
        required_replica_count: wire.required_replica_count,
        acknowledged_replica_count: wire.acknowledged_replica_count,
        updated_at: wire.updated_at,
    })
}

fn valid_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_rfc3339_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || !bytes[..19]
            .iter()
            .enumerate()
            .all(|(index, byte)| match index {
                4 | 7 => *byte == b'-',
                10 => *byte == b'T',
                13 | 16 => *byte == b':',
                _ => byte.is_ascii_digit(),
            })
    {
        return false;
    }
    let year = parse_ascii_number(&bytes[0..4]);
    let month = parse_ascii_number(&bytes[5..7]);
    let day = parse_ascii_number(&bytes[8..10]);
    let hour = parse_ascii_number(&bytes[11..13]);
    let minute = parse_ascii_number(&bytes[14..16]);
    let second = parse_ascii_number(&bytes[17..19]);
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) =
        (year, month, day, hour, minute, second)
    else {
        return false;
    };
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return false,
    };
    if year == 0 || day == 0 || day > days_in_month || hour > 23 || minute > 59 || second > 59 {
        return false;
    }
    let mut index = 19;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    if bytes.get(index) == Some(&b'Z') {
        return index + 1 == bytes.len();
    }
    if bytes.get(index) != Some(&b'+') && bytes.get(index) != Some(&b'-') {
        return false;
    }
    let zone = &bytes[index + 1..];
    zone.len() == 5
        && zone[0].is_ascii_digit()
        && zone[1].is_ascii_digit()
        && zone[2] == b':'
        && zone[3].is_ascii_digit()
        && zone[4].is_ascii_digit()
        && u16::from(zone[0] - b'0') * 10 + u16::from(zone[1] - b'0') <= 23
        && u16::from(zone[3] - b'0') * 10 + u16::from(zone[4] - b'0') <= 59
}

fn parse_ascii_number(bytes: &[u8]) -> Option<u16> {
    if !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::decode_context_document_deletion_status;

    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const VALID: &[u8] = br#"{"resource_id":"resource-1","status":"DELETION_PENDING","manifest_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required_replica_count":2,"acknowledged_replica_count":1,"updated_at":"2026-10-15T12:00:00Z"}"#;

    #[test]
    fn decodes_the_safe_deletion_projection_into_the_camel_case_view() {
        let status = decode_context_document_deletion_status(VALID, "resource-1")
            .expect("route status should decode");
        assert_eq!(status.resource_id, "resource-1");
        assert_eq!(status.status, "DELETION_PENDING");
        assert_eq!(status.manifest_digest.as_deref(), Some(DIGEST));
        assert_eq!(status.required_replica_count, 2);
        assert_eq!(status.acknowledged_replica_count, 1);
        let serialized = serde_json::to_value(status).expect("Tauri view serializes");
        assert_eq!(serialized["resourceId"], "resource-1");
        assert_eq!(serialized["requiredReplicaCount"], 2);
        assert!(serialized.get("resource_id").is_none());
    }

    #[test]
    fn rejects_private_unknown_missing_foreign_and_inconsistent_status_fields() {
        for invalid in [
            br#"{"resource_id":"resource-1","status":"DELETION_PENDING","manifest_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required_replica_count":2,"acknowledged_replica_count":1,"updated_at":"2026-10-15T12:00:00Z","provider_locator":"secret"}"#.as_slice(),
            br#"{"resource_id":"resource-1","status":"DELETION_PENDING","manifest_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required_replica_count":2,"acknowledged_replica_count":1}"#.as_slice(),
            br#"{"resource_id":"resource-2","status":"DELETION_PENDING","manifest_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required_replica_count":2,"acknowledged_replica_count":1,"updated_at":"2026-10-15T12:00:00Z"}"#.as_slice(),
            br#"{"resource_id":"resource-1","status":"DELETION_PENDING","manifest_digest":"locator/path","required_replica_count":2,"acknowledged_replica_count":1,"updated_at":"2026-10-15T12:00:00Z"}"#.as_slice(),
            br#"{"resource_id":"resource-1","status":"DELETED","manifest_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required_replica_count":2,"acknowledged_replica_count":1,"updated_at":"2026-10-15T12:00:00Z"}"#.as_slice(),
            br#"{"resource_id":"resource-1","status":"DELETION_PENDING","manifest_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required_replica_count":2,"acknowledged_replica_count":1,"updated_at":"2026-19-15T12:00:00Z"}"#.as_slice(),
            br#"{"resource_id":"resource-1","status":"DELETION_PENDING","manifest_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required_replica_count":2,"acknowledged_replica_count":1,"updated_at":"2026-10-15T25:00:00Z"}"#.as_slice(),
        ] {
            assert!(decode_context_document_deletion_status(invalid, "resource-1").is_err());
        }
    }
}
