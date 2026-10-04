# Automation LLD

An Automation defines when or why a pinned Routine should create ordinary Tasks. It is
not a workflow engine, agent, or worker process. Trigger hosting and Task execution are
independent placements.

## Trigger model

An immutable `AutomationRevision` has one or more `TriggerSpec` records. Trigger semantics
are `ANY`: each trigger can independently create an occurrence; the first event does not
consume the other trigger definitions. There is no implicit AND/correlation across
triggers. Different triggers may create distinct occurrences for the same real-world
condition; cross-trigger coalescing requires a future explicit policy. Every trigger has a stable `trigger_id` across revisions when its logical source
is unchanged; a replacement source gets a new ID. Editing instructions or Routine revision
does not change a trigger's identity or replay a delivery already accepted.

```text
TriggerSpec {
  trigger_id
  placement: HUB | SPECIFIC_RUNTIME | AUTO
  runtime_id?: RuntimeId      # required only for SPECIFIC_RUNTIME
  trigger: TriggerDefinition
}

TriggerDefinition =
  ScheduleTrigger | OneShotTrigger | WebhookTrigger | ConnectorEventTrigger |
  ChannelEventTrigger | ResourceEventTrigger | TaskEventTrigger | RuntimeEventTrigger |
  ProcessExitTrigger | CommandExitTrigger | ConditionWatchTrigger | ManualTrigger
```

`HUB` requires the Workspace's authoritative Hub to advertise the trigger provider.
`SPECIFIC_RUNTIME` pins one paired Runtime and never silently substitutes another.
`AUTO` resolves to exactly one trigger host when the Automation is enabled: prefer the Hub
for schedules/webhooks/remote connector events; otherwise select one eligible Runtime
advertising the needed local trigger provider. The selected host and cursor are persisted.
Host changes require an explicit Automation revision/handoff that transfers the cursor
and fencing ownership; v1 never has two active trigger owners for one trigger.

Trigger placement controls event detection only. `AutomationExecutionPolicy` and ordinary
Task placement determine where a resulting Task runs. A cloud schedule may create a Task
that is `WAITING_DEPENDENCY` for a laptop-only folder or application; an offline laptop
does not prevent the cloud TriggerHost from recording that occurrence.

## Trigger definitions

```text
ScheduleTrigger {
  kind: SCHEDULE
  recurrence_format: CRON_5 | RFC5545_RRULE
  timezone: IanaTimeZone
  rrule_or_cron: string
  recurrence_semantics_version: u32
  start_at?: Timestamp
  end_at?: Timestamp
  misfire_policy: MisfirePolicy
  ambiguous_local_time: EARLIER | LATER
  nonexistent_local_time: SKIP | NEXT_VALID
}

OneShotTrigger {
  kind: ONE_SHOT
  scheduled_at: Timestamp
  misfire_policy: MisfirePolicy
}

WebhookTrigger {
  kind: WEBHOOK
  source_identity
  auth_profile_ref: SecretRefId
  authentication_mode: SIGNATURE | BEARER_SECRET | MUTUAL_TLS
  replay_window_ms
  max_payload_bytes
  delivery_id_field?
}

ConnectorEventTrigger { kind: CONNECTOR_EVENT, connection_id, provider_event_type, filter_expression? }
ChannelEventTrigger { kind: CHANNEL_EVENT, channel_binding_id, provider_event_type, filter_expression? }
ResourceEventTrigger { kind: RESOURCE_EVENT, workspace_root_id | resource_id, event_kinds[], debounce_ms }
TaskEventTrigger { kind: TASK_EVENT, event_kinds[], task_filter?, ignore_origin_automation_id? }
RuntimeEventTrigger { kind: RUNTIME_EVENT, runtime_id?, event_kinds[], debounce_ms }

ProcessExitTrigger {
  kind: PROCESS_EXIT
  runtime_id
  managed_process_ref
  exit_filter?
}
CommandExitTrigger {
  kind: COMMAND_EXIT
  runtime_id
  managed_execution_ref
  exit_filter?
}

ConditionWatchTrigger {
  kind: CONDITION_WATCH
  provider_ref
  source_ref: ResourceRef | CapabilityRef
  poll_interval_ms
  condition_digest
  end_condition?
}

ManualTrigger { kind: MANUAL }
```

