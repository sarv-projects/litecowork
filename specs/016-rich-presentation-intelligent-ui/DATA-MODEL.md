# Data Model - Rich Presentation, Intelligent UI and Generated Media

**Status:** IN_REVIEW, revision 0.2 design proposal. Baseline B-40 requires no new migration; opt-in persistence requires a separately approved migration and retention design (T22).

Canonical definitions: [docs/RICH-RESPONSE.md](../../docs/RICH-RESPONSE.md), [docs/PRESENTATION-RUNTIME.md](../../docs/PRESENTATION-RUNTIME.md), [docs/STATE-MACHINES.md](../../docs/STATE-MACHINES.md) "RichPresentation", [docs/schemas/rich-presentation.schema.json](../../docs/schemas/rich-presentation.schema.json), [docs/ARTIFACTS-EVIDENCE.md](../../docs/ARTIFACTS-EVIDENCE.md).

## 1. Existing durable records (reused)

| Record | Location | Fields |
|---|---|---|
| `rich_presentations` | migration `rich_presentation_optional_v12` (`crates/storage-sqlite/src/lib.rs`) | presentation_id, workspace_id, conversation_id, message_id, schema_version, renderer_contract_version, semantic_content_digest, document_digest, document_size_bytes, blob ref, producer_agent_session_id, created_at |
| Blob with purpose `RICH_PRESENTATION` | `crates/storage-core/src/lib.rs` | content-addressed document bytes, media type `application/vnd.litecowork.rich-presentation+json` |
| Artifact, ArtifactVersion | `crates/storage-core/src/artifact.rs` (`ArtifactVersionRecord { artifact_id, version, resource_revision_id, input_refs, content, provenance, created_by_attempt, verification_refs, created_at }`) | Generated media is a new Artifact version; create path from Feature 007 |
| `conversation_messages` | Feature 001 | Authoritative semantic text; never modified here |

## 2. Document shape (proposed V1 profile; requires baseline owner acceptance at T01 and T02 promotion)

| Field | Bound |
|---|---|
| Document bytes | 1 to 524,288 (512 KiB), RFC 8785 canonical |
| root_blocks | at most 200, total block paths at most 200, depth at most 8 |
| JSON values | at most 30,000, JSON depth at most 32 |
| accessibility_summary | at most 4,000 characters, must match a semantic line |
| block kinds | TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE, CHART, SAFE_CALCULATOR |
| citations, actions | empty |
| block_provenance | one per block path, origin SEMANTIC_MESSAGE or MODEL_INTENT, all ref arrays empty |
| CHART | chart_kind BAR, LINE or AREA; binding to a semantic TABLE slice; accessible_summary at most 2,000 |
| SAFE_CALCULATOR | inputs at most 8 (id, label at most 200, initial value bound to a table cell), formula_ast at most 64 nodes and depth 12, output_format NUMBER, INTEGER, PERCENTAGE or CURRENCY_USD, fallback_text at most 2,000 |

## 3. Lifecycle

A RichPresentation is immutable and uniquely bound to one committed message. The trusted publisher allocates and retains its publish identity; replay uses that identity and digest. A binder upgrade does not create a second presentation for the same message. A conversational modification creates a new message and optional presentation; the original remains unchanged. Rejected candidates are not stored; a proposed `rich.presentation.rejected.v1` event requires T02 promotion and contains ids and reason only.

Calculator state is view-local React state: created on render, discarded on unmount, never persisted or transmitted.

Interactive Artifacts keep the INTERACTIVE-ARTIFACT-RUNTIME lifecycle; in V1 the only reachable state is PREVIEW_ONLY.

## 4. Generated media

| Step | Record |
|---|---|
| Sniff and bound | none (rejections emit an event with reason) |
| Store bytes | blob `blobs/<sha256>` |
| Create Artifact version | Feature 007 create transaction: Artifact, ArtifactVersion with `provenance = {conversation_id, turn_id, agent_session_id, source: "HARNESS_FILE", digest, media_type, width, height}`, association to the Conversation |

Allowed types: PNG, JPEG, WebP, GIF; at most 20 MiB; at most 16,384 pixels per side. SVG is rejected in V1.

## 5. Privacy, retention and deletion

Presentation documents are derived from semantic messages and follow their retention and the Conversation delete fences (Feature 014); deleting a message deletes its presentations. Media Artifacts follow Artifact retention. Logs and events carry ids, digests and reasons only.

## 6. Migration and compatibility

No new migration. The only storage change is the document cap constant (1 MiB to 512 KiB) for new publishes; no production rows exist today. Renderer contract version 1 documents remain valid.

## 7. Revision 0.2 proposed interaction records and lineage

The [pipeline LLD](../../docs/ACP-PRESENTATION-PIPELINE-LLD.md) sections 6-9 define the proposed fields, partition key, revision/idempotency/deletion fences, 32 KiB record/5 MiB Workspace quota proposal, consent, privacy and recovery. These are TARGET only; no table, migration number or served operation is allocated here. `PresentationInteractionState` stores only declared primitive inputs and consent under owner/device + Workspace + Conversation + message + presentation + document digest + renderer/state schema version. Derived values are recomputed; source document and message are immutable. Retention follows parent deletion and authorization, and Forget must fence late saves. Backup policy, encryption, quota and deletion receipt retention are T22 gates.

Transient controller state is shared between inline and expanded Canvas but discarded on unmount/restart under B-40. Persistence is separately opt-in and never implies sharing with the agent. A modification request explicitly snapshots selected inputs and base revision into a new Conversation turn; new-message provenance records parent presentation identity/digest and base state revision. State is not automatically migrated across document/version namespaces.
