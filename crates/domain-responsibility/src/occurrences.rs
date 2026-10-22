//! AutomationOccurrence lifecycle, claim fencing, and trigger cursor decisions.
//!
//! This module owns the pure state rules that the SQLite adapter persists. It has
//! no clock, no storage, and no Task authority: a caller supplies the current
//! aggregate facts and receives the exact transition it may commit.

use crate::DomainError;
use serde::{Deserialize, Serialize};

/// The durable status of one logical AutomationOccurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OccurrenceStatus {
    Pending,
    Claimed,
    WaitingDependency,
    Started,
    Completed,
    Skipped,
    Failed,
}

impl OccurrenceStatus {
    /// The persisted status token.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Claimed => "CLAIMED",
            Self::WaitingDependency => "WAITING_DEPENDENCY",
            Self::Started => "STARTED",
            Self::Completed => "COMPLETED",
            Self::Skipped => "SKIPPED",
            Self::Failed => "FAILED",
        }
    }

    /// Parses a persisted status token.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "PENDING" => Some(Self::Pending),
            "CLAIMED" => Some(Self::Claimed),
            "WAITING_DEPENDENCY" => Some(Self::WaitingDependency),
            "STARTED" => Some(Self::Started),
            "COMPLETED" => Some(Self::Completed),
            "SKIPPED" => Some(Self::Skipped),
            "FAILED" => Some(Self::Failed),
            _ => None,
        }
    }

    /// A terminal occurrence never transitions again.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Skipped | Self::Failed)
    }
}

/// The persisted trigger cursor for one `(automation_id, trigger_id)` pair.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationCursor {
    pub automation_id: String,
    pub trigger_id: String,
    /// The Automation revision that currently owns the trigger.
    pub active_automation_revision: u64,
    /// The Runtime that currently owns the trigger.
    pub trigger_host_runtime_id: String,
    /// The fencing epoch of the current host. Only a new host increments it.
    pub host_epoch: u64,
    /// The digest of the opaque provider cursor ciphertext, never the plaintext.
    pub cursor_digest: String,
    pub last_seen_digest: Option<String>,
    pub last_observation_ref: Option<serde_json::Value>,
    /// The earliest slot that has not been materialized yet.
    pub next_scheduled_at: Option<i64>,
    pub last_checked_at: i64,
    /// Set when the host observed a gap between the persisted deadline and now.
    pub observation_gap_since: Option<i64>,
    pub version: u64,
}

/// The decision a host may take about a persisted cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CursorDecision {
    /// The cursor is unchanged; nothing was due.
    Unchanged,
    /// The cursor advances to the supplied next slot.
    Advance {
        next_scheduled_at: Option<i64>,
        last_checked_at: i64,
        observation_gap_since: Option<i64>,
        version: u64,
    },
    /// A new host takes ownership. The cursor digest is reset because a digest
    /// cannot recover another Runtime's opaque cursor bytes.
    TransferHost {
        trigger_host_runtime_id: String,
        host_epoch: u64,
        next_scheduled_at: Option<i64>,
        last_checked_at: i64,
        version: u64,
    },
}

/// The persisted facts a cursor decision needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorFacts {
    /// The host's current instant.
    pub now: i64,
    /// The next slot the host computed, if any.
    pub next_scheduled_at: Option<i64>,
    /// True when the host observed at least one elapsed slot.
    pub misfired: bool,
}

/// Decides how a persisted cursor changes after one settlement pass.
///
/// An edit that retains the same logical trigger carries its cursor forward; a
/// changed host takes a new epoch and can never claim the previous host's opaque
/// cursor bytes.
pub fn decide_cursor(
    cursor: &AutomationCursor,
    facts: CursorFacts,
    next_host_runtime_id: &str,
) -> Result<CursorDecision, DomainError> {
    if cursor.automation_id.trim().is_empty()
        || cursor.trigger_id.trim().is_empty()
        || cursor.trigger_host_runtime_id.trim().is_empty()
        || cursor.host_epoch == 0
        || cursor.version == 0
    {
        return Err(DomainError::InvalidDefinition);
    }
    if next_host_runtime_id.trim().is_empty() {
        return Err(DomainError::InvalidDefinition);
    }
    let next_version = cursor
        .version
        .checked_add(1)
        .ok_or(DomainError::VersionOverflow)?;
    if cursor.trigger_host_runtime_id != next_host_runtime_id {
        return Ok(CursorDecision::TransferHost {
            trigger_host_runtime_id: next_host_runtime_id.to_owned(),
            host_epoch: cursor
                .host_epoch
                .checked_add(1)
                .ok_or(DomainError::VersionOverflow)?,
            next_scheduled_at: facts.next_scheduled_at,
            last_checked_at: facts.now,
            version: next_version,
        });
    }
    // A clock that moved backwards keeps the persisted deadline untouched.
    if let Some(next) = cursor.next_scheduled_at
        && next > facts.now
    {
        return Ok(CursorDecision::Unchanged);
    }
    if facts.next_scheduled_at == cursor.next_scheduled_at && !facts.misfired {
        return Ok(CursorDecision::Unchanged);
    }
    let observation_gap_since = if facts.misfired {
        Some(cursor.next_scheduled_at.unwrap_or(facts.now))
    } else {
        cursor.observation_gap_since
    };
    Ok(CursorDecision::Advance {
        next_scheduled_at: facts.next_scheduled_at,
        last_checked_at: facts.now,
        observation_gap_since,
        version: next_version,
    })
}

