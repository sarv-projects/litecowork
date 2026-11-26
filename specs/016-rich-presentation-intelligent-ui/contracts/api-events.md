# API and Event Contract - Feature 016

**Status:** IN_REVIEW, revision 0.4 TARGET proposal. The owner selected Saved Views for V1; Feature 007's B-21/B-30 contract is drafted, while cross-owner review and whole-scope T01/T02 remain open. A-84 records user-delegated implementation direction through scoped active TASKS items; this contract does not change their exact file scopes or claim formal owner acceptance.
**Canonical owners:** [Operator OpenAPI](../../../docs/schemas/operator-api.openapi.yaml), [operator stream schema](../../../docs/schemas/operator-stream.schema.json), [domain events](../../../docs/schemas/domain-event.schema.json), [RichPresentation schema](../../../docs/schemas/rich-presentation.schema.json), [presentation intent schema](../../../docs/schemas/presentation-intent.schema.json), and Feature 001/007/008/011 contracts.

## 1. Existing routes and evidence

| Method and path | Handler | Current evidence and behavior |
|---|---|---|
| `GET /v1/rich-presentations/{presentation_id}` | `get_rich_presentation` in `apps/litecoworkd/src/presentation_operator.rs` | SERVED read: Workspace-owner check, identifier validation, storage read, size recheck and immutable message-bound envelope. No production writer is wired. |
| `GET /v1/conversations/{conversation_id}/presentation` | `get_conversation_presentation` | SERVED projection containing committed message and presentation references. |
| `POST /v1/stream` | `subscribe_stream` | SERVED stream: `stream.ready`, `projection`, `stream.resync_required`; not a rich-component draft stream. |
| Tauri `get_rich_presentation` | `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs` | CODE bridge validates the response before returning it to the WebView. |

The Rust and TypeScript contract validators accept the same renderer-v1 and renderer-v2 METRIC/CHART/SAFE_CALCULATOR structures under the 512 KiB bound; the shared corpus includes exact source, grounded metric context and initial calculator-result bindings. The response parser maps fully validated METRIC/CHART/SAFE_CALCULATOR documents to registered desktop renderer models, including the local SAFE_CALCULATOR component. T08 wires a CODE-level table/text publisher after semantic Conversation settlement. Provider structured-intent ingress and the full renderer-v2 publication journey are not integrated or qualified.

## 2. Candidate contract promotions (T02, after T25/T01)

T02 must reconcile the accepted proposal with the owner documents and promote the exact profile in existing canonical schemas. It must not create a Feature 016-only route/event/schema authority.

- **RichPresentation:** versioned closed component registry; semantic-source bindings; exact source-derived metric measure plus grounded unit/scale/time/aggregation context; registered local inputs; bounded deterministic calculation; producer/source identity; immutable message binding; complete semantic fallback.
- **Presentation intent:** one model/provider-visible input shape mapped by a trusted adapter to the host-normalized intent. Host scope, message/session identities, authorization, sequence, retry and replay identity are host-derived. A-85 remains open until T20 proves exact ingress and sequence/replay semantics.
- **Events/errors:** bounded ids, digest, version and reason only; no text, credentials, native handles, private transcript or input-state values in events/logs. Existing event names and error-code owners govern any additions.
- **Feature 007 Saved View:** Feature 007 owns media/type, create, exact-version read, immutable update from a new committed presentation, archive, Library projection, idempotency receipt, retention and recovery. Under A-84, its typed create/version request may include an optional explicitly consented numeric `input_snapshot` inside ArtifactVersion content, omitted by default and validated against Feature 016's exact declarations. Events/receipts carry IDs, versions and digests only; saved values are not sent to an agent by saving or opening. Feature 016 does not mint a second API. Cross-owner review and route promotion remain open.
- **Feature 008 Canvas:** expansion is host navigation/controller reuse; no new Feature 016 route.
- **Feature 001 modification:** a host-owned Ask a follow-up action prepares a visible, editable draft for the selected Conversation. Values are omitted by default; explicitly selected values appear as ordinary readable message text in the exact preview and are sent only by the existing ordinary Send operation after user confirmation. No presentation-specific send route, hidden request field or event carries input values.

## 3. Events and publication boundary

Existing `rich.presentation.published.v1` identifies the committed message and presentation by ids, schema/renderer versions, semantic/document digests, size and permitted producer metadata. Existing event contract remains authoritative.

`rich.presentation.rejected.v1` and presentation intent acknowledgements remain TARGET until T02/T20 promote exact names and fields in `docs/EVENTS.md` and canonical schemas. If adopted, they contain ids, schema/version and a closed rejection reason only. Publishing occurs only after semantic settlement and must be idempotent for an exact message/turn/retry identity. Partial stream candidates are not durable presentations.

## 4. No new public routes are currently served

The internal table-only RichPresentation publisher runs after semantic Conversation settlement; no model-visible structured-intent route is mounted. No route exists for Remember/Forget state or Saved View create/open/update/archive. Saved View operations are Feature 007-owned and remain TARGET until T01/T02 promotion and owner implementation; they are not Feature 016 routes. Candidate Remember/Forget route shapes are design notes only; T22 must either promote exact canonical OpenAPI/event/state contracts or remove that separate operation before implementation.

Use only Feature 007's owner routes and receipts defined in its Saved View contract. Do not guess route names from this package, call SQLite from React, or treat a successful UI click as a durable write. No hard-delete operation or implicit session-state snapshot is in the V1 Saved View contract; an optional numeric snapshot requires explicit opt-in at Save.

## 5. Error and fallback mapping

| Condition | Required semantic result |
|---|---|
| Invalid/unknown presentation schema or block | Keep complete semantic answer; accepted per-block fallback may render safe siblings. |
| Missing or conflicting metric binding | Table or clarification, never an inferred numeric claim. |
| Unsupported provider ingress, replay or wrong scope | Refuse candidate; preserve answer and do not dispatch a host action. |
| Remembered-state conflict or unavailable storage | Keep local draft and show truthful state; reconcile ambiguous receipt before retry. |
| Saved View denial, missing exact version or corrupt content | Show unavailable; never substitute a moving head or expose stale protected data. |
| Feature 001 send timeout | Use the exact ordinary Conversation request receipt/reconciliation; do not create a second turn. |

Canonical error identifiers and wire responses are assigned by their owning schemas/API contracts in T02/T20/T22/T23, not invented here.

## 6. Security and ownership

No route, frame or event carries model HTML, scripts, CSS, fetchable URLs, secrets, native handles or a callable model-authored action. ACP content/tool events are untrusted provider output; ACP does not itself provide this UI contract or a universal subagent feed. A future supported ingress must be adapter/version-qualified, correlated, bounded and normalized into the single canonical RichPresentation profile. The renderer is a passive registered host UI; only explicit host-owned controls call owner routes.

## 7. Trace references

Read-only route behavior and current UI mapping are in [UI behavior](ui-behavior.md). State transitions and timeout/deletion behavior are in [Errors and recovery](errors-recovery.md). Proposed owner tasks are T02, T20-T24 and cross-owner gate T25. All proposed operations remain TARGET and unavailable until their owners accept, contracts are promoted, routes are served and exact UI journeys are qualified.
