use super::*;
use storage_core::{
    AgentRegistryCacheSnapshot, AgentRegistryCacheStore, validate_agent_registry_cache_snapshot,
};

impl AgentRegistryCacheStore for SqliteWorkspaceStore {
    fn read_agent_registry_snapshot(
        &self,
        source_registry: &str,
    ) -> Result<Option<AgentRegistryCacheSnapshot>, StoreError> {
        validate_registry_source(source_registry)?;
        let (reply, receiver) = mpsc::channel();
        self.execute_command(
            Command::ReadAgentRegistrySnapshot {
                source_registry: source_registry.to_owned(),
                reply,
            },
            receiver,
        )
    }

    fn replace_agent_registry_snapshot(
        &self,
        snapshot: AgentRegistryCacheSnapshot,
    ) -> Result<(), StoreError> {
        validate_agent_registry_cache_snapshot(&snapshot).map_err(StoreError::Invalid)?;
        let (reply, receiver) = mpsc::channel();
        self.execute_command(
            Command::ReplaceAgentRegistrySnapshot { snapshot, reply },
            receiver,
        )
    }
}

fn validate_registry_source(source: &str) -> Result<(), StoreError> {
    if source.is_empty() || source.len() > 256 || source.chars().any(char::is_control) {
        return Err(StoreError::Invalid(
            "Agent Registry source is invalid".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn read_snapshot(
    connection: &Connection,
    source_registry: &str,
) -> Result<Option<AgentRegistryCacheSnapshot>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT registry_agent_id, source_revision, metadata_json, observed_at, expires_at
             FROM agent_registry_cache WHERE source_registry = ?1
             ORDER BY registry_agent_id",
        )
        .map_err(map_database_error)?;
    let mut rows = statement
        .query([source_registry])
        .map_err(map_database_error)?;
    let mut snapshot: Option<AgentRegistryCacheSnapshot> = None;
    while let Some(row) = rows.next().map_err(map_database_error)? {
        let registry_agent_id: String = row.get(0).map_err(map_database_error)?;
        let source_revision: Option<String> = row.get(1).map_err(map_database_error)?;
        let metadata_json: String = row.get(2).map_err(map_database_error)?;
        let observed_at: String = row.get(3).map_err(map_database_error)?;
        let expires_at: Option<String> = row.get(4).map_err(map_database_error)?;
        let metadata: Value = serde_json::from_str(&metadata_json).map_err(|_| {
            StoreError::CorruptSchema("Agent Registry cache metadata is invalid JSON".to_owned())
        })?;
        let current = snapshot.get_or_insert_with(|| AgentRegistryCacheSnapshot {
            source_registry: source_registry.to_owned(),
            source_revision: source_revision.clone(),
            observed_at: observed_at.clone(),
            expires_at: expires_at.clone(),
            entries: Vec::new(),
        });
        if current.source_revision != source_revision
            || current.observed_at != observed_at
            || current.expires_at != expires_at
        {
            return Err(StoreError::CorruptSchema(
                "Agent Registry cache rows do not belong to one atomic snapshot".to_owned(),
            ));
        }
        current.entries.push(storage_core::AgentRegistryCacheEntry {
            registry_agent_id,
            metadata,
        });
    }
    if let Some(snapshot) = &snapshot {
        validate_agent_registry_cache_snapshot(snapshot).map_err(|message| {
            StoreError::CorruptSchema(format!("Agent Registry cache is invalid: {message}"))
        })?;
    }
    Ok(snapshot)
}

pub(super) fn replace_snapshot(
    connection: &mut Connection,
    snapshot: AgentRegistryCacheSnapshot,
) -> Result<(), StoreError> {
    validate_agent_registry_cache_snapshot(&snapshot).map_err(StoreError::Invalid)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    transaction
        .execute(
            "DELETE FROM agent_registry_cache WHERE source_registry = ?1",
            [&snapshot.source_registry],
        )
        .map_err(map_database_error)?;
    for entry in snapshot.entries {
        let metadata_json = String::from_utf8(canonical_json(&entry.metadata)?)
            .map_err(|_| StoreError::Invalid("registry metadata is not UTF-8".to_owned()))?;
        transaction
            .execute(
                "INSERT INTO agent_registry_cache(
                   registry_agent_id, source_registry, source_revision, metadata_json,
                   observed_at, expires_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    entry.registry_agent_id,
                    snapshot.source_registry,
                    snapshot.source_revision,
                    metadata_json,
                    snapshot.observed_at,
                    snapshot.expires_at,
                ],
            )
            .map_err(map_database_error)?;
    }
    transaction.commit().map_err(map_database_error)
}
