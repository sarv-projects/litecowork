use serde::{Deserialize, Serialize};
use tauri::AppHandle;

const MAX_USER_REQUEST_ID_BYTES: usize = 512;
const MAX_USER_REQUEST_PROMPT_BYTES: usize = 256 * 1024;
const MAX_USER_REQUEST_CHOICES_BYTES: usize = 512 * 1024;
const MAX_USER_REQUEST_DETAIL_BYTES: usize = 900 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UserRequestDetailWire {
    request_id: String,
    workspace_id: String,
    scope: UserRequestScopeWire,
    kind: String,
    interaction_mode: String,
    prompt: String,
    choices: Vec<UserRequestChoiceWire>,
    status: String,
    version: u64,
    created_at: String,
    #[serde(deserialize_with = "required_nullable")]
    expires_at: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    resolved_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
enum UserRequestScopeWire {
    Conversation {
        conversation_id: String,
        conversation_turn_id: String,
    },
    TaskPlanning {
        task_id: String,
    },
    AttemptExecution {
        task_id: String,
        attempt_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UserRequestChoiceWire {
    choice_id: String,
    label: String,
    #[serde(deserialize_with = "required_nullable")]
    description: Option<String>,
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
enum UserRequestScopeView {
    Conversation {
        conversation_id: String,
        conversation_turn_id: String,
    },
    TaskPlanning {
        task_id: String,
    },
    AttemptExecution {
        task_id: String,
        attempt_id: String,
    },
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserRequestChoiceView {
    choice_id: String,
    label: String,
    description: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserRequestDetailView {
    request_id: String,
    workspace_id: String,
    scope: UserRequestScopeView,
    kind: String,
    interaction_mode: String,
    prompt: String,
    choices: Vec<UserRequestChoiceView>,
    status: String,
    version: u64,
    created_at: String,
    expires_at: Option<String>,
    resolved_at: Option<String>,
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_USER_REQUEST_ID_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !(20..=64).contains(&bytes.len()) || !value.is_ascii() {
        return false;
    }
    let digit = |index: usize| bytes.get(index).is_some_and(u8::is_ascii_digit);
    if !(0..4).all(digit)
        || bytes.get(4) != Some(&b'-')
        || !(5..7).all(digit)
        || bytes.get(7) != Some(&b'-')
        || !(8..10).all(digit)
        || !matches!(bytes.get(10), Some(b'T' | b't'))
        || !(11..13).all(digit)
        || bytes.get(13) != Some(&b':')
        || !(14..16).all(digit)
        || bytes.get(16) != Some(&b':')
        || !(17..19).all(digit)
    {
        return false;
    }
    let month = u8::from_str_radix(&value[5..7], 10).unwrap_or(0);
    let day = u8::from_str_radix(&value[8..10], 10).unwrap_or(0);
    let hour = u8::from_str_radix(&value[11..13], 10).unwrap_or(255);
    let minute = u8::from_str_radix(&value[14..16], 10).unwrap_or(255);
    let second = u8::from_str_radix(&value[17..19], 10).unwrap_or(255);
    if !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(value[0..4].parse().unwrap_or(0), month)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return false;
    }
    let mut index = 19;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while digit(index) {
            index += 1;
        }
        if index == fraction_start {
            return false;
        }
    }
    match bytes.get(index) {
        Some(b'Z' | b'z') if index + 1 == bytes.len() => true,
        Some(b'+' | b'-') if index + 6 == bytes.len() => {
            digit(index + 1)
                && digit(index + 2)
                && bytes.get(index + 3) == Some(&b':')
                && digit(index + 4)
                && digit(index + 5)
                && value[index + 1..index + 3]
                    .parse::<u8>()
                    .is_ok_and(|hour| hour <= 23)
                && value[index + 4..index + 6]
                    .parse::<u8>()
                    .is_ok_and(|minute| minute <= 59)
        }
        _ => false,
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn map_user_request_detail(
    requested_workspace_id: &str,
    requested_request_id: &str,
    detail: UserRequestDetailWire,
) -> Result<UserRequestDetailView, String> {
    let scope = match detail.scope {
        UserRequestScopeWire::Conversation {
            conversation_id,
            conversation_turn_id,
        } if valid_id(&conversation_id) && valid_id(&conversation_turn_id) => {
            UserRequestScopeView::Conversation {
                conversation_id,
                conversation_turn_id,
            }
        }
        UserRequestScopeWire::TaskPlanning { task_id } if valid_id(&task_id) => {
            UserRequestScopeView::TaskPlanning { task_id }
        }
        UserRequestScopeWire::AttemptExecution {
            task_id,
            attempt_id,
        } if valid_id(&task_id) && valid_id(&attempt_id) => {
            UserRequestScopeView::AttemptExecution {
                task_id,
                attempt_id,
            }
        }
        _ => return Err("Local Runtime returned an invalid UserRequest scope".to_owned()),
    };
    let valid_kind = matches!(
        detail.kind.as_str(),
        "QUESTION" | "DECISION" | "RESOURCE_SELECTION" | "EXTERNAL_AUTHORIZATION"
    );
    let valid_status = matches!(
        detail.status.as_str(),
        "PENDING" | "ANSWERED" | "DISMISSED" | "EXPIRED" | "CANCELLED"
    );
    let choices_bytes = serde_json::to_vec(&detail.choices)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX);
    let choices_valid = detail.choices.len() <= 500
        && detail.choices.iter().all(|choice| {
            valid_id(&choice.choice_id)
                && !choice.label.trim().is_empty()
                && choice.label.len() <= 16_384
                && choice
                    .description
                    .as_ref()
                    .is_none_or(|value| value.len() <= 16_384)
        })
        && choices_bytes <= MAX_USER_REQUEST_CHOICES_BYTES;
    let interaction_valid = match detail.interaction_mode.as_str() {
        "FORM" => detail.kind != "EXTERNAL_AUTHORIZATION" && choices_valid,
        "EXTERNAL_URL" => detail.kind == "EXTERNAL_AUTHORIZATION" && detail.choices.is_empty(),
        _ => false,
    };
    if !valid_id(&requested_workspace_id)
        || !valid_id(&requested_request_id)
        || detail.workspace_id != requested_workspace_id
        || detail.request_id != requested_request_id
        || !valid_kind
        || !valid_status
        || !interaction_valid
        || detail.version == 0
        || detail.prompt.trim().is_empty()
        || detail.prompt.len() > MAX_USER_REQUEST_PROMPT_BYTES
        || !valid_timestamp(&detail.created_at)
        || detail
            .expires_at
            .as_ref()
            .is_some_and(|value| !valid_timestamp(value))
        || detail
            .resolved_at
            .as_ref()
            .is_some_and(|value| !valid_timestamp(value))
    {
        return Err("Local Runtime returned invalid UserRequest details".to_owned());
    }
    let view = UserRequestDetailView {
        request_id: detail.request_id,
        workspace_id: detail.workspace_id,
        scope,
        kind: detail.kind,
        interaction_mode: detail.interaction_mode,
        prompt: detail.prompt,
        choices: detail
            .choices
            .into_iter()
            .map(|choice| UserRequestChoiceView {
                choice_id: choice.choice_id,
                label: choice.label,
                description: choice.description,
            })
            .collect(),
        status: detail.status,
        version: detail.version,
        created_at: detail.created_at,
        expires_at: detail.expires_at,
        resolved_at: detail.resolved_at,
    };
    if serde_json::to_vec(&view)
        .map_err(|_| "Local Runtime returned malformed UserRequest details".to_owned())?
        .len()
        > MAX_USER_REQUEST_DETAIL_BYTES
    {
        return Err("Local Runtime returned oversized UserRequest details".to_owned());
    }
    Ok(view)
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
pub(crate) async fn get_user_request_detail(
    app: AppHandle,
    workspace_id: String,
    request_id: String,
) -> Result<UserRequestDetailView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id) || !valid_id(&request_id) {
            return Err("UserRequest selection is invalid".to_owned());
        }
        let (client, workspace_url) = super::operator_client(&app)?;
        let base_url = workspace_url.trim_end_matches("/workspaces");
        let response = client
            .get(format!(
                "{base_url}/user-requests/{}",
                encode_path_segment(&request_id)
            ))
            .header("X-Workspace-ID", &workspace_id)
            .send()
            .map_err(|_| "Local UserRequest details are unavailable".to_owned())?;
        if !response.is_success() {
            return Err("This UserRequest is unavailable in the selected Workspace".to_owned());
        }
        if response.header("cache-control") != Some("no-store") {
            return Err(
                "Local Runtime returned UserRequest details without no-store protection".to_owned(),
            );
        }
        let body = super::read_bounded_response(response)?;
        if body.len() > MAX_USER_REQUEST_DETAIL_BYTES {
            return Err("Local Runtime returned oversized UserRequest details".to_owned());
        }
        let detail: UserRequestDetailWire = serde_json::from_slice(&body)
            .map_err(|_| "Local Runtime returned malformed UserRequest details".to_owned())?;
        map_user_request_detail(&workspace_id, &request_id, detail)
    })
    .await
    .map_err(|_| "UserRequest detail read did not complete".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wire() -> UserRequestDetailWire {
        serde_json::from_value(json!({
            "request_id":"request-1",
            "workspace_id":"workspace-1",
            "scope":{"kind":"TASK_PLANNING","task_id":"task-1"},
            "kind":"QUESTION",
            "interaction_mode":"FORM",
            "prompt":"Choose one",
            "choices":[{"choice_id":"choice-1","label":"Option","description":null}],
            "status":"PENDING",
            "version":2,
            "created_at":"2026-10-16T10:00:00Z",
            "expires_at":null,
            "resolved_at":null
        }))
        .expect("valid detail wire")
    }

    #[test]
    fn bridge_maps_exact_scope_and_current_status_to_frontend_identity() {
        let view =
            map_user_request_detail("workspace-1", "request-1", wire()).expect("valid detail maps");
        let value = serde_json::to_value(view).expect("camelCase view serializes");
        assert_eq!(value["requestId"], "request-1");
        assert_eq!(value["workspaceId"], "workspace-1");
        assert_eq!(value["scope"]["kind"], "TASK_PLANNING");
        assert_eq!(value["scope"]["taskId"], "task-1");
        assert_eq!(value["status"], "PENDING");
        assert_eq!(value["version"], 2);
    }

    #[test]
    fn bridge_rejects_wrong_identity_scope_status_version_and_malformed_shapes() {
        assert!(map_user_request_detail("workspace-other", "request-1", wire()).is_err());
        assert!(map_user_request_detail("workspace-1", "request-other", wire()).is_err());

        let mut malformed_scope = serde_json::to_value(wire()).expect("wire serializes");
        malformed_scope["scope"]["attempt_id"] = json!("foreign-variant");
        assert!(serde_json::from_value::<UserRequestDetailWire>(malformed_scope).is_err());

        let mut invalid_status = wire();
        invalid_status.status = "WAITING".to_owned();
        assert!(map_user_request_detail("workspace-1", "request-1", invalid_status).is_err());
        let mut invalid_version = wire();
        invalid_version.version = 0;
        assert!(map_user_request_detail("workspace-1", "request-1", invalid_version).is_err());
        let mut invalid_time = wire();
        invalid_time.created_at = "yesterday".to_owned();
        assert!(map_user_request_detail("workspace-1", "request-1", invalid_time).is_err());
    }

    #[test]
    fn bridge_accepts_exact_needs_you_id_and_prompt_bounds_and_rejects_over_bounds() {
        let mut exact_id = wire();
        exact_id.request_id = "r".repeat(MAX_USER_REQUEST_ID_BYTES);
        let exact_id_value = exact_id.request_id.clone();
        assert!(map_user_request_detail("workspace-1", &exact_id_value, exact_id).is_ok());

        let mut long_id = wire();
        long_id.request_id = "r".repeat(MAX_USER_REQUEST_ID_BYTES + 1);
        assert!(
            map_user_request_detail("workspace-1", &long_id.request_id.clone(), long_id).is_err()
        );

        let mut exact_prompt = wire();
        exact_prompt.prompt = "p".repeat(MAX_USER_REQUEST_PROMPT_BYTES);
        assert!(map_user_request_detail("workspace-1", "request-1", exact_prompt).is_ok());

        let mut long_prompt = wire();
        long_prompt.prompt = "p".repeat(MAX_USER_REQUEST_PROMPT_BYTES + 1);
        assert!(map_user_request_detail("workspace-1", "request-1", long_prompt).is_err());
    }

    #[test]
    fn bridge_enforces_choices_and_total_response_byte_limits_at_the_boundary() {
        let make_choices = |target_bytes: usize| {
            let mut choices = (0..32)
                .map(|index| UserRequestChoiceWire {
                    choice_id: format!("choice-{index}"),
                    label: "x".repeat(15_000),
                    description: None,
                })
                .collect::<Vec<_>>();
            let current_bytes = serde_json::to_vec(&choices)
                .expect("baseline choices serialize")
                .len();
            let mut remaining = target_bytes
                .checked_sub(current_bytes)
                .expect("baseline fits budget");
            for choice in &mut choices {
                let addition = remaining.min(16_384 - choice.label.len());
                choice.label.push_str(&"x".repeat(addition));
                remaining -= addition;
            }
            assert_eq!(
                remaining, 0,
                "choice labels can reach the target byte boundary"
            );
            choices
        };
        let mut exact_choices = wire();
        exact_choices.choices = make_choices(MAX_USER_REQUEST_CHOICES_BYTES);
        assert_eq!(
            serde_json::to_vec(&exact_choices.choices)
                .expect("exact choices serialize")
                .len(),
            MAX_USER_REQUEST_CHOICES_BYTES
        );
        assert!(map_user_request_detail("workspace-1", "request-1", exact_choices).is_ok());

        let mut long_choices = wire();
        long_choices.choices = make_choices(MAX_USER_REQUEST_CHOICES_BYTES + 1);
        assert_eq!(
            serde_json::to_vec(&long_choices.choices)
                .expect("over-limit choices serialize")
                .len(),
            MAX_USER_REQUEST_CHOICES_BYTES + 1
        );
        assert!(map_user_request_detail("workspace-1", "request-1", long_choices).is_err());

        let mut exact_total = wire();
        exact_total.prompt = "x".to_owned();
        let base = map_user_request_detail("workspace-1", "request-1", exact_total.clone())
            .expect("base view maps");
        let base_size = serde_json::to_vec(&base)
            .expect("base view serializes")
            .len();
        let target_delta = MAX_USER_REQUEST_DETAIL_BYTES - base_size + 1;
        let (nulls, ascii) = (0..MAX_USER_REQUEST_PROMPT_BYTES)
            .find_map(|nulls| {
                let remainder = target_delta.checked_sub(nulls * 6)?;
                let prompt_len = nulls + remainder;
                (prompt_len < MAX_USER_REQUEST_PROMPT_BYTES).then_some((nulls, remainder))
            })
            .expect("construct prompt within per-field limit and exactly at total limit");
        exact_total.prompt = format!("{}{}", "\0".repeat(nulls), "a".repeat(ascii));
        let exact_view = map_user_request_detail("workspace-1", "request-1", exact_total.clone())
            .expect("exact total response fits");
        assert_eq!(
            serde_json::to_vec(&exact_view)
                .expect("exact view serializes")
                .len(),
            MAX_USER_REQUEST_DETAIL_BYTES
        );
        exact_total.prompt.push('a');
        assert!(map_user_request_detail("workspace-1", "request-1", exact_total).is_err());
    }

    #[test]
    fn bridge_wire_rejects_provider_url_and_inert_content_remains_plain_data() {
        let mut value = serde_json::to_value(wire()).expect("wire serializes");
        value["provider_url"] = json!("https://secret.example/sign-in");
        assert!(serde_json::from_value::<UserRequestDetailWire>(value).is_err());
        let mut value_with_choice_value = serde_json::to_value(wire()).expect("wire serializes");
        value_with_choice_value["choices"][0]["value"] = json!("secret");
        assert!(serde_json::from_value::<UserRequestDetailWire>(value_with_choice_value).is_err());
        let view =
            map_user_request_detail("workspace-1", "request-1", wire()).expect("valid detail maps");
        assert_eq!(view.choices[0].label, "Option");
        assert!(
            serde_json::to_value(view).expect("view serializes")["choices"][0]
                .get("value")
                .is_none()
        );
    }

    #[test]
    fn bridge_encodes_request_ids_as_one_path_segment() {
        assert_eq!(encode_path_segment("request/part 1"), "request%2Fpart%201");
    }
}
