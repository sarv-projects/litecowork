//! Durable projections used to evaluate hosted child Attempt admission.
//!
//! These rows are a read-only snapshot. They do not grant authority or make a child
//! Attempt admissible; the final writer must re-read them under its serialization
//! boundary and also prove every external authority below.

use crate::{
    AgentSessionRecord, AttemptRecord, ExecutionCheck, ExecutionLeaseRecord, PlanRevisionRecord,
    StepRecord, TaskRecord, TaskSpecRevisionRecord,
};

#[derive(Clone, Debug)]
pub struct DelegationProfileAdmissionProjection {
    pub profile_id: String,
    pub workspace_id: String,
    pub agent_binding_id: String,
    pub current_revision: u64,
    pub requested_revision: u64,
    pub status: String,
    pub binding_enabled: bool,
    pub max_concurrency: u8,
    pub max_host_delegation_depth: u8,
}

#[derive(Clone, Debug)]
pub struct ChildAttemptAdmissionProofs {
    /// Fresh adapter descriptor and exact negotiated option support.
    pub adapter_qualified: ExecutionCheck,
    /// Authenticated, current Runtime process/session identity.
    pub runtime_authenticated: ExecutionCheck,
    /// Current Environment control/write lease and isolation proof.
    pub environment_control: ExecutionCheck,
    /// TrustService decision and child-scoped Grants.
    pub trust_grants: ExecutionCheck,
    /// Private credential delivery without exposing secret bytes to the Task packet.
    pub private_credentials: ExecutionCheck,
    /// Process containment, cancellation, and writer-quiescence proof.
    pub process_containment: ExecutionCheck,
    /// No unresolved Invocation or Effect blocks replacement/admission.
    pub effects_reconciled: ExecutionCheck,
    /// Current budget decision and committed/reservable scoped capacity.
    pub budget_reserved: ExecutionCheck,
}

impl Default for ChildAttemptAdmissionProofs {
    fn default() -> Self {
        Self {
            adapter_qualified: ExecutionCheck::Unsupported,
            runtime_authenticated: ExecutionCheck::Unsupported,
            environment_control: ExecutionCheck::Unsupported,
            trust_grants: ExecutionCheck::Unsupported,
            private_credentials: ExecutionCheck::Unsupported,
            process_containment: ExecutionCheck::Unsupported,
            effects_reconciled: ExecutionCheck::Unsupported,
            budget_reserved: ExecutionCheck::Unsupported,
        }
    }
}

#[derive(Clone)]
pub struct ChildAttemptAdmissionSnapshot {
    pub task: TaskRecord,
    pub task_spec: TaskSpecRevisionRecord,
    pub plan: PlanRevisionRecord,
    pub step: StepRecord,
    pub dependency_steps: Vec<StepRecord>,
    pub parent_attempt: AttemptRecord,
    pub parent_step: Option<StepRecord>,
    pub parent_session: Option<AgentSessionRecord>,
    pub parent_lease: Option<ExecutionLeaseRecord>,
    pub parent_lease_current: bool,
    pub parent_ancestry: Vec<String>,
    pub parent_profile_max_delegation_depth: Option<u8>,
    pub candidate_profile: DelegationProfileAdmissionProjection,
    pub coworker_enabled_profile_ids: Vec<String>,
    pub task_max_concurrency: Option<u8>,
    pub coworker_max_concurrency: Option<u8>,
    pub active_task_child_attempts: u8,
    pub active_profile_attempts: u8,
    pub proofs: ChildAttemptAdmissionProofs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildAttemptAdmissionBlocker {
    AdapterQualification,
    RuntimeAuthentication,
    EnvironmentControl,
    TrustGrants,
    PrivateCredentialDelivery,
    ProcessContainment,
    EffectReconciliation,
    BudgetReservation,
}

/// Reports which required prerequisite is still absent. A caller must not interpret
/// a snapshot or its blocker as authorization to start a native process.
pub fn first_unproven_child_admission_gate(
    proofs: &ChildAttemptAdmissionProofs,
) -> Option<ChildAttemptAdmissionBlocker> {
    unproven_child_admission_gates(proofs).into_iter().next()
}

pub fn unproven_child_admission_gates(
    proofs: &ChildAttemptAdmissionProofs,
) -> Vec<ChildAttemptAdmissionBlocker> {
    [
        (
            proofs.adapter_qualified,
            ChildAttemptAdmissionBlocker::AdapterQualification,
        ),
        (
            proofs.runtime_authenticated,
            ChildAttemptAdmissionBlocker::RuntimeAuthentication,
        ),
        (
            proofs.environment_control,
            ChildAttemptAdmissionBlocker::EnvironmentControl,
        ),
        (
            proofs.trust_grants,
            ChildAttemptAdmissionBlocker::TrustGrants,
        ),
        (
            proofs.private_credentials,
            ChildAttemptAdmissionBlocker::PrivateCredentialDelivery,
        ),
        (
            proofs.process_containment,
            ChildAttemptAdmissionBlocker::ProcessContainment,
        ),
        (
            proofs.effects_reconciled,
            ChildAttemptAdmissionBlocker::EffectReconciliation,
        ),
        (
            proofs.budget_reserved,
            ChildAttemptAdmissionBlocker::BudgetReservation,
        ),
    ]
    .into_iter()
    .filter_map(|(check, blocker)| (check != ExecutionCheck::Confirmed).then_some(blocker))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_admission_fails_closed_at_first_missing_authority() {
        let mut proofs = ChildAttemptAdmissionProofs {
            adapter_qualified: ExecutionCheck::Confirmed,
            runtime_authenticated: ExecutionCheck::Confirmed,
            environment_control: ExecutionCheck::Confirmed,
            trust_grants: ExecutionCheck::Confirmed,
            private_credentials: ExecutionCheck::Confirmed,
            process_containment: ExecutionCheck::Confirmed,
            effects_reconciled: ExecutionCheck::Confirmed,
            budget_reserved: ExecutionCheck::Confirmed,
        };
        assert_eq!(first_unproven_child_admission_gate(&proofs), None);
        proofs.effects_reconciled = ExecutionCheck::Unknown;
        assert_eq!(
            first_unproven_child_admission_gate(&proofs),
            Some(ChildAttemptAdmissionBlocker::EffectReconciliation)
        );
        proofs.effects_reconciled = ExecutionCheck::Confirmed;
        proofs.trust_grants = ExecutionCheck::Denied;
        assert_eq!(
            first_unproven_child_admission_gate(&proofs),
            Some(ChildAttemptAdmissionBlocker::TrustGrants)
        );
        proofs.trust_grants = ExecutionCheck::Confirmed;
        proofs.adapter_qualified = ExecutionCheck::Unknown;
        assert_eq!(
            first_unproven_child_admission_gate(&proofs),
            Some(ChildAttemptAdmissionBlocker::AdapterQualification)
        );
    }
}
