//! Local notification policy and delivery decisions.
//!
//! Notifications are local-only. A delivery record is created because a
//! persisted outcome exists, never because an agent said something. Delivery
//! itself is only reported as sent when the local consumer proved it; a record
//! that nobody consumed stays pending and is never claimed as delivered.
//!
//! The rules come from [AUTOMATION.md](../../docs/AUTOMATION.md) and
//! [RESPONSIBILITIES.md](../../docs/RESPONSIBILITIES.md):
//!
//! - a notification is a separate fact from the Task outcome it reports;
//! - one notification is produced per settled occurrence, never per worker
//!   token or Step;
//! - a duplicate delivery of the same logical outcome is suppressed;
//! - a policy of `SILENT` never produces a delivery.

use crate::{AutomationNotification, DomainError};
use chrono::Timelike;
use serde::{Deserialize, Serialize};

/// The persisted event classes a notification preference can address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationEventClass {
    /// A triggered occurrence settled into an ordinary Task.
    AutomationOccurrenceSettled,
    /// A due slot was recorded as a bounded skipped occurrence.
    AutomationOccurrenceSkipped,
    /// The ordinary Task of an occurrence reached a terminal successful state.
    TaskCompleted,
    /// The ordinary Task of an occurrence reached a terminal failed state.
    TaskFailed,
    /// The occurrence or its Task is waiting on a named dependency.
    DependencyBlocked,
}

impl NotificationEventClass {
    /// The persisted token.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AutomationOccurrenceSettled => "AUTOMATION_OCCURRENCE_SETTLED",
            Self::AutomationOccurrenceSkipped => "AUTOMATION_OCCURRENCE_SKIPPED",
            Self::TaskCompleted => "TASK_COMPLETED",
            Self::TaskFailed => "TASK_FAILED",
            Self::DependencyBlocked => "DEPENDENCY_BLOCKED",
        }
    }

    /// Parses a persisted token.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "AUTOMATION_OCCURRENCE_SETTLED" => Some(Self::AutomationOccurrenceSettled),
            "AUTOMATION_OCCURRENCE_SKIPPED" => Some(Self::AutomationOccurrenceSkipped),
            "TASK_COMPLETED" => Some(Self::TaskCompleted),
            "TASK_FAILED" => Some(Self::TaskFailed),
            "DEPENDENCY_BLOCKED" => Some(Self::DependencyBlocked),
            _ => None,
        }
    }
}

/// The outcome a notification reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationOutcome {
    /// The occurrence settled into an ordinary Task.
    OccurrenceSettled,
    /// The due slot was recorded as skipped.
    OccurrenceSkipped,
    /// The ordinary Task completed successfully.
    Succeeded,
    /// The ordinary Task failed.
    Failed,
    /// The occurrence or its Task waits on a named dependency.
    Blocked,
}

/// Why a notification was not delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationSuppressReason {
    /// The governing policy is `SILENT`.
    SilentPolicy,
    /// The governing policy does not address this outcome.
    PolicyMismatch,
    /// The workspace preference is `SILENT` for this event class.
    ClassDisabled,
    /// The delivery would duplicate an already recorded outcome.
    Duplicate,
    /// The local time falls inside the configured quiet hours.
    QuietHours,
    /// No local channel is bound for the Workspace.
    NoChannel,
}

impl NotificationSuppressReason {
    /// The persisted token.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SilentPolicy => "SILENT_POLICY",
            Self::PolicyMismatch => "POLICY_MISMATCH",
            Self::ClassDisabled => "CLASS_DISABLED",
            Self::Duplicate => "DUPLICATE",
            Self::QuietHours => "QUIET_HOURS",
            Self::NoChannel => "NO_CHANNEL",
        }
    }
}

/// The decision about one logical outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotificationDecision {
    /// Create one local delivery record.
    Deliver {
        class: NotificationEventClass,
        /// The Workspace-scoped dedupe key of the logical outcome.
        dedupe_key: String,
    },
    /// Create no delivery record.
    Suppress {
        class: NotificationEventClass,
        reason: NotificationSuppressReason,
    },
}

/// The bounded quiet-hours window of a Workspace notification preference.
///
/// The persisted shape is exactly
/// `{"start":"HH:MM","end":"HH:MM","timezone":"<IANA zone>"}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuietHours {
    /// Minutes after local midnight, inclusive.
    pub start_minute: u16,
    /// Minutes after local midnight, exclusive.
    pub end_minute: u16,
    pub timezone: String,
}

impl QuietHours {
    /// Parses the bounded persisted shape.
    pub fn parse(value: &serde_json::Value) -> Result<Self, DomainError> {
        let object = value
            .as_object()
            .ok_or(DomainError::InvalidDefinition)?;
        if object.len() != 3 {
            return Err(DomainError::InvalidDefinition);
        }
        let start = parse_minute(
            object
                .get("start")
                .and_then(serde_json::Value::as_str)
                .ok_or(DomainError::InvalidDefinition)?,
        )?;
        let end = parse_minute(
            object
                .get("end")
                .and_then(serde_json::Value::as_str)
                .ok_or(DomainError::InvalidDefinition)?,
        )?;
        let timezone = object
            .get("timezone")
            .and_then(serde_json::Value::as_str)
            .ok_or(DomainError::InvalidDefinition)?;
        if timezone.trim().is_empty() || timezone.len() > 64 {
            return Err(DomainError::InvalidDefinition);
        }
        Ok(Self {
            start_minute: start,
            end_minute: end,
            timezone: timezone.to_owned(),
        })
    }

