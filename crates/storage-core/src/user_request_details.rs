use crate::StoreError;
use serde::{Deserialize, Serialize};

pub const USER_REQUEST_ID_MAX_BYTES: usize = 512;
pub const USER_REQUEST_PROMPT_MAX_BYTES: usize = 256 * 1024;
pub const USER_REQUEST_CHOICES_MAX_BYTES: usize = 512 * 1024;
pub const USER_REQUEST_DETAIL_MAX_BYTES: usize = 900 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserRequestScope {
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserRequestChoice {
    pub choice_id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Read-only, selected-Workspace detail for one exact persisted UserRequest.
/// This projection intentionally excludes Runtime-local ProviderInputBindings,
/// provider URLs, agent-session handles, and response records.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserRequestDetail {
    pub request_id: String,
    pub workspace_id: String,
    pub scope: UserRequestScope,
    pub kind: String,
    pub interaction_mode: String,
    pub prompt: String,
    pub choices: Vec<UserRequestChoice>,
    pub status: String,
    pub version: u64,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub resolved_at: Option<String>,
}

/// Looks up a UserRequest only within the already-authorized selected Workspace.
/// A missing or foreign-Workspace request is represented by `None` so callers can
/// return the same nondisclosing NOT_FOUND response for both cases.
pub trait UserRequestDetailReadStore: Send + Sync {
    fn get_user_request_detail(
        &self,
        workspace_id: &str,
        request_id: &str,
    ) -> Result<Option<UserRequestDetail>, StoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_wire_shape_is_tagged_and_rejects_fields_from_other_variants() {
        let scope = UserRequestScope::TaskPlanning {
            task_id: "task-1".to_owned(),
        };
        let value = serde_json::to_value(&scope).expect("scope serializes");
        assert_eq!(
            value,
            serde_json::json!({"kind":"TASK_PLANNING","task_id":"task-1"})
        );
        assert!(
            serde_json::from_value::<UserRequestScope>(serde_json::json!({
                "kind":"TASK_PLANNING",
                "task_id":"task-1",
                "attempt_id":"attempt-foreign-variant"
            }))
            .is_err()
        );
    }

    #[test]
    fn detail_wire_model_excludes_provider_binding_fields() {
        let detail = UserRequestDetail {
            request_id: "request-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            scope: UserRequestScope::TaskPlanning {
                task_id: "task-1".to_owned(),
            },
            kind: "QUESTION".to_owned(),
            interaction_mode: "FORM".to_owned(),
            prompt: "Choose an option".to_owned(),
            choices: vec![UserRequestChoice {
                choice_id: "choice-1".to_owned(),
                label: "Option".to_owned(),
                description: None,
            }],
            status: "PENDING".to_owned(),
            version: 1,
            created_at: "2026-10-16T10:00:00Z".to_owned(),
            expires_at: None,
            resolved_at: None,
        };
        let value = serde_json::to_value(detail).expect("detail serializes");
        assert!(value.get("provider_url").is_none());
        assert!(value.get("provider_input_binding").is_none());
        assert!(value.get("agent_session_id").is_none());
        assert!(value["choices"][0].get("value").is_none());
    }
}
