# Delegation and Worker Selection

This document owns the LiteCowork-hosted delegation contract. Agent-native subagents
remain owned by their native harness; deterministic operations remain Capability
Invocations. LiteCowork creates a child Attempt only for host delegation.

## Goals and non-goals

Delegation lets a lead agent assign a bounded part of an accepted Task plan to an
eligible worker profile. Core owns admission, provenance, budgets, isolation, grants,
leases, reconciliation, and result integration. The lead and worker own their reasoning
and native harness behavior.

This contract does not define a universal model registry, copy native configuration,
make cross-provider features identical, promise hard real-time starts, or treat a worker
report as verification. See `AGENT-FABRIC.md`, `TASK-RUNTIME.md`, `TRUST.md`, and
`ENVIRONMENTS.md` for the underlying records and authorities.

## Delegation classes

```text
AgentDelegationMode = NATIVE_INTERNAL | HOST_DELEGATED | CAPABILITY_EXECUTOR
```

| Class | Owner | LiteCowork representation |
|---|---|---|
| `NATIVE_INTERNAL` | Lead's agent harness | Reported child activity only when the adapter exposes it; no child Attempt, Core lease, or Core grant is claimed. |
| `HOST_DELEGATED` | LiteCowork Task Runtime around an external harness | A preplanned Step, child Attempt, pinned DelegationProfile revision, AgentSession, Environment, lease, scoped grants, and ResultEnvelope. |
| `CAPABILITY_EXECUTOR` | Capability Broker/provider | A CapabilityInvocation and its Effects/Evidence; it is not shown as a reasoning agent. |

No agent brand is special-cased by Core. A feature can be used only when the selected
adapter reports and passes the required conformance contract.

## Native harness integrity

```text
NativeHarnessIntegrityMode = NATIVE_UNMODIFIED | NATIVE_PLUS_BRIDGE
```

`NATIVE_UNMODIFIED` starts the selected harness without a LiteCowork-injected tool or
configuration change. `NATIVE_PLUS_BRIDGE` adds only the explicitly negotiated LiteCowork
bridge and records its normalized non-secret configuration digest. Both modes preserve
user-owned native files and settings. A managed overwrite mode is outside this contract.

LiteCowork must not rewrite `CLAUDE.md`, `AGENTS.md`, `.codex/config.toml`, OpenCode
configuration, Cline rules, native hooks, skills, plugins, MCP settings, permissions,
subagents, or memory. Adapters may read only configuration they need and may compute a
digest only from a normalized, non-secret option projection. Secret bytes, credential
material, private native prompts, and native session handles are never copied into a
descriptor, event, TaskPacket, ordinary Artifact, log, or backup.

An `AgentHarnessDescriptor` is a time-bounded observation returned by an adapter:

```text
AgentHarnessDescriptor {
  agent_profile_id: AgentProfileId
  endpoint_id: AgentEndpointId
  protocol_version?: string
  harness_version?: string
  observed_at: Timestamp
  expires_at?: Timestamp
  effective_config_digest?: Sha256Digest # normalized non-secret options only
  features: AgentCapabilities
  session_option_schema?: JsonObject       # adapter-owned schema, closed and size-bounded
}
```

It is not a copy of native configuration. Model names, effort values, and other option
values remain agent-owned opaque values and are validated by the adapter. Unsupported
or stale options fail admission with `AGENT_SESSION_OVERRIDE_UNSUPPORTED` or
`AGENT_NATIVE_CONFIG_CHANGED`; LiteCowork never silently substitutes a different value.
Descriptor loss/expiry requires a fresh probe before new work.

## AgentBinding use eligibility

`AgentBinding.enabled` authorizes LiteCowork to interact with that binding. Add
`lead_eligible: bool` to indicate whether the owner may select it for lead work. Neither
field enables delegated work. Delegated work requires at least one enabled
`DelegationProfile` revision for that binding.

```text
enabled = false                         => no new session or Attempt admission
enabled = true, lead_eligible = false   => worker only
enabled = true, lead_eligible = true    => may be a lead
profile status = ENABLED                => this worker configuration may be selected
```

Disabling a binding prevents new planning, lead, and worker admission. Already admitted
sessions/Attempts remain pinned and follow ordinary safe-settlement rules. Disabling a
binding that is the Workspace default remains rejected until the owner changes that
default.

