# LiteCowork implementation plan

Status: active source implementation; product qualification remains unverified. V1 is
desktop/local; cloud continuation and remote Runtime are post-V1.
Baseline: `8af59a5b9be0a00b12849caf40eac287527ac43e`. Prepared 2026-10-05.

This is the delivery plan for coding agents with one owner reviewing the work. V1 covers
the complete desktop/local product; cloud continuation and remote Runtime are planned in
post-V1 tracks. The architecture and domain documents remain contract authority. This
folder owns execution ordering, story acceptance, evidence, research references, and
release gates.

## 2026-10-22 current V1 release-definition pass

Start with [ten-area product contract](../docs/V1-PRODUCT-RELEASE-CONTRACT.md),
[final screen contract](../docs/V1-UI-SCREEN-CONTRACT.md),
[LiteSPM interop LLD](../docs/LITESPM-LITECOWORK-INTEROP-LLD.md),
[HorizonCode native-agent LLD](../docs/V1-HORIZONCODE-NATIVE-AGENT-LLD.md)
and [product acceptance matrix](V1-PRODUCT-ACCEPTANCE-MATRIX.md).
These are not a new Task engine, package database or story graph.
The current native Send, Attempt admission, LiteSPM bridge and other
end-to-end V1 journeys remain blocked pending qualified source evidence.

## Read in order

1. [Current work and agent handoff](CURRENT-RUN.md)
2. [Scope and release gates](SCOPE.md)
3. [Current contract audit and optimizations](AUDIT.md)
4. [Stack recommendations and qualification spikes](STACK.md)
5. [Agent development and Agile process](PROCESS.md)
6. [Roadmap](ROADMAP.md), then the dependency-ready [story backlog](backlog.json)
7. [UI delivery](UI.md) and [file/folder/ZIP retrieval delivery](RAG.md)
8. [Code, implementation, and real-work testing](TESTING.md)
9. [Practical Cowork replacement assessment](WORKFLOWS.md)
10. [Research and repository reference catalogue](SOURCES.md)
11. [Production operations and release checklist](RELEASE.md)
12. [Deferred licensing and packaging research](licensingandpkacgingresearch.md)

[Architecture coverage](ARCHITECTURE-COVERAGE.md) lists every tracked architecture source,
heading, ADR, flow, benchmark, story, and enumerated machine-contract object with a primary
implementation guardian. This is traceability, not proof of implementation. The
[baseline audit inventory](audit-inventory.csv) records the source files, sizes, and digests
examined. The generated [machine inventory](machine-inventory.json) is checked against the
current API, schemas, events, and SQLite contract.

Run `python3 scripts/validate_implementation_plan.py` alongside the architecture validator.
This also runs the coverage checker. The checks prove contract and backlog consistency; they
do not prove that the product works. The owner must implement and review each story's
CODE, SYSTEM, and USER evidence before accepting it.

## Current target UX and source-status distinction

The reviewed user requirements are preserved in [Coworker target](../docs/COWORKERS-TARGET.md) and detailed controls/tests in [UI](UI.md). Every owner should reconcile the current source, machine contracts and acceptance cases before implementing the target. Older snapshots in CURRENT-RUN are historical; do not present partial native-agent execution, simulated memory, or placeholder buttons as completed features.


## Agent-module implementation handoff

For the current Codex/OpenCode source seam and the exact refactor into modular agent
lifecycle/configuration, read
[AGENT-MODULE-AUDIT.md](AGENT-MODULE-AUDIT.md) before E03-S06–S11 or E04-S06 work.
It maps existing files/functions to target services, storage, Tauri/UI APIs, deletion
gates, research dependencies and CODE/SYSTEM/USER tests. Architecture authority remains
docs/AGENT-CONTROL.md and docs/AGENT-FABRIC.md.


## Final connector/learning design inputs — 2026-10-10

Before implementing connector breadth, schedules/provider events, external workflows or
Coworker learning, read:

- ../docs/REFERENCE-IMPLEMENTATIONS.md
- ../docs/CONNECTORS-WORKFLOWS.md
- ../docs/LEARNING.md

These targets intentionally preserve existing Core authority. Promote them slice by slice
with matching migrations, machine contracts, UI truth and CODE/SYSTEM/USER evidence.

## V1 detailed design integration — 2026-10-12

Start with ../docs/V1-DESIGN-INDEX.md, V1-DESIGN-COVERAGE.md and the exact epic; read relevant new LLD only. See ../docs/V1-FUNCTION-FILE-CATALOG.md for function/file ownership. New browser/agent native parity, rendering and shell requirements map to existing E03/E07/E08/E09/E10 stories; do not create a second roadmap or mark designs as completed code.

## 2026-10-13 implementation reconciliation handoff

Read [V1 contract reconciliation](V1-CONTRACT-RECONCILIATION-2026-10-13.md) immediately after the current run and before dependency-ready E03/E07/E08/E13 work. It maps the corrected canonical API/schema/security/memory contracts to existing stories, code/system/user tests, platform release blockers and their truthful current statuses. Use the backlog as the status/dependency authority; a passing documentation validator does not unblock Task planning, native Send, Windows roots, browser/terminal or private automatic learning.

## 2026-10-13 implementation re-audit

Read [the source-backed audit and remaining V1 work](V1-IMPLEMENTATION-REAUDIT-2026-10-13.md)
and the current evidence at [CURRENT-RUN](CURRENT-RUN.md).
The audit lists open source defects, corrected documentation contradictions, tooling
gaps, exact observed checks and remaining E01–E10/E13 qualification. It does not
promote story status or certify universal semantic/runtime correctness. The 2026-10-13
next-session handoff is historical. For the latest verified worktree, active ownership,
remaining local V1 story inventory, and restart instructions, use the
[2026-10-20 next-session handoff](NEXT-SESSION-HANDOFF-2026-10-20.md) together with
[CURRENT-RUN](CURRENT-RUN.md) and machine status in [backlog.json](backlog.json).
