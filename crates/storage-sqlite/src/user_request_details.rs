use super::{Command, SqliteWorkspaceStore, map_database_error};
use crate::workspace_access::require_read_access;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde::de::IgnoredAny;
use std::sync::mpsc;
use storage_core::{
    StoreError, USER_REQUEST_CHOICES_MAX_BYTES, USER_REQUEST_DETAIL_MAX_BYTES,
    USER_REQUEST_ID_MAX_BYTES, USER_REQUEST_PROMPT_MAX_BYTES, UserRequestChoice, UserRequestDetail,
    UserRequestDetailReadStore, UserRequestScope, UserRequestSummary,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredUserRequestChoice {
    choice_id: String,
    label: String,
    description: Option<String>,
    // The persisted value is required by the create-time contract, but is never
    // materialized in this read-only detail projection.
    #[serde(rename = "value")]
    _value: IgnoredAny,
}

impl SqliteWorkspaceStore {
    /// Reads one owner-scoped page of prompt-free UserRequest summaries.
    ///
    /// Ordering is descending `created_at` and ascending `request_id`; the
    /// all-or-nothing cursor pair follows that exact ordering. The caller asks
    /// for one extra row so the route can decide whether `next_cursor` exists.
    pub fn list_user_requests_page(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        status: Option<&str>,
        after_created_at: Option<&str>,
        after_request_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<UserRequestSummary>, StoreError> {
        if !(1..=101).contains(&limit)
            || owner_principal_id.trim().is_empty()
            || owner_principal_id.len() > 256
            || workspace_id.trim().is_empty()
            || workspace_id.len() > 256
            || status.is_some_and(|value| {
                !matches!(
                    value,
                    "PENDING" | "ANSWERED" | "DISMISSED" | "EXPIRED" | "CANCELLED"
                )
            })
            || after_created_at.is_some() != after_request_id.is_some()
            || after_created_at.is_some_and(|value| !valid_timestamp(value))
            || after_request_id.is_some_and(|value| {
                value.trim().is_empty() || value.len() > USER_REQUEST_ID_MAX_BYTES
            })
        {
            return Err(StoreError::Invalid(
                "UserRequest page query is invalid".to_owned(),
            ));
        }
        let owner = owner_principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let status = status.map(str::to_owned);
        let after = after_created_at
            .zip(after_request_id)
            .map(|(created_at, request_id)| (created_at.to_owned(), request_id.to_owned()));
        let (send, recv) = mpsc::channel();
        self.execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let result = (|| {
                        require_read_access(connection, &workspace, &owner)?;
                        let after_created = after.as_ref().map(|(created_at, _)| created_at);
                        let after_id = after.as_ref().map(|(_, request_id)| request_id);
                        let mut statement = connection
                            .prepare(
                                "SELECT request_id, workspace_id, conversation_id,
                                        conversation_turn_id, task_id, attempt_id, kind,
                                        interaction_mode, status, created_at, expires_at,
                                        resolved_at, version
                                 FROM user_requests
                                 WHERE workspace_id = ?1
                                   AND (?2 IS NULL OR status = ?2)
                                   AND (?3 IS NULL OR created_at < ?3
                                        OR (created_at = ?3 AND request_id > ?4))
                                 ORDER BY created_at DESC, request_id ASC
                                 LIMIT ?5",
                            )
                            .map_err(map_database_error)?;
                        let mut rows = statement
                            .query(params![workspace, status, after_created, after_id, limit as i64])
                            .map_err(map_database_error)?;
                        let mut items = Vec::new();
                        while let Some(row) = rows.next().map_err(map_database_error)? {
                            let request_id: String = row.get(0).map_err(map_database_error)?;
                            let workspace_id: String = row.get(1).map_err(map_database_error)?;
                            let conversation_id: Option<String> =
                                row.get(2).map_err(map_database_error)?;
                            let conversation_turn_id: Option<String> =
                                row.get(3).map_err(map_database_error)?;
                            let task_id: Option<String> = row.get(4).map_err(map_database_error)?;
                            let attempt_id: Option<String> =
                                row.get(5).map_err(map_database_error)?;
                            let kind: String = row.get(6).map_err(map_database_error)?;
                            let interaction_mode: String =
                                row.get(7).map_err(map_database_error)?;
                            let status: String = row.get(8).map_err(map_database_error)?;
                            let created_at: String = row.get(9).map_err(map_database_error)?;
                            let expires_at: Option<String> =
                                row.get(10).map_err(map_database_error)?;
                            let resolved_at: Option<String> =
                                row.get(11).map_err(map_database_error)?;
                            let version: i64 = row.get(12).map_err(map_database_error)?;
                            validate_id(&request_id, "UserRequest ID")
                                .map_err(as_integrity_error)?;
                            validate_id(&workspace_id, "Workspace ID")
                                .map_err(as_integrity_error)?;
                            if workspace_id != workspace {
                                return Err(StoreError::Integrity(
                                    "UserRequest Workspace changed during the page read"
                                        .to_owned(),
                                ));
                            }
                            if !matches!(
                                kind.as_str(),
                                "QUESTION"
                                    | "DECISION"
                                    | "RESOURCE_SELECTION"
                                    | "EXTERNAL_AUTHORIZATION"
                            ) || !matches!(
                                status.as_str(),
                                "PENDING" | "ANSWERED" | "DISMISSED" | "EXPIRED" | "CANCELLED"
                            ) || version < 1
                                || !valid_timestamp(&created_at)
                                || expires_at
                                    .as_deref()
                                    .is_some_and(|value| !valid_timestamp(value))
                                || resolved_at
                                    .as_deref()
                                    .is_some_and(|value| !valid_timestamp(value))
                            {
                                return Err(StoreError::Integrity(
                                    "UserRequest row is malformed".to_owned(),
                                ));
                            }
                            items.push(UserRequestSummary {
                                request_id,
                                workspace_id,
                                scope: user_request_scope(
                                    conversation_id,
                                    conversation_turn_id,
                                    task_id,
                                    attempt_id,
                                )?,
                                kind,
                                interaction_mode,
                                status,
                                created_at,
                                expires_at,
                                resolved_at,
                                version: u64::try_from(version).map_err(|_| {
                                    StoreError::Integrity(
                                        "UserRequest version is not positive".to_owned(),
                                    )
                                })?,
                            });
                        }
                        Ok(items)
                    })();
                    let _ = send.send(result);
                }),
            },
            recv,
        )
    }
}