## DelegationProfile data contract

One AgentBinding may have zero or more worker profiles. A profile is durable Workspace
policy; its immutable revisions configure future child admission. An installed/discovered
AgentProfile is never automatically added to a lead's worker catalogue.

```text
DelegationProfile {
  delegation_profile_id: DelegationProfileId
  workspace_id: WorkspaceId
  agent_binding_id: AgentBindingId
  name: string                  # projection of the current revision's name
  current_revision: u64
  status: ENABLED | DISABLED | ARCHIVED
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

DelegationProfileRevision {
  delegation_profile_id: DelegationProfileId
  revision: u64
  name: string                  # immutable snapshot; current head names are unique per binding
  routing_description: string       # short; only enabled profiles enter lead context
  instructions?: string             # worker guidance, never an enforcement boundary
  session_options: JsonObject        # agent-owned values; adapter validates every key
  session_options_descriptor_digest?: Sha256Digest # required when options are nonempty
  required_features: AgentFeature[]
  preferred_features: AgentFeature[]
  enforced_policy: DelegatedWorkerPolicy
  optimization_preference: QUALITY_FIRST | BALANCED | COST_FIRST | LATENCY_FIRST
  quality_floor?: AcceptanceCriterion[]
  max_concurrency: u32
  max_host_delegation_depth: u32 # descendant host-delegation levels this worker may create; 0 disables further delegation
  budget_ceiling?: BudgetSpec
  latency_class: STANDARD | INTERACTIVE | DEADLINE_SENSITIVE
  environment_policy: DelegatedEnvironmentPolicy
  native_delegation_policy: INHERIT | ALLOW | DENY_IF_SUPPORTED
  warm_policy: WarmPolicy
  authored_by: PrincipalRef
  created_at: Timestamp
}

DelegatedWorkerPolicy {
  capability_allowlist: CapabilityRef[] # upper bound; actual use still needs a Grant
  maximum_effect_risk: SAFE | SENSITIVE | HIGH_IMPACT
  filesystem_write_scope: WORKTREE_ONLY | ATTEMPT_PRIVATE | EXPLICIT_SHARED
  external_effects: DENY | REQUIRE_EXISTING_POLICY
  secret_access: NONE | TASK_SCOPED_GRANTS_ONLY
}

DelegatedEnvironmentPolicy {
  placement_preference: PlacementPreference
  isolation: REQUIRED | PREFERRED
  sharing_scope: ATTEMPT_PRIVATE | TASK_SHARED | COWORKER_PRIVATE | WORKSPACE_SHARED
}
```

Names are trimmed using Unicode whitespace rules, normalized to Unicode NFC, and
compared using Unicode Default Case Folding within one AgentBinding. The v1 name-key
algorithm is `NFC(NFC(trim(name)).casefold())`; its result is persisted so replicas do not
recompute an existing key. Only current head names are unique among non-archived
profiles; old revision snapshots do not reserve a name. Archiving frees the current name.
Renaming creates a new revision; it never changes the name recorded for an Attempt's
pinned revision. A profile may be duplicated only into a new,
disabled profile on the same AgentBinding. Duplication copies the source profile's
current non-secret revision values, assigns a new name and revision 1, and copies no
Attempt, session, grant, lease, Environment, or runtime state. If the source options
descriptor is stale, the duplicate remains disabled until an owner revises and validates
it. Portable profile export/import is deferred: option compatibility is adapter-versioned,
and Workspace policy must be reviewed at the destination. A future LitePSM template may
provide an explicitly reviewed, non-secret portability contract.

`GET /v1/delegation-profiles/{id}/revisions` returns immutable revision metadata and
configuration in descending order. Revision rows carry profile ID, revision number,
Workspace, name, author, and creation time. `POST .../{id}/duplicate` is idempotent and
requires the source profile's current version in `If-Match`; the source read and duplicate
commit are serialized so the new profile exactly matches the version the owner reviewed.

V1 defines root Attempt depth as zero and increments depth by one for every
LiteCowork-hosted child. The platform hard limit is depth 2 and at most 8 concurrently
active child Attempts per Task; these are admission safety ceilings, not throughput
targets. `DelegationProfile.max_host_delegation_depth` limits additional descendant
levels from an Attempt using that profile, and `max_concurrency` limits its nonterminal
Attempts. Task/Coworker `DelegationBudgetPolicy` may only lower the platform ceilings.
These caps apply across Runtimes and profiles, preventing a lead from bypassing limits by
splitting work among many profiles. The effective admission-policy version is recorded
with each child admission. Limits may be raised only in a versioned Core policy update
after the bounded-fanout benchmark passes.

