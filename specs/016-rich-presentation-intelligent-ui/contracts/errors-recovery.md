# Error and Recovery Contract - Feature 016

**Status:** IN_REVIEW - authored for owner review; does not authorize implementation.
Canonical codes: [docs/schemas/error-codes.schema.json](../../../docs/schemas/error-codes.schema.json). Recovery semantics: [docs/FAILURE-RECOVERY.md](../../../docs/FAILURE-RECOVERY.md).

## 1. Rejection reasons (`PRESENTATION_REJECTED.reason`, accepted target)

| Reason | Trigger | User-visible result |
|---|---|---|
| `TooLarge` | document over 512 KiB | semantic text |
| `NotCanonical` | bytes are not RFC 8785 canonical | semantic text |
| `DigestMismatch` | document or slice digest differs | semantic text |
| `UnknownKey` | key outside the closed object | semantic text |
| `UnsupportedBlock` | kind outside the V1 allowlist | semantic text |
| `DepthExceeded`, `TooManyBlocks` | depth over 8 or over 200 paths | semantic text |
| `BindingMismatch` | TABLE or CHART does not match the semantic table | semantic text (chart: table fallback if the table itself is valid) |
| `NonFiniteNumber` | NaN or Infinity in a value | semantic text |
| `CalcTooComplex`, `CalcUnknownVariable` | CalcAst over limits or unbound variable | semantic text |
| `ProvenanceInvalid` | provenance origin or refs not allowed | semantic text |

Media: `MEDIA_TYPE_UNSUPPORTED` (SVG, unknown, magic-byte mismatch, URL-only), `MEDIA_TOO_LARGE` (over 20 MiB or 16,384 pixels per side). No Artifact is created; the answer is unchanged.

## 2. Failure and crash matrix

| Window | Required behaviour |
|---|---|
| Binder throws or times out | Turn already settled; nothing published; counter increments |
| Crash after presentation blob write, before row commit | Orphan blob removed by the blob sweep; next binder run republishes idempotently |
| Crash after media blob write, before Artifact commit | Orphan blob removed; harness file can be re-ingested |
| Duplicate publish (same id, same digest) | Stored record returned |
| Same id, different digest | Conflict; no write |
| Two publishes race for one message | One commits; the other gets the conflict or the stored record |
| Stream gap | `stream.resync_required`; desktop reloads the Conversation presentation; no duplicate messages |
| Desktop validator rejects a published document | Semantic text shown; a local diagnostic counter increments (no network report) |
| Calculator divide by zero or non-finite | "Cannot calculate with these values" plus fallback text |

## 3. Mandatory behaviour

Presentation never blocks, retries or changes a turn. Validation failure is never shown as an answer failure. No recovery path rewrites a semantic message.

## 4. Revision 0.2 proposed denial and state recovery

LLD sections 3, 7 and 9 are the design reference; exact additional error codes require T20/T22 canonical promotion. Distinguish unsupported ingress, expired/foreign proposal scope, changed-body replay, unregistered kernel, missing metric semantics, state quota/validation refusal, stale expected revision, consent absent, and deleted/revoked namespace. No refusal fails a settled semantic turn.

| Window | Extension behavior |
|---|---|
| MCP tool accepted proposal but acknowledgement lost | Replay same digest/identity only inside live scope; no publication claim |
| Native tool_call carries arbitrary UI operation | Observation only; no execution |
| Missing unit/time/aggregation or unknown SIP kernel | Table/semantic fallback or explicit clarification |
| Expand/collapse/remount | Live controller shares state; B-40 unmount/restart discards it |
| State save commits then timeout | Keep saving/unknown; reconcile same RequestId before retry |
| Two windows update same state revision | One commits; loser retains draft and reviews current state |
| Forget races write or late success response | Deletion fence rejects stale write; UI cannot resurrect forgotten data |
| Restart/revocation/backup restore | Reauthorize exact namespace and consent; no data resurrection or native execution |
| Agent modification arrives after local edit | Preserve current values; explicit compare/review, new message lineage |

Off-by-default persistence, explicit snapshot sharing, migration allocation, quota, receipt retention and backup deletion semantics are unresolved owner gates; do not ship switches or routes before resolution.
