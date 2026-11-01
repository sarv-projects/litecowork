//! Bounded schedule recurrence, IANA timezone resolution, and misfire planning.
//!
//! This module is pure decision logic. It never reads a clock, never persists, and
//! never creates a Task. The trigger host supplies the current instant and the
//! persisted cursor; this module decides which scheduled slots are due, which
//! slots were missed, and how a misfire is recorded.
//!
//! Supported V1 recurrence subset:
//!
//! - `CRON_5`: five fields (`minute hour day-of-month month day-of-week`) with
//!   `*`, `,`, `-`, and `/` items plus three-letter month/day names.
//! - `RFC5545_RRULE`: `FREQ` of `DAILY`, `WEEKLY`, `MONTHLY`, `YEARLY`,
//!   `HOURLY`, or `MINUTELY` with `INTERVAL`, `COUNT`, `UNTIL`, `BYMONTH`,
//!   `BYMONTHDAY`, `BYDAY`, `BYHOUR`, `BYMINUTE`, and `BYSECOND`.
//!
//! Anything outside that subset is rejected at admission so a saved Automation
//! never appears schedulable while part of its recurrence is ignored.

use crate::{
    AmbiguousLocalTime, DomainError, MisfirePolicy, NonexistentLocalTime, RecurrenceFormat,
};
use chrono::offset::TimeZone as _;
use chrono::{Datelike, LocalResult, NaiveDate, NaiveDateTime, Offset as _, Timelike};
use chrono_tz::Tz;
use std::collections::BTreeSet;

/// A resolved instant on the UTC timeline, in Unix epoch milliseconds.
pub type InstantMs = i64;

/// The largest number of recurrence periods a single scan may enumerate before the
/// rule is treated as unusable. This bounds hostile or pathological definitions.
const MAX_SCANNED_PERIODS: usize = 20_000;
/// The longest horizon, in days, a forward scan may cover.
const MAX_HORIZON_DAYS: i64 = 366 * 5;

/// A parsed, executable schedule trigger.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleTrigger {
    pub recurrence: Recurrence,
    pub timezone: Tz,
    pub misfire_policy: MisfirePolicy,
    pub ambiguous_local_time: AmbiguousLocalTime,
    pub nonexistent_local_time: NonexistentLocalTime,
    /// Inclusive lower bound, in epoch milliseconds.
    pub start_at: Option<InstantMs>,
    /// Inclusive upper bound, in epoch milliseconds.
    pub end_at: Option<InstantMs>,
}

/// The two supported recurrence grammars.
#[derive(Clone, Debug, PartialEq)]
pub enum Recurrence {
    Cron5(Cron5),
    Rrule(Rrule),
}

/// A parsed five-field cron rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Cron5 {
    minutes: BTreeSet<u32>,
    hours: BTreeSet<u32>,
    days_of_month: BTreeSet<u32>,
    months: BTreeSet<u32>,
    days_of_week: BTreeSet<u32>,
    dom_restricted: bool,
    dow_restricted: bool,
}

/// The parsed subset of an RFC 5545 recurrence rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Rrule {
    freq: Freq,
    interval: u32,
    count: Option<u32>,
    until: Option<InstantMs>,
    by_month: Option<BTreeSet<u32>>,
    by_month_day: Option<BTreeSet<u32>>,
    by_day: Option<BTreeSet<u32>>,
    by_hour: Option<BTreeSet<u32>>,
    by_minute: Option<BTreeSet<u32>>,
    by_second: Option<BTreeSet<u32>>,
    /// The DTSTART anchor expressed as a local wall-clock time.
    anchor_local: NaiveDateTime,
}

/// The supported recurrence frequencies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freq {
    Minutely,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// Why a scheduled slot produced no Task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MisfireReason {
    /// The policy is `SKIP` and the slot elapsed without a host claim.
    MissedSlot,
    /// The policy is `CATCH_UP_BOUNDED` and the slot exceeded the bounded budget.
    ExcessCatchUp,
}

/// One durable occurrence intent produced by a settlement pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OccurrenceIntent {
    /// Materialize one occurrence and one ordinary Task.
    Run {
        instant: InstantMs,
        /// The covered interval when several missed slots coalesced into one run.
        covered_from: Option<InstantMs>,
        covered_to: Option<InstantMs>,
        /// How many slots this single run covers.
        covered_count: u32,
    },
    /// Record one bounded SKIPPED occurrence and create no Task.
    Skip {
        instant: InstantMs,
        reason: MisfireReason,
    },
}

/// The durable result of one settlement pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduleSettlement {
    /// Occurrences to create, in chronological order.
    pub intents: Vec<OccurrenceIntent>,
    /// The next slot the host must observe, or `None` when the rule is exhausted.
    pub next_scheduled_at: Option<InstantMs>,
    /// True when the host observed the schedule after one or more slots elapsed.
    pub misfired: bool,
}

/// The clock and grace window supplied by the trigger host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettlementWindow {
    /// The host's current instant.
    pub now: InstantMs,
    /// How late a slot may be and still be treated as an ordinary on-time run.
    pub grace_ms: i64,
}

