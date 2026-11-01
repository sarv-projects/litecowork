//! Action batches, deadline preflight, and warm Environment holds.
//!
//! `docs/ENVIRONMENTS.md` requires that a batch is not a transaction, that partial
//! success is reconciled rather than rolled back, that no deadline is presented as a
//! hard real-time guarantee, that prewarming never makes a model call, and that a warm
//! Environment carries no authority.
//!
//! This module is the decision layer for those rules. It plans a batch, tracks the
//! outcome of each member, and governs warm holds. It performs no provider call and
//! it grants nothing: a warm hold is a performance hint, never a permission.

/// The effect class of a batch member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectClass {
    /// Reads state and produces no effect.
    ReadOnly,
    /// Writes inside the Environment's own writable root.
    LocalWrite,
    /// Sends something outside the Environment.
    ExternalSend,
    /// Cannot be undone once it has been applied.
    Irreversible,
}

/// What to do when a member fails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbortPolicy {
    /// Stop dispatching further members after the first failure.
    StopOnFailure,
    /// Continue with members that have no unresolved dependency.
    ContinueIndependent,
}

/// One member of an action batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionBatchMember {
    pub member_id: String,
    pub effect_class: EffectClass,
    /// The precondition digest that must hold when the member is dispatched.
    pub precondition_digest: String,
    /// An optimistic duration estimate used for the deadline preflight.
    pub estimated_duration_ms: i64,
    /// Members that must have been applied before this one may run.
    pub depends_on: Vec<String>,
}

/// A bounded set of actions dispatched as one unit of intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionBatch {
    pub batch_id: String,
    pub members: Vec<ActionBatchMember>,
    /// The absolute deadline for the whole batch.
    pub deadline_at_ms: i64,
    pub abort_policy: AbortPolicy,
}

/// Why a batch member was not admitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchRefusal {
    TooManyMembers,
    DuplicateMember,
    UnknownDependency,
    DependencyNotAdmitted,
    /// The remaining deadline budget cannot cover the member's estimate.
    InsufficientDeadlineBudget,
    /// The member's precondition no longer holds.
    StalePrecondition,
    /// The batch deadline has already passed.
    DeadlinePassed,
}

/// A planned batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchPlan {
    pub batch_id: String,
    pub admitted: Vec<String>,
    pub refused: Vec<(String, BatchRefusal)>,
    pub estimated_duration_ms: i64,
}

/// Plan a batch against the current deadline and precondition state.
///
/// The preflight is a *budget* check, not a guarantee. LiteCowork never presents a
/// deadline as a hard real-time promise, so the plan records the remaining budget and
/// refuses members that cannot fit, rather than promising a completion time.
pub fn plan_batch(
    batch: &ActionBatch,
    now_ms: i64,
    max_members: usize,
    current_preconditions: &[(String, String)],
) -> BatchPlan {
    let mut admitted = Vec::new();
    let mut refused = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    if batch.members.len() > max_members {
        return BatchPlan {
            batch_id: batch.batch_id.clone(),
            admitted,
            refused: batch
                .members
                .iter()
                .map(|member| (member.member_id.clone(), BatchRefusal::TooManyMembers))
                .collect(),
            estimated_duration_ms: 0,
        };
    }
    if now_ms >= batch.deadline_at_ms {
        return BatchPlan {
            batch_id: batch.batch_id.clone(),
            admitted,
            refused: batch
                .members
                .iter()
                .map(|member| (member.member_id.clone(), BatchRefusal::DeadlinePassed))
                .collect(),
            estimated_duration_ms: 0,
        };
    }

    let mut remaining = batch.deadline_at_ms - now_ms;
    let mut estimated_total = 0_i64;
    for member in &batch.members {
        if !seen.insert(member.member_id.clone()) {
            refused.push((member.member_id.clone(), BatchRefusal::DuplicateMember));
            continue;
        }
        if member.depends_on.iter().any(|dependency| {
            !batch
                .members
                .iter()
                .any(|candidate| candidate.member_id == *dependency)
        }) {
            refused.push((
                member.member_id.clone(),
                BatchRefusal::UnknownDependency,
            ));
            continue;
        }
        if member
            .depends_on
            .iter()
            .any(|dependency| !admitted.contains(dependency))
        {
            refused.push((
                member.member_id.clone(),
                BatchRefusal::DependencyNotAdmitted,
            ));
            continue;
        }
        let precondition = current_preconditions
            .iter()
            .find(|(member_id, _)| member_id == &member.member_id)
            .map(|(_, digest)| digest.as_str());
        if precondition != Some(member.precondition_digest.as_str()) {
            refused.push((
                member.member_id.clone(),
                BatchRefusal::StalePrecondition,
            ));
            continue;
        }
        if member.estimated_duration_ms > remaining {
            refused.push((
                member.member_id.clone(),
                BatchRefusal::InsufficientDeadlineBudget,
            ));
            continue;
        }
        remaining -= member.estimated_duration_ms;
        estimated_total += member.estimated_duration_ms;
        admitted.push(member.member_id.clone());
    }

    BatchPlan {
        batch_id: batch.batch_id.clone(),
        admitted,
        refused,
        estimated_duration_ms: estimated_total,
    }
}

