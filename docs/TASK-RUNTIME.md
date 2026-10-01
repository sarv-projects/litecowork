# Task Runtime

The Task Runtime owns durable user intent and execution admission. It does not reason
about how to solve the user's problem; an external lead agent proposes and revises the
plan.

## Records

```text
Task
  TaskSpecRevision*
  PlanRevision*
  Step*
    Attempt*
      AgentBinding
      AgentSession (optional/replacable)
      Runtime
      Environment
      CapabilityGrant*
      ExecutionLease
      ArtifactVersion*
      Effect*
      Evidence*
```

Conversation and Task are distinct. One Conversation may have no Task, one Task, or
multiple Tasks. A Task can be surfaced through several operator and human-channel
adapters while retaining one identity.

## Task specification

Each immutable TaskSpecRevision contains objective, constraints, non-goals, required
outputs, acceptance criteria, approval requirements, budget, source-message references,
author, and creation time. Intent changes create a new revision; history is retained.

## Plans and Steps

PlanRevision records its source Attempt, TaskSpec revision, ordered/dependent Steps,
reason for revision, and timestamp. The lead agent owns strategy and decomposition.
Core stores versions, validates required structure and policy, displays the plan, and
tracks dependencies. Core does not silently rewrite plans or decide which work is
intellectually important.

## Attempts and delegation

An Attempt is a single execution try for one Step. It snapshots agent binding, Runtime,
Environment, capability-grant references, lease epoch, budgets, and input artifact
versions. A retry or replacement worker creates a new Attempt with explicit provenance.

Host delegation creates a child Attempt from a bounded contract:

```text
DelegateRequest {
  parent_attempt_id, objective, input_refs[], required_capabilities[],
  acceptance_criteria[], preferred_agent?, placement_preference?, isolation,
  budget, deadline?, return_schema
}
```

The child receives an objective, constraints, bounded context, resource references,
output contract, acceptance checks, capability grants, deadline, and budget. It does not
receive the parent's complete private transcript. A ResultEnvelope returns status,
summary, output/artifact/evidence references, unresolved questions, blockers, usage, and
optional confidence.

Native subagents remain inside their owning agent and are not host-created Attempts.
When visible, they are displayed with their native/reported provenance.

## Completion

`task.finish` is a completion proposal. The Task Runtime checks that required outputs
exist, required child Attempts are settled, approvals are resolved, ambiguous Effects
are reconciled, and acceptance criteria have adequate evidence. Otherwise the Task
remains verifying, needs-user, incomplete, or blocked.

## Portable checkpoint

ResumePacket contains the TaskSpec and Plan revisions, active Step, completed Steps,
important decisions, artifact/evidence references, unresolved questions, failed
strategies, remaining criteria, and Effect reconciliation state. AgentSession and
Environment snapshots are optional provider optimizations. No Task depends on them for
correctness.
