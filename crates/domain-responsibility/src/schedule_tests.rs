//! Unit and property tests for bounded schedule recurrence and misfire planning.

use super::*;
use crate::{AmbiguousLocalTime, MisfirePolicy, NonexistentLocalTime, RecurrenceFormat};

fn utc(text: &str) -> InstantMs {
    parse_instant(text).expect("test timestamps are valid RFC 3339 instants")
}

fn schedule(
    format: RecurrenceFormat,
    timezone: &str,
    expression: &str,
    policy: MisfirePolicy,
    ambiguous: AmbiguousLocalTime,
    nonexistent: NonexistentLocalTime,
    start_at: Option<InstantMs>,
) -> ScheduleTrigger {
    ScheduleTrigger::parse(
        format,
        timezone,
        expression,
        policy,
        ambiguous,
        nonexistent,
        start_at,
        None,
    )
    .expect("the test schedule is valid")
}

fn daily_cron(timezone: &str, expression: &str) -> ScheduleTrigger {
    schedule(
        RecurrenceFormat::Cron5,
        timezone,
        expression,
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        None,
    )
}

#[test]
fn cron_5_rejects_unsupported_shapes() {
    assert!(Cron5::parse("0 0 * *").is_err());
    assert!(Cron5::parse("0 0 * * * *").is_err());
    assert!(Cron5::parse("60 0 * * *").is_err());
    assert!(Cron5::parse("0 24 * * *").is_err());
    assert!(Cron5::parse("0 0 0 * *").is_err());
    assert!(Cron5::parse("0 0 * 13 *").is_err());
    assert!(Cron5::parse("0 0 * * 8").is_err());
    assert!(Cron5::parse("0 0 * * ?").is_err());
    assert!(Cron5::parse("0 0 * * 5#2").is_err());
    assert!(Cron5::parse("0 0 * * MON-").is_err());
    assert!(Cron5::parse("0 0 * * 5-1").is_err());
    assert!(Cron5::parse("0 0/0 * * *").is_err());
    assert!(Cron5::parse("").is_err());
    assert!(Cron5::parse("0 0 * * MON").is_ok());
    assert!(Cron5::parse("0 0 1 JAN *").is_ok());
    assert!(Cron5::parse("*/15 9-17 * * MON-FRI").is_ok());
}

#[test]
fn cron_5_named_month_and_day_are_normalized() {
    let rule = Cron5::parse("0 9 * JAN MON").unwrap();
    let january = chrono::NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
    assert!(rule.date_matches(january));
    let february = chrono::NaiveDate::from_ymd_opt(2026, 2, 2).unwrap();
    assert!(!rule.date_matches(february));
}

#[test]
fn cron_5_sunday_is_accepted_as_zero_and_seven() {
    let zero = Cron5::parse("0 0 * * 0").unwrap();
    let seven = Cron5::parse("0 0 * * 7").unwrap();
    let sunday = chrono::NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    assert!(zero.date_matches(sunday));
    assert!(seven.date_matches(sunday));
}

#[test]
fn cron_5_daily_slot_is_resolved_in_the_declared_timezone() {
    let trigger = daily_cron("UTC", "0 9 * * *");
    let start = utc("2026-03-01T00:00:00Z");
    let slots = trigger.slots_between(start, utc("2026-03-02T00:00:00Z")).unwrap();
    assert_eq!(slots, vec![utc("2026-03-01T09:00:00Z")]);
}

#[test]
fn cron_5_slot_uses_the_declared_timezone_offset() {
    let trigger = daily_cron("Asia/Tokyo", "0 9 * * *");
    let start = utc("2026-02-28T12:00:00Z");
    let slots = trigger.slots_between(start, utc("2026-03-01T12:00:00Z")).unwrap();
    // 09:00 JST is 00:00 UTC.
    assert_eq!(slots, vec![utc("2026-03-01T00:00:00Z")]);
}