/// The outcome of one dispatched member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberOutcome {
    Applied,
    Failed,
    /// The provider response was lost or ambiguous.
    Ambiguous,
}

/// How a batch ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchOutcome {
    Completed {
        applied: Vec<String>,
    },
    /// Some members applied and some failed. Applied effects are not rolled back.
    PartiallyApplied {
        applied: Vec<String>,
        failed: Vec<String>,
    },
    /// The provider response was lost for at least one member. The batch is not
    /// complete and must not be retried blindly.
    Ambiguous {
        applied: Vec<String>,
        ambiguous: Vec<String>,
    },
    Aborted {
        applied: Vec<String>,
        failed: Vec<String>,
        not_dispatched: Vec<String>,
    },
}

impl BatchOutcome {
    /// The members whose effect is known to have been applied.
    pub fn applied(&self) -> &[String] {
        match self {
            Self::Completed { applied }
            | Self::PartiallyApplied { applied, .. }
            | Self::Ambiguous { applied, .. }
            | Self::Aborted { applied, .. } => applied,
        }
    }

    /// Whether every admitted member applied.
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }
}

/// Reconcile member outcomes into the batch outcome.
///
/// A batch is not a transaction. Once a member's effect has been applied it stays
/// applied; the batch reports the partial result so that the caller reconciles it
/// instead of assuming a rollback.
pub fn reconcile_batch(
    batch: &ActionBatch,
    plan: &BatchPlan,
    outcomes: &[(String, MemberOutcome)],
) -> BatchOutcome {
    let mut applied = Vec::new();
    let mut failed = Vec::new();
    let mut ambiguous = Vec::new();
    let mut not_dispatched = Vec::new();

    for member_id in &plan.admitted {
        match outcomes
            .iter()
            .find(|(id, _)| id == member_id)
            .map(|(_, outcome)| *outcome)
        {
            Some(MemberOutcome::Applied) => applied.push(member_id.clone()),
            Some(MemberOutcome::Failed) => failed.push(member_id.clone()),
            Some(MemberOutcome::Ambiguous) => ambiguous.push(member_id.clone()),
            None => not_dispatched.push(member_id.clone()),
        }
    }

    if !ambiguous.is_empty() {
        return BatchOutcome::Ambiguous { applied, ambiguous };
    }
    if !failed.is_empty() {
        return match batch.abort_policy {
            AbortPolicy::StopOnFailure => BatchOutcome::Aborted {
                applied,
                failed,
                not_dispatched,
            },
            AbortPolicy::ContinueIndependent => BatchOutcome::PartiallyApplied { applied, failed },
        };
    }
    if !not_dispatched.is_empty() {
        return BatchOutcome::Aborted {
            applied,
            failed,
            not_dispatched,
        };
    }
    BatchOutcome::Completed { applied }
}

/// Evidence that a warm Environment was prepared without a model call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrewarmEvidence {
    /// The number of model calls the prewarm performed. It must be zero.
    pub model_calls: u32,
    /// The digest of the provider configuration the prewarm used.
    pub provider_config_digest: String,
}

/// A warm Environment hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WarmHold {
    pub hold_id: String,
    pub environment_id: String,
    pub created_at_ms: i64,
    /// Time to live of the hold.
    pub ttl_ms: i64,
    pub prewarm: PrewarmEvidence,
}

