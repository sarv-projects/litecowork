# 001 - Conversation Session Supervisor and Native Turn Execution

**Status:** ACCEPTED/LOCKED — normative specification locked and ready for execution.  
**Revision:** 1.0 | **Updated:** 2026-11-01  
**Owners:** Conversation, Agent Runtime, Storage, Operator/API, Desktop UI, Security/Trust and Product.

## User outcome

A user can send an ordinary LiteCowork Conversation message through the exact selected native agent, see truthful progress, stop/retry safely and reload the durable answer after restart, without creating a Task or Coworker.

## Current truth

The preserved source contains USER+OPEN turn commit, immutable host resolution, context-package read, STARTING reservation/LOST recovery and a storage-only atomic readiness activation foundation. No qualified supervisor invokes activation, dispatches the context, persists a final AGENT response with terminal turn settlement, or enables end-to-end Send. The route remains fail-closed and reports admission unavailable.

## Package

- [SPEC.md](SPEC.md) - target HLD/LLD, decisions, security and EARS acceptance.
- [PLAN.md](PLAN.md) - phases, dependency DAG, risks and gates.
- [TASKS.md](TASKS.md) - reviewable implementation tasks and blockers.
- [DATA-MODEL.md](DATA-MODEL.md) - reused records and proposed result-write boundary.
- [RESEARCH.md](RESEARCH.md) - sources, verified evidence, alternatives and open questions.
- [WIRING.yaml](WIRING.yaml) - component/owner/transaction graph.
- [contracts/](contracts/README.md) - API/event and failure/recovery contract use.

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Agent Control](../../docs/AGENT-CONTROL.md), [Agent Fabric](../../docs/AGENT-FABRIC.md), [Native Composer LLD](../../docs/NATIVE-COMPOSER-LLD.md), [State Machines](../../docs/STATE-MACHINES.md), [Services](../../docs/SERVICES.md), [Storage](../../docs/STORAGE.md), [Events](../../docs/EVENTS.md), [API](../../docs/API.md), [Schemas](../../docs/SCHEMAS.md), [Trust](../../docs/TRUST.md), [Failure Recovery](../../docs/FAILURE-RECOVERY.md), [Testing](../../docs/TESTING.md).

