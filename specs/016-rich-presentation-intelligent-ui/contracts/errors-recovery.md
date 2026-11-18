# Error and Recovery Contract - Feature 016

**Status:** IN_REVIEW, revision 0.4 TARGET proposal; no new canonical error code or recovery route is approved here.
Canonical codes: [error-codes.schema.json](../../../docs/schemas/error-codes.schema.json). Cross-domain recovery: [FAILURE-RECOVERY.md](../../../docs/FAILURE-RECOVERY.md), Feature 001 Conversation, Feature 007 Artifacts, Feature 008 Workbench, Feature 014 deletion and Feature 017 backup.

## 1. Presentation rejection and safe fallback

| Condition | Required behavior |
|---|---|
| Oversize/non-canonical document, digest mismatch or unknown key | Refuse the rich document; retain the complete semantic answer. |
| Unsupported block, excessive depth/count or malformed binding | Apply only the accepted per-block fallback; otherwise show semantic answer. Never render raw HTML or interpret source as code. |
| Missing/contradictory metric semantics or unknown aggregation | Do not show the metric/chart. Show the source table or ask for clarification. |
| Non-finite number, unknown calculation variable/kernel or AST over bound | Refuse the computed view and retain declared semantic explanation/fallback. No IPC or provider call. |
| Unsupported/replayed/wrong-session provider intent | Reject the candidate by host correlation and scope; settle or preserve Conversation according to Feature 001. Never invent a new answer. |
| Unsupported media type, mismatched magic bytes, oversize media | Do not create an Artifact; leave semantic answer unchanged. Feature 007 owns any Artifact failure receipt. |

Exact machine reason names and error codes must be promoted in the canonical owner schemas by T02/T20. This table is behavior, not a second error enum.

## 2. UI state and recovery

| Failure window | Required behavior |
|---|---|
| Rich read fails or document is rejected | Preserve semantic message; retry only the read/projection, never resend the agent turn. |
| Binder throws/times out after Conversation settlement | Conversation settlement remains final; no partial presentation. A later idempotent publication may retry under the accepted binder contract. |
| Blob write succeeds but immutable presentation transaction does not | Storage's orphan cleanup handles unreferenced content; retry exact publication identity/digest. |
| Duplicate publish with same identity/digest | Return/reuse the committed presentation; no duplicate row/event. |
| Same publication identity with different digest | Conflict; preserve committed presentation and semantic answer. |
| Stream gap/reconnect | Discard transient candidate UI and rehydrate committed Conversation and presentation projections. |
| WebView validator rejects server document | Show semantic answer and increment only bounded local diagnostics; no content/credential logging. |
| Canvas expand/collapse or remount | Keep the same in-memory controller and revision for the open presentation; no new agent call. Restart returns to original inputs unless separately remembered/saved. |
| Feature 007 Save/update times out after dispatch | Keep operation pending/unknown; reconcile the same owner RequestId/digest before retry. Do not show success without receipt. Saved View update uses a new committed presentation and does not persist transient calculator values. |
| Feature 007 version conflict | Preserve the user's draft; show exact current version for review; append only after explicit retry/confirmation under owner contract. |
| Remember/Forget timeout or two-window conflict | T22 contract must reconcile exact receipt/revision; preserve local draft, fence late writes and never claim forgotten/saved until durable outcome is known. |
| Conversation modification is cancelled or send is ambiguous | Cancel changes nothing. For ambiguous send use Feature 001's request identity, retry ordinal and receipt; never create a second turn or mutate the source presentation. |
| Workspace/Conversation deletion or revocation | Feature 014/retention owner fences access and late writes; clear protected live UI; saved Artifact behavior follows explicit cross-owner source-deletion policy. |
| Backup restore or unknown Saved View schema | Reauthorize exact current version and reject incompatible schema/content; preserve any unsaved local draft separately. Saved View restore follows Artifact retention and does not restore session-modified calculator values. |

## 3. Required invariants

1. Presentation failure never changes a settled Conversation turn.
2. Unknown is not success: no Saved, Forgotten, Published or Updated label before the owning durable receipt resolves.
3. RichPresentation and ArtifactVersion are immutable; update creates a new version and conversational modification creates a new message.
4. Remembered answer state, Saved View content and executable Artifact runtime data have distinct owners, namespaces, consent and deletion rules. Saved View content preserves declared defaults only; it does not persist session-modified calculator state under B-40.
5. No recovery operation retries a provider action, Artifact write or state mutation without its exact existing identity/idempotency contract.
6. No event/log contains answer content, input values, credentials, native session handles, private transcripts or arbitrary provider payloads.

## 4. Owner gates

- The product owner directed Saved Views to remain in V1 and Feature 007 B-21 to be amended on 2026-11-17. T25's detailed Feature 007 Artifact/Library contract is drafted; cross-owner review of source deletion, receipt, retention, backup, versioning and recovery remains a gate before T01/T02.
- T01 accepts or revises the complete V1 AC set after T25; B-40 alone does not authorize implementation.
- T02 promotes canonical RichPresentation, API/event/error/UI contracts before code.
- T20 resolves provider ingress/sequence/replay evidence; unsupported ingress remains semantic-only.
- T22 can implement remembered state only after quota, migration, consent, encryption/backup, deletion, receipt and revocation behavior are accepted.
- T23 can implement Saved View commands only through Feature 007; its target routes are not served by this package.
- T24/T16 require exact packaged SYSTEM and owner USER evidence per release row; unit/schema/route tests are not qualification.