/// Why a warm hold was not usable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WarmHoldRefusal {
    /// The hold outlived its time to live.
    TtlExpired,
    /// The provider configuration changed since the prewarm.
    ProviderConfigDrift,
    /// The prewarm performed a model call, which is not a prewarm.
    PrewarmPerformedModelCall,
    /// The prewarm configuration digest is empty.
    PrewarmNotRecorded,
    /// A warm hold carries no authority and cannot stand in for a grant or a lease.
    WarmHoldCarriesNoAuthority,
}

/// Decide whether a warm hold may be reused.
pub fn admit_warm_hold(
    hold: &WarmHold,
    now_ms: i64,
    current_provider_config_digest: &str,
) -> Result<(), WarmHoldRefusal> {
    if hold.prewarm.model_calls != 0 {
        return Err(WarmHoldRefusal::PrewarmPerformedModelCall);
    }
    if hold.prewarm.provider_config_digest.is_empty() {
        return Err(WarmHoldRefusal::PrewarmNotRecorded);
    }
    if hold.ttl_ms <= 0 || now_ms >= hold.created_at_ms + hold.ttl_ms {
        return Err(WarmHoldRefusal::TtlExpired);
    }
    if hold.prewarm.provider_config_digest != current_provider_config_digest {
        return Err(WarmHoldRefusal::ProviderConfigDrift);
    }
    Ok(())
}

/// The host footprint that one warm Environment reserves.
pub const PRESSURE_BYTES_PER_HOLD: u64 = 64 * 1024 * 1024;

/// Evict warm holds under host pressure.
///
/// Unusable holds (expired or drifted) are dropped first. While the retained
/// footprint exceeds the host limit, the *oldest* usable hold is dropped, so the most
/// recently prepared Environment survives.
pub fn evict_under_pressure(
    holds: &mut Vec<WarmHold>,
    now_ms: i64,
    pressure_limit_bytes: u64,
    current_provider_config_digest: &str,
) -> Vec<String> {
    let mut evicted = Vec::new();
    let mut usable = Vec::with_capacity(holds.len());
    for hold in holds.drain(..) {
        if admit_warm_hold(&hold, now_ms, current_provider_config_digest).is_ok() {
            usable.push(hold);
        } else {
            evicted.push(hold.hold_id);
        }
    }
    usable.sort_by_key(|hold| (hold.created_at_ms, hold.hold_id.clone()));
    while usable.len() as u64 * PRESSURE_BYTES_PER_HOLD > pressure_limit_bytes {
        if usable.is_empty() {
            break;
        }
        let hold = usable.remove(0);
        evicted.push(hold.hold_id);
    }
    *holds = usable;
    evicted
}