Process/command triggers observe a process/execution LiteCowork already manages or a
specific process identity the user explicitly authorized. They are not arbitrary shell
execution hooks. Resource events are scoped to an active WorkspaceRoot or exact Resource;
watcher notifications are hints, and wake/resume performs a scoped rescan. Task-event
triggers must declare loop prevention; self-triggering chains are bounded by lineage,
depth, and Workspace concurrency limits. Connector/channel payloads are untrusted.

Trigger kinds have explicit provider support. A well-formed but unsupported kind returns
`TRIGGER_UNSUPPORTED`; it is never silently treated as another trigger. V1 can ship only
schedule, one-shot, manual, webhook, and connector-event providers. The remaining variants
are schema-compatible extension points and cannot be enabled until an authorized provider
and conformance behavior exist.

## Misfire and observation policy

```text
MisfirePolicy =
  SKIP
  | RUN_ONCE_WHEN_AVAILABLE
  | CATCH_UP_BOUNDED { max_occurrences: u32 }
```

- `SKIP`: record skipped slots as bounded `SKIPPED` occurrences/ranges with a reason;
  create no Task. Advance the cursor transactionally so skipped slots cannot replay.
- `RUN_ONCE_WHEN_AVAILABLE`: coalesce the missed interval into one logical occurrence at
  the latest missed scheduled instant; store the covered interval and count.
- `CATCH_UP_BOUNDED`: create at most `max_occurrences` individual occurrences in
  chronological order; persist a summary for excess slots rather than unbounded rows.

`NEXT_SCHEDULE_ONLY` is represented by `SKIP` for missed slots while keeping the next
future slot enabled. Recurrence grammar/semantics and DST policy are pinned; timezone-rule
updates may recalculate future unclaimed deadlines but cannot rewrite claimed occurrences.

Policies apply to known due schedule instants and one-shot deadlines. A one-shot policy
produces one SKIPPED audit occurrence with no Task (`SKIP`) or one logical runnable
occurrence (`RUN_ONCE_WHEN_AVAILABLE`);
`CATCH_UP_BOUNDED` is equivalent to a maximum of one for a one-shot. For an offline file/process watcher, the
Runtime records a cursor gap and performs a bounded rescan on resume. If the provider can
prove exact events, it resumes from its cursor; otherwise `ConditionWatch` evaluates the
current state once and coalesces changes into at most one observation. It does not invent
every intermediate file edit that happened while asleep. Wall-clock changes are handled
through persisted UTC deadlines, IANA timezone rules, DST fold/gap policy, and a monotonic
clock for in-process timers.

## Trigger cursor

```text
AutomationCursor {
  automation_id
  trigger_id
  active_automation_revision
  trigger_host_runtime_id
  host_epoch
  cursor_digest
  last_seen_digest?
  last_observation_ref?
  next_scheduled_at?
  last_checked_at
  observation_gap_since?
  version
}
```

The opaque provider cursor is persisted separately as encrypted Runtime-local state. It
may contain a bearer-like continuation token and never appears in replicated cursor
events, the Operator API, or Workspace backups.

```text
AutomationTriggerBinding { # local to one Runtime
  automation_id
  trigger_id
  trigger_host_runtime_id
  host_epoch
  cursor_ciphertext
  encryption_key_version
  cursor_digest # digest of ciphertext, never of plaintext cursor
  updated_at
  version
}
```

The binding is usable only while its Runtime and `host_epoch` match the authoritative
AutomationCursor. A new TriggerHost must obtain a cursor through an explicitly supported
provider transfer or reconcile via a bounded rescan; it cannot copy another Runtime's
opaque cursor or assume a digest can recover it. Restore without the local encryption key
marks the binding unavailable and follows the same reconciliation path.

The cursor identity is `(automation_id, trigger_id)`. An edit retaining the same logical
trigger carries its cursor forward atomically and changes `active_automation_revision`;
a changed source gets a new trigger ID. Historical cursor states remain in event history,
without creating a second active cursor. The shared cursor commits its ciphertext digest,
last observed digest/reference, schedule deadline, and version with receipt/occurrence
deduplication. Opaque provider cursor bytes remain only in the host-epoch-scoped encrypted
`AutomationTriggerBinding`; a digest cannot recover them. A changed host first obtains a
provider-supported cursor transfer or reconciles through a bounded rescan, then proves
exclusive trigger ownership. Secret material and raw unbounded payloads never enter the
shared cursor or events.

## Routine pinning and immutable Automation revisions

