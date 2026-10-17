use super::{operator_client, read_bounded_response, AppHandle};
use serde::{Deserialize, Serialize};

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct WorkspaceRootObservationPageView {
    workspace_id: String,
    workspace_root_id: String,
    items: Vec<WorkspaceRootObservationView>,
    next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct WorkspaceRootObservationView {
    observation_id: String,
    display_name: String,
    size_bytes: u64,
    modified_at: Option<String>,
    observed_at: String,
    freshness: String,
    resource_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceRootObservationPageWire {
    workspace_id: String,
    workspace_root_id: String,
    items: Vec<WorkspaceRootObservationWire>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceRootObservationWire {
    observation_id: String,
    display_name: String,
    size_bytes: u64,
    modified_at: Option<String>,
    observed_at: String,
    freshness: String,
    resource_id: Option<String>,
}

fn valid_observation_request(
    workspace_id: &str,
    workspace_root_id: &str,
    root_version: u64,
    query: &str,
    freshness: Option<&str>,
    cursor: Option<&str>,
    limit: usize,
) -> bool {
    super::valid_workspace_root_scan_id(workspace_id)
        && super::valid_workspace_root_scan_id(workspace_root_id)
        && root_version > 0
        && root_version <= MAX_SAFE_INTEGER
        && query.len() <= 256
        && !query.contains('\0')
        && freshness.is_none_or(|value| matches!(value, "CURRENT" | "STALE" | "UNKNOWN"))
        && cursor.is_none_or(|value| !value.is_empty() && value.len() <= 4096)
        && (1..=100).contains(&limit)
}

fn parse_observation_page(
    body: &[u8],
    workspace_id: &str,
    workspace_root_id: &str,
    limit: usize,
) -> Result<WorkspaceRootObservationPageView, String> {
    let wire: WorkspaceRootObservationPageWire = serde_json::from_slice(body)
        .map_err(|_| "Local Runtime returned an unsupported observed-file page".to_owned())?;
    if wire.workspace_id != workspace_id
        || wire.workspace_root_id != workspace_root_id
        || wire.items.len() > limit
        || wire
            .next_cursor
            .as_deref()
            .is_some_and(|cursor| cursor.is_empty() || cursor.len() > 4096)
    {
        return Err("Local Runtime returned an invalid observed-file page".to_owned());
    }
    let mut items = Vec::with_capacity(wire.items.len());
    for item in wire.items {
        if !valid_digest_id(&item.observation_id)
            || item.display_name.is_empty()
            || item.display_name.len() > 1024
            || item.display_name.contains('\0')
            || item.size_bytes > MAX_SAFE_INTEGER
            || !super::is_rfc3339_timestamp(&item.observed_at)
            || item
                .modified_at
                .as_deref()
                .is_some_and(|timestamp| !super::is_rfc3339_timestamp(timestamp))
            || !matches!(item.freshness.as_str(), "CURRENT" | "STALE" | "UNKNOWN")
            || item.resource_id.is_some()
        {
            return Err("Local Runtime returned an invalid observed-file entry".to_owned());
        }
        items.push(WorkspaceRootObservationView {
            observation_id: item.observation_id,
            display_name: item.display_name,
            size_bytes: item.size_bytes,
            modified_at: item.modified_at,
            observed_at: item.observed_at,
            freshness: item.freshness,
            resource_id: None,
        });
    }
    Ok(WorkspaceRootObservationPageView {
        workspace_id: wire.workspace_id,
        workspace_root_id: wire.workspace_root_id,
        items,
        next_cursor: wire.next_cursor,
    })
}

fn valid_digest_id(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn observation_operator_error(error: String) -> String {
    let Some(code) = super::operator_error_code(&error) else {
        return "Observed files could not be loaded".to_owned();
    };
    if matches!(
        code,
        "STALE_WORKSPACE_VERSION" | "INVALID_ARGUMENT" | "NOT_FOUND" | "BUSY" | "INTERNAL"
    ) {
        code.to_owned()
    } else {
        "Observed files could not be loaded".to_owned()
    }
}

#[tauri::command]
pub(super) async fn search_workspace_root_observations(
    app: AppHandle,
    workspace_id: String,
    workspace_root_id: String,
    root_version: u64,
    query: String,
    freshness: Option<String>,
    cursor: Option<String>,
    limit: usize,
) -> Result<WorkspaceRootObservationPageView, String> {
    if !valid_observation_request(
        &workspace_id,
        &workspace_root_id,
        root_version,
        &query,
        freshness.as_deref(),
        cursor.as_deref(),
        limit,
    ) {
        return Err("Observed-file search request is invalid".to_owned());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let (client, workspace_url) = operator_client(&app)?;
        let base = format!(
            "{}/workspace-roots",
            workspace_url.trim_end_matches("/workspaces")
        );
        let mut request = client
            .get(format!("{base}/{workspace_root_id}/observations"))
            .header("X-Workspace-ID", &workspace_id)
            .header("If-Match", root_version.to_string());
        let mut query_pairs: Vec<(&str, String)> = vec![("q", query), ("limit", limit.to_string())];
        if let Some(freshness) = freshness {
            query_pairs.push(("freshness", freshness));
        }
        if let Some(cursor) = cursor {
            query_pairs.push(("cursor", cursor));
        }
        request = request.query(&query_pairs);
        let response = request
            .send()
            .map_err(|_| "Observed files could not be loaded".to_owned())?;
        if response.status() != 200 {
            return match read_bounded_response(response) {
                Err(error) => Err(observation_operator_error(error)),
                Ok(_) => {
                    Err("Local Runtime returned an unexpected observed-file response".to_owned())
                }
            };
        }
        let body = read_bounded_response(response)?;
        parse_observation_page(&body, &workspace_id, &workspace_root_id, limit)
    })
    .await
    .map_err(|_| "Observed-file search request did not complete".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::{
        observation_operator_error, parse_observation_page, valid_observation_request,
        WorkspaceRootObservationPageView,
    };
    use serde_json::{json, Value};

    fn page() -> Value {
        json!({
            "workspace_id": "workspace-1",
            "workspace_root_id": "root-1",
            "items": [{
                "observation_id": format!("sha256:{}", "a".repeat(64)),
                "display_name": "Résumé.txt",
                "size_bytes": 124,
                "modified_at": "2026-10-17T10:00:00Z",
                "observed_at": "2026-10-17T10:01:00Z",
                "freshness": "CURRENT",
                "resource_id": null
            }],
            "next_cursor": "opaque-cursor"
        })
    }

    fn parse(value: Value) -> Result<WorkspaceRootObservationPageView, String> {
        parse_observation_page(
            &serde_json::to_vec(&value).expect("fixture encodes"),
            "workspace-1",
            "root-1",
            50,
        )
    }

    #[test]
    fn validates_selected_workspace_root_version_and_bounded_filters() {
        assert!(valid_observation_request(
            "workspace-1",
            "root-1",
            1,
            "résumé",
            None,
            None,
            50
        ));
        assert!(!valid_observation_request(
            "../private",
            "root-1",
            1,
            "",
            None,
            None,
            50
        ));
        assert!(!valid_observation_request(
            "workspace-1",
            "root-1",
            0,
            "",
            None,
            None,
            50
        ));
        assert!(!valid_observation_request(
            "workspace-1",
            "root-1",
            1,
            &"x".repeat(257),
            None,
            None,
            50
        ));
        assert!(!valid_observation_request(
            "workspace-1",
            "root-1",
            1,
            "\0",
            None,
            None,
            50
        ));
        assert!(!valid_observation_request(
            "workspace-1",
            "root-1",
            1,
            "",
            Some("FRESH"),
            None,
            50
        ));
        assert!(!valid_observation_request(
            "workspace-1",
            "root-1",
            1,
            "",
            None,
            Some(""),
            50
        ));
        assert!(!valid_observation_request(
            "workspace-1",
            "root-1",
            1,
            "",
            None,
            None,
            101
        ));
    }

    #[test]
    fn accepts_metadata_page_and_serializes_snake_case_without_private_fields() {
        let result = parse(page()).expect("valid metadata page accepted");
        let serialized = serde_json::to_value(result).expect("Tauri response serializes");
        assert_eq!(serialized["workspace_id"], "workspace-1");
        assert_eq!(serialized["items"][0]["display_name"], "Résumé.txt");
        assert_eq!(serialized["items"][0]["resource_id"], Value::Null);
        assert!(serialized["items"][0]
            .get("private_relative_locator")
            .is_none());
        assert!(serialized["items"][0].get("file_identity_digest").is_none());
    }

    #[test]
    fn rejects_wrong_scope_unknown_fields_invalid_cursor_and_noncanonical_resource() {
        let mut wrong_scope = page();
        wrong_scope["workspace_root_id"] = json!("root-other");
        assert!(parse(wrong_scope).is_err());

        let mut unknown = page();
        unknown["items"][0]["private_relative_locator"] = json!("secret/path");
        assert!(parse(unknown).is_err());

        let mut noncanonical = page();
        noncanonical["items"][0]["resource_id"] = json!("resource-1");
        assert!(parse(noncanonical).is_err());

        let mut bad_cursor = page();
        bad_cursor["next_cursor"] = json!("");
        assert!(parse(bad_cursor).is_err());
    }

    #[test]
    fn maps_only_safe_observation_error_codes() {
        assert_eq!(
            observation_operator_error(
                "Folder state changed [STALE_WORKSPACE_VERSION; retryable=false; reference=cor_1]"
                    .to_owned()
            ),
            "STALE_WORKSPACE_VERSION"
        );
        assert_eq!(
            observation_operator_error(
                "private details [SOME_NEW_CODE; retryable=true; reference=cor_1]".to_owned()
            ),
            "Observed files could not be loaded"
        );
        assert_eq!(
            observation_operator_error("secret response body".to_owned()),
            "Observed files could not be loaded"
        );
    }
}