#[test]
fn rrule_rejects_unsupported_parts_and_ambiguous_bounds() {
    let anchor = local_from_instant(chrono_tz::Tz::UTC, utc("2026-01-01T00:00:00Z"));
    assert!(Rrule::parse("FREQ=DAILY;BYSETPOS=1", anchor).is_err());
    assert!(Rrule::parse("FREQ=DAILY;BYWEEKNO=2", anchor).is_err());
    assert!(Rrule::parse("FREQ=SECONDLY", anchor).is_err());
    assert!(Rrule::parse("INTERVAL=2", anchor).is_err());
    assert!(Rrule::parse("FREQ=DAILY;COUNT=2;UNTIL=20260101T000000Z", anchor).is_err());
    assert!(Rrule::parse("FREQ=DAILY;INTERVAL=0", anchor).is_err());
    assert!(Rrule::parse("FREQ=DAILY;BYDAY=2MO", anchor).is_err());
    assert!(Rrule::parse("FREQ=HOURLY;BYDAY=MO", anchor).is_err());
    assert!(Rrule::parse("FREQ=DAILY;BYMONTHDAY=1;BYDAY=MO", anchor).is_err());
    assert!(Rrule::parse("FREQ=DAILY;UNTIL=20260101", anchor).is_err());
    assert!(Rrule::parse("FREQ=DAILY", anchor).is_ok());
    assert!(Rrule::parse("FREQ=WEEKLY;BYDAY=MO,WE;WKST=MO", anchor).is_ok());
    assert!(Rrule::parse("FREQ=WEEKLY;BYDAY=MO,WE;WKST=SU", anchor).is_err());
}

#[test]
fn rrule_daily_every_second_day_produces_the_expected_grid() {
    let trigger = schedule(
        RecurrenceFormat::Rfc5545Rrule,
        "UTC",
        "FREQ=DAILY;INTERVAL=2",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        Some(utc("2026-03-01T08:00:00Z")),
    );
    let slots = trigger
        .slots_between(utc("2026-03-01T00:00:00Z"), utc("2026-03-06T00:00:00Z"))
        .unwrap();
    assert_eq!(
        slots,
        vec![
            utc("2026-03-01T08:00:00Z"),
            utc("2026-03-03T08:00:00Z"),
            utc("2026-03-05T08:00:00Z"),
        ]
    );
}

#[test]
fn rrule_count_bounds_the_rule_from_its_anchor() {
    let trigger = schedule(
        RecurrenceFormat::Rfc5545Rrule,
        "UTC",
        "FREQ=DAILY;COUNT=2",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        Some(utc("2026-03-01T08:00:00Z")),
    );
    let slots = trigger
        .slots_between(utc("2026-03-01T00:00:00Z"), utc("2026-03-06T00:00:00Z"))
        .unwrap();
    assert_eq!(
        slots,
        vec![utc("2026-03-01T08:00:00Z"), utc("2026-03-02T08:00:00Z")]
    );
    assert_eq!(trigger.next_slot_after(utc("2026-03-02T08:00:00Z")).unwrap(), None);
}

#[test]
fn rrule_until_bounds_the_rule() {
    let trigger = schedule(
        RecurrenceFormat::Rfc5545Rrule,
        "UTC",
        "FREQ=DAILY;UNTIL=20260302T080000Z",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        Some(utc("2026-03-01T08:00:00Z")),
    );
    let slots = trigger
        .slots_between(utc("2026-03-01T00:00:00Z"), utc("2026-03-06T00:00:00Z"))
        .unwrap();
    assert_eq!(
        slots,
        vec![utc("2026-03-01T08:00:00Z"), utc("2026-03-02T08:00:00Z")]
    );
}

#[test]
fn rrule_weekly_byday_selects_the_requested_weekdays() {
    let trigger = schedule(
        RecurrenceFormat::Rfc5545Rrule,
        "UTC",
        "FREQ=WEEKLY;BYDAY=MO,WE",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        Some(utc("2026-03-02T09:00:00Z")),
    );
    let slots = trigger
        .slots_between(utc("2026-03-01T00:00:00Z"), utc("2026-03-10T00:00:00Z"))
        .unwrap();
    assert_eq!(
        slots,
        vec![
            utc("2026-03-02T09:00:00Z"),
            utc("2026-03-04T09:00:00Z"),
            utc("2026-03-09T09:00:00Z"),
        ]
    );
}

#[test]
fn rrule_monthly_bymonthday_clamps_to_the_month_length() {
    let trigger = schedule(
        RecurrenceFormat::Rfc5545Rrule,
        "UTC",
        "FREQ=MONTHLY;BYMONTHDAY=31",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        Some(utc("2026-01-31T06:00:00Z")),
    );
    let slots = trigger
        .slots_between(utc("2026-01-01T00:00:00Z"), utc("2026-05-01T00:00:00Z"))
        .unwrap();
    assert_eq!(
        slots,
        vec![utc("2026-01-31T06:00:00Z"), utc("2026-03-31T06:00:00Z")]
    );
}

