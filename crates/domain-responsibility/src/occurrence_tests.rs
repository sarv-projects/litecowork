//! Unit tests for the AutomationOccurrence lifecycle and cursor decisions.

use super::*;
use crate::DomainError;

fn facts() -> OccurrenceFacts {
    OccurrenceFacts {
        version: 1,
        claim_epoch: 0,
        claim_expires_at: None,
        now: 1_000,
        has_task: false,
    }
}

#[test]
fn occurrence_status_tokens_round_trip() {
    for status in [
        OccurrenceStatus::Pending,
        OccurrenceStatus::Claimed,
        OccurrenceStatus::WaitingDependency,
        OccurrenceStatus::Started,
        OccurrenceStatus::Completed,
        OccurrenceStatus::Skipped,
        OccurrenceStatus::Failed,
    ] {
        assert_eq!(OccurrenceStatus::parse(status.as_str()), Some(status));
    }
    assert_eq!(OccurrenceStatus::parse("BLOCKED"), None);
}

#[test]
fn claiming_an_occurrence_increments_the_version_and_the_claim_epoch() {
    let transition =
        decide_occurrence_transition(OccurrenceStatus::Pending, OccurrenceStatus::Claimed, &facts())
            .unwrap();
    assert_eq!(transition.version, 2);
    assert_eq!(transition.claim_epoch, 1);
}

#[test]
fn materializing_a_claimed_occurrence_preserves_the_claim_epoch() {
    let transition = decide_occurrence_transition(
        OccurrenceStatus::Claimed,
        OccurrenceStatus::Started,
        &OccurrenceFacts {
            version: 2,
            claim_epoch: 1,
            claim_expires_at: Some(2_000),
            now: 1_500,
            has_task: true,
        },
    )
    .unwrap();
    assert_eq!(transition.version, 3);
    assert_eq!(transition.claim_epoch, 1);
}

#[test]
fn a_terminal_occurrence_never_transitions_again() {
    assert_eq!(
        decide_occurrence_transition(
            OccurrenceStatus::Completed,
            OccurrenceStatus::Failed,
            &OccurrenceFacts {
                version: 3,
                claim_epoch: 1,
                claim_expires_at: None,
                now: 2_000,
                has_task: true,
            },
        ),
        Err(DomainError::InvalidDefinition)
    );
}

#[test]
fn a_live_claim_is_never_released_back_to_pending() {
    assert_eq!(
        decide_occurrence_transition(
            OccurrenceStatus::Claimed,
            OccurrenceStatus::Pending,
            &OccurrenceFacts {
                version: 2,
                claim_epoch: 1,
                claim_expires_at: Some(5_000),
                now: 1_000,
                has_task: false,
            },
        ),
        Err(DomainError::InvalidDefinition)
    );
}

#[test]
fn an_expired_claim_is_requeued_with_an_incremented_claim_epoch() {
    let transition = decide_occurrence_transition(
        OccurrenceStatus::Claimed,
        OccurrenceStatus::Pending,
        &OccurrenceFacts {
            version: 2,
            claim_epoch: 1,
            claim_expires_at: Some(500),
            now: 1_000,
            has_task: false,
        },
    )
    .unwrap();
    assert_eq!(transition.status, OccurrenceStatus::Pending);
    assert_eq!(transition.version, 3);
    assert_eq!(transition.claim_expires_at, None);
    assert_eq!(transition.claim_epoch, 1);
}

#[test]
fn an_illegal_transition_is_rejected() {
    assert_eq!(
        decide_occurrence_transition(OccurrenceStatus::Pending, OccurrenceStatus::Started, &facts()),
        Err(DomainError::InvalidDefinition)
    );
    assert_eq!(
        decide_occurrence_transition(
            OccurrenceStatus::Pending,
            OccurrenceStatus::Completed,
            &facts()
        ),
        Err(DomainError::InvalidDefinition)
    );
}

