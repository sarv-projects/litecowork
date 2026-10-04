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

Every run passes ordinary Task admission, resource resolution, placement, TrustService,
budget, approval, Attempt, Effect, Artifact, and verification rules. Reusing a Routine
never reuses a prior Task's AgentSession, grant, secret lease, Environment authority, or
approval.

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
