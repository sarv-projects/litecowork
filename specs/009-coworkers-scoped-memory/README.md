# 009 - Coworkers and Scoped Memory

**Status:** DRAFT - review required; does not authorize implementation.
**Revision:** 0.2 | **Updated:** 2026-11-06
**Owners to approve:** Coworkers, Conversation, Memory, Trust, Desktop UI.

## User outcome

The owner optionally creates a Coworker, chats, attaches and revokes scoped context, restarts, and proves another Coworker or Workspace scope cannot read it.

## Current truth

The active checkout serves Coworker identity/revisions/status and Workspace primary selection, and persists Coworker configuration. The Coworker editor also saves lead/delegation/profile allowlist/interaction/failover/context/notification preferences, but most are not consumed by a runtime policy path: default lead is used for Task creation, lead-failover is copied into TaskSpec without an automatic dispatcher, interaction/notification/delegation/context preferences have no production consumer, and child-profile/budget checks exist only in storage admission primitives without a daemon caller. PAUSED status is only partially enforced: Automation enable checks Coworker activity, but existing enabled Automations are not re-fenced at occurrence settlement; the Task composer also rejects PAUSED Coworkers for owner Tasks. Presence returns 503. It does not serve Coworker-owned Conversations, Coworker-scoped ContextDocuments/retrieval, Coworker-private memory management or automatic learning. Quick Create creates an Assistant and optionally sets primary; it does not open a Coworker chat. Coworker Task provenance exists separately from Conversation ownership. The exact source/route/UI matrix is in [IMPLEMENTATION-STATUS.md](../../docs/IMPLEMENTATION-STATUS.md), [SPEC.md](SPEC.md) and [CONTEXT.md](../../docs/CONTEXT.md). This package remains DRAFT and does not authorize implementation until owner decisions and supporting contracts are accepted. Do not infer qualification from a matching route, persisted field, type or screen.

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
