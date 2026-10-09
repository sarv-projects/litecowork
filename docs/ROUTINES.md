# Routines

A Routine is a reusable definition of work, independent of when it runs. It is not a
Skill, Automation, Task, or deterministic workflow graph.

```text
Skill       = how an Agent performs a procedure
Routine     = what reusable job the user wants
Automation  = when/why a Routine is triggered
Task        = one durable execution of that work
Workflow    = deterministic graph execution, provided externally when needed
```

## Data contract

```text
Routine {
  routine_id
  workspace_id
  name
  current_revision
  status: ACTIVE | ARCHIVED
  created_at
  updated_at
  version
}

RoutineRevision {
  routine_id
  revision
  objective_template
  instructions
  input_schema: JsonSchema
  constraints[]
  non_goals[]
  required_outputs[]
  acceptance_criteria[]
  approvals_required[]
  input_bindings[]
  required_capabilities[]
  preferred_agent_binding_id?
  placement_preference
  budget_ceiling?
  verification_policy
  authored_by
  created_at
}
```

RoutineRevision is immutable. Inputs are typed bounded values and revision-pinned
ResourceRefs. The objective template and instructions are data, not authority: they cannot
grant capabilities, choose a secret location, weaken approvals, exceed a parent budget, or
override Workspace policy. The selected lead AgentBinding, CapabilityGrants, SecretLeases,
Runtime, and Environment are resolved again for each Task.

An AutomationRevision pins exactly one RoutineRevision. Routine edits never silently
change an existing Automation; the owner explicitly revises the Automation to select a new
RoutineRevision. A Task created from a Routine records `routine_id` and
`routine_revision`; an Automation-created Task also records its Occurrence. The TaskSpec
contains the rendered immutable objective, inputs, constraints, outputs, and acceptance
criteria, so it remains understandable if the Routine is later edited or archived.

## Creation and lifecycle

Routines are user-created or explicitly saved from prior work. “Save as Routine” creates a
reviewable draft with task-specific paths, customer data, secrets, account identifiers,
and incidental context removed or replaced by typed inputs. Nothing is published or
scheduled until the user saves the Routine and separately confirms any Automation.
SkillProposal remains the separate path for reusable procedural `SKILL.md` content.

`ACTIVE -> ARCHIVED` is allowed only after enabled Automations have been paused/disabled
or explicitly rebound to another active revision, and no creation request is in flight.
Existing Tasks/Occurrences keep their pinned revision and history. A Routine cannot be
deleted while referenced by retained Task or Automation provenance.

## Reuse paths

```text
Run now       → materialize one ordinary Task from a pinned RoutineRevision
Automation    → trigger occurrence → Task from the AutomationRevision's pinned Routine
Channel       → authenticated command resolves a Routine → ordinary Task
Webhook       → authenticated event binds typed inputs → ordinary Task
```

Task materialization passes ordinary Task admission. Once Task planning and execution are
enabled, those stages must pass ordinary resource resolution, placement, TrustService,
budget, approval, Attempt, Effect, Artifact, and verification rules. Reusing a Routine
never reuses a prior Task's AgentSession, grant, secret lease, Environment authority, or
approval.

### Desktop/local manual Run now admission

`POST /v1/routines/{id}/run` is an authenticated, selected-Workspace command requiring an
`Idempotency-Key`. The body supplies the exact `routine_revision` and a bounded `inputs`
object. A non-null `conversation_id` is rejected until Conversation and Task admission can
commit atomically. The current local implementation supports only top-level `TEXT` and
exact pinned `RESOURCE_REF` bindings; unsupported schema keywords, nested bindings,
unbound properties, extra values, malformed resource references, unsafe text controls, and
over-limit data are rejected. The desktop renders accessible text fields and same-Workspace
Resource selectors from the saved schema/bindings. Resource choices pin the exact immutable
revision; users are never asked to type opaque Resource IDs. If a Resource cannot be selected
from the loaded catalog, Run remains blocked until it is loaded. Task admission still
rechecks the exact Resource revision atomically.

The route first resolves the immutable revision for bounded materialization, then Task
storage rechecks Workspace ownership, Routine `ACTIVE` status, exact current revision,
revision/input binding, lead eligibility, and same-Workspace Resource revision inside the
same SQLite Task-creation transaction. A failure creates no Task. Success writes an
ordinary Task with status `READY`, TaskSpecRevision(1), exact `routine_id`/revision pins,
Task inputs/outputs/criteria/approvals/constraints, event, and idempotency receipt in one
commit. It creates no Plan, Step, AgentSession, Attempt, lease, Environment, Effect, or
capability grant. A same-key retry by the same authorized owner returns the originally
committed Task; reusing that key with changed inputs conflicts. The page calls it a
“Saved Task · READY” and only opens details after validating returned Workspace, Task,
Routine revision, TaskSpec identity, and READY status.

