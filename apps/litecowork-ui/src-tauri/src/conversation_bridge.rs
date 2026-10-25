use super::*;
use std::io::Read;

const MAX_CONVERSATION_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_CONVERSATION_CURSOR_BYTES: usize = 2048;

fn conversation_create_journal(
    app: &AppHandle,
) -> Result<super::conversation_create_journal::ConversationCreateJournal, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "LiteCowork application data directory is unavailable".to_owned())?;
    Ok(super::conversation_create_journal::ConversationCreateJournal::new(app_data_dir))
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_cursor(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CONVERSATION_CURSOR_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_rich_presentation_response(
    value: &serde_json::Value,
    workspace_id: &str,
    conversation_id: &str,
    message_id: &str,
    presentation_id: &str,
    semantic_content_digest: &str,
) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 10
        && object.contains_key("workspace_id")
        && object.contains_key("conversation_id")
        && object.contains_key("message_id")
        && object.contains_key("presentation_id")
        && object.contains_key("schema_version")
        && object.contains_key("renderer_contract_version")
        && object.contains_key("semantic_content_digest")
        && object.contains_key("document_digest")
        && object.contains_key("document_size_bytes")
        && object.contains_key("document")
        && value
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            == Some(workspace_id)
        && value
            .get("conversation_id")
            .and_then(serde_json::Value::as_str)
            == Some(conversation_id)
        && value.get("message_id").and_then(serde_json::Value::as_str) == Some(message_id)
        && value
            .get("presentation_id")
            .and_then(serde_json::Value::as_str)
            == Some(presentation_id)
        && value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            == Some(1)
        && value
            .get("renderer_contract_version")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|version| version >= 1)
        && value
            .get("semantic_content_digest")
            .and_then(serde_json::Value::as_str)
            == Some(semantic_content_digest)
        && value
            .get("document_digest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(valid_digest)
        && value
            .get("document_size_bytes")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|size| (1..=1_048_576).contains(&size))
        && value
            .get("document")
            .is_some_and(serde_json::Value::is_object)
}

fn read_json(response: LocalOperatorResponse) -> Result<serde_json::Value, String> {
    let status = response.status();
    let mut bytes = Vec::new();
    response
        .take(MAX_CONVERSATION_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Local Conversation response could not be read".to_owned())?;
    if bytes.len() > MAX_CONVERSATION_RESPONSE_BYTES {
        return Err("Local Conversation response exceeds its size limit".to_owned());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Local Runtime returned an unsupported Conversation response".to_owned())?;
    if !(200..300).contains(&status) {
        let message = value
            .pointer("/error/message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Local Conversation operation failed");
        return Err(message.to_owned());
    }
    Ok(value)
}

fn clear_pending_create(
    journal: &super::conversation_create_journal::ConversationCreateJournal,
    workspace_id: &str,
    request_id: &str,
) -> Result<(), String> {
    journal
        .acknowledge(workspace_id, request_id)
        .map_err(|_| "Pending Conversation retry record could not be cleared".to_owned())
}

fn initialize_create_client<T>(
    journal: &super::conversation_create_journal::ConversationCreateJournal,
    workspace_id: &str,
    request_id: &str,
    initialize: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    match initialize() {
        Ok(client) => Ok(client),
        Err(error) => {
            clear_pending_create(journal, workspace_id, request_id).map_err(|cleanup_error| {
                format!("{error}; pending retry record could not be cleared: {cleanup_error}")
            })?;
            Err(error)
        }
    }
}

fn read_create_response(
    response: LocalOperatorResponse,
    journal: &super::conversation_create_journal::ConversationCreateJournal,
    workspace_id: &str,
    request_id: &str,
) -> Result<serde_json::Value, String> {
    let status = response.status();
    let result = read_json(response);
    if (400..500).contains(&status) {
        clear_pending_create(journal, workspace_id, request_id)?;
    }
    result
}

#[tauri::command]
pub(crate) async fn list_conversations(
    app: AppHandle,
    workspace_id: String,
    cursor: Option<String>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id) {
            return Err("Conversation Workspace selection is invalid".to_owned());
        }
        if cursor.as_ref().is_some_and(|value| !valid_cursor(value)) {
            return Err("Conversation pagination cursor is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let mut request = client
            .get("/v1/conversations")
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("limit", "50")]);
        if let Some(cursor) = cursor {
            request = request.query(&[("cursor", cursor)]);
        }
        let value = read_json(
            request
                .send()
                .map_err(|_| "Local Conversations are unavailable".to_owned())?,
        )?;
        let items = value
            .get("items")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "Local Runtime returned an invalid Conversation list".to_owned())?;
        if items.iter().any(|item| {
            item.get("workspace_id").and_then(serde_json::Value::as_str)
                != Some(workspace_id.as_str())
        }) {
            return Err(
                "Local Runtime returned a Conversation outside the selected Workspace".to_owned(),
            );
        }
        Ok(value)
    })
    .await
    .map_err(|_| "Conversation list request did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn list_pending_conversation_creates(
    app: AppHandle,
) -> Result<Vec<super::conversation_create_journal::PendingConversationCreate>, String> {
    tauri::async_runtime::spawn_blocking(move || conversation_create_journal(&app)?.list())
        .await
        .map_err(|_| "Pending Conversation requests could not be loaded".to_owned())?
}

