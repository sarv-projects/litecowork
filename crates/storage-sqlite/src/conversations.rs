//! Durable Conversation catalog reads and owner-created Conversation aggregates.
use super::*;
use storage_core::conversation::{validate_conversation_record, *};

pub(super) type WriterOperation = Box<dyn FnOnce(&mut Connection) + Send>;

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
            Command::ConversationOperation {
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
            || commit.request.request_id.trim().is_empty()
            || commit.request.request_id.len() > 128
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
        let blob = self.store.inner.blobs.put(
            &commit.conversation.workspace_id,
            BlobPurpose::AggregateState,
            &state_bytes,
            super::STATE_MEDIA_TYPE,
        )?;
        if blob.size_bytes != state_bytes.len() as u64
            || self.store.inner.blobs.get(
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
        let request_digest = digest(&canonical_json(&commit.request.request_payload)?);
        let workspace_id = commit.conversation.workspace_id.clone();
        self.run(move |connection| {
            let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_database_error)?;
            if let Some(value) = verify_request_receipt::<Value>(&transaction, &commit.request.principal_id, &commit.request.request_id, &request_digest)? {
                let replay: CommittedConversation = serde_json::from_value(value).map_err(|_| StoreError::Integrity("Conversation idempotency receipt is invalid".to_owned()))?;
                transaction.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let allowed: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
                params![workspace_id, commit.request.principal_id], |row| row.get(0),
            ).map_err(map_database_error)?;
            if !allowed { return Err(StoreError::NotFound); }
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