    /// True when the supplied instant falls inside the quiet window.
    pub fn contains(&self, now_epoch_ms: i64) -> Result<bool, DomainError> {
        let timezone: chrono_tz::Tz = self
            .timezone
            .parse()
            .map_err(|_| DomainError::InvalidDefinition)?;
        let local = chrono::DateTime::from_timestamp_millis(now_epoch_ms)
            .ok_or(DomainError::InvalidDefinition)?
            .with_timezone(&timezone);
        let minute = (local.minute() as u16) + (local.hour() as u16) * 60;
        if self.start_minute == self.end_minute {
            return Ok(false);
        }
        if self.start_minute < self.end_minute {
            Ok(minute >= self.start_minute && minute < self.end_minute)
        } else {
            // The window wraps past local midnight.
            Ok(minute >= self.start_minute || minute < self.end_minute)
        }
    }
}

fn parse_minute(value: &str) -> Result<u16, DomainError> {
    let bytes = value.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return Err(DomainError::InvalidDefinition);
    }
    let hours: u16 = value[..2]
        .parse()
        .map_err(|_| DomainError::InvalidDefinition)?;
    let minutes: u16 = value[3..]
        .parse()
        .map_err(|_| DomainError::InvalidDefinition)?;
    if hours > 23 || minutes > 59 {
        return Err(DomainError::InvalidDefinition);
    }
    Ok(hours * 60 + minutes)
}

/// The facts one notification decision reads.
#[derive(Clone, Debug)]
pub struct NotificationFacts<'a> {
    /// The Automation's own notification policy.
    pub automation_policy: AutomationNotification,
    /// The Workspace preference for the event class, when one exists.
    pub workspace_policy: Option<AutomationNotification>,
    /// The settled outcome.
    pub outcome: NotificationOutcome,
    /// The Workspace-scoped dedupe key of the logical outcome.
    pub dedupe_key: &'a str,
    /// The Workspace quiet hours, when configured.
    pub quiet_hours: Option<&'a QuietHours>,
    /// The current instant, in Unix epoch milliseconds.
    pub now_epoch_ms: i64,
    /// True when a delivery for this dedupe key already exists.
    pub already_delivered: bool,
    /// True when the Workspace has at least one local channel binding.
    pub has_local_channel: bool,
}

/// The event class an outcome belongs to.
pub fn event_class_for(outcome: NotificationOutcome) -> NotificationEventClass {
    match outcome {
        NotificationOutcome::OccurrenceSettled => NotificationEventClass::AutomationOccurrenceSettled,
        NotificationOutcome::OccurrenceSkipped => NotificationEventClass::AutomationOccurrenceSkipped,
        NotificationOutcome::Succeeded => NotificationEventClass::TaskCompleted,
        NotificationOutcome::Failed => NotificationEventClass::TaskFailed,
        NotificationOutcome::Blocked => NotificationEventClass::DependencyBlocked,
    }
}

/// Decides whether one settled outcome produces a local notification.
///
/// A delivery record is only proposed for an outcome the policy addresses. The
/// decision never claims that the notification was delivered: delivery is
/// proven separately by the local consumer.
pub fn decide_notification(facts: &NotificationFacts<'_>) -> NotificationDecision {
    let class = event_class_for(facts.outcome);
    if facts.dedupe_key.trim().is_empty() {
        return NotificationDecision::Suppress {
            class,
            reason: NotificationSuppressReason::Duplicate,
        };
    }
    if facts.automation_policy == AutomationNotification::Silent {
        return NotificationDecision::Suppress {
            class,
            reason: NotificationSuppressReason::SilentPolicy,
        };
    }
    if facts.workspace_policy == Some(AutomationNotification::Silent) {
        return NotificationDecision::Suppress {
            class,
            reason: NotificationSuppressReason::ClassDisabled,
        };
    }
    if !policy_addresses(facts.automation_policy, facts.outcome) {
        return NotificationDecision::Suppress {
            class,
            reason: NotificationSuppressReason::PolicyMismatch,
        };
    }
    if facts.already_delivered {
        return NotificationDecision::Suppress {
            class,
            reason: NotificationSuppressReason::Duplicate,
        };
    }
    if !facts.has_local_channel {
        return NotificationDecision::Suppress {
            class,
            reason: NotificationSuppressReason::NoChannel,
        };
    }
    if let Some(quiet) = facts.quiet_hours
        && quiet.contains(facts.now_epoch_ms).unwrap_or(false)
    {
        return NotificationDecision::Suppress {
            class,
            reason: NotificationSuppressReason::QuietHours,
        };
    }
    NotificationDecision::Deliver {
        class,
        dedupe_key: facts.dedupe_key.to_owned(),
    }
}

fn policy_addresses(policy: AutomationNotification, outcome: NotificationOutcome) -> bool {
    match policy {
        AutomationNotification::Always => true,
        AutomationNotification::OnSuccess => matches!(
            outcome,
            NotificationOutcome::Succeeded | NotificationOutcome::OccurrenceSettled
        ),
        AutomationNotification::OnFailure => matches!(
            outcome,
            NotificationOutcome::Failed | NotificationOutcome::Blocked
        ),
        AutomationNotification::OnCondition => matches!(outcome, NotificationOutcome::Blocked),
        AutomationNotification::Silent => false,
    }
}
