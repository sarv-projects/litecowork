//! Core persistence port for immutable, revision-pinned VerificationRuns.

use crate::{DomainEvent, StoreError};
use domain_effects::{
    EvidenceRecord, VerificationFailureCode, VerificationRunRecord, VerificationRunStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRunEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub origin_runtime_incarnation_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

/// Actor identity is attached by an authenticated Core service boundary, never by
/// verifier output or an Operator request body.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRunActor {
    pub owner_principal_id: String,
    pub verifier_service_id: String,
}

#[derive(Clone, Debug)]
pub struct StartVerificationRunCommit {
    pub workspace_id: String,
    pub actor: VerificationRunActor,
    pub request_id: String,
    pub run: VerificationRunRecord,
    pub event: VerificationRunEventContext,
}

#[derive(Clone, Debug)]
pub struct SettleVerificationRunCommit {
    pub workspace_id: String,
    pub actor: VerificationRunActor,
    pub request_id: String,
    pub verification_run_id: String,
    pub expected_version: u64,
    pub next_status: VerificationRunStatus,
    pub evidence_refs: Vec<String>,
    /// Immutable Evidence emitted by the authenticated verifier in the same
    /// transaction that settles a passing run. For FAILED/INCONCLUSIVE results
    /// this must be empty. The store assigns the exact run binding and timestamp.
    pub verified_evidence: Vec<EvidenceRecord>,
    /// Bounded, typed failure code only. Raw verifier diagnostics are never stored
    /// in events or the shared run projection.
    pub failure_code: Option<VerificationFailureCode>,
    pub event: VerificationRunEventContext,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedVerificationRun {
    pub run: VerificationRunRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// Runtime persistence port. Implementations atomically pin current TaskSpec and
/// Resource heads, write immutable dependency edges/events, and enforce terminal
/// immutability. It does not execute a verifier or finalize a Task.
pub trait VerificationRunStore: Send + Sync {
    fn start_verification_run(
        &self,
        command: StartVerificationRunCommit,
    ) -> Result<CommittedVerificationRun, StoreError>;

    fn settle_verification_run(
        &self,
        command: SettleVerificationRunCommit,
    ) -> Result<CommittedVerificationRun, StoreError>;

    fn get_verification_run(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        verification_run_id: &str,
    ) -> Result<Option<VerificationRunRecord>, StoreError>;

    /// Bounded newest-first page for one Task. `after_started_at` and
    /// `after_verification_run_id` are a single keyset cursor from the previous item
    /// (either both set or both absent), so a page boundary stays stable as new runs
    /// are appended. A Task that is absent from the selected owned Workspace is
    /// reported as `NotFound`, never as an empty page.
    fn list_verification_runs(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
        task_id: &str,
        after_started_at: Option<&str>,
        after_verification_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VerificationRunRecord>, StoreError>;
}