fn user_request_scope(
    conversation_id: Option<String>,
    conversation_turn_id: Option<String>,
    task_id: Option<String>,
    attempt_id: Option<String>,
) -> Result<UserRequestScope, StoreError> {
    match (conversation_id, conversation_turn_id, task_id, attempt_id) {
        (Some(conversation_id), Some(conversation_turn_id), None, None) => {
            validate_id(&conversation_id, "Conversation ID").map_err(as_integrity_error)?;
            validate_id(&conversation_turn_id, "Conversation turn ID")
                .map_err(as_integrity_error)?;
            Ok(UserRequestScope::Conversation {
                conversation_id,
                conversation_turn_id,
            })
        }
        (None, None, Some(task_id), None) => {
            validate_id(&task_id, "Task ID").map_err(as_integrity_error)?;
            Ok(UserRequestScope::TaskPlanning { task_id })
        }
        (None, None, Some(task_id), Some(attempt_id)) => {
            validate_id(&task_id, "Task ID").map_err(as_integrity_error)?;
            validate_id(&attempt_id, "Attempt ID").map_err(as_integrity_error)?;
            Ok(UserRequestScope::AttemptExecution {
                task_id,
                attempt_id,
            })
        }
        _ => Err(StoreError::Integrity(
            "UserRequest scope is malformed".to_owned(),
        )),
    }
}

impl UserRequestDetailReadStore for SqliteWorkspaceStore {
    fn get_user_request_detail(
        &self,
        workspace_id: &str,
        request_id: &str,
    ) -> Result<Option<UserRequestDetail>, StoreError> {
        validate_id(workspace_id, "Workspace ID")?;
        validate_id(request_id, "UserRequest ID")?;
        let (reply, receiver) = mpsc::channel();
        self.execute_command(
            Command::GetUserRequestDetail {
                workspace_id: workspace_id.to_owned(),
                request_id: request_id.to_owned(),
                reply,
            },
            receiver,
        )
    }
}

