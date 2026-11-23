# 009 - Coworkers and Scoped Memory

**Status:** IN_REVIEW (revision 1.1 amendment; revision 1.0 ACCEPTED/LOCKED on 2026-11-09).
**Revision:** 1.1 | **Updated:** 2026-11-20 | **Owner acceptance:** revision 1.0 decisions B-24 and B-25 were accepted 2026-11-09 by the repository owner via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md); the A-91 amendment remains pending owner review. The automatic LearningCandidate pipeline moves to Feature 020. Coworker delegation-source completion is recorded as USER-DELEGATED direction in T19; the package remains IN_REVIEW.
**Implementation state (source cross-check 2026-11-20 at bdd3168):** No new Coworker Conversation, scoped-memory, assignment or responsibility implementation is present beyond pre-existing partial foundations. The canonical OpenAPI and SQLite v28 schema contain `delegation_policy_source`, but Rust, Operator, SQLite projection and UI do not yet round-trip it; T19 records its USER-DELEGATED direction and implementation scope. T16 still depends on qualified Feature 001/002 rows. Migration v39 is reserved but unwritten; A-91 blocks finalizing its manual-memory fields.
**Owners:** Coworkers, Conversation, Memory, Trust, Desktop UI.

## User outcome

The owner optionally creates a Coworker, chats, attaches and revokes scoped context, restarts, and proves another Coworker or Workspace scope cannot read it.

## Current truth

The active checkout serves Coworker identity/revisions/status and Workspace primary selection, and persists Coworker configuration. The canonical OpenAPI and SQLite v28 schema contain `delegation_policy_source`, but the Rust definition, Operator, SQLite read/write projection and editor have not yet carried it end to end; T19 records the USER-DELEGATED direction and its current implementation evidence. The editor also saves lead/delegation/profile allowlist/interaction/failover/context/notification preferences, but most are not consumed by a runtime policy path: default lead is used for Task creation, lead-failover is copied into TaskSpec without an automatic dispatcher, interaction/notification/delegation/context preferences have no production consumer, and child-profile/budget checks exist only in storage admission primitives without a daemon caller. PAUSED status is only partially enforced: Automation enable checks Coworker activity, but existing enabled Automations are not re-fenced at occurrence settlement; the Task composer also rejects PAUSED Coworkers for owner Tasks. Presence returns 503. It does not serve Coworker-owned Conversations, Coworker-scoped ContextDocuments/retrieval, Coworker-private memory management or automatic learning. Quick Create creates an Assistant and optionally sets primary; it does not open a Coworker chat. Coworker Task provenance exists separately from Conversation ownership. The exact source/route/UI matrix is in [IMPLEMENTATION-STATUS.md](../../docs/IMPLEMENTATION-STATUS.md), [SPEC.md](SPEC.md) and [CONTEXT.md](../../docs/CONTEXT.md). Revision 1.0 remains accepted; revision 1.1 awaits owner approval. A-91 blocks finalizing the manual-memory representation. Neither status claims the missing behavior above exists; do not infer qualification from a route, persisted field, type or screen.

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
