use serde_json::json;
use storage_core::{
    CommittedWorkspace, EventDraft, ReplicationPolicy, StoreError, Workspace, WorkspaceStore,
};

#[cfg(test)]
const RECORD_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct EventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

#[derive(Clone, Debug)]
pub struct CreateWorkspace {
    pub workspace_id: String,
    pub name: String,
    pub owner_principal_id: String,
    pub event: EventContext,
}

#[derive(Clone, Debug)]
pub struct ChangeReplicationPolicy {
    pub workspace_id: String,
    pub expected_version: u64,
    pub policy: ReplicationPolicy,
    pub replication_scope_root_ids: Vec<String>,
    pub event: EventContext,
}

pub struct WorkspaceService<S> {
    store: S,
}

impl<S: WorkspaceStore> WorkspaceService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn create(&self, command: CreateWorkspace) -> Result<CommittedWorkspace, StoreError> {
        require_non_empty("workspace_id", &command.workspace_id)?;
        require_non_empty("workspace name", command.name.trim())?;
        require_non_empty("owner_principal_id", &command.owner_principal_id)?;
        validate_event_context(&command.event)?;

        let workspace = Workspace {
            workspace_id: command.workspace_id.clone(),
            name: command.name.trim().to_owned(),
            owner_principal_id: command.owner_principal_id.clone(),
            replication_policy: ReplicationPolicy::LocalOnly,
            replication_scope_root_ids: Vec::new(),
            current_instruction_revision: None,
            default_agent_binding_id: None,
            primary_coworker_id: None,
            hub_runtime_id: None,
            status: "ACTIVE".to_owned(),
            created_at: command.event.recorded_at.clone(),
            updated_at: command.event.recorded_at.clone(),
            version: 1,
        };

        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: workspace.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "workspace.created.v1".to_owned(),
            payload: json!({
                "workspace_id": workspace.workspace_id,
                "owner_principal_id": workspace.owner_principal_id,
                "replication_policy": workspace.replication_policy.as_str(),
                "replication_scope_root_ids": workspace.replication_scope_root_ids,
            }),
            recorded_at: command.event.recorded_at,
        };

        self.store.commit_workspace(None, workspace, event)
    }

    pub fn change_replication_policy(
        &self,
        command: ChangeReplicationPolicy,
    ) -> Result<CommittedWorkspace, StoreError> {
        require_non_empty("workspace_id", &command.workspace_id)?;
        validate_event_context(&command.event)?;

        let mut workspace = self
            .store
            .get_workspace(&command.workspace_id)?
            .ok_or(StoreError::NotFound)?;

        if workspace.version != command.expected_version {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_version),
                actual: Some(workspace.version),
            });
        }
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid(
                "an archived Workspace is read-only".to_owned(),
            ));
        }
        if command.policy == ReplicationPolicy::SelectedFolders
            && command.replication_scope_root_ids.is_empty()
        {
            return Err(StoreError::Invalid(
                "SELECTED_FOLDERS requires at least one active WorkspaceRoot".to_owned(),
            ));
        }
        if command.policy != ReplicationPolicy::SelectedFolders
            && !command.replication_scope_root_ids.is_empty()
        {
            return Err(StoreError::Invalid(
                "replication roots are valid only with SELECTED_FOLDERS".to_owned(),
            ));
        }

        let previous_policy = workspace.replication_policy.clone();
        workspace.replication_policy = command.policy;
        workspace.replication_scope_root_ids = command.replication_scope_root_ids;
        workspace.version += 1;
        workspace.updated_at = command.event.recorded_at.clone();

        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: workspace.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "workspace.replication_policy.changed.v1".to_owned(),
            payload: json!({
                "workspace_id": workspace.workspace_id,
                "from": previous_policy.as_str(),
                "to": workspace.replication_policy.as_str(),
                "replication_scope_root_ids": workspace.replication_scope_root_ids,
                "aggregate_version": workspace.version,
            }),
            recorded_at: command.event.recorded_at,
        };

        self.store
            .commit_workspace(Some(command.expected_version), workspace, event)
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<(), StoreError> {
    if value.trim().is_empty() {
        return Err(StoreError::Invalid(format!("{field} must not be empty")));
    }
    Ok(())
}

