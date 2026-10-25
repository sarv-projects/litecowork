use super::{operator_client, read_bounded_response, AppHandle};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::Serialize;

const MAX_QUERY_BYTES: usize = 512;
const MAX_CURSOR_BYTES: usize = 2048;
const MAX_ITEM_REF_BYTES: usize = 256;
const DISCOVER_PAGE_LIMIT: &str = "50";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiscoverResponse {
    status: u16,
    content_type: String,
    body_base64: String,
}

fn valid_workspace_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_query(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_QUERY_BYTES && !value.chars().any(char::is_control)
}

fn valid_cursor(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CURSOR_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_item_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ITEM_REF_BYTES
        && value != "."
        && value != ".."
        && !value.chars().any(char::is_control)
}

fn valid_kind(value: &str) -> bool {
    ["APP", "CAPABILITY", "SKILL", "AGENT"].contains(&value)
}

fn encode_path_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    encoded
}

/// Narrow, read-only Discover transport. The WebView selects only `search` or
/// `item`; it cannot supply a method, URL, header, install action, or provider.
#[tauri::command]
pub(crate) async fn discover_request(
    app: AppHandle,
    workspace_id: String,
    operation: String,
    query: Option<String>,
    kind: Option<String>,
    cursor: Option<String>,
    item_ref: Option<String>,
) -> Result<DiscoverResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_workspace_id(&workspace_id)
            || query.as_deref().is_some_and(|value| !valid_query(value))
            || kind.as_deref().is_some_and(|value| !valid_kind(value))
            || cursor.as_deref().is_some_and(|value| !valid_cursor(value))
            || item_ref
                .as_deref()
                .is_some_and(|value| !valid_item_ref(value))
        {
            return Err("Discover request is invalid".to_owned());
        }

        let (client, _) = operator_client(&app)?;
        let request = match operation.as_str() {
            "search" if item_ref.is_none() => {
                let mut request = client
                    .get("/v1/discover/search")
                    .header("X-Workspace-ID", &workspace_id);
                if let Some(query) = query.as_deref() {
                    request = request.query(&[("q", query)]);
                }
                if let Some(kind) = kind.as_deref() {
                    request = request.query(&[("kind", kind)]);
                }
                if let Some(cursor) = cursor.as_deref() {
                    request = request.query(&[("cursor", cursor)]);
                }
                request.query(&[("limit", DISCOVER_PAGE_LIMIT)])
            }
            "item" if query.is_none() && kind.is_none() && cursor.is_none() => {
                let item_ref = item_ref
                    .as_deref()
                    .filter(|value| valid_item_ref(value))
                    .ok_or_else(|| "Discover item reference is invalid".to_owned())?;
                client
                    .get(format!(
                        "/v1/discover/items/{}",
                        encode_path_component(item_ref)
                    ))
                    .header("X-Workspace-ID", &workspace_id)
            }
            _ => return Err("Discover operation is unavailable".to_owned()),
        };

        let response = request
            .send()
            .map_err(|_| "Local Discover service is unavailable".to_owned())?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/json")
            .to_owned();
        let body = read_bounded_response(response)?;
        Ok(DiscoverResponse {
            status,
            content_type,
            body_base64: BASE64_STANDARD.encode(body),
        })
    })
    .await
    .map_err(|_| "Discover request did not complete".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::{encode_path_component, valid_cursor, valid_item_ref, valid_query};

    #[test]
    fn item_refs_are_encoded_as_one_path_component() {
        assert_eq!(
            encode_path_component("skill:cleanup/weekly"),
            "skill%3Acleanup%2Fweekly"
        );
        assert!(!valid_item_ref(".."));
        assert!(valid_item_ref("skill:cleanup/weekly"));
    }

    #[test]
    fn query_and_cursor_inputs_are_bounded() {
        assert!(valid_query("invoice cleanup"));
        assert!(!valid_query(""));
        assert!(!valid_query(&"q".repeat(513)));
        assert!(valid_cursor("eyJwYWdlIjoxfQ"));
        assert!(!valid_cursor("cursor/other"));
    }
}
