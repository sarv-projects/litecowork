# 013 - LiteSPM Package Lifecycle Bridge

- **Status:** IN_REVIEW - authored 2026-11-09; not owner-accepted; does not authorize implementation; not a claim of implementation or qualification.
- **Revision:** 0.1 | **Updated:** 2026-11-12 | **Open blockers:** B-36 (owner-approved MCP bridge pin/tool schemas, Runtime-owned process identity and consent/recovery path) and B-37 (first package kind and host for plan/apply), both PROPOSED, pending owner.
- **Implementation state (2026-11-10):** DEFERRED by the owner on 2026-11-10 (A-62): every LiteSPM task. Nothing implemented; migration v40 allocated and unused. See the DEFERRED note at the top of TASKS.md for why, what unblocks it and where to test later.
- **Owners:** Capability Fabric (LiteSPM boundary), Operator/API, Storage, Trust/Approval, Desktop UI, Security and Product. Upstream LiteSPM owns catalog, inventory, host registration, mutation and lifecycle.

## User outcome

The owner sees in Discover what LiteSPM reports as already on this computer and what it manages, reviews an exact plan to install or adopt one supported package for one supported agent and scope, approves that exact plan, and gets LiteSPM's receipt and a refreshed inventory. Lost responses and crashes are reconciled, never repeated. Update, repair, stop managing and remove use the same plan, approval and receipt path. Installation never grants use; Feature 006 owns assignment and grants.

## Current truth

Discover serves Workspace-local skills and up to 50 Runtime capability-host observations (`apps/litecoworkd/src/discover_operator.rs`, `crates/storage-sqlite/src/discover.rs`). `POST /v1/capabilities/{capability_id}/connect-or-install` always returns 503 `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`. The desktop Managed and Connections tabs say "Not available" and every row action is disabled (`apps/litecowork-ui/src/discover/`). `classify_agent_capability_route` exists with no caller. No LiteSPM client exists, and the corpus carries three different LiteSPM pins.

Under this package's IN_REVIEW status, proposed task P-17 is folded in as 013-T20: pin the published MCP bridge package/protocol, mirror only the upstream safety guarantees verified at that pin, and adopt single-flight plus backoff for Discover read-through. HEAD `84d5fab` (v0.6.0) is a research snapshot, not an owner-approved integration pin. The 2026-11-09 study's direct-daemon-client conclusion is superseded by its 2026-11-12 architecture correction; Feature 013 must not invent a private daemon transport. Re-read [RESEARCH.md](RESEARCH.md) and [docs/LITESPM-INTEGRATION-STUDY-2026-11.md](../../docs/LITESPM-INTEGRATION-STUDY-2026-11.md) before any future implementation.

## Package

- [SPEC.md](SPEC.md) - problem, decisions (B-36, B-37), HLD/LLD, security, EARS acceptance.
- [PLAN.md](PLAN.md) - phases, dependency graph, gates.
- [TASKS.md](TASKS.md) - 20 ordered tasks to V1 VERIFIED.
- [DATA-MODEL.md](DATA-MODEL.md) - approval, receipt and service-observation records; no inventory storage.
- [RESEARCH.md](RESEARCH.md) - pinned sources, alternatives, open research.
- [WIRING.yaml](WIRING.yaml) - components, edges, transactions, external boundaries.
- [contracts/](contracts/README.md) - API/event, errors/recovery and UI behavior.

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [V1 release contract](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [LiteSPM interop LLD](../../docs/LITESPM-LITECOWORK-INTEROP-LLD.md), [LiteSPM integration plan](../../docs/LITESPM-INTEGRATION-PLAN.md), [Capability Fabric](../../docs/CAPABILITY-FABRIC.md), [Capability Invocations](../../docs/CAPABILITY-INVOCATIONS.md), [Trust](../../docs/TRUST.md), [API](../../docs/API.md), [Storage](../../docs/STORAGE.md), [Events](../../docs/EVENTS.md), [Failure Recovery](../../docs/FAILURE-RECOVERY.md), [Acceptance and E2E plan](../../docs/ACCEPTANCE-AND-E2E-PLAN.md).
