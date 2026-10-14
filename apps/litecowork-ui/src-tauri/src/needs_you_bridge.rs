use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NeedsYouPageWire {
    items: Vec<NeedsYouItemWire>,
    #[serde(deserialize_with = "required_nullable")]
    next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NeedsYouItemWire {
    item_id: String,
    kind: String,
    status: String,
    #[serde(deserialize_with = "required_nullable")]
    approval_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    user_request_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    task_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    blocker_id: Option<String>,
    title: String,
    safe_summary: String,
    created_at: String,
    #[serde(deserialize_with = "required_nullable")]
    resolved_at: Option<String>,
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NeedsYouItemView {
    item_id: String,
    kind: String,
    status: String,
    approval_id: Option<String>,
    user_request_id: Option<String>,
    task_id: Option<String>,
    blocker_id: Option<String>,
    title: String,
    safe_summary: String,
    created_at: String,
    resolved_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NeedsYouPageView {
    items: Vec<NeedsYouItemView>,
    next_cursor: Option<String>,
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn valid_task_filter_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 200 && !value.chars().any(char::is_control)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_item_id(kind: &str, source_ids: &[&str]) -> Option<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"litecowork.needs-you.item.v1\0");
    for value in std::iter::once(kind).chain(source_ids.iter().copied()) {
        let length = u32::try_from(value.len()).ok()?;
        hasher.update(length.to_be_bytes());
        hasher.update(value.as_bytes());
    }
    Some(format!("sha256:{:x}", hasher.finalize()))
}

fn validate_item(item: &NeedsYouItemWire, requested_status: &str) -> Result<(), String> {
    let (identity_valid, source_ids): (bool, Vec<&str>) = match item.kind.as_str() {
        "APPROVAL" => {
            let approval_id = item.approval_id.as_deref();
            (
                approval_id.is_some_and(valid_id)
                    && item.task_id.as_deref().is_some_and(valid_id)
                    && item.user_request_id.is_none()
                    && item.blocker_id.is_none(),
                approval_id.into_iter().collect(),
            )
        }
        "USER_REQUEST" => {
            let request_id = item.user_request_id.as_deref();
            (
                item.approval_id.is_none()
                    && request_id.is_some_and(valid_id)
                    && item.blocker_id.is_none(),
                request_id.into_iter().collect(),
            )
        }
        "TASK_BLOCKER" => {
            let task_id = item.task_id.as_deref();
            let blocker_id = item.blocker_id.as_deref();
            (
                item.approval_id.is_none()
                    && item.user_request_id.is_none()
                    && task_id.is_some_and(valid_id)
                    && blocker_id.is_some_and(valid_id)
                    && item.status == "OPEN",
                task_id.into_iter().chain(blocker_id).collect(),
            )
        }
        _ => (false, Vec::new()),
    };
    if !valid_digest(&item.item_id)
        || canonical_item_id(&item.kind, &source_ids).as_deref() != Some(item.item_id.as_str())
        || !matches!(item.status.as_str(), "OPEN" | "RESOLVED")
        || item.status != requested_status
        || !identity_valid
        || item
            .task_id
            .as_deref()
            .is_some_and(|value| !valid_id(value))
        || !valid_display_text(&item.title)
        || !valid_display_text(&item.safe_summary)
        || item.created_at.trim().is_empty()
        || item.resolved_at.as_deref().is_some_and(str::is_empty)
    {
        return Err("Local Runtime returned an invalid Needs You item".to_owned());
    }
    Ok(())
}

fn valid_display_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 8192
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn map_needs_you_page(
    requested_status: &str,
    page: NeedsYouPageWire,
) -> Result<NeedsYouPageView, String> {
    if page.items.len() > 200
        || page
            .next_cursor
            .as_deref()
            .is_some_and(|cursor| cursor.is_empty() || cursor.len() > 4096)
    {
        return Err("Local Runtime returned an invalid Needs You page".to_owned());
    }
    let mut ids = std::collections::BTreeSet::new();
    let items = page
        .items
        .into_iter()
        .map(|item| {
            validate_item(&item, requested_status)?;
            if !ids.insert(item.item_id.clone()) {
                return Err("Local Runtime returned duplicate Needs You items".to_owned());
            }
            Ok(NeedsYouItemView {
                item_id: item.item_id,
                kind: item.kind,
                status: item.status,
                approval_id: item.approval_id,
                user_request_id: item.user_request_id,
                task_id: item.task_id,
                blocker_id: item.blocker_id,
                title: item.title,
                safe_summary: item.safe_summary,
                created_at: item.created_at,
                resolved_at: item.resolved_at,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(NeedsYouPageView {
        items,
        next_cursor: page.next_cursor,
    })
}

#[tauri::command]
pub(crate) async fn list_needs_you(
    app: AppHandle,
    workspace_id: String,
    status: String,
    cursor: Option<String>,
    limit: Option<u16>,
    kind: Option<String>,
    task_id: Option<String>,
) -> Result<NeedsYouPageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !matches!(status.as_str(), "OPEN" | "RESOLVED")
            || kind
                .as_deref()
                .is_some_and(|value| !matches!(value, "APPROVAL" | "USER_REQUEST" | "TASK_BLOCKER"))
            || task_id
                .as_deref()
                .is_some_and(|value| !valid_task_filter_id(value))
            || cursor
                .as_deref()
                .is_some_and(|value| value.is_empty() || value.len() > 4096)
            || limit.is_some_and(|value| !(1..=200).contains(&value))
        {
            return Err("Needs You request is invalid".to_owned());
        }
        let limit_text = limit.unwrap_or(50).to_string();
        let (client, workspace_url) = super::operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let mut request = client
            .get(format!("{base_url}/needs-you"))
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("status", status.as_str()), ("limit", limit_text.as_str())]);
        if let Some(cursor) = cursor.as_deref() {
            request = request.query(&[("cursor", cursor)]);
        }
        if let Some(kind) = kind.as_deref() {
            request = request.query(&[("kind", kind)]);
        }
        if let Some(task_id) = task_id.as_deref() {
            request = request.query(&[("task_id", task_id)]);
        }
        let response = request
            .send()
            .map_err(|_| "Local Needs You projection is unavailable".to_owned())?;
        let body = super::read_bounded_response(response)?;
        let page: NeedsYouPageWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned an unsupported Needs You page".to_owned())?;
        map_needs_you_page(&status, page)
    })
    .await
    .map_err(|_| "Needs You request did not complete".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn digest() -> String {
        canonical_item_id("TASK_BLOCKER", &["task-1", "blocker-1"]).unwrap()
    }

    fn wire_item() -> NeedsYouItemWire {
        NeedsYouItemWire {
            item_id: digest(),
            kind: "TASK_BLOCKER".to_owned(),
            status: "OPEN".to_owned(),
            approval_id: None,
            user_request_id: None,
            task_id: Some("task-1".to_owned()),
            blocker_id: Some("blocker-1".to_owned()),
            title: "Task blocker".to_owned(),
            safe_summary: "A Task needs attention.".to_owned(),
            created_at: "2026-10-14T12:00:00Z".to_owned(),
            resolved_at: None,
        }
    }

    #[test]
    fn needs_you_wire_requires_nullable_fields_and_emits_camel_case() {
        let wire: NeedsYouPageWire = serde_json::from_value(json!({
            "items": [{
                "item_id": digest(), "kind": "TASK_BLOCKER", "status": "OPEN",
                "approval_id": null, "user_request_id": null, "task_id": "task-1",
                "blocker_id": "blocker-1", "title": "Task blocker",
                "safe_summary": "A Task needs attention.",
                "created_at": "2026-10-14T12:00:00Z", "resolved_at": null
            }],
            "next_cursor": null
        }))
        .unwrap();
        let page = map_needs_you_page("OPEN", wire).unwrap();
        let value = serde_json::to_value(page).unwrap();
        assert!(value["items"][0].get("itemId").is_some());
        assert!(value["items"][0].get("item_id").is_none());
        assert_eq!(value["items"][0]["resolvedAt"], serde_json::Value::Null);

        let missing_nullable = serde_json::from_value::<NeedsYouPageWire>(json!({
            "items": [{
                "item_id": digest(), "kind": "TASK_BLOCKER", "status": "OPEN",
                "task_id": "task-1", "blocker_id": "blocker-1",
                "title": "Task blocker", "safe_summary": "A Task needs attention.",
                "created_at": "2026-10-14T12:00:00Z"
            }], "next_cursor": null
        }));
        assert!(missing_nullable.is_err());
    }

    #[test]
    fn needs_you_bridge_rejects_mismatched_status_identity_and_duplicate_rows() {
        assert!(validate_item(&wire_item(), "RESOLVED").is_err());
        let mut invalid_identity = wire_item();
        invalid_identity.approval_id = Some("approval-1".to_owned());
        assert!(validate_item(&invalid_identity, "OPEN").is_err());

        let item = wire_item();
        let duplicate_page = NeedsYouPageWire {
            items: vec![item.clone(), item],
            next_cursor: None,
        };
        assert!(map_needs_you_page("OPEN", duplicate_page).is_err());
    }

    #[test]
    fn needs_you_task_filter_matches_the_operator_length_bound() {
        assert!(valid_task_filter_id(&"t".repeat(200)));
        assert!(!valid_task_filter_id(&"t".repeat(201)));
        assert!(!valid_task_filter_id("\ttask"));
    }
}
