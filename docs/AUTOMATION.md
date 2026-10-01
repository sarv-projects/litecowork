# Automation LLD

LiteCowork automation is intentionally small. A trigger creates an ordinary Task; there is no second workflow engine.

## Trigger types

```text
ScheduleTrigger
WebhookTrigger
ConnectorEventTrigger
ManualTrigger
```

## ScheduleTrigger

```text
ScheduleTrigger {
  timezone
  rrule_or_cron
  start_at?
  end_at?
  missed_run_policy: SKIP | RUN_LATEST | RUN_ALL_BOUNDED
  ambiguous_local_time: EARLIER | LATER
  nonexistent_local_time: SKIP | NEXT_VALID
}
```

`timezone` is an IANA time-zone identifier. Persist scheduled instants in UTC and derive
the deterministic `scheduled_key` from the automation ID plus the resolved UTC instant.
This makes daylight-saving folds explicit and deduplication stable across Runtime
restarts. Editing a schedule affects future occurrences; already claimed occurrences
retain their original trigger revision.

## Execution policy

```text
AutomationExecutionPolicy {
  placement_preference
  max_concurrent_occurrences = 1
  overlap_policy: SKIP | QUEUE | CANCEL_OLD | ALLOW
  retry_policy
  budget?
  notification_policy
}
```

## Exactly-once logical occurrence

The trigger host derives deterministic `scheduled_key` from automation + trigger instance. `AutomationOccurrence(automation_id, scheduled_key)` is unique.

Multiple trigger deliveries may occur physically; only one logical occurrence may create a Task.
This is exactly-once Task materialization by uniqueness/transaction, not a promise that
the upstream schedule/webhook is delivered once. Occurrence claim, Task creation, and
recording its `task_id` are committed atomically. If the Hub crashes before commit, the
same key can be claimed again; after commit, duplicate deliveries return the existing
Task.

## Trigger flow

1. trigger received/fired.
2. create/claim AutomationOccurrence.
3. validate Automation still enabled.
4. evaluate overlap policy.
5. materialize Task from template.
6. store Task ID on occurrence.
7. execute through ordinary Task Runtime.
8. update occurrence from Task terminal state.

Webhook triggers require an unguessable route key and a configured authentication
mechanism. Verify signature/timestamp where the source supports it, enforce replay
windows, apply body-size limits, and deduplicate a source delivery ID. A connector event
uses its provider event ID and current CapabilityGrant. Trigger payloads are untrusted
Task inputs; they cannot alter policy, permissions, budget ceilings, or secret placement.

## Condition watches

A condition watch is represented as a recurring Automation whose Task may conclude `NO_ACTION`. Notification policy decides whether to surface only when condition is met.

## Pausing/disabling

Pausing prevents new occurrences; active Tasks continue unless user separately cancels them.

Disabling prevents new occurrences permanently for that record.

## Concurrency and retry semantics

`max_concurrent_occurrences` is enforced by TriggerCoordinator at claim time. Overlap
policy is evaluated against nonterminal occurrence Tasks:

- `SKIP`: record the new occurrence as skipped.
- `QUEUE`: keep it pending until the prior occurrence settles.
- `CANCEL_OLD`: request cooperative cancellation of the prior Task; do not start the new
  Task until its attempts/effects settle.
- `ALLOW`: run concurrently only within the Workspace budget/concurrency limit.

Occurrence retry creates/reuses no duplicate Task. A failed Task may be retried through
ordinary Task recovery while the occurrence retains that Task identity. A new scheduled
occurrence gets a new key and Task. Misfire catch-up is bounded by the configured policy;
`RUN_ALL_BOUNDED` has a hard maximum retained count.

## Condition checks and notifications

A condition check that finds no change completes its ordinary Task with a structured
`NO_ACTION` result and evidence; `NO_ACTION` is a result value, not a Task status. The
notification policy may suppress a notification. When a condition is met, Task outcomes
and notifications use ordinary Task/Channel services.
