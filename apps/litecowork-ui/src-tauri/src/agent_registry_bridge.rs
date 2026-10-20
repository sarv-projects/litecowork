use super::{AppHandle, operator_client, read_bounded_response};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryPageWire {
    items: Vec<RegistryEntryWire>,
    next_cursor: Option<String>,
    observed_at: String,
    stale: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrySnapshotWire {
    source_registry: String,
    source_revision: Option<String>,
    observed_at: String,
    expires_at: Option<String>,
    stale: bool,
    items: Vec<RegistryEntryWire>,
}

#[derive(Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all(serialize = "camelCase"))]
struct RegistryEntryWire {
    registry_agent_id: String,
    display_name: String,
    description: Option<String>,
    homepage: Option<String>,
    source_registry: String,
    source_revision: Option<String>,
    distributions: Vec<RegistryDistributionWire>,
    adapter_support: String,
    metadata_observed_at: String,
    cache_expires_at: Option<String>,
}

#[derive(Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all(serialize = "camelCase"))]
struct RegistryDistributionWire {
    distribution_id: String,
    kind: String,
    package_ref: String,
    version_constraint: Option<String>,
    os: Vec<String>,
    architecture: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RegistryPageView {
    items: Vec<RegistryEntryWire>,
    next_cursor: Option<String>,
    observed_at: String,
    stale: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RegistrySnapshotView {
    source_registry: String,
    source_revision: Option<String>,
    observed_at: String,
    expires_at: Option<String>,
    stale: bool,
    items: Vec<RegistryEntryWire>,
}

#[tauri::command]
pub(super) async fn list_agent_registry(
    app: AppHandle,
    q: Option<String>,
    cursor: Option<String>,
    limit: Option<u32>,
) -> Result<RegistryPageView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (client, _) = operator_client(&app)?;
        let mut path = "/v1/agent-registry".to_owned();
        let mut query = Vec::new();
        if let Some(q) = q {
            query.push(format!("q={}", encode_query_component(&q)));
        }
        if let Some(cursor) = cursor {
            query.push(format!("cursor={}", encode_query_component(&cursor)));
        }
        if let Some(limit) = limit {
            query.push(format!("limit={limit}"));
        }
        if !query.is_empty() {
            path.push('?');
            path.push_str(&query.join("&"));
        }
        let response = client
            .get(path)
            .send()
            .map_err(|_| "ACP Registry metadata is unavailable".to_owned())?;
        let body = read_bounded_response(response)?;
        let page: RegistryPageWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Operator returned an unsupported ACP Registry page".to_owned())?;
        Ok(RegistryPageView {
            items: page.items,
            next_cursor: page.next_cursor,
            observed_at: page.observed_at,
            stale: page.stale,
        })
    })
    .await
    .map_err(|_| "ACP Registry metadata request failed".to_owned())?
}

#[tauri::command]
pub(super) async fn refresh_agent_registry(app: AppHandle) -> Result<RegistrySnapshotView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (client, _) = operator_client(&app)?;
        let response = client
            .post("/v1/agent-registry/refresh")
            .send()
            .map_err(|_| "ACP Registry refresh request could not be sent".to_owned())?;
        let body = read_bounded_response(response)?;
        let snapshot: RegistrySnapshotWire = serde_json::from_slice(&body).map_err(|_| {
            "Local Operator returned an unsupported ACP Registry snapshot".to_owned()
        })?;
        Ok(RegistrySnapshotView {
            source_registry: snapshot.source_registry,
            source_revision: snapshot.source_revision,
            observed_at: snapshot.observed_at,
            expires_at: snapshot.expires_at,
            stale: snapshot.stale,
            items: snapshot.items,
        })
    })
    .await
    .map_err(|_| "ACP Registry refresh request failed".to_owned())?
}

fn encode_query_component(value: &str) -> String {
    let mut output = String::new();
    super::append_query_component(&mut output, value);
    output
}