impl ScheduleTrigger {
    /// Parses a stored schedule trigger definition.
    pub fn parse(
        recurrence_format: RecurrenceFormat,
        timezone: &str,
        rrule_or_cron: &str,
        misfire_policy: MisfirePolicy,
        ambiguous_local_time: AmbiguousLocalTime,
        nonexistent_local_time: NonexistentLocalTime,
        start_at: Option<InstantMs>,
        end_at: Option<InstantMs>,
    ) -> Result<Self, DomainError> {
        let timezone: Tz = timezone
            .parse()
            .map_err(|_| DomainError::InvalidDefinition)?;
        let recurrence = match recurrence_format {
            RecurrenceFormat::Cron5 => Recurrence::Cron5(
                Cron5::parse(rrule_or_cron).map_err(|_| DomainError::InvalidDefinition)?,
            ),
            RecurrenceFormat::Rfc5545Rrule => {
                // An RFC 5545 rule is anchored at DTSTART. Without an explicit
                // start the rule has no stable period grid, so it is rejected
                // instead of being anchored at an arbitrary moment.
                let anchor_local = start_at
                    .map(|start| local_from_instant(timezone, start))
                    .ok_or(DomainError::InvalidDefinition)?;
                Recurrence::Rrule(
                    Rrule::parse(rrule_or_cron, anchor_local)
                        .map_err(|_| DomainError::InvalidDefinition)?,
                )
            }
        };
        if let (Some(start), Some(end)) = (start_at, end_at)
            && end < start
        {
            return Err(DomainError::InvalidDefinition);
        }
        Ok(Self {
            recurrence,
            timezone,
            misfire_policy,
            ambiguous_local_time,
            nonexistent_local_time,
            start_at,
            end_at,
        })
    }

    /// Resolves a local wall-clock candidate to a UTC instant, honoring the
    /// configured DST fold and gap policy. `None` means the local time does not
    /// exist and the policy is `SKIP`.
    pub fn resolve_local(&self, local: NaiveDateTime) -> Option<InstantMs> {
        match self.timezone.from_local_datetime(&local) {
            LocalResult::Single(resolved) => Some(resolved.timestamp_millis()),
            LocalResult::Ambiguous(first, second) => {
                let a = first.timestamp_millis();
                let b = second.timestamp_millis();
                Some(match self.ambiguous_local_time {
                    AmbiguousLocalTime::Earlier => a.min(b),
                    AmbiguousLocalTime::Later => a.max(b),
                })
            }
            LocalResult::None => match self.nonexistent_local_time {
                NonexistentLocalTime::Skip => None,
                NonexistentLocalTime::NextValid => self.next_valid_after_gap(local),
            },
        }
    }

    /// Finds the first valid instant at or after a local time that falls in a DST gap.
    fn next_valid_after_gap(&self, local: NaiveDateTime) -> Option<InstantMs> {
        let before = local
            .checked_sub_signed(chrono::Duration::hours(1))
            .and_then(|value| self.timezone.from_local_datetime(&value).single())
            .map(|value| value.offset().fix())?;
        let after = local
            .checked_add_signed(chrono::Duration::hours(1))
            .and_then(|value| self.timezone.from_local_datetime(&value).single())
            .map(|value| value.offset().fix())?;
        let gap_seconds = i64::from(after.local_minus_utc() - before.local_minus_utc());
        if gap_seconds <= 0 {
            return None;
        }
        let shifted = local.checked_add_signed(chrono::Duration::seconds(gap_seconds))?;
        match self.timezone.from_local_datetime(&shifted) {
            LocalResult::Single(resolved) => Some(resolved.timestamp_millis()),
            LocalResult::Ambiguous(first, second) => {
                let a = first.timestamp_millis();
                let b = second.timestamp_millis();
                Some(a.min(b))
            }
            LocalResult::None => None,
        }
    }

    /// The rule's inclusive upper bound, if the rule declares one.
    pub fn until(&self) -> Option<InstantMs> {
        match &self.recurrence {
            Recurrence::Cron5(_) => None,
            Recurrence::Rrule(rule) => rule.until,
        }
    }