`INHERIT` preserves current native configuration. `ALLOW` explicitly permits native
delegation where the harness supports it; it does not grant LiteCowork worker eligibility.
`DENY_IF_SUPPORTED` is valid only when the adapter reports an enforceable per-session
control. Otherwise profile validation rejects the option; native files are never rewritten.

`ExistingPolicyClass` is a reference to the current Trust policy vocabulary, not a new
grant or a new authority source. The profile is a restrictive ceiling: it can remove
options, capabilities, write access, or effects, but can never grant them. In v1,
`capability_allowlist` must be a subset of capabilities independently granted to the
child Attempt. `EXPLICIT_SHARED` requires an Environment contract that serializes writes
or provides isolated overlays; otherwise admission rejects it.

For a Task pinned to a Coworker revision, candidate profiles must also appear in that
revision's `enabled_delegation_profile_ids`. A Task without Coworker origin may use any
eligible Workspace profile. V1 profile and Environment admission does not accept
`USER_SHARED`; it is reserved until the user-level Environment ownership and attachment
contract exists.

The profile's `max_concurrency` is 1..8 and `max_host_delegation_depth` is 0..2. Task and
Coworker budget-policy overrides use the same maximum ranges; the service rejects values
outside them instead of silently clamping. `session_options` may contain at most 64
adapter-negotiated non-secret options and 64 KiB of canonical JSON; unknown keys, secret
fields, stale descriptor digests, and values outside the adapter schema fail validation.
The digest pins the adapter's negotiated option schema for this immutable revision. A
changed digest requires an owner-reviewed profile revision before future admissions;
active Attempts retain their descriptor and profile provenance.

Malformed options objects, more than 64 options, or canonical JSON over 64 KiB fail with
`DELEGATION_PROFILE_OPTIONS_INVALID`. A well-formed option the adapter does not support
fails with `AGENT_SESSION_OVERRIDE_UNSUPPORTED`; a changed effective native configuration
fails closed with `AGENT_NATIVE_CONFIG_CHANGED` until re-probed/reviewed.

Routing descriptions are short summaries such as “Quick repository scans and mechanical
edits.” Detailed guidance is attached only to the worker session. Only enabled profiles
are presented in the lead's discovery catalogue. Session options are not copied into the
lead's context unless needed to explain a selected worker.

Each Attempt stores `delegation_profile_id` and
`delegation_profile_revision` together or neither. The pair is present exactly for
host-delegated child Attempts. Revision, binding, and selected endpoint are immutable
Attempt provenance. Profile edits affect future admission only. Disabling or archiving a
profile stops new admission but does not rewrite or silently cancel already admitted
Attempts.

## DelegationStrategy

```text
DelegationStrategy = NATIVE_DEFAULT | BALANCED | COST_SAVER | HOST_DELEGATION_ONLY
OptimizationPreference = QUALITY_FIRST | BALANCED | COST_FIRST | LATENCY_FIRST
```

`NATIVE_DEFAULT` leaves native delegation unchanged and does not add a host preference.
`BALANCED` exposes enabled profiles to the lead. `COST_SAVER` guides the lead toward lower
known-cost eligible profiles only when verification supports the required quality floor.
`HOST_DELEGATION_ONLY` is accepted only when the adapter can safely prevent native
delegation for that session; otherwise the setting is unavailable. None silently changes
the selected lead AgentBinding or model.

## DelegateRequest and accepted-plan rule

```text
DelegateRequest {
  parent_attempt_id: AttemptId
  step_id: StepId                       # required, already in current accepted PlanRevision
  objective: string                     # must match or narrow the Step objective
  input_refs: PinnedResourceRef[]
  artifact_refs: ArtifactVersionRef[]
  required_capabilities: CapabilityRequirement[]
  acceptance_criteria: AcceptanceCriterion[]
  profile_selection: AUTOMATIC | PREFER | REQUIRE
  preferred_delegation_profile_id?: DelegationProfileId
  optimization_preference: OptimizationPreference
  quality_floor?: AcceptanceCriterion[]
  max_cost?: BudgetSpec
  latency_class: STANDARD | INTERACTIVE | DEADLINE_SENSITIVE
  placement_preference?: PlacementPreference
  isolation: REQUIRED | PREFERRED
  budget?: BudgetSpec                   # capped by Task and profile ceilings
  deadline?: Timestamp                  # cannot exceed the Task deadline
  escalation_policy: EscalationPolicy
  return_schema: JsonObject
  request_id: RequestId
}
```

