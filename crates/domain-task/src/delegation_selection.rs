//! Bridge from owner-scoped worker performance projections into the deterministic
//! ranking function.
//!
//! The bridge intentionally does not derive cost from the performance projection:
//! `WorkerCostMedian.amount` has no declared minor-unit scale, so treating it as
//! `CostObservation.minor_units` would invent a conversion. A caller that has a
//! qualified cost projection with an exact currency scale supplies it separately.
//! Quality and latency metrics are consumed only at MEDIUM/HIGH confidence, matching
//! the delegation performance contract; LOW observations remain diagnostics.

use crate::{
    CostObservation, EnforcedCostCeiling, WorkerCandidate, WorkerObservationConfidence,
    WorkerPerformanceProjection, WorkerSelectionError, WorkerSelectionRequest,
    WorkerSelectionResult, rank_worker_candidates,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankableWorkerProfile {
    pub profile_id: String,
    /// `true` only after the admission caller checked active adapter/Environment,
    /// Trust, grants, budget, deadline and Task/Step eligibility for this profile.
    pub eligible: bool,
    pub performance: Option<WorkerPerformanceProjection>,
    /// Exact, caller-qualified prospective cost in the currency's minor units.
    pub qualified_cost: Option<CostObservation>,
    /// Provider-enforced ceiling for this exact request, when one exists.
    pub enforced_cost_ceiling: Option<EnforcedCostCeiling>,
}

/// Map projections to ranking inputs. This is a pure projection: it neither reads
/// storage nor creates execution authority. A projection whose profile does not match
/// its slot is rejected as an integrity error rather than silently relabeled.
pub fn rank_worker_profiles(
    profiles: &[RankableWorkerProfile],
    request: WorkerSelectionRequest,
) -> Result<WorkerSelectionResult, WorkerSelectionError> {
    let mut candidates = Vec::with_capacity(profiles.len());
    for profile in profiles {
        if profile.profile_id.trim().is_empty()
            || profile.profile_id.len() > 512
            || profile.profile_id.chars().any(char::is_control)
        {
            return Err(WorkerSelectionError::InvalidCandidate);
        }
        let performance = profile
            .performance
            .as_ref()
            .map(|projection| validate_projection(&profile.profile_id, projection))
            .transpose()?;
        let confidence = performance
            .map(|projection| projection.confidence)
            .unwrap_or(WorkerObservationConfidence::Low);
        let qualified = confidence != WorkerObservationConfidence::Low;
        candidates.push(WorkerCandidate {
            profile_id: profile.profile_id.clone(),
            eligible: profile.eligible,
            // The caller's cost observation carries its own confidence and scale. It is
            // not qualified by the performance projection's confidence, which measures
            // quality and latency samples only. The ranker independently checks cost
            // confidence, freshness and currency.
            qualified_expected_cost: profile.qualified_cost.clone(),
            enforced_cost_ceiling: profile.enforced_cost_ceiling.clone(),
            performance_confidence: confidence,
            verified_pass_rate_basis_points: qualified
                .then(|| {
                    performance.and_then(|projection| projection.verifier_pass_rate_basis_points)
                })
                .flatten(),
            qualified_end_to_end_latency_ms: qualified
                .then(|| performance.and_then(|projection| projection.median_latency_ms))
                .flatten(),
        });
    }
    rank_worker_candidates(&candidates, request)
}

fn validate_projection<'a>(
    expected_profile_id: &str,
    projection: &'a WorkerPerformanceProjection,
) -> Result<&'a WorkerPerformanceProjection, WorkerSelectionError> {
    if projection.profile_id != expected_profile_id
        || projection.profile_id.trim().is_empty()
        || projection.profile_id.len() > 512
        || projection.profile_id.chars().any(char::is_control)
        || projection
            .verifier_pass_rate_basis_points
            .is_some_and(|rate| rate > 10_000)
        || projection.sample_count > u64::from(u32::MAX)
        || projection.verified_attempt_count > projection.sample_count
        || projection.verification_observation_coverage_basis_points > 10_000
    {
        return Err(WorkerSelectionError::InvalidCandidate);
    }
    Ok(projection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ProfileSelection, WorkerCostMedian, WorkerPerformanceProjection, WorkerUsageConfidence,
        WorkerUsageSource,
    };
    use domain_responsibility::OptimizationPreference;

    fn projection(
        profile_id: &str,
        pass_rate: u16,
        latency_ms: u64,
        confidence: WorkerObservationConfidence,
    ) -> WorkerPerformanceProjection {
        WorkerPerformanceProjection {
            profile_id: profile_id.into(),
            task_category: "CODING".into(),
            sample_count: 10,
            verified_attempt_count: 10,
            verification_observation_coverage_basis_points: 10_000,
            verifier_pass_rate_basis_points: Some(pass_rate),
            median_latency_ms: Some(latency_ms),
            median_observed_cost: Some(WorkerCostMedian {
                amount: 10,
                unit: "input_tokens".into(),
                currency: "USD".into(),
                sources: vec![WorkerUsageSource::ProviderReported],
                confidence: WorkerUsageConfidence::Exact,
            }),
            retry_rate_basis_points: Some(0),
            human_intervention_rate_basis_points: Some(0),
            confidence,
        }
    }

    fn profile(
        profile_id: &str,
        performance: Option<WorkerPerformanceProjection>,
        cost: Option<u64>,
    ) -> RankableWorkerProfile {
        RankableWorkerProfile {
            profile_id: profile_id.into(),
            eligible: true,
            performance,
            qualified_cost: cost.map(|minor_units| CostObservation {
                currency: "USD".into(),
                minor_units,
                source: "qualified-provider-cost".into(),
                confidence: WorkerObservationConfidence::High,
                sample_count: 10,
                verification_coverage_basis_points: 10_000,
                observed_at: "2026-10-17T11:00:00Z".into(),
                expires_at: None,
            }),
            enforced_cost_ceiling: None,
        }
    }

    fn request(preference: OptimizationPreference) -> WorkerSelectionRequest {
        WorkerSelectionRequest {
            selection: ProfileSelection::Automatic,
            allow_preferred_fallback: true,
            preference,
            minimum_verified_pass_rate_basis_points: None,
            now: "2026-10-17T12:00:00Z".into(),
            max_cost: None,
        }
    }

    #[test]
    fn low_confidence_projection_cannot_outrank_qualified_quality_or_latency() {
        let profiles = [
            profile(
                "raw-better",
                Some(projection(
                    "raw-better",
                    10_000,
                    1,
                    WorkerObservationConfidence::Low,
                )),
                Some(1),
            ),
            profile(
                "qualified",
                Some(projection(
                    "qualified",
                    9_000,
                    80,
                    WorkerObservationConfidence::Medium,
                )),
                Some(50),
            ),
        ];
        let ranked = rank_worker_profiles(&profiles, request(OptimizationPreference::QualityFirst))
            .expect("eligible profiles rank");
        assert_eq!(ranked.candidates[0].profile_id, "qualified");
        assert_eq!(
            ranked.candidates[1].performance_confidence,
            WorkerObservationConfidence::Low
        );
        assert_eq!(ranked.candidates[1].verified_pass_rate_basis_points, None);
        assert_eq!(ranked.candidates[1].qualified_end_to_end_latency_ms, None);
    }

    #[test]
    fn unknown_cost_stays_unknown_and_never_becomes_zero() {
        let profiles = [
            profile("unknown", None, None),
            profile(
                "known",
                Some(projection(
                    "known",
                    9_000,
                    50,
                    WorkerObservationConfidence::Medium,
                )),
                Some(200),
            ),
        ];
        let ranked = rank_worker_profiles(&profiles, request(OptimizationPreference::CostFirst))
            .expect("both profiles are eligible");
        assert_eq!(
            ranked
                .candidates
                .iter()
                .map(|candidate| candidate.profile_id.as_str())
                .collect::<Vec<_>>(),
            ["known", "unknown"]
        );
        assert_eq!(ranked.unknown_cost_profile_ids, ["unknown"]);
        assert!(
            ranked
                .candidates
                .iter()
                .find(|candidate| candidate.profile_id == "unknown")
                .unwrap()
                .comparable_cost
                .is_none()
        );
    }

    #[test]
    fn projection_from_another_profile_is_rejected_not_relabeled() {
        let profiles = [profile(
            "slot",
            Some(projection(
                "other",
                9_000,
                50,
                WorkerObservationConfidence::Medium,
            )),
            None,
        )];
        let result = rank_worker_profiles(&profiles, request(OptimizationPreference::Balanced));
        assert_eq!(result, Err(WorkerSelectionError::InvalidCandidate));
    }

    #[test]
    fn quality_floor_filters_unknown_and_insufficient_quality() {
        let profiles = [
            profile("unknown", None, None),
            profile(
                "below",
                Some(projection(
                    "below",
                    8_000,
                    20,
                    WorkerObservationConfidence::Medium,
                )),
                None,
            ),
            profile(
                "above",
                Some(projection(
                    "above",
                    9_500,
                    90,
                    WorkerObservationConfidence::High,
                )),
                None,
            ),
        ];
        let mut request = request(OptimizationPreference::QualityFirst);
        request.minimum_verified_pass_rate_basis_points = Some(9_000);
        let ranked = rank_worker_profiles(&profiles, request).expect("one profile meets the floor");
        assert_eq!(ranked.candidates.len(), 1);
        assert_eq!(ranked.candidates[0].profile_id, "above");
    }

    #[test]
    fn explicit_require_is_never_substituted_by_a_ranked_fallback() {
        let profiles = [profile(
            "available",
            Some(projection(
                "available",
                9_000,
                50,
                WorkerObservationConfidence::Medium,
            )),
            None,
        )];
        let mut request = request(OptimizationPreference::Balanced);
        request.selection = ProfileSelection::Require("required".into());
        assert_eq!(
            rank_worker_profiles(&profiles, request),
            Err(WorkerSelectionError::RequiredProfileUnavailable(
                "required".into()
            ))
        );
    }

    #[test]
    fn a_qualified_cost_observation_is_not_disqualified_by_low_performance_confidence() {
        // Performance confidence measures quality and latency samples. A caller-supplied
        // exact-scale cost observation carries its own confidence and must still rank.
        let profiles = [profile(
            "sparse",
            Some(projection(
                "sparse",
                9_000,
                10,
                WorkerObservationConfidence::Low,
            )),
            Some(40),
        )];
        let ranked = rank_worker_profiles(&profiles, request(OptimizationPreference::CostFirst))
            .expect("a qualified cost observation still ranks");
        assert_eq!(
            ranked.candidates[0]
                .comparable_cost
                .as_ref()
                .map(|cost| cost.minor_units),
            Some(40)
        );
        assert_eq!(
            ranked.candidates[0].verified_pass_rate_basis_points, None,
            "low-confidence quality stays a diagnostic"
        );
    }

    #[test]
    fn an_empty_performance_projection_ranks_as_unknown_rather_than_failing() {
        let mut empty = projection("empty", 9_000, 10, WorkerObservationConfidence::Low);
        empty.sample_count = 0;
        empty.verified_attempt_count = 0;
        empty.verification_observation_coverage_basis_points = 0;
        empty.verifier_pass_rate_basis_points = None;
        empty.median_latency_ms = None;
        let profiles = [profile("empty", Some(empty), None)];
        let ranked = rank_worker_profiles(&profiles, request(OptimizationPreference::Balanced))
            .expect("an unobserved profile ranks with unknown quality");
        assert_eq!(ranked.candidates[0].verified_pass_rate_basis_points, None);
        assert_eq!(ranked.candidates[0].qualified_end_to_end_latency_ms, None);
    }
}
