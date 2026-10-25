//! Durable Conversation catalog reads and owner-created Conversation aggregates.
use super::*;

type ConversationTurnStorageRow = (
    String,
    String,
    String,
    Option<String>,
    String,
    i64,
    String,
    Option<String>,
    i64,
    String,
);
use domain_conversation::{ConversationTurn, TurnCommand};
use storage_core::conversation::{validate_conversation_record, *};


#[derive(Clone)]
pub struct SqliteConversationStore {
    store: SqliteWorkspaceStore,
}

impl SqliteConversationStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store }
    }

    fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let (send, recv) = mpsc::channel();
        self.store.execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let _ = send.send(f(connection));
                }),
            },
            recv,
        )
    }
}

impl ConversationStore for SqliteConversationStore {
    fn create_conversation(
        &self,
        commit: CreateConversationCommit,
    ) -> Result<CommittedConversation, StoreError> {
        validate_conversation_record(&commit.conversation)?;
        if commit.request.principal_id.trim().is_empty()
            || !super::valid_shared_idempotency_key(&commit.request.request_id)
            || commit.event.workspace_id != commit.conversation.workspace_id
            || commit.event.entity_type != "Conversation"
            || commit.event.entity_id != commit.conversation.conversation_id
            || commit.event.entity_revision != 1
            || commit.event.event_type != "conversation.created.v1"
            || commit
                .event
                .payload
                .get("conversation_id")
                .and_then(Value::as_str)
                != Some(commit.conversation.conversation_id.as_str())
        {
            return Err(StoreError::Invalid(
                "Conversation creation commit is inconsistent".to_owned(),
            ));
        }

        let state_bytes = canonical_json(&commit.conversation)?;
        let request_digest = digest(&canonical_json(&commit.request.request_payload)?);
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| {
            let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_database_error)?;
            if let Some(value) = verify_request_receipt::<Value>(&transaction, &commit.request.principal_id, &commit.request.request_id, &request_digest)? {
                let replay: CommittedConversation = serde_json::from_value(value).map_err(|_| StoreError::Integrity("Conversation idempotency receipt is invalid".to_owned()))?;
                transaction.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let workspace_status: Option<String> = transaction
                .query_row(
                    "SELECT status FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2",
                    params![commit.conversation.workspace_id, commit.request.principal_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            let Some(workspace_status) = workspace_status else {
                return Err(StoreError::NotFound);
            };
            if workspace_status != "ACTIVE" {
                return Err(StoreError::Invalid("WORKSPACE_ARCHIVED".to_owned()));
            }
            let blob = blobs.put(
                &commit.conversation.workspace_id,
                BlobPurpose::AggregateState,
                &state_bytes,
                super::STATE_MEDIA_TYPE,
            )?;
            if blob.size_bytes != state_bytes.len() as u64
                || blobs.get(
                    &commit.conversation.workspace_id,
                    BlobPurpose::AggregateState,
                    &blob,
                )? != state_bytes
            {
                return Err(StoreError::Integrity(
                    "Conversation aggregate state failed BlobStore verification".to_owned(),
                ));
            }
            let state_ref = AggregateStateRef {
                blob,
                entity_revision: 1,
                record_schema_version: 1,
            };
            transaction.execute(
                "INSERT INTO conversations(conversation_id,workspace_id,title,active_agent_binding_id,created_at,version) VALUES(?1,?2,?3,NULL,?4,1)",
                params![commit.conversation.conversation_id, commit.conversation.workspace_id, commit.conversation.title, commit.conversation.created_at],
            ).map_err(map_database_error)?;
            let event = insert_domain_event(&transaction, &commit.event, &state_ref)?;
            let accepted = CommittedConversation { conversation: commit.conversation, event };
            let response_json = String::from_utf8(canonical_json(&accepted)?).map_err(|error| StoreError::Invalid(error.to_string()))?;
            transaction.execute(
                "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,NULL)",
                params![commit.request.principal_id, commit.request.request_id, request_digest, response_json, digest(response_json.as_bytes()), accepted.event.recorded_at],
            ).map_err(map_database_error)?;
            transaction.commit().map_err(map_database_error)?;
            Ok(accepted)
        })
    }

    fn set_conversation_agent_binding_idempotent(
        &self,
        commit: SetConversationAgentBinding,
    ) -> Result<CommittedConversation, StoreError> {
        let next_version = commit
            .expected_version
            .checked_add(1)
            .filter(|version| *version <= i64::MAX as u64)
            .ok_or_else(|| StoreError::Invalid("Conversation version exhausted".to_owned()))?;
        let request_payload = &commit.request.request_payload;
        let binding_matches = match commit.agent_binding_id.as_deref() {
            Some(binding_id) => request_payload
                .get("agent_binding_id")
                .and_then(Value::as_str)
                == Some(binding_id),
            None => request_payload
                .get("agent_binding_id")
                .is_some_and(Value::is_null),
        };
        if commit.principal_id.trim().is_empty()
            || commit.request.principal_id != commit.principal_id
            || !super::valid_shared_idempotency_key(&commit.request.request_id)
            || commit.workspace_id.trim().is_empty()
            || commit.conversation_id.trim().is_empty()
            || commit.expected_version == 0
            || commit
                .agent_binding_id
                .as_ref()
                .is_some_and(|id| id.trim().is_empty() || id.len() > 256 || id.chars().any(char::is_control))
            || request_payload.get("operation").and_then(Value::as_str)
                != Some("conversation.agent_binding.set.v1")
            || request_payload.get("workspace_id").and_then(Value::as_str)
                != Some(commit.workspace_id.as_str())
            || request_payload.get("conversation_id").and_then(Value::as_str)
                != Some(commit.conversation_id.as_str())
            || request_payload.get("expected_version").and_then(Value::as_u64)
                != Some(commit.expected_version)
            || !binding_matches
            || commit.event.workspace_id != commit.workspace_id
            || commit.event.entity_type != "Conversation"
            || commit.event.entity_id != commit.conversation_id
            || commit.event.entity_revision != next_version
            || commit.event.schema_version != 1
            || commit.event.event_type != "conversation.agent_binding.changed.v1"
        {
            return Err(StoreError::Invalid(
                "Conversation AgentBinding mutation is inconsistent".to_owned(),
            ));
        }
        let request_digest = digest(&canonical_json(request_payload)?);
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            if let Some(replay) = verify_request_receipt::<CommittedConversation>(
                &tx,
                &commit.principal_id,
                &commit.request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let workspace_status: Option<String> = tx
                .query_row(
                    "SELECT status FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2",
                    params![commit.workspace_id, commit.principal_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            let Some(workspace_status) = workspace_status else {
                return Err(StoreError::NotFound);
            };
            if workspace_status != "ACTIVE" {
                return Err(StoreError::Invalid("WORKSPACE_ARCHIVED".to_owned()));
            }
            let current: Option<(Option<String>, i64, String, Option<String>)> = tx
                .query_row(
                    "SELECT title,version,created_at,active_agent_binding_id FROM conversations WHERE workspace_id=?1 AND conversation_id=?2",
                    params![commit.workspace_id, commit.conversation_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(map_database_error)?;
            let Some((title, actual_version, created_at, previous_binding_id)) = current else {
                return Err(StoreError::NotFound);
            };
            let actual_version = u64::try_from(actual_version)
                .map_err(|_| StoreError::Integrity("Conversation version is invalid".to_owned()))?;
            if actual_version != commit.expected_version {
                return Err(StoreError::Conflict {
                    expected: Some(commit.expected_version),
                    actual: Some(actual_version),
                });
            }
            if let Some(binding_id) = commit.agent_binding_id.as_deref() {
                let eligible: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id=?1 AND agent_binding_id=?2 AND enabled=1 AND lead_eligible=1)",
                        params![commit.workspace_id, binding_id],
                        |row| row.get(0),
                    )
                    .map_err(map_database_error)?;
                if !eligible {
                    return Err(StoreError::Invalid("AGENT_NOT_LEAD_ELIGIBLE".to_owned()));
                }
            }
            let conversation = ConversationRecord {
                conversation_id: commit.conversation_id.clone(),
                workspace_id: commit.workspace_id.clone(),
                title,
                active_agent_binding_id: commit.agent_binding_id.clone(),
                version: next_version,
                created_at,
            };
            validate_conversation_record(&conversation)?;
            let changed = tx
                .execute(
                    "UPDATE conversations SET active_agent_binding_id=?1,version=?2 WHERE workspace_id=?3 AND conversation_id=?4 AND version=?5",
                    params![conversation.active_agent_binding_id, next_version as i64, commit.workspace_id, commit.conversation_id, commit.expected_version as i64],
                )
                .map_err(map_database_error)?;
            if changed != 1 {
                return Err(StoreError::Conflict {
                    expected: Some(commit.expected_version),
                    actual: None,
                });
            }
            let state_bytes = canonical_json(&conversation)?;
            let state_blob = blobs.put(
                &commit.workspace_id,
                BlobPurpose::AggregateState,
                &state_bytes,
                super::STATE_MEDIA_TYPE,
            )?;
            if state_blob.size_bytes != state_bytes.len() as u64
                || blobs.get(
                    &commit.workspace_id,
                    BlobPurpose::AggregateState,
                    &state_blob,
                )? != state_bytes
            {
                return Err(StoreError::Integrity(
                    "Conversation aggregate state failed BlobStore verification".to_owned(),
                ));
            }
            let state_ref = AggregateStateRef {
                blob: state_blob,
                entity_revision: next_version,
                record_schema_version: 1,
            };
            let mut event_draft = commit.event;
            event_draft.payload = serde_json::json!({
                "workspace_id": commit.workspace_id,
                "conversation_id": commit.conversation_id,
                "from_agent_binding_id": previous_binding_id,
                "to_agent_binding_id": commit.agent_binding_id,
                "changed_by": {"kind":"USER","principal_id":commit.principal_id},
                "aggregate_version": next_version,
            });
            let event = insert_domain_event(&tx, &event_draft, &state_ref)?;
            let accepted = CommittedConversation { conversation, event };
            let response_json = String::from_utf8(canonical_json(&accepted)?)
                .map_err(|error| StoreError::Invalid(error.to_string()))?;
            tx.execute(
                "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,NULL)",
                params![
                    commit.principal_id,
                    commit.request.request_id,
                    request_digest,
                    response_json,
                    digest(response_json.as_bytes()),
                    accepted.event.recorded_at,
                ],
            )
            .map_err(map_database_error)?;
            tx.commit().map_err(map_database_error)?;
            Ok(accepted)
        })
    }

    fn get_conversation(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationRecord>, StoreError> {
        if principal_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || conversation_id.trim().is_empty()
        {
            return Err(StoreError::Invalid(
                "Conversation lookup is invalid".to_owned(),
            ));
        }
        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let id = conversation_id.to_owned();
        self.run(move |connection| {
            let allowed: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2)", params![workspace, principal], |row| row.get(0)).map_err(map_database_error)?;
            if !allowed { return Err(StoreError::NotFound); }
            connection.query_row("SELECT conversation_id,workspace_id,title,active_agent_binding_id,version,created_at FROM conversations WHERE workspace_id=?1 AND conversation_id=?2", params![workspace,id], |row| Ok(ConversationRecord { conversation_id: row.get(0)?,workspace_id:row.get(1)?,title:row.get(2)?,active_agent_binding_id:row.get(3)?,version:row.get::<_,i64>(4)? as u64,created_at:row.get(5)? })).optional().map_err(map_database_error)
        })
    }

    fn list_conversations(
        &self,
        principal_id: &str,
        workspace_id: &str,
        after_created_at: Option<&str>,
        after_conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ConversationRecord>, StoreError> {
        if principal_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || !(1..=101).contains(&limit)
            || after_created_at.is_some() != after_conversation_id.is_some()
        {
            return Err(StoreError::Invalid(
                "Conversation page query is invalid".to_owned(),
            ));
        }
        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let after_at = after_created_at.map(str::to_owned);
        let after_id = after_conversation_id.map(str::to_owned);
        self.run(move |connection| {
            let allowed: bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2)",params![workspace,principal],|row|row.get(0)).map_err(map_database_error)?;
            if !allowed { return Err(StoreError::NotFound); }
            let mut statement=connection.prepare("SELECT conversation_id,workspace_id,title,active_agent_binding_id,version,created_at FROM conversations WHERE workspace_id=?1 AND (?2 IS NULL OR (created_at,conversation_id)<(?2,?3)) ORDER BY created_at DESC,conversation_id DESC LIMIT ?4").map_err(map_database_error)?;
            statement.query_map(params![workspace,after_at,after_id,limit as i64],|row|Ok(ConversationRecord{conversation_id:row.get(0)?,workspace_id:row.get(1)?,title:row.get(2)?,active_agent_binding_id:row.get(3)?,version:row.get::<_,i64>(4)? as u64,created_at:row.get(5)?})).map_err(map_database_error)?.collect::<Result<Vec<_>,_>>().map_err(map_database_error)
        })
    }
}

impl ConversationTurnStore for SqliteConversationStore {
    fn replay_conversation_turn(
        &self,
        principal_id: &str,
        request_id: &str,
        request_payload: &Value,
    ) -> Result<Option<CommittedConversationTurn>, StoreError> {
        if principal_id.trim().is_empty() || !super::valid_shared_idempotency_key(request_id) {
            return Err(StoreError::Invalid(
                "Conversation turn receipt lookup identity is invalid".to_owned(),
            ));
        }
        let principal = principal_id.to_owned();
        let request_id = request_id.to_owned();
        let request_digest = digest(&canonical_json(request_payload)?);
        self.run(move |connection| {
            verify_request_receipt::<CommittedConversationTurn>(
                connection,
                &principal,
                &request_id,
                &request_digest,
            )
        })
    }

    fn create_conversation_turn(
        &self,
        commit: CreateConversationTurnCommit,
    ) -> Result<CommittedConversationTurn, StoreError> {
        validate_turn_create(&commit)?;
        let request_digest = digest(&canonical_json(&commit.request.request_payload)?);
        let message = commit.message.clone();
        let turn = commit.turn.clone();
        let mut message_event = commit.message_event.clone();
        let turn_state = canonical_json(&turn)?;
        let blobs = Arc::clone(&self.store.inner.blobs);
        let principal = commit.request.principal_id.clone();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            if let Some(replay) = verify_request_receipt::<CommittedConversationTurn>(
                &tx,
                &principal,
                &commit.request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let allowed: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
                    params![commit.workspace_id, principal],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !allowed {
                return Err(StoreError::NotFound);
            }
            let conversation: Option<(Option<String>, i64, String, Option<String>)> = tx
                .query_row(
                    "SELECT title,version,created_at,active_agent_binding_id FROM conversations WHERE workspace_id=?1 AND conversation_id=?2",
                    params![commit.workspace_id, commit.turn.conversation_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(map_database_error)?;
            let Some((title, version, created_at, conversation_binding)) = conversation else {
                return Err(StoreError::NotFound);
            };
            // The Conversation's admitted AgentBinding selection is resolved inside
            // this authoritative transaction: the explicit Conversation override
            // first, then the Workspace default. The first configured selection is
            // authoritative. A missing, disabled, or non-lead-eligible selection is
            // never skipped in favour of a lower-precedence binding, and it never
            // persists a partial message, turn, event, or receipt.
            let workspace_default: Option<String> = tx
                .query_row(
                    "SELECT default_agent_binding_id FROM workspaces WHERE workspace_id=?1",
                    params![commit.workspace_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            let selected_binding = conversation_binding.clone().or(workspace_default);
            let Some(selected_binding) = selected_binding else {
                return Err(StoreError::Invalid("AGENT_UNAVAILABLE".to_owned()));
            };
            let binding_eligible: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id=?1 AND agent_binding_id=?2 AND enabled=1 AND lead_eligible=1)",
                params![commit.workspace_id, selected_binding],
                |row| row.get(0),
            ).map_err(map_database_error)?;
            if !binding_eligible
                || commit.turn_event.payload.get("agent_binding_id").and_then(Value::as_str) != Some(selected_binding.as_str())
            {
                return Err(StoreError::Invalid("AGENT_UNAVAILABLE".to_owned()));
            }
            let next_conversation_version = version
                .checked_add(1)
                .ok_or_else(|| StoreError::Invalid("Conversation version exhausted".to_owned()))?;
            if commit.turn_event.payload.get("aggregate_version").and_then(Value::as_u64)
                != Some(commit.turn.version)
            {
                return Err(StoreError::Invalid(CONVERSATION_TURN_EVENT_REVISION_MISMATCH.to_owned()));
            }
            // Turn submission has no client If-Match precondition. The immediate
            // transaction serializes valid appends and assigns the event the actual next
            // Conversation revision, even if its request was prepared from an older read.
            message_event.entity_revision = u64::try_from(next_conversation_version)
                .map_err(|_| StoreError::Invalid("Conversation version is invalid".to_owned()))?;
            tx.execute(
                "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,agent_session_id,agent_binding_id,turn_id,content_json,resource_refs_json,source_channel_ref_json,created_at) VALUES(?1,?2,?3,'USER',NULL,NULL,?4,?5,?6,NULL,?7)",
                params![
                    commit.message.message_id,
                    commit.message.conversation_id,
                    canonical_json_string(&commit.message.author)?,
                    commit.turn.turn_id,
                    canonical_json_string(&commit.message.content)?,
                    canonical_json_string(&commit.message.resource_refs)?,
                    commit.message.created_at
                ],
            )
            .map_err(map_database_error)?;
            tx.execute(
                "INSERT INTO conversation_turns(turn_id,conversation_id,user_message_id,agent_session_id,status,retry_ordinal,created_at,settled_at,version,presentation_preference) VALUES(?1,?2,?3,NULL,'OPEN',0,?4,NULL,1,?5)",
                params![
                    commit.turn.turn_id,
                    commit.turn.conversation_id,
                    commit.turn.user_message_id,
                    commit.turn.created_at,
                    serde_json::to_value(commit.turn.presentation_preference)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .ok_or_else(|| StoreError::Invalid("invalid presentation preference".to_owned()))?
                ],
            )
            .map_err(map_database_error)?;
            tx.execute(
                "UPDATE conversations SET version=?1 WHERE workspace_id=?2 AND conversation_id=?3 AND version=?4",
                params![next_conversation_version, commit.workspace_id, commit.turn.conversation_id, version],
            )
            .map_err(map_database_error)?;
            let conversation_state = ConversationRecord {
                conversation_id: commit.turn.conversation_id.clone(),
                workspace_id: commit.workspace_id.clone(),
                title,
                // The Conversation aggregate keeps the Conversation's own override.
                // The binding this turn actually used is recorded on the turn
                // event, so a later Workspace default change still reaches newly
                // admitted turns instead of being frozen by this snapshot.
                active_agent_binding_id: conversation_binding,
                version: next_conversation_version as u64,
                created_at,
            };
            let conversation_bytes = canonical_json(&conversation_state)?;
            let conversation_blob = blobs.put(
                &commit.workspace_id,
                BlobPurpose::AggregateState,
                &conversation_bytes,
                super::STATE_MEDIA_TYPE,
            )?;
            if conversation_blob.size_bytes != conversation_bytes.len() as u64
                || blobs.get(&commit.workspace_id, BlobPurpose::AggregateState, &conversation_blob)? != conversation_bytes
            {
                return Err(StoreError::Integrity("Conversation aggregate state failed BlobStore verification".to_owned()));
            }
            let conversation_state_ref = AggregateStateRef {
                blob: conversation_blob,
                entity_revision: next_conversation_version as u64,
                record_schema_version: 1,
            };
            let message_event = insert_domain_event(&tx, &message_event, &conversation_state_ref)?;
            // The turn aggregate blob is written only after the receipt lookup. An exact
            // replay therefore performs no BlobStore write and cannot leave an
            // unreferenced snapshot of a fresh candidate timestamp.
            let turn_blob = blobs.put(
                &commit.workspace_id,
                BlobPurpose::AggregateState,
                &turn_state,
                super::STATE_MEDIA_TYPE,
            )?;
            if turn_blob.size_bytes != turn_state.len() as u64
                || blobs.get(&commit.workspace_id, BlobPurpose::AggregateState, &turn_blob)?
                    != turn_state
            {
                return Err(StoreError::Integrity(
                    "ConversationTurn aggregate state failed BlobStore verification".to_owned(),
                ));
            }
            let turn_state_ref = AggregateStateRef {
                blob: turn_blob,
                entity_revision: 1,
                record_schema_version: 1,
            };
            let turn_event = insert_domain_event(&tx, &commit.turn_event, &turn_state_ref)?;
            let accepted = CommittedConversationTurn {
                message: message.clone(),
                turn: turn.clone(),
                events: vec![message_event, turn_event],
            };
            let response_json = String::from_utf8(canonical_json(&accepted)?)
                .map_err(|error| StoreError::Invalid(error.to_string()))?;
            tx.execute(
                "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,NULL)",
                params![principal, commit.request.request_id, request_digest, response_json, digest(response_json.as_bytes()), commit.turn.created_at],
            ).map_err(map_database_error)?;
            tx.commit().map_err(map_database_error)?;
            Ok(accepted)
        })
    }

    fn transition_conversation_turn(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        expected_version: u64,
        command: TurnCommand,
        event: EventDraft,
    ) -> Result<ConversationTurn, StoreError> {
        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let conversation = conversation_id.to_owned();
        let id = turn_id.to_owned();
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            let allowed: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
                params![workspace, principal], |row| row.get(0),
            ).map_err(map_database_error)?;
            if !allowed { return Err(StoreError::NotFound); }
            let current = load_turn(&tx, &workspace, &conversation, &id)?
                .ok_or(StoreError::NotFound)?;
            if current.version != expected_version {
                return Err(StoreError::Conflict { expected: Some(expected_version), actual: Some(current.version) });
            }
            if matches!(command, TurnCommand::Start { .. }) {
                return Err(StoreError::Invalid(CONVERSATION_SESSION_ADMISSION_UNAVAILABLE.to_owned()));
            }
            let next = domain_conversation::transition(&current, expected_version, command.clone())
                .map_err(|error| match error {
                    domain_conversation::TurnTransitionError::StaleVersion => StoreError::Conflict { expected: Some(expected_version), actual: Some(current.version) },
                    other => StoreError::Invalid(format!("Conversation turn transition rejected: {other:?}")),
                })?;
            validate_turn_transition_event(&event, &current, &next, &command)?;
            if event.workspace_id != workspace {
                return Err(StoreError::Invalid("ConversationTurn event Workspace mismatch".to_owned()));
            }
            if matches!(command, TurnCommand::Retry { .. } | TurnCommand::Resume { .. }) {
                let Some(session_id) = next.agent_session_id.as_deref() else {
                    return Err(StoreError::Invalid("Conversation session is not ready".to_owned()));
                };
                let ready: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE workspace_id=?1 AND conversation_id=?2 AND conversation_turn_id=?3 AND agent_session_id=?4 AND scope_kind='CONVERSATION' AND status='ACTIVE')",
                    params![workspace, conversation, id, session_id],
                    |row| row.get(0),
                ).map_err(map_database_error)?;
                if !ready {
                    return Err(StoreError::Invalid("CONVERSATION_SESSION_NOT_READY".to_owned()));
                }
            }
            let state_bytes = canonical_json(&next)?;
            let blob = blobs.put(&workspace, BlobPurpose::AggregateState, &state_bytes, super::STATE_MEDIA_TYPE)?;
            let state_ref = AggregateStateRef { blob, entity_revision: next.version, record_schema_version: 1 };
            let next_version = i64::try_from(next.version).map_err(|_| StoreError::Invalid("ConversationTurn version exhausted".to_owned()))?;
            let updated = tx.execute(
                "UPDATE conversation_turns SET agent_session_id=?1,status=?2,retry_ordinal=?3,settled_at=?4,version=?5 WHERE turn_id=?6 AND conversation_id=?7 AND version=?8",
                params![next.agent_session_id, status_string(next.status)?, next.retry_ordinal, next.settled_at, next_version, id, conversation, i64::try_from(expected_version).unwrap_or(-1)],
            ).map_err(map_database_error)?;
            if updated != 1 {
                return Err(StoreError::Conflict { expected: Some(expected_version), actual: None });
            }
            insert_domain_event(&tx, &event, &state_ref)?;
            tx.commit().map_err(map_database_error)?;
            Ok(next)
        })
    }

    fn resolve_conversation_session_host(
        &self,
        workspace_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ResolvedConversationHost>, StoreError> {
        let workspace = workspace_id.to_owned();
        let conversation = conversation_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            // Both selections come from nullable columns. A Conversation with no
            // override stores SQL NULL, which `.optional()` does not cover: it only
            // handles the absent-row case. The value is read as Option<String> and the
            // absent-row wrapper is flattened, so NULL is a legitimate None.
            let conversation_binding: Option<Option<String>> = tx
                .query_row(
                    "SELECT active_agent_binding_id FROM conversations WHERE workspace_id=?1 AND conversation_id=?2",
                    params![workspace, conversation],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            let conversation_binding = conversation_binding.flatten();
            let workspace_default: Option<Option<String>> = tx
                .query_row(
                    "SELECT default_agent_binding_id FROM workspaces WHERE workspace_id=?1 AND status='ACTIVE'",
                    params![workspace],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?;
            let workspace_default = workspace_default.flatten();
            let Some(binding) = conversation_binding.or(workspace_default) else {
                return Ok(None);
            };
            // Same eligibility join the admission transaction re-checks, so a host
            // reported here is one the admission will accept at this instant. The clock
            // is formatted at second precision, matching how the stored instants are
            // written, so the expiry comparison stays a correct string comparison.
            let now_format = time::format_description::parse(
                "[year]-[month]-[day]T[hour]:[minute]:[second]Z",
            )
            .map_err(|_| StoreError::Invalid("timestamp format unavailable".to_owned()))?;
            let now = time::OffsetDateTime::now_utc()
                .format(&now_format)
                .map_err(|_| StoreError::Invalid("clock unavailable".to_owned()))?;
            let resolved: Option<(String, String, String)> = tx
                .query_row(
                    "SELECT b.agent_binding_id, e.endpoint_id, eb.runtime_id
                       FROM agent_bindings b
                       JOIN agent_endpoints e ON e.agent_profile_id = b.agent_profile_id
                       JOIN agent_endpoint_bindings eb ON eb.endpoint_id = e.endpoint_id
                       JOIN runtimes r ON r.runtime_id = eb.runtime_id
                       JOIN runtime_workspace_bindings rwb
                         ON rwb.runtime_id = r.runtime_id AND rwb.workspace_id = ?1
                        AND rwb.status = 'ACTIVE' AND rwb.revoked_at IS NULL
                       JOIN runtime_incarnations ri
                         ON ri.runtime_id = r.runtime_id
                        AND ri.runtime_incarnation_id = eb.runtime_incarnation_id
                       WHERE b.workspace_id = ?1 AND b.agent_binding_id = ?2
                         AND b.enabled = 1 AND b.lead_eligible = 1
                         AND (json_extract(b.endpoint_selection_policy_json, '$.mode') <> 'PINNED_ENDPOINT'
                              OR json_extract(b.endpoint_selection_policy_json, '$.endpoint_id') = e.endpoint_id)
                         AND (eb.expires_at IS NULL OR eb.expires_at > ?3)
                         AND EXISTS(SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value = 'EXECUTOR')
                         AND r.availability = 'ONLINE'
                         AND r.current_incarnation_id = eb.runtime_incarnation_id
                         AND ri.recovery_state = 'READY'
                       ORDER BY eb.observed_at DESC
                       LIMIT 1",
                    params![workspace, binding, now],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(map_database_error)?;
            let Some((agent_binding_id, endpoint_id, runtime_id)) = resolved else {
                return Ok(None);
            };
            let runtime_incarnation_id: String = tx
                .query_row(
                    "SELECT current_incarnation_id FROM runtimes WHERE runtime_id=?1",
                    params![runtime_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            Ok(Some(ResolvedConversationHost {
                agent_binding_id,
                endpoint_id,
                runtime_id,
                runtime_incarnation_id,
            }))
        })
    }
}

fn validate_turn_create(commit: &CreateConversationTurnCommit) -> Result<(), StoreError> {
    let message = &commit.message;
    let turn = &commit.turn;
    if commit.request.principal_id != commit.principal_id
        || !super::valid_shared_idempotency_key(&commit.request.request_id)
        || commit.workspace_id.trim().is_empty()
        || message.role != "USER"
        || message.author != serde_json::json!({"kind":"USER","principal_id":commit.principal_id})
        || !message.content.is_array()
        || !message.resource_refs.is_array()
        || message.conversation_id != turn.conversation_id
        || message.message_id != turn.user_message_id
        || message.turn_id.as_deref() != Some(turn.turn_id.as_str())
        || message.agent_session_id.is_some()
        || message.agent_binding_id.is_some()
        || message.source_channel_ref.is_some()
        || message.created_at != turn.created_at
        || turn.status != domain_conversation::ConversationTurnStatus::Open
        || turn.agent_session_id.is_some()
        || turn.version != 1
        || turn.retry_ordinal != 0
        || turn.settled_at.is_some()
        || turn.conversation_id.trim().is_empty()
        || turn.turn_id.trim().is_empty()
        || turn.user_message_id.trim().is_empty()
        || turn.created_at.trim().is_empty()
        || commit.message_event.workspace_id != commit.workspace_id
        || commit.message_event.entity_type != "Conversation"
        || commit.message_event.entity_id != turn.conversation_id
        || commit.message_event.event_type != "conversation.message.added.v1"
        || !payload_has_exact_keys(
            &commit.message_event.payload,
            &[
                "message_id",
                "conversation_id",
                "author",
                "content_digest",
                "resource_refs",
            ],
            &[],
        )
        || commit
            .message_event
            .payload
            .get("message_id")
            .and_then(Value::as_str)
            != Some(&message.message_id)
        || commit
            .message_event
            .payload
            .get("conversation_id")
            .and_then(Value::as_str)
            != Some(&message.conversation_id)
        || commit.message_event.payload.get("author") != Some(&message.author)
        || commit
            .message_event
            .payload
            .get("content_digest")
            .and_then(Value::as_str)
            != Some(digest(&canonical_json(&message.content)?).as_str())
        || commit.message_event.payload.get("resource_refs") != Some(&message.resource_refs)
        || commit.turn_event.workspace_id != commit.workspace_id
        || commit.turn_event.entity_type != "ConversationTurn"
        || commit.turn_event.entity_id != turn.turn_id
        || commit.turn_event.entity_revision != 1
        || commit.turn_event.event_type != "conversation.turn.created.v2"
        || !payload_has_exact_keys(
            &commit.turn_event.payload,
            &[
                "turn_id",
                "conversation_id",
                "user_message_id",
                "agent_binding_id",
                "aggregate_version",
                "presentation_preference",
            ],
            &[],
        )
        || commit
            .turn_event
            .payload
            .get("agent_binding_id")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || commit
            .turn_event
            .payload
            .get("turn_id")
            .and_then(Value::as_str)
            != Some(&turn.turn_id)
        || commit
            .turn_event
            .payload
            .get("conversation_id")
            .and_then(Value::as_str)
            != Some(&turn.conversation_id)
        || commit
            .turn_event
            .payload
            .get("user_message_id")
            .and_then(Value::as_str)
            != Some(&turn.user_message_id)
        || commit
            .turn_event
            .payload
            .get("presentation_preference")
            .and_then(Value::as_str)
            != serde_json::to_value(turn.presentation_preference)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .as_deref()
        || commit
            .turn_event
            .payload
            .get("aggregate_version")
            .and_then(Value::as_u64)
            != Some(turn.version)
    {
        return Err(StoreError::Invalid(
            "ConversationTurn creation commit is inconsistent".to_owned(),
        ));
    }
    let expected_request = serde_json::json!({
        "operation":"conversation.turn.create.v1", "workspace_id":commit.workspace_id,
        "conversation_id":turn.conversation_id, "turn_id":turn.turn_id,
        "message_id":message.message_id, "author":message.author,
        "content":message.content, "resource_refs":message.resource_refs,
        "presentation_preference":serde_json::to_value(turn.presentation_preference).map_err(|error|StoreError::Invalid(error.to_string()))?
    });
    if commit.request.request_payload != expected_request {
        return Err(StoreError::Invalid(
            "ConversationTurn request payload does not match the committed content".to_owned(),
        ));
    }
    Ok(())
}

fn validate_turn_transition_event(
    event: &EventDraft,
    current: &ConversationTurn,
    next: &ConversationTurn,
    command: &TurnCommand,
) -> Result<(), StoreError> {
    let expected_type = match command {
        TurnCommand::Retry { .. } => "conversation.turn.retried.v1",
        TurnCommand::Resume { .. } => "conversation.turn.resumed.v1",
        TurnCommand::Complete { .. }
        | TurnCommand::Fail { .. }
        | TurnCommand::SettleCancelled { .. } => "conversation.turn.settled.v1",
        TurnCommand::Start { .. } => {
            return Err(StoreError::Invalid(
                CONVERSATION_SESSION_ADMISSION_UNAVAILABLE.to_owned(),
            ));
        }
        _ => "conversation.turn.status.changed.v1",
    };
    let allowed_payload_keys: &[&str] = match command {
        TurnCommand::Retry { .. } => &[
            "turn_id",
            "prior_agent_session_id",
            "agent_session_id",
            "retry_ordinal",
            "aggregate_version",
        ],
        TurnCommand::Resume { .. } => &[
            "turn_id",
            "user_request_id",
            "prior_agent_session_id",
            "agent_session_id",
            "aggregate_version",
        ],
        TurnCommand::Complete { .. }
        | TurnCommand::Fail { .. }
        | TurnCommand::SettleCancelled { .. } => &[
            "turn_id",
            "from",
            "to",
            "reason_code",
            "agent_session_id",
            "aggregate_version",
        ],
        TurnCommand::Start { .. } => &[],
        _ => &[
            "turn_id",
            "from",
            "to",
            "reason_code",
            "agent_session_id",
            "aggregate_version",
        ],
    };
    let required_payload_keys: &[&str] = match command {
        TurnCommand::Retry { .. } => &[
            "turn_id",
            "agent_session_id",
            "retry_ordinal",
            "aggregate_version",
        ],
        TurnCommand::Resume { .. } => &[
            "turn_id",
            "user_request_id",
            "agent_session_id",
            "aggregate_version",
        ],
        TurnCommand::Complete { .. }
        | TurnCommand::Fail { .. }
        | TurnCommand::SettleCancelled { .. } => &["turn_id", "from", "to", "aggregate_version"],
        TurnCommand::Start { .. } => &[],
        _ => &["turn_id", "from", "to", "reason_code", "aggregate_version"],
    };
    let payload_matches =
        payload_has_exact_keys(&event.payload, required_payload_keys, allowed_payload_keys)
            && match command {
                TurnCommand::Retry { .. } => {
                    event.payload.get("turn_id").and_then(Value::as_str)
                        == Some(current.turn_id.as_str())
                        && event
                            .payload
                            .get("agent_session_id")
                            .and_then(Value::as_str)
                            == next.agent_session_id.as_deref()
                        && optional_event_string_matches(
                            &event.payload,
                            "prior_agent_session_id",
                            current.agent_session_id.as_deref(),
                        )
                        && event.payload.get("retry_ordinal").and_then(Value::as_u64)
                            == Some(u64::from(next.retry_ordinal))
                        && event
                            .payload
                            .get("aggregate_version")
                            .and_then(Value::as_u64)
                            == Some(next.version)
                }
                TurnCommand::Resume { .. } => {
                    event.payload.get("turn_id").and_then(Value::as_str)
                        == Some(current.turn_id.as_str())
                        && event
                            .payload
                            .get("user_request_id")
                            .and_then(Value::as_str)
                            .is_some_and(|id| !id.trim().is_empty())
                        && event
                            .payload
                            .get("agent_session_id")
                            .and_then(Value::as_str)
                            == next.agent_session_id.as_deref()
                        && optional_event_string_matches(
                            &event.payload,
                            "prior_agent_session_id",
                            current.agent_session_id.as_deref(),
                        )
                        && event
                            .payload
                            .get("aggregate_version")
                            .and_then(Value::as_u64)
                            == Some(next.version)
                }
                _ => {
                    let status_event = event.event_type == "conversation.turn.status.changed.v1";
                    event.payload.get("turn_id").and_then(Value::as_str)
                        == Some(current.turn_id.as_str())
                        && event.payload.get("from").and_then(Value::as_str)
                            == Some(status_string(current.status)?.as_str())
                        && event.payload.get("to").and_then(Value::as_str)
                            == Some(status_string(next.status)?.as_str())
                        && optional_event_string_matches(
                            &event.payload,
                            "agent_session_id",
                            next.agent_session_id.as_deref(),
                        )
                        && event
                            .payload
                            .get("aggregate_version")
                            .and_then(Value::as_u64)
                            == Some(next.version)
                        && (!status_event
                            || event
                                .payload
                                .get("reason_code")
                                .and_then(Value::as_str)
                                .is_some_and(|reason| !reason.trim().is_empty()))
                }
            };
    if event.workspace_id.trim().is_empty()
        || event.entity_type != "ConversationTurn"
        || event.entity_id != current.turn_id
        || event.entity_revision != next.version
        || event.event_type != expected_type
        || !payload_matches
        || matches!(
            command,
            TurnCommand::Complete { .. }
                | TurnCommand::Fail { .. }
                | TurnCommand::SettleCancelled { .. }
        ) && !matches!(
            next.status,
            domain_conversation::ConversationTurnStatus::Completed
                | domain_conversation::ConversationTurnStatus::Failed
                | domain_conversation::ConversationTurnStatus::Cancelled
        )
        || event.event_type == "conversation.turn.status.changed.v1"
            && (event
                .payload
                .get("reason_code")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
                || matches!(
                    next.status,
                    domain_conversation::ConversationTurnStatus::Completed
                        | domain_conversation::ConversationTurnStatus::Failed
                        | domain_conversation::ConversationTurnStatus::Cancelled
                ))
    {
        return Err(StoreError::Invalid(
            "ConversationTurn transition event is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn payload_has_exact_keys(payload: &Value, required: &[&str], optional: &[&str]) -> bool {
    let Some(object) = payload.as_object() else {
        return false;
    };
    required.iter().all(|key| object.contains_key(*key))
        && object
            .keys()
            .all(|key| required.contains(&key.as_str()) || optional.contains(&key.as_str()))
}

fn status_string(
    status: domain_conversation::ConversationTurnStatus,
) -> Result<String, StoreError> {
    serde_json::to_value(status)
        .map_err(|error| StoreError::Invalid(error.to_string()))?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| StoreError::Invalid("invalid ConversationTurn status".to_owned()))
}

fn optional_event_string_matches(payload: &Value, key: &str, expected: Option<&str>) -> bool {
    match expected {
        Some(expected) => payload.get(key).and_then(Value::as_str) == Some(expected),
        None => payload.get(key).is_none(),
    }
}

fn canonical_json_string(value: &Value) -> Result<String, StoreError> {
    String::from_utf8(canonical_json(value)?)
        .map_err(|error| StoreError::Invalid(error.to_string()))
}

fn load_turn(
    connection: &Connection,
    workspace_id: &str,
    conversation_id: &str,
    turn_id: &str,
) -> Result<Option<domain_conversation::ConversationTurn>, StoreError> {
    let raw: Option<ConversationTurnStorageRow> = connection.query_row(
        "SELECT t.turn_id,t.conversation_id,t.user_message_id,t.agent_session_id,t.status,t.retry_ordinal,t.created_at,t.settled_at,t.version,t.presentation_preference FROM conversation_turns t JOIN conversations c ON c.conversation_id=t.conversation_id WHERE c.workspace_id=?1 AND t.conversation_id=?2 AND t.turn_id=?3",
        params![workspace_id, conversation_id, turn_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?)),
    ).optional().map_err(map_database_error)?;
    raw.map(
        |(
            turn_id,
            conversation_id,
            user_message_id,
            agent_session_id,
            status,
            retry_ordinal,
            created_at,
            settled_at,
            version,
            presentation_preference,
        )| {
            let status = serde_json::from_value(Value::String(status)).map_err(|_| {
                StoreError::Integrity("ConversationTurn status is invalid".to_owned())
            })?;
            let presentation_preference =
                serde_json::from_value(Value::String(presentation_preference)).map_err(|_| {
                    StoreError::Integrity(
                        "ConversationTurn presentation preference is invalid".to_owned(),
                    )
                })?;
            Ok(domain_conversation::ConversationTurn {
                turn_id,
                conversation_id,
                user_message_id,
                agent_session_id,
                status,
                retry_ordinal: u32::try_from(retry_ordinal).map_err(|_| {
                    StoreError::Integrity("ConversationTurn retry ordinal is invalid".to_owned())
                })?,
                presentation_preference,
                created_at,
                settled_at,
                version: u64::try_from(version).map_err(|_| {
                    StoreError::Integrity("ConversationTurn version is invalid".to_owned())
                })?,
            })
        },
    )
    .transpose()
}