`AUTOMATIC` omits a profile ID and lets Core rank all eligible profiles. `PREFER` requires
an ID and ranks that profile first, then may use another eligible profile if policy allows
and it is unavailable. `REQUIRE` requires an ID and fails admission if it cannot be used;
it never falls back. A user action explicitly selecting a profile maps to `REQUIRE`.

`request_id` is idempotent within the authenticated caller scope. Replaying the same
canonical request digest returns the original `ChildAttemptRef`; reusing its ID with a
different digest returns `CONFLICT`. Concurrent requests for one Step serialize at
TaskService: only one can bind the current Attempt/lease, and the other receives the
current result or `STEP_NOT_READY`. The digest covers Task/Step/profile preferences and
bounded references, never secret or private prompt contents.

Delegation does not append arbitrary Steps. The target `Step` must belong to the Task's
current accepted PlanRevision, be `READY`, have satisfied dependencies, and have no
active Attempt. The lead Attempt must be current, RUNNING, and hold the valid Execution
Lease for the parent Step. The calling AgentSession must be the same active Attempt
session. If the accepted plan lacks a suitable Step, the lead proposes a new
PlanRevision; TaskService validates and accepts it before child admission. This preserves
PlanRevision immutability, review, and Step identity.

Admission uses one transaction for the child Attempt, Step.current_attempt_id, lease,
and events, following `TASK-RUNTIME.md`. The child Attempt's `parent_attempt_id` records
the requesting Attempt. Its `task_spec_revision` is the accepted PlanRevision's pinned
revision. A parent may settle while an admitted child continues under its own lease;
the child result remains attached to the Task and a future lead must validate it before
integration. Attempts are never reparented.

## Candidate eligibility and ranking

WorkerSelection first filters, then ranks. A profile is rejected if any hard predicate
fails:

1. Profile and AgentBinding are enabled and not archived; binding is authorized in the
   Task Workspace.
2. Target Step is ready in the current accepted plan; parent Attempt/session/lease are
   current; platform, Task, Coworker, profile concurrency/depth, and recovery ceilings
   permit admission.
3. Required features and session options are supported by a fresh adapter descriptor.
4. A compatible AgentEndpoint and Runtime incarnation are available or safely startable.
5. Required Environment class, sharing scope, isolation, resource freshness, and
   write-control lease are available.
6. Authentication is healthy; the profile's capability ceiling is met by new child-scoped
   grants; Trust policy permits all requested operations.
7. Budget can be reserved or provider-enforced to the requested ceiling. An unknown cost
   cannot satisfy a hard monetary ceiling without provider enforcement.
8. Deadline preconditions can be met at the requested latency class. Deadlines are
   best-effort scheduling targets, not real-time guarantees.

Eligible candidates are ranked by explicit request preference, the Task's pinned
Coworker strategy when present, profile optimization preference, observed verifier pass
rate for the task category, observed latency, and observed cost. The ranking policy/version and eligible candidate
IDs are retained in admission diagnostics. Metrics are projections, never truth. An
unknown cost is not zero; for `COST_FIRST`, known-cost candidates rank ahead of unknown
ones when all else is equal. The owner may pin a profile; if it is unavailable or
ineligible, return a typed failure rather than substituting another profile.

Provider quota is recorded only from a named observation source:

```text
QuotaObservation {
  agent_binding_id: AgentBindingId
  source: string
  observed_at: Timestamp
  expires_at?: Timestamp
  state: NORMAL | LOW | EXHAUSTED | UNKNOWN
  remaining_hint?: UsageQuantity
  reset_at?: Timestamp
}
```

An expired observation projects as `UNKNOWN`; provider units are retained in
`remaining_hint` and never normalized into a percentage unless the source explicitly
reports percent. `UNKNOWN` is valid and never converted into an estimated percentage. A quota observation
may trigger a prewarm or user-visible fallback offer; it does not authorize automatic
lead switching. Return `AGENT_QUOTA_EXHAUSTED` only when a provider/harness explicitly
confirms exhaustion; a generic rate limit or unavailable quota remains
`AGENT_UNAVAILABLE` with typed details.

