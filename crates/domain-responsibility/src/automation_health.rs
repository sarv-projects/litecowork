//! Read-only health and drift projections for Routines and Automations.
//!
//! Health is a projection over persisted facts. It is never an aggregate, never
//! an execution authority, and never a second source of truth. The rules come
//! from [ROUTINES.md](../../docs/ROUTINES.md) and
//! [AUTOMATION.md](../../docs/AUTOMATION.md):
//!
//! - `DRIFTED` requires explicit incompatibility evidence from a pinned
//!   Skill/capability preflight or a verifier bound to the current revision.
//! - A transient provider error or a failed Task alone is `WARNING`.
//! - The absence of an observation is `UNKNOWN`, never `HEALTHY`.
//! - A repair proposal never mutates the published revision; the owner reviews
//!   and saves a new revision.

use crate::{AutomationStatus, RoutineRevisionInput, RoutineStatus};
use serde::{Deserialize, Serialize};

/// The drift state of a pinned revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DriftState {
    /// Observations exist and show no incompatibility.
    Healthy,
    /// A transient failure or degraded dependency was observed.
    Warning,
    /// Explicit incompatibility evidence was observed for the pinned revision.
    Drifted,
    /// No freshness-qualified observation exists.
    Unknown,
}

impl DriftState {
    /// The persisted token.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "HEALTHY",
            Self::Warning => "WARNING",
            Self::Drifted => "DRIFTED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// The health of one pinned dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DependencyHealthState {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// One pinned dependency observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyHealth {
    /// The pinned `ResourceRef` or `CapabilityRef` the observation refers to.
    pub subject_ref: serde_json::Value,
    pub state: DependencyHealthState,
    pub observed_at: Option<String>,
    pub reason_code: Option<String>,
}

/// One observed cost quantity. Amounts are never converted between units or
/// currencies, and the confidence states whether the number was measured or
/// estimated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedCost {
    pub quantity: f64,
    pub unit: String,
    pub currency: Option<String>,
    pub confidence: String,
}

/// The read-only health projection of one Routine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutineHealth {
    pub routine_id: String,
    pub current_revision: u64,
    pub last_run_at: Option<String>,
    pub last_success_at: Option<String>,
    /// `None` until at least three eligible terminal runs exist.
    pub recent_success_rate: Option<f64>,
    pub sample_size: u32,
    pub average_duration_ms: Option<u64>,
    pub observed_costs: Vec<ObservedCost>,
    pub required_dependency_health: Vec<DependencyHealth>,
    pub drift_state: DriftState,
    pub drift_evidence_refs: Vec<String>,
}

/// The count of persisted occurrences by status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OccurrenceCounts {
    pub pending: u32,
    pub claimed: u32,
    pub waiting_dependency: u32,
    pub started: u32,
    pub completed: u32,
    pub skipped: u32,
    pub failed: u32,
}

/// The read-only health projection of one Automation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationHealth {
    pub automation_id: String,
    pub current_revision: u64,
    pub status: AutomationStatus,
    /// True when this Runtime currently owns an active trigger host binding for
    /// the Automation's triggers.
    pub host_ready: bool,
    pub blockers: Vec<String>,
    pub next_occurrence_at: Option<String>,
    pub last_occurrence_at: Option<String>,
    pub nonterminal_occurrences: u32,
    pub occurrence_counts: OccurrenceCounts,
    pub routine_health: RoutineHealth,
    pub drift_state: DriftState,
    pub drift_evidence_refs: Vec<String>,
}

/// The facts a drift evaluation reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DriftInputs {
    /// Explicit incompatibility evidence bound to the current revision.
    pub explicit_drift_evidence: Vec<String>,
    /// Terminal failures that are not accompanied by drift evidence.
    pub transient_failures: u32,
    /// The freshness-qualified dependency observations.
    pub dependencies: Vec<DependencyHealth>,
}

