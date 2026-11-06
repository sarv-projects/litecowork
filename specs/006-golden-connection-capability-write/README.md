# 006 - Golden Connection and One Capability Write

**Status:** DRAFT - review required; does not authorize implementation.
**Revision:** 0.3 | **Updated:** 2026-11-06
**Owners to approve:** Connectors, Capability Fabric, Security, LiteSPM boundary, Desktop UI.

## User outcome

The owner completes one real account Connection, performs one authorized read, then one consequential write with approval, idempotency and reconciliation, and revoke fences later use.

## Current truth

The active checkout serves Discover projections, fail-closed connect-or-install/test routes, exact Approval list/get/resolve and a narrow Task-planning Resource-read grant. Connect-or-install returns 503 without a receipt or grant; test returns UNKNOWN without contacting a provider. Account Connection/OAuth routes are not mounted; CapabilityInvocation is read-only; Trust/Effect provider dispatch and reconciliation are not wired. The Approval UI records an exact action decision but does not execute it. This package remains DRAFT; no golden provider read/write journey is qualified.

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
