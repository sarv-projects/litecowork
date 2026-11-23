# 006 - Golden Connection and One Capability Write

**Status:** IN_REVIEW - revision 1.1 carries USER-DELEGATED direction; formal domain-owner ratification remains pending.
**Revision:** 1.1 | **Updated:** 2026-11-20
**Implementation state (2026-11-20):** No Connection service/storage writer, device authorization flow, account UI, or external provider dispatch is served. T02 is complete as a USER-DELEGATED design direction: GitHub App user-to-server device flow, expiring tokens, `Issues: write` app permission, and a narrower LiteCowork repository/operation grant. T01 canonical schema/API promotion and implementation tasks remain NOT STARTED; migration v41 remains allocated. E2E-05 Task-write platform order remains OPEN under register A-80 because its Environment requirement is unresolved.
**Revision 1.0 history:** B-19 and B-20 were accepted via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md); revision 1.1 supersedes only the B-19 auth recipe for current implementation planning. B-20 remains the golden issue-comment write. Revision 1.1 is not owner acceptance, implementation, INTEGRATED evidence or qualification.
**Owners:** Connectors, Capability Fabric, Security, LiteSPM boundary, Desktop UI.

## User outcome

The owner completes one real account Connection, performs one authorized read, then one consequential write with approval, idempotency and reconciliation, and revoke fences later use.

## Current truth

The active checkout serves Discover projections, fail-closed connect-or-install/test routes, exact Approval list/get/resolve and a narrow Task-planning Resource-read grant. Connect-or-install returns 503 without a receipt or grant; test returns UNKNOWN without contacting a provider. Account Connection/authentication routes are not mounted; CapabilityInvocation is read-only; Trust/Effect provider dispatch and reconciliation are not wired. The Approval UI records an exact action decision but does not execute it. No golden provider read/write journey is implemented or qualified. The 2026-11-20 provider review led to the delegated revision 1.1 device-flow direction; A-82 formal owner ratification is still pending and no provider credentials or live evidence exist.

Cross-feature review found that T15's former Windows-first wording could conflict with Feature 005's Linux-first Task execution rider if the Task-scoped `STRUCTURED_API` write needs an admitted Environment. Feature 006 SPEC §18 leaves that requirement open; the dependency is now tracked as A-80. T15 must separate Connection/read evidence from Task-write qualification until Trust and Environments owners resolve it.

## Boundary

006 does not own the LiteSPM package lifecycle (observe, adopt, install, update, repair, unmanage, remove, inventory, Discover Managed/Installed tabs, E2E-06). That is package 013, litespm-package-lifecycle-bridge. 006 owns the account Connection, scoped read and write, approval, UNKNOWN reconciliation and revoke.

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
