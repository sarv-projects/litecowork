//! Local trigger-host settlement decisions.
//!
//! The trigger host owns exactly one Automation trigger at a time. It reads the
//! persisted cursor, asks this module which slots are due, and receives the
//! exact decisions it may commit. This module never reads a clock, never
//! persists, and never creates a Task: the storage adapter commits the
//! decision it is handed.
//!
//! The rules implemented here come from
//! [AUTOMATION.md](../../docs/AUTOMATION.md):
//!
//! - a misfire is recorded as a bounded `SKIPPED` occurrence and is never
//!   silently dropped;
//! - `max_concurrent_occurrences` and the overlap policy are enforced against
//!   nonterminal occurrence Tasks;
//! - a queued or cancel-first slot is not consumed. The cursor keeps naming it
//!   so a later pass can still materialize it.

use crate::{
    DomainError, MisfireReason, OccurrenceIntent, OverlapPolicy, ScheduleTrigger,
    SettlementWindow,
};

/// Why a due slot produced no ordinary Task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotOutcomeReason {
    /// The slot elapsed without a host claim and the misfire policy skipped it.
    Missed(MisfireReason),
    /// The concurrency limit was reached and the overlap policy skipped the slot.
    OverlapSkipped,
    /// The concurrency limit was reached and the overlap policy queued the slot.
    OverlapQueued,
    /// The concurrency limit was reached and the previous run must be cancelled
    /// before the new one may start.
    OverlapCancelOld,
}

/// The decision the host may commit for one due slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostSlotDecision {
    /// Materialize one occurrence and one ordinary Task for this slot.
    Materialize {
        instant: i64,
        /// The covered interval when several missed slots coalesced into one run.
        covered_from: Option<i64>,
        covered_to: Option<i64>,
        /// How many slots this single run covers.
        covered_count: u32,
    },
    /// Record one bounded `SKIPPED` occurrence and create no Task.
    Skip { instant: i64, reason: SlotOutcomeReason },
    /// Keep the slot due. A prior run must settle before this one may start.
    Defer { instant: i64, reason: SlotOutcomeReason },
}

impl HostSlotDecision {
    /// The slot instant this decision refers to.
    pub fn instant(&self) -> i64 {
        match self {
            Self::Materialize { instant, .. } | Self::Skip { instant, .. } | Self::Defer { instant, .. } => {
                *instant
            }
        }
    }

    /// True when the decision consumes the slot.
    pub fn consumes_slot(&self) -> bool {
        !matches!(self, Self::Defer { .. })
    }
}

/// The concurrency facts one settlement pass needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostConcurrencyFacts {
    /// The number of occurrence Tasks that are not yet terminal.
    pub nonterminal_occurrences: u32,
    /// The Automation's `max_concurrent_occurrences`.
    pub max_concurrent_occurrences: u32,
    /// The Automation's overlap policy.
    pub overlap_policy: OverlapPolicy,
}

/// The durable result of one settlement pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSettlementPlan {
    /// The per-slot decisions in chronological order.
    pub decisions: Vec<HostSlotDecision>,
    /// The next slot the host must observe, or `None` when the rule is exhausted.
    pub next_scheduled_at: Option<i64>,
    /// True when the host observed the schedule after one or more slots elapsed.
    pub misfired: bool,
}

impl HostSettlementPlan {
    /// The slots that must become ordinary Tasks.
    pub fn materialized(&self) -> impl Iterator<Item = i64> + '_ {
        self.decisions
            .iter()
            .filter_map(|decision| match decision {
                HostSlotDecision::Materialize { instant, .. } => Some(*instant),
                _ => None,
            })
    }

    /// The slots that must be recorded as bounded skipped occurrences.
    pub fn skipped(&self) -> impl Iterator<Item = (i64, SlotOutcomeReason)> + '_ {
        self.decisions
            .iter()
            .filter_map(|decision| match decision {
                HostSlotDecision::Skip { instant, reason } => Some((*instant, *reason)),
                _ => None,
            })
    }

    /// The slots that stay due because a prior run must settle first.
    pub fn deferred(&self) -> impl Iterator<Item = (i64, SlotOutcomeReason)> + '_ {
        self.decisions
            .iter()
            .filter_map(|decision| match decision {
                HostSlotDecision::Defer { instant, reason } => Some((*instant, *reason)),
                _ => None,
            })
    }
}

