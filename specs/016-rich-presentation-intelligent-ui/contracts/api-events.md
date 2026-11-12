# API and Event Contract - Feature 016

**Status:** IN_REVIEW - authored for owner review; does not authorize implementation. Links to global owners are normative.

## 1. Existing served routes

| Method and path | Handler | Behaviour |
|---|---|---|
| `GET /v1/rich-presentations/{presentation_id}` | `get_rich_presentation` in `apps/litecoworkd/src/presentation_operator.rs` | Workspace-owner check (403 `FORBIDDEN`), id validation (400 `INVALID_ARGUMENT`), read through `SqliteRichPresentationStore::read_rich_presentation` (500 `INTEGRITY_FAILURE` on failure), size recheck; returns the envelope with `workspace_id, conversation_id, message_id, presentation_id, schema_version, renderer_contract_version, semantic_content_digest, document_digest, document_size_bytes, document` |
| `GET /v1/conversations/{conversation_id}/presentation` | `get_conversation_presentation` | Conversation presentation snapshot with message and presentation references |
| `POST /v1/stream` | `subscribe_stream` | Frames `stream.ready`, `projection`, `stream.resync_required`; content type `application/vnd.litecowork.operator-stream+json` |

Tauri: `get_rich_presentation` in `apps/litecowork-ui/src-tauri/src/conversation_bridge.rs` checks the envelope with `valid_rich_presentation_response` before returning it to the WebView.

## 2. Baseline: no new public route

Publishing is daemon-internal (binder after commit). Media ingestion is daemon-internal and creates Artifacts through Feature 007, whose routes serve them. This package adds no desktop-callable write route; a rendered element therefore has nothing to call.

## 3. Schema changes (accepted target, promoted by T02)

- `docs/schemas/rich-presentation.schema.json`: renderer contract version 2 adds `SAFE_CALCULATOR`; a V1 allowlist profile names TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE, CHART, SAFE_CALCULATOR; CHART binding limited to a semantic table slice; `chart_kind` limited to BAR, LINE, AREA in the V1 profile.
- `docs/schemas/calc-ast.schema.json` (new): node kinds LITERAL, VARIABLE, BINARY_OP, UNARY_OP, FUNCTION; at most 64 nodes, depth 12.
- `docs/schemas/presentation-intent.schema.json`: current schema has no calculation variant. T02/T20 must promote the bounded calculation intent before SAFE_CALCULATOR proposals. CHART_FROM_RESULT requires an actual authorized invocation/result; semantic Markdown table charts are deterministic host derivation, never a fabricated invocation.

## 4. Events

Existing: `rich.presentation.published.v1` (`presentation_id`, `conversation_id`, `message_id`, `schema_version`, `renderer_contract_version`, `semantic_content_digest`, `document_digest`, `document_size_bytes`, `producer_agent_session_id?`, `host_instruction_digest?`, `host_skill_refs[]`, `aggregate_version`).

Accepted target: `rich.presentation.rejected.v1` (`conversation_id`, `message_id`, `binder_version`, `reason`). Media Artifacts use the Feature 007 Artifact events.

## 5. Errors

Accepted target codes: `PRESENTATION_REJECTED` (internal, with `reason`), `MEDIA_TYPE_UNSUPPORTED`, `MEDIA_TOO_LARGE`. These never reach the user as a failed answer; they are counted and the semantic text is shown.

## 6. Security contract

No route, frame or event carries model HTML, scripts or URLs to be fetched. Documents carry no actions or citations in V1.

## 7. Revision 0.2 TARGET interfaces (not served)

[Pipeline LLD](../../../docs/ACP-PRESENTATION-PIPELINE-LLD.md) sections 3 and 7 specify scope, budgets, receipts and transactions. `litecowork.presentation.propose` remains ephemeral and creates no domain write/Effect/CapabilityInvocation. Exact native structured carrier or optional first-party MCP exposure is AgentModule/version-qualified; current ACP sends no MCP servers and normalizes no presentation object. Tool observations are not callable host methods.

**A-85 unresolved:** the canonical intent schema requires `submitted_at_sequence`, while the pipeline says proposal sequence is host-assigned. No model-visible MCP `inputSchema`, trusted sequence source/replay identity or complete typed acknowledgement contract is promoted. This proposal tool is not exposed until T20 resolves the model-input-to-normalized-intent mapping in the owning canonical schema. Do not add an API route for this MCP tool or a feature-local duplicate schema.

Candidate route names for promotion by T22 (not added to OpenAPI or served inventory until reviewed): `GET`, `PUT`, `DELETE /v1/rich-presentations/{presentation_id}/interaction-state`; typed Tauri get/update/forget commands. Principal/device and exact Workspace/message/document namespace are host-derived; update includes expected revision, RequestId, body digest and allowlisted inputs. Read reauthorizes; writes require ACTIVE scope; forget advances a deletion fence. Proposed response distinguishes SAVED, CONFLICT, FORGOTTEN and unavailable; a timeout remains unresolved pending same-receipt reconciliation. Exact wire fields/status/error codes/event names require owner promotion in canonical schemas/OpenAPI/event registry before implementation. Do not treat these prose names as a second wire authority.

State mutation events contain identity/revision/reason only, no input values. New conversational modification is a normal Feature 001 turn; host provenance pins parent presentation/base revision, and a resulting presentation belongs to a new message. No mutation/supersession of an existing immutable RichPresentation. Generic presentation action arrays remain empty; host-owned state/navigation/Send controls do not authorize Task/Effect/Approval operations.