#[tauri::command]
pub(crate) async fn acknowledge_conversation_create(
    app: AppHandle,
    workspace_id: String,
    request_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        conversation_create_journal(&app)?.acknowledge(&workspace_id, &request_id)
    })
    .await
    .map_err(|_| "Conversation creation acknowledgement did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn create_conversation(
    app: AppHandle,
    workspace_id: String,
    title: Option<String>,
    request_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let title = super::conversation_create_journal::normalize_title(title)?;
        if !valid_id(&workspace_id)
            || !super::conversation_create_journal::valid_request_id(&request_id)
        {
            return Err("Conversation creation request is invalid".to_owned());
        }
        let journal = conversation_create_journal(&app)?;
        journal.record(
            super::conversation_create_journal::PendingConversationCreate {
                workspace_id: workspace_id.clone(),
                title: title.clone(),
                request_id: request_id.clone(),
            },
        )?;
        let (client, _) = initialize_create_client(&journal, &workspace_id, &request_id, || {
            operator_client(&app)
        })?;
        let response = client
            .post("/v1/conversations")
            .header("X-Workspace-ID", &workspace_id)
            .header("Idempotency-Key", &request_id)
            .json(&serde_json::json!({"workspace_id":workspace_id,"title":title}))
            .send()
            .map_err(|_| "Local Conversation service is unavailable".to_owned())?;
        let value = read_create_response(response, &journal, &workspace_id, &request_id)?;
        if value
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            != Some(workspace_id.as_str())
            || value
                .get("conversation_id")
                .and_then(serde_json::Value::as_str)
                .is_none()
        {
            return Err(
                "Local Runtime returned a Conversation outside the selected Workspace".to_owned(),
            );
        }
        Ok(value)
    })
    .await
    .map_err(|_| "Conversation creation request did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn get_conversation(
    app: AppHandle,
    workspace_id: String,
    conversation_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id) || !valid_id(&conversation_id) {
            return Err("Conversation selection is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let value = read_json(
            client
                .get(format!("/v1/conversations/{conversation_id}"))
                .header("X-Workspace-ID", &workspace_id)
                .send()
                .map_err(|_| "Local Conversation service is unavailable".to_owned())?,
        )?;
        if value.get("workspace_id").and_then(serde_json::Value::as_str)
            != Some(workspace_id.as_str())
            || value.get("conversation_id").and_then(serde_json::Value::as_str)
                != Some(conversation_id.as_str())
        {
            return Err("Local Runtime returned a Conversation outside the selected Workspace".to_owned());
        }
        Ok(value)
    })
    .await
    .map_err(|_| "Conversation read request did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn set_conversation_agent_binding(
    app: AppHandle,
    workspace_id: String,
    conversation_id: String,
    agent_binding_id: Option<String>,
    expected_version: u64,
    request_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !valid_id(&conversation_id)
            || agent_binding_id.as_ref().is_some_and(|id| !valid_id(id))
            || expected_version == 0
            || expected_version > i64::MAX as u64
            || !super::conversation_create_journal::valid_request_id(&request_id)
        {
            return Err("Conversation AgentBinding request is invalid".to_owned());
        }
        let expected_next_version = expected_version
            .checked_add(1)
            .ok_or_else(|| "Conversation version is exhausted".to_owned())?;
        let (client, _) = operator_client(&app)?;
        let value = read_json(
            client
                .patch(format!("/v1/conversations/{conversation_id}/agent-binding"))
                .header("X-Workspace-ID", &workspace_id)
                .header("If-Match", expected_version.to_string())
                .header("Idempotency-Key", &request_id)
                .json(&serde_json::json!({"agent_binding_id":agent_binding_id}))
                .send()
                .map_err(|_| "Local Conversation service is unavailable".to_owned())?,
        )?;
        let selected_binding_matches = match agent_binding_id.as_deref() {
            Some(binding_id) => value
                .get("active_agent_binding_id")
                .and_then(serde_json::Value::as_str)
                == Some(binding_id),
            None => value
                .get("active_agent_binding_id")
                .is_some_and(serde_json::Value::is_null),
        };
        if value.get("workspace_id").and_then(serde_json::Value::as_str)
            != Some(workspace_id.as_str())
            || value.get("conversation_id").and_then(serde_json::Value::as_str)
                != Some(conversation_id.as_str())
            || value.get("version").and_then(serde_json::Value::as_u64)
                != Some(expected_next_version)
            || !selected_binding_matches
        {
            return Err("Local Runtime returned an unexpected Conversation selection".to_owned());
        }
        Ok(value)
    })
    .await
    .map_err(|_| "Conversation AgentBinding request did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn get_conversation_presentation(
    app: AppHandle,
    workspace_id: String,
    conversation_id: String,
    cursor: Option<String>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !valid_id(&conversation_id)
            || cursor.as_ref().is_some_and(|value| !valid_id(value))
        {
            return Err("Conversation selection is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let mut request = client
            .get(format!("/v1/conversations/{conversation_id}/presentation"))
            .header("X-Workspace-ID", &workspace_id)
            .query(&[("limit", "50")]);
        if let Some(cursor) = cursor {
            request = request.query(&[("cursor", cursor)]);
        }
        let value = read_json(
            request
                .send()
                .map_err(|_| "Local Conversation history is unavailable".to_owned())?,
        )?;
        if value
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            != Some(workspace_id.as_str())
            || value
                .get("conversation_id")
                .and_then(serde_json::Value::as_str)
                != Some(conversation_id.as_str())
            || !value.get("items").is_some_and(serde_json::Value::is_array)
        {
            return Err(
                "Local Runtime returned history outside the selected Conversation".to_owned(),
            );
        }
        Ok(value)
    })
    .await
    .map_err(|_| "Conversation history request did not complete".to_owned())?
}

