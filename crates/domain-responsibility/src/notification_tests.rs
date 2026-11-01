//! Unit tests for the local notification policy decisions.

use super::*;

fn facts<'a>(
    automation_policy: AutomationNotification,
    outcome: NotificationOutcome,
    dedupe_key: &'a str,
) -> NotificationFacts<'a> {
    NotificationFacts {
        automation_policy,
        workspace_policy: None,
        outcome,
        dedupe_key,
        quiet_hours: None,
        now_epoch_ms: 0,
        already_delivered: false,
        has_local_channel: true,
    }
}

#[test]
fn a_silent_policy_never_delivers() {
    let decision = decide_notification(&facts(
        AutomationNotification::Silent,
        NotificationOutcome::Succeeded,
        "occurrence-1",
    ));
    assert!(matches!(
        decision,
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::SilentPolicy,
            ..
        }
    ));
}

#[test]
fn an_always_policy_delivers_the_logical_outcome() {
    let decision = decide_notification(&facts(
        AutomationNotification::Always,
        NotificationOutcome::Succeeded,
        "occurrence-1",
    ));
    assert_eq!(
        decision,
        NotificationDecision::Deliver {
            class: NotificationEventClass::TaskCompleted,
            dedupe_key: "occurrence-1".to_owned(),
        }
    );
}

#[test]
fn the_on_success_policy_ignores_a_failure() {
    let decision = decide_notification(&facts(
        AutomationNotification::OnSuccess,
        NotificationOutcome::Failed,
        "occurrence-1",
    ));
    assert!(matches!(
        decision,
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::PolicyMismatch,
            ..
        }
    ));
}

#[test]
fn the_on_failure_policy_ignores_a_success() {
    let decision = decide_notification(&facts(
        AutomationNotification::OnFailure,
        NotificationOutcome::Succeeded,
        "occurrence-1",
    ));
    assert!(matches!(
        decision,
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::PolicyMismatch,
            ..
        }
    ));
}

#[test]
fn the_on_condition_policy_only_reports_a_blocker() {
    let blocked = decide_notification(&facts(
        AutomationNotification::OnCondition,
        NotificationOutcome::Blocked,
        "occurrence-1",
    ));
    assert_eq!(
        blocked,
        NotificationDecision::Deliver {
            class: NotificationEventClass::DependencyBlocked,
            dedupe_key: "occurrence-1".to_owned(),
        }
    );
    let settled = decide_notification(&facts(
        AutomationNotification::OnCondition,
        NotificationOutcome::OccurrenceSettled,
        "occurrence-1",
    ));
    assert!(matches!(
        settled,
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::PolicyMismatch,
            ..
        }
    ));
}

#[test]
fn a_duplicate_logical_outcome_is_suppressed() {
    let mut facts = facts(
        AutomationNotification::Always,
        NotificationOutcome::Succeeded,
        "occurrence-1",
    );
    facts.already_delivered = true;
    let decision = decide_notification(&facts);
    assert!(matches!(
        decision,
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::Duplicate,
            ..
        }
    ));
}

#[test]
fn a_workspace_without_a_local_channel_never_claims_a_delivery() {
    let mut facts = facts(
        AutomationNotification::Always,
        NotificationOutcome::Succeeded,
        "occurrence-1",
    );
    facts.has_local_channel = false;
    let decision = decide_notification(&facts);
    assert!(matches!(
        decision,
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::NoChannel,
            ..
        }
    ));
}

#[test]
fn a_workspace_preference_can_disable_one_event_class() {
    let mut facts = facts(
        AutomationNotification::Always,
        NotificationOutcome::Succeeded,
        "occurrence-1",
    );
    facts.workspace_policy = Some(AutomationNotification::Silent);
    let decision = decide_notification(&facts);
    assert!(matches!(
        decision,
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::ClassDisabled,
            ..
        }
    ));
}

#[test]
fn an_empty_dedupe_key_is_never_delivered() {
    let decision = decide_notification(&facts(
        AutomationNotification::Always,
        NotificationOutcome::Succeeded,
        "",
    ));
    assert!(matches!(
        decision,
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::Duplicate,
            ..
        }
    ));
}

