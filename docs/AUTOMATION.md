# Automation LLD

> **Critical V1 settlement boundary (2026-10-22; producer landed 2026-10-22):**
> Occurrence completion is derived from authoritative Task outcome, not schedule
> firing, UI status or an agent report. Only a **terminal** Task status settles an
> occurrence: `COMPLETED` → `COMPLETED`, terminal `FAILED` → `FAILED`,
> `CANCELLED` → `SKIPPED` with an explicit reason. Every other status — including
> recoverable `INCOMPLETE`, plus `PAUSED`, `WAITING_USER`, `BLOCKED`, `NEEDS_USER`,
> `VERIFYING`, `READY`, `RUNNING`, `PAUSE_REQUESTED`, `CANCEL_REQUESTED` — leaves the
> occurrence at `STARTED`, keeps holding its `max_concurrent_occurrences` slot, and is
> retried by the next pass. The earlier `INCOMPLETE` → `FAILED` mapping is repaired.
>
> The producer is `SqliteAutomationHostStore::settle_started_occurrences_from_task_outcomes`,
> which commits the occurrence update, its `automation.occurrence.settled.v1` event and
> one local notification decision in a single transaction, and which the served local
> trigger-host settlement pass runs before it counts non-terminal occurrences. A replay
> finds no `STARTED` row and settles nothing.
>
> Still release-blocking: trigger-host cursor retirement on disable, real OS
> notification settlement (delivery attempts, `notification.delivery.changed.v1`, the
> local OS bridge), and owner USER evidence for the whole journey.
> See [release matrix](../implementation/V1-PRODUCT-ACCEPTANCE-MATRIX.md).

An Automation defines when or why a pinned Routine should create ordinary Tasks. It is
not a workflow engine, agent, or worker process. Trigger hosting and Task execution are
independent placements.

## Starter template gallery (V1 desktop)

The Automation destination includes a curated starter gallery with two top-level families:
**Coding** and **General cowork**. Example cards include “Review a repository change and
prepare a patch for approval” and “Prepare a weekly team update from selected sources.”
Each card is a versioned Routine template with a readable objective, required inputs,
capability/account prerequisites, execution location, output format, approval points, and
an example result preview. The preview is static and inert: it does not install packages,
connect accounts, assign capabilities, grant access, create an enabled trigger, or start a
Task.

Choosing a card creates an editable **Routine draft** only. To make it recurring, the
owner separately reviews and saves the Routine, chooses a supported trigger and Workspace,
checks timezone/misfire/overlap/retry/budget behavior, previews required permissions and
output, and explicitly enables the Automation. To run once, the owner selects Run now and
confirms ordinary Task admission. A missing agent, account, LiteSPM package, runtime,
source, or OS dependency remains a named prerequisite; the gallery never makes it look
installed or ready. A gallery template is not a LiteSPM package and cannot widen Trust
policy.

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
AutomationOccurrence.version is the aggregate revision. Creation persists PENDING at
version 1. Every later persisted transition increments version by exactly one: claim,
expired-claim requeue, reclaim, materialization, dependency wait/resume, and terminal
settlement. claim_epoch is a separate fencing counter: only acquiring/reacquiring a claim
increments it; all other transitions preserve it. Every event and aggregate snapshot uses
the resulting version as entity_revision. Transition requests carry expected version and,
when acting under a claim, expected claim_epoch.
```

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
  coworker_ref?: CoworkerRevisionRef
  authored_by
  created_at
}
```

The selected RoutineRevision is immutable and pinned. Editing the Routine alone does not
change existing Automations. Each occurrence pins both AutomationRevision and the
specific `trigger_id`; a Task stores the rendered TaskSpec plus Routine/Automation
provenance. See `ROUTINES.md` for Routine lifecycle and user flows.

When `coworker_ref` is present, it pins the Coworker revision whose lead, worker allowlist,
delegation strategy, budget defaults, context policy, and interaction defaults are used to
render each occurrence Task. The Coworker aggregate's current status is checked separately
at occurrence admission; `PAUSED` or `ARCHIVED` blocks proactive scheduled materialization.
Changing the Coworker does not rewrite an AutomationRevision. The owner must revise the
Automation to adopt another Coworker revision. Historical Tasks retain the revision that
their pinned AutomationRevision selected.

