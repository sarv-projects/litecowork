# 001 - Conversation Session Supervisor and Native Turn Execution

- **Status:** IN_REVIEW — core decisions B-01 through B-10 are locked; dependent UI and error/recovery contracts still require owner acceptance.
- **Revision:** 1.0 | **Updated:** 2026-11-06
- **Owners:** Conversation, Agent Runtime, Storage, Operator/API, Desktop UI, Security/Trust and Product.

## User outcome

A user can send an ordinary LiteCowork Conversation message through the exact selected native agent, see truthful progress, stop/retry safely and reload the durable answer after restart, without creating a Task or Coworker.

## Current truth

The active source contains an OpenCode-only supervisor path that reserves and activates a Conversation AgentSession, loads/sends bounded context, consumes the OpenCode response stream and commits the final AGENT message with terminal turn/session settlement. This is CODE-level integration evidence, not a qualified Windows desktop Send journey. The mounted UI nevertheless exposes Send despite the target disabled-until-qualified gate; Stop is ineffective for the OpenCode path, Retry has no UI call site, composer attachments/voice/more-options are inert, and the empty state contains Antigravity placeholder copy. The commit-to-spawn crash window has no durable dispatch outbox/recovery sweep; some supervisor errors are only logged; retry transitions to RUNNING before a reservation path that requires OPEN; and the storage submit path does not enforce the accepted single-flight policy. Keep Send/release claims gated until those issues and exact provider/OS/UI qualification are resolved.

## Package

- [SPEC.md](SPEC.md) - target HLD/LLD, decisions, security and EARS acceptance.
- [PLAN.md](PLAN.md) - phases, dependency DAG, risks and gates.
- [TASKS.md](TASKS.md) - reviewable implementation tasks and blockers.
- [DATA-MODEL.md](DATA-MODEL.md) - reused records, implemented result-write boundary and remaining durability gaps.
- [RESEARCH.md](RESEARCH.md) - sources, verified evidence, alternatives and open questions.
- [WIRING.yaml](WIRING.yaml) - component/owner/transaction graph.
- [contracts/](contracts/README.md) - API/event and failure/recovery contract use.

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Agent Control](../../docs/AGENT-CONTROL.md), [Agent Fabric](../../docs/AGENT-FABRIC.md), [Native Composer LLD](../../docs/NATIVE-COMPOSER-LLD.md), [State Machines](../../docs/STATE-MACHINES.md), [Services](../../docs/SERVICES.md), [Storage](../../docs/STORAGE.md), [Events](../../docs/EVENTS.md), [API](../../docs/API.md), [Schemas](../../docs/SCHEMAS.md), [Trust](../../docs/TRUST.md), [Failure Recovery](../../docs/FAILURE-RECOVERY.md), [Testing](../../docs/TESTING.md).

