use super::WorkerObservationConfidence;
use std::collections::BTreeSet;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const PERFORMANCE_WINDOW_SECONDS: i64 = 90 * 24 * 60 * 60;
const HIGH_CONFIDENCE_COVERAGE_BPS: u16 = 8_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerPerformanceQuery {
    pub profile_id: String,
    /// Missing TaskSpec category is normalized to `OTHER` by the caller.
    pub task_category: Option<String>,
    pub now: String,
    /// False when the owner-scoped source projection could not prove that all
    /// required observation families were loaded for this window.
    pub source_observations_complete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerPerformanceAttemptState {
    Running,
    Completed,
    Failed,
    Cancelled,
    Abandoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerVerificationOutcome {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkerUsageSource {
    AgentReported,
    ProviderReported,
    HostMeasured,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerUsageConfidence {
    Exact,
    Estimated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerCostObservation {
    /// Amount in the exact declared `unit`; no currency conversion is attempted.
    pub amount: u64,
    pub unit: String,
    pub currency: String,
    pub source: WorkerUsageSource,
    pub confidence: WorkerUsageConfidence,
}

/// Sanitized, owner-scoped attempt projection. The storage query determines whether a
/// later Attempt replaced this Attempt and whether an owner UserRequest blocked it.
/// It contains no prompt, output, native transcript, or provider-private data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerPerformanceAttempt {
    pub attempt_id: String,
    pub task_id: String,
    pub step_id: String,
    pub profile_id: String,
    pub task_category: String,
    pub state: WorkerPerformanceAttemptState,
    pub admitted_at: String,
    pub terminal_at: Option<String>,
    pub terminal_verification: Option<WorkerVerificationOutcome>,
    pub required_observations_complete: bool,
    pub superseded_by_later_attempt: bool,
    pub blocked_by_owner_request: bool,
    /// At most one pre-aggregated cost per Attempt. Missing cost remains unknown.
    pub cost: Option<WorkerCostObservation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerCostMedian {
    pub amount: u64,
    pub unit: String,
    pub currency: String,
    pub sources: Vec<WorkerUsageSource>,
    pub confidence: WorkerUsageConfidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerPerformanceProjection {
    pub profile_id: String,
    pub task_category: String,
    /// Completed and failed terminal child Attempts; cancelled/abandoned excluded.
    pub sample_count: u64,
    pub verified_attempt_count: u64,
    pub verification_observation_coverage_basis_points: u16,
    pub verifier_pass_rate_basis_points: Option<u16>,
    pub median_latency_ms: Option<u64>,
    pub median_observed_cost: Option<WorkerCostMedian>,
    pub retry_rate_basis_points: Option<u16>,
    pub human_intervention_rate_basis_points: Option<u16>,
    pub confidence: WorkerObservationConfidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerPerformanceError {
    InvalidQuery,
    InvalidSample,
    ScopeMismatch,
    DuplicateAttemptId(String),
    CandidateProfileMismatch,
}

/// Aggregate an already owner-scoped, category-scoped source projection. This function
/// does not query storage or authorize admission. Missing/incompatible observations stay
/// unknown; they are never rewritten to zero.
pub fn aggregate_worker_performance(
    query: &WorkerPerformanceQuery,
    attempts: &[WorkerPerformanceAttempt],
) -> Result<WorkerPerformanceProjection, WorkerPerformanceError> {
    let Some(now) = parse_timestamp(&query.now) else {
        return Err(WorkerPerformanceError::InvalidQuery);
    };
    let task_category = query.task_category.as_deref().unwrap_or("OTHER");
    if !valid_performance_id(&query.profile_id)
        || !valid_performance_category(task_category)
        || attempts.len() > 100_000
    {
        return Err(WorkerPerformanceError::InvalidQuery);
    }
    let cutoff = now - time::Duration::seconds(PERFORMANCE_WINDOW_SECONDS);
    let mut seen = BTreeSet::new();
    let mut samples = Vec::new();
    for attempt in attempts {
        if !valid_performance_id(&attempt.attempt_id)
            || !valid_performance_id(&attempt.task_id)
            || !valid_performance_id(&attempt.step_id)
            || !valid_performance_id(&attempt.profile_id)
            || !valid_performance_category(&attempt.task_category)
        {
            return Err(WorkerPerformanceError::InvalidSample);
        }
        if !seen.insert(attempt.attempt_id.as_str()) {
            return Err(WorkerPerformanceError::DuplicateAttemptId(
                attempt.attempt_id.clone(),
            ));
        }
        if attempt.profile_id != query.profile_id || attempt.task_category != task_category {
            return Err(WorkerPerformanceError::ScopeMismatch);
        }
        if !matches!(
            attempt.state,
            WorkerPerformanceAttemptState::Completed | WorkerPerformanceAttemptState::Failed
        ) {
            continue;
        }
        let Some(terminal_at) = attempt.terminal_at.as_deref().and_then(parse_timestamp) else {
            return Err(WorkerPerformanceError::InvalidSample);
        };
        let Some(admitted_at) = parse_timestamp(&attempt.admitted_at) else {
            return Err(WorkerPerformanceError::InvalidSample);
        };
        if terminal_at < admitted_at {
            return Err(WorkerPerformanceError::InvalidSample);
        }
        if terminal_at > now || terminal_at < cutoff {
            continue;
        }
        if let Some(cost) = &attempt.cost {
            if !valid_performance_id(&cost.unit) || !valid_performance_currency(&cost.currency) {
                return Err(WorkerPerformanceError::InvalidSample);
            }
        }
        samples.push((attempt, (terminal_at - admitted_at).whole_milliseconds()));
    }

    let sample_count = samples.len() as u64;
    let verified_attempts = samples
        .iter()
        .filter(|(attempt, _)| attempt.terminal_verification.is_some())
        .count() as u64;
    let complete_verifier_observations = samples
        .iter()
        .filter(|(attempt, _)| {
            attempt.terminal_verification.is_some() && attempt.required_observations_complete
        })
        .count() as u64;
    let pass_count = samples
        .iter()
        .filter(|(attempt, _)| {
            attempt.terminal_verification == Some(WorkerVerificationOutcome::Passed)
        })
        .count() as u64;
    let coverage = rate_basis_points(complete_verifier_observations, sample_count);

    let mut latencies = samples
        .iter()
        .map(|(_, latency)| {
            u64::try_from(*latency).map_err(|_| WorkerPerformanceError::InvalidSample)
        })
        .collect::<Result<Vec<_>, _>>()?;
    latencies.sort_unstable();
    let median_latency_ms = median(&latencies);
    let retry_count = samples
        .iter()
        .filter(|(attempt, _)| attempt.superseded_by_later_attempt)
        .count() as u64;
    let human_intervention_count = samples
        .iter()
        .filter(|(attempt, _)| attempt.blocked_by_owner_request)
        .count() as u64;
    let median_observed_cost = aggregate_cost_median(&samples);
    let confidence = if !query.source_observations_complete || complete_verifier_observations < 3 {
        WorkerObservationConfidence::Low
    } else if sample_count >= 10 && coverage >= HIGH_CONFIDENCE_COVERAGE_BPS {
        WorkerObservationConfidence::High
    } else {
        WorkerObservationConfidence::Medium
    };

    Ok(WorkerPerformanceProjection {
        profile_id: query.profile_id.clone(),
        task_category: task_category.to_owned(),
        sample_count,
        verified_attempt_count: verified_attempts,
        verification_observation_coverage_basis_points: coverage,
        verifier_pass_rate_basis_points: (verified_attempts > 0)
            .then(|| rate_basis_points(pass_count, verified_attempts)),
        median_latency_ms,
        median_observed_cost,
        retry_rate_basis_points: (sample_count > 0)
            .then(|| rate_basis_points(retry_count, sample_count)),
        human_intervention_rate_basis_points: (sample_count > 0)
            .then(|| rate_basis_points(human_intervention_count, sample_count)),
        confidence,
    })
}

impl WorkerPerformanceProjection {
    /// Attach only confidence-qualified metrics to the existing deterministic ranker.
    pub fn apply_to_candidate(
        &self,
        candidate: &mut super::WorkerCandidate,
    ) -> Result<(), WorkerPerformanceError> {
        if candidate.profile_id != self.profile_id {
            return Err(WorkerPerformanceError::CandidateProfileMismatch);
        }
        candidate.performance_confidence = self.confidence;
        if self.confidence == WorkerObservationConfidence::Low {
            candidate.verified_pass_rate_basis_points = None;
            candidate.qualified_end_to_end_latency_ms = None;
        } else {
            candidate.verified_pass_rate_basis_points = self.verifier_pass_rate_basis_points;
            candidate.qualified_end_to_end_latency_ms = self.median_latency_ms;
        }
        Ok(())
    }
}

fn aggregate_cost_median(
    samples: &[(&WorkerPerformanceAttempt, i128)],
) -> Option<WorkerCostMedian> {
    if samples.is_empty() {
        return None;
    }
    let costs = samples
        .iter()
        .map(|(attempt, _)| attempt.cost.as_ref())
        .collect::<Option<Vec<_>>>()?;
    let first = costs.first()?;
    if costs
        .iter()
        .any(|cost| cost.unit != first.unit || cost.currency != first.currency)
    {
        return None;
    }
    let mut amounts = costs.iter().map(|cost| cost.amount).collect::<Vec<_>>();
    amounts.sort_unstable();
    let sources = costs
        .iter()
        .map(|cost| cost.source)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let confidence = if costs
        .iter()
        .any(|cost| cost.confidence == WorkerUsageConfidence::Estimated)
    {
        WorkerUsageConfidence::Estimated
    } else {
        WorkerUsageConfidence::Exact
    };
    Some(WorkerCostMedian {
        amount: median(&amounts)?,
        unit: first.unit.clone(),
        currency: first.currency.clone(),
        sources,
        confidence,
    })
}

fn median(values: &[u64]) -> Option<u64> {
    match values.len() {
        0 => None,
        count if count % 2 == 1 => Some(values[count / 2]),
        count => {
            Some(((u128::from(values[count / 2 - 1]) + u128::from(values[count / 2])) / 2) as u64)
        }
    }
}

fn rate_basis_points(numerator: u64, denominator: u64) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((u128::from(numerator) * 10_000) / u128::from(denominator)) as u16
}

fn valid_performance_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn valid_performance_category(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn valid_performance_currency(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn parse_timestamp(value: &str) -> Option<OffsetDateTime> {
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return None;
    }
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> WorkerPerformanceQuery {
        WorkerPerformanceQuery {
            profile_id: "profile-a".into(),
            task_category: Some("CODE".into()),
            now: "2026-10-17T12:00:00Z".into(),
            source_observations_complete: true,
        }
    }

    fn attempt(id: &str, seconds_ago: i64) -> WorkerPerformanceAttempt {
        let terminal_at = timestamp_seconds_ago(seconds_ago);
        let admitted_at = timestamp_seconds_ago(seconds_ago + 60);
        WorkerPerformanceAttempt {
            attempt_id: id.into(),
            task_id: format!("task-{id}"),
            step_id: format!("step-{id}"),
            profile_id: "profile-a".into(),
            task_category: "CODE".into(),
            state: WorkerPerformanceAttemptState::Completed,
            admitted_at,
            terminal_at: Some(terminal_at),
            terminal_verification: Some(WorkerVerificationOutcome::Passed),
            required_observations_complete: true,
            superseded_by_later_attempt: false,
            blocked_by_owner_request: false,
            cost: Some(WorkerCostObservation {
                amount: 100,
                unit: "minor_unit".into(),
                currency: "USD".into(),
                source: WorkerUsageSource::ProviderReported,
                confidence: WorkerUsageConfidence::Exact,
            }),
        }
    }

    fn candidate(profile_id: &str) -> super::super::WorkerCandidate {
        super::super::WorkerCandidate {
            profile_id: profile_id.into(),
            eligible: true,
            qualified_expected_cost: None,
            enforced_cost_ceiling: None,
            performance_confidence: WorkerObservationConfidence::Low,
            verified_pass_rate_basis_points: None,
            qualified_end_to_end_latency_ms: None,
        }
    }

    fn timestamp_seconds_ago(seconds_ago: i64) -> String {
        const NINETY_DAYS: i64 = 90 * 24 * 60 * 60;
        let (date, elapsed) = if seconds_ago >= NINETY_DAYS {
            ("2026-07-19", 12 * 60 * 60 - (seconds_ago - NINETY_DAYS))
        } else {
            ("2026-10-17", 12 * 60 * 60 - seconds_ago)
        };
        let hour = elapsed / 3_600;
        let minute = (elapsed % 3_600) / 60;
        let second = elapsed % 60;
        format!("{date}T{hour:02}:{minute:02}:{second:02}Z")
    }

    #[test]
    fn rolling_window_includes_the_exact_ninety_day_boundary_and_excludes_older_and_future_samples()
    {
        // Implementation and assertions are intentionally written against the
        // intended public contract before its first domain implementation.
        let at_boundary = attempt("boundary", 90 * 24 * 60 * 60);
        let too_old = attempt("old", 90 * 24 * 60 * 60 + 1);
        let mut future = attempt("future", 0);
        future.terminal_at = Some("2026-10-17T12:00:01Z".into());

        let projection = aggregate_worker_performance(&query(), &[at_boundary, too_old, future])
            .expect("valid source projections aggregate");

        assert_eq!(projection.sample_count, 1);
        assert_eq!(projection.median_latency_ms, Some(60_000));
    }

    #[test]
    fn duplicate_attempts_are_rejected_and_nonterminal_attempts_are_excluded() {
        let mut running = attempt("running", 60);
        running.state = WorkerPerformanceAttemptState::Running;
        running.terminal_at = None;
        running.terminal_verification = None;
        let projection = aggregate_worker_performance(&query(), &[running.clone()])
            .expect("nonterminal projections are ignored");
        assert_eq!(projection.sample_count, 0);
        assert_eq!(projection.retry_rate_basis_points, None);

        assert_eq!(
            aggregate_worker_performance(&query(), &[running.clone(), running]),
            Err(WorkerPerformanceError::DuplicateAttemptId("running".into()))
        );
    }

    #[test]
    fn retry_and_owner_intervention_rates_use_non_cancelled_terminal_attempts_as_denominator() {
        let mut first = attempt("first", 120);
        first.superseded_by_later_attempt = true;
        first.blocked_by_owner_request = true;
        let mut cancelled = attempt("cancelled", 60);
        cancelled.state = WorkerPerformanceAttemptState::Cancelled;
        let abandoned = WorkerPerformanceAttempt {
            state: WorkerPerformanceAttemptState::Abandoned,
            ..attempt("abandoned", 30)
        };
        let second = attempt("second", 0);

        let projection =
            aggregate_worker_performance(&query(), &[first, cancelled, abandoned, second])
                .expect("valid samples aggregate");

        assert_eq!(projection.sample_count, 2);
        assert_eq!(projection.retry_rate_basis_points, Some(5_000));
        assert_eq!(projection.human_intervention_rate_basis_points, Some(5_000));
    }

    #[test]
    fn verifier_pass_rate_and_cost_median_preserve_terminal_and_usage_semantics() {
        let mut pass = attempt("pass", 120);
        pass.cost.as_mut().unwrap().amount = 100;
        pass.cost.as_mut().unwrap().confidence = WorkerUsageConfidence::Exact;
        let mut fail = attempt("fail", 0);
        fail.terminal_verification = Some(WorkerVerificationOutcome::Failed);
        fail.cost.as_mut().unwrap().amount = 300;
        fail.cost.as_mut().unwrap().confidence = WorkerUsageConfidence::Estimated;

        let projection =
            aggregate_worker_performance(&query(), &[pass, fail]).expect("valid samples aggregate");

        assert_eq!(projection.verifier_pass_rate_basis_points, Some(5_000));
        assert_eq!(
            projection.median_observed_cost.as_ref().unwrap().amount,
            200
        );
        assert_eq!(
            projection.median_observed_cost.as_ref().unwrap().confidence,
            WorkerUsageConfidence::Estimated
        );
        assert_eq!(projection.confidence, WorkerObservationConfidence::Low);
    }

    #[test]
    fn failed_terminal_attempts_are_samples_but_only_terminal_verifications_enter_pass_rate() {
        let mut failed = attempt("failed", 120);
        failed.state = WorkerPerformanceAttemptState::Failed;
        failed.terminal_verification = None;
        failed.cost.as_mut().unwrap().source = WorkerUsageSource::AgentReported;
        let mut completed = attempt("completed", 60);
        completed.terminal_verification = Some(WorkerVerificationOutcome::Failed);
        completed.cost.as_mut().unwrap().source = WorkerUsageSource::HostMeasured;

        let projection = aggregate_worker_performance(&query(), &[failed, completed]).unwrap();

        assert_eq!(projection.sample_count, 2);
        assert_eq!(projection.verified_attempt_count, 1);
        assert_eq!(projection.verifier_pass_rate_basis_points, Some(0));
        assert_eq!(
            projection.median_observed_cost.as_ref().unwrap().sources,
            vec![
                WorkerUsageSource::AgentReported,
                WorkerUsageSource::HostMeasured
            ]
        );
    }

    #[test]
    fn only_matching_profile_projection_feeds_qualified_metrics_to_existing_ranker() {
        use domain_responsibility::OptimizationPreference;

        let samples = (0..3)
            .map(|i| attempt(&format!("qualified-{i}"), i * 60))
            .collect::<Vec<_>>();
        let projection = aggregate_worker_performance(&query(), &samples).unwrap();
        let mut qualified = candidate("profile-a");
        projection
            .apply_to_candidate(&mut qualified)
            .expect("matching profile attaches projection");
        let mut weaker = candidate("profile-b");
        weaker.performance_confidence = WorkerObservationConfidence::Medium;
        weaker.verified_pass_rate_basis_points = Some(6_000);

        let ranked = super::super::rank_worker_candidates(
            &[weaker, qualified],
            super::super::WorkerSelectionRequest {
                selection: super::super::ProfileSelection::Automatic,
                allow_preferred_fallback: true,
                preference: OptimizationPreference::QualityFirst,
                max_cost: None,
                minimum_verified_pass_rate_basis_points: None,
                now: query().now,
            },
        )
        .expect("existing ranker consumes qualified projection");

        assert_eq!(ranked.candidates[0].profile_id, "profile-a");
        assert_eq!(
            ranked.candidates[0].verified_pass_rate_basis_points,
            Some(10_000)
        );
        assert_eq!(
            projection.apply_to_candidate(&mut candidate("different-profile")),
            Err(WorkerPerformanceError::CandidateProfileMismatch)
        );
    }

    #[test]
    fn incompatible_or_incomplete_cost_observations_remain_unknown() {
        let mut usd = attempt("usd", 120);
        usd.cost.as_mut().unwrap().amount = 100;
        let mut eur = attempt("eur", 60);
        eur.cost.as_mut().unwrap().currency = "EUR".into();
        assert_eq!(
            aggregate_worker_performance(&query(), &[usd, eur])
                .unwrap()
                .median_observed_cost,
            None
        );

        let mut known = attempt("known", 120);
        known.cost = None;
        let mut missing = attempt("missing", 60);
        missing.cost = None;
        assert_eq!(
            aggregate_worker_performance(&query(), &[known, missing])
                .unwrap()
                .median_observed_cost,
            None
        );
    }

    #[test]
    fn confidence_thresholds_require_three_complete_verifications_and_eighty_percent_for_high() {
        let two = (0..2)
            .map(|i| attempt(&format!("two-{i}"), i * 60))
            .collect::<Vec<_>>();
        assert_eq!(
            aggregate_worker_performance(&query(), &two)
                .unwrap()
                .confidence,
            WorkerObservationConfidence::Low
        );

        let three = (0..3)
            .map(|i| attempt(&format!("three-{i}"), i * 60))
            .collect::<Vec<_>>();
        assert_eq!(
            aggregate_worker_performance(&query(), &three)
                .unwrap()
                .confidence,
            WorkerObservationConfidence::Medium
        );

        let ten = (0..10)
            .map(|i| {
                let mut item = attempt(&format!("ten-{i}"), i * 60);
                if i >= 8 {
                    item.terminal_verification = None;
                    item.required_observations_complete = false;
                }
                item
            })
            .collect::<Vec<_>>();
        assert_eq!(
            aggregate_worker_performance(&query(), &ten)
                .unwrap()
                .confidence,
            WorkerObservationConfidence::High
        );

        let below_high_coverage = (0..10)
            .map(|i| {
                let mut item = attempt(&format!("below-high-{i}"), i * 60);
                if i >= 7 {
                    item.terminal_verification = None;
                    item.required_observations_complete = false;
                }
                item
            })
            .collect::<Vec<_>>();
        assert_eq!(
            aggregate_worker_performance(&query(), &below_high_coverage)
                .unwrap()
                .confidence,
            WorkerObservationConfidence::Medium
        );

        let mut incomplete_query = query();
        incomplete_query.source_observations_complete = false;
        assert_eq!(
            aggregate_worker_performance(&incomplete_query, &three)
                .unwrap()
                .confidence,
            WorkerObservationConfidence::Low
        );
    }
}
