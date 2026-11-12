# Error and Recovery Contract - Feature 014

**Status:** IN_REVIEW - not owner-accepted; does not authorize implementation; not a claim of implementation or qualification.
**Owners:** Conversation, Storage, Privacy, Desktop UI.
**Canonical owners:** [Failure Recovery](../../../docs/FAILURE-RECOVERY.md), [State Machines](../../../docs/STATE-MACHINES.md), [API](../../../docs/API.md), [Storage](../../../docs/STORAGE.md).

## 1. Classification and client rule

| Class | Examples | Server action | Client action |
|---|---|---|---|
| Owner/scope denial | foreign principal, foreign Workspace, archived Workspace | deny before mutation | show denial; no retry |
| Version conflict | stale If-Match | `STALE_VERSION`; no write | refresh row; reapply intent with a new key |
| Replay conflict | same key, different payload | `IDEMPOTENCY_CONFLICT` | new key only for changed intent |
| Invalid input | empty or over-long title, control characters, oversized query | `INVALID_ARGUMENT` | keep the edit open with the reason |
| Active turn | archive or delete while a turn is nonterminal | `CONVERSATION_TURN_IN_PROGRESS` | disable control with reason until settled |
| Not quiescent | unsettled Conversation-scoped invocation, grant, activation or open UserRequest; AgentSession not CLOSED/LOST or Runtime-local host binding cleanup unknown | `CONVERSATION_NOT_QUIESCENT` with blocker list | show blockers in impact preview; keep Delete disabled until known settled |
| Archived | submit, retry, binding change, rename on ARCHIVED | `CONVERSATION_ARCHIVED` | offer Unarchive |
| Deleted | any read or write after tombstone | 410 `CONVERSATION_DELETED` | show deleted state; remove from lists |
| Index building | backfill running | 200 with `index_state=BUILDING` | show "Search index is building" |
| Storage unavailable | writer stopped, timeout | `DEPENDENCY_UNAVAILABLE` | show offline; retry read |

## 2. Retry classes

- **SAFE_READ_RETRY:** list, search, impact, deletion status.
- **SAFE_LOCAL_REPLAY:** same principal, key and payload for rename, archive, unarchive, delete.
- **RECONCILER_RETRY:** purge targets stay PENDING until physically removed; retried with backoff; restart resumes.

## 3. Crash windows

| Window | Recovery |
|---|---|
| Before deletion transaction commits | Nothing changed; client replay with the same key re-runs it |
| After tombstone, before any purge | Fences hold; reconciler starts at Runtime startup |
| Mid-purge | Pending receipts resume; acknowledged targets are not re-processed |
| After last acknowledgement, before completion | Completion check runs on resume and commits DELETED |
| Backup taken during DELETION_PENDING, then restored | Tombstone and plan restored; purge resumes; content never readable |
| Index backfill interrupted | Watermark resumes; state stays BUILDING |

## 4. Delete fence table (completed by 014-T13)

| Reader | Owner | Fence | Test |
|---|---|---|---|
| `get_conversation`, `list_conversations` | Storage (this feature) | status check in query | to be added |
| Messages and presentation snapshot/stream | Storage / Presentation | status check | to be added |
| `rich_presentations` reads | Storage | status check | to be added |
| `load_conversation_context_for_turn` | Feature 001 | status check | to be added |
| Search | This feature | status filter in index query | to be added |
| Retrieval sources and memory candidates | Features 003, 009, 020 | `conversation_is_readable` | to be added by owners |
| Artifact association | Feature 007 | deleted-origin marker | to be added by owner |
| Diagnostics bundle | Feature 018 | excludes Conversation text | to be added by owner |
| Logs | All | canary test | to be added |

## 5. Never

Never show content after the tombstone, never acknowledge a target before physical removal, never delete Tasks, Artifacts, Evidence or memory as a side effect, never log titles or query text, never treat `BUILDING` as an empty result, and never claim LiteCowork deletion erases native-harness or external-channel copies.
