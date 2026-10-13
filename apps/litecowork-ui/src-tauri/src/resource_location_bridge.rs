use super::*;
use serde::Deserialize;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ResourceLocationView {
    location_id: String,
    resource_id: String,
    runtime_id: Option<String>,
    environment_id: Option<String>,
    connection_id: Option<String>,
    availability: String,
    writable: bool,
    observed_revision_id: Option<String>,
    observed_digest: Option<String>,
    observed_at: String,
    last_checked_at: Option<String>,
    freshness: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceLocationWire {
    location_id: String,
    resource_id: String,
    runtime_id: Option<String>,
    environment_id: Option<String>,
    connection_id: Option<String>,
    availability: String,
    writable: bool,
    observed_revision_id: Option<String>,
    observed_digest: Option<String>,
    observed_at: String,
    last_checked_at: Option<String>,
    freshness: String,
}

#[tauri::command]
pub(crate) async fn list_resource_locations(
    app: AppHandle,
    workspace_id: String,
    resource_id: String,
) -> Result<Vec<ResourceLocationView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        super::validate_resource_selection(&workspace_id, &resource_id)?;
        let (client, workspace_url) = super::operator_client(&app)?;
        let response = client
            .get(format!(
                "{}/resources/{}/locations",
                workspace_url.trim_end_matches("/workspaces"),
                resource_id
            ))
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "Resource locations are unavailable".to_owned())?;
        if !response.is_success() {
            return Err("Resource locations are unavailable in this Workspace".to_owned());
        }
        let body = super::read_bounded_response(response)?;
        decode_resource_locations(&body, &resource_id)
    })
    .await
    .map_err(|_| "Resource locations request did not complete".to_owned())?
}

fn decode_resource_locations(
    body: &[u8],
    expected_resource_id: &str,
) -> Result<Vec<ResourceLocationView>, String> {
    let locations: Vec<ResourceLocationWire> = serde_json::from_slice(body)
        .map_err(|_| "Local Runtime returned unsupported Resource locations".to_owned())?;
    locations
        .into_iter()
        .map(|location| {
            if location.location_id.is_empty()
                || location.resource_id != expected_resource_id
                || !matches!(
                    location.availability.as_str(),
                    "AVAILABLE" | "OFFLINE" | "PLACEHOLDER" | "REVOKED" | "UNKNOWN" | "UNAVAILABLE"
                )
                || !matches!(
                    location.freshness.as_str(),
                    "CURRENT" | "STALE" | "UNKNOWN" | "UNAVAILABLE"
                )
                || location.observed_at.is_empty()
                || location.observed_digest.as_deref().is_some_and(|digest| {
                    digest.len() != 71
                        || !digest.starts_with("sha256:")
                        || !digest[7..]
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
            {
                return Err("Local Runtime returned an invalid Resource location".to_owned());
            }
            Ok(ResourceLocationView {
                location_id: location.location_id,
                resource_id: location.resource_id,
                runtime_id: location.runtime_id,
                environment_id: location.environment_id,
                connection_id: location.connection_id,
                availability: location.availability,
                writable: location.writable,
                observed_revision_id: location.observed_revision_id,
                observed_digest: location.observed_digest,
                observed_at: location.observed_at,
                last_checked_at: location.last_checked_at,
                freshness: location.freshness,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::decode_resource_locations;

    #[test]
    fn decodes_only_public_locations_for_the_requested_resource() {
        let body = br#"[{"location_id":"location-1","resource_id":"resource-1","runtime_id":"runtime-1","environment_id":null,"connection_id":null,"availability":"AVAILABLE","writable":false,"observed_revision_id":"revision-1","observed_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","observed_at":"2026-10-10T12:00:00Z","last_checked_at":null,"freshness":"CURRENT"}]"#;

        let locations = decode_resource_locations(body, "resource-1").unwrap();

        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].resource_id, "resource-1");
        assert_eq!(locations[0].availability, "AVAILABLE");
        assert_eq!(locations[0].freshness, "CURRENT");

        let serialized = serde_json::to_value(&locations[0]).expect("Tauri view serializes");
        assert_eq!(serialized["location_id"], "location-1");
        assert_eq!(serialized["resource_id"], "resource-1");
        assert!(serialized.get("locationId").is_none());
    }

    #[test]
    fn rejects_foreign_resource_private_fields_and_unknown_status_values() {
        let foreign = br#"[{"location_id":"location-1","resource_id":"resource-2","availability":"AVAILABLE","writable":false,"observed_at":"2026-10-10T12:00:00Z","freshness":"CURRENT"}]"#;
        let private = br#"[{"location_id":"location-1","resource_id":"resource-1","locator_ref_id":"private","availability":"AVAILABLE","writable":false,"observed_at":"2026-10-10T12:00:00Z","freshness":"CURRENT"}]"#;
        let unsupported = br#"[{"location_id":"location-1","resource_id":"resource-1","availability":"LIVE_PROBED","writable":false,"observed_at":"2026-10-10T12:00:00Z","freshness":"CURRENT"}]"#;

        assert!(decode_resource_locations(foreign, "resource-1").is_err());
        assert!(decode_resource_locations(private, "resource-1").is_err());
        assert!(decode_resource_locations(unsupported, "resource-1").is_err());
    }
}