pub(super) fn get_user_request_detail(
    connection: &Connection,
    workspace_id: &str,
    request_id: &str,
) -> Result<Option<UserRequestDetail>, StoreError> {
    validate_id(workspace_id, "Workspace ID")?;
    validate_id(request_id, "UserRequest ID")?;
    let raw = connection
        .query_row(
            "SELECT request_id, workspace_id, conversation_id, conversation_turn_id,
                    task_id, attempt_id, kind, interaction_mode, prompt, choices_json,
                    status, created_at, expires_at, resolved_at, version
             FROM user_requests
             WHERE workspace_id = ?1 AND request_id = ?2",
            params![workspace_id, request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, i64>(14)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        stored_request_id,
        stored_workspace_id,
        conversation_id,
        conversation_turn_id,
        task_id,
        attempt_id,
        kind,
        interaction_mode,
        prompt,
        choices_json,
        status,
        created_at,
        expires_at,
        resolved_at,
        version,
    )) = raw
    else {
        return Ok(None);
    };

    // Each caller identity is checked against the row returned by the same query.
    // The SQL scope predicate means a request from another Workspace is indistinguishable
    // from an absent request to this selected Workspace.
    if stored_request_id != request_id || stored_workspace_id != workspace_id {
        return Ok(None);
    }
    validate_id(&stored_request_id, "UserRequest ID").map_err(as_integrity_error)?;
    validate_id(&stored_workspace_id, "Workspace ID").map_err(as_integrity_error)?;
    if prompt.trim().is_empty() || prompt.len() > USER_REQUEST_PROMPT_MAX_BYTES {
        return Err(StoreError::Integrity(
            "UserRequest prompt exceeds its read boundary".to_owned(),
        ));
    }
    if choices_json.len() > USER_REQUEST_CHOICES_MAX_BYTES {
        return Err(StoreError::Integrity(
            "UserRequest choices exceed their read boundary".to_owned(),
        ));
    }
    let scope = match (conversation_id, conversation_turn_id, task_id, attempt_id) {
        (Some(conversation_id), Some(conversation_turn_id), None, None) => {
            validate_id(&conversation_id, "Conversation ID").map_err(as_integrity_error)?;
            validate_id(&conversation_turn_id, "Conversation turn ID")
                .map_err(as_integrity_error)?;
            UserRequestScope::Conversation {
                conversation_id,
                conversation_turn_id,
            }
        }
        (None, None, Some(task_id), None) => {
            validate_id(&task_id, "Task ID").map_err(as_integrity_error)?;
            UserRequestScope::TaskPlanning { task_id }
        }
        (None, None, Some(task_id), Some(attempt_id)) => {
            validate_id(&task_id, "Task ID").map_err(as_integrity_error)?;
            validate_id(&attempt_id, "Attempt ID").map_err(as_integrity_error)?;
            UserRequestScope::AttemptExecution {
                task_id,
                attempt_id,
            }
        }
        _ => {
            return Err(StoreError::Integrity(
                "UserRequest scope is malformed".to_owned(),
            ));
        }
    };
    if !matches!(
        kind.as_str(),
        "QUESTION" | "DECISION" | "RESOURCE_SELECTION" | "EXTERNAL_AUTHORIZATION"
    ) || !matches!(
        status.as_str(),
        "PENDING" | "ANSWERED" | "DISMISSED" | "EXPIRED" | "CANCELLED"
    ) || version < 1
        || !valid_timestamp(&created_at)
        || expires_at
            .as_deref()
            .is_some_and(|value| !valid_timestamp(value))
        || resolved_at
            .as_deref()
            .is_some_and(|value| !valid_timestamp(value))
    {
        return Err(StoreError::Integrity(
            "UserRequest detail contains invalid status, version, or timestamp data".to_owned(),
        ));
    }
    validate_scope_parent(connection, workspace_id, &scope)?;
    let stored_choices: Vec<StoredUserRequestChoice> = serde_json::from_str(&choices_json)
        .map_err(|_| StoreError::Integrity("UserRequest choices are malformed".to_owned()))?;
    let choices: Vec<UserRequestChoice> = stored_choices
        .into_iter()
        .map(|choice| UserRequestChoice {
            choice_id: choice.choice_id,
            label: choice.label,
            description: choice.description,
        })
        .collect();
    if serde_json::to_vec(&choices)
        .map_err(|_| StoreError::Integrity("UserRequest choices are malformed".to_owned()))?
        .len()
        > USER_REQUEST_CHOICES_MAX_BYTES
    {
        return Err(StoreError::Integrity(
            "UserRequest choices exceed their detail boundary".to_owned(),
        ));
    }
    if choices.len() > 500
        || choices.iter().any(|choice| {
            choice.choice_id.trim().is_empty()
                || choice.choice_id.len() > 512
                || choice.choice_id.chars().any(char::is_control)
                || choice.label.trim().is_empty()
                || choice.label.len() > 16_384
                || choice
                    .description
                    .as_ref()
                    .is_some_and(|value| value.len() > 16_384)
        })
    {
        return Err(StoreError::Integrity(
            "UserRequest choices exceed their detail boundary".to_owned(),
        ));
    }
    match interaction_mode.as_str() {
        "FORM" if kind != "EXTERNAL_AUTHORIZATION" => {}
        "EXTERNAL_URL" if kind == "EXTERNAL_AUTHORIZATION" && choices.is_empty() => {}
        _ => {
            return Err(StoreError::Integrity(
                "UserRequest interaction mode is malformed".to_owned(),
            ));
        }
    }

    let detail = UserRequestDetail {
        request_id: stored_request_id,
        workspace_id: stored_workspace_id,
        scope,
        kind,
        interaction_mode,
        prompt,
        choices,
        status,
        version: u64::try_from(version)
            .map_err(|_| StoreError::Integrity("UserRequest version is invalid".to_owned()))?,
        created_at,
        expires_at,
        resolved_at,
    };
    if serde_json::to_vec(&detail)
        .map_err(|_| StoreError::Integrity("UserRequest details are malformed".to_owned()))?
        .len()
        > USER_REQUEST_DETAIL_MAX_BYTES
    {
        return Err(StoreError::Integrity(
            "UserRequest details exceed their response boundary".to_owned(),
        ));
    }
    Ok(Some(detail))
}