#[test]
fn the_quiet_hours_shape_is_bounded() {
    // The persisted shape is the owner-facing preference shape, so a stored
    // preference can be evaluated instead of failing to parse.
    let value = serde_json::json!({
        "timezone": "UTC",
        "weekdays": ["SUNDAY", "MONDAY"],
        "start_local_time": "22:00",
        "end_local_time": "07:00"
    });
    let quiet = QuietHours::parse(&value).expect("the quiet hours are valid");
    assert_eq!(quiet.start_minute, 22 * 60);
    assert_eq!(quiet.end_minute, 7 * 60);
    assert_eq!(quiet.weekdays, vec!["SUNDAY".to_owned(), "MONDAY".to_owned()]);
    assert!(
        QuietHours::parse(&serde_json::json!({"start": "22:00", "end": "07:00", "timezone": "UTC"}))
            .is_err()
    );
    assert!(QuietHours::parse(&serde_json::json!({
        "timezone": "UTC",
        "weekdays": ["SUNDAY"],
        "start_local_time": "24:00",
        "end_local_time": "07:00"
    }))
    .is_err());
    assert!(QuietHours::parse(&serde_json::json!({
        "timezone": "UTC",
        "weekdays": [],
        "start_local_time": "22:00",
        "end_local_time": "07:00"
    }))
    .is_err());
    assert!(QuietHours::parse(&serde_json::json!({
        "timezone": "UTC",
        "weekdays": ["FUNDAY"],
        "start_local_time": "22:00",
        "end_local_time": "07:00"
    }))
    .is_err());
    assert!(QuietHours::parse(&serde_json::json!({
        "timezone": "UTC",
        "weekdays": ["SUNDAY", "SUNDAY"],
        "start_local_time": "22:00",
        "end_local_time": "07:00"
    }))
    .is_err());
    assert!(QuietHours::parse(&serde_json::json!({
        "timezone": "UTC",
        "weekdays": ["SUNDAY"],
        "start_local_time": "22:00",
        "end_local_time": "07:00",
        "x": 1
    }))
    .is_err());
}

#[test]
fn a_quiet_window_suppresses_a_delivery() {
    let quiet = QuietHours::parse(&serde_json::json!({
        "timezone": "UTC",
        "weekdays": ["SUNDAY"],
        "start_local_time": "22:00",
        "end_local_time": "07:00"
    }))
    .expect("the quiet hours are valid");
    let mut facts = facts(
        AutomationNotification::Always,
        NotificationOutcome::Succeeded,
        "occurrence-1",
    );
    facts.quiet_hours = Some(&quiet);
    facts.now_epoch_ms = crate::parse_instant("2026-03-01T23:00:00Z")
        .expect("the test instant is valid");
    let decision = decide_notification(&facts);
    assert!(matches!(
        decision,
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::QuietHours,
            ..
        }
    ));
    // An overnight window belongs to the weekday it starts on, so the Monday
    // 01:00 tail of a Sunday window is still quiet.
    facts.now_epoch_ms = crate::parse_instant("2026-03-02T01:00:00Z")
        .expect("the test instant is valid");
    assert!(matches!(
        decide_notification(&facts),
        NotificationDecision::Suppress {
            reason: NotificationSuppressReason::QuietHours,
            ..
        }
    ));
    // The window only applies on the configured weekdays: the same local time on a
    // Monday is not quiet because Monday is not in the stored list.
    facts.now_epoch_ms = crate::parse_instant("2026-03-02T23:00:00Z")
        .expect("the test instant is valid");
    assert!(matches!(
        decide_notification(&facts),
        NotificationDecision::Deliver { .. }
    ));
    facts.now_epoch_ms = crate::parse_instant("2026-03-01T12:00:00Z")
        .expect("the test instant is valid");
    assert!(matches!(
        decide_notification(&facts),
        NotificationDecision::Deliver { .. }
    ));
}

#[test]
fn the_event_class_tokens_round_trip() {
    for class in [
        NotificationEventClass::AutomationOccurrenceSettled,
        NotificationEventClass::AutomationOccurrenceSkipped,
        NotificationEventClass::TaskCompleted,
        NotificationEventClass::TaskFailed,
        NotificationEventClass::DependencyBlocked,
    ] {
        assert_eq!(
            NotificationEventClass::parse(class.as_str()),
            Some(class),
            "the event class token must round trip"
        );
    }
    assert_eq!(NotificationEventClass::parse("UNKNOWN"), None);
}
