# API and Event Contract - Feature 014

**Status:** IN_REVIEW - not owner-accepted; does not authorize implementation; not a claim of implementation or qualification.
**Canonical owners:** [API](../../../docs/API.md), [Events](../../../docs/EVENTS.md), [Schemas](../../../docs/SCHEMAS.md), [State Machines](../../../docs/STATE-MACHINES.md).

## 1. Existing routes (served today)

`POST /v1/conversations` (title 1-160 printable characters, Idempotency-Key), `GET /v1/conversations?cursor=&limit=&status=` (1-100, newest first by creation, `ACTIVE` default or `ARCHIVED`), `GET /v1/conversations/{conversation_id}`, `PATCH /v1/conversations/{conversation_id}/agent-binding` (If-Match, Idempotency-Key), turn submit/retry/cancel, `PATCH /v1/conversations/{conversation_id}` (rename), `POST .../archive`, and `POST .../unarchive`. History mutations require If-Match and Idempotency-Key. Source: `apps/litecoworkd/src/conversation_operator.rs`.

## 2. Proposed routes (not yet in source)

| Route | Headers | Body / query | Success | Errors |
|---|---|---|---|---|
| `GET /v1/conversations` | Workspace | adds `status` (`ACTIVE` default, `ARCHIVED`) | page | `INVALID_ARGUMENT` |
| `GET /v1/conversations/search` | Workspace | `q` (1-256 chars, at most 32 terms), `scope` (`ACTIVE`, `ARCHIVED`, `ALL`), `cursor`, `limit` (1-50) | `items[]` (conversation_id, title, matched TITLE or MESSAGE, message_id, snippet up to 240 chars, status), `next_cursor`, `index_state` (`READY`, `BUILDING`) | `INVALID_ARGUMENT` |
| `PATCH /v1/conversations/{conversation_id}` | If-Match, Idempotency-Key | `{ "title": string }` (1-160 printable characters after trim; null/empty is invalid) | 200 Conversation | `STALE_VERSION`, `INVALID_ARGUMENT`, `CONVERSATION_ARCHIVED`, `CONVERSATION_DELETED`, `WORKSPACE_ARCHIVED`, `IDEMPOTENCY_CONFLICT` |
| `POST /v1/conversations/{conversation_id}/archive` | If-Match, Idempotency-Key | none | 200 Conversation | `STALE_VERSION`, `CONVERSATION_TURN_IN_PROGRESS`, `CONVERSATION_DELETED` |
| `POST /v1/conversations/{conversation_id}/unarchive` | If-Match, Idempotency-Key | none | 200 Conversation | `STALE_VERSION`, `CONVERSATION_DELETED` |
| `GET /v1/conversations/{conversation_id}/deletion-impact` | Workspace | none | linked task count, artifact count, memory source count, blockers[] | `CONVERSATION_DELETED` |
| `DELETE /v1/conversations/{conversation_id}` | If-Match, Idempotency-Key | none | 202 `{ status: DELETION_PENDING, plan_id, target_count }` | `STALE_VERSION`, `CONVERSATION_TURN_IN_PROGRESS`, `CONVERSATION_NOT_QUIESCENT` |
| `GET /v1/conversations/{conversation_id}/deletion` | Workspace | none | `{ status, pending_targets }` | `NOT_FOUND` |

After the tombstone, every other Conversation route for that id returns 410 `CONVERSATION_DELETED`. Foreign owner or Workspace returns `NOT_FOUND`/`FORBIDDEN` exactly as the existing routes do; search never reveals foreign ids.

## 3. Proposed events

`conversation.renamed.v1` (title_digest only), `conversation.archived.v1`, `conversation.unarchived.v1`, `conversation.deletion_requested.v1`, `conversation.deleted.v1`. Payloads in [DATA-MODEL.md](../DATA-MODEL.md) section 5. No event carries a title, snippet or message text.

## 4. Projection rules

- The presentation snapshot and stream for a deleted Conversation emit a terminal deleted marker and no content.
- Sidebar pages exclude DELETION_PENDING and DELETED rows.
- `index_state=BUILDING` is a state, not an error; the UI must not render it as "no results".
