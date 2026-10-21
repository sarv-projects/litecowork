//! Paired, evidence-backed B83 delegation benchmark aggregation.
//!
//! The module consumes measurements captured by a real benchmark runner. It does not
//! start providers, infer missing usage, select workers, or establish B83 thresholds.
//! Reports contain measured medians and paired millisecond/quantity deltas only.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const MAX_B83_SAMPLES: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum B83BenchmarkArm {
    LeadOnly,
    DeterministicCapability,
    QualifiedLowerCostWorker,
    IndependentParallelWorkers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum B83VerificationOutcome {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum B83RunOutcome {
    Completed,
    Failed,
    Blocked,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum B83ObservationConfidence {
    Exact,
    Estimated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum B83ObservationSource {
    AgentReported,
    ProviderReported,
    HostMeasured,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B83UsageObservation {
    pub quantity: u64,
    pub unit: String,
    pub source: B83ObservationSource,
    pub confidence: B83ObservationConfidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B83CostObservation {
    pub minor_units: u64,
    pub currency: String,
    pub source: B83ObservationSource,
    pub confidence: B83ObservationConfidence,
}

/// Sanitized run record. The runner must attach immutable artifact/evidence references;
/// task prompts, outputs, transcripts, credentials, and provider-private data are absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B83RunSample {
    pub sample_id: String,
    /// Identifies the same task and repetition across comparison arms.
    pub pair_id: String,
    pub case_id: String,
    pub arm: B83BenchmarkArm,
    pub task_digest: String,
    pub acceptance_oracle_digest: String,
    /// Exact runner configuration identity for this arm and benchmark run.
    pub configuration_digest: String,
    pub provider_id: String,
    pub provider_version: String,
    pub adapter_version: String,
    pub provider_qualification_ref: String,
    pub runtime_build: String,
    pub os_build: String,
    pub admitted_at: String,
    pub first_useful_at: Option<String>,
    pub settled_at: String,
    pub handoff_requested_at: Option<String>,
    pub worker_started_at: Option<String>,
    pub outcome: B83RunOutcome,
    pub verification: Option<B83VerificationOutcome>,
    pub retry_count: Option<u32>,
    pub recovery_duration_ms: Option<u64>,
    pub human_rescue_count: Option<u32>,
    pub usage: Option<B83UsageObservation>,
    pub cost: Option<B83CostObservation>,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum B83BenchmarkError {
    EmptySamples,
    TooManySamples,
    InvalidSample(String),
    DuplicateSampleId(String),
    DuplicateArmInPair {
        pair_id: String,
        arm: B83BenchmarkArm,
    },
    PairMismatch {
        pair_id: String,
    },
    MixedArmConfiguration {
        arm: B83BenchmarkArm,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B83ArmSummary {
    pub arm: B83BenchmarkArm,
    pub sample_count: u64,
    pub completed_count: u64,
    pub failed_count: u64,
    pub blocked_count: u64,
    pub cancelled_count: u64,
    pub median_end_to_end_latency_ms: u64,
    pub first_useful_observation_count: u64,
    pub median_time_to_first_useful_ms: Option<u64>,
    pub handoff_observation_count: u64,
    pub median_handoff_start_overhead_ms: Option<u64>,
    pub verified_sample_count: u64,
    pub verification_coverage_basis_points: u16,
    /// Rate among samples that have terminal verification. Missing verifications
    /// reduce coverage and are never counted as failures or successes.
    pub verified_success_rate_basis_points: Option<u16>,
    pub retry_observation_count: u64,
    pub median_retry_count: Option<u64>,
    pub recovery_observation_count: u64,
    pub median_recovery_duration_ms: Option<u64>,
    pub human_rescue_observation_count: u64,
    pub median_human_rescue_count: Option<u64>,
    pub usage_observation_count: u64,
    pub median_usage: Option<B83UsageMedian>,
    pub cost_observation_count: u64,
    pub median_cost: Option<B83CostMedian>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B83UsageMedian {
    pub quantity: u64,
    pub unit: String,
    pub sources: Vec<B83ObservationSource>,
    pub confidence: B83ObservationConfidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B83CostMedian {
    pub minor_units: u64,
    pub currency: String,
    pub sources: Vec<B83ObservationSource>,
    pub confidence: B83ObservationConfidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B83PairedComparison {
    pub arm: B83BenchmarkArm,
    pub paired_sample_count: u64,
    /// Candidate arm minus lead-only, in measured milliseconds. Negative is lower.
    pub median_end_to_end_latency_delta_ms: i64,
    /// Present only when every paired sample has compatible measured cost.
    /// Candidate arm minus lead-only, in the reported currency's minor units.
    pub median_cost_delta_minor_units: Option<i128>,
    pub cost_currency: Option<String>,
    pub cost_delta_confidence: Option<B83ObservationConfidence>,
    pub candidate_verification_coverage_basis_points: u16,
    pub lead_only_verification_coverage_basis_points: u16,
    pub candidate_verified_success_rate_basis_points: Option<u16>,
    pub lead_only_verified_success_rate_basis_points: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B83BenchmarkReport {
    pub task_case_count: u64,
    pub sample_count: u64,
    pub arms: Vec<B83ArmSummary>,
    pub comparisons_to_lead_only: Vec<B83PairedComparison>,
    /// Always false: this aggregation is evidence input, not a threshold calibration or
    /// an automatic selector decision.
    pub automatic_selection_qualified: bool,
}

/// Aggregate one fixed corpus of real, sanitized run measurements. An arm must use one
/// exact runner configuration per report; each pair must pin the same case, task digest,
/// and acceptance oracle. Incomplete cost data remains unknown rather than zero.
pub fn aggregate_b83_benchmark(
    samples: &[B83RunSample],
) -> Result<B83BenchmarkReport, B83BenchmarkError> {
    if samples.is_empty() {
        return Err(B83BenchmarkError::EmptySamples);
    }
    if samples.len() > MAX_B83_SAMPLES {
        return Err(B83BenchmarkError::TooManySamples);
    }

    let mut sample_ids = BTreeSet::new();
    let mut pair_identity: BTreeMap<&str, (&str, &str, &str)> = BTreeMap::new();
    let mut pair_arms: BTreeSet<(&str, B83BenchmarkArm)> = BTreeSet::new();
    let mut arm_configurations: BTreeMap<B83BenchmarkArm, &str> = BTreeMap::new();
    for sample in samples {
        validate_sample(sample)?;
        if !sample_ids.insert(sample.sample_id.as_str()) {
            return Err(B83BenchmarkError::DuplicateSampleId(
                sample.sample_id.clone(),
            ));
        }
        if !pair_arms.insert((&sample.pair_id, sample.arm)) {
            return Err(B83BenchmarkError::DuplicateArmInPair {
                pair_id: sample.pair_id.clone(),
                arm: sample.arm,
            });
        }
        match pair_identity.get(sample.pair_id.as_str()) {
            Some((case_id, task_digest, oracle_digest))
                if *case_id != sample.case_id
                    || *task_digest != sample.task_digest
                    || *oracle_digest != sample.acceptance_oracle_digest =>
            {
                return Err(B83BenchmarkError::PairMismatch {
                    pair_id: sample.pair_id.clone(),
                });
            }
            Some(_) => {}
            None => {
                pair_identity.insert(
                    sample.pair_id.as_str(),
                    (
                        sample.case_id.as_str(),
                        sample.task_digest.as_str(),
                        sample.acceptance_oracle_digest.as_str(),
                    ),
                );
            }
        }
        match arm_configurations.get(&sample.arm) {
            Some(configuration) if *configuration != sample.configuration_digest => {
                return Err(B83BenchmarkError::MixedArmConfiguration { arm: sample.arm });
            }
            Some(_) => {}
            None => {
                arm_configurations.insert(sample.arm, sample.configuration_digest.as_str());
            }
        }
    }

    let mut by_arm: BTreeMap<B83BenchmarkArm, Vec<&B83RunSample>> = BTreeMap::new();
    for sample in samples {
        by_arm.entry(sample.arm).or_default().push(sample);
    }
    let arms = by_arm
        .iter()
        .map(|(arm, values)| summarize_arm(*arm, values))
        .collect::<Vec<_>>();
    let comparisons_to_lead_only = paired_comparisons(samples);
    Ok(B83BenchmarkReport {
        task_case_count: pair_identity
            .values()
            .map(|(case_id, _, _)| *case_id)
            .collect::<BTreeSet<_>>()
            .len() as u64,
        sample_count: samples.len() as u64,
        arms,
        comparisons_to_lead_only,
        automatic_selection_qualified: false,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationBenefitObjective {
    Cost,
    Latency,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationBenefitPolicy {
    /// Released policy identity that owns these thresholds. The function never invents
    /// or tunes a threshold from an individual report.
    pub policy_version: String,
    pub objective: DelegationBenefitObjective,
    pub minimum_improvement_basis_points: u16,
    pub minimum_paired_samples: u64,
    pub minimum_verification_coverage_basis_points: u16,
    pub minimum_verified_success_rate_basis_points: u16,
    pub require_exact_cost_confidence: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegationBenefitBlocker {
    InvalidPolicy,
    NoBaseline,
    InsufficientSamples,
    InsufficientVerificationCoverage,
    QualityFloorUnmet,
    UnqualifiedCostObservation,
    BelowThreshold,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DelegationBenefitDecision {
    StayWithCurrentOption(DelegationBenefitBlocker),
    Delegate {
        arm: B83BenchmarkArm,
        objective: DelegationBenefitObjective,
        measured_improvement_basis_points: u16,
        paired_sample_count: u64,
        policy_version: String,
    },
}

/// Apply released B83 thresholds to one aggregated report. The decision is policy
/// evaluation only: it cannot select a profile, override an explicit `REQUIRE`, or
/// create an Attempt. Missing/incomparable observations, low coverage, a quality
/// regression, or a benefit below the released threshold keep the current lead.
pub fn decide_delegation_benefit(
    report: &B83BenchmarkReport,
    arm: B83BenchmarkArm,
    policy: &DelegationBenefitPolicy,
) -> DelegationBenefitDecision {
    if policy.policy_version.trim().is_empty()
        || policy.policy_version.len() > 128
        || policy.policy_version.chars().any(char::is_control)
        || arm == B83BenchmarkArm::LeadOnly
        || policy.minimum_improvement_basis_points > 10_000
        || policy.minimum_verification_coverage_basis_points > 10_000
        || policy.minimum_verified_success_rate_basis_points > 10_000
    {
        return DelegationBenefitDecision::StayWithCurrentOption(
            DelegationBenefitBlocker::InvalidPolicy,
        );
    }
    let Some(comparison) = report
        .comparisons_to_lead_only
        .iter()
        .find(|comparison| comparison.arm == arm)
    else {
        return DelegationBenefitDecision::StayWithCurrentOption(
            DelegationBenefitBlocker::NoBaseline,
        );
    };
    if comparison.paired_sample_count == 0
        || comparison.paired_sample_count < policy.minimum_paired_samples
    {
        return DelegationBenefitDecision::StayWithCurrentOption(
            DelegationBenefitBlocker::InsufficientSamples,
        );
    }
    if comparison.candidate_verification_coverage_basis_points
        < policy.minimum_verification_coverage_basis_points
        || comparison.lead_only_verification_coverage_basis_points
            < policy.minimum_verification_coverage_basis_points
    {
        return DelegationBenefitDecision::StayWithCurrentOption(
            DelegationBenefitBlocker::InsufficientVerificationCoverage,
        );
    }
    let (Some(candidate_rate), Some(lead_rate)) = (
        comparison.candidate_verified_success_rate_basis_points,
        comparison.lead_only_verified_success_rate_basis_points,
    ) else {
        return DelegationBenefitDecision::StayWithCurrentOption(
            DelegationBenefitBlocker::QualityFloorUnmet,
        );
    };
    if candidate_rate < policy.minimum_verified_success_rate_basis_points
        || candidate_rate < lead_rate
    {
        return DelegationBenefitDecision::StayWithCurrentOption(
            DelegationBenefitBlocker::QualityFloorUnmet,
        );
    }

    let improvement = match policy.objective {
        DelegationBenefitObjective::Cost => {
            let Some(delta) = comparison.median_cost_delta_minor_units else {
                return DelegationBenefitDecision::StayWithCurrentOption(
                    DelegationBenefitBlocker::NoBaseline,
                );
            };
            let Some(currency) = comparison.cost_currency.as_deref() else {
                return DelegationBenefitDecision::StayWithCurrentOption(
                    DelegationBenefitBlocker::NoBaseline,
                );
            };
            if policy.require_exact_cost_confidence
                && comparison.cost_delta_confidence != Some(B83ObservationConfidence::Exact)
            {
                return DelegationBenefitDecision::StayWithCurrentOption(
                    DelegationBenefitBlocker::UnqualifiedCostObservation,
                );
            }
            let lead = report
                .arms
                .iter()
                .find(|summary| summary.arm == B83BenchmarkArm::LeadOnly)
                .and_then(|summary| summary.median_cost.as_ref())
                .filter(|cost| cost.currency == currency && cost.minor_units > 0);
            let Some(lead) = lead else {
                return DelegationBenefitDecision::StayWithCurrentOption(
                    DelegationBenefitBlocker::NoBaseline,
                );
            };
            improvement_basis_points(delta, lead.minor_units)
        }
        DelegationBenefitObjective::Latency => {
            let lead = report
                .arms
                .iter()
                .find(|summary| summary.arm == B83BenchmarkArm::LeadOnly)
                .map(|summary| summary.median_end_to_end_latency_ms)
                .filter(|latency| *latency > 0);
            let Some(lead) = lead else {
                return DelegationBenefitDecision::StayWithCurrentOption(
                    DelegationBenefitBlocker::NoBaseline,
                );
            };
            improvement_basis_points(
                i128::from(comparison.median_end_to_end_latency_delta_ms),
                lead,
            )
        }
    };
    let Some(improvement) = improvement else {
        return DelegationBenefitDecision::StayWithCurrentOption(
            DelegationBenefitBlocker::BelowThreshold,
        );
    };
    if improvement < policy.minimum_improvement_basis_points {
        return DelegationBenefitDecision::StayWithCurrentOption(
            DelegationBenefitBlocker::BelowThreshold,
        );
    }
    DelegationBenefitDecision::Delegate {
        arm,
        objective: policy.objective,
        measured_improvement_basis_points: improvement,
        paired_sample_count: comparison.paired_sample_count,
        policy_version: policy.policy_version.clone(),
    }
}

fn improvement_basis_points(delta: i128, baseline: u64) -> Option<u16> {
    if delta >= 0 || baseline == 0 {
        return None;
    }
    let improvement = u128::from(delta.unsigned_abs()) * 10_000 / u128::from(baseline);
    u16::try_from(improvement).ok()
}

fn validate_sample(sample: &B83RunSample) -> Result<(), B83BenchmarkError> {
    let valid_text = |value: &str, max: usize| {
        !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
    };
    let valid_digest = |value: &str| {
        value.len() == 71
            && value.starts_with("sha256:")
            && value[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    };
    if !valid_text(&sample.sample_id, 256)
        || !valid_text(&sample.pair_id, 256)
        || !valid_text(&sample.case_id, 256)
        || !valid_digest(&sample.task_digest)
        || !valid_digest(&sample.acceptance_oracle_digest)
        || !valid_digest(&sample.configuration_digest)
        || !valid_text(&sample.provider_id, 256)
        || !valid_text(&sample.provider_version, 256)
        || !valid_text(&sample.adapter_version, 256)
        || !valid_text(&sample.provider_qualification_ref, 512)
        || !valid_text(&sample.runtime_build, 256)
        || !valid_text(&sample.os_build, 256)
        || sample.evidence_refs.is_empty()
        || sample.evidence_refs.len() > 64
        || sample
            .evidence_refs
            .iter()
            .any(|value| !valid_text(value, 512))
        || sample.evidence_refs.iter().collect::<BTreeSet<_>>().len() != sample.evidence_refs.len()
        || sample
            .usage
            .as_ref()
            .is_some_and(|usage| !valid_text(&usage.unit, 64))
        || sample.cost.as_ref().is_some_and(|cost| {
            cost.currency.len() != 3 || !cost.currency.bytes().all(|byte| byte.is_ascii_uppercase())
        })
    {
        return Err(B83BenchmarkError::InvalidSample(sample.sample_id.clone()));
    }
    let admitted = parse_time(&sample.admitted_at);
    let settled = parse_time(&sample.settled_at);
    if admitted.is_none() || settled.is_none() || settled <= admitted {
        return Err(B83BenchmarkError::InvalidSample(sample.sample_id.clone()));
    }
    if let Some(first_useful) = &sample.first_useful_at {
        let first_useful = parse_time(first_useful);
        if first_useful.is_none() || first_useful < admitted || first_useful > settled {
            return Err(B83BenchmarkError::InvalidSample(sample.sample_id.clone()));
        }
    }
    match (&sample.handoff_requested_at, &sample.worker_started_at) {
        (Some(handoff), Some(worker_start)) => {
            let handoff = parse_time(handoff);
            let worker_start = parse_time(worker_start);
            if handoff.is_none()
                || worker_start.is_none()
                || handoff < admitted
                || worker_start < handoff
                || worker_start > settled
            {
                return Err(B83BenchmarkError::InvalidSample(sample.sample_id.clone()));
            }
        }
        (None, None) => {}
        _ => return Err(B83BenchmarkError::InvalidSample(sample.sample_id.clone())),
    }
    Ok(())
}

fn summarize_arm(arm: B83BenchmarkArm, samples: &[&B83RunSample]) -> B83ArmSummary {
    let latencies = samples
        .iter()
        .filter_map(|sample| elapsed_ms(&sample.admitted_at, &sample.settled_at))
        .collect::<Vec<_>>();
    let first_useful = samples
        .iter()
        .filter_map(|sample| {
            sample
                .first_useful_at
                .as_deref()
                .and_then(|at| elapsed_ms(&sample.admitted_at, at))
        })
        .collect::<Vec<_>>();
    let handoff = samples
        .iter()
        .filter_map(|sample| {
            elapsed_ms(
                sample.handoff_requested_at.as_deref()?,
                sample.worker_started_at.as_deref()?,
            )
        })
        .collect::<Vec<_>>();
    let verified = samples
        .iter()
        .filter_map(|sample| sample.verification)
        .collect::<Vec<_>>();
    let verification_coverage_basis_points = rate_bps(verified.len() as u64, samples.len() as u64);
    let pass_count = verified
        .iter()
        .filter(|outcome| **outcome == B83VerificationOutcome::Passed)
        .count() as u64;
    let retries = samples
        .iter()
        .filter_map(|sample| sample.retry_count.map(u64::from))
        .collect::<Vec<_>>();
    let recovery = samples
        .iter()
        .filter_map(|sample| sample.recovery_duration_ms)
        .collect::<Vec<_>>();
    let rescues = samples
        .iter()
        .filter_map(|sample| sample.human_rescue_count.map(u64::from))
        .collect::<Vec<_>>();
    let usage = samples
        .iter()
        .filter_map(|sample| sample.usage.as_ref())
        .collect::<Vec<_>>();
    let cost = samples
        .iter()
        .filter_map(|sample| sample.cost.as_ref())
        .collect::<Vec<_>>();
    B83ArmSummary {
        arm,
        sample_count: samples.len() as u64,
        median_end_to_end_latency_ms: median(&latencies).expect("validated samples have latency"),
        completed_count: samples
            .iter()
            .filter(|sample| sample.outcome == B83RunOutcome::Completed)
            .count() as u64,
        failed_count: samples
            .iter()
            .filter(|sample| sample.outcome == B83RunOutcome::Failed)
            .count() as u64,
        blocked_count: samples
            .iter()
            .filter(|sample| sample.outcome == B83RunOutcome::Blocked)
            .count() as u64,
        cancelled_count: samples
            .iter()
            .filter(|sample| sample.outcome == B83RunOutcome::Cancelled)
            .count() as u64,
        first_useful_observation_count: first_useful.len() as u64,
        median_time_to_first_useful_ms: (first_useful.len() == samples.len())
            .then(|| median(&first_useful))
            .flatten(),
        handoff_observation_count: handoff.len() as u64,
        median_handoff_start_overhead_ms: (!handoff.is_empty() && handoff.len() == samples.len())
            .then(|| median(&handoff))
            .flatten(),
        verified_sample_count: verified.len() as u64,
        verification_coverage_basis_points,
        verified_success_rate_basis_points: (!verified.is_empty())
            .then(|| rate_bps(pass_count, verified.len() as u64)),
        retry_observation_count: retries.len() as u64,
        median_retry_count: (retries.len() == samples.len())
            .then(|| median(&retries))
            .flatten(),
        recovery_observation_count: recovery.len() as u64,
        median_recovery_duration_ms: (recovery.len() == samples.len())
            .then(|| median(&recovery))
            .flatten(),
        human_rescue_observation_count: rescues.len() as u64,
        median_human_rescue_count: (rescues.len() == samples.len())
            .then(|| median(&rescues))
            .flatten(),
        usage_observation_count: usage.len() as u64,
        median_usage: complete_usage_median(&usage, samples.len()),
        cost_observation_count: cost.len() as u64,
        median_cost: complete_cost_median(&cost, samples.len()),
    }
}

fn paired_comparisons(samples: &[B83RunSample]) -> Vec<B83PairedComparison> {
    let mut pairs: BTreeMap<&str, BTreeMap<B83BenchmarkArm, &B83RunSample>> = BTreeMap::new();
    for sample in samples {
        pairs
            .entry(sample.pair_id.as_str())
            .or_default()
            .insert(sample.arm, sample);
    }
    let mut by_arm: BTreeMap<B83BenchmarkArm, Vec<(&B83RunSample, &B83RunSample)>> =
        BTreeMap::new();
    for arms in pairs.values() {
        let Some(lead) = arms.get(&B83BenchmarkArm::LeadOnly) else {
            continue;
        };
        for (arm, candidate) in arms {
            if *arm != B83BenchmarkArm::LeadOnly {
                by_arm.entry(*arm).or_default().push((*candidate, *lead));
            }
        }
    }
    by_arm
        .into_iter()
        .map(|(arm, paired)| {
            let latency_deltas = paired
                .iter()
                .filter_map(|(candidate, lead)| {
                    Some(
                        i128::from(elapsed_ms(&candidate.admitted_at, &candidate.settled_at)?)
                            - i128::from(elapsed_ms(&lead.admitted_at, &lead.settled_at)?),
                    )
                })
                .collect::<Vec<_>>();
            let common_currency = paired
                .first()
                .and_then(|(candidate, _)| Some(candidate.cost.as_ref()?.currency.as_str()))
                .filter(|currency| {
                    paired.iter().all(|(candidate, lead)| {
                        candidate
                            .cost
                            .as_ref()
                            .is_some_and(|cost| cost.currency.as_str() == *currency)
                            && lead
                                .cost
                                .as_ref()
                                .is_some_and(|cost| cost.currency.as_str() == *currency)
                    })
                });
            let all_costs_comparable = common_currency.is_some();
            let cost_delta_confidence =
                all_costs_comparable.then(|| {
                    if paired.iter().any(|(candidate, lead)| {
                        candidate.cost.as_ref().is_some_and(|cost| {
                            cost.confidence == B83ObservationConfidence::Estimated
                        }) || lead.cost.as_ref().is_some_and(|cost| {
                            cost.confidence == B83ObservationConfidence::Estimated
                        })
                    }) {
                        B83ObservationConfidence::Estimated
                    } else {
                        B83ObservationConfidence::Exact
                    }
                });
            let cost_deltas = all_costs_comparable.then(|| {
                paired
                    .iter()
                    .filter_map(|(candidate, lead)| {
                        Some(
                            i128::from(candidate.cost.as_ref()?.minor_units)
                                - i128::from(lead.cost.as_ref()?.minor_units),
                        )
                    })
                    .collect::<Vec<_>>()
            });
            let currency = common_currency.map(str::to_owned);
            let candidate_verified = paired
                .iter()
                .filter_map(|(candidate, _)| candidate.verification)
                .collect::<Vec<_>>();
            let lead_verified = paired
                .iter()
                .filter_map(|(_, lead)| lead.verification)
                .collect::<Vec<_>>();
            Some(B83PairedComparison {
                arm,
                paired_sample_count: paired.len() as u64,
                median_end_to_end_latency_delta_ms: i64::try_from(median_signed(&latency_deltas)?)
                    .ok()?,
                median_cost_delta_minor_units: cost_deltas
                    .and_then(|values| median_signed(&values)),
                cost_currency: currency,
                cost_delta_confidence,
                candidate_verification_coverage_basis_points: rate_bps(
                    candidate_verified.len() as u64,
                    paired.len() as u64,
                ),
                lead_only_verification_coverage_basis_points: rate_bps(
                    lead_verified.len() as u64,
                    paired.len() as u64,
                ),
                candidate_verified_success_rate_basis_points: verified_rate(&candidate_verified),
                lead_only_verified_success_rate_basis_points: verified_rate(&lead_verified),
            })
        })
        .flatten()
        .collect()
}

fn complete_usage_median(values: &[&B83UsageObservation], total: usize) -> Option<B83UsageMedian> {
    if values.len() != total {
        return None;
    }
    let first = values.first()?;
    if values.iter().any(|value| value.unit != first.unit) {
        return None;
    }
    Some(B83UsageMedian {
        quantity: median(
            &values
                .iter()
                .map(|value| value.quantity)
                .collect::<Vec<_>>(),
        )?,
        unit: first.unit.clone(),
        sources: unique_sources(values.iter().map(|value| value.source)),
        confidence: if values
            .iter()
            .any(|value| value.confidence == B83ObservationConfidence::Estimated)
        {
            B83ObservationConfidence::Estimated
        } else {
            B83ObservationConfidence::Exact
        },
    })
}

fn complete_cost_median(values: &[&B83CostObservation], total: usize) -> Option<B83CostMedian> {
    if values.len() != total {
        return None;
    }
    let first = values.first()?;
    if values.iter().any(|value| value.currency != first.currency) {
        return None;
    }
    Some(B83CostMedian {
        minor_units: median(
            &values
                .iter()
                .map(|value| value.minor_units)
                .collect::<Vec<_>>(),
        )?,
        currency: first.currency.clone(),
        sources: unique_sources(values.iter().map(|value| value.source)),
        confidence: if values
            .iter()
            .any(|value| value.confidence == B83ObservationConfidence::Estimated)
        {
            B83ObservationConfidence::Estimated
        } else {
            B83ObservationConfidence::Exact
        },
    })
}

fn unique_sources(values: impl Iterator<Item = B83ObservationSource>) -> Vec<B83ObservationSource> {
    values.collect::<BTreeSet<_>>().into_iter().collect()
}

fn verified_rate(outcomes: &[B83VerificationOutcome]) -> Option<u16> {
    if outcomes.is_empty() {
        return None;
    }
    let passed = outcomes
        .iter()
        .filter(|outcome| **outcome == B83VerificationOutcome::Passed)
        .count() as u64;
    Some(rate_bps(passed, outcomes.len() as u64))
}

fn parse_time(value: &str) -> Option<OffsetDateTime> {
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return None;
    }
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

fn elapsed_ms(start: &str, end: &str) -> Option<u64> {
    let start = parse_time(start)?;
    let end = parse_time(end)?;
    u64::try_from((end - start).whole_milliseconds()).ok()
}

fn median(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        Some(values[middle])
    } else {
        Some(((u128::from(values[middle - 1]) + u128::from(values[middle])) / 2) as u64)
    }
}

fn median_signed(values: &[i128]) -> Option<i128> {
    if values.is_empty() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        Some(values[middle])
    } else {
        Some((values[middle - 1] + values[middle]) / 2)
    }
}

fn rate_bps(numerator: u64, denominator: u64) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((u128::from(numerator) * 10_000) / u128::from(denominator)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, pair: &str, arm: B83BenchmarkArm, duration_ms: i64) -> B83RunSample {
        let admitted = OffsetDateTime::parse("2026-10-18T10:00:00Z", &Rfc3339).unwrap();
        let settled = admitted + time::Duration::milliseconds(duration_ms);
        let fmt = |value: OffsetDateTime| {
            format!(
                "2026-10-18T10:00:{:02}.{:03}Z",
                value.second(),
                value.millisecond()
            )
        };
        let handoff = (arm != B83BenchmarkArm::LeadOnly)
            .then(|| fmt(admitted + time::Duration::milliseconds(100)));
        let worker = (arm != B83BenchmarkArm::LeadOnly)
            .then(|| fmt(admitted + time::Duration::milliseconds(200)));
        B83RunSample {
            sample_id: id.into(),
            pair_id: pair.into(),
            case_id: "code-change-01".into(),
            arm,
            task_digest: format!("sha256:{}", "a".repeat(64)),
            acceptance_oracle_digest: format!("sha256:{}", "b".repeat(64)),
            configuration_digest: format!("sha256:{}", "c".repeat(64)),
            provider_id: "provider-x".into(),
            provider_version: "1.2.3".into(),
            adapter_version: "adapter-1".into(),
            provider_qualification_ref: format!("qualification-{id}"),
            runtime_build: "runtime-1".into(),
            os_build: "Windows-26100".into(),
            admitted_at: fmt(admitted),
            first_useful_at: Some(fmt(admitted + time::Duration::milliseconds(500))),
            settled_at: fmt(settled),
            handoff_requested_at: handoff,
            worker_started_at: worker,
            outcome: B83RunOutcome::Completed,
            verification: Some(B83VerificationOutcome::Passed),
            retry_count: Some(0),
            recovery_duration_ms: Some(0),
            human_rescue_count: Some(0),
            usage: Some(B83UsageObservation {
                quantity: 100,
                unit: "input_tokens".into(),
                source: B83ObservationSource::ProviderReported,
                confidence: B83ObservationConfidence::Exact,
            }),
            cost: Some(B83CostObservation {
                minor_units: 100,
                currency: "USD".into(),
                source: B83ObservationSource::ProviderReported,
                confidence: B83ObservationConfidence::Exact,
            }),
            evidence_refs: vec![format!("evidence-{id}")],
        }
    }

    #[test]
    fn paired_report_compares_measured_latency_cost_and_verified_success_without_multiplier() {
        let mut lead_a = sample("lead-a", "pair-a", B83BenchmarkArm::LeadOnly, 1_000);
        lead_a.cost.as_mut().unwrap().minor_units = 100;
        let mut worker_a = sample(
            "worker-a",
            "pair-a",
            B83BenchmarkArm::QualifiedLowerCostWorker,
            1_200,
        );
        worker_a.cost.as_mut().unwrap().minor_units = 80;
        worker_a.verification = Some(B83VerificationOutcome::Failed);
        let mut lead_b = sample("lead-b", "pair-b", B83BenchmarkArm::LeadOnly, 2_000);
        lead_b.cost.as_mut().unwrap().minor_units = 200;
        let mut worker_b = sample(
            "worker-b",
            "pair-b",
            B83BenchmarkArm::QualifiedLowerCostWorker,
            1_800,
        );
        worker_b.cost.as_mut().unwrap().minor_units = 180;

        let report = aggregate_b83_benchmark(&[lead_a, worker_a, lead_b, worker_b]).unwrap();
        let worker = report
            .arms
            .iter()
            .find(|arm| arm.arm == B83BenchmarkArm::QualifiedLowerCostWorker)
            .unwrap();
        assert_eq!(worker.median_end_to_end_latency_ms, 1_500);
        assert_eq!(
            worker.median_cost.as_ref().map(|cost| cost.minor_units),
            Some(130)
        );
        assert_eq!(worker.verified_success_rate_basis_points, Some(5_000));
        assert_eq!(worker.median_handoff_start_overhead_ms, Some(100));
        let comparison = &report.comparisons_to_lead_only[0];
        assert_eq!(comparison.median_end_to_end_latency_delta_ms, 0);
        assert_eq!(comparison.median_cost_delta_minor_units, Some(-20));
        assert_eq!(
            comparison.candidate_verified_success_rate_basis_points,
            Some(5_000)
        );
        assert!(!report.automatic_selection_qualified);
    }

    #[test]
    fn missing_or_incompatible_cost_stays_unknown_and_verification_coverage_is_explicit() {
        let mut lead = sample("lead", "pair", B83BenchmarkArm::LeadOnly, 1_000);
        lead.cost = None;
        lead.verification = None;
        let mut worker = sample(
            "worker",
            "pair",
            B83BenchmarkArm::QualifiedLowerCostWorker,
            800,
        );
        worker.cost.as_mut().unwrap().currency = "EUR".into();
        let report = aggregate_b83_benchmark(&[lead, worker]).unwrap();
        assert!(
            report
                .arms
                .iter()
                .find(|arm| arm.arm == B83BenchmarkArm::LeadOnly)
                .unwrap()
                .median_cost
                .is_none()
        );
        assert_eq!(
            report
                .arms
                .iter()
                .find(|arm| arm.arm == B83BenchmarkArm::QualifiedLowerCostWorker)
                .unwrap()
                .median_cost
                .as_ref()
                .map(|cost| cost.currency.as_str()),
            Some("EUR")
        );
        assert_eq!(
            report.comparisons_to_lead_only[0].median_cost_delta_minor_units,
            None
        );
        let lead = report
            .arms
            .iter()
            .find(|arm| arm.arm == B83BenchmarkArm::LeadOnly)
            .unwrap();
        assert_eq!(lead.verification_coverage_basis_points, 0);
        assert_eq!(lead.verified_success_rate_basis_points, None);
    }

    #[test]
    fn paired_cost_delta_stays_unknown_when_currencies_differ_between_pairs() {
        let lead_usd = sample("lead-usd", "pair-usd", B83BenchmarkArm::LeadOnly, 1_000);
        let mut worker_usd = sample(
            "worker-usd",
            "pair-usd",
            B83BenchmarkArm::QualifiedLowerCostWorker,
            900,
        );
        worker_usd.cost.as_mut().unwrap().minor_units = 90;
        let mut lead_eur = sample("lead-eur", "pair-eur", B83BenchmarkArm::LeadOnly, 1_000);
        lead_eur.cost.as_mut().unwrap().currency = "EUR".into();
        let mut worker_eur = sample(
            "worker-eur",
            "pair-eur",
            B83BenchmarkArm::QualifiedLowerCostWorker,
            900,
        );
        worker_eur.cost.as_mut().unwrap().currency = "EUR".into();
        worker_eur.cost.as_mut().unwrap().minor_units = 90;

        let report =
            aggregate_b83_benchmark(&[lead_usd, worker_usd, lead_eur, worker_eur]).unwrap();
        let comparison = report
            .comparisons_to_lead_only
            .iter()
            .find(|item| item.arm == B83BenchmarkArm::QualifiedLowerCostWorker)
            .unwrap();
        assert_eq!(comparison.median_cost_delta_minor_units, None);
        assert_eq!(comparison.cost_currency, None);
        assert_eq!(comparison.cost_delta_confidence, None);
    }

    #[test]
    fn mismatched_or_duplicate_pairs_are_rejected() {
        let lead = sample("lead", "pair", B83BenchmarkArm::LeadOnly, 1_000);
        let mut worker = sample(
            "worker",
            "pair",
            B83BenchmarkArm::QualifiedLowerCostWorker,
            900,
        );
        worker.acceptance_oracle_digest = format!("sha256:{}", "d".repeat(64));
        assert!(matches!(
            aggregate_b83_benchmark(&[lead.clone(), worker]),
            Err(B83BenchmarkError::PairMismatch { .. })
        ));
        let worker = sample(
            "worker",
            "pair",
            B83BenchmarkArm::QualifiedLowerCostWorker,
            900,
        );
        let duplicate = sample(
            "worker-2",
            "pair",
            B83BenchmarkArm::QualifiedLowerCostWorker,
            900,
        );
        assert!(matches!(
            aggregate_b83_benchmark(&[lead, worker, duplicate]),
            Err(B83BenchmarkError::DuplicateArmInPair { .. })
        ));
    }

    #[test]
    fn each_arm_requires_one_exact_runner_configuration() {
        let first = sample("lead-a", "pair-a", B83BenchmarkArm::LeadOnly, 1_000);
        let mut second = sample("lead-b", "pair-b", B83BenchmarkArm::LeadOnly, 1_000);
        second.configuration_digest = format!("sha256:{}", "d".repeat(64));
        assert_eq!(
            aggregate_b83_benchmark(&[first, second]),
            Err(B83BenchmarkError::MixedArmConfiguration {
                arm: B83BenchmarkArm::LeadOnly
            })
        );
    }

    #[test]
    fn sanitized_sample_and_report_round_trip_and_reject_uncontracted_transcript_fields() {
        let input = sample("lead", "pair", B83BenchmarkArm::LeadOnly, 1_000);
        let encoded = serde_json::to_vec(&input).unwrap();
        let decoded: B83RunSample = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, input);
        assert!(
            String::from_utf8(encoded)
                .unwrap()
                .contains("\"arm\":\"lead_only\"")
        );

        let mut malicious = serde_json::to_value(&input).unwrap();
        malicious["transcript"] = serde_json::json!("private conversation content");
        assert!(serde_json::from_value::<B83RunSample>(malicious).is_err());

        let report = aggregate_b83_benchmark(&[input]).unwrap();
        let encoded = serde_json::to_vec(&report).unwrap();
        let decoded: B83BenchmarkReport = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, report);
    }

    fn arm_summary(
        arm: B83BenchmarkArm,
        latency_ms: u64,
        cost_minor_units: Option<u64>,
    ) -> B83ArmSummary {
        B83ArmSummary {
            arm,
            sample_count: 10,
            completed_count: 10,
            failed_count: 0,
            blocked_count: 0,
            cancelled_count: 0,
            median_end_to_end_latency_ms: latency_ms,
            first_useful_observation_count: 10,
            median_time_to_first_useful_ms: Some(latency_ms / 2),
            handoff_observation_count: 10,
            median_handoff_start_overhead_ms: Some(10),
            verified_sample_count: 10,
            verification_coverage_basis_points: 10_000,
            verified_success_rate_basis_points: Some(9_500),
            retry_observation_count: 10,
            median_retry_count: Some(0),
            recovery_observation_count: 10,
            median_recovery_duration_ms: Some(0),
            human_rescue_observation_count: 10,
            median_human_rescue_count: Some(0),
            usage_observation_count: 10,
            median_usage: None,
            cost_observation_count: u64::from(cost_minor_units.is_some()),
            median_cost: cost_minor_units.map(|minor_units| B83CostMedian {
                minor_units,
                currency: "USD".into(),
                sources: vec![B83ObservationSource::ProviderReported],
                confidence: B83ObservationConfidence::Exact,
            }),
        }
    }

    fn comparison(
        latency_delta_ms: i64,
        cost_delta_minor_units: Option<i128>,
        candidate_rate: Option<u16>,
        lead_rate: Option<u16>,
    ) -> B83PairedComparison {
        B83PairedComparison {
            arm: B83BenchmarkArm::QualifiedLowerCostWorker,
            paired_sample_count: 10,
            median_end_to_end_latency_delta_ms: latency_delta_ms,
            median_cost_delta_minor_units: cost_delta_minor_units,
            cost_currency: cost_delta_minor_units.map(|_| "USD".into()),
            cost_delta_confidence: cost_delta_minor_units.map(|_| B83ObservationConfidence::Exact),
            candidate_verification_coverage_basis_points: 10_000,
            lead_only_verification_coverage_basis_points: 10_000,
            candidate_verified_success_rate_basis_points: candidate_rate,
            lead_only_verified_success_rate_basis_points: lead_rate,
        }
    }

    fn report_with(comparison: B83PairedComparison) -> B83BenchmarkReport {
        B83BenchmarkReport {
            task_case_count: 10,
            sample_count: 20,
            arms: vec![
                arm_summary(B83BenchmarkArm::LeadOnly, 1_000, Some(1_000)),
                arm_summary(B83BenchmarkArm::QualifiedLowerCostWorker, 1_200, Some(800)),
            ],
            comparisons_to_lead_only: vec![comparison],
            automatic_selection_qualified: false,
        }
    }

    fn policy(objective: DelegationBenefitObjective) -> DelegationBenefitPolicy {
        DelegationBenefitPolicy {
            policy_version: "delegation-benefit-v1".into(),
            objective,
            minimum_improvement_basis_points: 1_000,
            minimum_paired_samples: 5,
            minimum_verification_coverage_basis_points: 9_000,
            minimum_verified_success_rate_basis_points: 9_000,
            require_exact_cost_confidence: false,
        }
    }

    #[test]
    fn measured_cost_reduction_above_the_released_threshold_delegates() {
        let decision = decide_delegation_benefit(
            &report_with(comparison(-100, Some(-200), Some(9_500), Some(9_500))),
            B83BenchmarkArm::QualifiedLowerCostWorker,
            &policy(DelegationBenefitObjective::Cost),
        );
        assert_eq!(
            decision,
            DelegationBenefitDecision::Delegate {
                arm: B83BenchmarkArm::QualifiedLowerCostWorker,
                objective: DelegationBenefitObjective::Cost,
                measured_improvement_basis_points: 2_000,
                paired_sample_count: 10,
                policy_version: "delegation-benefit-v1".into(),
            }
        );
    }

    #[test]
    fn measured_latency_reduction_is_scored_separately_from_cost() {
        let decision = decide_delegation_benefit(
            &report_with(comparison(-300, Some(500), Some(9_500), Some(9_500))),
            B83BenchmarkArm::QualifiedLowerCostWorker,
            &policy(DelegationBenefitObjective::Latency),
        );
        assert_eq!(
            decision,
            DelegationBenefitDecision::Delegate {
                arm: B83BenchmarkArm::QualifiedLowerCostWorker,
                objective: DelegationBenefitObjective::Latency,
                measured_improvement_basis_points: 3_000,
                paired_sample_count: 10,
                policy_version: "delegation-benefit-v1".into(),
            }
        );
    }

    #[test]
    fn below_threshold_unknown_or_incomparable_cost_stays_with_the_lead() {
        let below = decide_delegation_benefit(
            &report_with(comparison(-10, Some(-50), Some(9_500), Some(9_500))),
            B83BenchmarkArm::QualifiedLowerCostWorker,
            &policy(DelegationBenefitObjective::Cost),
        );
        assert_eq!(
            below,
            DelegationBenefitDecision::StayWithCurrentOption(
                DelegationBenefitBlocker::BelowThreshold
            )
        );
        let unknown = decide_delegation_benefit(
            &report_with(comparison(-100, None, Some(9_500), Some(9_500))),
            B83BenchmarkArm::QualifiedLowerCostWorker,
            &policy(DelegationBenefitObjective::Cost),
        );
        assert_eq!(
            unknown,
            DelegationBenefitDecision::StayWithCurrentOption(DelegationBenefitBlocker::NoBaseline)
        );
        let more_expensive = decide_delegation_benefit(
            &report_with(comparison(-100, Some(200), Some(9_500), Some(9_500))),
            B83BenchmarkArm::QualifiedLowerCostWorker,
            &policy(DelegationBenefitObjective::Cost),
        );
        assert_eq!(
            more_expensive,
            DelegationBenefitDecision::StayWithCurrentOption(
                DelegationBenefitBlocker::BelowThreshold
            )
        );
    }

    #[test]
    fn insufficient_samples_coverage_or_quality_regression_blocks_benefit_selection() {
        let mut low_samples = comparison(-500, Some(-500), Some(9_500), Some(9_500));
        low_samples.paired_sample_count = 2;
        assert_eq!(
            decide_delegation_benefit(
                &report_with(low_samples),
                B83BenchmarkArm::QualifiedLowerCostWorker,
                &policy(DelegationBenefitObjective::Cost),
            ),
            DelegationBenefitDecision::StayWithCurrentOption(
                DelegationBenefitBlocker::InsufficientSamples
            )
        );

        let mut low_coverage = comparison(-500, Some(-500), Some(9_500), Some(9_500));
        low_coverage.candidate_verification_coverage_basis_points = 8_000;
        assert_eq!(
            decide_delegation_benefit(
                &report_with(low_coverage),
                B83BenchmarkArm::QualifiedLowerCostWorker,
                &policy(DelegationBenefitObjective::Cost),
            ),
            DelegationBenefitDecision::StayWithCurrentOption(
                DelegationBenefitBlocker::InsufficientVerificationCoverage
            )
        );

        assert_eq!(
            decide_delegation_benefit(
                &report_with(comparison(-500, Some(-500), Some(8_000), Some(9_500))),
                B83BenchmarkArm::QualifiedLowerCostWorker,
                &policy(DelegationBenefitObjective::Cost),
            ),
            DelegationBenefitDecision::StayWithCurrentOption(
                DelegationBenefitBlocker::QualityFloorUnmet
            )
        );
        assert_eq!(
            decide_delegation_benefit(
                &report_with(comparison(-500, Some(-500), None, Some(9_500))),
                B83BenchmarkArm::QualifiedLowerCostWorker,
                &policy(DelegationBenefitObjective::Cost),
            ),
            DelegationBenefitDecision::StayWithCurrentOption(
                DelegationBenefitBlocker::QualityFloorUnmet
            )
        );
    }

    #[test]
    fn estimated_cost_cannot_satisfy_an_exact_confidence_policy() {
        let mut estimated = comparison(-100, Some(-200), Some(9_500), Some(9_500));
        estimated.cost_delta_confidence = Some(B83ObservationConfidence::Estimated);
        let mut exact_policy = policy(DelegationBenefitObjective::Cost);
        exact_policy.require_exact_cost_confidence = true;
        assert_eq!(
            decide_delegation_benefit(
                &report_with(estimated),
                B83BenchmarkArm::QualifiedLowerCostWorker,
                &exact_policy,
            ),
            DelegationBenefitDecision::StayWithCurrentOption(
                DelegationBenefitBlocker::UnqualifiedCostObservation
            )
        );
    }

    #[test]
    fn invalid_policy_missing_baseline_and_lead_only_arm_never_delegate() {
        let mut invalid = policy(DelegationBenefitObjective::Cost);
        invalid.minimum_improvement_basis_points = 10_001;
        assert_eq!(
            decide_delegation_benefit(
                &report_with(comparison(-500, Some(-500), Some(9_500), Some(9_500))),
                B83BenchmarkArm::QualifiedLowerCostWorker,
                &invalid,
            ),
            DelegationBenefitDecision::StayWithCurrentOption(
                DelegationBenefitBlocker::InvalidPolicy
            )
        );

        let mut missing = report_with(comparison(-500, Some(-500), Some(9_500), Some(9_500)));
        missing.comparisons_to_lead_only.clear();
        assert_eq!(
            decide_delegation_benefit(
                &missing,
                B83BenchmarkArm::QualifiedLowerCostWorker,
                &policy(DelegationBenefitObjective::Cost),
            ),
            DelegationBenefitDecision::StayWithCurrentOption(DelegationBenefitBlocker::NoBaseline)
        );

        assert_eq!(
            decide_delegation_benefit(
                &report_with(comparison(-500, Some(-500), Some(9_500), Some(9_500))),
                B83BenchmarkArm::LeadOnly,
                &policy(DelegationBenefitObjective::Cost),
            ),
            DelegationBenefitDecision::StayWithCurrentOption(
                DelegationBenefitBlocker::InvalidPolicy
            )
        );
    }
}