`GET /v1/agent-bindings/{id}/quota-observation` returns this observation or JSON `null`
when the adapter has made none. Expired observations are returned as `UNKNOWN` with their
original timestamps retained. The UI shows source and age and never draws a remaining
quota meter unless the source supplies a quantified unit.

## LeadFailoverPolicy

`LeadFailoverPolicy` is Task-scoped continuation policy, not authority. `CoworkerRevision`
may provide a default; Task creation resolves and pins the effective policy into
`TaskSpecRevision`. Without a Coworker default the policy is explicitly `DISABLED`. Later
Coworker edits do not alter existing Tasks. A TaskSpec revision inherits the previous
policy unless the owner changes it.

```text
LeadFailoverPolicy {
  mode: DISABLED | ASK | ALLOW_LISTED
  triggers: (AGENT_UNAVAILABLE | QUOTA_EXHAUSTED | RUNTIME_UNAVAILABLE)[]
  fallback_agent_binding_ids: AgentBindingId[] # ordered and unique; maximum 3
  max_lead_changes: u32 # 0..3
}
```

`DISABLED` requires empty triggers/fallbacks and zero changes. `ASK` requires at least one
trigger and may list up to three suggested bindings, but automatic changes remain zero.
`ALLOW_LISTED` requires at least one trigger and fallback and a positive change limit no
greater than the fallback count. Only a provider-confirmed trigger can activate this
policy. Each candidate is rechecked for Workspace scope, enabled/lead-eligible binding,
endpoint, Runtime/incarnation, auth, policy, resources, Trust, budget, and deadline. The
switch creates a fresh lead AgentSession and bounded handoff from durable Task state;
grants, Approvals, SecretLeases, provider handles, and native transcripts never transfer.
Existing Attempts retain their original binding and lease. User-requested lead changes
remain available independently of automatic failover policy.

`WorkerPerformanceProjection` is exposed by
`GET /v1/delegation-profiles/{id}/performance?task_category=`. It is derived from
terminal child Attempts and their VerificationRuns/UsageObservations in a rolling 90-day
window. `sample_count` counts terminal child Attempts except cancelled/abandoned ones;
`verifier_pass_rate` uses only attempts with terminal verifier results. Median latency uses
admission-to-terminal duration. Retry rate counts a profile's Attempts replaced by a later
Attempt for the same Step; human-intervention rate counts its Attempts blocked by an owner
UserRequest. Cost medians include only compatible known units/currencies; mixed or missing
units yield null, never zero. Confidence is a sample-quality label: LOW below three
terminal verifier results or when source observations are incomplete, MEDIUM at three or
more complete verifier results, and HIGH at ten or more with at least 80% of eligible
Attempts carrying terminal verification and required observations. Only MEDIUM/HIGH
projections may influence ranking; these thresholds are heuristics, not a quality
guarantee. Explicit profile selection always outranks the projection. The endpoint never
includes prompts, generated output, or native private transcript data.

## TaskPacket and ResultEnvelope

`docs/schemas/delegation.schema.json` is the canonical machine-readable contract for
DelegateRequest, TaskPacket, and ResultEnvelope. Canonical JSON size for each envelope is
capped at 1 MiB and may be lower under Task/profile context budgets; referenced Resource
and Artifact content is transferred through separately authorized reads and is not
inlined. Strings and arrays also have schema-level per-field bounds.
`BoundedDecision.source_event_id` must resolve to a committed user message or Task
decision in this Task's Conversation, and the summary is an untrusted convenience
projection, never authority. `ArtifactVersionRef` pins an immutable Artifact version;
output and changed-resource refs pin exact Resource revisions.

