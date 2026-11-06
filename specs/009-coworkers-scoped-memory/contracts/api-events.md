# API and events - Coworkers and Scoped Memory

**Feature:** 009 | **Status:** DRAFT | **Source cross-check:** 2026-11-06

## Currently served Coworker API

The authenticated Coworker route group in `apps/litecoworkd/src/coworker_operator.rs` currently mounts:

| Operation | Route | Owner behavior |
|---|---|---|
| List/create | `GET/POST /v1/coworkers` | Workspace-scoped list and idempotent Coworker creation |
| Read | `GET /v1/coworkers/{coworker_id}` | Current Coworker head/definition |
| Revise | `POST /v1/coworkers/{coworker_id}/revisions` | Expected-version, idempotent immutable definition revision |
| Read revision | `GET /v1/coworkers/{coworker_id}/revisions/{revision}` | Immutable revision read |
| Change status | `POST /v1/coworkers/{coworker_id}/status` | ACTIVE/PAUSED/ARCHIVED lifecycle through ResponsibilityService |
| Set primary | `POST /v1/workspaces/{workspace_id}/primary-coworker` | Versioned Workspace primary-Coworker update |
| Presence | `GET /v1/coworkers/{coworker_id}/presence` | Mounted, but currently returns 503 `DEPENDENCY_UNAVAILABLE` after Coworker authorization because required Task/UserRequest/Attempt/Runtime projections are absent |

These routes persist Coworker configuration. They do not create a Coworker-owned Conversation, a Coworker-scoped ContextDocument or a LearningCandidate.

## ContextDocument API boundary

The generic Resource upload request accepts create-time `context_document` metadata and the OpenAPI enum names four kinds. Current `domain-workspace::valid_context_document_metadata` accepts only:
- `PERSONAL_PROFILE` with `owner_ref.kind=USER` and the authenticated `principal_id`;
- `WORKSPACE_NOTES` with `owner_ref.kind=WORKSPACE` and the selected `workspace_id`.

It rejects `COWORKER_NOTES` and `GOAL_NOTES`. Their appearance in the generic schema, event schema or Coworker Settings does not make those create paths served. ContextDocument status and purge routes operate on supported Resource metadata, not on a Coworker-private memory aggregate.

## Not currently served

There are no current Operator endpoints for:
- Coworker-owned Conversation list/get/create/new-chat or Coworker origin binding;
- Coworker-owned ContextDocument create/list/edit/revision/revoke/purge;
- Coworker-scoped ContextDocument/Resource retrieval or ContextAttachment receipts;
- CoworkerLearningPolicy revisions, LearningCandidate list/get/classify/approve/dismiss/reject;
- Coworker-private MemoryRecord list/get/edit/revoke/supersede;
- LearningService observation/classification/auto-commit/reconciliation.

The target LearningService API in `docs/LEARNING.md` §11 and proposed routes elsewhere in the target docs are not CURRENT endpoints.

## Events

This DRAFT does not add event types. Coworker and Workspace lifecycle events remain owned by their current canonical domain/store schemas. No production learning-candidate or learning-memory event producer was found. Any future learning event must be added to `docs/EVENTS.md` and `docs/schemas/domain-event.schema.json` in the same owner-reviewed change; event payloads must not contain candidate body text, raw memory values, credentials or native handles.

## Versioning and replay

Coworker mutation commands require the current expected version and an Idempotency-Key. A stale version or idempotency key reused with a different canonical payload must return a conflict; retrying the same request must recover its original committed receipt. Revision reads are immutable and served with no-store semantics.

## Contract promotion

Proposed routes remain unmarked CURRENT until the canonical owner, OpenAPI schema, event schema, SQLite migration/store, authenticated handler, UI states, recovery contract and cross-scope tests land together. See [CONTEXT.md](../../../docs/CONTEXT.md), [LEARNING.md](../../../docs/LEARNING.md) and [IMPLEMENTATION-STATUS.md](../../../docs/IMPLEMENTATION-STATUS.md).