```text
AutomationRevision {
  automation_id
  revision
  routine_id
  routine_revision
  triggers: TriggerSpec[]
  execution_policy
  authored_by
  created_at
}
```

The selected RoutineRevision is immutable and pinned. Editing the Routine alone does not
change existing Automations. Each occurrence pins both AutomationRevision and the
specific `trigger_id`; a Task stores the rendered TaskSpec plus Routine/Automation
provenance. See `ROUTINES.md` for Routine lifecycle and user flows.

## Task inputs and trigger security

```text
TaskInputBinding {
  source_pointer: string
  template_variable: string
  value_kind: TEXT | RESOURCE_REF
  required: boolean
  max_bytes?: u32
}
```

Pointers are allowlisted when the RoutineRevision is created. A missing optional value
remains unbound; a missing required value skips the occurrence with a typed diagnostic.
Template rendering cannot select a Runtime, grant a capability, weaken approvals, select
secret placement, or raise any Workspace/Routine/Automation budget ceiling. At Task
materialization, values are copied into an immutable TaskSpecRevision with source
provenance to the occurrence and pinned input ResourceRefs.

Webhook route credentials are unguessable, Workspace-scoped, returned once, and stored
only as a digest/verifier. Verify signatures/timestamps where supported, enforce replay
windows and body limits, normalize to bounded immutable ResourceRefs, and deduplicate by
source delivery identity. A stable delivery ID reused with a different payload digest is
`OCCURRENCE_CONFLICT`; the original input is not overwritten. Connector/channel triggers
require current Connection/ChannelBinding authorization. Filter expressions are bounded,
side-effect-free predicates over normalized envelopes; they cannot invoke capabilities,
read secrets, run code, or change Task policy.

## Execution policy

```text
AutomationExecutionPolicy {
  placement_preference
  max_concurrent_occurrences: u32 = 1
  overlap_policy: SKIP | QUEUE | CANCEL_OLD | ALLOW
  retry_policy
  budget_ceiling?
  notification_policy: ALWAYS | ON_SUCCESS | ON_FAILURE | ON_CONDITION | SILENT
  wake_policy: NEVER | TRY_WAKE | REQUIRE_RUNTIME_AWAKE
}
```

Wake support is optional/best-effort and is not a placement guarantee. A required local
Runtime remains a dependency even if a WakeProvider exists. `REQUIRE_RUNTIME_AWAKE` means
the local Runtime must be present before execution admission; it does not imply an OS wake
capability. No WakeProvider is required for v1.

## Logical occurrence and idempotency

Occurrence identity is independent of AutomationRevision and includes the stable
`trigger_id`, so editing instructions cannot duplicate a delivered event. A direct
user-created Routine run is not an AutomationOccurrence. `Run now` on an Automation uses
the explicit ManualTrigger and a request-bound idempotency key. Unchanged
trigger identity across revisions also cannot rematerialize that already accepted source
event. Multiple trigger deliveries may physically repeat; the uniqueness key ensures one
logical occurrence per trigger delivery.

Canonical encoding is ASCII prefix `LiteCowork/AutomationOccurrence/v2`, NUL, then each
ordered UTF-8 field as a four-byte unsigned big-endian byte length and bytes. Hash the
complete byte sequence with SHA-256. Fields:

- schedule: `SCHEDULE`, trigger_id, resolved scheduled instant as Unix epoch milliseconds;
- one-shot: `ONE_SHOT`, trigger_id, scheduled instant;
- webhook: `WEBHOOK`, trigger_id, source identity, stable delivery ID; absent a stable ID,
  use bounded time-window start plus body digest and mark dedupe best-effort;
- connector/channel event: `CONNECTOR_EVENT`/`CHANNEL_EVENT`, trigger_id, ConnectionId/ChannelBindingId, provider event ID;
- resource/task/runtime/process/condition event: TriggerDefinition kind, trigger_id, provider event identity or
  durable source cursor plus normalized event digest;
- manual: `MANUAL`, trigger_id, authenticated PrincipalId, RequestId, AutomationId.

The v2 candidate vectors below fix UTF-8 byte-length encoding and trigger identity. They
are checked by the architecture validator; they are not implementation migration tests.
No v1 deployment or migration coverage is claimed.

