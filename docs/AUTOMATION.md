# Automation LLD

LiteCowork automation is intentionally small. A trigger creates an ordinary Task; there is no second workflow engine.

## Trigger specifications

Every revision contains exactly one discriminated TriggerSpec. Trigger parameters are
validated and immutable within AutomationRevision.

```text
TriggerSpec =
  ScheduleTrigger
  | WebhookTrigger
  | ConnectorEventTrigger
  | ManualTrigger

WebhookTrigger {
  kind: WEBHOOK
  source_identity
  auth_profile_ref: SecretRefId
  authentication_mode: SIGNATURE | BEARER_SECRET | MUTUAL_TLS
  replay_window_ms
  max_payload_bytes
  delivery_id_field?
}

ConnectorEventTrigger {
  kind: CONNECTOR_EVENT
  connection_id
  provider_event_type
  filter_expression?
}

ManualTrigger {
  kind: MANUAL
}
```

Webhook secret material is never part of AutomationRevision or the inbound payload.
source_identity is a non-secret stable label for the source; auth_profile_ref identifies a
separately managed SecretRef. Webhook routes are
Workspace-scoped and unguessable; the raw route credential is returned only at creation
and can be rotated/revoked without rewriting Task history. Connector triggers bind to a
Workspace Connection and provider event type; payload fields remain untrusted. Manual
runs use the normalized RequestId (HTTP Idempotency-Key or local IPC request_id) as stable occurrence identity.

A filter expression is a bounded, side-effect-free data predicate over the normalized
event envelope. It cannot call capabilities, change Task policy, access secrets, or
execute arbitrary code. A missing/invalid event field evaluates as non-match and is
reported in occurrence diagnostics.

## Immutable Automation revisions

Automation is a mutable lifecycle/current-revision pointer. Each definition edit appends an immutable AutomationRevision containing TriggerSpec, TaskTemplate, ExecutionPolicy, author, and creation time. Occurrences pin the revision whose definition produced them. Pause/resume/disable does not create a new definition revision. A running or pending occurrence is never reinterpreted after an edit.

## ScheduleTrigger

```text
ScheduleTrigger {
  kind: SCHEDULE
  timezone
  rrule_or_cron
  start_at?
  end_at?
  missed_run_policy: SKIP | RUN_LATEST | RUN_ALL_BOUNDED
  ambiguous_local_time: EARLIER | LATER
  nonexistent_local_time: SKIP | NEXT_VALID
}
```

`timezone` is an IANA time-zone identifier. Persist scheduled instants in UTC. Each schedule occurrence key is independent of AutomationRevision and is derived from the resolved UTC instant, so daylight-saving folds are explicit and retries remain stable across edits and Runtime restarts.

## Task template

```text
TaskTemplate {
  objective_template
  constraints[]
  non_goals[]
  required_outputs[]
  acceptance_criteria[]
  approvals_required[]
  input_bindings[]
  budget_ceiling?
}
```

Input bindings map an allowlisted trigger-envelope field to a typed ResourceRef or
bounded text value. Template rendering cannot select a Runtime, grant a capability,
weaken an Approval requirement, choose secret placement, or raise Workspace/Automation
budget ceilings. At occurrence materialization, values are resolved from the occurrence's bounded normalized
trigger-input ResourceRef and copied into a new immutable TaskSpecRevision; source
provenance references both the occurrence and that input ResourceRef.

```text
TaskInputBinding {
  source_pointer: string          # JSON Pointer into the normalized trigger envelope
  template_variable: string       # name substituted in the objective template
  value_kind: TEXT | RESOURCE_REF
  required: boolean
  max_bytes?: u32                 # mandatory for TEXT
}
```

Pointers are allowlisted when the AutomationRevision is created. Missing optional values
remain unbound; a missing required value skips the occurrence with an explicit diagnostic.

## Execution policy

```text
AutomationExecutionPolicy {
  placement_preference: PlacementPreference
  max_concurrent_occurrences: u32 = 1
  overlap_policy: SKIP | QUEUE | CANCEL_OLD | ALLOW
  retry_policy: RetryPolicy
  budget_ceiling?: BudgetSpec
  notification_policy: ALWAYS | ON_SUCCESS | ON_FAILURE | ON_CONDITION | SILENT
  misfire_max_count?: u32
}
```

