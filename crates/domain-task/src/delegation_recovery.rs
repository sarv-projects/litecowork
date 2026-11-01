//! Bounded escalation decisions after an individual delegated Attempt settles.
//!
//! A decision here can only recommend a new immutable Attempt. The execution
//! coordinator must reserve budget, recheck grants/Trust/Environment/lease state, and
//! atomically commit the next Attempt before starting a provider.

const MAX_WORKER_ATTEMPTS: u8 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EscalationPolicy {
    /// Total child Attempts including the initial worker Attempt.
    pub max_worker_attempts: u8,
    /// Ordered alternatives; the initial Attempt consumes one slot in the maximum.
    pub fallback_profile_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscalationAttemptOutcome {
    RecoverableVerificationFailure,
    Passed,
    PermanentFailure,
    Ambiguous,
    BudgetExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscalationCandidateReadiness {
    Qualified,
    Ineligible,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EscalationCandidate {
    pub profile_id: String,
    pub readiness: EscalationCandidateReadiness,
    pub checked_task_version: u64,
    pub checked_task_spec_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EscalationRequest {
    pub outcome: EscalationAttemptOutcome,
    /// Number of child Attempts already admitted for this Step, including the
    /// just-settled Attempt. The first Attempt counts as one.
    pub admitted_attempt_count: u8,
    pub attempted_profile_ids: Vec<String>,
    pub current_task_version: u64,
    pub task_spec_revision_at_start: u64,
    pub current_task_spec_revision: u64,
    pub task_attempt_ceiling: u8,
    pub active_sibling_children: u8,
    pub effective_max_concurrency: u8,
    pub effects_reconciled: bool,
    pub step_recoverable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscalationBlocker {
    InvalidPolicy,
    InvalidRequest,
    VerificationPassed,
    FailureNotRecoverable,
    AmbiguousEffect,
    BudgetExhausted,
    EffectsUnreconciled,
    StepNotRecoverable,
    TaskSpecChanged,
    AttemptLimitReached,
    ConcurrencyLimitReached,
    NoEligibleFallback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EscalationDecision {
    NoEscalation,
    Blocked(EscalationBlocker),
    RecommendNewAttempt {
        profile_id: String,
        attempt_number: u8,
        expected_task_version: u64,
        task_spec_revision: u64,
        expected_active_sibling_children: u8,
    },
}

/// Decide whether a settled worker failure can be retried with the next eligible
/// profile. The returned attempt number is proposed only; it is not persisted here.
pub fn decide_delegation_escalation(
    policy: &EscalationPolicy,
    request: &EscalationRequest,
    candidates: &[EscalationCandidate],
) -> EscalationDecision {
    if !valid_policy(policy) {
        return EscalationDecision::Blocked(EscalationBlocker::InvalidPolicy);
    }
    if request.admitted_attempt_count == 0
        || request.current_task_version == 0
        || request.task_spec_revision_at_start == 0
        || request.current_task_spec_revision == 0
        || request.task_attempt_ceiling == 0
        || request.task_attempt_ceiling > MAX_WORKER_ATTEMPTS
        || request.effective_max_concurrency == 0
        || request.effective_max_concurrency > 8
        || request.attempted_profile_ids.len() != usize::from(request.admitted_attempt_count)
        || !unique_ids(&request.attempted_profile_ids)
        || request
            .attempted_profile_ids
            .iter()
            .any(|profile_id| !valid_id(profile_id))
    {
        return EscalationDecision::Blocked(EscalationBlocker::InvalidRequest);
    }
    if request.outcome == EscalationAttemptOutcome::Passed {
        return EscalationDecision::NoEscalation;
    }
    match request.outcome {
        EscalationAttemptOutcome::RecoverableVerificationFailure => {}
        EscalationAttemptOutcome::PermanentFailure => {
            return EscalationDecision::Blocked(EscalationBlocker::FailureNotRecoverable);
        }
        EscalationAttemptOutcome::Ambiguous => {
            return EscalationDecision::Blocked(EscalationBlocker::AmbiguousEffect);
        }
        EscalationAttemptOutcome::BudgetExhausted => {
            return EscalationDecision::Blocked(EscalationBlocker::BudgetExhausted);
        }
        EscalationAttemptOutcome::Passed => unreachable!(),
    }
    if !request.effects_reconciled {
        return EscalationDecision::Blocked(EscalationBlocker::EffectsUnreconciled);
    }
    if !request.step_recoverable {
        return EscalationDecision::Blocked(EscalationBlocker::StepNotRecoverable);
    }
    if request.task_spec_revision_at_start != request.current_task_spec_revision {
        return EscalationDecision::Blocked(EscalationBlocker::TaskSpecChanged);
    }
    let max_attempts = policy.max_worker_attempts.min(request.task_attempt_ceiling);
    if request.admitted_attempt_count >= max_attempts {
        return EscalationDecision::Blocked(EscalationBlocker::AttemptLimitReached);
    }
    if request.active_sibling_children >= request.effective_max_concurrency {
        return EscalationDecision::Blocked(EscalationBlocker::ConcurrencyLimitReached);
    }

    let unique_candidates = candidates
        .iter()
        .map(|candidate| candidate.profile_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if unique_candidates.len() != candidates.len()
        || candidates
            .iter()
            .any(|candidate| !valid_id(&candidate.profile_id))
    {
        return EscalationDecision::Blocked(EscalationBlocker::InvalidRequest);
    }
    let next = policy.fallback_profile_ids.iter().find_map(|profile_id| {
        if request
            .attempted_profile_ids
            .iter()
            .any(|attempted| attempted == profile_id)
        {
            return None;
        }
        candidates.iter().find(|candidate| {
            candidate.profile_id == *profile_id
                && candidate.readiness == EscalationCandidateReadiness::Qualified
                && candidate.checked_task_version == request.current_task_version
                && candidate.checked_task_spec_revision == request.current_task_spec_revision
        })
    });
    let Some(candidate) = next else {
        return EscalationDecision::Blocked(EscalationBlocker::NoEligibleFallback);
    };
    EscalationDecision::RecommendNewAttempt {
        profile_id: candidate.profile_id.clone(),
        attempt_number: request.admitted_attempt_count + 1,
        expected_task_version: request.current_task_version,
        task_spec_revision: request.current_task_spec_revision,
        expected_active_sibling_children: request.active_sibling_children,
    }
}

fn valid_policy(policy: &EscalationPolicy) -> bool {
    (1..=MAX_WORKER_ATTEMPTS).contains(&policy.max_worker_attempts)
        && policy.fallback_profile_ids.len() < policy.max_worker_attempts as usize
        && unique_ids(&policy.fallback_profile_ids)
        && policy
            .fallback_profile_ids
            .iter()
            .all(|profile_id| valid_id(profile_id))
}

fn unique_ids(values: &[String]) -> bool {
    values
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == values.len()
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> EscalationPolicy {
        EscalationPolicy {
            max_worker_attempts: 3,
            fallback_profile_ids: vec!["profile-fast".into(), "profile-review".into()],
        }
    }

    fn request() -> EscalationRequest {
        EscalationRequest {
            outcome: EscalationAttemptOutcome::RecoverableVerificationFailure,
            admitted_attempt_count: 1,
            attempted_profile_ids: vec!["profile-initial".into()],
            current_task_version: 7,
            task_spec_revision_at_start: 3,
            current_task_spec_revision: 3,
            task_attempt_ceiling: 3,
            active_sibling_children: 0,
            effective_max_concurrency: 2,
            effects_reconciled: true,
            step_recoverable: true,
        }
    }

    fn candidate(profile_id: &str, readiness: EscalationCandidateReadiness) -> EscalationCandidate {
        EscalationCandidate {
            profile_id: profile_id.into(),
            readiness,
            checked_task_version: 7,
            checked_task_spec_revision: 3,
        }
    }

    #[test]
    fn recoverable_failure_recommends_next_fresh_eligible_profile_in_policy_order() {
        assert_eq!(
            decide_delegation_escalation(
                &policy(),
                &request(),
                &[
                    candidate("profile-review", EscalationCandidateReadiness::Qualified),
                    candidate("profile-fast", EscalationCandidateReadiness::Qualified),
                ],
            ),
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
    fn passed_permanent_ambiguous_and_budget_exhausted_outcomes_do_not_retry() {
        let mut request = request();
        request.outcome = EscalationAttemptOutcome::Passed;
        assert_eq!(
            decide_delegation_escalation(&policy(), &request, &[]),
            EscalationDecision::NoEscalation
        );
        request.outcome = EscalationAttemptOutcome::PermanentFailure;
        assert_eq!(
            decide_delegation_escalation(&policy(), &request, &[]),
            EscalationDecision::Blocked(EscalationBlocker::FailureNotRecoverable)
        );
        request.outcome = EscalationAttemptOutcome::Ambiguous;
        assert_eq!(
            decide_delegation_escalation(&policy(), &request, &[]),
            EscalationDecision::Blocked(EscalationBlocker::AmbiguousEffect)
        );
        request.outcome = EscalationAttemptOutcome::BudgetExhausted;
        assert_eq!(
            decide_delegation_escalation(&policy(), &request, &[]),
            EscalationDecision::Blocked(EscalationBlocker::BudgetExhausted)
        );
    }

    #[test]
    fn retry_waits_for_effect_reconciliation_and_step_recovery() {
        let mut request = request();
        request.effects_reconciled = false;
        assert_eq!(
            decide_delegation_escalation(&policy(), &request, &[]),
            EscalationDecision::Blocked(EscalationBlocker::EffectsUnreconciled)
        );
        request.effects_reconciled = true;
        request.step_recoverable = false;
        assert_eq!(
            decide_delegation_escalation(&policy(), &request, &[]),
            EscalationDecision::Blocked(EscalationBlocker::StepNotRecoverable)
        );
    }

    #[test]
    fn changed_task_spec_invalidates_automatic_escalation() {
        let mut request = request();
        request.current_task_spec_revision = 4;
        assert_eq!(
            decide_delegation_escalation(&policy(), &request, &[]),
            EscalationDecision::Blocked(EscalationBlocker::TaskSpecChanged)
        );
    }

    #[test]
    fn attempt_ceiling_concurrency_and_unqualified_profiles_block_escalation() {
        let mut request = request();
        request.admitted_attempt_count = 3;
        request.attempted_profile_ids = vec![
            "profile-initial".into(),
            "profile-fast".into(),
            "profile-review".into(),
        ];
        assert_eq!(
            decide_delegation_escalation(&policy(), &request, &[]),
            EscalationDecision::Blocked(EscalationBlocker::AttemptLimitReached)
        );
        request.admitted_attempt_count = 1;
        request.attempted_profile_ids.truncate(1);
        request.active_sibling_children = 2;
        assert_eq!(
            decide_delegation_escalation(&policy(), &request, &[]),
            EscalationDecision::Blocked(EscalationBlocker::ConcurrencyLimitReached)
        );
        request.active_sibling_children = 0;
        assert_eq!(
            decide_delegation_escalation(
                &policy(),
                &request,
                &[candidate(
                    "profile-fast",
                    EscalationCandidateReadiness::Unknown
                )]
            ),
            EscalationDecision::Blocked(EscalationBlocker::NoEligibleFallback)
        );
    }

    #[test]
    fn incomplete_attempt_profile_history_blocks_escalation() {
        let mut missing_history = request();
        missing_history.attempted_profile_ids.clear();
        assert_eq!(
            decide_delegation_escalation(
                &policy(),
                &missing_history,
                &[candidate(
                    "profile-fast",
                    EscalationCandidateReadiness::Qualified
                )]
            ),
            EscalationDecision::Blocked(EscalationBlocker::InvalidRequest)
        );

        let mut incomplete_history = request();
        incomplete_history
            .attempted_profile_ids
            .push("profile-fast".into());
        assert_eq!(
            decide_delegation_escalation(
                &policy(),
                &incomplete_history,
                &[candidate(
                    "profile-review",
                    EscalationCandidateReadiness::Qualified
                )]
            ),
            EscalationDecision::Blocked(EscalationBlocker::InvalidRequest)
        );
    }

    #[test]
    fn malformed_policy_and_stale_or_duplicate_candidates_fail_closed() {
        let mut invalid = policy();
        invalid.fallback_profile_ids.push("duplicate".into());
        invalid.fallback_profile_ids.push("duplicate".into());
        invalid.max_worker_attempts = 4;
        assert_eq!(
            decide_delegation_escalation(&invalid, &request(), &[]),
            EscalationDecision::Blocked(EscalationBlocker::InvalidPolicy)
        );
        let mut stale = candidate("profile-fast", EscalationCandidateReadiness::Qualified);
        stale.checked_task_version = 6;
        assert_eq!(
            decide_delegation_escalation(&policy(), &request(), &[stale]),
            EscalationDecision::Blocked(EscalationBlocker::NoEligibleFallback)
        );
        let same = candidate("profile-fast", EscalationCandidateReadiness::Qualified);
        assert_eq!(
            decide_delegation_escalation(&policy(), &request(), &[same.clone(), same]),
            EscalationDecision::Blocked(EscalationBlocker::InvalidRequest)
        );
    }
}