/// The occurrence facts a transition decision needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OccurrenceFacts {
    /// The current aggregate version.
    pub version: u64,
    /// The current claim fencing counter.
    pub claim_epoch: u64,
    /// The current claim deadline, when the occurrence is claimed.
    pub claim_expires_at: Option<i64>,
    /// The host's current instant.
    pub now: i64,
    /// True when the occurrence already links a Task.
    pub has_task: bool,
}

/// The transition a caller may commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OccurrenceTransition {
    pub status: OccurrenceStatus,
    pub version: u64,
    pub claim_epoch: u64,
    pub claim_expires_at: Option<i64>,
}

/// Decides whether an occurrence may move to the requested status.
///
/// Every persisted transition increments the aggregate version by exactly one.
/// `claim_epoch` increments only when a claim is acquired or reacquired; every
/// other transition preserves it.
pub fn decide_occurrence_transition(
    from: OccurrenceStatus,
    to: OccurrenceStatus,
    facts: &OccurrenceFacts,
) -> Result<OccurrenceTransition, DomainError> {
    if facts.version == 0 {
        return Err(DomainError::InvalidDefinition);
    }
    if from.is_terminal() {
        return Err(DomainError::InvalidDefinition);
    }
    let next_version = facts
        .version
        .checked_add(1)
        .ok_or(DomainError::VersionOverflow)?;
    let allowed = match (from, to) {
        (OccurrenceStatus::Pending, OccurrenceStatus::Claimed) => true,
        (OccurrenceStatus::Pending, OccurrenceStatus::Skipped) => true,
        (OccurrenceStatus::Claimed, OccurrenceStatus::Pending) => true,
        (OccurrenceStatus::Claimed, OccurrenceStatus::Started) => true,
        (OccurrenceStatus::Claimed, OccurrenceStatus::Skipped) => true,
        (OccurrenceStatus::Claimed, OccurrenceStatus::WaitingDependency) => true,
        (OccurrenceStatus::WaitingDependency, OccurrenceStatus::Started) => true,
        (OccurrenceStatus::WaitingDependency, OccurrenceStatus::Skipped) => true,
        (OccurrenceStatus::Started, OccurrenceStatus::Completed) => true,
        (OccurrenceStatus::Started, OccurrenceStatus::Failed) => true,
        (OccurrenceStatus::Started, OccurrenceStatus::Skipped) => true,
        _ => false,
    };
    if !allowed {
        return Err(DomainError::InvalidDefinition);
    }
    // A claim is only released back to PENDING when its lease expired before the
    // Task was materialized. A live claim is never silently dropped.
    if from == OccurrenceStatus::Claimed
        && to == OccurrenceStatus::Pending
        && facts
            .claim_expires_at
            .is_some_and(|expires_at| expires_at > facts.now)
    {
        return Err(DomainError::InvalidDefinition);
    }
    let claim_epoch = match to {
        OccurrenceStatus::Claimed => facts
            .claim_epoch
            .checked_add(1)
            .ok_or(DomainError::VersionOverflow)?,
        _ => facts.claim_epoch,
    };
    Ok(OccurrenceTransition {
        status: to,
        version: next_version,
        claim_epoch,
        claim_expires_at: match to {
            OccurrenceStatus::Claimed => Some(facts.now),
            OccurrenceStatus::Pending => None,
            _ => facts.claim_expires_at,
        },
    })
}

/// Decides how an ordinary Task outcome settles its occurrence.
///
/// Delivery of a notification, a completed schedule, and a successful Task are
/// separate facts; only the Task outcome settles the occurrence.
///
/// A Task settles its occurrence **only at a terminal Task status**. `COMPLETED`
/// maps to `COMPLETED`, terminal `FAILED` maps to `FAILED`, and terminal
/// `CANCELLED` maps to `SKIPPED`. Every other Task status — including
/// recoverable `INCOMPLETE`, plus `PAUSED`, `WAITING_USER`, `BLOCKED`,
/// `NEEDS_USER`, `VERIFYING`, `READY`, `RUNNING`, `PAUSE_REQUESTED` and
/// `CANCEL_REQUESTED` — returns `Ok(None)`: the occurrence stays unsettled at
/// `STARTED`, keeps its `max_concurrent_occurrences` slot, and is retried by the
/// next settlement pass.
///
/// This distinction is load-bearing. A recoverable `INCOMPLETE` Task may still
/// move back to `READY` and run again, so settling it as terminal `FAILED` would
/// (a) publish a terminal outcome that its authoritative Task state does not
/// support, (b) release a concurrency slot while the Task can still run, and
/// (c) make a later successful `COMPLETED` settlement impossible, because a
/// terminal occurrence never transitions again.
///
/// An unrecognized token is a contract violation, not a pending state, so it is
/// rejected as [`DomainError::InvalidDefinition`].
pub fn settle_from_task_outcome(
    outcome: &str,
    facts: &OccurrenceFacts,
) -> Result<Option<OccurrenceTransition>, DomainError> {
    let status = match outcome {
        "COMPLETED" => OccurrenceStatus::Completed,
        "FAILED" => OccurrenceStatus::Failed,
        "CANCELLED" => OccurrenceStatus::Skipped,
        // Terminal Task statuses settle; every other status stays unsettled.
        "READY" | "RUNNING" | "VERIFYING" | "BLOCKED" | "NEEDS_USER" | "INCOMPLETE"
        | "PAUSED" | "PAUSE_REQUESTED" | "WAITING_USER" | "CANCEL_REQUESTED" => return Ok(None),
        _ => {
            return Err(DomainError::InvalidDefinition);
        }
    };
    decide_occurrence_transition(OccurrenceStatus::Started, status, facts).map(Some)
}