#[test]
fn dst_gap_slot_is_skipped_and_next_valid_shifts_forward() {
    // 2026-03-08 02:30 America/New_York does not exist.
    let skipping = daily_cron("America/New_York", "30 2 * * *");
    let gap_day = skipping
        .slots_between(utc("2026-03-08T00:00:00Z"), utc("2026-03-09T00:00:00Z"))
        .unwrap();
    assert!(gap_day.is_empty());

    let shifting = schedule(
        RecurrenceFormat::Cron5,
        "America/New_York",
        "30 2 * * *",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::NextValid,
        None,
    );
    let shifted = shifting
        .slots_between(utc("2026-03-08T00:00:00Z"), utc("2026-03-09T00:00:00Z"))
        .unwrap();
    assert_eq!(shifted, vec![utc("2026-03-08T07:30:00Z")]);
}

#[test]
fn dst_fold_slot_honors_the_ambiguity_policy() {
    // 2026-11-01 01:30 America/New_York happens twice.
    let earlier = schedule(
        RecurrenceFormat::Cron5,
        "America/New_York",
        "30 1 * * *",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        None,
    );
    let later = schedule(
        RecurrenceFormat::Cron5,
        "America/New_York",
        "30 1 * * *",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Later,
        NonexistentLocalTime::Skip,
        None,
    );
    let earlier_slots = earlier
        .slots_between(utc("2026-11-01T00:00:00Z"), utc("2026-11-02T00:00:00Z"))
        .unwrap();
    let later_slots = later
        .slots_between(utc("2026-11-01T00:00:00Z"), utc("2026-11-02T00:00:00Z"))
        .unwrap();
    assert_eq!(earlier_slots, vec![utc("2026-11-01T05:30:00Z")]);
    assert_eq!(later_slots, vec![utc("2026-11-01T06:30:00Z")]);
}

#[test]
fn on_time_slot_produces_exactly_one_run_intent() {
    let trigger = daily_cron("UTC", "0 9 * * *");
    let settlement = trigger
        .plan_settlement(
            Some(utc("2026-03-01T09:00:00Z")),
            SettlementWindow {
                now: utc("2026-03-01T09:00:30Z"),
                grace_ms: 60_000,
            },
        )
        .unwrap();
    assert_eq!(
        settlement.intents,
        vec![OccurrenceIntent::Run {
            instant: utc("2026-03-01T09:00:00Z"),
            covered_from: None,
            covered_to: None,
            covered_count: 1,
        }]
    );
    assert!(!settlement.misfired);
    assert_eq!(settlement.next_scheduled_at, Some(utc("2026-03-02T09:00:00Z")));
}

#[test]
fn skip_misfire_records_every_missed_slot_and_creates_no_run() {
    let trigger = daily_cron("UTC", "0 9 * * *");
    let settlement = trigger
        .plan_settlement(
            Some(utc("2026-03-01T09:00:00Z")),
            SettlementWindow {
                now: utc("2026-03-04T09:00:30Z"),
                grace_ms: 60_000,
            },
        )
        .unwrap();
    assert!(settlement.misfired);
    assert_eq!(
        settlement.intents,
        vec![
            OccurrenceIntent::Skip {
                instant: utc("2026-03-01T09:00:00Z"),
                reason: MisfireReason::MissedSlot,
            },
            OccurrenceIntent::Skip {
                instant: utc("2026-03-02T09:00:00Z"),
                reason: MisfireReason::MissedSlot,
            },
            OccurrenceIntent::Skip {
                instant: utc("2026-03-03T09:00:00Z"),
                reason: MisfireReason::MissedSlot,
            },
            OccurrenceIntent::Run {
                instant: utc("2026-03-04T09:00:00Z"),
                covered_from: None,
                covered_to: None,
                covered_count: 1,
            },
        ]
    );
    assert_eq!(settlement.next_scheduled_at, Some(utc("2026-03-05T09:00:00Z")));
}

#[test]
fn run_once_misfire_coalesces_the_missed_interval_into_one_logical_occurrence() {
    let trigger = schedule(
        RecurrenceFormat::Cron5,
        "UTC",
        "0 9 * * *",
        MisfirePolicy::RunOnceWhenAvailable,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        None,
    );
    let settlement = trigger
        .plan_settlement(
            Some(utc("2026-03-01T09:00:00Z")),
            SettlementWindow {
                now: utc("2026-03-04T09:00:30Z"),
                grace_ms: 60_000,
            },
        )
        .unwrap();
    assert!(settlement.misfired);
    assert_eq!(
        settlement.intents,
        vec![
            OccurrenceIntent::Run {
                instant: utc("2026-03-03T09:00:00Z"),
                covered_from: Some(utc("2026-03-01T09:00:00Z")),
                covered_to: Some(utc("2026-03-03T09:00:00Z")),
                covered_count: 3,
            },
            OccurrenceIntent::Run {
                instant: utc("2026-03-04T09:00:00Z"),
                covered_from: None,
                covered_to: None,
                covered_count: 1,
            },
        ]
    );
}