/// A warm hold can never satisfy an authority requirement.
///
/// Reuse, freshness, and configuration agreement make a warm Environment *usable*.
/// They never make it *permitted*: a warm hold carries no capability grant, no
/// ExecutionLease, no SecretLease, and no Effect authority, so it can never stand in
/// for one.
pub fn warm_hold_satisfies_authority(_hold: &WarmHold) -> Result<(), WarmHoldRefusal> {
    Err(WarmHoldRefusal::WarmHoldCarriesNoAuthority)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(id: &str, class: EffectClass, duration: i64) -> ActionBatchMember {
        ActionBatchMember {
            member_id: id.to_owned(),
            effect_class: class,
            precondition_digest: format!("sha256:{id}"),
            estimated_duration_ms: duration,
            depends_on: Vec::new(),
        }
    }

    fn batch() -> ActionBatch {
        ActionBatch {
            batch_id: "batch-1".to_owned(),
            members: vec![
                member("m1", EffectClass::LocalWrite, 100),
                member("m2", EffectClass::ExternalSend, 200),
                member("m3", EffectClass::ExternalSend, 200),
            ],
            deadline_at_ms: 10_000,
            abort_policy: AbortPolicy::StopOnFailure,
        }
    }

    fn preconditions(batch: &ActionBatch) -> Vec<(String, String)> {
        batch
            .members
            .iter()
            .map(|member| {
                (
                    member.member_id.clone(),
                    member.precondition_digest.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn a_batch_plans_within_its_deadline_budget() {
        let batch = batch();
        let plan = plan_batch(&batch, 1_000, 16, &preconditions(&batch));
        assert_eq!(plan.admitted, vec!["m1", "m2", "m3"]);
        assert!(plan.refused.is_empty());
        assert_eq!(plan.estimated_duration_ms, 500);
    }

    #[test]
    fn a_member_that_does_not_fit_the_deadline_budget_is_refused() {
        let mut batch = batch();
        batch.members[2].estimated_duration_ms = 100_000;
        let plan = plan_batch(&batch, 1_000, 16, &preconditions(&batch));
        assert_eq!(plan.admitted, vec!["m1", "m2"]);
        assert_eq!(
            plan.refused,
            vec![(
                "m3".to_owned(),
                BatchRefusal::InsufficientDeadlineBudget
            )]
        );
    }

    #[test]
    fn a_passed_deadline_refuses_the_whole_batch() {
        let batch = batch();
        let plan = plan_batch(&batch, 10_000, 16, &preconditions(&batch));
        assert!(plan.admitted.is_empty());
        assert_eq!(plan.refused.len(), 3);
        assert!(plan
            .refused
            .iter()
            .all(|(_, reason)| *reason == BatchRefusal::DeadlinePassed));
    }

    #[test]
    fn a_stale_precondition_refuses_the_member() {
        let batch = batch();
        let mut current = preconditions(&batch);
        current[1].1 = "sha256:stale".to_owned();
        let plan = plan_batch(&batch, 1_000, 16, &current);
        assert_eq!(plan.admitted, vec!["m1", "m3"]);
        assert_eq!(
            plan.refused,
            vec![(
                "m2".to_owned(),
                BatchRefusal::StalePrecondition
            )]
        );
    }

    #[test]
    fn a_dependency_that_was_not_admitted_refuses_its_dependent_member() {
        let mut batch = batch();
        batch.members[2].depends_on = vec!["m2".to_owned()];
        let mut current = preconditions(&batch);
        current[1].1 = "sha256:stale".to_owned();
        let plan = plan_batch(&batch, 1_000, 16, &current);
        assert_eq!(plan.admitted, vec!["m1".to_owned()]);
        assert_eq!(
            plan.refused,
            vec![
                ("m2".to_owned(), BatchRefusal::StalePrecondition),
                ("m3".to_owned(), BatchRefusal::DependencyNotAdmitted),
            ]
        );
    }

    #[test]
    fn a_batch_is_not_a_transaction_and_partial_success_is_reconciled() {
        let batch = batch();
        let plan = plan_batch(&batch, 1_000, 16, &preconditions(&batch));
        let outcome = reconcile_batch(
            &batch,
            &plan,
            &[
                ("m1".to_owned(), MemberOutcome::Applied),
                ("m2".to_owned(), MemberOutcome::Applied),
                ("m3".to_owned(), MemberOutcome::Failed),
            ],
        );
        assert_eq!(
            outcome,
            BatchOutcome::Aborted {
                applied: vec!["m1".to_owned(), "m2".to_owned()],
                failed: vec!["m3".to_owned()],
                not_dispatched: Vec::new(),
            }
        );
        assert!(!outcome.is_complete());
        assert_eq!(outcome.applied().len(), 2);
    }

    #[test]
    fn an_ambiguous_member_result_is_never_treated_as_complete() {
        let batch = batch();
        let plan = plan_batch(&batch, 1_000, 16, &preconditions(&batch));
        let outcome = reconcile_batch(
            &batch,
            &plan,
            &[
                ("m1".to_owned(), MemberOutcome::Applied),
                ("m2".to_owned(), MemberOutcome::Ambiguous),
            ],
        );
        assert_eq!(
            outcome,
            BatchOutcome::Ambiguous {
                applied: vec!["m1".to_owned()],
                ambiguous: vec!["m2".to_owned()],
            }
        );
        assert!(!outcome.is_complete());
    }

    #[test]
    fn a_continue_independent_policy_reports_partial_success() {
        let mut batch = batch();
        batch.abort_policy = AbortPolicy::ContinueIndependent;
        let plan = plan_batch(&batch, 1_000, 16, &preconditions(&batch));
        let outcome = reconcile_batch(
            &batch,
            &plan,
            &[
                ("m1".to_owned(), MemberOutcome::Applied),
                ("m2".to_owned(), MemberOutcome::Failed),
                ("m3".to_owned(), MemberOutcome::Applied),
            ],
        );
        assert_eq!(
            outcome,
            BatchOutcome::PartiallyApplied {
                applied: vec!["m1".to_owned(), "m3".to_owned()],
                failed: vec!["m2".to_owned()],
            }
        );
    }

    #[test]
    fn a_prewarm_that_called_a_model_is_not_a_prewarm() {
        let hold = WarmHold {
            hold_id: "hold-1".to_owned(),
            environment_id: "env-1".to_owned(),
            created_at_ms: 1_000,
            ttl_ms: 60_000,
            prewarm: PrewarmEvidence {
                model_calls: 1,
                provider_config_digest: "sha256:config".to_owned(),
            },
        };
        assert_eq!(
            admit_warm_hold(&hold, 1_500, "sha256:config").unwrap_err(),
            WarmHoldRefusal::PrewarmPerformedModelCall
        );
    }

    #[test]
    fn a_warm_hold_expires_and_detects_provider_configuration_drift() {
        let hold = WarmHold {
            hold_id: "hold-1".to_owned(),
            environment_id: "env-1".to_owned(),
            created_at_ms: 1_000,
            ttl_ms: 60_000,
            prewarm: PrewarmEvidence {
                model_calls: 0,
                provider_config_digest: "sha256:config".to_owned(),
            },
        };
        assert_eq!(admit_warm_hold(&hold, 1_500, "sha256:config"), Ok(()));
        assert_eq!(
            admit_warm_hold(&hold, 61_000, "sha256:config").unwrap_err(),
            WarmHoldRefusal::TtlExpired
        );
        assert_eq!(
            admit_warm_hold(&hold, 1_500, "sha256:other").unwrap_err(),
            WarmHoldRefusal::ProviderConfigDrift
        );
    }

    #[test]
    fn pressure_eviction_drops_the_oldest_hold_first() {
        let hold = |id: &str, created: i64| WarmHold {
            hold_id: id.to_owned(),
            environment_id: format!("env-{id}"),
            created_at_ms: created,
            ttl_ms: 60_000,
            prewarm: PrewarmEvidence {
                model_calls: 0,
                provider_config_digest: "sha256:config".to_owned(),
            },
        };
        let mut holds = vec![hold("older", 1_000), hold("newer", 2_000)];
        let evicted = evict_under_pressure(
            &mut holds,
            3_000,
            PRESSURE_BYTES_PER_HOLD,
            "sha256:config",
        );
        assert_eq!(evicted, vec!["older".to_owned()]);
        assert_eq!(holds.len(), 1);
        assert_eq!(holds[0].hold_id, "newer");
    }

    #[test]
    fn pressure_eviction_drops_an_unusable_hold_before_any_usable_one() {
        let hold = |id: &str, created: i64, ttl: i64| WarmHold {
            hold_id: id.to_owned(),
            environment_id: format!("env-{id}"),
            created_at_ms: created,
            ttl_ms: ttl,
            prewarm: PrewarmEvidence {
                model_calls: 0,
                provider_config_digest: "sha256:config".to_owned(),
            },
        };
        let mut holds = vec![hold("expired", 1_000, 10), hold("fresh", 2_000, 60_000)];
        let evicted = evict_under_pressure(
            &mut holds,
            3_000,
            PRESSURE_BYTES_PER_HOLD * 4,
            "sha256:config",
        );
        assert_eq!(evicted, vec!["expired".to_owned()]);
        assert_eq!(holds.len(), 1);
        assert_eq!(holds[0].hold_id, "fresh");
    }

    #[test]
    fn a_warm_hold_carries_no_authority() {
        // The warm-hold type has no grant, lease, or effect field at all: a warm
        // Environment is a performance hint and can never stand in for authority.
        let hold = WarmHold {
            hold_id: "hold-1".to_owned(),
            environment_id: "env-1".to_owned(),
            created_at_ms: 1_000,
            ttl_ms: 60_000,
            prewarm: PrewarmEvidence {
                model_calls: 0,
                provider_config_digest: "sha256:config".to_owned(),
            },
        };
        assert_eq!(admit_warm_hold(&hold, 1_500, "sha256:config"), Ok(()));
        assert_eq!(
            warm_hold_satisfies_authority(&hold).unwrap_err(),
            WarmHoldRefusal::WarmHoldCarriesNoAuthority
        );
    }
}