    /// Enumerates every resolved slot in `[from, to]`.
    ///
    /// A persisted cursor's `next_scheduled_at` is the earliest slot that has not
    /// been materialized yet, so the lower bound is inclusive.
    ///
    /// Slots that fall in a DST gap under the `SKIP` policy produce no instant.
    pub fn slots_between(
        &self,
        from: InstantMs,
        to: InstantMs,
    ) -> Result<Vec<InstantMs>, DomainError> {
        if to < from {
            return Ok(Vec::new());
        }
        let from_local = local_from_instant(self.timezone, from);
        let to_local = local_from_instant(self.timezone, to);
        let candidates = self
            .recurrence
            .candidates_after(from_local, to_local)?;
        let until = self.until();
        let mut slots = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let Some(instant) = self.resolve_local(candidate) else {
                continue;
            };
            if instant < from || instant > to {
                continue;
            }
            if let Some(until) = until
                && instant > until
            {
                break;
            }
            if let Some(start) = self.start_at
                && instant < start
            {
                continue;
            }
            if let Some(end) = self.end_at
                && instant > end
            {
                break;
            }
            if slots.last().is_some_and(|last| *last == instant) {
                continue;
            }
            slots.push(instant);
        }
        Ok(slots)
    }

    /// The first resolved slot strictly after `after`.
    pub fn next_slot_after(&self, after: InstantMs) -> Result<Option<InstantMs>, DomainError> {
        let local = local_from_instant(self.timezone, after);
        let horizon = local
            .checked_add_signed(chrono::Duration::days(MAX_HORIZON_DAYS))
            .ok_or(DomainError::InvalidDefinition)?;
        let until = self.until();
        let mut cursor = local;
        for _ in 0..MAX_SCANNED_PERIODS {
            let Some(candidate) = self.recurrence.next_candidate(cursor)? else {
                return Ok(None);
            };
            if candidate >= horizon {
                return Ok(None);
            }
            if let Some(instant) = self.resolve_local(candidate) {
                if instant > after {
                    if let Some(until) = until
                        && instant > until
                    {
                        return Ok(None);
                    }
                    if let Some(end) = self.end_at
                        && instant > end
                    {
                        return Ok(None);
                    }
                    if let Some(start) = self.start_at
                        && instant < start
                    {
                        cursor = candidate;
                        continue;
                    }
                    return Ok(Some(instant));
                }
            }
            cursor = candidate;
        }
        Ok(None)
    }

    /// Decides which occurrences a host must create for the supplied window.
    ///
    /// The function is pure: the same cursor and window always produce the same
    /// intents, and a replay of an already-settled slot produces no new intent
    /// because the caller derives the occurrence identity from the slot instant.
    pub fn plan_settlement(
        &self,
        next_scheduled_at: Option<InstantMs>,
        window: SettlementWindow,
    ) -> Result<ScheduleSettlement, DomainError> {
        let now = window.now;
        // A clock that moved backwards must not re-fire or rewind a persisted
        // deadline. The next slot stays exactly where it was.
        if let Some(next) = next_scheduled_at
            && next > now
        {
            return Ok(ScheduleSettlement {
                intents: Vec::new(),
                next_scheduled_at: Some(next),
                misfired: false,
            });
        }
        let due = match next_scheduled_at {
            Some(next) => self.slots_between(next, now)?,
            None => Vec::new(),
        };
        let next_slot = self.next_slot_after(now)?;
        let mut on_time = Vec::new();
        let mut missed = Vec::new();
        for slot in due {
            if now - slot <= window.grace_ms {
                on_time.push(slot);
            } else {
                missed.push(slot);
            }
        }
        let mut intents = Vec::new();
        let mut misfired = !missed.is_empty();
        match (&self.misfire_policy, missed.is_empty()) {
            (MisfirePolicy::Skip, false) => {
                for slot in missed {
                    intents.push(OccurrenceIntent::Skip {
                        instant: slot,
                        reason: MisfireReason::MissedSlot,
                    });
                }
            }
            (MisfirePolicy::RunOnceWhenAvailable, false) => {
                let first = missed.first().copied().unwrap_or_default();
                let last = missed.last().copied().unwrap_or_default();
                intents.push(OccurrenceIntent::Run {
                    instant: last,
                    covered_from: Some(first),
                    covered_to: Some(last),
                    covered_count: u32::try_from(missed.len()).unwrap_or(u32::MAX),
                });
            }
            (MisfirePolicy::CatchUpBounded { max_occurrences }, false) => {
                let budget = *max_occurrences as usize;
                for slot in missed.iter().take(budget) {
                    intents.push(OccurrenceIntent::Run {
                        instant: *slot,
                        covered_from: None,
                        covered_to: None,
                        covered_count: 1,
                    });
                }
                let excess = &missed[budget.min(missed.len())..];
                if !excess.is_empty() {
                    misfired = true;
                    intents.push(OccurrenceIntent::Skip {
                        instant: excess.last().copied().unwrap_or_default(),
                        reason: MisfireReason::ExcessCatchUp,
                    });
                }
            }
            (_, true) => {}
        }
        for slot in on_time {
            intents.push(OccurrenceIntent::Run {
                instant: slot,
                covered_from: None,
                covered_to: None,
                covered_count: 1,
            });
        }
        intents.sort_by_key(|intent| match intent {
            OccurrenceIntent::Run { instant, .. } => *instant,
            OccurrenceIntent::Skip { instant, .. } => *instant,
        });
        Ok(ScheduleSettlement {
            intents,
            next_scheduled_at: next_slot,
            misfired,
        })
    }
}

impl Recurrence {
    fn candidates_after(
        &self,
        from_local: NaiveDateTime,
        to_local: NaiveDateTime,
    ) -> Result<Vec<NaiveDateTime>, DomainError> {
        match self {
            Recurrence::Cron5(rule) => rule.candidates_after(from_local, to_local),
            Recurrence::Rrule(rule) => rule.candidates_after(from_local, to_local),
        }
    }

    fn next_candidate(
        &self,
        after_local: NaiveDateTime,
    ) -> Result<Option<NaiveDateTime>, DomainError> {
        match self {
            Recurrence::Cron5(rule) => rule.next_candidate(after_local),
            Recurrence::Rrule(rule) => rule.next_candidate(after_local),
        }
    }
}

