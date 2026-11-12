# 015 - Task Control and Heterogeneous Delegation

- **Status:** IN_REVIEW - authored complete for owner review; does not authorize implementation until the owner accepts the SPEC and blocker B-39.
- **Revision:** 0.1 | **Updated:** 2026-11-12 | **Decision:** B-39 (first heterogeneous delegation pair and Environment sharing rule) ACCEPTED as proposed 2026-11-10 (A-62); whole-SPEC acceptance pending, package stays IN_REVIEW.
- **Implementation state (2026-11-12 source audit):** The checkout contains partial Rust admission, lead-change, result, escalation and verification foundations, plus served lead-binding change and read-only verification-runs routes. No Task-control or delegation producer/dispatch journey is integrated or qualified. Migration v42 is allocated.
- **Owners:** Task Runtime, Delegation, Storage, Operator/API, Desktop UI (Work screen), Security/Trust and Product.
- **Release rows:** Linux x86_64 first; Windows x64 UNAVAILABLE until Feature 005 B-35 qualifies.

## User outcome

On the Work screen the owner can pause, resume, steer and cancel a Task, inspect truthful Activity and Workers projections, and change the assigned lead. Lead-binding change and replacement of an execution Attempt are separate operations: changing the lead does not silently replace or rebind existing Attempts. The lead can delegate one eligible Step to a different native agent whose result an independent agent verifies. A late result from a superseded Attempt never changes the Task.

## Current truth

Child-admission decisions, lead-failover policy, lead-handoff packets and escalation policy exist as Rust domain code in `crates/domain-task` without a production producer. The served pieces are the lead-binding change route (`POST /v1/tasks/{task_id}/lead-agent`, which changes the binding only) and the read-only verification-runs list. Pause, resume, steer, Task-level cancel fan-out, worker dispatch, independent verification and the Work-screen timeline are absent. Feature 005 currently specifies a bounded file-write Attempt slice, not the generic provider AgentSession/Attempt admission interface assumed by several 015 tasks; owner alignment is required before those dependencies are implementable (SPEC O-06).

Under this package's IN_REVIEW status, proposed task P-14 is folded in as 015-T20: a secondary Mission Control canvas in Work > Task detail, bound to real runtime records after the first bounded workflow is proven, plus an explicitly-requested read-only Compare Agents fan-out. The Compare Agents control/path stays blocked by SPEC O-09 until its Conversation/Agent Runtime owner, UI surface, request/result identity, persistence and recovery are defined. Source: [docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md](../../docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md) section 8.

## Package

- [SPEC.md](SPEC.md) - problem, decisions (B-39), HLD/LLD, security, EARS acceptance.
- [PLAN.md](PLAN.md) - phases, dependency graph, gates, risks.
- [TASKS.md](TASKS.md) - ordered tasks to VERIFIED for V1.
- [DATA-MODEL.md](DATA-MODEL.md) - records, lifecycles, fences, migration.
- [RESEARCH.md](RESEARCH.md) - pinned references (OpenHands) and decisions.
- [WIRING.yaml](WIRING.yaml) - components, edges, transactions, boundaries.
- [contracts/](contracts/README.md) - API/events, errors/recovery, UI behavior.

## Dependencies

Hard: Feature 005 (Attempt admission, Environment, cancel, recovery). Also Feature 002 and Feature 004 (Claude Code as second agent), Feature 009 (Coworker worker allowlist), Feature 011 (IPC), Feature 019 (release rows), Feature 020 (Needs You destination).

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Task Runtime](../../docs/TASK-RUNTIME.md), [Delegation](../../docs/DELEGATION.md), [State Machines](../../docs/STATE-MACHINES.md), [Environments](../../docs/ENVIRONMENTS.md), [Trust](../../docs/TRUST.md), [API](../../docs/API.md), [Events](../../docs/EVENTS.md), [Storage](../../docs/STORAGE.md), [Failure Recovery](../../docs/FAILURE-RECOVERY.md), [UI screen contract](../../docs/V1-UI-SCREEN-CONTRACT.md).