/// Decides how a persisted cursor changes for one settlement pass.
///
/// The plan is a pure function of the persisted cursor, the host's clock, and
/// the persisted concurrency facts. The same inputs always produce the same
/// decisions, so a repeated pass after a crash cannot invent a second Task for
/// an already materialized slot: the caller derives the occurrence identity
/// from the slot instant and the storage layer enforces its uniqueness.
pub fn plan_host_settlement(
    schedule: &ScheduleTrigger,
    cursor_next_scheduled_at: Option<i64>,
    window: SettlementWindow,
    concurrency: HostConcurrencyFacts,
) -> Result<HostSettlementPlan, DomainError> {
    if concurrency.max_concurrent_occurrences == 0 {
        return Err(DomainError::InvalidDefinition);
    }
    let settlement = schedule.plan_settlement(cursor_next_scheduled_at, window)?;
    let mut decisions = Vec::with_capacity(settlement.intents.len());
    let mut nonterminal = concurrency.nonterminal_occurrences;
    let mut deferred_at: Option<i64> = None;
    for intent in settlement.intents {
        let (instant, covered_from, covered_to, covered_count) = match intent {
            OccurrenceIntent::Run {
                instant,
                covered_from,
                covered_to,
                covered_count,
            } => (instant, covered_from, covered_to, covered_count),
            OccurrenceIntent::Skip { instant, reason } => {
                decisions.push(HostSlotDecision::Skip {
                    instant,
                    reason: SlotOutcomeReason::Missed(reason),
                });
                continue;
            }
        };
        if nonterminal < concurrency.max_concurrent_occurrences {
            nonterminal += 1;
            decisions.push(HostSlotDecision::Materialize {
                instant,
                covered_from,
                covered_to,
                covered_count,
            });
            continue;
        }
        // The concurrency limit is reached. The overlap policy decides whether
        // the slot is skipped, queued, or waits for a cooperative cancellation.
        match concurrency.overlap_policy {
            OverlapPolicy::Allow => {
                nonterminal += 1;
                decisions.push(HostSlotDecision::Materialize {
                    instant,
                    covered_from,
                    covered_to,
                    covered_count,
                });
            }
            OverlapPolicy::Skip => {
                decisions.push(HostSlotDecision::Skip {
                    instant,
                    reason: SlotOutcomeReason::OverlapSkipped,
                });
            }
            OverlapPolicy::Queue => {
                deferred_at = Some(instant);
                decisions.push(HostSlotDecision::Defer {
                    instant,
                    reason: SlotOutcomeReason::OverlapQueued,
                });
                break;
            }
            OverlapPolicy::CancelOld => {
                deferred_at = Some(instant);
                decisions.push(HostSlotDecision::Defer {
                    instant,
                    reason: SlotOutcomeReason::OverlapCancelOld,
                });
                break;
            }
        }
    }
    // A deferred slot was not materialized. The cursor keeps naming it so the
    // next pass can still create its occurrence once a prior run settles.
    let next_scheduled_at = match deferred_at {
        Some(instant) => Some(instant),
        None => settlement.next_scheduled_at,
    };
    Ok(HostSettlementPlan {
        decisions,
        next_scheduled_at,
        misfired: settlement.misfired,
    })
}

/// The bounded trigger kinds the local host can materialize.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OccurrenceTriggerKind {
    /// The owner-issued one-shot `ManualTrigger`.
    Manual,
    /// A recurring schedule slot.
    Schedule,
}

impl OccurrenceTriggerKind {
    /// The persisted token.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "MANUAL",
            Self::Schedule => "SCHEDULE",
        }
    }

    /// Parses a persisted token.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "MANUAL" => Some(Self::Manual),
            "SCHEDULE" => Some(Self::Schedule),
            _ => None,
        }
    }
}