impl Cron5 {
    /// Parses a five-field cron expression.
    pub fn parse(expression: &str) -> Result<Self, String> {
        let fields: Vec<&str> = expression.split_whitespace().collect();
        if fields.len() != 5 {
            return Err("cron_5 requires exactly five fields".to_owned());
        }
        let (minutes, _) = parse_cron_field(fields[0], 0, 59, &[], 0)?;
        let (hours, _) = parse_cron_field(fields[1], 0, 23, &[], 0)?;
        let (days_of_month, dom_restricted) = parse_cron_field(fields[2], 1, 31, &[], 0)?;
        let (months, _) = parse_cron_field(fields[3], 1, 12, MONTH_NAMES, 1)?;
        let (mut days_of_week, dow_restricted) =
            parse_cron_field(fields[4], 0, 7, DAY_NAMES, 0)?;
        // Sunday is both 0 and 7 in cron; normalize to 0.
        if days_of_week.remove(&7) {
            days_of_week.insert(0);
        }
        Ok(Self {
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
            dom_restricted,
            dow_restricted,
        })
    }

    /// True when the local date satisfies the rule's day and month filters.
    pub fn date_matches(&self, date: NaiveDate) -> bool {
        if !self.months.contains(&date.month()) {
            return false;
        }
        let dom_ok = self.days_of_month.contains(&date.day());
        let dow_ok = self
            .days_of_week
            .contains(&(date.weekday().num_days_from_sunday() as u32));
        match (self.dom_restricted, self.dow_restricted) {
            (false, false) => true,
            (true, false) => dom_ok,
            (false, true) => dow_ok,
            // Vixie cron unions day-of-month and day-of-week when both are
            // restricted, so `0 0 1 * MON` fires on the 1st and on Mondays.
            (true, true) => dom_ok || dow_ok,
        }
    }

    fn next_candidate(&self, after: NaiveDateTime) -> Result<Option<NaiveDateTime>, DomainError> {
        let mut date = after.date();
        for _ in 0..MAX_HORIZON_DAYS {
            if self.date_matches(date) {
                for hour in &self.hours {
                    for minute in &self.minutes {
                        let Some(candidate) = date.and_hms_opt(*hour, *minute, 0) else {
                            continue;
                        };
                        if candidate > after {
                            return Ok(Some(candidate));
                        }
                    }
                }
            }
            date = date.succ_opt().ok_or(DomainError::InvalidDefinition)?;
        }
        Ok(None)
    }

    fn candidates_after(
        &self,
        from: NaiveDateTime,
        to: NaiveDateTime,
    ) -> Result<Vec<NaiveDateTime>, DomainError> {
        let mut candidates = Vec::new();
        // The lower bound is inclusive: a persisted cursor names the earliest
        // slot that has not been materialized yet.
        let mut cursor = from
            .checked_sub_signed(chrono::Duration::nanoseconds(1))
            .ok_or(DomainError::InvalidDefinition)?;
        for _ in 0..MAX_SCANNED_PERIODS {
            let Some(candidate) = self.next_candidate(cursor)? else {
                break;
            };
            if candidate > to {
                break;
            }
            candidates.push(candidate);
            cursor = candidate;
        }
        Ok(candidates)
    }
}