## Exactly-once logical occurrence

The trigger host derives a deterministic `occurrence_key` from the trigger identity. The database unique key is `(automation_id, occurrence_key)`; the definition revision is deliberately excluded so an edit cannot replay an already-delivered trigger.

### Canonical occurrence-key encoding

Encode ASCII prefix `LiteCowork/AutomationOccurrence/v1`, one NUL byte, then each ordered UTF-8 field as a four-byte unsigned big-endian byte length followed by the field bytes. `occurrence_key` is lowercase hexadecimal SHA-256 of the complete byte sequence. Trigger identity fields are:

- schedule: `SCHEDULE`, resolved scheduled instant as Unix epoch milliseconds;
- webhook: `WEBHOOK`, provider identity, stable source delivery ID; when the source has no stable ID, use a bounded time-window start plus the body digest and treat deduplication as best-effort;
- connector event: `CONNECTOR`, ConnectionId, provider event ID;
- manual run: `MANUAL`, authenticated PrincipalId, RequestId.

A fixed test vector is `SCHEDULE` at `1790899200000` -> `d556b01bdffef3f148eee549f3c1ac14c92f4be7437d281e1ccb2593dc0394da`.

Multiple trigger deliveries may occur physically; only one logical occurrence may create a Task.
This is exactly-once Task materialization by uniqueness/transaction, not a promise that
the upstream schedule/webhook is delivered once. Occurrence claim, Task creation, and
recording its `task_id` are committed atomically. If the Hub crashes before commit, the
same key can be claimed again; after commit, duplicate deliveries return the existing
Task. Reclaim increments claim_epoch, and all materialize/settle commands require the current
epoch. A payload digest/ref is retained separately from the occurrence key so retries do
not change trigger identity.

## Trigger flow

1. Trigger host authenticates the source, normalizes any external payload, computes its digest, and resolves a bounded content-addressed ResourceRef; raw secret material is excluded.
2. Trigger host derives the revision-independent occurrence key from stable trigger identity.
3. TriggerCoordinator atomically claims `(automation_id, occurrence_key)`, checks enabled status, pins the current AutomationRevision, stores the input ref/digest, and increments claim_epoch.
4. Duplicate delivery with the same payload digest returns the existing logical occurrence/Task. Reuse of the same stable delivery ID with a different digest is rejected as OCCURRENCE_CONFLICT and audited; it cannot overwrite the original input. A reclaimed claim has a higher epoch; stale claimants cannot create or settle a Task.
5. Overlap policy is evaluated against the pinned revision.
6. Task creation from the pinned template/input ref and the occurrence Task reference commit atomically.
7. Task executes through ordinary Task Runtime; occurrence settles from Task outcome and the current claim epoch.
8. Result artifact/notification follows ordinary Artifact/Channel rules.

Webhook triggers require an unguessable route key and a configured authentication
mechanism. Verify signature/timestamp where the source supports it, enforce replay
windows, apply body-size limits, and deduplicate a source delivery ID. A connector event
uses its provider event ID and current CapabilityGrant. Trigger payloads are untrusted
Task inputs; they cannot alter policy, permissions, budget ceilings, or secret placement.
Webhook and connector payloads are normalized to a bounded immutable Artifact/ResourceRef
before Task materialization; AutomationOccurrence stores that ref and its digest, never raw
credentials or an unbounded provider body. Manual trigger values pass the same allowlist
and size/type validation.

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
ordinary Task recovery while the occurrence retains that Task identity. Claim expiry before
Task creation returns the occurrence to PENDING with a higher claim_epoch; expiry after
Task creation does not reclaim it for a second Task, and settlement requires the current epoch. A new logical trigger occurrence gets a new key and Task. Misfire catch-up is bounded by the configured policy;
`RUN_ALL_BOUNDED` has a hard maximum retained count.

## Condition checks and notifications

A condition check that finds no change completes its ordinary Task with a structured
`NO_ACTION` result and evidence; `NO_ACTION` is a result value, not a Task status. The
notification policy may suppress a notification. When a condition is met, Task outcomes
and notifications use ordinary Task/Channel services.