fn validate_id(value: &str, name: &str) -> Result<(), StoreError> {
    if value.trim().is_empty()
        || value.len() > USER_REQUEST_ID_MAX_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(StoreError::Invalid(format!("{name} is malformed")));
    }
    Ok(())
}

fn valid_timestamp(value: &str) -> bool {
    value.len() <= 64 && value.is_ascii() && OffsetDateTime::parse(value, &Rfc3339).is_ok()
}

fn validate_scope_parent(
    connection: &Connection,
    workspace_id: &str,
    scope: &UserRequestScope,
) -> Result<(), StoreError> {
    let parent_exists = match scope {
        UserRequestScope::Conversation {
            conversation_id,
            conversation_turn_id,
        } => connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM conversations c
                    JOIN conversation_turns t ON t.conversation_id = c.conversation_id
                    WHERE c.workspace_id = ?1
                      AND c.conversation_id = ?2
                      AND t.turn_id = ?3
                )",
                params![workspace_id, conversation_id, conversation_turn_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(map_database_error)?,
        UserRequestScope::TaskPlanning { task_id } => connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM tasks
                    WHERE task_id = ?1 AND workspace_id = ?2
                )",
                params![task_id, workspace_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(map_database_error)?,
        UserRequestScope::AttemptExecution {
            task_id,
            attempt_id,
        } => connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM attempts a
                    JOIN tasks t ON t.task_id = a.task_id
                    WHERE t.workspace_id = ?1
                      AND t.task_id = ?2
                      AND a.attempt_id = ?3
                )",
                params![workspace_id, task_id, attempt_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(map_database_error)?,
    };
    if !parent_exists {
        return Err(StoreError::Integrity(
            "UserRequest scope parent is missing or outside its Workspace".to_owned(),
        ));
    }
    Ok(())
}

