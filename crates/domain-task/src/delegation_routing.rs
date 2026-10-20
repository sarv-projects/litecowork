use super::delegation_routing_impl::{
    CostLimit, CostObservation, EnforcedCostCeiling, ProfileSelection, WorkerCandidate,
    WorkerObservationConfidence, WorkerRankingRule, WorkerSelectionError, WorkerSelectionRequest,
    rank_worker_candidates,
};
use domain_responsibility::OptimizationPreference;

fn candidate(
    profile_id: &str,
    cost: Option<(&str, u64)>,
    quality_bps: Option<u16>,
    latency_ms: Option<u64>,
) -> WorkerCandidate {
    WorkerCandidate {
        profile_id: profile_id.into(),
        eligible: true,
        qualified_expected_cost: cost.map(|(currency, minor_units)| {
            observed_cost(
                currency,
                minor_units,
                WorkerObservationConfidence::Medium,
                3,
                "2026-10-17T11:00:00Z",
            )
        }),
        enforced_cost_ceiling: None,
        performance_confidence: WorkerObservationConfidence::Medium,
        verified_pass_rate_basis_points: quality_bps,
        qualified_end_to_end_latency_ms: latency_ms,
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

fn observed_cost(
    currency: &str,
    minor_units: u64,
    confidence: WorkerObservationConfidence,
    sample_count: u64,
    observed_at: &str,
) -> CostObservation {
    CostObservation {
        currency: currency.into(),
        minor_units,
        source: "qualified-provider-projection".into(),
        confidence,
        sample_count,
        verification_coverage_basis_points: 10_000,
        observed_at: observed_at.into(),
        expires_at: None,
    }
}

fn cost_limit(currency: &str, minor_units: u64) -> CostLimit {
    CostLimit {
        currency: currency.into(),
        minor_units,
    }
}

#[test]
fn cost_first_ranks_known_cost_before_unknown_without_treating_unknown_as_zero() {
    let candidates = vec![
        candidate("unknown", None, Some(9_000), Some(10)),
        candidate("known-high", Some(("USD", 250)), Some(9_000), Some(10)),
        candidate("known-low", Some(("USD", 125)), Some(9_000), Some(10)),
    ];

    let ranked = rank_worker_candidates(&candidates, request(OptimizationPreference::CostFirst))
        .expect("eligible candidates should rank");

    assert_eq!(
        ranked
            .candidates
            .iter()
            .map(|entry| entry.profile_id.as_str())
            .collect::<Vec<_>>(),
        ["known-low", "known-high", "unknown"]
    );
    assert_eq!(ranked.unknown_cost_profile_ids, ["unknown"]);
    assert_eq!(
        ranked.ranking_rule,
        WorkerRankingRule::CostThenQualityThenLatency
    );
}

#[test]
fn quality_first_does_not_choose_a_cheaper_candidate_over_a_better_verified_rate() {
    let candidates = vec![
        candidate("cheap", Some(("USD", 10)), Some(8_000), Some(40)),
        candidate("verified", Some(("USD", 80)), Some(9_500), Some(90)),
    ];

    let ranked = rank_worker_candidates(&candidates, request(OptimizationPreference::QualityFirst))
        .expect("both candidates are eligible");
    assert_eq!(ranked.candidates[0].profile_id, "verified");
}

#[test]
fn latency_first_uses_qualified_end_to_end_latency_before_cost() {
    let candidates = vec![
        candidate("cheap-slow", Some(("USD", 5)), Some(9_000), Some(500)),
        candidate("fast", Some(("USD", 50)), Some(8_500), Some(100)),
    ];

    let ranked = rank_worker_candidates(&candidates, request(OptimizationPreference::LatencyFirst))
        .expect("both candidates are eligible");
    assert_eq!(ranked.candidates[0].profile_id, "fast");
}

#[test]
fn required_profile_never_falls_back_to_another_eligible_profile() {
    let candidates = vec![candidate(
        "worker-a",
        Some(("USD", 10)),
        Some(9_000),
        Some(20),
    )];
    let request = WorkerSelectionRequest {
        selection: ProfileSelection::Require("worker-b".into()),
        allow_preferred_fallback: true,
        preference: OptimizationPreference::Balanced,
        minimum_verified_pass_rate_basis_points: None,
        now: "2026-10-17T12:00:00Z".into(),
        max_cost: None,
    };

    assert_eq!(
        rank_worker_candidates(&candidates, request),
        Err(WorkerSelectionError::RequiredProfileUnavailable(
            "worker-b".into()
        ))
    );
}

#[test]
fn hard_cost_ceiling_requires_provider_enforcement_even_when_observed_cost_is_below_limit() {
    let candidates = vec![
        candidate("unknown", None, Some(9_000), Some(10)),
        candidate("eur", Some(("EUR", 1)), Some(9_000), Some(10)),
        candidate("within", Some(("USD", 99)), Some(9_000), Some(10)),
        {
            let mut enforced = candidate("enforced", Some(("USD", 500)), Some(9_000), Some(10));
            enforced.enforced_cost_ceiling = Some(EnforcedCostCeiling {
                currency: "USD".into(),
                minor_units: 100,
            });
            enforced
        },
    ];
    let request = WorkerSelectionRequest {
        selection: ProfileSelection::Automatic,
        allow_preferred_fallback: true,
        preference: OptimizationPreference::CostFirst,
        minimum_verified_pass_rate_basis_points: None,
        now: "2026-10-17T12:00:00Z".into(),
        max_cost: Some(cost_limit("USD", 100)),
    };

    let ranked = rank_worker_candidates(&candidates, request)
        .expect("the provider-enforced ceiling admits one candidate");
    assert_eq!(
        ranked
            .candidates
            .iter()
            .map(|entry| entry.profile_id.as_str())
            .collect::<Vec<_>>(),
        ["enforced"]
    );
}

#[test]
fn provider_enforced_ceiling_allows_unknown_cost_without_reporting_a_cost_estimate() {
    let mut bounded = candidate("provider-bounded", None, None, None);
    bounded.enforced_cost_ceiling = Some(EnforcedCostCeiling {
        currency: "USD".into(),
        minor_units: 100,
    });
    let request = WorkerSelectionRequest {
        selection: ProfileSelection::Automatic,
        allow_preferred_fallback: true,
        preference: OptimizationPreference::CostFirst,
        minimum_verified_pass_rate_basis_points: None,
        now: "2026-10-17T12:00:00Z".into(),
        max_cost: Some(cost_limit("USD", 100)),
    };

    let ranked = rank_worker_candidates(&[bounded], request).expect("provider enforces ceiling");
    assert_eq!(ranked.candidates[0].comparable_cost, None);
    assert_eq!(ranked.unknown_cost_profile_ids, ["provider-bounded"]);
}

#[test]
fn prefer_orders_the_selected_profile_first_and_keeps_stable_ties() {
    let candidates = vec![
        candidate("z-worker", Some(("USD", 25)), Some(8_000), Some(50)),
        candidate("preferred", Some(("USD", 25)), Some(8_000), Some(50)),
        candidate("a-worker", Some(("USD", 25)), Some(8_000), Some(50)),
    ];
    let request = WorkerSelectionRequest {
        selection: ProfileSelection::Prefer("preferred".into()),
        allow_preferred_fallback: true,
        preference: OptimizationPreference::Balanced,
        minimum_verified_pass_rate_basis_points: None,
        now: "2026-10-17T12:00:00Z".into(),
        max_cost: None,
    };

    let ranked = rank_worker_candidates(&candidates, request).expect("eligible candidates rank");
    assert_eq!(
        ranked
            .candidates
            .iter()
            .map(|entry| entry.profile_id.as_str())
            .collect::<Vec<_>>(),
        ["preferred", "a-worker", "z-worker"]
    );
    assert_eq!(
        ranked
            .candidates
            .iter()
            .map(|entry| (entry.rank, entry.matched_requested_profile))
            .collect::<Vec<_>>(),
        [(1, true), (2, false), (3, false)]
    );
}

#[test]
fn preferred_profile_does_not_fall_back_when_policy_disallows_substitution() {
    let candidates = vec![
        candidate("preferred", Some(("USD", 25)), Some(8_000), Some(50)),
        candidate("fallback", Some(("USD", 10)), Some(9_000), Some(40)),
    ];
    let request = WorkerSelectionRequest {
        selection: ProfileSelection::Prefer("preferred".into()),
        allow_preferred_fallback: false,
        preference: OptimizationPreference::QualityFirst,
        minimum_verified_pass_rate_basis_points: None,
        now: "2026-10-17T12:00:00Z".into(),
        max_cost: None,
    };

    let ranked = rank_worker_candidates(&candidates, request).expect("preferred is eligible");
    assert_eq!(ranked.candidates.len(), 1);
    assert_eq!(ranked.candidates[0].profile_id, "preferred");
}

#[test]
fn quality_floor_filters_cheaper_candidates_with_insufficient_or_unknown_evidence() {
    let mut low_confidence = candidate(
        "cheap-low-confidence",
        Some(("USD", 10)),
        Some(10_000),
        Some(20),
    );
    low_confidence.performance_confidence = WorkerObservationConfidence::Low;
    let candidates = vec![
        candidate(
            "cheap-low-quality",
            Some(("USD", 10)),
            Some(8_999),
            Some(20),
        ),
        candidate("cheap-unknown-quality", Some(("USD", 5)), None, Some(10)),
        low_confidence,
        candidate("verified-floor", Some(("USD", 100)), Some(9_000), Some(80)),
    ];
    let request = WorkerSelectionRequest {
        selection: ProfileSelection::Automatic,
        allow_preferred_fallback: true,
        preference: OptimizationPreference::CostFirst,
        minimum_verified_pass_rate_basis_points: Some(9_000),
        now: "2026-10-17T12:00:00Z".into(),
        max_cost: None,
    };

    let ranked = rank_worker_candidates(&candidates, request).expect("one candidate meets floor");
    assert_eq!(ranked.candidates.len(), 1);
    assert_eq!(ranked.candidates[0].profile_id, "verified-floor");
}

#[test]
fn expired_and_low_confidence_cost_observations_remain_unknown() {
    let mut expired = candidate("expired-cheap", Some(("USD", 1)), Some(9_000), Some(10));
    expired
        .qualified_expected_cost
        .as_mut()
        .expect("test candidate has an observation")
        .expires_at = Some("2026-10-17T11:59:00Z".into());
    let low_confidence = WorkerCandidate {
        profile_id: "low-confidence-cheap".into(),
        eligible: true,
        qualified_expected_cost: Some(observed_cost(
            "USD",
            2,
            WorkerObservationConfidence::Low,
            2,
            "2026-10-17T11:00:00Z",
        )),
        enforced_cost_ceiling: None,
        performance_confidence: WorkerObservationConfidence::Low,
        verified_pass_rate_basis_points: Some(9_000),
        qualified_end_to_end_latency_ms: Some(10),
    };
    let current = candidate("current", Some(("USD", 50)), Some(9_000), Some(10));
    let ranked = rank_worker_candidates(
        &[expired, low_confidence, current],
        request(OptimizationPreference::CostFirst),
    )
    .expect("current qualified observation remains rankable");

    assert_eq!(ranked.candidates[0].profile_id, "current");
    assert_eq!(
        ranked.unknown_cost_profile_ids,
        ["expired-cheap", "low-confidence-cheap"]
    );
}

#[test]
fn cost_observations_outside_the_projection_window_or_from_the_future_remain_unknown() {
    let stale = WorkerCandidate {
        profile_id: "stale".into(),
        eligible: true,
        qualified_expected_cost: Some(observed_cost(
            "USD",
            1,
            WorkerObservationConfidence::Medium,
            3,
            "2026-07-17T11:59:59Z",
        )),
        enforced_cost_ceiling: None,
        performance_confidence: WorkerObservationConfidence::Medium,
        verified_pass_rate_basis_points: Some(9_000),
        qualified_end_to_end_latency_ms: Some(10),
    };
    let future = WorkerCandidate {
        profile_id: "future".into(),
        eligible: true,
        qualified_expected_cost: Some(observed_cost(
            "USD",
            2,
            WorkerObservationConfidence::Medium,
            3,
            "2026-10-17T12:00:01Z",
        )),
        enforced_cost_ceiling: None,
        performance_confidence: WorkerObservationConfidence::Medium,
        verified_pass_rate_basis_points: Some(9_000),
        qualified_end_to_end_latency_ms: Some(10),
    };
    let current = candidate("current", Some(("USD", 50)), Some(9_000), Some(10));

    let ranked = rank_worker_candidates(
        &[stale, future, current],
        request(OptimizationPreference::CostFirst),
    )
    .expect("current observation remains rankable");

    assert_eq!(ranked.candidates[0].profile_id, "current");
    assert_eq!(ranked.unknown_cost_profile_ids, ["future", "stale"]);
}

#[test]
fn medium_confidence_requires_complete_observation_coverage() {
    let mut incomplete = observed_cost(
        "USD",
        1,
        WorkerObservationConfidence::Medium,
        3,
        "2026-10-17T11:00:00Z",
    );
    incomplete.verification_coverage_basis_points = 9_999;

    assert_eq!(
        rank_worker_candidates(
            &[WorkerCandidate {
                profile_id: "incomplete-medium".into(),
                eligible: true,
                qualified_expected_cost: Some(incomplete),
                enforced_cost_ceiling: None,
                performance_confidence: WorkerObservationConfidence::Medium,
                verified_pass_rate_basis_points: Some(9_000),
                qualified_end_to_end_latency_ms: Some(10),
            }],
            request(OptimizationPreference::CostFirst),
        ),
        Err(WorkerSelectionError::InvalidCandidate)
    );
}

#[test]
fn high_confidence_requires_ten_samples_and_at_least_eighty_percent_coverage() {
    let mut insufficient_samples = observed_cost(
        "USD",
        1,
        WorkerObservationConfidence::High,
        9,
        "2026-10-17T11:00:00Z",
    );
    insufficient_samples.verification_coverage_basis_points = 10_000;
    let mut insufficient_coverage = observed_cost(
        "USD",
        1,
        WorkerObservationConfidence::High,
        10,
        "2026-10-17T11:00:00Z",
    );
    insufficient_coverage.verification_coverage_basis_points = 7_999;

    for (index, observation) in [insufficient_samples, insufficient_coverage]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            rank_worker_candidates(
                &[WorkerCandidate {
                    profile_id: format!("invalid-high-{index}"),
                    eligible: true,
                    qualified_expected_cost: Some(observation),
                    enforced_cost_ceiling: None,
                    performance_confidence: WorkerObservationConfidence::Medium,
                    verified_pass_rate_basis_points: Some(9_000),
                    qualified_end_to_end_latency_ms: Some(10),
                }],
                request(OptimizationPreference::CostFirst),
            ),
            Err(WorkerSelectionError::InvalidCandidate)
        );
    }
}

