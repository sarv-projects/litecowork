# Coworkers, Goals, Suggestions, and Personal Context

This contract defines the persistent identity and responsibility layer above Tasks.
Coworkers and Goals organize work; they do not become an alternate execution authority.
Task, Effect, Artifact, Evidence, and Verification contracts remain authoritative.

## Coworker

A Coworker is a user-facing identity and preference bundle within one Workspace. It may
have a name, optional pinned avatar Resource revision, role description, default lead
binding, delegation strategy, enabled worker-profile allowlist, lead-failover defaults,
interaction defaults, context preferences, and notification preferences. It does not own
an AgentSession, Runtime, Environment, Task state, grant, Effect, or native memory.

```text
Coworker {
  coworker_id: CoworkerId
  workspace_id: WorkspaceId
  current_revision: u64
  status: ACTIVE | PAUSED | ARCHIVED
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

CoworkerRevision {
  coworker_id: CoworkerId
  revision: u64
  name: string
  avatar_ref?: PinnedResourceRef
  role_description: string
  default_lead_agent_binding_id?: AgentBindingId
  delegation_strategy: DelegationStrategy
  enabled_delegation_profile_ids: DelegationProfileId[]
  delegation_budget_policy?: DelegationBudgetPolicy
  lead_failover_policy?: LeadFailoverPolicy
  interaction_policy: CoworkerInteractionPolicy
  context_policy: CoworkerContextPolicy
  notification_policy: NotificationPolicy
  authored_by: PrincipalRef
  created_at: Timestamp
}

CoworkerContextPolicy {
  allowed_context_kinds: ContextDocumentKind[]
  max_retrieved_items: u32
  retain_task_summaries: bool
  require_user_confirmation_for_memory: true
}

CoworkerInteractionPolicy {
  read_only_work: InteractionDefault
  draft_creation: InteractionDefault
  external_mutation: InteractionDefault
  destructive_action: InteractionDefault
  financial_commitment: InteractionDefault
}
```

Each edit appends an immutable revision and advances the head with expected-version
checking. The default lead must be an enabled, same-Workspace, lead-eligible AgentBinding.
Every listed DelegationProfile must be enabled and belong to the same Workspace. Profile
and binding state is rechecked at admission; a Coworker revision does not pin a running
session's provider options. An avatar reference pins an exact same-Workspace Resource
revision, so later edits to that Resource cannot change a historical Coworker revision.

`ACTIVE -> PAUSED` stops proactive Coworker-originated work and prevents new
Coworker-scheduled occurrences from being admitted. Existing Tasks and in-flight external
Effects continue under their own policies. An explicit owner-submitted Task may still name
a PAUSED Coworker; it follows ordinary Task admission and authority checks. An ARCHIVED
Coworker cannot be selected as a new Task origin. `PAUSED -> ACTIVE` resumes future
scheduling only after trigger and dependency reconciliation. Archive is allowed only when no
Coworker-owned Automation is active and all Tasks are terminal; it is a soft archive and
does not delete Tasks, Artifacts, ContextDocuments, or external data. The initial
single-user product uses one `primary_coworker_id` Workspace setting; it may be cleared
explicitly and does not rewrite the origin of historical Tasks. The Operator uses this
selection to prefill new work and sends the chosen Coworker ID with Task creation. An
archived Coworker
cannot remain primary; clear/change the Workspace primary setting before archiving. The
Workspace, Coworker, and Task-origin changes are separate versioned commands.

For new Task creation, the lead binding is resolved from the explicit Task request first,
then the selected Coworker revision's default lead binding, then the Workspace default.
The first configured binding is authoritative for that request: if it is disabled,
worker-only, outside the Workspace, or has no eligible endpoint, Task creation fails with
`AGENT_UNAVAILABLE` instead of silently choosing a lower-precedence binding. A missing
Coworker default allows Workspace fallback; an explicit invalid Coworker default does not.

When a Task is created for a Coworker, Task stores `origin_coworker_id` and
`origin_coworker_revision` together as immutable provenance. The Task separately pins its
TaskSpec, accepted plan, lead AgentBinding, and grants. Changing Coworker settings affects
future admission and never mutates an existing Task.