impl Rrule {
    /// Parses the supported RFC 5545 recurrence subset.
    pub fn parse(rule: &str, anchor_local: NaiveDateTime) -> Result<Self, String> {
        let mut freq = None;
        let mut interval = 1_u32;
        let mut count = None;
        let mut until = None;
        let mut by_month = None;
        let mut by_month_day = None;
        let mut by_day = None;
        let mut by_hour = None;
        let mut by_minute = None;
        let mut by_second = None;
        for part in rule.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let Some((key, value)) = part.split_once('=') else {
                return Err("rrule part must be KEY=VALUE".to_owned());
            };
            let key = key.trim().to_ascii_uppercase();
            let value = value.trim();
            match key.as_str() {
                "FREQ" => {
                    freq = Some(match value.to_ascii_uppercase().as_str() {
                        "MINUTELY" => Freq::Minutely,
                        "HOURLY" => Freq::Hourly,
                        "DAILY" => Freq::Daily,
                        "WEEKLY" => Freq::Weekly,
                        "MONTHLY" => Freq::Monthly,
                        "YEARLY" => Freq::Yearly,
                        other => return Err(format!("unsupported rrule FREQ {other}")),
                    });
                }
                "INTERVAL" => {
                    interval = value
                        .parse::<u32>()
                        .ok()
                        .filter(|value| *value > 0 && *value <= 1000)
                        .ok_or("rrule INTERVAL must be 1..=1000")?;
                }
                "COUNT" => {
                    count = Some(
                        value
                            .parse::<u32>()
                            .ok()
                            .filter(|value| *value > 0)
                            .ok_or("rrule COUNT must be a positive integer")?,
                    );
                }
                "UNTIL" => {
                    until = Some(parse_rrule_until(value)?);
                }
                "BYMONTH" => by_month = Some(parse_number_set(value, 1, 12)?),
                "BYMONTHDAY" => by_month_day = Some(parse_number_set(value, 1, 31)?),
                "BYDAY" => by_day = Some(parse_weekday_set(value)?),
                "BYHOUR" => by_hour = Some(parse_number_set(value, 0, 23)?),
                "BYMINUTE" => by_minute = Some(parse_number_set(value, 0, 59)?),
                "BYSECOND" => by_second = Some(parse_number_set(value, 0, 59)?),
                // `WKST` only matters for weekly intervals; the supported
                // implementation always uses Monday.
                "WKST" => {
                    if !value.eq_ignore_ascii_case("MO") {
                        return Err("rrule WKST is only supported with MO".to_owned());
                    }
                }
                other => {
                    return Err(format!("unsupported rrule part {other}"));
                }
            }
        }
        let freq = freq.ok_or("rrule FREQ is required")?;
        if count.is_some() && until.is_some() {
            return Err("rrule COUNT and UNTIL are mutually exclusive".to_owned());
        }
        if by_day.is_some() && freq != Freq::Weekly && freq != Freq::Monthly && freq != Freq::Yearly
        {
            return Err("rrule BYDAY requires WEEKLY, MONTHLY, or YEARLY".to_owned());
        }
        if by_month_day.is_some() && by_day.is_some() {
            return Err("rrule BYMONTHDAY and BYDAY are mutually exclusive".to_owned());
        }
        Ok(Self {
            freq,
            interval,
            count,
            until,
            by_month,
            by_month_day,
            by_day,
            by_hour,
            by_minute,
            by_second,
            anchor_local,
        })
    }

    fn default_time(&self) -> (u32, u32, u32) {
        (
            self.anchor_local.hour(),
            self.anchor_local.minute(),
            self.anchor_local.second(),
        )
    }

    /// Expands the rule's time-of-day parts for one local date.
    fn times_for(&self, date: NaiveDate) -> Result<Vec<NaiveDateTime>, DomainError> {
        let (default_hour, default_minute, default_second) = self.default_time();
        let hours = self
            .by_hour
            .clone()
            .unwrap_or_else(|| BTreeSet::from([default_hour]));
        let minutes = self
            .by_minute
            .clone()
            .unwrap_or_else(|| BTreeSet::from([default_minute]));
        let seconds = self
            .by_second
            .clone()
            .unwrap_or_else(|| BTreeSet::from([default_second]));
        if hours
            .len()
            .saturating_mul(minutes.len())
            .saturating_mul(seconds.len())
            > 256
        {
            return Err(DomainError::InvalidDefinition);
        }
        let mut times = Vec::new();
        for hour in &hours {
            for minute in &minutes {
                for second in &seconds {
                    if let Some(value) = date.and_hms_opt(*hour, *minute, *second) {
                        times.push(value);
                    }
                }
            }
        }
        Ok(times)
    }

    fn day_matches(&self, date: NaiveDate) -> bool {
        if let Some(months) = &self.by_month
            && !months.contains(&date.month())
        {
            return false;
        }
        if let Some(days) = &self.by_month_day
            && !days.contains(&date.day())
        {
            return false;
        }
        if let Some(days) = &self.by_day
            && !days.contains(&(date.weekday().num_days_from_sunday() as u32))
        {
            return false;
        }
        true
    }

    /// The local candidates produced by one base period of the rule.
    fn candidates_in_period(
        &self,
        period: usize,
    ) -> Result<Vec<NaiveDateTime>, DomainError> {
        let anchor_date = self.anchor_local.date();
        let mut dates = Vec::new();
        match self.freq {
            Freq::Daily => {
                let Some(date) = anchor_date.checked_add_signed(chrono::Duration::days(
                    (period as i64) * (self.interval as i64),
                )) else {
                    return Ok(Vec::new());
                };
                dates.push(date);
            }
            Freq::Weekly => {
                let anchor_week = anchor_date
                    - chrono::Duration::days(
                        anchor_date.weekday().num_days_from_monday() as i64,
                    );
                let Some(week) = anchor_week.checked_add_signed(chrono::Duration::weeks(
                    (period as i64) * (self.interval as i64),
                )) else {
                    return Ok(Vec::new());
                };
                let days = self.by_day.clone().unwrap_or_else(|| {
                    BTreeSet::from([(anchor_date.weekday().num_days_from_sunday()) as u32])
                });
                for weekday in days {
                    // Sunday is 0; convert to an offset from Monday.
                    let offset = (weekday + 6) % 7;
                    if let Some(date) =
                        week.checked_add_signed(chrono::Duration::days(offset as i64))
                    {
                        dates.push(date);
                    }
                }
            }
            Freq::Monthly => {
                let Some(date) =
                    add_months(anchor_date, period as i64 * self.interval as i64)
                else {
                    return Ok(Vec::new());
                };
                if let Some(days) = &self.by_month_day {
                    for day in days {
                        if let Some(value) = date.with_day(*day) {
                            dates.push(value);
                        }
                    }
                } else if let Some(days) = &self.by_day {
                    let mut day = date;
                    while day.month() == date.month() {
                        if days.contains(&(day.weekday().num_days_from_sunday() as u32)) {
                            dates.push(day);
                        }
                        let Some(next) = day.succ_opt() else { break };
                        day = next;
                    }
                } else if let Some(value) =
                    date.with_day(anchor_date.day().min(days_in_month(date)))
                {
                    dates.push(value);
                }
            }
            Freq::Yearly => {
                let Some(base) = add_months(
                    anchor_date.with_day(1).ok_or(DomainError::InvalidDefinition)?,
                    (period as i64) * (self.interval as i64) * 12,
                ) else {
                    return Ok(Vec::new());
                };
                let months = self
                    .by_month
                    .clone()
                    .unwrap_or_else(|| BTreeSet::from([anchor_date.month()]));
                for month in months {
                    let offset = month as i64 - anchor_date.month() as i64;
                    let Some(month_date) = add_months(base, offset) else {
                        continue;
                    };
                    if let Some(days) = &self.by_month_day {
                        for day in days {
                            if let Some(value) = month_date.with_day(*day) {
                                dates.push(value);
                            }
                        }
                    } else if let Some(days) = &self.by_day {
                        let mut day = month_date;
                        while day.month() == month {
                            if days.contains(&(day.weekday().num_days_from_sunday() as u32)) {
                                dates.push(day);
                            }
                            let Some(next) = day.succ_opt() else { break };
                            day = next;
                        }
                    } else if let Some(value) = month_date
                        .with_day(anchor_date.day().min(days_in_month(month_date)))
                    {
                        dates.push(value);
                    }
                }
            }
            Freq::Hourly | Freq::Minutely => {
                let step = match self.freq {
                    Freq::Hourly => {
                        chrono::Duration::hours((period as i64) * (self.interval as i64))
                    }
                    _ => chrono::Duration::minutes((period as i64) * (self.interval as i64)),
                };
                let Some(moment) = self.anchor_local.checked_add_signed(step) else {
                    return Ok(Vec::new());
                };
                let date = moment.date();
                if !self.day_matches(date) {
                    return Ok(Vec::new());
                }
                if let Some(hours) = &self.by_hour
                    && !hours.contains(&moment.hour())
                {
                    return Ok(Vec::new());
                }
                let (default_hour, default_minute, default_second) = self.default_time();
                let (hours, minutes, seconds) = match self.freq {
                    Freq::Hourly => (
                        BTreeSet::from([moment.hour()]),
                        self.by_minute
                            .clone()
                            .unwrap_or_else(|| BTreeSet::from([default_minute])),
                        self.by_second
                            .clone()
                            .unwrap_or_else(|| BTreeSet::from([default_second])),
                    ),
                    _ => (
                        self.by_hour
                            .clone()
                            .unwrap_or_else(|| BTreeSet::from([default_hour])),
                        BTreeSet::from([moment.minute()]),
                        self.by_second
                            .clone()
                            .unwrap_or_else(|| BTreeSet::from([default_second])),
                    ),
                };
                let mut times = Vec::new();
                for hour in &hours {
                    for minute in &minutes {
                        for second in &seconds {
                            if let Some(value) = date.and_hms_opt(*hour, *minute, *second) {
                                times.push(value);
                            }
                        }
                    }
                }
                return Ok(times);
            }
        }
        let mut times = Vec::new();
        for date in dates {
            if !self.day_matches(date) {
                continue;
            }
            times.extend(self.times_for(date)?);
        }
        Ok(times)
    }

    /// The index of the first period that can contain a candidate after `after`.
    fn first_period(&self, after: NaiveDateTime) -> usize {
        let anchor = self.anchor_local;
        match self.freq {
            Freq::Daily => {
                let days = after.date().signed_duration_since(anchor.date()).num_days();
                if days <= 0 {
                    0
                } else {
                    (days as usize).div_ceil(self.interval as usize)
                }
            }
            Freq::Weekly => {
                let anchor_week = anchor.date()
                    - chrono::Duration::days(
                        anchor.date().weekday().num_days_from_monday() as i64,
                    );
                let after_week = after.date()
                    - chrono::Duration::days(
                        after.date().weekday().num_days_from_monday() as i64,
                    );
                let weeks = after_week.signed_duration_since(anchor_week).num_weeks();
                if weeks <= 0 {
                    0
                } else {
                    (weeks as usize).div_ceil(self.interval as usize)
                }
            }
            Freq::Monthly => {
                let months = month_index(after.date()) - month_index(anchor.date());
                if months <= 0 {
                    0
                } else {
                    (months as usize).div_ceil(self.interval as usize)
                }
            }
            Freq::Yearly => {
                let years = after.date().year() - anchor.date().year();
                if years <= 0 {
                    0
                } else {
                    (years as usize).div_ceil(self.interval as usize)
                }
            }
            Freq::Hourly => {
                let hours = after.signed_duration_since(anchor).num_hours();
                if hours <= 0 {
                    0
                } else {
                    (hours as usize).div_ceil(self.interval as usize)
                }
            }
            Freq::Minutely => {
                let minutes = after.signed_duration_since(anchor).num_minutes();
                if minutes <= 0 {
                    0
                } else {
                    (minutes as usize).div_ceil(self.interval as usize)
                }
            }
        }
    }

    fn next_candidate(
        &self,
        after: NaiveDateTime,
    ) -> Result<Option<NaiveDateTime>, DomainError> {
        // `COUNT` bounds the rule from its anchor, so the scan has to start at the
        // anchor to know the ordinal of each candidate.
        let start = if self.count.is_some() {
            0
        } else {
            self.first_period(after)
        };
        let mut emitted = 0_usize;
        for period in start..=start.saturating_add(MAX_SCANNED_PERIODS) {
            for candidate in self.candidates_in_period(period)? {
                if let Some(count) = self.count {
                    if emitted >= count as usize {
                        return Ok(None);
                    }
                    emitted += 1;
                    if candidate > after {
                        return Ok(Some(candidate));
                    }
                    continue;
                }
                if candidate > after {
                    return Ok(Some(candidate));
                }
            }
        }
        Ok(None)
    }

    fn candidates_after(
        &self,
        from: NaiveDateTime,
        to: NaiveDateTime,
    ) -> Result<Vec<NaiveDateTime>, DomainError> {
        let mut candidates = Vec::new();
        // The lower bound is inclusive: a persisted cursor names the earliest
        // slot that has not been materialized yet.
        let mut cursor = from
            .checked_sub_signed(chrono::Duration::nanoseconds(1))
            .ok_or(DomainError::InvalidDefinition)?;
        for _ in 0..MAX_SCANNED_PERIODS {
            let Some(candidate) = self.next_candidate(cursor)? else {
                break;
            };
            if candidate > to {
                break;
            }
            candidates.push(candidate);
            cursor = candidate;
        }
        Ok(candidates)
    }
}

