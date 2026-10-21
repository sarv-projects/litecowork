//! Composed worker-selection decision.
//!
//! `DELEGATION.md` defines one decision with three outcomes: stay with the current
//! lead/capability, delegate/parallelize, or ask/fail with a typed reason. This module
//! composes the two bounded inputs that decision needs:
//!
//! * the deterministic ranker over already-eligible candidates, and
//! * the released B83 benefit gate, which decides whether automatic benefit-based
//!   selection is permitted at all for the current release.
//!
//! The composed decision never grants execution authority, never admits an Attempt,
//! and never substitutes a pinned worker. An explicit `REQUIRE` selection is either
//! kept exactly as pinned or reported as its typed unavailability error; the automatic
//! benefit gate applies only to an unpinned selection.

use crate::{
    B83BenchmarkArm, B83BenchmarkReport, DelegationBenefitDecision, DelegationBenefitPolicy,
    ProfileSelection, RankableWorkerProfile, WorkerSelectionError, WorkerSelectionRequest,
    WorkerSelectionResult, decide_delegation_benefit, rank_worker_profiles,
};
use serde_json::{Value, json};

/// Why the selector stays with the current lead or capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StayReason {
    /// No candidate met the hard eligibility predicates.
    NoEligibleCandidate,
    /// The released B83 benefit gate did not permit automatic benefit-based selection,
    /// or the measured benefit did not exceed its threshold.
    BenefitGateDidNotPermit,
}

/// The composed worker-selection decision.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkerSelectionDecision {
    /// Keep the current lead/capability and delegate nothing.
    StayWithCurrentOption { reason: StayReason },
    /// Use the selected worker profile. The caller still has to admit it through the
    /// ordinary child-attention path; this decision grants no authority.
    DelegateToProfile {
        profile_id: String,
        /// The versioned ranking rule that produced the order.
        ranking_policy_version: String,
        /// The measured improvement the released B83 gate accepted, in basis points.
        measured_improvement_basis_points: u16,
        /// The B83 policy version that authorized automatic benefit-based selection.
        benefit_policy_version: String,
    },
    /// The owner pinned a worker that is unavailable or ineligible.
    PinnedWorkerUnavailable { profile_id: String },
}

/// Inputs for the composed decision.
#[derive(Clone, Debug)]
pub struct WorkerSelectionInputs<'a> {
    /// Already-eligible candidates with their qualified observations.
    pub profiles: &'a [RankableWorkerProfile],
    /// The ranking request, including the explicit owner selection.
    pub request: &'a WorkerSelectionRequest,
    /// The released B83 policy that owns the automatic benefit thresholds.
    pub benefit_policy: &'a DelegationBenefitPolicy,
    /// The aggregated B83 evidence for the current release.
    pub benchmark_report: &'a B83BenchmarkReport,
    /// The arm the benefit gate evaluated.
    pub benefit_arm: B83BenchmarkArm,
}