Coworker presence is a read-only projection with independent axes, so one busy Task cannot
hide that the Coworker is paused or its Runtime is offline:

```text
CoworkerPresenceProjection {
  coworker_id: CoworkerId
  computed_at: Timestamp
  proactive_status: ACTIVE | PAUSED | ARCHIVED
  activity_status: AVAILABLE | PLANNING | WORKING | WAITING | NEEDS_YOU
  runtime_status: AVAILABLE | DEGRADED | OFFLINE | UNKNOWN
  active_task_count: u64
  waiting_task_count: u64
  needs_you_count: u64
  last_task_activity_at?: Timestamp
}
```

`runtime_status` describes only the Coworker's configured default lead path, evaluated
across eligible Runtime/endpoint offers; it does not summarize every Runtime, delegated
worker, or active Task. `AVAILABLE` means at least one eligible lead endpoint is currently
available or startable with valid auth; `DEGRADED` means the path exists but needs a
recoverable setup/health action; `OFFLINE` means no eligible path is currently reachable;
`UNKNOWN` means the supporting observations are absent or stale. Task rows expose their own
Runtime/location projection.

The activity axis is derived from linked nonterminal Tasks, UserRequests, blockers, and
active Attempts; `NEEDS_YOU` wins only for the activity label while counts preserve
simultaneous work. Runtime availability is derived from fresh Runtime/endpoint offers;
stale offers become `UNKNOWN`. This projection is served by
`GET /v1/coworkers/{id}/presence` and is not stored on Coworker or used for admission.
The UI presents proactive status, activity status, and Runtime status as separate labels;
it does not collapse them into one precedence-based state.

`CoworkerInteractionPolicy` uses one default per narrow action class:

```text
CoworkerInteractionPolicy {
  read_only_work: STANDARD_TRUST_POLICY | REQUIRE_OWNER_APPROVAL | HANDOFF_TO_OWNER
  draft_creation: STANDARD_TRUST_POLICY | REQUIRE_OWNER_APPROVAL | HANDOFF_TO_OWNER
  external_mutation: STANDARD_TRUST_POLICY | REQUIRE_OWNER_APPROVAL | HANDOFF_TO_OWNER
  destructive_action: STANDARD_TRUST_POLICY | REQUIRE_OWNER_APPROVAL | HANDOFF_TO_OWNER
  financial_commitment: STANDARD_TRUST_POLICY | REQUIRE_OWNER_APPROVAL | HANDOFF_TO_OWNER
}
```

This is a user-facing interaction preference, not an authority grant.
`STANDARD_TRUST_POLICY` leaves the existing Trust decision unchanged;
`REQUIRE_OWNER_APPROVAL` forces a fresh exact-action Approval; `HANDOFF_TO_OWNER` stops
before the operation and requires user takeover. No setting can reduce a Workspace or
provider requirement, create a Grant, or authorize an Effect. Recommended defaults are
ordinary Trust checks for read-only work and drafts, explicit Approval for external
mutation, and handoff for destructive/financial actions.

## Goals

A Goal is a user-authored, passive statement of desired outcome. It has an immutable
revision containing objective, success criteria, constraints, optional horizon, and
related Task/Routine/Artifact references. Links are provenance only. A Goal does not
schedule work, select an agent, issue authority, or create Tasks by itself.

```text
Goal {
  goal_id: GoalId
  workspace_id: WorkspaceId
  coworker_id?: CoworkerId
  current_revision: u64
  status: ACTIVE | PAUSED | COMPLETED | ARCHIVED
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}

GoalRevision {
  goal_id: GoalId
  revision: u64
  objective: string
  success_criteria: string[]
  constraints: string[]
  horizon?: Timestamp
  related_task_ids: TaskId[]
  related_routine_refs: RoutineRevisionRef[]
  related_artifact_refs: ArtifactVersionRef[]
  authored_by: PrincipalRef
  created_at: Timestamp
}
```