const MONTH_NAMES: &[&str] = &[
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];
const DAY_NAMES: &[&str] = &["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

fn parse_cron_field(
    field: &str,
    minimum: u32,
    maximum: u32,
    names: &[&str],
    name_offset: u32,
) -> Result<(BTreeSet<u32>, bool), String> {
    if field.is_empty() {
        return Err("cron field must not be empty".to_owned());
    }
    if field.contains('?') || field.contains('L') || field.contains('W') || field.contains('#') {
        return Err("cron field uses an unsupported symbol".to_owned());
    }
    let restricted = field != "*";
    let mut values = BTreeSet::new();
    for item in field.split(',') {
        let item = item.trim();
        if item.is_empty() {
            return Err("cron field item must not be empty".to_owned());
        }
        let (range_part, step) = match item.split_once('/') {
            Some((range_part, step)) => {
                let step = step
                    .parse::<u32>()
                    .ok()
                    .filter(|value| *value > 0 && *value <= maximum)
                    .ok_or("cron step must be a positive integer")?;
                (range_part, step)
            }
            None => (item, 1),
        };
        let (start, end) = if range_part == "*" {
            (minimum, maximum)
        } else if let Some((from, to)) = range_part.split_once('-') {
            (
                parse_cron_value(from, minimum, maximum, names, name_offset)?,
                parse_cron_value(to, minimum, maximum, names, name_offset)?,
            )
        } else {
            let value = parse_cron_value(range_part, minimum, maximum, names, name_offset)?;
            if step == 1 {
                (value, value)
            } else {
                (value, maximum)
            }
        };
        if start > end {
            return Err("cron range is inverted".to_owned());
        }
        let mut current = start;
        while current <= end {
            values.insert(current);
            current = match current.checked_add(step) {
                Some(value) => value,
                None => break,
            };
        }
    }
    if values.is_empty() {
        return Err("cron field produced no values".to_owned());
    }
    if values.len() > 128 {
        return Err("cron field produced too many values".to_owned());
    }
    Ok((values, restricted))
}

fn parse_cron_value(
    value: &str,
    minimum: u32,
    maximum: u32,
    names: &[&str],
    name_offset: u32,
) -> Result<u32, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("cron value must not be empty".to_owned());
    }
    if !names.is_empty()
        && let Some(index) = names
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(trimmed))
    {
        return Ok(index as u32 + name_offset);
    }
    if !names.is_empty()
        && trimmed.len() == 2
    {
        let upper = trimmed.to_ascii_uppercase();
        let matches: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.starts_with(&upper))
            .map(|(index, _)| index)
            .collect();
        if matches.len() == 1 {
            return Ok(matches[0] as u32 + name_offset);
        }
    }
    let number: u32 = trimmed
        .parse()
        .map_err(|_| format!("cron value {trimmed} is not a number"))?;
    if number < minimum || number > maximum {
        return Err(format!(
            "cron value {number} is outside {minimum}..={maximum}"
        ));
    }
    Ok(number)
}

