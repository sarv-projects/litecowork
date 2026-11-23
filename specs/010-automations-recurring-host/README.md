# 010 - Automations and Recurring Host

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Revision:** 1.0 | **Updated:** 2026-11-09 | **Owner acceptance:** B-26, B-27 and the A-35 quiet-hours ownership accepted 2026-11-09 by the repository owner via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md).
**Implementation state (cross-checked 2026-11-13):** local schedule hosting and occurrence-to-READY-Task materialization are CODE/SERVED foundations, not Task execution or qualification. T02's truthful-copy subcriterion is CODE; broader AC-019 pagination remains under T11. T03/T04/T05/T07/T11/T12/T13 remain PARTIAL. T06/T10/T14/T16/T17 are NOT STARTED. T08/T09/T15 are BLOCKED; T18 is outside the V1 release gate; T19 is DEFERRED. Migration v45 is only a provisional reservation pending exact schema-delta review (A-98). See [current cross-check](../../docs/audit/FEATURE-010-CURRENT-CROSSCHECK-2026-11-13.md).
**Owners:** Automation, Task Runtime, Runtime lifecycle, Desktop UI.

## User outcome

A saved schedule or event creates exactly one Task per occurrence, including across sleep and restart, with truthful history and notification status.

## Current truth

The active checkout contains the Rust daemon, Tauri desktop application and shared crates. This package is ACCEPTED/LOCKED (owner-accepted 2026-11-09). That authorizes implementation in the order given by [TASKS.md](TASKS.md); it is not a claim that delivery, event triggers or Task execution exist. Its current implementation boundary is mapped in docs/IMPLEMENTATION-STATUS.md; do not infer qualification from a matching route, type or screen.

## Package

- [SPEC.md](SPEC.md)
- [PLAN.md](PLAN.md)
- [TASKS.md](TASKS.md)
- [DATA-MODEL.md](DATA-MODEL.md)
- [RESEARCH.md](RESEARCH.md)
- [WIRING.yaml](WIRING.yaml)
- [contracts/](contracts/README.md)

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Master plan](../../docs/IMPLEMENTATION-MASTER-PLAN.md), [Specification standard](../../docs/SPECIFICATION-STANDARD.md). Domain narrative: [RESEARCH.md](RESEARCH.md).


## Workflow boundary

This package owns Automations, occurrences and Routine-to-Task materialization; it does not own a native graph editor or workflow runtime. The target is to connect existing external workflows through a future qualified WorkflowBackendAdapter and invoke them as bounded CapabilityInvocations within Tasks. No production adapter is present in the active source tree, so selection/invocation is not currently a served LiteCowork feature. Users may separately use n8n Assistant/MCP or Activepieces Build by Asking/MCP inside those provider products, subject to provider availability and permissions. A LiteCowork Coworker cannot currently mutate the provider graph; direct Coworker graph-authoring tools are deferred to 010-T19 and require a future provider-specific authorization decision. See [CONNECTORS-WORKFLOWS](../../docs/CONNECTORS-WORKFLOWS.md), [SECURITY](../../docs/SECURITY.md) and the [OpenBot source study](../../docs/OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md).
