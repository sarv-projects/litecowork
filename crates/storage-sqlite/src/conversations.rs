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

    pub fn get_conversation_turn(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
    ) -> Result<Option<domain_conversation::ConversationTurn>, StoreError> {
        let workspace = workspace_id.to_owned();
        let conversation = conversation_id.to_owned();
        let turn = turn_id.to_owned();
        self.run(move |connection| load_turn(connection, &workspace, &conversation, &turn))
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
            Some(binding_id) => {
                request_payload
                    .get("agent_binding_id")
                    .and_then(Value::as_str)
                    == Some(binding_id)
            }
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
            || commit.agent_binding_id.as_ref().is_some_and(|id| {
                id.trim().is_empty() || id.len() > 256 || id.chars().any(char::is_control)
            })
            || request_payload.get("operation").and_then(Value::as_str)
                != Some("conversation.agent_binding.set.v1")
            || request_payload.get("workspace_id").and_then(Value::as_str)
                != Some(commit.workspace_id.as_str())
            || request_payload
                .get("conversation_id")
                .and_then(Value::as_str)
                != Some(commit.conversation_id.as_str())
            || request_payload
                .get("expected_version")
                .and_then(Value::as_u64)
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
            require_conversation_active(&tx, &commit.conversation_id)?;
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
            require_conversation_active(&tx, &commit.turn.conversation_id)?;
            // Strict single-flight lane (B-01): the receipt lookup above already replayed an
            // exact repeat, so any nonterminal turn here belongs to a different request.
            let turn_in_flight: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id=?1 AND status IN ('OPEN','RUNNING','WAITING_USER','WAITING_DEPENDENCY','CANCEL_REQUESTED'))",
                    params![commit.turn.conversation_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if turn_in_flight {
                return Err(StoreError::Invalid(CONVERSATION_TURN_IN_PROGRESS.to_owned()));
            }
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
            let selected_binding = commit
                .turn_event
                .payload
                .get("agent_binding_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| conversation_binding.clone().or(workspace_default));
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
            // The dispatch intent commits atomically with the turn so a crash between
            // admission and dispatch leaves a durable UNCLAIMED row for the startup sweep.
            tx.execute(
                "INSERT INTO conversation_dispatch_intents(turn_id,retry_ordinal,conversation_id,state,created_at) VALUES(?1,0,?2,'UNCLAIMED',?3)",
                params![commit.turn.turn_id, commit.turn.conversation_id, commit.turn.created_at],
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

    fn create_conversation_with_first_turn(
        &self,
        commit: CreateConversationWithFirstTurnCommit,
    ) -> Result<CommittedConversationFirstTurn, StoreError> {
        let turn_commit = CreateConversationTurnCommit {
            request: commit.request.clone(),
            principal_id: commit.principal_id.clone(),
            workspace_id: commit.workspace_id.clone(),
            message: commit.message.clone(),
            turn: commit.turn.clone(),
            message_event: commit.message_event.clone(),
            turn_event: commit.turn_event.clone(),
        };
        validate_conversation_record(&commit.conversation)?;
        validate_turn_create_records(&turn_commit)?;
        let first_binding = commit
            .conversation
            .active_agent_binding_id
            .clone()
            .ok_or_else(|| StoreError::Invalid("initial AgentBinding is required".to_owned()))?;
        let expected_first_turn_request = serde_json::json!({
            "operation": "conversation.create_first_turn.v1",
            "workspace_id": commit.workspace_id,
            "title": commit.conversation.title,
            "content": commit.message.content,
            "resource_refs": commit.message.resource_refs,
            "requested_agent_binding_id": first_binding,
            "presentation_preference": serde_json::to_value(commit.turn.presentation_preference)
                .map_err(|error| StoreError::Invalid(error.to_string()))?
        });
        if commit.request.principal_id != commit.principal_id
            || !super::valid_shared_idempotency_key(&commit.request.request_id)
            || commit.request.request_payload != expected_first_turn_request
            || commit.conversation.workspace_id != commit.workspace_id
            || commit.conversation.version != 1
            || commit.message.conversation_id != commit.conversation.conversation_id
            || commit.message_event.entity_revision != 2
            || commit.conversation_event.workspace_id != commit.workspace_id
            || commit.conversation_event.entity_type != "Conversation"
            || commit.conversation_event.entity_id != commit.conversation.conversation_id
            || commit.conversation_event.entity_revision != 1
            || commit.conversation_event.event_type != "conversation.created.v1"
            || !payload_has_exact_keys(
                &commit.conversation_event.payload,
                &["conversation_id", "created_by"],
                &[],
            )
            || commit
                .conversation_event
                .payload
                .get("conversation_id")
                .and_then(Value::as_str)
                != Some(commit.conversation.conversation_id.as_str())
            || commit
                .conversation_event
                .payload
                .get("created_by")
                .and_then(|value| value.get("principal_id"))
                .and_then(Value::as_str)
                != Some(commit.principal_id.as_str())
            || commit
                .turn_event
                .payload
                .get("agent_binding_id")
                .and_then(Value::as_str)
                != Some(first_binding.as_str())
            || commit.conversation_event.correlation_id != commit.message_event.correlation_id
            || commit.conversation_event.correlation_id != commit.turn_event.correlation_id
        {
            return Err(StoreError::Invalid(
                "Conversation first-turn commit is inconsistent".to_owned(),
            ));
        }

        let request_digest = digest(&canonical_json(&commit.request.request_payload)?);
        let principal = commit.principal_id.clone();
        let workspace = commit.workspace_id.clone();
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            if let Some(replay) = verify_request_receipt::<CommittedConversationFirstTurn>(
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
                    params![workspace, principal],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !allowed {
                return Err(StoreError::NotFound);
            }
            let binding_eligible: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id=?1 AND agent_binding_id=?2 AND enabled=1 AND lead_eligible=1)",
                    params![workspace, first_binding],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !binding_eligible {
                return Err(StoreError::Invalid("AGENT_UNAVAILABLE".to_owned()));
            }

            let initial_bytes = canonical_json(&commit.conversation)?;
            let initial_blob = blobs.put(
                &workspace,
                BlobPurpose::AggregateState,
                &initial_bytes,
                super::STATE_MEDIA_TYPE,
            )?;
            if initial_blob.size_bytes != initial_bytes.len() as u64
                || blobs.get(&workspace, BlobPurpose::AggregateState, &initial_blob)?
                    != initial_bytes
            {
                return Err(StoreError::Integrity(
                    "Conversation aggregate state failed BlobStore verification".to_owned(),
                ));
            }
            let initial_state = AggregateStateRef {
                blob: initial_blob,
                entity_revision: 1,
                record_schema_version: 1,
            };
            tx.execute(
                "INSERT INTO conversations(conversation_id,workspace_id,title,active_agent_binding_id,created_at,version) VALUES(?1,?2,?3,?4,?5,1)",
                params![
                    commit.conversation.conversation_id,
                    workspace,
                    commit.conversation.title,
                    first_binding,
                    commit.conversation.created_at
                ],
            )
            .map_err(map_database_error)?;
            let created_event = insert_domain_event(&tx, &commit.conversation_event, &initial_state)?;

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
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .ok_or_else(|| StoreError::Invalid("invalid presentation preference".to_owned()))?
                ],
            )
            .map_err(map_database_error)?;
            tx.execute(
                "INSERT INTO conversation_dispatch_intents(turn_id,retry_ordinal,conversation_id,state,created_at) VALUES(?1,0,?2,'UNCLAIMED',?3)",
                params![commit.turn.turn_id, commit.turn.conversation_id, commit.turn.created_at],
            )
            .map_err(map_database_error)?;

            let mut admitted_conversation = commit.conversation.clone();
            admitted_conversation.version = 2;
            let admitted_bytes = canonical_json(&admitted_conversation)?;
            let admitted_blob = blobs.put(
                &workspace,
                BlobPurpose::AggregateState,
                &admitted_bytes,
                super::STATE_MEDIA_TYPE,
            )?;
            if admitted_blob.size_bytes != admitted_bytes.len() as u64
                || blobs.get(&workspace, BlobPurpose::AggregateState, &admitted_blob)?
                    != admitted_bytes
            {
                return Err(StoreError::Integrity(
                    "Conversation aggregate state failed BlobStore verification".to_owned(),
                ));
            }
            tx.execute(
                "UPDATE conversations SET version=2 WHERE workspace_id=?1 AND conversation_id=?2 AND version=1",
                params![workspace, admitted_conversation.conversation_id],
            )
            .map_err(map_database_error)?;
            let admitted_state = AggregateStateRef {
                blob: admitted_blob,
                entity_revision: 2,
                record_schema_version: 1,
            };
            let message_event = insert_domain_event(&tx, &commit.message_event, &admitted_state)?;
            let turn_bytes = canonical_json(&commit.turn)?;
            let turn_blob = blobs.put(
                &workspace,
                BlobPurpose::AggregateState,
                &turn_bytes,
                super::STATE_MEDIA_TYPE,
            )?;
            if turn_blob.size_bytes != turn_bytes.len() as u64
                || blobs.get(&workspace, BlobPurpose::AggregateState, &turn_blob)? != turn_bytes
            {
                return Err(StoreError::Integrity(
                    "ConversationTurn aggregate state failed BlobStore verification".to_owned(),
                ));
            }
            let turn_state = AggregateStateRef {
                blob: turn_blob,
                entity_revision: 1,
                record_schema_version: 1,
            };
            let turn_event = insert_domain_event(&tx, &commit.turn_event, &turn_state)?;
            let accepted = CommittedConversationFirstTurn {
                conversation: admitted_conversation,
                message: commit.message.clone(),
                turn: commit.turn.clone(),
                events: vec![created_event, message_event, turn_event],
            };
            let response_json = String::from_utf8(canonical_json(&accepted)?)
                .map_err(|error| StoreError::Invalid(error.to_string()))?;
            tx.execute(
                "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,NULL)",
                params![
                    principal,
                    commit.request.request_id,
                    request_digest,
                    response_json,
                    digest(response_json.as_bytes()),
                    commit.turn.created_at
                ],
            )
            .map_err(map_database_error)?;
            tx.commit().map_err(map_database_error)?;
            Ok(accepted)
        })
    }

    fn settle_conversation_turn(
        &self,
        settlement: SettleConversationTurn,
    ) -> Result<SettledConversationTurn, StoreError> {
        let valid_identity = |value: &str| {
            !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
        };
        if !valid_identity(&settlement.principal_id)
            || !valid_identity(&settlement.workspace_id)
            || !valid_identity(&settlement.conversation_id)
            || !valid_identity(&settlement.turn_id)
            || settlement.expected_turn_version == 0
            || !matches!(
                settlement.command,
                TurnCommand::Fail { .. } | TurnCommand::SettleCancelled { .. }
            )
            || settlement.session.as_ref().is_some_and(|session| {
                !valid_identity(&session.agent_session_id) || session.expected_session_version == 0
            })
        {
            return Err(StoreError::Invalid(
                "Conversation turn settlement identity is invalid".to_owned(),
            ));
        }
        let mut settlement = settlement;
        if let Some(session) = settlement.session.as_mut() {
            session.occurred_at = canonicalize_utc_timestamp(&session.occurred_at)?;
        }
        let blobs = Arc::clone(&self.store.inner.blobs);
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            let allowed: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
                params![settlement.workspace_id, settlement.principal_id], |row| row.get(0),
            ).map_err(map_database_error)?;
            if !allowed { return Err(StoreError::NotFound); }
            let current = load_turn(&tx, &settlement.workspace_id, &settlement.conversation_id, &settlement.turn_id)?
                .ok_or(StoreError::NotFound)?;
            if current.version != settlement.expected_turn_version {
                return Err(StoreError::Conflict { expected: Some(settlement.expected_turn_version), actual: Some(current.version) });
            }
            let next = domain_conversation::transition(&current, settlement.expected_turn_version, settlement.command.clone())
                .map_err(|error| match error {
                    domain_conversation::TurnTransitionError::StaleVersion => StoreError::Conflict { expected: Some(settlement.expected_turn_version), actual: Some(current.version) },
                    other => StoreError::Invalid(format!("Conversation turn transition rejected: {other:?}")),
                })?;
            validate_turn_transition_event(&settlement.turn_event, &current, &next, &settlement.command)?;
            if settlement.turn_event.workspace_id != settlement.workspace_id {
                return Err(StoreError::Invalid("ConversationTurn event Workspace mismatch".to_owned()));
            }

            let mut ended_session = None;
            if let Some(end) = settlement.session.as_ref() {
                let mut session = load_agent_session(&tx, &settlement.workspace_id, &end.agent_session_id)?
                    .ok_or(StoreError::NotFound)?;
                if session.scope_kind != "CONVERSATION"
                    || session.conversation_id.as_deref() != Some(settlement.conversation_id.as_str())
                    || session.conversation_turn_id.as_deref() != Some(settlement.turn_id.as_str())
                {
                    return Err(StoreError::Invalid("AgentSession scope does not match the settled turn".to_owned()));
                }
                if session.version != end.expected_session_version {
                    return Err(StoreError::Conflict { expected: Some(end.expected_session_version), actual: Some(session.version) });
                }
                let (target_status, expected_event_type) = match end.end {
                    ConversationSessionEnd::Closed => ("CLOSED", "agent.session.closed.v1"),
                    ConversationSessionEnd::Lost => ("LOST", "agent.session.lost.v1"),
                };
                let status_ok = match end.end {
                    ConversationSessionEnd::Closed => matches!(session.status.as_str(), "ACTIVE" | "INTERRUPTING" | "CLOSING"),
                    ConversationSessionEnd::Lost => matches!(session.status.as_str(), "STARTING" | "ACTIVE" | "INTERRUPTING" | "CLOSING"),
                };
                if !status_ok {
                    return Err(StoreError::Invalid(format!("AgentSession cannot end {target_status} from {}", session.status)));
                }
                let next_session_version = session.version.checked_add(1)
                    .ok_or_else(|| StoreError::Invalid("AgentSession version exhausted".to_owned()))?;
                if end.session_event.workspace_id != settlement.workspace_id
                    || end.session_event.entity_type != "AgentSession"
                    || end.session_event.entity_id != session.agent_session_id
                    || end.session_event.event_type != expected_event_type
                    || end.session_event.entity_revision != next_session_version
                {
                    return Err(StoreError::Invalid("AgentSession settlement event is inconsistent".to_owned()));
                }
                let updated = tx.execute(
                    "UPDATE agent_sessions SET status=?1, closed_at=?2, last_event_at=?2, version=?3 WHERE workspace_id=?4 AND agent_session_id=?5 AND version=?6",
                    params![target_status, end.occurred_at, to_sql_i64(next_session_version, "AgentSession version")?, settlement.workspace_id, session.agent_session_id, to_sql_i64(end.expected_session_version, "AgentSession version")?],
                ).map_err(map_database_error)?;
                if updated != 1 {
                    return Err(StoreError::Conflict { expected: Some(end.expected_session_version), actual: None });
                }
                if end.end == ConversationSessionEnd::Closed {
                    tx.execute(
                        "UPDATE agent_host_instances SET state='READY', last_used_at=?1, idle_since=?1 WHERE host_instance_id=(SELECT host_instance_id FROM agent_session_host_bindings WHERE agent_session_id=?2)",
                        params![end.occurred_at, session.agent_session_id],
                    ).map_err(map_database_error)?;
                }
                tx.execute(
                    "DELETE FROM agent_session_host_bindings WHERE agent_session_id=?1",
                    params![session.agent_session_id],
                ).map_err(map_database_error)?;
                session.status = target_status.to_owned();
                session.closed_at = Some(end.occurred_at.clone());
                session.last_event_at = Some(end.occurred_at.clone());
                session.version = next_session_version;
                ended_session = Some(session);
            }

            let state_bytes = canonical_json(&next)?;
            let blob = blobs.put(&settlement.workspace_id, BlobPurpose::AggregateState, &state_bytes, super::STATE_MEDIA_TYPE)?;
            let state_ref = AggregateStateRef { blob, entity_revision: next.version, record_schema_version: 1 };
            let next_version = i64::try_from(next.version).map_err(|_| StoreError::Invalid("ConversationTurn version exhausted".to_owned()))?;
            let updated = tx.execute(
                "UPDATE conversation_turns SET status=?1,settled_at=?2,version=?3 WHERE turn_id=?4 AND conversation_id=?5 AND version=?6",
                params![status_string(next.status)?, next.settled_at, next_version, settlement.turn_id, settlement.conversation_id, i64::try_from(settlement.expected_turn_version).unwrap_or(-1)],
            ).map_err(map_database_error)?;
            if updated != 1 {
                return Err(StoreError::Conflict { expected: Some(settlement.expected_turn_version), actual: None });
            }
            tx.execute(
                "UPDATE conversation_dispatch_intents SET state='SETTLED', settled_at=?1, version=version+1 WHERE turn_id=?2 AND retry_ordinal=?3 AND state<>'SETTLED'",
                params![next.settled_at, settlement.turn_id, i64::from(current.retry_ordinal)],
            ).map_err(map_database_error)?;
            // B-10: one usage row per attempt. ON CONFLICT DO NOTHING keeps a replay or an
            // earlier row from failing the settlement.
            let (attribution, input_tokens, output_tokens) = match settlement.usage {
                Some(usage) => (
                    match usage.attribution_source {
                        storage_core::conversation::UsageAttributionSource::ProviderReported => "PROVIDER_REPORTED",
                        storage_core::conversation::UsageAttributionSource::EstimatedLocal => "ESTIMATED_LOCAL",
                    },
                    Some(to_sql_i64(usage.input_tokens, "input tokens")?),
                    Some(to_sql_i64(usage.output_tokens, "output tokens")?),
                ),
                None => ("UNKNOWN", None, None),
            };
            tx.execute(
                "INSERT INTO conversation_turn_usage_records(usage_id,turn_id,retry_ordinal,conversation_id,attribution_source,input_tokens,output_tokens,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT DO NOTHING",
                params![
                    format!("usage-{}-{}", settlement.turn_id, current.retry_ordinal),
                    settlement.turn_id,
                    i64::from(current.retry_ordinal),
                    settlement.conversation_id,
                    attribution,
                    input_tokens,
                    output_tokens,
                    next.settled_at,
                ],
            ).map_err(map_database_error)?;
            if let Some(control) = settlement.control_event.as_ref() {
                if !matches!(settlement.command, TurnCommand::SettleCancelled { .. }) {
                    return Err(StoreError::Invalid(
                        "a control event requires a CANCELLED settlement".to_owned(),
                    ));
                }
                tx.execute(
                    "INSERT INTO conversation_control_events(control_event_id,workspace_id,conversation_id,turn_id,kind,occurred_at) VALUES(?1,?2,?3,?4,'OWNER_TOOK_CONTROL',?5)",
                    params![
                        control.control_event_id,
                        settlement.workspace_id,
                        settlement.conversation_id,
                        settlement.turn_id,
                        control.occurred_at,
                    ],
                )
                .map_err(map_database_error)?;
            }
            insert_domain_event(&tx, &settlement.turn_event, &state_ref)?;
            if let (Some(end), Some(session)) = (settlement.session.as_ref(), ended_session.as_ref()) {
                let session_bytes = canonical_json(session)?;
                let session_blob = blobs.put(&settlement.workspace_id, BlobPurpose::AggregateState, &session_bytes, "application/vnd.litecowork.agent-session+json")?;
                let session_ref = AggregateStateRef { blob: session_blob, entity_revision: session.version, record_schema_version: 1 };
                insert_domain_event(&tx, &end.session_event, &session_ref)?;
            }
            tx.commit().map_err(map_database_error)?;
            Ok(SettledConversationTurn { turn: next, session: ended_session })
        })
    }

    fn find_conversation_turn_command_replay(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        request_id: &str,
    ) -> Result<Option<ConversationTurn>, StoreError> {
        if workspace_id.trim().is_empty()
            || turn_id.trim().is_empty()
            || request_id.trim().is_empty()
        {
            return Err(StoreError::Invalid(
                "Conversation command replay identity is invalid".to_owned(),
            ));
        }
        let (workspace, conversation, turn, request) = (
            workspace_id.to_owned(),
            conversation_id.to_owned(),
            turn_id.to_owned(),
            request_id.to_owned(),
        );
        self.run(move |connection| {
            let replayed: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM domain_events WHERE workspace_id=?1 AND entity_type='ConversationTurn' AND entity_id=?2 AND causation_id=?3)",
                params![workspace, turn, request], |row| row.get(0),
            ).map_err(map_database_error)?;
            if !replayed { return Ok(None); }
            load_turn(connection, &workspace, &conversation, &turn)
        })
    }

    fn claim_conversation_dispatch_intent(
        &self,
        turn_id: &str,
        retry_ordinal: u32,
        runtime_incarnation_id: &str,
        claimed_at: &str,
    ) -> Result<bool, StoreError> {
        if turn_id.trim().is_empty() || runtime_incarnation_id.trim().is_empty() {
            return Err(StoreError::Invalid(
                "Conversation dispatch claim identity is invalid".to_owned(),
            ));
        }
        let claimed_at = canonicalize_utc_timestamp(claimed_at)?;
        let (turn_id, incarnation) = (turn_id.to_owned(), runtime_incarnation_id.to_owned());
        self.run(move |connection| {
            let changed = connection
                .execute(
                    "UPDATE conversation_dispatch_intents SET state='CLAIMED', claimed_by_runtime_incarnation=?1, claimed_at=?2, version=version+1 WHERE turn_id=?3 AND retry_ordinal=?4 AND state='UNCLAIMED'",
                    params![incarnation, claimed_at, turn_id, i64::from(retry_ordinal)],
                )
                .map_err(map_database_error)?;
            Ok(changed == 1)
        })
    }

    fn release_conversation_dispatch_intent(
        &self,
        turn_id: &str,
        retry_ordinal: u32,
    ) -> Result<bool, StoreError> {
        if turn_id.trim().is_empty() {
            return Err(StoreError::Invalid(
                "Conversation dispatch release identity is invalid".to_owned(),
            ));
        }
        let turn_id = turn_id.to_owned();
        self.run(move |connection| {
            let changed = connection
                .execute(
                    "UPDATE conversation_dispatch_intents SET state='UNCLAIMED', claimed_by_runtime_incarnation=NULL, claimed_at=NULL, version=version+1 WHERE turn_id=?1 AND retry_ordinal=?2 AND state='CLAIMED'",
                    params![turn_id, i64::from(retry_ordinal)],
                )
                .map_err(map_database_error)?;
            Ok(changed == 1)
        })
    }

    fn list_conversation_dispatch_intents(
        &self,
        state: ConversationDispatchState,
        limit: usize,
    ) -> Result<Vec<ConversationDispatchIntent>, StoreError> {
        if !(1..=1024).contains(&limit) {
            return Err(StoreError::Invalid(
                "Conversation dispatch intent page size is invalid".to_owned(),
            ));
        }
        self.run(move |connection| {
            let mut statement = connection
                .prepare(
                    "SELECT c.workspace_id,i.conversation_id,i.turn_id,i.retry_ordinal,i.state,i.claimed_by_runtime_incarnation,i.created_at FROM conversation_dispatch_intents i JOIN conversations c ON c.conversation_id=i.conversation_id WHERE i.state=?1 ORDER BY i.created_at,i.turn_id,i.retry_ordinal LIMIT ?2",
                )
                .map_err(map_database_error)?;
            let rows = statement
                .query_map(params![state.as_str(), limit as i64], |row| {
                    Ok(ConversationDispatchIntent {
                        workspace_id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        turn_id: row.get(2)?,
                        retry_ordinal: u32::try_from(row.get::<_, i64>(3)?).unwrap_or(u32::MAX),
                        state,
                        claimed_by_runtime_incarnation: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                })
                .map_err(map_database_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_database_error)
        })
    }

    fn append_conversation_turn_activity(
        &self,
        record: storage_core::conversation::ConversationActivityRecord,
    ) -> Result<(), StoreError> {
        let valid = |value: &str| {
            !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
        };
        if !valid(&record.turn_id)
            || !valid(&record.conversation_id)
            || record.sequence == 0
            || record.sequence as usize
                > storage_core::conversation::CONVERSATION_ACTIVITY_MAX_PER_ATTEMPT
            || record
                .call_ref
                .as_deref()
                .is_some_and(|v| !valid(v) || v.len() > 128)
            || record
                .label
                .as_deref()
                .is_some_and(|v| v.len() > 128 || v.chars().any(char::is_control))
        {
            return Err(StoreError::Invalid(
                "Conversation activity record is invalid".to_owned(),
            ));
        }
        self.run(move |connection| {
            connection
                .execute(
                    "INSERT INTO conversation_turn_activity_records(activity_id,turn_id,retry_ordinal,conversation_id,sequence,kind,status,call_ref,label,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT DO NOTHING",
                    params![
                        format!("activity-{}-{}-{}", record.turn_id, record.retry_ordinal, record.sequence),
                        record.turn_id,
                        i64::from(record.retry_ordinal),
                        record.conversation_id,
                        i64::from(record.sequence),
                        record.kind,
                        record.status,
                        record.call_ref,
                        record.label,
                        record.created_at,
                    ],
                )
                .map_err(map_database_error)?;
            Ok(())
        })
    }

    fn list_conversation_turn_activity(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
        retry_ordinal: u32,
        limit: usize,
    ) -> Result<Vec<storage_core::conversation::ConversationActivityRecord>, StoreError> {
        if !(1..=storage_core::conversation::CONVERSATION_ACTIVITY_MAX_PER_ATTEMPT).contains(&limit)
        {
            return Err(StoreError::Invalid(
                "Conversation activity page size is invalid".to_owned(),
            ));
        }
        let workspace = workspace_id.to_owned();
        let conversation = conversation_id.to_owned();
        let turn = turn_id.to_owned();
        self.run(move |connection| {
            let mut statement = connection
                .prepare(
                    "SELECT a.turn_id,a.retry_ordinal,a.conversation_id,a.sequence,a.kind,a.status,a.call_ref,a.label,a.created_at FROM conversation_turn_activity_records a JOIN conversations c ON c.conversation_id=a.conversation_id WHERE c.workspace_id=?1 AND a.conversation_id=?2 AND a.turn_id=?3 AND a.retry_ordinal=?4 ORDER BY a.sequence LIMIT ?5",
                )
                .map_err(map_database_error)?;
            let rows = statement
                .query_map(
                    params![workspace, conversation, turn, i64::from(retry_ordinal), limit as i64],
                    |row| {
                        Ok(storage_core::conversation::ConversationActivityRecord {
                            turn_id: row.get(0)?,
                            retry_ordinal: u32::try_from(row.get::<_, i64>(1)?).unwrap_or(u32::MAX),
                            conversation_id: row.get(2)?,
                            sequence: u32::try_from(row.get::<_, i64>(3)?).unwrap_or(u32::MAX),
                            kind: row.get(4)?,
                            status: row.get(5)?,
                            call_ref: row.get(6)?,
                            label: row.get(7)?,
                            created_at: row.get(8)?,
                        })
                    },
                )
                .map_err(map_database_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_database_error)
        })
    }

    fn commit_conversation_agent_response(
        &self,
        mut commit: CommitConversationAgentResponse,
    ) -> Result<CommittedConversationAgentResponse, StoreError> {
        let valid_identity = |value: &str| {
            !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
        };
        if !valid_identity(&commit.principal_id)
            || !valid_identity(&commit.workspace_id)
            || !valid_identity(&commit.conversation_id)
            || !valid_identity(&commit.turn_id)
            || !valid_identity(&commit.agent_session_id)
            || !valid_identity(&commit.message.message_id)
            || !valid_identity(&commit.provider_response_id)
            || commit.expected_turn_version == 0
            || commit.expected_session_version == 0
        {
            return Err(StoreError::Invalid(
                "Conversation agent response commit identity is invalid".to_owned(),
            ));
        }

        // B-04 is the final Agent-result transaction. User-request waiting and
        // continuation transitions belong to their separate B-06 operations; they
        // must not write a final response, settle the turn, or close its session.
        if !commit.turn_status.is_terminal() {
            return Err(StoreError::Invalid(
                "Conversation response commit requires a terminal turn status".to_owned(),
            ));
        }

        commit.message.created_at = canonicalize_utc_timestamp(&commit.message.created_at)?;
        commit.turn_event.recorded_at = canonicalize_utc_timestamp(&commit.turn_event.recorded_at)?;
        commit.session_event.recorded_at =
            canonicalize_utc_timestamp(&commit.session_event.recorded_at)?;
        commit.message_event.recorded_at =
            canonicalize_utc_timestamp(&commit.message_event.recorded_at)?;

        let principal = commit.principal_id.clone();
        let blobs = Arc::clone(&self.store.inner.blobs);

        self.run(move |connection| {
            let request_digest = digest(canonical_json_string(&commit.request.request_payload)?.as_bytes());
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;

            if let Some(replay) = verify_request_receipt::<CommittedConversationAgentResponse>(
                &tx,
                &principal,
                &commit.request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }

            // 1. Verify workspace exists, active, owned by principal
            let workspace_owner: Option<(String, String)> = tx
                .query_row(
                    "SELECT owner_principal_id, status FROM workspaces WHERE workspace_id=?1",
                    params![commit.workspace_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(map_database_error)?;
            let Some((owner, workspace_status)) = workspace_owner else {
                return Err(StoreError::NotFound);
            };
            if owner != principal {
                return Err(StoreError::Invalid(
                    "authenticated Principal does not own this Workspace".to_owned(),
                ));
            }
            if workspace_status != "ACTIVE" {
                return Err(StoreError::Invalid("Workspace is not active".to_owned()));
            }

            // 2. Verify conversation exists
            let conversation_row: Option<(Option<String>, i64, String, Option<String>)> = tx
                .query_row(
                    "SELECT title, version, created_at, active_agent_binding_id FROM conversations WHERE workspace_id=?1 AND conversation_id=?2",
                    params![commit.workspace_id, commit.conversation_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(map_database_error)?;
            let Some((title, conv_version, created_at, active_binding)) = conversation_row else {
                return Err(StoreError::NotFound);
            };

            // 2b. Final-response identity (B-04): (turn, retry ordinal, provider response).
            // An exact repeat replays the stored receipt even after the turn has moved on;
            // any other repeat of the same (turn, retry ordinal) is rejected before any write.
            let content_digest = digest(&canonical_json(&commit.message.content)?);
            let stored_identity: Option<(String, String, String)> = tx
                .query_row(
                    "SELECT provider_response_id,content_digest,receipt_request_id FROM conversation_response_commits WHERE turn_id=?1 AND retry_ordinal=?2",
                    params![commit.turn_id, i64::from(commit.retry_ordinal)],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(map_database_error)?;
            if let Some((stored_response_id, stored_digest, stored_request_id)) = stored_identity {
                if stored_response_id != commit.provider_response_id || stored_digest != content_digest {
                    return Err(StoreError::Invalid(CONVERSATION_RESPONSE_IDENTITY_CONFLICT.to_owned()));
                }
                let stored: Option<String> = tx
                    .query_row(
                        "SELECT response_json FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
                        params![principal, stored_request_id],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(map_database_error)?;
                let stored = stored.ok_or_else(|| {
                    StoreError::Integrity("Conversation response commit has no stored receipt".to_owned())
                })?;
                let replay: CommittedConversationAgentResponse = serde_json::from_str(&stored)
                    .map_err(|error| StoreError::Integrity(error.to_string()))?;
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }

            // 3. Verify the turn version. A single Stop transition may advance the
            // activation version before the provider's already-produced final answer
            // is committed; no other intervening turn mutation is accepted here.
            let current_turn = load_turn(&tx, &commit.workspace_id, &commit.conversation_id, &commit.turn_id)?
                .ok_or(StoreError::NotFound)?;
            let stop_advanced_version = commit.expected_turn_version.checked_add(1);
            let completion_after_stop = current_turn.status == ConversationTurnStatus::CancelRequested
                && commit.turn_status == ConversationTurnStatus::Completed
                && (commit.expected_turn_version == current_turn.version
                    || stop_advanced_version == Some(current_turn.version));
            let running_at_activation_version = current_turn.status == ConversationTurnStatus::Running
                && current_turn.version == commit.expected_turn_version;
            if !running_at_activation_version && !completion_after_stop {
                return Err(StoreError::Conflict {
                    expected: Some(commit.expected_turn_version),
                    actual: Some(current_turn.version),
                });
            }
            let turn_event_from = status_string(current_turn.status)?;
            let turn_event_to = status_string(commit.turn_status)?;
            if current_turn.retry_ordinal != commit.retry_ordinal {
                return Err(StoreError::Invalid(
                    "ConversationTurn retry ordinal does not match commit".to_owned(),
                ));
            }
            if current_turn.agent_session_id.as_deref() != Some(commit.agent_session_id.as_str()) {
                return Err(StoreError::Invalid(
                    "ConversationTurn active agent session does not match commit".to_owned(),
                ));
            }

            // 4. Verify agent session exists and is ACTIVE with expected_session_version
            let mut current_session = load_agent_session(&tx, &commit.workspace_id, &commit.agent_session_id)?
                .ok_or(StoreError::NotFound)?;
            if current_session.version != commit.expected_session_version {
                return Err(StoreError::Conflict {
                    expected: Some(commit.expected_session_version),
                    actual: Some(current_session.version),
                });
            }
            if current_session.status != "ACTIVE" {
                return Err(StoreError::Invalid(
                    format!("AgentSession is not ACTIVE (status: {})", current_session.status),
                ));
            }
            if current_session.scope_kind != "CONVERSATION"
                || current_session.conversation_id.as_deref() != Some(commit.conversation_id.as_str())
                || current_session.conversation_turn_id.as_deref() != Some(commit.turn_id.as_str())
            {
                return Err(StoreError::Invalid(
                    "AgentSession scope does not match committed turn".to_owned(),
                ));
            }

            // 4b. The final message must be authored by the AgentBinding this session
            // was pinned to, in this Conversation; the caller cannot attribute it elsewhere.
            let expected_author = serde_json::json!({
                "kind": "AGENT",
                "agent_binding_id": current_session.agent_binding_id,
            });
            if commit.message.author != expected_author
                || commit.message.conversation_id != commit.conversation_id
            {
                return Err(StoreError::Invalid(
                    "Conversation response author does not match the turn-pinned AgentBinding".to_owned(),
                ));
            }

            // 5. Version bumps
            let next_conversation_version = conv_version
                .checked_add(1)
                .ok_or_else(|| StoreError::Invalid("Conversation version exhausted".to_owned()))?;
            let next_turn_version = current_turn.version
                .checked_add(1)
                .ok_or_else(|| StoreError::Invalid("ConversationTurn version exhausted".to_owned()))?;
            let next_session_version = commit.expected_session_version
                .checked_add(1)
                .ok_or_else(|| StoreError::Invalid("AgentSession version exhausted".to_owned()))?;

            // 6. Update conversation
            tx.execute(
                "UPDATE conversations SET version=?1 WHERE workspace_id=?2 AND conversation_id=?3 AND version=?4",
                params![next_conversation_version, commit.workspace_id, commit.conversation_id, conv_version],
            )
            .map_err(map_database_error)?;

            // 7. Insert final AGENT message
            commit.message.agent_binding_id = Some(current_session.agent_binding_id.clone());
            commit.message.agent_session_id = Some(commit.agent_session_id.clone());
            commit.message.turn_id = Some(commit.turn_id.clone());
            commit.message.role = "AGENT".to_owned();

            tx.execute(
                "INSERT INTO conversation_messages(message_id,conversation_id,author_json,role,agent_session_id,agent_binding_id,turn_id,content_json,resource_refs_json,source_channel_ref_json,created_at) VALUES(?1,?2,?3,'AGENT',?4,?5,?6,?7,?8,NULL,?9)",
                params![
                    commit.message.message_id,
                    commit.conversation_id,
                    canonical_json_string(&commit.message.author)?,
                    commit.agent_session_id,
                    current_session.agent_binding_id,
                    commit.turn_id,
                    canonical_json_string(&commit.message.content)?,
                    canonical_json_string(&commit.message.resource_refs)?,
                    commit.message.created_at,
                ],
            )
            .map_err(map_database_error)?;

            tx.execute(
                "INSERT INTO conversation_response_commits(turn_id,retry_ordinal,provider_response_id,content_digest,message_id,receipt_request_id,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    commit.turn_id,
                    i64::from(commit.retry_ordinal),
                    commit.provider_response_id,
                    content_digest,
                    commit.message.message_id,
                    commit.request.request_id,
                    commit.message.created_at,
                ],
            )
            .map_err(map_database_error)?;

            // 8. Settle conversation turn
            let turn_status_db_str = match commit.turn_status {
                ConversationTurnStatus::Completed => "COMPLETED",
                ConversationTurnStatus::Failed => "FAILED",
                ConversationTurnStatus::Cancelled => "CANCELLED",
                _ => return Err(StoreError::Invalid("Unsupported terminal turn status".to_owned())),
            };
            let turn_settled = tx.execute(
                "UPDATE conversation_turns SET status=?1, settled_at=?2, version=?3 WHERE turn_id=?4 AND conversation_id=?5 AND status=?6 AND version=?7",
                params![
                    turn_status_db_str,
                    commit.message.created_at,
                    to_sql_i64(next_turn_version, "ConversationTurn version")?,
                    commit.turn_id,
                    commit.conversation_id,
                    status_string(current_turn.status)?,
                    to_sql_i64(current_turn.version, "ConversationTurn version")?,
                ],
            )
            .map_err(map_database_error)?;
            if turn_settled != 1 {
                return Err(StoreError::Conflict {
                    expected: Some(current_turn.version),
                    actual: None,
                });
            }

            // 8b. Settle the dispatch intent and record the turn usage (B-10).
            tx.execute(
                "UPDATE conversation_dispatch_intents SET state='SETTLED', settled_at=?1, version=version+1 WHERE turn_id=?2 AND retry_ordinal=?3 AND state<>'SETTLED'",
                params![commit.message.created_at, commit.turn_id, i64::from(commit.retry_ordinal)],
            )
            .map_err(map_database_error)?;
            let (attribution, input_tokens, output_tokens) = match commit.usage {
                Some(usage) => (
                    match usage.attribution_source {
                        storage_core::conversation::UsageAttributionSource::ProviderReported => "PROVIDER_REPORTED",
                        storage_core::conversation::UsageAttributionSource::EstimatedLocal => "ESTIMATED_LOCAL",
                    },
                    Some(to_sql_i64(usage.input_tokens, "input tokens")?),
                    Some(to_sql_i64(usage.output_tokens, "output tokens")?),
                ),
                None => ("UNKNOWN", None, None),
            };
            tx.execute(
                "INSERT INTO conversation_turn_usage_records(usage_id,turn_id,retry_ordinal,conversation_id,attribution_source,input_tokens,output_tokens,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    format!("usage-{}-{}", commit.turn_id, commit.retry_ordinal),
                    commit.turn_id,
                    i64::from(commit.retry_ordinal),
                    commit.conversation_id,
                    attribution,
                    input_tokens,
                    output_tokens,
                    commit.message.created_at,
                ],
            )
            .map_err(map_database_error)?;

            // 9. Close agent session
            let session_closed = tx.execute(
                "UPDATE agent_sessions SET status='CLOSED', closed_at=?1, last_event_at=?1, version=?2 WHERE workspace_id=?3 AND agent_session_id=?4 AND status='ACTIVE' AND version=?5",
                params![
                    commit.message.created_at,
                    to_sql_i64(next_session_version, "AgentSession version")?,
                    commit.workspace_id,
                    commit.agent_session_id,
                    to_sql_i64(commit.expected_session_version, "AgentSession version")?,
                ],
            )
            .map_err(map_database_error)?;
            if session_closed != 1 {
                return Err(StoreError::Conflict {
                    expected: Some(commit.expected_session_version),
                    actual: None,
                });
            }

            // 10. Update host instance to READY and idle_since
            tx.execute(
                "UPDATE agent_host_instances SET state='READY', last_used_at=?1, idle_since=?1 WHERE host_instance_id=(SELECT host_instance_id FROM agent_session_host_bindings WHERE agent_session_id=?2)",
                params![commit.message.created_at, commit.agent_session_id],
            )
            .map_err(map_database_error)?;

            // 11. Delete ephemeral host binding
            tx.execute(
                "DELETE FROM agent_session_host_bindings WHERE agent_session_id=?1",
                params![commit.agent_session_id],
            )
            .map_err(map_database_error)?;

            // 12. Aggregate states -> BlobStore
            // Conversation
            let conv_state = ConversationRecord {
                conversation_id: commit.conversation_id.clone(),
                workspace_id: commit.workspace_id.clone(),
                title: title.clone(),
                active_agent_binding_id: active_binding,
                version: next_conversation_version as u64,
                created_at: created_at.clone(),
            };
            let conv_bytes = canonical_json(&conv_state)?;
            let conv_blob = blobs.put(
                &commit.workspace_id,
                BlobPurpose::AggregateState,
                &conv_bytes,
                super::STATE_MEDIA_TYPE,
            )?;
            let conv_state_ref = AggregateStateRef {
                blob: conv_blob,
                entity_revision: next_conversation_version as u64,
                record_schema_version: 1,
            };

            // ConversationTurn
            let mut settled_turn = current_turn;
            settled_turn.status = commit.turn_status;
            settled_turn.settled_at = Some(commit.message.created_at.clone());
            settled_turn.version = next_turn_version;
            let turn_bytes = canonical_json(&settled_turn)?;
            let turn_blob = blobs.put(
                &commit.workspace_id,
                BlobPurpose::AggregateState,
                &turn_bytes,
                super::STATE_MEDIA_TYPE,
            )?;
            let turn_state_ref = AggregateStateRef {
                blob: turn_blob,
                entity_revision: next_turn_version,
                record_schema_version: 1,
            };
            // AgentSession
            current_session.status = "CLOSED".to_owned();
            current_session.closed_at = Some(commit.message.created_at.clone());
            current_session.last_event_at = Some(commit.message.created_at.clone());
            current_session.version = next_session_version;
            let session_bytes = canonical_json(&current_session)?;
            let session_blob = blobs.put(
                &commit.workspace_id,
                BlobPurpose::AggregateState,
                &session_bytes,
                "application/vnd.litecowork.agent-session+json",
            )?;
            let session_state_ref = AggregateStateRef {
                blob: session_blob,
                entity_revision: next_session_version,
                record_schema_version: 1,
            };

            // 13. Domain events
            commit.message_event.entity_revision = next_conversation_version as u64;
            let message_event = insert_domain_event(&tx, &commit.message_event, &conv_state_ref)?;

            // The event edge and aggregate version must reflect the durable state
            // that actually won the race, including the committed Stop request.
            commit.turn_event.payload["from"] = serde_json::json!(turn_event_from);
            commit.turn_event.payload["to"] = serde_json::json!(turn_event_to);
            commit.turn_event.payload["aggregate_version"] = serde_json::json!(next_turn_version);
            commit.turn_event.entity_revision = next_turn_version;
            let turn_event = insert_domain_event(&tx, &commit.turn_event, &turn_state_ref)?;

            commit.session_event.entity_revision = next_session_version;
            let session_event = insert_domain_event(&tx, &commit.session_event, &session_state_ref)?;

            // 14. Dedup receipt
            let accepted = CommittedConversationAgentResponse {
                message: commit.message.clone(),
                turn: settled_turn,
                session: current_session,
                message_event,
                turn_event,
                session_event,
            };
            let response_json = String::from_utf8(canonical_json(&accepted)?)
                .map_err(|error| StoreError::Invalid(error.to_string()))?;
            tx.execute(
                "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,NULL)",
                params![
                    principal,
                    commit.request.request_id,
                    request_digest,
                    response_json,
                    digest(response_json.as_bytes()),
                    commit.message.created_at,
                ],
            )
            .map_err(map_database_error)?;

            tx.commit().map_err(map_database_error)?;
            Ok(accepted)
        })
    }

    fn load_conversation_context_for_turn(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
    ) -> Result<ConversationContextPackage, StoreError> {
        if principal_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || conversation_id.trim().is_empty()
            || turn_id.trim().is_empty()
        {
            return Err(StoreError::Invalid(
                "Conversation context read identity is invalid".to_owned(),
            ));
        }
        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let conversation = conversation_id.to_owned();
        let turn = turn_id.to_owned();
        self.run(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            let allowed: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
                    params![workspace, principal],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !allowed {
                return Err(StoreError::NotFound);
            }

            let (through_message_id, through_revision, target_event_count): (
                String,
                Option<i64>,
                i64,
            ) = transaction
                .query_row(
                    "SELECT t.user_message_id,MIN(e.entity_revision),COUNT(e.event_id) FROM conversation_turns t JOIN conversations c ON c.conversation_id=t.conversation_id JOIN conversation_messages m ON m.message_id=t.user_message_id AND m.conversation_id=t.conversation_id LEFT JOIN domain_events e ON e.workspace_id=c.workspace_id AND e.entity_type='Conversation' AND e.entity_id=t.conversation_id AND e.type='conversation.message.added.v1' AND json_extract(e.payload_json,'$.message_id')=m.message_id WHERE c.workspace_id=?1 AND t.conversation_id=?2 AND t.turn_id=?3 GROUP BY t.user_message_id",
                    params![workspace, conversation, turn],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(map_database_error)?
                .ok_or(StoreError::NotFound)?;
            if target_event_count != 1 || through_revision.is_none_or(|revision| revision <= 0) {
                return Err(StoreError::Integrity(
                    "Conversation turn user message has no unique append revision".to_owned(),
                ));
            }
            let through_revision = through_revision.expect("validated message revision");
            let orphan_message_count: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM conversation_messages m WHERE m.conversation_id=?1 AND NOT EXISTS (SELECT 1 FROM domain_events e WHERE e.workspace_id=?2 AND e.entity_type='Conversation' AND e.entity_id=m.conversation_id AND e.type='conversation.message.added.v1' AND json_extract(e.payload_json,'$.message_id')=m.message_id)",
                    params![conversation, workspace],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if orphan_message_count != 0 {
                return Err(StoreError::Integrity(
                    "Conversation contains a message without its append event".to_owned(),
                ));
            }

            let event_payload: String = transaction
                .query_row(
                    "SELECT payload_json FROM domain_events WHERE workspace_id=?1 AND entity_type='ConversationTurn' AND entity_id=?2 AND type='conversation.turn.created.v2'",
                    params![workspace, turn],
                    |row| row.get(0),
                )
                .optional()
                .map_err(map_database_error)?
                .ok_or_else(|| StoreError::Integrity(
                    "Conversation turn creation event is missing".to_owned(),
                ))?;
            let event_payload: Value = serde_json::from_str(&event_payload)
                .map_err(|_| StoreError::Integrity(
                    "Conversation turn creation event is invalid".to_owned(),
                ))?;
            let target_agent_binding_id = event_payload
                .get("agent_binding_id")
                .and_then(Value::as_str)
                .ok_or_else(|| StoreError::Integrity(
                    "Conversation turn has no selected AgentBinding".to_owned(),
                ))?;

            let (message_count, context_bytes, event_count): (i64, i64, i64) = transaction
                .query_row(
                    "SELECT COUNT(DISTINCT m.message_id),COALESCE(SUM(length(CAST(m.message_id AS BLOB))+length(CAST(m.role AS BLOB))+length(CAST(m.content_json AS BLOB))+length(CAST(m.resource_refs_json AS BLOB))+length(CAST(m.created_at AS BLOB))+COALESCE(length(CAST(m.agent_binding_id AS BLOB)),0)),0),COUNT(*) FROM conversation_messages m JOIN domain_events e ON e.workspace_id=?2 AND e.entity_type='Conversation' AND e.entity_id=m.conversation_id AND e.type='conversation.message.added.v1' AND json_extract(e.payload_json,'$.message_id')=m.message_id WHERE m.conversation_id=?1 AND e.entity_revision<=?3",
                    params![conversation, workspace, through_revision],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(map_database_error)?;
            if event_count != message_count {
                return Err(StoreError::Integrity(
                    "Conversation message has duplicate append revisions".to_owned(),
                ));
            }
            // The B-07 window applies after this read: only the most recent
            // CONVERSATION_CONTEXT_MAX_MESSAGES committed messages are loaded and the
            // package builder reduces further by bytes, counting what it omits.
            if message_count < 0 || context_bytes < 0 {
                return Err(StoreError::Integrity(
                    "Conversation context size is invalid".to_owned(),
                ));
            }

            let mut statement = transaction
                .prepare(
                    "SELECT m.message_id,m.conversation_id,m.author_json,m.role,m.agent_session_id,m.agent_binding_id,m.turn_id,m.content_json,m.resource_refs_json,m.source_channel_ref_json,m.created_at,e.entity_revision FROM conversation_messages m JOIN domain_events e ON e.workspace_id=?2 AND e.entity_type='Conversation' AND e.entity_id=m.conversation_id AND e.type='conversation.message.added.v1' AND json_extract(e.payload_json,'$.message_id')=m.message_id WHERE m.conversation_id=?1 AND e.entity_revision<=?3 ORDER BY e.entity_revision DESC,m.message_id DESC LIMIT ?4",
                )
                .map_err(map_database_error)?;
            let mut rows = statement
                .query_map(
                    params![
                        conversation,
                        workspace,
                        through_revision,
                        CONVERSATION_CONTEXT_MAX_MESSAGES as i64
                    ],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<String>>(6)?,
                            row.get::<_, String>(7)?,
                            row.get::<_, String>(8)?,
                            row.get::<_, Option<String>>(9)?,
                            row.get::<_, String>(10)?,
                            row.get::<_, i64>(11)?,
                        ))
                    },
                )
                .map_err(map_database_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_database_error)?;
            drop(statement);
            rows.reverse();
            let oldest_loaded_revision = rows.first().map(|row| row.11);
            let messages = rows
                .into_iter()
                .map(|row| {
                    let conversation_revision = u64::try_from(row.11).map_err(|_| {
                        StoreError::Integrity("Conversation message revision is invalid".to_owned())
                    })?;
                    Ok(ConversationContextSourceMessage {
                        conversation_revision,
                        message: ConversationMessageRecord {
                            message_id: row.0,
                            conversation_id: row.1,
                            author: serde_json::from_str(&row.2).map_err(|_| {
                                StoreError::Integrity("Conversation message author is invalid".to_owned())
                            })?,
                            role: row.3,
                            agent_session_id: row.4,
                            agent_binding_id: row.5,
                            turn_id: row.6,
                            content: serde_json::from_str(&row.7).map_err(|_| {
                                StoreError::Integrity("Conversation message content is invalid".to_owned())
                            })?,
                            resource_refs: serde_json::from_str(&row.8).map_err(|_| {
                                StoreError::Integrity("Conversation message resources are invalid".to_owned())
                            })?,
                            source_channel_ref: row
                                .9
                                .map(|value| serde_json::from_str(&value))
                                .transpose()
                                .map_err(|_| StoreError::Integrity(
                                    "Conversation message channel reference is invalid".to_owned(),
                                ))?,
                            created_at: row.10,
                        },
                    })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;

            let mut context = build_conversation_context_package(
                &conversation,
                target_agent_binding_id,
                &through_message_id,
                &messages,
            )?;
            if let (true, Some(oldest)) = (
                message_count as usize > CONVERSATION_CONTEXT_MAX_MESSAGES,
                oldest_loaded_revision,
            ) {
                let older: i64 = transaction
                    .query_row(
                        "SELECT COUNT(DISTINCT m.message_id) FROM conversation_messages m JOIN domain_events e ON e.workspace_id=?2 AND e.entity_type='Conversation' AND e.entity_id=m.conversation_id AND e.type='conversation.message.added.v1' AND json_extract(e.payload_json,'$.message_id')=m.message_id WHERE m.conversation_id=?1 AND e.entity_revision<?3 AND m.role<>'SYSTEM_NOTICE'",
                        params![conversation, workspace, oldest],
                        |row| row.get(0),
                    )
                    .map_err(map_database_error)?;
                context.omitted_message_count = context
                    .omitted_message_count
                    .saturating_add(u32::try_from(older).unwrap_or(u32::MAX));
            }
            transaction.commit().map_err(map_database_error)?;
            Ok(context)
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
            if matches!(command, TurnCommand::Resume { .. }) {
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
            if matches!(command, TurnCommand::Retry) {
                let other_in_flight: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id=?1 AND turn_id<>?2 AND status IN ('OPEN','RUNNING','WAITING_USER','WAITING_DEPENDENCY','CANCEL_REQUESTED'))",
                    params![conversation, id], |row| row.get(0),
                ).map_err(map_database_error)?;
                if other_in_flight {
                    return Err(StoreError::Invalid(CONVERSATION_TURN_IN_PROGRESS.to_owned()));
                }
                // The new attempt owes a dispatch; the same transaction that reopens the
                // turn records it so a crash cannot strand an OPEN turn without an intent.
                tx.execute(
                    "INSERT INTO conversation_dispatch_intents(turn_id,retry_ordinal,conversation_id,state,created_at) VALUES(?1,?2,?3,'UNCLAIMED',?4)",
                    params![id, i64::from(next.retry_ordinal), conversation, event.recorded_at],
                ).map_err(map_database_error)?;
            }
            if next.status.is_terminal() {
                // A settled attempt owes no further dispatch (outbox state SETTLED).
                tx.execute(
                    "UPDATE conversation_dispatch_intents SET state='SETTLED', settled_at=?1, version=version+1 WHERE turn_id=?2 AND retry_ordinal=?3 AND state<>'SETTLED'",
                    params![next.settled_at, id, i64::from(current.retry_ordinal)],
                ).map_err(map_database_error)?;
            }
            insert_domain_event(&tx, &event, &state_ref)?;
            tx.commit().map_err(map_database_error)?;
            Ok(next)
        })
    }

    fn resolve_conversation_session_host_for_turn(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn_id: &str,
    ) -> Result<Option<ResolvedConversationHost>, StoreError> {
        if principal_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || conversation_id.trim().is_empty()
            || turn_id.trim().is_empty()
        {
            return Err(StoreError::Invalid(
                "Conversation host resolution identity is invalid".to_owned(),
            ));
        }
        let principal = principal_id.to_owned();
        let workspace = workspace_id.to_owned();
        let conversation = conversation_id.to_owned();
        let turn = turn_id.to_owned();
        self.run(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            let allowed: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
                    params![workspace, principal],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !allowed {
                return Err(StoreError::NotFound);
            }
            let now_format = time::format_description::parse_borrowed::<1>(
                "[year]-[month]-[day]T[hour]:[minute]:[second]Z",
            )
            .map_err(|_| StoreError::Invalid("timestamp format unavailable".to_owned()))?;
            let now = time::OffsetDateTime::now_utc()
                .format(&now_format)
                .map_err(|_| StoreError::Invalid("clock unavailable".to_owned()))?;
            resolve_conversation_session_host_in_transaction(
                &transaction,
                &workspace,
                &conversation,
                &turn,
                &now,
            )
        })
    }
}

pub(super) fn conversation_turn_pinned_binding_id(
    connection: &Connection,
    workspace_id: &str,
    conversation_id: &str,
    turn_id: &str,
) -> Result<String, StoreError> {
    let user_message_id: String = connection
        .query_row(
            "SELECT t.user_message_id
               FROM conversation_turns t
               JOIN conversations c ON c.conversation_id=t.conversation_id
              WHERE c.workspace_id=?1 AND t.conversation_id=?2 AND t.turn_id=?3",
            params![workspace_id, conversation_id, turn_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;

    let (event_count, event_payload, event_revision, event_schema_version): (
        i64,
        Option<String>,
        Option<i64>,
        Option<i64>,
    ) = connection
        .query_row(
            "SELECT COUNT(*),MIN(payload_json),MIN(entity_revision),MIN(schema_version)
               FROM domain_events
              WHERE workspace_id=?1 AND entity_type='ConversationTurn'
                AND entity_id=?2 AND type='conversation.turn.created.v2'",
            params![workspace_id, turn_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(map_database_error)?;
    if event_count != 1 || event_revision != Some(1) || event_schema_version != Some(1) {
        return Err(StoreError::Integrity(
            "Conversation turn creation event is missing, ambiguous, or has an invalid revision"
                .to_owned(),
        ));
    }
    let event_payload = event_payload.ok_or_else(|| {
        StoreError::Integrity("Conversation turn creation event is missing".to_owned())
    })?;
    let event: Value = serde_json::from_str(&event_payload).map_err(|_| {
        StoreError::Integrity("Conversation turn creation event is invalid".to_owned())
    })?;
    if !payload_has_exact_keys(
        &event,
        &[
            "turn_id",
            "conversation_id",
            "user_message_id",
            "agent_binding_id",
            "aggregate_version",
            "presentation_preference",
        ],
        &[],
    ) || event.get("turn_id").and_then(Value::as_str) != Some(turn_id)
        || event.get("conversation_id").and_then(Value::as_str) != Some(conversation_id)
        || event.get("user_message_id").and_then(Value::as_str) != Some(user_message_id.as_str())
        || event.get("aggregate_version").and_then(Value::as_u64) != Some(1)
        || !matches!(
            event.get("presentation_preference").and_then(Value::as_str),
            Some("AUTO" | "SIMPLE" | "RICH")
        )
    {
        return Err(StoreError::Integrity(
            "Conversation turn creation event identity does not match its record".to_owned(),
        ));
    }
    event
        .get("agent_binding_id")
        .and_then(Value::as_str)
        .filter(|binding| !binding.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            StoreError::Integrity("Conversation turn creation event has no AgentBinding".to_owned())
        })
}

pub(super) fn resolve_conversation_session_host_in_transaction(
    connection: &Connection,
    workspace_id: &str,
    conversation_id: &str,
    turn_id: &str,
    now: &str,
) -> Result<Option<ResolvedConversationHost>, StoreError> {
    let user_message_id: String = connection
        .query_row(
            "SELECT t.user_message_id
               FROM conversation_turns t
               JOIN conversations c ON c.conversation_id=t.conversation_id
              WHERE c.workspace_id=?1 AND t.conversation_id=?2 AND t.turn_id=?3",
            params![workspace_id, conversation_id, turn_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;

    let (event_count, event_payload, event_revision, event_schema_version): (
        i64,
        Option<String>,
        Option<i64>,
        Option<i64>,
    ) = connection
        .query_row(
            "SELECT COUNT(*),MIN(payload_json),MIN(entity_revision),MIN(schema_version)
               FROM domain_events
              WHERE workspace_id=?1 AND entity_type='ConversationTurn'
                AND entity_id=?2 AND type='conversation.turn.created.v2'",
            params![workspace_id, turn_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(map_database_error)?;
    if event_count != 1 || event_revision != Some(1) || event_schema_version != Some(1) {
        return Err(StoreError::Integrity(
            "Conversation turn creation event is missing, ambiguous, or has an invalid revision"
                .to_owned(),
        ));
    }
    let event_payload = event_payload.ok_or_else(|| {
        StoreError::Integrity("Conversation turn creation event is missing".to_owned())
    })?;
    let event: Value = serde_json::from_str(&event_payload).map_err(|_| {
        StoreError::Integrity("Conversation turn creation event is invalid".to_owned())
    })?;
    if !payload_has_exact_keys(
        &event,
        &[
            "turn_id",
            "conversation_id",
            "user_message_id",
            "agent_binding_id",
            "aggregate_version",
            "presentation_preference",
        ],
        &[],
    ) || event.get("turn_id").and_then(Value::as_str) != Some(turn_id)
        || event.get("conversation_id").and_then(Value::as_str) != Some(conversation_id)
        || event.get("user_message_id").and_then(Value::as_str) != Some(user_message_id.as_str())
        || event.get("aggregate_version").and_then(Value::as_u64) != Some(1)
        || !matches!(
            event.get("presentation_preference").and_then(Value::as_str),
            Some("AUTO" | "SIMPLE" | "RICH")
        )
    {
        return Err(StoreError::Integrity(
            "Conversation turn creation event identity does not match its record".to_owned(),
        ));
    }
    let binding = event
        .get("agent_binding_id")
        .and_then(Value::as_str)
        .filter(|binding| !binding.trim().is_empty())
        .ok_or_else(|| {
            StoreError::Integrity("Conversation turn creation event has no AgentBinding".to_owned())
        })?;

    let resolved: Option<(String, String, String, String)> = connection
        .query_row(
            "SELECT b.agent_binding_id,e.endpoint_id,eb.runtime_id,eb.runtime_incarnation_id
               FROM agent_bindings b
               JOIN agent_endpoints e ON e.agent_profile_id=b.agent_profile_id
               JOIN agent_endpoint_bindings eb ON eb.endpoint_id=e.endpoint_id
               JOIN runtimes r ON r.runtime_id=eb.runtime_id
               JOIN runtime_workspace_bindings rwb
                 ON rwb.runtime_id=r.runtime_id AND rwb.workspace_id=?1
                AND rwb.status='ACTIVE' AND rwb.revoked_at IS NULL
               JOIN runtime_incarnations ri
                 ON ri.runtime_id=r.runtime_id
                AND ri.runtime_incarnation_id=eb.runtime_incarnation_id
               JOIN runtime_offers ro
                 ON ro.runtime_id=eb.runtime_id
                AND ro.runtime_incarnation_id=eb.runtime_incarnation_id
                AND ro.offer_kind='AGENT_ENDPOINT' AND ro.offer_ref=e.endpoint_id
                AND ro.compatible=1 AND ro.readiness IN ('AVAILABLE','STARTABLE','READY')
                AND ro.expires_at>?3
              WHERE b.workspace_id=?1 AND b.agent_binding_id=?2
                AND b.enabled=1 AND b.lead_eligible=1
                AND (b.runtime_id IS NULL OR b.runtime_id=eb.runtime_id)
                AND (COALESCE(json_extract(b.endpoint_selection_policy_json,'$.mode'),'AUTO_COMPATIBLE') <> 'PINNED_ENDPOINT'
                     OR json_extract(b.endpoint_selection_policy_json,'$.endpoint_id')=e.endpoint_id)
                AND (eb.expires_at IS NULL OR eb.expires_at>?3)
                AND EXISTS(SELECT 1 FROM json_each(rwb.roles_json) role WHERE role.value='EXECUTOR')
                AND r.availability='ONLINE'
                AND r.current_incarnation_id=eb.runtime_incarnation_id
                AND ri.recovery_state='READY'
              ORDER BY eb.observed_at DESC,e.endpoint_id,eb.runtime_id,eb.runtime_incarnation_id
              LIMIT 1",
            params![workspace_id, binding, now],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((agent_binding_id, endpoint_id, runtime_id, runtime_incarnation_id)) = resolved else {
        return Ok(None);
    };
    Ok(Some(ResolvedConversationHost {
        agent_binding_id,
        endpoint_id,
        runtime_id,
        runtime_incarnation_id,
    }))
}

fn validate_turn_create(commit: &CreateConversationTurnCommit) -> Result<(), StoreError> {
    validate_turn_create_records(commit)?;
    let expected_request = serde_json::json!({
        "operation":"conversation.turn.create.v1", "workspace_id":commit.workspace_id,
        "conversation_id":commit.turn.conversation_id, "turn_id":commit.turn.turn_id,
        "message_id":commit.message.message_id, "author":commit.message.author,
        "content":commit.message.content, "resource_refs":commit.message.resource_refs,
        "presentation_preference":serde_json::to_value(commit.turn.presentation_preference).map_err(|error|StoreError::Invalid(error.to_string()))?
    });
    if commit.request.request_payload != expected_request {
        return Err(StoreError::Invalid(
            "ConversationTurn request payload does not match the committed content".to_owned(),
        ));
    }
    Ok(())
}

/// Validates the immutable message, OPEN turn, and event records shared by ordinary
/// turn append and atomic first-send. Each operation validates its own request receipt
/// separately because the idempotent request payloads intentionally differ.
fn validate_turn_create_records(commit: &CreateConversationTurnCommit) -> Result<(), StoreError> {
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
    Ok(())
}

fn validate_turn_transition_event(
    event: &EventDraft,
    current: &ConversationTurn,
    next: &ConversationTurn,
    command: &TurnCommand,
) -> Result<(), StoreError> {
    let expected_type = match command {
        TurnCommand::Retry => "conversation.turn.retried.v1",
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
        TurnCommand::Retry => &[
            "turn_id",
            "prior_agent_session_id",
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
        TurnCommand::Retry => &["turn_id", "retry_ordinal", "aggregate_version"],
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
                TurnCommand::Retry => {
                    event.payload.get("turn_id").and_then(Value::as_str)
                        == Some(current.turn_id.as_str())
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

pub(super) fn load_turn(
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

impl ConversationHistoryStore for SqliteConversationStore {
    fn rename_conversation(
        &self,
        commit: RenameConversation,
    ) -> Result<CommittedConversation, StoreError> {
        let title = commit.title.clone();
        let payload = commit.request.request_payload.clone();
        if title.trim().is_empty()
            || title.chars().count() > 160
            || title.chars().any(char::is_control)
            || payload.get("operation").and_then(Value::as_str) != Some("conversation.rename.v1")
            || payload.get("title").and_then(Value::as_str) != Some(title.as_str())
            || commit.event.event_type != "conversation.renamed.v1"
        {
            return Err(StoreError::Invalid(
                "Conversation rename is inconsistent".to_owned(),
            ));
        }
        let title_digest = digest(title.as_bytes());
        history_mutation(
            self,
            &commit.request,
            &commit.principal_id,
            &commit.workspace_id,
            &commit.conversation_id,
            commit.expected_version,
            commit.event,
            HistoryMutation::Rename {
                title,
                title_digest,
            },
        )
    }

    fn set_conversation_archived(
        &self,
        commit: SetConversationArchived,
    ) -> Result<CommittedConversation, StoreError> {
        let expected_event = if commit.archived {
            "conversation.archived.v1"
        } else {
            "conversation.unarchived.v1"
        };
        let expected_operation = if commit.archived {
            "conversation.archive.v1"
        } else {
            "conversation.unarchive.v1"
        };
        if commit
            .request
            .request_payload
            .get("operation")
            .and_then(Value::as_str)
            != Some(expected_operation)
            || commit.event.event_type != expected_event
        {
            return Err(StoreError::Invalid(
                "Conversation archive mutation is inconsistent".to_owned(),
            ));
        }
        history_mutation(
            self,
            &commit.request,
            &commit.principal_id,
            &commit.workspace_id,
            &commit.conversation_id,
            commit.expected_version,
            commit.event,
            HistoryMutation::Archive(commit.archived),
        )
    }

    fn get_conversation_status(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationStatus>, StoreError> {
        if principal_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || conversation_id.trim().is_empty()
        {
            return Err(StoreError::Invalid(
                "Conversation lookup is invalid".to_owned(),
            ));
        }
        let (principal, workspace, id) = (
            principal_id.to_owned(),
            workspace_id.to_owned(),
            conversation_id.to_owned(),
        );
        self.run(move |connection| {
            let allowed: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2)",
                    params![workspace, principal],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !allowed {
                return Err(StoreError::NotFound);
            }
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM conversations WHERE workspace_id=?1 AND conversation_id=?2)",
                    params![workspace, id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !exists {
                return Ok(None);
            }
            Ok(Some(read_conversation_status(connection, &id)?))
        })
    }

    fn list_conversations_by_status(
        &self,
        principal_id: &str,
        workspace_id: &str,
        status: ConversationStatus,
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
        let status = status.as_str();
        self.run(move |connection| {
            let allowed: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2)",
                    params![workspace, principal],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !allowed {
                return Err(StoreError::NotFound);
            }
            let mut statement = connection
                .prepare(
                    "SELECT c.conversation_id,c.workspace_id,c.title,c.active_agent_binding_id,c.version,c.created_at \
                     FROM conversations c LEFT JOIN conversation_history_state h ON h.conversation_id=c.conversation_id \
                     WHERE c.workspace_id=?1 AND COALESCE(h.status,'ACTIVE')=?5 \
                     AND (?2 IS NULL OR (c.created_at,c.conversation_id)<(?2,?3)) \
                     ORDER BY c.created_at DESC,c.conversation_id DESC LIMIT ?4",
                )
                .map_err(map_database_error)?;
            statement
                .query_map(params![workspace, after_at, after_id, limit as i64, status], |row| {
                    Ok(ConversationRecord {
                        conversation_id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        title: row.get(2)?,
                        active_agent_binding_id: row.get(3)?,
                        version: row.get::<_, i64>(4)? as u64,
                        created_at: row.get(5)?,
                    })
                })
                .map_err(map_database_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_database_error)
        })
    }
}

enum HistoryMutation {
    Rename { title: String, title_digest: String },
    Archive(bool),
}

fn read_conversation_status(
    connection: &Connection,
    conversation_id: &str,
) -> Result<ConversationStatus, StoreError> {
    let status: Option<String> = connection
        .query_row(
            "SELECT status FROM conversation_history_state WHERE conversation_id=?1",
            [conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    match status.as_deref() {
        None | Some("ACTIVE") => Ok(ConversationStatus::Active),
        Some("ARCHIVED") => Ok(ConversationStatus::Archived),
        Some("DELETION_PENDING") => Ok(ConversationStatus::DeletionPending),
        Some("DELETED") => Ok(ConversationStatus::Deleted),
        Some(_) => Err(StoreError::Integrity(
            "Conversation history status is invalid".to_owned(),
        )),
    }
}

/// Rejects a write against a Conversation that is not ACTIVE. Called inside the
/// caller's own IMMEDIATE transaction so the check cannot race an archive.
fn require_conversation_active(
    connection: &Connection,
    conversation_id: &str,
) -> Result<(), StoreError> {
    match read_conversation_status(connection, conversation_id)? {
        ConversationStatus::Active => Ok(()),
        ConversationStatus::Archived => Err(StoreError::Invalid(CONVERSATION_ARCHIVED.to_owned())),
        ConversationStatus::DeletionPending | ConversationStatus::Deleted => {
            Err(StoreError::Invalid(CONVERSATION_DELETED.to_owned()))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn history_mutation(
    store: &SqliteConversationStore,
    request: &WorkspaceCreateRequest,
    principal_id: &str,
    workspace_id: &str,
    conversation_id: &str,
    expected_version: u64,
    event: EventDraft,
    mutation: HistoryMutation,
) -> Result<CommittedConversation, StoreError> {
    let next_version = expected_version
        .checked_add(1)
        .filter(|version| *version <= i64::MAX as u64)
        .ok_or_else(|| StoreError::Invalid("Conversation version exhausted".to_owned()))?;
    let payload = &request.request_payload;
    if principal_id.trim().is_empty()
        || request.principal_id != principal_id
        || !super::valid_shared_idempotency_key(&request.request_id)
        || workspace_id.trim().is_empty()
        || conversation_id.trim().is_empty()
        || expected_version == 0
        || payload.get("workspace_id").and_then(Value::as_str) != Some(workspace_id)
        || payload.get("conversation_id").and_then(Value::as_str) != Some(conversation_id)
        || payload.get("expected_version").and_then(Value::as_u64) != Some(expected_version)
        || event.workspace_id != workspace_id
        || event.entity_type != "Conversation"
        || event.entity_id != conversation_id
        || event.entity_revision != next_version
        || event.schema_version != 1
    {
        return Err(StoreError::Invalid(
            "Conversation history mutation is inconsistent".to_owned(),
        ));
    }
    let request_digest = digest(&canonical_json(payload)?);
    let blobs = Arc::clone(&store.store.inner.blobs);
    let principal_id = principal_id.to_owned();
    let workspace_id = workspace_id.to_owned();
    let conversation_id = conversation_id.to_owned();
    let request_id = request.request_id.clone();
    store.run(move |connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_database_error)?;
        if let Some(replay) = verify_request_receipt::<CommittedConversation>(
            &tx,
            &principal_id,
            &request_id,
            &request_digest,
        )? {
            tx.commit().map_err(map_database_error)?;
            return Ok(replay);
        }
        let workspace_status: Option<String> = tx
            .query_row(
                "SELECT status FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2",
                params![workspace_id, principal_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_database_error)?;
        match workspace_status.as_deref() {
            None => return Err(StoreError::NotFound),
            Some("ACTIVE") => {}
            Some(_) => return Err(StoreError::Invalid("WORKSPACE_ARCHIVED".to_owned())),
        }
        let current: Option<(Option<String>, i64, String, Option<String>)> = tx
            .query_row(
                "SELECT title,version,created_at,active_agent_binding_id FROM conversations WHERE workspace_id=?1 AND conversation_id=?2",
                params![workspace_id, conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(map_database_error)?;
        let Some((title, actual_version, created_at, binding)) = current else {
            return Err(StoreError::NotFound);
        };
        let actual_version = u64::try_from(actual_version)
            .map_err(|_| StoreError::Integrity("Conversation version is invalid".to_owned()))?;
        if actual_version != expected_version {
            return Err(StoreError::Conflict {
                expected: Some(expected_version),
                actual: Some(actual_version),
            });
        }
        let status = read_conversation_status(&tx, &conversation_id)?;
        let mut event_draft = event;
        let mut new_title = title.clone();
        match &mutation {
            HistoryMutation::Rename { title, title_digest } => {
                require_conversation_active(&tx, &conversation_id)?;
                new_title = Some(title.clone());
                event_draft.payload = serde_json::json!({
                    "workspace_id": workspace_id,
                    "conversation_id": conversation_id,
                    "title_digest": title_digest,
                    "changed_by": {"kind":"USER","principal_id":principal_id},
                    "aggregate_version": next_version,
                });
            }
            HistoryMutation::Archive(true) => {
                require_conversation_active(&tx, &conversation_id)?;
                let in_flight: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id=?1 AND status IN ('OPEN','RUNNING','WAITING_USER','WAITING_DEPENDENCY','CANCEL_REQUESTED'))",
                        params![conversation_id],
                        |row| row.get(0),
                    )
                    .map_err(map_database_error)?;
                if in_flight {
                    return Err(StoreError::Invalid(CONVERSATION_NOT_QUIESCENT.to_owned()));
                }
                event_draft.payload = serde_json::json!({
                    "workspace_id": workspace_id,
                    "conversation_id": conversation_id,
                    "changed_by": {"kind":"USER","principal_id":principal_id},
                    "aggregate_version": next_version,
                });
            }
            HistoryMutation::Archive(false) => {
                match status {
                    ConversationStatus::Archived => {}
                    ConversationStatus::Active => {
                        return Err(StoreError::Conflict {
                            expected: Some(expected_version),
                            actual: Some(actual_version),
                        });
                    }
                    _ => return Err(StoreError::Invalid(CONVERSATION_DELETED.to_owned())),
                }
                event_draft.payload = serde_json::json!({
                    "workspace_id": workspace_id,
                    "conversation_id": conversation_id,
                    "changed_by": {"kind":"USER","principal_id":principal_id},
                    "aggregate_version": next_version,
                });
            }
        }
        let conversation = ConversationRecord {
            conversation_id: conversation_id.clone(),
            workspace_id: workspace_id.clone(),
            title: new_title,
            active_agent_binding_id: binding,
            version: next_version,
            created_at,
        };
        validate_conversation_record(&conversation)?;
        let changed = tx
            .execute(
                "UPDATE conversations SET title=?1,version=?2 WHERE workspace_id=?3 AND conversation_id=?4 AND version=?5",
                params![conversation.title, next_version as i64, workspace_id, conversation_id, expected_version as i64],
            )
            .map_err(map_database_error)?;
        if changed != 1 {
            return Err(StoreError::Conflict {
                expected: Some(expected_version),
                actual: None,
            });
        }
        if let HistoryMutation::Archive(archived) = &mutation {
            let at = event_draft.recorded_at.clone();
            if *archived {
                tx.execute(
                    "INSERT INTO conversation_history_state(conversation_id,workspace_id,status,archived_at,deleted_at,updated_at) VALUES(?1,?2,'ARCHIVED',?3,NULL,?3) ON CONFLICT(conversation_id) DO UPDATE SET status='ARCHIVED',archived_at=?3,updated_at=?3",
                    params![conversation_id, workspace_id, at],
                )
            } else {
                tx.execute(
                    "UPDATE conversation_history_state SET status='ACTIVE',archived_at=NULL,updated_at=?2 WHERE conversation_id=?1",
                    params![conversation_id, at],
                )
            }
            .map_err(map_database_error)?;
        }
        let state_bytes = canonical_json(&conversation)?;
        let state_blob = blobs.put(
            &workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            super::STATE_MEDIA_TYPE,
        )?;
        if state_blob.size_bytes != state_bytes.len() as u64
            || blobs.get(&workspace_id, BlobPurpose::AggregateState, &state_blob)? != state_bytes
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
        let event = insert_domain_event(&tx, &event_draft, &state_ref)?;
        let accepted = CommittedConversation { conversation, event };
        let response_json = String::from_utf8(canonical_json(&accepted)?)
            .map_err(|error| StoreError::Invalid(error.to_string()))?;
        tx.execute(
            "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,NULL)",
            params![
                principal_id,
                request_id,
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

impl ConversationCoeditStore for SqliteConversationStore {
    fn append_owner_side_note(
        &self,
        commit: AppendOwnerSideNote,
    ) -> Result<OwnerSideNoteRecord, StoreError> {
        let valid = |value: &str| {
            !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
        };
        let payload = &commit.request.request_payload;
        if !valid(&commit.principal_id)
            || commit.request.principal_id != commit.principal_id
            || !super::valid_shared_idempotency_key(&commit.request.request_id)
            || !valid(&commit.workspace_id)
            || !valid(&commit.conversation_id)
            || !valid(&commit.correlated_turn_id)
            || !valid(&commit.side_note_id)
            || payload.get("operation").and_then(Value::as_str)
                != Some("conversation.side_note.append.v1")
            || payload.get("workspace_id").and_then(Value::as_str)
                != Some(commit.workspace_id.as_str())
            || payload.get("conversation_id").and_then(Value::as_str)
                != Some(commit.conversation_id.as_str())
            || payload.get("correlated_turn_id").and_then(Value::as_str)
                != Some(commit.correlated_turn_id.as_str())
            || payload.get("body").and_then(Value::as_str) != Some(commit.body.as_str())
        {
            return Err(StoreError::Invalid(
                "owner side note request is inconsistent".to_owned(),
            ));
        }
        if commit.body.trim().is_empty() || commit.body.chars().count() > OWNER_SIDE_NOTE_MAX_CHARS
        {
            return Err(StoreError::Invalid(SIDE_NOTE_OVER_BUDGET.to_owned()));
        }
        let created_at = canonicalize_utc_timestamp(&commit.created_at)?;
        let request_digest = digest(&canonical_json(payload)?);
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            if let Some(replay) = verify_request_receipt::<OwnerSideNoteRecord>(
                &tx,
                &commit.principal_id,
                &commit.request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let allowed: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
                    params![commit.workspace_id, commit.principal_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !allowed {
                return Err(StoreError::NotFound);
            }
            let conversation_exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM conversations WHERE workspace_id=?1 AND conversation_id=?2)",
                    params![commit.workspace_id, commit.conversation_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !conversation_exists {
                return Err(StoreError::NotFound);
            }
            require_conversation_active(&tx, &commit.conversation_id)?;
            let turn = load_turn(
                &tx,
                &commit.workspace_id,
                &commit.conversation_id,
                &commit.correlated_turn_id,
            )?
            .ok_or(StoreError::NotFound)?;
            if turn.status.is_terminal() {
                return Err(StoreError::Invalid(SIDE_NOTE_TURN_TERMINAL.to_owned()));
            }
            tx.execute(
                "INSERT INTO conversation_owner_side_notes(side_note_id,workspace_id,conversation_id,correlated_turn_id,body,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    commit.side_note_id,
                    commit.workspace_id,
                    commit.conversation_id,
                    commit.correlated_turn_id,
                    commit.body,
                    created_at,
                ],
            )
            .map_err(map_database_error)?;
            let record = OwnerSideNoteRecord {
                side_note_id: commit.side_note_id,
                workspace_id: commit.workspace_id,
                conversation_id: commit.conversation_id,
                correlated_turn_id: commit.correlated_turn_id,
                body: commit.body,
                created_at: created_at.clone(),
            };
            let response_json = String::from_utf8(canonical_json(&record)?)
                .map_err(|error| StoreError::Invalid(error.to_string()))?;
            tx.execute(
                "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,NULL)",
                params![
                    commit.principal_id,
                    commit.request.request_id,
                    request_digest,
                    response_json,
                    digest(response_json.as_bytes()),
                    created_at,
                ],
            )
            .map_err(map_database_error)?;
            tx.commit().map_err(map_database_error)?;
            Ok(record)
        })
    }

    fn list_owner_side_notes(
        &self,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        correlated_turn_id: &str,
    ) -> Result<Vec<OwnerSideNoteRecord>, StoreError> {
        if principal_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || conversation_id.trim().is_empty()
            || correlated_turn_id.trim().is_empty()
        {
            return Err(StoreError::Invalid(
                "owner side note lookup is invalid".to_owned(),
            ));
        }
        let (principal, workspace, conversation, turn) = (
            principal_id.to_owned(),
            workspace_id.to_owned(),
            conversation_id.to_owned(),
            correlated_turn_id.to_owned(),
        );
        self.run(move |connection| {
            let allowed: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2)",
                    params![workspace, principal],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if !allowed {
                return Err(StoreError::NotFound);
            }
            let mut statement = connection
                .prepare(
                    "SELECT side_note_id,workspace_id,conversation_id,correlated_turn_id,body,created_at FROM conversation_owner_side_notes WHERE workspace_id=?1 AND conversation_id=?2 AND correlated_turn_id=?3 ORDER BY created_at,side_note_id",
                )
                .map_err(map_database_error)?;
            statement
                .query_map(params![workspace, conversation, turn], |row| {
                    Ok(OwnerSideNoteRecord {
                        side_note_id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        conversation_id: row.get(2)?,
                        correlated_turn_id: row.get(3)?,
                        body: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                })
                .map_err(map_database_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_database_error)
        })
    }
}