Definition create and revise requests explicitly carry either an exact
`CoworkerRevisionRef` or `null`. The desktop selector pins the active Coworker's current
revision when selected; editing an existing definition defaults to preserving its exact
existing pin, including an older revision. Clearing or changing the pin is an explicit
edit. Saving still creates a `PAUSED` Automation and does not admit a Task or run a trigger.

## Exact-revision test runs

An owner may create a real test Task from a saved AutomationRevision without creating an
occurrence. The test Task pins the exact AutomationRevision and its exact RoutineRevision,
uses the pinned Coworker revision when one is present (otherwise ordinary Routine/Workspace
lead admission), and is recorded with `automation_test_run_id`. The Automation must be
`PAUSED` or `ENABLED`; `DISABLED` is terminal and refuses new test runs. A test run requires
no trigger identity and no TriggerHost binding, writes no AutomationOccurrence, and never
advances or rewinds an AutomationCursor. The resulting Task is an ordinary authorized READY
Task: it may start the same planning/execution path as any other Task, and it is not a
dry-run or simulated preview. Exact idempotency receipts remain replayable after the
Automation is disabled or the Workspace is archived; a new test run still requires an
active Workspace and the exact current revision.

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

The local owner `POST /v1/automations/{id}/run` command requires the exact current
Automation revision (`If-Match` aggregate version), a stable `Idempotency-Key`, one
ManualTrigger, and an input object validated against that Automation's pinned
RoutineRevision. The authenticated daemon is the local TriggerHost; its exact current
Runtime incarnation and active Workspace binding version are checked atomically with
Task admission. The one-shot command is allowed while the Automation is PAUSED and does
not enable or recreate recurring trigger hosting. A disabled Automation cannot accept a
new run. The transaction persists PENDING/version 1, claims it as CLAIMED/version 2 with
claim_epoch 1, and links the ordinary READY Task while advancing the occurrence to
STARTED/version 3. Task, occurrence events/snapshots, and idempotency receipt commit
together; no Plan, Step, Attempt, AgentSession, lease, or Environment is created. A retry
with the same authenticated principal, RequestId, revision, inputs, and expected version
resolves the exact committed receipt before mutable host/Coworker/Resource checks; the
transaction repeats the receipt check to serialize concurrent requests. Changed command
content conflicts. The owner and selected Workspace must still authenticate, and the
immutable Automation revision must remain readable to identify the ManualTrigger. This
manual binding-version check is distinct from an AutomationCursor host epoch: a
never-enabled PAUSED Automation has no recurring cursor and is not implicitly activated.
`STARTED` means the occurrence and READY Task link were committed; it does not claim that
planning, agent execution, or uninterrupted Task work has started.

`POST /v1/automations/{id}/resume` changes one `PAUSED` Automation to `ENABLED` under the
same owner and expected-version rules. It requires an active local `TRIGGER_HOST` Workspace
binding, an active current pinned Routine (and Coworker, when pinned), and only locally
reconcilable Manual or Schedule triggers. The status change and exact trigger-host cursor
registration commit atomically, and an exact retry replays the receipt. `DISABLED` is
terminal. Enabling does not run existing definitions until their due or explicit triggers
fire.

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

## Event-trigger delivery, security, and local reachability — 2026-10-22

The trigger schema above already admits `WebhookTrigger`, `ConnectorEventTrigger`,
`ChannelEventTrigger` and the other event kinds, and [Connectors & Workflows](CONNECTORS-WORKFLOWS.md)
already owns the relay envelope and its hosting modes. This section specifies the delivery,
dedupe, security and failure contract that binds them, informed by how the field actually
ships event triggers. It changes no V1 scope line: **V1 ships schedule, one-shot, manual,
webhook and connector-event providers**; the rest stay schema-compatible extension points.

### Delivery semantics: at-least-once in, exactly-once occurrence

Every vendor in this space is at-least-once on the wire, and none of them promises
exactly-once externally. Inngest documents three separate dedupe layers; Trigger.dev states
plainly that its idempotency key dedupes *the trigger call only* and that the caller must
still pass provider-side keys for effects. LiteCowork keeps the same three layers:

1. **Delivery layer.** A delivery is identified by `(trigger_id, provider_delivery_id)`,
   falling back to a digest of the raw body when the provider sends no delivery id — GitHub's
   `X-GitHub-Delivery` is the model for the former. A replayed delivery inside the trigger's
   `replay_window_ms` resolves to the receipt already recorded and creates nothing new.
