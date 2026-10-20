//! Deterministic worker-profile ranking over already-qualified admission inputs.
//!
//! This module does not authorize or admit an Attempt. Its caller must compute
//! `eligible` from the current Task/Step/lease, adapter, Environment, Trust, and
//! budget state inside the admission boundary. The projections here are routing
//! diagnostics only.

mod performance;
pub use performance::*;

use domain_responsibility::OptimizationPreference;
use std::cmp::Ordering;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const WORKER_SELECTION_POLICY_VERSION: &str = "worker-selection-rank-v1";
const MAX_COST_OBSERVATION_AGE_SECONDS: i64 = 90 * 24 * 60 * 60;
const HIGH_CONFIDENCE_MIN_VERIFICATION_COVERAGE_BPS: u16 = 8_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerObservationConfidence {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostObservation {
    pub currency: String,
    pub minor_units: u64,
    pub source: String,
    pub confidence: WorkerObservationConfidence,
    pub sample_count: u64,
    pub verification_coverage_basis_points: u16,
    pub observed_at: String,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostLimit {
    pub currency: String,
    pub minor_units: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnforcedCostCeiling {
    pub currency: String,
    pub minor_units: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerCandidate {
    pub profile_id: String,
    /// `true` only after all hard eligibility predicates have been checked.
    pub eligible: bool,
    /// Qualified projection including handoff, execution, verification, and recovery.
    pub qualified_expected_cost: Option<CostObservation>,
    /// Provider-enforced maximum cost, when it independently enforces the exact
    /// ceiling for this candidate and request.
    pub enforced_cost_ceiling: Option<EnforcedCostCeiling>,
    /// Confidence for the performance projection. LOW observations may remain
    /// visible as diagnostics but never influence routing order.
    pub performance_confidence: WorkerObservationConfidence,
    /// Qualified verification pass rate in basis points for the Task category.
    pub verified_pass_rate_basis_points: Option<u16>,
    /// Qualified end-to-end latency including delegation and verification.
    pub qualified_end_to_end_latency_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileSelection {
    Automatic,
    Prefer(String),
    Require(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerSelectionRequest {
    pub selection: ProfileSelection,
    pub allow_preferred_fallback: bool,
    pub preference: OptimizationPreference,
    /// Optional minimum qualified verification rate, in basis points. Unknown
    /// quality cannot satisfy a requested floor.
    pub minimum_verified_pass_rate_basis_points: Option<u16>,
    /// RFC3339 time observed by the admission caller; stale observations cannot rank.
    pub now: String,
    pub max_cost: Option<CostLimit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankedWorkerCandidate {
    /// One-based position in the complete ranked candidate list.
    pub rank: usize,
    pub profile_id: String,
    /// True when this candidate matched the request's PREFER/REQUIRE selection.
    pub matched_requested_profile: bool,
    /// `None` means cost is absent or not comparable in a common currency.
    pub comparable_cost: Option<CostObservation>,
    pub performance_confidence: WorkerObservationConfidence,
    pub verified_pass_rate_basis_points: Option<u16>,
    pub qualified_end_to_end_latency_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerSelectionResult {
    pub policy_version: &'static str,
    pub ranking_rule: WorkerRankingRule,
    pub candidates: Vec<RankedWorkerCandidate>,
    /// Cost is unknown for these IDs; unknown is never encoded as zero.
    pub unknown_cost_profile_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerRankingRule {
    QualityThenLatencyThenCost,
    CostThenQualityThenLatency,
    LatencyThenQualityThenCost,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerSelectionError {
    InvalidCandidate,
    DuplicateProfileId(String),
    RequiredProfileUnavailable(String),
    PreferredProfileUnavailable(String),
    NoEligibleCandidates,
}

struct PreparedCandidate<'a> {
    candidate: &'a WorkerCandidate,
    comparable_cost: Option<&'a CostObservation>,
    preferred: bool,
}

/// Filters candidates against a requested hard monetary ceiling, then applies a
/// deterministic lexicographic order. It deliberately does not decide whether
/// delegation is beneficial; that requires released B83 thresholds and a lead-only
/// baseline. An explicit REQUIRE selection is never substituted.
pub fn rank_worker_candidates(
    candidates: &[WorkerCandidate],
    request: WorkerSelectionRequest,
) -> Result<WorkerSelectionResult, WorkerSelectionError> {
    let now = parse_timestamp(&request.now);
    if now.is_none()
        || request
            .max_cost
            .as_ref()
            .is_some_and(|cost| !valid_currency(&cost.currency))
        || request
            .minimum_verified_pass_rate_basis_points
            .is_some_and(|rate| rate > 10_000)
    {
        return Err(WorkerSelectionError::InvalidCandidate);
    }

    let mut seen = std::collections::BTreeSet::new();
    for candidate in candidates {
        if !valid_id(&candidate.profile_id)
            || candidate
                .enforced_cost_ceiling
                .as_ref()
                .is_some_and(|cost| !valid_currency(&cost.currency))
            || candidate
                .qualified_expected_cost
                .as_ref()
                .is_some_and(|cost| !valid_observation(cost))
            || candidate
                .verified_pass_rate_basis_points
                .is_some_and(|rate| rate > 10_000)
        {
            return Err(WorkerSelectionError::InvalidCandidate);
        }
        if !seen.insert(candidate.profile_id.as_str()) {
            return Err(WorkerSelectionError::DuplicateProfileId(
                candidate.profile_id.clone(),
            ));
        }
    }

    let (selected_profile, required) = match &request.selection {
        ProfileSelection::Automatic => (None, false),
        ProfileSelection::Prefer(id) if valid_id(id) => (Some(id.as_str()), false),
        ProfileSelection::Require(id) if valid_id(id) => (Some(id.as_str()), true),
        ProfileSelection::Prefer(_) | ProfileSelection::Require(_) => {
            return Err(WorkerSelectionError::InvalidCandidate);
        }
    };

    let now = now.ok_or(WorkerSelectionError::InvalidCandidate)?;
    let comparable_currency = request
        .max_cost
        .as_ref()
        .map(|cost| cost.currency.as_str())
        .or_else(|| common_currency(candidates, now));

    let mut eligible = Vec::new();
    for candidate in candidates.iter().filter(|candidate| candidate.eligible) {
        if request
            .minimum_verified_pass_rate_basis_points
            .is_some_and(|minimum| ranked_quality(candidate).is_none_or(|rate| rate < minimum))
        {
            continue;
        }
        let cost = candidate
            .qualified_expected_cost
            .as_ref()
            .filter(|cost| observation_is_current_and_qualified(cost, now))
            .filter(|cost| Some(cost.currency.as_str()) == comparable_currency);
        if let Some(ceiling) = &request.max_cost {
            // An observed or estimated cost ranks candidates but cannot enforce a hard
            // monetary ceiling. Only an exact matching provider-side ceiling can make
            // the candidate eligible under `max_cost`.
            let provider_enforced = candidate
                .enforced_cost_ceiling
                .as_ref()
                .is_some_and(|bound| {
                    bound.currency == ceiling.currency && bound.minor_units <= ceiling.minor_units
                });
            if !provider_enforced {
                continue;
            }
        }
        let is_preferred = selected_profile == Some(candidate.profile_id.as_str());
        eligible.push(PreparedCandidate {
            candidate,
            comparable_cost: cost,
            preferred: is_preferred,
        });
    }

    if required {
        let requested = selected_profile.ok_or(WorkerSelectionError::InvalidCandidate)?;
        if !eligible
            .iter()
            .any(|entry| entry.candidate.profile_id == requested)
        {
            return Err(WorkerSelectionError::RequiredProfileUnavailable(
                requested.to_owned(),
            ));
        }
        eligible.retain(|entry| entry.candidate.profile_id == requested);
    } else if let Some(preferred) = selected_profile {
        let preferred_is_eligible = eligible
            .iter()
            .any(|entry| entry.candidate.profile_id == preferred);
        if !preferred_is_eligible && !request.allow_preferred_fallback {
            return Err(WorkerSelectionError::PreferredProfileUnavailable(
                preferred.to_owned(),
            ));
        }
        if preferred_is_eligible && !request.allow_preferred_fallback {
            eligible.retain(|entry| entry.candidate.profile_id == preferred);
        }
    }

    if eligible.is_empty() {
        return Err(WorkerSelectionError::NoEligibleCandidates);
    }

    eligible.sort_by(|left, right| compare_candidates(left, right, request.preference));
    let unknown_cost_profile_ids = eligible
        .iter()
        .filter(|entry| entry.comparable_cost.is_none())
        .map(|entry| entry.candidate.profile_id.clone())
        .collect();
    let candidates = eligible
        .into_iter()
        .enumerate()
        .map(|(index, entry)| RankedWorkerCandidate {
            rank: index + 1,
            profile_id: entry.candidate.profile_id.clone(),
            matched_requested_profile: entry.preferred,
            comparable_cost: entry.comparable_cost.cloned(),
            performance_confidence: entry.candidate.performance_confidence,
            verified_pass_rate_basis_points: (entry.candidate.performance_confidence
                != WorkerObservationConfidence::Low)
                .then_some(entry.candidate.verified_pass_rate_basis_points)
                .flatten(),
            qualified_end_to_end_latency_ms: (entry.candidate.performance_confidence
                != WorkerObservationConfidence::Low)
                .then_some(entry.candidate.qualified_end_to_end_latency_ms)
                .flatten(),
        })
        .collect();

    Ok(WorkerSelectionResult {
        policy_version: WORKER_SELECTION_POLICY_VERSION,
        ranking_rule: ranking_rule(request.preference),
        candidates,
        unknown_cost_profile_ids,
    })
}

fn ranking_rule(preference: OptimizationPreference) -> WorkerRankingRule {
    match preference {
        OptimizationPreference::QualityFirst | OptimizationPreference::Balanced => {
            WorkerRankingRule::QualityThenLatencyThenCost
        }
        OptimizationPreference::CostFirst => WorkerRankingRule::CostThenQualityThenLatency,
        OptimizationPreference::LatencyFirst => WorkerRankingRule::LatencyThenQualityThenCost,
    }
}

fn compare_candidates(
    left: &PreparedCandidate<'_>,
    right: &PreparedCandidate<'_>,
    preference: OptimizationPreference,
) -> Ordering {
    let preferred = right.preferred.cmp(&left.preferred);
    if preferred != Ordering::Equal {
        return preferred;
    }
    let left_candidate = left.candidate;
    let right_candidate = right.candidate;
    let quality = compare_optional_desc(
        ranked_quality(left_candidate),
        ranked_quality(right_candidate),
    );
    let latency = compare_optional_asc(
        ranked_latency(left_candidate),
        ranked_latency(right_candidate),
    );
    let cost = compare_cost(left.comparable_cost, right.comparable_cost);

    let objective_order = match preference {
        OptimizationPreference::QualityFirst | OptimizationPreference::Balanced => {
            quality.then(latency).then(cost)
        }
        OptimizationPreference::CostFirst => cost.then(quality).then(latency),
        OptimizationPreference::LatencyFirst => latency.then(quality).then(cost),
    };
    objective_order.then_with(|| left_candidate.profile_id.cmp(&right_candidate.profile_id))
}

fn ranked_quality(candidate: &WorkerCandidate) -> Option<u16> {
    (candidate.performance_confidence != WorkerObservationConfidence::Low)
        .then_some(candidate.verified_pass_rate_basis_points)
        .flatten()
}

fn ranked_latency(candidate: &WorkerCandidate) -> Option<u64> {
    (candidate.performance_confidence != WorkerObservationConfidence::Low)
        .then_some(candidate.qualified_end_to_end_latency_ms)
        .flatten()
}

fn compare_optional_desc<T: Ord>(left: Option<T>, right: Option<T>) -> Ordering {
    right.cmp(&left)
}

fn compare_optional_asc<T: Ord>(left: Option<T>, right: Option<T>) -> Ordering {
    left.cmp(&right)
}

fn compare_cost(left: Option<&CostObservation>, right: Option<&CostObservation>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.minor_units.cmp(&right.minor_units),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn common_currency(candidates: &[WorkerCandidate], now: OffsetDateTime) -> Option<&str> {
    let mut currencies = candidates
        .iter()
        .filter(|candidate| candidate.eligible)
        .filter_map(|candidate| candidate.qualified_expected_cost.as_ref())
        .filter(|observation| observation_is_current_and_qualified(observation, now))
        .map(|cost| cost.currency.as_str());
    let first = currencies.next()?;
    currencies
        .all(|currency| currency == first)
        .then_some(first)
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn valid_currency(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn valid_observation(value: &CostObservation) -> bool {
    if !valid_currency(&value.currency)
        || value.source.trim().is_empty()
        || value.source.len() > 256
        || value.source.chars().any(char::is_control)
        || value.sample_count == 0
        || value.verification_coverage_basis_points > 10_000
    {
        return false;
    }
    let Some(observed_at) = parse_timestamp(&value.observed_at) else {
        return false;
    };
    if value.expires_at.as_deref().is_some_and(|expires_at| {
        parse_timestamp(expires_at).is_none_or(|expires_at| expires_at <= observed_at)
    }) {
        return false;
    }
    match value.confidence {
        WorkerObservationConfidence::Low => true,
        WorkerObservationConfidence::Medium => {
            value.sample_count >= 3 && value.verification_coverage_basis_points == 10_000
        }
        WorkerObservationConfidence::High => {
            value.sample_count >= 10
                && value.verification_coverage_basis_points
                    >= HIGH_CONFIDENCE_MIN_VERIFICATION_COVERAGE_BPS
        }
    }
}

fn observation_is_current_and_qualified(value: &CostObservation, now: OffsetDateTime) -> bool {
    if value.confidence == WorkerObservationConfidence::Low {
        return false;
    }
    let Some(observed_at) = parse_timestamp(&value.observed_at) else {
        return false;
    };
    let age_seconds = (now - observed_at).whole_seconds();
    if !(0..=MAX_COST_OBSERVATION_AGE_SECONDS).contains(&age_seconds) {
        return false;
    }
    value
        .expires_at
        .as_deref()
        .and_then(parse_timestamp)
        .is_none_or(|expires_at| expires_at > now)
}

fn parse_timestamp(value: &str) -> Option<OffsetDateTime> {
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return None;
    }
    OffsetDateTime::parse(value, &Rfc3339).ok()
}