/// Decides the drift state of a pinned revision.
///
/// The order matters: explicit incompatibility evidence wins, a transient
/// failure only degrades the state to `WARNING`, and the absence of any
/// observation stays `UNKNOWN`.
pub fn evaluate_drift_state(inputs: &DriftInputs) -> DriftState {
    if !inputs.explicit_drift_evidence.is_empty() {
        return DriftState::Drifted;
    }
    if inputs.transient_failures > 0
        || inputs
            .dependencies
            .iter()
            .any(|dependency| matches!(
                dependency.state,
                DependencyHealthState::Degraded | DependencyHealthState::Unhealthy
            ))
    {
        return DriftState::Warning;
    }
    if inputs
        .dependencies
        .iter()
        .any(|dependency| dependency.state == DependencyHealthState::Unknown)
    {
        return DriftState::Unknown;
    }
    if inputs.dependencies.is_empty() {
        // Nothing was observed. The projection cannot certify the revision as
        // healthy, and it never invents a healthy verdict.
        return DriftState::Unknown;
    }
    DriftState::Healthy
}

/// The reason codes a drift evaluation reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriftReason {
    /// An explicit incompatibility observation exists for the pinned revision.
    ExplicitEvidence(String),
    /// A terminal run failed without drift evidence.
    TransientFailure,
    /// A pinned dependency is degraded or unhealthy.
    Dependency(serde_json::Value),
    /// No freshness-qualified observation exists.
    Unobserved,
}

/// Lists the reason codes that justify a drift state.
pub fn drift_reasons(inputs: &DriftInputs) -> Vec<DriftReason> {
    let mut reasons = Vec::new();
    for evidence in &inputs.explicit_drift_evidence {
        reasons.push(DriftReason::ExplicitEvidence(evidence.clone()));
    }
    if inputs.transient_failures > 0 {
        reasons.push(DriftReason::TransientFailure);
    }
    for dependency in &inputs.dependencies {
        if matches!(
            dependency.state,
            DependencyHealthState::Degraded | DependencyHealthState::Unhealthy
        ) {
            reasons.push(DriftReason::Dependency(dependency.subject_ref.clone()));
        }
    }
    if reasons.is_empty()
        && (inputs.dependencies.is_empty()
            || inputs
                .dependencies
                .iter()
                .any(|dependency| dependency.state == DependencyHealthState::Unknown))
    {
        reasons.push(DriftReason::Unobserved);
    }
    reasons
}

/// A reviewable repair proposal for a drifted revision.
///
/// The proposal never mutates the published revision and never re-runs a Task.
/// It carries the immutable draft the owner must edit and save as a new
/// revision, plus the evidence that justifies the repair.
#[derive(Clone, Debug, PartialEq)]
pub struct DriftRepairProposal {
    pub routine_id: String,
    pub current_revision: u64,
    pub drift_state: DriftState,
    pub reasons: Vec<DriftReason>,
    pub evidence_refs: Vec<String>,
    /// The immutable draft the owner reviews. It starts from the current
    /// revision content and is never applied automatically.
    pub proposed_revision: RoutineRevisionInput,
    /// Always true: a repair is an owner-reviewed new revision.
    pub requires_owner_review: bool,
}

/// Builds a repair proposal for a drifted revision.
///
/// The proposal fails closed when the revision is not actually drifted, so the
/// UI never offers a repair for a healthy or unobserved revision.
pub fn propose_drift_repair(
    routine_id: &str,
    current_revision: u64,
    revision: &RoutineRevisionInput,
    inputs: &DriftInputs,
) -> Result<DriftRepairProposal, crate::DomainError> {
    if routine_id.trim().is_empty() || current_revision == 0 {
        return Err(crate::DomainError::InvalidDefinition);
    }
    let drift_state = evaluate_drift_state(inputs);
    if drift_state != DriftState::Drifted {
        return Err(crate::DomainError::InvalidDefinition);
    }
    Ok(DriftRepairProposal {
        routine_id: routine_id.to_owned(),
        current_revision,
        drift_state,
        reasons: drift_reasons(inputs),
        evidence_refs: inputs.explicit_drift_evidence.clone(),
        proposed_revision: revision.clone(),
        requires_owner_review: true,
    })
}