/// Decide whether to stay with the current lead, delegate to a worker profile, or
/// report that a pinned worker is unavailable.
///
/// The ranker always runs first, so a pinned selection returns its typed
/// unavailability error and is never replaced by the automatic benefit gate.
pub fn decide_worker_selection(
    inputs: &WorkerSelectionInputs<'_>,
) -> Result<WorkerSelectionDecision, WorkerSelectionError> {
    let ranked: WorkerSelectionResult =
        match rank_worker_profiles(inputs.profiles, inputs.request.clone()) {
            Ok(ranked) => ranked,
            // The ranker already refused the pinned or preferred worker with its
            // typed error. The automatic benefit gate never replaces it.
            Err(error @ WorkerSelectionError::RequiredProfileUnavailable(_))
            | Err(error @ WorkerSelectionError::PreferredProfileUnavailable(_)) => {
                return Ok(WorkerSelectionDecision::PinnedWorkerUnavailable {
                    profile_id: match error {
                        WorkerSelectionError::RequiredProfileUnavailable(profile_id)
                        | WorkerSelectionError::PreferredProfileUnavailable(profile_id) => {
                            profile_id
                        }
                        _ => unreachable!(),
                    },
                });
            }
            Err(error) => return Err(error),
        };
    if matches!(inputs.request.selection, ProfileSelection::Require(_)) {
        // The pinned worker was ranked first. It stays pinned; the automatic benefit
        // gate never replaces an explicit owner selection.
        let Some(pinned) = ranked.candidates.first() else {
            return Ok(WorkerSelectionDecision::StayWithCurrentOption {
                reason: StayReason::NoEligibleCandidate,
            });
        };
        return Ok(WorkerSelectionDecision::DelegateToProfile {
            profile_id: pinned.profile_id.clone(),
            ranking_policy_version: ranked.policy_version.to_owned(),
            measured_improvement_basis_points: 0,
            benefit_policy_version: inputs.benefit_policy.policy_version.clone(),
        });
    }
    let Some(selected) = ranked.candidates.first() else {
        return Ok(WorkerSelectionDecision::StayWithCurrentOption {
            reason: StayReason::NoEligibleCandidate,
        });
    };
    let benefit = decide_delegation_benefit(
        inputs.benchmark_report,
        inputs.benefit_arm,
        inputs.benefit_policy,
    );
    let DelegationBenefitDecision::Delegate {
        measured_improvement_basis_points,
        policy_version,
        ..
    } = benefit
    else {
        // The released policy did not authorize automatic benefit-based selection, or
        // the measured benefit did not exceed its threshold. Stay with the current
        // option rather than substituting a cheaper or faster worker.
        return Ok(WorkerSelectionDecision::StayWithCurrentOption {
            reason: StayReason::BenefitGateDidNotPermit,
        });
    };
    Ok(WorkerSelectionDecision::DelegateToProfile {
        profile_id: selected.profile_id.clone(),
        ranking_policy_version: ranked.policy_version.to_owned(),
        measured_improvement_basis_points,
        benefit_policy_version: policy_version,
    })
}

/// The bounded diagnostics a caller records with an admission. They are projections,
/// never authority, and they never contain prompts, transcripts, or provider handles.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkerSelectionDiagnostics {
    pub ranking_policy_version: String,
    pub ranking_rule: String,
    pub eligible_profile_ids: Vec<String>,
    pub unknown_cost_profile_ids: Vec<String>,
    pub matched_requested_profile: bool,
    pub decision: String,
}

impl WorkerSelectionDecision {
    /// Build the bounded admission diagnostics for this decision.
    pub fn diagnostics(
        &self,
        ranked: &WorkerSelectionResult,
        matched_requested_profile: bool,
    ) -> WorkerSelectionDiagnostics {
        let decision = match self {
            WorkerSelectionDecision::StayWithCurrentOption { .. } => "STAY_WITH_CURRENT_OPTION",
            WorkerSelectionDecision::DelegateToProfile { .. } => "DELEGATE",
            WorkerSelectionDecision::PinnedWorkerUnavailable { .. } => "PINNED_WORKER_UNAVAILABLE",
        };
        WorkerSelectionDiagnostics {
            ranking_policy_version: ranked.policy_version.to_owned(),
            ranking_rule: format!("{:?}", ranked.ranking_rule),
            eligible_profile_ids: ranked
                .candidates
                .iter()
                .map(|candidate| candidate.profile_id.clone())
                .collect(),
            unknown_cost_profile_ids: ranked.unknown_cost_profile_ids.clone(),
            matched_requested_profile,
            decision: decision.to_owned(),
        }
    }