Task and Routine references are validated in the same Workspace and stored in the
immutable revision. Artifact links pin an exact `ArtifactVersionRef` from that Workspace;
the referenced Artifact version must exist when the Goal revision is committed. Linking
or unlinking is an ordinary owner-authored Goal revision guarded by `If-Match` and an
idempotency key. Unlinking never deletes the prior revision or the Artifact. Links are
provenance/context only and do not create a second source of Task, Routine, or Artifact
state. Evidence remains discoverable through linked Tasks. The current local
`GoalProgressProjection` reads current linked Task status and bounded committed Evidence
IDs. It remains `PARTIAL` while VerificationRun and Task/Artifact dependency-freshness
readers are unavailable; verified/stale/conflicted counts are null rather than synthetic
zeroes when those dimensions cannot be proved. Only an authenticated owner command may
mark a Goal `COMPLETED`; inferred success may prompt the owner but cannot transition it.
An owner may reopen a completed Goal by changing it to `ACTIVE`; prior completion events
and linked Task outcomes remain intact. Completing or reopening a Goal does not change
linked Tasks or disable linked Routines. An archived Goal is terminal; mutating commands
that require an active Goal return `GOAL_ARCHIVED`, while historical Goal/revision reads
remain available.

`GoalProgressProjection` is factual rather than a semantic judgment of free-text Goal
criteria:

```text
GoalProgressProjection {
  computed_at: Timestamp
  availability: COMPLETE | PARTIAL
  limitations: GoalProgressLimitation[]
  verified_task_count: u64 | null # null when verification state cannot be proven
  linked_task_count: u64
  stale_source_count: u64 | null # null when dependency freshness is unavailable
  conflicted_source_count: u64 | null # null when dependency freshness is unavailable
  contributions: GoalTaskContribution[]
  artifact_evidence_refs: GoalArtifactEvidenceRefs[]
  summary: string
}

`availability: COMPLETE` means all dimensions applicable to the linked Task and Artifact
set were readable; it never means that the Goal's success criteria are satisfied. An
empty linked-work set can therefore have complete projection availability while making
no claim about Goal completion.

GoalArtifactEvidenceRefs {
  artifact_id: ArtifactId
  version: u64
  evidence_refs: EvidenceId[] # only committed Evidence references in this Workspace
}

GoalProgressLimitation =
  VERIFICATION_RUN_READ_MODEL_UNAVAILABLE
  TASK_DEPENDENCY_FRESHNESS_UNAVAILABLE
  ARTIFACT_DEPENDENCY_FRESHNESS_UNAVAILABLE
  ARTIFACT_EVIDENCE_REFERENCE_UNRESOLVED
  EVIDENCE_LIST_TRUNCATED

GoalTaskContribution {
  task_id: TaskId
  task_status: TaskStatus
  outcome_state: VERIFIED | INCOMPLETE | UNVERIFIED | STALE | CONFLICTED
  evidence_refs: EvidenceId[]
}
```

`VERIFIED` means the linked Task's pinned mandatory acceptance criteria passed against
current inputs; it does not mean the Goal itself is achieved. A complete projector may
report verified, stale, or conflicting outcomes only from current Task, VerificationRun,
Evidence, and dependency-freshness readers. Only the owner changes Goal status. The current local projection reads linked
Task status and bounded committed Evidence IDs. Since VerificationRun and dependency
freshness readers are not integrated, verified/stale/conflicted counts are null whenever
those dimensions cannot be proved; `limitations` names each unavailable dimension. A
Goal-pinned Artifact version exposes only its exact version's Evidence references that
resolve to committed same-Workspace Evidence; unresolved references are omitted and
reported. Evidence references are bounded to 1,000 total per projection (200 per Task or
Artifact) and truncation is explicit. A terminal `COMPLETED` Task remains `UNVERIFIED` until its current mandatory criteria can be
matched to passing VerificationRuns and current inputs. `FAILED`, `CANCELLED`, and
`INCOMPLETE` Task statuses are reported as `INCOMPLETE`. This read-only projection is
included in Goal list/get responses and never changes Goal aggregate version.

## Suggestions

A Suggestion is a bounded, expiring proposal with source provenance. It is not a Task,
plan, trigger, authorization, or executable command.