#[tauri::command]
pub(crate) async fn get_rich_presentation(
    app: AppHandle,
    workspace_id: String,
    conversation_id: String,
    message_id: String,
    presentation_id: String,
    semantic_content_digest: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_id(&workspace_id)
            || !valid_id(&conversation_id)
            || !valid_id(&message_id)
            || !valid_id(&presentation_id)
            || !valid_digest(&semantic_content_digest)
        {
            return Err("Rich presentation selection is invalid".to_owned());
        }
        let (client, _) = operator_client(&app)?;
        let value = read_json(
            client
                .get(format!("/v1/rich-presentations/{presentation_id}"))
                .header("X-Workspace-ID", &workspace_id)
                .send()
                .map_err(|_| "Local RichPresentation is unavailable".to_owned())?,
        )?;
        if !valid_rich_presentation_response(
            &value,
            &workspace_id,
            &conversation_id,
            &message_id,
            &presentation_id,
            &semantic_content_digest,
        ) {
            return Err(
                "Local Runtime returned a RichPresentation outside the selected Message".to_owned(),
            );
        }
        Ok(value)
    })
    .await
    .map_err(|_| "RichPresentation request did not complete".to_owned())?
}

#[cfg(test)]
mod rich_presentation_response_tests {
    use super::*;

    fn response() -> serde_json::Value {
        serde_json::json!({
            "workspace_id":"workspace-1", "conversation_id":"conversation-1", "message_id":"message-1",
            "presentation_id":"presentation-1", "schema_version":1, "renderer_contract_version":1,
            "semantic_content_digest":format!("sha256:{}", "a".repeat(64)),
            "document_digest":format!("sha256:{}", "b".repeat(64)), "document_size_bytes":128,
            "document":{"schema_version":1}
        })
    }

    #[test]
    fn rich_presentation_response_is_bound_to_selected_records_and_closed() {
        let expected_digest = format!("sha256:{}", "a".repeat(64));
        let value = response();
        assert!(valid_rich_presentation_response(
            &value,
            "workspace-1",
            "conversation-1",
            "message-1",
            "presentation-1",
            &expected_digest
        ));
        let mut foreign = value.clone();
        foreign["workspace_id"] = serde_json::json!("workspace-2");
        assert!(!valid_rich_presentation_response(
            &foreign,
            "workspace-1",
            "conversation-1",
            "message-1",
            "presentation-1",
            &expected_digest
        ));
        let mut unexpected = value;
        unexpected["agent_session_id"] = serde_json::json!("private");
        assert!(!valid_rich_presentation_response(
            &unexpected,
            "workspace-1",
            "conversation-1",
            "message-1",
            "presentation-1",
            &expected_digest
        ));
    }

