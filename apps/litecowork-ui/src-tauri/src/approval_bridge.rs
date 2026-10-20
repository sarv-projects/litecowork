use serde::{Deserialize, Serialize};
use tauri::AppHandle;

const MAX_APPROVAL_ID_BYTES: usize = 512;
const MAX_REQUEST_ID_BYTES: usize = 200;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalWire {
    approval_id: String,
    task_id: String,
    #[serde(deserialize_with = "required_nullable")]
    requested_by_attempt: Option<String>,
    kind: String,
    action_summary: String,
    target_ref: ResourceRefWire,
    scope_digest: String,
    action_digest: String,
    risk: String,
    required_assurance: String,
    status: String,
    requested_at: String,
    #[serde(deserialize_with = "required_nullable")]
    expires_at: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    resolved_by: Option<PrincipalRefWire>,
    #[serde(deserialize_with = "required_nullable")]
    resolved_at: Option<String>,
    version: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResourceRefWire {
    workspace_id: String,
    resource_id: String,
    revision_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PrincipalRefWire {
    principal_id: String,
    kind: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ResourceRefView {
    workspace_id: String,
    resource_id: String,
    revision_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PrincipalRefView {
    principal_id: String,
    kind: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApprovalView {
    approval_id: String,
    task_id: String,
    requested_by_attempt: Option<String>,
    kind: String,
    action_summary: String,
    target_ref: ResourceRefView,
    scope_digest: String,
    action_digest: String,
    risk: String,
    required_assurance: String,
    status: String,
    requested_at: String,
    expires_at: Option<String>,
    resolved_by: Option<PrincipalRefView>,
    resolved_at: Option<String>,
    version: u64,
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_APPROVAL_ID_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_timestamp(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 64 && !value.chars().any(char::is_control)
}

fn map_approval(
    requested_workspace_id: &str,
    requested_approval_id: &str,
    value: ApprovalWire,
) -> Result<ApprovalView, String> {
    if !valid_id(requested_workspace_id)
        || !valid_id(requested_approval_id)
        || value.approval_id != requested_approval_id
        || !valid_id(&value.approval_id)
        || !valid_id(&value.task_id)
        || value
            .requested_by_attempt
            .as_deref()
            .is_some_and(|value| !valid_id(value))
        || !valid_id(&value.kind)
        || value.action_summary.trim().is_empty()
        || value.action_summary.len() > 16_384
        || value.target_ref.workspace_id != requested_workspace_id
        || !valid_id(&value.target_ref.resource_id)
        || value
            .target_ref
            .revision_id
            .as_deref()
            .is_some_and(|value| !valid_id(value))
        || !valid_digest(&value.scope_digest)
        || !valid_digest(&value.action_digest)
        || !matches!(value.risk.as_str(), "SAFE" | "SENSITIVE" | "HIGH_IMPACT")
        || !matches!(
            value.required_assurance.as_str(),
            "VIEW_ONLY" | "STEER_SAFE" | "APPROVE_SAFE" | "APPROVE_SENSITIVE" | "LOCAL_STRONG"
        )
        || !matches!(
            value.status.as_str(),
            "PENDING" | "APPROVED" | "DENIED" | "EXPIRED" | "CANCELLED"
        )
        || !valid_timestamp(&value.requested_at)
        || value
            .expires_at
            .as_deref()
            .is_some_and(|value| !valid_timestamp(value))
        || value.resolved_by.as_ref().is_some_and(|principal| {
            !valid_id(&principal.principal_id)
                || !matches!(
                    principal.kind.as_str(),
                    "USER" | "SERVICE" | "RUNTIME" | "AGENT" | "CHANNEL_IDENTITY"
                )
        })
        || value
            .resolved_at
            .as_deref()
            .is_some_and(|value| !valid_timestamp(value))
        || value.version == 0
        || (value.status == "PENDING"
            && (value.resolved_at.is_some() || value.resolved_by.is_some()))
        || (matches!(value.status.as_str(), "APPROVED" | "DENIED")
            && (value.resolved_at.is_none() || value.resolved_by.is_none()))
    {
        return Err("Local Runtime returned an invalid Approval for this Workspace".to_owned());
    }
    Ok(ApprovalView {
        approval_id: value.approval_id,
        task_id: value.task_id,
        requested_by_attempt: value.requested_by_attempt,
        kind: value.kind,
        action_summary: value.action_summary,
        target_ref: ResourceRefView {
            workspace_id: value.target_ref.workspace_id,
            resource_id: value.target_ref.resource_id,
            revision_id: value.target_ref.revision_id,
        },
        scope_digest: value.scope_digest,
        action_digest: value.action_digest,
        risk: value.risk,
        required_assurance: value.required_assurance,
        status: value.status,
        requested_at: value.requested_at,
        expires_at: value.expires_at,
        resolved_by: value.resolved_by.map(|principal| PrincipalRefView {
            principal_id: principal.principal_id,
            kind: principal.kind,
        }),
        resolved_at: value.resolved_at,
        version: value.version,
    })
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

#[tauri::command]
pub(crate) async fn get_approval_detail(
    app: AppHandle,
    workspace_id: String,
    approval_id: String,
) -> Result<ApprovalView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id) || !valid_id(&approval_id) {
            return Err("Approval selection is invalid".to_owned());
        }
        let (client, workspace_url) = super::operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .get(format!(
                "{base_url}/approvals/{}",
                encode_path_segment(&approval_id)
            ))
            .header("X-Workspace-ID", &workspace_id)
            .send()?;
        if !response.is_success() {
            return Err(super::read_bounded_response(response)
                .err()
                .unwrap_or_else(|| {
                    "Approval is unavailable in the selected Workspace".to_owned()
                }));
        }
        if response.header("cache-control") != Some("no-store") {
            return Err(
                "Local Runtime returned Approval details without no-store protection".to_owned(),
            );
        }
        let body = super::read_bounded_response(response)?;
        let wire: ApprovalWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned malformed Approval details".to_owned())?;
        map_approval(&workspace_id, &approval_id, wire)
    })
    .await
    .map_err(|_| "Approval detail read did not complete".to_owned())?
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Decision {
    Approve,
    Deny,
}

#[derive(Serialize)]
struct ResolveRequest {
    decision: Decision,
    confirmation_digest: Option<String>,
}

#[tauri::command]
pub(crate) async fn resolve_approval(
    app: AppHandle,
    workspace_id: String,
    approval_id: String,
    expected_version: u64,
    decision: String,
    request_id: String,
    confirmation_digest: Option<String>,
) -> Result<ApprovalView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !valid_id(&approval_id)
            || expected_version == 0
            || request_id.is_empty()
            || request_id.len() > MAX_REQUEST_ID_BYTES
            || !request_id.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
            || confirmation_digest
                .as_deref()
                .is_some_and(|digest| !valid_digest(digest))
        {
            return Err("Approval decision request is invalid".to_owned());
        }
        let decision = match decision.as_str() {
            "APPROVE" => Decision::Approve,
            "DENY" => Decision::Deny,
            _ => return Err("Approval decision is invalid".to_owned()),
        };
        let (client, workspace_url) = super::operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .post(format!(
                "{base_url}/approvals/{}/resolve",
                encode_path_segment(&approval_id)
            ))
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", &request_id)
            .header("If-Match", expected_version)
            .json(&ResolveRequest {
                decision,
                confirmation_digest,
            })
            .send()?;
        if !response.is_success() {
            return Err(super::read_bounded_response(response)
                .err()
                .unwrap_or_else(|| "Approval decision could not be confirmed".to_owned()));
        }
        if response.header("cache-control") != Some("no-store") {
            return Err(
                "Local Runtime returned Approval decision without no-store protection".to_owned(),
            );
        }
        let body = super::read_bounded_response(response)?;
        let wire: ApprovalWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned malformed Approval decision".to_owned())?;
        map_approval(&workspace_id, &approval_id, wire)
    })
    .await
    .map_err(|_| "Approval decision did not complete".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wire() -> ApprovalWire {
        serde_json::from_value(json!({
            "approval_id":"approval-1",
            "task_id":"task-1",
            "requested_by_attempt":null,
            "kind":"EXTERNAL_MUTATION",
            "action_summary":"Publish the approved document",
            "target_ref":{"workspace_id":"workspace-1","resource_id":"resource-1","revision_id":null},
            "scope_digest":format!("sha256:{}", "a".repeat(64)),
            "action_digest":format!("sha256:{}", "b".repeat(64)),
            "risk":"SENSITIVE",
            "required_assurance":"APPROVE_SENSITIVE",
            "status":"PENDING",
            "requested_at":"2026-10-16T10:00:00Z",
            "expires_at":null,
            "resolved_by":null,
            "resolved_at":null,
            "version":2
        }))
        .expect("approval fixture deserializes")
    }

    #[test]
    fn bridge_maps_exact_workspace_and_approval_identity_to_camel_case() {
        let view = map_approval("workspace-1", "approval-1", wire()).expect("valid approval");
        let value = serde_json::to_value(view).expect("view serializes");
        assert_eq!(value["approvalId"], "approval-1");
        assert_eq!(value["targetRef"]["workspaceId"], "workspace-1");
        assert_eq!(value["targetRef"]["revisionId"], serde_json::Value::Null);
        assert_eq!(value["status"], "PENDING");
    }

    #[test]
    fn bridge_rejects_foreign_identity_malformed_digest_and_unsupported_states() {
        assert!(map_approval("workspace-other", "approval-1", wire()).is_err());
        assert!(map_approval("workspace-1", "approval-other", wire()).is_err());
        let mut bad_digest = wire();
        bad_digest.action_digest = "sha256:bad".to_owned();
        assert!(map_approval("workspace-1", "approval-1", bad_digest).is_err());
        let mut unresolved = wire();
        unresolved.status = "WAITING".to_owned();
        assert!(map_approval("workspace-1", "approval-1", unresolved).is_err());
        let mut terminal_without_actor = wire();
        terminal_without_actor.status = "APPROVED".to_owned();
        terminal_without_actor.resolved_at = Some("2026-10-16T11:00:00Z".to_owned());
        assert!(map_approval("workspace-1", "approval-1", terminal_without_actor).is_err());
    }

    #[test]
    fn approval_path_segment_is_encoded_without_interpreting_path_controls() {
        assert_eq!(
            encode_path_segment("approval/with spaces"),
            "approval%2Fwith%20spaces"
        );
    }
}