fn as_integrity_error(error: StoreError) -> StoreError {
    match error {
        StoreError::Invalid(message) => StoreError::Integrity(message),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileBlobStore, SqliteConfig, WorkspaceBlobKey, WorkspaceBlobKeyProvider};
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use rusqlite::Connection;
    use std::{sync::Arc, time::Duration};
    use storage_core::{BlobPurpose, StoreError};
    use zeroize::Zeroizing;

    const AT: &str = "2026-10-16T10:00:00Z";

    struct TestKeys;

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([41; 32]),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob("unknown test key version".to_owned()));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    fn fixture() -> (tempfile::TempDir, SqliteWorkspaceStore, Connection) {
        let directory = tempfile::tempdir().expect("UserRequest detail fixture directory");
        let database = directory.path().join("state").join("state.sqlite3");
        let store = SqliteWorkspaceStore::open(
            &database,
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), TestKeys)),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(2),
            },
        )
        .expect("real SQLite store opens");
        for (workspace_id, event_id) in [("workspace-a", "event-a"), ("workspace-b", "event-b")] {
            WorkspaceService::new(store.clone())
                .create(CreateWorkspace {
                    workspace_id: workspace_id.to_owned(),
                    name: workspace_id.to_owned(),
                    owner_principal_id: "owner".to_owned(),
                    event: EventContext {
                        event_id: event_id.to_owned(),
                        origin_runtime_id: "runtime-detail-tests".to_owned(),
                        hlc_timestamp: AT.to_owned(),
                        correlation_id: format!("correlation-{event_id}"),
                        causation_id: None,
                        recorded_at: AT.to_owned(),
                    },
                })
                .expect("Workspace commits");
        }
        let connection = Connection::open(database).expect("open seeded SQLite database");
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .expect("disable fixture-only foreign keys");
        connection
            .execute_batch(
                "DROP TRIGGER user_request_admission_matches_scope;
                 DROP TRIGGER user_request_provenance_immutable;",
            )
            .expect(
                "remove admission and immutable-provenance guards for synthetic fixture updates",
            );
        connection
            .execute(
                "INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status,
                    lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                    created_at, updated_at, version)
                 VALUES ('task-detail', 'workspace-a', 1, 'READY', 'missing-test-binding',
                    '[]', 'NORMAL', '{}', ?1, ?1, 1)",
                [AT],
            )
            .expect("same-Workspace Task parent exists for valid detail fixtures");
        // This read-boundary fixture updates synthetic persisted prompt/choice bytes
        // after insert to test malformed and over-budget rows. The production schema
        // keeps identity/provenance immutable; only this temporary connection disables it.
        (directory, store, connection)
    }

    fn insert_request(
        connection: &Connection,
        request_id: &str,
        workspace_id: &str,
        choices_json: &str,
    ) {
        insert_request_for_task(
            connection,
            request_id,
            workspace_id,
            "task-detail",
            choices_json,
        );
    }

    fn insert_request_for_task(
        connection: &Connection,
        request_id: &str,
        workspace_id: &str,
        task_id: &str,
        choices_json: &str,
    ) {
        connection.execute(
            "INSERT INTO user_requests(request_id, workspace_id, task_id, agent_session_id, kind,
                interaction_mode, prompt, choices_json, status, created_at, expires_at, version)
             VALUES (?1, ?2, ?3, 'session-private', 'QUESTION', 'FORM',
                'Which option?', ?4, 'PENDING', ?5, NULL, 1)",
            params![request_id, workspace_id, task_id, choices_json, AT],
        ).expect("test UserRequest is inserted");
    }

    fn source_choices_at_byte_size(target_bytes: usize) -> String {
        let mut choices = (0..32)
            .map(|index| {
                serde_json::json!({
                    "choice_id": format!("choice-{index}"),
                    "label": "x".repeat(15_000),
                    "description": null,
                    "value": {"private": "provider-side choice data"},
                })
            })
            .collect::<Vec<_>>();
        let current_bytes = serde_json::to_vec(&choices)
            .expect("source choices serialize")
            .len();
        let mut remaining = target_bytes
            .checked_sub(current_bytes)
            .expect("baseline fits choice budget");
        for choice in &mut choices {
            let label = choice["label"]
                .as_str()
                .expect("fixture label is text")
                .to_owned();
            let addition = remaining.min(16_384 - label.len());
            choice["label"] = format!("{label}{}", "x".repeat(addition)).into();
            remaining -= addition;
        }
        assert_eq!(
            remaining, 0,
            "choice labels can reach the requested byte boundary"
        );
        let encoded = serde_json::to_string(&choices).expect("source choices encode");
        assert_eq!(
            encoded.len(),
            target_bytes,
            "fixture matches its UTF-8 byte target"
        );
        encoded
    }

    #[test]
    fn lookup_returns_exact_workspace_scoped_safe_detail_and_hides_foreign_rows() {
        let (_directory, store, connection) = fixture();
        insert_request(
            &connection,
            "request-a",
            "workspace-a",
            r#"[{"choice_id":"choice-a","label":"First","value":"first"}]"#,
        );
        insert_request(&connection, "request-b", "workspace-b", "[]");
        let detail = store
            .get_user_request_detail("workspace-a", "request-a")
            .expect("detail read succeeds")
            .expect("selected Workspace request exists");
        assert_eq!(detail.request_id, "request-a");
        assert_eq!(detail.workspace_id, "workspace-a");
        assert_eq!(
            detail.scope,
            UserRequestScope::TaskPlanning {
                task_id: "task-detail".to_owned()
            }
        );
        assert_eq!(detail.choices[0].label, "First");
        assert_eq!(detail.status, "PENDING");
        assert_eq!(detail.version, 1);
        let wire = serde_json::to_value(&detail).expect("detail serializes");
        assert!(wire.get("provider_url").is_none());
        assert!(wire.get("provider_input_binding").is_none());
        assert!(wire.get("agent_session_id").is_none());
        assert!(wire["choices"][0].get("value").is_none());
        assert_eq!(
            store
                .get_user_request_detail("workspace-a", "request-b")
                .expect("foreign lookup is safe"),
            None
        );
        assert_eq!(
            store
                .get_user_request_detail("workspace-a", "missing")
                .expect("absent lookup is safe"),
            None
        );
    }

    #[test]
    fn detail_read_observes_the_current_terminal_version_and_rejects_malformed_choices() {
        let (_directory, store, connection) = fixture();
        insert_request(&connection, "request-current", "workspace-a", "[]");
        connection.execute(
            "UPDATE user_requests SET status = 'DISMISSED', resolved_at = ?1, version = 2 WHERE request_id = 'request-current'",
            [AT],
        ).expect("request can resolve before detail read");
        let current = store
            .get_user_request_detail("workspace-a", "request-current")
            .expect("detail read succeeds")
            .expect("request exists");
        assert_eq!(current.status, "DISMISSED");
        assert_eq!(current.version, 2);

        insert_request(
            &connection,
            "request-bad-choices",
            "workspace-a",
            "not-json",
        );
        assert!(matches!(
            store.get_user_request_detail("workspace-a", "request-bad-choices"),
            Err(StoreError::Integrity(_)),
        ));
    }

    #[test]
    fn detail_rejects_missing_or_foreign_workspace_scope_parents() {
        let (_directory, store, connection) = fixture();
        connection
            .execute(
                "INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status,
                    lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                    created_at, updated_at, version)
                 VALUES ('task-foreign', 'workspace-b', 1, 'READY', 'missing-test-binding',
                    '[]', 'NORMAL', '{}', ?1, ?1, 1)",
                [AT],
            )
            .expect("foreign Workspace Task fixture inserts");
        insert_request_for_task(
            &connection,
            "request-foreign-task",
            "workspace-a",
            "task-foreign",
            "[]",
        );
        insert_request_for_task(
            &connection,
            "request-missing-task",
            "workspace-a",
            "task-missing",
            "[]",
        );

        for request_id in ["request-foreign-task", "request-missing-task"] {
            assert!(
                matches!(
                    store.get_user_request_detail("workspace-a", request_id),
                    Err(StoreError::Integrity(_)),
                ),
                "{request_id} must fail closed when its Task is outside the selected Workspace"
            );
        }
    }

    #[test]
    fn detail_scope_parents_require_workspace_owned_conversation_turns_and_attempts() {
        let (_directory, _store, connection) = fixture();
        for (conversation_id, workspace_id) in [
            ("conversation-detail", "workspace-a"),
            ("conversation-foreign", "workspace-b"),
        ] {
            connection
                .execute(
                    "INSERT INTO conversations(conversation_id, workspace_id, created_at, version)
                     VALUES (?1, ?2, ?3, 1)",
                    params![conversation_id, workspace_id, AT],
                )
                .expect("conversation fixture inserts");
        }
        for (turn_id, conversation_id) in [
            ("turn-detail", "conversation-detail"),
            ("turn-foreign", "conversation-foreign"),
        ] {
            connection
                .execute(
                    "INSERT INTO conversation_turns(turn_id, conversation_id, user_message_id,
                        status, retry_ordinal, created_at, version)
                     VALUES (?1, ?2, ?3, 'OPEN', 0, ?4, 1)",
                    params![turn_id, conversation_id, format!("message-{turn_id}"), AT],
                )
                .expect("conversation turn fixture inserts");
        }
        connection
            .execute(
                "INSERT INTO tasks(task_id, workspace_id, current_spec_revision, status,
                    lead_agent_binding_id, blocking_conditions_json, priority, created_by_json,
                    created_at, updated_at, version)
                 VALUES ('task-foreign-attempt', 'workspace-b', 1, 'READY',
                    'missing-test-binding', '[]', 'NORMAL', '{}', ?1, ?1, 1)",
                [AT],
            )
            .expect("foreign Workspace Task fixture inserts");
        connection
            .execute_batch("DROP TRIGGER attempt_environment_owner_guard;")
            .expect("allow intentionally corrupt Attempt rows for read-boundary test");
        for (attempt_id, task_id) in [
            ("attempt-detail", "task-detail"),
            ("attempt-foreign", "task-foreign-attempt"),
        ] {
            connection
                .execute(
                    "INSERT INTO attempts(attempt_id, task_id, step_id, agent_binding_id,
                        runtime_id, runtime_incarnation_id, environment_id, failover_class,
                        status, created_at, version)
                     VALUES (?1, ?2, 'step-detail', 'binding-detail', 'runtime-detail',
                        'incarnation-detail', 'environment-detail', 'SAFE_PORTABLE',
                        'CREATED', ?3, 1)",
                    params![attempt_id, task_id, AT],
                )
                .expect("attempt fixture inserts");
        }

        assert!(
            validate_scope_parent(
                &connection,
                "workspace-a",
                &UserRequestScope::Conversation {
                    conversation_id: "conversation-detail".to_owned(),
                    conversation_turn_id: "turn-detail".to_owned(),
                },
            )
            .is_ok()
        );
        for (conversation_id, conversation_turn_id) in [
            ("conversation-foreign", "turn-foreign"),
            ("conversation-detail", "missing-turn"),
            ("conversation-foreign", "turn-detail"),
        ] {
            assert!(matches!(
                validate_scope_parent(
                    &connection,
                    "workspace-a",
                    &UserRequestScope::Conversation {
                        conversation_id: conversation_id.to_owned(),
                        conversation_turn_id: conversation_turn_id.to_owned(),
                    },
                ),
                Err(StoreError::Integrity(_)),
            ));
        }

        assert!(
            validate_scope_parent(
                &connection,
                "workspace-a",
                &UserRequestScope::AttemptExecution {
                    task_id: "task-detail".to_owned(),
                    attempt_id: "attempt-detail".to_owned(),
                },
            )
            .is_ok()
        );
        for (task_id, attempt_id) in [
            ("task-foreign-attempt", "attempt-foreign"),
            ("task-detail", "missing-attempt"),
            ("task-detail", "attempt-foreign"),
        ] {
            assert!(matches!(
                validate_scope_parent(
                    &connection,
                    "workspace-a",
                    &UserRequestScope::AttemptExecution {
                        task_id: task_id.to_owned(),
                        attempt_id: attempt_id.to_owned(),
                    },
                ),
                Err(StoreError::Integrity(_)),
            ));
        }
    }

    #[test]
    fn detail_rejects_malformed_created_expiry_and_resolution_timestamps() {
        let (_directory, store, connection) = fixture();
        insert_request(&connection, "request-bad-created-at", "workspace-a", "[]");
        connection
            .execute(
                "UPDATE user_requests SET created_at = 'yesterday' WHERE request_id = 'request-bad-created-at'",
                [],
            )
            .expect("malformed created_at fixture persists");

        insert_request(&connection, "request-bad-expiry", "workspace-a", "[]");
        connection
            .execute(
                "UPDATE user_requests SET expires_at = 'yesterday' WHERE request_id = 'request-bad-expiry'",
                [],
            )
            .expect("malformed expires_at fixture persists");

        insert_request(&connection, "request-bad-resolved-at", "workspace-a", "[]");
        connection
            .execute(
                "UPDATE user_requests SET status = 'DISMISSED', resolved_at = 'yesterday', version = 2 WHERE request_id = 'request-bad-resolved-at'",
                [],
            )
            .expect("malformed resolved_at fixture persists");

        for request_id in [
            "request-bad-created-at",
            "request-bad-expiry",
            "request-bad-resolved-at",
        ] {
            assert!(
                matches!(
                    store.get_user_request_detail("workspace-a", request_id),
                    Err(StoreError::Integrity(_)),
                ),
                "{request_id} must be rejected as corrupt persisted detail"
            );
        }
    }

    #[test]
    fn detail_enforces_byte_budgets_and_accepts_the_full_needs_you_id_bound() {
        let (_directory, store, connection) = fixture();
        insert_request(&connection, "request-budget", "workspace-a", "[]");
        let at_prompt_limit = "p".repeat(USER_REQUEST_PROMPT_MAX_BYTES);
        connection
            .execute(
                "UPDATE user_requests SET prompt = ?1 WHERE request_id = 'request-budget'",
                [&at_prompt_limit],
            )
            .expect("boundary prompt persists");
        assert_eq!(
            store
                .get_user_request_detail("workspace-a", "request-budget")
                .expect("prompt at byte limit is readable")
                .expect("request exists")
                .prompt
                .len(),
            USER_REQUEST_PROMPT_MAX_BYTES,
        );
        let over_prompt_limit = format!("{at_prompt_limit}p");
        connection
            .execute(
                "UPDATE user_requests SET prompt = ?1 WHERE request_id = 'request-budget'",
                [&over_prompt_limit],
            )
            .expect("over-limit prompt persists for integrity test");
        assert!(matches!(
            store.get_user_request_detail("workspace-a", "request-budget"),
            Err(StoreError::Integrity(_)),
        ));

        assert_eq!(
            store
                .get_user_request_detail("workspace-a", &"x".repeat(512))
                .expect("512-byte request IDs are accepted"),
            None,
        );
        assert!(matches!(
            store.get_user_request_detail("workspace-a", &"x".repeat(513)),
            Err(StoreError::Invalid(_)),
        ));
    }

    #[test]
    fn detail_rejects_normalized_choices_and_total_response_over_budget() {
        let (_directory, store, connection) = fixture();
        insert_request(&connection, "request-choices-budget", "workspace-a", "[]");
        let exact_choices_json = source_choices_at_byte_size(USER_REQUEST_CHOICES_MAX_BYTES);
        assert_eq!(exact_choices_json.len(), USER_REQUEST_CHOICES_MAX_BYTES);
        connection.execute(
            "UPDATE user_requests SET choices_json = ?1 WHERE request_id = 'request-choices-budget'",
            [&exact_choices_json],
        ).expect("boundary choice fixture persists");
        let exact_detail = store
            .get_user_request_detail("workspace-a", "request-choices-budget")
            .expect("source choices at the byte limit are readable")
            .expect("request exists");
        assert_eq!(exact_detail.choices.len(), 32);
        assert!(
            serde_json::to_value(&exact_detail).expect("detail serializes")["choices"][0]
                .get("value")
                .is_none()
        );

        let over_choices_json = source_choices_at_byte_size(USER_REQUEST_CHOICES_MAX_BYTES + 1);
        assert_eq!(over_choices_json.len(), USER_REQUEST_CHOICES_MAX_BYTES + 1);
        connection.execute(
            "UPDATE user_requests SET choices_json = ?1 WHERE request_id = 'request-choices-budget'",
            [&over_choices_json],
        ).expect("over-limit choice fixture persists");
        assert!(matches!(
            store.get_user_request_detail("workspace-a", "request-choices-budget"),
            Err(StoreError::Integrity(_)),
        ));

        insert_request(&connection, "request-total-budget", "workspace-a", "[]");
        let escaped_prompt = "\0".repeat(160_000);
        connection
            .execute(
                "UPDATE user_requests SET prompt = ?1 WHERE request_id = 'request-total-budget'",
                [&escaped_prompt],
            )
            .expect("total-size fixture persists");
        assert!(matches!(
            store.get_user_request_detail("workspace-a", "request-total-budget"),
            Err(StoreError::Integrity(_)),
        ));
    }
}