```json
[
  {
    "fields": [
      "SCHEDULE",
      "trigger-weekly",
      "1791000000000"
    ],
    "occurrence_key": "3761b46716c5a9a3c4088c0dcb18cfc42dd647fea86ccf2d3149140c1d502460"
  },
  {
    "fields": [
      "MANUAL",
      "trigger-manual",
      "principal-1",
      "request-1",
      "automation-1"
    ],
    "occurrence_key": "1a29dba53c8865650745074940a438f97e44eca49c011dcfb6883cd212d7f294"
  },
  {
    "fields": [
      "CONNECTOR_EVENT",
      "trigger-github",
      "connection-1",
      "delivery-1"
    ],
    "occurrence_key": "b0b42fa53ec4eb84d1d5487f6b72f3156550746414df4c239e562d47e8e63b45"
  }
]
``` `occurrence_key`
is lowercase hexadecimal SHA-256; its transport form is stored with `sha256:` only when
typed as `Sha256Digest` (the occurrence key itself is raw lowercase hex).

## Occurrence lifecycle and waiting dependencies

```text
PENDING → CLAIMED → STARTED → COMPLETED | FAILED
                   ├→ WAITING_DEPENDENCY → STARTED
                   └→ SKIPPED
PENDING → SKIPPED
CLAIMED → PENDING only when its lease expires before Task materialization
WAITING_DEPENDENCY → SKIPPED only by explicit policy/owner decision
```

The TriggerHost authenticates/normalizes the source, computes a digest and occurrence key,
then TriggerCoordinator atomically claims `(automation_id, trigger_id, occurrence_key)`,
pins the current AutomationRevision and RoutineRevision, stores input refs, and increments
`claim_epoch`. Task creation and its occurrence `task_id` reference commit atomically.
The Task is immediately visible. If mandatory resources, Runtime, AgentEndpoint,
application, capability, secret, or policy dependencies are unavailable, the Task is
`BLOCKED` with named blockers and the occurrence is `WAITING_DEPENDENCY`; it is distinct
from a future `PENDING` occurrence. Runtime/resource/authorization events re-evaluate the
blockers. Once normal Task admission begins, occurrence becomes `STARTED`; it settles from
the ordinary Task outcome. If dependency readiness cannot be proven, it stays waiting or
is explicitly skipped by policy; no worker is launched prematurely. For a materialized
Task, skipping requires ordinary Task cancellation to settle first. Task COMPLETED maps
to occurrence COMPLETED, terminal FAILED maps to FAILED, and terminal CANCELLED maps to
SKIPPED with an explicit reason. Paused/NEEDS_USER/VERIFYING/recoverable failure remains
unsettled; Runs displays the Task state as well as occurrence state.

`claim_epoch` fences late trigger workers. A claim expiry before Task commit may be
reclaimed; after commit, retries return the same Task and cannot create a second one.
The trigger cursor, receipt, claim, occurrence, and Task materialization use one durable
idempotency boundary or a recoverable outbox protocol. No external network call occurs
inside the StateStore transaction.

## Concurrency, pause, disable, and retry

`max_concurrent_occurrences` and overlap policy are enforced against nonterminal
occurrence Tasks:

- `SKIP`: persist a skipped occurrence and reason.
- `QUEUE`: keep the due occurrence waiting until a prior run settles.
- `CANCEL_OLD`: request cooperative Task cancellation; do not start the new Task before
  prior Attempts/Invocations/Effects settle.
- `ALLOW`: run concurrently within Workspace, Runtime, and budget limits.

Pausing an Automation stops new trigger claims; it does not cancel an active Task. Disabling
stops future occurrences, while already materialized Tasks continue under their pinned
specification. Occurrences not yet materialized are retained or marked skipped with a
reason; they are never silently discarded. Editing a trigger host/source creates a new
AutomationRevision and explicit cursor handoff. A running Task is never reinterpreted by a
new revision.

Occurrence retries reuse the same Task identity and ordinary Step recovery while that Task
is nonterminal. A terminal FAILED Task is not reopened; follow-up work is a new Task and
requires an explicit manual trigger or a later logical occurrence. Provider retry/backoff
does not bypass Effect reconciliation. A Task may finish with a non-success result; an
Automation's schedule having no future runs does not certify Task success.

Condition checks use AutomationCursor state. A no-change check records a bounded
observation and may settle as `NO_ACTION`; the cursor advances atomically only after the
observation/occurrence is durably recorded. A condition change creates one occurrence with
the pinned before/after observation refs. End conditions stop future checks by pausing or
disabling the Automation through an explicit transition.