```text
TaskPacket {
  task_id: TaskId
  step_id: StepId
  parent_attempt_id: AttemptId
  task_spec_revision: u64
  plan_revision: u64
  workspace_instruction_revision?: u64
  delegation_profile_id: DelegationProfileId
  delegation_profile_revision: u64
  objective: string
  constraints: string[]
  input_refs: PinnedResourceRef[]
  artifact_refs: ArtifactVersionRef[]
  relevant_decisions: BoundedDecision[]
  required_output: OutputRequirement[]
  acceptance_criteria: AcceptanceCriterion[]
  capability_grant_refs: CapabilityGrantId[] # child scope only
  deadline?: Timestamp
  budget?: BudgetSpec
}

ResultEnvelope {
  child_attempt_id: AttemptId
  status: COMPLETED | FAILED | BLOCKED | CANCELLED
  summary: string
  output_refs: PinnedResourceRef[]
  artifact_refs: ArtifactVersionRef[]
  evidence_refs: EvidenceId[]
  changed_resource_refs: PinnedResourceRef[]
  unresolved: string[]
  blockers: Blocker[]
  usage?: UsageObservationId[]
  confidence?: number
  verification_hints: string[]
  revision_at_start: u64
}
```

## DelegationChannel

```text
interface DelegationChannel {
  send_ephemeral(child_attempt_id, bounded_message) -> Ack
  steer(child_attempt_id, SteerCommand) -> DurableSteerReceipt
  status(child_attempt_id) -> ChildAttemptStatus
  stream_events(child_attempt_id, cursor?) -> Stream<NormalizedWorkerEvent>
  result(child_attempt_id) -> ResultEnvelope?
  cancel(child_attempt_id, reason) -> CancelReceipt
}
```

`send_ephemeral` is bounded, authenticated, rate-limited, Runtime-incarnation/Attempt
scoped, and expires with the session. It is not replayed after restart and cannot mutate
Task state, grant authority, or settle an Effect. `steer` is a TaskService command with
RequestId/version and is durable. `stream_events` contains normalized progress/status and
referenced outputs, not raw token deltas by default. `result()` reads the durable child
settlement. Cancellation requests normal Attempt/AgentSession stop and does not assert
provider quiescence until observed. Retention of ephemeral content is zero/minimal by
default; diagnostics store bounded digests/outcome classes only.

The packet is derived from durable Task state and is bounded by the profile and Task
context limits. It excludes full parent transcripts, hidden reasoning, native session
IDs, private system prompts, credentials, ungranted resources, and unrelated repository
content. A worker output is untrusted input. Parent integration rechecks provenance,
resource freshness, current TaskSpec applicability, output schema, Effects, and verifier
requirements. `confidence` is a report, not Evidence.

## Verification and escalation

Each worker execution and retry is a separate Attempt. `EscalationPolicy` defines an
ordered, bounded list of fallback profile IDs and a maximum number of worker Attempts;
the initial Attempt counts toward the limit, the value is 1..8, and it cannot exceed the
Task/policy ceiling. The fallback list contains no more than `max_worker_attempts - 1`
unique profile IDs.
A verifier runs against the Step's pinned acceptance criteria. A pass returns
the result to the lead. A recoverable failure may admit the next eligible profile for a
new Attempt after Effects are reconciled and the Step is recoverable. A permanent,
ambiguous, or budget-exhausted result blocks automatic escalation and returns a typed
blocker to the lead or Needs You. The scheduler never mutates an Attempt's binding or
model in place, loops indefinitely, or escalates around Trust policy.

Only the lead or TaskService may accept the worker output into Task completion. A passing
child verifier is evidence for its criteria; it does not itself complete the Task.

## Cost, budgets, and performance projections

Use existing `BudgetSpec`, `BudgetReservation`, `UsageObservation`, and `BudgetService`.
TaskSpec may pin `DelegationBudgetPolicy`; CoworkerRevision may supply a default policy;
profile revision adds a worker ceiling. Effective numeric ceilings are the strictest
compatible values across Task, Coworker, and profile. Incompatible units/currencies fail
admission. Provider-reported, agent-reported, and host-measured
usage retain source and confidence. Unknown native usage stays unknown. A hard monetary
ceiling is enforceable only when the provider enforces it. A host-monitored threshold can
stop new work after a usage observation arrives, but it may overshoot because usage can
arrive late; the UI labels it as monitored and never as a hard cap.

```text
DelegationBudgetPolicy {
  max_concurrent_children?: u32
  max_host_delegation_depth?: u32
  max_per_attempt?: BudgetSpec
  max_per_task?: BudgetSpec
  max_per_profile?: BudgetSpec
  on_threshold: WARN | REDUCE_CONCURRENCY | PREFER_CHEAPER |
                REQUIRE_APPROVAL | STOP_NEW_DELEGATION
}
```

