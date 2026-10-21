//! Unit tests for the local trigger-host settlement decisions.

use super::*;
use crate::{
    AmbiguousLocalTime, MisfirePolicy, NonexistentLocalTime, OccurrenceIdentity, OverlapPolicy,
    RecurrenceFormat,
};

fn utc(text: &str) -> i64 {
    crate::parse_instant(text).expect("test timestamps are valid RFC 3339 instants")
}

fn daily_cron(expression: &str, policy: MisfirePolicy) -> ScheduleTrigger {
    ScheduleTrigger::parse(
        RecurrenceFormat::Cron5,
        "UTC",
        expression,
        policy,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        None,
        None,
    )
    .expect("the test schedule is valid")
}

fn concurrency(
    nonterminal: u32,
    max: u32,
    overlap_policy: OverlapPolicy,
) -> HostConcurrencyFacts {
    HostConcurrencyFacts {
        nonterminal_occurrences: nonterminal,
        max_concurrent_occurrences: max,
        overlap_policy,
    }
}

#[test]
fn a_due_slot_is_materialized_and_the_cursor_moves_forward() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let first = utc("2026-03-01T09:00:00Z");
    let now = utc("2026-03-01T09:00:30Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(first),
        SettlementWindow { now, grace_ms: 60_000 },
        concurrency(0, 1, OverlapPolicy::Skip),
    )
    .expect("the settlement plan is valid");
    assert_eq!(plan.materialized().collect::<Vec<_>>(), vec![first]);
    assert_eq!(plan.skipped().count(), 0);
    assert_eq!(plan.deferred().count(), 0);
    assert!(!plan.misfired);
    assert_eq!(plan.next_scheduled_at, Some(utc("2026-03-02T09:00:00Z")));
}

#[test]
fn a_slot_that_is_not_yet_due_produces_no_decision() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let next = utc("2026-03-01T09:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(next),
        SettlementWindow {
            now: utc("2026-03-01T08:59:59Z"),
            grace_ms: 60_000,
        },
        concurrency(0, 1, OverlapPolicy::Skip),
    )
    .expect("the settlement plan is valid");
    assert!(plan.decisions.is_empty());
    assert_eq!(plan.next_scheduled_at, Some(next));
}

#[test]
fn a_misfire_is_recorded_as_a_skip_and_never_dropped() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let first = utc("2026-03-01T09:00:00Z");
    let second = utc("2026-03-02T09:00:00Z");
    let now = utc("2026-03-02T12:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(first),
        SettlementWindow { now, grace_ms: 60_000 },
        concurrency(0, 1, OverlapPolicy::Skip),
    )
    .expect("the settlement plan is valid");
    let skipped = plan.skipped().collect::<Vec<_>>();
    assert_eq!(skipped.len(), 2);
    assert_eq!(skipped[0].0, first);
    assert_eq!(skipped[1].0, second);
    assert_eq!(
        skipped[0].1,
        SlotOutcomeReason::Missed(MisfireReason::MissedSlot)
    );
    assert!(plan.misfired);
    // A skipped slot is consumed, so the cursor moves past it.
    assert_eq!(plan.next_scheduled_at, Some(utc("2026-03-03T09:00:00Z")));
}

#[test]
fn catch_up_bounded_records_the_excess_slots_as_one_skip() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::CatchUpBounded { max_occurrences: 2 });
    let first = utc("2026-03-01T09:00:00Z");
    let now = utc("2026-03-05T12:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(first),
        SettlementWindow { now, grace_ms: 60_000 },
        concurrency(0, 8, OverlapPolicy::Allow),
    )
    .expect("the settlement plan is valid");
    assert_eq!(plan.materialized().count(), 2);
    let skipped = plan.skipped().collect::<Vec<_>>();
    assert_eq!(skipped.len(), 1);
    assert_eq!(
        skipped[0].1,
        SlotOutcomeReason::Missed(MisfireReason::ExcessCatchUp)
    );
    assert!(plan.misfired);
}

#[test]
fn run_once_when_available_coalesces_the_missed_interval() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::RunOnceWhenAvailable);
    let first = utc("2026-03-01T09:00:00Z");
    let last = utc("2026-03-03T09:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(first),
        SettlementWindow {
            now: utc("2026-03-03T12:00:00Z"),
            grace_ms: 60_000,
        },
        concurrency(0, 1, OverlapPolicy::Skip),
    )
    .expect("the settlement plan is valid");
    let materialized = plan.materialized().collect::<Vec<_>>();
    assert_eq!(materialized, vec![last]);
    assert_eq!(plan.skipped().count(), 0);
}

#[test]
fn the_overlap_skip_policy_records_a_skipped_occurrence() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let first = utc("2026-03-01T09:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(first),
        SettlementWindow {
            now: utc("2026-03-01T09:00:10Z"),
            grace_ms: 60_000,
        },
        concurrency(1, 1, OverlapPolicy::Skip),
    )
    .expect("the settlement plan is valid");
    assert_eq!(plan.materialized().count(), 0);
    let skipped = plan.skipped().collect::<Vec<_>>();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].1, SlotOutcomeReason::OverlapSkipped);
}