```text
Suggestion {
  suggestion_id: SuggestionId
  workspace_id: WorkspaceId
  coworker_id?: CoworkerId
  dedupe_key: Sha256Digest
  kind: TASK_OPPORTUNITY | ROUTINE_OPPORTUNITY | AUTOMATION_OPPORTUNITY
  reason: string
  source_refs: PinnedResourceRef[]
  goal_refs: GoalRevisionRef[]
  proposed_action: TASK | OPEN_ROUTINE_EDITOR | OPEN_AUTOMATION_EDITOR
  proposed_by: ServiceRef
  proposed_task_spec?: TaskSpecProposal
  estimated_cost?: UsageQuantity
  latency_class_hint?: ExecutionLatencyClass
  status: PROPOSED | ACCEPTED | DISMISSED | EXPIRED
  created_at: Timestamp
  expires_at: Timestamp
  snoozed_until?: Timestamp
  resolved_at?: Timestamp
  resolved_by?: PrincipalRef
  resolution_reason?: ACCEPTED_BY_OWNER | DISMISSED_BY_OWNER | MUTED_KIND | SYSTEM_EXPIRY
  result_task_id?: TaskId
  version: u64
}
```

`kind` is derived deterministically from `proposed_action`: `TASK` maps to
`TASK_OPPORTUNITY`, `OPEN_ROUTINE_EDITOR` to `ROUTINE_OPPORTUNITY`, and
`OPEN_AUTOMATION_EDITOR` to `AUTOMATION_OPPORTUNITY`. `dedupe_key` is computed from
normalized action identity, target, exact source Resource and Goal revision references,
not private prompt text. One unresolved Suggestion per Workspace/dedupe key is allowed.
Expiration is settled by SuggestionService using its clock; expired, dismissed, or
accepted suggestions are terminal. Snooze changes only `snoozed_until`; the Suggestion remains `PROPOSED` and
is hidden until that time, or until its earlier expiry. The owner may clear the value to
show it immediately.

Every source reference pins an exact Resource revision, and every Goal reference pins an
exact Goal revision. This preserves why the proposal was made if inputs later change.

Workspace suggestion preferences may mute any SuggestionKind. Muting atomically resolves
currently proposed Suggestions of that kind as `DISMISSED`, then suppresses new proposals
of that kind. Unmuting affects future proposals only; it does not revive dismissed
records. Dismissing an individual Suggestion suppresses only its exact `dedupe_key` for
30 days. Home shows at most one prominent Idea and the Ideas drawer at most three; these
are projection limits, not domain authority. The card explains why it appeared, which
sources were considered, what acceptance will create, and what permissions may be
required. Source content is untrusted and cannot grant authority.

```text
SuggestionPreference {
  workspace_id: WorkspaceId
  kind: SuggestionKind
  muted: bool
  updated_at: Timestamp?
  version: u64
}
```

An absent preference means `muted=false`, `version=0`, and `updated_at=null`; it is a
virtual default, not a persisted row and has no truthful update time. Settings list all
supported kinds, including defaults. Changes use expected-version checking (including
`If-Match: 0` for a virtual default) and emit `suggestion.preference.changed.v1`; they replicate with
Workspace state. The persisted key is `(workspace_id, kind)`; in the DomainEvent,
`entity_type` is `SuggestionPreference`, `entity_id` is the kind enum, and
`workspace_id` scopes that aggregate key. A mute command and resolution events for its
currently proposed items commit atomically. Expiration and snooze use the SuggestionService Clock. Individual
dismissal cooldown checks only a prior owner dismissal of the exact key with
`resolved_at` in the preceding 30 days.

Accepting a `TASK` Suggestion validates its pinned proposal and creates an ordinary Task
with the normal TaskSpec, lead, budget, and Trust checks; Task creation and Suggestion
resolution commit atomically. The accepted Task begins in `READY`; acceptance itself does
not create a Plan, start an AgentSession, or begin execution. Any later start follows the
ordinary explicit Task admission path. The Task preserves exact source Resource inputs
and, when present, pins its originating Coworker revision. If a required source is
unavailable or its dependency freshness
is STALE/CONFLICTED, acceptance opens a review/update step and cannot silently advance the
reference to a newer version. Accepting an editor action opens a user-confirmed Routine or
Automation editor; nothing is persisted until the owner saves through the existing
Routine/Automation service. A suggestion never silently executes, schedules, authorizes,
installs a package, or sends a message.