Task/Coworker concurrency and depth values may only lower the v1 platform ceilings. An
unset value inherits the tighter enclosing/default limit; it never means unlimited.
Effective concurrency is the minimum of platform, Task, pinned Coworker, and per-profile
limits. The depth counter is global to the Task branch across Runtimes and profiles.

`on_threshold` is an admission response; it never kills an active Attempt mid-Effect.
If policies at multiple scopes disagree, apply the most restrictive action in this order:
`STOP_NEW_DELEGATION`, `REQUIRE_APPROVAL`, `REDUCE_CONCURRENCY`, `PREFER_CHEAPER`,
`WARN`. A TaskSpec can tighten a Coworker default, but cannot widen its cost ceiling or
Trust policy. A profile cannot widen either. `REQUIRE_APPROVAL` blocks new child admission
and creates a Needs You request to review a TaskSpec/policy revision; it never expands the
active Attempt's budget or approves a consequential Effect. If no cheaper eligible worker
exists, `PREFER_CHEAPER` falls back to the next policy action and never admits an unknown-
cost candidate against an unenforced hard ceiling.

```text
WorkerPerformanceProjection {
  delegation_profile_id: DelegationProfileId
  task_category: string
  sample_count: u64
  verifier_pass_rate?: number
  median_latency_ms?: u64
  median_observed_cost?: UsageQuantity
  retry_rate?: number
  human_intervention_rate?: number
  failure_categories: string[]
  confidence: LOW | MEDIUM | HIGH
}
```

`task_category` comes from the pinned TaskSpecRevision's bounded owner/lead-selected
category; absent value maps to `OTHER`, with no hidden text classifier. The projection
excludes prompt/output content and is recomputable from eligible Task,
Attempt, Usage, and Verification records under retention policy. Small samples are shown
as insufficient evidence and do not outrank a user's explicit profile choice.

## Warm execution

Warmth is an optimization over separately owned lifecycles. AgentHostSupervisor owns
agent hosts/sessions; CapabilityHostSupervisor owns capability hosts; EnvironmentManager
owns Environments and browsers; a local model backend owns model memory/cache. No shared
`WarmthManager` may bypass those authorities.

```text
WarmPolicy {
  host: COLD | TTL | PIN_WHILE_ACTIVE
  native_session: CLOSE_ON_SETTLE | REUSE_IF_SAFE
  capability_hosts: COLD | TTL
  browser_environment: COLD | TASK | WORKSPACE
  local_model: PROVIDER_DEFAULT | KEEP_RECENT_HINT
  ttl_ms?: u64
  max_memory_bytes?: u64
  max_idle_cost?: CostLimit
  triggers: (ACTIVE_TASK | RECENT_USE | USER_SELECTED | QUOTA_LOW |
             PREDICTED_FAILOVER | DEADLINE_APPROACHING)[]
}
```

Default is cold/demand-start. TTL and pins are bounded by host policy, resource limits,
owner consent, and idle-cost policy. Prewarm may start/attach a process, validate auth,
probe config, prepare an Environment, or prepare a bounded TaskPacket. It must not call a
model solely to warm it. A warm host never implies a warm native session, valid grant,
lease, authenticated browser, or preserved provider cache. Eviction order is: speculative
prewarm; expired native sessions; idle capability hosts; idle secondary agent hosts;
optional persistent Environments under their own retention policy. Active Attempts,
open Effect reconciliation, user takeover, and required in-flight dependencies cannot be
evicted. Runtime restart invalidates process/session observations and triggers re-probe.

## Environment sharing and write coordination

`EnvironmentLifetime` answers how long an Environment may exist. `EnvironmentSharingScope`
answers who may reuse it:

```text
EnvironmentSharingScope = ATTEMPT_PRIVATE | TASK_SHARED | COWORKER_PRIVATE |
                          WORKSPACE_SHARED | USER_SHARED
```

Parallel code workers default to separate worktrees or private overlays. A shared writable
tree requires an explicit provider-backed write lock/lease or a deterministic merge
workflow. Browser profile persistence never shares input control: one current
`EnvironmentControlLease` owner acts at a time. Reuse never carries Trust grants,
Approvals, SecretLeases, Attempt leases, or control leases across Task/Attempt boundaries.

## Lead changes and prewarming