fn parse_number_set(value: &str, minimum: u32, maximum: u32) -> Result<BTreeSet<u32>, String> {
    let mut values = BTreeSet::new();
    for item in value.split(',') {
        let item = item.trim();
        if item.is_empty() {
            return Err("rrule value list must not contain empty items".to_owned());
        }
        let number: u32 = item
            .parse()
            .map_err(|_| format!("rrule value {item} is not a number"))?;
        if number < minimum || number > maximum {
            return Err(format!(
                "rrule value {number} is outside {minimum}..={maximum}"
            ));
        }
        values.insert(number);
    }
    if values.is_empty() {
        return Err("rrule value list must not be empty".to_owned());
    }
    Ok(values)
}

fn parse_weekday_set(value: &str) -> Result<BTreeSet<u32>, String> {
    let mut values = BTreeSet::new();
    for item in value.split(',') {
        let item = item.trim();
        if item.is_empty() {
            return Err("rrule BYDAY must not contain empty items".to_owned());
        }
        if item.len() > 2 && item[..2].chars().all(|c| c.is_ascii_digit()) {
            return Err("rrule BYDAY ordinal forms are not supported".to_owned());
        }
        let index = weekday_index(item)
            .ok_or_else(|| format!("rrule BYDAY value {item} is not a weekday"))?;
        values.insert(index as u32);
    }
    if values.is_empty() {
        return Err("rrule BYDAY must not be empty".to_owned());
    }
    Ok(values)
}