2. **Occurrence layer.** Occurrence identity stays exactly as contracted above — derived from
   `(trigger, resolved slot instant)` for schedules and from
   `(trigger, provider_event_id)` for events, with the same `UNIQUE (automation_id,
   trigger_id, occurrence_key)` fence. A duplicate delivery therefore never becomes a second
   occurrence, and a repeated settlement is a no-op.
3. **Effect layer.** Nothing here weakens the Core rule: an unknown external result is
   reconciled before any retry, so a re-delivered webhook can never duplicate a
   consequential external action.

A debounced or coalesced trigger (`debounce_ms` on resource/runtime events, and any future
coalescing policy) collapses N provider events into **one** occurrence for one logical
condition, and the occurrence records how many events it absorbed. Coalescing is a policy
on the Automation, never a silent drop.

### Inbound webhook security

The contract for a `WebhookTrigger` with `authentication_mode: SIGNATURE` is the
GitHub/Slack pattern, which is the industry floor:

- **Verify over the raw request bytes, before parsing.** The signature is an HMAC (SHA-256
  unless the provider specifies otherwise) keyed by the trigger's `auth_profile_ref` secret,
  compared in **constant time**. Parsing first and signing the parsed object is a known
  bypass and is forbidden.
- **Enforce a timestamp window.** Slack rejects when `|now - timestamp| > 5 minutes`; that
  replay window is the baseline, and a delivery older than the trigger's `replay_window_ms`
  is refused as a replay rather than executed.
- **Bound everything.** `max_payload_bytes` is enforced before the body is buffered; an
  over-capacity delivery is refused with a reason, never truncated and executed. Payloads
  remain untrusted input: a webhook body can never carry authority, and a provider's own
  "please skip approval" text inside a payload has no effect on Trust policy.
- **Secret custody.** The signing secret lives in the local SecretStore and is never written
  to events, aggregate-state blobs, logs, metrics or backups. A `BEARER_SECRET` or
  `MUTUAL_TLS` profile follows the same custody rule.
- **Endpoint ownership.** Where the provider supports a challenge handshake, the endpoint
  answers it at creation time; a handshake is not a security boundary.

`Authentication` is per-trigger and explicit. A webhook without a verifiable profile is
refused at enable time, not at first delivery.

### Local reachability: why polling is the default, not the fallback

GitHub and Slack require a **publicly reachable HTTPS endpoint with a verified certificate**;
a laptop behind NAT cannot be one. Slack's own documented answer for this situation is
Socket Mode (WebSocket outbound), and Activepieces marks app-side webhooks "Not Supported"
and prescribes polling. The design consequence for LiteCowork is that the hosting mode is
chosen at enable time and shown to the user:

```text
LOCAL_POLL             default for a local-only Workspace. The trigger host polls the
                       provider on `poll_interval_ms` with a persisted cursor and
                       LAST_ITEM or TIMEBASED dedupe, and records a bounded skipped
                       result when a poll returns nothing new.
LOCAL_PUSH_WHEN_REACHABLE  opt-in for a provider that can reach this machine (loopback,
                       LAN, or an owner-supplied tunnel with a real certificate). The
                       trigger host publishes its reachability; when unreachable the
                       trigger degrades to polling rather than failing silently.
LITECOWORK_RELAY       opt-in. The relay carries only the signed minimal
                       EventRelayEnvelope wake hint — never provider OAuth refresh tokens
                       and never provider content — and the local host fetches the real
                       object with its own credentials.
USER_SELF_HOSTED_RELAY for an owner-operated relay.
```

Three rules follow, and they are the honest difference from a cloud product:
1. **Never claim 24/7.** When the machine is asleep or the Runtime is down, occurrences are
   **recorded as missed** with a reason, exactly as the schedule misfire policy already
   requires. OpenWork's model is the reference: warn *before* an occurrence is due when no
   runner is present, record the miss when it happens, and never imply the run occurred.
2. **Runner presence and liveness.** A trigger host publishes a lease with a heartbeat. A
   due occurrence with no live lease is not executed; it is either recorded as missed or
   held according to the Automation's overlap policy, and a recovery pass re-evaluates the
   cursor after restart.
3. **Fast-ack, then queue.** Slack requires a 2xx within 3 seconds and GitHub within 30
   seconds. The intake path therefore verifies the signature, persists a bounded receipt
   and answers 2xx, then performs provider fetch and occurrence creation asynchronously —
   never inside the request handler.