Keep four operations distinct: Conversation agent change, Task lead change, delegated
work, and worker replacement. Task lead change follows `TASK-RUNTIME.md` and creates a
bounded handoff projection from TaskSpec, Plan, Steps, decisions, Artifacts, Evidence,
open Effects, blockers, remaining budget, and reason. It contains no hidden reasoning,
full native transcript, or provider handle. It does not reparent active child Attempts.

A low-quota or predicted-failure observation may prepare an eligible fallback host without
invoking it. A Task may automatically change lead only under its pinned `ALLOW_LISTED`
LeadFailoverPolicy; `ASK` presents choices in Needs You and `DISABLED` stops for the owner.
Every new lead gets a fresh AgentSession and bounded projection. A live process is never
portrayed as having migrated.

## Deadline-sensitive execution

`ExecutionLatencyClass = STANDARD | INTERACTIVE | DEADLINE_SENSITIVE`. The last value
requests preflight and priority placement only. It does not promise hard real time.
Preflight verifies the selected agent/capability, Runtime incarnation, auth, resources,
freshness, budget reservation, Environment, Trust policy, approvals, and fallback before
the stated deadline window. A failed required check produces
`DEADLINE_EXECUTION_PRECONDITION_FAILED` before side effects.

Select an execution method by semantic suitability and authorization: structured provider
API, structured browser, accessibility/browser interface, then screen-based computer
use. A lower layer is selected only if it can faithfully perform the requested operation
and satisfy the same policy/evidence contract. There is no generic fallback that changes
meaning. A provider may execute a bounded `ActionBatch` only when each consequential
suboperation remains individually authorized, idempotent/reconcilable, and represented by
its own CapabilityInvocation, optional Effect, and Evidence. Each batch member has its own
request digest, grant/policy check, idempotency key, and settlement. Members persist the
same `action_batch_id` and digest with a zero-based ordinal and total operation count; the
ActionBatch is a grouping value, not a transaction or aggregate. A provider may transport
multiple operations in one request only when it returns per-member outcomes and preserves
per-member idempotency and reconciliation. Final consequential actions may require explicit
human takeover. An ActionBatch is not an atomic transaction: if a suboperation fails or an abort condition
becomes true, the provider stops later operations and LiteCowork reconciles every
dispatched Effect. Continue only from a fresh observation after reconciliation; changing
execution method must not restart a possibly completed operation. Batch timeout, user
takeover, and provider loss use the same Effect and EnvironmentControlLease recovery rules
as individual actions.

## Services and sequence

`DelegationProfileService` owns profile CRUD/revisions/enablement. `DelegationCoordinator`
is an internal Task Runtime module: it validates requests, asks WorkerSelection to filter
and rank, obtains child-scoped grants from TrustService, reserves budget, and asks
TaskService/AttemptRunner to atomically admit the Attempt and lease. It does not own
Agents, Environments, grants, Effects, or provider handles. `WorkerSelectionService` is a
pure candidate evaluation port over current adapter, Runtime, Environment, Trust, budget,
and performance observations.

```text
active lead Attempt
  -> validate current session + parent lease + accepted READY Step
  -> filter enabled profiles and negotiated harness options
  -> check Environment, grants, budget, depth, concurrency, deadline
  -> rank eligible candidates under explicit preference
  -> reserve budget and child-scoped grants
  -> atomically create child Attempt + lease + Step binding + events
  -> start a fresh delegated AgentSession
  -> stream ephemeral progress; persist only meaningful normalized events
  -> reconcile Effects, verify pinned criteria, publish ResultEnvelope
  -> parent validates applicability and integrates or requests bounded recovery
```

If any pre-commit check fails, no active child Attempt/lease exists. If startup fails after
commit, the Attempt follows ordinary failure/recovery rules and all reservations/grants
are released or settled through their owning services.

## Adapter conformance

An adapter used as a delegated worker must pass these checks before its profile can be
enabled: reports protocol/version and supported features; validates every session option;
rejects unsupported options without silent fallback; starts a fresh session scoped to one
Attempt; supports interrupt/cancel or explicitly reports that it does not; reports
session loss and usage confidence; preserves its native config; does not expose secrets in
events or output; and returns bounded outputs with resource provenance. The host must
verify every claimed result and retain a clear limitation when the adapter cannot expose
native child activity or exact usage.