    /// The JSON shape a caller stores in admission diagnostics.
    pub fn to_diagnostic_json(&self, diagnostics: &WorkerSelectionDiagnostics) -> Value {
        json!({
            "decision": diagnostics.decision,
            "ranking_policy_version": diagnostics.ranking_policy_version,
            "ranking_rule": diagnostics.ranking_rule,
            "eligible_profile_ids": diagnostics.eligible_profile_ids,
            "unknown_cost_profile_ids": diagnostics.unknown_cost_profile_ids,
            "matched_requested_profile": diagnostics.matched_requested_profile,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DelegationBenefitObjective, WorkerObservationConfidence, WorkerPerformanceProjection,
    };
    use domain_responsibility::OptimizationPreference;

    fn projection(
        profile_id: &str,
        confidence: WorkerObservationConfidence,
    ) -> WorkerPerformanceProjection {
        WorkerPerformanceProjection {
            profile_id: profile_id.into(),
            task_category: "CODING".into(),
            sample_count: 10,
            verified_attempt_count: 10,
            verification_observation_coverage_basis_points: 10_000,
            verifier_pass_rate_basis_points: Some(9_500),
            median_latency_ms: Some(100),
            median_observed_cost: None,
            retry_rate_basis_points: Some(0),
            human_intervention_rate_basis_points: Some(0),
            confidence,
        }
    }

    fn profile(profile_id: &str) -> RankableWorkerProfile {
        RankableWorkerProfile {
            profile_id: profile_id.into(),
            eligible: true,
            performance: Some(projection(profile_id, WorkerObservationConfidence::High)),
            qualified_cost: None,
            enforced_cost_ceiling: None,
        }
    }

    fn request(selection: ProfileSelection) -> WorkerSelectionRequest {
        WorkerSelectionRequest {
            selection,
            allow_preferred_fallback: true,
            preference: OptimizationPreference::CostFirst,
            minimum_verified_pass_rate_basis_points: None,
            now: "2026-10-17T12:00:00Z".into(),
            max_cost: None,
        }
    }

    fn policy() -> DelegationBenefitPolicy {
        DelegationBenefitPolicy {
            policy_version: "worker-selection-benefit-v1".into(),
            objective: DelegationBenefitObjective::Cost,
            minimum_improvement_basis_points: 1_000,
            minimum_paired_samples: 1,
            minimum_verification_coverage_basis_points: 10_000,
            minimum_verified_success_rate_basis_points: 9_000,
            require_exact_cost_confidence: false,
        }
    }

    fn report() -> B83BenchmarkReport {
        B83BenchmarkReport {
            task_case_count: 1,
            sample_count: 2,
            arms: vec![],
            comparisons_to_lead_only: vec![],
            automatic_selection_qualified: false,
        }
    }

    fn decide(
        profiles: &[RankableWorkerProfile],
        request: &WorkerSelectionRequest,
    ) -> WorkerSelectionDecision {
        decide_worker_selection(&WorkerSelectionInputs {
            profiles,
            request,
            benefit_policy: &policy(),
            benchmark_report: &report(),
            benefit_arm: B83BenchmarkArm::QualifiedLowerCostWorker,
        })
        .expect("the decision composes")
    }

    #[test]
    fn an_unqualified_benchmark_stays_with_the_current_option() {
        let profiles = [profile("worker-a")];
        let request = request(ProfileSelection::Automatic);
        assert_eq!(
            decide(&profiles, &request),
            WorkerSelectionDecision::StayWithCurrentOption {
                reason: StayReason::BenefitGateDidNotPermit
            }
        );
    }

    #[test]
    fn a_pinned_worker_is_never_replaced_by_the_automatic_gate() {
        let profiles = [profile("worker-a")];
        let request = request(ProfileSelection::Require("worker-pinned".into()));
        assert_eq!(
            decide(&profiles, &request),
            WorkerSelectionDecision::PinnedWorkerUnavailable {
                profile_id: "worker-pinned".into()
            }
        );
    }

    #[test]
    fn an_available_pinned_worker_stays_pinned_without_benefit_evidence() {
        let profiles = [profile("worker-pinned")];
        let request = request(ProfileSelection::Require("worker-pinned".into()));
        assert_eq!(
            decide(&profiles, &request),
            WorkerSelectionDecision::DelegateToProfile {
                profile_id: "worker-pinned".into(),
                ranking_policy_version: "worker-selection-rank-v1".into(),
                measured_improvement_basis_points: 0,
                benefit_policy_version: "worker-selection-benefit-v1".into(),
            }
        );
    }

    #[test]
    fn diagnostics_record_the_order_without_prompts_or_provider_handles() {
        let profiles = [profile("worker-a")];
        let request = request(ProfileSelection::Automatic);
        let ranked = rank_worker_profiles(&profiles, request.clone()).expect("rank");
        let decision = decide(&profiles, &request);
        let diagnostics = decision.diagnostics(&ranked, false);
        let value = decision.to_diagnostic_json(&diagnostics);
        assert_eq!(value["decision"], "STAY_WITH_CURRENT_OPTION");
        assert_eq!(value["eligible_profile_ids"][0], "worker-a");
        let text = value.to_string();
        for forbidden in ["prompt", "transcript", "provider_handle", "secret"] {
            assert!(!text.contains(forbidden), "unexpected field {forbidden}");
        }
    }
}
