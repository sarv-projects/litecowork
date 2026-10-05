# Agile process for coding agents and one reviewer

## Working cadence

Use a rolling backlog and short review cycles. A sprint is a one-week planning window,
not a promise of a week's fixed feature output. Select one small vertical outcome plus
necessary enabling work. Measure actual completion/review time for the first three
increments before forecasting. Break any story that cannot produce a reviewable result
in a few focused agent sessions into dependent children retaining the parent acceptance.

WIP: one implementation story awaiting owner review, plus at most one independent
research/spike story. The default agent completes its story itself. Multiple agents are
used only when the owner authorizes them and assigns non-overlapping paths/contracts.
The owner need not answer routine reversible implementation choices.

Every session reads AGENTS, architecture, the story and its owning contracts; records
branch/status; identifies exact affected commands, schemas, events, transactions,
authorization, failure cases and UI. Work from current source, never historical archives.

## Definition of ready

- Story ID, user outcome and dependencies are concrete; dependencies are accepted.
- Owner contracts and exact API/event/schema versions are listed.
- Trust scope, recovery behavior, data placement and UI states are understood.
- Deterministic fixtures and real-world acceptance oracle exist or are part of the story.
- Unknown provider semantics have a named spike/blocker, not an invented endpoint.
- Planned paths, test commands, review demo and rollback are named.

## Agent work cycle

1. Produce a short implementation note with contract references and touched paths.
2. Implement the smallest end-to-end behavior behind deep, testable module interfaces.
3. Add/run relevant unit, property, contract, integration and system tests; keep test-only
   adapters labeled. Do not publish a simulated provider as a working integration.
4. Run affected invariant/failure tests; inspect logs/artifacts for secrets and honest UI.
5. Demonstrate the acceptance case using the real adapter/build when available.
6. Inspect the complete diff, stage intended paths and commit a coherent milestone.
7. Produce the review packet below. Continue independent useful work if a dependency is
   blocked; do not silently mark the blocked story done.
8. Owner accepts or requests correction. Update status/evidence references. Merge/push
   according to the owner's established repository workflow and branch protection.

## Definition of done

Contract alignment; compilable behavior; positive and negative tests; crash/race/replay
coverage where relevant; real-provider qualification; truthful UI; observability;
installation/upgrade impact; docs; no placeholders on a production path. Existing flows
and benchmarks assigned to the story have executable cases with recorded outcomes.
A reviewer must be able to replay the demo without relying on the agent's narrative.

Status vocabulary in backlog: PLANNED, READY, IN_PROGRESS, IN_REVIEW, BLOCKED, ACCEPTED.
A separate execution evidence record names exact build/version/OS/provider/fixtures,
commands, outputs and reviewer acceptance. This initial plan has no ACCEPTED stories.

## Review packet

```text
Story / child story IDs:
Commit and base:
Outcome and touched modules:
Contract/API/event/schema references:
Commands and test results:
Real adapter versions and OS:
Fixture digests and expected vs observed outputs:
Failure/race/authorization checks:
Screenshots/accessibility/motion evidence when applicable:
Known limitations, cost and measured performance:
Upgrade/rollback/demo instructions:
Assigned flows/benchmarks and evidence locations:
Owner decision and corrections:
```

Keep personal test data and raw production diagnostics out of Git. Commit synthetic
fixtures and sanitized evidence summaries; retain private live evidence under owner
control. Public issue/PR text must never include credentials or private artifacts.

## Planning, review and retrospective

At planning, select dependency-ready stories from roadmap; list the demo and risk being
retired. Daily async update: completed evidence, next outcome, actual blocker. Review:
owner sees installed behavior and tests. Retrospective: reduce one source of repeated
rework, flaky tests, review overload or unclear contracts. Update forecasts from measured
cycle time, not agent token counts. Track escaped defects and time-to-verified-output.

## Change management

A discovered contradiction is a contract task before implementation invents behavior.
Update architecture and owning documents together for ownership/invariant changes;
use an ADR for enduring non-obvious tradeoffs. Preserve coverage IDs when splitting work.
New finalized requirements acquire stories/tests/source references and a release impact.
No repeated architecture overhaul without implementation or benchmark evidence.