fn validate_event_context(context: &EventContext) -> Result<(), StoreError> {
    require_non_empty("event_id", &context.event_id)?;
    require_non_empty("origin_runtime_id", &context.origin_runtime_id)?;
    require_non_empty("hlc_timestamp", &context.hlc_timestamp)?;
    require_non_empty("correlation_id", &context.correlation_id)?;
    require_non_empty("recorded_at", &context.recorded_at)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use storage_core::{BlobRef, DomainEvent, EventStore, StateStore};

    #[derive(Clone, Default)]
    struct MemoryStore(Arc<Mutex<Vec<CommittedWorkspace>>>);

    impl StateStore for MemoryStore {
        fn get_workspace(&self, workspace_id: &str) -> Result<Option<Workspace>, StoreError> {
            Ok(self
                .0
                .lock()
                .expect("lock")
                .last()
                .filter(|row| row.workspace.workspace_id == workspace_id)
                .map(|row| row.workspace.clone()))
        }

        fn commit_workspace(
            &self,
            expected_version: Option<u64>,
            workspace: Workspace,
            draft: EventDraft,
        ) -> Result<CommittedWorkspace, StoreError> {
            let mut rows = self.0.lock().expect("lock");
            let current = rows.last().map(|row| row.workspace.version);
            if current != expected_version {
                return Err(StoreError::Conflict {
                    expected: expected_version,
                    actual: current,
                });
            }
            let event = DomainEvent {
                event_id: draft.event_id,
                workspace_id: draft.workspace_id,
                entity_type: draft.entity_type,
                entity_id: draft.entity_id,
                origin_runtime_id: draft.origin_runtime_id,
                origin_sequence: current.unwrap_or(0) + 1,
                entity_revision: draft.entity_revision,
                hlc_timestamp: draft.hlc_timestamp,
                correlation_id: draft.correlation_id,
                causation_id: draft.causation_id,
                schema_version: draft.schema_version,
                event_type: draft.event_type,
                payload: draft.payload,
                aggregate_state_ref: storage_core::AggregateStateRef {
                    blob: BlobRef {
                        digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                        size_bytes: 1,
                        media_type: "application/json".to_owned(),
                    },
                    entity_revision: workspace.version,
                    record_schema_version: RECORD_SCHEMA_VERSION,
                },
                recorded_at: draft.recorded_at,
                payload_digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            };
            let result = CommittedWorkspace { workspace, event };
            rows.push(result.clone());
            Ok(result)
        }
    }

    impl EventStore for MemoryStore {
        fn read_workspace_events(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DomainEvent>, StoreError> {
            Ok(self
                .0
                .lock()
                .expect("lock")
                .iter()
                .map(|row| row.event.clone())
                .collect())
        }
    }

    fn event_context(id: &str, time: &str) -> EventContext {
        EventContext {
            event_id: id.to_owned(),
            origin_runtime_id: "runtime-1".to_owned(),
            hlc_timestamp: time.to_owned(),
            correlation_id: "correlation-1".to_owned(),
            causation_id: None,
            recorded_at: time.to_owned(),
        }
    }

    #[test]
    fn creates_local_workspace_with_empty_scope_and_created_event() {
        let service = WorkspaceService::new(MemoryStore::default());
        let created = service
            .create(CreateWorkspace {
                workspace_id: "workspace-1".to_owned(),
                name: "  My Workspace  ".to_owned(),
                owner_principal_id: "owner-1".to_owned(),
                event: event_context("event-1", "2026-10-06T10:00:00Z"),
            })
            .expect("workspace created");

        assert_eq!(created.workspace.name, "My Workspace");
        assert_eq!(
            created.workspace.replication_policy,
            ReplicationPolicy::LocalOnly
        );
        assert!(created.workspace.replication_scope_root_ids.is_empty());
        assert_eq!(created.workspace.version, 1);
        assert_eq!(created.event.event_type, "workspace.created.v1");
    }

    #[test]
    fn refuses_selected_folders_without_an_active_root() {
        let service = WorkspaceService::new(MemoryStore::default());
        service
            .create(CreateWorkspace {
                workspace_id: "workspace-1".to_owned(),
                name: "Workspace".to_owned(),
                owner_principal_id: "owner-1".to_owned(),
                event: event_context("event-1", "2026-10-06T10:00:00Z"),
            })
            .expect("workspace created");

        let result = service.change_replication_policy(ChangeReplicationPolicy {
            workspace_id: "workspace-1".to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::SelectedFolders,
            replication_scope_root_ids: Vec::new(),
            event: event_context("event-2", "2026-10-06T10:01:00Z"),
        });

        assert!(matches!(result, Err(StoreError::Invalid(_))));
    }
}