#[test]
fn the_overlap_queue_policy_keeps_the_slot_due() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let first = utc("2026-03-01T09:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(first),
        SettlementWindow {
            now: utc("2026-03-01T09:00:10Z"),
            grace_ms: 60_000,
        },
        concurrency(1, 1, OverlapPolicy::Queue),
    )
    .expect("the settlement plan is valid");
    assert_eq!(plan.materialized().count(), 0);
    assert_eq!(plan.skipped().count(), 0);
    let deferred = plan.deferred().collect::<Vec<_>>();
    assert_eq!(deferred.len(), 1);
    assert_eq!(deferred[0].1, SlotOutcomeReason::OverlapQueued);
    // The queued slot was not consumed, so the cursor keeps naming it.
    assert_eq!(plan.next_scheduled_at, Some(first));
}

#[test]
fn the_cancel_old_policy_defers_until_the_previous_run_settles() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let first = utc("2026-03-01T09:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(first),
        SettlementWindow {
            now: utc("2026-03-01T09:00:10Z"),
            grace_ms: 60_000,
        },
        concurrency(2, 1, OverlapPolicy::CancelOld),
    )
    .expect("the settlement plan is valid");
    let deferred = plan.deferred().collect::<Vec<_>>();
    assert_eq!(deferred.len(), 1);
    assert_eq!(deferred[0].1, SlotOutcomeReason::OverlapCancelOld);
    assert_eq!(plan.next_scheduled_at, Some(first));
}

#[test]
fn the_allow_policy_materializes_beyond_the_concurrency_limit() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let first = utc("2026-03-01T09:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(first),
        SettlementWindow {
            now: utc("2026-03-01T09:00:10Z"),
            grace_ms: 60_000,
        },
        concurrency(5, 1, OverlapPolicy::Allow),
    )
    .expect("the settlement plan is valid");
    assert_eq!(plan.materialized().count(), 1);
}

#[test]
fn slots_materialized_in_one_pass_count_towards_the_concurrency_limit() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let first = utc("2026-03-01T09:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(first),
        SettlementWindow {
            now: utc("2026-03-03T12:00:00Z"),
            // A wide grace window keeps the three elapsed slots on time, so the
            // concurrency limit is what bounds this pass.
            grace_ms: 5 * 24 * 60 * 60 * 1000,
        },
        concurrency(0, 2, OverlapPolicy::Skip),
    )
    .expect("the settlement plan is valid");
    // Two slots fit inside the limit; the third is skipped by the overlap rule.
    assert_eq!(plan.materialized().count(), 2);
    let skipped = plan.skipped().collect::<Vec<_>>();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].1, SlotOutcomeReason::OverlapSkipped);
}

#[test]
fn a_zero_concurrency_limit_is_rejected() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let result = plan_host_settlement(
        &schedule,
        Some(utc("2026-03-01T09:00:00Z")),
        SettlementWindow {
            now: utc("2026-03-01T09:00:10Z"),
            grace_ms: 60_000,
        },
        concurrency(0, 0, OverlapPolicy::Skip),
    );
    assert!(result.is_err());
}

#[test]
fn a_clock_that_moved_backwards_never_rewinds_the_cursor() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let next = utc("2026-03-02T09:00:00Z");
    let plan = plan_host_settlement(
        &schedule,
        Some(next),
        SettlementWindow {
            now: utc("2026-03-01T09:00:00Z"),
            grace_ms: 60_000,
        },
        concurrency(0, 1, OverlapPolicy::Skip),
    )
    .expect("the settlement plan is valid");
    assert!(plan.decisions.is_empty());
    assert_eq!(plan.next_scheduled_at, Some(next));
}

#[test]
fn the_repeated_pass_over_the_same_slot_produces_the_same_decision() {
    let schedule = daily_cron("0 9 * * *", MisfirePolicy::Skip);
    let first = utc("2026-03-01T09:00:00Z");
    let window = SettlementWindow {
        now: utc("2026-03-01T09:00:10Z"),
        grace_ms: 60_000,
    };
    let facts = concurrency(0, 1, OverlapPolicy::Skip);
    let first_plan =
        plan_host_settlement(&schedule, Some(first), window, facts).expect("the plan is valid");
    let second_plan =
        plan_host_settlement(&schedule, Some(first), window, facts).expect("the plan is valid");
    assert_eq!(first_plan.decisions, second_plan.decisions);
}

#[test]
fn a_scheduled_occurrence_identity_is_independent_of_the_revision() {
    let key = OccurrenceIdentity::Schedule {
        trigger_id: "trigger-daily".to_owned(),
        scheduled_epoch_ms: utc("2026-03-01T09:00:00Z"),
    }
    .key()
    .expect("the occurrence identity is valid");
    assert_eq!(key.len(), 64);
    assert!(key.chars().all(|value| value.is_ascii_hexdigit()));
}

#[test]
fn the_trigger_kind_tokens_round_trip() {
    assert_eq!(
        OccurrenceTriggerKind::parse(OccurrenceTriggerKind::Manual.as_str()),
        Some(OccurrenceTriggerKind::Manual)
    );
    assert_eq!(
        OccurrenceTriggerKind::parse(OccurrenceTriggerKind::Schedule.as_str()),
        Some(OccurrenceTriggerKind::Schedule)
    );
    assert_eq!(OccurrenceTriggerKind::parse("WEBHOOK"), None);
}