/// Resolves an RFC 5545 weekday token. Both the two-letter (`MO`) and the
/// three-letter (`MON`) forms are accepted; Sunday is `0`.
fn weekday_index(value: &str) -> Option<usize> {
    let upper = value.to_ascii_uppercase();
    if let Some(index) = DAY_NAMES
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(&upper))
    {
        return Some(index);
    }
    if upper.len() == 2
        && let Some(index) = DAY_NAMES
            .iter()
            .position(|candidate| candidate.starts_with(&upper))
    {
        return Some(index);
    }
    None
}

fn parse_rrule_until(value: &str) -> Result<InstantMs, String> {
    let value = value.trim();
    let (body, utc) = match value.strip_suffix('Z') {
        Some(body) => (body, true),
        None => (value, false),
    };
    let naive = if body.len() == 8 {
        let year: i32 = body[0..4].parse().map_err(|_| "rrule UNTIL year")?;
        let month: u32 = body[4..6].parse().map_err(|_| "rrule UNTIL month")?;
        let day: u32 = body[6..8].parse().map_err(|_| "rrule UNTIL day")?;
        NaiveDate::from_ymd_opt(year, month, day)
            .ok_or("rrule UNTIL is not a valid date")?
            .and_hms_opt(0, 0, 0)
            .ok_or("rrule UNTIL is not a valid date")?
    } else if body.len() == 15 {
        let year: i32 = body[0..4].parse().map_err(|_| "rrule UNTIL year")?;
        let month: u32 = body[4..6].parse().map_err(|_| "rrule UNTIL month")?;
        let day: u32 = body[6..8].parse().map_err(|_| "rrule UNTIL day")?;
        let hour: u32 = body[9..11].parse().map_err(|_| "rrule UNTIL hour")?;
        let minute: u32 = body[11..13].parse().map_err(|_| "rrule UNTIL minute")?;
        let second: u32 = body[13..15].parse().map_err(|_| "rrule UNTIL second")?;
        NaiveDate::from_ymd_opt(year, month, day)
            .ok_or("rrule UNTIL is not a valid date")?
            .and_hms_opt(hour, minute, second)
            .ok_or("rrule UNTIL is not a valid time")?
    } else {
        return Err("rrule UNTIL must be YYYYMMDD or YYYYMMDDTHHMMSSZ".to_owned());
    };
    if !utc {
        return Err("rrule UNTIL must be expressed in UTC".to_owned());
    }
    Ok(naive.and_utc().timestamp_millis())
}

fn days_in_month(date: NaiveDate) -> u32 {
    let (year, month) = (date.year(), date.month());
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|value| value.pred_opt())
        .map(|value| value.day())
        .unwrap_or(28)
}

fn add_months(date: NaiveDate, months: i64) -> Option<NaiveDate> {
    let total = date.year() as i64 * 12 + (date.month() as i64 - 1) + months;
    let year = total.div_euclid(12) as i32;
    let month = total.rem_euclid(12) as u32 + 1;
    let day = date
        .day()
        .min(days_in_month(NaiveDate::from_ymd_opt(year, month, 1)?));
    NaiveDate::from_ymd_opt(year, month, day)
}

fn month_index(date: NaiveDate) -> i64 {
    date.year() as i64 * 12 + (date.month() as i64 - 1)
}

/// Converts a UTC instant to the local wall-clock time in `timezone`.
pub fn local_from_instant(timezone: Tz, instant: InstantMs) -> NaiveDateTime {
    chrono::DateTime::from_timestamp_millis(instant)
        .expect("schedule instants are bounded")
        .with_timezone(&timezone)
        .naive_local()
}

/// Formats an instant as a canonical UTC RFC 3339 timestamp with nanosecond
/// precision, matching the storage layer's timestamp format.
pub fn format_instant(instant: InstantMs) -> Result<String, DomainError> {
    let value =
        chrono::DateTime::from_timestamp_millis(instant).ok_or(DomainError::InvalidDefinition)?;
    Ok(value.format("%Y-%m-%dT%H:%M:%S%.9fZ").to_string())
}

/// Parses a canonical UTC RFC 3339 timestamp into an instant.
pub fn parse_instant(value: &str) -> Option<InstantMs> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.timestamp_millis())
}
