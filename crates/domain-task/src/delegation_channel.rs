//! Internal parent-side result consumer for the DelegationChannel `result(id)` call.
//! The Runtime must construct the scope from its authenticated parent session; the
//! storage adapter independently verifies ownership and the exact parent/child pins.

use crate::{ChildResultReadContext, DelegationResultService, ResultEnvelope};
use storage_core::{DelegationResultStore, StoreError};

/// Read authority captured from the current authenticated parent AgentSession.
/// Construct this only inside the Runtime channel factory after resolving the active
/// parent session; SQLite still rechecks these bindings before returning child output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegationChannelParentScope {
    owner_principal_id: String,
    workspace_id: String,
    task_id: String,
    parent_attempt_id: String,
    task_spec_revision: u64,
}

impl DelegationChannelParentScope {
    pub fn new(
        owner_principal_id: impl Into<String>,
        workspace_id: impl Into<String>,
        task_id: impl Into<String>,
        parent_attempt_id: impl Into<String>,
        task_spec_revision: u64,
    ) -> Result<Self, StoreError> {
        let scope = Self {
            owner_principal_id: owner_principal_id.into(),
            workspace_id: workspace_id.into(),
            task_id: task_id.into(),
            parent_attempt_id: parent_attempt_id.into(),
            task_spec_revision,
        };
        if [
            scope.owner_principal_id.as_str(),
            scope.workspace_id.as_str(),
            scope.task_id.as_str(),
            scope.parent_attempt_id.as_str(),
        ]
        .iter()
        .any(|value| invalid_identifier(value))
            || task_spec_revision == 0
        {
            return Err(StoreError::Invalid(
                "DelegationChannel parent scope is invalid".to_owned(),
            ));
        }
        Ok(scope)
    }
}

/// Internal adapter for the `DelegationChannel.result(child_attempt_id)` operation.
/// Other channel operations remain with the Runtime's authenticated event/control
/// transports; this reader exposes no dispatch or mutation method.
pub struct DelegationChannelResultReader<S> {
    results: DelegationResultService<S>,
    parent: DelegationChannelParentScope,
}

impl<S: DelegationResultStore> DelegationChannelResultReader<S> {
    pub fn new(store: S, parent: DelegationChannelParentScope) -> Self {
        Self {
            results: DelegationResultService::new(store),
            parent,
        }
    }

    /// Return only a durable result bound by storage to this authenticated parent,
    /// Workspace, Task and pinned TaskSpec revision.
    pub fn result(&self, child_attempt_id: &str) -> Result<Option<ResultEnvelope>, StoreError> {
        if invalid_identifier(child_attempt_id) {
            return Err(StoreError::Invalid(
                "DelegationChannel child Attempt ID is invalid".to_owned(),
            ));
        }
        self.results.result(ChildResultReadContext {
            owner_principal_id: self.parent.owner_principal_id.clone(),
            workspace_id: self.parent.workspace_id.clone(),
            task_id: self.parent.task_id.clone(),
            parent_attempt_id: self.parent.parent_attempt_id.clone(),
            child_attempt_id: child_attempt_id.to_owned(),
            task_spec_revision: self.parent.task_spec_revision,
        })
    }
}

fn invalid_identifier(value: &str) -> bool {
    value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use storage_core::{
        CommittedDelegationResult, DelegationResultPublishCommit, DelegationResultReadQuery,
        DelegationResultStore, StoreError,
    };

    #[derive(Clone, Default)]
    struct ReadCaptureStore {
        reads: Arc<Mutex<Vec<DelegationResultReadQuery>>>,
    }

    impl DelegationResultStore for ReadCaptureStore {
        fn publish_delegation_result(
            &self,
            _commit: DelegationResultPublishCommit,
        ) -> Result<CommittedDelegationResult, StoreError> {
            Err(StoreError::NotFound)
        }

        fn get_delegation_result(
            &self,
            query: DelegationResultReadQuery,
        ) -> Result<Option<CommittedDelegationResult>, StoreError> {
            self.reads.lock().expect("query lock").push(query);
            Ok(None)
        }
    }

    fn parent_scope() -> DelegationChannelParentScope {
        DelegationChannelParentScope::new("owner-1", "workspace-1", "task-1", "parent-attempt-1", 7)
            .expect("authenticated Runtime parent scope is valid")
    }

    #[test]
    fn result_reads_only_the_child_under_its_bound_parent_context() {
        let store = ReadCaptureStore::default();
        let reader = DelegationChannelResultReader::new(store.clone(), parent_scope());

        assert!(reader.result("child-attempt-1").unwrap().is_none());

        let reads = store.reads.lock().expect("query lock");
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].owner_principal_id, "owner-1");
        assert_eq!(reads[0].workspace_id, "workspace-1");
        assert_eq!(reads[0].task_id, "task-1");
        assert_eq!(reads[0].parent_attempt_id, "parent-attempt-1");
        assert_eq!(reads[0].child_attempt_id, "child-attempt-1");
        assert_eq!(reads[0].task_spec_revision, 7);
    }

    #[test]
    fn malformed_child_ids_are_rejected_before_storage_access() {
        let store = ReadCaptureStore::default();
        let reader = DelegationChannelResultReader::new(store.clone(), parent_scope());

        assert!(reader.result("child\nforeign").is_err());
        assert!(store.reads.lock().expect("query lock").is_empty());
    }

    #[test]
    fn malformed_parent_context_is_rejected_at_adapter_construction() {
        assert!(
            DelegationChannelParentScope::new("owner-1", "workspace-1", "task-1", "", 7,).is_err()
        );
        assert!(
            DelegationChannelParentScope::new(
                "owner-1",
                "workspace-1",
                "task-1",
                "parent-attempt-1",
                0,
            )
            .is_err()
        );
    }
}