#[test]
fn catch_up_bounded_misfire_creates_a_bounded_run_set_and_one_summary() {
    let trigger = schedule(
        RecurrenceFormat::Cron5,
        "UTC",
        "0 9 * * *",
        MisfirePolicy::CatchUpBounded {
            max_occurrences: 2,
        },
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        None,
    );
    let settlement = trigger
        .plan_settlement(
            Some(utc("2026-03-01T09:00:00Z")),
            SettlementWindow {
                now: utc("2026-03-04T09:00:30Z"),
                grace_ms: 60_000,
            },
        )
        .unwrap();
    assert!(settlement.misfired);
    assert_eq!(
        settlement.intents,
        vec![
            OccurrenceIntent::Run {
                instant: utc("2026-03-01T09:00:00Z"),
                covered_from: None,
                covered_to: None,
                covered_count: 1,
            },
            OccurrenceIntent::Run {
                instant: utc("2026-03-02T09:00:00Z"),
                covered_from: None,
                covered_to: None,
                covered_count: 1,
            },
            OccurrenceIntent::Skip {
                instant: utc("2026-03-03T09:00:00Z"),
                reason: MisfireReason::ExcessCatchUp,
            },
            OccurrenceIntent::Run {
                instant: utc("2026-03-04T09:00:00Z"),
                covered_from: None,
                covered_to: None,
                covered_count: 1,
            },
        ]
    );
}

#[test]
fn a_backwards_clock_never_fires_or_rewinds_the_persisted_deadline() {
    let trigger = daily_cron("UTC", "0 9 * * *");
    let settlement = trigger
        .plan_settlement(
            Some(utc("2026-03-05T09:00:00Z")),
            SettlementWindow {
                now: utc("2026-03-04T09:00:30Z"),
                grace_ms: 60_000,
            },
        )
        .unwrap();
    assert!(settlement.intents.is_empty());
    assert!(!settlement.misfired);
    assert_eq!(settlement.next_scheduled_at, Some(utc("2026-03-05T09:00:00Z")));
}

#[test]
fn a_first_observation_creates_no_occurrence_and_finds_the_next_slot() {
    let trigger = daily_cron("UTC", "0 9 * * *");
    let settlement = trigger
        .plan_settlement(
            None,
            SettlementWindow {
                now: utc("2026-03-01T10:00:00Z"),
                grace_ms: 60_000,
            },
        )
        .unwrap();
    assert!(settlement.intents.is_empty());
    assert_eq!(settlement.next_scheduled_at, Some(utc("2026-03-02T09:00:00Z")));
}

#[test]
fn settlement_is_deterministic_for_the_same_cursor_and_window() {
    let trigger = daily_cron("UTC", "0 9 * * *");
    let window = SettlementWindow {
        now: utc("2026-03-04T09:00:30Z"),
        grace_ms: 60_000,
    };
    let first = trigger
        .plan_settlement(Some(utc("2026-03-01T09:00:00Z")), window)
        .unwrap();
    let second = trigger
        .plan_settlement(Some(utc("2026-03-01T09:00:00Z")), window)
        .unwrap();
    assert_eq!(first, second);
}

#[test]
fn an_exhausted_rule_reports_no_next_slot() {
    let trigger = schedule(
        RecurrenceFormat::Cron5,
        "UTC",
        "0 9 * * *",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        None,
    );
    let bounded = ScheduleTrigger {
        end_at: Some(utc("2026-03-02T00:00:00Z")),
        ..trigger
    };
    assert_eq!(
        bounded.next_slot_after(utc("2026-03-02T00:00:00Z")).unwrap(),
        None
    );
}

#[test]
fn schedule_parsing_rejects_an_unknown_timezone() {
    assert!(ScheduleTrigger::parse(
        RecurrenceFormat::Cron5,
        "Mars/Olympus_Mons",
        "0 9 * * *",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        None,
        None,
    )
    .is_err());
}

#[test]
fn rrule_without_a_start_anchor_is_rejected() {
    assert!(ScheduleTrigger::parse(
        RecurrenceFormat::Rfc5545Rrule,
        "UTC",
        "FREQ=DAILY",
        MisfirePolicy::Skip,
        AmbiguousLocalTime::Earlier,
        NonexistentLocalTime::Skip,
        None,
        None,
    )
    .is_err());
}

#[test]
fn instants_round_trip_through_the_storage_timestamp_format() {
    let instant = utc("2026-03-01T09:00:00Z");
    let formatted = format_instant(instant).unwrap();
    assert_eq!(formatted, "2026-03-01T09:00:00.000000000Z");
    assert_eq!(parse_instant(&formatted), Some(instant));
}