#[test]
fn low_confidence_performance_metrics_do_not_influence_candidate_order() {
    let mut low_confidence = candidate("a-low-confidence", None, Some(10_000), Some(1));
    low_confidence.performance_confidence = WorkerObservationConfidence::Low;
    let qualified = candidate("z-qualified", None, Some(9_000), Some(10));

    let ranked = rank_worker_candidates(
        &[low_confidence, qualified],
        request(OptimizationPreference::QualityFirst),
    )
    .expect("both candidates are eligible");

    assert_eq!(ranked.candidates[0].profile_id, "z-qualified");
    assert_eq!(ranked.candidates[1].profile_id, "a-low-confidence");
    assert_eq!(
        ranked.candidates[1].performance_confidence,
        WorkerObservationConfidence::Low
    );
    assert_eq!(ranked.candidates[1].verified_pass_rate_basis_points, None);
    assert_eq!(ranked.candidates[1].qualified_end_to_end_latency_ms, None);
}

#[test]
fn ineligible_candidates_never_reach_ranking() {
    let mut unavailable = candidate("offline", Some(("USD", 1)), Some(10_000), Some(1));
    unavailable.eligible = false;
    let candidates = vec![unavailable, candidate("ready", None, None, None)];

    let ranked = rank_worker_candidates(&candidates, request(OptimizationPreference::QualityFirst))
        .expect("ready candidate is permitted despite unknown projections");
    assert_eq!(ranked.candidates.len(), 1);
    assert_eq!(ranked.candidates[0].profile_id, "ready");
}