#[test]
fn only_a_terminal_task_outcome_settles_a_started_occurrence() {
    let started = OccurrenceFacts {
        version: 3,
        claim_epoch: 1,
        claim_expires_at: None,
        now: 2_000,
        has_task: true,
    };
    assert_eq!(
        settle_from_task_outcome("COMPLETED", &started)
            .unwrap()
            .status,
        OccurrenceStatus::Completed
    );
    assert_eq!(
        settle_from_task_outcome("FAILED", &started).unwrap().status,
        OccurrenceStatus::Failed
    );
    assert_eq!(
        settle_from_task_outcome("CANCELLED", &started)
            .unwrap()
            .status,
        OccurrenceStatus::Skipped
    );
    for unsettled in ["PAUSED", "NEEDS_USER", "VERIFYING", "RUNNING", "BLOCKED"] {
        assert_eq!(
            settle_from_task_outcome(unsettled, &started),
            Err(DomainError::InvalidDefinition),
            "{unsettled} must not settle an occurrence"
        );
    }
}

fn cursor() -> AutomationCursor {
    AutomationCursor {
        automation_id: "automation-1".into(),
        trigger_id: "trigger-weekly".into(),
        active_automation_revision: 3,
        trigger_host_runtime_id: "runtime-1".into(),
        host_epoch: 1,
        cursor_digest: "sha256:".to_owned() + &"a".repeat(64),
        last_seen_digest: None,
        last_observation_ref: None,
        next_scheduled_at: Some(1_000),
        last_checked_at: 500,
        observation_gap_since: None,
        version: 4,
    }
}

#[test]
fn a_cursor_advances_when_the_next_slot_moves() {
    let decision = decide_cursor(
        &cursor(),
        CursorFacts {
            now: 1_200,
            next_scheduled_at: Some(2_000),
            misfired: false,
        },
        "runtime-1",
    )
    .unwrap();
    assert_eq!(
        decision,
        CursorDecision::Advance {
            next_scheduled_at: Some(2_000),
            last_checked_at: 1_200,
            observation_gap_since: None,
            version: 5,
        }
    );
}

#[test]
fn a_cursor_is_unchanged_when_nothing_was_due() {
    let decision = decide_cursor(
        &cursor(),
        CursorFacts {
            now: 600,
            next_scheduled_at: Some(1_000),
            misfired: false,
        },
        "runtime-1",
    )
    .unwrap();
    assert_eq!(decision, CursorDecision::Unchanged);
}

#[test]
fn a_misfire_records_the_gap_start_without_dropping_the_deadline() {
    let decision = decide_cursor(
        &cursor(),
        CursorFacts {
            now: 9_000,
            next_scheduled_at: Some(5_000),
            misfired: true,
        },
        "runtime-1",
    )
    .unwrap();
    assert_eq!(
        decision,
        CursorDecision::Advance {
            next_scheduled_at: Some(5_000),
            last_checked_at: 9_000,
            observation_gap_since: Some(1_000),
            version: 5,
        }
    );
}

#[test]
fn a_backwards_clock_never_rewinds_the_persisted_deadline() {
    let decision = decide_cursor(
        &cursor(),
        CursorFacts {
            now: 100,
            next_scheduled_at: Some(2_000),
            misfired: false,
        },
        "runtime-1",
    )
    .unwrap();
    assert_eq!(decision, CursorDecision::Unchanged);
}

#[test]
fn a_new_trigger_host_takes_a_new_epoch_and_never_reuses_the_old_digest() {
    let decision = decide_cursor(
        &cursor(),
        CursorFacts {
            now: 1_200,
            next_scheduled_at: Some(2_000),
            misfired: false,
        },
        "runtime-2",
    )
    .unwrap();
    assert_eq!(
        decision,
        CursorDecision::TransferHost {
            trigger_host_runtime_id: "runtime-2".into(),
            host_epoch: 2,
            next_scheduled_at: Some(2_000),
            last_checked_at: 1_200,
            version: 5,
        }
    );
}

#[test]
fn an_invalid_cursor_is_rejected() {
    let mut invalid = cursor();
    invalid.host_epoch = 0;
    assert_eq!(
        decide_cursor(
            &invalid,
            CursorFacts {
                now: 1,
                next_scheduled_at: None,
                misfired: false
            },
            "runtime-1"
        ),
        Err(DomainError::InvalidDefinition)
    );
}