## Personal context

Core owns context authorization, source provenance, scope, retention, deletion, and
versioning. Retrieval, embeddings, ranking, and extraction are provider capabilities;
Core does not require a vector database or a hidden memory engine.

```text
PersonalContextProvider {
  search_context(scope, query, max_results) -> ContextMatch[]
  get_profile(scope) -> ContextDocumentRef[]
  get_preferences(scope) -> ContextDocumentRef[]
  get_prior_task_context(scope, task_refs) -> ContextMatch[]
  revoke_memory(source_ref) -> Ack
  list_sources(scope) -> ContextSource[]
}
```

This optional interface is itself a capability and must be granted/scoped like other
capabilities. Retrieved material is untrusted context, never policy or authority.
User-authored context documents are ordinary versioned Workspace Resources with
`context_document` metadata containing a context kind and a typed owner reference. Resource
Service validates the metadata/owner pairing and Workspace scope. They do not need a
parallel content store or revision aggregate.

Context precedence, highest first:

1. Current user instruction and accepted TaskSpec.
2. Explicit current attachments, interpreted only for the requested scope.
3. Current Workspace instruction revision.
4. Current Goal revision when linked to the Task.
5. Current Coworker revision.
6. User-confirmed ContextDocuments.
7. Retrieved prior Task summaries and other provider-returned context.

Lower-priority context cannot override current instructions or policy. Contradictions
that materially affect the Task become a clarification/blocker rather than an inferred
merge. V1 supports user-authored ContextDocument revisions and retrieval only; providers
cannot propose newly extracted personal memory until a separate reviewable proposal
lifecycle exists. Revocation/deletion is honored by Core across provider sources where
supported, with an auditable incomplete-deletion state when an external provider cannot
confirm removal.

## Operator API and services

Coworker, Goal, and Suggestion routes are listed in `API.md` and specified in the
OpenAPI document. Mutations use RequestId/idempotency and `If-Match` where they update a
versioned aggregate. `CoworkerService`, `GoalService`, and `SuggestionService` own their
aggregates. `PersonalContextService` authorizes access to the external provider but does
not implement semantic retrieval itself. `ProjectionService` derives presence, Goal
progress, and suggestion ordering from committed state.

```text
SuggestionService.list(workspace_id, status, cursor) -> SuggestionPage
SuggestionService.snooze(id, snoozed_until, expected_version, RequestId) -> Suggestion
SuggestionService.resolve(id, resolution, expected_version, RequestId) -> Suggestion
SuggestionService.set_kind_preference(workspace_id, kind, muted, expected_version, RequestId) -> SuggestionPreference
SuggestionService.propose(candidate) -> CREATED | SUPPRESSED_MUTED | SUPPRESSED_COOLDOWN
```

`propose` checks current mute preference, the exact-key dismissal cooldown, expiry bounds,
source visibility, and open-key uniqueness in the same admission transaction. Suppressed
candidates create no durable Suggestion and retain no candidate text. List projections
exclude snoozed proposals until their time and settle expired proposals before returning
an actionable page.

Suggestion candidates come only from registered `SuggestionProducer`s; no background
agent polls or self-authorizes new work. A producer may be a deterministic event rule or a
separately authorized read-only capability. Each candidate carries a non-secret
`ServiceRef`, bounded trigger reason, exact source/Goal revisions, proposed action/task
specification, and expiry. The producer cannot commit a Suggestion directly. SuggestionService
validates the producer identity, source access, Workspace/Coworker scope, size limits,
mute/cooldown/dedupe policy, and then records `proposed_by` provenance on an accepted
Suggestion. Candidate text is discarded when suppressed.

## User-visible rules

Normal navigation calls Tasks “Work”; API and architecture continue to call them Tasks.
The Coworker surface shows identity, responsibilities, Goals, Context, access, autonomy,
and enabled worker profiles. Machinery such as AgentSession IDs, lease epochs, provider
handles, worktree paths, and private prompts stays in Inspector. Presence is derived only
from real Task/session/Runtime state, and keeps proactive status, current activity, and
Runtime availability as separate labels. A Coworker has no synthetic typing or working
animation.