The immutable RoutineRevision remains the source for fields that TaskSpec does not
represent, including `required_capabilities` and `verification_policy`. Future planning,
Trust, and verifier admission must resolve the exact Task-pinned Routine revision again;
the save-only Run now operation does not claim those checks have run or grant authority.

Input JSON Schema support is intentionally narrower than general JSON Schema: the
top-level object has at most 64 declared properties, no unbound properties, and each
property must bind to one supported top-level TEXT string or exact RESOURCE_REF object.
TEXT allows bounded `minLength`, `maxLength`, and string `enum`; ResourceRef requires
exactly `workspace_id`, `resource_id`, and `revision_id`, all strings, with no additional
properties. Empty `{}` input schema is valid and means no inputs. Requiredness must agree
between the schema and its binding. Unsupported integer/number/boolean/null/array or
arbitrary nested object input shapes are rejected when a revision is saved so the saved
Routine cannot appear runnable while the desktop silently ignores a value.

## Automation relationship

Automation is a trigger definition and execution policy, not the reusable work body. An
AutomationRevision stores `routine_id` plus `routine_revision` and its one-or-more
TriggerSpecs. Occurrences pin both revisions and the normalized trigger input. Editing a
Routine alone does not alter the Automation's next run. “Schedule…” from the composer
first presents the Routine, trigger host, execution requirements, timezone/misfire policy,
and cost/permission summary; the user confirms before durable Automation creation.

## UI

The Automations destination has `Routines | Automations | Runs` views. Routine cards show
name, inputs, output/verification expectations, eligible locations, and pinned revision.
Automation cards show all triggers, trigger host, next occurrence, execution placement,
dependencies/blockers, and last Task outcome. Runs are occurrence/Task history; “completed
schedule” means no future occurrences, not that every produced Task succeeded.

Routines can be run, edited as a new revision, duplicated, archived, or used to create an
Automation. Advanced protocol/package details stay in Inspector/Discover. See
`EXPERIENCE.md`, `AUTOMATION.md`, `API.md`, and `FLOWS.md`.

## RoutineHealth projection

Routine health is a read-only ProjectionService view, never a persisted aggregate or
execution authority:

```text
RoutineHealth {
  routine_id: RoutineId
  current_revision: u64
  last_run_at?: Timestamp
  last_success_at?: Timestamp
  recent_success_rate?: number
  sample_size: u32
  average_duration_ms?: u64
  observed_costs: UsageQuantity[] # separate units/currencies; no conversion
  required_dependency_health: DependencyHealth[]
  drift_state: HEALTHY | WARNING | DRIFTED | UNKNOWN
  drift_evidence_refs: EvidenceId[]
}

DependencyHealth {
  subject_ref: ResourceRef | CapabilityRef
  state: HEALTHY | DEGRADED | UNHEALTHY | UNKNOWN
  observed_at?: Timestamp
  reason_code?: string
}
```

The rate is calculated from the ten most recent terminal Tasks created from the current
RoutineRevision; terminal COMPLETED counts as success, FAILED/INCOMPLETE as unsuccessful,
and CANCELLED/SKIPPED/nonterminal runs are excluded. Fewer than three eligible Tasks is
shown as “Not enough runs yet” instead of a percent. Duration covers Task admission to
terminal settlement. Cost is grouped by comparable unit/currency and remains Unknown when
source observations are absent, stale, or incomparable. The latest dependency observation
is freshness-qualified; absence is `UNKNOWN`, not Healthy.

`DRIFTED` requires explicit incompatibility evidence from a pinned Skill/capability
preflight or a verifier bound to the current RoutineRevision. A transient provider error
or failed Task alone is `WARNING`. The Task receives `SKILL_DRIFT_DETECTED` and a Needs You
item before unsafe replay. The user may request a repair proposal; SkillProposal review,
redaction, and LiteSPM publication rules still apply. Published Skill changes do not
rewrite RoutineRevision or AutomationRevision pins: the owner reviews a new Routine
revision and separately updates affected Automation pins. Repair never mutates a package
or reruns the Task silently.

## Coworker standing responsibility integration target

A StandingResponsibility may link several version-pinned Routine revisions; it does not own or execute them. Enable from chat compiles a plain-language What/When/Where/review proposal, then creates or links the actual Routines and Automations through their existing authority. A run pins the selected RoutineRevision and TaskSpec exactly. Edits cannot mutate prior runs. Repeated or triggered work never runs when source access is revoked, lead is ineligible, machine unavailable, or the Responsibility/Coworker is paused. A missing required app or tool produces a durable dependency blocker with a Connect action, NOT an implicit install or guessed method. A one-shot manual Task does not require a StandingResponsibility.
