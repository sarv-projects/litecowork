# 010 - Automations and Recurring Host

**Status:** DRAFT - review required; does not authorize implementation.
**Revision:** 0.2 | **Updated:** 2026-11-06
**Owners to approve:** Automation, Task Runtime, Runtime lifecycle, Desktop UI.

## User outcome

A saved schedule or event creates exactly one Task per occurrence, including across sleep and restart, with truthful history and notification status.

## Current truth

The active checkout contains the Rust daemon, Tauri desktop application and shared crates. This package remains DRAFT and does not authorize implementation until owner decisions and supporting contracts are accepted. Its current implementation boundary is mapped in docs/IMPLEMENTATION-STATUS.md; do not infer qualification from a matching route, type or screen.

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

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## Workflow boundary

This package owns Automations, occurrences and Routine-to-Task materialization; it does not own a native graph editor or workflow runtime. The target is to connect existing external workflows through a future qualified WorkflowBackendAdapter and invoke them as bounded CapabilityInvocations within Tasks. No production adapter is present in the active source tree, so selection/invocation is not currently a served LiteCowork feature. Users may separately use n8n Assistant/MCP or Activepieces Build by Asking/MCP inside those provider products, subject to provider availability and permissions. A LiteCowork Coworker cannot currently mutate the provider graph; direct Coworker graph-authoring tools are deferred to 010-T06 and require a future provider-specific authorization decision. See [CONNECTORS-WORKFLOWS](../../docs/CONNECTORS-WORKFLOWS.md), [SECURITY](../../docs/SECURITY.md) and the [OpenBot source study](../../docs/OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md).