    #[test]
    fn rich_presentation_route_ids_and_digests_are_path_safe() {
        assert!(valid_id("presentation_1-2"));
        assert!(!valid_id("../other"));
        assert!(!valid_id("presentation/other"));
        assert!(valid_digest(&format!("sha256:{}", "0".repeat(64))));
        assert!(!valid_digest(&format!("sha256:{}", "A".repeat(64))));
    }
}

#[cfg(test)]
mod conversation_cursor_tests {
    use super::valid_cursor;

    #[test]
    fn conversation_cursor_is_bounded_and_url_safe() {
        assert!(valid_cursor("eyJ3b3Jrc3BhY2VfaWQiOiJ3b3Jrc3BhY2UtMSJ9"));
        assert!(valid_cursor(&"a".repeat(2048)));
        assert!(!valid_cursor(""));
        assert!(!valid_cursor("../another-workspace"));
        assert!(!valid_cursor("cursor=value"));
        assert!(!valid_cursor(&"a".repeat(2049)));
    }
}

#[cfg(test)]
mod create_recovery_tests {
    use super::*;
    use crate::conversation_create_journal::{
        ConversationCreateJournal, PendingConversationCreate,
    };
    use operator_ipc::ResponseHeader;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "litecowork-conversation-bridge-{}-{}",
                std::process::id(),
                TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create unique temporary directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn pending() -> PendingConversationCreate {
        PendingConversationCreate {
            workspace_id: "workspace-1".to_owned(),
            title: Some("Draft".to_owned()),
            request_id: "request-1".to_owned(),
        }
    }

    fn local_response(status: u16, body: &str) -> LocalOperatorResponse {
        let budget = InFlightBodyBudget::new(body.len().max(1)).expect("create response budget");
        let frame = ResponseFrame::new(
            ResponseHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: "test-response".to_owned(),
                status,
                headers: Vec::new(),
                body_length: body.len() as u64,
            },
            body.as_bytes().to_vec(),
            &budget,
        )
        .expect("construct bounded local response");
        let (header, body) = frame.into_parts();
        LocalOperatorResponse {
            status: header.status,
            headers: header.headers,
            body,
            body_offset: 0,
        }
    }

    #[test]
    fn confirmed_client_rejection_clears_only_the_exact_pending_request() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        let rejected = pending();
        let other = PendingConversationCreate {
            workspace_id: "workspace-2".to_owned(),
            title: Some("Draft".to_owned()),
            request_id: "request-2".to_owned(),
        };
        journal
            .record(rejected.clone())
            .expect("pending rejected request");
        journal
            .record(other.clone())
            .expect("unrelated pending request");

        let error = read_create_response(
            local_response(422, r#"{"error":{"message":"title rejected"}}"#),
            &journal,
            &rejected.workspace_id,
            &rejected.request_id,
        )
        .expect_err("client rejection is returned to the UI");

        assert_eq!(error, "title rejected");
        assert_eq!(
            journal.list().expect("unrelated entry remains"),
            vec![other]
        );
    }

    #[test]
    fn server_failure_and_malformed_success_preserve_the_ambiguous_request() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        let request = pending();
        journal.record(request.clone()).expect("pending request");

        let transport_failure: Result<LocalOperatorResponse, String> =
            Err("connection closed after request send".to_owned());
        assert!(transport_failure
            .and_then(|response| {
                read_create_response(
                    response,
                    &journal,
                    &request.workspace_id,
                    &request.request_id,
                )
            })
            .is_err());
        assert_eq!(
            journal.list().expect("transport failure keeps request"),
            vec![request.clone()]
        );

        assert!(read_create_response(
            local_response(503, r#"{"error":{"message":"unavailable"}}"#),
            &journal,
            &request.workspace_id,
            &request.request_id,
        )
        .is_err());
        assert_eq!(
            journal.list().expect("5xx keeps request"),
            vec![request.clone()]
        );

        assert!(read_create_response(
            local_response(200, "not-json"),
            &journal,
            &request.workspace_id,
            &request.request_id,
        )
        .is_err());
        assert_eq!(
            journal.list().expect("malformed success keeps request"),
            vec![request]
        );
    }

    #[test]
    fn local_operator_client_initialization_failure_clears_before_any_post() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        let request = pending();
        journal
            .record(request.clone())
            .expect("journal before client setup");
        let mut post_started = false;

        let initialized: Result<(), String> =
            initialize_create_client(&journal, &request.workspace_id, &request.request_id, || {
                Err("Local Operator initialization failed".to_owned())
            });
        if initialized.is_ok() {
            post_started = true;
        }

        assert_eq!(
            initialized.unwrap_err(),
            "Local Operator initialization failed"
        );
        assert!(!post_started);
        assert!(journal
            .list()
            .expect("preflight failure clears request")
            .is_empty());
    }
}