### Failure, retry, and run history

- **Bounded retries with explicit classes.** Retries use exponential backoff with jitter and
   a bounded count; a failure is classified `retryable` or terminal before it is retried.
   A provider 5xx or timeout is retryable; an authorization failure, an invalid signature,
   an over-capacity payload and an unknown event type are terminal and are surfaced, never
   retried as if the provider were down.
- **Observable dead letter.** A delivery that exhausts retries is retained as a bounded,
   owner-visible failure record with its reason code and delivery id. Nothing is dropped
   silently, and the run history links occurrence → delivery → outcome.
- **Run lifecycle.** Every event-triggered run uses the same recorded lifecycle as scheduled
   runs — `queued → claimed → running → succeeded | failed | cancelled | skipped` — with
   `attempt_count`, a `needs_attention_reason`, and a `retryable` flag, so an event run and
   a scheduled run are indistinguishable in history.
- **No silent provider mutation.** As with scheduled runs, a test run of an event-triggered
   Automation performs real work against real systems; the UI must say so.

## Standing responsibilities and unattended trigger UX target

StandingResponsibility is a user-facing grouping and admission fence, not an alternative scheduler. One standing request may contain a daily watch and a separate weekly report using multiple existing Automation revisions. A confirmed enable action must atomically persist the reviewed configuration and durable trigger registration/outbox, or report blocked; never call a proposed definition enabled without host readiness. Source events require provider identity/replay verification. Deterministic filtering suppresses unchanged observations without model calls or noisy notifications. Offline reconciliation obeys the exact timezone, DST fold/gap, misfire and overlap policies; duplicate events/host epochs produce only one canonical occurrence. Run now creates a distinct manual occurrence under existing owner authority and never re-enables paused recurrence. Pausing new work fences new occurrences; Pause all additionally invokes safe Task pause. Future cloud TriggerHost placement is separate from the local Runtime that owns desktop resources. See [Coworker target](COWORKERS-TARGET.md).


## Unified Automation UX, connector events, workflow calls, and run continuation

The internal TriggerDefinition union remains explicit, but the normal UI presents one
plain Automation concept. The first choice is **When** (On a schedule, Something happens
in an app, A condition becomes true, etc.); the reusable Routine remains **Do**. Cron and
subscription storage vocabulary is advanced detail, not the default product language.

Connector event delivery follows
[Connectors & Workflows](CONNECTORS-WORKFLOWS.md). A hosted EventRelay may queue a minimal
signed event hint while a local Runtime is offline; it is not allowed to hold a general
Connection grant or mint a Task. TriggerCoordinator still authenticates/deduplicates the
source and creates the canonical AutomationOccurrence/Task under existing cursor and
idempotency rules.

A Routine may invoke an existing external deterministic workflow through a qualified
WorkflowBackendAdapter, but that invocation is an ordinary capability operation inside the
Task. The external engine does not become the Automation scheduler or Task state machine.

### Run history and Continue

Runs remains occurrence/Task history. When its Task is readable, the user may choose
**Continue**.

~~~text
AutomationOccurrence -> Task
                         |
                         +-- user clicks Continue
                              |
                              v
                    PRIMARY_FOLLOW_UP Conversation
~~~

Continue is on-demand; it does not create hidden Conversations for every unattended run.
The command atomically creates or returns the Task's one follow-up Conversation. The UI
renders a trusted run-context card rather than fake historical assistant messages.

Native-session continuity is opportunistic, not invented. If the Task's AgentModule proves
a durable resume point still matches binding/config/runtime policy, the first follow-up
turn may reuse it. Otherwise LiteCowork rebuilds bounded context from durable TaskSpec,
selected Artifacts, Evidence and unresolved blockers. Continue never reruns the Automation,
changes its cursor, or grants new authority.

## V1 detailed design integration — 2026-10-12

The cross-surface V1 implementation sequence and required functions for Routine->Automation->Trigger->Occurrence->Task->Continue are in V1-WORKFLOW-EVENT-LLD.md and V1-FUNCTION-FILE-CATALOG.md. Enablement requires source/grants/host readiness and consent. Missed local offline slots and DST folds/gaps must be recorded honestly; Run now never enables recurrence. Notification content reflects persisted status, not an agent sentence.
