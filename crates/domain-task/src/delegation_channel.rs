//! Internal parent-side result consumer for the DelegationChannel `result(id)` call.
//! The Runtime must construct the scope from its authenticated parent session; the
//! storage adapter independently verifies ownership and the exact parent/child pins.

use crate::{
    ChildResultReadContext, DelegationResultService, EscalationBlocker, EscalationCandidate,
    EscalationDecision, EscalationPolicy, EscalationRequest, ResultEnvelope,
    decide_delegation_escalation,
};
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

    /// Apply the bounded escalation policy to the durable child result. The decision is
    /// advisory: it proposes a new immutable Attempt but cannot admit, start, or mutate
    /// one. A result pinned to a different TaskSpec revision is stale and never escalates.
    pub fn recovery_decision(
        &self,
        child_attempt_id: &str,
        policy: &EscalationPolicy,
        request: &EscalationRequest,
        candidates: &[EscalationCandidate],
    ) -> Result<EscalationDecision, StoreError> {
        let result = self.result(child_attempt_id)?.ok_or(StoreError::NotFound)?;
        Ok(decide_child_recovery(&result, policy, request, candidates))
    }
}

/// Pure applicability guard shared by the channel consumer and its callers. The child
/// result must have started on the same TaskSpec revision the policy is evaluated
/// against; a later TaskSpec revision makes the old result inapplicable for automatic
/// escalation.
pub fn decide_child_recovery(
    result: &ResultEnvelope,
    policy: &EscalationPolicy,
    request: &EscalationRequest,
    candidates: &[EscalationCandidate],
) -> EscalationDecision {
    if result.revision_at_start != request.task_spec_revision_at_start
        || result.revision_at_start != request.current_task_spec_revision
        || request.task_spec_revision_at_start != request.current_task_spec_revision
    {
        return EscalationDecision::Blocked(EscalationBlocker::TaskSpecChanged);
    }
    decide_delegation_escalation(policy, request, candidates)
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

    fn result_envelope(revision_at_start: u64) -> ResultEnvelope {
        ResultEnvelope {
            child_attempt_id: "child-attempt-1".into(),
            status: crate::DelegationResultStatus::Failed,
            summary: "bounded failure report".into(),
            output_refs: vec![],
            artifact_refs: vec![],
            evidence_refs: vec![],
            changed_resource_refs: vec![],
            unresolved: vec![],
            blockers: vec![],
            usage: None,
            confidence: None,
            verification_hints: vec![],
            revision_at_start,
        }
    }

    fn escalation_policy() -> EscalationPolicy {
        EscalationPolicy {
            max_worker_attempts: 3,
            fallback_profile_ids: vec!["profile-fast".into()],
        }
    }

    fn escalation_request(task_spec_revision: u64) -> EscalationRequest {
        EscalationRequest {
            outcome: crate::EscalationAttemptOutcome::RecoverableVerificationFailure,
            admitted_attempt_count: 1,
            attempted_profile_ids: vec!["profile-initial".into()],
            current_task_version: 7,
            task_spec_revision_at_start: task_spec_revision,
            current_task_spec_revision: task_spec_revision,
            task_attempt_ceiling: 3,
            active_sibling_children: 0,
            effective_max_concurrency: 2,
            effects_reconciled: true,
            step_recoverable: true,
        }
    }

    fn escalation_candidate() -> EscalationCandidate {
        EscalationCandidate {
            profile_id: "profile-fast".into(),
            readiness: crate::EscalationCandidateReadiness::Qualified,
            checked_task_version: 7,
            checked_task_spec_revision: 3,
        }
    }

    #[test]
    fn recovery_decision_uses_the_durable_result_and_recommends_the_next_attempt() {
        let decision = decide_child_recovery(
            &result_envelope(3),
            &escalation_policy(),
            &escalation_request(3),
            &[escalation_candidate()],
        );
        assert_eq!(
            decision,
            EscalationDecision::RecommendNewAttempt {
                profile_id: "profile-fast".into(),
                attempt_number: 2,
                expected_task_version: 7,
                task_spec_revision: 3,
                expected_active_sibling_children: 0,
            }
        );
    }

    #[test]
    fn stale_child_result_revision_never_escalates() {
        assert_eq!(
            decide_child_recovery(
                &result_envelope(2),
                &escalation_policy(),
                &escalation_request(3),
                &[escalation_candidate()],
            ),
            EscalationDecision::Blocked(EscalationBlocker::TaskSpecChanged)
        );
        let mut changed_after_start = escalation_request(3);
        changed_after_start.current_task_spec_revision = 4;
        assert_eq!(
            decide_child_recovery(
                &result_envelope(3),
                &escalation_policy(),
                &changed_after_start,
                &[escalation_candidate()],
            ),
            EscalationDecision::Blocked(EscalationBlocker::TaskSpecChanged)
        );
    }

    #[test]
    fn missing_durable_result_is_not_found_and_channel_recovery_stays_bound() {
        let store = ReadCaptureStore::default();
        let reader = DelegationChannelResultReader::new(store.clone(), parent_scope());
        assert!(matches!(
            reader.recovery_decision(
                "child-attempt-1",
                &escalation_policy(),
                &escalation_request(7),
                &[escalation_candidate()],
            ),
            Err(StoreError::NotFound)
        ));
        assert_eq!(store.reads.lock().expect("query lock").len(), 1);
    }
}
